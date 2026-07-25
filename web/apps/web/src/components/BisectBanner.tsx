// Bisect status banner (P2.6). Shown whenever a `git bisect` session is active — mounted just
// under the toolbar in App.tsx so it spans the full content width regardless of which center
// tab/mode is open. Presentational only: parent owns the bisect status + action side effects.

import type { ReactElement } from "react"

import { CheckIcon, GitCommitVerticalIcon, SkipForwardIcon, XIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@workspace/ui/components/alert"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"

import { shortId } from "@/lib/format"
import type { BisectStatus } from "@/lib/types"

export function BisectBanner({
  status,
  onGood,
  onBad,
  onSkip,
  onReset,
}: {
  status: BisectStatus
  onGood: () => void
  onBad: () => void
  onSkip: () => void
  onReset: () => void
}): ReactElement {
  return (
    <Alert
      data-testid="bisect-banner"
      className="rounded-none border-x-0 border-t-0 px-3 py-2"
    >
      <GitCommitVerticalIcon />
      <AlertTitle className="flex flex-wrap items-center gap-2">
        Bisecting
        {status.current_commit && (
          <code className="text-muted-foreground font-mono text-xs font-normal">
            {shortId(status.current_commit)}
          </code>
        )}
        {typeof status.steps_remaining === "number" && (
          <Badge variant="outline">
            ~{status.steps_remaining} step{status.steps_remaining === 1 ? "" : "s"} left
          </Badge>
        )}
      </AlertTitle>
      <AlertDescription className="flex flex-wrap items-center justify-between gap-2">
        <span>
          Test the checked-out commit, then mark it good or bad to narrow the range.
        </span>
        <span className="flex shrink-0 items-center gap-2">
          <Button data-testid="bisect-good" size="xs" variant="outline" onClick={onGood}>
            <CheckIcon data-icon="inline-start" />
            Good
          </Button>
          <Button data-testid="bisect-bad" size="xs" variant="outline" onClick={onBad}>
            <XIcon data-icon="inline-start" />
            Bad
          </Button>
          <Button data-testid="bisect-skip" size="xs" variant="outline" onClick={onSkip}>
            <SkipForwardIcon data-icon="inline-start" />
            Skip
          </Button>
          <Button data-testid="bisect-reset" size="xs" variant="destructive" onClick={onReset}>
            Reset
          </Button>
        </span>
      </AlertDescription>
    </Alert>
  )
}
