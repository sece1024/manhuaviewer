use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ArchiveQuery {
    #[serde(alias = "sort_by")]
    pub sort: Option<String>,
    #[serde(alias = "sort_order")]
    pub order: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
    #[allow(dead_code)]
    pub tag: Option<String>,
    #[allow(dead_code)]
    pub category: Option<String>,
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

// Helper function to create error response with proper status code
fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

pub async fn list_archives(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArchiveQuery>,
) -> Response {
    let db = state.db.lock().await;

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    let sort = query.sort.as_deref().unwrap_or("updated");
    let order = query.order.as_deref().unwrap_or("desc");

    // TODO: Implement tag and category filtering
    match db.list_archives(query.search.as_deref(), sort, order, limit, offset) {
        Ok((archives, _total)) => Json(archives).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_archive(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let db = state.db.lock().await;

    match db.get_archive(id) {
        Ok(Some(archive)) => Json(serde_json::json!({ "data": archive })).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete_archive(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let db = state.db.lock().await;

    match db.delete_archive(id) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_cover(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let (archive_path, archive_type) = {
        let db = state.db.lock().await;
        match db.get_archive(id) {
            Ok(Some(a)) => (a.path, a.archive_type),
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        let reader = crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
        reader.get_cover()
    })
    .await;

    match result {
        Ok(Ok(cover_data)) => {
            (StatusCode::OK, [("Content-Type", "image/jpeg")], cover_data).into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Task error: {}", e),
        ),
    }
}

pub async fn list_pages(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let (archive_path, archive_type) = {
        let db = state.db.lock().await;
        match db.get_archive(id) {
            Ok(Some(a)) => (a.path, a.archive_type),
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        let reader = crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
        reader.list_pages()
    })
    .await;

    match result {
        Ok(Ok(pages)) => {
            let page_list: Vec<serde_json::Value> = pages.iter().enumerate().map(|(i, p)| {
                let filename = std::path::Path::new(p)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                serde_json::json!({
                    "id": i,
                    "archive_id": id,
                    "filename": filename,
                    "filepath": p,
                    "sort_order": i,
                    "url": format!("/api/archives/{}/pages/{}", id, i),
                    "thumb_url": format!("/api/archives/{}/pages/{}/thumb", id, i),
                })
            }).collect();

            Json(serde_json::json!({
                "pages": page_list,
            }))
            .into_response()
        }
        Ok(Err(e)) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Task error: {}", e),
        ),
    }
}

pub async fn get_page(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
) -> Response {
    if page_index < 0 {
        return error_response(StatusCode::BAD_REQUEST, "Page index must be non-negative");
    }

    let (archive_path, archive_type) = {
        let db = state.db.lock().await;
        match db.get_archive(id) {
            Ok(Some(a)) => (a.path, a.archive_type),
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        let reader = crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
        let pages = reader.list_pages()?;
        let idx = page_index as usize;
        if idx >= pages.len() {
            anyhow::bail!("Page index {} out of range (total: {})", idx, pages.len());
        }
        let page_name = pages[idx].clone();
        let data = reader.extract_page(&page_name)?;
        let mime = mime_guess::from_path(&page_name)
            .first_or_octet_stream()
            .to_string();
        Ok((data, mime))
    })
    .await;

    match result {
        Ok(Ok((data, mime))) => {
            (StatusCode::OK, [("Content-Type", mime.as_str())], data).into_response()
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
            &format!("Task error: {}", e),
        ),
    }
}

pub async fn get_page_thumb(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
) -> Response {
    if page_index < 0 {
        return error_response(StatusCode::BAD_REQUEST, "Page index must be non-negative");
    }

    let (archive_path, archive_type) = {
        let db = state.db.lock().await;
        match db.get_archive(id) {
            Ok(Some(a)) => (a.path, a.archive_type),
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        let reader = crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
        let pages = reader.list_pages()?;
        let idx = page_index as usize;
        if idx >= pages.len() {
            anyhow::bail!("Page index {} out of range (total: {})", idx, pages.len());
        }
        let page_name = &pages[idx];
        let data = reader.extract_page(page_name)?;
        let thumb_gen = crate::services::thumbnail::ThumbnailGenerator::default();
        thumb_gen.generate(&data)
    })
    .await;

    match result {
        Ok(Ok(thumb_data)) => {
            (StatusCode::OK, [("Content-Type", "image/jpeg")], thumb_data).into_response()
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
            &format!("Task error: {}", e),
        ),
    }
}

pub async fn open_file(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenFileRequest>,
) -> Response {
    let file_path = payload.file_path.clone();

    // Check DB first (quick operation)
    {
        let db = state.db.lock().await;
        if let Ok(Some(existing)) = db.get_archive_by_path(&file_path) {
            return Json(serde_json::json!({
                "id": existing.id,
                "message": "文件已存在于库中"
            }))
            .into_response();
        }
    }

    // Detect archive type (fast string check)
    let scanner = crate::services::scanner::Scanner::new();
    let archive_type = scanner.detect_archive_type(&file_path);

    if archive_type == "unknown" {
        return error_response(StatusCode::BAD_REQUEST, "Unsupported file type");
    }

    // Do blocking I/O in spawn_blocking
    let archive_type_clone = archive_type.clone();
    let result = tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&file_path);
        if !path.exists() {
            anyhow::bail!("File not found");
        }

        let title = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let file_size = std::fs::metadata(&file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let page_count =
            match crate::services::archive::create_archive_reader(&file_path, &archive_type_clone) {
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

            let db = state.db.lock().await;
            match db.insert_archive(&title, &payload.file_path, &archive_type, page_count, file_size) {
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
            &format!("Task error: {}", e),
        ),
    }
}

pub async fn scan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> Response {
    let (root_dir, depth) = {
        let db = state.db.lock().await;

        let root_dir = if let Some(p) = payload.path {
            p
        } else {
            db.get_setting("root_dir").unwrap_or_default()
        };

        if root_dir.is_empty() {
            return error_response(StatusCode::BAD_REQUEST, "No root directory configured");
        }

        let depth = payload.depth.unwrap_or_else(|| {
            db.get_setting("scan_depth")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
        });

        (root_dir, depth)
    };

    // Scan directory and count pages in blocking thread
    let result = tokio::task::spawn_blocking(move || {
        let scanner = crate::services::scanner::Scanner::new();
        let archives = scanner.scan_directory(&root_dir, depth)?;

        let mut archive_infos = Vec::new();
        for archive_path in &archives {
            let archive_type = scanner.detect_archive_type(archive_path);
            let title = std::path::Path::new(archive_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

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

            archive_infos.push((title, archive_path.clone(), archive_type, page_count, file_size));
        }

        Ok::<_, anyhow::Error>(archive_infos)
    })
    .await;

    match result {
        Ok(Ok(archive_infos)) => {
            let db = state.db.lock().await;
            let mut added = 0;
            let mut errors = 0;

            for (title, path, archive_type, page_count, file_size) in &archive_infos {
                match db.insert_archive(title, path, archive_type, *page_count, *file_size) {
                    Ok(_) => added += 1,
                    Err(_) => errors += 1,
                }
            }

            Json(serde_json::json!({
                "scanned": archive_infos.len(),
                "added": added,
                "errors": errors,
                "message": format!("扫描完成：{} 个档案，{} 个新增，{} 个错误", archive_infos.len(), added, errors)
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
    let db = state.db.lock().await;

    // 确定输出目录：优先使用请求参数，否则从设置中读取
    let output_dir = match payload.output_dir {
        Some(ref dir) if !dir.is_empty() => dir.clone(),
        _ => match db.get_setting("cbz_export_dir") {
            Ok(dir) if !dir.is_empty() => dir,
            _ => return error_response(StatusCode::BAD_REQUEST, "请先在设置中配置 CBZ 归档目录"),
        },
    };

    let folder_path = payload.folder_path.clone();

    // 释放 DB 锁，避免在打包期间长时间持有
    drop(db);

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
