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
- `crates/ui`: legacy Dioxus web UI. Excluded from the Cargo workspace; kept on
  disk for reference pending removal.

## Features

- Add local repositories or clone a remote repository into a local path.
- Watch opened repositories and refresh status, diffs, files, branches, stashes,
  conflicts, and commit graph through workspace events.
- Review local changes, stage or unstage files, inspect changed files, and commit
  from the footer composer.
- Commit with amend, sign-off, and optional push-after-commit controls.
- Fetch, pull, and push against remotes.
- Browse commit history with an SVG lane graph (curved merge connectors),
  branch/tag badges on commit rows, author, short SHA, and formatted dates.
- Inspect commits with author/committer identities, ref badges, SHA, and parent
  links in the detail panel.
- Review side-by-side diffs with aligned removal/addition pairs and word-level
  change highlighting.
- Scroll large histories smoothly: the commit list is virtualized and live
  sync refreshes only the data each event actually changed.
- View repository statistics (commit count, contributors, commits per month)
  from the Repository tab, and branch ahead/behind markers in the sidebar.
- Switch between registered repositories from a Fork-style tab strip.
- Create branches with Fork-style local-changes handling: keep them in place,
  stash and reapply after checkout, or discard.
- Right-click any commit for Fork-style actions: new branch/tag, rebase or
  interactive rebase to here, reword/edit/squash/fixup/drop, reset, checkout,
  cherry-pick, revert, save as patch, compare to local changes, and copy SHA.
- See Gravatar avatars for authors and committers, and inspect per-line blame
  (commit, author, code) for any file from the diff panel.
- Checkout branches, merge branches, rebase branches, create branches, create
  tags, rename branches, delete branches, and copy branch names from the branch
  menu.
- Checkout a branch or revision, create a branch from a revision, create and
  delete tags, cherry-pick commits, revert commits, and run rebase controls from
  Git Tools.
- Manage stashes, remotes, remote branches, upstreams, and submodules.
- Remove repositories from the Zync registry without deleting the repository from
  disk.

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

Vite proxies `/repositories`, `/workspace`, `/files`, `/auth`, `/collaboration`,
`/health`, and `/ws` to the API at `http://127.0.0.1:58271` in dev (see
`web/apps/web/vite.config.ts`). In production the server serves the built app
same-origin, so no proxy is involved.

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
