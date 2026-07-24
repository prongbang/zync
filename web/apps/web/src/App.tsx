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
import { CommitGraph } from "./components/CommitGraph"
import { ConflictResolver } from "./components/ConflictResolver"
import { DiffPanel } from "./components/DiffPanel"
import { GitToolsPanel } from "./components/GitToolsPanel"
import { RepoMinibar } from "./components/RepoMinibar"
import { RepoStatsPanel } from "./components/RepoStatsPanel"
import { Toolbar } from "./components/Toolbar"
import {
  AddRepositoryDialog,
  DeleteDialog,
  DeleteTagDialog,
  DropDialog,
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
import { WORKDIR_REVISION } from "./lib/api"
import { graphRows, statusLabel, type BlameRow } from "./lib/helpers"
import { formatCommitTime, gravatarSrc, shortId } from "./lib/format"
import type { CreateRepositoryRequest, RepositoryRecord } from "./lib/types"
import { useWorkspace } from "./lib/useWorkspace"

type CenterMode = "changes" | "commits"

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
  | null

export function App() {
  const ws = useWorkspace()
  const [selectedCommit, setSelectedCommit] = useState<string | null>(null)
  const [mode, setMode] = useState<CenterMode>("commits")
  const [message, setMessage] = useState("")
  const [blame, setBlame] = useState<BlameRow[] | null>(null)
  const [dialog, setDialog] = useState<ActiveDialog>(null)
  const [addRepoOpen, setAddRepoOpen] = useState(false)

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

  const rows = useMemo(() => graphRows(ws.commits), [ws.commits])
  const selected =
    ws.commits.find((c) => c.id === selectedCommit) ?? ws.commits[0] ?? null
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
      case "interactiveRebase":
        ws.setNotice("Rebase onto a branch is available from a commit menu")
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
            />
          ) : (
            <Tabs
              defaultValue="commit"
              className="min-h-0 flex-1 gap-0"
              onValueChange={(v) => {
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
                className="min-h-0 flex-1 overflow-y-auto p-4"
              >
                <CommitDetail commit={selected} />
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
                <GitToolsPanel repositoryId={currentRepoId} onRefresh={() => ws.refresh()} />
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
        <div className="flex min-h-0 min-w-0 flex-1 items-center justify-center p-6">
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
        <Button
          variant="ghost"
          size="icon-sm"
          className="ml-auto md:hidden"
          aria-label="Open details"
          onClick={() => setDetailSheetOpen(true)}
        >
          <Info />
        </Button>
      </header>

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
