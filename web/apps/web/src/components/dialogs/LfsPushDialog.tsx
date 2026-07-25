// LFS push dialog (LFS tab of GitToolsPanel, P2.2). `git lfs push` needs an
// explicit remote + branch (unlike `lfs pull`, which infers both from the
// current checkout), so this collects them instead of guessing.

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

export type LfsPushPayload = { remote: string; branch: string }

export function LfsPushDialog({
  open,
  onOpenChange,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (payload: LfsPushPayload) => void
}): ReactElement {
  const [remote, setRemote] = useState("origin")
  const [branch, setBranch] = useState("")

  useEffect(() => {
    if (open) {
      setRemote("origin")
      setBranch("")
    }
  }, [open])

  const canSubmit = remote.trim() !== "" && branch.trim() !== ""

  const submit = () => {
    if (!canSubmit) return
    onSubmit({ remote: remote.trim(), branch: branch.trim() })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="lfs-push-dialog">
        <DialogHeader>
          <DialogTitle>Push LFS Objects</DialogTitle>
          <DialogDescription>
            Uploads large-file objects tracked on this branch to the remote.
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
              <FieldLabel htmlFor="lfs-push-remote">Remote</FieldLabel>
              <Input
                id="lfs-push-remote"
                autoFocus
                placeholder="origin"
                value={remote}
                onChange={(event) => setRemote(event.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="lfs-push-branch">Branch</FieldLabel>
              <Input
                id="lfs-push-branch"
                placeholder="main"
                value={branch}
                onChange={(event) => setBranch(event.target.value)}
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
              Push
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
