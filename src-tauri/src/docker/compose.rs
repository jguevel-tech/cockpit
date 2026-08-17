use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

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
    pub compose_file: String,
}

impl Compose {
    pub fn new(project_dir: &str, compose_file: &str) -> Self {
        Self {
            project_dir: PathBuf::from(project_dir),
            compose_file: compose_file.to_string(),
        }
    }

    pub fn has_compose_file(&self) -> bool {
        if !self.compose_file.is_empty() {
            return self.project_dir.join(&self.compose_file).exists();
        }
        for name in &["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
            if self.project_dir.join(name).exists() {
                return true;
            }
        }
        false
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
        if !self.compose_file.is_empty() {
            return Err(format!(
                "fichier compose introuvable : {} (renseigne dans les parametres du projet)",
                self.project_dir.join(&self.compose_file).display()
            ));
        }
        Err(format!(
            "aucun fichier compose dans {} — placez un docker-compose.yml dans le dossier, \
             ou indiquez son nom dans Parametres du projet",
            self.project_dir.display()
        ))
    }

    fn base_args(&self) -> Vec<String> {
        let mut args = vec!["compose".to_string()];
        if !self.compose_file.is_empty() {
            args.push("-f".to_string());
            args.push(self.compose_file.clone());
        }
        args
    }

    pub async fn up(&self) -> Result<(), String> {
        let mut args = self.base_args();
        args.extend(["up".into(), "-d".into()]);

        let output = tokio::time::timeout(COMMAND_TIMEOUT, async {
            Command::new("docker")
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

    pub async fn ps(&self) -> Result<Vec<ContainerStatus>, String> {
        let mut args = self.base_args();
        args.extend(["ps".into(), "--format".into(), "json".into()]);

        let output = tokio::time::timeout(PS_TIMEOUT, async {
            Command::new("docker")
                .args(&args)
                .current_dir(&self.project_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        })
        .await
        .map_err(|_| format!("docker compose ps timeout in {:?}", self.project_dir))?
        .map_err(|e| format!("docker compose ps failed in {:?}: {}", self.project_dir, e))?;

        // Un echec (docker.sock inaccessible, docker absent...) laissait stdout vide et
        // sortait Ok(vec![]) : le projet restait "stopped" sans explication. L'erreur doit
        // remonter pour etre affichee.
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("docker compose ps: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        // docker compose ps --format json can return JSON array or NDJSON
        if trimmed.starts_with('[') {
            let raw: Vec<serde_json::Value> =
                serde_json::from_str(trimmed).map_err(|e| format!("JSON parse error: {}", e))?;
            raw.iter().map(parse_container).collect()
        } else {
            trimmed
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|line| {
                    let val: serde_json::Value =
                        serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;
                    parse_container(&val)
                })
                .collect()
        }
    }
}

/// Repli quand aucun fichier compose n'est trouvable dans le dossier : `docker compose up`
/// etiquette chaque conteneur avec le dossier d'ou il a ete lance
/// (label com.docker.compose.project.working_dir). On retrouve donc les conteneurs du projet
/// meme si le fichier porte un nom non standard ou a ete passe en -f depuis ailleurs.
/// Seuls les conteneurs EN COURS sont listes (pas de -a) : meme semantique que compose ps,
/// c'est ce que le calcul d'etat attend.
pub async fn ps_by_working_dir(project_dir: &std::path::Path) -> Result<Vec<ContainerStatus>, String> {
    let filter = format!(
        "label=com.docker.compose.project.working_dir={}",
        project_dir.display()
    );
    let output = tokio::time::timeout(PS_TIMEOUT, async {
        Command::new("docker")
            .args(["ps", "--format", "json", "--filter", &filter])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
    })
    .await
    .map_err(|_| format!("docker ps timeout in {:?}", project_dir))?
    .map_err(|e| format!("docker ps failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("docker ps: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_docker_ps_line)
        .collect()
}

/// `docker ps --format json` sort du NDJSON avec d'autres cles que compose ps
/// (Names, State, Ports, Labels). Le nom de service est extrait du label compose.
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

fn parse_container(raw: &serde_json::Value) -> Result<ContainerStatus, String> {
    let get = |keys: &[&str]| -> String {
        for key in keys {
            if let Some(s) = raw.get(key).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
        String::new()
    };

    Ok(ContainerStatus {
        name: get(&["Name", "name"]),
        service: get(&["Service", "service"]),
        status: get(&["State", "state", "Status", "status"]),
        health: get(&["Health", "health"]),
        ports: get(&["Ports", "ports", "Publishers"]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_container_uppercase() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"Name":"myapp-web-1","Service":"web","State":"running","Health":"healthy","Ports":"0.0.0.0:8080->80/tcp"}"#,
        ).unwrap();
        let cs = parse_container(&json).unwrap();
        assert_eq!(cs.name, "myapp-web-1");
        assert_eq!(cs.service, "web");
        assert_eq!(cs.status, "running");
        assert_eq!(cs.health, "healthy");
    }

    #[test]
    fn test_parse_container_lowercase() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"name":"app-db-1","service":"db","state":"running","health":"","ports":""}"#,
        ).unwrap();
        let cs = parse_container(&json).unwrap();
        assert_eq!(cs.name, "app-db-1");
        assert_eq!(cs.status, "running");
    }

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

    #[test]
    fn require_compose_file_signale_un_nom_configure_absent() {
        let c = Compose::new("/nonexistent", "stack.yml");
        let err = c.require_compose_file().unwrap_err();
        assert!(err.contains("stack.yml"), "message sans le nom configure : {}", err);
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
