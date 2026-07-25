use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
}

/// A user together with its stored argon2 password hash (`None` for the
/// un-bootstrapped seed row). Never serialized to a response — the hash stays
/// server-side; only [`User`] is ever returned over HTTP. `Debug` is
/// hand-written to redact `password_hash` even on a stray `{:?}` (log line,
/// panic message) — mirrors `CredentialSecretRow`'s manual impl. An argon2id
/// hash is still secret material (offline-crackable) and must never appear
/// outside the verify path.
#[derive(Clone)]
pub struct UserWithHash {
    pub user: User,
    pub password_hash: Option<String>,
}

impl std::fmt::Debug for UserWithHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserWithHash")
            .field("user", &self.user)
            .field(
                "password_hash",
                &self.password_hash.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// The admin user-list projection (P3.5) — adds `created_at` to [`User`]'s
/// fields without touching every existing `User` SELECT (`user_from_row` stays
/// the lean 4-column shape login/`/auth/me`/authz use). Never carries
/// `password_hash`; the only shape `GET /auth/users` may return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub remote_url: Option<String>,
    pub favorite: bool,
    pub created_at: String,
    /// The repo's owner (creator) user id (ADR-002 Decision 5). `None` only on a
    /// pre-auth row the backfill hasn't yet normalized; every row created or
    /// migrated by this build carries an owner.
    pub owner_id: Option<String>,
}

/// A member row on a repository's workspace, joined with the user's display
/// fields for the member-management API (ADR-002 Decision 5 / P3.5). `email`/
/// `name` are `None` if the member's user row is missing (shouldn't happen —
/// membership FKs `users` — but the join stays lenient).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMember {
    pub user_id: String,
    pub role: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub id: String,
    pub repository_id: String,
    pub name: String,
    pub created_at: String,
}

/// A server-side session row. `id` is the SHA-256 hex of the raw cookie token
/// (never the raw token itself — see ADR-002 Decision 2), so a leaked DB yields
/// session *hashes*, not live bearer tokens. Timestamps are RFC3339 strings;
/// the auth layer parses them for the sliding-expiry logic.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub created_at: String,
    pub last_used: String,
    pub expires_at: String,
}

/// Masked credential projection — the only shape ever returned over HTTP.
/// Never carries `secret_cipher`/`secret_nonce`; see ADR-001 "Write-only
/// secrets".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: String,
    pub user_id: String,
    pub label: String,
    pub host_pattern: String,
    pub kind: String,
    pub username: Option<String>,
    pub created_at: String,
}

/// Full credential row, including the encrypted secret bundle. Only ever
/// read internally (just-in-time decrypt in a remote-op handler) — never
/// serialized to a response and never logged. `Debug` is hand-written to
/// redact the secret columns even if something accidentally `{:?}`s this.
#[derive(Clone)]
pub struct CredentialSecretRow {
    pub id: String,
    pub user_id: String,
    pub label: String,
    pub host_pattern: String,
    pub kind: String,
    pub username: Option<String>,
    pub secret_cipher: Vec<u8>,
    pub secret_nonce: Vec<u8>,
    pub created_at: String,
}

impl std::fmt::Debug for CredentialSecretRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialSecretRow")
            .field("id", &self.id)
            .field("user_id", &self.user_id)
            .field("label", &self.label)
            .field("host_pattern", &self.host_pattern)
            .field("kind", &self.kind)
            .field("username", &self.username)
            .field("secret_cipher", &"<redacted>")
            .field("secret_nonce", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

/// Sentinel error for [`Database::create_user_with_password`]'s race path:
/// the identifier passed the caller's pre-check but a concurrent insert won
/// the `email` UNIQUE constraint first. Callers can `downcast_ref` for this
/// on the returned `anyhow::Error` to map it to `409 Conflict` instead of a
/// generic `500`.
#[derive(Debug, thiserror::Error)]
#[error("a user with that identifier already exists")]
pub struct UserConflict;

/// Whether `err` is a SQLite UNIQUE (or other) constraint-violation error,
/// as opposed to a connection/IO/syntax failure that genuinely warrants a
/// `500`.
fn is_unique_constraint_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::ConstraintViolation,
                ..
            },
            _
        )
    )
}

/// Connection-level pragmas applied once, right after opening (P5.1).
///
/// - `journal_mode = WAL`: readers no longer block writers (and vice versa)
///   the way rollback-journal mode does. This server serializes all access
///   through a single `Arc<Mutex<Connection>>` anyway, so WAL's concurrency
///   benefit is mostly for *external* readers of the same file — an
///   `sqlite3 zync.db` shell, a future backup/admin CLI (P5.2) — which would
///   otherwise contend with the server's writer lock.
/// - `synchronous = NORMAL`: the documented safe pairing with WAL (SQLite
///   docs: "safe from corruption... but may lose the most recent commits" only
///   on a full OS/power failure, not an application crash). `FULL` is
///   unnecessary overhead once WAL is on.
/// - `busy_timeout = 5000`: any transient `SQLITE_BUSY` (e.g. a checkpoint in
///   progress) retries for up to 5s instead of failing the request
///   immediately — matters more once more than one process/connection ever
///   touches the file (backups, tooling), but is a no-cost safety margin now.
/// - `foreign_keys = ON`: OFF by default in SQLite for backward
///   compatibility. Without it every `FOREIGN KEY` clause in this schema
///   (`workspaces.repository_id`, `sessions.user_id`, etc.) is silently
///   decorative and never enforced.
fn apply_pragmas(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;",
    )?;

    // Verify WAL actually engaged rather than trusting the statement not
    // erroring — `journal_mode` is one of the pragmas that reports back the
    // mode SQLite actually ended up in, which can differ from what was
    // requested (an in-memory database, used throughout this module's tests,
    // can never use WAL and correctly reports `memory`; some exotic
    // filesystems that don't support shared memory / mmap silently fall back
    // too). Only warn — this is an operational concern, not a reason to
    // refuse to boot.
    let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    if journal_mode != "wal" && journal_mode != "memory" {
        tracing::warn!(
            journal_mode = %journal_mode,
            "sqlite did not engage WAL journal mode; concurrent access may block"
        );
    }
    Ok(())
}

/// A single, ordered, atomically-applied schema change. `apply` runs inside a
/// transaction that only commits (bumping `PRAGMA user_version` to
/// `version`) if it returns `Ok` — see [`run_migrations`].
struct Migration {
    version: i64,
    name: &'static str,
    apply: fn(&Connection) -> anyhow::Result<()>,
}

/// Every migration this build knows about, oldest first, tracked via
/// `PRAGMA user_version` rather than a separate `schema_version` table.
///
/// Why `user_version` over a table: it's a single integer already built into
/// the SQLite file header, so bumping it is just another statement inside
/// the same transaction as the migration's DDL/DML — there's no separate
/// table whose row can itself drift out of sync with the schema it claims to
/// describe, and no risk of "the migration committed but the bookkeeping
/// insert didn't" (or vice versa) since both happen atomically. A
/// `schema_version` table would add an audit trail (timestamps, names) that
/// nothing here reads back programmatically; `PRAGMA user_version` is the
/// simpler tool that fits what this project actually needs: one current
/// version number.
///
/// Migration 1 is deliberately the *entire* schema as it exists today — the
/// P0-P4 accumulation of `CREATE TABLE IF NOT EXISTS` + idempotent
/// `ALTER TABLE ADD COLUMN`s this file already had before P5.1, unchanged.
/// That is what makes upgrading an already-deployed database safe: such a
/// database has every table/column migration 1 would create, so running it
/// again is a structural no-op (every `CREATE TABLE`/`ADD COLUMN` is
/// existence-guarded) — the *only* effect on that database is stamping
/// `user_version = 1`, exactly the "detect + stamp without re-creating"
/// behavior an existing deployment needs. A brand-new database has none of
/// this yet, so the same migration *builds* the whole schema there. Either
/// way, after migration 1 both databases are byte-for-byte the same shape.
/// Any future schema change is a new migration appended here with the next
/// version number — migration 1's body must never be edited once shipped.
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "baseline schema (P0-P4 accumulated ad-hoc schema)",
    apply: migration_001_baseline,
}];

/// Applies every migration in `migrations` whose version is greater than the
/// database's current `PRAGMA user_version`, in ascending order. Each
/// migration runs inside its own transaction (`Connection::transaction`):
/// `apply` runs, then `user_version` is bumped to that migration's version,
/// then the transaction commits — a `Transaction` that goes out of scope
/// without an explicit `commit()` rolls back automatically, so a migration
/// that returns `Err` (whether from the SQL itself or a later step in the
/// same closure/fn) leaves *both* its own writes and the version bump
/// undone. The error propagates to the caller (`Database::open`, and from
/// there `main`), so a broken migration refuses to boot rather than running
/// against a half-migrated schema.
fn run_migrations(conn: &mut Connection, migrations: &[Migration]) -> anyhow::Result<()> {
    let current_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for migration in migrations {
        if migration.version <= current_version {
            continue;
        }
        let tx = conn.transaction()?;
        (migration.apply)(&tx).map_err(|err| {
            anyhow::anyhow!(
                "migration {} ({}) failed, refusing to boot: {err}",
                migration.version,
                migration.name
            )
        })?;
        tx.pragma_update(None, "user_version", migration.version)?;
        tx.commit()?;
        tracing::info!(
            version = migration.version,
            name = migration.name,
            "applied database migration"
        );
    }
    Ok(())
}

/// Migration 1 — see [`MIGRATIONS`] for why this is the full current schema
/// rather than an incremental step. Verbatim behavior of the pre-P5.1
/// `Database::migrate` body: base tables, additive `ALTER TABLE`s, the
/// one-time `sessions` reshape (ADR-002 Decision 2/6), and the role/owner
/// backfill (ADR-002 Decision 6) — all still guarded so they're no-ops on a
/// database that already has this shape.
fn migration_001_baseline(conn: &Connection) -> anyhow::Result<()> {
    // Base tables (unchanged shape). `sessions` is handled separately below
    // because its shape is reshaped for ADR-002 and its rows are ephemeral.
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            role TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS repositories (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            remote_url TEXT,
            favorite INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY,
            repository_id TEXT NOT NULL,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(repository_id) REFERENCES repositories(id)
        );

        CREATE TABLE IF NOT EXISTS workspace_members (
            workspace_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL,
            PRIMARY KEY(workspace_id, user_id),
            FOREIGN KEY(workspace_id) REFERENCES workspaces(id),
            FOREIGN KEY(user_id) REFERENCES users(id)
        );

        CREATE TABLE IF NOT EXISTS credentials (
            id            TEXT PRIMARY KEY,
            user_id       TEXT NOT NULL,
            label         TEXT NOT NULL,
            host_pattern  TEXT NOT NULL,
            kind          TEXT NOT NULL CHECK (kind IN ('https_token', 'ssh_key')),
            username      TEXT,
            secret_cipher BLOB NOT NULL,
            secret_nonce  BLOB NOT NULL,
            created_at    TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        "#,
    )?;

    // ADR-002 Decision 6 — additive migration. `ALTER TABLE ADD COLUMN` is
    // idempotent here because it's guarded by a column-existence check, so
    // this runs cleanly on both a fresh DB (base tables above lack these
    // columns) and an already-populated one.
    add_column_if_missing(conn, "users", "password_hash", "TEXT")?;
    add_column_if_missing(conn, "users", "created_at", "TEXT")?;
    add_column_if_missing(conn, "repositories", "owner_id", "TEXT")?;

    // `sessions` reshape (ADR-002 Decision 2/6). The old shape carried a
    // `token`/`refresh_token`; those rows were never real (unauthenticated)
    // sessions, so drop+recreate. Detecting the old shape by column keeps
    // this a one-time migration — after the first boot the table already
    // has the new shape and is left untouched (so restarts don't wipe live
    // sessions).
    if table_exists(conn, "sessions")? && column_exists(conn, "sessions", "token")? {
        conn.execute_batch("DROP TABLE sessions;")?;
    }
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id         TEXT PRIMARY KEY,
            user_id    TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_used  TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id)
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
        "#,
    )?;

    // Backfill / normalize (ADR-002 Decision 6).
    let now = Utc::now().to_rfc3339();
    // Normalize the global role vocabulary to 'admin' | 'user'. The seeded
    // owner is the bootstrap admin seat; any other legacy role collapses to
    // 'user'.
    conn.execute(
        "UPDATE users SET role = 'admin' WHERE id = 'owner'",
        params![],
    )?;
    conn.execute(
        "UPDATE users SET role = 'user' WHERE role NOT IN ('admin', 'user')",
        params![],
    )?;
    conn.execute(
        "UPDATE users SET created_at = ?1 WHERE created_at IS NULL",
        params![now],
    )?;
    // Every pre-auth repository belongs to the bootstrap admin so nothing
    // becomes orphaned/invisible when auth flips on.
    conn.execute(
        "UPDATE repositories SET owner_id = 'owner' WHERE owner_id IS NULL",
        params![],
    )?;
    // Normalize the repo-scoped role vocabulary (the code used to seed a
    // stray 'Owner'; ADR-002 standardizes on lowercase 'owner').
    conn.execute(
        "UPDATE workspace_members SET role = 'owner' WHERE role = 'Owner'",
        params![],
    )?;
    // Backfill the owner membership row (ADR-002 Decision 6 — the P3.2 review
    // flagged this as missing). Every repository's workspace must carry an
    // `owner` `workspace_members` row for its `owner_id`, or the repo owner
    // would resolve to no repo-scoped role once the authz guard is live.
    // Idempotent: `INSERT OR IGNORE` no-ops when the row already exists (e.g.
    // the legacy hardcoded `('owner','owner')` row, which matches the
    // backfilled `owner_id = 'owner'`).
    //
    // Latent coupling (flagged in P5.1 review): this INSERT selects
    // `r.owner_id` straight into `workspace_members.user_id`, which has a
    // `FOREIGN KEY REFERENCES users(id)` — with `foreign_keys = ON` (P5.1)
    // that insert is rejected outright if the referenced user row doesn't
    // exist. Today that's always satisfied: every pre-auth repository's
    // `owner_id` was just backfilled to `'owner'` above, and `'owner'` is
    // guaranteed to exist by `seed_default_user` (called right after
    // `migrate()` in `Database::open`, and idempotently on every boot). Any
    // future change that reorders `seed_default_user` after `migrate()`, or
    // adds a `delete_user` that can remove a row still referenced by
    // `repositories.owner_id`, must preserve this referent or this backfill
    // (and the FK it relies on) breaks.
    conn.execute(
        "INSERT OR IGNORE INTO workspace_members (workspace_id, user_id, role) \
         SELECT w.id, r.owner_id, 'owner' \
         FROM workspaces w JOIN repositories r ON r.id = w.repository_id \
         WHERE r.owner_id IS NOT NULL",
        params![],
    )?;
    Ok(())
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        apply_pragmas(&conn)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        db.seed_default_user()?;
        Ok(db)
    }

    /// Runs every pending migration (see [`MIGRATIONS`] / [`run_migrations`]).
    /// A no-op once the database is already at the latest version — safe to
    /// call on every boot (and, as the existing tests do, more than once in a
    /// row).
    pub fn migrate(&self) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().expect("database lock");
        run_migrations(&mut conn, MIGRATIONS)
    }

    pub fn seed_default_user(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        // The seed owner starts with password_hash = NULL — an un-bootstrapped
        // `enabled` server has no one who can log in (fail-closed) until the
        // first-boot bootstrap (env or setup token) sets the password.
        conn.execute(
            "INSERT OR IGNORE INTO users (id, email, name, role, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "owner",
                "owner@zync.local",
                "Workspace Owner",
                "admin",
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    // ---- Auth: users ----

    /// Look up a user by email (case-insensitive) together with its password
    /// hash, for login verification. Returns `None` if no such user.
    pub fn user_with_hash_by_email(&self, email: &str) -> anyhow::Result<Option<UserWithHash>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT id, email, name, role, password_hash FROM users WHERE lower(email) = lower(?1)",
            params![email],
            user_with_hash_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    /// Load a user by id (used by the auth middleware and `/auth/me`).
    pub fn user_by_id(&self, id: &str) -> anyhow::Result<Option<User>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT id, email, name, role FROM users WHERE id = ?1",
            params![id],
            user_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    /// True iff at least one user has a non-NULL `password_hash` — i.e. the
    /// server has been bootstrapped (ADR-002 Decision 1). Used to gate the
    /// first-boot admin bootstrap.
    pub fn any_password_set(&self) -> anyhow::Result<bool> {
        let conn = self.conn.lock().expect("database lock");
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE password_hash IS NOT NULL",
            params![],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Set the bootstrap admin's email + password hash on the seed `owner` row
    /// and ensure it is `role = 'admin'`. Only ever called from the first-boot
    /// bootstrap while the server is still un-bootstrapped.
    pub fn set_admin_password(&self, email: &str, password_hash: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "UPDATE users SET email = ?2, password_hash = ?3, role = 'admin' WHERE id = 'owner'",
            params!["owner", email, password_hash],
        )?;
        Ok(())
    }

    /// Resolve a user by id or (case-insensitive) email — used to add a member
    /// "by user identifier" (ADR-002 Decision 5). Returns `None` if neither
    /// matches.
    pub fn find_user_by_identifier(&self, identifier: &str) -> anyhow::Result<Option<User>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT id, email, name, role FROM users WHERE id = ?1 OR lower(email) = lower(?1) LIMIT 1",
            params![identifier],
            user_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    /// Insert a user (no password — password-set is a separate bootstrap/admin
    /// path). Used by tests and the bootstrap seed.
    pub fn create_user(&self, id: &str, email: &str, name: &str, role: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT INTO users (id, email, name, role, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, email, name, role, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Insert an admin-created user with an initial password already hashed
    /// (P3.5 — `POST /auth/users`, ADR-002 Decision 1: "User creation is
    /// admin-only"). Unlike [`create_user`](Self::create_user), this always
    /// sets `password_hash` so the new user can log in immediately. The
    /// caller should already have checked the identifier is free (e.g. via
    /// [`find_user_by_identifier`](Self::find_user_by_identifier)) so the
    /// common case surfaces as a clean `409`; a concurrent insert that races
    /// past that pre-check instead trips the `email` UNIQUE constraint here,
    /// which is reported as [`UserConflict`] so the caller can still map it
    /// to `409` rather than a generic `500`.
    pub fn create_user_with_password(
        &self,
        id: &str,
        email: &str,
        name: &str,
        role: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT INTO users (id, email, name, role, password_hash, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                email,
                name,
                role,
                password_hash,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|err| {
            if is_unique_constraint_violation(&err) {
                anyhow::Error::new(UserConflict)
            } else {
                anyhow::Error::from(err)
            }
        })?;
        Ok(())
    }

    /// List every user (id/email/name/role/created_at, never
    /// `password_hash`) for the admin user-management UI (P3.5). Ordered by
    /// creation so the bootstrap admin appears first.
    pub fn list_users(&self) -> anyhow::Result<Vec<UserSummary>> {
        let conn = self.conn.lock().expect("database lock");
        let mut stmt = conn.prepare(
            "SELECT id, email, name, role, created_at FROM users ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(UserSummary {
                id: row.get(0)?,
                email: row.get(1)?,
                name: row.get(2)?,
                role: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // ---- Authz: repo membership (ADR-002 Decision 5) ----

    /// Resolve `user_id`'s effective repo-scoped role on `repository_id`, or
    /// `None` if they have no access. `owner_id` is authoritative for ownership
    /// (so the owner resolves to `owner` even if the membership row were somehow
    /// missing); otherwise the `workspace_members` role via the repo's workspace.
    /// A global `admin` is not consulted here — the guard grants admins full
    /// access before calling this.
    pub fn repo_role_for_user(
        &self,
        repository_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT role FROM ( \
               SELECT 'owner' AS role, 0 AS rank FROM repositories \
                 WHERE id = ?1 AND owner_id = ?2 \
               UNION ALL \
               SELECT wm.role, 1 AS rank FROM workspace_members wm \
                 JOIN workspaces w ON w.id = wm.workspace_id \
                 WHERE w.repository_id = ?1 AND wm.user_id = ?2 \
             ) ORDER BY rank LIMIT 1",
            params![repository_id, user_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
    }

    /// List the members of `repository_id`'s workspace, with each member's user
    /// display fields (owner/member management — ADR-002 Decision 5 / P3.5).
    pub fn list_repo_members(&self, repository_id: &str) -> anyhow::Result<Vec<RepoMember>> {
        let conn = self.conn.lock().expect("database lock");
        let mut stmt = conn.prepare(
            "SELECT wm.user_id, wm.role, u.email, u.name FROM workspace_members wm \
             JOIN workspaces w ON w.id = wm.workspace_id \
             LEFT JOIN users u ON u.id = wm.user_id \
             WHERE w.repository_id = ?1 \
             GROUP BY wm.user_id \
             ORDER BY wm.role, wm.user_id",
        )?;
        let rows = stmt.query_map(params![repository_id], |row| {
            Ok(RepoMember {
                user_id: row.get(0)?,
                role: row.get(1)?,
                email: row.get(2)?,
                name: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Add (or update the role of) a member on `repository_id`'s workspace(s).
    /// The caller must have ensured the workspace exists (via
    /// `workspace_for_repository`). Upserts so re-adding an existing member just
    /// changes their role.
    pub fn add_repo_member(
        &self,
        repository_id: &str,
        user_id: &str,
        role: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT INTO workspace_members (workspace_id, user_id, role) \
             SELECT id, ?2, ?3 FROM workspaces WHERE repository_id = ?1 \
             ON CONFLICT(workspace_id, user_id) DO UPDATE SET role = excluded.role",
            params![repository_id, user_id, role],
        )?;
        Ok(())
    }

    /// Change an existing member's role on `repository_id`. Returns the number of
    /// membership rows updated — `0` means the user was not a member (the caller
    /// surfaces that as a `404` rather than a misleading success).
    pub fn set_repo_member_role(
        &self,
        repository_id: &str,
        user_id: &str,
        role: &str,
    ) -> anyhow::Result<usize> {
        let conn = self.conn.lock().expect("database lock");
        let updated = conn.execute(
            "UPDATE workspace_members SET role = ?3 \
             WHERE user_id = ?2 AND workspace_id IN \
               (SELECT id FROM workspaces WHERE repository_id = ?1)",
            params![repository_id, user_id, role],
        )?;
        Ok(updated)
    }

    /// Remove a member from `repository_id`'s workspace(s).
    pub fn remove_repo_member(&self, repository_id: &str, user_id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "DELETE FROM workspace_members \
             WHERE user_id = ?2 AND workspace_id IN \
               (SELECT id FROM workspaces WHERE repository_id = ?1)",
            params![repository_id, user_id],
        )?;
        Ok(())
    }

    // ---- Auth: sessions ----

    /// Insert a freshly-minted session. `id` is `sha256(raw_token)` hex.
    pub fn create_session(
        &self,
        id: &str,
        user_id: &str,
        created_at: &str,
        last_used: &str,
        expires_at: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT INTO sessions (id, user_id, created_at, last_used, expires_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, user_id, created_at, last_used, expires_at],
        )?;
        Ok(())
    }

    /// Fetch a session by its hashed id. Expiry is enforced by the caller (the
    /// auth middleware), which parses the timestamps.
    pub fn session_by_id(&self, id: &str) -> anyhow::Result<Option<Session>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT id, user_id, created_at, last_used, expires_at FROM sessions WHERE id = ?1",
            params![id],
            |row| {
                Ok(Session {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    created_at: row.get(2)?,
                    last_used: row.get(3)?,
                    expires_at: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// Bump a session's sliding window (throttled — the middleware only calls
    /// this once the refresh window has elapsed).
    pub fn touch_session(&self, id: &str, last_used: &str, expires_at: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "UPDATE sessions SET last_used = ?2, expires_at = ?3 WHERE id = ?1",
            params![id, last_used, expires_at],
        )?;
        Ok(())
    }

    /// Delete a single session (logout, or opportunistic cleanup of an expired
    /// row on read).
    pub fn delete_session(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Background sweep: drop every session whose `expires_at` is at or before
    /// `now`. `now` must use the same fixed RFC3339 format the auth layer writes
    /// (seconds precision, `Z`) so the lexical comparison is monotonic.
    pub fn sweep_expired_sessions(&self, now: &str) -> anyhow::Result<usize> {
        let conn = self.conn.lock().expect("database lock");
        let removed = conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", params![now])?;
        Ok(removed)
    }

    pub fn list_repositories(&self) -> anyhow::Result<Vec<RepositoryRecord>> {
        let conn = self.conn.lock().expect("database lock");
        let mut stmt = conn.prepare(&format!(
            "SELECT {REPOSITORY_COLUMNS} FROM repositories ORDER BY favorite DESC, name ASC",
        ))?;
        let rows = stmt.query_map([], repository_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Repositories visible to `user_id` (ADR-002 Decision 5): those they own
    /// (`owner_id`) plus those they hold a `workspace_members` row on. A global
    /// `admin` should call [`list_repositories`] instead (it sees all).
    pub fn list_repositories_for_user(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<RepositoryRecord>> {
        let conn = self.conn.lock().expect("database lock");
        // DISTINCT because a repo the user both owns and is a member of would
        // otherwise appear twice through the membership join.
        let mut stmt = conn.prepare(&format!(
            "SELECT DISTINCT {columns} FROM repositories r \
             LEFT JOIN workspaces w ON w.repository_id = r.id \
             LEFT JOIN workspace_members wm ON wm.workspace_id = w.id AND wm.user_id = ?1 \
             WHERE r.owner_id = ?1 OR wm.user_id = ?1 \
             ORDER BY r.favorite DESC, r.name ASC",
            columns = REPOSITORY_COLUMNS
                .split(", ")
                .map(|c| format!("r.{c}"))
                .collect::<Vec<_>>()
                .join(", "),
        ))?;
        let rows = stmt.query_map(params![user_id], repository_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn create_repository(
        &self,
        name: &str,
        path: &str,
        remote_url: Option<&str>,
        owner_id: &str,
    ) -> anyhow::Result<RepositoryRecord> {
        let record = RepositoryRecord {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            path: path.to_string(),
            remote_url: remote_url.map(ToOwned::to_owned),
            favorite: false,
            created_at: Utc::now().to_rfc3339(),
            owner_id: Some(owner_id.to_string()),
        };
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT INTO repositories (id, name, path, remote_url, favorite, created_at, owner_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![record.id, record.name, record.path, record.remote_url, record.favorite as i64, record.created_at, record.owner_id],
        )?;
        Ok(record)
    }

    pub fn remove_repository(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        // workspaces/workspace_members reference repositories; clear them
        // first or the FK constraint rejects the delete.
        conn.execute(
            "DELETE FROM workspace_members WHERE workspace_id IN (SELECT id FROM workspaces WHERE repository_id = ?1)",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM workspaces WHERE repository_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM repositories WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "UPDATE repositories SET favorite = ?2 WHERE id = ?1",
            params![id, favorite as i64],
        )?;
        Ok(())
    }

    pub fn repository(&self, id: &str) -> anyhow::Result<Option<RepositoryRecord>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            &format!("SELECT {REPOSITORY_COLUMNS} FROM repositories WHERE id = ?1"),
            params![id],
            repository_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn repository_by_path(&self, path: &str) -> anyhow::Result<Option<RepositoryRecord>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            &format!("SELECT {REPOSITORY_COLUMNS} FROM repositories WHERE path = ?1"),
            params![path],
            repository_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn workspace(&self, id: &str) -> anyhow::Result<Option<WorkspaceRecord>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT id, repository_id, name, created_at FROM workspaces WHERE id = ?1",
            params![id],
            |row| {
                Ok(WorkspaceRecord {
                    id: row.get(0)?,
                    repository_id: row.get(1)?,
                    name: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn workspace_for_repository(
        &self,
        repository_id: &str,
        name: &str,
    ) -> anyhow::Result<WorkspaceRecord> {
        let conn = self.conn.lock().expect("database lock");
        if let Some(existing) = conn
            .query_row(
                "SELECT id, repository_id, name, created_at FROM workspaces WHERE repository_id = ?1 LIMIT 1",
                params![repository_id],
                |row| {
                    Ok(WorkspaceRecord {
                        id: row.get(0)?,
                        repository_id: row.get(1)?,
                        name: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                },
            )
            .optional()?
        {
            return Ok(existing);
        }

        let workspace = WorkspaceRecord {
            id: Uuid::new_v4().to_string(),
            repository_id: repository_id.to_string(),
            name: name.to_string(),
            created_at: Utc::now().to_rfc3339(),
        };
        conn.execute(
            "INSERT INTO workspaces (id, repository_id, name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                workspace.id,
                workspace.repository_id,
                workspace.name,
                workspace.created_at
            ],
        )?;
        // Seed the owner membership from the repository's real `owner_id`
        // (ADR-002 Decision 5) rather than the old hardcoded literal — so the
        // creator, not a synthetic `"owner"`, holds the owner seat. Falls back to
        // no row if `owner_id` is somehow NULL (a pre-backfill row); the
        // migration backfill and `create_repository` both guarantee it is set.
        conn.execute(
            "INSERT OR IGNORE INTO workspace_members (workspace_id, user_id, role) \
             SELECT ?1, owner_id, 'owner' FROM repositories WHERE id = ?2 AND owner_id IS NOT NULL",
            params![workspace.id, repository_id],
        )?;
        Ok(workspace)
    }

    /// Insert a new credential row. `secret_cipher`/`secret_nonce` must
    /// already be encrypted (see `crate::crypto`) — this method never sees
    /// plaintext.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_credential(
        &self,
        user_id: &str,
        label: &str,
        host_pattern: &str,
        kind: &str,
        username: Option<&str>,
        secret_cipher: &[u8],
        secret_nonce: &[u8],
    ) -> anyhow::Result<CredentialSummary> {
        let record = CredentialSummary {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            label: label.to_string(),
            host_pattern: host_pattern.to_string(),
            kind: kind.to_string(),
            username: username.map(ToOwned::to_owned),
            created_at: Utc::now().to_rfc3339(),
        };
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT INTO credentials (id, user_id, label, host_pattern, kind, username, secret_cipher, secret_nonce, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.user_id,
                record.label,
                record.host_pattern,
                record.kind,
                record.username,
                secret_cipher,
                secret_nonce,
                record.created_at,
            ],
        )?;
        Ok(record)
    }

    /// Masked projection for a user's credentials — never includes secret
    /// columns. This is the only shape the list/read API may return.
    pub fn list_credentials_by_user(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<CredentialSummary>> {
        let conn = self.conn.lock().expect("database lock");
        let mut stmt = conn.prepare(
            "SELECT id, user_id, label, host_pattern, kind, username, created_at \
             FROM credentials WHERE user_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(CredentialSummary {
                id: row.get(0)?,
                user_id: row.get(1)?,
                label: row.get(2)?,
                host_pattern: row.get(3)?,
                kind: row.get(4)?,
                username: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Full row including the encrypted secret bundle. Callers must decrypt
    /// just-in-time and drop the plaintext immediately — never cache or log
    /// it (see ADR-001 "Just-in-time decrypt, immediate drop"). Scoped by
    /// `user_id` so one user can never read another's row by guessing an id.
    pub fn get_decryptable(
        &self,
        id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<CredentialSecretRow>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT id, user_id, label, host_pattern, kind, username, secret_cipher, secret_nonce, created_at \
             FROM credentials WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
            |row| {
                Ok(CredentialSecretRow {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    label: row.get(2)?,
                    host_pattern: row.get(3)?,
                    kind: row.get(4)?,
                    username: row.get(5)?,
                    secret_cipher: row.get(6)?,
                    secret_nonce: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// Scoped by `user_id` so a delete for another user's credential id is a
    /// silent no-op rather than an IDOR (matters once real per-user auth
    /// lands — see ADR-001 / `credentials::DEFAULT_USER_ID` TODO).
    pub fn delete_credential(&self, id: &str, user_id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "DELETE FROM credentials WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )?;
        Ok(())
    }
}

fn repository_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepositoryRecord> {
    Ok(RepositoryRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        remote_url: row.get(3)?,
        favorite: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        owner_id: row.get(6)?,
    })
}

/// The shared column list for every `RepositoryRecord` SELECT, so the column
/// order stays in lockstep with [`repository_from_row`]'s positional `get`s.
const REPOSITORY_COLUMNS: &str = "id, name, path, remote_url, favorite, created_at, owner_id";

fn user_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        email: row.get(1)?,
        name: row.get(2)?,
        role: row.get(3)?,
    })
}

fn user_with_hash_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserWithHash> {
    Ok(UserWithHash {
        user: user_from_row(row)?,
        password_hash: row.get(4)?,
    })
}

fn table_exists(conn: &Connection, table: &str) -> anyhow::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> anyhow::Result<bool> {
    // PRAGMA table_info can't be parameterized, but `table` is only ever a
    // hard-coded literal from this module — never user input.
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Idempotent `ALTER TABLE ADD COLUMN`. SQLite errors if the column already
/// exists, so guard with a `PRAGMA table_info` check first. `table`/`column`/
/// `decl` are hard-coded literals from `migrate()`, never user input.
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> anyhow::Result<()> {
    if !column_exists(conn, table, column)? {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl};"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open(":memory:").expect("open in-memory db")
    }

    // ---- P5.1: pragmas + versioned migrations ----

    /// The pragmas set in `apply_pragmas` actually engage on a real,
    /// file-backed database (an in-memory database can never report `wal` —
    /// that path is exercised separately by every other test in this module,
    /// which all use `:memory:` and must keep working unaffected).
    #[test]
    fn pragmas_engage_wal_and_foreign_keys_on_a_file_backed_db() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("pragmas.db");
        let db = Database::open(&path).expect("open file-backed db");

        let conn = db.conn.lock().expect("database lock");
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal_mode");
        assert_eq!(journal_mode, "wal", "WAL must actually engage on disk");

        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign_keys");
        assert_eq!(foreign_keys, 1, "foreign_keys must be ON");

        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .expect("busy_timeout");
        assert_eq!(busy_timeout, 5000);

        let synchronous: i64 = conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .expect("synchronous");
        assert_eq!(synchronous, 1, "synchronous=NORMAL reports as 1");
    }

    /// A brand-new database runs migration 1 end to end and lands at the
    /// latest version with every table present.
    #[test]
    fn fresh_db_migrates_to_latest_version_with_all_tables() {
        let db = test_db();
        let conn = db.conn.lock().expect("database lock");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("user_version");
        assert_eq!(
            version,
            MIGRATIONS.last().unwrap().version,
            "fresh db lands exactly at the latest known migration version"
        );

        for table in [
            "users",
            "repositories",
            "workspaces",
            "workspace_members",
            "credentials",
            "sessions",
        ] {
            assert!(
                table_exists(&conn, table).unwrap(),
                "fresh db must have table `{table}`"
            );
        }
    }

    /// The trickiest case: a database that predates versioned migrations
    /// entirely — built by running migration 1's exact SQL directly, with no
    /// `PRAGMA user_version` ever stamped, exactly what every zync.db on disk
    /// from before this change looks like — must be recognized as
    /// already-baseline, stamped at version 1, and left with its data intact.
    /// No `CREATE TABLE`/`ALTER TABLE` may run destructively against it, and
    /// opening it must not error.
    #[test]
    fn existing_ad_hoc_schema_db_is_stamped_without_data_loss() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("legacy.db");

        {
            let conn = Connection::open(&path).expect("open legacy db");
            migration_001_baseline(&conn).expect("build legacy ad-hoc schema");
            conn.execute(
                "INSERT INTO users (id, email, name, role, password_hash, created_at) \
                 VALUES ('legacy-user', 'legacy@zync.local', 'Legacy User', 'admin', \
                 '$argon2id$legacyhash', ?1)",
                params![Utc::now().to_rfc3339()],
            )
            .expect("insert legacy user");
            conn.execute(
                "INSERT INTO repositories (id, name, path, remote_url, favorite, created_at, owner_id) \
                 VALUES ('legacy-repo', 'legacy', '/tmp/legacy', NULL, 0, ?1, 'legacy-user')",
                params![Utc::now().to_rfc3339()],
            )
            .expect("insert legacy repo");
            // Deliberately no `PRAGMA user_version` write: this file sits at
            // SQLite's implicit default of 0, same as any database that has
            // never had a versioned migration run against it.
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 0, "sanity check: legacy db starts unstamped");
        }

        let db =
            Database::open(&path).expect("opening an existing ad-hoc-schema db must not error");

        let conn = db.conn.lock().expect("database lock");
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            version, 1,
            "existing ad-hoc-schema db is stamped at the baseline version, not left unstamped"
        );
        drop(conn);

        let user = db
            .find_user_by_identifier("legacy-user")
            .unwrap()
            .expect("legacy user survives the upgrade");
        assert_eq!(user.email, "legacy@zync.local");
        let repo = db
            .repository("legacy-repo")
            .unwrap()
            .expect("legacy repo survives the upgrade");
        assert_eq!(repo.owner_id.as_deref(), Some("legacy-user"));
        assert_eq!(repo.path, "/tmp/legacy");
    }

    /// A migration that fails partway through must roll back its own writes
    /// and must not advance `user_version` — the next boot retries it from
    /// the last good version rather than resuming from a half-applied state.
    #[test]
    fn failing_migration_rolls_back_and_does_not_advance_version() {
        fn ok_step(conn: &Connection) -> anyhow::Result<()> {
            conn.execute_batch("CREATE TABLE step_one (id INTEGER PRIMARY KEY);")?;
            Ok(())
        }
        fn failing_step(conn: &Connection) -> anyhow::Result<()> {
            // Partial DDL before the deliberate failure — this must not
            // survive the rollback.
            conn.execute_batch("CREATE TABLE step_two (id INTEGER PRIMARY KEY);")?;
            anyhow::bail!("deliberate migration failure")
        }
        let migrations = [
            Migration {
                version: 1,
                name: "step one",
                apply: ok_step,
            },
            Migration {
                version: 2,
                name: "step two (fails)",
                apply: failing_step,
            },
        ];

        let mut conn = Connection::open_in_memory().expect("open in-memory db");
        let result = run_migrations(&mut conn, &migrations);
        assert!(result.is_err(), "a failing migration must return Err");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            version, 1,
            "version stops at the last successfully applied migration"
        );
        assert!(
            table_exists(&conn, "step_one").unwrap(),
            "the successful migration's DDL persists"
        );
        assert!(
            !table_exists(&conn, "step_two").unwrap(),
            "the failing migration's DDL must be rolled back, not partially applied"
        );

        // Retrying with the same migration list resumes from version 1 and
        // fails again the same way — it doesn't skip the broken migration.
        let retry = run_migrations(&mut conn, &migrations);
        assert!(
            retry.is_err(),
            "retrying a still-broken migration still refuses to boot"
        );
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    /// Positive path of the one-time `sessions` reshape (ADR-002 Decision
    /// 2/6): a real OLD token-shaped `sessions` table (with a `token` column,
    /// the pre-auth shape) plus a live row must be reshaped to the new
    /// `id`/`expires_at` shape, and the stale old-shape row must not survive
    /// — it belonged to a schema with no sliding-expiry columns, so it can't
    /// be carried forward meaningfully.
    #[test]
    fn sessions_reshape_guard_drops_the_old_token_shape() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE users (
                 id TEXT PRIMARY KEY,
                 email TEXT NOT NULL UNIQUE,
                 name TEXT NOT NULL,
                 role TEXT NOT NULL
             );
             CREATE TABLE sessions (
                 id TEXT PRIMARY KEY,
                 user_id TEXT NOT NULL,
                 token TEXT NOT NULL,
                 refresh_token TEXT,
                 created_at TEXT NOT NULL
             );
             INSERT INTO users (id, email, name, role)
                 VALUES ('owner', 'owner@zync.local', 'Owner', 'admin');
             INSERT INTO sessions (id, user_id, token, refresh_token, created_at)
                 VALUES ('stale-old-session', 'owner', 'stale-raw-token', NULL, '2024-01-01T00:00:00Z');",
        )
        .expect("seed a real old token-shaped sessions table + row");

        migration_001_baseline(&conn).expect("migration must reshape the old sessions table");

        assert!(
            column_exists(&conn, "sessions", "id").unwrap()
                && column_exists(&conn, "sessions", "expires_at").unwrap(),
            "sessions table must have the new shape (id/expires_at) after migration"
        );
        assert!(
            !column_exists(&conn, "sessions", "token").unwrap(),
            "the old `token` column must be gone after the reshape"
        );
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            count, 0,
            "the stale old-shape session row must not survive the one-time reshape"
        );
    }

    /// The other half of the same guard property: a `sessions` table already
    /// in the NEW (current) shape, holding a real live session row, must
    /// NOT be dropped by a repeated migration run. This is the regression
    /// the Warning called out — weakening the guard to `table_exists`
    /// alone (dropping the check that it's specifically the *old* shape)
    /// would wipe every live session on every boot, and only this test
    /// would catch it: the fresh-db and existing-ad-hoc-schema tests above
    /// never insert a session row through the new shape.
    #[test]
    fn sessions_reshape_guard_never_drops_a_current_shape_table() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        migration_001_baseline(&conn).expect("build baseline schema");
        conn.execute(
            "INSERT INTO users (id, email, name, role, created_at) \
             VALUES ('bob', 'bob@zync.local', 'Bob', 'user', ?1)",
            params![Utc::now().to_rfc3339()],
        )
        .expect("insert user");
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, user_id, created_at, last_used, expires_at) \
             VALUES ('live-session', 'bob', ?1, ?1, ?1)",
            params![now],
        )
        .expect("insert a live, current-shape session row");

        // Simulate a later boot re-running the same migration body directly.
        // In production this specific call is skipped once `PRAGMA
        // user_version` is stamped (see `run_migrations`'s version gate),
        // but that gate is a separate safety net — this test pins the
        // guard's own property regardless of it: it must key off the
        // table's *shape* (the `token` column), not merely its existence.
        migration_001_baseline(&conn).expect("re-running the migration body must not error");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id = 'live-session'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "a current-shape session row must survive a repeated migration run"
        );
    }

    /// P4.3 closing-pass regression test: `UserWithHash` used to derive a plain `Debug` that would
    /// print `password_hash` verbatim on a stray `{:?}` (log line, panic message, `expect`
    /// failure) — the one secret-bearing type in this file that didn't follow the hand-written-
    /// redacting-`Debug` convention every other one here does (`CredentialSecretRow`, etc.). Pins
    /// the fix with a known sentinel standing in for an argon2id hash.
    #[test]
    fn user_with_hash_debug_redacts_password_hash() {
        const SENTINEL: &str = "SENTINEL_SECRET_bkq9";
        let with_hash = UserWithHash {
            user: User {
                id: "owner".to_string(),
                email: "owner@zync.local".to_string(),
                name: "Owner".to_string(),
                role: "admin".to_string(),
            },
            password_hash: Some(SENTINEL.to_string()),
        };
        let debug = format!("{with_hash:?}");
        assert!(
            !debug.contains(SENTINEL),
            "UserWithHash Debug must not print password_hash: {debug}"
        );
    }

    #[test]
    fn delete_credential_is_scoped_by_user() {
        let db = test_db();
        let record = db
            .insert_credential(
                "owner",
                "GitHub PAT",
                "github.com",
                "https_token",
                None,
                b"cipher",
                b"nonce",
            )
            .expect("insert");

        // A delete issued as a different user must be a silent no-op — the
        // row must survive (this is the IDOR guard: an attacker who guesses
        // another user's credential id can't delete it).
        db.delete_credential(&record.id, "someone-else")
            .expect("delete as wrong user should not error");
        assert!(
            db.get_decryptable(&record.id, "owner")
                .expect("get_decryptable")
                .is_some(),
            "row must survive a delete issued by a different user_id"
        );

        // The owning user's delete actually removes it.
        db.delete_credential(&record.id, "owner")
            .expect("delete as owner");
        assert!(db
            .get_decryptable(&record.id, "owner")
            .expect("get_decryptable")
            .is_none());
    }

    /// A freshly created repo + workspace yields an `owner` membership for the
    /// creator, and `repo_role_for_user` resolves each role (or `None` for a
    /// stranger). This is the data-layer core of the authz guard.
    #[test]
    fn repo_role_for_user_resolves_ownership_and_membership() {
        let db = test_db();
        db.create_user("bob", "bob@z", "Bob", "user").unwrap();
        db.create_user("mem", "mem@z", "Mem", "user").unwrap();
        db.create_user("vwr", "vwr@z", "Vwr", "user").unwrap();
        let repo = db.create_repository("p", "/tmp/p", None, "bob").unwrap();
        db.workspace_for_repository(&repo.id, &repo.name).unwrap();
        db.add_repo_member(&repo.id, "mem", "member").unwrap();
        db.add_repo_member(&repo.id, "vwr", "viewer").unwrap();

        assert_eq!(
            db.repo_role_for_user(&repo.id, "bob").unwrap().as_deref(),
            Some("owner"),
            "creator holds the owner seat"
        );
        assert_eq!(
            db.repo_role_for_user(&repo.id, "mem").unwrap().as_deref(),
            Some("member")
        );
        assert_eq!(
            db.repo_role_for_user(&repo.id, "vwr").unwrap().as_deref(),
            Some("viewer")
        );
        assert_eq!(db.repo_role_for_user(&repo.id, "out").unwrap(), None);
    }

    /// The owner membership backfill is idempotent: running `migrate()` again
    /// (as every process restart does) neither errors nor duplicates the row,
    /// and the owner still resolves to `owner`.
    #[test]
    fn owner_membership_backfill_is_idempotent() {
        let db = test_db();
        db.create_user("bob", "bob@z", "Bob", "user").unwrap();
        let repo = db.create_repository("p", "/tmp/p", None, "bob").unwrap();
        db.workspace_for_repository(&repo.id, &repo.name).unwrap();

        db.migrate().unwrap();
        db.migrate().unwrap();

        let members = db.list_repo_members(&repo.id).unwrap();
        let owners: Vec<_> = members
            .iter()
            .filter(|m| m.user_id == "bob" && m.role == "owner")
            .collect();
        assert_eq!(
            owners.len(),
            1,
            "exactly one owner row after repeated migrate"
        );
        assert_eq!(
            db.repo_role_for_user(&repo.id, "bob").unwrap().as_deref(),
            Some("owner")
        );
    }

    #[test]
    fn list_repositories_for_user_filters_by_access() {
        let db = test_db();
        db.create_user("bob", "bob@z", "Bob", "user").unwrap();
        db.create_user("out", "out@z", "Out", "user").unwrap();
        let owned = db
            .create_repository("own", "/tmp/own", None, "bob")
            .unwrap();
        db.workspace_for_repository(&owned.id, &owned.name).unwrap();
        let shared = db
            .create_repository("shr", "/tmp/shr", None, "out")
            .unwrap();
        db.workspace_for_repository(&shared.id, &shared.name)
            .unwrap();
        db.add_repo_member(&shared.id, "bob", "viewer").unwrap();
        let hidden = db
            .create_repository("hid", "/tmp/hid", None, "out")
            .unwrap();
        db.workspace_for_repository(&hidden.id, &hidden.name)
            .unwrap();

        let visible: Vec<_> = db
            .list_repositories_for_user("bob")
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert!(visible.contains(&owned.id), "sees owned repo");
        assert!(visible.contains(&shared.id), "sees shared (member) repo");
        assert!(
            !visible.contains(&hidden.id),
            "cannot see a repo it has no role on"
        );
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn member_add_update_remove_round_trip() {
        let db = test_db();
        db.create_user("bob", "bob@z", "Bob", "user").unwrap();
        db.create_user("mem", "mem@z", "Mem", "user").unwrap();
        let repo = db.create_repository("p", "/tmp/p", None, "bob").unwrap();
        db.workspace_for_repository(&repo.id, &repo.name).unwrap();

        db.add_repo_member(&repo.id, "mem", "viewer").unwrap();
        assert_eq!(
            db.repo_role_for_user(&repo.id, "mem").unwrap().as_deref(),
            Some("viewer")
        );
        // Re-add upserts the role.
        db.add_repo_member(&repo.id, "mem", "member").unwrap();
        assert_eq!(
            db.repo_role_for_user(&repo.id, "mem").unwrap().as_deref(),
            Some("member")
        );
        // Explicit role change reports one row updated.
        assert_eq!(
            db.set_repo_member_role(&repo.id, "mem", "viewer").unwrap(),
            1
        );
        assert_eq!(
            db.repo_role_for_user(&repo.id, "mem").unwrap().as_deref(),
            Some("viewer")
        );
        // N5: changing the role of a non-member updates zero rows — the handler
        // maps that to a 404 rather than a misleading success.
        assert_eq!(
            db.set_repo_member_role(&repo.id, "ghost", "member")
                .unwrap(),
            0
        );
        // Removal revokes access.
        db.remove_repo_member(&repo.id, "mem").unwrap();
        assert_eq!(db.repo_role_for_user(&repo.id, "mem").unwrap(), None);
    }

    /// `create_user_with_password` sets a real password hash (unlike the bare
    /// `create_user` test helper) and the row shows up in `list_users` without
    /// ever exposing the hash (P3.5).
    #[test]
    fn create_user_with_password_is_listed_without_hash() {
        let db = test_db();
        db.create_user_with_password("u1", "u1@zync.local", "User One", "user", "$argon2id$fake")
            .expect("create_user_with_password");

        let users = db.list_users().expect("list_users");
        let created = users
            .iter()
            .find(|u| u.id == "u1")
            .expect("new user is listed");
        assert_eq!(created.email, "u1@zync.local");
        assert_eq!(created.name, "User One");
        assert_eq!(created.role, "user");
        assert!(!created.created_at.is_empty());

        // The seeded owner is listed too, and the password hash never leaks
        // through `UserSummary` (the struct has no such field to leak).
        assert!(users.iter().any(|u| u.id == "owner"));

        // The hash is real and only reachable via the by-email lookup used
        // for login — not via list_users.
        let with_hash = db
            .user_with_hash_by_email("u1@zync.local")
            .expect("user_with_hash_by_email")
            .expect("user exists");
        assert_eq!(with_hash.password_hash.as_deref(), Some("$argon2id$fake"));
    }

    #[test]
    fn find_user_by_identifier_matches_id_or_email() {
        let db = test_db();
        db.create_user("bob", "bob@zync.local", "Bob", "user")
            .unwrap();
        assert_eq!(
            db.find_user_by_identifier("bob").unwrap().map(|u| u.id),
            Some("bob".to_string())
        );
        assert_eq!(
            db.find_user_by_identifier("BOB@ZYNC.LOCAL")
                .unwrap()
                .map(|u| u.id),
            Some("bob".to_string())
        );
        assert!(db.find_user_by_identifier("ghost").unwrap().is_none());
    }

    #[test]
    fn get_decryptable_is_scoped_by_user() {
        let db = test_db();
        let record = db
            .insert_credential(
                "owner",
                "GitHub PAT",
                "github.com",
                "https_token",
                None,
                b"cipher",
                b"nonce",
            )
            .expect("insert");

        assert!(db
            .get_decryptable(&record.id, "someone-else")
            .expect("get_decryptable")
            .is_none());
        assert!(db
            .get_decryptable(&record.id, "owner")
            .expect("get_decryptable")
            .is_some());
    }
}
