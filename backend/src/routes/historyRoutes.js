/**
 * historyRoutes.js — 阅读历史
 */
const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');

// 获取所有阅读历史
router.get('/history', (req, res) => {
  const db = getDb();
  const rows = db.prepare(`
    SELECT h.archive_id, h.page_index, h.total_pages, h.updated_at,
           a.title, a.path, a.page_count, a.archive_type
    FROM history h
    JOIN archives a ON a.id = h.archive_id
    ORDER BY h.updated_at DESC
  `).all();

  // 批量获取标签（避免 N+1 查询）
  let tagMap = {};
  if (rows.length > 0) {
    const archiveIds = rows.map(r => r.archive_id);
    const placeholders = archiveIds.map(() => '?').join(',');
    const allTags = db.prepare(`
      SELECT at2.archive_id, t.namespace, t.name, t.color
      FROM archive_tags at2
      JOIN tags t ON t.id = at2.tag_id
      WHERE at2.archive_id IN (${placeholders})
    `).all(...archiveIds);

    for (const tag of allTags) {
      if (!tagMap[tag.archive_id]) tagMap[tag.archive_id] = [];
      tagMap[tag.archive_id].push({ namespace: tag.namespace, name: tag.name, color: tag.color });
    }
  }

  const result = rows.map(r => ({
    ...r,
    cover_url: `/api/archives/${r.archive_id}/cover`,
    tags: tagMap[r.archive_id] || [],
  }));

  res.json(result);
});

// 保存/更新阅读进度
router.post('/history', (req, res) => {
  const { archive_id, page_index, total_pages } = req.body;
  if (!archive_id || page_index == null) {
    return res.status(400).json({ error: 'archive_id 和 page_index 必填' });
  }
  const db = getDb();
  db.prepare(`
    INSERT INTO history (archive_id, page_index, total_pages, updated_at)
    VALUES (?, ?, ?, datetime('now'))
    ON CONFLICT(archive_id) DO UPDATE SET
      page_index = excluded.page_index,
      total_pages = excluded.total_pages,
      updated_at = excluded.updated_at
  `).run(archive_id, page_index, total_pages || 0);
  res.json({ success: true });
});

// 删除历史记录
router.delete('/history/:archiveId', (req, res) => {
  const archiveId = parseInt(req.params.archiveId);
  if (isNaN(archiveId)) return res.status(400).json({ error: '无效的 archiveId' });
  const db = getDb();
  db.prepare('DELETE FROM history WHERE archive_id = ?').run(archiveId);
  res.json({ success: true });
});

// 清空所有历史
router.delete('/history', (req, res) => {
  const db = getDb();
  db.prepare('DELETE FROM history').run();
  res.json({ success: true });
});

module.exports = router;
