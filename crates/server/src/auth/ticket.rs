//! Short-lived single-use WebSocket tickets — ADR-002 Decision 4.
//!
//! Cookies don't propagate reliably onto a WS upgrade through the dev Vite
//! proxy or non-browser clients, and a long-lived token in the URL query would
//! leak into access logs. So an authed `POST /auth/ws-ticket` mints an opaque,
//! ~30s-TTL, single-use ticket bound to `(user_id, workspace_id)`, held here in
//! memory (never the DB — it's ephemeral). The `/ws/workspace/:id` handshake
//! validates and consumes it before `on_upgrade`.

use chrono::{DateTime, Duration, Utc};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::session;

/// Ticket time-to-live from mint. Deliberately tiny — a ticket is used within
/// milliseconds of being minted (the client immediately opens the socket).
pub const TICKET_TTL_SECS: i64 = 30;

struct Ticket {
    user_id: String,
    workspace_id: String,
    expires_at: DateTime<Utc>,
}

/// In-memory ticket store. Cheap to clone (shares the inner map).
#[derive(Clone, Default)]
pub struct WsTicketStore {
    inner: Arc<Mutex<HashMap<String, Ticket>>>,
}

impl WsTicketStore {
    /// Mint a ticket bound to `(user_id, workspace_id)`, expiring `TICKET_TTL_SECS`
    /// from now. Returns the opaque ticket string.
    pub fn mint(&self, user_id: &str, workspace_id: &str) -> String {
        self.mint_at(user_id, workspace_id, Utc::now() + Duration::seconds(TICKET_TTL_SECS))
    }

    /// Testable mint with an explicit expiry.
    pub fn mint_at(&self, user_id: &str, workspace_id: &str, expires_at: DateTime<Utc>) -> String {
        let ticket = session::generate_token();
        let mut map = self.inner.lock().expect("ws ticket lock");
        map.insert(
            ticket.clone(),
            Ticket {
                user_id: user_id.to_string(),
                workspace_id: workspace_id.to_string(),
                expires_at,
            },
        );
        ticket
    }

    /// Validate + consume a ticket (single-use: it is removed regardless of the
    /// outcome). Returns the bound `user_id` iff the ticket exists, has not
    /// expired at `now`, and matches `workspace_id`.
    pub fn consume(&self, ticket: &str, workspace_id: &str, now: DateTime<Utc>) -> Option<String> {
        let mut map = self.inner.lock().expect("ws ticket lock");
        let entry = map.remove(ticket)?;
        if now >= entry.expires_at {
            return None;
        }
        if entry.workspace_id != workspace_id {
            return None;
        }
        Some(entry.user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_then_consume_returns_user_and_is_single_use() {
        let store = WsTicketStore::default();
        let ticket = store.mint("owner", "ws-1");
        // First consume succeeds and yields the bound user.
        assert_eq!(
            store.consume(&ticket, "ws-1", Utc::now()).as_deref(),
            Some("owner")
        );
        // Second consume of the same ticket fails — single-use.
        assert!(store.consume(&ticket, "ws-1", Utc::now()).is_none());
    }

    #[test]
    fn wrong_workspace_is_rejected_and_still_consumed() {
        let store = WsTicketStore::default();
        let ticket = store.mint("owner", "ws-1");
        // Mismatched workspace → rejected...
        assert!(store.consume(&ticket, "ws-2", Utc::now()).is_none());
        // ...and the ticket is burned even on a mismatch (no retry against the
        // right workspace).
        assert!(store.consume(&ticket, "ws-1", Utc::now()).is_none());
    }

    #[test]
    fn expired_ticket_is_rejected() {
        let store = WsTicketStore::default();
        let past = Utc::now() - Duration::seconds(1);
        let ticket = store.mint_at("owner", "ws-1", past);
        assert!(store.consume(&ticket, "ws-1", Utc::now()).is_none());
    }

    #[test]
    fn unknown_ticket_is_rejected() {
        let store = WsTicketStore::default();
        assert!(store.consume("nope", "ws-1", Utc::now()).is_none());
    }
}
