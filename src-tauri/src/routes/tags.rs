use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;

#[derive(Deserialize)]
pub struct TagQuery {
    pub namespace: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Deserialize)]
pub struct AssignTagRequest {
    pub archive_id: i64,
    pub tag_id: i64,
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

pub async fn list_tags(
    State(state): State<Arc<AppState>>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.list_tags() {
        Ok(tags) => Json(serde_json::json!({ "data": tags })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn list_namespaces(
    State(state): State<Arc<AppState>>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.list_namespaces() {
        Ok(namespaces) => Json(serde_json::json!({ "data": namespaces })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TagQuery>,
) -> Response {
    let db = state.db.lock().await;
    let namespace = payload.namespace.unwrap_or_default();
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    
    match db.create_tag(&namespace, &payload.name, &color) {
        Ok(id) => Json(serde_json::json!({
            "data": {
                "id": id,
                "namespace": namespace,
                "name": payload.name,
                "color": color
            }
        })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<TagQuery>,
) -> Response {
    let db = state.db.lock().await;
    let namespace = payload.namespace.unwrap_or_default();
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    
    // Delete and recreate (simple approach)
    match db.delete_tag(id) {
        Ok(_) => {
            match db.create_tag(&namespace, &payload.name, &color) {
                Ok(new_id) => Json(serde_json::json!({
                    "data": {
                        "id": new_id,
                        "namespace": namespace,
                        "name": payload.name,
                        "color": color
                    }
                })).into_response(),
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        },
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.delete_tag(id) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn assign_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AssignTagRequest>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.assign_tag(payload.archive_id, payload.tag_id) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn remove_tag(
    State(state): State<Arc<AppState>>,
    Path((archive_id, tag_id)): Path<(i64, i64)>,
) -> Response {
    let db = state.db.lock().await;
    
    match db.remove_tag(archive_id, tag_id) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
