//! Authentication core — DESIGN.md ADR-002.
//!
//! Owns password auth (`password`), opaque cookie sessions with sliding expiry
//! (`session`), the WebSocket ticket store (`ticket`), the router-wide auth
//! middleware + `AuthUser` extractor, the `/auth/*` and `/setup` routes, the
//! first-boot admin bootstrap, and the background session sweep.
//!
//! `ZYNC_AUTH=disabled` (Decision 3) reproduces today's single-user/no-auth
//! behavior byte-for-byte: the middleware injects a synthetic `owner`/`admin`
//! user into every request, login/logout are no-ops, and the WS ticket check is
//! skipped. Default is `enabled`.

pub mod authz;
pub mod password;
pub mod session;
pub mod ticket;

use crate::AppState;
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header, request::Parts, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// The seeded owner's user id — retained as the bootstrap admin's id so the
/// migration backfill lines up (ADR-002 Decision 6) and `disabled` mode yields
/// exactly today's `"owner"` identity.
pub const OWNER_ID: &str = "owner";
/// The global role that bypasses every repo-scope authorization check (ADR-002
/// Decision 5). The synthetic `disabled`-mode principal and the bootstrap owner
/// both carry it, so `disabled` mode retains full, unchecked access.
pub const ADMIN_ROLE: &str = "admin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Enabled,
    Disabled,
}

/// Per-process auth configuration + ephemeral state, held on `AppState`.
#[derive(Clone)]
pub struct AuthState {
    pub mode: AuthMode,
    /// `Secure` attribute on the session cookie; dropped only via
    /// `ZYNC_COOKIE_INSECURE=1` for plain-HTTP LAN/dev.
    pub cookie_secure: bool,
    pub tickets: ticket::WsTicketStore,
    setup: Arc<Mutex<Option<SetupToken>>>,
}

struct SetupToken {
    token: String,
    expires_at: DateTime<Utc>,
}

impl AuthState {
    /// Resolve auth config from the environment. `ZYNC_AUTH` is validated at
    /// boot — an unknown value refuses to start (ADR-002 Decision 3) so a typo
    /// can't silently open a server the operator believed was locked.
    pub fn load() -> anyhow::Result<Self> {
        let mode = match std::env::var("ZYNC_AUTH").ok().as_deref() {
            None | Some("enabled") => AuthMode::Enabled,
            Some("disabled") => AuthMode::Disabled,
            Some(other) => {
                anyhow::bail!("ZYNC_AUTH must be 'enabled' or 'disabled', got '{other}'")
            }
        };
        let cookie_secure = std::env::var("ZYNC_COOKIE_INSECURE")
            .map(|v| v != "1")
            .unwrap_or(true);
        Ok(Self {
            mode,
            cookie_secure,
            tickets: ticket::WsTicketStore::default(),
            setup: Arc::new(Mutex::new(None)),
        })
    }

    /// Test-only constructor: other modules' tests (e.g. `repository::tests`)
    /// need an `AuthState` but can't build the struct literal directly — the
    /// `setup` field is private to this module.
    #[cfg(test)]
    pub(crate) fn disabled_for_test() -> Self {
        Self {
            mode: AuthMode::Disabled,
            cookie_secure: false,
            tickets: ticket::WsTicketStore::default(),
            setup: Arc::new(Mutex::new(None)),
        }
    }

    fn set_setup_token(&self, token: String, expires_at: DateTime<Utc>) {
        *self.setup.lock().expect("setup token lock") = Some(SetupToken { token, expires_at });
    }

    /// True iff `token` matches the current, unexpired setup token. Does not
    /// consume it (see `clear_setup_token`).
    fn setup_token_valid(&self, token: &str, now: DateTime<Utc>) -> bool {
        self.setup
            .lock()
            .expect("setup token lock")
            .as_ref()
            .is_some_and(|s| now < s.expires_at && constant_time_eq(&s.token, token))
    }

    fn clear_setup_token(&self) {
        *self.setup.lock().expect("setup token lock") = None;
    }
}

pub fn routes() -> Router<Arc<AppState>> {
    // Rate limiting (P4.2, DESIGN.md ADR-002 Decision 7): `/auth/login` and
    // `/setup` are brute-force-sensitive and get a strict per-IP quota;
    // `/auth/ws-ticket` is fetched on every WebSocket reconnect and gets a
    // deliberately generous one so a flaky connection can't lock itself out
    // of live sync. See `net_hardening::with_strict_rate_limit`/
    // `with_ws_ticket_rate_limit` for the exact quotas and rationale.
    let login_and_setup = crate::net_hardening::with_strict_rate_limit(
        Router::new()
            .route("/auth/login", post(login))
            .route("/setup", get(setup_get).post(setup_post)),
    );
    let ws_ticket_route = crate::net_hardening::with_ws_ticket_rate_limit(
        Router::new().route("/auth/ws-ticket", post(ws_ticket)),
    );

    Router::new()
        .merge(login_and_setup)
        .merge(ws_ticket_route)
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        // Admin user provisioning (P3.5, ADR-002 Decision 1: "User creation is
        // admin-only"). Deliberately NOT in `is_public`'s allowlist below — a
        // session is required — and each handler additionally checks
        // `auth.role == ADMIN_ROLE` itself (the repo-scope authz middleware
        // only classifies `/repositories/*` and `/workspace/*`, so admin-only
        // `/auth/*` routes gate in the handler, same as `ws_ticket` above).
        .route("/auth/users", get(list_users).post(create_user))
}

// ---- AuthUser extractor ----

/// The authenticated principal for a request, injected into request extensions
/// by [`require_auth`]. Handlers take it as an extractor to get the real user id
/// + global role (replacing the old hardcoded `DEFAULT_USER_ID`).
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    /// Global role (`admin` | `user`). `admin` bypasses every repo-scope
    /// authorization check (ADR-002 Decision 5); a normal `user`'s access is
    /// resolved per-repository via `workspace_members`.
    pub role: String,
}

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or((StatusCode::UNAUTHORIZED, "unauthenticated".to_string()))
    }
}

// ---- Middleware ----

/// Router-wide auth layer (ADR-002 Decision 4). One `from_fn_with_state` layer
/// wraps every merged route, so a newly-added route is authenticated by default
/// — you must opt *out* via the allowlist, not remember to opt in.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Response {
    // Disabled mode: inject the synthetic owner into every request; no cookie
    // logic, all routes open — exactly today's behavior.
    if state.auth.mode == AuthMode::Disabled {
        req.extensions_mut().insert(owner_auth_user());
        return next.run(req).await;
    }

    // Public allowlist (login, health, setup, WS handshake). The SPA fallback
    // is served outside this layer (see `main.rs`), so it never reaches here.
    if is_public(req.method(), req.uri().path()) {
        return next.run(req).await;
    }

    // Everything else requires a valid session cookie.
    let Some(cookie) = jar.get(session::COOKIE_NAME) else {
        return unauthorized();
    };
    let token = cookie.value().to_string();
    let id = session::hash_token(&token);
    let Ok(Some(sess)) = state.db.session_by_id(&id) else {
        return unauthorized();
    };

    let now = Utc::now();
    let mut refreshed = false;
    match session::evaluate(&sess.created_at, &sess.last_used, &sess.expires_at, now) {
        session::SessionCheck::Invalid => {
            // Opportunistically drop the dead row (ADR-002 Decision 2).
            let _ = state.db.delete_session(&id);
            return unauthorized();
        }
        session::SessionCheck::Valid => {}
        session::SessionCheck::Refresh {
            last_used,
            expires_at,
        } => {
            let _ = state.db.touch_session(&id, &last_used, &expires_at);
            refreshed = true;
        }
    }

    let Ok(Some(user)) = state.db.user_by_id(&sess.user_id) else {
        // Session points at a user that no longer exists — treat as invalid.
        let _ = state.db.delete_session(&id);
        return unauthorized();
    };
    req.extensions_mut().insert(AuthUser {
        id: user.id,
        role: user.role,
    });

    let mut response = next.run(req).await;
    if refreshed {
        // Re-set the sliding cookie (same token, refreshed Max-Age).
        if let Ok(value) = HeaderValue::from_str(&set_cookie_value(&token, state.auth.cookie_secure))
        {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

fn owner_auth_user() -> AuthUser {
    AuthUser {
        id: OWNER_ID.to_string(),
        role: ADMIN_ROLE.to_string(),
    }
}

/// The allowlist of API routes reachable without a session (ADR-002
/// Decision 4). This is a small *positive* list of specific routes the auth
/// layer lets through — NOT a denylist of protected prefixes. The layer wraps
/// only the merged API routes (the SPA fallback lives outside it, see
/// `main.rs`), so everything the middleware sees that isn't listed here
/// requires a session — a newly-added API route is authenticated by default.
///
/// To open a new route, add it here explicitly. Keep the list minimal:
/// - `POST /auth/login` — the login endpoint itself,
/// - `GET /health` — liveness probe,
/// - `/setup*` — the one-time first-boot admin bootstrap flow,
/// - `/ws/*` — the WS handshake, which is ticket-guarded inside
///   `workspace_socket` (cookies don't propagate reliably onto a WS upgrade).
fn is_public(method: &Method, path: &str) -> bool {
    (method == Method::GET && path == "/health")
        || (method == Method::POST && path == "/auth/login")
        || path == "/setup"
        || path.starts_with("/setup/")
        || path.starts_with("/ws/")
}

fn unauthorized() -> Response {
    (StatusCode::UNAUTHORIZED, "unauthenticated").into_response()
}

// ---- Cookie helpers ----
//
// The cookie is read via `CookieJar` (extractor) but written as an explicit
// `Set-Cookie` header string so `Max-Age` (ADR-002 Decision 2) is set precisely
// without threading `time::Duration` through every call site. The token is
// base64url (URL-safe alphabet), so it needs no cookie-value escaping.

const IDLE_TTL_SECS: i64 = session::IDLE_TTL_DAYS * 24 * 60 * 60;

fn set_cookie_value(token: &str, secure: bool) -> String {
    let mut value = format!(
        "{}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={IDLE_TTL_SECS}",
        session::COOKIE_NAME
    );
    if secure {
        value.push_str("; Secure");
    }
    value
}

fn clear_cookie_value(secure: bool) -> String {
    let mut value = format!(
        "{}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        session::COOKIE_NAME
    );
    if secure {
        value.push_str("; Secure");
    }
    value
}

fn set_cookie_header(value: &str) -> [(header::HeaderName, HeaderValue); 1] {
    // `value` is entirely ASCII (name, base64url token, fixed attributes), so
    // `from_str` never fails; fall back to an empty header if it somehow does.
    let header_value = HeaderValue::from_str(value).unwrap_or_else(|_| HeaderValue::from_static(""));
    [(header::SET_COOKIE, header_value)]
}

// ---- Handlers ----

#[derive(Deserialize)]
struct LoginRequest {
    identifier: String,
    password: String,
}

#[derive(Serialize)]
struct UserResponse {
    id: String,
    email: String,
    name: String,
    role: String,
}

impl From<crate::db::User> for UserResponse {
    fn from(user: crate::db::User) -> Self {
        UserResponse {
            id: user.id,
            email: user.email,
            name: user.name,
            role: user.role,
        }
    }
}

/// `POST /auth/login { identifier, password }` — email + password, generic 401
/// on any failure, constant-time dummy-verify on unknown user (ADR-002
/// Decision 1). On success mints a session and sets the `zync_session` cookie.
async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, (StatusCode, String)> {
    // Disabled: no-op success against the synthetic owner so an auth-aware
    // frontend still boots.
    if state.auth.mode == AuthMode::Disabled {
        let user = state
            .db
            .user_by_id(OWNER_ID)
            .map_err(internal_error)?
            .ok_or_else(|| internal_error(anyhow::anyhow!("seed owner missing")))?;
        return Ok(Json(UserResponse::from(user)).into_response());
    }

    let found = state
        .db
        .user_with_hash_by_email(&request.identifier)
        .map_err(internal_error)?;
    // Wrapped so the plaintext password is wiped when the verify task's closure
    // drops it (parity with the credentials module's secret handling).
    let password = Zeroizing::new(request.password);

    // Verify on a blocking thread — argon2 is deliberately CPU/memory-heavy.
    // Always run exactly one verification (against the dummy hash when the user
    // is unknown or un-bootstrapped) so timing can't distinguish the cases.
    let verified = tokio::task::spawn_blocking(move || {
        let (hash, user) = match found {
            Some(uwh) => (uwh.password_hash, Some(uwh.user)),
            None => (None, None),
        };
        let hash = hash.unwrap_or_else(|| password::dummy_hash().to_string());
        if password::verify_password(&password, &hash) {
            user
        } else {
            None
        }
    })
    .await
    .map_err(|e| internal_error(anyhow::anyhow!("password verify task failed: {e}")))?;

    let Some(user) = verified else {
        return Err((StatusCode::UNAUTHORIZED, "invalid credentials".to_string()));
    };

    let token = session::generate_token();
    let id = session::hash_token(&token);
    let ts = session::new_session_timestamps(Utc::now());
    state
        .db
        .create_session(&id, &user.id, &ts.created_at, &ts.last_used, &ts.expires_at)
        .map_err(internal_error)?;

    let headers = set_cookie_header(&set_cookie_value(&token, state.auth.cookie_secure));
    Ok((headers, Json(UserResponse::from(user))).into_response())
}

/// `POST /auth/logout` — deletes the session row (read from the cookie, not the
/// body) and clears the cookie. No-op success in `disabled` mode.
async fn logout(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Response, (StatusCode, String)> {
    if state.auth.mode == AuthMode::Disabled {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    if let Some(cookie) = jar.get(session::COOKIE_NAME) {
        let id = session::hash_token(cookie.value());
        state.db.delete_session(&id).map_err(internal_error)?;
    }
    let headers = set_cookie_header(&clear_cookie_value(state.auth.cookie_secure));
    Ok((headers, StatusCode::NO_CONTENT).into_response())
}

/// `GET /auth/me` — the current user (id, email, name, role) or 401.
async fn me(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    let user = state
        .db
        .user_by_id(&auth.id)
        .map_err(internal_error)?
        .ok_or((StatusCode::UNAUTHORIZED, "unauthenticated".to_string()))?;
    Ok(Json(UserResponse::from(user)))
}

#[derive(Deserialize)]
struct WsTicketRequest {
    workspace_id: String,
}

#[derive(Serialize)]
struct WsTicketResponse {
    ticket: String,
}

/// `POST /auth/ws-ticket { workspace_id }` — mint a short-lived single-use
/// ticket bound to `(user, workspace)` for the WS handshake (ADR-002 Decision 4).
///
/// Closes the P3.2 review's W1 IDOR: the ticket carries `workspace_id` in the
/// request *body* (not a path segment), so the repo-scope authz guard — which
/// keys off the URL — can't cover it. We therefore check membership here: the
/// caller must be a member (any role) of the requested workspace's repository,
/// or a global admin, before a ticket is minted. A non-member gets `403` and no
/// ticket, so they can never reach the (otherwise ticket-only) WS handshake.
async fn ws_ticket(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(request): Json<WsTicketRequest>,
) -> Result<Json<WsTicketResponse>, (StatusCode, String)> {
    if auth.role != ADMIN_ROLE {
        let workspace = state
            .db
            .workspace(&request.workspace_id)
            .map_err(internal_error)?
            .ok_or((StatusCode::NOT_FOUND, "workspace not found".to_string()))?;
        let role = state
            .db
            .repo_role_for_user(&workspace.repository_id, &auth.id)
            .map_err(internal_error)?;
        if role.is_none() {
            return Err((
                StatusCode::FORBIDDEN,
                "not authorized for this workspace".to_string(),
            ));
        }
    }
    let ticket = state.auth.tickets.mint(&auth.id, &request.workspace_id);
    Ok(Json(WsTicketResponse { ticket }))
}

// ---- Admin user provisioning (P3.5) ----

#[derive(Deserialize)]
struct CreateUserRequest {
    /// The new user's login email.
    identifier: String,
    password: String,
    name: Option<String>,
    /// Defaults to `"user"`. Must be `"admin"` or `"user"`.
    role: Option<String>,
}

fn validate_global_role(role: &str) -> Result<(), (StatusCode, String)> {
    match role {
        "admin" | "user" => Ok(()),
        _ => Err((
            StatusCode::BAD_REQUEST,
            "role must be 'admin' or 'user'".to_string(),
        )),
    }
}

/// `POST /auth/users { identifier, password, name?, role? }` — admin-only user
/// provisioning (ADR-002 Decision 1: "User creation is admin-only"; P3.5).
/// `identifier` is the new user's login email; the password is argon2id-hashed
/// (reusing the login/bootstrap `password` module) before it ever reaches the
/// DB. Returns the created user — never the password hash.
async fn create_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    if auth.role != ADMIN_ROLE {
        return Err((StatusCode::FORBIDDEN, "admin role required".to_string()));
    }
    let identifier = request.identifier.trim();
    if identifier.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "identifier is required".to_string(),
        ));
    }
    if request.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "password is required".to_string()));
    }
    let role = request.role.as_deref().unwrap_or("user");
    validate_global_role(role)?;
    if state
        .db
        .find_user_by_identifier(identifier)
        .map_err(internal_error)?
        .is_some()
    {
        return Err((
            StatusCode::CONFLICT,
            "a user with that identifier already exists".to_string(),
        ));
    }
    let name = request
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(identifier)
        .to_string();

    // Hash on a blocking thread — argon2 is deliberately CPU/memory-heavy, same
    // posture as login/bootstrap (ADR-002 Decision 1).
    let password = Zeroizing::new(request.password);
    let hash = tokio::task::spawn_blocking(move || password::hash_password(&password))
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("password hash task failed: {e}")))?
        .map_err(internal_error)?;

    let id = uuid::Uuid::new_v4().to_string();
    if let Err(err) = state
        .db
        .create_user_with_password(&id, identifier, &name, role, &hash)
    {
        if err.downcast_ref::<crate::db::UserConflict>().is_some() {
            return Err((
                StatusCode::CONFLICT,
                "a user with that identifier already exists".to_string(),
            ));
        }
        tracing::error!("failed to create user: {err:#}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create user".to_string(),
        ));
    }
    let user = state
        .db
        .user_by_id(&id)
        .map_err(internal_error)?
        .ok_or_else(|| internal_error(anyhow::anyhow!("created user vanished")))?;
    Ok(Json(UserResponse::from(user)))
}

/// `GET /auth/users` — admin-only list of every user (id/email/name/role/
/// created_at), never `password_hash` (P3.5).
async fn list_users(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Vec<crate::db::UserSummary>>, (StatusCode, String)> {
    if auth.role != ADMIN_ROLE {
        return Err((StatusCode::FORBIDDEN, "admin role required".to_string()));
    }
    state.db.list_users().map(Json).map_err(internal_error)
}

// ---- First-boot setup token flow ----

#[derive(Deserialize)]
struct SetupRequest {
    token: String,
    identifier: String,
    password: String,
}

async fn setup_get() -> Response {
    (
        StatusCode::OK,
        "POST /setup with JSON { token, identifier, password } to set the initial admin password.",
    )
        .into_response()
}

/// `POST /setup { token, identifier, password }` — consume the one-time setup
/// token and set the admin password. Only works while the server is
/// un-bootstrapped (ADR-002 Decision 1).
async fn setup_post(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SetupRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !state.auth.setup_token_valid(&request.token, Utc::now()) {
        return Err((
            StatusCode::FORBIDDEN,
            "invalid or expired setup token".to_string(),
        ));
    }
    if state.db.any_password_set().map_err(internal_error)? {
        return Err((
            StatusCode::CONFLICT,
            "server is already bootstrapped".to_string(),
        ));
    }
    if request.identifier.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "identifier is required".to_string()));
    }
    if request.password.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "password is required".to_string()));
    }

    let password = Zeroizing::new(request.password);
    let hash = tokio::task::spawn_blocking(move || password::hash_password(&password))
        .await
        .map_err(|e| internal_error(anyhow::anyhow!("password hash task failed: {e}")))?
        .map_err(internal_error)?;
    state
        .db
        .set_admin_password(request.identifier.trim(), &hash)
        .map_err(internal_error)?;
    state.auth.clear_setup_token();
    tracing::info!("admin bootstrapped via /setup token");
    Ok(StatusCode::NO_CONTENT)
}

// ---- Bootstrap + background sweep ----

/// First-boot admin bootstrap (ADR-002 Decision 1). No-op once any admin
/// password exists, or in `disabled` mode. Env path (`ZYNC_ADMIN_USER` +
/// `ZYNC_ADMIN_PASSWORD`) is preferred; otherwise a one-time `/setup?token=…`
/// link is logged.
pub async fn bootstrap(state: &AppState) -> anyhow::Result<()> {
    if state.auth.mode == AuthMode::Disabled {
        return Ok(());
    }
    // Pre-warm the dummy hash off the async runtime so the first unknown-user
    // login doesn't pay a one-time extra argon2 hash — login timing stays
    // uniform from the very first request (ADR-002 Decision 1).
    tokio::task::spawn_blocking(|| {
        let _ = password::dummy_hash();
    })
    .await
    .ok();

    if state.db.any_password_set()? {
        return Ok(());
    }

    let admin_user = non_empty_env("ZYNC_ADMIN_USER");
    let admin_password = non_empty_env("ZYNC_ADMIN_PASSWORD").map(Zeroizing::new);
    match (admin_user, admin_password) {
        (Some(user), Some(password)) => {
            let hash = tokio::task::spawn_blocking(move || password::hash_password(&password))
                .await
                .map_err(|e| anyhow::anyhow!("password hash task failed: {e}"))??;
            state.db.set_admin_password(&user, &hash)?;
            tracing::info!("bootstrapped admin '{user}' from ZYNC_ADMIN_USER/ZYNC_ADMIN_PASSWORD");
        }
        _ => {
            let token = session::generate_token();
            let expires_at = Utc::now() + Duration::hours(24);
            state.auth.set_setup_token(token.clone(), expires_at);
            tracing::warn!(
                "no admin configured. Set the initial admin password within 24h via \
                 /setup?token={token} (POST /setup with {{ token, identifier, password }}), or set \
                 ZYNC_ADMIN_USER/ZYNC_ADMIN_PASSWORD and restart."
            );
        }
    }
    Ok(())
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Background task: sweep expired sessions every ~30 min (ADR-002 Decision 2)
/// so dead rows don't accumulate.
pub fn spawn_session_sweeper(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            let now = session::format_ts(Utc::now());
            match state.db.sweep_expired_sessions(&now) {
                Ok(count) if count > 0 => tracing::debug!("swept {count} expired sessions"),
                Ok(_) => {}
                Err(error) => tracing::warn!("session sweep failed: {error}"),
            }
        }
    });
}

/// Constant-time string comparison for the setup token (avoids a timing oracle
/// on the one-time bootstrap secret). Both operands are ASCII here.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use axum::{body::Body, routing::get, Router};
    use tower::ServiceExt; // oneshot

    fn app_state(mode: AuthMode) -> Arc<AppState> {
        Arc::new(AppState {
            db: Database::open(":memory:").expect("open in-memory db"),
            hub: crate::websocket::WorkspaceHub::default(),
            sync: crate::sync::WorkspaceSync::default(),
            collaboration: crate::collaboration::CollaborationState::default(),
            secrets: crate::crypto::KeyState::Unconfigured,
            auth: AuthState {
                mode,
                cookie_secure: false,
                tickets: ticket::WsTicketStore::default(),
                setup: Arc::new(Mutex::new(None)),
            },
            repos_root: crate::repos_root::ReposRoot::default(),
        })
    }

    /// A minimal router that mirrors production wiring: a protected `/repositories`
    /// route behind the `require_auth` layer.
    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/repositories", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn protected_route_without_cookie_is_401() {
        let state = app_state(AuthMode::Enabled);
        let app = test_app(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/repositories")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn protected_route_with_valid_session_is_200() {
        let state = app_state(AuthMode::Enabled);
        // Seed a live session for the owner.
        let token = session::generate_token();
        let id = session::hash_token(&token);
        let ts = session::new_session_timestamps(Utc::now());
        state
            .db
            .create_session(&id, OWNER_ID, &ts.created_at, &ts.last_used, &ts.expires_at)
            .expect("create session");

        let app = test_app(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/repositories")
                    .header(header::COOKIE, format!("{}={token}", session::COOKIE_NAME))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn expired_session_cookie_is_401() {
        let state = app_state(AuthMode::Enabled);
        let token = session::generate_token();
        let id = session::hash_token(&token);
        // Already-expired session.
        let created = session::format_ts(Utc::now() - Duration::days(10));
        let last = session::format_ts(Utc::now() - Duration::days(9));
        let expires = session::format_ts(Utc::now() - Duration::days(1));
        state
            .db
            .create_session(&id, OWNER_ID, &created, &last, &expires)
            .expect("create session");

        let app = test_app(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/repositories")
                    .header(header::COOKIE, format!("{}={token}", session::COOKIE_NAME))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        // The dead row was opportunistically swept.
        assert!(state.db.session_by_id(&id).unwrap().is_none());
    }

    #[tokio::test]
    async fn disabled_mode_allows_protected_route_without_cookie() {
        let state = app_state(AuthMode::Disabled);
        let app = test_app(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/repositories")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn public_allowlist_is_a_minimal_positive_list() {
        // The four explicitly-open route classes.
        assert!(is_public(&Method::GET, "/health"));
        assert!(is_public(&Method::POST, "/auth/login"));
        assert!(is_public(&Method::GET, "/setup"));
        assert!(is_public(&Method::POST, "/setup"));
        assert!(is_public(&Method::GET, "/ws/workspace/abc"));
        // Everything else the middleware sees requires a session — including
        // any API route not on the list. (`/` and `/assets/*` are public in
        // production not because `is_public` returns true, but because the SPA
        // fallback is served *outside* the auth layer — see the router-structure
        // test below.)
        assert!(!is_public(&Method::GET, "/"));
        assert!(!is_public(&Method::GET, "/assets/index.js"));
        assert!(!is_public(&Method::GET, "/repositories"));
        assert!(!is_public(&Method::GET, "/auth/me"));
        assert!(!is_public(&Method::POST, "/auth/logout"));
        assert!(!is_public(&Method::POST, "/credentials"));
        assert!(!is_public(&Method::GET, "/workspace/abc"));
        // A hypothetical future route under a brand-new prefix: closed by
        // default (the old inverted denylist would have leaked this as public).
        assert!(!is_public(&Method::GET, "/brand-new-feature"));
        assert!(!is_public(&Method::POST, "/brand-new-feature/action"));
    }

    /// W2 guard (P3.2 security review): locks in the closed-by-default routing
    /// shape. The auth layer wraps the API routes; the SPA fallback sits outside
    /// it. So (a) any wrapped route that isn't allowlisted rejects an
    /// unauthenticated request, and (b) unmatched paths fall through to the
    /// public SPA fallback. A new API route added inside the layer is protected
    /// automatically — there is no prefix denylist to keep in sync.
    #[tokio::test]
    async fn spa_fallback_is_public_but_api_routes_are_closed_by_default() {
        let state = app_state(AuthMode::Enabled);
        // Mirror production wiring: routes wrapped by auth, fallback outside it.
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/repositories", get(|| async { "ok" }))
            // A route with no allowlist entry stands in for "some future API
            // route someone forgot to think about" — it must still be protected.
            .route("/brand-new-feature", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .fallback(|| async { "spa-index" })
            .with_state(state);

        let get = |uri: &str| {
            app.clone().oneshot(
                Request::builder()
                    .uri(uri.to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
        };

        // Allowlisted route → public.
        assert_eq!(get("/health").await.unwrap().status(), StatusCode::OK);
        // Wrapped, non-allowlisted API routes → 401 without a session, even a
        // brand-new one nobody added to any prefix list.
        assert_eq!(
            get("/repositories").await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            get("/brand-new-feature").await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        // Unmatched path → served by the public SPA fallback (outside the layer).
        assert_eq!(
            get("/some/client/route").await.unwrap().status(),
            StatusCode::OK
        );
    }

    /// W1 IDOR (P3.2 review): `ws_ticket` must refuse to mint a ticket for a
    /// workspace whose repository the caller isn't a member of. Otherwise any
    /// authenticated user could mint a valid `(self, someone-else's-workspace)`
    /// ticket and ride it onto the otherwise-guarded WS stream.
    #[tokio::test]
    async fn ws_ticket_requires_workspace_membership() {
        let state = app_state(AuthMode::Enabled);
        // A repo owned by `bob`, with `mem` added as a member; `out` is a
        // stranger. `owner` (seeded) is a global admin.
        state
            .db
            .create_user("bob", "bob@zync.local", "Bob", "user")
            .unwrap();
        state
            .db
            .create_user("mem", "mem@zync.local", "Mem", "user")
            .unwrap();
        state
            .db
            .create_user("out", "out@zync.local", "Out", "user")
            .unwrap();
        let repo = state
            .db
            .create_repository("proj", "/tmp/proj", None, "bob")
            .unwrap();
        let workspace = state
            .db
            .workspace_for_repository(&repo.id, &repo.name)
            .unwrap();
        state.db.add_repo_member(&repo.id, "mem", "member").unwrap();

        let mint = |user: &str| {
            let state = state.clone();
            let ws = workspace.id.clone();
            let user = user.to_string();
            async move {
                ws_ticket(
                    State(state),
                    AuthUser {
                        id: user,
                        role: "user".to_string(),
                    },
                    Json(WsTicketRequest { workspace_id: ws }),
                )
                .await
            }
        };

        // Owner (bob) and a member (mem) can mint.
        assert!(mint("bob").await.is_ok());
        assert!(mint("mem").await.is_ok());
        // A non-member is forbidden — no ticket.
        let err = mint("out").await.err().expect("non-member must be rejected");
        assert_eq!(err.0, StatusCode::FORBIDDEN);

        // A global admin bypasses the membership check.
        let admin = ws_ticket(
            State(state.clone()),
            AuthUser {
                id: "owner".to_string(),
                role: ADMIN_ROLE.to_string(),
            },
            Json(WsTicketRequest {
                workspace_id: workspace.id.clone(),
            }),
        )
        .await;
        assert!(admin.is_ok());
    }

    /// A ticket for a workspace that doesn't exist is a `404`, not a `403` —
    /// there's no repo to check membership against.
    #[tokio::test]
    async fn ws_ticket_for_unknown_workspace_is_404() {
        let state = app_state(AuthMode::Enabled);
        let err = ws_ticket(
            State(state),
            AuthUser {
                id: "bob".to_string(),
                role: "user".to_string(),
            },
            Json(WsTicketRequest {
                workspace_id: "no-such-ws".to_string(),
            }),
        )
        .await
        .err()
        .expect("unknown workspace must be rejected");
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    // ---- Admin user provisioning (P3.5) ----

    fn user_auth(id: &str, role: &str) -> AuthUser {
        AuthUser {
            id: id.to_string(),
            role: role.to_string(),
        }
    }

    #[tokio::test]
    async fn create_user_requires_admin_role() {
        let state = app_state(AuthMode::Enabled);
        let request = CreateUserRequest {
            identifier: "new@zync.local".to_string(),
            password: "correct horse battery staple".to_string(),
            name: None,
            role: None,
        };
        let err = create_user(State(state), user_auth("bob", "user"), Json(request))
            .await
            .err()
            .expect("non-admin must be rejected");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_users_requires_admin_role() {
        let state = app_state(AuthMode::Enabled);
        let err = list_users(State(state), user_auth("bob", "user"))
            .await
            .err()
            .expect("non-admin must be rejected");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    /// The full loop the P3.5 review will check: an admin provisions a user
    /// (identifier/password/name/role), the new user shows up in the admin
    /// list without a password hash anywhere in sight, and the created user
    /// can immediately log in with the password the admin set.
    #[tokio::test]
    async fn admin_creates_lists_and_new_user_can_log_in() {
        let state = app_state(AuthMode::Enabled);
        let admin = user_auth(OWNER_ID, ADMIN_ROLE);

        let request = CreateUserRequest {
            identifier: "new@zync.local".to_string(),
            password: "correct horse battery staple".to_string(),
            name: Some("New User".to_string()),
            role: Some("user".to_string()),
        };
        let created = create_user(State(state.clone()), admin.clone(), Json(request))
            .await
            .expect("admin can create a user");
        assert_eq!(created.email, "new@zync.local");
        assert_eq!(created.name, "New User");
        assert_eq!(created.role, "user");

        let users = list_users(State(state.clone()), admin.clone())
            .await
            .expect("admin can list users");
        assert!(
            users.iter().any(|u| u.email == "new@zync.local"),
            "created user is listed"
        );

        // The new user authenticates with the password the admin set.
        let login_result = login(
            State(state.clone()),
            Json(LoginRequest {
                identifier: "new@zync.local".to_string(),
                password: "correct horse battery staple".to_string(),
            }),
        )
        .await;
        assert!(
            login_result.is_ok(),
            "the admin-created user can log in with its initial password"
        );
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_identifier() {
        let state = app_state(AuthMode::Enabled);
        let admin = user_auth(OWNER_ID, ADMIN_ROLE);
        let request = || CreateUserRequest {
            identifier: "dup@zync.local".to_string(),
            password: "correct horse battery staple".to_string(),
            name: None,
            role: None,
        };
        let _ = create_user(State(state.clone()), admin.clone(), Json(request()))
            .await
            .expect("first create succeeds");
        let err = create_user(State(state.clone()), admin, Json(request()))
            .await
            .err()
            .expect("duplicate identifier must be rejected");
        assert_eq!(err.0, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn create_user_rejects_invalid_role() {
        let state = app_state(AuthMode::Enabled);
        let admin = user_auth(OWNER_ID, ADMIN_ROLE);
        let request = CreateUserRequest {
            identifier: "weird-role@zync.local".to_string(),
            password: "correct horse battery staple".to_string(),
            name: None,
            role: Some("superuser".to_string()),
        };
        let err = create_user(State(state), admin, Json(request))
            .await
            .err()
            .expect("invalid role must be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    /// P4.3 closing-pass regression test: a failed login must never echo the submitted password
    /// (or any detail derived from it) back in the response body — `login` already returns a
    /// fixed, generic `"invalid credentials"` string on any failure (ADR-002 Decision 1), but this
    /// pins that behavior against a known sentinel so a future refactor that starts interpolating
    /// verification detail into the error would be caught here.
    #[tokio::test]
    async fn failed_login_never_echoes_submitted_password() {
        const SENTINEL: &str = "SENTINEL_SECRET_bkq9";
        let state = app_state(AuthMode::Enabled);

        // Unknown user: still runs the dummy-hash verify (timing parity), then fails.
        let err = login(
            State(state.clone()),
            Json(LoginRequest {
                identifier: "nobody@zync.local".to_string(),
                password: SENTINEL.to_string(),
            }),
        )
        .await
        .err()
        .expect("login with an unknown identifier must fail");
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert!(!err.1.contains(SENTINEL), "error body leaked the password: {}", err.1);

        // Known user, wrong password.
        let admin = user_auth(OWNER_ID, ADMIN_ROLE);
        let _ = create_user(
            State(state.clone()),
            admin,
            Json(CreateUserRequest {
                identifier: "real@zync.local".to_string(),
                password: "correct horse battery staple".to_string(),
                name: None,
                role: None,
            }),
        )
        .await
        .expect("create user");

        let err = login(
            State(state.clone()),
            Json(LoginRequest {
                identifier: "real@zync.local".to_string(),
                password: SENTINEL.to_string(),
            }),
        )
        .await
        .err()
        .expect("login with the wrong password must fail");
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert!(!err.1.contains(SENTINEL), "error body leaked the password: {}", err.1);
    }

    #[test]
    fn setup_token_lifecycle() {
        let state = AuthState {
            mode: AuthMode::Enabled,
            cookie_secure: true,
            tickets: ticket::WsTicketStore::default(),
            setup: Arc::new(Mutex::new(None)),
        };
        let now = Utc::now();
        state.set_setup_token("secret-token".to_string(), now + Duration::hours(1));
        assert!(state.setup_token_valid("secret-token", now));
        assert!(!state.setup_token_valid("wrong-token", now));
        // Expired.
        assert!(!state.setup_token_valid("secret-token", now + Duration::hours(2)));
        // Cleared.
        state.clear_setup_token();
        assert!(!state.setup_token_valid("secret-token", now));
    }
}
