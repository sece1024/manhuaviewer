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

use super::{error_response, internal_error};

/// 单任务并发：重复 start 会返回 409。
static CURRENT_JOB: OnceLock<Mutex<Option<Arc<SyncJob>>>> = OnceLock::new();

fn current_job() -> &'static Mutex<Option<Arc<SyncJob>>> {
    CURRENT_JOB.get_or_init(|| Mutex::new(None))
}

pub struct SyncJob {
    running: AtomicBool,
    cancel: AtomicBool,
    total: AtomicUsize,
    done: AtomicUsize,
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
            current: Mutex::new(String::new()),
            failed: Mutex::new(Vec::new()),
        })
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "running": self.running.load(Ordering::SeqCst),
            "total": self.total.load(Ordering::SeqCst),
            "done": self.done.load(Ordering::SeqCst),
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
    file_size: i64,
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

/// 由标题生成安全的本地文件名：清掉路径分隔符/非法字符，冲突时追加 _2、_3…
fn local_filename(title: &str, archive_type: &str, dir: &Path, size: i64) -> String {
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
        "archive".to_string()
    } else {
        base
    };
    // Windows 保留设备名（CON/PRN/AUX/NUL/COM*/LPT*）与尾随点/空格：加下划线前缀避免落盘失败
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = base.split('.').next().unwrap_or(&base).to_ascii_uppercase();
    let base = if RESERVED.contains(&stem.as_str()) || base.ends_with('.') || base.ends_with(' ') {
        format!("_{}", base.trim_end_matches(['.', ' ']))
    } else {
        base
    };
    let ext = extension_for(archive_type);
    let mut candidate = format!("{base}.{ext}");
    if size > 0 {
        let mut i = 2u32;
        while dir.join(&candidate).is_file() {
            // 同名同大小视为已同步（断点续传）
            if std::fs::metadata(dir.join(&candidate))
                .map(|m| m.len() as i64 == size)
                .unwrap_or(false)
            {
                break;
            }
            candidate = format!("{base}_{i}.{ext}");
            i += 1;
        }
    }
    candidate
}

// ── 远端 HTTP（阻塞式，运行在 spawn_blocking）──

/// 同步目标合法性：同步的本意是“局域网内另一台相同软件”，
/// 因此只放行私网/普通公网地址，拒绝回环、未指定、链路本地与组播——
/// 防止本机后端被当成 SSRF 跳板访问本机自身或内网无关服务。
fn validate_sync_url(url: &str) -> bool {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return false;
    }
    let rest = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let hostname = host
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(host)
        .trim_start_matches('[')
        .trim_end_matches(']');

    fn allowed(ip: std::net::IpAddr) -> bool {
        if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
            return false;
        }
        match ip {
            std::net::IpAddr::V4(v4) => !v4.is_link_local(),
            std::net::IpAddr::V6(v6) => !v6.is_unicast_link_local(),
        }
    }

    if let Ok(ip) = hostname.parse::<std::net::IpAddr>() {
        return allowed(ip);
    }
    // 主机名：解析全部地址，任一允许即视为合法（LAN 常用主机名）
    use std::net::ToSocketAddrs;
    match (hostname, 80u16).to_socket_addrs() {
        Ok(addrs) => addrs.map(|a| a.ip()).any(allowed),
        Err(_) => false,
    }
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

fn download_archive(url: &str, token: &str, id: i64, local_path: &Path) -> anyhow::Result<()> {
    let req = authorized(
        ureq::post(&format!("{}/api/archives/{}/file", base_url(url), id)),
        token,
    )
    .timeout(std::time::Duration::from_secs(30 * 60));
    let resp = req
        .call()
        .map_err(|e| anyhow::anyhow!("下载档案 {id} 失败: {e}"))?;
    let mut reader = resp.into_reader();

    // 原子写：先写 .part 再 rename，避免半截文件被当成“已同步”
    let part = local_path.with_extension("part");
    let mut out = std::fs::File::create(&part)?;
    std::io::copy(&mut reader, &mut out)?;
    out.flush()?;
    std::fs::rename(&part, local_path)?;
    Ok(())
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
    if let Some(existing) = db.get_archive_by_path(path_str)? {
        return Ok(existing.id);
    }
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
    let file_mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
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

        for entry in &entries {
            if job.cancel.load(Ordering::SeqCst) {
                break;
            }
            *job.current.lock().unwrap() = entry.title.clone();

            let filename = local_filename(
                &entry.title,
                &entry.archive_type,
                &sync_dir,
                entry.file_size,
            );
            let local_path = sync_dir.join(&filename);
            let already = local_path.is_file()
                && entry.file_size > 0
                && std::fs::metadata(&local_path)
                    .map(|m| m.len() as i64 == entry.file_size)
                    .unwrap_or(false);

            let outcome: anyhow::Result<i64> = (|| {
                if !already {
                    download_archive(&url, &token, entry.id, &local_path)?;
                }
                register_local(&db, entry, &local_path)
            })();
            match outcome {
                Ok(local_id) => {
                    apply_metadata(
                        &db,
                        local_id,
                        &entry.title,
                        &tag_ids_by_title,
                        &category_ids_by_title,
                        &history_by_title,
                    );
                }
                Err(e) => {
                    job.failed
                        .lock()
                        .unwrap()
                        .push(format!("{}: {}", entry.title, e));
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
    match super::run_db(&state, |db| db.sync_manifest()).await {
        Ok(manifest) => Json(manifest).into_response(),
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
        .unwrap_or(serde_json::json!({ "running": false, "total": 0, "done": 0, "current": "", "failed": [] }));
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
    fn local_filename_sanitizes_and_avoids_windows_reserved_names() {
        let dir = Path::new("/tmp/synctest");
        assert_eq!(local_filename("海贼王", "cbz", dir, 0), "海贼王.cbz");
        assert_eq!(local_filename("CON", "cbz", dir, 0), "_CON.cbz");
        assert_eq!(local_filename("NUL", "folder", dir, 0), "_NUL.cbz");
        assert_eq!(local_filename("a/b:c*?", "rar", dir, 0), "a_b_c__.rar");
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
}
