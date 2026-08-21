//! Connexion Claude Code (abonnement) : statut lu depuis ~/.claude/.credentials.json,
//! et flow de connexion guide en pilotant `claude setup-token` dans un PTY
//! (la CLI affiche une URL OAuth, l'utilisateur colle le code, la CLI stocke tout).

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
pub struct ClaudeAuthStatus {
    pub cli_installed: bool,
    pub cli_version: Option<String>,
    pub logged_in: bool,
    pub subscription_type: Option<String>,
    pub rate_limit_tier: Option<String>,
    /// Epoch secondes d'expiration du token courant.
    pub expires_at: Option<i64>,
    /// Pourquoi le statut n'a pas pu etre determine, quand c'est le cas.
    ///
    /// « Non connecte » et « on n'a pas su regarder » sont deux choses differentes, et
    /// jusqu'ici elles s'affichaient pareil : dossier personnel introuvable ou fichier de
    /// jetons illisible rendaient `logged_in: false`, sans un mot. L'utilisateur relancait
    /// alors une connexion qui ne changeait rien.
    pub problem: Option<String>,
}

#[derive(Default)]
pub struct ClaudeLoginState {
    session: Mutex<Option<LoginSession>>,
}

struct LoginSession {
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    alive: Arc<std::sync::atomic::AtomicBool>,
    _master: Box<dyn MasterPty + Send>, // garde le PTY vivant pendant le flow
}

pub fn status() -> ClaudeAuthStatus {
    let version = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let mut auth = ClaudeAuthStatus {
        cli_installed: version.is_some(),
        cli_version: version,
        logged_in: false,
        subscription_type: None,
        rate_limit_tier: None,
        expires_at: None,
        problem: None,
    };

    let chemin = match crate::chemins::dossier_personnel() {
        Ok(home) => home.join(".claude").join(".credentials.json"),
        Err(e) => {
            auth.problem = Some(e);
            return auth;
        }
    };
    // Fichier absent = pas encore connecte : c'est le cas normal, pas un probleme a signaler.
    let raw = match std::fs::read_to_string(&chemin) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return auth,
        Err(e) => {
            auth.problem = Some(format!("{} illisible : {e}", chemin.display()));
            return auth;
        }
    };
    let json = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(json) => json,
        Err(e) => {
            auth.problem = Some(format!("{} n'est pas du JSON valide : {e}", chemin.display()));
            return auth;
        }
    };
    let Some(oauth) = json.get("claudeAiOauth") else {
        auth.problem = Some(format!("{} ne contient pas de bloc claudeAiOauth", chemin.display()));
        return auth;
    };

    auth.logged_in = oauth
        .get("accessToken")
        .and_then(|t| t.as_str())
        .map(|t| !t.is_empty())
        .unwrap_or(false);
    auth.subscription_type = oauth
        .get("subscriptionType")
        .and_then(|v| v.as_str())
        .map(String::from);
    auth.rate_limit_tier = oauth
        .get("rateLimitTier")
        .and_then(|v| v.as_str())
        .map(String::from);
    auth.expires_at = oauth.get("expiresAt").and_then(|v| v.as_i64()).map(|ts| {
        // millisecondes si valeur trop grande pour des secondes
        if ts > 100_000_000_000 { ts / 1000 } else { ts }
    });

    auth
}

impl ClaudeLoginState {
    /// Lance `claude setup-token` dans un PTY ; la sortie part vers le frontend
    /// via l'event `claude_login_output`, la fin via `claude_login_done`.
    pub fn start(&self, app: AppHandle) -> Result<(), String> {
        self.cancel();

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("openpty: {}", e))?;

        let mut cmd = CommandBuilder::new("claude");
        cmd.arg("setup-token");
        cmd.env("TERM", "xterm-256color");
        // Un dossier personnel introuvable n'empeche pas le flow : la CLI partira du
        // dossier courant. On ne bloque donc pas dessus, mais on le journalise.
        match crate::chemins::dossier_personnel() {
            Ok(home) => cmd.cwd(home),
            Err(e) => log::warn!("claude setup-token lance sans dossier de depart : {e}"),
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("lancement claude setup-token: {}", e))?;
        drop(pair.slave);

        let killer = child.clone_killer();
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
        {
            let alive = alive.clone();
            let app = app.clone();
            std::thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match reader.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&chunk[..n]).to_string();
                            let _ = app.emit("claude_login_output", text);
                        }
                    }
                }
                alive.store(false, std::sync::atomic::Ordering::SeqCst);
                let _ = app.emit("claude_login_done", ());
            });
        }

        *self.session.lock().unwrap() =
            Some(LoginSession { writer, killer, alive, _master: pair.master });
        Ok(())
    }

    pub fn input(&self, data: &str) -> Result<(), String> {
        let mut guard = self.session.lock().unwrap();
        let session = guard.as_mut().ok_or("aucune connexion en cours")?;
        session
            .writer
            .write_all(format!("{}\r", data.trim()).as_bytes())
            .map_err(|e| e.to_string())
    }

    pub fn cancel(&self) {
        if let Some(mut s) = self.session.lock().unwrap().take() {
            if s.alive.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = s.killer.kill();
            }
        }
    }
}
