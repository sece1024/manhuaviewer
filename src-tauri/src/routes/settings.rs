use crate::AppState;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::{internal_error, run_db};

#[derive(Deserialize)]
pub struct UpdateSettings {
    // Accept flat key-value pairs directly
    #[serde(flatten)]
    pub settings: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct UpdateConfig {
    pub root_dir: String,
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.get_settings()).await {
        Ok(settings) => Json(settings).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateSettings>,
) -> Response {
    match run_db(&state, move |db| db.update_settings(&payload.settings)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.get_setting("root_dir")).await {
        Ok(root_dir) => Json(serde_json::json!({ "root_dir": root_dir })).into_response(),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Json(serde_json::json!({ "root_dir": "" })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateConfig>,
) -> Response {
    let mut settings = std::collections::HashMap::new();
    settings.insert("root_dir".to_string(), payload.root_dir);

    match run_db(&state, move |db| db.update_settings(&settings)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn get_stats(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.get_stats()).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => internal_error(e),
    }
}

/// 本机局域网可达的 IPv4 地址 + 服务端口，供设置页/侧边栏展示"手机/平板访问地址"。
/// 枚举全部非回环、非链路本地的 IPv4（含多网卡/VPN），私有网段优先排序。
pub async fn lan_ip() -> Response {
    let ips: Vec<String> = if_addrs::get_if_addrs()
        .map(|ifaces| {
            let mut seen = std::collections::BTreeSet::new();
            for i in &ifaces {
                if let std::net::IpAddr::V4(v4) = i.ip() {
                    if !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified() {
                        seen.insert(v4);
                    }
                }
            }
            let mut v: Vec<String> = seen.iter().map(|ip| ip.to_string()).collect();
            // 私有网段（RFC1918）优先：它们才是局域网可达的常规地址
            v.sort_by_key(|ip| {
                let first = ip
                    .split('.')
                    .next()
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                match first {
                    10 => 0,
                    172 => 1,
                    192 => 2,
                    _ => 3,
                }
            });
            v
        })
        .unwrap_or_default();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5002);

    Json(serde_json::json!({ "ipv4": ips, "port": port })).into_response()
}

pub async fn export_backup(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.export_backup()).await {
        Ok(backup) => Json(backup).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn import_backup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    match run_db(&state, move |db| db.import_backup(&payload)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}
