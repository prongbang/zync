mod auth;
mod collaboration;
mod db;
mod files;
mod git;
mod repository;
mod sync;
mod websocket;
mod workspace;

use axum::{routing::get, Router};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub db: db::Database,
    pub hub: websocket::WorkspaceHub,
    pub sync: sync::WorkspaceSync,
    pub collaboration: collaboration::CollaborationState,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zync_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_path = std::env::var("ZYNC_DB").unwrap_or_else(|_| "zync.db".to_string());
    let state = Arc::new(AppState {
        db: db::Database::open(db_path)?,
        hub: websocket::WorkspaceHub::default(),
        sync: sync::WorkspaceSync::default(),
        collaboration: collaboration::CollaborationState::default(),
    });

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(auth::routes())
        .merge(repository::routes())
        .merge(workspace::routes())
        .merge(files::routes())
        .merge(git::routes())
        .merge(websocket::routes())
        .merge(collaboration::routes())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = std::env::var("ZYNC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:58271".to_string())
        .parse()?;
    tracing::info!("zync server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
