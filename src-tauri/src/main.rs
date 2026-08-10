// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod logging;
mod routes;
mod services;

use std::sync::Arc;
use tauri::Manager;
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<db::Database>,
    pub data_dir: std::path::PathBuf,
}

/// Logs a fatal startup error, shows a native error dialog so the user isn't
/// left staring at an unresponsive/vanishing app, and exits the process.
///
/// This replaces `.expect()`/`panic!` for startup failures: on Windows the
/// release build hides the console (`windows_subsystem = "windows"`), so an
/// unhandled panic previously meant the app would just silently disappear
/// with no indication of what went wrong.
fn fatal_error(context: &str, err: impl std::fmt::Display) -> ! {
    let message = format!("{}: {}", context, err);
    tracing::error!("{}", message);
    let _ = rfd::MessageDialog::new()
        .set_title("MangaViewer 启动失败")
        .set_description(&message)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    // Determine data directory
    let data_dir = if let Ok(dir) = std::env::var("DATA_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("MangaViewer")
            .join("data")
    };

    // Initialize file logging (rotates daily, keeps 7 days, installs a panic
    // hook). The guard must stay alive for the whole process lifetime.
    let _log_guard = logging::init_logging(&data_dir);

    info!("Starting MangaViewer v{}", env!("CARGO_PKG_VERSION"));

    // Create data directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        fatal_error(&format!("无法创建数据目录 {:?}", data_dir), e);
    }

    // Initialize database
    let db_path = data_dir.join("manhuaviewer.db");
    let db_path_str = match db_path.to_str() {
        Some(s) => s,
        None => fatal_error("数据库路径包含无效字符", db_path.display()),
    };
    let database = match db::Database::new(db_path_str) {
        Ok(d) => d,
        Err(e) => fatal_error("无法打开数据库", e),
    };
    if let Err(e) = database.init() {
        fatal_error("无法初始化数据库", e);
    }

    info!("Database initialized at {:?}", db_path);

    // Create app state
    let state = AppState {
        db: Arc::new(database),
        data_dir: data_dir.clone(),
    };

    // Build Axum router for API
    let api_router = routes::create_router(state.clone());

    // Start Tauri application
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            info!("Another instance was launched; focusing existing window instead");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state)
        .setup(move |_app| {
            // Start embedded web server
            tokio::spawn(async move {
                let port: u16 = std::env::var("PORT")
                    .unwrap_or_else(|_| "5002".to_string())
                    .parse()
                    .unwrap_or(5002);

                let listener =
                    match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                        Ok(l) => l,
                        Err(e) => fatal_error(
                            &format!(
                            "无法绑定端口 {}，可能已有 MangaViewer 实例正在运行，请检查任务管理器",
                            port
                        ),
                            e,
                        ),
                    };

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
