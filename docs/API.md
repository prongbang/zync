# Zync API

HTTP reference for the `zync-server` Axum API. Routes are grouped by the
server module that registers them (`crates/server/src/*/mod.rs`, merged in
`crates/server/src/main.rs`). This document is generated from the route
tables and handler signatures in source — if you add or change a route,
update the matching section here too.

## Conventions

**Base URL.** No version prefix. In dev, Vite proxies `/repositories`,
`/workspace`, `/auth`, `/credentials`, `/directories`, `/health`, `/ready`,
and `/ws` to the API (`web/apps/web/vite.config.ts`); in production the
server serves the built React app same-origin, so every path below is
relative to that one origin.

**Authentication.** Every route is behind one router-wide middleware
(`auth::require_auth`) *except* a small, explicit allowlist
(`auth::is_public`):

- `GET /health`, `GET /ready`
- `POST /auth/login`
- `GET /setup`, `POST /setup`
- everything under `/ws/` (the WebSocket handshake is ticket-guarded inside
  the handler instead — cookies don't propagate reliably onto a WS upgrade)

Everything else requires a valid `zync_session` cookie (HttpOnly, SameSite=Lax,
sliding expiry — minted by `POST /auth/login`), or a `401 unauthenticated`.
The SPA static-file fallback (unmatched paths → `index.html`) is served
*outside* this layer and is always public.

`ZYNC_AUTH=disabled` (single-user/no-auth mode) bypasses all of the above: the
middleware injects a synthetic `owner`/`admin` principal into every request,
login/logout become no-ops, and the WS ticket check is skipped.

**Repository-scope authorization.** A second middleware
(`auth::authz::require_repo_authz`) runs after authentication and applies to
every `/repositories/:id/...` and `/workspace/:id/...` route (the git router
alone is ~75 such routes). It resolves the caller's role on that repository
(`owner` / `member` / `viewer`, from `workspace_members`) and enforces:

- safe methods (`GET`/`HEAD`/`OPTIONS`) require **viewer+**
- mutating methods require **member+**
- the whole `/repositories/:id/members*` subtree, and `DELETE
  /repositories/:id`, require **owner** regardless of method
- `POST /repositories/:id/open` is a deliberate carve-out: it only requires
  **viewer** (opening is how a viewer starts viewing)
- a caller with the global `admin` role bypasses every repo-scope check

A request that fails this check gets `403` (a real repo/workspace the caller
has no role on) or `404` (the repository/workspace id doesn't exist) —
distinct from the `401` `require_auth` returns for "not logged in". Routes
not under `/repositories/:id/...` or `/workspace/:id/...` (e.g. `GET
/repositories`, `/directories`, `/credentials/*`, `/auth/*`) are not
repo-scoped; they authorize themselves in-handler where needed (see below).

A few routes gate on the caller's **global** role instead of a repo role,
checked inside the handler: `GET/POST /auth/users` and `GET /metrics` require
`role == "admin"`; `POST /auth/ws-ticket` requires the caller be a member (any
role) of the ticket's target workspace, or a global admin.

**Errors.** A non-2xx response body is the raw error string returned by the
failing operation (`anyhow`/`git2` messages, validation text, etc.) — plain
text, not a JSON envelope. Remote git operations (fetch/pull/push/clone/
force-push/remote-branch-delete/tag-push/prune) map their failure kind to an
HTTP status via `map_git_error`: `Auth`→`401`, `Network`→`502`,
`NonFastForward`/`Conflict`/`Precondition`→`409`, `Timeout`→`504`,
`Other`/anything else→`500`.

**Request correlation.** Every response carries an `X-Request-Id` header — a
well-formed inbound `X-Request-Id` is echoed back verbatim, otherwise the
server generates a 32-character hex id. The same id is threaded into every
tracing span for that request, in both the default human log format and
`ZYNC_LOG_FORMAT=json`.

**Response bodies.** Most endpoints return JSON. A number of git endpoints
(diffs, `fetch`/`pull`/`push` output, LFS/submodule/rebase-continue command
output) return a raw `text/plain` string instead — noted per-endpoint below.
Mutations that return nothing meaningful respond `204 No Content`.

---

## Health & Metrics

Registered directly in `main.rs`, not a submodule. Not repo-scoped.

| Method | Path | Auth | Notes |
| --- | --- | --- | --- |
| `GET` | `/health` | public | Liveness. Always `200`, no I/O. Body: `{ "status": "ok", "version": "<CARGO_PKG_VERSION>" }`. |
| `GET` | `/ready` | public | Readiness. Does a cheap DB read (looks up the seeded `owner` row). `200 { "status": "ready" }`, or `503 { "status": "not_ready" }` if the DB doesn't answer. |
| `GET` | `/metrics` | session + **admin role** | Prometheus text exposition format (`text/plain; version=0.0.4`): `zync_http_requests_total{status=...}`, `zync_http_request_duration_seconds_{bucket,sum,count}`, `zync_ws_connections` (gauge), `zync_sync_watchers` (gauge). `403 "admin only"` for a non-admin caller. |

---

## Auth (`crates/server/src/auth/mod.rs`)

| Method | Path | Auth | Notes |
| --- | --- | --- | --- |
| `POST` | `/auth/login` | public | Rate-limited. Body `{ identifier, password }` (`LoginRequest`). `200` with the user (`UserResponse { id, email, name, role }`) and sets the `zync_session` cookie on success; `401 "invalid credentials"` on any failure (unknown user and wrong password are indistinguishable, timing-safe). In `ZYNC_AUTH=disabled` mode, always succeeds as the synthetic owner. |
| `POST` | `/auth/logout` | session | Deletes the session row read from the cookie and clears it. `204`. No-op `204` in disabled mode. |
| `GET` | `/auth/me` | session | Returns the current user (`UserResponse`), or `401` if the session's user row is gone. |
| `POST` | `/auth/ws-ticket` | session | Rate-limited (generous — reconnect-friendly). Body `{ workspace_id }` (`WsTicketRequest`). Mints a short-lived, single-use ticket bound to `(user, workspace)` for the WS handshake: `200 { "ticket": "..." }`. Requires the caller be a member (any role) of the workspace's repository, or a global admin — `403` otherwise, `404` if the workspace doesn't exist. |
| `GET` | `/auth/users` | session + **admin role** | List every user: `Vec<UserSummary>` (`id, email, name, role, created_at` — never a password hash). `403 "admin role required"` for a non-admin. |
| `POST` | `/auth/users` | session + **admin role** | Admin-only user provisioning. Body `{ identifier, password, name?, role? }` (`CreateUserRequest`; `role` defaults to `"user"`, must be `"admin"` or `"user"`). `200` with the created `UserResponse`. `400` on empty identifier/password or an invalid role; `409` on a duplicate identifier. |
| `GET` | `/setup` | public | Returns a plain-text hint: `POST /setup with JSON { token, identifier, password } to set the initial admin password.` |
| `POST` | `/setup` | public | Rate-limited. Body `{ token, identifier, password }` (`SetupRequest`). Consumes the one-time first-boot setup token (logged at startup, or set via `ZYNC_ADMIN_USER`/`ZYNC_ADMIN_PASSWORD`) and sets the admin password. `204` on success; `403` on an invalid/expired token; `409` if the server is already bootstrapped; `400` on empty identifier/password. |

---

## Repositories & Directories (`crates/server/src/repository/mod.rs`)

Not repo-scoped except where noted (a request under `/repositories/:id/...`
is repo-scoped per the rules above).

| Method | Path | Cap | Notes |
| --- | --- | --- | --- |
| `GET` | `/directories` | session | Directory browser for the "open/clone/init a repository" picker. Query `?path=` (optional). If `ZYNC_REPOS_ROOT` is configured, browsing is confined to it: no `path` lists the configured roots themselves (`403` for anything outside them); an out-of-root `path` also `403`s. Response `DirectoryList { current_path, parent_path, directories: [{ name, path }] }`. |
| `GET` | `/repositories` | session | Lists repositories the caller can see: owned or member-of for a normal user, every repository for a global admin. `Vec<RepositoryRecord>`. |
| `POST` | `/repositories` | session | Registers a repository in one of three modes via `CreateRepositoryRequest { name?, path?, remote_url?, clone_to?, init? }`: **register existing** (`path`), **clone** (`remote_url` + `clone_to` — resolves a stored credential for the URL via `credentials::resolve_credential_spec_for_url`), or **init** (`path` + `init: true`, plain `git init`, no commit). Registering a path that's already registered attaches to the existing repo (requires the caller already have a role on it, or global admin). Enforces `ZYNC_REPOS_ROOT` when configured (`403` outside it). Any inline `user:token@` userinfo in `remote_url` is stripped before it's stored/echoed. Response `RepositoryWithWorkspace { repository, workspace }`; also starts the filesystem watcher and broadcasts `repository_opened` on the workspace's socket. |
| `DELETE` | `/repositories/:id` | **owner** | Removes the repository's DB registration (does not touch the filesystem). `204`. |
| `PUT` | `/repositories/:id/favorite` | member | Body `{ favorite: bool }` (`FavoriteRequest`). `204`. |
| `POST` | `/repositories/:id/open` | viewer | Re-opens an already-registered repository: `zync_git_core::open_repo`, (re)starts the fs watcher, broadcasts `repository_opened`. `404` if the repository id is unknown. Response `RepositoryWithWorkspace`. |
| `GET` | `/repositories/:id/members` | **owner** | List the repo's members: `Vec<RepoMember> { user_id, role, email, name }`. |
| `POST` | `/repositories/:id/members` | **owner** | Body `{ user: <id-or-email>, role }` (`AddMemberRequest`; `role` one of `owner`/`member`/`viewer`). `404` if the user doesn't exist. `204`. |
| `PUT` | `/repositories/:id/members/:user_id` | **owner** | Body `{ role }` (`UpdateMemberRequest`). `409` if `user_id` is the repo's own `owner_id` (can't demote the owner this way); `404` if there's no membership row for that user. `204`. |
| `DELETE` | `/repositories/:id/members/:user_id` | **owner** | `409` if `user_id` is the repo's `owner_id`. `204`. |

---

## Workspace (`crates/server/src/workspace/mod.rs`)

| Method | Path | Cap | Notes |
| --- | --- | --- | --- |
| `GET` | `/workspace/:id` | viewer | The workspace "shell" payload: `WorkspaceResponse { workspace: WorkspaceRecord, repository: RepositoryRecord, files: Vec<FileNode>, online_users: Vec<PresenceUser> }`. `files` is the full recursive file tree (`{ path, name, is_dir, size }`); `online_users` comes from in-memory presence (see Collaboration below), not persisted. `404` if the workspace or its repository is missing. |

---

## Files (`crates/server/src/files/mod.rs`)

All under `/workspace/:id/...`; capability follows the workspace authz rule
(reads = viewer, writes = member). Paths are resolved against the
repository's working directory and re-validated after symlink resolution to
stay inside it (and inside `ZYNC_REPOS_ROOT`, when configured) — `403
"unsafe path"`/`"...outside the allowed root"` on an escape attempt,
including through a symlink or a dangling one.

| Method | Path | Cap | Notes |
| --- | --- | --- | --- |
| `POST` | `/workspace/:id/files` | member | Create a file or directory. Body `{ path, content?, is_dir? }` (`CreateFileRequest`). `201`. Broadcasts `file_created`/`folder_created`. |
| `PUT` | `/workspace/:id/files/rename` | member | Body `{ old_path, new_path }` (`RenameFileRequest`). `204`. Broadcasts `file_renamed` (`payload: { new_path }`). |
| `GET` | `/workspace/:id/files/search` | viewer | Query `?q=` (substring, case-insensitive, matched against the full recursive file listing). `Vec<FileNode>`. |
| `GET` | `/workspace/:id/assets/*path` | viewer | Raw file bytes for inline UI rendering (e.g. images in the diff/blob viewer). `Content-Type` inferred from the extension (`apng/avif/gif/jpg/jpeg/png/svg/webp` → the matching image type; else `application/octet-stream`). |
| `GET` | `/workspace/:id/files/*path` | viewer | Read a text file: `FileContent { path, content }` (`fs::read_to_string` — binary files error). |
| `PUT` | `/workspace/:id/files/*path` | member | Overwrite a file. Body `{ content }` (`WriteFileRequest`). `204`. Broadcasts `file_changed`. |
| `DELETE` | `/workspace/:id/files/*path` | member | Removes a file or (recursively) a directory. `204`. Broadcasts `file_deleted`/`folder_deleted`. |

---

## Collaboration — Presence & Locks (`crates/server/src/collaboration/mod.rs`)

In-memory only (not persisted); capability follows the workspace authz rule.

| Method | Path | Cap | Notes |
| --- | --- | --- | --- |
| `GET` | `/workspace/:id/presence` | viewer | `Vec<PresenceUser> { user_id, name, current_file, cursor_line, cursor_column }` currently registered for the workspace. |
| `PUT` | `/workspace/:id/presence/:user_id` | member | Body `{ name, current_file?, cursor_line?, cursor_column? }` (`PresenceRequest`). `:user_id` must equal the caller's own id (or the caller be a global admin) — `403 "cannot act as another user"` otherwise. `204`; broadcasts `user_joined`. |
| `DELETE` | `/workspace/:id/presence/:user_id` | member | Same self-or-admin rule as above. `204`; broadcasts `user_left`. |
| `PUT` | `/workspace/:id/locks/:path` | member | Advisory file lock, always taken **as the authenticated caller** (no body). `204`; broadcasts `file_locked` (`user_id` = caller). |
| `DELETE` | `/workspace/:id/locks/:path` | member | Only the lock's current holder or a global admin may clear it — `403 "cannot clear another user's lock"` otherwise; unlocking an unlocked path is a no-op. `204`; broadcasts `file_unlocked`. |

---

## WebSocket (`crates/server/src/websocket/mod.rs`)

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `/ws/workspace/:id` | Live-sync socket. Query `?ticket=<token>` — obtain via `POST /auth/ws-ticket`; the ticket is single-use and validated/consumed before the upgrade (`401 "invalid or missing ws ticket"` otherwise). Not required in `ZYNC_AUTH=disabled` mode. |

Frames are JSON `WorkspaceEvent { id, workspace_id, kind, path?, user_id?,
payload, timestamp }`. Server→client `kind`s include: `repository_opened`,
`file_created`/`file_changed`/`file_deleted`/`folder_created`/
`folder_deleted`/`file_renamed` (individual fs writes via the Files API),
`workspace_batch` (`payload: { events: [...] }` — coalesced raw filesystem
watcher events, 120ms-debounced, `.git` paths excluded), `git_changed`
(`payload: { scopes: [...] }` — emitted by every mutating git route; see
`crates/server/src/git/mod.rs`'s `broadcast_git_change`), `user_joined`/
`user_left`, `file_locked`/`file_unlocked`.

Client→server: only a caller resolved as `member`/`owner` of the workspace's
repository (or a global admin) may inject events — the ticket carries the
resolved role from the handshake. A `viewer`'s (or any read-only caller's)
inbound text frames are silently dropped rather than rebroadcast; their
outbound (receive) stream is unaffected.

---

## Credentials (`crates/server/src/credentials/mod.rs`)

Not repo-scoped — every credential is scoped to the authenticated caller
(`user_id`), never shared across users. Not returned with secret material.

| Method | Path | Auth | Notes |
| --- | --- | --- | --- |
| `GET` | `/credentials` | session | The caller's stored credentials: `Vec<CredentialResponse> { id, label, host_pattern, kind, username, created_at }` — never the secret. |
| `POST` | `/credentials` | session | Body `CreateCredentialRequest { label, host_pattern, kind, username?, token?, private_key?, passphrase?, public_key? }`. `kind` is `"https_token"` (requires `token`) or `"ssh_key"` (requires `private_key`; `passphrase`/`public_key` optional). `host_pattern` must be an exact host or a single leading `"*.suffix"` wildcard (`400` otherwise, e.g. `"*github.com"` — missing dot — or a bare `"*"`). The secret is encrypted at rest before storage. `201` with the created `CredentialResponse`. `503` if server-side encryption isn't configured (`ZYNC_SECRET_KEY`/similar unset). |
| `DELETE` | `/credentials/:id` | session | Deletes a credential owned by the caller. `204`. |

Credentials are selected automatically for git remote operations by host +
scheme (exact host beats a wildcard, most-specific wildcard wins ties,
newest `created_at` breaks a tie) — there's no per-request credential
selection endpoint; see `resolve_credential_spec[_for_url]` in
`credentials/mod.rs`.

---

## Git (`crates/server/src/git/mod.rs`)

All routes are `/repositories/:id/git/...` and repo-scoped per the rules
above (reads = viewer, writes = member, no owner-only carve-outs in this
module). Every mutating route ends by calling `broadcast_git_change(state,
repository_id, &[scopes])`, which emits a `git_changed` WS event listing
which of `status`/`diff`/`commits`/`branches`/`tags`/`stashes`/`conflicts`/
`workspace` changed — the frontend's `scopeForEvent`/`useWorkspace` use this
to refetch only what's stale rather than reloading everything. Remote-op
routes (fetch/pull/push/clone-adjacent) map failures through `map_git_error`
(see Conventions); everything else in this module falls back to a plain
`500` with the raw error string on failure.

### Status, staging, and commit

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/status` | `Vec<FileStatus> { path, staged, unstaged, untracked, ignored, conflicted }`. |
| `POST` | `.../git/add` | Body `{ files: [...] }` (`FilesRequest`). `204`. |
| `POST` | `.../git/unstage` | Body `{ files: [...] }`. `204`. |
| `POST` | `.../git/discard` | Discards working-tree changes to the given files. Body `{ files: [...] }`. `204`. |
| `POST` | `.../git/stage-patch` | Applies a hand-edited unified diff to the index (partial/hunk staging). Body `{ patch: "<diff text>" }` (`PatchRequest`). `204`. |
| `POST` | `.../git/commit` | Body `CommitRequest { message, author_name, author_email, amend?, sign_off? }` (`sign_off` appends a `Signed-off-by:` trailer). `200 { "commit": "<oid>" }`. |

### Diff

Query `?path=` scopes a diff to one file (workdir/staged only — never
truncated, since the staging UI builds a stage-patch from it). Query
`?max_bytes=` (default 5,000,000; clamped 65,536–50,000,000) caps a
whole-tree diff, appending a truncation note when exceeded. All four return
raw unified-diff text (`text/plain`), not JSON.

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/diff/workdir` | Unstaged changes. `?path=`, `?max_bytes=`. |
| `GET` | `.../git/diff/staged` | Staged (index vs HEAD) changes. `?path=`, `?max_bytes=`. |
| `GET` | `.../git/diff/commit/:commit_id` | That commit vs its first parent. `?max_bytes=` only. |
| `GET` | `.../git/diff/compare/:commit_id` | That commit vs the current working tree. `?max_bytes=` only. |

### Remotes, fetch, pull, push

`RemoteRequest { remote?, branch?, url?, mode?, force_with_lease?,
set_upstream? }` is reused across most of this group; `remote` defaults to
`"origin"` where applicable. Credentials are resolved per-call from the
caller's stored credentials (`credentials::resolve_credential_spec[_for_url]`).

| Method | Path | Notes |
| --- | --- | --- |
| `POST` | `.../git/fetch` | Body `{ remote? }`. Raw text result. |
| `POST` | `.../git/fetch-all` | No body. Fetches every configured remote in turn; stops (and still broadcasts if anything already landed) at the first failure. Raw text (`"no remotes configured"` if none exist), joined per-remote with `\n`. |
| `POST` | `.../git/pull` | Body `{ remote?, branch?, mode? }`; `mode` is `"ff-only"` (default) \| `"merge"` \| `"rebase"` (`400` on anything else). Raw text result. |
| `POST` | `.../git/push` | Body `{ remote?, branch?, force_with_lease?, set_upstream? }`. Plain push always attempts `-u`-style upstream tracking; `force_with_lease: true` routes through a lease push instead (and only sets upstream if `set_upstream: true` is also passed). Raw text result. |
| `GET` | `.../git/remotes` | `Vec<RemoteSummary> { name, url, push_url }`. |
| `POST` | `.../git/remotes` | Add a remote. Body `{ remote? (name, default "origin"), url }` (`url` required, `400` otherwise). `204`. |
| `POST` | `.../git/remotes/delete` | Body `{ remote? }`. `204`. |
| `POST` | `.../git/remotes/prune` | Body `{ remote? }`. Prunes stale remote-tracking refs; contacts the remote (can fail Auth/Network/Timeout). Raw text result. |
| `POST` | `.../git/remotes/branch/delete` | Delete a branch on the remote. Body `{ remote?, branch }` (`branch` required). `204`. |
| `POST` | `.../git/push/force-with-lease` | Body `{ remote?, branch }` (`branch` required). Raw text result. |

### Branches

`BranchRequest { name, new_name?, checkout?, revision?, strategy? }` is
reused across this group (`name` means different things per route — the
branch being created/checked-out/deleted/merged/renamed).

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/branches` | `Vec<BranchSummary> { name, is_head, kind, target, ahead?, behind? }`. |
| `POST` | `.../git/branches` | Create a branch. Body `{ name, revision?, checkout? }` — from `revision` if given, else from `HEAD`; `checkout` defaults `false`. `204`. |
| `POST` | `.../git/checkout` | Checkout an existing branch. Body `{ name }`. `204`. |
| `POST` | `.../git/checkout/revision` | Detached checkout of an arbitrary revision. Body `{ revision, hard? }` (`RevisionRequest`). `204`. |
| `POST` | `.../git/branches/rename` | Body `{ name, new_name }` (`new_name` required, `400` otherwise). `204`. |
| `POST` | `.../git/branches/merge` | Body `{ name, strategy? }`; `strategy` is `"ff-only"` \| `"no-ff"` (default) \| `"squash"` (`400` on anything else). `204`. |
| `POST` | `.../git/branches/delete` | Body `{ name }`. `204`. |
| `POST` | `.../git/branches/upstream` | Set upstream tracking. Body `{ remote?, branch }` (`branch` required) — sets `branch` to track `remote/branch`. Raw text result. |

### Tags

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/tags` | `Vec<TagSummary> { name, target, annotated, message?, tagger?, time? }`. |
| `POST` | `.../git/tags` | Body `{ name, target? }` (`TagRequest`; `target` defaults to `HEAD`). `204`. |
| `POST` | `.../git/tags/delete` | Body `{ name }`. `204`. |
| `POST` | `.../git/tags/push` | Body `{ name, remote? }` (`PushTagRequest`; `name` required non-empty, `400` otherwise). Raw text result. |

### History, browsing, and stats

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/graph` | Paginated commit list for the graph view. Query `?limit=` (default 500, max 5000), `?cursor=` (opaque continuation token). `Vec<CommitSummary> { id, summary, author, author_email, committer, committer_email, time, parents, refs: [{name, kind}] }`. |
| `GET` | `.../git/search` | Full-history commit search (not limited to a loaded page). Query `?q=` (message/author/SHA substring, case-insensitive; empty matches all), `?limit=` (default 200, max 2000), `?path=` (only commits touching this file). `Vec<CommitSummary>`. |
| `GET` | `.../git/stats` | Repo-wide contributor/activity stats. Query `?limit=` (default 20000, commits scanned). `RepoStats { commit_count, contributors: [{name, commits}], monthly: [{year, month, total, top}], first_commit_time, last_commit_time }`. |
| `GET` | `.../git/blame` | Query `?path=` (required, `400` otherwise). `Vec<BlameLine> { start_line, line_count, commit, author, summary }`. |
| `GET` | `.../git/history/file` | Commits touching one file. Query `?path=` (required), `?limit=` (default 100, max 1000). `Vec<CommitSummary>`. |
| `GET` | `.../git/tree` | Query `?revision=` (default `HEAD`). `Vec<TreeEntrySummary> { path, kind, id, size? }`. |
| `GET` | `.../git/blob` | Raw file bytes at a revision. Query `?path=` (required), `?revision=` (default `HEAD`; the sentinel `:workdir` reads the uncommitted working-tree copy instead of a committed blob — used for the "after" side of an added/modified file in image diffs). `Content-Type` inferred from extension (adds `bmp`/`ico` to the Files-module set above); always sends `X-Content-Type-Options: nosniff`, and for `image/svg+xml` additionally `Content-Security-Policy: sandbox` + `Content-Disposition: inline` so an SVG can't execute script if navigated to directly. |
| `GET` | `.../git/reflog` | Query `?limit=` (default 100, max 1000). `Vec<ReflogEntrySummary> { index, old_id, new_id, message, committer, time }`. |
| `GET` | `.../git/rebase/plan` | The commits available to build an interactive rebase plan from (reuses the graph query). Query `?limit=` (default 20, max 200). `Vec<CommitSummary>`. |

### Reset, revert, cherry-pick

| Method | Path | Notes |
| --- | --- | --- |
| `POST` | `.../git/reset` | Body `{ revision, hard? }` (`RevisionRequest`). `204`. |
| `POST` | `.../git/revert` | Body `{ commit, mainline? }` (`CommitIdRequest`; `mainline` is the 1-based parent number, required only when `commit` is a merge commit). `200 { "commit": "<oid>" }`. |
| `POST` | `.../git/cherry-pick` | Body `{ commits: [...] }` (`CherryPickRequest`, applied in order). `204`. |
| `POST` | `.../git/cherry-pick/abort` | No body. `204`. |

### Conflicts

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/conflicts` | `Vec<ConflictSummary> { ancestor, ours, theirs }` (each an optional path). |
| `GET` | `.../git/conflicts/detail` | Query `?path=` (required). `ConflictDetail { path, ancestor_path?, ours_path?, theirs_path?, ancestor_content, ours_content, theirs_content }`. |
| `POST` | `.../git/conflicts/resolve` | Body `{ path, side }` (`ResolveConflictRequest`; `side` is `"local"` or `"remote"`, `400` otherwise) — resolves via checkout of the chosen side. `204`. |

### Stashes

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/stashes` | `Vec<StashSummary> { index, name, message }`. |
| `POST` | `.../git/stashes` | Body `StashRequest { message?, author_name?, author_email?, index?, pop? }` — `message` defaults `"WIP"`, `author_name`/`author_email` default `"Zync"`/`"zync@local"`. `204`. |
| `POST` | `.../git/stashes/apply` | Body `{ index? (default 0), pop? (default false) }`. `204`. |
| `POST` | `.../git/stashes/drop` | Body `{ index? (default 0) }`. `204`. |

### Rebase

`interactive_rebase` is the mechanism behind the UI's quick reword/edit/
squash/fixup/drop actions (client-built plans; see `quickRebasePlan` in
`web/apps/web/src/lib/helpers.ts`), and `rebase/branch` backs the branch
sidebar's plain "rebase onto" action.

| Method | Path | Notes |
| --- | --- | --- |
| `POST` | `.../git/rebase/branch` | Plain (non-interactive) branch-onto-branch rebase. Body `{ name }` (`BranchRequest`, `name` = the upstream branch). **Broadcasts before mapping the result to an HTTP error** — a mid-rebase conflict leaves the repo in a conflicted state and the client must still refresh; the failure itself still comes back mapped via `map_git_error`. `204` on success. |
| `POST` | `.../git/rebase/interactive` | Body `RebaseRequest { base, steps: [{ commit, action, message? }] }`; `action` is one of `pick`/`squash`/`fixup`/`drop`/`edit` (`RebaseAction`, snake_case on the wire). Requires a clean working tree. `200` with `RebaseResult { head?, stopped_at?, applied: [...], dropped: [...] }` — `stopped_at` is set when the rebase paused (e.g. on an `edit` step or a conflict). |
| `POST` | `.../git/rebase/continue` | Continues a paused rebase after conflicts are resolved. No body. Raw text result. |
| `POST` | `.../git/rebase/abort` | No body. Raw text result. |
| `POST` | `.../git/rebase/skip` | Skips the current stopped commit. No body. Raw text result. |

### Bisect

| Method | Path | Notes |
| --- | --- | --- |
| `POST` | `.../git/bisect/start` | Body `{ bad, good?: [...] }` (`BisectStartRequest`). Starts a session and checks out the first candidate (moves HEAD). Raw text result. |
| `POST` | `.../git/bisect/good` | Body `{ rev? }` (`BisectMarkRequest`; omit `rev` to mark the commit currently checked out). Raw text result. |
| `POST` | `.../git/bisect/bad` | Body `{ rev? }`. Raw text result. |
| `POST` | `.../git/bisect/skip` | Body `{ rev? }`. Raw text result. |
| `POST` | `.../git/bisect/reset` | No body. Ends the session, returns to the pre-bisect HEAD. Raw text result. |
| `GET` | `.../git/bisect/status` | `BisectStatus { in_progress, current_commit?, bad?, good: [...], skipped: [...], steps_remaining? }`. |

### Submodules

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/submodules` | `Vec<SubmoduleSummary> { name, path, url?, head? }`. |
| `POST` | `.../git/submodules/init` | No body. Raw text result. |
| `POST` | `.../git/submodules/update` | No body. Raw text result. |
| `POST` | `.../git/submodules/sync` | No body. Raw text result. |
| `POST` | `.../git/submodules/add` | Body `{ path, url }` (`SubmoduleRequest`; both required non-empty, `400` otherwise). Raw text result. |
| `POST` | `.../git/submodules/remove` | Body `{ path }`. Raw text result. |

### LFS (Git Large File Storage)

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `.../git/lfs` | `LfsSummary { configured, tracked_patterns: [...] }`. |
| `POST` | `.../git/lfs/install` | No body. Raw text result. |
| `POST` | `.../git/lfs/track` | Body `{ pattern }` (`LfsRequest`; required, `400` otherwise). Raw text result. |
| `POST` | `.../git/lfs/untrack` | Body `{ pattern }`. Raw text result. |
| `POST` | `.../git/lfs/pull` | No body. Raw text result. |
| `POST` | `.../git/lfs/push` | Body `{ remote?, branch }` (`branch` required, `400` otherwise). Raw text result. |

---

## Route count

113 method+path route entries across 9 route modules plus 3 top-level
(`/health`, `/ready`, `/metrics`) routes registered directly in `main.rs`:

| Subsystem (module) | Routes |
| --- | --- |
| Health & Metrics (`main.rs`) | 3 |
| Auth (`auth::routes`) | 8 |
| Repositories & Directories (`repository::routes`) | 10 |
| Workspace (`workspace::routes`) | 1 |
| Files (`files::routes`) | 7 |
| Collaboration (`collaboration::routes`) | 5 |
| WebSocket (`websocket::routes`) | 1 |
| Credentials (`credentials::routes`) | 3 |
| Git (`git::routes`) | 75 |
| **Total** | **113** |

The 75-route git count is cross-checked by a compile-time source-parsing
test in `crates/server/src/auth/authz.rs`
(`git_write_classification_matches_broadcast_call_sites`), which asserts the
git router has at least 70 method-route entries and that every one's
authz-guard classification (viewer/member) matches whether its handler
actually calls `broadcast_git_change`.
