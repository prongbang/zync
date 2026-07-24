import { useState } from "react"
import { Plus, PanelLeftClose, PanelLeftOpen, Star } from "lucide-react"

import { Avatar, AvatarFallback } from "@workspace/ui/components/avatar"
import { Button } from "@workspace/ui/components/button"
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@workspace/ui/components/context-menu"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@workspace/ui/components/tooltip"
import { cn } from "@workspace/ui/lib/utils"

import type { RepositoryRecord } from "@/lib/types"

import { ConfirmDialog } from "./dialogs"

const EXPANDED_STORAGE_KEY = "zync.repo-minibar.expanded"

function readExpanded(): boolean {
  try {
    return localStorage.getItem(EXPANDED_STORAGE_KEY) === "1"
  } catch {
    return false
  }
}

// Persistent far-left rail for switching between registered repositories —
// VS Code activity bar / Fork repo rail. Always visible on desktop (hidden on
// mobile, where the branches sheet lists repositories instead). Collapsed it
// shows monogram avatars with tooltips; expanded it shows avatar + repo name.
// A "+" trigger above the collapse toggle opens the Add/Clone/Init dialog,
// and each repo button carries a right-click context menu (open / favorite /
// remove) in both rail states.
export function RepoMinibar({
  repos,
  activeId,
  onSelect,
  onAddRepository,
  onToggleFavorite,
  onRemoveRepository,
}: {
  repos: RepositoryRecord[]
  activeId: string | null
  onSelect: (id: string) => void
  onAddRepository: () => void
  onToggleFavorite: (repo: RepositoryRecord) => void
  onRemoveRepository: (repo: RepositoryRecord) => void
}) {
  const [expanded, setExpanded] = useState(readExpanded)
  const [removeTarget, setRemoveTarget] = useState<RepositoryRecord | null>(
    null,
  )

  function toggleExpanded() {
    setExpanded((prev) => {
      const next = !prev
      try {
        localStorage.setItem(EXPANDED_STORAGE_KEY, next ? "1" : "0")
      } catch {
        // Persistence is best-effort only.
      }
      return next
    })
  }

  function repoMenu(repo: RepositoryRecord) {
    return (
      <ContextMenuContent data-testid="repo-context-menu">
        <ContextMenuGroup>
          <ContextMenuItem onClick={() => onSelect(repo.id)}>
            Open
          </ContextMenuItem>
          <ContextMenuItem onClick={() => onToggleFavorite(repo)}>
            {repo.favorite ? "Unfavorite" : "Favorite"}
          </ContextMenuItem>
        </ContextMenuGroup>
        <ContextMenuSeparator />
        <ContextMenuGroup>
          <ContextMenuItem
            variant="destructive"
            onClick={() => setRemoveTarget(repo)}
          >
            Remove from Zync&hellip;
          </ContextMenuItem>
        </ContextMenuGroup>
      </ContextMenuContent>
    )
  }

  return (
    <TooltipProvider>
      <nav
        aria-label="Repositories"
        className={cn(
          "bg-sidebar border-border hidden shrink-0 flex-col border-r py-2 md:flex",
          expanded ? "w-52" : "w-13",
        )}
      >
        <div
          className={cn(
            "flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto",
            expanded ? "px-2" : "items-center",
          )}
        >
          {repos.map((repo) => {
            const active = repo.id === activeId
            if (expanded) {
              return (
                <ContextMenu key={repo.id}>
                  <ContextMenuTrigger>
                    <Button
                      variant={active ? "secondary" : "ghost"}
                      data-testid="repo-minibar-item"
                      data-repo-id={repo.id}
                      aria-current={active ? "page" : undefined}
                      onClick={() => onSelect(repo.id)}
                      className="h-9 w-full shrink-0 justify-start px-1.5"
                    >
                      <Avatar className="size-7 rounded-md">
                        <AvatarFallback className="rounded-md">
                          {monogram(repo.name)}
                        </AvatarFallback>
                      </Avatar>
                      <span className="min-w-0 truncate">{repo.name}</span>
                      {repo.favorite && (
                        <Star className="text-muted-foreground ml-auto shrink-0 fill-current" />
                      )}
                    </Button>
                  </ContextMenuTrigger>
                  {repoMenu(repo)}
                </ContextMenu>
              )
            }
            return (
              <ContextMenu key={repo.id}>
                <ContextMenuTrigger>
                  <Tooltip>
                    <TooltipTrigger
                      render={
                        <Button
                          variant={active ? "secondary" : "ghost"}
                          size="icon"
                          data-testid="repo-minibar-item"
                          data-repo-id={repo.id}
                          aria-label={repo.name}
                          aria-current={active ? "page" : undefined}
                          onClick={() => onSelect(repo.id)}
                          className="relative size-9 shrink-0"
                        />
                      }
                    >
                      <Avatar className="size-7 rounded-md">
                        <AvatarFallback className="rounded-md">
                          {monogram(repo.name)}
                        </AvatarFallback>
                      </Avatar>
                      {repo.favorite && (
                        <Star className="text-muted-foreground absolute right-0.5 bottom-0.5 size-3 shrink-0 fill-current" />
                      )}
                    </TooltipTrigger>
                    <TooltipContent side="right">
                      <div className="flex flex-col">
                        <span className="font-medium">{repo.name}</span>
                        <span className="text-muted-foreground truncate">
                          {repo.path}
                        </span>
                      </div>
                    </TooltipContent>
                  </Tooltip>
                </ContextMenuTrigger>
                {repoMenu(repo)}
              </ContextMenu>
            )
          })}
        </div>
        <div
          className={cn(
            "border-border mt-auto flex shrink-0 flex-col gap-1 border-t pt-2",
            expanded ? "px-2" : "items-center",
          )}
        >
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size={expanded ? "sm" : "icon"}
                  data-testid="add-repo-btn"
                  aria-label="Add repository"
                  onClick={onAddRepository}
                  className={expanded ? "justify-start" : undefined}
                />
              }
            >
              <Plus data-icon={expanded ? "inline-start" : undefined} />
              {expanded && "Add repository"}
            </TooltipTrigger>
            {!expanded && <TooltipContent side="right">Add repository</TooltipContent>}
          </Tooltip>
          <div className={cn("flex", expanded ? "justify-end" : "justify-center")}>
            <Button
              variant="ghost"
              size="icon"
              aria-label={expanded ? "Collapse projects" : "Expand projects"}
              onClick={toggleExpanded}
            >
              {expanded ? <PanelLeftClose /> : <PanelLeftOpen />}
            </Button>
          </div>
        </div>
      </nav>
      {removeTarget !== null && (
        <ConfirmDialog
          open
          onOpenChange={(next) => !next && setRemoveTarget(null)}
          title="Remove from Zync"
          description="This only unregisters the repository from Zync — the files stay on disk and nothing is deleted."
          subject={removeTarget.path}
          confirmLabel="Remove"
          destructive
          testId="remove-repo-confirm"
          onConfirm={() => {
            onRemoveRepository(removeTarget)
            setRemoveTarget(null)
          }}
        />
      )}
    </TooltipProvider>
  )
}

// First 1–2 alphanumeric chars of the repo name, uppercased.
function monogram(name: string): string {
  const cleaned = name.replace(/[^a-zA-Z0-9]/g, "")
  return (cleaned.slice(0, 2) || name.slice(0, 2) || "?").toUpperCase()
}
