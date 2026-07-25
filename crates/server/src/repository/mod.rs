use crate::{auth::AuthUser, credentials, git::map_git_error, websocket::WorkspaceEvent, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf, sync::Arc};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/directories", get(list_directories))
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
    /// When `true`, `path` is initialized as a brand-new repository (`git init`, no commit)
    /// instead of being opened as an existing one. Mutually exclusive with the clone mode
    /// above; ignored (treated as `false`) when `remote_url`/`clone_to` are both set.
    #[serde(default)]
    init: bool,
}

#[derive(Debug, Deserialize)]
struct FavoriteRequest {
    favorite: bool,
}

#[derive(Debug, Deserialize)]
struct DirectoryQuery {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct DirectoryEntry {
    name: String,
    path: String,
}

#[derive(Debug, Serialize)]
struct DirectoryList {
    current_path: String,
    parent_path: Option<String>,
    directories: Vec<DirectoryEntry>,
}

#[derive(Debug, Serialize)]
struct RepositoryWithWorkspace {
    repository: crate::db::RepositoryRecord,
    workspace: crate::db::WorkspaceRecord,
}

async fn list_directories(
    Query(query): Query<DirectoryQuery>,
) -> Result<Json<DirectoryList>, (StatusCode, String)> {
    let requested = query
        .path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
    let current = requested
        .canonicalize()
        .map_err(anyhow::Error::from)
        .map_err(internal_error)?;
    if !current.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            "path is not a directory".to_string(),
        ));
    }

    let mut directories = Vec::new();
    for entry in fs::read_dir(&current)
        .map_err(anyhow::Error::from)
        .map_err(internal_error)?
    {
        let entry = entry.map_err(anyhow::Error::from).map_err(internal_error)?;
        let file_type = entry
            .file_type()
            .map_err(anyhow::Error::from)
            .map_err(internal_error)?;
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        directories.push(DirectoryEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: path.to_string_lossy().to_string(),
        });
    }
    directories.sort_by_key(|entry| entry.name.to_lowercase());

    Ok(Json(DirectoryList {
        current_path: current.to_string_lossy().to_string(),
        parent_path: current
            .parent()
            .map(|path| path.to_string_lossy().to_string()),
        directories,
    }))
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
    auth: AuthUser,
    Json(request): Json<CreateRepositoryRequest>,
) -> Result<Json<RepositoryWithWorkspace>, (StatusCode, String)> {
    let path = if let (Some(remote_url), Some(clone_to)) = (&request.remote_url, &request.clone_to)
    {
        let spec = credentials::resolve_credential_spec_for_url(&state, &auth.id, remote_url)?;
        zync_git_core::clone_repo_with_credentials(remote_url, clone_to, Some(&spec))
            .map_err(map_git_error)?;
        clone_to.clone()
    } else if request.init {
        let target = request.path.clone().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "path is required to init a repository".to_string(),
            )
        })?;
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "path must not be empty".to_string(),
            ));
        }
        let target_path = PathBuf::from(trimmed);
        if target_path.is_file() {
            return Err((
                StatusCode::BAD_REQUEST,
                "path exists and is a file, not a directory".to_string(),
            ));
        }
        if target_path.join(".git").exists() {
            return Err((
                StatusCode::BAD_REQUEST,
                "path already contains a git repository".to_string(),
            ));
        }
        // TODO(P4.1): enforce ZYNC_REPOS_ROOT allowlist here
        zync_git_core::init_repo(&target_path).map_err(internal_error)?;
        // Canonicalize after a successful init (never before — the path may not exist yet) so
        // `repository_by_path` dedup and the fs watcher see the same resolved path that
        // `/directories` and every other repo-path source already use.
        target_path
            .canonicalize()
            .map_err(anyhow::Error::from)
            .map_err(internal_error)?
            .to_string_lossy()
            .to_string()
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
    // Scrub any inline `user[:pass]@` userinfo before persisting: the clone above still used
    // `request.remote_url` verbatim (so a one-shot credentialed URL works), but `RepositoryRecord`
    // is `Serialize` and listed back to every client — a token embedded in the URL must never be
    // stored or echoed (P0.11 security review, W3).
    let stored_remote_url = request
        .remote_url
        .as_deref()
        .map(zync_git_core::redact_url_userinfo);
    let repository =
        if let Some(existing) = state.db.repository_by_path(&path).map_err(internal_error)? {
            existing
        } else {
            state
                .db
                .create_repository(&name, &path, stored_remote_url.as_deref())
                .map_err(internal_error)?
        };
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

#[cfg(test)]
mod tests {
    use crate::db::Database;

    // P0.11 security review, W3: `create_repository`'s clone-on-register path used to store
    // `request.remote_url` verbatim, so a credentialed URL's inline `user:token@` userinfo
    // ended up in the `repositories` table and got echoed straight back out again (the row is
    // `Serialize` and returned/listed to every client). This exercises the exact
    // redact-then-store pipeline the handler runs — `zync_git_core::redact_url_userinfo` over
    // the URL before it ever reaches `Database::create_repository` — and asserts the stored (and
    // therefore echoed) record carries no token.
    #[test]
    fn stores_remote_url_with_userinfo_stripped() {
        let db = Database::open(":memory:").expect("open in-memory db");
        let remote_url = "https://x-access-token:SUPERSECRETTOKEN@github.com/org/repo.git";

        let stored_remote_url = Some(remote_url).map(zync_git_core::redact_url_userinfo);
        let record = db
            .create_repository("repo", "/tmp/repo", stored_remote_url.as_deref())
            .expect("create_repository");

        assert_eq!(
            record.remote_url.as_deref(),
            Some("https://github.com/org/repo.git")
        );
        let stored = record.remote_url.expect("remote_url stored");
        assert!(
            !stored.contains("SUPERSECRETTOKEN"),
            "stored/echoed remote_url must not contain the token: {stored}"
        );

        // The record round-tripped through the DB (what a list/get response would return) is
        // equally clean.
        let fetched = db
            .repository(&record.id)
            .expect("repository lookup")
            .expect("repository exists");
        assert!(!fetched
            .remote_url
            .expect("remote_url stored")
            .contains("SUPERSECRETTOKEN"));
    }
}
