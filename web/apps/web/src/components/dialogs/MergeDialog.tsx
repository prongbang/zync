// Merge-branch dialog. Ported from BranchDialog::Merge; extended with a merge-strategy picker
// (P1.7) using the same FieldSet + RadioGroup shape as ResetDialog's mode picker.

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

import type { MergeStrategy } from "../../lib/types"

export type MergePayload = { strategy: MergeStrategy }

export function MergeDialog({
  open,
  onOpenChange,
  branch,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  branch: string
  onSubmit: (payload: MergePayload) => void
}): ReactElement {
  // "no-ff" matches the server's pre-strategy default: always create a merge commit.
  const [strategy, setStrategy] = useState<MergeStrategy>("no-ff")

  useEffect(() => {
    if (open) setStrategy("no-ff")
  }, [open])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="merge-dialog">
        <DialogHeader>
          <DialogTitle>Merge Branch</DialogTitle>
          <DialogDescription>
            Merge this branch into the current branch.
          </DialogDescription>
        </DialogHeader>
        <code className="truncate rounded bg-muted px-2 py-1 font-mono text-[12px] text-muted-foreground">
          {branch}
        </code>
        <FieldSet>
          <FieldLegend variant="label">Strategy</FieldLegend>
          <RadioGroup
            value={strategy}
            onValueChange={(value) => setStrategy(value as MergeStrategy)}
            data-testid="merge-strategy"
          >
            <FieldGroup className="gap-3">
              <Field orientation="horizontal">
                <RadioGroupItem value="no-ff" id="merge-no-ff" />
                <FieldLabel htmlFor="merge-no-ff" className="font-normal">
                  No fast-forward — always create a merge commit
                </FieldLabel>
              </Field>
              <Field orientation="horizontal">
                <RadioGroupItem value="ff-only" id="merge-ff-only" />
                <FieldLabel htmlFor="merge-ff-only" className="font-normal">
                  Fast-forward only — fail if the branches diverged
                </FieldLabel>
              </Field>
              <Field orientation="horizontal">
                <RadioGroupItem value="squash" id="merge-squash" />
                <FieldLabel htmlFor="merge-squash" className="font-normal">
                  Squash — stage the changes without committing
                </FieldLabel>
              </Field>
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
              onSubmit({ strategy })
              onOpenChange(false)
            }}
          >
            Merge
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
