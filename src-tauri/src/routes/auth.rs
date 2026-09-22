//! 局域网安全：可选口令鉴权（可配置）+ DNS 重绑定防护。
//! 纯判定逻辑放最上面方便单测；`lan_guard_core` 供 `from_fn` 包裹后挂到 /api。
//! 语义（不影响单机默认体验）：server_token 为空 = 关闭鉴权（保持旧行为）；
//! 非空时，局域网（非回环）的**一切**请求——读、写、OPDS、逐页图片——都需校验口令
//! （本机回环始终放行，防锁死桌面端）。详见 `request_allowed` 上方注释与单测。
//! 另有最外层守卫 `security_guard`（Host/Origin 主机形态校验）：攻击者把 evil.com
//! 先指到自己再改指回环（DNS 重绑定）后，浏览器会带 `Host: evil.com:5002` 访问本服务，
//! 而回环对端在上面的口令规则里始终放行——没有该守卫时本地 API 等于全开。

use std::sync::Arc;

use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, StatusCode};
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

// ── DNS 重绑定防护：Host/Origin 主机形态校验（纯判定，与口令守卫同文件单测）──

/// 从 Host/authority 剥掉端口与 IPv6 方括号，取出主机名：
/// `"nas:5002" → "nas"`、`"[::1]:5002" → "::1"`、`"127.0.0.1 → "127.0.0.1"`。
fn hostname_of(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    match host.rsplit_once(':') {
        Some((h, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => h,
        _ => host,
    }
}

/// 主机名（可含端口）是否为"本机/局域网"形态：字面 IP、`localhost`
/// （RFC 6761 保证 `*.localhost` 也恒解析到本机，覆盖 Windows WebView 的
/// `tauri.localhost`）、mDNS `*.local` 或无点的单标签短名（nas、manga）。
/// 其余带点域名一律拒绝：公网 DNS 域名都能被攻击者先指到自己页面、再改指
/// 回环（DNS 重绑定）；`127.0.0.1.nip.io` 这类"回环到域名"的花样也因
/// 带点且非 .local/.localhost 被挡下。访问方式因此限定为 IP/短名（见 README）。
fn is_localish_hostname(authority: &str) -> bool {
    let name = hostname_of(authority);
    if name.is_empty() {
        return false;
    }
    // 字面 IP（含裸 IPv6）：浏览器的 Host 来自所连 URL，攻击者无法让
    // 浏览器对本服务伪造出"公网域名"以外的字面量；裸 IPv6 也可能被 rsplit
    // 截成 "::"，仍是 IP 字面量而非域名，同样安全。
    if name.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    if name.contains(':') {
        return false; // 残缺/畸形的冒号形态（如 "evil:com"）不猜、直接拒
    }
    let lower = name.to_ascii_lowercase();
    lower == "localhost"
        || lower.ends_with(".localhost")
        || lower.ends_with(".local")
        || !lower.contains('.')
}

/// Host 头是否可接受。缺失时放行：浏览器发起的 HTTP/1.1 请求必带 Host，
/// 重绑定攻击换不来"无 Host"的请求；不带 Host 的只有 curl/脚本等非浏览器
/// 客户端，它们照常走口令鉴权，不构成重绑定向量（同时兼容 HTTP/2 的
/// `:authority` 未被映射为 Host 的情形）。
pub fn host_allowed(host: Option<&str>) -> bool {
    match host {
        None => true,
        Some(h) => is_localish_hostname(h),
    }
}

/// Origin 头是否可接受：无 Origin（curl、OPDS 阅读器、页面导航）放行；有则
/// 解析 `scheme://host[:port]` 并按同一主机形态规则判定。`null`（沙盒 iframe
/// 等不透明来源）与非 http(s)/tauri 的 scheme（chrome-extension:// 等）拒绝。
pub fn origin_allowed(origin: Option<&str>) -> bool {
    let Some(o) = origin.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    if o == "null" {
        return false;
    }
    let Some((scheme, rest)) = o.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https" | "tauri") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    is_localish_hostname(authority)
}

/// 整请求判定（守卫与单测共用）：Host 与 Origin（若存在）都必须是本机/局域网形态。
pub fn request_host_origin_ok(headers: &HeaderMap) -> bool {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok());
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    host_allowed(host) && origin_allowed(origin)
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

/// DNS 重绑定守卫（挂最外层，覆盖 /api、/opds 与静态兜底）：主机形态不符一律 403。
/// 没有它，重绑定页面作为"回环本机"在口令规则下始终放行，可直接读写本地 API。
pub async fn security_guard(req: Request, next: Next) -> Response {
    if request_host_origin_ok(req.headers()) {
        next.run(req).await
    } else {
        super::error_response(
            StatusCode::FORBIDDEN,
            "拒绝以该 Host/Origin 访问（防 DNS 重绑定），请改用 IP/localhost/局域网短名",
        )
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

    #[test]
    fn host_guard_allows_local_forms_rejects_rebind_domains() {
        // 本机/局域网形态放行：字面 IP（含端口/IPv6 括号）、localhost 族、mDNS、单标签短名
        assert!(host_allowed(Some("127.0.0.1:5002")));
        assert!(host_allowed(Some("127.0.0.1")));
        assert!(host_allowed(Some("192.168.1.5:5002")));
        assert!(host_allowed(Some("[::1]:5002")));
        assert!(host_allowed(Some("[fd00::1]")));
        assert!(host_allowed(Some("localhost:3000")));
        assert!(host_allowed(Some("LocalHost")));
        assert!(host_allowed(Some("tauri.localhost:5002")));
        assert!(host_allowed(Some("nas:5002"))); // 无点局域网短名
        assert!(host_allowed(Some("manga.local:5002"))); // mDNS
        assert!(host_allowed(None)); // 无 Host：非浏览器客户端，不构成重绑定

        // 重绑定域名与残缺形态拒绝
        assert!(!host_allowed(Some("evil.com:5002")));
        assert!(!host_allowed(Some("evil.com")));
        assert!(!host_allowed(Some("127.0.0.1.nip.io:5002"))); // "回环到域名"花样
        assert!(!host_allowed(Some("localhost.evil.com")));
        assert!(!host_allowed(Some("evil:com")));
        assert!(!host_allowed(Some("")));
    }

    #[test]
    fn origin_guard_allows_local_tauri_rejects_rebind() {
        // 无 Origin（curl/OPDS/导航）与本机/局域网来源放行
        assert!(origin_allowed(None));
        assert!(origin_allowed(Some("")));
        assert!(origin_allowed(Some("http://192.168.1.5:5002")));
        assert!(origin_allowed(Some("http://127.0.0.1:5002")));
        assert!(origin_allowed(Some("http://localhost:3000")));
        assert!(origin_allowed(Some("tauri://localhost"))); // macOS/Linux 桌面端
        assert!(origin_allowed(Some("http://tauri.localhost"))); // Windows WebView
        assert!(origin_allowed(Some("http://nas:5002")));

        // 重绑定来源、不透明来源与外来 scheme 拒绝
        assert!(!origin_allowed(Some("http://evil.com:5002")));
        assert!(!origin_allowed(Some("https://attacker.example")));
        assert!(!origin_allowed(Some("null")));
        assert!(!origin_allowed(Some("file://")));
        assert!(!origin_allowed(Some("chrome-extension://abcdefghijklmnop")));
        assert!(!origin_allowed(Some("evil.com"))); // 无 scheme
    }

    #[test]
    fn request_host_origin_ok_combines_both_headers() {
        let ok = |host: &str, origin: Option<&str>| {
            let mut b = axum::http::Request::builder().header("host", host);
            if let Some(o) = origin {
                b = b.header("origin", o);
            }
            let req = b.body(()).unwrap();
            request_host_origin_ok(req.headers())
        };
        assert!(ok("127.0.0.1:5002", Some("tauri://localhost")));
        assert!(ok("192.168.1.5:5002", None));
        assert!(!ok("evil.com:5002", None), "重绑定 Host 单独就该拒");
        assert!(
            !ok("127.0.0.1:5002", Some("http://evil.com")),
            "恶意 Origin 单独就该拒"
        );
    }
}
