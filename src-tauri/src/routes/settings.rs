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
    
    match db.get_settings() {
        Ok(settings) => Json(serde_json::json!({ "data": settings })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateSettings>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.update_settings(&payload.settings) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.get_setting("root_dir") {
        Ok(root_dir) => Json(serde_json::json!({ "root_dir": root_dir })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateConfig>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    let mut settings = std::collections::HashMap::new();
    settings.insert("root_dir".to_string(), payload.root_dir);
    
    match db.update_settings(&settings) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.get_stats() {
        Ok(stats) => Json(serde_json::json!({ "data": stats })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn export_backup(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.export_backup() {
        Ok(backup) => Json(backup),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub async fn import_backup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    
    match db.import_backup(&payload) {
        Ok(_) => Json(serde_json::json!({ "success": true })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}
