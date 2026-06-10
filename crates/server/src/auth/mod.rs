use crate::AppState;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LogoutRequest {
    token: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    user: crate::db::User,
    token: String,
    refresh_token: String,
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let (user, session) = state
        .db
        .login(&request.email, request.name.as_deref())
        .map_err(internal_error)?;
    Ok(Json(LoginResponse {
        user,
        token: session.token,
        refresh_token: session.refresh_token,
    }))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LogoutRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.db.logout(&request.token).map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
