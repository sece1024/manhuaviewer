use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use super::{error_response, internal_error};

const CACHE_CONTROL: &str = "private, max-age=3600, must-revalidate";

/// 缩略图访问节流表（进程内），避免每个图片请求都写一次 DB。
static THUMB_TOUCH: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<i64, std::time::Instant>>,
> = std::sync::OnceLock::new();

fn archive_mtime(path: &str) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

pub(crate) fn archive_mtime_secs(path: &str) -> i64 {
    crate::services::fs_ext::mtime_secs(std::path::Path::new(path))
}

/// 档案已不在磁盘上的统一响应：404 + 可操作提示（提示重新扫描书库）。
/// 手动从磁盘删除档案后 DB 记录会残留，打开时应先预检（见 `archive_exists`），
/// 否则会在列目录/解压等深层 I/O 上失败并被 `internal_error` 吞成笼统的 500。
fn archive_missing_response() -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "档案文件不存在或已被移动，请重新扫描书库",
    )
}

/// 返回路径的父目录（去除末尾分隔符），与 `title`+`parent` 过滤的分组键保持一致。
fn parent_dir_of(path: &str) -> String {
    std::path::Path::new(path)
        .parent()
        .map(|p| {
            p.to_string_lossy()
                .trim_end_matches(['/', '\\'])
                .to_string()
        })
        .unwrap_or_default()
}

/// 列表接口返回项：普通档案或合并后的组卡片。
#[derive(Serialize)]
pub struct ListItem {
    #[serde(flatten)]
    pub archive: crate::db::ArchiveRow,
    #[serde(rename = "_isGroup")]
    pub is_group: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter_count: Option<i64>,
    #[serde(rename = "_autoGroup", skip_serializing_if = "Option::is_none")]
    pub auto_group: Option<bool>,
    #[serde(rename = "_autoKey", skip_serializing_if = "Option::is_none")]
    pub auto_key: Option<String>,
    #[serde(rename = "_parentDir", skip_serializing_if = "Option::is_none")]
    pub parent_dir: Option<String>,
    /// 最近阅读进度（0-based 页码），来自 history 表；无阅读记录时为 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_page: Option<i64>,
    /// 该档案的标签（批量 IN 查询附加）；无标签时为 Some([])。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<crate::db::TagRow>>,
}

enum GroupKey {
    Permanent(i64),
    Auto(String),
}

/// 将档案按 group_id（永久合并）与「同标题 + 同父目录」（自动合并）聚合为组卡片，
/// 保持首条成员在原排序中的位置。
fn group_archives(rows: Vec<crate::db::ArchiveRow>) -> Vec<ListItem> {
    let mut order: Vec<GroupKey> = Vec::new();
    let mut permanent: HashMap<i64, Vec<crate::db::ArchiveRow>> = HashMap::new();
    let mut auto: HashMap<String, Vec<crate::db::ArchiveRow>> = HashMap::new();
    let mut seen_perm: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut seen_auto: std::collections::HashSet<String> = std::collections::HashSet::new();

    for a in rows {
        if let Some(gid) = a.group_id {
            if seen_perm.insert(gid) {
                order.push(GroupKey::Permanent(gid));
            }
            permanent.entry(gid).or_default().push(a);
        } else {
            let parent = parent_dir_of(&a.path);
            let key = format!("{}\u{0}{}", parent, a.title.to_lowercase());
            if seen_auto.insert(key.clone()) {
                order.push(GroupKey::Auto(key.clone()));
            }
            auto.entry(key).or_default().push(a);
        }
    }

    order
        .into_iter()
        .map(|key| match key {
            GroupKey::Permanent(gid) => {
                let members = permanent.remove(&gid).unwrap_or_default();
                let primary = members
                    .iter()
                    .find(|m| m.id == gid)
                    .cloned()
                    .unwrap_or_else(|| members[0].clone());
                ListItem {
                    archive: primary,
                    is_group: true,
                    chapter_count: Some(members.len() as i64),
                    auto_group: None,
                    auto_key: None,
                    parent_dir: None,
                    read_page: None,
                    tags: None,
                }
            }
            GroupKey::Auto(key) => {
                let members = auto.remove(&key).unwrap_or_default();
                if members.len() < 2 {
                    let archive = members.into_iter().next().unwrap();
                    ListItem {
                        archive,
                        is_group: false,
                        chapter_count: None,
                        auto_group: None,
                        auto_key: None,
                        parent_dir: None,
                        read_page: None,
                        tags: None,
                    }
                } else {
                    let primary = members[0].clone();
                    let parent = parent_dir_of(&primary.path);
                    ListItem {
                        archive: primary,
                        is_group: true,
                        chapter_count: Some(members.len() as i64),
                        auto_group: Some(true),
                        auto_key: Some(key),
                        parent_dir: Some(parent),
                        read_page: None,
                        tags: None,
                    }
                }
            }
        })
        .collect()
}

fn etag_for_page(id: i64, page_index: i64, mtime: Option<SystemTime>) -> String {
    let secs = mtime
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("\"p-{}-{}-{}\"", id, page_index, secs)
}

fn etag_for_cover(id: i64, mtime: Option<SystemTime>, override_key: Option<&str>) -> String {
    let secs = mtime
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("\"c-{}-{}-{}\"", id, secs, override_key.unwrap_or(""))
}

/// 缩略图目录内的档案 mtime 标记文件：缓存命中时校验它，档案变更即作废整批缩略图。
const THUMB_ARCHIVE_MARKER: &str = "archive.mtime";

fn read_thumb_archive_marker(dir: &std::path::Path) -> Option<i64> {
    std::fs::read_to_string(dir.join(THUMB_ARCHIVE_MARKER))
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn write_thumb_archive_marker(dir: &std::path::Path, mtime_secs: i64) {
    let _ = std::fs::write(dir.join(THUMB_ARCHIVE_MARKER), mtime_secs.to_string());
}

fn http_date(t: SystemTime) -> Option<String> {
    let dt: DateTime<Utc> = t.into();
    Some(dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string())
}

fn parse_http_date(s: &str) -> Option<SystemTime> {
    DateTime::parse_from_rfc2822(s)
        .ok()
        .map(|d| d.with_timezone(&Utc).into())
}

fn build_response<B>(status: StatusCode, pairs: Vec<(&'static str, String)>, body: B) -> Response
where
    B: IntoResponse,
{
    let mut hm = HeaderMap::new();
    for (k, v) in pairs {
        if let (Ok(name), Ok(val)) = (k.parse::<HeaderName>(), v.parse::<HeaderValue>()) {
            hm.insert(name, val);
        }
    }
    (status, hm, body).into_response()
}

fn not_modified(etag: String, last_modified: Option<String>) -> Response {
    let mut pairs: Vec<(&'static str, String)> =
        vec![("ETag", etag), ("Cache-Control", CACHE_CONTROL.to_string())];
    if let Some(lm) = last_modified {
        pairs.push(("Last-Modified", lm));
    }
    build_response(StatusCode::NOT_MODIFIED, pairs, "")
}

/// 节流记录缩略图访问（每档案 ≤1 次/60 秒），避免书库滚动时产生大量 DB 写；
/// touch 后 LRU 会按真实使用时间保留/淘汰缩略图目录。
async fn touch_thumbnail_usage(state: &Arc<AppState>, id: i64) {
    {
        let map = THUMB_TOUCH
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
            .lock()
            .unwrap();
        if let Some(t) = map.get(&id) {
            if t.elapsed().as_secs() < 60 {
                return;
            }
        }
    }
    let _ = super::run_db(state, move |db| db.touch_thumbnail_access(id)).await;
    THUMB_TOUCH
        .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
        .lock()
        .unwrap()
        .insert(id, std::time::Instant::now());
}

/// 生成缩略图后登记 thumbnail_path 并按需触发 LRU 淘汰（每分钟最多一次）。
async fn register_thumbnail(
    state: &Arc<AppState>,
    id: i64,
    thumb_dir_str: String,
    already_set: bool,
) {
    let mut do_evict = false;
    {
        let mut last = state.last_thumb_eviction.lock().unwrap();
        let elapsed = last
            .as_ref()
            .map(|t| t.elapsed().as_secs() >= 60)
            .unwrap_or(true);
        if elapsed {
            *last = Some(std::time::Instant::now());
            do_evict = true;
        }
    }

    let evicted = if do_evict {
        let dir = thumb_dir_str.clone();
        super::run_db(state, move |db| {
            db.set_thumbnail_path(id, &dir)?;
            // 排除刚注册的档案，避免自淘汰刚写入的缩略图目录
            db.evict_old_thumbnails(Some(id))
        })
        .await
        .unwrap_or_default()
    } else {
        if !already_set {
            let dir = thumb_dir_str.clone();
            let _ = super::run_db(state, move |db| db.set_thumbnail_path(id, &dir)).await;
        }
        vec![]
    };

    for (_evicted_id, evicted_path) in evicted {
        let _ = tokio::fs::remove_dir_all(&evicted_path).await;
    }
}

#[derive(Deserialize)]
pub struct ArchiveQuery {
    #[serde(alias = "sort_by")]
    pub sort: Option<String>,
    #[serde(alias = "sort_order")]
    pub order: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
    pub tag: Option<String>,
    pub category_id: Option<i64>,
    /// read=已读（有阅读记录）/ unread=未读，缺省全部
    pub read: Option<String>,
    pub group_id: Option<i64>,
    /// 随机排序的会话种子：同一 seed 下顺序确定，滚动加载更多不会跨页重复/遗漏
    pub seed: Option<i64>,
    /// 精确标题过滤（配合 parent 用于拉取自动分组完整成员列表）
    pub title: Option<String>,
    /// 父目录路径（精确标题过滤时按此筛选同目录成员）
    pub parent: Option<String>,
}

#[derive(Deserialize)]
pub struct OpenFileRequest {
    #[serde(alias = "filePath")]
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct ScanRequest {
    pub path: Option<String>,
    pub depth: Option<u32>,
}

#[derive(Deserialize)]
pub struct PackCbzRequest {
    /// 源文件夹路径
    #[serde(alias = "folderPath")]
    pub folder_path: String,
    /// 可选：覆盖归档目录（不传则从 settings 读取）
    #[serde(alias = "outputDir")]
    pub output_dir: Option<String>,
}

/// 确定性伪随机 key：给定 (id, seed) 稳定输出，用于随机排序的分页一致性。
/// SQL 的 RANDOM() 每次查询重排，导致“加载更多”会重复/漏掉条目。
fn stable_random_key(id: i64, seed: i64) -> u64 {
    let mut h = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (seed as u64);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

/// 把「原始档案行」转成不带组信息的 ListItem，并批量附上阅读进度。
/// 供标题精确查询与 group 章节列表使用（章节行可显示已读进度）。
fn plain_list_items(
    db: &crate::db::Database,
    rows: Vec<crate::db::ArchiveRow>,
) -> rusqlite::Result<Vec<ListItem>> {
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let progress = db.get_history_for_archives(&ids)?;
    let map: std::collections::HashMap<i64, i64> = progress.into_iter().collect();
    // 批量附加标签（单条 IN 查询，避免逐档案 N+1）
    let tags_map = db.get_archive_tags_batch(&ids)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let archive_id = row.id;
            let read_page = map.get(&archive_id).copied();
            ListItem {
                archive: row,
                is_group: false,
                chapter_count: None,
                auto_group: None,
                auto_key: None,
                parent_dir: None,
                read_page,
                tags: Some(tags_map.get(&archive_id).cloned().unwrap_or_default()),
            }
        })
        .collect())
}

pub async fn list_archives(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArchiveQuery>,
) -> Response {
    enum ListResult {
        Raw(Vec<ListItem>),
        Grouped(Vec<ListItem>),
    }

    let result = super::run_db(&state, move |db| {
        // 精确标题 + 父目录：拉取自动分组完整成员列表（不做分页/搜索）
        if let Some(title) = query.title.as_deref().filter(|t| !t.is_empty()) {
            let mut rows = db.get_archives_by_title(title)?;
            if let Some(parent) = query.parent.as_deref().filter(|p| !p.is_empty()) {
                let normalized = parent.trim_end_matches(['/', '\\']);
                rows.retain(|a| {
                    let parent_dir = std::path::Path::new(&a.path).parent().map(|p| {
                        p.to_string_lossy()
                            .trim_end_matches(['/', '\\'])
                            .to_string()
                    });
                    parent_dir.as_deref() == Some(normalized)
                });
            }
            return plain_list_items(db, rows).map(ListResult::Raw);
        }

        // 如果指定了 group_id，返回组内所有章节
        if let Some(group_id) = query.group_id {
            let rows = db.get_group_chapters(group_id)?;
            return plain_list_items(db, rows).map(ListResult::Raw);
        }

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).clamp(1, 500);
        let offset = (page - 1).checked_mul(limit).unwrap_or(0);
        let sort = query.sort.as_deref().unwrap_or("updated");
        let order = query.order.as_deref().unwrap_or("desc");

        let mut rows = db.list_archives_all(
            query.search.as_deref(),
            query.tag.as_deref(),
            query.category_id,
            query.read.as_deref(),
            sort,
            order,
        )?;
        // 随机排序：带会话种子时改为确定性洗牌，保证分页/无限滚动不重不漏；
        // 未带 seed（旧客户端）保持库里 RANDOM() 的原行为。
        if sort == "random" {
            let seed = query.seed.unwrap_or(0);
            rows.sort_by_key(|a| stable_random_key(a.id, seed));
        }
        let mut grouped: Vec<ListItem> = group_archives(rows)
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();

        // 附上当前页卡片的阅读进度（read_page），供书库进度条/已读展示使用。
        // 合并组卡片显示其主成员（即排序最靠前的那一话）的进度。
        let ids: Vec<i64> = grouped.iter().map(|item| item.archive.id).collect();
        let progress = db.get_history_for_archives(&ids)?;
        let progress_map: std::collections::HashMap<i64, i64> = progress.into_iter().collect();
        // 批量附加标签：让书库卡片的标签 chips / 色点真正有数据可渲染
        let tags_map = db.get_archive_tags_batch(&ids)?;
        for item in grouped.iter_mut() {
            if let Some(page) = progress_map.get(&item.archive.id) {
                item.read_page = Some(*page);
            }
            item.tags = Some(tags_map.get(&item.archive.id).cloned().unwrap_or_default());
        }

        Ok(ListResult::Grouped(grouped))
    })
    .await;

    match result {
        Ok(ListResult::Raw(archives)) => Json(archives).into_response(),
        Ok(ListResult::Grouped(items)) => Json(items).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn get_archive(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    match super::run_db(&state, move |db| db.get_archive(id)).await {
        Ok(Some(archive)) => Json(serde_json::json!({ "data": archive })).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => internal_error(e),
    }
}

pub async fn delete_archive(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let thumb_dir = state.data_dir.join("thumbnails").join(id.to_string());
    let extract_dir = state.data_dir.join("extract").join(id.to_string());

    match super::run_db(&state, move |db| db.delete_archive(id)).await {
        Ok(_) => {
            // 删除缩略图目录与解压缓存目录
            let _ = tokio::fs::remove_dir_all(&thumb_dir).await;
            let _ = tokio::fs::remove_dir_all(&extract_dir).await;
            Json(serde_json::json!({ "success": true })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

#[derive(Deserialize)]
pub struct BatchDeleteRequest {
    pub ids: Vec<i64>,
}

pub async fn batch_delete_archives(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BatchDeleteRequest>,
) -> Response {
    if payload.ids.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "ids 不能为空");
    }

    let ids = payload.ids;
    let ids_db = ids.clone();
    match super::run_db(&state, move |db| db.batch_delete_archives(&ids_db)).await {
        Ok(affected) => {
            // 逐个清理缩略图目录与解压缓存目录
            for id in &ids {
                let thumb_dir = state.data_dir.join("thumbnails").join(id.to_string());
                let _ = tokio::fs::remove_dir_all(&thumb_dir).await;
                let extract_dir = state.data_dir.join("extract").join(id.to_string());
                let _ = tokio::fs::remove_dir_all(&extract_dir).await;
            }
            Json(serde_json::json!({ "success": true, "affected": affected })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

#[derive(Deserialize)]
pub struct UpdateTitleRequest {
    title: String,
}

pub async fn update_archive_title(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateTitleRequest>,
) -> Response {
    let title_db = payload.title.clone();
    match super::run_db(&state, move |db| db.update_archive_title(id, &title_db)).await {
        Ok(_) => Json(serde_json::json!({ "id": id, "title": payload.title })).into_response(),
        Err(e) => internal_error(e),
    }
}

#[derive(Deserialize)]
pub struct MergeRequest {
    archive_ids: Vec<i64>,
}

pub async fn merge_archives(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MergeRequest>,
) -> Response {
    if payload.archive_ids.len() < 2 {
        return error_response(StatusCode::BAD_REQUEST, "需要至少选择 2 个档案进行合并");
    }

    let ids = payload.archive_ids;
    let result = super::run_db(&state, move |db| {
        let primary_id = db.merge_archives(&ids)?;
        let chapter_count = db.get_group_chapters(primary_id)?.len();
        Ok((primary_id, chapter_count))
    })
    .await;

    match result {
        Ok((primary_id, chapter_count)) => Json(serde_json::json!({
            "group_id": primary_id,
            "chapter_count": chapter_count,
        }))
        .into_response(),
        Err(e) => internal_error(e),
    }
}

/// 下载远程图片到本地缓存文件（幂等：已有文件则直接返回其内容）。
/// 用系统 curl（桌面端均有），避免为单次下载引入 HTTP 依赖。
/// 目标地址必须通过出站白名单（拒绝回环/未指定/链路本地），防止 SSRF。
fn remote_cover_bytes(id: i64, url: &str, covers_dir: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    if !super::validate_outbound_url(url) {
        anyhow::bail!("拒绝下载远程封面：仅允许 http(s) 的局域网/公网地址");
    }
    std::fs::create_dir_all(covers_dir)?;
    let dest = covers_dir.join(format!("{}.img", id));
    if !dest.is_file() {
        let status = std::process::Command::new("curl")
            .args(["-fsSL", "--max-time", "25", "-o"])
            .arg(&dest)
            .arg(url)
            .status()?;
        if !status.success() || !dest.is_file() {
            let _ = std::fs::remove_file(&dest);
            anyhow::bail!("failed to download remote cover");
        }
    }
    Ok(std::fs::read(&dest)?)
}

pub async fn get_cover(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    let (archive_path, archive_type, thumb_already_set, cover_override, remote_cover) =
        match super::run_db(&state, move |db| db.get_archive_with_remote_cover(id)).await {
            Ok(Some((a, rc))) => (
                a.path,
                a.archive_type,
                a.thumbnail_path.is_some(),
                a.cover_image,
                rc,
            ),
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return internal_error(e),
        };

    // 磁盘上已不存在的档案（手动删除/磁盘变动）：回到明确 404，而不是深层 I/O 后 500
    if !crate::services::archive::archive_exists(&archive_type, &archive_path) {
        return archive_missing_response();
    }

    let mtime = archive_mtime(&archive_path);
    // ETag 同时纳入覆写页与远程封面，二者任一变化都会使浏览器缓存失效
    let etag_key = cover_override
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| remote_cover.as_deref().filter(|s| !s.is_empty()));
    let etag = etag_for_cover(id, mtime, etag_key);
    let last_modified = mtime.and_then(http_date);

    if let Some(inm) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
        if inm == etag {
            return not_modified(etag, last_modified);
        }
    }
    if let (Some(ims), Some(mt)) = (
        headers
            .get("if-modified-since")
            .and_then(|v| v.to_str().ok()),
        mtime,
    ) {
        if let Some(parsed) = parse_http_date(ims) {
            if mt <= parsed {
                return not_modified(etag, last_modified);
            }
        }
    }

    // 封面走缩略图缓存：生成 2:3 的等比缩略图，避免每屏都解压原始首页大图。
    let thumb_dir = state.data_dir.join("thumbnails").join(id.to_string());
    let thumb_dir_str = thumb_dir.to_string_lossy().to_string();
    // RAR/7z 等压缩包：持久化解压目录（<data_dir>/extract/{id}/），首次访问整包解压后直接读盘
    let extract_dir = state.data_dir.join("extract").join(id.to_string());
    // 远程封面缓存目录
    let covers_dir = state.data_dir.join("covers");
    let result = tokio::task::spawn_blocking(move || {
        let cache_path = thumb_dir.join("cover.jpg");

        // 缓存有效性：缓存文件 mtime 不早于档案 mtime，否则视为过期需重生成
        let cache_valid = std::fs::metadata(&cache_path)
            .and_then(|m| m.modified())
            .map(|cache_mt| mtime.map(|am| cache_mt >= am).unwrap_or(false))
            .unwrap_or(false);
        if cache_valid {
            if let Ok(data) = std::fs::read(&cache_path) {
                return Ok::<_, anyhow::Error>((data, "image/jpeg".to_string(), false));
            }
        }

        let reader = crate::services::archive::create_archive_reader_with_cache(
            &archive_path,
            &archive_type,
            Some(extract_dir),
        )?;
        // 封面来源优先级：手动指定页 > 远程封面 URL > 首页
        let cover = if let Some(name) = cover_override.as_deref().filter(|s| !s.is_empty()) {
            reader.extract_page(name)?
        } else if let Some(url) = remote_cover.as_deref().filter(|s| !s.is_empty()) {
            match remote_cover_bytes(id, url, &covers_dir) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::warn!("Remote cover download failed ({}): {}", url, e);
                    reader.get_cover()?
                }
            }
        } else {
            reader.get_cover()?
        };
        match crate::services::thumbnail::with_generation_permit(|| {
            crate::services::thumbnail::ThumbnailGenerator::default().generate(&cover)
        }) {
            Ok(thumb) => {
                std::fs::create_dir_all(&thumb_dir)?;
                // 原子写（tmp+rename）：并发请求同时生成时，读取方永远拿不到半截 jpg
                let tmp_path = cache_path.with_extension("jpg.tmp");
                std::fs::write(&tmp_path, &thumb)?;
                std::fs::rename(&tmp_path, &cache_path)?;
                Ok((thumb, "image/jpeg".to_string(), true))
            }
            Err(e) => {
                // 解码器不支持的格式（image crate 无 avif 解码器等）：降级返回原始封面，
                // 让系统 WebView 自己解码，而不是对整个档案报 500。
                tracing::warn!(
                    "Thumbnail decode failed for archive {}: {}; serving original cover",
                    id,
                    e
                );
                let first_page = reader.list_pages()?.into_iter().next().unwrap_or_default();
                let mime = mime_guess::from_path(first_page)
                    .first_or_octet_stream()
                    .to_string();
                Ok((cover, mime, false))
            }
        }
    })
    .await;

    match result {
        Ok(Ok((cover_data, content_type, fresh))) => {
            if fresh {
                register_thumbnail(&state, id, thumb_dir_str, thumb_already_set).await;
            }
            // 记录一次访问（节流），让 LRU 保留真正在用的封面
            touch_thumbnail_usage(&state, id).await;
            let mut pairs: Vec<(&'static str, String)> = vec![
                ("Content-Type", content_type),
                ("ETag", etag),
                ("Cache-Control", CACHE_CONTROL.to_string()),
            ];
            if let Some(lm) = last_modified {
                pairs.push(("Last-Modified", lm));
            }
            build_response(StatusCode::OK, pairs, cover_data)
        }
        Ok(Err(e)) => internal_error(e),
        Err(e) => internal_error(e),
    }
}

pub async fn list_pages(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let (archive, read_page) = match super::run_db(&state, move |db| {
        let archive = db.get_archive(id)?;
        let read_page = db
            .get_history_for_archive(id)
            .map(|h| h.map(|h| h.page_index).unwrap_or(0))
            .unwrap_or(0);
        Ok((archive, read_page))
    })
    .await
    {
        Ok((Some(a), read_page)) => (a, read_page),
        Ok((None, _)) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => return internal_error(e),
    };

    // 磁盘上已不存在的档案：明确 404（见 archive_missing_response）
    if !crate::services::archive::archive_exists(&archive.archive_type, &archive.path) {
        return archive_missing_response();
    }

    let archive_path = archive.path.clone();
    let archive_type = archive.archive_type.clone();
    let archive_id = archive.id;
    let mtime_secs = archive_mtime_secs(&archive_path);
    let db = state.db.clone();

    let result = tokio::task::spawn_blocking(move || {
        crate::services::page_cache::load_page_rows(
            &db,
            archive_id,
            &archive_path,
            &archive_type,
            mtime_secs,
        )
    })
    .await;

    match result {
        Ok(Ok(pages)) => {
            let page_list: Vec<serde_json::Value> = pages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    serde_json::json!({
                        "id": i,
                        "archive_id": id,
                        "filename": p.filename,
                        "filepath": p.filepath,
                        "sort_order": i,
                        "url": format!("/api/archives/{}/pages/{}", id, i),
                        "thumb_url": format!("/api/archives/{}/pages/{}/thumb", id, i),
                    })
                })
                .collect();

            Json(serde_json::json!({
                "archive": {
                    "id": archive.id,
                    "title": archive.title,
                    "archive_type": archive.archive_type,
                    "path": archive.path,
                },
                "pages": page_list,
                "read_page": read_page,
            }))
            .into_response()
        }
        Ok(Err(e)) => internal_error(e),
        Err(e) => internal_error(e),
    }
}

pub async fn get_page(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
    headers: HeaderMap,
) -> Response {
    if page_index < 0 {
        return error_response(StatusCode::BAD_REQUEST, "Page index must be non-negative");
    }

    let (archive_id, archive_path, archive_type) =
        match super::run_db(&state, move |db| db.get_archive(id)).await {
            Ok(Some(a)) => (a.id, a.path, a.archive_type),
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return internal_error(e),
        };

    // 磁盘上已不存在的档案：明确 404（见 archive_missing_response）
    if !crate::services::archive::archive_exists(&archive_type, &archive_path) {
        return archive_missing_response();
    }

    let mtime = archive_mtime(&archive_path);
    let etag = etag_for_page(id, page_index, mtime);
    let last_modified = mtime.and_then(http_date);

    if let Some(inm) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
        if inm == etag {
            return not_modified(etag, last_modified);
        }
    }
    if let (Some(ims), Some(mt)) = (
        headers
            .get("if-modified-since")
            .and_then(|v| v.to_str().ok()),
        mtime,
    ) {
        if let Some(parsed) = parse_http_date(ims) {
            if mt <= parsed {
                return not_modified(etag, last_modified);
            }
        }
    }

    let mtime_secs = archive_mtime_secs(&archive_path);
    let db = state.db.clone();

    // 文件夹档案：与原来一致，直接流式输出文件
    if archive_type == "folder" {
        let result = tokio::task::spawn_blocking(move || {
            let pages = crate::services::page_cache::load_page_rows(
                &db,
                archive_id,
                &archive_path,
                &archive_type,
                mtime_secs,
            )?;
            let idx = page_index as usize;
            if idx >= pages.len() {
                anyhow::bail!("Page index {} out of range (total: {})", idx, pages.len());
            }
            let page_name = &pages[idx].filepath;
            let mime = mime_guess::from_path(page_name)
                .first_or_octet_stream()
                .to_string();
            Ok::<_, anyhow::Error>((mime, page_name.clone()))
        })
        .await;

        return match result {
            Ok(Ok((mime, page_name))) => match tokio::fs::File::open(&page_name).await {
                Ok(file) => {
                    let stream = tokio_util::io::ReaderStream::new(file);
                    let mut pairs: Vec<(&'static str, String)> = vec![
                        ("Content-Type", mime),
                        ("ETag", etag),
                        ("Cache-Control", CACHE_CONTROL.to_string()),
                    ];
                    if let Some(lm) = last_modified {
                        pairs.push(("Last-Modified", lm));
                    }
                    build_response(StatusCode::OK, pairs, axum::body::Body::from_stream(stream))
                }
                Err(e) => internal_error(e),
            },
            Ok(Err(e)) => {
                let msg = e.to_string();
                if msg.contains("out of range") {
                    error_response(StatusCode::NOT_FOUND, &msg)
                } else {
                    internal_error(msg)
                }
            }
            Err(e) => internal_error(e),
        };
    }

    // 压缩包页面：分块流式（64KB/块，mpsc 管道），整页不再一次进内存。
    // oneshot 先传回响应的 mime（页面名在阻塞线程里才拿到），随后逐块喂给响应体。
    let extract_dir = state.data_dir.join("extract").join(archive_id.to_string());
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<axum::body::Bytes>>(4);
    let (mtx, mrx) = tokio::sync::oneshot::channel::<String>();
    tokio::task::spawn_blocking(move || {
        // 初始化（取行 + mime）；失败时发空 mime 并在流里发一条错误
        let init = (|| -> anyhow::Result<(String, String)> {
            let pages = crate::services::page_cache::load_page_rows(
                &db,
                archive_id,
                &archive_path,
                &archive_type,
                mtime_secs,
            )?;
            let idx = page_index as usize;
            if idx >= pages.len() {
                anyhow::bail!("Page index {} out of range (total: {})", idx, pages.len());
            }
            let page_name = &pages[idx].filepath;
            let mime = mime_guess::from_path(page_name)
                .first_or_octet_stream()
                .to_string();
            Ok((mime, page_name.clone()))
        })();
        let (mime, page_name) = match init {
            Ok(v) => v,
            Err(e) => {
                let _ = mtx.send(String::new());
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };
        let _ = mtx.send(mime);
        let reader = match crate::services::archive::create_archive_reader_with_cache(
            &archive_path,
            &archive_type,
            Some(extract_dir),
        ) {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
                return;
            }
        };
        let mut drain = |chunk: &[u8]| -> anyhow::Result<()> {
            let b = axum::body::Bytes::copy_from_slice(chunk);
            // spawn_blocking 内用阻塞式发送，不占异步运行时
            tx.blocking_send(Ok(b))
                .map_err(|_| anyhow::anyhow!("stream consumer closed"))?;
            Ok(())
        };
        if let Err(e) = reader.stream_page(&page_name, &mut drain) {
            let _ = tx.blocking_send(Err(std::io::Error::other(e.to_string())));
        }
    });

    match mrx.await {
        Ok(mime) if !mime.is_empty() => {
            let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
            let mut pairs: Vec<(&'static str, String)> = vec![
                ("Content-Type", mime),
                ("ETag", etag),
                ("Cache-Control", CACHE_CONTROL.to_string()),
            ];
            if let Some(lm) = last_modified {
                pairs.push(("Last-Modified", lm));
            }
            build_response(StatusCode::OK, pairs, axum::body::Body::from_stream(stream))
        }
        // 初始化失败：错误项已在流里（客户端读流时会遇到），这里回 500
        Ok(_) => error_response(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误"),
        Err(_) => internal_error("页面读取失败（连接中断）"),
    }
}

pub async fn get_page_thumb(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
    headers: HeaderMap,
) -> Response {
    if page_index < 0 {
        return error_response(StatusCode::BAD_REQUEST, "Page index must be non-negative");
    }

    let (archive_path, archive_type, thumb_dir, thumb_already_set) =
        match super::run_db(&state, move |db| db.get_archive(id)).await {
            Ok(Some(a)) => {
                let dir = state.data_dir.join("thumbnails").join(id.to_string());
                (a.path, a.archive_type, dir, a.thumbnail_path.is_some())
            }
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return internal_error(e),
        };

    // 磁盘上已不存在的档案：明确 404（见 archive_missing_response）
    if !crate::services::archive::archive_exists(&archive_type, &archive_path) {
        return archive_missing_response();
    }

    // 缓存命中路径整体放进阻塞线程（exists/stat/read 都是同步 IO），
    // 并校验档案 mtime 标记：档案已变更时旧缩略图作废（清目录后走重新生成）。
    let thumb_dir_cache = thumb_dir.clone();
    let archive_path_cache = archive_path.clone();
    let cache_hit =
        tokio::task::spawn_blocking(move || -> Option<(Vec<u8>, Option<SystemTime>)> {
            if read_thumb_archive_marker(&thumb_dir_cache)
                != Some(archive_mtime_secs(&archive_path_cache))
            {
                let _ = std::fs::remove_dir_all(&thumb_dir_cache);
            }
            let cache_path = thumb_dir_cache.join(format!("{}.jpg", page_index));
            if !cache_path.exists() {
                return None;
            }
            let file_mtime = std::fs::metadata(&cache_path)
                .and_then(|m| m.modified())
                .ok();
            match std::fs::read(&cache_path) {
                Ok(data) => Some((data, file_mtime)),
                Err(e) => {
                    tracing::warn!("Failed to read thumbnail cache: {}", e);
                    None
                }
            }
        })
        .await
        .unwrap_or(None);

    // 缓存命中则直接返回（304 判断与响应装配仍在 async 侧）
    if let Some((data, file_mtime)) = cache_hit {
        if let (Some(ims), Some(fmt)) = (
            headers
                .get("if-modified-since")
                .and_then(|v| v.to_str().ok()),
            file_mtime,
        ) {
            if let Some(parsed) = parse_http_date(ims) {
                if fmt <= parsed {
                    let mut pairs: Vec<(&'static str, String)> =
                        vec![("Cache-Control", CACHE_CONTROL.to_string())];
                    if let Some(lm) = http_date(fmt) {
                        pairs.push(("Last-Modified", lm));
                    }
                    return build_response(StatusCode::NOT_MODIFIED, pairs, "");
                }
            }
        }
        {
            let mut pairs: Vec<(&'static str, String)> = vec![
                ("Content-Type", "image/jpeg".to_string()),
                ("Cache-Control", CACHE_CONTROL.to_string()),
            ];
            if let Some(lm) = file_mtime.and_then(http_date) {
                pairs.push(("Last-Modified", lm));
            }
            if let Some(d) = file_mtime.and_then(|mt| mt.duration_since(std::time::UNIX_EPOCH).ok())
            {
                pairs.push(("ETag", format!("\"thumb-{}-{}\"", id, d.as_secs())));
            }
            touch_thumbnail_usage(&state, id).await;
            return build_response(StatusCode::OK, pairs, data);
        }
    }

    // 缓存未命中，打开压缩包生成缩略图
    let thumb_dir_clone = thumb_dir.clone();
    let mtime_secs = archive_mtime_secs(&archive_path);
    let db = state.db.clone();
    // 压缩包页面：持久化解压目录，避免 RAR/7z 每页 spawn 子进程 + tempdir
    let extract_dir = state.data_dir.join("extract").join(id.to_string());
    let result = tokio::task::spawn_blocking(move || {
        let pages = crate::services::page_cache::load_page_rows(
            &db,
            id,
            &archive_path,
            &archive_type,
            mtime_secs,
        )?;
        let idx = page_index as usize;
        if idx >= pages.len() {
            anyhow::bail!("Page index {} out of range (total: {})", idx, pages.len());
        }
        let page_name = &pages[idx].filepath;
        let page_mime = mime_guess::from_path(page_name)
            .first_or_octet_stream()
            .to_string();
        let reader = crate::services::archive::create_archive_reader_with_cache(
            &archive_path,
            &archive_type,
            Some(extract_dir),
        )?;
        let data = reader.extract_page(page_name)?;
        let thumb_gen = crate::services::thumbnail::ThumbnailGenerator::default();
        // generate_with_cache 使用 thumb_dir 作为缓存目录；解码失败（如 avif 无解码器）时
        // 降级返回原图 bytes（由系统 WebView 解码），而不是对整页缩略图报 500。
        // 阻塞获取全局生成许可：面板/网格并发 miss 的解码被限制在 4 路以内。
        match crate::services::thumbnail::with_generation_permit(|| {
            thumb_gen.generate_with_cache(&data, &thumb_dir_clone, &page_index.to_string())
        }) {
            Ok(thumb) => {
                // 记录档案 mtime 标记：之后缓存命中时据此判定整批缩略图是否仍有效
                write_thumb_archive_marker(&thumb_dir_clone, mtime_secs);
                Ok::<_, anyhow::Error>((thumb, "image/jpeg".to_string(), true))
            }
            Err(e) => {
                tracing::warn!(
                    "Thumbnail decode failed for archive {} page {}: {}; serving original page",
                    id,
                    idx,
                    e
                );
                Ok((data, page_mime, false))
            }
        }
    })
    .await;

    match result {
        Ok(Ok((thumb_data, content_type, fresh))) => {
            if fresh {
                // 首次成功生成 jpg，更新数据库记录；LRU 淘汰最多每分钟跑一次
                let thumb_dir_str = thumb_dir.to_string_lossy().to_string();
                register_thumbnail(&state, id, thumb_dir_str, thumb_already_set).await;
            }

            // 记录一次访问（节流），让 LRU 保留真正在用的缩略图
            touch_thumbnail_usage(&state, id).await;

            let mut pairs: Vec<(&'static str, String)> = vec![
                ("Content-Type", content_type),
                ("Cache-Control", CACHE_CONTROL.to_string()),
            ];
            // 200 响应同样带验证头：fresh 用缓存文件 mtime，降级原图用档案 mtime
            let lm = if fresh {
                std::fs::metadata(thumb_dir.join(format!("{}.jpg", page_index)))
                    .and_then(|m| m.modified())
                    .ok()
            } else {
                Some(
                    std::time::UNIX_EPOCH
                        + std::time::Duration::from_secs(mtime_secs.max(0) as u64),
                )
            };
            if let Some(mt) = lm {
                if let Some(d) = http_date(mt) {
                    pairs.push(("Last-Modified", d));
                }
                if let Ok(d) = mt.duration_since(std::time::UNIX_EPOCH) {
                    pairs.push(("ETag", format!("\"thumb-{}-{}\"", id, d.as_secs())));
                }
            }

            build_response(StatusCode::OK, pairs, thumb_data)
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("out of range") {
                error_response(StatusCode::NOT_FOUND, &msg)
            } else {
                internal_error(msg)
            }
        }
        Err(e) => internal_error(e),
    }
}

/// 下载原始档案文件（跨机同步/迁移用）：压缩包直接流式回传原文件字节，
/// 文件夹就地打成临时 CBZ 后流式回传。
/// 用 POST 路由——局域网下写操作一律需要口令，避免未授权设备整包拉走漫画文件。
pub async fn download_archive_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Response {
    let (archive_path, archive_type) =
        match super::run_db(&state, move |db| db.get_archive(id)).await {
            Ok(Some(a)) => (a.path, a.archive_type),
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
            Err(e) => return internal_error(e),
        };

    // 磁盘上已不存在的档案：明确 404（见 archive_missing_response）
    if !crate::services::archive::archive_exists(&archive_type, &archive_path) {
        return archive_missing_response();
    }

    if crate::services::is_compressed(&archive_type) {
        // 压缩包：直接流式回传原文件（不解包、不重打包），源文件 mtime 随响应头带回
        let source_mtime = archive_mtime_secs(&archive_path);
        match tokio::fs::File::open(&archive_path).await {
            Ok(file) => {
                let base = std::path::Path::new(&archive_path)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("archive_{}.{}", id, archive_type));
                let stream = tokio_util::io::ReaderStream::new(file);
                build_response(
                    StatusCode::OK,
                    vec![
                        ("Content-Type", "application/octet-stream".to_string()),
                        ("Content-Disposition", safe_content_disposition(&base)),
                        ("Cache-Control", "no-store".to_string()),
                        ("X-Source-Mtime", source_mtime.to_string()),
                    ],
                    axum::body::Body::from_stream(stream),
                )
            }
            Err(e) => internal_error(e),
        }
    } else {
        // 文件夹：阻塞打包到临时 CBZ，再流式回传；临时文件延迟清理
        let folder_path = archive_path.clone();
        // 目录本身 mtime（秒）随响应头带回，本机下载后据此恢复
        let source_mtime = archive_mtime_secs(&archive_path);
        // 文件名在移到闭包前先算好
        let base = std::path::Path::new(&folder_path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("archive_{}.cbz", id));
        let filename = format!("{}.cbz", base.trim_end_matches('/'));
        let sync_tmp = state.data_dir.join("sync_tmp");
        let packed = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            std::fs::create_dir_all(&sync_tmp)?;
            let tmp = crate::services::cbz::pack_folder_to_tempfile(&folder_path)?;
            // keep() 转成持久路径（不 drop 自动删除），由下面的延迟清理负责
            let (_, path) = tmp.keep().map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Ok(path.to_string_lossy().into_owned())
        })
        .await;

        match packed {
            Ok(Ok(temp_path)) => {
                match tokio::fs::File::open(&temp_path).await {
                    Ok(file) => {
                        let stream = tokio_util::io::ReaderStream::new(file);
                        // 回传完成后延迟清理临时文件（客户端可能中断，无法在流结束回调里删除）
                        let cleanup = temp_path.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
                            let _ = tokio::fs::remove_file(&cleanup).await;
                        });
                        build_response(
                            StatusCode::OK,
                            vec![
                                ("Content-Type", "application/octet-stream".to_string()),
                                ("Content-Disposition", safe_content_disposition(&filename)),
                                ("Cache-Control", "no-store".to_string()),
                                ("X-Source-Mtime", source_mtime.to_string()),
                            ],
                            axum::body::Body::from_stream(stream),
                        )
                    }
                    Err(e) => internal_error(e),
                }
            }
            Ok(Err(e)) => internal_error(e),
            Err(e) => internal_error(e),
        }
    }
}

/// 用于 Content-Disposition filename 的安全转义：拒绝引号、反斜杠与控制字符。
fn safe_content_disposition(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '"' | '\\') {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("attachment; filename=\"{}\"", clean)
}

pub async fn open_file(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OpenFileRequest>,
) -> Response {
    let file_path = payload.file_path.trim().to_string();
    // 入参卫生：拒绝空串/含 NUL/非绝对路径，避免把任意输入喂给文件系统或外部工具
    if file_path.is_empty() || file_path.contains('\0') {
        return error_response(StatusCode::BAD_REQUEST, "无效的文件路径");
    }
    if !std::path::Path::new(&file_path).is_absolute() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "路径必须是绝对路径（桌面端由文件选择器返回）",
        );
    }

    // Check DB first (quick operation)
    let existing = super::run_db(&state, {
        let file_path = file_path.clone();
        move |db| db.get_archive_by_path(&file_path)
    })
    .await;
    if let Ok(Some(existing)) = existing {
        return Json(serde_json::json!({
            "id": existing.id,
            "message": "文件已存在于库中"
        }))
        .into_response();
    }

    // Detect archive type (fast string check)
    let scanner = crate::services::scanner::Scanner::new();
    let archive_type = scanner.detect_archive_type(&file_path);

    if archive_type == "unknown" {
        return error_response(StatusCode::BAD_REQUEST, "Unsupported file type");
    }

    // Read title depth setting (quick DB operation)
    let title_depth = super::run_db(&state, move |db| {
        Ok(db
            .get_setting("title_depth")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1))
    })
    .await
    .unwrap_or(1);

    // Do blocking I/O in spawn_blocking
    let archive_type_clone = archive_type.clone();
    let file_path_for_insert = file_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&file_path);
        if !path.exists() {
            anyhow::bail!("File not found");
        }

        let title = crate::services::scanner::derive_title(path, title_depth);

        let file_size = std::fs::metadata(&file_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        let file_mtime = archive_mtime_secs(&file_path);

        let page_count = match crate::services::archive::create_archive_reader(
            &file_path,
            &archive_type_clone,
        ) {
            Ok(reader) => reader.list_pages().map(|p| p.len() as i64).unwrap_or(0),
            Err(_) => 0,
        };

        Ok((title, file_size, page_count, file_mtime))
    })
    .await;

    match result {
        Ok(Ok((title, file_size, page_count, file_mtime))) => {
            if page_count == 0 {
                let msg = if archive_type == "folder" {
                    "文件夹中没有找到图片文件"
                } else {
                    "压缩包中没有图片"
                };
                return error_response(StatusCode::BAD_REQUEST, msg);
            }

            let result = super::run_db(&state, {
                let file_path = file_path_for_insert.clone();
                let title = title.clone();
                let archive_type = archive_type.clone();
                move |db| {
                    db.upsert_scanned_archive(
                        &title,
                        &file_path,
                        &archive_type,
                        page_count,
                        file_size,
                        file_mtime,
                    )
                }
            })
            .await;

            match result {
                Ok(id) => Json(serde_json::json!({
                    "id": id,
                    "title": title,
                    "archive_type": archive_type,
                }))
                .into_response(),
                Err(e) => internal_error(e),
            }
        }
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                error_response(StatusCode::NOT_FOUND, &msg)
            } else {
                internal_error(msg)
            }
        }
        Err(e) => internal_error(e),
    }
}

pub async fn scan(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ScanRequest>,
) -> Response {
    let (root_dir, depth) = match super::run_db(&state, move |db| {
        let root_dir = if let Some(p) = payload.path {
            p
        } else {
            db.get_setting("root_dir").unwrap_or_default()
        };

        if root_dir.is_empty() {
            return Ok(None);
        }

        let depth = payload.depth.unwrap_or_else(|| {
            db.get_setting("scan_depth")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
        });

        Ok::<Option<(String, u32)>, rusqlite::Error>(Some((root_dir, depth)))
    })
    .await
    {
        Ok(Some(v)) => v,
        Ok(None) => return error_response(StatusCode::BAD_REQUEST, "No root directory configured"),
        Err(e) => return internal_error(e),
    };

    // 增量扫描 + 孤儿清理，全部在阻塞线程执行
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
        let scanner = crate::services::scanner::Scanner::new();
        let discovered = scanner.scan_directory(&root_dir, depth)?;
        let present: std::collections::HashSet<String> = discovered.iter().cloned().collect();

        // 快照：本 root 下已入库档案的 (page_count, file_size, file_mtime)
        let meta = db.scan_meta_for_root(&root_dir)?;
        let meta_by_path: std::collections::HashMap<&str, (i64, i64, i64)> = meta
            .iter()
            .map(|(p, pc, fs, fm)| (p.as_str(), (*pc, *fs, *fm)))
            .collect();

        let mut added = 0usize;
        let mut updated = 0usize;
        let mut upserts: Vec<(String, String, String, i64, i64, i64)> = Vec::new();

        for archive_path in &discovered {
            let archive_type = scanner.detect_archive_type(archive_path);
            let path = std::path::Path::new(archive_path);

            let file_size = std::fs::metadata(archive_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            let file_mtime = archive_mtime_secs(archive_path);
            let existing = meta_by_path.get(archive_path.as_str()).copied();

            // 文件签名（mtime+size）与已入库一致且已有页数 → 完全跳过，不再开包数页
            if let Some((pc, fs, fm)) = existing {
                if fm == file_mtime && fs == file_size && pc > 0 {
                    continue;
                }
                updated += 1;
            } else {
                added += 1;
            }

            // 仅在需要时重新数页（签名变化 / 新增 / 此前未数到页数）
            let page_count = match existing {
                Some((pc, fs, fm)) if fm == file_mtime && fs == file_size && pc > 0 => pc,
                _ => crate::services::archive::create_archive_reader(archive_path, &archive_type)
                    .ok()
                    .and_then(|r| r.list_pages().ok())
                    .map(|p| p.len() as i64)
                    .unwrap_or(0),
            };

            let title = {
                let relative = path.strip_prefix(&root_dir).unwrap_or(path);
                let first = relative.components().next();
                match first {
                    Some(std::path::Component::Normal(name)) => {
                        let s = name.to_string_lossy().to_string();
                        if path.is_file() {
                            std::path::Path::new(&s)
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string()
                        } else {
                            s
                        }
                    }
                    _ => path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                }
            };

            upserts.push((
                title,
                archive_path.clone(),
                archive_type,
                page_count,
                file_size,
                file_mtime,
            ));
        }

        // 单事务批量写入，避免每个档案单独获取连接 + SELECT id
        db.batch_upsert_scanned_archives(&upserts)?;

        // 清理孤儿档案：本 root 下、磁盘上确实已不存在的路径（只清本 root，不影响其它根）。
        // 磁盘上仍存在但本次扫描未发现的路径（深度限制、扩展名白名单外、无图片文件夹、
        // 路径字符串形态差异等）一律跳过，避免误删手动打开或位于扫描盲区的档案及其标签/历史。
        let mut removed = 0usize;
        let mut skipped = 0usize;
        for (row_path, _pc, _fs, _fm) in meta {
            if present.contains(&row_path) {
                continue; // 本次扫描已发现，保留
            }
            if std::path::Path::new(&row_path).exists() {
                tracing::info!(
                    "Scan cleanup: keeping {} (exists on disk but not discovered this scan)",
                    row_path
                );
                skipped += 1;
                continue;
            }
            tracing::info!("Scan cleanup: removing orphan archive {}", row_path);
            db.delete_archive_by_path(&row_path)?;
            removed += 1;
        }

        Ok(serde_json::json!({
            "scanned": discovered.len(),
            "added": added,
            "updated": updated,
            "removed": removed,
            "skipped": skipped,
            "message": format!(
                "扫描完成：共 {} 个档案，新增 {}，更新 {}，清理 {} 个已删除档案，跳过 {} 个仍存在的档案",
                discovered.len(),
                added,
                updated,
                removed,
                skipped
            )
        }))
    })
    .await;

    match result {
        Ok(Ok(body)) => Json(body).into_response(),
        Ok(Err(e)) => internal_error(e),
        Err(e) => internal_error(e),
    }
}

/// 按当前「初始标题层级」设置，批量重生成自动派生标题（跳过已手动改名的档案）
pub async fn regenerate_titles(State(state): State<Arc<AppState>>) -> Response {
    let title_depth = super::run_db(&state, move |db| {
        Ok(db
            .get_setting("title_depth")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1))
    })
    .await
    .unwrap_or(1);

    let result = super::run_db(&state, move |db| {
        let rows = db.list_auto_titled()?;
        // 批量计算标题，单事务一次写入（旧实现逐行独立取连接 + prepare）
        let entries: Vec<(i64, String)> = rows
            .into_iter()
            .map(|(id, path)| {
                let new_title = crate::services::scanner::derive_title(
                    std::path::Path::new(&path),
                    title_depth,
                );
                (id, new_title)
            })
            .collect();
        db.update_titles_auto(&entries)
    })
    .await;

    match result {
        Ok(changed) => Json(serde_json::json!({
            "success": true,
            "updated": changed,
            "message": format!("已按当前层级重新生成 {} 个标题", changed),
        }))
        .into_response(),
        Err(e) => internal_error(e),
    }
}

/// 将文件夹打包为 CBZ 归档文件
pub async fn pack_cbz(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PackCbzRequest>,
) -> Response {
    let folder_path = payload.folder_path.clone();

    // 确定输出目录：优先使用请求参数，否则从设置中读取
    let output_dir = match super::run_db(&state, move |db| {
        if let Some(ref dir) = payload.output_dir {
            if !dir.is_empty() {
                return Ok::<Option<String>, rusqlite::Error>(Some(dir.clone()));
            }
        }
        match db.get_setting("cbz_export_dir") {
            Ok(dir) if !dir.is_empty() => Ok(Some(dir)),
            _ => Ok(None),
        }
    })
    .await
    {
        Ok(Some(dir)) => dir,
        Ok(None) => {
            return error_response(StatusCode::BAD_REQUEST, "请先在设置中配置 CBZ 归档目录")
        }
        Err(e) => return internal_error(e),
    };

    // 在独立线程中执行 CPU/IO 密集型打包任务
    let result = tokio::task::spawn_blocking(move || {
        crate::services::cbz::pack_folder_to_cbz(&folder_path, &output_dir)
    })
    .await;

    match result {
        Ok(Ok(cbz_path)) => Json(serde_json::json!({
            "success": true,
            "cbz_path": cbz_path,
            "message": format!("归档成功: {}", cbz_path),
        }))
        .into_response(),
        Ok(Err(e)) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
        Err(e) => internal_error(e),
    }
}

/// 列出 CBZ 导出目录中的所有 .cbz 文件
pub async fn list_cbz_files(State(state): State<Arc<AppState>>) -> Response {
    let export_dir = match super::run_db(&state, move |db| db.get_setting("cbz_export_dir")).await {
        Ok(dir) if !dir.is_empty() => dir,
        _ => return Json(serde_json::json!([])).into_response(),
    };

    let dir = std::path::PathBuf::from(&export_dir);
    if !dir.exists() || !dir.is_dir() {
        return Json(serde_json::json!([])).into_response();
    }

    // 目录枚举 + 每文件 stat 是阻塞 I/O，放到 spawn_blocking 避免占满 tokio worker
    let mut files = tokio::task::spawn_blocking(move || -> Vec<serde_json::Value> {
        std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().is_file()
                    && e.path()
                        .extension()
                        .map(|ext| ext == "cbz")
                        .unwrap_or(false)
            })
            .filter_map(|e| {
                let metadata = e.metadata().ok()?;
                let mtime = crate::services::fs_ext::mtime_secs(&e.path());
                Some(serde_json::json!({
                    "name": e.file_name().to_string_lossy(),
                    "path": e.path().to_string_lossy(),
                    "size": metadata.len(),
                    "modified": mtime,
                }))
            })
            .collect()
    })
    .await
    .unwrap_or_default();

    files.sort_by(|a, b| {
        let ma = a["modified"].as_u64().unwrap_or(0);
        let mb = b["modified"].as_u64().unwrap_or(0);
        mb.cmp(&ma)
    });

    Json(files).into_response()
}

#[derive(Deserialize)]
pub struct BookmarkRequest {
    pub page_index: i64,
}

/// 列出某档案的全部书签页码。
pub async fn list_bookmarks(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    match super::run_db(&state, move |db| db.list_bookmarks(id)).await {
        Ok(pages) => Json(serde_json::json!({ "archive_id": id, "pages": pages })).into_response(),
        Err(e) => internal_error(e),
    }
}

/// 在当前页添加书签（同一页重复添加幂等）。
pub async fn add_bookmark(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<BookmarkRequest>,
) -> Response {
    if payload.page_index < 0 {
        return error_response(StatusCode::BAD_REQUEST, "page_index 不能为负");
    }
    match super::run_db(&state, move |db| db.add_bookmark(id, payload.page_index)).await {
        Ok(_) => Json(serde_json::json!({ "success": true, "page_index": payload.page_index }))
            .into_response(),
        Err(e) => internal_error(e),
    }
}

/// 移除书签。
pub async fn remove_bookmark(
    State(state): State<Arc<AppState>>,
    Path((id, page_index)): Path<(i64, i64)>,
) -> Response {
    match super::run_db(&state, move |db| db.remove_bookmark(id, page_index)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}

#[derive(Deserialize)]
pub struct SetCoverRequest {
    pub page_index: Option<i64>,
}

/// 设置/清除手动封面：page_index=Some(i) 用第 i 页作封面，None 恢复默认（首页）。
pub async fn set_archive_cover(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<SetCoverRequest>,
) -> Response {
    let archive_row = match super::run_db(&state, move |db| db.get_archive(id)).await {
        Ok(Some(a)) => a,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "Archive not found"),
        Err(e) => return internal_error(e),
    };

    // 磁盘上已不存在的档案：明确 404（见 archive_missing_response）
    if !crate::services::archive::archive_exists(&archive_row.archive_type, &archive_row.path) {
        return archive_missing_response();
    }

    let page_name = if let Some(idx) = payload.page_index {
        if idx < 0 {
            return error_response(StatusCode::BAD_REQUEST, "page_index 不能为负");
        }
        let path = archive_row.path.clone();
        let atype = archive_row.archive_type.clone();
        let db = state.db.clone();
        let mtime_secs = archive_mtime_secs(&path);
        let name = tokio::task::spawn_blocking(move || {
            let pages =
                crate::services::page_cache::load_page_rows(&db, id, &path, &atype, mtime_secs)?;
            let i = idx as usize;
            if i >= pages.len() {
                anyhow::bail!("Page index {} out of range (total: {})", i, pages.len());
            }
            Ok::<String, anyhow::Error>(pages[i].filepath.clone())
        })
        .await
        .map_err(|e| format!("task error: {}", e));
        match name {
            Ok(Ok(n)) => Some(n),
            Ok(Err(e)) => {
                let msg = e.to_string();
                return if msg.contains("out of range") {
                    error_response(StatusCode::BAD_REQUEST, &msg)
                } else {
                    internal_error(msg)
                };
            }
            Err(e) => return internal_error(e),
        }
    } else {
        None
    };

    let cover_for_db = page_name.clone();
    let upd = super::run_db(&state, move |db| {
        db.set_archive_cover(id, cover_for_db.as_deref())
    })
    .await;
    match upd {
        Ok(_) => {
            // 清掉旧封面缩略图，强制下次按新封面重新生成（浏览器的 ETag 也随覆写变化）
            let thumb_dir = state.data_dir.join("thumbnails").join(id.to_string());
            let _ = tokio::fs::remove_file(thumb_dir.join("cover.jpg")).await;
            Json(serde_json::json!({ "success": true, "cover_image": page_name })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

#[derive(Deserialize)]
pub struct RemoteCoverRequest {
    pub url: Option<String>,
}

/// 设置/清除远程封面 URL（http/https）。设置时清掉旧下载缓存，下次访问封面会重新拉取。
pub async fn set_remote_cover_url(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<RemoteCoverRequest>,
) -> Response {
    let url = payload
        .url
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    if let Some(u) = &url {
        if !super::validate_outbound_url(u) {
            return error_response(
                StatusCode::BAD_REQUEST,
                "仅支持 http(s) 的局域网/公网图片地址（拒绝本机/回环/链路本地）",
            );
        }
    }

    let db_url = url.clone();
    match super::run_db(&state, move |db| db.set_remote_cover(id, db_url.as_deref())).await {
        Ok(_) => {
            // 失效缓存：缩略图与远程原图缓存都要重新生成/下载
            let thumb_dir = state.data_dir.join("thumbnails").join(id.to_string());
            let _ = tokio::fs::remove_file(thumb_dir.join("cover.jpg")).await;
            let covers_file = state.data_dir.join("covers").join(format!("{}.img", id));
            let _ = tokio::fs::remove_file(&covers_file).await;
            Json(serde_json::json!({ "success": true, "remote_cover": url })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ArchiveRow;

    fn row(id: i64, title: &str, path: &str, group_id: Option<i64>) -> ArchiveRow {
        ArchiveRow {
            id,
            title: title.to_string(),
            path: path.to_string(),
            archive_type: "folder".to_string(),
            page_count: 10,
            cover_image: None,
            file_size: 0,
            thumbnail_path: None,
            group_id,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn group_archives_merges_same_title_and_parent() {
        let items = group_archives(vec![
            row(1, "海贼王", "/manhua/海贼王/01", None),
            row(2, "海贼王", "/manhua/海贼王/02", None),
        ]);
        assert_eq!(items.len(), 1);
        assert!(items[0].is_group);
        assert_eq!(items[0].chapter_count, Some(2));
        assert_eq!(items[0].auto_group, Some(true));
        assert_eq!(items[0].parent_dir.as_deref(), Some("/manhua/海贼王"));
    }

    #[test]
    fn group_archives_keeps_single_archive_as_plain() {
        let items = group_archives(vec![row(1, "海贼王", "/manhua/海贼王/01", None)]);
        assert_eq!(items.len(), 1);
        assert!(!items[0].is_group);
    }

    #[test]
    fn group_archives_does_not_merge_different_parents() {
        let items = group_archives(vec![
            row(1, "海贼王", "/manhua/海贼王/01", None),
            row(2, "海贼王", "/other/海贼王/02", None),
        ]);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| !i.is_group));
    }

    #[test]
    fn group_archives_merges_case_insensitive_titles() {
        let items = group_archives(vec![
            row(1, "One Piece", "/manhua/one-piece/01", None),
            row(2, "one piece", "/manhua/one-piece/02", None),
        ]);
        assert_eq!(items.len(), 1);
        assert!(items[0].is_group);
        assert_eq!(items[0].chapter_count, Some(2));
    }

    #[test]
    fn group_archives_merges_permanent_group() {
        let items = group_archives(vec![
            row(1, "海贼王", "/manhua/海贼王/01", Some(1)),
            row(2, "海贼王", "/manhua/海贼王/02", Some(1)),
        ]);
        assert_eq!(items.len(), 1);
        assert!(items[0].is_group);
        assert_eq!(items[0].chapter_count, Some(2));
        assert_eq!(items[0].auto_group, None);
        assert_eq!(items[0].archive.id, 1);
    }

    /// 随机排序的 key 必须与 (id, seed) 一一确定：同 seed 稳定、跨 seed 变化，
    /// 这样分页/无限滚动才不会重复或漏掉条目。
    #[test]
    fn stable_random_key_is_deterministic_and_seed_sensitive() {
        assert_eq!(stable_random_key(42, 7), stable_random_key(42, 7));
        assert_ne!(stable_random_key(42, 7), stable_random_key(42, 8));
        // 同一 seed 下不同 id 应给出不同 key（避免退化成原顺序）
        assert_ne!(stable_random_key(1, 7), stable_random_key(2, 7));

        // 分页一致性：按该 key 排序后切页，并集恰好覆盖全部且无重复
        let mut ids: Vec<i64> = (1..=50).collect();
        ids.sort_by_key(|id| stable_random_key(*id, 12345));
        let page1: Vec<i64> = ids[..20].to_vec();
        let page2: Vec<i64> = ids[20..40].to_vec();
        let page3: Vec<i64> = ids[40..].to_vec();
        let mut union: Vec<i64> = page1.iter().chain(&page2).chain(&page3).copied().collect();
        union.sort_unstable();
        union.dedup();
        assert_eq!(union, (1..=50).collect::<Vec<i64>>());
    }
}
