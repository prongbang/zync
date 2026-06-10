use crate::{sync::list_workspace_files, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/workspace/:id", get(get_workspace))
}

#[derive(Debug, Serialize)]
struct WorkspaceResponse {
    workspace: crate::db::WorkspaceRecord,
    repository: crate::db::RepositoryRecord,
    files: Vec<crate::sync::FileNode>,
    online_users: Vec<crate::collaboration::PresenceUser>,
}

async fn get_workspace(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<WorkspaceResponse>, (StatusCode, String)> {
    let workspace = state
        .db
        .workspace(&id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "workspace not found".to_string()))?;
    let repository = state
        .db
        .repository(&workspace.repository_id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "repository not found".to_string()))?;
    let files = list_workspace_files(&repository.path).map_err(internal_error)?;
    let online_users = state.collaboration.online_users(&id);
    Ok(Json(WorkspaceResponse {
        workspace,
        repository,
        files,
        online_users,
    }))
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
