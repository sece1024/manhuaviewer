# AGENTS.md — MangaViewer

## Commands

```bash
# Start both (root): frontend :3000 proxies to backend :5002
pnpm start

# Backend
pnpm --filter manhuaviewer-backend start
cd backend && pnpm run dev     # nodemon (auto-reload)
cd backend && pnpm test        # Jest — tests in backend/tests/**/*.test.js

# Frontend
pnpm --filter manhuaviewer-frontend start
cd frontend && pnpm start      # CRA dev server
cd frontend && pnpm test       # React Testing Library — tests in frontend/src/__tests__/

# Production build (backend serves frontend/build/)
pnpm run build

# Electron (standalone macOS app)
pnpm run electron              # dev: starts Electron with backend
pnpm run build:electron        # packages into .dmg + .zip in out/
pnpm run pack                  # packages directory only (no installer)
```

## Architecture

- **Backend**: Express + better-sqlite3 (sync API — never `await` DB calls)
- **Frontend**: React 19 + React Router v7 (CRA)
- **Electron**: `electron/main.js` starts Express on a free port, loads `http://localhost:{port}` in BrowserWindow
- **Database**: SQLite at `backend/data/` (dev) or `~/Library/Application Support/MangaViewer/data/` (Electron)
- **Two archive types**: `folder` (read directory at request time) vs compressed (page list in DB, extract on demand)

## Key Conventions

- **pnpm workspace**: root `package.json` scripts use `pnpm --filter` to run backend/frontend. Do NOT use `cd backend && npm start` — it won't find dependencies.
- **Native module builds**: `pnpm-workspace.yaml` → `allowBuilds` lists packages that need postinstall scripts (better-sqlite3, electron, sharp). If you add a new native dep, add it there.
- All frontend API calls go through `frontend/src/utils/api.js` — never fetch directly
- DB access is synchronous (`better-sqlite3`). Use `.prepare().get()` / `.all()` / `.run()`
- Settings are key-value rows in `settings` table; defaults set in `backend/src/db/database.js`
- Theme is client-side only (`localStorage` → `data-theme` attribute on `<html>`)
- OPDS routes mount at `/` (not `/api`) — must not conflict with static middleware
- Frontend proxy in dev: `frontend/package.json` → `"proxy": "http://localhost:5002"`

## Adding a new API route

1. Create `backend/src/routes/newRoutes.js`
2. `require` and `router.use('/', newRoutes)` in `routes/api.js`
3. Add methods to `frontend/src/utils/api.js`

## Gotchas

- `backend/data/` is runtime-generated (DB + thumbnails) — not committed
- Electron data dir: set via `DATA_DIR` env var, defaults to `app.getPath('userData')/data/`
- Auto-scan timer (`services/scanTimer.js`) restarts on settings change
- Search uses custom DSL server-side: `keyword`, `tag:name`, `-exclusion`
- Legacy migration logic in `database.js` handles v1→v2 schema upgrades
- `pnpm install` requires approval for native builds — if you see `ERR_PNPM_IGNORED_BUILDS`, add the package to `allowBuilds` in `pnpm-workspace.yaml`
