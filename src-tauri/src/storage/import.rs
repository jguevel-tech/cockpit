//! Import depuis l'ancienne base Go. Tout est fait dans UNE transaction :
//! un echec au milieu ne laisse jamais d'import partiel (la garde "destination
//! non vide" empecherait ensuite de rejouer proprement).

use super::db::Database;
use std::collections::HashMap;

pub fn import_from(db: &Database, source_path: &str) -> Result<String, String> {
    let source = std::path::Path::new(source_path);
    if !source.exists() {
        return Err(format!("file not found: {}", source_path));
    }

    let src_conn = rusqlite::Connection::open(source).map_err(|e| e.to_string())?;

    let conn = db.conn();
    let dst_projects: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap_or(0);
    if dst_projects > 0 {
        return Err("La base de destination contient deja des projets. Importation annulee pour eviter les doublons.".into());
    }

    // Lecture complete de la source avant d'ecrire quoi que ce soit
    let mut stmt = src_conn.prepare("SELECT name, path, compose_file, description, depends_on, position, created_at FROM projects ORDER BY id").map_err(|e| e.to_string())?;
    let projects: Vec<(String, String, String, String, String, i32, String)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt = src_conn.prepare("SELECT project, content, created_at, updated_at FROM notes ORDER BY id").map_err(|e| e.to_string())?;
    let notes: Vec<(String, String, String, String)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt = src_conn.prepare("SELECT id, project, name, position FROM note_folders WHERE parent_id IS NULL ORDER BY id").map_err(|e| e.to_string())?;
    let root_folders: Vec<(i64, String, String, i32)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt = src_conn.prepare("SELECT id, project, parent_id, name, position FROM note_folders WHERE parent_id IS NOT NULL ORDER BY id").map_err(|e| e.to_string())?;
    let child_folders: Vec<(i64, String, i64, String, i32)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt = src_conn.prepare("SELECT project, folder_id, name, content, position, updated_at FROM note_files ORDER BY id").map_err(|e| e.to_string())?;
    let files: Vec<(String, Option<i64>, String, String, i32, String)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt = src_conn.prepare("SELECT project, text, done, position, created_at FROM todos ORDER BY id").map_err(|e| e.to_string())?;
    let todos: Vec<(String, String, i32, i32, String)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt = src_conn.prepare("SELECT project, label, url, position FROM urls ORDER BY id").map_err(|e| e.to_string())?;
    let urls: Vec<(String, String, String, i32)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // Ecriture atomique
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    for (name, path, cf, desc, deps, pos, created) in &projects {
        tx.execute(
            "INSERT INTO projects (name, path, compose_file, description, depends_on, position, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            rusqlite::params![name, path, cf, desc, deps, pos, created],
        ).map_err(|e| format!("project {}: {}", name, e))?;
    }

    for (project, content, ca, ua) in &notes {
        tx.execute(
            "INSERT INTO notes (project, content, created_at, updated_at) VALUES (?1,?2,?3,?4)",
            rusqlite::params![project, content, ca, ua],
        ).map_err(|e| e.to_string())?;
    }

    // Dossiers : mapping ancien id -> nouvel id (racines puis enfants)
    let mut old_to_new_folder: HashMap<i64, i64> = HashMap::new();
    for (old_id, project, name, pos) in &root_folders {
        tx.execute(
            "INSERT INTO note_folders (project, parent_id, name, position) VALUES (?1, NULL, ?2, ?3)",
            rusqlite::params![project, name, pos],
        ).map_err(|e| e.to_string())?;
        old_to_new_folder.insert(*old_id, tx.last_insert_rowid());
    }
    for (old_id, project, old_parent, name, pos) in &child_folders {
        let new_parent = old_to_new_folder.get(old_parent).copied();
        tx.execute(
            "INSERT INTO note_folders (project, parent_id, name, position) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project, new_parent, name, pos],
        ).map_err(|e| e.to_string())?;
        old_to_new_folder.insert(*old_id, tx.last_insert_rowid());
    }

    for (project, old_folder_id, name, content, pos, ua) in &files {
        let new_folder_id = old_folder_id.and_then(|id| old_to_new_folder.get(&id).copied());
        tx.execute(
            "INSERT INTO note_files (project, folder_id, name, content, position, updated_at) VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![project, new_folder_id, name, content, pos, ua],
        ).map_err(|e| e.to_string())?;
    }

    for (project, text, done, pos, ca) in &todos {
        tx.execute(
            "INSERT INTO todos (project, text, done, position, created_at) VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![project, text, done, pos, ca],
        ).map_err(|e| e.to_string())?;
    }

    for (project, label, url, pos) in &urls {
        tx.execute(
            "INSERT INTO urls (project, label, url, position) VALUES (?1,?2,?3,?4)",
            rusqlite::params![project, label, url, pos],
        ).map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(format!(
        "Import termine: {} projets, {} notes, {} dossiers, {} fichiers, {} todos, {} urls",
        projects.len(), notes.len(), root_folders.len() + child_folders.len(),
        files.len(), todos.len(), urls.len()
    ))
}
