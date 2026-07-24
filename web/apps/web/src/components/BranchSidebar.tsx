// Fork-style branch navigator. Presentational: the parent owns branch data and
// turns each emitted command into a workspace mutation / dialog (see App.tsx).
// Ported from crates/ui/src/components/sidebar.rs.

import type { ReactElement } from "react"

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
import type { BranchSummary } from "@/lib/types"

// Emitted intents. `rename` carries `newName` seeded to the current name — the
// orchestrator opens RenameDialog to collect the real value before applying.
export type BranchCommand =
  | { kind: "checkout"; name: string }
  | { kind: "merge"; name: string }
  | { kind: "delete"; name: string }
  | { kind: "rename"; name: string; newName: string }
  | { kind: "newBranch"; name: string }
  | { kind: "newTag"; name: string }
  | { kind: "rebase"; name: string }
  | { kind: "interactiveRebase"; name: string }

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
  onCommand,
}: {
  branch: BranchSummary
  label: string
  indent: boolean
  onCommand: (command: BranchCommand) => void
}): ReactElement {
  const { name, is_head } = branch
  return (
    <ContextMenu>
      <ContextMenuTrigger
        data-testid="branch-row"
        data-branch-name={name}
        className={cn(
          "flex h-[26px] w-full cursor-pointer items-center gap-2 rounded px-2 text-[13px] text-foreground/90 outline-none focus-visible:ring-2 focus-visible:ring-ring",
          indent && "pl-5",
          is_head ? "bg-accent" : "hover:bg-accent/40",
        )}
        onClick={() => onCommand({ kind: "checkout", name })}
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
      <ContextMenuContent className="w-60">
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
  )
}

function BranchSection({
  title,
  rows,
  onCommand,
}: {
  title: string
  rows: BranchSummary[]
  onCommand: (command: BranchCommand) => void
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
                onCommand={onCommand}
              />
            ))}
          </div>
        ))
      )}
    </section>
  )
}

export function BranchSidebar({
  branches,
  onCommand,
}: {
  branches: BranchSummary[]
  onCommand: (command: BranchCommand) => void
}): ReactElement {
  const locals = branches.filter((branch) => branch.kind === "local")
  const remotes = branches.filter((branch) => branch.kind !== "local")

  return (
    <nav
      className="flex h-full min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-2"
      aria-label="Branches"
    >
      <BranchSection title="Branches" rows={locals} onCommand={onCommand} />
      <BranchSection title="Remotes" rows={remotes} onCommand={onCommand} />
    </nav>
  )
}
