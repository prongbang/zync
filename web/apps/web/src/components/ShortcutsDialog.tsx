// P2.3 — keyboard shortcuts cheat sheet. Opened from the command palette's
// "Keyboard shortcuts" item or the "?" key (see use-shortcuts.ts).

import type { ReactElement } from "react"

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@workspace/ui/components/dialog"

import { formatKey, SHORTCUTS } from "@/hooks/use-shortcuts"

export function ShortcutsDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}): ReactElement {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="shortcuts-dialog">
        <DialogHeader>
          <DialogTitle>Keyboard shortcuts</DialogTitle>
          <DialogDescription>
            Fork-style shortcuts for mouse-free navigation.
          </DialogDescription>
        </DialogHeader>
        <dl className="flex flex-col gap-1">
          {SHORTCUTS.map((shortcut) => (
            <div
              key={shortcut.keys.join("+") + shortcut.description}
              className="flex items-center justify-between gap-4 py-1"
            >
              <dt className="text-muted-foreground text-sm">
                {shortcut.description}
              </dt>
              <dd className="flex items-center gap-1">
                {shortcut.keys.map((key, index) => (
                  <kbd
                    key={index}
                    className="bg-muted text-muted-foreground inline-flex h-5 min-w-5 items-center justify-center rounded border px-1.5 font-mono text-xs"
                  >
                    {formatKey(key)}
                  </kbd>
                ))}
              </dd>
            </div>
          ))}
        </dl>
      </DialogContent>
    </Dialog>
  )
}
