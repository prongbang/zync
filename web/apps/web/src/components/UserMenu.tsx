import { KeyRound, LogOut } from "lucide-react"

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
}: {
  user: CurrentUser
  onLogout: () => void
  onOpenCredentials: () => void
}) {
  const label = user.name || user.email || user.id
  const initial = (label.charAt(0) || "Z").toUpperCase()

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
        <DropdownMenuLabel>{user.email || label}</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem onClick={onOpenCredentials}>
            <KeyRound data-icon="inline-start" />
            Credentials
          </DropdownMenuItem>
          <DropdownMenuItem data-testid="logout-btn" onClick={onLogout}>
            <LogOut data-icon="inline-start" />
            Log out
          </DropdownMenuItem>
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
