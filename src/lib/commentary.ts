// ---------------------------------------------------------------------------
// Deterministic, localization-ready match commentary generator.
//
// Maps engine match events (plus the surrounding match context) to i18n keys
// with interpolation params. Variants are picked deterministically from the
// event data so replays and tests are stable, and dramatic moments
// (equalizers, stoppage-time winners, hat-tricks, red cards, penalty drama)
// select dedicated variant pools.
// ---------------------------------------------------------------------------

export interface CommentaryEventInput {
  minute: number;
  event_type: string;
  side: "Home" | "Away";
  player_id: string | null;
  secondary_player_id: string | null;
}

export interface CommentaryTeams {
  homeName: string;
  awayName: string;
}

export interface CommentaryLine {
  key: string;
  params: Record<string, string | number>;
}

export type PlayerNameResolver = (playerId: string | null) => string;

export const LATE_GOAL_MINUTE = 80;
export const STOPPAGE_TIME_MINUTE = 90;
export const LATE_DRAMA_MINUTE = 88;
export const UPSET_POSITION_GAP = 6;

// Variant counts per live-commentary pool. Every pool key must exist in the
// locale files under `match.commentary.<pool>.<index>`.
export const COMMENTARY_POOLS: Record<string, number> = {
  kickOff: 2,
  halfTime: 2,
  secondHalfStart: 2,
  fullTime: 2,
  goal: 4,
  goalOpener: 3,
  goalEqualizer: 3,
  goalGoAhead: 3,
  goalLate: 3,
  goalStoppageWinner: 3,
  goalBrace: 2,
  goalHatTrick: 3,
  penaltyGoal: 3,
  penaltyMiss: 3,
  penaltyAwarded: 2,
  yellowCard: 2,
  redCard: 3,
  secondYellow: 2,
  substitution: 2,
  injury: 2,
  shotSaved: 2,
  shotOffTarget: 2,
  shotBlocked: 2,
  corner: 2,
  freeKick: 2,
  foul: 2,
};

// Variant counts per post-match summary pool, stored under
// `match.summary.<pool>.<index>` in the locale files.
export const SUMMARY_POOLS: Record<string, number> = {
  goallessDraw: 2,
  scoreDraw: 2,
  narrowWin: 2,
  comfortableWin: 2,
  routWin: 2,
  comebackWin: 2,
  upsetWin: 2,
  topScorerBrace: 2,
  topScorerHatTrick: 2,
  redCardMoment: 2,
  lateDrama: 2,
};

const GOAL_EVENT_TYPES = new Set(["Goal", "PenaltyGoal"]);
const SENDING_OFF_EVENT_TYPES = new Set(["RedCard", "SecondYellow"]);

// 32-bit FNV-1a hash — deterministic variant selection across replays.
export function commentaryHash(seed: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) {
    hash ^= seed.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function pickCommentaryKey(pool: string, seed: string): string {
  const size = COMMENTARY_POOLS[pool] ?? 1;
  return `match.commentary.${pool}.${commentaryHash(seed) % size}`;
}

function pickSummaryKey(pool: string, seed: string): string {
  const size = SUMMARY_POOLS[pool] ?? 1;
  return `match.summary.${pool}.${commentaryHash(`${pool}|${seed}`) % size}`;
}

interface ScoreState {
  home: number;
  away: number;
}

function scoreUpTo(
  events: CommentaryEventInput[],
  endExclusive: number,
): ScoreState {
  let home = 0;
  let away = 0;
  const end = Math.min(endExclusive, events.length);
  for (let i = 0; i < end; i++) {
    const evt = events[i];
    if (GOAL_EVENT_TYPES.has(evt.event_type)) {
      if (evt.side === "Home") home++;
      else away++;
    }
  }
  return { home, away };
}

function goalsByPlayerBefore(
  events: CommentaryEventInput[],
  endExclusive: number,
  playerId: string,
): number {
  let goals = 0;
  const end = Math.min(endExclusive, events.length);
  for (let i = 0; i < end; i++) {
    const evt = events[i];
    if (GOAL_EVENT_TYPES.has(evt.event_type) && evt.player_id === playerId) {
      goals++;
    }
  }
  return goals;
}

function sendingsOffUpTo(
  events: CommentaryEventInput[],
  endInclusive: number,
  side: "Home" | "Away",
): number {
  let count = 0;
  const end = Math.min(endInclusive + 1, events.length);
  for (let i = 0; i < end; i++) {
    const evt = events[i];
    if (SENDING_OFF_EVENT_TYPES.has(evt.event_type) && evt.side === side) {
      count++;
    }
  }
  return count;
}

export type GoalContext =
  | "goalHatTrick"
  | "goalStoppageWinner"
  | "goalBrace"
  | "goalEqualizer"
  | "goalLate"
  | "goalOpener"
  | "goalGoAhead"
  | "goal";

// Detects the dramatic context for a goal event at `index` within the ordered
// event feed: hat-tricks, stoppage-time winners, braces, equalizers, late
// goals, openers and go-ahead goals, falling back to a generic goal pool.
export function detectGoalContext(
  events: CommentaryEventInput[],
  index: number,
): GoalContext {
  const evt = events[index];
  const before = scoreUpTo(events, index);
  const scoringBefore = evt.side === "Home" ? before.home : before.away;
  const concedingBefore = evt.side === "Home" ? before.away : before.home;
  const scoringAfter = scoringBefore + 1;

  const playerGoals = evt.player_id
    ? goalsByPlayerBefore(events, index, evt.player_id) + 1
    : 1;
  const takesLead =
    scoringAfter > concedingBefore && scoringBefore <= concedingBefore;
  const isEqualizer = scoringAfter === concedingBefore;

  if (playerGoals >= 3) return "goalHatTrick";
  if (evt.minute >= STOPPAGE_TIME_MINUTE && takesLead) {
    return "goalStoppageWinner";
  }
  if (playerGoals === 2) return "goalBrace";
  if (isEqualizer) return "goalEqualizer";
  if (evt.minute >= LATE_GOAL_MINUTE) return "goalLate";
  if (before.home === 0 && before.away === 0) return "goalOpener";
  if (takesLead) return "goalGoAhead";
  return "goal";
}

// Dramatic goal contexts that override the dedicated penalty-goal pool.
const PENALTY_OVERRIDE_CONTEXTS = new Set<GoalContext>([
  "goalHatTrick",
  "goalStoppageWinner",
  "goalEqualizer",
]);

// Returns the commentary pool id for the event at `index`, or null when the
// event type has no narrative line (engine-internal events like passes).
export function detectEventPool(
  events: CommentaryEventInput[],
  index: number,
): string | null {
  const evt = events[index];
  if (!evt) return null;
  switch (evt.event_type) {
    case "Goal":
      return detectGoalContext(events, index);
    case "PenaltyGoal": {
      const context = detectGoalContext(events, index);
      return PENALTY_OVERRIDE_CONTEXTS.has(context) ? context : "penaltyGoal";
    }
    case "PenaltyMiss":
      return "penaltyMiss";
    case "PenaltyAwarded":
      return "penaltyAwarded";
    case "YellowCard":
      return "yellowCard";
    case "RedCard":
      return "redCard";
    case "SecondYellow":
      return "secondYellow";
    case "Substitution":
      return "substitution";
    case "Injury":
      return "injury";
    case "ShotSaved":
      return "shotSaved";
    case "ShotOffTarget":
      return "shotOffTarget";
    case "ShotBlocked":
      return "shotBlocked";
    case "Corner":
      return "corner";
    case "FreeKick":
      return "freeKick";
    case "Foul":
      return "foul";
    case "KickOff":
      return "kickOff";
    case "HalfTime":
      return "halfTime";
    case "SecondHalfStart":
      return "secondHalfStart";
    case "FullTime":
      return "fullTime";
    default:
      return null;
  }
}

// Builds the commentary line (i18n key + params) for the event at `index`
// within the ordered event feed. Returns null for events with no narrative.
export function getEventCommentary(
  events: CommentaryEventInput[],
  index: number,
  teams: CommentaryTeams,
  resolvePlayerName: PlayerNameResolver,
): CommentaryLine | null {
  const evt = events[index];
  if (!evt) return null;
  const pool = detectEventPool(events, index);
  if (!pool) return null;

  const team = evt.side === "Home" ? teams.homeName : teams.awayName;
  const opponent = evt.side === "Home" ? teams.awayName : teams.homeName;
  const score = scoreUpTo(events, index);
  if (GOAL_EVENT_TYPES.has(evt.event_type)) {
    if (evt.side === "Home") score.home++;
    else score.away++;
  }

  const params: Record<string, string | number> = {
    player: resolvePlayerName(evt.player_id),
    team,
    opponent,
    minute: evt.minute,
    homeTeam: teams.homeName,
    awayTeam: teams.awayName,
    homeScore: score.home,
    awayScore: score.away,
  };

  if (evt.event_type === "Substitution") {
    params.playerOn = resolvePlayerName(evt.player_id);
    params.playerOff = resolvePlayerName(evt.secondary_player_id);
  }

  if (SENDING_OFF_EVENT_TYPES.has(evt.event_type)) {
    params.menLeft = 11 - sendingsOffUpTo(events, index, evt.side);
  }

  const seed = `${evt.minute}|${evt.event_type}|${evt.player_id ?? ""}|${index}`;
  return { key: pickCommentaryKey(pool, seed), params };
}

export interface MatchSummaryInput {
  homeName: string;
  awayName: string;
  homeScore: number;
  awayScore: number;
  events: CommentaryEventInput[];
  resolvePlayerName: PlayerNameResolver;
  homePosition?: number | null;
  awayPosition?: number | null;
}

function winnerTrailed(
  events: CommentaryEventInput[],
  winnerSide: "Home" | "Away",
): boolean {
  let home = 0;
  let away = 0;
  for (const evt of events) {
    if (!GOAL_EVENT_TYPES.has(evt.event_type)) continue;
    if (evt.side === "Home") home++;
    else away++;
    if (winnerSide === "Home" ? home < away : away < home) {
      return true;
    }
  }
  return false;
}

// Generates the post-match summary as an ordered list of commentary lines:
// result framing (comeback / upset / rout / comfortable / narrow / draw),
// the standout scorer, sending-off moments and late drama.
export function generateMatchSummary(
  input: MatchSummaryInput,
): CommentaryLine[] {
  const { homeName, awayName, homeScore, awayScore, events } = input;
  const lines: CommentaryLine[] = [];
  const seedBase = `${homeName}|${awayName}|${homeScore}|${awayScore}|${events.length}`;
  const baseParams: Record<string, string | number> = {
    homeTeam: homeName,
    awayTeam: awayName,
    homeScore,
    awayScore,
  };

  const winnerSide: "Home" | "Away" | null =
    homeScore > awayScore ? "Home" : awayScore > homeScore ? "Away" : null;
  const margin = Math.abs(homeScore - awayScore);

  if (!winnerSide) {
    const pool = homeScore === 0 ? "goallessDraw" : "scoreDraw";
    lines.push({ key: pickSummaryKey(pool, seedBase), params: { ...baseParams } });
  } else {
    const winner = winnerSide === "Home" ? homeName : awayName;
    const loser = winnerSide === "Home" ? awayName : homeName;
    const winnerPosition =
      winnerSide === "Home" ? input.homePosition : input.awayPosition;
    const loserPosition =
      winnerSide === "Home" ? input.awayPosition : input.homePosition;
    const isComeback = winnerTrailed(events, winnerSide);
    const isUpset =
      winnerPosition != null &&
      loserPosition != null &&
      winnerPosition - loserPosition >= UPSET_POSITION_GAP;

    const pool = isComeback
      ? "comebackWin"
      : isUpset
        ? "upsetWin"
        : margin >= 3
          ? "routWin"
          : margin === 2
            ? "comfortableWin"
            : "narrowWin";
    lines.push({
      key: pickSummaryKey(pool, seedBase),
      params: { ...baseParams, winner, loser },
    });
  }

  // Standout scorer (brace or hat-trick).
  const goalsByPlayer = new Map<
    string,
    { goals: number; side: "Home" | "Away" }
  >();
  for (const evt of events) {
    if (!GOAL_EVENT_TYPES.has(evt.event_type) || !evt.player_id) continue;
    const entry = goalsByPlayer.get(evt.player_id) ?? {
      goals: 0,
      side: evt.side,
    };
    entry.goals++;
    goalsByPlayer.set(evt.player_id, entry);
  }
  let topScorerId: string | null = null;
  let topScorer: { goals: number; side: "Home" | "Away" } | null = null;
  for (const [playerId, entry] of goalsByPlayer) {
    if (!topScorer || entry.goals > topScorer.goals) {
      topScorerId = playerId;
      topScorer = entry;
    }
  }
  if (topScorerId && topScorer && topScorer.goals >= 2) {
    const pool = topScorer.goals >= 3 ? "topScorerHatTrick" : "topScorerBrace";
    lines.push({
      key: pickSummaryKey(pool, seedBase),
      params: {
        player: input.resolvePlayerName(topScorerId),
        team: topScorer.side === "Home" ? homeName : awayName,
        goals: topScorer.goals,
      },
    });
  }

  // First sending-off as a key moment.
  const sendingOff = events.find((evt) =>
    SENDING_OFF_EVENT_TYPES.has(evt.event_type),
  );
  if (sendingOff) {
    lines.push({
      key: pickSummaryKey("redCardMoment", seedBase),
      params: {
        player: input.resolvePlayerName(sendingOff.player_id),
        team: sendingOff.side === "Home" ? homeName : awayName,
        minute: sendingOff.minute,
      },
    });
  }

  // Late drama: a decisive goal in the final minutes.
  const goalEvents = events.filter((evt) =>
    GOAL_EVENT_TYPES.has(evt.event_type),
  );
  const lastGoal = goalEvents[goalEvents.length - 1];
  if (
    lastGoal &&
    lastGoal.minute >= LATE_DRAMA_MINUTE &&
    (winnerSide === null || (margin === 1 && lastGoal.side === winnerSide))
  ) {
    lines.push({
      key: pickSummaryKey("lateDrama", seedBase),
      params: {
        player: input.resolvePlayerName(lastGoal.player_id),
        team: lastGoal.side === "Home" ? homeName : awayName,
        minute: lastGoal.minute,
      },
    });
  }

  return lines;
}
