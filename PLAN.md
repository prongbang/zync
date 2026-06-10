# Git Workspace Manager (Fork-like)

## Goal

Build a cross-platform Git client similar to Fork.

Platforms:

- Web Browser
- Desktop (Windows, macOS, Linux)
- Mobile (iOS, Android)

Technology:

- Rust
- Dioxus
- Axum
- Tokio
- Git2 (libgit2)
- WebSocket
- SQLite

Core Features:

- Repository Management
- Commit Graph
- Branch Tree
- Diff Viewer
- Interactive Rebase
- Cherry Pick
- Stash
- Conflict Resolution
- Real-time Workspace Sync
- Multi-user Collaboration

---

## Phase 1: Foundation

### Backend

Create server modules:

- auth
- repository
- workspace
- git
- websocket

Database:

- users
- repositories
- workspaces
- workspace_members
- sessions

Endpoints:

```text
POST   /auth/login
POST   /auth/logout

GET    /repositories
POST   /repositories

GET    /workspace/:id
```

WebSocket:

```text
/ws/workspace/:id
```

### Git Core

Create crate:

```text
crates/git-core
```

Features:

- open repository
- clone repository
- fetch
- pull
- push
- status
- add
- commit

Functions:

- `open_repo()`
- `clone_repo()`
- `fetch()`
- `pull()`
- `push()`
- `status()`
- `commit()`

---

## Phase 2: Workspace Sync

Goal:

Changes from Machine A appear instantly on Machine B.

Components:

- workspace-sync
- file-watcher
- websocket

Flow:

```text
User A
  |
File Change
  |
Notify Watcher
  |
Broadcast Event
  |
User B
  |
Update UI
```

Events:

- file_changed
- file_deleted
- file_created
- folder_created
- folder_deleted

---

## Phase 3: Repository Explorer

UI:

- Repository List

Features:

- clone repository
- open repository
- remove repository
- favorite repository

Sidebar:

- Working Tree
- Branches
- Tags
- Remotes
- Stashes

---

## Phase 4: File Explorer

Features:

- folder tree
- file tree
- search file
- rename file
- create file
- delete file

File Types:

- text
- json
- yaml
- toml
- markdown

---

## Phase 5: Editor

Features:

- Monaco Editor
- Syntax Highlight
- Multi Tab
- Search
- Replace

Supported:

- Rust
- Go
- TypeScript
- JavaScript
- JSON
- YAML
- Markdown

Realtime:

- auto save
- live update

---

## Phase 6: Git Status

Sections:

- Staged
- Unstaged
- Untracked

Features:

- stage file
- unstage file
- stage all
- discard change

---

## Phase 7: Diff Viewer

Features:

- side by side diff
- inline diff
- syntax highlight

Modes:

- working tree diff
- staged diff
- commit diff

---

## Phase 8: Commit

Features:

- commit message
- amend commit
- sign off
- push after commit

---

## Phase 9: Commit Graph

Goal:

Fork-like graph.

Features:

- branch graph
- merge graph
- tag graph
- remote graph

View:

```text
* commit 123
|
| * commit 122
|/
* commit 121
```

---

## Phase 10: Branch Manager

Features:

- create branch
- rename branch
- delete branch
- checkout branch
- merge branch

Branch Tree:

```text
main
|-- develop
|-- feature/auth
`-- feature/ui
```

---

## Phase 11: Cherry Pick

Features:

- single commit
- multiple commits

Actions:

- continue
- abort

---

## Phase 12: Stash

Features:

- create stash
- apply stash
- pop stash
- drop stash

---

## Phase 13: Conflict Resolution

Viewer:

- LOCAL
- BASE
- REMOTE

Actions:

- Accept Local
- Accept Remote
- Accept Both
- Manual Edit

---

## Phase 14: Interactive Rebase

Features:

- reorder commits
- squash
- fixup
- drop
- edit

UI:

- Drag and Drop

Example:

```text
pick
pick
squash
drop
```

---

## Phase 15: Collaboration

Features:

- online users
- file presence
- cursor presence
- file lock

Events:

- user_joined
- user_left
- cursor_changed
- file_locked
- file_unlocked

---

## Phase 16: Security

Authentication:

- JWT
- Refresh Token

Authorization:

- Owner
- Admin
- Developer
- Viewer

---

## Phase 17: Performance

Target:

Repository Size:

- 10GB+

Commit Count:

- 1,000,000+

File Count:

- 100,000+

Strategies:

- virtualized tree
- incremental graph loading
- lazy diff loading
- websocket batching

---

## Phase 18: Release

Desktop:

- macOS
- Windows
- Linux

Mobile:

- iOS
- Android

Web:

- Browser

Deployment:

- Docker
- Docker Compose
- Kubernetes

---

## Future

- AI Commit Message
- AI Conflict Resolution
- AI Code Review
- AI Pull Request Review
- AI Branch Cleanup
- GitHub Integration
- GitLab Integration
- Bitbucket Integration
- Self Hosted Mode
- Enterprise

---

## Implementation Status

Current workspace implementation:

- `crates/git-core`: Git operations for open, clone, fetch, pull, push, status, add, commit, amend, diff, branch operations, merge, cherry-pick, stash, commit graph, and conflict helpers.
- `crates/git-core`: Advanced Git operations for unstage, discard, patch staging, and UI-driven linear interactive rebase with pick, squash, fixup, drop, and edit actions.
- `crates/server`: Axum backend with auth, repositories, workspaces, file explorer, Git API, WebSocket workspace events, deduplicated workspace watchers, batched sync payloads, SQLite persistence, and collaboration presence/locks.
- `crates/ui`: Dioxus UI scaffold and browser API client for repository management, workspace loading, status, branches, graph, diff, commit, and interactive rebase calls.
- `docs/API.md`: API endpoint reference.
- `Dockerfile`, `docker-compose.yml`, `k8s/`: release and deployment scaffolding.

Verified:

- `cargo fmt`
- `cargo check`
- `cargo test`
- local server `/health`, `/repositories`, and `/auth/login`
