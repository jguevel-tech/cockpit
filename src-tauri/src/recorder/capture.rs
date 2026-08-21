//! Capture audio : micro + son qui sort des enceintes, chacun dans sa piste.
//! Chaque piste sort en PCM brut s16le mono 16 kHz (stdout de l'enregistreur -> .raw).
//!
//! DEUX ENREGISTREURS, ESSAYES DANS CET ORDRE (ne pas simplifier en un seul) :
//!   1. `pw-record` (PipeWire), l'outil des systemes recents ;
//!   2. `parecord` (PulseAudio), en repli.
//! Sur Ubuntu 22.04, `pipewire` est installe mais ce n'est PAS lui qui gere l'audio :
//! PulseAudio l'a garde. `pw-record` s'y connecte donc a un serveur PipeWire sans aucun
//! peripherique et meurt sur « no node available », alors que le micro fonctionne tres
//! bien — cas remonte par un utilisateur, reproduit dans scripts/recette/.
//!
//! Le repli se decide AU CONSTAT D'ECHEC, piste par piste, et non sur un diagnostic
//! devine : la piste qui meurt est relancee avec l'autre outil. C'est ce qui rend le
//! comportement juste quelle que soit la cause (option refusee, serveur absent, aucun
//! peripherique expose).

use crate::commande::SansConsole;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::{Child, Command};

/// Delai laisse a un enregistreur pour s'attacher aux peripheriques avant de le juger.
const DELAI_ATTACHE: Duration = Duration::from_millis(300);

pub struct CaptureHandles {
    mic: Option<Child>,
    system: Option<Child>,
    mic_path: PathBuf,
    system_path: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outil {
    PipeWire,
    PulseAudio,
}

impl Outil {
    fn programme(self) -> &'static str {
        match self {
            Outil::PipeWire => "pw-record",
            Outil::PulseAudio => "parecord",
        }
    }

    /// Suffixe du fichier de sortie d'erreur : une par outil, pour que le message final
    /// puisse dire ce que CHAQUE tentative a repondu.
    fn suffixe_err(self) -> &'static str {
        match self {
            Outil::PipeWire => "pw.err",
            Outil::PulseAudio => "pa.err",
        }
    }
}

fn err_path(out_path: &Path, outil: Outil) -> PathBuf {
    out_path.with_extension(outil.suffixe_err())
}

/// Lit la sortie d'erreur d'une tentative.
fn read_err(out_path: &Path, outil: Outil) -> String {
    std::fs::read_to_string(err_path(out_path, outil))
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

/// Assemble le message d'echec a partir de ce que les enregistreurs ont dit.
///
/// Sans cela le diagnostic etait invente : le message annoncait « PipeWire indisponible ? »
/// alors que la sortie d'erreur etait jetee. Un utilisateur avec PipeWire et pw-record
/// installes n'avait aucun moyen de savoir ce qui bloquait.
pub fn startup_error(mic: &str, system: &str) -> String {
    let mut details: Vec<String> = Vec::new();
    if !mic.is_empty() {
        details.push(format!("micro : {}", borne(mic)));
    }
    if !system.is_empty() && system != mic {
        details.push(format!("son systeme : {}", borne(system)));
    }

    if details.is_empty() {
        "L'enregistreur s'est arrete aussitot, sans message. Verifie qu'un serveur audio \
         tourne (`pactl info`) et qu'une entree est disponible."
            .to_string()
    } else {
        format!(
            "Aucun enregistreur audio n'a pu demarrer (ni PipeWire ni PulseAudio) — {}",
            details.join(" ; ")
        )
    }
}

/// `pw-record` accepte-t-il `-P` (proprietes de noeud) ?
///
/// L'option n'existe pas avant PipeWire 0.3.5x : sur Ubuntu 22.04 (libpipewire 0.3.48)
/// elle fait echouer la commande avec « option invalide -- 'P' », et l'enregistrement du
/// son systeme mourait aussitot. On lit donc l'aide de la commande une seule fois, et on
/// choisit la forme qu'elle comprend. Cockpit ne demande pas a l'utilisateur de mettre son
/// systeme a jour pour une option d'outil.
fn supports_properties_flag() -> bool {
    static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        std::process::Command::new("pw-record")
            .sans_console()
            .arg("--help")
            .output()
            .map(|out| {
                // L'aide part sur stdout ou stderr selon la version.
                let mut help = String::from_utf8_lossy(&out.stdout).to_string();
                help.push_str(&String::from_utf8_lossy(&out.stderr));
                help_mentions_properties(&help)
            })
            .unwrap_or(false)
    })
}

/// Reconnait l'option de proprietes dans l'aide de pw-record.
fn help_mentions_properties(help: &str) -> bool {
    help.contains("--properties") || help.contains("-P ")
}

/// Arguments de capture d'une piste.
///
/// Fonction pure, et testee : c'est exactement le genre de detail dont l'erreur ne se voit
/// pas ici, mais sur la machine de quelqu'un d'autre. Les noms `@DEFAULT_SOURCE@` et
/// `@DEFAULT_MONITOR@` sont les formes generiques de PulseAudio — verifiees en recette,
/// elles evitent d'avoir a resoudre le nom du peripherique par un appel a `pactl`.
fn args_capture(outil: Outil, capture_sink: bool, properties_flag: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    match outil {
        Outil::PipeWire => {
            args.extend(
                ["--rate", "16000", "--channels", "1", "--format", "s16"]
                    .iter()
                    .map(|s| s.to_string()),
            );
            if capture_sink && properties_flag {
                args.push("-P".to_string());
                args.push("stream.capture.sink=true".to_string());
            }
            args.push("-".to_string()); // "-" = PCM brut sur stdout
        }
        Outil::PulseAudio => {
            let device = if capture_sink {
                "--device=@DEFAULT_MONITOR@"
            } else {
                "--device=@DEFAULT_SOURCE@"
            };
            args.extend(
                [
                    "--rate=16000",
                    "--channels=1",
                    "--format=s16le",
                    "--raw",
                    device,
                ]
                .iter()
                .map(|s| s.to_string()),
            );
        }
    }
    args
}

/// Lance une piste avec l'outil demande. `append` conserve ce qui a deja ete capture :
/// une piste relancee en repli ne doit pas effacer le debut de l'enregistrement.
fn spawn_track(out_path: &Path, capture_sink: bool, outil: Outil) -> Result<Child, String> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_path)
        .map_err(|e| format!("creation {}: {}", out_path.display(), e))?;
    // La sortie d'erreur va dans un fichier a cote de la piste : en cas d'echec elle est
    // remontee a l'utilisateur, et le dossier etant conserve, elle reste consultable.
    let err_file = std::fs::File::create(err_path(out_path, outil))
        .map_err(|e| format!("creation {}: {}", err_path(out_path, outil).display(), e))?;

    let properties_flag = outil == Outil::PipeWire && supports_properties_flag();
    let mut cmd = Command::new(outil.programme());
    cmd.sans_console();
    cmd.args(args_capture(outil, capture_sink, properties_flag));
    if outil == Outil::PipeWire && capture_sink && !properties_flag {
        // Forme comprise par les pw-record qui ignorent `-P`.
        cmd.env("PIPEWIRE_PROPS", "{ stream.capture.sink=true }");
    }
    cmd.stdout(std::process::Stdio::from(file))
        .stderr(std::process::Stdio::from(err_file))
        .kill_on_drop(true);

    cmd.spawn()
        .map_err(|e| format!("lancement {}: {}", outil.programme(), e))
}

fn vivant(child: &mut Option<Child>) -> bool {
    match child {
        Some(c) => matches!(c.try_wait(), Ok(None)),
        None => false,
    }
}

/// Demarre les deux captures dans `dir` (mic.raw + system.raw).
///
/// Ne renvoie une erreur que si AUCUNE piste ne tient : l'appelant decide quoi faire
/// d'une seule piste vivante.
pub async fn start_capture(dir: &Path) -> Result<CaptureHandles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creation dossier: {}", e))?;
    let mic_path = dir.join("mic.raw");
    let system_path = dir.join("system.raw");

    let mut mic = spawn_track(&mic_path, false, Outil::PipeWire).ok();
    let mut system = spawn_track(&system_path, true, Outil::PipeWire).ok();
    tokio::time::sleep(DELAI_ATTACHE).await;

    // Repli piste par piste : sur un systeme ou PulseAudio garde l'audio, PipeWire
    // repond mais n'expose aucun peripherique.
    let mut replie = false;
    if !vivant(&mut mic) {
        mic = spawn_track(&mic_path, false, Outil::PulseAudio).ok();
        replie = true;
    }
    if !vivant(&mut system) {
        system = spawn_track(&system_path, true, Outil::PulseAudio).ok();
        replie = true;
    }
    if replie {
        tokio::time::sleep(DELAI_ATTACHE).await;
    }

    let handles = CaptureHandles {
        mic,
        system,
        mic_path,
        system_path,
    };
    Ok(handles)
}

impl CaptureHandles {
    /// Arrete proprement les enregistreurs (SIGTERM puis attente).
    pub async fn stop(mut self) -> Result<(), String> {
        for child in [&mut self.mic, &mut self.system].into_iter().flatten() {
            if let Some(pid) = child.id() {
                let _ = Command::new("kill").sans_console().arg(pid.to_string()).output().await;
            }
        }
        // Laisse le temps de flusher, puis force si besoin
        for child in [&mut self.mic, &mut self.system].into_iter().flatten() {
            match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
                Ok(_) => {}
                Err(_) => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                }
            }
        }
        Ok(())
    }

    /// Etat de chaque piste : (micro, son systeme).
    ///
    /// Renvoie les deux separement, et non un booleen d'ensemble : une machine peut tres
    /// bien capter une piste et pas l'autre, et refuser l'enregistrement entier pour cette
    /// raison privait l'utilisateur de celle qui marchait.
    pub fn alive_tracks(&mut self) -> (bool, bool) {
        (vivant(&mut self.mic), vivant(&mut self.system))
    }

    /// Ce que les deux enregistreurs ont ecrit, piste par piste.
    pub fn startup_error(&self) -> String {
        let assemble = |path: &Path| {
            [Outil::PipeWire, Outil::PulseAudio]
                .iter()
                .filter_map(|o| {
                    let texte = read_err(path, *o);
                    (!texte.is_empty()).then(|| format!("{} : {}", o.programme(), texte))
                })
                .collect::<Vec<_>>()
                .join(" / ")
        };
        startup_error(&assemble(&self.mic_path), &assemble(&self.system_path))
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
        assert!(msg.contains("pactl"), "{msg}");
    }

    #[test]
    fn le_message_nomme_les_deux_enregistreurs_essayes() {
        let msg = startup_error("rien", "");
        assert!(msg.contains("PipeWire") && msg.contains("PulseAudio"), "{msg}");
    }

    #[test]
    fn sortie_longue_bornee() {
        let long = "x".repeat(1000);
        let msg = startup_error(&long, "");
        assert!(
            msg.chars().count() < 500,
            "message trop long: {}",
            msg.chars().count()
        );
    }

    #[test]
    fn aide_recente_annonce_l_option_de_proprietes() {
        // pw-record 1.0.5
        let aide = "  -v, --verbose                         Enable verbose operations\n  \
                    -P  --properties                      Set node properties\n";
        assert!(help_mentions_properties(aide));
    }

    #[test]
    fn aide_ancienne_ne_l_annonce_pas() {
        // pw-record 0.3.48 (Ubuntu 22.04) : aide remontee par l'utilisateur dont
        // l'enregistrement du son systeme echouait sur « option invalide -- 'P' ».
        let aide = "pw-record [options] <file>\n  -h, --help                            Show this help\n  \
                    --version                         Show version\n  \
                    -v, --verbose                     Enable verbose operations\n  \
                    -R, --remote                      Remote daemon name\n";
        assert!(!help_mentions_properties(aide));
    }

    #[test]
    fn pipewire_capte_le_son_systeme_par_l_option_quand_elle_existe() {
        let args = args_capture(Outil::PipeWire, true, true);
        assert!(args.contains(&"-P".to_string()), "{args:?}");
        assert!(args.contains(&"stream.capture.sink=true".to_string()), "{args:?}");
    }

    #[test]
    fn pipewire_sans_l_option_ne_la_passe_pas_en_argument() {
        // Sinon pw-record 0.3.48 refuse de demarrer : « option invalide -- 'P' ».
        let args = args_capture(Outil::PipeWire, true, false);
        assert!(!args.contains(&"-P".to_string()), "{args:?}");
        assert!(args.contains(&"-".to_string()), "{args:?}");
    }

    #[test]
    fn pulseaudio_prend_la_source_par_defaut_pour_le_micro() {
        // La forme generique evite de resoudre le nom du peripherique : verifie en recette,
        // parecord sans `--device` ne capte rien d'utile.
        let args = args_capture(Outil::PulseAudio, false, false);
        assert!(args.contains(&"--device=@DEFAULT_SOURCE@".to_string()), "{args:?}");
        assert!(args.contains(&"--raw".to_string()), "{args:?}");
    }

    #[test]
    fn pulseaudio_prend_le_monitor_pour_le_son_systeme() {
        let args = args_capture(Outil::PulseAudio, true, false);
        assert!(args.contains(&"--device=@DEFAULT_MONITOR@".to_string()), "{args:?}");
    }

    #[test]
    fn les_deux_outils_ecrivent_dans_des_fichiers_d_erreur_distincts() {
        // Sans cela, la seconde tentative effacerait ce que la premiere a dit.
        let piste = Path::new("/tmp/rec/mic.raw");
        assert_ne!(
            err_path(piste, Outil::PipeWire),
            err_path(piste, Outil::PulseAudio)
        );
        assert_eq!(err_path(piste, Outil::PipeWire), Path::new("/tmp/rec/mic.pw.err"));
    }
}
