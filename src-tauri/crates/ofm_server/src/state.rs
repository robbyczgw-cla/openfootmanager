//! Shared server state and the error type used by command handlers.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use db::save_manager::SaveManager;
use ofm_core::state::StateManager;

/// Cloneable handle to the process-wide game state.
///
/// `StateManager` already guards its fields with interior `Mutex`es (it is the
/// same type the desktop app `manage`s), so it can be shared behind an `Arc`.
/// `SaveManager` needs `&mut`, so it gets its own `Mutex`.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    state: StateManager,
    saves: Mutex<SaveManager>,
    data_dir: PathBuf,
}

impl AppState {
    pub fn init(data_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
        let saves_dir = data_dir.join("saves");
        let save_manager = SaveManager::init(&saves_dir).map_err(|e| e.to_string())?;

        Ok(Self {
            inner: Arc::new(Inner {
                state: StateManager::new(),
                saves: Mutex::new(save_manager),
                data_dir,
            }),
        })
    }

    pub fn state(&self) -> &StateManager {
        &self.inner.state
    }

    pub fn saves(&self) -> &Mutex<SaveManager> {
        &self.inner.saves
    }

    pub fn data_dir(&self) -> &Path {
        &self.inner.data_dir
    }
}

/// Error returned from a command handler.
pub enum AppError {
    /// A backend/domain error keyed for i18n (e.g. `be.error.noActiveGameSession`).
    Backend(String),
    /// The command exists in the desktop build but is not yet mirrored for web.
    NotImplemented(String),
}

impl AppError {
    pub fn backend(message: impl Into<String>) -> Self {
        AppError::Backend(message.into())
    }

    pub fn not_implemented(command: impl Into<String>) -> Self {
        AppError::NotImplemented(command.into())
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        AppError::Backend(message)
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        AppError::Backend(message.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Backend(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            AppError::NotImplemented(command) => {
                log::warn!("command '{command}' is not implemented in the web build yet");
                (
                    StatusCode::NOT_IMPLEMENTED,
                    Json(json!({
                        "error": "be.error.web.commandNotImplemented",
                        "command": command,
                    })),
                )
                    .into_response()
            }
        }
    }
}
