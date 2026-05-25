use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;

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

pub async fn root_catalog(
    State(_state): State<Arc<AppState>>,
) -> Html<String> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer</id>
  <title>MangaViewer OPDS</title>
  <updated>2026-01-01T00:00:00Z</updated>
  <link rel="self" href="/opds" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  <entry>
    <title>All Archives</title>
    <link rel="http://opds-spec.org/featured" href="/opds/catalog" type="application/atom+xml"/>
    <id>manhuaviewer-catalog</id>
    <updated>2026-01-01T00:00:00Z</updated>
  </entry>
  <entry>
    <title>Recent Reading</title>
    <link rel="http://opds-spec.org/recent" href="/opds/recent" type="application/atom+xml"/>
    <id>manhuaviewer-recent</id>
    <updated>2026-01-01T00:00:00Z</updated>
  </entry>
  <entry>
    <title>Tags</title>
    <link rel="subsection" href="/opds/tags" type="application/atom+xml"/>
    <id>manhuaviewer-tags</id>
    <updated>2026-01-01T00:00:00Z</updated>
  </entry>
</feed>"#;
    
    Html(xml.to_string())
}

pub async fn catalog(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OpdsQuery>,
) -> Html<String> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    
    let mut entries = String::new();
    
    let mut stmt = conn.prepare(
        "SELECT id, title, path, archive_type, page_count FROM archives ORDER BY updated_at DESC LIMIT ? OFFSET ?"
    ).unwrap();
    
    let archives = stmt.query_map([limit, offset], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    }).unwrap();
    
    for archive in archives.flatten() {
        let (id, title, path, archive_type, page_count) = archive;
        entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/acquisition" href="/opds/archive/{}" type="application/atom+xml"/>
    <id>manhuaviewer-archive-{}</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <content type="text">{} pages - {}</content>
  </entry>"#, xml_escape(&title), id, id, page_count, xml_escape(&archive_type)));
    }
    
    let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-catalog</id>
  <title>All Archives</title>
  <updated>2026-01-01T00:00:00Z</updated>
  <link rel="self" href="/opds/catalog" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, entries);
    
    Html(xml)
}

pub async fn archive_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Html<String> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let archive: Option<(String, i64)> = conn.query_row(
        "SELECT title, page_count FROM archives WHERE id = ?",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?))
    ).ok();
    
    match archive {
        Some((title, page_count)) => {
            let mut entries = String::new();
            
            let mut stmt = conn.prepare(
                "SELECT filename, sort_order FROM pages WHERE archive_id = ? ORDER BY sort_order"
            ).unwrap();
            
            let pages = stmt.query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            }).unwrap();
            
            for page in pages.flatten() {
                let (filename, sort_order) = page;
                entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/image" href="/api/archives/{}/pages/{}" type="image/jpeg"/>
    <id>manhuaviewer-page-{}-{}</id>
    <updated>2026-01-01T00:00:00Z</updated>
  </entry>"#, xml_escape(&filename), id, sort_order, id, sort_order));
            }
            
            let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-archive-{}-pages</id>
  <title>{} ({} pages)</title>
  <updated>2026-01-01T00:00:00Z</updated>
  <link rel="self" href="/opds/archive/{}" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, id, xml_escape(&title), page_count, id, entries);
            
            Html(xml)
        },
        None => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Archive not found</title>
</feed>"#.to_string()),
    }
}

pub async fn recent(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let mut entries = String::new();
    
    let mut stmt = conn.prepare(
        "SELECT h.archive_id, a.title, h.page_index, h.total_pages, h.updated_at
         FROM history h
         JOIN archives a ON a.id = h.archive_id
         ORDER BY h.updated_at DESC LIMIT 20"
    ).unwrap();
    
    let history = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    }).unwrap();
    
    for item in history.flatten() {
        let (archive_id, title, page_index, total_pages, updated_at) = item;
        entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/acquisition" href="/opds/archive/{}" type="application/atom+xml"/>
    <id>manhuaviewer-archive-{}</id>
    <updated>{}</updated>
    <content type="text">Page {} of {}</content>
  </entry>"#, xml_escape(&title), archive_id, archive_id, updated_at, page_index + 1, total_pages));
    }
    
    let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-recent</id>
  <title>Recent Reading</title>
  <updated>2026-01-01T00:00:00Z</updated>
  <link rel="self" href="/opds/recent" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, entries);
    
    Html(xml)
}

pub async fn tags_list(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    let mut entries = String::new();
    
    let mut stmt = conn.prepare(
        "SELECT t.id, t.namespace, t.name, COUNT(at.archive_id) as count
         FROM tags t
         LEFT JOIN archive_tags at ON at.tag_id = t.id
         GROUP BY t.id
         ORDER BY t.namespace, t.name"
    ).unwrap();
    
    let tags = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    }).unwrap();
    
    for tag in tags.flatten() {
        let (id, namespace, name, count) = tag;
        let display_name = if namespace.is_empty() {
            name.clone()
        } else {
            format!("{}:{}", namespace, name)
        };
        entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="subsection" href="/opds/tag/{}" type="application/atom+xml"/>
    <id>manhuaviewer-tag-{}</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <content type="text">{} archives</content>
  </entry>"#, xml_escape(&display_name), id, id, count));
    }
    
    let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-tags</id>
  <title>Tags</title>
  <updated>2026-01-01T00:00:00Z</updated>
  <link rel="self" href="/opds/tags" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, entries);
    
    Html(xml)
}

pub async fn tag_archives(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i64>,
) -> Html<String> {
    let db = state.db.lock().await;
    let conn = db.get_conn();
    
    // Get tag name
    let tag_name: Option<String> = conn.query_row(
        "SELECT name FROM tags WHERE id = ?",
        [tag_id],
        |row| row.get(0)
    ).ok();
    
    let tag_name = tag_name.unwrap_or_else(|| "Unknown".to_string());
    
    let mut entries = String::new();
    
    let mut stmt = conn.prepare(
        "SELECT a.id, a.title, a.archive_type, a.page_count
         FROM archives a
         JOIN archive_tags at ON at.archive_id = a.id
         WHERE at.tag_id = ?
         ORDER BY a.updated_at DESC"
    ).unwrap();
    
    let archives = stmt.query_map([tag_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    }).unwrap();
    
    for archive in archives.flatten() {
        let (id, title, archive_type, page_count) = archive;
        entries.push_str(&format!(r#"
  <entry>
    <title>{}</title>
    <link rel="http://opds-spec.org/acquisition" href="/opds/archive/{}" type="application/atom+xml"/>
    <id>manhuaviewer-archive-{}</id>
    <updated>2026-01-01T00:00:00Z</updated>
    <content type="text">{} pages - {}</content>
  </entry>"#, xml_escape(&title), id, id, page_count, xml_escape(&archive_type)));
    }
    
    let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-tag-{}-archives</id>
  <title>Archives with tag: {}</title>
  <updated>2026-01-01T00:00:00Z</updated>
  <link rel="self" href="/opds/tag/{}" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, tag_id, xml_escape(&tag_name), tag_id, entries);
    
    Html(xml)
}
