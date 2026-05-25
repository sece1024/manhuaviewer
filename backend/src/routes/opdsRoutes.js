/**
 * opdsRoutes.js — OPDS Catalog 支持
 * 允许第三方漫画阅读器（Perfect Viewer、ComicScreen 等）通过标准协议访问
 */
const express = require('express');
const router = express.Router();
const { getDb } = require('../db/database');
const archiveService = require('../services/archiveService');

const OPDS_NS = 'http://www.w3.org/2005/Atom';
const OPDS_ACQUISITION = 'http://opds-spec.org/acquisition';

/**
 * 生成 OPDS XML header
 */
function xmlHeader() {
  return '<?xml version="1.0" encoding="UTF-8"?>';
}

/**
 * OPDS Root Catalog
 */
router.get('/opds', (req, res) => {
  const baseUrl = `${req.protocol}://${req.get('host')}`;

  const xml = `${xmlHeader()}
<feed xmlns="${OPDS_NS}">
  <title>MangaViewer OPDS</title>
  <id>urn:manhuaviewer:root</id>
  <updated>${new Date().toISOString()}</updated>
  <link href="${baseUrl}/opds" rel="self" type="application/atom+xml"/>
  <link href="${baseUrl}/opds/catalog" rel="http://opds-spec.org/catalog" type="application/atom+xml"/>
  <entry>
    <title>漫画库</title>
    <id>urn:manhuaviewer:catalog</id>
    <updated>${new Date().toISOString()}</updated>
    <link href="${baseUrl}/opds/catalog" rel="subsection" type="application/atom+xml"/>
    <content type="text">浏览所有漫画</content>
  </entry>
  <entry>
    <title>最近阅读</title>
    <id>urn:manhuaviewer:recent</id>
    <updated>${new Date().toISOString()}</updated>
    <link href="${baseUrl}/opds/recent" rel="subsection" type="application/atom+xml"/>
    <content type="text">按最近阅读排序</content>
  </entry>
  <entry>
    <title>标签</title>
    <id>urn:manhuaviewer:tags</id>
    <updated>${new Date().toISOString()}</updated>
    <link href="${baseUrl}/opds/tags" rel="subsection" type="application/atom+xml"/>
    <content type="text">按标签浏览</content>
  </entry>
</feed>`;

  res.set('Content-Type', 'application/atom+xml; charset=utf-8');
  res.send(xml);
});

/**
 * OPDS Catalog — 所有漫画列表
 */
router.get('/opds/catalog', (req, res) => {
  const db = getDb();
  const baseUrl = `${req.protocol}://${req.get('host')}`;
  const page = parseInt(req.query.page) || 0;
  const perPage = 50;

  const archives = db.prepare(`
    SELECT a.*, h.page_index as read_page
    FROM archives a
    LEFT JOIN history h ON h.archive_id = a.id
    ORDER BY a.title ASC
    LIMIT ? OFFSET ?
  `).all(perPage, page * perPage);

  const totalCount = db.prepare('SELECT COUNT(*) as count FROM archives').get().count;

  let entries = '';
  for (const a of archives) {
    const updated = a.updated_at || a.created_at || new Date().toISOString();
    entries += `
  <entry>
    <title>${escapeXml(a.title)}</title>
    <id>urn:manhuaviewer:archive:${a.id}</id>
    <updated>${updated}Z</updated>
    <link href="${baseUrl}/opds/archive/${a.id}" rel="subsection" type="application/atom+xml"/>
    <link href="${baseUrl}/api/archives/${a.id}/cover" rel="http://opds-spec.org/image" type="image/jpeg"/>
    <link href="${baseUrl}/api/archives/${a.id}/cover" rel="http://opds-spec.org/image/thumbnail" type="image/jpeg"/>
    <content type="text">${a.page_count} 页 · ${a.archive_type}</content>
  </entry>`;
  }

  const xml = `${xmlHeader()}
<feed xmlns="${OPDS_NS}">
  <title>漫画库</title>
  <id>urn:manhuaviewer:catalog</id>
  <updated>${new Date().toISOString()}</updated>
  <link href="${baseUrl}/opds/catalog" rel="self" type="application/atom+xml"/>
  <link href="${baseUrl}/opds" rel="start" type="application/atom+xml"/>
  ${page > 0 ? `<link href="${baseUrl}/opds/catalog?page=${page - 1}" rel="previous" type="application/atom+xml"/>` : ''}
  ${((page + 1) * perPage) < totalCount ? `<link href="${baseUrl}/opds/catalog?page=${page + 1}" rel="next" type="application/atom+xml"/>` : ''}
  ${entries}
</feed>`;

  res.set('Content-Type', 'application/atom+xml; charset=utf-8');
  res.send(xml);
});

/**
 * OPDS Archive 详情 — 单个漫画的页面列表
 */
router.get('/opds/archive/:id', async (req, res) => {
  const db = getDb();
  const baseUrl = `${req.protocol}://${req.get('host')}`;
  const id = parseInt(req.params.id);

  const archive = db.prepare('SELECT * FROM archives WHERE id = ?').get(id);
  if (!archive) return res.status(404).send('Not found');

  let pages = [];
  if (archive.archive_type === 'folder') {
    try {
      const files = await archiveService.listFolderImages(archive.path);
      pages = files.map((f, i) => ({
        filename: f,
        sort_order: i,
        url: `${baseUrl}/api/archives/${id}/pages/${i}`,
      }));
    } catch {}
  } else {
    const dbPages = db.prepare('SELECT * FROM pages WHERE archive_id = ? ORDER BY sort_order').all(id);
    pages = dbPages.map(p => ({
      filename: p.filename,
      sort_order: p.sort_order,
      url: `${baseUrl}/api/archives/${id}/pages/${p.sort_order}`,
    }));
  }

  let entries = '';
  for (const p of pages) {
    const ext = p.filename.split('.').pop().toLowerCase();
    const mime = archiveService.MIME_TYPES[ext] || 'image/jpeg';

    entries += `
  <entry>
    <title>${escapeXml(p.filename)}</title>
    <id>urn:manhuaviewer:archive:${id}:page:${p.sort_order}</id>
    <updated>${new Date().toISOString()}</updated>
    <link href="${p.url}" rel="http://opds-spec.org/acquisition" type="${mime}"/>
    <content type="text">第 ${p.sort_order + 1} 页</content>
  </entry>`;
  }

  const updated = archive.updated_at || archive.created_at || new Date().toISOString();
  const xml = `${xmlHeader()}
<feed xmlns="${OPDS_NS}">
  <title>${escapeXml(archive.title)}</title>
  <id>urn:manhuaviewer:archive:${id}</id>
  <updated>${updated}Z</updated>
  <link href="${baseUrl}/opds/archive/${id}" rel="self" type="application/atom+xml"/>
  <link href="${baseUrl}/opds/catalog" rel="up" type="application/atom+xml"/>
  <link href="${baseUrl}/api/archives/${id}/cover" rel="http://opds-spec.org/image" type="image/jpeg"/>
  ${entries}
</feed>`;

  res.set('Content-Type', 'application/atom+xml; charset=utf-8');
  res.send(xml);
});

/**
 * OPDS 最近阅读
 */
router.get('/opds/recent', (req, res) => {
  const db = getDb();
  const baseUrl = `${req.protocol}://${req.get('host')}`;

  const archives = db.prepare(`
    SELECT a.*, h.updated_at as last_read, h.page_index
    FROM archives a
    INNER JOIN history h ON h.archive_id = a.id
    ORDER BY h.updated_at DESC
    LIMIT 50
  `).all();

  let entries = '';
  for (const a of archives) {
    entries += `
  <entry>
    <title>${escapeXml(a.title)}</title>
    <id>urn:manhuaviewer:archive:${a.id}</id>
    <updated>${(a.last_read || a.updated_at || new Date().toISOString())}Z</updated>
    <link href="${baseUrl}/opds/archive/${a.id}" rel="subsection" type="application/atom+xml"/>
    <link href="${baseUrl}/api/archives/${a.id}/cover" rel="http://opds-spec.org/image" type="image/jpeg"/>
    <link href="${baseUrl}/api/archives/${a.id}/cover" rel="http://opds-spec.org/image/thumbnail" type="image/jpeg"/>
    <content type="text">第 ${(a.page_index || 0) + 1}/${a.page_count} 页</content>
  </entry>`;
  }

  const xml = `${xmlHeader()}
<feed xmlns="${OPDS_NS}">
  <title>最近阅读</title>
  <id>urn:manhuaviewer:recent</id>
  <updated>${new Date().toISOString()}</updated>
  <link href="${baseUrl}/opds/recent" rel="self" type="application/atom+xml"/>
  <link href="${baseUrl}/opds" rel="start" type="application/atom+xml"/>
  ${entries}
</feed>`;

  res.set('Content-Type', 'application/atom+xml; charset=utf-8');
  res.send(xml);
});

/**
 * OPDS 标签列表
 */
router.get('/opds/tags', (req, res) => {
  const db = getDb();
  const baseUrl = `${req.protocol}://${req.get('host')}`;

  const tags = db.prepare(`
    SELECT t.*, COUNT(at2.archive_id) as archive_count
    FROM tags t
    LEFT JOIN archive_tags at2 ON at2.tag_id = t.id
    GROUP BY t.id
    HAVING archive_count > 0
    ORDER BY t.namespace, t.name
  `).all();

  let entries = '';
  for (const t of tags) {
    const fullName = t.namespace ? `${t.namespace}:${t.name}` : t.name;
    entries += `
  <entry>
    <title>${escapeXml(fullName)}</title>
    <id>urn:manhuaviewer:tag:${t.id}</id>
    <updated>${new Date().toISOString()}</updated>
    <link href="${baseUrl}/opds/tag/${t.id}" rel="subsection" type="application/atom+xml"/>
    <content type="text">${t.archive_count} 个漫画</content>
  </entry>`;
  }

  const xml = `${xmlHeader()}
<feed xmlns="${OPDS_NS}">
  <title>标签</title>
  <id>urn:manhuaviewer:tags</id>
  <updated>${new Date().toISOString()}</updated>
  <link href="${baseUrl}/opds/tags" rel="self" type="application/atom+xml"/>
  <link href="${baseUrl}/opds" rel="start" type="application/atom+xml"/>
  ${entries}
</feed>`;

  res.set('Content-Type', 'application/atom+xml; charset=utf-8');
  res.send(xml);
});

/**
 * OPDS 按标签浏览
 */
router.get('/opds/tag/:tagId', (req, res) => {
  const db = getDb();
  const baseUrl = `${req.protocol}://${req.get('host')}`;
  const tagId = parseInt(req.params.tagId);

  const tag = db.prepare('SELECT * FROM tags WHERE id = ?').get(tagId);
  if (!tag) return res.status(404).send('Not found');

  const fullName = tag.namespace ? `${tag.namespace}:${tag.name}` : tag.name;

  const archives = db.prepare(`
    SELECT a.*
    FROM archives a
    INNER JOIN archive_tags at2 ON at2.archive_id = a.id
    WHERE at2.tag_id = ?
    ORDER BY a.title ASC
  `).all(tagId);

  let entries = '';
  for (const a of archives) {
    const updated = a.updated_at || a.created_at || new Date().toISOString();
    entries += `
  <entry>
    <title>${escapeXml(a.title)}</title>
    <id>urn:manhuaviewer:archive:${a.id}</id>
    <updated>${updated}Z</updated>
    <link href="${baseUrl}/opds/archive/${a.id}" rel="subsection" type="application/atom+xml"/>
    <link href="${baseUrl}/api/archives/${a.id}/cover" rel="http://opds-spec.org/image" type="image/jpeg"/>
    <link href="${baseUrl}/api/archives/${a.id}/cover" rel="http://opds-spec.org/image/thumbnail" type="image/jpeg"/>
    <content type="text">${a.page_count} 页</content>
  </entry>`;
  }

  const xml = `${xmlHeader()}
<feed xmlns="${OPDS_NS}">
  <title>${escapeXml(fullName)}</title>
  <id>urn:manhuaviewer:tag:${tagId}</id>
  <updated>${new Date().toISOString()}</updated>
  <link href="${baseUrl}/opds/tag/${tagId}" rel="self" type="application/atom+xml"/>
  <link href="${baseUrl}/opds/tags" rel="up" type="application/atom+xml"/>
  ${entries}
</feed>`;

  res.set('Content-Type', 'application/atom+xml; charset=utf-8');
  res.send(xml);
});

function escapeXml(str) {
  if (!str) return '';
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&apos;');
}

module.exports = router;
