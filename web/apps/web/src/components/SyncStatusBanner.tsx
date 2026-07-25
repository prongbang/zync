// Live-sync reconnect banner (P5.6). Shown only when the workspace websocket
// dropped after having connected at least once and is now backing off to
// reconnect — so the user knows live updates are paused. Auto-dismisses when the
// socket reconnects (the parent stops passing `visible`). Mounted just under the
// toolbar in App.tsx, spanning the full content width like BisectBanner.
//
// Presentational only: the parent (useWorkspace) owns the connection state. A
// neutral shadcn Alert with a Spinner conveys "reconnecting in progress" using
// the component's own default (neutral foreground) styling — no brand accent, in
// keeping with DESIGN.md's neutral theme and the styling rules.

import type { ReactElement } from "react"

import { Alert, AlertDescription, AlertTitle } from "@workspace/ui/components/alert"
import { Spinner } from "@workspace/ui/components/spinner"

export function SyncStatusBanner(): ReactElement {
  return (
    <Alert
      data-testid="sync-reconnecting-banner"
      className="rounded-none border-x-0 border-t-0 px-3 py-2"
    >
      <Spinner />
      <AlertTitle>Live sync reconnecting…</AlertTitle>
      <AlertDescription>
        The connection to this workspace dropped. Live updates are paused and
        will resume automatically once it reconnects.
      </AlertDescription>
    </Alert>
  )
}
