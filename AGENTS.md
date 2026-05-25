# AGENTS.md — MangaViewer

## Commands

```bash
# Start both (root): frontend :3000 proxies to backend :5002
pnpm start

# Backend (Legacy Node.js)
pnpm --filter manhuaviewer-backend start
cd backend && pnpm run dev     # nodemon (auto-reload)
cd backend && pnpm test        # Jest — tests in backend/tests/**/*.test.js

# Frontend
pnpm --filter manhuaviewer-frontend start
cd frontend && pnpm start      # CRA dev server
cd frontend && pnpm test       # React Testing Library — tests in frontend/src/__tests__/

# Production build (backend serves frontend/build/)
pnpm run build

# Electron (Legacy - standalone macOS app)
pnpm run electron              # dev: starts Electron with backend
pnpm run build:electron        # packages into .dmg + .zip in out/
pnpm run pack                  # packages directory only (no installer)

# Tauri (New - cross-platform app)
pnpm tauri dev                 # development mode with hot-reload
pnpm tauri build               # production build for current platform
pnpm tauri ios build           # iOS build (requires macOS + Xcode)
pnpm tauri android build       # Android build
```

## Architecture

### Current (Electron - Legacy)
- **Backend**: Express + better-sqlite3 (sync API — never `await` DB calls)
- **Frontend**: React 19 + React Router v7 (CRA)
- **Electron**: `electron/main.js` starts Express on a free port, loads `http://localhost:{port}` in BrowserWindow
- **Database**: SQLite at `backend/data/` (dev) or `~/Library/Application Support/MangaViewer/data/` (Electron)
- **Two archive types**: `folder` (read directory at request time) vs compressed (page list in DB, extract on demand)

### New (Tauri 2.0 - In Progress)
- **Backend**: Rust + Axum (native performance)
- **Frontend**: React 19 (reused from current implementation)
- **Tauri**: `src-tauri/` contains Rust backend and configuration
- **Database**: SQLite via rusqlite at `~/Library/Application Support/MangaViewer/data/`
- **Platforms**: macOS, Windows, Linux, iOS, Android

## Key Conventions

### General
- **pnpm workspace**: root `package.json` scripts use `pnpm --filter` to run backend/frontend. Do NOT use `cd backend && npm start` — it won't find dependencies.
- All frontend API calls go through `frontend/src/utils/api.js` — never fetch directly
- Settings are key-value rows in `settings` table; defaults set in database initialization
- Theme is client-side only (`localStorage` → `data-theme` attribute on `<html>`)
- OPDS routes mount at `/` (not `/api`) — must not conflict with static middleware
- Frontend proxy in dev: `frontend/package.json` → `"proxy": "http://localhost:5002"`

### Tauri Specific
- Rust code lives in `src-tauri/src/` with modules: `db/`, `routes/`, `services/`, `models/`, `utils/`
- API routes mirror the Node.js backend structure for frontend compatibility
- Database schema is in `src-tauri/src/db/schema.rs`
- Tauri configuration: `src-tauri/tauri.conf.json`
- Capabilities/permissions: `src-tauri/capabilities/default.json`
- Build with `pnpm tauri build` — outputs to `src-tauri/target/release/`

## Adding a new API route

### Tauri (New)
1. Add handler function in `src-tauri/src/routes/` appropriate file
2. Register route in `src-tauri/src/routes/mod.rs` using `Router::new().route()`
3. Add methods to `frontend/src/utils/api.js`

### Node.js (Legacy)
1. Create `backend/src/routes/newRoutes.js`
2. `require` and `router.use('/', newRoutes)` in `routes/api.js`
3. Add methods to `frontend/src/utils/api.js`

## Gotchas

- `backend/data/` is runtime-generated (DB + thumbnails) — not committed
- Tauri data dir: `~/Library/Application Support/MangaViewer/data/` on macOS
- Auto-scan timer restarts on settings change
- Search uses custom DSL server-side: `keyword`, `tag:name`, `-exclusion`
- Legacy migration logic handles v1→v2 schema upgrades; Tauri includes same logic in Rust
- `pnpm install` requires approval for native builds — if you see `ERR_PNPM_IGNORED_BUILDS`, add the package to `allowBuilds` in `pnpm-workspace.yaml`
- Tauri uses system WebView — test across platforms for compatibility
- RAR support may require system `unrar` binary as fallback
