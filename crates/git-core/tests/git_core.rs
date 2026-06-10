use git2::{Repository, Signature};
use std::fs;

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
            },
            zync_git_core::RebaseStep {
                commit: commit_b.clone(),
                action: zync_git_core::RebaseAction::Drop,
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
