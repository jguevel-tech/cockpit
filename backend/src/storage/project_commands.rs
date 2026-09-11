use super::db::Database;
use serde::{Deserialize, Serialize};

/// Commande rapide d'un projet : un bouton qui lance `command` dans un terminal Cockpit.
/// La commande est TAPEE dans le shell du terminal (send-keys), jamais interpretee par Rust.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCommand {
    pub id: i64,
    pub project: String,
    pub label: String,
    pub command: String,
    pub position: i32,
}

impl ProjectCommand {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            label: row.get(2)?,
            command: row.get(3)?,
            position: row.get(4)?,
        })
    }
}

const SELECT: &str = "SELECT id, project, label, command, position FROM project_commands";

impl Database {
    pub fn get_project_commands(&self, project: &str) -> Result<Vec<ProjectCommand>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!("{SELECT} WHERE project=?1 ORDER BY position, id"))
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([project], ProjectCommand::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn create_project_command(&self, project: &str, label: &str, command: &str) -> Result<ProjectCommand, String> {
        let pos = self.next_position("project_commands", "project", &project);
        let conn = self.conn();

        conn.execute(
            "INSERT INTO project_commands (project, label, command, position) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project, label, command, pos],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(&format!("{SELECT} WHERE id=?1"), [id], ProjectCommand::from_row)
            .map_err(|e| e.to_string())
    }

    pub fn update_project_command(&self, id: i64, label: &str, command: &str) -> Result<ProjectCommand, String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE project_commands SET label=?1, command=?2 WHERE id=?3",
            rusqlite::params![label, command, id],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(&format!("{SELECT} WHERE id=?1"), [id], ProjectCommand::from_row)
            .map_err(|e| e.to_string())
    }

    pub fn delete_project_command(&self, id: i64) -> Result<(), String> {
        self.conn()
            .execute("DELETE FROM project_commands WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn reorder_project_commands(&self, ids: &[i64]) -> Result<(), String> {
        self.reorder_by_ids("project_commands", "id", ids)
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_project_command_crud_et_ordre() {
        let db = Database::new(":memory:").unwrap();
        let a = db.create_project_command("proj", "Dev", "npm run dev").unwrap();
        let b = db.create_project_command("proj", "Up", "make up").unwrap();
        assert_eq!(a.command, "npm run dev");

        let updated = db.update_project_command(a.id, "Dev serveur", "npm run dev -- --host").unwrap();
        assert_eq!(updated.label, "Dev serveur");

        db.reorder_project_commands(&[b.id, a.id]).unwrap();
        let cmds = db.get_project_commands("proj").unwrap();
        assert_eq!(cmds[0].label, "Up");

        db.delete_project_command(a.id).unwrap();
        assert_eq!(db.get_project_commands("proj").unwrap().len(), 1);
    }

    #[test]
    fn test_project_commands_dans_les_tables_scopees() {
        // Regle du projet : toute table referencant un projet DOIT etre dans
        // PROJECT_SCOPED_TABLES, sinon delete/rename laissent des orphelins.
        assert!(crate::storage::projects::PROJECT_SCOPED_TABLES.contains(&"project_commands"));
    }
}
