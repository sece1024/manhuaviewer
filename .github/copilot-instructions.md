# Copilot Instructions — MangaViewer

## Commands

```bash
# Tauri desktop app (primary)
pnpm tauri dev                 # dev with hot-reload
pnpm tauri build               # production build

# Frontend only (CRA dev server, proxied to :5002)
pnpm --filter manhuaviewer-frontend start
cd frontend && pnpm test       # React Testing Library — frontend/src/__tests__/

# Production build (backend serves frontend/build/)
pnpm run build

# Code quality
pnpm format:check              # cargo fmt --check
pnpm format                    # cargo fmt (auto-fix)
pnpm lint                      # cargo clippy -D warnings
```

## Architecture

Single Tauri 2.0 application with Rust backend and React frontend:

- **Backend** (`src-tauri/`): Rust + Axum + rusqlite. Modules: `db/`, `routes/`, `services/`.
- **Frontend** (`frontend/`): React 19 + React Router v7 (CRA). All API calls go through `frontend/src/utils/api.js`.
- **Database**: SQLite at `~/Library/Application Support/MangaViewer/data/`.
- **Two archive types**: `folder` (directory scanned at request time) vs compressed (page list stored in DB, extracted on demand).

## Key Conventions

- **Settings** are key-value rows in the `settings` table; unified via `useSettings` hook (server is single source of truth).
- **Theme** is the only client-side setting (`localStorage` → `data-theme` attribute on `<html>`).
- **OPDS** routes mount at `/opds` — must not conflict with API routes at `/api`.
- **Search** uses a custom DSL server-side: `keyword`, `tag:name`, `-exclusion`.
- **Blocking I/O** (archive extraction, thumbnail gen) must use `spawn_blocking` to avoid starving the tokio runtime.
- **Temp files** for RAR/7z extraction use `tempfile::tempdir()` to prevent race conditions.

## Adding a New API Route

1. Add handler in `src-tauri/src/routes/<file>.rs`
2. Register in `src-tauri/src/routes/mod.rs` via `Router::new().route()`
3. Add client method to `frontend/src/utils/api.js`
