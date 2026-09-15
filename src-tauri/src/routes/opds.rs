use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::sync::Arc;

use super::run_db;

/// OPDS 响应必须是 `application/atom+xml`（此前返回 text/html，多数阅读器会解析失败）。
fn opds_response(xml: String) -> Response {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/atom+xml; charset=utf-8",
        )],
        xml,
    )
        .into_response()
}

/// 给 OPDS 报文里所有站内链接（`href="/opds...`、`href="/api/...`）统一追加 `?token=`。
/// 口令模式下客户端会以 XML 里的 href 原样发起后续请求（翻页、页面图片），不加 token
/// 会被 401 拦截，OPDS 阅读器将无法翻页/看图。在最终 XML 上做一次轻量文本改写。
pub(crate) fn append_token_to_links(xml: &str, token: &str) -> String {
    if token.is_empty() {
        return xml.to_string();
    }
    let needle: &str = "href=\"/";
    let qs = format!("?token={}", token);
    let mut out = String::with_capacity(xml.len() + 64);
    let mut rest = xml;
    while let Some(pos) = rest.find(needle) {
        out.push_str(&rest[..pos + needle.len()]);
        rest = &rest[pos + needle.len()..];
        match rest.find('"') {
            Some(end) => {
                out.push_str(&rest[..end]);
                out.push_str(&qs);
                out.push('"');
                rest = &rest[end + 1..];
            }
            None => {
                // 未闭合的引号：原样收尾，避免破坏报文
                out.push_str(rest);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

#[derive(Deserialize)]
pub struct OpdsQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn opds_error_xml(message: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Error</title>
  <id>manhuaviewer-error</id>
  <updated>{}</updated>
  <entry>
    <title>{}</title>
    <content type="text">{}</content>
  </entry>
</feed>"#,
        current_timestamp(),
        xml_escape(message),
        xml_escape(message)
    )
}

/// 标准 OPDS 目录 feed 骨架：id + 标题 + 更新时间 + 自引用链接 + `/opds` 起始链接 + 条目区。
/// 除根目录（含更多入口链接）与错误响应外，所有目录共用这一模板，
/// 改 XML 结构只需动这一处。
fn opds_feed(id: &str, title: &str, self_href: &str, entries: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>{id}</id>
  <title>{title}</title>
  <updated>{}</updated>
  <link rel="self" href="{self_href}" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {entries}
</feed>"#,
        current_timestamp()
    )
}

/// 单条 OPDS 条目：标题 + 一条链接 + id + updated + 可选的 content 文本。
/// 目录类（acquisition）、子目录类（subsection）与页面图片类（image）条目
/// 只是 rel/href/type/content 不同，结构完全一致，统一用一个建造器生成。
fn opds_entry(
    title: &str,
    link_rel: &str,
    link_href: &str,
    link_type: &str,
    entry_id: &str,
    updated: &str,
    content: Option<&str>,
) -> String {
    let content_xml = content
        .map(|c| format!("\n    <content type=\"text\">{}</content>", xml_escape(c)))
        .unwrap_or_default();
    format!(
        r#"
  <entry>
    <title>{}</title>
    <link rel="{}" href="{}" type="{}"/>
    <id>{}</id>
    <updated>{}</updated>{}
  </entry>"#,
        xml_escape(title),
        link_rel,
        link_href,
        link_type,
        entry_id,
        updated,
        content_xml
    )
}

/// acquisition 类条目：指向档案详情，附 "N pages - type" 描述。
fn archive_entry(title: &str, archive_id: i64, updated: &str, content: &str) -> String {
    opds_entry(
        title,
        "http://opds-spec.org/acquisition",
        &format!("/opds/archive/{}", archive_id),
        "application/atom+xml",
        &format!("manhuaviewer-archive-{}", archive_id),
        updated,
        Some(content),
    )
}

/// subsection 类条目：指向标签/分类子目录。
fn subsection_entry(title: &str, entry_id: &str, link_href: &str, updated: &str) -> String {
    opds_entry(
        title,
        "subsection",
        link_href,
        "application/atom+xml",
        entry_id,
        updated,
        None,
    )
}

/// image 类条目：档案详情里的页面，链接类型用页面的真实 MIME。
fn image_entry(filename: &str, image_type: &str, archive_id: i64, page_index: usize) -> String {
    opds_entry(
        filename,
        "http://opds-spec.org/image",
        &format!("/api/archives/{}/pages/{}", archive_id, page_index),
        image_type,
        &format!("manhuaviewer-page-{}-{}", archive_id, page_index),
        &current_timestamp(),
        None,
    )
}

pub async fn root_catalog(State(_state): State<Arc<AppState>>) -> Response {
    let ts = current_timestamp();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer</id>
  <title>MangaViewer OPDS</title>
  <updated>{ts}</updated>
  <link rel="self" href="/opds" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  <entry>
    <title>All Archives</title>
    <link rel="http://opds-spec.org/featured" href="/opds/catalog" type="application/atom+xml"/>
    <id>manhuaviewer-catalog</id>
    <updated>{ts}</updated>
  </entry>
  <entry>
    <title>Recent Reading</title>
    <link rel="http://opds-spec.org/recent" href="/opds/recent" type="application/atom+xml"/>
    <id>manhuaviewer-recent</id>
    <updated>{ts}</updated>
  </entry>
  <entry>
    <title>Tags</title>
    <link rel="subsection" href="/opds/tags" type="application/atom+xml"/>
    <id>manhuaviewer-tags</id>
    <updated>{ts}</updated>
  </entry>
  <entry>
    <title>Categories</title>
    <link rel="subsection" href="/opds/categories" type="application/atom+xml"/>
    <id>manhuaviewer-categories</id>
    <updated>{ts}</updated>
  </entry>
</feed>"#
    );

    opds_response(xml)
}

pub async fn catalog(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OpdsQuery>,
) -> Response {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    match run_db(&state, move |db| {
        db.list_archives(None, None, None, "updated", "desc", limit, offset)
    })
    .await
    {
        Ok(archives) => {
            let entries = archives
                .iter()
                .map(|archive| {
                    archive_entry(
                        &archive.title,
                        archive.id,
                        &archive.updated_at,
                        &format!("{} pages - {}", archive.page_count, archive.archive_type),
                    )
                })
                .collect::<String>();

            opds_response(opds_feed(
                "manhuaviewer-catalog",
                "All Archives",
                "/opds/catalog",
                &entries,
            ))
        }
        Err(_) => opds_response(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading catalog</title>
</feed>"#
                .to_string(),
        ),
    }
}

pub async fn archive_detail(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    let (archive_path, archive_type, archive_title) =
        match run_db(&state, move |db| db.get_archive(id)).await {
            Ok(Some(a)) => (a.path, a.archive_type, a.title),
            Ok(None) => {
                return opds_response(opds_error_xml("Archive not found"));
            }
            Err(e) => {
                tracing::error!("Failed to get archive {}: {}", id, e);
                return opds_response(opds_error_xml("Database error"));
            }
        };

    // 磁盘上已不存在的档案（手动删除）：返回明确错误而非笼统的 “Error loading pages”
    if !crate::services::archive::archive_exists(&archive_type, &archive_path) {
        return opds_response(opds_error_xml("档案文件不存在或已被移动"));
    }

    // 复用 /api/archives/:id/pages 的页表缓存（进程内 + pages 表），
    // 避免每次请求都重开压缩包/重列目录（此前 unrar/7z 每请求起一次子进程）。
    let result = tokio::task::spawn_blocking({
        let db = state.db.clone();
        move || {
            let mtime = crate::routes::archives::archive_mtime_secs(&archive_path);
            crate::services::page_cache::load_page_rows(
                &db,
                id,
                &archive_path,
                &archive_type,
                mtime,
            )
        }
    })
    .await;

    match result {
        Ok(Ok(pages)) => {
            let entries = pages
                .iter()
                .enumerate()
                .map(|(i, page)| {
                    let filename = std::path::Path::new(&page.filepath)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    // 按真实扩展名给出 image type（此前硬编码 image/jpeg，PNG/WebP/AVIF 均错标）
                    let image_type = mime_guess::from_path(&page.filepath)
                        .first()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "application/octet-stream".to_string());
                    image_entry(&filename, &image_type, id, i)
                })
                .collect::<String>();

            let title = format!("{} ({} pages)", archive_title, pages.len());
            opds_response(opds_feed(
                &format!("manhuaviewer-archive-{}-pages", id),
                &title,
                &format!("/opds/archive/{}", id),
                &entries,
            ))
        }
        Ok(Err(e)) => {
            tracing::error!("Failed to list pages for archive {}: {}", id, e);
            opds_response(opds_error_xml("Error loading pages"))
        }
        Err(e) => {
            tracing::error!("Task error for archive {}: {}", id, e);
            opds_response(opds_error_xml("Internal error"))
        }
    }
}

pub async fn recent(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, move |db| db.get_history(None, 20, 0)).await {
        Ok((history, _total)) => {
            let entries = history
                .iter()
                .map(|(h, title, _path, _archive_type)| {
                    archive_entry(
                        title,
                        h.archive_id,
                        &h.updated_at,
                        &format!("Page {} of {}", h.page_index + 1, h.total_pages),
                    )
                })
                .collect::<String>();

            opds_response(opds_feed(
                "manhuaviewer-recent",
                "Recent Reading",
                "/opds/recent",
                &entries,
            ))
        }
        Err(_) => opds_response(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading history</title>
</feed>"#
                .to_string(),
        ),
    }
}

pub async fn tags_list(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.list_tags()).await {
        Ok(tags) => {
            let entries = tags
                .iter()
                .map(|tag| {
                    let display_name = if tag.namespace.is_empty() {
                        tag.name.clone()
                    } else {
                        format!("{}:{}", tag.namespace, tag.name)
                    };
                    subsection_entry(
                        &display_name,
                        &format!("manhuaviewer-tag-{}", tag.id),
                        &format!("/opds/tag/{}", tag.id),
                        &current_timestamp(),
                    )
                })
                .collect::<String>();

            opds_response(opds_feed(
                "manhuaviewer-tags",
                "Tags",
                "/opds/tags",
                &entries,
            ))
        }
        Err(_) => opds_response(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading tags</title>
</feed>"#
                .to_string(),
        ),
    }
}

pub async fn tag_archives(State(state): State<Arc<AppState>>, Path(tag_id): Path<i64>) -> Response {
    // Get archives with this tag using a single JOIN query
    let result = run_db(&state, move |db| {
        let tag_name = db
            .get_tag_name(tag_id)?
            .unwrap_or_else(|| "Unknown".to_string());
        let archives = db.list_archives_by_tag(tag_id, 100, 0)?;
        Ok((tag_name, archives))
    })
    .await;

    match result {
        Ok((tag_name, archives)) => {
            let entries = archives
                .iter()
                .map(|archive| {
                    archive_entry(
                        &archive.title,
                        archive.id,
                        &archive.updated_at,
                        &format!("{} pages - {}", archive.page_count, archive.archive_type),
                    )
                })
                .collect::<String>();

            opds_response(opds_feed(
                &format!("manhuaviewer-tag-{}-archives", tag_id),
                &format!("Archives with tag: {}", tag_name),
                &format!("/opds/tag/{}", tag_id),
                &entries,
            ))
        }
        Err(e) => {
            tracing::error!("Failed to list archives for tag {}: {}", tag_id, e);
            opds_response(opds_error_xml("Database error"))
        }
    }
}

pub async fn categories_list(State(state): State<Arc<AppState>>) -> Response {
    match run_db(&state, |db| db.list_categories()).await {
        Ok(categories) => {
            let entries = categories
                .iter()
                .map(|category| {
                    subsection_entry(
                        &category.name,
                        &format!("manhuaviewer-category-{}", category.id),
                        &format!("/opds/category/{}", category.id),
                        &category.created_at,
                    )
                })
                .collect::<String>();

            opds_response(opds_feed(
                "manhuaviewer-categories",
                "Categories",
                "/opds/categories",
                &entries,
            ))
        }
        Err(_) => opds_response(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading categories</title>
</feed>"#
                .to_string(),
        ),
    }
}

/// 分类下的档案列表（静态分类按关联表、动态分类按标题匹配，语义与库内一致）。
pub async fn category_archives(
    State(state): State<Arc<AppState>>,
    Path(category_id): Path<i64>,
) -> Response {
    let result = run_db(&state, move |db| {
        let category_name = db.get_category_name(category_id)?.unwrap_or_default();
        let archives =
            db.list_archives(None, None, Some(category_id), "updated", "desc", 200, 0)?;
        Ok((category_name, archives))
    })
    .await;

    match result {
        Ok((category_name, archives)) => {
            if category_name.is_empty() {
                return opds_response(opds_error_xml("Category not found"));
            }
            let entries = archives
                .iter()
                .map(|archive| {
                    archive_entry(
                        &archive.title,
                        archive.id,
                        &archive.updated_at,
                        &format!("{} pages - {}", archive.page_count, archive.archive_type),
                    )
                })
                .collect::<String>();

            opds_response(opds_feed(
                &format!("manhuaviewer-category-{}-archives", category_id),
                &format!("Category: {}", category_name),
                &format!("/opds/category/{}", category_id),
                &entries,
            ))
        }
        Err(e) => {
            tracing::error!(
                "Failed to list archives for category {}: {}",
                category_id,
                e
            );
            opds_response(opds_error_xml("Database error"))
        }
    }
}
