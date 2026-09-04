use super::db::Database;
use serde::{Deserialize, Serialize};

/// Ce qui, d'un terminal, doit survivre a l'extinction du poste : son projet, son nom
/// d'onglet, le dossier ou son shell a ete ouvert, et la derniere photo de son ecran.
///
/// Le shell lui-meme ne survit pas — le noyau le tue — et le service de terminaux non plus.
/// Ce qui est rendu au retour, c'est ce que le terminal AFFICHAIT, dans le meme dossier, avec
/// un shell neuf a la suite. Voir `terminal/adaptateur.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalRow {
    pub id: i64,
    pub project: String,
    pub name: String,
    /// Dossier de depart du shell. Vide = la racine du projet.
    pub cwd: String,
}

impl TerminalRow {
    const SELECT_COLS: &'static str = "id, project, name, cwd";

    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            name: row.get(2)?,
            cwd: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        })
    }
}

impl Database {
    pub fn create_terminal_row(&self, project: &str) -> Result<TerminalRow, String> {
        self.create_terminal_row_dans(project, "")
    }

    /// La meme chose, en retenant le dossier ou le shell s'ouvre : c'est lui qu'un terminal
    /// restaure doit retrouver, et pas la racine du projet.
    pub fn create_terminal_row_dans(
        &self,
        project: &str,
        cwd: &str,
    ) -> Result<TerminalRow, String> {
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
            "INSERT INTO terminals (project, name, cwd) VALUES (?1, ?2, ?3)",
            [project, &name, cwd],
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

    /// Range la derniere photo d'un terminal. Des OCTETS de terminal, jamais du texte : ils
    /// portent les couleurs, le curseur et les modes, et se redonnent tels quels.
    pub fn set_terminal_snapshot(&self, id: i64, octets: &[u8]) -> Result<(), String> {
        let quand = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn()
            .execute(
                "UPDATE terminals SET snapshot=?1, snapshot_at=?2 WHERE id=?3",
                rusqlite::params![octets, quand, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// La derniere photo d'un terminal, si elle existe.
    ///
    /// Une photo ABSENTE n'est pas une erreur : un terminal qui vient de naitre n'en a pas,
    /// et un terminal dont la photo a echoue doit pouvoir se rouvrir vide.
    pub fn get_terminal_snapshot(&self, id: i64) -> Vec<u8> {
        self.conn()
            .query_row("SELECT snapshot FROM terminals WHERE id=?1", [id], |r| {
                r.get::<_, Option<Vec<u8>>>(0)
            })
            .ok()
            .flatten()
            .unwrap_or_default()
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

    /// Ce qui rend un terminal « comme on l'a quitte » : le dossier ET la photo de l'ecran.
    ///
    /// La photo est BINAIRE : elle porte des octets de terminal (couleurs, curseur), pas du
    /// texte. Un aller-retour qui passerait par une chaine les abimerait sans rien dire.
    #[test]
    fn le_dossier_et_la_photo_traversent_la_base() {
        let db = Database::new(":memory:").unwrap();
        let t = db.create_terminal_row_dans("cockpit", "/tmp/projet/api").unwrap();
        assert_eq!(t.cwd, "/tmp/projet/api");
        assert_eq!(db.get_terminal_row(t.id).unwrap().cwd, "/tmp/projet/api");

        // Aucune photo au depart : un terminal neuf s'ouvre vide, ce n'est pas une panne.
        assert!(db.get_terminal_snapshot(t.id).is_empty());

        let photo = vec![0x1b, b'c', 0x00, 0xff, b'\n', 0xfe];
        db.set_terminal_snapshot(t.id, &photo).unwrap();
        assert_eq!(db.get_terminal_snapshot(t.id), photo);

        // Une photo se REMPLACE, elle ne s'efface pas : la perdre entre une restauration et
        // la photo suivante ferait perdre l'ecran pour de bon si la machine s'arretait la.
        db.set_terminal_snapshot(t.id, b"\x1b[2Jplus recent").unwrap();
        assert_eq!(db.get_terminal_snapshot(t.id), b"\x1b[2Jplus recent");

        // Un terminal sans dossier retombe sur la racine du projet, cote adaptateur.
        let nu = db.create_terminal_row("cockpit").unwrap();
        assert_eq!(nu.cwd, "");
        // Et un identifiant inconnu ne fait pas paniquer la lecture.
        assert!(db.get_terminal_snapshot(999_999).is_empty());
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
