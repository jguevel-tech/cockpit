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
    let mut commande = Command::new(binaire_a_relancer()?);
    commande.arg(DRAPEAU_SERVICE).arg(chemin);
    Ok(commande)
}

/// Quel fichier relancer. NE PAS SIMPLIFIER en `current_exe()` (voir plus bas).
///
/// Sous AppImage, `current_exe()` pointe dans le montage `/tmp/.mount_cockpi*`, que le
/// runtime demonte a la fermeture de l'application. Or le service doit lui SURVIVRE : son
/// executable disparaitrait sous lui. La variable `APPIMAGE` designe, elle, le fichier
/// `.AppImage` pose sur le disque : le service en obtient son propre montage, vivant tant
/// qu'il tourne. C'est la meme lecon que le tmux embarque, qu'il fallait copier hors du
/// montage avant de le lancer.
fn binaire_a_relancer() -> Result<std::path::PathBuf, String> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let chemin = std::path::PathBuf::from(appimage);
        if chemin.is_file() {
            return Ok(chemin);
        }
    }
    std::env::current_exe().map_err(|e| format!("chemin de l'executable : {e}"))
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
