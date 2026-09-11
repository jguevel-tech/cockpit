//! Lancer le service, et le detacher pour de bon.
//!
//! Le service doit survivre a la fermeture de l'application : il ne peut donc etre ni un
//! enfant ordinaire (il mourrait avec le terminal ou le groupe de processus), ni un
//! service systeme (les terminaux appartiennent a une session UTILISATEUR — son `HOME`,
//! son environnement, son presse-papier).
//!
//! - **Unix : double `fork` + `setsid`.** Le premier `fork` est celui de `Command::spawn`.
//!   Le second a lieu dans `pre_exec`, et l'intermediaire disparait aussitot : le
//!   processus que l'appelant attend se termine tout de suite (donc aucun zombie tant que
//!   Cockpit tourne) et le service, orphelin, est adopte par init. `setsid` lui donne
//!   ensuite sa propre session, sans terminal de controle : plus aucun signal du terminal
//!   d'origine ne l'atteint.
//! - **Windows : `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`.** Pas de console heritee,
//!   et le Ctrl+C de la console d'origine ne le concerne plus.
//!
//! La fabrique de commande est un parametre : c'est ce qui permet aux tests de lancer un
//! VRAI processus detache sans construire l'application.

use std::process::{Command, Stdio};

/// L'argument qui fait tourner le binaire en service au lieu d'ouvrir l'application.
pub const DRAPEAU_SERVICE: &str = "--service-terminaux";

/// Combien de temps on laisse au service pour ouvrir son socket.
const ATTENTE_DEMARRAGE: std::time::Duration = std::time::Duration::from_secs(10);

/// La commande qui relance CE binaire en mode service.
pub fn commande_du_service(chemin: &std::path::Path) -> Result<Command, String> {
    let mut commande = Command::new(binaire_du_service()?);
    commande.arg(DRAPEAU_SERVICE).arg(chemin);
    // **L'ENVIRONNEMENT DE L'APPIMAGE NE SUIT PAS LE SERVICE.** Il designe un montage qui
    // disparait a la fermeture de l'application : un `LD_LIBRARY_PATH` qui pointe dedans
    // ferait charger au service des bibliotheques qui s'evaporent sous lui.
    for variable in crate::terminal::environnement::VARIABLES_APPIMAGE {
        commande.env_remove(variable);
    }
    Ok(commande)
}

/// Quel fichier relancer pour rouvrir L'APPLICATION. Sous AppImage, c'est le fichier
/// `.AppImage` pose sur le disque, jamais l'executable monte dans `/tmp/.mount_*` qui
/// disparait avec elle.
///
/// **CE N'EST PAS LE BINAIRE DU SERVICE.** Depuis la 0.59.0, `$APPIMAGE` designe la
/// coquille Electron : la lancer ouvre COCKPIT, pas le service. Voir `binaire_du_service`.
/// Quel fichier lancer pour obtenir LE SERVICE DE TERMINAUX.
///
/// **LANCER `$APPIMAGE` A OUVERT L'APPLICATION AU LIEU DU SERVICE, ET CA A COUTE TROIS
/// PANNES D'UN COUP (2026-09-11).** Jusqu'a la 0.58.x, `$APPIMAGE` et le binaire du service
/// etaient le MEME programme : le relancer avec `--service-terminaux` marchait. Depuis la
/// 0.59.0, `$APPIMAGE` est la coquille Electron, qui ne connait pas ce drapeau : elle
/// ouvrait donc une fenetre et n'ouvrait aucun socket. Consequences vues par l'utilisateur :
/// chaque terminal attendait dix secondes puis repartait a vide, la barre laterale ramait
/// d'autant, et quand il quittait Cockpit le lancement suivant du « service » ROUVRAIT
/// l'application.
///
/// Le service doit donc partir du binaire Rust courant — mais `current_exe()` seul ne suffit
/// pas : il vit dans le montage de l'AppImage, que le runtime demonte a la fermeture de
/// l'application, et le service doit lui SURVIVRE. On en pose donc une copie hors du
/// montage, une fois par version, et c'est elle qu'on lance.
pub(crate) fn binaire_du_service() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("chemin de l'executable : {e}"))?;
    if std::env::var_os("APPIMAGE").is_none() {
        return Ok(exe);
    }
    let donnees = crate::chemins::calculer_le_dossier_de_donnees()
        .ok_or_else(|| "dossier de donnees introuvable".to_string())?;
    poser_la_copie_du_service(&exe, &donnees.join("service"), env!("CARGO_PKG_VERSION"))
}

/// Pose (si besoin) une copie du binaire dans `dossier` et rend son chemin.
///
/// Ecrite a part de toute variable d'environnement pour etre eprouvable : c'est une
/// fonction LIBRE, rien de Tauri n'entre dans le binaire d'essais.
///
/// - La copie porte la VERSION : une mise a jour en pose une neuve au lieu d'ecraser celle
///   qu'un service en cours d'execution utilise peut-etre.
/// - Elle est ecrite a cote puis RENOMMEE : un service ne demarre jamais sur un fichier a
///   moitie copie.
/// - Les copies des autres versions sont retirees. Sans danger pour un service qui tourne :
///   sous Unix il garde son inode ouvert.
pub(crate) fn poser_la_copie_du_service(
    exe: &std::path::Path,
    dossier: &std::path::Path,
    version: &str,
) -> Result<std::path::PathBuf, String> {
    let nom = format!("cockpit-service-{version}{}", std::env::consts::EXE_SUFFIX);
    let cible = dossier.join(&nom);
    let taille_source = std::fs::metadata(exe)
        .map_err(|e| format!("lecture de l'executable : {e}"))?
        .len();
    let deja_bonne = std::fs::metadata(&cible).map(|m| m.len() == taille_source).unwrap_or(false);
    if !deja_bonne {
        std::fs::create_dir_all(dossier)
            .map_err(|e| format!("creation de {} : {e}", dossier.display()))?;
        let temporaire = dossier.join(format!(".{nom}.{}", std::process::id()));
        std::fs::copy(exe, &temporaire).map_err(|e| format!("copie du service : {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporaire, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("droits de la copie du service : {e}"))?;
        }
        std::fs::rename(&temporaire, &cible).map_err(|e| {
            let _ = std::fs::remove_file(&temporaire);
            format!("mise en place du service : {e}")
        })?;
    }
    retirer_les_copies_perimees(dossier, &nom);
    Ok(cible)
}

/// Retire les copies des versions precedentes. Les fichiers temporaires (prefixe `.`) sont
/// laisses : ils peuvent appartenir a un autre lancement en train de copier.
fn retirer_les_copies_perimees(dossier: &std::path::Path, nom_courant: &str) {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return };
    for entree in entrees.flatten() {
        let nom = entree.file_name();
        let Some(nom) = nom.to_str() else { continue };
        if nom == nom_courant || nom.starts_with('.') {
            continue;
        }
        let _ = std::fs::remove_file(entree.path());
    }
}

/// Detache une commande de l'application et la lance.
pub fn lancer_detache(mut commande: Command) -> Result<(), String> {
    commande.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    detacher(&mut commande);
    let mut enfant = commande.spawn().map_err(|e| format!("lancement du service : {e}"))?;
    // Sous Unix c'est l'intermediaire du double fork qui est attendu : il est deja mort.
    // Sous Windows c'est le service lui-meme, et `wait` n'est PAS appele — on le laisse
    // vivre sa vie, le handle est simplement relache.
    #[cfg(unix)]
    {
        enfant.wait().map_err(|e| format!("lancement du service : {e}"))?;
    }
    #[cfg(not(unix))]
    {
        let _ = &mut enfant;
    }
    Ok(())
}

#[cfg(unix)]
fn detacher(commande: &mut Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY : `fork`, `_exit` et `setsid` sont sans danger entre fork et exec (ils
    // figurent dans la liste POSIX des appels utilisables la). Rien d'autre n'est fait
    // ici : pas d'allocation, pas de verrou, pas d'affichage.
    unsafe {
        commande.pre_exec(|| {
            match libc::fork() {
                -1 => return Err(std::io::Error::last_os_error()),
                0 => {}
                // L'intermediaire s'efface : c'est lui que l'appelant attend, et son
                // depart immediat rend le service orphelin, donc adopte par init.
                _ => libc::_exit(0),
            }
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(windows)]
fn detacher(commande: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    commande.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

/// S'assure qu'un service repond sur ce socket, en le lancant s'il le faut.
///
/// Ne rend la main qu'une fois le socket joignable : l'appelant peut enchainer sur une
/// creation de terminal sans course.
pub fn demarrer(chemin: &std::path::Path) -> Result<(), String> {
    demarrer_avec(chemin, || commande_du_service(chemin))
}

/// La meme chose, avec une fabrique de commande a soi (les tests s'en servent pour lancer
/// un service depuis leur propre binaire).
pub fn demarrer_avec(
    chemin: &std::path::Path,
    fabrique: impl Fn() -> Result<Command, String>,
) -> Result<(), String> {
    if super::tuyau::connecter(chemin).is_ok() {
        return Ok(());
    }
    lancer_detache(fabrique()?)?;
    let debut = std::time::Instant::now();
    while debut.elapsed() < ATTENTE_DEMARRAGE {
        if super::tuyau::connecter(chemin).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Err(format!(
        "le service de terminaux n'a pas ouvert son socket ({}) en {} s",
        chemin.display(),
        ATTENTE_DEMARRAGE.as_secs()
    ))
}

/// Appele en tout premier par `main` : si les arguments demandent le service, on le fait
/// tourner ici et l'application ne s'ouvre pas.
///
/// Rend `true` quand le processus doit s'arreter apres cet appel.
pub fn tourner_si_demande() -> bool {
    let mut args = std::env::args_os().skip(1);
    let Some(premier) = args.next() else { return false };
    if premier != DRAPEAU_SERVICE {
        return false;
    }
    let chemin = match args.next() {
        Some(donne) => std::path::PathBuf::from(donne),
        None => match super::tuyau::chemin() {
            Ok(defaut) => defaut,
            Err(e) => {
                eprintln!("service de terminaux : {e}");
                return true;
            }
        },
    };
    if let Err(e) = super::serveur::servir(&chemin, super::serveur::HISTORIQUE) {
        // Personne ne lit cette sortie (le service est detache, ses flux vont au neant) :
        // le vrai canal de diagnostic est l'echec de connexion cote application, qui, lui,
        // remonte a l'utilisateur. Ce message sert au lancement a la main, en recette.
        eprintln!("service de terminaux : {e}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Un dossier a soi, retire a la fin meme si l'essai tombe.
    struct DossierDEssai(PathBuf);
    impl DossierDEssai {
        fn neuf(nom: &str) -> Self {
            let chemin =
                std::env::temp_dir().join(format!("cockpit-{nom}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&chemin);
            std::fs::create_dir_all(&chemin).expect("dossier d'essai");
            Self(chemin)
        }
        fn chemin(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for DossierDEssai {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn faux_binaire(dossier: &std::path::Path, contenu: &str) -> PathBuf {
        let chemin = dossier.join("cockpit");
        std::fs::write(&chemin, contenu).expect("ecriture du faux binaire");
        chemin
    }

    /// **LA REGRESSION DE LA 0.59.0.** Le service partait de `$APPIMAGE`, c'est-a-dire de la
    /// coquille Electron : elle ouvrait l'application et n'ouvrait aucun socket. La copie
    /// doit donc etre celle du binaire COURANT, posee ailleurs que dans le montage.
    #[test]
    fn la_copie_du_service_est_le_binaire_courant_pose_hors_du_montage() {
        let bac = DossierDEssai::neuf("service-copie");
        let montage = bac.chemin().join("mount");
        std::fs::create_dir_all(&montage).unwrap();
        let exe = faux_binaire(&montage, "le binaire rust");
        let dossier = bac.chemin().join("service");

        let pose = poser_la_copie_du_service(&exe, &dossier, "1.2.3").expect("copie");

        assert!(pose.starts_with(&dossier), "la copie doit vivre hors du montage : {pose:?}");
        assert_ne!(pose, exe, "on ne relance pas le fichier du montage");
        assert_eq!(std::fs::read_to_string(&pose).unwrap(), "le binaire rust");
        assert!(pose.to_string_lossy().contains("1.2.3"), "la copie porte la version");
    }

    #[cfg(unix)]
    #[test]
    fn la_copie_du_service_est_executable() {
        use std::os::unix::fs::PermissionsExt;
        let bac = DossierDEssai::neuf("service-droits");
        let exe = faux_binaire(bac.chemin(), "x");
        let pose = poser_la_copie_du_service(&exe, &bac.chemin().join("service"), "0.1.0").unwrap();
        let mode = std::fs::metadata(&pose).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "sans droit d'execution, le service ne demarre pas");
    }

    #[test]
    fn une_copie_deja_a_jour_n_est_pas_refaite() {
        let bac = DossierDEssai::neuf("service-idempotent");
        let exe = faux_binaire(bac.chemin(), "abcd");
        let dossier = bac.chemin().join("service");
        let pose = poser_la_copie_du_service(&exe, &dossier, "9.9.9").unwrap();
        // Un contenu de MEME taille : si la copie etait refaite, il serait ecrase.
        std::fs::write(&pose, "ZZZZ").unwrap();
        let repose = poser_la_copie_du_service(&exe, &dossier, "9.9.9").unwrap();
        assert_eq!(pose, repose);
        assert_eq!(std::fs::read_to_string(&repose).unwrap(), "ZZZZ", "copie refaite pour rien");
    }

    #[test]
    fn une_version_neuve_remplace_la_copie_precedente_et_ne_laisse_rien_derriere() {
        let bac = DossierDEssai::neuf("service-versions");
        let exe = faux_binaire(bac.chemin(), "v1");
        let dossier = bac.chemin().join("service");
        let ancienne = poser_la_copie_du_service(&exe, &dossier, "1.0.0").unwrap();

        std::fs::write(&exe, "v2222").unwrap();
        let neuve = poser_la_copie_du_service(&exe, &dossier, "2.0.0").unwrap();

        assert!(neuve.exists());
        assert!(!ancienne.exists(), "la copie de l'ancienne version reste sur le disque");
        let restants: Vec<_> = std::fs::read_dir(&dossier)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(restants, vec![neuve.file_name().unwrap().to_string_lossy().into_owned()],
            "un fichier temporaire est reste : {restants:?}");
    }
}
