const Database = require('better-sqlite3');
const path = require('path');
const fs = require('fs');
const logger = require('../config/logger');

const DB_PATH = path.join(__dirname, '../../data/manhuaviewer.db');

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
    CREATE TABLE IF NOT EXISTS folders (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      path TEXT NOT NULL UNIQUE,
      image_count INTEGER DEFAULT 0,
      created_at TEXT DEFAULT (datetime('now'))
    );

    CREATE TABLE IF NOT EXISTS images (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      folder_id INTEGER NOT NULL,
      filename TEXT NOT NULL,
      filepath TEXT NOT NULL,
      sort_order INTEGER NOT NULL,
      width INTEGER DEFAULT 0,
      height INTEGER DEFAULT 0,
      file_size INTEGER DEFAULT 0,
      FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS history (
      folder_id INTEGER PRIMARY KEY,
      page_index INTEGER DEFAULT 0,
      total_pages INTEGER DEFAULT 0,
      updated_at TEXT DEFAULT (datetime('now')),
      FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS tags (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL UNIQUE,
      color TEXT DEFAULT '#4a86e8'
    );

    CREATE TABLE IF NOT EXISTS folder_tags (
      folder_id INTEGER NOT NULL,
      tag_id INTEGER NOT NULL,
      PRIMARY KEY (folder_id, tag_id),
      FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE,
      FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );
  `);

  // 确保 root_dir 设置存在
  const rootDir = db.prepare('SELECT value FROM settings WHERE key = ?').get('root_dir');
  if (!rootDir) {
    db.prepare('INSERT INTO settings (key, value) VALUES (?, ?)').run('root_dir', process.env.ROOT_DIR || '');
  }

  logger.info('数据库初始化完成');
}

module.exports = { getDb, initDatabase };
