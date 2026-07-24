// Interactive rebase todo editor (P1.6). Given a target commit (the oldest
// commit in the plan — the one the context menu's "Interactive rebase..."
// action was opened on), builds the same base/range `rebaseRangeForTarget`
// computes for the single-commit quick actions (reword/edit/squash/fixup/drop
// in CommitGraph's context menu), then lets the user reorder every row and
// pick a per-row action before executing the whole plan in one rebase call.
//
// Row order matches git's own rebase todo file: oldest first, HEAD last.
// "Reword" is not a server-side RebaseAction — interactive_rebase's Pick step
// already accepts an optional message override (crates/git-core/src/lib.rs
// `replay_commit`'s `ReplayMode::Pick(Option<String>)`), so a reworded row is
// sent on the wire as `{ action: "pick", message }`, not a "reword" variant.

import {
  useEffect,
  useMemo,
  useState,
  type DragEvent,
  type ReactElement,
} from "react"

import { ChevronDown, ChevronUp, GripVertical } from "lucide-react"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@workspace/ui/components/alert"
import { Button } from "@workspace/ui/components/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@workspace/ui/components/dialog"
import { Input } from "@workspace/ui/components/input"
import { ScrollArea } from "@workspace/ui/components/scroll-area"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@workspace/ui/components/select"
import { cn } from "@workspace/ui/lib/utils"

import { shortId } from "@/lib/format"
import { rebaseRangeForTarget } from "@/lib/helpers"
import type { CommitSummary, FileStatus, RebaseStepRequest } from "@/lib/types"

export type RebaseRowAction =
  | "pick"
  | "reword"
  | "edit"
  | "squash"
  | "fixup"
  | "drop"

type Row = {
  id: string
  summary: string
  action: RebaseRowAction
  /** New commit message, only read when `action === "reword"`. */
  message: string
}

const ACTIONS: { value: RebaseRowAction; label: string }[] = [
  { value: "pick", label: "Pick" },
  { value: "reword", label: "Reword" },
  { value: "edit", label: "Edit" },
  { value: "squash", label: "Squash" },
  { value: "fixup", label: "Fixup" },
  { value: "drop", label: "Drop" },
]

const ACTION_LABEL: Record<RebaseRowAction, string> = ACTIONS.reduce(
  (acc, a) => ({ ...acc, [a.value]: a.label }),
  {} as Record<RebaseRowAction, string>,
)

export function InteractiveRebaseDialog({
  open,
  onOpenChange,
  commits,
  targetId,
  gitStatus,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Full loaded graph, newest-first — same list the quick single-commit
   * rebase actions (`quickRebasePlan`) read. */
  commits: CommitSummary[]
  /** Commit the context menu was opened on; becomes the oldest row. */
  targetId: string | null
  gitStatus: FileStatus[]
  onSubmit: (payload: { base: string; steps: RebaseStepRequest[] }) => void
}): ReactElement {
  const [rows, setRows] = useState<Row[]>([])
  const [base, setBase] = useState<string | null>(null)
  const [rangeError, setRangeError] = useState<string | null>(null)
  const [dragIndex, setDragIndex] = useState<number | null>(null)
  const [overIndex, setOverIndex] = useState<number | null>(null)

  // Seed the row list only when the dialog opens (or switches target) — not
  // on every background commits refresh while it's open, or an in-progress
  // reorder/reword edit would get silently clobbered by a live-sync update.
  useEffect(() => {
    if (!open || !targetId) return
    try {
      const range = rebaseRangeForTarget(commits, targetId)
      setBase(range.base)
      setRangeError(null)
      setRows(
        range.ids.map((id) => {
          const commit = commits.find((c) => c.id === id)
          return {
            id,
            summary: commit?.summary ?? id,
            action: "pick" as RebaseRowAction,
            message: commit?.summary ?? "",
          }
        }),
      )
    } catch (error) {
      setBase(null)
      setRangeError(error instanceof Error ? error.message : String(error))
      setRows([])
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, targetId])

  const hasLocalChanges = gitStatus.some(
    (f) => f.staged || f.unstaged || f.untracked || f.conflicted,
  )
  // Drop is a server-side no-op that never advances HEAD, so it's not enough
  // to check the literal first row: [drop, squash] would still leave the
  // squash as the first thing that actually runs, amending HEAD while it's
  // still sitting at `base` — squashing into a commit outside this plan.
  // Check the first row that isn't dropped instead.
  const firstKept = rows.find((row) => row.action !== "drop")
  const firstActionInvalid =
    firstKept !== undefined &&
    (firstKept.action === "squash" || firstKept.action === "fixup")

  const summary = useMemo(() => {
    const counts: Record<RebaseRowAction, number> = {
      pick: 0,
      reword: 0,
      edit: 0,
      squash: 0,
      fixup: 0,
      drop: 0,
    }
    for (const row of rows) counts[row.action] += 1
    return ACTIONS.filter((a) => counts[a.value] > 0)
      .map((a) => `${counts[a.value]} ${a.label.toLowerCase()}`)
      .join(", ")
  }, [rows])

  const stoppingAt = rows.find((row) => row.action === "edit")

  const canExecute =
    rows.length > 0 &&
    rangeError === null &&
    base !== null &&
    !hasLocalChanges &&
    !firstActionInvalid

  const setAction = (index: number, action: RebaseRowAction) => {
    setRows((prev) =>
      prev.map((row, i) => (i === index ? { ...row, action } : row)),
    )
  }

  const setMessage = (index: number, message: string) => {
    setRows((prev) =>
      prev.map((row, i) => (i === index ? { ...row, message } : row)),
    )
  }

  const reorder = (from: number, to: number) => {
    if (from === to) return
    setRows((prev) => {
      if (from < 0 || to < 0 || from >= prev.length || to >= prev.length)
        return prev
      const next = [...prev]
      const [moved] = next.splice(from, 1)
      next.splice(to, 0, moved)
      return next
    })
  }

  const submit = () => {
    if (!canExecute || base === null) return
    const steps: RebaseStepRequest[] = rows.map((row) =>
      row.action === "reword"
        ? {
            commit: row.id,
            action: "pick",
            message: row.message.trim() || undefined,
          }
        : { commit: row.id, action: row.action },
    )
    onSubmit({ base, steps })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        data-testid="interactive-rebase-dialog"
        className="sm:max-w-lg"
      >
        <DialogHeader>
          <DialogTitle>Interactive Rebase</DialogTitle>
          <DialogDescription>
            Reorder commits and choose an action for each, then run the whole
            plan in one rebase.
          </DialogDescription>
        </DialogHeader>

        {rangeError !== null && (
          <Alert variant="destructive">
            <AlertTitle>Can&rsquo;t build a rebase plan</AlertTitle>
            <AlertDescription>{rangeError}</AlertDescription>
          </Alert>
        )}
        {hasLocalChanges && (
          <Alert variant="destructive" data-testid="rebase-dirty-warning">
            <AlertTitle>Working tree not clean</AlertTitle>
            <AlertDescription>
              Commit or stash your changes first.
            </AlertDescription>
          </Alert>
        )}
        {rangeError === null && firstActionInvalid && (
          <Alert variant="destructive">
            <AlertTitle>Nothing to combine with</AlertTitle>
            <AlertDescription>
              The oldest row can&rsquo;t squash or fixup — there&rsquo;s no
              earlier commit in this plan to combine it into.
            </AlertDescription>
          </Alert>
        )}

        {rangeError === null && rows.length > 0 && (
          <>
            <ScrollArea className="border-border h-72 rounded-md border">
              <div className="flex flex-col gap-1 p-2">
                {rows.map((row, index) => (
                  <div key={row.id} className="flex flex-col gap-1">
                    <div
                      draggable
                      data-testid="rebase-row"
                      onDragStart={() => setDragIndex(index)}
                      onDragOver={(event: DragEvent<HTMLDivElement>) => {
                        event.preventDefault()
                        if (overIndex !== index) setOverIndex(index)
                      }}
                      onDrop={(event: DragEvent<HTMLDivElement>) => {
                        event.preventDefault()
                        if (dragIndex !== null) reorder(dragIndex, index)
                        setDragIndex(null)
                        setOverIndex(null)
                      }}
                      onDragEnd={() => {
                        setDragIndex(null)
                        setOverIndex(null)
                      }}
                      className={cn(
                        "flex items-center gap-2 rounded-md border border-transparent px-2 py-1.5",
                        overIndex === index &&
                          dragIndex !== index &&
                          "bg-accent",
                        dragIndex === index && "opacity-50",
                        row.action === "drop" &&
                          "border-destructive/30 bg-destructive/5",
                      )}
                    >
                      <GripVertical
                        className="text-muted-foreground size-4 shrink-0 cursor-grab"
                        aria-hidden
                      />
                      <div className="flex shrink-0 flex-col">
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon-xs"
                          aria-label="Move up"
                          disabled={index === 0}
                          onClick={() => reorder(index, index - 1)}
                        >
                          <ChevronUp />
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon-xs"
                          aria-label="Move down"
                          disabled={index === rows.length - 1}
                          onClick={() => reorder(index, index + 1)}
                        >
                          <ChevronDown />
                        </Button>
                      </div>
                      <Select
                        value={row.action}
                        onValueChange={(value) =>
                          setAction(index, value as RebaseRowAction)
                        }
                      >
                        <SelectTrigger
                          size="sm"
                          className="w-28 shrink-0"
                          data-testid="rebase-action-select"
                        >
                          <SelectValue>
                            {(value: RebaseRowAction) =>
                              ACTION_LABEL[value] ?? value
                            }
                          </SelectValue>
                        </SelectTrigger>
                        <SelectContent>
                          <SelectGroup>
                            {ACTIONS.map((a) => (
                              <SelectItem key={a.value} value={a.value}>
                                {a.label}
                              </SelectItem>
                            ))}
                          </SelectGroup>
                        </SelectContent>
                      </Select>
                      <code className="text-muted-foreground shrink-0 font-mono text-[11px]">
                        {shortId(row.id)}
                      </code>
                      <span
                        className={cn(
                          "min-w-0 flex-1 truncate text-sm",
                          row.action === "drop" &&
                            "text-muted-foreground line-through",
                        )}
                      >
                        {row.summary}
                      </span>
                    </div>
                    {row.action === "reword" && (
                      <Input
                        aria-label={`New message for ${shortId(row.id)}`}
                        value={row.message}
                        onChange={(event) =>
                          setMessage(index, event.target.value)
                        }
                        placeholder="New commit message"
                        className="ml-8"
                      />
                    )}
                  </div>
                ))}
              </div>
            </ScrollArea>

            <p className="text-muted-foreground text-sm">
              Base {shortId(base ?? "")} — {rows.length} commit
              {rows.length === 1 ? "" : "s"}
              {summary ? ` (${summary})` : ""}.
              {stoppingAt &&
                ` Rebase will pause for editing at ${shortId(stoppingAt.id)} — anything after it needs Continue Rebase.`}
            </p>
          </>
        )}

        <DialogFooter className="mt-2">
          <DialogClose
            data-testid="dialog-cancel"
            render={<Button variant="outline" type="button" />}
          >
            Cancel
          </DialogClose>
          <Button
            data-testid="rebase-execute"
            disabled={!canExecute}
            onClick={submit}
          >
            Execute
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
