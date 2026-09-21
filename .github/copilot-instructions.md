# Copilot Instructions — MangaViewer

## Commands

```bash
# Tauri desktop app (primary)
pnpm tauri dev                 # dev with hot-reload
pnpm tauri build               # production build

# Frontend only (Vite dev server, proxied to :5002)
pnpm --filter manhuaviewer-frontend start

# Tests
cd frontend && pnpm test                           # all frontend tests (React Testing Library)
cd frontend && pnpm test --testPathPattern Library    # single test file (space form, NOT `-- --testPathPattern=X` — pnpm mangles the `=` form)
cd src-tauri && cargo test                         # all backend tests
cd src-tauri && cargo test test_name               # single backend test

# Code quality (CI runs these on every push/PR)
pnpm format:check              # cargo fmt --check
pnpm format                    # cargo fmt (auto-fix)
pnpm lint                      # cargo clippy -D warnings
```

CI (`.github/workflows/ci.yml`) runs two jobs: **frontend** (`pnpm --filter manhuaviewer-frontend build` + `pnpm test`) and **rust** (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`). The rust job does **not** build the frontend, so backend code must compile with an empty `frontend/build`.

## Architecture

Tauri 2.0 desktop app: Rust backend spawns an Axum HTTP server on port 5002; React frontend communicates via REST. In dev mode the Vite dev server proxies to `:5002`; in production Tauri loads the built frontend via `tauri://localhost` with CORS to the Axum server.

- **Backend** (`src-tauri/src/`): `routes/` (Axum handlers: archives, tags, categories, history, settings, metadata, update, opds, plus `auth.rs` middleware), `services/` (`archive.rs` extraction, `scanner.rs`, `thumbnail.rs`, `cbz.rs`, `backup.rs`, `cleanup.rs`, `metadata.rs`), `db/` (rusqlite wrapper + schema + migrations), `logging.rs` (file logging, panic hook).
- **Frontend** (`frontend/src/`): React 19 + React Router v7, built with Vite (Jest tests still run through `react-scripts test`). Pages: `Library`, `Reader`, `History`, `Settings`. Shared hooks in `hooks/` (`useSettings`, `useTags`, `useReaderKeyboard`, `useGamepad`), all API calls through `utils/api.js`.
- **Database**: SQLite via rusqlite at `~/Library/Application Support/MangaViewer/data/manhuaviewer.db` (macOS; other platforms via `dirs::data_dir()`). Overridable via `DATA_DIR` env var (use it for isolated dev/test runs); HTTP port overridable via `PORT`. `db/mod.rs` holds the `Database` struct + all queries; `db/schema.rs` is the canonical idempotent schema; `db/migrations.rs` migrates legacy tables (`folders`, `folder_tags`, `read_history`) and adds columns.
- **Two archive types**: `folder` (directory scanned at request time, no pages stored in DB) vs compressed (`zip`/`cbz`/`rar`/`cbr`/`7z` — page list stored in DB, extracted on demand).
- **Startup side effects** (`main.rs`): after DB init, `services::cleanup::cleanup_caches` prunes `extract/`, `thumbnails/`, and `page_thumbs/` subdirectories whose archive IDs no longer exist, and enforces the byte-budget LRU for covers, page thumbnails, and extract caches (`services/cache_budget.rs`).
- **LAN auth** (`routes/auth.rs`): a `from_fn` middleware layered over `/api` + `/opds`. Empty `server_token` setting = auth disabled (default single-machine behavior). When set, "sensitive" requests (any write method, or GET under `/settings`, `/backup`, `/config`) require the token via `Authorization` header or `?token=`. Loopback requests are always allowed so the desktop app can't lock itself out. Pure decision functions (`request_is_sensitive`, `token_authorized`) sit at the top of the file and are unit-tested directly.


## Key Conventions

- **API client**: All frontend HTTP calls go through `frontend/src/utils/api.js` — never use `fetch` directly. The `api.js` module handles base URL resolution (dev proxy vs Tauri production), retries for GET requests, `fixUrl()` rewriting of relative image URLs, and the server token header.
- **Client cache**: `api.js` keeps an in-memory GET cache (30s default TTL, 60s for `/settings` and `/tags`, max 200 entries) plus in-flight dedup. Writes call `_invalidate(pattern)`, which bumps a generation counter so responses from requests started before the invalidation are not written back. When adding a mutating API method, invalidate the affected prefixes; page-image endpoints (`/pages/{n}` and `/thumb`) are deliberately excluded and rely on browser caching.
- **Settings**: Key-value rows in the `settings` table; unified via `useSettings` hook + `SettingsContext`. Server is single source of truth; `localStorage` is only used as an optimistic cache to prevent first-paint flicker.
- **Theme** is the only purely client-side setting (`localStorage` → `data-theme` attribute on `<html>`).
- **Route namespaces**: API routes mount at `/api`, OPDS routes at `/opds` — these must not conflict with each other or with static file serving.
- **State sharing**: `AppState` (defined in `main.rs`) wraps `Arc<Mutex<Database>>` and `data_dir`. It's shared via Axum's `with_state(Arc<AppState>)`.
- **Error responses**: Use `routes::error_response(StatusCode, &str)` helper which returns `{ "error": "..." }` JSON — don't return `String`/`Html` from handlers.
- **Search/filter semantics**: search is a server-side `title LIKE %kw%`; tag filters accept `namespace:name` syntax; categories are either static (join table) or dynamic (a stored `search` expression matched against title).
- **Blocking I/O** (archive extraction, thumbnail generation, cleanup) must use `spawn_blocking` to avoid starving the tokio runtime.
- **Testable pure functions**: backend logic that can be tested without a DB or filesystem (version comparison in `update.rs`, auth predicates in `auth.rs`, response parsing in `services/metadata.rs`) is factored into free functions at the top of the module with `#[cfg(test)]` tests below. Follow that split when adding similar logic.
- **Temp files** for RAR/7z extraction use `tempfile::tempdir()` to prevent race conditions.
- **Logging** is file-based (`logging.rs`): daily-rotating files under `<data_dir>/logs/`, 7-day retention, panic hook. Startup failures also show a native error dialog.
- **Code comments are written in Chinese** throughout both the Rust and JS sources — match the surrounding language when editing.
- **Commit messages**: Follow [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `docs:`, `ci:`, `chore:`.

## Testing

Frontend tests live in `frontend/src/__tests__/` (React Testing Library + `react-scripts test`). Page-level tests must wrap the component in the same providers as `App.js`: `SettingsProvider`, `TagsProvider`, `ToastProvider`, and `MemoryRouter`. API calls are mocked with `jest.mock('../utils/api')` (no factory) — Jest automocks the real module, so every method is a `jest.fn()` and each test sets return values in `beforeEach` via `api.xxx.mockResolvedValue(...)`. (`frontend/src/__mocks__/api.js` is dead code for this relative-path mock.) `frontend/package.json` carries a `moduleNameMapper` for `react-router-dom` to work around ESM bundling — don't remove it.

Backend tests are inline `#[cfg(test)]` modules. Prefer testing pure helpers; tests that need a DB should point `DATA_DIR` at a temp directory.


## Adding a New API Route

1. Add handler in `src-tauri/src/routes/<file>.rs` (use `error_response` for failures)
2. Register in `src-tauri/src/routes/mod.rs` via `Router::new().route()` (under `/api` unless it's OPDS)
3. Add client method to `frontend/src/utils/api.js` (invalidate affected cache prefixes if it mutates)
4. Note that any non-GET route is automatically treated as "sensitive" by the LAN auth middleware

## Before Finishing a Change

1. `pnpm lint` — clippy must pass with zero warnings
2. `pnpm format:check`
3. `cd src-tauri && cargo test`
4. `pnpm --filter manhuaviewer-frontend build` (and `cd frontend && pnpm test` if frontend code changed — CI won't)
5. Commit with a Conventional Commits message


## Releasing

Version must be synchronized in three places before tagging: `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and root `package.json` (use `scripts/bump-version.sh`). Push a `v*` tag to trigger the release workflow (`.github/workflows/release.yml`), which creates a draft GitHub Release with platform installers. `frontend/package.json` has its own separate version that is intentionally NOT synced.

## Gotchas

- Package manager is pnpm (CI pins pnpm 9); `.npmrc` sets `node-linker=hoisted`. Always use `pnpm`, never `npm install` (`frontend/` still carries a legacy `package-lock.json`).
- If `pnpm install` fails with `ERR_PNPM_IGNORED_BUILDS`, whitelist the native package via `onlyBuiltDependencies` in pnpm config (the old `allowBuilds` entries were removed as invalid).
- RAR/7Z support shells out to system binaries (`unrar`, `7z`); on macOS install via Homebrew (`brew install unrar p7zip`). ZIP/CBZ are handled natively by the Rust `zip` crate.
- Tauri uses the system WebView — CSS/JS behavior varies across platforms.
- `pnpm tauri dev` already runs `beforeDevCommand` (the Vite dev server) — don't start it manually alongside.
- `scripts/bump-version.sh` uses macOS `sed -i ''` syntax; on Linux it needs plain `sed -i`.
- The CSP in `src-tauri/tauri.conf.json` allows `unsafe-inline`/`unsafe-eval` for the bundler's inline runtime — don't tighten it without testing a dev build.
- Deleting `manhuaviewer.db` resets state but loses settings and history.
- `AGENTS.md` at the repo root covers the same ground for other agents; keep the two in sync when changing conventions. `CONTRIBUTING.md` has environment setup and Linux deps; `README.md` has the API endpoint table and keyboard shortcuts.

