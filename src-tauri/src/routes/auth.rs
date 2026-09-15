//! 局域网安全：可选口令鉴权（可配置）。
//! 纯判定逻辑放最上面方便单测；`lan_guard_core` 供 `from_fn` 包裹后挂到 /api。
//! 语义（不影响单机默认体验）：server_token 为空 = 关闭鉴权（保持旧行为）；
//! 非空时，对局域网发来的“敏感请求”校验口令（本机回环始终放行，防锁死桌面端）。

use std::sync::Arc;

use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::db::Database;

/// 恒定时间字符串比较：长度不一致直接短路，长度一致时按位异或累计，
/// 避免侧信道（现实威胁低，成本可忽略）。
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 口令匹配：未配置(空) => 放行；否则 header(Bearer/裸) 或 URL ?token= 命中其一即可。
pub fn token_authorized(auth: Option<&str>, query: Option<&str>, expected: &str) -> bool {
    if expected.is_empty() {
        return true;
    }
    let bear = auth
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|v| constant_time_eq(v, expected));
    let raw = auth.is_some_and(|v| constant_time_eq(v, expected));
    let q = query.is_some_and(|v| constant_time_eq(v, expected));
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

/// 仅信任传输层真实对端 IP（ConnectInfo 由服务器注入，不可伪造）。
/// 曾经信任过客户端可控的 `X-Forwarded-For`（把 `127.0.0.1` 视为回环），
/// 而本后端直连 axum、无受信反代，LAN 攻击者加一个头即可绕过 token——
/// 该分支已删除。无 ConnectInfo（直连单测等）默认放行，防误锁。
pub(crate) fn peer_is_loopback(req: &Request) -> bool {
    req.extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().is_loopback())
        .unwrap_or(true)
}

/// 决策（纯判定，便于单测）：
/// - 回环本机始终放行（防锁死桌面端）；
/// - 未配置口令：局域网全部开放（保持旧行为）；
/// - 配置了口令：局域网一切请求（读 + 写 + OPDS）都需口令——书库内容、逐页
///   图片与阅读历史对同网段设备同样受保护。
pub fn request_allowed(
    expected_token: &str,
    authz: Option<&str>,
    query_token: Option<&str>,
    is_loopback: bool,
) -> bool {
    if is_loopback {
        return true;
    }
    if expected_token.is_empty() {
        return true;
    }
    token_authorized(authz, query_token, expected_token)
}

/// 读取当前局域网口令（供守卫与 OPDS 链接透传共用）。rusqlite 是同步 I/O，
/// 必须 offload 到阻塞线程，否则每个请求都会在 Tokio worker 上阻塞式取池连接。
pub(crate) async fn current_token(db: Arc<Database>) -> String {
    tokio::task::spawn_blocking(move || db.get_setting("server_token").unwrap_or_default())
        .await
        .unwrap_or_default()
}

/// 统一守卫核心，供 from_fn 闭包包装。先同步克隆需要的字段，再 await DB，
/// 避免在 Future（需 Send）中持有 &Request。
pub async fn lan_guard_core(db: Arc<Database>, req: Request, next: Next) -> Response {
    let headers = req.headers().clone();
    let query = req.uri().query().map(|s| s.to_string());
    let is_loopback = peer_is_loopback(&req);

    let authz = headers.get("authorization").and_then(|v| v.to_str().ok());
    let qtoken = extract_query_token(query.as_deref());

    let expected = current_token(db).await;
    let allowed = request_allowed(&expected, authz, qtoken.as_deref(), is_loopback);
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
    fn configured_token_guards_everything_on_lan() {
        // 配置口令后：读（含 OPDS）、写、全部路径都需要口令
        for p in [
            "/archives",
            "/archives/5/pages/2",
            "/history",
            "/opds",
            "/opds/catalog",
            "/settings",
        ] {
            assert!(
                !request_allowed("sec1", None, None, false),
                "GET {p} 无口令应拒"
            );
            assert!(
                request_allowed("sec1", Some("Bearer sec1"), None, false),
                "GET {p} Bearer 应放行"
            );
            assert!(
                request_allowed("sec1", None, Some("sec1"), false),
                "GET {p} ?token= 应放行"
            );
        }
        // 写操作同样需要口令
        assert!(!request_allowed("sec1", None, None, false));
        assert!(request_allowed("sec1", Some("Bearer sec1"), None, false));
    }

    #[test]
    fn loopback_always_allowed_even_with_token() {
        // 回环本机放行：桌面端不会因口令缺失被锁死
        assert!(request_allowed("sec1", None, None, true));
        assert!(request_allowed("sec1", Some("wrong"), None, true));
    }

    #[test]
    fn no_token_keeps_legacy_open_access() {
        // 未配置口令：局域网完全开放（读、写、设置均为旧行为）
        assert!(request_allowed("", None, None, false));
        assert!(request_allowed("", None, None, false));
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

    #[test]
    fn xff_header_cannot_spoof_loopback() {
        // 非回环真实对端 + 伪造 X-Forwarded-For：必须判定为非回环（曾有信任该头的绕过）
        let addr: std::net::SocketAddr = "203.0.113.5:9999".parse().unwrap();
        let req = Request::builder()
            .header("x-forwarded-for", "127.0.0.1")
            .extension(ConnectInfo(addr))
            .body(axum::body::Body::empty())
            .unwrap();
        assert!(!peer_is_loopback(&req));

        // 回环真实对端：放行（防锁死桌面端）
        let loopback: std::net::SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let req2 = Request::builder()
            .extension(ConnectInfo(loopback))
            .body(axum::body::Body::empty())
            .unwrap();
        assert!(peer_is_loopback(&req2));
    }
}
