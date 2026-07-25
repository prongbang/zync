// Fork-style branch navigator. Presentational: the parent owns branch data and
// turns each emitted command into a workspace mutation / dialog (see App.tsx).
// Ported from crates/ui/src/components/sidebar.rs.

import { useState, type DragEvent, type ReactElement } from "react"

import { Badge } from "@workspace/ui/components/badge"
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@workspace/ui/components/context-menu"
import { cn } from "@workspace/ui/lib/utils"

import { branchGroupRows, branchLeafLabel } from "@/lib/helpers"
import type { BranchSummary, TagSummary } from "@/lib/types"

// Emitted intents. `rename` carries `newName` seeded to the current name — the
// orchestrator opens RenameDialog to collect the real value before applying.
//
// P2.4 drag-and-drop semantics: git can only merge/rebase *into* the checked-out
// (HEAD) branch, so the HEAD row is the only valid drop target. Dragging branch
// A onto the HEAD row ("target" below, always the current branch) opens a small
// chooser offering "Merge A into <target>" (kind: "merge", name: A — identical
// to right-clicking A and choosing "Merge into current branch...") and "Rebase
// <target> on A" (kind: "rebase", name: A — identical to right-clicking A and
// choosing "Rebase on 'A'..."). Dropping onto any non-HEAD row is disallowed
// (no drop effect, no highlight) rather than auto-checking-out — that would be
// a surprising side effect for what looks like a simple drag.
export type BranchCommand =
  | { kind: "checkout"; name: string }
  | { kind: "merge"; name: string }
  | { kind: "delete"; name: string }
  | { kind: "rename"; name: string; newName: string }
  | { kind: "newBranch"; name: string }
  | { kind: "newTag"; name: string }
  | { kind: "rebase"; name: string }
  | { kind: "interactiveRebase"; name: string }
  | { kind: "dropMergeChooser"; source: string; target: string }

// Emitted intents for the Tags section. `checkout` reuses the same detached
// checkout-at-revision flow commits use (`runCommitAction("checkout", ...)`
// in App.tsx) since a tag name resolves the same way as a commit id.
export type TagCommand =
  | { kind: "checkout"; name: string }
  | { kind: "push"; name: string }
  | { kind: "copySha"; sha: string }
  | { kind: "delete"; name: string }

function AheadBehind({ branch }: { branch: BranchSummary }): ReactElement | null {
  const ahead = branch.ahead ?? 0
  const behind = branch.behind ?? 0
  if (ahead <= 0 && behind <= 0) return null
  return (
    <span className="flex shrink-0 items-center gap-1">
      {ahead > 0 ? (
        <Badge variant="outline" className="tabular-nums">
          {ahead}↑
        </Badge>
      ) : null}
      {behind > 0 ? (
        <Badge variant="secondary" className="tabular-nums">
          {behind}↓
        </Badge>
      ) : null}
    </span>
  )
}

function BranchRow({
  branch,
  label,
  indent,
  draggedBranch,
  dragOverBranch,
  onCommand,
  onDragStart,
  onDragOverTarget,
  onDragLeaveTarget,
  onDropTarget,
  onDragEnd,
}: {
  branch: BranchSummary
  label: string
  indent: boolean
  draggedBranch: string | null
  dragOverBranch: string | null
  onCommand: (command: BranchCommand) => void
  onDragStart: (name: string) => void
  onDragOverTarget: (name: string) => void
  onDragLeaveTarget: (name: string) => void
  onDropTarget: (source: string, target: string) => void
  onDragEnd: () => void
}): ReactElement {
  const { name, is_head } = branch
  const isDragging = draggedBranch === name
  // Only the HEAD row is ever a legal drop target — see the BranchCommand
  // doc comment above for the full merge/rebase-direction rationale.
  const isValidDropTarget =
    is_head && draggedBranch !== null && draggedBranch !== name
  const isDragOver = isValidDropTarget && dragOverBranch === name

  return (
    // `contents` keeps this wrapper out of the flex layout — it exists only
    // to expose a stable "this row accepts a branch drop" test hook.
    <div
      className="contents"
      data-testid={is_head ? "branch-drop-target" : undefined}
    >
      <ContextMenu>
        <ContextMenuTrigger
          data-testid="branch-row"
          data-branch-name={name}
          draggable
          className={cn(
            "flex h-[26px] w-full cursor-pointer items-center gap-2 rounded px-2 text-[13px] text-foreground/90 outline-none focus-visible:ring-2 focus-visible:ring-ring",
            indent && "pl-5",
            is_head ? "bg-accent" : "hover:bg-accent/40",
            isDragging && "opacity-50",
            isDragOver && "bg-accent ring-2 ring-inset ring-ring",
          )}
          onClick={() => onCommand({ kind: "checkout", name })}
          onDragStart={(event: DragEvent<HTMLDivElement>) => {
            event.dataTransfer.effectAllowed = "move"
            onDragStart(name)
          }}
          onDragOver={(event: DragEvent<HTMLDivElement>) => {
            if (!isValidDropTarget) return
            // Only preventDefault on a legal target — leaving it un-prevented
            // on every other row makes the browser show its native
            // "not-allowed" cursor there and blocks the drop, which is the
            // "clear hint" that a checkout-requiring drop is disallowed.
            event.preventDefault()
            event.dataTransfer.dropEffect = "move"
            onDragOverTarget(name)
          }}
          onDragLeave={() => {
            if (isValidDropTarget) onDragLeaveTarget(name)
          }}
          onDrop={(event: DragEvent<HTMLDivElement>) => {
            if (!isValidDropTarget || draggedBranch === null) return
            event.preventDefault()
            onDropTarget(draggedBranch, name)
          }}
          onDragEnd={onDragEnd}
        >
          <span
            className={cn(
              "min-w-0 flex-1 truncate",
              is_head && "text-primary font-medium",
            )}
          >
            {label}
          </span>
          <AheadBehind branch={branch} />
        </ContextMenuTrigger>
        <ContextMenuContent data-testid="branch-context-menu" className="w-60">
          <ContextMenuGroup>
            <ContextMenuItem
              disabled={is_head}
              onClick={() => onCommand({ kind: "checkout", name })}
            >
              Checkout...
            </ContextMenuItem>
          </ContextMenuGroup>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            <ContextMenuItem
              disabled={is_head}
              onClick={() => onCommand({ kind: "merge", name })}
            >
              Merge into current branch...
            </ContextMenuItem>
            <ContextMenuItem
              disabled={is_head}
              onClick={() => onCommand({ kind: "rebase", name })}
            >
              Rebase on '{name}'...
            </ContextMenuItem>
            <ContextMenuItem
              disabled={is_head}
              onClick={() => onCommand({ kind: "interactiveRebase", name })}
            >
              Interactively Rebase on '{name}'...
            </ContextMenuItem>
          </ContextMenuGroup>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            <ContextMenuItem onClick={() => onCommand({ kind: "newBranch", name })}>
              New Branch...
            </ContextMenuItem>
            <ContextMenuItem onClick={() => onCommand({ kind: "newTag", name })}>
              New Tag...
            </ContextMenuItem>
          </ContextMenuGroup>
          <ContextMenuSeparator />
          <ContextMenuGroup>
            <ContextMenuItem
              disabled={is_head}
              onClick={() => onCommand({ kind: "rename", name, newName: name })}
            >
              Rename...
            </ContextMenuItem>
            <ContextMenuItem
              variant="destructive"
              disabled={is_head}
              onClick={() => onCommand({ kind: "delete", name })}
            >
              Delete...
            </ContextMenuItem>
          </ContextMenuGroup>
        </ContextMenuContent>
      </ContextMenu>
    </div>
  )
}

function BranchSection({
  title,
  rows,
  draggedBranch,
  dragOverBranch,
  onCommand,
  onDragStart,
  onDragOverTarget,
  onDragLeaveTarget,
  onDropTarget,
  onDragEnd,
}: {
  title: string
  rows: BranchSummary[]
  draggedBranch: string | null
  dragOverBranch: string | null
  onCommand: (command: BranchCommand) => void
  onDragStart: (name: string) => void
  onDragOverTarget: (name: string) => void
  onDragLeaveTarget: (name: string) => void
  onDropTarget: (source: string, target: string) => void
  onDragEnd: () => void
}): ReactElement {
  const grouped = branchGroupRows(rows)
  return (
    <section className="flex flex-col gap-0.5">
      <div className="px-2 py-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
        {title}
      </div>
      {rows.length === 0 ? (
        <div className="px-2 py-1 text-[12px] text-muted-foreground">
          No branches
        </div>
      ) : (
        grouped.map(([group, branches]) => (
          <div key={group || "__root__"} className="flex flex-col gap-0.5">
            {group ? (
              <div className="truncate px-2 py-1 text-[12px] font-medium text-muted-foreground">
                {group}
              </div>
            ) : null}
            {branches.map((branch) => (
              <BranchRow
                key={branch.name}
                branch={branch}
                label={branchLeafLabel(branch, group)}
                indent={group !== ""}
                draggedBranch={draggedBranch}
                dragOverBranch={dragOverBranch}
                onCommand={onCommand}
                onDragStart={onDragStart}
                onDragOverTarget={onDragOverTarget}
                onDragLeaveTarget={onDragLeaveTarget}
                onDropTarget={onDropTarget}
                onDragEnd={onDragEnd}
              />
            ))}
          </div>
        ))
      )}
    </section>
  )
}

function TagRow({
  tag,
  onCommand,
}: {
  tag: TagSummary
  onCommand: (command: TagCommand) => void
}): ReactElement {
  const { name, target, annotated } = tag
  return (
    <ContextMenu>
      <ContextMenuTrigger
        data-testid="tag-row"
        data-tag-name={name}
        className="flex h-[26px] w-full cursor-pointer items-center gap-2 rounded px-2 text-[13px] text-foreground/90 outline-none hover:bg-accent/40 focus-visible:ring-2 focus-visible:ring-ring"
        onClick={() => onCommand({ kind: "checkout", name })}
      >
        <span className="min-w-0 flex-1 truncate">{name}</span>
        {annotated && (
          <Badge variant="secondary" className="shrink-0">
            annotated
          </Badge>
        )}
      </ContextMenuTrigger>
      <ContextMenuContent data-testid="tag-context-menu" className="w-60">
        <ContextMenuGroup>
          <ContextMenuItem onClick={() => onCommand({ kind: "checkout", name })}>
            Checkout (detached)...
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem onClick={() => onCommand({ kind: "push", name })}>
            Push to origin
          </ContextMenuItem>
          <ContextMenuItem
            disabled={!target}
            onClick={() => target && onCommand({ kind: "copySha", sha: target })}
          >
            Copy SHA
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem
            variant="destructive"
            onClick={() => onCommand({ kind: "delete", name })}
          >
            Delete...
          </ContextMenuItem>
        </ContextMenuGroup>
      </ContextMenuContent>
    </ContextMenu>
  )
}

function TagSection({
  tags,
  onCommand,
}: {
  tags: TagSummary[]
  onCommand: (command: TagCommand) => void
}): ReactElement {
  return (
    <section className="flex flex-col gap-0.5">
      <div className="px-2 py-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
        Tags
      </div>
      {tags.length === 0 ? (
        <div className="px-2 py-1 text-[12px] text-muted-foreground">
          No tags
        </div>
      ) : (
        tags.map((tag) => (
          <TagRow key={tag.name} tag={tag} onCommand={onCommand} />
        ))
      )}
    </section>
  )
}

export function BranchSidebar({
  branches,
  tags,
  onCommand,
  onTagCommand,
}: {
  branches: BranchSummary[]
  tags: TagSummary[]
  onCommand: (command: BranchCommand) => void
  onTagCommand: (command: TagCommand) => void
}): ReactElement {
  const locals = branches.filter((branch) => branch.kind === "local")
  const remotes = branches.filter((branch) => branch.kind !== "local")

  // Drag state is lifted here (not into BranchSection) because a drag can
  // start in the Branches section and land in Remotes or vice versa.
  const [draggedBranch, setDraggedBranch] = useState<string | null>(null)
  const [dragOverBranch, setDragOverBranch] = useState<string | null>(null)

  function resetDrag() {
    setDraggedBranch(null)
    setDragOverBranch(null)
  }

  function handleDropTarget(source: string, target: string) {
    onCommand({ kind: "dropMergeChooser", source, target })
    resetDrag()
  }

  return (
    <nav
      className="flex h-full min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-2"
      aria-label="Branches and tags"
    >
      <BranchSection
        title="Branches"
        rows={locals}
        draggedBranch={draggedBranch}
        dragOverBranch={dragOverBranch}
        onCommand={onCommand}
        onDragStart={setDraggedBranch}
        onDragOverTarget={setDragOverBranch}
        onDragLeaveTarget={() => setDragOverBranch(null)}
        onDropTarget={handleDropTarget}
        onDragEnd={resetDrag}
      />
      <BranchSection
        title="Remotes"
        rows={remotes}
        draggedBranch={draggedBranch}
        dragOverBranch={dragOverBranch}
        onCommand={onCommand}
        onDragStart={setDraggedBranch}
        onDragOverTarget={setDragOverBranch}
        onDragLeaveTarget={() => setDragOverBranch(null)}
        onDropTarget={handleDropTarget}
        onDragEnd={resetDrag}
      />
      <TagSection tags={tags} onCommand={onTagCommand} />
    </nav>
  )
}
