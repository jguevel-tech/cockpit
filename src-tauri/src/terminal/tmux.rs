//! Implementation tmux du trait `Terminaux` (voir `interface.rs`).
//!
//! Chaque terminal est une session tmux (`ckpt_<id>`) sur un socket dedie (`-L cockpit`,
//! isole du tmux perso). Le serveur tmux survit a la fermeture de l'app : au redemarrage,
//! Cockpit recharge les terminaux depuis SQLite et se rattache aux sessions vivantes.
//!
//! Attach = 1 process `tmux attach` dans un PTY + 1 thread lecteur + 1 thread emetteur
//! qui regroupe. Les events IPC (`terminal_output`, base64) ne partent que si une UI est
//! attachee. Il n'y a PLUS de buffer de replay : il n'etait plus lu par personne depuis
//! que le frontend garde ses xterm dans un pool, et il recopiait 100 Ko a chaque lecture
//! du PTY.
//!
//! Ce fichier entier disparait a l'etape C du chantier des terminaux
//! (`docs/portabilite/plan-terminaux.md`) : tout ce qui est specifique a tmux doit rester
//! DEDANS, jamais remonter dans `interface.rs` ni dans `lib.rs`.

use super::agents_llm::{est_commande_llm, ArbreProcess};
use super::environnement;
use super::interface::{ActionRecherche, Creation, Taille, TerminalInfo, Terminaux};
use crate::storage::Database;
use base64::Engine;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use tauri::{AppHandle, Emitter, Manager};

const TMUX_SOCKET: &str = "cockpit";

/// Taille du tampon de lecture du PTY. Ne change RIEN a la latence : `read()` rend la main
/// des qu'UN octet est disponible, l'echo d'une touche part donc toujours seul. Mais sur une
/// rafale, chaque lecture devient un evenement de moins — banc du 2026-08-20 sur une rafale
/// de 1,9 Mo a travers un vrai PTY : 2547 evenements en 8 Ko, 3 avec ce tampon et le
/// regroupement de `pomper`.
const PTY_READ_BUF: usize = 64 * 1024;

/// Au-dela de ce volume en attente, on considere qu'une RAFALE est en cours et on laisse
/// `PTY_GROUP_WAIT` d'accumulation avant d'emettre. En dessous, emission immediate.
///
/// 8 Ko d'un coup ne peuvent pas etre de la frappe : l'echo d'une touche fait quelques
/// octets, un redraw de prompt quelques centaines. Le seuil ne se declenche donc jamais
/// sur le chemin de frappe, dont la latence reste inchangee.
const PTY_GROUP_THRESHOLD: usize = 8 * 1024;

/// Fenetre d'accumulation en rafale. 2 ms : un huitieme d'image a 60 Hz, donc invisible,
/// et assez pour agreger plusieurs lectures. NE PAS monter a 20-50 ms « pour mieux
/// regrouper » : ce serait payer en latence percue ce qu'on gagne en nombre d'evenements.
const PTY_GROUP_WAIT: std::time::Duration = std::time::Duration::from_millis(2);

#[derive(Default)]
pub struct TerminauxTmux {
    live: Mutex<HashMap<i64, LiveAttach>>,
}

/// File d'attente entre le thread lecteur du PTY et le thread qui emet vers le webview.
/// UN seul tampon FIFO : l'ordre des octets est strict par construction.
struct FileSortie {
    en_attente: Vec<u8>,
    /// Une UI est attachee : sinon on VIDE quand meme le PTY (sans quoi tmux se bloquerait)
    /// mais on ne pousse rien vers le webview.
    attached: bool,
    /// Le PTY est ferme : l'emetteur finit d'ecouler la file puis s'arrete.
    fini: bool,
}

struct LiveAttach {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    shared: Arc<(Mutex<FileSortie>, Condvar)>,
    alive: Arc<std::sync::atomic::AtomicBool>,
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
fn setup_bundled_tmux(app: &AppHandle) {
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
    cmd.env("LANG", environnement::locale_utf8())
        .env("LC_ALL", environnement::locale_utf8());
    // Le serveur tmux (et donc tous les shells) herite de CET environnement.
    environnement::appliquer(&mut cmd);
    cmd
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
/// tmux dit-il DEFINITIVEMENT qu'il n'y a aucun serveur sur notre socket ?
///
/// Distinguer cette reponse d'un echec transitoire est ce qui permet de purger : la
/// confondre avec « on ne sait pas » laissait des lignes zombies infermables (close()
/// refusait de supprimer faute de confirmation, purge_dead ne purgait jamais).
///
/// tmux 3.4 a DEUX formulations pour la meme realite, mesurees le 2026-08-20 :
/// - serveur mort mais fichier de socket encore la -> « no server running on <chemin> » ;
/// - fichier de socket absent -> « error connecting to <chemin> (No such file or directory) ».
/// Seule la premiere etait reconnue, et c'est la SECONDE qui se produit apres un
/// redemarrage de la machine (/tmp est vide) : les terminaux de la session precedente
/// restaient donc affiches sans pouvoir etre fermes.
fn absence_definitive(stderr: &str) -> bool {
    stderr.contains("no server running")
        || (stderr.contains("error connecting to") && stderr.contains("No such file or directory"))
}

fn tmux_alive_sessions() -> Option<HashSet<String>> {
    let out = tmux_cmd(&["list-sessions", "-F", "#S"]).output().ok()?;
    if !out.status.success() {
        if absence_definitive(&String::from_utf8_lossy(&out.stderr)) {
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

/// Sessions tmux du socket cockpit dans lesquelles un CLI LLM tourne.
/// 1) `pane_current_command` (process au premier plan) couvre les binaires natifs ;
/// 2) sinon on descend les descendants du shell pour attraper les CLIs lances via
///    node/wrapper — et parce qu'argv mentait sur 4 sessions sur 9 a la mesure du
///    2026-08-20 (elles annoncaient `cockpit`), ce second passage est le cas NORMAL,
///    pas l'exception. D'ou l'importance qu'il soit bon marche.
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
        if est_commande_llm(cmd) {
            result.insert(session.to_string());
        } else if let Ok(pid) = pid.parse::<u32>() {
            need_tree.push((session.to_string(), pid));
        }
    }
    if need_tree.is_empty() {
        return result;
    }

    let arbre = ArbreProcess::nouveau();
    for (session, root) in need_tree {
        if arbre.contient_un_llm(root) {
            result.insert(session);
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
# Clic molette et clic droit sont geres par Cockpit : pas de collage ni de menu tmux\n\
unbind -n MouseDown2Pane\n\
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

/// Options a (re)poser sur un serveur tmux DEJA en route. Sortie en const pour que le
/// test qui verrouille l'enchainement puisse les relire.
const OPTIONS_SERVEUR: &[&[&str]] = &[
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
    // Le collage du clic molette est fait par Cockpit (meme chemin que « Coller »
    // du clic droit) : laisser tmux coller de son cote donnait deux collages.
    ["unbind", "-n", "MouseDown2Pane"].as_slice(),
    ["unbind", "-n", "MouseDown3Pane"].as_slice(),
    ["unbind", "-n", "M-MouseDown3Pane"].as_slice(),
    ["unbind", "-n", "MouseDrag3Pane"].as_slice(),
];

/// La conf n'est lue qu'au DEMARRAGE du serveur tmux — or il survit a l'app
/// (c'est le but). On applique donc aussi les options au serveur deja en route.
///
/// Les echecs sont JOURNALISES, jamais avales : `unbind -n MouseDown2Pane` est ce qui
/// empeche tmux de coller de son cote au clic molette, et un echec silencieux ici
/// ramene le double collage sans laisser la moindre trace — le symptome a deja ete
/// rediagnostique de zero deux fois. Rien n'est notifie dans l'interface : l'utilisateur
/// n'a aucune action a faire, la panne se lit dans <app_data>/logs/cockpit.log.
///
/// Aucun serveur en route = rien a appliquer, et surtout rien a signaler : la conf sera
/// lue a la creation du serveur. Sans cette sortie, chaque premier lancement journalisait
/// une quinzaine de faux echecs (mesure : sur un socket absent, tmux 3.4 rend 1 pour
/// TOUTES les commandes).
fn apply_server_options(app: &AppHandle) {
    if tmux_alive_sessions().is_none_or(|sessions| sessions.is_empty()) {
        return;
    }
    let mut commandes: Vec<&[&str]> = OPTIONS_SERVEUR.to_vec();

    let retraits: Vec<[&str; 4]> = environnement::VARIABLES_APPIMAGE
        .iter()
        .map(|var| ["set-environment", "-g", "-r", *var])
        .collect();
    commandes.extend(retraits.iter().map(|a| a.as_slice()));

    // Un seul tmux pour les 41 commandes, separees par `;`. Le cout etait le fork+exec, pas
    // le travail : 41 appels = 167 ms, la meme chaine en un appel = 9,1 ms (mesure du
    // 2026-08-20 sur tmux 3.4). C'est 157 ms rendus a chaque demarrage.
    let echecs = match applique_en_chaine(&commandes) {
        Ok(()) => Vec::new(),
        // La chaine s'ARRETE a la premiere erreur et ne rapporte que celle-la (verifie sur
        // tmux 3.4 : dans `set @a 1 ; set bidon on ; set @b 2`, @b n'est jamais pose). On ne
        // peut donc pas se contenter de journaliser le message de la chaine : les commandes
        // suivantes seraient silencieusement sautees, et un `unbind MouseDown2Pane` saute
        // ramene le double collage sans laisser de trace. On rejoue donc une par une — 167 ms
        // dans ce cas, mais c'est un cas de panne, ou l'on veut justement le detail complet.
        Err(_) => commandes.iter().filter_map(|args| releve(args)).collect(),
    };

    if !echecs.is_empty() {
        journaliser(app, "terminal.optionsTmux", &echecs.join(" | "));
    }
}

/// Enchaine des commandes tmux dans UN seul appel, `;` en argument autonome entre chacune.
/// `Err(message)` des que tmux rend un code non nul — sans dire laquelle a echoue, voir
/// l'appelant.
fn applique_en_chaine(commandes: &[&[&str]]) -> Result<(), String> {
    let args = chaine_args(commandes);
    match tmux_cmd(&args).output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if err.is_empty() { out.status.to_string() } else { err })
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Aplatit des commandes tmux en une seule ligne d'arguments, `;` autonome entre chacune.
fn chaine_args<'a>(commandes: &[&'a [&'a str]]) -> Vec<&'a str> {
    let mut args: Vec<&str> = Vec::new();
    for (i, cmd) in commandes.iter().enumerate() {
        if i > 0 {
            args.push(";");
        }
        args.extend_from_slice(cmd);
    }
    args
}

/// Lance UNE commande tmux et rend son echec au lieu de le perdre.
/// `unbind` d'une touche deja non liee sort en 0 (mesure sur tmux 3.4) : pas de faux positif.
fn releve(args: &[&str]) -> Option<String> {
    let rendu = |detail: String| format!("tmux {} : {}", args.join(" "), detail);
    match tmux_cmd(args).output() {
        Ok(out) if out.status.success() => None,
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Some(rendu(if err.is_empty() { out.status.to_string() } else { err }))
        }
        Err(e) => Some(rendu(e.to_string())),
    }
}

/// Journal local (toujours ecrit, sans consentement — cf. report/mod.rs). Utilise pour les
/// pannes qui n'appellent aucune action de l'utilisateur : elles ne doivent pas etre
/// silencieuses pour autant.
fn journaliser(app: &AppHandle, scope: &str, message: &str) {
    if let Ok(dir) = app.path().app_data_dir() {
        let horodatage = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        crate::report::append_log(&dir, &crate::report::format_log_line(&horodatage, scope, message));
    }
}

/// Boucle du thread lecteur : du PTY vers la file, et rien d'autre. Elle ne construit ni
/// base64 ni evenement — c'est ce qui lui permet de continuer a vider le PTY pendant que
/// l'emetteur travaille, et donc de regrouper.
fn lire_pty(reader: &mut (impl Read + ?Sized), shared: &(Mutex<FileSortie>, Condvar)) {
    let (file, signal) = shared;
    let mut chunk = vec![0u8; PTY_READ_BUF];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                let mut guard = file.lock().unwrap();
                // Detache : on VIDE quand meme le PTY (sinon tmux se bloquerait sur son
                // ecriture) mais on ne garde rien — personne n'attend ces octets.
                if guard.attached {
                    guard.en_attente.extend_from_slice(&chunk[..n]);
                }
                drop(guard);
                signal.notify_one();
            }
        }
    }
}

/// Thread emetteur : pousse vers le webview tout ce que le lecteur du PTY a empile.
fn emettre_sortie(shared: &(Mutex<FileSortie>, Condvar), app: &AppHandle, id: i64) {
    pomper(shared, |octets| {
        let _ = app.emit("terminal_output", OutputPayload { id, data: b64(octets) });
    });
}

/// Coeur de l'emetteur, isole de Tauri pour etre mesurable et testable.
///
/// POURQUOI REGROUPER : Tauri v2 ne livre pas un evenement par un canal binaire, il
/// CONSTRUIT une source JavaScript avec la charge inseree dedans et l'evalue dans le
/// webview (tauri-2.11.0, `event/mod.rs::emit_js_script`). Chaque evenement coute donc un
/// saut vers le WebProcess et une evaluation de script — 8 Ko d'octets font ~11 Ko de
/// source. Mesures du 2026-08-20 : une rafale de 1,9 Mo a travers un vrai PTY partait en
/// 2547 evenements, elle en fait 3 avec ce regroupement ; et cote webview (banc WebKitGTK
/// 2.52 offscreen), evaluer la meme quantite d'octets en 240 scripts coute 11,2 ms contre
/// 2,3 ms en 15.
///
/// POURQUOI LA LATENCE NE BOUGE PAS : au repos, ce thread attend sur la condition ; l'echo
/// d'une touche le reveille et part immediatement. L'attente de `PTY_GROUP_WAIT` ne se
/// declenche qu'au-dela de `PTY_GROUP_THRESHOLD`, volume qu'une frappe n'atteint jamais.
fn pomper(shared: &(Mutex<FileSortie>, Condvar), mut emettre: impl FnMut(&[u8])) {
    let (file, signal) = shared;
    loop {
        let lot = {
            let mut guard = file.lock().unwrap();
            while guard.en_attente.is_empty() {
                if guard.fini {
                    return;
                }
                guard = signal.wait(guard).unwrap();
            }
            if guard.en_attente.len() >= PTY_GROUP_THRESHOLD && !guard.fini {
                // Rafale : on rend le lock quelques millisecondes pour que le lecteur
                // continue d'empiler, et on repart avec un lot plus gros.
                drop(guard);
                std::thread::sleep(PTY_GROUP_WAIT);
                guard = file.lock().unwrap();
            }
            // Un seul tampon, un seul preneur : l'ordre des octets ne peut pas s'inverser.
            std::mem::take(&mut guard.en_attente)
        };
        // detach() vide la file : le lot peut etre vide au reveil, et un evenement vide
        // ferait un aller-retour vers le webview pour rien.
        if lot.is_empty() {
            continue;
        }
        emettre(&lot);
    }
}

impl TerminauxTmux {
    /// Verifie que tmux est disponible (message clair sinon). Avec le tmux embarque,
    /// ce cas ne peut plus arriver dans l'AppImage — il reste possible en build de dev
    /// (--no-bundle, pas de ressource) sur une machine sans tmux.
    fn ensure_tmux() -> Result<(), String> {
        let hint = if cfg!(target_os = "macos") { "brew install tmux" } else { "sudo apt-get install tmux" };
        std::process::Command::new(tmux_program())
            .arg("-V")
            .output()
            .map_err(|_| format!("tmux introuvable — installe-le : {}", hint))?;
        Ok(())
    }

    /// Au demarrage : supprime les lignes dont la session tmux n'existe plus
    /// (reboot machine, kill manuel...).
    fn purge_dead(db: &Database) {
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
        //
        // SAUF si l'on tourne sur une base de donnees choisie a la main (`COCKPIT_DB`, mode
        // developpement ou recette) : ces sessions appartiennent alors a une AUTRE
        // installation, celle de l'utilisateur, et les tuer detruirait ses terminaux. C'est
        // le scenario qui a coute des sessions vieilles de plusieurs jours.
        if std::env::var_os("COCKPIT_DB").is_some() {
            return;
        }
        for session in orphan_sessions(db) {
            tmux_kill_session(&session);
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
        taille: Taille,
        init_command: Option<String>,
    ) -> Result<(), String> {
        let Taille { colonnes: cols, lignes: rows } = taille;
        let program = tmux_program();
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("openpty: {}", e))?;

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        cmd.env("TERM", "xterm-256color");
        // Locale UTF-8 forcee + nettoyage de l'environnement AppImage : ce client peut
        // etre celui qui DEMARRE le serveur tmux, et sans ca l'environnement du montage
        // fuite dans tous les shells (python3 casse, et un utilisateur a vu `mise`
        // echouer pour la meme raison).
        environnement::appliquer_pty(&mut cmd);

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
        let writer = pair
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
        let shared = Arc::new((
            Mutex::new(FileSortie { en_attente: Vec::new(), attached: true, fini: false }),
            Condvar::new(),
        ));
        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));

        {
            let shared = shared.clone();
            let alive = alive.clone();
            let app = app.clone();
            let db = db.clone();
            let tmux_name = tmux_name.to_string();
            std::thread::spawn(move || {
                // Un thread emetteur separe, plutot qu'un emit dans la boucle de lecture :
                // pendant qu'il construit et evalue la source JS de l'evenement, le lecteur
                // continue a empiler. Le regroupement se fait donc TOUT SEUL sous debit,
                // et sur un terminal au repos l'emetteur est deja en attente sur la
                // condition : l'echo d'une touche part sans delai ajoute.
                let emetteur = {
                    let shared = shared.clone();
                    let app = app.clone();
                    std::thread::spawn(move || emettre_sortie(&shared, &app, id))
                };

                lire_pty(&mut reader, &shared);

                // Le PTY est ferme : on laisse l'emetteur ecouler ce qui reste AVANT
                // d'annoncer la fin, sinon `terminal_exit` doublerait les derniers octets
                // de sortie et l'utilisateur perdrait la fin de l'affichage.
                {
                    let (file, signal) = &*shared;
                    file.lock().unwrap().fini = true;
                    signal.notify_one();
                }
                let _ = emetteur.join();

                alive.store(false, std::sync::atomic::Ordering::SeqCst);
                // Il n'y a plus de kill volontaire a taire ici : depuis le revirement du
                // 2026-08-13 (pool persistant), attach() REUTILISE un client vivant au lieu
                // de le tuer pour en respawner un frais. Le seul kill restant est close(),
                // ou terminal_exit est justement ce qu'on veut (les listes se rafraichissent).
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
            LiveAttach { writer, master: pair.master, killer, shared, alive },
        );
        Ok(())
    }
}

impl Terminaux for TerminauxTmux {
    /// L'ORDRE COMPTE : la resolution du binaire tmux vient en premier, purge_dead et
    /// apply_server_options lancent des commandes tmux et en dependent.
    fn preparer(&self, app: &AppHandle, db: &Database) {
        // Resout le binaire tmux (systeme, deja deploye, ou embarque dans l'AppImage).
        setup_bundled_tmux(app);
        // Terminaux dont la session tmux n'existe plus (reboot...) -> purge.
        Self::purge_dead(db);
        // Options presse-papier/style sur le serveur tmux deja en route (la conf n'est
        // relue qu'a la creation du serveur, et il survit a l'app).
        apply_server_options(app);
    }

    fn creer(&self, app: AppHandle, db: &Database, demande: Creation) -> Result<i64, String> {
        Self::ensure_tmux()?;
        let Creation { projet, dossier, taille, commande_initiale } = demande;
        let row = db.create_terminal_row(&projet)?;

        let conf = tmux_conf_path(&app);
        let mut args: Vec<String> = vec!["-u".into(), "-L".into(), TMUX_SOCKET.into()];
        if let Some(conf) = &conf {
            args.push("-f".into());
            args.push(conf.to_string_lossy().to_string());
        }
        args.extend(["new-session", "-A", "-s", &row.tmux_name].map(String::from));
        if std::path::Path::new(&dossier).is_dir() {
            args.push("-c".into());
            args.push(dossier);
        }

        match self.spawn_attach(&app, db, row.id, &row.tmux_name, &args, taille, commande_initiale) {
            Ok(()) => Ok(row.id),
            Err(e) => {
                let _ = db.delete_terminal_row(row.id);
                Err(e)
            }
        }
    }

    /// Attache l'UI. Si aucun client tmux n'est vivant pour cette session
    /// (app relancee), on en respawn un — tmux repeint l'ecran tout seul.
    fn attacher(
        &self,
        app: AppHandle,
        db: &Database,
        id: i64,
        taille: Taille,
    ) -> Result<(), String> {
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
                return Ok(());
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
            &taille.colonnes.to_string(),
            "-y",
            &taille.lignes.to_string(),
        ])
        .output();

        // -u : force UTF-8. -d : detache les autres clients (tailles concurrentes)
        let args: Vec<String> =
            ["-u", "-L", TMUX_SOCKET, "attach-session", "-d", "-t", &row.tmux_name]
                .map(String::from)
                .to_vec();
        self.spawn_attach(&app, db, id, &row.tmux_name, &args, taille, None)
    }

    fn detacher(&self, id: i64) {
        let live = self.live.lock().unwrap();
        if let Some(l) = live.get(&id) {
            let (file, _) = &*l.shared;
            let mut guard = file.lock().unwrap();
            guard.attached = false;
            // Ce qui n'est pas encore parti ne servira plus a personne : le xterm de ce
            // terminal ne l'attend plus. On evite de garder un tampon en memoire.
            guard.en_attente.clear();
        }
    }

    fn ecrire(&self, id: i64, donnees: &str) -> Result<(), String> {
        let mut live = self.live.lock().unwrap();
        let l = live.get_mut(&id).ok_or("terminal non attache")?;
        l.writer.write_all(donnees.as_bytes()).map_err(|e| e.to_string())
    }

    fn redimensionner(&self, db: &Database, id: i64, taille: Taille) -> Result<(), String> {
        let Taille { colonnes: cols, lignes: rows } = taille;
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

    fn renommer(&self, db: &Database, id: i64, nom: &str) -> Result<(), String> {
        let clean: String = nom.trim().chars().take(40).collect();
        db.rename_terminal_row(id, &clean)
    }

    fn fermer(&self, db: &Database, id: i64) -> Result<(), String> {
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
    fn copier_selection(&self, db: &Database, id: i64) -> Result<(), String> {
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
    fn chercher(
        &self,
        db: &Database,
        id: i64,
        action: ActionRecherche,
        motif: &str,
    ) -> Result<(), String> {
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
            ActionRecherche::Demarrer => {
                if motif.trim().is_empty() {
                    return Err("recherche vide".into());
                }
                // no-op si le pane est deja en copy-mode
                run(&["copy-mode", "-t", &target])?;
                // Variante -text : sous-chaine LITTERALE, pas une regex — c'est une
                // recherche d'utilisateur, "1.2.3" ne doit pas matcher "1x2y3".
                run(&["send-keys", "-t", &target, "-X", "search-backward-text", motif])
            }
            ActionRecherche::Suivante => run(&["send-keys", "-t", &target, "-X", "search-again"]),
            ActionRecherche::Precedente => run(&["send-keys", "-t", &target, "-X", "search-reverse"]),
            ActionRecherche::Annuler => {
                // Best effort : hors copy-mode, sortir n'a rien a faire (et ne doit pas toaster)
                let _ = tmux_cmd(&["send-keys", "-t", &target, "-X", "cancel"]).output();
                Ok(())
            }
        }
    }

    /// Le programme QUI TOURNE DANS la session tmux est-il en ecran alternatif
    /// (vim, claude, htop...) ? Ne pas confondre avec le buffer xterm : le
    /// client tmux met TOUJOURS le terminal hote en ecran alternatif.
    fn ecran_alternatif(&self, db: &Database, id: i64) -> bool {
        let Ok(row) = db.get_terminal_row(id) else { return false };
        tmux_cmd(&["display-message", "-p", "-t", &row.tmux_name, "#{alternate_on}"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
            .unwrap_or(false)
    }

    fn lister(&self, db: &Database, projet: Option<&str>) -> Vec<TerminalInfo> {
        // tmux injoignable -> on annonce les terminaux comme VIVANTS plutot que morts. Le
        // store `terminals` du frontend filtre sur ce flag et les ferait disparaitre de la
        // sidebar a chaque hoquet de tmux.
        let alive = tmux_alive_sessions();
        let llm = tmux_llm_sessions();
        db.get_terminal_rows(projet)
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
    /// Messages RELEVES sur tmux 3.4 le 2026-08-20, pas inventes : les deux disent
    /// « aucun serveur », et seule la premiere forme etait reconnue.
    #[test]
    fn serveur_mort_avec_socket_encore_present_est_une_absence_certaine() {
        assert!(super::absence_definitive(
            "no server running on /tmp/tmux-1000/cockpit\n"
        ));
    }

    /// Le cas d'apres un redemarrage de la machine : /tmp est vide, donc plus de socket.
    /// C'est celui qui laissait des terminaux affiches et impossibles a fermer.
    #[test]
    fn socket_absent_est_une_absence_certaine() {
        assert!(super::absence_definitive(
            "error connecting to /tmp/tmux-1000/cockpit (No such file or directory)\n"
        ));
    }

    /// Tout le reste reste un « on ne sait pas » : on ne supprime RIEN sur un doute.
    #[test]
    fn les_autres_echecs_ne_valent_pas_absence() {
        for stderr in [
            "error connecting to /tmp/tmux-1000/cockpit (Connection refused)",
            "server exited unexpectedly",
            "lost server",
            "",
        ] {
            assert!(!super::absence_definitive(stderr), "{stderr:?} ne prouve pas l'absence");
        }
    }

    /// L'enchainement des options tmux repose sur un `;` ARGUMENT AUTONOME. Un argument qui
    /// vaudrait exactement ";" serait pris pour un separateur et couperait la commande en
    /// deux — silencieusement. Le test verrouille la liste.
    #[test]
    fn aucun_argument_point_virgule_dans_les_options_serveur() {
        for cmd in super::OPTIONS_SERVEUR {
            assert!(
                !cmd.contains(&";"),
                "la commande {:?} contient un ';' : incompatible avec l'enchainement",
                cmd
            );
        }
    }

    #[test]
    fn chaine_args_separe_les_commandes_par_un_point_virgule() {
        let a: &[&str] = &["set", "-s", "x", "on"];
        let b: &[&str] = &["unbind", "-n", "MouseDown2Pane"];
        assert_eq!(
            super::chaine_args(&[a, b]),
            vec!["set", "-s", "x", "on", ";", "unbind", "-n", "MouseDown2Pane"]
        );
        // Une seule commande : aucun separateur parasite.
        assert_eq!(super::chaine_args(&[a]), vec!["set", "-s", "x", "on"]);
        assert!(super::chaine_args(&[]).is_empty());
    }

    fn file_vide() -> std::sync::Arc<(std::sync::Mutex<super::FileSortie>, std::sync::Condvar)> {
        std::sync::Arc::new((
            std::sync::Mutex::new(super::FileSortie {
                en_attente: Vec::new(),
                attached: true,
                fini: false,
            }),
            std::sync::Condvar::new(),
        ))
    }

    fn empiler(
        partage: &std::sync::Arc<(std::sync::Mutex<super::FileSortie>, std::sync::Condvar)>,
        octets: &[u8],
    ) {
        let (file, signal) = &**partage;
        file.lock().unwrap().en_attente.extend_from_slice(octets);
        signal.notify_one();
    }

    fn terminer(partage: &std::sync::Arc<(std::sync::Mutex<super::FileSortie>, std::sync::Condvar)>) {
        let (file, signal) = &**partage;
        file.lock().unwrap().fini = true;
        signal.notify_one();
    }

    /// L'invariant qui compte : pas un octet perdu, pas un octet interverti. Un terminal
    /// qui recoit ses octets dans le desordre affiche n'importe quoi.
    #[test]
    fn le_regroupement_conserve_tous_les_octets_dans_l_ordre() {
        let partage = file_vide();
        let attendu: Vec<u8> = (0..200u32).flat_map(|i| vec![(i % 251) as u8; 8192]).collect();

        let producteur = {
            let partage = partage.clone();
            let attendu = attendu.clone();
            std::thread::spawn(move || {
                for morceau in attendu.chunks(8192) {
                    empiler(&partage, morceau);
                }
                terminer(&partage);
            })
        };

        let mut recu: Vec<u8> = Vec::new();
        let mut lots = 0usize;
        super::pomper(&partage, |octets| {
            lots += 1;
            recu.extend_from_slice(octets);
        });
        producteur.join().unwrap();

        assert_eq!(recu, attendu, "octets perdus ou desordonnes");
        assert!(lots >= 1 && lots <= 200, "lots={}", lots);
    }

    /// Ce qui est DEJA empile part en un seul evenement : c'est tout l'objet du
    /// regroupement (240 evenements Tauri pour une rafale de 1,96 Mo avant ce correctif).
    #[test]
    fn ce_qui_est_deja_empile_part_en_un_seul_lot() {
        let partage = file_vide();
        for _ in 0..20 {
            empiler(&partage, &[7u8; 8192]);
        }
        terminer(&partage);

        let mut lots: Vec<usize> = Vec::new();
        super::pomper(&partage, |octets| lots.push(octets.len()));
        assert_eq!(lots, vec![20 * 8192]);
    }

    /// Le chemin de frappe : ce qui est petit ne doit PAS etre retenu en attendant la
    /// suite. Le producteur ne pousse le second octet qu'une fois le premier recu : si
    /// l'emetteur attendait d'avoir « assez » de matiere, le test se bloquerait.
    #[test]
    fn un_echo_de_touche_part_seul_sans_attendre_la_suite() {
        let partage = file_vide();
        let (recu_tx, recu_rx) = std::sync::mpsc::channel::<()>();

        let producteur = {
            let partage = partage.clone();
            std::thread::spawn(move || {
                empiler(&partage, b"a");
                recu_rx.recv().unwrap();
                empiler(&partage, b"b");
                recu_rx.recv().unwrap();
                terminer(&partage);
            })
        };

        let mut lots: Vec<Vec<u8>> = Vec::new();
        super::pomper(&partage, |octets| {
            lots.push(octets.to_vec());
            let _ = recu_tx.send(());
        });
        producteur.join().unwrap();

        assert_eq!(lots, vec![b"a".to_vec(), b"b".to_vec()]);
    }

    /// Le chemin complet, avec un VRAI PTY : `lire_pty` puis `pomper`. C'est ce test qui
    /// garantit qu'un regroupement mal ecrit ne perd ni n'intervertit rien — la sortie d'un
    /// terminal desordonnee est indetectable a la relecture du code.
    #[test]
    fn le_chemin_pty_complet_rend_les_octets_intacts() {
        use portable_pty::{native_pty_system, CommandBuilder, PtySize};

        let attendu = "cockpit".repeat(6000); // ~42 Ko, plusieurs lectures et un regroupement
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .unwrap();
        let mut cmd = CommandBuilder::new("printf");
        cmd.arg("%s");
        cmd.arg(&attendu);
        let mut child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();

        let partage = file_vide();
        let lecteur = {
            let partage = partage.clone();
            std::thread::spawn(move || {
                super::lire_pty(&mut reader, &partage);
                terminer(&partage);
            })
        };

        let mut recu: Vec<u8> = Vec::new();
        super::pomper(&partage, |octets| recu.extend_from_slice(octets));
        lecteur.join().unwrap();
        let _ = child.wait();

        // Le PTY est en mode « cooked » : il traduit chaque \n en \r\n. `printf %s` n'en
        // emet aucun, la comparaison est donc directe.
        assert_eq!(String::from_utf8_lossy(&recu), attendu);
    }
}
