use crate::{websocket::WorkspaceEvent, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/workspace/:id/presence", get(presence))
        .route("/workspace/:id/presence/:user_id", put(join).delete(leave))
        .route(
            "/workspace/:id/locks/:path",
            put(lock_file).delete(unlock_file),
        )
}

#[derive(Clone, Default)]
pub struct CollaborationState {
    inner: Arc<RwLock<HashMap<String, WorkspaceCollaboration>>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceCollaboration {
    pub users: HashMap<String, PresenceUser>,
    pub locks: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceUser {
    pub user_id: String,
    pub name: String,
    pub current_file: Option<String>,
    pub cursor_line: Option<u32>,
    pub cursor_column: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PresenceRequest {
    name: String,
    current_file: Option<String>,
    cursor_line: Option<u32>,
    cursor_column: Option<u32>,
}

impl CollaborationState {
    pub fn online_users(&self, workspace_id: &str) -> Vec<PresenceUser> {
        self.inner
            .read()
            .expect("collaboration lock")
            .get(workspace_id)
            .map(|workspace| workspace.users.values().cloned().collect())
            .unwrap_or_default()
    }

    fn upsert_user(&self, workspace_id: &str, user: PresenceUser) {
        let mut inner = self.inner.write().expect("collaboration lock");
        inner
            .entry(workspace_id.to_string())
            .or_default()
            .users
            .insert(user.user_id.clone(), user);
    }

    fn remove_user(&self, workspace_id: &str, user_id: &str) {
        if let Some(workspace) = self
            .inner
            .write()
            .expect("collaboration lock")
            .get_mut(workspace_id)
        {
            workspace.users.remove(user_id);
        }
    }

    fn set_lock(&self, workspace_id: &str, path: &str, user_id: &str) {
        let mut inner = self.inner.write().expect("collaboration lock");
        inner
            .entry(workspace_id.to_string())
            .or_default()
            .locks
            .insert(path.to_string(), user_id.to_string());
    }

    fn remove_lock(&self, workspace_id: &str, path: &str) {
        if let Some(workspace) = self
            .inner
            .write()
            .expect("collaboration lock")
            .get_mut(workspace_id)
        {
            workspace.locks.remove(path);
        }
    }
}

async fn presence(
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
) -> Json<Vec<PresenceUser>> {
    Json(state.collaboration.online_users(&workspace_id))
}

async fn join(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, user_id)): Path<(String, String)>,
    Json(request): Json<PresenceRequest>,
) -> StatusCode {
    let user = PresenceUser {
        user_id: user_id.clone(),
        name: request.name,
        current_file: request.current_file,
        cursor_line: request.cursor_line,
        cursor_column: request.cursor_column,
    };
    state.collaboration.upsert_user(&workspace_id, user);
    let mut event = WorkspaceEvent::new("user_joined");
    event.user_id = Some(user_id);
    state.hub.broadcast(&workspace_id, event);
    StatusCode::NO_CONTENT
}

async fn leave(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, user_id)): Path<(String, String)>,
) -> StatusCode {
    state.collaboration.remove_user(&workspace_id, &user_id);
    let mut event = WorkspaceEvent::new("user_left");
    event.user_id = Some(user_id);
    state.hub.broadcast(&workspace_id, event);
    StatusCode::NO_CONTENT
}

async fn lock_file(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, path)): Path<(String, String)>,
    Json(request): Json<serde_json::Value>,
) -> Result<StatusCode, (StatusCode, String)> {
    let user_id = request
        .get("user_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "user_id is required".to_string()))?;
    state.collaboration.set_lock(&workspace_id, &path, user_id);
    let mut event = WorkspaceEvent::new("file_locked");
    event.path = Some(path);
    event.user_id = Some(user_id.to_string());
    state.hub.broadcast(&workspace_id, event);
    Ok(StatusCode::NO_CONTENT)
}

async fn unlock_file(
    State(state): State<Arc<AppState>>,
    Path((workspace_id, path)): Path<(String, String)>,
) -> StatusCode {
    state.collaboration.remove_lock(&workspace_id, &path);
    let mut event = WorkspaceEvent::new("file_unlocked");
    event.path = Some(path);
    state.hub.broadcast(&workspace_id, event);
    StatusCode::NO_CONTENT
}
