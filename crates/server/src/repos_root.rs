//! Filesystem boundary for user-supplied repository paths (PLAN.md P4.1,
//! DESIGN.md ADR-001/ADR-002 deferred-boundary notes).
//!
//! `ZYNC_REPOS_ROOT` is a colon-separated list of directories a caller is
//! allowed to register, clone into, or `git init` under; `GET /directories`
//! is confined to the same roots. Without it, any authenticated caller can
//! register (and thereby read/write) an arbitrary path the server process
//! can see — e.g. `POST /repositories {"path":"/home/otheruser/private"}` —
//! and `/directories` can enumerate the whole host filesystem.
//!
//! Same boot-time posture as `crypto::KeyState`/`auth::AuthState`: resolved
//! once from the environment at startup. Unlike those, an unset
//! `ZYNC_REPOS_ROOT` is not a degraded mode — it's today's existing
//! unbounded behavior (P4.1 back-compat for existing single-user LAN
//! deploys), so every enforcement call site gates on `is_configured()`
//! first. A *set but invalid* root (doesn't exist, not a directory) is
//! treated as a boot-time configuration error and refuses to start, the same
//! way an unknown `ZYNC_AUTH` value does — a security boundary that
//! silently resolves to "no roots configured" would be worse than not
//! booting.

use std::path::{Component, Path, PathBuf};

#[derive(Clone, Default)]
pub struct ReposRoot {
    roots: Vec<PathBuf>,
}

impl ReposRoot {
    /// Resolves `ZYNC_REPOS_ROOT` (colon-separated) at boot. Each entry is
    /// canonicalized immediately so later `ensure_within` calls compare
    /// against absolute, symlink-resolved paths. Unset or blank = disabled
    /// (today's unbounded behavior).
    pub fn load() -> anyhow::Result<Self> {
        let raw = match std::env::var("ZYNC_REPOS_ROOT") {
            Ok(raw) if !raw.trim().is_empty() => raw,
            _ => return Ok(Self::default()),
        };

        let mut roots = Vec::new();
        for entry in raw.split(':') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let canonical = Path::new(entry).canonicalize().map_err(|err| {
                anyhow::anyhow!(
                    "ZYNC_REPOS_ROOT entry '{entry}' could not be resolved: {err} \
                     (it must exist and be a directory at boot)"
                )
            })?;
            if !canonical.is_dir() {
                anyhow::bail!("ZYNC_REPOS_ROOT entry '{entry}' is not a directory");
            }
            roots.push(canonical);
        }
        if roots.is_empty() {
            anyhow::bail!("ZYNC_REPOS_ROOT is set but contains no usable entries");
        }

        tracing::info!(
            "ZYNC_REPOS_ROOT enforced: {}",
            roots
                .iter()
                .map(|root| root.display().to_string())
                .collect::<Vec<_>>()
                .join(":")
        );
        Ok(Self { roots })
    }

    pub fn is_configured(&self) -> bool {
        !self.roots.is_empty()
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Resolves `candidate` and asserts it falls under one of the configured
    /// roots. Callers gate on [`Self::is_configured`] first — calling this
    /// with no roots configured always errors, so "unenforced" and "in
    /// bounds" stay distinguishable.
    pub fn ensure_within(&self, candidate: &Path) -> anyhow::Result<PathBuf> {
        within_repos_root(&self.roots, candidate)
    }

    #[cfg(test)]
    pub(crate) fn for_test(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }
}

/// Resolves `candidate` against `roots`, rejecting anything that escapes
/// them: symlink escapes, `..` traversal, or an absolute path outside every
/// root. `candidate` need not exist yet — a clone destination or `git init`
/// target may not exist until the operation that creates it — so the
/// deepest existing ancestor is canonicalized (resolving any symlinks along
/// it) and the remaining, not-yet-existing components are appended
/// lexically.
///
/// `roots` empty always errors — callers gate on `ReposRoot::is_configured()`
/// so "no roots configured" is never misread as "everything allowed".
pub fn within_repos_root(roots: &[PathBuf], candidate: &Path) -> anyhow::Result<PathBuf> {
    if roots.is_empty() {
        anyhow::bail!("ZYNC_REPOS_ROOT is not configured");
    }
    // A literal `..` component is rejected outright, before any filesystem
    // resolution: a legitimate mounted repository path never needs one, and
    // this gives a clear, consistent error regardless of what exists on disk
    // yet (resolution below would also eventually reject the escape, but
    // only after walking the filesystem).
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        anyhow::bail!("path must not contain '..'");
    }

    let resolved = resolve_maybe_missing(candidate)?;
    if roots.iter().any(|root| resolved.starts_with(root)) {
        Ok(resolved)
    } else {
        anyhow::bail!(
            "path '{}' is outside the allowed repository roots",
            resolved.display()
        )
    }
}

/// Like [`Path::canonicalize`], but tolerant of a path whose final
/// component(s) don't exist yet: walks up to the nearest existing ancestor,
/// canonicalizes that (resolving any symlinks along the way, so a symlinked
/// existing prefix can't be used to escape), then re-appends the
/// non-existent tail lexically.
///
/// Security note (P4.1 review W1): `canonicalize()` fails both for a
/// component that's genuinely absent AND for one that exists but is an
/// unresolvable symlink (dangling — target doesn't exist, or a symlink
/// loop). Those two cases must NOT be treated the same: a dangling symlink
/// `root/link -> /etc/cron.d/evil` means `root/link/sub` would otherwise get
/// `link/sub` appended lexically onto the canonicalized `root`, pass the
/// `starts_with(root)` check, and then have the caller's real filesystem
/// write (e.g. libgit2's recursive mkdir for clone/init) follow the symlink
/// straight outside the root. `symlink_metadata` (lstat — does not follow
/// the link) distinguishes "nothing here" from "something here that
/// canonicalize couldn't resolve"; the latter is rejected outright rather
/// than treated as an absent path component.
fn resolve_maybe_missing(path: &Path) -> anyhow::Result<PathBuf> {
    let mut existing = path;
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match existing.canonicalize() {
            Ok(base) => {
                let mut resolved = base;
                for part in tail.into_iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(_) => {
                if existing.symlink_metadata().is_ok() {
                    // Something exists at this component (an lstat succeeded)
                    // but canonicalize() couldn't resolve it — a dangling
                    // symlink or a symlink loop. Never silently treat this as
                    // "absent and safe to append lexically."
                    anyhow::bail!(
                        "path '{}' traverses an unresolvable symlink",
                        existing.display()
                    );
                }
                let name = existing
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("cannot resolve path '{}'", path.display()))?
                    .to_owned();
                tail.push(name);
                existing = existing
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("cannot resolve path '{}'", path.display()))?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_existing_path_under_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        let sub = root.join("repo");
        std::fs::create_dir(&sub).expect("create sub");

        let resolved = within_repos_root(&[root], &sub).expect("path is within root");
        assert_eq!(resolved, sub);
    }

    #[test]
    fn accepts_not_yet_existing_path_under_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        let target = root.join("not-created-yet");

        let resolved = within_repos_root(&[root], &target).expect("path is within root");
        assert_eq!(resolved, target);
    }

    #[test]
    fn rejects_dotdot_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        let escape = root.join("../escape");

        let err = within_repos_root(&[root], &escape).expect_err("must reject '..'");
        assert!(err.to_string().contains(".."), "unexpected error: {err}");
    }

    #[test]
    fn rejects_absolute_path_outside_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        let other = tempfile::tempdir().expect("other tempdir");
        let other_root = other
            .path()
            .canonicalize()
            .expect("canonicalize other root");

        let err =
            within_repos_root(&[root], &other_root).expect_err("must reject path outside root");
        assert!(
            err.to_string().contains("outside"),
            "unexpected error: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");
        let outside = tempfile::tempdir().expect("outside tempdir");
        let outside_root = outside
            .path()
            .canonicalize()
            .expect("canonicalize outside root");

        let link = root.join("escape-link");
        std::os::unix::fs::symlink(&outside_root, &link).expect("create symlink");

        let err = within_repos_root(&[root], &link).expect_err("must reject symlink escape");
        assert!(
            err.to_string().contains("outside"),
            "unexpected error: {err}"
        );
    }

    /// P4.1 security review W1: a *dangling* symlink (target doesn't exist)
    /// inside the root must not be treated as "component absent, safe to
    /// append lexically" — `root/link/sub` must not resolve to
    /// `canonicalize(root)/link/sub` and pass the `starts_with(root)` check,
    /// because a real filesystem write through `link` (e.g. libgit2's
    /// recursive mkdir on clone/init) would follow the symlink outside the
    /// root regardless of what this function computed.
    #[cfg(unix)]
    #[test]
    fn rejects_dangling_symlink_even_when_followed_by_more_components() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().canonicalize().expect("canonicalize root");

        let link = root.join("dangling-link");
        std::os::unix::fs::symlink("/tmp/zync-p41-does-not-exist", &link)
            .expect("create dangling symlink");
        assert!(
            !link.exists(),
            "sanity: the symlink target must not exist (dangling)"
        );

        // Both the bare dangling link and a path that walks *through* it
        // (as a non-existent intermediate component) must be rejected —
        // never lexically appended and accepted.
        let err = within_repos_root(&[root.clone()], &link)
            .expect_err("must reject the dangling symlink itself");
        assert!(
            err.to_string().contains("unresolvable symlink"),
            "unexpected error: {err}"
        );

        let through_link = link.join("sub").join("repo");
        let err = within_repos_root(&[root], &through_link)
            .expect_err("must reject a path traversing the dangling symlink");
        assert!(
            err.to_string().contains("unresolvable symlink"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn empty_roots_always_errors() {
        let err = within_repos_root(&[], Path::new("/anything")).expect_err("must reject");
        assert!(err.to_string().contains("not configured"));
    }
}
