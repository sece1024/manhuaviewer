// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod models;
mod routes;
mod services;
mod utils;

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<db::Database>>,
    pub data_dir: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("Starting MangaViewer v3.0.0");

    // Determine data directory
    let data_dir = if let Ok(dir) = std::env::var("DATA_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("MangaViewer")
            .join("data")
    };

    // Create data directory if it doesn't exist
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    // Initialize database
    let db_path = data_dir.join("manhuaviewer.db");
    let db_path_str = db_path.to_str().expect("Database path contains invalid UTF-8");
    let database = db::Database::new(db_path_str).expect("Failed to open database");
    database.init().expect("Failed to initialize database");

    info!("Database initialized at {:?}", db_path);

    // Create app state
    let state = AppState {
        db: Arc::new(Mutex::new(database)),
        data_dir: data_dir.clone(),
    };

    // Build Axum router for API
    let api_router = routes::create_router(state.clone());

    // Start Tauri application
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |_app| {
            // Start embedded web server
            tokio::spawn(async move {
                let port: u16 = std::env::var("PORT")
                    .unwrap_or_else(|_| "5002".to_string())
                    .parse()
                    .unwrap_or(5002);

                let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                    .await
                    .expect("Failed to bind to port");

                let addr = listener.local_addr().unwrap();
                info!("API server listening on http://{}", addr);

                info!("API server ready on port {}", addr.port());

                if let Err(e) = axum::serve(listener, api_router).await {
                    tracing::error!("API server error: {}", e);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
