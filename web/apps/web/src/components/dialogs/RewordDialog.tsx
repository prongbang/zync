// Reword-commit dialog. Ported from BranchDialog::RewordCommit.

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
import { Textarea } from "@workspace/ui/components/textarea"

export type RewordPayload = { message: string }

export function RewordDialog({
  open,
  onOpenChange,
  commit,
  message = "",
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Short id of the commit being reworded (shown for context). */
  commit: string
  message?: string
  onSubmit: (payload: RewordPayload) => void
}): ReactElement {
  const [value, setValue] = useState(message)

  useEffect(() => {
    if (open) setValue(message)
  }, [open, message])

  const submit = () => {
    if (value.trim() === "") return
    onSubmit({ message: value.trim() })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="reword-dialog">
        <DialogHeader>
          <DialogTitle>Reword Commit</DialogTitle>
          <DialogDescription className="truncate">{commit}</DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="reword-message">
                New commit message
              </FieldLabel>
              <Textarea
                id="reword-message"
                autoFocus
                rows={4}
                value={value}
                onChange={(event) => setValue(event.target.value)}
              />
              <FieldDescription>
                Rewrites this commit and re-applies every later commit.
              </FieldDescription>
            </Field>
          </FieldGroup>
          <DialogFooter className="mt-6">
            <DialogClose
              data-testid="dialog-cancel"
              render={<Button variant="outline" type="button" />}
            >
              Cancel
            </DialogClose>
            <Button data-testid="dialog-submit" type="submit" disabled={value.trim() === ""}>
              Reword
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
