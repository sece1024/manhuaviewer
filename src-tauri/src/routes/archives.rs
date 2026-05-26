use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;

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

pub async fn get_archive(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => Json(serde_json::json!({ "data": archive })).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete_archive(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.delete_archive(id) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_cover(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => {
            match crate::services::archive::create_archive_reader(&archive.path, &archive.archive_type) {
                Ok(reader) => {
                    match reader.get_cover() {
                        Ok(cover_data) => {
                            (
                                StatusCode::OK,
                                [("Content-Type", "image/jpeg")],
                                cover_data,
                            ).into_response()
                        },
                        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
                    }
                },
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
            }
        },
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn list_pages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => {
            match crate::services::archive::create_archive_reader(&archive.path, &archive.archive_type) {
                Ok(reader) => {
                    match reader.list_pages() {
                        Ok(pages) => {
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
                                "archive": archive,
                                "pages": page_list,
                                "read_page": null
                            })).into_response()
                        },
                        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
                    }
                },
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
            }
        },
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_page(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => {
            match crate::services::archive::create_archive_reader(&archive.path, &archive.archive_type) {
                Ok(reader) => {
                    let pages = reader.list_pages().unwrap_or_default();
                    if (page_index as usize) < pages.len() {
                        let page_name = &pages[page_index as usize];
                        match reader.extract_page(page_name) {
                            Ok(data) => {
                                let mime = mime_guess::from_path(page_name)
                                    .first_or_octet_stream()
                                    .to_string();
                                
                                (
                                    StatusCode::OK,
                                    [("Content-Type", mime)],
                                    data,
                                ).into_response()
                            },
                            Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
                        }
                    } else {
                        error_response(StatusCode::NOT_FOUND, "Page index out of range")
                    }
                },
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
            }
        },
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_page_thumb(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => {
            match crate::services::archive::create_archive_reader(&archive.path, &archive.archive_type) {
                Ok(reader) => {
                    let pages = reader.list_pages().unwrap_or_default();
                    if (page_index as usize) < pages.len() {
                        let page_name = &pages[page_index as usize];
                        match reader.extract_page(page_name) {
                            Ok(data) => {
                                let thumb_gen = crate::services::thumbnail::ThumbnailGenerator::default();
                                match thumb_gen.generate(&data) {
                                    Ok(thumb_data) => {
                                        (
                                            StatusCode::OK,
                                            [("Content-Type", "image/jpeg")],
                                            thumb_data,
                                        ).into_response()
                                    },
                                    Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
                                }
                            },
                            Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
                        }
                    } else {
                        error_response(StatusCode::NOT_FOUND, "Page index out of range")
                    }
                },
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
            }
        },
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn open_file(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenFileRequest>,
) -> Response {
    let db = state.db.lock().await;
    
    let path = std::path::Path::new(&payload.file_path);
    
    if !path.exists() {
        return error_response(StatusCode::NOT_FOUND, "File not found");
    }
    
    // Check if archive already exists in database
    if let Ok(Some(existing)) = db.get_archive_by_path(&payload.file_path) {
        return Json(serde_json::json!({
            "id": existing.id,
            "message": "文件已存在于库中"
        })).into_response();
    }
    
    let scanner = crate::services::scanner::Scanner::new();
    let archive_type = scanner.detect_archive_type(&payload.file_path);
    
    if archive_type == "unknown" {
        return error_response(StatusCode::BAD_REQUEST, "Unsupported file type");
    }
    
    let title = path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    
    let file_size = std::fs::metadata(&payload.file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    
    let page_count = match crate::services::archive::create_archive_reader(&payload.file_path, &archive_type) {
        Ok(reader) => reader.list_pages().map(|p| p.len() as i64).unwrap_or(0),
        Err(_) => 0,
    };

    if page_count == 0 {
        let msg = if archive_type == "folder" {
            "文件夹中没有找到图片文件"
        } else {
            "压缩包中没有图片"
        };
        return error_response(StatusCode::BAD_REQUEST, msg);
    }
    
    match db.insert_archive(&title, &payload.file_path, &archive_type, page_count, file_size) {
        Ok(id) => Json(serde_json::json!({
            "id": id,
            "title": title,
            "archive_type": archive_type,
        })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn scan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> Response {
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
    
    let scanner = crate::services::scanner::Scanner::new();
    match scanner.scan_directory(&root_dir, depth) {
        Ok(archives) => {
            let mut added = 0;
            let mut errors = 0;
            
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
                
                let page_count = match crate::services::archive::create_archive_reader(archive_path, &archive_type) {
                    Ok(reader) => reader.list_pages().map(|p| p.len() as i64).unwrap_or(0),
                    Err(_) => 0,
                };
                
                match db.insert_archive(&title, archive_path, &archive_type, page_count, file_size) {
                    Ok(_) => added += 1,
                    Err(_) => errors += 1,
                }
            }
            
            Json(serde_json::json!({
                "scanned": archives.len(),
                "added": added,
                "errors": errors,
                "message": format!("扫描完成：{} 个档案，{} 个新增，{} 个错误", archives.len(), added, errors)
            })).into_response()
        },
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
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
            _ => return error_response(
                StatusCode::BAD_REQUEST,
                "请先在设置中配置 CBZ 归档目录",
            ),
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
        })).into_response(),
        Ok(Err(e)) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("打包任务异常: {}", e),
        ),
    }
}
