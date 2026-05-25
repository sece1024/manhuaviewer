use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;
use crate::models::Archive;

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
    let conn = db.get_conn();
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    
    // Simple implementation - will be enhanced later
    let mut sql = String::from(
        "SELECT * FROM archives WHERE 1=1"
    );
    let mut params: Vec<String> = Vec::new();
    
    if let Some(search) = &query.search {
        if !search.is_empty() {
            sql.push_str(" AND title LIKE ?");
            params.push(format!("%{}%", search));
        }
    }
    
    sql.push_str(" ORDER BY ");
    match query.sort.as_deref() {
        Some("name") => sql.push_str("title"),
        Some("created") => sql.push_str("created_at"),
        Some("pages") => sql.push_str("page_count"),
        Some("size") => sql.push_str("file_size"),
        _ => sql.push_str("updated_at"),
    }
    
    sql.push_str(match query.order.as_deref() {
        Some("asc") => " ASC",
        _ => " DESC",
    });
    
    sql.push_str(" LIMIT ? OFFSET ?");
    
    let mut stmt = conn.prepare(&sql).unwrap();
    let archives: Vec<Archive> = stmt.query_map(
        rusqlite::params_from_iter(params.iter().map(|s| s.as_str()).chain([limit.to_string().as_str(), offset.to_string().as_str()])),
        |row| {
            Ok(Archive {
                id: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                archive_type: row.get(3)?,
                page_count: row.get(4)?,
                cover_image: row.get(5)?,
                file_size: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        }
    ).unwrap().filter_map(|r| r.ok()).collect();
    
    // Get total count
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM archives",
        [],
        |row| row.get(0)
    ).unwrap_or(0);
    
    Json(serde_json::json!({
        "data": archives,
        "total": total,
        "page": page,
        "limit": limit
    }))
}

pub async fn get_archive(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let archive: Option<Archive> = conn.query_row(
        "SELECT * FROM archives WHERE id = ?",
        [id],
        |row| {
            Ok(Archive {
                id: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                archive_type: row.get(3)?,
                page_count: row.get(4)?,
                cover_image: row.get(5)?,
                file_size: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        }
    ).ok();
    
    match archive {
        Some(a) => Json(serde_json::json!({ "data": a })),
        None => Json(serde_json::json!({ "error": "Archive not found" })),
    }
}

pub async fn delete_archive(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute("DELETE FROM archives WHERE id = ?", [id]) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_cover(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    // TODO: Implement cover image serving
    Json(serde_json::json!({ "error": "Not implemented" }))
}

pub async fn list_pages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let mut stmt = conn.prepare(
        "SELECT * FROM pages WHERE archive_id = ? ORDER BY sort_order"
    ).unwrap();
    
    let pages: Vec<crate::models::Page> = stmt.query_map([id], |row| {
        Ok(crate::models::Page {
            id: row.get(0)?,
            archive_id: row.get(1)?,
            filename: row.get(2)?,
            filepath: row.get(3)?,
            sort_order: row.get(4)?,
            width: row.get(5)?,
            height: row.get(6)?,
            file_size: row.get(7)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect();
    
    Json(serde_json::json!({ "data": pages }))
}

pub async fn get_page(
    State(_state): State<Arc<AppState>>,
    Path((_id, _page)): Path<(i64, i64)>,
) -> Json<serde_json::Value> {
    // TODO: Implement page serving
    Json(serde_json::json!({ "error": "Not implemented" }))
}

pub async fn get_page_thumb(
    State(_state): State<Arc<AppState>>,
    Path((_id, _page)): Path<(i64, i64)>,
) -> Json<serde_json::Value> {
    // TODO: Implement thumbnail generation
    Json(serde_json::json!({ "error": "Not implemented" }))
}

pub async fn open_file(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // TODO: Implement file opening
    Json(serde_json::json!({ "error": "Not implemented" }))
}

pub async fn scan(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: Implement scanning
    Json(serde_json::json!({ "error": "Not implemented" }))
}
