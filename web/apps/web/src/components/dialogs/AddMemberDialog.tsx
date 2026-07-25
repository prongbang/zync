// Add-member dialog (Members tab of GitToolsPanel, P3.5). Grants an existing
// Zync user a repo-scoped role — the server resolves `identifier` by user id
// or (case-insensitive) email and 404s if no such user exists.

import { useEffect, useState, type ReactElement } from "react"

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
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@workspace/ui/components/select"

export type MemberRole = "owner" | "member" | "viewer"

export const MEMBER_ROLE_LABEL: Record<MemberRole, string> = {
  owner: "Owner",
  member: "Member",
  viewer: "Viewer",
}

export type AddMemberPayload = { identifier: string; role: MemberRole }

export function AddMemberDialog({
  open,
  onOpenChange,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (payload: AddMemberPayload) => void
}): ReactElement {
  const [identifier, setIdentifier] = useState("")
  const [role, setRole] = useState<MemberRole>("member")

  useEffect(() => {
    if (open) {
      setIdentifier("")
      setRole("member")
    }
  }, [open])

  const canSubmit = identifier.trim() !== ""

  const submit = () => {
    if (!canSubmit) return
    onSubmit({ identifier: identifier.trim(), role })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="add-member-dialog">
        <DialogHeader>
          <DialogTitle>Add Member</DialogTitle>
          <DialogDescription>
            Grant an existing Zync user access to this repository.
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event) => {
            event.preventDefault()
            submit()
          }}
        >
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="member-identifier">User</FieldLabel>
              <Input
                id="member-identifier"
                autoFocus
                placeholder="user@example.com"
                value={identifier}
                onChange={(event) => setIdentifier(event.target.value)}
              />
              <FieldDescription>
                User id or email of an existing Zync account.
              </FieldDescription>
            </Field>
            <Field>
              <FieldLabel htmlFor="member-role">Role</FieldLabel>
              <Select
                value={role}
                onValueChange={(value) => setRole(value as MemberRole)}
              >
                <SelectTrigger
                  id="member-role"
                  data-testid="add-member-role-select"
                >
                  <SelectValue>
                    {(value: MemberRole) => MEMBER_ROLE_LABEL[value] ?? value}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    {(Object.keys(MEMBER_ROLE_LABEL) as MemberRole[]).map((r) => (
                      <SelectItem key={r} value={r}>
                        {MEMBER_ROLE_LABEL[r]}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          </FieldGroup>
          <DialogFooter className="mt-6">
            <DialogClose
              data-testid="dialog-cancel"
              render={<Button variant="outline" type="button" />}
            >
              Cancel
            </DialogClose>
            <Button data-testid="dialog-submit" type="submit" disabled={!canSubmit}>
              Add Member
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
