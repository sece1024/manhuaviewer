use crate::AppState;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::{db_json, error_response, internal_error, run_db};

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

/// 纯判定：该设置键是否允许当前对端写入。`server_token`/`server_bind`
/// （LAN_SENSITIVE_SETTINGS）与响应脱敏同源——仅回环本机可改：未配口令时
/// 局域网请求完全开放，若放任改绑定地址/口令，任何同网段设备都能把服务
/// 暴露到 0.0.0.0 或改掉口令接管访问。规则与恢复备份时的过滤保持一致。
fn setting_key_writable(key: &str, is_loopback: bool) -> bool {
    is_loopback || !super::LAN_SENSITIVE_SETTINGS.contains(&key)
}

/// 类型 ↔ 扩展名同族：zip/cbz、rar/cbr 互认（内容格式相同），7z 只认 .7z。
/// 扫描/转换/同步产出的条目必然同族，跨族即为伪造备份。
fn extension_matches_type(archive_type: &str, path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match archive_type {
        "zip" | "cbz" => matches!(ext.as_str(), "zip" | "cbz"),
        "rar" | "cbr" => matches!(ext.as_str(), "rar" | "cbr"),
        "7z" => ext == "7z",
        _ => false,
    }
}

/// 备份载荷校验（POST /api/restore 完全由对端控制）：档案条目必须满足类型
/// 白名单、类型与路径扩展名同族、路径非空且无 NUL——`/archives/:id/file` 会按
/// path 原样回传文件字节，不校验等于把"恢复备份"变成任意文件读取入口
/// （如把 /Users/x/.ssh/id_rsa 标成 zip 后经 /file 拉走）。
/// 入选条件与 `db.import_backup` 的四字段判断一致：不齐全的条目它本就跳过。
pub(crate) fn validate_import_payload(backup: &serde_json::Value) -> Result<(), String> {
    let Some(archives) = backup["archives"].as_array() else {
        return Ok(());
    };
    for entry in archives {
        let (Some(_title), Some(path), Some(archive_type), Some(_page_count)) = (
            entry["title"].as_str(),
            entry["path"].as_str(),
            entry["archive_type"].as_str(),
            entry["page_count"].as_i64(),
        ) else {
            continue;
        };
        if path.is_empty() || path.contains('\0') {
            return Err(format!("备份含非法档案路径：{path:?}"));
        }
        match archive_type {
            // 目录无扩展名约束（目录名可以带点）
            "folder" => {}
            "zip" | "cbz" | "rar" | "cbr" | "7z" => {
                if !extension_matches_type(archive_type, path) {
                    return Err(format!("档案类型与扩展名不匹配：{path} ({archive_type})"));
                }
            }
            other => return Err(format!("备份含未知档案类型：{other}")),
        }
    }
    Ok(())
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> Response {
    db_json(&state, |db| db.get_settings()).await
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    peer: ConnectInfo<std::net::SocketAddr>,
    Json(payload): Json<UpdateSettings>,
) -> Response {
    // 敏感键（server_token/server_bind）仅回环可写：非回环携带时整单 403，
    // 前端会 toast 错误——不做静默丢弃，否则界面显示"已保存"而值未变更误导。
    let is_loopback = peer.0.ip().is_loopback();
    if let Some(key) = payload
        .settings
        .keys()
        .find(|k| !setting_key_writable(k, is_loopback))
    {
        return error_response(StatusCode::FORBIDDEN, &format!("设置 {key} 仅限本机修改"));
    }
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
    db_json(&state, |db| db.get_stats()).await
}

/// 枚举本机非回环、非链路本地的 IPv4（含多网卡/VPN），私有网段（RFC1918）优先排序。
fn enumerate_lan_ipv4() -> Vec<String> {
    if_addrs::get_if_addrs()
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
        .unwrap_or_default()
}

/// 本机局域网可达的 IPv4 地址 + 服务端口，供设置页/侧边栏展示"手机/平板访问地址"。
/// 只对回环客户端（本机桌面端或本机浏览器）返回网卡地址：手机/平板等局域网设备
/// 打开页面时拿不到宿主 IP——它们本就是通过该地址连进来的，无需展示，也避免向局域网广播主机网络布局。
pub async fn lan_ip(req: Request) -> Response {
    let is_loopback = crate::routes::auth::peer_is_loopback(&req);
    let ips = if is_loopback {
        enumerate_lan_ipv4()
    } else {
        Vec::new()
    };

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5002);

    Json(serde_json::json!({ "ipv4": ips, "port": port, "loopback": is_loopback })).into_response()
}

pub async fn export_backup(State(state): State<Arc<AppState>>) -> Response {
    db_json(&state, |db| db.export_backup()).await
}

pub async fn import_backup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Response {
    // 恢复备份是对端可控输入：先整体校验档案条目，非法内容返回 400（而非落库后 500）
    if let Err(msg) = validate_import_payload(&payload) {
        return error_response(StatusCode::BAD_REQUEST, &msg);
    }
    match run_db(&state, move |db| db.import_backup(&payload)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 枚举到的地址必须是合法 IPv4 且不含回环/链路本地。
    #[test]
    fn enumerate_lan_ipv4_filters_loopback_and_link_local() {
        for ip in enumerate_lan_ipv4() {
            let parsed: std::net::Ipv4Addr = ip.parse().expect("应为合法 IPv4");
            assert!(!parsed.is_loopback(), "{ip} 不应是回环");
            assert!(!parsed.is_link_local(), "{ip} 不应是链路本地");
        }
    }

    /// 敏感设置仅回环可写；普通键两侧都可写（与响应脱敏/备份过滤同源规则）。
    #[test]
    fn sensitive_settings_writable_only_from_loopback() {
        for key in ["server_token", "server_bind"] {
            assert!(setting_key_writable(key, true), "{key} 回环应可写");
            assert!(!setting_key_writable(key, false), "{key} 局域网应拒");
        }
        assert!(setting_key_writable("page_mode", false));
        assert!(setting_key_writable("theme", true));
    }

    /// 备份档案条目校验：正常条目放行，篡改条目（类型伪造/跨族扩展名/脏路径）拒绝。
    #[test]
    fn import_payload_rejects_tampered_archives() {
        let v = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();

        // 正常压缩包与大写扩展名（归一化后同族）
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"/m/a.cbz","archive_type":"cbz","page_count":9}]}"#
        ))
        .is_ok());
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"/m/A.ZIP","archive_type":"zip","page_count":9}]}"#
        ))
        .is_ok());
        // 目录：名字可带点，无扩展名约束
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"/m/My.Folder","archive_type":"folder","page_count":9}]}"#
        ))
        .is_ok());

        // id_rsa 标成 zip：无扩展名 → 拒（任意文件读的原始入口）
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"/Users/x/.ssh/id_rsa","archive_type":"zip","page_count":1}]}"#
        ))
        .is_err());
        // 类型与扩展名跨族 → 拒
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"/m/a.zip","archive_type":"rar","page_count":1}]}"#
        ))
        .is_err());
        // 未知类型 → 拒
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"/m/a","archive_type":"exe","page_count":1}]}"#
        ))
        .is_err());
        // 空路径与 NUL → 拒
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"","archive_type":"zip","page_count":1}]}"#
        ))
        .is_err());
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"title":"t","path":"/m/a\u0000.cbz","archive_type":"cbz","page_count":1}]}"#
        ))
        .is_err());

        // 四字段不齐（不会入库）→ 跳过不报错；无 archives 段 → Ok
        assert!(validate_import_payload(&v(
            r#"{"archives":[{"path":"/x/id_rsa","archive_type":"zip"}]}"#
        ))
        .is_ok());
        assert!(validate_import_payload(&v(r#"{"tags":[]}"#)).is_ok());
    }
}
