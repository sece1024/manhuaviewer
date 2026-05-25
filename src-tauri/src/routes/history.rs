use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;
use crate::models::History;

#[derive(Deserialize)]
pub struct SaveHistory {
    pub archive_id: i64,
    pub page_index: i64,
    pub total_pages: i64,
}

pub async fn get_history(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let mut stmt = conn.prepare(
        "SELECT h.*, a.title, a.path, a.archive_type, a.page_count, a.cover_image
         FROM history h
         JOIN archives a ON a.id = h.archive_id
         ORDER BY h.updated_at DESC"
    ).unwrap();
    
    let history: Vec<serde_json::Value> = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "archive_id": row.get::<_, i64>(0)?,
            "page_index": row.get::<_, i64>(1)?,
            "total_pages": row.get::<_, i64>(2)?,
            "updated_at": row.get::<_, String>(3)?,
            "title": row.get::<_, String>(4)?,
            "path": row.get::<_, String>(5)?,
            "archive_type": row.get::<_, String>(6)?,
            "page_count": row.get::<_, i64>(7)?,
            "cover_image": row.get::<_, Option<String>>(8)?,
        }))
    }).unwrap().filter_map(|r| r.ok()).collect();
    
    Json(serde_json::json!({ "data": history }))
}

pub async fn save_history(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveHistory>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute(
        "INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at) VALUES (?, ?, ?, datetime('now'))",
        (payload.archive_id, payload.page_index, payload.total_pages),
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn delete_history(
    State(state): State<Arc<AppState>>,
    Path(archive_id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute("DELETE FROM history WHERE archive_id = ?", [archive_id]) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn clear_history(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute("DELETE FROM history", []) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}
