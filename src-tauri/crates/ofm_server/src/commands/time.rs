//! Time-advancement commands.
//!
//! Mirrors `src-tauri/src/commands/time.rs` + `application/time_advancement.rs`.
//!
//! Web v1 limitation: the interactive live-match screen is not wired up yet, so
//! `"live"` / `"spectator"` modes fall through to the normal auto-simulation
//! path (the user's fixture is simulated automatically, same as `"instant"`).
//! `"delegate"` keeps its dedicated path. Interactive live matches are the next
//! milestone — see `docs/WEB.md`.

use log::info;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use ofm_core::game::Game;
use ofm_core::state::StateManager;

use super::round_summary::{build_round_summary_dto, RoundSummaryDto};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvanceTimeWithModeResponse {
    pub action: String,
    pub game: Option<Game>,
    pub snapshot: Option<engine::MatchSnapshot>,
    pub fixture_index: Option<usize>,
    pub mode: Option<String>,
    pub round_summary: Option<RoundSummaryDto>,
}

fn round_context_for_today(
    game: &Game,
    today: &str,
) -> Option<(u32, Vec<domain::league::StandingEntry>)> {
    let league = game.league.as_ref()?;
    let matchday = league
        .fixtures
        .iter()
        .find(|fixture| fixture.date == today)
        .map(|fixture| fixture.matchday)?;

    Some((matchday, league.standings.clone()))
}

fn scheduled_user_fixture_index(game: &Game, today: &str) -> Option<usize> {
    let user_team_id = game.manager.team_id.as_ref()?;
    let league = game.league.as_ref()?;

    league
        .fixtures
        .iter()
        .enumerate()
        .find_map(|(index, fixture)| {
            if fixture.date == today
                && fixture.status == domain::league::FixtureStatus::Scheduled
                && (fixture.home_team_id == *user_team_id || fixture.away_team_id == *user_team_id)
            {
                Some(index)
            } else {
                None
            }
        })
}

pub fn advance_time(state: &StateManager) -> Result<Game, String> {
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;

    info!(
        "[cmd] advance_time: date={}",
        game.clock.current_date.format("%Y-%m-%d")
    );

    let mut captures = Vec::new();
    ofm_core::turn::process_day_with_capture(&mut game, &mut |capture| {
        captures.push(capture);
    });
    for capture in captures {
        state.append_stats_state(capture);
    }

    state.set_game(game.clone());
    Ok(game)
}

pub fn advance_time_with_mode(
    state: &StateManager,
    mode: String,
) -> Result<AdvanceTimeWithModeResponse, String> {
    info!("[cmd] advance_time_with_mode: mode={}", mode);
    let mut game = state
        .get_game(|current_game| current_game.clone())
        .ok_or("be.error.noActiveGameSession")?;

    let today = game.clock.current_date.format("%Y-%m-%d").to_string();
    let round_context = round_context_for_today(&game, &today);
    let user_fixture_idx = scheduled_user_fixture_index(&game, &today);

    match (mode.as_str(), user_fixture_idx) {
        ("delegate", Some(index)) => {
            let mut session =
                ofm_core::live_match_manager::create_live_match(&game, index, ofm_core::live_match_manager::MatchMode::Instant, false)?;
            session.user_side = None;
            session.run_to_completion();

            let home_team_id = session.home_team_id.clone();
            let away_team_id = session.away_team_id.clone();
            let report = session.match_state.into_report();

            let mut captures = Vec::new();
            ofm_core::turn::simulate_other_matches_with_capture(
                &mut game,
                &today,
                Some(index),
                &mut |capture| captures.push(capture),
            );
            ofm_core::turn::apply_match_report_with_capture(
                &mut game,
                index,
                &home_team_id,
                &away_team_id,
                &report,
                &mut |capture| captures.push(capture),
            );
            for capture in captures {
                state.append_stats_state(capture);
            }

            let round_summary = round_context
                .as_ref()
                .and_then(|(matchday, previous)| build_round_summary_dto(&game, *matchday, previous));

            ofm_core::turn::finish_live_match_day(&mut game);
            state.set_game(game.clone());

            Ok(AdvanceTimeWithModeResponse {
                action: "advanced".to_string(),
                game: Some(game),
                snapshot: None,
                fixture_index: None,
                mode: None,
                round_summary,
            })
        }
        // "live" / "spectator" / "instant" / no user fixture: auto-advance the
        // day (the user's match, if any, is simulated automatically in web v1).
        _ => {
            let mut captures = Vec::new();
            ofm_core::turn::process_day_with_capture(&mut game, &mut |capture| {
                captures.push(capture);
            });
            for capture in captures {
                state.append_stats_state(capture);
            }
            let round_summary = round_context
                .as_ref()
                .and_then(|(matchday, previous)| build_round_summary_dto(&game, *matchday, previous));
            state.set_game(game.clone());

            Ok(AdvanceTimeWithModeResponse {
                action: "advanced".to_string(),
                game: Some(game),
                snapshot: None,
                fixture_index: None,
                mode: None,
                round_summary,
            })
        }
    }
}

/// Web v1: blocking-action analysis (`application/time_blockers.rs`) is not yet
/// mirrored, so no blockers are reported. The daily loop is unaffected.
pub fn check_blocking_actions(state: &StateManager) -> Result<Value, String> {
    let _ = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession")?;
    Ok(json!([]))
}

pub fn skip_to_match_day(state: &StateManager) -> Result<Value, String> {
    info!("[cmd] skip_to_match_day");
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession")?;

    let user_team_id = game.manager.team_id.clone().ok_or("be.error.noTeamAssigned")?;

    let mut days_skipped = 0u32;
    loop {
        if days_skipped >= 60 {
            break;
        }

        let today = game.clock.current_date.format("%Y-%m-%d").to_string();
        let has_match = game.league.as_ref().is_some_and(|league| {
            league.fixtures.iter().any(|fixture| {
                fixture.date == today
                    && fixture.status == domain::league::FixtureStatus::Scheduled
                    && (fixture.home_team_id == user_team_id || fixture.away_team_id == user_team_id)
            })
        });

        if has_match {
            break;
        }

        let mut captures = Vec::new();
        ofm_core::turn::process_day_with_capture(&mut game, &mut |capture| {
            captures.push(capture);
        });
        for capture in captures {
            state.append_stats_state(capture);
        }
        days_skipped += 1;

        if game.manager.team_id.is_none() {
            state.set_game(game.clone());
            return Ok(json!({
                "action": "fired",
                "game": game,
                "days_skipped": days_skipped
            }));
        }
    }

    state.set_game(game.clone());
    Ok(json!({
        "action": "arrived",
        "game": game,
        "days_skipped": days_skipped
    }))
}
