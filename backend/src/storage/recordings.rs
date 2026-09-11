use super::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: i64,
    pub project: String,
    pub started_at: String,
    pub duration_secs: i64,
    pub state: String,
    pub error: Option<String>,
    pub dir: String,
}

impl Recording {
    const SELECT_COLS: &'static str = "id, project, started_at, duration_secs, state, error, dir";

    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            started_at: row.get(2)?,
            duration_secs: row.get(3)?,
            state: row.get(4)?,
            error: row.get(5)?,
            dir: row.get(6)?,
        })
    }
}

impl Database {
    pub fn create_recording(&self, project: &str, started_at: &str) -> Result<Recording, String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO recordings (project, started_at, state, dir) VALUES (?1, ?2, 'recording', '')",
            rusqlite::params![project, started_at],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        conn.query_row(
            &format!("SELECT {} FROM recordings WHERE id=?1", Recording::SELECT_COLS),
            [id],
            Recording::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn get_recording(&self, id: i64) -> Result<Recording, String> {
        let conn = self.conn();
        conn.query_row(
            &format!("SELECT {} FROM recordings WHERE id=?1", Recording::SELECT_COLS),
            [id],
            Recording::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn set_recording_dir(&self, id: i64, dir: &str) -> Result<(), String> {
        self.conn()
            .execute("UPDATE recordings SET dir=?1 WHERE id=?2", rusqlite::params![dir, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_recording_duration(&self, id: i64, secs: i64) -> Result<(), String> {
        self.conn()
            .execute("UPDATE recordings SET duration_secs=?1 WHERE id=?2", rusqlite::params![secs, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_recording_state(&self, id: i64, state: &str, error: Option<&str>) -> Result<(), String> {
        self.conn()
            .execute(
                "UPDATE recordings SET state=?1, error=?2 WHERE id=?3",
                rusqlite::params![state, error, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_failed_recordings(&self, project: &str) -> Result<Vec<Recording>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM recordings WHERE project=?1 AND state='error' ORDER BY started_at DESC",
                Recording::SELECT_COLS
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([project], Recording::from_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn delete_recording(&self, id: i64) -> Result<(), String> {
        self.conn()
            .execute("DELETE FROM recordings WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Au demarrage : les enregistrements restes dans un etat intermediaire
    /// (app fermee en plein pipeline) passent en erreur.
    pub fn fail_stale_recordings(&self) -> Result<(), String> {
        self.conn()
            .execute(
                "UPDATE recordings SET state='error', error='Interrompu (application fermee)'
                 WHERE state NOT IN ('done', 'error')",
                [],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_project_summary_prompt(&self, project: &str) -> Result<Option<String>, String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT summary_prompt FROM projects WHERE name=?1",
            [project],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|e| e.to_string())
    }

    pub fn set_project_summary_prompt(&self, project: &str, prompt: Option<&str>) -> Result<(), String> {
        // Chaine vide = pas d'override
        let value = prompt.map(|p| p.trim()).filter(|p| !p.is_empty());
        self.conn()
            .execute(
                "UPDATE projects SET summary_prompt=?1 WHERE name=?2",
                rusqlite::params![value, project],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_recording_lifecycle() {
        let db = Database::new(":memory:").unwrap();
        let rec = db.create_recording("proj", "2026-07-08 14:30:00").unwrap();
        assert_eq!(rec.state, "recording");

        db.set_recording_dir(rec.id, "/tmp/rec_1").unwrap();
        db.set_recording_duration(rec.id, 120).unwrap();
        db.set_recording_state(rec.id, "error", Some("boom")).unwrap();

        let failed = db.get_failed_recordings("proj").unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].duration_secs, 120);
        assert_eq!(failed[0].error.as_deref(), Some("boom"));

        db.delete_recording(rec.id).unwrap();
        assert!(db.get_failed_recordings("proj").unwrap().is_empty());
    }

    #[test]
    fn test_fail_stale_recordings() {
        let db = Database::new(":memory:").unwrap();
        let a = db.create_recording("proj", "2026-07-08 14:30:00").unwrap();
        let b = db.create_recording("proj", "2026-07-08 15:00:00").unwrap();
        db.set_recording_state(b.id, "done", None).unwrap();

        db.fail_stale_recordings().unwrap();

        assert_eq!(db.get_recording(a.id).unwrap().state, "error");
        assert_eq!(db.get_recording(b.id).unwrap().state, "done");
    }

    #[test]
    fn test_project_summary_prompt() {
        let db = Database::new(":memory:").unwrap();
        db.create_project("proj", "/p", "", "", &[]).unwrap();
        assert!(db.get_project_summary_prompt("proj").unwrap().is_none());

        db.set_project_summary_prompt("proj", Some("Resume court")).unwrap();
        assert_eq!(db.get_project_summary_prompt("proj").unwrap().unwrap(), "Resume court");

        db.set_project_summary_prompt("proj", Some("  ")).unwrap();
        assert!(db.get_project_summary_prompt("proj").unwrap().is_none());
    }
}
