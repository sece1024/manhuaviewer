//! 磁盘缓存（缩略图 / 解压产物）的预算与 LRU 淘汰。
//!
//! - 书库封面：`thumbnails/{id}/cover.jpg`，封面很小，512MB 足以容纳数万本，按 DB 的
//!   `thumb_accessed_at`（真实访问时间）做 LRU；
//! - 页面缩略图：`page_thumbs/{id}/{index}.jpg`，量大，按目录 mtime 做 LRU；
//! - 解压产物：`extract/{id}/`（RAR/7z 整包解压），可能与本库同量级，按目录 mtime 做 LRU。
//!
//! 淘汰按「最近使用优先保留」进行，且永不清空最近一个目录（避免单个超预算目录被立刻
//! 删掉又立即重建）。

use crate::db::Database;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 书库封面缓存预算。封面约 10–30KB，512MB 可缓存数万本，基本不会触发封面淘汰。
pub const COVER_CACHE_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
/// 阅读器页面缩略图缓存预算（每页一张 jpg，量随阅读量增长）。
pub const PAGE_THUMB_CACHE_BUDGET_BYTES: u64 = 1024 * 1024 * 1024;
/// RAR/7z 整包解压缓存预算（解压产物通常与档案本身等大，必须有上限，否则会重复占用大量磁盘）。
pub const EXTRACT_CACHE_BUDGET_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// 递归统计目录占用字节数；不存在或不可读按 0。
pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        match entry.metadata() {
            Ok(meta) if meta.is_dir() => total += dir_size(&entry.path()),
            Ok(meta) => total += meta.len(),
            Err(_) => {}
        }
    }
    total
}

struct CacheDir {
    path: PathBuf,
    mtime: SystemTime,
    size: u64,
}

/// 列举 `root` 下的子目录及其 (mtime, size)。
fn list_subdirs(root: &Path) -> Vec<CacheDir> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return vec![];
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        dirs.push(CacheDir {
            size: dir_size(&path),
            path,
            mtime,
        });
    }
    dirs
}

/// 封面缓存 LRU：按 DB 访问时间从新到旧，累计超过 `budget` 的最旧目录淘汰。
/// 返回被淘汰的目录路径，并同步清除 DB 里的 `thumbnail_path`。
pub fn evict_cover_dirs(
    db: &Database,
    thumbs_root: &Path,
    budget: u64,
    exclude_id: Option<i64>,
) -> Vec<PathBuf> {
    let ids = db.get_cached_archive_ids().unwrap_or_default();
    let entries: Vec<(i64, PathBuf, u64)> = ids
        .into_iter()
        .map(|id| {
            let path = thumbs_root.join(id.to_string());
            let size = dir_size(&path);
            (id, path, size)
        })
        .collect();

    let mut used = 0u64;
    let mut evict_ids = Vec::new();
    let mut evict_paths = Vec::new();
    for (i, (id, path, size)) in entries.iter().enumerate() {
        // 最近一个与 exclude 的目录强制保留：单个目录超预算时也不至于删了又立刻重建
        if i == 0 || Some(*id) == exclude_id || used + size <= budget {
            used += size;
        } else {
            evict_ids.push(*id);
            evict_paths.push(path.clone());
        }
    }

    if !evict_ids.is_empty() {
        let _ = db.clear_thumbnail_paths(&evict_ids);
    }
    evict_paths
}

/// 按目录 mtime 的通用 LRU：从新到旧累计，超过 `budget` 的最旧目录淘汰。
/// 适用于 `page_thumbs/`、`extract/` 这类按档案 id 命名的目录缓存。
pub fn evict_dirs_by_mtime(root: &Path, budget: u64, exclude_id: Option<i64>) -> Vec<PathBuf> {
    let mut dirs = list_subdirs(root);
    dirs.sort_by_key(|d| std::cmp::Reverse(d.mtime));

    let mut used = 0u64;
    let mut evict = Vec::new();
    for (i, d) in dirs.iter().enumerate() {
        let id = d
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.parse::<i64>().ok());
        let excluded = exclude_id.is_some() && id == exclude_id;
        if i == 0 || excluded || used + d.size <= budget {
            used += d.size;
        } else {
            evict.push(d.path.clone());
        }
    }
    evict
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn dir_size_sums_files_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(&tmp.path().join("a.jpg"), 100);
        write_file(&tmp.path().join("sub/b.jpg"), 250);
        assert_eq!(dir_size(tmp.path()), 350);
        assert_eq!(dir_size(&tmp.path().join("missing")), 0);
    }

    #[test]
    fn evicts_oldest_page_thumb_dirs_over_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // 每个目录 1KB，按时间先后创建（1 最旧、3 最新）；目录 mtime 随之递增
        for id in [1, 2, 3] {
            write_file(&root.join(id.to_string()).join("0.jpg"), 1000);
            std::thread::sleep(std::time::Duration::from_millis(15));
        }

        // 预算 2.5KB：保留最新的 3、2，淘汰最旧的 1
        let evicted = evict_dirs_by_mtime(root, 2500, None);
        assert_eq!(evicted.len(), 1);
        assert_eq!(
            evicted[0].file_name().unwrap().to_str().unwrap(),
            "1",
            "应按 mtime 淘汰最旧目录"
        );
    }

    #[test]
    fn always_keeps_the_most_recent_dir_even_over_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_file(&root.join("1").join("0.jpg"), 5000);
        let evicted = evict_dirs_by_mtime(root, 100, None);
        assert!(evicted.is_empty(), "唯一且最新的目录必须保留");
    }

    #[test]
    fn excluded_dir_is_never_evicted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for id in [1, 2, 3] {
            write_file(&root.join(id.to_string()).join("0.jpg"), 1000);
            std::thread::sleep(std::time::Duration::from_millis(15));
        }

        // 预算只够 1 个；排除最旧的 1，则次旧的 2 被淘汰、1 保留
        let evicted = evict_dirs_by_mtime(root, 1000, Some(1));
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0].file_name().unwrap().to_str().unwrap(), "2");
    }

    /// 封面淘汰按预算删目录并同步清除 DB 的 thumbnail_path。
    /// 等大小 + 等预算下被淘汰数量确定（不确定具体是哪一本，取决于访问时间）。
    #[test]
    fn evict_cover_dirs_respects_budget_and_clears_db_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::new(tmp.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let root = tmp.path().join("thumbnails");
        for id in 1..=3 {
            db.upsert_scanned_archive(&format!("A{id}"), &format!("/x/{id}.cbz"), "cbz", 1, 1, 1)
                .unwrap();
            let dir = root.join(id.to_string());
            write_file(&dir.join("cover.jpg"), 1000);
            db.set_thumbnail_path(id, dir.to_str().unwrap()).unwrap();
        }
        assert_eq!(db.get_cached_archive_ids().unwrap().len(), 3);

        // 预算仅够 2 本 → 恰好淘汰 1 本
        let evicted = evict_cover_dirs(&db, &root, 2000, None);
        assert_eq!(evicted.len(), 1);
        assert_eq!(db.get_cached_archive_ids().unwrap().len(), 2);
    }
}
