//! Remontee des erreurs : journal local, et envoi vers le serveur de suivi si l'utilisateur
//! l'a accepte.
//!
//! POURQUOI CE MODULE EXISTE
//! -------------------------
//! Plusieurs correctifs ont demande des allers-retours avec des utilisateurs pour apprendre
//! des choses que la machine savait deja : la version de `pw-record`, le serveur audio
//! reellement actif, la distribution. Un diagnostic invente a meme ete affiche a la place
//! d'une cause reelle. Une erreur doit donc arriver avec la fiche de la machine qui l'a
//! produite.
//!
//! DEUX SORTIES, DANS CET ORDRE
//!   1. le JOURNAL LOCAL, toujours ecrit : il marche hors ligne, sans consentement, et
//!      c'est lui que l'utilisateur peut relire ou joindre a un signalement ;
//!   2. l'ENVOI, seulement si l'utilisateur a dit oui.
//!
//! REGLE DE TRANSPORT : rien ne part en clair. L'adresse doit etre en `https://`, sauf
//! serveur local (mise au point). Ce sont les erreurs et le nom de quelqu'un d'autre qui
//! circulent : les laisser en HTTP sur Internet serait indefendable.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::commande::SansConsole;

/// Cle de reglage : "on" / "off". Absente = la question n'a pas encore ete posee.
pub const CONSENT_KEY: &str = "error_reporting";
/// Cle de reglage : nom affiche a cote des erreurs remontees.
pub const USER_KEY: &str = "error_reporting_user";

const LOG_NAME: &str = "cockpit.log";
/// Au-dela, le journal est reparti dans un `.1` : on garde de quoi diagnostiquer sans
/// laisser un fichier grossir sans fin.
const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Un message d'erreur peut contenir une sortie de commande entiere.
const MESSAGE_MAX: usize = 1000;

/// Caracteristiques de la machine, jointes a chaque erreur.
#[derive(Clone, Debug, serde::Serialize)]
pub struct MachineInfo {
    pub app_version: String,
    /// "Ubuntu 24.04" — lu dans /etc/os-release.
    pub distro: String,
    /// Serveur audio REELLEMENT actif ("pulseaudio", "pipewire", "aucun") : c'est
    /// l'information qui manquait pour comprendre l'echec d'un enregistrement.
    pub audio_server: String,
    pub pw_record: String,
    /// "appimage" ou "binaire" : les fuites d'environnement de l'AppImage ont deja
    /// explique des pannes absentes du binaire nu.
    pub packaging: String,
}

/// Adresse du serveur de suivi et identifiant du site.
///
/// Fournis au BUILD (le depot est public : l'adresse d'un serveur prive n'y a pas sa
/// place), surchargeables par l'environnement pour la mise au point et la recette.
fn endpoint() -> Option<(String, String)> {
    let url = std::env::var("COCKPIT_REPORT_URL")
        .ok()
        .or_else(|| option_env!("COCKPIT_REPORT_URL").map(str::to_string))?;
    let site = std::env::var("COCKPIT_REPORT_SITE")
        .ok()
        .or_else(|| option_env!("COCKPIT_REPORT_SITE").map(str::to_string))?;
    if url.trim().is_empty() || site.trim().is_empty() {
        return None;
    }
    Some((url.trim().trim_end_matches('/').to_string(), site.trim().to_string()))
}

/// Le transport est-il acceptable pour des donnees personnelles ?
///
/// `https` par defaut, parce que les messages d'erreur contiennent des chemins de fichiers —
/// donc des noms de projets, parfois de clients — et le nom de l'utilisateur.
///
/// Deux exceptions : le serveur LOCAL (mise au point, rien ne quitte la machine) et un
/// `http` assume explicitement par `COCKPIT_REPORT_ALLOW_HTTP=1` au build. Cette porte
/// existe pour ne pas bloquer un serveur maison sans certificat ; elle demande un geste
/// deliberé, ce qui est la difference entre un choix et un oubli.
pub fn transport_acceptable(url: &str) -> bool {
    transport_acceptable_avec(url, http_autorise())
}

fn http_autorise() -> bool {
    let brut = std::env::var("COCKPIT_REPORT_ALLOW_HTTP")
        .ok()
        .or_else(|| option_env!("COCKPIT_REPORT_ALLOW_HTTP").map(str::to_string))
        .unwrap_or_default();
    matches!(brut.trim(), "1" | "true" | "oui")
}

/// Separee pour etre testable sans toucher a l'environnement du processus.
pub fn transport_acceptable_avec(url: &str, http_autorise: bool) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    if let Some(reste) = url.strip_prefix("http://") {
        if http_autorise {
            return true;
        }
        let hote = reste.split(['/', ':']).next().unwrap_or("");
        return hote == "localhost" || hote == "127.0.0.1" || hote == "::1";
    }
    false
}

/// Version de pw-record.
///
/// `pw-record --version` repond sur trois lignes dont la premiere est le nom du programme :
/// la prendre telle quelle donnait « pw-record » au lieu du numero, et c'est justement ce
/// numero qui a explique une panne (l'option `-P` n'existe pas avant 0.3.5x).
pub fn pw_record_version(sortie: &str) -> String {
    for ligne in sortie.lines() {
        let l = ligne.trim();
        if let Some(reste) = l.strip_prefix("Compiled with libpipewire") {
            return reste.trim().to_string();
        }
    }
    // Rien de reconnu : on renvoie la premiere ligne utile, faute de mieux.
    sortie
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && *l != "pw-record")
        .unwrap_or("")
        .to_string()
}

/// Valeur d'une cle de /etc/os-release.
fn os_release_value(contenu: &str, cle: &str) -> Option<String> {
    contenu.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == cle).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// Nom lisible de la distribution.
pub fn distro_from_os_release(contenu: &str) -> String {
    let nom = os_release_value(contenu, "NAME").unwrap_or_default();
    let version = os_release_value(contenu, "VERSION_ID").unwrap_or_default();
    match (nom.is_empty(), version.is_empty()) {
        (true, true) => "inconnue".to_string(),
        (false, true) => nom,
        (true, false) => version,
        (false, false) => format!("{nom} {version}"),
    }
}

/// Le systeme, tel qu'il figure a cote des erreurs.
///
/// `/etc/os-release` d'abord : c'est le seul endroit qui distingue Ubuntu de Fedora, et le
/// CLAUDE.md rappelle que c'est cette fiche « qui a manque pendant plusieurs corrections ».
/// Hors Linux le fichier n'existe pas, et la fiche affichait alors « inconnue » : on porte
/// a la aussi une reponse, celle de `sysinfo`, qui rend « Windows 11 » ou « macOS 14.6 ».
fn systeme_lisible() -> String {
    let depuis_os_release =
        distro_from_os_release(&std::fs::read_to_string("/etc/os-release").unwrap_or_default());
    if depuis_os_release != "inconnue" {
        return depuis_os_release;
    }
    sysinfo::System::long_os_version().unwrap_or_else(|| "inconnue".to_string())
}

/// Serveur audio actif, d'apres la sortie de `pactl info`.
///
/// Sur les systemes ou PipeWire remplace PulseAudio, `pactl` repond « PulseAudio (on
/// PipeWire 1.0.5) » : c'est bien PipeWire qui tient l'audio. Distinguer les deux est
/// exactement ce qui manquait pour comprendre un echec d'enregistrement.
pub fn audio_server_from_pactl(sortie: &str) -> String {
    let ligne = sortie
        .lines()
        .find(|l| l.starts_with("Server Name:"))
        .unwrap_or("")
        .to_lowercase();
    if ligne.contains("pipewire") {
        "pipewire".to_string()
    } else if ligne.contains("pulseaudio") {
        "pulseaudio".to_string()
    } else {
        "aucun".to_string()
    }
}

fn sortie_commande(programme: &str, args: &[&str]) -> String {
    std::process::Command::new(programme)
        .sans_console()
        .args(args)
        .output()
        .map(|o| {
            let mut texte = String::from_utf8_lossy(&o.stdout).to_string();
            texte.push_str(&String::from_utf8_lossy(&o.stderr));
            texte
        })
        .unwrap_or_default()
}

/// Fiche de la machine, calculee une seule fois : elle interroge des commandes externes.
pub fn machine_info() -> &'static MachineInfo {
    static INFO: OnceLock<MachineInfo> = OnceLock::new();
    INFO.get_or_init(|| MachineInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        distro: systeme_lisible(),
        audio_server: audio_server_from_pactl(&sortie_commande("pactl", &["info"])),
        pw_record: pw_record_version(&sortie_commande("pw-record", &["--version"])),
        packaging: if std::env::var_os("APPDIR").is_some() {
            "appimage".to_string()
        } else {
            "binaire".to_string()
        },
    })
}

fn log_path(app_data: &Path) -> PathBuf {
    app_data.join("logs").join(LOG_NAME)
}

/// Ecrit une ligne dans le journal local, en le faisant tourner s'il est trop gros.
pub fn append_log(app_data: &Path, ligne: &str) {
    let chemin = log_path(app_data);
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0) > LOG_MAX_BYTES {
        let _ = std::fs::rename(&chemin, chemin.with_extension("log.1"));
    }
    // Le journal ne doit jamais faire echouer ce qu'il observe : une ecriture impossible
    // (disque plein, droits) est ignoree.
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&chemin) {
        let _ = writeln!(f, "{ligne}");
    }
}

/// Ligne de journal : horodatee, sur une seule ligne pour rester lisible et greppable.
pub fn format_log_line(horodatage: &str, scope: &str, message: &str) -> String {
    format!(
        "{horodatage} [{scope}] {}",
        message.replace('\n', " ").replace('\r', " ")
    )
}

fn borne(texte: &str) -> String {
    if texte.chars().count() > MESSAGE_MAX {
        texte.chars().take(MESSAGE_MAX).collect()
    } else {
        texte.to_string()
    }
}

/// Corps de l'evenement envoye au serveur de suivi (format Umami).
///
/// `hostname` reprend le domaine declare pour le site, sinon l'evenement est refuse.
pub fn build_payload(
    site: &str,
    scope: &str,
    message: &str,
    info: &MachineInfo,
    utilisateur: &str,
) -> serde_json::Value {
    serde_json::json!({
        "type": "event",
        "payload": {
            "website": site,
            "hostname": "cockpit.local",
            "language": "fr",
            "screen": "0x0",
            "url": format!("/{scope}"),
            "title": scope,
            "name": "erreur",
            "data": {
                "scope": scope,
                "message": borne(message),
                "version": info.app_version,
                "distro": info.distro,
                "audio": info.audio_server,
                "pw_record": info.pw_record,
                "packaging": info.packaging,
                "utilisateur": utilisateur,
            }
        }
    })
}

/// Envoie l'evenement. Silencieux par nature : une remontee qui echoue ne doit ni bloquer
/// ni afficher quoi que ce soit — l'utilisateur a deja son erreur a l'ecran.
pub async fn send(scope: &str, message: &str, utilisateur: &str) {
    let Some((url, site)) = endpoint() else { return };
    if !transport_acceptable(&url) {
        return;
    }
    let corps = build_payload(&site, scope, message, machine_info(), utilisateur);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };
    // Umami exige un User-Agent : sans lui l'evenement est ignore sans erreur.
    let _ = client
        .post(format!("{url}/api/send"))
        .header(
            "User-Agent",
            format!("Cockpit/{} (X11; Linux x86_64)", machine_info().app_version),
        )
        .json(&corps)
        .send()
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_est_accepte() {
        assert!(transport_acceptable("https://suivi.example.org"));
    }

    #[test]
    fn http_public_est_refuse_par_defaut() {
        // Ce sont les erreurs et le nom d'un collegue : pas en clair sans le vouloir.
        assert!(!transport_acceptable_avec("http://umami.86.253.219.203.sslip.io", false));
    }

    #[test]
    fn http_public_passe_si_on_l_assume() {
        // Serveur maison sans certificat : la porte existe, mais elle se pousse a la main.
        assert!(transport_acceptable_avec("http://umami.86.253.219.203.sslip.io", true));
    }

    #[test]
    fn http_local_est_accepte_pour_la_mise_au_point() {
        assert!(transport_acceptable_avec("http://localhost:3000", false));
        assert!(transport_acceptable_avec("http://127.0.0.1:3000/", false));
    }

    #[test]
    fn adresse_sans_protocole_est_refusee_meme_si_http_autorise() {
        assert!(!transport_acceptable_avec("suivi.example.org", true));
    }

    #[test]
    fn distribution_lue_dans_os_release() {
        let contenu = "PRETTY_NAME=\"Ubuntu 22.04.5 LTS\"\nNAME=\"Ubuntu\"\nVERSION_ID=\"22.04\"\n";
        assert_eq!(distro_from_os_release(contenu), "Ubuntu 22.04");
    }

    #[test]
    fn distribution_inconnue_ne_casse_pas() {
        assert_eq!(distro_from_os_release(""), "inconnue");
    }

    #[test]
    fn serveur_audio_pulseaudio_reconnu() {
        // Le cas qui a coute des allers-retours : PipeWire installe, PulseAudio aux
        // commandes.
        assert_eq!(
            audio_server_from_pactl("Server Name: pulseaudio\nServer Version: 15.99.1\n"),
            "pulseaudio"
        );
    }

    #[test]
    fn serveur_audio_pipewire_reconnu_derriere_la_compatibilite_pulse() {
        assert_eq!(
            audio_server_from_pactl("Server Name: PulseAudio (on PipeWire 1.0.5)\n"),
            "pipewire"
        );
    }

    #[test]
    fn sans_serveur_audio_on_le_dit() {
        assert_eq!(audio_server_from_pactl(""), "aucun");
    }

    #[test]
    fn version_de_pw_record_extraite_et_non_le_nom_du_programme() {
        let sortie = "pw-record\nCompiled with libpipewire 0.3.48\nLinked with libpipewire 0.3.48\n";
        assert_eq!(pw_record_version(sortie), "0.3.48");
    }

    #[test]
    fn version_de_pw_record_absente_ne_rend_pas_le_nom() {
        assert_eq!(pw_record_version("pw-record\n"), "");
        assert_eq!(pw_record_version(""), "");
    }

    #[test]
    fn ligne_de_journal_tient_sur_une_ligne() {
        let ligne = format_log_line("2026-08-19 14:00:00", "git.commit", "echec\nsur deux lignes");
        assert!(!ligne.trim_end().contains('\n'), "{ligne}");
        assert!(ligne.contains("[git.commit]"), "{ligne}");
    }

    #[test]
    fn le_message_envoye_est_borne() {
        let info = MachineInfo {
            app_version: "0.28.1".into(),
            distro: "Ubuntu 22.04".into(),
            audio_server: "pulseaudio".into(),
            pw_record: "0.3.48".into(),
            packaging: "appimage".into(),
        };
        let long = "x".repeat(5000);
        let payload = build_payload("site", "recorder.start", &long, &info, "gilles");
        let message = payload["payload"]["data"]["message"].as_str().unwrap();
        assert!(message.chars().count() <= MESSAGE_MAX, "{}", message.len());
    }

    #[test]
    fn la_fiche_machine_accompagne_l_erreur() {
        let info = MachineInfo {
            app_version: "0.28.1".into(),
            distro: "Ubuntu 22.04".into(),
            audio_server: "pulseaudio".into(),
            pw_record: "0.3.48".into(),
            packaging: "appimage".into(),
        };
        let payload = build_payload("site", "recorder.start", "no node available", &info, "gilles");
        let data = &payload["payload"]["data"];
        assert_eq!(data["audio"], "pulseaudio");
        assert_eq!(data["distro"], "Ubuntu 22.04");
        assert_eq!(data["packaging"], "appimage");
        assert_eq!(data["utilisateur"], "gilles");
        assert_eq!(payload["payload"]["name"], "erreur");
        // Umami refuse un evenement dont l'hote ne correspond pas au site declare.
        assert_eq!(payload["payload"]["hostname"], "cockpit.local");
    }
}
