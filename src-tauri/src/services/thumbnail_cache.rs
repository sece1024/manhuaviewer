//! 缩略图运行时缓存与磁盘预算淘汰（封面 + 页面缩略图）。
//!
//! 三条入口分别对应三处缩略图生命周期事件，把「何时必须写 DB / 何时该跑 LRU
//! 淘汰」的全部复杂度藏在同一小接口后面：
//!
//! - `record_cover_access`：封面被访问，节流记录（每档案 ≤1 次/60 秒），避免书库滚动
//!   产生大量 DB 写；touch 后 LRU 按真实使用时间保留/淘汰封面目录。
//! - `on_cover_registered`：新封面缩略图落盘，登记 `thumbnail_path` 并按磁盘预算触发封面
//!   LRU 淘汰（每分钟最多一次）。
//! - `on_page_thumb_registered`：新页面缩略图落盘，按磁盘预算触发页面缩略图目录 LRU
//!   淘汰（每分钟最多一次，独立于封面淘汰）。

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use crate::AppState;

/// 缩略图访问节流表（进程内），避免每个图片请求都写一次 DB。
static THUMB_TOUCH: OnceLock<Mutex<std::collections::HashMap<i64, Instant>>> = OnceLock::new();

/// 页面缩略图 LRU 淘汰节流（每分钟最多一次），独立于封面淘汰。
static PAGE_THUMB_EVICTION: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

/// 两次封面 LRU 淘汰之间、以及访问节流的最小间隔（秒）。
const EVICTION_INTERVAL_SECS: u64 = 60;

/// 判断 `last` 是否已超过节流间隔，命中时刷新时间戳并返回 true。
fn due_since(last: &mut Option<Instant>) -> bool {
    let elapsed = last
        .as_ref()
        .map(|t| t.elapsed().as_secs() >= EVICTION_INTERVAL_SECS)
        .unwrap_or(true);
    if elapsed {
        *last = Some(Instant::now());
    }
    elapsed
}

/// 封面访问（节流）：每档案 ≤1 次/60 秒写 `touch_thumbnail_access`，让 LRU 保留
/// 真正在使用的封面目录。
pub async fn record_cover_access(state: &Arc<AppState>, id: i64) {
    {
        let map = THUMB_TOUCH
            .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
            .lock()
            .unwrap();
        if let Some(t) = map.get(&id) {
            if t.elapsed().as_secs() < EVICTION_INTERVAL_SECS {
                return;
            }
        }
    }
    let _ = super::super::routes::run_db(state, move |db| db.touch_thumbnail_access(id)).await;
    THUMB_TOUCH
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap()
        .insert(id, Instant::now());
}

/// 新封面缩略图落盘：登记 `thumbnail_path`（若尚未登记），并按磁盘预算触发封面
/// LRU 淘汰（每分钟最多一次）。
pub async fn on_cover_registered(
    state: &Arc<AppState>,
    id: i64,
    thumb_dir_str: String,
    already_set: bool,
) {
    if !already_set {
        let dir = thumb_dir_str;
        let _ =
            super::super::routes::run_db(state, move |db| db.set_thumbnail_path(id, &dir)).await;
    }

    let do_evict = {
        let mut last = state.last_thumb_eviction.lock().unwrap();
        due_since(&mut last)
    };
    if !do_evict {
        return;
    }

    let root = state.data_dir.join("thumbnails");
    let db = state.db.clone();
    let evicted = tokio::task::spawn_blocking(move || {
        crate::services::cache_budget::evict_cover_dirs(
            &db,
            &root,
            crate::services::cache_budget::COVER_CACHE_BUDGET_BYTES,
            Some(id),
        )
    })
    .await
    .unwrap_or_default();
    for path in evicted {
        let _ = tokio::fs::remove_dir_all(&path).await;
    }
}

/// 新页面缩略图落盘：按磁盘预算触发页面缩略图目录 LRU 淘汰（每分钟最多一次）。
pub async fn on_page_thumb_registered(state: &Arc<AppState>, id: i64) {
    let due = {
        let lock = PAGE_THUMB_EVICTION.get_or_init(|| Mutex::new(None));
        let mut last = lock.lock().unwrap();
        due_since(&mut last)
    };
    if !due {
        return;
    }

    let root = state.data_dir.join("page_thumbs");
    let evicted = tokio::task::spawn_blocking(move || {
        crate::services::cache_budget::evict_dirs_by_mtime(
            &root,
            crate::services::cache_budget::PAGE_THUMB_CACHE_BUDGET_BYTES,
            Some(id),
        )
    })
    .await
    .unwrap_or_default();
    for path in evicted {
        let _ = tokio::fs::remove_dir_all(&path).await;
    }
}
