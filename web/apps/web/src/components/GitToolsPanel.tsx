// React port of the "Files / Remotes / Submodules" and reflog sections of
// crates/ui/src/components/panels.rs (BasicGitToolsPanel /
// RepositoryToolsPanel), condensed into a compact tabbed surface. Purely
// presentational: each tab exposes a refresh action and a placeholder list
// area — the orchestrator wires real data (reflog entries, submodules, LFS
// tracking, remotes) into these tabs next. Rebuilt on shadcn Tabs + Card
// primitives per web/.agents/skills/shadcn/SKILL.md.

import type { ReactElement } from "react"
import { GitCommitHorizontal, HardDrive, Layers, RefreshCw, Server } from "lucide-react"

import { Button } from "@workspace/ui/components/button"
import { Card, CardContent, CardHeader, CardTitle } from "@workspace/ui/components/card"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@workspace/ui/components/empty"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@workspace/ui/components/tabs"

export type GitToolKind = "reflog" | "submodules" | "lfs" | "remotes"

export interface GitToolsPanelProps {
  onRefresh: (kind: GitToolKind) => void
}

interface ToolTabConfig {
  kind: GitToolKind
  label: string
  icon: typeof GitCommitHorizontal
  emptyTitle: string
  emptyDescription: string
}

const TOOL_TABS: ToolTabConfig[] = [
  {
    kind: "reflog",
    label: "Reflog",
    icon: GitCommitHorizontal,
    emptyTitle: "No reflog entries loaded",
    emptyDescription: "Refresh to load the reference log for this repository.",
  },
  {
    kind: "submodules",
    label: "Submodules",
    icon: Layers,
    emptyTitle: "No submodules loaded",
    emptyDescription: "Refresh to list this repository's submodules.",
  },
  {
    kind: "lfs",
    label: "LFS",
    icon: HardDrive,
    emptyTitle: "No LFS data loaded",
    emptyDescription: "Refresh to check Git LFS configuration and tracked patterns.",
  },
  {
    kind: "remotes",
    label: "Remotes",
    icon: Server,
    emptyTitle: "No remotes loaded",
    emptyDescription: "Refresh to list this repository's remotes.",
  },
]

export function GitToolsPanel({ onRefresh }: GitToolsPanelProps): ReactElement {
  return (
    <Card size="sm" data-testid="git-tools-panel">
      <CardHeader>
        <CardTitle>Git tools</CardTitle>
      </CardHeader>
      <CardContent>
        <Tabs defaultValue={TOOL_TABS[0].kind}>
          <TabsList>
            {TOOL_TABS.map((tab) => (
              <TabsTrigger key={tab.kind} value={tab.kind}>
                {tab.label}
              </TabsTrigger>
            ))}
          </TabsList>
          {TOOL_TABS.map((tab) => (
            <TabsContent key={tab.kind} value={tab.kind}>
              <ToolTabBody tab={tab} onRefresh={onRefresh} />
            </TabsContent>
          ))}
        </Tabs>
      </CardContent>
    </Card>
  )
}

function ToolTabBody({
  tab,
  onRefresh,
}: {
  tab: ToolTabConfig
  onRefresh: (kind: GitToolKind) => void
}): ReactElement {
  const Icon = tab.icon
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-end">
        <Button variant="outline" size="sm" onClick={() => onRefresh(tab.kind)}>
          <RefreshCw data-icon="inline-start" />
          Refresh
        </Button>
      </div>
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Icon />
          </EmptyMedia>
          <EmptyTitle>{tab.emptyTitle}</EmptyTitle>
          <EmptyDescription>{tab.emptyDescription}</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button variant="secondary" size="sm" onClick={() => onRefresh(tab.kind)}>
            <RefreshCw data-icon="inline-start" />
            Load {tab.label.toLowerCase()}
          </Button>
        </EmptyContent>
      </Empty>
    </div>
  )
}
