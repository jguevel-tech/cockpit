use super::db::Database;
use std::collections::HashMap;

impl Database {
    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn();
        conn.query_row("SELECT value FROM settings WHERE key=?1", [key], |row| row.get(0))
            .ok()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            rusqlite::params![key, value],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<HashMap<String, String>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_settings_set_get_upsert() {
        let db = Database::new(":memory:").unwrap();
        assert!(db.get_setting("k").is_none());
        db.set_setting("k", "v1").unwrap();
        assert_eq!(db.get_setting("k").unwrap(), "v1");
        db.set_setting("k", "v2").unwrap();
        assert_eq!(db.get_setting("k").unwrap(), "v2");

        // On compte NOTRE cle, pas la table : le demarrage y range ses propres reglages, et
        // un total fige transforme chaque ajout legitime en essai casse.
        let miennes = db.get_all_settings().unwrap();
        let pour_k = miennes.iter().filter(|(cle, _)| cle.as_str() == "k").count();
        assert_eq!(pour_k, 1, "une seule ligne pour `k`");
    }
}
