# AGENTS.md — MangaViewer

Tauri 2.0 desktop app: Rust backend (Axum HTTP server, default `127.0.0.1:5002`) + React 19 frontend (Vite). Frontend talks to the backend over REST; in dev the Vite dev server proxies `/api` + `/opds` to `:5002`, in prod Tauri loads the built `frontend/build/` and CORS allows only the Tauri origins (`tauri://localhost`, `http://tauri.localhost`, plus dev origins in debug builds — see `routes/mod.rs::create_router`). LAN mode re-embeds `frontend/build` via `rust-embed` and serves the SPA from the Axum server itself.

## Commands

```bash
# Development
pnpm tauri dev                 # starts Vite dev server (beforeDevCommand) + Tauri window
pnpm --filter manhuaviewer-frontend start   # Vite dev server only (backend must run separately)
pnpm tauri build               # production build (runs beforeBuildCommand, then bundles)

# Tests
cd frontend && pnpm test                           # all frontend tests (React Testing Library, via react-scripts test)
cd frontend && pnpm test --testPathPattern Library     # single frontend test file (space form, NOT `-- --testPathPattern=X` — pnpm mangles the `=` form)
cd src-tauri && cargo test                         # all backend tests
cd src-tauri && cargo test test_name               # single backend test (use full path::name for nested)

# Lint / format (root-level scripts; no cd needed — they pass --manifest-path)
pnpm lint                      # cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
pnpm format:check              # cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm format                    # cargo fmt (auto-fix)
```

CI (`.github/workflows/ci.yml`) runs on every push/PR to `main`. It runs `pnpm --filter manhuaviewer-frontend build` (compile + ESLint) **and frontend tests** (`cd frontend && pnpm test`). Rust CI runs `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test`. Run these locally before pushing.

## Architecture

- **Backend** (`src-tauri/src/`): `main.rs` defines `AppState { db: Arc<Database>, data_dir, last_thumb_eviction }` (an r2d2 pool — **no** `Mutex`) and spawns the Axum server. Modules: `routes/` (`archives`, `tags`, `categories`, `history`, `settings`, `scan`, `convert`, `sync`, `metadata`, `update`, `opds` + `auth.rs` LAN-auth middleware), `services/` (`archive.rs`, `scanner.rs`, `thumbnail.rs`, `cbz.rs`, `backup.rs`, `cleanup.rs`, `cache_budget.rs`, `page_cache.rs`, `metadata.rs`, `fs_ext.rs`), `db/` (`mod.rs` = `Database` struct + r2d2 pool + shared helpers; SQL queries are split into per-domain submodules `archives.rs`/`tags.rs`/`categories.rs`/`history.rs`/`settings.rs`/`bookmarks.rs`/`backup.rs`; `schema.rs` = canonical idempotent schema; `migrations.rs` = legacy-table migration + column additions), `logging.rs` (daily-rotating file logs at `<data_dir>/logs/`, panic hook).
- **Frontend** (`frontend/src/`): React 19 + React Router v7 (Vite). Pages: `Library`, `Reader`, `History`, `Settings`. Shared hooks in `hooks/` (`useSettings`, `useTags`, `useReaderKeyboard`, `useGamepad`, `useScan`, `useSync`, `useCbzConvert`, `useLibrarySession`, …); tests in `__tests__/`.
- **Database**: SQLite via rusqlite, file at `<data_dir>/manhuaviewer.db`. Default `data_dir` is `~/Library/Application Support/MangaViewer/data` on macOS (other platforms via `dirs::data_dir()`). Overridable via the `DATA_DIR` env var (use this for isolated test/dev runs). HTTP port overridable via `PORT` (default `5002`).
- **Platforms**: macOS, Windows, Linux (Tauri 2.0; Linux build needs `libwebkit2gtk-4.1-dev` etc. — see `CONTRIBUTING.md`).
- **Two archive types**: `folder` (directory read at request time, no pages in DB) vs compressed (`zip`/`cbz`/`rar`/`cbr`/`7z` — page list cached in DB, files extracted on demand via `tempfile::tempdir()`).

## Key Conventions

- All frontend HTTP calls go through `frontend/src/utils/api.js` — never `fetch` directly. This module resolves base URL (dev proxy vs Tauri prod `http://127.0.0.1:5002`), retries GETs up to 3×, and rewrites relative image URLs via `fixUrl()`.
- API routes mount at `/api`, OPDS routes at `/opds` (see `src-tauri/src/routes/mod.rs`). These namespaces must not collide with each other or with static file serving.
- Settings are key-value rows in the `settings` table; unified via the `useSettings` hook + `SettingsContext`. Server is the source of truth; `localStorage` is only an optimistic cache to prevent first-paint flicker.
- Theme is the only purely client-side setting (`localStorage` → `data-theme` attribute on `<html>`).
- Backend errors: use `routes::error_response(StatusCode, &str)` (in `routes/mod.rs`) which returns `{"error": "..."}` JSON — don't return `String`/`Html` directly from handlers.
- Blocking I/O (archive extraction, thumbnail generation) **must** use `tokio::task::spawn_blocking` to avoid starving the tokio runtime.
- Search filtering is a server-side `LIKE` match against **title OR tag name** (space-separated terms are AND-ed, a `-term` prefix excludes); tag filters accept `namespace:name` syntax; categories are either static (join table) or dynamic (a `search` expression matched against title).
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `ci:`, `chore:`).

## Testing

Frontend tests live in `frontend/src/__tests__/` and use React Testing Library via react-scripts (CRA's Jest runner). Every page test must wrap the component in the same providers used by `App.js`: `SettingsProvider`, `TagsProvider`, `ToastProvider`, and `MemoryRouter`. Tests mock the API with `jest.mock('../utils/api')` (no factory), so Jest **automocks** the real module — every method becomes a `jest.fn()` and each test sets return values in `beforeEach` via `api.xxx.mockResolvedValue(...)`. (There is intentionally **no** `__mocks__/api.js` — the old one was dead code and has been deleted.) Frontend `package.json` also has a `moduleNameMapper` for `react-router-dom` to work around CRA's bundling.

## Adding a new API route

1. Add the handler in the appropriate `src-tauri/src/routes/<file>.rs` (use `error_response` for failures).
2. Register it in `src-tauri/src/routes/mod.rs` via `Router::new().route(...)` (under `/api` unless it's OPDS).
3. Add a client method in `frontend/src/utils/api.js`.

## Implementation workflow

Every completed change MUST be committed with `git commit` — never leave work uncommitted at the end of a session. Follow this sequence before finishing:

1. `pnpm lint` — Rust clippy must pass with zero warnings.
2. `pnpm format:check` — Rust formatting must be clean (`pnpm format` to fix).
3. `pnpm --filter manhuaviewer-frontend build` — frontend must compile.
4. `cd frontend && pnpm test` — frontend tests must pass (CI runs them).
5. `cd src-tauri && cargo test` — all backend tests must pass.
6. `git add` the changed files and `git commit` with a Conventional Commits message (`feat:`, `fix:`, etc.).

## Releasing

Versions live in three places and must be kept in sync: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`. Use `./scripts/bump-version.sh <x.y.z>` to update all three at once. Then `git tag v<x.y.z>` and push — `.github/workflows/release.yml` builds macOS arm64 + Windows x64 installers and creates a **draft** GitHub Release (manually publish from the Releases page). Full flow: see `CONTRIBUTING.md`.

`frontend/package.json` has its own separate version (`2.0.0`, `private`) that is intentionally **not** synced and not touched by `bump-version.sh` — don't "fix" the mismatch.

## Gotchas

- Package manager is pnpm (CI pins pnpm 9 via `pnpm/action-setup`). `.npmrc` sets `node-linker=hoisted` (CRA requirement) + `package-manager-strict=false`. Always use `pnpm`, never `npm install` — `frontend/` still carries a legacy `package-lock.json` that is not used.
- If `pnpm install` fails with `ERR_PNPM_IGNORED_BUILDS` for a native package, whitelist it in `pnpm-workspace.yaml`, which this repo configures via the `allowBuilds:` map (e.g. `core-js: true`). Note the history: an earlier placeholder-valued `allowBuilds` block was invalid and removed (`a73c160`), then re-added as a proper map (`ee4284f`) — so the current file is correct; don't "fix" it back to `onlyBuiltDependencies` without checking pnpm's docs for your installed version.
- RAR/7Z archives shell out to system binaries (`unrar`, `7z`); ZIP/CBZ are handled natively by the Rust `zip` crate. On macOS install `unrar` + `p7zip` via Homebrew or those archive types fail with a clear error.
- Tauri uses the system WebView — CSS/JS quirks vary across platforms; test on each target.
- The CSP in `src-tauri/tauri.conf.json` allows `unsafe-inline` for the bundled runtime and adds `object-src 'none'`/`base-uri 'none'`/`frame-ancestors 'none'`; only tighten further after testing the dev build.
- `pnpm tauri dev` already runs `beforeDevCommand` (`pnpm --filter manhuaviewer-frontend start`) — do not start the Vite dev server manually alongside it.
- `data_dir` and the DB file are created on first run; deleting `manhuaviewer.db` resets state but loses settings/history.
- App logs to `<data_dir>/logs/manhuaviewer.log.<YYYY-MM-DD>` (daily rotation, 7-day retention, panic hook). Startup failures (DB init, port bind) also show a native error dialog — check the log if the app silently fails to open (esp. Windows, where the console is hidden).
- `scripts/bump-version.sh` uses `sed -i ''` (macOS syntax). On Linux it needs `sed -i` without the empty-string argument.

## Reference

- `.github/copilot-instructions.md` — overlapping guidance (single-test commands, OPDS notes, backup/restore endpoints), kept in sync.
- `CONTRIBUTING.md` — environment setup, Linux deps, platform-specific build targets, release flow.
- `README.md` — API endpoint table, keyboard shortcuts, project tree.
