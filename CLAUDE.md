# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Zync is a Fork-inspired Git workspace client: a Rust workspace with an Axum API server, a SQLite-backed repository registry, and a React 19 + Vite + TypeScript web app. The server operates on Git repositories that exist on disk where the server process runs (local paths in dev, mounted volumes in Docker) — there is no server-side clone storage beyond what the user registers.

## Workspace layout

- `crates/git-core`: all libgit2 (`git2`) operations, pure functions over a repo path. No knowledge of HTTP, the DB, or the UI. This is where every actual Git command lives (status, diff, commit, branch/tag/stash/remote CRUD, cherry-pick, revert, reset, interactive rebase, blame, reflog, submodules, LFS passthrough).
- `crates/server`: Axum API. Each subsystem is a module with its own `routes()` fn merged into the router in `main.rs`: `auth`, `repository`, `workspace`, `files`, `git`, `websocket`, `collaboration`. `db` wraps a single-connection (`Arc<Mutex<Connection>>`) SQLite store for `users`, `repositories`, `workspaces`, `workspace_members`, `sessions`. `sync` runs the filesystem watcher. In production it also serves the built React app from `ZYNC_STATIC_DIR`.
- `web/`: bun + turbo monorepo. `web/apps/web` is the React app: `src/lib/api.ts` is the typed HTTP client (one method per server endpoint, base `""` = same-origin), `src/lib/types.ts` mirrors the server's DTOs, `src/lib/helpers.ts` holds pure logic (graph row layout, diff parsing, blame, formatting — ported from the old Rust helpers), `src/lib/format.ts` (gravatar), and `src/lib/useWorkspace.ts` owns all workspace state, the live-sync WebSocket reconnect loop, and mutation actions. Presentational components live under `src/components/` (`CommitGraph`, `DiffPanel`, `Toolbar`, `BranchSidebar`, `RepoStatsPanel`, `ConflictResolver`, `GitToolsPanel`, `dialogs/*`). Shared shadcn/ui primitives live in `web/packages/ui` (`@workspace/ui`); the shadcn skill workflow (`web/.agents/skills/shadcn/SKILL.md`) governs component additions and Tailwind usage for all React UI work.

## Commands

Build/check (no `cargo test`/`cargo build` workspace-wide shortcut exists in CI; use per-crate):

```sh
cargo check -p zync-git-core
cargo check -p zync-server
```

Tests (only `git-core` has tests, using real `git2::Repository::init` in a tempdir — no mocking):

```sh
cargo test -p zync-git-core
cargo test -p zync-git-core --test git_core status_add_commit_and_branch_flow   # single test
```

Web app checks (from `web/`, a bun + turbo monorepo):

```sh
bun install
cd apps/web && bun run typecheck
bun run build   # tsc -b && vite build, outputs web/apps/web/dist
```

Run locally (two processes: API + Vite dev server):

```sh
ZYNC_BIND=0.0.0.0:58271 cargo run -p zync-server
cd web/apps/web && bun run dev --port 5173 --host
```

Vite proxies backend route prefixes (`/repositories`, `/workspace`, `/auth`, `/credentials`, `/directories`, `/health`, and the `/ws` WebSocket) to the API at `http://127.0.0.1:58271` in dev — `files::routes()`/`collaboration::routes()` only register nested `/workspace/:id/...` paths, so they ride on `/workspace` rather than needing their own prefix — see `web/apps/web/vite.config.ts`, which must stay in sync with the route modules merged in `crates/server/src/main.rs`. In production the app is served same-origin, so no proxy target matters there.

Docker (production-shaped build/run):

```sh
docker compose up --build
```

`Dockerfile` is a three-stage build: an `oven/bun` stage runs `bun install` and `bun run build` for `web/apps/web` (output `web/apps/web/dist`), a `rust` stage builds a release `zync-server` binary, and a `debian:bookworm-slim` final stage copies both — the server serves the static React build from `ZYNC_STATIC_DIR` (default `/app/public`) with an index.html SPA fallback for any unmatched route.

End-to-end tests live in `tests/e2e` (Playwright, plain npm): with both dev servers running, `cd tests/e2e && npm install && npm run audit` clicks through the UI flows in `audit.cjs` against a self-registered fixture repo. The audit targets `E2E_BASE_URL` (default `http://127.0.0.1:5173`); CI can point it instead at `http://127.0.0.1:58271` to exercise the production build served by `zync-server`.

## Architecture notes that span files

**Repository → Workspace → live sync.** A "repository" (DB row: id/name/path) is registered once; opening it creates/reuses a "workspace" (DB row tied to a repository). The UI subscribes to `/ws/workspace/:id`. `WorkspaceSync` (`crates/server/src/sync/mod.rs`) spawns one OS-thread file watcher per (workspace, root) pair, debounces raw fs events (120ms), and batches them into a single `workspace_batch` `WorkspaceEvent`. Every mutating Git route in `crates/server/src/git/mod.rs` also explicitly calls `broadcast_git_change(&state, repository_id, &["scope", ...])` after a successful mutation, publishing a `git_changed` event whose payload lists which scopes changed (`status`, `diff`, `branches`, `commits`, `stashes`, `conflicts`, `workspace`).

**Scoped refresh, not full reload.** On the frontend, `scopeForEvent()` (`web/apps/web/src/lib/helpers.ts`) maps an incoming websocket event to a bitmask of which pieces of state to refetch; `useWorkspace` (`web/apps/web/src/lib/useWorkspace.ts`) only calls the API endpoints for those scopes and coalesces overlapping in-flight refreshes by merging scope bitmasks (a refresh already in-flight absorbs a newly-requested scope rather than starting a second network round-trip). When adding a new mutating git endpoint, add its scopes to `broadcast_git_change` on the server side and confirm `scopeForEvent`/`scopeBit` on the frontend cover it — otherwise other connected clients (or other tabs) silently miss the update.

**Commit graph model.** `graphRows()` (`web/apps/web/src/lib/helpers.ts`) builds `GraphRow`s (lane index, `topLanes`/`bottomLanes`/`mergeLanes` sets) from the flat `CommitSummary` list purely on the frontend — the server does no lane layout. `CommitGraph` (`web/apps/web/src/components/CommitGraph.tsx`) memoizes it and renders each row's lane cell as SVG (straight rails + bezier merge curves), not CSS grid — don't reintroduce class-based lane rendering.

**Virtualized commit list.** `CommitGraph` only renders the commit rows in the current scroll window (fixed 34px row height, `ROW_HEIGHT`/`OVERSCAN_ROWS` constants in `CommitGraph.tsx`, computed from scroll offset + viewport height) plus top/bottom spacers. Any change to row markup must keep every row exactly 34px tall or the windowing math drifts.

**Interactive rebase is the mechanism behind most "quick" commit actions.** Reword/Edit/Squash/Fixup/Drop-on-a-single-commit (from the commit context menu) are implemented client-side as `quickRebasePlan()` (`web/apps/web/src/lib/helpers.ts`): reset to the target commit's parent, apply one rebase step action to it, then re-`pick` every descendant already loaded in the graph. This only works for linear history (single-parent commits) currently — `quickRebasePlan` deliberately errors out on merge commits rather than guessing. The actual rebase execution (`interactive_rebase` in git-core) requires a clean working tree and will error otherwise.

**One diff panel, two render paths, same hunk parser.** `diffHunks()`/`splitDiffLines()` (`web/apps/web/src/lib/helpers.ts`) parse a unified-diff string (from git-core, which explicitly prepends the `+`/`-`/` ` origin byte in `diff_to_patch` — libgit2's `line.content()` omits it) into hunk/line structs. `DiffPanel` (`web/apps/web/src/components/DiffPanel.tsx`) renders `InlineDiffView` from `diffHunks()`, and `SplitDiffView` from `splitDiffLines()` — which pairs up adjacent removal/addition runs and computes word-level diff segments (a prefix/suffix common-substring trim) for the side-by-side view — behind a single Inline/Split/Blame toggle. Both stay driven by the same parser; extend `DiffPanel`/`helpers.ts` rather than adding a separate diff surface.

**Auth is a stub.** `crates/server/src/auth/mod.rs` and the seeded default user in `db::seed_default_user` exist but there is no real login flow wired into the frontend — treat single-user/no-auth as the current state, not a gap to silently "fix" unless asked.

**Theme.** `web/apps/web/src/zync-theme.css` implements the "Glass Teal Dark" design system by remapping shadcn/ui's semantic tokens onto the Zync palette and exposing raw `--zync-*` custom properties for bespoke surfaces (commit graph, diff, status dots) — see `DESIGN.md` for the full rationale. Practical rules: teal (`--zync-teal*`) is the only accent for active/selected/primary/focus states; amber is attention/added-file only; coral is destructive/conflict only; violet is untracked/secondary-lane only; never let blue dominate. New UI should reuse `--zync-*` tokens or shadcn semantic tokens (`bg-primary`, `text-muted-foreground`, etc. — see the shadcn skill) rather than introducing new hex values or raw Tailwind color utilities.

## Conventions specific to this repo

- `git-core` functions take `impl AsRef<Path>` for the repo path and return `anyhow::Result<T>`; they open a fresh `git2::Repository` per call rather than holding one open — follow this pattern for new functions instead of threading a `Repository` handle through.
- Server handlers resolve the repository via `state.db.repository(&id)` (or the shared `repository(&state, &id)` helper in `git/mod.rs`), map errors with a local `internal_error`/`(StatusCode, String)` helper, and — for anything that mutates the repo — end with a `broadcast_git_change` call before returning.
- New `ZyncApi` (`web/apps/web/src/lib/api.ts`) methods follow the existing `getJson`/`getText`/`postJson`/`postText`/`postEmpty`/`putEmpty` helper set; each throws the server's raw error body verbatim on non-2xx responses via `readOkOrThrow` rather than trying (and failing) to JSON-decode it — reuse it rather than hand-rolling `fetch` calls.
- `PLAN.md` is a local, gitignored scratch planning file (not part of the shipped docs) — `README.md` and `DESIGN.md` are the durable docs.
