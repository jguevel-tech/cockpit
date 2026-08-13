use super::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitemapPair {
    pub id: i64,
    pub project: String,
    pub label: String,
    pub sitemap_ref_url: String,
    pub sitemap_check_url: String,
    pub ref_query: String,
    pub check_query: String,
    pub position: i32,
    pub limit_urls: Option<i64>,
}

impl SitemapPair {
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            project: row.get(1)?,
            label: row.get(2)?,
            sitemap_ref_url: row.get(3)?,
            sitemap_check_url: row.get(4)?,
            ref_query: row.get(5)?,
            check_query: row.get(6)?,
            position: row.get(7)?,
            limit_urls: row.get(8)?,
        })
    }

    const COLS: &'static str = "id, project, label, sitemap_ref_url, sitemap_check_url, ref_query, check_query, position, limit_urls";
}

impl Database {
    pub fn get_sitemap_pairs(&self, project: &str) -> Result<Vec<SitemapPair>, String> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM sitemap_pairs WHERE project=?1 ORDER BY position, id",
                SitemapPair::COLS
            ))
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([project], SitemapPair::from_row)
            .map_err(|e| e.to_string())?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn get_sitemap_pair(&self, id: i64) -> Result<SitemapPair, String> {
        let conn = self.conn();
        conn.query_row(
            &format!("SELECT {} FROM sitemap_pairs WHERE id=?1", SitemapPair::COLS),
            [id],
            SitemapPair::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn create_sitemap_pair(
        &self,
        project: &str,
        label: &str,
        sitemap_ref_url: &str,
        sitemap_check_url: &str,
        ref_query: &str,
        check_query: &str,
        limit_urls: Option<i64>,
    ) -> Result<SitemapPair, String> {
        let pos = self.next_position("sitemap_pairs", "project", &project);
        let conn = self.conn();
        conn.execute(
            "INSERT INTO sitemap_pairs (project, label, sitemap_ref_url, sitemap_check_url, ref_query, check_query, position, limit_urls) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![project, label, sitemap_ref_url, sitemap_check_url, ref_query, check_query, pos, limit_urls],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        conn.query_row(
            &format!("SELECT {} FROM sitemap_pairs WHERE id=?1", SitemapPair::COLS),
            [id],
            SitemapPair::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn update_sitemap_pair(
        &self,
        id: i64,
        label: &str,
        sitemap_ref_url: &str,
        sitemap_check_url: &str,
        ref_query: &str,
        check_query: &str,
        limit_urls: Option<i64>,
    ) -> Result<SitemapPair, String> {
        let conn = self.conn();
        conn.execute(
            "UPDATE sitemap_pairs SET label=?1, sitemap_ref_url=?2, sitemap_check_url=?3, ref_query=?4, check_query=?5, limit_urls=?6 WHERE id=?7",
            rusqlite::params![label, sitemap_ref_url, sitemap_check_url, ref_query, check_query, limit_urls, id],
        )
        .map_err(|e| e.to_string())?;

        conn.query_row(
            &format!("SELECT {} FROM sitemap_pairs WHERE id=?1", SitemapPair::COLS),
            [id],
            SitemapPair::from_row,
        )
        .map_err(|e| e.to_string())
    }

    pub fn delete_sitemap_pair(&self, id: i64) -> Result<(), String> {
        self.conn()
            .execute("DELETE FROM sitemap_pairs WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::db::Database;

    #[test]
    fn test_sitemap_pair_crud() {
        let db = Database::new(":memory:").unwrap();
        let p = db
            .create_sitemap_pair(
                "proj",
                "Blog",
                "https://prod.com/sitemap.xml",
                "https://staging.com/sitemap.xml",
                "",
                "?new=1",
                None,
            )
            .unwrap();
        assert_eq!(p.label, "Blog");
        assert_eq!(p.check_query, "?new=1");
        assert_eq!(p.limit_urls, None);

        let updated = db
            .update_sitemap_pair(
                p.id,
                "Blog v2",
                "https://prod.com/sitemap.xml",
                "https://prod.com/sitemap.xml",
                "",
                "?v=2",
                Some(10),
            )
            .unwrap();
        assert_eq!(updated.label, "Blog v2");
        assert_eq!(updated.sitemap_check_url, "https://prod.com/sitemap.xml");
        assert_eq!(updated.limit_urls, Some(10));

        let fetched = db.get_sitemap_pair(p.id).unwrap();
        assert_eq!(fetched.id, p.id);
        assert_eq!(fetched.limit_urls, Some(10));

        let all = db.get_sitemap_pairs("proj").unwrap();
        assert_eq!(all.len(), 1);

        db.delete_sitemap_pair(p.id).unwrap();
        let empty = db.get_sitemap_pairs("proj").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_sitemap_pair_scoped_by_project() {
        let db = Database::new(":memory:").unwrap();
        db.create_sitemap_pair("p1", "a", "u1", "u1", "", "", None).unwrap();
        db.create_sitemap_pair("p2", "b", "u2", "u2", "", "", None).unwrap();

        let p1 = db.get_sitemap_pairs("p1").unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].project, "p1");
    }
}
