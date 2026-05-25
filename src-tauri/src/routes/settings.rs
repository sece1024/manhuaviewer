use axum::{
    extract::State,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;

#[derive(Deserialize)]
pub struct UpdateSettings {
    pub settings: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct UpdateConfig {
    pub root_dir: String,
}

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let mut stmt = conn.prepare("SELECT key, value FROM settings").unwrap();
    let settings: std::collections::HashMap<String, String> = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }).unwrap().filter_map(|r| r.ok()).collect();
    
    Json(serde_json::json!({ "data": settings }))
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateSettings>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    for (key, value) in &payload.settings {
        if let Err(e) = conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
            (key, value),
        ) {
            return Json(serde_json::json!({ "error": e.to_string() }));
        }
    }
    
    Json(serde_json::json!({ "success": true }))
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let root_dir: String = conn.query_row(
        "SELECT value FROM settings WHERE key = 'root_dir'",
        [],
        |row| row.get(0)
    ).unwrap_or_default();
    
    Json(serde_json::json!({ "root_dir": root_dir }))
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateConfig>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    match conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('root_dir', ?)",
        [&payload.root_dir],
    ) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let total_archives: i64 = conn.query_row(
        "SELECT COUNT(*) FROM archives",
        [],
        |row| row.get(0)
    ).unwrap_or(0);
    
    let total_pages: i64 = conn.query_row(
        "SELECT COALESCE(SUM(page_count), 0) FROM archives",
        [],
        |row| row.get(0)
    ).unwrap_or(0);
    
    let total_tags: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tags",
        [],
        |row| row.get(0)
    ).unwrap_or(0);
    
    let total_categories: i64 = conn.query_row(
        "SELECT COUNT(*) FROM categories",
        [],
        |row| row.get(0)
    ).unwrap_or(0);
    
    let history_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM history",
        [],
        |row| row.get(0)
    ).unwrap_or(0);
    
    Json(serde_json::json!({
        "data": {
            "total_archives": total_archives,
            "total_pages": total_pages,
            "total_tags": total_tags,
            "total_categories": total_categories,
            "history_count": history_count
        }
    }))
}

pub async fn export_backup(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    // Export all data
    let mut archives: Vec<serde_json::Value> = Vec::new();
    let mut stmt = conn.prepare("SELECT * FROM archives").unwrap();
    let _ = stmt.query_map([], |row| {
        archives.push(serde_json::json!({
            "title": row.get::<_, String>(1)?,
            "path": row.get::<_, String>(2)?,
            "archive_type": row.get::<_, String>(3)?,
            "page_count": row.get::<_, i64>(4)?,
        }));
        Ok(())
    });
    
    // TODO: Export other tables
    
    Json(serde_json::json!({
        "version": "3.0.0",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "archives": archives,
    }))
}

pub async fn import_backup(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // TODO: Implement backup import
    Json(serde_json::json!({ "error": "Not implemented" }))
}
