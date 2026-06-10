//! Mirrors `src-tauri/src/commands/season.rs`.

use log::info;
use serde_json::{json, Value};

use ofm_core::state::StateManager;

fn load_game(state: &StateManager) -> Result<ofm_core::game::Game, String> {
    state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())
}

pub fn check_season_complete(state: &StateManager) -> Result<bool, String> {
    let game = load_game(state)?;
    Ok(ofm_core::end_of_season::is_season_complete(&game))
}

pub fn advance_to_next_season(state: &StateManager) -> Result<Value, String> {
    info!("[cmd] advance_to_next_season");
    let mut game = load_game(state)?;

    if !ofm_core::end_of_season::is_season_complete(&game) {
        return Err("be.error.seasonNotComplete".to_string());
    }

    let summary = ofm_core::end_of_season::process_end_of_season(&mut game);
    ofm_core::firing::check_manager_firing(&mut game);
    state.set_game(game.clone());

    if game.manager.team_id.is_none() {
        return Ok(json!({
            "action": "fired",
            "game": game,
            "summary": summary,
        }));
    }

    Ok(json!({
        "game": game,
        "summary": summary,
    }))
}

pub fn get_season_awards(
    state: &StateManager,
) -> Result<ofm_core::season_awards::SeasonAwards, String> {
    let game = load_game(state)?;
    Ok(ofm_core::season_awards::compute_season_awards(&game))
}
