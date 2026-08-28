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
/// Le PATH que le shell de connexion exporte, extrait de son environnement.
///
/// Il se lisait par une SECONDE interrogation du shell, en mode non interactif : elle ne voyait
/// donc pas ce que `~/.zshrc` ajoute au PATH. Une seule interrogation desormais, et elle est
/// interactive.
#[cfg(unix)]
fn path_du_shell() -> Option<String> {
    env_du_shell()
        .iter()
        .find(|(nom, _)| nom == "PATH")
        .map(|(_, valeur)| valeur.clone())
        .filter(|v| !v.is_empty())
}

/// Windows n'a pas de shell de connexion qui exporte un PATH : le PATH du processus est deja
/// celui de la session, et cette fonction n'aurait rien a demander.
#[cfg(windows)]
fn path_du_shell() -> Option<String> {
    None
}


// ─── L'environnement du shell de connexion ────────────────────────────────────────────────
//
// **MEME FAMILLE QUE LE PATH, ET CA A COUTE DES ERREURS DOCKER CHEZ PLUSIEURS UTILISATEURS.**
// Une application lancee depuis un menu de bureau n'a pas lu les fichiers de demarrage du
// shell : les variables que l'utilisateur y exporte n'existent pas pour elle. Mesure du
// 2026-08-28 sur une application qui tournait : `CCM_SPHINX_DIR`, `NPM_TOKEN` et
// `COMPOSER_AUTH` etaient dans le `.zshrc`, et ABSENTES des trois. `docker compose` recevait
// donc un volume `${CCM_SPHINX_DIR}:/mnt/sphinxsearch` avec une variable vide, et refusait :
// « invalid spec: :/mnt/sphinxsearch: empty section between colons ». La meme commande tapee
// dans un terminal marchait — d'ou une panne incomprehensible vue de l'utilisateur.

/// Les variables du shell de connexion, demandees une seule fois.
static ENV_DU_SHELL: OnceLock<Vec<(String, String)>> = OnceLock::new();

/// Ce qu'on n'injecte JAMAIS : ces variables decrivent l'etat du shell qui a repondu, pas
/// l'environnement de travail. `PATH` est traite a part, plus haut dans ce fichier.
const JAMAIS_INJECTEES: &[&str] = &["PWD", "OLDPWD", "SHLVL", "_", "PATH"];

/// Le marqueur qui separe le bruit de configuration des variables.
///
/// Un shell INTERACTIF a le droit d'ecrire : banniere, message de gestionnaire de plugins,
/// invite. Sans repere, ce bruit se collerait a la premiere variable lue.
// Sous Windows, rien ne lit ceci : il n'y a pas de shell de connexion a interroger. On garde
// pourtant le code compile partout, parce que l'analyse est PORTABLE et tenue par des essais qui
// tournent sur les deux cibles — les faire disparaitre sous Windows y supprimerait leur couverture.
#[cfg_attr(windows, allow(dead_code))]
const MARQUEUR: &str = "@@COCKPIT@@";

/// Combien de temps on laisse au shell. Au-dela, on le tue.
///
/// Un shell interactif charge beaucoup de choses, et certains gestionnaires de plugins
/// interrogent le reseau au demarrage. Sans borne, un fil et un processus resteraient coinces
/// pour toute la duree de la session.
#[cfg(unix)]
const DELAI_SHELL: std::time::Duration = std::time::Duration::from_secs(15);

/// Analyse ce que le shell a repondu. Pure, donc verifiable sans lancer de shell.
///
/// Deux separateurs acceptes : l'octet nul (`env -0`, ce que rend Linux) et le retour a la
/// ligne (repli pour les `env` qui ignorent `-0`, dont celui de macOS). Une entree qui ne
/// ressemble pas a `NOM=...` est jetee : c'est ce qui protege du bruit restant.
#[cfg_attr(windows, allow(dead_code))]
fn analyser_env(sortie: &[u8]) -> Vec<(String, String)> {
    let texte = String::from_utf8_lossy(sortie);
    // Tout ce qui precede le marqueur est du bruit de configuration.
    let utile = match texte.split_once(MARQUEUR) {
        Some((_, apres)) => apres,
        None => texte.as_ref(),
    };
    let separateur = if utile.contains('\0') { '\0' } else { '\n' };
    utile
        .split(separateur)
        .filter_map(|entree| entree.split_once('='))
        .filter(|(nom, _)| {
            !nom.is_empty()
                && nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !nom.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|(nom, valeur)| (nom.to_string(), valeur.to_string()))
        .collect()
}

/// Demande au shell de connexion, EN MODE INTERACTIF, tout ce qu'il exporte.
///
/// **`-lic` ET PAS `-lc`, ET CA A COUTE UNE RELEASE POUR RIEN.** zsh ne lit `~/.zshrc` que
/// pour un shell INTERACTIF ; un shell de connexion non interactif ne lit que `.zshenv`,
/// `.zprofile` et `.zlogin`. Or c'est dans `.zshrc` que la plupart des gens ecrivent leurs
/// `export`. Mesure du 2026-08-28 avec l'environnement reel de l'application : `-lc` rendait
/// 70 variables et AUCUNE des trois attendues, `-lic` en rendait 97 et les trois. Le meme
/// piege valait pour la recherche des CLI, qui ne trouvait pas ce que `.zshrc` ajoute au PATH.
#[cfg(unix)]
fn lire_env_du_shell() -> Vec<(String, String)> {
    let Some(shell) = std::env::var("SHELL").ok().filter(|s| !s.is_empty()) else {
        return vec![];
    };
    // `env -0` d'abord — les valeurs peuvent contenir un retour a la ligne, `COMPOSER_AUTH`
    // est du JSON — avec un repli sur `env` la ou `-0` n'existe pas.
    // `command` devant chacun : en mode INTERACTIF les alias et les fonctions du shell sont
    // actifs, et un alias sur `env` ou `printf` — ca existe — casserait la lecture.
    let ordre =
        format!("command printf %s '{MARQUEUR}'; command env -0 2>/dev/null || command env");
    let mut enfant = match std::process::Command::new(shell)
        .sans_console()
        .args(["-lic", &ordre])
        // Un shell interactif ne doit RIEN pouvoir attendre sur son entree.
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(enfant) => enfant,
        Err(_) => return vec![],
    };

    let debut = std::time::Instant::now();
    loop {
        match enfant.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if debut.elapsed() < DELAI_SHELL => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            // Depasse, ou impossible a interroger : on ne laisse pas un shell derriere nous.
            _ => {
                let _ = enfant.kill();
                let _ = enfant.wait();
                return vec![];
            }
        }
    }

    let mut sortie = Vec::new();
    if let Some(mut flux) = enfant.stdout.take() {
        use std::io::Read;
        let _ = flux.read_to_end(&mut sortie);
    }
    analyser_env(&sortie)
}

/// Windows n'a pas de shell de connexion qui exporte un environnement : celui du processus est
/// deja celui de la session.
#[cfg(windows)]
fn lire_env_du_shell() -> Vec<(String, String)> {
    vec![]
}

/// Ce qu'il faut AJOUTER a notre environnement pour ressembler a celui du shell.
///
/// **On ajoute, on ne remplace jamais.** Une variable que nous avons deja posee — `LD_PRELOAD`
/// pour le contournement de l'AppImage, `GTK_IM_MODULE` pour les accents, `FONTCONFIG_FILE` —
/// serait ecrasee par celle du shell, et deux contournements documentes tomberaient d'un coup.
/// Pur, donc verifiable sans lancer de shell.
fn a_ajouter<'a>(
    du_shell: &'a [(String, String)],
    deja_presente: impl Fn(&str) -> bool,
) -> Vec<(&'a str, &'a str)> {
    du_shell
        .iter()
        .filter(|(nom, _)| !JAMAIS_INJECTEES.contains(&nom.as_str()))
        .filter(|(nom, _)| !deja_presente(nom))
        .map(|(nom, valeur)| (nom.as_str(), valeur.as_str()))
        .collect()
}

fn env_du_shell() -> &'static [(String, String)] {
    ENV_DU_SHELL.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// Complete l'environnement d'un processus enfant avec ce que le shell de connexion definit et
/// que nous n'avons pas.
pub trait EnvDuShell {
    fn avec_env_du_shell(&mut self) -> &mut Self;
}

impl EnvDuShell for std::process::Command {
    fn avec_env_du_shell(&mut self) -> &mut Self {
        for (nom, valeur) in a_ajouter(env_du_shell(), |n| std::env::var_os(n).is_some()) {
            self.env(nom, valeur);
        }
        self
    }
}

impl EnvDuShell for tokio::process::Command {
    fn avec_env_du_shell(&mut self) -> &mut Self {
        for (nom, valeur) in a_ajouter(env_du_shell(), |n| std::env::var_os(n).is_some()) {
            self.env(nom, valeur);
        }
        self
    }
}

/// A appeler UNE fois au demarrage : remplit la liste en tache de fond.
///
/// Tant qu'elle n'est pas prete, `chemin_du_programme` se debrouille avec le PATH du processus
/// et les dossiers habituels — donc l'interface repond tout de suite, et se corrige d'elle-meme
/// une fraction de seconde plus tard.
pub fn precharger_les_chemins() {
    std::thread::spawn(|| {
        // L'environnement EN PREMIER : les chemins s'y lisent maintenant.
        if ENV_DU_SHELL.set(lire_env_du_shell()).is_err() {
            log::debug!("l'environnement du shell etait deja connu");
        }
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

    /// Le bruit d'un shell interactif ne doit pas se coller a la premiere variable : c'est le
    /// role du marqueur, et sans essai on ne saurait pas s'il sert encore.
    #[test]
    fn le_bruit_de_configuration_est_jete() {
        let sortie = format!(
            "Bienvenue !\nplugins charges\n{MARQUEUR}PATH=/bin\0HOME=/home/qui\0"
        );
        let lu = analyser_env(sortie.as_bytes());
        assert_eq!(lu.len(), 2, "lu : {lu:?}");
        assert_eq!(lu[0], ("PATH".to_string(), "/bin".to_string()));
    }

    /// Le repli pour un `env` qui ignore `-0` : on decoupe sur les lignes. Une entree qui ne
    /// ressemble pas a un nom de variable est jetee plutot que collee a la precedente.
    #[test]
    fn le_repli_sur_les_lignes_ne_ramasse_pas_n_importe_quoi() {
        let sortie = format!("{MARQUEUR}PATH=/bin\nceci n'est pas une variable\nHOME=/home/qui\n");
        let lu = analyser_env(sortie.as_bytes());
        assert_eq!(lu.len(), 2, "lu : {lu:?}");
        assert!(lu.iter().any(|(n, _)| n == "HOME"));
    }

    /// Une sortie sans marqueur reste lisible : un shell peut tres bien ne rien ecrire.
    #[test]
    fn une_sortie_sans_bruit_se_lit_aussi() {
        let lu = analyser_env(b"PATH=/bin\0HOME=/home/qui\0");
        assert_eq!(lu.len(), 2, "lu : {lu:?}");
    }

    /// Ce qui protege les contournements documentes : une variable que NOUS avons deja posee
    /// ne doit jamais etre remplacee par celle du shell. `LD_PRELOAD` (AppImage) et
    /// `GTK_IM_MODULE` (accents) tomberaient tous les deux, et ce sont huit iterations de
    /// diagnostic chacun.
    #[test]
    fn on_complete_l_environnement_sans_jamais_ecraser_le_notre() {
        let du_shell = vec![
            ("CCM_SPHINX_DIR".to_string(), "/srv/sphinx".to_string()),
            ("LD_PRELOAD".to_string(), "/usr/lib/celui-du-shell.so".to_string()),
            ("PWD".to_string(), "/home/qui-que-ce-soit".to_string()),
            ("PATH".to_string(), "/celui-du-shell".to_string()),
            ("SHLVL".to_string(), "3".to_string()),
        ];
        // On fait comme si LD_PRELOAD etait deja posee par nous, et pas les autres.
        let ajouts = a_ajouter(&du_shell, |nom| nom == "LD_PRELOAD");

        assert!(
            ajouts.contains(&("CCM_SPHINX_DIR", "/srv/sphinx")),
            "une variable du shell qui nous manque doit etre ajoutee : {ajouts:?}"
        );
        for interdit in ["LD_PRELOAD", "PWD", "PATH", "SHLVL"] {
            assert!(
                !ajouts.iter().any(|(nom, _)| *nom == interdit),
                "{interdit} n'a rien a faire dans les ajouts : {ajouts:?}"
            );
        }
    }

    /// Une valeur peut contenir un retour a la ligne — `COMPOSER_AUTH` est du JSON. C'est la
    /// raison du `env -0` : un decoupage sur les lignes couperait la valeur en deux.
    #[test]
    fn une_valeur_multiligne_reste_entiere() {
        let du_shell = vec![(
            "COMPOSER_AUTH".to_string(),
            "{\n  \"http-basic\": {}\n}".to_string(),
        )];
        let ajouts = a_ajouter(&du_shell, |_| false);
        assert_eq!(ajouts.len(), 1);
        assert!(ajouts[0].1.contains('\n'), "la valeur a ete tronquee : {:?}", ajouts[0]);
    }
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
