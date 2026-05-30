//! Mirrors `src-tauri/src/commands/club.rs`.

use log::info;

use ofm_core::finances::{self, FinanceHealthLevel};
use ofm_core::game::Game;
use ofm_core::state::StateManager;

pub fn upgrade_facility(state: &StateManager, facility: String) -> Result<Game, String> {
    info!("[cmd] upgrade_facility: {}", facility);
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;

    let team_id = game
        .manager
        .team_id
        .clone()
        .ok_or("be.error.noTeamAssigned".to_string())?;

    let facility_type = match facility.as_str() {
        "Training" => domain::team::FacilityType::Training,
        "Medical" => domain::team::FacilityType::Medical,
        "Scouting" => domain::team::FacilityType::Scouting,
        _ => return Err("be.error.unknownFacilityType".to_string()),
    };

    let snapshot = finances::team_finance_snapshot(&game, &team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;
    if snapshot.currently_over_budget {
        return Err("be.error.finance.facilityUpgradeOverBudget".to_string());
    }
    if matches!(
        snapshot.overall_status,
        FinanceHealthLevel::Warning | FinanceHealthLevel::Critical
    ) {
        return Err("be.error.finance.facilityUpgradeCritical".to_string());
    }

    let team = game
        .teams
        .iter_mut()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    ofm_core::club::upgrade_facility(team, facility_type)?;

    state.set_game(game.clone());
    Ok(game)
}
