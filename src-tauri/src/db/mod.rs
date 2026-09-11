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

/// 判断 `path` 是否位于扫描根目录 `root` 之下（含相等）。
///
/// 使用 `Path::starts_with` 做**整组件**比较（Rust std 语义），而不是字符串前缀：
/// 目录名互为字符串前缀（如 `海贼` 与 `海贼王`）不会被误判为从属关系——
/// `/manhua/海贼王/01` 属于 `/manhua`，但不属于 `/manhua/海贼`。
/// 注意 `..`/`.` 片段按字面组件参与比较（与扫描/入库用的绝对化路径一致）。
fn path_is_within(root: &str, path: &str) -> bool {
    let r = std::path::Path::new(root);
    let p = std::path::Path::new(path);
    p == r || p.starts_with(r)
}

/// 统一的档案查询列（带 `a.` 前缀，用于 JOIN 场景）。
const ARCHIVE_COLUMNS: &str = "a.id, a.title, a.path, a.archive_type, a.page_count, a.cover_image, a.file_size, a.thumbnail_path, a.group_id, a.created_at, a.updated_at";

/// 档案列表过滤条件：(JOIN 片段, WHERE 片段, 参数)。
type ArchiveFilters = (String, String, Vec<Box<dyn rusqlite::types::ToSql>>);

/// 将一行查询结果映射为 `ArchiveRow`（列序必须与 `ARCHIVE_COLUMNS` 一致）。
fn archive_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArchiveRow> {
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

/// 排序方式 → ORDER BY 表达式。"updated"（最近阅读）优先按阅读时间排序：
/// 有 history 记录的用 history.updated_at，从未读过的回退到 archives.updated_at，
/// 这样"最近阅读"排序才是真实语义（读过的按最近读的时间冒泡，新加的仍按添加时间排）。
fn order_expr_for(sort: &str) -> &'static str {
    match sort {
        "name" | "title" => "a.title",
        "created" => "a.created_at",
        "pages" => "a.page_count",
        "size" => "a.file_size",
        "updated" => "COALESCE(h.updated_at, a.updated_at)",
        "random" => "RANDOM()",
        _ => "a.updated_at",
    }
}

/// 仅当按"最近阅读"排序时需要 LEFT JOIN history（history.archive_id 是主键，不产生重复行）。
fn history_join_for(sort: &str) -> &'static str {
    if sort == "updated" {
        " LEFT JOIN history h ON h.archive_id = a.id"
    } else {
        ""
    }
}

/// 写操作的轻量忙重试：busy_timeout 之后仍可能与另一个连接的长事务（扫描/批量导入）
/// 短暂冲突，重试几次兜底；每次重试重新取连接，通常在竞争者提交后即可成功。
fn execute_with_busy_retry<F, T>(mut f: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    const MAX_ATTEMPTS: usize = 3;
    let mut last_err = None;
    for attempt in 0..MAX_ATTEMPTS {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) => {
                let is_busy = match &e {
                    rusqlite::Error::SqliteFailure(err, _) => {
                        err.code == rusqlite::ffi::ErrorCode::DatabaseBusy
                            || err.code == rusqlite::ffi::ErrorCode::DatabaseLocked
                    }
                    _ => false,
                };
                if !is_busy || attempt + 1 == MAX_ATTEMPTS {
                    return Err(e);
                }
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt as u64 + 1)));
            }
        }
    }
    Err(last_err.unwrap())
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
            // 池内多连接并发写（如翻页存 history 撞上扫描长事务）时等待而不是立刻报错
            conn.busy_timeout(std::time::Duration::from_secs(5))?;
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
            ("reader_double", "0"),
            ("reader_long", "0"),
            ("reader_bg", "#1a1a1a"),
            ("server_bind", "127.0.0.1"),
            ("server_token", ""),
            ("backup_interval_hours", "0"),
            ("backup_keep", "10"),
            ("auto_scan_interval", "0"),
            ("scan_depth", "1"),
            ("rename_suggest_depth", "3"),
            ("title_depth", "1"),
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

    /// 一次查询取回档案行与其远程封面 URL，避免封面请求两次往返 DB。
    pub fn get_archive_with_remote_cover(
        &self,
        id: i64,
    ) -> Result<Option<(ArchiveRow, Option<String>)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at, remote_cover FROM archives WHERE id = ?"
        )?;
        let mut rows = stmt.query_map([id], |row| {
            Ok((
                ArchiveRow {
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
                },
                row.get::<_, Option<String>>(11)?,
            ))
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
        let (join_clause, where_clause, mut params) =
            Self::build_archive_filters(&conn, search, tag, category_id)?;

        let order_clause = order_expr_for(sort);
        let history_join = history_join_for(sort);
        let direction = if order == "asc" { "ASC" } else { "DESC" };

        let sql = format!(
            "SELECT {} FROM archives a{} {} {} ORDER BY {} {} LIMIT ? OFFSET ?",
            ARCHIVE_COLUMNS, history_join, join_clause, where_clause, order_clause, direction
        );

        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql)?;
        let archives = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                archive_row,
            )?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    /// 拉取所有符合过滤条件的档案（不分页），供服务端分组后统一分页。
    /// `read`: None=全部, "read"=已有阅读记录, "unread"=从未读过。
    #[allow(clippy::too_many_arguments)]
    pub fn list_archives_all(
        &self,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
        read: Option<&str>,
        sort: &str,
        order: &str,
    ) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let (join_clause, mut where_clause, params) =
            Self::build_archive_filters(&conn, search, tag, category_id)?;

        let read_clause = match read {
            Some("read") => " AND EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)",
            Some("unread") => {
                " AND NOT EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)"
            }
            _ => "",
        };
        where_clause.push_str(read_clause);

        let order_clause = order_expr_for(sort);
        let history_join = history_join_for(sort);
        let direction = if order == "asc" { "ASC" } else { "DESC" };

        let sql = format!(
            "SELECT {} FROM archives a{} {} {} ORDER BY {} {}",
            ARCHIVE_COLUMNS, history_join, join_clause, where_clause, order_clause, direction
        );

        let mut stmt = conn.prepare(&sql)?;
        let archives = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                archive_row,
            )?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    /// 构造档案列表查询的 JOIN / WHERE 片段与参数（供 list_archives 与 list_archives_all 共用）。
    fn build_archive_filters(
        conn: &rusqlite::Connection,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
    ) -> Result<ArchiveFilters> {
        let mut where_clause = String::from("WHERE 1=1");
        let mut join_clause = String::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = search {
            if !s.is_empty() {
                // 解析搜索语法：tag:xxx（标签名或 ns:name）、-xxx（排除）、普通关键词（标题或标签名）
                for token in s.split_whitespace() {
                    if token.is_empty() {
                        continue;
                    }
                    if let Some(spec) = token.strip_prefix("tag:") {
                        let (ns, name) = match spec.split_once(':') {
                            Some((ns, name)) => (Some(ns), name),
                            None => (None, spec),
                        };
                        let mut cond = String::from(
                            "EXISTS (SELECT 1 FROM archive_tags atx JOIN tags tx ON tx.id = atx.tag_id WHERE atx.archive_id = a.id",
                        );
                        if let Some(ns) = ns {
                            cond.push_str(" AND tx.namespace = ?");
                            params.push(Box::new(ns.to_string()));
                        }
                        cond.push_str(" AND tx.name = ?)");
                        params.push(Box::new(name.to_string()));
                        where_clause.push_str(" AND ");
                        where_clause.push_str(&cond);
                    } else if let Some(ex) = token.strip_prefix('-') {
                        if !ex.is_empty() {
                            let pattern = format!("%{}%", ex);
                            where_clause.push_str(
                                " AND a.title NOT LIKE ? AND NOT EXISTS (SELECT 1 FROM archive_tags atx JOIN tags tx ON tx.id = atx.tag_id WHERE atx.archive_id = a.id AND tx.name LIKE ?)",
                            );
                            params.push(Box::new(pattern.clone()));
                            params.push(Box::new(pattern));
                        }
                    } else {
                        let pattern = format!("%{}%", token);
                        where_clause.push_str(
                            " AND (a.title LIKE ? OR EXISTS (SELECT 1 FROM archive_tags atx JOIN tags tx ON tx.id = atx.tag_id WHERE atx.archive_id = a.id AND tx.name LIKE ?))",
                        );
                        params.push(Box::new(pattern.clone()));
                        params.push(Box::new(pattern));
                    }
                }
            }
        }

        // 按标签过滤：支持 "namespace:name" 或 "name" 格式
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

        Ok((join_clause, where_clause, params))
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
        conn.execute(
            "INSERT INTO archives (title, path, archive_type, page_count, file_size)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO NOTHING",
            (title, path, archive_type, page_count, file_size),
        )?;
        // ON CONFLICT DO NOTHING 吞掉并发重复插入，随后按 path 取回 id（可能已存在）。
        let id = conn.query_row("SELECT id FROM archives WHERE path = ?", [path], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(id)
    }

    pub fn delete_archive(&self, id: i64) -> Result<usize> {
        self.conn()?
            .execute("DELETE FROM archives WHERE id = ?", [id])
    }

    /// 增量扫描入库：按 path upsert（更新标题/类型/页数/大小/file_mtime），
    /// 不使用 INSERT OR REPLACE，避免级联删除 history 与标签/分类关联。返回该档案 id。
    pub fn upsert_scanned_archive(
        &self,
        title: &str,
        path: &str,
        archive_type: &str,
        page_count: i64,
        file_size: i64,
        file_mtime: i64,
    ) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO archives (title, path, archive_type, page_count, file_size, file_mtime, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                archive_type = excluded.archive_type,
                page_count = excluded.page_count,
                file_size = excluded.file_size,
                file_mtime = excluded.file_mtime,
                updated_at = excluded.updated_at",
            (title, path, archive_type, page_count, file_size, file_mtime),
        )?;
        let id = conn.query_row("SELECT id FROM archives WHERE path = ?", [path], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(id)
    }

    /// 批量 upsert 扫描结果：单连接 + 单事务内完成全部写入，避免每次 upsert
    /// 都单独获取连接并在 `SELECT id` 上往返。`entries` 为 (title, path, archive_type,
    /// page_count, file_size, file_mtime)。
    pub fn batch_upsert_scanned_archives(
        &self,
        entries: &[(String, String, String, i64, i64, i64)],
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO archives (title, path, archive_type, page_count, file_size, file_mtime, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
                 ON CONFLICT(path) DO UPDATE SET
                    title = excluded.title,
                    archive_type = excluded.archive_type,
                    page_count = excluded.page_count,
                    file_size = excluded.file_size,
                    file_mtime = excluded.file_mtime,
                    updated_at = excluded.updated_at",
            )?;
            for (title, path, archive_type, page_count, file_size, file_mtime) in entries {
                stmt.execute((
                    title.as_str(),
                    path.as_str(),
                    archive_type.as_str(),
                    *page_count,
                    *file_size,
                    *file_mtime,
                ))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 读取某根目录下所有档案的扫描元数据快照：(path, page_count, file_size, file_mtime)。
    pub fn scan_meta_for_root(&self, root: &str) -> Result<Vec<(String, i64, i64, i64)>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT path, page_count, file_size, file_mtime FROM archives")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        Ok(rows
            .filter_map(log_and_skip)
            .filter(|(path, _, _, _)| path_is_within(root, path))
            .collect())
    }

    /// 按 path 删除档案（供扫描清理孤儿档案；级联删除 pages/history/标签分类关联）。
    pub fn delete_archive_by_path(&self, path: &str) -> Result<usize> {
        self.conn()?
            .execute("DELETE FROM archives WHERE path = ?", [path])
    }

    // Bookmarks
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
            "UPDATE archives SET title = ?, title_auto = 0, updated_at = datetime('now') WHERE id = ?",
            (title, id),
        )
    }

    /// 列出需要按「初始标题层级」重生成的档案（自动派生标题且未被手动改名）
    pub fn list_auto_titled(&self) -> Result<Vec<(i64, String)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, path FROM archives WHERE title_auto = 1")?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(log_and_skip)
            .collect();
        Ok(rows)
    }

    /// 更新自动派生标题（保持 title_auto = 1）；title 未变化时不更新，返回是否变更。
    pub fn update_title_auto(&self, id: i64, title: &str) -> Result<bool> {
        let affected = self.conn()?.execute(
            "UPDATE archives SET title = ?, updated_at = datetime('now') WHERE id = ? AND title != ?",
            (title, id, title),
        )?;
        Ok(affected > 0)
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

    /// 按精确标题查询所有档案（供自动分组展开时拉取完整成员列表）
    pub fn get_archives_by_title(&self, title: &str) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at
             FROM archives WHERE title = ? COLLATE NOCASE ORDER BY path",
        )?;

        let archives = stmt
            .query_map([title], |row| {
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
    const MAX_CACHED_ARCHIVES: i64 = 30;

    pub fn set_thumbnail_path(&self, archive_id: i64, thumb_path: &str) -> Result<()> {
        self.conn()?.execute(
            "UPDATE archives SET thumbnail_path = ? WHERE id = ?",
            (thumb_path, archive_id),
        )?;
        Ok(())
    }

    /// 记录一次缩略图访问（由路由层节流调用），供 LRU 按真实使用时间淘汰。
    pub fn touch_thumbnail_access(&self, archive_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE archives SET thumb_accessed_at = datetime('now') WHERE id = ?",
            [archive_id],
        )
    }

    /// 有缩略图缓存的档案，按「最近访问 → updated_at」倒序排列（最近使用的留在缓存）。
    pub fn get_cached_archive_ids(&self) -> Result<Vec<i64>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.id FROM archives a
             WHERE a.thumbnail_path IS NOT NULL
             ORDER BY COALESCE(a.thumb_accessed_at, a.updated_at) DESC",
        )?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .filter_map(log_and_skip)
            .collect();
        Ok(ids)
    }

    /// 所有存活档案的 id 集合，用于清理不再被任何档案引用的缓存目录
    /// （extract/thumbnails 下的孤立子目录）。
    pub fn live_archive_ids(&self) -> Result<std::collections::HashSet<i64>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id FROM archives")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .filter_map(log_and_skip)
            .collect();
        Ok(ids)
    }

    /// 淘汰超出上限的最旧缩略图目录。`exclude_id` 为刚写入的档案时跳过它，
    /// 避免“注册后立刻把自己的目录淘汰掉”。
    pub fn evict_old_thumbnails(&self, exclude_id: Option<i64>) -> Result<Vec<(i64, String)>> {
        let cached_ids = self.get_cached_archive_ids()?;
        if cached_ids.len() as i64 <= Self::MAX_CACHED_ARCHIVES {
            return Ok(vec![]);
        }

        // 要淘汰的：超出限制的最旧条目
        let to_evict: Vec<i64> = cached_ids[Self::MAX_CACHED_ARCHIVES as usize..]
            .iter()
            .copied()
            .filter(|id| Some(*id) != exclude_id)
            .collect();
        let mut evicted = Vec::new();
        let conn = self.conn()?;

        for id in to_evict {
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

    /// 设置/清除手动封面页（cover_image 存档案内页面名；None 恢复默认首页）。
    pub fn set_archive_cover(&self, id: i64, cover: Option<&str>) -> Result<usize> {
        let conn = self.conn()?;
        match cover {
            Some(c) => conn.execute(
                "UPDATE archives SET cover_image = ?1 WHERE id = ?2",
                (c, id),
            ),
            None => conn.execute("UPDATE archives SET cover_image = NULL WHERE id = ?", [id]),
        }
    }

    /// 读取远程封面 URL（无则 None）。
    pub fn get_remote_cover(&self, id: i64) -> Result<Option<String>> {
        self.conn()?
            .query_row(
                "SELECT remote_cover FROM archives WHERE id = ?",
                [id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|v| v.flatten())
    }

    /// 设置/清除远程封面 URL（None 清除，恢复 页面封面/首页 的优先级）。
    pub fn set_remote_cover(&self, id: i64, url: Option<&str>) -> Result<usize> {
        let conn = self.conn()?;
        match url {
            Some(u) => conn.execute(
                "UPDATE archives SET remote_cover = ?1 WHERE id = ?2",
                (u, id),
            ),
            None => conn.execute("UPDATE archives SET remote_cover = NULL WHERE id = ?", [id]),
        }
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
        // 翻页时频繁调用；扫描等长事务可能短暂持锁，busy_timeout 后用重试兜底
        execute_with_busy_retry(|| {
            self.conn()?.execute(
                "INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at) VALUES (?, ?, ?, datetime('now'))",
                (archive_id, page_index, total_pages),
            )
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

        let mut stmt = conn.prepare(
            "SELECT title, path, archive_type, page_count, file_size, cover_image FROM archives",
        )?;
        let archives: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "title": row.get::<_, String>(0)?,
                    "path": row.get::<_, String>(1)?,
                    "archive_type": row.get::<_, String>(2)?,
                    "page_count": row.get::<_, i64>(3)?,
                    "file_size": row.get::<_, i64>(4)?,
                    "cover_image": row.get::<_, Option<String>>(5)?,
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

        let mut stmt = conn.prepare("SELECT name, color, search, pinned FROM categories")?;
        let categories: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "name": row.get::<_, String>(0)?,
                    "color": row.get::<_, String>(1)?,
                    "search": row.get::<_, String>(2)?,
                    "pinned": row.get::<_, bool>(3)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        // 关联关系与阅读历史以 path / namespace:name 为键导出，
        // 这样导入到新机器（id 不同）也能正确重建。
        let mut stmt = conn.prepare(
            "SELECT a.path, t.namespace, t.name
             FROM archive_tags at
             JOIN archives a ON a.id = at.archive_id
             JOIN tags t ON t.id = at.tag_id
             ORDER BY a.path, t.namespace, t.name",
        )?;
        let archive_tags: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "path": row.get::<_, String>(0)?,
                    "namespace": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare(
            "SELECT a.path, c.name
             FROM archive_categories ac
             JOIN archives a ON a.id = ac.archive_id
             JOIN categories c ON c.id = ac.category_id
             ORDER BY a.path, c.name",
        )?;
        let archive_categories: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "path": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare(
            "SELECT a.path, h.page_index, h.total_pages, h.updated_at
             FROM history h
             JOIN archives a ON a.id = h.archive_id",
        )?;
        let history: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "path": row.get::<_, String>(0)?,
                    "page_index": row.get::<_, i64>(1)?,
                    "total_pages": row.get::<_, i64>(2)?,
                    "updated_at": row.get::<_, String>(3)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare(
            "SELECT a.path, b.page_index
             FROM bookmarks b
             JOIN archives a ON a.id = b.archive_id
             ORDER BY a.path, b.page_index",
        )?;
        let bookmarks: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "path": row.get::<_, String>(0)?,
                    "page_index": row.get::<_, i64>(1)?,
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
            "archive_tags": archive_tags,
            "archive_categories": archive_categories,
            "history": history,
            "bookmarks": bookmarks,
            "settings": settings,
        }))
    }

    pub fn import_backup(&self, backup: &serde_json::Value) -> Result<()> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;

        // 导入档案：以 path 为键 upsert，绝不用 INSERT OR REPLACE——
        // REPLACE 会先 DELETE 再 INSERT，级联删掉该档案已有的 history/标签/分类关联。
        if let Some(archives) = backup["archives"].as_array() {
            for archive in archives {
                if let (Some(title), Some(path), Some(archive_type), Some(page_count)) = (
                    archive["title"].as_str(),
                    archive["path"].as_str(),
                    archive["archive_type"].as_str(),
                    archive["page_count"].as_i64(),
                ) {
                    tx.execute(
                        "INSERT INTO archives (title, path, archive_type, page_count, file_size)
                         VALUES (?, ?, ?, ?, ?)
                         ON CONFLICT(path) DO UPDATE SET
                            title = excluded.title,
                            archive_type = excluded.archive_type,
                            page_count = excluded.page_count,
                            file_size = excluded.file_size",
                        (
                            title,
                            path,
                            archive_type,
                            page_count,
                            archive["file_size"].as_i64().unwrap_or(0),
                        ),
                    )?;
                }
            }
        }

        // 导入标签（ns+name 唯一键）
        if let Some(tags) = backup["tags"].as_array() {
            for tag in tags {
                if let (Some(namespace), Some(name), Some(color)) = (
                    tag["namespace"].as_str(),
                    tag["name"].as_str(),
                    tag["color"].as_str(),
                ) {
                    tx.execute(
                        "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)
                         ON CONFLICT(namespace, name) DO UPDATE SET color = excluded.color",
                        (namespace, name, color),
                    )?;
                }
            }
        }

        // 导入分类（name 唯一键）
        if let Some(categories) = backup["categories"].as_array() {
            for category in categories {
                if let (Some(name), Some(color), Some(search)) = (
                    category["name"].as_str(),
                    category["color"].as_str(),
                    category["search"].as_str(),
                ) {
                    tx.execute(
                        "INSERT INTO categories (name, color, search, pinned) VALUES (?, ?, ?, ?)
                         ON CONFLICT(name) DO UPDATE SET
                            color = excluded.color,
                            search = excluded.search,
                            pinned = excluded.pinned",
                        (
                            name,
                            color,
                            search,
                            category["pinned"].as_bool().unwrap_or(false),
                        ),
                    )?;
                }
            }
        }

        // 重建档案-标签关联（按 path + ns:name 解析 id，存在性缺失的行自然忽略）
        if let Some(archive_tags) = backup["archive_tags"].as_array() {
            for at in archive_tags {
                if let (Some(path), Some(namespace), Some(name)) = (
                    at["path"].as_str(),
                    at["namespace"].as_str(),
                    at["name"].as_str(),
                ) {
                    tx.execute(
                        "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id)
                         SELECT a.id, t.id FROM archives a, tags t
                         WHERE a.path = ? AND t.namespace = ? AND t.name = ?",
                        (path, namespace, name),
                    )?;
                }
            }
        }

        // 重建档案-分类关联
        if let Some(archive_categories) = backup["archive_categories"].as_array() {
            for ac in archive_categories {
                if let (Some(path), Some(name)) = (ac["path"].as_str(), ac["name"].as_str()) {
                    tx.execute(
                        "INSERT OR IGNORE INTO archive_categories (archive_id, category_id)
                         SELECT a.id, c.id FROM archives a, categories c
                         WHERE a.path = ? AND c.name = ?",
                        (path, name),
                    )?;
                }
            }
        }

        // 导入阅读历史（按 path 解析档案 id，恢复断点续读位置）
        if let Some(history) = backup["history"].as_array() {
            for h in history {
                if let (Some(path), Some(page_index), Some(total_pages)) = (
                    h["path"].as_str(),
                    h["page_index"].as_i64(),
                    h["total_pages"].as_i64(),
                ) {
                    let updated_at = h["updated_at"].as_str().unwrap_or_default();
                    tx.execute(
                        "INSERT INTO history (archive_id, page_index, total_pages, updated_at)
                         SELECT a.id, ?1, ?2, ?3 FROM archives a WHERE a.path = ?4
                         ON CONFLICT(archive_id) DO UPDATE SET
                            page_index = excluded.page_index,
                            total_pages = excluded.total_pages,
                            updated_at = excluded.updated_at",
                        (page_index, total_pages, updated_at, path),
                    )?;
                }
            }
        }

        // 导入书签（按 path 解析档案 id；重复页会被 UNIQUE 忽略）
        if let Some(bookmarks) = backup["bookmarks"].as_array() {
            for b in bookmarks {
                if let (Some(path), Some(page_index)) =
                    (b["path"].as_str(), b["page_index"].as_i64())
                {
                    tx.execute(
                        "INSERT OR IGNORE INTO bookmarks (archive_id, page_index)
                         SELECT a.id, ?2 FROM archives a WHERE a.path = ?1",
                        (path, page_index),
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
    fn test_migrations_set_schema_version_and_are_skippable() {
        let db = setup_test_db(); // init() 内部会跑迁移
        let conn = db.conn_for_test().unwrap();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, crate::db::migrations::CURRENT_SCHEMA_VERSION);

        // 版本已达标时再跑一次 init 应短路、不报错
        db.init().unwrap();
    }

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
    fn test_list_archives_search_syntax() {
        let db = setup_test_db();

        let naruto = db
            .insert_archive("Naruto", "/path/naruto", "zip", 100, 5000)
            .unwrap();
        let naruto_2 = db
            .insert_archive("Naruto Shippuden", "/path/naruto2", "zip", 120, 6000)
            .unwrap();
        let one_piece = db
            .insert_archive("One Piece", "/path/onepiece", "zip", 200, 10000)
            .unwrap();

        let shonen = db.create_tag("genre", "shonen", "#ff0000").unwrap();
        let adventure = db.create_tag("", "adventure", "#00ff00").unwrap();
        db.assign_tag(naruto, shonen).unwrap();
        db.assign_tag(naruto_2, shonen).unwrap();
        db.assign_tag(one_piece, adventure).unwrap();

        // 普通关键词：匹配标题或标签名
        let r = db
            .list_archives(Some("shonen"), None, None, "title", "asc", 10, 0)
            .unwrap();
        assert_eq!(r.len(), 2);

        // tag:name —— 匹配特定标签
        let r = db
            .list_archives(Some("tag:adventure"), None, None, "title", "asc", 10, 0)
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].title, "One Piece");

        // tag:ns:name —— 带命名空间
        let r = db
            .list_archives(Some("tag:genre:shonen"), None, None, "title", "asc", 10, 0)
            .unwrap();
        assert_eq!(r.len(), 2);

        // -排除 —— 从标题和标签中排除
        let r = db
            .list_archives(Some("-Shippuden"), None, None, "title", "asc", 10, 0)
            .unwrap();
        assert!(r.iter().all(|a| a.title != "Naruto Shippuden"));

        // 多关键词 AND
        let r = db
            .list_archives(
                Some("Naruto tag:genre:shonen"),
                None,
                None,
                "title",
                "asc",
                10,
                0,
            )
            .unwrap();
        assert_eq!(r.len(), 2);
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
    fn test_list_archives_recent_read_sort_uses_history() {
        let db = setup_test_db();

        let alpha = db
            .insert_archive("Alpha", "/path/alpha", "zip", 10, 100)
            .unwrap();
        db.insert_archive("Beta", "/path/beta", "folder", 10, 100)
            .unwrap();

        // 把 Beta 的 updated_at 改到过去：Alpha 之后被读过（history.updated_at=now），
        // Beta 从未阅读（回退到过去式 updated_at）→ "最近阅读"排序 Alpha 必须在前。
        let conn = db.conn_for_test().unwrap();
        conn.execute(
            "UPDATE archives SET updated_at = datetime('now', '-1 day') WHERE title = 'Beta'",
            [],
        )
        .unwrap();
        drop(conn);

        db.save_history(alpha, 3, 10).unwrap();

        // 全量列表（书库主路径）按最近阅读排序
        let rows = db
            .list_archives_all(None, None, None, None, "updated", "desc")
            .unwrap();
        assert_eq!(rows[0].id, alpha, "最近读过的档案应排在第一位");
        assert_eq!(rows[1].title, "Beta");

        // 分页列表（OPDS 等路径）同样按最近阅读排序
        let rows = db
            .list_archives(None, None, None, "updated", "desc", 10, 0)
            .unwrap();
        assert_eq!(rows[0].id, alpha);

        // 其它排序方式不受 history 影响（如按名称）
        let rows = db
            .list_archives_all(None, None, None, None, "name", "asc")
            .unwrap();
        assert_eq!(rows[0].title, "Alpha");
        assert_eq!(rows[1].title, "Beta");
    }

    #[test]
    fn test_list_archives_read_filter_and_random_sort() {
        let db = setup_test_db();
        let a = db.insert_archive("Alpha", "/r/a", "zip", 10, 100).unwrap();
        db.insert_archive("Beta", "/r/b", "zip", 10, 100).unwrap();
        db.save_history(a, 3, 10).unwrap();

        let read = db
            .list_archives_all(None, None, None, Some("read"), "name", "asc")
            .unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].id, a);

        let unread = db
            .list_archives_all(None, None, None, Some("unread"), "name", "asc")
            .unwrap();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].title, "Beta");

        // 随机排序不崩溃且返回全集
        let random = db
            .list_archives_all(None, None, None, None, "random", "asc")
            .unwrap();
        assert_eq!(random.len(), 2);
    }

    #[test]
    fn test_get_history_for_archives_batch() {
        let db = setup_test_db();
        let a = db.insert_archive("A", "/a", "zip", 10, 100).unwrap();
        let b = db.insert_archive("B", "/b", "zip", 10, 100).unwrap();
        let c = db.insert_archive("C", "/c", "zip", 10, 100).unwrap();
        db.save_history(a, 2, 10).unwrap();
        db.save_history(c, 7, 10).unwrap();

        let progress = db.get_history_for_archives(&[a, b, c]).unwrap();
        assert_eq!(progress.len(), 2);
        assert!(progress.contains(&(a, 2)));
        assert!(progress.contains(&(c, 7)));

        let empty = db.get_history_for_archives(&[]).unwrap();
        assert!(empty.is_empty());
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

        // Add some data：档案 + 标签/分类绑定 + 阅读历史
        let a_id = db1
            .insert_archive("Manga A", "/path/a", "zip", 10, 500)
            .unwrap();
        let tag_id = db1.create_tag("", "favorite", "#ff0000").unwrap();
        let cat_id = db1.create_category("Action", "#00ff00", false, "").unwrap();
        db1.assign_tag(a_id, tag_id).unwrap();
        db1.assign_category(a_id, cat_id).unwrap();
        db1.save_history(a_id, 6, 10).unwrap();
        db1.add_bookmark(a_id, 4).unwrap();
        db1.add_bookmark(a_id, 8).unwrap();

        // Export backup
        let backup = db1.export_backup().unwrap();
        assert!(backup["archive_tags"].is_array());
        assert!(backup["archive_categories"].is_array());
        assert_eq!(backup["history"].as_array().unwrap().len(), 1);
        assert_eq!(backup["bookmarks"].as_array().unwrap().len(), 2);

        // Create new database and restore
        let db2 = setup_test_db();
        db2.import_backup(&backup).unwrap();

        // Verify archives / tags / categories
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

        // Verify history and associations survived（id 跨库不同，按 path/ns:name 重建）
        let restored_a = db2.get_archive_by_path("/path/a").unwrap().unwrap();
        let history = db2.get_history_for_archive(restored_a.id).unwrap().unwrap();
        assert_eq!(history.page_index, 6);
        assert_eq!(history.total_pages, 10);
        let restored_tags = db2.get_archive_tags(restored_a.id).unwrap();
        assert_eq!(restored_tags.len(), 1);
        assert_eq!(restored_tags[0].name, "favorite");
        let restored_cats = db2.get_archive_categories(restored_a.id).unwrap();
        assert_eq!(restored_cats.len(), 1);
        assert_eq!(restored_cats[0].name, "Action");

        // 书签同样按 path 重建
        assert_eq!(db2.list_bookmarks(restored_a.id).unwrap(), vec![4, 8]);
    }

    #[test]
    fn test_bookmark_crud() {
        let db = setup_test_db();
        let a = db
            .insert_archive("Manga A", "/path/a", "zip", 10, 500)
            .unwrap();

        db.add_bookmark(a, 3).unwrap();
        db.add_bookmark(a, 7).unwrap();
        db.add_bookmark(a, 3).unwrap(); // 重复页幂等
        assert_eq!(db.list_bookmarks(a).unwrap(), vec![3, 7]);

        db.remove_bookmark(a, 3).unwrap();
        assert_eq!(db.list_bookmarks(a).unwrap(), vec![7]);

        // 其它档案互不影响
        let b = db
            .insert_archive("Manga B", "/path/b", "zip", 5, 100)
            .unwrap();
        assert!(db.list_bookmarks(b).unwrap().is_empty());
    }

    #[test]
    fn test_set_archive_cover() {
        let db = setup_test_db();
        let a = db
            .insert_archive("Manga A", "/path/a", "zip", 10, 100)
            .unwrap();

        db.set_archive_cover(a, Some("page02.jpg")).unwrap();
        assert_eq!(
            db.get_archive(a).unwrap().unwrap().cover_image.as_deref(),
            Some("page02.jpg")
        );

        // 恢复默认
        db.set_archive_cover(a, None).unwrap();
        assert!(db.get_archive(a).unwrap().unwrap().cover_image.is_none());
    }

    #[test]
    fn test_remote_cover_set_get_clear() {
        let db = setup_test_db();
        let a = db
            .insert_archive("Manga A", "/path/a", "zip", 10, 100)
            .unwrap();

        assert!(db.get_remote_cover(a).unwrap().is_none());
        db.set_remote_cover(a, Some("https://example.com/c.jpg"))
            .unwrap();
        assert_eq!(
            db.get_remote_cover(a).unwrap().as_deref(),
            Some("https://example.com/c.jpg")
        );
        db.set_remote_cover(a, None).unwrap();
        assert!(db.get_remote_cover(a).unwrap().is_none());
    }

    #[test]
    fn test_restore_never_replaces_existing_archive() {
        // 回归：旧实现用 INSERT OR REPLACE 导入，先 DELETE 再 INSERT 会通过
        // ON DELETE CASCADE 删掉该 path 已有的 history/标签/分类。导入必须保留档案 id 与关联。
        let source = setup_test_db();
        let a_id = source
            .insert_archive("Manga A", "/path/a", "zip", 10, 500)
            .unwrap();
        let src_tag = source.create_tag("", "src", "#ff0000").unwrap();
        source.assign_tag(a_id, src_tag).unwrap();
        source.save_history(a_id, 8, 10).unwrap();
        let backup = source.export_backup().unwrap();

        // 目标库中同 path 档案已存在，且有自己的历史与标签
        let target = setup_test_db();
        let existing_id = target
            .insert_archive("Manga A", "/path/a", "folder", 20, 999)
            .unwrap();
        let keep_tag = target.create_tag("", "keep", "#00ff00").unwrap();
        target.assign_tag(existing_id, keep_tag).unwrap();
        target.save_history(existing_id, 2, 20).unwrap();

        target.import_backup(&backup).unwrap();

        // id 不变（未 DELETE），已有历史被备份值覆盖，已有标签被保留
        let after = target.get_archive_by_path("/path/a").unwrap().unwrap();
        assert_eq!(after.id, existing_id, "导入不应替换已存在档案");
        let history = target
            .get_history_for_archive(existing_id)
            .unwrap()
            .unwrap();
        assert_eq!(history.page_index, 8);
        assert_eq!(history.total_pages, 10);
        let tag_names: Vec<String> = target
            .get_archive_tags(existing_id)
            .unwrap()
            .iter()
            .map(|t| t.name.clone())
            .collect();
        assert!(tag_names.contains(&"keep".to_string()), "已有标签不应丢失");
        assert!(tag_names.contains(&"src".to_string()), "备份标签应被导入");
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

    #[test]
    fn test_get_archives_by_title() {
        let db = setup_test_db();

        db.insert_archive("海贼王", "/manhua/海贼王/01", "folder", 10, 100)
            .unwrap();
        db.insert_archive("海贼王", "/manhua/海贼王/02", "folder", 12, 120)
            .unwrap();
        db.insert_archive("火影忍者", "/manhua/火影忍者/01", "folder", 8, 80)
            .unwrap();

        let matches = db.get_archives_by_title("海贼王").unwrap();
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|a| a.title == "海贼王"));

        let none = db.get_archives_by_title("不存在").unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn test_get_archives_by_title_case_insensitive() {
        let db = setup_test_db();

        db.insert_archive("One Piece", "/manhua/one-piece/01", "folder", 10, 100)
            .unwrap();
        db.insert_archive("one piece", "/manhua/one-piece/02", "folder", 12, 120)
            .unwrap();
        db.insert_archive("One Punch", "/manhua/one-punch/01", "folder", 8, 80)
            .unwrap();

        let matches = db.get_archives_by_title("ONE PIECE").unwrap();
        assert_eq!(matches.len(), 2);
        assert!(matches
            .iter()
            .all(|a| a.title.to_lowercase() == "one piece"));
    }

    #[test]
    fn test_auto_title_regeneration() {
        let db = setup_test_db();

        let id = db
            .insert_archive("auto-title", "/path/to/manhua01/第一章", "folder", 5, 100)
            .unwrap();

        // 新插入的档案默认标记为自动标题
        let auto = db.list_auto_titled().unwrap();
        assert_eq!(auto.len(), 1);
        assert_eq!(auto[0].0, id);
        assert_eq!(auto[0].1, "/path/to/manhua01/第一章");

        // 更新为不同标题时返回 true
        assert!(db.update_title_auto(id, "第一章").unwrap());
        // 更新为相同标题时返回 false（未变化）
        assert!(!db.update_title_auto(id, "第一章").unwrap());

        // 手动改名后不再属于自动标题
        db.update_archive_title(id, "手动改名").unwrap();
        assert!(db.list_auto_titled().unwrap().is_empty());
    }

    #[test]
    fn test_migration_marks_existing_archives_as_not_auto() {
        // 模拟旧版数据库：archives 表没有 title_auto 列且已有数据
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();
        {
            let conn = rusqlite::Connection::open(path).unwrap();
            conn.execute_batch(
                "CREATE TABLE archives (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    path TEXT NOT NULL UNIQUE,
                    archive_type TEXT NOT NULL DEFAULT 'folder',
                    page_count INTEGER DEFAULT 0,
                    cover_image TEXT,
                    file_size INTEGER DEFAULT 0,
                    thumbnail_path TEXT,
                    group_id INTEGER,
                    page_list_mtime INTEGER DEFAULT 0,
                    created_at TEXT DEFAULT (datetime('now')),
                    updated_at TEXT DEFAULT (datetime('now'))
                );
                INSERT INTO archives (title, path, archive_type) VALUES ('旧标题', '/old/path', 'folder');",
            )
            .unwrap();
        }

        let db = Database::new(path).unwrap();
        db.init().unwrap();

        // 迁移后列存在，且已有行被保守标记为 0（不参与批量重生成）
        let conn = db.conn_for_test().unwrap();
        let auto: i64 = conn
            .query_row(
                "SELECT title_auto FROM archives WHERE path = '/old/path'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(auto, 0);
    }

    #[test]
    fn test_pool_connections_set_busy_timeout() {
        let db = setup_test_db();
        let conn = db.conn_for_test().unwrap();
        // with_init 里设置的 busy_timeout 必须生效（毫秒）
        let timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(timeout, 5000);
    }

    #[test]
    fn test_execute_with_busy_retry_recovers_from_transient_busy() {
        let mut calls = 0;
        let result = execute_with_busy_retry(|| {
            calls += 1;
            if calls <= 2 {
                Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                    None,
                ))
            } else {
                Ok(42usize)
            }
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls, 3, "前两次 busy 应触发重试");

        // 非 busy 错误不重试，直接返回
        let non_busy: Result<usize> =
            execute_with_busy_retry(|| Err(rusqlite::Error::QueryReturnedNoRows));
        assert!(matches!(
            non_busy,
            Err(rusqlite::Error::QueryReturnedNoRows)
        ));
    }

    #[test]
    fn test_upsert_scanned_archive_inserts_then_updates_in_place() {
        let db = setup_test_db();
        let id1 = db
            .upsert_scanned_archive("Title", "/root/a.cbz", "cbz", 10, 100, 1111)
            .unwrap();

        // 已绑定的历史必须在再次 upsert（内容更新）后保留——不允许 REPLACE 级联删除
        db.save_history(id1, 3, 10).unwrap();

        let id2 = db
            .upsert_scanned_archive("Title2", "/root/a.cbz", "cbz", 12, 120, 2222)
            .unwrap();
        assert_eq!(id1, id2, "upsert 必须保持同一行/同一 id");

        let a = db.get_archive(id1).unwrap().unwrap();
        assert_eq!(a.title, "Title2");
        assert_eq!(a.page_count, 12);
        assert_eq!(a.file_size, 120);

        let h = db.get_history_for_archive(id1).unwrap().unwrap();
        assert_eq!(h.page_index, 3, "upsert 不应清掉阅读历史");
    }

    #[test]
    fn test_scan_meta_root_filter_and_delete_orphan_by_path() {
        let db = setup_test_db();
        db.upsert_scanned_archive("In", "/root/a", "folder", 5, 1, 111)
            .unwrap();
        db.upsert_scanned_archive("Out", "/other/b", "folder", 5, 1, 222)
            .unwrap();
        // 前缀安全：/root-x 不应被算进 /root
        db.upsert_scanned_archive("Trap", "/root-x/c", "folder", 5, 1, 333)
            .unwrap();

        let meta = db.scan_meta_for_root("/root").unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].0, "/root/a");
        assert_eq!(meta[0].3, 111);

        db.delete_archive_by_path("/root/a").unwrap();
        assert!(db.get_archive_by_path("/root/a").unwrap().is_none());
        assert!(db.get_archive_by_path("/other/b").unwrap().is_some());
        assert!(db.get_archive_by_path("/root-x/c").unwrap().is_some());
    }

    #[test]
    fn test_scan_meta_root_component_level_prefix() {
        let db = setup_test_db();
        // 目录名互为字符串前缀（海贼 vs 海贼王）：组件级比较下互不归属
        db.upsert_scanned_archive("海贼", "/manhua/海贼", "folder", 5, 1, 111)
            .unwrap();
        db.upsert_scanned_archive("海贼王01", "/manhua/海贼王/01", "folder", 5, 1, 222)
            .unwrap();
        db.upsert_scanned_archive("海贼王02", "/manhua/海贼王/02", "folder", 5, 1, 333)
            .unwrap();

        // 扫描 /manhua/海贼：只有路径恰为 /manhua/海贼 的记录属于它
        let meta = db.scan_meta_for_root("/manhua/海贼").unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].0, "/manhua/海贼");

        // 扫描 /manhua：三者都在其下
        assert_eq!(db.scan_meta_for_root("/manhua").unwrap().len(), 3);
    }
}
