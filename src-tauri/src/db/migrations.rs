use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Check for legacy 'folders' table and migrate
    let has_folders: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='folders'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    ).unwrap_or(false);

    if has_folders {
        tracing::info!("检测到旧版数据表，执行迁移...");
        
        // Migrate folders to archives
        let mut stmt = conn.prepare("SELECT * FROM folders")?;
        let folders = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>("name").or_else(|_| row.get::<_, String>("title")).unwrap_or_default(),
                row.get::<_, String>("path")?,
                row.get::<_, i64>("image_count").or_else(|_| row.get::<_, i64>("page_count")).unwrap_or(0),
            ))
        })?;

        let mut migrated = 0;
        for folder in folders {
            if let Ok((name, path, page_count)) = folder {
                if let Err(e) = conn.execute(
                    "INSERT OR IGNORE INTO archives (title, path, archive_type, page_count, created_at) VALUES (?1, ?2, 'folder', ?3, datetime('now'))",
                    (&name, &path, page_count),
                ) {
                    tracing::debug!("迁移文件夹失败: {} — {}", path, e);
                } else {
                    migrated += 1;
                }
            }
        }
        
        tracing::info!("迁移了 {} 个文件夹", migrated);
    }

    // Check for legacy folder_tags table
    let has_folder_tags: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='folder_tags'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    ).unwrap_or(false);

    if has_folder_tags {
        tracing::info!("检测到旧版 folder_tags，迁移到 archive_tags...");
        
        conn.execute_batch(
            "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id)
             SELECT a.id, ft.tag_id FROM folder_tags ft
             JOIN folders f ON f.id = ft.folder_id
             JOIN archives a ON a.path = f.path;
             DROP TABLE IF EXISTS folder_tags;"
        )?;
        
        tracing::info!("folder_tags 迁移完成");
    }

    // Check for legacy read_history table
    let has_read_history: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='read_history'",
        [],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    ).unwrap_or(false);

    if has_read_history {
        tracing::info!("检测到旧版 read_history，迁移到 history...");
        
        conn.execute_batch(
            "INSERT OR IGNORE INTO history (archive_id, page_index, total_pages, updated_at)
             SELECT a.id, rh.page_index, rh.total_pages, datetime('now')
             FROM read_history rh
             JOIN folders f ON f.id = rh.folder_id
             JOIN archives a ON a.path = f.path;
             DROP TABLE IF EXISTS read_history;"
        )?;
        
        tracing::info!("read_history 迁移完成");
    }

    Ok(())
}
