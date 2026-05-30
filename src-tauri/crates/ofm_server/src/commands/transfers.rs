//! Mirrors `src-tauri/src/commands/transfers.rs` — transfers / scouting.

use log::info;

use domain::negotiation::NegotiationFeedback;
use domain::player::Position;
use ofm_core::game::Game;
use ofm_core::game::{YouthScoutingObjective, YouthScoutingRegion};
use ofm_core::state::StateManager;
use ofm_core::transfers::{
    TransferBidFinancialProjection, TransferNegotiationDecision, TransferNegotiationOutcome,
};

const INVALID_YOUTH_SCOUTING_REGION_ERROR: &str = "be.error.transfers.invalidYouthScoutingRegion";
const INVALID_YOUTH_SCOUTING_OBJECTIVE_ERROR: &str =
    "be.error.transfers.invalidYouthScoutingObjective";
const INVALID_YOUTH_SCOUTING_TARGET_POSITION_ERROR: &str =
    "be.error.transfers.invalidYouthScoutingTargetPosition";

#[derive(Debug, Clone, serde::Serialize)]
pub struct TransferNegotiationCommandResponse {
    pub decision: TransferNegotiationDecision,
    pub suggested_fee: Option<u64>,
    pub is_terminal: bool,
    pub feedback: NegotiationFeedback,
    pub game: Game,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TransferBidFinancialProjectionCommandResponse {
    pub projection: TransferBidFinancialProjection,
}

fn load_game(state: &StateManager) -> Result<Game, String> {
    state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())
}

fn map_transfer_negotiation_response(
    outcome: TransferNegotiationOutcome,
    game: Game,
) -> TransferNegotiationCommandResponse {
    TransferNegotiationCommandResponse {
        decision: outcome.decision,
        suggested_fee: outcome.suggested_fee,
        is_terminal: outcome.is_terminal,
        feedback: outcome.feedback,
        game,
    }
}

pub fn toggle_transfer_list(state: &StateManager, player_id: String) -> Result<Game, String> {
    info!("[cmd] toggle_transfer_list: player_id={}", player_id);
    let mut game = load_game(state)?;
    if let Some(p) = game.players.iter_mut().find(|p| p.id == player_id) {
        p.transfer_listed = !p.transfer_listed;
    } else {
        return Err("be.error.playerNotFound".into());
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn toggle_loan_list(state: &StateManager, player_id: String) -> Result<Game, String> {
    info!("[cmd] toggle_loan_list: player_id={}", player_id);
    let mut game = load_game(state)?;
    if let Some(p) = game.players.iter_mut().find(|p| p.id == player_id) {
        p.loan_listed = !p.loan_listed;
    } else {
        return Err("be.error.playerNotFound".into());
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn make_transfer_bid(
    state: &StateManager,
    player_id: String,
    fee: u64,
) -> Result<TransferNegotiationCommandResponse, String> {
    info!("[cmd] make_transfer_bid: player_id={}, fee={}", player_id, fee);
    let mut game = load_game(state)?;
    let result = ofm_core::transfers::make_transfer_bid(&mut game, &player_id, fee)?;
    state.set_game(game.clone());
    Ok(map_transfer_negotiation_response(result, game))
}

pub fn preview_transfer_bid_financial_impact(
    state: &StateManager,
    player_id: String,
    fee: u64,
) -> Result<TransferBidFinancialProjectionCommandResponse, String> {
    let game = load_game(state)?;
    let projection =
        ofm_core::transfers::project_transfer_bid_financial_impact(&game, &player_id, fee)?;
    Ok(TransferBidFinancialProjectionCommandResponse { projection })
}

pub fn respond_to_offer(
    state: &StateManager,
    player_id: String,
    offer_id: String,
    accept: bool,
) -> Result<Game, String> {
    info!(
        "[cmd] respond_to_offer: player_id={}, offer_id={}, accept={}",
        player_id, offer_id, accept
    );
    let mut game = load_game(state)?;
    ofm_core::transfers::respond_to_offer(&mut game, &player_id, &offer_id, accept)?;
    state.set_game(game.clone());
    Ok(game)
}

pub fn counter_offer(
    state: &StateManager,
    player_id: String,
    offer_id: String,
    requested_fee: u64,
) -> Result<TransferNegotiationCommandResponse, String> {
    info!("[cmd] counter_offer: player_id={}, offer_id={}", player_id, offer_id);
    let mut game = load_game(state)?;
    let result = ofm_core::transfers::counter_offer(&mut game, &player_id, &offer_id, requested_fee)?;
    state.set_game(game.clone());
    Ok(map_transfer_negotiation_response(result, game))
}

pub fn send_scout(
    state: &StateManager,
    scout_id: String,
    player_id: String,
) -> Result<Game, String> {
    info!("[cmd] send_scout: scout_id={}, player_id={}", scout_id, player_id);
    let mut game = load_game(state)?;
    ofm_core::scouting::send_scout(&mut game, &scout_id, &player_id)?;
    state.set_game(game.clone());
    Ok(game)
}

pub fn start_youth_scouting(
    state: &StateManager,
    scout_id: String,
    region: Option<String>,
    objective: Option<String>,
    target_position: Option<String>,
) -> Result<Game, String> {
    info!("[cmd] start_youth_scouting: scout_id={}", scout_id);
    let mut game = load_game(state)?;
    let region = parse_youth_region(region.as_deref())?;
    let objective = parse_youth_objective(objective.as_deref())?;
    let target_position = parse_youth_target_position(target_position.as_deref())?;
    ofm_core::scouting::start_youth_scouting(
        &mut game,
        &scout_id,
        region,
        objective,
        target_position,
    )?;
    state.set_game(game.clone());
    Ok(game)
}

pub fn cancel_youth_scouting(
    state: &StateManager,
    assignment_id: String,
) -> Result<Game, String> {
    info!("[cmd] cancel_youth_scouting: assignment_id={}", assignment_id);
    let mut game = load_game(state)?;
    ofm_core::scouting::cancel_youth_scouting(&mut game, &assignment_id)?;
    state.set_game(game.clone());
    Ok(game)
}

pub fn reassign_youth_scouting(
    state: &StateManager,
    assignment_id: String,
    scout_id: String,
) -> Result<Game, String> {
    info!("[cmd] reassign_youth_scouting: assignment_id={}", assignment_id);
    let mut game = load_game(state)?;
    ofm_core::scouting::reassign_youth_scouting(&mut game, &assignment_id, &scout_id)?;
    state.set_game(game.clone());
    Ok(game)
}

fn parse_youth_region(value: Option<&str>) -> Result<YouthScoutingRegion, String> {
    match value {
        None | Some("") | Some("Domestic") => Ok(YouthScoutingRegion::Domestic),
        Some("International") => Ok(YouthScoutingRegion::International),
        Some(_) => Err(INVALID_YOUTH_SCOUTING_REGION_ERROR.to_string()),
    }
}

fn parse_youth_objective(value: Option<&str>) -> Result<YouthScoutingObjective, String> {
    match value {
        None | Some("") | Some("Balanced") => Ok(YouthScoutingObjective::Balanced),
        Some("HighPotential") => Ok(YouthScoutingObjective::HighPotential),
        Some("ReadySoon") => Ok(YouthScoutingObjective::ReadySoon),
        Some(_) => Err(INVALID_YOUTH_SCOUTING_OBJECTIVE_ERROR.to_string()),
    }
}

fn parse_youth_target_position(value: Option<&str>) -> Result<Option<Position>, String> {
    match value {
        None | Some("") => Ok(None),
        Some("Defender") => Ok(Some(Position::Defender)),
        Some("Midfielder") => Ok(Some(Position::Midfielder)),
        Some("Forward") => Ok(Some(Position::Forward)),
        Some(_) => Err(INVALID_YOUTH_SCOUTING_TARGET_POSITION_ERROR.to_string()),
    }
}
