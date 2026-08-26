//! Lancer un programme externe sans faire clignoter une console sous Windows.
//!
//! Une application GRAPHIQUE Windows n'a pas de console. Chaque programme console qu'elle
//! lance en ouvre donc une, le temps de son execution : une fenetre noire qui apparait et
//! disparait. Ce n'est pas un defaut cosmetique isole — le monitor Docker lance un
//! `docker compose ps` PAR PROJET toutes les cinq secondes, en permanence, plus le statut
//! git, la liste des conteneurs, la verification des URLs. Avec cinq projets, cinq
//! clignotements toutes les cinq secondes pendant toute la journee de travail.
//!
//! `CREATE_NO_WINDOW` supprime la console sans rien changer d'autre : les flux restent
//! redirigeables, le code de sortie et la sortie standard sont identiques. Sur Unix
//! l'implementation ne fait RIEN — c'est ce qui permet d'appeler `.sans_console()` partout
//! sans `#[cfg]` chez l'appelant.
//!
//! Aucun test automatique ne peut voir une fenetre clignoter : la seule protection est que
//! tout lancement passe par ici. Un `Command::new(...)` sans `.sans_console()` est un oubli.

/// <https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags>
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A appeler sur toute commande externe, juste apres `new()`.
pub trait SansConsole {
    fn sans_console(&mut self) -> &mut Self;
}

impl SansConsole for std::process::Command {
    fn sans_console(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

impl SansConsole for tokio::process::Command {
    fn sans_console(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// Trouver un programme de l'utilisateur
// ─────────────────────────────────────────────────────────────────────────────────────────────

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Ou chercher un programme, une fois pour toutes.
static CHEMINS: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// Dossiers ou les outils d'un utilisateur s'installent, quand ils ne sont pas dans le PATH
/// d'une application graphique. Relatifs au dossier personnel.
#[cfg(unix)]
const DOSSIERS_HABITUELS: &[&str] =
    &[".local/bin", "bin", ".cargo/bin", ".npm-global/bin", ".bun/bin", "go/bin", ".claude/local"];
#[cfg(windows)]
const DOSSIERS_HABITUELS: &[&str] =
    &["AppData/Roaming/npm", "AppData/Local/Microsoft/WindowsApps", ".cargo/bin", ".bun/bin"];

/// Dossiers systeme a essayer meme absents du PATH (Homebrew sur macOS Apple Silicon).
#[cfg(unix)]
const DOSSIERS_SYSTEME: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];
#[cfg(windows)]
const DOSSIERS_SYSTEME: &[&str] = &[];

/// Construit la liste des dossiers ou chercher.
///
/// **LE PATH DU PROCESSUS NE SUFFIT PAS.** Une application lancee depuis un menu de bureau
/// n'herite pas des fichiers de demarrage du shell : `~/.local/bin` en est absent. Mesure le
/// 2026-08-26 sur une installation reelle — la CLI de Claude Code y vivait et Cockpit
/// l'annoncait « introuvable », en desactivant au passage le bouton de connexion. Un outil
/// present etait declare absent, ce qui envoie chercher une panne qui n'existe pas.
///
/// Trois sources, dans cet ordre : le PATH du shell de CONNEXION quand on a su le demander, le
/// PATH du processus, et les dossiers habituels. Les dossiers du montage AppImage sont retires :
/// ils ne contiennent que nos propres bibliotheques, et un programme homonyme y serait le notre.
///
/// Fonction PURE pour etre testable sans lancer de shell : l'appelant fournit ce qu'il a lu.
fn assembler(du_shell: Option<&str>, du_processus: Option<&str>, maison: Option<&Path>) -> Vec<PathBuf> {
    let appdir = std::env::var("APPDIR").ok().filter(|d| !d.is_empty());
    let mut sortie: Vec<PathBuf> = Vec::new();
    let mut pousser = |chemin: PathBuf| {
        if !chemin.as_os_str().is_empty() && !sortie.contains(&chemin) {
            sortie.push(chemin);
        }
    };

    for brut in [du_shell, du_processus].into_iter().flatten() {
        for morceau in std::env::split_paths(brut) {
            let sous_le_montage = appdir
                .as_deref()
                .is_some_and(|d| morceau.to_string_lossy().starts_with(d));
            if !sous_le_montage {
                pousser(morceau);
            }
        }
    }
    if let Some(maison) = maison {
        for relatif in DOSSIERS_HABITUELS {
            pousser(maison.join(relatif));
        }
    }
    for absolu in DOSSIERS_SYSTEME {
        pousser(PathBuf::from(absolu));
    }
    sortie
}

/// Demande son PATH au shell de connexion. `None` quand on n'a pas su.
///
/// UN SEUL lancement de processus, au demarrage et hors du fil de l'interface : un shell de
/// connexion avec beaucoup de fichiers de configuration met facilement une demi-seconde, et
/// figer l'interface pour savoir ou vit un programme serait absurde.
#[cfg(unix)]
fn path_du_shell() -> Option<String> {
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty())?;
    let sortie = std::process::Command::new(shell)
        .sans_console()
        .args(["-lc", "printf %s \"$PATH\""])
        .output()
        .ok()?;
    if !sortie.status.success() {
        return None;
    }
    let lu = String::from_utf8_lossy(&sortie.stdout).trim().to_string();
    (!lu.is_empty()).then_some(lu)
}

/// Windows n'a pas de shell de connexion qui exporte un PATH : le PATH du processus est deja
/// celui de la session, et cette fonction n'aurait rien a demander.
#[cfg(windows)]
fn path_du_shell() -> Option<String> {
    None
}

/// A appeler UNE fois au demarrage : remplit la liste en tache de fond.
///
/// Tant qu'elle n'est pas prete, `chemin_du_programme` se debrouille avec le PATH du processus
/// et les dossiers habituels — donc l'interface repond tout de suite, et se corrige d'elle-meme
/// une fraction de seconde plus tard.
pub fn precharger_les_chemins() {
    std::thread::spawn(|| {
        let du_shell = path_du_shell();
        let du_processus = std::env::var("PATH").ok();
        let maison = crate::chemins::dossier_personnel().ok();
        let liste = assembler(du_shell.as_deref(), du_processus.as_deref(), maison.as_deref());
        if CHEMINS.set(liste).is_err() {
            log::debug!("les chemins de recherche etaient deja connus");
        }
    });
}

fn chemins_de_recherche() -> Vec<PathBuf> {
    if let Some(prets) = CHEMINS.get() {
        return prets.clone();
    }
    let du_processus = std::env::var("PATH").ok();
    let maison = crate::chemins::dossier_personnel().ok();
    assembler(None, du_processus.as_deref(), maison.as_deref())
}

/// Les extensions qu'un programme peut porter. Sous Windows, `claude` designe `claude.cmd` :
/// chercher le nom nu n'y trouve jamais rien.
fn suffixes() -> Vec<String> {
    if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_lowercase())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    }
}

/// Le chemin COMPLET d'un programme, ou `None`.
///
/// On rend le chemin et pas un booleen : c'est lui qu'il faut lancer. Lancer `claude` par son
/// nom nu echouerait exactement la ou la detection vient de reussir — l'application n'a pas ce
/// dossier dans son PATH, et c'est tout le probleme qu'on corrige.
pub fn chemin_du_programme(nom: &str) -> Option<PathBuf> {
    let suffixes = suffixes();
    for dossier in chemins_de_recherche() {
        for suffixe in &suffixes {
            let candidat = dossier.join(format!("{nom}{suffixe}"));
            if candidat.is_file() {
                return Some(candidat);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le contrat est le meme sur les deux familles de commande, et l'appel doit rester
    /// chainable : c'est ce qui fait qu'on peut l'inserer dans un `Command::new().args()`
    /// existant sans le reecrire.
    #[test]
    fn l_appel_est_chainable_sur_les_deux_familles() {
        let mut std_cmd = std::process::Command::new("true");
        let programme = std_cmd.sans_console().arg("-n").get_program().to_owned();
        assert_eq!(programme, "true");

        let mut tokio_cmd = tokio::process::Command::new("true");
        tokio_cmd.sans_console().arg("-n");
        assert_eq!(tokio_cmd.as_std().get_program(), "true");
    }

    /// UN PATH S'ECRIT AVEC LE SEPARATEUR DE LA PLATEFORME : `:` sous Unix, `;` sous Windows.
    /// Ecrire « /usr/bin:/bin » en dur faisait tomber ces essais sur Windows, ou la chaine
    /// entiere compte pour UN seul dossier — et la compilation croisee ne le dit pas, elle ne
    /// prouve que la compilation. On laisse donc la bibliotheque assembler la chaine.
    fn en_path(morceaux: &[&str]) -> String {
        std::env::join_paths(morceaux.iter().map(Path::new))
            .expect("chemins assemblables")
            .to_string_lossy()
            .into_owned()
    }

    /// **LE DEFAUT CORRIGE.** Le PATH d'une application lancee depuis un menu de bureau ne
    /// contient pas `~/.local/bin`, ou vivent les CLI installes par l'utilisateur. Sans les
    /// dossiers habituels, un outil present etait annonce absent.
    #[test]
    fn les_dossiers_habituels_completent_le_path_du_processus() {
        let maison = PathBuf::from("/home/moi");
        let liste = assembler(None, Some(&en_path(&["/usr/bin", "/bin"])), Some(&maison));

        assert!(liste.contains(&PathBuf::from("/usr/bin")));
        // La liste des dossiers habituels depend de la plateforme : on verifie que le premier
        // d'entre eux y est, pas qu'un chemin Unix y est.
        let attendu = maison.join(DOSSIERS_HABITUELS[0]);
        assert!(
            liste.contains(&attendu),
            "{} doit etre cherche meme absent du PATH : {liste:?}",
            attendu.display()
        );
    }

    /// Le PATH du shell de connexion passe DEVANT celui du processus : c'est lui qui reflete
    /// l'installation de la personne.
    #[test]
    fn le_path_du_shell_passe_devant() {
        let liste = assembler(
            Some(&en_path(&["/opt/outils/bin"])),
            Some(&en_path(&["/usr/bin"])),
            None,
        );
        assert_eq!(liste.first().unwrap(), &PathBuf::from("/opt/outils/bin"));
    }

    /// Les dossiers du montage AppImage sont retires : ils ne portent que nos bibliotheques, et
    /// un programme homonyme trouve la serait le notre. Meme lecon que la fuite d'environnement
    /// vers les terminaux.
    #[test]
    fn les_dossiers_du_montage_appimage_sont_ecartes() {
        // SAFETY: essai a un seul fil sur cette variable, remise a zero juste apres.
        unsafe { std::env::set_var("APPDIR", "/tmp/.mount_Cockpit42") };
        let liste = assembler(
            None,
            Some(&en_path(&["/tmp/.mount_Cockpit42/usr/bin", "/usr/bin"])),
            None,
        );
        unsafe { std::env::remove_var("APPDIR") };

        assert!(!liste.iter().any(|c| c.starts_with("/tmp/.mount_Cockpit42")));
        assert!(liste.contains(&PathBuf::from("/usr/bin")));
    }

    /// Un meme dossier annonce par le shell ET par le processus ne se cherche qu'une fois : la
    /// liste est parcourue pour chaque programme et pour chaque extension.
    #[test]
    fn la_liste_ne_contient_pas_de_doublon() {
        let liste = assembler(
            Some(&en_path(&["/usr/bin", "/bin"])),
            Some(&en_path(&["/bin", "/usr/bin"])),
            None,
        );
        let mut unique = liste.clone();
        unique.dedup();
        assert_eq!(liste.len(), unique.len());
        assert_eq!(liste.iter().filter(|c| *c == &PathBuf::from("/bin")).count(), 1);
    }

    /// Un programme qui n'existe pas ne se trouve pas — et la recherche ne panique pas sur un
    /// dossier inexistant.
    #[test]
    fn un_programme_absent_ne_se_trouve_pas() {
        assert!(chemin_du_programme("cockpit-programme-qui-n-existe-pas").is_none());
    }
}
