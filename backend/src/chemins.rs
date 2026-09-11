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

/// L'identifiant du paquet, tel qu'il figure aussi dans la coquille. Un essai le
/// verifie : les deux valeurs doivent rester egales, sinon ce chemin et celui de Tauri
/// divergent EN SILENCE et le fichier cherche n'est jamais trouve.
pub const IDENTIFIANT: &str = "com.cockpit.dev";

/// Le dossier de donnees de l'application, calcule SANS Tauri.
///
/// Pour les chemins qui servent AVANT que la fenetre existe — `rendu::decider()` tourne
/// avant l'initialisation de GTK, donc avant tout `AppHandle` — et pour tout hote qui n'est
/// pas Tauri, le pont compris.
///
/// **ELLE ETAIT SOUS `#[cfg(linux)]` ET CA A CASSE DEUX FOIS LA COMPILATION CROISEE**, le
/// 2026-09-09, une fois dans le journal des terminaux et une fois dans le pont. Une
/// fonction dont tout le monde a besoin ne peut pas n'exister que sur un systeme : la regle
/// de chaque plateforme est donc ecrite ici, celle que Tauri applique de son cote.
///
/// L'alignement avec `app_data_dir()` n'est tenu par un essai que sous Linux, faute d'y
/// pouvoir executer les deux autres. Les regles y sont celles des conventions du systeme,
/// pas une mesure : `%APPDATA%` sous Windows, `~/Library/Application Support` sous macOS.
pub fn calculer_le_dossier_de_donnees() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| dossier_personnel().ok().map(|d| d.join(".local/share")))?;
        Some(base.join(IDENTIFIANT))
    }
    #[cfg(target_os = "macos")]
    {
        Some(
            dossier_personnel()
                .ok()?
                .join("Library/Application Support")
                .join(IDENTIFIANT),
        )
    }
    #[cfg(target_os = "windows")]
    {
        // `%APPDATA%` d'abord : c'est ce que le systeme donne, et il ne vaut pas toujours
        // `%USERPROFILE%\AppData\Roaming` (profil itinerant, redirection de dossier).
        let base = std::env::var_os("APPDATA")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| dossier_personnel().ok().map(|d| d.join("AppData").join("Roaming")))?;
        Some(base.join(IDENTIFIANT))
    }
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

    /// **LE BACKEND ET LA COQUILLE DOIVENT DESIGNER LE MEME DOSSIER.** Le premier le calcule
    /// ici, la seconde le pose sur la page. Si les deux identifiants divergent, le stockage
    /// de l'interface atterrit a cote de celui du backend et chacun lit un dossier vide —
    /// sans une erreur nulle part. C'est arrive une fois, en 0.59.0, faute de cet essai.
    #[cfg(target_os = "linux")]
    #[test]
    fn le_dossier_du_backend_est_celui_de_la_coquille() {
        let coquille = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../coquille/main.js"
        ))
        .expect("main.js de la coquille lisible");
        assert!(
            coquille.contains(&format!("'{IDENTIFIANT}'")),
            "l'identifiant de chemins.rs ({IDENTIFIANT}) n'apparait pas dans la coquille"
        );

        std::env::set_var("XDG_DATA_HOME", "/tmp/essai-donnees");
        assert_eq!(
            calculer_le_dossier_de_donnees(),
            Some(PathBuf::from("/tmp/essai-donnees").join(IDENTIFIANT))
        );
        std::env::remove_var("XDG_DATA_HOME");
        let sans_xdg = calculer_le_dossier_de_donnees().expect("chemin sans XDG_DATA_HOME");
        assert!(
            sans_xdg.ends_with(IDENTIFIANT) && sans_xdg.to_string_lossy().contains(".local/share"),
            "chemin inattendu sans XDG_DATA_HOME : {sans_xdg:?}"
        );
    }
}
