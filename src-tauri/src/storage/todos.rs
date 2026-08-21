use super::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: i64,
    pub project: String,
    pub text: String,
    pub done: bool,
    pub position: i32,
    pub created_at: String,
    /// Echeance optionnelle, date ISO "2026-08-20" (NULL = sans echeance)
    pub due_date: Option<String>,
    /// Avancement en pourcentage, 0 a 100. 0 = pas commencee.
    pub progress: i32,
}

impl Todo {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            text: row.get(2)?,
            done: row.get::<_, i32>(3)? != 0,
            position: row.get(4)?,
            created_at: row.get(5)?,
            due_date: row.get(6)?,
            progress: row.get(7)?,
        })
    }
}

impl Database {
    pub fn get_todos(&self, project: &str) -> Result<Vec<Todo>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT id, project, text, done, position, created_at, due_date, progress FROM todos WHERE project=?1 ORDER BY position, id")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([project], Todo::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn create_todo(&self, project: &str, text: &str) -> Result<Todo, String> {
        let pos = self.next_position("todos", "project", &project);
        let conn = self.conn();

        conn.execute(
            "INSERT INTO todos (project, text, position) VALUES (?1, ?2, ?3)",
            rusqlite::params![project, text, pos],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, project, text, done, position, created_at, due_date, progress FROM todos WHERE id=?1",
            [id],
            Todo::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn update_todo(&self, id: i64, text: &str, done: bool) -> Result<Todo, String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE todos SET text=?1, done=?2 WHERE id=?3",
            rusqlite::params![text, done as i32, id],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, project, text, done, position, created_at, due_date, progress FROM todos WHERE id=?1",
            [id],
            Todo::from_row,
        )
        .map_err(|e| e.to_string())
    }

    /// Pose ou retire l'echeance d'une tache (None = retirer).
    pub fn set_todo_due(&self, id: i64, due_date: Option<&str>) -> Result<Todo, String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE todos SET due_date=?1 WHERE id=?2",
            rusqlite::params![due_date, id],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, project, text, done, position, created_at, due_date, progress FROM todos WHERE id=?1",
            [id],
            Todo::from_row,
        )
        .map_err(|e| e.to_string())
    }

    /// Pose l'avancement d'une tache, en pourcentage.
    ///
    /// La valeur est BORNEE ici, et pas seulement dans l'interface : une commande Tauri est
    /// appelable depuis n'importe ou, et une barre de progression a -30 % ou 400 % dessinerait
    /// n'importe quoi. 100 % marque la tache comme finie — c'est ce que veut dire « fini »,
    /// et deux verites pour une meme chose finiraient par diverger.
    pub fn set_todo_progress(&self, id: i64, progress: i32) -> Result<Todo, String> {
        let progress = progress.clamp(0, 100);
        let conn = self.conn();
        conn.execute(
            "UPDATE todos SET progress=?1, done=?2 WHERE id=?3",
            rusqlite::params![progress, progress >= 100, id],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            "SELECT id, project, text, done, position, created_at, due_date, progress FROM todos WHERE id=?1",
            [id],
            Todo::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn delete_todo(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        conn.execute("DELETE FROM todos WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn reorder_todos(&self, ids: &[i64]) -> Result<(), String> {
        self.reorder_by_ids("todos", "id", ids)
    }

    pub fn move_todo(&self, id: i64, new_project: &str) -> Result<(), String> {
        let pos = self.next_position("todos", "project", &new_project);
        let conn = self.conn();
        conn.execute(
            "UPDATE todos SET project=?1, position=?2 WHERE id=?3",
            rusqlite::params![new_project, pos, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_pending_todos(&self) -> Result<Vec<Todo>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.project, t.text, t.done, t.position, t.created_at, t.due_date,
                        t.progress
                 FROM todos t
                 LEFT JOIN projects p ON t.project = p.name
                 WHERE t.done = 0
                 ORDER BY COALESCE(p.position, 999), t.project, t.position, t.id",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], Todo::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    /// L'avancement est BORNE en base, pas seulement dans l'interface : une commande Tauri est
    /// appelable depuis n'importe ou, et une barre a -30 % ou 400 % dessinerait n'importe quoi.
    #[test]
    fn l_avancement_est_borne_entre_0_et_100() {
        let db = Database::new(":memory:").unwrap();
        let t = db.create_todo("proj", "Ranger le garage").unwrap();
        assert_eq!(t.progress, 0, "une tache neuve n'est pas commencee");

        assert_eq!(db.set_todo_progress(t.id, 40).unwrap().progress, 40);
        assert_eq!(db.set_todo_progress(t.id, -30).unwrap().progress, 0);
        assert_eq!(db.set_todo_progress(t.id, 400).unwrap().progress, 100);
    }

    /// 100 % et « finie » sont la MEME chose : deux verites pour un seul etat finiraient par
    /// diverger, et l'utilisateur verrait une tache pleine a 100 % encore dans ses en-cours.
    #[test]
    fn cent_pour_cent_marque_la_tache_finie() {
        let db = Database::new(":memory:").unwrap();
        let t = db.create_todo("proj", "Sortir les poubelles").unwrap();

        let en_cours = db.set_todo_progress(t.id, 60).unwrap();
        assert!(!en_cours.done, "60 % n'est pas fini");

        let finie = db.set_todo_progress(t.id, 100).unwrap();
        assert!(finie.done, "100 % doit marquer la tache finie");

        // Et on peut la reprendre : redescendre la remet dans les en-cours.
        let reprise = db.set_todo_progress(t.id, 70).unwrap();
        assert!(!reprise.done, "redescendre sous 100 % doit rouvrir la tache");
    }

    #[test]
    fn test_todo_crud() {
        let db = Database::new(":memory:").unwrap();
        let t = db.create_todo("proj", "Buy milk").unwrap();
        assert_eq!(t.text, "Buy milk");
        assert!(!t.done);

        let updated = db.update_todo(t.id, "Buy oat milk", true).unwrap();
        assert_eq!(updated.text, "Buy oat milk");
        assert!(updated.done);

        db.delete_todo(updated.id).unwrap();
        let todos = db.get_todos("proj").unwrap();
        assert!(todos.is_empty());
    }

    #[test]
    fn test_todo_due_date() {
        let db = Database::new(":memory:").unwrap();
        let t = db.create_todo("proj", "Rendre le rapport").unwrap();
        assert_eq!(t.due_date, None);

        let with_due = db.set_todo_due(t.id, Some("2026-08-20")).unwrap();
        assert_eq!(with_due.due_date.as_deref(), Some("2026-08-20"));

        // L'echeance survit aux autres mises a jour et sort dans les listes
        db.update_todo(t.id, "Rendre le rapport final", false).unwrap();
        assert_eq!(db.get_todos("proj").unwrap()[0].due_date.as_deref(), Some("2026-08-20"));
        assert_eq!(db.get_pending_todos().unwrap()[0].due_date.as_deref(), Some("2026-08-20"));

        let cleared = db.set_todo_due(t.id, None).unwrap();
        assert_eq!(cleared.due_date, None);
    }

    #[test]
    fn test_todo_reorder() {
        let db = Database::new(":memory:").unwrap();
        let a = db.create_todo("proj", "A").unwrap();
        let b = db.create_todo("proj", "B").unwrap();
        let c = db.create_todo("proj", "C").unwrap();

        db.reorder_todos(&[c.id, a.id, b.id]).unwrap();
        let todos = db.get_todos("proj").unwrap();
        assert_eq!(todos[0].text, "C");
        assert_eq!(todos[1].text, "A");
        assert_eq!(todos[2].text, "B");
    }

    #[test]
    fn test_move_todo() {
        let db = Database::new(":memory:").unwrap();
        let t = db.create_todo("proj_a", "Task1").unwrap();
        assert_eq!(t.project, "proj_a");

        db.move_todo(t.id, "proj_b").unwrap();
        let todos_a = db.get_todos("proj_a").unwrap();
        let todos_b = db.get_todos("proj_b").unwrap();
        assert!(todos_a.is_empty());
        assert_eq!(todos_b.len(), 1);
        assert_eq!(todos_b[0].text, "Task1");
        assert_eq!(todos_b[0].project, "proj_b");
    }
}
