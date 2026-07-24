// Per-file History view (P1.2). Reached with a single click from DiffPanel's
// "History" button (or a local-changes row) — a master/detail Sheet: commits
// that touched the path on the left (api.fileHistory), the selected commit's
// diff for just that file on the right (the commit's full patch filtered
// client-side via patchForPath, reusing splitPatchByFile — same approach
// DiffPanel already uses for its multi-file diff tree). "Open file at
// revision" shows the full blob content (text or image) in a nested Dialog.

import { useEffect, useState } from "react"
import type { ReactElement } from "react"

import { Button } from "@workspace/ui/components/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@workspace/ui/components/dialog"
import { ScrollArea } from "@workspace/ui/components/scroll-area"
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@workspace/ui/components/sheet"
import { Spinner } from "@workspace/ui/components/spinner"
import { cn } from "@workspace/ui/lib/utils"

import { InlineDiffView } from "./DiffPanel"
import { formatCommitTime, shortId } from "@/lib/format"
import { isImagePath, patchForPath, pathBasename } from "@/lib/helpers"
import type { CommitSummary } from "@/lib/types"

type RevisionView = {
  commitId: string
  content: string | null
  isImage: boolean
  loading: boolean
  error: string | null
}

export function FileHistorySheet({
  open,
  onOpenChange,
  path,
  repositoryId,
  fileHistory,
  diffCommit,
  blobText,
  blobUrl,
  onJumpToCommit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  path: string
  repositoryId: string | null
  fileHistory: (path: string) => Promise<CommitSummary[]>
  diffCommit: (repositoryId: string, commitId: string) => Promise<string>
  blobText: (
    repositoryId: string,
    revision: string,
    path: string,
  ) => Promise<string>
  blobUrl: (repositoryId: string, revision: string, path: string) => string
  /** Jumps to a commit in the graph (App owns selectedCommit) — offered next
   * to "Open file at revision" so a history entry is also a way in, matching
   * the blame gutter's jump-to-commit affordance. */
  onJumpToCommit: (commitId: string) => void
}): ReactElement {
  const [entries, setEntries] = useState<CommitSummary[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selectedDiff, setSelectedDiff] = useState<string | null>(null)
  const [diffLoading, setDiffLoading] = useState(false)
  const [revisionView, setRevisionView] = useState<RevisionView | null>(null)

  useEffect(() => {
    if (!open || !repositoryId || path === "") return
    setEntries(null)
    setLoadError(null)
    setSelectedId(null)
    setSelectedDiff(null)
    setLoading(true)
    fileHistory(path)
      .then((results) => {
        setEntries(results)
        if (results.length > 0) setSelectedId(results[0].id)
      })
      .catch((error) =>
        setLoadError(error instanceof Error ? error.message : String(error)),
      )
      .finally(() => setLoading(false))
  }, [open, repositoryId, path, fileHistory])

  useEffect(() => {
    if (!open || !repositoryId || !selectedId) {
      setSelectedDiff(null)
      return
    }
    let cancelled = false
    setDiffLoading(true)
    setSelectedDiff(null)
    diffCommit(repositoryId, selectedId)
      .then((patch) => {
        if (!cancelled) setSelectedDiff(patchForPath(patch, path))
      })
      .catch((error) => {
        if (!cancelled) {
          const message = error instanceof Error ? error.message : String(error)
          setSelectedDiff(`Failed to load diff: ${message}`)
        }
      })
      .finally(() => {
        if (!cancelled) setDiffLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [open, repositoryId, selectedId, path, diffCommit])

  function openAtRevision(commitId: string) {
    if (!repositoryId) return
    if (isImagePath(path)) {
      setRevisionView({
        commitId,
        content: null,
        isImage: true,
        loading: false,
        error: null,
      })
      return
    }
    setRevisionView({
      commitId,
      content: null,
      isImage: false,
      loading: true,
      error: null,
    })
    blobText(repositoryId, commitId, path)
      .then((content) =>
        setRevisionView({
          commitId,
          content,
          isImage: false,
          loading: false,
          error: null,
        }),
      )
      .catch((error) =>
        setRevisionView({
          commitId,
          content: null,
          isImage: false,
          loading: false,
          error: error instanceof Error ? error.message : String(error),
        }),
      )
  }

  return (
    <>
      <Sheet open={open} onOpenChange={onOpenChange}>
        <SheetContent
          side="right"
          className="w-full gap-0 data-[side=right]:sm:max-w-3xl"
          data-testid="file-history-view"
        >
          <SheetHeader className="border-border border-b">
            <SheetTitle className="truncate font-mono text-sm">
              History — {pathBasename(path)}
            </SheetTitle>
          </SheetHeader>
          <div className="flex min-h-0 flex-1">
            <nav
              aria-label="File history"
              className="border-border flex w-72 shrink-0 flex-col border-r"
            >
              <ScrollArea className="min-h-0 flex-1">
                {loading ? (
                  <div className="text-muted-foreground flex items-center gap-2 p-4 text-sm">
                    <Spinner className="size-4" /> Loading history…
                  </div>
                ) : loadError ? (
                  <div className="text-destructive p-4 text-sm">{loadError}</div>
                ) : entries && entries.length === 0 ? (
                  <div className="text-muted-foreground p-4 text-sm">
                    No history for this file.
                  </div>
                ) : (
                  <ul className="flex flex-col">
                    {(entries ?? []).map((entry) => (
                      <li key={entry.id}>
                        <button
                          type="button"
                          data-testid="file-history-row"
                          onClick={() => setSelectedId(entry.id)}
                          aria-current={entry.id === selectedId ? "true" : undefined}
                          className={cn(
                            "border-border/50 flex w-full flex-col gap-0.5 border-b px-3 py-2 text-left text-sm",
                            entry.id === selectedId
                              ? "bg-accent"
                              : "hover:bg-accent/50",
                          )}
                        >
                          <span className="truncate">{entry.summary}</span>
                          <span className="text-muted-foreground flex items-center gap-1.5 text-[11px]">
                            <code>{shortId(entry.id)}</code>
                            <span aria-hidden="true">·</span>
                            <span className="truncate">{entry.author}</span>
                            <span aria-hidden="true">·</span>
                            <span className="shrink-0">
                              {formatCommitTime(entry.time)}
                            </span>
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </ScrollArea>
            </nav>
            <div className="flex min-h-0 flex-1 flex-col">
              <header className="border-border flex shrink-0 items-center justify-between gap-2 border-b px-2.5 py-1.5">
                <code className="text-muted-foreground min-w-0 flex-1 truncate text-[11px]">
                  {selectedId ? `${path} @ ${shortId(selectedId)}` : path}
                </code>
                <div className="flex shrink-0 items-center gap-1.5">
                  <Button
                    type="button"
                    variant="ghost"
                    size="xs"
                    disabled={!selectedId}
                    onClick={() => {
                      if (!selectedId) return
                      onJumpToCommit(selectedId)
                      onOpenChange(false)
                    }}
                  >
                    View commit
                  </Button>
                  <Button
                    type="button"
                    data-testid="open-file-at-revision"
                    variant="outline"
                    size="xs"
                    disabled={!selectedId}
                    onClick={() => selectedId && openAtRevision(selectedId)}
                  >
                    Open file at revision
                  </Button>
                </div>
              </header>
              <div className="min-h-0 flex-1 overflow-auto font-mono text-[12px] leading-5">
                {diffLoading ? (
                  <div className="text-muted-foreground flex items-center gap-2 p-4">
                    <Spinner className="size-4" /> Loading diff…
                  </div>
                ) : selectedDiff === null ? (
                  <div className="text-muted-foreground p-4">
                    Select a commit to view its diff.
                  </div>
                ) : selectedDiff === "" ? (
                  <div className="text-muted-foreground p-4">
                    No changes to this file in this commit.
                  </div>
                ) : (
                  <InlineDiffView diff={selectedDiff} />
                )}
              </div>
            </div>
          </div>
        </SheetContent>
      </Sheet>

      <Dialog
        open={revisionView !== null}
        onOpenChange={(o) => !o && setRevisionView(null)}
      >
        <DialogContent
          data-testid="file-revision-dialog"
          className="sm:max-w-3xl"
        >
          <DialogHeader>
            <DialogTitle className="truncate font-mono text-sm">
              {path} @ {revisionView ? shortId(revisionView.commitId) : ""}
            </DialogTitle>
          </DialogHeader>
          {revisionView?.isImage && repositoryId ? (
            <div className="bg-muted flex max-h-[70vh] items-center justify-center overflow-auto p-4">
              <img
                src={blobUrl(repositoryId, revisionView.commitId, path)}
                alt={path}
                className="max-h-full max-w-full object-contain"
              />
            </div>
          ) : revisionView?.loading ? (
            <div className="text-muted-foreground flex items-center gap-2 p-4 text-sm">
              <Spinner className="size-4" /> Loading file…
            </div>
          ) : revisionView?.error ? (
            <div className="text-destructive p-4 text-sm">{revisionView.error}</div>
          ) : (
            <pre className="bg-muted max-h-[70vh] overflow-auto rounded-md p-3 font-mono text-[12px] leading-5 whitespace-pre-wrap break-words">
              {revisionView?.content}
            </pre>
          )}
        </DialogContent>
      </Dialog>
    </>
  )
}
