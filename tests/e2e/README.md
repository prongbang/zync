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
  `origin/main` exactly (used by `remote.cjs` and `features.cjs`, whose flows
  need a tree that starts clean - fast-forwards for `remote.cjs`, interactive
  rebase's clean-tree guard for `features.cjs`). `cleanup(fixture)` removes
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
- `features.cjs` - covers the P1 surfaces (P1.8 in PLAN.md): tags, commit
  search, the diff file tree, image diff, per-file history + blame, and
  interactive rebase (including the drop-then-squash guard). Builds its own
  `buildFixture(baseDir, { dirty: false })` fixture (the interactive rebase
  steps need a clean tree to start from) and extends it with plain `git`
  calls via `fixture.cjs`'s exported `git()` - a baseline+modified image pair
  for the image-diff flow, and a post-rebase commit so file history has more
  than one entry to pick between. See "Known non-bugs handled deliberately"
  below for why most of its steps call a `forceRefresh()` helper after
  mutating the fixture's filesystem directly (outside the browser).

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
npm run audit          # audit.cjs, remote.cjs, then features.cjs - exits 1 if any fail
npm run audit:core     # audit.cjs only
npm run audit:remote   # remote.cjs only
npm run audit:features # features.cjs only
```

## Target origin: `E2E_BASE_URL`

`audit.cjs`, `remote.cjs`, and `features.cjs` all navigate to `E2E_BASE_URL`
(default `http://127.0.0.1:5173`, the Vite dev server). Override it to point
at a different origin, e.g. when `zync-server` is serving a production UI
build same-origin on `58271`:

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

### `features.cjs`

- Tags (P1.1): create a tag on `main` via the branch context menu's "New
  Tag...", confirm its `tag-row` appears in the sidebar Tags section, open
  its `tag-context-menu`, "Copy SHA" (verified via the footer notice), then
  "Delete..." (confirmed) and the row is gone
- Commit search (P1.3): typing an author query updates
  `search-result-count` and dims (`opacity-40`) non-matching rows in-place;
  Clear resets both; "Search all history" (shown because the fixture's match
  count is under the few-matches threshold) swaps in `historyResults` and
  "Back to graph" returns to the live graph
- Interactive rebase (P1.6) - run **before** the dirty-tree steps below,
  since it needs a clean working tree: opens `interactive-rebase-dialog`
  from a commit, exercises the drop-then-squash guard (row 0 = Drop, a later
  row = Squash -> `rebase-execute` stays disabled because the first
  *kept* row, not literally row 0, can't squash/fixup into nothing), then a
  plain squash plan is executed and the commit count drop is verified via
  `git rev-list --count HEAD` against the real fixture on disk
- Diff file tree (P1.4): see the note below - exercised via a multi-file
  *workdir* diff (two dirtied tracked files), since that is the only
  reachable multi-file diff surface today. `diff-file-tree` lists both
  files; clicking a `diff-file-row` swaps the diff pane to that file
- Image diff (P1.5): commits a small (real, decodable) baseline PNG, then
  modifies it uncommitted; the DiffPanel shows `diff-image-before` (HEAD) /
  `diff-image-after` (`:workdir` sentinel) with distinct `src` URLs, and both
  `<img>` elements are asserted to actually decode (`naturalWidth > 0`) in
  the browser, not just be present in the DOM. Also asserts at the API level
  that the raw-blob route serves the PNG with `Content-Type: image/png` and
  `X-Content-Type-Options: nosniff`
- File history + blame (P1.2): selects a file, opens `open-file-history`
  from the DiffPanel header, confirms `file-history-view` lists 2+
  `file-history-row` entries (a post-rebase commit is added so this is true
  even after the rebase step above reduces history), selects a different
  row and confirms the diff pane's header changes; separately, toggles the
  DiffPanel to Blame, confirms `blame-commit-link` is present, and clicking
  it selects that commit (verified by switching to the Commit detail tab and
  comparing the full SHA shown there)

Each dirty-tree step in `features.cjs` mutates the fixture's filesystem
directly (`fs.appendFileSync` / `git commit` in the working clone) rather
than through the browser, then calls a local `forceRefresh(page)` helper
(clicks Toolbar Fetch and waits for "Fetch complete") before asserting on
the UI. This is a deliberate workaround for a real bug - see "Known non-bugs
handled deliberately" below.

**Note on diff-file-tree and "select a multi-file commit":** there is
currently no UI path to view a *selected commit's* full multi-file diff.
`CommitGraph` never fetches a commit's diff, and the "Commit" detail tab
(`App.tsx`'s `CommitDetail`, shown in All Commits mode) renders only
metadata (author/SHA/parents/message) - never a diff. `DiffPanel` (the
component that owns `diff-file-tree`) is only ever mounted while the center
pane is in "Local Changes" mode, fed `ws.diff`, which defaults to the whole
*workdir* diff (`api.diffWorkdir`) - a multi-file patch once 2+ files are
dirty. The commit-context-menu's "Compare to local changes..." action
(`compare-local`) does fetch a commit-vs-workdir diff into `ws.diff`, but
doesn't switch the center pane to "Local Changes" mode itself, so nothing
visibly changes unless the user was already on that tab. `features.cjs`
exercises the same `DiffFileTree`/`DiffFileRow` component the only way it's
actually reachable today: a multi-file workdir diff.

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
- **The live-sync file watcher (`WorkspaceSync` /
  `spawn_workspace_watcher` in `crates/server/src/sync/mod.rs`) does not
  appear to ever deliver `workspace_batch` events for filesystem changes
  made outside a mutating git route, at least in this environment - a real,
  reproducible bug, not a test artifact.** Verified independently of the
  browser: a raw `WebSocket` client subscribed to
  `/ws/workspace/:id` receives **zero** messages for 15+ seconds after
  creating or modifying a file directly in a freshly-opened (watcher
  registered) workspace's working tree, even though `GET
  /repositories/:id/git/status` reflects the change immediately (a plain
  filesystem read, independent of the watcher) - so the server process can
  see the files fine, it's specifically the `notify`-crate-based watch that
  never fires or never reaches the hub. The websocket/broadcast pipeline
  itself is not at fault: the same test, using a mutating git route (`POST
  .../git/branches`) instead of a raw fs write, reliably delivers a
  `git_changed` event within ~1s. Net effect: a file edited by another tool
  while Zync's tab is open (the file watcher's entire purpose) does not
  live-refresh the UI; the user has to trigger some other action (a git
  mutation via the UI, a manual repo re-open, etc.) before Local
  Changes/diff reflects it. `features.cjs`'s dirty-tree steps work around
  this with a `forceRefresh(page)` helper (clicks Toolbar Fetch, whose
  success handler calls `refresh(SCOPE_ALL)` client-side regardless of any
  websocket event - see `runRemote` in `useWorkspace.ts`) rather than
  waiting on the live-sync path the same way a real "external edit" flow
  would. This is worth investigating upstream - not fixed here.

## Safety

The fixtures always create brand-new temporary repositories and only ever
register/remove those repositories via the API. They never touch
pre-existing registered repositories (in this environment: `zync`, `Orca`,
`appmo`, `vane`).
