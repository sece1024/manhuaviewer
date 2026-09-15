//! 备份/恢复与跨机同步清单：把全库序列化/反序列化为 JSON。
//! 导入以 path / ns:name / name 为键重建关联，跨机器 id 不同也能对齐。

use rusqlite::Result;

use super::{log_and_skip, Database};

impl Database {
    pub fn export_backup(&self) -> Result<serde_json::Value> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, file_size, cover_image FROM archives",
        )?;
        let archives: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "path": row.get::<_, String>(2)?,
                    "archive_type": row.get::<_, String>(3)?,
                    "page_count": row.get::<_, i64>(4)?,
                    "file_size": row.get::<_, i64>(5)?,
                    "cover_image": row.get::<_, Option<String>>(6)?,
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

        let mut settings = self.get_settings()?;
        // 敏感设置不进备份文件：server_token 是口令，server_bind 决定监听面；
        // 恶意备份导入这两项可清空口令并开 0.0.0.0（见 import_backup 的对应排除）。
        settings.retain(|key, _| key != "server_token" && key != "server_bind");

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

    /// 同步专用清单：只含同步客户端需要且不含主机路径的字段，
    /// 关联关系一律以 title 为键（跨机路径必然不同）。
    /// 字段刻意避开 `path` 等被局域网响应脱敏中间件剔除的名字。
    pub fn sync_manifest(&self) -> Result<serde_json::Value> {
        let conn = self.conn()?;

        let mut stmt = conn.prepare(
            "SELECT id, title, archive_type, page_count, file_size, file_mtime FROM archives",
        )?;
        let archives: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "archive_type": row.get::<_, String>(2)?,
                    "page_count": row.get::<_, i64>(3)?,
                    "file_size": row.get::<_, i64>(4)?,
                    "file_mtime": row.get::<_, i64>(5)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare("SELECT id, namespace, name, color FROM tags")?;
        let tags: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "namespace": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                    "color": row.get::<_, String>(3)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare("SELECT id, name, color, pinned, search FROM categories")?;
        let categories: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "color": row.get::<_, String>(2)?,
                    "pinned": row.get::<_, bool>(3)?,
                    "search": row.get::<_, String>(4)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare(
            "SELECT a.title, t.namespace, t.name
             FROM archive_tags at
             JOIN archives a ON a.id = at.archive_id
             JOIN tags t ON t.id = at.tag_id
             ORDER BY a.title, t.namespace, t.name",
        )?;
        let archive_tags: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "title": row.get::<_, String>(0)?,
                    "namespace": row.get::<_, String>(1)?,
                    "name": row.get::<_, String>(2)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare(
            "SELECT a.title, c.name
             FROM archive_categories ac
             JOIN archives a ON a.id = ac.archive_id
             JOIN categories c ON c.id = ac.category_id
             ORDER BY a.title, c.name",
        )?;
        let archive_categories: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "title": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        let mut stmt = conn.prepare(
            "SELECT a.title, h.page_index, h.total_pages
             FROM history h JOIN archives a ON a.id = h.archive_id",
        )?;
        let history: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "title": row.get::<_, String>(0)?,
                    "page_index": row.get::<_, i64>(1)?,
                    "total_pages": row.get::<_, i64>(2)?,
                }))
            })?
            .filter_map(log_and_skip)
            .collect();

        Ok(serde_json::json!({
            "archives": archives,
            "tags": tags,
            "categories": categories,
            "archive_tags": archive_tags,
            "archive_categories": archive_categories,
            "history": history,
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

        // Import settings（排除敏感项：防恶意备份把 server_bind 设 0.0.0.0 / 清空口令）
        if let Some(settings) = backup["settings"].as_object() {
            for (key, value) in settings {
                if key == "server_token" || key == "server_bind" {
                    continue;
                }
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
