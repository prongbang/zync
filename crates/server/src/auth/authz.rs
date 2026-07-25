//! Repo-scope authorization guard — ADR-002 Decision 5.
//!
//! One middleware layer, sitting *inside* [`super::require_auth`] (so it runs
//! after authentication has populated the request's [`AuthUser`]), enforces
//! per-repository access on every repo-scoped route. It classifies the request
//! URL into a repository and a required capability, resolves the caller's
//! effective role on that repo, and rejects with `403` when the role is
//! insufficient — distinct from the `401` `require_auth` returns for "not
//! logged in".
//!
//! Why a central middleware rather than a per-handler check: the git router
//! alone has ~60 repo-scoped routes, all keyed on the same `:id` path segment.
//! A single guard over the merged routes is closed-by-default — a newly-added
//! `/repositories/:id/...` or `/workspace/:id/...` route is authorized
//! automatically — and keeps the whole policy in one auditable place.
//!
//! Method → capability (the ADR's rule): safe methods (`GET`/`HEAD`/`OPTIONS`)
//! need `viewer+`; mutating methods need `member+`. Two explicit carve-outs
//! override the method default: the `/repositories/:id/members*` subtree and
//! `DELETE /repositories/:id` are `owner`-only, and `POST /repositories/:id/open`
//! is a deliberate read-only-`POST` that a `viewer` may call (opening is how a
//! viewer starts viewing). A global `admin` bypasses every check.

use crate::auth::{AuthUser, ADMIN_ROLE};
use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Repo-scoped capability levels, ordered by privilege (`Viewer < Member <
/// Owner` — the derived `Ord` follows declaration order).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Capability {
    Viewer,
    Member,
    Owner,
}

/// What a classified request touches.
#[derive(Debug, PartialEq, Eq)]
enum Scope {
    /// `/repositories/{id}/...` — the repository id is in the path directly.
    Repo { id: String, required: Capability },
    /// `/workspace/{id}/...` — needs a workspace→repository lookup to find the
    /// repo the capability is checked against.
    Workspace { id: String, required: Capability },
}

/// The repo-scope authorization middleware. Non-repo-scoped requests (auth,
/// credentials, health, the SPA, `POST`/`GET /repositories` themselves) pass
/// straight through — those are guarded elsewhere (their handlers use
/// `auth.id`, or they're intentionally public).
pub async fn require_repo_authz(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let Some(scope) = classify(&method, &path) else {
        return next.run(req).await;
    };

    // `require_auth` (the outer layer) guarantees an `AuthUser` on every
    // non-public path, and repo-scoped paths are never public — but fail closed
    // if it is somehow absent rather than trusting an unauthenticated request.
    let Some(auth) = req.extensions().get::<AuthUser>().cloned() else {
        return (StatusCode::UNAUTHORIZED, "unauthenticated").into_response();
    };

    // Global admins bypass repo-scope checks entirely. This is also the
    // `disabled`-mode path: `require_auth` injects a synthetic `admin` there, so
    // disabled mode keeps full, unchecked access (no regression).
    if auth.role == ADMIN_ROLE {
        return next.run(req).await;
    }

    let (repository_id, required) = match scope {
        Scope::Repo { id, required } => (id, required),
        Scope::Workspace { id, required } => match state.db.workspace(&id) {
            Ok(Some(workspace)) => (workspace.repository_id, required),
            Ok(None) => return (StatusCode::NOT_FOUND, "workspace not found").into_response(),
            Err(_) => return internal_error(),
        },
    };

    // Distinguish a genuinely missing repo (`404`) from a real repo the caller
    // has no role on (`403`), matching the handlers' own not-found behavior.
    match state.db.repository(&repository_id) {
        Ok(Some(_)) => {}
        Ok(None) => return (StatusCode::NOT_FOUND, "repository not found").into_response(),
        Err(_) => return internal_error(),
    }

    let granted = match state.db.repo_role_for_user(&repository_id, &auth.id) {
        Ok(role) => role.as_deref().and_then(role_capability),
        Err(_) => return internal_error(),
    };

    match granted {
        Some(capability) if capability >= required => next.run(req).await,
        _ => (
            StatusCode::FORBIDDEN,
            "not authorized for this repository",
        )
            .into_response(),
    }
}

/// Classify a request URL + method into the repository (or workspace) it
/// touches and the capability it requires. `None` means "not repo-scoped" —
/// the middleware passes those through untouched.
fn classify(method: &Method, path: &str) -> Option<Scope> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        // `/repositories` (list/create) has no id segment and is intentionally
        // not matched here — the list handler filters by membership and create
        // sets ownership. Only `/repositories/{id}...` is repo-scoped.
        ["repositories", id, rest @ ..] => Some(Scope::Repo {
            id: (*id).to_string(),
            required: repo_capability(method, rest),
        }),
        ["workspace", id, ..] => Some(Scope::Workspace {
            id: (*id).to_string(),
            required: method_capability(method),
        }),
        _ => None,
    }
}

/// Required capability for a `/repositories/{id}/{rest...}` request.
fn repo_capability(method: &Method, rest: &[&str]) -> Capability {
    match rest {
        // Member management — owner/admin only, whatever the method.
        ["members", ..] => Capability::Owner,
        // `DELETE /repositories/{id}` (repo delete) — owner only.
        [] if method == Method::DELETE => Capability::Owner,
        // `POST /repositories/{id}/open` — the ADR's explicit read-only-`POST`
        // carve-out: opening a repo is how a viewer starts viewing it.
        ["open"] => Capability::Viewer,
        // Everything else follows the method → capability rule.
        _ => method_capability(method),
    }
}

/// The default method → capability rule: safe methods read (`viewer+`),
/// mutating methods write (`member+`).
fn method_capability(method: &Method) -> Capability {
    if is_safe(method) {
        Capability::Viewer
    } else {
        Capability::Member
    }
}

/// RFC 7231 safe methods — the only ones the git/workspace routers use for
/// reads. The read/write split is *test-enforced* (see
/// `every_mutating_git_route_uses_a_non_safe_method`): the method-based default
/// is sound only while no mutating route hides behind a safe method.
fn is_safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Map a repo-scoped role string to its capability level; unknown roles map to
/// `None` (no access).
fn role_capability(role: &str) -> Option<Capability> {
    match role {
        "owner" => Some(Capability::Owner),
        "member" => Some(Capability::Member),
        "viewer" => Some(Capability::Viewer),
        _ => None,
    }
}

fn internal_error() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{require_auth, session, AuthMode, AuthState, OWNER_ID};
    use crate::db::Database;
    use axum::{body::Body, http::header, routing, Router};
    use chrono::Utc;
    use std::sync::Mutex;
    use tower::ServiceExt; // oneshot

    // ---- classification unit tests ----

    fn get(path: &str) -> Option<Scope> {
        classify(&Method::GET, path)
    }
    fn post(path: &str) -> Option<Scope> {
        classify(&Method::POST, path)
    }

    #[test]
    fn non_repo_scoped_paths_are_not_classified() {
        assert!(get("/repositories").is_none());
        assert!(post("/repositories").is_none());
        assert!(get("/directories").is_none());
        assert!(get("/credentials").is_none());
        assert!(post("/auth/ws-ticket").is_none());
        assert!(get("/health").is_none());
    }

    #[test]
    fn git_reads_need_viewer_and_writes_need_member() {
        assert_eq!(
            get("/repositories/r1/git/status"),
            Some(Scope::Repo {
                id: "r1".into(),
                required: Capability::Viewer
            })
        );
        assert_eq!(
            post("/repositories/r1/git/commit"),
            Some(Scope::Repo {
                id: "r1".into(),
                required: Capability::Member
            })
        );
        // A representative sweep of mutating git verbs — all must be member+.
        for path in [
            "/repositories/r1/git/push",
            "/repositories/r1/git/pull",
            "/repositories/r1/git/branches/merge",
            "/repositories/r1/git/rebase/interactive",
            "/repositories/r1/git/reset",
            "/repositories/r1/git/stashes/drop",
        ] {
            assert_eq!(
                post(path).map(|s| capability_of(&s)),
                Some(Capability::Member),
                "{path} should require member"
            );
        }
    }

    #[test]
    fn repo_delete_and_members_are_owner_only() {
        assert_eq!(
            classify(&Method::DELETE, "/repositories/r1"),
            Some(Scope::Repo {
                id: "r1".into(),
                required: Capability::Owner
            })
        );
        for (method, path) in [
            (Method::GET, "/repositories/r1/members"),
            (Method::POST, "/repositories/r1/members"),
            (Method::PUT, "/repositories/r1/members/u2"),
            (Method::DELETE, "/repositories/r1/members/u2"),
        ] {
            assert_eq!(
                classify(&method, path).map(|s| capability_of(&s)),
                Some(Capability::Owner),
                "{method} {path} should be owner-only"
            );
        }
    }

    #[test]
    fn open_is_a_viewer_readonly_post() {
        assert_eq!(
            post("/repositories/r1/open"),
            Some(Scope::Repo {
                id: "r1".into(),
                required: Capability::Viewer
            })
        );
    }

    #[test]
    fn favorite_put_needs_member() {
        assert_eq!(
            classify(&Method::PUT, "/repositories/r1/favorite"),
            Some(Scope::Repo {
                id: "r1".into(),
                required: Capability::Member
            })
        );
    }

    #[test]
    fn workspace_reads_and_writes_follow_the_method_rule() {
        assert_eq!(
            get("/workspace/w1"),
            Some(Scope::Workspace {
                id: "w1".into(),
                required: Capability::Viewer
            })
        );
        assert_eq!(
            put_scope("/workspace/w1/files/src/main.rs"),
            Some(Capability::Member)
        );
        assert_eq!(
            post("/workspace/w1/files").map(|s| capability_of(&s)),
            Some(Capability::Member)
        );
    }

    /// The ADR's test-enforced invariant, made concrete: "the set of routes
    /// calling `broadcast_git_change` == the set the guard treats as writes."
    ///
    /// This is NOT a tautology over `method_capability` — it enumerates the
    /// ACTUAL git routes from source (compile-time `include_str!` of
    /// `git/mod.rs`), determines each route's real mutation behavior (a git
    /// handler mutates iff its body calls `broadcast_git_change`), runs each
    /// through the real `classify`, and asserts the guard's write-classification
    /// matches the mutation reality exactly. So:
    ///   - a future mutating `GET` (handler broadcasts, but reachable by a
    ///     viewer) fails here — the security-critical direction;
    ///   - a non-safe route that forgets to broadcast also fails — documenting
    ///     the "every mutation broadcasts" contract from CLAUDE.md.
    #[test]
    fn git_write_classification_matches_broadcast_call_sites() {
        const GIT_SRC: &str = include_str!("../git/mod.rs");
        let routes = parse_routes(GIT_SRC);
        assert!(
            routes.len() >= 70,
            "sanity: expected the full git route table (~75 method entries), parsed {}",
            routes.len()
        );

        for (method, path, handler) in &routes {
            let mutates = handler_calls_broadcast(GIT_SRC, handler);
            let capability = match classify(method, path) {
                Some(Scope::Repo { required, .. }) => required,
                other => panic!("git route `{method} {path}` did not classify as repo-scoped: {other:?}"),
            };
            let treated_as_write = capability >= Capability::Member;
            assert_eq!(
                treated_as_write, mutates,
                "git route `{method} {path}` (handler `{handler}`): guard treats it as \
                 {}, but it {} `broadcast_git_change`. A mutation MUST use a non-safe \
                 method (member+); a member+ git route MUST broadcast.",
                if treated_as_write { "a WRITE (member+)" } else { "a READ (viewer)" },
                if mutates { "DOES call" } else { "does NOT call" },
            );
        }
    }

    /// Extract `(method, path, handler)` triples from a module's `routes()` fn.
    /// Deliberately source-parsing (the axum `Router` isn't introspectable) but
    /// scoped to the balanced `routes()` body so handler code below can't leak
    /// false matches.
    fn parse_routes(src: &str) -> Vec<(Method, String, String)> {
        let body = extract_routes_body(src);
        let mut out = Vec::new();
        for segment in body.split(".route(").skip(1) {
            let path = first_quoted(segment).expect("route path string");
            for (keyword, method) in [
                ("get(", Method::GET),
                ("post(", Method::POST),
                ("put(", Method::PUT),
                ("delete(", Method::DELETE),
            ] {
                let mut rest = segment;
                while let Some(index) = rest.find(keyword) {
                    let after = &rest[index + keyword.len()..];
                    let handler: String = after
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !handler.is_empty() {
                        out.push((method.clone(), path.clone(), handler));
                    }
                    rest = after;
                }
            }
        }
        out
    }

    /// The balanced `{ ... }` body of the `pub fn routes()` fn.
    fn extract_routes_body(src: &str) -> &str {
        let start = src.find("pub fn routes()").expect("routes() fn");
        let open = src[start..].find('{').expect("routes() open brace") + start;
        let bytes = src.as_bytes();
        let mut depth = 0i32;
        for i in open..src.len() {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &src[open..=i];
                    }
                }
                _ => {}
            }
        }
        panic!("unbalanced routes() body");
    }

    /// Whether the `async fn <handler>` body (up to the next `async fn`) calls
    /// `broadcast_git_change` — the git module's one mutation signal.
    fn handler_calls_broadcast(src: &str, handler: &str) -> bool {
        let signature = format!("async fn {handler}(");
        let Some(start) = src.find(&signature) else {
            return false;
        };
        let after = &src[start + signature.len()..];
        let end = after.find("\nasync fn ").unwrap_or(after.len());
        after[..end].contains("broadcast_git_change")
    }

    fn first_quoted(segment: &str) -> Option<String> {
        let open = segment.find('"')?;
        let rest = &segment[open + 1..];
        let close = rest.find('"')?;
        Some(rest[..close].to_string())
    }

    fn capability_of(scope: &Scope) -> Capability {
        match scope {
            Scope::Repo { required, .. } | Scope::Workspace { required, .. } => *required,
        }
    }
    fn put_scope(path: &str) -> Option<Capability> {
        classify(&Method::PUT, path).map(|s| capability_of(&s))
    }

    // ---- authorization-matrix integration tests ----
    //
    // These drive the real middleware stack (require_auth → require_repo_authz)
    // over stub handlers at the production paths. Stubs isolate the authz
    // decision from git/fs logic: a 403 never reaches the handler, and an
    // allowed request returns the stub's 200. The DB carries real users, a repo
    // owned by a normal (non-admin) user, and viewer/member/non-member rows.

    struct Fixture {
        state: Arc<AppState>,
        repo_id: String,
        workspace_id: String,
    }

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
                tickets: crate::auth::ticket::WsTicketStore::default(),
                setup: Arc::new(Mutex::new(None)),
            },
            repos_root: crate::repos_root::ReposRoot::default(),
            metrics: Arc::new(crate::observability::Metrics::default()),
        })
    }

    /// Seed a live session for `user_id` and return the raw cookie token.
    fn session_for(state: &AppState, user_id: &str) -> String {
        let token = session::generate_token();
        let id = session::hash_token(&token);
        let ts = session::new_session_timestamps(Utc::now());
        state
            .db
            .create_session(&id, user_id, &ts.created_at, &ts.last_used, &ts.expires_at)
            .expect("create session");
        token
    }

    /// Build the fixture: users `bob` (owner, role=user), `mem` (member), `vwr`
    /// (viewer), `out` (non-member), plus the seeded `owner` (admin). A repo
    /// owned by `bob` with the corresponding membership rows.
    fn fixture(mode: AuthMode) -> Fixture {
        let state = app_state(mode);
        let db = &state.db;
        for (id, role) in [("bob", "user"), ("mem", "user"), ("vwr", "user"), ("out", "user")] {
            db.create_user(id, &format!("{id}@zync.local"), id, role)
                .expect("create user");
        }
        let repo = db
            .create_repository("proj", "/tmp/proj", None, "bob")
            .expect("create repo");
        let workspace = db
            .workspace_for_repository(&repo.id, &repo.name)
            .expect("workspace");
        db.add_repo_member(&repo.id, "mem", "member").expect("add member");
        db.add_repo_member(&repo.id, "vwr", "viewer").expect("add viewer");
        Fixture {
            state,
            repo_id: repo.id,
            workspace_id: workspace.id,
        }
    }

    fn matrix_app(state: Arc<AppState>) -> Router {
        async fn ok() -> &'static str {
            "ok"
        }
        Router::new()
            .route("/repositories/:id/git/status", routing::get(ok))
            .route("/repositories/:id/git/commit", routing::post(ok))
            .route("/repositories/:id", routing::delete(ok))
            .route("/repositories/:id/open", routing::post(ok))
            .route("/repositories/:id/members", routing::get(ok).post(ok))
            .route(
                "/repositories/:id/members/:user_id",
                routing::put(ok).delete(ok),
            )
            .route("/repositories/:id/favorite", routing::put(ok))
            .route("/workspace/:id", routing::get(ok))
            .route("/workspace/:id/files/x", routing::put(ok))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_repo_authz,
            ))
            .layer(axum::middleware::from_fn_with_state(state.clone(), require_auth))
            .with_state(state)
    }

    async fn request(
        app: &Router,
        method: Method,
        uri: &str,
        cookie: Option<&str>,
    ) -> StatusCode {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = cookie {
            builder = builder.header(header::COOKIE, format!("{}={token}", session::COOKIE_NAME));
        }
        app.clone()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn owner_can_read_mutate_manage_and_delete() {
        let fx = fixture(AuthMode::Enabled);
        let app = matrix_app(fx.state.clone());
        let cookie = session_for(&fx.state, "bob");
        let c = Some(cookie.as_str());
        let r = &fx.repo_id;
        assert_eq!(
            request(&app, Method::GET, &format!("/repositories/{r}/git/status"), c).await,
            StatusCode::OK
        );
        assert_eq!(
            request(&app, Method::POST, &format!("/repositories/{r}/git/commit"), c).await,
            StatusCode::OK
        );
        assert_eq!(
            request(&app, Method::GET, &format!("/repositories/{r}/members"), c).await,
            StatusCode::OK
        );
        assert_eq!(
            request(&app, Method::POST, &format!("/repositories/{r}/members"), c).await,
            StatusCode::OK
        );
        assert_eq!(
            request(&app, Method::DELETE, &format!("/repositories/{r}"), c).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn member_can_mutate_but_not_manage_or_delete_repo() {
        let fx = fixture(AuthMode::Enabled);
        let app = matrix_app(fx.state.clone());
        let cookie = session_for(&fx.state, "mem");
        let c = Some(cookie.as_str());
        let r = &fx.repo_id;
        assert_eq!(
            request(&app, Method::GET, &format!("/repositories/{r}/git/status"), c).await,
            StatusCode::OK
        );
        assert_eq!(
            request(&app, Method::POST, &format!("/repositories/{r}/git/commit"), c).await,
            StatusCode::OK
        );
        // Member management + repo delete are owner-only.
        assert_eq!(
            request(&app, Method::GET, &format!("/repositories/{r}/members"), c).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(&app, Method::POST, &format!("/repositories/{r}/members"), c).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(&app, Method::DELETE, &format!("/repositories/{r}"), c).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn viewer_can_read_but_not_mutate() {
        let fx = fixture(AuthMode::Enabled);
        let app = matrix_app(fx.state.clone());
        let cookie = session_for(&fx.state, "vwr");
        let c = Some(cookie.as_str());
        let r = &fx.repo_id;
        assert_eq!(
            request(&app, Method::GET, &format!("/repositories/{r}/git/status"), c).await,
            StatusCode::OK
        );
        // Opening is a viewer-allowed read-only POST.
        assert_eq!(
            request(&app, Method::POST, &format!("/repositories/{r}/open"), c).await,
            StatusCode::OK
        );
        // A mutating git route is forbidden for a viewer.
        assert_eq!(
            request(&app, Method::POST, &format!("/repositories/{r}/git/commit"), c).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(&app, Method::PUT, &format!("/repositories/{r}/favorite"), c).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn non_member_is_forbidden_on_read_and_mutate() {
        let fx = fixture(AuthMode::Enabled);
        let app = matrix_app(fx.state.clone());
        let cookie = session_for(&fx.state, "out");
        let c = Some(cookie.as_str());
        let r = &fx.repo_id;
        assert_eq!(
            request(&app, Method::GET, &format!("/repositories/{r}/git/status"), c).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            request(&app, Method::POST, &format!("/repositories/{r}/git/commit"), c).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn admin_bypasses_every_check() {
        let fx = fixture(AuthMode::Enabled);
        let app = matrix_app(fx.state.clone());
        // The seeded `owner` is a global admin.
        let cookie = session_for(&fx.state, OWNER_ID);
        let c = Some(cookie.as_str());
        let r = &fx.repo_id;
        for (method, path) in [
            (Method::GET, format!("/repositories/{r}/git/status")),
            (Method::POST, format!("/repositories/{r}/git/commit")),
            (Method::POST, format!("/repositories/{r}/members")),
            (Method::DELETE, format!("/repositories/{r}")),
        ] {
            assert_eq!(
                request(&app, method.clone(), &path, c).await,
                StatusCode::OK,
                "admin should pass {method} {path}"
            );
        }
    }

    #[tokio::test]
    async fn workspace_routes_are_membership_scoped() {
        let fx = fixture(AuthMode::Enabled);
        let app = matrix_app(fx.state.clone());
        let w = &fx.workspace_id;

        // Viewer can read the workspace, non-member cannot.
        let vwr = session_for(&fx.state, "vwr");
        assert_eq!(
            request(&app, Method::GET, &format!("/workspace/{w}"), Some(&vwr)).await,
            StatusCode::OK
        );
        let out = session_for(&fx.state, "out");
        assert_eq!(
            request(&app, Method::GET, &format!("/workspace/{w}"), Some(&out)).await,
            StatusCode::FORBIDDEN
        );
        // Viewer cannot write a file; member can.
        assert_eq!(
            request(&app, Method::PUT, &format!("/workspace/{w}/files/x"), Some(&vwr)).await,
            StatusCode::FORBIDDEN
        );
        let mem = session_for(&fx.state, "mem");
        assert_eq!(
            request(&app, Method::PUT, &format!("/workspace/{w}/files/x"), Some(&mem)).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn disabled_mode_owner_has_full_access() {
        let fx = fixture(AuthMode::Disabled);
        let app = matrix_app(fx.state.clone());
        let r = &fx.repo_id;
        // No cookie at all — disabled mode injects the synthetic admin owner.
        for (method, path) in [
            (Method::GET, format!("/repositories/{r}/git/status")),
            (Method::POST, format!("/repositories/{r}/git/commit")),
            (Method::POST, format!("/repositories/{r}/members")),
            (Method::DELETE, format!("/repositories/{r}")),
        ] {
            assert_eq!(
                request(&app, method.clone(), &path, None).await,
                StatusCode::OK,
                "disabled-mode owner should pass {method} {path}"
            );
        }
    }

    #[tokio::test]
    async fn unknown_repo_is_404_for_a_normal_user() {
        let fx = fixture(AuthMode::Enabled);
        let app = matrix_app(fx.state.clone());
        let cookie = session_for(&fx.state, "bob");
        // A syntactically valid but nonexistent repo id → 404 (not 403): there is
        // no repo to protect. (Admins would also 404 via the handler.)
        assert_eq!(
            request(
                &app,
                Method::GET,
                "/repositories/does-not-exist/git/status",
                Some(&cookie)
            )
            .await,
            StatusCode::NOT_FOUND
        );
    }
}
