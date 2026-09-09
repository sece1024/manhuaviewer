//! 局域网安全：可选口令鉴权（可配置）。
//! 纯判定逻辑放最上面方便单测；`lan_guard_core` 供 `from_fn` 包裹后挂到 /api。
//! 语义（不影响单机默认体验）：server_token 为空 = 关闭鉴权（保持旧行为）；
//! 非空时，对局域网发来的“敏感请求”校验口令（本机回环始终放行，防锁死桌面端）。

use std::sync::Arc;

use axum::extract::{ConnectInfo, Request};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::db::Database;

/// 即便是 GET 也会泄露配置/整库，需口令的 /api 前缀（带前导斜杠）。
const SENSITIVE_GET_PREFIXES: &[&str] = &["/settings", "/backup", "/config"];

fn is_write_method(m: &Method) -> bool {
    matches!(
        *m,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// 敏感判定：写操作一律敏感；GET 命中 settings/backup/config 前缀也敏感。
/// `path` 为带前导 `/`、已剥 `/api` 前缀的形式（如 `/scan`、`/settings`）。
pub fn request_is_sensitive(m: &Method, path: &str) -> bool {
    if is_write_method(m) {
        return true;
    }
    let first = path.trim_start_matches('/').split('/').next().unwrap_or("");
    SENSITIVE_GET_PREFIXES
        .iter()
        .any(|p| first == p.trim_start_matches('/'))
}

/// 口令匹配：未配置(空) => 放行；否则 header(Bearer/裸) 或 URL ?token= 命中其一即可。
pub fn token_authorized(auth: Option<&str>, query: Option<&str>, expected: &str) -> bool {
    if expected.is_empty() {
        return true;
    }
    let bear = auth
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|v| v == expected);
    let raw = auth.is_some_and(|v| v == expected);
    let q = query.is_some_and(|v| v == expected);
    bear || raw || q
}

fn extract_query_token(q: Option<&str>) -> Option<String> {
    q.and_then(|query| {
        query
            .split('&')
            .find_map(|pair| match pair.split_once('=') {
                Some(("token", v)) => Some(v.to_string()),
                _ => None,
            })
    })
}

fn peer_is_loopback(req: &Request) -> bool {
    req.extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().is_loopback())
        .unwrap_or(true) // 无 ConnectInfo（直连单测等）默认放行，防误锁
        || req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.starts_with("127.0.0.1") || v.starts_with("::1"))
            .unwrap_or(false)
}

async fn configured_token(db: &Database) -> String {
    db.get_setting("server_token").unwrap_or_default()
}

/// 决策（只依赖 owned、Send 数据，可在 await 间安全持有）。
/// 返回 true=放行；false=需要口令但缺失（调用方回 401）。
async fn sensitive_allowed(
    db: &Database,
    method: &Method,
    api_path: &str,
    authz: Option<&str>,
    query_token: Option<&str>,
    is_loopback: bool,
) -> bool {
    if !request_is_sensitive(method, api_path) {
        return true;
    }
    if is_loopback {
        return true;
    }
    let expected = configured_token(db).await;
    if expected.is_empty() {
        return true;
    }
    token_authorized(authz, query_token, &expected)
}

fn strip_api(path: &str) -> String {
    let stripped = path.strip_prefix("/api").unwrap_or(path);
    if stripped.is_empty() || !stripped.starts_with('/') {
        format!("/{stripped}")
    } else {
        stripped.to_string()
    }
}

/// 统一守卫核心，供 from_fn 闭包包装。先同步克隆需要的字段，再 await DB，
/// 避免在 Future（需 Send）中持有 &Request。
pub async fn lan_guard_core(db: Arc<Database>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let api_path = strip_api(req.uri().path());
    let headers = req.headers().clone();
    let query = req.uri().query().map(|s| s.to_string());
    let is_loopback = peer_is_loopback(&req);

    let authz = headers.get("authorization").and_then(|v| v.to_str().ok());
    let qtoken = extract_query_token(query.as_deref());

    let allowed = sensitive_allowed(
        &db,
        &method,
        &api_path,
        authz,
        qtoken.as_deref(),
        is_loopback,
    )
    .await;
    if allowed {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "需要局域网访问口令 (server_token)" })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_are_sensitive() {
        for m in [Method::POST, Method::PUT, Method::DELETE] {
            for p in ["/scan", "/open", "/restore", "/archives/5", "/merge"] {
                assert!(request_is_sensitive(&m, p), "{m} {p}");
            }
        }
    }

    #[test]
    fn gets_open_except_sensitive_prefixes() {
        for p in ["/archives", "/archives/5/pages/2", "/history", "/scan"] {
            assert!(!request_is_sensitive(&Method::GET, p), "{p}");
        }
        for p in ["/settings", "/backup", "/config"] {
            assert!(request_is_sensitive(&Method::GET, p), "{p}");
        }
    }

    #[test]
    fn token_scenarios() {
        // 未配置 => 放行
        assert!(token_authorized(None, None, ""));
        assert!(token_authorized(Some("anything"), Some("x"), ""));
        // 配置后匹配规则
        assert!(token_authorized(Some("Bearer sec1"), Some("bad"), "sec1"));
        assert!(token_authorized(Some("sec1"), None, "sec1"));
        assert!(token_authorized(None, Some("sec1"), "sec1"));
        assert!(
            !token_authorized(Some("Bearer sec1 "), None, "sec1"),
            "尾随空格应视为不同字符串"
        );
        assert!(!token_authorized(
            Some("Bearer wrong"),
            Some("other"),
            "sec1"
        ));
    }
}
