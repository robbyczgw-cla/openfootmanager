//! Mirrors `src-tauri/src/commands/squad.rs` — squad / tactics / training.

use chrono::Datelike;
use log::info;
use serde_json::{json, Value};

use ofm_core::game::Game;
use ofm_core::state::StateManager;

fn parse_squad_role(squad_role: &str) -> Option<domain::player::SquadRole> {
    match squad_role {
        "Senior" => Some(domain::player::SquadRole::Senior),
        "Youth" => Some(domain::player::SquadRole::Youth),
        _ => None,
    }
}

fn player_age_on(current_date: chrono::NaiveDate, date_of_birth: &str) -> Option<i32> {
    let dob = chrono::NaiveDate::parse_from_str(date_of_birth, "%Y-%m-%d").ok()?;
    let mut age = current_date.year() - dob.year();
    if (current_date.month(), current_date.day()) < (dob.month(), dob.day()) {
        age -= 1;
    }
    Some(age)
}

fn require_managed_team(game: &Game) -> Result<String, String> {
    game.manager
        .team_id
        .clone()
        .ok_or("be.error.noTeamAssigned".to_string())
}

fn load_game(state: &StateManager) -> Result<Game, String> {
    state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())
}

pub fn set_formation(state: &StateManager, formation: String) -> Result<Game, String> {
    info!("[cmd] set_formation: {}", formation);
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;

    let parts: Vec<usize> = formation.split('-').filter_map(|s| s.parse().ok()).collect();
    let (num_def, num_mid, num_fwd) = match parts.len() {
        3 => (parts[0], parts[1], parts[2]),
        4 => (parts[0], parts[1] + parts[2], parts[3]),
        _ => (4, 4, 2),
    };

    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.formation = formation;
    }

    let player_ids: Vec<String> = game
        .players
        .iter()
        .filter(|p| {
            p.team_id.as_deref() == Some(&team_id)
                && p.position != domain::player::Position::Goalkeeper
        })
        .map(|p| p.id.clone())
        .collect();

    let mut sorted_ids = player_ids.clone();
    sorted_ids.sort_by(|a_id, b_id| {
        let pa = game.players.iter().find(|p| p.id == *a_id).unwrap();
        let pb = game.players.iter().find(|p| p.id == *b_id).unwrap();
        let def_a = pa.attributes.defending as u16
            + pa.attributes.tackling as u16
            + pa.attributes.strength as u16;
        let def_b = pb.attributes.defending as u16
            + pb.attributes.tackling as u16
            + pb.attributes.strength as u16;
        def_b.cmp(&def_a)
    });

    for (slot, pid) in sorted_ids.iter().enumerate() {
        let new_pos = if slot < num_def {
            domain::player::Position::Defender
        } else if slot < num_def + num_mid {
            domain::player::Position::Midfielder
        } else if slot < num_def + num_mid + num_fwd {
            domain::player::Position::Forward
        } else {
            continue;
        };
        if let Some(player) = game.players.iter_mut().find(|p| p.id == *pid) {
            player.position = new_pos;
        }
    }

    state.set_game(game.clone());
    Ok(game)
}

pub fn set_starting_xi(state: &StateManager, player_ids: Vec<String>) -> Result<Game, String> {
    info!("[cmd] set_starting_xi: {} players", player_ids.len());
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.starting_xi_ids = player_ids;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn set_play_style(state: &StateManager, play_style: String) -> Result<Game, String> {
    info!("[cmd] set_play_style: {}", play_style);
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    let style = match play_style.as_str() {
        "Attacking" => domain::team::PlayStyle::Attacking,
        "Defensive" => domain::team::PlayStyle::Defensive,
        "Possession" => domain::team::PlayStyle::Possession,
        "Counter" => domain::team::PlayStyle::Counter,
        "HighPress" => domain::team::PlayStyle::HighPress,
        _ => domain::team::PlayStyle::Balanced,
    };
    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.play_style = style;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn set_team_match_roles(
    state: &StateManager,
    match_roles: domain::team::MatchRoles,
) -> Result<Game, String> {
    info!("[cmd] set_team_match_roles");
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.match_roles = match_roles;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn set_training(state: &StateManager, focus: String, intensity: String) -> Result<Game, String> {
    info!("[cmd] set_training: focus={}, intensity={}", focus, intensity);
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    let training_focus = match focus.as_str() {
        "Physical" => domain::team::TrainingFocus::Physical,
        "Technical" => domain::team::TrainingFocus::Technical,
        "Tactical" => domain::team::TrainingFocus::Tactical,
        "Defending" => domain::team::TrainingFocus::Defending,
        "Attacking" => domain::team::TrainingFocus::Attacking,
        "Recovery" => domain::team::TrainingFocus::Recovery,
        _ => domain::team::TrainingFocus::Physical,
    };
    let training_intensity = match intensity.as_str() {
        "Low" => domain::team::TrainingIntensity::Low,
        "Medium" => domain::team::TrainingIntensity::Medium,
        "High" => domain::team::TrainingIntensity::High,
        _ => domain::team::TrainingIntensity::Medium,
    };
    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.training_focus = training_focus;
        team.training_intensity = training_intensity;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn set_training_schedule(state: &StateManager, schedule: String) -> Result<Game, String> {
    info!("[cmd] set_training_schedule: {}", schedule);
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    let training_schedule = match schedule.as_str() {
        "Intense" => domain::team::TrainingSchedule::Intense,
        "Balanced" => domain::team::TrainingSchedule::Balanced,
        "Light" => domain::team::TrainingSchedule::Light,
        _ => domain::team::TrainingSchedule::Balanced,
    };
    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.training_schedule = training_schedule;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn set_training_groups(
    state: &StateManager,
    groups: Vec<domain::team::TrainingGroup>,
) -> Result<Game, String> {
    info!("[cmd] set_training_groups: {} groups", groups.len());
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    if let Some(team) = game.teams.iter_mut().find(|t| t.id == team_id) {
        team.training_groups = groups;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn set_player_training_focus(
    state: &StateManager,
    player_id: String,
    focus: Option<String>,
) -> Result<Game, String> {
    info!(
        "[cmd] set_player_training_focus: player={}, focus={:?}",
        player_id, focus
    );
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    let training_focus = focus.as_deref().and_then(|f| match f {
        "Physical" => Some(domain::team::TrainingFocus::Physical),
        "Technical" => Some(domain::team::TrainingFocus::Technical),
        "Tactical" => Some(domain::team::TrainingFocus::Tactical),
        "Defending" => Some(domain::team::TrainingFocus::Defending),
        "Attacking" => Some(domain::team::TrainingFocus::Attacking),
        "Recovery" => Some(domain::team::TrainingFocus::Recovery),
        _ => None,
    });
    if let Some(player) = game
        .players
        .iter_mut()
        .find(|p| p.id == player_id && p.team_id.as_deref() == Some(team_id.as_str()))
    {
        player.training_focus = training_focus;
    } else {
        return Err("be.error.playerNotFound".to_string());
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn set_player_squad_role(
    state: &StateManager,
    player_id: String,
    squad_role: String,
) -> Result<Game, String> {
    info!(
        "[cmd] set_player_squad_role: player={}, squad_role={}",
        player_id, squad_role
    );
    let mut game = load_game(state)?;
    let team_id = require_managed_team(&game)?;
    let target_role =
        parse_squad_role(&squad_role).ok_or("be.error.invalidSquadRole".to_string())?;
    let current_date = game.clock.current_date.date_naive();

    let player_index = game
        .players
        .iter()
        .position(|player| player.id == player_id)
        .ok_or("be.error.playerNotFound".to_string())?;

    if game.players[player_index].team_id.as_deref() != Some(team_id.as_str()) {
        return Err("be.error.playerNotInSquad".to_string());
    }

    if matches!(target_role, domain::player::SquadRole::Youth) {
        let age = player_age_on(current_date, &game.players[player_index].date_of_birth)
            .ok_or("be.error.invalidDateOfBirth".to_string())?;
        if age > 21 {
            return Err("be.error.youthAcademyOverage".to_string());
        }
    }

    game.players[player_index].squad_role = target_role;

    if matches!(target_role, domain::player::SquadRole::Youth) {
        if let Some(team) = game.teams.iter_mut().find(|team| team.id == team_id) {
            team.starting_xi_ids.retain(|id| id != &player_id);
        }
    }

    state.set_game(game.clone());
    Ok(game)
}

pub fn auto_select_set_pieces(
    state: &StateManager,
    player_ids: Vec<String>,
) -> Result<Value, String> {
    let game = load_game(state)?;
    let (captain, penalty, free_kick, corner) =
        ofm_core::live_match_manager::auto_select_set_pieces(&game, &player_ids);
    Ok(json!({
        "captain": captain,
        "penalty_taker": penalty,
        "free_kick_taker": free_kick,
        "corner_taker": corner,
    }))
}
