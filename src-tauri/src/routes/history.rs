use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;

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
    
    match db.get_history() {
        Ok(history) => {
            let data: Vec<serde_json::Value> = history.into_iter().map(|(h, title, path)| {
                serde_json::json!({
                    "archive_id": h.archive_id,
                    "page_index": h.page_index,
                    "total_pages": h.total_pages,
                    "updated_at": h.updated_at,
                    "title": title,
                    "path": path,
                })
            }).collect();
            Json(serde_json::json!({ "data": data }))
        },
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn save_history(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveHistory>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.save_history(payload.archive_id, payload.page_index, payload.total_pages) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn delete_history(
    State(state): State<Arc<AppState>>,
    Path(archive_id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.delete_history(archive_id) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn clear_history(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.clear_history() {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}
