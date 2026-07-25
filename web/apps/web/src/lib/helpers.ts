// Pure logic ported from crates/ui/src/helpers.rs. No rendering — components
// consume these and apply their own Tailwind/token styling.

import type {
  BlameLine,
  BranchSummary,
  CommitSummary,
  FileStatus,
  RebaseStepRequest,
  StashSummary,
} from "./types"

// ---------------------------------------------------------------------------
// Scope bitmask — which workspace signals a websocket event should refetch.
// ---------------------------------------------------------------------------
export const SCOPE_WORKSPACE = 1 << 0
export const SCOPE_STATUS = 1 << 1
export const SCOPE_BRANCHES = 1 << 2
export const SCOPE_GRAPH = 1 << 3
export const SCOPE_STASHES = 1 << 4
export const SCOPE_CONFLICTS = 1 << 5
export const SCOPE_DIFF = 1 << 6
export const SCOPE_TAGS = 1 << 7
export const SCOPE_ALL = 0xff
export const SCOPE_WORKDIR = SCOPE_STATUS | SCOPE_DIFF | SCOPE_CONFLICTS

export function scopeBit(name: string): number {
  switch (name) {
    case "workspace":
      return SCOPE_WORKSPACE
    case "status":
      return SCOPE_STATUS
    case "branches":
      return SCOPE_BRANCHES
    case "commits":
    case "graph":
      return SCOPE_GRAPH
    case "stashes":
      return SCOPE_STASHES
    case "conflicts":
      return SCOPE_CONFLICTS
    case "diff":
      return SCOPE_DIFF
    case "tags":
      return SCOPE_TAGS
    default:
      return SCOPE_ALL
  }
}

export function scopeForEvent(text: string): number {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    return SCOPE_ALL
  }
  if (typeof value !== "object" || value === null) return SCOPE_ALL
  const obj = value as Record<string, unknown>
  const kind = typeof obj.kind === "string" ? obj.kind : ""
  switch (kind) {
    case "workspace_batch":
    case "file_created":
    case "file_changed":
    case "file_deleted":
    case "folder_created":
    case "folder_deleted":
    case "file_renamed":
      return SCOPE_WORKDIR
    case "git_changed": {
      const payload = obj.payload as Record<string, unknown> | undefined
      const scopes = payload?.scopes
      if (!Array.isArray(scopes)) return SCOPE_ALL
      const mask = scopes
        .filter((s): s is string => typeof s === "string")
        .reduce((acc, name) => acc | scopeBit(name), 0)
      return mask !== 0 ? mask : SCOPE_ALL
    }
    default:
      return SCOPE_ALL
  }
}

// ---------------------------------------------------------------------------
// Commit graph lane layout.
// ---------------------------------------------------------------------------
export type GraphRow = {
  commit: CommitSummary
  lane: number
  laneCount: number
  topLanes: Set<number>
  bottomLanes: Set<number>
  mergeLanes: Set<number>
}

export function graphRows(commits: CommitSummary[]): GraphRow[] {
  const lanes: (string | null)[] = []
  const rows: GraphRow[] = []

  const firstFree = () => {
    const idx = lanes.findIndex((id) => id === null)
    return idx === -1 ? lanes.length : idx
  }

  for (const commit of commits) {
    let lane = lanes.findIndex((id) => id === commit.id)
    if (lane === -1) {
      lane = firstFree()
      if (lane === lanes.length) lanes.push(null)
    }

    const topLanes = new Set<number>()
    lanes.forEach((id, index) => {
      if (id !== null) topLanes.add(index)
    })

    lanes[lane] = commit.parents.length > 0 ? commit.parents[0] : null

    const mergeLanes = new Set<number>()
    for (const parent of commit.parents.slice(1)) {
      let target = lanes.findIndex((id) => id === parent)
      if (target === -1) {
        target = firstFree()
        if (target === lanes.length) lanes.push(parent)
        else lanes[target] = parent
      }
      mergeLanes.add(target)
    }

    const bottomLanes = new Set<number>()
    lanes.forEach((id, index) => {
      if (id !== null) bottomLanes.add(index)
    })

    let laneCount = lane
    topLanes.forEach((l) => (laneCount = Math.max(laneCount, l)))
    bottomLanes.forEach((l) => (laneCount = Math.max(laneCount, l)))
    laneCount += 1

    rows.push({ commit, lane, laneCount, topLanes, bottomLanes, mergeLanes })

    while (lanes.length > 0 && lanes[lanes.length - 1] === null) lanes.pop()
  }

  return rows
}

const LANE_COLORS = [
  "#2dd4bf",
  "#f59e0b",
  "#a78bfa",
  "#fb7185",
  "#38bdf8",
  "#34d399",
  "#f472b6",
]

export function laneColor(lane: number): string {
  return LANE_COLORS[lane % LANE_COLORS.length]
}

// ---------------------------------------------------------------------------
// Commit search/filter (P1.3). Client-side matching over already-loaded
// commits; mirrors the case-insensitive substring match `search_commits`
// (crates/git-core/src/lib.rs) does server-side for full-history search, so
// a query behaves the same whether it's filtering the loaded window or
// hitting the network.
// ---------------------------------------------------------------------------
export function commitMatchesQuery(commit: CommitSummary, query: string): boolean {
  const needle = query.trim().toLowerCase()
  if (needle === "") return true
  return (
    commit.summary.toLowerCase().includes(needle) ||
    commit.author.toLowerCase().includes(needle) ||
    commit.author_email.toLowerCase().includes(needle) ||
    commit.id.toLowerCase().includes(needle)
  )
}

// ---------------------------------------------------------------------------
// Diff parsing (unified patch -> hunks/lines/split/word segments).
// ---------------------------------------------------------------------------
export type DiffLine = {
  key: string
  index: number
  text: string
  selectable: boolean
}

export type DiffHunk = {
  title: string
  header: string[]
  oldStart: number
  newStart: number
  lines: DiffLine[]
  patch: string
}

export type DiffSegment = { text: string; changed: boolean }

export type SplitKind = "header" | "removed" | "added" | "context" | "empty"

export type SplitDiffLine = {
  old: DiffSegment[]
  new: DiffSegment[]
  oldKind: SplitKind
  newKind: SplitKind
}

export function diffIsPatch(diff: string): boolean {
  return diff.includes("diff --git") && diff.includes("@@")
}

function parseRangeStart(value: string): number | null {
  const first = value.split(",")[0]
  const n = Number.parseInt(first, 10)
  return Number.isNaN(n) ? null : n
}

export function parseHunkStarts(header: string): [number, number] | null {
  const parts = header.split(/\s+/)
  if (parts.length < 3) return null
  const oldStart = parseRangeStart(parts[1].replace(/^-/, ""))
  const newStart = parseRangeStart(parts[2].replace(/^\+/, ""))
  if (oldStart === null || newStart === null) return null
  return [oldStart, newStart]
}

function buildPatch(header: string[], hunk: string[]): string {
  let patch = header.join("\n")
  if (patch.length > 0) patch += "\n"
  patch += hunk.join("\n")
  patch += "\n"
  return patch
}

function diffLines(hunkIndex: number, lines: string[]): DiffLine[] {
  return lines.map((line, index) => ({
    key: `${hunkIndex}:${index}`,
    index,
    text: line,
    selectable:
      index > 0 &&
      (line.startsWith("+") || line.startsWith("-")) &&
      !line.startsWith("+++ ") &&
      !line.startsWith("--- "),
  }))
}

export function diffHunks(diff: string): DiffHunk[] {
  if (!diffIsPatch(diff)) return []
  let fileHeader: string[] = []
  let current: string[] = []
  let title = ""
  let oldStart = 0
  let newStart = 0
  const hunks: DiffHunk[] = []
  let hunkIndex = 0

  const flush = () => {
    hunks.push({
      title,
      header: [...fileHeader],
      oldStart,
      newStart,
      lines: diffLines(hunkIndex, current),
      patch: buildPatch(fileHeader, current),
    })
    hunkIndex += 1
    current = []
  }

  for (const line of diff.split("\n")) {
    if (line.startsWith("diff --git ")) {
      if (current.length > 0) flush()
      fileHeader = [line]
      title = line
    } else if (line.startsWith("@@")) {
      if (current.length > 0) flush()
      title = line
      const starts = parseHunkStarts(line)
      if (starts) {
        oldStart = starts[0]
        newStart = starts[1]
      }
      current.push(line)
    } else if (current.length === 0) {
      fileHeader.push(line)
    } else {
      current.push(line)
    }
  }
  if (current.length > 0) flush()
  return hunks
}

// ---------------------------------------------------------------------------
// Per-file split of a multi-file unified patch. A commit / working-tree diff
// concatenates every file, each introduced by a `diff --git a/… b/…` header.
// splitPatchByFile() slices the patch on those headers into ordered per-file
// sub-patches (each still a valid unified diff) so DiffPanel can lazily feed
// one file at a time to diffHunks()/splitDiffLines(). Pure + unit-testable.
// ---------------------------------------------------------------------------
export type PatchFileStatus = "added" | "modified" | "deleted" | "renamed"

export type PatchFile = {
  oldPath: string
  newPath: string
  status: PatchFileStatus
  patch: string
}

function stripDiffPrefix(value: string): string {
  if (value.startsWith("a/") || value.startsWith("b/")) return value.slice(2)
  return value
}

function parsePatchFile(block: string[]): PatchFile {
  const header = block[0] ?? ""
  let oldPath = ""
  let newPath = ""
  let isNew = false
  let isDeleted = false
  let isRename = false

  const headerMatch = /^diff --git a\/(.+) b\/(.+)$/.exec(header)
  if (headerMatch) {
    oldPath = headerMatch[1]
    newPath = headerMatch[2]
  }

  for (const line of block) {
    if (line.startsWith("new file mode")) {
      isNew = true
    } else if (line.startsWith("deleted file mode")) {
      isDeleted = true
    } else if (line.startsWith("rename from ")) {
      oldPath = line.slice("rename from ".length)
      isRename = true
    } else if (line.startsWith("rename to ")) {
      newPath = line.slice("rename to ".length)
      isRename = true
    } else if (line.startsWith("copy from ")) {
      oldPath = line.slice("copy from ".length)
      isRename = true
    } else if (line.startsWith("copy to ")) {
      newPath = line.slice("copy to ".length)
      isRename = true
    } else if (line.startsWith("--- ")) {
      const value = line.slice(4).trim()
      if (value === "/dev/null") isNew = true
      else oldPath = stripDiffPrefix(value)
    } else if (line.startsWith("+++ ")) {
      const value = line.slice(4).trim()
      if (value === "/dev/null") isDeleted = true
      else newPath = stripDiffPrefix(value)
    }
  }

  let status: PatchFileStatus
  if (isRename) status = "renamed"
  else if (isNew) status = "added"
  else if (isDeleted) status = "deleted"
  else status = "modified"

  if (oldPath === "") oldPath = newPath
  if (newPath === "") newPath = oldPath

  return { oldPath, newPath, status, patch: `${block.join("\n")}\n` }
}

export function splitPatchByFile(patch: string): PatchFile[] {
  if (!patch.includes("diff --git ")) return []
  const files: PatchFile[] = []
  let block: string[] = []

  const flush = () => {
    if (block.length === 0) return
    files.push(parsePatchFile(block))
    block = []
  }

  for (const line of patch.split("\n")) {
    if (line.startsWith("diff --git ")) {
      flush()
      block = [line]
    } else if (block.length > 0) {
      block.push(line)
    }
  }
  flush()
  return files
}

/** Extracts one file's sub-patch from a full commit patch, for the file-history
 * "diff at this commit" view (P1.2). Matches on new or old path since a commit
 * could add, modify, delete, or rename the file. Returns "" if the commit's
 * patch didn't touch the path (also its natural "nothing changed" state). */
export function patchForPath(patch: string, path: string): string {
  const file = splitPatchByFile(patch).find(
    (f) => f.newPath === path || f.oldPath === path,
  )
  return file ? file.patch : ""
}

function plainSegments(text: string): DiffSegment[] {
  return [{ text, changed: false }]
}

function intraLineSegments(
  oldText: string,
  newText: string,
): [DiffSegment[], DiffSegment[]] {
  const oldMarker = oldText.slice(0, Math.min(1, oldText.length))
  const newMarker = newText.slice(0, Math.min(1, newText.length))
  const oldChars = [...oldText].slice(1)
  const newChars = [...newText].slice(1)

  let prefix = 0
  while (
    prefix < oldChars.length &&
    prefix < newChars.length &&
    oldChars[prefix] === newChars[prefix]
  )
    prefix += 1

  let suffix = 0
  while (
    suffix < oldChars.length - prefix &&
    suffix < newChars.length - prefix &&
    oldChars[oldChars.length - 1 - suffix] ===
      newChars[newChars.length - 1 - suffix]
  )
    suffix += 1

  const build = (marker: string, chars: string[]): DiffSegment[] => {
    const head = chars.slice(0, prefix).join("")
    const middle = chars.slice(prefix, chars.length - suffix).join("")
    const tail = chars.slice(chars.length - suffix).join("")
    const segs: DiffSegment[] = [{ text: `${marker}${head}`, changed: false }]
    if (middle.length > 0) segs.push({ text: middle, changed: true })
    if (tail.length > 0) segs.push({ text: tail, changed: false })
    return segs
  }

  return [build(oldMarker, oldChars), build(newMarker, newChars)]
}

export function splitDiffLines(hunks: DiffHunk[]): SplitDiffLine[] {
  const rows: SplitDiffLine[] = []
  for (const hunk of hunks) {
    rows.push({
      old: plainSegments(hunk.title),
      new: plainSegments(hunk.title),
      oldKind: "header",
      newKind: "header",
    })
    let removed: string[] = []
    let added: string[] = []
    const flush = () => {
      const pairs = Math.max(removed.length, added.length)
      for (let i = 0; i < pairs; i++) {
        const o = removed[i]
        const n = added[i]
        if (o !== undefined && n !== undefined) {
          const [os, ns] = intraLineSegments(o, n)
          rows.push({ old: os, new: ns, oldKind: "removed", newKind: "added" })
        } else if (o !== undefined) {
          rows.push({
            old: plainSegments(o),
            new: [],
            oldKind: "removed",
            newKind: "empty",
          })
        } else if (n !== undefined) {
          rows.push({
            old: [],
            new: plainSegments(n),
            oldKind: "empty",
            newKind: "added",
          })
        }
      }
      removed = []
      added = []
    }
    for (const line of hunk.lines.slice(1)) {
      if (line.text.startsWith("-") && !line.text.startsWith("--- ")) {
        removed.push(line.text)
      } else if (line.text.startsWith("+") && !line.text.startsWith("+++ ")) {
        added.push(line.text)
      } else {
        flush()
        rows.push({
          old: plainSegments(line.text),
          new: plainSegments(line.text),
          oldKind: "context",
          newKind: "context",
        })
      }
    }
    flush()
  }
  return rows
}

export function selectedPatchForHunk(
  hunk: DiffHunk,
  selected: Set<number>,
): string | null {
  if (selected.size === 0) return null
  const body: string[] = []
  let oldCount = 0
  let newCount = 0
  for (const line of hunk.lines.slice(1)) {
    const isContext = line.text.startsWith(" ") || line.text.startsWith("\\")
    const isSelected = selected.has(line.index)
    if (isContext || isSelected) {
      if (line.text.startsWith("+") && !line.text.startsWith("+++ ")) newCount++
      else if (line.text.startsWith("-") && !line.text.startsWith("--- "))
        oldCount++
      else if (line.text.startsWith(" ")) {
        oldCount++
        newCount++
      }
      body.push(line.text)
    }
  }
  if (body.every((l) => l.startsWith(" ") || l.startsWith("\\"))) return null
  let patch = hunk.header.join("\n")
  if (patch.length > 0) patch += "\n"
  patch += `@@ -${hunk.oldStart},${oldCount} +${hunk.newStart},${newCount} @@\n`
  patch += body.join("\n")
  patch += "\n"
  return patch
}

export function compactDiffMarker(line: string): string {
  if (line.startsWith("+") && !line.startsWith("+++")) return "+"
  if (line.startsWith("-") && !line.startsWith("---")) return "-"
  return ""
}

export function compactDiffText(line: string): string {
  if (
    line.startsWith("+++") ||
    line.startsWith("---") ||
    line.startsWith("@@")
  )
    return line
  return line.replace(/^[+\- ]/, "")
}

export type CompactKind = "added" | "removed" | "hunk" | "context"

export function compactDiffKind(line: string): CompactKind {
  if (line.startsWith("+") && !line.startsWith("+++")) return "added"
  if (line.startsWith("-") && !line.startsWith("---")) return "removed"
  if (line.startsWith("@@")) return "hunk"
  return "context"
}

// ---------------------------------------------------------------------------
// Blame.
// ---------------------------------------------------------------------------
export type BlameRow = {
  line: number
  commit: string
  author: string
  code: string
}

export function buildBlameRows(ranges: BlameLine[], content: string): BlameRow[] {
  const owner = new Map<number, { commit: string; author: string }>()
  for (const range of ranges) {
    for (let offset = 0; offset < range.line_count; offset++) {
      owner.set(range.start_line + offset, {
        commit: range.commit,
        author: range.author,
      })
    }
  }
  return content.split("\n").map((code, index) => {
    const lineNumber = index + 1
    const o = owner.get(lineNumber)
    return {
      line: lineNumber,
      commit: o?.commit ?? "",
      author: o?.author ?? "",
      code,
    }
  })
}

// ---------------------------------------------------------------------------
// Changed-file tree.
// ---------------------------------------------------------------------------
export type ChangedTreeEntry = {
  name: string
  path: string
  depth: number
  isFile: boolean
  status: string
}

export function statusLabel(file: FileStatus): string {
  if (file.conflicted) return "!"
  if (file.untracked) return "?"
  if (file.staged) return "+"
  if (file.unstaged) return "~"
  return "•"
}

export function changedTreeEntries(files: FileStatus[]): ChangedTreeEntry[] {
  const entries: ChangedTreeEntry[] = []
  const seenDirs = new Set<string>()
  const sorted = [...files].sort((a, b) => a.path.localeCompare(b.path))

  for (const file of sorted) {
    const parts = file.path.split("/")
    let prefix = ""
    parts.forEach((part, index) => {
      const isFile = index === parts.length - 1
      if (prefix.length > 0) prefix += "/"
      prefix += part
      if (isFile) {
        entries.push({
          name: part,
          path: file.path,
          depth: index,
          isFile: true,
          status: statusLabel(file),
        })
      } else if (!seenDirs.has(prefix)) {
        seenDirs.add(prefix)
        entries.push({
          name: part,
          path: prefix,
          depth: index,
          isFile: false,
          status: "",
        })
      }
    })
  }
  return entries
}

// ---------------------------------------------------------------------------
// Branch/stash helpers.
// ---------------------------------------------------------------------------
export function branchGroupRows(
  rows: BranchSummary[],
): [string, BranchSummary[]][] {
  const grouped: [string, BranchSummary[]][] = []
  for (const branch of rows) {
    const slash = branch.name.indexOf("/")
    const groupName = slash === -1 ? "" : branch.name.slice(0, slash)
    // All un-namespaced (root-level) branches share a single "" group, same as
    // any other namespace — otherwise each root branch would produce its own
    // ["", [branch]] tuple, and callers keying off the group name (e.g.
    // BranchSidebar's `key={group || "__root__"}`) would collide on duplicate
    // React keys.
    const existing = grouped.find(([name]) => name === groupName)
    if (existing) existing[1].push(branch)
    else grouped.push([groupName, [branch]])
  }
  return grouped
}

export function branchLeafLabel(branch: BranchSummary, group: string): string {
  if (group === "") return branch.name
  const prefix = `${group}/`
  return branch.name.startsWith(prefix)
    ? branch.name.slice(prefix.length)
    : branch.name
}

export function stashLabel(stash: StashSummary): string {
  if (stash.message.trim() === "") return `#${stash.index} ${stash.name}`
  return `stash@{${stash.index}} ${stash.message.trim()}`
}

// ---------------------------------------------------------------------------
// Interactive-rebase plan builders.
// ---------------------------------------------------------------------------
export function quickRebasePlan(
  commits: CommitSummary[],
  targetId: string,
  action: string,
  message: string | undefined,
): { base: string; steps: RebaseStepRequest[] } {
  const index = commits.findIndex((c) => c.id === targetId)
  if (index === -1) throw new Error("Commit is not in the loaded graph")
  const target = commits[index]
  if (target.parents.length !== 1)
    throw new Error("Quick actions need a commit with exactly one parent")
  const base = target.parents[0]
  // "reword" is a UI-only intent, not a server-side RebaseAction (git-core's
  // RebaseAction enum only has pick/squash/fixup/drop/edit). Interactive
  // rebase's Pick step already accepts an optional message override
  // (crates/git-core/src/lib.rs `replay_commit`'s `ReplayMode::Pick(Option<String>)`),
  // so a reword is sent on the wire as `{ action: "pick", message }` — same
  // mapping InteractiveRebaseDialog uses for its per-row "reword" action.
  const wireStep: RebaseStepRequest =
    action === "reword"
      ? { commit: targetId, action: "pick", message }
      : { commit: targetId, action, message }
  const steps: RebaseStepRequest[] = [wireStep]
  for (let i = index - 1; i >= 0; i--) {
    const descendant = commits[i]
    if (descendant.parents.length > 1)
      throw new Error("Quick actions across merge commits are not supported")
    steps.push({ commit: descendant.id, action: "pick" })
  }
  return { base, steps }
}

export function moveRebaseStep(
  steps: RebaseStepRequest[],
  commit: string,
  direction: number,
): RebaseStepRequest[] {
  const next = [...steps]
  const index = next.findIndex((s) => s.commit === commit)
  if (index === -1) return next
  const target =
    direction < 0
      ? Math.max(0, index - 1)
      : Math.min(index + 1, next.length - 1)
  ;[next[index], next[target]] = [next[target], next[index]]
  return next
}

// The full-todo variant of quickRebasePlan's single-target range computation
// (same base/merge-commit rules), used by the interactive rebase todo editor
// to seed its ordered, oldest-first row list before the user reorders/retypes
// per-row actions. Returns commit ids only — the caller owns action/message
// state per row.
export function rebaseRangeForTarget(
  commits: CommitSummary[],
  targetId: string,
): { base: string; ids: string[] } {
  const index = commits.findIndex((c) => c.id === targetId)
  if (index === -1) throw new Error("Commit is not in the loaded graph")
  const target = commits[index]
  if (target.parents.length !== 1)
    throw new Error(
      "Interactive rebase needs a base commit with exactly one parent",
    )
  const base = target.parents[0]
  const ids = [targetId]
  for (let i = index - 1; i >= 0; i--) {
    const descendant = commits[i]
    if (descendant.parents.length > 1)
      throw new Error(
        "Interactive rebase across merge commits is not supported",
      )
    ids.push(descendant.id)
  }
  return { base, ids }
}

// ---------------------------------------------------------------------------
// Formatting / misc.
// ---------------------------------------------------------------------------
const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
]

function divEuclid(a: number, b: number): number {
  return Math.floor(a / b)
}

function remEuclid(a: number, b: number): number {
  return ((a % b) + b) % b
}

// Howard Hinnant civil-from-days; UTC.
export function civilFromDays(days: number): [number, number, number] {
  const z = days + 719468
  const era = divEuclid(z, 146097)
  const doe = remEuclid(z, 146097)
  const yoe = Math.floor(
    (doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365,
  )
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100))
  const mp = Math.floor((5 * doy + 2) / 153)
  const day = doy - Math.floor((153 * mp + 2) / 5) + 1
  const month = mp < 10 ? mp + 3 : mp - 9
  const year = yoe + era * 400 + (month <= 2 ? 1 : 0)
  return [year, month, day]
}

export function formatCommitTime(seconds: number): string {
  const days = divEuclid(seconds, 86400)
  const secondOfDay = remEuclid(seconds, 86400)
  const [year, month, day] = civilFromDays(days)
  const hh = String(Math.floor(secondOfDay / 3600)).padStart(2, "0")
  const mm = String(Math.floor((secondOfDay % 3600) / 60)).padStart(2, "0")
  return `${MONTHS[month - 1]} ${day}, ${year} ${hh}:${mm}`
}

export function shortId(id: string): string {
  return id.slice(0, 8)
}

export function isImagePath(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? ""
  return [
    "apng",
    "avif",
    "bmp",
    "gif",
    "ico",
    "jpg",
    "jpeg",
    "png",
    "svg",
    "webp",
  ].includes(ext)
}

// How DiffPanel should render a per-file sub-patch:
//   "image"  -> side-by-side before/after <img> preview (no textual hunks)
//   "binary" -> non-image binary blob; keep the "Binary files … differ" fallback
//   "text"   -> ordinary unified-diff hunks
export type PatchFileKind = "text" | "image" | "binary"

export function patchFileKind(file: PatchFile): PatchFileKind {
  if (isImagePath(file.newPath) || isImagePath(file.oldPath)) return "image"
  if (/^Binary files .* differ$/m.test(file.patch) || file.patch.includes("GIT binary patch"))
    return "binary"
  return "text"
}

// ---------------------------------------------------------------------------
// Add / Clone / Init repository dialog helpers.
// ---------------------------------------------------------------------------

/** Last path segment, for deriving a default repo display name from a directory path. */
export function pathBasename(path: string): string {
  const trimmed = path.trim().replace(/[/\\]+$/, "")
  const segments = trimmed.split(/[/\\]/)
  return segments[segments.length - 1] ?? ""
}

/** Derives a repo folder name from a clone URL, e.g. `git@host:org/repo.git` -> `repo`. */
export function repoNameFromCloneUrl(url: string): string {
  const trimmed = url.trim().replace(/[/\\]+$/, "")
  const lastSegment = trimmed.split(/[/\\:]/).pop() ?? ""
  return lastSegment.replace(/\.git$/i, "")
}

/** Joins a directory path and a folder name, tolerating a trailing slash on `dir`. */
export function joinRepoPath(dir: string, name: string): string {
  const trimmedDir = dir.trim().replace(/\/+$/, "")
  const trimmedName = name.trim().replace(/^\/+/, "")
  if (trimmedDir === "") return trimmedName
  if (trimmedName === "") return trimmedDir
  return `${trimmedDir}/${trimmedName}`
}

export function githubRepoUrl(remoteUrl: string): string | null {
  const trimmed = remoteUrl.trim().replace(/\.git$/, "")
  if (trimmed.startsWith("git@github.com:"))
    return `https://github.com/${trimmed.slice("git@github.com:".length)}`
  if (trimmed.startsWith("ssh://git@github.com/"))
    return `https://github.com/${trimmed.slice("ssh://git@github.com/".length)}`
  if (
    trimmed.startsWith("https://github.com/") ||
    trimmed.startsWith("http://github.com/")
  )
    return trimmed.replace(/^http:\/\//, "https://")
  return null
}
