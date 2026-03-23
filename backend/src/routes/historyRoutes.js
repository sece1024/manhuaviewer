const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');

// 获取所有阅读历史
router.get('/history', (req, res) => {
  const db = getDb();
  const rows = db.prepare(`
    SELECT h.folder_id, h.page_index, h.total_pages, h.updated_at,
           f.name, f.path, f.image_count
    FROM history h
    JOIN folders f ON f.id = h.folder_id
    ORDER BY h.updated_at DESC
  `).all();

  const tagStmt = db.prepare(`
    SELECT t.name, t.color FROM folder_tags ft
    JOIN tags t ON t.id = ft.tag_id WHERE ft.folder_id = ?
  `);

  const result = rows.map((r) => ({ ...r, tags: tagStmt.all(r.folder_id) }));
  res.json(result);
});

// 保存/更新阅读进度
router.post('/history', (req, res) => {
  const { folder_id, page_index, total_pages } = req.body;
  if (!folder_id || page_index == null) {
    return res.status(400).json({ error: 'folder_id 和 page_index 必填' });
  }
  const db = getDb();
  db.prepare(`
    INSERT INTO history (folder_id, page_index, total_pages, updated_at)
    VALUES (?, ?, ?, datetime('now'))
    ON CONFLICT(folder_id) DO UPDATE SET
      page_index = excluded.page_index,
      total_pages = excluded.total_pages,
      updated_at = excluded.updated_at
  `).run(folder_id, page_index, total_pages || 0);
  res.json({ success: true });
});

// 删除历史记录
router.delete('/history/:folderId', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM history WHERE folder_id = ?').run(parseInt(req.params.folderId));
  res.json({ success: true });
});

module.exports = router;
