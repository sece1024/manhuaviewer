# AGENTS.md — MangaViewer

## Commands

```bash
# Start both (root): frontend :3000 proxies to backend :5002
npm run dev          # or npm start

# Backend
cd backend && npm run dev     # nodemon (auto-reload)
cd backend && npm test        # Jest — tests in backend/tests/**/*.test.js

# Frontend
cd frontend && npm start      # CRA dev server
cd frontend && npm test       # React Testing Library — tests in frontend/src/__tests__/

# Production build (backend serves frontend/build/)
npm run build
```

## Architecture

- **Backend**: Express + better-sqlite3 (sync API — never `await` DB calls)
- **Frontend**: React 19 + React Router v7 (CRA)
- **Database**: SQLite at `backend/data/manhuaviewer.db` (generated, not committed)
- **Two archive types**: `folder` (read directory at request time) vs compressed (page list in DB, extract on demand)

## Key Conventions

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
- Auto-scan timer (`services/scanTimer.js`) restarts on settings change
- Search uses custom DSL server-side: `keyword`, `tag:name`, `-exclusion`
- Legacy migration logic in `database.js` handles v1→v2 schema upgrades

## Further reading

- `.github/copilot-instructions.md` — detailed architecture and conventions
