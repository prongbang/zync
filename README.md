# Zync

Zync is a minimal Git workspace client inspired by Fork. It is built as a Rust
workspace with an Axum API, SQLite-backed repository registry, and a Dioxus web
UI for day-to-day Git operations.

## Workspace

- `crates/git-core`: libgit2-backed Git operations.
- `crates/server`: Axum API, repository persistence, WebSocket workspace events,
  and Git command endpoints.
- `crates/ui`: Dioxus web UI for repository management, local changes, commit
  history, branch actions, and Git tools.

## Features

- Add local repositories or clone a remote repository into a local path.
- Watch opened repositories and refresh status, diffs, files, branches, stashes,
  conflicts, and commit graph through workspace events.
- Review local changes, stage or unstage files, inspect changed files, and commit
  from the footer composer.
- Commit with amend, sign-off, and optional push-after-commit controls.
- Fetch, pull, and push against remotes.
- Browse commit history with a compact graph, selected-row state, changed files,
  and commit diffs.
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

Start the API server:

```sh
ZYNC_BIND=0.0.0.0:58271 cargo run -p zync-server
```

Start the Dioxus UI:

```sh
dx serve --web --package zync-ui --port 8080 --addr 0.0.0.0 --open false
```

Then open:

```text
http://127.0.0.1:8080/
```

The UI talks to the API at `http://127.0.0.1:58271` by default. When the UI is
served from another host, it uses the same hostname with port `58271`.

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
cargo check -p zync-ui
cargo check --target wasm32-unknown-unknown -p zync-ui
cargo check -p zync-server
```

## Notes

- `PLAN.md` is a local planning file and is intentionally ignored by Git.
- Runtime state such as `zync.db` is local-only and should not be committed.
