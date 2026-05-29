//! Mirrors `src-tauri/src/commands/world.rs`.
//!
//! Desktop builds resolve world databases from Tauri's resource/app-data dirs;
//! the web build scans `<data_dir>/databases` instead. `export_world_database`
//! is intentionally not mirrored (it relies on a native save dialog / path).

use chrono::Utc;
use log::info;

use ofm_core::generator::WorldDatabaseInfo;

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
