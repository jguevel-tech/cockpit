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
}
