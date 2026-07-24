// React port of crates/ui/src/components/graph.rs (CommitGraph + GraphLaneStrip
// + CommitContextMenu). Presentational only: the parent owns commit data,
// selection state, and menu-action side effects (see App.tsx).

import * as React from "react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { Button } from "@workspace/ui/components/button"
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@workspace/ui/components/context-menu"
import { cn } from "@workspace/ui/lib/utils"

import type { GraphRow } from "@/lib/helpers"
import { laneColor } from "@/lib/helpers"
import { formatCommitTime, shortId } from "@/lib/format"

export type CommitMenuAction =
  | "new-branch"
  | "new-tag"
  | "rebase-here"
  | "interactive-rebase"
  | "reword"
  | "edit"
  | "squash"
  | "fixup"
  | "drop"
  | "reset-here"
  | "checkout"
  | "cherry-pick"
  | "revert"
  | "save-patch"
  | "compare-local"
  | "copy-sha"

// Fixed row/lane geometry — any change here must keep every row exactly 34px
// tall or the windowing math below drifts (see CLAUDE.md "Virtualized commit
// list").
const ROW_HEIGHT = 34
const LANE_WIDTH = 13
const OVERSCAN_ROWS = 10
const GRAPH_COLUMN_WIDTH = 90

// graph | description | author | commit | date
const GRID_TEMPLATE_COLUMNS = `${GRAPH_COLUMN_WIDTH}px minmax(0,1fr) 96px 84px 132px`

function refClassName(kind: string): string {
  switch (kind) {
    case "head":
      return "border-primary/60 bg-accent text-foreground"
    case "local":
      return "border-border text-foreground"
    case "remote":
      return "border-border text-muted-foreground"
    case "tag":
      return "border-dashed border-border text-muted-foreground"
    default:
      return "border-border text-muted-foreground"
  }
}

function GraphLaneCell({ row }: { row: GraphRow }) {
  const width = row.laneCount * LANE_WIDTH
  const laneCenter = (lane: number) => lane * LANE_WIDTH + LANE_WIDTH / 2
  const dotX = laneCenter(row.lane)
  const midY = ROW_HEIGHT / 2

  return (
    <div
      className="h-[34px] shrink-0 overflow-hidden"
      style={{ width: GRAPH_COLUMN_WIDTH }}
    >
      <svg
        className="block h-[34px]"
        width={width}
        height={ROW_HEIGHT}
        viewBox={`0 0 ${width} ${ROW_HEIGHT}`}
      >
        {Array.from({ length: row.laneCount }, (_, lane) => {
          const top = row.topLanes.has(lane)
          const bottom = row.bottomLanes.has(lane)
          const isCommitLane = lane === row.lane
          const isMergeLane = !isCommitLane && row.mergeLanes.has(lane)
          const color = laneColor(lane)
          const x = laneCenter(lane)
          return (
            <g key={lane}>
              {top ? (
                <line
                  x1={x}
                  y1={0}
                  x2={x}
                  y2={bottom && !isCommitLane ? ROW_HEIGHT : midY}
                  stroke={color}
                  strokeWidth={2}
                  strokeLinecap="round"
                />
              ) : null}
              {bottom && (isCommitLane || (!top && !isMergeLane)) ? (
                <line
                  x1={x}
                  y1={midY}
                  x2={x}
                  y2={ROW_HEIGHT}
                  stroke={color}
                  strokeWidth={2}
                  strokeLinecap="round"
                />
              ) : null}
              {isMergeLane ? (
                <path
                  d={`M ${dotX} ${midY} Q ${x} ${midY} ${x} ${ROW_HEIGHT}`}
                  fill="none"
                  stroke={color}
                  strokeWidth={2}
                  strokeLinecap="round"
                />
              ) : null}
            </g>
          )
        })}
        <circle
          cx={dotX}
          cy={midY}
          r={3.6}
          fill={laneColor(row.lane)}
          stroke="var(--background)"
          strokeWidth={1.6}
        />
      </svg>
    </div>
  )
}

function CommitRow({
  row,
  selected,
  onSelect,
  onMenuAction,
}: {
  row: GraphRow
  selected: boolean
  onSelect: (id: string) => void
  onMenuAction: (action: CommitMenuAction, commitId: string) => void
}) {
  const commit = row.commit
  const fire = useCallback(
    (action: CommitMenuAction) => onMenuAction(action, commit.id),
    [onMenuAction, commit.id],
  )

  return (
    <ContextMenu>
      <ContextMenuTrigger
        data-testid="commit-row"
        data-commit-id={commit.id}
        className={cn(
          "grid h-[34px] cursor-pointer items-center gap-2 border-b border-border/60 px-2 text-[13px] text-foreground/90",
          selected
            ? "bg-accent"
            : "hover:bg-accent/40",
        )}
        style={{ gridTemplateColumns: GRID_TEMPLATE_COLUMNS }}
        onClick={() => onSelect(commit.id)}
      >
        <GraphLaneCell row={row} />
        <span className="flex min-w-0 items-center gap-1.5 overflow-hidden">
          {commit.refs.slice(0, 4).map((ref) => (
            <span
              key={`${ref.kind}:${ref.name}`}
              className={cn(
                "inline-flex max-w-[140px] shrink-0 items-center truncate rounded border px-1 py-0 text-[11px] font-semibold leading-tight",
                refClassName(ref.kind),
              )}
            >
              {ref.name}
            </span>
          ))}
          {commit.refs.length > 4 ? (
            <span className="shrink-0 text-[11px] text-muted-foreground">
              +{commit.refs.length - 4}
            </span>
          ) : null}
          <span className="min-w-0 truncate font-medium text-foreground">
            {commit.summary}
          </span>
        </span>
        <span className="w-24 min-w-0 truncate text-[12px] text-muted-foreground">
          {commit.author}
        </span>
        <code className="text-muted-foreground min-w-0 truncate font-mono text-[12px] font-semibold">
          {shortId(commit.id)}
        </code>
        <span className="min-w-0 truncate text-[12px] tabular-nums text-muted-foreground">
          {formatCommitTime(commit.time)}
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent className="w-56">
        <ContextMenuGroup>
          <ContextMenuItem onClick={() => fire("new-branch")}>
            New Branch...
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("new-tag")}>
            New Tag...
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem onClick={() => fire("rebase-here")}>
            Rebase to Here...
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("interactive-rebase")}>
            Interactive Rebase...
          </ContextMenuItem>
          <ContextMenuLabel>Quick Actions</ContextMenuLabel>
          <ContextMenuItem onClick={() => fire("reword")}>
            Reword Message...
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("edit")}>Edit...</ContextMenuItem>
          <ContextMenuItem onClick={() => fire("squash")}>
            Squash into Parent
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("fixup")}>
            Fixup into Parent
          </ContextMenuItem>
          <ContextMenuItem variant="destructive" onClick={() => fire("drop")}>
            Drop...
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem
            variant="destructive"
            onClick={() => fire("reset-here")}
          >
            Reset to Here...
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem onClick={() => fire("checkout")}>
            Checkout Commit
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("cherry-pick")}>
            Cherry-pick Commit
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("revert")}>
            Revert Commit
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("save-patch")}>
            Save as Patch
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem onClick={() => fire("compare-local")}>
            Compare to Local Changes
          </ContextMenuItem>
          <ContextMenuItem onClick={() => fire("copy-sha")}>
            Copy Commit SHA
          </ContextMenuItem>
        </ContextMenuGroup>
      </ContextMenuContent>
    </ContextMenu>
  )
}

export function CommitGraph(props: {
  rows: GraphRow[]
  selectedId: string | null
  onSelect: (id: string) => void
  onLoadMore: () => void
  onMenuAction: (action: CommitMenuAction, commitId: string) => void
}): React.ReactElement {
  const { rows, selectedId, onSelect, onLoadMore, onMenuAction } = props

  const listRef = useRef<HTMLOListElement>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(720)

  const measure = useCallback(() => {
    const el = listRef.current
    if (!el) return
    setViewportHeight(Math.max(el.clientHeight, 200))
  }, [])

  useEffect(() => {
    measure()
    const el = listRef.current
    if (!el || typeof ResizeObserver === "undefined") return
    const observer = new ResizeObserver(measure)
    observer.observe(el)
    return () => observer.disconnect()
  }, [measure])

  const handleScroll = useCallback(() => {
    const el = listRef.current
    if (!el) return
    setScrollTop(el.scrollTop)
    setViewportHeight(Math.max(el.clientHeight, 200))
  }, [])

  const { firstRow, lastRow, topSpacer, bottomSpacer } = useMemo(() => {
    const totalRows = rows.length
    const first = Math.min(
      Math.max(Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN_ROWS, 0),
      totalRows,
    )
    const visible = Math.min(
      Math.ceil(viewportHeight / ROW_HEIGHT) + 2 * OVERSCAN_ROWS,
      totalRows - first,
    )
    const last = first + visible
    return {
      firstRow: first,
      lastRow: last,
      topSpacer: first * ROW_HEIGHT,
      bottomSpacer: (totalRows - last) * ROW_HEIGHT,
    }
  }, [rows.length, scrollTop, viewportHeight])

  const visibleRows = rows.slice(firstRow, lastRow)

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background">
      <header className="flex shrink-0 flex-col border-b border-border">
        <div className="flex h-9 items-center justify-between gap-2 px-3">
          <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Commits
          </span>
          <Button data-testid="load-more" size="xs" variant="outline" onClick={onLoadMore}>
            Load more
          </Button>
        </div>
        <div
          className="grid items-center gap-2 border-t border-border px-2 py-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground"
          style={{ gridTemplateColumns: GRID_TEMPLATE_COLUMNS }}
        >
          <span>Graph</span>
          <span>Description</span>
          <span>Author</span>
          <span>Commit</span>
          <span>Date</span>
        </div>
      </header>
      <ol
        ref={listRef}
        data-testid="commit-list"
        className="min-h-0 flex-1 list-none overflow-y-auto"
        onScroll={handleScroll}
      >
        {firstRow > 0 ? <li style={{ height: topSpacer }} /> : null}
        {visibleRows.map((row) => (
          <li key={row.commit.id}>
            <CommitRow
              row={row}
              selected={row.commit.id === selectedId}
              onSelect={onSelect}
              onMenuAction={onMenuAction}
            />
          </li>
        ))}
        {lastRow < rows.length ? <li style={{ height: bottomSpacer }} /> : null}
      </ol>
    </div>
  )
}
