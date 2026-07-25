// Add-submodule dialog (Submodules tab of GitToolsPanel, P2.2). Single
// mode — url + destination path — mirrors RemoteDialog's controlled
// open-state-in / typed-payload-out shape.

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

export type SubmodulePayload = { url: string; path: string }

export function SubmoduleDialog({
  open,
  onOpenChange,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (payload: SubmodulePayload) => void
}): ReactElement {
  const [url, setUrl] = useState("")
  const [path, setPath] = useState("")

  useEffect(() => {
    if (open) {
      setUrl("")
      setPath("")
    }
  }, [open])

  const canSubmit = url.trim() !== "" && path.trim() !== ""

  const submit = () => {
    if (!canSubmit) return
    onSubmit({ url: url.trim(), path: path.trim() })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="submodule-dialog">
        <DialogHeader>
          <DialogTitle>Add Submodule</DialogTitle>
          <DialogDescription>
            Clones the repository into the given path and registers it in
            .gitmodules.
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
              <FieldLabel htmlFor="submodule-url">URL</FieldLabel>
              <Input
                id="submodule-url"
                autoFocus
                placeholder="git@github.com:owner/repo.git"
                value={url}
                onChange={(event) => setUrl(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="submodule-path">Path</FieldLabel>
              <Input
                id="submodule-path"
                placeholder="vendor/lib"
                value={path}
                onChange={(event) => setPath(event.target.value)}
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
            <Button
              data-testid="dialog-submit"
              type="submit"
              disabled={!canSubmit}
            >
              Add Submodule
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
