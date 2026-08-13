//! Capture audio via PipeWire (pw-record) : micro + monitor de la sortie systeme.
//! Chaque piste sort en PCM brut s16le mono 16 kHz (stdout de pw-record -> fichier .raw).

use std::path::Path;
use tokio::process::{Child, Command};

pub struct CaptureHandles {
    mic: Child,
    system: Child,
}

fn spawn_pw_record(out_path: &Path, capture_sink: bool) -> Result<Child, String> {
    let file = std::fs::File::create(out_path)
        .map_err(|e| format!("creation {}: {}", out_path.display(), e))?;

    let mut cmd = Command::new("pw-record");
    cmd.args(["--rate", "16000", "--channels", "1", "--format", "s16"]);
    if capture_sink {
        // Capture le monitor du sink par defaut = tout ce qui sort des enceintes/casque
        cmd.args(["-P", "stream.capture.sink=true"]);
    }
    cmd.arg("-") // "-" = PCM brut sur stdout
        .stdout(std::process::Stdio::from(file))
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);

    cmd.spawn().map_err(|e| format!("lancement pw-record: {}", e))
}

/// Demarre les deux captures dans `dir` (mic.raw + system.raw).
pub fn start_capture(dir: &Path) -> Result<CaptureHandles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creation dossier: {}", e))?;
    let mut mic = spawn_pw_record(&dir.join("mic.raw"), false)?;
    let system = match spawn_pw_record(&dir.join("system.raw"), true) {
        Ok(c) => c,
        Err(e) => {
            // Ne pas laisser la capture micro orpheline si la seconde echoue
            let _ = mic.start_kill();
            return Err(e);
        }
    };
    Ok(CaptureHandles { mic, system })
}

impl CaptureHandles {
    /// Arrete proprement les deux pw-record (SIGTERM puis attente).
    pub async fn stop(mut self) -> Result<(), String> {
        for child in [&mut self.mic, &mut self.system] {
            if let Some(pid) = child.id() {
                let _ = Command::new("kill").arg(pid.to_string()).output().await;
            }
        }
        // Laisse le temps de flusher, puis force si besoin
        for child in [&mut self.mic, &mut self.system] {
            match tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await {
                Ok(_) => {}
                Err(_) => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                }
            }
        }
        Ok(())
    }

    /// Verifie que les process tournent toujours (echec pw-record au demarrage).
    pub fn is_alive(&mut self) -> bool {
        matches!(self.mic.try_wait(), Ok(None)) && matches!(self.system.try_wait(), Ok(None))
    }
}
