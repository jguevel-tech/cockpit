use super::db::Database;
use serde::{Deserialize, Serialize};

/// Projet des taches qui n'appartiennent a aucun projet : les idees notees au vol.
///
/// Une chaine VIDE et non une colonne nullable : la colonne est `NOT NULL` depuis toujours, la
/// synchronisation et l'import la transportent deja comme du texte, et aucun projet ne peut
/// porter ce nom (`create_project` et `rename_project` le refusent).
pub const SANS_PROJET: &str = "";

/// Prefixe des listes de taches nommees : une liste « Idees » range ses taches sous le projet
/// `liste:Idees`. Meme raison que `SANS_PROJET` : la colonne voyage deja comme du texte, donc
/// une liste creee ici arrive sur une autre machine sans rien changer a la synchronisation.
/// Aucun projet ne peut commencer par ce prefixe (`nom_de_projet_valide`).
pub const PREFIXE_LISTE: &str = "liste:";

/// Une valeur de `todos.project` qui designe une liste hors projet : « Hors projet » ou une
/// liste nommee. C'est ce que les commandes sur les listes exigent, pour ne JAMAIS renommer ou
/// vider un vrai projet a partir d'une valeur envoyee par l'interface.
pub fn est_une_liste(cle: &str) -> bool {
    cle == SANS_PROJET || nom_de_liste(cle).is_some()
}

/// Le nom d'une liste nommee, sans son prefixe ; `None` pour tout le reste.
fn nom_de_liste(cle: &str) -> Option<&str> {
    cle.strip_prefix(PREFIXE_LISTE).filter(|n| !n.trim().is_empty())
}

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

    /// Renomme une liste hors projet : ses taches passent sous le nouveau nom. Renommer
    /// « Hors projet » en fait une liste nommee comme les autres.
    ///
    /// Refuse de fusionner deux listes : un renommage qui melangerait en silence les taches de
    /// deux listes ne se defait pas.
    pub fn renommer_liste_de_taches(&self, ancienne: &str, nouvelle: &str) -> Result<(), String> {
        if !est_une_liste(ancienne) || nom_de_liste(nouvelle).is_none() {
            return Err("Ce n'est pas une liste de tâches.".into());
        }
        if ancienne == nouvelle {
            return Ok(());
        }
        let conn = self.conn();
        let deja: i64 = conn
            .query_row("SELECT COUNT(*) FROM todos WHERE project=?1", [nouvelle], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if deja > 0 {
            return Err("Une liste porte déjà ce nom.".into());
        }
        conn.execute("UPDATE todos SET project=?1 WHERE project=?2", rusqlite::params![nouvelle, ancienne])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Supprime une liste hors projet et TOUTES ses taches, terminees comprises. Rend combien
    /// ont ete supprimees.
    pub fn supprimer_liste_de_taches(&self, cle: &str) -> Result<usize, String> {
        if !est_une_liste(cle) {
            return Err("Ce n'est pas une liste de tâches.".into());
        }
        self.conn()
            .execute("DELETE FROM todos WHERE project=?1", [cle])
            .map_err(|e| e.to_string())
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
                 ORDER BY (t.project = '' OR t.project LIKE 'liste:%') DESC, COALESCE(p.position, 999), t.project, t.position, t.id",
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
    /// Les taches hors projet passent en tete du tableau de bord : c'est la qu'on les note,
    /// et un projet qui n'existe pas les rangerait sinon tout en bas, apres les 999.
    #[test]
    fn les_taches_hors_projet_passent_en_tete() {
        let db = Database::new(":memory:").unwrap();
        db.create_project("alpha", "/tmp/alpha", "", "", &[]).unwrap();
        db.create_todo("alpha", "dans un projet").unwrap();
        db.create_todo(crate::storage::SANS_PROJET, "une idee\nsur deux lignes").unwrap();
        let toutes = db.get_pending_todos().unwrap();
        assert_eq!(toutes[0].project, "");
        assert_eq!(toutes[0].text, "une idee\nsur deux lignes", "les sauts de ligne sont gardes");
    }

    /// Les commandes sur les listes ne touchent JAMAIS un vrai projet : la cle vient de
    /// l'interface, et un renommage ou une suppression viderait sinon la liste d'un projet.
    #[test]
    fn une_liste_se_renomme_et_se_supprime_sans_toucher_aux_projets() {
        let db = Database::new(":memory:").unwrap();
        db.create_todo("alpha", "du projet").unwrap();
        db.create_todo("", "une idee").unwrap();

        db.renommer_liste_de_taches("", "liste:Idees").unwrap();
        let toutes = db.get_pending_todos().unwrap();
        assert_eq!(toutes[0].project, "liste:Idees", "les listes passent en tete");

        assert!(db.renommer_liste_de_taches("alpha", "liste:Vol").is_err(), "un projet n'est pas une liste");
        assert!(db.renommer_liste_de_taches("liste:Idees", "beta").is_err(), "une liste ne devient pas un projet");
        assert!(db.renommer_liste_de_taches("liste:Idees", "liste:  ").is_err(), "nom vide refuse");
        assert!(db.supprimer_liste_de_taches("alpha").is_err(), "on ne vide pas un projet");

        db.create_todo("liste:Courses", "pain").unwrap();
        assert!(db.renommer_liste_de_taches("liste:Courses", "liste:Idees").is_err(), "pas de fusion");

        assert_eq!(db.supprimer_liste_de_taches("liste:Idees").unwrap(), 1);
        assert_eq!(db.get_todos("alpha").unwrap().len(), 1, "le projet garde sa tache");
    }

    /// Un projet au nom vide recevrait toutes les taches hors projet.
    #[test]
    fn un_projet_ne_peut_pas_porter_un_nom_vide() {
        let db = Database::new(":memory:").unwrap();
        assert!(db.create_project("  ", "/tmp/x", "", "", &[]).is_err());
        db.create_project("beta", "/tmp/beta", "", "", &[]).unwrap();
        assert!(db.rename_project("beta", "", None).is_err());
        assert!(db.create_project("liste:Idees", "/tmp/y", "", "", &[]).is_err(), "le prefixe des listes est reserve");
    }

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
