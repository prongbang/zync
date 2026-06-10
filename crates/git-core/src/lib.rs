use git2::{
    ApplyLocation, BranchType, Cred, DiffFormat, FetchOptions, IndexAddOption, MergeOptions, Oid,
    PushOptions, RemoteCallbacks, Repository, ResetType, Signature, StatusOptions,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
pub struct CommitSummary {
    pub id: String,
    pub summary: String,
    pub author: String,
    pub time: i64,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSummary {
    pub name: String,
    pub is_head: bool,
    pub kind: String,
    pub target: Option<String>,
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
pub struct RebaseStep {
    pub commit: String,
    pub action: RebaseAction,
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

pub fn clone_repo(url: &str, destination: impl AsRef<Path>) -> anyhow::Result<RepoInfo> {
    let repo = Repository::clone(url, destination.as_ref())?;
    repo_info(&repo)
}

pub fn fetch(path: impl AsRef<Path>, remote_name: Option<&str>) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let remote_name = remote_name.unwrap_or("origin");
    let mut remote = repo.find_remote(remote_name)?;
    let mut options = FetchOptions::new();
    options.remote_callbacks(callbacks());
    remote.fetch(&[] as &[&str], Some(&mut options), None)?;
    Ok(())
}

pub fn pull(
    path: impl AsRef<Path>,
    remote_name: Option<&str>,
    branch: Option<&str>,
) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let remote_name = remote_name.unwrap_or("origin");
    let branch_name = branch
        .map(ToOwned::to_owned)
        .or_else(|| current_branch(&repo).ok().flatten())
        .ok_or_else(|| anyhow::anyhow!("cannot pull without a current branch"))?;

    fetch(path, Some(remote_name))?;

    let fetch_head = format!("refs/remotes/{remote_name}/{branch_name}");
    let oid = repo.refname_to_id(&fetch_head)?;
    let annotated = repo.find_annotated_commit(oid)?;
    let (analysis, _) = repo.merge_analysis(&[&annotated])?;

    if analysis.is_up_to_date() {
        return Ok(());
    }

    if !analysis.is_fast_forward() {
        anyhow::bail!("non-fast-forward pull is not implemented yet");
    }

    let local_ref = format!("refs/heads/{branch_name}");
    let mut reference = repo.find_reference(&local_ref)?;
    reference.set_target(oid, "Fast-forward pull")?;
    repo.set_head(&local_ref)?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
    Ok(())
}

pub fn push(
    path: impl AsRef<Path>,
    remote_name: Option<&str>,
    branch: Option<&str>,
) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let remote_name = remote_name.unwrap_or("origin");
    let branch_name = branch
        .map(ToOwned::to_owned)
        .or_else(|| current_branch(&repo).ok().flatten())
        .ok_or_else(|| anyhow::anyhow!("cannot push without a current branch"))?;
    let mut remote = repo.find_remote(remote_name)?;
    let refspec = format!("refs/heads/{branch_name}:refs/heads/{branch_name}");
    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks());
    remote.push(&[refspec], Some(&mut options))?;
    Ok(())
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
        branches.push(BranchSummary {
            name,
            is_head: branch.is_head(),
            kind: match kind {
                BranchType::Local => "local",
                BranchType::Remote => "remote",
            }
            .to_string(),
            target,
        });
    }
    Ok(branches)
}

pub fn commit_graph(path: impl AsRef<Path>, limit: usize) -> anyhow::Result<Vec<CommitSummary>> {
    let repo = Repository::open(path.as_ref())?;
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    let mut commits = Vec::new();
    for oid in walk.take(limit) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        commits.push(CommitSummary {
            id: oid.to_string(),
            summary: commit.summary().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("").to_string(),
            time: commit.time().seconds(),
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
        });
    }
    Ok(commits)
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

pub fn delete_branch(path: impl AsRef<Path>, name: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let mut branch = repo.find_branch(name, BranchType::Local)?;
    branch.delete()?;
    Ok(())
}

pub fn merge_branch(path: impl AsRef<Path>, name: &str) -> anyhow::Result<()> {
    let repo = Repository::open(path.as_ref())?;
    let oid = repo.refname_to_id(&format!("refs/heads/{name}"))?;
    let annotated = repo.find_annotated_commit(oid)?;
    let mut options = MergeOptions::new();
    repo.merge(&[&annotated], Some(&mut options), None)?;
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
    Ok(())
}

pub fn diff_workdir(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let diff = repo.diff_index_to_workdir(None, None)?;
    diff_to_patch(&diff)
}

pub fn diff_staged(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let repo = Repository::open(path.as_ref())?;
    let head_tree = repo.head().ok().and_then(|head| head.peel_to_tree().ok());
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, None)?;
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
    repo.cleanup_state()?;
    Ok(())
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
                replay_commit(&repo, &step.commit, ReplayMode::Pick)?;
                result.head = head_oid(&repo);
                result.applied.push(step.commit.clone());
            }
            RebaseAction::Squash => {
                replay_commit(&repo, &step.commit, ReplayMode::Squash)?;
                result.head = head_oid(&repo);
                result.applied.push(step.commit.clone());
            }
            RebaseAction::Fixup => {
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
        output.extend_from_slice(line.content());
        true
    })?;
    Ok(String::from_utf8_lossy(&output).to_string())
}

enum ReplayMode {
    Pick,
    Squash,
    Fixup,
}

fn ensure_clean_for_history_rewrite(repo: &Repository) -> anyhow::Result<()> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    if repo.statuses(Some(&mut options))?.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("working tree must be clean before interactive rebase")
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
        ReplayMode::Pick => {
            commit_current_index(repo, commit.message().unwrap_or("Rebased commit"))?;
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

fn callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_, username, _| {
        if let Some(username) = username {
            Cred::ssh_key_from_agent(username)
        } else {
            Cred::default()
        }
    });
    callbacks
}
