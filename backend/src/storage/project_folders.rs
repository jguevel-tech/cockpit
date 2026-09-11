use super::db::Database;
use serde::{Deserialize, Serialize};

/// Un dossier de la barre laterale. `parent_id` a NULL = premier niveau ;
/// l'imbrication n'a pas de limite de profondeur (issue #2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFolder {
    pub id: i64,
    pub name: String,
    pub position: i32,
    pub parent_id: Option<i64>,
}

/// Garde-fou contre une boucle deja presente en base : sans plafond, remonter la chaine
/// des parents tournerait indefiniment.
const PROFONDEUR_MAX: usize = 1000;

impl ProjectFolder {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            position: row.get(2)?,
            parent_id: row.get(3)?,
        })
    }
}

impl Database {
    pub fn get_project_folders(&self) -> Result<Vec<ProjectFolder>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, position, parent_id FROM project_folders ORDER BY position, name",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], ProjectFolder::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    fn get_project_folder(&self, id: i64) -> Result<Option<ProjectFolder>, String> {
        let conn = self.conn();
        match conn.query_row(
            "SELECT id, name, position, parent_id FROM project_folders WHERE id=?1",
            [id],
            ProjectFolder::from_row,
        ) {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Prochaine position DANS LA FRATRIE : les positions sont locales a un parent, sinon le
    /// second niveau melangerait son ordre avec celui de la racine.
    /// `parent_id IS ?1` couvre les deux cas (valeur ou NULL) — `=` ne verrait jamais NULL.
    fn next_folder_position(&self, parent_id: Option<i64>) -> i32 {
        let conn = self.conn();
        conn.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM project_folders WHERE parent_id IS ?1",
            [parent_id],
            |row| row.get::<_, i32>(0),
        )
        .unwrap_or(-1)
            + 1
    }

    /// Le dossier `parent` est-il `id` lui-meme ou un de ses descendants ?
    /// On remonte la chaine des parents depuis `parent` : si on retombe sur `id`, deplacer
    /// `id` sous `parent` fabriquerait une boucle (branche detachee de la racine, rendu
    /// recursif sans fin).
    fn est_descendant(&self, id: i64, parent: i64) -> Result<bool, String> {
        let mut courant = Some(parent);
        let mut pas = 0;
        while let Some(c) = courant {
            if c == id {
                return Ok(true);
            }
            pas += 1;
            if pas > PROFONDEUR_MAX {
                return Err("la hierarchie des dossiers contient une boucle".to_string());
            }
            courant = match self.get_project_folder(c)? {
                Some(f) => f.parent_id,
                None => None,
            };
        }
        Ok(false)
    }

    /// Nombre d'enfants DIRECTS d'un dossier : (sous-dossiers, projets).
    /// Sert a la garde de suppression — un dossier ne se supprime que vide.
    pub fn project_folder_children(&self, id: i64) -> Result<(i64, i64), String> {
        let conn = self.conn();
        let dossiers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM project_folders WHERE parent_id=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let projets: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE folder_id=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok((dossiers, projets))
    }

    pub fn create_project_folder(
        &self,
        name: &str,
        parent_id: Option<i64>,
    ) -> Result<ProjectFolder, String> {
        if let Some(pid) = parent_id {
            if self.get_project_folder(pid)?.is_none() {
                return Err("le dossier parent n'existe plus".to_string());
            }
        }
        let pos = self.next_folder_position(parent_id);
        let conn = self.conn();
        conn.execute(
            "INSERT INTO project_folders (name, position, parent_id) VALUES (?1, ?2, ?3)",
            rusqlite::params![name, pos, parent_id],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(
            "SELECT id, name, position, parent_id FROM project_folders WHERE id=?1",
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

    /// Deplace un dossier sous un autre (ou a la racine avec `None`).
    /// Refuse explicitement les boucles : un dossier ne peut pas devenir son propre
    /// descendant. Le message remonte jusqu'au toast, pas de garde muette.
    pub fn move_project_folder(&self, id: i64, parent_id: Option<i64>) -> Result<(), String> {
        let dossier = self
            .get_project_folder(id)?
            .ok_or_else(|| "ce dossier n'existe plus".to_string())?;
        if dossier.parent_id == parent_id {
            return Ok(());
        }
        if let Some(pid) = parent_id {
            if pid == id {
                return Err("un dossier ne peut pas se contenir lui-meme".to_string());
            }
            if self.get_project_folder(pid)?.is_none() {
                return Err("le dossier de destination n'existe plus".to_string());
            }
            if self.est_descendant(id, pid)? {
                return Err(
                    "on ne peut pas ranger un dossier dans un de ses sous-dossiers".to_string(),
                );
            }
        }
        let pos = self.next_folder_position(parent_id);
        self.conn()
            .execute(
                "UPDATE project_folders SET parent_id=?1, position=?2 WHERE id=?3",
                rusqlite::params![parent_id, pos, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Ne supprime QUE un dossier vide — ni projet, ni sous-dossier.
    /// Detacher en silence ce qu'il contient serait une surprise, et avec l'imbrication ce
    /// serait pire : une branche entiere disparaitrait derriere un dossier replie.
    /// L'interface controle d'abord pour afficher un message traduit ; ce refus-ci est le
    /// filet, il vaut aussi pour tout autre appelant.
    pub fn delete_project_folder(&self, id: i64) -> Result<(), String> {
        let (sous_dossiers, projets) = self.project_folder_children(id)?;
        if sous_dossiers > 0 || projets > 0 {
            return Err(format!(
                "ce dossier n'est pas vide : {} sous-dossier(s) et {} projet(s) a deplacer d'abord",
                sous_dossiers, projets
            ));
        }
        self.conn()
            .execute("DELETE FROM project_folders WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Reordonne UNE fratrie : l'appelant envoie les ids d'un meme parent, dans l'ordre
    /// voulu. Les positions sont locales au parent (voir next_folder_position).
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
        let f = db.create_project_folder("DevOps", None).unwrap();
        assert_eq!(f.name, "DevOps");
        assert_eq!(f.parent_id, None);

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
        let f = db.create_project_folder("Web", None).unwrap();
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
    fn test_delete_folder_refuse_si_projets() {
        let db = Database::new(":memory:").unwrap();
        let f = db.create_project_folder("Temp", None).unwrap();
        db.create_project("proj1", "/p1", "", "", &[]).unwrap();
        db.move_project_to_folder("proj1", Some(f.id)).unwrap();

        let err = db.delete_project_folder(f.id).unwrap_err();
        assert!(err.contains("1 projet"), "message inattendu : {err}");
        // Le dossier est toujours la, le projet toujours dedans : rien en silence.
        assert_eq!(db.get_project_folders().unwrap().len(), 1);
        assert_eq!(
            db.get_project_by_name("proj1").unwrap().folder_id,
            Some(f.id)
        );
    }

    #[test]
    fn test_delete_folder_refuse_si_sous_dossier() {
        let db = Database::new(":memory:").unwrap();
        let parent = db.create_project_folder("Core", None).unwrap();
        let enfant = db.create_project_folder("Back", Some(parent.id)).unwrap();

        let err = db.delete_project_folder(parent.id).unwrap_err();
        assert!(err.contains("1 sous-dossier"), "message inattendu : {err}");
        assert_eq!(db.get_project_folders().unwrap().len(), 2);

        // Vider d'abord, puis supprimer : le chemin normal reste ouvert.
        db.delete_project_folder(enfant.id).unwrap();
        db.delete_project_folder(parent.id).unwrap();
        assert!(db.get_project_folders().unwrap().is_empty());
    }

    #[test]
    fn test_imbrication_profonde() {
        let db = Database::new(":memory:").unwrap();
        let mut parent = None;
        let mut ids = Vec::new();
        for i in 0..6 {
            let f = db
                .create_project_folder(&format!("niveau{i}"), parent)
                .unwrap();
            assert_eq!(f.parent_id, parent);
            ids.push(f.id);
            parent = Some(f.id);
        }
        let folders = db.get_project_folders().unwrap();
        assert_eq!(folders.len(), 6);
        // Chaque dossier pointe sur le precedent : la chaine est bien conservee.
        for (i, id) in ids.iter().enumerate() {
            let f = folders.iter().find(|f| f.id == *id).unwrap();
            assert_eq!(f.parent_id, if i == 0 { None } else { Some(ids[i - 1]) });
        }
    }

    #[test]
    fn test_move_folder_refuse_les_boucles() {
        let db = Database::new(":memory:").unwrap();
        let a = db.create_project_folder("A", None).unwrap();
        let b = db.create_project_folder("B", Some(a.id)).unwrap();
        let c = db.create_project_folder("C", Some(b.id)).unwrap();

        // Dans soi-meme
        let err = db.move_project_folder(a.id, Some(a.id)).unwrap_err();
        assert!(err.contains("lui-meme"), "message inattendu : {err}");
        // Dans son enfant direct
        let err = db.move_project_folder(a.id, Some(b.id)).unwrap_err();
        assert!(err.contains("sous-dossiers"), "message inattendu : {err}");
        // Dans son petit-enfant
        let err = db.move_project_folder(a.id, Some(c.id)).unwrap_err();
        assert!(err.contains("sous-dossiers"), "message inattendu : {err}");
        // La hierarchie n'a pas bouge
        let folders = db.get_project_folders().unwrap();
        assert_eq!(folders.iter().find(|f| f.id == a.id).unwrap().parent_id, None);

        // Dans l'autre sens, c'est legitime : C remonte a la racine, puis A va dans C.
        db.move_project_folder(c.id, None).unwrap();
        db.move_project_folder(a.id, Some(c.id)).unwrap();
        let folders = db.get_project_folders().unwrap();
        assert_eq!(
            folders.iter().find(|f| f.id == a.id).unwrap().parent_id,
            Some(c.id)
        );
    }

    #[test]
    fn test_move_folder_destination_inconnue() {
        let db = Database::new(":memory:").unwrap();
        let a = db.create_project_folder("A", None).unwrap();
        let err = db.move_project_folder(a.id, Some(9999)).unwrap_err();
        assert!(err.contains("n'existe plus"), "message inattendu : {err}");
        let err = db.move_project_folder(9999, None).unwrap_err();
        assert!(err.contains("n'existe plus"), "message inattendu : {err}");
        let err = db.create_project_folder("orphelin", Some(9999)).unwrap_err();
        assert!(err.contains("n'existe plus"), "message inattendu : {err}");
    }

    #[test]
    fn test_positions_par_fratrie() {
        let db = Database::new(":memory:").unwrap();
        // Racine : trois dossiers -> positions 0,1,2 (l'ancien code les laissait tous a 0,
        // l'ordre retombait alors sur le nom et reorder_project_folders etait inoperant).
        let r1 = db.create_project_folder("r1", None).unwrap();
        let r2 = db.create_project_folder("r2", None).unwrap();
        let r3 = db.create_project_folder("r3", None).unwrap();
        assert_eq!((r1.position, r2.position, r3.position), (0, 1, 2));

        // Sous r1 : la numerotation repart a 0, independamment de la racine.
        let a = db.create_project_folder("a", Some(r1.id)).unwrap();
        let b = db.create_project_folder("b", Some(r1.id)).unwrap();
        assert_eq!((a.position, b.position), (0, 1));

        // Reordonner une fratrie ne touche pas l'autre.
        db.reorder_project_folders(&[b.id, a.id]).unwrap();
        let folders = db.get_project_folders().unwrap();
        let pos = |id: i64| folders.iter().find(|f| f.id == id).unwrap().position;
        assert_eq!((pos(b.id), pos(a.id)), (0, 1));
        assert_eq!((pos(r1.id), pos(r2.id), pos(r3.id)), (0, 1, 2));

        // Un dossier deplace arrive EN FIN de sa nouvelle fratrie.
        db.move_project_folder(r3.id, Some(r1.id)).unwrap();
        let folders = db.get_project_folders().unwrap();
        let pos = |id: i64| folders.iter().find(|f| f.id == id).unwrap().position;
        assert_eq!(pos(r3.id), 2);
    }

    #[test]
    fn test_compte_enfants_directs() {
        let db = Database::new(":memory:").unwrap();
        let parent = db.create_project_folder("Core", None).unwrap();
        let enfant = db.create_project_folder("Back", Some(parent.id)).unwrap();
        db.create_project("api", "/api", "", "", &[]).unwrap();
        db.move_project_to_folder("api", Some(enfant.id)).unwrap();

        // Le compte est DIRECT : le projet est dans l'enfant, pas dans le parent.
        assert_eq!(db.project_folder_children(parent.id).unwrap(), (1, 0));
        assert_eq!(db.project_folder_children(enfant.id).unwrap(), (0, 1));
    }
}
