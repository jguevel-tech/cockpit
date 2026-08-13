use super::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub project: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteFolder {
    pub id: i64,
    pub project: String,
    pub parent_id: Option<i64>,
    pub name: String,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteFile {
    pub id: i64,
    pub project: String,
    pub folder_id: Option<i64>,
    pub name: String,
    pub content: String,
    pub position: i32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteTree {
    pub folders: Vec<NoteFolder>,
    pub files: Vec<NoteFile>,
}

impl NoteFolder {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            parent_id: row.get(2)?,
            name: row.get(3)?,
            position: row.get(4)?,
        })
    }
}

impl NoteFile {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            folder_id: row.get(2)?,
            name: row.get(3)?,
            content: row.get(4)?,
            position: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }
}

impl Database {
    // --- Simple notes (one per project) ---

    pub fn get_note(&self, project: &str) -> Result<Option<Note>, String> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, project, content, created_at, updated_at FROM notes WHERE project=?1 ORDER BY id DESC LIMIT 1",
            [project],
            |row| Ok(Note { id: row.get(0)?, project: row.get(1)?, content: row.get(2)?, created_at: row.get(3)?, updated_at: row.get(4)? }),
        ) {
            Ok(note) => Ok(Some(note)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn save_note(&self, project: &str, content: &str) -> Result<Note, String> {
        let conn = self.conn();
        let existing_id: Option<i64> = conn
            .query_row("SELECT id FROM notes WHERE project=?1 LIMIT 1", [project], |row| row.get(0))
            .ok();

        let id = match existing_id {
            Some(id) => {
                conn.execute(
                    "UPDATE notes SET content=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
                    rusqlite::params![content, id],
                ).map_err(|e| e.to_string())?;
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO notes (project, content) VALUES (?1, ?2)",
                    rusqlite::params![project, content],
                ).map_err(|e| e.to_string())?;
                conn.last_insert_rowid()
            }
        };

        conn.query_row(
            "SELECT id, project, content, created_at, updated_at FROM notes WHERE id=?1",
            [id],
            |row| Ok(Note { id: row.get(0)?, project: row.get(1)?, content: row.get(2)?, created_at: row.get(3)?, updated_at: row.get(4)? }),
        ).map_err(|e| e.to_string())
    }

    // --- Note tree (folders + files) ---

    pub fn get_note_tree(&self, project: &str) -> Result<NoteTree, String> {
        let conn = self.conn();

        let mut folder_stmt = conn
            .prepare("SELECT id, project, parent_id, name, position FROM note_folders WHERE project=?1 ORDER BY position, name")
            .map_err(|e| e.to_string())?;
        let folders: Vec<NoteFolder> = folder_stmt
            .query_map([project], NoteFolder::from_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let mut file_stmt = conn
            .prepare("SELECT id, project, folder_id, name, content, position, updated_at FROM note_files WHERE project=?1 ORDER BY position, name")
            .map_err(|e| e.to_string())?;
        let files: Vec<NoteFile> = file_stmt
            .query_map([project], NoteFile::from_row)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(NoteTree { folders, files })
    }

    pub fn create_note_folder(&self, project: &str, parent_id: Option<i64>, name: &str) -> Result<NoteFolder, String> {
        let pos = match parent_id {
            Some(pid) => {
                // On ne peut pas utiliser next_position directement car il faut filtrer sur 2 colonnes
                let conn = self.conn();
                conn.query_row(
                    "SELECT COALESCE(MAX(position), -1) FROM note_folders WHERE project=?1 AND parent_id=?2",
                    rusqlite::params![project, pid], |row| row.get::<_, i32>(0),
                ).unwrap_or(-1) + 1
            }
            None => self.next_position_null("note_folders", "parent_id"),
        };

        let conn = self.conn();
        conn.execute(
            "INSERT INTO note_folders (project, parent_id, name, position) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project, parent_id, name, pos],
        ).map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, project, parent_id, name, position FROM note_folders WHERE id=?1",
            [id],
            NoteFolder::from_row,
        ).map_err(|e| e.to_string())
    }

    pub fn rename_note_folder(&self, id: i64, name: &str) -> Result<(), String> {
        self.conn().execute("UPDATE note_folders SET name=?1 WHERE id=?2", rusqlite::params![name, id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_note_folder(&self, id: i64) -> Result<(), String> {
        self.conn().execute("DELETE FROM note_folders WHERE id=?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn create_note_file(&self, project: &str, folder_id: Option<i64>, name: &str) -> Result<NoteFile, String> {
        let pos = match folder_id {
            Some(fid) => {
                let conn = self.conn();
                conn.query_row(
                    "SELECT COALESCE(MAX(position), -1) FROM note_files WHERE project=?1 AND folder_id=?2",
                    rusqlite::params![project, fid], |row| row.get::<_, i32>(0),
                ).unwrap_or(-1) + 1
            }
            None => self.next_position_null("note_files", "folder_id"),
        };

        let conn = self.conn();
        conn.execute(
            "INSERT INTO note_files (project, folder_id, name, position) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project, folder_id, name, pos],
        ).map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, project, folder_id, name, content, position, updated_at FROM note_files WHERE id=?1",
            [id],
            NoteFile::from_row,
        ).map_err(|e| e.to_string())
    }

    pub fn get_note_file(&self, id: i64) -> Result<NoteFile, String> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, project, folder_id, name, content, position, updated_at FROM note_files WHERE id=?1",
            [id],
            NoteFile::from_row,
        ).map_err(|e| e.to_string())
    }

    pub fn save_note_file(&self, id: i64, content: &str) -> Result<(), String> {
        self.conn().execute("UPDATE note_files SET content=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2", rusqlite::params![content, id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn rename_note_file(&self, id: i64, name: &str) -> Result<(), String> {
        self.conn().execute("UPDATE note_files SET name=?1 WHERE id=?2", rusqlite::params![name, id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_note_file(&self, id: i64) -> Result<(), String> {
        self.conn().execute("DELETE FROM note_files WHERE id=?1", [id]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn reorder_note_folders(&self, ids: &[i64]) -> Result<(), String> {
        self.reorder_by_ids("note_folders", "id", ids)
    }

    pub fn reorder_note_files(&self, ids: &[i64]) -> Result<(), String> {
        self.reorder_by_ids("note_files", "id", ids)
    }

    pub fn move_note_file(&self, id: i64, folder_id: Option<i64>) -> Result<(), String> {
        let pos = match folder_id {
            Some(fid) => self.next_position("note_files", "folder_id", &fid),
            None => self.next_position_null("note_files", "folder_id"),
        };

        let conn = self.conn();
        conn.execute(
            "UPDATE note_files SET folder_id=?1, position=?2 WHERE id=?3",
            rusqlite::params![folder_id, pos, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_note_save_and_get() {
        let db = Database::new(":memory:").unwrap();
        assert!(db.get_note("proj").unwrap().is_none());

        let n = db.save_note("proj", "Hello").unwrap();
        assert_eq!(n.content, "Hello");

        let n2 = db.save_note("proj", "Updated").unwrap();
        assert_eq!(n2.id, n.id);
        assert_eq!(n2.content, "Updated");
    }

    #[test]
    fn test_note_tree() {
        let db = Database::new(":memory:").unwrap();
        let folder = db.create_note_folder("proj", None, "Docs").unwrap();
        assert_eq!(folder.name, "Docs");

        let file = db.create_note_file("proj", Some(folder.id), "README.md").unwrap();
        assert_eq!(file.name, "README.md");

        db.save_note_file(file.id, "# Title").unwrap();
        let loaded = db.get_note_file(file.id).unwrap();
        assert_eq!(loaded.content, "# Title");

        let tree = db.get_note_tree("proj").unwrap();
        assert_eq!(tree.folders.len(), 1);
        assert_eq!(tree.files.len(), 1);

        db.delete_note_file(file.id).unwrap();
        db.delete_note_folder(folder.id).unwrap();
        let tree2 = db.get_note_tree("proj").unwrap();
        assert!(tree2.folders.is_empty());
        assert!(tree2.files.is_empty());
    }

    #[test]
    fn test_reorder_note_folders() {
        let db = Database::new(":memory:").unwrap();
        let a = db.create_note_folder("proj", None, "Alpha").unwrap();
        let b = db.create_note_folder("proj", None, "Beta").unwrap();
        let c = db.create_note_folder("proj", None, "Gamma").unwrap();

        db.reorder_note_folders(&[c.id, a.id, b.id]).unwrap();

        let tree = db.get_note_tree("proj").unwrap();
        assert_eq!(tree.folders[0].name, "Gamma");
        assert_eq!(tree.folders[1].name, "Alpha");
        assert_eq!(tree.folders[2].name, "Beta");
    }

    #[test]
    fn test_reorder_note_files() {
        let db = Database::new(":memory:").unwrap();
        let folder = db.create_note_folder("proj", None, "Docs").unwrap();
        let a = db.create_note_file("proj", Some(folder.id), "A.md").unwrap();
        let b = db.create_note_file("proj", Some(folder.id), "B.md").unwrap();
        let c = db.create_note_file("proj", Some(folder.id), "C.md").unwrap();

        db.reorder_note_files(&[c.id, a.id, b.id]).unwrap();

        let tree = db.get_note_tree("proj").unwrap();
        let folder_files: Vec<_> = tree.files.iter().filter(|f| f.folder_id == Some(folder.id)).collect();
        assert_eq!(folder_files[0].name, "C.md");
        assert_eq!(folder_files[1].name, "A.md");
        assert_eq!(folder_files[2].name, "B.md");
    }

    #[test]
    fn test_move_note_file() {
        let db = Database::new(":memory:").unwrap();
        let folder_a = db.create_note_folder("proj", None, "FolderA").unwrap();
        let folder_b = db.create_note_folder("proj", None, "FolderB").unwrap();
        let file = db.create_note_file("proj", Some(folder_a.id), "notes.md").unwrap();

        db.move_note_file(file.id, Some(folder_b.id)).unwrap();
        let moved = db.get_note_file(file.id).unwrap();
        assert_eq!(moved.folder_id, Some(folder_b.id));

        db.move_note_file(file.id, None).unwrap();
        let at_root = db.get_note_file(file.id).unwrap();
        assert_eq!(at_root.folder_id, None);
    }
}
