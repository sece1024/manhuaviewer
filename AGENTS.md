# AGENTS.md — MangaViewer

## Commands

```bash
# Tauri desktop app (primary)
pnpm tauri dev                 # development mode with hot-reload
pnpm tauri build               # production build for current platform

# Frontend only (CRA dev server, proxied to :5002)
pnpm --filter manhuaviewer-frontend start
cd frontend && pnpm test       # React Testing Library — tests in frontend/src/__tests__/

# Production build (backend serves frontend/build/)
pnpm run build
```

## Architecture

### Tauri 2.0 (Primary)
- **Backend**: Rust + Axum (native performance)
- **Frontend**: React 19 + React Router v7 (CRA)
- **Tauri**: `src-tauri/` contains Rust backend and configuration
- **Database**: SQLite via rusqlite at `~/Library/Application Support/MangaViewer/data/`
- **Platforms**: macOS, Windows, Linux, iOS, Android
- **Two archive types**: `folder` (read directory at request time) vs compressed (page list in DB, extract on demand)

### Legacy Node.js Backend (Optional)
- Express + better-sqlite3 (sync API — never `await` DB calls)
- Only used for `pnpm start` web mode; not required for Tauri builds
- Commands: `pnpm --filter manhuaviewer-backend start`, `cd backend && pnpm test`

## Key Conventions

### General
- **pnpm workspace**: root `package.json` scripts use `pnpm --filter` to run backend/frontend. Do NOT use `cd backend && npm start` — it won't find dependencies.
- All frontend API calls go through `frontend/src/utils/api.js` — never fetch directly
- Settings are key-value rows in `settings` table; unified via `useSettings` hook (server is single source of truth)
- Theme is the only client-side setting (`localStorage` → `data-theme` attribute on `<html>`)
- OPDS routes mount at `/opds` (not `/api`) — must not conflict with static middleware
- Frontend proxy in dev: `frontend/package.json` → `"proxy": "http://localhost:5002"`

### Tauri Specific
- Rust code lives in `src-tauri/src/` with modules: `db/`, `routes/`, `services/`, `models/`, `utils/`
- API routes mirror the Node.js backend structure for frontend compatibility
- Database schema is in `src-tauri/src/db/schema.rs`
- Tauri configuration: `src-tauri/tauri.conf.json`
- Capabilities/permissions: `src-tauri/capabilities/default.json`
- Build with `pnpm tauri build` — outputs to `src-tauri/target/release/`

## Adding a new API route

### Tauri
1. Add handler function in `src-tauri/src/routes/` appropriate file
2. Register route in `src-tauri/src/routes/mod.rs` using `Router::new().route()`
3. Add methods to `frontend/src/utils/api.js`

## Gotchas

- Tauri data dir: `~/Library/Application Support/MangaViewer/data/` on macOS
- `backend/data/` is runtime-generated (DB + thumbnails) for legacy mode — not committed
- Auto-scan timer restarts on settings change
- Search uses custom DSL server-side: `keyword`, `tag:name`, `-exclusion`
- `pnpm install` requires approval for native builds — if you see `ERR_PNPM_IGNORED_BUILDS`, add the package to `allowBuilds` in `pnpm-workspace.yaml`
- Tauri uses system WebView — test across platforms for compatibility
- RAR support may require system `unrar` binary as fallback
- Blocking I/O (archive extraction, thumbnail gen) must use `spawn_blocking` to avoid starving the tokio runtime
- Temporary files for RAR/7z extraction use `tempfile::tempdir()` to prevent race conditions
