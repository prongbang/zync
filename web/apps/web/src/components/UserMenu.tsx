import { KeyRound, LogOut, ShieldUser } from "lucide-react"

import {
  Avatar,
  AvatarFallback,
} from "@workspace/ui/components/avatar"
import { Button } from "@workspace/ui/components/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@workspace/ui/components/dropdown-menu"

import type { CurrentUser } from "../lib/types"

/**
 * Header account menu (P3.4). Shows the current user (from /auth/me) with an
 * entry into the Credentials settings and a Logout that returns to the Login
 * screen. In ZYNC_AUTH=disabled mode `user` is the synthetic owner.
 */
export function UserMenu({
  user,
  onLogout,
  onOpenCredentials,
  onOpenAdminUsers,
}: {
  user: CurrentUser
  onLogout: () => void
  onOpenCredentials: () => void
  /** Omitted -> no "Admin: Users" entry (e.g. render sites that don't wire it
   * up yet). Also gated on `user.role === "admin"` below regardless. */
  onOpenAdminUsers?: () => void
}) {
  const label = user.name || user.email || user.id
  const initial = (label.charAt(0) || "Z").toUpperCase()
  const isAdmin = user.role === "admin"

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        data-testid="user-menu"
        render={
          <Button
            variant="ghost"
            size="sm"
            className="gap-2"
            aria-label="Account menu"
          />
        }
      >
        <Avatar className="size-5">
          <AvatarFallback>{initial}</AvatarFallback>
        </Avatar>
        <span className="max-w-32 truncate">{label}</span>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {/* DropdownMenuLabel (Menu.GroupLabel) requires a DropdownMenuGroup
            ancestor — see web/.agents/skills/shadcn/rules/composition.md —
            or base-ui throws "MenuGroupContext is missing" on open. */}
        <DropdownMenuGroup>
          <DropdownMenuLabel>{user.email || label}</DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem onClick={onOpenCredentials}>
            <KeyRound data-icon="inline-start" />
            Credentials
          </DropdownMenuItem>
          {isAdmin && onOpenAdminUsers && (
            <DropdownMenuItem
              data-testid="admin-users-menu-item"
              onClick={onOpenAdminUsers}
            >
              <ShieldUser data-icon="inline-start" />
              Admin: Users
            </DropdownMenuItem>
          )}
          <DropdownMenuItem data-testid="logout-btn" onClick={onLogout}>
            <LogOut data-icon="inline-start" />
            Log out
          </DropdownMenuItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
