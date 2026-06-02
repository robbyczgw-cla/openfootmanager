// TEMPORARY dev harness — isolated render of the 2D pitch with mock data so it
// can be screenshotted/inspected without a running game. Remove when done.
import type { JSX } from "react";

import MatchPitch2D from "../components/match/MatchPitch2D";
import MatchPitch3D from "../components/match/MatchPitch3D";
import { buildFormationFrame } from "../components/match/matchFrame";
import type { MatchSnapshot } from "../components/match/types";

const NAMES = [
  "Silva", "Costa", "Mendes", "Pereira", "Santos", "Oliveira",
  "Sousa", "Rodrigues", "Fernandes", "Lopes", "Marques",
];

function mkPlayers(prefix: string): MatchSnapshot["home_team"]["players"] {
  return Array.from({ length: 11 }, (_, i) => ({
    id: `${prefix}-${i}`,
    name: NAMES[i % NAMES.length],
    position: "",
    ovr: 75,
    condition: 90,
    pace: 70, stamina: 70, strength: 70, agility: 70, passing: 70,
    shooting: 70, tackling: 70, dribbling: 70, defending: 70, positioning: 70,
    vision: 70, decisions: 70, composure: 70, aggression: 70, teamwork: 70,
    leadership: 70, handling: 70, reflexes: 70, aerial: 70,
    traits: [],
  }));
}

const mockSnapshot = {
  current_minute: 23,
  ball_zone: "Midfield",
  possession: "Home",
  home_team: { id: "h", name: "Porto", formation: "4-4-2", play_style: "Balanced", players: mkPlayers("h") },
  away_team: { id: "a", name: "Man City", formation: "4-3-3", play_style: "Balanced", players: mkPlayers("a") },
} as unknown as MatchSnapshot;

export default function PitchPreview(): JSX.Element {
  return (
    <div style={{ minHeight: "100vh", background: "#0b1220", padding: 24 }}>
      <div style={{ maxWidth: 1100, margin: "0 auto", display: "grid", gap: 16 }}>
        <MatchPitch2D
          frame={buildFormationFrame(mockSnapshot)}
          homeColor="#10b981"
          awayColor="#6366f1"
          homeName="Porto"
          awayName="Man City"
        />
        <MatchPitch3D
          frame={buildFormationFrame(mockSnapshot)}
          homeColor="#10b981"
          awayColor="#6366f1"
          homeName="Porto"
          awayName="Man City"
        />
      </div>
    </div>
  );
}
