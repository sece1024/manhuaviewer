// 生产模式下 Tauri 前端通过资源协议加载，需要用绝对 URL 访问 Axum API
const isTauriProd = window.__TAURI__ !== undefined && !window.location.port;
const API_ORIGIN = isTauriProd ? 'http://127.0.0.1:5002' : '';
const BASE = `${API_ORIGIN}/api`;

// 导出 helpers 供外部使用
export const apiOrigin = () => API_ORIGIN;
export const apiBase = () => BASE;

// ---- 局域网访问口令（可配置鉴权）----
// 口令在桌面端“设置→局域网”里生成；这里仅负责把它附到请求头上。
// 用 localStorage 保存便于跨页面/跨设备会话使用（桌面端由设置页同步）。
const TOKEN_KEY = 'mv_server_token';
function localStorageGet(k) {
  try { return window.localStorage.getItem(k) || ''; } catch (e) { return ''; }
}
function localStorageSet(k, v) {
  try { if (v) window.localStorage.setItem(k, v); else window.localStorage.removeItem(k); } catch (e) { /* 忽略 */ }
}
let _serverToken = localStorageGet(TOKEN_KEY);
export const getServerToken = () => _serverToken;
export function setServerToken(token) {
  _serverToken = (token || '').trim();
  localStorageSet(TOKEN_KEY, _serverToken);
}

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
let _generation = 0;            // 每次失效 +1：in-flight 响应落地时比对，跳过失效后的旧数据回写
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

/// 失效匹配：同一端点（可带查询串）及其子路径；不做子串匹配，避免误伤
/// `/archives/5/pages` 这类看起来“包含”但语义不同的缓存键。
function _matchesPattern(key, pattern) {
  return key === pattern || key.startsWith(pattern + '/') || key.startsWith(pattern + '?');
}

function _invalidate(pattern) {
  _generation += 1; // 使所有 in-flight GET 的缓存回写失效（见 request() 的代数比对）
  for (const key of _cache.keys()) {
    if (_matchesPattern(key, pattern)) _cache.delete(key);
  }
  // 同步清除仍在途的同 URL 请求：失效后新发起的请求会重新拉取，
  // 而不是复用失效前发出、即将返回的旧结果。
  for (const key of _inflight.keys()) {
    if (_matchesPattern(key, pattern)) _inflight.delete(key);
  }
}

/// 只失效“端点本身及其查询串”的缓存（不含子路径）。
/// 用于阅读进度这类只影响列表/卡片、不影响 `/archives/{id}/pages` 等子资源的变化，
/// 避免翻一次页就清掉整份页面清单与书签缓存。
function _invalidateQuery(pattern) {
  _generation += 1;
  for (const key of _cache.keys()) {
    if (key === pattern || key.startsWith(pattern + '?')) _cache.delete(key);
  }
  for (const key of _inflight.keys()) {
    if (key === pattern || key.startsWith(pattern + '?')) _inflight.delete(key);
  }
}

async function request(url, options = {}) {
  const method = (options.method || 'GET').toUpperCase();
  const isIdempotent = method === 'GET';
  const maxAttempts = isIdempotent ? MAX_RETRIES : 1;
  // 显式跳过缓存（如轮询进度接口），同时不与其他 GET 做 inflight 去重
  const noCache = options.cache === false;

  // GET 请求：检查缓存 + in-flight dedup
  if (isIdempotent && !noCache && _isCacheable(url, options)) {
    const cached = _getCached(url);
    if (cached !== null) return cached;

    // 同一 URL 正在请求中，复用 Promise
    if (_inflight.has(url)) return _inflight.get(url);

    const generation = _generation; // 记录发起时的代数
    const promise = _doFetch(url, options, maxAttempts).then((result) => {
      // 若在请求期间缓存被失效（保存/删除等写操作），不再把旧数据写回
      if (generation === _generation) _setCache(url, result);
      return result;
    });
    _inflight.set(url, promise);
    try {
      return await promise;
    } finally {
      // 仅在仍指向本次请求时清理，避免误删失效后新注册的同 URL 请求
      if (_inflight.get(url) === promise) _inflight.delete(url);
    }
  }

  return _doFetch(url, options, maxAttempts);
}

async function _doFetch(url, options, maxAttempts) {
  let lastError;

  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    try {
      const res = await fetch(`${BASE}${url}`, {
        method: options.method || 'GET',
        headers: {
          ...(options.body ? { 'Content-Type': 'application/json' } : {}),
          ...(_serverToken ? { Authorization: `Bearer ${_serverToken}` } : {}),
        },
        ...(options.body ? { body: options.body } : {}),
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

  // 批量扫描根目录（增量：新增入库、变更更新、磁盘已删除的档案会被清理）
  scan: (path, depth) =>
    request('/scan', { method: 'POST', body: JSON.stringify({ path, depth }) }).then(r => {
      _invalidate('/archives');
      _invalidate('/history');
      return r;
    }),

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
  // 阅读书签（档案内任意页码）
  getBookmarks: (archiveId) =>
    request(`/archives/${archiveId}/bookmarks`).then(r => (r && Array.isArray(r.pages) ? r.pages : [])),
  addBookmark: (archiveId, pageIndex) =>
    request(`/archives/${archiveId}/bookmarks`, { method: 'POST', body: JSON.stringify({ page_index: pageIndex }) })
      .then(r => { _invalidate(`/archives/${archiveId}/bookmarks`); return r; }),
  removeBookmark: (archiveId, pageIndex) =>
    request(`/archives/${archiveId}/bookmarks/${pageIndex}`, { method: 'DELETE' })
      .then(r => { _invalidate(`/archives/${archiveId}/bookmarks`); return r; }),
  deleteArchive: (id) =>
    request(`/archives/${id}`, { method: 'DELETE' }).then(r => { _invalidate('/archives'); _invalidate('/history'); return r; }),
  batchDeleteArchives: (ids) =>
    request('/archives/batch-delete', { method: 'POST', body: JSON.stringify({ ids }) })
      .then(r => { _invalidate('/archives'); _invalidate('/history'); return r; }),
  updateTitle: (id, title) =>
    request(`/archives/${id}/title`, { method: 'PUT', body: JSON.stringify({ title }) })
      .then(r => { _invalidate('/archives'); return r; }),
  // 手动封面：pageIndex=null 恢复默认（首页）
  setArchiveCover: (id, pageIndex) =>
    request(`/archives/${id}/cover`, { method: 'PUT', body: JSON.stringify({ page_index: pageIndex }) })
      .then(r => { _invalidate('/archives'); return r; }),
  // 远程封面 URL：url=null 清除远程封面
  setRemoteCover: (id, url) =>
    request(`/archives/${id}/cover-url`, { method: 'PUT', body: JSON.stringify({ url }) })
      .then(r => { _invalidate('/archives'); return r; }),
  // 元数据搜索（Bangumi）：{ items: [{title,cover,score,tags,source_id}] }
  metadataSearch: (q) => request(`/metadata/search?q=${encodeURIComponent(q)}`),
  regenerateTitles: () =>
    request('/archives/regenerate-titles', { method: 'POST' })
      .then(r => { _invalidate('/archives'); _invalidate('/history'); return r; }),
  mergeArchives: (archiveIds) =>
    request('/merge', { method: 'POST', body: JSON.stringify({ archive_ids: archiveIds }) })
      .then(r => { _invalidate('/archives'); return r; }),
  getGroupChapters: (groupId) =>
    request(`/archives?group_id=${groupId}`).then(archives =>
      archives.map(a => ({ ...a, cover_url: a.cover_url ? fixUrl(a.cover_url) : `${BASE}/archives/${a.id}/cover` }))
    ),
  getArchivesByTitle: (title, parent) =>
    request(`/archives?title=${encodeURIComponent(title)}&parent=${encodeURIComponent(parent || '')}`).then(archives =>
      archives.map(a => ({ ...a, cover_url: a.cover_url ? fixUrl(a.cover_url) : `${BASE}/archives/${a.id}/cover` }))
    ),

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
      .then(r => {
        _invalidate('/history');
        // 阅读进度只影响书库列表内容（卡片进度条、“最近阅读”排序）；
        // 用查询级失效，避免把 /archives/{id}/pages 与书签缓存一起清掉
        _invalidateQuery('/archives');
        return r;
      }),
  deleteHistory: (archiveId) =>
    request(`/history/${archiveId}`, { method: 'DELETE' }).then(r => {
      _invalidate('/history');
      _invalidateQuery('/archives');
      return r;
    }),
  clearHistory: () =>
    request('/history', { method: 'DELETE' }).then(r => {
      _invalidate('/history');
      _invalidateQuery('/archives');
      return r;
    }),

  // 跨机同步（详见“设置 → 同步”）
  syncStart: (payload) => request('/sync/start', { method: 'POST', body: JSON.stringify(payload) }),
  syncStatus: () => request('/sync/status', { cache: false }),
  syncCancel: () => request('/sync/cancel', { method: 'POST' }),

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
  // 本机局域网可达 IPv4 + 端口：{ ipv4: [...], port }
  getLanIps: () => request('/lan-ip'),
  // 更新检查：{ current, latest, update_available, release_url }
  checkUpdate: () => request('/update/check'),

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
