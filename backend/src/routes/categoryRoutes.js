/**
 * categoryRoutes.js — 分类 CRUD
 */
const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');
const logger = require('../config/logger');

// 获取所有分类
router.get('/categories', (req, res) => {
  const db = getDb();
  const categories = db.prepare('SELECT * FROM categories ORDER BY pinned DESC, name').all();

  const countStmt = db.prepare('SELECT COUNT(*) as count FROM archive_categories WHERE category_id = ?');
  const result = categories.map(c => ({
    ...c,
    archive_count: countStmt.get(c.id).count,
  }));

  res.json(result);
});

// 创建分类
router.post('/categories', (req, res) => {
  const { name, color, pinned, search } = req.body;
  if (!name) return res.status(400).json({ error: 'name 必填' });
  const db = getDb();

  try {
    const info = db.prepare('INSERT INTO categories (name, color, pinned, search) VALUES (?, ?, ?, ?)')
      .run(name, color || '#4a86e8', pinned ? 1 : 0, search || '');
    res.json({ id: info.lastInsertRowid, name, color: color || '#4a86e8' });
  } catch (e) {
    if (e.message.includes('UNIQUE')) return res.status(409).json({ error: '分类已存在' });
    throw e;
  }
});

// 更新分类
router.put('/categories/:id', (req, res) => {
  const { name, color, pinned, search } = req.body;
  const db = getDb();
  const cat = db.prepare('SELECT * FROM categories WHERE id = ?').get(parseInt(req.params.id));
  if (!cat) return res.status(404).json({ error: '分类不存在' });

  db.prepare('UPDATE categories SET name = ?, color = ?, pinned = ?, search = ? WHERE id = ?')
    .run(name || cat.name, color || cat.color, pinned !== undefined ? (pinned ? 1 : 0) : cat.pinned, search !== undefined ? search : cat.search, cat.id);
  res.json({ success: true });
});

// 删除分类
router.delete('/categories/:id', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM categories WHERE id = ?').run(parseInt(req.params.id));
  res.json({ success: true });
});

// 将档案添加到分类
router.post('/categories/assign', (req, res) => {
  const { archive_id, category_id } = req.body;
  if (!archive_id || !category_id) return res.status(400).json({ error: 'archive_id 和 category_id 必填' });
  const db = getDb();
  try {
    db.prepare('INSERT INTO archive_categories (archive_id, category_id) VALUES (?, ?)').run(archive_id, category_id);
  } catch (e) {
    if (!e.message.includes('UNIQUE')) logger.warn(`分类分配失败: ${e.message}`);
  }
  res.json({ success: true });
});

// 从分类中移除档案
router.delete('/categories/:archiveId/:categoryId', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM archive_categories WHERE archive_id = ? AND category_id = ?')
    .run(parseInt(req.params.archiveId), parseInt(req.params.categoryId));
  res.json({ success: true });
});

module.exports = router;
