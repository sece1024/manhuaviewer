use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::{error_response, run_db};

#[derive(Deserialize)]
pub struct UpdateSettings {
    // Accept flat key-value pairs directly
    #[serde(flatten)]
    pub settings: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct UpdateConfig {
    pub root_dir: String,
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.get_settings()).await {
        Ok(settings) => Json(settings).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateSettings>,
) -> Response {
    match run_db(&state, move |db| db.update_settings(&payload.settings)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.get_setting("root_dir")).await {
        Ok(root_dir) => Json(serde_json::json!({ "root_dir": root_dir })).into_response(),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Json(serde_json::json!({ "root_dir": "" })).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateConfig>,
) -> Response {
    let mut settings = std::collections::HashMap::new();
    settings.insert("root_dir".to_string(), payload.root_dir);

    match run_db(&state, move |db| db.update_settings(&settings)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_stats(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.get_stats()).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn export_backup(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.export_backup()).await {
        Ok(backup) => Json(backup).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn import_backup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    match run_db(&state, move |db| db.import_backup(&payload)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
