// Drop-commit confirm dialog. Ported from BranchDialog::DropCommit.

import type { ReactElement } from "react"

import { ConfirmDialog } from "./ConfirmDialog"

export function DropDialog({
  open,
  onOpenChange,
  commit,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Short id of the commit to drop. */
  commit: string
  onSubmit: () => void
}): ReactElement {
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Drop Commit"
      description="Remove this commit from history and re-apply every later commit. This cannot be undone from Zync."
      subject={commit}
      confirmLabel="Drop"
      destructive
      onConfirm={onSubmit}
      testId="drop-dialog"
    />
  )
}
