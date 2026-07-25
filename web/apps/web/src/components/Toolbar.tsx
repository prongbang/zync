// Desktop-style git toolbar. Presentational: the parent owns repo state and wires
// each action to a workspace mutation (see App.tsx / useWorkspace.ts).
//
// Fetch/Pull/Push are split controls (ButtonGroup: main action + a small
// chevron trigger opening a DropdownMenu of alternate modes). Each op tracks
// its own busy state and reports its outcome as a toast, in addition to the
// footer notice that useWorkspace's remote actions already set.

import { useState, type ReactElement } from "react"
import { ArrowDown, ArrowUp, ArrowDownToLine, Archive, ChevronDown } from "lucide-react"

import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { ButtonGroup } from "@workspace/ui/components/button-group"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@workspace/ui/components/dropdown-menu"
import { Spinner } from "@workspace/ui/components/spinner"
import { toast } from "@workspace/ui/components/toast"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@workspace/ui/components/tooltip"

import type { BranchSummary, PullMode } from "@/lib/types"
import { ConfirmDialog } from "./dialogs"

type RemoteOp = "fetch" | "pull" | "push"

export function Toolbar({
  disabled,
  branches,
  onFetch,
  onPull,
  onPush,
  onStash,
}: {
  disabled: boolean
  branches: BranchSummary[]
  onFetch: (all?: boolean) => Promise<string>
  onPull: (mode?: PullMode) => Promise<string>
  onPush: (opts?: { forceWithLease?: boolean; setUpstream?: boolean }) => Promise<string>
  onStash: () => void
}): ReactElement {
  const [busy, setBusy] = useState<RemoteOp | null>(null)
  const [forcePushOpen, setForcePushOpen] = useState(false)

  const current = branches.find((b) => b.is_head) ?? null
  const ahead = current?.ahead ?? 0
  const behind = current?.behind ?? 0
  // Best-effort signal: the server never sends an explicit "has upstream"
  // flag (see BranchSummary in lib/types.ts), so a head branch whose ahead
  // *and* behind are both absent is inferred to have no upstream configured.
  const noUpstream = current != null && current.ahead == null && current.behind == null

  const controlsDisabled = disabled || busy !== null

  async function runOp(op: RemoteOp, action: () => Promise<string>) {
    setBusy(op)
    try {
      const message = await action()
      toast.add({ title: message, type: "success" })
    } catch (error) {
      toast.add({
        title: error instanceof Error ? error.message : String(error),
        type: "error",
      })
    } finally {
      setBusy(null)
    }
  }

  return (
    <TooltipProvider>
      <div className="flex items-center gap-1.5">
        {/* Fetch */}
        <ButtonGroup>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  data-testid="toolbar-fetch"
                  variant="ghost"
                  size="icon-sm"
                  disabled={controlsDisabled}
                  aria-label="Fetch"
                  onClick={() => void runOp("fetch", () => onFetch())}
                />
              }
            >
              {busy === "fetch" ? <Spinner /> : <ArrowDownToLine />}
            </TooltipTrigger>
            <TooltipContent>Fetch</TooltipContent>
          </Tooltip>
          <DropdownMenu>
            <DropdownMenuTrigger
              data-testid="fetch-menu"
              render={
                <Button
                  variant="ghost"
                  size="icon-xs"
                  disabled={controlsDisabled}
                  aria-label="Fetch options"
                />
              }
            >
              <ChevronDown />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              <DropdownMenuGroup>
                <DropdownMenuItem onClick={() => void runOp("fetch", () => onFetch(true))}>
                  Fetch all remotes
                </DropdownMenuItem>
              </DropdownMenuGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </ButtonGroup>

        {/* Pull */}
        <div className="relative">
          <ButtonGroup>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    data-testid="toolbar-pull"
                    variant="ghost"
                    size="icon-sm"
                    disabled={controlsDisabled}
                    aria-label="Pull"
                    onClick={() => void runOp("pull", () => onPull("ff-only"))}
                  />
                }
              >
                {busy === "pull" ? <Spinner /> : <ArrowDown />}
              </TooltipTrigger>
              <TooltipContent>Pull (fast-forward only)</TooltipContent>
            </Tooltip>
            <DropdownMenu>
              <DropdownMenuTrigger
                data-testid="pull-menu"
                render={
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    disabled={controlsDisabled}
                    aria-label="Pull options"
                  />
                }
              >
                <ChevronDown />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start">
                <DropdownMenuGroup>
                  <DropdownMenuItem onClick={() => void runOp("pull", () => onPull("ff-only"))}>
                    Pull (fast-forward only)
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => void runOp("pull", () => onPull("merge"))}>
                    Pull (merge)
                  </DropdownMenuItem>
                  <DropdownMenuItem onClick={() => void runOp("pull", () => onPull("rebase"))}>
                    Pull (rebase)
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </ButtonGroup>
          {behind > 0 && (
            <Badge
              variant="secondary"
              aria-label={`${behind} commit${behind === 1 ? "" : "s"} behind`}
              className="pointer-events-none absolute -top-1.5 -right-1.5 h-5 min-w-5 justify-center rounded-full px-1 tabular-nums"
            >
              {behind}
            </Badge>
          )}
        </div>

        {/* Push / Publish branch */}
        <div className="relative">
          <ButtonGroup>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    data-testid="toolbar-push"
                    variant="ghost"
                    size="icon-sm"
                    disabled={controlsDisabled}
                    aria-label={noUpstream ? "Publish branch" : "Push"}
                    onClick={() =>
                      void runOp("push", () => onPush(noUpstream ? { setUpstream: true } : undefined))
                    }
                  />
                }
              >
                {busy === "push" ? <Spinner /> : <ArrowUp />}
              </TooltipTrigger>
              <TooltipContent>{noUpstream ? "Publish branch" : "Push"}</TooltipContent>
            </Tooltip>
            <DropdownMenu>
              <DropdownMenuTrigger
                data-testid="push-menu"
                render={
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    disabled={controlsDisabled}
                    aria-label="Push options"
                  />
                }
              >
                <ChevronDown />
              </DropdownMenuTrigger>
              <DropdownMenuContent align="start">
                <DropdownMenuGroup>
                  <DropdownMenuItem
                    onClick={() =>
                      void runOp("push", () => onPush(noUpstream ? { setUpstream: true } : undefined))
                    }
                  >
                    {noUpstream ? "Publish branch" : "Push"}
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    variant="destructive"
                    onClick={() => setForcePushOpen(true)}
                  >
                    Force Push (with lease)...
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </ButtonGroup>
          {ahead > 0 && (
            <Badge
              variant="secondary"
              aria-label={`${ahead} commit${ahead === 1 ? "" : "s"} ahead`}
              className="pointer-events-none absolute -top-1.5 -right-1.5 h-5 min-w-5 justify-center rounded-full px-1 tabular-nums"
            >
              {ahead}
            </Badge>
          )}
        </div>

        {/* Stash */}
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                data-testid="toolbar-stash"
                variant="ghost"
                size="icon-sm"
                disabled={disabled}
                aria-label="Stash"
                onClick={onStash}
              />
            }
          >
            <Archive />
          </TooltipTrigger>
          <TooltipContent>Stash</TooltipContent>
        </Tooltip>
      </div>

      <ConfirmDialog
        open={forcePushOpen}
        onOpenChange={setForcePushOpen}
        title="Force push with lease?"
        description="This overwrites the remote branch's history with your local branch. Force-with-lease aborts if the remote moved since your last fetch, but any commits only on the remote will still be lost."
        subject={current ? `origin/${current.name}` : "origin"}
        confirmLabel="Force Push"
        destructive
        testId="force-push-confirm"
        onConfirm={() => void runOp("push", () => onPush({ forceWithLease: true }))}
      />
    </TooltipProvider>
  )
}
