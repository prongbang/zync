use git2::{Repository, Signature};
use std::fs;
use std::time::{Duration, Instant};
use zync_git_core::GitErrorKind;

#[test]
fn status_add_commit_and_branch_flow() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = Repository::init(temp.path()).expect("init repo");
    let signature = Signature::now("Zync Test", "zync@test.local").expect("signature");

    fs::write(temp.path().join("README.md"), "hello").expect("write readme");
    zync_git_core::add(temp.path(), &["README.md".to_string()]).expect("add readme");
    let mut index = repo.index().expect("index");
    let tree_id = index.write_tree().expect("tree");
    let tree = repo.find_tree(tree_id).expect("tree");
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .expect("initial commit");

    fs::write(temp.path().join("README.md"), "hello\nworld").expect("modify readme");
    let status = zync_git_core::status(temp.path()).expect("status");
    assert!(status
        .iter()
        .any(|file| file.path == "README.md" && file.unstaged));

    zync_git_core::add(temp.path(), &["README.md".to_string()]).expect("stage readme");
    let commit =
        zync_git_core::commit(temp.path(), "Update readme", "Zync Test", "zync@test.local")
            .expect("commit");
    assert!(!commit.is_empty());

    zync_git_core::create_branch(temp.path(), "feature/test", true).expect("create branch");
    let info = zync_git_core::open_repo(temp.path()).expect("open repo");
    assert_eq!(info.current_branch.as_deref(), Some("feature/test"));
}

#[test]
fn unstage_discard_and_interactive_rebase_flow() {
    let temp = tempfile::tempdir().expect("tempdir");
    Repository::init(temp.path()).expect("init repo");

    fs::write(temp.path().join("base.txt"), "base").expect("write base");
    zync_git_core::add(temp.path(), &["base.txt".to_string()]).expect("add base");
    let base = zync_git_core::commit(temp.path(), "Base", "Zync Test", "zync@test.local")
        .expect("base commit");

    fs::write(temp.path().join("b.txt"), "b").expect("write b");
    zync_git_core::add(temp.path(), &["b.txt".to_string()]).expect("add b");
    let commit_b = zync_git_core::commit(temp.path(), "Commit B", "Zync Test", "zync@test.local")
        .expect("commit b");

    fs::write(temp.path().join("c.txt"), "c").expect("write c");
    zync_git_core::add(temp.path(), &["c.txt".to_string()]).expect("add c");
    let commit_c = zync_git_core::commit(temp.path(), "Commit C", "Zync Test", "zync@test.local")
        .expect("commit c");

    fs::write(temp.path().join("scratch.txt"), "scratch").expect("write scratch");
    zync_git_core::add(temp.path(), &["scratch.txt".to_string()]).expect("stage scratch");
    zync_git_core::unstage(temp.path(), &["scratch.txt".to_string()]).expect("unstage scratch");
    let status = zync_git_core::status(temp.path()).expect("status");
    assert!(status
        .iter()
        .any(|file| file.path == "scratch.txt" && file.untracked));
    fs::remove_file(temp.path().join("scratch.txt")).expect("remove scratch");

    let result = zync_git_core::interactive_rebase(
        temp.path(),
        &base,
        &[
            zync_git_core::RebaseStep {
                commit: commit_c.clone(),
                action: zync_git_core::RebaseAction::Pick,
                message: None,
            },
            zync_git_core::RebaseStep {
                commit: commit_b.clone(),
                action: zync_git_core::RebaseAction::Drop,
                message: None,
            },
        ],
    )
    .expect("interactive rebase");

    assert_eq!(result.applied, vec![commit_c]);
    assert_eq!(result.dropped, vec![commit_b]);
    assert!(temp.path().join("c.txt").exists());
    assert!(!temp.path().join("b.txt").exists());
    assert!(zync_git_core::status(temp.path())
        .expect("status")
        .is_empty());
}

#[test]
fn commit_graph_cursor_pagination() {
    let temp = tempfile::tempdir().expect("tempdir");
    Repository::init(temp.path()).expect("init repo");

    let mut commits = Vec::new();
    for name in ["one", "two", "three", "four"] {
        fs::write(temp.path().join(format!("{name}.txt")), name).expect("write file");
        zync_git_core::add(temp.path(), &[format!("{name}.txt")]).expect("add file");
        let oid = zync_git_core::commit(temp.path(), name, "Zync Test", "zync@test.local")
            .expect("commit");
        commits.push(oid);
    }
    // commits is oldest-first: [one, two, three, four]; the graph walks newest-first.
    let newest_first: Vec<_> = commits.iter().rev().cloned().collect();

    let full = zync_git_core::commit_graph(temp.path(), 10, None).expect("full graph");
    let full_ids: Vec<_> = full.iter().map(|commit| commit.id.clone()).collect();
    assert_eq!(full_ids, newest_first);

    let first_page = zync_git_core::commit_graph(temp.path(), 2, None).expect("first page");
    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page[0].id, newest_first[0]);
    assert_eq!(first_page[1].id, newest_first[1]);

    let cursor = first_page.last().unwrap().id.clone();
    let second_page =
        zync_git_core::commit_graph(temp.path(), 10, Some(&cursor)).expect("second page");
    let second_ids: Vec<_> = second_page.iter().map(|commit| commit.id.clone()).collect();
    assert_eq!(second_ids, newest_first[2..]);
}

#[test]
fn push_to_bare_remote_via_libgit2() {
    let bare = tempfile::tempdir().expect("bare tempdir");
    Repository::init_bare(bare.path()).expect("init bare repo");

    let temp = tempfile::tempdir().expect("tempdir");
    let repo = Repository::init(temp.path()).expect("init repo");
    let signature = Signature::now("Zync Test", "zync@test.local").expect("signature");

    fs::write(temp.path().join("README.md"), "hello").expect("write readme");
    zync_git_core::add(temp.path(), &["README.md".to_string()]).expect("add readme");
    let mut index = repo.index().expect("index");
    let tree_id = index.write_tree().expect("tree");
    let tree = repo.find_tree(tree_id).expect("tree");
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .expect("initial commit");

    let remote_url = format!("file://{}", bare.path().display());
    zync_git_core::add_remote(temp.path(), "origin", &remote_url).expect("add remote");

    // Exercises the libgit2 Remote::push path (CredentialSpec::Default via the `None` spec,
    // plus the push-rejection check and post-push set_upstream that replace the old CLI `git
    // push -u`) against a real file:// remote end to end.
    let output = zync_git_core::push(temp.path(), Some("origin"), None).expect("push succeeds");
    assert!(!output.is_empty());

    let branch = zync_git_core::open_repo(temp.path())
        .expect("open repo")
        .current_branch
        .expect("current branch");
    let bare_repo = Repository::open(bare.path()).expect("open bare repo");
    let bare_ref = bare_repo
        .find_reference(&format!("refs/heads/{branch}"))
        .expect("bare repo has pushed branch");
    let pushed_commit = bare_ref.peel_to_commit().expect("peel to commit");
    assert_eq!(pushed_commit.message(), Some("Initial commit"));

    // set_upstream ran as part of push_with_credentials, matching the old `git push -u`.
    let repo = Repository::open(temp.path()).expect("reopen repo");
    let local_branch = repo
        .find_branch(&branch, git2::BranchType::Local)
        .expect("local branch");
    let upstream = local_branch.upstream().expect("upstream is set");
    assert_eq!(
        upstream.name().expect("upstream name"),
        Some(format!("origin/{branch}").as_str())
    );
}

/// Creates an initialized (non-bare) repo at `path` with a single commit containing one file,
/// returning the commit oid as a string. Shared setup for the credentialed-transport tests
/// below, all of which exercise the new libgit2 network paths (`spec: None` = `CredentialSpec::
/// Default`, exactly what `file://` transport needs — see P0.3 test plan).
fn init_repo_with_commit(path: &std::path::Path, file_name: &str, contents: &str) -> String {
    let repo = Repository::init(path).expect("init repo");
    let signature = Signature::now("Zync Test", "zync@test.local").expect("signature");
    fs::write(path.join(file_name), contents).expect("write file");
    zync_git_core::add(path, &[file_name.to_string()]).expect("add file");
    let mut index = repo.index().expect("index");
    let tree_id = index.write_tree().expect("tree");
    let tree = repo.find_tree(tree_id).expect("tree");
    let oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .expect("initial commit");
    oid.to_string()
}

#[test]
fn clone_repo_round_trip_via_libgit2_file_url() {
    let origin = tempfile::tempdir().expect("origin tempdir");
    init_repo_with_commit(origin.path(), "README.md", "hello");
    let origin_info = zync_git_core::open_repo(origin.path()).expect("open origin");

    let dest = tempfile::tempdir().expect("dest tempdir");
    let dest_path = dest.path().join("clone");
    let url = format!("file://{}", origin.path().display());

    // Exercises the libgit2 RepoBuilder + FetchOptions clone path (CredentialSpec::Default via
    // the `None` spec) end to end against a real file:// remote.
    let cloned_info = zync_git_core::clone_repo(&url, &dest_path).expect("clone via libgit2");

    assert!(dest_path.join("README.md").exists());
    assert_eq!(cloned_info.current_branch, origin_info.current_branch);
    assert_eq!(cloned_info.head, origin_info.head);
}

#[test]
fn fetch_and_push_round_trip_via_libgit2() {
    let bare = tempfile::tempdir().expect("bare tempdir");
    Repository::init_bare(bare.path()).expect("init bare repo");
    let bare_url = format!("file://{}", bare.path().display());

    let origin = tempfile::tempdir().expect("origin tempdir");
    init_repo_with_commit(origin.path(), "a.txt", "a");
    zync_git_core::add_remote(origin.path(), "origin", &bare_url).expect("add remote");
    zync_git_core::push(origin.path(), Some("origin"), None).expect("initial push");
    let branch = zync_git_core::open_repo(origin.path())
        .expect("open origin")
        .current_branch
        .expect("origin has a current branch");

    let clone_dir = tempfile::tempdir().expect("clone tempdir");
    let clone_path = clone_dir.path().join("clone");
    zync_git_core::clone_repo(&bare_url, &clone_path).expect("clone");

    fs::write(origin.path().join("b.txt"), "b").expect("write b");
    zync_git_core::add(origin.path(), &["b.txt".to_string()]).expect("add b");
    let second_commit =
        zync_git_core::commit(origin.path(), "Second", "Zync Test", "zync@test.local")
            .expect("commit b");
    // Exercises the libgit2 Remote::push path a second time (upstream already set from the
    // initial push above).
    zync_git_core::push(origin.path(), Some("origin"), None).expect("second push");

    // Exercises the libgit2 Remote::fetch path: the clone picks up the new commit and its
    // remote-tracking ref (and FETCH_HEAD) get updated exactly like a real `git fetch` would.
    let output = zync_git_core::fetch(&clone_path, Some("origin")).expect("fetch");
    assert!(!output.is_empty());

    let clone_repo = Repository::open(&clone_path).expect("open clone");
    let tracking_ref = clone_repo
        .find_reference(&format!("refs/remotes/origin/{branch}"))
        .expect("tracking ref updated by fetch");
    let tracking_commit = tracking_ref.peel_to_commit().expect("peel to commit");
    assert_eq!(tracking_commit.id().to_string(), second_commit);

    let fetch_head = clone_repo
        .find_reference("FETCH_HEAD")
        .expect("FETCH_HEAD written by fetch");
    assert_eq!(
        fetch_head.target().expect("FETCH_HEAD has a target").to_string(),
        second_commit
    );
}

#[test]
fn push_force_with_lease_rejects_stale_and_accepts_current() {
    let bare = tempfile::tempdir().expect("bare tempdir");
    Repository::init_bare(bare.path()).expect("init bare repo");
    let bare_url = format!("file://{}", bare.path().display());

    let repo_a = tempfile::tempdir().expect("repo a tempdir");
    init_repo_with_commit(repo_a.path(), "a.txt", "a");
    zync_git_core::add_remote(repo_a.path(), "origin", &bare_url).expect("add remote a");
    zync_git_core::push(repo_a.path(), Some("origin"), None).expect("initial push");
    let branch = zync_git_core::open_repo(repo_a.path())
        .expect("open a")
        .current_branch
        .expect("repo a has a current branch");

    // repo B clones the same remote and pushes an extra commit repo A doesn't know about.
    let repo_b = tempfile::tempdir().expect("repo b tempdir");
    let repo_b_path = repo_b.path().join("clone");
    zync_git_core::clone_repo(&bare_url, &repo_b_path).expect("clone b");
    fs::write(repo_b_path.join("b.txt"), "b").expect("write b");
    zync_git_core::add(&repo_b_path, &["b.txt".to_string()]).expect("add b");
    zync_git_core::commit(&repo_b_path, "From B", "Zync Test", "zync@test.local")
        .expect("commit b");
    zync_git_core::push(&repo_b_path, Some("origin"), None).expect("push from b");

    // repo A hasn't fetched since, so its cached remote-tracking ref no longer matches the
    // bare remote's actual current oid: the manual lease check must reject the force push.
    let stale = zync_git_core::push_force_with_lease(repo_a.path(), "origin", &branch)
        .expect_err("stale lease should be rejected");
    let stale_err = stale
        .downcast_ref::<zync_git_core::GitCommandError>()
        .expect("stale rejection is a GitCommandError");
    assert_eq!(stale_err.kind, GitErrorKind::NonFastForward);

    // The bare remote must be unaffected by the rejected attempt.
    let bare_repo = Repository::open(bare.path()).expect("open bare");
    let bare_ref = bare_repo
        .find_reference(&format!("refs/heads/{branch}"))
        .expect("bare has branch");
    assert_eq!(
        bare_ref.peel_to_commit().expect("peel").message(),
        Some("From B")
    );

    // Fetching refreshes repo A's remote-tracking ref to match the bare remote's actual state...
    zync_git_core::fetch(repo_a.path(), Some("origin")).expect("fetch updates tracking ref");

    // ...so the lease now matches and a force push of repo A's own divergent history succeeds.
    fs::write(repo_a.path().join("c.txt"), "c").expect("write c");
    zync_git_core::add(repo_a.path(), &["c.txt".to_string()]).expect("add c");
    zync_git_core::commit(repo_a.path(), "From A", "Zync Test", "zync@test.local")
        .expect("commit c");

    let output = zync_git_core::push_force_with_lease(repo_a.path(), "origin", &branch)
        .expect("lease-verified force push succeeds");
    assert!(!output.is_empty());

    let bare_ref = bare_repo
        .find_reference(&format!("refs/heads/{branch}"))
        .expect("bare has branch");
    assert_eq!(
        bare_ref.peel_to_commit().expect("peel").message(),
        Some("From A")
    );
}

#[test]
fn pull_ff_only_fast_forwards_after_remote_advances() {
    let bare = tempfile::tempdir().expect("bare tempdir");
    Repository::init_bare(bare.path()).expect("init bare repo");
    let bare_url = format!("file://{}", bare.path().display());

    let origin = tempfile::tempdir().expect("origin tempdir");
    init_repo_with_commit(origin.path(), "a.txt", "a");
    zync_git_core::add_remote(origin.path(), "origin", &bare_url).expect("add remote");
    zync_git_core::push(origin.path(), Some("origin"), None).expect("initial push");
    let branch = zync_git_core::open_repo(origin.path())
        .expect("open origin")
        .current_branch
        .expect("origin has a current branch");

    let clone_dir = tempfile::tempdir().expect("clone tempdir");
    let clone_path = clone_dir.path().join("clone");
    zync_git_core::clone_repo(&bare_url, &clone_path).expect("clone");

    // origin advances and pushes; the clone hasn't fetched yet.
    fs::write(origin.path().join("b.txt"), "b").expect("write b");
    zync_git_core::add(origin.path(), &["b.txt".to_string()]).expect("add b");
    let advanced_commit =
        zync_git_core::commit(origin.path(), "Advance", "Zync Test", "zync@test.local")
            .expect("commit b");
    zync_git_core::push(origin.path(), Some("origin"), None).expect("push advance");

    // Exercises the libgit2 ff-only pull path: fetch + fast-forward of the local branch.
    zync_git_core::pull(&clone_path, Some("origin"), Some(&branch)).expect("ff-only pull");

    let clone_info = zync_git_core::open_repo(&clone_path).expect("open clone");
    assert_eq!(clone_info.head, Some(advanced_commit));
    assert!(clone_path.join("b.txt").exists());
}

#[test]
fn classify_git_stderr_maps_known_patterns_to_kinds() {
    let cases: &[(&str, GitErrorKind)] = &[
        (
            "fatal: Authentication failed for 'https://example.com/repo.git'",
            GitErrorKind::Auth,
        ),
        (
            "fatal: could not read Username for 'https://example.com': terminal prompts disabled",
            GitErrorKind::Auth,
        ),
        (
            "git@github.com: Permission denied (publickey).",
            GitErrorKind::Auth,
        ),
        (
            "ssh: Could not resolve hostname example.invalid: nodename nor servname provided",
            GitErrorKind::Network,
        ),
        (
            "ssh: connect to host 127.0.0.1 port 1: Connection refused",
            GitErrorKind::Network,
        ),
        (
            "! [rejected]        main -> main (fetch first)",
            GitErrorKind::NonFastForward,
        ),
        (
            "! [rejected]  main -> main (non-fast-forward)",
            GitErrorKind::NonFastForward,
        ),
        (
            "CONFLICT (content): Merge conflict in README.md",
            GitErrorKind::Conflict,
        ),
        (
            "fatal: some completely unrelated failure",
            GitErrorKind::Other,
        ),
    ];

    for (stderr, expected) in cases {
        assert_eq!(
            zync_git_core::classify_git_stderr(stderr),
            *expected,
            "misclassified: {stderr}"
        );
    }
}

#[test]
fn fetch_from_unreachable_host_fails_fast() {
    let temp = tempfile::tempdir().expect("tempdir");
    Repository::init(temp.path()).expect("init repo");

    // Port 1 on localhost has nothing listening; batch-mode SSH refuses the connection almost
    // immediately instead of hanging on a credential prompt.
    zync_git_core::add_remote(temp.path(), "unreachable", "ssh://127.0.0.1:1/nonexistent")
        .expect("add remote");

    let start = Instant::now();
    let error = zync_git_core::fetch(temp.path(), Some("unreachable"))
        .expect_err("fetch from unreachable host should fail");
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(10),
        "fetch should fail fast, took {elapsed:?}"
    );

    // Kind is intentionally not asserted here: without ssh on PATH (e.g. a minimal CI image)
    // the failure text won't match the Network/Auth patterns and would classify as Other. The
    // fast-fail behavior (no hang on the timeout) is what this test guards.
    error
        .downcast_ref::<zync_git_core::GitCommandError>()
        .expect("error should be a GitCommandError");
}
