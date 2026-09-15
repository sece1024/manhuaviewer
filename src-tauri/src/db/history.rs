//! 阅读历史（history 表，联动 archives.last_read_at 冗余列）相关的查询。

use rusqlite::Result;

use super::{execute_with_busy_retry, log_and_skip, Database, HistoryEntry, HistoryRow};

impl Database {
    pub fn get_history(
        &self,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<HistoryEntry>, i64)> {
        let conn = self.conn()?;
        let mut where_clause = String::from("WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = search {
            if !s.is_empty() {
                where_clause.push_str(
                    " AND (a.title LIKE ? OR EXISTS (
                        SELECT 1 FROM archive_tags at2
                        JOIN tags t2 ON t2.id = at2.tag_id
                        WHERE at2.archive_id = h.archive_id AND t2.name LIKE ?
                    ))",
                );
                let pattern = format!("%{}%", s);
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
            }
        }

        let count_sql = format!(
            "SELECT COUNT(*) FROM history h JOIN archives a ON a.id = h.archive_id {}",
            where_clause
        );
        let total: i64 = conn.query_row(
            &count_sql,
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        let sql = format!(
            "SELECT h.archive_id, h.page_index, h.total_pages, h.updated_at, a.title, a.path, a.archive_type
             FROM history h
             JOIN archives a ON a.id = h.archive_id
             {}
             ORDER BY h.updated_at DESC
             LIMIT ? OFFSET ?",
            where_clause
        );
        let mut stmt = conn.prepare(&sql)?;

        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let history = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |row| {
                    Ok((
                        HistoryRow {
                            archive_id: row.get(0)?,
                            page_index: row.get(1)?,
                            total_pages: row.get(2)?,
                            updated_at: row.get(3)?,
                        },
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )?
            .filter_map(log_and_skip)
            .collect();

        Ok((history, total))
    }

    pub fn save_history(
        &self,
        archive_id: i64,
        page_index: i64,
        total_pages: i64,
    ) -> Result<usize> {
        // 翻页时频繁调用；扫描等长事务可能短暂持锁，busy_timeout 后用重试兜底。
        // 同一事务内写入 history 并同步冗余列 last_read_at（供“最近阅读”排序走索引）。
        execute_with_busy_retry(|| {
            let conn = self.conn()?;
            let tx = conn.unchecked_transaction()?;
            let n = tx.execute(
                "INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at) VALUES (?, ?, ?, datetime('now'))",
                (archive_id, page_index, total_pages),
            )?;
            tx.execute(
                "UPDATE archives SET last_read_at = datetime('now') WHERE id = ?",
                [archive_id],
            )?;
            tx.commit()?;
            Ok(n)
        })
    }

    pub fn get_history_for_archive(&self, archive_id: i64) -> Result<Option<HistoryRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT archive_id, page_index, total_pages, updated_at FROM history WHERE archive_id = ?",
        )?;

        let mut rows = stmt.query_map([archive_id], |row| {
            Ok(HistoryRow {
                archive_id: row.get(0)?,
                page_index: row.get(1)?,
                total_pages: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 批量读取一批档案的阅读进度 (archive_id, page_index)，供列表接口附在卡片上。
    pub fn get_history_for_archives(&self, ids: &[i64]) -> Result<Vec<(i64, i64)>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT archive_id, page_index FROM history WHERE archive_id IN ({})",
            placeholders
        );
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?;
        Ok(rows.filter_map(log_and_skip).collect())
    }

    pub fn delete_history(&self, archive_id: i64) -> Result<usize> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        let n = tx.execute("DELETE FROM history WHERE archive_id = ?", [archive_id])?;
        tx.execute(
            "UPDATE archives SET last_read_at = NULL WHERE id = ?",
            [archive_id],
        )?;
        tx.commit()?;
        Ok(n)
    }

    pub fn clear_history(&self) -> Result<usize> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        let n = tx.execute("DELETE FROM history", [])?;
        tx.execute("UPDATE archives SET last_read_at = NULL", [])?;
        tx.commit()?;
        Ok(n)
    }
}
