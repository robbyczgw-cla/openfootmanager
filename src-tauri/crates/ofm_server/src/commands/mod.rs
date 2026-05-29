//! HTTP-side command adapter.
//!
//! Each function here mirrors a Tauri `#[command]` from `src-tauri/src/commands`,
//! but takes plain `&StateManager` / `&Mutex<SaveManager>` instead of Tauri's
//! `State<..>` wrappers. The heavy game logic itself is *not* duplicated — it
//! lives in `ofm_core` / `db` and is called directly, exactly as the desktop
//! commands do.
//!
//! ## Adding a command when syncing upstream
//! 1. Find the new `#[command]` in `src-tauri/src/commands/<area>.rs`.
//! 2. Add a sibling handler here that performs the same orchestration.
//! 3. Register it in [`dispatch`] below.
//! Commands not yet mirrored fall through to `AppError::NotImplemented`, which
//! the frontend surfaces as a clear "not available in the web build" message.

mod finances;
mod game;
mod jobs;
mod profiles;
mod round_summary;
mod settings;
mod time;
mod world;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::state::{AppError, AppState};

/// Route a command name + JSON args to its handler, returning the JSON result.
pub fn dispatch(app: &AppState, command: &str, args: Value) -> Result<Value, AppError> {
    match command {
        // ----- world / databases -----
        "list_world_databases" => ok(world::list_world_databases(app)),
        "write_temp_database" => ok(world::write_temp_database(app, arg(&args, "json")?)),

        // ----- new game / saves lifecycle -----
        "start_new_game" => ok(game::start_new_game(
            app.state(),
            arg(&args, "firstName")?,
            arg(&args, "lastName")?,
            arg(&args, "dob")?,
            arg(&args, "nationality")?,
            arg_opt(&args, "startupOptions")?,
            arg_opt(&args, "worldSource")?,
        )),
        "select_team" => ok(game::select_team(app, arg(&args, "teamId")?)),
        "get_saves" => ok(game::get_saves(app)),
        "load_game" => ok(game::load_game(app, arg(&args, "saveId")?)),
        "get_active_game" => ok(game::get_active_game(app.state())),
        "save_game" => ok(game::save_game(app)),
        "delete_save" => ok(game::delete_save(app, arg(&args, "saveId")?)),
        "exit_to_menu" => ok(game::exit_to_menu(app)),

        // ----- time advancement -----
        "advance_time" => ok(time::advance_time(app.state())),
        "advance_time_with_mode" => {
            ok(time::advance_time_with_mode(app.state(), arg(&args, "mode")?))
        }
        "skip_to_match_day" => ok(time::skip_to_match_day(app.state())),
        "check_blocking_actions" => ok(time::check_blocking_actions(app.state())),

        // ----- settings -----
        "get_settings" => ok(settings::get_settings(app)),
        "save_settings" => ok(settings::save_settings(app, arg(&args, "settings")?)),
        "clear_all_saves" => ok(settings::clear_all_saves(app)),

        // ----- jobs -----
        "get_available_jobs" => ok(jobs::get_available_jobs(app.state())),
        "apply_for_job" => ok(jobs::apply_for_job(app.state(), arg(&args, "teamId")?)),

        // ----- finances -----
        "get_finance_snapshot" => ok(finances::get_finance_snapshot(
            app.state(),
            arg_opt(&args, "teamId")?,
        )),
        "request_board_support" => ok(finances::request_board_support(app.state())),
        "request_sponsor_pitch" => ok(finances::request_sponsor_pitch(app.state())),
        "request_marketing_campaign" => ok(finances::request_marketing_campaign(app.state())),

        // ----- manager profiles -----
        "get_manager_profiles" => ok(profiles::get_manager_profiles(app)),
        "touch_manager_profile" => ok(profiles::touch_manager_profile(app, arg(&args, "id")?)),
        "save_manager_profile" => ok(profiles::save_manager_profile(
            app,
            arg(&args, "firstName")?,
            arg(&args, "lastName")?,
            arg(&args, "dob")?,
            arg(&args, "nationality")?,
            arg_opt(&args, "force")?,
        )),
        "update_manager_profile" => ok(profiles::update_manager_profile(
            app,
            arg(&args, "id")?,
            arg(&args, "firstName")?,
            arg(&args, "lastName")?,
            arg(&args, "dob")?,
            arg(&args, "nationality")?,
        )),
        "delete_manager_profile" => ok(profiles::delete_manager_profile(app, arg(&args, "id")?)),

        _ => Err(AppError::not_implemented(command)),
    }
}

/// Serialize a handler's `Result<T, String>` into a JSON `Result<Value, AppError>`.
fn ok<T: Serialize>(result: Result<T, String>) -> Result<Value, AppError> {
    let value = result.map_err(AppError::backend)?;
    serde_json::to_value(value).map_err(|e| AppError::backend(e.to_string()))
}

/// Extract a required argument by (camelCase) key, matching Tauri's argument
/// naming convention.
fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, AppError> {
    let raw = args
        .get(key)
        .ok_or_else(|| AppError::backend(format!("be.error.web.missingArgument:{key}")))?;
    serde_json::from_value(raw.clone())
        .map_err(|_| AppError::backend(format!("be.error.web.invalidArgument:{key}")))
}

/// Extract an optional argument; absent or `null` yields `None`.
fn arg_opt<T: DeserializeOwned>(args: &Value, key: &str) -> Result<Option<T>, AppError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(raw) => serde_json::from_value(raw.clone())
            .map(Some)
            .map_err(|_| AppError::backend(format!("be.error.web.invalidArgument:{key}"))),
    }
}
