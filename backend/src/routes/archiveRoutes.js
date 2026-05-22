/**
 * archiveRoutes.js — 漫画档案 CRUD + 扫描
 */
const express = require('express');
const router = express.Router();
const fs = require('fs');
const path = require('path');
const { getDb } = require('../db/database');
const { scanRoot } = require('../services/scanService');
const archiveService = require('../services/archiveService');
const { parseSearchSyntax } = require('../utils/searchParser');

// 获取档案列表（支持搜索、标签筛选、排序）
router.get('/archives', (req, res) => {
  try {
    const db = getDb();
    const { search, tag, category, sort_by, sort_order } = req.query;
  const settingsRow = db.prepare("SELECT key, value FROM settings WHERE key IN ('sort_by','sort_order')").all();
  const settings = {};
  settingsRow.forEach(r => settings[r.key] = r.value);

  let sql = `
    SELECT DISTINCT a.*, h.page_index as read_page, h.total_pages as read_total, h.updated_at as last_read
    FROM archives a
    LEFT JOIN history h ON h.archive_id = a.id
  `;
  const params = [];
  const conditions = [];

  // 解析搜索语法
  const parsed = parseSearchSyntax(search);

  // 通过侧边栏 tag 参数筛选
  if (tag) {
    sql += ` INNER JOIN archive_tags at2 ON at2.archive_id = a.id
             INNER JOIN tags t ON t.id = at2.tag_id `;
    if (tag.includes(':')) {
      const [ns, name] = tag.split(':');
      conditions.push('(t.namespace = ? AND t.name = ?)');
      params.push(ns, name);
    } else {
      conditions.push('t.name = ?');
      params.push(tag);
    }
  }

  // 搜索框中的标签包含 (tag:xxx 或 namespace:xxx)
  for (const tagExpr of parsed.includeTags) {
    sql += ` INNER JOIN archive_tags at_inc ON at_inc.archive_id = a.id
             INNER JOIN tags t_inc ON t_inc.id = at_inc.tag_id `;
    if (tagExpr.includes(':')) {
      const [ns, name] = tagExpr.split(':');
      conditions.push('(t_inc.namespace = ? AND t_inc.name = ?)');
      params.push(ns, name);
    } else {
      conditions.push('t_inc.name = ?');
      params.push(tagExpr);
    }
  }

  // 排除标签 (-tag:xxx 或 -namespace:xxx)
  for (const tagExpr of parsed.excludeTags) {
    if (tagExpr.includes(':')) {
      const [ns, name] = tagExpr.split(':');
      conditions.push(`a.id NOT IN (SELECT at_ex.archive_id FROM archive_tags at_ex JOIN tags t_ex ON t_ex.id = at_ex.tag_id WHERE t_ex.namespace = ? AND t_ex.name = ?)`);
      params.push(ns, name);
    } else {
      conditions.push(`a.id NOT IN (SELECT at_ex.archive_id FROM archive_tags at_ex JOIN tags t_ex ON t_ex.id = at_ex.tag_id WHERE t_ex.name = ?)`);
      params.push(tagExpr);
    }
  }

  // 分类筛选
  if (category) {
    sql += ` INNER JOIN archive_categories ac ON ac.archive_id = a.id
             INNER JOIN categories c ON c.id = ac.category_id `;
    conditions.push('c.name = ?');
    params.push(category);
  }

  // 普通文本搜索
  if (parsed.textTerms.length > 0) {
    for (const term of parsed.textTerms) {
      conditions.push('(a.title LIKE ? OR a.path LIKE ?)');
      params.push(`%${term}%`, `%${term}%`);
    }
  }

  // 排除文本 (-keyword)
  for (const term of parsed.excludeText) {
    conditions.push('(a.title NOT LIKE ? AND a.path NOT LIKE ?)');
    params.push(`%${term}%`, `%${term}%`);
  }

  if (conditions.length > 0) {
    sql += ' WHERE ' + conditions.join(' AND ');
  }

  // 排序
  const sortField = sort_by || settings.sort_by || 'updated';
  const sortOrder = (sort_order || settings.sort_order || 'desc').toUpperCase() === 'ASC' ? 'ASC' : 'DESC';
  const sortMap = {
    name: 'a.title',
    updated: 'COALESCE(h.updated_at, a.updated_at)',
    created: 'a.created_at',
    pages: 'a.page_count',
    size: 'a.file_size',
  };
  sql += ` ORDER BY ${sortMap[sortField] || sortMap.updated} ${sortOrder}`;

  const archives = db.prepare(sql).all(...params);

  // 批量获取所有相关档案的标签（避免 N+1 查询）
  let tagMap = {};
  if (archives.length > 0) {
    const placeholders = archives.map(() => '?').join(',');
    const allTags = db.prepare(`
      SELECT at2.archive_id, t.namespace, t.name, t.color
      FROM archive_tags at2
      JOIN tags t ON t.id = at2.tag_id
      WHERE at2.archive_id IN (${placeholders})
    `).all(...archives.map(a => a.id));

    for (const tag of allTags) {
      if (!tagMap[tag.archive_id]) tagMap[tag.archive_id] = [];
      tagMap[tag.archive_id].push({ namespace: tag.namespace, name: tag.name, color: tag.color });
    }
  }

  const result = archives.map(a => ({
    ...a,
    cover_url: `/api/archives/${a.id}/cover`,
    tags: tagMap[a.id] || [],
  }));

  res.json(result);
  } catch (err) {
    res.status(500).json({ error: '查询档案列表失败' });
  }
});

// 获取单个档案详情
router.get('/archives/:id', (req, res) => {
  const db = getDb();
  const id = parseInt(req.params.id);
  if (isNaN(id)) return res.status(400).json({ error: '无效的 ID' });
  const archive = db.prepare('SELECT * FROM archives WHERE id = ?').get(id);
  if (!archive) return res.status(404).json({ error: '档案不存在' });

  const tags = db.prepare(`
    SELECT t.namespace, t.name, t.color FROM archive_tags at2
    JOIN tags t ON t.id = at2.tag_id WHERE at2.archive_id = ?
  `).all(id);

  const categories = db.prepare(`
    SELECT c.id, c.name, c.color FROM archive_categories ac
    JOIN categories c ON c.id = ac.category_id WHERE ac.archive_id = ?
  `).all(id);

  const history = db.prepare('SELECT * FROM history WHERE archive_id = ?').get(id);

  res.json({
    ...archive,
    cover_url: `/api/archives/${id}/cover`,
    tags,
    categories,
    history: history || null,
  });
});

// 生成默认封面 SVG（作为 fallback）
function generateDefaultCover(title, type) {
  const emoji = type === 'folder' ? '📁' : '📦';
  const safeTitle = (title || '?').replace(/[<>&"']/g, c => ({ '<':'&lt;', '>':'&gt;', '&':'&amp;', '"':'&quot;', "'":'&#39;' }[c]));
  const truncated = safeTitle.length > 15 ? safeTitle.slice(0, 14) + '…' : safeTitle;
  return `<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="300" height="420" viewBox="0 0 300 420">
  <rect width="300" height="420" fill="#252528"/>
  <text x="150" y="170" text-anchor="middle" font-size="64">${emoji}</text>
  <text x="150" y="240" text-anchor="middle" font-size="16" fill="#a0a0a0" font-family="sans-serif">${truncated}</text>
  <text x="150" y="270" text-anchor="middle" font-size="12" fill="#666" font-family="sans-serif">封面未生成</text>
</svg>`;
}

// 获取档案封面
router.get('/archives/:id/cover', (req, res) => {
  const db = getDb();
  const id = parseInt(req.params.id);
  if (isNaN(id)) return res.status(400).json({ error: '无效的 ID' });
  const archive = db.prepare('SELECT * FROM archives WHERE id = ?').get(id);
  if (!archive) return res.status(404).json({ error: '档案不存在' });

  const { getDataDir } = require('../db/database');
  const thumbPath = path.join(getDataDir(), 'thumbnails', `${id}_cover.jpg`);

  if (fs.existsSync(thumbPath)) {
    return res.sendFile(path.resolve(thumbPath));
  }

  // 没有封面，返回默认 SVG
  const svg = generateDefaultCover(archive.title, archive.archive_type);
  res.set('Content-Type', 'image/svg+xml');
  res.set('Cache-Control', 'public, max-age=3600');
  res.send(svg);
});

// 获取档案页面（图片列表）
router.get('/archives/:id/pages', async (req, res) => {
  const db = getDb();
  const id = parseInt(req.params.id);
  if (isNaN(id)) return res.status(400).json({ error: '无效的 ID' });
  const archive = db.prepare('SELECT * FROM archives WHERE id = ?').get(id);
  if (!archive) return res.status(404).json({ error: '档案不存在' });

  let pages = [];

  if (archive.archive_type === 'folder') {
    // 文件夹类型：异步扫描
    if (!fs.existsSync(archive.path)) {
      return res.status(404).json({ error: '文件夹不存在' });
    }
    const allFiles = await fs.promises.readdir(archive.path);
    const files = allFiles
      .filter(f => archiveService.isImage(f))
      .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));

    pages = files.map((f, i) => ({
      id: `${archive.id}_${i}`,
      archive_id: archive.id,
      filename: f,
      filepath: f,
      sort_order: i,
      url: `/api/archives/${archive.id}/pages/${i}`,
      thumb_url: `/api/archives/${archive.id}/pages/${i}/thumb`,
    }));
  } else {
    // 压缩包类型：从数据库读取
    const dbPages = db.prepare(
      'SELECT * FROM pages WHERE archive_id = ? ORDER BY sort_order'
    ).all(id);

    pages = dbPages.map(p => ({
      ...p,
      url: `/api/archives/${id}/pages/${p.sort_order}`,
      thumb_url: `/api/archives/${id}/pages/${p.sort_order}/thumb`,
    }));
  }

  // 获取阅读进度
  const history = db.prepare('SELECT page_index FROM history WHERE archive_id = ?').get(id);

  res.json({
    archive: { ...archive, cover_url: `/api/archives/${id}/cover` },
    pages,
    read_page: history ? history.page_index : 0,
  });
});

// 读取单页图片
router.get('/archives/:archiveId/pages/:pageIndex', async (req, res) => {
  const db = getDb();
  const archiveId = parseInt(req.params.archiveId);
  const pageIndex = parseInt(req.params.pageIndex);

  const archive = db.prepare('SELECT * FROM archives WHERE id = ?').get(archiveId);
  if (!archive) return res.status(404).json({ error: '档案不存在' });

  try {
    if (archive.archive_type === 'folder') {
      // 文件夹：异步读文件
      const allFiles = await fs.promises.readdir(archive.path);
      const files = allFiles
        .filter(f => archiveService.isImage(f))
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));

      if (pageIndex >= files.length) return res.status(404).json({ error: '页码超出范围' });

      const filePath = path.join(archive.path, files[pageIndex]);
      if (!fs.existsSync(filePath)) return res.status(404).json({ error: '图片文件不存在' });

      return res.sendFile(path.resolve(filePath));
    } else {
      // 压缩包：解压
      const page = db.prepare(
        'SELECT * FROM pages WHERE archive_id = ? AND sort_order = ?'
      ).get(archiveId, pageIndex);
      if (!page) return res.status(404).json({ error: '页面不存在' });

      const data = await archiveService.extractFile(archive.path, page.filepath);
      const ext = path.extname(page.filename).toLowerCase().replace('.', '');
      const mimeMap = { jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', webp: 'image/webp', gif: 'image/gif', bmp: 'image/bmp', avif: 'image/avif' };

      res.set('Content-Type', mimeMap[ext] || 'image/jpeg');
      res.set('Cache-Control', 'public, max-age=3600');
      return res.send(data);
    }
  } catch (err) {
    res.status(500).json({ error: `读取图片失败: ${err.message}` });
  }
});

// 读取单页缩略图
router.get('/archives/:archiveId/pages/:pageIndex/thumb', async (req, res) => {
  const db = getDb();
  const archiveId = parseInt(req.params.archiveId);
  const pageIndex = parseInt(req.params.pageIndex);

  const { getDataDir } = require('../db/database');
  const thumbDir = path.join(getDataDir(), 'thumbnails', 'pages');
  fs.mkdirSync(thumbDir, { recursive: true });
  const thumbPath = path.join(thumbDir, `${archiveId}_${pageIndex}.jpg`);

  if (fs.existsSync(thumbPath)) {
    return res.sendFile(path.resolve(thumbPath));
  }

  // 生成缩略图
  const archive = db.prepare('SELECT * FROM archives WHERE id = ?').get(archiveId);
  if (!archive) return res.status(404).json({ error: '档案不存在' });

  try {
    let imageData;
    if (archive.archive_type === 'folder') {
      const allFiles = await fs.promises.readdir(archive.path);
      const files = allFiles
        .filter(f => archiveService.isImage(f))
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
      if (pageIndex >= files.length) return res.status(404).end();
      imageData = await fs.promises.readFile(path.join(archive.path, files[pageIndex]));
    } else {
      const page = db.prepare('SELECT * FROM pages WHERE archive_id = ? AND sort_order = ?').get(archiveId, pageIndex);
      if (!page) return res.status(404).end();
      imageData = await archiveService.extractFile(archive.path, page.filepath);
    }

    const sharp = require('sharp');
    await sharp(imageData)
      .resize(150, 200, { fit: 'inside' })
      .jpeg({ quality: 60 })
      .toFile(thumbPath);

    res.sendFile(path.resolve(thumbPath));
  } catch (err) {
    res.status(500).json({ error: '缩略图生成失败' });
  }
});

// 直接打开文件/文件夹路径
router.post('/open', async (req, res) => {
  try {
    const { filePath } = req.body;
    if (!filePath) return res.status(400).json({ error: '请提供文件路径' });

    const db = getDb();
    const resolved = path.resolve(filePath);

    // 检查路径是否存在
    if (!fs.existsSync(resolved)) {
      return res.status(404).json({ error: `路径不存在: ${resolved}` });
    }

    // 检查是否已在数据库中
    const existing = db.prepare('SELECT id FROM archives WHERE path = ?').get(resolved);
    if (existing) {
      return res.json({ id: existing.id, message: '文件已存在于库中' });
    }

    const stat = fs.statSync(resolved);

    if (stat.isDirectory()) {
      // 文件夹：检查是否包含图片
      const allFiles = await fs.promises.readdir(resolved);
      const images = allFiles.filter(f => archiveService.isImage(f));
      if (images.length === 0) {
        return res.status(400).json({ error: '文件夹中没有找到图片文件' });
      }

      const sorted = images.sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
      let totalSize = 0;
      for (const f of images) {
        try { totalSize += fs.statSync(path.join(resolved, f)).size; } catch {}
      }

      const title = path.basename(resolved);
      const info = db.prepare(`INSERT INTO archives (title, path, archive_type, page_count, file_size)
        VALUES (?, ?, 'folder', ?, ?)`).run(title, resolved, sorted.length, totalSize);

      const archiveId = info.lastInsertRowid;
      archiveService.extractFolderCover(resolved, archiveId).catch(() => {});

      return res.json({ id: archiveId, title, archive_type: 'folder' });
    } else {
      // 压缩包文件
      if (!archiveService.isArchive(path.basename(resolved))) {
        return res.status(400).json({ error: '不支持的文件格式，请选择图片文件夹或压缩包 (ZIP/CBZ/RAR/CBR/7Z)' });
      }

      let images;
      try {
        images = await archiveService.getImageList(resolved);
      } catch (err) {
        return res.status(400).json({ error: `无法读取压缩包: ${err.message}` });
      }
      if (images.length === 0) {
        return res.status(400).json({ error: '压缩包中没有图片' });
      }

      const ext = path.extname(resolved).toLowerCase().replace('.', '');
      const archiveType = ext === 'cbz' ? 'zip' : ext === 'cbr' ? 'rar' : ext;
      const title = path.basename(resolved, path.extname(resolved));

      const result = db.prepare(`INSERT INTO archives (title, path, archive_type, page_count, file_size)
        VALUES (?, ?, ?, ?, ?)`).run(title, resolved, archiveType, images.length, stat.size);
      const archiveId = result.lastInsertRowid;

      // 写入 pages 表
      const insertPage = db.prepare(
        'INSERT INTO pages (archive_id, filename, filepath, sort_order, file_size) VALUES (?, ?, ?, ?, ?)'
      );
      const insertAll = db.transaction(() => {
        for (let i = 0; i < images.length; i++) {
          insertPage.run(archiveId, images[i].name, images[i].path, i, images[i].size);
        }
      });
      insertAll();

      archiveService.extractCover(resolved, archiveId).catch(() => {});

      return res.json({ id: archiveId, title, archive_type: archiveType });
    }
  } catch (err) {
    res.status(500).json({ error: `打开失败: ${err.message}` });
  }
});

// 扫描根目录
router.post('/scan', async (req, res) => {
  const db = getDb();
  const rootRow = db.prepare('SELECT value FROM settings WHERE key = ?').get('root_dir');
  const rootDir = rootRow ? rootRow.value : '';
  const depthRow = db.prepare("SELECT value FROM settings WHERE key = 'scan_depth'").get();
  const maxDepth = depthRow ? parseInt(depthRow.value) || 0 : 0;

  try {
    const result = await scanRoot(rootDir, maxDepth);
    res.json(result);
  } catch (err) {
    res.status(400).json({ error: err.message });
  }
});

// 删除档案
router.delete('/archives/:id', (req, res) => {
  const db = getDb();
  const id = parseInt(req.params.id);
  if (isNaN(id)) return res.status(400).json({ error: '无效的 ID' });

  // 清理缩略图文件
  const { getDataDir } = require('../db/database');
  try {
    const coverPath = path.join(getDataDir(), 'thumbnails', `${id}_cover.jpg`);
    if (fs.existsSync(coverPath)) fs.unlinkSync(coverPath);
    // 清理页面缩略图
    const pagesDir = path.join(getDataDir(), 'thumbnails', 'pages');
    if (fs.existsSync(pagesDir)) {
      const pageThumbs = fs.readdirSync(pagesDir).filter(f => f.startsWith(`${id}_`));
      for (const f of pageThumbs) {
        fs.unlinkSync(path.join(pagesDir, f));
      }
    }
  } catch (e) {
    // 缩略图清理失败不影响删除操作
  }

  db.prepare('DELETE FROM archives WHERE id = ?').run(id);
  res.json({ success: true });
});

module.exports = router;
