/**
 * backupRoutes.js — 备份与恢复
 * 支持导出/导入 JSON 备份（包含所有档案元数据、标签、分类、阅读历史）
 */
const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');
const logger = require('../config/logger');

/**
 * 导出备份 — GET /api/backup
 * 返回 JSON 包含所有档案元数据、标签、分类、阅读历史、设置
 */
router.get('/backup', (req, res) => {
  const db = getDb();

  const archives = db.prepare('SELECT * FROM archives ORDER BY id').all();
  const tags = db.prepare('SELECT * FROM tags ORDER BY id').all();
  const categories = db.prepare('SELECT * FROM categories ORDER BY id').all();
  const archiveTags = db.prepare('SELECT * FROM archive_tags ORDER BY archive_id').all();
  const archiveCategories = db.prepare('SELECT * FROM archive_categories ORDER BY archive_id').all();
  const history = db.prepare('SELECT * FROM history ORDER BY archive_id').all();
  const settings = db.prepare('SELECT * FROM settings ORDER BY key').all();

  const backup = {
    version: '2.0.0',
    exported_at: new Date().toISOString(),
    data: {
      archives,
      tags,
      categories,
      archive_tags: archiveTags,
      archive_categories: archiveCategories,
      history,
      settings,
    },
  };

  res.set('Content-Type', 'application/json');
  res.set('Content-Disposition', `attachment; filename="manhuaviewer-backup-${new Date().toISOString().slice(0, 10)}.json"`);
  res.json(backup);
});

/**
 * 导入恢复 — POST /api/restore
 * 从 JSON 备份恢复数据
 */
router.post('/restore', (req, res) => {
  const db = getDb();
  const backup = req.body;

  if (!backup || !backup.data) {
    return res.status(400).json({ error: '无效的备份格式' });
  }

  const { archives, tags, categories, archive_tags, archive_categories, history, settings } = backup.data;

  try {
    const doRestore = db.transaction(() => {
      // 恢复标签（保留 ID）
      if (tags && Array.isArray(tags)) {
        const stmt = db.prepare('INSERT OR REPLACE INTO tags (id, namespace, name, color) VALUES (?, ?, ?, ?)');
        for (const t of tags) {
          stmt.run(t.id, t.namespace || '', t.name, t.color || '#4a86e8');
        }
      }

      // 恢复分类（保留 ID）
      if (categories && Array.isArray(categories)) {
        const stmt = db.prepare('INSERT OR REPLACE INTO categories (id, name, color, pinned, search, created_at) VALUES (?, ?, ?, ?, ?, ?)');
        for (const c of categories) {
          stmt.run(c.id, c.name, c.color || '#4a86e8', c.pinned || 0, c.search || '', c.created_at);
        }
      }

      // 恢复档案（保留 ID）
      if (archives && Array.isArray(archives)) {
        const stmt = db.prepare(`INSERT OR REPLACE INTO archives (id, title, path, archive_type, page_count, cover_image, file_size, created_at, updated_at)
          VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`);
        for (const a of archives) {
          stmt.run(a.id, a.title, a.path, a.archive_type, a.page_count || 0, a.cover_image, a.file_size || 0, a.created_at, a.updated_at);
        }
      }

      // 恢复档案-标签关联
      if (archive_tags && Array.isArray(archive_tags)) {
        db.prepare('DELETE FROM archive_tags').run();
        const stmt = db.prepare('INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)');
        for (const at of archive_tags) {
          stmt.run(at.archive_id, at.tag_id);
        }
      }

      // 恢复档案-分类关联
      if (archive_categories && Array.isArray(archive_categories)) {
        db.prepare('DELETE FROM archive_categories').run();
        const stmt = db.prepare('INSERT OR IGNORE INTO archive_categories (archive_id, category_id) VALUES (?, ?)');
        for (const ac of archive_categories) {
          stmt.run(ac.archive_id, ac.category_id);
        }
      }

      // 恢复阅读历史
      if (history && Array.isArray(history)) {
        const stmt = db.prepare(`INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at) VALUES (?, ?, ?, ?)`);
        for (const h of history) {
          stmt.run(h.archive_id, h.page_index || 0, h.total_pages || 0, h.updated_at);
        }
      }

      // 恢复设置
      if (settings && Array.isArray(settings)) {
        const stmt = db.prepare('INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)');
        for (const s of settings) {
          stmt.run(s.key, s.value);
        }
      }
    });

    doRestore();

    const summary = {
      archives: archives ? archives.length : 0,
      tags: tags ? tags.length : 0,
      categories: categories ? categories.length : 0,
      history: history ? history.length : 0,
      settings: settings ? settings.length : 0,
    };

    logger.info(`备份恢复完成: ${JSON.stringify(summary)}`);
    res.json({ success: true, message: '备份恢复成功', restored: summary });
  } catch (err) {
    logger.error(`备份恢复失败: ${err.message}`);
    res.status(500).json({ error: `恢复失败: ${err.message}` });
  }
});

module.exports = router;
