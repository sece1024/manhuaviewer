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

fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub async fn root_catalog(
    State(_state): State<Arc<AppState>>,
) -> Html<String> {
    let ts = current_timestamp();
    let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
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
</feed>"#);
    
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
    
    match db.list_archives(None, "updated", "desc", limit, offset) {
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
            
            Html(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-catalog</id>
  <title>All Archives</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/catalog" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, current_timestamp(), entries))
        },
        Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading catalog</title>
</feed>"#.to_string()),
    }
}

pub async fn archive_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Html<String> {
    let db = state.db.lock().await;
    
    match db.get_archive(id) {
        Ok(Some(archive)) => {
            match crate::services::archive::create_archive_reader(&archive.path, &archive.archive_type) {
                Ok(reader) => {
                    match reader.list_pages() {
                        Ok(pages) => {
                            let mut entries = String::new();
                            
                            for (i, page_name) in pages.iter().enumerate() {
                                let filename = std::path::Path::new(page_name)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();
                                
                                entries.push_str(&format!(r#"
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
                            
                            Html(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-archive-{}-pages</id>
  <title>{} ({} pages)</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/archive/{}" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, id, xml_escape(&archive.title), pages.len(), current_timestamp(), id, entries))
                        },
                        Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading pages</title>
</feed>"#.to_string()),
                    }
                },
                Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error opening archive</title>
</feed>"#.to_string()),
            }
        },
        Ok(None) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Archive not found</title>
</feed>"#.to_string()),
        Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading archive</title>
</feed>"#.to_string()),
    }
}

pub async fn recent(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
    let db = state.db.lock().await;
    
    match db.get_history() {
        Ok(history) => {
            let mut entries = String::new();
            
            for (h, title, _path) in history.iter().take(20) {
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
            
            Html(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-recent</id>
  <title>Recent Reading</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/recent" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, current_timestamp(), entries))
        },
        Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading history</title>
</feed>"#.to_string()),
    }
}

pub async fn tags_list(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
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
                
                entries.push_str(&format!(r#"
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
            
            Html(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-tags</id>
  <title>Tags</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/tags" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, current_timestamp(), entries))
        },
        Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading tags</title>
</feed>"#.to_string()),
    }
}

pub async fn tag_archives(
    State(state): State<Arc<AppState>>,
    Path(tag_id): Path<i64>,
) -> Html<String> {
    let db = state.db.lock().await;
    
    // Get all archives with this tag
    match db.list_archives(None, "updated", "desc", 100, 0) {
        Ok((archives, _)) => {
            // Filter archives that have the specified tag
            let conn = db.get_conn();
            let mut archive_ids = std::collections::HashSet::new();
            
            let mut stmt = conn.prepare(
                "SELECT archive_id FROM archive_tags WHERE tag_id = ?"
            ).unwrap();
            
            let ids = stmt.query_map([tag_id], |row| row.get::<_, i64>(0)).unwrap();
            for id in ids.flatten() {
                archive_ids.insert(id);
            }
            
            let filtered_archives: Vec<_> = archives.into_iter()
                .filter(|a| archive_ids.contains(&a.id))
                .collect();
            
            let mut entries = String::new();
            
            for archive in filtered_archives {
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
            
            // Get tag name
            let tag_name = conn.query_row(
                "SELECT name FROM tags WHERE id = ?",
                [tag_id],
                |row| row.get::<_, String>(0)
            ).unwrap_or_else(|_| "Unknown".to_string());
            
            Html(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-tag-{}-archives</id>
  <title>Archives with tag: {}</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/tag/{}" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, tag_id, xml_escape(&tag_name), current_timestamp(), tag_id, entries))
        },
        Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading archives</title>
</feed>"#.to_string()),
    }
}

pub async fn categories_list(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
    let db = state.db.lock().await;
    
    match db.list_categories() {
        Ok(categories) => {
            let mut entries = String::new();
            
            for category in categories {
                entries.push_str(&format!(r#"
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
            
            Html(format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:opds="http://opds-spec.org/2010/catalog">
  <id>manhuaviewer-categories</id>
  <title>Categories</title>
  <updated>{}</updated>
  <link rel="self" href="/opds/categories" type="application/atom+xml"/>
  <link rel="start" href="/opds" type="application/atom+xml"/>
  {}
</feed>"#, current_timestamp(), entries))
        },
        Err(_) => Html(r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <id>manhuaviewer-error</id>
  <title>Error loading categories</title>
</feed>"#.to_string()),
    }
}
