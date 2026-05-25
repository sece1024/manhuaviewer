use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;

#[derive(Deserialize)]
pub struct ArchiveQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

pub async fn list_archives(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArchiveQuery>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    let sort = query.sort.as_deref().unwrap_or("updated");
    let order = query.order.as_deref().unwrap_or("desc");
    
    match db.list_archives(query.search.as_deref(), sort, order, limit, offset) {
        Ok((archives, total)) => Json(serde_json::json!({
            "data": archives,
            "total": total,
            "page": page,
            "limit": limit
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_archive(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => Json(serde_json::json!({ "data": archive })),
        Ok(None) => Json(serde_json::json!({ "error": "Archive not found" })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn delete_archive(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.delete_archive(id) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_cover(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => {
            match crate::services::archive::create_archive_reader(&archive.path, &archive.archive_type) {
                Ok(reader) => {
                    match reader.get_cover() {
                        Ok(cover_data) => {
                            Json(serde_json::json!({
                                "data": {
                                    "mime": "image/jpeg",
                                    "base64": base64::encode(&cover_data)
                                }
                            }))
                        },
                        Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
                    }
                },
                Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
            }
        },
        Ok(None) => Json(serde_json::json!({ "error": "Archive not found" })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn list_pages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => {
            match crate::services::archive::create_archive_reader(&archive.path, &archive.archive_type) {
                Ok(reader) => {
                    match reader.list_pages() {
                        Ok(pages) => {
                            let page_list: Vec<serde_json::Value> = pages.iter().enumerate().map(|(i, p)| {
                                serde_json::json!({
                                    "id": i,
                                    "archive_id": id,
                                    "filename": std::path::Path::new(p).file_name().unwrap_or_default().to_string_lossy(),
                                    "filepath": p,
                                    "sort_order": i,
                                })
                            }).collect();
                            Json(serde_json::json!({ "data": page_list }))
                        },
                        Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
                    }
                },
                Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
            }
        },
        Ok(None) => Json(serde_json::json!({ "error": "Archive not found" })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_page(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
) -> Json<serde_json::Value> {
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
                                
                                Json(serde_json::json!({
                                    "data": {
                                        "index": page_index,
                                        "filename": page_name,
                                        "mime": mime,
                                        "base64": base64::encode(&data)
                                    }
                                }))
                            },
                            Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
                        }
                    } else {
                        Json(serde_json::json!({ "error": "Page index out of range" }))
                    }
                },
                Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
            }
        },
        Ok(None) => Json(serde_json::json!({ "error": "Archive not found" })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_page_thumb(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
) -> Json<serde_json::Value> {
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
                                        Json(serde_json::json!({
                                            "data": {
                                                "index": page_index,
                                                "mime": "image/jpeg",
                                                "base64": base64::encode(&thumb_data)
                                            }
                                        }))
                                    },
                                    Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
                                }
                            },
                            Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
                        }
                    } else {
                        Json(serde_json::json!({ "error": "Page index out of range" }))
                    }
                },
                Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
            }
        },
        Ok(None) => Json(serde_json::json!({ "error": "Archive not found" })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
pub struct OpenFileRequest {
    pub file_path: String,
}

pub async fn open_file(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenFileRequest>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    let path = std::path::Path::new(&payload.file_path);
    
    if !path.exists() {
        return Json(serde_json::json!({ "error": "File not found" }));
    }
    
    let scanner = crate::services::scanner::Scanner::new();
    let archive_type = scanner.detect_archive_type(&payload.file_path);
    
    if archive_type == "unknown" {
        return Json(serde_json::json!({ "error": "Unsupported file type" }));
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
    
    match db.insert_archive(&title, &payload.file_path, &archive_type, page_count, file_size) {
        Ok(id) => Json(serde_json::json!({
            "data": {
                "id": id,
                "title": title,
                "path": payload.file_path,
                "archive_type": archive_type,
                "page_count": page_count,
                "file_size": file_size
            }
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
pub struct ScanRequest {
    pub path: Option<String>,
    pub depth: Option<u32>,
}

pub async fn scan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    let root_dir = if let Some(p) = payload.path {
        p
    } else {
        db.get_setting("root_dir").unwrap_or_default()
    };
    
    if root_dir.is_empty() {
        return Json(serde_json::json!({ "error": "No root directory configured" }));
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
                "data": {
                    "scanned": archives.len(),
                    "added": added,
                    "errors": errors
                }
            }))
        },
        Err(e) => Json(serde_json::json!({ "error": e.to_string() }))
    }
}
