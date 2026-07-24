// Add-credential dialog (Credentials tab of GitToolsPanel). Secrets are
// write-only: the server only ever returns the masked CredentialRecord
// projection, so this dialog is create-only (update = delete + recreate).
//
// `onSubmit` is async and may throw the server's raw error text: a 400 about
// the host pattern is surfaced inline on that field (data-invalid per
// forms.md); anything else (e.g. 503 when ZYNC_SECRET_KEY is unset) shows as
// a destructive Alert inside the dialog.

import { useEffect, useState, type ReactElement } from "react"

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
  FieldTitle,
} from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"
import { Spinner } from "@workspace/ui/components/spinner"
import { Textarea } from "@workspace/ui/components/textarea"
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@workspace/ui/components/toggle-group"

import type { CreateCredentialRequest } from "@/lib/types"

type CredentialKind = "https_token" | "ssh_key"

export function CredentialDialog({
  open,
  onOpenChange,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Resolves on success (dialog closes itself); throws server error text. */
  onSubmit: (request: CreateCredentialRequest) => Promise<void>
}): ReactElement {
  const [kind, setKind] = useState<CredentialKind>("https_token")
  const [label, setLabel] = useState("")
  const [hostPattern, setHostPattern] = useState("")
  const [username, setUsername] = useState("")
  const [token, setToken] = useState("")
  const [privateKey, setPrivateKey] = useState("")
  const [passphrase, setPassphrase] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [hostError, setHostError] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)

  useEffect(() => {
    if (open) {
      setKind("https_token")
      setLabel("")
      setHostPattern("")
      setUsername("")
      setToken("")
      setPrivateKey("")
      setPassphrase("")
      setSubmitting(false)
      setHostError(null)
      setFormError(null)
    }
  }, [open])

  const secretPresent =
    kind === "https_token" ? token.trim() !== "" : privateKey.trim() !== ""
  const canSubmit =
    label.trim() !== "" && hostPattern.trim() !== "" && secretPresent

  const submit = async () => {
    if (!canSubmit || submitting) return
    setHostError(null)
    setFormError(null)
    setSubmitting(true)
    const request: CreateCredentialRequest = {
      label: label.trim(),
      host_pattern: hostPattern.trim(),
      kind,
      username: username.trim() === "" ? null : username.trim(),
      token: kind === "https_token" ? token : null,
      private_key: kind === "ssh_key" ? privateKey : null,
      passphrase: kind === "ssh_key" && passphrase !== "" ? passphrase : null,
      public_key: null,
    }
    try {
      await onSubmit(request)
      onOpenChange(false)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (/pattern|host/i.test(message)) {
        setHostError(message)
      } else {
        setFormError(message)
      }
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="credential-dialog">
        <DialogHeader>
          <DialogTitle>Add Credential</DialogTitle>
          <DialogDescription>
            Used when fetching, pulling or pushing to hosts matching the
            pattern.
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}
        >
          <FieldGroup>
            <Field>
              <FieldTitle id="credential-kind-label">Kind</FieldTitle>
              <ToggleGroup
                aria-labelledby="credential-kind-label"
                variant="outline"
                size="sm"
                value={[kind]}
                onValueChange={(value) => {
                  const next = value[0] as CredentialKind | undefined
                  if (next) setKind(next)
                }}
              >
                <ToggleGroupItem value="https_token">
                  HTTPS token
                </ToggleGroupItem>
                <ToggleGroupItem value="ssh_key">SSH key</ToggleGroupItem>
              </ToggleGroup>
            </Field>
            <Field>
              <FieldLabel htmlFor="credential-label">Label</FieldLabel>
              <Input
                id="credential-label"
                autoFocus
                placeholder="Work GitHub"
                value={label}
                onChange={(event) => setLabel(event.target.value)}
              />
            </Field>
            <Field data-invalid={hostError !== null || undefined}>
              <FieldLabel htmlFor="credential-host">Host pattern</FieldLabel>
              <Input
                id="credential-host"
                placeholder="*.github.com or github.com"
                aria-invalid={hostError !== null || undefined}
                value={hostPattern}
                onChange={(event) => {
                  setHostPattern(event.target.value)
                  setHostError(null)
                }}
              />
              {hostError !== null ? (
                <FieldError>{hostError}</FieldError>
              ) : (
                <FieldDescription>
                  Matched against the remote host when credentials are needed.
                </FieldDescription>
              )}
            </Field>
            <Field>
              <FieldLabel htmlFor="credential-username">Username</FieldLabel>
              <Input
                id="credential-username"
                placeholder={kind === "ssh_key" ? "git" : "token owner"}
                value={username}
                onChange={(event) => setUsername(event.target.value)}
              />
              <FieldDescription>Optional.</FieldDescription>
            </Field>
            {kind === "https_token" ? (
              <Field>
                <FieldLabel htmlFor="credential-token">Token</FieldLabel>
                <Input
                  id="credential-token"
                  type="password"
                  autoComplete="off"
                  value={token}
                  onChange={(event) => setToken(event.target.value)}
                />
              </Field>
            ) : (
              <>
                <Field>
                  <FieldLabel htmlFor="credential-private-key">
                    Private key
                  </FieldLabel>
                  <Textarea
                    id="credential-private-key"
                    rows={5}
                    autoComplete="off"
                    spellCheck={false}
                    placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                    value={privateKey}
                    onChange={(event) => setPrivateKey(event.target.value)}
                  />
                </Field>
                <Field>
                  <FieldLabel htmlFor="credential-passphrase">
                    Passphrase
                  </FieldLabel>
                  <Input
                    id="credential-passphrase"
                    type="password"
                    autoComplete="off"
                    value={passphrase}
                    onChange={(event) => setPassphrase(event.target.value)}
                  />
                  <FieldDescription>
                    Optional — only if the key is encrypted.
                  </FieldDescription>
                </Field>
              </>
            )}
            <FieldDescription>
              Secrets are write-only: they cannot be viewed again after saving.
            </FieldDescription>
            {formError !== null && (
              <Alert variant="destructive">
                <AlertTitle>Could not save credential</AlertTitle>
                <AlertDescription className="break-words">
                  {formError}
                </AlertDescription>
              </Alert>
            )}
          </FieldGroup>
          <DialogFooter className="mt-6">
            <DialogClose
              data-testid="dialog-cancel"
              render={<Button variant="outline" type="button" />}
            >
              Cancel
            </DialogClose>
            <Button
              data-testid="dialog-submit"
              type="submit"
              disabled={!canSubmit || submitting}
            >
              {submitting && <Spinner data-icon="inline-start" />}
              Save Credential
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
