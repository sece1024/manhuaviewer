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
// 导出供 App/useSettings 复用：隐私模式/存储被禁用时读写会抛错，统一捕获，
// 避免初始化崩溃（ErrorBoundary 全屏）或主题切换时 effect 抛错卸载整棵树。
export function localStorageGet(k) {
  try { return window.localStorage.getItem(k) || ''; } catch (e) { return ''; }
}
export function localStorageSet(k, v) {
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

// ---- 跨路由浏览会话（Library）失效 ----
// Library 卸载时会把当前列表写进模块级会话缓存，返回时先恢复旧列表再后台比对，
// 这是“秒开”的来源。但写操作（扫描/删除/导入等）会改变成员集合：若不在这里一并
// 作废会话，用户从设置页扫描完再回到书库，会先看到已被清理的档案名，甚至（比对
// 失败或中途切页时）被再次写回，表现为“删掉的漫画一直在”。
//
// 用一个单调递增的代际号而不是回调注册：调用方（useLibrarySession）只需比较自己
// 记住的代际与当前值，无需在模块加载期注册监听，也就不会受模块加载顺序影响。
let _membershipGeneration = 0;

/// 当前档案成员代际；每次影响列表成员的写操作都会 +1。
export function membershipGeneration() {
  return _membershipGeneration;
}

/// 作废浏览会话（仅递增代际，避免直接触碰调用方的缓存对象）。
/// 影响成员集合的写操作在 `_invalidate('/archives')` 里自动调用；
/// 导出供需要在测试/特殊流程中显式作废的调用方使用。
export function invalidateLibrarySessions() {
  _membershipGeneration += 1;
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
  // 档案成员集合可能已变化：浏览会话不能再用旧列表秒开
  if (_matchesPattern('/archives', pattern)) _membershipGeneration += 1;
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
        // 401 = 局域网口令缺失/错误：让 App 弹出口令输入（桌面端回环请求不会 401）
        if (res.status === 401) {
          try {
            window.dispatchEvent(new CustomEvent('mv:auth-required'));
          } catch (e) { /* 忽略 */ }
        }
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
  // 扫描进度：轮询用，跳过缓存与 in-flight 去重
  scanStatus: () => request('/scan/status', { cache: false }),
  scanCancel: () => request('/scan/cancel', { method: 'POST' }),

  // 批量转换为 CBZ（后台任务 + 进度 + 取消；成功后删除原文件）。
  // 传 ids 时只转换这些档案，否则转换全部可转换档案。
  convertCbzStart: (ids) =>
    request('/archives/convert-cbz/start', {
      method: 'POST',
      body: JSON.stringify(ids && ids.length ? { ids } : {}),
    }).then(r => {
      // 转换会改变档案路径/类型，作废书库缓存
      _invalidate('/archives');
      return r;
    }),
  convertCbzStatus: () => request('/archives/convert-cbz/status', { cache: false }),
  convertCbzCancel: () => request('/archives/convert-cbz/cancel', { method: 'POST' }),

  // CBZ export
  listCbz: () => request('/cbz/list'),

  // Archives
  getArchives: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return request(`/archives${qs ? '?' + qs : ''}`).then(archives =>
      archives.map(a => ({ ...a, cover_url: a.cover_url ? fixUrl(a.cover_url) : `${BASE}/archives/${a.id}/cover` }))
    );
  },
  // 按添加日期的年/月聚合（侧栏"日期"树）。缓存键 /archives/added-tree 落在
  // _invalidate('/archives') 的前缀规则内：任何档案写操作都会自动作废它。
  getAddedTree: () => request('/archives/added-tree'),
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
  syncPlan: (payload) => request('/sync/plan', { method: 'POST', body: JSON.stringify(payload) }),
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
