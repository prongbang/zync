import { useState, type FormEvent } from "react"

import { Alert, AlertDescription, AlertTitle } from "@workspace/ui/components/alert"
import { Button } from "@workspace/ui/components/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@workspace/ui/components/card"
import {
  Field,
  FieldGroup,
  FieldLabel,
} from "@workspace/ui/components/field"
import { Input } from "@workspace/ui/components/input"
import { Spinner } from "@workspace/ui/components/spinner"

/**
 * The auth gate's Login screen (P3.4). Presentational: it owns only its field
 * state; `onLogin` throws on failure so this can surface the server's generic
 * "invalid credentials" without leaking a user-enumeration signal. In
 * ZYNC_AUTH=disabled mode the AuthGate never renders this — /auth/me returns the
 * synthetic owner, so the app is transparently signed in.
 */
export function Login({
  onLogin,
}: {
  onLogin: (identifier: string, password: string) => Promise<void>
}) {
  const [identifier, setIdentifier] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (pending) return
    setPending(true)
    setError(null)
    try {
      await onLogin(identifier, password)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to sign in")
    } finally {
      setPending(false)
    }
  }

  const invalid = error !== null

  return (
    <div className="bg-background text-foreground grid h-svh place-items-center p-6">
      <Card className="w-full max-w-sm" data-testid="login-screen">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <span className="bg-primary size-2 rounded-full" />
            Sign in to Zync
          </CardTitle>
          <CardDescription>
            Enter your credentials to open your repositories.
          </CardDescription>
        </CardHeader>
        <form onSubmit={handleSubmit}>
          <CardContent>
            <FieldGroup>
              <Field data-invalid={invalid || undefined}>
                <FieldLabel htmlFor="login-identifier">Email</FieldLabel>
                <Input
                  id="login-identifier"
                  data-testid="login-identifier"
                  type="text"
                  autoComplete="username"
                  autoFocus
                  value={identifier}
                  aria-invalid={invalid || undefined}
                  onChange={(e) => setIdentifier(e.target.value)}
                />
              </Field>
              <Field data-invalid={invalid || undefined}>
                <FieldLabel htmlFor="login-password">Password</FieldLabel>
                <Input
                  id="login-password"
                  data-testid="login-password"
                  type="password"
                  autoComplete="current-password"
                  value={password}
                  aria-invalid={invalid || undefined}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </Field>
              {error && (
                <Alert variant="destructive" data-testid="login-error">
                  <AlertTitle>Sign in failed</AlertTitle>
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              )}
            </FieldGroup>
          </CardContent>
          <CardFooter>
            <Button
              type="submit"
              className="w-full"
              data-testid="login-submit"
              disabled={pending}
            >
              {pending && <Spinner data-icon="inline-start" />}
              Sign in
            </Button>
          </CardFooter>
        </form>
      </Card>
    </div>
  )
}
