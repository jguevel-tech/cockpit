//! Trouver le fichier compose d'un projet, tout seul.
//!
//! **Pourquoi ce module existe.** Le nom du fichier se renseignait a la main dans les
//! parametres du projet, et deux listes DIFFERENTES decidaient par ailleurs de ce qu'etait un
//! fichier compose : celle du scanner (qui acceptait les suffixes `docker-compose.<x>.yml`) et
//! celle de l'onglet Docker (quatre noms exacts). Elles divergeaient en silence : un projet dont
//! le fichier s'appelle `docker-compose.local.yml` etait reconnu a la creation, puis introuvable
//! au moment de demarrer. D'ou un champ a remplir pour rattraper la machine.
//!
//! Ici, une seule regle, et personne ne saisit plus de chemin.
//!
//! **Le classement est separe de la lecture du disque** (`classer`), pour etre verifiable sans
//! creer un seul fichier.

use std::path::Path;

/// Les noms que docker reconnait LUI-MEME, dans SON ordre de preference.
///
/// Quand le fichier retenu est l'un de ceux-la, a la racine, on ne passe aucun `-f` : docker
/// applique alors ses propres regles, et notamment la fusion automatique de
/// `docker-compose.override.yml`. Passer `-f` la desactiverait sans que personne ne le demande.
pub const NOMS_CANONIQUES: &[&str] =
    &["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];

/// Suffixes qui designent un fichier de DEVELOPPEMENT. C'est ce qu'on veut demarrer depuis un
/// poste de travail.
const SUFFIXES_DEV: &[&str] = &["local", "dev", "development", "devbox", "docker-devbox"];

/// Suffixes a ne prendre qu'en DERNIER RECOURS.
///
/// `override` n'est pas un fichier a lui seul : docker le fusionne avec le fichier principal, le
/// choisir comme fichier unique donnerait une pile incomplete. Et demarrer par megarde une pile
/// de production depuis un poste de travail est le genre d'erreur qu'on ne repare pas.
const SUFFIXES_A_EVITER: &[&str] =
    &["override", "prod", "production", "staging", "preprod", "ci", "test", "tests", "e2e"];

/// Dossiers ou l'on ne cherche pas : ils sont gros, et un fichier compose d'exemple qui y
/// traine n'est pas celui du projet.
const DOSSIERS_IGNORES: &[&str] = &[
    ".git", "node_modules", "vendor", "target", "dist", "build", "coverage", ".venv", "venv",
    "__pycache__", ".next", ".nuxt", ".cache", "tmp", "bower_components", ".terraform",
];

/// Profondeur maximale de la recherche.
///
/// Trois niveaux couvrent les conventions repandues (`docker/compose.yml`,
/// `deploy/docker/compose.yml`) et refusent de parcourir un depot entier : la detection tourne a
/// chaque ouverture de l'onglet, elle doit rester gratuite.
const PROFONDEUR: usize = 3;

/// Est-ce un nom de fichier compose ?
///
/// Accepte les noms canoniques et leurs variantes suffixees, dans les deux familles
/// (`compose.<x>.yml` comme `docker-compose.<x>.yml`) — la seconde manquait, et c'est
/// exactement le cas qui obligeait a saisir un chemin.
pub fn est_un_fichier_compose(nom: &str) -> bool {
    let bas = nom.to_lowercase();
    if NOMS_CANONIQUES.contains(&bas.as_str()) {
        return true;
    }
    if !(bas.ends_with(".yml") || bas.ends_with(".yaml")) {
        return false;
    }
    bas.starts_with("compose.") || bas.starts_with("docker-compose.")
}

/// Le suffixe d'un nom suffixe : `docker-compose.local.yml` -> `local`. `None` pour un nom
/// canonique.
fn suffixe(nom: &str) -> Option<String> {
    let bas = nom.to_lowercase();
    if NOMS_CANONIQUES.contains(&bas.as_str()) {
        return None;
    }
    let sans_extension = bas.rsplit_once('.').map(|(debut, _)| debut)?;
    let reste = sans_extension
        .strip_prefix("docker-compose.")
        .or_else(|| sans_extension.strip_prefix("compose."))?;
    (!reste.is_empty()).then(|| reste.to_string())
}

/// Note un candidat : plus c'est bas, plus c'est probable. Le second nombre est la profondeur,
/// qui tranche entre deux candidats de meme nature.
fn note(chemin_relatif: &str) -> (u8, usize) {
    let profondeur = chemin_relatif.matches('/').count();
    let nom = chemin_relatif.rsplit('/').next().unwrap_or(chemin_relatif).to_lowercase();
    let a_la_racine = profondeur == 0;

    let suffixe = suffixe(&nom);
    if let Some(s) = &suffixe {
        if SUFFIXES_A_EVITER.iter().any(|mauvais| s == mauvais) {
            return (90, profondeur);
        }
    }

    match suffixe {
        // Un nom canonique : l'ordre est celui de docker.
        None => {
            let rang = NOMS_CANONIQUES.iter().position(|n| *n == nom).unwrap_or(9) as u8;
            if a_la_racine {
                (rang, profondeur)
            } else {
                (30 + rang, profondeur)
            }
        }
        Some(s) if SUFFIXES_DEV.iter().any(|dev| s == *dev) => {
            if a_la_racine {
                (10, profondeur)
            } else {
                (40, profondeur)
            }
        }
        Some(_) => {
            if a_la_racine {
                (20, profondeur)
            } else {
                (50, profondeur)
            }
        }
    }
}

/// Classe des chemins relatifs, le plus probable d'abord. Pur : aucun acces au disque.
pub fn classer(mut trouves: Vec<String>) -> Vec<String> {
    trouves.sort_by(|a, b| {
        let (na, pa) = note(a);
        let (nb, pb) = note(b);
        (na, pa, a.as_str()).cmp(&(nb, pb, b.as_str()))
    });
    trouves.dedup();
    trouves
}

/// Le resultat de la DESCENTE, et quand. Sans ce souvenir, un projet sans fichier compose a la
/// racine ferait repartir une descente a chaque interrogation de l'onglet et a chaque tour du
/// surveillant — c'est le seul cas ou la detection coute quelque chose, et c'est aussi le plus
/// frequent chez qui n'utilise pas les conteneurs.
///
/// **LA RACINE, ELLE, N'EST JAMAIS MEMORISEE.** Un fichier compose qu'on vient d'y creer doit
/// etre vu tout de suite, sans redemarrer l'application : c'est une promesse tenue par un essai
/// (`has_compose_suit_l_apparition_du_fichier`), et memoriser la racine l'a cassee. Quelques
/// interrogations du disque ne se mettent pas en cache.
static SOUVENIR: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, (i64, Vec<String>)>>,
> = std::sync::OnceLock::new();

/// Duree de validite du souvenir. Assez courte pour qu'un fichier compose qu'on vient de creer
/// apparaisse sans y penser, assez longue pour que la descente ne soit pas repetee en boucle.
const VALIDITE_MS: i64 = 10_000;

fn souvenir() -> &'static std::sync::Mutex<
    std::collections::HashMap<std::path::PathBuf, (i64, Vec<String>)>,
> {
    SOUVENIR.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn maintenant_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

/// Tous les fichiers compose du projet, le meilleur d'abord. Chemins relatifs, en `/`.
///
/// **La racine est regardee d'abord, et on ne parcourt rien si elle suffit.** C'est le cas de la
/// quasi-totalite des projets : quelques interrogations du disque, aucune descente.
pub fn candidats(racine: &Path) -> Vec<String> {
    // Toujours frais : quelques interrogations du disque, et la promesse « un fichier ajoute
    // apres coup est vu sans redemarrer » est tenue.
    let a_la_racine = a_la_racine(racine);
    if !a_la_racine.is_empty() {
        return classer(a_la_racine);
    }

    let maintenant = maintenant_ms();
    if let Ok(memoire) = souvenir().lock() {
        if let Some((quand, liste)) = memoire.get(racine) {
            if maintenant - *quand < VALIDITE_MS {
                return liste.clone();
            }
        }
    }

    let classes = classer(en_descendant(racine));
    if let Ok(mut memoire) = souvenir().lock() {
        memoire.insert(racine.to_path_buf(), (maintenant, classes.clone()));
    }
    classes
}

/// Oublie ce qu'on savait d'un projet : a appeler quand l'utilisateur demande explicitement de
/// regarder de nouveau, pour qu'un fichier tout juste cree apparaisse sans attendre.
pub fn oublier(racine: &Path) {
    if let Ok(mut memoire) = souvenir().lock() {
        memoire.remove(racine);
    }
}

/// Le fichier a utiliser, s'il y en a un.
pub fn choisir(racine: &Path) -> Option<String> {
    candidats(racine).into_iter().next()
}

fn a_la_racine(racine: &Path) -> Vec<String> {
    let Ok(entrees) = std::fs::read_dir(racine) else {
        return vec![];
    };
    entrees
        .flatten()
        .filter(|e| e.file_type().map(|t| !t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|nom| est_un_fichier_compose(nom))
        .collect()
}

fn en_descendant(racine: &Path) -> Vec<String> {
    let mut trouves = Vec::new();
    let marcheur = ignore::WalkBuilder::new(racine)
        .max_depth(Some(PROFONDEUR))
        // Les dossiers utiles sont parfois caches (`.docker`, `.devcontainer`) : on les veut,
        // et on ecarte le poids par la liste ci-dessous plutot que par le point du nom.
        .hidden(false)
        .git_ignore(true)
        .require_git(false)
        .filter_entry(|entree| {
            let nom = entree.file_name().to_string_lossy().to_lowercase();
            !DOSSIERS_IGNORES.contains(&nom.as_str())
        })
        .build();

    for entree in marcheur.flatten() {
        if entree.file_type().map(|t| t.is_dir()).unwrap_or(true) {
            continue;
        }
        let nom = entree.file_name().to_string_lossy().to_string();
        if !est_un_fichier_compose(&nom) {
            continue;
        }
        // Le chemin rendu au frontend et a docker se recolle par COMPOSANTS : un `replace` des
        // antislashs casserait un nom de fichier qui en contient sous Unix.
        if let Ok(relatif) = entree.path().strip_prefix(racine) {
            let morceaux: Vec<String> = relatif
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect();
            trouves.push(morceaux.join("/"));
        }
    }
    trouves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_deux_familles_de_noms_sont_reconnues() {
        for bon in [
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
            "docker-compose.local.yml",
            // Celui-la manquait dans l'ancienne detection : `compose.` suffixe.
            "compose.dev.yaml",
            "DOCKER-COMPOSE.YML",
        ] {
            assert!(est_un_fichier_compose(bon), "{bon} devrait etre reconnu");
        }
        for mauvais in ["compose.txt", "docker-compose", "monfichier.yml", "composer.json"] {
            assert!(!est_un_fichier_compose(mauvais), "{mauvais} ne devrait pas etre reconnu");
        }
    }

    #[test]
    fn le_nom_canonique_de_la_racine_passe_devant() {
        let classe = classer(vec![
            "docker/compose.yml".into(),
            "docker-compose.local.yml".into(),
            "docker-compose.yml".into(),
        ]);
        assert_eq!(classe[0], "docker-compose.yml");
    }

    #[test]
    fn l_ordre_de_docker_est_respecte_entre_noms_canoniques() {
        let classe = classer(vec![
            "docker-compose.yml".into(),
            "compose.yaml".into(),
            "docker-compose.yaml".into(),
            "compose.yml".into(),
        ]);
        assert_eq!(classe, NOMS_CANONIQUES.to_vec());
    }

    #[test]
    fn un_fichier_de_developpement_passe_devant_un_autre_suffixe() {
        let classe = classer(vec![
            "docker-compose.perso.yml".into(),
            "docker-compose.local.yml".into(),
        ]);
        assert_eq!(classe[0], "docker-compose.local.yml");
    }

    /// `override` seul donne une pile incomplete, et une pile de production demarree par megarde
    /// ne se repare pas : les deux passent DERNIERE, jamais en premier.
    #[test]
    fn override_et_production_ne_sont_jamais_choisis_les_premiers() {
        let classe = classer(vec![
            "docker-compose.override.yml".into(),
            "docker-compose.prod.yml".into(),
            "docker/compose.yml".into(),
        ]);
        assert_eq!(classe[0], "docker/compose.yml");

        // Et s'il n'y a QUE ceux-la, on les propose quand meme : mieux qu'aucun choix.
        let seuls = classer(vec![
            "docker-compose.prod.yml".into(),
            "docker-compose.override.yml".into(),
        ]);
        assert_eq!(seuls.len(), 2);
    }

    #[test]
    fn la_racine_est_regardee_avant_de_descendre() {
        let dossier = std::env::temp_dir().join(format!("cockpit-detect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(dossier.join("docker")).unwrap();
        std::fs::write(dossier.join("docker").join("compose.yml"), "services: {}").unwrap();

        // Rien a la racine : on descend et on trouve.
        assert_eq!(choisir(&dossier).as_deref(), Some("docker/compose.yml"));

        // Un fichier ajoute A LA RACINE est vu TOUT DE SUITE : la racine n'est jamais
        // memorisee, et c'est deliberе — un fichier compose qu'on vient de creer ne doit pas
        // attendre.
        std::fs::write(dossier.join("docker-compose.local.yml"), "services: {}").unwrap();
        assert_eq!(choisir(&dossier).as_deref(), Some("docker-compose.local.yml"));

        let _ = std::fs::remove_dir_all(&dossier);
    }

    /// Le souvenir ne porte QUE sur la descente : sans cet essai, on ne saurait pas s'il
    /// existe encore.
    #[test]
    fn la_descente_est_memorisee_et_s_oublie_sur_demande() {
        let dossier = std::env::temp_dir().join(format!("cockpit-souvenir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(dossier.join("docker")).unwrap();
        assert_eq!(choisir(&dossier), None);

        // Un fichier apparait EN PROFONDEUR : le souvenir tient encore, on ne le voit pas.
        std::fs::write(dossier.join("docker").join("compose.yml"), "services: {}").unwrap();
        assert_eq!(choisir(&dossier), None, "la descente doit etre memorisee");

        // Un geste explicite de l'utilisateur (bouton « chercher de nouveau ») l'oublie.
        oublier(&dossier);
        assert_eq!(choisir(&dossier).as_deref(), Some("docker/compose.yml"));

        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn un_projet_sans_compose_ne_rend_rien() {
        let dossier = std::env::temp_dir().join(format!("cockpit-vide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(&dossier).unwrap();
        std::fs::write(dossier.join("README.md"), "rien ici").unwrap();
        assert_eq!(choisir(&dossier), None);
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
