use crate::{websocket::WorkspaceEvent, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/repositories",
            get(list_repositories).post(create_repository),
        )
        .route("/repositories/:id", delete(remove_repository))
        .route("/repositories/:id/favorite", put(set_favorite))
        .route("/repositories/:id/open", post(open_repository))
}

#[derive(Debug, Deserialize)]
struct CreateRepositoryRequest {
    name: Option<String>,
    path: Option<String>,
    remote_url: Option<String>,
    clone_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FavoriteRequest {
    favorite: bool,
}

#[derive(Debug, Serialize)]
struct RepositoryWithWorkspace {
    repository: crate::db::RepositoryRecord,
    workspace: crate::db::WorkspaceRecord,
}

async fn list_repositories(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::db::RepositoryRecord>>, (StatusCode, String)> {
    state
        .db
        .list_repositories()
        .map(Json)
        .map_err(internal_error)
}

async fn create_repository(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateRepositoryRequest>,
) -> Result<Json<RepositoryWithWorkspace>, (StatusCode, String)> {
    let path = if let (Some(remote_url), Some(clone_to)) = (&request.remote_url, &request.clone_to)
    {
        zync_git_core::clone_repo(remote_url, clone_to).map_err(internal_error)?;
        clone_to.clone()
    } else {
        request.path.clone().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "path or clone_to is required".to_string(),
            )
        })?
    };

    let name = request.name.clone().unwrap_or_else(|| {
        PathBuf::from(&path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Repository".to_string())
    });
    let repository = state
        .db
        .create_repository(&name, &path, request.remote_url.as_deref())
        .map_err(internal_error)?;
    let workspace = state
        .db
        .workspace_for_repository(&repository.id, &repository.name)
        .map_err(internal_error)?;
    state.sync.watch(
        workspace.id.clone(),
        PathBuf::from(&repository.path),
        state.hub.clone(),
    );
    state.hub.broadcast(
        &workspace.id,
        WorkspaceEvent::repository_opened(&repository.id),
    );
    Ok(Json(RepositoryWithWorkspace {
        repository,
        workspace,
    }))
}

async fn remove_repository(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.db.remove_repository(&id).map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_favorite(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<FavoriteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .set_favorite(&id, request.favorite)
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn open_repository(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RepositoryWithWorkspace>, (StatusCode, String)> {
    let repository = state
        .db
        .repository(&id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "repository not found".to_string()))?;
    zync_git_core::open_repo(&repository.path).map_err(internal_error)?;
    let workspace = state
        .db
        .workspace_for_repository(&repository.id, &repository.name)
        .map_err(internal_error)?;
    state.sync.watch(
        workspace.id.clone(),
        PathBuf::from(&repository.path),
        state.hub.clone(),
    );
    state.hub.broadcast(
        &workspace.id,
        WorkspaceEvent::repository_opened(&repository.id),
    );
    Ok(Json(RepositoryWithWorkspace {
        repository,
        workspace,
    }))
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
