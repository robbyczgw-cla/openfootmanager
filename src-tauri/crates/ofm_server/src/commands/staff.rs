//! Mirrors `src-tauri/src/commands/staff.rs`.

use log::info;

use ofm_core::game::Game;
use ofm_core::state::StateManager;

fn load_game(state: &StateManager) -> Result<Game, String> {
    state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())
}

pub fn hire_staff(state: &StateManager, staff_id: String) -> Result<Game, String> {
    info!("[cmd] hire_staff: staff_id={}", staff_id);
    let mut game = load_game(state)?;
    let team_id = game
        .manager
        .team_id
        .clone()
        .ok_or("be.error.noTeamAssigned".to_string())?;

    let staff = game
        .staff
        .iter_mut()
        .find(|s| s.id == staff_id)
        .ok_or("be.error.staffMemberNotFound".to_string())?;

    if staff.team_id.is_some() {
        return Err("be.error.staffMemberAlreadyEmployed".to_string());
    }

    staff.team_id = Some(team_id.clone());
    let wage = staff.wage as i64;

    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.season_expenses += wage;
    }

    state.set_game(game.clone());
    Ok(game)
}

pub fn release_staff(state: &StateManager, staff_id: String) -> Result<Game, String> {
    info!("[cmd] release_staff: staff_id={}", staff_id);
    let mut game = load_game(state)?;
    let team_id = game
        .manager
        .team_id
        .clone()
        .ok_or("be.error.noTeamAssigned".to_string())?;

    let staff = game
        .staff
        .iter_mut()
        .find(|s| s.id == staff_id)
        .ok_or("be.error.staffMemberNotFound".to_string())?;

    if staff.team_id.as_deref() != Some(&team_id) {
        return Err("be.error.staffMemberNotInTeam".to_string());
    }

    let wage = staff.wage as i64;
    staff.team_id = None;

    if let Some(team) = game.teams.iter_mut().find(|team| team.id == team_id) {
        team.season_expenses = team.season_expenses.saturating_sub(wage);
    }

    state.set_game(game.clone());
    Ok(game)
}
