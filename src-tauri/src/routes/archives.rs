use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use std::time::SystemTime;

use super::error_response;

const CACHE_CONTROL: &str = "private, max-age=86400";

fn archive_mtime(path: &str) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn archive_mtime_secs(path: &str) -> i64 {
    archive_mtime(path)
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn is_compressed(archive_type: &str) -> bool {
    matches!(archive_type, "zip" | "rar" | "cbz" | "cbr" | "7z")
}

fn to_page_rows(archive_id: i64, list: &[String]) -> Vec<crate::db::PageRow> {
    list.iter()
        .enumerate()
        .map(|(i, p)| {
            let filename = std::path::Path::new(p)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            crate::db::PageRow {
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
fn load_page_rows(
    db: &crate::db::Database,
    archive_id: i64,
    archive_path: &str,
    archive_type: &str,
    mtime_secs: i64,
) -> anyhow::Result<Vec<crate::db::PageRow>> {
    let reader = crate::services::archive::create_archive_reader(archive_path, archive_type)?;
    if is_compressed(archive_type) {
        let cached_mtime = db.get_page_list_mtime(archive_id).ok().flatten();
        if cached_mtime == Some(mtime_secs) {
            let cached = db.get_pages(archive_id).unwrap_or_default();
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
        let list = reader.list_pages()?;
        let rows = to_page_rows(archive_id, &list);
        let _ = db.save_pages(archive_id, &rows, mtime_secs);
        Ok(rows)
    } else {
        Ok(to_page_rows(archive_id, &reader.list_pages()?))
    }
}

fn etag_for_page(id: i64, page_index: i64, mtime: Option<SystemTime>) -> String {
    let secs = mtime
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("\"p-{}-{}-{}\"", id, page_index, secs)
}

fn etag_for_cover(id: i64, mtime: Option<SystemTime>) -> String {
    let secs = mtime
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("\"c-{}-{}\"", id, secs)
}

fn http_date(t: SystemTime) -> Option<String> {
    let dt: DateTime<Utc> = t.into();
    Some(dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string())
}

fn parse_http_date(s: &str) -> Option<SystemTime> {
    DateTime::parse_from_rfc2822(s)
        .ok()
        .map(|d| d.with_timezone(&Utc).into())
}

fn build_response<B>(status: StatusCode, pairs: Vec<(&'static str, String)>, body: B) -> Response
where
    B: IntoResponse,
{
    let mut hm = HeaderMap::new();
    for (k, v) in pairs {
        if let (Ok(name), Ok(val)) = (k.parse::<HeaderName>(), v.parse::<HeaderValue>()) {
            hm.insert(name, val);
        }
    }
    (status, hm, body).into_response()
}

fn not_modified(etag: String, last_modified: Option<String>) -> Response {
    let mut pairs: Vec<(&'static str, String)> =
        vec![("ETag", etag), ("Cache-Control", CACHE_CONTROL.to_string())];
    if let Some(lm) = last_modified {
        pairs.push(("Last-Modified", lm));
    }
    build_response(StatusCode::NOT_MODIFIED, pairs, "")
}

#[derive(Deserialize)]
pub struct ArchiveQuery {
    #[serde(alias = "sort_by")]
    pub sort: Option<String>,
    #[serde(alias = "sort_order")]
    pub order: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
    pub tag: Option<String>,
    pub category_id: Option<i64>,
    pub group_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct OpenFileRequest {
    #[serde(alias = "filePath")]
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct ScanRequest {
    pub path: Option<String>,
    pub depth: Option<u32>,
}

#[derive(Deserialize)]
pub struct PackCbzRequest {
    /// 源文件夹路径
    #[serde(alias = "folderPath")]
    pub folder_path: String,
    /// 可选：覆盖归档目录（不传则从 settings 读取）
    #[serde(alias = "outputDir")]
    pub output_dir: Option<String>,
}

pub async fn list_archives(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArchiveQuery>,
) -> Response {
    enum ListResult {
        Group(Vec<crate::db::ArchiveRow>),
        List(Vec<crate::db::ArchiveRow>),
    }

    let result = super::run_db(&state, move |db| {
        // 如果指定了 group_id，返回组内所有章节
        if let Some(group_id) = query.group_id {
            return db.get_group_chapters(group_id).map(ListResult::Group);
        }

        let page = query.page.unwrap_or(1);
        let limit = query.limit.unwrap_or(20);
        let offset = (page - 1) * limit;
        let sort = query.sort.as_deref().unwrap_or("updated");
        let order = query.order.as_deref().unwrap_or("desc");

        db.list_archives(
            query.search.as_deref(),
            query.tag.as_deref(),
            query.category_id,
            sort,
            order,
            limit,
            offset,
        )
        .map(ListResult::List)
    })
    .await;

    match result {
        Ok(ListResult::Group(chapters)) => Json(chapters).into_response(),
        Ok(ListResult::List(archives)) => Json(archives).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_archive(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    match super::run_db(&state, move |db| db.get_archive(id)).await {
        Ok(Some(archive)) => Json(serde_json::json!({ "data": archive })).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete_archive(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let thumb_dir = state.data_dir.join("thumbnails").join(id.to_string());

    match super::run_db(&state, move |db| db.delete_archive(id)).await {
        Ok(_) => {
            // 删除缩略图目录
            let _ = tokio::fs::remove_dir_all(&thumb_dir).await;
            Json(serde_json::json!({ "success": true })).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct BatchDeleteRequest {
    pub ids: Vec<i64>,
}

pub async fn batch_delete_archives(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BatchDeleteRequest>,
) -> Response {
    if payload.ids.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "ids 不能为空");
    }

    let ids = payload.ids;
    let ids_db = ids.clone();
    match super::run_db(&state, move |db| db.batch_delete_archives(&ids_db)).await {
        Ok(affected) => {
            // 逐个清理缩略图目录
            for id in &ids {
                let thumb_dir = state.data_dir.join("thumbnails").join(id.to_string());
                let _ = tokio::fs::remove_dir_all(&thumb_dir).await;
            }
            Json(serde_json::json!({ "success": true, "affected": affected })).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct UpdateTitleRequest {
    title: String,
}

pub async fn update_archive_title(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateTitleRequest>,
) -> Response {
    let title_db = payload.title.clone();
    match super::run_db(&state, move |db| db.update_archive_title(id, &title_db)).await {
        Ok(_) => Json(serde_json::json!({ "id": id, "title": payload.title })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct MergeRequest {
    archive_ids: Vec<i64>,
}

pub async fn merge_archives(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MergeRequest>,
) -> Response {
    if payload.archive_ids.len() < 2 {
        return error_response(StatusCode::BAD_REQUEST, "需要至少选择 2 个档案进行合并");
    }

    let ids = payload.archive_ids;
    let result = super::run_db(&state, move |db| {
        let primary_id = db.merge_archives(&ids)?;
        let chapter_count = db.get_group_chapters(primary_id)?.len();
        Ok((primary_id, chapter_count))
    })
    .await;

    match result {
        Ok((primary_id, chapter_count)) => Json(serde_json::json!({
            "group_id": primary_id,
            "chapter_count": chapter_count,
        }))
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_cover(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let (archive_path, archive_type) = match super::run_db(&state, move |db| db.get_archive(id))
        .await
    {
        Ok(Some(a)) => (a.path, a.archive_type),
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let mtime = archive_mtime(&archive_path);
    let etag = etag_for_cover(id, mtime);
    let last_modified = mtime.and_then(http_date);

    if let Some(inm) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
        if inm == etag {
            return not_modified(etag, last_modified);
        }
    }
    if let (Some(ims), Some(mt)) = (
        headers
            .get("if-modified-since")
            .and_then(|v| v.to_str().ok()),
        mtime,
    ) {
        if let Some(parsed) = parse_http_date(ims) {
            if mt <= parsed {
                return not_modified(etag, last_modified);
            }
        }
    }

    let result = tokio::task::spawn_blocking(move || {
        let reader = crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
        reader.get_cover()
    })
    .await;

    match result {
        Ok(Ok(cover_data)) => {
            let mut pairs: Vec<(&'static str, String)> = vec![
                ("Content-Type", "image/jpeg".to_string()),
                ("ETag", etag),
                ("Cache-Control", CACHE_CONTROL.to_string()),
            ];
            if let Some(lm) = last_modified {
                pairs.push(("Last-Modified", lm));
            }
            build_response(StatusCode::OK, pairs, cover_data)
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Internal error: {}", e),
        ),
    }
}

pub async fn list_pages(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let (archive, read_page) = match super::run_db(&state, move |db| {
        let archive = db.get_archive(id)?;
        let read_page = db
            .get_history_for_archive(id)
            .map(|h| h.map(|h| h.page_index).unwrap_or(0))
            .unwrap_or(0);
        Ok((archive, read_page))
    })
    .await
    {
        Ok((Some(a), read_page)) => (a, read_page),
        Ok((None, _)) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let archive_path = archive.path.clone();
    let archive_type = archive.archive_type.clone();
    let archive_id = archive.id;
    let mtime_secs = archive_mtime_secs(&archive_path);
    let db = state.db.clone();

    let result = tokio::task::spawn_blocking(move || {
        load_page_rows(&db, archive_id, &archive_path, &archive_type, mtime_secs)
    })
    .await;

    match result {
        Ok(Ok(pages)) => {
            let page_list: Vec<serde_json::Value> = pages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    serde_json::json!({
                        "id": i,
                        "archive_id": id,
                        "filename": p.filename,
                        "filepath": p.filepath,
                        "sort_order": i,
                        "url": format!("/api/archives/{}/pages/{}", id, i),
                        "thumb_url": format!("/api/archives/{}/pages/{}/thumb", id, i),
                    })
                })
                .collect();

            Json(serde_json::json!({
                "archive": {
                    "id": archive.id,
                    "title": archive.title,
                    "archive_type": archive.archive_type,
                    "path": archive.path,
                },
                "pages": page_list,
                "read_page": read_page,
            }))
            .into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Internal error: {}", e),
        ),
    }
}

pub async fn get_page(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
    headers: HeaderMap,
) -> Response {
    if page_index < 0 {
        return error_response(StatusCode::BAD_REQUEST, "Page index must be non-negative");
    }

    let (archive_id, archive_path, archive_type) = match super::run_db(&state, move |db| {
        db.get_archive(id)
    })
    .await
    {
        Ok(Some(a)) => (a.id, a.path, a.archive_type),
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let mtime = archive_mtime(&archive_path);
    let etag = etag_for_page(id, page_index, mtime);
    let last_modified = mtime.and_then(http_date);

    if let Some(inm) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
        if inm == etag {
            return not_modified(etag, last_modified);
        }
    }
    if let (Some(ims), Some(mt)) = (
        headers
            .get("if-modified-since")
            .and_then(|v| v.to_str().ok()),
        mtime,
    ) {
        if let Some(parsed) = parse_http_date(ims) {
            if mt <= parsed {
                return not_modified(etag, last_modified);
            }
        }
    }

    let mtime_secs = archive_mtime_secs(&archive_path);
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let pages = load_page_rows(&db, archive_id, &archive_path, &archive_type, mtime_secs)?;
        let idx = page_index as usize;
        if idx >= pages.len() {
            anyhow::bail!("Page index {} out of range (total: {})", idx, pages.len());
        }
        let page_name = &pages[idx].filepath;
        let mime = mime_guess::from_path(page_name)
            .first_or_octet_stream()
            .to_string();
        if is_compressed(&archive_type) {
            // 压缩包：解压后整体返回（需要解压，无法直接流式）
            let reader =
                crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
            let data = reader.extract_page(page_name)?;
            Ok::<_, anyhow::Error>((Some(data), mime, None))
        } else {
            // 文件夹：直接流式输出文件，避免整页 2-10MB 双份内存缓冲
            Ok((None, mime, Some(page_name.clone())))
        }
    })
    .await;

    match result {
        Ok(Ok((Some(data), mime, _))) => {
            let mut pairs: Vec<(&'static str, String)> = vec![
                ("Content-Type", mime),
                ("ETag", etag),
                ("Cache-Control", CACHE_CONTROL.to_string()),
            ];
            if let Some(lm) = last_modified {
                pairs.push(("Last-Modified", lm));
            }
            build_response(StatusCode::OK, pairs, data)
        }
        Ok(Ok((None, mime, Some(stream_path)))) => match tokio::fs::File::open(&stream_path).await
        {
            Ok(file) => {
                let stream = tokio_util::io::ReaderStream::new(file);
                let mut pairs: Vec<(&'static str, String)> = vec![
                    ("Content-Type", mime),
                    ("ETag", etag),
                    ("Cache-Control", CACHE_CONTROL.to_string()),
                ];
                if let Some(lm) = last_modified {
                    pairs.push(("Last-Modified", lm));
                }
                build_response(
                    StatusCode::OK,
                    pairs,
                    axum::body::Body::from_stream(stream),
                )
            }
            Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        },
        Ok(Ok((None, _, None))) => {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "No page data")
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("out of range") {
                error_response(StatusCode::NOT_FOUND, &msg)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, &msg)
            }
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Internal error: {}", e),
        ),
    }
}

pub async fn get_page_thumb(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
    headers: HeaderMap,
) -> Response {
    if page_index < 0 {
        return error_response(StatusCode::BAD_REQUEST, "Page index must be non-negative");
    }

    let (archive_path, archive_type, thumb_dir, thumb_already_set) =
        match super::run_db(&state, move |db| db.get_archive(id)).await {
            Ok(Some(a)) => {
                let dir = state.data_dir.join("thumbnails").join(id.to_string());
                (
                    a.path,
                    a.archive_type,
                    dir,
                    a.thumbnail_path.is_some(),
                )
            }
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        };

    let cache_path = thumb_dir.join(format!("{}.jpg", page_index));

    // 先检查缓存，命中则直接返回
    if cache_path.exists() {
        let file_mtime = std::fs::metadata(&cache_path)
            .and_then(|m| m.modified())
            .ok();
        if let (Some(ims), Some(fmt)) = (
            headers
                .get("if-modified-since")
                .and_then(|v| v.to_str().ok()),
            file_mtime,
        ) {
            if let Some(parsed) = parse_http_date(ims) {
                if fmt <= parsed {
                    let mut pairs: Vec<(&'static str, String)> =
                        vec![("Cache-Control", CACHE_CONTROL.to_string())];
                    if let Some(lm) = http_date(fmt) {
                        pairs.push(("Last-Modified", lm));
                    }
                    return build_response(StatusCode::NOT_MODIFIED, pairs, "");
                }
            }
        }
        match std::fs::read(&cache_path) {
            Ok(data) => {
                let mut pairs: Vec<(&'static str, String)> = vec![
                    ("Content-Type", "image/jpeg".to_string()),
                    ("Cache-Control", CACHE_CONTROL.to_string()),
                ];
                if let Some(lm) = file_mtime.and_then(http_date) {
                    pairs.push(("Last-Modified", lm));
                }
                return build_response(StatusCode::OK, pairs, data);
            }
            Err(e) => {
                tracing::warn!("Failed to read thumbnail cache: {}", e);
            }
        }
    }

    // 缓存未命中，打开压缩包生成缩略图
    let thumb_dir_clone = thumb_dir.clone();
    let mtime_secs = archive_mtime_secs(&archive_path);
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let pages = load_page_rows(&db, id, &archive_path, &archive_type, mtime_secs)?;
        let idx = page_index as usize;
        if idx >= pages.len() {
            anyhow::bail!("Page index {} out of range (total: {})", idx, pages.len());
        }
        let page_name = &pages[idx].filepath;
        let reader = crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
        let data = reader.extract_page(page_name)?;
        let thumb_gen = crate::services::thumbnail::ThumbnailGenerator::default();
        // generate_with_cache 使用 thumb_dir 作为缓存目录
        thumb_gen.generate_with_cache(&data, &thumb_dir_clone, &page_index.to_string())
    })
    .await;

    match result {
        Ok(Ok(thumb_data)) => {
            // 首次生成成功，更新数据库记录；LRU 淘汰最多每分钟跑一次
            let thumb_dir_str = thumb_dir.to_string_lossy().to_string();
            let mut do_evict = false;
            {
                let mut last = state.last_thumb_eviction.lock().unwrap();
                let elapsed = last
                    .as_ref()
                    .map(|t| t.elapsed().as_secs() >= 60)
                    .unwrap_or(true);
                if elapsed {
                    *last = Some(std::time::Instant::now());
                    do_evict = true;
                }
            }

            let evicted = if do_evict {
                super::run_db(&state, move |db| {
                    db.set_thumbnail_path(id, &thumb_dir_str)?;
                    db.evict_old_thumbnails()
                })
                .await
                .unwrap_or_default()
            } else {
                if !thumb_already_set {
                    let _ = super::run_db(&state, move |db| {
                        db.set_thumbnail_path(id, &thumb_dir_str)
                    })
                    .await;
                }
                vec![]
            };

            // 删除被淘汰漫画的缩略图目录
            for (_evicted_id, evicted_path) in evicted {
                let _ = tokio::fs::remove_dir_all(&evicted_path).await;
            }

            build_response(
                StatusCode::OK,
                vec![
                    ("Content-Type", "image/jpeg".to_string()),
                    ("Cache-Control", CACHE_CONTROL.to_string()),
                ],
                thumb_data,
            )
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("out of range") {
                error_response(StatusCode::NOT_FOUND, &msg)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, &msg)
            }
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Internal error: {}", e),
        ),
    }
}

pub async fn open_file(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenFileRequest>,
) -> Response {
    let file_path = payload.file_path.clone();

    // Check DB first (quick operation)
    let existing = super::run_db(&state, {
        let file_path = file_path.clone();
        move |db| db.get_archive_by_path(&file_path)
    })
    .await;
    if let Ok(Some(existing)) = existing {
        return Json(serde_json::json!({
            "id": existing.id,
            "message": "文件已存在于库中"
        }))
        .into_response();
    }

    // Detect archive type (fast string check)
    let scanner = crate::services::scanner::Scanner::new();
    let archive_type = scanner.detect_archive_type(&file_path);

    if archive_type == "unknown" {
        return error_response(StatusCode::BAD_REQUEST, "Unsupported file type");
    }

    // Read title depth setting (quick DB operation)
    let title_depth = super::run_db(&state, move |db| {
        Ok(db
            .get_setting("title_depth")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1))
    })
    .await
    .unwrap_or(1);

    // Do blocking I/O in spawn_blocking
    let archive_type_clone = archive_type.clone();
    let file_path_for_insert = file_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&file_path);
        if !path.exists() {
            anyhow::bail!("File not found");
        }

        let title = crate::services::scanner::derive_title(path, title_depth);

        let file_size = std::fs::metadata(&file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let page_count = match crate::services::archive::create_archive_reader(
            &file_path,
            &archive_type_clone,
        ) {
            Ok(reader) => reader.list_pages().map(|p| p.len() as i64).unwrap_or(0),
            Err(_) => 0,
        };

        Ok((title, file_size, page_count))
    })
    .await;

    match result {
        Ok(Ok((title, file_size, page_count))) => {
            if page_count == 0 {
                let msg = if archive_type == "folder" {
                    "文件夹中没有找到图片文件"
                } else {
                    "压缩包中没有图片"
                };
                return error_response(StatusCode::BAD_REQUEST, msg);
            }

            let result = super::run_db(&state, {
                let file_path = file_path_for_insert.clone();
                let title = title.clone();
                let archive_type = archive_type.clone();
                move |db| {
                    db.insert_archive(&title, &file_path, &archive_type, page_count, file_size)
                }
            })
            .await;

            match result {
                Ok(id) => Json(serde_json::json!({
                    "id": id,
                    "title": title,
                    "archive_type": archive_type,
                }))
                .into_response(),
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                error_response(StatusCode::NOT_FOUND, &msg)
            } else {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, &msg)
            }
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Internal error: {}", e),
        ),
    }
}

pub async fn scan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> Response {
    let (root_dir, depth) = match super::run_db(&state, move |db| {
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
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    // Scan directory and count pages in blocking thread
    let result = tokio::task::spawn_blocking(move || {
        let scanner = crate::services::scanner::Scanner::new();
        let archives = scanner.scan_directory(&root_dir, depth)?;

        let mut archive_infos = Vec::new();
        for archive_path in &archives {
            let archive_type = scanner.detect_archive_type(archive_path);
            let title = {
                let path = std::path::Path::new(archive_path);
                let relative = path.strip_prefix(&root_dir).unwrap_or(path);
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

            let file_size = std::fs::metadata(archive_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let page_count = match crate::services::archive::create_archive_reader(
                archive_path,
                &archive_type,
            ) {
                Ok(reader) => reader.list_pages().map(|p| p.len() as i64).unwrap_or(0),
                Err(_) => 0,
            };

            archive_infos.push((
                title,
                archive_path.clone(),
                archive_type,
                page_count,
                file_size,
            ));
        }

        Ok::<_, anyhow::Error>(archive_infos)
    })
    .await;

    match result {
        Ok(Ok(archive_infos)) => {
            let total = archive_infos.len();
            let infos = archive_infos.clone();
            let added = super::run_db(&state, move |db| db.insert_archives_many(&infos))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to insert scanned archives: {}", e);
                    (0, total)
                });

            let (added, errors) = added;
            Json(serde_json::json!({
                "scanned": total,
                "added": added,
                "errors": errors,
                "message": format!("扫描完成：{} 个档案，{} 个新增，{} 个错误", total, added, errors)
            })).into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Scan task error: {}", e),
        ),
    }
}

/// 将文件夹打包为 CBZ 归档文件
pub async fn pack_cbz(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PackCbzRequest>,
) -> Response {
    let folder_path = payload.folder_path.clone();

    // 确定输出目录：优先使用请求参数，否则从设置中读取
    let output_dir = match super::run_db(&state, move |db| {
        if let Some(ref dir) = payload.output_dir {
            if !dir.is_empty() {
                return Ok::<Option<String>, rusqlite::Error>(Some(dir.clone()));
            }
        }
        match db.get_setting("cbz_export_dir") {
            Ok(dir) if !dir.is_empty() => Ok(Some(dir)),
            _ => Ok(None),
        }
    })
    .await
    {
        Ok(Some(dir)) => dir,
        Ok(None) => return error_response(StatusCode::BAD_REQUEST, "请先在设置中配置 CBZ 归档目录"),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    // 在独立线程中执行 CPU/IO 密集型打包任务
    let result = tokio::task::spawn_blocking(move || {
        crate::services::cbz::pack_folder_to_cbz(&folder_path, &output_dir)
    })
    .await;

    match result {
        Ok(Ok(cbz_path)) => Json(serde_json::json!({
            "success": true,
            "cbz_path": cbz_path,
            "message": format!("归档成功: {}", cbz_path),
        }))
        .into_response(),
        Ok(Err(e)) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("打包任务异常: {}", e),
        ),
    }
}

/// 列出 CBZ 导出目录中的所有 .cbz 文件
pub async fn list_cbz_files(State(state): State<Arc<AppState>>) -> Response {
    let export_dir = match super::run_db(&state, move |db| db.get_setting("cbz_export_dir")).await {
        Ok(dir) if !dir.is_empty() => dir,
        _ => return Json(serde_json::json!([])).into_response(),
    };

    let dir = std::path::Path::new(&export_dir);
    if !dir.exists() || !dir.is_dir() {
        return Json(serde_json::json!([])).into_response();
    }

    let mut files: Vec<serde_json::Value> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && e.path()
                    .extension()
                    .map(|ext| ext == "cbz")
                    .unwrap_or(false)
        })
        .filter_map(|e| {
            let metadata = e.metadata().ok()?;
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Some(serde_json::json!({
                "name": e.file_name().to_string_lossy(),
                "path": e.path().to_string_lossy(),
                "size": metadata.len(),
                "modified": mtime,
            }))
        })
        .collect();

    files.sort_by(|a, b| {
        let ma = a["modified"].as_u64().unwrap_or(0);
        let mb = b["modified"].as_u64().unwrap_or(0);
        mb.cmp(&ma)
    });

    Json(files).into_response()
}
