use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Archive {
    pub id: i64,
    pub title: String,
    pub path: String,
    pub archive_type: String,
    pub page_count: i64,
    pub cover_image: Option<String>,
    pub file_size: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Page {
    pub id: i64,
    pub archive_id: i64,
    pub filename: String,
    pub filepath: String,
    pub sort_order: i64,
    pub width: i64,
    pub height: i64,
    pub file_size: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tag {
    pub id: i64,
    pub namespace: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub pinned: bool,
    pub search: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct History {
    pub archive_id: i64,
    pub page_index: i64,
    pub total_pages: i64,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveWithTags {
    #[serde(flatten)]
    pub archive: Archive,
    pub tags: Vec<Tag>,
    pub categories: Vec<Category>,
    pub history: Option<History>,
}
