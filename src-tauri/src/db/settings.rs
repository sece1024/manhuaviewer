//! 设置（settings 表）与全局统计相关的查询。

use rusqlite::Result;

use super::{log_and_skip, Database};

impl Database {
    pub fn get_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let settings = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(log_and_skip)
            .collect();
        Ok(settings)
    }

    pub fn update_settings(
        &self,
        settings: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        for (key, value) in settings {
            tx.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
                (key, value),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<String> {
        self.conn()?
            .query_row("SELECT value FROM settings WHERE key = ?", [key], |row| {
                row.get(0)
            })
    }

    // Stats
    pub fn get_stats(&self) -> Result<serde_json::Value> {
        let conn = self.conn()?;
        let total_archives: i64 =
            conn.query_row("SELECT COUNT(*) FROM archives", [], |row| row.get(0))?;
        let total_pages: i64 = conn.query_row(
            "SELECT COALESCE(SUM(page_count), 0) FROM archives",
            [],
            |row| row.get(0),
        )?;
        let total_size: i64 = conn.query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM archives",
            [],
            |row| row.get(0),
        )?;
        let total_tags: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        let total_categories: i64 =
            conn.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?;
        let history_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;

        Ok(serde_json::json!({
            "total_archives": total_archives,
            "total_pages": total_pages,
            "total_size": total_size,
            "total_tags": total_tags,
            "total_categories": total_categories,
            "history_count": history_count
        }))
    }
}
