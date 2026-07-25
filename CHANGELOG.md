# Changelog

All notable changes to Zync are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
(pre-1.0: minor version bumps may include breaking changes).

## [Unreleased]

## [0.1.0] - 2026-07-25

First tagged release. Zync is a Fork-inspired Git workspace client: a Rust/Axum
API server backed by SQLite, operating on Git repositories on disk, with a
React 19 + Vite web UI. This release folds together everything shipped across
plan phases P0-P6.

### Added

- **Remotes, credentials, and repo onboarding (P0)** — credentialed
  clone/fetch/pull/push (HTTPS token and SSH key, AEAD-encrypted at rest via
  `ZYNC_SECRET_KEY`), remotes manager, ahead/behind sync badges,
  force-with-lease and publish-branch flows, and an add/clone/init repository
  flow — all reachable without server-shell access.
- **History, search, and diff parity (P1)** — commit graph with lane layout,
  diff file tree, merge/revert, tag management, image diffs, inline/split diff
  views, and blame.
- **Power features & ergonomics (P2)** — reflog, submodules, and Git LFS tabs;
  command palette and keyboard shortcuts; drag-branch merge/rebase chooser;
  interactive rebase (reword/edit/squash/fixup/drop) driving the commit
  context menu; git bisect.
- **Real auth & multi-user (P3)** — argon2 password auth with HttpOnly
  session cookies, per-user repository ownership and `workspace_members`
  roles (owner/member/viewer), a login screen with a 401 redirect
  interceptor, WebSocket auth via a short-lived ticket, and member/user
  management UI. `ZYNC_AUTH=disabled` remains available as a single-user/LAN
  escape hatch.
- **Security hardening (P4)** — `ZYNC_REPOS_ROOT` filesystem boundary for
  repository registration, network hardening (same-origin CORS default with
  `ZYNC_CORS_ORIGINS` override, security headers/CSP, rate limiting, request
  body caps, `ZYNC_TRUSTED_PROXY`), a secret-hygiene audit (no plaintext
  secrets in logs/DB/API responses), and an argv-injection closure across all
  git shellout call sites.
- **Production platform (P5)** — SQLite productionization (WAL +
  `busy_timeout` + `foreign_keys` pragmas, versioned migrations, refuse-to-boot
  on migration failure), a Docker image (bun → rust → debian multi-stage
  build) with `docker-compose.yml`, and this release-engineering workflow
  (tag-triggered multi-arch image publish to `ghcr.io`).
- **Observability (P6)** — per-request `X-Request-Id` correlation (honored if
  the caller supplies a well-formed id, generated otherwise, always echoed back
  on the response), `ZYNC_LOG_FORMAT=json` for structured log output,
  `GET /health` (liveness, no I/O), `GET /ready` (readiness, a non-mutating DB
  read), and `GET /metrics` (Prometheus text exposition, gated to authenticated
  admins).
- **Single-binary distribution (P6)** — an `embed-ui` build feature that bakes
  the web UI into the `zync` binary, prebuilt single binaries for Linux and macOS
  (x86_64 + aarch64) attached to every GitHub Release, and a `curl … | sh`
  installer (`install.sh`) that verifies a SHA-256 checksum before installing.
  The binary needs only a system `git` at runtime.
- **Operational hardening & docs (P6)** — the container image now runs as a
  non-root user; `docs/BACKUP.md` (a `zync.db` backup/restore runbook covering
  WAL-mode online-backup hazards and the `ZYNC_SECRET_KEY` rotation caveat),
  `docs/DEPLOY.md` (the full `ZYNC_*` reference, nginx/Caddy TLS reverse-proxy
  examples, and health/readiness probe wiring), and a rewritten `docs/API.md`
  covering every endpoint; CI now gates the server test suite and the
  end-to-end suite.

### Fixed

- The live-sync footer indicator now resets per workspace, so it honestly
  reflects the current workspace's WebSocket rather than showing a stale
  "connected" state carried over from the previously open repository.

### Known limitations (tracked for a future release)

- Quick rebase actions (reword/edit/squash/fixup/drop from the commit context
  menu) only support linear history; merge commits are rejected rather than
  guessed at.

[Unreleased]: https://github.com/prongbang/zync/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/prongbang/zync/releases/tag/v0.1.0
