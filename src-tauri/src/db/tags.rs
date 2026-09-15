//! 标签（tags / archive_tags 表）相关的查询。

use rusqlite::{OptionalExtension, Result};

use super::{log_and_skip, Database, TagRow};

impl Database {
    pub fn list_tags(&self) -> Result<Vec<TagRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT t.id, t.namespace, t.name, t.color, COUNT(at.archive_id)
             FROM tags t
             LEFT JOIN archive_tags at ON at.tag_id = t.id
             GROUP BY t.id
             ORDER BY t.namespace, t.name",
        )?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagRow {
                    id: row.get(0)?,
                    namespace: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    archive_count: row.get(4)?,
                })
            })?
            .filter_map(log_and_skip)
            .collect();
        Ok(tags)
    }

    pub fn create_tag(&self, namespace: &str, name: &str, color: &str) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)",
            (namespace, name, color),
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_tag(&self, id: i64, namespace: &str, name: &str, color: &str) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE tags SET namespace = ?, name = ?, color = ? WHERE id = ?",
            (namespace, name, color, id),
        )
    }

    pub fn delete_tag(&self, id: i64) -> Result<usize> {
        self.conn()?.execute("DELETE FROM tags WHERE id = ?", [id])
    }

    pub fn assign_tag(&self, archive_id: i64, tag_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
            (archive_id, tag_id),
        )
    }

    /// 按 (namespace, name) 幂等获取或创建标签并返回 id（供同步/回放元数据使用）。
    pub fn get_or_create_tag(&self, namespace: &str, name: &str, color: &str) -> Result<i64> {
        let conn = self.conn()?;
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE namespace = ? AND name = ?",
                (namespace, name),
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)",
            (namespace, name, color),
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn remove_tag(&self, archive_id: i64, tag_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "DELETE FROM archive_tags WHERE archive_id = ? AND tag_id = ?",
            (archive_id, tag_id),
        )
    }

    /// 批量为多个档案分配标签，单事务执行
    pub fn batch_assign_tag(&self, archive_ids: &[i64], tag_id: i64) -> Result<usize> {
        if archive_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &archive_id in archive_ids {
            affected += tx.execute(
                "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
                (archive_id, tag_id),
            )?;
        }
        tx.commit()?;
        Ok(affected)
    }

    /// 批量移除多个档案的标签，单事务执行
    pub fn batch_remove_tag(&self, archive_ids: &[i64], tag_id: i64) -> Result<usize> {
        if archive_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &archive_id in archive_ids {
            affected += tx.execute(
                "DELETE FROM archive_tags WHERE archive_id = ? AND tag_id = ?",
                (archive_id, tag_id),
            )?;
        }
        tx.commit()?;
        Ok(affected)
    }

    pub fn list_namespaces(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT namespace FROM tags WHERE namespace != '' ORDER BY namespace",
        )?;
        let namespaces = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(log_and_skip)
            .collect();
        Ok(namespaces)
    }

    pub fn get_archive_tags(&self, archive_id: i64) -> Result<Vec<TagRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT t.id, t.namespace, t.name, t.color
             FROM tags t
             JOIN archive_tags at ON at.tag_id = t.id
             WHERE at.archive_id = ?
             ORDER BY t.namespace, t.name",
        )?;

        let tags = stmt
            .query_map([archive_id], |row| {
                Ok(TagRow {
                    id: row.get(0)?,
                    namespace: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    archive_count: 0,
                })
            })?
            .filter_map(log_and_skip)
            .collect();

        Ok(tags)
    }

    pub fn get_archive_tags_batch(
        &self,
        archive_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Vec<TagRow>>> {
        if archive_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let placeholders: String = archive_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT at.archive_id, t.id, t.namespace, t.name, t.color
             FROM tags t
             JOIN archive_tags at ON at.tag_id = t.id
             WHERE at.archive_id IN ({})
             ORDER BY at.archive_id, t.namespace, t.name",
            placeholders
        );

        let conn = self.conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = archive_ids
            .iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let mut map: std::collections::HashMap<i64, Vec<TagRow>> = std::collections::HashMap::new();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                TagRow {
                    id: row.get(1)?,
                    namespace: row.get(2)?,
                    name: row.get(3)?,
                    color: row.get(4)?,
                    archive_count: 0,
                },
            ))
        })?;

        for row in rows.flatten() {
            map.entry(row.0).or_default().push(row.1);
        }

        Ok(map)
    }

    /// 单个标签的名称查询（供 OPDS 使用）
    pub fn get_tag_name(&self, tag_id: i64) -> Result<Option<String>> {
        self.conn()?
            .query_row("SELECT name FROM tags WHERE id = ?", [tag_id], |row| {
                row.get(0)
            })
            .optional()
    }
}
