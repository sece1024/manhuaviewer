use crate::db::Database;
use crate::services::cache_budget;
use anyhow::Result;
use std::path::Path;

/// 清理不再被任何档案引用的缓存目录，并在启动时强制执行一次缩略图 LRU 淘汰。
///
/// 场景：
/// - RAR/7z 首次访问会把整包解压到 `<data_dir>/extract/{id}/`，但该目录此前只在
///   档案被显式删除时清理——一旦档案被删而目录残留，磁盘会无限增长。
/// - 缩略图 LRU 只在新生成缩略图时触发（60s 节流），长时间不生成新缩略图时，
///   一次性浏览过的大量缩略图目录会一直驻留磁盘。
///
/// 本函数枚举 `extract/`、`thumbnails/`、`page_thumbs/` 下的子目录，删除编号不再属于
/// 任何存活档案的孤立目录，并按磁盘预算执行封面 / 页面缩略图 / 解压产物各一次 LRU 淘汰。
/// 全部为磁盘 I/O，调用方应放在 spawn_blocking 中。
pub fn cleanup_caches(data_dir: &Path, db: &Database) -> Result<()> {
    let live = db.live_archive_ids()?;

    for sub in ["extract", "thumbnails", "page_thumbs"] {
        prune_orphans(&data_dir.join(sub), &live)?;
    }

    let thumbs_root = data_dir.join("thumbnails");
    // 旧版把页面缩略图与封面放在同一目录：清掉封面以外的残留文件，避免占用预算/误判大小
    purge_legacy_cover_dir_files(&thumbs_root);

    for path in cache_budget::evict_cover_dirs(
        db,
        &thumbs_root,
        cache_budget::COVER_CACHE_BUDGET_BYTES,
        None,
    ) {
        let _ = std::fs::remove_dir_all(path);
    }
    for path in cache_budget::evict_dirs_by_mtime(
        &data_dir.join("page_thumbs"),
        cache_budget::PAGE_THUMB_CACHE_BUDGET_BYTES,
        None,
    ) {
        let _ = std::fs::remove_dir_all(path);
    }
    for path in cache_budget::evict_dirs_by_mtime(
        &data_dir.join("extract"),
        cache_budget::EXTRACT_CACHE_BUDGET_BYTES,
        None,
    ) {
        let _ = std::fs::remove_dir_all(path);
    }

    Ok(())
}

/// 删除 `thumbnails/{id}/` 下除 `cover.jpg` 以外的一切残留（旧版页面缩略图 `{index}.jpg`
/// 与 `archive.mtime` 标记）。页面缩略图现已迁移到 `page_thumbs/`。
fn purge_legacy_cover_dir_files(thumbs_root: &Path) {
    let Ok(entries) = std::fs::read_dir(thumbs_root) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.is_file() && path.file_name().and_then(|n| n.to_str()) != Some("cover.jpg") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
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

    /// 旧版页面缩略图与封面同目录：清理时应只保留 cover.jpg，删掉残留的页面图与 mtime 标记。
    #[test]
    fn purge_legacy_cover_dir_keeps_only_cover() {
        let dir = TempDir::new().unwrap();
        let arch = dir.path().join("7");
        std::fs::create_dir_all(&arch).unwrap();
        std::fs::write(arch.join("cover.jpg"), b"cover").unwrap();
        std::fs::write(arch.join("0.jpg"), b"page").unwrap();
        std::fs::write(arch.join("archive.mtime"), b"123").unwrap();

        purge_legacy_cover_dir_files(dir.path());

        assert!(arch.join("cover.jpg").exists());
        assert!(!arch.join("0.jpg").exists());
        assert!(!arch.join("archive.mtime").exists());
    }
}
