// Mainline-parent picker for reverting a merge commit (P1.7). libgit2 needs a 1-based mainline
// parent number to revert a merge commit unambiguously — plain, single-parent commits never hit
// this dialog (see App.tsx's commit-menu "revert" wiring). Same FieldSet + RadioGroup shape as
// ResetDialog/MergeDialog.

import { useEffect, useState, type ReactElement } from "react"

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
import {
  Field,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@workspace/ui/components/field"
import { RadioGroup, RadioGroupItem } from "@workspace/ui/components/radio-group"

import { shortId } from "../../lib/format"

export type RevertParentPayload = { mainline: number }

export function RevertParentDialog({
  open,
  onOpenChange,
  commit,
  parents,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Short id of the merge commit being reverted. */
  commit: string
  /** Full parent commit ids, in the merge's own order (index 0 = mainline 1). */
  parents: string[]
  onSubmit: (payload: RevertParentPayload) => void
}): ReactElement {
  const [mainline, setMainline] = useState(1)

  useEffect(() => {
    if (open) setMainline(1)
  }, [open])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="revert-parent-dialog">
        <DialogHeader>
          <DialogTitle>Revert Merge Commit</DialogTitle>
          <DialogDescription>
            This commit has multiple parents. Choose which side to keep as the mainline —
            reverting undoes the changes introduced relative to it.
          </DialogDescription>
        </DialogHeader>
        <code className="truncate rounded bg-muted px-2 py-1 font-mono text-[12px] text-muted-foreground">
          {commit}
        </code>
        <FieldSet>
          <FieldLegend variant="label">Mainline parent</FieldLegend>
          <RadioGroup
            value={String(mainline)}
            onValueChange={(value) => setMainline(Number(value))}
          >
            <FieldGroup className="gap-3">
              {parents.map((parentId, index) => (
                <Field orientation="horizontal" key={parentId}>
                  <RadioGroupItem
                    value={String(index + 1)}
                    id={`revert-mainline-${index + 1}`}
                  />
                  <FieldLabel
                    htmlFor={`revert-mainline-${index + 1}`}
                    className="font-normal"
                  >
                    Parent {index + 1} — <code className="font-mono">{shortId(parentId)}</code>
                  </FieldLabel>
                </Field>
              ))}
            </FieldGroup>
          </RadioGroup>
        </FieldSet>
        <DialogFooter>
          <DialogClose data-testid="dialog-cancel" render={<Button variant="outline" />}>
            Cancel
          </DialogClose>
          <Button
            data-testid="dialog-submit"
            onClick={() => {
              onSubmit({ mainline })
              onOpenChange(false)
            }}
          >
            Revert
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
