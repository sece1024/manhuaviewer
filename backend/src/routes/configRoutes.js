const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');

// 获取根目录配置
router.get('/config', (req, res) => {
  const db = getDb();
  const row = db.prepare('SELECT value FROM settings WHERE key = ?').get('root_dir');
  res.json({ root_dir: row ? row.value : '' });
});

// 更新根目录配置
router.put('/config', (req, res) => {
  const { root_dir } = req.body;
  if (!root_dir || typeof root_dir !== 'string') {
    return res.status(400).json({ error: 'root_dir 不能为空' });
  }
  const db = getDb();
  db.prepare('INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)').run('root_dir', root_dir);
  res.json({ root_dir });
});

module.exports = router;
