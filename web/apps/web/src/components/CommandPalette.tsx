// P2.3 — desktop-style command palette. Presentational: the parent (App) owns all
// workspace state and passes the data to list plus one callback per action. The
// palette itself only maps rows to shadcn Command primitives and closes on run.
//
// Filtering is Command's (cmdk's) built-in fuzzy filter over each item's value
// (its text content) + optional `keywords`; selection highlight is Command's
// own `data-selected` — never a hand-rolled active indicator.

import type { ReactElement } from "react"
import {
  Archive,
  ArrowDown,
  ArrowDownToLine,
  ArrowUp,
  FolderGit2,
  GitBranch,
  GitBranchPlus,
  GitCommitHorizontal,
  Keyboard,
  RefreshCw,
  Search,
  Tag,
} from "lucide-react"

import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@workspace/ui/components/command"

import { shortId } from "@/lib/format"
import type {
  BranchSummary,
  CommitSummary,
  PullMode,
  RepositoryRecord,
} from "@/lib/types"

export type CommandPaletteProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Whether a repository is currently open — gates the repo-scoped actions. */
  hasRepo: boolean
  repositories: RepositoryRecord[]
  activeRepoId: string | null
  branches: BranchSummary[]
  commits: CommitSummary[]
  onOpenRepository: (id: string) => void
  onCheckoutBranch: (name: string) => void
  onSelectCommit: (id: string) => void
  onFetch: () => void
  onFetchAll: () => void
  onPull: (mode: PullMode) => void
  onPush: () => void
  onStash: () => void
  onNewBranch: () => void
  onNewTag: () => void
  onFocusSearch: () => void
  onRefresh: () => void
  onShowShortcuts: () => void
}

// Only the most recent commits are worth listing; the full-history search lives
// behind the "Search commits" action (and the graph's own search bar).
const RECENT_COMMIT_LIMIT = 8

export function CommandPalette({
  open,
  onOpenChange,
  hasRepo,
  repositories,
  activeRepoId,
  branches,
  commits,
  onOpenRepository,
  onCheckoutBranch,
  onSelectCommit,
  onFetch,
  onFetchAll,
  onPull,
  onPush,
  onStash,
  onNewBranch,
  onNewTag,
  onFocusSearch,
  onRefresh,
  onShowShortcuts,
}: CommandPaletteProps): ReactElement {
  // Every item closes the palette before running its action.
  const run = (action: () => void) => () => {
    onOpenChange(false)
    action()
  }

  const recentCommits = commits.slice(0, RECENT_COMMIT_LIMIT)

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Command palette"
      description="Search repositories, branches, commits, and actions."
    >
      <div data-testid="command-palette">
        <CommandInput
          aria-label="Command palette search"
          placeholder="Type a command or search…"
        />
        <CommandList>
          <CommandEmpty>No results found.</CommandEmpty>

          <CommandGroup heading="Actions">
            <CommandItem
              data-testid="command-item"
              keywords={["remote", "sync", "download"]}
              disabled={!hasRepo}
              onSelect={run(onFetch)}
            >
              <ArrowDownToLine />
              Fetch
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["remote", "sync", "all remotes"]}
              disabled={!hasRepo}
              onSelect={run(onFetchAll)}
            >
              <ArrowDownToLine />
              Fetch all remotes
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["remote", "fast-forward", "ff"]}
              disabled={!hasRepo}
              onSelect={run(() => onPull("ff-only"))}
            >
              <ArrowDown />
              Pull (fast-forward only)
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["remote", "merge"]}
              disabled={!hasRepo}
              onSelect={run(() => onPull("merge"))}
            >
              <ArrowDown />
              Pull (merge)
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["remote", "rebase"]}
              disabled={!hasRepo}
              onSelect={run(() => onPull("rebase"))}
            >
              <ArrowDown />
              Pull (rebase)
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["remote", "publish", "upload"]}
              disabled={!hasRepo}
              onSelect={run(onPush)}
            >
              <ArrowUp />
              Push
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["wip", "shelve"]}
              disabled={!hasRepo}
              onSelect={run(onStash)}
            >
              <Archive />
              Stash changes
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["create branch", "checkout new"]}
              disabled={!hasRepo}
              onSelect={run(onNewBranch)}
            >
              <GitBranchPlus />
              New branch…
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["create tag", "release"]}
              disabled={!hasRepo}
              onSelect={run(onNewTag)}
            >
              <Tag />
              New tag…
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["find", "filter", "history"]}
              disabled={!hasRepo}
              onSelect={run(onFocusSearch)}
            >
              <Search />
              Search commits
              <CommandShortcut>⌘⇧F</CommandShortcut>
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["reload", "refetch"]}
              disabled={!hasRepo}
              onSelect={run(onRefresh)}
            >
              <RefreshCw />
              Refresh workspace
              <CommandShortcut>⌘R</CommandShortcut>
            </CommandItem>
            <CommandItem
              data-testid="command-item"
              keywords={["help", "cheat sheet", "keys"]}
              onSelect={run(onShowShortcuts)}
            >
              <Keyboard />
              Keyboard shortcuts
              <CommandShortcut>?</CommandShortcut>
            </CommandItem>
          </CommandGroup>

          {repositories.length > 0 && (
            <>
              <CommandSeparator />
              <CommandGroup heading="Repositories">
                {repositories.map((repo) => (
                  <CommandItem
                    key={repo.id}
                    data-testid="command-item"
                    value={`repo ${repo.name} ${repo.path}`}
                    disabled={repo.id === activeRepoId}
                    onSelect={run(() => onOpenRepository(repo.id))}
                  >
                    <FolderGit2 />
                    <span className="truncate">{repo.name}</span>
                    {repo.id === activeRepoId && (
                      <CommandShortcut>current</CommandShortcut>
                    )}
                  </CommandItem>
                ))}
              </CommandGroup>
            </>
          )}

          {branches.length > 0 && (
            <>
              <CommandSeparator />
              <CommandGroup heading="Checkout branch">
                {branches.map((branch) => (
                  <CommandItem
                    key={branch.name}
                    data-testid="command-item"
                    value={`branch ${branch.name}`}
                    disabled={branch.is_head}
                    onSelect={run(() => onCheckoutBranch(branch.name))}
                  >
                    <GitBranch />
                    <span className="truncate">{branch.name}</span>
                    {branch.is_head && (
                      <CommandShortcut>current</CommandShortcut>
                    )}
                  </CommandItem>
                ))}
              </CommandGroup>
            </>
          )}

          {recentCommits.length > 0 && (
            <>
              <CommandSeparator />
              <CommandGroup heading="Recent commits">
                {recentCommits.map((commit) => (
                  <CommandItem
                    key={commit.id}
                    data-testid="command-item"
                    value={`commit ${commit.id} ${commit.summary} ${commit.author}`}
                    onSelect={run(() => onSelectCommit(commit.id))}
                  >
                    <GitCommitHorizontal />
                    <code className="text-muted-foreground font-mono">
                      {shortId(commit.id)}
                    </code>
                    <span className="truncate">{commit.summary}</span>
                  </CommandItem>
                ))}
              </CommandGroup>
            </>
          )}
        </CommandList>
      </div>
    </CommandDialog>
  )
}
