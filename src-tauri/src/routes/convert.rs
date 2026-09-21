//! 批量格式转换：把库中的非 CBZ 压缩档案（zip/rar/cbr/7z）转换为位于**同目录、同名**的
//! CBZ，并在转换成功、DB 更新后删除原文件。保留档案 id，因此标签/历史/书签/分类/分组
//! 全部延续。
//!
//! - `POST /archives/convert-cbz/start` 启动后台任务（已有任务运行时返回 409）；
//! - `GET /archives/convert-cbz/status` 轮询进度（total/done/converted/skipped/failed）；
//! - `POST /archives/convert-cbz/cancel` 请求取消（档案之间检查取消标志）。
//!
//! ZIP 内容本身即合法 CBZ，仅做同目录改名（免重压）；RAR/7z 用持久化解压目录逐页读盘后
//! 重打包为 Stored CBZ。任何一步失败都不删除原文件，也不留半截目标文件。

use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::{error_response, internal_error};

static CURRENT_JOB: OnceLock<Mutex<Option<Arc<ConvertJob>>>> = OnceLock::new();

fn current_job() -> &'static Mutex<Option<Arc<ConvertJob>>> {
    CURRENT_JOB.get_or_init(|| Mutex::new(None))
}

/// 转换任务的可观测状态。
pub struct ConvertJob {
    running: AtomicBool,
    cancel: AtomicBool,
    total: AtomicUsize,
    done: AtomicUsize,
    converted: AtomicUsize,
    skipped: AtomicUsize,
    failed: AtomicUsize,
    current: Mutex<String>,
    /// 失败项（标题: 原因），最多展示前若干条。
    errors: Mutex<Vec<String>>,
}

impl ConvertJob {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            total: AtomicUsize::new(0),
            done: AtomicUsize::new(0),
            converted: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
            current: Mutex::new(String::new()),
            errors: Mutex::new(Vec::new()),
        })
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running.load(Ordering::SeqCst),
            "total": self.total.load(Ordering::SeqCst),
            "done": self.done.load(Ordering::SeqCst),
            "converted": self.converted.load(Ordering::SeqCst),
            "skipped": self.skipped.load(Ordering::SeqCst),
            "failed": self.failed.load(Ordering::SeqCst),
            "current": self.current.lock().unwrap().clone(),
            "errors": self.errors.lock().unwrap().clone(),
        })
    }
}

/// 任务结束（含 panic）时统一清 running/cancel。
struct ConvertEndGuard(Arc<ConvertJob>);

impl Drop for ConvertEndGuard {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
        self.0.cancel.store(false, Ordering::SeqCst);
    }
}

/// 转换单个档案。`Ok(true)`=已转换，`Ok(false)`=跳过（无需/不可转换），`Err`=失败。
fn convert_one(
    db: &crate::db::Database,
    data_dir: &Path,
    a: &crate::db::ArchiveRow,
) -> anyhow::Result<bool> {
    let src = Path::new(&a.path);
    if !src.is_file() {
        tracing::warn!("CBZ 转换跳过（源文件不存在）: {}", a.path);
        return Ok(false);
    }
    let target = src.with_extension("cbz");
    if target == src {
        return Ok(false);
    }
    let target_str = target.to_string_lossy().to_string();
    if target.exists() {
        tracing::warn!("CBZ 转换跳过（目标文件已存在）: {}", target_str);
        return Ok(false);
    }
    if db.get_archive_by_path(&target_str)?.is_some() {
        tracing::warn!("CBZ 转换跳过（目标路径已被其它档案占用）: {}", target_str);
        return Ok(false);
    }

    if a.archive_type == "zip" {
        // ZIP 本身即合法 CBZ：仅改名。先改名再更新 DB，DB 失败则回滚改名。
        let file_size = std::fs::metadata(src)?.len() as i64;
        let file_mtime = crate::services::fs_ext::mtime_secs(src);
        std::fs::rename(src, &target)?;
        if let Err(e) =
            db.update_archive_converted(a.id, &target_str, a.page_count, file_size, file_mtime)
        {
            let _ = std::fs::rename(&target, src);
            return Err(e.into());
        }
    } else {
        let extract_dir = data_dir.join("extract").join(a.id.to_string());
        let pages = crate::services::cbz::repack_archive_to_cbz(
            &a.path,
            &a.archive_type,
            Some(extract_dir),
            &target,
        )?;
        let file_size = std::fs::metadata(&target)?.len() as i64;
        let file_mtime = crate::services::fs_ext::mtime_secs(&target);
        if let Err(e) =
            db.update_archive_converted(a.id, &target_str, pages as i64, file_size, file_mtime)
        {
            let _ = std::fs::remove_file(&target);
            return Err(e.into());
        }
        // 仅在 DB 更新成功后删除原文件
        std::fs::remove_file(src)?;
    }

    // 清理该档案的旧缓存目录（解压产物/页面缩略图/封面；封面下次访问按需重建）
    let _ = std::fs::remove_dir_all(data_dir.join("extract").join(a.id.to_string()));
    let _ = std::fs::remove_dir_all(data_dir.join("page_thumbs").join(a.id.to_string()));
    let _ = std::fs::remove_dir_all(data_dir.join("thumbnails").join(a.id.to_string()));
    Ok(true)
}

fn run_convert_job(
    db: Arc<crate::db::Database>,
    data_dir: PathBuf,
    job: Arc<ConvertJob>,
    candidates: Vec<crate::db::ArchiveRow>,
) {
    let _guard = ConvertEndGuard(job.clone());
    job.total.store(candidates.len(), Ordering::SeqCst);

    // 小并发：档案之间并行（受硬件并发数与 3 上限约束），每个索引只处理一次。
    // 转换是重 I/O（解压/重压/改名），并发过高会拖慢磁盘，故上限取 3。
    let cursor = AtomicUsize::new(0);
    let workers = worker_count(candidates.len());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                if job.cancel.load(Ordering::SeqCst) {
                    break;
                }
                let i = cursor.fetch_add(1, Ordering::SeqCst);
                if i >= candidates.len() {
                    break;
                }
                let a = &candidates[i];
                *job.current.lock().unwrap() = a.title.clone();
                match convert_one(&db, &data_dir, a) {
                    Ok(true) => {
                        job.converted.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(false) => {
                        job.skipped.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => {
                        job.failed.fetch_add(1, Ordering::SeqCst);
                        job.errors
                            .lock()
                            .unwrap()
                            .push(format!("{}: {}", a.title, e));
                    }
                }
                job.done.fetch_add(1, Ordering::SeqCst);
            });
        }
    });
}

/// 转换并发数：受硬件并发数与 3 上限约束，且不超过任务数。
fn worker_count(tasks: usize) -> usize {
    let hw = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    hw.clamp(1, 3).min(tasks.max(1))
}

#[derive(Deserialize, Default)]
pub struct ConvertStartRequest {
    /// 指定要转换的档案 id；省略/为空表示转换全部可转换档案。
    #[serde(default)]
    pub ids: Option<Vec<i64>>,
}

pub async fn convert_start(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConvertStartRequest>,
) -> Response {
    {
        let cur = current_job().lock().unwrap();
        if let Some(j) = cur.as_ref() {
            if j.running.load(Ordering::SeqCst) {
                return error_response(StatusCode::CONFLICT, "已有转换任务在运行");
            }
        }
    }

    let ids = payload.ids.filter(|v| !v.is_empty());
    let candidates = match super::run_db(&state, move |db| match ids {
        Some(ids) => db.list_convertible_archives_by_ids(&ids),
        None => db.list_convertible_archives(),
    })
    .await
    {
        Ok(v) => v,
        Err(e) => return internal_error(e),
    };
    let total = candidates.len();
    if total == 0 {
        return Json(serde_json::json!({ "started": true, "total": 0 })).into_response();
    }

    let job = ConvertJob::new();
    job.running.store(true, Ordering::SeqCst);
    job.total.store(total, Ordering::SeqCst);
    *job.current.lock().unwrap() = "准备中...".to_string();
    *current_job().lock().unwrap() = Some(job.clone());

    let db = state.db.clone();
    let data_dir = state.data_dir.clone();
    tokio::task::spawn_blocking(move || run_convert_job(db, data_dir, job, candidates));

    Json(serde_json::json!({ "started": true, "total": total })).into_response()
}

pub async fn convert_status() -> Response {
    let json = current_job()
        .lock()
        .unwrap()
        .as_ref()
        .map(|j| j.to_json())
        .unwrap_or_else(|| {
            serde_json::json!({
                "running": false,
                "total": 0,
                "done": 0,
                "converted": 0,
                "skipped": 0,
                "failed": 0,
                "current": "",
                "errors": [],
            })
        });
    Json(json).into_response()
}

pub async fn convert_cancel() -> Response {
    if let Some(job) = current_job().lock().unwrap().as_ref() {
        job.cancel.store(true, Ordering::SeqCst);
    }
    Json(serde_json::json!({ "cancelled": true })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use tempfile::TempDir;

    fn make_zip(path: &Path) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        for name in ["001.jpg", "002.jpg"] {
            zip.start_file(name, opts).unwrap();
            zip.write_all(b"img").unwrap();
        }
        zip.finish().unwrap();
    }

    /// ZIP→CBZ 快速路径：改名 + DB 就地更新（保留 id/路径类型），源文件不再存在。
    #[test]
    fn convert_one_zip_renames_and_updates_db() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let src = dir.path().join("book.zip");
        make_zip(&src);

        let db = Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let id = db
            .insert_archive("Book", src.to_str().unwrap(), "zip", 2, 6)
            .unwrap();

        let archive = db.get_archive(id).unwrap().unwrap();
        assert!(convert_one(&db, &data_dir, &archive).unwrap());

        let updated = db.get_archive(id).unwrap().unwrap();
        assert_eq!(updated.archive_type, "cbz");
        assert!(updated.path.ends_with(".cbz"));
        assert!(!src.exists(), "原 zip 应已改名");
        assert!(dir.path().join("book.cbz").exists());
        assert_eq!(updated.page_count, 2);
    }

    /// 目标已存在时跳过，保持源文件与 DB 不变。
    #[test]
    fn convert_one_skips_when_target_exists() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let src = dir.path().join("book.zip");
        make_zip(&src);
        std::fs::write(dir.path().join("book.cbz"), b"existing").unwrap();

        let db = Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let id = db
            .insert_archive("Book", src.to_str().unwrap(), "zip", 2, 6)
            .unwrap();
        let archive = db.get_archive(id).unwrap().unwrap();

        assert!(!convert_one(&db, &data_dir, &archive).unwrap());
        assert!(src.exists());
        assert_eq!(db.get_archive(id).unwrap().unwrap().archive_type, "zip");
    }

    #[test]
    fn worker_count_is_small_and_bounded() {
        assert_eq!(worker_count(0), 1);
        assert_eq!(worker_count(1), 1);
        assert!((1..=3).contains(&worker_count(1000)));
    }

    /// 指定 id 时只返回其中可转换的档案。
    #[test]
    fn list_convertible_archives_by_ids_filters() {
        let dir = TempDir::new().unwrap();
        let db = Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let seven_z = db.insert_archive("SevenZ", "/x/a.7z", "7z", 2, 10).unwrap();
        let cbz = db.insert_archive("Cbz", "/x/b.cbz", "cbz", 2, 10).unwrap();
        let folder = db
            .insert_archive("Folder", "/x/c", "folder", 2, 10)
            .unwrap();

        let rows = db
            .list_convertible_archives_by_ids(&[seven_z, cbz, folder])
            .unwrap();
        assert_eq!(rows.len(), 1, "只应返回 7z（cbz/folder 不可转换）");
        assert_eq!(rows[0].id, seven_z);
    }
}
