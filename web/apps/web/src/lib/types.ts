// Ported from crates/ui/src/api.rs (serde structs).
//
// Field names are kept snake_case to match the server's JSON wire format
// exactly (do NOT camelCase these).
//
// Naming convention used below for optionality:
//   - `T | null`            -> Rust `Option<T>` with no `#[serde(default)]`.
//     The server always sends the key (possibly `null`).
//   - `field?: T | null`    -> Rust `Option<T>` with `#[serde(default)]`.
//     The key may be entirely absent from the JSON, or present as `null`.
//   - bare `T` with a note  -> Rust non-Option field with `#[serde(default)]`
//     (defaults to the type's zero value, e.g. `""` for String, `[]` for Vec,
//     when the server omits the key).

export interface RepositoryRecord {
  id: string
  name: string
  path: string
  remote_url: string | null
  favorite: boolean
  created_at: string
}

export interface WorkspaceRecord {
  id: string
  repository_id: string
  name: string
  created_at: string
}

export interface RepositoryWithWorkspace {
  repository: RepositoryRecord
  workspace: WorkspaceRecord
}

export interface DirectoryEntry {
  name: string
  path: string
}

export interface DirectoryList {
  current_path: string
  parent_path: string | null
  directories: DirectoryEntry[]
}

export interface FileNode {
  path: string
  name: string
  is_dir: boolean
  size: number
}

export interface FileContent {
  path: string
  content: string
}

export interface FileStatus {
  path: string
  staged: boolean
  unstaged: boolean
  untracked: boolean
  ignored: boolean
  conflicted: boolean
}

export interface CommitRef {
  name: string
  kind: string
}

export interface CommitSummary {
  id: string
  summary: string
  author: string
  /** `#[serde(default)]` -> defaults to "" when absent. */
  author_email: string
  /** `#[serde(default)]` -> defaults to "" when absent. */
  committer: string
  /** `#[serde(default)]` -> defaults to "" when absent. */
  committer_email: string
  time: number
  parents: string[]
  /** `#[serde(default)]` -> defaults to [] when absent. */
  refs: CommitRef[]
}

export interface BranchSummary {
  name: string
  is_head: boolean
  kind: string
  target: string | null
  /** `Option<usize>` + `#[serde(default)]` -> key may be absent, or null. */
  ahead?: number | null
  /** `Option<usize>` + `#[serde(default)]` -> key may be absent, or null. */
  behind?: number | null
}

export interface AuthorStat {
  name: string
  commits: number
}

export interface MonthStat {
  year: number
  month: number
  total: number
  top: AuthorStat[]
}

export interface RepoStats {
  commit_count: number
  contributors: AuthorStat[]
  monthly: MonthStat[]
  first_commit_time: number
  last_commit_time: number
}

export interface TagSummary {
  name: string
  target: string | null
}

export interface RemoteSummary {
  name: string
  url: string | null
  push_url: string | null
}

export interface BlameLine {
  start_line: number
  line_count: number
  commit: string
  author: string
  summary: string
}

export interface TreeEntrySummary {
  path: string
  kind: string
  id: string
  size: number | null
}

export interface ReflogEntrySummary {
  index: number
  old_id: string
  new_id: string
  message: string
  committer: string
  time: number
}

export interface SubmoduleSummary {
  name: string
  path: string
  url: string | null
  head: string | null
}

export interface LfsSummary {
  configured: boolean
  tracked_patterns: string[]
}

export interface StashSummary {
  index: number
  name: string
  message: string
}

export interface ConflictSummary {
  ancestor: string | null
  ours: string | null
  theirs: string | null
}

export interface ConflictDetail {
  path: string
  ancestor_path: string | null
  ours_path: string | null
  theirs_path: string | null
  ancestor_content: string
  ours_content: string
  theirs_content: string
}

export interface PresenceUser {
  user_id: string
  name: string
  current_file: string | null
  cursor_line: number | null
  cursor_column: number | null
}

export interface WorkspaceResponse {
  workspace: WorkspaceRecord
  repository: RepositoryRecord
  files: FileNode[]
  online_users: PresenceUser[]
}

// Masked credential projection — never carries secret material (see
// crates/server/src/db/mod.rs CredentialSummary / credentials::mod.rs
// CredentialResponse). There is no "reveal" endpoint; update = delete + recreate.
export interface CredentialRecord {
  id: string
  label: string
  host_pattern: string
  kind: string
  username: string | null
  created_at: string
}

// ---- Request bodies (mirrors the `Serialize`-only structs in api.rs) ----

export interface CreateRepositoryRequest {
  name: string | null
  path: string | null
  remote_url: string | null
  clone_to: string | null
  /** `#[serde(default)]` -> key may be absent, defaults to `false` server-side. When `true`,
   * `path` is initialized as a brand-new repository instead of opened as an existing one. */
  init?: boolean
}

export interface FavoriteRepositoryRequest {
  favorite: boolean
}

// `kind` is 'https_token' | 'ssh_key'. For 'https_token' send `token`; for
// 'ssh_key' send `private_key` (and optionally `passphrase`/`public_key`).
export interface CreateCredentialRequest {
  label: string
  host_pattern: string
  kind: string
  username: string | null
  token: string | null
  private_key: string | null
  passphrase: string | null
  public_key: string | null
}

export interface CommitRequest {
  message: string
  author_name: string
  author_email: string
  amend: boolean
  sign_off: boolean
}

export interface FilesRequest {
  files: string[]
}

export interface PatchRequest {
  patch: string
}

export interface WriteFileRequest {
  content: string
}

export interface CreateFileRequest {
  path: string
  content: string | null
  is_dir: boolean | null
}

export interface RenameFileRequest {
  old_path: string
  new_path: string
}

// Merge strategy — mirrors `zync_git_core::MergeStrategy`. Omitted/undefined = "no-ff" (old
// default behavior, preserved).
export type MergeStrategy = "ff-only" | "no-ff" | "squash"

export interface BranchRequest {
  name: string
  new_name: string | null
  checkout: boolean | null
  revision: string | null
  /** Merge only. `#[serde(default)]` -> key may be absent, or null. */
  strategy?: MergeStrategy | null
}

// Pull strategy — mirrors `zync_git_core::PullMode`. Omitted/undefined = "ff-only" (old
// default behavior, preserved).
export type PullMode = "ff-only" | "merge" | "rebase"

export interface RemoteRequest {
  remote: string | null
  branch: string | null
  url: string | null
  // Pull only.
  mode?: PullMode | null
  // Push only: use force-with-lease semantics instead of a plain push.
  force_with_lease?: boolean | null
  // Push only, force-with-lease path: also set upstream tracking after a successful lease
  // push (a plain push always does this already; force-with-lease does not by default).
  set_upstream?: boolean | null
}

export interface LfsRequest {
  pattern: string | null
  remote: string | null
  branch: string | null
}

export interface RevisionRequest {
  revision: string
  hard: boolean | null
}

export interface TagRequest {
  name: string
  target: string | null
}

export interface CommitIdRequest {
  commit: string
  /** Revert only: 1-based mainline parent, required when `commit` is a merge commit.
   * `#[serde(default)]` -> key may be absent, or null. */
  mainline?: number | null
}

export interface CherryPickRequest {
  commits: string[]
}

export interface ConflictResolveRequest {
  path: string
  side: string
}

export interface RebaseStepRequest {
  commit: string
  action: string
  /** `#[serde(skip_serializing_if = "Option::is_none")]` -> key omitted, not null, when absent. */
  message?: string
}

export interface InteractiveRebaseRequest {
  base: string
  steps: RebaseStepRequest[]
}

export interface StashRequest {
  message: string | null
  author_name: string | null
  author_email: string | null
  index: number | null
  pop: boolean | null
}
