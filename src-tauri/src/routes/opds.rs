use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use serde::Deserialize;
use std::sync::Arc;

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

pub async fn root_catalog(State(_state): State<Arc<AppState>>) -> Html<String> {
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

    Html(xml)
}

pub async fn catalog(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OpdsQuery>,
) -> Html<String> {
    let db = state.db.lock().await;

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    match db.list_archives(None, None, "updated", "desc", limit, offset) {
        Ok((archives, _total)) => {
            let mut entries = String::new();

            for archive in archives {
                entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/acquisition" href="/opds/archive/{}" type="application/atom+xml"/>
    <id>manhuaviewer-archive-{}</id>
    <updated>{}</updated>
    <content type="text">{} pages - {}</content>
  </entry>"#,
                    xml_escape(&archive.title),
                    archive.id,
                    archive.id,
                    archive.updated_at,
                    archive.page_count,
                    xml_escape(&archive.archive_type)
                ));
            }

            Html(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-catalog</id>
  <title>All Archives</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/catalog" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#,
                current_timestamp(),
                entries
            ))
        }
        Err(_) => Html(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading catalog</title>
</feed>"#
                .to_string(),
        ),
    }
}

pub async fn archive_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Html<String> {
    let (archive_path, archive_type, archive_title) = {
        let db = state.db.lock().await;
        match db.get_archive(id) {
            Ok(Some(a)) => (a.path, a.archive_type, a.title),
            Ok(None) => {
                return Html(opds_error_xml("Archive not found"));
            }
            Err(e) => {
                tracing::error!("Failed to get archive {}: {}", id, e);
                return Html(opds_error_xml("Database error"));
            }
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        let reader = crate::services::archive::create_archive_reader(&archive_path, &archive_type)?;
        reader.list_pages()
    })
    .await;

    match result {
        Ok(Ok(pages)) => {
            let mut entries = String::new();

            for (i, page_name) in pages.iter().enumerate() {
                let filename = std::path::Path::new(page_name)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();

                entries.push_str(&format!(
                    r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/image" href="/api/archives/{}/pages/{}" type="image/jpeg"/>
    <id>manhuaviewer-page-{}-{}</id>
    <updated>{}</updated>
  </entry>"#,
                    xml_escape(&filename),
                    id,
                    i,
                    id,
                    i,
                    current_timestamp()
                ));
            }

            Html(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-archive-{}-pages</id>
  <title>{} ({} pages)</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/archive/{}" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#,
                id,
                xml_escape(&archive_title),
                pages.len(),
                current_timestamp(),
                id,
                entries
            ))
        }
        Ok(Err(e)) => {
            tracing::error!("Failed to list pages for archive {}: {}", id, e);
            Html(opds_error_xml("Error loading pages"))
        }
        Err(e) => {
            tracing::error!("Task error for archive {}: {}", id, e);
            Html(opds_error_xml("Internal error"))
        }
    }
}

pub async fn recent(State(state): State<Arc<AppState>>) -> Html<String> {
    let db = state.db.lock().await;

    match db.get_history() {
        Ok(history) => {
            let mut entries = String::new();

            for (h, title, _path, _archive_type) in history.iter().take(20) {
                entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/acquisition" href="/opds/archive/{}" type="application/atom+xml"/>
    <id>manhuaviewer-archive-{}</id>
    <updated>{}</updated>
    <content type="text">Page {} of {}</content>
  </entry>"#,
                    xml_escape(title),
                    h.archive_id,
                    h.archive_id,
                    h.updated_at,
                    h.page_index + 1,
                    h.total_pages
                ));
            }

            Html(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-recent</id>
  <title>Recent Reading</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/recent" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#,
                current_timestamp(),
                entries
            ))
        }
        Err(_) => Html(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading history</title>
</feed>"#
                .to_string(),
        ),
    }
}

pub async fn tags_list(State(state): State<Arc<AppState>>) -> Html<String> {
    let db = state.db.lock().await;

    match db.list_tags() {
        Ok(tags) => {
            let mut entries = String::new();

            for tag in tags {
                let display_name = if tag.namespace.is_empty() {
                    tag.name.clone()
                } else {
                    format!("{}:{}", tag.namespace, tag.name)
                };

                entries.push_str(&format!(
                    r#"
  <entry>
    <title>{}</title>
    <link rel="subsection" href="/opds/tag/{}" type="application/atom+xml"/>
    <id>manhuaviewer-tag-{}</id>
    <updated>{}</updated>
  </entry>"#,
                    xml_escape(&display_name),
                    tag.id,
                    tag.id,
                    current_timestamp()
                ));
            }

            Html(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-tags</id>
  <title>Tags</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/tags" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#,
                current_timestamp(),
                entries
            ))
        }
        Err(_) => Html(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading tags</title>
</feed>"#
                .to_string(),
        ),
    }
}

pub async fn tag_archives(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i64>,
) -> Html<String> {
    let db = state.db.lock().await;

    // Get archives with this tag using a single JOIN query
    let tag_name =
        match db
            .get_conn()
            .query_row("SELECT name FROM tags WHERE id = ?", [tag_id], |row| {
                row.get::<_, String>(0)
            }) {
            Ok(name) => name,
            Err(_) => "Unknown".to_string(),
        };

    match db.list_archives_by_tag(tag_id, 100, 0) {
        Ok(archives) => {
            let mut entries = String::new();

            for archive in archives {
                entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/acquisition" href="/opds/archive/{}" type="application/atom+xml"/>
    <id>manhuaviewer-archive-{}</id>
    <updated>{}</updated>
    <content type="text">{} pages - {}</content>
  </entry>"#,
                    xml_escape(&archive.title),
                    archive.id,
                    archive.id,
                    archive.updated_at,
                    archive.page_count,
                    xml_escape(&archive.archive_type)
                ));
            }

            Html(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-tag-{}-archives</id>
  <title>Archives with tag: {}</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/tag/{}" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#,
                tag_id,
                xml_escape(&tag_name),
                current_timestamp(),
                tag_id,
                entries
            ))
        }
        Err(e) => {
            tracing::error!("Failed to list archives for tag {}: {}", tag_id, e);
            Html(opds_error_xml("Database error"))
        }
    }
}

pub async fn categories_list(State(state): State<Arc<AppState>>) -> Html<String> {
    let db = state.db.lock().await;

    match db.list_categories() {
        Ok(categories) => {
            let mut entries = String::new();

            for category in categories {
                entries.push_str(&format!(
                    r#"
  <entry>
    <title>{}</title>
    <link rel="subsection" href="/opds/category/{}" type="application/atom+xml"/>
    <id>manhuaviewer-category-{}</id>
    <updated>{}</updated>
  </entry>"#,
                    xml_escape(&category.name),
                    category.id,
                    category.id,
                    category.created_at
                ));
            }

            Html(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-categories</id>
  <title>Categories</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/categories" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#,
                current_timestamp(),
                entries
            ))
        }
        Err(_) => Html(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading categories</title>
</feed>"#
                .to_string(),
        ),
    }
}
