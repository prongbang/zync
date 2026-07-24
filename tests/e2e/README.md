# tests/e2e

A repeatable Playwright click-through for the Zync web UI, plus a standalone
git/API fixture builder.

The UI under test is the React app in `web/apps/web/src` (Vite dev server by
default). It drives the app through stable `data-testid` hooks added to the
React components, not CSS classes.

Files:

- `fixture.cjs` - `buildFixture(baseDir)` creates a bare `origin.git`, a
  working clone with 3+ commits, one dirty tracked file, one untracked file,
  pushes `main` to `origin`, and registers the working clone with a running
  `zync-server` via `POST /repositories`. `cleanup(fixture)` removes it again
  via `DELETE /repositories/:id`. Talks only to git (CLI) and the server API -
  no browser required.
- `audit.cjs` - builds a fresh fixture in a temp directory
  (`os.tmpdir()/zync-e2e-*`), drives the UI through the click-through flows
  below, prints `PASS`/`FAIL`/`SKIP` per step, always tears the fixture down
  in a `finally` block, and exits with code `1` if any step failed.

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

## Running

```bash
cd tests/e2e
npm install
npm run audit
```

## Target origin: `E2E_BASE_URL`

`audit.cjs` navigates to `E2E_BASE_URL` (default `http://127.0.0.1:5173`, the
Vite dev server). Override it to point at a different origin, e.g. when
`zync-server` is serving a production UI build same-origin on `58271`:

```bash
E2E_BASE_URL=http://127.0.0.1:58271 npm run audit
```

In that case the API client resolves to the same `58271` origin per the port
mapping above, so `E2E_API_BASE` (used only by `fixture.cjs` to register/clean
up the fixture repository, default `http://127.0.0.1:58271`) does not need to
change.

## Flows covered

- Repo tab strip: switch to the fixture repository (matched by its
  `data-repo-id`, so it is exact even when other repositories are already
  registered on the same server)
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

### Known non-bugs handled deliberately

- The commit composer is targeted via the `commit-input` / `commit-btn`
  `data-testid`s, not a bare `input`/`textarea` selector.
- The Commit/Repository/Git Tools detail tabs are only rendered while the
  commit section is in "All Commits" mode (see the `mode === "commits"`
  branch in `web/apps/web/src/App.tsx`) - the script only targets them after
  switching to that mode, never while "Local Changes" is active.
- Branch/local-change rows carry an exact-match data attribute
  (`data-branch-name` / `data-path`) alongside their `data-testid`, so lookups
  don't fall prey to substring collisions between fixture names (e.g. a
  branch and its renamed sibling both containing the same prefix).
- Branch context menus are real right-click (`contextmenu`) menus (Base UI's
  `ContextMenu`), not click-to-open dropdowns - the script always opens them
  with `row.click({ button: 'right' })`.
- The stash-apply dialog (`StashApplyDialog`) exists as a component but has no
  UI entry point yet - nothing in `App.tsx` ever sets the `stashApply` dialog
  kind, because the sidebar has no stash list. The script logs this as a
  `SKIP`, not a `FAIL`, until that surface is built.

## Safety

The fixture always creates a brand-new temporary repository and only ever
registers/removes that one repository via the API. It never touches
pre-existing registered repositories.
