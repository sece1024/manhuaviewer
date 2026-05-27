use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::error_response;

#[derive(Deserialize)]
pub struct SaveHistory {
    pub archive_id: i64,
    pub page_index: i64,
    pub total_pages: i64,
}

pub async fn get_history(State(state): State<Arc<AppState>>) -> Response {
    let db = state.db.lock().await;

    match db.get_history() {
        Ok(history) => {
            // Batch fetch tags for all archives in one query
            let archive_ids: Vec<i64> = history.iter().map(|(h, _, _, _)| h.archive_id).collect();
            let tags_map = db.get_archive_tags_batch(&archive_ids).unwrap_or_default();

            let data: Vec<serde_json::Value> = history
                .into_iter()
                .map(|(h, title, path, archive_type)| {
                    let tags = tags_map
                        .get(&h.archive_id)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    let tags_json: Vec<serde_json::Value> = tags
                        .iter()
                        .map(|t| {
                            serde_json::json!({
                                "id": t.id,
                                "namespace": t.namespace,
                                "name": t.name,
                                "color": t.color,
                            })
                        })
                        .collect();

                    serde_json::json!({
                        "archive_id": h.archive_id,
                        "page_index": h.page_index,
                        "total_pages": h.total_pages,
                        "updated_at": h.updated_at,
                        "title": title,
                        "path": path,
                        "archive_type": archive_type,
                        "tags": tags_json,
                        "cover_url": format!("/api/archives/{}/cover", h.archive_id),
                    })
                })
                .collect();
            Json(data).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn save_history(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveHistory>,
) -> Response {
    let db = state.db.lock().await;

    match db.save_history(payload.archive_id, payload.page_index, payload.total_pages) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete_history(
    State(state): State<Arc<AppState>>,
    Path(archive_id): Path<i64>,
) -> Response {
    let db = state.db.lock().await;

    match db.delete_history(archive_id) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn clear_history(State(state): State<Arc<AppState>>) -> Response {
    let db = state.db.lock().await;

    match db.clear_history() {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
