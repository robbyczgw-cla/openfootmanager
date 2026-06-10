import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { EventFeed } from "./MatchPanels";
import type { MatchEvent, MatchSnapshot } from "./types";
import { getEventCommentary } from "../../lib/commentary";
import en from "../../i18n/locales/en.json";

function translate(
  key: string,
  params?: Record<string, string | number>,
): string {
  const value = key
    .split(".")
    .reduce<unknown>(
      (node, part) =>
        node && typeof node === "object"
          ? (node as Record<string, unknown>)[part]
          : undefined,
      en,
    );
  if (typeof value !== "string") return key;
  return value.replace(/\{\{(\w+)\}\}/g, (_match, name: string) =>
    String(params?.[name] ?? ""),
  );
}

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, string | number>) =>
      translate(key, params),
  }),
}));

function makePlayer(id: string, name: string) {
  return { id, name, position: "Forward", condition: 90 };
}

function makeSnapshot(events: MatchEvent[]): MatchSnapshot {
  return {
    phase: "FirstHalf",
    current_minute: 45,
    home_score: 1,
    away_score: 1,
    possession: "Home",
    ball_zone: "Midfield",
    home_team: {
      id: "home1",
      name: "Alpha FC",
      formation: "4-4-2",
      play_style: "Balanced",
      players: [makePlayer("h1", "Alice"), makePlayer("h2", "Carol")],
    },
    away_team: {
      id: "away1",
      name: "Beta FC",
      formation: "4-4-2",
      play_style: "Balanced",
      players: [makePlayer("a1", "Bob")],
    },
    home_bench: [],
    away_bench: [],
    home_possession_pct: 50,
    away_possession_pct: 50,
    events,
    home_subs_made: 0,
    away_subs_made: 0,
    max_subs: 5,
    home_set_pieces: {
      free_kick_taker: null,
      corner_taker: null,
      penalty_taker: null,
      captain: null,
    },
    away_set_pieces: {
      free_kick_taker: null,
      corner_taker: null,
      penalty_taker: null,
      captain: null,
    },
    substitutions: [],
    allows_extra_time: false,
    home_yellows: {},
    away_yellows: {},
    sent_off: [],
  } as unknown as MatchSnapshot;
}

function makeEvent(
  minute: number,
  event_type: string,
  side: "Home" | "Away",
  player_id: string | null = null,
  secondary_player_id: string | null = null,
): MatchEvent {
  return {
    minute,
    event_type,
    side,
    zone: "Midfield",
    player_id,
    secondary_player_id,
  };
}

describe("EventFeed", () => {
  it("renders a commentary line for every narrated event", () => {
    const events = [
      makeEvent(10, "Goal", "Home", "h1"),
      makeEvent(40, "Goal", "Away", "a1"),
    ];
    const snapshot = makeSnapshot(events);
    const teams = { homeName: "Alpha FC", awayName: "Beta FC" };
    const resolveName = (playerId: string | null) => {
      if (playerId === "h1") return "Alice";
      if (playerId === "a1") return "Bob";
      return playerId ?? "";
    };

    render(
      <EventFeed
        events={events}
        snapshot={snapshot}
        feedRef={{ current: null }}
      />,
    );

    for (let i = 0; i < events.length; i++) {
      const line = getEventCommentary(events, i, teams, resolveName);
      expect(line).not.toBeNull();
      expect(
        screen.getByText(translate(line!.key, line!.params)),
      ).toBeInTheDocument();
    }
  });

  it("uses the dramatic equalizer pool for a leveling goal", () => {
    const events = [
      makeEvent(10, "Goal", "Home", "h1"),
      makeEvent(40, "Goal", "Away", "a1"),
    ];
    const snapshot = makeSnapshot(events);

    render(
      <EventFeed
        events={events}
        snapshot={snapshot}
        feedRef={{ current: null }}
      />,
    );

    const line = getEventCommentary(
      events,
      1,
      { homeName: "Alpha FC", awayName: "Beta FC" },
      (id) => (id === "a1" ? "Bob" : ""),
    );
    expect(line!.key).toMatch(/^match\.commentary\.goalEqualizer\.[0-2]$/);
    expect(
      screen.getByText(translate(line!.key, line!.params)),
    ).toBeInTheDocument();
  });

  it("shows the waiting message when there are no events", () => {
    render(
      <EventFeed
        events={[]}
        snapshot={makeSnapshot([])}
        feedRef={{ current: null }}
      />,
    );

    expect(
      screen.getByText(translate("match.waitingKickoff")),
    ).toBeInTheDocument();
  });
});
