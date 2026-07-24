// Rename-branch dialog. Ported from BranchDialog::Rename.

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
import { Field, FieldGroup, FieldLabel } from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"

export type RenamePayload = { newName: string }

export function RenameDialog({
  open,
  onOpenChange,
  branch,
  currentName = "",
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Branch being renamed (shown for context). */
  branch: string
  /** Value the input is seeded with. */
  currentName?: string
  onSubmit: (payload: RenamePayload) => void
}): ReactElement {
  const [newName, setNewName] = useState(currentName)

  useEffect(() => {
    if (open) setNewName(currentName)
  }, [open, currentName])

  const submit = () => {
    if (newName.trim() === "") return
    onSubmit({ newName: newName.trim() })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="rename-dialog">
        <DialogHeader>
          <DialogTitle>Rename Branch</DialogTitle>
          <DialogDescription className="truncate">{branch}</DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="rename-branch">New branch name</FieldLabel>
              <Input
                id="rename-branch"
                autoFocus
                value={newName}
                onChange={(event) => setNewName(event.target.value)}
              />
            </Field>
          </FieldGroup>
          <DialogFooter className="mt-6">
            <DialogClose
              data-testid="dialog-cancel"
              render={<Button variant="outline" type="button" />}
            >
              Cancel
            </DialogClose>
            <Button data-testid="dialog-submit" type="submit" disabled={newName.trim() === ""}>
              Rename
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
