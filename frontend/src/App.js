import React, { useState, useEffect } from 'react';
import { BrowserRouter as Router, Routes, Route, NavLink } from 'react-router-dom';
import FolderList from './pages/FolderList';
import Reader from './pages/Reader';
import History from './pages/History';
import { ToastProvider } from './components/Toast';

function App() {
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') || 'light');

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  return (
    <ToastProvider>
      <Router>
        <div className="app-layout">
          <aside className="sidebar">
            <div className="sidebar-brand">Manga<span>Viewer</span></div>
            <nav className="sidebar-nav">
              <NavLink to="/" end>📚 漫画库</NavLink>
              <NavLink to="/history">📖 阅读历史</NavLink>
            </nav>
            <div style={{ padding: '0 16px', marginTop: 'auto' }}>
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
      </Router>
    </ToastProvider>
  );
}

export default App;
