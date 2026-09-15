//! 书签（bookmarks 表）相关的查询。

use rusqlite::Result;

use super::{log_and_skip, Database};

impl Database {
    /// 某档案的全部书签页码（升序）。
    pub fn list_bookmarks(&self, archive_id: i64) -> Result<Vec<i64>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT page_index FROM bookmarks WHERE archive_id = ? ORDER BY page_index")?;
        let rows = stmt.query_map([archive_id], |row| row.get::<_, i64>(0))?;
        Ok(rows.filter_map(log_and_skip).collect())
    }

    /// 添加书签（同一页重复添加会被 UNIQUE 忽略）。
    pub fn add_bookmark(&self, archive_id: i64, page_index: i64) -> Result<usize> {
        self.conn()?.execute(
            "INSERT OR IGNORE INTO bookmarks (archive_id, page_index) VALUES (?, ?)",
            (archive_id, page_index),
        )
    }

    /// 移除书签。
    pub fn remove_bookmark(&self, archive_id: i64, page_index: i64) -> Result<usize> {
        self.conn()?.execute(
            "DELETE FROM bookmarks WHERE archive_id = ? AND page_index = ?",
            (archive_id, page_index),
        )
    }
}
