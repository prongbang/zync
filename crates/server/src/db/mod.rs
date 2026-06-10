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
