# Zync

Zync is a cross-platform Git workspace manager inspired by Fork.

The current implementation is a Rust workspace with:

- `crates/git-core`: libgit2-backed Git operations.
- `crates/server`: Axum API, SQLite persistence, WebSocket workspace events, and collaboration state.
- `crates/ui`: Dioxus UI scaffold for repository, explorer, status, diff, graph, branch, stash, rebase, conflict, and collaboration views.

See [PLAN.md](PLAN.md) for the full implementation roadmap.

## UI Assets

The Dioxus UI vendors Tailwind CSS at `crates/ui/src/tailwind.min.css` and embeds it before the app-specific `style.css`. The UI can render without waiting on an external CDN, while local Fork-like styling still overrides Tailwind where needed.

## Run

```sh
cargo run -p zync-server
```

The server listens on `127.0.0.1:58271` by default.

## Mounted Repository Flow

Zync manages repositories that the server process can see on disk.

For local development:

```sh
cargo run -p zync-server
```

Then open the web UI build and add a repository path such as:

```text
/Users/you/Development/my-git-project
```

For Docker, mount host projects into `/workspaces` and add the container path in the UI:

```yaml
volumes:
  - /Users/you/Development/my-git-project:/workspaces/my-git-project
```

Then add:

```text
/workspaces/my-git-project
```

When the workspace is opened, the server attaches a watcher. File changes made on the mounted filesystem are batched through `/ws/workspace/:id`, and the browser refreshes the workspace status, diff, files, branches, and commit graph.
