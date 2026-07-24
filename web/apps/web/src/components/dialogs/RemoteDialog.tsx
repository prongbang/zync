// Add / edit-URL / rename dialog for git remotes (Remotes tab of
// GitToolsPanel). One dialog, three modes — each mode only shows the fields
// it needs. Presentational: controlled open state in, typed payload out.

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

export type RemoteDialogMode = "add" | "edit-url" | "rename"

export type RemotePayload = { name: string; url: string }

const COPY: Record<
  RemoteDialogMode,
  { title: string; submit: string; description: string }
> = {
  add: {
    title: "Add Remote",
    submit: "Add Remote",
    description: "Register a new remote for this repository.",
  },
  "edit-url": {
    title: "Edit Remote URL",
    submit: "Save URL",
    description:
      "The remote is re-added with the new URL; remote-tracking branches refresh on the next fetch.",
  },
  rename: {
    title: "Rename Remote",
    submit: "Rename",
    description:
      "The remote is re-added under the new name; remote-tracking branches refresh on the next fetch.",
  },
}

export function RemoteDialog({
  open,
  onOpenChange,
  mode,
  remoteName = "",
  remoteUrl = "",
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: RemoteDialogMode
  /** Existing remote name (edit-url / rename context, rename seed). */
  remoteName?: string
  /** Existing remote URL (edit-url seed). */
  remoteUrl?: string
  onSubmit: (payload: RemotePayload) => void
}): ReactElement {
  const [name, setName] = useState(mode === "add" ? "" : remoteName)
  const [url, setUrl] = useState(mode === "add" ? "" : remoteUrl)

  useEffect(() => {
    if (open) {
      setName(mode === "add" ? "" : remoteName)
      setUrl(mode === "add" ? "" : remoteUrl)
    }
  }, [open, mode, remoteName, remoteUrl])

  const showName = mode === "add" || mode === "rename"
  const showUrl = mode === "add" || mode === "edit-url"
  const canSubmit =
    (!showName || name.trim() !== "") && (!showUrl || url.trim() !== "")

  const submit = () => {
    if (!canSubmit) return
    onSubmit({
      name: showName ? name.trim() : remoteName,
      url: showUrl ? url.trim() : remoteUrl,
    })
    onOpenChange(false)
  }

  const copy = COPY[mode]

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="remote-dialog">
        <DialogHeader>
          <DialogTitle>
            {copy.title}
            {mode !== "add" && (
              <>
                {" "}
                <span className="text-muted-foreground font-mono text-sm">
                  {remoteName}
                </span>
              </>
            )}
          </DialogTitle>
          <DialogDescription>{copy.description}</DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <FieldGroup>
            {showName && (
              <Field>
                <FieldLabel htmlFor="remote-name">
                  {mode === "rename" ? "New name" : "Name"}
                </FieldLabel>
                <Input
                  id="remote-name"
                  autoFocus
                  placeholder="origin"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                />
              </Field>
            )}
            {showUrl && (
              <Field>
                <FieldLabel htmlFor="remote-url">URL</FieldLabel>
                <Input
                  id="remote-url"
                  autoFocus={mode === "edit-url"}
                  placeholder="git@github.com:owner/repo.git"
                  value={url}
                  onChange={(event) => setUrl(event.target.value)}
                />
              </Field>
            )}
          </FieldGroup>
          <DialogFooter className="mt-6">
            <DialogClose
              data-testid="dialog-cancel"
              render={<Button variant="outline" type="button" />}
            >
              Cancel
            </DialogClose>
            <Button data-testid="dialog-submit" type="submit" disabled={!canSubmit}>
              {copy.submit}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
