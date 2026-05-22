const BASE = '/api';

async function request(url, options = {}) {
  const res = await fetch(`${BASE}${url}`, {
    headers: { 'Content-Type': 'application/json' },
    ...options,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `HTTP ${res.status}`);
  }
  return res.json();
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
    return request(`/archives${qs ? '?' + qs : ''}`);
  },
  getArchive: (id) => request(`/archives/${id}`),
  getPages: (archiveId) => request(`/archives/${archiveId}/pages`),
  pageUrl: (archiveId, pageIndex) => `${BASE}/archives/${archiveId}/pages/${pageIndex}`,
  pageThumbUrl: (archiveId, pageIndex) => `${BASE}/archives/${archiveId}/pages/${pageIndex}/thumb`,
  coverUrl: (archiveId) => `${BASE}/archives/${archiveId}/cover`,
  deleteArchive: (id) => request(`/archives/${id}`, { method: 'DELETE' }),

  // History
  getHistory: () => request('/history'),
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
};

export default api;
