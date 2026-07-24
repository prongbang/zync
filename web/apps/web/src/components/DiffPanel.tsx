import { useMemo, useState } from "react"
import type { ReactElement } from "react"
import {
  FileMinus2,
  FilePen,
  FilePlus2,
  FileSymlink,
  History,
} from "lucide-react"

import { Button } from "@workspace/ui/components/button"
import { ScrollArea } from "@workspace/ui/components/scroll-area"
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@workspace/ui/components/toggle-group"
import { cn } from "@workspace/ui/lib/utils"

import { shortId } from "@/lib/format"
import {
  compactDiffKind,
  compactDiffMarker,
  compactDiffText,
  diffHunks,
  diffIsPatch,
  patchFileKind,
  pathBasename,
  splitDiffLines,
  splitPatchByFile,
} from "@/lib/helpers"
import type {
  BlameRow,
  CompactKind,
  DiffHunk,
  DiffLine,
  DiffSegment,
  PatchFile,
  PatchFileStatus,
  SplitDiffLine,
  SplitKind,
} from "@/lib/helpers"

// Which committed/working revision to preview for each side of an image diff.
export type ImageDiffSide = "before" | "after"

// Fork-style diff panel: inline / split / blame toolbar over a single file's
// unified diff. Ported from crates/ui/src/components/diff.rs
// (ForkCompactDiff / SplitDiffSection / BlameTable) — same hunk parser
// (diffHunks/splitDiffLines from @/lib/helpers). Neutral shadcn surfaces plus
// functional --zync-diff-* highlight colors, per
// web/.agents/skills/shadcn/SKILL.md.

type DiffViewMode = "inline" | "split"

export interface DiffPanelProps {
  path: string
  diff: string
  blame: BlameRow[] | null
  onStageHunk: (patch: string) => void
  onRequestBlame: () => void
  onCloseBlame: () => void
  /**
   * Resolves the <img> src for one side of an image file's diff, or `null` when
   * unavailable (e.g. no repository open). `path` is the file's old path for
   * "before" and new path for "after". Omit to fall back to the text renderer.
   */
  imageSrc?: (path: string, side: ImageDiffSide) => string | null
  /**
   * Opens the per-file History view (P1.2) for the currently-displayed file
   * (single-file diff's `path`, or the diff-file-tree's active file). Omit to
   * hide the History button, e.g. when there is no file context to key off of.
   */
  onOpenFileHistory?: (path: string) => void
  /**
   * Jumps to a commit from the Blame view's commit/author gutter (P1.2) —
   * drives the same selected-commit state as clicking a row in the graph.
   * Omit to render blame rows' commit column as plain (non-interactive) text.
   */
  onSelectBlameCommit?: (commitId: string) => void
}

// A multi-file patch (whole-commit / whole-workdir diff) is split per
// `diff --git` header so we can render a file list and lazily parse only the
// selected file's hunks. `displayPath` keys each row and drives selection.
function displayPath(file: PatchFile): string {
  return file.status === "deleted" ? file.oldPath : file.newPath
}

export function DiffPanel({
  path,
  diff,
  blame,
  onStageHunk,
  onRequestBlame,
  onCloseBlame,
  imageSrc,
  onOpenFileHistory,
  onSelectBlameCommit,
}: DiffPanelProps): ReactElement {
  const [viewMode, setViewMode] = useState<DiffViewMode>("inline")
  const [selectedKey, setSelectedKey] = useState<string | null>(null)

  const files = useMemo(() => splitPatchByFile(diff), [diff])
  const isMultiFile = files.length > 1

  // Resolve the active file: the user's pick if it still exists in the current
  // patch, otherwise the first file. When single-file, we feed the whole diff.
  const activeFile = isMultiFile
    ? (files.find((file) => displayPath(file) === selectedKey) ?? files[0])
    : null
  const activeDiff = activeFile ? activeFile.patch : diff
  const headerPath = activeFile ? displayPath(activeFile) : path

  // Image files render as a before/after preview instead of textual hunks, so the
  // Inline/Split/Blame toggle is meaningless and hidden. Detection works in both
  // single-file (files[0]) and multi-file (activeFile) modes.
  const currentFile = activeFile ?? files[0] ?? null
  const isImage =
    currentFile !== null && imageSrc !== undefined && patchFileKind(currentFile) === "image"

  // Blame is keyed to the workspace's selected file (the `path`/`blame` props);
  // for multi-file whole-commit diffs `path` is empty, so blame stays disabled.
  const showBlame = blame !== null
  const mode: DiffViewMode | "blame" = showBlame ? "blame" : viewMode
  const blameDisabled = isMultiFile || path === ""

  const handleModeChange = (value: string[]) => {
    const next = value[0]
    if (!next) return
    if (next === "blame") {
      onRequestBlame()
    } else {
      setViewMode(next as DiffViewMode)
      onCloseBlame()
    }
  }

  const content = (
    <div className="bg-background flex h-full min-h-0 min-w-0 flex-1 flex-col">
      <header className="border-border flex shrink-0 items-center gap-2 border-b px-2.5 py-1.5">
        <code className="min-w-0 flex-1 truncate text-[11px] text-muted-foreground">
          {headerPath === "" ? "Select a file" : headerPath}
        </code>
        {headerPath !== "" && onOpenFileHistory && (
          <Button
            type="button"
            data-testid="open-file-history"
            variant="ghost"
            size="xs"
            className="shrink-0"
            onClick={() => onOpenFileHistory(headerPath)}
          >
            <History data-icon="inline-start" />
            History
          </Button>
        )}
        {!isImage && (
          <ToggleGroup
            size="sm"
            className="shrink-0"
            aria-label="Diff view mode"
            value={[mode]}
            onValueChange={handleModeChange}
          >
            <ToggleGroupItem value="inline" data-testid="diff-inline">
              Inline
            </ToggleGroupItem>
            <ToggleGroupItem value="split" data-testid="diff-split">
              Split
            </ToggleGroupItem>
            <ToggleGroupItem
              value="blame"
              data-testid="diff-blame"
              disabled={blameDisabled}
            >
              Blame
            </ToggleGroupItem>
          </ToggleGroup>
        )}
      </header>
      {isImage && currentFile ? (
        <ImageDiffView file={currentFile} imageSrc={imageSrc!} />
      ) : (
        <div className="min-h-0 flex-1 overflow-auto font-mono text-[12px] leading-5">
          {showBlame ? (
            <BlameView rows={blame} onSelectCommit={onSelectBlameCommit} />
          ) : activeDiff.trim() === "" ? (
            <EmptyDiffState />
          ) : viewMode === "split" ? (
            <SplitDiffView diff={activeDiff} />
          ) : (
            <InlineDiffView diff={activeDiff} onStageHunk={onStageHunk} />
          )}
        </div>
      )}
    </div>
  )

  if (!isMultiFile) return content

  return (
    <div className="bg-background flex h-full min-h-0 min-w-0">
      <DiffFileTree
        files={files}
        activePath={activeFile ? displayPath(activeFile) : null}
        onSelect={setSelectedKey}
      />
      {content}
    </div>
  )
}

// ---------------------------------------------------------------------------
// File tree — a narrow, scrollable list of the files touched by the patch.
// Selection uses the Button ghost/secondary variant pair (no invented state
// styling) and aria-current for assistive tech.
// ---------------------------------------------------------------------------

const statusMeta: Record<
  PatchFileStatus,
  { icon: typeof FilePlus2; className: string; label: string }
> = {
  added: { icon: FilePlus2, className: "text-primary", label: "Added" },
  modified: { icon: FilePen, className: "text-muted-foreground", label: "Modified" },
  deleted: { icon: FileMinus2, className: "text-destructive", label: "Deleted" },
  renamed: { icon: FileSymlink, className: "text-muted-foreground", label: "Renamed" },
}

function DiffFileTree({
  files,
  activePath,
  onSelect,
}: {
  files: PatchFile[]
  activePath: string | null
  onSelect: (path: string) => void
}) {
  return (
    <nav
      data-testid="diff-file-tree"
      aria-label="Changed files"
      className="border-border flex w-56 shrink-0 flex-col border-r"
    >
      <div className="border-border text-muted-foreground flex shrink-0 items-center justify-between border-b px-2.5 py-1.5 text-xs font-semibold tracking-wide uppercase">
        <span>Files</span>
        <span>{files.length}</span>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <ul className="flex flex-col gap-0.5 p-1">
          {files.map((file) => {
            const key = displayPath(file)
            const isActive = key === activePath
            return (
              <li key={key}>
                <DiffFileRow
                  file={file}
                  isActive={isActive}
                  onSelect={() => onSelect(key)}
                />
              </li>
            )
          })}
        </ul>
      </ScrollArea>
    </nav>
  )
}

function DiffFileRow({
  file,
  isActive,
  onSelect,
}: {
  file: PatchFile
  isActive: boolean
  onSelect: () => void
}) {
  const meta = statusMeta[file.status]
  const Icon = meta.icon
  const full = displayPath(file)
  const name = pathBasename(full)
  const dir = full.slice(0, full.length - name.length)
  return (
    <Button
      type="button"
      data-testid="diff-file-row"
      variant={isActive ? "secondary" : "ghost"}
      size="xs"
      aria-current={isActive ? "true" : undefined}
      title={file.status === "renamed" ? `${file.oldPath} → ${file.newPath}` : full}
      onClick={onSelect}
      className="w-full justify-start px-2 font-normal"
    >
      <Icon
        data-icon="inline-start"
        className={meta.className}
        aria-label={meta.label}
      />
      <span className="min-w-0 flex-1 truncate text-left font-mono">
        {dir !== "" && <span className="text-muted-foreground/70">{dir}</span>}
        <span>{name}</span>
      </span>
    </Button>
  )
}

function EmptyDiffState() {
  return (
    <div className="p-4 text-muted-foreground">Select a changed file to show its diff.</div>
  )
}

// ---------------------------------------------------------------------------
// Image view — side-by-side Before/After preview for image files. Added files
// show only After, deleted only Before, modified/renamed both. Panes sit on a
// muted background so transparent pixels read against it.
// ---------------------------------------------------------------------------

function ImageDiffView({
  file,
  imageSrc,
}: {
  file: PatchFile
  imageSrc: (path: string, side: ImageDiffSide) => string | null
}) {
  const showBefore = file.status !== "added"
  const showAfter = file.status !== "deleted"
  return (
    <div
      className={cn(
        "grid min-h-0 flex-1 gap-px bg-border",
        showBefore && showAfter ? "grid-cols-2" : "grid-cols-1",
      )}
    >
      {showBefore && (
        <ImagePane
          label="Before"
          testid="diff-image-before"
          src={imageSrc(file.oldPath, "before")}
        />
      )}
      {showAfter && (
        <ImagePane
          label="After"
          testid="diff-image-after"
          src={imageSrc(file.newPath, "after")}
        />
      )}
    </div>
  )
}

function ImagePane({
  label,
  testid,
  src,
}: {
  label: string
  testid: string
  src: string | null
}) {
  return (
    <figure className="bg-background flex min-h-0 min-w-0 flex-col">
      <figcaption className="border-border text-muted-foreground border-b px-3 py-1.5 text-[11px] font-semibold tracking-wide uppercase">
        {label}
      </figcaption>
      <div className="bg-muted flex min-h-0 flex-1 items-center justify-center overflow-auto p-4">
        {src === null ? (
          <span className="text-muted-foreground text-xs">No preview available</span>
        ) : (
          <img
            src={src}
            alt={`${label} image`}
            data-testid={testid}
            className="max-h-full max-w-full object-contain"
          />
        )}
      </div>
    </figure>
  )
}

// ---------------------------------------------------------------------------
// Inline view — one hunk per block, "Stage hunk" button, +/- line coloring.
// ---------------------------------------------------------------------------

// Exported for reuse by FileHistorySheet (P1.2), which shows a historical
// commit's per-file diff read-only — same hunk parser/renderer, just without a
// stage action (`onStageHunk` omitted hides the "Stage hunk" button).
export function InlineDiffView({
  diff,
  onStageHunk,
}: {
  diff: string
  onStageHunk?: (patch: string) => void
}) {
  const hunks = diffHunks(diff)
  if (hunks.length === 0) {
    return (
      <pre className="whitespace-pre-wrap break-words p-3 text-muted-foreground">{diff}</pre>
    )
  }
  return (
    <div className="divide-border divide-y">
      {hunks.map((hunk, index) => (
        <HunkBlock key={`${hunk.title}:${index}`} hunk={hunk} onStageHunk={onStageHunk} />
      ))}
    </div>
  )
}

function HunkBlock({
  hunk,
  onStageHunk,
}: {
  hunk: DiffHunk
  onStageHunk?: (patch: string) => void
}) {
  const canStage = diffIsPatch(hunk.patch)
  return (
    <article>
      <div className="border-border bg-muted sticky top-0 z-10 flex items-center justify-between gap-2 border-b px-2.5 py-1">
        <code className="min-w-0 truncate text-[11px] text-muted-foreground">{hunk.title}</code>
        {onStageHunk && (
          <Button
            data-testid="stage-hunk"
            type="button"
            variant="outline"
            size="xs"
            className="h-5 shrink-0 px-2 text-[10px] font-semibold"
            disabled={!canStage}
            onClick={() => onStageHunk(hunk.patch)}
          >
            Stage hunk
          </Button>
        )}
      </div>
      <div className="px-1 py-0.5">
        {hunk.lines.map((line) => (
          <DiffLineRow key={line.key} line={line} />
        ))}
      </div>
    </article>
  )
}

function inlineKindClass(kind: CompactKind): string {
  switch (kind) {
    case "added":
      return "bg-[var(--zync-diff-added-bg)] text-[var(--zync-diff-added-fg)]"
    case "removed":
      return "bg-[var(--zync-diff-removed-bg)] text-[var(--zync-diff-removed-fg)]"
    case "hunk":
      return "bg-[var(--zync-diff-hunk-bg)] text-[var(--zync-diff-hunk-fg)]"
    default:
      return "text-muted-foreground"
  }
}

function DiffLineRow({ line }: { line: DiffLine }) {
  const kind = compactDiffKind(line.text)
  const marker = compactDiffMarker(line.text)
  const text = compactDiffText(line.text)
  return (
    <div className={cn("grid grid-cols-[18px_1fr] gap-1 rounded-sm px-1", inlineKindClass(kind))}>
      <span className="text-muted-foreground/70 text-center select-none">{marker}</span>
      <pre className="min-w-0 overflow-visible whitespace-pre-wrap break-words">{text}</pre>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Split view — old|new columns with word-level highlight segments.
// ---------------------------------------------------------------------------

function SplitDiffView({ diff }: { diff: string }) {
  const lines = splitDiffLines(diffHunks(diff))
  if (lines.length === 0) {
    return <EmptyDiffState />
  }
  return (
    <div className="min-w-[560px]">
      <div className="border-border bg-muted text-muted-foreground sticky top-0 z-10 grid grid-cols-2 border-b text-[11px] font-semibold tracking-wide uppercase">
        <span className="px-2 py-1.5">Old</span>
        <span className="border-border border-l px-2 py-1.5">New</span>
      </div>
      {lines.map((line, index) => (
        <SplitDiffRow key={index} line={line} />
      ))}
    </div>
  )
}

function splitKindClass(kind: SplitKind): string {
  switch (kind) {
    case "header":
      return "bg-[var(--zync-diff-hunk-bg)] text-[var(--zync-diff-hunk-fg)]"
    case "removed":
      return "bg-[var(--zync-diff-removed-bg)] text-[var(--zync-diff-removed-fg)]"
    case "added":
      return "bg-[var(--zync-diff-added-bg)] text-[var(--zync-diff-added-fg)]"
    case "empty":
      return "text-muted-foreground"
    default:
      return "text-muted-foreground"
  }
}

function SplitDiffRow({ line }: { line: SplitDiffLine }) {
  return (
    <div className="grid grid-cols-2">
      <SplitDiffCell
        segments={line.old}
        kind={line.oldKind}
        wordClass="bg-[var(--zync-diff-removed-bg)] rounded-[2px]"
      />
      <SplitDiffCell
        segments={line.new}
        kind={line.newKind}
        wordClass="bg-[var(--zync-diff-added-bg)] rounded-[2px]"
        className="border-border border-l"
      />
    </div>
  )
}

function SplitDiffCell({
  segments,
  kind,
  wordClass,
  className,
}: {
  segments: DiffSegment[]
  kind: SplitKind
  wordClass: string
  className?: string
}) {
  return (
    <pre
      className={cn(
        "min-w-0 px-2 py-0.5 whitespace-pre-wrap break-words",
        splitKindClass(kind),
        className,
      )}
    >
      {segments.map((segment, index) => (
        <span key={index} className={segment.changed ? wordClass : undefined}>
          {segment.text}
        </span>
      ))}
    </pre>
  )
}

// ---------------------------------------------------------------------------
// Blame view.
// ---------------------------------------------------------------------------

function BlameView({
  rows,
  onSelectCommit,
}: {
  rows: BlameRow[]
  onSelectCommit?: (commitId: string) => void
}) {
  if (rows.length === 0) {
    return <div className="text-muted-foreground p-3">No blame data available.</div>
  }
  return (
    <div className="divide-border/40 divide-y">
      {rows.map((row) => (
        <div
          key={row.line}
          className="hover:bg-accent grid grid-cols-[44px_70px_110px_minmax(0,1fr)] gap-2 px-3 py-0.5"
        >
          <span className="text-muted-foreground text-right select-none">{row.line}</span>
          {row.commit !== "" && onSelectCommit ? (
            <Button
              type="button"
              data-testid="blame-commit-link"
              variant="link"
              size="xs"
              className="h-auto justify-start p-0 font-mono"
              title={`Jump to commit ${row.commit}`}
              onClick={() => onSelectCommit(row.commit)}
            >
              {shortId(row.commit)}
            </Button>
          ) : (
            <code className="text-muted-foreground">
              {row.commit === "" ? "" : shortId(row.commit)}
            </code>
          )}
          <span className="text-muted-foreground truncate">{row.author}</span>
          <pre className="text-foreground/90 min-w-0 overflow-hidden text-ellipsis whitespace-pre">
            {row.code}
          </pre>
        </div>
      ))}
    </div>
  )
}
