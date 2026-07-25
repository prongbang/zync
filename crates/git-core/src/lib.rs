use git2::{
    ApplyLocation, AutotagOption, BranchType, Cred, CredentialType, DiffFormat, DiffOptions,
    FetchOptions, FetchPrune, IndexAddOption, MergeOptions, Oid, PushOptions, Remote,
    RemoteCallbacks, Repository, ResetType, Signature, StatusOptions, TreeWalkMode,
    TreeWalkResult,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub path: PathBuf,
    pub head: Option<String>,
    pub current_branch: Option<String>,
    pub is_bare: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub ignored: bool,
    pub conflicted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRef {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    pub id: String,
    pub summary: String,
    pub author: String,
    #[serde(default)]
    pub author_email: String,
    #[serde(default)]
    pub committer: String,
    #[serde(default)]
    pub committer_email: String,
    pub time: i64,
    pub parents: Vec<String>,
    #[serde(default)]
    pub refs: Vec<CommitRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSummary {
    pub name: String,
    pub is_head: bool,
    pub kind: String,
    pub target: Option<String>,
    #[serde(default)]
    pub ahead: Option<usize>,
    #[serde(default)]
    pub behind: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSummary {
    pub name: String,
    pub target: Option<String>,
    /// `true` for an annotated tag (a real tag object with its own message/tagger/date),
    /// `false` for a lightweight tag (a ref pointing directly at a commit).
    pub annotated: bool,
    /// Annotated tags only — the tag message, trailing newline trimmed.
    pub message: Option<String>,
    /// Annotated tags only — the tagger's display name.
    pub tagger: Option<String>,
    /// Annotated tags only — the tagger's timestamp, Unix seconds.
    pub time: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSummary {
    pub name: String,
    pub url: Option<String>,
    pub push_url: Option<String>,
}

/// Credentials for a network Git operation (fetch/pull/push/clone). Callers (the server) decrypt
/// a stored credential just-in-time, build one of these, pass it by reference into a
/// `*_with_credentials` fn, and let it drop at the end of the call — see DESIGN.md ADR-001.
/// Every secret-bearing field is `Zeroizing<String>` so it is wiped from memory on drop.
pub enum CredentialSpec {
    /// HTTPS token/password auth. `username` is the token user (e.g. `"x-access-token"`,
    /// `"oauth2"`, or the account name); `secret` is the PAT/OAuth token/password.
    UserpassPlaintext {
        username: String,
        secret: Zeroizing<String>,
    },
    /// SSH private key held in memory (never written to disk on the libgit2 network paths; only
    /// the CLI-only pull merge/rebase path writes it to a 0600 temp file, per ADR-001).
    SshKey {
        username: String,
        private_key: Zeroizing<String>,
        passphrase: Option<Zeroizing<String>>,
    },
    /// Explicit ssh-agent use (username optional; falls back to the URL's username, then "git").
    SshAgent { username: Option<String> },
    /// Ambient: current behavior — ssh-agent (when the URL carries a username) then
    /// `Cred::default()`. The default when no spec is supplied.
    Default,
}

/// Manual `Debug` so secrets never end up in a log line or panic message: every secret-bearing
/// field prints as `"<redacted>"` regardless of formatter flags.
impl std::fmt::Debug for CredentialSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialSpec::UserpassPlaintext { username, .. } => f
                .debug_struct("UserpassPlaintext")
                .field("username", username)
                .field("secret", &"<redacted>")
                .finish(),
            CredentialSpec::SshKey { username, .. } => f
                .debug_struct("SshKey")
                .field("username", username)
                .field("private_key", &"<redacted>")
                .field("passphrase", &"<redacted>")
                .finish(),
            CredentialSpec::SshAgent { username } => f
                .debug_struct("SshAgent")
                .field("username", username)
                .finish(),
            CredentialSpec::Default => write!(f, "Default"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub start_line: usize,
    pub line_count: usize,
    pub commit: String,
    pub author: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntrySummary {
    pub path: String,
    pub kind: String,
    pub id: String,
    pub size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflogEntrySummary {
    pub index: usize,
    pub old_id: String,
    pub new_id: String,
    pub message: String,
    pub committer: String,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmoduleSummary {
    pub name: String,
    pub path: String,
    pub url: Option<String>,
    pub head: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfsSummary {
    pub configured: bool,
    pub tracked_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorStat {
    pub name: String,
    pub commits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthStat {
    pub year: i64,
    pub month: u32,
    pub total: usize,
    pub top: Vec<AuthorStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStats {
    pub commit_count: usize,
    pub contributors: Vec<AuthorStat>,
    pub monthly: Vec<MonthStat>,
    pub first_commit_time: i64,
    pub last_commit_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashSummary {
    pub index: usize,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictSummary {
    pub ancestor: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDetail {
    pub path: String,
    pub ancestor_path: Option<String>,
    pub ours_path: Option<String>,
    pub theirs_path: Option<String>,
    pub ancestor_content: String,
    pub ours_content: String,
    pub theirs_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseStep {
    pub commit: String,
    pub action: RebaseAction,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebaseAction {
    Pick,
    Squash,
    Fixup,
    Drop,
    Edit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseResult {
    pub head: Option<String>,
    pub stopped_at: Option<String>,
    pub applied: Vec<String>,
    pub dropped: Vec<String>,
}

pub fn open_repo(path: impl AsRef<Path>) -> anyhow::Result<RepoInfo> {
    let repo = Repository::open(path.as_ref())?;
    repo_info(&repo)
}

/// Initializes a new, empty repository at `path` (creating the directory if needed) — no
/// initial commit, matching plain `git init`. `repo_info` handles the resulting headless state
/// gracefully (`head`/`current_branch` come back `None`).
///
/// On failure (either the directory creation or `git init` itself), any directories this call
/// created are removed again — `first_uncreated_ancestor` is captured before touching the
/// filesystem so a pre-existing directory is never deleted.
pub fn init_repo(path: impl AsRef<Path>) -> anyhow::Result<RepoInfo> {
    let path = path.as_ref();
    let created_root = first_uncreated_ancestor(path);

    let result = fs::create_dir_all(path)
        .map_err(anyhow::Error::from)
        .and_then(|()| Repository::init(path).map_err(anyhow::Error::from))
        .and_then(|repo| repo_info(&repo));

    if result.is_err() {
        if let Some(root) = created_root {
            let _ = fs::remove_dir_all(&root);
        }
    }
    result
}

/// Returns the topmost ancestor of `path` that does not currently exist on disk — i.e. the
/// directory `fs::create_dir_all(path)` would actually create (removing it removes every nested
/// directory created alongside it). `None` when `path` already exists, so callers know not to
/// remove anything on a later failure.
fn first_uncreated_ancestor(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return None;
    }
    let mut highest = path.to_path_buf();
    let mut cursor = path;
    while let Some(parent) = cursor.parent() {
        if parent.exists() {
            break;
        }
        highest = parent.to_path_buf();
        cursor = parent;
    }
    Some(highest)
}

pub fn clone_repo(url: &str, destination: impl AsRef<Path>) -> anyhow::Result<RepoInfo> {
    clone_repo_with_credentials(url, destination, None)
}

/// Clones over libgit2 (`RepoBuilder` + `FetchOptions`) so credentials stay in memory and the
/// caller gets real transfer progress for free. `spec: None` behaves like today (ambient
/// ssh-agent / `Cred::default()`).
pub fn clone_repo_with_credentials(
    url: &str,
    destination: impl AsRef<Path>,
    spec: Option<&CredentialSpec>,
) -> anyhow::Result<RepoInfo> {
    let host = remote_host(url);
    let default_spec = CredentialSpec::Default;
    let spec = spec.unwrap_or(&default_spec);

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks(spec));

    let repo = git2::build::RepoBuilder::new()
        .fetch_options(fetch_options)
        .clone(url, destination.as_ref())
        .map_err(|err| {
            map_git2_error(&format!("git clone {}", redact_url_userinfo(url)), &host, err)
        })?;
    repo_info(&repo)
}

/// Strips any inline `scheme://user[:pass]@` userinfo out of every `https://`/`http://`/`ssh://`
/// URL occurring anywhere inside `message` — not just when `message` is itself a bare URL (see
/// ADR-001 "Secrets never enter errors"). A user-registered `https://x-access-token:TOKEN@host/...`
/// remote URL must never leak into `GitCommandError::command`/`stderr` (and from there,
/// `map_git_error`'s HTTP body) or into a persisted `RepositoryRecord`. Keeps the scheme, host,
/// and path; the scp-like `user@host:path` form is left untouched — that `user` is just the ssh
/// login name, never a secret.
///
/// Scanning the *whole message* (rather than assuming it's nothing but a URL) matters because
/// libgit2 error text isn't always just the URL — a DNS/TLS failure on the clone path can echo
/// the full request URL, userinfo included, embedded in a longer sentence.
pub fn redact_url_userinfo(message: &str) -> String {
    const SCHEMES: [&str; 3] = ["https://", "http://", "ssh://"];
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while !rest.is_empty() {
        let Some(scheme) = SCHEMES.iter().find(|scheme| rest.starts_with(**scheme)) else {
            // Advance by one char (not one byte) to stay on a UTF-8 boundary.
            let ch = rest.chars().next().expect("rest is non-empty");
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
            continue;
        };
        out.push_str(scheme);
        let after_scheme = &rest[scheme.len()..];
        // The authority (where userinfo can live) ends at the first `/` or whitespace, or at
        // the end of the string — never past the host/port section.
        let authority_end = after_scheme
            .find(|c: char| c == '/' || c.is_whitespace())
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        match authority.find('@') {
            Some(idx) => out.push_str(&authority[idx + 1..]),
            None => out.push_str(authority),
        }
        rest = &after_scheme[authority_end..];
    }
    out
}

pub fn fetch(path: impl AsRef<Path>, remote_name: Option<&str>) -> anyhow::Result<String> {
    fetch_with_credentials(path, remote_name, None)
}

/// Fetches over libgit2 `Remote::fetch` (download tags auto, prune off — pruning stays a
/// separate explicit `prune_remote` op). Updates `FETCH_HEAD` the same way `git fetch` does
/// (libgit2's default). `spec: None` behaves like today.
pub fn fetch_with_credentials(
    path: impl AsRef<Path>,
    remote_name: Option<&str>,
    spec: Option<&CredentialSpec>,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let remote_name = remote_name.unwrap_or("origin");
    let mut remote = repo.find_remote(remote_name)?;
    let host = remote_host(remote.url().unwrap_or(""));
    let default_spec = CredentialSpec::Default;
    let spec = spec.unwrap_or(&default_spec);

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks(spec));
    fetch_options.download_tags(AutotagOption::Auto);
    fetch_options.prune(FetchPrune::Off);

    remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .map_err(|err| map_git2_error(&format!("git fetch {remote_name}"), &host, err))?;

    let stats = remote.stats();
    Ok(format!(
        "fetched {} object(s) from {remote_name}",
        stats.total_objects()
    ))
}

/// Pull strategy. `FfOnly` runs over libgit2 (fetch + fast-forward the local branch). `Merge`
/// and `Rebase` stay on the `git` CLI — reimplementing merge/rebase conflict resolution on top
/// of libgit2's plumbing is exactly the error-prone logic the CLI already gets right (ADR-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullMode {
    FfOnly,
    Merge,
    Rebase,
}

pub fn pull(
    path: impl AsRef<Path>,
    remote_name: Option<&str>,
    branch: Option<&str>,
) -> anyhow::Result<String> {
    pull_with_credentials(path, remote_name, branch, PullMode::FfOnly, None)
}

/// `mode` selects the strategy (see [`PullMode`]); `spec: None` behaves like today. `FfOnly`
/// runs entirely over libgit2 (credentials stay in memory). `Merge`/`Rebase` shell out to `git
/// pull` with credentials injected via environment only, never argv:
/// - HTTPS (`UserpassPlaintext`): a one-shot `GIT_ASKPASS` shim script (0700 temp file, removed
///   after) echoes the secret from an env var set on the child process.
/// - SSH (`SshKey`): the private key is written to a 0600 temp file for the duration of the
///   call (`GIT_SSH_COMMAND` points at it) and removed immediately after — the only place an
///   in-memory key ever touches disk.
pub fn pull_with_credentials(
    path: impl AsRef<Path>,
    remote_name: Option<&str>,
    branch: Option<&str>,
    mode: PullMode,
    spec: Option<&CredentialSpec>,
) -> anyhow::Result<String> {
    let path = path.as_ref();
    let repo = Repository::open(path)?;
    let remote_name = remote_name.unwrap_or("origin");
    let branch_name = branch
        .map(ToOwned::to_owned)
        .or_else(|| upstream_branch(&repo).ok().flatten())
        .or_else(|| current_branch(&repo).ok().flatten())
        .ok_or_else(|| anyhow::anyhow!("cannot pull without a current branch"))?;

    let default_spec = CredentialSpec::Default;
    let spec = spec.unwrap_or(&default_spec);

    match mode {
        PullMode::FfOnly => pull_ff_only(&repo, remote_name, &branch_name, spec),
        PullMode::Merge | PullMode::Rebase => {
            pull_via_cli(path, remote_name, &branch_name, mode, spec)
        }
    }
}

/// `git fetch` + fast-forward of the local branch, entirely over libgit2.
fn pull_ff_only(
    repo: &Repository,
    remote_name: &str,
    branch_name: &str,
    spec: &CredentialSpec,
) -> anyhow::Result<String> {
    // Refuse to silently switch branches: a plain `git pull` only ever fast-forwards whatever
    // is currently checked out. If the caller resolved a different branch (e.g. an explicit
    // `branch` argument that isn't HEAD), bail instead of quietly moving/checking out a branch
    // the user didn't ask to touch.
    let head_branch = current_branch(repo)?;
    if head_branch.as_deref() != Some(branch_name) {
        return Err(anyhow::anyhow!(
            "cannot ff-only pull '{branch_name}': the checked-out branch is {}",
            head_branch
                .map(|name| format!("'{name}'"))
                .unwrap_or_else(|| "detached (no current branch)".to_string())
        ));
    }

    let mut remote = repo.find_remote(remote_name)?;
    let host = remote_host(remote.url().unwrap_or(""));

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks(spec));
    fetch_options.download_tags(AutotagOption::Auto);
    fetch_options.prune(FetchPrune::Off);
    // Empty refspecs = the remote's configured fetch refspecs, same as `fetch_with_credentials`
    // uses. A bare branch-name refspec (e.g. just `"main"`) isn't guaranteed to update
    // refs/remotes/<remote>/<branch> the way the configured mapping does, and that
    // remote-tracking ref is exactly what we (and push_force_with_lease's freshness check)
    // read next — so it needs to be reliably current.
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .map_err(|err| {
            map_git2_error(
                &format!("git pull --ff-only {remote_name} {branch_name}"),
                &host,
                err,
            )
        })?;

    let tracking_ref_name = format!("refs/remotes/{remote_name}/{branch_name}");
    let tracking_ref = repo.find_reference(&tracking_ref_name).map_err(|_| {
        anyhow::anyhow!("remote '{remote_name}' has no branch '{branch_name}' after fetch")
    })?;
    let fetch_commit = repo.reference_to_annotated_commit(&tracking_ref)?;
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    if analysis.0.is_up_to_date() {
        return Ok("Already up to date.".to_string());
    }
    if !analysis.0.is_fast_forward() {
        return Err(GitCommandError {
            command: format!("git pull --ff-only {remote_name} {branch_name}"),
            stderr: "not possible to fast-forward, aborting (local and remote history have diverged)"
                .to_string(),
            kind: GitErrorKind::NonFastForward,
        }
        .into());
    }

    let target = fetch_commit.id();
    let target_commit = repo.find_commit(target)?;
    // Check out the target tree BEFORE moving any ref. git2's "safe" checkout aborts cleanly on
    // a dirty/conflicting working tree instead of clobbering uncommitted local changes, which a
    // set_target/set_head-first ordering (followed by a *forced* checkout_head) would silently
    // overwrite — real `git pull --ff-only` aborts in that situation rather than losing data.
    repo.checkout_tree(
        target_commit.as_object(),
        Some(git2::build::CheckoutBuilder::default().safe()),
    )?;

    let refname = format!("refs/heads/{branch_name}");
    match repo.find_reference(&refname) {
        Ok(mut reference) => {
            reference.set_target(target, "zync: fast-forward pull")?;
        }
        Err(_) => {
            repo.reference(&refname, target, true, "zync: fast-forward pull")?;
        }
    }
    repo.set_head(&refname)?;

    Ok(format!("Fast-forwarded {branch_name} to {target}"))
}

/// `git pull`/`git pull --rebase` via the hardened `run_git` shellout, with credentials injected
/// through environment variables only (see [`pull_with_credentials`] doc for the mechanism).
fn pull_via_cli(
    path: &Path,
    remote_name: &str,
    branch_name: &str,
    mode: PullMode,
    spec: &CredentialSpec,
) -> anyhow::Result<String> {
    let args: Vec<&str> = match mode {
        PullMode::Merge => vec!["pull", remote_name, branch_name],
        PullMode::Rebase => vec!["pull", "--rebase", remote_name, branch_name],
        PullMode::FfOnly => unreachable!("ff-only pulls run over libgit2, see pull_ff_only"),
    };

    match spec {
        CredentialSpec::UserpassPlaintext { secret, .. } => {
            // `shim` (a `TempSecretFile`) deletes itself on drop at the end of this arm,
            // regardless of whether run_git_with_env returns Ok or Err (or panics).
            let shim = write_askpass_shim()?;
            run_git_with_env(
                path,
                &args,
                &[
                    ("GIT_ASKPASS", shim.path().to_string_lossy().as_ref()),
                    ("ZYNC_ASKPASS_TOKEN", secret.as_str()),
                ],
            )
        }
        CredentialSpec::SshKey { private_key, .. } => {
            let key_file = write_temp_secret_file("zync-ssh-key", private_key, 0o600)?;
            let ssh_command = format!(
                "ssh -i {} -oBatchMode=yes -oIdentitiesOnly=yes",
                key_file.path().display()
            );
            run_git_with_env(path, &args, &[("GIT_SSH_COMMAND", &ssh_command)])
        }
        CredentialSpec::SshAgent { .. } | CredentialSpec::Default => run_git(path, &args),
    }
}

pub fn push(
    path: impl AsRef<Path>,
    remote_name: Option<&str>,
    branch: Option<&str>,
) -> anyhow::Result<String> {
    push_with_credentials(path, remote_name, branch, None)
}

/// Pushes over libgit2 `Remote::push` (current refspec semantics preserved: pushes the current
/// branch when `branch` is `None`, and sets it as upstream on success — matching the old `git
/// push -u` behavior). `spec: None` behaves like today.
pub fn push_with_credentials(
    path: impl AsRef<Path>,
    remote_name: Option<&str>,
    branch: Option<&str>,
    spec: Option<&CredentialSpec>,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let remote_name = remote_name.unwrap_or("origin");
    let branch_name = branch
        .map(ToOwned::to_owned)
        .or_else(|| current_branch(&repo).ok().flatten())
        .ok_or_else(|| anyhow::anyhow!("cannot push without a current branch"))?;

    let mut remote = repo.find_remote(remote_name)?;
    let host = remote_host(remote.url().unwrap_or(""));
    let default_spec = CredentialSpec::Default;
    let spec = spec.unwrap_or(&default_spec);

    let refspec = format!("refs/heads/{branch_name}:refs/heads/{branch_name}");
    let command = format!("git push -u {remote_name} {branch_name}");
    push_refspecs(&mut remote, &[refspec], spec, &host, &command)?;

    // The push already landed on the remote at this point — a failure to record the local
    // upstream-tracking config (a purely local, non-network op) shouldn't be reported as the
    // push itself having failed. Degrade to a note in the success message instead.
    let message = format!("pushed {branch_name} to {remote_name}");
    match set_upstream(path.as_ref(), &branch_name, remote_name, &branch_name) {
        Ok(_) => Ok(message),
        Err(err) => Ok(format!(
            "{message} (warning: failed to set upstream tracking: {err})"
        )),
    }
}

pub fn remotes(path: impl AsRef<Path>) -> anyhow::Result<Vec<RemoteSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let names = repo.remotes()?;
    let mut remotes = Vec::new();
    for name in names.iter().flatten() {
        let remote = repo.find_remote(name)?;
        remotes.push(RemoteSummary {
            name: name.to_string(),
            url: remote.url().map(ToOwned::to_owned),
            push_url: remote.pushurl().map(ToOwned::to_owned),
        });
    }
    Ok(remotes)
}

pub fn add_remote(path: impl AsRef<Path>, name: &str, url: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    repo.remote(name, url)?;
    Ok(())
}

pub fn delete_remote(path: impl AsRef<Path>, name: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    repo.remote_delete(name)?;
    Ok(())
}

pub fn prune_remote(path: impl AsRef<Path>, remote_name: &str) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["remote", "prune", remote_name])
}

pub fn delete_remote_branch(
    path: impl AsRef<Path>,
    remote_name: &str,
    branch: &str,
) -> anyhow::Result<()> {
    delete_remote_branch_with_credentials(path, remote_name, branch, None)
}

pub fn delete_remote_branch_with_credentials(
    path: impl AsRef<Path>,
    remote_name: &str,
    branch: &str,
    spec: Option<&CredentialSpec>,
) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let mut remote = repo.find_remote(remote_name)?;
    let host = remote_host(remote.url().unwrap_or(""));
    let default_spec = CredentialSpec::Default;
    let spec = spec.unwrap_or(&default_spec);

    let refspec = format!(":refs/heads/{branch}");
    let command = format!("git push {remote_name} :{branch}");
    push_refspecs(&mut remote, &[refspec], spec, &host, &command)?;
    Ok(())
}

pub fn push_tag(path: impl AsRef<Path>, remote_name: &str, tag: &str) -> anyhow::Result<String> {
    push_tag_with_credentials(path, remote_name, tag, None)
}

/// Pushes `refs/tags/<tag>:refs/tags/<tag>` to `remote_name` over the same libgit2
/// `push_refspecs` path used by branch pushes (ADR-001 credentialed transport). `spec: None`
/// behaves like today (ambient ssh-agent / `Cred::default()`).
pub fn push_tag_with_credentials(
    path: impl AsRef<Path>,
    remote_name: &str,
    tag: &str,
    spec: Option<&CredentialSpec>,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let mut remote = repo.find_remote(remote_name)?;
    let host = remote_host(remote.url().unwrap_or(""));
    let default_spec = CredentialSpec::Default;
    let spec = spec.unwrap_or(&default_spec);

    let refspec = format!("refs/tags/{tag}:refs/tags/{tag}");
    let command = format!("git push {remote_name} refs/tags/{tag}");
    push_refspecs(&mut remote, &[refspec], spec, &host, &command)?;
    Ok(format!("pushed tag {tag} to {remote_name}"))
}

pub fn set_upstream(
    path: impl AsRef<Path>,
    branch: &str,
    remote_name: &str,
    remote_branch: &str,
) -> anyhow::Result<String> {
    let upstream = format!("{remote_name}/{remote_branch}");
    run_git(
        path.as_ref(),
        &["branch", "--set-upstream-to", &upstream, branch],
    )
}

pub fn push_force_with_lease(
    path: impl AsRef<Path>,
    remote_name: &str,
    branch: &str,
) -> anyhow::Result<String> {
    push_force_with_lease_with_credentials(path, remote_name, branch, None)
}

/// libgit2 has no native "force-with-lease" (its push refspec expresses force but carries no
/// expected-old-oid check), so the lease is implemented client-side per ADR-001: connect and
/// list the remote's current oid for `branch`, compare it against our locally cached
/// remote-tracking ref (`refs/remotes/<remote>/<branch>`), and only proceed with a forced push
/// if they match — i.e. nothing has landed on the remote since our last fetch of it. A mismatch
/// (or the remote having a branch we don't know about at all) is rejected as a stale lease.
pub fn push_force_with_lease_with_credentials(
    path: impl AsRef<Path>,
    remote_name: &str,
    branch: &str,
    spec: Option<&CredentialSpec>,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let mut remote = repo.find_remote(remote_name)?;
    let host = remote_host(remote.url().unwrap_or(""));
    let default_spec = CredentialSpec::Default;
    let spec = spec.unwrap_or(&default_spec);
    let command = format!("git push --force-with-lease {remote_name} {branch}");

    let tracking_ref = format!("refs/remotes/{remote_name}/{branch}");
    let expected_oid = repo
        .find_reference(&tracking_ref)
        .ok()
        .and_then(|reference| reference.target());

    let remote_branch_ref = format!("refs/heads/{branch}");
    let actual_oid = {
        let connection = remote
            .connect_auth(git2::Direction::Fetch, Some(callbacks(spec)), None)
            .map_err(|err| map_git2_error(&command, &host, err))?;
        let heads = connection
            .list()
            .map_err(|err| map_git2_error(&command, &host, err))?;
        heads
            .iter()
            .find(|head| head.name() == remote_branch_ref)
            .map(|head| head.oid())
    };

    if expected_oid != actual_oid {
        return Err(GitCommandError {
            command,
            stderr: format!(
                "stale info: {remote_name}/{branch} has moved since the last fetch; fetch before forcing"
            ),
            kind: GitErrorKind::NonFastForward,
        }
        .into());
    }

    let refspec = format!("+refs/heads/{branch}:refs/heads/{branch}");
    push_refspecs(&mut remote, &[refspec], spec, &host, &command)?;

    Ok(format!(
        "force-pushed {branch} to {remote_name} (lease verified)"
    ))
}

pub fn status(path: impl AsRef<Path>) -> anyhow::Result<Vec<FileStatus>> {
    let repo = Repository::open(path.as_ref())?;
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo.statuses(Some(&mut options))?;
    let mut files = Vec::new();
    for entry in statuses.iter() {
        let flags = entry.status();
        let path = entry
            .head_to_index()
            .and_then(|d| d.new_file().path())
            .or_else(|| entry.index_to_workdir().and_then(|d| d.new_file().path()))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        files.push(FileStatus {
            path,
            staged: flags.intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE,
            ),
            unstaged: flags.intersects(
                git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED
                    | git2::Status::WT_RENAMED
                    | git2::Status::WT_TYPECHANGE,
            ),
            untracked: flags.contains(git2::Status::WT_NEW),
            ignored: flags.contains(git2::Status::IGNORED),
            conflicted: flags.contains(git2::Status::CONFLICTED),
        });
    }
    Ok(files)
}

pub fn add(path: impl AsRef<Path>, files: &[String]) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let mut index = repo.index()?;
    if files.is_empty() {
        index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    } else {
        for file in files {
            index.add_path(Path::new(file))?;
        }
    }
    index.write()?;
    Ok(())
}

pub fn unstage(path: impl AsRef<Path>, files: &[String]) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let head = repo
        .head()
        .ok()
        .and_then(|head| head.peel(git2::ObjectType::Commit).ok());
    repo.reset_default(head.as_ref(), files)?;
    Ok(())
}

pub fn discard(path: impl AsRef<Path>, files: &[String]) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    for file in files {
        checkout.path(file);
    }
    repo.checkout_head(Some(&mut checkout))?;
    Ok(())
}

pub fn stage_patch(path: impl AsRef<Path>, patch: &[u8]) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let diff = git2::Diff::from_buffer(patch)?;
    repo.apply(&diff, ApplyLocation::Index, None)?;
    Ok(())
}

pub fn commit(
    path: impl AsRef<Path>,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let signature = Signature::now(author_name, author_email)?;
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let parent = repo
        .head()
        .ok()
        .and_then(|head| head.target())
        .and_then(|oid| repo.find_commit(oid).ok());

    let parents = parent.iter().collect::<Vec<_>>();
    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;
    Ok(oid.to_string())
}

pub fn amend_commit(
    path: impl AsRef<Path>,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let signature = Signature::now(author_name, author_email)?;
    let head = repo.head()?.peel_to_commit()?;
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let oid = head.amend(
        Some("HEAD"),
        Some(&signature),
        Some(&signature),
        None,
        Some(message),
        Some(&tree),
    )?;
    Ok(oid.to_string())
}

pub fn branches(path: impl AsRef<Path>) -> anyhow::Result<Vec<BranchSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let mut branches = Vec::new();
    for item in repo.branches(None)? {
        let (branch, kind) = item?;
        let name = branch.name()?.unwrap_or("unknown").to_string();
        let target = branch.get().target().map(|oid| oid.to_string());
        let (ahead, behind) = match kind {
            BranchType::Local => target
                .as_deref()
                .and_then(|_| branch.get().target())
                .and_then(|local_oid| {
                    branch
                        .upstream()
                        .ok()
                        .and_then(|upstream| upstream.get().target())
                        .and_then(|upstream_oid| {
                            repo.graph_ahead_behind(local_oid, upstream_oid).ok()
                        })
                })
                .map(|(ahead, behind)| (Some(ahead), Some(behind)))
                .unwrap_or((None, None)),
            BranchType::Remote => (None, None),
        };
        branches.push(BranchSummary {
            name,
            is_head: branch.is_head(),
            kind: match kind {
                BranchType::Local => "local",
                BranchType::Remote => "remote",
            }
            .to_string(),
            target,
            ahead,
            behind,
        });
    }
    Ok(branches)
}

pub fn tags(path: impl AsRef<Path>) -> anyhow::Result<Vec<TagSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let names = repo.tag_names(None)?;
    let mut tags = Vec::new();
    for name in names.iter().flatten() {
        // `^{commit}` peels an annotated tag down to the commit it ultimately points at (a
        // bare `refs/tags/<name>` revspec resolves to the tag object itself for annotated
        // tags) — lightweight tags already point straight at the commit either way.
        let target = repo
            .revparse_single(&format!("refs/tags/{name}^{{commit}}"))
            .ok()
            .map(|object| object.id().to_string());

        // A ref's direct target (before peeling) is either an annotated tag object (for
        // annotated tags) or the commit itself (for lightweight tags) — `find_tag` only
        // succeeds in the former case, which is how we tell them apart.
        let annotated_tag = repo
            .find_reference(&format!("refs/tags/{name}"))
            .ok()
            .and_then(|reference| reference.target())
            .and_then(|oid| repo.find_tag(oid).ok());

        let (annotated, message, tagger, time) = match &annotated_tag {
            Some(tag) => (
                true,
                tag.message().map(|message| message.trim_end().to_string()),
                tag.tagger()
                    .and_then(|signature| signature.name().map(ToOwned::to_owned)),
                tag.tagger().map(|signature| signature.when().seconds()),
            ),
            None => (false, None, None, None),
        };

        tags.push(TagSummary {
            name: name.to_string(),
            target,
            annotated,
            message,
            tagger,
            time,
        });
    }
    Ok(tags)
}

pub fn create_tag(path: impl AsRef<Path>, name: &str, target: Option<&str>) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let object = target
        .map(|revision| repo.revparse_single(revision))
        .unwrap_or_else(|| repo.head()?.peel(git2::ObjectType::Commit))?;
    repo.tag_lightweight(name, &object, false)?;
    Ok(())
}

pub fn delete_tag(path: impl AsRef<Path>, name: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    repo.tag_delete(name)?;
    Ok(())
}

fn commit_ref_map(repo: &Repository) -> HashMap<Oid, Vec<CommitRef>> {
    let mut map: HashMap<Oid, Vec<CommitRef>> = HashMap::new();
    let head_target = repo
        .head()
        .ok()
        .filter(|head| head.is_branch())
        .and_then(|head| head.shorthand().map(ToOwned::to_owned));
    let Ok(references) = repo.references() else {
        return map;
    };
    for reference in references.flatten() {
        let Some(name) = reference.shorthand().map(ToOwned::to_owned) else {
            continue;
        };
        let kind = if reference.is_branch() {
            if head_target.as_deref() == Some(name.as_str()) {
                "head"
            } else {
                "local"
            }
        } else if reference.is_remote() {
            "remote"
        } else if reference.is_tag() {
            "tag"
        } else {
            continue;
        };
        let Ok(commit) = reference.peel_to_commit() else {
            continue;
        };
        map.entry(commit.id()).or_default().push(CommitRef {
            name,
            kind: kind.to_string(),
        });
    }
    for refs in map.values_mut() {
        refs.sort_by(|a, b| ref_kind_order(&a.kind).cmp(&ref_kind_order(&b.kind)));
    }
    map
}

fn ref_kind_order(kind: &str) -> u8 {
    match kind {
        "head" => 0,
        "local" => 1,
        "remote" => 2,
        "tag" => 3,
        _ => 4,
    }
}

fn summarize_commit(commit: &git2::Commit, refs: Vec<CommitRef>) -> CommitSummary {
    CommitSummary {
        id: commit.id().to_string(),
        summary: commit.summary().unwrap_or("").to_string(),
        author: commit.author().name().unwrap_or("").to_string(),
        author_email: commit.author().email().unwrap_or("").to_string(),
        committer: commit.committer().name().unwrap_or("").to_string(),
        committer_email: commit.committer().email().unwrap_or("").to_string(),
        time: commit.time().seconds(),
        parents: commit.parent_ids().map(|id| id.to_string()).collect(),
        refs,
    }
}

pub fn commit_graph(
    path: impl AsRef<Path>,
    limit: usize,
    cursor: Option<&str>,
) -> anyhow::Result<Vec<CommitSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let mut walk = repo.revwalk()?;
    let cursor_oid = match cursor {
        Some(cursor) => {
            let oid = Oid::from_str(cursor)?;
            walk.push(oid)?;
            Some(oid)
        }
        None => {
            walk.push_head()?;
            None
        }
    };
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    let mut ref_map = commit_ref_map(&repo);
    let mut commits = Vec::new();
    for oid in walk {
        let oid = oid?;
        if Some(oid) == cursor_oid {
            // The cursor commit was already returned on the previous page;
            // the next page starts after it.
            continue;
        }
        let commit = repo.find_commit(oid)?;
        let refs = ref_map.remove(&oid).unwrap_or_default();
        commits.push(summarize_commit(&commit, refs));
        if commits.len() >= limit {
            break;
        }
    }
    Ok(commits)
}

/// Full-history commit search (unlike `commit_graph`, walks the whole history reachable
/// from HEAD rather than a windowed page — commits that exist only on an unmerged branch
/// won't be found). Case-insensitive substring match over the commit's summary,
/// author name/email, and full SHA; an empty `query` matches every commit. When
/// `file_path` is set, a commit only matches if it touched that path (diffed against
/// its first parent — a simple "commit touched this path" check, not a `--follow`
/// rename tracker).
pub fn search_commits(
    path: impl AsRef<Path>,
    query: &str,
    limit: usize,
    file_path: Option<&str>,
) -> anyhow::Result<Vec<CommitSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    let needle = query.trim().to_lowercase();
    let mut ref_map = commit_ref_map(&repo);
    let mut commits = Vec::new();
    for oid in walk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        if !needle.is_empty() {
            let sha = oid.to_string().to_lowercase();
            let summary = commit.summary().unwrap_or("").to_lowercase();
            let author_name = commit.author().name().unwrap_or("").to_lowercase();
            let author_email = commit.author().email().unwrap_or("").to_lowercase();
            let matched = sha.contains(&needle)
                || summary.contains(&needle)
                || author_name.contains(&needle)
                || author_email.contains(&needle);
            if !matched {
                continue;
            }
        }

        if let Some(file_path) = file_path {
            let tree = commit.tree()?;
            let parent_tree = if commit.parent_count() > 0 {
                Some(commit.parent(0)?.tree()?)
            } else {
                None
            };
            let mut options = DiffOptions::new();
            options.pathspec(file_path);
            let diff =
                repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))?;
            if diff.deltas().len() == 0 {
                continue;
            }
        }

        let refs = ref_map.remove(&oid).unwrap_or_default();
        commits.push(summarize_commit(&commit, refs));
        if commits.len() >= limit {
            break;
        }
    }
    Ok(commits)
}

pub fn repo_stats(path: impl AsRef<Path>, max_commits: usize) -> anyhow::Result<RepoStats> {
    let repo = Repository::open(path.as_ref())?;
    let mut walk = match repo.revwalk() {
        Ok(walk) => walk,
        Err(_) => return Ok(empty_repo_stats()),
    };
    if walk.push_head().is_err() {
        return Ok(empty_repo_stats());
    }
    walk.set_sorting(git2::Sort::TIME)?;

    let mut commit_count = 0usize;
    let mut author_totals: HashMap<String, usize> = HashMap::new();
    let mut month_totals: HashMap<(i64, u32), HashMap<String, usize>> = HashMap::new();
    let mut first_commit_time = i64::MAX;
    let mut last_commit_time = i64::MIN;

    for oid in walk.take(max_commits) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let author_name = commit.author().name().unwrap_or("").to_string();
        let time = commit.time().seconds();

        commit_count += 1;
        *author_totals.entry(author_name.clone()).or_insert(0) += 1;
        first_commit_time = first_commit_time.min(time);
        last_commit_time = last_commit_time.max(time);

        let (year, month, _day) = civil_from_unix_time(time);
        *month_totals
            .entry((year, month))
            .or_default()
            .entry(author_name)
            .or_insert(0) += 1;
    }

    if commit_count == 0 {
        return Ok(empty_repo_stats());
    }

    let mut contributors: Vec<AuthorStat> = author_totals
        .into_iter()
        .map(|(name, commits)| AuthorStat { name, commits })
        .collect();
    contributors.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.name.cmp(&b.name)));

    let mut month_keys: Vec<(i64, u32)> = month_totals.keys().copied().collect();
    month_keys.sort();
    if month_keys.len() > 24 {
        let skip = month_keys.len() - 24;
        month_keys.drain(0..skip);
    }

    let monthly = month_keys
        .into_iter()
        .map(|(year, month)| {
            let authors = month_totals.remove(&(year, month)).unwrap_or_default();
            let total = authors.values().sum();
            let mut top: Vec<AuthorStat> = authors
                .into_iter()
                .map(|(name, commits)| AuthorStat { name, commits })
                .collect();
            top.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.name.cmp(&b.name)));
            top.truncate(5);
            MonthStat {
                year,
                month,
                total,
                top,
            }
        })
        .collect();

    Ok(RepoStats {
        commit_count,
        contributors,
        monthly,
        first_commit_time,
        last_commit_time,
    })
}

fn empty_repo_stats() -> RepoStats {
    RepoStats {
        commit_count: 0,
        contributors: Vec::new(),
        monthly: Vec::new(),
        first_commit_time: 0,
        last_commit_time: 0,
    }
}

// Howard Hinnant's civil-from-days algorithm; dates rendered in UTC.
fn civil_from_unix_time(seconds: i64) -> (i64, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

pub fn create_branch(path: impl AsRef<Path>, name: &str, checkout: bool) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let head = repo.head()?.peel_to_commit()?;
    repo.branch(name, &head, false)?;
    if checkout {
        checkout_branch(path, name)?;
    }
    Ok(())
}

pub fn create_branch_at(
    path: impl AsRef<Path>,
    name: &str,
    revision: &str,
    checkout: bool,
) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let commit = repo.revparse_single(revision)?.peel_to_commit()?;
    repo.branch(name, &commit, false)?;
    if checkout {
        checkout_branch(path, name)?;
    }
    Ok(())
}

pub fn rename_branch(path: impl AsRef<Path>, old_name: &str, new_name: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let mut branch = repo.find_branch(old_name, BranchType::Local)?;
    branch.rename(new_name, false)?;
    Ok(())
}

pub fn checkout_branch(path: impl AsRef<Path>, name: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let refname = format!("refs/heads/{name}");
    repo.set_head(&refname)?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().safe()))?;
    Ok(())
}

pub fn checkout_revision(path: impl AsRef<Path>, revision: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let object = repo.revparse_single(revision)?;
    repo.checkout_tree(
        &object,
        Some(git2::build::CheckoutBuilder::default().safe()),
    )?;
    if let Ok(commit) = object.peel_to_commit() {
        repo.set_head_detached(commit.id())?;
    }
    Ok(())
}

pub fn delete_branch(path: impl AsRef<Path>, name: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let mut branch = repo.find_branch(name, BranchType::Local)?;
    branch.delete()?;
    Ok(())
}

/// Merge strategy for [`merge_branch_with_strategy`]. `FfOnly` and `Squash` mirror `git merge
/// --ff-only`/`--squash`; `NoFf` always creates a merge commit (matching `git merge --no-ff`) and
/// is the strategy [`merge_branch`] uses, preserving the pre-existing (strategy-less) behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStrategy {
    FfOnly,
    NoFf,
    Squash,
}

pub fn merge_branch(path: impl AsRef<Path>, name: &str) -> anyhow::Result<()> {
    merge_branch_with_strategy(path, name, MergeStrategy::NoFf)
}

/// Merges branch `name` into the currently checked-out branch. See [`MergeStrategy`] for the
/// per-strategy behavior; all three read the target branch tip once up front, so which strategy
/// runs never depends on more than the repo state at the moment this call starts.
pub fn merge_branch_with_strategy(
    path: impl AsRef<Path>,
    name: &str,
    strategy: MergeStrategy,
) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let oid = repo.refname_to_id(&format!("refs/heads/{name}"))?;
    let annotated = repo.find_annotated_commit(oid)?;
    match strategy {
        MergeStrategy::FfOnly => merge_ff_only(&repo, name, oid, &annotated),
        MergeStrategy::NoFf => merge_no_ff(&repo, name, oid, &annotated),
        MergeStrategy::Squash => merge_squash(&repo, &annotated),
    }
}

/// Fast-forwards the current branch's ref to `target_oid` entirely over libgit2 (same shape as
/// `pull_ff_only`): no merge commit is ever created, and diverged history is a hard error rather
/// than silently falling back to a real merge.
fn merge_ff_only(
    repo: &Repository,
    name: &str,
    target_oid: Oid,
    annotated: &git2::AnnotatedCommit,
) -> anyhow::Result<()> {
    let analysis = repo.merge_analysis(&[annotated])?;
    if analysis.0.is_up_to_date() {
        return Ok(());
    }
    if !analysis.0.is_fast_forward() {
        return Err(GitCommandError {
            command: format!("git merge --ff-only {name}"),
            stderr: "not possible to fast-forward, aborting (branches have diverged)".to_string(),
            kind: GitErrorKind::NonFastForward,
        }
        .into());
    }

    let target_commit = repo.find_commit(target_oid)?;
    // Check out the target tree before moving the ref, same reasoning as `pull_ff_only`: a
    // "safe" checkout aborts on a dirty/conflicting working tree instead of clobbering
    // uncommitted local changes.
    repo.checkout_tree(
        target_commit.as_object(),
        Some(git2::build::CheckoutBuilder::default().safe()),
    )?;

    let head_branch = current_branch(repo)?
        .ok_or_else(|| anyhow::anyhow!("cannot fast-forward merge: HEAD is not on a branch"))?;
    let refname = format!("refs/heads/{head_branch}");
    let mut reference = repo.find_reference(&refname)?;
    reference.set_target(target_oid, &format!("zync: fast-forward merge '{name}'"))?;
    repo.set_head(&refname)?;
    Ok(())
}

/// Always creates a merge commit, even when a fast-forward was possible — the original
/// (strategy-less) `merge_branch` behavior, unchanged.
fn merge_no_ff(
    repo: &Repository,
    name: &str,
    oid: Oid,
    annotated: &git2::AnnotatedCommit,
) -> anyhow::Result<()> {
    let mut options = MergeOptions::new();
    repo.merge(&[annotated], Some(&mut options), None)?;
    if repo.index()?.has_conflicts() {
        return Ok(());
    }

    let signature = repo.signature()?;
    let tree_id = {
        let mut index = repo.index()?;
        index.write_tree()?
    };
    let tree = repo.find_tree(tree_id)?;
    let head = repo.head()?.peel_to_commit()?;
    let other = repo.find_commit(oid)?;
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &format!("Merge branch '{name}'"),
        &tree,
        &[&head, &other],
    )?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().safe()))?;
    // The commit above already resolved the merge; clear MERGE_HEAD/MERGE_MSG so
    // `repo.state()` reports `Clean` again, matching real `git merge` after it commits. Only
    // reached on the conflict-free path — a conflicted merge returns early above and correctly
    // leaves MERGE_HEAD in place for the user to resolve and finish with a normal commit.
    repo.cleanup_state()?;
    Ok(())
}

/// Stages the merge result (index + working directory) without committing, matching
/// `git merge --squash`. Leaves the branch's history untouched — no merge commit, and HEAD keeps
/// a single parent whenever the caller does eventually commit.
fn merge_squash(repo: &Repository, annotated: &git2::AnnotatedCommit) -> anyhow::Result<()> {
    let mut options = MergeOptions::new();
    repo.merge(&[annotated], Some(&mut options), None)?;
    // `git merge --squash` never sets MERGE_HEAD in real git, but libgit2's `repo.merge()` always
    // does (it has no concept of squash). Left in place, a later plain `git commit` (e.g. from a
    // terminal) would pick up MERGE_HEAD and produce a 2-parent merge commit — exactly what
    // squash is supposed to avoid. `cleanup_state()` only clears MERGE_HEAD/MERGE_MSG; it never
    // touches the staged index or any conflict entries in it, so both stay intact whether or not
    // the merge produced conflicts.
    repo.cleanup_state()?;
    Ok(())
}

pub fn revert_commit(path: impl AsRef<Path>, commit_id: &str) -> anyhow::Result<String> {
    revert_commit_with_mainline(path, commit_id, None)
}

/// `mainline` is the 1-based parent number libgit2 needs to revert a merge commit (matching
/// `git revert -m`). Required when `commit_id` has 2+ parents (a merge commit) — omitting it
/// there is a clear error rather than an ambiguous libgit2 failure. Ignored for plain
/// (single-parent) commits, so passing it there is harmless.
pub fn revert_commit_with_mainline(
    path: impl AsRef<Path>,
    commit_id: &str,
    mainline: Option<u32>,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let oid = Oid::from_str(commit_id)?;
    let commit = repo.find_commit(oid)?;

    let mut options = git2::RevertOptions::new();
    if commit.parent_count() > 1 {
        let mainline = mainline.ok_or_else(|| {
            anyhow::anyhow!(
                "commit {commit_id} is a merge commit with {} parents; specify a mainline parent number (1-based) to revert it",
                commit.parent_count()
            )
        })?;
        options.mainline(mainline);
    }

    repo.revert(&commit, Some(&mut options))?;
    if repo.index()?.has_conflicts() {
        anyhow::bail!("revert stopped on conflicts");
    }
    let message = format!("Revert \"{}\"", commit.summary().unwrap_or(commit_id));
    commit_current_index(&repo, &message)
}

pub fn diff_workdir(path: impl AsRef<Path>) -> anyhow::Result<String> {
    diff_workdir_path(path, None)
}

pub fn diff_workdir_path(
    path: impl AsRef<Path>,
    file_path: Option<&str>,
) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let mut options = DiffOptions::new();
    if let Some(file_path) = file_path {
        options.pathspec(file_path);
    }
    let diff = repo.diff_index_to_workdir(None, Some(&mut options))?;
    diff_to_patch(&diff)
}

pub fn diff_staged(path: impl AsRef<Path>) -> anyhow::Result<String> {
    diff_staged_path(path, None)
}

pub fn diff_staged_path(path: impl AsRef<Path>, file_path: Option<&str>) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());
    let mut options = DiffOptions::new();
    if let Some(file_path) = file_path {
        options.pathspec(file_path);
    }
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut options))?;
    diff_to_patch(&diff)
}

pub fn diff_commit(path: impl AsRef<Path>, commit_id: &str) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let oid = git2::Oid::from_str(commit_id)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let parent_tree = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
    diff_to_patch(&diff)
}

pub fn diff_commit_to_workdir(path: impl AsRef<Path>, commit_id: &str) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let oid = git2::Oid::from_str(commit_id)?;
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    let diff = repo.diff_tree_to_workdir_with_index(Some(&tree), None)?;
    diff_to_patch(&diff)
}

pub fn blame_file(path: impl AsRef<Path>, file_path: &str) -> anyhow::Result<Vec<BlameLine>> {
    let repo = Repository::open(path.as_ref())?;
    let blame = repo.blame_file(Path::new(file_path), None)?;
    let mut lines = Vec::new();
    for hunk in blame.iter() {
        let commit_id = hunk.final_commit_id();
        let commit = repo.find_commit(commit_id).ok();
        lines.push(BlameLine {
            start_line: hunk.final_start_line(),
            line_count: hunk.lines_in_hunk(),
            commit: commit_id.to_string(),
            author: commit
                .as_ref()
                .and_then(|commit| commit.author().name().map(ToOwned::to_owned))
                .unwrap_or_default(),
            summary: commit
                .as_ref()
                .and_then(|commit| commit.summary().map(ToOwned::to_owned))
                .unwrap_or_default(),
        });
    }
    Ok(lines)
}

pub fn file_history(
    path: impl AsRef<Path>,
    file_path: &str,
    limit: usize,
) -> anyhow::Result<Vec<CommitSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
    let mut commits = Vec::new();
    for oid in walk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };
        let mut options = DiffOptions::new();
        options.pathspec(file_path);
        let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut options))?;
        if diff.deltas().len() > 0 {
            commits.push(summarize_commit(&commit, Vec::new()));
        }
        if commits.len() >= limit {
            break;
        }
    }
    Ok(commits)
}

pub fn tree_at_revision(
    path: impl AsRef<Path>,
    revision: &str,
) -> anyhow::Result<Vec<TreeEntrySummary>> {
    let repo = Repository::open(path.as_ref())?;
    let tree = repo.revparse_single(revision)?.peel_to_tree()?;
    let mut entries = Vec::new();
    tree.walk(TreeWalkMode::PreOrder, |root, entry| {
        let Some(name) = entry.name() else {
            return TreeWalkResult::Ok;
        };
        let full_path = format!("{root}{name}");
        let kind = entry
            .kind()
            .map(|kind| format!("{kind:?}").to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());
        let size = if entry.kind() == Some(git2::ObjectType::Blob) {
            repo.find_blob(entry.id()).ok().map(|blob| blob.size())
        } else {
            None
        };
        entries.push(TreeEntrySummary {
            path: full_path,
            kind,
            id: entry.id().to_string(),
            size,
        });
        TreeWalkResult::Ok
    })?;
    Ok(entries)
}

pub fn blob_at_revision(
    path: impl AsRef<Path>,
    revision: &str,
    file_path: &str,
) -> anyhow::Result<Vec<u8>> {
    let repo = Repository::open(path.as_ref())?;
    let object = repo.revparse_single(revision)?;
    let commit = object.peel_to_commit()?;
    let tree = commit.tree()?;
    let entry = tree.get_path(Path::new(file_path))?;
    let blob = repo.find_blob(entry.id())?;
    Ok(blob.content().to_vec())
}

/// Reads a file's raw bytes from the repository working tree (the "new"/uncommitted
/// side of an image diff). Guards against path traversal by requiring the resolved
/// path to stay inside the canonicalized working directory.
/// TODO(P4.1): the broader `ZYNC_REPOS_ROOT` filesystem boundary is enforced there.
pub fn read_workdir_file(path: impl AsRef<Path>, file_path: &str) -> anyhow::Result<Vec<u8>> {
    let repo = Repository::open(path.as_ref())?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("bare repository has no working directory"))?;
    let root = workdir.canonicalize()?;
    let resolved = root.join(file_path).canonicalize()?;
    if !resolved.starts_with(&root) {
        anyhow::bail!("path escapes repository working directory");
    }
    Ok(fs::read(resolved)?)
}

pub fn reflog(path: impl AsRef<Path>, limit: usize) -> anyhow::Result<Vec<ReflogEntrySummary>> {
    let repo = Repository::open(path.as_ref())?;
    let log = repo.reflog("HEAD")?;
    let mut entries = Vec::new();
    for (index, entry) in log.iter().rev().take(limit).enumerate() {
        let committer = entry.committer();
        entries.push(ReflogEntrySummary {
            index,
            old_id: entry.id_old().to_string(),
            new_id: entry.id_new().to_string(),
            message: entry.message().unwrap_or("").to_string(),
            committer: committer.name().unwrap_or("").to_string(),
            time: committer.when().seconds(),
        });
    }
    Ok(entries)
}

pub fn reset_to_revision(path: impl AsRef<Path>, revision: &str, hard: bool) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let object = repo.revparse_single(revision)?;
    repo.reset(
        &object,
        if hard {
            ResetType::Hard
        } else {
            ResetType::Mixed
        },
        None,
    )?;
    Ok(())
}

pub fn submodules(path: impl AsRef<Path>) -> anyhow::Result<Vec<SubmoduleSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let mut modules = Vec::new();
    for module in repo.submodules()? {
        modules.push(SubmoduleSummary {
            name: module.name().unwrap_or("").to_string(),
            path: module.path().to_string_lossy().to_string(),
            url: module.url().map(ToOwned::to_owned),
            head: module.head_id().map(|id| id.to_string()),
        });
    }
    Ok(modules)
}

pub fn submodule_init(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["submodule", "init"])
}

pub fn submodule_update(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["submodule", "update", "--recursive"])
}

pub fn submodule_sync(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["submodule", "sync", "--recursive"])
}

pub fn submodule_add(path: impl AsRef<Path>, url: &str, sub_path: &str) -> anyhow::Result<String> {
    // `-c protocol.file.allow=always`: git 2.38+ refuses `file://`/local-path submodule clones
    // by default (CVE-2022-39253 hardening). Zync's whole point is registering repos that live
    // on the same host filesystem, so a user-supplied local/file:// submodule URL is exactly as
    // trusted as any other repo path they've already registered — allow it explicitly rather
    // than failing every same-host submodule add.
    //
    // `--`: terminates option parsing before the caller-supplied `url`/`sub_path` positionals,
    // so a value that happens to start with `-` (e.g. a path like `-x`) can't be parsed as a
    // `git submodule add` flag.
    run_git(
        path.as_ref(),
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--",
            url,
            sub_path,
        ],
    )
}

/// Removes a submodule the way `git` itself recommends (there is no single porcelain command
/// for it): deinit its working tree, `git rm` the gitlink + `.gitmodules` entry, then best-effort
/// clean up the cached `.git/modules/<name>` clone so re-adding the same path later doesn't hit
/// a stale gitdir. Runs as two `run_git` calls plus a filesystem cleanup rather than one shellout
/// so a deinit failure (e.g. uncommitted submodule changes) surfaces before anything is removed.
pub fn submodule_remove(path: impl AsRef<Path>, sub_path: &str) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let git_dir = repo.path().to_path_buf();

    // `.git/modules/<key>` is keyed by the submodule's *name* (the `.gitmodules` section
    // name), which is usually equal to its path but can differ if the submodule was renamed.
    // Resolve it via git2 while `.gitmodules` still has the entry — `git rm` below deletes
    // that entry, so `sub_path` would be the only handle left afterward. `find_submodule`
    // accepts either the name or the path, so this also works for the common name == path
    // case; falls back to `sub_path` if the submodule can't be looked up for any reason.
    let modules_key = repo
        .find_submodule(sub_path)
        .ok()
        .and_then(|submodule| submodule.name().map(ToOwned::to_owned))
        .unwrap_or_else(|| sub_path.to_string());
    drop(repo);

    // `--`: terminates option parsing before `sub_path`, so a path starting with `-` can't be
    // parsed as a flag (mirrors `submodule_add`'s guard).
    let deinit_output = run_git(
        path.as_ref(),
        &["submodule", "deinit", "-f", "--", sub_path],
    )?;
    let rm_output = run_git(path.as_ref(), &["rm", "-f", "--", sub_path])?;

    // Best-effort cleanup of the cached `.git/modules/<key>` clone. Canonicalize both the
    // modules root and the resolved target and verify containment before deleting anything —
    // relying on `modules_key` alone (sourced from repo data, but still worth double-checking
    // before a filesystem-destructive call) would be an unnecessary trust assumption. Silently
    // skips cleanup (does not error) when there's nothing cached to remove.
    if let Ok(modules_root) = git_dir.join("modules").canonicalize() {
        let target = modules_root.join(&modules_key);
        if let Ok(canonical_target) = target.canonicalize() {
            if !canonical_target.starts_with(&modules_root) {
                anyhow::bail!("submodule cache path escapes .git/modules");
            }
            let _ = fs::remove_dir_all(canonical_target);
        }
        // else: nothing cached under `modules_key` — nothing to clean up.
    }
    // else: no `.git/modules` directory at all — nothing to clean up.

    let combined = format!("{deinit_output}\n{rm_output}");
    Ok(combined.trim().to_string())
}

pub fn lfs_summary(path: impl AsRef<Path>) -> anyhow::Result<LfsSummary> {
    let repo = Repository::open(path.as_ref())?;
    let root = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("bare repository has no working tree"))?;
    let attrs = root.join(".gitattributes");
    let content = fs::read_to_string(attrs).unwrap_or_default();
    let tracked_patterns = content
        .lines()
        .filter(|line| line.contains("filter=lfs"))
        .map(|line| line.split_whitespace().next().unwrap_or("").to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(LfsSummary {
        configured: !tracked_patterns.is_empty(),
        tracked_patterns,
    })
}

pub fn lfs_install(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["lfs", "install", "--local"])
}

pub fn lfs_track(path: impl AsRef<Path>, pattern: &str) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["lfs", "track", pattern])
}

pub fn lfs_untrack(path: impl AsRef<Path>, pattern: &str) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["lfs", "untrack", pattern])
}

pub fn lfs_pull(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["lfs", "pull"])
}

pub fn lfs_push(path: impl AsRef<Path>, remote_name: &str, branch: &str) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["lfs", "push", remote_name, branch])
}

pub fn cherry_pick(path: impl AsRef<Path>, commit_ids: &[String]) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    for commit_id in commit_ids {
        let oid = git2::Oid::from_str(commit_id)?;
        let commit = repo.find_commit(oid)?;
        repo.cherrypick(&commit, None)?;
        if repo.index()?.has_conflicts() {
            anyhow::bail!("cherry-pick stopped on conflicts");
        }
        let signature = repo.signature()?;
        let tree_id = {
            let mut index = repo.index()?;
            index.write_tree()?
        };
        let tree = repo.find_tree(tree_id)?;
        let head = repo.head()?.peel_to_commit()?;
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            commit.message().unwrap_or("Cherry pick"),
            &tree,
            &[&head],
        )?;
    }
    Ok(())
}

pub fn cherry_pick_abort(path: impl AsRef<Path>) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let has_conflicts = repo.index()?.has_conflicts();
    if repo.state() == git2::RepositoryState::Clean && !has_conflicts {
        anyhow::bail!("no cherry-pick in progress");
    }
    // cleanup_state alone leaves the conflicted index/workdir behind;
    // restore HEAD like `git cherry-pick --abort` does.
    let head = repo.head()?.peel_to_commit()?;
    repo.reset(head.as_object(), ResetType::Hard, None)?;
    repo.cleanup_state()?;
    Ok(())
}

/// Rebases the currently checked-out branch onto `upstream` (a branch name, tag, or any revision
/// `git rev-parse` can resolve) — the plain (non-interactive) counterpart to
/// [`interactive_rebase`]. Implemented as a `git rebase <upstream>` shellout via [`run_git`]
/// (Fork-parity pragmatism: replaying an entire branch's worth of commits one-by-one over
/// libgit2, the way `interactive_rebase` does for an explicit step list, would just reimplement
/// what the real `git rebase` already does correctly — including its own conflict bookkeeping —
/// worse). `run_git` already hardens the shellout (`GIT_TERMINAL_PROMPT=0`, batch-mode SSH, a
/// timeout) and classifies failures into a [`GitCommandError`] via [`classify_git_stderr`].
///
/// On conflict, `git rebase` stops mid-rebase with the conflicted state left in the working
/// tree/index — exactly like a real terminal `git rebase` would. This function does NOT
/// auto-abort: the conflict is returned as an error (classified `GitErrorKind::Conflict`, since
/// `classify_git_stderr` matches "conflict" in git's stderr) so the caller can surface it and the
/// repo is left mid-rebase for the caller to resolve and [`rebase_continue`], or bail out via
/// [`rebase_abort`] — the same recovery path already used for [`interactive_rebase`] conflicts.
pub fn rebase_branch(path: impl AsRef<Path>, upstream: &str) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    ensure_clean_for_history_rewrite(&repo)?;

    // `upstream` comes straight from an HTTP request body (effectively unauthenticated — auth is
    // a stub) with no shape restriction. Without a guard, a value like `--exec=<cmd>` or
    // `--onto=...`/`--root` would be parsed by `git rebase` as an option rather than a revision —
    // `--exec` runs arbitrary commands via the merge backend (default since git 2.26), and
    // `--onto`/`--root` enable other destructive rewrites. Defense in depth, same as
    // `submodule_add`'s `--`:
    //   1. Resolve `upstream` with libgit2 first, entirely independent of the `git` CLI's own
    //      option parser — a string like `--exec=...` does not `revparse` to a real object, so
    //      this fails cleanly before the shellout is ever reached.
    //   2. Pass `--` before `upstream` in the shellout too, so even a value that *does* revparse
    //      (e.g. a ref literally named `--foo` is technically legal in git) can never be
    //      misparsed as an option by `git rebase` itself.
    repo.revparse_single(upstream)
        .map_err(|_| anyhow::anyhow!("'{upstream}' is not a valid revision"))?;
    run_git(path.as_ref(), &["rebase", "--", upstream])
}

pub fn interactive_rebase(
    path: impl AsRef<Path>,
    base: &str,
    steps: &[RebaseStep],
) -> anyhow::Result<RebaseResult> {
    let repo = Repository::open(path.as_ref())?;
    ensure_clean_for_history_rewrite(&repo)?;

    let base_oid = Oid::from_str(base)?;
    let base_object = repo.find_object(base_oid, None)?;
    repo.reset(&base_object, ResetType::Hard, None)?;

    let mut result = RebaseResult {
        head: Some(base_oid.to_string()),
        stopped_at: None,
        applied: Vec::new(),
        dropped: Vec::new(),
    };

    for step in steps {
        match step.action {
            RebaseAction::Drop => {
                result.dropped.push(step.commit.clone());
            }
            RebaseAction::Pick => {
                replay_commit(&repo, &step.commit, ReplayMode::Pick(step.message.clone()))?;
                result.head = head_oid(&repo);
                result.applied.push(step.commit.clone());
            }
            RebaseAction::Squash => {
                // Squash amends whatever HEAD currently is. If nothing has
                // been picked yet this run (e.g. every earlier step was a
                // Drop), HEAD is still sitting at `base` — amending it would
                // silently fold this commit into a commit outside the
                // requested range instead of failing like real git does
                // ("cannot squash without a previous commit").
                if result.applied.is_empty() {
                    anyhow::bail!(
                        "cannot squash {}: no preceding commit in this rebase to combine it into",
                        step.commit
                    );
                }
                replay_commit(&repo, &step.commit, ReplayMode::Squash)?;
                result.head = head_oid(&repo);
                result.applied.push(step.commit.clone());
            }
            RebaseAction::Fixup => {
                if result.applied.is_empty() {
                    anyhow::bail!(
                        "cannot fixup {}: no preceding commit in this rebase to combine it into",
                        step.commit
                    );
                }
                replay_commit(&repo, &step.commit, ReplayMode::Fixup)?;
                result.head = head_oid(&repo);
                result.applied.push(step.commit.clone());
            }
            RebaseAction::Edit => {
                apply_commit_without_committing(&repo, &step.commit)?;
                repo.cleanup_state()?;
                result.stopped_at = Some(step.commit.clone());
                result.head = head_oid(&repo);
                break;
            }
        }
    }

    Ok(result)
}

pub fn rebase_continue(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["rebase", "--continue"])
}

pub fn rebase_abort(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["rebase", "--abort"])
}

pub fn rebase_skip(path: impl AsRef<Path>) -> anyhow::Result<String> {
    run_git(path.as_ref(), &["rebase", "--skip"])
}

pub fn conflicts(path: impl AsRef<Path>) -> anyhow::Result<Vec<ConflictSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let index = repo.index()?;
    let mut conflicts = Vec::new();
    if !index.has_conflicts() {
        return Ok(conflicts);
    }

    for conflict in index.conflicts()? {
        let conflict = conflict?;
        conflicts.push(ConflictSummary {
            ancestor: conflict
                .ancestor
                .and_then(|entry| String::from_utf8(entry.path).ok()),
            ours: conflict
                .our
                .and_then(|entry| String::from_utf8(entry.path).ok()),
            theirs: conflict
                .their
                .and_then(|entry| String::from_utf8(entry.path).ok()),
        });
    }
    Ok(conflicts)
}

pub fn conflict_detail(path: impl AsRef<Path>, file: &str) -> anyhow::Result<ConflictDetail> {
    let repo = Repository::open(path.as_ref())?;
    let index = repo.index()?;
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        let ancestor_path = conflict
            .ancestor
            .as_ref()
            .and_then(|entry| String::from_utf8(entry.path.clone()).ok());
        let ours_path = conflict
            .our
            .as_ref()
            .and_then(|entry| String::from_utf8(entry.path.clone()).ok());
        let theirs_path = conflict
            .their
            .as_ref()
            .and_then(|entry| String::from_utf8(entry.path.clone()).ok());
        let matches = [&ancestor_path, &ours_path, &theirs_path]
            .into_iter()
            .flatten()
            .any(|path| path == file);
        if !matches {
            continue;
        }

        return Ok(ConflictDetail {
            path: file.to_string(),
            ancestor_content: conflict_blob(&repo, conflict.ancestor.as_ref())?,
            ours_content: conflict_blob(&repo, conflict.our.as_ref())?,
            theirs_content: conflict_blob(&repo, conflict.their.as_ref())?,
            ancestor_path,
            ours_path,
            theirs_path,
        });
    }

    anyhow::bail!("conflict not found for {file}")
}

pub fn resolve_conflict_with_checkout(
    path: impl AsRef<Path>,
    file: &str,
    side: ConflictSide,
) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.path(file).force();
    match side {
        ConflictSide::Local => {
            checkout.use_ours(true);
        }
        ConflictSide::Remote => {
            checkout.use_theirs(true);
        }
    }
    repo.checkout_index(None, Some(&mut checkout))?;
    add(path, &[file.to_string()])?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConflictSide {
    Local,
    Remote,
}

pub fn stash_list(path: impl AsRef<Path>) -> anyhow::Result<Vec<StashSummary>> {
    let mut repo = Repository::open(path.as_ref())?;
    let mut stashes = Vec::new();
    repo.stash_foreach(|index, name, oid| {
        stashes.push(StashSummary {
            index,
            name: name.to_string(),
            message: oid.to_string(),
        });
        true
    })?;
    Ok(stashes)
}

pub fn create_stash(
    path: impl AsRef<Path>,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> anyhow::Result<()> {
    let mut repo = Repository::open(path.as_ref())?;
    let signature = Signature::now(author_name, author_email)?;
    repo.stash_save(
        &signature,
        message,
        Some(git2::StashFlags::INCLUDE_UNTRACKED),
    )?;
    Ok(())
}

pub fn apply_stash(path: impl AsRef<Path>, index: usize, pop: bool) -> anyhow::Result<()> {
    let mut repo = Repository::open(path.as_ref())?;
    if pop {
        repo.stash_pop(index, None)?;
    } else {
        repo.stash_apply(index, None)?;
    }
    Ok(())
}

pub fn drop_stash(path: impl AsRef<Path>, index: usize) -> anyhow::Result<()> {
    let mut repo = Repository::open(path.as_ref())?;
    repo.stash_drop(index)?;
    Ok(())
}

fn repo_info(repo: &Repository) -> anyhow::Result<RepoInfo> {
    let path = repo
        .workdir()
        .or_else(|| repo.path().parent())
        .unwrap_or_else(|| repo.path())
        .to_path_buf();
    Ok(RepoInfo {
        path,
        head: repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|oid| oid.to_string()),
        current_branch: current_branch(repo)?,
        is_bare: repo.is_bare(),
    })
}

fn diff_to_patch(diff: &git2::Diff<'_>) -> anyhow::Result<String> {
    let mut output = Vec::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        // line.content() excludes the origin marker; without it the output is
        // not a valid unified diff (git2::Diff::from_buffer rejects it and the
        // UI cannot tell additions from removals).
        match line.origin() {
            '+' | '-' | ' ' => output.push(line.origin() as u8),
            _ => {}
        }
        output.extend_from_slice(line.content());
        true
    })?;
    Ok(String::from_utf8_lossy(&output).to_string())
}

fn conflict_blob(repo: &Repository, entry: Option<&git2::IndexEntry>) -> anyhow::Result<String> {
    let Some(entry) = entry else {
        return Ok(String::new());
    };
    if entry.id == Oid::zero() {
        return Ok(String::new());
    }
    let blob = repo.find_blob(entry.id)?;
    Ok(String::from_utf8_lossy(blob.content()).to_string())
}

enum ReplayMode {
    Pick(Option<String>),
    Squash,
    Fixup,
}

/// Shared dirty-tree guard for anything that rewrites history in place (interactive rebase, plain
/// branch-onto-branch rebase): a dirty working tree can't safely be `reset --hard`/replayed over.
///
/// Returns a `GitCommandError` (kind `Precondition`) rather than a plain `anyhow` error so HTTP
/// callers that downcast via `map_git_error` (like `rebase_branch`'s server handler) can map this
/// to 409 instead of falling back to 500 — while callers that still use a blanket
/// `.map_err(internal_error)` (like `interactive_rebase`'s handler) are unaffected: the message
/// text is unchanged and `internal_error` never looks at `kind`.
fn ensure_clean_for_history_rewrite(repo: &Repository) -> anyhow::Result<()> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    if repo.statuses(Some(&mut options))?.is_empty() {
        Ok(())
    } else {
        Err(GitCommandError {
            command: "git rebase".to_string(),
            stderr: "working tree must be clean before rebasing".to_string(),
            kind: GitErrorKind::Precondition,
        }
        .into())
    }
}

fn replay_commit(repo: &Repository, commit_id: &str, mode: ReplayMode) -> anyhow::Result<()> {
    let oid = Oid::from_str(commit_id)?;
    let commit = repo.find_commit(oid)?;
    repo.cherrypick(&commit, None)?;
    if repo.index()?.has_conflicts() {
        anyhow::bail!("rebase stopped on conflicts at {commit_id}");
    }

    match mode {
        ReplayMode::Pick(message_override) => {
            let message = message_override
                .as_deref()
                .unwrap_or_else(|| commit.message().unwrap_or("Rebased commit"));
            commit_current_index(repo, message)?;
        }
        ReplayMode::Squash => {
            let head = repo.head()?.peel_to_commit()?;
            let previous_message = head.message().unwrap_or("");
            let message = format!(
                "{}\n\n{}",
                previous_message.trim_end(),
                commit.message().unwrap_or("").trim()
            );
            amend_head(repo, &message)?;
        }
        ReplayMode::Fixup => {
            let head = repo.head()?.peel_to_commit()?;
            let message = head.message().unwrap_or("Fixup").to_string();
            amend_head(repo, &message)?;
        }
    }
    repo.cleanup_state()?;
    Ok(())
}

fn apply_commit_without_committing(repo: &Repository, commit_id: &str) -> anyhow::Result<()> {
    let oid = Oid::from_str(commit_id)?;
    let commit = repo.find_commit(oid)?;
    repo.cherrypick(&commit, None)?;
    if repo.index()?.has_conflicts() {
        anyhow::bail!("rebase edit stopped on conflicts at {commit_id}");
    }
    Ok(())
}

fn commit_current_index(repo: &Repository, message: &str) -> anyhow::Result<String> {
    let signature = repo_signature(repo)?;
    let tree_id = {
        let mut index = repo.index()?;
        index.write_tree()?
    };
    let tree = repo.find_tree(tree_id)?;
    let head = repo.head()?.peel_to_commit()?;
    let oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&head],
    )?;
    Ok(oid.to_string())
}

fn amend_head(repo: &Repository, message: &str) -> anyhow::Result<String> {
    let signature = repo_signature(repo)?;
    let head = repo.head()?.peel_to_commit()?;
    let tree_id = {
        let mut index = repo.index()?;
        index.write_tree()?
    };
    let tree = repo.find_tree(tree_id)?;
    let oid = head.amend(
        Some("HEAD"),
        Some(&signature),
        Some(&signature),
        None,
        Some(message),
        Some(&tree),
    )?;
    Ok(oid.to_string())
}

fn repo_signature(repo: &Repository) -> anyhow::Result<Signature<'_>> {
    repo.signature()
        .or_else(|_| Signature::now("Zync", "zync@local"))
        .map_err(Into::into)
}

fn head_oid(repo: &Repository) -> Option<String> {
    repo.head()
        .ok()
        .and_then(|head| head.target())
        .map(|oid| oid.to_string())
}

fn current_branch(repo: &Repository) -> anyhow::Result<Option<String>> {
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(None),
    };
    Ok(if head.is_branch() {
        head.shorthand().map(ToOwned::to_owned)
    } else {
        None
    })
}

fn upstream_branch(repo: &Repository) -> anyhow::Result<Option<String>> {
    let Some(branch_name) = current_branch(repo)? else {
        return Ok(None);
    };
    let branch = repo.find_branch(&branch_name, BranchType::Local)?;
    let upstream = match branch.upstream() {
        Ok(upstream) => upstream,
        Err(_) => return Ok(None),
    };
    Ok(upstream
        .name()?
        .and_then(|name| name.rsplit('/').next())
        .map(ToOwned::to_owned))
}

/// Default ceiling for a single `git` CLI shellout. Remote operations (fetch/pull/push/lfs)
/// are the main reason this exists: with `GIT_TERMINAL_PROMPT=0` and batch-mode SSH a bad
/// credential or unreachable host fails in well under a second, but a wedged network peer can
/// otherwise leave the child process running (and the server request hanging) forever.
const DEFAULT_GIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Coarse classification of a failed `git` invocation, derived from stderr text. Lets callers
/// (eventually the server) react differently per failure mode instead of pattern-matching a raw
/// error string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitErrorKind {
    /// Missing/invalid credentials (HTTPS auth failure, SSH key rejected, prompts disabled).
    Auth,
    /// Host unreachable, DNS failure, connection refused/reset/timed out.
    Network,
    /// Push rejected because the remote has commits we don't have locally.
    NonFastForward,
    /// Merge/rebase/checkout produced a conflict.
    Conflict,
    /// A precondition the caller could have avoided (e.g. a dirty working tree before a
    /// history-rewriting operation) rather than a git/transport failure.
    Precondition,
    /// The child process did not exit within the allotted timeout and was killed.
    Timeout,
    /// Anything else; `GitCommandError::stderr` carries the raw detail.
    Other,
}

/// Error returned by a failed `run_git` shellout. Implements `std::error::Error` (via
/// `thiserror`) so it can be attached as an `anyhow` error and later recovered with
/// `error.downcast_ref::<GitCommandError>()` to map `kind` to an HTTP response.
#[derive(Debug, thiserror::Error)]
#[error("{command} failed: {stderr}")]
pub struct GitCommandError {
    /// The command that was run, e.g. `"git fetch --prune origin"`.
    pub command: String,
    /// stderr (or, if stderr was empty, stdout) captured from the child process.
    pub stderr: String,
    pub kind: GitErrorKind,
}

/// Classify a `git` stderr (or combined) string into a [`GitErrorKind`]. Pure and side-effect
/// free so it can be unit tested directly against captured stderr fixtures.
pub fn classify_git_stderr(stderr: &str) -> GitErrorKind {
    let lower = stderr.to_lowercase();

    let is_auth = lower.contains("authentication failed")
        || lower.contains("could not read username")
        || lower.contains("could not read password")
        || lower.contains("permission denied (publickey)")
        || lower.contains("invalid credentials")
        || lower.contains("terminal prompts disabled")
        || lower.contains("access denied")
        || lower.contains("403")
        || lower.contains("support for password authentication was removed")
        || lower.contains("invalid username or password");
    if is_auth {
        return GitErrorKind::Auth;
    }

    let is_network = lower.contains("could not resolve host")
        || lower.contains("connection timed out")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("network is unreachable")
        || lower.contains("could not connect")
        || lower.contains("failed to connect")
        || lower.contains("no route to host")
        || lower.contains("ssl certificate problem");
    if is_network {
        return GitErrorKind::Network;
    }

    let is_non_fast_forward = lower.contains("non-fast-forward")
        || lower.contains("fetch first")
        || lower.contains("tip of your current branch is behind");
    if is_non_fast_forward {
        return GitErrorKind::NonFastForward;
    }

    if lower.contains("conflict") {
        return GitErrorKind::Conflict;
    }

    GitErrorKind::Other
}

fn run_git(repo_path: &Path, args: &[&str]) -> anyhow::Result<String> {
    run_git_with_timeout(repo_path, args, DEFAULT_GIT_TIMEOUT, &[])
}

/// `run_git`, but with additional environment variables set on the child process. Used to
/// inject credentials into the CLI-only pull merge/rebase path (per ADR-001) without ever
/// putting a secret in argv, where it would end up embedded in `GitCommandError::command`. The
/// public `run_git`-equivalent surface (`fetch`/`push`/etc.) is unaffected; this is only reached
/// from `pull_via_cli`.
fn run_git_with_env(
    repo_path: &Path,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> anyhow::Result<String> {
    run_git_with_timeout(repo_path, args, DEFAULT_GIT_TIMEOUT, extra_env)
}

fn run_git_with_timeout(
    repo_path: &Path,
    args: &[&str],
    timeout: Duration,
    extra_env: &[(&str, &str)],
) -> anyhow::Result<String> {
    let command_str = format!("git {}", args.join(" "));

    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(repo_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Non-interactive hardening: never let git fall back to a terminal credential prompt (it
    // would otherwise block the server process indefinitely), and put SSH in batch mode unless
    // the caller's environment already set a custom GIT_SSH_COMMAND we shouldn't clobber.
    command.env("GIT_TERMINAL_PROMPT", "0");
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        command.env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes");
    }
    // Applied last so a credentialed caller's GIT_SSH_COMMAND/GIT_ASKPASS always wins over the
    // defaults above.
    for (key, value) in extra_env {
        command.env(key, value);
    }

    let mut child = command.spawn()?;
    let mut stdout_pipe = child.stdout.take().expect("stdout is piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr is piped");

    // Drain stdout/stderr on background threads while we poll for exit, so a chatty command
    // can't deadlock by filling the OS pipe buffer before we get around to reading it.
    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        buf
    });
    let stderr_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        buf
    });

    let start = Instant::now();
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Do NOT join the drain threads here: if git spawned a grandchild (e.g.
                    // ssh) that inherited the pipe's write end, killing git alone won't make
                    // that grandchild exit, so the pipe never EOFs and read_to_end would block
                    // forever. Drop the handles instead — the threads detach and die on their
                    // own once the grandchild eventually exits and the pipe closes.
                    drop(stdout_handle);
                    drop(stderr_handle);
                    return Err(GitCommandError {
                        command: command_str,
                        stderr: format!("timed out after {}s", timeout.as_secs()),
                        kind: GitErrorKind::Timeout,
                    }
                    .into());
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
    };

    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_handle.join().unwrap_or_default();
    let stdout = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    if status.success() {
        if stdout.is_empty() {
            Ok(stderr)
        } else {
            Ok(stdout)
        }
    } else {
        let detail = if stderr.is_empty() { stdout } else { stderr };
        let kind = classify_git_stderr(&detail);
        Err(GitCommandError {
            command: command_str,
            stderr: detail,
            kind,
        }
        .into())
    }
}

/// Ceiling on how many times the credentials callback below will be invoked for a single
/// network operation. libgit2 retries the callback (sometimes with a narrower `allowed_types`
/// bitmask) when a returned credential is rejected; without a cap, a persistently-wrong
/// credential — or a server that never accepts any type we offer — could make the callback spin
/// indefinitely instead of surfacing a clean auth error.
const MAX_CREDENTIAL_ATTEMPTS: u32 = 5;

/// Builds the `RemoteCallbacks` credential closure for a given [`CredentialSpec`]. Every
/// libgit2 network op (fetch/push/force-with-lease/clone/delete-remote-branch) goes through
/// this so credential handling stays in one place.
fn callbacks<'a>(spec: &'a CredentialSpec) -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    let attempts = Cell::new(0u32);
    callbacks.credentials(move |_url, username_from_url, allowed_types| {
        let attempt = attempts.get();
        attempts.set(attempt + 1);
        if attempt >= MAX_CREDENTIAL_ATTEMPTS {
            return Err(git2::Error::from_str(
                "authentication failed: exceeded maximum credential attempts",
            ));
        }

        // SSH's two-step negotiation asks for a bare username before it asks for the actual key
        // (this is what `allowed_types == USERNAME` means, distinct from `SSH_KEY`). Without
        // this branch, `ssh://host/path` URLs that carry no inline user fail outright for
        // `SshKey`/`SshAgent` even with otherwise-correct credentials.
        if allowed_types.contains(CredentialType::USERNAME) {
            let user = match spec {
                CredentialSpec::UserpassPlaintext { username, .. } => username.as_str(),
                CredentialSpec::SshKey { username, .. } => username.as_str(),
                CredentialSpec::SshAgent { username } => {
                    username.as_deref().or(username_from_url).unwrap_or("git")
                }
                CredentialSpec::Default => username_from_url.unwrap_or("git"),
            };
            return Cred::username(user);
        }

        match spec {
            CredentialSpec::UserpassPlaintext { username, secret } => {
                if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
                    Cred::userpass_plaintext(username, secret)
                } else {
                    Err(git2::Error::from_str(
                        "authentication failed: remote does not accept username/password credentials",
                    ))
                }
            }
            CredentialSpec::SshKey {
                username,
                private_key,
                passphrase,
            } => {
                if allowed_types.contains(CredentialType::SSH_KEY) {
                    let passphrase = passphrase.as_ref().map(|value| value.as_str());
                    Cred::ssh_key_from_memory(username, None, private_key, passphrase)
                } else {
                    Err(git2::Error::from_str(
                        "authentication failed: remote does not accept SSH key credentials",
                    ))
                }
            }
            CredentialSpec::SshAgent { username } => {
                let user = username.as_deref().or(username_from_url).unwrap_or("git");
                if attempt == 0 && allowed_types.contains(CredentialType::SSH_KEY) {
                    Cred::ssh_key_from_agent(user)
                } else {
                    Cred::default()
                }
            }
            CredentialSpec::Default => {
                // Preserves the pre-credentials behavior exactly on the first attempt: try the
                // ssh-agent when the URL carries a username, otherwise Cred::default(). Later
                // attempts fall back to Cred::default() so a failing agent can't loop forever.
                if attempt == 0 {
                    if let Some(username) = username_from_url {
                        return Cred::ssh_key_from_agent(username);
                    }
                }
                Cred::default()
            }
        }
    });
    callbacks
}

/// Best-effort host extraction from a remote URL, used only to build user-facing error text
/// (never logged or returned with credentials). Handles `https://`/`http://`, `ssh://`, and the
/// scp-like `user@host:path` form; falls back to the raw URL if none match.
fn remote_host(url: &str) -> String {
    fn strip_userinfo(rest: &str) -> &str {
        rest.rsplit('@').next().unwrap_or(rest)
    }

    if let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    {
        let rest = strip_userinfo(rest);
        rest.split('/').next().unwrap_or(rest).to_string()
    } else if let Some(rest) = url.strip_prefix("ssh://") {
        let rest = strip_userinfo(rest);
        rest.split('/').next().unwrap_or(rest).to_string()
    } else if let Some(idx) = url.find('@') {
        // scp-like `git@host:path`.
        let rest = &url[idx + 1..];
        rest.split(':').next().unwrap_or(rest).to_string()
    } else {
        url.split('/').next().unwrap_or(url).to_string()
    }
}

/// Maps a `git2::Error` from a network operation into the same [`GitCommandError`] used by the
/// `run_git` CLI path, so callers can `downcast_ref::<GitCommandError>()` uniformly regardless
/// of which transport handled the operation. Per ADR-001, an auth failure never carries the
/// underlying libgit2 message (which can echo back URL/username detail) — it gets a fixed,
/// secret-free message instead. Every other kind is classified the same way CLI stderr is, and
/// is still run through [`redact_url_userinfo`] before being kept: libgit2's own error text
/// normally doesn't embed credentials, but a non-auth failure class (DNS/TLS) can echo the raw
/// request URL — including inline userinfo — back in its message (P0.11 security review, W2).
fn map_git2_error(command: &str, host: &str, err: git2::Error) -> anyhow::Error {
    let raw = err.message();
    let kind = if err.code() == git2::ErrorCode::Auth {
        GitErrorKind::Auth
    } else {
        classify_git_stderr(raw)
    };
    let stderr = if kind == GitErrorKind::Auth {
        format!("authentication failed for {host}")
    } else {
        redact_url_userinfo(raw)
    };
    GitCommandError {
        command: command.to_string(),
        stderr,
        kind,
    }
    .into()
}

/// Pushes `refspecs` and turns a server-side rejection into an `Err`. `Remote::push` alone
/// returns `Ok(())` even when the remote rejects a ref update (a well-known libgit2 gotcha), so
/// this always installs a `push_update_reference` callback and checks it after the call.
fn push_refspecs(
    remote: &mut Remote<'_>,
    refspecs: &[String],
    spec: &CredentialSpec,
    host: &str,
    command: &str,
) -> anyhow::Result<()> {
    let rejected: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let rejected_writer = Rc::clone(&rejected);

    let mut push_callbacks = callbacks(spec);
    push_callbacks.push_update_reference(move |refname, status| {
        if let Some(message) = status {
            *rejected_writer.borrow_mut() = Some(format!("{refname}: {message}"));
        }
        Ok(())
    });

    let mut options = PushOptions::new();
    options.remote_callbacks(push_callbacks);

    remote
        .push(refspecs, Some(&mut options))
        .map_err(|err| map_git2_error(command, host, err))?;

    if let Some(message) = rejected.borrow_mut().take() {
        return Err(GitCommandError {
            command: command.to_string(),
            kind: classify_git_stderr(&message),
            stderr: message,
        }
        .into());
    }
    Ok(())
}

/// A temp file created with owner-only permissions that deletes itself on drop — including
/// during panic unwinding — so a credentialed pull's key/token temp file is never orphaned on
/// disk.
struct TempSecretFile(PathBuf);

impl TempSecretFile {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempSecretFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Writes a one-shot `GIT_ASKPASS` shim: a tiny shell script that prints `ZYNC_ASKPASS_TOKEN`
/// (set on the same child process, never argv) whenever git prompts for a credential. Created
/// atomically with its final (0700, executable) permission bits — see `create_owner_only_file`.
fn write_askpass_shim() -> anyhow::Result<TempSecretFile> {
    let path = std::env::temp_dir().join(format!("zync-askpass-{}.sh", temp_file_suffix()));
    create_owner_only_file(
        &path,
        b"#!/bin/sh\nprintf '%s' \"$ZYNC_ASKPASS_TOKEN\"\n",
        0o700,
    )?;
    Ok(TempSecretFile(path))
}

/// Writes `contents` to a fresh temp file with owner-only permissions (`mode`), for the
/// CLI-only SSH pull path: the private key touches disk only for the duration of that one `git`
/// invocation and the returned guard removes it as soon as it's dropped.
fn write_temp_secret_file(
    prefix: &str,
    contents: &str,
    mode: u32,
) -> anyhow::Result<TempSecretFile> {
    let path = std::env::temp_dir().join(format!("{prefix}-{}", temp_file_suffix()));
    create_owner_only_file(&path, contents.as_bytes(), mode)?;
    Ok(TempSecretFile(path))
}

/// Creates `path` atomically with `mode` already in effect at creation time — no `write` then
/// `chmod` window during which the file briefly exists with the process umask's permissive
/// default (e.g. world-readable) — and refuses to write through a pre-existing path, including
/// a pre-planted symlink: `create_new` maps to `O_CREAT | O_EXCL`, which fails rather than
/// following a symlink already at that path.
#[cfg(unix)]
fn create_owner_only_file(path: &Path, contents: &[u8], mode: u32) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    file.write_all(contents)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_owner_only_file(path: &Path, contents: &[u8], _mode: u32) -> anyhow::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(contents)?;
    Ok(())
}

/// A unique-enough suffix for temp file names: process id + monotonic-ish nanosecond timestamp
/// + a per-process counter, avoiding a dependency on `rand`/`uuid` in this crate.
fn temp_file_suffix() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}-{count}", std::process::id())
}

#[cfg(test)]
mod url_redaction_tests {
    use super::*;

    #[test]
    fn redact_url_userinfo_strips_https_token() {
        assert_eq!(
            redact_url_userinfo("https://x-access-token:SUPERSECRET@github.com/org/repo.git"),
            "https://github.com/org/repo.git"
        );
    }

    #[test]
    fn redact_url_userinfo_strips_bare_username() {
        assert_eq!(
            redact_url_userinfo("https://oauth2@github.com/org/repo.git"),
            "https://github.com/org/repo.git"
        );
    }

    #[test]
    fn redact_url_userinfo_leaves_url_without_userinfo_unchanged() {
        assert_eq!(
            redact_url_userinfo("https://github.com/org/repo.git"),
            "https://github.com/org/repo.git"
        );
    }

    #[test]
    fn redact_url_userinfo_leaves_scp_like_url_unchanged() {
        // The `user` in `git@host:path` is an ssh login name, never a secret — nothing to strip.
        assert_eq!(
            redact_url_userinfo("git@github.com:org/repo.git"),
            "git@github.com:org/repo.git"
        );
    }

    // P0.11 security review, W2: a non-auth libgit2 error (DNS/TLS failure) can echo the raw
    // request URL — inline userinfo included — somewhere inside a longer sentence, not as the
    // entire message. `redact_url_userinfo` must scrub it wherever it appears.

    #[test]
    fn redact_url_userinfo_strips_url_embedded_in_a_longer_message() {
        assert_eq!(
            redact_url_userinfo(
                "failed to resolve address for https://x-access-token:SUPERSECRET@github.com/org/repo.git: nodename nor servname provided, or not known"
            ),
            "failed to resolve address for https://github.com/org/repo.git: nodename nor servname provided, or not known"
        );
    }

    #[test]
    fn redact_url_userinfo_strips_ssh_scheme_url_embedded_in_a_longer_message() {
        assert_eq!(
            redact_url_userinfo(
                "unable to connect to ssh://git:SUPERSECRET@example.com:22/org/repo.git: connection refused"
            ),
            "unable to connect to ssh://example.com:22/org/repo.git: connection refused"
        );
    }

    #[test]
    fn redact_url_userinfo_strips_multiple_urls_in_one_message() {
        assert_eq!(
            redact_url_userinfo(
                "redirected from https://a:SECRET1@github.com/x.git to https://b:SECRET2@github.com/y.git"
            ),
            "redirected from https://github.com/x.git to https://github.com/y.git"
        );
    }

    #[test]
    fn redact_url_userinfo_leaves_message_without_a_url_unchanged() {
        assert_eq!(
            redact_url_userinfo("connection refused"),
            "connection refused"
        );
    }
}

#[cfg(all(test, unix))]
mod credential_temp_file_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn create_owner_only_file_sets_mode_atomically_and_rejects_existing_path() {
        let path = std::env::temp_dir().join(format!("zync-test-{}", temp_file_suffix()));

        create_owner_only_file(&path, b"sekrit", 0o600).expect("creates a fresh file");
        let metadata = fs::metadata(&path).expect("stat temp file");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_to_string(&path).expect("read back"), "sekrit");

        // A second create at the same path must fail (create_new = O_CREAT | O_EXCL) rather
        // than silently overwriting or following a pre-existing path/symlink.
        let result = create_owner_only_file(&path, b"other", 0o600);
        assert!(result.is_err(), "must refuse a pre-existing path");
        assert_eq!(
            fs::read_to_string(&path).expect("original untouched"),
            "sekrit",
            "a rejected create must not modify the pre-existing file"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn write_temp_secret_file_produces_owner_only_mode_0600_and_self_deletes() {
        let path = {
            let guard = write_temp_secret_file("zync-test-secret", "sekrit", 0o600)
                .expect("creates a fresh temp file");
            let metadata = fs::metadata(guard.path()).expect("stat temp file");
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
            guard.path().to_path_buf()
        };
        // `guard` (a TempSecretFile) drops at the end of the block above; its Drop impl must
        // have removed the file, including on the panic-unwind path (N4).
        assert!(!path.exists(), "TempSecretFile must delete itself on drop");
    }

    #[test]
    fn write_askpass_shim_is_owner_only_and_executable() {
        let guard = write_askpass_shim().expect("creates the askpass shim");
        let metadata = fs::metadata(guard.path()).expect("stat shim");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
    }
}
