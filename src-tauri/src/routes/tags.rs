use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;
use crate::models::Tag;

#[derive(Deserialize)]
pub struct CreateTag {
    pub namespace: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Deserialize)]
pub struct AssignTag {
    pub archive_id: i64,
    pub tag_id: i64,
}

pub async fn list_tags(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let mut stmt = conn.prepare("SELECT * FROM tags ORDER BY namespace, name").unwrap();
    let tags: Vec<Tag> = stmt.query_map([], |row| {
        Ok(Tag {
            id: row.get(0)?,
            namespace: row.get(1)?,
            name: row.get(2)?,
            color: row.get(3)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect();
    
    Json(serde_json::json!({ "data": tags }))
}

pub async fn list_namespaces(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let mut stmt = conn.prepare("SELECT DISTINCT namespace FROM tags WHERE namespace != '' ORDER BY namespace").unwrap();
    let namespaces: Vec<String> = stmt.query_map([], |row| row.get(0))
        .unwrap().filter_map(|r| r.ok()).collect();
    
    Json(serde_json::json!({ "data": namespaces }))
}

pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTag>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let namespace = payload.namespace.unwrap_or_default();
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    
    match conn.execute(
        "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)",
        (&namespace, &payload.name, &color),
    ) {
        Ok(_) => {
            let id = conn.last_insert_rowid();
            Json(serde_json::json!({ "data": { "id": id, "namespace": namespace, "name": payload.name, "color": color } }))
        },
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<CreateTag>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let namespace = payload.namespace.unwrap_or_default();
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    
    match conn.execute(
        "UPDATE tags SET namespace = ?, name = ?, color = ? WHERE id = ?",
        (&namespace, &payload.name, &color, id),
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute("DELETE FROM tags WHERE id = ?", [id]) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn assign_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AssignTag>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute(
        "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
        (payload.archive_id, payload.tag_id),
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn remove_tag(
    State(state): State<Arc<AppState>>,
    Path((archive_id, tag_id)): Path<(i64, i64)>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute(
        "DELETE FROM archive_tags WHERE archive_id = ? AND tag_id = ?",
        (archive_id, tag_id),
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}
