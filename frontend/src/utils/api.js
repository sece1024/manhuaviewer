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

  // Folders
  getFolders: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return request(`/folders${qs ? '?' + qs : ''}`);
  },

  // Images
  getImages: (folderId) => request(`/folders/${folderId}/images`),
  imageUrl: (id) => `${BASE}/images/${id}`,
  thumbUrl: (id) => `${BASE}/images/${id}/thumbnail`,

  // History
  getHistory: () => request('/history'),
  saveHistory: (folder_id, page_index, total_pages) =>
    request('/history', { method: 'POST', body: JSON.stringify({ folder_id, page_index, total_pages }) }),
  deleteHistory: (folderId) => request(`/history/${folderId}`, { method: 'DELETE' }),

  // Tags
  getTags: () => request('/tags'),
  createTag: (name, color) => request('/tags', { method: 'POST', body: JSON.stringify({ name, color }) }),
  updateTag: (id, data) => request(`/tags/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
  deleteTag: (id) => request(`/tags/${id}`, { method: 'DELETE' }),
  assignTag: (folder_id, tag_id) => request('/tags/assign', { method: 'POST', body: JSON.stringify({ folder_id, tag_id }) }),
  removeFolderTag: (folderId, tagId) => request(`/tags/${folderId}/${tagId}`, { method: 'DELETE' }),

  // Settings
  getSettings: () => request('/settings'),
  updateSettings: (data) => request('/settings', { method: 'PUT', body: JSON.stringify(data) }),
};

export default api;
