//! Capture audio via PipeWire (pw-record) : micro + monitor de la sortie systeme.
//! Chaque piste sort en PCM brut s16le mono 16 kHz (stdout de pw-record -> fichier .raw).

use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};

pub struct CaptureHandles {
    mic: Child,
    system: Child,
    mic_path: PathBuf,
    system_path: PathBuf,
}

/// Nom du fichier ou est conservee la sortie d'erreur d'une piste.
fn err_path(out_path: &Path) -> PathBuf {
    out_path.with_extension("err")
}

/// Lit la sortie d'erreur d'une piste.
fn read_err(out_path: &Path) -> String {
    std::fs::read_to_string(err_path(out_path))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Borne un texte destine a un message d'interface.
///
/// La coupe est faite ICI, a la construction du message, et non a la lecture : c'est le
/// seul endroit par ou tout passe, donc le seul ou la garantie tient quelle que soit
/// l'origine du texte.
fn borne(text: &str) -> String {
    const MAX: usize = 300;
    if text.chars().count() > MAX {
        let cut: String = text.chars().take(MAX).collect();
        format!("{cut}…")
    } else {
        text.to_string()
    }
}

/// Assemble le message d'echec au demarrage a partir de ce que pw-record a dit.
///
/// Sans cela le diagnostic etait invente : le message annoncait « PipeWire indisponible ? »
/// alors que la sortie d'erreur etait jetee (`Stdio::null()`). Un utilisateur avec PipeWire
/// et pw-record installes n'avait donc aucun moyen de savoir ce qui bloquait.
pub fn startup_error(mic: &str, system: &str) -> String {
    let mut details: Vec<String> = Vec::new();
    if !mic.is_empty() {
        details.push(format!("micro : {}", borne(mic)));
    }
    if !system.is_empty() && system != mic {
        details.push(format!("son systeme : {}", borne(system)));
    }
    if details.is_empty() {
        // Aucune sortie : pw-record est mort sans rien dire.
        "pw-record s'est arrete aussitot, sans message. Verifie que PipeWire tourne \
         (`pactl info` ou `pw-cli info 0`) et qu'une entree audio est disponible."
            .to_string()
    } else {
        format!("pw-record a echoue au demarrage — {}", details.join(" ; "))
    }
}

fn spawn_pw_record(out_path: &Path, capture_sink: bool) -> Result<Child, String> {
    let file = std::fs::File::create(out_path)
        .map_err(|e| format!("creation {}: {}", out_path.display(), e))?;
    // La sortie d'erreur va dans un fichier a cote de la piste : en cas d'echec elle est
    // remontee a l'utilisateur, et le dossier etant conserve, elle reste consultable.
    let err_file = std::fs::File::create(err_path(out_path))
        .map_err(|e| format!("creation {}: {}", err_path(out_path).display(), e))?;

    let mut cmd = Command::new("pw-record");
    cmd.args(["--rate", "16000", "--channels", "1", "--format", "s16"]);
    if capture_sink {
        // Capture le monitor du sink par defaut = tout ce qui sort des enceintes/casque
        cmd.args(["-P", "stream.capture.sink=true"]);
    }
    cmd.arg("-") // "-" = PCM brut sur stdout
        .stdout(std::process::Stdio::from(file))
        .stderr(std::process::Stdio::from(err_file))
        .kill_on_drop(true);

    cmd.spawn().map_err(|e| format!("lancement pw-record: {}", e))
}

/// Demarre les deux captures dans `dir` (mic.raw + system.raw).
pub fn start_capture(dir: &Path) -> Result<CaptureHandles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creation dossier: {}", e))?;
    let mic_path = dir.join("mic.raw");
    let system_path = dir.join("system.raw");
    let mut mic = spawn_pw_record(&mic_path, false)?;
    let system = match spawn_pw_record(&system_path, true) {
        Ok(c) => c,
        Err(e) => {
            // Ne pas laisser la capture micro orpheline si la seconde echoue
            let _ = mic.start_kill();
            return Err(e);
        }
    };
    Ok(CaptureHandles { mic, system, mic_path, system_path })
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

    /// Ce que pw-record a ecrit sur sa sortie d'erreur, piste par piste.
    pub fn startup_error(&self) -> String {
        startup_error(&read_err(&self.mic_path), &read_err(&self.system_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_reprend_la_sortie_du_micro() {
        let msg = startup_error("connection refused", "");
        assert!(msg.contains("micro : connection refused"), "{msg}");
        assert!(!msg.contains("son systeme"), "{msg}");
    }

    #[test]
    fn message_distingue_les_deux_pistes() {
        let msg = startup_error("pas de source", "pas de sink");
        assert!(msg.contains("micro : pas de source"), "{msg}");
        assert!(msg.contains("son systeme : pas de sink"), "{msg}");
    }

    #[test]
    fn sortie_identique_annoncee_une_seule_fois() {
        let msg = startup_error("meme erreur", "meme erreur");
        assert_eq!(msg.matches("meme erreur").count(), 1, "{msg}");
    }

    #[test]
    fn sans_sortie_le_message_dit_quoi_verifier() {
        // Le piege d'origine : inventer « PipeWire indisponible ? » sans rien savoir.
        let msg = startup_error("", "");
        assert!(msg.contains("sans message"), "{msg}");
        assert!(msg.contains("pw-cli"), "{msg}");
    }

    #[test]
    fn sortie_longue_bornee() {
        let long = "x".repeat(1000);
        let msg = startup_error(&long, "");
        assert!(msg.chars().count() < 400, "message trop long: {}", msg.chars().count());
    }

    #[test]
    fn le_fichier_d_erreur_est_a_cote_de_la_piste() {
        assert_eq!(err_path(Path::new("/tmp/rec/mic.raw")), Path::new("/tmp/rec/mic.err"));
    }
}
