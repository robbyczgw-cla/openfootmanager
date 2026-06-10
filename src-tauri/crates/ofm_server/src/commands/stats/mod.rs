//! Stats commands. `dto.rs`, `shared.rs`, `player.rs`, `team.rs` are copied
//! verbatim from `src-tauri/src/commands/stats/`; this module exposes plain
//! functions (no Tauri wrappers) over their `*_internal` helpers.

mod dto;
mod player;
mod shared;
mod team;

use ofm_core::state::StateManager;

pub use self::dto::{
    PlayerMatchHistoryEntryDto, PlayerStatsOverviewDto, TeamMatchHistoryEntryDto,
    TeamStatsOverviewDto,
};
use self::player::{get_player_match_history_internal, get_player_stats_overview_internal};
use self::team::{get_team_match_history_internal, get_team_stats_overview_internal};

pub fn get_player_match_history(
    state: &StateManager,
    player_id: String,
    limit: Option<usize>,
) -> Result<Vec<PlayerMatchHistoryEntryDto>, String> {
    get_player_match_history_internal(state, &player_id, limit)
}

pub fn get_player_stats_overview(
    state: &StateManager,
    player_id: String,
) -> Result<PlayerStatsOverviewDto, String> {
    get_player_stats_overview_internal(state, &player_id)
}

pub fn get_team_stats_overview(
    state: &StateManager,
    team_id: String,
) -> Result<Option<TeamStatsOverviewDto>, String> {
    get_team_stats_overview_internal(state, &team_id)
}

pub fn get_team_match_history(
    state: &StateManager,
    team_id: String,
    limit: Option<usize>,
) -> Result<Vec<TeamMatchHistoryEntryDto>, String> {
    get_team_match_history_internal(state, &team_id, limit)
}
