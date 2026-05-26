// 生产模式下 Tauri 前端通过资源协议加载，需要用绝对 URL 访问 Axum API
const isTauriProd = window.__TAURI__ !== undefined && !window.location.port;
const API_ORIGIN = isTauriProd ? 'http://127.0.0.1:5002' : '';
const BASE = `${API_ORIGIN}/api`;

// 将后端返回的相对路径 URL 补全为可用的绝对 URL
function fixUrl(url) {
  if (!API_ORIGIN || !url || !url.startsWith('/')) return url;
  return `${API_ORIGIN}${url}`;
}
const MAX_RETRIES = 10;
const RETRY_DELAY = 1000; // 1 second

async function request(url, options = {}) {
  let lastError;
  
  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    try {
      const res = await fetch(`${BASE}${url}`, {
        headers: { 'Content-Type': 'application/json' },
        ...options,
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error || `HTTP ${res.status}`);
      }
      return res.json();
    } catch (err) {
      lastError = err;
      
      // Only retry on connection errors
      if (err.message.includes('Failed to fetch') || 
          err.message.includes('ECONNREFUSED') ||
          err.message.includes('NetworkError')) {
        if (attempt < MAX_RETRIES - 1) {
          await new Promise(resolve => setTimeout(resolve, RETRY_DELAY));
          continue;
        }
      }
      
      // Don't retry for other errors
      throw err;
    }
  }
  
  throw lastError;
}

const api = {
  // Config
  getConfig: () => request('/config'),
  updateConfig: (root_dir) => request('/config', { method: 'PUT', body: JSON.stringify({ root_dir }) }),

  // Scan
  scan: () => request('/scan', { method: 'POST' }),

  // Direct open
  openFile: (filePath) => request('/open', { method: 'POST', body: JSON.stringify({ filePath }) }),

  // Archives
  getArchives: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return request(`/archives${qs ? '?' + qs : ''}`).then(archives =>
      archives.map(a => ({ ...a, cover_url: a.cover_url ? fixUrl(a.cover_url) : `${BASE}/archives/${a.id}/cover` }))
    );
  },
  getArchive: (id) => request(`/archives/${id}`),
  getPages: (archiveId) => request(`/archives/${archiveId}/pages`).then(data => ({
    ...data,
    pages: data.pages.map(p => ({ ...p, url: fixUrl(p.url), thumb_url: fixUrl(p.thumb_url) })),
  })),
  pageUrl: (archiveId, pageIndex) => `${BASE}/archives/${archiveId}/pages/${pageIndex}`,
  pageThumbUrl: (archiveId, pageIndex) => `${BASE}/archives/${archiveId}/pages/${pageIndex}/thumb`,
  coverUrl: (archiveId) => `${BASE}/archives/${archiveId}/cover`,
  deleteArchive: (id) => request(`/archives/${id}`, { method: 'DELETE' }),

  // History
  getHistory: () => request('/history').then(items =>
    items.map(h => ({ ...h, cover_url: fixUrl(h.cover_url) }))
  ),
  saveHistory: (archive_id, page_index, total_pages) =>
    request('/history', { method: 'POST', body: JSON.stringify({ archive_id, page_index, total_pages }) }),
  deleteHistory: (archiveId) => request(`/history/${archiveId}`, { method: 'DELETE' }),
  clearHistory: () => request('/history', { method: 'DELETE' }),

  // Tags
  getTags: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return request(`/tags${qs ? '?' + qs : ''}`);
  },
  getNamespaces: () => request('/tags/namespaces'),
  createTag: (data) => request('/tags', { method: 'POST', body: JSON.stringify(data) }),
  updateTag: (id, data) => request(`/tags/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  deleteTag: (id) => request(`/tags/${id}`, { method: 'DELETE' }),
  assignTag: (archive_id, tag_id) => request('/tags/assign', { method: 'POST', body: JSON.stringify({ archive_id, tag_id }) }),
  removeTag: (archiveId, tagId) => request(`/tags/${archiveId}/${tagId}`, { method: 'DELETE' }),

  // Categories
  getCategories: () => request('/categories'),
  createCategory: (data) => request('/categories', { method: 'POST', body: JSON.stringify(data) }),
  updateCategory: (id, data) => request(`/categories/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  deleteCategory: (id) => request(`/categories/${id}`, { method: 'DELETE' }),
  assignCategory: (archive_id, category_id) => request('/categories/assign', { method: 'POST', body: JSON.stringify({ archive_id, category_id }) }),
  removeCategory: (archiveId, categoryId) => request(`/categories/${archiveId}/${categoryId}`, { method: 'DELETE' }),

  // Settings
  getSettings: () => request('/settings'),
  updateSettings: (data) => request('/settings', { method: 'PUT', body: JSON.stringify(data) }),
  getStats: () => request('/stats'),

  // Backup & Restore
  exportBackup: () => `${BASE}/backup`,
  importBackup: (data) => request('/restore', { method: 'POST', body: JSON.stringify(data) }),

  // CBZ 打包归档
  packCbz: (folderPath, outputDir) => request('/archives/pack-cbz', {
    method: 'POST',
    body: JSON.stringify({ folderPath, outputDir }),
  }),
};

export default api;
