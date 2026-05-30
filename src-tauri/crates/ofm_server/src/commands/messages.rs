//! Mirrors `src-tauri/src/commands/messages.rs`.

use std::collections::HashSet;

use log::info;
use serde_json::{json, Value};

use ofm_core::game::Game;
use ofm_core::state::StateManager;

fn load_game(state: &StateManager) -> Result<Game, String> {
    state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())
}

pub fn mark_message_read(state: &StateManager, message_id: String) -> Result<Game, String> {
    let mut game = load_game(state)?;
    if let Some(msg) = game.messages.iter_mut().find(|m| m.id == message_id) {
        msg.read = true;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn delete_message(state: &StateManager, message_id: String) -> Result<Game, String> {
    let mut game = load_game(state)?;
    game.messages.retain(|message| message.id != message_id);
    state.set_game(game.clone());
    Ok(game)
}

pub fn delete_messages(state: &StateManager, message_ids: Vec<String>) -> Result<Game, String> {
    let mut game = load_game(state)?;
    let message_ids: HashSet<String> = message_ids.into_iter().collect();
    game.messages
        .retain(|message| !message_ids.contains(&message.id));
    state.set_game(game.clone());
    Ok(game)
}

pub fn mark_all_messages_read(state: &StateManager) -> Result<Game, String> {
    let mut game = load_game(state)?;
    for msg in game.messages.iter_mut() {
        msg.read = true;
    }
    state.set_game(game.clone());
    Ok(game)
}

pub fn clear_old_messages(state: &StateManager) -> Result<Game, String> {
    let mut game = load_game(state)?;
    let current_date = game.clock.current_date.format("%Y-%m-%d").to_string();
    game.messages.retain(|m| {
        if !m.read {
            return true;
        }
        if m.actions.iter().any(|a| !a.resolved) {
            return true;
        }
        if let Ok(msg_date) = chrono::NaiveDate::parse_from_str(&m.date, "%Y-%m-%d") {
            if let Ok(cur_date) = chrono::NaiveDate::parse_from_str(&current_date, "%Y-%m-%d") {
                return (cur_date - msg_date).num_days() <= 14;
            }
        }
        false
    });
    state.set_game(game.clone());
    Ok(game)
}

pub fn resolve_message_action(
    state: &StateManager,
    message_id: String,
    action_id: String,
    option_id: Option<String>,
) -> Result<Value, String> {
    info!(
        "[cmd] resolve_message_action: msg={}, action={}, option={:?}",
        message_id, action_id, option_id
    );
    let mut game = load_game(state)?;

    let (effect, effect_i18n_key, effect_i18n_params) = if let Some(opt) = option_id.as_deref() {
        let player_effect =
            ofm_core::player_events::apply_player_response(&mut game, &message_id, &action_id, opt);
        if let Some(player_effect) = player_effect {
            (
                Some(player_effect.message),
                Some(player_effect.i18n_key),
                Some(player_effect.i18n_params),
            )
        } else {
            let random_effect = ofm_core::random_events::apply_event_response(
                &mut game,
                &message_id,
                &action_id,
                opt,
            );
            if let Some(effect) = random_effect {
                (
                    Some(effect.message),
                    Some(effect.i18n_key),
                    Some(effect.i18n_params),
                )
            } else {
                match ofm_core::job_offers::apply_job_offer_response(
                    &mut game,
                    &message_id,
                    &action_id,
                    opt,
                ) {
                    Some(effect) => (
                        Some(effect.message),
                        Some(effect.i18n_key),
                        Some(effect.i18n_params),
                    ),
                    None => match ofm_core::scouting::apply_youth_recruitment_response(
                        &mut game,
                        &message_id,
                        &action_id,
                        opt,
                    ) {
                        Some(effect) => (
                            Some(effect.message),
                            Some(effect.i18n_key),
                            Some(effect.i18n_params),
                        ),
                        None => (None, None, None),
                    },
                }
            }
        }
    } else {
        if let Some(msg) = game.messages.iter_mut().find(|m| m.id == message_id) {
            if let Some(action) = msg.actions.iter_mut().find(|a| a.id == action_id) {
                action.resolved = true;
            }
        }
        (None, None, None)
    };

    state.set_game(game.clone());
    Ok(json!({
        "game": game,
        "effect": effect,
        "effect_i18n_key": effect_i18n_key,
        "effect_i18n_params": effect_i18n_params
    }))
}
