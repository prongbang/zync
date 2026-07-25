// Add / Clone / Init repository dialog (RepoMinibar "+" trigger and the
// zero-repositories empty state). Three modes share one dialog via Tabs:
//
//  - "add"   opens an existing repository already on disk.
//  - "clone" clones a remote URL into a new destination folder.
//  - "init"  runs `git init` at a new destination folder (no commit).
//
// The directory browser (`DirectoryPicker`) is shared by all three modes: it
// wraps `api.directories()`, drilling down on row click and resolving an
// out-of-date/relative path to the server's canonical `current_path` on
// every fetch. For clone/init the browsed path is the *parent* folder; it is
// joined with a separate folder-name field to form the final target path.
//
// The actual `createRepository` call is owned by the caller (`onCreate`), to
// match the RemoteDialog/CredentialDialog pattern: this component stays
// presentational, awaits the caller's promise, and closes itself on success.

import { useEffect, useState, type ReactElement } from "react"
import { ArrowUp, Folder } from "lucide-react"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@workspace/ui/components/alert"
import { Button } from "@workspace/ui/components/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@workspace/ui/components/dialog"
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"
import { ScrollArea } from "@workspace/ui/components/scroll-area"
import { Spinner } from "@workspace/ui/components/spinner"
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@workspace/ui/components/tabs"

import { zyncApi } from "@/lib/api"
import { joinRepoPath, pathBasename, repoNameFromCloneUrl } from "@/lib/helpers"
import type { CreateRepositoryRequest, DirectoryList } from "@/lib/types"

type Mode = "add" | "clone" | "init"

export function AddRepositoryDialog({
  open,
  onOpenChange,
  onCreate,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Resolves with the new repository's id on success (dialog closes itself); throws the
   * server's raw error text on failure. */
  onCreate: (request: CreateRepositoryRequest) => Promise<string>
}): ReactElement {
  const [mode, setMode] = useState<Mode>("add")

  // "add" mode: the browsed path *is* the repository path.
  const [addPath, setAddPath] = useState("")
  const [addName, setAddName] = useState("")
  const [addNameTouched, setAddNameTouched] = useState(false)

  // "clone" mode: browsed path is the destination *parent*; joined with a name.
  const [cloneUrl, setCloneUrl] = useState("")
  const [cloneDest, setCloneDest] = useState("")
  const [cloneName, setCloneName] = useState("")
  const [cloneNameTouched, setCloneNameTouched] = useState(false)

  // "init" mode: same shape as clone, minus the URL.
  const [initDest, setInitDest] = useState("")
  const [initName, setInitName] = useState("")

  // Whether each DirectoryPicker's own browse fetch is currently failing — surfaced on the
  // wrapping `Field` as `data-invalid` per forms.md (the picker's Input carries the matching
  // `aria-invalid` itself).
  const [addPathInvalid, setAddPathInvalid] = useState(false)
  const [cloneDestInvalid, setCloneDestInvalid] = useState(false)
  const [initDestInvalid, setInitDestInvalid] = useState(false)

  const [submitting, setSubmitting] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)

  useEffect(() => {
    if (!open) return
    setMode("add")
    setAddPath("")
    setAddName("")
    setAddNameTouched(false)
    setCloneUrl("")
    setCloneDest("")
    setCloneName("")
    setCloneNameTouched(false)
    setInitDest("")
    setInitName("")
    setAddPathInvalid(false)
    setCloneDestInvalid(false)
    setInitDestInvalid(false)
    setSubmitting(false)
    setFormError(null)
  }, [open])

  const clonePath = joinRepoPath(cloneDest, cloneName)
  const initPath = joinRepoPath(initDest, initName)

  const canSubmit =
    !submitting &&
    (mode === "add"
      ? addPath.trim() !== ""
      : mode === "clone"
        ? cloneUrl.trim() !== "" && cloneDest.trim() !== "" && cloneName.trim() !== ""
        : initDest.trim() !== "" && initName.trim() !== "")

  const submit = async () => {
    if (!canSubmit) return
    setFormError(null)
    setSubmitting(true)
    try {
      const request: CreateRepositoryRequest =
        mode === "add"
          ? {
              name: addName.trim() || null,
              path: addPath.trim(),
              remote_url: null,
              clone_to: null,
            }
          : mode === "clone"
            ? {
                name: cloneName.trim() || null,
                path: null,
                remote_url: cloneUrl.trim(),
                clone_to: clonePath,
              }
            : {
                name: initName.trim() || null,
                path: initPath,
                remote_url: null,
                clone_to: null,
                init: true,
              }
      await onCreate(request)
      onOpenChange(false)
    } catch (error) {
      setFormError(error instanceof Error ? error.message : String(error))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !submitting && onOpenChange(next)}>
      <DialogContent data-testid="add-repository-dialog">
        <DialogHeader>
          <DialogTitle>Add Repository</DialogTitle>
          <DialogDescription>
            Open an existing repository, clone one from a URL, or start a new
            one.
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}
        >
          <Tabs value={mode} onValueChange={(value) => setMode(value as Mode)}>
            <TabsList className="w-full">
              <TabsTrigger value="add" data-testid="add-repo-tab-add">
                Add Existing
              </TabsTrigger>
              <TabsTrigger value="clone" data-testid="add-repo-tab-clone">
                Clone
              </TabsTrigger>
              <TabsTrigger value="init" data-testid="add-repo-tab-init">
                Init
              </TabsTrigger>
            </TabsList>

            <TabsContent value="add" className="pt-3">
              <FieldGroup>
                <Field data-invalid={addPathInvalid || undefined}>
                  <FieldLabel htmlFor="add-repo-path">Path</FieldLabel>
                  <DirectoryPicker
                    id="add-repo-path"
                    value={addPath}
                    autoFocus
                    onValidityChange={setAddPathInvalid}
                    onChange={(next) => {
                      setAddPath(next)
                      if (!addNameTouched) setAddName(pathBasename(next))
                    }}
                  />
                  <FieldDescription>
                    Browse to and select the folder that contains the
                    repository.
                  </FieldDescription>
                </Field>
                <Field>
                  <FieldLabel htmlFor="add-repo-name">Name</FieldLabel>
                  <Input
                    id="add-repo-name"
                    placeholder="Repository"
                    value={addName}
                    onChange={(event) => {
                      setAddNameTouched(true)
                      setAddName(event.target.value)
                    }}
                  />
                </Field>
              </FieldGroup>
            </TabsContent>

            <TabsContent value="clone" className="pt-3">
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="clone-repo-url">Repository URL</FieldLabel>
                  <Input
                    id="clone-repo-url"
                    autoFocus
                    spellCheck={false}
                    placeholder="git@github.com:owner/repo.git"
                    value={cloneUrl}
                    onChange={(event) => {
                      const value = event.target.value
                      setCloneUrl(value)
                      if (!cloneNameTouched) setCloneName(repoNameFromCloneUrl(value))
                    }}
                  />
                  <FieldDescription>
                    Uses saved credentials for the URL&rsquo;s host — manage
                    them in Git Tools &rarr; Credentials.
                  </FieldDescription>
                </Field>
                <Field data-invalid={cloneDestInvalid || undefined}>
                  <FieldLabel htmlFor="clone-repo-dest">
                    Destination folder
                  </FieldLabel>
                  <DirectoryPicker
                    id="clone-repo-dest"
                    value={cloneDest}
                    onValidityChange={setCloneDestInvalid}
                    onChange={setCloneDest}
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="clone-repo-name">Folder name</FieldLabel>
                  <Input
                    id="clone-repo-name"
                    value={cloneName}
                    onChange={(event) => {
                      setCloneNameTouched(true)
                      setCloneName(event.target.value)
                    }}
                  />
                  <FieldDescription className="truncate font-mono">
                    {clonePath || "Choose a destination and folder name"}
                  </FieldDescription>
                </Field>
              </FieldGroup>
            </TabsContent>

            <TabsContent value="init" className="pt-3">
              <FieldGroup>
                <Field data-invalid={initDestInvalid || undefined}>
                  <FieldLabel htmlFor="init-repo-dest">Location</FieldLabel>
                  <DirectoryPicker
                    id="init-repo-dest"
                    value={initDest}
                    autoFocus
                    onValidityChange={setInitDestInvalid}
                    onChange={setInitDest}
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="init-repo-name">
                    Repository name
                  </FieldLabel>
                  <Input
                    id="init-repo-name"
                    placeholder="my-new-repo"
                    value={initName}
                    onChange={(event) => setInitName(event.target.value)}
                  />
                  <FieldDescription className="truncate font-mono">
                    {initPath || "Choose a location and repository name"}
                  </FieldDescription>
                </Field>
              </FieldGroup>
            </TabsContent>
          </Tabs>

          {submitting && (
            <p
              className="text-muted-foreground mt-3 flex items-center gap-2 text-xs"
              data-testid="add-repo-progress"
            >
              <Spinner />
              {mode === "clone"
                ? "Cloning repository… this can take a while for large repos."
                : mode === "init"
                  ? "Creating repository…"
                  : "Opening repository…"}
            </p>
          )}

          {formError !== null && (
            <Alert variant="destructive" className="mt-3">
              <AlertTitle>Could not add repository</AlertTitle>
              <AlertDescription className="break-words">
                {formError}
              </AlertDescription>
            </Alert>
          )}

          <DialogFooter className="mt-6">
            <DialogClose
              data-testid="dialog-cancel"
              render={<Button variant="outline" type="button" disabled={submitting} />}
            >
              Cancel
            </DialogClose>
            <Button data-testid="dialog-submit" type="submit" disabled={!canSubmit}>
              {submitting && <Spinner data-icon="inline-start" />}
              {mode === "add"
                ? "Add Repository"
                : mode === "clone"
                  ? "Clone Repository"
                  : "Create Repository"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

// Compact directory browser: text input (editable, commits on blur/Enter) +
// a drill-down list backed by `api.directories()`. Selecting a row both
// navigates into it and adopts it as the current value — there is no
// separate "confirm" step, matching the desktop-style folder picker this ports.
function DirectoryPicker({
  id,
  value,
  onChange,
  onValidityChange,
  autoFocus,
}: {
  id: string
  value: string
  onChange: (path: string) => void
  /** Reports whenever the browse fetch starts/stops failing, so the caller's wrapping `Field`
   * can mirror it as `data-invalid` (forms.md). */
  onValidityChange?: (invalid: boolean) => void
  autoFocus?: boolean
}): ReactElement {
  const [inputValue, setInputValue] = useState(value)
  const [listing, setListing] = useState<DirectoryList | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setInputValue(value)
  }, [value])

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setError(null)
    zyncApi
      .directories(value || null)
      .then((result) => {
        if (cancelled) return
        setListing(result)
        onValidityChange?.(false)
        // Anchor to the server's canonical path (resolves "", relative
        // paths, symlinks, trailing slashes) so browsing and the committed
        // value never drift apart.
        if (result.current_path !== value) onChange(result.current_path)
      })
      .catch((err) => {
        if (cancelled) return
        setListing(null)
        setError(err instanceof Error ? err.message : String(err))
        onValidityChange?.(true)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
    // Re-fetch whenever the committed value changes; `onChange`/`onValidityChange` are setState
    // setters from the parent and stay referentially stable across renders.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value])

  const commit = () => {
    const next = inputValue.trim()
    if (next !== value) onChange(next)
  }

  return (
    <div className="flex flex-col gap-1.5">
      <Input
        id={id}
        autoFocus={autoFocus}
        spellCheck={false}
        className="font-mono text-xs"
        aria-invalid={error !== null || undefined}
        value={inputValue}
        onChange={(event) => setInputValue(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault()
            commit()
          }
        }}
      />
      <div className="border-input rounded-md border" data-testid="directory-picker">
        <ScrollArea className="h-36">
          <div className="flex flex-col gap-0.5 p-1">
            {loading && listing === null && (
              <div className="text-muted-foreground flex items-center gap-2 px-2 py-1.5 text-xs">
                <Spinner /> Loading…
              </div>
            )}
            {listing?.parent_path != null && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                data-testid="directory-picker-up"
                aria-label="Go to parent folder"
                className="justify-start text-muted-foreground"
                onClick={() => onChange(listing.parent_path as string)}
              >
                <ArrowUp data-icon="inline-start" />
                ..
              </Button>
            )}
            {listing?.directories.map((dir) => (
              <Button
                key={dir.path}
                type="button"
                variant="ghost"
                size="sm"
                data-testid="directory-picker-row"
                className="justify-start"
                onClick={() => onChange(dir.path)}
              >
                <Folder data-icon="inline-start" />
                <span className="truncate">{dir.name}</span>
              </Button>
            ))}
            {listing !== null && listing.directories.length === 0 && (
              <div className="text-muted-foreground px-2 py-1.5 text-xs">
                No subfolders
              </div>
            )}
          </div>
        </ScrollArea>
      </div>
      {error !== null && <FieldError>{error}</FieldError>}
    </div>
  )
}
