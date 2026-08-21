//! Sessions Claude Code d'un projet, lues depuis ~/.claude/projects/<chemin-encode>/*.jsonl.
//! Chaque fichier .jsonl = une conversation, nommee par son session id (UUID).

use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

/// Nombre max de sessions retournees (les plus recentes).
const MAX_SESSIONS: usize = 15;
/// On ne lit que le debut du fichier pour trouver le premier message user.
const SCAN_BYTES: u64 = 256 * 1024;
const SCAN_LINES: usize = 300;

#[derive(Serialize, Clone)]
pub struct ClaudeSession {
    pub id: String,
    pub label: String,
    /// Epoch secondes de la derniere modification (= derniere activite).
    pub updated_at: i64,
    /// true si le label vient d'un renommage utilisateur (stocke en DB).
    pub renamed: bool,
}

/// Encodage utilise par Claude Code : tout caractere non alphanumerique -> '-'.
fn encode_project_path(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn claude_projects_dir(project_path: &str) -> Result<PathBuf, String> {
    Ok(crate::chemins::dossier_personnel()?
        .join(".claude/projects")
        .join(encode_project_path(project_path)))
}

/// Noms personnalises des sessions (table claude_session_names).
fn custom_names(db: &crate::storage::Database) -> std::collections::HashMap<String, String> {
    let conn = db.conn();
    let Ok(mut stmt) = conn.prepare("SELECT session_id, name FROM claude_session_names") else {
        return Default::default();
    };
    let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) else {
        return Default::default();
    };
    rows.flatten().collect()
}

pub fn rename_claude_session(
    db: &crate::storage::Database,
    session_id: &str,
    name: &str,
) -> Result<(), String> {
    let clean = name.trim();
    let conn = db.conn();
    if clean.is_empty() {
        // Nom vide = retour au label automatique
        conn.execute("DELETE FROM claude_session_names WHERE session_id=?1", [session_id])
            .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO claude_session_names (session_id, name) VALUES (?1, ?2)
             ON CONFLICT(session_id) DO UPDATE SET name=excluded.name",
            rusqlite::params![session_id, clean.chars().take(80).collect::<String>()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn list_claude_sessions(
    db: &crate::storage::Database,
    project_path: &str,
) -> Result<Vec<ClaudeSession>, String> {
    let names = custom_names(db);
    // Un dossier personnel introuvable remonte : sans ca, la liste sortait vide et on
    // cherchait la panne du cote de Claude Code.
    let dir = claude_projects_dir(project_path)?;
    // Le dossier absent, lui, n'est PAS une panne : ce projet n'a simplement encore aucune
    // conversation.
    if !dir.is_dir() {
        return Ok(vec![]);
    }

    let mut sessions: Vec<(i64, PathBuf, String)> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
            continue;
        }
        let Some(id) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        sessions.push((mtime, path, id));
    }

    // Les plus recentes d'abord, et on ne parse le label que pour celles retenues
    sessions.sort_by(|a, b| b.0.cmp(&a.0));
    sessions.truncate(MAX_SESSIONS);

    Ok(sessions
        .into_iter()
        .map(|(mtime, path, id)| match names.get(&id) {
            Some(custom) => ClaudeSession {
                label: custom.clone(),
                id,
                updated_at: mtime,
                renamed: true,
            },
            None => ClaudeSession {
                label: extract_label(&path).unwrap_or_else(|| "(conversation)".into()),
                id,
                updated_at: mtime,
                renamed: false,
            },
        })
        .collect())
}

/// Premier message utilisateur "humain" du fichier (hors sidechains et
/// contenus injectes par le harness), tronque pour servir de label.
fn extract_label(path: &std::path::Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file.take(SCAN_BYTES));

    for line in reader.lines().take(SCAN_LINES) {
        let Ok(line) = line else { break };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        if value.get("type").and_then(|t| t.as_str()) != Some("user") {
            continue;
        }
        if value.get("isSidechain").and_then(|b| b.as_bool()) == Some(true) {
            continue;
        }
        let Some(text) = user_text(&value) else { continue };
        let text = text.trim();
        if text.is_empty() || text.starts_with('<') || text.starts_with("Caveat:") {
            continue;
        }
        let label: String = text.chars().take(90).collect();
        return Some(if text.chars().count() > 90 { label + "…" } else { label });
    }
    None
}

fn user_text(value: &serde_json::Value) -> Option<String> {
    let content = value.get("message")?.get("content")?;
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    for part in content.as_array()? {
        if part.get("type").and_then(|t| t.as_str()) == Some("text") {
            return part.get("text").and_then(|t| t.as_str()).map(String::from);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_project_path() {
        assert_eq!(
            encode_project_path("/home/jguevel/Documents/workspace/core/cockpit"),
            "-home-jguevel-Documents-workspace-core-cockpit"
        );
        assert_eq!(encode_project_path("/a/b.c_d"), "-a-b-c-d");
    }

    #[test]
    fn test_rename_claude_session() {
        let db = crate::storage::Database::new(":memory:").unwrap();
        rename_claude_session(&db, "abc-123", "audit forum FR").unwrap();
        assert_eq!(custom_names(&db).get("abc-123").unwrap(), "audit forum FR");

        rename_claude_session(&db, "abc-123", "nouveau nom").unwrap();
        assert_eq!(custom_names(&db).get("abc-123").unwrap(), "nouveau nom");

        // Nom vide = retour au label automatique
        rename_claude_session(&db, "abc-123", "  ").unwrap();
        assert!(custom_names(&db).get("abc-123").is_none());
    }

    #[test]
    fn test_extract_label_from_jsonl() {
        let dir = std::env::temp_dir().join(format!("cockpit_claude_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("abc.jsonl");
        std::fs::write(&f, concat!(
            "{\"type\":\"mode\",\"mode\":\"x\"}\n",
            "{\"type\":\"user\",\"isSidechain\":true,\"message\":{\"content\":\"sidechain a ignorer\"}}\n",
            "{\"type\":\"user\",\"message\":{\"content\":\"<system-reminder>injecte</system-reminder>\"}}\n",
            "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"corrige le bug du login\"}]}}\n",
        )).unwrap();

        assert_eq!(extract_label(&f).unwrap(), "corrige le bug du login");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
