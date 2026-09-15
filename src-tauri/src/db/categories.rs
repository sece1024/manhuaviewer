//! 分类（categories / archive_categories 表）相关的查询。

use rusqlite::{OptionalExtension, Result};

use super::{log_and_skip, CategoryRow, Database};

impl Database {
    /// 单个分类的名称查询（供 OPDS 等只需要名字的场景，免拉全量分类及其计数）。
    pub fn get_category_name(&self, id: i64) -> Result<Option<String>> {
        self.conn()?
            .query_row("SELECT name FROM categories WHERE id = ?", [id], |row| {
                row.get(0)
            })
            .optional()
    }

    pub fn list_categories(&self) -> Result<Vec<CategoryRow>> {
        // Single query with per-row counts instead of N separate COUNT round trips.
        // Static categories count join rows; dynamic categories (search) count title matches.
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.name, c.color, c.pinned, c.search, c.created_at,
                    CASE WHEN c.search = '' THEN
                        (SELECT COUNT(*) FROM archive_categories ac WHERE ac.category_id = c.id)
                    ELSE
                        (SELECT COUNT(*) FROM archives a WHERE a.title LIKE '%' || c.search || '%')
                    END AS archive_count
             FROM categories c
             ORDER BY c.name",
        )?;
        let categories = stmt
            .query_map([], |row| {
                Ok(CategoryRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    pinned: row.get::<_, i64>(3)? != 0,
                    search: row.get(4)?,
                    created_at: row.get(5)?,
                    archive_count: row.get(6)?,
                })
            })?
            .filter_map(log_and_skip)
            .collect();

        Ok(categories)
    }

    /// 获取指定档案已分配的（静态）分类
    pub fn get_archive_categories(&self, archive_id: i64) -> Result<Vec<CategoryRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.name, c.color, c.pinned, c.search, c.created_at
             FROM categories c
             JOIN archive_categories ac ON ac.category_id = c.id
             WHERE ac.archive_id = ?
             ORDER BY c.name",
        )?;

        let categories = stmt
            .query_map([archive_id], |row| {
                Ok(CategoryRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    pinned: row.get::<_, i64>(3)? != 0,
                    search: row.get(4)?,
                    created_at: row.get(5)?,
                    archive_count: 0,
                })
            })?
            .filter_map(log_and_skip)
            .collect();

        Ok(categories)
    }

    pub fn create_category(
        &self,
        name: &str,
        color: &str,
        pinned: bool,
        search: &str,
    ) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO categories (name, color, pinned, search) VALUES (?, ?, ?, ?)",
            (name, color, pinned as i64, search),
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_category(
        &self,
        id: i64,
        name: &str,
        color: &str,
        pinned: bool,
        search: &str,
    ) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE categories SET name = ?, color = ?, pinned = ?, search = ? WHERE id = ?",
            (name, color, pinned as i64, search, id),
        )
    }

    pub fn delete_category(&self, id: i64) -> Result<usize> {
        self.conn()?
            .execute("DELETE FROM categories WHERE id = ?", [id])
    }

    pub fn assign_category(&self, archive_id: i64, category_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "INSERT OR IGNORE INTO archive_categories (archive_id, category_id) VALUES (?, ?)",
            (archive_id, category_id),
        )
    }

    /// 按 name 幂等获取或创建分类并返回 id（供同步/回放元数据使用）。
    pub fn get_or_create_category(
        &self,
        name: &str,
        color: &str,
        pinned: bool,
        search: &str,
    ) -> Result<i64> {
        let conn = self.conn()?;
        let existing: Option<i64> = conn
            .query_row("SELECT id FROM categories WHERE name = ?", [name], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO categories (name, color, pinned, search) VALUES (?, ?, ?, ?)",
            (name, color, pinned as i64, search),
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 批量为多个档案分配分类，单事务执行
    pub fn batch_assign_category(&self, archive_ids: &[i64], category_id: i64) -> Result<usize> {
        if archive_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &archive_id in archive_ids {
            affected += tx.execute(
                "INSERT OR IGNORE INTO archive_categories (archive_id, category_id) VALUES (?, ?)",
                (archive_id, category_id),
            )?;
        }
        tx.commit()?;
        Ok(affected)
    }

    /// 批量移除多个档案的分类，单事务执行
    pub fn batch_remove_category(&self, archive_ids: &[i64], category_id: i64) -> Result<usize> {
        if archive_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &archive_id in archive_ids {
            affected += tx.execute(
                "DELETE FROM archive_categories WHERE archive_id = ? AND category_id = ?",
                (archive_id, category_id),
            )?;
        }
        tx.commit()?;
        Ok(affected)
    }

    pub fn remove_category(&self, archive_id: i64, category_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "DELETE FROM archive_categories WHERE archive_id = ? AND category_id = ?",
            (archive_id, category_id),
        )
    }
}
