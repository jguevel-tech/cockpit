//! Terminaux integres PERSISTANTS : chaque terminal est une session tmux
//! (`ckpt_<id>`) sur un socket dedie (`-L cockpit`, isole du tmux perso).
//! Le serveur tmux survit a la fermeture de l'app : au redemarrage, Cockpit
//! recharge les terminaux depuis SQLite et se rattache aux sessions vivantes.
//!
//! Attach = 1 process `tmux attach` dans un PTY + 1 thread lecteur + buffer de
//! replay. Les events IPC (`terminal_output`, base64) ne partent que si une UI
//! est attachee.

pub mod history;

use crate::storage::Database;
use base64::Engine;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const REPLAY_BUFFER_MAX: usize = 200 * 1024;
const TMUX_SOCKET: &str = "cockpit";

#[derive(Default)]
pub struct TerminalState {
    live: Mutex<HashMap<i64, LiveAttach>>,
}

struct SharedBuffer {
    data: Vec<u8>,
    attached: bool,
}

struct LiveAttach {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    shared: Arc<Mutex<SharedBuffer>>,
    alive: Arc<std::sync::atomic::AtomicBool>,
    /// Kill volontaire (respawn au re-attach) : ne pas emettre terminal_exit.
    suppress_exit: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Serialize, Clone)]
pub struct TerminalInfo {
    pub id: i64,
    pub project: String,
    pub name: String,
    pub alive: bool,
    /// Un CLI d'agent LLM (claude, codex, gemini...) tourne dans la session.
    pub llm: bool,
}

#[derive(Serialize, Clone)]
struct OutputPayload {
    id: i64,
    data: String, // base64
}

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

// --- Helpers tmux (socket dedie) ---

/// Variables du runtime AppImage a NE PAS transmettre aux shells.
///
/// L'AppImage pose PYTHONHOME/PYTHONPATH/LD_LIBRARY_PATH... pointant dans son montage
/// /tmp/.mount_cockpi*. Le serveur tmux etant lance par Cockpit, chaque shell les heritait :
/// `python3` plantait dans TOUS les terminaux Cockpit (« ModuleNotFoundError: encodings »,
/// constate le 2026-08-13), et LD_LIBRARY_PATH pouvait derregler n'importe quel binaire.
const APPIMAGE_LEAKED_VARS: &[&str] = &[
    "PYTHONHOME", "PYTHONPATH", "LD_LIBRARY_PATH", "LD_PRELOAD",
    "APPDIR", "APPIMAGE", "OWD", "GTK_PATH", "GDK_PIXBUF_MODULE_FILE",
    "GIO_MODULE_DIR", "GSETTINGS_SCHEMA_DIR", "PERLLIB",
];

/// Chemin du binaire tmux a utiliser, resolu UNE FOIS par setup_bundled_tmux().
/// Vide tant que la resolution n'a pas eu lieu -> repli sur le tmux du PATH.
static TMUX_PROGRAM: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn tmux_program() -> &'static str {
    TMUX_PROGRAM.get().map(String::as_str).unwrap_or("tmux")
}

/// Choisit le tmux qui fera tourner les terminaux. Appele au demarrage, AVANT toute
/// commande tmux (purge_dead, apply_server_options).
///
/// Cockpit embarque un tmux statique (ressource AppImage) : l'utilisateur n'a RIEN a
/// installer — exiger `apt install tmux` etait le premier retour utilisateur (2026-08-14).
///
/// Ordre de priorite, pense pour la stabilite du couple client/serveur tmux (un client
/// d'une version differente du serveur echoue en "protocol version mismatch") :
/// 1. Le binaire DEJA DEPLOYE dans <app_data>/bin/tmux : les sessions vivantes tournent
///    sur lui, on s'y tient meme si un tmux systeme apparait plus tard.
/// 2. Le tmux systeme : les utilisateurs historiques ont leurs sessions dessus.
/// 3. Le binaire embarque, COPIE hors du montage AppImage : le montage disparait a la
///    fermeture de l'app alors que le serveur tmux doit survivre (persistance des
///    terminaux). L'executer depuis le montage condamnerait les sessions.
pub fn setup_bundled_tmux(app: &AppHandle) {
    let deployed = app
        .path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("bin").join("tmux"));

    if let Some(dest) = &deployed {
        if dest.exists() {
            let _ = TMUX_PROGRAM.set(dest.to_string_lossy().into_owned());
            refresh_deployed_tmux(app, dest);
            return;
        }
    }

    let system_ok = std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if system_ok {
        return;
    }

    let (Some(dest), Some(resource)) = (deployed, bundled_tmux_resource(app)) else { return };
    if copy_executable(&resource, &dest).is_ok() {
        let _ = TMUX_PROGRAM.set(dest.to_string_lossy().into_owned());
    }
}

fn bundled_tmux_resource(app: &AppHandle) -> Option<std::path::PathBuf> {
    // Les ressources gardent normalement leur chemin relatif a src-tauri
    // (resources/bin/tmux), mais on tolere aussi un aplatissement en bin/tmux.
    for rel in ["resources/bin/tmux", "bin/tmux"] {
        if let Ok(path) = app.path().resolve(rel, tauri::path::BaseDirectory::Resource) {
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

/// Met a jour le binaire deploye quand une nouvelle version de Cockpit embarque un tmux
/// different — mais SEULEMENT si aucun serveur ne tourne : remplacer le client sous un
/// serveur d'une autre version provoquerait un "protocol version mismatch" sur toutes
/// les sessions vivantes.
fn refresh_deployed_tmux(app: &AppHandle, dest: &std::path::Path) {
    let Some(resource) = bundled_tmux_resource(app) else { return };
    let same_size = match (std::fs::metadata(&resource), std::fs::metadata(dest)) {
        (Ok(a), Ok(b)) => a.len() == b.len(),
        _ => true, // dans le doute, ne rien toucher
    };
    if same_size {
        return;
    }
    // Some(vide) = reponse definitive "aucune session" (le serveur tmux s'eteint de
    // lui-meme sans session). None ou des sessions -> on ne touche a rien.
    if !matches!(tmux_alive_sessions(), Some(s) if s.is_empty()) {
        return;
    }
    let _ = copy_executable(&resource, dest);
}

/// Copie via fichier temporaire + rename : ecrire directement dans un binaire
/// potentiellement en cours d'execution echoue (ETXTBSY) ou corrompt le processus.
fn copy_executable(src: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    let dir = dest.parent().ok_or("chemin sans parent")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("tmux.new");
    std::fs::copy(src, &tmp).map_err(|e| e.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

fn tmux_cmd(args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new(tmux_program());
    // -u : force le mode UTF-8 quelle que soit la locale detectee
    cmd.arg("-u").arg("-L").arg(TMUX_SOCKET).args(args);
    cmd.env("LANG", utf8_locale()).env("LC_ALL", utf8_locale());
    // Le serveur tmux (et donc tous les shells) herite de CET environnement.
    for var in APPIMAGE_LEAKED_VARS {
        cmd.env_remove(var);
    }
    cmd
}

/// Locale UTF-8 a imposer aux terminaux : celle de l'utilisateur si elle est
/// deja en UTF-8, sinon un repli garanti disponible.
fn utf8_locale() -> String {
    for var in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if v.to_lowercase().contains("utf") {
                return v;
            }
        }
    }
    // Replis courants (au moins un existe sur toute distro moderne)
    "C.UTF-8".to_string()
}

/// Sessions tmux vivantes, ou `None` si on n'a PAS PU le savoir.
///
/// La distinction est critique : la version precedente renvoyait un ensemble vide aussi bien
/// quand aucune session n'existait que quand `tmux list-sessions` avait echoue. Les appelants
/// en concluaient que la session etait morte et SUPPRIMAIENT la ligne en base — des terminaux
/// bien vivants disparaissaient de l'interface. Constate le 2026-08-13 : sept attach en sept
/// secondes suffisent a solliciter le serveur tmux, et un seul `list-sessions` en echec
/// suffisait a perdre un terminal.
///
/// Regle : en cas de doute, on NE DETRUIT RIEN.
fn tmux_alive_sessions() -> Option<HashSet<String>> {
    let out = tmux_cmd(&["list-sessions", "-F", "#S"]).output().ok()?;
    if !out.status.success() {
        // « no server running » n'est PAS un echec : c'est une reponse definitive — pas de
        // serveur, donc zero session. La confondre avec un echec transitoire laissait des
        // lignes zombies infermables apres une mort du serveur : close() refusait de
        // supprimer faute de confirmation, et purge_dead ne purgait jamais.
        if String::from_utf8_lossy(&out.stderr).contains("no server running") {
            return Some(HashSet::new());
        }
        // Tout AUTRE code != 0 (timeout, serveur occupe...) reste un « on ne sait pas ».
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .collect(),
    )
}

/// `true` seulement si on a pu interroger tmux ET que la session n'y figure pas.
/// A utiliser partout ou la reponse declenche une SUPPRESSION.
fn tmux_session_is_gone(name: &str) -> bool {
    tmux_alive_sessions().is_some_and(|alive| !alive.contains(name))
}

/// CLIs d'agents LLM reconnus (basename du binaire ou du script node).
const LLM_COMMANDS: &[&str] = &[
    "claude", "codex", "gemini", "aider", "goose", "opencode",
    "copilot", "cursor-agent", "amp", "qwen", "ollama",
];

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn is_llm_command(cmd: &str) -> bool {
    LLM_COMMANDS.contains(&cmd)
}

/// Le BINAIRE REEL du process est-il un CLI LLM ? (`/proc/<pid>/exe`)
///
/// Necessaire parce que argv[0] peut mentir : constate le 2026-08-14, un claude natif lance
/// depuis un shell ou trainait la variable APPIMAGE (fuite corrigee en 0.6.7) s'affichait
/// comme `.../target/release/cockpit -r` dans `ps` ET dans `pane_current_command` — la
/// detection par nom de commande devenait aveugle. `/proc/exe` pointe, lui, sur le vrai
/// binaire (`~/.local/share/claude/versions/2.1.231`) : on matche chaque composant du chemin
/// (le basename est un numero de version, c'est le dossier `claude` qui signe).
fn exe_is_llm(pid: u32) -> bool {
    let Ok(exe) = std::fs::read_link(format!("/proc/{}/exe", pid)) else {
        return false;
    };
    exe.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| is_llm_command(s.trim_end_matches(".js").trim_end_matches(".mjs")))
    })
}

/// Une ligne de commande complete correspond-elle a un CLI LLM ?
/// Reconnait `claude ...`, `/usr/bin/claude`, mais aussi `node /path/gemini.js`.
fn args_are_llm(args: &str) -> bool {
    let mut tokens = args.split_whitespace();
    let Some(first) = tokens.next() else { return false };
    let base = basename(first);
    if is_llm_command(base) {
        return true;
    }
    if matches!(base, "node" | "bun" | "deno" | "python" | "python3") {
        if let Some(second) = tokens.next() {
            let script = basename(second);
            return is_llm_command(script.trim_end_matches(".js").trim_end_matches(".mjs"));
        }
    }
    false
}

/// Sessions tmux du socket cockpit dans lesquelles un CLI LLM tourne.
/// 1) `pane_current_command` (process au premier plan) couvre les binaires natifs ;
/// 2) sinon on inspecte les descendants du shell (un seul `ps` pour tout le monde)
///    pour attraper les CLIs lances via node/wrapper.
fn tmux_llm_sessions() -> HashSet<String> {
    let mut result = HashSet::new();

    let Some(out) = tmux_cmd(&["list-panes", "-a", "-F", "#{session_name}\t#{pane_pid}\t#{pane_current_command}"])
        .output()
        .ok()
        .filter(|o| o.status.success())
    else {
        return result;
    };

    let mut need_tree: Vec<(String, u32)> = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut parts = line.split('\t');
        let (Some(session), Some(pid), Some(cmd)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if is_llm_command(cmd) {
            result.insert(session.to_string());
        } else if let Ok(pid) = pid.parse::<u32>() {
            need_tree.push((session.to_string(), pid));
        }
    }
    if need_tree.is_empty() {
        return result;
    }

    // Arbre de process complet en un seul appel
    let Ok(ps) = std::process::Command::new("ps")
        .args(["-e", "-o", "pid=,ppid=,args="])
        .output()
    else {
        return result;
    };
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut args_of: HashMap<u32, String> = HashMap::new();
    for line in String::from_utf8_lossy(&ps.stdout).lines() {
        let mut parts = line.split_whitespace();
        let (Some(pid), Some(ppid)) = (
            parts.next().and_then(|p| p.parse::<u32>().ok()),
            parts.next().and_then(|p| p.parse::<u32>().ok()),
        ) else {
            continue;
        };
        let args: String = parts.collect::<Vec<_>>().join(" ");
        children.entry(ppid).or_default().push(pid);
        args_of.insert(pid, args);
    }

    for (session, root) in need_tree {
        let mut stack = vec![root];
        while let Some(pid) = stack.pop() {
            if args_of.get(&pid).map(|a| args_are_llm(a)).unwrap_or(false)
                || exe_is_llm(pid)
            {
                result.insert(session.clone());
                break;
            }
            if let Some(kids) = children.get(&pid) {
                stack.extend(kids);
            }
        }
    }
    result
}

/// Tue une session. `true` si elle n'existe plus apres l'appel.
///
/// Le retour compte : `close()` ne doit pas supprimer la ligne en base si la session a
/// survecu, sinon elle devient une orpheline — une session vivante que l'interface ne peut
/// plus jamais afficher, donc un shell qui tourne indefiniment sans que personne le sache.
fn tmux_kill_session(name: &str) -> bool {
    let _ = tmux_cmd(&["kill-session", "-t", name]).output();
    // On verifie plutot que de faire confiance au code de retour : `kill-session` sort en
    // erreur quand la session n'existait deja plus, ce qui est le resultat souhaite.
    tmux_session_is_gone(name)
}

/// Sessions de notre socket sans ligne en base : injoignables depuis l'interface, donc
/// perdues. Elles proviennent de tout chemin ou la ligne a disparu sans que la session soit
/// tuee — notamment l'ancien bug de suppression sur echec transitoire de `list-sessions`.
/// Constate le 2026-08-13 : 3 terminaux affiches, 14 sessions vivantes.
fn orphan_sessions(db: &Database) -> Vec<String> {
    let Some(alive) = tmux_alive_sessions() else { return Vec::new() };
    let known: HashSet<String> = db
        .get_terminal_rows(None)
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.tmux_name)
        .collect();
    alive
        .into_iter()
        // Uniquement NOS sessions : le prefixe garantit qu'on ne touche jamais a une session
        // creee a la main sur ce socket.
        .filter(|s| s.starts_with("ckpt_") && !known.contains(s))
        .collect()
}


/// Config tmux minimale : pas de barre de statut (l'UI a deja ses onglets),
/// molette active (copy-mode auto), gros historique, presse-papier via OSC 52
/// (la selection souris est copiee vers le systeme, voir set_clipboard).
/// Fichier gere par Cockpit : reecrit a chaque demarrage (source de verite ici).
const TMUX_CONF: &str = "set -g status off\n\
set -g mouse on\n\
set -g history-limit 10000\n\
set -g escape-time 10\n\
# TAILLE DE FENETRE : PAS d'option `window-size manual` ici (NE PAS LA REMETTRE).\n\
# `set -g window-size manual` dans un fichier de conf FAIT PLANTER tmux 3.4 au demarrage\n\
# du serveur (« server exited unexpectedly ») : plus aucun terminal ne pouvait etre cree.\n\
# Prouve par bissection le 2026-08-13 — la ligne seule suffit a tuer le serveur.\n\
# La taille est pilotee par Cockpit via `resize-window` avant chaque attach (mod.rs), qui\n\
# marque la fenetre en taille manuelle PAR FENETRE : meme effet anti-saut-de-ligne, sans\n\
# le plantage.\n\
set -s set-clipboard on\n\
set -sa terminal-features ',xterm*:clipboard:RGB:strikethrough:usstyle'\n\
set -g mode-style 'bg=#4f7cff,fg=#ffffff'\n\
# Selection souris : elle RESTE affichee au relachement (pas de copie auto).\n\
# C'est l'utilisateur qui copie : Ctrl+C (avec selection) ou clic droit Cockpit.\n\
bind -T copy-mode MouseDragEnd1Pane send -X stop-selection\n\
bind -T copy-mode-vi MouseDragEnd1Pane send -X stop-selection\n\
bind -T copy-mode C-c send -X copy-pipe-and-cancel 'tmux load-buffer -w -'\n\
bind -T copy-mode-vi C-c send -X copy-pipe-and-cancel 'tmux load-buffer -w -'\n\
bind -T copy-mode Escape send -X cancel\n\
bind -T copy-mode-vi Escape send -X cancel\n\
# Pas de menus contextuels tmux au clic droit (Cockpit affiche le sien)\n\
unbind -n MouseDown3Pane\n\
unbind -n M-MouseDown3Pane\n\
unbind -n MouseDrag3Pane\n";

fn tmux_conf_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    let path = dir.join("tmux.conf");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, TMUX_CONF);
    Some(path)
}

/// La conf n'est lue qu'au DEMARRAGE du serveur tmux — or il survit a l'app
/// (c'est le but). On applique donc aussi les options au serveur deja en route.
pub fn apply_server_options() {
    for args in [
        ["set", "-s", "set-clipboard", "on"].as_slice(),
        ["set", "-sa", "terminal-features", ",xterm*:clipboard:RGB:strikethrough:usstyle"].as_slice(),
        ["set", "-g", "mode-style", "bg=#4f7cff,fg=#ffffff"].as_slice(),
        // PAS de ["set","-g","window-size","manual"] ici non plus : l'option plante tmux 3.4
        // (voir la conf generee). Le dimensionnement passe par resize-window avant l'attach.
        // La selection reste affichee au relachement ; copie a la demande
        ["bind", "-T", "copy-mode", "MouseDragEnd1Pane", "send", "-X", "stop-selection"].as_slice(),
        ["bind", "-T", "copy-mode-vi", "MouseDragEnd1Pane", "send", "-X", "stop-selection"].as_slice(),
        // Ctrl+C copie QUAND une selection est active (sinon SIGINT normal)
        ["bind", "-T", "copy-mode", "C-c", "send", "-X",
         "copy-pipe-and-cancel", "tmux load-buffer -w -"].as_slice(),
        ["bind", "-T", "copy-mode-vi", "C-c", "send", "-X",
         "copy-pipe-and-cancel", "tmux load-buffer -w -"].as_slice(),
        ["bind", "-T", "copy-mode", "Escape", "send", "-X", "cancel"].as_slice(),
        ["bind", "-T", "copy-mode-vi", "Escape", "send", "-X", "cancel"].as_slice(),
        // Pas de menus contextuels tmux au clic droit
        ["unbind", "-n", "MouseDown3Pane"].as_slice(),
        ["unbind", "-n", "M-MouseDown3Pane"].as_slice(),
        ["unbind", "-n", "MouseDrag3Pane"].as_slice(),
    ] {
        let _ = tmux_cmd(args).output();
    }

    // Un serveur DEJA vivant a pu etre demarre par une version qui fuyait l'environnement
    // AppImage : on marque ces variables pour retrait, les prochains shells naitront propres.
    // (Les shells existants gardent leur environnement, c'est inevitable.)
    for var in APPIMAGE_LEAKED_VARS {
        let _ = tmux_cmd(&["set-environment", "-g", "-r", var]).output();
    }
}

impl TerminalState {
    /// Verifie que tmux est disponible (message clair sinon). Avec le tmux embarque,
    /// ce cas ne peut plus arriver dans l'AppImage — il reste possible en build de dev
    /// (--no-bundle, pas de ressource) sur une machine sans tmux.
    fn ensure_tmux() -> Result<(), String> {
        std::process::Command::new(tmux_program())
            .arg("-V")
            .output()
            .map_err(|_| "tmux introuvable — installe-le : sudo apt-get install tmux".to_string())?;
        Ok(())
    }

    /// Au demarrage : supprime les lignes dont la session tmux n'existe plus
    /// (reboot machine, kill manuel...).
    pub fn purge_dead(db: &Database) {
        // Sans reponse de tmux, on ne purge RIEN : mieux vaut une ligne obsolete qu'un
        // terminal vivant supprime.
        let Some(alive) = tmux_alive_sessions() else { return };
        if let Ok(rows) = db.get_terminal_rows(None) {
            for row in rows {
                if !alive.contains(&row.tmux_name) {
                    let _ = db.delete_terminal_row(row.id);
                }
            }
        }

        // Nettoyage SYMETRIQUE : une ligne sans session etait deja traitee, mais une session
        // sans ligne restait a tourner indefiniment. Elle est injoignable depuis l'interface —
        // aucun onglet ne peut plus l'afficher — donc son shell est perdu et ne fait que
        // consommer de la memoire. Ne concerne que nos sessions `ckpt_*`.
        for session in orphan_sessions(db) {
            tmux_kill_session(&session);
        }
    }

    pub fn create(
        &self,
        app: AppHandle,
        db: &Database,
        project: String,
        cwd: String,
        cols: u16,
        rows: u16,
        init_command: Option<String>,
    ) -> Result<i64, String> {
        Self::ensure_tmux()?;
        let row = db.create_terminal_row(&project)?;

        let conf = tmux_conf_path(&app);
        let mut args: Vec<String> = vec!["-u".into(), "-L".into(), TMUX_SOCKET.into()];
        if let Some(conf) = &conf {
            args.push("-f".into());
            args.push(conf.to_string_lossy().to_string());
        }
        args.extend(["new-session", "-A", "-s", &row.tmux_name].map(String::from));
        if std::path::Path::new(&cwd).is_dir() {
            args.push("-c".into());
            args.push(cwd);
        }

        match self.spawn_attach(&app, db, row.id, &row.tmux_name, &args, cols, rows, init_command) {
            Ok(()) => Ok(row.id),
            Err(e) => {
                let _ = db.delete_terminal_row(row.id);
                Err(e)
            }
        }
    }

    /// Spawn le client tmux (new-session ou attach) dans un PTY + thread lecteur.
    fn spawn_attach(
        &self,
        app: &AppHandle,
        db: &Database,
        id: i64,
        tmux_name: &str,
        args: &[String],
        cols: u16,
        rows: u16,
        init_command: Option<String>,
    ) -> Result<(), String> {
        let program = tmux_program();
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("openpty: {}", e))?;

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        cmd.env("TERM", "xterm-256color");
        // Locale UTF-8 forcee : lance depuis un .desktop, l'app peut heriter d'un
        // environnement sans LANG -> tmux compte chaque octet UTF-8 comme une
        // colonne (accents decales). On garantit une locale UTF-8 valide.
        cmd.env("LANG", utf8_locale());
        cmd.env("LC_ALL", utf8_locale());
        // Ce client peut etre celui qui DEMARRE le serveur : meme nettoyage que tmux_cmd,
        // sinon l'environnement AppImage fuite dans tous les shells (python3 casse).
        for var in APPIMAGE_LEAKED_VARS {
            cmd.env_remove(var);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn tmux: {}", e))?;
        drop(pair.slave);

        let killer = child.clone_killer();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("clone reader: {}", e))?;
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("take writer: {}", e))?;

        // Commande initiale via tmux send-keys vers la SESSION (pas via le PTY
        // du client : celui-ci peut etre tue/respawne par attach() avant que le
        // shell n'ait lu). On attend brievement que la session existe.
        if let Some(cmd_line) = init_command {
            let session = tmux_name.to_string();
            std::thread::spawn(move || {
                for _ in 0..40 {
                    // Ici on ATTEND une presence : un echec de tmux equivaut a "pas encore
                    // pret", donc on retente. `is_some_and` s'en charge sans rien detruire.
                    if tmux_alive_sessions().is_some_and(|a| a.contains(&session)) {
                        let _ = tmux_cmd(&["send-keys", "-t", &session, &cmd_line, "Enter"]).output();
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            });
        }

        // attached=true DES LA NAISSANCE : les toutes premieres sequences du
        // client tmux (ecran alternatif, activation souris, redraw) doivent
        // atteindre xterm, sinon la molette et l'affichage initial sont casses.
        let shared = Arc::new(Mutex::new(SharedBuffer { data: Vec::new(), attached: true }));
        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let suppress_exit = Arc::new(std::sync::atomic::AtomicBool::new(false));

        {
            let shared = shared.clone();
            let alive = alive.clone();
            let suppress_exit = suppress_exit.clone();
            let app = app.clone();
            let db = db.clone();
            let tmux_name = tmux_name.to_string();
            std::thread::spawn(move || {
                let mut chunk = [0u8; 8192];
                loop {
                    match reader.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let mut guard = shared.lock().unwrap();
                            guard.data.extend_from_slice(&chunk[..n]);
                            if guard.data.len() > REPLAY_BUFFER_MAX {
                                let excess = guard.data.len() - REPLAY_BUFFER_MAX / 2;
                                guard.data.drain(..excess);
                            }
                            if guard.attached {
                                // Sous le lock : ordre garanti vis-a-vis d'attach()
                                let _ = app.emit(
                                    "terminal_output",
                                    OutputPayload { id, data: b64(&chunk[..n]) },
                                );
                            }
                        }
                    }
                }
                alive.store(false, std::sync::atomic::Ordering::SeqCst);
                // Kill volontaire (respawn d'un client frais) : silencieux.
                if suppress_exit.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                // Shell termine (session tmux disparue) -> on nettoie la ligne.
                // Client simplement detache (session encore la) -> on garde.
                // tmux injoignable -> on garde AUSSI : c'est ce qui faisait disparaitre des
                // terminaux vivants quand `list-sessions` echouait sous la charge des attach.
                if tmux_session_is_gone(&tmux_name) {
                    let _ = db.delete_terminal_row(id);
                }
                let _ = app.emit("terminal_exit", id);
            });
        }

        self.live.lock().unwrap().insert(
            id,
            LiveAttach { writer, master: pair.master, killer, shared, alive, suppress_exit },
        );
        Ok(())
    }

    /// Attache l'UI. Si aucun client tmux n'est vivant pour cette session
    /// (app relancee), on en respawn un — tmux repeint l'ecran tout seul.
    pub fn attach(
        &self,
        app: AppHandle,
        db: &Database,
        id: i64,
        cols: u16,
        rows: u16,
    ) -> Result<String, String> {
        let row = db.get_terminal_row(id)?;

        // Uniquement si tmux a REPONDU que la session n'existe pas. S'il est injoignable, on
        // tente l'attach : au pire il echoue, ce qui est reversible — contrairement a une
        // suppression de ligne.
        if tmux_session_is_gone(&row.tmux_name) {
            let _ = db.delete_terminal_row(id);
            return Err("session terminee (le processus n'existe plus)".into());
        }

        // REUTILISER un client vivant, ne JAMAIS le remplacer (revirement du 2026-08-13).
        //
        // L'ancienne doctrine — « toujours un client frais, lui seul renvoie la sequence
        // d'initialisation complete » — reste vraie pour un xterm NEUF. Mais le frontend
        // conserve desormais les xterm dans un pool persistant (TerminalTab, script module) :
        // le xterm d'origine revit au retour sur l'onglet, avec ses modes deja inities.
        //
        // Et tuer/respawner avait un cout invisible, prouve par mesure : tmux SYNTHETISE des
        // evenements focus (in/out) vers l'application du pane a chaque attache/detache de
        // client — meme avec focus-events off, qui ne gouverne que le focus du terminal
        // exterieur. Un cycle attache/tue/rattache SANS AUCUNE entree fait reagir claude
        // (re-render), et ce re-render laissait des sauts de ligne a chaque switch.
        if let Some(l) = self.live.lock().unwrap().get(&id) {
            if l.alive.load(std::sync::atomic::Ordering::SeqCst) {
                return Ok(String::new());
            }
        }
        // Client mort (serveur tmux redemarre, crash...) : on nettoie l'entree et on respawn.
        self.live.lock().unwrap().remove(&id);

        // Taille fixee AVANT l'attach (NE PAS DEPLACER APRES).
        //
        // `resize-window` marque la fenetre en taille manuelle PAR FENETRE (l option globale
        // plante tmux 3.4, voir la conf generee). Si on attachait d abord, le client se
        // dessinerait a l ancienne taille puis le redimensionnement provoquerait un SIGWINCH :
        // le TUI se redessine et laisse un saut de ligne. Bug constate en switchant de
        // terminal — les sessions portaient cinq tailles differentes.
        let _ = tmux_cmd(&[
            "resize-window",
            "-t",
            &row.tmux_name,
            "-x",
            &cols.to_string(),
            "-y",
            &rows.to_string(),
        ])
        .output();

        // -u : force UTF-8. -d : detache les autres clients (tailles concurrentes)
        let args: Vec<String> =
            ["-u", "-L", TMUX_SOCKET, "attach-session", "-d", "-t", &row.tmux_name]
                .map(String::from)
                .to_vec();
        self.spawn_attach(&app, db, id, &row.tmux_name, &args, cols, rows, None)?;

        Ok(String::new())
    }

    pub fn detach(&self, id: i64) {
        let live = self.live.lock().unwrap();
        if let Some(l) = live.get(&id) {
            l.shared.lock().unwrap().attached = false;
        }
    }

    pub fn write(&self, id: i64, data: &str) -> Result<(), String> {
        let mut live = self.live.lock().unwrap();
        let l = live.get_mut(&id).ok_or("terminal non attache")?;
        l.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn resize(&self, db: &Database, id: i64, cols: u16, rows: u16) -> Result<(), String> {
        {
            let live = self.live.lock().unwrap();
            let l = live.get(&id).ok_or("terminal non attache")?;
            l.master
                .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
                .map_err(|e| e.to_string())?;
        }
        // La fenetre etant en taille manuelle (resize-window a l attach), redimensionner le
        // PTY du client ne suffit plus : la fenetre tmux ne suit pas, il faut le lui dire.
        if let Ok(row) = db.get_terminal_row(id) {
            let _ = tmux_cmd(&[
                "resize-window",
                "-t",
                &row.tmux_name,
                "-x",
                &cols.to_string(),
                "-y",
                &rows.to_string(),
            ])
            .output();
        }
        Ok(())
    }

    pub fn rename(&self, db: &Database, id: i64, name: &str) -> Result<(), String> {
        let clean: String = name.trim().chars().take(40).collect();
        db.rename_terminal_row(id, &clean)
    }

    pub fn close(&self, db: &Database, id: i64) -> Result<(), String> {
        if let Some(mut l) = self.live.lock().unwrap().remove(&id) {
            let _ = l.killer.kill();
        }
        // La session est tuee AVANT de toucher a la base, et la ligne n'est supprimee que si
        // elle a bien disparu. L'ordre inverse creait des orphelines : quand la lecture de la
        // ligne ou le kill echouait, la ligne partait quand meme et la session survivait —
        // injoignable pour toujours, avec son shell qui continue de tourner.
        let row = db.get_terminal_row(id)?;
        if !tmux_kill_session(&row.tmux_name) {
            return Err(format!(
                "la session {} n'a pas pu etre arretee ; le terminal est conserve pour permettre un nouvel essai",
                row.tmux_name
            ));
        }
        db.delete_terminal_row(id)
    }

    /// Copie la selection copy-mode de la session vers le presse-papier systeme
    /// (pipe vers `tmux set-buffer -w` -> OSC 52 -> set_clipboard cote app).
    /// Sans selection active, tmux ignore la commande : sans effet, pas d'erreur.
    pub fn copy_selection(&self, db: &Database, id: i64) -> Result<(), String> {
        let row = db.get_terminal_row(id).map_err(|e| e.to_string())?;
        let _ = tmux_cmd(&[
            "send-keys", "-t", &row.tmux_name, "-X",
            "copy-pipe-and-cancel", "tmux load-buffer -w -",
        ])
        .output();
        Ok(())
    }

    /// Recherche dans l'historique du terminal via la recherche NATIVE du copy-mode tmux :
    /// c'est tmux qui possede le scrollback (le buffer xterm ne contient qu'un ecran en
    /// mode alternatif), donc c'est lui qui cherche, surligne les occurrences et affiche
    /// le compteur (n/N) dans le coin du pane. Aucun octet n'est injecte dans le shell.
    ///
    /// action : "start" (entre en copy-mode et cherche `query` vers le haut),
    /// "next"/"prev" (occurrence plus ancienne / plus recente), "cancel" (sort du copy-mode).
    pub fn search(&self, db: &Database, id: i64, action: &str, query: &str) -> Result<(), String> {
        let row = db.get_terminal_row(id).map_err(|e| e.to_string())?;
        let target = row.tmux_name.clone();
        let run = |args: &[&str]| -> Result<(), String> {
            let out = tmux_cmd(args).output().map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
            }
            Ok(())
        };
        match action {
            "start" => {
                if query.trim().is_empty() {
                    return Err("recherche vide".into());
                }
                // no-op si le pane est deja en copy-mode
                run(&["copy-mode", "-t", &target])?;
                // Variante -text : sous-chaine LITTERALE, pas une regex — c'est une
                // recherche d'utilisateur, "1.2.3" ne doit pas matcher "1x2y3".
                run(&["send-keys", "-t", &target, "-X", "search-backward-text", query])
            }
            "next" => run(&["send-keys", "-t", &target, "-X", "search-again"]),
            "prev" => run(&["send-keys", "-t", &target, "-X", "search-reverse"]),
            "cancel" => {
                // Best effort : hors copy-mode, sortir n'a rien a faire (et ne doit pas toaster)
                let _ = tmux_cmd(&["send-keys", "-t", &target, "-X", "cancel"]).output();
                Ok(())
            }
            _ => Err(format!("action de recherche inconnue: {}", action)),
        }
    }

    /// Le programme QUI TOURNE DANS la session tmux est-il en ecran alternatif
    /// (vim, claude, htop...) ? Ne pas confondre avec le buffer xterm : le
    /// client tmux met TOUJOURS le terminal hote en ecran alternatif.
    pub fn inner_alternate(&self, db: &Database, id: i64) -> bool {
        let Ok(row) = db.get_terminal_row(id) else { return false };
        tmux_cmd(&["display-message", "-p", "-t", &row.tmux_name, "#{alternate_on}"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
            .unwrap_or(false)
    }

    pub fn list(&self, db: &Database, project: Option<&str>) -> Vec<TerminalInfo> {
        // tmux injoignable -> on annonce les terminaux comme VIVANTS plutot que morts. Le
        // store `terminals` du frontend filtre sur ce flag et les ferait disparaitre de la
        // sidebar a chaque hoquet de tmux.
        let alive = tmux_alive_sessions();
        let llm = tmux_llm_sessions();
        db.get_terminal_rows(project)
            .unwrap_or_default()
            .into_iter()
            .map(|row| TerminalInfo {
                id: row.id,
                project: row.project,
                name: row.name,
                alive: alive.as_ref().is_none_or(|a| a.contains(&row.tmux_name)),
                llm: llm.contains(&row.tmux_name),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_llm_command_lines() {
        assert!(args_are_llm("claude --resume abc"));
        assert!(args_are_llm("/usr/local/bin/claude"));
        assert!(args_are_llm("node /home/x/.npm/bin/gemini.js chat"));
        assert!(args_are_llm("python3 /opt/aider serve"));
        assert!(args_are_llm("codex"));
    }

    #[test]
    fn ignores_normal_commands() {
        assert!(!args_are_llm("zsh"));
        assert!(!args_are_llm("vim notes-claude.md"));
        assert!(!args_are_llm("tail -f claude.log"));
        assert!(!args_are_llm("node server.js"));
        assert!(!args_are_llm(""));
    }
}
