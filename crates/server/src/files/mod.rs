use crate::{repos_root, websocket::WorkspaceEvent, AppState};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
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
        .route("/workspace/:id/assets/*path", get(read_asset))
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

async fn read_asset(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, path)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let root = workspace_root(&state, &workspace_id)?;
    let target = safe_join(&root, &path)?;
    let bytes = fs::read(&target).map_err(io_error)?;
    let content_type = content_type_for_path(&path);
    let headers = [(header::CONTENT_TYPE, HeaderValue::from_static(content_type))];
    Ok((headers, bytes))
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

/// Joins `path` (a user-supplied, `/workspace/:id/files/*path`-style relative
/// path) onto `root` (the workspace's repository directory) and asserts the
/// *resolved* result stays inside it.
///
/// W1 (P4.4 security review): a naive lexical join plus a `path.contains("..")`
/// substring check does not account for symlinks. Git checks out symlinks as
/// real filesystem symlinks on unix, so a committed `etc-link -> /etc` inside
/// a repo would previously let `GET/PUT/DELETE .../files/etc-link/passwd`
/// follow the link straight out of the repo (and out of the P4.1
/// `ZYNC_REPOS_ROOT` boundary) — the OS resolves symlinks on every path
/// operation regardless of what the string looked like. This mirrors
/// `git_core::read_workdir_file`'s canonicalize-and-`starts_with` guard, but
/// reuses `repos_root::within_repos_root` (the same dangling-symlink-safe
/// resolution the P4.1 `ZYNC_REPOS_ROOT` boundary uses) so both call sites
/// share one hardened implementation instead of two subtly different ones.
/// The old `contains("..")` substring check is dropped now that resolved
/// containment is enforced directly — it was over-broad and rejected
/// legitimate names like `foo..bar`.
fn safe_join(root: &std::path::Path, path: &str) -> Result<PathBuf, (StatusCode, String)> {
    if path.starts_with('/') {
        return Err((StatusCode::BAD_REQUEST, "unsafe path".to_string()));
    }
    let canonical_root = root.canonicalize().map_err(io_error)?;
    let candidate = canonical_root.join(path);
    repos_root::within_repos_root(std::slice::from_ref(&canonical_root), &candidate)
        .map_err(|err| (StatusCode::FORBIDDEN, err.to_string()))
}

fn broadcast_path(state: &AppState, workspace_id: &str, kind: &str, path: String) {
    let mut event = WorkspaceEvent::new(kind);
    event.path = Some(path);
    state.hub.broadcast(workspace_id, event);
}

fn content_type_for_path(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "apng" => "image/apng",
        "avif" => "image/avif",
        "gif" => "image/gif",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

fn io_error(error: std::io::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Builds a workspace whose repository root is a fresh tempdir containing
    /// one legitimate tracked file (`hello.txt`). The `TempDir` must be kept
    /// alive by the caller for the duration of the test.
    fn setup_workspace() -> (Arc<AppState>, String, tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        fs::write(root.join("hello.txt"), "hi").expect("seed legit file");

        let db = Database::open(":memory:").expect("open in-memory db");
        let repository = db
            .create_repository("repo", &root.to_string_lossy(), None, "owner")
            .expect("create repository");
        let workspace = db
            .workspace_for_repository(&repository.id, "workspace")
            .expect("create workspace");

        let state = Arc::new(AppState {
            db,
            hub: crate::websocket::WorkspaceHub::default(),
            sync: crate::sync::WorkspaceSync::default(),
            collaboration: crate::collaboration::CollaborationState::default(),
            secrets: crate::crypto::KeyState::Unconfigured,
            auth: crate::auth::AuthState::disabled_for_test(),
            repos_root: crate::repos_root::ReposRoot::default(),
        });
        (state, workspace.id, dir, root)
    }

    #[tokio::test]
    async fn read_file_accepts_legit_in_tree_file() {
        let (state, workspace_id, _dir, _root) = setup_workspace();

        let result = read_file(
            State(state),
            Path((workspace_id, "hello.txt".to_string())),
        )
        .await
        .expect("legit in-tree file must be readable");
        assert_eq!(result.0.content, "hi");
    }

    /// W1 (P4.4 security review): a committed symlink pointing *outside* the
    /// repository (e.g. shipped as ordinary tracked content — git checks out
    /// symlinks as real filesystem symlinks on unix) must not let a viewer
    /// read through it. Before the fix, `safe_join` only checked the request
    /// string for `..`/a leading `/` and never re-resolved the joined path,
    /// so the OS silently followed the symlink out of the repo (and out of
    /// the `ZYNC_REPOS_ROOT` boundary) on every read/write/delete.
    #[cfg(unix)]
    #[tokio::test]
    async fn read_file_rejects_symlink_escape_to_existing_target() {
        let (state, workspace_id, _dir, root) = setup_workspace();
        std::os::unix::fs::symlink("/etc", root.join("etc-link")).expect("create symlink");

        let result = read_file(
            State(state),
            Path((workspace_id, "etc-link/passwd".to_string())),
        )
        .await;
        let (status, _) = result.expect_err("read through an escaping symlink must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn read_file_rejects_dangling_symlink() {
        let (state, workspace_id, _dir, root) = setup_workspace();
        std::os::unix::fs::symlink("/tmp/zync-w1-does-not-exist", root.join("dangling-link"))
            .expect("create dangling symlink");

        let result = read_file(
            State(state),
            Path((workspace_id, "dangling-link/secret".to_string())),
        )
        .await;
        let (status, body) =
            result.expect_err("read through a dangling symlink must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            body.contains("unresolvable symlink"),
            "unexpected error body: {body}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn write_file_rejects_symlink_escape() {
        let (state, workspace_id, _dir, root) = setup_workspace();
        std::os::unix::fs::symlink("/etc", root.join("etc-link")).expect("create symlink");

        let result = write_file(
            State(state),
            Path((workspace_id, "etc-link/zync-w1-should-not-write".to_string())),
            Json(WriteFileRequest {
                content: "pwned".to_string(),
            }),
        )
        .await;
        let (status, _) = result.expect_err("write through an escaping symlink must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            !std::path::Path::new("/etc/zync-w1-should-not-write").exists(),
            "write must not have escaped the repository root"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn delete_file_rejects_symlink_escape() {
        let (state, workspace_id, _dir, root) = setup_workspace();
        std::os::unix::fs::symlink("/etc", root.join("etc-link")).expect("create symlink");

        let result = delete_file(
            State(state),
            Path((workspace_id, "etc-link/passwd".to_string())),
        )
        .await;
        let (status, _) = result.expect_err("delete through an escaping symlink must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            std::path::Path::new("/etc/passwd").exists(),
            "sanity: /etc/passwd must still exist"
        );
    }
}
