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
