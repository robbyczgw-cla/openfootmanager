# OpenFootManager — Match Engine Plan & Handoff

A standalone guide so **any agent (or human) can pick up the 2D/3D match-engine
work**. It covers: how to run OpenFootManager, the vision, the current state, and
a concrete staged plan with implementation guidance for the next big step.

Companion docs: `CLAUDE.md` (repo conventions/run/verify), `docs/WEB.md`
(web-port architecture). Working branch: **`claude/2d-match-engine`**.

---

## 0. Run OpenFootManager (web version)

Prerequisites: **Node 18+** and **Rust (rustup)**.

```bash
npm install
npm run build:web            # tsc + vite → dist/
npm run server:web           # cargo run -p ofm_server → serves dist/ + API on :8080
# open http://localhost:8080  → load/start a game → play a match
```

- The Rust server (`src-tauri/crates/ofm_server`) serves the built SPA from
  `dist/` and the game API at `POST /api/invoke/{command}`. `GET /api/health` → `ok`.
- It **re-reads `index.html` per request**, so after a frontend `build:web` you do
  **NOT** need to restart the server. (History: it used to cache index.html at
  startup → after a rebuild the asset hashes changed → blank white page. Fixed in
  `ofm_server/src/main.rs`.)
- Frontend hot-reload during dev: `npm run dev:web` (Vite :1430, proxies /api → :8080).
- Env vars: `OFM_WEB_PORT`, `OFM_WEB_HOST`, `OFM_WEB_DIST`, `OFM_WEB_DATA_DIR`.

### Verify UI changes by actually looking at them
Do **not** trust "it compiles". After a frontend change, screenshot the route:

```bash
node scripts/shot.mjs http://localhost:8080/<route> /tmp/shot.png
```

It reports console/`pageerror`/failed requests, `#root` HTML and an `svgCount`,
and writes a PNG — then open the PNG. Isolated pitch render (no game needed):
**`/pitch-preview`**. (Powered by `puppeteer-core` + system Chrome.)

---

## 1. Vision — "as close to Football Manager as possible"

**Hard, non-negotiable rule: ONE engine is the single source of truth.** Every
view mode — 2D (now), 3D (later), highlights, instant result, bulk simulation of
other teams' league matches — must produce **identical, deterministic** results
from the same basis (fixed RNG seed → reproducible). The visualization is only a
camera; it **never** changes the outcome. "What you see = what happened."

- Orientation: **horizontal / broadcast**.
- The pitch is the **primary** match view; text commentary becomes a
  between-highlights ticker.
- Full 90 minutes viewable, but default focus is **highlights** (FM modes:
  Instant / Key / Extended / Full) — a frontend layer over the one engine.

The 2D and the eventual 3D renderer are **two views of the same simulation**,
exactly like FM. The bridge that makes this possible is the **frame contract**.

---

## 2. Architecture — model/view split via a "frame" contract

```
 Engine (Rust)                Frames                  Renderer(s)
 ─────────────                ──────                   ───────────
 positional sim (the truth) → MatchFrame[] (x,y,z) →  2D pitch (now)
 events + result derived                          →  3D pitch (later, same frames)
```

The frame contract decouples *what positions are* from *how they're drawn*, so
3D later is a new renderer — not a rewrite.

- **`src/components/match/matchFrame.ts`** — `MatchFrame`:
  `{ t, ball:{x,y,z}, players:[{ id, side, x, y, z?, facing?, has_ball, number,
  name, role, state? }] }`. Coordinates **normalized 0..1** (x = Home→Away goal,
  y = top→bottom touchline, z = ball height for 3D arcs). Field names are
  **snake_case** so a Rust-produced frame can drop in unchanged.
- **`src/components/match/MatchPitch2D.tsx`** — pure horizontal SVG renderer,
  **zero game logic**. Tokens are positioned with `transform: translate(...)` and
  CSS-transitioned, so streamed per-tick frames animate smoothly for free.
- **`src/components/match/MatchLive.tsx`** — match screen; renders the pitch as
  the primary view, plays the step loop (`step_live_match` → snapshot) on a speed
  timer (paused/slow/normal/fast/instant).

---

## 3. Current state (Stage 0 — DONE)

The **current engine is a placeholder**: `src-tauri/crates/engine/src/live_match/`
is minute-stepped with a coarse 5-zone field (`Zone`: HomeBox, HomeDefense,
Midfield, AwayDefense, AwayBox) + `possession`, and **no player coordinates**
(`MatchSnapshot.ball_zone` is all the spatial info — see
`engine/src/live_match/mod.rs`, `engine/src/types.rs`).

Stage 0 **honestly** visualizes that placeholder: players stand in their real
formation (`buildFormationFrame()` maps the formation string → slots), the ball
sits at the x of the **real** `ball_zone`, and the ball-carrier ring shows only
when a possessing player is genuinely near the ball. Nothing is faked — players
don't move yet because the engine has no positions to show. That is Stage 1.

---

## 4. The staged plan

### Stage 1 — Positional engine in Rust (THE next big step)
Replace the zone resolution with a continuous positional simulation whose
**player positions drive the events/results**. Atomic switch: movement + result
derivation land together, the zone engine is retired, and from then on the one
engine feeds every mode.

Concrete guidance:

1. **Field & state.** Continuous pitch (normalized 0..1, or meters 105×68 mapped
   to 0..1 for frames). Each player: `pos(x,y)`, `vel`, `target`, `role`, and the
   existing `PlayerData` attributes (pace, passing, shooting, tackling, vision,
   positioning, stamina, …). Ball: `pos(x,y,z)`, `vel`, `possessor: Option<id>`.
2. **Tick loop.** Simulate at a fixed small timestep (e.g. 0.1 s). One match
   minute = N ticks. Record a `MatchFrame` every ~0.12 s of match time (≈8/s) —
   enough for smooth motion without huge payloads.
3. **Movement.** Each player steers toward a target = role/formation home
   position shifted toward the ball and the current phase (attack vs defend) and
   tactics (play_style/formation), with max speed scaled by pace & condition;
   add simple separation so they don't overlap.
4. **Ball logic.** The possessor dribbles toward goal/space; decides
   pass/dribble/shoot from attributes + pressure + available space. A pass travels
   point-to-point with a `z` arc; opponents in the lane may intercept; nearby
   defenders tackle based on proximity + tackling. Shots resolve vs. the keeper
   (positioning/reflexes) → goal / save / miss / corner.
5. **Outcomes → existing events.** Emit the SAME `MatchEvent` types
   (`engine/src/event.rs`) so `MatchReport`/stats/commentary keep working. Keep
   the `MatchReport` pipeline (`engine/src/report.rs`).
6. **Determinism & consistency (critical).** Use the injected `Rng`. The result
   must be identical whether or not frames are recorded — recording must be
   *passive* (never consume RNG). Bulk league sim & instant result run the SAME
   tick logic with frame recording OFF → identical numbers, much faster.
7. **Transport.** Add `frames: Vec<MatchFrame>` to the per-minute result
   (`MinuteResult` in `engine/src/live_match/mod.rs`, mirrored in
   `ofm_server` and `src/components/match/types.ts`, snake_case). `MatchLive`
   plays the frame stream from each `step_live_match` instead of the single
   static `buildFormationFrame(snapshot)`.

Verify: Rust test asserting same seed → identical score/events with frames on vs
off; and screenshot the moving players via `/pitch-preview` (feed it a recorded
frame sequence) or a live match.

### Stage 2 — Deeper realism
Off-ball runs, marking, pressing lines, keeper positioning, set-piece shapes,
visible tactical influence (formation/play_style change the shape on screen).

### Stage 3 — 3D renderer
A Three.js renderer consuming the **same** `MatchFrame[]`. Maps x,y,z into a 3D
pitch; ball arcs use `z`; players use `facing`/`state`. No engine changes.

### Highlights (cross-cutting)
View modes (Instant/Key/Extended/Full) as a frontend layer: detect key events
via `getEventDisplay(evt).important` (`src/components/match/helpers.tsx`); show
the pitch for highlight windows, fast-forward the rest with the text ticker.
Never changes the result.

---

## 5. Conventions & gotchas
- Frontend types mirror Rust serde **snake_case** — keep new contracts snake_case.
- Tailwind v4; lucide-react; **i18n via `t("…")`** — never hardcode user-facing
  strings (locales in `src/i18n/locales`).
- `npm test` (vitest+jsdom). `PostMatchScreen.test.tsx` fails with
  `localStorage.getItem is not a function` — a **pre-existing Node-25 jsdom
  test-env** issue, not app code.
- Temporary dev artifacts to remove before finalizing: `/pitch-preview` route +
  `src/pages/PitchPreview.tsx`. Keep `scripts/shot.mjs` + `puppeteer-core`.

## 6. Key files
| File | Role |
| --- | --- |
| `src/components/match/matchFrame.ts` | Frame contract + Stage-0 frame source |
| `src/components/match/MatchPitch2D.tsx` | 2D SVG renderer (pure view) |
| `src/components/match/MatchLive.tsx` | Live match screen + step loop |
| `src/components/match/types.ts` | TS mirror of engine JSON types |
| `src-tauri/crates/engine/src/live_match/` | Engine to replace in Stage 1 |
| `src-tauri/crates/engine/src/{event,report,types}.rs` | Events, report, Zone |
| `src-tauri/crates/ofm_server/src/main.rs` | Static serving + index.html (per-request) |
| `scripts/shot.mjs` | Screenshot/verify tool |
