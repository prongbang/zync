use crate::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
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
        let hub = self.clone();
        let workspace_id = workspace_id.to_string();
        tokio::spawn(async move {
            let sender = hub.sender(&workspace_id).await;
            let _ = sender.send(event);
        });
    }

    async fn sender(&self, workspace_id: &str) -> broadcast::Sender<WorkspaceEvent> {
        if let Some(sender) = self.channels.read().await.get(workspace_id).cloned() {
            return sender;
        }
        let mut channels = self.channels.write().await;
        channels
            .entry(workspace_id.to_string())
            .or_insert_with(|| broadcast::channel(512).0)
            .clone()
    }
}

async fn workspace_socket(
    State(state): State<Arc<AppState>>,
    Path(workspace_id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(state, workspace_id, socket))
}

async fn handle_socket(state: Arc<AppState>, workspace_id: String, socket: WebSocket) {
    let sender = state.hub.sender(&workspace_id).await;
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
            Message::Text(text) => {
                if let Ok(mut event) = serde_json::from_str::<WorkspaceEvent>(&text) {
                    event.workspace_id = Some(workspace_id.clone());
                    let _ = sender.send(event);
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    outbound.abort();
}
