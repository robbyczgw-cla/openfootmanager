// ---------------------------------------------------------------------------
// Match frame contract — the position "truth" that BOTH the 2D renderer (now)
// and the future 3D renderer consume. Coordinates are normalized 0..1 so they
// are independent of pixels/DOM and can be mapped into a 2D SVG or a 3D scene.
//
// Field convention (horizontal / broadcast):
//   x = 0   → Home goal line (left)        x = 1   → Away goal line (right)
//   y = 0   → top touchline                y = 1   → bottom touchline
//   z       → height above the pitch (0 = ground). Ignored by 2D, used by 3D
//             for ball arcs (crosses/shots). Present from day one on purpose.
//
// Field names are snake_case to match the existing engine JSON convention
// (see types.ts) so a Rust-produced frame can drop in unchanged later.
// ---------------------------------------------------------------------------

import type { MatchSnapshot } from "./types";

export type PlayerRole = "GK" | "DF" | "MF" | "FW";

export interface FrameBall {
  x: number;
  y: number;
  z: number; // height (3D-ready); 2D renders a shadow/size cue from this later
}

export interface FramePlayer {
  id: string;
  side: "Home" | "Away";
  x: number;
  y: number;
  z?: number;
  facing?: number; // radians, 0 = facing +x (toward Away goal). 3D-ready.
  has_ball: boolean;
  number: number; // shirt-ish slot number (GK = 1) until real numbers exist
  name: string;
  role?: PlayerRole;
  state?: "idle" | "run" | "shoot" | "tackle" | "celebrate";
}

export interface MatchFrame {
  t: number; // seconds into the match (sub-minute once the engine emits them)
  ball: FrameBall;
  players: FramePlayer[];
}

// ---------------------------------------------------------------------------
// Stage 0 placeholder source: derive a single honest frame from the CURRENT
// engine snapshot. Players stand in their real formation; the ball sits at the
// x that corresponds to the engine's REAL `ball_zone`, and the ball-carrier is
// the possessing side's player nearest the ball (all derived from real state,
// nothing invented). Stage 1 replaces this with per-tick frames from the
// positional engine.
// ---------------------------------------------------------------------------

// Real engine `Zone` (serde variant names) → x along the length of the pitch.
const BALL_ZONE_X: Record<string, number> = {
  HomeBox: 0.08,
  HomeDefense: 0.3,
  Midfield: 0.5,
  AwayDefense: 0.7,
  AwayBox: 0.92,
};

function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

interface Slot {
  depth: number;
  lateral: number;
  role: PlayerRole;
}

/**
 * Turn a formation string ("4-4-2", "4-2-3-1", ...) into 11 own-half-relative
 * slots. `depth` runs 0 (own goal) → ~0.48 (just shy of halfway), `lateral`
 * runs 0 (one touchline) → 1 (the other).
 */
function formationSlots(formation: string): Slot[] {
  const lines = formation
    .split("-")
    .map((n) => parseInt(n, 10))
    .filter((n) => Number.isFinite(n) && n > 0);

  const slots: Slot[] = [{ depth: 0.04, lateral: 0.5, role: "GK" }];

  const lineCount = lines.length;
  lines.forEach((count, lineIndex) => {
    const depth =
      lineCount === 1 ? 0.32 : lerp(0.18, 0.47, lineIndex / (lineCount - 1));
    const role: PlayerRole =
      lineIndex === 0 ? "DF" : lineIndex === lineCount - 1 ? "FW" : "MF";
    for (let j = 0; j < count; j++) {
      const lateral = count === 1 ? 0.5 : lerp(0.12, 0.88, j / (count - 1));
      slots.push({ depth, lateral, role });
    }
  });

  return slots;
}

function placeTeam(
  players: MatchSnapshot["home_team"]["players"],
  side: "Home" | "Away",
  formation: string,
): FramePlayer[] {
  const slots = formationSlots(formation);
  return players.slice(0, slots.length).map((p, i) => {
    const slot = slots[i];
    // Home attacks toward +x (own goal at x=0). Away is mirrored on both axes.
    const x = side === "Home" ? slot.depth : 1 - slot.depth;
    const y = side === "Home" ? slot.lateral : 1 - slot.lateral;
    return {
      id: p.id,
      side,
      x,
      y,
      z: 0,
      facing: side === "Home" ? 0 : Math.PI,
      has_ball: false,
      number: i + 1,
      name: p.name,
      role: slot.role,
      state: "idle" as const,
    };
  });
}

/** Build a single static frame (kickoff shape) from the current snapshot. */
export function buildFormationFrame(snapshot: MatchSnapshot): MatchFrame {
  const home = placeTeam(
    snapshot.home_team.players,
    "Home",
    snapshot.home_team.formation,
  );
  const away = placeTeam(
    snapshot.away_team.players,
    "Away",
    snapshot.away_team.formation,
  );
  const players = [...home, ...away];

  const ballX = BALL_ZONE_X[snapshot.ball_zone] ?? 0.5;
  const ball: FrameBall = { x: ballX, y: 0.5, z: 0 };

  // Ball-carrier: the possessing side's outfield player nearest the ball.
  let best: FramePlayer | null = null;
  let bestD = Infinity;
  for (const p of players) {
    if (p.side !== snapshot.possession || p.role === "GK") continue;
    const d = (p.x - ball.x) ** 2 + (p.y - ball.y) ** 2;
    if (d < bestD) {
      bestD = d;
      best = p;
    }
  }
  // Only mark a carrier when a possession-side player is plausibly on the ball.
  // With static Stage-0 formations the ball is often in a zone no teammate is
  // near, and a far-away ring would read as a bug. Stage 1 makes them coincide.
  if (best && bestD < 0.05) best.has_ball = true;

  return { t: snapshot.current_minute * 60, ball, players };
}
