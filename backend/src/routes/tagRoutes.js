/**
 * tagRoutes.js — 标签 CRUD（支持命名空间）
 */
const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');

// 获取所有标签（可按命名空间筛选）
router.get('/tags', (req, res) => {
  const db = getDb();
  const { namespace } = req.query;

  let sql = 'SELECT * FROM tags';
  const params = [];
  if (namespace) {
    sql += ' WHERE namespace = ?';
    params.push(namespace);
  }
  sql += ' ORDER BY namespace, name';

  const tags = db.prepare(sql).all(...params);

  // 统计每个标签关联的档案数
  const countStmt = db.prepare('SELECT COUNT(*) as count FROM archive_tags WHERE tag_id = ?');
  const result = tags.map(t => ({
    ...t,
    full_name: t.namespace ? `${t.namespace}:${t.name}` : t.name,
    archive_count: countStmt.get(t.id).count,
  }));

  res.json(result);
});

// 获取所有命名空间
router.get('/tags/namespaces', (req, res) => {
  const db = getDb();
  const rows = db.prepare(
    "SELECT DISTINCT namespace FROM tags WHERE namespace != '' ORDER BY namespace"
  ).all();
  res.json(rows.map(r => r.namespace));
});

// 创建标签
router.post('/tags', (req, res) => {
  const { name, color, namespace } = req.body;
  if (!name) return res.status(400).json({ error: 'name 必填' });
  const db = getDb();

  const ns = namespace || '';
  // 解析 "namespace:name" 格式
  let tagName = name;
  let tagNs = ns;
  if (name.includes(':') && !ns) {
    const parts = name.split(':');
    tagNs = parts[0];
    tagName = parts.slice(1).join(':');
  }

  try {
    const info = db.prepare('INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)').run(tagNs, tagName, color || '#4a86e8');
    res.json({ id: info.lastInsertRowid, namespace: tagNs, name: tagName, color: color || '#4a86e8' });
  } catch (e) {
    if (e.message.includes('UNIQUE')) {
      return res.status(409).json({ error: '标签已存在' });
    }
    throw e;
  }
});

// 更新标签
router.put('/tags/:id', (req, res) => {
  const { name, color, namespace } = req.body;
  const db = getDb();
  const tag = db.prepare('SELECT * FROM tags WHERE id = ?').get(parseInt(req.params.id));
  if (!tag) return res.status(404).json({ error: '标签不存在' });

  const newName = name || tag.name;
  const newColor = color || tag.color;
  const newNs = namespace !== undefined ? namespace : tag.namespace;
  db.prepare('UPDATE tags SET name = ?, color = ?, namespace = ? WHERE id = ?').run(newName, newColor, newNs, tag.id);
  res.json({ id: tag.id, namespace: newNs, name: newName, color: newColor });
});

// 删除标签
router.delete('/tags/:id', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM tags WHERE id = ?').run(parseInt(req.params.id));
  res.json({ success: true });
});

// 给档案分配标签
router.post('/tags/assign', (req, res) => {
  const { archive_id, tag_id } = req.body;
  if (!archive_id || !tag_id) return res.status(400).json({ error: 'archive_id 和 tag_id 必填' });
  const db = getDb();
  try {
    db.prepare('INSERT INTO archive_tags (archive_id, tag_id) VALUES (?, ?)').run(archive_id, tag_id);
  } catch {}
  res.json({ success: true });
});

// 批量分配标签
router.post('/tags/assign-batch', (req, res) => {
  const { archive_id, tag_ids } = req.body;
  if (!archive_id || !tag_ids || !Array.isArray(tag_ids)) {
    return res.status(400).json({ error: 'archive_id 和 tag_ids 必填' });
  }
  const db = getDb();
  const stmt = db.prepare('INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)');
  const assignAll = db.transaction(() => {
    for (const tagId of tag_ids) {
      stmt.run(archive_id, tagId);
    }
  });
  assignAll();
  res.json({ success: true });
});

// 移除档案标签
router.delete('/tags/:archiveId/:tagId', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM archive_tags WHERE archive_id = ? AND tag_id = ?').run(
    parseInt(req.params.archiveId),
    parseInt(req.params.tagId)
  );
  res.json({ success: true });
});

module.exports = router;
