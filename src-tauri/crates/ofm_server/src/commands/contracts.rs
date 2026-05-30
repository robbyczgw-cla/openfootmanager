//! Mirrors `src-tauri/src/commands/contracts.rs`.

use log::info;
use serde::Serialize;

use domain::negotiation::NegotiationFeedback;
use domain::player::RenewalSessionStatus;
use ofm_core::contracts::{
    ContractTerminationPreview, ContractTerminationResult, DelegatedRenewalOptions,
    DelegatedRenewalReport, RenewalDecision, RenewalFinancialProjection, RenewalOffer,
};
use ofm_core::game::Game;
use ofm_core::squad_safety::SquadSafetyReport;
use ofm_core::state::StateManager;

#[derive(Debug, Clone, Serialize)]
pub struct RenewalCommandResponse {
    pub outcome: RenewalDecision,
    pub game: Game,
    pub suggested_wage: Option<u32>,
    pub suggested_years: Option<u32>,
    pub session_status: String,
    pub is_terminal: bool,
    pub cooled_off: bool,
    pub feedback: Option<NegotiationFeedback>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DelegatedRenewalCommandResponse {
    pub game: Game,
    pub report: DelegatedRenewalReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenewalFinancialProjectionCommandResponse {
    pub projection: RenewalFinancialProjection,
}

#[derive(Debug, Clone, Serialize)]
pub struct FreeAgentContractCommandResponse {
    pub outcome: RenewalDecision,
    pub game: Game,
    pub suggested_wage: Option<u32>,
    pub suggested_years: Option<u32>,
    pub session_status: String,
    pub is_terminal: bool,
    pub cooled_off: bool,
    pub feedback: Option<NegotiationFeedback>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FreeAgentContractProjectionCommandResponse {
    pub projection: RenewalFinancialProjection,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractExitIntentCommandResponse {
    pub game: Game,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractTerminationPreviewCommandResponse {
    pub preview: ContractTerminationPreview,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractTerminationCommandResponse {
    pub game: Game,
    pub severance_cost: i64,
    pub squad_safety: SquadSafetyReport,
}

fn serialize_session_status(status: RenewalSessionStatus) -> String {
    match status {
        RenewalSessionStatus::Idle => "idle",
        RenewalSessionStatus::Open => "open",
        RenewalSessionStatus::Agreed => "agreed",
        RenewalSessionStatus::Blocked => "blocked",
        RenewalSessionStatus::Stalled => "stalled",
    }
    .to_string()
}

fn load_game(state: &StateManager) -> Result<Game, String> {
    state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())
}

pub fn propose_renewal(
    state: &StateManager,
    player_id: String,
    weekly_wage: u32,
    contract_years: u32,
) -> Result<RenewalCommandResponse, String> {
    info!("[cmd] propose_renewal: player_id={}", player_id);
    let mut game = load_game(state)?;
    let outcome = ofm_core::contracts::propose_renewal(
        &mut game,
        &player_id,
        RenewalOffer {
            weekly_wage,
            contract_years,
        },
    )?;
    state.set_game(game.clone());
    Ok(RenewalCommandResponse {
        outcome: outcome.decision,
        game,
        suggested_wage: outcome.suggested_wage,
        suggested_years: outcome.suggested_years,
        session_status: serialize_session_status(outcome.session_status),
        is_terminal: outcome.is_terminal,
        cooled_off: outcome.cooled_off,
        feedback: outcome.feedback,
    })
}

pub fn delegate_renewals(
    state: &StateManager,
    player_ids: Option<Vec<String>>,
    max_wage_increase_pct: u32,
    max_contract_years: u32,
) -> Result<DelegatedRenewalCommandResponse, String> {
    info!("[cmd] delegate_renewals");
    let mut game = load_game(state)?;
    let report = ofm_core::contracts::delegate_renewals(
        &mut game,
        DelegatedRenewalOptions {
            player_ids,
            max_wage_increase_pct,
            max_contract_years,
        },
    )?;
    state.set_game(game.clone());
    Ok(DelegatedRenewalCommandResponse { game, report })
}

pub fn preview_renewal_financial_impact(
    state: &StateManager,
    player_id: String,
    weekly_wage: u32,
) -> Result<RenewalFinancialProjectionCommandResponse, String> {
    let game = load_game(state)?;
    let projection =
        ofm_core::contracts::project_renewal_financial_impact(&game, &player_id, weekly_wage)?;
    Ok(RenewalFinancialProjectionCommandResponse { projection })
}

pub fn offer_free_agent_contract(
    state: &StateManager,
    player_id: String,
    weekly_wage: u32,
    contract_years: u32,
) -> Result<FreeAgentContractCommandResponse, String> {
    info!("[cmd] offer_free_agent_contract: player_id={}", player_id);
    let mut game = load_game(state)?;
    let outcome = ofm_core::contracts::offer_free_agent_contract(
        &mut game,
        &player_id,
        RenewalOffer {
            weekly_wage,
            contract_years,
        },
    )?;
    state.set_game(game.clone());
    Ok(FreeAgentContractCommandResponse {
        outcome: outcome.decision,
        game,
        suggested_wage: outcome.suggested_wage,
        suggested_years: outcome.suggested_years,
        session_status: serialize_session_status(outcome.session_status),
        is_terminal: outcome.is_terminal,
        cooled_off: outcome.cooled_off,
        feedback: outcome.feedback,
    })
}

pub fn preview_free_agent_contract_impact(
    state: &StateManager,
    player_id: String,
    weekly_wage: u32,
) -> Result<FreeAgentContractProjectionCommandResponse, String> {
    let game = load_game(state)?;
    let projection =
        ofm_core::contracts::project_free_agent_contract_impact(&game, &player_id, weekly_wage)?;
    Ok(FreeAgentContractProjectionCommandResponse { projection })
}

pub fn set_contract_exit_intent(
    state: &StateManager,
    player_id: String,
    reason: Option<String>,
) -> Result<ContractExitIntentCommandResponse, String> {
    info!("[cmd] set_contract_exit_intent: player_id={}", player_id);
    let mut game = load_game(state)?;
    ofm_core::contracts::set_contract_exit_intent(&mut game, &player_id, reason)?;
    state.set_game(game.clone());
    Ok(ContractExitIntentCommandResponse { game })
}

pub fn clear_contract_exit_intent(
    state: &StateManager,
    player_id: String,
) -> Result<ContractExitIntentCommandResponse, String> {
    info!("[cmd] clear_contract_exit_intent: player_id={}", player_id);
    let mut game = load_game(state)?;
    ofm_core::contracts::clear_contract_exit_intent(&mut game, &player_id)?;
    state.set_game(game.clone());
    Ok(ContractExitIntentCommandResponse { game })
}

pub fn preview_contract_termination(
    state: &StateManager,
    player_id: String,
) -> Result<ContractTerminationPreviewCommandResponse, String> {
    let game = load_game(state)?;
    let preview = ofm_core::contracts::preview_contract_termination(&game, &player_id)?;
    Ok(ContractTerminationPreviewCommandResponse { preview })
}

pub fn terminate_contract_now(
    state: &StateManager,
    player_id: String,
) -> Result<ContractTerminationCommandResponse, String> {
    info!("[cmd] terminate_contract_now: player_id={}", player_id);
    let mut game = load_game(state)?;
    let ContractTerminationResult {
        severance_cost,
        squad_safety,
    } = ofm_core::contracts::terminate_contract_now(&mut game, &player_id)?;
    state.set_game(game.clone());
    Ok(ContractTerminationCommandResponse {
        game,
        severance_cost,
        squad_safety,
    })
}
