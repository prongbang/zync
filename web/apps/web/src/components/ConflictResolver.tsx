// React port of the conflict-resolution surface in
// crates/ui/src/components/panels.rs (ConflictEditorPanel's per-file accept
// actions, simplified to a flat picklist). Presentational only: the parent
// owns the conflict list and performs the actual `git checkout --ours/theirs`
// + add on `onResolve` (see App.tsx). Rebuilt on shadcn Card primitives per
// web/.agents/skills/shadcn/SKILL.md.

import type { ReactElement } from "react"
import { AlertTriangle, FileWarning } from "lucide-react"

import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import {
  Card,
  CardAction,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@workspace/ui/components/card"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@workspace/ui/components/empty"

import type { ConflictSummary } from "@/lib/types"

export interface ConflictResolverProps {
  conflicts: ConflictSummary[]
  onResolve: (path: string, side: "local" | "remote") => void
}

function conflictPath(conflict: ConflictSummary): string {
  return conflict.ours ?? conflict.theirs ?? conflict.ancestor ?? ""
}

export function ConflictResolver({
  conflicts,
  onResolve,
}: ConflictResolverProps): ReactElement {
  if (conflicts.length === 0) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <FileWarning />
          </EmptyMedia>
          <EmptyTitle>No conflicts</EmptyTitle>
          <EmptyDescription>
            Nothing needs manual resolution right now.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <AlertTriangle className="size-4 text-destructive" />
        <span className="text-sm text-muted-foreground">
          {conflicts.length} conflict{conflicts.length === 1 ? "" : "s"} need
          resolution
        </span>
      </div>

      {conflicts.map((conflict) => {
        const path = conflictPath(conflict)
        return (
          <Card key={path} size="sm">
            <CardHeader>
              <CardTitle className="min-w-0">
                <code className="block truncate text-xs text-foreground">{path}</code>
              </CardTitle>
              <CardAction>
                <Badge variant="destructive">Conflicted</Badge>
              </CardAction>
            </CardHeader>
            <CardContent>
              <dl className="grid grid-cols-2 gap-2 text-xs text-muted-foreground">
                <div className="min-w-0">
                  <dt className="uppercase tracking-wide">Ours</dt>
                  <dd className="truncate">{conflict.ours ?? "—"}</dd>
                </div>
                <div className="min-w-0">
                  <dt className="uppercase tracking-wide">Theirs</dt>
                  <dd className="truncate">{conflict.theirs ?? "—"}</dd>
                </div>
              </dl>
            </CardContent>
            <CardFooter className="justify-end gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => onResolve(path, "local")}
              >
                Take ours
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onResolve(path, "remote")}
              >
                Take theirs
              </Button>
            </CardFooter>
          </Card>
        )
      })}
    </div>
  )
}
