pub mod archives;
pub mod backup;
pub mod bookmarks;
pub mod categories;
pub mod history;
pub mod migrations;
pub mod schema;
pub mod settings;
pub mod tags;

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Result;
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

/// 路径的父目录（去掉尾部 `/`/`\`）；无父目录时为空串。
/// 与 routes 层展示/自动分组用的 `parent_dir_of` 语义一致。
fn parent_dir_of_path(path: &str) -> String {
    std::path::Path::new(path)
        .parent()
        .map(|p| {
            p.to_string_lossy()
                .trim_end_matches(['/', '\\'])
                .to_string()
        })
        .unwrap_or_default()
}

/// 列表分组键：永久组 `G{group_id}`；否则 `A{父目录}\0{小写标题}`。
/// 与 routes 层 `group_archives` 的分组规则一致——注册为 SQL 标量函数后即可在
/// SQL 内按组聚合分页，无需把整表拉回内存。
fn archive_group_key(path: &str, title: &str, group_id: Option<i64>) -> String {
    match group_id {
        Some(g) => format!("G{g}"),
        None => format!("A{}\u{0}{}", parent_dir_of_path(path), title.to_lowercase()),
    }
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

/// 同 `archive_row`，额外读取第 12 列 `remote_cover`。
fn archive_row_with_remote_cover(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(ArchiveRow, Option<String>)> {
    Ok((archive_row(row)?, row.get(11)?))
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

/// 服务端分组分页返回的一行：组代表档案 + 该组成员数。
#[derive(Debug, Clone)]
pub struct GroupedArchiveRow {
    pub archive: ArchiveRow,
    pub chapter_count: i64,
}

/// 排序方式 → ORDER BY 表达式。"updated"（最近阅读）优先按阅读时间排序：
/// 用冗余列 last_read_at（由 save_history 同步写），从未读过的回退到 archives.updated_at，
/// 这样"最近阅读"排序不依赖 history JOIN，且能走索引。
fn order_expr_for(sort: &str) -> &'static str {
    match sort {
        "name" | "title" => "a.title",
        "created" => "a.created_at",
        "pages" => "a.page_count",
        "size" => "a.file_size",
        "updated" => "COALESCE(a.last_read_at, a.updated_at)",
        "random" => "RANDOM()",
        _ => "a.updated_at",
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
            // 列表分组分页用的分组键函数（与 routes::group_archives 规则一致）
            conn.create_scalar_function(
                "archive_group_key",
                3,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8
                    | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
                |ctx| {
                    let path: String = ctx.get(0)?;
                    let title: String = ctx.get(1)?;
                    let group_id: Option<i64> = ctx.get(2)?;
                    Ok(archive_group_key(&path, &title, group_id))
                },
            )?;
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
            ("card_density", "normal"),
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

    /// 服务端分组分页：自动组按「同父目录 + 同标题」合并，且不影响不同父目录的同名档案。
    #[test]
    fn test_list_archives_grouped_page_auto_group() {
        let db = setup_test_db();
        db.insert_archive("海贼王", "/manhua/hzw/01", "folder", 10, 100)
            .unwrap();
        db.insert_archive("海贼王", "/manhua/hzw/02", "folder", 12, 100)
            .unwrap();
        // 同标题但父目录不同：不得合并
        db.insert_archive("海贼王", "/other/hzw/01", "folder", 5, 100)
            .unwrap();
        db.insert_archive("火影", "/manhua/hyr", "folder", 8, 100)
            .unwrap();

        let groups = db
            .list_archives_grouped_page(None, None, None, None, "name", "asc", 50, 0)
            .unwrap();
        assert_eq!(groups.len(), 3, "自动组应把 2 话合并为 1 项");

        let hzw = groups
            .iter()
            .find(|g| g.archive.path.starts_with("/manhua/hzw/"))
            .unwrap();
        assert_eq!(hzw.chapter_count, 2);
        assert_eq!(hzw.archive.title, "海贼王");

        let other = groups
            .iter()
            .find(|g| g.archive.path.starts_with("/other/"))
            .unwrap();
        assert_eq!(other.chapter_count, 1, "不同父目录不合并");
    }

    /// 永久合并组：代表行取主档案（id == group_id），并对「组」整体分页。
    #[test]
    fn test_list_archives_grouped_page_manual_group_and_paging() {
        let db = setup_test_db();
        let a = db.insert_archive("A", "/g/a", "zip", 10, 100).unwrap();
        let b = db.insert_archive("B", "/g/b", "zip", 10, 100).unwrap();
        let c = db.insert_archive("C", "/g/c", "zip", 10, 100).unwrap();
        let d = db.insert_archive("D", "/g/d", "zip", 10, 100).unwrap();
        assert_eq!(db.merge_archives(&[a, b]).unwrap(), a);

        // 名称升序：A 组（成员 A/B，取 MIN title = "A"）最靠前，随后 C、D
        let page1 = db
            .list_archives_grouped_page(None, None, None, None, "name", "asc", 1, 0)
            .unwrap();
        assert_eq!(page1.len(), 1);
        assert_eq!(page1[0].archive.id, a, "永久组代表应为主档案");
        assert_eq!(page1[0].chapter_count, 2);

        let page2 = db
            .list_archives_grouped_page(None, None, None, None, "name", "asc", 1, 1)
            .unwrap();
        assert_eq!(page2[0].archive.id, c);

        let page3 = db
            .list_archives_grouped_page(None, None, None, None, "name", "asc", 1, 2)
            .unwrap();
        assert_eq!(page3[0].archive.id, d);

        // 越界页返回空
        let empty = db
            .list_archives_grouped_page(None, None, None, None, "name", "asc", 1, 5)
            .unwrap();
        assert!(empty.is_empty());
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
        assert_eq!(stats["total_size"], 1500); // file_size 500 + 1000
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
