use crate::db::Database;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 备份文件前缀，用于识别与保留轮换。
pub const BACKUP_PREFIX: &str = "manhuaviewer-backup-";
/// 保留的最近备份数量下限。
pub const DEFAULT_KEEP: usize = 10;

/// 读取“保留份数”设置（>=1）。
pub fn read_keep(db: &Database) -> usize {
    db.get_setting("backup_keep")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_KEEP as i64)
        .max(1) as usize
}

/// 是否到点需要备份：interval_hours <= 0 表示关闭；无历史备份则立即；否则看距最近一次是否达标。
pub fn due(last_backup_unix: Option<i64>, now_unix: i64, interval_hours: i64) -> bool {
    if interval_hours <= 0 {
        return false;
    }
    match last_backup_unix {
        None => true,
        Some(last) => now_unix - last >= interval_hours * 3600,
    }
}

/// 备份输出目录：<data_dir>/backups
pub fn backup_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// 目录内按文件名（时间戳）升序排列的备份文件。
pub fn list_backup_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            let is_backup = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(BACKUP_PREFIX))
                .unwrap_or(false);
            if p.is_file() && is_backup {
                files.push(p);
            }
        }
    }
    files.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    });
    files
}

fn newest_backup_mtime(dir: &Path) -> Option<i64> {
    list_backup_files(dir)
        .iter()
        .map(|p| super::fs_ext::mtime_secs(p))
        .max()
}

/// 执行一次备份并做保留轮换，返回新备份文件路径。
pub fn perform_backup(db: &Database, data_dir: &Path) -> Result<PathBuf> {
    let dir = backup_dir(data_dir);
    std::fs::create_dir_all(&dir)?;

    let payload = db.export_backup()?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = dir.join(format!("{}{}.json", BACKUP_PREFIX, stamp));
    std::fs::write(&path, serde_json::to_string_pretty(&payload)?)?;

    // 保留轮换：超出保留数的旧备份按名字（时间戳）删除
    let keep = read_keep(db);
    let mut files = list_backup_files(&dir);
    while files.len() > keep {
        if let Some(oldest) = files.first() {
            let _ = std::fs::remove_file(oldest);
            files.remove(0);
        }
    }
    Ok(path)
}

fn run_once_if_due(db: &Database, data_dir: &Path) {
    let interval_hours: i64 = db
        .get_setting("backup_interval_hours")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let last = newest_backup_mtime(&backup_dir(data_dir));
    if due(last, now, interval_hours) {
        match perform_backup(db, data_dir) {
            Ok(path) => tracing::info!("Scheduled backup written to {}", path.display()),
            Err(e) => tracing::error!("Scheduled backup failed: {}", e),
        }
    }
}

/// 后台循环：每小时检查一次 backup_interval_hours 设置，到期即备份。
pub async fn backup_loop(db: Arc<Database>, data_dir: PathBuf) {
    // 启动后先检查一次（刚开启定时、还没有任何备份时会立刻补一份）
    let (db2, dir2) = (db.clone(), data_dir.clone());
    if let Err(e) = tokio::task::spawn_blocking(move || run_once_if_due(&db2, &dir2)).await {
        tracing::error!("Scheduled backup check failed: {}", e);
    }

    let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
    loop {
        tick.tick().await;
        let (db2, dir2) = (db.clone(), data_dir.clone());
        if let Err(e) = tokio::task::spawn_blocking(move || run_once_if_due(&db2, &dir2)).await {
            tracing::error!("Scheduled backup check failed: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn setup_db() -> Database {
        let tmp = NamedTempFile::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap()).unwrap();
        db.init().unwrap();
        db
    }

    #[test]
    fn backup_due_logic() {
        let now = 1_700_000_000i64;
        // 关闭
        assert!(!due(Some(now - 100), now, 0));
        // 无历史 → 立即
        assert!(due(None, now, 24));
        // 间隔未到
        assert!(!due(Some(now - 3600 * 10), now, 24));
        // 刚好/超过间隔
        assert!(due(Some(now - 3600 * 24), now, 24));
        assert!(due(Some(now - 3600 * 30), now, 24));
    }

    #[test]
    fn perform_backup_writes_and_rotates() {
        let db = setup_db();
        let data_dir = tempfile::tempdir().unwrap();
        db.insert_archive("Manga A", "/path/a", "zip", 10, 100)
            .unwrap();

        // 预置两份“旧”备份，验证轮换
        let dir = backup_dir(data_dir.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{}20200101-000000.json", BACKUP_PREFIX)),
            "{}",
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("{}20200102-000000.json", BACKUP_PREFIX)),
            "{}",
        )
        .unwrap();

        // 只保留 1 份 → perform 后应只剩新文件
        db.update_settings(&std::collections::HashMap::from([(
            "backup_keep".to_string(),
            "1".to_string(),
        )]))
        .unwrap();

        let path = perform_backup(&db, data_dir.path()).unwrap();
        assert!(path.exists());
        let files = list_backup_files(&dir);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], path);
    }
}
