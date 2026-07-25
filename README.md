# Zync

Zync is a minimal Git workspace client inspired by Fork. It is built as a Rust
workspace with an Axum API and SQLite-backed repository registry, and a React
19 + Vite + TypeScript web app for day-to-day Git operations.

## Workspace

- `crates/git-core`: libgit2-backed Git operations.
- `crates/server`: Axum API, repository persistence, WebSocket workspace events,
  Git command endpoints, and — in production — serves the built React app.
- `web/`: bun + turbo monorepo. `web/apps/web` is the React web UI (Vite +
  React 19 + TypeScript + Tailwind v4 + shadcn/ui) for repository management,
  local changes, commit history, branch actions, and Git tools. Shared shadcn/ui
  primitives live in `web/packages/ui` (`@workspace/ui`).

## Features

**Repositories & onboarding**

- Add an existing local repository, clone from a URL, or `git init` a new
  repository — all from the UI, with a built-in directory browser.
- Switch between registered repositories from a collapsible project rail
  (monogram strip that expands to full names), favorite them, or remove them
  from the registry without deleting anything on disk.
- Watch opened repositories and live-refresh only the data each filesystem or
  git event actually changed (status, diffs, files, branches, tags, stashes,
  conflicts, commit graph) over a WebSocket.

**Remotes & credentials**

- Fetch, fetch-all, pull (fast-forward / merge / rebase), and push — with
  ahead/behind badges, force-push (with lease) behind a confirm, and
  publish-branch (set-upstream) prompts.
- Per-user encrypted credential store (HTTPS token or SSH key), so cloning,
  fetching, pulling, and pushing private repositories works from the UI on a
  host with no ambient git credentials. Secrets are encrypted at rest
  (XChaCha20Poly1305) and never returned, logged, or echoed in errors.
- Manage remotes (add / edit URL / rename / delete / prune, per-remote
  fetch/pull/push) from the Git Tools panel.

**History, search & diff**

- Browse commit history with a virtualized SVG lane graph (curved merge
  connectors), ref badges, author, short SHA, and dates.
- Select any commit to see its full multi-file diff with a navigable file tree,
  per-file inline/split views, word-level highlighting, and image before/after
  previews.
- Search commits by message / author / SHA (with in-window dimming) and across
  full history including by touched file path.
- Per-file history and per-line blame, with blame → jump-to-commit and
  "open file at revision".

**Branching & rewriting**

- Create branches with Fork-style local-changes handling (keep / stash-reapply /
  discard); checkout, merge (fast-forward / no-ff / squash), rebase a branch,
  rename, delete, and drag a branch onto the current one to merge or rebase.
- Tags panel: create, checkout, push, copy SHA, and delete (annotated and
  lightweight).
- Commit context menu: new branch/tag, rebase / interactive rebase to here,
  reword/edit/squash/fixup/drop, reset, checkout, cherry-pick, revert (including
  reverting merge commits with a mainline picker), save as patch, compare to
  local changes, copy SHA.
- Interactive rebase editor: a draggable / keyboard-reorderable todo list with
  per-row pick/reword/edit/squash/fixup/drop and precondition guards.

**Power tools & ergonomics**

- Git Tools panel: reflog (checkout / branch / reset here), submodules
  (init/update/sync/add/remove), and Git LFS (track/untrack, pull/push).
- Command palette (⌘/Ctrl-P) over repos, branches, recent commits, and every
  action, plus a keyboard-shortcut map and cheat sheet.
- `git bisect` with a status banner and mark-good/bad/skip/reset.
- Repository statistics (commit count, contributors, commits per month) and a
  conflict resolver (ours/theirs) for merges and rebases.
- Responsive layout: resizable panels on desktop, a single-column + sheet layout
  on mobile.

## Run

### Docker

Start the full app with the API and web UI in one container:

```sh
docker compose up --build
```

Then open:

```text
http://127.0.0.1:58271/
```

The container stores Zync state in the `zync-data` volume. Mount host Git
projects under `/workspaces` in `docker-compose.yml`, then add the mounted path
in the UI, for example `/workspaces/my-git-project`.

### Local development

Start the API server:

```sh
ZYNC_BIND=0.0.0.0:58271 cargo run -p zync-server
```

Start the React app:

```sh
cd web/apps/web
bun run dev --port 5173 --host
```

Then open:

```text
http://127.0.0.1:5173/
```

Vite proxies `/repositories`, `/workspace`, `/auth`, `/credentials`,
`/directories`, `/health`, and `/ws` to the API at `http://127.0.0.1:58271` in
dev (see `web/apps/web/vite.config.ts`). In production the server serves the
built app same-origin, so no proxy is involved.

To use the encrypted credential store, set `ZYNC_SECRET_KEY` (a base64-encoded
32-byte key) so stored HTTPS tokens and SSH keys can be encrypted at rest;
without it, credential operations are disabled with a clear error. For local
development only, `ZYNC_DEV=1` falls back to a fixed all-zero key (never use it
for real secrets — the database must not leave the dev machine).

## Repository Flow

Zync manages repositories that the server process can see on disk.

For local development, add a repository path such as:

```text
/Users/you/Development/my-git-project
```

For Docker or remote containers, mount host projects into a path visible to the
server and add the mounted path in the UI:

```yaml
volumes:
  - /Users/you/Development/my-git-project:/workspaces/my-git-project
```

Then add:

```text
/workspaces/my-git-project
```

When a workspace is opened, the server attaches a watcher and batches filesystem
changes through `/ws/workspace/:id`, so the browser can refresh the workspace
state without manual reloads.

## Checks

```sh
cargo check -p zync-git-core -p zync-server
cargo test -p zync-git-core
```

```sh
cd web && bun install
cd apps/web && bun run typecheck
bun run build   # outputs web/apps/web/dist
```

End-to-end (Playwright, reads `E2E_BASE_URL`, default `http://127.0.0.1:5173`):

```sh
cd tests/e2e && npm install && npm run audit
```

## Notes

- `PLAN.md` is a local planning file and is intentionally ignored by Git.
- Runtime state such as `zync.db` is local-only and should not be committed.
