//! File-based logging with daily rotation and crash (panic) reporting.
//!
//! On Windows release builds the console window is hidden
//! (`windows_subsystem = "windows"` in `main.rs`), so `tracing`'s default
//! stdout output is invisible to the user. This module redirects logs to a
//! rotating file under `<data_dir>/logs/` so that crashes or startup
//! failures can be diagnosed after the fact.

use std::path::Path;
use std::time::{Duration, SystemTime};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Number of days of log files to keep. Older files are deleted on startup.
const LOG_RETENTION_DAYS: u64 = 7;

/// Initializes global file logging and installs a panic hook that records
/// panics to the log file.
///
/// The returned [`WorkerGuard`] must be kept alive for the lifetime of the
/// process (e.g. by holding it in a `let _guard = ...;` binding in `main`),
/// otherwise the non-blocking writer may drop pending log lines on exit.
pub fn init_logging(data_dir: &Path) -> WorkerGuard {
    let log_dir = data_dir.join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        // Logging isn't available yet, fall back to eprintln.
        eprintln!("Failed to create log directory {:?}: {}", log_dir, e);
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "manhuaviewer.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    // Also log to stdout in debug builds for convenience during `tauri dev`.
    #[cfg(debug_assertions)]
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let stdout_layer = tracing_subscriber::fmt::layer();
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stdout_layer)
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();
    }

    install_panic_hook();

    // Run after the subscriber is installed so any warnings are captured.
    cleanup_old_logs(&log_dir);

    guard
}

/// Installs a panic hook that logs panic messages (including payload and
/// location) via `tracing::error!` before running the default hook, so
/// crashes are recorded in the log file even though the console is hidden.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<dyn Any>".to_string()
        };

        tracing::error!(target: "panic", "Panic at {}: {}", location, message);

        default_hook(panic_info);
    }));
}

/// Removes log files under `log_dir` older than [`LOG_RETENTION_DAYS`].
fn cleanup_old_logs(log_dir: &Path) {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(_) => return, // Directory may not exist yet on first run.
    };

    let max_age = Duration::from_secs(LOG_RETENTION_DAYS * 24 * 60 * 60);
    let now = SystemTime::now();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let modified = match entry.metadata().and_then(|m| m.modified()) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if let Ok(age) = now.duration_since(modified) {
            if age > max_age {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!("Failed to remove old log file {:?}: {}", path, e);
                }
            }
        }
    }
}
