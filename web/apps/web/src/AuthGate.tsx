import { useCallback, useEffect, useState } from "react"

import { Spinner } from "@workspace/ui/components/spinner"

import { App } from "./App"
import { Login } from "./components/Login"
import { setUnauthorizedHandler, zyncApi } from "./lib/api"
import type { CurrentUser } from "./lib/types"

type AuthStatus = "loading" | "login" | "authed"

/**
 * Top-level auth gate (P3.4). Probes `/auth/me` once on load: a user renders the
 * app (mounting `useWorkspace` only when authenticated); a 401 renders Login.
 *
 * In ZYNC_AUTH=disabled mode the server answers `/auth/me` with the synthetic
 * owner, so this is fully transparent — the Login screen never appears.
 *
 * It also registers the single 401 interceptor: any expired-session 401 from
 * any api call drops back to `login`, which unmounts `App` (and thus clears all
 * workspace state) rather than leaving a half-dead authed shell.
 */
export function AuthGate() {
  const [status, setStatus] = useState<AuthStatus>("loading")
  const [user, setUser] = useState<CurrentUser | null>(null)

  useEffect(() => {
    setUnauthorizedHandler(() => {
      setUser(null)
      setStatus("login")
    })
    let cancelled = false
    void zyncApi
      .me()
      .then((me) => {
        if (cancelled) return
        setUser(me)
        setStatus("authed")
      })
      .catch(() => {
        if (cancelled) return
        setStatus("login")
      })
    return () => {
      cancelled = true
      setUnauthorizedHandler(null)
    }
  }, [])

  const handleLogin = useCallback(
    async (identifier: string, password: string) => {
      await zyncApi.login(identifier, password)
      // Re-probe so `authed` always reflects the server's own view of the user.
      const me = await zyncApi.me()
      setUser(me)
      setStatus("authed")
    },
    [],
  )

  const handleLogout = useCallback(() => {
    void zyncApi.logout().finally(() => {
      setUser(null)
      setStatus("login")
    })
  }, [])

  if (status === "loading") {
    return (
      <div
        className="bg-background text-muted-foreground grid h-svh place-items-center"
        data-testid="auth-loading"
      >
        <Spinner />
      </div>
    )
  }

  if (status === "login" || !user) {
    return <Login onLogin={handleLogin} />
  }

  return <App currentUser={user} onLogout={handleLogout} />
}
