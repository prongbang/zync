use dioxus::prelude::*;
use crate::*;

pub(crate) fn load_repositories(
    api: api::ZyncApi,
    mut repositories: Signal<Vec<api::RepositoryRecord>>,
    mut notice: Signal<String>,
) {
    spawn(async move {
        match api.repositories().await {
            Ok(items) => repositories.set(items),
            Err(error) => notice.set(error),
        }
    });
}

thread_local! {
    static WORKSPACE_REFRESH_RUNNING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static WORKSPACE_REFRESH_PENDING: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static WORKSPACE_REFRESH_KEY: std::cell::RefCell<String> =
        const { std::cell::RefCell::new(String::new()) };
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn load_workspace(
    api: api::ZyncApi,
    repository_id: String,
    workspace_id: String,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    notice: Signal<String>,
) {
    load_workspace_scoped(
        api,
        repository_id,
        workspace_id,
        workspace,
        git_status,
        branches,
        commits,
        stashes,
        conflicts,
        diff,
        notice,
        SCOPE_ALL,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_repository_workspace(
    api: api::ZyncApi,
    repository_id: String,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut repo_stats: Signal<Option<api::RepoStats>>,
    mut notice: Signal<String>,
    live_sync: Signal<bool>,
) {
    spawn(async move {
        match api.open_repository(&repository_id).await {
            Ok(opened) => {
                notice.set("Workspace opened and watcher attached".to_string());
                repo_stats.set(None);
                start_live_events(
                    api.clone(),
                    opened.repository.id.clone(),
                    opened.workspace.id.clone(),
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                    live_sync,
                );
                load_workspace(
                    api,
                    opened.repository.id,
                    opened.workspace.id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn load_workspace_scoped(
    api: api::ZyncApi,
    repository_id: String,
    workspace_id: String,
    mut workspace: Signal<Option<api::WorkspaceResponse>>,
    mut git_status: Signal<Vec<api::FileStatus>>,
    mut branches: Signal<Vec<api::BranchSummary>>,
    mut commits: Signal<Vec<api::CommitSummary>>,
    mut stashes: Signal<Vec<api::StashSummary>>,
    mut conflicts: Signal<Vec<api::ConflictSummary>>,
    mut diff: Signal<String>,
    mut notice: Signal<String>,
    scope: u8,
) {
    // Coalesce refresh storms (e.g. many watcher events during a fetch/pull):
    // if a refresh for the same workspace is already in flight, merge the
    // requested scope into one follow-up pass instead of piling up requests.
    let key = format!("{repository_id}:{workspace_id}");
    let same_target = WORKSPACE_REFRESH_KEY.with(|current| *current.borrow() == key);
    if WORKSPACE_REFRESH_RUNNING.with(std::cell::Cell::get) && same_target {
        WORKSPACE_REFRESH_PENDING.with(|pending| pending.set(pending.get() | scope));
        return;
    }
    WORKSPACE_REFRESH_KEY.with(|current| *current.borrow_mut() = key.clone());
    WORKSPACE_REFRESH_RUNNING.with(|running| running.set(true));
    WORKSPACE_REFRESH_PENDING.with(|pending| pending.set(0));

    spawn(async move {
        let mut scope = scope;
        loop {
            // Keep however many commits the user has already loaded via "Load more".
            let graph_limit = commits.read().len().max(500);
            let (
                workspace_result,
                status_result,
                branches_result,
                graph_result,
                stashes_result,
                conflicts_result,
                diff_result,
            ) = futures_util::join!(
                async {
                    if scope & SCOPE_WORKSPACE != 0 {
                        Some(api.workspace(&workspace_id).await)
                    } else {
                        None
                    }
                },
                async {
                    if scope & SCOPE_STATUS != 0 {
                        Some(api.status(&repository_id).await)
                    } else {
                        None
                    }
                },
                async {
                    if scope & SCOPE_BRANCHES != 0 {
                        Some(api.branches(&repository_id).await)
                    } else {
                        None
                    }
                },
                async {
                    if scope & SCOPE_GRAPH != 0 {
                        Some(api.graph_with_limit(&repository_id, graph_limit).await)
                    } else {
                        None
                    }
                },
                async {
                    if scope & SCOPE_STASHES != 0 {
                        Some(api.stashes(&repository_id).await)
                    } else {
                        None
                    }
                },
                async {
                    if scope & SCOPE_CONFLICTS != 0 {
                        Some(api.conflicts(&repository_id).await)
                    } else {
                        None
                    }
                },
                async {
                    if scope & SCOPE_DIFF != 0 {
                        Some(api.diff_workdir(&repository_id).await)
                    } else {
                        None
                    }
                },
            );
            match workspace_result {
                Some(Ok(next_workspace)) => workspace.set(Some(next_workspace)),
                Some(Err(error)) => notice.set(error),
                None => {}
            }
            match status_result {
                Some(Ok(items)) => git_status.set(items),
                Some(Err(error)) => notice.set(error),
                None => {}
            }
            match branches_result {
                Some(Ok(items)) => branches.set(items),
                Some(Err(error)) => notice.set(error),
                None => {}
            }
            match graph_result {
                Some(Ok(items)) => commits.set(items),
                Some(Err(error)) => notice.set(error),
                None => {}
            }
            match stashes_result {
                Some(Ok(items)) => stashes.set(items),
                Some(Err(error)) => notice.set(error),
                None => {}
            }
            match conflicts_result {
                Some(Ok(items)) => conflicts.set(items),
                Some(Err(error)) => notice.set(error),
                None => {}
            }
            match diff_result {
                Some(Ok(patch)) => diff.set(patch),
                Some(Err(error)) => notice.set(error),
                None => {}
            }

            let still_target = WORKSPACE_REFRESH_KEY.with(|current| *current.borrow() == key);
            if still_target {
                let pending = WORKSPACE_REFRESH_PENDING.with(|pending| pending.replace(0));
                if pending != 0 {
                    scope = pending;
                    continue;
                }
                WORKSPACE_REFRESH_RUNNING.with(|running| running.set(false));
            }
            break;
        }
    });
}

pub(crate) fn run_file_action_from_workspace(
    api: api::ZyncApi,
    current: Option<api::WorkspaceResponse>,
    files: Vec<String>,
    action: FileAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let Some(current) = current else {
        notice.set("Open a repository first".to_string());
        return;
    };
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let result = match action {
            FileAction::Stage => api.stage_files(&repository_id, files).await,
            FileAction::Unstage => api.unstage_files(&repository_id, files).await,
            FileAction::Discard => api.discard_files(&repository_id, files).await,
        };
        match result {
            Ok(()) => {
                notice.set("Git status updated".to_string());
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

pub(crate) fn run_file_tree_action(
    api: api::ZyncApi,
    current: Option<api::WorkspaceResponse>,
    action: FileTreeAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let Some(current) = current else {
        notice.set("Open a workspace first".to_string());
        return;
    };
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let result = match action {
            FileTreeAction::Create(path, is_dir) => {
                if path.trim().is_empty() {
                    Err("Path is required".to_string())
                } else {
                    api.create_file(&workspace_id, &path, is_dir).await
                }
            }
            FileTreeAction::Rename(old_path, new_path) => {
                if old_path.trim().is_empty() || new_path.trim().is_empty() {
                    Err("Both old and new paths are required".to_string())
                } else {
                    api.rename_file(&workspace_id, &old_path, &new_path).await
                }
            }
            FileTreeAction::Delete(path) => {
                if path.trim().is_empty() {
                    Err("Select a file before deleting".to_string())
                } else {
                    api.delete_file(&workspace_id, &path).await
                }
            }
        };
        match result {
            Ok(()) => {
                notice.set("File tree updated".to_string());
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

pub(crate) fn run_commit_action(
    api: api::ZyncApi,
    current: Option<api::WorkspaceResponse>,
    message: String,
    amend: bool,
    sign_off: bool,
    push_after: bool,
    mut commit_message: Signal<String>,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
    toast: Signal<Option<ToastMessage>>,
) {
    let Some(current) = current else {
        notice.set("Open a repository before committing".to_string());
        return;
    };
    let message = message.trim().to_string();
    if message.is_empty() {
        notice.set("Commit message is required".to_string());
        return;
    }
    let repository_id = current.repository.id.clone();
    let workspace_id = current.workspace.id.clone();
    spawn(async move {
        let request = api::CommitRequest {
            message,
            author_name: "Zync".to_string(),
            author_email: "zync@local".to_string(),
            amend,
            sign_off,
        };
        match api.commit(&repository_id, &request).await {
            Ok(_) => {
                if push_after {
                    match api.push(&repository_id).await {
                        Ok(output) => {
                            show_toast(
                                notice,
                                toast,
                                ToastKind::Success,
                                "Committed and pushed",
                                output,
                            );
                        }
                        Err(error) => show_toast(
                            notice,
                            toast,
                            ToastKind::Error,
                            "Committed, push failed",
                            error,
                        ),
                    }
                } else {
                    show_toast(notice, toast, ToastKind::Success, "Committed", "");
                }
                commit_message.set(String::new());
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

// Fork-style branch creation: optionally stash-and-reapply or discard local
// changes around the create+checkout, so switching never fails on a dirty tree.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_create_branch_action(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    name: String,
    revision: String,
    checkout: bool,
    local_mode: LocalChangesMode,
    changed_files: Vec<String>,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        if name.trim().is_empty() {
            notice.set("Branch name is required".to_string());
            return;
        }

        let handle_changes = checkout && !changed_files.is_empty();
        let mut stashed = false;
        if handle_changes {
            match local_mode {
                LocalChangesMode::StashReapply => {
                    let message = format!("Auto-stash before switching to {name}");
                    if let Err(error) = api.create_stash(&repository_id, &message).await {
                        notice.set(format!("Stash before branch failed: {error}"));
                        return;
                    }
                    stashed = true;
                }
                LocalChangesMode::Discard => {
                    if let Err(error) =
                        api.discard_files(&repository_id, changed_files.clone()).await
                    {
                        notice.set(format!("Discard before branch failed: {error}"));
                        return;
                    }
                }
                LocalChangesMode::DontChange => {}
            }
        }

        let create_result = if revision.trim().is_empty() {
            api.create_branch(&repository_id, &name, checkout).await
        } else {
            api.create_branch_at(&repository_id, &name, &revision, checkout)
                .await
        };
        if let Err(error) = create_result {
            if stashed {
                // Put the working tree back the way we found it.
                let _ = api.apply_stash(&repository_id, 0, true).await;
            }
            notice.set(error);
            return;
        }

        if stashed {
            match api.apply_stash(&repository_id, 0, true).await {
                Ok(()) => notice.set(format!("Created {name} and reapplied local changes")),
                Err(error) => notice.set(format!(
                    "Created {name}, but reapplying the stash failed: {error}"
                )),
            }
        } else {
            notice.set(format!("Created branch {name}"));
        }

        load_workspace(
            api,
            repository_id,
            workspace_id,
            workspace,
            git_status,
            branches,
            commits,
            stashes,
            conflicts,
            diff,
            notice,
        );
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_interactive_rebase_plan(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    base: String,
    steps: Vec<api::RebaseStepRequest>,
    success_notice: String,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let request = api::InteractiveRebaseRequest { base, steps };
        match api.interactive_rebase(&repository_id, &request).await {
            Ok(_) => {
                notice.set(success_notice);
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_commit_quick_action(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    action: CommitQuickAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let result = match &action {
            CommitQuickAction::Checkout(id) => api
                .checkout_revision(&repository_id, id)
                .await
                .map(|_| format!("Checked out {} (detached HEAD)", short_id(id))),
            CommitQuickAction::CherryPick(id) => api
                .cherry_pick(&repository_id, vec![id.clone()])
                .await
                .map(|_| format!("Cherry-picked {}", short_id(id))),
            CommitQuickAction::Revert(id) => api
                .revert_commit(&repository_id, id)
                .await
                .map(|_| format!("Reverted {}", short_id(id))),
            CommitQuickAction::Reset(id, hard) => api
                .reset_to_revision(&repository_id, id, *hard)
                .await
                .map(|_| {
                    format!(
                        "Reset ({}) to {}",
                        if *hard { "hard" } else { "mixed" },
                        short_id(id),
                    )
                }),
        };
        match result {
            Ok(message) => {
                notice.set(message);
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn download_text_file(filename: &str, content: &str, mut notice: Signal<String>) {
    use wasm_bindgen::JsCast;
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        notice.set("Browser download is unavailable".to_string());
        return;
    };
    let Ok(element) = document.create_element("a") else {
        notice.set("Browser download is unavailable".to_string());
        return;
    };
    let Ok(anchor) = element.dyn_into::<web_sys::HtmlAnchorElement>() else {
        notice.set("Browser download is unavailable".to_string());
        return;
    };
    anchor.set_href(&format!(
        "data:text/plain;charset=utf-8,{}",
        urlencoding::encode(content)
    ));
    anchor.set_download(filename);
    anchor.click();
    notice.set(format!("Saved {filename}"));
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn download_text_file(_filename: &str, _content: &str, mut notice: Signal<String>) {
    notice.set("Download is only available in the browser".to_string());
}

pub(crate) fn run_branch_action(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    action: BranchAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let result = match action {
            BranchAction::Create(name) => api.create_branch(&repository_id, &name, true).await,
            BranchAction::Checkout(name) => api.checkout_branch(&repository_id, &name).await,
            BranchAction::Merge(name) => api.merge_branch(&repository_id, &name).await,
            BranchAction::Delete(name) => api.delete_branch(&repository_id, &name).await,
            BranchAction::Rename(name, new_name) => {
                if new_name.trim().is_empty() {
                    Err("New branch name is required".to_string())
                } else {
                    api.rename_branch(&repository_id, &name, &new_name).await
                }
            }
        };
        match result {
            Ok(()) => {
                notice.set("Branch action complete".to_string());
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

pub(crate) fn run_tag_action(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    action: TagAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let result = match action {
            TagAction::Create(name, target) => {
                if name.trim().is_empty() {
                    Err("Tag name is required".to_string())
                } else {
                    let target = target.trim();
                    api.create_tag(
                        &repository_id,
                        &name,
                        if target.is_empty() {
                            None
                        } else {
                            Some(target)
                        },
                    )
                    .await
                }
            }
        };
        match result {
            Ok(()) => {
                notice.set("Tag action complete".to_string());
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

pub(crate) fn load_branch_rebase_steps(
    api: api::ZyncApi,
    repository_id: String,
    mut steps: Signal<Vec<api::RebaseStepRequest>>,
    mut notice: Signal<String>,
) {
    spawn(async move {
        match api.rebase_plan(&repository_id, 12).await {
            Ok(plan) => {
                steps.set(
                    plan.into_iter()
                        .map(|commit| api::RebaseStepRequest {
                            commit: commit.id,
                            action: "pick".to_string(),
                                            message: None,
                        })
                        .collect(),
                );
                notice.set("Rebase todo loaded".to_string());
            }
            Err(error) => notice.set(error),
        }
    });
}

pub(crate) fn copy_to_clipboard(value: String, mut notice: Signal<String>) {
    #[cfg(target_arch = "wasm32")]
    {
        spawn(async move {
            let Some(window) = web_sys::window() else {
                notice.set(format!("Branch name: {value}"));
                return;
            };
            let clipboard = window.navigator().clipboard();
            match wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&value)).await {
                Ok(_) => notice.set(format!("Copied to clipboard: {value}")),
                Err(_) => notice.set(format!("Branch name: {value}")),
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    notice.set(format!("Copied to clipboard: {value}"));
}

pub(crate) fn show_toast(
    mut notice: Signal<String>,
    mut toast: Signal<Option<ToastMessage>>,
    kind: ToastKind,
    title: impl Into<String>,
    detail: impl Into<String>,
) {
    let title = title.into();
    let detail = detail.into();
    let footer_message = if detail.trim().is_empty() {
        title.clone()
    } else {
        format!("{title}: {}", detail.trim())
    };
    notice.set(footer_message);
    toast.set(Some(ToastMessage {
        kind,
        title,
        detail: detail.trim().to_string(),
    }));
}

pub(crate) fn run_remote_action(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    action: RemoteAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
    toast: Signal<Option<ToastMessage>>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let label = match action {
            RemoteAction::Fetch => "Fetch",
            RemoteAction::Pull => "Pull",
            RemoteAction::Push => "Push",
        };
        notice.set(format!("{label} running"));
        let result = match action {
            RemoteAction::Fetch => api.fetch(&repository_id).await,
            RemoteAction::Pull => api.pull(&repository_id).await,
            RemoteAction::Push => api.push(&repository_id).await,
        };
        match result {
            Ok(output) => {
                show_toast(
                    notice,
                    toast,
                    ToastKind::Success,
                    format!("{label} complete"),
                    output,
                );
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => show_toast(
                notice,
                toast,
                ToastKind::Error,
                format!("{label} failed"),
                error,
            ),
        }
    });
}

pub(crate) fn run_stash_action(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    action: StashAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    notice: Signal<String>,
    toast: Signal<Option<ToastMessage>>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let label = match &action {
            StashAction::Create(_) => "Stash created",
            StashAction::Apply(_) => "Stash applied",
            StashAction::Pop(_) => "Stash popped",
            StashAction::Drop(_) => "Stash dropped",
        };
        let result = match action {
            StashAction::Create(message) => api.create_stash(&repository_id, &message).await,
            StashAction::Apply(index) => api.apply_stash(&repository_id, index, false).await,
            StashAction::Pop(index) => api.apply_stash(&repository_id, index, true).await,
            StashAction::Drop(index) => api.drop_stash(&repository_id, index).await,
        };
        match result {
            Ok(()) => {
                show_toast(notice, toast, ToastKind::Success, label, "");
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => show_toast(notice, toast, ToastKind::Error, "Stash failed", error),
        }
    });
}

pub(crate) fn run_history_action(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    action: HistoryAction,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let result = match action {
            HistoryAction::CherryPick(ids) => api.cherry_pick(&repository_id, ids).await,
            HistoryAction::CherryAbort => api.cherry_pick_abort(&repository_id).await,
            HistoryAction::Rebase(base, steps) => api
                .interactive_rebase(
                    &repository_id,
                    &api::InteractiveRebaseRequest { base, steps },
                )
                .await
                .map(|_| ()),
            HistoryAction::Resolve(path, side) => {
                api.resolve_conflict(&repository_id, &path, &side).await
            }
        };
        match result {
            Ok(()) => {
                notice.set("History action complete".to_string());
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_repository_tool(
    api: api::ZyncApi,
    current: api::WorkspaceResponse,
    action: ToolAction,
    selected_file: String,
    revision: String,
    branch_name: String,
    tag_name: String,
    file_path: String,
    remote_name: String,
    remote_url: String,
    flow_name: String,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let revision = revision.trim().to_string();
        let branch_name = branch_name.trim().to_string();
        let tag_name = tag_name.trim().to_string();
        let file_path = if file_path.trim().is_empty() {
            selected_file
        } else {
            file_path.trim().to_string()
        };
        let remote_name = remote_name.trim().to_string();
        let remote_url = remote_url.trim().to_string();
        let flow_name = flow_name.trim().to_string();

        let result = match action {
            ToolAction::CheckoutRevision => api
                .checkout_revision(&repository_id, revision.as_str())
                .await
                .map(|_| "Checked out revision".to_string()),
            ToolAction::BranchFromRevision => {
                if branch_name.is_empty() {
                    Err("Branch name is required".to_string())
                } else {
                    api.create_branch_at(&repository_id, &branch_name, &revision, true)
                        .await
                        .map(|_| format!("Created branch {branch_name} at {revision}"))
                }
            }
            ToolAction::RevertCommit => api
                .revert_commit(&repository_id, &revision)
                .await
                .map(|_| format!("Reverted {revision}")),
            ToolAction::CreateTag => {
                if tag_name.is_empty() {
                    Err("Tag name is required".to_string())
                } else {
                    api.create_tag(&repository_id, &tag_name, Some(&revision))
                        .await
                        .map(|_| format!("Created tag {tag_name}"))
                }
            }
            ToolAction::DeleteTag => {
                if tag_name.is_empty() {
                    Err("Tag name is required".to_string())
                } else {
                    api.delete_tag(&repository_id, &tag_name)
                        .await
                        .map(|_| format!("Deleted tag {tag_name}"))
                }
            }
            ToolAction::Tags => api.tags(&repository_id).await.and_then(pretty_json),
            ToolAction::Blame => {
                if file_path.is_empty() {
                    Err("File path is required".to_string())
                } else {
                    api.blame(&repository_id, &file_path)
                        .await
                        .and_then(pretty_json)
                }
            }
            ToolAction::FileHistory => {
                if file_path.is_empty() {
                    Err("File path is required".to_string())
                } else {
                    api.file_history(&repository_id, &file_path)
                        .await
                        .and_then(pretty_json)
                }
            }
            ToolAction::TreeAtRevision => api
                .tree_at_revision(&repository_id, &revision)
                .await
                .and_then(pretty_json),
            ToolAction::Reflog => api.reflog(&repository_id).await.and_then(pretty_json),
            ToolAction::ResetMixed => api
                .reset_to_revision(&repository_id, &revision, false)
                .await
                .map(|_| format!("Reset mixed to {revision}")),
            ToolAction::ResetHard => api
                .reset_to_revision(&repository_id, &revision, true)
                .await
                .map(|_| format!("Reset hard to {revision}")),
            ToolAction::Submodules => api.submodules(&repository_id).await.and_then(pretty_json),
            ToolAction::Lfs => api.lfs_summary(&repository_id).await.and_then(pretty_json),
            ToolAction::Remotes => api.remotes(&repository_id).await.and_then(pretty_json),
            ToolAction::AddRemote => {
                if remote_name.is_empty() || remote_url.is_empty() {
                    Err("Remote name and URL are required".to_string())
                } else {
                    api.add_remote(&repository_id, &remote_name, &remote_url)
                        .await
                        .map(|_| format!("Added remote {remote_name}"))
                }
            }
            ToolAction::DeleteRemote => {
                if remote_name.is_empty() {
                    Err("Remote name is required".to_string())
                } else {
                    api.delete_remote(&repository_id, &remote_name)
                        .await
                        .map(|_| format!("Deleted remote {remote_name}"))
                }
            }
            ToolAction::PruneRemote => api.prune_remote(&repository_id, &remote_name).await,
            ToolAction::DeleteRemoteBranch => {
                if flow_name.is_empty() {
                    Err("Branch name is required".to_string())
                } else {
                    api.delete_remote_branch(&repository_id, &remote_name, &flow_name)
                        .await
                        .map(|_| format!("Deleted {remote_name}/{flow_name}"))
                }
            }
            ToolAction::SetUpstream => {
                if flow_name.is_empty() {
                    Err("Branch name is required".to_string())
                } else {
                    api.set_upstream(&repository_id, &remote_name, &flow_name)
                        .await
                }
            }
            ToolAction::PushForceWithLease => {
                if flow_name.is_empty() {
                    Err("Branch name is required".to_string())
                } else {
                    api.push_force_with_lease(&repository_id, &remote_name, &flow_name)
                        .await
                }
            }
            ToolAction::SubmoduleInit => api.submodule_init(&repository_id).await,
            ToolAction::SubmoduleUpdate => api.submodule_update(&repository_id).await,
            ToolAction::SubmoduleSync => api.submodule_sync(&repository_id).await,
            ToolAction::LfsInstall => api.lfs_install(&repository_id).await,
            ToolAction::LfsTrack => {
                if flow_name.is_empty() {
                    Err("LFS pattern is required".to_string())
                } else {
                    api.lfs_track(&repository_id, &flow_name).await
                }
            }
            ToolAction::LfsUntrack => {
                if flow_name.is_empty() {
                    Err("LFS pattern is required".to_string())
                } else {
                    api.lfs_untrack(&repository_id, &flow_name).await
                }
            }
            ToolAction::LfsPull => api.lfs_pull(&repository_id).await,
            ToolAction::LfsPush => {
                if flow_name.is_empty() {
                    Err("Branch name is required".to_string())
                } else {
                    api.lfs_push(&repository_id, &remote_name, &flow_name).await
                }
            }
            ToolAction::RebaseContinue => api.rebase_continue(&repository_id).await,
            ToolAction::RebaseAbort => api.rebase_abort(&repository_id).await,
            ToolAction::RebaseSkip => api.rebase_skip(&repository_id).await,
            ToolAction::GitFlowDevelop => api
                .create_branch(&repository_id, "develop", true)
                .await
                .map(|_| "Created develop branch".to_string()),
            ToolAction::GitFlowFeature => {
                create_flow_branch(&api, &repository_id, "feature", &flow_name).await
            }
            ToolAction::GitFlowRelease => {
                create_flow_branch(&api, &repository_id, "release", &flow_name).await
            }
            ToolAction::GitFlowHotfix => {
                create_flow_branch(&api, &repository_id, "hotfix", &flow_name).await
            }
            ToolAction::GithubLinks => github_links(&api, &repository_id, &revision).await,
        };

        match result {
            Ok(message) => {
                notice.set(message);
                load_workspace(
                    api,
                    repository_id,
                    workspace_id,
                    workspace,
                    git_status,
                    branches,
                    commits,
                    stashes,
                    conflicts,
                    diff,
                    notice,
                );
            }
            Err(error) => {
                notice.set(error);
            }
        }
    });
}

async fn create_flow_branch(
    api: &api::ZyncApi,
    repository_id: &str,
    prefix: &str,
    name: &str,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Git-flow name is required".to_string());
    }
    let branch = format!("{prefix}/{}", name.trim());
    api.create_branch(repository_id, &branch, true).await?;
    Ok(format!("Created {branch}"))
}

async fn github_links(
    api: &api::ZyncApi,
    repository_id: &str,
    revision: &str,
) -> Result<String, String> {
    let remotes = api.remotes(repository_id).await?;
    let mut links = Vec::new();
    for remote in remotes {
        let Some(url) = remote.url.or(remote.push_url) else {
            continue;
        };
        let Some(repo_url) = github_repo_url(&url) else {
            continue;
        };
        let target = if revision.trim().is_empty() {
            "HEAD"
        } else {
            revision.trim()
        };
        links.push(serde_json::json!({
            "remote": remote.name,
            "repository": repo_url,
            "commits": format!("{repo_url}/commits"),
            "branches": format!("{repo_url}/branches"),
            "compare": format!("{repo_url}/compare"),
            "target": format!("{repo_url}/tree/{target}"),
        }));
    }
    if links.is_empty() {
        Err("No GitHub remote URL found".to_string())
    } else {
        pretty_json(links)
    }
}

// Bumped every time a new workspace subscribes; older reconnect loops notice
// their generation is stale and exit instead of fighting over the socket.
#[cfg(target_arch = "wasm32")]
thread_local! {
    static LIVE_SYNC_GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(target_arch = "wasm32")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn start_live_events(
    api: api::ZyncApi,
    repository_id: String,
    workspace_id: String,
    workspace: Signal<Option<api::WorkspaceResponse>>,
    git_status: Signal<Vec<api::FileStatus>>,
    branches: Signal<Vec<api::BranchSummary>>,
    commits: Signal<Vec<api::CommitSummary>>,
    stashes: Signal<Vec<api::StashSummary>>,
    conflicts: Signal<Vec<api::ConflictSummary>>,
    diff: Signal<String>,
    mut notice: Signal<String>,
    mut live_sync: Signal<bool>,
) {
    use futures_util::StreamExt;
    use gloo_net::websocket::futures::WebSocket;
    use gloo_net::websocket::Message;

    let generation = LIVE_SYNC_GENERATION.with(|cell| {
        let next = cell.get() + 1;
        cell.set(next);
        next
    });
    let is_stale = move || LIVE_SYNC_GENERATION.with(std::cell::Cell::get) != generation;

    let url = api.websocket_url(&workspace_id);
    spawn(async move {
        let mut attempts = 0u32;
        let mut connected_before = false;
        loop {
            if is_stale() {
                return;
            }
            match WebSocket::open(&url) {
                Ok(mut socket) => {
                    attempts = 0;
                    live_sync.set(true);
                    if connected_before {
                        notice.set("Live sync reconnected".to_string());
                        // Events may have been missed while offline.
                        load_workspace_scoped(
                            api.clone(),
                            repository_id.clone(),
                            workspace_id.clone(),
                            workspace,
                            git_status,
                            branches,
                            commits,
                            stashes,
                            conflicts,
                            diff,
                            notice,
                            SCOPE_ALL,
                        );
                    } else {
                        notice.set("Live sync connected".to_string());
                    }
                    connected_before = true;
                    while let Some(message) = socket.next().await {
                        if is_stale() {
                            return;
                        }
                        match message {
                            Ok(message) => {
                                let scope = match &message {
                                    Message::Text(text) => scope_for_event(text),
                                    Message::Bytes(_) => SCOPE_ALL,
                                };
                                load_workspace_scoped(
                                    api.clone(),
                                    repository_id.clone(),
                                    workspace_id.clone(),
                                    workspace,
                                    git_status,
                                    branches,
                                    commits,
                                    stashes,
                                    conflicts,
                                    diff,
                                    notice,
                                    scope,
                                );
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(_) => {}
            }
            if is_stale() {
                return;
            }
            live_sync.set(false);
            attempts = attempts.saturating_add(1);
            let delay_seconds = 2u64.saturating_pow(attempts.min(5)).min(30);
            notice.set(format!(
                "Live sync offline - reconnecting in {delay_seconds}s"
            ));
            gloo_timers::future::TimeoutFuture::new((delay_seconds * 1000) as u32).await;
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn start_live_events(
    _api: api::ZyncApi,
    _repository_id: String,
    _workspace_id: String,
    _workspace: Signal<Option<api::WorkspaceResponse>>,
    _git_status: Signal<Vec<api::FileStatus>>,
    _branches: Signal<Vec<api::BranchSummary>>,
    _commits: Signal<Vec<api::CommitSummary>>,
    _stashes: Signal<Vec<api::StashSummary>>,
    _conflicts: Signal<Vec<api::ConflictSummary>>,
    _diff: Signal<String>,
    _notice: Signal<String>,
    _live_sync: Signal<bool>,
) {
}
