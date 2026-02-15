//! # crdt-dev-ui
//!
//! Embedded web panel for inspecting [`crdt-kit`] databases during development.
//!
//! Launches a lightweight local web server (Axum) that serves a single-page
//! dashboard for browsing namespaces, entities, events, and migration metadata.
//!
//! ## Quick Start
//!
//! ```no_run
//! #[tokio::main]
//! async fn main() {
//!     crdt_dev_ui::start("app.db", 4242).await.unwrap();
//! }
//! ```

mod api;

use std::sync::Arc;

use axum::{routing::get, Router};
use crdt_store::SqliteStore;

/// Shared application state for Axum handlers.
pub(crate) struct AppState {
    pub store: SqliteStore,
    pub db_path: String,
}

/// Start the Dev UI web server.
///
/// Opens the SQLite database at `db_path` and serves the inspection panel
/// on `http://127.0.0.1:{port}`. This function blocks until the server is
/// shut down (Ctrl-C).
pub async fn start(db_path: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteStore::open(db_path)?;
    let state = Arc::new(AppState {
        store,
        db_path: db_path.to_string(),
    });

    let app = Router::new()
        .route("/", get(api::index))
        .route("/api/status", get(api::status))
        .route("/api/namespaces/{ns}/entities", get(api::list_entities))
        .route("/api/namespaces/{ns}/entities/{key}", get(api::get_entity))
        .route(
            "/api/namespaces/{ns}/entities/{key}/events",
            get(api::get_events),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    eprintln!("  Dev UI: http://localhost:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
