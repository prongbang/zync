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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub created_at: String,
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

            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                refresh_token TEXT NOT NULL,
                user_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
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
        Ok(())
    }

    pub fn seed_default_user(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT OR IGNORE INTO users (id, email, name, role) VALUES (?1, ?2, ?3, ?4)",
            params!["owner", "owner@zync.local", "Workspace Owner", "Owner"],
        )?;
        Ok(())
    }

    pub fn login(&self, email: &str, name: Option<&str>) -> anyhow::Result<(User, SessionRecord)> {
        let id = Uuid::new_v4().to_string();
        let display_name = name.unwrap_or(email);
        let conn = self.conn.lock().expect("database lock");
        conn.execute(
            "INSERT OR IGNORE INTO users (id, email, name, role) VALUES (?1, ?2, ?3, ?4)",
            params![id, email, display_name, "Developer"],
        )?;
        let user = conn.query_row(
            "SELECT id, email, name, role FROM users WHERE email = ?1",
            params![email],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    name: row.get(2)?,
                    role: row.get(3)?,
                })
            },
        )?;
        let session = SessionRecord {
            token: Uuid::new_v4().to_string(),
            refresh_token: Uuid::new_v4().to_string(),
            user_id: user.id.clone(),
            created_at: Utc::now().to_rfc3339(),
        };
        conn.execute(
            "INSERT INTO sessions (token, refresh_token, user_id, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![session.token, session.refresh_token, session.user_id, session.created_at],
        )?;
        Ok((user, session))
    }

    pub fn logout(&self, token: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().expect("database lock");
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
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
            params![workspace.id, "owner", "Owner"],
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
