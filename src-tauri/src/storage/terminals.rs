use super::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalRow {
    pub id: i64,
    pub project: String,
    pub name: String,
    pub tmux_name: String,
}

impl TerminalRow {
    const SELECT_COLS: &'static str = "id, project, name, tmux_name";

    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            name: row.get(2)?,
            tmux_name: row.get(3)?,
        })
    }
}

impl Database {
    pub fn create_terminal_row(&self, project: &str) -> Result<TerminalRow, String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO terminals (project, name, tmux_name) VALUES (?1, '', '')",
            [project],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        let tmux_name = format!("ckpt_{}", id);
        conn.execute(
            "UPDATE terminals SET tmux_name=?1 WHERE id=?2",
            rusqlite::params![tmux_name, id],
        )
        .map_err(|e| e.to_string())?;
        conn.query_row(
            &format!("SELECT {} FROM terminals WHERE id=?1", TerminalRow::SELECT_COLS),
            [id],
            TerminalRow::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn get_terminal_row(&self, id: i64) -> Result<TerminalRow, String> {
        let conn = self.conn();
        conn.query_row(
            &format!("SELECT {} FROM terminals WHERE id=?1", TerminalRow::SELECT_COLS),
            [id],
            TerminalRow::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn get_terminal_rows(&self, project: Option<&str>) -> Result<Vec<TerminalRow>, String> {
        let conn = self.conn();
        match project {
            Some(p) => {
                let mut stmt = conn
                    .prepare(&format!(
                        "SELECT {} FROM terminals WHERE project=?1 ORDER BY id",
                        TerminalRow::SELECT_COLS
                    ))
                    .map_err(|e| e.to_string())?;
                let rows = stmt.query_map([p], TerminalRow::from_row).map_err(|e| e.to_string())?;
                rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
            }
            None => {
                let mut stmt = conn
                    .prepare(&format!(
                        "SELECT {} FROM terminals ORDER BY project, id",
                        TerminalRow::SELECT_COLS
                    ))
                    .map_err(|e| e.to_string())?;
                let rows = stmt.query_map([], TerminalRow::from_row).map_err(|e| e.to_string())?;
                rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
            }
        }
    }

    pub fn rename_terminal_row(&self, id: i64, name: &str) -> Result<(), String> {
        self.conn()
            .execute("UPDATE terminals SET name=?1 WHERE id=?2", rusqlite::params![name, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_terminal_row(&self, id: i64) -> Result<(), String> {
        self.conn()
            .execute("DELETE FROM terminals WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_terminal_rows_lifecycle() {
        let db = Database::new(":memory:").unwrap();
        let t = db.create_terminal_row("proj").unwrap();
        assert_eq!(t.tmux_name, format!("ckpt_{}", t.id));

        db.rename_terminal_row(t.id, "logs api").unwrap();
        assert_eq!(db.get_terminal_row(t.id).unwrap().name, "logs api");

        let t2 = db.create_terminal_row("autre").unwrap();
        assert_eq!(db.get_terminal_rows(Some("proj")).unwrap().len(), 1);
        assert_eq!(db.get_terminal_rows(None).unwrap().len(), 2);

        db.delete_terminal_row(t.id).unwrap();
        db.delete_terminal_row(t2.id).unwrap();
        assert!(db.get_terminal_rows(None).unwrap().is_empty());
    }
}
