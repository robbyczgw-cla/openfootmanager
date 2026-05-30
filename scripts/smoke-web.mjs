#!/usr/bin/env node
// End-to-end smoke test for the web (browser) backend.
//
// Boots a fresh game over HTTP and calls every command the desktop app exposes,
// using the same argument names the React frontend sends. It fails only on a
// *contract* problem — a missing/invalid argument, an unimplemented command, or
// a 500 — not on legitimate domain errors (those are expected when probing with
// invalid ids, e.g. bidding on a non-existent player).
//
// Usage:
//   1. Build + start the server:  npm run build:web && npm run server:web
//   2. In another shell:          npm run test:web:smoke
//
// Configure the target with OFM_WEB_URL (default http://localhost:8080).

const BASE = (process.env.OFM_WEB_URL ?? "http://localhost:8080").replace(/\/$/, "");
const API = `${BASE}/api/invoke`;

async function call(cmd, args) {
  const res = await fetch(`${API}/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args ?? {}),
  });
  const text = await res.text();
  let body;
  try {
    body = JSON.parse(text);
  } catch {
    body = text;
  }
  const err =
    body && typeof body === "object" && "error" in body ? body.error : null;
  return { status: res.status, err, body };
}

// A contract bug = wrong/missing argument name, unimplemented command, or crash.
const isContractBug = (r) =>
  r.status === 501 ||
  r.status === 500 ||
  (typeof r.err === "string" &&
    (r.err.includes("missingArgument") ||
      r.err.includes("invalidArgument") ||
      r.err.includes("commandNotImplemented")));

async function waitForServer() {
  for (let i = 0; i < 30; i++) {
    try {
      const r = await fetch(`${BASE}/api/health`);
      if (r.ok) return true;
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
  return false;
}

async function main() {
  if (!(await waitForServer())) {
    console.error(
      `✗ Server not reachable at ${BASE}. Start it with: npm run server:web`,
    );
    process.exit(2);
  }

  const results = [];
  const run = async (cmd, args) => {
    const r = await call(cmd, args);
    results.push({ cmd, ...r });
    return r;
  };

  // --- bootstrap a real game ---
  const ng = await run("start_new_game", {
    firstName: "Smoke",
    lastName: "Test",
    dob: "1980-01-01",
    nationality: "England",
    startupOptions: { startYear: 2026, startPhase: "seasonStart" },
    worldSource: "random",
  });
  if (ng.status !== 200) {
    console.error("✗ start_new_game failed:", ng.status, ng.err);
    process.exit(1);
  }
  const teamId = ng.body.teams[0].id;
  const myPlayers = ng.body.players.filter((p) => p.team_id === teamId);
  const playerId = myPlayers[0].id;
  const xi = myPlayers.slice(0, 11).map((p) => p.id);

  await run("select_team", { teamId });
  const active = (await run("get_active_game", {})).body;
  const messageId = (active.messages[0] || {}).id || "none";
  const staffId = (active.staff[0] || {}).id || "none";
  const saves = (await run("get_saves", {})).body;
  const saveId = (saves[0] || {}).id || "none";

  // --- world / databases ---
  await run("list_world_databases", {});
  await run("write_temp_database", {
    json: JSON.stringify({ name: "x", description: "y", teams: [], players: [], staff: [] }),
  });
  await run("export_world_database", { exportPath: "smoke.json" });

  // --- profiles ---
  await run("save_manager_profile", {
    firstName: "P", lastName: "Q", dob: "1980-01-01", nationality: "England", force: true,
  });
  const profId = ((await run("get_manager_profiles", {})).body[0] || {}).id || "none";
  await run("touch_manager_profile", { id: profId });
  await run("update_manager_profile", {
    id: profId, firstName: "P2", lastName: "Q2", dob: "1980-01-01", nationality: "England",
  });

  // --- squad / tactics / training ---
  await run("set_formation", { formation: "4-3-3" });
  await run("set_starting_xi", { playerIds: xi });
  await run("set_play_style", { playStyle: "Attacking" });
  await run("set_team_match_roles", {
    matchRoles: { captain: playerId, vice_captain: null, penalty_taker: null, free_kick_taker: null, corner_taker: null },
  });
  await run("set_training", { focus: "Technical", intensity: "High" });
  await run("set_training_schedule", { schedule: "Balanced" });
  await run("set_training_groups", { groups: [] });
  await run("set_player_training_focus", { playerId, focus: "Physical" });
  await run("set_player_squad_role", { playerId, squadRole: "Senior" });
  await run("auto_select_set_pieces", { playerIds: xi });

  // --- staff / club ---
  await run("hire_staff", { staffId });
  await run("release_staff", { staffId });
  await run("upgrade_facility", { facility: "Training" });

  // --- messages / inbox ---
  await run("mark_message_read", { messageId });
  await run("mark_all_messages_read", {});
  await run("resolve_message_action", { messageId, actionId: "a", optionId: null });
  await run("delete_message", { messageId: "zzz" });
  await run("delete_messages", { messageIds: ["zzz"] });
  await run("clear_old_messages", {});

  // --- contracts / renewals ---
  await run("propose_renewal", { playerId, weeklyWage: 5000, contractYears: 3 });
  await run("delegate_renewals", { playerIds: null, maxWageIncreasePct: 10, maxContractYears: 3 });
  await run("preview_renewal_financial_impact", { playerId, weeklyWage: 5000 });
  await run("offer_free_agent_contract", { playerId: "nope", weeklyWage: 5000, contractYears: 2 });
  await run("preview_free_agent_contract_impact", { playerId: "nope", weeklyWage: 5000 });
  await run("set_contract_exit_intent", { playerId, reason: "x" });
  await run("clear_contract_exit_intent", { playerId });
  await run("preview_contract_termination", { playerId });
  await run("terminate_contract_now", { playerId });

  // --- transfers / scouting ---
  await run("toggle_transfer_list", { playerId });
  await run("toggle_loan_list", { playerId });
  await run("make_transfer_bid", { playerId: "nope", fee: 1000000 });
  await run("preview_transfer_bid_financial_impact", { playerId: "nope", fee: 1000000 });
  await run("respond_to_offer", { playerId, offerId: "o", accept: true });
  await run("counter_offer", { playerId, offerId: "o", requestedFee: 500000 });
  await run("send_scout", { scoutId: staffId, playerId });
  await run("start_youth_scouting", { scoutId: staffId, region: "Domestic", objective: "Balanced", targetPosition: null });
  await run("cancel_youth_scouting", { assignmentId: "as" });
  await run("reassign_youth_scouting", { assignmentId: "as", scoutId: staffId });

  // --- finances ---
  await run("get_finance_snapshot", { teamId: null });
  await run("request_board_support", {});
  await run("request_sponsor_pitch", {});
  await run("request_marketing_campaign", {});

  // --- stats ---
  await run("get_player_match_history", { playerId, limit: 5 });
  await run("get_player_stats_overview", { playerId });
  await run("get_team_stats_overview", { teamId });
  await run("get_team_match_history", { teamId, limit: 5 });

  // --- jobs ---
  await run("get_available_jobs", {});
  await run("apply_for_job", { teamId: "nope" });

  // --- season ---
  await run("check_season_complete", {});
  await run("get_season_awards", {});
  await run("advance_to_next_season", {});

  // --- time + live match ---
  await run("check_blocking_actions", {});
  await run("skip_to_match_day", {});
  const live = await run("advance_time_with_mode", { mode: "live" });
  if (live.body && live.body.action === "live_match") {
    await run("get_match_snapshot", {});
    await run("step_live_match", { minutes: 5 });
    await run("apply_match_command", {
      command: { Substitute: { side: "Home", player_off_id: xi[10], player_on_id: myPlayers[11].id } },
    });
    await run("apply_team_talk", { tone: "motivational", context: "drawing" });
    await run("step_live_match", { minutes: 120 });
    await run("submit_press_conference", {
      answers: [], homeTeam: "H", awayTeam: "A", homeScore: 0, awayScore: 0,
      userTeamName: "H", userTeamId: teamId, prerenderedBody: null, prerenderedHeadline: null,
    });
    await run("finish_live_match", {});
  }
  await run("start_live_match", { fixtureIndex: 0, mode: "spectator", allowsExtraTime: false });
  await run("advance_time", {});

  // --- destructive last ---
  await run("delete_save", { saveId });
  await run("delete_manager_profile", { id: profId });
  await run("exit_to_menu", {});
  await run("clear_all_saves", {});

  // --- report ---
  const bugs = results.filter(isContractBug);
  console.log(`Smoke test against ${BASE}`);
  console.log(`  commands exercised: ${results.length}`);
  console.log(
    `  domain errors (expected for invalid probes): ${
      results.filter((r) => r.status >= 400 && !isContractBug(r)).length
    }`,
  );

  if (bugs.length > 0) {
    console.error(`\n✗ ${bugs.length} contract problem(s):`);
    for (const b of bugs) {
      console.error(`    ${b.cmd}  http=${b.status} err=${JSON.stringify(b.err)}`);
    }
    process.exit(1);
  }

  console.log("\n✓ No contract problems: every command is reachable with the frontend's arguments.");
}

main().catch((e) => {
  console.error("Smoke test crashed:", e);
  process.exit(1);
});
