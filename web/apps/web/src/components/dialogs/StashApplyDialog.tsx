// Apply-stash dialog. Ported from StashApplyDialog in
// crates/ui/src/components/dialogs.rs.

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
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@workspace/ui/components/field"
import { Checkbox } from "@workspace/ui/components/checkbox"

import { stashLabel } from "@/lib/helpers"
import type { StashSummary } from "@/lib/types"

export type StashApplyPayload = { dropAfterApply: boolean }

export function StashApplyDialog({
  open,
  onOpenChange,
  stash,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  stash: StashSummary
  onSubmit: (payload: StashApplyPayload) => void
}): ReactElement {
  const [dropAfterApply, setDropAfterApply] = useState(false)

  useEffect(() => {
    if (open) setDropAfterApply(false)
  }, [open])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="stash-apply-dialog">
        <DialogHeader>
          <DialogTitle>Apply Stash</DialogTitle>
          <DialogDescription>
            Apply changes of the stash to your working directory.
          </DialogDescription>
        </DialogHeader>
        <code className="truncate rounded bg-muted px-2 py-1 font-mono text-[12px] text-muted-foreground">
          {stashLabel(stash)}
        </code>
        <FieldGroup>
          <Field orientation="horizontal">
            <Checkbox
              id="stash-drop-after"
              checked={dropAfterApply}
              onCheckedChange={(value) => setDropAfterApply(value === true)}
            />
            <FieldLabel htmlFor="stash-drop-after" className="font-normal">
              Drop stash after applying
            </FieldLabel>
          </Field>
          <FieldDescription>
            The stash is kept if a conflict occurs.
          </FieldDescription>
        </FieldGroup>
        <DialogFooter>
          <DialogClose data-testid="dialog-cancel" render={<Button variant="outline" />}>
            Cancel
          </DialogClose>
          <Button
            data-testid="dialog-submit"
            onClick={() => {
              onSubmit({ dropAfterApply })
              onOpenChange(false)
            }}
          >
            Apply
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
