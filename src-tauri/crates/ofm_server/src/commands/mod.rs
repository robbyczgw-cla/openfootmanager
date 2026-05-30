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

mod club;
mod contracts;
mod finances;
mod game;
mod jobs;
mod live_match;
mod live_match_service;
mod messages;
mod profiles;
mod round_summary;
mod season;
mod settings;
mod squad;
mod staff;
mod stats;
mod team_talk;
mod time;
mod time_advancement;
mod time_blockers;
mod transfers;
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
        "export_world_database" => {
            ok(world::export_world_database(app, arg(&args, "exportPath")?))
        }

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

        // ----- club / facilities -----
        "upgrade_facility" => ok(club::upgrade_facility(app.state(), arg(&args, "facility")?)),

        // ----- squad / tactics / training -----
        "set_formation" => ok(squad::set_formation(app.state(), arg(&args, "formation")?)),
        "set_starting_xi" => {
            ok(squad::set_starting_xi(app.state(), arg(&args, "playerIds")?))
        }
        "set_play_style" => ok(squad::set_play_style(app.state(), arg(&args, "playStyle")?)),
        "set_team_match_roles" => {
            ok(squad::set_team_match_roles(app.state(), arg(&args, "matchRoles")?))
        }
        "set_training" => ok(squad::set_training(
            app.state(),
            arg(&args, "focus")?,
            arg(&args, "intensity")?,
        )),
        "set_training_schedule" => {
            ok(squad::set_training_schedule(app.state(), arg(&args, "schedule")?))
        }
        "set_training_groups" => {
            ok(squad::set_training_groups(app.state(), arg(&args, "groups")?))
        }
        "set_player_training_focus" => ok(squad::set_player_training_focus(
            app.state(),
            arg(&args, "playerId")?,
            arg_opt(&args, "focus")?,
        )),
        "set_player_squad_role" => ok(squad::set_player_squad_role(
            app.state(),
            arg(&args, "playerId")?,
            arg(&args, "squadRole")?,
        )),
        "auto_select_set_pieces" => {
            ok(squad::auto_select_set_pieces(app.state(), arg(&args, "playerIds")?))
        }

        // ----- staff -----
        "hire_staff" => ok(staff::hire_staff(app.state(), arg(&args, "staffId")?)),
        "release_staff" => ok(staff::release_staff(app.state(), arg(&args, "staffId")?)),

        // ----- messages / inbox -----
        "mark_message_read" => {
            ok(messages::mark_message_read(app.state(), arg(&args, "messageId")?))
        }
        "delete_message" => ok(messages::delete_message(app.state(), arg(&args, "messageId")?)),
        "delete_messages" => {
            ok(messages::delete_messages(app.state(), arg(&args, "messageIds")?))
        }
        "mark_all_messages_read" => ok(messages::mark_all_messages_read(app.state())),
        "clear_old_messages" => ok(messages::clear_old_messages(app.state())),
        "resolve_message_action" => ok(messages::resolve_message_action(
            app.state(),
            arg(&args, "messageId")?,
            arg(&args, "actionId")?,
            arg_opt(&args, "optionId")?,
        )),

        // ----- contracts / renewals -----
        "propose_renewal" => ok(contracts::propose_renewal(
            app.state(),
            arg(&args, "playerId")?,
            arg(&args, "weeklyWage")?,
            arg(&args, "contractYears")?,
        )),
        "delegate_renewals" => ok(contracts::delegate_renewals(
            app.state(),
            arg_opt(&args, "playerIds")?,
            arg(&args, "maxWageIncreasePct")?,
            arg(&args, "maxContractYears")?,
        )),
        "preview_renewal_financial_impact" => ok(contracts::preview_renewal_financial_impact(
            app.state(),
            arg(&args, "playerId")?,
            arg(&args, "weeklyWage")?,
        )),
        "offer_free_agent_contract" => ok(contracts::offer_free_agent_contract(
            app.state(),
            arg(&args, "playerId")?,
            arg(&args, "weeklyWage")?,
            arg(&args, "contractYears")?,
        )),
        "preview_free_agent_contract_impact" => {
            ok(contracts::preview_free_agent_contract_impact(
                app.state(),
                arg(&args, "playerId")?,
                arg(&args, "weeklyWage")?,
            ))
        }
        "set_contract_exit_intent" => ok(contracts::set_contract_exit_intent(
            app.state(),
            arg(&args, "playerId")?,
            arg_opt(&args, "reason")?,
        )),
        "clear_contract_exit_intent" => {
            ok(contracts::clear_contract_exit_intent(app.state(), arg(&args, "playerId")?))
        }
        "preview_contract_termination" => {
            ok(contracts::preview_contract_termination(app.state(), arg(&args, "playerId")?))
        }
        "terminate_contract_now" => {
            ok(contracts::terminate_contract_now(app.state(), arg(&args, "playerId")?))
        }

        // ----- transfers / scouting -----
        "toggle_transfer_list" => {
            ok(transfers::toggle_transfer_list(app.state(), arg(&args, "playerId")?))
        }
        "toggle_loan_list" => {
            ok(transfers::toggle_loan_list(app.state(), arg(&args, "playerId")?))
        }
        "make_transfer_bid" => ok(transfers::make_transfer_bid(
            app.state(),
            arg(&args, "playerId")?,
            arg(&args, "fee")?,
        )),
        "preview_transfer_bid_financial_impact" => {
            ok(transfers::preview_transfer_bid_financial_impact(
                app.state(),
                arg(&args, "playerId")?,
                arg(&args, "fee")?,
            ))
        }
        "respond_to_offer" => ok(transfers::respond_to_offer(
            app.state(),
            arg(&args, "playerId")?,
            arg(&args, "offerId")?,
            arg(&args, "accept")?,
        )),
        "counter_offer" => ok(transfers::counter_offer(
            app.state(),
            arg(&args, "playerId")?,
            arg(&args, "offerId")?,
            arg(&args, "requestedFee")?,
        )),
        "send_scout" => ok(transfers::send_scout(
            app.state(),
            arg(&args, "scoutId")?,
            arg(&args, "playerId")?,
        )),
        "start_youth_scouting" => ok(transfers::start_youth_scouting(
            app.state(),
            arg(&args, "scoutId")?,
            arg_opt(&args, "region")?,
            arg_opt(&args, "objective")?,
            arg_opt(&args, "targetPosition")?,
        )),
        "cancel_youth_scouting" => {
            ok(transfers::cancel_youth_scouting(app.state(), arg(&args, "assignmentId")?))
        }
        "reassign_youth_scouting" => ok(transfers::reassign_youth_scouting(
            app.state(),
            arg(&args, "assignmentId")?,
            arg(&args, "scoutId")?,
        )),

        // ----- season rollover -----
        "check_season_complete" => ok(season::check_season_complete(app.state())),
        "advance_to_next_season" => ok(season::advance_to_next_season(app.state())),
        "get_season_awards" => ok(season::get_season_awards(app.state())),

        // ----- stats -----
        "get_player_match_history" => ok(stats::get_player_match_history(
            app.state(),
            arg(&args, "playerId")?,
            arg_opt(&args, "limit")?,
        )),
        "get_player_stats_overview" => {
            ok(stats::get_player_stats_overview(app.state(), arg(&args, "playerId")?))
        }
        "get_team_stats_overview" => {
            ok(stats::get_team_stats_overview(app.state(), arg(&args, "teamId")?))
        }
        "get_team_match_history" => ok(stats::get_team_match_history(
            app.state(),
            arg(&args, "teamId")?,
            arg_opt(&args, "limit")?,
        )),

        // ----- live match -----
        "start_live_match" => ok(live_match::start_live_match(
            app.state(),
            arg(&args, "fixtureIndex")?,
            arg(&args, "mode")?,
            arg(&args, "allowsExtraTime")?,
        )),
        "step_live_match" => {
            ok(live_match::step_live_match(app.state(), arg(&args, "minutes")?))
        }
        "apply_match_command" => {
            ok(live_match::apply_match_command(app.state(), arg(&args, "command")?))
        }
        "get_match_snapshot" => ok(live_match::get_match_snapshot(app.state())),
        "finish_live_match" => ok(live_match::finish_live_match(app.state())),
        "apply_team_talk" => ok(live_match::apply_team_talk(
            app.state(),
            arg(&args, "tone")?,
            arg(&args, "context")?,
        )),
        "submit_press_conference" => ok(live_match::submit_press_conference(
            app.state(),
            arg(&args, "answers")?,
            arg(&args, "homeTeam")?,
            arg(&args, "awayTeam")?,
            arg(&args, "homeScore")?,
            arg(&args, "awayScore")?,
            arg(&args, "userTeamName")?,
            arg(&args, "userTeamId")?,
        )),

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
