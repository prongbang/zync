// Ported from crates/ui/src/api.rs.
//
// Base URL defaults to "" (same-origin): in dev the Vite proxy forwards
// /repositories, /workspace, /ws, etc. to the Rust API server; in prod the
// Axum server serves this app's static build itself, so relative paths are
// already correct.

import type {
  BlameLine,
  BranchRequest,
  BranchSummary,
  CherryPickRequest,
  CommitIdRequest,
  CommitRequest,
  CommitSummary,
  ConflictDetail,
  ConflictResolveRequest,
  ConflictSummary,
  CreateCredentialRequest,
  CreateFileRequest,
  CreateRepositoryRequest,
  CredentialRecord,
  DirectoryList,
  FavoriteRepositoryRequest,
  FileContent,
  FileNode,
  FilesRequest,
  FileStatus,
  InteractiveRebaseRequest,
  LfsRequest,
  LfsSummary,
  MergeStrategy,
  PatchRequest,
  PullMode,
  PushTagRequest,
  ReflogEntrySummary,
  RemoteRequest,
  RemoteSummary,
  RenameFileRequest,
  RepoStats,
  RepositoryRecord,
  RepositoryWithWorkspace,
  RevisionRequest,
  StashRequest,
  StashSummary,
  SubmoduleSummary,
  TagRequest,
  TagSummary,
  TreeEntrySummary,
  WorkspaceResponse,
  WriteFileRequest,
} from "./types"

/** Reads the response body as text and throws it verbatim on non-2xx. */
async function readOkOrThrow(response: Response): Promise<string> {
  const text = await response.text()
  if (!response.ok) {
    throw new Error(
      text.trim().length > 0
        ? text
        : `request failed with status ${response.status}`
    )
  }
  return text
}

async function getJson<T>(url: string): Promise<T> {
  const response = await fetch(url)
  const text = await readOkOrThrow(response)
  return JSON.parse(text) as T
}

async function getText(url: string): Promise<string> {
  const response = await fetch(url)
  return readOkOrThrow(response)
}

async function postJson<T>(url: string, body: unknown): Promise<T> {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  })
  const text = await readOkOrThrow(response)
  return JSON.parse(text) as T
}

async function postText(url: string, body: unknown): Promise<string> {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  })
  return readOkOrThrow(response)
}

async function postEmpty(url: string, body: unknown): Promise<void> {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  })
  await readOkOrThrow(response)
}

async function putEmpty(url: string, body: unknown): Promise<void> {
  const response = await fetch(url, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  })
  await readOkOrThrow(response)
}

async function del(url: string): Promise<void> {
  const response = await fetch(url, { method: "DELETE" })
  await readOkOrThrow(response)
}

/**
 * Reserved `revision` sentinel for the raw-blob route: read the uncommitted
 * working-tree copy of a file (the "After" side of an added/modified image
 * diff) instead of a committed revision. Mirrors the server's `WORKDIR_REVISION`.
 */
export const WORKDIR_REVISION = ":workdir"

export class ZyncApi {
  readonly baseUrl: string

  constructor(baseUrl = "") {
    this.baseUrl = baseUrl
  }

  private url(path: string): string {
    return `${this.baseUrl.replace(/\/+$/, "")}${path}`
  }

  // ---- Repositories ----

  async repositories(): Promise<RepositoryRecord[]> {
    return getJson(this.url("/repositories"))
  }

  async createRepository(
    request: CreateRepositoryRequest
  ): Promise<RepositoryWithWorkspace> {
    return postJson(this.url("/repositories"), request)
  }

  async deleteRepository(id: string): Promise<void> {
    return del(this.url(`/repositories/${id}`))
  }

  async setRepositoryFavorite(id: string, favorite: boolean): Promise<void> {
    const request: FavoriteRepositoryRequest = { favorite }
    return putEmpty(this.url(`/repositories/${id}/favorite`), request)
  }

  async directories(path?: string | null): Promise<DirectoryList> {
    const trimmed = (path ?? "").trim()
    if (trimmed.length === 0) {
      return getJson(this.url("/directories"))
    }
    return getJson(
      this.url(`/directories?path=${encodeURIComponent(trimmed)}`)
    )
  }

  async openRepository(id: string): Promise<RepositoryWithWorkspace> {
    return postJson(this.url(`/repositories/${id}/open`), {})
  }

  async workspace(id: string): Promise<WorkspaceResponse> {
    return getJson(this.url(`/workspace/${id}`))
  }

  // ---- Credentials ----
  // List/read is always the masked projection — secret material never
  // round-trips back to the client. There is no update; delete + recreate.

  async listCredentials(): Promise<CredentialRecord[]> {
    return getJson(this.url("/credentials"))
  }

  async createCredential(
    request: CreateCredentialRequest
  ): Promise<CredentialRecord> {
    return postJson(this.url("/credentials"), request)
  }

  async deleteCredential(id: string): Promise<void> {
    return del(this.url(`/credentials/${id}`))
  }

  // ---- Status / branches / graph / stats ----

  async status(repositoryId: string): Promise<FileStatus[]> {
    return getJson(this.url(`/repositories/${repositoryId}/git/status`))
  }

  async branches(repositoryId: string): Promise<BranchSummary[]> {
    return getJson(this.url(`/repositories/${repositoryId}/git/branches`))
  }

  async graph(repositoryId: string): Promise<CommitSummary[]> {
    return this.graphWithLimit(repositoryId, 500)
  }

  async repoStats(repositoryId: string): Promise<RepoStats> {
    return getJson(this.url(`/repositories/${repositoryId}/git/stats`))
  }

  async graphWithLimit(
    repositoryId: string,
    limit: number
  ): Promise<CommitSummary[]> {
    return getJson(
      this.url(`/repositories/${repositoryId}/git/graph?limit=${limit}`)
    )
  }

  /** Full-history commit search — unlike `graph`/`graphWithLimit`, walks the whole
   * history rather than a windowed page. `path` restricts to commits that touched
   * that file (a simple "touched this path" diff-tree check, not `--follow`). */
  async searchCommits(
    repositoryId: string,
    query: string,
    limit = 200,
    path?: string
  ): Promise<CommitSummary[]> {
    const params = new URLSearchParams({ q: query, limit: String(limit) })
    if (path) params.set("path", path)
    return getJson(
      this.url(`/repositories/${repositoryId}/git/search?${params.toString()}`)
    )
  }

  // ---- Diffs ----

  async diffWorkdir(repositoryId: string): Promise<string> {
    return getText(this.url(`/repositories/${repositoryId}/git/diff/workdir`))
  }

  async diffWorkdirFile(repositoryId: string, path: string): Promise<string> {
    return getText(
      this.url(
        `/repositories/${repositoryId}/git/diff/workdir?path=${encodeURIComponent(path)}`
      )
    )
  }

  async diffStagedFile(repositoryId: string, path: string): Promise<string> {
    return getText(
      this.url(
        `/repositories/${repositoryId}/git/diff/staged?path=${encodeURIComponent(path)}`
      )
    )
  }

  async diffCommitToWorkdir(
    repositoryId: string,
    commitId: string
  ): Promise<string> {
    return getText(
      this.url(`/repositories/${repositoryId}/git/diff/compare/${commitId}`)
    )
  }

  async diffCommit(repositoryId: string, commitId: string): Promise<string> {
    return getText(
      this.url(`/repositories/${repositoryId}/git/diff/commit/${commitId}`)
    )
  }

  // ---- Files / workspace tree ----

  async readFile(workspaceId: string, path: string): Promise<FileContent> {
    return getJson(
      this.url(`/workspace/${workspaceId}/files/${encodeURIComponent(path)}`)
    )
  }

  assetUrl(workspaceId: string, path: string): string {
    return this.url(
      `/workspace/${workspaceId}/assets/${encodeURIComponent(path)}`
    )
  }

  blobUrl(repositoryId: string, revision: string, path: string): string {
    return this.url(
      `/repositories/${repositoryId}/git/blob?revision=${encodeURIComponent(
        revision
      )}&path=${encodeURIComponent(path)}`
    )
  }

  async writeFile(
    workspaceId: string,
    path: string,
    content: string
  ): Promise<void> {
    const request: WriteFileRequest = { content }
    return putEmpty(
      this.url(`/workspace/${workspaceId}/files/${encodeURIComponent(path)}`),
      request
    )
  }

  async createFile(
    workspaceId: string,
    path: string,
    isDir: boolean
  ): Promise<void> {
    const request: CreateFileRequest = {
      path,
      content: null,
      is_dir: isDir,
    }
    return postEmpty(this.url(`/workspace/${workspaceId}/files`), request)
  }

  async renameFile(
    workspaceId: string,
    oldPath: string,
    newPath: string
  ): Promise<void> {
    const request: RenameFileRequest = {
      old_path: oldPath,
      new_path: newPath,
    }
    return putEmpty(
      this.url(`/workspace/${workspaceId}/files/rename`),
      request
    )
  }

  async deleteFile(workspaceId: string, path: string): Promise<void> {
    return del(
      this.url(`/workspace/${workspaceId}/files/${encodeURIComponent(path)}`)
    )
  }

  async searchFiles(workspaceId: string, query: string): Promise<FileNode[]> {
    return getJson(
      this.url(
        `/workspace/${workspaceId}/files/search?q=${encodeURIComponent(query)}`
      )
    )
  }

  // ---- Staging ----

  async stageFiles(repositoryId: string, files: string[]): Promise<void> {
    const request: FilesRequest = { files }
    return postEmpty(this.url(`/repositories/${repositoryId}/git/add`), request)
  }

  async unstageFiles(repositoryId: string, files: string[]): Promise<void> {
    const request: FilesRequest = { files }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/unstage`),
      request
    )
  }

  async discardFiles(repositoryId: string, files: string[]): Promise<void> {
    const request: FilesRequest = { files }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/discard`),
      request
    )
  }

  async stagePatch(repositoryId: string, patch: string): Promise<void> {
    const request: PatchRequest = { patch }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/stage-patch`),
      request
    )
  }

  // ---- Branches ----

  async checkoutBranch(repositoryId: string, name: string): Promise<void> {
    const request: BranchRequest = {
      name,
      new_name: null,
      checkout: null,
      revision: null,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/checkout`),
      request
    )
  }

  async mergeBranch(
    repositoryId: string,
    name: string,
    strategy?: MergeStrategy | null
  ): Promise<void> {
    const request: BranchRequest = {
      name,
      new_name: null,
      checkout: null,
      revision: null,
      strategy: strategy ?? null,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/branches/merge`),
      request
    )
  }

  async deleteBranch(repositoryId: string, name: string): Promise<void> {
    const request: BranchRequest = {
      name,
      new_name: null,
      checkout: null,
      revision: null,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/branches/delete`),
      request
    )
  }

  async createBranch(
    repositoryId: string,
    name: string,
    checkout: boolean
  ): Promise<void> {
    const request: BranchRequest = {
      name,
      new_name: null,
      checkout,
      revision: null,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/branches`),
      request
    )
  }

  async createBranchAt(
    repositoryId: string,
    name: string,
    revision: string,
    checkout: boolean
  ): Promise<void> {
    const request: BranchRequest = {
      name,
      new_name: null,
      checkout,
      revision,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/branches`),
      request
    )
  }

  async renameBranch(
    repositoryId: string,
    oldName: string,
    newName: string
  ): Promise<void> {
    const request: BranchRequest = {
      name: oldName,
      new_name: newName,
      checkout: null,
      revision: null,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/branches/rename`),
      request
    )
  }

  async checkoutRevision(
    repositoryId: string,
    revision: string
  ): Promise<void> {
    const request: RevisionRequest = { revision, hard: null }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/checkout/revision`),
      request
    )
  }

  async revertCommit(
    repositoryId: string,
    commit: string,
    mainline?: number | null
  ): Promise<void> {
    const request: CommitIdRequest = { commit, mainline: mainline ?? null }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/revert`),
      request
    )
  }

  // ---- Tags ----

  async tags(repositoryId: string): Promise<TagSummary[]> {
    return getJson(this.url(`/repositories/${repositoryId}/git/tags`))
  }

  async createTag(
    repositoryId: string,
    name: string,
    target?: string | null
  ): Promise<void> {
    const request: TagRequest = { name, target: target ?? null }
    return postEmpty(this.url(`/repositories/${repositoryId}/git/tags`), request)
  }

  async deleteTag(repositoryId: string, name: string): Promise<void> {
    const request: TagRequest = { name, target: null }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/tags/delete`),
      request
    )
  }

  async pushTag(
    repositoryId: string,
    name: string,
    remote?: string | null
  ): Promise<string> {
    const request: PushTagRequest = { name, remote: remote ?? null }
    return postText(
      this.url(`/repositories/${repositoryId}/git/tags/push`),
      request
    )
  }

  // ---- Remotes ----

  async remotes(repositoryId: string): Promise<RemoteSummary[]> {
    return getJson(this.url(`/repositories/${repositoryId}/git/remotes`))
  }

  async addRemote(
    repositoryId: string,
    name: string,
    url: string
  ): Promise<void> {
    const request: RemoteRequest = { remote: name, branch: null, url }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/remotes`),
      request
    )
  }

  async deleteRemote(repositoryId: string, name: string): Promise<void> {
    const request: RemoteRequest = { remote: name, branch: null, url: null }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/remotes/delete`),
      request
    )
  }

  async pruneRemote(repositoryId: string, name: string): Promise<string> {
    const request: RemoteRequest = { remote: name, branch: null, url: null }
    return postText(
      this.url(`/repositories/${repositoryId}/git/remotes/prune`),
      request
    )
  }

  async deleteRemoteBranch(
    repositoryId: string,
    remote: string,
    branch: string
  ): Promise<void> {
    const request: RemoteRequest = { remote, branch, url: null }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/remotes/branch/delete`),
      request
    )
  }

  async setUpstream(
    repositoryId: string,
    remote: string,
    branch: string
  ): Promise<string> {
    const request: RemoteRequest = { remote, branch, url: null }
    return postText(
      this.url(`/repositories/${repositoryId}/git/branches/upstream`),
      request
    )
  }

  async pushForceWithLease(
    repositoryId: string,
    remote: string,
    branch: string
  ): Promise<string> {
    const request: RemoteRequest = { remote, branch, url: null }
    return postText(
      this.url(`/repositories/${repositoryId}/git/push/force-with-lease`),
      request
    )
  }

  // ---- Fetch / pull / push ----

  async fetch(repositoryId: string): Promise<string> {
    return this.fetchRemote(repositoryId, "origin")
  }

  async fetchRemote(repositoryId: string, remote: string): Promise<string> {
    const request: RemoteRequest = { remote, branch: null, url: null }
    return postText(
      this.url(`/repositories/${repositoryId}/git/fetch`),
      request
    )
  }

  /** Fetches every configured remote in turn; stops at the first failure. */
  async fetchAll(repositoryId: string): Promise<string> {
    return postText(
      this.url(`/repositories/${repositoryId}/git/fetch-all`),
      undefined
    )
  }

  async pull(repositoryId: string): Promise<string> {
    return this.pullRemote(repositoryId, "origin", null)
  }

  async pullRemote(
    repositoryId: string,
    remote: string,
    branch?: string | null,
    mode?: PullMode | null
  ): Promise<string> {
    const request: RemoteRequest = {
      remote,
      branch: branch ?? null,
      url: null,
      mode: mode ?? null,
    }
    return postText(
      this.url(`/repositories/${repositoryId}/git/pull`),
      request
    )
  }

  async push(repositoryId: string): Promise<string> {
    return this.pushRemote(repositoryId, "origin", null)
  }

  async pushRemote(
    repositoryId: string,
    remote: string,
    branch?: string | null,
    options?: { forceWithLease?: boolean; setUpstream?: boolean }
  ): Promise<string> {
    const request: RemoteRequest = {
      remote,
      branch: branch ?? null,
      url: null,
      force_with_lease: options?.forceWithLease ?? null,
      set_upstream: options?.setUpstream ?? null,
    }
    return postText(
      this.url(`/repositories/${repositoryId}/git/push`),
      request
    )
  }

  // ---- Blame / history / tree / reflog ----

  async blame(repositoryId: string, path: string): Promise<BlameLine[]> {
    return getJson(
      this.url(
        `/repositories/${repositoryId}/git/blame?path=${encodeURIComponent(path)}`
      )
    )
  }

  async fileHistory(
    repositoryId: string,
    path: string
  ): Promise<CommitSummary[]> {
    return getJson(
      this.url(
        `/repositories/${repositoryId}/git/history/file?path=${encodeURIComponent(
          path
        )}&limit=100`
      )
    )
  }

  async treeAtRevision(
    repositoryId: string,
    revision: string
  ): Promise<TreeEntrySummary[]> {
    return getJson(
      this.url(
        `/repositories/${repositoryId}/git/tree?revision=${encodeURIComponent(revision)}`
      )
    )
  }

  async reflog(repositoryId: string): Promise<ReflogEntrySummary[]> {
    return getJson(
      this.url(`/repositories/${repositoryId}/git/reflog?limit=100`)
    )
  }

  async resetToRevision(
    repositoryId: string,
    revision: string,
    hard: boolean
  ): Promise<void> {
    const request: RevisionRequest = { revision, hard }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/reset`),
      request
    )
  }

  // ---- Submodules ----

  async submodules(repositoryId: string): Promise<SubmoduleSummary[]> {
    return getJson(this.url(`/repositories/${repositoryId}/git/submodules`))
  }

  async submoduleInit(repositoryId: string): Promise<string> {
    return postText(
      this.url(`/repositories/${repositoryId}/git/submodules/init`),
      {}
    )
  }

  async submoduleUpdate(repositoryId: string): Promise<string> {
    return postText(
      this.url(`/repositories/${repositoryId}/git/submodules/update`),
      {}
    )
  }

  async submoduleSync(repositoryId: string): Promise<string> {
    return postText(
      this.url(`/repositories/${repositoryId}/git/submodules/sync`),
      {}
    )
  }

  // ---- LFS ----

  async lfsSummary(repositoryId: string): Promise<LfsSummary> {
    return getJson(this.url(`/repositories/${repositoryId}/git/lfs`))
  }

  async lfsInstall(repositoryId: string): Promise<string> {
    return postText(this.url(`/repositories/${repositoryId}/git/lfs/install`), {})
  }

  async lfsTrack(repositoryId: string, pattern: string): Promise<string> {
    const request: LfsRequest = { pattern, remote: null, branch: null }
    return postText(
      this.url(`/repositories/${repositoryId}/git/lfs/track`),
      request
    )
  }

  async lfsUntrack(repositoryId: string, pattern: string): Promise<string> {
    const request: LfsRequest = { pattern, remote: null, branch: null }
    return postText(
      this.url(`/repositories/${repositoryId}/git/lfs/untrack`),
      request
    )
  }

  async lfsPull(repositoryId: string): Promise<string> {
    return postText(this.url(`/repositories/${repositoryId}/git/lfs/pull`), {})
  }

  async lfsPush(
    repositoryId: string,
    remote: string,
    branch: string
  ): Promise<string> {
    const request: LfsRequest = { pattern: null, remote, branch }
    return postText(
      this.url(`/repositories/${repositoryId}/git/lfs/push`),
      request
    )
  }

  // ---- Commit / rebase ----

  async commit(
    repositoryId: string,
    request: CommitRequest
  ): Promise<unknown> {
    return postJson(
      this.url(`/repositories/${repositoryId}/git/commit`),
      request
    )
  }

  async rebasePlan(
    repositoryId: string,
    limit: number
  ): Promise<CommitSummary[]> {
    return getJson(
      this.url(`/repositories/${repositoryId}/git/rebase/plan?limit=${limit}`)
    )
  }

  websocketUrl(workspaceId: string): string {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:"
    const base = this.baseUrl.replace(/\/+$/, "")
    return `${protocol}//${window.location.host}${base}/ws/workspace/${workspaceId}`
  }

  async interactiveRebase(
    repositoryId: string,
    request: InteractiveRebaseRequest
  ): Promise<unknown> {
    return postJson(
      this.url(`/repositories/${repositoryId}/git/rebase/interactive`),
      request
    )
  }

  async rebaseContinue(repositoryId: string): Promise<string> {
    return postText(
      this.url(`/repositories/${repositoryId}/git/rebase/continue`),
      {}
    )
  }

  async rebaseAbort(repositoryId: string): Promise<string> {
    return postText(
      this.url(`/repositories/${repositoryId}/git/rebase/abort`),
      {}
    )
  }

  async rebaseSkip(repositoryId: string): Promise<string> {
    return postText(
      this.url(`/repositories/${repositoryId}/git/rebase/skip`),
      {}
    )
  }

  // ---- Cherry-pick / conflicts ----

  async cherryPick(repositoryId: string, commits: string[]): Promise<void> {
    const request: CherryPickRequest = { commits }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/cherry-pick`),
      request
    )
  }

  async cherryPickAbort(repositoryId: string): Promise<void> {
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/cherry-pick/abort`),
      {}
    )
  }

  async conflicts(repositoryId: string): Promise<ConflictSummary[]> {
    return getJson(this.url(`/repositories/${repositoryId}/git/conflicts`))
  }

  async conflictDetail(
    repositoryId: string,
    path: string
  ): Promise<ConflictDetail> {
    return getJson(
      this.url(
        `/repositories/${repositoryId}/git/conflicts/detail?path=${encodeURIComponent(path)}`
      )
    )
  }

  async resolveConflict(
    repositoryId: string,
    path: string,
    side: string
  ): Promise<void> {
    const request: ConflictResolveRequest = { path, side }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/conflicts/resolve`),
      request
    )
  }

  // ---- Stashes ----

  async stashes(repositoryId: string): Promise<StashSummary[]> {
    return getJson(this.url(`/repositories/${repositoryId}/git/stashes`))
  }

  async createStash(repositoryId: string, message: string): Promise<void> {
    const request: StashRequest = {
      message,
      author_name: "Zync",
      author_email: "zync@local",
      index: null,
      pop: null,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/stashes`),
      request
    )
  }

  async applyStash(
    repositoryId: string,
    index: number,
    pop: boolean
  ): Promise<void> {
    const request: StashRequest = {
      message: null,
      author_name: null,
      author_email: null,
      index,
      pop,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/stashes/apply`),
      request
    )
  }

  async dropStash(repositoryId: string, index: number): Promise<void> {
    const request: StashRequest = {
      message: null,
      author_name: null,
      author_email: null,
      index,
      pop: null,
    }
    return postEmpty(
      this.url(`/repositories/${repositoryId}/git/stashes/drop`),
      request
    )
  }
}

export const zyncApi = new ZyncApi()
