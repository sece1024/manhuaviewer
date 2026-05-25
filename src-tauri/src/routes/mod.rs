pub mod archives;
pub mod tags;
pub mod categories;
pub mod history;
pub mod settings;
pub mod opds;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use crate::AppState;
use std::sync::Arc;

pub fn create_router(state: AppState) -> Router {
    let api_routes = Router::new()
        // Archives
        .route("/archives", get(archives::list_archives))
        .route("/archives/:id", get(archives::get_archive))
        .route("/archives/:id", delete(archives::delete_archive))
        .route("/archives/:id/cover", get(archives::get_cover))
        .route("/archives/:id/pages", get(archives::list_pages))
        .route("/archives/:id/pages/:page", get(archives::get_page))
        .route("/archives/:id/pages/:page/thumb", get(archives::get_page_thumb))
        .route("/open", post(archives::open_file))
        .route("/scan", post(archives::scan))
        
        // Tags
        .route("/tags", get(tags::list_tags))
        .route("/tags", post(tags::create_tag))
        .route("/tags/:id", put(tags::update_tag))
        .route("/tags/:id", delete(tags::delete_tag))
        .route("/tags/assign", post(tags::assign_tag))
        .route("/tags/:archive_id/:tag_id", delete(tags::remove_tag))
        .route("/tags/namespaces", get(tags::list_namespaces))
        
        // Categories
        .route("/categories", get(categories::list_categories))
        .route("/categories", post(categories::create_category))
        .route("/categories/:id", put(categories::update_category))
        .route("/categories/:id", delete(categories::delete_category))
        .route("/categories/assign", post(categories::assign_category))
        .route("/categories/:archive_id/:category_id", delete(categories::remove_category))
        
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

    Router::new()
        .nest("/api", api_routes)
        .with_state(Arc::new(state))
}
