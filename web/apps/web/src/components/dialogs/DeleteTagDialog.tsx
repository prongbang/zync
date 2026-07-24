// Delete-tag confirm dialog. Mirrors DeleteDialog (branch delete).

import type { ReactElement } from "react"

import { ConfirmDialog } from "./ConfirmDialog"

export function DeleteTagDialog({
  open,
  onOpenChange,
  tag,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  tag: string
  onSubmit: () => void
}): ReactElement {
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Delete Tag"
      description="Delete this local tag. This cannot be undone from Zync."
      subject={tag}
      confirmLabel="Delete"
      destructive
      onConfirm={onSubmit}
      testId="delete-tag-dialog"
    />
  )
}
