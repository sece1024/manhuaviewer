pub const SCHEMA: &str = r#"
-- 漫画档案（文件夹或压缩包）
CREATE TABLE IF NOT EXISTS archives (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    archive_type TEXT NOT NULL DEFAULT 'folder',  -- folder | zip | rar | cbz | cbr | 7z
    page_count INTEGER DEFAULT 0,
    cover_image TEXT,           -- 封面图片路径（相对于档案）
    file_size INTEGER DEFAULT 0,
    thumbnail_path TEXT,        -- 缩略图目录路径（thumbnails/{id}/）
    group_id INTEGER REFERENCES archives(id) ON DELETE SET NULL,  -- 合并组主档案 id
    page_list_mtime INTEGER DEFAULT 0,  -- 缓存页面列表时的档案文件 mtime（秒），用于失效检测
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- 页面（仅压缩包需要，文件夹类型实时扫描）
CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    archive_id INTEGER NOT NULL,
    filename TEXT NOT NULL,
    filepath TEXT NOT NULL,     -- 压缩包内路径
    sort_order INTEGER NOT NULL,
    width INTEGER DEFAULT 0,
    height INTEGER DEFAULT 0,
    file_size INTEGER DEFAULT 0,
    FOREIGN KEY (archive_id) REFERENCES archives(id) ON DELETE CASCADE
);

-- 标签（支持命名空间，如 artist:xxx）
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    namespace TEXT DEFAULT '',
    name TEXT NOT NULL,
    color TEXT DEFAULT '#4a86e8',
    UNIQUE(namespace, name)
);

-- 分类
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT DEFAULT '#4a86e8',
    pinned INTEGER DEFAULT 0,
    search TEXT DEFAULT '',     -- 动态分类的搜索表达式
    created_at TEXT DEFAULT (datetime('now'))
);

-- 档案-标签 关联
CREATE TABLE IF NOT EXISTS archive_tags (
    archive_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (archive_id, tag_id),
    FOREIGN KEY (archive_id) REFERENCES archives(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- 档案-分类 关联
CREATE TABLE IF NOT EXISTS archive_categories (
    archive_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    PRIMARY KEY (archive_id, category_id),
    FOREIGN KEY (archive_id) REFERENCES archives(id) ON DELETE CASCADE,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE CASCADE
);

-- 阅读历史
CREATE TABLE IF NOT EXISTS history (
    archive_id INTEGER PRIMARY KEY,
    page_index INTEGER DEFAULT 0,
    total_pages INTEGER DEFAULT 0,
    updated_at TEXT DEFAULT (datetime('now')),
    FOREIGN KEY (archive_id) REFERENCES archives(id) ON DELETE CASCADE
);

-- 系统设置
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 索引：加速列表排序、搜索与历史记录查询
CREATE INDEX IF NOT EXISTS idx_archives_updated_at ON archives(updated_at);
CREATE INDEX IF NOT EXISTS idx_archives_created_at ON archives(created_at);
CREATE INDEX IF NOT EXISTS idx_archives_title ON archives(title);
CREATE INDEX IF NOT EXISTS idx_archives_page_count ON archives(page_count);
CREATE INDEX IF NOT EXISTS idx_archives_file_size ON archives(file_size);
CREATE INDEX IF NOT EXISTS idx_archives_group_id ON archives(group_id);
CREATE INDEX IF NOT EXISTS idx_history_updated_at ON history(updated_at);
CREATE INDEX IF NOT EXISTS idx_archive_tags_tag_id ON archive_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_pages_archive_id ON pages(archive_id);
"#;
