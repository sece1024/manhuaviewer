import React, { useState, useEffect } from 'react';
import { BrowserRouter as Router, Routes, Route, NavLink, useLocation } from 'react-router-dom';
import FolderList from './pages/FolderList';
import Reader from './pages/Reader';
import History from './pages/History';
import { ToastProvider } from './components/Toast';

function AppContent() {
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') || 'light');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const location = useLocation();

  // 路由切换时关闭侧边栏
  useEffect(() => {
    setSidebarOpen(false);
  }, [location.pathname]);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  // 检测是否为移动端（无需 state，用 CSS 控制即可，这里只控制 open 状态）

  return (
    <div className="app-layout">
      {/* 移动端遮罩 */}
      <div
        className={`sidebar-overlay ${sidebarOpen ? 'visible' : ''}`}
        onClick={() => setSidebarOpen(false)}
      />

      <aside className={`sidebar ${sidebarOpen ? 'open' : ''}`}>
        <div className="sidebar-brand">Manga<span>Viewer</span></div>
        <nav className="sidebar-nav">
          <NavLink to="/" end>📚 漫画库</NavLink>
          <NavLink to="/history">📖 历史</NavLink>
        </nav>
        <div className="theme-select-wrapper" style={{ padding: '0 16px', marginTop: 'auto' }}>
          <select value={theme} onChange={(e) => setTheme(e.target.value)} style={{ width: '100%' }}>
            <option value="light">☀️ 浅色</option>
            <option value="dark">🌙 深色</option>
            <option value="eye-care">🌿 护眼</option>
          </select>
        </div>
      </aside>

      <main className="main-content">
        <Routes>
          <Route path="/" element={<FolderList />} />
          <Route path="/reader/:folderId" element={<Reader />} />
          <Route path="/history" element={<History />} />
        </Routes>
      </main>
    </div>
  );
}

function App() {
  return (
    <ToastProvider>
      <Router>
        <AppContent />
      </Router>
    </ToastProvider>
  );
}

export default App;
