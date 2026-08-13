use super::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFolder {
    pub id: i64,
    pub name: String,
    pub position: i32,
}

impl ProjectFolder {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            position: row.get(2)?,
        })
    }
}

impl Database {
    pub fn get_project_folders(&self) -> Result<Vec<ProjectFolder>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT id, name, position FROM project_folders ORDER BY position, name")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], ProjectFolder::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn create_project_folder(&self, name: &str) -> Result<ProjectFolder, String> {
        let pos = self.next_position_null("project_folders", "id");
        let conn = self.conn();
        conn.execute(
            "INSERT INTO project_folders (name, position) VALUES (?1, ?2)",
            rusqlite::params![name, pos],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, name, position FROM project_folders WHERE id=?1",
            [id],
            ProjectFolder::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn rename_project_folder(&self, id: i64, name: &str) -> Result<(), String> {
        self.conn()
            .execute(
                "UPDATE project_folders SET name=?1 WHERE id=?2",
                rusqlite::params![name, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_project_folder(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        // Detacher les projets du dossier avant suppression
        // (ALTER TABLE ne supporte pas ON DELETE SET NULL dans SQLite)
        conn.execute("UPDATE projects SET folder_id=NULL WHERE folder_id=?1", [id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM project_folders WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn reorder_project_folders(&self, ids: &[i64]) -> Result<(), String> {
        self.reorder_by_ids("project_folders", "id", ids)
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_project_folder_crud() {
        let db = Database::new(":memory:").unwrap();
        let f = db.create_project_folder("DevOps").unwrap();
        assert_eq!(f.name, "DevOps");

        db.rename_project_folder(f.id, "Infrastructure").unwrap();
        let folders = db.get_project_folders().unwrap();
        assert_eq!(folders[0].name, "Infrastructure");

        db.delete_project_folder(f.id).unwrap();
        let folders = db.get_project_folders().unwrap();
        assert!(folders.is_empty());
    }

    #[test]
    fn test_move_project_to_folder() {
        let db = Database::new(":memory:").unwrap();
        let f = db.create_project_folder("Web").unwrap();
        let p = db.create_project("mysite", "/site", "", "", &[]).unwrap();
        assert_eq!(p.folder_id, None);

        db.move_project_to_folder("mysite", Some(f.id)).unwrap();
        let p2 = db.get_project_by_name("mysite").unwrap();
        assert_eq!(p2.folder_id, Some(f.id));

        db.move_project_to_folder("mysite", None).unwrap();
        let p3 = db.get_project_by_name("mysite").unwrap();
        assert_eq!(p3.folder_id, None);
    }

    #[test]
    fn test_delete_folder_unlinks_projects() {
        let db = Database::new(":memory:").unwrap();
        let f = db.create_project_folder("Temp").unwrap();
        db.create_project("proj1", "/p1", "", "", &[]).unwrap();
        db.move_project_to_folder("proj1", Some(f.id)).unwrap();

        db.delete_project_folder(f.id).unwrap();
        let p = db.get_project_by_name("proj1").unwrap();
        assert_eq!(p.folder_id, None);
    }
}
