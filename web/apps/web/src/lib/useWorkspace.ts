// Workspace state + live-sync, ported from crates/ui/src/actions.rs
// (load_workspace_scoped, start_live_events) and app.rs signal wiring.

import { useCallback, useEffect, useRef, useState } from "react"

import { ZyncApi } from "./api"
import {
  buildBlameRows,
  quickRebasePlan,
  SCOPE_ALL,
  SCOPE_BRANCHES,
  SCOPE_CONFLICTS,
  SCOPE_DIFF,
  SCOPE_GRAPH,
  SCOPE_STASHES,
  SCOPE_STATUS,
  SCOPE_WORKDIR,
  SCOPE_WORKSPACE,
  scopeForEvent,
  shortId,
  type BlameRow,
} from "./helpers"
import type {
  BranchSummary,
  CommitSummary,
  ConflictSummary,
  FileStatus,
  PullMode,
  RepoStats,
  RepositoryRecord,
  StashSummary,
  WorkspaceResponse,
} from "./types"

const api = new ZyncApi()

export type WorkspaceState = {
  api: ZyncApi
  repositories: RepositoryRecord[]
  /** True once the initial `loadRepositories()` has settled (success or failure) — lets callers
   * distinguish "still loading" from a genuine zero-repositories empty state. */
  repositoriesLoaded: boolean
  /** Set when the most recent `loadRepositories()` call failed (server unreachable, etc.); `null`
   * once a load succeeds. Lets an empty-state screen tell "nothing registered yet" apart from
   * "couldn't even ask the server" instead of silently showing the former for both. */
  repositoriesError: string | null
  workspace: WorkspaceResponse | null
  gitStatus: FileStatus[]
  branches: BranchSummary[]
  commits: CommitSummary[]
  stashes: StashSummary[]
  conflicts: ConflictSummary[]
  diff: string
  notice: string
  liveSyncOk: boolean
  selectedFile: string
  loadRepositories: () => Promise<void>
  openRepository: (repositoryId: string) => Promise<void>
  // Repository registry actions (RepoMinibar context menu / Add Repository dialog).
  setRepositoryFavorite: (repositoryId: string, favorite: boolean) => Promise<void>
  /** Unregisters a repository. If it was the open one, switches to another registered
   * repository (or clears the open workspace so the zero-repositories empty state shows). */
  removeRepository: (repositoryId: string) => Promise<void>
  refresh: (scope?: number) => void
  loadMore: () => void
  setDiff: (patch: string) => void
  setNotice: (message: string) => void
  // Working-copy actions.
  selectFileDiff: (path: string) => Promise<void>
  stageFiles: (paths: string[]) => Promise<void>
  unstageFiles: (paths: string[]) => Promise<void>
  discardFiles: (paths: string[]) => Promise<void>
  stagePatch: (patch: string) => Promise<void>
  commit: (message: string, opts?: CommitOptions) => Promise<void>
  requestBlame: (path: string) => Promise<BlameRow[]>
  // Commit-menu / remote actions.
  runCommitAction: (action: CommitAction, commitId: string) => Promise<void>
  // Toolbar remote ops. Unlike `run`-based actions above, these resolve to
  // the server's success message and reject with the thrown error instead of
  // swallowing it, so the toolbar can drive a per-button busy state and toast
  // the outcome itself (still setting the footer notice as a side effect).
  fetchRemote: (all?: boolean) => Promise<string>
  pullRemote: (mode?: PullMode) => Promise<string>
  pushRemote: (opts?: {
    forceWithLease?: boolean
    setUpstream?: boolean
  }) => Promise<string>
  // Branch / tag / stash / conflict / rebase actions.
  createBranch: (
    name: string,
    opts: { startPoint?: string; checkout: boolean; localMode: LocalChangesMode },
    changedPaths: string[],
  ) => Promise<void>
  renameBranch: (name: string, newName: string) => Promise<void>
  deleteBranch: (name: string) => Promise<void>
  checkoutBranch: (name: string) => Promise<void>
  mergeBranch: (name: string) => Promise<void>
  createTag: (name: string, target?: string) => Promise<void>
  deleteTag: (name: string) => Promise<void>
  createStash: (message: string) => Promise<void>
  applyStash: (index: number, pop: boolean) => Promise<void>
  dropStash: (index: number) => Promise<void>
  resolveConflict: (path: string, side: "local" | "remote") => Promise<void>
  runInteractiveRebase: (
    commits: CommitSummary[],
    targetId: string,
    action: "reword" | "edit" | "squash" | "fixup" | "drop" | "pick",
    message: string | undefined,
    successNotice: string,
  ) => Promise<void>
  resetToCommit: (commitId: string, hard: boolean) => Promise<void>
  loadStats: () => Promise<void>
  repoStats: RepoStats | null
}

export type LocalChangesMode = "dont-change" | "stash-reapply" | "discard"

export type CommitOptions = {
  amend?: boolean
  signOff?: boolean
  pushAfter?: boolean
}

export type CommitAction =
  | "checkout"
  | "cherry-pick"
  | "revert"
  | "copy-sha"
  | "save-patch"
  | "compare-local"

export function useWorkspace(): WorkspaceState {
  const [repositories, setRepositories] = useState<RepositoryRecord[]>([])
  const [repositoriesLoaded, setRepositoriesLoaded] = useState(false)
  const [repositoriesError, setRepositoriesError] = useState<string | null>(null)
  const [workspace, setWorkspace] = useState<WorkspaceResponse | null>(null)
  const [gitStatus, setGitStatus] = useState<FileStatus[]>([])
  const [branches, setBranches] = useState<BranchSummary[]>([])
  const [commits, setCommits] = useState<CommitSummary[]>([])
  const [stashes, setStashes] = useState<StashSummary[]>([])
  const [conflicts, setConflicts] = useState<ConflictSummary[]>([])
  const [diff, setDiff] = useState("")
  const [notice, setNotice] = useState("Ready")
  const [liveSyncOk, setLiveSyncOk] = useState(false)
  const [selectedFile, setSelectedFile] = useState("")
  const [repoStats, setRepoStats] = useState<RepoStats | null>(null)

  // Identifiers + graph limit read inside async callbacks — keep in refs so the
  // callbacks stay stable and always see the latest values.
  const repoIdRef = useRef<string | null>(null)
  const workspaceIdRef = useRef<string | null>(null)
  const graphLimitRef = useRef(500)
  const commitsCountRef = useRef(0)
  commitsCountRef.current = commits.length

  const loadRepositories = useCallback(async () => {
    try {
      setRepositories(await api.repositories())
      setRepositoriesError(null)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      setNotice(message)
      setRepositoriesError(message)
    } finally {
      setRepositoriesLoaded(true)
    }
  }, [])

  // Concurrent scoped fetch (futures join! equivalent). Coalescing is handled by
  // React batching + the ref guards; a full port of the in-flight merge is not
  // needed because fetches are idempotent and the latest response wins.
  const refresh = useCallback((scope: number = SCOPE_ALL) => {
    const repositoryId = repoIdRef.current
    const workspaceId = workspaceIdRef.current
    if (!repositoryId || !workspaceId) return
    const limit = Math.max(graphLimitRef.current, commitsCountRef.current, 500)

    void (async () => {
      const jobs: Promise<void>[] = []
      const run = (bit: number, fn: () => Promise<void>) => {
        if (scope & bit) jobs.push(fn())
      }
      run(SCOPE_WORKSPACE, async () => setWorkspace(await api.workspace(workspaceId)))
      run(SCOPE_STATUS, async () => setGitStatus(await api.status(repositoryId)))
      run(SCOPE_BRANCHES, async () => setBranches(await api.branches(repositoryId)))
      run(SCOPE_GRAPH, async () =>
        setCommits(await api.graphWithLimit(repositoryId, limit)),
      )
      run(SCOPE_STASHES, async () => setStashes(await api.stashes(repositoryId)))
      run(SCOPE_CONFLICTS, async () => setConflicts(await api.conflicts(repositoryId)))
      run(SCOPE_DIFF, async () => setDiff(await api.diffWorkdir(repositoryId)))
      const results = await Promise.allSettled(jobs)
      const failure = results.find((r) => r.status === "rejected")
      if (failure && failure.status === "rejected") {
        const reason = failure.reason
        setNotice(reason instanceof Error ? reason.message : String(reason))
      }
    })()
  }, [])

  const openRepository = useCallback(
    async (repositoryId: string) => {
      try {
        const opened = await api.openRepository(repositoryId)
        repoIdRef.current = opened.repository.id
        workspaceIdRef.current = opened.workspace.id
        graphLimitRef.current = 500
        commitsCountRef.current = 0
        setCommits([])
        setDiff("")
        setNotice("Workspace opened and watcher attached")
        refresh(SCOPE_ALL)
      } catch (error) {
        setNotice(error instanceof Error ? error.message : String(error))
      }
    },
    [refresh],
  )

  const setRepositoryFavorite = useCallback(
    async (repositoryId: string, favorite: boolean) => {
      try {
        await api.setRepositoryFavorite(repositoryId, favorite)
        setRepositories(await api.repositories())
      } catch (error) {
        const err = error instanceof Error ? error : new Error(String(error))
        setNotice(err.message)
        throw err
      }
    },
    [],
  )

  const removeRepository = useCallback(
    async (repositoryId: string) => {
      try {
        await api.deleteRepository(repositoryId)
        const remaining = await api.repositories()
        setRepositories(remaining)
        if (repoIdRef.current === repositoryId) {
          repoIdRef.current = null
          workspaceIdRef.current = null
          setWorkspace(null)
          setCommits([])
          setBranches([])
          setGitStatus([])
          setStashes([])
          setConflicts([])
          setDiff("")
          setSelectedFile("")
          if (remaining.length > 0) {
            await openRepository(remaining[0].id)
          } else {
            setNotice("Ready")
          }
        }
      } catch (error) {
        const err = error instanceof Error ? error : new Error(String(error))
        setNotice(err.message)
        throw err
      }
    },
    [openRepository],
  )

  const loadMore = useCallback(() => {
    graphLimitRef.current += 500
    refresh(SCOPE_GRAPH)
  }, [refresh])

  const guard = useCallback(() => {
    const repositoryId = repoIdRef.current
    if (!repositoryId) {
      setNotice("Open a repository first")
      return null
    }
    return repositoryId
  }, [])

  const run = useCallback(
    async (fn: (repositoryId: string) => Promise<string>, scope: number) => {
      const repositoryId = guard()
      if (!repositoryId) return
      try {
        setNotice(await fn(repositoryId))
        refresh(scope)
      } catch (error) {
        setNotice(error instanceof Error ? error.message : String(error))
      }
    },
    [guard, refresh],
  )

  const selectFileDiff = useCallback(async (path: string) => {
    const repositoryId = guard()
    if (!repositoryId) return
    setSelectedFile(path)
    try {
      const workdir = await api.diffWorkdirFile(repositoryId, path)
      const staged = workdir.trim()
        ? ""
        : await api.diffStagedFile(repositoryId, path)
      const patch = workdir.trim()
        ? workdir
        : staged.trim()
          ? staged
          : `No diff for ${path}`
      setDiff(patch)
      setNotice(`Showing local diff for ${path}`)
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error))
    }
  }, [guard])

  const stageFiles = useCallback(
    (paths: string[]) =>
      run((id) => api.stageFiles(id, paths).then(() => "Staged"), SCOPE_WORKDIR),
    [run],
  )
  const unstageFiles = useCallback(
    (paths: string[]) =>
      run((id) => api.unstageFiles(id, paths).then(() => "Unstaged"), SCOPE_WORKDIR),
    [run],
  )
  const discardFiles = useCallback(
    (paths: string[]) =>
      run((id) => api.discardFiles(id, paths).then(() => "Discarded"), SCOPE_WORKDIR),
    [run],
  )
  const stagePatch = useCallback(
    (patch: string) =>
      run((id) => api.stagePatch(id, patch).then(() => "Hunk staged"), SCOPE_WORKDIR),
    [run],
  )

  const commit = useCallback(
    (message: string, opts?: CommitOptions) =>
      run(async (id) => {
        if (!message.trim()) throw new Error("Commit message is required")
        await api.commit(id, {
          message,
          author_name: "Zync",
          author_email: "zync@local",
          amend: opts?.amend ?? false,
          sign_off: opts?.signOff ?? false,
        })
        if (opts?.pushAfter) await api.push(id)
        return "Committed"
      }, SCOPE_ALL),
    [run],
  )

  const requestBlame = useCallback(
    async (path: string): Promise<BlameRow[]> => {
      const repositoryId = guard()
      const workspaceId = workspaceIdRef.current
      if (!repositoryId || !workspaceId) return []
      const [ranges, file] = await Promise.all([
        api.blame(repositoryId, path),
        api.readFile(workspaceId, path),
      ])
      return buildBlameRows(ranges, file.content)
    },
    [guard],
  )

  const runCommitAction = useCallback(
    async (action: CommitAction, commitId: string) => {
      const repositoryId = guard()
      if (!repositoryId) return
      switch (action) {
        case "copy-sha":
          await navigator.clipboard.writeText(commitId).catch(() => {})
          setNotice(`Copied ${shortId(commitId)}`)
          return
        case "save-patch":
          try {
            const patch = await api.diffCommit(repositoryId, commitId)
            const blob = new Blob([patch], { type: "text/plain" })
            const url = URL.createObjectURL(blob)
            const a = document.createElement("a")
            a.href = url
            a.download = `${shortId(commitId)}.patch`
            a.click()
            URL.revokeObjectURL(url)
            setNotice(`Saved ${shortId(commitId)}.patch`)
          } catch (error) {
            setNotice(error instanceof Error ? error.message : String(error))
          }
          return
        case "compare-local":
          try {
            setSelectedFile("")
            setDiff(await api.diffCommitToWorkdir(repositoryId, commitId))
            setNotice(`Comparing ${shortId(commitId)} to local changes`)
          } catch (error) {
            setNotice(error instanceof Error ? error.message : String(error))
          }
          return
        case "checkout":
          await run(
            (id) =>
              api
                .checkoutRevision(id, commitId)
                .then(() => `Checked out ${shortId(commitId)}`),
            SCOPE_ALL,
          )
          return
        case "cherry-pick":
          await run(
            (id) =>
              api
                .cherryPick(id, [commitId])
                .then(() => `Cherry-picked ${shortId(commitId)}`),
            SCOPE_ALL,
          )
          return
        case "revert":
          await run(
            (id) =>
              api
                .revertCommit(id, commitId)
                .then(() => `Reverted ${shortId(commitId)}`),
            SCOPE_ALL,
          )
          return
      }
    },
    [guard, run],
  )

  // Same shape as `run`, but resolves to a message and rethrows on failure
  // instead of swallowing it — the toolbar needs both to drive its own
  // busy/toast state per button. The footer notice keeps the old fixed
  // "<Verb> complete" wording (kept for the e2e audit's notice assertions);
  // the resolved value prefers the server's own message when it sent one,
  // falling back to that same label so callers (toast) never show blank text.
  const runRemote = useCallback(
    async (
      fn: (repositoryId: string) => Promise<string>,
      scope: number,
      noticeOnSuccess: string,
    ): Promise<string> => {
      const repositoryId = guard()
      if (!repositoryId) throw new Error("Open a repository first")
      try {
        const message = await fn(repositoryId)
        setNotice(noticeOnSuccess)
        refresh(scope)
        return message.trim() ? message : noticeOnSuccess
      } catch (error) {
        const err = error instanceof Error ? error : new Error(String(error))
        setNotice(err.message)
        throw err
      }
    },
    [guard, refresh],
  )

  const fetchRemote = useCallback(
    (all?: boolean) =>
      runRemote((id) => (all ? api.fetchAll(id) : api.fetch(id)), SCOPE_ALL, "Fetch complete"),
    [runRemote],
  )
  const pullRemote = useCallback(
    (mode?: PullMode) =>
      runRemote(
        (id) => api.pullRemote(id, "origin", null, mode ?? null),
        SCOPE_ALL,
        "Pull complete",
      ),
    [runRemote],
  )
  const pushRemote = useCallback(
    (opts?: { forceWithLease?: boolean; setUpstream?: boolean }) =>
      runRemote(
        (id) => api.pushRemote(id, "origin", null, opts),
        SCOPE_ALL,
        "Push complete",
      ),
    [runRemote],
  )

  // Fork-style branch create with local-changes handling around the checkout.
  const createBranch = useCallback(
    (
      name: string,
      opts: { startPoint?: string; checkout: boolean; localMode: LocalChangesMode },
      changedPaths: string[],
    ) =>
      run(async (id) => {
        if (!name.trim()) throw new Error("Branch name is required")
        const handle = opts.checkout && changedPaths.length > 0
        let stashed = false
        if (handle && opts.localMode === "stash-reapply") {
          await api.createStash(id, `Auto-stash before switching to ${name}`)
          stashed = true
        } else if (handle && opts.localMode === "discard") {
          await api.discardFiles(id, changedPaths)
        }
        try {
          if (opts.startPoint?.trim())
            await api.createBranchAt(id, name, opts.startPoint, opts.checkout)
          else await api.createBranch(id, name, opts.checkout)
        } catch (error) {
          if (stashed) await api.applyStash(id, 0, true).catch(() => {})
          throw error
        }
        if (stashed) {
          await api.applyStash(id, 0, true)
          return `Created ${name} and reapplied local changes`
        }
        return `Created branch ${name}`
      }, SCOPE_ALL),
    [run],
  )

  const renameBranch = useCallback(
    (name: string, newName: string) =>
      run(
        (id) =>
          api.renameBranch(id, name, newName).then(() => `Renamed to ${newName}`),
        SCOPE_BRANCHES,
      ),
    [run],
  )
  const deleteBranch = useCallback(
    (name: string) =>
      run((id) => api.deleteBranch(id, name).then(() => `Deleted ${name}`), SCOPE_BRANCHES),
    [run],
  )
  const checkoutBranch = useCallback(
    (name: string) =>
      run((id) => api.checkoutBranch(id, name).then(() => `Checked out ${name}`), SCOPE_ALL),
    [run],
  )
  // Merge that surfaces "already up to date" as a friendly notice, not an error.
  const mergeBranch = useCallback(
    (name: string) =>
      run(async (id) => {
        try {
          await api.mergeBranch(id, name)
          return `Merged ${name}`
        } catch (error) {
          const msg = error instanceof Error ? error.message : String(error)
          if (/up.to.date|nothing to merge/i.test(msg)) return "Already up to date"
          throw error
        }
      }, SCOPE_ALL),
    [run],
  )
  const createTag = useCallback(
    (name: string, target?: string) =>
      run((id) => api.createTag(id, name, target ?? null).then(() => `Created tag ${name}`), SCOPE_GRAPH),
    [run],
  )
  const deleteTag = useCallback(
    (name: string) =>
      run((id) => api.deleteTag(id, name).then(() => `Deleted tag ${name}`), SCOPE_GRAPH),
    [run],
  )
  const createStash = useCallback(
    (message: string) =>
      run((id) => api.createStash(id, message).then(() => "Stash created"), SCOPE_ALL),
    [run],
  )
  const applyStash = useCallback(
    (index: number, pop: boolean) =>
      run((id) => api.applyStash(id, index, pop).then(() => (pop ? "Stash popped" : "Stash applied")), SCOPE_ALL),
    [run],
  )
  const dropStash = useCallback(
    (index: number) =>
      run((id) => api.dropStash(id, index).then(() => "Stash dropped"), SCOPE_STASHES),
    [run],
  )
  const resolveConflict = useCallback(
    (path: string, side: "local" | "remote") =>
      run((id) => api.resolveConflict(id, path, side).then(() => `Resolved ${path}`), SCOPE_ALL),
    [run],
  )

  const runInteractiveRebase = useCallback(
    (
      commits: CommitSummary[],
      targetId: string,
      action: "reword" | "edit" | "squash" | "fixup" | "drop" | "pick",
      message: string | undefined,
      successNotice: string,
    ) =>
      run(async (id) => {
        const { base, steps } = quickRebasePlan(commits, targetId, action, message)
        await api.interactiveRebase(id, { base, steps })
        return successNotice
      }, SCOPE_ALL),
    [run],
  )

  const resetToCommit = useCallback(
    (commitId: string, hard: boolean) =>
      run(
        (id) =>
          api
            .resetToRevision(id, commitId, hard)
            .then(() => `Reset (${hard ? "hard" : "mixed"}) to ${shortId(commitId)}`),
        SCOPE_ALL,
      ),
    [run],
  )

  const loadStats = useCallback(async () => {
    const repositoryId = repoIdRef.current
    if (!repositoryId) return
    try {
      setRepoStats(await api.repoStats(repositoryId))
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error))
    }
  }, [])

  // Live sync: reconnect with backoff, generation-guarded so switching repos
  // retires the previous socket loop (ported from start_live_events).
  useEffect(() => {
    const workspaceId = workspace?.workspace.id
    if (!workspaceId) return

    let generation = 0
    let socket: WebSocket | null = null
    let timer: ReturnType<typeof setTimeout> | null = null
    let attempts = 0
    let connectedBefore = false
    let stopped = false
    const myGen = ++generation

    const isStale = () => stopped || myGen !== generation

    const connect = () => {
      if (isStale()) return
      socket = new WebSocket(api.websocketUrl(workspaceId))
      socket.onopen = () => {
        attempts = 0
        setLiveSyncOk(true)
        if (connectedBefore) {
          setNotice("Live sync reconnected")
          refresh(SCOPE_ALL)
        } else {
          setNotice("Live sync connected")
        }
        connectedBefore = true
      }
      socket.onmessage = (event) => {
        if (isStale()) return
        const scope =
          typeof event.data === "string" ? scopeForEvent(event.data) : SCOPE_ALL
        refresh(scope)
      }
      socket.onclose = () => {
        if (isStale()) return
        setLiveSyncOk(false)
        attempts += 1
        const delay = Math.min(2 ** Math.min(attempts, 5), 30)
        setNotice(`Live sync offline - reconnecting in ${delay}s`)
        timer = setTimeout(connect, delay * 1000)
      }
      socket.onerror = () => socket?.close()
    }
    connect()

    return () => {
      stopped = true
      generation++
      if (timer) clearTimeout(timer)
      socket?.close()
    }
  }, [workspace?.workspace.id, refresh])

  useEffect(() => {
    void loadRepositories()
  }, [loadRepositories])

  return {
    api,
    repositories,
    repositoriesLoaded,
    repositoriesError,
    workspace,
    gitStatus,
    branches,
    commits,
    stashes,
    conflicts,
    diff,
    notice,
    liveSyncOk,
    selectedFile,
    loadRepositories,
    openRepository,
    setRepositoryFavorite,
    removeRepository,
    refresh,
    loadMore,
    setDiff,
    setNotice,
    selectFileDiff,
    stageFiles,
    unstageFiles,
    discardFiles,
    stagePatch,
    commit,
    requestBlame,
    runCommitAction,
    fetchRemote,
    pullRemote,
    pushRemote,
    createBranch,
    renameBranch,
    deleteBranch,
    checkoutBranch,
    mergeBranch,
    createTag,
    deleteTag,
    createStash,
    applyStash,
    dropStash,
    resolveConflict,
    runInteractiveRebase,
    resetToCommit,
    loadStats,
    repoStats,
  }
}
