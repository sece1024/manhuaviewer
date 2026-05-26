# Copilot Instructions — MangaViewer

## Commands

```bash
# Start both (root): frontend :3000 proxies to backend :5002
pnpm start

# Backend only (Node.js)
pnpm --filter manhuaviewer-backend start
cd backend && pnpm run dev          # nodemon auto-reload

# Frontend only (React CRA)
pnpm --filter manhuaviewer-frontend start

# Tests
cd backend && pnpm test             # Jest — backend/tests/**/*.test.js
cd frontend && pnpm test            # React Testing Library — frontend/src/__tests__/

# Run a single test file
cd backend && npx jest tests/searchParser.test.js
cd frontend && npx react-scripts test --watchAll=false Settings.test.js

# Production build (backend serves frontend/build/)
pnpm run build

# Tauri (cross-platform app)
pnpm tauri dev                      # dev with hot-reload
pnpm tauri build                    # production build
```

## Architecture

Two parallel backend implementations sharing one React frontend:

- **Tauri/Rust backend** (`src-tauri/`): Axum + rusqlite. Primary backend for desktop app. Modules: `db/`, `routes/`, `services/`, `models/`, `utils/`.
- **Node.js backend** (`backend/`): Express + better-sqlite3. Legacy web-based backend. Sync DB API — never `await` database calls. Routes in `backend/src/routes/`, aggregated in `api.js`.
- **Frontend** (`frontend/`): React 19 + React Router v7 (CRA). All API calls go through `frontend/src/utils/api.js` — never use `fetch` directly.
- **Database**: SQLite. Dev at `backend/data/`, packaged apps at `~/Library/Application Support/MangaViewer/data/`.
- **Two archive types**: `folder` (directory scanned at request time) vs compressed (page list stored in DB, extracted on demand).

## Key Conventions

- **pnpm workspace**: Always use `pnpm --filter <package>` or root scripts. Do not `cd backend && npm start` — dependencies won't resolve.
- **`pnpm install` native builds**: If you see `ERR_PNPM_IGNORED_BUILDS`, add the package to `allowBuilds` in `pnpm-workspace.yaml`.
- Settings are key-value rows in the `settings` table; defaults are set during database initialization.
- Theme is client-side only (`localStorage` → `data-theme` attribute on `<html>`).
- OPDS routes mount at `/opds` (Tauri) or `/` (Node.js) — must not conflict with static middleware.
- Search uses a custom DSL server-side: `keyword`, `tag:name`, `-exclusion`.
- Auto-scan timer restarts on settings change.

## Adding a New API Route

### Tauri (Rust)
1. Add handler in `src-tauri/src/routes/<file>.rs`
2. Register in `src-tauri/src/routes/mod.rs` via `Router::new().route()`
3. Add client method to `frontend/src/utils/api.js`

### Node.js
1. Add route handler in `backend/src/routes/`
2. `require` and mount in `backend/src/routes/api.js`
3. Add client method to `frontend/src/utils/api.js`
