use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::{error_response, run_db};

#[derive(Deserialize)]
pub struct SaveHistory {
    pub archive_id: i64,
    pub page_index: i64,
    pub total_pages: i64,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
}

pub async fn get_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistoryQuery>,
) -> Response {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(30).clamp(1, 200);
    let offset = (page - 1) * limit;

    match run_db(&state, move |db| {
        let (history, total) = db.get_history(query.search.as_deref(), limit, offset)?;
        // Batch fetch tags for all archives in one query
        let archive_ids: Vec<i64> = history.iter().map(|(h, _, _, _)| h.archive_id).collect();
        let tags_map = db.get_archive_tags_batch(&archive_ids).unwrap_or_default();
        Ok((history, total, tags_map))
    })
    .await
    {
        Ok((history, total, tags_map)) => {
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
            Json(serde_json::json!({ "items": data, "total": total })).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn save_history(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveHistory>,
) -> Response {
    match run_db(&state, move |db| {
        db.save_history(payload.archive_id, payload.page_index, payload.total_pages)
    })
    .await
    {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete_history(
    State(state): State<Arc<AppState>>,
    Path(archive_id): Path<i64>,
) -> Response {
    match run_db(&state, move |db| db.delete_history(archive_id)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn clear_history(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.clear_history()).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
