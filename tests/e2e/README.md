# tests/e2e

A repeatable Playwright click-through for the Zync web UI, plus a standalone
git/API fixture builder.

The UI under test is the React app in `web/apps/web/src` (Vite dev server by
default). It drives the app through stable `data-testid` hooks added to the
React components, not CSS classes.

Files:

- `fixture.cjs` - `buildFixture(baseDir, options?)` creates a bare `origin.git`, a
  working clone with 3+ commits, pushes `main` to `origin`, and registers the
  working clone with a running `zync-server` via `POST /repositories`. By
  default it also leaves one dirty tracked file and one untracked file in the
  working clone (for `audit.cjs`'s Local Changes flows); pass
  `{ dirty: false }` for a working clone whose tree is clean and matches
  `origin/main` exactly (used by `remote.cjs`, whose fetch/pull/push flows
  need a tree that never blocks a fast-forward). `cleanup(fixture)` removes
  the registered repository again via `DELETE /repositories/:id`. Also
  exports `git(args, cwd)`, the small `execFileSync` wrapper it builds the
  fixture with, for reuse by other scripts. Talks only to git (CLI) and the
  server API - no browser required.
- `audit.cjs` - builds a fresh fixture in a temp directory
  (`os.tmpdir()/zync-e2e-*`), drives the UI through the local-changes /
  branch / dialog click-through flows below, prints `PASS`/`FAIL`/`SKIP` per
  step, always tears the fixture down in a `finally` block, and exits with
  code `1` if any step failed.
- `remote.cjs` - a sibling script (same conventions as `audit.cjs`) covering
  the remote-operation flows below: fetch, push, pull (ff-only / merge /
  rebase), force-with-lease (accept and reject), credentials CRUD, and the
  remotes tab. Builds its own fixture with `buildFixture(baseDir, { dirty: false })`
  plus a second, independent clone of the same bare origin (to simulate
  another machine pushing to the remote without the browser-driven working
  clone ever fetching it - used by the pull and stale-force-with-lease
  flows). Kept separate from `audit.cjs` because it needs that second clone
  and deliberately exercises negative/error paths that would be noisy
  interleaved with `audit.cjs`'s happy-path run.

## Prerequisites

Both servers must already be running:

```bash
# Terminal 1 - API on 127.0.0.1:58271
cargo run -p zync-server

# Terminal 2 - Vite dev server for the React UI on 127.0.0.1:5173
cd web && bun run dev --filter=web
# (or: cd web/apps/web && bun run dev)
```

The React app's API client (`web/apps/web/src/lib/api.ts`) derives the API
base URL from the browser location the same way the legacy Dioxus UI did:
dev ports map to `58271`; any other port is assumed to already be the API
port. No extra configuration is needed for the default dev setup.

Credential creation (see "Remote flows" below) requires the server to have
been started with a secret key configured (e.g. `ZYNC_DEV=1`, which uses a
fixed dev key) - without one, `POST /credentials` returns `503` and the
credentials flows in `remote.cjs` fail.

## Running

```bash
cd tests/e2e
npm install
npm run audit          # audit.cjs then remote.cjs - exits 1 if either fails
npm run audit:core     # audit.cjs only
npm run audit:remote   # remote.cjs only
```

## Target origin: `E2E_BASE_URL`

Both `audit.cjs` and `remote.cjs` navigate to `E2E_BASE_URL` (default
`http://127.0.0.1:5173`, the Vite dev server). Override it to point at a
different origin, e.g. when `zync-server` is serving a production UI build
same-origin on `58271`:

```bash
E2E_BASE_URL=http://127.0.0.1:58271 npm run audit
```

In that case the API client resolves to the same `58271` origin per the port
mapping above, so `E2E_API_BASE` (used only by `fixture.cjs` to register/clean
up the fixture repository, default `http://127.0.0.1:58271`) does not need to
change.

## Flows covered

### `audit.cjs`

- Repo rail: switch to the fixture repository via its `RepoMinibar` entry
  (`data-testid="repo-minibar-item"`, matched by its `data-repo-id`, so it is
  exact even when other repositories are already registered on the same
  server)
- Local Changes: row select, stage/unstage, stage a single hunk, stage an
  untracked file
- Diff view toggles: Inline / Split / Blame (verified via `aria-pressed` on
  the corresponding `diff-inline` / `diff-split` / `diff-blame` toggle)
- Commit via the footer composer (`commit-input` + `commit-btn`)
- Toolbar Fetch / Pull / Push against the fixture's `file://` origin
  (verified via the footer `notice` text)
- Sidebar branch checkout (clicking a branch row)
- New Branch dialog, including the "Stash and reapply" local-changes option
- Merge, New Tag, Rename, and Delete branch dialogs, all opened via the
  sidebar branch row's right-click context menu
- Detail tabs: Commit, Git Tools, Repository (only rendered in "All Commits"
  mode - see "Known non-bugs handled deliberately" below)
- Repository stats panel
- All Commits mode: Load more, commit row select

### `remote.cjs`

- Lets the app's own default-repository auto-open settle, then switches to
  the fixture repo and waits for its live-sync WebSocket to connect (see
  "Known non-bugs / bugs found" below for why both matter)
- Toolbar Fetch against the fixture's `file://` origin
- Push: a locally-created commit is pushed via the toolbar, then verified by
  comparing `git rev-parse HEAD` in the working clone against
  `git rev-parse main` in the bare origin directly (not just the UI notice)
- Pull (fast-forward only): a second, independent clone pushes a commit to
  the bare origin directly (simulating another client); toolbar Pull is
  asserted to bring that commit into the graph
- Pull menu: merge mode and rebase mode, each against a freshly-advanced
  origin, asserting the request succeeds
- Force push with lease: an accept case (local `HEAD` amended, no
  divergence since the last fetch) and a reject case (the origin is advanced
  behind the fixture's back via the second clone with no fetch in between,
  so the lease must be rejected as stale) - both verified against the bare
  origin directly, and the reject case additionally asserts the UI is still
  responsive afterward (a follow-up Fetch completes normally, not a hang)
- Git Tools -> Credentials tab: add an `https_token` credential, delete it,
  and a negative case where an invalid host pattern (`*bad.com`, missing the
  required `.` after the wildcard) surfaces a field-level error and keeps
  the dialog open rather than submitting
- Git Tools -> Remotes tab: the `origin` remote row shows its `file://` URL;
  add a second remote, then delete it

### Known non-bugs handled deliberately

- The commit composer is targeted via the `commit-input` / `commit-btn`
  `data-testid`s, not a bare `input`/`textarea` selector.
- The Commit/Repository/Git Tools detail tabs are only rendered while the
  commit section is in "All Commits" mode (see the `mode === "commits"`
  branch in `web/apps/web/src/App.tsx`) - the scripts only target them after
  switching to that mode, never while "Local Changes" is active.
- Branch/local-change rows carry an exact-match data attribute
  (`data-branch-name` / `data-path`) alongside their `data-testid`, so lookups
  don't fall prey to substring collisions between fixture names (e.g. a
  branch and its renamed sibling both containing the same prefix).
- Branch context menus are real right-click (`contextmenu`) menus (Base UI's
  `ContextMenu`), not click-to-open dropdowns - the scripts always open them
  with `row.click({ button: 'right' })`.
- The stash-apply dialog (`StashApplyDialog`) exists as a component but has no
  UI entry point yet - nothing in `App.tsx` ever sets the `stashApply` dialog
  kind, because the sidebar has no stash list. `audit.cjs` logs this as a
  `SKIP`, not a `FAIL`, until that surface is built.
- **`remote.cjs` deliberately waits for the app's default-repository
  auto-open to settle before switching to the fixture tab, and then waits
  again for the fixture's own live-sync WebSocket to connect before doing
  any fetch/pull/push.** This is not just defensive test pacing: `App.tsx`
  auto-opens the first registered repository on initial load
  (`useEffect` keyed on `!ws.workspace && ws.repositories.length > 0`), and
  `useWorkspace.ts`'s `openRepository()` has no guard against a
  *later*-resolving call overwriting a repo switch that already happened. In
  a fresh page load with other repositories already registered (as in this
  environment), switching to a different repo very soon after load can let
  the default repo's *delayed* `openRepository()` resolution land afterward
  and silently reconnect the live-sync socket (and re-run the full
  `refresh(SCOPE_ALL)`) against the *stale, no-longer-selected* default
  repo - observed here as the footer `notice` jumping from a just-completed
  op's success text (e.g. "Push complete") back to "Live sync connected" for
  an unrelated repository. `audit.cjs` never hits this because it does many
  UI steps between switching repos and the first remote op, which happens to
  give the stray default-repo open time to settle; `remote.cjs` makes that
  wait explicit instead of relying on incidental pacing. This looks like a
  real, reproducible race in `openRepository`/the default-repo auto-open
  effect worth fixing upstream (no request is cancelled/ignored based on
  which repo is still selected when it resolves) - it is *worked around*
  here for a deterministic e2e run, not fixed at the source.

## Safety

The fixtures always create brand-new temporary repositories and only ever
register/remove those repositories via the API. They never touch
pre-existing registered repositories (in this environment: `zync`, `Orca`,
`appmo`, `vane`).
