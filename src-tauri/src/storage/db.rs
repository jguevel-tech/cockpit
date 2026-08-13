use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // Un panic dans un thread tenant le lock ne doit pas condamner tous les
        // acces DB suivants : on recupere le guard empoisonne (la connexion
        // SQLite reste valide, les transactions non commitees sont rollback).
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Reordonne des lignes par position a partir d'une liste d'IDs.
    /// `table` et `id_col` sont des noms de table/colonne connus (pas d'input utilisateur).
    pub fn reorder_by_ids(&self, table: &str, id_col: &str, ids: &[i64]) -> Result<(), String> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        let sql = format!("UPDATE {} SET position=?1 WHERE {}=?2", table, id_col);
        for (i, id) in ids.iter().enumerate() {
            tx.execute(&sql, rusqlite::params![i as i32, id])
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// Retourne la prochaine position disponible pour un insert.
    pub fn next_position(&self, table: &str, filter_col: &str, filter_val: &dyn rusqlite::types::ToSql) -> i32 {
        let conn = self.conn();
        let sql = format!(
            "SELECT COALESCE(MAX(position), -1) FROM {} WHERE {}=?1",
            table, filter_col
        );
        conn.query_row(&sql, [filter_val], |row| row.get::<_, i32>(0))
            .unwrap_or(-1)
            + 1
    }

    /// Variante pour filtre IS NULL (pas de valeur parente).
    pub fn next_position_null(&self, table: &str, filter_col: &str) -> i32 {
        let conn = self.conn();
        let sql = format!(
            "SELECT COALESCE(MAX(position), -1) FROM {} WHERE {} IS NULL",
            table, filter_col
        );
        conn.query_row(&sql, [], |row| row.get::<_, i32>(0))
            .unwrap_or(-1)
            + 1
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                compose_file TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                depends_on TEXT NOT NULL DEFAULT '[]',
                position INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS note_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                parent_id INTEGER DEFAULT NULL,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (parent_id) REFERENCES note_folders(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS note_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                folder_id INTEGER DEFAULT NULL,
                name TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                position INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (folder_id) REFERENCES note_folders(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                text TEXT NOT NULL,
                done INTEGER NOT NULL DEFAULT 0,
                position INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS urls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                label TEXT NOT NULL,
                url TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_notes_project ON notes(project);
            CREATE INDEX IF NOT EXISTS idx_note_folders_project ON note_folders(project);
            CREATE INDEX IF NOT EXISTS idx_note_files_project ON note_files(project);
            CREATE INDEX IF NOT EXISTS idx_note_files_folder ON note_files(folder_id);
            CREATE INDEX IF NOT EXISTS idx_todos_project ON todos(project);
            CREATE INDEX IF NOT EXISTS idx_urls_project ON urls(project);
            ",
        )?;

        // Migration: add position column if missing (same as Go)
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN position INTEGER NOT NULL DEFAULT 0", []);

        // Migration: project_folders table + folder_id on projects
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN folder_id INTEGER DEFAULT NULL REFERENCES project_folders(id) ON DELETE SET NULL", []);
        let _ = conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_projects_folder ON projects(folder_id);");

        // Migration: drop removed AI orchestration tables if they exist.
        conn.execute_batch(
            "DROP TABLE IF EXISTS ticket_comments;
             DROP TABLE IF EXISTS tickets;
             DROP TABLE IF EXISTS team_templates;
             DROP TABLE IF EXISTS agent_roles;",
        )?;

        // Migration: sitemap pairs for HTML diff checks
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sitemap_pairs (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                project           TEXT NOT NULL,
                label             TEXT NOT NULL,
                sitemap_ref_url   TEXT NOT NULL,
                sitemap_check_url TEXT NOT NULL,
                ref_query         TEXT NOT NULL DEFAULT '',
                check_query       TEXT NOT NULL DEFAULT '',
                position          INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_sitemap_pairs_project ON sitemap_pairs(project);",
        )?;
        // Migration: add limit_urls column if missing (nullable, NULL = no limit)
        let _ = conn.execute(
            "ALTER TABLE sitemap_pairs ADD COLUMN limit_urls INTEGER DEFAULT NULL",
            [],
        );

        // Migration: settings key/value + recordings (enregistrement de reunions)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS recordings (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                project       TEXT NOT NULL,
                started_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_secs INTEGER NOT NULL DEFAULT 0,
                state         TEXT NOT NULL DEFAULT 'recording',
                error         TEXT DEFAULT NULL,
                dir           TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_recordings_project ON recordings(project);",
        )?;
        // Migration: prompt de resume par projet (NULL = prompt global)
        let _ = conn.execute(
            "ALTER TABLE projects ADD COLUMN summary_prompt TEXT DEFAULT NULL",
            [],
        );

        // Migration: terminaux persistants (sessions tmux)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS terminals (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                project    TEXT NOT NULL,
                name       TEXT NOT NULL DEFAULT '',
                tmux_name  TEXT NOT NULL DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_terminals_project ON terminals(project);",
        )?;

        // Migration: historique de commandes (autosuggestion + Ctrl+R)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS command_history (
                command TEXT PRIMARY KEY,
                project TEXT NOT NULL,
                ts      INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_command_history_ts ON command_history(ts);",
        )?;

        // Migration: noms personnalises des sessions Claude Code
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS claude_session_names (
                session_id TEXT PRIMARY KEY,
                name       TEXT NOT NULL
            );",
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_init_and_migrate() {
        let db = Database::new(":memory:").unwrap();
        let conn = db.conn();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"projects".to_string()));
        assert!(tables.contains(&"notes".to_string()));
        assert!(tables.contains(&"note_folders".to_string()));
        assert!(tables.contains(&"note_files".to_string()));
        assert!(tables.contains(&"todos".to_string()));
        assert!(tables.contains(&"urls".to_string()));
    }
}
