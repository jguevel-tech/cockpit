use super::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Url {
    pub id: i64,
    pub project: String,
    pub label: String,
    pub url: String,
    pub position: i32,
}

impl Url {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            label: row.get(2)?,
            url: row.get(3)?,
            position: row.get(4)?,
        })
    }
}

impl Database {
    pub fn get_urls(&self, project: &str) -> Result<Vec<Url>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT id, project, label, url, position FROM urls WHERE project=?1 ORDER BY position, id")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([project], Url::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn create_url(&self, project: &str, label: &str, url: &str) -> Result<Url, String> {
        let pos = self.next_position("urls", "project", &project);
        let conn = self.conn();

        conn.execute(
            "INSERT INTO urls (project, label, url, position) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project, label, url, pos],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, project, label, url, position FROM urls WHERE id=?1",
            [id],
            Url::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn update_url(&self, id: i64, label: &str, url: &str) -> Result<Url, String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE urls SET label=?1, url=?2 WHERE id=?3",
            rusqlite::params![label, url, id],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, project, label, url, position FROM urls WHERE id=?1",
            [id],
            Url::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn delete_url(&self, id: i64) -> Result<(), String> {
        self.conn()
            .execute("DELETE FROM urls WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_url_crud() {
        let db = Database::new(":memory:").unwrap();
        let u = db.create_url("proj", "Google", "https://google.com").unwrap();
        assert_eq!(u.label, "Google");

        let updated = db.update_url(u.id, "DuckDuckGo", "https://duckduckgo.com").unwrap();
        assert_eq!(updated.label, "DuckDuckGo");
        assert_eq!(updated.url, "https://duckduckgo.com");

        db.delete_url(updated.id).unwrap();
        let urls = db.get_urls("proj").unwrap();
        assert!(urls.is_empty());
    }
}
