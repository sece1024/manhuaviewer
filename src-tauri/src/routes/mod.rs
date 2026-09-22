pub mod archives;
pub mod auth;
pub mod categories;
pub mod convert;
pub mod history;
pub mod metadata;
pub mod opds;
pub mod scan;
pub mod settings;
pub mod sync;
pub mod tags;
pub mod update;

use crate::AppState;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// 内部错误统一出口：细节进日志，客户端只收通用消息（避免泄露主机路径/DB 细节）。
pub fn internal_error(err: impl std::fmt::Display) -> Response {
    tracing::error!("Internal error: {}", err);
    error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
}

/// 局域网（非回环）环境下敏感的设置项：不出现在备份/恢复与 API 响应里。
pub(crate) const LAN_SENSITIVE_SETTINGS: &[&str] = &["server_token", "server_bind"];

/// 出站 URL 目标合法性：同步的本意是“局域网内另一台相同软件”、远端封面只是下载
/// 图片，因此只放行 http(s) 的私网/普通公网地址，拒绝回环、未指定、链路本地与组播
/// ——防止本机后端被当成 SSRF 跳板访问本机自身或内网无关服务。
/// 跨机同步（sync.rs）与远程封面 URL（archives.rs）共用。
pub(crate) fn validate_outbound_url(url: &str) -> bool {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return false;
    }
    let rest = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    // 端口剥离：带括号的 IPv6 字面量按 ']' 取地址（"[::ffff:127.0.0.1]" 或
    // "[::ffff:127.0.0.1]:5002"），不能按最后一个 ':' 拆——裸 IPv6 无端口时会在
    // 地址内部被截断（"[::ffff:169.254.169.254]" 会被拆成 "::ffff"，反成合法地址）。
    let hostname = if let Some(r) = host.strip_prefix('[') {
        r.split_once(']').map(|(h, _)| h).unwrap_or(r)
    } else {
        host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
    };

    fn allowed(ip: std::net::IpAddr) -> bool {
        if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
            return false;
        }
        match ip {
            std::net::IpAddr::V4(v4) => !v4.is_link_local(),
            std::net::IpAddr::V6(v6) => {
                // IPv4 映射地址（::ffff:127.0.0.1 等）：Ipv6Addr::is_loopback() 只认 ::1，
                // 必须转回 IPv4 按 IPv4 规则再审一遍，否则回环/链路本地可借映射形式绕过白名单。
                if let Some(v4) = v6.to_ipv4_mapped() {
                    return allowed(std::net::IpAddr::V4(v4));
                }
                !v6.is_unicast_link_local()
            }
        }
    }

    if let Ok(ip) = hostname.parse::<std::net::IpAddr>() {
        return allowed(ip);
    }
    // 主机名：解析全部地址。只要任一解析结果是回环/未指定就整体拒绝（混合 A 记录
    // 可把校验与 curl/ureq 的实际连接导向本机）；其余按“任一允许”放行（LAN 主机名
    // 常见多记录，含公网/私网/链路本地混合时链路本地可容忍）。
    use std::net::ToSocketAddrs;
    let addrs: Vec<std::net::IpAddr> = match (hostname, 80u16).to_socket_addrs() {
        Ok(it) => it.map(|a| a.ip()).collect(),
        Err(_) => return false,
    };
    if addrs.is_empty() {
        return false;
    }
    if addrs
        .iter()
        .any(|ip| ip.is_loopback() || ip.is_unspecified())
    {
        return false;
    }
    addrs.iter().any(|ip| allowed(*ip))
}

/// Run a blocking closure against the DB pool off the async runtime.
///
/// All rusqlite calls are synchronous and must not run on the Tokio worker
/// threads; this offloads them to a blocking thread. The connection pool
/// (inside `Database`) handles concurrency, so handlers no longer serialize
/// on a single global mutex.
/// 把 BlockingError（任务 panic / 取消）映射进 rusqlite 的错误空间。
/// `run_db` 与 `db_json` 共用：两者都只关心“DB 调用失败”，不区分错误来源。
fn blocking_error(e: tokio::task::JoinError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(e))
}

pub async fn run_db<T, F>(state: &Arc<AppState>, f: F) -> Result<T, rusqlite::Error>
where
    F: FnOnce(&crate::db::Database) -> rusqlite::Result<T> + Send + 'static,
    T: Send + 'static,
{
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || f(&db))
        .await
        .map_err(blocking_error)?
}

/// 薄 handler 的统一收尾：把 `run_db` 的结果直接序列化为 JSON 响应，
/// 失败时统一走 `internal_error`（细节进日志，客户端只收通用消息）。
///
/// 收录的 handler 都是同一个形状——查询结果本身就是响应体，不需要后处理。
/// 需要改形状（包一层 `{"data": …}`、附加字段、按结果给不同状态码）的 handler
/// 仍然手写 match，不要为了统一而扭曲语义。
pub async fn db_json<T, F>(state: &Arc<AppState>, f: F) -> Response
where
    T: serde::Serialize + Send + 'static,
    F: FnOnce(&crate::db::Database) -> rusqlite::Result<T> + Send + 'static,
{
    match run_db(state, f).await {
        Ok(value) => Json(value).into_response(),
        Err(e) => internal_error(e),
    }
}

/// 局域网模式的前端：把 `frontend/build` 在编译期内嵌进二进制，由 HTTP 直接提供 SPA
/// （浏览器访问 `http://<本机IP>:<端口>/`）。
///
/// 不能用磁盘路径（`env!("CARGO_MANIFEST_DIR")` 是**构建机**的源码目录，安装到用户机器后
/// 根本不存在），也不能依赖 Tauri 的资源协议（那只对 WebView 可见）。内嵌是唯一与安装
/// 方式无关的做法。debug 构建下 rust-embed 改为运行时读盘，前端改动无需重编后端。
#[derive(rust_embed::RustEmbed)]
#[folder = "../frontend/build"]
struct FrontendAssets;

fn asset_response(path: &str, data: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Vite 产物带内容哈希（/assets/xxx-<hash>.js），可长期强缓存；
    // index.html 必须每次校验，否则升级后仍指向旧 chunk。
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut resp = (
        [
            (axum::http::header::CONTENT_TYPE, mime.as_ref()),
            (axum::http::header::CACHE_CONTROL, cache),
            (axum::http::header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        data,
    )
        .into_response();
    // 局域网浏览器直连 HTTP 服务时无 tauri.conf.json 的 CSP 兜底，这里为 HTML 补一份。
    if path.ends_with(".html") {
        resp.headers_mut().insert(
            axum::http::header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; connect-src 'self' http://localhost:* http://127.0.0.1:*; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'"
                .parse()
                .expect("static CSP header"),
        );
    }
    resp
}

/// SPA 兜底：静态文件命中则返回；未命中一律回 index.html，让前端路由处理
/// （否则浏览器刷新 `/history`、`/reader/1` 这类深链接会 404）。
async fn serve_frontend(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // 未匹配到的 API/OPDS 路径不能被 SPA 吞掉，否则 404 会变成 200 的 HTML。
    if path == "api" || path.starts_with("api/") || path == "opds" || path.starts_with("opds/") {
        return error_response(StatusCode::NOT_FOUND, "未知接口");
    }

    let lookup = if path.is_empty() { "index.html" } else { path };
    if let Some(f) = FrontendAssets::get(lookup) {
        return asset_response(lookup, f.data.into_owned());
    }

    match FrontendAssets::get("index.html") {
        Some(index) => asset_response("index.html", index.data.into_owned()),
        // 只有“未构建前端”的后端开发场景会走到这里。
        None => error_response(
            StatusCode::NOT_FOUND,
            "前端资源未内嵌，请先运行 pnpm --filter manhuaviewer-frontend build 后重新构建",
        ),
    }
}

/// 递归移除 JSON 中的主机内部路径字段与局域网敏感设置，防止非回环客户端枚举宿主文件系统。
/// `path`/`filepath`（文件夹档案的页面绝对路径）、组卡片的 `_parentDir`、
/// `cover_image`/`thumbnail_path`、书库根目录与打包目录，以及 server_token/server_bind。
fn strip_private_fields(value: &mut serde_json::Value) {
    match value {
        // 顶层数组（/api/archives 两个分支都返回数组）也必须递归处理——此前只处理对象，
        // 顶层数组直接透传，脱敏形同虚设
        serde_json::Value::Array(items) => {
            for item in items {
                strip_private_fields(item);
            }
        }
        serde_json::Value::Object(map) => {
            map.retain(|k, v| {
                let keep = !matches!(
                    k.as_str(),
                    "path"
                        | "filepath"
                        | "cover_image"
                        | "thumbnail_path"
                        | "parent_dir"
                        | "_parentDir"
                        | "root_dir"
                        | "cbz_export_dir"
                ) && !LAN_SENSITIVE_SETTINGS.contains(&k.as_str());
                if keep && (v.is_object() || v.is_array()) {
                    strip_private_fields(v);
                }
                keep
            });
        }
        _ => {}
    }
}

/// OPDS 报文里的站内链接透传口令：非回环 + 配置了口令时，把 XML 中所有
/// `href="/opds…/href="/api…` 追加 `?token=…`，否则客户端按 href 发起的后续请求会 401。
async fn opds_rewrite_token_links(
    db: std::sync::Arc<crate::db::Database>,
    req: Request,
    next: Next,
) -> Response {
    // 回环判定必须在把 req 交给 next 之前取（req 会被 move）
    let is_loopback = crate::routes::auth::peer_is_loopback(&req);
    let resp = next.run(req).await;
    // 只处理 OPDS XML（401 的 JSON 错误响应、SPA/静态文件直接透传）
    let is_atom = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/atom+xml"))
        .unwrap_or(false);
    if !is_atom {
        return resp;
    }
    if is_loopback {
        return resp; // 桌面端本机 OPDS 请求无需口令
    }
    let token = crate::routes::auth::current_token(db).await;
    if token.is_empty() {
        return resp;
    }
    let (mut parts, body) = resp.into_parts();
    let bytes = match axum::body::to_bytes(body, 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, axum::body::Body::empty()),
    };
    let rewritten =
        crate::routes::opds::append_token_to_links(&String::from_utf8_lossy(&bytes), &token);
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    if let Ok(len_header) = axum::http::HeaderValue::from_str(&rewritten.len().to_string()) {
        parts
            .headers
            .insert(axum::http::header::CONTENT_LENGTH, len_header);
    }
    Response::from_parts(parts, axum::body::Body::from(rewritten))
}

/// 非回环请求的 JSON 响应脱敏：剥离主机路径等内部字段。
/// 只在响应确为 application/json 时解析改写（图片/XML 等二进制体直接透传）。
async fn redact_lan_paths(req: Request, next: Next) -> Response {
    let is_loopback = crate::routes::auth::peer_is_loopback(&req);
    let resp = next.run(req).await;
    if is_loopback {
        return resp;
    }
    let is_json = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/json"))
        .unwrap_or(false);
    if !is_json {
        return resp;
    }
    let (mut parts, body) = resp.into_parts();
    let bytes = match axum::body::to_bytes(body, 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, axum::body::Body::empty()),
    };
    let mut value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return Response::from_parts(parts, axum::body::Body::from(bytes)),
    };
    strip_private_fields(&mut value);
    let new_body = serde_json::to_vec(&value).unwrap_or_else(|_| bytes.to_vec());
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    if let Ok(len_header) = axum::http::HeaderValue::from_str(&new_body.len().to_string()) {
        parts
            .headers
            .insert(axum::http::header::CONTENT_LENGTH, len_header);
    }
    Response::from_parts(parts, axum::body::Body::from(new_body))
}

pub fn create_router(state: AppState) -> Router {
    // CORS 只放行自己的前端来源（Tauri 生产 origin + 本机开发端口）。
    // 局域网模式下 UI 与 API 同源，不需要通配；通配会让任意网页跨站调用本地库。
    // 开发端口仅 debug 构建放行，release 严格只留 Tauri 生产 origin。
    let mut origin_strs = vec!["tauri://localhost", "http://tauri.localhost"];
    if cfg!(debug_assertions) {
        origin_strs.extend([
            "http://localhost:1420",
            "http://localhost:3000",
            "http://127.0.0.1:3000",
        ]);
    }
    let origins: Vec<axum::http::HeaderValue> = origin_strs
        .iter()
        .map(|s| s.parse().expect("static origin header"))
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::list(origins))
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        // Archives
        .route("/archives", get(archives::list_archives))
        .route("/archives/:id", get(archives::get_archive))
        .route("/archives/:id", delete(archives::delete_archive))
        .route("/archives/:id/title", put(archives::update_archive_title))
        .route(
            "/archives/:id/cover",
            get(archives::get_cover).put(archives::set_archive_cover),
        )
        .route(
            "/archives/:id/cover-url",
            put(archives::set_remote_cover_url),
        )
        .route("/archives/:id/pages", get(archives::list_pages))
        .route("/archives/:id/pages/:page", get(archives::get_page))
        .route(
            "/archives/:id/pages/:page/thumb",
            get(archives::get_page_thumb),
        )
        .route("/archives/:id/bookmarks", get(archives::list_bookmarks))
        .route("/archives/:id/bookmarks", post(archives::add_bookmark))
        .route(
            "/archives/:id/bookmarks/:page_index",
            delete(archives::remove_bookmark),
        )
        .route("/open", post(archives::open_file))
        .route("/scan", post(scan::scan))
        .route("/scan/status", get(scan::scan_status))
        .route("/scan/cancel", post(scan::scan_cancel))
        .route("/merge", post(archives::merge_archives))
        .route("/archives/pack-cbz", post(archives::pack_cbz))
        .route("/archives/convert-cbz/start", post(convert::convert_start))
        .route("/archives/convert-cbz/status", get(convert::convert_status))
        .route(
            "/archives/convert-cbz/cancel",
            post(convert::convert_cancel),
        )
        .route("/cbz/list", get(archives::list_cbz_files))
        .route(
            "/archives/batch-delete",
            post(archives::batch_delete_archives),
        )
        .route(
            "/archives/regenerate-titles",
            post(archives::regenerate_titles),
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
        .route("/lan-ip", get(settings::lan_ip))
        .route("/metadata/search", get(metadata::search))
        .route("/update/check", get(update::update_check))
        // Backup
        .route("/backup", get(settings::export_backup))
        .route("/restore", post(settings::import_backup))
        // 跨机同步（清单/下载端点；plan/start/status/cancel 见 sync.rs）
        .route("/sync/manifest", get(sync::sync_manifest))
        .route("/sync/plan", post(sync::sync_plan))
        .route("/sync/start", post(sync::sync_start))
        .route("/sync/status", get(sync::sync_status))
        .route("/sync/cancel", post(sync::sync_cancel))
        // 档案原文件下载（同步用，POST 以默认纳入局域网口令保护）
        .route("/archives/:id/file", post(archives::download_archive_file));

    // OPDS routes
    let opds_routes = Router::new()
        .route("/", get(opds::root_catalog))
        .route("/catalog", get(opds::catalog))
        .route("/archive/:id", get(opds::archive_detail))
        .route("/recent", get(opds::recent))
        .route("/tags", get(opds::tags_list))
        .route("/tag/:tag_id", get(opds::tag_archives))
        .route("/categories", get(opds::categories_list))
        .route("/category/:id", get(opds::category_archives));

    // 可配置局域网鉴权：关闭(server_token 为空)时与旧行为一致；开启后回环本机放行、
    // 局域网一切请求（读 + 写 + OPDS）都需口令。DB Arc 在 build 期克隆进闭包。
    let auth_db = state.db.clone();
    let auth_layer = middleware::from_fn(move |req: Request, next: Next| {
        let db = auth_db.clone();
        async move { auth::lan_guard_core(db, req, next).await }
    });

    // OPDS 链接透传口令：必须在鉴权之后跑（此时请求已通过校验），改写响应 XML
    let opds_db = state.db.clone();
    let opds_layer = middleware::from_fn(move |req: Request, next: Next| {
        let db = opds_db.clone();
        async move { opds_rewrite_token_links(db, req, next).await }
    });

    Router::new()
        .nest("/api", api_routes)
        .nest("/opds", opds_routes)
        // 部分 OPDS 阅读器会补尾斜杠；nest 下的 "/" 只匹配 `/opds`，这里补上 `/opds/`。
        .route("/opds/", get(opds::root_catalog))
        .layer(auth_layer)
        .layer(cors)
        // 非回环 JSON 响应脱敏（在鉴权之后、静态资源兜底之外）
        .layer(middleware::from_fn(redact_lan_paths))
        // OPDS XML 站内链接透传口令（口令模式下阅读器才能继续翻页/取图）
        .layer(opds_layer)
        .fallback(serve_frontend)
        // 最外层：DNS 重绑定防护——Host/Origin 主机形态校验，覆盖上面全部路由与
        // 静态兜底（axum 的 layer 会同时包住 path_router、fallback 与 catch-all）。
        // 攻击域名连 SPA 都拿不到，更无法借"回环放行"读写本地 API。
        .layer(middleware::from_fn(auth::security_guard))
        .with_state(Arc::new(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// 用给定的 DB 与数据目录起一个真实的 Axum 服务，返回端口；用裸 TCP 发请求，
    /// 避免引入 HTTP 客户端依赖。便于在启动前预置种子数据。
    async fn spawn_server_with(db: Arc<crate::db::Database>, data_dir: std::path::PathBuf) -> u16 {
        let state = crate::AppState {
            db,
            data_dir,
            last_thumb_eviction: Arc::new(std::sync::Mutex::new(None)),
        };
        let app = create_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        port
    }

    /// 起一个真实的 Axum 服务，返回端口；用裸 TCP 发请求，避免引入 HTTP 客户端依赖。
    async fn spawn_server() -> (u16, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let port = spawn_server_with(Arc::new(db), dir.path().to_path_buf()).await;
        (port, dir)
    }

    async fn get(port: u16, path: &str) -> (u16, String) {
        get_with(port, path, "127.0.0.1", None).await
    }

    /// 裸 TCP GET，可自定义 Host/Origin 头（DNS 重绑定防护用例）。
    async fn get_with(port: u16, path: &str, host: &str, origin: Option<&str>) -> (u16, String) {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n");
        if let Some(o) = origin {
            req.push_str(&format!("Origin: {o}\r\n"));
        }
        req.push_str("Connection: close\r\n\r\n");
        s.write_all(req.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8_lossy(&buf).to_string();
        let status = raw
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        (status, raw)
    }

    async fn post(port: u16, path: &str, body: &str) -> (u16, String) {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        s.write_all(
            format!(
                "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8_lossy(&buf).to_string();
        let status = raw
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        (status, raw)
    }

    /// 局域网浏览器访问：根路径与前端深链接都必须拿到 index.html。
    /// 历史回归：静态资源曾用编译期磁盘路径，安装到用户机器后整站 404。
    #[tokio::test]
    async fn serves_spa_and_deep_links() {
        if FrontendAssets::get("index.html").is_none() {
            return; // 纯后端 CI 未构建前端，跳过
        }
        let (port, _dir) = spawn_server().await;

        for path in ["/", "/history", "/reader/1"] {
            let (status, body) = get(port, path).await;
            assert_eq!(status, 200, "{path} 应返回 SPA 入口");
            assert!(
                body.contains("<div id=\"root\""),
                "{path} 应返回 index.html"
            );
        }
    }

    /// SPA 兜底不能吞掉未知接口，否则客户端会把 HTML 当 JSON 解析。
    #[tokio::test]
    async fn unknown_api_paths_stay_json_404() {
        let (port, _dir) = spawn_server().await;

        for path in ["/api/does-not-exist", "/opds/does-not-exist"] {
            let (status, body) = get(port, path).await;
            assert_eq!(status, 404, "{path} 应为 404");
            assert!(body.contains("\"error\""), "{path} 应返回 JSON 错误体");
        }
    }

    /// 部分 OPDS 阅读器会给根目录补尾斜杠。
    #[tokio::test]
    async fn opds_root_accepts_trailing_slash() {
        let (port, _dir) = spawn_server().await;

        for path in ["/opds", "/opds/"] {
            let (status, _) = get(port, path).await;
            assert_eq!(status, 200, "{path} 应可访问 OPDS 根目录");
        }
    }

    /// DNS 重绑定回归：即使对端是回环（口令规则一律放行的那类），
    /// Host/Origin 为公网域名的请求也必须 403；合法 Host 不受影响。
    #[tokio::test]
    async fn rebinding_host_and_origin_rejected() {
        let (port, _dir) = spawn_server().await;

        // Host 是重绑定域名 → 403（攻击页面把 evil.com 指到回环后的形态）
        let (status, body) = get_with(port, "/api/settings", "evil.example.com:5002", None).await;
        assert_eq!(status, 403, "重绑定域名 Host 应被拒");
        assert!(body.contains("\"error\""), "403 应为 JSON 错误体");

        // Host 正常但 Origin 是攻击者页面（跨站 POST 的形态）→ 403
        let (status, _) = get_with(
            port,
            "/api/settings",
            "127.0.0.1",
            Some("http://evil.example.com"),
        )
        .await;
        assert_eq!(status, 403, "重绑定 Origin 应被拒");

        // 合法形态照常放行：IP Host、桌面端 Origin
        let (status, _) = get_with(port, "/api/settings", "127.0.0.1", None).await;
        assert_eq!(status, 200, "回环 IP Host 应放行");
        let (status, _) = get_with(
            port,
            "/api/settings",
            "127.0.0.1",
            Some("tauri://localhost"),
        )
        .await;
        assert_eq!(status, 200, "桌面端 tauri Origin 应放行");
    }

    /// 出站白名单必须拒绝 IPv4 映射的回环/链路本地（::ffff:127.0.0.1 等），
    /// 此前 is_loopback() 只认 ::1，映射形式可绕过白名单直连本机回环服务。
    #[test]
    fn outbound_url_rejects_ipv4_mapped_loopback_and_link_local() {
        // 映射的回环/云元数据地址：一律拒绝
        assert!(!validate_outbound_url(
            "http://[::ffff:127.0.0.1]:5002/cover"
        ));
        assert!(!validate_outbound_url(
            "http://[::ffff:169.254.169.254]/latest/meta-data/"
        ));
        // 映射的私网/公网地址仍可放行（局域网同步/远程封面正常使用）
        assert!(validate_outbound_url("http://[::ffff:192.168.1.5]:5002/"));
        assert!(validate_outbound_url("http://[::ffff:203.0.113.5]:5002/"));
        // 原有行为不回归
        assert!(!validate_outbound_url("http://127.0.0.1:5002/"));
        assert!(!validate_outbound_url("http://[::1]:5002/"));
        assert!(!validate_outbound_url("ftp://192.168.1.5/x"));
        // 无法解析的主机名拒绝（离线/不存在域名）
        assert!(!validate_outbound_url(
            "http://definitely-not-a-real-host-xyz.invalid/"
        ));
    }

    /// 扫描的孤儿清理：只删除磁盘上确实不存在的路径；
    /// 磁盘上仍存在但本次扫描未发现的（深度限制/扩展名白名单等）一律保留，
    /// 避免误删手动打开或处于扫描盲区的档案及其标签/历史。
    #[tokio::test]
    async fn scan_cleanup_removes_only_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("library");
        // series/ch1 是深度 2 的文件夹档案（含图片）；deep/manga.cbz 是深度 2 的压缩包
        std::fs::create_dir_all(root.join("series/ch1")).unwrap();
        std::fs::write(root.join("series/ch1/page01.jpg"), b"img").unwrap();
        std::fs::create_dir_all(root.join("deep")).unwrap();
        std::fs::write(root.join("deep/manga.cbz"), b"x").unwrap();

        let db = crate::db::Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let root_s = root.to_string_lossy().into_owned();
        // S/D：磁盘上存在（本次扫不到应跳过不删）；G：磁盘上不存在（应清理）
        db.upsert_scanned_archive("S", &format!("{root_s}/series/ch1"), "folder", 3, 10, 111)
            .unwrap();
        db.upsert_scanned_archive("D", &format!("{root_s}/deep/manga.cbz"), "cbz", 3, 20, 222)
            .unwrap();
        db.upsert_scanned_archive("G", &format!("{root_s}/gone.cbz"), "cbz", 3, 30, 333)
            .unwrap();

        let db_arc = Arc::new(db);
        let port = spawn_server_with(db_arc.clone(), dir.path().to_path_buf()).await;

        // 深度 1：只有根的直接子项被遍历，series/ch1 与 deep/manga.cbz 都扫不到
        let body = serde_json::json!({ "path": root_s, "depth": 1 }).to_string();
        let (status, raw) = post(port, "/api/scan", &body).await;
        assert_eq!(status, 200, "扫描应成功: {}", raw);
        let json_part = raw.split("\r\n\r\n").nth(1).expect("响应应有 JSON 体");
        let resp: serde_json::Value = serde_json::from_str(json_part).expect("响应体应为合法 JSON");
        assert_eq!(
            resp["removed"].as_u64(),
            Some(1),
            "磁盘上已消失的档案应被清理: {raw}"
        );
        assert_eq!(
            resp["skipped"].as_u64(),
            Some(2),
            "存在但本次扫不到的档案应被跳过: {raw}"
        );

        // 数据库终态：G 被删，S/D 保留
        assert!(db_arc
            .get_archive_by_path(&format!("{root_s}/series/ch1"))
            .unwrap()
            .is_some());
        assert!(db_arc
            .get_archive_by_path(&format!("{root_s}/deep/manga.cbz"))
            .unwrap()
            .is_some());
        assert!(db_arc
            .get_archive_by_path(&format!("{root_s}/gone.cbz"))
            .unwrap()
            .is_none());
    }

    /// 脱敏必须对顶层数组也生效（/api/archives 返回数组；只处理对象会整个透传）。
    #[test]
    fn strip_private_fields_handles_top_level_arrays_and_new_keys() {
        let mut value = serde_json::json!([
            {
                "id": 1,
                "title": "A",
                "path": "/Users/host/secret/a.cbz",
                "_parentDir": "/Users/host/secret",
                "filepath": "/Users/host/secret/pages/1.jpg",
                "cover_image": "/Users/host/secret/c.jpg",
                "thumbnail_path": "thumbnails/1",
                "extras": [{ "path": "/leak", "name": "ok" }],
            }
        ]);
        strip_private_fields(&mut value);
        let first = &value[0];
        assert!(first.get("path").is_none(), "顶层数组内的 path 应被剥离");
        assert!(first.get("_parentDir").is_none());
        assert!(first.get("filepath").is_none());
        assert!(first.get("cover_image").is_none());
        assert!(first.get("thumbnail_path").is_none());
        assert_eq!(first["title"], "A");
        assert_eq!(first["extras"][0]["name"], "ok");
        assert!(first["extras"][0].get("path").is_none(), "嵌套对象同样剥离");
    }

    /// 书库列表必须携带每档案的标签（此前 /archives 不带 tags，卡片标签/色点永不显示）。
    #[tokio::test]
    async fn archives_list_includes_tags() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let id = db
            .upsert_scanned_archive("Tagged", "/x/a.cbz", "cbz", 5, 10, 1)
            .unwrap();
        let tag_id = db.create_tag("", "日常", "#4a86e8").unwrap();
        db.assign_tag(id, tag_id).unwrap();

        let db_arc = Arc::new(db);
        let port = spawn_server_with(db_arc.clone(), dir.path().to_path_buf()).await;
        let (status, raw) = get(
            port,
            "/api/archives?limit=50&page=1&sort_by=created&sort_order=asc",
        )
        .await;
        assert_eq!(status, 200, "/archives 应成功: {}", raw);
        assert!(raw.contains("\"日常\""), "列表响应应包含档案标签: {}", raw);
    }

    /// 手动删除磁盘档案后 DB 记录残留：打开（页列表/封面/页面图片/缩略图）应回
    /// 明确 404 + 可操作提示，而不是在列目录/解压等深层 I/O 上失败后被吞成笼统的 500。
    /// 文件夹与压缩包两类档案都要覆盖。
    #[tokio::test]
    async fn missing_archive_file_returns_404_not_500() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Database::new(dir.path().join("t.db").to_str().unwrap()).unwrap();
        db.init().unwrap();
        let file_id = db
            .upsert_scanned_archive("GoneCbz", "/definitely/not/on/disk/a.cbz", "cbz", 5, 100, 1)
            .unwrap();
        let folder_id = db
            .upsert_scanned_archive(
                "GoneFolder",
                "/definitely/not/on/disk/folder",
                "folder",
                3,
                0,
                1,
            )
            .unwrap();

        let db_arc = Arc::new(db);
        let port = spawn_server_with(db_arc.clone(), dir.path().to_path_buf()).await;

        for (id, kind) in [(file_id, "压缩包"), (folder_id, "文件夹")] {
            for endpoint in [
                format!("/api/archives/{id}/pages"),
                format!("/api/archives/{id}/cover"),
                format!("/api/archives/{id}/pages/0"),
                format!("/api/archives/{id}/pages/0/thumb"),
            ] {
                let (status, raw) = get(port, &endpoint).await;
                assert_eq!(
                    status, 404,
                    "{kind} 档案 {endpoint} 应回 404 而非 500: {raw}"
                );
                assert!(raw.contains("档案"), "{endpoint} 应带可操作提示: {raw}");
            }
        }
    }
}
