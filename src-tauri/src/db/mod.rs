pub mod migrations;
pub mod schema;

use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRow {
    pub id: i64,
    pub title: String,
    pub path: String,
    pub archive_type: String,
    pub page_count: i64,
    pub cover_image: Option<String>,
    pub file_size: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRow {
    pub id: i64,
    pub namespace: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub pinned: bool,
    pub search: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRow {
    pub archive_id: i64,
    pub page_index: i64,
    pub total_pages: i64,
    pub updated_at: String,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn })
    }

    pub fn init(&self) -> Result<()> {
        self.conn.execute_batch(schema::SCHEMA)?;
        migrations::run_migrations(&self.conn)?;
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
            ("page_direction", "rtl"),
            ("theme", "dark"),
        ];

        for (key, value) in defaults {
            self.conn.execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                (key, value),
            )?;
        }

        Ok(())
    }

    pub fn get_conn(&self) -> &Connection {
        &self.conn
    }

    // Archive operations
    pub fn get_archive(&self, id: i64) -> Result<Option<ArchiveRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, created_at, updated_at FROM archives WHERE id = ?"
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
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn get_archive_by_path(&self, path: &str) -> Result<Option<ArchiveRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, created_at, updated_at FROM archives WHERE path = ?"
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
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_archives(
        &self,
        search: Option<&str>,
        sort: &str,
        order: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ArchiveRow>, i64)> {
        let mut where_clause = String::from("WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = search {
            if !s.is_empty() {
                where_clause.push_str(" AND title LIKE ?");
                params.push(Box::new(format!("%{}%", s)));
            }
        }

        // Get total count
        let count_sql = format!("SELECT COUNT(*) FROM archives {}", where_clause);
        let total: i64 = self.conn.query_row(
            &count_sql,
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        // Build main query
        let order_clause = match sort {
            "name" => "title",
            "created" => "created_at",
            "pages" => "page_count",
            "size" => "file_size",
            _ => "updated_at",
        };
        let direction = if order == "asc" { "ASC" } else { "DESC" };

        let sql = format!(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, created_at, updated_at FROM archives {} ORDER BY {} {} LIMIT ? OFFSET ?",
            where_clause, order_clause, direction
        );

        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = self.conn.prepare(&sql)?;
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
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok((archives, total))
    }

    pub fn insert_archive(
        &self,
        title: &str,
        path: &str,
        archive_type: &str,
        page_count: i64,
        file_size: i64,
    ) -> Result<i64> {
        // Check for existing archive with the same path first
        let existing: Option<i64> = self
            .conn
            .query_row("SELECT id FROM archives WHERE path = ?", [path], |row| {
                row.get(0)
            })
            .ok();

        if let Some(id) = existing {
            return Ok(id);
        }

        self.conn.execute(
            "INSERT INTO archives (title, path, archive_type, page_count, file_size) VALUES (?, ?, ?, ?, ?)",
            (title, path, archive_type, page_count, file_size),
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_archive(&self, id: i64) -> Result<usize> {
        self.conn.execute("DELETE FROM archives WHERE id = ?", [id])
    }

    // Tag operations
    pub fn list_tags(&self) -> Result<Vec<TagRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, namespace, name, color FROM tags ORDER BY namespace, name")?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagRow {
                    id: row.get(0)?,
                    namespace: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tags)
    }

    pub fn create_tag(&self, namespace: &str, name: &str, color: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)",
            (namespace, name, color),
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_tag(&self, id: i64) -> Result<usize> {
        self.conn.execute("DELETE FROM tags WHERE id = ?", [id])
    }

    pub fn assign_tag(&self, archive_id: i64, tag_id: i64) -> Result<usize> {
        self.conn.execute(
            "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
            (archive_id, tag_id),
        )
    }

    pub fn remove_tag(&self, archive_id: i64, tag_id: i64) -> Result<usize> {
        self.conn.execute(
            "DELETE FROM archive_tags WHERE archive_id = ? AND tag_id = ?",
            (archive_id, tag_id),
        )
    }

    pub fn list_namespaces(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT namespace FROM tags WHERE namespace != '' ORDER BY namespace",
        )?;
        let namespaces = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(namespaces)
    }

    pub fn get_archive_tags(&self, archive_id: i64) -> Result<Vec<TagRow>> {
        let mut stmt = self.conn.prepare(
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
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(tags)
    }

    // Category operations
    pub fn list_categories(&self) -> Result<Vec<CategoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, pinned, search, created_at FROM categories ORDER BY name",
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
                })
            })?
            .filter_map(|r| r.ok())
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
        self.conn.execute(
            "INSERT INTO categories (name, color, pinned, search) VALUES (?, ?, ?, ?)",
            (name, color, pinned as i64, search),
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_category(&self, id: i64) -> Result<usize> {
        self.conn
            .execute("DELETE FROM categories WHERE id = ?", [id])
    }

    pub fn assign_category(&self, archive_id: i64, category_id: i64) -> Result<usize> {
        self.conn.execute(
            "INSERT OR IGNORE INTO archive_categories (archive_id, category_id) VALUES (?, ?)",
            (archive_id, category_id),
        )
    }

    pub fn remove_category(&self, archive_id: i64, category_id: i64) -> Result<usize> {
        self.conn.execute(
            "DELETE FROM archive_categories WHERE archive_id = ? AND category_id = ?",
            (archive_id, category_id),
        )
    }

    // History operations
    pub fn get_history(&self) -> Result<Vec<(HistoryRow, String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT h.archive_id, h.page_index, h.total_pages, h.updated_at, a.title, a.path, a.archive_type
             FROM history h
             JOIN archives a ON a.id = h.archive_id
             ORDER BY h.updated_at DESC"
        )?;

        let history = stmt
            .query_map([], |row| {
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
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(history)
    }

    pub fn save_history(
        &self,
        archive_id: i64,
        page_index: i64,
        total_pages: i64,
    ) -> Result<usize> {
        self.conn.execute(
            "INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at) VALUES (?, ?, ?, datetime('now'))",
            (archive_id, page_index, total_pages),
        )
    }

    pub fn delete_history(&self, archive_id: i64) -> Result<usize> {
        self.conn
            .execute("DELETE FROM history WHERE archive_id = ?", [archive_id])
    }

    pub fn clear_history(&self) -> Result<usize> {
        self.conn.execute("DELETE FROM history", [])
    }

    // Settings operations
    pub fn get_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let settings = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(settings)
    }

    pub fn update_settings(
        &self,
        settings: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        for (key, value) in settings {
            self.conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
                (key, value),
            )?;
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?", [key], |row| {
                row.get(0)
            })
    }

    // Stats
    pub fn get_stats(&self) -> Result<serde_json::Value> {
        let total_archives: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM archives", [], |row| row.get(0))?;
        let total_pages: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(page_count), 0) FROM archives",
            [],
            |row| row.get(0),
        )?;
        let total_tags: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        let total_categories: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?;
        let history_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;

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
        let mut stmt = self
            .conn
            .prepare("SELECT title, path, archive_type, page_count FROM archives")?;
        let archives: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "title": row.get::<_, String>(0)?,
                    "path": row.get::<_, String>(1)?,
                    "archive_type": row.get::<_, String>(2)?,
                    "page_count": row.get::<_, i64>(3)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self
            .conn
            .prepare("SELECT namespace, name, color FROM tags")?;
        let tags: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "namespace": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "color": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self
            .conn
            .prepare("SELECT name, color, search FROM categories")?;
        let categories: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "name": row.get::<_, String>(0)?,
                    "color": row.get::<_, String>(1)?,
                    "search": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let settings = self.get_settings()?;

        Ok(serde_json::json!({
            "version": "3.0.0",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "archives": archives,
            "tags": tags,
            "categories": categories,
            "settings": settings,
        }))
    }

    pub fn import_backup(&self, backup: &serde_json::Value) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

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
        let conn = db.get_conn();

        // Verify tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
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

        let (archives, total) = db.list_archives(None, "title", "asc", 10, 0).unwrap();
        assert_eq!(total, 3);
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

        let (archives, total) = db
            .list_archives(Some("Naruto"), "title", "asc", 10, 0)
            .unwrap();
        assert_eq!(total, 1);
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
        let history = db.get_history().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0.archive_id, archive_id);
        assert_eq!(history[0].0.page_index, 5);
        assert_eq!(history[0].0.total_pages, 10);

        // Delete history
        db.delete_history(archive_id).unwrap();
        let history = db.get_history().unwrap();
        assert_eq!(history.len(), 0);
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
        let (archives, _) = db2.list_archives(None, "title", "asc", 10, 0).unwrap();
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
