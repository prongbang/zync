// Start-bisect dialog (P2.6). Opened from the commit context menu's "Start Bisect from
// Here..." — the right-clicked commit is the presumed-bad starting point; the user supplies a
// known-good revision (an older SHA, tag, or branch name) to bound the search.

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
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"

import { shortId } from "@/lib/format"

export type BisectStartPayload = { bad: string; good: string }

export function BisectStartDialog({
  open,
  onOpenChange,
  bad,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Full id of the commit to mark bad — the bisect's starting point (shown abbreviated). */
  bad: string
  onSubmit: (payload: BisectStartPayload) => void
}): ReactElement {
  const [good, setGood] = useState("")

  useEffect(() => {
    if (open) setGood("")
  }, [open])

  const submit = () => {
    if (good.trim() === "") return
    onSubmit({ bad, good: good.trim() })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="bisect-start-dialog">
        <DialogHeader>
          <DialogTitle>Start Bisect</DialogTitle>
          <DialogDescription className="truncate">
            Marks {shortId(bad)} as bad and searches back to a known-good revision.
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
              <FieldLabel htmlFor="bisect-good">Good revision</FieldLabel>
              <Input
                id="bisect-good"
                autoFocus
                placeholder="v1.0.0, a branch name, or a commit SHA"
                value={good}
                onChange={(event) => setGood(event.target.value)}
              />
              <FieldDescription>
                A commit, tag, or branch known to be good — before the bug was introduced.
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
              data-testid="bisect-start"
              type="submit"
              disabled={good.trim() === ""}
            >
              Start Bisect
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
