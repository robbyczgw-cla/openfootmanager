//! Mirrors `src-tauri/src/commands/world.rs`.
//!
//! Desktop builds resolve world databases from Tauri's resource/app-data dirs;
//! the web build scans `<data_dir>/databases` instead. `export_world_database`
//! is intentionally not mirrored (it relies on a native save dialog / path).

use chrono::{Datelike, Utc};
use log::info;

use ofm_core::generator::WorldDatabaseInfo;
use ofm_core::state::StateManager;

use crate::state::AppState;

const RANDOM_WORLD_NAME_KEY: &str = "be.msg.world.randomName";
const RANDOM_WORLD_DESCRIPTION_KEY: &str = "be.msg.world.randomDescription";
const TEAM_COUNT_PARAM: &str = "teamCount";

fn backend_text_with_param(key: &str, param_name: &str, param_value: impl ToString) -> String {
    format!("{key}?{param_name}={}", param_value.to_string())
}

pub fn list_world_databases(app: &AppState) -> Result<Vec<WorldDatabaseInfo>, String> {
    info!("[cmd] list_world_databases");

    let mut databases = vec![WorldDatabaseInfo {
        id: "random".to_string(),
        name: RANDOM_WORLD_NAME_KEY.to_string(),
        description: backend_text_with_param(RANDOM_WORLD_DESCRIPTION_KEY, TEAM_COUNT_PARAM, 16),
        team_count: 16,
        player_count: 352,
        history_mode: "generated".to_string(),
        base_year: None,
        snapshot_date: None,
        source: "builtin".to_string(),
        path: String::new(),
    }];

    let user_dir = app.data_dir().join("databases");
    databases.extend(ofm_core::generator::scan_world_databases(&user_dir));

    Ok(databases)
}

/// Mirrors `export_world_database`, but writes under `<data_dir>/exports`
/// (the browser cannot pick an arbitrary server path). The provided
/// `export_path` is reduced to its file name and sandboxed into that dir.
pub fn export_world_database(app: &AppState, export_path: String) -> Result<String, String> {
    info!("[cmd] export_world_database: path={}", export_path);
    let file_name = std::path::Path::new(&export_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("exported_world.json");

    let export_dir = app.data_dir().join("exports");
    std::fs::create_dir_all(&export_dir)
        .map_err(|_| "be.error.worldWriteFileFailed".to_string())?;
    let target = export_dir.join(file_name);

    export_world_database_internal(app.state(), &target)
}

fn export_world_database_internal(
    state: &StateManager,
    export_path: &std::path::Path,
) -> Result<String, String> {
    const EXPORTED_WORLD_NAME_KEY: &str = "be.msg.world.exportedName";
    const EXPORTED_WORLD_DESCRIPTION_KEY: &str = "be.msg.world.exportedDescription";

    let game = state
        .get_game(|g| g.clone())
        .ok_or("be.error.noActiveGameSession".to_string())?;
    let stats = state.get_stats_state(|stats| stats.clone()).unwrap_or_default();
    let mut managers = game.managers.clone();
    if let Some(existing) = managers.iter_mut().find(|manager| manager.id == game.manager_id) {
        *existing = game.manager.clone();
    } else {
        managers.push(game.manager.clone());
    }

    let world = ofm_core::generator::WorldData {
        name: EXPORTED_WORLD_NAME_KEY.to_string(),
        description: backend_text_with_param(
            EXPORTED_WORLD_DESCRIPTION_KEY,
            TEAM_COUNT_PARAM,
            game.teams.len(),
        ),
        teams: game.teams.clone(),
        players: game.players.clone(),
        staff: game.staff.clone(),
        managers,
        league: game.league.clone(),
        news: game.news.clone(),
        stats,
        world_history: game.world_history.clone(),
        metadata: ofm_core::generator::WorldDataMetadata {
            kind: ofm_core::generator::WorldDataKind::HistoricalSnapshot,
            base_year: Some(game.clock.start_date.year()),
            snapshot_date: Some(game.clock.current_date.to_rfc3339()),
        },
    };

    let json = ofm_core::generator::export_world_to_json(&world)?;
    std::fs::write(export_path, json).map_err(|_| "be.error.worldWriteFileFailed".to_string())?;
    Ok(export_path.to_string_lossy().to_string())
}

pub fn write_temp_database(app: &AppState, json: String) -> Result<String, String> {
    info!("[cmd] write_temp_database: json_len={}", json.len());
    let db_dir = app.data_dir().join("databases");
    std::fs::create_dir_all(&db_dir).map_err(|_| "be.error.worldWriteDatabaseFailed".to_string())?;

    let world = ofm_core::generator::load_world_from_json(&json)?;
    let normalized_json = ofm_core::generator::export_world_to_json(&world)?;

    let filename = format!("imported_{}.json", Utc::now().format("%Y%m%d_%H%M%S"));
    let path = db_dir.join(filename);
    std::fs::write(&path, normalized_json)
        .map_err(|_| "be.error.worldWriteDatabaseFailed".to_string())?;
    Ok(path.to_string_lossy().to_string())
}
