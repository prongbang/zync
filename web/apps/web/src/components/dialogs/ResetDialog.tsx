// Reset-to-commit dialog. Ported from BranchDialog::ResetToCommit.

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

export type ResetMode = "mixed" | "hard"
export type ResetPayload = { mode: ResetMode }

export function ResetDialog({
  open,
  onOpenChange,
  commit,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Short id of the commit to reset onto. */
  commit: string
  onSubmit: (payload: ResetPayload) => void
}): ReactElement {
  const [mode, setMode] = useState<ResetMode>("mixed")

  useEffect(() => {
    if (open) setMode("mixed")
  }, [open])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="reset-dialog">
        <DialogHeader>
          <DialogTitle>Reset to Commit</DialogTitle>
          <DialogDescription>
            Move the current branch to this commit.
          </DialogDescription>
        </DialogHeader>
        <code className="truncate rounded bg-muted px-2 py-1 font-mono text-[12px] text-muted-foreground">
          {commit}
        </code>
        <FieldSet>
          <FieldLegend variant="label">Mode</FieldLegend>
          <RadioGroup
            value={mode}
            onValueChange={(value) => setMode(value as ResetMode)}
          >
            <FieldGroup className="gap-3">
              <Field orientation="horizontal">
                <RadioGroupItem value="mixed" id="reset-mixed" />
                <FieldLabel htmlFor="reset-mixed" className="font-normal">
                  Mixed — keep changes in the working tree
                </FieldLabel>
              </Field>
              <Field orientation="horizontal">
                <RadioGroupItem value="hard" id="reset-hard" />
                <FieldLabel
                  htmlFor="reset-hard"
                  className="font-normal text-destructive"
                >
                  Hard — discard all changes after this commit
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
            variant={mode === "hard" ? "destructive" : "default"}
            onClick={() => {
              onSubmit({ mode })
              onOpenChange(false)
            }}
          >
            Reset
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
