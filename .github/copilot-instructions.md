# Copilot Instructions — MangaViewer

## Commands

```bash
# Tauri desktop app (primary)
pnpm tauri dev                 # dev with hot-reload
pnpm tauri build               # production build

# Frontend only (CRA dev server, proxied to :5002)
pnpm --filter manhuaviewer-frontend start

# Tests
cd frontend && pnpm test                           # all frontend tests (React Testing Library)
cd frontend && pnpm test -- --testPathPattern=Library  # single test file
cd src-tauri && cargo test                         # all backend tests
cd src-tauri && cargo test test_name               # single backend test

# Code quality (CI runs these on every push/PR)
pnpm format:check              # cargo fmt --check
pnpm format                    # cargo fmt (auto-fix)
pnpm lint                      # cargo clippy -D warnings
```

## Architecture

Tauri 2.0 desktop app: Rust backend spawns an Axum HTTP server on port 5002; React frontend communicates via REST. In dev mode the CRA dev server proxies to `:5002`; in production Tauri loads the built frontend via `tauri://localhost` with CORS to the Axum server.

- **Backend** (`src-tauri/src/`): `routes/` (Axum handlers), `services/` (archive extraction, scanning, thumbnails, CBZ packing), `db/` (rusqlite wrapper + schema + migrations), `logging.rs` (file logging, panic hook).
- **Frontend** (`frontend/src/`): React 19 + React Router v7 (CRA). Pages: `Library`, `Reader`, `History`, `Settings`. Shared hooks in `hooks/`, all API calls through `utils/api.js`.
- **Database**: SQLite via rusqlite at `~/Library/Application Support/MangaViewer/data/manhuaviewer.db` (macOS). Overridable via `DATA_DIR` env var. `db/mod.rs` holds the `Database` struct + all queries; `db/schema.rs` is the canonical idempotent schema; `db/migrations.rs` migrates legacy tables (`folders`, `folder_tags`, `read_history`) and adds columns.
- **Two archive types**: `folder` (directory scanned at request time, no pages stored in DB) vs compressed (`zip`/`cbz`/`rar`/`cbr`/`7z` — page list stored in DB, extracted on demand).

## Key Conventions

- **API client**: All frontend HTTP calls go through `frontend/src/utils/api.js` — never use `fetch` directly. The `api.js` module handles base URL resolution (dev proxy vs Tauri production), retries for GET requests, and URL fixing.
- **Settings**: Key-value rows in the `settings` table; unified via `useSettings` hook + `SettingsContext`. Server is single source of truth; `localStorage` is only used as an optimistic cache to prevent first-paint flicker.
- **Theme** is the only purely client-side setting (`localStorage` → `data-theme` attribute on `<html>`).
- **Route namespaces**: API routes mount at `/api`, OPDS routes at `/opds` — these must not conflict with each other or with static file serving.
- **State sharing**: `AppState` (defined in `main.rs`) wraps `Arc<Mutex<Database>>` and `data_dir`. It's shared via Axum's `with_state(Arc<AppState>)`.
- **Error responses**: Use `routes::error_response(StatusCode, &str)` helper which returns `{ "error": "..." }` JSON.
- **Blocking I/O** (archive extraction, thumbnail generation) must use `spawn_blocking` to avoid starving the tokio runtime.
- **Temp files** for RAR/7z extraction use `tempfile::tempdir()` to prevent race conditions.
- **Logging** is file-based (`logging.rs`): daily-rotating files under `<data_dir>/logs/`, 7-day retention, panic hook. Startup failures also show a native error dialog.
- **Commit messages**: Follow [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `docs:`, `ci:`, `chore:`.

## Adding a New API Route

1. Add handler in `src-tauri/src/routes/<file>.rs`
2. Register in `src-tauri/src/routes/mod.rs` via `Router::new().route()`
3. Add client method to `frontend/src/utils/api.js`

## Releasing

Version must be synchronized in three places before tagging: `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and root `package.json` (use `scripts/bump-version.sh`). Push a `v*` tag to trigger the release workflow (`.github/workflows/release.yml`), which creates a draft GitHub Release with platform installers. `frontend/package.json` has its own separate version that is intentionally NOT synced.

## Gotchas

- Package manager is pnpm (CI pins pnpm 9); `.npmrc` sets `node-linker=hoisted`. Always use `pnpm`, never `npm install` (`frontend/` still carries a legacy `package-lock.json`).
- If `pnpm install` fails with `ERR_PNPM_IGNORED_BUILDS`, whitelist the native package via `onlyBuiltDependencies` in pnpm config (the old `allowBuilds` entries were removed as invalid).
- RAR/7Z support shells out to system binaries (`unrar`, `7z`); on macOS install via Homebrew (`brew install unrar p7zip`). ZIP/CBZ are handled natively by the Rust `zip` crate.
- Tauri uses the system WebView — CSS/JS behavior varies across platforms.
