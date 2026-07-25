// Admin user management (P3.5) — reachable from the header UserMenu's
// "Admin: Users" entry (visible only when `currentUser.role === "admin"`).
// Lists every Zync account (`GET /auth/users`) and provisions new ones
// (`POST /auth/users` via CreateUserDialog) — both admin-only server-side, so
// this component trusts the caller already gated the entry point but still
// degrades gracefully (an Alert, not a crash) if a request 403s regardless.

import { useCallback, useEffect, useState, type ReactElement } from "react"

import { ShieldUser, UserPlus } from "lucide-react"

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@workspace/ui/components/alert"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@workspace/ui/components/empty"
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from "@workspace/ui/components/sheet"
import { Spinner } from "@workspace/ui/components/spinner"

import { zyncApi } from "@/lib/api"
import type { AdminUser } from "@/lib/types"

import { CreateUserDialog } from "./dialogs/CreateUserDialog"

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function AdminUsersSheet({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}): ReactElement {
  const [users, setUsers] = useState<AdminUser[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [addOpen, setAddOpen] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      setUsers(await zyncApi.listUsers())
    } catch (err) {
      setError(errorText(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (open) void load()
  }, [open, load])

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent data-testid="admin-users" className="gap-0 sm:max-w-md">
        <SheetHeader className="border-border border-b">
          <SheetTitle>Admin: Users</SheetTitle>
        </SheetHeader>
        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
          <div className="flex items-center justify-between gap-2">
            <Button
              data-testid="add-user-btn"
              variant="outline"
              size="xs"
              onClick={() => setAddOpen(true)}
            >
              <UserPlus data-icon="inline-start" />
              Add user
            </Button>
            <Button
              variant="ghost"
              size="xs"
              disabled={loading}
              onClick={() => void load()}
            >
              {loading && <Spinner data-icon="inline-start" />}
              Refresh
            </Button>
          </div>

          {error !== null && (
            <Alert variant="destructive">
              <AlertTitle>Could not load users</AlertTitle>
              <AlertDescription className="break-words">
                {error}
              </AlertDescription>
            </Alert>
          )}

          {users !== null && users.length === 0 ? (
            <Empty className="border">
              <EmptyHeader>
                <EmptyMedia variant="icon">
                  <ShieldUser />
                </EmptyMedia>
                <EmptyTitle>No users yet</EmptyTitle>
                <EmptyDescription>
                  Provision the first account below.
                </EmptyDescription>
              </EmptyHeader>
              <EmptyContent>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => setAddOpen(true)}
                >
                  <UserPlus data-icon="inline-start" />
                  Add user
                </Button>
              </EmptyContent>
            </Empty>
          ) : (
            <ul className="flex flex-col">
              {(users ?? []).map((user) => (
                <li
                  key={user.id}
                  data-testid="admin-user-row"
                  data-user-id={user.id}
                  className="flex items-center gap-2 border-b py-1.5"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <span className="truncate text-xs font-medium">
                        {user.name || user.email}
                      </span>
                      <Badge variant={user.role === "admin" ? "default" : "secondary"}>
                        {user.role}
                      </Badge>
                    </div>
                    <div className="text-muted-foreground truncate text-xs">
                      {user.email} · joined {user.created_at.slice(0, 10)}
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>

        <CreateUserDialog
          open={addOpen}
          onOpenChange={setAddOpen}
          onSubmit={async (request) => {
            await zyncApi.createUser(request)
            await load()
          }}
        />
      </SheetContent>
    </Sheet>
  )
}
