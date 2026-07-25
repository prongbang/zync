use crate::auth::{AuthMode, ADMIN_ROLE, OWNER_ID};
use crate::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast;
use uuid::Uuid;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/ws/workspace/:id", get(workspace_socket))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEvent {
    pub id: String,
    pub workspace_id: Option<String>,
    pub kind: String,
    pub path: Option<String>,
    pub user_id: Option<String>,
    pub payload: serde_json::Value,
    pub timestamp: String,
}

impl WorkspaceEvent {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            workspace_id: None,
            kind: kind.into(),
            path: None,
            user_id: None,
            payload: serde_json::Value::Null,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn repository_opened(repository_id: &str) -> Self {
        let mut event = Self::new("repository_opened");
        event.payload = serde_json::json!({ "repository_id": repository_id });
        event
    }
}

#[derive(Clone, Default)]
pub struct WorkspaceHub {
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<WorkspaceEvent>>>>,
}

impl WorkspaceHub {
    pub fn broadcast(&self, workspace_id: &str, mut event: WorkspaceEvent) {
        event.workspace_id = Some(workspace_id.to_string());
        let sender = self.sender(workspace_id);
        let _ = sender.send(event);
    }

    fn sender(&self, workspace_id: &str) -> broadcast::Sender<WorkspaceEvent> {
        if let Some(sender) = self
            .channels
            .read()
            .expect("workspace hub lock")
            .get(workspace_id)
            .cloned()
        {
            return sender;
        }
        let mut channels = self.channels.write().expect("workspace hub lock");
        channels
            .entry(workspace_id.to_string())
            .or_insert_with(|| broadcast::channel(512).0)
            .clone()
    }
}

#[derive(Debug, Deserialize)]
struct WsHandshakeQuery {
    ticket: Option<String>,
}

/// WebSocket handshake. Auth is via a short-lived single-use ticket in the
/// query string (ADR-002 Decision 4) — the cookie auth middleware allowlists
/// `/ws/` and defers to this check, because cookies don't propagate reliably
/// onto a WS upgrade through the dev proxy / non-browser clients. The ticket is
/// validated and consumed *before* `on_upgrade`; an invalid one is rejected
/// with `401`. In `disabled` mode the ticket check is skipped entirely and the
/// synthetic owner (a global admin) drives the socket.
///
/// The consumed ticket yields the connecting user, whose repo-scoped role we
/// resolve once here to decide whether inbound (client→server) events may be
/// re-broadcast: reads (server→client) work for any member incl. viewers, but
/// only `member+` may *inject* events (N3 — otherwise a viewer with a valid
/// ticket could forge `git_changed`/presence events for everyone).
async fn workspace_socket(
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
    Query(query): Query<WsHandshakeQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let user_id = if state.auth.mode == AuthMode::Disabled {
        // No ticket in disabled mode; the synthetic owner drives the socket.
        OWNER_ID.to_string()
    } else {
        match query
            .ticket
            .as_deref()
            .and_then(|ticket| {
                state
                    .auth
                    .tickets
                    .consume(ticket, &workspace_id, chrono::Utc::now())
            }) {
            Some(user_id) => user_id,
            None => {
                return (StatusCode::UNAUTHORIZED, "invalid or missing ws ticket")
                    .into_response()
            }
        }
    };

    let can_write = user_can_write_workspace(&state.db, &user_id, &workspace_id);
    ws.on_upgrade(move |socket| handle_socket(state, workspace_id, can_write, socket))
}

/// Resolve whether `user_id` may inject events into `workspace_id`'s stream:
/// a global `admin`, or a repo-scoped `owner`/`member` of the workspace's
/// repository. Viewers (and any non-member) are read-only on the socket.
fn user_can_write_workspace(db: &crate::db::Database, user_id: &str, workspace_id: &str) -> bool {
    if let Ok(Some(user)) = db.user_by_id(user_id) {
        if user.role == ADMIN_ROLE {
            return true;
        }
    }
    let Ok(Some(workspace)) = db.workspace(workspace_id) else {
        return false;
    };
    matches!(
        db.repo_role_for_user(&workspace.repository_id, user_id),
        Ok(Some(role)) if role == "owner" || role == "member"
    )
}

#[cfg(test)]
mod tests {
    use super::user_can_write_workspace;
    use crate::db::Database;

    /// N3: only `member+`/`admin` may inject inbound socket events; viewers and
    /// non-members are read-only.
    #[test]
    fn socket_write_access_is_member_plus() {
        let db = Database::open(":memory:").expect("db");
        // `owner` is the seeded global admin.
        db.create_user("bob", "bob@z", "Bob", "user").unwrap();
        db.create_user("mem", "mem@z", "Mem", "user").unwrap();
        db.create_user("vwr", "vwr@z", "Vwr", "user").unwrap();
        db.create_user("out", "out@z", "Out", "user").unwrap();
        let repo = db.create_repository("p", "/tmp/p", None, "bob").unwrap();
        let ws = db.workspace_for_repository(&repo.id, &repo.name).unwrap();
        db.add_repo_member(&repo.id, "mem", "member").unwrap();
        db.add_repo_member(&repo.id, "vwr", "viewer").unwrap();

        assert!(user_can_write_workspace(&db, "owner", &ws.id), "global admin writes");
        assert!(user_can_write_workspace(&db, "bob", &ws.id), "repo owner writes");
        assert!(user_can_write_workspace(&db, "mem", &ws.id), "member writes");
        assert!(!user_can_write_workspace(&db, "vwr", &ws.id), "viewer is read-only");
        assert!(!user_can_write_workspace(&db, "out", &ws.id), "non-member is read-only");
        assert!(
            !user_can_write_workspace(&db, "mem", "no-such-ws"),
            "unknown workspace denies write"
        );
    }
}

/// RAII guard pairing [`observability::Metrics::ws_connection_opened`] with
/// its `ws_connection_closed` — dropped at the end of [`handle_socket`]'s
/// scope no matter how it exits (normal return, an early `break`, a panic
/// such as a poisoned-mutex unwind in `state.hub.sender()`, or task
/// cancellation), so the gauge can't leak upward the way an explicit
/// tail-of-function decrement could.
struct WsGauge(Arc<AppState>);

impl Drop for WsGauge {
    fn drop(&mut self) {
        self.0.metrics.ws_connection_closed();
    }
}

async fn handle_socket(
    state: Arc<AppState>,
    workspace_id: String,
    can_write: bool,
    socket: WebSocket,
) {
    // Live connection gauge for `/metrics` (P5.3) — opened here, closed by
    // `WsGauge`'s `Drop` impl regardless of how this function exits.
    state.metrics.ws_connection_opened();
    let _gauge = WsGauge(state.clone());
    let sender = state.hub.sender(&workspace_id);
    let mut receiver = sender.subscribe();
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let outbound = tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            if let Ok(text) = serde_json::to_string(&event) {
                if ws_sender.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        }
    });

    while let Some(Ok(message)) = ws_receiver.next().await {
        match message {
            // Inbound client→server events are re-broadcast to every subscriber,
            // so they are a write: a read-only viewer must not be able to inject
            // forged events (N3). Drain-and-ignore their text frames instead of
            // rebroadcasting; the outbound (read) path keeps working.
            Message::Text(text) if can_write => {
                if let Ok(mut event) = serde_json::from_str::<WorkspaceEvent>(&text) {
                    event.workspace_id = Some(workspace_id.clone());
                    let _ = sender.send(event);
                }
            }
            Message::Text(_) => {}
            Message::Close(_) => break,
            _ => {}
        }
    }

    outbound.abort();
}
