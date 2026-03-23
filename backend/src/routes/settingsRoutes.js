const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');

// 获取用户设置
router.get('/settings', (req, res) => {
  const db = getDb();
  const rows = db.prepare('SELECT key, value FROM settings').all();
  const settings = {};
  for (const row of rows) {
    settings[row.key] = row.value;
  }
  // 返回前端关心的设置，附带默认值
  res.json({
    root_dir: settings.root_dir || '',
    scale: parseFloat(settings.scale || '1'),
    theme: settings.theme || 'light',
    bg_color: settings.bg_color || '#ffffff',
    double_page: settings.double_page === 'true',
    long_image: settings.long_image === 'true',
  });
});

// 更新设置
router.put('/settings', (req, res) => {
  const db = getDb();
  const stmt = db.prepare('INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)');
  const upsert = db.transaction((entries) => {
    for (const [key, value] of entries) {
      stmt.run(key, String(value));
    }
  });

  const entries = Object.entries(req.body).filter(([k]) => k !== 'root_dir'); // root_dir 用 /config 改
  upsert(entries);
  res.json({ success: true });
});

module.exports = router;
