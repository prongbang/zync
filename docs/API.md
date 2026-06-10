# Zync API

## Auth

- `POST /auth/login`
- `POST /auth/logout`

## Repositories

- `GET /repositories`
- `POST /repositories`
- `DELETE /repositories/:id`
- `PUT /repositories/:id/favorite`
- `POST /repositories/:id/open`

## Workspace

- `GET /workspace/:id`
- `GET /ws/workspace/:id`

## Files

- `POST /workspace/:id/files`
- `GET /workspace/:id/files/search?q=term`
- `GET /workspace/:id/files/*path`
- `PUT /workspace/:id/files/*path`
- `DELETE /workspace/:id/files/*path`
- `PUT /workspace/:id/files/rename`

## Git

- `GET /repositories/:id/git/status`
- `POST /repositories/:id/git/add`
- `POST /repositories/:id/git/unstage`
- `POST /repositories/:id/git/discard`
- `POST /repositories/:id/git/stage-patch`
- `POST /repositories/:id/git/commit`
- `GET /repositories/:id/git/diff/workdir`
- `GET /repositories/:id/git/diff/staged`
- `GET /repositories/:id/git/diff/commit/:commit_id`
- `POST /repositories/:id/git/fetch`
- `POST /repositories/:id/git/pull`
- `POST /repositories/:id/git/push`
- `GET /repositories/:id/git/branches`
- `POST /repositories/:id/git/branches`
- `POST /repositories/:id/git/checkout`
- `POST /repositories/:id/git/branches/rename`
- `POST /repositories/:id/git/branches/merge`
- `POST /repositories/:id/git/branches/delete`
- `GET /repositories/:id/git/graph`
- `GET /repositories/:id/git/rebase/plan`
- `POST /repositories/:id/git/rebase/interactive`
- `POST /repositories/:id/git/cherry-pick`
- `POST /repositories/:id/git/cherry-pick/abort`
- `GET /repositories/:id/git/conflicts`
- `POST /repositories/:id/git/conflicts/resolve`
- `GET /repositories/:id/git/stashes`
- `POST /repositories/:id/git/stashes`
- `POST /repositories/:id/git/stashes/apply`
- `POST /repositories/:id/git/stashes/drop`

## Collaboration

- `GET /workspace/:id/presence`
- `PUT /workspace/:id/presence/:user_id`
- `DELETE /workspace/:id/presence/:user_id`
- `PUT /workspace/:id/locks/:path`
- `DELETE /workspace/:id/locks/:path`
