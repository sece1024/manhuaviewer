// 生产模式下 Tauri 前端通过资源协议加载，需要用绝对 URL 访问 Axum API
const isTauriProd = window.__TAURI__ !== undefined && !window.location.port;
const API_ORIGIN = isTauriProd ? 'http://127.0.0.1:5002' : '';
const BASE = `${API_ORIGIN}/api`;

// 导出 helpers 供外部使用
export const apiOrigin = () => API_ORIGIN;
export const apiBase = () => BASE;

// 将后端返回的相对路径 URL 补全为可用的绝对 URL
function fixUrl(url) {
  if (!API_ORIGIN || !url || !url.startsWith('/')) return url;
  return `${API_ORIGIN}${url}`;
}
const MAX_RETRIES = 3;
const RETRY_DELAY = 500; // 500ms base delay

// --- GET 请求内存缓存 ---
// 缓存以 request() 收到的相对路径为 key（如 '/archives?limit=50&page=1'），
// 失效时必须用同样的相对路径前缀（如 '/archives'）去 _invalidate。
const _cache = new Map();       // key -> { data, ts }
const _inflight = new Map();    // key -> Promise (去重同 URL 的并发请求)
const DEFAULT_TTL = 30_000;     // 默认 30s
const MAX_CACHE_ENTRIES = 200;  // 防止搜索/翻页等变化 URL 无限累积

// 特定端点的 TTL 配置（key 与缓存 key 一致：相对路径）
const _ttlConfig = {
  '/settings': 60_000,
  '/tags': 60_000,
};

function _getTtl(url) {
  // 精确匹配优先
  if (_ttlConfig[url]) return _ttlConfig[url];
  // 前缀匹配
  for (const [prefix, ttl] of Object.entries(_ttlConfig)) {
    if (url.startsWith(prefix + '?') || url.startsWith(prefix + '/')) return ttl;
  }
  return DEFAULT_TTL;
}

// 匹配实际图片端点：/api/archives/{id}/pages/{idx} 或 .../thumb
const _imagePageRe = /\/pages\/\d+(\/thumb)?$/;
function _isCacheable(url, options) {
  const method = (options?.method || 'GET').toUpperCase();
  return method === 'GET' && !_imagePageRe.test(url); // 页面图片请求走浏览器缓存
}

function _getCached(url) {
  const entry = _cache.get(url);
  if (entry && Date.now() - entry.ts < _getTtl(url)) return entry.data;
  if (entry) _cache.delete(url);
  return null;
}

function _setCache(url, data) {
  _cache.set(url, { data, ts: Date.now() });
  // 超出上限时逐出最旧条目（Map 保持插入顺序）
  if (_cache.size > MAX_CACHE_ENTRIES) {
    for (const oldest of _cache.keys()) {
      _cache.delete(oldest);
      if (_cache.size <= MAX_CACHE_ENTRIES) break;
    }
  }
}

function _invalidate(pattern) {
  for (const key of _cache.keys()) {
    if (key.includes(pattern)) _cache.delete(key);
  }
}

async function request(url, options = {}) {
  const method = (options.method || 'GET').toUpperCase();
  const isIdempotent = method === 'GET';
  const maxAttempts = isIdempotent ? MAX_RETRIES : 1;

  // GET 请求：检查缓存 + in-flight dedup
  if (isIdempotent && _isCacheable(url, options)) {
    const cached = _getCached(url);
    if (cached !== null) return cached;

    // 同一 URL 正在请求中，复用 Promise
    if (_inflight.has(url)) return _inflight.get(url);

    const promise = _doFetch(url, options, maxAttempts);
    _inflight.set(url, promise);
    try {
      const result = await promise;
      _setCache(url, result);
      return result;
    } finally {
      _inflight.delete(url);
    }
  }

  return _doFetch(url, options, maxAttempts);
}

async function _doFetch(url, options, maxAttempts) {
  let lastError;

  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    try {
      const res = await fetch(`${BASE}${url}`, {
        ...(options.body && { headers: { 'Content-Type': 'application/json' } }),
        ...options,
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error || `HTTP ${res.status}`);
      }
      return res.json();
    } catch (err) {
      lastError = err;

      // Only retry on connection errors for idempotent requests
      if (maxAttempts > 1 && attempt < maxAttempts - 1 &&
          (err.message.includes('Failed to fetch') ||
           err.message.includes('ECONNREFUSED') ||
           err.message.includes('NetworkError'))) {
        await new Promise(resolve => setTimeout(resolve, RETRY_DELAY * (attempt + 1)));
        continue;
      }

      throw err;
    }
  }

  throw lastError;
}

const api = {
  // Direct open
  openFile: (filePath) =>
    request('/open', { method: 'POST', body: JSON.stringify({ filePath }) }).then(r => { _invalidate('/archives'); return r; }),

  // CBZ export
  listCbz: () => request('/cbz/list'),

  // Archives
  getArchives: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return request(`/archives${qs ? '?' + qs : ''}`).then(archives =>
      archives.map(a => ({ ...a, cover_url: a.cover_url ? fixUrl(a.cover_url) : `${BASE}/archives/${a.id}/cover` }))
    );
  },
  getPages: (archiveId) => request(`/archives/${archiveId}/pages`).then(data => ({
    ...data,
    pages: data.pages.map(p => ({ ...p, url: fixUrl(p.url), thumb_url: fixUrl(p.thumb_url) })),
  })),
  deleteArchive: (id) =>
    request(`/archives/${id}`, { method: 'DELETE' }).then(r => { _invalidate('/archives'); _invalidate('/history'); return r; }),
  batchDeleteArchives: (ids) =>
    request('/archives/batch-delete', { method: 'POST', body: JSON.stringify({ ids }) })
      .then(r => { _invalidate('/archives'); _invalidate('/history'); return r; }),
  updateTitle: (id, title) =>
    request(`/archives/${id}/title`, { method: 'PUT', body: JSON.stringify({ title }) })
      .then(r => { _invalidate('/archives'); return r; }),
  mergeArchives: (archiveIds) =>
    request('/merge', { method: 'POST', body: JSON.stringify({ archive_ids: archiveIds }) })
      .then(r => { _invalidate('/archives'); return r; }),
  getGroupChapters: (groupId) =>
    request(`/archives?group_id=${groupId}`),

  // History
  getHistory: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return request(`/history${qs ? '?' + qs : ''}`).then(res => ({
      items: (res.items || []).map(h => ({ ...h, cover_url: fixUrl(h.cover_url) })),
      total: res.total ?? 0,
    }));
  },
  saveHistory: (archive_id, page_index, total_pages) =>
    request('/history', { method: 'POST', body: JSON.stringify({ archive_id, page_index, total_pages }) })
      .then(r => { _invalidate('/history'); return r; }),
  deleteHistory: (archiveId) =>
    request(`/history/${archiveId}`, { method: 'DELETE' }).then(r => { _invalidate('/history'); return r; }),
  clearHistory: () =>
    request('/history', { method: 'DELETE' }).then(r => { _invalidate('/history'); return r; }),

  // Tags
  getTags: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return request(`/tags${qs ? '?' + qs : ''}`);
  },
  getArchiveTags: (archiveId) => request(`/archives/${archiveId}/tags`),
  createTag: (data) =>
    request('/tags', { method: 'POST', body: JSON.stringify(data) }).then(r => { _invalidate('/tags'); _invalidate('/archives'); return r; }),
  updateTag: (id, data) =>
    request(`/tags/${id}`, { method: 'PUT', body: JSON.stringify(data) }).then(r => { _invalidate('/tags'); _invalidate('/archives'); return r; }),
  deleteTag: (id) =>
    request(`/tags/${id}`, { method: 'DELETE' }).then(r => { _invalidate('/tags'); _invalidate('/archives'); return r; }),
  assignTag: (archive_id, tag_id) =>
    request('/tags/assign', { method: 'POST', body: JSON.stringify({ archive_id, tag_id }) }).then(r => { _invalidate('/tags'); _invalidate('/archives'); _invalidate(`/archives/${archive_id}/tags`); return r; }),
  removeTag: (archiveId, tagId) =>
    request(`/tags/${archiveId}/${tagId}`, { method: 'DELETE' }).then(r => { _invalidate('/tags'); _invalidate('/archives'); _invalidate(`/archives/${archiveId}/tags`); return r; }),
  batchAssignTag: (archiveIds, tagId) =>
    request('/tags/batch-assign', { method: 'POST', body: JSON.stringify({ archive_ids: archiveIds, tag_id: tagId }) })
      .then(r => { _invalidate('/tags'); _invalidate('/archives'); return r; }),
  batchRemoveTag: (archiveIds, tagId) =>
    request('/tags/batch-remove', { method: 'POST', body: JSON.stringify({ archive_ids: archiveIds, tag_id: tagId }) })
      .then(r => { _invalidate('/tags'); _invalidate('/archives'); return r; }),

  // Categories
  getCategories: () => request('/categories'),
  getArchiveCategories: (archiveId) => request(`/archives/${archiveId}/categories`),
  createCategory: (data) =>
    request('/categories', { method: 'POST', body: JSON.stringify(data) }).then(r => { _invalidate('/categories'); return r; }),
  updateCategory: (id, data) =>
    request(`/categories/${id}`, { method: 'PUT', body: JSON.stringify(data) }).then(r => { _invalidate('/categories'); return r; }),
  deleteCategory: (id) =>
    request(`/categories/${id}`, { method: 'DELETE' }).then(r => { _invalidate('/categories'); return r; }),
  assignCategory: (archive_id, category_id) =>
    request('/categories/assign', { method: 'POST', body: JSON.stringify({ archive_id, category_id }) }).then(r => { _invalidate('/categories'); _invalidate('/archives'); return r; }),
  removeCategory: (archiveId, categoryId) =>
    request(`/categories/${archiveId}/${categoryId}`, { method: 'DELETE' }).then(r => { _invalidate('/categories'); _invalidate('/archives'); return r; }),
  batchAssignCategory: (archiveIds, categoryId) =>
    request('/categories/batch-assign', { method: 'POST', body: JSON.stringify({ archive_ids: archiveIds, category_id: categoryId }) })
      .then(r => { _invalidate('/categories'); _invalidate('/archives'); return r; }),
  batchRemoveCategory: (archiveIds, categoryId) =>
    request('/categories/batch-remove', { method: 'POST', body: JSON.stringify({ archive_ids: archiveIds, category_id: categoryId }) })
      .then(r => { _invalidate('/categories'); _invalidate('/archives'); return r; }),

  // Settings
  getSettings: () => request('/settings'),
  updateSettings: (data) =>
    request('/settings', { method: 'PUT', body: JSON.stringify(data) }).then(r => { _invalidate('/settings'); return r; }),
  getStats: () => request('/stats'),

  // Backup & Restore
  exportBackup: () => `${BASE}/backup`,
  importBackup: (data) =>
    request('/restore', { method: 'POST', body: JSON.stringify(data) }).then(r => {
      _invalidate('/archives'); _invalidate('/tags'); _invalidate('/categories'); _invalidate('/history');
      return r;
    }),

  // CBZ 打包归档
  packCbz: (folderPath, outputDir) =>
    request('/archives/pack-cbz', { method: 'POST', body: JSON.stringify({ folderPath, outputDir }) })
      .then(r => { _invalidate('/archives'); return r; }),
};

export default api;
