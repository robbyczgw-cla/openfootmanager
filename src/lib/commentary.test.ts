import { describe, expect, it } from "vitest";

import {
  COMMENTARY_POOLS,
  SUMMARY_POOLS,
  CommentaryEventInput,
  detectEventPool,
  detectGoalContext,
  generateMatchSummary,
  getEventCommentary,
  MatchSummaryInput,
} from "./commentary";
import en from "../i18n/locales/en.json";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const TEAMS = { homeName: "Alpha FC", awayName: "Beta FC" };

const NAMES: Record<string, string> = {
  p1: "Alice",
  p2: "Bob",
  p3: "Carol",
  p4: "Dave",
};

const resolveName = (playerId: string | null) =>
  playerId ? (NAMES[playerId] ?? playerId) : "";

function makeEvent(
  minute: number,
  event_type: string,
  side: "Home" | "Away",
  player_id: string | null = null,
  secondary_player_id: string | null = null,
): CommentaryEventInput {
  return { minute, event_type, side, player_id, secondary_player_id };
}

function lookup(tree: unknown, key: string): unknown {
  return key
    .split(".")
    .reduce<unknown>(
      (node, part) =>
        node && typeof node === "object"
          ? (node as Record<string, unknown>)[part]
          : undefined,
      tree,
    );
}

function makeSummaryInput(
  overrides: Partial<MatchSummaryInput> = {},
): MatchSummaryInput {
  return {
    homeName: TEAMS.homeName,
    awayName: TEAMS.awayName,
    homeScore: 0,
    awayScore: 0,
    events: [],
    resolvePlayerName: resolveName,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Template coverage
// ---------------------------------------------------------------------------

describe("commentary templates", () => {
  it("has an English template for every commentary pool variant", () => {
    for (const [pool, size] of Object.entries(COMMENTARY_POOLS)) {
      for (let variant = 0; variant < size; variant++) {
        const value = lookup(en, `match.commentary.${pool}.${variant}`);
        expect(value, `match.commentary.${pool}.${variant}`).toBeTypeOf(
          "string",
        );
      }
    }
  });

  it("has an English template for every summary pool variant", () => {
    for (const [pool, size] of Object.entries(SUMMARY_POOLS)) {
      for (let variant = 0; variant < size; variant++) {
        const value = lookup(en, `match.summary.${pool}.${variant}`);
        expect(value, `match.summary.${pool}.${variant}`).toBeTypeOf("string");
      }
    }
  });

  it("provides at least three variants for goals and two for other events", () => {
    expect(COMMENTARY_POOLS.goal).toBeGreaterThanOrEqual(3);
    for (const size of Object.values(COMMENTARY_POOLS)) {
      expect(size).toBeGreaterThanOrEqual(2);
    }
  });
});

// ---------------------------------------------------------------------------
// Variant determinism
// ---------------------------------------------------------------------------

describe("variant determinism", () => {
  it("returns the same variant for the same event every time", () => {
    const events = [makeEvent(12, "Goal", "Home", "p1", "p2")];
    const first = getEventCommentary(events, 0, TEAMS, resolveName);
    const second = getEventCommentary(events, 0, TEAMS, resolveName);
    expect(first).not.toBeNull();
    expect(second).toEqual(first);
  });

  it("always picks a variant within the pool bounds", () => {
    for (let minute = 1; minute < 45; minute++) {
      const events = [makeEvent(minute, "Foul", "Away", "p2")];
      const line = getEventCommentary(events, 0, TEAMS, resolveName);
      expect(line).not.toBeNull();
      expect(line!.key).toMatch(/^match\.commentary\.foul\.[01]$/);
    }
  });

  it("returns null for engine-internal events without narrative", () => {
    const events = [makeEvent(5, "PassCompleted", "Home", "p1")];
    expect(getEventCommentary(events, 0, TEAMS, resolveName)).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Context detection
// ---------------------------------------------------------------------------

describe("goal context detection", () => {
  it("detects the opening goal", () => {
    const events = [makeEvent(10, "Goal", "Home", "p1")];
    expect(detectGoalContext(events, 0)).toBe("goalOpener");
  });

  it("detects an equalizer", () => {
    const events = [
      makeEvent(10, "Goal", "Home", "p1"),
      makeEvent(40, "Goal", "Away", "p2"),
    ];
    expect(detectGoalContext(events, 1)).toBe("goalEqualizer");
    const line = getEventCommentary(events, 1, TEAMS, resolveName);
    expect(line!.key).toMatch(/^match\.commentary\.goalEqualizer\.[0-2]$/);
    expect(line!.params).toMatchObject({
      player: "Bob",
      team: "Beta FC",
      homeScore: 1,
      awayScore: 1,
    });
  });

  it("detects a go-ahead goal", () => {
    const events = [
      makeEvent(10, "Goal", "Home", "p1"),
      makeEvent(30, "Goal", "Away", "p2"),
      makeEvent(55, "Goal", "Away", "p3"),
    ];
    expect(detectGoalContext(events, 2)).toBe("goalGoAhead");
  });

  it("detects a late goal from the 80th minute", () => {
    const events = [
      makeEvent(10, "Goal", "Home", "p1"),
      makeEvent(83, "Goal", "Home", "p3"),
    ];
    expect(detectGoalContext(events, 1)).toBe("goalLate");
  });

  it("detects a stoppage-time winner", () => {
    const events = [
      makeEvent(20, "Goal", "Home", "p1"),
      makeEvent(60, "Goal", "Away", "p2"),
      makeEvent(90, "Goal", "Away", "p3"),
    ];
    expect(detectGoalContext(events, 2)).toBe("goalStoppageWinner");
    const line = getEventCommentary(events, 2, TEAMS, resolveName);
    expect(line!.key).toMatch(
      /^match\.commentary\.goalStoppageWinner\.[0-2]$/,
    );
  });

  it("detects braces and hat-tricks for the same scorer", () => {
    const events = [
      makeEvent(10, "Goal", "Home", "p1"),
      makeEvent(25, "Goal", "Home", "p1"),
      makeEvent(70, "Goal", "Home", "p1"),
    ];
    expect(detectGoalContext(events, 1)).toBe("goalBrace");
    expect(detectGoalContext(events, 2)).toBe("goalHatTrick");
    const line = getEventCommentary(events, 2, TEAMS, resolveName);
    expect(line!.key).toMatch(/^match\.commentary\.goalHatTrick\.[0-2]$/);
  });

  it("routes dramatic penalty goals to the goal context pools", () => {
    const equalizingPenalty = [
      makeEvent(10, "Goal", "Home", "p1"),
      makeEvent(75, "PenaltyGoal", "Away", "p2"),
    ];
    expect(detectEventPool(equalizingPenalty, 1)).toBe("goalEqualizer");

    const routinePenalty = [
      makeEvent(10, "Goal", "Home", "p1"),
      makeEvent(40, "PenaltyGoal", "Home", "p3"),
    ];
    expect(detectEventPool(routinePenalty, 1)).toBe("penaltyGoal");
  });
});

describe("red card context", () => {
  it("maps red cards to the red card pool with the men-left count", () => {
    const events = [makeEvent(58, "RedCard", "Home", "p1")];
    expect(detectEventPool(events, 0)).toBe("redCard");
    const line = getEventCommentary(events, 0, TEAMS, resolveName);
    expect(line!.key).toMatch(/^match\.commentary\.redCard\.[0-2]$/);
    expect(line!.params).toMatchObject({
      player: "Alice",
      team: "Alpha FC",
      menLeft: 10,
    });
  });

  it("counts a second sending-off down to nine men", () => {
    const events = [
      makeEvent(30, "RedCard", "Home", "p1"),
      makeEvent(80, "SecondYellow", "Home", "p3"),
    ];
    const line = getEventCommentary(events, 1, TEAMS, resolveName);
    expect(line!.key).toMatch(/^match\.commentary\.secondYellow\.[01]$/);
    expect(line!.params.menLeft).toBe(9);
  });
});

describe("substitution commentary", () => {
  it("exposes the players coming on and off", () => {
    const events = [makeEvent(64, "Substitution", "Away", "p2", "p4")];
    const line = getEventCommentary(events, 0, TEAMS, resolveName);
    expect(line!.key).toMatch(/^match\.commentary\.substitution\.[01]$/);
    expect(line!.params).toMatchObject({ playerOn: "Bob", playerOff: "Dave" });
  });
});

// ---------------------------------------------------------------------------
// Post-match summary
// ---------------------------------------------------------------------------

describe("generateMatchSummary", () => {
  it("frames a goalless draw", () => {
    const lines = generateMatchSummary(makeSummaryInput());
    expect(lines).toHaveLength(1);
    expect(lines[0].key).toMatch(/^match\.summary\.goallessDraw\.[01]$/);
  });

  it("frames a score draw", () => {
    const lines = generateMatchSummary(
      makeSummaryInput({
        homeScore: 2,
        awayScore: 2,
        events: [
          makeEvent(10, "Goal", "Home", "p1"),
          makeEvent(20, "Goal", "Away", "p2"),
          makeEvent(50, "Goal", "Home", "p3"),
          makeEvent(70, "Goal", "Away", "p4"),
        ],
      }),
    );
    expect(lines[0].key).toMatch(/^match\.summary\.scoreDraw\.[01]$/);
  });

  it("frames narrow, comfortable and rout wins by margin", () => {
    const narrow = generateMatchSummary(
      makeSummaryInput({
        homeScore: 1,
        awayScore: 0,
        events: [makeEvent(30, "Goal", "Home", "p1")],
      }),
    );
    expect(narrow[0].key).toMatch(/^match\.summary\.narrowWin\.[01]$/);
    expect(narrow[0].params).toMatchObject({
      winner: "Alpha FC",
      loser: "Beta FC",
    });

    const comfortable = generateMatchSummary(
      makeSummaryInput({ homeScore: 2, awayScore: 0 }),
    );
    expect(comfortable[0].key).toMatch(
      /^match\.summary\.comfortableWin\.[01]$/,
    );

    const rout = generateMatchSummary(
      makeSummaryInput({ homeScore: 4, awayScore: 1 }),
    );
    expect(rout[0].key).toMatch(/^match\.summary\.routWin\.[01]$/);
  });

  it("frames a comeback win when the winner trailed", () => {
    const lines = generateMatchSummary(
      makeSummaryInput({
        homeScore: 1,
        awayScore: 2,
        events: [
          makeEvent(15, "Goal", "Home", "p1"),
          makeEvent(60, "Goal", "Away", "p2"),
          makeEvent(78, "Goal", "Away", "p4"),
        ],
      }),
    );
    expect(lines[0].key).toMatch(/^match\.summary\.comebackWin\.[01]$/);
    expect(lines[0].params).toMatchObject({ winner: "Beta FC" });
  });

  it("frames an upset when the winner sits far below the loser", () => {
    const lines = generateMatchSummary(
      makeSummaryInput({
        homeScore: 0,
        awayScore: 1,
        events: [makeEvent(40, "Goal", "Away", "p2")],
        homePosition: 1,
        awayPosition: 12,
      }),
    );
    expect(lines[0].key).toMatch(/^match\.summary\.upsetWin\.[01]$/);
  });

  it("mentions a hat-trick hero and a sending-off", () => {
    const lines = generateMatchSummary(
      makeSummaryInput({
        homeScore: 3,
        awayScore: 0,
        events: [
          makeEvent(10, "Goal", "Home", "p1"),
          makeEvent(25, "Goal", "Home", "p1"),
          makeEvent(44, "RedCard", "Away", "p2"),
          makeEvent(70, "Goal", "Home", "p1"),
        ],
      }),
    );
    const keys = lines.map((line) => line.key);
    expect(keys[0]).toMatch(/^match\.summary\.routWin\.[01]$/);
    expect(keys.some((k) => k.startsWith("match.summary.topScorerHatTrick."))).toBe(
      true,
    );
    const redCardLine = lines.find((line) =>
      line.key.startsWith("match.summary.redCardMoment."),
    );
    expect(redCardLine).toBeDefined();
    expect(redCardLine!.params).toMatchObject({
      player: "Bob",
      team: "Beta FC",
      minute: 44,
    });
  });

  it("mentions a brace for a two-goal scorer", () => {
    const lines = generateMatchSummary(
      makeSummaryInput({
        homeScore: 2,
        awayScore: 0,
        events: [
          makeEvent(10, "Goal", "Home", "p1"),
          makeEvent(55, "Goal", "Home", "p1"),
        ],
      }),
    );
    expect(
      lines.some((line) => line.key.startsWith("match.summary.topScorerBrace.")),
    ).toBe(true);
  });

  it("flags late drama for a decisive last-gasp winner", () => {
    const lines = generateMatchSummary(
      makeSummaryInput({
        homeScore: 2,
        awayScore: 1,
        events: [
          makeEvent(10, "Goal", "Home", "p1"),
          makeEvent(50, "Goal", "Away", "p2"),
          makeEvent(90, "Goal", "Home", "p3"),
        ],
      }),
    );
    const lateLine = lines.find((line) =>
      line.key.startsWith("match.summary.lateDrama."),
    );
    expect(lateLine).toBeDefined();
    expect(lateLine!.params).toMatchObject({ player: "Carol", minute: 90 });
  });

  it("is deterministic for the same match data", () => {
    const input = makeSummaryInput({
      homeScore: 2,
      awayScore: 1,
      events: [
        makeEvent(10, "Goal", "Home", "p1"),
        makeEvent(50, "Goal", "Away", "p2"),
        makeEvent(85, "Goal", "Home", "p3"),
      ],
    });
    expect(generateMatchSummary(input)).toEqual(generateMatchSummary(input));
  });
});
