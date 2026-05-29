# Web (Browser) Version

OpenFootManager ships as a **Tauri desktop app**, but the same game can run in a
**web browser** through a thin client–server split. This document explains how
that works, how to run it, and how to keep it in sync with upstream.

> Status: **early / foundational.** The full new-game → team-selection →
> dashboard → advance-day → save/load loop works in the browser today. Several
> management screens and the interactive live-match view are not wired up yet —
> see [Command coverage](#command-coverage) and [Limitations](#known-limitations).

## Why this design

The desktop app is a React frontend talking to a Rust core (`ofm_core`,
`engine`, `db`, `domain`) through Tauri's `invoke()`. A browser has no Tauri
runtime, so `invoke()` cannot reach native code.

Three approaches were considered (see PR discussion). We chose the one that keeps
the fork **easiest to sync with upstream**:

| Approach | Reuses game logic | Frontend edits | Upstream-merge friction |
| --- | --- | --- | --- |
| **Server-backed (chosen)** | 100% (incl. SQLite) | none (build-time alias) | low — additive |
| Full client-side WASM | logic only; persistence rewritten | none | high — persistence fork |
| Stub/mock backend | none | none | n/a (not a real game) |

The server-backed approach reuses the **entire** Rust workspace unchanged and
leaves the frontend source untouched, so pulling in upstream desktop changes
stays a near-conflict-free `git merge`.

## Architecture

```
 ┌───────────────────────┐         HTTP (fetch)          ┌──────────────────────────┐
 │  React SPA (browser)   │  POST /api/invoke/{command}   │  ofm_server (axum, Rust)  │
 │                        │ ────────────────────────────▶ │                          │
 │  invoke(cmd, args) ────┼─▶ src/web/tauriCore.ts        │  commands/* (HTTP adapter)│
 │  (unchanged app code)  │   (fetch shim, aliased in)    │        │                  │
 └───────────────────────┘ ◀──────────────────────────── │        ▼                  │
                              JSON result / {error}        │  ofm_core · db · engine  │
                                                           │  (same crates as desktop)│
                                                           └──────────────────────────┘
```

Two adapters wrap the **one** shared game core:

- **Desktop adapter** — `src-tauri/src/commands/*` (`#[tauri::command]`), unchanged.
- **Web adapter** — `src-tauri/crates/ofm_server/src/commands/*`, new and additive.

Both are thin: they pull state, call `ofm_core`/`db`, and store it back. No game
logic is duplicated.

### Frontend: zero-touch shim

The app keeps importing `@tauri-apps/api/core` and `@tauri-apps/api/window`.
`vite.web.config.ts` aliases those module specifiers to small shims:

- `src/web/tauriCore.ts` — `invoke()` → `POST /api/invoke/{command}`. Preserves
  Tauri's contract: resolves with the return value, rejects with the backend
  error key (`be.error.*`) so existing `resolveBackendError`/i18n keeps working.
- `src/web/tauriWindow.ts` — maps window close/destroy to browser equivalents.

Because the swap happens at build time, **no application source changes** for the
web build, and the desktop build is completely unaffected.

### Backend: one endpoint, a dispatch table

`ofm_server` is an axum binary. `POST /api/invoke/{command}` deserializes the
JSON args (camelCase, matching Tauri) and routes to a handler in
`commands/dispatch`. It also serves the built SPA (`dist/`) with a deep-link
fallback to `index.html`. State (`StateManager` + `SaveManager`) is process-wide
and in-memory, exactly like the desktop app; saves persist to SQLite on disk.

## Running it

### Production-style (single server serves API + built SPA)

```bash
# 1. Build the browser bundle
npm install
npm run build:web            # outputs to dist/

# 2. Run the server (serves dist/ and the API on :8080)
npm run server:web           # = cargo run -p ofm_server

# 3. Open http://localhost:8080
```

### Development (hot reload)

```bash
# Terminal 1 — backend
npm run server:web           # http://localhost:8080

# Terminal 2 — Vite dev server (proxies /api → :8080)
npm run dev:web              # http://localhost:1430
```

### Configuration (env vars for `ofm_server`)

| Variable | Default | Purpose |
| --- | --- | --- |
| `OFM_WEB_PORT` | `8080` | Listen port |
| `OFM_WEB_HOST` | `127.0.0.1` | Bind address |
| `OFM_WEB_DIST` | `dist` | Directory of the built SPA to serve |
| `OFM_WEB_DATA_DIR` | `ofm-web-data` | Where saves / settings / profiles live |
| `OFM_WEB_API` | `http://localhost:8080` | (frontend dev) proxy target for `/api` |

## Command coverage

Commands not yet mirrored return HTTP `501` with
`{"error":"be.error.web.commandNotImplemented","command":"<name>"}`, which the UI
surfaces as a clear "not available in the web build" message rather than crashing.

**Implemented (web v1):** world list/import, new game, team selection, save /
load / delete / list saves, active game, save game, exit to menu, advance day
(`advance_time`, `advance_time_with_mode`, `skip_to_match_day`), settings, clear
saves, jobs, finance snapshot + board/sponsor/marketing actions, manager
profiles (CRUD).

**Not yet mirrored:** squad/tactics/training/transfers/scouting/staff/messages
mutations, season rollover, stats overviews, the interactive live-match commands
(`start_live_match`, `step_live_match`, …), and `export_world_database` (relies
on a native save dialog).

## Adding a command (when syncing upstream)

When upstream adds or changes a `#[tauri::command]`:

1. Open the desktop version in `src-tauri/src/commands/<area>.rs`.
2. Add a sibling handler in `src-tauri/crates/ofm_server/src/commands/<area>.rs`
   that performs the same orchestration but takes `&StateManager` /
   `&AppState` instead of Tauri's `State<..>`.
3. Register it in `commands::dispatch`.

This is additive work in the new crate — it does not touch upstream-tracked files.

## Known limitations

- **Single session / in-memory state.** Like the desktop app, the server holds
  one active game in memory; it is a single-player local server, not multi-user.
- **Interactive live matches** are not implemented yet. In web v1, `live` /
  `spectator` modes auto-simulate the user's match (same as `instant`);
  `delegate` is fully supported. Interactive matches are the next milestone.
- **Blocking-action prompts** (`check_blocking_actions`) return none for now, so
  the dashboard will not raise pre-advance warnings yet.
- **Close-to-save** maps to the browser's native "leave site?" prompt; the
  desktop "save before quit" modal flow does not translate 1:1.
