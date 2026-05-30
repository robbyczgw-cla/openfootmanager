# Web (Browser) Version

OpenFootManager ships as a **Tauri desktop app**, but the same game can run in a
**web browser** through a thin client–server split. This document explains how
that works, how to run it, and how to keep it in sync with upstream.

> Status: **full command parity.** All 81 commands the desktop app exposes are
> mirrored by the web server, including the interactive live-match flow, season
> rollover, transfers, contracts, scouting, training and stats — see
> [Command coverage](#command-coverage).

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

**All 81 desktop commands are mirrored.** This is verified mechanically — the
command list in `src-tauri/src/lib.rs` (`invoke_handler!`) and the dispatch table
in `ofm_server` match exactly:

```bash
# desktop command names
awk '/invoke_handler/,/\]\)/' src-tauri/src/lib.rs | grep -oE '^\s+[a-z_]+,?$' \
  | tr -d ' ,' | sort -u > /tmp/desktop.txt
# server dispatch arms
grep -oE '"[a-z_]+" =>' src-tauri/crates/ofm_server/src/commands/mod.rs \
  | sed 's/"//g; s/ =>//' | sort -u > /tmp/server.txt
comm -23 /tmp/desktop.txt /tmp/server.txt   # → empty = full parity
```

Covered areas: world list/import/export, new game, team selection, saves
(create/load/delete/list/save), time advancement, **interactive live matches**
(`start/step/apply_command/snapshot/finish`, team talks, press conferences),
squad/tactics/training, transfers & scouting, contracts & renewals, staff,
finances, inbox/messages, season rollover & awards, player/team stats, settings,
jobs, and manager profiles.

Any future command added upstream that has not yet been mirrored returns HTTP
`501` with `{"error":"be.error.web.commandNotImplemented","command":"<name>"}`,
which the UI surfaces as a clear message rather than crashing.

## Testing

Two layers, both runnable locally and in CI:

**1. Server dispatch tests (no browser, runs in `cargo test`)**

```bash
cargo test --manifest-path src-tauri/Cargo.toml -p ofm_server
```

These boot an isolated `AppState` and drive the dispatch table directly — new
game → team selection → save/load round-trip through SQLite, a tactics command,
time advancement — plus the unknown-command and missing-argument error paths.
They are part of the workspace, so the existing `cargo test --workspace` CI job
already covers them.

**2. End-to-end HTTP smoke test (every command, real server)**

```bash
npm run build:web && npm run server:web   # terminal 1
npm run test:web:smoke                     # terminal 2 (OFM_WEB_URL optional)
```

`scripts/smoke-web.mjs` calls **every** command over HTTP using the exact
argument names the React frontend sends, and exits non-zero on any *contract*
failure (missing/invalid argument, unimplemented command, or 500). Legitimate
domain errors from invalid probes (e.g. bidding on a non-existent player) are
expected and ignored. This is the quickest way to confirm the web layer still
matches the frontend after a command is added or an upstream sync.

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
- **`export_world_database`** writes under `<data_dir>/exports/` (a browser
  cannot choose an arbitrary server path) and returns that path.
- **Close-to-save** maps to the browser's native "leave site?" prompt; the
  desktop "save before quit" modal flow does not translate 1:1.
