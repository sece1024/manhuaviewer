import { useState, useEffect, Suspense, lazy } from 'react';
import { BrowserRouter as Router, Routes, Route, NavLink, Navigate, useLocation } from 'react-router-dom';
import Library from './pages/Library';
import { ToastProvider } from './components/Toast';
import { SettingsProvider } from './hooks/useSettings';
import useSettings from './hooks/useSettings';
import { TagsProvider } from './hooks/useTags';
import ErrorBoundary from './components/ErrorBoundary';
import Modal from './components/Modal';
import JobIndicator from './components/JobIndicator';
import CommandPalette from './components/CommandPalette';
import api, { localStorageGet, localStorageSet, setServerToken } from './utils/api';

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
  // 侧边栏收起：iPad/小屏上 240px 的导航栏会吃掉近三成宽度，收成图标窄栏把空间还给内容。
  // 纯客户端偏好（按设备记忆）：iPad 需要收，桌面外接大屏往往不需要，放服务端设置反而别扭。
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => localStorageGet('sidebar_collapsed') === '1');
  const location = useLocation();
  const { settings } = useSettings();
  const [lanUrls, setLanUrls] = useState([]);
  // 局域网口令登录弹窗：api.js 在任何请求收到 401 时派发事件（桌面端回环不会 401）
  // 命令面板：全局入口，任何页面都能开（含阅读器——它是"我来错地方了"的逃生门）
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [tokenOpen, setTokenOpen] = useState(false);
  const [tokenInput, setTokenInput] = useState('');

  useEffect(() => {
    const onAuthRequired = () => setTokenOpen(true);
    window.addEventListener('mv:auth-required', onAuthRequired);
    return () => window.removeEventListener('mv:auth-required', onAuthRequired);
  }, []);

  // ⌘K / Ctrl-K 开面板。用 e.key 同时认 'k' 与 'K'，并 preventDefault 掉浏览器
  // 自带的 Ctrl-K（Chrome 是"搜索"、Safari 是"聚焦地址栏"）
  useEffect(() => {
    const onKeyDown = (e) => {
      if ((e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'K')) {
        e.preventDefault();
        setPaletteOpen(v => !v);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const handleTokenSubmit = () => {
    const token = tokenInput.trim();
    if (!token) return;
    // 注意：setServerToken 是具名导出，不在 api 默认导出对象上（此前写成 api.setServerToken
    // 会抛 TypeError，导致口令保存与随后的 reload 都不执行）
    setServerToken(token);
    // 刷新使所有请求带上 Authorization 头，并重置缓存/页面状态
    window.location.reload();
  };

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

  // 阅读器全屏沉浸：底部导航与侧边栏都隐藏，整块屏幕留给漫画（工具栏自带"← 返回书库"）
  const isReader = location.pathname.startsWith('/reader/');

  // 收起状态按设备记忆（与主题同为纯客户端偏好）
  useEffect(() => {
    localStorageSet('sidebar_collapsed', sidebarCollapsed ? '1' : '0');
  }, [sidebarCollapsed]);

  useEffect(() => {
    const mainEl = document.querySelector('.main-content');
    if (mainEl) mainEl.scrollTop = 0;
  }, [location.pathname]);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorageSet('theme', theme);
  }, [theme]);

  return (
    <div className={`app-layout ${sidebarCollapsed ? 'sidebar-collapsed' : ''} ${isReader ? 'reader-immersive' : ''}`}>
      <div
        className={`sidebar-overlay ${sidebarOpen ? 'visible' : ''}`}
        onClick={() => setSidebarOpen(false)}
      />

      <aside className={`sidebar ${sidebarOpen ? 'open' : ''}`}>
        <div className="sidebar-top">
          <div className="sidebar-brand">Manga<span>Viewer</span></div>
          <button
            type="button"
            className="sidebar-toggle"
            onClick={() => setSidebarCollapsed(v => !v)}
            aria-label={sidebarCollapsed ? '展开侧边栏' : '收起侧边栏'}
            aria-expanded={!sidebarCollapsed}
            title={sidebarCollapsed ? '展开侧边栏' : '收起侧边栏，腾出更多阅读空间'}
          >
            {sidebarCollapsed ? '»' : '«'}
          </button>
        </div>
        <button
          type="button"
          className="sidebar-command-entry"
          onClick={() => setPaletteOpen(true)}
          aria-label="打开命令面板"
        >
          <span className="nav-icon">🔍</span>
          <span className="nav-label">命令 / 搜索</span>
          <kbd className="sidebar-command-kbd">⌘K</kbd>
        </button>
        <nav className="sidebar-nav">
          <NavLink to="/" end aria-label="漫画库">
            <span className="nav-icon">📚</span>
            <span className="nav-label">漫画库</span>
          </NavLink>
          <NavLink to="/history" aria-label="历史">
            <span className="nav-icon">📖</span>
            <span className="nav-label">历史</span>
          </NavLink>
          <NavLink to="/settings" aria-label="设置">
            <span className="nav-icon">⚙️</span>
            <span className="nav-label">设置</span>
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
            {/* 旧的 /collection 已合并进统一书库：保留重定向，避免旧书签/深链接 404 */}
            <Route path="/collection" element={<Navigate to="/" replace />} />
            <Route path="/reader/:archiveId" element={<ErrorBoundary><Reader /></ErrorBoundary>} />
            <Route path="/history" element={<ErrorBoundary><History /></ErrorBoundary>} />
            <Route path="/settings" element={<ErrorBoundary><Settings /></ErrorBoundary>} />
          </Routes>
        </Suspense>
      </main>

      {/* 长任务指示器：扫描/同步/转 CBZ 都是分钟级后台任务，进度必须跟着用户走，
          不能只在发起它的设置页可见。阅读器整屏沉浸时隐藏以免盖住画面；设置页本身
          就把三个任务的进度与取消渲染在各自设置项旁边（且读同一份任务层状态），
          再叠一个角标只是同一屏上两份重复信息，故一并排除 */}
      {!isReader && location.pathname !== '/settings' && <JobIndicator />}

      {/* 移动端底部导航：仅 ≤768px 显示（CSS），reader 路由下隐藏保持沉浸 */}
      {!isReader && (
        <nav className="mobile-bottom-bar" aria-label="主导航">
          <NavLink to="/" end>
            <span className="nav-icon">📚</span>
            <span>书库</span>
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

      {paletteOpen && <CommandPalette onClose={() => setPaletteOpen(false)} />}

      {/* 局域网口令登录：配置了口令后，LAN 端一切请求（读+写）都返回 401 触发 */}
      {tokenOpen && (
        <Modal onClose={() => setTokenOpen(false)} ariaLabel="局域网访问口令">
          <div style={{ minWidth: 320 }}>
            <h3 style={{ marginBottom: 8 }}>🔒 需要局域网访问口令</h3>
            <div className="settings-row-desc" style={{ marginBottom: 12 }}>
              该服务配置了访问口令，书库读写、历史与 OPDS 都需要它。输入口令后自动刷新（本机桌面端不受影响，无需输入）。
            </div>
            <input
              type="password"
              value={tokenInput}
              onChange={e => setTokenInput(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && handleTokenSubmit()}
              placeholder="访问口令"
              autoFocus
              style={{ width: '100%' }}
            />
            <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
              <button className="btn btn-primary" onClick={handleTokenSubmit} disabled={!tokenInput.trim()}>保存并刷新</button>
            </div>
          </div>
        </Modal>
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
