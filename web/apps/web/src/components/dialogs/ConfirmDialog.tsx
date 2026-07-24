// Shared confirm scaffold for the no-input git dialogs (merge/delete/drop).
// Ported from the confirm branches of BranchActionDialog.

import type { ReactElement, ReactNode } from "react"

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

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  subject,
  confirmLabel,
  destructive = false,
  onConfirm,
  testId,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description: ReactNode
  /** The git object the action targets, rendered as monospace. */
  subject: string
  confirmLabel: string
  destructive?: boolean
  onConfirm: () => void
  /** data-testid for the dialog root, so callers with distinct purposes (delete/merge/drop) get stable, distinguishable selectors. */
  testId?: string
}): ReactElement {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid={testId}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <code className="truncate rounded bg-muted px-2 py-1 font-mono text-[12px] text-muted-foreground">
          {subject}
        </code>
        <DialogFooter>
          <DialogClose data-testid="dialog-cancel" render={<Button variant="outline" />}>
            Cancel
          </DialogClose>
          <Button
            data-testid="dialog-submit"
            variant={destructive ? "destructive" : "default"}
            onClick={() => {
              onConfirm()
              onOpenChange(false)
            }}
          >
            {confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
