// React port of the "Files / Remotes / Submodules" and reflog sections of
// crates/ui/src/components/panels.rs, condensed into a compact tabbed surface.
//
// The Remotes and Credentials tabs are live (P0.6 / P0.8): they fetch their
// own data through the `zyncApi` singleton and refresh themselves after their
// own mutations, so the panel stays self-contained. Reflog / Submodules / LFS
// remain placeholder tabs wired to `onRefresh` until the orchestrator ports
// them. Built on shadcn Tabs + Card + Field + DropdownMenu primitives per
// web/.agents/skills/shadcn/SKILL.md.
//
// Server note: there are no rename / set-url remote endpoints, so "Rename"
// and "Edit URL" are composites over add + delete (add-first for rename so a
// failure never loses the remote; delete-then-add with rollback for edit-URL).

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
  GitCommitHorizontal,
  HardDrive,
  KeyRound,
  Layers,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Server,
  Trash2,
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

import { zyncApi } from "@/lib/api"
import type { CredentialRecord, RemoteSummary } from "@/lib/types"

import { ConfirmDialog } from "./dialogs/ConfirmDialog"
import { CredentialDialog } from "./dialogs/CredentialDialog"
import { RemoteDialog } from "./dialogs/RemoteDialog"

export type GitToolKind = "reflog" | "submodules" | "lfs" | "remotes"

export interface GitToolsPanelProps {
  /**
   * Repository the Remotes tab operates on. Optional so existing call sites
   * keep compiling; without it the tab shows a "no repository" empty state.
   */
  repositoryId?: string | null
  onRefresh: (kind: GitToolKind) => void
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function GitToolsPanel({
  repositoryId = null,
  onRefresh,
}: GitToolsPanelProps): ReactElement {
  return (
    <Card size="sm" data-testid="git-tools-panel">
      <CardHeader>
        <CardTitle>Git tools</CardTitle>
      </CardHeader>
      <CardContent>
        <Tabs defaultValue="remotes">
          <div className="scroll-fade-x overflow-x-auto">
            <TabsList>
              <TabsTrigger value="remotes">Remotes</TabsTrigger>
              <TabsTrigger value="credentials">Credentials</TabsTrigger>
              {PLACEHOLDER_TABS.map((tab) => (
                <TabsTrigger key={tab.kind} value={tab.kind}>
                  {tab.label}
                </TabsTrigger>
              ))}
            </TabsList>
          </div>
          <TabsContent value="remotes">
            <RemotesTab
              repositoryId={repositoryId}
              onWorkspaceRefresh={() => onRefresh("remotes")}
            />
          </TabsContent>
          <TabsContent value="credentials">
            <CredentialsTab />
          </TabsContent>
          {PLACEHOLDER_TABS.map((tab) => (
            <TabsContent key={tab.kind} value={tab.kind}>
              <PlaceholderTabBody tab={tab} onRefresh={onRefresh} />
            </TabsContent>
          ))}
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
// Placeholder tabs (reflog / submodules / LFS) — unchanged behavior.
// ---------------------------------------------------------------------------

interface PlaceholderTabConfig {
  kind: GitToolKind
  label: string
  icon: typeof GitCommitHorizontal
  emptyTitle: string
  emptyDescription: string
}

const PLACEHOLDER_TABS: PlaceholderTabConfig[] = [
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
    emptyDescription:
      "Refresh to check Git LFS configuration and tracked patterns.",
  },
]

function PlaceholderTabBody({
  tab,
  onRefresh,
}: {
  tab: PlaceholderTabConfig
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
          <Button
            variant="secondary"
            size="sm"
            onClick={() => onRefresh(tab.kind)}
          >
            <RefreshCw data-icon="inline-start" />
            Load {tab.label.toLowerCase()}
          </Button>
        </EmptyContent>
      </Empty>
    </div>
  )
}
