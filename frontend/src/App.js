import React, { useState, useEffect, Suspense, lazy } from 'react';
import { BrowserRouter as Router, Routes, Route, NavLink, useLocation } from 'react-router-dom';
import Library from './pages/Library';
import { ToastProvider } from './components/Toast';
import { SettingsProvider } from './hooks/useSettings';
import useSettings from './hooks/useSettings';
import { TagsProvider } from './hooks/useTags';
import ErrorBoundary from './components/ErrorBoundary';
import api, { localStorageGet, localStorageSet } from './utils/api';

// 非首屏页面按需加载，减小首屏 bundle
const Reader = lazy(() => import('./pages/Reader'));
const History = lazy(() => import('./pages/History'));
const Settings = lazy(() => import('./pages/Settings'));

const PageFallback = () => (
  <div className="empty-state">
    <div className="empty-state-icon">⏳</div>
    <div className="empty-state-text">加载中...</div>
  </div>
);

function AppContent() {
  // localStorage 读取走 try/catch（隐私模式/存储禁用时不抛错，回退默认主题）
  const [theme, setTheme] = useState(() => localStorageGet('theme') || 'dark');
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const location = useLocation();
  const { settings } = useSettings();
  const [lanUrls, setLanUrls] = useState([]);

  // 局域网模式（server_bind=0.0.0.0）开启后，在侧边栏常驻显示本机可访问地址
  useEffect(() => {
    let cancelled = false;
    api.getLanIps()
      .then(info => {
        if (cancelled) return;
        setLanUrls((info?.ipv4 || []).map(ip => `http://${ip}:${info.port}/`));
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  const lanActive = settings.server_bind === '0.0.0.0';

  useEffect(() => { setSidebarOpen(false); }, [location.pathname]);

  // 阅读器全屏沉浸：底部导航在 reader 路由下隐藏（工具栏自带"← 返回书库"）
  const isReader = location.pathname.startsWith('/reader/');

  useEffect(() => {
    const mainEl = document.querySelector('.main-content');
    if (mainEl) mainEl.scrollTop = 0;
  }, [location.pathname]);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorageSet('theme', theme);
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
          {lanActive && lanUrls.length > 0 && (
            <a
              href={lanUrls[0]}
              target="_blank"
              rel="noreferrer"
              title="局域网访问地址（手机/平板浏览器打开）"
              style={{
                display: 'block', marginTop: 8, fontSize: 11, fontFamily: 'monospace',
                color: 'var(--accent)', wordBreak: 'break-all', textDecoration: 'none',
              }}
            >
              🌐 {lanUrls[0]}
            </a>
          )}
        </div>
      </aside>

      <main className="main-content">
        <Suspense fallback={<PageFallback />}>
          <Routes>
            <Route path="/" element={<ErrorBoundary><Library /></ErrorBoundary>} />
            <Route path="/collection" element={<ErrorBoundary><Library mode="collection" /></ErrorBoundary>} />
            <Route path="/reader/:archiveId" element={<ErrorBoundary><Reader /></ErrorBoundary>} />
            <Route path="/history" element={<ErrorBoundary><History /></ErrorBoundary>} />
            <Route path="/settings" element={<ErrorBoundary><Settings /></ErrorBoundary>} />
          </Routes>
        </Suspense>
      </main>

      {/* 移动端底部导航：仅 ≤768px 显示（CSS），reader 路由下隐藏保持沉浸 */}
      {!isReader && (
        <nav className="mobile-bottom-bar" aria-label="主导航">
          <NavLink to="/" end>
            <span className="nav-icon">📚</span>
            <span>书库</span>
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
      )}
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
