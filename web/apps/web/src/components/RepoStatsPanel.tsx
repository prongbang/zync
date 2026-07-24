// React port of crates/ui/src/components/detail.rs (RepoStatsPanel +
// RepoStatsChart). Presentational only: the parent fetches/owns `stats` and
// passes it down (see App.tsx). Neutral shadcn Card primitives; bars use the
// functional chart-1 data-viz token, per web/.agents/skills/shadcn/SKILL.md.

import type { ReactElement } from "react"

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@workspace/ui/components/card"
import { Skeleton } from "@workspace/ui/components/skeleton"
import { cn } from "@workspace/ui/lib/utils"

import { formatCommitTime } from "@/lib/format"
import type { AuthorStat, MonthStat, RepoStats } from "@/lib/types"

export interface RepoStatsPanelProps {
  stats: RepoStats | null
}

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

export function RepoStatsPanel({ stats }: RepoStatsPanelProps): ReactElement {
  if (!stats) {
    return <RepoStatsLoading />
  }

  return (
    <div className="flex flex-col gap-4" data-testid="repo-stats">
      <div className="grid grid-cols-2 gap-3 @xl:grid-cols-4">
        <StatCard label="Commits" value={String(stats.commit_count)} />
        <StatCard label="Contributors" value={String(stats.contributors.length)} />
        <StatCard label="First commit" value={formatCommitTime(stats.first_commit_time)} />
        <StatCard label="Last commit" value={formatCommitTime(stats.last_commit_time)} />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Commits per month</CardTitle>
        </CardHeader>
        <CardContent>
          <MonthlyChart monthly={stats.monthly} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Top contributors</CardTitle>
        </CardHeader>
        <CardContent>
          <ContributorBars contributors={stats.contributors} />
        </CardContent>
      </Card>
    </div>
  )
}

function StatCard({ label, value }: { label: string; value: string }): ReactElement {
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription className="tracking-wide uppercase">
          {label}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="truncate text-lg font-semibold text-foreground">{value}</div>
      </CardContent>
    </Card>
  )
}

const CHART_WIDTH = 640
const CHART_HEIGHT = 170
const BASELINE = 148

function MonthlyChart({ monthly }: { monthly: MonthStat[] }): ReactElement {
  if (monthly.length === 0) {
    return <p className="text-sm text-muted-foreground">No commits in range.</p>
  }

  const maxTotal = Math.max(1, ...monthly.map((month) => month.total))
  const count = Math.max(1, monthly.length)
  const slot = CHART_WIDTH / count
  const barWidth = Math.min(42, Math.max(1, slot - 3))
  const barInset = (slot - barWidth) / 2
  const labelStep = Math.max(1, Math.ceil(count / 8))

  return (
    <svg
      viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}
      className="h-[170px] w-full"
      role="img"
      aria-label="Commits per month"
    >
      <line
        x1={0}
        y1={BASELINE}
        x2={CHART_WIDTH}
        y2={BASELINE}
        stroke="var(--border)"
        strokeWidth={1}
      />
      {monthly.map((month, index) => {
        const barHeight = Math.max(1.5, (month.total / maxTotal) * (BASELINE - 12))
        const x = index * slot + barInset
        const y = BASELINE - barHeight
        const monthName = MONTHS[Math.min(11, Math.max(0, month.month - 1))]
        return (
          <rect
            key={`${month.year}-${month.month}`}
            x={x}
            y={y}
            width={barWidth}
            height={barHeight}
            rx={1.5}
            fill="var(--chart-1)"
            opacity={0.85}
          >
            <title>
              {monthName} {month.year}: {month.total} commit(s)
            </title>
          </rect>
        )
      })}
      {monthly.map((month, index) => {
        if (index % labelStep !== 0) return null
        const monthName = MONTHS[Math.min(11, Math.max(0, month.month - 1))]
        return (
          <text
            key={`${month.year}-${month.month}-label`}
            x={index * slot + slot / 2}
            y={CHART_HEIGHT - 6}
            fill="var(--muted-foreground)"
            fontSize={9}
            textAnchor="middle"
          >
            {monthName} {month.year % 100}
          </text>
        )
      })}
    </svg>
  )
}

function ContributorBars({ contributors }: { contributors: AuthorStat[] }): ReactElement {
  if (contributors.length === 0) {
    return <p className="text-sm text-muted-foreground">No contributors yet.</p>
  }

  const max = Math.max(1, contributors[0]?.commits ?? 1)

  return (
    <div className="flex flex-col gap-2">
      {contributors.slice(0, 8).map((author) => {
        const width = Math.max(2, (author.commits / max) * 100)
        return (
          <div key={author.name} className="flex items-center gap-3">
            <span className="w-28 shrink-0 truncate text-sm text-foreground">{author.name}</span>
            <div className="h-2 min-w-0 flex-1 rounded-full bg-muted">
              <div
                className="bg-chart-1 h-2 rounded-full"
                style={{ width: `${width}%` }}
              />
            </div>
            <span className="w-8 shrink-0 text-right text-sm text-muted-foreground">
              {author.commits}
            </span>
          </div>
        )
      })}
    </div>
  )
}

function RepoStatsLoading(): ReactElement {
  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 @xl:grid-cols-4">
        {[0, 1, 2, 3].map((index) => (
          <Card key={index} size="sm">
            <CardHeader>
              <Skeleton className="h-3 w-16" />
            </CardHeader>
            <CardContent>
              <Skeleton className="h-5 w-20" />
            </CardContent>
          </Card>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Commits per month</CardTitle>
        </CardHeader>
        <CardContent>
          <Skeleton className="h-[170px] w-full" />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Top contributors</CardTitle>
        </CardHeader>
        <CardContent className={cn("flex flex-col gap-2")}>
          {[0, 1, 2].map((index) => (
            <Skeleton key={index} className="h-2 w-full" />
          ))}
        </CardContent>
      </Card>
    </div>
  )
}
