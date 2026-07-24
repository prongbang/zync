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
fn push_to_bare_remote_via_hardened_run_git() {
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

    // Exercises the hardened `run_git` shellout (GIT_TERMINAL_PROMPT=0 / batch-mode SSH env,
    // background-drained pipes, timeout guard) against a real file:// remote end to end.
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
