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
/// server-side; only [`User`] is ever returned over HTTP.
#[derive(Debug, Clone)]
pub struct UserWithHash {
    pub user: User,
    pub password_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub remote_url: Option<String>,
    pub favorite: bool,
    pub created_at: String,
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

impl Database {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        db.seed_default_user()?;
        Ok(db)
    }

    pub fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
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
        add_column_if_missing(&conn, "users", "password_hash", "TEXT")?;
        add_column_if_missing(&conn, "users", "created_at", "TEXT")?;
        add_column_if_missing(&conn, "repositories", "owner_id", "TEXT")?;

        // `sessions` reshape (ADR-002 Decision 2/6). The old shape carried a
        // `token`/`refresh_token`; those rows were never real (unauthenticated)
        // sessions, so drop+recreate. Detecting the old shape by column keeps
        // this a one-time migration — after the first boot the table already
        // has the new shape and is left untouched (so restarts don't wipe live
        // sessions).
        if table_exists(&conn, "sessions")? && column_exists(&conn, "sessions", "token")? {
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
        Ok(())
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
        let removed = conn.execute(
            "DELETE FROM sessions WHERE expires_at <= ?1",
            params![now],
        )?;
        Ok(removed)
    }

    pub fn list_repositories(&self) -> anyhow::Result<Vec<RepositoryRecord>> {
        let conn = self.conn.lock().expect("database lock");
        let mut stmt = conn.prepare(
            "SELECT id, name, path, remote_url, favorite, created_at FROM repositories ORDER BY favorite DESC, name ASC",
        )?;
        let rows = stmt.query_map([], repository_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn create_repository(
        &self,
        name: &str,
        path: &str,
        remote_url: Option<&str>,
    ) -> anyhow::Result<RepositoryRecord> {
        let record = RepositoryRecord {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            path: path.to_string(),
            remote_url: remote_url.map(ToOwned::to_owned),
            favorite: false,
            created_at: Utc::now().to_rfc3339(),
        };
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT INTO repositories (id, name, path, remote_url, favorite, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![record.id, record.name, record.path, record.remote_url, record.favorite as i64, record.created_at],
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
            "SELECT id, name, path, remote_url, favorite, created_at FROM repositories WHERE id = ?1",
            params![id],
            repository_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn repository_by_path(&self, path: &str) -> anyhow::Result<Option<RepositoryRecord>> {
        let conn = self.conn.lock().expect("database lock");
        conn.query_row(
            "SELECT id, name, path, remote_url, favorite, created_at FROM repositories WHERE path = ?1",
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
        conn.execute(
            "INSERT OR IGNORE INTO workspace_members (workspace_id, user_id, role) VALUES (?1, ?2, ?3)",
            params![workspace.id, "owner", "owner"],
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
    pub fn list_credentials_by_user(&self, user_id: &str) -> anyhow::Result<Vec<CredentialSummary>> {
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
    })
}

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
