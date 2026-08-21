use super::db::Database;
use serde::{Deserialize, Serialize};

/// Ce qui, d'un terminal, doit survivre a un redemarrage de la machine : son projet et son
/// nom d'onglet. Tout le reste (le shell, l'ecran, la taille) appartient au service de
/// terminaux, qui ne survit pas au redemarrage — voir `terminal/adaptateur.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalRow {
    pub id: i64,
    pub project: String,
    pub name: String,
}

impl TerminalRow {
    const SELECT_COLS: &'static str = "id, project, name";

    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            name: row.get(2)?,
        })
    }
}

impl Database {
    pub fn create_terminal_row(&self, project: &str) -> Result<TerminalRow, String> {
        let conn = self.conn();
        // Nom par defaut : « PROJET - N » (projet en majuscules). N = plus grand suffixe
        // deja attribue pour CE projet + 1, et non le nombre de terminaux : apres fermeture
        // de « COCKPIT - 1 », le suivant est « - 3 », jamais un doublon de « - 2 ».
        let prefix = format!("{} - ", project.to_uppercase());
        let next = {
            let mut stmt = conn
                .prepare("SELECT name FROM terminals WHERE project=?1")
                .map_err(|e| e.to_string())?;
            let max = stmt
                .query_map([project], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .flatten()
                .filter_map(|name| name.strip_prefix(&prefix)?.trim().parse::<i64>().ok())
                .max()
                .unwrap_or(0);
            max + 1
        };
        let name = format!("{}{}", prefix, next);
        conn.execute(
            "INSERT INTO terminals (project, name) VALUES (?1, ?2)",
            [project, &name],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
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
        assert!(t.id > 0);

        db.rename_terminal_row(t.id, "logs api").unwrap();
        assert_eq!(db.get_terminal_row(t.id).unwrap().name, "logs api");

        let t2 = db.create_terminal_row("autre").unwrap();
        assert_eq!(db.get_terminal_rows(Some("proj")).unwrap().len(), 1);
        assert_eq!(db.get_terminal_rows(None).unwrap().len(), 2);

        db.delete_terminal_row(t.id).unwrap();
        db.delete_terminal_row(t2.id).unwrap();
        assert!(db.get_terminal_rows(None).unwrap().is_empty());
    }

    #[test]
    fn noms_par_defaut_projet_majuscule_et_numerote() {
        let db = Database::new(":memory:").unwrap();

        let t1 = db.create_terminal_row("cockpit").unwrap();
        let t2 = db.create_terminal_row("cockpit").unwrap();
        assert_eq!(t1.name, "COCKPIT - 1");
        assert_eq!(t2.name, "COCKPIT - 2");

        // La numerotation est PAR PROJET.
        let autre = db.create_terminal_row("inkdrop").unwrap();
        assert_eq!(autre.name, "INKDROP - 1");

        // Apres fermeture du - 1, le suivant est - 3 : jamais de doublon avec le - 2 restant.
        db.delete_terminal_row(t1.id).unwrap();
        let t3 = db.create_terminal_row("cockpit").unwrap();
        assert_eq!(t3.name, "COCKPIT - 3");

        // Un rename manuel sort le terminal de la serie sans casser la suite.
        db.rename_terminal_row(t2.id, "logs api").unwrap();
        let t4 = db.create_terminal_row("cockpit").unwrap();
        assert_eq!(t4.name, "COCKPIT - 4");
    }
}
