//! Ou vivent les fichiers de l'utilisateur, sur les trois systemes.
//!
//! Un SEUL endroit resout le dossier personnel. Il y en avait six, tous ecrits
//! `std::env::var("HOME")`, et tous avec le meme defaut : ils repondaient « rien trouve »
//! (`Ok(vec![])`, `logged_in: false`, `"/root"`) au lieu de dire que le dossier personnel
//! etait introuvable. Sous Windows, ou `HOME` n'existe pas, ca donnait des sessions Claude
//! vides, un marketplace d'agents introuvable et un historique de commandes muet — sans un
//! mot pour expliquer pourquoi.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Le dossier personnel de l'utilisateur courant.
pub fn dossier_personnel() -> Result<PathBuf, String> {
    resoudre_dossier_personnel(&|nom| std::env::var_os(nom).filter(|v| !v.is_empty()))
}

/// La regle, separee de la lecture de l'environnement pour etre testable — meme decoupe que
/// `terminal::environnement::modifications`.
///
/// `HOME` d'abord, y compris sous Windows : les outils portes d'Unix (git, la CLI `claude`)
/// le posent et le respectent, donc s'en ecarter ferait chercher les memes fichiers a deux
/// endroits differents. Puis `USERPROFILE`, puis le couple `HOMEDRIVE`+`HOMEPATH` que
/// certains profils de domaine sont seuls a renseigner.
///
/// Rend une ERREUR explicite, jamais un chemin invente : un appelant qui n'a pas de dossier
/// personnel doit le DIRE, pas afficher une liste vide.
fn resoudre_dossier_personnel(
    lire: &dyn Fn(&str) -> Option<OsString>,
) -> Result<PathBuf, String> {
    if let Some(home) = lire("HOME") {
        return Ok(PathBuf::from(home));
    }
    if let Some(profil) = lire("USERPROFILE") {
        return Ok(PathBuf::from(profil));
    }
    if let (Some(lecteur), Some(chemin)) = (lire("HOMEDRIVE"), lire("HOMEPATH")) {
        // Concatenation d'OsString et NON `PathBuf::push` : `HOMEPATH` vaut `\Users\moi`,
        // un chemin qui a une racine. `push` le traiterait comme absolu et JETTERAIT le
        // lecteur — c'est ce que fait la version Unix de `PathBuf`, donc le resultat
        // dependrait du systeme qui compile.
        let mut base = lecteur;
        base.push(chemin);
        return Ok(PathBuf::from(base));
    }
    Err("dossier personnel introuvable : ni HOME, ni USERPROFILE, ni HOMEDRIVE+HOMEPATH \
         ne sont definis"
        .to_string())
}

/// L'identifiant du paquet, tel qu'il figure aussi dans `tauri.conf.json`. Un essai le
/// verifie : les deux valeurs doivent rester egales, sinon ce chemin et celui de Tauri
/// divergent EN SILENCE et le fichier cherche n'est jamais trouve.
#[cfg(target_os = "linux")]
pub const IDENTIFIANT: &str = "com.cockpit.dev";

/// Le dossier de donnees de l'application, calcule SANS Tauri.
///
/// Pour les rares chemins qui servent AVANT que la fenetre existe — `rendu::decider()`
/// tourne avant l'initialisation de GTK, donc avant tout `AppHandle`. Sous Linux c'est
/// `XDG_DATA_HOME` (ou `~/.local/share`) auquel Tauri ajoute l'identifiant : la regle est
/// recopiee ici, et un essai la tient alignee sur `tauri.conf.json`.
#[cfg(target_os = "linux")]
pub fn dossier_donnees_sans_tauri() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| dossier_personnel().ok().map(|d| d.join(".local/share")))?;
    Some(base.join(IDENTIFIANT))
}

/// Le dossier de donnees de l'application, memorise au demarrage.
///
/// Le hook de panic en a besoin : a l'instant d'un panic on ne peut pas compter sur le
/// handle Tauri. Il reconstruisait donc un chemin `~/.local/share/com.cockpit.dev` a la
/// main — juste sous Linux, un dossier fantome sous macOS, nulle part sous Windows. On
/// memorise plutot le VRAI chemin pendant le `setup`, quand tout va bien.
static DOSSIER_DONNEES: OnceLock<PathBuf> = OnceLock::new();

/// Appelee une fois pendant le `setup`, avec le chemin que Tauri a resolu.
pub fn memoriser_dossier_donnees(chemin: PathBuf) {
    let _ = DOSSIER_DONNEES.set(chemin);
}

/// Le dossier de donnees, s'il a ete memorise. `None` avant le `setup` — un panic si tot
/// n'a nulle part a ecrire, et inventer un chemin serait pire que de ne rien ecrire.
pub fn dossier_donnees() -> Option<&'static PathBuf> {
    DOSSIER_DONNEES.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn faux(paires: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        let paires: Vec<_> = paires.iter().map(|(k, v)| (*k, *v)).collect();
        move |nom| {
            paires
                .iter()
                .find(|(k, _)| *k == nom)
                .map(|(_, v)| OsString::from(*v))
        }
    }

    #[test]
    fn home_l_emporte_sur_le_reste() {
        let lire = faux(&[("HOME", "/home/moi"), ("USERPROFILE", r"C:\Users\moi")]);
        assert_eq!(
            resoudre_dossier_personnel(&lire).unwrap(),
            PathBuf::from("/home/moi")
        );
    }

    #[test]
    fn userprofile_prend_le_relais_sans_home() {
        let lire = faux(&[("USERPROFILE", r"C:\Users\moi")]);
        assert_eq!(
            resoudre_dossier_personnel(&lire).unwrap(),
            PathBuf::from(r"C:\Users\moi")
        );
    }

    #[test]
    fn le_couple_homedrive_homepath_est_recolle() {
        let lire = faux(&[("HOMEDRIVE", "C:"), ("HOMEPATH", r"\Users\moi")]);
        assert_eq!(
            resoudre_dossier_personnel(&lire).unwrap(),
            PathBuf::from(r"C:\Users\moi")
        );
    }

    /// Le message de refus doit NOMMER les variables cherchees : c'est la seule chose qui
    /// permette a quelqu'un de le corriger chez lui.
    #[test]
    fn rien_de_pose_donne_une_erreur_qui_nomme_les_variables() {
        let erreur = resoudre_dossier_personnel(&faux(&[])).unwrap_err();
        for variable in ["HOME", "USERPROFILE", "HOMEDRIVE", "HOMEPATH"] {
            assert!(erreur.contains(variable), "{variable} absent de « {erreur} »");
        }
    }

    /// Une variable posee mais VIDE ne vaut pas un chemin : `PathBuf::from("")` donnerait
    /// des chemins relatifs au dossier courant, donc des lectures au hasard.
    #[test]
    fn une_variable_vide_ne_compte_pas() {
        // La regle vit dans le filtre de `dossier_personnel` ; on verifie qu'un lecteur qui
        // rend une chaine vide n'est pas accepte comme dossier.
        let lire = |nom: &str| {
            if nom == "HOME" {
                Some(OsString::new())
            } else {
                None
            }
        };
        let filtre = |nom: &str| lire(nom).filter(|v: &OsString| !v.is_empty());
        assert!(resoudre_dossier_personnel(&filtre).is_err());
    }

    /// Sur les machines des trois systemes, au moins une des variables est posee : la
    /// fonction ne doit pas echouer la ou l'application tourne.
    #[test]
    fn le_dossier_personnel_est_trouve_sur_cette_machine() {
        let dossier = dossier_personnel().expect("dossier personnel");
        assert!(!dossier.as_os_str().is_empty());
    }

    /// `dossier_donnees_sans_tauri` recopie la regle de Tauri : `XDG_DATA_HOME` (sinon
    /// `~/.local/share`) puis l'identifiant. Si l'identifiant diverge de `tauri.conf.json`,
    /// le chemin calcule avant GTK ne designe plus le meme dossier que celui de
    /// `app_data_dir()` et le fichier pose d'un cote est invisible de l'autre — sans aucune
    /// erreur nulle part. D'ou cet essai.
    #[cfg(target_os = "linux")]
    #[test]
    fn le_chemin_sans_tauri_monte_vers_le_dossier_de_tauri() {
        let conf = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"))
            .expect("tauri.conf.json lisible");
        let conf: serde_json::Value = serde_json::from_str(&conf).expect("tauri.conf.json valide");
        assert_eq!(
            conf["identifier"].as_str(),
            Some(IDENTIFIANT),
            "l'identifiant recopie dans chemins.rs a diverge de tauri.conf.json"
        );

        std::env::set_var("XDG_DATA_HOME", "/tmp/essai-donnees");
        assert_eq!(
            dossier_donnees_sans_tauri(),
            Some(PathBuf::from("/tmp/essai-donnees").join(IDENTIFIANT))
        );
        std::env::remove_var("XDG_DATA_HOME");
        let sans_xdg = dossier_donnees_sans_tauri().expect("chemin sans XDG_DATA_HOME");
        assert!(
            sans_xdg.ends_with(IDENTIFIANT) && sans_xdg.to_string_lossy().contains(".local/share"),
            "chemin inattendu sans XDG_DATA_HOME : {sans_xdg:?}"
        );
    }
}
