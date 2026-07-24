import { Avatar, AvatarFallback } from "@workspace/ui/components/avatar"
import { Button } from "@workspace/ui/components/button"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@workspace/ui/components/tooltip"

import type { RepositoryRecord } from "@/lib/types"

// Persistent far-left rail for switching between registered repositories —
// VS Code activity bar / Fork repo rail. Always visible, even for a single
// repo, so it reads as the project switcher.
export function RepoMinibar({
  repos,
  activeId,
  onSelect,
}: {
  repos: RepositoryRecord[]
  activeId: string | null
  onSelect: (id: string) => void
}) {
  return (
    <TooltipProvider>
      <nav
        aria-label="Repositories"
        className="bg-sidebar border-border flex w-13 shrink-0 flex-col items-center gap-1 overflow-y-auto border-r py-2"
      >
        {repos.map((repo) => {
          const active = repo.id === activeId
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
                  <AvatarFallback className="rounded-md text-[11px] font-semibold">
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
      </nav>
    </TooltipProvider>
  )
}

// First 1–2 alphanumeric chars of the repo name, uppercased.
function monogram(name: string): string {
  const cleaned = name.replace(/[^a-zA-Z0-9]/g, "")
  return (cleaned.slice(0, 2) || name.slice(0, 2) || "?").toUpperCase()
}
