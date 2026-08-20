//! Le tuyau : ou ecoute le service, et comment on refuse de parler a quelqu'un d'autre.
//!
//! `interprocess` couvre le socket de domaine Unix et le tuyau nomme Windows derriere la
//! meme interface. Ce qui reste a decider ici est le NOM, et la verification que l'autre
//! bout est bien l'utilisateur courant.
//!
//! **Un service par utilisateur, jamais un service systeme.** Les terminaux appartiennent
//! a une session utilisateur : son `HOME`, son environnement, son presse-papier. Un
//! service partage devrait usurper une identite a chaque commande — et un socket partage
//! donnerait a n'importe quel compte de la machine un shell chez l'utilisateur.
//!
//! **Le chemin n'est PAS celui de tmux.** Le socket de tmux (`/tmp/tmux-<uid>/cockpit`)
//! continue de vivre sa vie tant que l'etape C n'a pas eu lieu ; les deux doivent pouvoir
//! tourner en meme temps sur la meme machine.

use std::io;
use std::path::PathBuf;

use interprocess::local_socket::traits::{Stream as _, StreamCommon as _};
use interprocess::local_socket::{GenericFilePath, Listener, ListenerOptions, Stream, ToFsName};

/// De quoi pointer un service donne. Permet a un test — ou a une installation de
/// developpement pilotee par `COCKPIT_DB` — d'avoir son propre service sans toucher a
/// celui de l'utilisateur.
pub const VARIABLE_SOCKET: &str = "COCKPIT_TERMINAUX_SOCKET";

/// Le chemin du socket du service de terminaux de l'utilisateur courant.
///
/// Sous Unix il vit dans un dossier a nous, cree en 0700 : c'est ce dossier qui protege le
/// socket, `interprocess` n'offrant pas de reglage de permissions a la creation.
pub fn chemin() -> io::Result<PathBuf> {
    if let Some(force) = std::env::var_os(VARIABLE_SOCKET) {
        return Ok(PathBuf::from(force));
    }
    Ok(dossier_utilisateur()?.join("terminaux.sock"))
}

#[cfg(unix)]
fn dossier_utilisateur() -> io::Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    let uid = unsafe { libc::geteuid() };
    // XDG_RUNTIME_DIR est deja en 0700 et nettoye a la deconnexion : c'est la bonne place.
    // Sans lui (sessions sans systemd, conteneurs), un dossier a nous dans le temporaire,
    // suffixe par l'uid pour que deux comptes ne se marchent pas dessus.
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
        .map(|d| d.join("cockpit"))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("cockpit-{uid}")));

    if !base.exists() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&base)?;
    }
    let infos = std::fs::metadata(&base)?;
    if !infos.is_dir() {
        return Err(refus(format!("{} n'est pas un dossier", base.display())));
    }
    if infos.uid() != uid {
        return Err(refus(format!(
            "{} appartient a l'utilisateur {}, pas a {uid}",
            base.display(),
            infos.uid()
        )));
    }
    // Ecriture ouverte au groupe ou au monde = n'importe qui peut y poser SON socket et
    // recevoir nos frappes. On refuse au lieu de corriger en silence : le dossier peut
    // avoir ete ouvert volontairement, et le durcir dans le dos de l'utilisateur serait
    // aussi surprenant.
    let mode = infos.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(refus(format!(
            "{} est accessible en ecriture au-dela de son proprietaire (mode {mode:o})",
            base.display()
        )));
    }
    Ok(base)
}

#[cfg(windows)]
fn dossier_utilisateur() -> io::Result<PathBuf> {
    // Les tuyaux nommes ne vivent pas dans le systeme de fichiers : le « chemin » est un
    // nom dans l'espace `\\.\pipe\`, rendu propre a l'utilisateur par son nom de compte.
    // La protection ne vient pas d'un dossier mais de la verification du pair
    // (`verifier_pair`) et du fait qu'un tuyau nomme n'est accessible qu'aux processus
    // autorises par sa liste de controle par defaut, celle du compte qui l'a cree.
    let compte = std::env::var("USERNAME").unwrap_or_else(|_| "utilisateur".into());
    Ok(PathBuf::from(format!(r"\\.\pipe\cockpit-{compte}")))
}

fn refus(detail: String) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, detail)
}

/// Ouvre l'ecoute. Un fichier de socket laisse par un service mort est nettoye — mais
/// SEULEMENT apres avoir verifie que plus personne ne repond dessus.
pub fn ecouter(chemin: &std::path::Path) -> io::Result<Listener> {
    match ecoute_brute(chemin) {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            if connecter(chemin).is_ok() {
                // Quelqu'un repond : ce n'est pas un reste, c'est un service vivant.
                return Err(e);
            }
            // Personne au bout : le fichier survit a un service tue (kill -9, panne de
            // courant). Sans ce nettoyage, plus aucun terminal ne peut s'ouvrir jusqu'a
            // ce que l'utilisateur efface le fichier a la main.
            #[cfg(unix)]
            let _ = std::fs::remove_file(chemin);
            ecoute_brute(chemin)
        }
        autre => autre,
    }
}

fn ecoute_brute(chemin: &std::path::Path) -> io::Result<Listener> {
    let nom = chemin.to_fs_name::<GenericFilePath>()?;
    ListenerOptions::new().name(nom).create_sync()
}

/// Se connecte au service. Echoue si personne n'ecoute — c'est ainsi que l'application
/// sait qu'elle doit le lancer.
pub fn connecter(chemin: &std::path::Path) -> io::Result<Stream> {
    let nom = chemin.to_fs_name::<GenericFilePath>()?;
    Stream::connect(nom)
}

/// L'autre bout est-il bien l'utilisateur courant ?
///
/// Vaut dans LES DEUX SENS : le service refuse une connexion venue d'un autre compte, et
/// le client refuse de confier ses frappes a un service qui n'est pas le sien. Un socket
/// pose par quelqu'un d'autre a l'endroit attendu recevrait sinon tout ce qui est tape.
pub fn verifier_pair(flux: &Stream) -> Result<(), String> {
    let creds = flux.peer_creds().map_err(|e| format!("identite du pair illisible : {e}"))?;
    verdict_sur_le_pair(creds.euid())
}

/// La decision, separee de la lecture des identifiants pour etre testable.
#[cfg(unix)]
fn verdict_sur_le_pair(euid: Option<libc::uid_t>) -> Result<(), String> {
    let nous = unsafe { libc::geteuid() };
    match euid {
        Some(pair) if pair == nous => Ok(()),
        Some(pair) => Err(format!(
            "le service de terminaux tourne sous l'utilisateur {pair}, pas {nous}"
        )),
        // Un Unix qui ne rend pas l'uid du pair n'existe pas parmi les cibles du projet ;
        // s'il en apparaissait un, refuser vaut mieux que faire confiance.
        None => Err("l'identite du pair n'est pas disponible sur ce systeme".into()),
    }
}

#[cfg(windows)]
fn verdict_sur_le_pair(_euid: Option<u32>) -> Result<(), String> {
    // Sous Windows la protection est portee par le tuyau nomme lui-meme : sa liste de
    // controle par defaut n'autorise que le compte qui l'a cree. `peer_creds` n'y rend pas
    // d'uid, il n'y a donc rien a comparer.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le chemin doit etre propre a l'utilisateur, et surtout PAS celui de tmux : les deux
    /// serveurs cohabitent jusqu'a l'etape C.
    #[test]
    #[cfg(unix)]
    fn le_chemin_est_a_nous_et_pas_a_tmux() {
        // La variable d'environnement est peut-etre posee par un autre test qui tourne en
        // parallele : on interroge la fonction de dossier, pas `chemin()`.
        let dossier = dossier_utilisateur().expect("dossier utilisateur");
        let chemin = dossier.join("terminaux.sock");
        assert!(!chemin.to_string_lossy().contains("tmux"), "{}", chemin.display());
        assert!(chemin.to_string_lossy().contains("cockpit"), "{}", chemin.display());
    }

    #[test]
    fn la_variable_d_environnement_l_emporte() {
        // Test sans effet de bord sur l'environnement partage : on verifie la regle sur la
        // valeur, pas en posant la variable (les tests tournent en parallele).
        let force = std::env::var_os(VARIABLE_SOCKET);
        match force {
            Some(v) => assert_eq!(chemin().unwrap(), PathBuf::from(v)),
            None => assert!(chemin().unwrap().ends_with("terminaux.sock")),
        }
    }

    #[test]
    #[cfg(unix)]
    fn un_pair_d_un_autre_compte_est_refuse() {
        let nous = unsafe { libc::geteuid() };
        assert!(verdict_sur_le_pair(Some(nous)).is_ok());
        let erreur = verdict_sur_le_pair(Some(nous + 1)).unwrap_err();
        assert!(erreur.contains(&(nous + 1).to_string()), "{erreur}");
        assert!(verdict_sur_le_pair(None).is_err());
    }

    /// Un socket laisse par un service tue ne doit pas condamner les terminaux : c'est le
    /// cas apres un `kill -9`, et il se produit pour de bon.
    #[test]
    #[cfg(unix)]
    fn un_socket_orphelin_est_nettoye() {
        let dossier = std::env::temp_dir().join(format!("cockpit-test-orphelin-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join("terminaux.sock");
        {
            let _ecoute = ecouter(&chemin).unwrap();
        } // le listener tombe : sous Unix, interprocess efface son nom en partant

        // On refabrique le cas reel : un fichier de socket present sans personne derriere.
        std::fs::write(&chemin, b"").unwrap();
        let ecoute = ecouter(&chemin);
        assert!(ecoute.is_ok(), "{:?}", ecoute.err());
        drop(ecoute);
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
