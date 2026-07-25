// Minimal "new branch at a specific revision" dialog for the Reflog tab of
// GitToolsPanel (P2.1). Deliberately smaller than NewBranchDialog — no
// local-changes handling, since GitToolsPanel calls the git-core
// `createBranchAt` endpoint directly rather than going through useWorkspace's
// stash/discard orchestration (see GitToolsPanel.tsx's self-contained
// pattern note).

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
import { Field, FieldGroup, FieldLabel } from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"

export type BranchAtRevisionPayload = { name: string; checkout: boolean }

export function BranchAtRevisionDialog({
  open,
  onOpenChange,
  revision,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Short id of the commit the new branch starts at. */
  revision: string
  onSubmit: (payload: BranchAtRevisionPayload) => void
}): ReactElement {
  const [name, setName] = useState("")
  const [checkout, setCheckout] = useState(true)

  useEffect(() => {
    if (open) {
      setName("")
      setCheckout(true)
    }
  }, [open])

  const canSubmit = name.trim() !== ""

  const submit = () => {
    if (!canSubmit) return
    onSubmit({ name: name.trim(), checkout })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="branch-at-revision-dialog">
        <DialogHeader>
          <DialogTitle>New Branch</DialogTitle>
          <DialogDescription className="truncate">
            Create a branch at{" "}
            <span className="text-muted-foreground font-mono">
              {revision}
            </span>
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
              <FieldLabel htmlFor="branch-at-revision-name">
                Branch name
              </FieldLabel>
              <Input
                id="branch-at-revision-name"
                autoFocus
                placeholder="feature/name"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
            </Field>
            <Field orientation="horizontal">
              <Checkbox
                id="branch-at-revision-checkout"
                checked={checkout}
                onCheckedChange={(value) => setCheckout(value === true)}
              />
              <FieldLabel
                htmlFor="branch-at-revision-checkout"
                className="font-normal"
              >
                Check out after create
              </FieldLabel>
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
              disabled={!canSubmit}
            >
              {checkout ? "Create and Checkout" : "Create Branch"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
