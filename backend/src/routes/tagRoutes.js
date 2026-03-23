const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');

// 获取所有标签
router.get('/tags', (req, res) => {
  const db = getDb();
  const tags = db.prepare('SELECT * FROM tags ORDER BY name').all();
  res.json(tags);
});

// 创建标签
router.post('/tags', (req, res) => {
  const { name, color } = req.body;
  if (!name) return res.status(400).json({ error: 'name 必填' });
  const db = getDb();
  try {
    const info = db.prepare('INSERT INTO tags (name, color) VALUES (?, ?)').run(name, color || '#4a86e8');
    res.json({ id: info.lastInsertRowid, name, color: color || '#4a86e8' });
  } catch (e) {
    if (e.message.includes('UNIQUE')) {
      return res.status(409).json({ error: '标签已存在' });
    }
    throw e;
  }
});

// 更新标签
router.put('/tags/:id', (req, res) => {
  const { name, color } = req.body;
  const db = getDb();
  const tag = db.prepare('SELECT * FROM tags WHERE id = ?').get(parseInt(req.params.id));
  if (!tag) return res.status(404).json({ error: '标签不存在' });

  const newName = name || tag.name;
  const newColor = color || tag.color;
  db.prepare('UPDATE tags SET name = ?, color = ? WHERE id = ?').run(newName, newColor, tag.id);
  res.json({ id: tag.id, name: newName, color: newColor });
});

// 删除标签
router.delete('/tags/:id', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM tags WHERE id = ?').run(parseInt(req.params.id));
  res.json({ success: true });
});

// 给文件夹分配标签
router.post('/tags/assign', (req, res) => {
  const { folder_id, tag_id } = req.body;
  if (!folder_id || !tag_id) return res.status(400).json({ error: 'folder_id 和 tag_id 必填' });
  const db = getDb();
  try {
    db.prepare('INSERT INTO folder_tags (folder_id, tag_id) VALUES (?, ?)').run(folder_id, tag_id);
  } catch {
    // 已存在则忽略
  }
  res.json({ success: true });
});

// 移除文件夹标签
router.delete('/tags/:folderId/:tagId', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM folder_tags WHERE folder_id = ? AND tag_id = ?').run(
    parseInt(req.params.folderId),
    parseInt(req.params.tagId)
  );
  res.json({ success: true });
});

module.exports = router;
