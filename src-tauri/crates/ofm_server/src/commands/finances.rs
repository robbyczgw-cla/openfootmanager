//! Mirrors `src-tauri/src/commands/finances.rs`.

use log::info;
use serde::Serialize;

use ofm_core::finances::{
    BoardSupportResult, FinanceActionPreviews, MarketingCampaignResult, SponsorPitchResult,
    TeamFinanceSnapshot,
};
use ofm_core::game::Game;
use ofm_core::state::StateManager;

#[derive(Debug, Clone, Serialize)]
pub struct FinanceSnapshotCommandResponse {
    pub snapshot: TeamFinanceSnapshot,
    pub previews: FinanceActionPreviews,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardSupportCommandResponse {
    pub game: Game,
    pub result: BoardSupportResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct SponsorPitchCommandResponse {
    pub game: Game,
    pub result: SponsorPitchResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketingCampaignCommandResponse {
    pub game: Game,
    pub result: MarketingCampaignResult,
}

fn resolve_team_id(game: &Game, team_id: Option<String>) -> Result<String, String> {
    match team_id {
        Some(team_id) => Ok(team_id),
        None => game
            .manager
            .team_id
            .clone()
            .ok_or("be.error.noTeamAssigned".to_string()),
    }
}

pub fn get_finance_snapshot(
    state: &StateManager,
    team_id: Option<String>,
) -> Result<FinanceSnapshotCommandResponse, String> {
    info!("[cmd] get_finance_snapshot: team_id={:?}", team_id);
    let game = state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;

    let resolved_team_id = resolve_team_id(&game, team_id)?;
    let snapshot = ofm_core::finances::team_finance_snapshot(&game, &resolved_team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;
    let previews =
        ofm_core::finances::finance_action_previews(&game, &resolved_team_id).unwrap_or_default();

    Ok(FinanceSnapshotCommandResponse { snapshot, previews })
}

pub fn request_board_support(state: &StateManager) -> Result<BoardSupportCommandResponse, String> {
    info!("[cmd] request_board_support");
    let mut game = state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;
    let team_id = resolve_team_id(&game, None)?;
    let result = ofm_core::finances::request_board_support(&mut game, &team_id)?;
    state.set_game(game.clone());
    Ok(BoardSupportCommandResponse { game, result })
}

pub fn request_sponsor_pitch(state: &StateManager) -> Result<SponsorPitchCommandResponse, String> {
    info!("[cmd] request_sponsor_pitch");
    let mut game = state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;
    let team_id = resolve_team_id(&game, None)?;
    let result = ofm_core::finances::request_sponsor_pitch(&mut game, &team_id)?;
    state.set_game(game.clone());
    Ok(SponsorPitchCommandResponse { game, result })
}

pub fn request_marketing_campaign(
    state: &StateManager,
) -> Result<MarketingCampaignCommandResponse, String> {
    info!("[cmd] request_marketing_campaign");
    let mut game = state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;
    let team_id = resolve_team_id(&game, None)?;
    let result = ofm_core::finances::request_marketing_campaign(&mut game, &team_id)?;
    state.set_game(game.clone());
    Ok(MarketingCampaignCommandResponse { game, result })
}
