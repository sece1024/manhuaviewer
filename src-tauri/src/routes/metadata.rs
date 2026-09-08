use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
}

/// GET /api/metadata/search?q=… —— 从公开目录搜索漫画元数据（标题/封面/评分/标签）。
pub async fn search(Query(params): Query<SearchParams>) -> Response {
    let q = params.q.trim().to_string();
    if q.is_empty() {
        return super::error_response(StatusCode::BAD_REQUEST, "q 不能为空");
    }
    let result =
        tokio::task::spawn_blocking(move || crate::services::metadata::search_bangumi(&q)).await;
    match result {
        Ok(Ok(items)) => Json(serde_json::json!({ "items": items })).into_response(),
        Ok(Err(e)) => super::error_response(StatusCode::BAD_GATEWAY, &e.to_string()),
        Err(e) => super::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Internal error: {}", e),
        ),
    }
}
