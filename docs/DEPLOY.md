# Production Deployment & TLS Guide

This guide covers running `zync-server` in production: the full `ZYNC_*`
environment variable surface, a TLS-terminating reverse proxy in front of the
container (nginx and Caddy), health/readiness wiring for an orchestrator, and
the gotchas specific to Zync's WebSocket live-sync and IP-based rate limiting.

It assumes the Docker image described in `Dockerfile` / `docker-compose.yml`
(bun → rust → debian multi-stage build, server binary at
`/usr/local/bin/zync-server`, static React build baked into `/app/public`).
Everything below is verified against `crates/server/src/main.rs`,
`crates/server/src/net_hardening.rs`, `crates/server/src/auth/mod.rs`,
`crates/server/src/crypto/mod.rs`, `crates/server/src/repos_root.rs`, and
`crates/server/src/observability.rs`.

## 1. Environment variable reference

| Variable | Purpose | Required | Default | Production recommendation |
|---|---|---|---|---|
| `ZYNC_BIND` | Socket the Axum server listens on (`SocketAddr`, e.g. `0.0.0.0:58271`) | No | `127.0.0.1:58271` (the Dockerfile's `ENV` overrides this to `0.0.0.0:58271` inside the image) | Behind a reverse proxy, bind to a container-internal address only — do not publish this port directly to the internet. |
| `ZYNC_STATIC_DIR` | Filesystem path the built React SPA (`index.html` + `/assets/*`) is served from, with an `index.html` fallback for unmatched routes | No | `/app/public` | Leave at the image default unless you rebuild the static assets into a different path. |
| `ZYNC_DB` | SQLite database file path | No | `zync.db` (relative to the process's working directory) | Point at a path on a persistent volume, e.g. `/data/zync.db` (see the `zync-data` volume in `docker-compose.yml`). |
| `ZYNC_LOG_FORMAT` | `json` switches `tracing_subscriber`'s formatter to structured JSON (same `RUST_LOG`/`EnvFilter` filtering either way) | No | unset = human-readable format | Set `ZYNC_LOG_FORMAT=json` so a log collector (Loki, CloudWatch, Datadog, etc.) can parse fields, including the per-request `request_id` span. |
| `ZYNC_AUTH` | `enabled` or `disabled`. `disabled` reproduces single-user/no-auth behavior byte-for-byte (every request gets a synthetic `owner`/`admin` principal, login/logout are no-ops). An unrecognized value refuses to boot. | No | `enabled` | **Always `enabled` in production.** `disabled` is only for trusted single-user/LAN dev use — never expose a `disabled`-mode server to untrusted networks. |
| `ZYNC_ADMIN_USER` + `ZYNC_ADMIN_PASSWORD` | First-boot admin bootstrap: if both are set (and no admin password exists yet), the server hashes and sets this as the initial admin login instead of printing a one-time `/setup` token | No (either this pair or the `/setup` link) | unset | Prefer setting both via your secrets manager on first boot only ("first boot" = until any admin password is set — subsequent boots no-op even if the vars are still present); rotate the admin password afterward via `POST /auth/users`/normal login, since these vars are only consulted once. |
| `ZYNC_COOKIE_INSECURE` | `1` drops the `Secure` attribute from the `zync_session` cookie | No | unset = cookie is `Secure` | **Never set this in production.** It exists only for plain-HTTP local/LAN dev. Setting it in production means the session cookie is sent over any plain-HTTP hop — see §3. |
| `ZYNC_SECRET_KEY` | Base64-encoded 32-byte AEAD key (XChaCha20Poly1305) encrypting the `credentials` table (stored HTTPS tokens / SSH keys) at rest | Required to use the credential store (HTTPS token / SSH key auth for private remotes) | unset = credential operations fail fast with a clear `NotConfigured` error, everything else still works | Generate with `openssl rand -base64 32` and inject via your secrets manager. Rotating it invalidates previously stored credentials (they were encrypted under the old key) — re-add them after rotation. |
| `ZYNC_DEV` (or `--dev` CLI arg) | If `ZYNC_SECRET_KEY` is unset/invalid, `ZYNC_DEV=1` falls back to a **fixed all-zero** encryption key so credential features still work in dev | No | unset | **Never set in production** — stored credentials under the dev fallback key are not meaningfully encrypted. If both `ZYNC_SECRET_KEY` and `ZYNC_DEV` are unset, credentials are simply disabled (safe default) rather than silently using a weak key. |
| `ZYNC_REPOS_ROOT` | Colon-separated (`:`) list of directories a caller may register/clone-into/`git init` under, and that `GET /directories` is confined to browsing. Canonicalized and validated to exist at boot; a *set-but-unresolvable* entry refuses to boot. | No, but **strongly recommended once `ZYNC_AUTH=enabled` with more than one user** | unset = unbounded (any path the server process can see is registrable — today's single-user back-compat behavior) | Set to the parent director{y,ies} of every host repo you intend to mount, e.g. `ZYNC_REPOS_ROOT=/workspaces`. The server logs a startup warning if auth is enabled and this is unset. |
| `ZYNC_CORS_ORIGINS` | Comma-separated list of origins allowed to make credentialed cross-origin requests (`Access-Control-Allow-Origin` + `-Credentials: true`) | No | unset/empty = no CORS allowlist (safe default — same-origin requests never consult CORS headers at all) | Leave unset for the standard same-origin deployment (proxy/container serves the SPA and API from the same origin). Only set this if you deliberately run the API cross-origin from a separately-hosted SPA, and lock it to that exact origin (e.g. `https://zync.example.com`) — never a wildcard, since wildcard + credentials is invalid per the fetch spec and this app always sends the session cookie. |
| `ZYNC_TRUSTED_PROXY` | Exact value `1` switches the `/auth/login` and `/auth/ws-ticket` rate limiters from keying on the raw TCP peer address to recovering the real client IP from forwarded headers (`X-Forwarded-For`/etc., via `SmartIpKeyExtractor`) | **Required whenever the app sits behind any reverse proxy** | unset = keys on the raw TCP peer address (`PeerIpKeyExtractor`) | Set `ZYNC_TRUSTED_PROXY=1` **only** when the proxy in front of the app is one you control and that proxy discards/overwrites any client-supplied `X-Forwarded-For`/`X-Real-IP`/`Forwarded` headers before setting its own. See §6 for why this matters. |

Two related knobs that exist but are not configurable via env vars, included
for completeness:

- Request body size cap: `net_hardening::MAX_REQUEST_BODY_BYTES` = **10 MiB**, hardcoded. Match this at the proxy (see the nginx/Caddy snippets below) so the proxy doesn't allow a body the app will reject anyway, or reject one the app would accept.
- Rate limits: `POST /auth/login` and `POST /setup` are limited to a burst of 10 requests per key, refilling 1 every 6 seconds (~10/min steady state); `POST /auth/ws-ticket` is limited to a burst of 40, refilling 1/sec (~60/min steady state, deliberately generous so a flaky WebSocket reconnect loop doesn't lock itself out).

## 2. Docker Compose production example

The shipped `docker-compose.yml` publishes `zync` directly on `58271` with no
TLS. For production, put a TLS-terminating reverse proxy in front of it and
keep the `zync` service off the public network entirely.

```yaml
services:
  zync:
    image: ghcr.io/prongbang/zync:v0.1.0   # pin an exact release tag, not :latest
    environment:
      ZYNC_BIND: 0.0.0.0:58271
      ZYNC_DB: /data/zync.db
      ZYNC_AUTH: enabled
      ZYNC_SECRET_KEY: ${ZYNC_SECRET_KEY}          # openssl rand -base64 32
      ZYNC_REPOS_ROOT: /workspaces
      ZYNC_TRUSTED_PROXY: "1"                       # proxy is the "caddy"/"nginx" service below
      ZYNC_LOG_FORMAT: json
      # ZYNC_ADMIN_USER / ZYNC_ADMIN_PASSWORD: set only for the very first boot.
    # No `ports:` — only reachable from other services on this network, via
    # the proxy below. Do not publish 58271 to the host in production.
    expose:
      - "58271"
    volumes:
      - zync-data:/data
      - zync-workspaces:/workspaces
      # - /srv/git/my-project:/workspaces/my-project
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "wget", "-q", "-O-", "http://127.0.0.1:58271/health"]
      interval: 30s
      timeout: 3s
      retries: 3

  proxy:
    image: caddy:2-alpine        # or nginx:alpine — see the two configs below
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro   # or ./nginx.conf:/etc/nginx/conf.d/default.conf:ro
      - caddy-data:/data                       # Caddy's ACME state; drop for nginx+certbot
    depends_on:
      - zync
    restart: unless-stopped

volumes:
  zync-data:
  zync-workspaces:
  caddy-data:
```

> **Note on the `wget` healthcheck above:** the current `debian:bookworm-slim`
> final stage (`Dockerfile`) installs only `ca-certificates`, `git`, and
> `openssh-client` — it does **not** include `curl` or `wget`. The
> `healthcheck:` block above will fail with "executable file not found" until
> one of those is added to the image. I could not confirm from the repo
> whether that's planned; flagging it here rather than silently shipping a
> healthcheck that can't run. Two ways around it without changing the
> Dockerfile yourself: drop the Compose-level `healthcheck:` and rely solely
> on the orchestrator-level probes in §5 (which run from outside the
> container and need nothing installed inside it), or add
> `--no-install-recommends wget` (a few hundred KB) to the existing `apt-get
> install` line in the final stage.

### nginx server block

```nginx
upstream zync_app {
    server zync:58271;
}

server {
    listen 443 ssl http2;
    server_name zync.example.com;

    ssl_certificate     /etc/letsencrypt/live/zync.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/zync.example.com/privkey.pem;

    # Match the app's own request body cap (net_hardening::MAX_REQUEST_BODY_BYTES).
    client_max_body_size 10m;

    location / {
        proxy_pass http://zync_app;
        proxy_http_version 1.1;

        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # WebSocket upgrade for live-sync (/ws/workspace/:id) — without these two
    # headers the Upgrade handshake fails and the UI falls back to polling/
    # never live-updates.
    location /ws/ {
        proxy_pass http://zync_app;
        proxy_http_version 1.1;
        proxy_set_header Upgrade    $http_upgrade;
        proxy_set_header Connection "upgrade";

        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket connections are long-lived; nginx's default proxy_read_timeout
        # (60s) will kill an idle-but-live connection otherwise.
        proxy_read_timeout 1h;
    }
}

server {
    listen 80;
    server_name zync.example.com;
    return 301 https://$host$request_uri;
}
```

Pair this with `ZYNC_TRUSTED_PROXY=1` on the `zync` service — see §6.

### Caddyfile equivalent

Caddy auto-provisions and renews TLS (Let's Encrypt/ZeroSSL) for the
configured domain, and upgrades WebSocket connections automatically for any
reverse-proxied route, so no separate `/ws/` block is needed:

```caddyfile
zync.example.com {
    request_body {
        max_size 10MB
    }

    reverse_proxy zync:58271 {
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
        header_up Host {host}
    }
}
```

Caddy sets `X-Forwarded-For`/`X-Forwarded-Proto` by default even without the
explicit `header_up` lines above; they're included here for clarity and to
make the nginx/Caddy configs symmetric.

## 3. TLS

Terminate TLS at the proxy (Caddy's automatic HTTPS, or nginx + certbot) —
`zync-server` itself speaks plain HTTP and has no TLS support built in.

- Do **not** set `ZYNC_COOKIE_INSECURE` in production. The `zync_session`
  cookie is `HttpOnly; SameSite=Lax` and `Secure` by default
  (`crates/server/src/auth/mod.rs`); `Secure` means the browser will only
  ever send it over HTTPS. Dropping `Secure` (`ZYNC_COOKIE_INSECURE=1`) is
  correct only for a plain-HTTP LAN/dev setup with no TLS anywhere in the
  path — it is wrong in a TLS-terminating-proxy deployment even though the
  proxy→app hop is plain HTTP internally, because what matters is the
  browser→proxy hop, which must stay HTTPS for `Secure` to have any teeth.
- Make sure `X-Forwarded-Proto: https` reaches the app from the proxy (both
  snippets above set it). The app doesn't currently branch on this header
  itself, but downstream tooling (log processors, links generated from
  request metadata) and any future redirect-to-HTTPS logic will depend on it
  being accurate — set it correctly now rather than retrofitting later.
- Redirect HTTP → HTTPS at the proxy (the nginx config's second `server`
  block; Caddy does this automatically for any domain it manages TLS for).

## 4. Hardening checklist for go-live

- [ ] `ZYNC_AUTH=enabled` (or unset — that's the default) — never `disabled` on a network-reachable deployment.
- [ ] `ZYNC_SECRET_KEY` set to a freshly generated key: `openssl rand -base64 32`. Store it in your secrets manager, not in a committed `.env` or Compose file.
- [ ] `ZYNC_REPOS_ROOT` set to the exact parent director{y,ies} you intend to mount repos under (e.g. `/workspaces`) — without it, once `ZYNC_AUTH=enabled` with more than one user, any authenticated user can register/clone/`git init` an arbitrary path the server process can see.
- [ ] The bootstrap admin password rotated off whatever `ZYNC_ADMIN_USER`/`ZYNC_ADMIN_PASSWORD` (or the one-time `/setup` token) set it to — those env vars are only consulted while no admin password exists yet, so they're safe to leave in your deploy config, but the *password itself* should be changed by the admin after first login if it was seeded via env vars committed anywhere.
- [ ] `ZYNC_CORS_ORIGINS` left unset (same-origin deploy via the proxy) or, if you do run a cross-origin SPA, locked to the exact real front-end origin — never a wildcard.
- [ ] `ZYNC_DEV` and `ZYNC_COOKIE_INSECURE` are **not** set anywhere in the production environment (double-check inherited/base Compose files and CI-injected env, not just the production override file).
- [ ] `ZYNC_TRUSTED_PROXY=1` set on the app **and** the proxy in front of it is configured to overwrite (not merely append to) any inbound `X-Forwarded-For`/`X-Real-IP`/`Forwarded` — see §6.
- [ ] Proxy `client_max_body_size`/`request_body.max_size` set to 10 MB to match `net_hardening::MAX_REQUEST_BODY_BYTES`, so large-but-legitimate payloads (e.g. `stage_patch`'s full unified diff, `write_file`'s raw file content) aren't rejected by the proxy before they'd even reach the app's own limit — and so the proxy doesn't buffer/accept something the app will reject anyway.
- [ ] TLS certificates auto-renewing (Caddy: automatic; nginx+certbot: a renewal cron/systemd timer) and HTTP→HTTPS redirect in place.
- [ ] `ZYNC_DB` pointed at a path on a volume that's actually backed up (the SQLite file is the entire persistence layer — users, sessions, repositories, workspaces, encrypted credentials).

## 5. Health/readiness wiring

- `GET /health` — **liveness**. Always `200`, does no I/O (no DB touch). Use this to answer "is the process alive enough to keep routing to", not "is it fully functional". Public (no session required).
- `GET /ready` — **readiness**. Does a cheap, non-mutating DB read (looks up the seeded `owner` row) and returns `503` if the DB doesn't answer. Use this to hold traffic back from an instance whose DB connection/lock/schema isn't healthy yet. Also public.

Both are outside the auth layer (see `auth::is_public` in
`crates/server/src/auth/mod.rs`) specifically so an orchestrator's probes
never need a session.

### Docker Compose healthcheck

```yaml
healthcheck:
  test: ["CMD", "wget", "-q", "-O-", "http://127.0.0.1:58271/ready"]
  interval: 15s
  timeout: 3s
  retries: 3
  start_period: 10s
```

(See the §2 callout — the shipped image doesn't currently bundle `wget`/`curl`;
this needs that dependency added to the Dockerfile, or use the orchestrator
probes below instead, which run from outside the container.)

### Kubernetes-style probes

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 58271
  initialDelaySeconds: 5
  periodSeconds: 15
  timeoutSeconds: 3
  failureThreshold: 3

readinessProbe:
  httpGet:
    path: /ready
    port: 58271
  initialDelaySeconds: 5
  periodSeconds: 10
  timeoutSeconds: 3
  failureThreshold: 3
```

kubelet's `httpGet` probe is executed from the node, not from inside the
container, so it needs no HTTP client installed in the image — unlike the
Compose `CMD`-style healthcheck above.

> Zync ships no Kubernetes manifests in this repo today (Deployment/Service/
> Ingress). The snippet above is deliberately just the probe stanza to drop
> into whatever manifest you write; a full manifest is out of scope here and
> unverified against anything in the repo.

## 6. Reverse-proxy gotchas

**WebSocket upgrade.** The live-sync connection is `GET /ws/workspace/:id`
(`crates/server/src/websocket/mod.rs`). It's session-ticket-guarded (see
`POST /auth/ws-ticket` in `auth/mod.rs`) rather than cookie-guarded, because
cookies don't propagate reliably onto a WS upgrade — but the HTTP-level
`Upgrade: websocket` / `Connection: upgrade` headers still have to reach the
app unmolested. Both example configs in §2 forward them explicitly for `/ws/`
(nginx) or automatically (Caddy's `reverse_proxy`, which upgrades any
matching connection transparently — no separate block needed). If you swap in
a different proxy, confirm it forwards `Upgrade`/`Connection` and doesn't
apply an aggressive idle timeout to long-lived connections — the nginx
example bumps `proxy_read_timeout` to `1h` for exactly this reason.

**Forwarded headers + `ZYNC_TRUSTED_PROXY`.** This is the single most
important reverse-proxy setting to get right. The rate limiter for
`/auth/login`/`/setup`/`/auth/ws-ticket` keys on a caller identity extracted
one of two ways (`crates/server/src/net_hardening.rs`):

- `ZYNC_TRUSTED_PROXY` unset (default): keys on the raw TCP peer address
  (`PeerIpKeyExtractor`) — correct only when the app is reachable directly,
  with no proxy in front of it.
- `ZYNC_TRUSTED_PROXY=1`: keys on the real client IP recovered from
  `X-Forwarded-For`/similar headers (`SmartIpKeyExtractor`), falling back to
  the peer address if none are present.

Behind a reverse proxy, every request's TCP peer address is the *proxy's*
own IP — not the end client's. Left at the default, this means **every
client sharing that proxy collapses into one shared rate-limit bucket**: one
noisy or malicious client can exhaust the `/auth/login` quota and lock out
everyone else's login attempts, and the per-IP brute-force defense is
nullified because an attacker appears to share an IP with legitimate users.
Setting `ZYNC_TRUSTED_PROXY=1` fixes this — but **only do so when the proxy
in front of the app is one you control, and that proxy overwrites (not
appends to) any client-supplied `X-Forwarded-For`/`X-Real-IP`/`Forwarded`
header before setting its own.** Both nginx's `$proxy_add_x_forwarded_for`
and Caddy's default forwarding behavior do this correctly (they append the
proxy's own hop to any preexisting header rather than blindly trusting a
client-supplied value as the final one, and — more importantly — they're the
last hop before the app, so the header they set is authoritative). If you
ever put an *additional*, less-trusted proxy in front of the one talking to
`zync-server`, re-verify that a client can't spoof the header the app
ultimately reads. Terminating TLS at a proxy without setting
`ZYNC_TRUSTED_PROXY=1` doesn't break anything functionally, it just silently
makes the app's own rate limiting ineffective — enforce rate limiting at the
proxy layer instead in that case (nginx's `limit_req`, Caddy's `rate_limit`
plugin, or your CDN/WAF).

**Body-size limits at the proxy.** The app enforces a hard 10 MiB cap on
every request body (`net_hardening::MAX_REQUEST_BODY_BYTES`, via
`RequestBodyLimitLayer`, with axum's own independent 2 MB default explicitly
disabled so this is the only limit in effect). Set the proxy's own cap
(`client_max_body_size 10m` for nginx, `request_body { max_size 10MB }` for
Caddy) to the same value — a lower proxy limit will reject legitimate
large payloads (a full unified diff from `stage_patch`, raw file content
from `write_file`/`create_file`) before they ever reach the app's own,
correctly-sized limit; a higher proxy limit just wastes buffer/memory on
bodies the app will reject anyway.

## Facts I could not verify from the repo

- Whether the `debian:bookworm-slim` final stage is expected to gain
  `curl`/`wget` in a future change — today it doesn't, so the Compose-level
  `healthcheck:` examples in §2/§5 will not run as-is (flagged inline above).
- No Kubernetes manifest exists in this repo to confirm field names/labels
  against — the §5 probe snippet is a standalone stanza, not validated
  against a real Deployment in this codebase.

(`ghcr.io/prongbang/zync` in the §2 Compose example is confirmed directly
against `.github/workflows/release.yml`'s `IMAGE_NAME: ${{ github.repository }}`
and its `github.repository == 'prongbang/zync'` guard — not a guess.)
