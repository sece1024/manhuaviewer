use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateCategory {
    pub name: String,
    pub color: Option<String>,
    pub pinned: Option<bool>,
    pub search: Option<String>,
}

pub async fn list_categories(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.list_categories() {
        Ok(categories) => Json(serde_json::json!({ "data": categories })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn create_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateCategory>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let pinned = payload.pinned.unwrap_or(false);
    let search = payload.search.unwrap_or_default();
    
    match db.create_category(&payload.name, &color, pinned, &search) {
        Ok(id) => Json(serde_json::json!({
            "data": {
                "id": id,
                "name": payload.name,
                "color": color,
                "pinned": pinned,
                "search": search
            }
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn update_category(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<CreateCategory>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let pinned = payload.pinned.unwrap_or(false);
    let search = payload.search.unwrap_or_default();
    
    // Delete and recreate (simple approach)
    match db.delete_category(id) {
        Ok(_) => {
            match db.create_category(&payload.name, &color, pinned, &search) {
                Ok(new_id) => Json(serde_json::json!({
                    "data": {
                        "id": new_id,
                        "name": payload.name,
                        "color": color,
                        "pinned": pinned,
                        "search": search
                    }
                })),
                Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
            }
        },
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn delete_category(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.delete_category(id) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn assign_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let archive_id = payload["archive_id"].as_i64().unwrap_or(0);
    let category_id = payload["category_id"].as_i64().unwrap_or(0);
    
    match db.assign_category(archive_id, category_id) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn remove_category(
    State(state): State<Arc<AppState>>,
    Path((archive_id, category_id)): Path<(i64, i64)>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.remove_category(archive_id, category_id) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}
