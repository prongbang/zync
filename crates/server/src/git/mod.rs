use crate::credentials;
use crate::websocket::WorkspaceEvent;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/repositories/:id/git/status", get(status))
        .route("/repositories/:id/git/add", post(add))
        .route("/repositories/:id/git/unstage", post(unstage))
        .route("/repositories/:id/git/discard", post(discard))
        .route("/repositories/:id/git/stage-patch", post(stage_patch))
        .route("/repositories/:id/git/commit", post(commit))
        .route("/repositories/:id/git/diff/workdir", get(diff_workdir))
        .route("/repositories/:id/git/diff/staged", get(diff_staged))
        .route(
            "/repositories/:id/git/diff/commit/:commit_id",
            get(diff_commit),
        )
        .route(
            "/repositories/:id/git/diff/compare/:commit_id",
            get(diff_compare_commit),
        )
        .route("/repositories/:id/git/fetch", post(fetch))
        .route("/repositories/:id/git/fetch-all", post(fetch_all))
        .route("/repositories/:id/git/pull", post(pull))
        .route("/repositories/:id/git/push", post(push))
        .route(
            "/repositories/:id/git/remotes",
            get(remotes).post(add_remote),
        )
        .route("/repositories/:id/git/remotes/delete", post(delete_remote))
        .route("/repositories/:id/git/remotes/prune", post(prune_remote))
        .route(
            "/repositories/:id/git/remotes/branch/delete",
            post(delete_remote_branch),
        )
        .route(
            "/repositories/:id/git/push/force-with-lease",
            post(push_force_with_lease),
        )
        .route(
            "/repositories/:id/git/branches",
            get(branches).post(create_branch),
        )
        .route("/repositories/:id/git/checkout", post(checkout_branch))
        .route(
            "/repositories/:id/git/checkout/revision",
            post(checkout_revision),
        )
        .route("/repositories/:id/git/branches/rename", post(rename_branch))
        .route("/repositories/:id/git/branches/merge", post(merge_branch))
        .route("/repositories/:id/git/branches/delete", post(delete_branch))
        .route(
            "/repositories/:id/git/branches/upstream",
            post(set_upstream),
        )
        .route("/repositories/:id/git/tags", get(tags).post(create_tag))
        .route("/repositories/:id/git/tags/delete", post(delete_tag))
        .route("/repositories/:id/git/revert", post(revert_commit))
        .route("/repositories/:id/git/graph", get(commit_graph))
        .route("/repositories/:id/git/stats", get(repo_stats))
        .route("/repositories/:id/git/blame", get(blame))
        .route("/repositories/:id/git/history/file", get(file_history))
        .route("/repositories/:id/git/tree", get(tree_at_revision))
        .route("/repositories/:id/git/blob", get(blob_at_revision))
        .route("/repositories/:id/git/reflog", get(reflog))
        .route("/repositories/:id/git/reset", post(reset_to_revision))
        .route("/repositories/:id/git/submodules", get(submodules))
        .route(
            "/repositories/:id/git/submodules/init",
            post(submodule_init),
        )
        .route(
            "/repositories/:id/git/submodules/update",
            post(submodule_update),
        )
        .route(
            "/repositories/:id/git/submodules/sync",
            post(submodule_sync),
        )
        .route("/repositories/:id/git/lfs", get(lfs_summary))
        .route("/repositories/:id/git/lfs/install", post(lfs_install))
        .route("/repositories/:id/git/lfs/track", post(lfs_track))
        .route("/repositories/:id/git/lfs/untrack", post(lfs_untrack))
        .route("/repositories/:id/git/lfs/pull", post(lfs_pull))
        .route("/repositories/:id/git/lfs/push", post(lfs_push))
        .route("/repositories/:id/git/rebase/plan", get(rebase_plan))
        .route(
            "/repositories/:id/git/rebase/interactive",
            post(interactive_rebase),
        )
        .route(
            "/repositories/:id/git/rebase/continue",
            post(rebase_continue),
        )
        .route("/repositories/:id/git/rebase/abort", post(rebase_abort))
        .route("/repositories/:id/git/rebase/skip", post(rebase_skip))
        .route("/repositories/:id/git/cherry-pick", post(cherry_pick))
        .route(
            "/repositories/:id/git/cherry-pick/abort",
            post(cherry_pick_abort),
        )
        .route("/repositories/:id/git/conflicts", get(conflicts))
        .route(
            "/repositories/:id/git/conflicts/detail",
            get(conflict_detail),
        )
        .route(
            "/repositories/:id/git/conflicts/resolve",
            post(resolve_conflict),
        )
        .route(
            "/repositories/:id/git/stashes",
            get(stashes).post(create_stash),
        )
        .route("/repositories/:id/git/stashes/apply", post(apply_stash))
        .route("/repositories/:id/git/stashes/drop", post(drop_stash))
}

#[derive(Debug, Deserialize)]
struct FilesRequest {
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PatchRequest {
    patch: String,
}

#[derive(Debug, Deserialize)]
struct CommitRequest {
    message: String,
    author_name: String,
    author_email: String,
    amend: Option<bool>,
    sign_off: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RemoteRequest {
    remote: Option<String>,
    branch: Option<String>,
    url: Option<String>,
    /// Pull only: `"ff-only"` (default) | `"merge"` | `"rebase"`. See `zync_git_core::PullMode`.
    #[serde(default)]
    mode: Option<String>,
    /// Push only: use `push_force_with_lease_with_credentials` instead of a plain push.
    #[serde(default)]
    force_with_lease: Option<bool>,
    /// Push only, force-with-lease path: a plain push always attempts to set upstream
    /// tracking (matching `git push -u`), but a force-with-lease push does not touch it by
    /// default — pass `true` to additionally set it after a successful lease push (e.g. a
    /// "publish branch" flow that force-pushed once already).
    #[serde(default)]
    set_upstream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct LfsRequest {
    pattern: Option<String>,
    remote: Option<String>,
    branch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BranchRequest {
    name: String,
    new_name: Option<String>,
    checkout: Option<bool>,
    revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RevisionRequest {
    revision: String,
    hard: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TagRequest {
    name: String,
    target: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CommitIdRequest {
    commit: String,
}

#[derive(Debug, Deserialize)]
struct CherryPickRequest {
    commits: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ResolveConflictRequest {
    path: String,
    side: String,
}

#[derive(Debug, Deserialize)]
struct RebaseRequest {
    base: String,
    steps: Vec<RebaseStepRequest>,
}

#[derive(Debug, Deserialize)]
struct RebaseStepRequest {
    commit: String,
    action: zync_git_core::RebaseAction,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StashRequest {
    message: Option<String>,
    author_name: Option<String>,
    author_email: Option<String>,
    index: Option<usize>,
    pop: Option<bool>,
}

async fn status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<zync_git_core::FileStatus>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::status(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn add(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<FilesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::add(repository.path, &request.files).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn unstage(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<FilesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::unstage(repository.path, &request.files).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn discard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<FilesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::discard(repository.path, &request.files).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn stage_patch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<PatchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::stage_patch(repository.path, request.patch.as_bytes())
        .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn commit(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<CommitRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let mut message = request.message;
    if request.sign_off.unwrap_or(false) {
        message.push_str(&format!(
            "\n\nSigned-off-by: {} <{}>",
            request.author_name, request.author_email
        ));
    }
    let oid = if request.amend.unwrap_or(false) {
        zync_git_core::amend_commit(
            repository.path,
            &message,
            &request.author_name,
            &request.author_email,
        )
    } else {
        zync_git_core::commit(
            repository.path,
            &message,
            &request.author_name,
            &request.author_email,
        )
    }
    .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff", "commits", "branches"]);
    Ok(Json(serde_json::json!({ "commit": oid })))
}

async fn diff_workdir(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let path = query.get("path").map(String::as_str);
    let patch = zync_git_core::diff_workdir_path(repository.path, path).map_err(internal_error)?;
    // Only cap whole-tree diffs: path-scoped diffs feed the staging UI and
    // must never be truncated, or the client would build a stage patch from
    // incomplete input.
    Ok(if path.is_none() {
        cap_diff(patch, max_bytes(&query))
    } else {
        patch
    })
}

async fn diff_staged(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let path = query.get("path").map(String::as_str);
    let patch = zync_git_core::diff_staged_path(repository.path, path).map_err(internal_error)?;
    // Same rule as diff_workdir: leave path-scoped (staging) diffs untouched.
    Ok(if path.is_none() {
        cap_diff(patch, max_bytes(&query))
    } else {
        patch
    })
}

async fn diff_commit(
    State(state): State<Arc<AppState>>,
    Path((id, commit_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let patch = zync_git_core::diff_commit(repository.path, &commit_id).map_err(internal_error)?;
    Ok(cap_diff(patch, max_bytes(&query)))
}

async fn diff_compare_commit(
    State(state): State<Arc<AppState>>,
    Path((id, commit_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let patch =
        zync_git_core::diff_commit_to_workdir(repository.path, &commit_id).map_err(internal_error)?;
    Ok(cap_diff(patch, max_bytes(&query)))
}

const DEFAULT_DIFF_MAX_BYTES: usize = 5_000_000;
const MIN_DIFF_MAX_BYTES: usize = 65_536;
const MAX_DIFF_MAX_BYTES: usize = 50_000_000;

/// Reads the optional `max_bytes` query param, clamped to a sane range.
fn max_bytes(query: &HashMap<String, String>) -> usize {
    query
        .get("max_bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_DIFF_MAX_BYTES)
        .clamp(MIN_DIFF_MAX_BYTES, MAX_DIFF_MAX_BYTES)
}

/// Truncates a whole-tree diff at the last newline before `max_bytes`,
/// appending a note so clients know the patch was cut off. Never call this
/// on path-scoped diffs used to build stage patches.
fn cap_diff(patch: String, max_bytes: usize) -> String {
    if patch.len() <= max_bytes {
        return patch;
    }
    // Clamp to the nearest valid char boundary at or before max_bytes so we
    // never slice inside a multi-byte UTF-8 sequence.
    let mut boundary = max_bytes.min(patch.len());
    while boundary > 0 && !patch.is_char_boundary(boundary) {
        boundary -= 1;
    }
    let cut = patch[..boundary]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let mut truncated = patch[..cut].to_string();
    truncated.push_str(&format!(
        "\n... [diff truncated: exceeded {max_bytes} bytes; pass ?max_bytes= to raise]"
    ));
    truncated
}

async fn fetch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remote_name = request.remote.as_deref().unwrap_or("origin");
    let spec = credentials::resolve_credential_spec(
        &state,
        credentials::DEFAULT_USER_ID,
        &repository.path,
        remote_name,
    )?;
    let result = zync_git_core::fetch_with_credentials(
        &repository.path,
        request.remote.as_deref(),
        Some(&spec),
    )
    .map_err(map_git_error)?;
    broadcast_git_change(&state, &id, &["branches", "commits"]);
    Ok(result)
}

/// Fetches every configured remote in turn. Stops at the first failure (its mapped status/body
/// is returned) rather than partially succeeding silently.
async fn fetch_all(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remotes = zync_git_core::remotes(&repository.path).map_err(internal_error)?;
    if remotes.is_empty() {
        return Ok("no remotes configured".to_string());
    }

    let mut results = Vec::with_capacity(remotes.len());
    for remote in &remotes {
        let spec = credentials::resolve_credential_spec(
            &state,
            credentials::DEFAULT_USER_ID,
            &repository.path,
            &remote.name,
        )?;
        let fetch_result = zync_git_core::fetch_with_credentials(
            &repository.path,
            Some(remote.name.as_str()),
            Some(&spec),
        );
        let result = match fetch_result {
            Ok(result) => result,
            Err(err) => {
                // A later remote can fail after an earlier one already fetched successfully —
                // broadcast now so clients pick up whatever landed instead of missing it because
                // the request as a whole errored out.
                if !results.is_empty() {
                    broadcast_git_change(&state, &id, &["branches", "commits"]);
                }
                return Err(map_git_error(err));
            }
        };
        results.push(result);
    }
    broadcast_git_change(&state, &id, &["branches", "commits"]);
    Ok(results.join("\n"))
}

async fn pull(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remote_name = request.remote.as_deref().unwrap_or("origin");
    let mode = parse_pull_mode(request.mode.as_deref())?;
    let spec = credentials::resolve_credential_spec(
        &state,
        credentials::DEFAULT_USER_ID,
        &repository.path,
        remote_name,
    )?;
    let result = zync_git_core::pull_with_credentials(
        &repository.path,
        request.remote.as_deref(),
        request.branch.as_deref(),
        mode,
        Some(&spec),
    )
    .map_err(map_git_error)?;
    broadcast_git_change(
        &state,
        &id,
        &["status", "diff", "commits", "branches", "conflicts"],
    );
    Ok(result)
}

fn parse_pull_mode(mode: Option<&str>) -> Result<zync_git_core::PullMode, (StatusCode, String)> {
    match mode {
        None | Some("ff-only") => Ok(zync_git_core::PullMode::FfOnly),
        Some("merge") => Ok(zync_git_core::PullMode::Merge),
        Some("rebase") => Ok(zync_git_core::PullMode::Rebase),
        Some(other) => Err((
            StatusCode::BAD_REQUEST,
            format!("mode must be 'ff-only', 'merge', or 'rebase', got '{other}'"),
        )),
    }
}

async fn push(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remote_name = request.remote.as_deref().unwrap_or("origin");
    let spec = credentials::resolve_credential_spec(
        &state,
        credentials::DEFAULT_USER_ID,
        &repository.path,
        remote_name,
    )?;

    let result = if request.force_with_lease.unwrap_or(false) {
        let branch = resolve_push_branch(&repository.path, request.branch.as_deref())?;
        let result = zync_git_core::push_force_with_lease_with_credentials(
            &repository.path,
            remote_name,
            &branch,
            Some(&spec),
        )
        .map_err(map_git_error)?;
        if request.set_upstream.unwrap_or(false) {
            match zync_git_core::set_upstream(&repository.path, &branch, remote_name, &branch) {
                Ok(_) => result,
                Err(err) => format!("{result} (warning: failed to set upstream tracking: {err})"),
            }
        } else {
            result
        }
    } else {
        zync_git_core::push_with_credentials(
            &repository.path,
            Some(remote_name),
            request.branch.as_deref(),
            Some(&spec),
        )
        .map_err(map_git_error)?
    };

    broadcast_git_change(&state, &id, &["branches", "commits"]);
    Ok(result)
}

/// Resolves an explicit branch or falls back to the repo's current branch — used by the
/// force-with-lease push path, whose git-core fn requires a concrete branch name rather than
/// resolving `None` itself the way `push_with_credentials` does.
fn resolve_push_branch(
    repo_path: &str,
    branch: Option<&str>,
) -> Result<String, (StatusCode, String)> {
    if let Some(branch) = branch {
        return Ok(branch.to_string());
    }
    zync_git_core::open_repo(repo_path)
        .ok()
        .and_then(|info| info.current_branch)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "branch is required: repository has no current branch".to_string(),
            )
        })
}

async fn remotes(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<zync_git_core::RemoteSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::remotes(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn add_remote(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let name = request.remote.as_deref().unwrap_or("origin");
    let url = request
        .url
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "url is required".to_string()))?;
    zync_git_core::add_remote(repository.path, name, url).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_remote(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let name = request.remote.as_deref().unwrap_or("origin");
    zync_git_core::delete_remote(repository.path, name).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn prune_remote(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let name = request.remote.as_deref().unwrap_or("origin");
    // `prune_remote` is a `run_git` CLI shellout (no credentialed variant exists in git-core —
    // it doesn't push/fetch objects, just prunes stale remote-tracking refs), but it still
    // contacts the remote and can fail with Auth/Network/Timeout, so it gets the same kind→status
    // mapping as the credentialed ops for a consistent error shape.
    let result = zync_git_core::prune_remote(repository.path, name).map_err(map_git_error)?;
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(result)
}

async fn delete_remote_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remote = request.remote.as_deref().unwrap_or("origin");
    let branch = request
        .branch
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "branch is required".to_string()))?;
    let spec = credentials::resolve_credential_spec(
        &state,
        credentials::DEFAULT_USER_ID,
        &repository.path,
        remote,
    )?;
    zync_git_core::delete_remote_branch_with_credentials(&repository.path, remote, branch, Some(&spec))
        .map_err(map_git_error)?;
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn push_force_with_lease(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remote = request.remote.as_deref().unwrap_or("origin");
    let branch = request
        .branch
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "branch is required".to_string()))?;
    let spec = credentials::resolve_credential_spec(
        &state,
        credentials::DEFAULT_USER_ID,
        &repository.path,
        remote,
    )?;
    let result = zync_git_core::push_force_with_lease_with_credentials(
        &repository.path,
        remote,
        branch,
        Some(&spec),
    )
    .map_err(map_git_error)?;
    broadcast_git_change(&state, &id, &["branches", "commits"]);
    Ok(result)
}

async fn branches(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<zync_git_core::BranchSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::branches(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn create_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    if let Some(revision) = request.revision.as_deref() {
        zync_git_core::create_branch_at(
            repository.path,
            &request.name,
            revision,
            request.checkout.unwrap_or(false),
        )
        .map_err(internal_error)?;
    } else {
        zync_git_core::create_branch(
            repository.path,
            &request.name,
            request.checkout.unwrap_or(false),
        )
        .map_err(internal_error)?;
    }
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn checkout_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::checkout_branch(repository.path, &request.name).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &["status", "diff", "commits", "branches", "conflicts"],
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn checkout_revision(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RevisionRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::checkout_revision(repository.path, &request.revision).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &["status", "diff", "commits", "branches", "conflicts"],
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::delete_branch(repository.path, &request.name).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn rename_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let new_name = request
        .new_name
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "new_name is required".to_string()))?;
    zync_git_core::rename_branch(repository.path, &request.name, &new_name)
        .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn merge_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::merge_branch(repository.path, &request.name).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &["status", "diff", "commits", "branches", "conflicts"],
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn set_upstream(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remote = request.remote.as_deref().unwrap_or("origin");
    let branch = request
        .branch
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "branch is required".to_string()))?;
    let result = zync_git_core::set_upstream(repository.path, branch, remote, branch)
        .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["branches"]);
    Ok(result)
}

async fn tags(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<zync_git_core::TagSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::tags(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn create_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<TagRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::create_tag(repository.path, &request.name, request.target.as_deref())
        .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["commits", "branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_tag(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<TagRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::delete_tag(repository.path, &request.name).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["commits", "branches"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn revert_commit(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<CommitIdRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let commit =
        zync_git_core::revert_commit(repository.path, &request.commit).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(Json(serde_json::json!({ "commit": commit })))
}

async fn commit_graph(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<zync_git_core::CommitSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(500)
        .min(5000);
    let cursor = query.get("cursor").map(String::as_str);
    zync_git_core::commit_graph(repository.path, limit, cursor)
        .map(Json)
        .map_err(internal_error)
}

async fn repo_stats(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<zync_git_core::RepoStats>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20_000);
    zync_git_core::repo_stats(repository.path, limit)
        .map(Json)
        .map_err(internal_error)
}

async fn rebase_plan(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<zync_git_core::CommitSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20)
        .min(200);
    zync_git_core::commit_graph(repository.path, limit, None)
        .map(Json)
        .map_err(internal_error)
}

async fn blame(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<zync_git_core::BlameLine>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let path = query
        .get("path")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "path is required".to_string()))?;
    zync_git_core::blame_file(repository.path, path)
        .map(Json)
        .map_err(internal_error)
}

async fn file_history(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<zync_git_core::CommitSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let path = query
        .get("path")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "path is required".to_string()))?;
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100)
        .min(1000);
    zync_git_core::file_history(repository.path, path, limit)
        .map(Json)
        .map_err(internal_error)
}

async fn tree_at_revision(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<zync_git_core::TreeEntrySummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let revision = query.get("revision").map(String::as_str).unwrap_or("HEAD");
    zync_git_core::tree_at_revision(repository.path, revision)
        .map(Json)
        .map_err(internal_error)
}

async fn blob_at_revision(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let path = query
        .get("path")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "path is required".to_string()))?;
    let revision = query.get("revision").map(String::as_str).unwrap_or("HEAD");
    let bytes =
        zync_git_core::blob_at_revision(repository.path, revision, path).map_err(internal_error)?;
    let headers = [(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_path(path)),
    )];
    Ok((headers, bytes))
}

async fn reflog(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<zync_git_core::ReflogEntrySummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(100)
        .min(1000);
    zync_git_core::reflog(repository.path, limit)
        .map(Json)
        .map_err(internal_error)
}

async fn reset_to_revision(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RevisionRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::reset_to_revision(
        repository.path,
        &request.revision,
        request.hard.unwrap_or(false),
    )
    .map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn submodules(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<zync_git_core::SubmoduleSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::submodules(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn submodule_init(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::submodule_init(repository.path).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn submodule_update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::submodule_update(repository.path).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn submodule_sync(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::submodule_sync(repository.path).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn lfs_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<zync_git_core::LfsSummary>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::lfs_summary(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn lfs_install(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::lfs_install(repository.path).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn lfs_track(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<LfsRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let pattern = request
        .pattern
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "pattern is required".to_string()))?;
    let result = zync_git_core::lfs_track(repository.path, pattern).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn lfs_untrack(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<LfsRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let pattern = request
        .pattern
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "pattern is required".to_string()))?;
    let result = zync_git_core::lfs_untrack(repository.path, pattern).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn lfs_pull(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::lfs_pull(repository.path).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn lfs_push(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<LfsRequest>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let remote = request.remote.as_deref().unwrap_or("origin");
    let branch = request
        .branch
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "branch is required".to_string()))?;
    let result =
        zync_git_core::lfs_push(repository.path, remote, branch).map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["status", "diff"]);
    Ok(result)
}

async fn interactive_rebase(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RebaseRequest>,
) -> Result<Json<zync_git_core::RebaseResult>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let steps = request
        .steps
        .into_iter()
        .map(|step| zync_git_core::RebaseStep {
            commit: step.commit,
            action: step.action,
            message: step.message,
        })
        .collect::<Vec<_>>();
    let result = zync_git_core::interactive_rebase(repository.path, &request.base, &steps)
        .map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(Json(result))
}

async fn rebase_continue(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::rebase_continue(repository.path).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(result)
}

async fn rebase_abort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::rebase_abort(repository.path).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(result)
}

async fn rebase_skip(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let result = zync_git_core::rebase_skip(repository.path).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(result)
}

async fn stashes(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<zync_git_core::StashSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::stash_list(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn cherry_pick(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<CherryPickRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::cherry_pick(repository.path, &request.commits).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn cherry_pick_abort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::cherry_pick_abort(repository.path).map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn conflicts(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<zync_git_core::ConflictSummary>>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::conflicts(repository.path)
        .map(Json)
        .map_err(internal_error)
}

async fn conflict_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<zync_git_core::ConflictDetail>, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let path = query
        .get("path")
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "path is required".to_string()))?;
    zync_git_core::conflict_detail(repository.path, path)
        .map(Json)
        .map_err(internal_error)
}

async fn resolve_conflict(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<ResolveConflictRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    let side = match request.side.as_str() {
        "local" => zync_git_core::ConflictSide::Local,
        "remote" => zync_git_core::ConflictSide::Remote,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "side must be local or remote".to_string(),
            ))
        }
    };
    zync_git_core::resolve_conflict_with_checkout(repository.path, &request.path, side)
        .map_err(internal_error)?;
    broadcast_git_change(
        &state,
        &id,
        &[
            "status",
            "diff",
            "commits",
            "branches",
            "conflicts",
            "stashes",
        ],
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn create_stash(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<StashRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::create_stash(
        repository.path,
        request.message.as_deref().unwrap_or("WIP"),
        request.author_name.as_deref().unwrap_or("Zync"),
        request.author_email.as_deref().unwrap_or("zync@local"),
    )
    .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["stashes", "status", "diff"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn apply_stash(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<StashRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::apply_stash(
        repository.path,
        request.index.unwrap_or(0),
        request.pop.unwrap_or(false),
    )
    .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["stashes", "status", "diff"]);
    Ok(StatusCode::NO_CONTENT)
}

async fn drop_stash(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<StashRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::drop_stash(repository.path, request.index.unwrap_or(0))
        .map_err(internal_error)?;
    broadcast_git_change(&state, &id, &["stashes", "status", "diff"]);
    Ok(StatusCode::NO_CONTENT)
}

fn repository(
    state: &AppState,
    id: &str,
) -> Result<crate::db::RepositoryRecord, (StatusCode, String)> {
    state
        .db
        .repository(id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "repository not found".to_string()))
}

fn broadcast_git_change(state: &AppState, repository_id: &str, scopes: &[&str]) {
    let Ok(Some(repository)) = state.db.repository(repository_id) else {
        return;
    };
    let Ok(workspace) = state
        .db
        .workspace_for_repository(&repository.id, &repository.name)
    else {
        return;
    };
    let mut event = WorkspaceEvent::new("git_changed");
    event.payload = serde_json::json!({ "scopes": scopes });
    state.hub.broadcast(&workspace.id, event);
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

/// Maps a failed remote-op (fetch/pull/push/clone) error to an HTTP status. Every credentialed
/// git-core network fn returns a `zync_git_core::GitCommandError` on failure — for both the
/// libgit2 and CLI-shellout transports (ADR-001) — so downcasting it classifies the failure by
/// `GitErrorKind` regardless of which transport handled the call; anything else (e.g. a repo-open
/// failure before the network call even starts) falls back to the same 500 as `internal_error`.
/// The raw, readable error string is always kept as the response body, matching the existing
/// repo convention — this is safe because `GitCommandError` never carries secret material (see
/// ADR-001 "Secrets never enter errors").
pub(crate) fn map_git_error(error: anyhow::Error) -> (StatusCode, String) {
    (git_error_status(&error), error.to_string())
}

fn git_error_status(error: &anyhow::Error) -> StatusCode {
    match error.downcast_ref::<zync_git_core::GitCommandError>() {
        Some(git_error) => git_error_kind_status(git_error.kind),
        None => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn git_error_kind_status(kind: zync_git_core::GitErrorKind) -> StatusCode {
    match kind {
        zync_git_core::GitErrorKind::Auth => StatusCode::UNAUTHORIZED,
        zync_git_core::GitErrorKind::Network => StatusCode::BAD_GATEWAY,
        zync_git_core::GitErrorKind::NonFastForward | zync_git_core::GitErrorKind::Conflict => {
            StatusCode::CONFLICT
        }
        zync_git_core::GitErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT,
        zync_git_core::GitErrorKind::Other => StatusCode::INTERNAL_SERVER_ERROR,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn git_error(kind: zync_git_core::GitErrorKind) -> anyhow::Error {
        zync_git_core::GitCommandError {
            command: "git fetch origin".to_string(),
            stderr: "boom".to_string(),
            kind,
        }
        .into()
    }

    #[test]
    fn auth_maps_to_401() {
        assert_eq!(
            git_error_status(&git_error(zync_git_core::GitErrorKind::Auth)),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn network_maps_to_502() {
        assert_eq!(
            git_error_status(&git_error(zync_git_core::GitErrorKind::Network)),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn non_fast_forward_maps_to_409() {
        assert_eq!(
            git_error_status(&git_error(zync_git_core::GitErrorKind::NonFastForward)),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn conflict_maps_to_409() {
        assert_eq!(
            git_error_status(&git_error(zync_git_core::GitErrorKind::Conflict)),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn timeout_maps_to_504() {
        assert_eq!(
            git_error_status(&git_error(zync_git_core::GitErrorKind::Timeout)),
            StatusCode::GATEWAY_TIMEOUT
        );
    }

    #[test]
    fn other_maps_to_500() {
        assert_eq!(
            git_error_status(&git_error(zync_git_core::GitErrorKind::Other)),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn non_git_command_error_falls_back_to_500() {
        assert_eq!(
            git_error_status(&anyhow::anyhow!("repository not found")),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn map_git_error_preserves_readable_body() {
        let (status, body) = map_git_error(git_error(zync_git_core::GitErrorKind::Auth));
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.contains("boom"), "body should keep the raw detail: {body}");
    }

    #[test]
    fn parse_pull_mode_defaults_to_ff_only() {
        assert_eq!(parse_pull_mode(None).unwrap(), zync_git_core::PullMode::FfOnly);
        assert_eq!(
            parse_pull_mode(Some("ff-only")).unwrap(),
            zync_git_core::PullMode::FfOnly
        );
        assert_eq!(
            parse_pull_mode(Some("merge")).unwrap(),
            zync_git_core::PullMode::Merge
        );
        assert_eq!(
            parse_pull_mode(Some("rebase")).unwrap(),
            zync_git_core::PullMode::Rebase
        );
    }

    #[test]
    fn parse_pull_mode_rejects_unknown_value() {
        let (status, _) = parse_pull_mode(Some("squash")).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
