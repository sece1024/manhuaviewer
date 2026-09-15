//! 跨机同步：把局域网远端（另一台电脑上的 MangaViewer）的整个漫画库
//! 拉到本机。本机后端作为代理直连远端（后端对后端，不经过浏览器 CORS）：
//! 1. 拉取远端的 sync manifest（标题/类型/大小 + 标签/分类/进度，均以 title 为键）；
//! 2. 逐个调用远端 `POST /api/archives/{id}/file` 下载档案字节到本地目录；
//! 3. 直接调用本地 db 入库（复用 upsert_scanned_archive），并按标题回填元数据；
//! 4. 后台任务 + 进度查询 + 取消；下载按 (文件名, 大小) 断点续传。

use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::{db_json, error_response, internal_error};

/// 单任务并发：重复 start 会返回 409。
static CURRENT_JOB: OnceLock<Mutex<Option<Arc<SyncJob>>>> = OnceLock::new();

fn current_job() -> &'static Mutex<Option<Arc<SyncJob>>> {
    CURRENT_JOB.get_or_init(|| Mutex::new(None))
}

pub struct SyncJob {
    running: AtomicBool,
    cancel: AtomicBool,
    /// 计划工作量（新增+更新，不含已同步）
    total: AtomicUsize,
    done: AtomicUsize,
    new_count: AtomicUsize,
    changed_count: AtomicUsize,
    skipped: AtomicUsize,
    current: Mutex<String>,
    failed: Mutex<Vec<String>>,
}

impl SyncJob {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            running: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            total: AtomicUsize::new(0),
            done: AtomicUsize::new(0),
            new_count: AtomicUsize::new(0),
            changed_count: AtomicUsize::new(0),
            skipped: AtomicUsize::new(0),
            current: Mutex::new(String::new()),
            failed: Mutex::new(Vec::new()),
        })
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running.load(Ordering::SeqCst),
            "total": self.total.load(Ordering::SeqCst),
            "done": self.done.load(Ordering::SeqCst),
            "new": self.new_count.load(Ordering::SeqCst),
            "changed": self.changed_count.load(Ordering::SeqCst),
            "skipped": self.skipped.load(Ordering::SeqCst),
            "current": self.current.lock().unwrap().clone(),
            "failed": self.failed.lock().unwrap().clone(),
        })
    }
}

// ── 远端清单 ──

#[derive(Deserialize, Clone)]
struct ManifestArchive {
    id: i64,
    title: String,
    #[serde(rename = "archive_type")]
    archive_type: String,
    page_count: i64,
    file_size: i64,
    /// 远端档案 mtime（秒），配合 /file 的 X-Source-Mtime 头把原始时间带回本机
    file_mtime: i64,
}

fn extension_for(archive_type: &str) -> &'static str {
    match archive_type {
        "zip" => "zip",
        "rar" => "rar",
        "cbr" => "cbr",
        "7z" => "7z",
        _ => "cbz", // cbz 与 folder（folder 下载时已打包为 cbz）
    }
}

/// 由标题生成安全的本地文件名：清掉路径分隔符/非法字符、规避 Windows 保留名。
/// `occurrence` 是同一标题在远端清单里的第几次出现（0 起）：同名多次出现时
/// 追加 `_2/_3` 后缀，避免两本不同漫画互相覆盖；单次出现固定用 `{title}.{ext}`，
/// “内容有变化”时下载会覆盖同名文件（覆盖更新语义）。
fn sync_filename(title: &str, archive_type: &str, occurrence: usize) -> String {
    let base: String = title
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(120)
        .collect();
    let base = if base.is_empty() {
        "archive"
    } else {
        base.as_str()
    };
    // Windows 保留设备名（CON/PRN/AUX/NUL/COM*/LPT*）与尾随点/空格：加下划线前缀避免落盘失败
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = base.split('.').next().unwrap_or(base).to_ascii_uppercase();
    let base = if RESERVED.contains(&stem.as_str()) || base.ends_with('.') || base.ends_with(' ') {
        format!("_{}", base.trim_end_matches(['.', ' ']))
    } else {
        base.to_string()
    };
    let ext = extension_for(archive_type);
    if occurrence == 0 {
        format!("{base}.{ext}")
    } else {
        format!("{base}_{}.{ext}", occurrence + 1)
    }
}

/// 计划条目：远端档案 + 落到本地的文件名 + 对比结论。
struct PlanItem {
    archive: ManifestArchive,
    filename: String,
    status: PlanStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanStatus {
    /// 本地没有 → 下载
    New,
    /// 本地同名存在但大小/页数不一致 → 下载覆盖（覆盖更新）
    Changed,
    /// 本地同名且一致 → 跳过
    UpToDate,
}

/// 把远端清单与本地目录/本地库比对，产出同步计划。
/// 压缩包按文件大小判变化；文件夹档案远端 file_size 恒 0，按本地库同名档案的
/// page_count 与远端 page_count 判变化。
fn build_plan(db: &crate::db::Database, entries: &[ManifestArchive], dir: &Path) -> Vec<PlanItem> {
    // 统计同标题出现次数，决定 _2/_3 后缀
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut plan = Vec::with_capacity(entries.len());
    for e in entries {
        let occurrence = {
            let n = used.entry(e.title.clone()).or_default();
            let occ = *n;
            *n += 1;
            occ
        };
        let filename = sync_filename(&e.title, &e.archive_type, occurrence);
        let path = dir.join(&filename);

        let status = if !path.is_file() {
            PlanStatus::New
        } else if e.archive_type != "folder" {
            // 压缩包：本地文件大小一致判“已同步”，不一致判“变化”
            let same_size = std::fs::metadata(&path)
                .map(|m| m.len() as i64 == e.file_size)
                .unwrap_or(false);
            if same_size {
                PlanStatus::UpToDate
            } else {
                PlanStatus::Changed
            }
        } else {
            // 文件夹：远端打包成 cbz 后大小与原目录无关（Windows 目录 size 甚至非 0），
            // 统一用本地同路径档案的页数与远端 page_count 对比
            let local = path
                .to_str()
                .and_then(|p| db.get_archive_by_path(p).ok().flatten());
            match local {
                Some(a) if a.page_count == e.page_count => PlanStatus::UpToDate,
                _ => PlanStatus::Changed,
            }
        };

        plan.push(PlanItem {
            archive: e.clone(),
            filename,
            status,
        });
    }
    plan
}

// ── 远端 HTTP（阻塞式，运行在 spawn_blocking）──

/// 同步目标合法性：见 `super::validate_outbound_url`（拒绝回环/本机/链路本地）。
fn validate_sync_url(url: &str) -> bool {
    super::validate_outbound_url(url)
}

fn base_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

fn authorized(mut req: ureq::Request, token: &str) -> ureq::Request {
    if !token.is_empty() {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    req
}

/// manifest 是远端可控数据：读取设 32MB 上限，防止恶意/异常远端撑爆内存。
const MANIFEST_MAX_BYTES: usize = 32 * 1024 * 1024;

fn fetch_manifest(url: &str, token: &str) -> anyhow::Result<serde_json::Value> {
    let req = authorized(
        ureq::get(&format!("{}/api/sync/manifest", base_url(url))),
        token,
    )
    .timeout(std::time::Duration::from_secs(60));
    let resp = req
        .call()
        .map_err(|e| anyhow::anyhow!("拉取远端清单失败: {e}"))?;
    let reader = resp.into_reader();
    let mut body = String::new();
    reader
        .take(MANIFEST_MAX_BYTES as u64)
        .read_to_string(&mut body)
        .map_err(|e| anyhow::anyhow!("读取远端清单失败: {e}"))?;
    if body.len() >= MANIFEST_MAX_BYTES {
        anyhow::bail!(
            "远端清单过大（超过 {}MB），已中止",
            MANIFEST_MAX_BYTES / 1024 / 1024
        );
    }
    Ok(serde_json::from_str(&body)?)
}

fn download_archive(
    url: &str,
    token: &str,
    id: i64,
    local_path: &Path,
    fallback_mtime: i64,
    cancel: &AtomicBool,
) -> anyhow::Result<()> {
    let req = authorized(
        ureq::post(&format!("{}/api/archives/{}/file", base_url(url), id)),
        token,
    )
    .timeout(std::time::Duration::from_secs(30 * 60));
    let resp = req
        .call()
        .map_err(|e| anyhow::anyhow!("下载档案 {id} 失败: {e}"))?;
    // 远端原始 mtime（秒）：优先用响应头；旧版远端没有该头时回退到清单里的 file_mtime
    let source_mtime = resp
        .header("X-Source-Mtime")
        .and_then(|v| v.trim().parse().ok())
        .filter(|s| *s > 0)
        .or_else(|| (fallback_mtime > 0).then_some(fallback_mtime));
    let mut reader = resp.into_reader();

    // 原子写：先写 .part 再 rename，避免半截文件被当成“已同步”。
    // 分块拷贝，每块后检查取消——取消立即中断并删除 .part，不留残留。
    let part = local_path.with_extension("part");
    let write_result: anyhow::Result<()> = (|| {
        let mut out = std::fs::File::create(&part)?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(anyhow::Error::new(SyncCancelled));
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| anyhow::anyhow!("读取下载流失败: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
        }
        out.flush()?;
        // 落地前恢复源 mtime，让本机文件时间与主机一致（register_local 读到的 file_mtime 亦然）
        if let Some(secs) = source_mtime.filter(|s| *s > 0) {
            use std::fs::FileTimes;
            let _ =
                out.set_times(FileTimes::new().set_modified(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64),
                ));
        }
        Ok(())
    })();
    write_result?;
    std::fs::rename(&part, local_path)?;
    Ok(())
}

/// 取消信号：任务被用户取消时返回的错误，不计入失败列表。
#[derive(Debug)]
struct SyncCancelled;
impl std::fmt::Display for SyncCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "同步已取消")
    }
}
impl std::error::Error for SyncCancelled {}

/// 清理同步目录里的 .part 残留（上次中断留下的半截文件）。
fn cleanup_part_files(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("part") {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

// ── 本地入库与元数据回填 ──

fn register_local(
    db: &crate::db::Database,
    entry: &ManifestArchive,
    path: &Path,
) -> anyhow::Result<i64> {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("本地路径非 UTF-8"))?;
    // folder 已在远端打包为 cbz
    let archive_type = if entry.archive_type == "folder" {
        "cbz"
    } else {
        entry.archive_type.as_str()
    };
    let page_count = crate::services::archive::create_archive_reader(path_str, archive_type)
        .ok()
        .and_then(|r| r.list_pages().ok())
        .map(|l| l.len() as i64)
        .unwrap_or(0);
    if page_count == 0 {
        anyhow::bail!("下载的档案没有可读页面，跳过: {}", entry.title);
    }
    let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    let file_mtime = crate::services::fs_ext::mtime_secs(path);
    db.upsert_scanned_archive(
        &entry.title,
        path_str,
        archive_type,
        page_count,
        file_size,
        file_mtime,
    )
    .map_err(anyhow::Error::from)
}

fn apply_metadata(
    db: &crate::db::Database,
    local_id: i64,
    title: &str,
    tag_ids_by_title: &HashMap<String, Vec<i64>>,
    category_ids_by_title: &HashMap<String, Vec<i64>>,
    history_by_title: &HashMap<String, (i64, i64)>,
) {
    if let Some(ids) = tag_ids_by_title.get(title) {
        for &tid in ids {
            let _ = db.assign_tag(local_id, tid);
        }
    }
    if let Some(ids) = category_ids_by_title.get(title) {
        for &cid in ids {
            let _ = db.assign_category(local_id, cid);
        }
    }
    if let Some(&(page, total)) = history_by_title.get(title) {
        let _ = db.save_history(local_id, page, total);
    }
}

/// 清理 sync_tmp 残留（下载文件夹档案时远端/本机生成的临时打包文件）。
fn cleanup_sync_tmp(data_dir: &Path) {
    let dir = data_dir.join("sync_tmp");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn run_sync_job(
    db: Arc<crate::db::Database>,
    data_dir: PathBuf,
    job: Arc<SyncJob>,
    url: String,
    token: String,
    dir: String,
) {
    job.running.store(true, Ordering::SeqCst);
    let sync_dir = PathBuf::from(&dir);

    let result = (|| -> anyhow::Result<()> {
        cleanup_sync_tmp(&data_dir);
        std::fs::create_dir_all(&sync_dir)?;
        // 清掉上次中断残留的 .part 半截文件
        cleanup_part_files(&sync_dir);

        let manifest = fetch_manifest(&url, &token)?;
        let entries: Vec<ManifestArchive> = serde_json::from_value(
            manifest
                .get("archives")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )?;
        job.total.store(entries.len(), Ordering::SeqCst);

        // 预建标签/分类（幂等），并索引各标题的关联
        let manifest_tags = manifest["tags"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let manifest_categories = manifest["categories"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut tag_ids_by_title: HashMap<String, Vec<i64>> = HashMap::new();
        for at in manifest["archive_tags"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let title = at["title"].as_str().unwrap_or("").to_string();
            let ns = at["namespace"].as_str().unwrap_or("");
            let name = at["name"].as_str().unwrap_or("");
            if title.is_empty() || name.is_empty() {
                continue;
            }
            // 按 ns:name 找到清单里的标签颜色
            let color = manifest_tags
                .iter()
                .find(|t| {
                    t["namespace"].as_str().unwrap_or("") == ns && t["name"].as_str() == Some(name)
                })
                .and_then(|t| t["color"].as_str())
                .unwrap_or("#4a86e8");
            let tag_id = db.get_or_create_tag(ns, name, color)?;
            tag_ids_by_title.entry(title).or_default().push(tag_id);
        }

        let mut category_ids_by_title: HashMap<String, Vec<i64>> = HashMap::new();
        for ac in manifest["archive_categories"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let title = ac["title"].as_str().unwrap_or("").to_string();
            let name = ac["name"].as_str().unwrap_or("");
            if title.is_empty() || name.is_empty() {
                continue;
            }
            let color = manifest_categories
                .iter()
                .find(|c| c["name"].as_str() == Some(name))
                .and_then(|c| c["color"].as_str())
                .unwrap_or("#4a86e8");
            let pinned = manifest_categories
                .iter()
                .find(|c| c["name"].as_str() == Some(name))
                .and_then(|c| c["pinned"].as_bool())
                .unwrap_or(false);
            let search = manifest_categories
                .iter()
                .find(|c| c["name"].as_str() == Some(name))
                .and_then(|c| c["search"].as_str())
                .unwrap_or("");
            let cat_id = db.get_or_create_category(name, color, pinned, search)?;
            category_ids_by_title.entry(title).or_default().push(cat_id);
        }

        let mut history_by_title: HashMap<String, (i64, i64)> = HashMap::new();
        for h in manifest["history"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            if let (Some(title), Some(page), Some(total)) = (
                h["title"].as_str(),
                h["page_index"].as_i64(),
                h["total_pages"].as_i64(),
            ) {
                history_by_title.insert(title.to_string(), (page, total));
            }
        }

        // 对比：计算同步计划（新增/更新/跳过），job total 只统计真正要做的工作
        let plan = build_plan(&db, &entries, &sync_dir);
        let work: Vec<&PlanItem> = plan
            .iter()
            .filter(|p| p.status != PlanStatus::UpToDate)
            .collect();
        let new_count = work.iter().filter(|p| p.status == PlanStatus::New).count();
        let changed_count = work.len() - new_count;
        let skipped = plan.len() - work.len();
        job.new_count.store(new_count, Ordering::SeqCst);
        job.changed_count.store(changed_count, Ordering::SeqCst);
        job.skipped.store(skipped, Ordering::SeqCst);
        job.total.store(work.len(), Ordering::SeqCst);

        for item in work {
            if job.cancel.load(Ordering::SeqCst) {
                break;
            }
            *job.current.lock().unwrap() = item.archive.title.clone();
            let local_path = sync_dir.join(&item.filename);

            // New/Changed 都真实下载（Changed 覆盖同名文件 = 覆盖更新）
            let outcome: anyhow::Result<i64> = (|| {
                download_archive(
                    &url,
                    &token,
                    item.archive.id,
                    &local_path,
                    item.archive.file_mtime,
                    &job.cancel,
                )?;
                register_local(&db, &item.archive, &local_path)
            })();
            match outcome {
                Ok(local_id) => {
                    apply_metadata(
                        &db,
                        local_id,
                        &item.archive.title,
                        &tag_ids_by_title,
                        &category_ids_by_title,
                        &history_by_title,
                    );
                }
                Err(e) => {
                    // 用户取消：中断当前条目（.part 已删除），静默结束，不计入失败
                    if e.downcast_ref::<SyncCancelled>().is_some() {
                        *job.current.lock().unwrap() =
                            "已取消，下次同步将只补未完成部分".to_string();
                        break;
                    }
                    job.failed
                        .lock()
                        .unwrap()
                        .push(format!("{}: {}", item.archive.title, e));
                }
            }
            job.done.fetch_add(1, Ordering::SeqCst);
        }

        Ok(())
    })();

    if let Err(e) = result {
        job.current
            .lock()
            .unwrap()
            .push_str(&format!(" —— 同步失败: {e}"));
    }
    // 兜底清理：无论正常结束还是取消，都不留 .part 残留
    cleanup_part_files(&sync_dir);
    job.running.store(false, Ordering::SeqCst);
    job.cancel.store(false, Ordering::SeqCst);
}

// ── HTTP 端点 ──

#[derive(Deserialize)]
pub struct SyncStartRequest {
    pub url: String,
    pub token: Option<String>,
    pub dir: String,
}

pub async fn sync_manifest(State(state): State<Arc<AppState>>) -> Response {
    db_json(&state, |db| db.sync_manifest()).await
}

/// 对比预览：拉取远端清单并与本地目录/本地库比对，返回差异统计与标题列表，
/// 不下载任何文件。用户确认后再调 /sync/start 只同步差异项。
pub async fn sync_plan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SyncStartRequest>,
) -> Response {
    let url = payload.url.trim().to_string();
    let dir = payload.dir.trim().to_string();
    if !validate_sync_url(&url) || dir.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "url 必须为 http(s):// 的局域网地址（拒绝回环/本机/链路本地）且必须指定本地目录",
        );
    }
    if !std::path::Path::new(&dir).is_absolute() {
        return error_response(StatusCode::BAD_REQUEST, "本地目录必须为绝对路径");
    }

    let db = state.db.clone();
    let token = payload.token.unwrap_or_default();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
        let manifest = fetch_manifest(&url, &token)?;
        let entries: Vec<ManifestArchive> = serde_json::from_value(
            manifest
                .get("archives")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )?;
        let plan = build_plan(&db, &entries, Path::new(&dir));
        let (mut new, mut changed, mut up_to_date) = (Vec::new(), Vec::new(), Vec::new());
        for item in plan {
            match item.status {
                PlanStatus::New => new.push(item.archive.title),
                PlanStatus::Changed => changed.push(item.archive.title),
                PlanStatus::UpToDate => up_to_date.push(item.archive.title),
            }
        }
        Ok(serde_json::json!({
            "total": entries.len(),
            "new": new,
            "changed": changed,
            "up_to_date": up_to_date,
        }))
    })
    .await;

    match result {
        Ok(Ok(plan)) => Json(plan).into_response(),
        Ok(Err(e)) => error_response(StatusCode::BAD_GATEWAY, &e.to_string()),
        Err(e) => internal_error(e),
    }
}

pub async fn sync_start(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SyncStartRequest>,
) -> Response {
    let url = payload.url.trim().to_string();
    let dir = payload.dir.trim().to_string();
    if !validate_sync_url(&url) || dir.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "url 必须为 http(s):// 的局域网地址（拒绝回环/本机/链路本地）且必须指定本地目录",
        );
    }
    if !std::path::Path::new(&dir).is_absolute() {
        return error_response(StatusCode::BAD_REQUEST, "本地目录必须为绝对路径");
    }

    {
        let job = current_job().lock().unwrap();
        if let Some(j) = job.as_ref() {
            if j.running.load(Ordering::SeqCst) {
                return error_response(StatusCode::CONFLICT, "已有同步任务在运行");
            }
        }
    }

    let job = SyncJob::new();
    *current_job().lock().unwrap() = Some(job.clone());
    let db = state.db.clone();
    let data_dir = state.data_dir.clone();
    let token = payload.token.unwrap_or_default();
    tokio::task::spawn_blocking(move || run_sync_job(db, data_dir, job, url, token, dir));

    Json(serde_json::json!({ "started": true })).into_response()
}

pub async fn sync_status() -> Response {
    let json = current_job()
        .lock()
        .unwrap()
        .as_ref()
        .map(|j| j.to_json())
        .unwrap_or(serde_json::json!({ "running": false, "total": 0, "done": 0, "new": 0, "changed": 0, "skipped": 0, "current": "", "failed": [] }));
    Json(json).into_response()
}

pub async fn sync_cancel() -> Response {
    if let Some(job) = current_job().lock().unwrap().as_ref() {
        job.cancel.store(true, Ordering::SeqCst);
    }
    Json(serde_json::json!({ "cancelled": true })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 起一个假“远端”服务（127.0.0.1 回环即可；URL 白名单单独单测，
    /// 这里直接调用任务函数绕开 start 端点的地址校验，但清单/下载仍走真实 HTTP）。
    async fn spawn_remote(db: Arc<crate::db::Database>, data_dir: PathBuf) -> u16 {
        let state = crate::AppState {
            db,
            data_dir,
            last_thumb_eviction: Arc::new(std::sync::Mutex::new(None)),
        };
        let app = crate::routes::create_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        port
    }

    #[test]
    fn sync_url_rejects_loopback_and_link_local() {
        assert!(!validate_sync_url("http://127.0.0.1:5002"));
        assert!(!validate_sync_url("http://localhost:5002"));
        assert!(!validate_sync_url("http://0.0.0.0:5002"));
        assert!(!validate_sync_url("http://169.254.1.1:5002"));
        assert!(!validate_sync_url("http://[::1]:5002"));
        assert!(!validate_sync_url("ftp://192.168.1.2"));
    }

    #[test]
    fn sync_url_accepts_lan_private_ranges() {
        assert!(validate_sync_url("http://192.168.31.52:5002"));
        assert!(validate_sync_url("http://10.0.0.1:5002/"));
        assert!(validate_sync_url("http://172.16.0.9:5002"));
    }

    #[test]
    fn sync_filename_sanitizes_and_avoids_windows_reserved_names() {
        assert_eq!(sync_filename("海贼王", "cbz", 0), "海贼王.cbz");
        assert_eq!(sync_filename("CON", "cbz", 0), "_CON.cbz");
        assert_eq!(sync_filename("NUL", "folder", 0), "_NUL.cbz");
        assert_eq!(sync_filename("a/b:c*?", "rar", 0), "a_b_c__.rar");
        // 同名出现多次：第 0 次用原名，之后加 _N 后缀避免互相覆盖
        assert_eq!(sync_filename("海贼王", "cbz", 0), "海贼王.cbz");
        assert_eq!(sync_filename("海贼王", "cbz", 1), "海贼王_2.cbz");
        assert_eq!(sync_filename("海贼王", "cbz", 2), "海贼王_3.cbz");
    }

    /// 计划分类：新增 / 更新（同名不同大小）/ 已最新（同名同大小）。
    #[test]
    fn build_plan_classifies_new_changed_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        // 已存在两个档案文件：已同步.cbz(100)/变化.cbz(10)
        std::fs::write(dir.path().join("已同步.cbz"), vec![0u8; 100]).unwrap();
        std::fs::write(dir.path().join("变化.cbz"), vec![0u8; 10]).unwrap();

        let db = crate::db::Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();

        let entries = vec![
            ManifestArchive {
                id: 1,
                title: "新增".into(),
                archive_type: "cbz".into(),
                page_count: 1,
                file_size: 50,
                file_mtime: 0,
            },
            ManifestArchive {
                id: 2,
                title: "已同步".into(),
                archive_type: "cbz".into(),
                page_count: 1,
                file_size: 100,
                file_mtime: 0,
            },
            ManifestArchive {
                id: 3,
                title: "变化".into(),
                archive_type: "cbz".into(),
                page_count: 1,
                file_size: 200,
                file_mtime: 0,
            },
        ];
        let plan = build_plan(&db, &entries, dir.path());
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].status, PlanStatus::New);
        assert_eq!(plan[1].status, PlanStatus::UpToDate);
        assert_eq!(plan[2].status, PlanStatus::Changed);
    }

    /// 同步任务端到端：远端有 1 个文件夹档案 + 标签 + 进度，
    /// 应下载到本地目录、入库并回填元数据（清单/下载均走真实 HTTP）。
    // 需要多线程 runtime：任务在测试线程里同步阻塞（ureq），远端服务必须由工作线程轮询。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sync_job_downloads_registers_and_backfills() {
        // ── 远端 ──
        let remote_dir = tempfile::tempdir().unwrap();
        let remote_root = remote_dir.path().join("library");
        std::fs::create_dir_all(remote_root.join("series1/ch01")).unwrap();
        std::fs::write(remote_root.join("series1/ch01/page01.jpg"), b"img01").unwrap();
        std::fs::write(remote_root.join("series1/ch01/page02.png"), b"img02").unwrap();

        let remote_db =
            crate::db::Database::new(remote_dir.path().join("r.db").to_str().unwrap()).unwrap();
        remote_db.init().unwrap();
        let root_s = remote_root.to_string_lossy().into_owned();
        let rid = remote_db
            .upsert_scanned_archive(
                "系列1",
                &format!("{root_s}/series1/ch01"),
                "folder",
                2,
                10,
                111,
            )
            .unwrap();
        let tag_id = remote_db.create_tag("", "热血", "#e5484d").unwrap();
        remote_db.assign_tag(rid, tag_id).unwrap();
        remote_db.save_history(rid, 1, 2).unwrap();
        let remote_port = spawn_remote(Arc::new(remote_db), remote_dir.path().to_path_buf()).await;

        // ── 本机 ──
        let local_dir = tempfile::tempdir().unwrap();
        let local_db = Arc::new(
            crate::db::Database::new(local_dir.path().join("l.db").to_str().unwrap()).unwrap(),
        );
        local_db.init().unwrap();
        let sync_dir = local_dir.path().join("synced");
        std::fs::create_dir_all(&sync_dir).unwrap();
        let job = SyncJob::new();

        run_sync_job(
            local_db.clone(),
            local_dir.path().to_path_buf(),
            job.clone(),
            format!("http://127.0.0.1:{remote_port}"),
            String::new(),
            sync_dir.to_string_lossy().into_owned(),
        );

        assert!(!job.running.load(Ordering::SeqCst), "任务应已结束");
        assert!(
            job.failed.lock().unwrap().is_empty(),
            "不应有失败项: {:?}",
            *job.failed.lock().unwrap()
        );
        assert_eq!(
            job.new_count.load(Ordering::SeqCst),
            1,
            "首次同步应为新增 1"
        );
        assert_eq!(job.changed_count.load(Ordering::SeqCst), 0);
        assert_eq!(job.skipped.load(Ordering::SeqCst), 0);

        let files: Vec<_> = std::fs::read_dir(&sync_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(files.len(), 1, "应下载一个档案文件");
        let la = local_db
            .get_archive_by_path(files[0].to_str().unwrap())
            .unwrap()
            .expect("档案应已入库");
        assert_eq!(la.archive_type, "cbz", "folder 应打包为 cbz");
        assert_eq!(la.title, "系列1");
        let tags = local_db.get_archive_tags(la.id).unwrap();
        assert_eq!(tags.len(), 1, "标签应按标题回填");
        assert_eq!(tags[0].name, "热血");
        let hist = local_db.get_history_for_archive(la.id).unwrap();
        assert!(hist.is_some(), "阅读进度应回填");
        assert_eq!(hist.unwrap().page_index, 1);
    }

    /// 覆盖更新：首次同步 → 重跑应全跳过（不产生 _2 副本）；远端内容变化
    /// （页数变化）→ 重跑应覆盖同名文件并原地更新页数，仍是一条记录。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sync_job_resync_skips_then_overwrites_changed() {
        // ── 远端（挂载后需要能再改盘 + 改库）──
        let remote_dir = tempfile::tempdir().unwrap();
        let remote_root = remote_dir.path().join("library");
        std::fs::create_dir_all(remote_root.join("series1/ch01")).unwrap();
        std::fs::write(remote_root.join("series1/ch01/page01.jpg"), b"img01").unwrap();
        std::fs::write(remote_root.join("series1/ch01/page02.png"), b"img02").unwrap();
        let remote_db = Arc::new(
            crate::db::Database::new(remote_dir.path().join("r.db").to_str().unwrap()).unwrap(),
        );
        remote_db.init().unwrap();
        let root_s = remote_root.to_string_lossy().into_owned();
        let rid = remote_db
            .upsert_scanned_archive(
                "系列1",
                &format!("{root_s}/series1/ch01"),
                "folder",
                2,
                10,
                111,
            )
            .unwrap();
        let remote_port = spawn_remote(remote_db.clone(), remote_dir.path().to_path_buf()).await;

        let local_dir = tempfile::tempdir().unwrap();
        let local_db = Arc::new(
            crate::db::Database::new(local_dir.path().join("l.db").to_str().unwrap()).unwrap(),
        );
        local_db.init().unwrap();
        let sync_dir = local_dir.path().join("synced");
        std::fs::create_dir_all(&sync_dir).unwrap();
        let url = format!("http://127.0.0.1:{remote_port}");

        // 第一次同步：新增 1
        let job1 = SyncJob::new();
        run_sync_job(
            local_db.clone(),
            local_dir.path().to_path_buf(),
            job1.clone(),
            url.clone(),
            String::new(),
            sync_dir.to_string_lossy().into_owned(),
        );
        assert_eq!(job1.new_count.load(Ordering::SeqCst), 1);
        assert!(
            job1.failed.lock().unwrap().is_empty(),
            "首次同步不应失败: {:?}",
            *job1.failed.lock().unwrap()
        );

        // 直接重跑：全部已最新 → 跳过，文件数不变
        let job2 = SyncJob::new();
        run_sync_job(
            local_db.clone(),
            local_dir.path().to_path_buf(),
            job2.clone(),
            url.clone(),
            String::new(),
            sync_dir.to_string_lossy().into_owned(),
        );
        assert_eq!(job2.skipped.load(Ordering::SeqCst), 1, "重跑应全部跳过");
        assert_eq!(
            std::fs::read_dir(&sync_dir).unwrap().count(),
            1,
            "不应产生 _2 副本"
        );

        // 远端内容变化：磁盘加一页 + 库页数更新（模拟新增一话后重新扫描）
        std::fs::write(remote_root.join("series1/ch01/page03.webp"), b"img03").unwrap();
        remote_db
            .upsert_scanned_archive(
                "系列1",
                &format!("{root_s}/series1/ch01"),
                "folder",
                3,
                10,
                222,
            )
            .unwrap();
        let _ = rid;

        // 第三次同步：判定 Changed → 覆盖同名文件，原地更新页数，仍只有一条记录
        let job3 = SyncJob::new();
        run_sync_job(
            local_db.clone(),
            local_dir.path().to_path_buf(),
            job3.clone(),
            url.clone(),
            String::new(),
            sync_dir.to_string_lossy().into_owned(),
        );
        assert_eq!(job3.changed_count.load(Ordering::SeqCst), 1, "应判定为更新");
        assert!(
            job3.failed.lock().unwrap().is_empty(),
            "更新不应失败: {:?}",
            *job3.failed.lock().unwrap()
        );
        let files: Vec<_> = std::fs::read_dir(&sync_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(files.len(), 1, "覆盖更新后仍只有一条文件");
        let la = local_db
            .get_archive_by_path(files[0].to_str().unwrap())
            .unwrap()
            .expect("档案应存在");
        assert_eq!(la.page_count, 3, "页数应随覆盖更新刷新");
    }

    /// 时间信息保留：远端压缩包 mtime=T → 同步后本机文件 mtime 与库里 file_mtime 都等于 T。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sync_job_preserves_source_mtime() {
        let remote_dir = tempfile::tempdir().unwrap();
        let zip_path = remote_dir.path().join("sample.zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("p1.jpg", opts).unwrap();
            std::io::Write::write_all(&mut zw, b"img1").unwrap();
            zw.start_file("p2.png", opts).unwrap();
            std::io::Write::write_all(&mut zw, b"img2").unwrap();
            zw.finish().unwrap();
        }
        let t_secs: i64 = 1_700_000_000;
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(t_secs as u64);
        {
            let f = std::fs::File::open(&zip_path).unwrap();
            f.set_times(std::fs::FileTimes::new().set_modified(t))
                .unwrap();
        }

        let remote_db = Arc::new(
            crate::db::Database::new(remote_dir.path().join("r.db").to_str().unwrap()).unwrap(),
        );
        remote_db.init().unwrap();
        let zs = zip_path.to_string_lossy().into_owned();
        remote_db
            .upsert_scanned_archive("样本", &zs, "zip", 2, 8, t_secs)
            .unwrap();
        let remote_port = spawn_remote(remote_db.clone(), remote_dir.path().to_path_buf()).await;

        let local_dir = tempfile::tempdir().unwrap();
        let local_db = Arc::new(
            crate::db::Database::new(local_dir.path().join("l.db").to_str().unwrap()).unwrap(),
        );
        local_db.init().unwrap();
        let sync_dir = local_dir.path().join("synced");
        std::fs::create_dir_all(&sync_dir).unwrap();
        let job = SyncJob::new();
        run_sync_job(
            local_db.clone(),
            local_dir.path().to_path_buf(),
            job.clone(),
            format!("http://127.0.0.1:{remote_port}"),
            String::new(),
            sync_dir.to_string_lossy().into_owned(),
        );

        assert!(
            job.failed.lock().unwrap().is_empty(),
            "同步不应失败: {:?}",
            *job.failed.lock().unwrap()
        );
        let files: Vec<_> = std::fs::read_dir(&sync_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        assert_eq!(files.len(), 1);
        let got = std::fs::metadata(&files[0])
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(got, t_secs, "本机文件 mtime 应等于源 mtime");
        let conn = local_db.conn_for_test().unwrap();
        let fm: i64 = conn
            .query_row(
                "SELECT file_mtime FROM archives WHERE path = ?1",
                [files[0].to_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fm, t_secs, "库里 file_mtime 应为源 mtime");
    }

    /// 取消：下载中被取消 → 立即中断当前条目，无 .part 残留、不算失败、不计数。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sync_job_cancel_stops_download_cleanly() {
        use axum::extract::Path as AxumPath;
        use axum::http::StatusCode as HttpStatus;
        use axum::routing::{get, post};

        // 慢速 /file：睡 400ms 再返回，保证下载请求已发出但数据未到齐时取消能落在“下载中”
        async fn slow_file(_: AxumPath<i64>) -> axum::response::Response {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            let body = vec![0u8; 8 * 1024 * 1024];
            (
                HttpStatus::OK,
                [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
                body,
            )
                .into_response()
        }

        let remote_dir = tempfile::tempdir().unwrap();
        let remote_db = Arc::new(
            crate::db::Database::new(remote_dir.path().join("r.db").to_str().unwrap()).unwrap(),
        );
        remote_db.init().unwrap();
        remote_db
            .upsert_scanned_archive("慢速", "/fake/slow.zip", "zip", 100, 8 * 1024 * 1024, 1)
            .unwrap();
        let state = crate::AppState {
            db: remote_db.clone(),
            data_dir: remote_dir.path().to_path_buf(),
            last_thumb_eviction: Arc::new(std::sync::Mutex::new(None)),
        };
        let app = axum::Router::new()
            .route(
                "/api/sync/manifest",
                get(crate::routes::sync::sync_manifest),
            )
            .route("/api/archives/:id/file", post(slow_file))
            .with_state(Arc::new(state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote_port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let local_dir = tempfile::tempdir().unwrap();
        let local_db = Arc::new(
            crate::db::Database::new(local_dir.path().join("l.db").to_str().unwrap()).unwrap(),
        );
        local_db.init().unwrap();
        let sync_dir = local_dir.path().join("synced");
        std::fs::create_dir_all(&sync_dir).unwrap();
        let job = SyncJob::new();

        // 120ms 后置取消：manifest/计划早已完成，下载正卡在慢 handler 的 sleep 上
        let cancel_job = job.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            cancel_job.cancel.store(true, Ordering::SeqCst);
        });
        run_sync_job(
            local_db.clone(),
            local_dir.path().to_path_buf(),
            job.clone(),
            format!("http://127.0.0.1:{remote_port}"),
            String::new(),
            sync_dir.to_string_lossy().into_owned(),
        );

        assert!(!job.running.load(Ordering::SeqCst), "任务应已结束");
        assert!(
            job.failed.lock().unwrap().is_empty(),
            "取消不应计入失败: {:?}",
            *job.failed.lock().unwrap()
        );
        assert_eq!(job.done.load(Ordering::SeqCst), 0, "被取消的条目不应计数");
        assert_eq!(
            std::fs::read_dir(&sync_dir).unwrap().count(),
            0,
            "不应残留 .part 或半截文件"
        );
    }
}
