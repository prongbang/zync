// React port of the "Files / Remotes / Submodules" and reflog sections of
// crates/ui/src/components/panels.rs, condensed into a compact tabbed surface.
//
// Every tab is self-contained (P0.6 / P0.8 / P2.1 / P2.2): each fetches its
// own data through the `zyncApi` singleton and refreshes itself after its own
// mutations, calling `onRefresh` afterward so the rest of the workspace
// (status/branches/etc.) picks up any side effect too. Built on shadcn Tabs +
// Card + Field + DropdownMenu primitives per web/.agents/skills/shadcn/SKILL.md.
//
// Server notes:
// - There are no rename / set-url remote endpoints, so "Rename" and "Edit
//   URL" are composites over add + delete (add-first for rename so a failure
//   never loses the remote; delete-then-add with rollback for edit-URL).
// - Reflog checkout/branch/reset reuse the same revision-scoped endpoints as
//   the commit graph's context menu (checkout/branches/reset), so results are
//   identical whichever surface triggers them.
// - Submodule init/update/sync are repo-wide (`git submodule <verb>
//   --recursive`), not scoped to one row — add/remove are the only per-row
//   mutations, and (per P2.2) had no server endpoint before this change.

import {
  useCallback,
  useEffect,
  useState,
  type ReactElement,
} from "react"
import {
  ArrowDown,
  ArrowDownToLine,
  ArrowUp,
  Download,
  GitBranchPlus,
  GitCommitHorizontal,
  HardDrive,
  KeyRound,
  Layers,
  LogIn,
  MoreHorizontal,
  Plus,
  PlayCircle,
  RefreshCw,
  RotateCcw,
  Server,
  Trash2,
  Users,
} from "lucide-react"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@workspace/ui/components/alert"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@workspace/ui/components/card"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@workspace/ui/components/dropdown-menu"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@workspace/ui/components/empty"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
} from "@workspace/ui/components/input-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@workspace/ui/components/select"
import { Spinner } from "@workspace/ui/components/spinner"
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@workspace/ui/components/tabs"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@workspace/ui/components/tooltip"

import { ApiError, zyncApi } from "@/lib/api"
import { formatCommitTime, shortId } from "@/lib/helpers"
import type {
  CredentialRecord,
  LfsSummary,
  ReflogEntrySummary,
  RemoteSummary,
  RepoMember,
  SubmoduleSummary,
} from "@/lib/types"

import {
  AddMemberDialog,
  MEMBER_ROLE_LABEL,
  type MemberRole,
} from "./dialogs/AddMemberDialog"
import { BranchAtRevisionDialog } from "./dialogs/BranchAtRevisionDialog"
import { ConfirmDialog } from "./dialogs/ConfirmDialog"
import { CredentialDialog } from "./dialogs/CredentialDialog"
import { LfsPushDialog } from "./dialogs/LfsPushDialog"
import { RemoteDialog } from "./dialogs/RemoteDialog"
import { ResetDialog } from "./dialogs/ResetDialog"
import { SubmoduleDialog } from "./dialogs/SubmoduleDialog"

export type GitToolKind = "reflog" | "submodules" | "lfs" | "remotes"

export interface GitToolsPanelProps {
  /**
   * Repository the Remotes tab operates on. Optional so existing call sites
   * keep compiling; without it the tab shows a "no repository" empty state.
   */
  repositoryId?: string | null
  onRefresh: (kind: GitToolKind) => void
  /**
   * Optional controlled active tab. Lets the header user menu deep-link to the
   * Credentials settings (P3.4). Omitted → the panel is uncontrolled and opens
   * on "remotes" as before.
   */
  tab?: string
  onTabChange?: (tab: string) => void
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function GitToolsPanel({
  repositoryId = null,
  onRefresh,
  tab,
  onTabChange,
}: GitToolsPanelProps): ReactElement {
  // Controlled only when the caller supplies `tab` (deep-link to Credentials);
  // otherwise fall back to the original uncontrolled "remotes" default.
  const tabsProps =
    tab !== undefined
      ? { value: tab, onValueChange: onTabChange }
      : { defaultValue: "remotes" }
  return (
    <Card size="sm" data-testid="git-tools-panel">
      <CardHeader>
        <CardTitle>Git tools</CardTitle>
      </CardHeader>
      <CardContent>
        <Tabs {...tabsProps}>
          <div className="scroll-fade-x overflow-x-auto">
            <TabsList>
              <TabsTrigger value="remotes">Remotes</TabsTrigger>
              <TabsTrigger value="members">Members</TabsTrigger>
              <TabsTrigger value="credentials">Credentials</TabsTrigger>
              <TabsTrigger value="reflog">Reflog</TabsTrigger>
              <TabsTrigger value="submodules">Submodules</TabsTrigger>
              <TabsTrigger value="lfs">LFS</TabsTrigger>
            </TabsList>
          </div>
          <TabsContent value="remotes">
            <RemotesTab
              repositoryId={repositoryId}
              onWorkspaceRefresh={() => onRefresh("remotes")}
            />
          </TabsContent>
          <TabsContent value="members">
            <MembersTab repositoryId={repositoryId} />
          </TabsContent>
          <TabsContent value="credentials">
            <CredentialsTab />
          </TabsContent>
          <TabsContent value="reflog">
            <ReflogTab
              repositoryId={repositoryId}
              onWorkspaceRefresh={() => onRefresh("reflog")}
            />
          </TabsContent>
          <TabsContent value="submodules">
            <SubmodulesTab
              repositoryId={repositoryId}
              onWorkspaceRefresh={() => onRefresh("submodules")}
            />
          </TabsContent>
          <TabsContent value="lfs">
            <LfsTab
              repositoryId={repositoryId}
              onWorkspaceRefresh={() => onRefresh("lfs")}
            />
          </TabsContent>
        </Tabs>
      </CardContent>
    </Card>
  )
}

// ---------------------------------------------------------------------------
// Remotes tab (P0.6)
// ---------------------------------------------------------------------------

type RemoteRowAction =
  | "add"
  | "fetch"
  | "pull"
  | "push"
  | "prune"
  | "edit-url"
  | "rename"
  | "delete"

type RemotesDialogState =
  | { kind: "add" }
  | { kind: "edit-url" | "rename"; remote: RemoteSummary }
  | { kind: "delete"; remote: RemoteSummary }
  | null

const REMOTE_QUICK_ACTIONS: {
  action: RemoteRowAction
  label: string
  icon: typeof ArrowDownToLine
}[] = [
  { action: "fetch", label: "Fetch", icon: ArrowDownToLine },
  { action: "pull", label: "Pull", icon: ArrowDown },
  { action: "push", label: "Push", icon: ArrowUp },
]

function RemotesTab({
  repositoryId,
  onWorkspaceRefresh,
}: {
  repositoryId: string | null
  onWorkspaceRefresh: () => void
}): ReactElement {
  const [remotes, setRemotes] = useState<RemoteSummary[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState<{
    remote: string
    action: RemoteRowAction
  } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [output, setOutput] = useState<string | null>(null)
  const [dialog, setDialog] = useState<RemotesDialogState>(null)

  const load = useCallback(async () => {
    if (!repositoryId) return
    setLoading(true)
    setError(null)
    try {
      setRemotes(await zyncApi.remotes(repositoryId))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setLoading(false)
    }
  }, [repositoryId])

  useEffect(() => {
    setRemotes(null)
    setOutput(null)
    setError(null)
    void load()
  }, [load])

  // Runs one remote operation with per-row busy state; errors surface in the
  // tab-level Alert (the api throws the server's raw error text verbatim).
  async function run(
    remote: string,
    action: RemoteRowAction,
    task: () => Promise<string | void>,
    options?: {
      reload?: boolean
      refreshWorkspace?: boolean
      success?: string
    },
  ) {
    setBusy({ remote, action })
    setError(null)
    setOutput(null)
    try {
      const result = await task()
      const text = typeof result === "string" ? result.trim() : ""
      setOutput(text !== "" ? text : (options?.success ?? "Done"))
      if (options?.reload) await load()
      if (options?.refreshWorkspace) onWorkspaceRefresh()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(null)
    }
  }

  if (!repositoryId) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Server />
          </EmptyMedia>
          <EmptyTitle>No repository connected</EmptyTitle>
          <EmptyDescription>
            Open a repository to manage its remotes.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  const quickAction = (remote: RemoteSummary, action: RemoteRowAction) => {
    switch (action) {
      case "fetch":
        return run(
          remote.name,
          "fetch",
          () => zyncApi.fetchRemote(repositoryId, remote.name),
          { refreshWorkspace: true, success: `Fetched ${remote.name}` },
        )
      case "pull":
        return run(
          remote.name,
          "pull",
          () => zyncApi.pullRemote(repositoryId, remote.name),
          { refreshWorkspace: true, success: `Pulled from ${remote.name}` },
        )
      case "push":
        return run(
          remote.name,
          "push",
          () => zyncApi.pushRemote(repositoryId, remote.name),
          { refreshWorkspace: true, success: `Pushed to ${remote.name}` },
        )
      default:
        return Promise.resolve()
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <Button
          data-testid="add-remote-btn"
          variant="outline"
          size="xs"
          disabled={busy !== null}
          onClick={() => setDialog({ kind: "add" })}
        >
          <Plus data-icon="inline-start" />
          Add remote
        </Button>
        <Button
          variant="ghost"
          size="xs"
          disabled={loading || busy !== null}
          onClick={() => void load()}
        >
          {loading ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <RefreshCw data-icon="inline-start" />
          )}
          Refresh
        </Button>
      </div>

      {error !== null && (
        <Alert variant="destructive">
          <AlertTitle>Remote operation failed</AlertTitle>
          <AlertDescription className="break-words">{error}</AlertDescription>
        </Alert>
      )}

      {remotes !== null && remotes.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Server />
            </EmptyMedia>
            <EmptyTitle>No remotes configured</EmptyTitle>
            <EmptyDescription>
              Add a remote to fetch, pull and push.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setDialog({ kind: "add" })}
            >
              <Plus data-icon="inline-start" />
              Add remote
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <TooltipProvider>
          <ul className="flex flex-col">
            {(remotes ?? []).map((remote) => {
              const rowBusyAction =
                busy !== null && busy.remote === remote.name
                  ? busy.action
                  : null
              return (
                <li
                  key={remote.name}
                  data-testid="remote-row"
                  data-remote={remote.name}
                  className="flex items-center gap-1.5 border-b py-1.5"
                >
                  <div className="min-w-0 flex-1">
                    <div className="text-xs font-medium">{remote.name}</div>
                    <code className="text-muted-foreground block truncate font-mono text-xs">
                      {remote.url ?? "no URL"}
                    </code>
                  </div>
                  {REMOTE_QUICK_ACTIONS.map(({ action, label, icon: Icon }) => (
                    <Tooltip key={action}>
                      <TooltipTrigger
                        render={
                          <Button
                            data-testid={`remote-${action}-btn`}
                            variant="ghost"
                            size="icon-xs"
                            aria-label={`${label} ${remote.name}`}
                            disabled={busy !== null}
                            onClick={() => void quickAction(remote, action)}
                          />
                        }
                      >
                        {rowBusyAction === action ? <Spinner /> : <Icon />}
                      </TooltipTrigger>
                      <TooltipContent>
                        {label} {remote.name}
                      </TooltipContent>
                    </Tooltip>
                  ))}
                  <DropdownMenu>
                    <DropdownMenuTrigger
                      render={
                        <Button
                          data-testid="remote-more-btn"
                          variant="ghost"
                          size="icon-xs"
                          aria-label={`More actions for ${remote.name}`}
                          disabled={busy !== null}
                        />
                      }
                    >
                      {rowBusyAction !== null &&
                      rowBusyAction !== "fetch" &&
                      rowBusyAction !== "pull" &&
                      rowBusyAction !== "push" ? (
                        <Spinner />
                      ) : (
                        <MoreHorizontal />
                      )}
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuGroup>
                        <DropdownMenuItem
                          onClick={() =>
                            void run(
                              remote.name,
                              "prune",
                              () =>
                                zyncApi.pruneRemote(repositoryId, remote.name),
                              {
                                refreshWorkspace: true,
                                success: `Pruned ${remote.name}`,
                              },
                            )
                          }
                        >
                          Prune
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          onClick={() =>
                            setDialog({ kind: "edit-url", remote })
                          }
                        >
                          Edit URL…
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          onClick={() => setDialog({ kind: "rename", remote })}
                        >
                          Rename…
                        </DropdownMenuItem>
                      </DropdownMenuGroup>
                      <DropdownMenuSeparator />
                      <DropdownMenuGroup>
                        <DropdownMenuItem
                          variant="destructive"
                          onClick={() => setDialog({ kind: "delete", remote })}
                        >
                          Delete…
                        </DropdownMenuItem>
                      </DropdownMenuGroup>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </li>
              )
            })}
          </ul>
        </TooltipProvider>
      )}

      {output !== null && (
        <p
          role="status"
          className="text-muted-foreground font-mono text-xs whitespace-pre-wrap"
        >
          {output}
        </p>
      )}

      {dialog?.kind === "add" && (
        <RemoteDialog
          open
          mode="add"
          onOpenChange={(open) => !open && setDialog(null)}
          onSubmit={({ name, url }) =>
            void run(
              name,
              "add",
              () => zyncApi.addRemote(repositoryId, name, url),
              {
                reload: true,
                refreshWorkspace: true,
                success: `Added remote ${name}`,
              },
            )
          }
        />
      )}
      {dialog?.kind === "edit-url" && (
        <RemoteDialog
          open
          mode="edit-url"
          remoteName={dialog.remote.name}
          remoteUrl={dialog.remote.url ?? ""}
          onOpenChange={(open) => !open && setDialog(null)}
          onSubmit={({ name, url }) => {
            const previousUrl = dialog.remote.url
            void run(
              name,
              "edit-url",
              async () => {
                // No set-url endpoint: re-add under the same name. If the
                // re-add fails, restore the previous URL so the remote is
                // never lost.
                await zyncApi.deleteRemote(repositoryId, name)
                try {
                  await zyncApi.addRemote(repositoryId, name, url)
                } catch (err) {
                  if (previousUrl) {
                    await zyncApi.addRemote(repositoryId, name, previousUrl)
                  }
                  throw err
                }
              },
              {
                reload: true,
                refreshWorkspace: true,
                success: `Updated URL for ${name}`,
              },
            )
          }}
        />
      )}
      {dialog?.kind === "rename" && (
        <RemoteDialog
          open
          mode="rename"
          remoteName={dialog.remote.name}
          remoteUrl={dialog.remote.url ?? ""}
          onOpenChange={(open) => !open && setDialog(null)}
          onSubmit={({ name }) => {
            const oldName = dialog.remote.name
            const url = dialog.remote.url
            void run(
              oldName,
              "rename",
              async () => {
                if (!url) {
                  throw new Error(
                    `cannot rename ${oldName}: it has no URL configured`,
                  )
                }
                // Add-first so a failure never loses the existing remote.
                await zyncApi.addRemote(repositoryId, name, url)
                await zyncApi.deleteRemote(repositoryId, oldName)
              },
              {
                reload: true,
                refreshWorkspace: true,
                success: `Renamed ${oldName} to ${name}`,
              },
            )
          }}
        />
      )}
      {dialog?.kind === "delete" && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setDialog(null)}
          title="Delete Remote"
          description="Removes the remote and its remote-tracking branches from this repository. The remote repository itself is not touched."
          subject={`${dialog.remote.name} — ${dialog.remote.url ?? "no URL"}`}
          confirmLabel="Delete Remote"
          destructive
          testId="delete-remote-dialog"
          onConfirm={() =>
            void run(
              dialog.remote.name,
              "delete",
              () => zyncApi.deleteRemote(repositoryId, dialog.remote.name),
              {
                reload: true,
                refreshWorkspace: true,
                success: `Deleted remote ${dialog.remote.name}`,
              },
            )
          }
        />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Members tab (P3.5)
// ---------------------------------------------------------------------------
//
// Owner/admin-only server-side (the repo-scope authz guard 403s the whole
// `/repositories/:id/members*` subtree for a member/viewer caller). A member
// or viewer opening this tab is an expected, non-error case, so a 403 on load
// degrades to a friendly "owner-only" empty state rather than a destructive
// Alert — everything else (add/role-change/remove failing mid-session, e.g. a
// role change racing a concurrent removal) still surfaces as an Alert.
//
// The repository's own `owner_id` can't be demoted or removed (the server
// returns 409 Conflict for both) — the member row carrying `role === "owner"`
// is that owner, so its role Select and remove button are disabled with an
// explanatory tooltip rather than left to fail server-side.

type MembersDialogState = { kind: "remove"; member: RepoMember } | null

function isProtectedOwner(member: RepoMember): boolean {
  return member.role === "owner"
}

function memberDisplayName(member: RepoMember): string {
  return member.name || member.email || member.user_id
}

function MembersTab({
  repositoryId,
}: {
  repositoryId: string | null
}): ReactElement {
  const [members, setMembers] = useState<RepoMember[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [restricted, setRestricted] = useState(false)
  const [busyUserId, setBusyUserId] = useState<string | null>(null)
  const [addOpen, setAddOpen] = useState(false)
  const [adding, setAdding] = useState(false)
  const [dialog, setDialog] = useState<MembersDialogState>(null)

  const load = useCallback(async () => {
    if (!repositoryId) return
    setLoading(true)
    setError(null)
    setRestricted(false)
    try {
      setMembers(await zyncApi.listMembers(repositoryId))
    } catch (err) {
      if (err instanceof ApiError && err.status === 403) {
        setMembers(null)
        setRestricted(true)
      } else {
        setError(errorText(err))
      }
    } finally {
      setLoading(false)
    }
  }, [repositoryId])

  useEffect(() => {
    setMembers(null)
    setError(null)
    setRestricted(false)
    void load()
  }, [load])

  async function addMember(identifier: string, role: MemberRole) {
    if (!repositoryId) return
    setAdding(true)
    setError(null)
    try {
      await zyncApi.addMember(repositoryId, identifier, role)
      await load()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setAdding(false)
    }
  }

  async function changeRole(member: RepoMember, role: string) {
    if (!repositoryId || role === member.role) return
    setBusyUserId(member.user_id)
    setError(null)
    try {
      await zyncApi.updateMemberRole(repositoryId, member.user_id, role)
      await load()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusyUserId(null)
    }
  }

  async function removeMember(member: RepoMember) {
    if (!repositoryId) return
    setBusyUserId(member.user_id)
    setError(null)
    try {
      await zyncApi.removeMember(repositoryId, member.user_id)
      await load()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusyUserId(null)
    }
  }

  if (!repositoryId) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Users />
          </EmptyMedia>
          <EmptyTitle>No repository connected</EmptyTitle>
          <EmptyDescription>
            Open a repository to manage its members.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  if (restricted) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Users />
          </EmptyMedia>
          <EmptyTitle>Members are owner-only</EmptyTitle>
          <EmptyDescription>
            Only the repository owner or an admin can view and manage
            members.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <Button
          data-testid="add-member-btn"
          variant="outline"
          size="xs"
          disabled={adding || busyUserId !== null}
          onClick={() => setAddOpen(true)}
        >
          {adding ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <Plus data-icon="inline-start" />
          )}
          Add member
        </Button>
        <Button
          variant="ghost"
          size="xs"
          disabled={loading || busyUserId !== null}
          onClick={() => void load()}
        >
          {loading ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <RefreshCw data-icon="inline-start" />
          )}
          Refresh
        </Button>
      </div>

      {error !== null && (
        <Alert variant="destructive">
          <AlertTitle>Member operation failed</AlertTitle>
          <AlertDescription className="break-words">{error}</AlertDescription>
        </Alert>
      )}

      {members !== null && members.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Users />
            </EmptyMedia>
            <EmptyTitle>No members yet</EmptyTitle>
            <EmptyDescription>
              Add an existing Zync user to grant them access.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setAddOpen(true)}
            >
              <Plus data-icon="inline-start" />
              Add member
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <TooltipProvider>
          <ul className="flex flex-col">
            {(members ?? []).map((member) => {
              const protectedOwner = isProtectedOwner(member)
              const rowBusy = busyUserId === member.user_id
              return (
                <li
                  key={member.user_id}
                  data-testid="member-row"
                  data-user-id={member.user_id}
                  className="flex items-center gap-1.5 border-b py-1.5"
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-xs font-medium">
                      {memberDisplayName(member)}
                    </div>
                    {member.email && (
                      <div className="text-muted-foreground truncate text-xs">
                        {member.email}
                      </div>
                    )}
                  </div>
                  {protectedOwner ? (
                    <Tooltip>
                      <TooltipTrigger
                        render={
                          <span>
                            <Select
                              value={member.role}
                              disabled
                              onValueChange={(value) => {
                                if (value) void changeRole(member, value)
                              }}
                            >
                              <SelectTrigger
                                data-testid="member-role-select"
                                aria-label={`Role for ${memberDisplayName(member)}`}
                                size="sm"
                                className="w-24 shrink-0"
                              >
                                <SelectValue>
                                  {(value: string) =>
                                    MEMBER_ROLE_LABEL[value as MemberRole] ??
                                    value
                                  }
                                </SelectValue>
                              </SelectTrigger>
                              <SelectContent>
                                <SelectGroup>
                                  {(
                                    Object.keys(
                                      MEMBER_ROLE_LABEL,
                                    ) as MemberRole[]
                                  ).map((r) => (
                                    <SelectItem key={r} value={r}>
                                      {MEMBER_ROLE_LABEL[r]}
                                    </SelectItem>
                                  ))}
                                </SelectGroup>
                              </SelectContent>
                            </Select>
                          </span>
                        }
                      />
                      <TooltipContent>
                        The repository owner&rsquo;s role can&rsquo;t be
                        changed here.
                      </TooltipContent>
                    </Tooltip>
                  ) : (
                    <Select
                      value={member.role}
                      disabled={rowBusy}
                      onValueChange={(value) => {
                        if (value) void changeRole(member, value)
                      }}
                    >
                      <SelectTrigger
                        data-testid="member-role-select"
                        aria-label={`Role for ${memberDisplayName(member)}`}
                        size="sm"
                        className="w-24 shrink-0"
                      >
                        <SelectValue>
                          {(value: string) =>
                            MEMBER_ROLE_LABEL[value as MemberRole] ?? value
                          }
                        </SelectValue>
                      </SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          {(
                            Object.keys(MEMBER_ROLE_LABEL) as MemberRole[]
                          ).map((r) => (
                            <SelectItem key={r} value={r}>
                              {MEMBER_ROLE_LABEL[r]}
                            </SelectItem>
                          ))}
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                  )}
                  <Tooltip>
                    <TooltipTrigger
                      render={
                        <Button
                          variant="ghost"
                          size="icon-xs"
                          aria-label={`Remove ${memberDisplayName(member)}`}
                          disabled={protectedOwner || rowBusy}
                          onClick={() => setDialog({ kind: "remove", member })}
                        />
                      }
                    >
                      {rowBusy ? <Spinner /> : <Trash2 />}
                    </TooltipTrigger>
                    <TooltipContent>
                      {protectedOwner
                        ? "The repository owner can't be removed."
                        : `Remove ${memberDisplayName(member)}`}
                    </TooltipContent>
                  </Tooltip>
                </li>
              )
            })}
          </ul>
        </TooltipProvider>
      )}

      <AddMemberDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        onSubmit={({ identifier, role }) => void addMember(identifier, role)}
      />
      {dialog?.kind === "remove" && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setDialog(null)}
          title="Remove Member"
          description="Revokes this user's access to the repository. They can be re-added later."
          subject={`${memberDisplayName(dialog.member)} — ${MEMBER_ROLE_LABEL[dialog.member.role as MemberRole] ?? dialog.member.role}`}
          confirmLabel="Remove Member"
          destructive
          testId="remove-member-dialog"
          onConfirm={() => void removeMember(dialog.member)}
        />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Credentials tab (P0.8)
// ---------------------------------------------------------------------------

function credentialKindLabel(kind: string): string {
  switch (kind) {
    case "https_token":
      return "HTTPS token"
    case "ssh_key":
      return "SSH key"
    default:
      return kind
  }
}

function CredentialsTab(): ReactElement {
  const [credentials, setCredentials] = useState<CredentialRecord[] | null>(
    null,
  )
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [addOpen, setAddOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<CredentialRecord | null>(
    null,
  )
  const [deletingId, setDeletingId] = useState<string | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      setCredentials(await zyncApi.listCredentials())
    } catch (err) {
      setError(errorText(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  async function deleteCredential(credential: CredentialRecord) {
    setDeletingId(credential.id)
    setError(null)
    try {
      await zyncApi.deleteCredential(credential.id)
      await load()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setDeletingId(null)
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <Button
          data-testid="add-credential-btn"
          variant="outline"
          size="xs"
          onClick={() => setAddOpen(true)}
        >
          <Plus data-icon="inline-start" />
          Add credential
        </Button>
        <Button
          variant="ghost"
          size="xs"
          disabled={loading}
          onClick={() => void load()}
        >
          {loading ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <RefreshCw data-icon="inline-start" />
          )}
          Refresh
        </Button>
      </div>

      {error !== null && (
        <Alert variant="destructive">
          <AlertTitle>Credential operation failed</AlertTitle>
          <AlertDescription className="break-words">{error}</AlertDescription>
        </Alert>
      )}

      {credentials !== null && credentials.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <KeyRound />
            </EmptyMedia>
            <EmptyTitle>No credentials saved</EmptyTitle>
            <EmptyDescription>
              Add an HTTPS token or SSH key for pushes and fetches that need
              authentication.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setAddOpen(true)}
            >
              <Plus data-icon="inline-start" />
              Add credential
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <ul className="flex flex-col">
          {(credentials ?? []).map((credential) => (
            <li
              key={credential.id}
              data-testid="credential-row"
              data-credential-id={credential.id}
              className="flex items-center gap-2 border-b py-1.5"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <span className="truncate text-xs font-medium">
                    {credential.label}
                  </span>
                  <Badge
                    variant={
                      credential.kind === "ssh_key" ? "outline" : "secondary"
                    }
                  >
                    {credentialKindLabel(credential.kind)}
                  </Badge>
                </div>
                <div className="text-muted-foreground truncate text-xs">
                  <code className="font-mono">{credential.host_pattern}</code>
                  {credential.username && <> · {credential.username}</>}
                  <> · added {credential.created_at.slice(0, 10)}</>
                </div>
              </div>
              <Button
                data-testid="delete-credential-btn"
                variant="ghost"
                size="icon-xs"
                aria-label={`Delete credential ${credential.label}`}
                disabled={deletingId !== null}
                onClick={() => setDeleteTarget(credential)}
              >
                {deletingId === credential.id ? <Spinner /> : <Trash2 />}
              </Button>
            </li>
          ))}
        </ul>
      )}

      <CredentialDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        onSubmit={async (request) => {
          await zyncApi.createCredential(request)
          await load()
        }}
      />
      {deleteTarget !== null && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setDeleteTarget(null)}
          title="Delete Credential"
          description="Deletes the stored secret permanently. Remotes that relied on it will fail to authenticate until a matching credential is added again."
          subject={`${deleteTarget.label} (${deleteTarget.host_pattern})`}
          confirmLabel="Delete Credential"
          destructive
          testId="delete-credential-dialog"
          onConfirm={() => void deleteCredential(deleteTarget)}
        />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Reflog tab (P2.1)
// ---------------------------------------------------------------------------

type ReflogRowAction = "checkout" | "branch" | "reset"

type ReflogDialogState =
  | { kind: "branch" | "reset"; entry: ReflogEntrySummary }
  | null

function ReflogTab({
  repositoryId,
  onWorkspaceRefresh,
}: {
  repositoryId: string | null
  onWorkspaceRefresh: () => void
}): ReactElement {
  const [entries, setEntries] = useState<ReflogEntrySummary[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState<{
    index: number
    action: ReflogRowAction
  } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [output, setOutput] = useState<string | null>(null)
  const [dialog, setDialog] = useState<ReflogDialogState>(null)

  const load = useCallback(async () => {
    if (!repositoryId) return
    setLoading(true)
    setError(null)
    try {
      setEntries(await zyncApi.reflog(repositoryId))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setLoading(false)
    }
  }, [repositoryId])

  useEffect(() => {
    setEntries(null)
    setOutput(null)
    setError(null)
    void load()
  }, [load])

  // A reflog action never reloads the reflog itself (the actions all move
  // HEAD/branches, they don't add a distinguishable entry worth re-fetching
  // for) — it just surfaces the result and asks the rest of the workspace to
  // refresh, mirroring the Remotes tab's `run` helper.
  async function run(
    index: number,
    action: ReflogRowAction,
    task: () => Promise<string | void>,
    success: string,
  ) {
    setBusy({ index, action })
    setError(null)
    setOutput(null)
    try {
      const result = await task()
      const text = typeof result === "string" ? result.trim() : ""
      setOutput(text !== "" ? text : success)
      onWorkspaceRefresh()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(null)
    }
  }

  if (!repositoryId) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <GitCommitHorizontal />
          </EmptyMedia>
          <EmptyTitle>No repository connected</EmptyTitle>
          <EmptyDescription>
            Open a repository to view its reflog.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-end">
        <Button
          variant="ghost"
          size="xs"
          disabled={loading || busy !== null}
          onClick={() => void load()}
        >
          {loading ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <RefreshCw data-icon="inline-start" />
          )}
          Refresh
        </Button>
      </div>

      {error !== null && (
        <Alert variant="destructive">
          <AlertTitle>Reflog operation failed</AlertTitle>
          <AlertDescription className="break-words">{error}</AlertDescription>
        </Alert>
      )}

      {entries !== null && entries.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <GitCommitHorizontal />
            </EmptyMedia>
            <EmptyTitle>No reflog entries</EmptyTitle>
            <EmptyDescription>
              HEAD has no recorded history yet.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ul className="flex flex-col">
          {(entries ?? []).map((entry) => {
            const rowBusyAction =
              busy !== null && busy.index === entry.index ? busy.action : null
            return (
              <li
                key={entry.index}
                data-testid="reflog-row"
                data-reflog-index={entry.index}
                className="flex items-center gap-1.5 border-b py-1.5"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1.5">
                    <code className="text-muted-foreground font-mono text-xs">
                      {shortId(entry.new_id)}
                    </code>
                    <span className="truncate text-xs">{entry.message}</span>
                  </div>
                  <div className="text-muted-foreground truncate text-xs">
                    {entry.committer} · {formatCommitTime(entry.time)}
                  </div>
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger
                    render={
                      <Button
                        data-testid="reflog-more-btn"
                        variant="ghost"
                        size="icon-xs"
                        aria-label={`Actions for reflog entry ${entry.index}`}
                        disabled={busy !== null}
                      />
                    }
                  >
                    {rowBusyAction !== null ? <Spinner /> : <MoreHorizontal />}
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuGroup>
                      <DropdownMenuItem
                        onClick={() =>
                          void run(
                            entry.index,
                            "checkout",
                            () =>
                              zyncApi.checkoutRevision(
                                repositoryId,
                                entry.new_id,
                              ),
                            `Checked out ${shortId(entry.new_id)}`,
                          )
                        }
                      >
                        <LogIn data-icon="inline-start" />
                        Checkout revision
                      </DropdownMenuItem>
                      <DropdownMenuItem
                        onClick={() =>
                          setDialog({ kind: "branch", entry })
                        }
                      >
                        <GitBranchPlus data-icon="inline-start" />
                        New branch here…
                      </DropdownMenuItem>
                    </DropdownMenuGroup>
                    <DropdownMenuSeparator />
                    <DropdownMenuGroup>
                      <DropdownMenuItem
                        variant="destructive"
                        onClick={() => setDialog({ kind: "reset", entry })}
                      >
                        <RotateCcw data-icon="inline-start" />
                        Reset here…
                      </DropdownMenuItem>
                    </DropdownMenuGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
              </li>
            )
          })}
        </ul>
      )}

      {output !== null && (
        <p
          role="status"
          className="text-muted-foreground font-mono text-xs whitespace-pre-wrap"
        >
          {output}
        </p>
      )}

      {dialog?.kind === "branch" && (
        <BranchAtRevisionDialog
          open
          revision={shortId(dialog.entry.new_id)}
          onOpenChange={(open) => !open && setDialog(null)}
          onSubmit={({ name, checkout }) => {
            const entry = dialog.entry
            void run(
              entry.index,
              "branch",
              () =>
                zyncApi.createBranchAt(
                  repositoryId,
                  name,
                  entry.new_id,
                  checkout,
                ),
              `Created branch ${name}`,
            )
          }}
        />
      )}
      {dialog?.kind === "reset" && (
        <ResetDialog
          open
          commit={shortId(dialog.entry.new_id)}
          onOpenChange={(open) => !open && setDialog(null)}
          onSubmit={({ mode }) => {
            const entry = dialog.entry
            void run(
              entry.index,
              "reset",
              () =>
                zyncApi.resetToRevision(
                  repositoryId,
                  entry.new_id,
                  mode === "hard",
                ),
              `Reset (${mode}) to ${shortId(entry.new_id)}`,
            )
          }}
        />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Submodules tab (P2.2)
// ---------------------------------------------------------------------------

type SubmoduleBulkAction = "init" | "update" | "sync" | "add"

function SubmodulesTab({
  repositoryId,
  onWorkspaceRefresh,
}: {
  repositoryId: string | null
  onWorkspaceRefresh: () => void
}): ReactElement {
  const [submodules, setSubmodules] = useState<SubmoduleSummary[] | null>(
    null,
  )
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState<SubmoduleBulkAction | null>(null)
  const [removingPath, setRemovingPath] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [output, setOutput] = useState<string | null>(null)
  const [addOpen, setAddOpen] = useState(false)
  const [removeTarget, setRemoveTarget] = useState<SubmoduleSummary | null>(
    null,
  )

  const load = useCallback(async () => {
    if (!repositoryId) return
    setLoading(true)
    setError(null)
    try {
      setSubmodules(await zyncApi.submodules(repositoryId))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setLoading(false)
    }
  }, [repositoryId])

  useEffect(() => {
    setSubmodules(null)
    setOutput(null)
    setError(null)
    void load()
  }, [load])

  async function run(action: SubmoduleBulkAction, task: () => Promise<string>) {
    setBusy(action)
    setError(null)
    setOutput(null)
    try {
      const result = await task()
      setOutput(result.trim() !== "" ? result : "Done")
      await load()
      onWorkspaceRefresh()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(null)
    }
  }

  async function removeSubmodule(target: SubmoduleSummary) {
    if (!repositoryId) return
    setRemovingPath(target.path)
    setError(null)
    setOutput(null)
    try {
      const result = await zyncApi.submoduleRemove(repositoryId, target.path)
      setOutput(result.trim() !== "" ? result : `Removed ${target.path}`)
      await load()
      onWorkspaceRefresh()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setRemovingPath(null)
    }
  }

  if (!repositoryId) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <Layers />
          </EmptyMedia>
          <EmptyTitle>No repository connected</EmptyTitle>
          <EmptyDescription>
            Open a repository to manage its submodules.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  const anyBusy = busy !== null || removingPath !== null

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <Button
          data-testid="add-submodule-btn"
          variant="outline"
          size="xs"
          disabled={anyBusy}
          onClick={() => setAddOpen(true)}
        >
          {busy === "add" ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <Plus data-icon="inline-start" />
          )}
          Add submodule
        </Button>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="xs"
            disabled={anyBusy}
            onClick={() =>
              void run("init", () => zyncApi.submoduleInit(repositoryId))
            }
          >
            {busy === "init" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <PlayCircle data-icon="inline-start" />
            )}
            Init
          </Button>
          <Button
            variant="ghost"
            size="xs"
            disabled={anyBusy}
            onClick={() =>
              void run("update", () => zyncApi.submoduleUpdate(repositoryId))
            }
          >
            {busy === "update" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <Download data-icon="inline-start" />
            )}
            Update
          </Button>
          <Button
            variant="ghost"
            size="xs"
            disabled={anyBusy}
            onClick={() =>
              void run("sync", () => zyncApi.submoduleSync(repositoryId))
            }
          >
            {busy === "sync" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <RefreshCw data-icon="inline-start" />
            )}
            Sync
          </Button>
          <Button
            variant="ghost"
            size="xs"
            disabled={loading || anyBusy}
            onClick={() => void load()}
          >
            {loading ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <RefreshCw data-icon="inline-start" />
            )}
            Refresh
          </Button>
        </div>
      </div>

      {error !== null && (
        <Alert variant="destructive">
          <AlertTitle>Submodule operation failed</AlertTitle>
          <AlertDescription className="break-words">{error}</AlertDescription>
        </Alert>
      )}

      {submodules !== null && submodules.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <Layers />
            </EmptyMedia>
            <EmptyTitle>No submodules</EmptyTitle>
            <EmptyDescription>
              Add a submodule to track another repository inside this one.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setAddOpen(true)}
            >
              <Plus data-icon="inline-start" />
              Add submodule
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <ul className="flex flex-col">
          {(submodules ?? []).map((submodule) => (
            <li
              key={submodule.path}
              data-testid="submodule-row"
              data-submodule-path={submodule.path}
              className="flex items-center gap-1.5 border-b py-1.5"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <span className="truncate text-xs font-medium">
                    {submodule.path}
                  </span>
                  {submodule.head && (
                    <Badge variant="outline">
                      <code className="font-mono">
                        {shortId(submodule.head)}
                      </code>
                    </Badge>
                  )}
                </div>
                <code className="text-muted-foreground block truncate font-mono text-xs">
                  {submodule.url ?? "no URL"}
                </code>
              </div>
              <Button
                data-testid="remove-submodule-btn"
                variant="ghost"
                size="icon-xs"
                aria-label={`Remove submodule ${submodule.path}`}
                disabled={anyBusy}
                onClick={() => setRemoveTarget(submodule)}
              >
                {removingPath === submodule.path ? <Spinner /> : <Trash2 />}
              </Button>
            </li>
          ))}
        </ul>
      )}

      {output !== null && (
        <p
          role="status"
          className="text-muted-foreground font-mono text-xs whitespace-pre-wrap"
        >
          {output}
        </p>
      )}

      <SubmoduleDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        onSubmit={({ url, path }) =>
          void run("add", () => zyncApi.submoduleAdd(repositoryId, url, path))
        }
      />
      {removeTarget !== null && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setRemoveTarget(null)}
          title="Remove Submodule"
          description="Deinitializes the submodule's working tree and removes it from .gitmodules. The submodule's own history and remote repository are not touched."
          subject={`${removeTarget.path} — ${removeTarget.url ?? "no URL"}`}
          confirmLabel="Remove Submodule"
          destructive
          testId="remove-submodule-dialog"
          onConfirm={() => void removeSubmodule(removeTarget)}
        />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// LFS tab (P2.2)
// ---------------------------------------------------------------------------

type LfsAction = "install" | "track" | "pull" | "push"

function LfsTab({
  repositoryId,
  onWorkspaceRefresh,
}: {
  repositoryId: string | null
  onWorkspaceRefresh: () => void
}): ReactElement {
  const [summary, setSummary] = useState<LfsSummary | null>(null)
  const [loading, setLoading] = useState(false)
  const [busy, setBusy] = useState<LfsAction | null>(null)
  const [untrackBusy, setUntrackBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [output, setOutput] = useState<string | null>(null)
  const [pattern, setPattern] = useState("")
  const [pushOpen, setPushOpen] = useState(false)
  const [untrackTarget, setUntrackTarget] = useState<string | null>(null)

  const load = useCallback(async () => {
    if (!repositoryId) return
    setLoading(true)
    setError(null)
    try {
      setSummary(await zyncApi.lfsSummary(repositoryId))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setLoading(false)
    }
  }, [repositoryId])

  useEffect(() => {
    setSummary(null)
    setOutput(null)
    setError(null)
    void load()
  }, [load])

  async function run(action: LfsAction, task: () => Promise<string>) {
    setBusy(action)
    setError(null)
    setOutput(null)
    try {
      const result = await task()
      setOutput(result.trim() !== "" ? result : "Done")
      await load()
      onWorkspaceRefresh()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(null)
    }
  }

  async function untrack(patternToRemove: string) {
    if (!repositoryId) return
    setUntrackBusy(patternToRemove)
    setError(null)
    setOutput(null)
    try {
      const result = await zyncApi.lfsUntrack(repositoryId, patternToRemove)
      setOutput(result.trim() !== "" ? result : `Untracked ${patternToRemove}`)
      await load()
      onWorkspaceRefresh()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setUntrackBusy(null)
    }
  }

  if (!repositoryId) {
    return (
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <HardDrive />
          </EmptyMedia>
          <EmptyTitle>No repository connected</EmptyTitle>
          <EmptyDescription>
            Open a repository to manage Git LFS.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  }

  const anyBusy = busy !== null || untrackBusy !== null
  const trimmedPattern = pattern.trim()

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <Button
          variant="outline"
          size="xs"
          disabled={anyBusy}
          onClick={() =>
            void run("install", () => zyncApi.lfsInstall(repositoryId))
          }
        >
          {busy === "install" ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <PlayCircle data-icon="inline-start" />
          )}
          Install LFS
        </Button>
        <Button
          variant="ghost"
          size="xs"
          disabled={loading || anyBusy}
          onClick={() => void load()}
        >
          {loading ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <RefreshCw data-icon="inline-start" />
          )}
          Refresh
        </Button>
      </div>

      {error !== null && (
        <Alert variant="destructive">
          <AlertTitle>LFS operation failed</AlertTitle>
          <AlertDescription className="break-words">{error}</AlertDescription>
        </Alert>
      )}

      {summary !== null && !summary.configured && (
        <Alert>
          <AlertTitle>Git LFS is not configured</AlertTitle>
          <AlertDescription>
            No patterns are tracked in .gitattributes yet. Install LFS and
            track a pattern below (e.g. *.psd) to get started.
          </AlertDescription>
        </Alert>
      )}

      <InputGroup>
        <InputGroupInput
          data-testid="lfs-pattern-input"
          placeholder="*.psd"
          aria-label="LFS pattern to track"
          value={pattern}
          disabled={anyBusy}
          onChange={(event) => setPattern(event.target.value)}
        />
        <InputGroupAddon align="inline-end">
          <InputGroupButton
            data-testid="lfs-track-btn"
            size="xs"
            disabled={anyBusy || trimmedPattern === ""}
            onClick={() =>
              void run("track", () =>
                zyncApi.lfsTrack(repositoryId, trimmedPattern).then((result) => {
                  setPattern("")
                  return result
                }),
              )
            }
          >
            {busy === "track" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <Plus data-icon="inline-start" />
            )}
            Track
          </InputGroupButton>
        </InputGroupAddon>
      </InputGroup>

      {summary !== null && summary.tracked_patterns.length === 0 ? (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HardDrive />
            </EmptyMedia>
            <EmptyTitle>No tracked patterns</EmptyTitle>
            <EmptyDescription>
              Track a file pattern above to start storing matching files in
              LFS.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <ul className="flex flex-col">
          {(summary?.tracked_patterns ?? []).map((trackedPattern) => (
            <li
              key={trackedPattern}
              data-testid="lfs-pattern-row"
              data-pattern={trackedPattern}
              className="flex items-center gap-1.5 border-b py-1.5"
            >
              <code className="min-w-0 flex-1 truncate font-mono text-xs">
                {trackedPattern}
              </code>
              <Button
                data-testid="lfs-untrack-btn"
                variant="ghost"
                size="icon-xs"
                aria-label={`Untrack ${trackedPattern}`}
                disabled={anyBusy}
                onClick={() => setUntrackTarget(trackedPattern)}
              >
                {untrackBusy === trackedPattern ? <Spinner /> : <Trash2 />}
              </Button>
            </li>
          ))}
        </ul>
      )}

      <div className="flex items-center gap-1.5">
        <Button
          variant="outline"
          size="xs"
          disabled={anyBusy}
          onClick={() => void run("pull", () => zyncApi.lfsPull(repositoryId))}
        >
          {busy === "pull" ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <ArrowDown data-icon="inline-start" />
          )}
          Pull
        </Button>
        <Button
          variant="outline"
          size="xs"
          disabled={anyBusy}
          onClick={() => setPushOpen(true)}
        >
          {busy === "push" ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <ArrowUp data-icon="inline-start" />
          )}
          Push
        </Button>
      </div>

      {output !== null && (
        <p
          role="status"
          className="text-muted-foreground font-mono text-xs whitespace-pre-wrap"
        >
          {output}
        </p>
      )}

      <LfsPushDialog
        open={pushOpen}
        onOpenChange={setPushOpen}
        onSubmit={({ remote, branch }) =>
          void run("push", () => zyncApi.lfsPush(repositoryId, remote, branch))
        }
      />
      {untrackTarget !== null && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setUntrackTarget(null)}
          title="Untrack Pattern"
          description={`Stop tracking ${untrackTarget} with Git LFS?`}
          subject={untrackTarget}
          confirmLabel="Untrack"
          destructive
          testId="untrack-lfs-pattern-dialog"
          onConfirm={() => void untrack(untrackTarget)}
        />
      )}
    </div>
  )
}
