use crate::{
    auth::{AuthUser, ADMIN_ROLE},
    websocket::WorkspaceEvent,
    AppState,
};
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

    /// The `user_id` currently holding the lock on `path`, if any. Used to
    /// authorize `unlock_file`, whose route carries no `:user_id` — only the
    /// path being unlocked.
    fn lock_owner(&self, workspace_id: &str, path: &str) -> Option<String> {
        self.inner
            .read()
            .expect("collaboration lock")
            .get(workspace_id)
            .and_then(|workspace| workspace.locks.get(path).cloned())
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
    auth: AuthUser,
    Path((workspace_id, user_id)): Path<(String, String)>,
    Json(request): Json<PresenceRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    authorize_presence_actor(&auth, &user_id)?;
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
    Ok(StatusCode::NO_CONTENT)
}

async fn leave(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((workspace_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    authorize_presence_actor(&auth, &user_id)?;
    state.collaboration.remove_user(&workspace_id, &user_id);
    let mut event = WorkspaceEvent::new("user_left");
    event.user_id = Some(user_id);
    state.hub.broadcast(&workspace_id, event);
    Ok(StatusCode::NO_CONTENT)
}

/// Presence is asserted under a `:user_id` in the path; a caller may only act as
/// themselves (a global `admin` may act as anyone). This closes the presence
/// spoof where any member could register/clear presence as another user. The
/// repo-scope guard already ensures the caller is a member of the workspace's
/// repository — this narrows *which* identity they can present as.
fn authorize_presence_actor(auth: &AuthUser, user_id: &str) -> Result<(), (StatusCode, String)> {
    if auth.role == ADMIN_ROLE || auth.id == user_id {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            "cannot act as another user".to_string(),
        ))
    }
}

async fn lock_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((workspace_id, path)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.collaboration.set_lock(&workspace_id, &path, &auth.id);
    let mut event = WorkspaceEvent::new("file_locked");
    event.path = Some(path);
    event.user_id = Some(auth.id);
    state.hub.broadcast(&workspace_id, event);
    Ok(StatusCode::NO_CONTENT)
}

async fn unlock_file(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((workspace_id, path)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    authorize_lock_actor(&auth, &state, &workspace_id, &path)?;
    state.collaboration.remove_lock(&workspace_id, &path);
    let mut event = WorkspaceEvent::new("file_unlocked");
    event.path = Some(path);
    state.hub.broadcast(&workspace_id, event);
    Ok(StatusCode::NO_CONTENT)
}

/// N1 (P4.4 security review): `lock_file`/`unlock_file` carry no `:user_id`
/// in the route — `lock_file` used to take the actor straight from the
/// request body (any member could lock a path *as* someone else), and
/// `unlock_file` had no actor check at all (any member could clear anyone's
/// lock). `lock_file` now always locks as the authenticated caller
/// (`AuthUser`, ignoring any body-supplied identity); `unlock_file` may only
/// be called by the lock's current holder or a global admin. Unlocking a
/// path with no active lock is a no-op regardless of caller — nothing to
/// authorize against.
fn authorize_lock_actor(
    auth: &AuthUser,
    state: &AppState,
    workspace_id: &str,
    path: &str,
) -> Result<(), (StatusCode, String)> {
    if auth.role == ADMIN_ROLE {
        return Ok(());
    }
    match state.collaboration.lock_owner(workspace_id, path) {
        Some(owner) if owner == auth.id => Ok(()),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            "cannot clear another user's lock".to_string(),
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: &str, role: &str) -> AuthUser {
        AuthUser {
            id: id.to_string(),
            role: role.to_string(),
        }
    }

    /// N6: presence is asserted under a `:user_id`; a caller may only present as
    /// themselves, except a global admin who may act as anyone.
    #[test]
    fn presence_actor_must_be_self_or_admin() {
        // Acting as yourself is allowed.
        assert!(authorize_presence_actor(&user("bob", "user"), "bob").is_ok());
        // Acting as someone else is forbidden...
        let err = authorize_presence_actor(&user("bob", "user"), "eve").unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        // ...unless you are a global admin.
        assert!(authorize_presence_actor(&user("root", ADMIN_ROLE), "eve").is_ok());
    }

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            db: crate::db::Database::open(":memory:").expect("open in-memory db"),
            hub: crate::websocket::WorkspaceHub::default(),
            sync: crate::sync::WorkspaceSync::default(),
            collaboration: CollaborationState::default(),
            secrets: crate::crypto::KeyState::Unconfigured,
            auth: crate::auth::AuthState::disabled_for_test(),
            repos_root: crate::repos_root::ReposRoot::default(),
        })
    }

    /// N1: `unlock_file`'s route has no `:user_id` — the actor is authorized
    /// against whoever currently holds the lock (or nobody, which is a no-op).
    #[test]
    fn lock_actor_must_be_holder_or_admin() {
        let state = test_state();
        state.collaboration.set_lock("ws", "a.txt", "bob");

        // The lock holder may clear their own lock.
        assert!(authorize_lock_actor(&user("bob", "user"), &state, "ws", "a.txt").is_ok());
        // Someone else may not.
        let err =
            authorize_lock_actor(&user("eve", "user"), &state, "ws", "a.txt").unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        // ...unless they are a global admin.
        assert!(authorize_lock_actor(&user("root", ADMIN_ROLE), &state, "ws", "a.txt").is_ok());
        // A path with no active lock has nothing to authorize against.
        assert!(authorize_lock_actor(&user("eve", "user"), &state, "ws", "unlocked.txt").is_ok());
    }

    /// N1 end-to-end: `lock_file` always locks as the authenticated caller
    /// (never a body-supplied identity), and `unlock_file` rejects a
    /// non-owner, non-admin caller trying to clear someone else's lock.
    #[tokio::test]
    async fn lock_file_locks_as_caller_and_unlock_rejects_non_owner() {
        let state = test_state();
        let workspace_id = "ws".to_string();
        let path = "a.txt".to_string();

        lock_file(
            State(state.clone()),
            user("bob", "user"),
            Path((workspace_id.clone(), path.clone())),
        )
        .await
        .expect("bob can lock a.txt");
        assert_eq!(
            state.collaboration.lock_owner(&workspace_id, &path),
            Some("bob".to_string())
        );

        let (status, _) = unlock_file(
            State(state.clone()),
            user("eve", "user"),
            Path((workspace_id.clone(), path.clone())),
        )
        .await
        .expect_err("eve must not be able to clear bob's lock");
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            state.collaboration.lock_owner(&workspace_id, &path),
            Some("bob".to_string()),
            "lock must still be held after the rejected unlock"
        );

        unlock_file(
            State(state.clone()),
            user("bob", "user"),
            Path((workspace_id.clone(), path.clone())),
        )
        .await
        .expect("bob can clear his own lock");
        assert_eq!(state.collaboration.lock_owner(&workspace_id, &path), None);
    }
}
