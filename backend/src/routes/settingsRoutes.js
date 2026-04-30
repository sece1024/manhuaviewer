const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');
const { startAutoScanTimer } = require('../services/scanTimer');

// 获取所有设置
router.get('/settings', (req, res) => {
  const db = getDb();
  const rows = db.prepare('SELECT key, value FROM settings').all();
  const settings = {};
  rows.forEach(r => settings[r.key] = r.value);
  res.json(settings);
});

// 更新设置（部分更新）
router.put('/settings', (req, res) => {
  const db = getDb();
  const updates = req.body;
  const stmt = db.prepare('UPDATE settings SET value = ? WHERE key = ?');

  const doUpdate = db.transaction(() => {
    for (const [key, value] of Object.entries(updates)) {
      // 验证 key 存在
      const existing = db.prepare('SELECT key FROM settings WHERE key = ?').get(key);
      if (existing) {
        stmt.run(String(value), key);
      }
    }
  });

  doUpdate();

  // 如果更新了自动扫描间隔，重启定时器
  if ('auto_scan_interval' in updates) {
    startAutoScanTimer();
  }

  res.json({ success: true });
});

// 获取根目录配置
router.get('/config', (req, res) => {
  const db = getDb();
  const row = db.prepare('SELECT value FROM settings WHERE key = ?').get('root_dir');
  res.json({ root_dir: row ? row.value : '' });
});

// 更新根目录
router.put('/config', (req, res) => {
  const { root_dir } = req.body;
  const db = getDb();
  db.prepare(`INSERT INTO settings (key, value) VALUES ('root_dir', ?)
    ON CONFLICT(key) DO UPDATE SET value = excluded.value`).run(root_dir);
  res.json({ success: true, root_dir });
});

// 数据库统计
router.get('/stats', (req, res) => {
  const db = getDb();
  const archiveCount = db.prepare('SELECT COUNT(*) as count FROM archives').get().count;
  const tagCount = db.prepare('SELECT COUNT(*) as count FROM tags').get().count;
  const categoryCount = db.prepare('SELECT COUNT(*) as count FROM categories').get().count;
  const historyCount = db.prepare('SELECT COUNT(*) as count FROM history').get().count;
  const totalPages = db.prepare('SELECT SUM(page_count) as total FROM archives').get().total || 0;
  const totalSize = db.prepare('SELECT SUM(file_size) as total FROM archives').get().total || 0;

  res.json({
    archives: archiveCount,
    tags: tagCount,
    categories: categoryCount,
    history: historyCount,
    total_pages: totalPages,
    total_size: totalSize,
  });
});

module.exports = router;
