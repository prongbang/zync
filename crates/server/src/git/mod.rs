use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
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
        .route("/repositories/:id/git/fetch", post(fetch))
        .route("/repositories/:id/git/pull", post(pull))
        .route("/repositories/:id/git/push", post(push))
        .route(
            "/repositories/:id/git/branches",
            get(branches).post(create_branch),
        )
        .route("/repositories/:id/git/checkout", post(checkout_branch))
        .route("/repositories/:id/git/branches/rename", post(rename_branch))
        .route("/repositories/:id/git/branches/merge", post(merge_branch))
        .route("/repositories/:id/git/branches/delete", post(delete_branch))
        .route("/repositories/:id/git/graph", get(commit_graph))
        .route("/repositories/:id/git/rebase/plan", get(rebase_plan))
        .route(
            "/repositories/:id/git/rebase/interactive",
            post(interactive_rebase),
        )
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
}

#[derive(Debug, Deserialize)]
struct BranchRequest {
    name: String,
    new_name: Option<String>,
    checkout: Option<bool>,
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
    Ok(StatusCode::NO_CONTENT)
}

async fn unstage(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<FilesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::unstage(repository.path, &request.files).map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn discard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<FilesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::discard(repository.path, &request.files).map_err(internal_error)?;
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
    Ok(Json(serde_json::json!({ "commit": oid })))
}

async fn diff_workdir(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::diff_workdir_path(repository.path, query.get("path").map(String::as_str))
        .map_err(internal_error)
}

async fn diff_staged(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::diff_staged_path(repository.path, query.get("path").map(String::as_str))
        .map_err(internal_error)
}

async fn diff_commit(
    State(state): State<Arc<AppState>>,
    Path((id, commit_id)): Path<(String, String)>,
) -> Result<String, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::diff_commit(repository.path, &commit_id).map_err(internal_error)
}

async fn fetch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::fetch(repository.path, request.remote.as_deref()).map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn pull(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::pull(
        repository.path,
        request.remote.as_deref(),
        request.branch.as_deref(),
    )
    .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn push(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<RemoteRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::push(
        repository.path,
        request.remote.as_deref(),
        request.branch.as_deref(),
    )
    .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
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
    zync_git_core::create_branch(
        repository.path,
        &request.name,
        request.checkout.unwrap_or(false),
    )
    .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn checkout_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::checkout_branch(repository.path, &request.name).map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::delete_branch(repository.path, &request.name).map_err(internal_error)?;
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
    Ok(StatusCode::NO_CONTENT)
}

async fn merge_branch(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<BranchRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::merge_branch(repository.path, &request.name).map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
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
    zync_git_core::commit_graph(repository.path, limit)
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
    zync_git_core::commit_graph(repository.path, limit)
        .map(Json)
        .map_err(internal_error)
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
        })
        .collect::<Vec<_>>();
    zync_git_core::interactive_rebase(repository.path, &request.base, &steps)
        .map(Json)
        .map_err(internal_error)
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
    Ok(StatusCode::NO_CONTENT)
}

async fn cherry_pick_abort(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let repository = repository(&state, &id)?;
    zync_git_core::cherry_pick_abort(repository.path).map_err(internal_error)?;
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

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
