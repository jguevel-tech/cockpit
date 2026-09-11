use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

use crate::commande::{EnvDuShell, SansConsole};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const PS_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    pub name: String,
    pub service: String,
    pub status: String,
    pub health: String,
    pub ports: String,
}

#[derive(Debug, Clone)]
pub struct Compose {
    pub project_dir: PathBuf,
    /// Ce que l'utilisateur a retenu PARMI les fichiers detectes, ou une chaine vide. Ce n'est
    /// pas le fichier utilise : celui-la se demande a `fichier()`, qui regarde le disque.
    pub choix: String,
}

impl Compose {
    /// `choix` est ce que l'utilisateur a retenu PARMI les fichiers detectes, ou une chaine
    /// vide. Personne ne saisit plus de chemin : la detection decide (`super::detection`).
    ///
    /// **Un choix qui ne designe plus rien est ignore, pas honore.** Un fichier renomme ou
    /// supprime rendait sinon l'onglet inutilisable, avec un message qui renvoyait vers un
    /// champ de parametres — champ qui n'existe plus.
    pub fn new(project_dir: &str, choix: &str) -> Self {
        Self { project_dir: PathBuf::from(project_dir), choix: choix.to_string() }
    }

    /// Le fichier a utiliser, DEMANDE AU DISQUE a chaque fois.
    ///
    /// **Ce n'est pas resolu une fois pour toutes, et c'est deliberе.** Un objet `Compose` vit
    /// aussi longtemps que l'orchestrateur : figer la reponse a la construction faisait qu'un
    /// fichier compose cree apres le demarrage de l'application n'etait JAMAIS vu. Un essai le
    /// tient (`has_compose_suit_l_apparition_du_fichier`), et il a rattrape exactement ca.
    /// Le cout reste nul dans le cas courant : la detection regarde la racine, et ne memorise
    /// que la descente.
    fn fichier(&self) -> Option<String> {
        Some(self.choix.as_str())
            .filter(|c| !c.is_empty() && self.project_dir.join(c).is_file())
            .map(|c| c.to_string())
            .or_else(|| super::detection::choisir(&self.project_dir))
    }

    pub fn has_compose_file(&self) -> bool {
        self.fichier().is_some()
    }

    /// Garde a passer AVANT toute commande compose qui AGIT (up/down). Sans elle, docker
    /// repond "no configuration file provided: not found" — un message qui ne dit ni ou il a
    /// cherche, ni quoi faire (constate chez un utilisateur externe le 2026-08-17 : bouton
    /// Start d'un projet sans fichier compose). Le fichier compose est optionnel dans Cockpit,
    /// donc ce cas est NORMAL et doit s'expliquer, pas remonter une erreur de docker.
    pub fn require_compose_file(&self) -> Result<(), String> {
        if self.has_compose_file() {
            return Ok(());
        }
        Err(format!(
            "aucun fichier compose trouve dans {} — Cockpit cherche les noms de docker \
             (compose.yml, docker-compose.yml), leurs variantes suffixees \
             (docker-compose.local.yml) et les sous-dossiers habituels sur trois niveaux",
            self.project_dir.display()
        ))
    }

    fn base_args(&self) -> Vec<String> {
        let mut args = vec!["compose".to_string()];
        // **Pas de `-f` sur un nom que docker connait deja.** Le passer desactive ses regles a
        // lui, dont la fusion automatique de `docker-compose.override.yml` : une pile qui
        // marchait en ligne de commande arriverait ici incomplete, sans que rien ne le dise.
        if let Some(fichier) = self.fichier() {
            let canonique =
                super::detection::NOMS_CANONIQUES.contains(&fichier.to_lowercase().as_str());
            if !canonique {
                args.push("-f".to_string());
                args.push(fichier);
            }
        }
        args
    }

    pub async fn up(&self) -> Result<(), String> {
        let mut args = self.base_args();
        args.extend(["up".into(), "-d".into()]);

        let output = tokio::time::timeout(COMMAND_TIMEOUT, async {
            Command::new("docker")
                .sans_console()
                .avec_env_du_shell()
                .args(&args)
                .current_dir(&self.project_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        })
        .await
        .map_err(|_| format!("docker compose up timeout in {:?}", self.project_dir))?
        .map_err(|e| format!("docker compose up failed in {:?}: {}", self.project_dir, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker compose up failed in {:?}:\n{}", self.project_dir, stderr));
        }
        Ok(())
    }

    pub async fn down(&self) -> Result<(), String> {
        let mut args = self.base_args();
        args.push("down".into());

        let output = tokio::time::timeout(COMMAND_TIMEOUT, async {
            Command::new("docker")
                .sans_console()
                .avec_env_du_shell()
                .args(&args)
                .current_dir(&self.project_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        })
        .await
        .map_err(|_| format!("docker compose down timeout in {:?}", self.project_dir))?
        .map_err(|e| format!("docker compose down failed in {:?}: {}", self.project_dir, e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker compose down failed in {:?}:\n{}", self.project_dir, stderr));
        }
        Ok(())
    }

}

/// `docker ps --format json` sort du NDJSON avec d'autres cles que compose ps
/// (Names, State, Ports, Labels). Le nom de service est extrait du label compose.
/// L'etiquette que docker compose pose sur chaque conteneur : le dossier depuis lequel la pile
/// a ete lancee.
const ETIQUETTE_DOSSIER: &str = "com.docker.compose.project.working_dir=";

/// Le dossier de travail lu dans les etiquettes d'un conteneur. Pur.
fn dossier_de_travail(etiquettes: &str) -> Option<String> {
    etiquettes
        .split(',')
        .find_map(|kv| kv.strip_prefix(ETIQUETTE_DOSSIER))
        .map(|d| d.to_string())
        .filter(|d| !d.is_empty())
}

/// Range les lignes de `docker ps` par dossier de travail. Pur, donc verifiable sans docker.
fn ranger_par_dossier(sortie: &str) -> HashMap<String, Vec<ContainerStatus>> {
    let mut par_dossier: HashMap<String, Vec<ContainerStatus>> = HashMap::new();
    for ligne in sortie.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(ligne) else { continue };
        let etiquettes = val.get("Labels").and_then(|v| v.as_str()).unwrap_or("");
        let Some(dossier) = dossier_de_travail(etiquettes) else { continue };
        if let Ok(conteneur) = parse_docker_ps_line(ligne) {
            par_dossier.entry(dossier).or_default().push(conteneur);
        }
    }
    par_dossier
}

/// Tous les conteneurs compose de la machine, en **UN SEUL** appel a docker, ranges par dossier.
///
/// **POURQUOI UN SEUL APPEL.** La surveillance interrogeait docker UNE FOIS PAR PROJET, l'un
/// apres l'autre, toutes les cinq secondes. Mesure du 2026-08-31 sur une installation de 32
/// projets : chaque appel coute 100 a 400 ms, soit plus de six secondes par passage — le passage
/// n'avait jamais fini avant le suivant, docker tournait a 200 % de processeur en permanence, et
/// tout le poste ralentissait. Les conteneurs portent le dossier de leur pile en etiquette : une
/// seule question suffit a savoir qui tourne ou.
pub async fn ps_de_tous_les_projets() -> Result<HashMap<String, Vec<ContainerStatus>>, String> {
    let output = tokio::time::timeout(PS_TIMEOUT, async {
        Command::new("docker")
            .sans_console()
            .avec_env_du_shell()
            .args(["ps", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    })
    .await
    .map_err(|_| "docker ps timeout".to_string())?
    .map_err(|e| format!("docker ps failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("docker ps: {}", stderr.trim()));
    }

    Ok(ranger_par_dossier(&String::from_utf8_lossy(&output.stdout)))
}

/// Les conteneurs d'un projet donne, parmi ce que le seul appel a rapporte.
///
/// Un dossier de travail EGAL au chemin du projet, ou SITUE DEDANS : le fichier compose peut
/// vivre dans un sous-dossier (`docker/compose.yml`), et docker pose alors ce sous-dossier.
pub fn conteneurs_du_projet(
    par_dossier: &HashMap<String, Vec<ContainerStatus>>,
    chemin_du_projet: &std::path::Path,
) -> Vec<ContainerStatus> {
    let racine = chemin_du_projet.to_string_lossy().to_string();
    let mut trouves = Vec::new();
    for (dossier, conteneurs) in par_dossier {
        let dedans = dossier == &racine
            || dossier.strip_prefix(&racine).is_some_and(|reste| reste.starts_with('/'));
        if dedans {
            trouves.extend(conteneurs.iter().cloned());
        }
    }
    trouves
}

fn parse_docker_ps_line(line: &str) -> Result<ContainerStatus, String> {
    let val: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;
    let get = |key: &str| val.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let labels = get("Labels");
    let service = labels
        .split(',')
        .find_map(|kv| kv.strip_prefix("com.docker.compose.service="))
        .unwrap_or("")
        .to_string();
    Ok(ContainerStatus {
        name: get("Names"),
        service,
        status: get("State"),
        health: String::new(),
        ports: get("Ports"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_compose_file() {
        let c = Compose::new("/nonexistent", "");
        assert!(!c.has_compose_file());
    }

    #[test]
    fn require_compose_file_explique_l_absence() {
        let c = Compose::new("/nonexistent", "");
        let err = c.require_compose_file().unwrap_err();
        // Le message doit nommer le dossier fouille et dire quoi faire : c'est ce qui
        // manquait au "no configuration file provided: not found" de docker.
        assert!(err.contains("/nonexistent"), "message sans le chemin : {}", err);
        assert!(err.contains("docker-compose.yml"), "message sans remede : {}", err);
    }

    /// Un choix qui ne designe plus rien ne doit pas condamner l'onglet : la detection reprend
    /// la main. Avant, le message renvoyait vers un champ des parametres — champ supprime, donc
    /// un conseil impossible a suivre.
    #[test]
    fn un_choix_devenu_faux_est_ignore_et_la_detection_reprend() {
        let dir = std::env::temp_dir().join(format!("cockpit_choix_faux_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("docker-compose.yml"), "services: {}\n").unwrap();

        let c = Compose::new(dir.to_str().unwrap(), "stack-qui-nexiste-plus.yml");
        assert!(c.require_compose_file().is_ok());
        assert_eq!(c.fichier().as_deref(), Some("docker-compose.yml"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn require_compose_file_accepte_un_fichier_standard() {
        let dir = std::env::temp_dir().join(format!("cockpit_compose_req_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("docker-compose.yml"), "services: {}\n").unwrap();
        let c = Compose::new(dir.to_str().unwrap(), "");
        assert!(c.require_compose_file().is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// L'appel unique doit ranger chaque conteneur sous le dossier de sa pile, et ignorer ce
    /// qui n'a pas cette etiquette — un conteneur lance a la main n'appartient a aucun projet.
    #[test]
    fn les_conteneurs_se_rangent_par_dossier_de_travail() {
        let sortie = concat!(
            r#"{"Names":"web-1","State":"running","Ports":"80/tcp","Labels":"com.docker.compose.project=demo,com.docker.compose.service=web,com.docker.compose.project.working_dir=/srv/demo"}"#, "\n",
            r#"{"Names":"db-1","State":"running","Ports":"","Labels":"com.docker.compose.service=db,com.docker.compose.project.working_dir=/srv/demo"}"#, "\n",
            r#"{"Names":"seul","State":"running","Ports":"","Labels":"autre=chose"}"#, "\n",
            r#"{"Names":"api-1","State":"running","Ports":"","Labels":"com.docker.compose.project.working_dir=/srv/autre/docker"}"#, "\n",
        );
        let range = ranger_par_dossier(sortie);
        assert_eq!(range.len(), 2, "deux dossiers attendus : {range:?}");
        assert_eq!(range["/srv/demo"].len(), 2);
        assert_eq!(range["/srv/autre/docker"].len(), 1);
        assert!(
            !range.values().flatten().any(|c| c.name == "seul"),
            "un conteneur sans etiquette de dossier n'appartient a aucun projet"
        );
    }

    /// Le fichier compose peut vivre dans un sous-dossier : docker pose alors CE sous-dossier
    /// en etiquette, et le projet doit quand meme reconnaitre ses conteneurs.
    #[test]
    fn un_projet_reconnait_les_conteneurs_de_ses_sous_dossiers() {
        let mut par_dossier: HashMap<String, Vec<ContainerStatus>> = HashMap::new();
        let conteneur = |nom: &str| ContainerStatus {
            name: nom.to_string(),
            service: String::new(),
            status: "running".to_string(),
            health: String::new(),
            ports: String::new(),
        };
        par_dossier.insert("/srv/demo".into(), vec![conteneur("racine")]);
        par_dossier.insert("/srv/demo/docker".into(), vec![conteneur("sous-dossier")]);
        // Un voisin dont le nom COMMENCE par le meme texte ne doit pas etre pris : c'est un
        // autre projet, pas un sous-dossier.
        par_dossier.insert("/srv/demo-autre".into(), vec![conteneur("voisin")]);

        let a_nous = conteneurs_du_projet(&par_dossier, std::path::Path::new("/srv/demo"));
        let noms: std::collections::HashSet<&str> = a_nous.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(noms.len(), 2, "attendu racine + sous-dossier, recu {noms:?}");
        assert!(noms.contains("racine") && noms.contains("sous-dossier"));
        assert!(!noms.contains("voisin"), "/srv/demo-autre n'est pas dans /srv/demo");
    }

    #[test]
    fn test_parse_docker_ps_line() {
        let line = r#"{"Names":"web-1","State":"running","Ports":"80/tcp","Labels":"com.docker.compose.project=demo,com.docker.compose.service=web,com.docker.compose.project.working_dir=/srv/demo"}"#;
        let cs = parse_docker_ps_line(line).unwrap();
        assert_eq!(cs.name, "web-1");
        assert_eq!(cs.service, "web");
        assert_eq!(cs.status, "running");
        assert_eq!(cs.ports, "80/tcp");
    }

    #[test]
    fn test_parse_docker_ps_line_sans_label_service() {
        let line = r#"{"Names":"solo","State":"running","Ports":"","Labels":""}"#;
        let cs = parse_docker_ps_line(line).unwrap();
        assert_eq!(cs.name, "solo");
        assert_eq!(cs.service, "");
    }
}
