const Database = require('better-sqlite3');
const path = require('path');
const fs = require('fs');
const logger = require('../config/logger');

const DATA_DIR = path.join(__dirname, '../../data');
const DB_PATH = path.join(DATA_DIR, 'manhuaviewer.db');

let db;

function getDb() {
  if (!db) {
    fs.mkdirSync(path.dirname(DB_PATH), { recursive: true });
    db = new Database(DB_PATH);
    db.pragma('journal_mode = WAL');
    db.pragma('foreign_keys = ON');
  }
  return db;
}

function initDatabase() {
  const db = getDb();

  db.exec(`
    -- 漫画档案（文件夹或压缩包）
    CREATE TABLE IF NOT EXISTS archives (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      title TEXT NOT NULL,
      path TEXT NOT NULL UNIQUE,
      archive_type TEXT NOT NULL DEFAULT 'folder',  -- folder | zip | rar | cbz | cbr | 7z
      page_count INTEGER DEFAULT 0,
      cover_image TEXT,           -- 封面图片路径（相对于档案）
      file_size INTEGER DEFAULT 0,
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
  `);

  // 迁移旧表结构（如果存在）
  try {
    const oldTables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='folders'").all();
    if (oldTables.length > 0) {
      logger.info('检测到旧版数据表，执行迁移...');

      // 将旧 folders 数据迁移到 archives
      const oldFolders = db.prepare('SELECT * FROM folders').all();
      let migrated = 0;
      for (const folder of oldFolders) {
        try {
          db.prepare(`INSERT OR IGNORE INTO archives (title, path, archive_type, page_count, created_at)
            VALUES (?, ?, 'folder', ?, datetime('now'))`).run(folder.name || folder.title || '', folder.path, folder.image_count || folder.page_count || 0);
          migrated++;
        } catch (e) {
          logger.debug(`迁移文件夹失败: ${folder.path} — ${e.message}`);
        }
      }

      // 迁移旧的 folder_tags 到 archive_tags
      try {
        const hasFolderTags = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='folder_tags'").all();
        if (hasFolderTags.length > 0) {
          const oldFT = db.prepare('SELECT * FROM folder_tags').all();
          for (const ft of oldFT) {
            try {
              const archive = db.prepare('SELECT id FROM archives WHERE path = (SELECT path FROM folders WHERE id = ?)').get(ft.folder_id);
              if (archive) {
                db.prepare('INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)').run(archive.id, ft.tag_id);
              }
            } catch {}
          }
        }
      } catch {}

      // 迁移旧的 history（folder_id → archive_id 映射）
      try {
        const hasOldHistory = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='read_history'").all();
        if (hasOldHistory.length > 0) {
          const oldHistory = db.prepare('SELECT * FROM read_history').all();
          for (const h of oldHistory) {
            try {
              const archive = db.prepare('SELECT id FROM archives WHERE path = (SELECT path FROM folders WHERE id = ?)').get(h.folder_id);
              if (archive) {
                db.prepare(`INSERT OR IGNORE INTO history (archive_id, page_index, total_pages, updated_at)
                  VALUES (?, ?, ?, datetime('now'))`).run(archive.id, h.page_index || 0, h.total_pages || 0);
              }
            } catch {}
          }
        }
      } catch {}

      logger.info(`迁移了 ${migrated} 个文件夹`);
    }
  } catch (e) {
    // 迁移失败不影响启动
    logger.debug('迁移检查完成');
  }

  // 修复旧版 history 表（folder_id → archive_id）
  try {
    const historyInfo = db.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='history'").get();
    if (historyInfo && historyInfo.sql.includes('folder_id')) {
      logger.info('检测到旧版 history 表，执行迁移...');
      db.exec(`
        CREATE TABLE IF NOT EXISTS history_new (
          archive_id INTEGER PRIMARY KEY,
          page_index INTEGER DEFAULT 0,
          total_pages INTEGER DEFAULT 0,
          updated_at TEXT DEFAULT (datetime('now')),
          FOREIGN KEY (archive_id) REFERENCES archives(id) ON DELETE CASCADE
        );
        INSERT OR IGNORE INTO history_new (archive_id, page_index, total_pages, updated_at)
          SELECT folder_id, page_index, total_pages, updated_at FROM history;
        DROP TABLE history;
        ALTER TABLE history_new RENAME TO history;
      `);
      logger.info('history 表迁移完成');
    }
  } catch (e) {
    logger.debug(`history 表迁移跳过: ${e.message}`);
  }

  // 修复旧版 tags 表（添加 namespace 列）
  try {
    const tagsInfo = db.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='tags'").get();
    if (tagsInfo && !tagsInfo.sql.includes('namespace')) {
      logger.info('检测到旧版 tags 表，添加 namespace 列...');
      db.exec("ALTER TABLE tags ADD COLUMN namespace TEXT DEFAULT ''");
      logger.info('tags 表迁移完成');
    }
  } catch (e) {
    logger.debug(`tags 表迁移跳过: ${e.message}`);
  }

  // 迁移 folder_tags 数据到 archive_tags
  try {
    const hasFolderTags = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='folder_tags'").all();
    if (hasFolderTags.length > 0) {
      logger.info('检测到旧版 folder_tags，迁移到 archive_tags...');
      const oldFT = db.prepare('SELECT * FROM folder_tags').all();
      const stmt = db.prepare('INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)');
      for (const ft of oldFT) {
        try {
          // folder_tags.folder_id → archives.id（通过 folders.path 关联）
          const archive = db.prepare("SELECT a.id FROM archives a JOIN folders f ON f.path = a.path WHERE f.id = ?").get(ft.folder_id);
          if (archive) {
            stmt.run(archive.id, ft.tag_id);
          }
        } catch {}
      }
      db.exec('DROP TABLE folder_tags');
      logger.info('folder_tags 迁移完成');
    }
  } catch (e) {
    logger.debug(`folder_tags 迁移跳过: ${e.message}`);
  }

  // 确保设置存在
  const defaults = {
    root_dir: process.env.ROOT_DIR || '',
    view_mode: 'grid',         // grid | list
    sort_by: 'updated',        // name | updated | created | pages | size
    sort_order: 'desc',        // asc | desc
    reader_fit: 'height',      // height | width | original
    reader_bg: '#1a1a1a',
    auto_scan_interval: '0',   // 自动扫描间隔(分钟)，0=关闭
    page_direction: 'rtl',     // rtl (右到左) | ltr (左到右)
    theme: 'dark',
  };

  for (const [key, value] of Object.entries(defaults)) {
    const existing = db.prepare('SELECT value FROM settings WHERE key = ?').get(key);
    if (!existing) {
      db.prepare('INSERT INTO settings (key, value) VALUES (?, ?)').run(key, value);
    }
  }

  logger.info('数据库初始化完成');
}

module.exports = { getDb, initDatabase, DATA_DIR };
