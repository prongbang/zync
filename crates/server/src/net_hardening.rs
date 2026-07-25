//! Network hardening (PLAN.md P4.2, DESIGN.md ADR-002 Decision 7 threat
//! notes): CORS, security response headers, request-body size limits, and
//! rate limiting for the brute-force-sensitive auth endpoints. Kept in one
//! module so `main.rs` (CORS/headers/body-limit, applied to the whole app)
//! and `auth::routes()` (rate limiting, applied to specific routes) share a
//! single source of truth for these knobs.
//!
//! The app is same-origin in both dev (Vite proxies API route prefixes onto
//! the Axum server, see `web/apps/web/vite.config.ts`) and production (the
//! server serves the built SPA from `ZYNC_STATIC_DIR`), so cross-origin CORS
//! is not required for normal operation — a browser only consults
//! `Access-Control-*` headers for cross-origin requests, so a same-origin
//! request is unaffected either way. `ZYNC_CORS_ORIGINS` is an explicit
//! opt-in for anyone running the API cross-origin from its own SPA.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::Router;
use std::sync::Arc;
use std::time::Duration;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::{KeyExtractor, PeerIpKeyExtractor, SmartIpKeyExtractor};
use tower_governor::{GovernorError, GovernorLayer};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};

use crate::AppState;

// ---- CORS ----

/// Builds the CORS layer from `ZYNC_CORS_ORIGINS` (a comma-separated list of
/// origins, e.g. `https://zync.example.com,https://other.example.com`).
pub fn cors_layer() -> CorsLayer {
    build_cors_layer(&std::env::var("ZYNC_CORS_ORIGINS").unwrap_or_default())
}

/// Unset/blank `origins_env` (the default) allow-lists nothing: no origin
/// gets `Access-Control-Allow-Origin`, which is safe for same-origin use —
/// same-origin requests never consult CORS headers at all, so this is "no
/// cross-origin API access", not "same-origin breaks".
///
/// When origins ARE configured, credentials (the `zync_session` cookie) are
/// allowed only for those explicit origins — never combined with a
/// wildcard: `Access-Control-Allow-Credentials: true` together with
/// `Access-Control-Allow-Origin: *` is invalid per the fetch spec and no
/// browser honors it, so allow-listing `*` here would be worse than no CORS
/// layer at all.
fn build_cors_layer(origins_env: &str) -> CorsLayer {
    let origins = parse_origins(origins_env);
    if origins.is_empty() {
        return CorsLayer::new();
    }
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_credentials(true)
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request())
}

fn parse_origins(origins_env: &str) -> Vec<HeaderValue> {
    origins_env
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| match HeaderValue::from_str(s) {
            Ok(value) => Some(value),
            Err(error) => {
                // Silently dropping this would let an operator believe a
                // typo'd/malformed entry is allow-listed when it never made
                // it into the CORS layer at all.
                tracing::warn!("ZYNC_CORS_ORIGINS: ignoring invalid origin {s:?}: {error}");
                None
            }
        })
        .collect()
}

// ---- Security response headers ----

/// A CSP that lets the bundled Vite SPA run while locking everything else
/// down:
///   - `script-src` is left unset so it falls back to `default-src 'self'`:
///     same-origin only, no inline `<script>`, no `eval`. Verified against
///     the production Vite build, which emits only external
///     `/assets/*.js` module scripts (`web/apps/web/index.html` has no
///     inline script either) — no adjustment needed.
///   - `img-src` allows `https:` for gravatar avatars (`format.ts`'s
///     `gravatarSrc`, `https://www.gravatar.com/avatar/...`).
///   - `style-src` allows `'unsafe-inline'` because Radix UI/shadcn
///     primitives (popovers, tooltips, dialogs) position themselves via
///     inline `style` attributes.
///   - `connect-src 'self'` covers same-origin `fetch` and the `/ws`
///     WebSocket upgrade (both same-origin in dev via the Vite proxy and in
///     production).
///   - `frame-ancestors 'none'` + `object-src 'none'` close off
///     clickjacking and plugin embedding.
const CSP: &str = "default-src 'self'; img-src 'self' data: https:; \
     style-src 'self' 'unsafe-inline'; connect-src 'self'; font-src 'self' data:; \
     frame-ancestors 'none'; base-uri 'self'; object-src 'none'";

/// Applied to every response (ADR-002 Decision 7 / PLAN.md P4.2). Only
/// inserts a header when the handler hasn't already set one — the raw-blob
/// route (`blob_at_revision`/`blob_response_headers` in
/// `crates/server/src/git/mod.rs`) sets its own `nosniff` and, for SVG, a
/// stricter sandboxed CSP for attacker-controlled repository bytes; this
/// layer must not override or conflict with that.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    insert_if_absent(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    insert_if_absent(headers, header::X_FRAME_OPTIONS, "DENY");
    insert_if_absent(headers, header::REFERRER_POLICY, "same-origin");
    insert_if_absent(headers, header::CONTENT_SECURITY_POLICY, CSP);
    response
}

fn insert_if_absent(headers: &mut HeaderMap, name: HeaderName, value: &'static str) {
    if !headers.contains_key(&name) {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

// ---- Request body size limit ----

/// Global request body cap (PLAN.md P4.2): bounds worst-case memory from a
/// hostile giant body while staying big enough for legitimate large
/// payloads — `stage_patch` (`crates/server/src/git/mod.rs`) ships a full
/// unified diff as JSON, and `write_file`/`create_file`
/// (`crates/server/src/files/mod.rs`) ship raw file content as JSON.
///
/// axum's `Bytes`/`Json` extractors already enforce their own 2MB default
/// independently of any layer — see `main.rs`, which pairs this with
/// `DefaultBodyLimit::disable()` so this explicit, documented cap is the
/// only one in effect (otherwise the tighter of the two limits would win,
/// silently capping legitimate payloads at 2MB regardless of this value).
pub const MAX_REQUEST_BODY_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

// ---- Rate limiting (tower_governor) ----

/// `ZYNC_TRUSTED_PROXY=1` — set this only when a reverse proxy you control
/// terminates TLS in front of this server AND that proxy discards/rewrites
/// any inbound `X-Forwarded-For`/`X-Real-IP`/`Forwarded` headers before
/// setting its own (so a client can't spoof them).
///
/// Unset (the default) keys rate limiting on the raw TCP peer address
/// (`PeerIpKeyExtractor`) — correct for direct exposure, but WRONG behind a
/// proxy: the peer address is then always the proxy's own IP, so every
/// client collapses into one shared bucket. Concretely, that means one
/// noisy client can exhaust the `/auth/login` bucket and lock out
/// *everyone's* login, and the per-IP brute-force defense is nullified
/// since an attacker shares the same apparent IP as legitimate users.
/// Setting this flag switches to `SmartIpKeyExtractor`, which recovers the
/// real client IP from forwarded headers (falling back to the peer address
/// if none are present) so each client gets its own bucket again.
///
/// Terminating TLS at a proxy WITHOUT setting this makes peer-IP rate
/// limiting effectively inoperative for that deployment shape — enforce
/// rate limiting at the proxy instead in that case. See `DESIGN.md`
/// ADR-002 Decision 7 / the P5.5 deploy-env notes.
fn trusted_proxy() -> bool {
    trusted_proxy_env(std::env::var("ZYNC_TRUSTED_PROXY").ok().as_deref())
}

fn trusted_proxy_env(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Wraps `router` with the strict rate limit for the brute-force-sensitive
/// endpoints: `POST /auth/login` and `POST /setup` (the one-time
/// admin-bootstrap token consumption). ~10/min per IP: a burst of 10
/// immediately, replenished one every 6s thereafter.
pub fn with_strict_rate_limit(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    let label = "auth/login+setup rate limiter";
    if trusted_proxy() {
        with_rate_limit(router, SmartIpKeyExtractor, label, 6, 10)
    } else {
        with_rate_limit(router, PeerIpKeyExtractor, label, 6, 10)
    }
}

/// Wraps `router` with a deliberately generous rate limit for
/// `POST /auth/ws-ticket`. It's fetched on EVERY WebSocket reconnect (the
/// frontend's backoff loop in `useWorkspace.ts` retries every 2-30s, so a
/// flaky connection can burst several calls in the first minute) — too
/// tight a limit here breaks live-sync reconnection, which is worse than
/// not rate-limiting it at all. It also already requires a valid session
/// (`ws_ticket` takes an `AuthUser` extractor and isn't in `is_public`'s
/// allowlist), so this is a courtesy ceiling against a runaway reconnect
/// loop, not a brute-force defense. 60/min steady state, burst of 40.
pub fn with_ws_ticket_rate_limit(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    let label = "auth/ws-ticket rate limiter";
    if trusted_proxy() {
        with_rate_limit(router, SmartIpKeyExtractor, label, 1, 40)
    } else {
        with_rate_limit(router, PeerIpKeyExtractor, label, 1, 40)
    }
}

/// Builds a governor-rate-limited router for the given key extractor —
/// `PeerIpKeyExtractor` (direct exposure, default) or `SmartIpKeyExtractor`
/// (`ZYNC_TRUSTED_PROXY=1`, see `trusted_proxy`'s doc comment).
fn with_rate_limit<K>(
    router: Router<Arc<AppState>>,
    key_extractor: K,
    label: &'static str,
    per_second: u64,
    burst_size: u32,
) -> Router<Arc<AppState>>
where
    K: KeyExtractor + Send + Sync + 'static,
    K::Key: Send + Sync,
{
    let config = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(key_extractor)
            .per_second(per_second)
            .burst_size(burst_size)
            .error_handler(rate_limit_error_response)
            .finish()
            .expect("static governor config: burst_size and period are both nonzero"),
    );
    let limiter = config.limiter().clone();
    spawn_governor_cleanup(label, move || limiter.retain_recent());
    router.layer(GovernorLayer { config })
}

/// Returns `429 Too Many Requests` with a standard `Retry-After: <seconds>`
/// header. tower_governor's own `GovernorError::as_response` is `pub(crate)`
/// to its crate (inaccessible here) and only sets a nonstandard
/// `x-ratelimit-after` header, so we build the response ourselves.
fn rate_limit_error_response(error: GovernorError) -> Response {
    match error {
        GovernorError::TooManyRequests { wait_time, .. } => {
            let mut response = Response::new(Body::from(format!(
                "rate limit exceeded, retry in {wait_time}s"
            )));
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            // `wait_time` truncates to whole seconds and can legitimately be
            // `0` (replenishment due in under a second) — round up so
            // `Retry-After` never tells a client to retry with no delay at
            // all, which would just recreate a tight retry loop.
            let retry_after_secs = wait_time.max(1);
            if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            response
        }
        GovernorError::UnableToExtractKey => {
            let mut response = Response::new(Body::from("rate limit key extraction failed"));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            response
        }
        GovernorError::Other { code, msg, .. } => {
            let mut response = Response::new(Body::from(msg.unwrap_or_default()));
            *response.status_mut() = code;
            response
        }
    }
}

/// Periodically evicts stale per-IP buckets from a governor rate limiter's
/// in-memory store so a long-lived deployment doesn't accumulate one entry
/// per unique caller IP forever (recommended by tower_governor's own docs).
fn spawn_governor_cleanup(label: &'static str, cleanup: impl Fn() + Send + Sync + 'static) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10 * 60));
        loop {
            interval.tick().await;
            cleanup();
            tracing::debug!("{label}: swept stale rate-limit entries");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::connect_info::ConnectInfo;
    use axum::http::Request;
    use axum::routing::{get, post};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use tower::ServiceExt; // oneshot

    fn peer(ip: [u8; 4]) -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), 12345))
    }

    /// Minimal `AppState` for routes that never touch the DB/auth beyond
    /// being constructible (mirrors `auth::tests::app_state`).
    fn app_state() -> Arc<AppState> {
        Arc::new(AppState {
            db: crate::db::Database::open(":memory:").expect("open in-memory db"),
            hub: crate::websocket::WorkspaceHub::default(),
            sync: crate::sync::WorkspaceSync::default(),
            collaboration: crate::collaboration::CollaborationState::default(),
            secrets: crate::crypto::KeyState::Unconfigured,
            auth: crate::auth::AuthState::disabled_for_test(),
            repos_root: crate::repos_root::ReposRoot::default(),
            metrics: Arc::new(crate::observability::Metrics::default()),
        })
    }

    // ---- CORS ----

    #[test]
    fn unset_origins_env_yields_no_allowlist() {
        assert!(parse_origins("").is_empty());
        assert!(parse_origins("   ").is_empty());
    }

    #[test]
    fn origins_env_is_comma_separated_and_trimmed() {
        let origins = parse_origins(" https://a.example , https://b.example ,,");
        assert_eq!(
            origins,
            vec![
                HeaderValue::from_static("https://a.example"),
                HeaderValue::from_static("https://b.example"),
            ]
        );
    }

    #[tokio::test]
    async fn disallowed_origin_gets_no_acao_header() {
        let app = axum::Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(build_cors_layer("https://allowed.example"));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header(header::ORIGIN, "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Not a CORS rejection (the browser enforces CORS, not the server) —
        // the request still reaches the handler — but no ACAO header means
        // the browser will withhold the response from cross-origin JS.
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    }

    #[tokio::test]
    async fn allowed_origin_gets_acao_and_credentials() {
        let app = axum::Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(build_cors_layer("https://allowed.example"));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header(header::ORIGIN, "https://allowed.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://allowed.example"
        );
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap(),
            "true"
        );
    }

    #[tokio::test]
    async fn default_empty_allowlist_still_serves_same_origin_requests() {
        // No ZYNC_CORS_ORIGINS configured: no ACAO header is ever added, but
        // a same-origin request (no Origin header, as browsers send for
        // same-origin navigations/fetches) still reaches the handler fine.
        let app = axum::Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(build_cors_layer(""));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // ---- Security headers ----

    #[tokio::test]
    async fn security_headers_present_on_normal_response() {
        let app = axum::Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(security_headers));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let headers = response.headers();
        assert_eq!(
            headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(),
            "nosniff"
        );
        assert_eq!(headers.get(header::X_FRAME_OPTIONS).unwrap(), "DENY");
        assert_eq!(headers.get(header::REFERRER_POLICY).unwrap(), "same-origin");
        let csp = headers
            .get(header::CONTENT_SECURITY_POLICY)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("frame-ancestors 'none'"));
    }

    #[tokio::test]
    async fn security_headers_do_not_override_a_handler_set_csp() {
        // Mirrors the raw-blob route: the handler sets its own, stricter
        // headers for attacker-controlled bytes; the global layer must not
        // clobber them.
        async fn blob_like() -> Response {
            let mut response = Response::new(Body::from("<svg/>"));
            response.headers_mut().insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("sandbox"),
            );
            response
        }

        let app = axum::Router::new()
            .route("/blob", get(blob_like))
            .layer(axum::middleware::from_fn(security_headers));

        let response = app
            .oneshot(Request::builder().uri("/blob").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .unwrap(),
            "sandbox"
        );
        // The layer still fills in headers the handler didn't set itself.
        assert_eq!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap(),
            "nosniff"
        );
    }

    // ---- Rate limiting ----

    #[tokio::test]
    async fn login_rate_limit_allows_burst_then_429s_with_retry_after() {
        let app = with_strict_rate_limit(
            axum::Router::<Arc<AppState>>::new().route("/auth/login", post(|| async { "ok" })),
        )
        .with_state(app_state());

        for attempt in 1..=10 {
            let mut request = Request::builder()
                .method("POST")
                .uri("/auth/login")
                .body(Body::empty())
                .unwrap();
            request.extensions_mut().insert(peer([203, 0, 113, 10]));
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "attempt {attempt} should be within the burst"
            );
        }

        let mut request = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(peer([203, 0, 113, 10]));
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(response.headers().get(header::RETRY_AFTER).is_some());

        // A different caller IP is unaffected (per-IP key, not global).
        let mut request = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(peer([203, 0, 113, 11]));
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ws_ticket_rate_limit_tolerates_a_realistic_reconnect_burst() {
        // The frontend's reconnect backoff can fire several calls in the
        // first minute of a flaky connection; the generous ws-ticket limit
        // must absorb that without 429ing.
        let app = with_ws_ticket_rate_limit(
            axum::Router::<Arc<AppState>>::new().route("/auth/ws-ticket", post(|| async { "ok" })),
        )
        .with_state(app_state());

        for attempt in 1..=20 {
            let mut request = Request::builder()
                .method("POST")
                .uri("/auth/ws-ticket")
                .body(Body::empty())
                .unwrap();
            request.extensions_mut().insert(peer([198, 51, 100, 5]));
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "attempt {attempt} should not be rate-limited"
            );
        }
    }

    // ---- ZYNC_TRUSTED_PROXY / key extractor selection ----

    #[test]
    fn trusted_proxy_flag_requires_exact_value() {
        assert!(!trusted_proxy_env(None));
        assert!(!trusted_proxy_env(Some("")));
        assert!(!trusted_proxy_env(Some("true")));
        assert!(!trusted_proxy_env(Some("0")));
        assert!(trusted_proxy_env(Some("1")));
    }

    /// Reproduces the reviewed footgun directly: behind a reverse proxy every
    /// caller shares one TCP peer address, so `PeerIpKeyExtractor` (the
    /// default, `ZYNC_TRUSTED_PROXY` unset) must key on that shared address
    /// regardless of any `X-Forwarded-For` a client or proxy sets — two
    /// "different" callers (as seen via XFF) still exhaust the SAME bucket.
    #[tokio::test]
    async fn peer_ip_extractor_ignores_x_forwarded_for_and_shares_one_bucket() {
        let app = with_rate_limit(
            axum::Router::<Arc<AppState>>::new().route("/auth/login", post(|| async { "ok" })),
            PeerIpKeyExtractor,
            "test",
            6,
            2,
        )
        .with_state(app_state());

        // Same peer (the shared proxy IP), two different claimed XFF clients.
        for xff in ["203.0.113.1", "203.0.113.2"] {
            let mut request = Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("x-forwarded-for", xff)
                .body(Body::empty())
                .unwrap();
            request.extensions_mut().insert(peer([10, 0, 0, 1]));
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        // The burst (2) is now exhausted for the shared peer address, even
        // though this is a third distinct XFF-claimed client — this is
        // exactly the "one noisy client locks out everyone" failure mode.
        let mut request = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("x-forwarded-for", "203.0.113.3")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(peer([10, 0, 0, 1]));
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    /// The fix: `SmartIpKeyExtractor` (`ZYNC_TRUSTED_PROXY=1`) recovers the
    /// real per-client IP from `X-Forwarded-For`, so distinct clients behind
    /// the same proxy peer address get independent buckets again.
    #[tokio::test]
    async fn smart_ip_extractor_gives_each_forwarded_client_its_own_bucket() {
        let app = with_rate_limit(
            axum::Router::<Arc<AppState>>::new().route("/auth/login", post(|| async { "ok" })),
            SmartIpKeyExtractor,
            "test",
            6,
            1,
        )
        .with_state(app_state());

        // Same shared peer (the proxy), different XFF clients: each gets its
        // own single-request burst rather than sharing one.
        for xff in ["203.0.113.1", "203.0.113.2", "203.0.113.3"] {
            let mut request = Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("x-forwarded-for", xff)
                .body(Body::empty())
                .unwrap();
            request.extensions_mut().insert(peer([10, 0, 0, 1]));
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "client {xff} should get its own bucket"
            );
        }

        // The SAME forwarded client hitting again immediately does exhaust
        // its own (burst-of-1) bucket.
        let mut request = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("x-forwarded-for", "203.0.113.1")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(peer([10, 0, 0, 1]));
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn with_strict_rate_limit_uses_peer_ip_extractor_by_default() {
        // `with_strict_rate_limit`/`with_ws_ticket_rate_limit` read
        // `ZYNC_TRUSTED_PROXY` via `trusted_proxy()`; absent it (the default
        // in this test process), they must behave like the `PeerIpKeyExtractor`
        // case above rather than `SmartIpKeyExtractor` — i.e. XFF is ignored
        // and same-peer callers share a bucket. This only holds as long as no
        // other test in this binary sets `ZYNC_TRUSTED_PROXY=1`, which none do.
        assert!(std::env::var("ZYNC_TRUSTED_PROXY").is_err());
        let app = with_strict_rate_limit(
            axum::Router::<Arc<AppState>>::new().route("/auth/login", post(|| async { "ok" })),
        )
        .with_state(app_state());

        for i in 0..10 {
            let mut request = Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("x-forwarded-for", format!("203.0.113.{i}"))
                .body(Body::empty())
                .unwrap();
            request.extensions_mut().insert(peer([10, 0, 0, 2]));
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        // 11th request from the same peer (still a "new" XFF client) is
        // still limited, proving XFF was never consulted.
        let mut request = Request::builder()
            .method("POST")
            .uri("/auth/login")
            .header("x-forwarded-for", "203.0.113.99")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(peer([10, 0, 0, 2]));
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    // ---- Request body size limit ----

    #[tokio::test]
    async fn oversized_body_is_rejected_with_413() {
        use axum::extract::DefaultBodyLimit;
        use tower_http::limit::RequestBodyLimitLayer;

        // Mirrors the real handlers this protects (`Json<T>` in `login`,
        // `stage_patch`, `write_file`, ...): the body must actually be read
        // for the limit to trigger — tower_http's `Limited` wrapper only
        // errors when the caller consumes past the cap (or a `Content-Length`
        // header announces it upfront), not merely because a big body was
        // sent and ignored.
        async fn echo(body: axum::body::Bytes) -> StatusCode {
            let _ = body;
            StatusCode::OK
        }

        let app = axum::Router::new()
            .route("/echo", post(echo))
            // Mirrors main.rs: disable axum's own independent 2MB default so
            // this layer's limit is the only one in effect.
            .layer(DefaultBodyLimit::disable())
            .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES));

        let over_limit = vec![0u8; MAX_REQUEST_BODY_BYTES + 1];
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/echo")
                    .body(Body::from(over_limit))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let within_limit = vec![0u8; 1024];
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/echo")
                    .body(Body::from(within_limit))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
