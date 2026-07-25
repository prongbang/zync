# Backup & Restore Runbook

Zync's server state lives in one SQLite file (`zync.db`, WAL mode). This
runbook covers backing it up safely, restoring it, and verifying the restore
— for both a bare-metal/systemd deployment and the Docker Compose deployment
shipped in this repo.

## 1. What's in the backup, and what's not

The `zync.db` file (opened via `Database::open`, `crates/server/src/db/mod.rs`)
contains every table this server persists to:

| Table | Contents |
|---|---|
| `users` | accounts, argon2id password hashes, roles |
| `repositories` | registered repo id/name/**path**/remote URL/owner |
| `workspaces` / `workspace_members` | per-repo workspace + membership/roles |
| `sessions` | hashed session tokens (SHA-256 of the cookie, not the raw token) |
| `credentials` | **encrypted** HTTPS tokens / SSH keys (XChaCha20Poly1305 ciphertext + nonce) |

Backing up `zync.db` backs up the **registry, users, sessions, and encrypted
credentials** — it does not back up:

- **The Git repositories themselves.** `repositories.path` is just a
  filesystem path (or, in Docker, a path under the bind-mounted
  `/workspaces` volume) that the server reads/writes with `git2` at request
  time. Zync never clones or stores a copy of repo content in the DB or in
  `zync-data`. Back up those working trees/bare repos the normal way — they
  are already Git remotes, or filesystem/volume snapshots, or both. This
  runbook is scoped to `zync.db` only.
- **`ZYNC_SECRET_KEY`.** The `credentials` table's `secret_cipher`/
  `secret_nonce` columns are ciphertext produced by `crypto::encrypt`
  (`crates/server/src/crypto/mod.rs`) under this key. The key never lives in
  the database — it's an environment variable read once at process startup.
  A `zync.db` backup without the matching key is permanently useless for
  decrypting stored tokens/SSH keys (see §4 below).

## 2. Hot backup — do it the right way (WAL mode matters)

`apply_pragmas` (`crates/server/src/db/mod.rs`) sets `journal_mode = WAL`,
`synchronous = NORMAL`, `busy_timeout = 5000`, `foreign_keys = ON`. In WAL
mode, recent writes live in a separate `zync.db-wal` file (and `zync.db-shm`
for the shared-memory index) until a checkpoint folds them back into the main
file. **A plain `cp zync.db backup.db` while the server is running can copy
the main file mid-write, or copy it without its `-wal`/`-shm` sidecars,
producing a backup that is missing recent commits or is outright
inconsistent.** Never do this for a live server.

Use SQLite's online backup API instead, via the `sqlite3` CLI's `.backup`
command (or the SQL `VACUUM INTO`) — both take a consistent snapshot while
the server keeps writing, no downtime required.

### Bare-metal / systemd host

```sh
# Preferred: online backup API. Safe to run against a live, writing server.
sqlite3 /path/to/zync.db ".backup '/backups/zync-$(date +%Y%m%d-%H%M%S).db'"
```

Equivalent, using `VACUUM INTO` (also online-consistent, and additionally
compacts the copy):

```sh
sqlite3 /path/to/zync.db "VACUUM INTO '/backups/zync-$(date +%Y%m%d-%H%M%S).db'"
```

Either command produces a single, self-contained `.db` file — no `-wal`/
`-shm` sidecars to track separately, because the backup API replays the WAL
into the destination file as it copies.

Verify the copy is a valid, complete SQLite file before trusting it:

```sh
sqlite3 /backups/zync-20260725-020000.db "PRAGMA integrity_check;"
# expected output: ok
```

### Docker Compose variant

This repo's `docker-compose.yml` runs the server as service `zync`, with
`ZYNC_DB=/data/zync.db` inside the container and that `/data` directory
backed by the named volume `zync-data` (the actual Git repos live under the
separate `zync-workspaces` volume / host bind mounts, per §1 — not part of
this backup).

Run `sqlite3` inside the running container against the live path — the
container image doesn't ship `sqlite3` today (it's built on
`debian:bookworm-slim` with only `ca-certificates git openssh-client`
installed), so exec the backup from a throwaway container that mounts the
same volume instead:

```sh
docker run --rm \
  -v zync_zync-data:/data \
  -v "$(pwd)/backups:/backups" \
  alpine:3 \
  sh -c "apk add --no-cache sqlite >/dev/null && \
         sqlite3 /data/zync.db \".backup '/backups/zync-\$(date +%Y%m%d-%H%M%S).db'\""
```

(`zync_zync-data` is Compose's default volume name — `<project>_<volume>`;
run `docker volume ls | grep zync-data` to confirm the exact name if your
Compose project name differs from the directory name `zync`.)

If you'd rather not run a second container, `docker cp` a `.backup` output
that's already inside the running `zync` container's writable layer — but
since the `zync` container has no `sqlite3` binary, the volume-mount approach
above is the practical option unless you add `sqlite3` to the image.

## 3. Restore

SQLite is single-writer: **stop the server before replacing the DB file.**
The server holds one shared `Arc<Mutex<Connection>>` for its own writes, but
nothing stops a second process (a stray `zync`, a `sqlite3` shell)
from opening the same file concurrently — always fully stop the service
first.

### Bare-metal / systemd

```sh
systemctl stop zync   # or however the process is supervised

# Remove any WAL/SHM sidecars from the old file so nothing stale is replayed.
rm -f /path/to/zync.db-wal /path/to/zync.db-shm

cp /backups/zync-20260725-020000.db /path/to/zync.db

# ZYNC_SECRET_KEY must be the SAME key that was active when the backup's
# credentials were encrypted — see §4. Confirm it's set in the environment
# the service will start with (e.g. the systemd unit's EnvironmentFile),
# then start the service.
systemctl start zync
```

### Docker Compose

```sh
docker compose down zync     # stop the writer; leaves the volume intact

# Copy the backup into the zync-data volume via a throwaway container.
docker run --rm \
  -v zync_zync-data:/data \
  -v "$(pwd)/backups:/backups" \
  busybox \
  sh -c "rm -f /data/zync.db-wal /data/zync.db-shm && cp /backups/zync-20260725-020000.db /data/zync.db"

# Confirm ZYNC_SECRET_KEY in docker-compose.yml (or its .env) is the same
# key used when the backup's credentials were encrypted, then bring it back up.
docker compose up -d zync
```

### Migrations run automatically on boot

`Database::open` calls `migrate()` on every startup, which applies any
pending schema migration tracked via `PRAGMA user_version`
(`run_migrations` / `MIGRATIONS` in `crates/server/src/db/mod.rs`). Each
migration is transactional and idempotent against an already-migrated
schema, so:

- **Restoring an older backup into a newer server binary is safe** — the
  newer binary runs whatever migrations the old backup hasn't seen yet, the
  same as it would on first boot of a pre-upgrade database.
- **Restoring a newer backup into an older server binary is not safe** — an
  older binary doesn't know how to interpret a schema shape from migrations
  it has never seen, and `PRAGMA user_version` won't roll a schema
  *backward*. Always restore into a server binary at the same version as (or
  newer than) the one that produced the backup.

### Verify the restore

Once the server is back up, confirm both probes (`crates/server/src/observability.rs`):

```sh
curl -sf http://127.0.0.1:58271/health
# {"status":"ok","version":"..."}   — process is alive; no DB touch yet

curl -sf http://127.0.0.1:58271/ready
# {"status":"ready"}                — confirms the restored zync.db actually
#                                      opened and answered a real query
```

`/ready` does a non-mutating lookup of the seeded `owner` user row
(`state.db.user_by_id("owner")`), so a `200 {"status":"ready"}` response is a
real signal the restored database file is valid and readable by this binary
— not just that the file exists. A `503 {"status":"not_ready"}` means the DB
didn't answer (wrong path, corrupt file, permissions) and needs
investigation before declaring the restore done.

Then confirm application-level correctness: log in, open a previously
registered repository, and check that a credential (if any were configured)
still decrypts successfully on a remote operation — this is the step that
actually exercises `ZYNC_SECRET_KEY` matching (see §4).

## 4. The `ZYNC_SECRET_KEY` caveat — read this before you need it

`crypto::KeyState::load()` (`crates/server/src/crypto/mod.rs`) reads
`ZYNC_SECRET_KEY` (base64, must decode to exactly 32 bytes) once at process
startup and never persists it anywhere — not in `zync.db`, not on disk in
any file this server writes. Every `credentials.secret_cipher`/
`secret_nonce` pair in the database was encrypted under whatever key was
active at write time.

**Consequences for backup/restore:**

- A `zync.db` backup is **useless for decrypting stored credentials** unless
  restored alongside the exact same `ZYNC_SECRET_KEY` that was in effect
  when those credentials were written. There is no recovery path for a
  lost or rotated key — `decrypt` (AEAD) simply fails, by design (ADR-001).
- **Do not** put `ZYNC_SECRET_KEY` in the same backup archive, volume, or
  location as `zync.db`. Storing the key next to its own ciphertext defeats
  the point of encrypting it in the first place. Back the key up separately
  and securely — a secrets manager (Vault, AWS/GCP secret manager, a sealed
  secret in your orchestrator), or at minimum a separately access-controlled
  location with its own retention/audit trail, not the same disk or bucket
  as the `.db` backups from §2.
- If you ever rotate `ZYNC_SECRET_KEY`, every previously stored credential
  becomes undecryptable under the new key. There is no re-encryption path
  in this codebase today — rotating the key means users must re-enter their
  credentials after the rotation.
- Restoring a `zync.db` backup with a **different or missing**
  `ZYNC_SECRET_KEY` (or no key at all — `KeyState::Unconfigured`) still
  boots the server fine (registry, users, sessions all work normally); only
  credential decrypt operations fail with `CryptoError::NotConfigured` /
  `CryptoError::Decrypt` at the point they're used, not at startup.

## 5. Scheduling

A cron entry calling the same online-backup command from §2, with rotation
so backups don't accumulate forever, and — critically — **written to a
different disk/volume than the live database**, so a lost disk/volume
doesn't take out both the live DB and its backups together.

```cron
# /etc/cron.d/zync-backup — daily at 02:00, keep 14 days, off-volume target.
0 2 * * * zync sqlite3 /path/to/zync.db ".backup '/mnt/backup-disk/zync/zync-$(date +\%Y\%m\%d-\%H\%M\%S).db'" \
  && find /mnt/backup-disk/zync -name 'zync-*.db' -mtime +14 -delete
```

Notes:

- `/mnt/backup-disk` here stands for any location that is **not** the same
  filesystem/volume as `zync.db` itself (a separate disk, an object store
  synced by a separate job, a different Docker volume backed by different
  underlying storage) and, ideally, off the same host entirely for disaster
  recovery.
- In the Docker Compose deployment, run the equivalent of the §2 Docker
  snippet from a host cron job (or a dedicated backup sidecar/`docker compose
  run` on a timer) rather than cron *inside* the `zync` container — the
  slim runtime image doesn't ship `cron` or `sqlite3`.
- Rotate/retain per your own compliance needs; 14 daily copies is a
  starting point, not a policy.
- `ZYNC_SECRET_KEY` is not part of this schedule — it belongs in whatever
  separate secrets-backup process your organization already uses (see §4).

## 6. Test your restore — checklist

A backup you have never restored is not verified. Periodically (at minimum
whenever you change the schema, rotate infrastructure, or after setting this
up for the first time):

- [ ] Run the §2 backup command against the live DB.
- [ ] `sqlite3 <backup>.db "PRAGMA integrity_check;"` returns `ok`.
- [ ] Restore the backup into a **scratch environment** (a throwaway
      container or a second `ZYNC_DB` path), not production — pointed at the
      matching `ZYNC_SECRET_KEY` from your secrets store.
- [ ] Start `zync` against the restored file; `/health` returns
      `{"status":"ok",...}`.
- [ ] `/ready` returns `{"status":"ready"}` (confirms the restored DB opened
      and answered a real query).
- [ ] Log in as a known user from the restored DB and confirm the expected
      repositories/workspaces/members are present.
- [ ] If credentials were configured, exercise one (e.g. a fetch/push
      against a remote using a stored token/SSH key) and confirm it decrypts
      and works — this is the only way to confirm the `ZYNC_SECRET_KEY` you
      backed up separately is actually the right one.
- [ ] Tear down the scratch environment; it now holds live decrypted-capable
      credentials and should not be left running.
