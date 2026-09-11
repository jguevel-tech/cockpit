//! Historique de commandes pour l'autosuggestion et la recherche Ctrl+R.
//! Fusionne l'historique enregistre par Cockpit (DB, timestamps precis) avec
//! ~/.zsh_history (format etendu) et ~/.bash_history.

use crate::storage::Database;
use serde::Serialize;
use std::collections::HashMap;

/// Nombre max de lignes lues en queue de chaque fichier d'historique shell.
const SHELL_HISTORY_TAIL: usize = 5000;

#[derive(Serialize, Clone)]
pub struct HistoryEntry {
    pub command: String,
    /// Epoch secondes de la derniere utilisation (None si inconnue).
    pub last_used: Option<i64>,
}

pub fn record(db: &Database, project: &str, command: &str) -> Result<(), String> {
    let cmd = command.trim();
    // Convention shell : espace initial = ne pas historiser ; on ignore le bruit
    if cmd.len() < 2 || command.starts_with(' ') {
        return Ok(());
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    db.conn()
        .execute(
            "INSERT INTO command_history (command, project, ts) VALUES (?1, ?2, ?3)
             ON CONFLICT(command) DO UPDATE SET ts=excluded.ts, project=excluded.project",
            rusqlite::params![cmd, project, ts],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn search(db: &Database, query: &str, limit: usize) -> Vec<HistoryEntry> {
    // command -> last_used ; la date la plus recente gagne
    let mut merged: HashMap<String, Option<i64>> = HashMap::new();

    for (cmd, ts) in read_shell_histories() {
        let slot = merged.entry(cmd).or_insert(None);
        if ts > *slot {
            *slot = ts;
        }
    }

    for (cmd, ts) in cockpit_entries(db) {
        let slot = merged.entry(cmd).or_insert(None);
        if Some(ts) > *slot {
            *slot = Some(ts);
        }
    }

    let needle = query.trim().to_lowercase();
    let mut results: Vec<(i64, Option<i64>, String)> = merged
        .into_iter()
        .filter_map(|(cmd, ts)| {
            if needle.is_empty() {
                return Some((0, ts, cmd));
            }
            // Sous-chaine insensible a la casse ; position basse = meilleur score
            cmd.to_lowercase().find(&needle).map(|pos| (pos as i64, ts, cmd))
        })
        .collect();

    // Tri : match le plus a gauche, puis le plus recent, puis le plus court
    results.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(b.1.unwrap_or(0).cmp(&a.1.unwrap_or(0)))
            .then(a.2.len().cmp(&b.2.len()))
    });
    if needle.is_empty() {
        // Sans requete : purement chronologique
        results.sort_by(|a, b| b.1.unwrap_or(0).cmp(&a.1.unwrap_or(0)));
    }

    results
        .into_iter()
        .take(limit)
        .map(|(_, ts, cmd)| HistoryEntry { command: cmd, last_used: ts })
        .collect()
}

fn cockpit_entries(db: &Database) -> Vec<(String, i64)> {
    let conn = db.conn();
    let Ok(mut stmt) = conn.prepare("SELECT command, ts FROM command_history ORDER BY ts DESC LIMIT 2000") else {
        return vec![];
    };
    let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))) else {
        return vec![];
    };
    rows.flatten().collect()
}

fn read_shell_histories() -> Vec<(String, Option<i64>)> {
    // Ces fichiers ne sont qu'un COMPLEMENT a l'historique tenu par Cockpit : leur absence
    // degrade la suggestion, elle ne fausse rien. On journalise quand meme, sinon une
    // recherche pauvre reste inexplicable.
    let home = match crate::chemins::dossier_personnel() {
        Ok(home) => home,
        Err(e) => {
            log::warn!("historique des shells ignore : {e}");
            return vec![];
        }
    };
    let mut out = Vec::new();

    if let Ok(content) = std::fs::read_to_string(home.join(".zsh_history")) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(SHELL_HISTORY_TAIL);
        for line in &lines[start..] {
            if let Some((cmd, ts)) = parse_zsh_line(line) {
                out.push((cmd, ts));
            }
        }
    }

    if let Ok(content) = std::fs::read_to_string(home.join(".bash_history")) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(SHELL_HISTORY_TAIL);
        for line in &lines[start..] {
            let cmd = line.trim();
            if cmd.len() >= 2 && !cmd.starts_with('#') {
                out.push((cmd.to_string(), None));
            }
        }
    }

    out
}

/// Format etendu zsh `: 1699999999:0;commande`, ou ligne brute.
fn parse_zsh_line(line: &str) -> Option<(String, Option<i64>)> {
    let line = line.trim_end();
    if let Some(rest) = line.strip_prefix(": ") {
        let (meta, cmd) = rest.split_once(';')?;
        let ts = meta.split(':').next()?.trim().parse::<i64>().ok();
        let cmd = cmd.trim();
        if cmd.len() < 2 {
            return None;
        }
        Some((cmd.to_string(), ts))
    } else {
        let cmd = line.trim();
        if cmd.len() < 2 {
            return None;
        }
        Some((cmd.to_string(), None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_zsh_line() {
        assert_eq!(
            parse_zsh_line(": 1783585603:0;npx tauri build"),
            Some(("npx tauri build".into(), Some(1783585603)))
        );
        assert_eq!(parse_zsh_line("plain command"), Some(("plain command".into(), None)));
        assert_eq!(parse_zsh_line(": bad"), None);
        assert_eq!(parse_zsh_line("x"), None);
    }

    #[test]
    fn test_record_and_search_db() {
        let db = Database::new(":memory:").unwrap();
        record(&db, "proj", "docker compose up -d").unwrap();
        record(&db, "proj", "docker compose logs -f api").unwrap();
        record(&db, "proj", " secret --token abc").unwrap(); // espace initial = ignore
        record(&db, "proj", "docker compose up -d").unwrap(); // upsert, pas de doublon

        let all = search(&db, "compose", 10);
        let cockpit: Vec<&str> = all
            .iter()
            .filter(|e| e.command.starts_with("docker compose"))
            .map(|e| e.command.as_str())
            .collect();
        assert!(cockpit.contains(&"docker compose up -d"));
        assert!(cockpit.contains(&"docker compose logs -f api"));
        assert!(!all.iter().any(|e| e.command.contains("secret")));
    }
}
