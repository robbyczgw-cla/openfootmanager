//! Mirrors `src-tauri/src/commands/jobs.rs`.

use log::info;
use ofm_core::job_offers::{self, JobApplicationResult, JobOpportunity};
use ofm_core::state::StateManager;
use serde_json::{json, Value};

pub fn get_available_jobs(state: &StateManager) -> Result<Vec<JobOpportunity>, String> {
    let game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;
    Ok(job_offers::get_available_jobs(&game))
}

pub fn apply_for_job(state: &StateManager, team_id: String) -> Result<Value, String> {
    info!("[cmd] apply_for_job: team_id={}", team_id);
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;

    let result = job_offers::apply_for_job(&mut game, &team_id);
    state.set_game(game.clone());

    Ok(json!({
        "result": match result {
            JobApplicationResult::Hired => "hired",
            JobApplicationResult::Rejected => "rejected",
            JobApplicationResult::InvalidTeam => "invalid_team",
            JobApplicationResult::AlreadyEmployed => "already_employed",
        },
        "game": game,
    }))
}
