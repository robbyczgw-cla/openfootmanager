import type { JSX } from "react";

import type { FramePlayer, MatchFrame } from "./matchFrame";

// ---------------------------------------------------------------------------
// MatchPitch2D — horizontal (broadcast) 2D renderer for a single MatchFrame.
//
// Pure VIEW: it draws whatever frame it is given and has zero game logic. The
// same frames will later feed a 3D renderer. Coordinates arrive normalized
// 0..1 (see matchFrame.ts) and are mapped into this SVG viewBox. Tokens are
// positioned with `transform: translate(...)` and CSS-transitioned, so once the
// positional engine streams per-tick frames the motion is smooth for free.
// ---------------------------------------------------------------------------

interface MatchPitch2DProps {
  frame: MatchFrame;
  homeColor: string;
  awayColor: string;
  homeName: string;
  awayName: string;
}

const W = 1050;
const H = 680;
const PAD = 28; // touchline inset

const IW = W - 2 * PAD;
const IH = H - 2 * PAD;
const CY = H / 2;

// Box geometry (viewBox units)
const PB_DEPTH = 150;
const PB_H = IH * 0.6;
const PB_Y0 = PAD + (IH - PB_H) / 2;
const SY_DEPTH = 56;
const SY_H = IH * 0.28;
const SY_Y0 = PAD + (IH - SY_H) / 2;
const GOAL_DEPTH = 16;
const GOAL_H = IH * 0.16;
const GOAL_Y0 = PAD + (IH - GOAL_H) / 2;
const PEN_SPOT = 100;
const ARC_DY = 76; // half-height of the penalty "D" where it meets the box

const STRIPES = 12;
const STRIPE_W = IW / STRIPES;
const GK_COLOR = "#f59e0b";

const sx = (xn: number): number => PAD + xn * IW;
const sy = (yn: number): number => PAD + yn * IH;

const LINE = "rgba(255,255,255,0.72)";

function lastName(name: string): string {
  const parts = name.trim().split(/\s+/);
  return parts[parts.length - 1] || name;
}

function PlayerToken({
  p,
  color,
}: {
  p: FramePlayer;
  color: string;
}): JSX.Element {
  const fill = p.role === "GK" ? GK_COLOR : color;
  const label = lastName(p.name);
  const pillW = Math.max(30, label.length * 6.5 + 10);
  return (
    <g
      transform={`translate(${sx(p.x)}, ${sy(p.y)})`}
      style={{ transition: "transform 0.12s linear" }}
    >
      {/* grounding shadow */}
      <ellipse cx={0} cy={17} rx={13} ry={4} fill="rgba(0,0,0,0.28)" />
      {/* ball-carrier highlight */}
      {p.has_ball && (
        <circle r={21} fill="none" stroke="#fde047" strokeWidth={3} opacity={0.95} />
      )}
      {/* kit */}
      <circle r={15} fill={fill} stroke="rgba(255,255,255,0.92)" strokeWidth={2} />
      <circle r={15} fill="url(#ofmShine)" />
      <text
        y={5}
        textAnchor="middle"
        fill="#fff"
        fontSize={15}
        fontWeight={700}
        style={{ pointerEvents: "none" }}
      >
        {p.number}
      </text>
      {/* name pill */}
      <rect x={-pillW / 2} y={22} width={pillW} height={15} rx={7} fill="rgba(0,0,0,0.5)" />
      <text y={32.5} textAnchor="middle" fill="#fff" fontSize={11} fontWeight={600}>
        {label}
      </text>
    </g>
  );
}

export default function MatchPitch2D({
  frame,
  homeColor,
  awayColor,
  homeName,
  awayName,
}: MatchPitch2DProps): JSX.Element {
  const ball = frame.ball;
  const lBox = PAD + PB_DEPTH; // left box front edge
  const rBox = W - PAD - PB_DEPTH; // right box front edge

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      className="block h-auto w-full rounded-xl"
      style={{ maxHeight: "58vh" }}
      role="img"
      aria-label="2D match pitch"
    >
      <defs>
        <filter id="ofmTokenShadow" x="-50%" y="-50%" width="200%" height="200%">
          <feDropShadow dx="0" dy="2" stdDeviation="2" floodColor="#000" floodOpacity="0.4" />
        </filter>
        <radialGradient id="ofmShine" cx="35%" cy="28%" r="75%">
          <stop offset="0%" stopColor="#fff" stopOpacity="0.45" />
          <stop offset="55%" stopColor="#fff" stopOpacity="0" />
        </radialGradient>
        <radialGradient id="ofmBall" cx="35%" cy="30%" r="75%">
          <stop offset="0%" stopColor="#ffffff" />
          <stop offset="100%" stopColor="#d4d4d8" />
        </radialGradient>
        <radialGradient id="ofmVignette" cx="50%" cy="50%" r="72%">
          <stop offset="62%" stopColor="#000" stopOpacity="0" />
          <stop offset="100%" stopColor="#000" stopOpacity="0.3" />
        </radialGradient>
      </defs>

      {/* Mown-grass stripes */}
      {Array.from({ length: STRIPES }, (_, i) => (
        <rect
          key={`s-${i}`}
          x={PAD + i * STRIPE_W}
          y={PAD}
          width={STRIPE_W}
          height={IH}
          fill={i % 2 === 0 ? "#2e8b4b" : "#298046"}
        />
      ))}
      <rect x={PAD} y={PAD} width={IW} height={IH} fill="url(#ofmVignette)" />

      {/* Pitch markings */}
      <g stroke={LINE} strokeWidth={3} fill="none">
        <rect x={PAD} y={PAD} width={IW} height={IH} rx={2} />
        <line x1={W / 2} y1={PAD} x2={W / 2} y2={H - PAD} />
        <circle cx={W / 2} cy={CY} r={82} />

        {/* boxes */}
        <rect x={PAD} y={PB_Y0} width={PB_DEPTH} height={PB_H} />
        <rect x={PAD} y={SY_Y0} width={SY_DEPTH} height={SY_H} />
        <rect x={rBox} y={PB_Y0} width={PB_DEPTH} height={PB_H} />
        <rect x={W - PAD - SY_DEPTH} y={SY_Y0} width={SY_DEPTH} height={SY_H} />

        {/* penalty "D" arcs (bulge away from goal) */}
        <path d={`M ${lBox} ${CY - ARC_DY} A 91 91 0 0 1 ${lBox} ${CY + ARC_DY}`} />
        <path d={`M ${rBox} ${CY - ARC_DY} A 91 91 0 0 0 ${rBox} ${CY + ARC_DY}`} />

        {/* corner arcs */}
        <path d={`M ${PAD + 16} ${PAD} A 16 16 0 0 1 ${PAD} ${PAD + 16}`} />
        <path d={`M ${W - PAD - 16} ${PAD} A 16 16 0 0 0 ${W - PAD} ${PAD + 16}`} />
        <path d={`M ${PAD} ${H - PAD - 16} A 16 16 0 0 1 ${PAD + 16} ${H - PAD}`} />
        <path d={`M ${W - PAD} ${H - PAD - 16} A 16 16 0 0 0 ${W - PAD - 16} ${H - PAD}`} />
      </g>

      {/* spots */}
      <g fill={LINE}>
        <circle cx={W / 2} cy={CY} r={4} />
        <circle cx={PAD + PEN_SPOT} cy={CY} r={4} />
        <circle cx={W - PAD - PEN_SPOT} cy={CY} r={4} />
      </g>

      {/* goals with simple nets */}
      <g stroke={LINE} strokeWidth={2} fill="rgba(255,255,255,0.1)">
        <rect x={PAD - GOAL_DEPTH} y={GOAL_Y0} width={GOAL_DEPTH} height={GOAL_H} />
        <rect x={W - PAD} y={GOAL_Y0} width={GOAL_DEPTH} height={GOAL_H} />
      </g>
      <g stroke="rgba(255,255,255,0.25)" strokeWidth={1}>
        {[0.33, 0.66].map((f) => (
          <line key={`lnv-${f}`} x1={PAD - GOAL_DEPTH * (1 - f)} y1={GOAL_Y0} x2={PAD - GOAL_DEPTH * (1 - f)} y2={GOAL_Y0 + GOAL_H} />
        ))}
        {[0.33, 0.66].map((f) => (
          <line key={`rnv-${f}`} x1={W - PAD + GOAL_DEPTH * f} y1={GOAL_Y0} x2={W - PAD + GOAL_DEPTH * f} y2={GOAL_Y0 + GOAL_H} />
        ))}
      </g>

      {/* Team labels + attack direction */}
      <text x={PAD + 10} y={PAD + 24} fill="rgba(255,255,255,0.8)" fontSize={20} fontWeight={800} style={{ letterSpacing: 1 }}>
        {homeName.substring(0, 3).toUpperCase()} ▸
      </text>
      <text x={W - PAD - 10} y={PAD + 24} textAnchor="end" fill="rgba(255,255,255,0.8)" fontSize={20} fontWeight={800} style={{ letterSpacing: 1 }}>
        ◂ {awayName.substring(0, 3).toUpperCase()}
      </text>

      {/* Players */}
      <g filter="url(#ofmTokenShadow)">
        {frame.players.map((p) => (
          <PlayerToken key={p.id} p={p} color={p.side === "Home" ? homeColor : awayColor} />
        ))}
      </g>

      {/* Ball */}
      <g
        transform={`translate(${sx(ball.x)}, ${sy(ball.y)})`}
        style={{ transition: "transform 0.12s linear" }}
      >
        <ellipse cx={0} cy={11} rx={8} ry={3} fill="rgba(0,0,0,0.3)" />
        <circle r={9} fill="url(#ofmBall)" stroke="#111" strokeWidth={1.5} />
        <circle cx={-2} cy={-2} r={2.2} fill="#1f2937" />
        <circle cx={3} cy={2} r={1.6} fill="#1f2937" />
      </g>
    </svg>
  );
}
