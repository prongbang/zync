use crate::{
    auth::AuthUser, credentials, git::map_git_error, repos_root, websocket::WorkspaceEvent,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

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
        // Member management (ADR-002 Decision 5). Owner/admin-only — enforced by
        // the repo-scope authz guard, which treats the whole `/members` subtree
        // as owner-only regardless of method.
        .route(
            "/repositories/:id/members",
            get(list_members).post(add_member),
        )
        .route(
            "/repositories/:id/members/:user_id",
            put(update_member).delete(remove_member),
        )
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

/// When `ZYNC_REPOS_ROOT` is configured (P4.1), the directory browser is
/// confined to it: an explicit `path` must resolve inside one of the roots
/// (403 otherwise), and no `path` lists the roots themselves instead of
/// falling back to the server's CWD/"/" — the browser never sees anything
/// above the allowlist. Unconfigured preserves today's unbounded behavior.
async fn list_directories(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DirectoryQuery>,
) -> Result<Json<DirectoryList>, (StatusCode, String)> {
    let requested_path = query.path.filter(|path| !path.trim().is_empty());

    if state.repos_root.is_configured() {
        let Some(requested_path) = requested_path else {
            let mut directories: Vec<DirectoryEntry> = state
                .repos_root
                .roots()
                .iter()
                .map(|root| DirectoryEntry {
                    name: root
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| root.to_string_lossy().to_string()),
                    path: root.to_string_lossy().to_string(),
                })
                .collect();
            directories.sort_by_key(|entry| entry.name.to_lowercase());
            return Ok(Json(DirectoryList {
                current_path: String::new(),
                parent_path: None,
                directories,
            }));
        };

        let current = state
            .repos_root
            .ensure_within(FsPath::new(&requested_path))
            .map_err(|err| (StatusCode::FORBIDDEN, err.to_string()))?;
        return list_directory_at(&current, &state.repos_root);
    }

    let requested = requested_path
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
    let current = requested
        .canonicalize()
        .map_err(anyhow::Error::from)
        .map_err(internal_error)?;
    list_directory_at(&current, &state.repos_root)
}

/// Shared directory-read + response-building logic for `list_directories`.
/// `current` is assumed already validated/canonicalized by the caller. When
/// `repos_root` is configured, the reported `parent_path` is clamped to
/// `None` once stepping up would leave every configured root, rather than
/// leaking (and letting the client navigate to) a directory above the
/// allowlist.
fn list_directory_at(
    current: &FsPath,
    repos_root: &repos_root::ReposRoot,
) -> Result<Json<DirectoryList>, (StatusCode, String)> {
    if !current.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            "path is not a directory".to_string(),
        ));
    }

    let mut directories = Vec::new();
    for entry in fs::read_dir(current)
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

    let parent_path = current.parent().and_then(|parent| {
        if repos_root.is_configured() && repos_root.ensure_within(parent).is_err() {
            None
        } else {
            Some(parent.to_string_lossy().to_string())
        }
    });

    Ok(Json(DirectoryList {
        current_path: current.to_string_lossy().to_string(),
        parent_path,
        directories,
    }))
}

/// Lists only the repositories the caller can see (ADR-002 Decision 5): those
/// they own or are a member of. A global `admin` sees all. This is the
/// list-level counterpart to the repo-scope authz guard — a non-member can't
/// even enumerate a repo they have no role on.
async fn list_repositories(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Vec<crate::db::RepositoryRecord>>, (StatusCode, String)> {
    let repositories = if auth.role == crate::auth::ADMIN_ROLE {
        state.db.list_repositories()
    } else {
        state.db.list_repositories_for_user(&auth.id)
    }
    .map_err(internal_error)?;
    Ok(Json(repositories))
}

async fn create_repository(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(request): Json<CreateRepositoryRequest>,
) -> Result<Json<RepositoryWithWorkspace>, (StatusCode, String)> {
    let path = if let (Some(remote_url), Some(clone_to)) = (&request.remote_url, &request.clone_to)
    {
        // N1 (P4.1 security review): use the resolved/validated path `enforce_repos_root`
        // computed for the boundary check as the ACTUAL clone destination — never re-derive a
        // path from the original `clone_to` string after the check has passed. Re-deriving would
        // let the check and the real write target disagree (e.g. a dangling symlink component
        // that `enforce_repos_root`/W1 rejects up front, but that a naive second lookup from the
        // raw string could still resolve differently at write time).
        let destination = enforce_repos_root(&state, clone_to)?.unwrap_or_else(|| PathBuf::from(clone_to));
        let spec = credentials::resolve_credential_spec_for_url(&state, &auth.id, remote_url)?;
        zync_git_core::clone_repo_with_credentials(remote_url, &destination, Some(&spec))
            .map_err(map_git_error)?;
        destination.to_string_lossy().to_string()
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
        // Same N1 rule as the clone branch above: `target_path` is the resolved path from the
        // boundary check itself, not re-derived from `trimmed` afterward.
        let target_path = enforce_repos_root(&state, trimmed)?.unwrap_or_else(|| PathBuf::from(trimmed));
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
        let path = request.path.clone().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "path or clone_to is required".to_string(),
            )
        })?;
        // No filesystem write happens on this branch (register-existing), but resolve through
        // the same boundary check for a consistent, canonical stored path when a root is
        // configured; unconfigured keeps today's exact behavior (the raw string, unchanged).
        enforce_repos_root(&state, &path)?
            .map(|resolved| resolved.to_string_lossy().to_string())
            .unwrap_or(path)
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
            // Registering a path that's already registered is an "open" of an
            // existing repo — the caller must already have access to it, or this
            // would let any authenticated user attach to (and mutate) a repo
            // owned by someone else. Admins bypass (they see all).
            if auth.role != crate::auth::ADMIN_ROLE
                && state
                    .db
                    .repo_role_for_user(&existing.id, &auth.id)
                    .map_err(internal_error)?
                    .is_none()
            {
                return Err((
                    StatusCode::FORBIDDEN,
                    "not authorized for this repository".to_string(),
                ));
            }
            existing
        } else {
            state
                .db
                .create_repository(&name, &path, stored_remote_url.as_deref(), &auth.id)
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

// ---- Member management (ADR-002 Decision 5) ----
//
// All four handlers are owner/admin-only; the repo-scope authz guard gates the
// `/repositories/:id/members*` subtree before any of them run, so they trust
// that the caller may manage members and focus on input validation + the
// owner-protection invariant (the repo's `owner_id` can't be demoted or
// removed here — owner transfer is a separate, later flow).

#[derive(Debug, Deserialize)]
struct AddMemberRequest {
    /// User id or email of an existing user to grant access to.
    user: String,
    role: String,
}

#[derive(Debug, Deserialize)]
struct UpdateMemberRequest {
    role: String,
}

/// The repo-scoped roles, most-privileged first. `owner` is assignable here so
/// P3.5 can add co-owners; demoting/removing the repo's *own* `owner_id` is
/// still refused below.
fn validate_repo_role(role: &str) -> Result<(), (StatusCode, String)> {
    match role {
        "owner" | "member" | "viewer" => Ok(()),
        _ => Err((
            StatusCode::BAD_REQUEST,
            "role must be 'owner', 'member', or 'viewer'".to_string(),
        )),
    }
}

async fn list_members(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::db::RepoMember>>, (StatusCode, String)> {
    require_repository(&state, &id)?;
    state
        .db
        .list_repo_members(&id)
        .map(Json)
        .map_err(internal_error)
}

async fn add_member(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<AddMemberRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = require_repository(&state, &id)?;
    validate_repo_role(&request.role)?;
    let user = state
        .db
        .find_user_by_identifier(request.user.trim())
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "user not found".to_string()))?;
    // Ensure the workspace (and thus the row the membership attaches to) exists
    // before inserting — a repo registered but never opened has no workspace yet.
    state
        .db
        .workspace_for_repository(&repository.id, &repository.name)
        .map_err(internal_error)?;
    state
        .db
        .add_repo_member(&id, &user.id, &request.role)
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_member(
    State(state): State<Arc<AppState>>,
    Path((id, user_id)): Path<(String, String)>,
    Json(request): Json<UpdateMemberRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = require_repository(&state, &id)?;
    validate_repo_role(&request.role)?;
    if repository.owner_id.as_deref() == Some(user_id.as_str()) {
        return Err((
            StatusCode::CONFLICT,
            "cannot change the role of the repository owner".to_string(),
        ));
    }
    let updated = state
        .db
        .set_repo_member_role(&id, &user_id, &request.role)
        .map_err(internal_error)?;
    if updated == 0 {
        // No membership row matched — the target isn't a member. Report that
        // rather than a misleading 204 success.
        return Err((StatusCode::NOT_FOUND, "member not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<Arc<AppState>>,
    Path((id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = require_repository(&state, &id)?;
    if repository.owner_id.as_deref() == Some(user_id.as_str()) {
        return Err((
            StatusCode::CONFLICT,
            "cannot remove the repository owner".to_string(),
        ));
    }
    state
        .db
        .remove_repo_member(&id, &user_id)
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Load a repository or 404 — shared by the member handlers so a bad `:id`
/// still yields a clean not-found rather than a silent empty result.
fn require_repository(
    state: &AppState,
    id: &str,
) -> Result<crate::db::RepositoryRecord, (StatusCode, String)> {
    state
        .db
        .repository(id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "repository not found".to_string()))
}

/// Enforces the `ZYNC_REPOS_ROOT` boundary (P4.1) on a caller-supplied path
/// before it's used to register, clone into, or `git init` a repository,
/// returning the resolved/validated path the caller MUST use for the actual
/// operation (P4.1 security review N1) — never re-derive a path from the
/// original string after this check has passed, or the boundary check and
/// the real write target can disagree. `Ok(None)` means no root is
/// configured: a no-op, preserving today's unbounded behavior for existing
/// single-user deploys (the caller falls back to the original string as-is).
fn enforce_repos_root(
    state: &AppState,
    candidate: &str,
) -> Result<Option<PathBuf>, (StatusCode, String)> {
    if !state.repos_root.is_configured() {
        return Ok(None);
    }
    state
        .repos_root
        .ensure_within(FsPath::new(candidate))
        .map(Some)
        .map_err(|err| (StatusCode::FORBIDDEN, err.to_string()))
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{create_repository, list_directories, CreateRepositoryRequest, DirectoryQuery};
    use crate::db::Database;
    use crate::AppState;
    use axum::extract::{Query, State};
    use axum::http::StatusCode;
    use axum::Json;
    use std::sync::Arc;

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
            .create_repository("repo", "/tmp/repo", stored_remote_url.as_deref(), "owner")
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

    // ---- ZYNC_REPOS_ROOT enforcement (P4.1) ----

    fn test_state(repos_root: crate::repos_root::ReposRoot) -> Arc<AppState> {
        Arc::new(AppState {
            db: Database::open(":memory:").expect("open in-memory db"),
            hub: crate::websocket::WorkspaceHub::default(),
            sync: crate::sync::WorkspaceSync::default(),
            collaboration: crate::collaboration::CollaborationState::default(),
            secrets: crate::crypto::KeyState::Unconfigured,
            auth: crate::auth::AuthState::disabled_for_test(),
            repos_root,
            metrics: Arc::new(crate::observability::Metrics::default()),
        })
    }

    fn owner_auth_user() -> crate::auth::AuthUser {
        crate::auth::AuthUser {
            id: "owner".to_string(),
            role: crate::auth::ADMIN_ROLE.to_string(),
        }
    }

    #[tokio::test]
    async fn create_repository_rejects_path_outside_configured_root() {
        let allowed_dir = tempfile::tempdir().expect("allowed tempdir");
        let allowed = allowed_dir
            .path()
            .canonicalize()
            .expect("canonicalize allowed root");
        let outside_dir = tempfile::tempdir().expect("outside tempdir");
        let outside = outside_dir
            .path()
            .canonicalize()
            .expect("canonicalize outside dir");

        let state = test_state(crate::repos_root::ReposRoot::for_test(vec![allowed]));
        let request = CreateRepositoryRequest {
            name: None,
            path: Some(outside.to_string_lossy().to_string()),
            remote_url: None,
            clone_to: None,
            init: false,
        };

        let result = create_repository(State(state), owner_auth_user(), Json(request)).await;
        let (status, _) = result.expect_err("path outside the configured root must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn create_repository_accepts_path_inside_configured_root() {
        let allowed_dir = tempfile::tempdir().expect("allowed tempdir");
        let allowed = allowed_dir
            .path()
            .canonicalize()
            .expect("canonicalize allowed root");
        let repo_path = allowed.join("repo");
        std::fs::create_dir(&repo_path).expect("create repo dir");

        let state = test_state(crate::repos_root::ReposRoot::for_test(vec![allowed]));
        let request = CreateRepositoryRequest {
            name: None,
            path: Some(repo_path.to_string_lossy().to_string()),
            remote_url: None,
            clone_to: None,
            init: false,
        };

        let result = create_repository(State(state), owner_auth_user(), Json(request)).await;
        assert!(
            result.is_ok(),
            "path inside the configured root must be accepted: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn create_repository_init_rejects_path_outside_configured_root() {
        let allowed_dir = tempfile::tempdir().expect("allowed tempdir");
        let allowed = allowed_dir
            .path()
            .canonicalize()
            .expect("canonicalize allowed root");
        let outside_dir = tempfile::tempdir().expect("outside tempdir");
        let outside = outside_dir
            .path()
            .canonicalize()
            .expect("canonicalize outside dir")
            .join("new-repo");

        let state = test_state(crate::repos_root::ReposRoot::for_test(vec![allowed]));
        let request = CreateRepositoryRequest {
            name: None,
            path: Some(outside.to_string_lossy().to_string()),
            remote_url: None,
            clone_to: None,
            init: true,
        };

        let result = create_repository(State(state), owner_auth_user(), Json(request)).await;
        let (status, _) = result.expect_err("init path outside the configured root must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
        // Nothing should have been created on disk.
        assert!(!outside.exists());
    }

    /// P4.1 security review W1/N1: a dangling symlink *inside* the allowed root
    /// (e.g. shipped as ordinary tracked content in a mounted/cloned repo tree)
    /// must not let `init` write through it to wherever the link points. Before
    /// the fix, `resolve_maybe_missing` treated the unresolvable symlink
    /// component as "absent", lexically appended the rest of the path onto the
    /// canonicalized root, passed `starts_with(root)`, and `init_repo`'s
    /// recursive mkdir would then follow the real symlink outside the root.
    #[cfg(unix)]
    #[tokio::test]
    async fn create_repository_init_rejects_path_through_dangling_symlink() {
        let allowed_dir = tempfile::tempdir().expect("allowed tempdir");
        let allowed = allowed_dir
            .path()
            .canonicalize()
            .expect("canonicalize allowed root");

        let link = allowed.join("dangling-link");
        std::os::unix::fs::symlink("/tmp/zync-p41-w1-repro-does-not-exist", &link)
            .expect("create dangling symlink");
        let escape_target = link.join("evil-repo");

        let state = test_state(crate::repos_root::ReposRoot::for_test(vec![allowed]));
        let request = CreateRepositoryRequest {
            name: None,
            path: Some(escape_target.to_string_lossy().to_string()),
            remote_url: None,
            clone_to: None,
            init: true,
        };

        let result = create_repository(State(state), owner_auth_user(), Json(request)).await;
        let (status, body) =
            result.expect_err("init through a dangling symlink must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(
            body.contains("unresolvable symlink"),
            "unexpected error body: {body}"
        );
        // Nothing should have been created anywhere reachable through the link.
        assert!(!escape_target.exists());
        assert!(!std::path::Path::new("/tmp/zync-p41-w1-repro-does-not-exist").exists());
    }

    #[tokio::test]
    async fn list_directories_rejects_path_outside_configured_root() {
        let allowed_dir = tempfile::tempdir().expect("allowed tempdir");
        let allowed = allowed_dir
            .path()
            .canonicalize()
            .expect("canonicalize allowed root");
        let outside_dir = tempfile::tempdir().expect("outside tempdir");
        let outside = outside_dir
            .path()
            .canonicalize()
            .expect("canonicalize outside dir");

        let state = test_state(crate::repos_root::ReposRoot::for_test(vec![allowed]));
        let result = list_directories(
            State(state),
            Query(DirectoryQuery {
                path: Some(outside.to_string_lossy().to_string()),
            }),
        )
        .await;
        let (status, _) = result.expect_err("listing outside the configured root must be rejected");
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_directories_without_path_lists_configured_roots() {
        let allowed_dir = tempfile::tempdir().expect("allowed tempdir");
        let allowed = allowed_dir
            .path()
            .canonicalize()
            .expect("canonicalize allowed root");

        let state = test_state(crate::repos_root::ReposRoot::for_test(vec![allowed.clone()]));
        let result = list_directories(State(state), Query(DirectoryQuery { path: None }))
            .await
            .expect("listing the configured roots must succeed");

        assert_eq!(result.0.directories.len(), 1);
        assert_eq!(result.0.directories[0].path, allowed.to_string_lossy());
        assert!(result.0.parent_path.is_none());
    }

    #[tokio::test]
    async fn list_directories_accepts_path_inside_configured_root() {
        let allowed_dir = tempfile::tempdir().expect("allowed tempdir");
        let allowed = allowed_dir
            .path()
            .canonicalize()
            .expect("canonicalize allowed root");
        let sub = allowed.join("sub");
        std::fs::create_dir(&sub).expect("create sub dir");

        let state = test_state(crate::repos_root::ReposRoot::for_test(vec![allowed.clone()]));
        let result = list_directories(
            State(state),
            Query(DirectoryQuery {
                path: Some(allowed.to_string_lossy().to_string()),
            }),
        )
        .await
        .expect("listing an in-root directory must succeed");

        assert_eq!(result.0.current_path, allowed.to_string_lossy());
        // The root's parent (outside every configured root) must not leak.
        assert!(result.0.parent_path.is_none());
    }
}
