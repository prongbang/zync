import { useState } from "react"
import { PanelLeftClose, PanelLeftOpen } from "lucide-react"

import { Avatar, AvatarFallback } from "@workspace/ui/components/avatar"
import { Button } from "@workspace/ui/components/button"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@workspace/ui/components/tooltip"
import { cn } from "@workspace/ui/lib/utils"

import type { RepositoryRecord } from "@/lib/types"

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
export function RepoMinibar({
  repos,
  activeId,
  onSelect,
}: {
  repos: RepositoryRecord[]
  activeId: string | null
  onSelect: (id: string) => void
}) {
  const [expanded, setExpanded] = useState(readExpanded)

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
                <Button
                  key={repo.id}
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
                </Button>
              )
            }
            return (
              <Tooltip key={repo.id}>
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
                      className="size-9 shrink-0"
                    />
                  }
                >
                  <Avatar className="size-7 rounded-md">
                    <AvatarFallback className="rounded-md">
                      {monogram(repo.name)}
                    </AvatarFallback>
                  </Avatar>
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
            )
          })}
        </div>
        <div
          className={cn(
            "border-border mt-auto flex shrink-0 border-t pt-2",
            expanded ? "justify-end px-2" : "justify-center",
          )}
        >
          <Button
            variant="ghost"
            size="icon"
            aria-label={expanded ? "Collapse projects" : "Expand projects"}
            onClick={toggleExpanded}
          >
            {expanded ? <PanelLeftClose /> : <PanelLeftOpen />}
          </Button>
        </div>
      </nav>
    </TooltipProvider>
  )
}

// First 1–2 alphanumeric chars of the repo name, uppercased.
function monogram(name: string): string {
  const cleaned = name.replace(/[^a-zA-Z0-9]/g, "")
  return (cleaned.slice(0, 2) || name.slice(0, 2) || "?").toUpperCase()
}
