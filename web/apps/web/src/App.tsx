import { useEffect, useMemo, useState } from "react"

import { FolderGit2, Info, PanelLeft, Plus, RefreshCw } from "lucide-react"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@workspace/ui/components/alert"
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@workspace/ui/components/avatar"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@workspace/ui/components/empty"
import { Input } from "@workspace/ui/components/input"
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@workspace/ui/components/resizable"
import { Separator } from "@workspace/ui/components/separator"
import { Skeleton } from "@workspace/ui/components/skeleton"
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@workspace/ui/components/sheet"
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@workspace/ui/components/tabs"
import { toast } from "@workspace/ui/components/toast"
import { cn } from "@workspace/ui/lib/utils"

import {
  BranchSidebar,
  type BranchCommand,
  type TagCommand,
} from "./components/BranchSidebar"
import { AdminUsersSheet } from "./components/AdminUsersSheet"
import { BisectBanner } from "./components/BisectBanner"
import { CommandPalette } from "./components/CommandPalette"
import { CommitGraph } from "./components/CommitGraph"
import { ConflictResolver } from "./components/ConflictResolver"
import { DiffPanel } from "./components/DiffPanel"
import { FileHistorySheet } from "./components/FileHistorySheet"
import { GitToolsPanel } from "./components/GitToolsPanel"
import { RepoMinibar } from "./components/RepoMinibar"
import { RepoStatsPanel } from "./components/RepoStatsPanel"
import { ShortcutsDialog } from "./components/ShortcutsDialog"
import { SyncStatusBanner } from "./components/SyncStatusBanner"
import { Toolbar } from "./components/Toolbar"
import { UserMenu } from "./components/UserMenu"
import {
  AddRepositoryDialog,
  BisectStartDialog,
  BranchMergeChooserDialog,
  DeleteDialog,
  DeleteTagDialog,
  DropDialog,
  InteractiveRebaseDialog,
  MergeDialog,
  NewBranchDialog,
  RenameDialog,
  ResetDialog,
  RevertParentDialog,
  RewordDialog,
  StashApplyDialog,
  TagDialog,
} from "./components/dialogs"
import { useIsMobile } from "./hooks/use-mobile"
import { useShortcuts } from "./hooks/use-shortcuts"
import { WORKDIR_REVISION } from "./lib/api"
import { graphRows, statusLabel, type BlameRow } from "./lib/helpers"
import { formatCommitTime, gravatarSrc, shortId } from "./lib/format"
import type {
  CommitSummary,
  CreateRepositoryRequest,
  CurrentUser,
  RepositoryRecord,
} from "./lib/types"
import { useWorkspace } from "./lib/useWorkspace"

type CenterMode = "changes" | "commits"
type DetailTab = "commit" | "repository" | "tools"

// The dialog currently open, carrying the data it needs.
type ActiveDialog =
  | { kind: "newBranch"; at: string }
  | { kind: "tag"; target: string }
  | { kind: "rename"; name: string }
  | { kind: "delete"; name: string }
  | { kind: "deleteTag"; name: string }
  | { kind: "merge"; name: string }
  | { kind: "reword"; commitId: string; message: string }
  | { kind: "reset"; commitId: string }
  | { kind: "drop"; commitId: string }
  | { kind: "stashApply"; index: number }
  | { kind: "revertParent"; commitId: string; parents: string[] }
  | { kind: "interactiveRebase"; commitId: string }
  | { kind: "branchMergeChooser"; source: string; target: string }
  | { kind: "bisectStart"; commitId: string }
  | null

export function App({
  currentUser,
  onLogout,
}: {
  currentUser: CurrentUser
  onLogout: () => void
}) {
  const ws = useWorkspace()
  const [selectedCommit, setSelectedCommit] = useState<string | null>(null)
  const [mode, setMode] = useState<CenterMode>("commits")
  // Detail-aside tab + the Git Tools sub-tab, controlled so the header user menu
  // can deep-link into the Credentials settings (P3.4).
  const [detailTab, setDetailTab] = useState<DetailTab>("commit")
  const [toolsTab, setToolsTab] = useState("remotes")
  const [message, setMessage] = useState("")
  const [blame, setBlame] = useState<BlameRow[] | null>(null)
  const [dialog, setDialog] = useState<ActiveDialog>(null)
  const [addRepoOpen, setAddRepoOpen] = useState(false)
  // P3.5 admin user management, opened from the header UserMenu.
  const [adminUsersOpen, setAdminUsersOpen] = useState(false)
  // P2.3 command palette + keyboard-shortcuts cheat sheet.
  const [paletteOpen, setPaletteOpen] = useState(false)
  const [shortcutsOpen, setShortcutsOpen] = useState(false)
  // Commit search/filter (P1.3). historyResults is null until a full-history
  // search has run; non-null replaces CommitGraph's list with a flat results view.
  const [commitQuery, setCommitQuery] = useState("")
  const [historyResults, setHistoryResults] = useState<CommitSummary[] | null>(
    null,
  )
  const [searchingHistory, setSearchingHistory] = useState(false)
  // File History view (P1.2) — non-null path opens the sheet for that file.
  const [fileHistoryTarget, setFileHistoryTarget] = useState<string | null>(
    null,
  )
  // Jump-to-commit (P1.2, from a blame row or a file-history entry) can target
  // a commit outside the loaded graph window / historyResults; this holds a
  // one-off lookup for that case so `selected` below can still resolve it
  // without disturbing the full-history search UI in CommitGraph.
  const [jumpCommit, setJumpCommit] = useState<CommitSummary | null>(null)

  const isMobile = useIsMobile()
  const [branchSheetOpen, setBranchSheetOpen] = useState(false)
  const [detailSheetOpen, setDetailSheetOpen] = useState(false)

  const currentRepoId = ws.workspace?.repository.id ?? null

  useEffect(() => {
    // Skip while an open is already in flight (auto-open or an explicit switch) —
    // otherwise this effect can re-fire (e.g. ws.repositories getting a new
    // reference) before that open resolves and start a second, competing default-repo
    // open that races the one already running. See useWorkspace's openRepository
    // generation guard and tests/e2e/README.md "Known non-bugs" for the race this avoids.
    if (!ws.workspace && !ws.openingRepository && ws.repositories.length > 0) {
      void ws.openRepository(ws.repositories[0].id)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ws.repositories, ws.workspace, ws.openingRepository])

  async function handleCreateRepository(
    request: CreateRepositoryRequest,
  ): Promise<string> {
    const created = await ws.api.createRepository(request)
    await ws.loadRepositories()
    await ws.openRepository(created.repository.id)
    return created.repository.id
  }

  function handleToggleFavorite(repo: RepositoryRecord) {
    void ws
      .setRepositoryFavorite(repo.id, !repo.favorite)
      .then(() =>
        toast.add({
          title: repo.favorite
            ? `Removed ${repo.name} from favorites`
            : `Favorited ${repo.name}`,
          type: "success",
        }),
      )
      .catch((error) =>
        toast.add({
          title: error instanceof Error ? error.message : String(error),
          type: "error",
        }),
      )
  }

  function handleRemoveRepository(repo: RepositoryRecord) {
    void ws
      .removeRepository(repo.id)
      .then(() =>
        toast.add({ title: `Removed ${repo.name} from Zync`, type: "success" }),
      )
      .catch((error) =>
        toast.add({
          title: error instanceof Error ? error.message : String(error),
          type: "error",
        }),
      )
  }

  function handleCommitQueryChange(value: string) {
    setCommitQuery(value)
    // A changed (or cleared) query invalidates a prior full-history result set.
    if (historyResults !== null) setHistoryResults(null)
  }

  function handleClearHistoryResults() {
    setHistoryResults(null)
  }

  async function handleSearchAllHistory() {
    const query = commitQuery.trim()
    if (!query) return
    setSearchingHistory(true)
    try {
      const results = await ws.searchCommits(commitQuery)
      setHistoryResults(results)
      if (results.length === 0) {
        toast.add({ title: `No commits match "${query}"`, type: "info" })
      }
    } catch (error) {
      toast.add({
        title: error instanceof Error ? error.message : String(error),
        type: "error",
      })
    } finally {
      setSearchingHistory(false)
    }
  }

  const rows = useMemo(() => graphRows(ws.commits), [ws.commits])
  const selected =
    ws.commits.find((c) => c.id === selectedCommit) ??
    historyResults?.find((c) => c.id === selectedCommit) ??
    (jumpCommit?.id === selectedCommit ? jumpCommit : undefined) ??
    ws.commits[0] ??
    null

  // P1.4: All Commits mode has no per-file selection like Local Changes does,
  // so the moment a commit is the active detail target, fetch its whole-commit
  // patch so the Commit tab can render DiffPanel's file tree + per-file diff
  // (previously only reachable via Local Changes' workdir diff).
  useEffect(() => {
    if (mode !== "commits" || !selected) return
    void ws.loadCommitDiff(selected.id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, selected?.id, ws.loadCommitDiff])

  // Blame gutter / file-history "View commit" -> select that commit in the
  // graph and switch to its detail (P1.2). The target may be outside the
  // loaded graph window/historyResults, so fall back to a one-off search-by-sha
  // lookup (search_commits matches a full SHA substring) purely to populate
  // CommitDetail — this intentionally does not touch historyResults/CommitGraph's
  // search-results view.
  function handleJumpToCommit(commitId: string) {
    setSelectedCommit(commitId)
    setMode("commits")
    if (isMobile) setDetailSheetOpen(true)
    const known =
      ws.commits.some((c) => c.id === commitId) ||
      historyResults?.some((c) => c.id === commitId)
    if (known) {
      setJumpCommit(null)
      return
    }
    void ws
      .searchCommits(commitId)
      .then((results) => setJumpCommit(results.find((c) => c.id === commitId) ?? null))
      .catch(() => setJumpCommit(null))
  }

  function openFileHistory(path: string) {
    if (path === "") return
    setFileHistoryTarget(path)
  }

  // Deep-link from the header user menu into the Git Tools → Credentials tab,
  // which only renders in All Commits mode's detail aside (P3.4).
  function openCredentials() {
    setMode("commits")
    setDetailTab("tools")
    setToolsTab("credentials")
    if (isMobile) setDetailSheetOpen(true)
  }

  // P2.3 — palette + shortcut handlers. Toasts the outcome of a remote op the
  // same way the Toolbar buttons do (useWorkspace's remote actions resolve to a
  // message / reject with the error).
  function toastRemote(op: Promise<string>) {
    op.then((message) => toast.add({ title: message, type: "success" })).catch(
      (error) =>
        toast.add({
          title: error instanceof Error ? error.message : String(error),
          type: "error",
        }),
    )
  }

  function focusCommitSearch() {
    // The search field only renders in All Commits mode; focus it once painted.
    setMode("commits")
    requestAnimationFrame(() => {
      document
        .querySelector<HTMLInputElement>('[data-testid="search-input"]')
        ?.focus()
    })
  }

  function selectCommitFromPalette(commitId: string) {
    setSelectedCommit(commitId)
    setMode("commits")
    if (isMobile) setDetailSheetOpen(true)
  }

  function commitFromShortcut() {
    if (!currentRepoId || !message.trim()) return
    void ws.commit(message).then(() => setMessage(""))
  }

  const headBranch = ws.branches.find((b) => b.is_head)?.name ?? "HEAD"

  useShortcuts({
    onOpenPalette: () => setPaletteOpen((open) => !open),
    onCommit: commitFromShortcut,
    onFocusSearch: focusCommitSearch,
    onRefresh: () => ws.refresh(),
    onShowShortcuts: () => setShortcutsOpen(true),
    hasRepo: currentRepoId !== null,
  })

  const changedPaths = ws.gitStatus
    .filter((f) => f.staged || f.unstaged || f.untracked || f.conflicted)
    .map((f) => f.path)

  function onBranchCommand(cmd: BranchCommand) {
    switch (cmd.kind) {
      case "checkout":
        void ws.checkoutBranch(cmd.name)
        break
      case "merge":
        setDialog({ kind: "merge", name: cmd.name })
        break
      case "delete":
        setDialog({ kind: "delete", name: cmd.name })
        break
      case "rename":
        setDialog({ kind: "rename", name: cmd.name })
        break
      case "newBranch":
        setDialog({ kind: "newBranch", at: cmd.name })
        break
      case "newTag":
        setDialog({ kind: "tag", target: cmd.name })
        break
      case "rebase":
        void ws.rebaseBranch(cmd.name)
        break
      case "interactiveRebase":
        // The commit-menu interactive rebase editor (P1.6, InteractiveRebaseDialog) targets a
        // single commit's range, not "onto a whole other branch" — no equivalent flow exists yet
        // for a branch-onto-branch interactive rebase, so this stays a stub.
        ws.setNotice("Interactive rebase onto a branch is available from a commit menu")
        break
      case "dropMergeChooser":
        setDialog({
          kind: "branchMergeChooser",
          source: cmd.source,
          target: cmd.target,
        })
        break
    }
  }

  function onTagCommand(cmd: TagCommand) {
    switch (cmd.kind) {
      case "checkout":
        // Reuses the same detached checkout-at-revision path commits use — a tag
        // name resolves via revparse the same way a commit id does.
        void ws.runCommitAction("checkout", cmd.name)
        break
      case "push":
        ws.pushTag(cmd.name)
          .then((message) => toast.add({ title: message, type: "success" }))
          .catch((error) =>
            toast.add({
              title: error instanceof Error ? error.message : String(error),
              type: "error",
            }),
          )
        break
      case "copySha":
        navigator.clipboard
          .writeText(cmd.sha)
          .then(() => ws.setNotice(`Copied ${shortId(cmd.sha)}`))
          .catch(() =>
            toast.add({ title: "Couldn't copy to clipboard", type: "error" }),
          )
        break
      case "delete":
        setDialog({ kind: "deleteTag", name: cmd.name })
        break
    }
  }

  // Shared between the desktop panel layout and the mobile single-column +
  // sheet layout, so both render identical content.
  const centerMain = (
    <main className="flex h-full min-h-0 flex-col">
          <Tabs
            value={mode}
            onValueChange={(v) => setMode(v as CenterMode)}
            className="border-border shrink-0 gap-0 border-b px-3 py-1"
          >
            <TabsList>
              <TabsTrigger value="changes" data-testid="changes-tab">
                Local Changes ({ws.gitStatus.length})
              </TabsTrigger>
              <TabsTrigger value="commits" data-testid="commits-tab">
                All Commits
              </TabsTrigger>
            </TabsList>
          </Tabs>

          {mode === "changes" ? (
            <>
              <ul className="min-h-0 flex-1 overflow-y-auto">
                {ws.gitStatus.length === 0 && (
                  <li className="text-muted-foreground grid h-32 place-items-center text-sm">
                    No local changes
                  </li>
                )}
                {ws.gitStatus.map((file) => (
                  <li
                    key={file.path}
                    data-testid="local-change-row"
                    data-path={file.path}
                    className={cn(
                      "border-border/50 flex h-8 items-center gap-2 border-b px-3 text-sm",
                      file.path === ws.selectedFile && "bg-accent",
                    )}
                  >
                    <button
                      className="flex min-w-0 flex-1 items-center gap-2 text-left"
                      onClick={() => {
                        void ws.selectFileDiff(file.path)
                        if (isMobile) setDetailSheetOpen(true)
                      }}
                    >
                      <span
                        className={cn(
                          "w-3 shrink-0 text-center font-bold",
                          statusColor(file),
                        )}
                      >
                        {statusLabel(file)}
                      </span>
                      <code className="min-w-0 truncate">{file.path}</code>
                    </button>
                    {!file.untracked && (
                      <Button
                        data-testid="file-history-btn"
                        variant="ghost"
                        size="xs"
                        className="shrink-0"
                        onClick={() => openFileHistory(file.path)}
                      >
                        History
                      </Button>
                    )}
                    {(file.unstaged || file.untracked || file.conflicted) && (
                      <Button
                        data-testid="stage-btn"
                        variant="ghost"
                        size="xs"
                        className="shrink-0"
                        onClick={() => void ws.stageFiles([file.path])}
                      >
                        Stage
                      </Button>
                    )}
                    {file.staged && (
                      <Button
                        data-testid="unstage-btn"
                        variant="ghost"
                        size="xs"
                        className="shrink-0"
                        onClick={() => void ws.unstageFiles([file.path])}
                      >
                        Unstage
                      </Button>
                    )}
                  </li>
                ))}
              </ul>
              <div className="border-border flex shrink-0 items-center gap-2 border-t p-3">
                <Input
                  data-testid="commit-input"
                  value={message}
                  onChange={(e) => setMessage(e.target.value)}
                  placeholder="Commit message"
                  className="h-8"
                />
                <Button
                  data-testid="commit-btn"
                  size="sm"
                  className="h-8"
                  disabled={!message.trim()}
                  onClick={() =>
                    void ws.commit(message).then(() => setMessage(""))
                  }
                >
                  Commit
                </Button>
              </div>
            </>
          ) : (
            <CommitGraph
              rows={rows}
              selectedId={selected?.id ?? null}
              onSelect={(id) => {
                setSelectedCommit(id)
                if (isMobile) setDetailSheetOpen(true)
              }}
              onLoadMore={ws.loadMore}
              searchQuery={commitQuery}
              onSearchQueryChange={handleCommitQueryChange}
              historyResults={historyResults}
              onSearchAllHistory={() => void handleSearchAllHistory()}
              onClearHistoryResults={handleClearHistoryResults}
              searchingHistory={searchingHistory}
              loading={ws.workspaceLoading}
              bisectActive={ws.bisectStatus?.in_progress ?? false}
              onMenuAction={(action, commitId) => {
                const commit = ws.commits.find((c) => c.id === commitId)
                switch (action) {
                  case "new-branch":
                    setDialog({ kind: "newBranch", at: commitId })
                    break
                  case "new-tag":
                    setDialog({ kind: "tag", target: commitId })
                    break
                  case "reword":
                    setDialog({
                      kind: "reword",
                      commitId,
                      message: commit?.summary ?? "",
                    })
                    break
                  case "reset-here":
                    setDialog({ kind: "reset", commitId })
                    break
                  case "drop":
                    setDialog({ kind: "drop", commitId })
                    break
                  case "interactive-rebase":
                    setDialog({ kind: "interactiveRebase", commitId })
                    break
                  case "bisect-start":
                    setDialog({ kind: "bisectStart", commitId })
                    break
                  case "bisect-good":
                    void ws.bisectGood(commitId)
                    break
                  case "bisect-bad":
                    void ws.bisectBad(commitId)
                    break
                  case "edit":
                  case "squash":
                  case "fixup":
                    void ws.runInteractiveRebase(
                      ws.commits,
                      commitId,
                      action,
                      undefined,
                      `${action} ${shortId(commitId)}`,
                    )
                    break
                  case "revert":
                    if (commit && commit.parents.length >= 2) {
                      setDialog({
                        kind: "revertParent",
                        commitId,
                        parents: commit.parents,
                      })
                    } else {
                      void ws.runCommitAction("revert", commitId)
                    }
                    break
                  case "checkout":
                  case "cherry-pick":
                  case "copy-sha":
                  case "save-patch":
                  case "compare-local":
                    void ws.runCommitAction(action, commitId)
                    break
                  default:
                    ws.setNotice(`${action} needs a target branch`)
                }
              }}
            />
          )}
        </main>
  )

  const detailAside = (
        <aside className="flex h-full min-h-0 flex-col overflow-hidden">
          {ws.conflicts.length > 0 && (
            <div className="border-border max-h-48 overflow-y-auto border-b p-3">
              <ConflictResolver
                conflicts={ws.conflicts}
                onResolve={(path, side) => void ws.resolveConflict(path, side)}
              />
            </div>
          )}
          {mode === "changes" ? (
            <DiffPanel
              path={ws.selectedFile}
              diff={ws.diff}
              blame={blame}
              onStageHunk={(patch) => void ws.stagePatch(patch)}
              onRequestBlame={() => {
                if (!ws.selectedFile) return
                void ws.requestBlame(ws.selectedFile).then(setBlame)
              }}
              onCloseBlame={() => setBlame(null)}
              imageSrc={(path, side) =>
                currentRepoId
                  ? ws.api.blobUrl(
                      currentRepoId,
                      side === "before" ? "HEAD" : WORKDIR_REVISION,
                      path,
                    )
                  : null
              }
              onOpenFileHistory={openFileHistory}
              onSelectBlameCommit={handleJumpToCommit}
            />
          ) : (
            <Tabs
              value={detailTab}
              className="min-h-0 flex-1 gap-0"
              onValueChange={(v) => {
                setDetailTab(v as DetailTab)
                if (v === "repository") void ws.loadStats()
              }}
            >
              <TabsList className="mx-3 mt-2">
                <TabsTrigger value="commit" data-testid="detail-tab-commit">
                  Commit
                </TabsTrigger>
                <TabsTrigger value="repository" data-testid="detail-tab-repository">
                  Repository
                </TabsTrigger>
                <TabsTrigger value="tools" data-testid="detail-tab-tools">
                  Git Tools
                </TabsTrigger>
              </TabsList>
              <TabsContent
                value="commit"
                className="flex min-h-0 flex-1 flex-col overflow-hidden"
              >
                <div className="shrink-0 overflow-y-auto p-4">
                  <CommitDetail commit={selected} />
                </div>
                {selected && (
                  <>
                    <Separator />
                    <div className="min-h-0 flex-1" data-testid="commit-diff-panel">
                      {ws.selectedCommitDiffLoading ? (
                        <div
                          data-testid="diff-loading-skeleton"
                          aria-busy="true"
                          aria-label="Loading commit diff"
                          className="flex flex-col gap-2 p-4"
                        >
                          {["70%", "90%", "55%", "80%", "40%", "85%", "60%"].map(
                            (width, index) => (
                              <Skeleton
                                key={index}
                                className="h-3"
                                style={{ width }}
                              />
                            ),
                          )}
                        </div>
                      ) : ws.selectedCommitDiffError ? (
                        <div className="text-destructive p-4 text-sm">
                          {ws.selectedCommitDiffError}
                        </div>
                      ) : (
                        <DiffPanel
                          path=""
                          diff={ws.selectedCommitDiff}
                          blame={null}
                          onRequestBlame={() => {}}
                          onCloseBlame={() => {}}
                          imageSrc={(path, side) => {
                            if (!currentRepoId) return null
                            if (side === "before") {
                              const parent = selected.parents[0]
                              return parent
                                ? ws.api.blobUrl(currentRepoId, parent, path)
                                : null
                            }
                            return ws.api.blobUrl(currentRepoId, selected.id, path)
                          }}
                          onOpenFileHistory={openFileHistory}
                          onSelectBlameCommit={handleJumpToCommit}
                        />
                      )}
                    </div>
                  </>
                )}
              </TabsContent>
              <TabsContent
                value="repository"
                className="min-h-0 flex-1 overflow-y-auto p-4"
              >
                <RepoStatsPanel stats={ws.repoStats} />
              </TabsContent>
              <TabsContent
                value="tools"
                className="min-h-0 flex-1 overflow-y-auto p-4"
              >
                <GitToolsPanel
                  repositoryId={currentRepoId}
                  onRefresh={() => ws.refresh()}
                  tab={toolsTab}
                  onTabChange={setToolsTab}
                />
              </TabsContent>
            </Tabs>
          )}
        </aside>
  )

  // Empty state: nothing registered yet. Skip the toolbar/panel chrome
  // entirely and surface a single CTA into the Add/Clone/Init dialog.
  if (ws.repositoriesLoaded && ws.repositories.length === 0) {
    return (
      <div className="bg-background text-foreground flex h-svh">
        <RepoMinibar
          repos={[]}
          activeId={null}
          onSelect={() => {}}
          onAddRepository={() => setAddRepoOpen(true)}
          onToggleFavorite={() => {}}
          onRemoveRepository={() => {}}
        />
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <header className="border-border flex h-12 shrink-0 items-center gap-2 border-b px-3">
            <span className="bg-primary size-2 rounded-full" />
            <span className="text-sm font-semibold">Zync</span>
            <div className="ml-auto">
              <UserMenu
                user={currentUser}
                onLogout={onLogout}
                onOpenCredentials={openCredentials}
                onOpenAdminUsers={() => setAdminUsersOpen(true)}
              />
            </div>
          </header>
          <div className="flex min-h-0 flex-1 items-center justify-center p-6">
          {ws.repositoriesError !== null ? (
            // The load itself failed (server unreachable, etc.) — this is not the same as a
            // genuine zero-repositories registry, so say so instead of silently showing the
            // "No repositories yet" copy over a hidden error (ws.notice is in the footer only,
            // which isn't visible in this empty-chrome layout).
            <Empty>
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <FolderGit2 />
                </EmptyMedia>
                <EmptyTitle>Couldn&rsquo;t load repositories</EmptyTitle>
                <EmptyDescription>
                  The repository list failed to load — this isn&rsquo;t
                  necessarily an empty registry.
                </EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Alert variant="destructive" className="text-left">
                  <AlertTitle>Load failed</AlertTitle>
                  <AlertDescription
                    className="break-words"
                    data-testid="repositories-load-error"
                  >
                    {ws.repositoriesError}
                  </AlertDescription>
                </Alert>
                <Button
                  variant="outline"
                  data-testid="retry-load-repositories-btn"
                  onClick={() => void ws.loadRepositories()}
                >
                  <RefreshCw data-icon="inline-start" />
                  Retry
                </Button>
              </EmptyContent>
            </Empty>
          ) : (
            <Empty>
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <FolderGit2 />
                </EmptyMedia>
                <EmptyTitle>No repositories yet</EmptyTitle>
                <EmptyDescription>
                  Add an existing repository, clone one from a URL, or start a
                  new one.
                </EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Button
                  data-testid="empty-add-repo-btn"
                  onClick={() => setAddRepoOpen(true)}
                >
                  <Plus data-icon="inline-start" />
                  Add Repository
                </Button>
              </EmptyContent>
            </Empty>
          )}
          </div>
        </div>
        <AddRepositoryDialog
          open={addRepoOpen}
          onOpenChange={setAddRepoOpen}
          onCreate={handleCreateRepository}
        />
      </div>
    )
  }

  return (
    <div className="bg-background text-foreground flex h-svh">
      <RepoMinibar
        repos={ws.repositories}
        activeId={currentRepoId}
        onSelect={(id) => void ws.openRepository(id)}
        onAddRepository={() => setAddRepoOpen(true)}
        onToggleFavorite={handleToggleFavorite}
        onRemoveRepository={handleRemoveRepository}
      />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="border-border flex h-12 shrink-0 items-center gap-2 border-b px-3">
        <Button
          variant="ghost"
          size="icon-sm"
          className="md:hidden"
          aria-label="Open branches"
          onClick={() => setBranchSheetOpen(true)}
        >
          <PanelLeft />
        </Button>
        <span className="bg-primary size-2 rounded-full" />
        <span className="text-sm font-semibold">Zync</span>
        <Toolbar
          disabled={!currentRepoId}
          branches={ws.branches}
          onFetch={(all) => ws.fetchRemote(all)}
          onPull={(mode) => ws.pullRemote(mode)}
          onPush={(opts) => ws.pushRemote(opts)}
          onStash={() => void ws.createStash("WIP from Zync")}
        />
        <div className="ml-auto flex items-center gap-1">
          <UserMenu
            user={currentUser}
            onLogout={onLogout}
            onOpenCredentials={openCredentials}
            onOpenAdminUsers={() => setAdminUsersOpen(true)}
          />
          <Button
            variant="ghost"
            size="icon-sm"
            className="md:hidden"
            aria-label="Open details"
            onClick={() => setDetailSheetOpen(true)}
          >
            <Info />
          </Button>
        </div>
      </header>

      {ws.liveSyncReconnecting && <SyncStatusBanner />}

      {ws.bisectStatus?.in_progress && (
        <BisectBanner
          status={ws.bisectStatus}
          onGood={() => void ws.bisectGood()}
          onBad={() => void ws.bisectBad()}
          onSkip={() => void ws.bisectSkip()}
          onReset={() => void ws.bisectReset()}
        />
      )}

      {isMobile ? (
        <div className="flex min-h-0 flex-1 flex-col">{centerMain}</div>
      ) : (
        <ResizablePanelGroup
          orientation="horizontal"
          className="min-h-0 flex-1"
        >
          <ResizablePanel defaultSize="18" minSize="12" maxSize="35">
            <aside className="h-full min-h-0 overflow-y-auto">
              <BranchSidebar
                branches={ws.branches}
                tags={ws.tags}
                onCommand={onBranchCommand}
                onTagCommand={onTagCommand}
                loading={ws.workspaceLoading}
              />
            </aside>
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize="52" minSize="30">
            {centerMain}
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize="30" minSize="20">
            {detailAside}
          </ResizablePanel>
        </ResizablePanelGroup>
      )}

      <footer className="border-border text-muted-foreground flex h-7 shrink-0 items-center gap-2 border-t px-3 text-xs">
        <span
          className={cn(
            "size-1.5 rounded-full",
            ws.liveSyncOk ? "bg-primary" : "bg-destructive",
          )}
        />
        <span className="truncate" data-testid="notice">
          {ws.notice}
        </span>
      </footer>
      </div>

      {/* Mobile overlays: branches (left) and detail (right) sheets. */}
      {isMobile && (
        <Sheet open={branchSheetOpen} onOpenChange={setBranchSheetOpen}>
          <SheetContent side="left" className="gap-0">
            <SheetHeader className="border-border border-b">
              <SheetTitle>Branches</SheetTitle>
            </SheetHeader>
            <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
              <nav aria-label="Repositories" className="flex flex-col gap-1 p-2">
                {ws.repositories.map((repo) => (
                  <Button
                    key={repo.id}
                    variant={repo.id === currentRepoId ? "secondary" : "ghost"}
                    size="sm"
                    className="justify-start"
                    onClick={() => {
                      void ws.openRepository(repo.id)
                      setBranchSheetOpen(false)
                    }}
                  >
                    <span className="truncate">{repo.name}</span>
                  </Button>
                ))}
              </nav>
              <Separator />
              <BranchSidebar
                branches={ws.branches}
                tags={ws.tags}
                loading={ws.workspaceLoading}
                onCommand={(cmd) => {
                  onBranchCommand(cmd)
                  if (cmd.kind === "checkout") setBranchSheetOpen(false)
                }}
                onTagCommand={(cmd) => {
                  onTagCommand(cmd)
                  if (cmd.kind === "checkout") setBranchSheetOpen(false)
                }}
              />
            </div>
          </SheetContent>
        </Sheet>
      )}
      {isMobile && (
        <Sheet open={detailSheetOpen} onOpenChange={setDetailSheetOpen}>
          <SheetContent side="right" className="gap-0">
            <SheetHeader className="border-border border-b">
              <SheetTitle>Details</SheetTitle>
            </SheetHeader>
            <div className="flex min-h-0 flex-1 flex-col">{detailAside}</div>
          </SheetContent>
        </Sheet>
      )}

      {/* Dialogs */}
      {dialog?.kind === "newBranch" && (
        <NewBranchDialog
          open
          onOpenChange={(o) => !o && setDialog(null)}
          branch={dialog.at}
          startPoint={dialog.at}
          hasLocalChanges={changedPaths.length > 0}
          onSubmit={(p) => {
            void ws.createBranch(
              p.name,
              { startPoint: p.startPoint, checkout: p.checkout, localMode: p.localMode },
              changedPaths,
            )
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "tag" && (
        <TagDialog
          open
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) => {
            void ws.createTag(p.name, p.target || dialog.target)
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "rename" && (
        <RenameDialog
          open
          branch={dialog.name}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) => {
            void ws.renameBranch(dialog.name, p.newName)
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "delete" && (
        <DeleteDialog
          open
          branch={dialog.name}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={() => {
            void ws.deleteBranch(dialog.name)
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "deleteTag" && (
        <DeleteTagDialog
          open
          tag={dialog.name}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={() => {
            void ws.deleteTag(dialog.name)
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "merge" && (
        <MergeDialog
          open
          branch={dialog.name}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) => {
            void ws.mergeBranch(dialog.name, p.strategy)
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "branchMergeChooser" && (
        <BranchMergeChooserDialog
          open
          source={dialog.source}
          target={dialog.target}
          onOpenChange={(o) => !o && setDialog(null)}
          onChoose={(choice) => {
            // Reuses the exact BranchCommand the right-click context menu
            // already emits for `source` — merge opens MergeDialog, rebase
            // runs ws.rebaseBranch directly (P2.4).
            setDialog(null)
            onBranchCommand({ kind: choice, name: dialog.source })
          }}
        />
      )}
      {dialog?.kind === "revertParent" && (
        <RevertParentDialog
          open
          commit={shortId(dialog.commitId)}
          parents={dialog.parents}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) => {
            void ws.runCommitAction("revert", dialog.commitId, {
              mainline: p.mainline,
            })
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "reword" && (
        <RewordDialog
          open
          commit={shortId(dialog.commitId)}
          message={dialog.message}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) => {
            void ws.runInteractiveRebase(
              ws.commits,
              dialog.commitId,
              "reword",
              p.message,
              `Reworded ${shortId(dialog.commitId)}`,
            )
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "interactiveRebase" && (
        <InteractiveRebaseDialog
          open
          commits={ws.commits}
          targetId={dialog.commitId}
          gitStatus={ws.gitStatus}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) =>
            void ws.runInteractiveRebasePlan(
              p.base,
              p.steps,
              `Rebased ${p.steps.length} commit${p.steps.length === 1 ? "" : "s"} onto ${shortId(p.base)}`,
            )
          }
        />
      )}
      {dialog?.kind === "bisectStart" && (
        <BisectStartDialog
          open
          bad={dialog.commitId}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) => void ws.bisectStart(p.bad, [p.good])}
        />
      )}
      {dialog?.kind === "reset" && (
        <ResetDialog
          open
          commit={shortId(dialog.commitId)}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={(p) => {
            void ws.resetToCommit(dialog.commitId, p.mode === "hard")
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "drop" && (
        <DropDialog
          open
          commit={shortId(dialog.commitId)}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={() => {
            void ws.runInteractiveRebase(
              ws.commits,
              dialog.commitId,
              "drop",
              undefined,
              `Dropped ${shortId(dialog.commitId)}`,
            )
            setDialog(null)
          }}
        />
      )}
      {dialog?.kind === "stashApply" &&
        (() => {
          const stash = ws.stashes.find((s) => s.index === dialog.index)
          if (!stash) return null
          return (
            <StashApplyDialog
              open
              stash={stash}
              onOpenChange={(o) => !o && setDialog(null)}
              onSubmit={(p) => {
                void ws.applyStash(dialog.index, p.dropAfterApply)
                setDialog(null)
              }}
            />
          )
        })()}
      <AddRepositoryDialog
        open={addRepoOpen}
        onOpenChange={setAddRepoOpen}
        onCreate={handleCreateRepository}
      />
      <AdminUsersSheet open={adminUsersOpen} onOpenChange={setAdminUsersOpen} />
      <FileHistorySheet
        open={fileHistoryTarget !== null}
        onOpenChange={(open) => !open && setFileHistoryTarget(null)}
        path={fileHistoryTarget ?? ""}
        repositoryId={currentRepoId}
        fileHistory={ws.fileHistory}
        diffCommit={(repositoryId, commitId) =>
          ws.api.diffCommit(repositoryId, commitId)
        }
        blobText={(repositoryId, revision, path) =>
          ws.api.blobText(repositoryId, revision, path)
        }
        blobUrl={(repositoryId, revision, path) =>
          ws.api.blobUrl(repositoryId, revision, path)
        }
        onJumpToCommit={(commitId) => {
          setFileHistoryTarget(null)
          handleJumpToCommit(commitId)
        }}
      />
      <CommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        hasRepo={currentRepoId !== null}
        repositories={ws.repositories}
        activeRepoId={currentRepoId}
        branches={ws.branches}
        commits={ws.commits}
        onOpenRepository={(id) => void ws.openRepository(id)}
        onCheckoutBranch={(name) => void ws.checkoutBranch(name)}
        onSelectCommit={selectCommitFromPalette}
        onFetch={() => toastRemote(ws.fetchRemote())}
        onFetchAll={() => toastRemote(ws.fetchRemote(true))}
        onPull={(mode) => toastRemote(ws.pullRemote(mode))}
        onPush={() => toastRemote(ws.pushRemote())}
        onStash={() => void ws.createStash("WIP from Zync")}
        onNewBranch={() => setDialog({ kind: "newBranch", at: headBranch })}
        onNewTag={() => setDialog({ kind: "tag", target: "HEAD" })}
        onFocusSearch={focusCommitSearch}
        onRefresh={() => ws.refresh()}
        onShowShortcuts={() => setShortcutsOpen(true)}
      />
      <ShortcutsDialog open={shortcutsOpen} onOpenChange={setShortcutsOpen} />
    </div>
  )
}

// Returns a semantic text-color class for a file's status marker letter. The
// glyph (from statusLabel) carries the meaning; color adds subtle emphasis.
function statusColor(file: {
  conflicted: boolean
  untracked: boolean
  staged: boolean
}): string {
  if (file.conflicted) return "text-destructive"
  if (file.untracked) return "text-muted-foreground"
  if (file.staged) return "text-primary"
  return "text-foreground"
}

function CommitDetail({
  commit,
}: {
  commit: ReturnType<typeof useWorkspace>["commits"][number] | null
}) {
  if (!commit) {
    return <div className="text-muted-foreground text-sm">No commit selected.</div>
  }
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3">
        <CommitAvatar email={commit.author_email} name={commit.author} />
        <div className="min-w-0">
          <SectionTitle>Author</SectionTitle>
          <div className="font-medium">{commit.author}</div>
          {commit.author_email && (
            <div className="text-muted-foreground text-xs">
              {commit.author_email}
            </div>
          )}
          <div className="text-muted-foreground text-xs">
            {formatCommitTime(commit.time)}
          </div>
        </div>
      </div>
      {commit.refs.length > 0 && (
        <div>
          <SectionTitle>Refs</SectionTitle>
          <div className="flex flex-wrap gap-1">
            {commit.refs.map((ref) => (
              <Badge key={ref.name} variant="outline">
                {ref.name}
              </Badge>
            ))}
          </div>
        </div>
      )}
      <div>
        <SectionTitle>SHA</SectionTitle>
        <code className="text-muted-foreground font-mono text-xs break-all">
          {commit.id}
        </code>
      </div>
      <div>
        <SectionTitle>Parents</SectionTitle>
        <div className="flex flex-wrap gap-1">
          {commit.parents.map((parent) => (
            <code
              key={parent}
              className="text-muted-foreground font-mono text-xs"
            >
              {shortId(parent)}
            </code>
          ))}
        </div>
      </div>
      <div className="border-border border-t pt-3">
        <h3 className="text-sm font-medium">{commit.summary}</h3>
      </div>
    </div>
  )
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-muted-foreground mb-1 text-[11px] font-semibold tracking-wide uppercase">
      {children}
    </div>
  )
}

function CommitAvatar({ email, name }: { email: string; name: string }) {
  const src = gravatarSrc(email, 96)
  return (
    <Avatar className="size-11 shrink-0">
      {src ? <AvatarImage src={src} alt="" /> : null}
      <AvatarFallback>{name.charAt(0) || "Z"}</AvatarFallback>
    </Avatar>
  )
}
