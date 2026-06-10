//! Live-match commands. Mirrors `src-tauri/src/commands/live_match.rs`.
//!
//! The match-engine orchestration lives in the copied `live_match_service` and
//! `team_talk` modules (mirrored from `src-tauri/src/application/*`); this file
//! provides the command-level wrappers plus the press-conference handler.

use std::collections::HashMap;

use log::info;
use rand::RngExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use ofm_core::state::StateManager;

pub use super::live_match_service::FinishLiveMatchResponse;
use super::live_match_service::{
    apply_match_command as apply_match_command_service,
    finish_live_match as finish_live_match_service,
    get_match_snapshot as get_match_snapshot_service, start_live_match as start_live_match_service,
    step_live_match as step_live_match_service,
};
use super::team_talk::apply_team_talk as apply_team_talk_service;

#[derive(Debug, Deserialize)]
pub struct PressConferenceAnswer {
    question_id: String,
    response_id: String,
    #[serde(rename = "response_tone")]
    _response_tone: String,
    response_text: String,
    #[serde(default)]
    response_text_key: String,
    #[serde(default)]
    response_text_params: HashMap<String, String>,
    #[serde(default)]
    question_text: String,
    #[serde(default)]
    player_id: String,
}

#[derive(Debug, Serialize)]
struct LocalizedPressQuote {
    #[serde(skip_serializing_if = "String::is_empty")]
    key: String,
    fallback: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    params: HashMap<String, String>,
}

pub fn start_live_match(
    state: &StateManager,
    fixture_index: usize,
    mode: String,
    allows_extra_time: bool,
) -> Result<engine::MatchSnapshot, String> {
    start_live_match_service(state, fixture_index, &mode, allows_extra_time)
}

pub fn step_live_match(
    state: &StateManager,
    minutes: u16,
) -> Result<Vec<engine::MinuteResult>, String> {
    step_live_match_service(state, minutes)
}

pub fn apply_match_command(
    state: &StateManager,
    command: engine::MatchCommand,
) -> Result<engine::MatchSnapshot, String> {
    apply_match_command_service(state, command)
}

pub fn get_match_snapshot(state: &StateManager) -> Result<engine::MatchSnapshot, String> {
    get_match_snapshot_service(state)
}

pub fn finish_live_match(state: &StateManager) -> Result<FinishLiveMatchResponse, String> {
    finish_live_match_service(state)
}

pub fn apply_team_talk(
    state: &StateManager,
    tone: String,
    context: String,
) -> Result<Vec<Value>, String> {
    info!("[cmd] apply_team_talk: tone={}, context={}", tone, context);
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession")?;
    let seed = rand::rng().random::<u64>();
    let results = apply_team_talk_service(&mut game, &tone, &context, seed)?;
    state.set_game(game);
    Ok(results)
}

#[allow(clippy::too_many_arguments)]
pub fn submit_press_conference(
    state: &StateManager,
    answers: Vec<PressConferenceAnswer>,
    home_team: String,
    away_team: String,
    home_score: u8,
    away_score: u8,
    user_team_name: String,
    user_team_id: String,
) -> Result<Value, String> {
    info!(
        "[cmd] submit_press_conference: {} {} - {} {}",
        home_team, home_score, away_score, away_team
    );
    let mut game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession")?;

    let today = game.clock.current_date.format("%Y-%m-%d").to_string();
    let mut rng = rand::rng();

    let mut quotes: Vec<String> = Vec::new();
    let mut localized_quotes: Vec<LocalizedPressQuote> = Vec::new();
    let mut morale_delta: i16 = 0;
    let mut mentioned_player_ids: Vec<String> = Vec::new();

    for answer in &answers {
        let rid = answer.response_id.as_str();
        let text = answer.response_text.as_str();
        let qid = answer.question_id.as_str();

        let _ = &answer.question_text;

        if !text.is_empty() {
            quotes.push(format!("\"{}\"", text));
            localized_quotes.push(LocalizedPressQuote {
                key: answer.response_text_key.clone(),
                fallback: text.to_string(),
                params: answer.response_text_params.clone(),
            });
        }

        if !answer.player_id.is_empty() {
            mentioned_player_ids.push(answer.player_id.clone());
        }

        match rid {
            "humble" | "fair" | "positive" | "focused" | "grateful" | "patience" | "appreciate"
            | "understand" => morale_delta += rng.random_range(1..=3),
            "confident" | "ambitious" | "shared" => morale_delta += rng.random_range(2..=5),
            "defiant" | "frustrated" => morale_delta += rng.random_range(-2..=2),
            "curt" | "evasive" => morale_delta += rng.random_range(-3..=0),
            "accept" | "detailed" | "apologize" => morale_delta += rng.random_range(0..=2),
            "deflect" => morale_delta += rng.random_range(-1..=1),
            "praise" => morale_delta += rng.random_range(3..=6),
            "demanding" => morale_delta += rng.random_range(-2..=3),
            _ => {}
        }

        if qid == "player_focus" && !answer.player_id.is_empty() {
            let player_delta: i16 = match rid {
                "praise" => rng.random_range(4..=8),
                "demanding" => rng.random_range(-3..=4),
                "deflect" => rng.random_range(-2..=1),
                _ => rng.random_range(0..=3),
            };
            if let Some(p) = game.players.iter_mut().find(|p| p.id == answer.player_id) {
                p.morale = ((p.morale as i16) + player_delta).clamp(10, 100) as u8;
            }
        }
    }

    morale_delta = morale_delta.clamp(-8, 8);
    if morale_delta != 0 {
        for p in game.players.iter_mut() {
            if p.team_id.as_deref() == Some(&user_team_id) {
                p.morale = ((p.morale as i16) + morale_delta).clamp(10, 100) as u8;
            }
        }
    }

    let result_str = format!("{} {} - {} {}", home_team, home_score, away_score, away_team);
    let headline_key = if quotes.is_empty() {
        "be.news.pressConference.headlinePostMatch"
    } else if rng.random::<bool>() {
        "be.news.pressConference.headlineManagerQuote"
    } else {
        "be.news.pressConference.headlinePressConf"
    };

    let body_key = if quotes.len() > 1 {
        "be.news.pressConference.bodyMultiple"
    } else if quotes.len() == 1 {
        "be.news.pressConference.bodySingle"
    } else {
        "be.news.pressConference.bodyNone"
    };

    let mut i18n_params = HashMap::new();
    i18n_params.insert("team".to_string(), user_team_name.clone());
    i18n_params.insert("result".to_string(), result_str.clone());
    if !localized_quotes.is_empty() {
        if let Ok(serialized_quotes) = serde_json::to_string(&localized_quotes) {
            i18n_params.insert("quotesData".to_string(), serialized_quotes);
        }
        i18n_params.insert("quote".to_string(), quotes[0].trim_matches('"').to_string());
    }

    let article_id = format!("press_conf_{}", today);
    let article = domain::news::NewsArticle::new(
        article_id,
        String::new(),
        String::new(),
        String::new(),
        today.clone(),
        domain::news::NewsCategory::MatchReport,
    )
    .with_teams(vec![user_team_id.clone()])
    .with_players(mentioned_player_ids)
    .with_i18n(headline_key, body_key, "be.source.sportsDaily", i18n_params);

    game.news.push(article);
    state.set_game(game.clone());

    Ok(json!({
        "game": game,
        "morale_delta": morale_delta
    }))
}
