import { useEffect, useMemo, useState } from "react"

import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@workspace/ui/components/avatar"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Input } from "@workspace/ui/components/input"
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@workspace/ui/components/tabs"
import { cn } from "@workspace/ui/lib/utils"

import { BranchSidebar, type BranchCommand } from "./components/BranchSidebar"
import { CommitGraph } from "./components/CommitGraph"
import { ConflictResolver } from "./components/ConflictResolver"
import { DiffPanel } from "./components/DiffPanel"
import { GitToolsPanel } from "./components/GitToolsPanel"
import { RepoStatsPanel } from "./components/RepoStatsPanel"
import { Toolbar } from "./components/Toolbar"
import {
  DeleteDialog,
  DropDialog,
  MergeDialog,
  NewBranchDialog,
  RenameDialog,
  ResetDialog,
  RewordDialog,
  StashApplyDialog,
  TagDialog,
} from "./components/dialogs"
import { graphRows, statusLabel, type BlameRow } from "./lib/helpers"
import { formatCommitTime, gravatarSrc, shortId } from "./lib/format"
import { useWorkspace } from "./lib/useWorkspace"

type CenterMode = "changes" | "commits"

// The dialog currently open, carrying the data it needs.
type ActiveDialog =
  | { kind: "newBranch"; at: string }
  | { kind: "tag"; target: string }
  | { kind: "rename"; name: string }
  | { kind: "delete"; name: string }
  | { kind: "merge"; name: string }
  | { kind: "reword"; commitId: string; message: string }
  | { kind: "reset"; commitId: string }
  | { kind: "drop"; commitId: string }
  | { kind: "stashApply"; index: number }
  | null

export function App() {
  const ws = useWorkspace()
  const [selectedCommit, setSelectedCommit] = useState<string | null>(null)
  const [mode, setMode] = useState<CenterMode>("commits")
  const [message, setMessage] = useState("")
  const [blame, setBlame] = useState<BlameRow[] | null>(null)
  const [dialog, setDialog] = useState<ActiveDialog>(null)

  const currentRepoId = ws.workspace?.repository.id ?? null

  useEffect(() => {
    if (!ws.workspace && ws.repositories.length > 0) {
      void ws.openRepository(ws.repositories[0].id)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ws.repositories, ws.workspace])

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

  return (
    <div className="bg-background text-foreground flex h-svh flex-col">
      <header className="border-border flex h-12 shrink-0 items-center gap-2 border-b px-3">
        <span className="bg-primary size-2 rounded-full" />
        <span className="text-sm font-semibold">Zync</span>
        <Toolbar
          disabled={!currentRepoId}
          onAction={(kind) => {
            if (kind === "stash") void ws.createStash("WIP from Zync")
            else void ws.remoteAction(kind)
          }}
        />
        {ws.repositories.length > 1 && (
          <div className="ml-auto flex items-stretch overflow-x-auto">
            {ws.repositories.map((repo) => (
              <button
                key={repo.id}
                data-testid="repo-tab"
                data-repo-id={repo.id}
                onClick={() => void ws.openRepository(repo.id)}
                className={cn(
                  "border-border shrink-0 border-l border-b-2 border-b-transparent px-3 text-xs font-medium whitespace-nowrap",
                  repo.id === currentRepoId
                    ? "border-b-primary text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {repo.name}
              </button>
            ))}
          </div>
        )}
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[260px_minmax(0,1fr)_380px]">
        <aside className="border-border min-h-0 overflow-y-auto border-r">
          <BranchSidebar branches={ws.branches} onCommand={onBranchCommand} />
        </aside>

        <main className="flex min-h-0 flex-col">
          <div className="border-border flex h-9 shrink-0 items-center gap-3 border-b px-3">
            <button
              data-testid="changes-tab"
              onClick={() => setMode("changes")}
              className={cn(
                "text-xs font-semibold",
                mode === "changes"
                  ? "text-foreground border-primary border-b-2"
                  : "text-muted-foreground",
              )}
            >
              Local Changes ({ws.gitStatus.length})
            </button>
            <button
              data-testid="commits-tab"
              onClick={() => setMode("commits")}
              className={cn(
                "text-xs font-semibold",
                mode === "commits"
                  ? "text-foreground border-primary border-b-2"
                  : "text-muted-foreground",
              )}
            >
              All Commits
            </button>
          </div>

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
                      onClick={() => void ws.selectFileDiff(file.path)}
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
                      <button
                        data-testid="stage-btn"
                        className="text-muted-foreground hover:text-foreground shrink-0 text-xs"
                        onClick={() => void ws.stageFiles([file.path])}
                      >
                        Stage
                      </button>
                    )}
                    {file.staged && (
                      <button
                        data-testid="unstage-btn"
                        className="text-muted-foreground hover:text-foreground shrink-0 text-xs"
                        onClick={() => void ws.unstageFiles([file.path])}
                      >
                        Unstage
                      </button>
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
              onSelect={setSelectedCommit}
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
                  case "checkout":
                  case "cherry-pick":
                  case "revert":
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

        <aside className="border-border flex min-h-0 flex-col overflow-hidden border-l">
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
                <GitToolsPanel onRefresh={() => ws.refresh()} />
              </TabsContent>
            </Tabs>
          )}
        </aside>
      </div>

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
      {dialog?.kind === "merge" && (
        <MergeDialog
          open
          branch={dialog.name}
          onOpenChange={(o) => !o && setDialog(null)}
          onSubmit={() => {
            void ws.mergeBranch(dialog.name)
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
