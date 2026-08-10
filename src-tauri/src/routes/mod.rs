pub mod archives;
pub mod categories;
pub mod history;
pub mod opds;
pub mod settings;
pub mod tags;

use crate::AppState;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// Run a blocking closure against the DB pool off the async runtime.
///
/// All rusqlite calls are synchronous and must not run on the Tokio worker
/// threads; this offloads them to a blocking thread. The connection pool
/// (inside `Database`) handles concurrency, so handlers no longer serialize
/// on a single global mutex.
pub async fn run_db<T, F>(state: &Arc<AppState>, f: F) -> Result<T, rusqlite::Error>
where
    F: FnOnce(&crate::db::Database) -> rusqlite::Result<T> + Send + 'static,
    T: Send + 'static,
{
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || f(&db))
        .await
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
}

pub fn create_router(state: AppState) -> Router {
    // 生产模式下前端从 tauri://localhost 加载，需要 CORS
    let cors = CorsLayer::permissive();

    let api_routes = Router::new()
        // Archives
        .route("/archives", get(archives::list_archives))
        .route("/archives/:id", get(archives::get_archive))
        .route("/archives/:id", delete(archives::delete_archive))
        .route("/archives/:id/title", put(archives::update_archive_title))
        .route("/archives/:id/cover", get(archives::get_cover))
        .route("/archives/:id/pages", get(archives::list_pages))
        .route("/archives/:id/pages/:page", get(archives::get_page))
        .route(
            "/archives/:id/pages/:page/thumb",
            get(archives::get_page_thumb),
        )
        .route("/open", post(archives::open_file))
        .route("/scan", post(archives::scan))
        .route("/merge", post(archives::merge_archives))
        .route("/archives/pack-cbz", post(archives::pack_cbz))
        .route("/cbz/list", get(archives::list_cbz_files))
        .route(
            "/archives/batch-delete",
            post(archives::batch_delete_archives),
        )
        // Tags
        .route("/tags", get(tags::list_tags))
        .route("/tags", post(tags::create_tag))
        .route("/tags/:id", put(tags::update_tag))
        .route("/tags/:id", delete(tags::delete_tag))
        .route("/tags/assign", post(tags::assign_tag))
        .route("/tags/:archive_id/:tag_id", delete(tags::remove_tag))
        .route("/tags/namespaces", get(tags::list_namespaces))
        .route("/tags/batch-assign", post(tags::batch_assign_tag))
        .route("/tags/batch-remove", post(tags::batch_remove_tag))
        .route("/archives/:id/tags", get(tags::get_archive_tags))
        // Categories
        .route("/categories", get(categories::list_categories))
        .route("/categories", post(categories::create_category))
        .route("/categories/:id", put(categories::update_category))
        .route("/categories/:id", delete(categories::delete_category))
        .route("/categories/assign", post(categories::assign_category))
        .route(
            "/categories/:archive_id/:category_id",
            delete(categories::remove_category),
        )
        .route(
            "/categories/batch-assign",
            post(categories::batch_assign_category),
        )
        .route(
            "/categories/batch-remove",
            post(categories::batch_remove_category),
        )
        .route(
            "/archives/:id/categories",
            get(categories::get_archive_categories),
        )
        // History
        .route("/history", get(history::get_history))
        .route("/history", post(history::save_history))
        .route("/history/:archive_id", delete(history::delete_history))
        .route("/history", delete(history::clear_history))
        // Settings
        .route("/settings", get(settings::get_settings))
        .route("/settings", put(settings::update_settings))
        .route("/config", get(settings::get_config))
        .route("/config", put(settings::update_config))
        .route("/stats", get(settings::get_stats))
        // Backup
        .route("/backup", get(settings::export_backup))
        .route("/restore", post(settings::import_backup));

    // OPDS routes
    let opds_routes = Router::new()
        .route("/", get(opds::root_catalog))
        .route("/catalog", get(opds::catalog))
        .route("/archive/:id", get(opds::archive_detail))
        .route("/recent", get(opds::recent))
        .route("/tags", get(opds::tags_list))
        .route("/tag/:tag_id", get(opds::tag_archives))
        .route("/categories", get(opds::categories_list));

    Router::new()
        .nest("/api", api_routes)
        .nest("/opds", opds_routes)
        .layer(cors)
        .with_state(Arc::new(state))
}
