// Fork-style git toolbar. Presentational: the parent owns repo state and wires
// each action to a workspace mutation (see App.tsx / useWorkspace.ts).

import type { ReactElement } from "react"
import { ArrowDown, ArrowDownToLine, ArrowUp, Archive } from "lucide-react"

import { Button } from "@workspace/ui/components/button"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@workspace/ui/components/tooltip"

export type ToolbarAction = "fetch" | "pull" | "push" | "stash"

const ACTIONS: {
  kind: ToolbarAction
  label: string
  icon: typeof ArrowDownToLine
}[] = [
  { kind: "fetch", label: "Fetch", icon: ArrowDownToLine },
  { kind: "pull", label: "Pull", icon: ArrowDown },
  { kind: "push", label: "Push", icon: ArrowUp },
  { kind: "stash", label: "Stash", icon: Archive },
]

export function Toolbar({
  disabled,
  onAction,
}: {
  disabled: boolean
  onAction: (kind: ToolbarAction) => void
}): ReactElement {
  return (
    <TooltipProvider>
      <div className="flex items-center gap-1">
        {ACTIONS.map(({ kind, label, icon: Icon }) => (
          <Tooltip key={kind}>
            <TooltipTrigger
              render={
                <Button
                  data-testid={`toolbar-${kind}`}
                  variant="ghost"
                  size="icon-sm"
                  disabled={disabled}
                  aria-label={label}
                  onClick={() => onAction(kind)}
                />
              }
            >
              <Icon />
            </TooltipTrigger>
            <TooltipContent>{label}</TooltipContent>
          </Tooltip>
        ))}
      </div>
    </TooltipProvider>
  )
}
