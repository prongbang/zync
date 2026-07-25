# Zync Design

## Product Direction

Zync is a desktop-style Git workspace client for mounted repositories. The app should feel like a real desktop Git client first, with web access as a delivery surface.

Primary reference:

- Established desktop Git client UI patterns.
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

The UI should feel closer to a native macOS/Windows Git client than a web dashboard. It uses shadcn/ui's stock **neutral dark theme** — a calm grayscale surface where the content (the commit graph, diffs, file lists) provides the only color.

Theme:

- shadcn/ui default **neutral** theme (baseColor `neutral`), dark mode. Not a bespoke brand palette — the app should read as a stock shadcn app.

Implementation:

- The web app (`web/apps/web`) is React 19 + Vite + TypeScript + Tailwind v4, built on shadcn/ui components (`web/packages/ui`, `@workspace/ui`); see the shadcn skill (`web/.agents/skills/shadcn/SKILL.md`) for how components are added and styled.
- The theme itself is shadcn's default: the `.dark` semantic tokens (`--background`, `--card`, `--primary`, `--muted`, `--accent`, `--destructive`, `--ring`, `--chart-*`) live in `web/packages/ui/src/styles/globals.css` and are all achromatic grayscale in dark mode. `web/apps/web/src/zync-theme.css` does **not** override them — it only defines the functional data-viz tokens shadcn lacks.
- Always style with shadcn semantic tokens and Tailwind utilities (`bg-primary`, `bg-accent`, `text-muted-foreground`, `bg-card`, `border-border`, `text-destructive`, `ring-ring`, `Badge` variants). The `--zync-*` custom properties are ONLY for the two functional surfaces where distinct colors carry meaning — never as a general escape hatch or brand accent.

Functional (data-viz) tokens — the only `--zync-*` that remain, defined in `web/apps/web/src/zync-theme.css`:

```css
/* Commit-graph lane palette (index % 7) — distinguishes git branch lanes. */
--zync-lane-0 … --zync-lane-6;
/* Diff highlighting — added (green), removed (red), hunk (muted neutral). */
--zync-diff-added-bg / --zync-diff-added-fg;
--zync-diff-removed-bg / --zync-diff-removed-fg;
--zync-diff-hunk-bg / --zync-diff-hunk-fg;
```

Color rules:

- The app is neutral grayscale. Active, selected, connected, focused, and primary states use shadcn semantic tokens (`bg-primary`/`bg-accent`/`ring`), which are neutral in the default dark theme — no colored brand accent.
- Destructive/conflict/offline states use `text-destructive`/`bg-destructive`.
- Color appears only in the data: commit-graph lanes (functional lane palette) and diff add/remove highlighting. Do not introduce brand color elsewhere.
- Keep separators subtle and one-pixel. Prefer flat surfaces over glow.

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
- Match a desktop Git client's core feel: navigator + commit graph + working copy/diff.

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

- Visual rebase workflow similar to desktop Git clients.

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

Priority 1: Desktop-style main workspace

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

The UI feels like a desktop Git client when:

- A user can open a mounted Git repository and immediately see branch tree, commit graph, working copy, and diff.
- Fetch, pull, push, branch, commit, stash, cherry-pick, rebase, and conflict actions are reachable from the object they affect.
- Commit graph lanes are visual and readable.
- Diff supports side-by-side, inline, and partial staging.
- Conflict resolution can be completed in-app.
- Interactive rebase can be completed in-app.
- The app remains usable on mobile with one workflow visible at a time.
- Large repositories stay responsive through virtualization and lazy loading.

---

## ADR-001: Credentials & remote transport

Status: Accepted (2026-07-25). Scope: P0.1 decision record for P0.3 (git-core credentialed
transports) and P0.4 (server credentials API + at-rest encryption).

### Context

Today every network Git op runs against the *server host's* ambient credentials:
`fetch`/`push`/`pull`/`push_force_with_lease` shell out through `run_git` (`crates/git-core/src/lib.rs`),
`clone_repo` calls `Repository::clone` with no callbacks at all, and the one libgit2 network path that
exists — `delete_remote_branch` — wires `PushOptions::remote_callbacks(callbacks())`, where `callbacks()`
only ever offers `Cred::ssh_key_from_agent` / `Cred::default`. So a private repo can only sync if the host
happens to have an ssh-agent or a credential helper configured. Zync needs **per-user, at-rest-encrypted**
credentials that git-core can consume without them leaking into logs, argv, or error strings.

### Decision 1 — `credentials` table + at-rest encryption

New table (added via the same `migrate()` batch in `crates/server/src/db/mod.rs`):

```sql
CREATE TABLE IF NOT EXISTS credentials (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL,
    label         TEXT NOT NULL,                    -- human name, e.g. "GitHub PAT"
    host_pattern  TEXT NOT NULL,                    -- "github.com" or "*.github.com"
    kind          TEXT NOT NULL,                    -- 'https_token' | 'ssh_key'
    username      TEXT,                             -- https token user / ssh user (default 'git')
    secret_cipher BLOB NOT NULL,                    -- AEAD ciphertext of the secret bundle
    secret_nonce  BLOB NOT NULL,                    -- 24-byte XChaCha20 nonce, unique per write
    created_at    TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id)
);
```

- **The encrypted blob is a serialized bundle, not a bare string.** Plaintext is
  `serde_json` of `{ "token": ... }` for `https_token`, or
  `{ "private_key": ..., "passphrase": ..., "public_key": ... }` for `ssh_key`. This keeps the schema at
  one ciphertext column while letting an SSH credential carry its (also-secret) passphrase and optional
  public key. `kind` selects how the decrypted bundle is interpreted.
- **AEAD: `chacha20poly1305::XChaCha20Poly1305` (RustCrypto), chosen over `aes-gcm`.** Rationale: (a) the
  server ships as a container image that may run on ARM or on hosts without guaranteed AES-NI — the
  pure-Rust ChaCha20 software path is constant-time and fast without hardware AES, whereas software AES-GCM
  is both slower and a timing risk; (b) XChaCha20's **192-bit nonce** can be generated randomly per write
  (`OsRng`) with a negligible birthday-collision bound, so we avoid the stateful counter discipline that
  AES-GCM's 96-bit nonce demands to stay safe. Both are AEAD (confidentiality + integrity); the operational
  nonce-safety difference is the deciding factor for a low-write-volume secret store.
- **Key material: `ZYNC_SECRET_KEY`** — a base64-encoded 32-byte key read at startup. Decoded once into a
  `zeroize`-wrapped `[u8; 32]` held on `AppState`.
  - **Production (default):** if `ZYNC_SECRET_KEY` is unset or not exactly 32 bytes after base64-decode, the
    server still boots (local, credential-less repos keep working) but **every credential create/decrypt and
    every credentialed remote op fails fast with a clear, non-secret error** — e.g.
    `"credentials disabled: set ZYNC_SECRET_KEY (base64, 32 bytes) to enable encrypted credential storage"`.
    Refusing the op, not crashing the server, keeps the non-remote workflow usable.
  - **Dev fallback:** when `ZYNC_DEV=1` (or `--dev`), a missing key falls back to a fixed all-zero dev key
    and logs a single loud `WARN` that credentials are **not** meaningfully encrypted and the DB must not
    leave the dev machine. This makes local clone/push work without ceremony while making the weakening
    explicit and opt-in.
- **Write-only secrets.** The list/read API returns `{ id, label, host_pattern, kind, username, created_at }`
  only — never the ciphertext, nonce, or plaintext. There is no "reveal" endpoint; update = delete + recreate.
  Secrets are decrypted **only** in the remote-op handler, just-in-time, and never enter a log line, a
  `tracing` span, an error body, or a JSON response.

### Decision 2 — Transport: libgit2 for clone/fetch/push, CLI only for pull merge/rebase

**Confirmed the plan's recommendation, with two grounded refinements.**

- **`clone_repo`, `fetch`, `push`, `delete_remote_branch`, force-with-lease → libgit2 `RemoteCallbacks`.**
  This unifies with the transport `delete_remote_branch` already uses, keeps secrets **in memory only**
  (`Cred::userpass_plaintext` for tokens, `Cred::ssh_key_from_memory` for keys — available because
  `git2 = "0.18"` ships the `ssh_key_from_memory` default feature), and gives transfer/progress callbacks for
  free (clone gets a real progress bar; `clone_repo`'s current no-callback `Repository::clone` gets replaced
  by `RepoBuilder` + `FetchOptions`). The `callbacks()` fn is generalized to
  `callbacks(spec: &CredentialSpec) -> RemoteCallbacks` and the credential closure walks the chain below.
  - *Refinement — force-with-lease:* libgit2's push refspec expresses **force** (`+refs/...`) but has **no
    lease** (no expected-old-oid check). So `push_force_with_lease` stays correct by doing the lease check in
    git-core: read the remote-tracking ref, verify it equals the caller's expected oid, then issue a forced
    libgit2 push through the same credentialed callbacks. (Falling back to CLI-with-askpass for this one op is
    acceptable if the manual lease proves fiddly, but the libgit2 path keeps it credentialed uniformly.)
- **`pull` → CLI shellout, kept, with an injected non-interactive credential shim.** libgit2 has no `pull`;
  reimplementing correct `merge` and especially `rebase` resolution on top of `fetch` is exactly the kind of
  error-prone logic the git CLI already does correctly. Pull grows a mode (`ff-only | merge | rebase`) and
  keeps using `run_git`. Credentials are injected via **environment, never argv** (argv is what `run_git`
  interpolates into its error string):
  - HTTPS: `GIT_ASKPASS` points at a tiny bundled shim that echoes a token read from an env var we set on the
    child `Command` (e.g. `ZYNC_ASKPASS_TOKEN`); plus `GIT_TERMINAL_PROMPT=0` so a missing/blank credential
    fails fast instead of hanging on a prompt.
  - SSH: an in-memory key can't be handed to the git CLI, so an SSH-key pull writes the key to a `0600`
    temp file (OS temp / tmpfs), sets `GIT_SSH_COMMAND="ssh -i <tmpkey> -o BatchMode=yes -o IdentitiesOnly=yes"`,
    and unlinks it immediately after the process exits. ssh-agent is used directly when present.
  - *Refinement — shrink the CLI surface:* `ff-only` pull is trivially a libgit2 `fetch` + fast-forward of the
    ref, so it can go through the in-memory libgit2 path too. That leaves **only `merge` and `rebase` modes**
    truly on the CLI, which is the only place an SSH key ever touches disk — the smallest possible exposure.

Consequence: git-core keeps its "open a fresh `Repository` per call, `impl AsRef<Path>` in, `anyhow::Result`
out" shape; the new seam is a `&CredentialSpec` parameter threaded into the network fns and the generalized
`callbacks()`.

### Decision 3 — Host-pattern matching & credential selection order

Given a remote URL, git-core reports its host; the server picks a credential for the current user:

1. **Parse the host** from the URL — `https://host[:port]/...`, `ssh://[user@]host[:port]/...`, and the scp-like
   `[user@]host:path`. Also derive the **scheme class**: `https` vs `ssh`.
2. **Filter by compatibility:** an `https://` remote may only match `kind = 'https_token'`; an ssh remote
   (`ssh://` or scp-like) may only match `kind = 'ssh_key'`. A kind/scheme mismatch is never selected.
3. **Match `host_pattern` against the host:**
   - exact, case-insensitive host equality (`github.com` == `github.com`), or
   - a single leading-`*` glob (`*.github.com` matches `api.github.com`, `git.github.com`, but **not** the
     apex `github.com`). No other glob syntax — keep matching total and predictable.
4. **Selection order (first wins):**
   1. an **explicit per-remote assignment** (a `credential_id` the user pinned to that remote) — overrides all
      pattern logic;
   2. **exact host** match over any wildcard;
   3. among wildcards, the **most specific** (longest literal suffix) pattern;
   4. tie-break by **most recently created**.
5. **No match → attempt the ambient chain** (ssh-agent / `Cred::default`, i.e. today's behavior) so nothing
   regresses for hosts the user hasn't registered a credential for; if that also fails, surface a structured
   `auth` error.

### Decision 4 — `CredentialSpec` (the git-core seam) and secret hygiene

git-core owns this type so P0.3 can build against it before P0.4's storage lands. The server decrypts a
`credentials` row, maps `kind` + bundle into a `CredentialSpec`, and passes it by reference into the network
fn; the fn builds `RemoteCallbacks` from it and drops it at end of call.

```rust
// crates/git-core: secret-bearing fields wrapped so they zero on drop.
use zeroize::Zeroizing;

pub enum CredentialSpec {
    /// HTTPS token. `username` is the token user ("x-access-token", "oauth2",
    /// or the account name); `password` is the PAT/OAuth token.
    UserpassPlaintext { username: String, password: Zeroizing<String> },
    /// SSH private key held in memory (never written to disk on the libgit2 path).
    SshKey {
        username: String,                       // usually "git"
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
        public_key: Option<String>,
    },
    /// Explicit ssh-agent use (username optional).
    SshAgent { username: Option<String> },
    /// Ambient: current behavior — agent then Cred::default. The default.
    Default,
}
```

Secret-hygiene rules (binding on both crates, and the P4.3 audit):

- **`zeroize`:** every secret-bearing field is `Zeroizing<String>`, so key/token bytes are wiped when the spec
  drops. Add `zeroize` to `crates/git-core/Cargo.toml`.
- **No derived `Debug` that prints secrets.** `CredentialSpec` gets a **manual `Debug`** that renders variant
  names and `username` but prints every secret field as `"<redacted>"`. Never `{:?}` a raw token/key.
- **Secrets never enter errors.** The libgit2 credential callback failing maps to a fixed
  `auth`-kind error (`"authentication failed for <host>"`) with **no** secret and **no** userinfo-bearing URL.
  For the CLI pull path, secrets go through env (`GIT_ASKPASS` shim / `GIT_SSH_COMMAND` key file), **never
  argv**, so `run_git`'s `"git {args} failed: {detail}"` string can't contain them; the URL passed to the CLI
  must be the plain remote URL, never a `https://user:token@host` form.
- **Just-in-time decrypt, immediate drop.** The server decrypts inside the handler, constructs the spec, calls
  git-core, and lets the spec drop before returning. Decrypted material is never stored on `AppState`, cached,
  or broadcast.

### Consequences

- P0.3 can start immediately against the `CredentialSpec` above and the "libgit2 for clone/fetch/push, CLI for
  pull merge/rebase" split; `callbacks()` becomes `callbacks(&CredentialSpec)` and `clone_repo` moves to
  `RepoBuilder`.
- P0.4 owns the `credentials` table, the `XChaCha20Poly1305` encrypt/decrypt around `ZYNC_SECRET_KEY`, and the
  host-pattern selection fn; list responses are the masked projection only.
- New deploy env: `ZYNC_SECRET_KEY` (base64 32 bytes) is **required in production to use credentials**; document
  it alongside `ZYNC_REPOS_ROOT`/`ZYNC_AUTH` (P5.5). Losing the key makes stored ciphertext unrecoverable — key
  rotation = re-enter credentials.
- The SSH-key-on-disk exposure is confined to `merge`/`rebase` pulls (0600, tmpfs, unlinked); everything else is
  in-memory. Force-with-lease keeps its safety via a manual remote-oid check on the libgit2 push.
- New dependencies: `chacha20poly1305`, `base64`, `rand`/`getrandom` (server); `zeroize` (git-core).

---

## ADR-002: Authentication & multi-user

Status: Accepted (2026-07-25). Scope: P3.1 decision record for P3.2 (server auth core), P3.3 (per-user
authorization), P3.4 (frontend auth), and P3.5 (member management). Sequel to ADR-001, which already made
credentials per-user (`credentials.user_id`) behind a hardcoded `DEFAULT_USER_ID = "owner"` seam this ADR
turns into a request-derived identity.

### Context

Auth is a stub. `crate::auth::login` (`crates/server/src/auth/mod.rs`) takes an email + optional name,
`INSERT OR IGNORE`s a user, and hands back a random `token`/`refresh_token` — **no password is ever checked**
and any caller can mint a session for any email. `logout` deletes a session by a token passed in the request
*body*. Nothing consumes the session: every route runs unauthenticated, `CorsLayer::permissive()` accepts any
origin, and the `/ws/workspace/:id` upgrade (`crates/server/src/websocket/mod.rs`) performs no handshake auth
at all. The whole server acts as the one seeded `owner` user — `credentials` hardcodes `DEFAULT_USER_ID`
(`crates/server/src/credentials/mod.rs:25`), `workspace_for_repository` seeds `workspace_members` with a
literal `"owner"`/`"Owner"` row, and repositories have no owner column.

Zync is a self-hosted, single-tenant Git client for teams (not a SaaS). It needs **real password auth,
server-side sessions, and per-repository authorization** — while preserving today's zero-friction
single-user LAN/dev experience so existing deploys don't break. Registration is **admin-invite, not open
signup**: there is no public "create account" flow.

### Decision 1 — Password auth (argon2id) + first-boot admin bootstrap

`users` gains a password column; login verifies a password against it.

```sql
-- users: add (migration below preserves the existing seeded `owner` row)
password_hash TEXT           -- argon2id PHC string; NULL only for the un-bootstrapped seed row
created_at    TEXT           -- backfilled to now() for the existing row
-- role vocabulary normalized to the global scale: 'admin' | 'user'
-- (repo-scoped roles live in workspace_members — Decision 5, distinct axis)
```

- **Hashing: `argon2` crate, `Argon2::default()` = argon2id, v19, m=19456 KiB, t=2, p=1.** argon2id (not
  bcrypt/scrypt/pbkdf2) is the current OWASP first choice — hybrid resistance to both GPU and side-channel
  attack. Salt is a fresh 16 bytes from `OsRng` per hash; the full **PHC string** (`$argon2id$v=19$m=...`)
  is stored in `password_hash`, so the params travel with the hash and can be tuned later without a schema
  change. Verification uses constant-time `PasswordVerifier::verify_password`. Password hashing runs on a
  blocking thread (`tokio::task::spawn_blocking`) — argon2 is deliberately CPU/memory-heavy and must not
  block the async runtime, and this doubles as a natural throttle on the single-connection DB.
- **Login is username/email + password.** `POST /auth/login { identifier, password }` looks up the user by
  email (case-insensitive), verifies the hash, and on success issues a session cookie (Decision 2). It
  **never auto-creates a user** (the current `INSERT OR IGNORE` is deleted). A failed lookup still runs a
  verify against a fixed dummy hash so timing doesn't distinguish "no such user" from "wrong password", and
  the response is a flat `401` with a generic body (`"invalid credentials"`) — no user-enumeration signal.
- **First-boot admin bootstrap — env-first, printed one-time link as fallback.** On startup, if **no user
  has a non-NULL `password_hash`** (i.e. only the un-bootstrapped seed exists):
  1. **`ZYNC_ADMIN_USER` + `ZYNC_ADMIN_PASSWORD` set →** hash the password and set it on the seed `owner`
     user (promoting it to `role='admin'`, updating its email to `ZYNC_ADMIN_USER`). Deterministic and
     container-friendly, mirroring the `ZYNC_SECRET_KEY` env pattern from ADR-001 — this is the
     **recommended path for Docker/compose/k8s**.
  2. **Neither env var set →** generate a single-use, 24-hour **setup token** and log one loud `WARN` line
     with a `/setup?token=…` URL. Visiting it (a minimal server-rendered/one-shot API flow) lets the
     operator set the admin email + password once; the token is consumed and can never bootstrap again.
     This is the path for a bare interactive host where baking a password into the environment is awkward.
  - Once any admin exists, both mechanisms are inert. Bootstrap **only ever touches the un-bootstrapped
    state** — it can't reset a live admin's password (that's an authed admin action), so leaving the env
    vars set across restarts is safe.
- **User creation is admin-only** (P3.5): `POST /auth/users` (guarded by `role='admin'`) creates a user with
  an initial password; there is no self-service registration route.

### Decision 2 — Opaque cookie sessions, sliding expiry, server-side revocation

The client authenticates with an **HttpOnly cookie carrying an opaque, high-entropy session id**; all session
state is server-side in `sessions`, so logout and expiry are real (not just client-forgotten JWTs).

```sql
-- sessions: replace the current (token, refresh_token, user_id, created_at) shape
CREATE TABLE sessions (
    id         TEXT PRIMARY KEY,   -- sha256(raw_token) hex — NOT the raw token
    user_id    TEXT NOT NULL,
    created_at TEXT NOT NULL,      -- absolute-lifetime anchor
    last_used  TEXT NOT NULL,      -- sliding-window anchor; throttled writes (below)
    expires_at TEXT NOT NULL,      -- now + idle TTL, bumped on refresh
    FOREIGN KEY(user_id) REFERENCES users(id)
);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
```

- **The cookie value is a 256-bit random token (32 bytes `OsRng`, base64url); the DB stores only its
  SHA-256.** This is the same "DB read ≠ usable secret" posture as ADR-001's write-only credentials: a leaked
  DB backup yields session *hashes*, not live bearer tokens (SHA-256 is fine here — the token is
  high-entropy, so there's nothing to brute-force). `refresh_token` is **dropped**: an opaque server-side
  session that slides its own expiry needs no separate refresh token (that construct only earns its keep for
  stateless JWTs).
- **Cookie attributes.** Name `zync_session`; `HttpOnly` (JS can't read it — blunts XSS token theft);
  `SameSite=Lax` (CSRF baseline, Decision 7); `Path=/`; `Secure` **on by default**, disableable via
  `ZYNC_COOKIE_INSECURE=1` for plain-HTTP LAN/dev (documented as a weakening, like ADR-001's dev key). No
  `Domain` attribute (host-only cookie). `Max-Age` mirrors the idle TTL and is re-sent on refresh.
- **TTLs — sliding idle window under an absolute cap.**
  - **Idle TTL = 7 days** (`expires_at = now + 7d`). A session unused for 7 days is dead.
  - **Refresh window = 1 day.** On an authenticated request, if `now - last_used > 1d` (and the session is
    still valid), bump `last_used = now`, `expires_at = now + 7d`, and re-set the cookie. The 1-day
    threshold means at most one session-row write per active session per day — critical because the DB is a
    single `Arc<Mutex<Connection>>` and we will not take that lock on every request.
  - **Absolute lifetime cap = 30 days** from `created_at`. Past the cap, refresh is refused and the user
    re-logs-in regardless of activity — bounds the damage window of a silently-stolen cookie.
- **Revocation & sweep.** `POST /auth/logout` reads the session from the cookie (not the body), deletes the
  row, and returns a `Set-Cookie` that clears `zync_session`. Expiry is enforced on read (an
  `expires_at <= now` row is treated as absent and opportunistically deleted). A background `tokio` task
  sweeps `DELETE FROM sessions WHERE expires_at <= now()` every ~30 min so dead rows don't accumulate. An
  admin "revoke all sessions for user X" is a straight `DELETE ... WHERE user_id = ?` (P3.5, optional).

### Decision 3 — `ZYNC_AUTH=disabled` escape hatch (preserve today's behavior)

A single env var selects the auth mode, read once at startup into `AppState`:

- **`ZYNC_AUTH=enabled` (default once P3 ships):** everything in this ADR is live.
- **`ZYNC_AUTH=disabled`:** **exactly today's single-user/no-auth behavior.** Concretely: the auth
  middleware (Decision 4) short-circuits and injects a **synthetic session for the seeded `owner` user**
  (`AuthUser { id: "owner", role: "admin" }`) into every request; **all routes are open**; `/auth/login`
  and `/auth/logout` become no-ops that succeed against the synthetic owner (so a frontend built for auth
  still boots); the WebSocket ticket check (Decision 4) is bypassed; authorization checks (Decision 5)
  all resolve to the owner and pass. No cookie is set or required. This is the LAN/dev/existing-deploy mode
  — the same `DEFAULT_USER_ID = "owner"` identity, now flowing through the real request-user seam instead of
  being hardcoded per handler. It is **not** a security feature; docs must state that `disabled` means "trust
  every caller on the network" and is only appropriate behind a trusted boundary.

The value is validated at boot (unknown value = refuse to start) so a typo can't silently open a server that
the operator believed was locked.

### Decision 4 — One auth middleware over every route; WebSocket via short-lived ticket

- **`AuthUser` extractor + a `tower` middleware layer over the whole router.** A `middleware::from_fn_with_state`
  layer runs before the merged route modules (`auth`, `repository`, `workspace`, `files`, `git`, `websocket`,
  `collaboration`, `credentials` in `main.rs`). It (a) resolves `ZYNC_AUTH`; (b) in `disabled` mode injects
  the synthetic owner; (c) in `enabled` mode reads `zync_session`, looks up the (unexpired) session, performs
  the throttled sliding refresh, loads the user, and inserts an `AuthUser` into request extensions — or
  returns **`401`** on missing/expired/unknown session. Handlers then take `AuthUser` via `Extension<AuthUser>`
  (or a thin `FromRequestParts` newtype), replacing every hardcoded `DEFAULT_USER_ID`. Putting auth in one
  layer (not per-handler) means a newly-added route is authenticated by default — you must *opt out*, not
  remember to opt in.
- **Unauthenticated allowlist (the only open routes):** `POST /auth/login`, `GET /health`, `GET /ready`
  (added by P5.3 — an orchestrator's readiness check must not itself require a session, same reasoning as
  `/health`; unlike `/health` it does touch the DB, but that's a cheap read, not auth), the setup-token
  flow (`/setup*`), and the SPA static assets / index fallback (so the login page itself can load). The
  allowlist is matched inside the middleware by exact path; **everything else requires a session**, including
  `/auth/logout`, `/auth/me`, all `/repositories/*`, `/workspace/*`, `/files/*`, `/collaboration/*`,
  `/credentials/*`, and the WS route. `GET /metrics` (also P5.3) is deliberately **not** in this allowlist —
  it exposes internal operational state, so it requires a session plus an `admin` role check inside the
  handler itself (see `crates/server/src/observability.rs`, and `docs/DEPLOY.md` §5 for probe wiring).
- **`GET /auth/me`** returns the current `AuthUser` (id, email, name, role) or `401` — the frontend's
  session-probe on load and the seam for the 401-interceptor→login-redirect (P3.4).
- **WebSocket auth — short-lived single-use ticket (recommended over cookie-on-WS).** The browser *does* send
  cookies on a same-origin WS upgrade, so cookie auth "works" in production — but it is fragile exactly where
  Zync runs: the dev Vite proxy, non-browser clients, and any future cross-origin embed don't reliably carry
  the cookie on the upgrade, and putting a long-lived session token in the WS URL query would leak it into
  access logs. So we standardize on a **ticket**: an authed `POST /auth/ws-ticket` (behind the normal
  middleware, so it *is* cookie-authed) returns a single-use, ~30 s-TTL opaque ticket bound to
  `(user_id, workspace_id)`, held in an in-memory `AppState` map (not the DB — it's ephemeral). The client
  opens `/ws/workspace/:id?ticket=…`; `workspace_socket` validates the ticket **before `on_upgrade`**,
  consumes it (single-use), checks it matches the path's workspace, resolves the `AuthUser`, and only then
  upgrades — else `401`/close. Short TTL + single-use + ephemeral store bound the query-string exposure to
  near-nothing. (A `Sec-WebSocket-Protocol` subprotocol carrying the ticket, keeping it out of the URL
  entirely, is a clean refinement if the frontend WS client supports it — same ticket, different channel.)
  One code path serves both prod and dev, independent of cookie propagation quirks.

### Decision 5 — Per-user authorization: repo `owner_id` + `workspace_members` roles

Two distinct role axes: a **global** role on `users` (`admin` | `user` — admin manages users/all repos) and a
**per-repository** role via `workspace_members` (`owner` | `member` | `viewer`). This ADR standardizes the
repo-scoped vocabulary (the code currently seeds a stray `"Owner"`; normalize to lowercase `owner`).

```sql
-- repositories: add
owner_id TEXT REFERENCES users(id)   -- creator; backfilled to 'owner' (Decision 6)
```

- **Access model.** A user may act on a repository iff they are its `owner_id` **or** hold a
  `workspace_members` row on that repo's workspace (or are a global `admin`, who sees all). Membership is the
  unit of sharing; P3.5's member-management UI adds/removes `workspace_members` rows and picks the role.
- **Roles.** `owner` = full control incl. repo delete, member management, owner transfer. `member` = all git
  operations (read + write/mutate) but no member management. `viewer` = **read-only**: may hit read endpoints,
  may not mutate.
- **Enforcing read vs write at the route layer.** The git router already follows a clean invariant — **`GET`
  is read, `POST`/`PUT`/`DELETE` mutate** (and, per CLAUDE.md, every mutating route ends with
  `broadcast_git_change`). So a **repo-scope guard** (an extractor resolving `:id` → the caller's role on that
  repo, layered on the `/repositories/:id/*` and `/workspace/:id/*` subtrees) maps **HTTP method → required
  capability**: safe methods need `viewer+`; mutating methods need `member+`; owner-only actions
  (repo delete, `POST /…/members*`) need `owner`. This reuses the existing `repository(&state, &id)` lookup
  point. Non-members get `403` (distinct from the `401` for "not logged in"); a viewer attempting a `POST`
  gets `403`.
  - *Caveat, test-enforced:* method-based read/write is correct **only while the GET=read invariant holds**.
    A future non-mutating `POST` (or a mutating `GET`) would misclassify. Guard it with a test that asserts
    every registered mutating route uses a non-safe method (and, ideally, that the set of routes calling
    `broadcast_git_change` equals the set the guard treats as writes). If a genuine read-only POST ever
    appears, give it an explicit per-route capability tag rather than bending the rule.
- **Credentials become truly per-user.** `DEFAULT_USER_ID` in `crate::credentials` is replaced by
  `auth_user.id`; the already-user-scoped queries (`list_credentials_by_user`, `get_decryptable`,
  `delete_credential` — all take `user_id` and were built IDOR-safe in ADR-001) now receive the real id, so
  the credentials authorization story lands for free.

### Decision 6 — Migration & back-compat

Additive migration in the same `migrate()` batch (`ALTER TABLE … ADD COLUMN`; the `sessions` reshape is a
drop+recreate since its rows are ephemeral and any live "sessions" are unauthenticated anyway):

- **`users`:** add `password_hash TEXT` (NULL-able), `created_at TEXT`; backfill `created_at` for the seed
  row; normalize `role` to `admin`/`user`. The seeded `owner` row survives with `password_hash = NULL` until
  bootstrap (Decision 1) sets it — an un-bootstrapped `enabled` server has no one who can log in, which is
  the correct fail-closed state until the operator bootstraps.
- **`repositories`:** add `owner_id TEXT`; **backfill every existing repo's `owner_id = 'owner'`** and ensure
  a `workspace_members(workspace_id, 'owner', 'owner')` row exists for each — so all pre-auth repositories
  belong to the bootstrapped admin and nothing becomes orphaned/invisible when auth flips on.
- **`sessions`:** drop and recreate with the new shape; existing rows are discarded (everyone re-logs-in
  once — acceptable, they were never real sessions).
- **The `DEFAULT_USER_ID = "owner"` seam** becomes request-derived: in `disabled` mode the middleware still
  yields `"owner"` (identical behavior); in `enabled` mode it yields the real authed id. The literal `"owner"`
  user id is retained as the bootstrap admin's id so the backfill lines up and no data migration of foreign
  keys is needed.

### Decision 7 — Threat notes & composition with P4

- **CSRF.** `SameSite=Lax` is the primary mitigation: the browser withholds `zync_session` on cross-site
  **sub-resource / form-POST** requests, so a malicious page can't drive a state-changing `POST` with the
  victim's cookie. Lax (not Strict) is chosen so a top-level navigation *to* Zync still carries the cookie
  (login-then-land UX). **Do we need CSRF tokens too?** Lax leaves a residual gap only for top-level
  `GET`-triggered navigations, and all our mutations are non-GET, so **no synchronizer token for v1** —
  but we add belt-and-suspenders that cost nothing: (a) require a custom header the browser only sends
  same-origin (the frontend already calls JSON APIs via `fetch`, which can set `X-Requested-With` /
  `Content-Type: application/json`; a simple cross-site form can't set those), and (b) the P4.2 same-origin
  CORS lockdown, which independently blocks cross-origin credentialed reads. If we later add cookie-authed
  cross-origin use, revisit with a double-submit token.
- **Session fixation.** Sessions are server-minted only on successful login and the id is never
  attacker-settable (no "accept a session id from the client"); on login we always create a **fresh** row.
  Nothing to fixate.
- **Cookie theft.** `HttpOnly` blocks XSS-based reads; `Secure` blocks network sniffing (behind TLS); storing
  only `sha256(token)` blocks DB-leak reuse; the 30-day absolute cap and 7-day idle TTL bound a stolen
  cookie's usefulness; admin revoke + logout give an active kill switch. Residual XSS that *acts through* the
  cookie (rather than exfiltrating it) is mitigated by the P4.2 CSP.
- **Brute force / enumeration.** Generic `401` + constant-time dummy-verify (Decision 1) blunt enumeration;
  **rate-limiting `/auth/login` and `/auth/ws-ticket` is P4.2** (`tower_governor`) — this ADR leaves the
  hooks (distinct routes, generic errors) so P4 can clamp them without reshaping auth.
- **Composition with P4.** This ADR assumes and depends on P4.2 (replace `CorsLayer::permissive()` with a
  same-origin default + `ZYNC_CORS_ORIGINS`, security headers incl. CSP, request-body limits) and P4.1
  (`ZYNC_REPOS_ROOT` filesystem boundary — an authed non-admin still must not register `/etc`). Auth is
  necessary but not sufficient; the two land adjacent by design and P3.7/P4.4 security reviews cover the seam.
- **P4.2 implemented** (`crates/server/src/net_hardening.rs`): CORS defaults to no cross-origin allowlist
  (same-origin needs no CORS headers either way) with `ZYNC_CORS_ORIGINS` as the explicit, credentialed
  opt-in — never combined with a wildcard origin. Every response gets `X-Content-Type-Options: nosniff`,
  `X-Frame-Options: DENY`, `Referrer-Policy: same-origin`, and a CSP (`default-src 'self'`, `img-src`
  allows `https:` for gravatar, `style-src` allows `'unsafe-inline'` for Radix/shadcn's inline-styled
  popovers/tooltips, `connect-src 'self'` covers the `/ws` upgrade) — verified against the production Vite
  build with no violations; the raw-blob route's own stricter per-response CSP (Decision 4 area,
  `git::blob_response_headers`) is left untouched since the layer only fills in headers a handler hasn't
  already set. `POST /auth/login` and `POST /setup` get a strict `tower_governor` rate limit (~10/min per
  IP); `POST /auth/ws-ticket` gets a deliberately generous one (60/min) so the reconnect backoff loop in
  `useWorkspace.ts` can't lock itself out of live sync — both return `429` with a standard `Retry-After`
  header. A global 10 MiB request body cap (`RequestBodyLimitLayer` + `DefaultBodyLimit::disable()`)
  replaces axum's independent 2MB default.
- **P4.2 follow-up — `ZYNC_TRUSTED_PROXY` (post-review fix):** the rate limiters above default to keying
  on the raw TCP peer address (`PeerIpKeyExtractor`), which is correct for direct exposure but WRONG once
  a reverse proxy sits in front (the P5.5 direction): every client's peer address is then the proxy's own
  IP, so all callers collapse into one shared bucket — one noisy client can exhaust `/auth/login`'s bucket
  and lock out *everyone's* login, and the per-IP brute-force defense is nullified since an attacker hides
  behind the same apparent IP as legitimate users. `ZYNC_TRUSTED_PROXY=1` switches the key extractor to
  `SmartIpKeyExtractor`, which recovers the real client IP from `X-Forwarded-For`/`X-Real-IP`/`Forwarded`
  (falling back to the peer address if none are present) — set it **only** when a reverse proxy you
  control terminates TLS and discards/rewrites any inbound copies of those headers before setting its own
  (otherwise a direct, untrusted client can spoof its own rate-limit key). Terminating TLS at a proxy
  WITHOUT setting this makes peer-IP rate limiting effectively inoperative for that deployment shape —
  enforce rate limiting at the proxy instead in that case. See `net_hardening::trusted_proxy`.

### Consequences

- **P3.2 can start immediately** against: the `users.password_hash` + reshaped `sessions` schema, the
  `zync_session` cookie contract (name/attrs/TTLs above), the `AuthUser` extractor + whole-router middleware,
  `/auth/login`·`/auth/logout`·`/auth/me`·`/auth/ws-ticket`, the bootstrap logic, and the WS ticket handshake.
- **P3.3** adds `repositories.owner_id`, the repo-scope role guard (method→capability), the migration backfill,
  and swaps `DEFAULT_USER_ID` for `auth_user.id` in `credentials` (and anywhere else the owner id is assumed).
- **New deploy env (document alongside `ZYNC_SECRET_KEY`/`ZYNC_REPOS_ROOT` in P5.5):** `ZYNC_AUTH`
  (`enabled`|`disabled`, default `enabled`), `ZYNC_ADMIN_USER`/`ZYNC_ADMIN_PASSWORD` (first-boot bootstrap),
  `ZYNC_COOKIE_INSECURE` (drop `Secure` for plain-HTTP LAN), and (P4.2) `ZYNC_CORS_ORIGINS`
  (comma-separated list of origins allowed cross-origin, credentialed API access; unset/blank — the
  default — allows no cross-origin caller since same-origin deploys need none) and `ZYNC_TRUSTED_PROXY`
  (`1` to trust `X-Forwarded-For`/`X-Real-IP`/`Forwarded` for rate-limit keying — **only** behind a proxy
  you control that strips/rewrites those headers; unset, the default, keys on the raw TCP peer address,
  which is the SAFE choice for direct exposure but goes fleet-wide-shared and stops discriminating
  between clients once ANY proxy is added in front without also setting this flag). The Docker image
  ships `ZYNC_AUTH=enabled` by default (launch-checklist item).
- **Behavior change:** `enabled` mode makes the frontend's current no-login flow break by design — P3.4 must
  ship the login screen + 401 interceptor in the same release that flips the default. `disabled` mode is the
  bridge that keeps existing LAN deploys working untouched.
- **New dependencies (server):** `argon2`, `cookie` (or `axum-extra`'s `CookieJar`), `sha2`, `rand`; `tokio`
  blocking pool already present for the argon2 offload.
- **Not in scope (deferred):** OAuth/SSO, email-based password reset (admin resets in-app for now), 2FA,
  per-branch ACLs, and audit logging — noted as post-v1 if demand appears.

#### Task-by-task breakdown (maps to PLAN.md P3)

- **P3.2 — Server auth core.** Schema (`users.password_hash`/`created_at`, `sessions` reshape); argon2id
  hash/verify on `spawn_blocking`; cookie issue/clear/refresh (sliding + absolute cap); `sessions` sweep task;
  `AuthUser` extractor + router-wide middleware with the unauthenticated allowlist; `/auth/login`,
  `/auth/logout`, `/auth/me`; first-boot bootstrap (env + one-time-link); `ZYNC_AUTH=disabled` synthetic-owner
  path; WS `/auth/ws-ticket` + handshake validation in `workspace_socket`.
- **P3.3 — Per-user scoping.** `repositories.owner_id` + backfill; repo-scope role guard (viewer/member/owner
  via method→capability) over `/repositories/:id/*`; normalize `workspace_members` role vocabulary; replace
  `DEFAULT_USER_ID` with the request user in `credentials`; the "no GET mutates" regression test.
- **P3.4 — Frontend auth.** Login screen; `api.ts` 401 interceptor → redirect to login; `/auth/me` session
  probe on load; call `/auth/ws-ticket` before opening the workspace socket and pass the ticket in the WS
  URL/subprotocol; user menu (logout, credentials entry); transparently no-op the login flow when the server
  reports `disabled`.
- **P3.5 — Member management UI.** Admin `POST /auth/users` (create user) + user list; per-repository
  member add/remove + role picker writing `workspace_members`; owner-only guards on those routes.
