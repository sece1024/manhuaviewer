import React, { useState, useEffect } from 'react';
import { BrowserRouter as Router, Routes, Route, NavLink, useLocation } from 'react-router-dom';
import Library from './pages/Library';
import Reader from './pages/Reader';
import History from './pages/History';
import Settings from './pages/Settings';
import { ToastProvider } from './components/Toast';
import { SettingsProvider } from './hooks/useSettings';
import { TagsProvider } from './hooks/useTags';
import ErrorBoundary from './components/ErrorBoundary';

function AppContent() {
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') || 'dark');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const location = useLocation();

  useEffect(() => { setSidebarOpen(false); }, [location.pathname]);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }, [theme]);

  return (
    <div className="app-layout">
      <div
        className={`sidebar-overlay ${sidebarOpen ? 'visible' : ''}`}
        onClick={() => setSidebarOpen(false)}
      />

      <aside className={`sidebar ${sidebarOpen ? 'open' : ''}`}>
        <div className="sidebar-brand">Manga<span>Viewer</span></div>
        <nav className="sidebar-nav">
          <NavLink to="/" end>
            <span className="nav-icon">📚</span>
            <span>漫画库</span>
          </NavLink>
          <NavLink to="/collection">
            <span className="nav-icon">📦</span>
            <span>文件夹</span>
          </NavLink>
          <NavLink to="/history">
            <span className="nav-icon">📖</span>
            <span>历史</span>
          </NavLink>
          <NavLink to="/settings">
            <span className="nav-icon">⚙️</span>
            <span>设置</span>
          </NavLink>
        </nav>
        <div className="sidebar-footer">
          <select value={theme} onChange={(e) => setTheme(e.target.value)} style={{ width: '100%' }} aria-label="主题切换">
            <option value="light">☀️ 浅色</option>
            <option value="dark">🌙 深色</option>
            <option value="eye-care">🌿 护眼</option>
          </select>
        </div>
      </aside>

      <main className="main-content">
        <Routes>
          <Route path="/" element={<ErrorBoundary><Library /></ErrorBoundary>} />
          <Route path="/collection" element={<ErrorBoundary><Library mode="collection" /></ErrorBoundary>} />
          <Route path="/reader/:archiveId" element={<ErrorBoundary><Reader /></ErrorBoundary>} />
          <Route path="/history" element={<ErrorBoundary><History /></ErrorBoundary>} />
          <Route path="/settings" element={<ErrorBoundary><Settings /></ErrorBoundary>} />
        </Routes>
      </main>
    </div>
  );
}

function App() {
  return (
    <ErrorBoundary>
      <ToastProvider>
        <SettingsProvider>
          <TagsProvider>
            <Router>
              <AppContent />
            </Router>
          </TagsProvider>
        </SettingsProvider>
      </ToastProvider>
    </ErrorBoundary>
  );
}

export default App;
