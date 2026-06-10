//! Mirrors `src-tauri/src/commands/profiles.rs`.
//! Profiles are stored in `<data_dir>/profiles.json`.

use db::manager_profile::{self, ManagerProfile};

use crate::state::AppState;

const PROFILES_PATH_ERROR: &str = "be.error.profiles.pathFailed";

fn profiles_path(app: &AppState) -> Result<std::path::PathBuf, String> {
    let dir = app.data_dir();
    std::fs::create_dir_all(dir).map_err(|_| PROFILES_PATH_ERROR.to_string())?;
    Ok(dir.join("profiles.json"))
}

pub fn get_manager_profiles(app: &AppState) -> Result<Vec<ManagerProfile>, String> {
    let path = profiles_path(app)?;
    let mut index = manager_profile::load_profiles(&path)?;
    index.profiles.sort_by(|a, b| {
        let a_date = a.last_used_at.as_deref().unwrap_or(&a.created_at);
        let b_date = b.last_used_at.as_deref().unwrap_or(&b.created_at);
        b_date.cmp(a_date)
    });
    Ok(index.profiles)
}

pub fn touch_manager_profile(app: &AppState, id: String) -> Result<bool, String> {
    let path = profiles_path(app)?;
    manager_profile::touch_profile(&path, &id)
}

pub fn save_manager_profile(
    app: &AppState,
    first_name: String,
    last_name: String,
    dob: String,
    nationality: String,
    force: Option<bool>,
) -> Result<ManagerProfile, String> {
    let path = profiles_path(app)?;
    if force.unwrap_or(false) {
        manager_profile::add_profile_force(&path, first_name, last_name, dob, nationality)
    } else {
        manager_profile::add_profile(&path, first_name, last_name, dob, nationality)
    }
}

pub fn update_manager_profile(
    app: &AppState,
    id: String,
    first_name: String,
    last_name: String,
    dob: String,
    nationality: String,
) -> Result<Option<ManagerProfile>, String> {
    let path = profiles_path(app)?;
    manager_profile::update_profile(&path, &id, first_name, last_name, dob, nationality)
}

pub fn delete_manager_profile(app: &AppState, id: String) -> Result<bool, String> {
    let path = profiles_path(app)?;
    manager_profile::remove_profile(&path, &id)
}
