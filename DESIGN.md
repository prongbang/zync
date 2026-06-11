# Zync Design

## Product Direction

Zync is a Fork-like Git workspace client for mounted repositories. The app should feel like a real desktop Git client first, with web access as a delivery surface.

Primary reference:

- Fork desktop Git client UI from `https://git-fork.com/`
- Key reference surfaces: commit list, working directory changes, side-by-side diff, repository manager, merge conflict resolver, interactive rebase, history, blame, and line-by-line staging.

Design goals:

- Make the commit graph and working copy the center of the product.
- Keep Git operations visible, direct, and close to the selected object.
- Prefer dense, scan-friendly desktop UI over marketing-style cards.
- Support responsive mobile usage without losing core Git workflows.
- Keep every panel resizable or collapsible on desktop.

---

## App Shell

Desktop layout:

```text
+--------------------------------------------------------------------------------+
| Top Toolbar: repo name, current branch, fetch, pull, push, search, layout       |
+-------------+----------------------------------------------+-------------------+
| Repository  | Commit Graph / History                       | Inspector         |
| Navigator   |                                              |                   |
|             | - graph lanes                                | - commit details  |
| - repos     | - commit subject                             | - refs/tags       |
| - branches  | - author/date                                | - changed files   |
| - tags      | - labels: local/remote/tag/stash              | - actions         |
| - remotes   |                                              |                   |
| - stashes   +----------------------------------------------+-------------------+
|             | Working Copy / Diff / Conflict / Rebase tabs                     |
+-------------+------------------------------------------------------------------+
```

Desktop pane rules:

- Left repository navigator: 220-420px.
- Commit graph should be the primary central surface.
- Inspector: 320-560px.
- Bottom workflow area: resizable height, with tabs for working copy, diff, editor, conflict, rebase, stash, and tools.
- Splitters must be draggable and also provide small step controls for accessibility.
- Avoid nested card layouts. Panels should look like native app panes with thin borders and compact headers.

Mobile layout:

```text
+------------------------------------------------+
| Top Toolbar                                    |
+------------------------------------------------+
| View Switcher: Repo / Graph / Changes / Diff   |
+------------------------------------------------+
| Active View                                    |
|                                                |
| One major workflow at a time                   |
+------------------------------------------------+
| Sticky Action Bar                              |
+------------------------------------------------+
```

Mobile rules:

- Use tabs or segmented controls instead of multi-column panes.
- Keep commit graph horizontally scrollable.
- Keep file actions in sticky bottom bars or compact row menus.
- Diff viewer defaults to inline mode; side-by-side becomes optional on wider screens.
- Rebase, stash, cherry-pick, and conflict flows should use full-screen workflow views.

---

## Visual Language

The UI should feel closer to a native macOS/Windows Git client than a web dashboard.

Palette:

- App background: near black or dark zinc.
- Pane background: slightly lifted dark gray.
- Border: subtle gray, one-pixel separators.
- Accent: restrained cyan/blue for active branch, selected rows, and primary actions.
- Status colors:
  - Added: green.
  - Modified: amber or blue.
  - Deleted/conflict: red.
  - Untracked: purple or muted cyan.

Typography:

- Use compact system UI fonts.
- Use monospace only for hashes, paths, diffs, and code.
- Avoid hero-size type inside the app shell.
- Prefer 11-13px UI text for dense panes.

Spacing:

- Use 4px and 8px spacing rhythm.
- Headers should be compact: 36-48px.
- Rows should be scan-friendly: 26-34px.
- Cards are only for repository manager items or modal content, not for app sections.

Controls:

- Toolbar actions use icon-first buttons with tooltips.
- Git operation buttons are compact and grouped: Fetch, Pull, Push.
- Destructive actions use muted danger styling until hover/confirmation.
- Context menus are the primary place for branch, commit, stash, and file actions.

---

## Core Screens

## 1. Repository Manager

Purpose:

- Open existing mounted repositories.
- Clone new repositories into a server-mounted path.
- Show recent/favorite repositories quickly.

Layout:

```text
+----------------------------------------------------------------+
| Repository Manager                                             |
+----------------------------+-----------------------------------+
| Recent / Favorites         | Repository Details                |
|                            |                                   |
| repo name                  | path                              |
| branch                     | current branch                    |
| dirty count                | status summary                    |
| last opened                | remotes                           |
| favorite                   | open / remove / reveal / clone    |
+----------------------------+-----------------------------------+
```

Required UI:

- Recent repositories list.
- Favorite toggle.
- Clone repository form.
- Add mounted repository form.
- Repository summary: branch, dirty files, ahead/behind, last commit.
- Empty state with mounted path guidance.

---

## 2. Main Git Workspace

Purpose:

- The daily Git client surface.
- Match Fork's core feel: navigator + commit graph + working copy/diff.

Default desktop layout:

- Left pane: Repository Navigator.
- Center top: Commit Graph.
- Right pane: Commit/Ref Inspector.
- Bottom pane: Working Copy and Diff tabs.

Toolbar:

- Repository switcher.
- Current branch selector.
- Fetch.
- Pull.
- Push.
- New branch.
- Stash.
- Search.
- Layout controls.

Status bar:

- Workspace sync state.
- Backend connection state.
- Current repository path.
- Current branch.
- Ahead/behind.
- Last operation result.

---

## 3. Repository Navigator

Sections:

- Working Tree.
- Branches.
- Tags.
- Remotes.
- Stashes.
- Submodules.

Branch tree behavior:

- Local and remote branches are grouped.
- Current branch is highlighted.
- Ahead/behind badges appear next to branch names.
- Branch names support context menu actions.

Branch context menu:

- Checkout.
- Create branch from here.
- Rename.
- Delete.
- Merge into current branch.
- Rebase current onto this branch.
- Cherry-pick selected commit.
- Push branch.
- Pull branch.
- Track remote branch.
- Copy branch name.

---

## 4. Commit Graph

Purpose:

- This is the main visual anchor of the app.
- It should look like a real Git client graph, not just a list.

Rows:

```text
graph | subject                  | refs        | author | date
------+--------------------------+-------------+--------+------
o     | Add auth module          | main        | you    | 1m
| o   | WIP workspace sync       | feature/ws  | you    | 5m
|/    | Merge workspace base     |             | you    | 1h
o     | Initial repository core  | tag:v0.1.0  | you    | 2h
```

Graph lane rules:

- Use colored lane lines.
- Keep row height stable.
- Render merge joins clearly.
- Show local branches, remote branches, tags, and stashes as compact pills.
- Use virtualized loading for large histories.
- Keep selected commit synced with inspector and diff view.

Commit row actions:

- Checkout commit.
- Create branch.
- Cherry-pick.
- Revert.
- Rebase from here.
- Copy hash.
- Browse files at commit.

---

## 5. Working Copy

Purpose:

- Fast staging, unstaging, and commit preparation.

Sections:

- Merge conflicts.
- Staged.
- Unstaged.
- Untracked.

File row:

```text
[status] path/to/file.rs                     +12 -4     Stage
```

Required actions:

- Stage file.
- Unstage file.
- Stage all.
- Unstage all.
- Discard changes.
- Open diff.
- Open file.
- Rename file.
- Delete file.

Partial staging:

- Diff hunks must have inline controls:
  - Stage hunk.
  - Unstage hunk.
  - Stage selected lines.
  - Discard hunk.
- Hunk headers stay sticky inside the diff viewer.
- Selected lines must be visually distinct.

---

## 6. Diff Viewer

Modes:

- Working tree diff.
- Staged diff.
- Commit diff.
- Side-by-side diff.
- Inline diff.
- Image diff.

Desktop layout:

```text
+----------------------------------------------------------------+
| file path                                      mode / actions   |
+--------------------------------+-------------------------------+
| before                         | after                         |
| code                           | code                          |
+--------------------------------+-------------------------------+
```

Rules:

- Side-by-side is default on desktop.
- Inline is default on mobile.
- Show file path, status, additions/deletions, and current diff mode in the header.
- Hunk actions live directly next to hunk headers.
- Code uses monospace with stable line heights.
- Empty diff state should explain why no diff is available.

Image diff:

- Show before and after.
- Modes: side-by-side, swipe, onion/opacity.
- Show image dimensions and file size when available.

---

## 7. Commit Panel

Purpose:

- Compose commit without leaving the working copy.

Required UI:

- Commit message editor.
- Recent commit messages.
- Amend toggle.
- Sign-off toggle.
- Push after commit toggle.
- Author identity.
- Commit button.
- Commit and push button.

Validation:

- Empty commit message is blocked.
- If nothing is staged, show a clear warning.
- If conflicts exist, commit is blocked.

---

## 8. Interactive Rebase

Purpose:

- Visual rebase workflow similar to Fork.

Layout:

```text
+----------------------------------------------------------------+
| Rebase onto: main                              Start / Abort    |
+--------+-------------------------------------------------------+
| action | commit                                                |
+--------+-------------------------------------------------------+
| pick   | abc123 Add repository API                             |
| squash | def456 Clean up UI state                              |
| drop   | 999999 Temporary debug                                |
+--------+-------------------------------------------------------+
```

Required UI:

- Reorder commits with drag and drop.
- Action selector per commit:
  - pick
  - reword
  - edit
  - squash
  - fixup
  - drop
- Continue.
- Abort.
- Preview resulting sequence.
- Show conflict state if rebase stops.

---

## 9. Stash

Purpose:

- Stashes should be visible in the Git history and navigator.

Required UI:

- Stash list.
- Create stash with message.
- Include untracked toggle.
- Apply.
- Pop.
- Drop.
- Show stash diff.
- Create branch from stash.

---

## 10. Cherry-pick

Purpose:

- Apply one or more selected commits onto the current branch.

Required UI:

- Selected commits queue.
- Reorder queue.
- Pick single commit.
- Pick multiple commits.
- Continue.
- Abort.
- Show conflicts if stopped.

Commit graph integration:

- Cherry-pick starts from commit row context menu.
- Multi-select commits from graph.
- Queue appears in inspector or workflow drawer.

---

## 11. Conflict Resolution

Purpose:

- Resolve merge/rebase/cherry-pick conflicts without leaving the app.

Layout:

```text
+---------------------+---------------------+---------------------+
| LOCAL               | BASE                | REMOTE              |
+---------------------+---------------------+---------------------+
| code                | code                | code                |
+---------------------+---------------------+---------------------+
| Result editor                                               |
+----------------------------------------------------------------+
| Accept Local | Accept Remote | Accept Both | Mark Resolved       |
+----------------------------------------------------------------+
```

Required UI:

- Conflict file list.
- Three-way viewer: local, base, remote.
- Result editor.
- Accept local.
- Accept remote.
- Accept both.
- Manual edit.
- Mark resolved.
- Continue operation.
- Abort operation.

Rules:

- Conflict workflow should take over the bottom workspace area or a full-screen mobile view.
- Do not hide unresolved files.
- Show current operation: merge, rebase, or cherry-pick.

---

## 12. History And Blame

History:

- Show commits that touched the selected file or folder.
- Allow diffing any selected history entry.
- Allow browsing file content at a commit.

Blame:

- Show line number, commit hash, author, and date.
- Selecting a blamed line selects the related commit in the graph.
- Commit details open in inspector.

---

## 13. Collaboration Layer

Purpose:

- Zync's differentiator is mounted workspace sync and multi-user presence.

UI requirements:

- Online users in the top toolbar or status bar.
- File presence badges in file tree.
- Cursor presence in editor/diff where applicable.
- File lock indicator.
- Workspace sync status:
  - connected
  - reconnecting
  - offline
  - syncing
  - error

Collaboration should not disrupt core Git workflows. Presence indicators must stay subtle.

---

## Responsive Behavior

Breakpoints:

- Mobile: below 768px.
- Tablet: 768-1279px.
- Desktop: 1280px and above.

Mobile:

- Single active view.
- Bottom or top segmented view switcher.
- Inline diff default.
- Context menus become action sheets.
- Full-screen flows for conflict, rebase, stash, cherry-pick.

Tablet:

- Two-column layout.
- Navigator collapses.
- Inspector can become a drawer.

Desktop:

- Full multi-pane layout.
- Draggable splitters.
- Keyboard shortcuts.
- Context menus.

---

## Interaction Rules

Selection:

- Selecting a repository opens or focuses its workspace.
- Selecting a branch updates inspector and enables branch actions.
- Selecting a commit updates inspector and commit diff.
- Selecting a changed file updates diff viewer.
- Selecting a conflict file opens conflict editor.

Context menus:

- Branch rows.
- Commit rows.
- File rows.
- Stash rows.
- Diff hunk headers.

Keyboard:

- `f`: fetch.
- `p`: pull.
- `shift+p`: push.
- `c`: focus commit message.
- `cmd/ctrl+enter`: commit.
- `cmd/ctrl+f`: search.
- `j/k`: move selection.
- `space`: preview selected item.

Keyboard shortcuts should be configurable later.

---

## Implementation Priority

Priority 1: Fork-like main workspace

- Make commit graph the center pane.
- Move working copy and diff into a stronger bottom workflow area.
- Improve row density and pane headers.
- Add direct context menus for branch, commit, file, and stash rows.

Priority 2: Working copy quality

- Partial hunk staging.
- Stage selected lines.
- Better staged/unstaged/untracked grouping.
- Commit message history.

Priority 3: Advanced Git workflows

- Full interactive rebase view.
- Full stash workflow.
- Full cherry-pick queue.
- Conflict editor with three-way result.

Priority 4: History tools

- File history.
- Blame.
- Reflog.
- Browse tree at commit.

Priority 5: Polish and scale

- Virtualized commit graph.
- Virtualized file tree.
- Lazy diff loading.
- Image diff modes.
- Keyboard shortcuts.
- Mobile action sheets.

---

## Definition Of Done

The UI is Fork-like enough when:

- A user can open a mounted Git repository and immediately see branch tree, commit graph, working copy, and diff.
- Fetch, pull, push, branch, commit, stash, cherry-pick, rebase, and conflict actions are reachable from the object they affect.
- Commit graph lanes are visual and readable.
- Diff supports side-by-side, inline, and partial staging.
- Conflict resolution can be completed in-app.
- Interactive rebase can be completed in-app.
- The app remains usable on mobile with one workflow visible at a time.
- Large repositories stay responsive through virtualization and lazy loading.
