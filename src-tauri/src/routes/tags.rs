use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use super::{db_json, error_response, internal_error, run_db};

/// 「档案 → 标签」计数请求的 id 上限：对端可控输入，不设上限时一条请求就能让
/// 后端拼出超长 IN 列表。与浏览会话比对窗口（500）取同一个量级。
const MAX_TAG_COUNT_IDS: usize = 500;

/// 解析逗号分隔的档案 id 列表（去重、丢弃非正整数、截断到上限）。
///
/// 纯函数，便于直接断言「脏输入不会变成 SQL 参数」。
fn parse_archive_ids(raw: &str) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for part in raw.split(',') {
        let Ok(id) = part.trim().parse::<i64>() else {
            continue;
        };
        if id <= 0 {
            continue;
        }
        if seen.insert(id) {
            out.push(id);
            if out.len() >= MAX_TAG_COUNT_IDS {
                break;
            }
        }
    }
    out
}

/// 把「档案 → 标签」聚合成「标签 id → 命中的选中档案数」。
///
/// 批量打标签的 UI 要靠它区分三种状态：全部选中项都有（count == total）、
/// 部分有（0 < count < total）、都没有。判定留给调用方，这里只计数。
/// 同一档案上同一标签不会重复（archive_tags 有唯一约束），所以直接累加即可。
fn count_tags_by_archive(
    tags_by_archive: &std::collections::HashMap<i64, Vec<crate::db::TagRow>>,
) -> std::collections::BTreeMap<i64, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for tags in tags_by_archive.values() {
        for tag in tags {
            *counts.entry(tag.id).or_insert(0usize) += 1;
        }
    }
    counts
}

#[derive(Deserialize)]
pub struct TagCountsQuery {
    /// 逗号分隔的档案 id（例如 `ids=1,2,3`）
    pub ids: Option<String>,
}

/// GET /api/tags/counts?ids=1,2,3 — 每个标签在选中档案里出现了几次。
///
/// 供批量打标签弹窗判断「全部包含 / 部分包含 / 未包含」；没有这个接口就只能对
/// 每个选中项各发一次 GET（选中 200 本 = 200 个请求）。
pub async fn tag_counts(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<TagCountsQuery>,
) -> Response {
    let ids = parse_archive_ids(query.ids.as_deref().unwrap_or(""));
    if ids.is_empty() {
        return Json(serde_json::json!({ "total": 0, "counts": {} })).into_response();
    }
    let total = ids.len();
    match run_db(&state, move |db| db.get_archive_tags_batch(&ids)).await {
        Ok(map) => {
            let counts = count_tags_by_archive(&map);
            // 键转成字符串：JSON 对象的键本来就是字符串，前端不必再猜类型
            let counts: std::collections::BTreeMap<String, usize> = counts
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
            Json(serde_json::json!({ "total": total, "counts": counts })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

#[cfg(test)]
mod counts_tests {
    use super::*;
    use crate::db::TagRow;

    fn tag(id: i64) -> TagRow {
        TagRow {
            id,
            namespace: String::new(),
            name: format!("t{id}"),
            color: "#fff".into(),
            archive_count: 0,
        }
    }

    #[test]
    fn parse_archive_ids_rejects_dirty_input() {
        assert_eq!(parse_archive_ids("1,2,3"), vec![1, 2, 3]);
        assert_eq!(parse_archive_ids(" 1 , 2 "), vec![1, 2]);
        // 非数字、0、负数一律丢弃：它们绝不该变成 SQL 参数
        assert_eq!(parse_archive_ids("1,abc,,2,0,-3,4"), vec![1, 2, 4]);
        assert_eq!(parse_archive_ids(""), Vec::<i64>::new());
        assert_eq!(parse_archive_ids("abc"), Vec::<i64>::new());
        // 去重保留首次出现顺序
        assert_eq!(parse_archive_ids("3,1,3,2"), vec![3, 1, 2]);
    }

    #[test]
    fn parse_archive_ids_is_capped() {
        let raw = (1..=(MAX_TAG_COUNT_IDS as i64 + 50))
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse_archive_ids(&raw).len(), MAX_TAG_COUNT_IDS);
    }

    #[test]
    fn count_tags_by_archive_counts_membership() {
        let mut map = std::collections::HashMap::new();
        map.insert(1, vec![tag(11), tag(12)]);
        map.insert(2, vec![tag(11)]);
        map.insert(3, vec![]);
        let counts = count_tags_by_archive(&map);

        assert_eq!(
            counts.get(&11),
            Some(&2),
            "11 出现在 2 个档案里（= 部分包含）"
        );
        assert_eq!(counts.get(&12), Some(&1));
        assert_eq!(counts.get(&99), None, "没出现过的标签不产生条目");
        assert_eq!(counts.len(), 2);
    }

    #[test]
    fn count_tags_by_archive_handles_empty_input() {
        assert!(count_tags_by_archive(&std::collections::HashMap::new()).is_empty());
    }
}

#[derive(Deserialize)]
pub struct TagQuery {
    pub namespace: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Deserialize)]
pub struct AssignTagRequest {
    pub archive_id: i64,
    pub tag_id: i64,
}

pub async fn list_tags(State(state): State<Arc<AppState>>) -> Response {
    db_json(&state, |db| db.list_tags()).await
}

pub async fn list_namespaces(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.list_namespaces()).await {
        Ok(namespaces) => Json(serde_json::json!({ "data": namespaces })).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn create_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TagQuery>,
) -> Response {
    let namespace = payload.namespace.unwrap_or_default();
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let name = payload.name.clone();
    let namespace_db = namespace.clone();
    let color_db = color.clone();

    match run_db(&state, move |db| {
        db.create_tag(&namespace_db, &name, &color_db)
    })
    .await
    {
        Ok(id) => Json(serde_json::json!({
            "data": {
                "id": id,
                "namespace": namespace,
                "name": payload.name,
                "color": color
            }
        }))
        .into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn update_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<TagQuery>,
) -> Response {
    let namespace = payload.namespace.unwrap_or_default();
    let color = payload.color.unwrap_or_else(|| "#4a86e8".to_string());
    let name = payload.name.clone();
    let namespace_db = namespace.clone();
    let color_db = color.clone();

    match run_db(&state, move |db| {
        db.update_tag(id, &namespace_db, &name, &color_db)
    })
    .await
    {
        Ok(_) => Json(serde_json::json!({
            "data": {
                "id": id,
                "namespace": namespace,
                "name": payload.name,
                "color": color
            }
        }))
        .into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn delete_tag(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    match run_db(&state, move |db| db.delete_tag(id)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn assign_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AssignTagRequest>,
) -> Response {
    match run_db(&state, move |db| {
        db.assign_tag(payload.archive_id, payload.tag_id)
    })
    .await
    {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}

pub async fn remove_tag(
    State(state): State<Arc<AppState>>,
    Path((archive_id, tag_id)): Path<(i64, i64)>,
) -> Response {
    match run_db(&state, move |db| db.remove_tag(archive_id, tag_id)).await {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => internal_error(e),
    }
}

#[derive(Deserialize)]
pub struct BatchAssignTagRequest {
    pub archive_ids: Vec<i64>,
    pub tag_id: i64,
}

pub async fn batch_assign_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BatchAssignTagRequest>,
) -> Response {
    if payload.archive_ids.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "archive_ids 不能为空");
    }

    match run_db(&state, move |db| {
        db.batch_assign_tag(&payload.archive_ids, payload.tag_id)
    })
    .await
    {
        Ok(affected) => {
            Json(serde_json::json!({ "success": true, "affected": affected })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

pub async fn batch_remove_tag(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BatchAssignTagRequest>,
) -> Response {
    if payload.archive_ids.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "archive_ids 不能为空");
    }

    match run_db(&state, move |db| {
        db.batch_remove_tag(&payload.archive_ids, payload.tag_id)
    })
    .await
    {
        Ok(affected) => {
            Json(serde_json::json!({ "success": true, "affected": affected })).into_response()
        }
        Err(e) => internal_error(e),
    }
}

pub async fn get_archive_tags(
    State(state): State<Arc<AppState>>,
    Path(archive_id): Path<i64>,
) -> Response {
    db_json(&state, move |db| db.get_archive_tags(archive_id)).await
}
