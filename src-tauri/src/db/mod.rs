pub mod schema;
pub mod migrations;

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

    pub fn list_archives(&self, search: Option<&str>, sort: &str, order: &str, limit: i64, offset: i64) -> Result<(Vec<ArchiveRow>, i64)> {
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
            |row| row.get(0)
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
        let archives = stmt.query_map(
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
            }
        )?.filter_map(|r| r.ok()).collect();

        Ok((archives, total))
    }

    pub fn insert_archive(&self, title: &str, path: &str, archive_type: &str, page_count: i64, file_size: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO archives (title, path, archive_type, page_count, file_size) VALUES (?, ?, ?, ?, ?)",
            (title, path, archive_type, page_count, file_size),
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_archive(&self, id: i64) -> Result<usize> {
        self.conn.execute("DELETE FROM archives WHERE id = ?", [id])
    }

    // Tag operations
    pub fn list_tags(&self) -> Result<Vec<TagRow>> {
        let mut stmt = self.conn.prepare("SELECT id, namespace, name, color FROM tags ORDER BY namespace, name")?;
        let tags = stmt.query_map([], |row| {
            Ok(TagRow {
                id: row.get(0)?,
                namespace: row.get(1)?,
                name: row.get(2)?,
                color: row.get(3)?,
            })
        })?.filter_map(|r| r.ok()).collect();
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
        let mut stmt = self.conn.prepare("SELECT DISTINCT namespace FROM tags WHERE namespace != '' ORDER BY namespace")?;
        let namespaces = stmt.query_map([], |row| row.get(0))?.filter_map(|r| r.ok()).collect();
        Ok(namespaces)
    }

    // Category operations
    pub fn list_categories(&self) -> Result<Vec<CategoryRow>> {
        let mut stmt = self.conn.prepare("SELECT id, name, color, pinned, search, created_at FROM categories ORDER BY name")?;
        let categories = stmt.query_map([], |row| {
            Ok(CategoryRow {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                pinned: row.get::<_, i64>(3)? != 0,
                search: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(categories)
    }

    pub fn create_category(&self, name: &str, color: &str, pinned: bool, search: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO categories (name, color, pinned, search) VALUES (?, ?, ?, ?)",
            (name, color, pinned as i64, search),
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_category(&self, id: i64) -> Result<usize> {
        self.conn.execute("DELETE FROM categories WHERE id = ?", [id])
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
    pub fn get_history(&self) -> Result<Vec<(HistoryRow, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT h.archive_id, h.page_index, h.total_pages, h.updated_at, a.title, a.path
             FROM history h
             JOIN archives a ON a.id = h.archive_id
             ORDER BY h.updated_at DESC"
        )?;
        
        let history = stmt.query_map([], |row| {
            Ok((
                HistoryRow {
                    archive_id: row.get(0)?,
                    page_index: row.get(1)?,
                    total_pages: row.get(2)?,
                    updated_at: row.get(3)?,
                },
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?.filter_map(|r| r.ok()).collect();
        
        Ok(history)
    }

    pub fn save_history(&self, archive_id: i64, page_index: i64, total_pages: i64) -> Result<usize> {
        self.conn.execute(
            "INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at) VALUES (?, ?, ?, datetime('now'))",
            (archive_id, page_index, total_pages),
        )
    }

    pub fn delete_history(&self, archive_id: i64) -> Result<usize> {
        self.conn.execute("DELETE FROM history WHERE archive_id = ?", [archive_id])
    }

    pub fn clear_history(&self) -> Result<usize> {
        self.conn.execute("DELETE FROM history", [])
    }

    // Settings operations
    pub fn get_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let settings = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?.filter_map(|r| r.ok()).collect();
        Ok(settings)
    }

    pub fn update_settings(&self, settings: &std::collections::HashMap<String, String>) -> Result<()> {
        for (key, value) in settings {
            self.conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
                (key, value),
            )?;
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<String> {
        self.conn.query_row(
            "SELECT value FROM settings WHERE key = ?",
            [key],
            |row| row.get(0),
        )
    }

    // Stats
    pub fn get_stats(&self) -> Result<serde_json::Value> {
        let total_archives: i64 = self.conn.query_row("SELECT COUNT(*) FROM archives", [], |row| row.get(0))?;
        let total_pages: i64 = self.conn.query_row("SELECT COALESCE(SUM(page_count), 0) FROM archives", [], |row| row.get(0))?;
        let total_tags: i64 = self.conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        let total_categories: i64 = self.conn.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?;
        let history_count: i64 = self.conn.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;
        
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
        let mut stmt = self.conn.prepare("SELECT title, path, archive_type, page_count FROM archives")?;
        let archives: Vec<serde_json::Value> = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "title": row.get::<_, String>(0)?,
                "path": row.get::<_, String>(1)?,
                "archive_type": row.get::<_, String>(2)?,
                "page_count": row.get::<_, i64>(3)?,
            }))
        })?.filter_map(|r| r.ok()).collect();
        
        let mut stmt = self.conn.prepare("SELECT namespace, name, color FROM tags")?;
        let tags: Vec<serde_json::Value> = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "namespace": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "color": row.get::<_, String>(2)?,
            }))
        })?.filter_map(|r| r.ok()).collect();
        
        let mut stmt = self.conn.prepare("SELECT name, color, search FROM categories")?;
        let categories: Vec<serde_json::Value> = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "name": row.get::<_, String>(0)?,
                "color": row.get::<_, String>(1)?,
                "search": row.get::<_, String>(2)?,
            }))
        })?.filter_map(|r| r.ok()).collect();
        
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
