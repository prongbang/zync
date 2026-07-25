// Admin user-provisioning dialog (AdminUsersSheet, P3.5). Server-side this is
// `POST /auth/users` — admin-only (403 for anyone else), no self-service
// registration route exists. `onSubmit` is async and may throw the server's
// raw error text (e.g. a 409 for a duplicate identifier), surfaced inline.

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
} from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@workspace/ui/components/select"
import { Spinner } from "@workspace/ui/components/spinner"

import type { CreateUserRequest } from "@/lib/types"

export type AdminRole = "admin" | "user"

const ROLE_LABEL: Record<AdminRole, string> = {
  admin: "Admin",
  user: "User",
}

export function CreateUserDialog({
  open,
  onOpenChange,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Resolves on success (dialog closes itself); throws server error text. */
  onSubmit: (request: CreateUserRequest) => Promise<void>
}): ReactElement {
  const [identifier, setIdentifier] = useState("")
  const [password, setPassword] = useState("")
  const [name, setName] = useState("")
  const [role, setRole] = useState<AdminRole>("user")
  const [submitting, setSubmitting] = useState(false)
  const [identifierError, setIdentifierError] = useState<string | null>(null)
  const [formError, setFormError] = useState<string | null>(null)

  useEffect(() => {
    if (open) {
      setIdentifier("")
      setPassword("")
      setName("")
      setRole("user")
      setSubmitting(false)
      setIdentifierError(null)
      setFormError(null)
    }
  }, [open])

  const canSubmit = identifier.trim() !== "" && password.trim() !== ""

  const submit = async () => {
    if (!canSubmit || submitting) return
    setIdentifierError(null)
    setFormError(null)
    setSubmitting(true)
    const request: CreateUserRequest = {
      identifier: identifier.trim(),
      password,
      name: name.trim() === "" ? null : name.trim(),
      role,
    }
    try {
      await onSubmit(request)
      onOpenChange(false)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (/identifier|email|exists/i.test(message)) {
        setIdentifierError(message)
      } else {
        setFormError(message)
      }
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="create-user-dialog">
        <DialogHeader>
          <DialogTitle>Add User</DialogTitle>
          <DialogDescription>
            Provisions a new Zync account with an initial password. There is
            no self-service sign-up — only an admin can create users.
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}
        >
          <FieldGroup>
            <Field data-invalid={identifierError !== null || undefined}>
              <FieldLabel htmlFor="new-user-identifier">Email</FieldLabel>
              <Input
                id="new-user-identifier"
                autoFocus
                type="email"
                placeholder="teammate@example.com"
                aria-invalid={identifierError !== null || undefined}
                value={identifier}
                onChange={(event) => {
                  setIdentifier(event.target.value)
                  setIdentifierError(null)
                }}
              />
              {identifierError !== null && (
                <FieldError>{identifierError}</FieldError>
              )}
            </Field>
            <Field>
              <FieldLabel htmlFor="new-user-name">Name</FieldLabel>
              <Input
                id="new-user-name"
                placeholder="Optional"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
              <FieldDescription>
                Defaults to the email address if left blank.
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="new-user-password">
                Initial password
              </FieldLabel>
              <Input
                id="new-user-password"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
              <FieldDescription>
                The user can log in with this immediately.
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="new-user-role">Role</FieldLabel>
              <Select
                value={role}
                onValueChange={(value) => {
                  if (value) setRole(value as AdminRole)
                }}
              >
                <SelectTrigger id="new-user-role" data-testid="new-user-role-select">
                  <SelectValue>
                    {(value: AdminRole) => ROLE_LABEL[value] ?? value}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {(Object.keys(ROLE_LABEL) as AdminRole[]).map((r) => (
                      <SelectItem key={r} value={r}>
                        {ROLE_LABEL[r]}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
              <FieldDescription>
                Admins bypass every repository&rsquo;s member permissions.
              </FieldDescription>
            </Field>
            {formError !== null && (
              <Alert variant="destructive">
                <AlertTitle>Could not create user</AlertTitle>
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
              Add User
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
