//! Liste globale des conteneurs Docker de la machine (via `docker ps`),
//! independamment des projets Compose connus de Cockpit, + actions de base.

use serde::Serialize;
use std::time::Duration;
use tokio::process::Command;

use crate::commande::{EnvDuShell, SansConsole};

const TIMEOUT: Duration = Duration::from_secs(15);
/// Operations longues : `system df` mesure chaque volume/image (10s+ avec
/// beaucoup de volumes), prune et stop en masse peuvent durer plusieurs minutes.
const TIMEOUT_LONG: Duration = Duration::from_secs(300);

#[derive(Serialize, Clone)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,  // running, exited, paused, created...
    pub status: String, // "Up 3 hours", "Exited (0) 2 days ago"...
    pub ports: String,
    /// Projet Compose (label com.docker.compose.project), vide si autonome.
    pub project: String,
}

async fn run_docker(args: &[&str]) -> Result<String, String> {
    run_docker_timeout(args, TIMEOUT).await
}

async fn run_docker_timeout(args: &[&str], timeout: Duration) -> Result<String, String> {
    let output = tokio::time::timeout(
        timeout,
        Command::new("docker")
            .sans_console()
            .avec_env_du_shell()
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "docker: delai depasse".to_string())?
    .map_err(|e| format!("docker introuvable ou erreur: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().chars().take(300).collect());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Liste tous les conteneurs (`docker ps -a`), tries : running d'abord.
pub async fn list_all() -> Result<Vec<DockerContainer>, String> {
    // Format JSON ligne par ligne (une par conteneur)
    let raw = run_docker(&[
        "ps", "-a", "--no-trunc",
        "--format", "{{json .}}",
    ])
    .await?;

    let mut containers: Vec<DockerContainer> = raw
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|v| {
            let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            DockerContainer {
                id: get("ID"),
                name: get("Names"),
                image: get("Image"),
                state: get("State"),
                status: get("Status"),
                ports: get("Ports"),
                project: get("Label.com.docker.compose.project"),
            }
        })
        .collect();

    // Fallback pour le label projet : docker ps json expose Labels en bloc,
    // pas toujours Label.xxx selon la version -> re-parse depuis Labels si vide.
    for c in containers.iter_mut() {
        if c.project.is_empty() {
            c.project = extract_compose_project(&raw, &c.id);
        }
    }

    // Running d'abord, puis par nom
    containers.sort_by(|a, b| {
        let ra = a.state == "running";
        let rb = b.state == "running";
        rb.cmp(&ra).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(containers)
}

/// Cherche le label compose.project dans le champ Labels brut d'un conteneur.
fn extract_compose_project(raw: &str, id: &str) -> String {
    for line in raw.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if v.get("ID").and_then(|x| x.as_str()) != Some(id) {
            continue;
        }
        let labels = v.get("Labels").and_then(|x| x.as_str()).unwrap_or("");
        for kv in labels.split(',') {
            if let Some(p) = kv.strip_prefix("com.docker.compose.project=") {
                return p.to_string();
            }
        }
    }
    String::new()
}

pub async fn container_action(id: &str, action: &str) -> Result<(), String> {
    container_action_bulk(std::slice::from_ref(&id.to_string()), action).await
}

/// Dernieres lignes de logs d'un conteneur.
///
/// `docker logs` restitue le stdout ET le stderr du conteneur sur les DEUX flux du CLI :
/// ne lire que stdout perdrait la moitie des logs (la plupart des serveurs loggent sur
/// stderr). On demande les timestamps RFC3339 (tri lexicographique = tri chronologique),
/// on fusionne les deux flux par tri, puis on retire le prefixe pour l'affichage.
pub async fn container_logs(id: &str, tail: u32) -> Result<String, String> {
    let tail_s = tail.to_string();
    let output = tokio::time::timeout(
        TIMEOUT,
        Command::new("docker")
            .sans_console()
            .avec_env_du_shell()
            .args(["logs", "--tail", &tail_s, "--timestamps", id])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "docker logs: delai depasse".to_string())?
    .map_err(|e| format!("docker introuvable ou erreur: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().chars().take(300).collect());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let mut lines: Vec<&str> = stdout.lines().chain(stderr.lines()).collect();
    lines.sort_unstable();
    Ok(lines.iter().map(|l| strip_log_timestamp(l)).collect::<Vec<_>>().join("\n"))
}

/// Retire le prefixe "2026-08-14T10:00:00.123456789Z " pose par --timestamps.
/// Une ligne sans prefixe (continuation, log exotique) est rendue telle quelle.
fn strip_log_timestamp(line: &str) -> &str {
    let Some((ts, rest)) = line.split_once(' ') else { return line };
    let looks_rfc3339 = ts.len() >= 20
        && ts.ends_with('Z')
        && ts.as_bytes().first().is_some_and(|b| b.is_ascii_digit())
        && ts.contains('T');
    if looks_rfc3339 { rest } else { line }
}

/// Applique une action a un lot de conteneurs (docker accepte plusieurs ids).
pub async fn container_action_bulk(ids: &[String], action: &str) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let verb = match action {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        "remove" => "rm",
        _ => return Err(format!("action inconnue: {}", action)),
    };
    let mut args: Vec<&str> = Vec::with_capacity(ids.len() + 2);
    args.push(verb);
    if verb == "rm" {
        args.push("-f"); // force : retire meme en cours d'execution
    }
    args.extend(ids.iter().map(|s| s.as_str()));
    run_docker_timeout(&args, TIMEOUT_LONG).await.map(|_| ())
}

// --- Volumes, images, espace disque, prune ---

#[derive(Serialize, Clone)]
pub struct DiskUsage {
    /// "Images", "Containers", "Local Volumes", "Build Cache"
    pub kind: String,
    pub total: String,
    pub active: String,
    pub size: String,
    pub reclaimable: String,
}

#[derive(Serialize, Clone)]
pub struct DockerVolume {
    pub name: String,
    pub driver: String,
    pub dangling: bool,
}

#[derive(Serialize, Clone)]
pub struct DockerImage {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub dangling: bool,
}

/// Resume `docker system df` : ce qui prend de la place et le recuperable.
pub async fn disk_usage() -> Result<Vec<DiskUsage>, String> {
    let raw = run_docker_timeout(&["system", "df", "--format", "{{json .}}"], TIMEOUT_LONG).await?;
    Ok(raw
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            DiskUsage {
                kind: g("Type"),
                total: g("TotalCount"),
                active: g("Active"),
                size: g("Size"),
                reclaimable: g("Reclaimable"),
            }
        })
        .collect())
}

pub async fn list_volumes() -> Result<Vec<DockerVolume>, String> {
    let dangling: std::collections::HashSet<String> =
        run_docker(&["volume", "ls", "-f", "dangling=true", "-q"])
            .await
            .unwrap_or_default()
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

    let raw = run_docker(&["volume", "ls", "--format", "{{json .}}"]).await?;
    let mut vols: Vec<DockerVolume> = raw
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let name = g("Name");
            DockerVolume {
                dangling: dangling.contains(&name),
                name,
                driver: g("Driver"),
            }
        })
        .collect();
    // Non utilises (dangling) en premier
    vols.sort_by(|a, b| b.dangling.cmp(&a.dangling).then(a.name.cmp(&b.name)));
    Ok(vols)
}

pub async fn list_images() -> Result<Vec<DockerImage>, String> {
    let raw = run_docker(&["images", "--format", "{{json .}}"]).await?;
    let mut imgs: Vec<DockerImage> = raw
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let repo = g("Repository");
            let tag = g("Tag");
            DockerImage {
                id: g("ID"),
                dangling: repo == "<none>" || tag == "<none>",
                repository: repo,
                tag,
                size: g("Size"),
            }
        })
        .collect();
    imgs.sort_by(|a, b| b.dangling.cmp(&a.dangling).then(a.repository.cmp(&b.repository)));
    Ok(imgs)
}

pub async fn remove_volume(name: &str) -> Result<(), String> {
    run_docker(&["volume", "rm", name]).await.map(|_| ())
}

pub async fn remove_image(id: &str) -> Result<(), String> {
    run_docker(&["rmi", id]).await.map(|_| ())
}

/// Nettoyage : conteneurs arretes, images (dangling ou toutes inutilisees),
/// volumes non utilises, cache de build. Retourne le message de docker.
pub async fn prune(target: &str) -> Result<String, String> {
    let args: Vec<&str> = match target {
        "containers" => vec!["container", "prune", "-f"],
        "images" => vec!["image", "prune", "-f"],       // dangling seulement
        "images_all" => vec!["image", "prune", "-a", "-f"], // toutes les inutilisees
        "volumes" => vec!["volume", "prune", "-f"],
        "builder" => vec!["builder", "prune", "-f"],
        _ => return Err(format!("cible de prune inconnue: {}", target)),
    };
    run_docker_timeout(&args, TIMEOUT_LONG).await
}

#[cfg(test)]
mod tests {
    use super::strip_log_timestamp;

    #[test]
    fn strip_log_timestamp_retire_le_prefixe_rfc3339() {
        assert_eq!(
            strip_log_timestamp("2026-08-14T10:00:00.123456789Z GET /health 200"),
            "GET /health 200"
        );
        // Ligne sans prefixe : inchangee
        assert_eq!(strip_log_timestamp("stack trace line"), "stack trace line");
        // Un premier mot quelconque n'est pas confondu avec un timestamp
        assert_eq!(strip_log_timestamp("ERROR something failed"), "ERROR something failed");
        assert_eq!(strip_log_timestamp(""), "");
    }
}
