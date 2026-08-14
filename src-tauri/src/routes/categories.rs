use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::{error_response, run_db};

#[derive(Deserialize)]
pub struct CreateCategory {
    pub name: String,
    pub color: Option<String>,
    pub pinned: Option<bool>,
    pub search: Option<String>,
}

pub async fn list_categories(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.list_categories()).await {
        Ok(categories) => Json(categories).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn create_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateCategory>,
) -> Response {
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let pinned = payload.pinned.unwrap_or(false);
    let search = payload.search.unwrap_or_default();
    let name = payload.name.clone();
    let color_db = color.clone();
    let search_db = search.clone();

    match run_db(&state, move |db| {
        db.create_category(&name, &color_db, pinned, &search_db)
    })
    .await
    {
        Ok(id) => Json(serde_json::json!({
            "data": {
                "id": id,
                "name": payload.name,
                "color": color,
                "pinned": pinned,
                "search": search
            }
        }))
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn update_category(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<CreateCategory>,
) -> Response {
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let pinned = payload.pinned.unwrap_or(false);
    let search = payload.search.unwrap_or_default();
    let name = payload.name.clone();
    let color_db = color.clone();
    let search_db = search.clone();

    match run_db(&state, move |db| {
        db.update_category(id, &name, &color_db, pinned, &search_db)
    })
    .await
    {
        Ok(_) => Json(serde_json::json!({
            "data": {
                "id": id,
                "name": payload.name,
                "color": color,
                "pinned": pinned,
                "search": search
            }
        }))
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn delete_category(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    match run_db(&state, move |db| db.delete_category(id)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn assign_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    let archive_id = match payload["archive_id"].as_i64() {
        Some(id) if id > 0 => id,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid archive_id"),
    };
    let category_id = match payload["category_id"].as_i64() {
        Some(id) if id > 0 => id,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid category_id"),
    };

    match run_db(&state, move |db| {
        db.assign_category(archive_id, category_id)
    })
    .await
    {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn remove_category(
    State(state): State<Arc<AppState>>,
    Path((archive_id, category_id)): Path<(i64, i64)>,
) -> Response {
    match run_db(&state, move |db| {
        db.remove_category(archive_id, category_id)
    })
    .await
    {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn get_archive_categories(
    State(state): State<Arc<AppState>>,
    Path(archive_id): Path<i64>,
) -> Response {
    match run_db(&state, move |db| db.get_archive_categories(archive_id)).await {
        Ok(categories) => Json(categories).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct BatchAssignCategoryRequest {
    pub archive_ids: Vec<i64>,
    pub category_id: i64,
}

pub async fn batch_assign_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BatchAssignCategoryRequest>,
) -> Response {
    if payload.archive_ids.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "archive_ids 不能为空");
    }

    match run_db(&state, move |db| {
        db.batch_assign_category(&payload.archive_ids, payload.category_id)
    })
    .await
    {
        Ok(affected) => {
            Json(serde_json::json!({ "success": true, "affected": affected })).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn batch_remove_category(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BatchAssignCategoryRequest>,
) -> Response {
    if payload.archive_ids.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "archive_ids 不能为空");
    }

    match run_db(&state, move |db| {
        db.batch_remove_category(&payload.archive_ids, payload.category_id)
    })
    .await
    {
        Ok(affected) => {
            Json(serde_json::json!({ "success": true, "affected": affected })).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
