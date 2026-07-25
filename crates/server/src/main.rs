mod auth;
mod collaboration;
mod credentials;
mod crypto;
mod db;
mod files;
mod git;
mod repos_root;
mod repository;
mod sync;
mod websocket;
mod workspace;

use axum::{routing::get, Router};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub db: db::Database,
    pub hub: websocket::WorkspaceHub,
    pub sync: sync::WorkspaceSync,
    pub collaboration: collaboration::CollaborationState,
    pub secrets: crypto::KeyState,
    pub auth: auth::AuthState,
    /// `ZYNC_REPOS_ROOT` filesystem boundary (P4.1). Empty/unconfigured
    /// preserves today's unbounded behavior for existing single-user
    /// deploys; see `repos_root` module docs.
    pub repos_root: repos_root::ReposRoot,
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
        secrets: crypto::KeyState::load(),
        // Validates ZYNC_AUTH at boot — an unknown value refuses to start.
        auth: auth::AuthState::load()?,
        // Validates ZYNC_REPOS_ROOT at boot — a configured-but-unresolvable
        // root refuses to start (P4.1), same posture as ZYNC_AUTH above.
        repos_root: repos_root::ReposRoot::load()?,
    });

    // P4.1 rollout note: ZYNC_REPOS_ROOT is not required to boot (existing
    // single-user LAN deploys keep working unbounded), but leaving it unset
    // once auth is enabled means any authenticated user can register/clone/
    // init an arbitrary host path. Warn loudly rather than silently allowing it.
    if state.auth.mode == auth::AuthMode::Enabled && !state.repos_root.is_configured() {
        tracing::warn!(
            "multi-user without ZYNC_REPOS_ROOT lets any user mount arbitrary host paths — \
             set ZYNC_REPOS_ROOT"
        );
    }

    // First-boot admin bootstrap (env or one-time /setup link) and the periodic
    // expired-session sweep (ADR-002).
    auth::bootstrap(&state).await?;
    auth::spawn_session_sweeper(state.clone());

    // Serve the built React app (Vite emits index.html + /assets/*). Unmatched
    // routes fall back to index.html with a 200 so client-side navigation and
    // hard refreshes work (a plain not_found_service would preserve the 404
    // status even while serving the index body).
    let static_root = static_dir();
    let index_path = std::path::Path::new(&static_root).join("index.html");
    let spa = ServeDir::new(&static_root)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index_path));

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(auth::routes())
        .merge(repository::routes())
        .merge(workspace::routes())
        .merge(files::routes())
        .merge(git::routes())
        .merge(websocket::routes())
        .merge(collaboration::routes())
        .merge(credentials::routes())
        // Repo-scope authorization (ADR-002 Decision 5). This is the INNER of
        // the two auth layers: because it's added before `require_auth` below,
        // it wraps closer to the handlers and therefore runs *after*
        // authentication has populated the request's `AuthUser`. It enforces
        // per-repository roles (viewer/member/owner) on `/repositories/:id/*`
        // and `/workspace/:id/*`; non-repo-scoped routes pass through untouched.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::authz::require_repo_authz,
        ))
        // One auth layer over every merged API route (ADR-002 Decision 4).
        // Applied BEFORE `fallback_service` so the SPA fallback stays *outside*
        // the layer: unmatched paths (client-side routes, static assets) are
        // served by the unwrapped fallback and remain public, while every API
        // route is closed-by-default — a newly-added route above is
        // authenticated automatically, and `require_auth`'s allowlist is the
        // only opt-out (no path-prefix guessing). This is the OUTER auth layer
        // (added last → runs first), so authentication happens before the
        // authorization layer above. Added before CORS so CORS remains outermost
        // (preflight) and auth runs just before the handlers.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
        .fallback_service(spa)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(state);

    let addr: SocketAddr = std::env::var("ZYNC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:58271".to_string())
        .parse()?;
    tracing::info!("zync server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn static_dir() -> String {
    std::env::var("ZYNC_STATIC_DIR").unwrap_or_else(|_| "/app/public".to_string())
}

