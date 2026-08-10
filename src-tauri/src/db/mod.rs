pub mod migrations;
pub mod schema;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, Result};
use serde::{Deserialize, Serialize};

/// Helper: log and skip row-level errors instead of silently swallowing them
fn log_and_skip<T>(row_result: std::result::Result<T, rusqlite::Error>) -> Option<T> {
    match row_result {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("Skipping row due to error: {}", e);
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRow {
    pub id: i64,
    pub title: String,
    pub path: String,
    pub archive_type: String,
    pub page_count: i64,
    pub cover_image: Option<String>,
    pub file_size: i64,
    pub thumbnail_path: Option<String>,
    pub group_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRow {
    pub id: i64,
    pub namespace: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub archive_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub pinned: bool,
    pub search: String,
    pub created_at: String,
    #[serde(default)]
    pub archive_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRow {
    pub archive_id: i64,
    pub page_index: i64,
    pub total_pages: i64,
    pub updated_at: String,
}

/// A cached page list entry for a compressed archive.
#[derive(Debug, Clone)]
pub struct PageRow {
    pub id: i64,
    pub archive_id: i64,
    pub filename: String,
    pub filepath: String,
    pub sort_order: i64,
}

/// (history row, archive title, archive path, archive type)
pub type HistoryEntry = (HistoryRow, String, String, String);

/// Connection pool wrapper. Each query acquires its own SQLite connection, so
/// concurrent requests no longer serialize on a single `Connection` behind a
/// global mutex (WAL mode already permits concurrent readers).
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let manager = SqliteConnectionManager::file(path).with_init(|conn| {
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "foreign_keys", "ON")?;
            Ok(())
        });
        let pool = r2d2::Pool::builder().max_size(8).build(manager)?;
        Ok(Self { pool })
    }

    /// Acquire a pooled connection. A pool exhaustion / init failure is a real
    /// runtime error, not a SQL issue; map it into rusqlite's error space so
    /// callers keep working with `?`.
    fn conn(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
    }

    pub fn init(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute_batch(schema::SCHEMA)?;
        migrations::run_migrations(&conn)?;
        drop(conn);
        self.init_settings()?;
        Ok(())
    }

    fn init_settings(&self) -> Result<()> {
        let defaults = [
            ("root_dir", ""),
            ("view_mode", "grid"),
            ("sort_by", "updated"),
            ("sort_order", "desc"),
            ("reader_fit", "height"),
            ("reader_bg", "#1a1a1a"),
            ("auto_scan_interval", "0"),
            ("scan_depth", "1"),
            ("rename_suggest_depth", "3"),
            ("page_direction", "rtl"),
            ("theme", "dark"),
        ];

        let conn = self.conn()?;
        for (key, value) in defaults {
            conn.execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                (key, value),
            )?;
        }

        Ok(())
    }

    // Archive operations
    pub fn get_archive(&self, id: i64) -> Result<Option<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at FROM archives WHERE id = ?"
        )?;

        let mut rows = stmt.query_map([id], |row| {
            Ok(ArchiveRow {
                id: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                archive_type: row.get(3)?,
                page_count: row.get(4)?,
                cover_image: row.get(5)?,
                file_size: row.get(6)?,
                thumbnail_path: row.get(7)?,
                group_id: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn get_archive_by_path(&self, path: &str) -> Result<Option<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at FROM archives WHERE path = ?"
        )?;

        let mut rows = stmt.query_map([path], |row| {
            Ok(ArchiveRow {
                id: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                archive_type: row.get(3)?,
                page_count: row.get(4)?,
                cover_image: row.get(5)?,
                file_size: row.get(6)?,
                thumbnail_path: row.get(7)?,
                group_id: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    // Page list cache operations (compressed archives only)
    pub fn get_page_list_mtime(&self, archive_id: i64) -> Result<Option<i64>> {
        self.conn()?
            .query_row(
                "SELECT page_list_mtime FROM archives WHERE id = ?",
                [archive_id],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn get_pages(&self, archive_id: i64) -> Result<Vec<PageRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, archive_id, filename, filepath, sort_order
             FROM pages WHERE archive_id = ? ORDER BY sort_order",
        )?;
        let pages = stmt
            .query_map([archive_id], |row| {
                Ok(PageRow {
                    id: row.get(0)?,
                    archive_id: row.get(1)?,
                    filename: row.get(2)?,
                    filepath: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .filter_map(log_and_skip)
            .collect();
        Ok(pages)
    }

    /// Replace the cached page list for an archive and record the archive file
    /// mtime used to build it (used to detect staleness on later requests).
    pub fn save_pages(&self, archive_id: i64, pages: &[PageRow], mtime_secs: i64) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM pages WHERE archive_id = ?", [archive_id])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO pages (archive_id, filename, filepath, sort_order) VALUES (?, ?, ?, ?)",
            )?;
            for p in pages {
                stmt.execute((archive_id, &p.filename, &p.filepath, p.sort_order))?;
            }
        }
        tx.execute(
            "UPDATE archives SET page_list_mtime = ? WHERE id = ?",
            (mtime_secs, archive_id),
        )?;
        tx.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_archives(
        &self,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
        sort: &str,
        order: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut where_clause = String::from("WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = search {
            if !s.is_empty() {
                where_clause.push_str(" AND a.title LIKE ?");
                params.push(Box::new(format!("%{}%", s)));
            }
        }

        // 按标签过滤：支持 "namespace:name" 或 "name" 格式
        let mut join_clause = String::new();
        if let Some(t) = tag {
            if !t.is_empty() {
                join_clause.push_str(
                    " JOIN archive_tags at_f ON at_f.archive_id = a.id JOIN tags t_f ON t_f.id = at_f.tag_id",
                );
                if let Some((ns, name)) = t.split_once(':') {
                    where_clause.push_str(" AND t_f.namespace = ? AND t_f.name = ?");
                    params.push(Box::new(ns.to_string()));
                    params.push(Box::new(name.to_string()));
                } else {
                    where_clause.push_str(" AND t_f.name = ? AND t_f.namespace = ''");
                    params.push(Box::new(t.to_string()));
                }
            }
        }

        // 按分类过滤：静态分类走关联表 JOIN，动态分类（配置了 search）走标题匹配
        if let Some(cid) = category_id {
            let dynamic_search: Option<String> = conn
                .query_row("SELECT search FROM categories WHERE id = ?", [cid], |row| {
                    row.get(0)
                })
                .ok();
            match dynamic_search {
                Some(s) if !s.is_empty() => {
                    where_clause.push_str(" AND a.title LIKE ?");
                    params.push(Box::new(format!("%{}%", s)));
                }
                _ => {
                    join_clause.push_str(" JOIN archive_categories ac_f ON ac_f.archive_id = a.id");
                    where_clause.push_str(" AND ac_f.category_id = ?");
                    params.push(Box::new(cid));
                }
            }
        }

        // Build main query
        let order_clause = match sort {
            "name" => "a.title",
            "created" => "a.created_at",
            "pages" => "a.page_count",
            "size" => "a.file_size",
            _ => "a.updated_at",
        };
        let direction = if order == "asc" { "ASC" } else { "DESC" };

        let sql = format!(
            "SELECT a.id, a.title, a.path, a.archive_type, a.page_count, a.cover_image, a.file_size, a.thumbnail_path, a.group_id, a.created_at, a.updated_at FROM archives a{} {} ORDER BY {} {} LIMIT ? OFFSET ?",
            join_clause, where_clause, order_clause, direction
        );

        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql)?;
        let archives = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |row| {
                    Ok(ArchiveRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        path: row.get(2)?,
                        archive_type: row.get(3)?,
                        page_count: row.get(4)?,
                        cover_image: row.get(5)?,
                        file_size: row.get(6)?,
                        thumbnail_path: row.get(7)?,
                        group_id: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    pub fn list_archives_by_tag(
        &self,
        tag_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.title, a.path, a.archive_type, a.page_count, a.cover_image, a.file_size, a.thumbnail_path, a.group_id, a.created_at, a.updated_at
             FROM archives a
             JOIN archive_tags at ON at.archive_id = a.id
             WHERE at.tag_id = ?
             ORDER BY a.updated_at DESC
             LIMIT ? OFFSET ?",
        )?;

        let archives = stmt
            .query_map(rusqlite::params![tag_id, limit, offset], |row| {
                Ok(ArchiveRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    path: row.get(2)?,
                    archive_type: row.get(3)?,
                    page_count: row.get(4)?,
                    cover_image: row.get(5)?,
                    file_size: row.get(6)?,
                    thumbnail_path: row.get(7)?,
                    group_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    pub fn insert_archive(
        &self,
        title: &str,
        path: &str,
        archive_type: &str,
        page_count: i64,
        file_size: i64,
    ) -> Result<i64> {
        let conn = self.conn()?;
        // Check for existing archive with the same path first
        let existing: Option<i64> = conn
            .query_row("SELECT id FROM archives WHERE path = ?", [path], |row| {
                row.get(0)
            })
            .optional()?;

        if let Some(id) = existing {
            return Ok(id);
        }

        conn.execute(
            "INSERT INTO archives (title, path, archive_type, page_count, file_size) VALUES (?, ?, ?, ?, ?)",
            (title, path, archive_type, page_count, file_size),
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_archive(&self, id: i64) -> Result<usize> {
        self.conn()?
            .execute("DELETE FROM archives WHERE id = ?", [id])
    }

    /// 批量插入档案，单事务执行。返回 (实际新增数, 错误数)；
    /// 已存在的路径（path 唯一约束冲突）不计入新增。
    pub fn insert_archives_many(
        &self,
        items: &[(String, String, String, i64, i64)],
    ) -> Result<(usize, usize)> {
        if items.is_empty() {
            return Ok((0, 0));
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut added = 0;
        let mut errors = 0;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO archives (title, path, archive_type, page_count, file_size) VALUES (?, ?, ?, ?, ?)",
            )?;
            for (title, path, archive_type, page_count, file_size) in items {
                match stmt.execute((title, path, archive_type, page_count, file_size)) {
                    Ok(affected) if affected > 0 => added += 1,
                    Ok(_) => {} // duplicate path, skipped
                    Err(e) => {
                        tracing::warn!("Failed to insert {}: {}", path, e);
                        errors += 1;
                    }
                }
            }
        }
        tx.commit()?;
        Ok((added, errors))
    }

    /// 批量删除档案，单事务执行
    pub fn batch_delete_archives(&self, ids: &[i64]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &id in ids {
            affected += tx.execute("DELETE FROM archives WHERE id = ?", [id])?;
        }
        tx.commit()?;
        Ok(affected)
    }

    pub fn update_archive_title(&self, id: i64, title: &str) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE archives SET title = ?, updated_at = datetime('now') WHERE id = ?",
            (title, id),
        )
    }

    /// 获取组内所有章节（按路径排序）
    pub fn get_group_chapters(&self, group_id: i64) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at
             FROM archives WHERE group_id = ? ORDER BY path",
        )?;

        let archives = stmt
            .query_map([group_id], |row| {
                Ok(ArchiveRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    path: row.get(2)?,
                    archive_type: row.get(3)?,
                    page_count: row.get(4)?,
                    cover_image: row.get(5)?,
                    file_size: row.get(6)?,
                    thumbnail_path: row.get(7)?,
                    group_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    /// 合并多个档案：第一个为主档案，其余 group_id 设为主档案 id
    pub fn merge_archives(&self, archive_ids: &[i64]) -> Result<i64> {
        let primary_id = archive_ids[0];
        let conn = self.conn()?;

        // 主档案: group_id 设为自身 id
        conn.execute(
            "UPDATE archives SET group_id = ?, updated_at = datetime('now') WHERE id = ?",
            (primary_id, primary_id),
        )?;

        // 其余档案: group_id 设为主档案 id
        for &id in &archive_ids[1..] {
            conn.execute(
                "UPDATE archives SET group_id = ?, updated_at = datetime('now') WHERE id = ?",
                (primary_id, id),
            )?;
        }

        Ok(primary_id)
    }

    // Thumbnail cache operations
    const MAX_CACHED_ARCHIVES: i64 = 20;

    pub fn set_thumbnail_path(&self, archive_id: i64, thumb_path: &str) -> Result<()> {
        self.conn()?.execute(
            "UPDATE archives SET thumbnail_path = ? WHERE id = ?",
            (thumb_path, archive_id),
        )?;
        Ok(())
    }

    pub fn get_cached_archive_ids(&self) -> Result<Vec<i64>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.id FROM archives a
             LEFT JOIN history h ON h.archive_id = a.id
             WHERE a.thumbnail_path IS NOT NULL
             ORDER BY COALESCE(h.updated_at, a.updated_at) DESC",
        )?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .filter_map(log_and_skip)
            .collect();
        Ok(ids)
    }

    pub fn evict_old_thumbnails(&self) -> Result<Vec<(i64, String)>> {
        let cached_ids = self.get_cached_archive_ids()?;
        if cached_ids.len() as i64 <= Self::MAX_CACHED_ARCHIVES {
            return Ok(vec![]);
        }

        // 要淘汰的：超出限制的最旧条目
        let to_evict = &cached_ids[Self::MAX_CACHED_ARCHIVES as usize..];
        let mut evicted = Vec::new();
        let conn = self.conn()?;

        for &id in to_evict {
            let thumb_path: Option<String> = conn.query_row(
                "SELECT thumbnail_path FROM archives WHERE id = ?",
                [id],
                |row| row.get(0),
            )?;
            if let Some(path) = thumb_path {
                conn.execute(
                    "UPDATE archives SET thumbnail_path = NULL WHERE id = ?",
                    [id],
                )?;
                evicted.push((id, path));
            }
        }

        Ok(evicted)
    }

    // Tag operations
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

    // Category operations
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

    /// 批量为多个档案分配分类，单事务执行
    pub fn batch_assign_category(
        &self,
        archive_ids: &[i64],
        category_id: i64,
    ) -> Result<usize> {
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
    pub fn batch_remove_category(
        &self,
        archive_ids: &[i64],
        category_id: i64,
    ) -> Result<usize> {
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

    // History operations
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
        self.conn()?.execute(
            "INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at) VALUES (?, ?, ?, datetime('now'))",
            (archive_id, page_index, total_pages),
        )
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

    pub fn delete_history(&self, archive_id: i64) -> Result<usize> {
        self.conn()?
            .execute("DELETE FROM history WHERE archive_id = ?", [archive_id])
    }

    pub fn clear_history(&self) -> Result<usize> {
        self.conn()?.execute("DELETE FROM history", [])
    }

    // Settings operations
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
        for (key, value) in settings {
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
                (key, value),
            )?;
        }
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
        let total_tags: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        let total_categories: i64 =
            conn.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?;
        let history_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;

        Ok(serde_json::json!({
            "total_archives": total_archives,
            "total_pages": total_pages,
            "total_tags": total_tags,
            "total_categories": total_categories,
            "history_count": history_count
        }))
    }

    // Backup
    pub fn export_backup(&self) -> Result<serde_json::Value> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT title, path, archive_type, page_count FROM archives")?;
        let archives: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "title": row.get::<_, String>(0)?,
                    "path": row.get::<_, String>(1)?,
                    "archive_type": row.get::<_, String>(2)?,
                    "page_count": row.get::<_, i64>(3)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare("SELECT namespace, name, color FROM tags")?;
        let tags: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "namespace": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "color": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare("SELECT name, color, search FROM categories")?;
        let categories: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "name": row.get::<_, String>(0)?,
                    "color": row.get::<_, String>(1)?,
                    "search": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let settings = self.get_settings()?;

        Ok(serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "archives": archives,
            "tags": tags,
            "categories": categories,
            "settings": settings,
        }))
    }

    pub fn import_backup(&self, backup: &serde_json::Value) -> Result<()> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;

        // Import archives
        if let Some(archives) = backup["archives"].as_array() {
            for archive in archives {
                if let (Some(title), Some(path), Some(archive_type), Some(page_count)) = (
                    archive["title"].as_str(),
                    archive["path"].as_str(),
                    archive["archive_type"].as_str(),
                    archive["page_count"].as_i64(),
                ) {
                    tx.execute(
                        "INSERT OR REPLACE INTO archives (title, path, archive_type, page_count) VALUES (?, ?, ?, ?)",
                        (title, path, archive_type, page_count),
                    )?;
                }
            }
        }

        // Import tags
        if let Some(tags) = backup["tags"].as_array() {
            for tag in tags {
                if let (Some(namespace), Some(name), Some(color)) = (
                    tag["namespace"].as_str(),
                    tag["name"].as_str(),
                    tag["color"].as_str(),
                ) {
                    tx.execute(
                        "INSERT OR IGNORE INTO tags (namespace, name, color) VALUES (?, ?, ?)",
                        (namespace, name, color),
                    )?;
                }
            }
        }

        // Import categories
        if let Some(categories) = backup["categories"].as_array() {
            for category in categories {
                if let (Some(name), Some(color), Some(search)) = (
                    category["name"].as_str(),
                    category["color"].as_str(),
                    category["search"].as_str(),
                ) {
                    tx.execute(
                        "INSERT OR IGNORE INTO categories (name, color, search) VALUES (?, ?, ?)",
                        (name, color, search),
                    )?;
                }
            }
        }

        // Import settings
        if let Some(settings) = backup["settings"].as_object() {
            for (key, value) in settings {
                if let Some(v) = value.as_str() {
                    tx.execute(
                        "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
                        (key, v),
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// 单个标签的名称查询（供 OPDS 使用）
    pub fn get_tag_name(&self, tag_id: i64) -> Result<Option<String>> {
        self.conn()?
            .query_row("SELECT name FROM tags WHERE id = ?", [tag_id], |row| {
                row.get(0)
            })
            .optional()
    }

    /// 测试专用：暴露一条原始连接
    #[cfg(test)]
    pub fn conn_for_test(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        self.conn()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> Database {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        let db = Database::new(path).unwrap();
        db.init().unwrap();
        db
    }

    #[test]
    fn test_database_creation() {
        let db = setup_test_db();
        let conn = db.conn_for_test().unwrap();

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(log_and_skip)
            .collect();

        assert!(tables.contains(&"archives".to_string()));
        assert!(tables.contains(&"tags".to_string()));
        assert!(tables.contains(&"categories".to_string()));
        assert!(tables.contains(&"history".to_string()));
        assert!(tables.contains(&"settings".to_string()));
    }

    #[test]
    fn test_default_settings() {
        let db = setup_test_db();
        let settings = db.get_settings().unwrap();

        assert_eq!(settings.get("view_mode").unwrap(), "grid");
        assert_eq!(settings.get("sort_by").unwrap(), "updated");
        assert_eq!(settings.get("sort_order").unwrap(), "desc");
        assert_eq!(settings.get("theme").unwrap(), "dark");
    }

    #[test]
    fn test_insert_and_get_archive() {
        let db = setup_test_db();

        let id = db
            .insert_archive("Test Manga", "/path/to/manga", "zip", 10, 1024)
            .unwrap();

        assert!(id > 0);

        let archive = db.get_archive(id).unwrap();
        assert!(archive.is_some());

        let archive = archive.unwrap();
        assert_eq!(archive.title, "Test Manga");
        assert_eq!(archive.path, "/path/to/manga");
        assert_eq!(archive.archive_type, "zip");
        assert_eq!(archive.page_count, 10);
        assert_eq!(archive.file_size, 1024);
    }

    #[test]
    fn test_list_archives() {
        let db = setup_test_db();

        db.insert_archive("Manga A", "/path/a", "zip", 5, 500)
            .unwrap();
        db.insert_archive("Manga B", "/path/b", "folder", 10, 1000)
            .unwrap();
        db.insert_archive("Manga C", "/path/c", "rar", 15, 1500)
            .unwrap();

        let archives = db
            .list_archives(None, None, None, "title", "asc", 10, 0)
            .unwrap();
        assert_eq!(archives.len(), 3);
        assert_eq!(archives[0].title, "Manga A");
        assert_eq!(archives[1].title, "Manga B");
        assert_eq!(archives[2].title, "Manga C");
    }

    #[test]
    fn test_list_archives_with_search() {
        let db = setup_test_db();

        db.insert_archive("Naruto", "/path/naruto", "zip", 100, 5000)
            .unwrap();
        db.insert_archive("One Piece", "/path/onepiece", "zip", 200, 10000)
            .unwrap();
        db.insert_archive("Dragon Ball", "/path/db", "folder", 50, 2500)
            .unwrap();

        let archives = db
            .list_archives(Some("Naruto"), None, None, "title", "asc", 10, 0)
            .unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].title, "Naruto");
    }

    #[test]
    fn test_delete_archive() {
        let db = setup_test_db();

        let id = db.insert_archive("Test", "/path", "zip", 5, 500).unwrap();
        assert!(db.get_archive(id).unwrap().is_some());

        db.delete_archive(id).unwrap();
        assert!(db.get_archive(id).unwrap().is_none());
    }

    #[test]
    fn test_tag_operations() {
        let db = setup_test_db();

        // Create tag
        let tag_id = db.create_tag("artist", "mika", "#ff0000").unwrap();
        assert!(tag_id > 0);

        // List tags
        let tags = db.list_tags().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "mika");
        assert_eq!(tags[0].namespace, "artist");
        assert_eq!(tags[0].color, "#ff0000");

        // List namespaces
        let namespaces = db.list_namespaces().unwrap();
        assert_eq!(namespaces.len(), 1);
        assert_eq!(namespaces[0], "artist");

        // Delete tag
        db.delete_tag(tag_id).unwrap();
        let tags = db.list_tags().unwrap();
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn test_category_operations() {
        let db = setup_test_db();

        // Create category
        let cat_id = db.create_category("Action", "#00ff00", false, "").unwrap();
        assert!(cat_id > 0);

        // List categories
        let categories = db.list_categories().unwrap();
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].name, "Action");
        assert_eq!(categories[0].color, "#00ff00");

        // Delete category
        db.delete_category(cat_id).unwrap();
        let categories = db.list_categories().unwrap();
        assert_eq!(categories.len(), 0);
    }

    #[test]
    fn test_archive_tag_assignment() {
        let db = setup_test_db();

        let archive_id = db.insert_archive("Test", "/path", "zip", 5, 500).unwrap();
        let tag_id = db.create_tag("", "favorite", "#ff0000").unwrap();

        // Assign tag to archive
        db.assign_tag(archive_id, tag_id).unwrap();

        // Remove tag from archive
        db.remove_tag(archive_id, tag_id).unwrap();
    }

    #[test]
    fn test_history_operations() {
        let db = setup_test_db();

        let archive_id = db.insert_archive("Test", "/path", "zip", 10, 500).unwrap();

        // Save history
        db.save_history(archive_id, 5, 10).unwrap();

        // Get history
        let (history, total) = db.get_history(None, 50, 0).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(total, 1);
        assert_eq!(history[0].0.archive_id, archive_id);
        assert_eq!(history[0].0.page_index, 5);
        assert_eq!(history[0].0.total_pages, 10);

        // Delete history
        db.delete_history(archive_id).unwrap();
        let (history, total) = db.get_history(None, 50, 0).unwrap();
        assert_eq!(history.len(), 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn test_update_settings() {
        let db = setup_test_db();

        let mut settings = std::collections::HashMap::new();
        settings.insert("theme".to_string(), "light".to_string());
        settings.insert("view_mode".to_string(), "list".to_string());

        db.update_settings(&settings).unwrap();

        let updated = db.get_settings().unwrap();
        assert_eq!(updated.get("theme").unwrap(), "light");
        assert_eq!(updated.get("view_mode").unwrap(), "list");
    }

    #[test]
    fn test_get_stats() {
        let db = setup_test_db();

        db.insert_archive("A", "/a", "zip", 10, 500).unwrap();
        db.insert_archive("B", "/b", "folder", 20, 1000).unwrap();
        db.create_tag("", "tag1", "#ff0000").unwrap();
        db.create_category("Cat1", "#00ff00", false, "").unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats["total_archives"], 2);
        assert_eq!(stats["total_pages"], 30);
        assert_eq!(stats["total_tags"], 1);
        assert_eq!(stats["total_categories"], 1);
        assert_eq!(stats["history_count"], 0);
    }

    #[test]
    fn test_backup_and_restore() {
        let db1 = setup_test_db();

        // Add some data
        db1.insert_archive("Manga A", "/path/a", "zip", 10, 500)
            .unwrap();
        db1.create_tag("", "favorite", "#ff0000").unwrap();
        db1.create_category("Action", "#00ff00", false, "").unwrap();

        // Export backup
        let backup = db1.export_backup().unwrap();

        // Create new database and restore
        let db2 = setup_test_db();
        db2.import_backup(&backup).unwrap();

        // Verify data
        let archives = db2
            .list_archives(None, None, None, "title", "asc", 10, 0)
            .unwrap();
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].title, "Manga A");

        let tags = db2.list_tags().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "favorite");

        let categories = db2.list_categories().unwrap();
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].name, "Action");
    }

    #[test]
    fn test_insert_duplicate_archive_returns_existing_id() {
        let db = setup_test_db();

        let id1 = db
            .insert_archive("Manga A", "/path/a", "folder", 5, 100)
            .unwrap();
        // Insert a different archive in between
        let _id2 = db
            .insert_archive("Manga B", "/path/b", "zip", 10, 200)
            .unwrap();

        // Inserting the same path as A should return A's id, not B's
        let id3 = db
            .insert_archive("Manga A", "/path/a", "folder", 5, 100)
            .unwrap();
        assert_eq!(
            id3, id1,
            "Duplicate insert should return the original archive id, not the last inserted id"
        );
    }
}
