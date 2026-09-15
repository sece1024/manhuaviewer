//! 进程内页表缓存：把「解析档案页表（压缩包读 pages 表 / 文件夹现场扫描）」
//! 收敛成带进程级缓存的统一入口，消灭翻页/缩略图热路径里每页请求的两类重复工作：
//! - 压缩包档案的 2 次 DB 查询（get_page_list_mtime + get_pages）；
//! - 文件夹档案的每次 read_dir 全扫 + stat（此前一本 200 页 = O(pages²)）。
//!
//! 档案 mtime 变化即失效；容量满时逐出任意一项（个人书库规模足够）。
//! 被路由层（档案页/缩略图/封面设置）与 OPDS 档案详情共用，避免各自重开压缩包。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::db::{Database, PageRow};

use super::is_compressed;

/// 进程内页表缓存：(archive_id) -> (mtime_secs, Arc<页面行>)
const PAGE_LIST_CACHE_MAX: usize = 256;
type PageListCache = Mutex<HashMap<i64, (i64, Arc<Vec<PageRow>>)>>;
static PAGE_LIST_CACHE: OnceLock<PageListCache> = OnceLock::new();

fn page_list_cache() -> &'static PageListCache {
    PAGE_LIST_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn to_page_rows(archive_id: i64, list: &[String]) -> Vec<PageRow> {
    list.iter()
        .enumerate()
        .map(|(i, p)| {
            let filename = std::path::Path::new(p)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            PageRow {
                id: i as i64,
                archive_id,
                filename,
                filepath: p.clone(),
                sort_order: i as i64,
            }
        })
        .collect()
}

/// Resolve the page list for an archive, using the cached `pages` table when it
/// is still valid (compressed archives whose file mtime is unchanged), falling
/// back to a full archive scan otherwise. Folder archives always scan live.
/// Runs on a blocking thread and needs `db` for the cache.
/// Returns an Arc so every hot-path caller shares one page table instead of cloning.
pub fn load_page_rows(
    db: &Database,
    archive_id: i64,
    archive_path: &str,
    archive_type: &str,
    mtime_secs: i64,
) -> anyhow::Result<Arc<Vec<PageRow>>> {
    // 进程内缓存命中（mtime 未变）：跳过 DB 与磁盘扫描
    {
        let cache = page_list_cache().lock().unwrap();
        if let Some((mt, rows)) = cache.get(&archive_id) {
            if *mt == mtime_secs {
                return Ok(rows.clone());
            }
        }
    }

    let reader = crate::services::archive::create_archive_reader(archive_path, archive_type)?;
    let rows = if is_compressed(archive_type) {
        let cached_mtime = db.get_page_list_mtime(archive_id).ok().flatten();
        if cached_mtime == Some(mtime_secs) {
            let cached = db.get_pages(archive_id).unwrap_or_default();
            if !cached.is_empty() {
                cached
            } else {
                let list = reader.list_pages()?;
                let rows = to_page_rows(archive_id, &list);
                let _ = db.save_pages(archive_id, &rows, mtime_secs);
                rows
            }
        } else {
            let list = reader.list_pages()?;
            let rows = to_page_rows(archive_id, &list);
            let _ = db.save_pages(archive_id, &rows, mtime_secs);
            rows
        }
    } else {
        to_page_rows(archive_id, &reader.list_pages()?)
    };

    let arc = Arc::new(rows);
    let mut cache = page_list_cache().lock().unwrap();
    if !cache.contains_key(&archive_id) && cache.len() >= PAGE_LIST_CACHE_MAX {
        if let Some(key) = cache.keys().next().copied() {
            cache.remove(&key);
        }
    }
    cache.insert(archive_id, (mtime_secs, arc.clone()));
    Ok(arc)
}
