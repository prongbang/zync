// Create-tag dialog. Ported from BranchDialog::NewTag.

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

export type TagPayload = { name: string; target: string }

export function TagDialog({
  open,
  onOpenChange,
  target = "",
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Default tag target (branch/commit the tag points at). */
  target?: string
  onSubmit: (payload: TagPayload) => void
}): ReactElement {
  const [name, setName] = useState("")
  const [tagTarget, setTagTarget] = useState(target)

  useEffect(() => {
    if (open) {
      setName("")
      setTagTarget(target)
    }
  }, [open, target])

  const submit = () => {
    if (name.trim() === "") return
    onSubmit({ name: name.trim(), target: tagTarget.trim() })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="tag-dialog">
        <DialogHeader>
          <DialogTitle>New Tag</DialogTitle>
          <DialogDescription className="truncate">
            Tag {target}
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="tag-name">Tag name</FieldLabel>
              <Input
                id="tag-name"
                autoFocus
                placeholder="v1.0.0"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="tag-target">Target</FieldLabel>
              <Input
                id="tag-target"
                placeholder={target}
                value={tagTarget}
                onChange={(event) => setTagTarget(event.target.value)}
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
            <Button data-testid="dialog-submit" type="submit" disabled={name.trim() === ""}>
              Create Tag
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
