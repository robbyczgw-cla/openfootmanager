//! New-game / save-lifecycle commands.
//!
//! The orchestration helpers below are mirrored from
//! `src-tauri/src/commands/game.rs`. They take plain values (no Tauri
//! `State<..>`) and call `ofm_core` / `db` directly. If upstream changes the
//! desktop `game.rs` orchestration, re-sync the helpers here.

use chrono::{Datelike, Duration, TimeZone, Utc};
use log::info;

use db::{save_index::SaveEntry, save_manager::SaveManager};
use domain::manager::Manager;
use domain::stats::StatsState;
use ofm_core::clock::GameClock;
use ofm_core::game::Game;
use ofm_core::state::StateManager;

use crate::state::AppState;

const SAVE_MANAGER_UNAVAILABLE_ERROR: &str = "be.error.saveManagerUnavailable";
const DEFAULT_GENERATED_HISTORY_DEPTH_YEARS: u32 = 12;
const MAX_GENERATED_HISTORY_DEPTH_YEARS: u32 = 24;

fn load_world_data_from_path(world_source: &str) -> Result<ofm_core::generator::WorldData, String> {
    let path = world_source.strip_prefix("file:").unwrap_or(world_source);
    let json =
        std::fs::read_to_string(path).map_err(|_| "be.error.worldReadFileFailed".to_string())?;
    ofm_core::generator::load_world_from_json(&json)
}

fn require_active_stats_state(state: &StateManager) -> Result<StatsState, String> {
    state
        .get_stats_state(|stats| stats.clone())
        .ok_or("be.error.noActiveStatsSession".to_string())
}

fn default_league_name() -> String {
    ["Premier", "Division"].join(" ")
}

fn long_date_format() -> String {
    ['%', 'B', ' ', '%', 'd', ',', ' ', '%', 'Y']
        .into_iter()
        .collect()
}

fn default_save_name(manager_name: &str) -> String {
    let mut save_name = manager_name.to_string();
    save_name.push('\'');
    save_name.push('s');
    save_name.push(' ');
    save_name.push_str("Career");
    save_name
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawStartupOptions {
    #[serde(default)]
    start_year: Option<i32>,
    #[serde(default)]
    start_phase: Option<String>,
    #[serde(default)]
    history_depth_years: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartPhase {
    SeasonStart,
    MidSeason,
}

impl StartPhase {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "seasonStart" => Some(Self::SeasonStart),
            "midSeason" => Some(Self::MidSeason),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SeasonStart => "seasonStart",
            Self::MidSeason => "midSeason",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartupOptions {
    start_year: i32,
    start_phase: StartPhase,
    history_depth_years: u32,
}

fn default_start_year() -> i32 {
    chrono::Utc::now().year().max(2020)
}

fn default_history_depth_years() -> u32 {
    DEFAULT_GENERATED_HISTORY_DEPTH_YEARS
}

fn start_date_for_year(start_year: i32) -> Result<chrono::DateTime<Utc>, String> {
    Utc.with_ymd_and_hms(start_year, 7, 1, 0, 0, 0)
        .single()
        .ok_or_else(|| "be.error.createManager.invalidStartYear".to_string())
}

fn current_date_for_phase(
    start_year: i32,
    start_phase: StartPhase,
) -> Result<chrono::DateTime<Utc>, String> {
    let start_date = start_date_for_year(start_year)?;
    Ok(match start_phase {
        StartPhase::SeasonStart => start_date,
        StartPhase::MidSeason => start_date + Duration::days(120),
    })
}

fn age_on_date(birth_date: chrono::NaiveDate, reference_date: chrono::NaiveDate) -> i64 {
    let mut age = i64::from(reference_date.year() - birth_date.year());
    let has_had_birthday =
        (reference_date.month(), reference_date.day()) >= (birth_date.month(), birth_date.day());
    if !has_had_birthday {
        age -= 1;
    }
    age
}

fn start_phase_for_game(game: &Game) -> StartPhase {
    if game.clock.current_date > game.clock.start_date {
        StartPhase::MidSeason
    } else {
        StartPhase::SeasonStart
    }
}

fn preseason_season_start(clock: &GameClock) -> chrono::DateTime<Utc> {
    clock.start_date + Duration::days(30)
}

fn preseason_league_year(clock: &GameClock) -> u32 {
    u32::try_from(clock.start_date.year()).unwrap_or(2020)
}

fn normalize_startup_options(raw: Option<RawStartupOptions>) -> Result<StartupOptions, String> {
    let raw = raw.unwrap_or_default();
    let start_year = raw.start_year.unwrap_or_else(default_start_year);
    if start_year < 2020 {
        return Err("be.error.createManager.startYearMin".to_string());
    }

    let start_phase = match raw.start_phase.as_deref() {
        None | Some("") => StartPhase::SeasonStart,
        Some(value) => StartPhase::parse(value)
            .ok_or_else(|| "be.error.createManager.invalidStartPhase".to_string())?,
    };
    let history_depth_years = raw
        .history_depth_years
        .unwrap_or_else(default_history_depth_years);
    if history_depth_years > MAX_GENERATED_HISTORY_DEPTH_YEARS {
        return Err("be.error.createManager.historyDepthMax".to_string());
    }

    Ok(StartupOptions {
        start_year,
        start_phase,
        history_depth_years,
    })
}

fn apply_generated_past_history(game: &mut Game, startup_options: &StartupOptions) {
    ofm_core::history_generation::generate_past_world_history(
        game,
        startup_options.start_year,
        startup_options.history_depth_years,
    );
}

fn load_world_data(world_source: Option<&str>) -> Result<ofm_core::generator::WorldData, String> {
    match world_source {
        None | Some("random") => Ok(ofm_core::generator::generate_world_data(None)),
        Some(source) => load_world_data_from_path(source),
    }
}

fn world_start_year(
    startup_options: &StartupOptions,
    metadata: &ofm_core::generator::WorldDataMetadata,
) -> i32 {
    match metadata.kind {
        ofm_core::generator::WorldDataKind::HistoricalSnapshot => {
            metadata.base_year.unwrap_or(startup_options.start_year)
        }
        ofm_core::generator::WorldDataKind::RosterBaseline => startup_options.start_year,
    }
}

fn game_clock_for_world(
    startup_options: &StartupOptions,
    metadata: &ofm_core::generator::WorldDataMetadata,
) -> Result<GameClock, String> {
    let start_year = world_start_year(startup_options, metadata);
    let mut clock = GameClock::new(start_date_for_year(start_year)?);
    clock.current_date = match metadata.kind {
        ofm_core::generator::WorldDataKind::HistoricalSnapshot => metadata
            .snapshot_date
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or(current_date_for_phase(
                start_year,
                startup_options.start_phase,
            )?),
        ofm_core::generator::WorldDataKind::RosterBaseline => {
            current_date_for_phase(startup_options.start_year, startup_options.start_phase)?
        }
    };
    Ok(clock)
}

fn build_game_from_world_data(
    clock: GameClock,
    manager: Manager,
    startup_options: &StartupOptions,
    world: ofm_core::generator::WorldData,
) -> (Game, StatsState) {
    let ofm_core::generator::WorldData {
        teams,
        players,
        staff,
        managers,
        league,
        news,
        stats,
        world_history,
        metadata,
        ..
    } = world;

    let mut game = Game::new(clock, manager, teams, players, staff, vec![]);

    match metadata.kind {
        ofm_core::generator::WorldDataKind::HistoricalSnapshot => {
            game.managers.extend(
                managers
                    .into_iter()
                    .filter(|existing_manager| existing_manager.id != game.manager.id),
            );
            game.league = league;
            game.news = news;
            game.world_history = world_history;
            ofm_core::season_context::refresh_game_context(&mut game);
            (game, stats)
        }
        ofm_core::generator::WorldDataKind::RosterBaseline => {
            apply_generated_past_history(&mut game, startup_options);
            (game, StatsState::default())
        }
    }
}

fn has_existing_world_context(game: &Game, stats_state: &StatsState) -> bool {
    game.league.is_some()
        || !game.news.is_empty()
        || !stats_state.player_matches.is_empty()
        || !stats_state.team_matches.is_empty()
}

fn bootstrap_existing_world_takeover(
    game: &mut Game,
    team_id: &str,
    stats_state: StatsState,
) -> Result<StatsState, String> {
    let team = game
        .teams
        .iter()
        .find(|t| t.id == team_id)
        .ok_or("be.error.teamNotFound".to_string())?;
    let team_name = team.name.clone();

    ofm_core::ai_hiring::seed_ai_managers(game);

    let takeover_date = game.clock.current_date.format("%Y-%m-%d").to_string();
    let incumbent_manager_id = game
        .teams
        .iter()
        .find(|candidate| candidate.id == team_id)
        .and_then(|candidate| candidate.manager_id.clone());

    if incumbent_manager_id.as_deref() != Some(game.manager.id.as_str()) {
        let fired = ofm_core::firing::fire_ai_manager_for_team(game, team_id, &takeover_date);
        if !fired {
            if let Some(team) = game
                .teams
                .iter_mut()
                .find(|candidate| candidate.id == team_id)
            {
                team.manager_id = None;
            }
        }
        ofm_core::job_offers::hire_manager(game, team_id, &takeover_date)?;
    }

    let staff_msg = ofm_core::messages::staff_advice_message(&team_name, team_id, &takeover_date);
    game.messages.push(staff_msg);
    ofm_core::player_events::generate_takeover_contract_review_message(game);
    ofm_core::season_context::refresh_game_context(game);

    Ok(stats_state)
}

fn create_new_save(
    save_manager: &mut SaveManager,
    game: &Game,
    stats_state: &StatsState,
    save_name: &str,
) -> Result<String, String> {
    save_manager.create_save_with_stats(game, stats_state, save_name)
}

fn bootstrap_season_start(game: &mut Game, team_id: &str) -> Result<StatsState, String> {
    let team = game
        .teams
        .iter()
        .find(|t| t.id == team_id)
        .ok_or("be.error.teamNotFound".to_string())?;
    let team_name = team.name.clone();

    game.manager.hire(team_id.to_string());
    if let Some(t) = game.teams.iter_mut().find(|t| t.id == team_id) {
        t.manager_id = Some(game.manager.id.clone());
    }
    game.manager_id = game.manager.id.clone();
    ofm_core::ai_hiring::seed_ai_managers(game);

    let season_start = preseason_season_start(&game.clock);
    let team_ids: Vec<String> = game.teams.iter().map(|t| t.id.clone()).collect();
    let league_name = default_league_name();
    let mut league = ofm_core::schedule::generate_league(
        &league_name,
        preseason_league_year(&game.clock),
        &team_ids,
        season_start,
    );
    let friendlies = ofm_core::schedule::generate_preseason_friendlies(&team_ids, season_start, 4);
    ofm_core::schedule::append_fixtures(&mut league, friendlies);
    game.league = Some(league);
    ofm_core::season_context::refresh_game_context(game);

    let date_str = game.clock.current_date.to_rfc3339();
    let welcome_msg = ofm_core::messages::welcome_message(&team_name, team_id, &date_str);
    game.messages.push(welcome_msg);

    let season_msg = ofm_core::messages::season_schedule_message(
        &league_name,
        &season_start.format(&long_date_format()).to_string(),
        &date_str,
    );
    game.messages.push(season_msg);

    let team_names: Vec<String> = game.teams.iter().map(|team| team.name.clone()).collect();
    game.news.push(ofm_core::news::season_preview_article(
        &team_names,
        &date_str,
    ));

    let staff_msg = ofm_core::messages::staff_advice_message(&team_name, team_id, &date_str);
    game.messages.push(staff_msg);

    ofm_core::player_events::generate_takeover_contract_review_message(game);

    Ok(StatsState::default())
}

fn competitive_fixture_count_for_team(game: &Game, team_id: &str) -> usize {
    game.league
        .as_ref()
        .map(|league| {
            league
                .fixtures
                .iter()
                .filter(|fixture| {
                    fixture.counts_for_league_standings()
                        && (fixture.home_team_id == team_id || fixture.away_team_id == team_id)
                })
                .count()
        })
        .unwrap_or_default()
}

fn completed_competitive_fixture_count_for_team(game: &Game, team_id: &str) -> usize {
    game.league
        .as_ref()
        .map(|league| {
            league
                .fixtures
                .iter()
                .filter(|fixture| {
                    fixture.counts_for_league_standings()
                        && fixture.status == domain::league::FixtureStatus::Completed
                        && (fixture.home_team_id == team_id || fixture.away_team_id == team_id)
                })
                .count()
        })
        .unwrap_or_default()
}

fn bootstrap_midseason_takeover(game: &mut Game, team_id: &str) -> Result<StatsState, String> {
    let team = game
        .teams
        .iter()
        .find(|t| t.id == team_id)
        .ok_or("be.error.teamNotFound".to_string())?;
    let team_name = team.name.clone();

    ofm_core::ai_hiring::seed_ai_managers(game);

    let season_start = preseason_season_start(&game.clock);
    let league_name = default_league_name();
    let team_ids: Vec<String> = game.teams.iter().map(|t| t.id.clone()).collect();
    game.league = Some(ofm_core::schedule::generate_league(
        &league_name,
        preseason_league_year(&game.clock),
        &team_ids,
        season_start,
    ));
    game.clock.current_date = season_start;
    ofm_core::season_context::refresh_game_context(game);

    let total_fixtures = competitive_fixture_count_for_team(game, team_id);
    let target_completed = (total_fixtures / 2).max(1);
    let mut stats_state = StatsState::default();
    let mut safeguard_days = 0usize;
    while completed_competitive_fixture_count_for_team(game, team_id) < target_completed {
        let mut captures = Vec::new();
        ofm_core::turn::process_day_with_capture(game, &mut |capture| captures.push(capture));
        for capture in captures {
            stats_state.append(capture);
        }
        safeguard_days += 1;
        if safeguard_days > 240 {
            break;
        }
    }

    let takeover_date = game.clock.current_date.format("%Y-%m-%d").to_string();
    let _ = ofm_core::firing::fire_ai_manager_for_team(game, team_id, &takeover_date);
    ofm_core::job_offers::hire_manager(game, team_id, &takeover_date)?;

    let staff_msg = ofm_core::messages::staff_advice_message(&team_name, team_id, &takeover_date);
    game.messages.push(staff_msg);
    ofm_core::player_events::generate_takeover_contract_review_message(game);
    ofm_core::season_context::refresh_game_context(game);

    Ok(stats_state)
}

fn bootstrap_team_selection(
    game: &mut Game,
    team_id: &str,
    start_phase: StartPhase,
    stats_state: StatsState,
) -> Result<StatsState, String> {
    if has_existing_world_context(game, &stats_state) {
        return bootstrap_existing_world_takeover(game, team_id, stats_state);
    }

    match start_phase {
        StartPhase::SeasonStart => bootstrap_season_start(game, team_id),
        StartPhase::MidSeason => bootstrap_midseason_takeover(game, team_id),
    }
}

/// Step 1: create manager + generate world. No team assigned yet.
pub fn start_new_game(
    state: &StateManager,
    first_name: String,
    last_name: String,
    dob: String,
    nationality: String,
    startup_options: Option<RawStartupOptions>,
    world_source: Option<String>,
) -> Result<Game, String> {
    let first_name = first_name.trim().to_string();
    let last_name = last_name.trim().to_string();
    if first_name.is_empty() || last_name.is_empty() {
        return Err("be.error.createManager.nameRequired".to_string());
    }
    if first_name.len() > 30 || last_name.len() > 30 {
        return Err("be.error.createManager.nameMaxLength".to_string());
    }
    let nationality = nationality.trim().to_string();
    if nationality.is_empty() {
        return Err("be.error.createManager.nationalityRequired".to_string());
    }

    let birth_date = chrono::NaiveDate::parse_from_str(&dob, "%Y-%m-%d")
        .map_err(|_| "be.error.createManager.invalidDobFormat".to_string())?;

    let startup_options = normalize_startup_options(startup_options)?;
    let world = load_world_data(world_source.as_deref())?;
    let clock = game_clock_for_world(&startup_options, &world.metadata)?;
    let reference_date = clock.current_date.date_naive();
    let age = age_on_date(birth_date, reference_date);
    if age < 30 {
        return Err("be.error.createManager.minAge".to_string());
    }
    if age > 99 {
        return Err("be.error.createManager.invalidDob".to_string());
    }

    let manager = Manager::new(
        "mgr_user".to_string(),
        first_name,
        last_name,
        dob,
        nationality,
    );
    info!(
        "[cmd] start_new_game: {} {} (start_year={}, phase={}, world_source={:?})",
        manager.first_name,
        manager.last_name,
        startup_options.start_year,
        startup_options.start_phase.as_str(),
        world_source
    );

    let (new_game, stats_state) =
        build_game_from_world_data(clock, manager, &startup_options, world);

    state.set_game(new_game.clone());
    state.set_stats_state(stats_state);
    Ok(new_game)
}

/// Step 2: user picks a team. Assigns manager, generates messages, saves to DB.
pub fn select_team(app: &AppState, team_id: String) -> Result<Game, String> {
    info!("[cmd] select_team: team_id={}", team_id);
    let state = app.state();
    let mut game = state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;
    let current_stats_state = state
        .get_stats_state(|stats| stats.clone())
        .unwrap_or_default();

    let start_phase = start_phase_for_game(&game);
    let stats_state =
        bootstrap_team_selection(&mut game, &team_id, start_phase, current_stats_state)?;

    let manager_name = format!("{} {}", game.manager.first_name, game.manager.last_name);
    let save_name = default_save_name(&manager_name);

    let mut sm = app
        .saves()
        .lock()
        .map_err(|_| SAVE_MANAGER_UNAVAILABLE_ERROR.to_string())?;
    let save_id = create_new_save(&mut sm, &game, &stats_state, &save_name)?;
    drop(sm);
    state.set_save_id(save_id);

    state.set_game(game.clone());
    state.set_stats_state(stats_state);
    Ok(game)
}

pub fn get_saves(app: &AppState) -> Result<Vec<SaveEntry>, String> {
    let mut sm = app
        .saves()
        .lock()
        .map_err(|_| SAVE_MANAGER_UNAVAILABLE_ERROR.to_string())?;
    sm.load_saves()
}

pub fn delete_save(app: &AppState, save_id: String) -> Result<bool, String> {
    info!("[cmd] delete_save: save_id={}", save_id);
    let mut sm = app
        .saves()
        .lock()
        .map_err(|_| SAVE_MANAGER_UNAVAILABLE_ERROR.to_string())?;
    sm.delete_save(&save_id)
}

pub fn load_game(app: &AppState, save_id: String) -> Result<String, String> {
    info!("[cmd] load_game: save_id={}", save_id);
    let state = app.state();
    let mut sm = app
        .saves()
        .lock()
        .map_err(|_| SAVE_MANAGER_UNAVAILABLE_ERROR.to_string())?;
    let mut game = sm.load_game(&save_id)?;
    let stats_state = sm.load_stats_state(&save_id)?;
    drop(sm);
    ofm_core::ai_hiring::seed_ai_managers(&mut game);
    ofm_core::season_context::refresh_game_context(&mut game);

    let mgr_name = format!("{} {}", game.manager.first_name, game.manager.last_name);

    state.set_save_id(save_id);
    state.set_game(game);
    state.set_stats_state(stats_state);
    Ok(mgr_name)
}

pub fn get_active_game(state: &StateManager) -> Result<Game, String> {
    state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())
}

pub fn save_game(app: &AppState) -> Result<(), String> {
    info!("[cmd] save_game");
    let state = app.state();
    let game = state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;

    let save_id = state
        .get_save_id()
        .ok_or("be.error.noActiveSaveSession".to_string())?;

    let stats_state = require_active_stats_state(state)?;
    let mut sm = app
        .saves()
        .lock()
        .map_err(|_| SAVE_MANAGER_UNAVAILABLE_ERROR.to_string())?;
    sm.save_game_with_stats(&game, &stats_state, &save_id)
}

/// Save the current game and clear the active session (return to main menu).
pub fn exit_to_menu(app: &AppState) -> Result<(), String> {
    info!("[cmd] exit_to_menu");
    let state = app.state();
    let game = state
        .get_game(|g: &Game| g.clone())
        .ok_or("be.error.noActiveGameSession")?;

    if let Some(save_id) = state.get_save_id() {
        let stats_state = require_active_stats_state(state)?;
        let mut sm = app
            .saves()
            .lock()
            .map_err(|_| SAVE_MANAGER_UNAVAILABLE_ERROR.to_string())?;
        sm.save_game_with_stats(&game, &stats_state, &save_id)?;
    }

    state.clear_game();
    state.clear_save_id();

    Ok(())
}
