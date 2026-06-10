//! Time-advancement commands. Mirrors `src-tauri/src/commands/time.rs`.
//!
//! `advance_time_with_mode` and `check_blocking_actions` delegate to the copied
//! `time_advancement` / `time_blockers` service modules (mirrored from
//! `src-tauri/src/application/*`), so live-match setup and blocking-action
//! analysis behave exactly like the desktop build.

use log::info;
use serde_json::{json, Value};

use ofm_core::game::Game;
use ofm_core::state::StateManager;

pub use super::time_advancement::AdvanceTimeWithModeResponse;

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
    super::time_advancement::advance_time_with_mode(state, &mode)
}

pub fn check_blocking_actions(state: &StateManager) -> Result<Value, String> {
    let game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession")?;
    let blockers = super::time_blockers::compute_blocking_actions(&game);
    Ok(json!(blockers))
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

        let blockers = super::time_blockers::compute_blocking_actions(&game);
        if !blockers.is_empty() {
            state.set_game(game.clone());
            return Ok(json!({
                "action": "blocked",
                "game": game,
                "blockers": blockers,
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
