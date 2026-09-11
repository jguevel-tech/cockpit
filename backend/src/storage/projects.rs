use super::db::Database;
use serde::{Deserialize, Serialize};

/// Tables qui referencent un projet par son NOM (colonne `project`, pas de FK).
/// Toute nouvelle table liee a un projet DOIT etre ajoutee ici : la liste est
/// utilisee par delete_project (cascade) ET rename_project (mise a jour).
pub const PROJECT_SCOPED_TABLES: &[&str] = &[
    "notes", "note_folders", "note_files", "todos", "urls",
    "recordings", "terminals", "command_history", "project_commands",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub compose_file: String,
    pub description: String,
    /// Stocke en DB comme JSON array (TEXT), expose deserialise au frontend.
    pub depends_on: Vec<String>,
    pub position: i32,
    pub folder_id: Option<i64>,
    pub created_at: String,
}

/// Traduit l'echec d'unicite de `projects.name` en message comprehensible.
///
/// SQLite rend « UNIQUE constraint failed: projects.name » et ce texte remontait tel quel
/// jusqu'au toast : un utilisateur qui renommait un projet vers un nom deja pris lisait un
/// message technique en anglais sans rapport apparent avec son geste (issue #6). Toute
/// ecriture du nom d'un projet passe par ici. L'interface controle deja le nom AVANT
/// d'appeler (message traduit) ; ceci est le filet, il couvre les courses et les appels
/// venus d'ailleurs.
fn erreur_nom(e: rusqlite::Error, name: &str) -> String {
    let msg = e.to_string();
    if msg.contains("UNIQUE constraint failed: projects.name") {
        format!("un autre projet s'appelle deja « {} »", name)
    } else {
        msg
    }
}

impl Project {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        let deps_raw: String = row.get(5)?;
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            compose_file: row.get(3)?,
            description: row.get(4)?,
            depends_on: serde_json::from_str(&deps_raw).unwrap_or_default(),
            position: row.get(6)?,
            folder_id: row.get(7)?,
            created_at: row.get(8)?,
        })
    }

    const SELECT_COLS: &'static str = "id, name, path, compose_file, description, depends_on, position, folder_id, created_at";
}

impl Database {
    pub fn get_projects(&self) -> Result<Vec<Project>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!("SELECT {} FROM projects ORDER BY position, name", Project::SELECT_COLS))
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], Project::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn create_project(
        &self,
        name: &str,
        path: &str,
        compose_file: &str,
        description: &str,
        depends_on: &[String],
    ) -> Result<Project, String> {
        let conn = self.conn();
        let deps_json = serde_json::to_string(depends_on).unwrap_or_else(|_| "[]".into());

        conn.execute(
            "INSERT INTO projects (name, path, compose_file, description, depends_on) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![name, path, compose_file, description, deps_json],
        )
        .map_err(|e| erreur_nom(e, name))?;

        conn.query_row(
            &format!("SELECT {} FROM projects WHERE name=?1", Project::SELECT_COLS),
            [name],
            Project::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn update_project(
        &self,
        id: i64,
        name: &str,
        path: &str,
        compose_file: &str,
        description: &str,
        depends_on: &[String],
    ) -> Result<Project, String> {
        let conn = self.conn();
        let deps_json = serde_json::to_string(depends_on).unwrap_or_else(|_| "[]".into());

        conn.execute(
            "UPDATE projects SET name=?1, path=?2, compose_file=?3, description=?4, depends_on=?5 WHERE id=?6",
            rusqlite::params![name, path, compose_file, description, deps_json, id],
        )
        .map_err(|e| erreur_nom(e, name))?;

        conn.query_row(
            &format!("SELECT {} FROM projects WHERE id=?1", Project::SELECT_COLS),
            [id],
            Project::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn update_project_by_name(
        &self,
        name: &str,
        path: &str,
        compose_file: &str,
        description: &str,
        depends_on: &[String],
    ) -> Result<Project, String> {
        let conn = self.conn();
        let deps_json = serde_json::to_string(depends_on).unwrap_or_else(|_| "[]".into());

        conn.execute(
            "UPDATE projects SET path=?1, compose_file=?2, description=?3, depends_on=?4 WHERE name=?5",
            rusqlite::params![path, compose_file, description, deps_json, name],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            &format!("SELECT {} FROM projects WHERE name=?1", Project::SELECT_COLS),
            [name],
            Project::from_row,
        )
        .map_err(|e| e.to_string())
    }

    /// Supprime le projet ET toutes ses donnees. Les tables liees referencent
    /// le projet par NOM (pas de FK cascade), on les nettoie donc explicitement.
    /// note_folders/note_files sont supprimes en cascade via leur FK sur
    /// note_folders, mais on les cible aussi par nom pour les fichiers a la racine.
    pub fn delete_project(&self, id: i64) -> Result<(), String> {
        let conn = self.conn();
        let name: String = conn
            .query_row("SELECT name FROM projects WHERE id=?1", [id], |r| r.get(0))
            .map_err(|e| e.to_string())?;

        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        for table in PROJECT_SCOPED_TABLES {
            tx.execute(&format!("DELETE FROM {} WHERE project=?1", table), [&name])
                .map_err(|e| format!("{}: {}", table, e))?;
        }
        tx.execute("DELETE FROM projects WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    /// Nom stocke en DB pour un chemin donne (identite stable d'un projet,
    /// utilise quand le nom affiche a derive du nom stocke).
    pub fn get_project_name_by_path(&self, path: &str) -> Result<String, String> {
        let conn = self.conn();
        conn.query_row("SELECT name FROM projects WHERE path=?1", [path], |r| r.get(0))
            .map_err(|e| e.to_string())
    }

    pub fn get_project_by_name(&self, name: &str) -> Result<Project, String> {
        let conn = self.conn();
        conn.query_row(
            &format!("SELECT {} FROM projects WHERE name=?1", Project::SELECT_COLS),
            [name],
            Project::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn get_project_by_id(&self, id: i64) -> Result<Project, String> {
        let conn = self.conn();
        conn.query_row(
            &format!("SELECT {} FROM projects WHERE id=?1", Project::SELECT_COLS),
            [id],
            Project::from_row,
        )
        .map_err(|e| e.to_string())
    }

    /// Renomme un projet et met a jour toutes les tables liees. Auto-reparant :
    /// si `old_name` ne correspond a aucune ligne (derive nom affiche <-> DB),
    /// retrouve la ligne par `path_hint` et utilise SON nom reel comme source.
    pub fn rename_project(&self, old_name: &str, new_name: &str, path_hint: Option<&str>) -> Result<(), String> {
        let conn = self.conn();

        // Nom reel en DB : par nom si trouve, sinon par chemin (identite stable)
        let db_old: Option<String> = conn
            .query_row("SELECT name FROM projects WHERE name=?1", [old_name], |r| r.get(0))
            .ok()
            .or_else(|| {
                path_hint.and_then(|p| {
                    conn.query_row("SELECT name FROM projects WHERE path=?1", [p], |r| r.get(0)).ok()
                })
            });
        let db_old = match db_old {
            Some(n) => n,
            None => return Err(format!("projet introuvable en base (nom affiche: {})", old_name)),
        };

        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute("UPDATE projects SET name=?1 WHERE name=?2", rusqlite::params![new_name, db_old])
            .map_err(|e| erreur_nom(e, new_name))?;

        // Toutes les tables referençant le projet par nom
        for table in PROJECT_SCOPED_TABLES {
            tx.execute(&format!("UPDATE {} SET project=?1 WHERE project=?2", table),
                       rusqlite::params![new_name, db_old])
                .map_err(|e| format!("{}: {}", table, e))?;
        }

        // depends_on (JSON array) des autres projets
        let mut stmt = tx.prepare("SELECT id, depends_on FROM projects WHERE depends_on LIKE ?1")
            .map_err(|e| e.to_string())?;
        let pattern = format!("%\"{}%", db_old);
        let rows: Vec<(i64, String)> = stmt.query_map([&pattern], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        for (id, deps_raw) in &rows {
            if let Ok(mut deps) = serde_json::from_str::<Vec<String>>(deps_raw) {
                for dep in &mut deps {
                    if *dep == db_old { *dep = new_name.to_string(); }
                }
                let new_json = serde_json::to_string(&deps).unwrap_or_else(|_| "[]".into());
                tx.execute("UPDATE projects SET depends_on=?1 WHERE id=?2", rusqlite::params![new_json, id])
                    .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().map_err(|e| e.to_string())
    }

    pub fn move_project_to_folder(&self, project_name: &str, folder_id: Option<i64>) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE projects SET folder_id=?1 WHERE name=?2",
            rusqlite::params![folder_id, project_name],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn reorder_projects(&self, names: &[String]) -> Result<(), String> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        for (i, name) in names.iter().enumerate() {
            tx.execute(
                "UPDATE projects SET position=?1 WHERE name=?2",
                rusqlite::params![i as i32, name],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::new(":memory:").unwrap()
    }

    #[test]
    fn test_create_and_get_project() {
        let db = test_db();
        let p = db.create_project("test", "/tmp/test", "docker-compose.yml", "A test project", &[]).unwrap();
        assert_eq!(p.name, "test");
        assert_eq!(p.path, "/tmp/test");

        let fetched = db.get_project_by_name("test").unwrap();
        assert_eq!(fetched.id, p.id);
    }

    #[test]
    fn test_update_project() {
        let db = test_db();
        let p = db.create_project("proj", "/a", "", "", &[]).unwrap();
        let updated = db.update_project(p.id, "proj", "/b", "compose.yml", "Updated", &["dep1".into()]).unwrap();
        assert_eq!(updated.path, "/b");
        assert_eq!(updated.description, "Updated");
        assert_eq!(updated.depends_on, vec!["dep1"]);
    }

    #[test]
    fn test_delete_project() {
        let db = test_db();
        let p = db.create_project("todelete", "/x", "", "", &[]).unwrap();
        db.delete_project(p.id).unwrap();
        assert!(db.get_project_by_name("todelete").is_err());
    }

    #[test]
    fn test_reorder_projects() {
        let db = test_db();
        db.create_project("a", "/a", "", "", &[]).unwrap();
        db.create_project("b", "/b", "", "", &[]).unwrap();
        db.create_project("c", "/c", "", "", &[]).unwrap();

        db.reorder_projects(&["c".into(), "a".into(), "b".into()]).unwrap();

        let projects = db.get_projects().unwrap();
        assert_eq!(projects[0].name, "c");
        assert_eq!(projects[1].name, "a");
        assert_eq!(projects[2].name, "b");
    }

    /// Renommer vers un nom deja pris doit DIRE ce qui se passe. SQLite rendait
    /// « UNIQUE constraint failed: projects.name », affiche tel quel a l'utilisateur.
    #[test]
    fn test_rename_project_to_existing_name() {
        let db = test_db();
        db.create_project("alpha", "/a", "", "", &[]).unwrap();
        db.create_project("beta", "/b", "", "", &[]).unwrap();

        let err = db.rename_project("alpha", "beta", Some("/a")).unwrap_err();
        assert!(err.contains("beta"), "le message doit nommer le projet en cause: {}", err);
        assert!(!err.contains("UNIQUE"), "message SQLite brut remonte: {}", err);

        // Et rien n'a bouge en base.
        assert_eq!(db.get_project_by_name("alpha").unwrap().path, "/a");
    }

    #[test]
    fn test_create_project_with_existing_name() {
        let db = test_db();
        db.create_project("alpha", "/a", "", "", &[]).unwrap();
        let err = db.create_project("alpha", "/autre", "", "", &[]).unwrap_err();
        assert!(err.contains("alpha"), "le message doit nommer le projet en cause: {}", err);
        assert!(!err.contains("UNIQUE"), "message SQLite brut remonte: {}", err);
    }

    #[test]
    fn test_rename_project_renames_scoped_rows() {
        let db = test_db();
        db.create_project("alpha", "/a", "", "", &[]).unwrap();
        db.create_todo("alpha", "une tache").unwrap();

        db.rename_project("alpha", "omega", Some("/a")).unwrap();

        assert!(db.get_project_by_name("omega").is_ok());
        assert_eq!(db.get_todos("omega").unwrap().len(), 1);
        assert_eq!(db.get_todos("alpha").unwrap().len(), 0);
    }
}
