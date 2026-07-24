// Delete-branch confirm dialog. Ported from BranchDialog::Delete.

import type { ReactElement } from "react"

import { ConfirmDialog } from "./ConfirmDialog"

export function DeleteDialog({
  open,
  onOpenChange,
  branch,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  branch: string
  onSubmit: () => void
}): ReactElement {
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Delete Branch"
      description="Delete this local branch. This cannot be undone from Zync."
      subject={branch}
      confirmLabel="Delete"
      destructive
      onConfirm={onSubmit}
      testId="delete-dialog"
    />
  )
}
