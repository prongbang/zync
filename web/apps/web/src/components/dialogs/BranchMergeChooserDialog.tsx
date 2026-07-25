// Drag-and-drop branch chooser (P2.4). Opened when a branch row is dropped onto
// the HEAD row in BranchSidebar (see the BranchCommand doc comment there for the
// full direction rationale). `source` is the branch that was dragged, `target`
// is always the current/checked-out branch — git can only merge/rebase into it.
// Each option maps 1:1 onto the same BranchCommand the right-click context menu
// already emits for `source`, so App.tsx's existing onBranchCommand switch (merge
// opens MergeDialog, rebase runs the rebase) handles it without any new plumbing.

import type { ReactElement } from "react"

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

export type BranchMergeChoice = "merge" | "rebase"

export function BranchMergeChooserDialog({
  open,
  onOpenChange,
  source,
  target,
  onChoose,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The branch that was dragged. */
  source: string
  /** The current/HEAD branch it was dropped onto. */
  target: string
  onChoose: (choice: BranchMergeChoice) => void
}): ReactElement {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="branch-merge-chooser">
        <DialogHeader>
          <DialogTitle>Merge or Rebase</DialogTitle>
          <DialogDescription>
            <code className="font-mono">{source}</code> was dropped onto the current
            branch, <code className="font-mono">{target}</code>. Choose how to
            integrate them.
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-2">
          <Button
            variant="outline"
            data-testid="chooser-merge"
            className="h-auto flex-col items-start gap-0.5 whitespace-normal px-3 py-2 text-left"
            // Don't also call onOpenChange(false) here: onChoose drives the
            // caller's dialog state directly (it may replace this dialog with
            // MergeDialog rather than closing outright), so closing here too
            // would race it and clobber whatever it just set.
            onClick={() => onChoose("merge")}
          >
            <span className="font-medium">
              Merge {source} into {target}
            </span>
            <span className="font-normal text-muted-foreground">
              Creates a merge commit on {target} bringing in {source}'s changes.
            </span>
          </Button>
          <Button
            variant="outline"
            data-testid="chooser-rebase"
            className="h-auto flex-col items-start gap-0.5 whitespace-normal px-3 py-2 text-left"
            onClick={() => onChoose("rebase")}
          >
            <span className="font-medium">
              Rebase {target} on {source}
            </span>
            <span className="font-normal text-muted-foreground">
              Replays {target}'s commits onto the tip of {source}.
            </span>
          </Button>
        </div>
        <DialogFooter>
          <DialogClose data-testid="dialog-cancel" render={<Button variant="outline" />}>
            Cancel
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
