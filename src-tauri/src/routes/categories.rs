use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;
use crate::models::Category;

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
    let conn = db.get_conn();
    
    let mut stmt = conn.prepare("SELECT * FROM categories ORDER BY name").unwrap();
    let categories: Vec<Category> = stmt.query_map([], |row| {
        Ok(Category {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            pinned: row.get::<_, i64>(3)? != 0,
            search: row.get(4)?,
            created_at: row.get(5)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect();
    
    Json(serde_json::json!({ "data": categories }))
}

pub async fn create_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateCategory>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let pinned = payload.pinned.unwrap_or(false) as i64;
    let search = payload.search.unwrap_or_default();
    
    match conn.execute(
        "INSERT INTO categories (name, color, pinned, search) VALUES (?, ?, ?, ?)",
        (&payload.name, &color, pinned, &search),
    ) {
        Ok(_) => {
            let id = conn.last_insert_rowid();
            Json(serde_json::json!({ "data": { "id": id, "name": payload.name, "color": color } }))
        },
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn update_category(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<CreateCategory>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let pinned = payload.pinned.unwrap_or(false) as i64;
    let search = payload.search.unwrap_or_default();
    
    match conn.execute(
        "UPDATE categories SET name = ?, color = ?, pinned = ?, search = ? WHERE id = ?",
        (&payload.name, &color, pinned, &search, id),
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn delete_category(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute("DELETE FROM categories WHERE id = ?", [id]) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn assign_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let archive_id = payload["archive_id"].as_i64().unwrap_or(0);
    let category_id = payload["category_id"].as_i64().unwrap_or(0);
    
    match conn.execute(
        "INSERT OR IGNORE INTO archive_categories (archive_id, category_id) VALUES (?, ?)",
        (archive_id, category_id),
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn remove_category(
    State(state): State<Arc<AppState>>,
    Path((archive_id, category_id)): Path<(i64, i64)>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute(
        "DELETE FROM archive_categories WHERE archive_id = ? AND category_id = ?",
        (archive_id, category_id),
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}
