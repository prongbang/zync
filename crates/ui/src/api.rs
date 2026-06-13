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
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DirectoryList {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub directories: Vec<DirectoryEntry>,
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
pub struct TagSummary {
    pub name: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteSummary {
    pub name: String,
    pub url: Option<String>,
    pub push_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlameLine {
    pub start_line: usize,
    pub line_count: usize,
    pub commit: String,
    pub author: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeEntrySummary {
    pub path: String,
    pub kind: String,
    pub id: String,
    pub size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflogEntrySummary {
    pub index: usize,
    pub old_id: String,
    pub new_id: String,
    pub message: String,
    pub committer: String,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmoduleSummary {
    pub name: String,
    pub path: String,
    pub url: Option<String>,
    pub head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LfsSummary {
    pub configured: bool,
    pub tracked_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StashSummary {
    pub index: usize,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictSummary {
    pub ancestor: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ConflictDetail {
    pub path: String,
    pub ancestor_path: Option<String>,
    pub ours_path: Option<String>,
    pub theirs_path: Option<String>,
    pub ancestor_content: String,
    pub ours_content: String,
    pub theirs_content: String,
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
pub struct PatchRequest {
    pub patch: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WriteFileRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateFileRequest {
    pub path: String,
    pub content: Option<String>,
    pub is_dir: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenameFileRequest {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BranchRequest {
    pub name: String,
    pub new_name: Option<String>,
    pub checkout: Option<bool>,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteRequest {
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LfsRequest {
    pub pattern: Option<String>,
    pub remote: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevisionRequest {
    pub revision: String,
    pub hard: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagRequest {
    pub name: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommitIdRequest {
    pub commit: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CherryPickRequest {
    pub commits: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictResolveRequest {
    pub path: String,
    pub side: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RebaseStepRequest {
    pub commit: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InteractiveRebaseRequest {
    pub base: String,
    pub steps: Vec<RebaseStepRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StashRequest {
    pub message: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub index: Option<usize>,
    pub pop: Option<bool>,
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

    pub async fn directories(&self, path: Option<&str>) -> Result<DirectoryList, String> {
        let path = path.unwrap_or("").trim();
        if path.is_empty() {
            get_json(&self.url("/directories")).await
        } else {
            get_json(&self.url(&format!("/directories?path={}", urlencoding::encode(path)))).await
        }
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
        self.graph_with_limit(repository_id, 500).await
    }

    pub async fn graph_with_limit(
        &self,
        repository_id: &str,
        limit: usize,
    ) -> Result<Vec<CommitSummary>, String> {
        get_json(&self.url(&format!(
            "/repositories/{repository_id}/git/graph?limit={limit}"
        )))
        .await
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

    pub async fn diff_commit(
        &self,
        repository_id: &str,
        commit_id: &str,
    ) -> Result<String, String> {
        get_text(&self.url(&format!(
            "/repositories/{repository_id}/git/diff/commit/{commit_id}"
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

    pub fn asset_url(&self, workspace_id: &str, path: &str) -> String {
        self.url(&format!(
            "/workspace/{workspace_id}/assets/{}",
            urlencoding::encode(path)
        ))
    }

    pub fn blob_url(&self, repository_id: &str, revision: &str, path: &str) -> String {
        self.url(&format!(
            "/repositories/{repository_id}/git/blob?revision={}&path={}",
            urlencoding::encode(revision),
            urlencoding::encode(path)
        ))
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

    pub async fn create_file(
        &self,
        workspace_id: &str,
        path: &str,
        is_dir: bool,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/workspace/{workspace_id}/files")),
            &CreateFileRequest {
                path: path.to_string(),
                content: None,
                is_dir: Some(is_dir),
            },
        )
        .await
    }

    pub async fn rename_file(
        &self,
        workspace_id: &str,
        old_path: &str,
        new_path: &str,
    ) -> Result<(), String> {
        put_empty(
            &self.url(&format!("/workspace/{workspace_id}/files/rename")),
            &RenameFileRequest {
                old_path: old_path.to_string(),
                new_path: new_path.to_string(),
            },
        )
        .await
    }

    pub async fn delete_file(&self, workspace_id: &str, path: &str) -> Result<(), String> {
        delete_empty(&self.url(&format!(
            "/workspace/{workspace_id}/files/{}",
            urlencoding::encode(path)
        )))
        .await
    }

    pub async fn search_files(
        &self,
        workspace_id: &str,
        query: &str,
    ) -> Result<Vec<FileNode>, String> {
        get_json(&self.url(&format!(
            "/workspace/{workspace_id}/files/search?q={}",
            urlencoding::encode(query)
        )))
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

    pub async fn stage_patch(&self, repository_id: &str, patch: String) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/stage-patch")),
            &PatchRequest { patch },
        )
        .await
    }

    pub async fn checkout_branch(&self, repository_id: &str, name: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/checkout")),
            &BranchRequest {
                name: name.to_string(),
                new_name: None,
                checkout: None,
                revision: None,
            },
        )
        .await
    }

    pub async fn merge_branch(&self, repository_id: &str, name: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/branches/merge")),
            &BranchRequest {
                name: name.to_string(),
                new_name: None,
                checkout: None,
                revision: None,
            },
        )
        .await
    }

    pub async fn delete_branch(&self, repository_id: &str, name: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!(
                "/repositories/{repository_id}/git/branches/delete"
            )),
            &BranchRequest {
                name: name.to_string(),
                new_name: None,
                checkout: None,
                revision: None,
            },
        )
        .await
    }

    pub async fn create_branch(
        &self,
        repository_id: &str,
        name: &str,
        checkout: bool,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/branches")),
            &BranchRequest {
                name: name.to_string(),
                new_name: None,
                checkout: Some(checkout),
                revision: None,
            },
        )
        .await
    }

    pub async fn create_branch_at(
        &self,
        repository_id: &str,
        name: &str,
        revision: &str,
        checkout: bool,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/branches")),
            &BranchRequest {
                name: name.to_string(),
                new_name: None,
                checkout: Some(checkout),
                revision: Some(revision.to_string()),
            },
        )
        .await
    }

    pub async fn rename_branch(
        &self,
        repository_id: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!(
                "/repositories/{repository_id}/git/branches/rename"
            )),
            &BranchRequest {
                name: old_name.to_string(),
                new_name: Some(new_name.to_string()),
                checkout: None,
                revision: None,
            },
        )
        .await
    }

    pub async fn checkout_revision(
        &self,
        repository_id: &str,
        revision: &str,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!(
                "/repositories/{repository_id}/git/checkout/revision"
            )),
            &RevisionRequest {
                revision: revision.to_string(),
                hard: None,
            },
        )
        .await
    }

    pub async fn revert_commit(&self, repository_id: &str, commit: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/revert")),
            &CommitIdRequest {
                commit: commit.to_string(),
            },
        )
        .await
    }

    pub async fn tags(&self, repository_id: &str) -> Result<Vec<TagSummary>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/tags"))).await
    }

    pub async fn create_tag(
        &self,
        repository_id: &str,
        name: &str,
        target: Option<&str>,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/tags")),
            &TagRequest {
                name: name.to_string(),
                target: target.map(ToOwned::to_owned),
            },
        )
        .await
    }

    pub async fn delete_tag(&self, repository_id: &str, name: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/tags/delete")),
            &TagRequest {
                name: name.to_string(),
                target: None,
            },
        )
        .await
    }

    pub async fn remotes(&self, repository_id: &str) -> Result<Vec<RemoteSummary>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/remotes"))).await
    }

    pub async fn add_remote(
        &self,
        repository_id: &str,
        name: &str,
        url: &str,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/remotes")),
            &RemoteRequest {
                remote: Some(name.to_string()),
                branch: None,
                url: Some(url.to_string()),
            },
        )
        .await
    }

    pub async fn delete_remote(&self, repository_id: &str, name: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/remotes/delete")),
            &RemoteRequest {
                remote: Some(name.to_string()),
                branch: None,
                url: None,
            },
        )
        .await
    }

    pub async fn prune_remote(&self, repository_id: &str, name: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/remotes/prune")),
            &RemoteRequest {
                remote: Some(name.to_string()),
                branch: None,
                url: None,
            },
        )
        .await
    }

    pub async fn delete_remote_branch(
        &self,
        repository_id: &str,
        remote: &str,
        branch: &str,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!(
                "/repositories/{repository_id}/git/remotes/branch/delete"
            )),
            &RemoteRequest {
                remote: Some(remote.to_string()),
                branch: Some(branch.to_string()),
                url: None,
            },
        )
        .await
    }

    pub async fn set_upstream(
        &self,
        repository_id: &str,
        remote: &str,
        branch: &str,
    ) -> Result<String, String> {
        post_text(
            &self.url(&format!(
                "/repositories/{repository_id}/git/branches/upstream"
            )),
            &RemoteRequest {
                remote: Some(remote.to_string()),
                branch: Some(branch.to_string()),
                url: None,
            },
        )
        .await
    }

    pub async fn push_force_with_lease(
        &self,
        repository_id: &str,
        remote: &str,
        branch: &str,
    ) -> Result<String, String> {
        post_text(
            &self.url(&format!(
                "/repositories/{repository_id}/git/push/force-with-lease"
            )),
            &RemoteRequest {
                remote: Some(remote.to_string()),
                branch: Some(branch.to_string()),
                url: None,
            },
        )
        .await
    }

    pub async fn fetch(&self, repository_id: &str) -> Result<String, String> {
        self.fetch_remote(repository_id, "origin").await
    }

    pub async fn fetch_remote(&self, repository_id: &str, remote: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/fetch")),
            &RemoteRequest {
                remote: Some(remote.to_string()),
                branch: None,
                url: None,
            },
        )
        .await
    }

    pub async fn pull(&self, repository_id: &str) -> Result<String, String> {
        self.pull_remote(repository_id, "origin", None).await
    }

    pub async fn pull_remote(
        &self,
        repository_id: &str,
        remote: &str,
        branch: Option<&str>,
    ) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/pull")),
            &RemoteRequest {
                remote: Some(remote.to_string()),
                branch: branch.map(ToOwned::to_owned),
                url: None,
            },
        )
        .await
    }

    pub async fn push(&self, repository_id: &str) -> Result<String, String> {
        self.push_remote(repository_id, "origin", None).await
    }

    pub async fn push_remote(
        &self,
        repository_id: &str,
        remote: &str,
        branch: Option<&str>,
    ) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/push")),
            &RemoteRequest {
                remote: Some(remote.to_string()),
                branch: branch.map(ToOwned::to_owned),
                url: None,
            },
        )
        .await
    }

    pub async fn blame(&self, repository_id: &str, path: &str) -> Result<Vec<BlameLine>, String> {
        get_json(&self.url(&format!(
            "/repositories/{repository_id}/git/blame?path={}",
            urlencoding::encode(path)
        )))
        .await
    }

    pub async fn file_history(
        &self,
        repository_id: &str,
        path: &str,
    ) -> Result<Vec<CommitSummary>, String> {
        get_json(&self.url(&format!(
            "/repositories/{repository_id}/git/history/file?path={}&limit=100",
            urlencoding::encode(path)
        )))
        .await
    }

    pub async fn tree_at_revision(
        &self,
        repository_id: &str,
        revision: &str,
    ) -> Result<Vec<TreeEntrySummary>, String> {
        get_json(&self.url(&format!(
            "/repositories/{repository_id}/git/tree?revision={}",
            urlencoding::encode(revision)
        )))
        .await
    }

    pub async fn reflog(&self, repository_id: &str) -> Result<Vec<ReflogEntrySummary>, String> {
        get_json(&self.url(&format!(
            "/repositories/{repository_id}/git/reflog?limit=100"
        )))
        .await
    }

    pub async fn reset_to_revision(
        &self,
        repository_id: &str,
        revision: &str,
        hard: bool,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/reset")),
            &RevisionRequest {
                revision: revision.to_string(),
                hard: Some(hard),
            },
        )
        .await
    }

    pub async fn submodules(&self, repository_id: &str) -> Result<Vec<SubmoduleSummary>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/submodules"))).await
    }

    pub async fn submodule_init(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!(
                "/repositories/{repository_id}/git/submodules/init"
            )),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn submodule_update(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!(
                "/repositories/{repository_id}/git/submodules/update"
            )),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn submodule_sync(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!(
                "/repositories/{repository_id}/git/submodules/sync"
            )),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn lfs_summary(&self, repository_id: &str) -> Result<LfsSummary, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/lfs"))).await
    }

    pub async fn lfs_install(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/lfs/install")),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn lfs_track(&self, repository_id: &str, pattern: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/lfs/track")),
            &LfsRequest {
                pattern: Some(pattern.to_string()),
                remote: None,
                branch: None,
            },
        )
        .await
    }

    pub async fn lfs_untrack(&self, repository_id: &str, pattern: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/lfs/untrack")),
            &LfsRequest {
                pattern: Some(pattern.to_string()),
                remote: None,
                branch: None,
            },
        )
        .await
    }

    pub async fn lfs_pull(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/lfs/pull")),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn lfs_push(
        &self,
        repository_id: &str,
        remote: &str,
        branch: &str,
    ) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/lfs/push")),
            &LfsRequest {
                pattern: None,
                remote: Some(remote.to_string()),
                branch: Some(branch.to_string()),
            },
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

    pub async fn rebase_plan(
        &self,
        repository_id: &str,
        limit: usize,
    ) -> Result<Vec<CommitSummary>, String> {
        get_json(&self.url(&format!(
            "/repositories/{repository_id}/git/rebase/plan?limit={limit}"
        )))
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

    pub async fn rebase_continue(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!(
                "/repositories/{repository_id}/git/rebase/continue"
            )),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn rebase_abort(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/rebase/abort")),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn rebase_skip(&self, repository_id: &str) -> Result<String, String> {
        post_text(
            &self.url(&format!("/repositories/{repository_id}/git/rebase/skip")),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn cherry_pick(
        &self,
        repository_id: &str,
        commits: Vec<String>,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/cherry-pick")),
            &CherryPickRequest { commits },
        )
        .await
    }

    pub async fn cherry_pick_abort(&self, repository_id: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!(
                "/repositories/{repository_id}/git/cherry-pick/abort"
            )),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn conflicts(&self, repository_id: &str) -> Result<Vec<ConflictSummary>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/conflicts"))).await
    }

    pub async fn conflict_detail(
        &self,
        repository_id: &str,
        path: &str,
    ) -> Result<ConflictDetail, String> {
        get_json(&self.url(&format!(
            "/repositories/{repository_id}/git/conflicts/detail?path={}",
            urlencoding::encode(path)
        )))
        .await
    }

    pub async fn resolve_conflict(
        &self,
        repository_id: &str,
        path: &str,
        side: &str,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!(
                "/repositories/{repository_id}/git/conflicts/resolve"
            )),
            &ConflictResolveRequest {
                path: path.to_string(),
                side: side.to_string(),
            },
        )
        .await
    }

    pub async fn stashes(&self, repository_id: &str) -> Result<Vec<StashSummary>, String> {
        get_json(&self.url(&format!("/repositories/{repository_id}/git/stashes"))).await
    }

    pub async fn create_stash(&self, repository_id: &str, message: &str) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/stashes")),
            &StashRequest {
                message: Some(message.to_string()),
                author_name: Some("Zync".to_string()),
                author_email: Some("zync@local".to_string()),
                index: None,
                pop: None,
            },
        )
        .await
    }

    pub async fn apply_stash(
        &self,
        repository_id: &str,
        index: usize,
        pop: bool,
    ) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/stashes/apply")),
            &StashRequest {
                message: None,
                author_name: None,
                author_email: None,
                index: Some(index),
                pop: Some(pop),
            },
        )
        .await
    }

    pub async fn drop_stash(&self, repository_id: &str, index: usize) -> Result<(), String> {
        post_empty(
            &self.url(&format!("/repositories/{repository_id}/git/stashes/drop")),
            &StashRequest {
                message: None,
                author_name: None,
                author_email: None,
                index: Some(index),
                pop: None,
            },
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
async fn post_text<T>(url: &str, body: &T) -> Result<String, String>
where
    T: Serialize,
{
    let response = gloo_net::http::Request::post(url)
        .json(body)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(if text.is_empty() {
            format!("request failed with status {status}")
        } else {
            text
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn post_text<T>(_url: &str, _body: &T) -> Result<String, String>
where
    T: Serialize,
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

#[cfg(target_arch = "wasm32")]
async fn delete_empty(url: &str) -> Result<(), String> {
    let response = gloo_net::http::Request::delete(url)
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
async fn delete_empty(_url: &str) -> Result<(), String> {
    Err("ZyncApi network calls are available in wasm32 browser builds".to_string())
}
