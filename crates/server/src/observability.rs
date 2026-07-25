//! P5.3 observability: per-request ids threaded through tracing spans (both
//! log formats — see `ZYNC_LOG_FORMAT` in `main.rs`), `/health` (liveness)
//! vs `/ready` (readiness — a cheap DB touch), and a minimal hand-rolled
//! Prometheus-text `/metrics`.
//!
//! `/metrics` is deliberately dependency-free: the workspace `Cargo.toml` is
//! owned by a parallel P5.4 (release engineering) change, so instead of
//! pulling in `metrics`/`metrics-exporter-prometheus` this keeps a handful of
//! atomic counters in [`Metrics`] and formats them by hand into Prometheus
//! text exposition format on request. Revisit with a real `metrics` crate if
//! per-route cardinality or richer histograms are ever needed.

use crate::auth::{AuthUser, ADMIN_ROLE};
use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::{
    sync::atomic::{AtomicI64, AtomicU64, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Header read (inbound, if the caller/proxy already assigned one) and
/// always written (outbound) for correlating a request across log lines and
/// client/server.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// The per-request id, stashed in request extensions by
/// [`request_id_middleware`] so `TraceLayer`'s `make_span_with` ([`make_span`],
/// wired in `main.rs`) can read it back into the tracing span. That layer
/// ordering matters: `request_id_middleware` must run *outside* `TraceLayer`
/// in the tower stack (added *after* `TraceLayer::layer(...)` in `main.rs`,
/// which makes it the outer of the two — the last `.layer()` call wraps
/// outermost) so the extension already exists by the time `make_span` fires.
#[derive(Clone)]
pub struct RequestId(pub String);

/// A short, log-friendly id: a v4 UUID without dashes (32 lowercase hex
/// chars) — enough entropy to correlate a request without a full UUID's
/// visual noise.
fn generate_request_id() -> String {
    Uuid::new_v4().simple().to_string()
}

/// A conservative allowlist for an inbound `X-Request-Id`: bounded length,
/// ASCII alphanumeric plus `-`/`_`. Anything else (missing header, empty,
/// oversized, exotic bytes) falls back to a freshly generated id rather than
/// letting a client stuff arbitrary bytes into log lines and the echoed
/// response header.
fn sanitize_inbound_id(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 128 {
        return None;
    }
    if !raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(raw.to_string())
}

/// Assigns a request id (honoring a well-formed inbound `X-Request-Id`, else
/// generating one), stashes it in request extensions for `TraceLayer`'s span
/// ([`make_span`]), and echoes it back as the `X-Request-Id` response header
/// so a client/proxy can correlate its request with server-side logs.
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(sanitize_inbound_id)
        .unwrap_or_else(generate_request_id);

    req.extensions_mut().insert(RequestId(id.clone()));

    let mut res = next.run(req).await;
    if let Ok(value) = HeaderValue::from_str(&id) {
        res.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    res
}

/// `TraceLayer::make_span_with` callback (wired in `main.rs`) — reads the
/// [`RequestId`] stashed by [`request_id_middleware`] into every log line's
/// span context, in both the default human format and `ZYNC_LOG_FORMAT=json`.
pub fn make_span(req: &Request) -> tracing::Span {
    let request_id = req
        .extensions()
        .get::<RequestId>()
        .map(|r| r.0.as_str())
        .unwrap_or("-");
    tracing::info_span!(
        "http_request",
        method = %req.method(),
        uri = %req.uri(),
        request_id = %request_id,
    )
}

// ---- Metrics ----

/// A handful of atomic counters/gauges backing `/metrics`. Cheap to update
/// (`Ordering::Relaxed` — these are independent counters, not used to
/// synchronize other memory) and cheap to read; `AppState` holds one shared
/// instance behind an `Arc`.
#[derive(Default)]
pub struct Metrics {
    requests_total: AtomicU64,
    requests_2xx: AtomicU64,
    requests_4xx: AtomicU64,
    requests_5xx: AtomicU64,
    /// Cumulative request-handling time, in microseconds (converted to
    /// seconds when rendered — Prometheus convention).
    request_duration_micros_sum: AtomicU64,
    bucket_under_10ms: AtomicU64,
    bucket_under_50ms: AtomicU64,
    bucket_under_100ms: AtomicU64,
    bucket_under_500ms: AtomicU64,
    bucket_under_1s: AtomicU64,
    bucket_over_1s: AtomicU64,
    /// Signed so a mismatched open/close pair (shouldn't happen, but this is
    /// a best-effort gauge, not a safety invariant) saturates visibly at a
    /// negative number instead of wrapping a `u64` to a huge positive one.
    ws_connections: AtomicI64,
}

impl Metrics {
    /// Records one completed request: bumps the total + status-class
    /// counters and files its latency into the bucket histogram.
    pub fn record_request(&self, status: StatusCode, elapsed: Duration) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        match status.as_u16() {
            200..=299 => {
                self.requests_2xx.fetch_add(1, Ordering::Relaxed);
            }
            400..=499 => {
                self.requests_4xx.fetch_add(1, Ordering::Relaxed);
            }
            500..=599 => {
                self.requests_5xx.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
        self.request_duration_micros_sum
            .fetch_add(micros, Ordering::Relaxed);

        let bucket = if elapsed <= Duration::from_millis(10) {
            &self.bucket_under_10ms
        } else if elapsed <= Duration::from_millis(50) {
            &self.bucket_under_50ms
        } else if elapsed <= Duration::from_millis(100) {
            &self.bucket_under_100ms
        } else if elapsed <= Duration::from_millis(500) {
            &self.bucket_under_500ms
        } else if elapsed <= Duration::from_secs(1) {
            &self.bucket_under_1s
        } else {
            &self.bucket_over_1s
        };
        bucket.fetch_add(1, Ordering::Relaxed);
    }

    pub fn ws_connection_opened(&self) {
        self.ws_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn ws_connection_closed(&self) {
        self.ws_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Renders Prometheus text exposition format. `sync_watchers` is passed
    /// in rather than stored here, since it lives in `WorkspaceSync`
    /// (`sync::WorkspaceSync::watcher_count`), not in `Metrics` itself.
    pub fn render_prometheus(&self, sync_watchers: usize) -> String {
        let total = self.requests_total.load(Ordering::Relaxed);
        let ok = self.requests_2xx.load(Ordering::Relaxed);
        let client_err = self.requests_4xx.load(Ordering::Relaxed);
        let server_err = self.requests_5xx.load(Ordering::Relaxed);
        let other = total.saturating_sub(ok + client_err + server_err);
        let duration_sum_secs =
            self.request_duration_micros_sum.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let ws = self.ws_connections.load(Ordering::Relaxed);

        let mut out = String::new();
        out.push_str(
            "# HELP zync_http_requests_total Total HTTP requests handled, by status class.\n",
        );
        out.push_str("# TYPE zync_http_requests_total counter\n");
        out.push_str(&format!(
            "zync_http_requests_total{{status=\"2xx\"}} {ok}\n"
        ));
        out.push_str(&format!(
            "zync_http_requests_total{{status=\"4xx\"}} {client_err}\n"
        ));
        out.push_str(&format!(
            "zync_http_requests_total{{status=\"5xx\"}} {server_err}\n"
        ));
        out.push_str(&format!(
            "zync_http_requests_total{{status=\"other\"}} {other}\n"
        ));

        out.push_str(
            "# HELP zync_http_request_duration_seconds Request latency histogram, in seconds.\n",
        );
        out.push_str("# TYPE zync_http_request_duration_seconds histogram\n");
        let mut cumulative = 0u64;
        for (le, bucket) in [
            ("0.01", &self.bucket_under_10ms),
            ("0.05", &self.bucket_under_50ms),
            ("0.1", &self.bucket_under_100ms),
            ("0.5", &self.bucket_under_500ms),
            ("1", &self.bucket_under_1s),
        ] {
            cumulative += bucket.load(Ordering::Relaxed);
            out.push_str(&format!(
                "zync_http_request_duration_seconds_bucket{{le=\"{le}\"}} {cumulative}\n"
            ));
        }
        cumulative += self.bucket_over_1s.load(Ordering::Relaxed);
        out.push_str(&format!(
            "zync_http_request_duration_seconds_bucket{{le=\"+Inf\"}} {cumulative}\n"
        ));
        out.push_str(&format!(
            "zync_http_request_duration_seconds_sum {duration_sum_secs:.6}\n"
        ));
        out.push_str(&format!(
            "zync_http_request_duration_seconds_count {total}\n"
        ));

        out.push_str("# HELP zync_ws_connections Active WebSocket connections.\n");
        out.push_str("# TYPE zync_ws_connections gauge\n");
        out.push_str(&format!("zync_ws_connections {ws}\n"));

        out.push_str("# HELP zync_sync_watchers Active filesystem watcher threads.\n");
        out.push_str("# TYPE zync_sync_watchers gauge\n");
        out.push_str(&format!("zync_sync_watchers {sync_watchers}\n"));

        out
    }
}

/// Times the full request (including auth/authz and the route handler) and
/// files it into `state.metrics`. Ordering relative to `request_id_middleware`
/// doesn't matter (each is independent bookkeeping over the same request).
pub async fn metrics_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let res = next.run(req).await;
    state.metrics.record_request(res.status(), start.elapsed());
    res
}

// ---- Probes ----

/// `GET /health` — liveness. Always 200, no I/O (in particular, no DB touch —
/// that's `/ready`'s job): a load balancer/orchestrator uses this to decide
/// "is the process alive enough to keep routing to", not "is it fully
/// functional". Also reports the running server's version (P5.4 wires this
/// into the release pipeline / a UI footer).
pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// `GET /ready` — readiness. Does a trivial, non-mutating DB touch (a lookup
/// of the always-present seeded `owner` row — see `db::seed_default_user`) so
/// an orchestrator can hold traffic back from an instance whose DB
/// connection/lock/schema isn't answering, without paying for that check on
/// every `/health` liveness ping.
pub async fn ready(State(state): State<Arc<AppState>>) -> Response {
    match state.db.user_by_id("owner") {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ready" }))).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "readiness probe: database did not answer");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "not_ready" })),
            )
                .into_response()
        }
    }
}

/// `GET /metrics` — Prometheus text exposition format. Admin-gated (not
/// listed in `auth::is_public`, so a session is required at all; this
/// additionally requires the `admin` role) because request counts, latency,
/// and connection gauges are internal operational state, not something every
/// authenticated user should see. A future multi-tenant deploy that wants a
/// separate scrape identity instead of an admin session can swap this check
/// for a `ZYNC_METRICS_TOKEN` bearer check without touching the rest of the
/// module.
pub async fn metrics(auth: AuthUser, State(state): State<Arc<AppState>>) -> Response {
    if auth.role != ADMIN_ROLE {
        return (StatusCode::FORBIDDEN, "admin only").into_response();
    }
    let body = state.metrics.render_prometheus(state.sync.watcher_count());
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn sanitize_inbound_id_accepts_reasonable_tokens_only() {
        assert_eq!(
            sanitize_inbound_id("abc-123_XYZ"),
            Some("abc-123_XYZ".to_string())
        );
        assert_eq!(sanitize_inbound_id(""), None);
        assert_eq!(sanitize_inbound_id("has spaces"), None);
        assert_eq!(sanitize_inbound_id("has/slash"), None);
        assert_eq!(sanitize_inbound_id(&"a".repeat(129)), None);
        assert_eq!(sanitize_inbound_id(&"a".repeat(128)), Some("a".repeat(128)));
    }

    #[test]
    fn generate_request_id_is_short_and_hex() {
        let id = generate_request_id();
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn metrics_render_reflects_recorded_requests() {
        let metrics = Metrics::default();
        metrics.record_request(StatusCode::OK, Duration::from_millis(5));
        metrics.record_request(StatusCode::NOT_FOUND, Duration::from_millis(75));
        metrics.record_request(StatusCode::INTERNAL_SERVER_ERROR, Duration::from_secs(2));
        metrics.ws_connection_opened();
        metrics.ws_connection_opened();
        metrics.ws_connection_closed();

        let text = metrics.render_prometheus(3);
        assert!(text.contains("zync_http_requests_total{status=\"2xx\"} 1"));
        assert!(text.contains("zync_http_requests_total{status=\"4xx\"} 1"));
        assert!(text.contains("zync_http_requests_total{status=\"5xx\"} 1"));
        assert!(text.contains("zync_http_request_duration_seconds_count 3"));
        assert!(text.contains("zync_http_request_duration_seconds_bucket{le=\"+Inf\"} 3"));
        assert!(text.contains("zync_ws_connections 1"));
        assert!(text.contains("zync_sync_watchers 3"));
    }

    // ---- Route-level tests (P5.3 item 5) ----

    use axum::body::{to_bytes, Body};
    use axum::http::Request as HttpRequest;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt; // oneshot

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            db: crate::db::Database::open(":memory:").expect("open in-memory db"),
            hub: crate::websocket::WorkspaceHub::default(),
            sync: crate::sync::WorkspaceSync::default(),
            collaboration: crate::collaboration::CollaborationState::default(),
            secrets: crate::crypto::KeyState::Unconfigured,
            auth: crate::auth::AuthState::disabled_for_test(),
            repos_root: crate::repos_root::ReposRoot::default(),
            metrics: Arc::new(Metrics::default()),
        })
    }

    async fn json_body(res: Response) -> serde_json::Value {
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_is_always_200_and_reports_version() {
        let res = health().await.into_response();
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn ready_is_200_when_db_answers() {
        let state = test_state();
        let res = ready(State(state)).await;
        assert_eq!(res.status(), StatusCode::OK);
        let body = json_body(res).await;
        assert_eq!(body["status"], "ready");
    }

    #[tokio::test]
    async fn request_id_middleware_generates_and_echoes_a_header() {
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(request_id_middleware));

        let res = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let generated = res
            .headers()
            .get(REQUEST_ID_HEADER)
            .expect("x-request-id present")
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(generated.len(), 32);
        assert!(generated.chars().all(|c| c.is_ascii_hexdigit()));

        // A well-formed inbound id is honored verbatim rather than replaced.
        let res = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/health")
                    .header(REQUEST_ID_HEADER, "caller-supplied-id-123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            res.headers().get(REQUEST_ID_HEADER).unwrap(),
            "caller-supplied-id-123"
        );
    }

    /// `metrics`'s own role gate, exercised directly (no router/extractor
    /// plumbing needed — `AuthUser` is a plain extractor argument, so it can
    /// be constructed by hand here the same way `require_auth` would have
    /// populated it from a real session).
    #[tokio::test]
    async fn metrics_rejects_non_admin_and_allows_admin() {
        let state = test_state();

        let non_admin = AuthUser {
            id: "someone".to_string(),
            role: "user".to_string(),
        };
        let res = metrics(non_admin, State(state.clone())).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);

        let admin = AuthUser {
            id: "owner".to_string(),
            role: ADMIN_ROLE.to_string(),
        };
        let res = metrics(admin, State(state)).await;
        assert_eq!(res.status(), StatusCode::OK);
    }
}
