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

fn tmux_cmd(args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new("tmux");
    // -u : force le mode UTF-8 quelle que soit la locale detectee
    cmd.arg("-u").arg("-L").arg(TMUX_SOCKET).args(args);
    cmd.env("LANG", utf8_locale()).env("LC_ALL", utf8_locale());
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
    // "no server running" sort en code != 0 : c'est un echec, pas une absence de session.
    if !out.status.success() {
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
            if args_of.get(&pid).map(|a| args_are_llm(a)).unwrap_or(false) {
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

fn tmux_kill_session(name: &str) {
    let _ = tmux_cmd(&["kill-session", "-t", name]).output();
}


/// Config tmux minimale : pas de barre de statut (l'UI a deja ses onglets),
/// molette active (copy-mode auto), gros historique, presse-papier via OSC 52
/// (la selection souris est copiee vers le systeme, voir set_clipboard).
/// Fichier gere par Cockpit : reecrit a chaque demarrage (source de verite ici).
const TMUX_CONF: &str = "set -g status off\n\
set -g mouse on\n\
set -g history-limit 10000\n\
set -g escape-time 10\n\
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
}

impl TerminalState {
    /// Verifie que tmux est disponible (message clair sinon).
    fn ensure_tmux() -> Result<(), String> {
        std::process::Command::new("tmux")
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
        let program = "tmux";
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

        // TOUJOURS repartir d'un client tmux FRAIS : lui seul renvoie la
        // sequence d'initialisation complete (ecran alternatif, modes souris,
        // redraw) dont le nouveau xterm a besoin. Un client conserve ne
        // redessine jamais spontanement et ses modes sont perdus pour l'UI.
        if let Some(old) = self.live.lock().unwrap().remove(&id) {
            old.suppress_exit.store(true, std::sync::atomic::Ordering::SeqCst);
            let mut killer = old.killer;
            let _ = killer.kill();
        }

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

    pub fn resize(&self, id: i64, cols: u16, rows: u16) -> Result<(), String> {
        let live = self.live.lock().unwrap();
        let l = live.get(&id).ok_or("terminal non attache")?;
        l.master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())
    }

    pub fn rename(&self, db: &Database, id: i64, name: &str) -> Result<(), String> {
        let clean: String = name.trim().chars().take(40).collect();
        db.rename_terminal_row(id, &clean)
    }

    pub fn close(&self, db: &Database, id: i64) -> Result<(), String> {
        if let Some(mut l) = self.live.lock().unwrap().remove(&id) {
            let _ = l.killer.kill();
        }
        if let Ok(row) = db.get_terminal_row(id) {
            tmux_kill_session(&row.tmux_name);
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
