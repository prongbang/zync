// Create-branch dialog. Presentational: manages its own form state, emits a
// payload on submit. Ported from BranchDialog::NewBranch in
// crates/ui/src/components/dialogs.rs.

import { useEffect, useState, type ReactElement } from "react"

import { Button } from "@workspace/ui/components/button"
import { Checkbox } from "@workspace/ui/components/checkbox"
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
  FieldTitle,
} from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@workspace/ui/components/toggle-group"

export type LocalChangesMode = "dont-change" | "stash-reapply" | "discard"

export type NewBranchPayload = {
  name: string
  startPoint: string
  checkout: boolean
  localMode: LocalChangesMode
}

export function NewBranchDialog({
  open,
  onOpenChange,
  branch,
  startPoint = "",
  hasLocalChanges = false,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Commit/branch the new branch is created at. */
  branch: string
  startPoint?: string
  hasLocalChanges?: boolean
  onSubmit: (payload: NewBranchPayload) => void
}): ReactElement {
  const [name, setName] = useState("")
  const [start, setStart] = useState(startPoint)
  const [checkout, setCheckout] = useState(true)
  const [localMode, setLocalMode] = useState<LocalChangesMode>("dont-change")

  useEffect(() => {
    if (open) {
      setName("")
      setStart(startPoint)
      setCheckout(true)
      setLocalMode("dont-change")
    }
  }, [open, startPoint])

  const submit = () => {
    if (name.trim() === "") return
    onSubmit({ name: name.trim(), startPoint: start.trim(), checkout, localMode })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="new-branch-dialog">
        <DialogHeader>
          <DialogTitle>New Branch</DialogTitle>
          <DialogDescription className="truncate">
            Create a branch at {branch}
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
              <FieldLabel htmlFor="new-branch-name">Branch name</FieldLabel>
              <Input
                id="new-branch-name"
                autoFocus
                placeholder="feature/name"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="new-branch-start">
                Start point (optional)
              </FieldLabel>
              <Input
                id="new-branch-start"
                placeholder={branch}
                value={start}
                onChange={(event) => setStart(event.target.value)}
              />
            </Field>
            <Field orientation="horizontal">
              <Checkbox
                id="new-branch-checkout"
                checked={checkout}
                onCheckedChange={(value) => setCheckout(value === true)}
              />
              <FieldLabel htmlFor="new-branch-checkout" className="font-normal">
                Check out after create
              </FieldLabel>
            </Field>
            <Field data-disabled={!checkout || undefined}>
              <FieldTitle id="new-branch-local-label">
                {hasLocalChanges
                  ? "Local changes"
                  : "Local changes (working tree is clean)"}
              </FieldTitle>
              <ToggleGroup
                aria-labelledby="new-branch-local-label"
                disabled={!checkout}
                value={[localMode]}
                onValueChange={(value) => {
                  const next = value[0] as LocalChangesMode | undefined
                  if (next) setLocalMode(next)
                }}
              >
                <ToggleGroupItem
                  value="dont-change"
                  data-testid="new-branch-local-dont-change"
                >
                  Don't change
                </ToggleGroupItem>
                <ToggleGroupItem
                  value="stash-reapply"
                  data-testid="new-branch-local-stash-reapply"
                >
                  Stash and reapply
                </ToggleGroupItem>
                <ToggleGroupItem value="discard" data-testid="new-branch-local-discard">
                  Discard
                </ToggleGroupItem>
              </ToggleGroup>
              <FieldDescription>
                Discard permanently removes uncommitted changes before checkout.
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
            <Button
              data-testid="dialog-submit"
              type="submit"
              disabled={name.trim() === ""}
            >
              {checkout ? "Create and Checkout" : "Create Branch"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
