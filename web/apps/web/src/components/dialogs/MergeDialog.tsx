// Merge-branch confirm dialog. Ported from BranchDialog::Merge.

import type { ReactElement } from "react"

import { ConfirmDialog } from "./ConfirmDialog"

export function MergeDialog({
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
      title="Merge Branch"
      description="Merge this branch into the current branch."
      subject={branch}
      confirmLabel="Merge"
      onConfirm={onSubmit}
      testId="merge-dialog"
    />
  )
}
