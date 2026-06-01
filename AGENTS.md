# AGENTS.md — MangaViewer

Tauri 2.0 desktop app: Rust backend (Axum HTTP server on `127.0.0.1:5002`) + React 19 frontend (CRA). Frontend talks to the backend over REST; in dev the CRA dev server proxies to `:5002`, in prod Tauri loads the built `frontend/build/` and CORS allows `http://127.0.0.1:5002`.

## Commands

```bash
# Development
pnpm tauri dev                 # starts CRA (beforeDevCommand) + Tauri window
pnpm --filter manhuaviewer-frontend start   # CRA dev server only (backend must run separately)
pnpm tauri build               # production build (runs beforeBuildCommand, then bundles)

# Tests
cd frontend && pnpm test                           # all frontend tests (React Testing Library, CRA)
cd frontend && pnpm test -- --testPathPattern=Library   # single frontend test file
cd src-tauri && cargo test                         # all backend tests
cd src-tauri && cargo test test_name               # single backend test (use full path::name for nested)

# Lint / format (root-level scripts; no cd needed — they pass --manifest-path)
pnpm lint                      # cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
pnpm format:check              # cargo fmt --manifest-path src-tauri/Cargo.toml --check
pnpm format                    # cargo fmt (auto-fix)
```

CI (`.github/workflows/ci.yml`) runs on every push/PR to `main`: `pnpm --filter manhuaviewer-frontend build` + `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test`. Run these locally before pushing.

## Architecture

- **Backend** (`src-tauri/src/`): `main.rs` defines `AppState { db: Arc<Mutex<Database>>, data_dir }` and spawns the Axum server. Modules: `routes/` (handlers), `services/` (`archive.rs`, `scanner.rs`, `thumbnail.rs`, `cbz.rs`), `db/` (`schema.rs` = canonical schema, `migrations.rs` = additive migrations).
- **Frontend** (`frontend/src/`): React 19 + React Router v7 (CRA). Pages: `Library`, `Reader`, `History`, `Settings`. Shared hooks in `hooks/`; tests in `__tests__/`; API mocks in `__mocks__/api.js`.
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
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `ci:`, `chore:`).

## Adding a new API route

1. Add the handler in the appropriate `src-tauri/src/routes/<file>.rs` (use `error_response` for failures).
2. Register it in `src-tauri/src/routes/mod.rs` via `Router::new().route(...)` (under `/api` unless it's OPDS).
3. Add a client method in `frontend/src/utils/api.js`.

## Releasing

Versions live in three places and must be kept in sync: `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`. Use `./scripts/bump-version.sh <x.y.z>` to update all three at once. Then `git tag v<x.y.z>` and push — `.github/workflows/release.yml` builds macOS arm64 + Windows x64 installers and creates a **draft** GitHub Release (manually publish from the Releases page). Full flow: see `CONTRIBUTING.md`.

## Gotchas

- `pnpm install` may fail with `ERR_PNPM_IGNORED_BUILDS` for native packages (e.g. `better-sqlite3`, `sharp`, `core-js`, `unrs-resolver`) — add the offender to `allowBuilds` in `pnpm-workspace.yaml`. `.npmrc` uses `node-linker=hoisted` (CRA requirement).
- RAR support may require a system `unrar` binary as a fallback.
- Tauri uses the system WebView — CSS/JS quirks vary across platforms; test on each target.
- The CSP in `src-tauri/tauri.conf.json` whitelists `unsafe-inline`/`unsafe-eval` because CRA's inline runtime needs them; don't tighten without testing the dev build.
- `pnpm tauri dev` already runs `beforeDevCommand` (`pnpm --filter manhuaviewer-frontend start`) — do not start the CRA dev server manually alongside it.
- `data_dir` and the DB file are created on first run; deleting `manhuaviewer.db` resets state but loses settings/history.

## Reference

- `.github/copilot-instructions.md` — overlapping guidance (single-test commands, OPDS notes, backup/restore endpoints), kept in sync.
- `CONTRIBUTING.md` — environment setup, Linux deps, platform-specific build targets, release flow.
- `README.md` — API endpoint table, keyboard shortcuts, project tree.
