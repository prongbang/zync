use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepositoryRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub remote_url: Option<String>,
    pub favorite: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub id: String,
    pub repository_id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepositoryWithWorkspace {
    pub repository: RepositoryRecord,
    pub workspace: WorkspaceRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub ignored: bool,
    pub conflicted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommitSummary {
    pub id: String,
    pub summary: String,
    pub author: String,
    pub time: i64,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchSummary {
    pub name: String,
    pub is_head: bool,
    pub kind: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresenceUser {
    pub user_id: String,
    pub name: String,
    pub current_file: Option<String>,
    pub cursor_line: Option<u32>,
    pub cursor_column: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceResponse {
    pub workspace: WorkspaceRecord,
    pub repository: RepositoryRecord,
    pub files: Vec<FileNode>,
    pub online_users: Vec<PresenceUser>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateRepositoryRequest {
    pub name: Option<String>,
    pub path: Option<String>,
    pub remote_url: Option<String>,
    pub clone_to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitRequest {
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub amend: bool,
    pub sign_off: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilesRequest {
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WriteFileRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RebaseStepRequest {
    pub commit: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InteractiveRebaseRequest {
    pub base: String,
    pub steps: Vec<RebaseStepRequest>,
}

#[derive(Debug, Clone)]
pub struct ZyncApi {
    pub base_url: String,
}

impl Default for ZyncApi {
    #[cfg(not(target_arch = "wasm32"))]
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:58271".to_string(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn default() -> Self {
        let Some(window) = web_sys::window() else {
            return Self {
                base_url: "http://127.0.0.1:58271".to_string(),
            };
        };
        let location = window.location();
        let protocol = location
            .protocol()
            .ok()
            .filter(|protocol| protocol == "https:")
            .unwrap_or_else(|| "http:".to_string());
        let hostname = location
            .hostname()
            .ok()
            .filter(|hostname| !hostname.is_empty())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        Self {
            base_url: format!("{protocol}//{hostname}:58271"),
        }
    }
}

impl ZyncApi {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub async fn repositories(&self) -> Result<Vec<RepositoryRecord>, String> {
        get_json(&self.url("/repositories")).await
    }

    pub async fn create_repository(
        &self,
        request: &CreateRepositoryRequest,
    ) -> Result<RepositoryWithWorkspace, String> {
        post_json(&self.url("/repositories"), request).await
    }

    pub async fn open_repository(&self, id: &str) -> Result<RepositoryWithWorkspace, String> {
        post_json(
            &self.url(&format!("/repositories/{id}/open")),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn workspace(&self, id: &str) -> Result<WorkspaceResponse, String> {
        get_json(&self.url(&format!("/workspace/{id}"))).await
    }

    pub async fn status(&self, repository_id: &str) -> Result<Vec<FileStatus>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/status"))).await
    }

    pub async fn branches(&self, repository_id: &str) -> Result<Vec<BranchSummary>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/branches"))).await
    }

    pub async fn graph(&self, repository_id: &str) -> Result<Vec<CommitSummary>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/graph"))).await
    }

    pub async fn diff_workdir(&self, repository_id: &str) -> Result<String, String> {
        get_text(&self.url(&format!("/repositories/{repository_id}/git/diff/workdir"))).await
    }

    pub async fn diff_workdir_file(
        &self,
        repository_id: &str,
        path: &str,
    ) -> Result<String, String> {
        get_text(&self.url(&format!(
            "/repositories/{repository_id}/git/diff/workdir?path={}",
            urlencoding::encode(path)
        )))
        .await
    }

    pub async fn diff_staged_file(
        &self,
        repository_id: &str,
        path: &str,
    ) -> Result<String, String> {
        get_text(&self.url(&format!(
            "/repositories/{repository_id}/git/diff/staged?path={}",
            urlencoding::encode(path)
        )))
        .await
    }

    pub async fn read_file(&self, workspace_id: &str, path: &str) -> Result<FileContent, String> {
        get_json(&self.url(&format!(
            "/workspace/{workspace_id}/files/{}",
            urlencoding::encode(path)
        )))
        .await
    }

    pub async fn write_file(
        &self,
        workspace_id: &str,
        path: &str,
        content: String,
    ) -> Result<(), String> {
        put_empty(
            &self.url(&format!(
                "/workspace/{workspace_id}/files/{}",
                urlencoding::encode(path)
            )),
            &WriteFileRequest { content },
        )
        .await
    }

    pub async fn stage_files(&self, repository_id: &str, files: Vec<String>) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/add")),
            &FilesRequest { files },
        )
        .await
    }

    pub async fn unstage_files(
        &self,
        repository_id: &str,
        files: Vec<String>,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/unstage")),
            &FilesRequest { files },
        )
        .await
    }

    pub async fn discard_files(
        &self,
        repository_id: &str,
        files: Vec<String>,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/discard")),
            &FilesRequest { files },
        )
        .await
    }

    pub async fn commit(
        &self,
        repository_id: &str,
        request: &CommitRequest,
    ) -> Result<serde_json::Value, String> {
        post_json(
            &self.url(&format!("/repositories/{repository_id}/git/commit")),
            request,
        )
        .await
    }

    pub fn websocket_url(&self, workspace_id: &str) -> String {
        let base = self
            .base_url
            .trim_end_matches('/')
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        format!("{base}/ws/workspace/{workspace_id}")
    }

    pub async fn interactive_rebase(
        &self,
        repository_id: &str,
        request: &InteractiveRebaseRequest,
    ) -> Result<serde_json::Value, String> {
        post_json(
            &self.url(&format!(
                "/repositories/{repository_id}/git/rebase/interactive"
            )),
            request,
        )
        .await
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }
}

#[cfg(target_arch = "wasm32")]
async fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn get_json<T: for<'de> Deserialize<'de>>(_url: &str) -> Result<T, String> {
    Err("ZyncApi network calls are available in wasm32 browser builds".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn get_text(url: &str) -> Result<String, String> {
    gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .text()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn get_text(_url: &str) -> Result<String, String> {
    Err("ZyncApi network calls are available in wasm32 browser builds".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn post_json<T, R>(url: &str, body: &T) -> Result<R, String>
where
    T: Serialize,
    R: for<'de> Deserialize<'de>,
{
    gloo_net::http::Request::post(url)
        .json(body)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn post_json<T, R>(_url: &str, _body: &T) -> Result<R, String>
where
    T: Serialize,
    R: for<'de> Deserialize<'de>,
{
    Err("ZyncApi network calls are available in wasm32 browser builds".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn post_empty<T>(url: &str, body: &T) -> Result<(), String>
where
    T: Serialize,
{
    let response = gloo_net::http::Request::post(url)
        .json(body)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.ok() {
        Ok(())
    } else {
        Err(format!("request failed with status {}", response.status()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn post_empty<T>(_url: &str, _body: &T) -> Result<(), String>
where
    T: Serialize,
{
    Err("ZyncApi network calls are available in wasm32 browser builds".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn put_empty<T>(url: &str, body: &T) -> Result<(), String>
where
    T: Serialize,
{
    let response = gloo_net::http::Request::put(url)
        .json(body)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.ok() {
        Ok(())
    } else {
        Err(format!("request failed with status {}", response.status()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn put_empty<T>(_url: &str, _body: &T) -> Result<(), String>
where
    T: Serialize,
{
    Err("ZyncApi network calls are available in wasm32 browser builds".to_string())
}
