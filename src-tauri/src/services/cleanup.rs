use crate::db::Database;
use anyhow::Result;
use std::path::Path;

/// 清理不再被任何档案引用的缓存目录，并在启动时强制一次缩略图 LRU 淘汰。
///
/// 场景：
/// - RAR/7z 首次访问会把整包解压到 `<data_dir>/extract/{id}/`，但该目录此前只在
///   档案被显式删除时清理——一旦档案被删而目录残留，磁盘会无限增长。
/// - 缩略图 LRU 只在新生成缩略图时触发（60s 节流），长时间不生成新缩略图时，
///   一次性浏览过的大量缩略图目录会一直驻留磁盘。
///
/// 本函数枚举 `extract/` 与 `thumbnails/` 下的子目录，删除编号不再属于任何
/// 存活档案的孤立目录，并执行一次缩略图 LRU 淘汰以回收超上限的最旧目录。
/// 全部为磁盘 I/O，调用方应放在 spawn_blocking 中。
pub fn cleanup_caches(data_dir: &Path, db: &Database) -> Result<()> {
    let live = db.live_archive_ids()?;

    for sub in ["extract", "thumbnails"] {
        prune_orphans(&data_dir.join(sub), &live)?;
    }

    let evicted = db.evict_old_thumbnails(None).unwrap_or_default();
    for (_, thumb_path) in evicted {
        let _ = std::fs::remove_dir_all(thumb_path);
    }

    Ok(())
}

/// 删除 `dir` 下编号不属于 `live` 的子目录。
/// 仅处理名字为纯数字（archive id）的子目录；非数字目录不属于本缓存命名空间，原样保留。
fn prune_orphans(dir: &Path, live: &std::collections::HashSet<i64>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.parse::<i64>().ok())
        else {
            continue;
        };
        if !live.contains(&id) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn prune_orphans_removes_stale_only() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("1")).unwrap();
        std::fs::create_dir(dir.path().join("2")).unwrap();
        std::fs::create_dir(dir.path().join("3")).unwrap();
        // 非数字目录应跳过（不清除）
        std::fs::create_dir(dir.path().join("not-a-number")).unwrap();

        let live: std::collections::HashSet<i64> = [2, 3].iter().copied().collect();
        prune_orphans(dir.path(), &live).unwrap();

        assert!(!dir.path().join("1").exists());
        assert!(dir.path().join("2").exists());
        assert!(dir.path().join("3").exists());
        assert!(dir.path().join("not-a-number").exists());
    }
}

