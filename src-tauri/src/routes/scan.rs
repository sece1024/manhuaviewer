//! 书库扫描：可观测进度 + 并行数页的增量扫描。
//!
//! - `POST /scan` 同步执行并返回汇总（保持既有契约），扫描期间通过全局
//!   `ScanJob` 暴露实时进度；
//! - `GET /scan/status` 供前端 1s 轮询进度（total/done/added/updated/...）；
//! - `POST /scan/cancel` 请求取消，扫描在档案间与数页前检查取消标志。
//!
//! 主要耗时是「开包数页」：ZIP/CBZ 只读中央目录，但 RAR/7z 每本都要启动一次
//! `unrar lb` / `7z l` 子进程。这里先把需要数页的档案收集起来，再用有界工作线程
//! 池并行数页（受硬件并发数与 8 上限约束），首扫大库提速明显。

use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::{error_response, internal_error, run_db};

/// 单任务并发：重复扫描会返回 409。
static CURRENT_SCAN: OnceLock<Mutex<Option<Arc<ScanJob>>>> = OnceLock::new();

fn current_scan() -> &'static Mutex<Option<Arc<ScanJob>>> {
    CURRENT_SCAN.get_or_init(|| Mutex::new(None))
}

/// 扫描任务的可观测状态。计数器均为原子，工作线程可并发更新。
pub struct ScanJob {
    running: AtomicBool,
    cancel: AtomicBool,
    /// 本次发现的档案总数（进度分母）
    total: AtomicUsize,
    /// 已处理数：未变化档案即时计入；需数页的档案在数页完成后计入
    done: AtomicUsize,
    added: AtomicUsize,
    updated: AtomicUsize,
    /// 签名未变化、直接跳过的档案数
    unchanged: AtomicUsize,
    /// 孤儿清理：磁盘上已消失、已从库中删除的档案数
    removed: AtomicUsize,
    /// 孤儿清理：磁盘上仍存在、本次未发现而保留的档案数
    skipped: AtomicUsize,
    current: Mutex<String>,
}

impl ScanJob {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            total: AtomicUsize::new(0),
            done: AtomicUsize::new(0),
            added: AtomicUsize::new(0),
            updated: AtomicUsize::new(0),
            unchanged: AtomicUsize::new(0),
            removed: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            current: Mutex::new(String::new()),
        })
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running.load(Ordering::SeqCst),
            "total": self.total.load(Ordering::SeqCst),
            "done": self.done.load(Ordering::SeqCst),
            "added": self.added.load(Ordering::SeqCst),
            "updated": self.updated.load(Ordering::SeqCst),
            "unchanged": self.unchanged.load(Ordering::SeqCst),
            "removed": self.removed.load(Ordering::SeqCst),
            "skipped": self.skipped.load(Ordering::SeqCst),
            "current": self.current.lock().unwrap().clone(),
        })
    }
}

/// 任务结束（含 panic）时统一清 running/cancel，避免异常退出后永远 409。
struct ScanEndGuard(Arc<ScanJob>);

impl Drop for ScanEndGuard {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
        self.0.cancel.store(false, Ordering::SeqCst);
    }
}

#[derive(Deserialize)]
pub struct ScanRequest {
    pub path: Option<String>,
    pub depth: Option<u32>,
}

/// 待数页并入库的档案。页数在并行阶段填充（保持 0 表示数页失败/未完成，
/// 下次扫描会因 `page_count = 0` 重新数）。
struct Pending {
    title: String,
    path: String,
    archive_type: String,
    file_size: i64,
    file_mtime: i64,
    page_count: i64,
}

/// 开包数页；任何失败都回退为 0（与既有语义一致：不中断扫描，下次重试）。
fn count_pages(path: &str, archive_type: &str) -> i64 {
    crate::services::archive::create_archive_reader(path, archive_type)
        .ok()
        .and_then(|r| r.list_pages().ok())
        .map(|p| p.len() as i64)
        .unwrap_or(0)
}

/// 工作线程数：受硬件并发数与 8 上限约束，且不超过任务数。
fn worker_count(tasks: usize) -> usize {
    let hw = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    hw.clamp(1, 8).min(tasks.max(1))
}

fn scan_inner(
    db: &crate::db::Database,
    root_dir: &str,
    depth: u32,
    job: &Arc<ScanJob>,
) -> anyhow::Result<serde_json::Value> {
    let scanner = crate::services::scanner::Scanner::new();
    let discovered = scanner.scan_directory(root_dir, depth)?;
    job.total.store(discovered.len(), Ordering::SeqCst);
    let present: HashSet<String> = discovered.iter().cloned().collect();

    // 快照：本 root 下已入库档案的 (page_count, file_size, file_mtime)
    let meta = db.scan_meta_for_root(root_dir)?;
    let meta_by_path: HashMap<&str, (i64, i64, i64)> = meta
        .iter()
        .map(|(p, pc, fs, fm)| (p.as_str(), (*pc, *fs, *fm)))
        .collect();

    // 第一遍（遍历 + 分类，快）：跳过未变化档案，其余收集待数页
    let mut pending: Vec<Pending> = Vec::with_capacity(discovered.len());
    for archive_path in &discovered {
        if job.cancel.load(Ordering::SeqCst) {
            break;
        }

        let archive_type = scanner.detect_archive_type(archive_path);
        let path = std::path::Path::new(archive_path);

        let file_size = std::fs::metadata(archive_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        let file_mtime = super::archives::archive_mtime_secs(archive_path);
        let existing = meta_by_path.get(archive_path.as_str()).copied();

        // 文件签名（mtime+size）与已入库一致且已有页数 → 完全跳过，不再开包数页
        if let Some((pc, fs, fm)) = existing {
            if fm == file_mtime && fs == file_size && pc > 0 {
                job.unchanged.fetch_add(1, Ordering::SeqCst);
                job.done.fetch_add(1, Ordering::SeqCst);
                continue;
            }
            job.updated.fetch_add(1, Ordering::SeqCst);
        } else {
            job.added.fetch_add(1, Ordering::SeqCst);
        }

        let title = {
            let relative = path.strip_prefix(root_dir).unwrap_or(path);
            let first = relative.components().next();
            match first {
                Some(std::path::Component::Normal(name)) => {
                    let s = name.to_string_lossy().to_string();
                    if path.is_file() {
                        std::path::Path::new(&s)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    } else {
                        s
                    }
                }
                _ => path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            }
        };

        pending.push(Pending {
            title,
            path: archive_path.clone(),
            archive_type,
            file_size,
            file_mtime,
            page_count: 0,
        });
    }

    // 第二遍（并行数页，慢）：每个索引只被一个线程处理，用原子数组收集结果
    if !pending.is_empty() {
        let counts: Vec<AtomicI64> = (0..pending.len()).map(|_| AtomicI64::new(0)).collect();
        let cursor = AtomicUsize::new(0);
        let workers = worker_count(pending.len());
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    if job.cancel.load(Ordering::SeqCst) {
                        break;
                    }
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= pending.len() {
                        break;
                    }
                    let p = &pending[i];
                    let pc = count_pages(&p.path, &p.archive_type);
                    counts[i].store(pc, Ordering::SeqCst);
                    *job.current.lock().unwrap() = p.title.clone();
                    job.done.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        for (i, p) in pending.iter_mut().enumerate() {
            p.page_count = counts[i].load(Ordering::SeqCst);
        }
    }

    // 单事务批量写入，避免每个档案单独获取连接 + SELECT id
    let upserts: Vec<(String, String, String, i64, i64, i64)> = pending
        .iter()
        .map(|p| {
            (
                p.title.clone(),
                p.path.clone(),
                p.archive_type.clone(),
                p.page_count,
                p.file_size,
                p.file_mtime,
            )
        })
        .collect();
    db.batch_upsert_scanned_archives(&upserts)?;

    // 清理孤儿档案：本 root 下、磁盘上确实已不存在的路径（只清本 root，不影响其它根）。
    // 磁盘上仍存在但本次扫描未发现的路径（深度限制、扩展名白名单外、无图片文件夹、
    // 路径字符串形态差异等）一律跳过，避免误删手动打开或位于扫描盲区的档案及其标签/历史。
    let mut removed = 0usize;
    let mut skipped = 0usize;
    for (row_path, _pc, _fs, _fm) in &meta {
        if present.contains(row_path) {
            continue; // 本次扫描已发现，保留
        }
        if std::path::Path::new(row_path).exists() {
            tracing::info!(
                "Scan cleanup: keeping {} (exists on disk but not discovered this scan)",
                row_path
            );
            skipped += 1;
            continue;
        }
        tracing::info!("Scan cleanup: removing orphan archive {}", row_path);
        db.delete_archive_by_path(row_path)?;
        removed += 1;
        job.removed.fetch_add(1, Ordering::SeqCst);
    }
    job.skipped.store(skipped, Ordering::SeqCst);

    let added = job.added.load(Ordering::SeqCst);
    let updated = job.updated.load(Ordering::SeqCst);
    let cancelled = job.cancel.load(Ordering::SeqCst);
    let message = if cancelled {
        format!(
            "扫描已取消：共发现 {} 个档案，新增 {}，更新 {}，清理 {} 个已删除档案",
            discovered.len(),
            added,
            updated,
            removed
        )
    } else {
        format!(
            "扫描完成：共 {} 个档案，新增 {}，更新 {}，清理 {} 个已删除档案，跳过 {} 个仍存在的档案",
            discovered.len(),
            added,
            updated,
            removed,
            skipped
        )
    };

    Ok(serde_json::json!({
        "scanned": discovered.len(),
        "added": added,
        "updated": updated,
        "removed": removed,
        "skipped": skipped,
        "cancelled": cancelled,
        "message": message,
    }))
}

fn run_scan(
    db: Arc<crate::db::Database>,
    root_dir: String,
    depth: u32,
    job: Arc<ScanJob>,
) -> anyhow::Result<serde_json::Value> {
    let _guard = ScanEndGuard(job.clone());
    scan_inner(&db, &root_dir, depth, &job)
}

/// 增量扫描：新增入库、变更更新、磁盘已删除的档案会被清理。
/// 整个扫描在阻塞线程执行，期间进度经全局 ScanJob 暴露给 /scan/status。
pub async fn scan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> Response {
    let (root_dir, depth) = match run_db(&state, move |db| {
        let root_dir = if let Some(p) = payload.path {
            p
        } else {
            db.get_setting("root_dir").unwrap_or_default()
        };

        if root_dir.is_empty() {
            return Ok(None);
        }

        let depth = payload.depth.unwrap_or_else(|| {
            db.get_setting("scan_depth")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
        });

        Ok::<Option<(String, u32)>, rusqlite::Error>(Some((root_dir, depth)))
    })
    .await
    {
        Ok(Some(v)) => v,
        Ok(None) => return error_response(StatusCode::BAD_REQUEST, "No root directory configured"),
        Err(e) => return internal_error(e),
    };

    {
        let cur = current_scan().lock().unwrap();
        if let Some(j) = cur.as_ref() {
            if j.running.load(Ordering::SeqCst) {
                return error_response(StatusCode::CONFLICT, "已有扫描任务在运行");
            }
        }
    }

    let job = ScanJob::new();
    job.running.store(true, Ordering::SeqCst);
    *job.current.lock().unwrap() = "正在遍历目录...".to_string();
    *current_scan().lock().unwrap() = Some(job.clone());

    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || run_scan(db, root_dir, depth, job)).await;

    match result {
        Ok(Ok(body)) => Json(body).into_response(),
        Ok(Err(e)) => internal_error(e),
        Err(e) => internal_error(e),
    }
}

pub async fn scan_status() -> Response {
    let json = current_scan()
        .lock()
        .unwrap()
        .as_ref()
        .map(|j| j.to_json())
        .unwrap_or_else(|| {
            serde_json::json!({
                "running": false,
                "total": 0,
                "done": 0,
                "added": 0,
                "updated": 0,
                "unchanged": 0,
                "removed": 0,
                "skipped": 0,
                "current": "",
            })
        });
    Json(json).into_response()
}

pub async fn scan_cancel() -> Response {
    if let Some(job) = current_scan().lock().unwrap().as_ref() {
        job.cancel.store(true, Ordering::SeqCst);
    }
    Json(serde_json::json!({ "cancelled": true })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_count_is_bounded() {
        assert_eq!(worker_count(0), 1);
        assert!((1..=3).contains(&worker_count(3)));
        assert!(worker_count(10_000) <= 8);
    }

    /// 并行数页的正确性：多个文件夹档案的页数应各自数准，进度计数收敛到总数。
    #[test]
    fn scan_counts_pages_in_parallel_and_reports_progress() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let expected = [2usize, 3, 1];
        for (idx, count) in expected.iter().enumerate() {
            let ch = root.join(format!("book{idx}"));
            std::fs::create_dir_all(&ch).unwrap();
            for page in 0..*count {
                std::fs::write(ch.join(format!("page{page:02}.jpg")), b"img").unwrap();
            }
        }

        let db = crate::db::Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();

        let job = ScanJob::new();
        let root_s = root.to_string_lossy().into_owned();
        let out = scan_inner(&db, &root_s, 1, &job).unwrap();

        assert_eq!(out["scanned"].as_u64(), Some(3));
        assert_eq!(out["added"].as_u64(), Some(3));
        assert_eq!(out["cancelled"].as_bool(), Some(false));
        assert_eq!(job.total.load(Ordering::SeqCst), 3);
        assert_eq!(job.done.load(Ordering::SeqCst), 3);
        assert_eq!(job.added.load(Ordering::SeqCst), 3);

        for (idx, count) in expected.iter().enumerate() {
            let path = root
                .join(format!("book{idx}"))
                .to_string_lossy()
                .into_owned();
            let row = db.get_archive_by_path(&path).unwrap().unwrap();
            assert_eq!(row.page_count, *count as i64, "book{idx} 页数应正确");
        }
    }

    /// 未变化档案应被跳过（unchanged），不再重复数页。
    #[test]
    fn rescan_skips_unchanged_archives() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let ch = root.join("book");
        std::fs::create_dir_all(&ch).unwrap();
        std::fs::write(ch.join("page01.jpg"), b"img").unwrap();

        let db = crate::db::Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let root_s = root.to_string_lossy().into_owned();

        let first = ScanJob::new();
        scan_inner(&db, &root_s, 1, &first).unwrap();
        assert_eq!(first.added.load(Ordering::SeqCst), 1);

        let second = ScanJob::new();
        let out = scan_inner(&db, &root_s, 1, &second).unwrap();
        assert_eq!(second.unchanged.load(Ordering::SeqCst), 1);
        assert_eq!(second.added.load(Ordering::SeqCst), 0);
        assert_eq!(second.done.load(Ordering::SeqCst), 1);
        assert_eq!(out["added"].as_u64(), Some(0));
    }
}
