use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub path: String,
    pub name: String,
    pub compose_files: Vec<String>,
    pub has_dockerfile: bool,
}

pub fn scan(dir_path: &str) -> Result<ScanResult, String> {
    let path = Path::new(dir_path);
    if !path.is_dir() {
        return Err(format!("not a directory: {}", dir_path));
    }

    let abs_path = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path: {}", e))?;

    let name = abs_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut result = ScanResult {
        path: abs_path.to_string_lossy().to_string(),
        name,
        compose_files: vec![],
        has_dockerfile: false,
    };

    let entries = fs::read_dir(&abs_path).map_err(|e| format!("cannot read dir: {}", e))?;

    for entry in entries.flatten() {
        if entry.file_type().map_or(true, |t| t.is_dir()) {
            continue;
        }
        let fname = entry.file_name().to_string_lossy().to_string();
        let lower = fname.to_lowercase();

        // **UNE SEULE REGLE DECIDE de ce qu'est un fichier compose**, et elle vit dans
        // `docker::detection`. Ce fichier en portait une seconde, plus etroite : un projet
        // reconnu ici pouvait etre declare sans compose par l'onglet Docker, et inversement.
        // Deux listes pour une meme question, c'est la garantie qu'elles divergent.
        if crate::docker::detection::est_un_fichier_compose(&fname) {
            result.compose_files.push(fname.clone());
        }

        if lower == "dockerfile" || lower.starts_with("dockerfile.") {
            result.has_dockerfile = true;
        }
    }

    result.compose_files = crate::docker::detection::classer(result.compose_files);

    Ok(result)
}

pub fn scan_subdirs(parent_path: &str) -> Result<Vec<ScanResult>, String> {
    let abs_path = Path::new(parent_path)
        .canonicalize()
        .map_err(|e| format!("cannot resolve path: {}", e))?;

    let entries = fs::read_dir(&abs_path).map_err(|e| format!("cannot read dir: {}", e))?;

    let mut results = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map_or(false, |t| t.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let sub = abs_path.join(&name);
        if let Ok(result) = scan(&sub.to_string_lossy()) {
            if !result.compose_files.is_empty() || result.has_dockerfile {
                results.push(result);
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_with_compose() {
        let dir = std::env::temp_dir().join("cockpit_test_scan");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("docker-compose.yml"), "version: '3'").unwrap();
        fs::write(dir.join("Dockerfile"), "FROM alpine").unwrap();

        let result = scan(&dir.to_string_lossy()).unwrap();
        assert!(result.compose_files.contains(&"docker-compose.yml".to_string()));
        assert!(result.has_dockerfile);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_empty_dir() {
        let dir = std::env::temp_dir().join("cockpit_test_scan_empty");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let result = scan(&dir.to_string_lossy()).unwrap();
        assert!(result.compose_files.is_empty());
        assert!(!result.has_dockerfile);

        let _ = fs::remove_dir_all(&dir);
    }
}
