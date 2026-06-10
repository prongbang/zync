use crate::{websocket::WorkspaceEvent, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Arc};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/workspace/:id/files", post(create_file))
        .route("/workspace/:id/files/rename", put(rename_file))
        .route("/workspace/:id/files/search", get(search_files))
        .route(
            "/workspace/:id/files/*path",
            get(read_file).put(write_file).delete(delete_file),
        )
}

#[derive(Debug, Deserialize)]
struct CreateFileRequest {
    path: String,
    content: Option<String>,
    is_dir: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct WriteFileRequest {
    content: String,
}

#[derive(Debug, Deserialize)]
struct RenameFileRequest {
    old_path: String,
    new_path: String,
}

#[derive(Debug, Serialize)]
struct FileContent {
    path: String,
    content: String,
}

async fn create_file(
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateFileRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = workspace_root(&state, &workspace_id)?;
    let target = safe_join(&root, &request.path)?;
    if request.is_dir.unwrap_or(false) {
        fs::create_dir_all(&target).map_err(io_error)?;
        broadcast_path(&state, &workspace_id, "folder_created", request.path);
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        fs::write(&target, request.content.unwrap_or_default()).map_err(io_error)?;
        broadcast_path(&state, &workspace_id, "file_created", request.path);
    }
    Ok(StatusCode::CREATED)
}

async fn read_file(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, path)): Path<(String, String)>,
) -> Result<Json<FileContent>, (StatusCode, String)> {
    let root = workspace_root(&state, &workspace_id)?;
    let target = safe_join(&root, &path)?;
    let content = fs::read_to_string(target).map_err(io_error)?;
    Ok(Json(FileContent { path, content }))
}

async fn write_file(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, path)): Path<(String, String)>,
    Json(request): Json<WriteFileRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = workspace_root(&state, &workspace_id)?;
    let target = safe_join(&root, &path)?;
    fs::write(target, request.content).map_err(io_error)?;
    broadcast_path(&state, &workspace_id, "file_changed", path);
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_file(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, path)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = workspace_root(&state, &workspace_id)?;
    let target = safe_join(&root, &path)?;
    if target.is_dir() {
        fs::remove_dir_all(&target).map_err(io_error)?;
        broadcast_path(&state, &workspace_id, "folder_deleted", path);
    } else {
        fs::remove_file(&target).map_err(io_error)?;
        broadcast_path(&state, &workspace_id, "file_deleted", path);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn rename_file(
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
    Json(request): Json<RenameFileRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = workspace_root(&state, &workspace_id)?;
    let old_path = safe_join(&root, &request.old_path)?;
    let new_path = safe_join(&root, &request.new_path)?;
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    fs::rename(old_path, new_path).map_err(io_error)?;
    let mut event = WorkspaceEvent::new("file_renamed");
    event.path = Some(request.old_path);
    event.payload = serde_json::json!({ "new_path": request.new_path });
    state.hub.broadcast(&workspace_id, event);
    Ok(StatusCode::NO_CONTENT)
}

async fn search_files(
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<crate::sync::FileNode>>, (StatusCode, String)> {
    let root = workspace_root(&state, &workspace_id)?;
    let needle = query.get("q").cloned().unwrap_or_default().to_lowercase();
    let files = crate::sync::list_workspace_files(root)
        .map_err(internal_error)?
        .into_iter()
        .filter(|file| file.path.to_lowercase().contains(&needle))
        .collect();
    Ok(Json(files))
}

fn workspace_root(state: &AppState, workspace_id: &str) -> Result<PathBuf, (StatusCode, String)> {
    let workspace = state
        .db
        .workspace(workspace_id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "workspace not found".to_string()))?;
    let repository = state
        .db
        .repository(&workspace.repository_id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "repository not found".to_string()))?;
    Ok(PathBuf::from(repository.path))
}

fn safe_join(root: &std::path::Path, path: &str) -> Result<PathBuf, (StatusCode, String)> {
    if path.contains("..") || path.starts_with('/') {
        return Err((StatusCode::BAD_REQUEST, "unsafe path".to_string()));
    }
    Ok(root.join(path))
}

fn broadcast_path(state: &AppState, workspace_id: &str, kind: &str, path: String) {
    let mut event = WorkspaceEvent::new(kind);
    event.path = Some(path);
    state.hub.broadcast(workspace_id, event);
}

fn io_error(error: std::io::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
