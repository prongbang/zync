# Zync

A self-hosted, desktop-style Git workspace client: a single Rust binary that
serves a fast React web UI — commit graph, diffs, branches, remotes, and live
sync — over your Git repositories on disk.

![Zync commit graph, diff, and live sync](docs/preview.png)

## Install

### Install script (single binary)

Linux and macOS (x86_64 and arm64):

```sh
curl -fsSL https://raw.githubusercontent.com/prongbang/zync/main/install.sh | sh
```

This installs the `zync` binary (the web UI is embedded in it). Then run it and
open <http://127.0.0.1:58271>:

```sh
ZYNC_SECRET_KEY="$(openssl rand -base64 32)" zync serve
```

A system `git` (and `ssh` for SSH remotes) must be installed. Override the
version or install location with `ZYNC_VERSION` and `ZYNC_INSTALL_DIR`.

### First run / sign in

Zync requires a login by default. Two ways to start it:

**Quick local run, no login (single-user)** — the simplest way to try Zync on
your own machine:

```sh
ZYNC_SECRET_KEY="$(openssl rand -base64 32)" ZYNC_AUTH=disabled zync serve
```

Then open <http://127.0.0.1:58271> — it opens straight in, no sign-in. This
mode has no authentication, so only use it on a trusted machine / localhost —
don't expose it on a network without auth.

**With a login (multi-user, or anything reachable beyond localhost)** — set
`ZYNC_ADMIN_USER` (the login email) and `ZYNC_ADMIN_PASSWORD` on first start
to create the admin account:

```sh
ZYNC_SECRET_KEY="$(openssl rand -base64 32)" \
  ZYNC_ADMIN_USER="you@example.com" \
  ZYNC_ADMIN_PASSWORD="a-strong-password" \
  zync serve
```

Then open <http://127.0.0.1:58271> and sign in with that email and password.
The admin is created once, in `zync.db` (path via `ZYNC_DB`) — later starts
ignore `ZYNC_ADMIN_*`, so don't delete the DB or change `ZYNC_SECRET_KEY`
afterwards. If you start without `ZYNC_ADMIN_*` (and without
`ZYNC_AUTH=disabled`), follow the one-time `/setup?token=...` link (valid
~24h) that the server logs on boot instead.

### Docker

```sh
docker compose up --build
```

Then open <http://127.0.0.1:58271>. Mount host Git projects under `/workspaces`
in `docker-compose.yml` and add the mounted path in the UI. Set the same
`ZYNC_ADMIN_USER`/`ZYNC_ADMIN_PASSWORD` (or `ZYNC_AUTH=disabled`) in
`docker-compose.yml` for first-run sign-in, as above.

### From source

Requires Rust and [bun](https://bun.sh):

```sh
cd web && bun install && cd apps/web && bun run build    # build the web UI
cargo build --release -p zync-server --features embed-ui # embed it into the binary
ZYNC_SECRET_KEY="$(openssl rand -base64 32)" ./target/release/zync serve
```

---

Production deployment, backups, and the HTTP API are documented in
[docs/DEPLOY.md](docs/DEPLOY.md), [docs/BACKUP.md](docs/BACKUP.md), and
[docs/API.md](docs/API.md).
