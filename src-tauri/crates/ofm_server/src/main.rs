//! OpenFootManager web backend.
//!
//! This binary is the browser-version counterpart to the Tauri desktop shell.
//! The desktop app exposes the game core (`ofm_core` + `db`) to the React
//! frontend through Tauri `#[command]` functions; this server exposes the
//! *same* core through a single HTTP endpoint, so the unchanged frontend can
//! talk to it via `fetch` instead of Tauri's `invoke`.
//!
//! Architecture (see `docs/WEB.md`):
//!   * The pure game logic lives in the workspace crates (`ofm_core`, `db`,
//!     `engine`, `domain`) and is reused verbatim — never forked.
//!   * `src/commands/*` here is the "HTTP adapter". It mirrors the thin
//!     orchestration that `src-tauri/src/commands/*` performs for Tauri.
//!     When upstream adds or changes a command, mirror it here (additive work).
//!   * `POST /api/invoke/{command}` dispatches `{command, jsonArgs}` exactly
//!     like Tauri's `invoke(command, args)` would.

mod commands;
mod state;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use state::AppState;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_dir = data_dir();
    let dist_dir = dist_dir();
    let addr = bind_addr();

    let app_state = match AppState::init(data_dir.clone()) {
        Ok(state) => state,
        Err(err) => {
            log::error!("failed to initialise save manager: {err}");
            std::process::exit(1);
        }
    };

    let api = Router::new()
        .route("/api/invoke/{command}", post(invoke_handler))
        .route("/api/health", axum::routing::get(|| async { "ok" }))
        .with_state(app_state)
        .layer(CorsLayer::permissive());

    // Serve the built SPA: real files are streamed from `dist`; any other path
    // falls back to index.html with a 200 so client-side routing (react-router)
    // works on hard refreshes / deep links.
    let index_html = std::fs::read_to_string(dist_dir.join("index.html"))
        .unwrap_or_else(|_| MISSING_DIST_HTML.to_string());
    let static_state = StaticState {
        dist: dist_dir.clone(),
        index: Arc::new(index_html),
    };
    let static_files = get(serve_static_or_spa).with_state(static_state);

    let app = api.fallback_service(static_files);

    log::info!("OpenFootManager web server");
    log::info!("  data dir : {}", data_dir.display());
    log::info!("  dist dir : {}", dist_dir.display());
    log::info!("  listening: http://{addr}");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            log::error!("failed to bind {addr}: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = axum::serve(listener, app).await {
        log::error!("server error: {err}");
        std::process::exit(1);
    }
}

/// Handles `POST /api/invoke/{command}` with a JSON body of arguments.
///
/// Mirrors Tauri's `invoke(command, args)` contract: on success the response
/// body is the command's return value (200); on failure it is
/// `{ "error": "<backend.key>" }` with a non-2xx status, which the frontend
/// shim re-throws so existing `resolveBackendError` handling keeps working.
async fn invoke_handler(
    State(app): State<AppState>,
    Path(command): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let args = body.map(|Json(value)| value).unwrap_or_else(|| json!({}));

    // Game simulation can be CPU heavy (advancing a whole season), so run the
    // synchronous dispatch off the async runtime to avoid stalling other
    // requests.
    let result =
        tokio::task::spawn_blocking(move || commands::dispatch(&app, &command, args)).await;

    match result {
        Ok(Ok(value)) => (StatusCode::OK, Json(value)).into_response(),
        Ok(Err(err)) => err.into_response(),
        Err(join_err) => {
            log::error!("dispatch task panicked: {join_err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "be.error.web.internal" })),
            )
                .into_response()
        }
    }
}

#[derive(Clone)]
struct StaticState {
    dist: PathBuf,
    index: Arc<String>,
}

/// Serve a real file from `dist` if it exists, otherwise fall back to
/// index.html (HTTP 200) so the SPA router can handle deep links.
async fn serve_static_or_spa(State(state): State<StaticState>, uri: Uri) -> Response {
    let rel = uri.path().trim_start_matches('/');

    // Reject path traversal and absolute paths; anything suspicious just gets
    // the SPA shell.
    let is_safe = !rel.is_empty()
        && !rel.contains("..")
        && !rel.starts_with('/')
        && !rel.contains('\\');

    if is_safe {
        let candidate = state.dist.join(rel);
        if candidate.is_file() {
            if let Ok(bytes) = tokio::fs::read(&candidate).await {
                let mime = mime_for(&candidate);
                return ([(header::CONTENT_TYPE, mime)], bytes).into_response();
            }
        }
    }

    Html((*state.index).clone()).into_response()
}

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("map") => "application/json",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

const MISSING_DIST_HTML: &str = "<!doctype html><title>OpenFootManager</title>\
<body style=\"font-family:sans-serif;padding:2rem\">\
<h1>Web frontend not built</h1>\
<p>Run <code>npm run build:web</code> (or set <code>OFM_WEB_DIST</code>) and restart the server.</p>\
</body>";

fn data_dir() -> PathBuf {
    std::env::var_os("OFM_WEB_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ofm-web-data"))
}

fn dist_dir() -> PathBuf {
    std::env::var_os("OFM_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("dist"))
}

fn bind_addr() -> SocketAddr {
    let host = std::env::var("OFM_WEB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("OFM_WEB_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    format!("{host}:{port}")
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], port)))
}
