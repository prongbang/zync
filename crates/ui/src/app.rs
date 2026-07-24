use dioxus::prelude::*;
use crate::*;

pub fn app() -> Element {
    let api = use_signal(api::ZyncApi::default);
    let api_base = api.read().base_url.clone();

    let mut repositories = use_signal(Vec::<api::RepositoryRecord>::new);
    let mut workspace = use_signal(|| None::<api::WorkspaceResponse>);
    let mut git_status = use_signal(Vec::<api::FileStatus>::new);
    let mut branches = use_signal(Vec::<api::BranchSummary>::new);
    let mut commits = use_signal(Vec::<api::CommitSummary>::new);
    let mut selected_commit = use_signal(|| None::<api::CommitSummary>);
    let mut stashes = use_signal(Vec::<api::StashSummary>::new);
    let mut conflicts = use_signal(Vec::<api::ConflictSummary>::new);
    let mut conflict_detail = use_signal(api::ConflictDetail::default);
    let mut manual_conflict_content = use_signal(String::new);
    let mut diff = use_signal(String::new);
    let mut selected_file = use_signal(String::new);
    let mut editor_content = use_signal(String::new);
    let mut repo_add_mode = use_signal(|| RepoAddMode::Folder);
    let mut repo_path = use_signal(String::new);
    let mut repo_browser_open = use_signal(|| false);
    let mut repo_browser = use_signal(api::DirectoryList::default);
    let mut repo_remote_url = use_signal(String::new);
    let mut repo_clone_to = use_signal(String::new);
    let mut repo_name = use_signal(String::new);
    let mut commit_message = use_signal(String::new);
    let mut commit_amend = use_signal(|| false);
    let mut commit_sign_off = use_signal(|| false);
    let mut commit_push_after = use_signal(|| false);
    let mut stash_message = use_signal(|| "WIP from Zync".to_string());
    let mut cherry_pick_input = use_signal(String::new);
    let mut new_branch_name = use_signal(String::new);
    let mut rebase_base = use_signal(String::new);
    let mut rebase_steps = use_signal(Vec::<api::RebaseStepRequest>::new);
    let mut graph_limit = use_signal(|| 500usize);
    let mut tool_revision = use_signal(|| "HEAD".to_string());
    let mut tool_branch = use_signal(String::new);
    let mut tool_tag = use_signal(String::new);
    let mut tool_file = use_signal(String::new);
    let mut tool_remote_name = use_signal(|| "origin".to_string());
    let mut tool_remote_url = use_signal(String::new);
    let mut tool_flow_name = use_signal(String::new);
    let mut sidebar_width = use_signal(|| 320u16);
    let mut left_pane_width = use_signal(|| 260u16);
    let mut inspector_width = use_signal(|| 380u16);
    let mut history_height = use_signal(|| 320u16);
    let mut active_resize = use_signal(|| None::<ResizeDragTarget>);
    let mut auto_opened_first_repo = use_signal(|| false);
    let mut mobile_sidebar_open = use_signal(|| false);
    let mut sidebar_open_menu = use_signal(|| None::<String>);
    let mut sidebar_stash_menu = use_signal(|| None::<usize>);
    let mut branch_dialog = use_signal(|| None::<BranchDialog>);
    let mut branch_dialog_value = use_signal(String::new);
    let mut branch_dialog_target = use_signal(String::new);
    let mut branch_dialog_checkout = use_signal(|| true);
    let mut branch_dialog_local_mode = use_signal(|| LocalChangesMode::StashReapply);
    let mut branch_dialog_rebase_steps = use_signal(Vec::<api::RebaseStepRequest>::new);
    let mut stash_apply_dialog = use_signal(|| None::<api::StashSummary>);
    let mut stash_apply_delete = use_signal(|| true);
    let mut commit_section_mode = use_signal(|| CommitSectionMode::Commits);
    let mut notice = use_signal(|| "Ready".to_string());
    let mut toast = use_signal(|| None::<ToastMessage>);
    let mut repo_stats = use_signal(|| None::<api::RepoStats>);
    let mut blame_view = use_signal(|| None::<BlameView>);
    let mut commit_menu = use_signal(|| None::<String>);
    let live_sync_ok = use_signal(|| false);

    // Lane layout is O(commits x lanes); recompute only when the commit list changes.
    let graph_row_data = use_memo(move || graph_rows(&commits.read()));

    {
        let api = api.read().clone();
        use_effect(move || {
            load_repositories(api.clone(), repositories, notice);
        });
    }

    {
        let api = api.read().clone();
        use_effect(move || {
            if *auto_opened_first_repo.read() || workspace.read().is_some() {
                return;
            }
            let Some(repository) = repositories.read().first().cloned() else {
                return;
            };
            auto_opened_first_repo.set(true);
            let api_client = api.clone();
            spawn(async move {
                match api_client.open_repository(&repository.id).await {
                    Ok(opened) => {
                        notice.set("Workspace opened and watcher attached".to_string());
                        start_live_events(
                            api_client.clone(),
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
                            live_sync_ok,
                        );
                        load_workspace(
                            api_client,
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
        });
    }

    let current_repository_id = workspace
        .read()
        .as_ref()
        .map(|item| item.repository.id.clone())
        .unwrap_or_default();
    let changed_count = git_status.read().len();
    let current_branch = branches
        .read()
        .iter()
        .find(|branch| branch.is_head)
        .map(|branch| branch.name.clone())
        .unwrap_or_else(|| "no branch".to_string());

    {
        use_effect(move || {
            if *commit_section_mode.read() == CommitSectionMode::LocalChanges
                && git_status.read().is_empty()
            {
                selected_commit.set(None);
                selected_file.set(String::new());
                diff.set(String::new());
            }
        });
    }

    let layout_style = format!(
        "--sidebar-width:{}px;--left-pane:{}px;--right-pane:{}px;--history-height:{}px;",
        *sidebar_width.read(),
        *left_pane_width.read(),
        *inspector_width.read(),
        *history_height.read()
    );
    let shell_class = format!(
        "app-shell min-h-screen xl:h-screen bg-zinc-950 text-zinc-100 flex flex-col xl:flex-row overflow-y-auto xl:overflow-hidden{}",
        match *active_resize.read() {
            Some(ResizeDragTarget::History) => " is-resizing is-resizing-row",
            Some(_) => " is-resizing is-resizing-col",
            None => "",
        }
    );
    let sidebar_class = format!(
        "workspace-sidebar fork-sidebar{} w-full xl:w-[280px] xl:h-screen shrink-0 border-b xl:border-b-0 xl:border-r border-zinc-800 bg-zinc-950 flex flex-col",
        if *mobile_sidebar_open.read() {
            " fork-sidebar-open"
        } else {
            ""
        }
    );
    let toast_snapshot = toast.read().clone();

    rsx! {
        style { "{TAILWIND_CSS}" }
        style { "{APP_CSS}" }
        main {
            class: "{shell_class}",
            style: "{layout_style}",
            onpointermove: move |event| {
                let target = *active_resize.read();
                let Some(target) = target else {
                    return;
                };
                let coordinates = event.client_coordinates();
                match target {
                    ResizeDragTarget::Sidebar => {
                        sidebar_width.set(clamp_pane_size(coordinates.x, 220, 420));
                    }
                    ResizeDragTarget::LeftPane => {
                        let grid_left = f64::from(*sidebar_width.read()) + 14.0;
                        left_pane_width.set(clamp_pane_size(coordinates.x - grid_left, 220, 420));
                    }
                    ResizeDragTarget::Inspector => {
                        if let Some(width) = viewport_width() {
                            inspector_width.set(clamp_pane_size(width - coordinates.x, 320, 560));
                        }
                    }
                    ResizeDragTarget::History => {
                        history_height.set(clamp_pane_size(coordinates.y - 48.0, 240, 520));
                    }
                }
            },
            onpointerup: move |_| active_resize.set(None),
            onpointercancel: move |_| active_resize.set(None),
            if *mobile_sidebar_open.read() {
                button {
                    class: "mobile-sidebar-scrim",
                    title: "Close navigation",
                    onclick: move |_| {
                        mobile_sidebar_open.set(false);
                        sidebar_open_menu.set(None);
                    }
                }
            }
            if let Some(message) = toast_snapshot {
                div {
                    class: match message.kind {
                        ToastKind::Success => "zync-toast zync-toast-success",
                        ToastKind::Error => "zync-toast zync-toast-error",
                    },
                    div { class: "zync-toast-mark",
                        match message.kind {
                            ToastKind::Success => "OK",
                            ToastKind::Error => "!",
                        }
                    }
                    div { class: "zync-toast-copy",
                        strong { "{message.title}" }
                        if !message.detail.is_empty() {
                            p { "{message.detail}" }
                        }
                    }
                    button {
                        class: "zync-toast-close",
                        title: "Dismiss",
                        onclick: move |_| toast.set(None),
                        "x"
                    }
                }
            }
            aside { class: "{sidebar_class}",
                header { class: "fork-sidebar-title h-12 shrink-0 border-b border-zinc-800 px-3 flex items-center justify-between gap-3",
                    div { class: "min-w-0",
                        h1 { class: "text-sm font-semibold tracking-tight", if let Some(current) = workspace.read().as_ref() { "{current.repository.name}" } else { "Zync" } }
                        p { class: "min-w-0 truncate text-[11px] text-zinc-500", "API {api_base}" }
                    }
                    div { class: "flex items-center gap-2",
                        span { class: "text-zinc-500", "..." }
                        button {
                            class: "mobile-sidebar-close",
                            title: "Close navigation",
                            onclick: move |_| {
                                mobile_sidebar_open.set(false);
                                sidebar_open_menu.set(None);
                            },
                            "x"
                        }
                    }
                }

                RepositorySelector {
                    repositories: repositories.read().clone(),
                    selected_repository_id: current_repository_id.clone(),
                    current_branch: current_branch.clone(),
                    on_open: move |repository_id: String| {
                        open_repository_workspace(
                            api.read().clone(),
                            repository_id,
                            workspace,
                            git_status,
                            branches,
                            commits,
                            stashes,
                            conflicts,
                            diff,
                            repo_stats,
                            notice,
                            live_sync_ok,
                        );
                    },
                    on_favorite: move |(repository_id, favorite): (String, bool)| {
                        let api_client = api.read().clone();
                        spawn(async move {
                            match api_client.set_repository_favorite(&repository_id, favorite).await {
                                Ok(()) => {
                                    let current_workspace = { workspace.read().clone() };
                                    if let Some(mut current) = current_workspace {
                                        if current.repository.id == repository_id {
                                            current.repository.favorite = favorite;
                                            workspace.set(Some(current));
                                        }
                                    }
                                    notice.set(if favorite {
                                        "Repository marked as favorite".to_string()
                                    } else {
                                        "Repository removed from favorites".to_string()
                                    });
                                    load_repositories(api_client, repositories, notice);
                                }
                                Err(error) => notice.set(error),
                            }
                        });
                    }
                }

                details { class: "fork-mount-panel shrink-0 border-b border-zinc-800 bg-zinc-900/40",
                    summary { class: "fork-mount-summary",
                        span { "Add repository" }
                    }
                    div { class: "fork-mount-body space-y-2",
                    div { class: "fork-add-mode-tabs",
                        button {
                            class: if *repo_add_mode.read() == RepoAddMode::Folder { "fork-add-mode-tab fork-add-mode-tab-active" } else { "fork-add-mode-tab" },
                            onclick: move |_| repo_add_mode.set(RepoAddMode::Folder),
                            "Folder"
                        }
                        button {
                            class: if *repo_add_mode.read() == RepoAddMode::GitUrl { "fork-add-mode-tab fork-add-mode-tab-active" } else { "fork-add-mode-tab" },
                            onclick: move |_| repo_add_mode.set(RepoAddMode::GitUrl),
                            "Git URL"
                        }
                    }
                    if *repo_add_mode.read() == RepoAddMode::Folder {
                        div { class: "grid grid-cols-[1fr_auto] gap-2",
                            input {
                                class: "w-full min-w-0 rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                                placeholder: "Repository folder path mounted on server",
                                value: "{repo_path}",
                                oninput: move |event| repo_path.set(event.value())
                            }
                            button {
                                class: "rounded border border-zinc-700 px-2 py-1.5 text-xs text-zinc-200 hover:bg-zinc-800",
                                onclick: move |_| {
                                    let api_client = api.read().clone();
                                    let path = repo_path.read().trim().to_string();
                                    repo_browser_open.set(true);
                                    spawn(async move {
                                        match api_client.directories(if path.is_empty() { None } else { Some(path.as_str()) }).await {
                                            Ok(list) => repo_browser.set(list),
                                            Err(error) => notice.set(error),
                                        }
                                    });
                                },
                                "Browse"
                            }
                        }
                        if *repo_browser_open.read() {
                            div { class: "fork-folder-browser",
                                div { class: "fork-folder-browser-head",
                                    span { class: "min-w-0 truncate", "{repo_browser.read().current_path}" }
                                    button {
                                        class: "fork-folder-browser-close",
                                        onclick: move |_| repo_browser_open.set(false),
                                        "Close"
                                    }
                                }
                                div { class: "fork-folder-browser-actions",
                                    if let Some(parent) = repo_browser.read().parent_path.clone() {
                                        button {
                                            class: "fork-folder-browser-row",
                                            onclick: move |_| {
                                                let api_client = api.read().clone();
                                                let parent_path = parent.clone();
                                                spawn(async move {
                                                    match api_client.directories(Some(&parent_path)).await {
                                                        Ok(list) => repo_browser.set(list),
                                                        Err(error) => notice.set(error),
                                                    }
                                                });
                                            },
                                            ".."
                                        }
                                    }
                                    button {
                                        class: "fork-folder-browser-row fork-folder-browser-select",
                                        onclick: move |_| {
                                            repo_path.set(repo_browser.read().current_path.clone());
                                            repo_browser_open.set(false);
                                        },
                                        "Use this folder"
                                    }
                                }
                                div { class: "fork-folder-browser-list",
                                    for entry in repo_browser.read().directories.clone() {
                                        button {
                                            class: "fork-folder-browser-row",
                                            title: "{entry.path}",
                                            onclick: move |_| {
                                                let api_client = api.read().clone();
                                                let path = entry.path.clone();
                                                spawn(async move {
                                                    match api_client.directories(Some(&path)).await {
                                                        Ok(list) => repo_browser.set(list),
                                                        Err(error) => notice.set(error),
                                                    }
                                                });
                                            },
                                            span { class: "min-w-0 truncate", "{entry.name}" }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        input {
                            class: "w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                            placeholder: "Git URL, e.g. https://github.com/org/repo.git",
                            value: "{repo_remote_url}",
                            oninput: move |event| repo_remote_url.set(event.value())
                        }
                        input {
                            class: "w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                            placeholder: "Clone destination folder on server",
                            value: "{repo_clone_to}",
                            oninput: move |event| repo_clone_to.set(event.value())
                        }
                    }
                    input {
                        class: "w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                        placeholder: "Name (optional)",
                        value: "{repo_name}",
                        oninput: move |event| repo_name.set(event.value())
                    }
                    div { class: "grid grid-cols-[1fr_auto] gap-2",
                        button {
                            class: "rounded bg-cyan-500 px-2 py-1.5 text-xs font-medium text-zinc-950 hover:bg-cyan-400 disabled:opacity-50",
                            onclick: move |_| {
                                let api_client = api.read().clone();
                                let mode = *repo_add_mode.read();
                                let path = repo_path.read().trim().to_string();
                                let remote_url = repo_remote_url.read().trim().to_string();
                                let clone_to = repo_clone_to.read().trim().to_string();
                                let name = repo_name.read().trim().to_string();
                                spawn(async move {
                                    let name = if name.is_empty() { None } else { Some(name) };
                                    let request = match mode {
                                        RepoAddMode::Folder => {
                                            if path.is_empty() {
                                                notice.set("Repository folder path is required".to_string());
                                                return;
                                            }
                                            api::CreateRepositoryRequest {
                                                name,
                                                path: Some(path),
                                                remote_url: None,
                                                clone_to: None,
                                            }
                                        }
                                        RepoAddMode::GitUrl => {
                                            if remote_url.is_empty() || clone_to.is_empty() {
                                                notice.set("Git URL and clone destination are required".to_string());
                                                return;
                                            }
                                            api::CreateRepositoryRequest {
                                                name,
                                                path: None,
                                                remote_url: Some(remote_url),
                                                clone_to: Some(clone_to),
                                            }
                                        }
                                    };
                                    match api_client.create_repository(&request).await {
                                        Ok(opened) => {
                                            notice.set("Repository added and watcher started".to_string());
                                            repo_path.set(String::new());
                                            repo_remote_url.set(String::new());
                                            repo_clone_to.set(String::new());
                                            repo_name.set(String::new());
                                            repositories.write().push(opened.repository.clone());
                                            start_live_events(
                                                api_client.clone(),
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
                                                live_sync_ok
                                            );
                                            load_workspace(
                                                api_client,
                                                opened.repository.id,
                                                opened.workspace.id,
                                                workspace,
                                                git_status,
                                                branches,
                                                commits,
                                                stashes,
                                                conflicts,
                                                diff,
                                                notice
                                            );
                                        }
                                        Err(error) => notice.set(error),
                                    }
                                });
                            },
                            if *repo_add_mode.read() == RepoAddMode::Folder { "Add folder repo" } else { "Clone git repo" }
                        }
                        button {
                            class: "rounded border border-zinc-700 px-2 py-1.5 text-xs text-zinc-200 hover:bg-zinc-800",
                            onclick: move |_| load_repositories(api.read().clone(), repositories, notice),
                            "Refresh"
                        }
                    }
                    }
                }

                ForkSidebarNavigation {
                    branches: branches.read().clone(),
                    stashes: stashes.read().clone(),
                    open_menu: sidebar_open_menu.read().clone(),
                    open_stash_menu: *sidebar_stash_menu.read(),
                    on_open_menu: move |name: String| sidebar_open_menu.set(Some(name)),
                    on_open_stash_menu: move |index: usize| {
                        sidebar_open_menu.set(None);
                        sidebar_stash_menu.set(Some(index));
                    },
                    on_close_menu: move |_| {
                        sidebar_open_menu.set(None);
                        sidebar_stash_menu.set(None);
                    },
                    on_checkout: move |name: String| {
                        mobile_sidebar_open.set(false);
                        sidebar_open_menu.set(None);
                        sidebar_stash_menu.set(None);
                        if let Some(current) = workspace.read().as_ref().cloned() {
                            run_branch_action(api.read().clone(), current, BranchAction::Checkout(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        }
                    },
                    on_branch_command: move |command: SidebarBranchCommand| {
                        mobile_sidebar_open.set(false);
                        sidebar_open_menu.set(None);
                        sidebar_stash_menu.set(None);
                        let branch_name = match &command {
                            SidebarBranchCommand::Checkout(name)
                            | SidebarBranchCommand::Merge(name)
                            | SidebarBranchCommand::Rebase(name)
                            | SidebarBranchCommand::InteractiveRebase(name)
                            | SidebarBranchCommand::NewBranch(name)
                            | SidebarBranchCommand::NewTag(name)
                            | SidebarBranchCommand::Rename(name)
                            | SidebarBranchCommand::Delete(name)
                            | SidebarBranchCommand::CopyName(name) => name.clone(),
                        };

                        let Some(current) = workspace.read().as_ref().cloned() else {
                            notice.set(format!("Open a repository before using {branch_name}"));
                            return;
                        };

                        match command {
                            SidebarBranchCommand::Checkout(name) => {
                                branch_dialog_value.set(name.clone());
                                branch_dialog.set(Some(BranchDialog::Checkout { branch: name }));
                            }
                            SidebarBranchCommand::Merge(name) => {
                                branch_dialog_value.set(name.clone());
                                branch_dialog.set(Some(BranchDialog::Merge { branch: name }));
                            }
                            SidebarBranchCommand::Delete(name) => {
                                branch_dialog_value.set(name.clone());
                                branch_dialog.set(Some(BranchDialog::Delete { branch: name }));
                            }
                            SidebarBranchCommand::CopyName(name) => {
                                copy_to_clipboard(name.clone(), notice);
                            }
                            SidebarBranchCommand::Rebase(name) => {
                                branch_dialog_value.set(name.clone());
                                branch_dialog_rebase_steps.set(Vec::new());
                                branch_dialog.set(Some(BranchDialog::Rebase {
                                    branch: name.clone(),
                                    interactive: false,
                                }));
                                load_branch_rebase_steps(
                                    api.read().clone(),
                                    current.repository.id,
                                    branch_dialog_rebase_steps,
                                    notice,
                                );
                            }
                            SidebarBranchCommand::InteractiveRebase(name) => {
                                branch_dialog_value.set(name.clone());
                                branch_dialog_rebase_steps.set(Vec::new());
                                branch_dialog.set(Some(BranchDialog::Rebase {
                                    branch: name.clone(),
                                    interactive: true,
                                }));
                                load_branch_rebase_steps(
                                    api.read().clone(),
                                    current.repository.id,
                                    branch_dialog_rebase_steps,
                                    notice,
                                );
                            }
                            SidebarBranchCommand::NewBranch(name) => {
                                let target = branches
                                    .read()
                                    .iter()
                                    .find(|branch| branch.name == name)
                                    .and_then(|branch| branch.target.clone());
                                branch_dialog_value.set(format!("{name}-copy"));
                                branch_dialog_target.set(target.clone().unwrap_or_else(|| name.clone()));
                                branch_dialog_checkout.set(true);
                                branch_dialog.set(Some(BranchDialog::NewBranch { branch: name, target }));
                            }
                            SidebarBranchCommand::NewTag(name) => {
                                let target = branches
                                    .read()
                                    .iter()
                                    .find(|branch| branch.name == name)
                                    .and_then(|branch| branch.target.clone());
                                branch_dialog_value.set(String::new());
                                branch_dialog_target.set(target.clone().unwrap_or_else(|| name.clone()));
                                branch_dialog.set(Some(BranchDialog::NewTag { branch: name, target }));
                            }
                            SidebarBranchCommand::Rename(name) => {
                                branch_dialog_value.set(name.clone());
                                branch_dialog.set(Some(BranchDialog::Rename { branch: name }));
                            }
                        }
                    },
                    on_stash_command: move |command: SidebarStashCommand| {
                        mobile_sidebar_open.set(false);
                        sidebar_open_menu.set(None);
                        sidebar_stash_menu.set(None);
                        match command {
                            SidebarStashCommand::Apply(stash) => {
                                stash_apply_delete.set(true);
                                stash_apply_dialog.set(Some(stash));
                            }
                            SidebarStashCommand::Drop(index) => {
                                if let Some(current) = workspace.read().as_ref().cloned() {
                                    run_stash_action(api.read().clone(), current, StashAction::Drop(index), workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                                } else {
                                    notice.set("Open a repository before stash action".to_string());
                                }
                            }
                        }
                    }
                }
            }
            PaneStepSplitter {
                label: "Sidebar".to_string(),
                class_name: "sidebar-step-splitter".to_string(),
                on_decrease: move |_| {
                    let next = (*sidebar_width.read()).saturating_sub(20).max(220);
                    sidebar_width.set(next);
                },
                on_increase: move |_| {
                    let next = ((*sidebar_width.read()).saturating_add(20)).min(420);
                    sidebar_width.set(next);
                },
                on_drag_start: move |_| active_resize.set(Some(ResizeDragTarget::Sidebar))
            }

            section { class: "fork-main-window relative min-w-0 flex-1 min-h-[70vh] xl:min-h-0 flex flex-col bg-zinc-900",
                header { class: "workspace-header fork-top-toolbar h-auto xl:h-12 shrink-0 border-b border-zinc-800 px-3 flex flex-col xl:flex-row xl:items-center justify-between gap-2 bg-zinc-950",
                    div { class: "fork-toolbar-left",
                        button {
                            class: "mobile-sidebar-toggle",
                            title: "Open navigation",
                            onclick: move |_| mobile_sidebar_open.set(true),
                            span { class: "mobile-sidebar-toggle-line" }
                            span { class: "mobile-sidebar-toggle-line" }
                            span { class: "mobile-sidebar-toggle-line" }
                        }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Fetch, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast); } },
                            ToolbarIcon { icon: ToolbarGlyph::Fetch }
                            span { "Fetch" }
                        }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Pull, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast); } },
                            ToolbarIcon { icon: ToolbarGlyph::Pull }
                            span { "Pull" }
                        }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Push, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast); } },
                            ToolbarIcon { icon: ToolbarGlyph::Push }
                            span { "Push" }
                        }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_stash_action(api.read().clone(), current, StashAction::Create(stash_message.read().clone()), workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast); } },
                            ToolbarIcon { icon: ToolbarGlyph::Stash }
                            span { "Stash" }
                        }
                    }
                    div { class: "legacy-toolbar-actions",
                    WorkspaceToolbar {
                        disabled: current_repository_id.is_empty(),
                        on_refresh: move |_| {
                            if let Some(current) = workspace.read().as_ref() {
                                load_workspace(api.read().clone(), current.repository.id.clone(), current.workspace.id.clone(), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_fetch: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_remote_action(api.read().clone(), current, RemoteAction::Fetch, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            }
                        },
                        on_pull: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_remote_action(api.read().clone(), current, RemoteAction::Pull, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            }
                        },
                        on_push: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_remote_action(api.read().clone(), current, RemoteAction::Push, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            }
                        }
                    }
                    }
                }

                if repositories.read().len() > 1 {
                    div { class: "repo-tab-strip",
                        for repository in repositories.read().iter().cloned() {
                            button {
                                class: if repository.id == current_repository_id { "repo-tab repo-tab-active" } else { "repo-tab" },
                                onclick: {
                                    let repository_id = repository.id.clone();
                                    let active_id = current_repository_id.clone();
                                    move |_| {
                                        if repository_id == active_id {
                                            return;
                                        }
                                        open_repository_workspace(
                                            api.read().clone(),
                                            repository_id.clone(),
                                            workspace,
                                            git_status,
                                            branches,
                                            commits,
                                            stashes,
                                            conflicts,
                                            diff,
                                            repo_stats,
                                            notice,
                                            live_sync_ok,
                                        );
                                    }
                                },
                                "{repository.name}"
                            }
                        }
                    }
                }

                div { class: "workspace-grid fork-workspace-grid relative min-h-0 flex-1 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-[260px_minmax(0,1fr)_380px] xl:grid-rows-[minmax(260px,0.95fr)_minmax(260px,0.75fr)_minmax(220px,0.55fr)_minmax(360px,auto)] gap-px bg-zinc-800 overflow-y-auto xl:overflow-hidden",
                    PaneGridSplitters {
                        on_left_decrease: move |_| {
                            let next = (*left_pane_width.read()).saturating_sub(20).max(220);
                            left_pane_width.set(next);
                        },
                        on_left_increase: move |_| {
                            let next = ((*left_pane_width.read()).saturating_add(20)).min(420);
                            left_pane_width.set(next);
                        },
                        on_left_drag_start: move |_| active_resize.set(Some(ResizeDragTarget::LeftPane)),
                        on_right_decrease: move |_| {
                            let next = (*inspector_width.read()).saturating_sub(20).max(320);
                            inspector_width.set(next);
                        },
                        on_right_increase: move |_| {
                            let next = ((*inspector_width.read()).saturating_add(20)).min(560);
                            inspector_width.set(next);
                        },
                        on_right_drag_start: move |_| active_resize.set(Some(ResizeDragTarget::Inspector)),
                        on_history_decrease: move |_| {
                            let next = (*history_height.read()).saturating_sub(20).max(240);
                            history_height.set(next);
                        },
                        on_history_increase: move |_| {
                            let next = ((*history_height.read()).saturating_add(20)).min(520);
                            history_height.set(next);
                        },
                        on_history_drag_start: move |_| active_resize.set(Some(ResizeDragTarget::History))
                    }
                    FileExplorer {
                        files: workspace.read().as_ref().map(|item| item.files.clone()).unwrap_or_default(),
                        selected: selected_file.read().clone(),
                        on_select: move |path: String| {
                            selected_file.set(path.clone());
                            if let Some(current) = workspace.read().as_ref() {
                                let api_client = api.read().clone();
                                let workspace_id = current.workspace.id.clone();
                                spawn(async move {
                                    match api_client.read_file(&workspace_id, &path).await {
                                        Ok(file) => editor_content.set(file.content),
                                        Err(error) => notice.set(error),
                                    }
                                });
                            } else {
                                notice.set("Open a workspace first".to_string());
                            }
                        },
                        on_create: move |(path, is_dir): (String, bool)| {
                            run_file_tree_action(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                FileTreeAction::Create(path, is_dir),
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_rename: move |(old_path, new_path): (String, String)| {
                            run_file_tree_action(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                FileTreeAction::Rename(old_path, new_path),
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_delete: move |path: String| {
                            run_file_tree_action(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                FileTreeAction::Delete(path),
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_search: move |query: String| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a workspace before searching files".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            spawn(async move {
                                match api_client.search_files(&current.workspace.id, &query).await {
                                    Ok(files) => {
                                        let mut next = current.clone();
                                        next.files = files;
                                        workspace.set(Some(next));
                                        notice.set(format!("Search matched files for '{query}'"));
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                    }

                    EditorPanel {
                        path: selected_file.read().clone(),
                        content: editor_content.read().clone(),
                        on_change: move |content: String| editor_content.set(content),
                        on_save: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a workspace before saving".to_string());
                                return;
                            };
                            if selected_file.read().is_empty() {
                                notice.set("Select a file before saving".to_string());
                                return;
                            }
                            let api_client = api.read().clone();
                            let workspace_id = current.workspace.id.clone();
                            let repository_id = current.repository.id.clone();
                            let path = selected_file.read().clone();
                            let content = editor_content.read().clone();
                            spawn(async move {
                                match api_client.write_file(&workspace_id, &path, content).await {
                                    Ok(()) => {
                                        notice.set("File saved".to_string());
                                        load_workspace(
                                            api_client,
                                            repository_id,
                                            workspace_id,
                                            workspace,
                                            git_status,
                                            branches,
                                            commits,
                                            stashes,
                                            conflicts,
                                            diff,
                                            notice
                                        );
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        }
                    }

                    GitStatusPanel {
                        files: git_status.read().clone(),
                        on_stage_all: move |paths: Vec<String>| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                paths,
                                FileAction::Stage,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_stage: move |path: String| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                vec![path],
                                FileAction::Stage,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_unstage_all: move |paths: Vec<String>| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                paths,
                                FileAction::Unstage,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_unstage: move |path: String| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                vec![path],
                                FileAction::Unstage,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_discard: move |path: String| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                vec![path],
                                FileAction::Discard,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_diff: move |path: String| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before viewing diff".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id;
                            spawn(async move {
                                let workdir = api_client.diff_workdir_file(&repository_id, &path).await.unwrap_or_default();
                                let staged = api_client.diff_staged_file(&repository_id, &path).await.unwrap_or_default();
                                let patch = if !workdir.trim().is_empty() {
                                    workdir
                                } else if !staged.trim().is_empty() {
                                    staged
                                } else {
                                    format!("No diff for {path}")
                                };
                                diff.set(patch);
                                notice.set(format!("Showing diff for {path}"));
                            });
                        }
                    }

                    if let Some(view) = blame_view.read().clone() {
                        BlameViewer {
                            view,
                            on_close: move |_| blame_view.set(None),
                        }
                    } else {
                    DiffViewer {
                        blame_available: !selected_file.read().is_empty(),
                        on_blame: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before viewing blame".to_string());
                                return;
                            };
                            let path = selected_file.read().clone();
                            if path.is_empty() {
                                notice.set("Select a file to blame".to_string());
                                return;
                            }
                            let api_client = api.read().clone();
                            spawn(async move {
                                let blame = api_client.blame(&current.repository.id, &path).await;
                                let content = api_client.read_file(&current.workspace.id, &path).await;
                                match (blame, content) {
                                    (Ok(ranges), Ok(file)) => {
                                        let rows = build_blame_rows(&ranges, &file.content);
                                        notice.set(format!("Showing blame for {path}"));
                                        blame_view.set(Some(BlameView { path, rows }));
                                    }
                                    (Err(error), _) | (_, Err(error)) => notice.set(error),
                                }
                            });
                        },
                        diff: diff.read().clone(),
                        image_path: selected_file.read().clone(),
                        image_before_url: workspace
                            .read()
                            .as_ref()
                            .map(|current| api.read().blob_url(&current.repository.id, "HEAD", &selected_file.read()))
                            .unwrap_or_default(),
                        image_after_url: workspace
                            .read()
                            .as_ref()
                            .map(|current| api.read().asset_url(&current.workspace.id, &selected_file.read()))
                            .unwrap_or_default(),
                        on_stage_patch: move |patch: String| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before staging a patch".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id.clone();
                            let workspace_id = current.workspace.id.clone();
                            spawn(async move {
                                match api_client.stage_patch(&repository_id, patch).await {
                                    Ok(()) => {
                                        notice.set("Patch staged".to_string());
                                        load_workspace(
                                            api_client,
                                            repository_id,
                                            workspace_id,
                                            workspace,
                                            git_status,
                                            branches,
                                            commits,
                                            stashes,
                                            conflicts,
                                            diff,
                                            notice
                                        );
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        }
                    }
                    }

                    CommitPanel {
                        message: commit_message.read().clone(),
                        amend: *commit_amend.read(),
                        sign_off: *commit_sign_off.read(),
                        push_after: *commit_push_after.read(),
                        on_message: move |message: String| commit_message.set(message),
                        on_amend: move |checked: bool| commit_amend.set(checked),
                        on_sign_off: move |checked: bool| commit_sign_off.set(checked),
                        on_push_after: move |checked: bool| commit_push_after.set(checked),
                        on_commit: move |_| {
                            run_commit_action(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                commit_message.read().trim().to_string(),
                                *commit_amend.read(),
                                *commit_sign_off.read(),
                                *commit_push_after.read(),
                                commit_message,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                                toast,
                            );
                        }
                    }

                    BranchPanel {
                        branches: branches.read().clone(),
                        new_branch_name: new_branch_name.read().clone(),
                        on_new_branch_name: move |name: String| new_branch_name.set(name),
                        on_create: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before creating a branch".to_string());
                                return;
                            };
                            let name = new_branch_name.read().trim().to_string();
                            if name.is_empty() {
                                notice.set("Branch name is required".to_string());
                                return;
                            }
                            let api_client = api.read().clone();
                            run_branch_action(
                                api_client,
                                current,
                                BranchAction::Create(name),
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                            new_branch_name.set(String::new());
                        },
                        on_checkout: move |name: String| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_branch_action(api.read().clone(), current, BranchAction::Checkout(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_merge: move |name: String| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_branch_action(api.read().clone(), current, BranchAction::Merge(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_delete: move |name: String| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_branch_action(api.read().clone(), current, BranchAction::Delete(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_rename: move |(name, new_name): (String, String)| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_branch_action(api.read().clone(), current, BranchAction::Rename(name, new_name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        }
                    }
                    CommitGraph {
                        open_menu: commit_menu.read().clone(),
                        on_open_menu: move |commit_id: String| commit_menu.set(Some(commit_id)),
                        on_close_menu: move |_| commit_menu.set(None),
                        on_menu_command: move |command: CommitMenuCommand| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before running commit actions".to_string());
                                return;
                            };
                            match command {
                                CommitMenuCommand::NewBranch(id) => {
                                    branch_dialog_value.set(String::new());
                                    branch_dialog_target.set(id.clone());
                                    branch_dialog_checkout.set(true);
                                    branch_dialog.set(Some(BranchDialog::NewBranch {
                                        branch: short_id(&id).to_string(),
                                        target: Some(id),
                                    }));
                                }
                                CommitMenuCommand::NewTag(id) => {
                                    branch_dialog_value.set(String::new());
                                    branch_dialog_target.set(id.clone());
                                    branch_dialog.set(Some(BranchDialog::NewTag {
                                        branch: short_id(&id).to_string(),
                                        target: Some(id),
                                    }));
                                }
                                CommitMenuCommand::RebaseToHere(id) => {
                                    branch_dialog_value.set(id.clone());
                                    branch_dialog_rebase_steps.set(Vec::new());
                                    branch_dialog.set(Some(BranchDialog::Rebase { branch: id, interactive: false }));
                                    load_branch_rebase_steps(api.read().clone(), current.repository.id, branch_dialog_rebase_steps, notice);
                                }
                                CommitMenuCommand::InteractiveRebase(id) => {
                                    branch_dialog_value.set(id.clone());
                                    branch_dialog_rebase_steps.set(Vec::new());
                                    branch_dialog.set(Some(BranchDialog::Rebase { branch: id, interactive: true }));
                                    load_branch_rebase_steps(api.read().clone(), current.repository.id, branch_dialog_rebase_steps, notice);
                                }
                                CommitMenuCommand::Reword(id) => {
                                    let summary = commits
                                        .read()
                                        .iter()
                                        .find(|commit| commit.id == id)
                                        .map(|commit| commit.summary.clone())
                                        .unwrap_or_default();
                                    branch_dialog_value.set(summary);
                                    branch_dialog.set(Some(BranchDialog::RewordCommit { commit: id }));
                                }
                                CommitMenuCommand::EditCommit(id) => {
                                    match quick_rebase_plan(&commits.read(), &id, "edit", None) {
                                        Ok((base, steps)) => run_interactive_rebase_plan(
                                            api.read().clone(), current, base, steps,
                                            format!("Stopped at {} with its changes staged for editing", short_id(&id)),
                                            workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                        ),
                                        Err(error) => notice.set(error),
                                    }
                                }
                                CommitMenuCommand::SquashIntoParent(id) => {
                                    match quick_rebase_plan(&commits.read(), &id, "squash", None) {
                                        Ok((base, steps)) => run_interactive_rebase_plan(
                                            api.read().clone(), current, base, steps,
                                            format!("Squashed {} into its parent", short_id(&id)),
                                            workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                        ),
                                        Err(error) => notice.set(error),
                                    }
                                }
                                CommitMenuCommand::FixupIntoParent(id) => {
                                    match quick_rebase_plan(&commits.read(), &id, "fixup", None) {
                                        Ok((base, steps)) => run_interactive_rebase_plan(
                                            api.read().clone(), current, base, steps,
                                            format!("Fixed up {} into its parent", short_id(&id)),
                                            workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                        ),
                                        Err(error) => notice.set(error),
                                    }
                                }
                                CommitMenuCommand::DropCommit(id) => {
                                    branch_dialog.set(Some(BranchDialog::DropCommit { commit: id }));
                                }
                                CommitMenuCommand::ResetToHere(id) => {
                                    branch_dialog_target.set("mixed".to_string());
                                    branch_dialog.set(Some(BranchDialog::ResetToCommit { commit: id }));
                                }
                                CommitMenuCommand::CheckoutCommit(id) => run_commit_quick_action(
                                    api.read().clone(), current, CommitQuickAction::Checkout(id),
                                    workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                ),
                                CommitMenuCommand::CherryPick(id) => run_commit_quick_action(
                                    api.read().clone(), current, CommitQuickAction::CherryPick(id),
                                    workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                ),
                                CommitMenuCommand::Revert(id) => run_commit_quick_action(
                                    api.read().clone(), current, CommitQuickAction::Revert(id),
                                    workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                ),
                                CommitMenuCommand::SaveAsPatch(id) => {
                                    let api_client = api.read().clone();
                                    let repository_id = current.repository.id;
                                    spawn(async move {
                                        match api_client.diff_commit(&repository_id, &id).await {
                                            Ok(patch) => download_text_file(
                                                &format!("{}.patch", short_id(&id)),
                                                &patch,
                                                notice,
                                            ),
                                            Err(error) => notice.set(error),
                                        }
                                    });
                                }
                                CommitMenuCommand::CompareToLocal(id) => {
                                    let api_client = api.read().clone();
                                    let repository_id = current.repository.id;
                                    spawn(async move {
                                        match api_client.diff_commit_to_workdir(&repository_id, &id).await {
                                            Ok(patch) => {
                                                blame_view.set(None);
                                                diff.set(patch);
                                                notice.set(format!(
                                                    "Comparing {} to local changes",
                                                    short_id(&id),
                                                ));
                                            }
                                            Err(error) => notice.set(error),
                                        }
                                    });
                                }
                                CommitMenuCommand::CopySha(id) => copy_to_clipboard(id, notice),
                            }
                        },
                        rows: graph_row_data.read().clone(),
                        files: git_status.read().clone(),
                        changed_count,
                        selected_file: selected_file.read().clone(),
                        selected_commit_id: selected_commit
                            .read()
                            .as_ref()
                            .map(|commit| commit.id.clone())
                            .unwrap_or_else(|| commits.read().first().map(|commit| commit.id.clone()).unwrap_or_default()),
                        mode: *commit_section_mode.read(),
                        on_local_changes: move |_| {
                            mobile_sidebar_open.set(false);
                            commit_section_mode.set(CommitSectionMode::LocalChanges);
                            notice.set("Showing local changes".to_string());
                        },
                        on_all_commits: move |_| {
                            mobile_sidebar_open.set(false);
                            commit_section_mode.set(CommitSectionMode::Commits);
                            notice.set("Commit graph focused".to_string());
                        },
                        on_select_local_file: move |path: String| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before viewing local changes".to_string());
                                return;
                            };
                            selected_file.set(path.clone());
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id;
                            spawn(async move {
                                let workdir = api_client.diff_workdir_file(&repository_id, &path).await.unwrap_or_default();
                                let staged = api_client.diff_staged_file(&repository_id, &path).await.unwrap_or_default();
                                let patch = if !workdir.trim().is_empty() {
                                    workdir
                                } else if !staged.trim().is_empty() {
                                    staged
                                } else {
                                    format!("No diff for {path}")
                                };
                                diff.set(patch);
                                notice.set(format!("Showing local diff for {path}"));
                            });
                        },
                        on_stage_local_file: move |path: String| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                vec![path],
                                FileAction::Stage,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_unstage_local_file: move |path: String| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                vec![path],
                                FileAction::Unstage,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_select_commit: move |commit_id: String| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before viewing commit diff".to_string());
                                return;
                            };
                            let selected = commits
                                .read()
                                .iter()
                                .find(|commit| commit.id == commit_id)
                                .cloned();
                            selected_commit.set(selected);
                            commit_section_mode.set(CommitSectionMode::Commits);
                            let api_client = api.read().clone();
                            spawn(async move {
                                match api_client.diff_commit(&current.repository.id, &commit_id).await {
                                    Ok(patch) => {
                                        diff.set(patch);
                                        notice.set(format!("Showing commit {}", short_id(&commit_id)));
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                        on_load_more: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before loading history".to_string());
                                return;
                            };
                            let next_limit = (*graph_limit.read() + 500).min(5000);
                            graph_limit.set(next_limit);
                            let api_client = api.read().clone();
                            spawn(async move {
                                match api_client.graph_with_limit(&current.repository.id, next_limit).await {
                                    Ok(items) => {
                                        commits.set(items);
                                        notice.set(format!("Loaded {next_limit} graph commits"));
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        }
                    }
                    ForkCommitDetailPanel {
                        selected: if *commit_section_mode.read() == CommitSectionMode::LocalChanges {
                            None
                        } else {
                            selected_commit.read().clone().or_else(|| commits.read().first().cloned())
                        },
                        files: git_status.read().clone(),
                        stashes: stashes.read().clone(),
                        diff: if *commit_section_mode.read() == CommitSectionMode::LocalChanges && changed_count == 0 {
                            String::new()
                        } else {
                            diff.read().clone()
                        },
                        selected_file: if *commit_section_mode.read() == CommitSectionMode::LocalChanges && changed_count == 0 {
                            String::new()
                        } else {
                            selected_file.read().clone()
                        },
                        commit_mode: *commit_section_mode.read(),
                        commit_message: commit_message.read().clone(),
                        stash_message: stash_message.read().clone(),
                        cherry_pick_input: cherry_pick_input.read().clone(),
                        rebase_base: rebase_base.read().clone(),
                        rebase_steps: rebase_steps.read().clone(),
                        tool_revision: tool_revision.read().clone(),
                        tool_branch: tool_branch.read().clone(),
                        tool_tag: tool_tag.read().clone(),
                        tool_file: tool_file.read().clone(),
                        tool_remote_name: tool_remote_name.read().clone(),
                        tool_remote_url: tool_remote_url.read().clone(),
                        tool_flow_name: tool_flow_name.read().clone(),
                        blame: blame_view.read().clone(),
                        on_blame: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before viewing blame".to_string());
                                return;
                            };
                            let path = selected_file.read().clone();
                            if path.is_empty() {
                                notice.set("Select a file to blame".to_string());
                                return;
                            }
                            let api_client = api.read().clone();
                            spawn(async move {
                                let blame = api_client.blame(&current.repository.id, &path).await;
                                let content = api_client.read_file(&current.workspace.id, &path).await;
                                match (blame, content) {
                                    (Ok(ranges), Ok(file)) => {
                                        let rows = build_blame_rows(&ranges, &file.content);
                                        notice.set(format!("Showing blame for {path}"));
                                        blame_view.set(Some(BlameView { path, rows }));
                                    }
                                    (Err(error), _) | (_, Err(error)) => notice.set(error),
                                }
                            });
                        },
                        on_close_blame: move |_| blame_view.set(None),
                        on_stage_patch: move |patch: String| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before staging a patch".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id.clone();
                            let workspace_id = current.workspace.id.clone();
                            spawn(async move {
                                match api_client.stage_patch(&repository_id, patch).await {
                                    Ok(()) => {
                                        notice.set("Hunk staged".to_string());
                                        load_workspace(
                                            api_client,
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
                        },
                        repo_stats: repo_stats.read().clone(),
                        on_load_stats: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before loading statistics".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            spawn(async move {
                                match api_client.repo_stats(&current.repository.id).await {
                                    Ok(stats) => repo_stats.set(Some(stats)),
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                        on_commit_message: move |message: String| commit_message.set(message),
                        on_commit: move |_| {
                            run_commit_action(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                commit_message.read().trim().to_string(),
                                *commit_amend.read(),
                                *commit_sign_off.read(),
                                *commit_push_after.read(),
                                commit_message,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                                toast,
                            );
                        },
                        on_stash_message: move |message: String| stash_message.set(message),
                        on_cherry_pick_input: move |value: String| cherry_pick_input.set(value),
                        on_rebase_base: move |value: String| rebase_base.set(value),
                        on_rebase_action: move |(commit, action): (String, String)| {
                            let mut next = rebase_steps.read().clone();
                            if let Some(step) = next.iter_mut().find(|step| step.commit == commit) {
                                step.action = action;
                            }
                            rebase_steps.set(next);
                        },
                        on_tool_revision: move |value: String| tool_revision.set(value),
                        on_tool_branch: move |value: String| tool_branch.set(value),
                        on_tool_tag: move |value: String| tool_tag.set(value),
                        on_tool_file: move |value: String| tool_file.set(value),
                        on_tool_remote_name: move |value: String| tool_remote_name.set(value),
                        on_tool_remote_url: move |value: String| tool_remote_url.set(value),
                        on_tool_flow_name: move |value: String| tool_flow_name.set(value),
                        on_remote_action: move |action: RemoteAction| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_remote_action(api.read().clone(), current, action, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            } else {
                                notice.set("Open a repository before remote action".to_string());
                            }
                        },
                        on_stash_action: move |action: StashAction| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, action, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            } else {
                                notice.set("Open a repository before stash action".to_string());
                            }
                        },
                        on_load_rebase: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before loading rebase plan".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            spawn(async move {
                                match api_client.rebase_plan(&current.repository.id, 20).await {
                                    Ok(plan) => {
                                        let steps = plan.into_iter().map(|commit| api::RebaseStepRequest {
                                            commit: commit.id,
                                            action: "pick".to_string(),
                                            message: None,
                                        }).collect::<Vec<_>>();
                                        rebase_steps.set(steps);
                                        notice.set("Rebase todo loaded".to_string());
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                        on_cherry_pick: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before cherry-pick".to_string());
                                return;
                            };
                            let ids = cherry_pick_input.read().split_whitespace().map(ToOwned::to_owned).collect::<Vec<_>>();
                            if ids.is_empty() {
                                notice.set("Enter commit ids to cherry-pick".to_string());
                                return;
                            }
                            run_history_action(api.read().clone(), current, HistoryAction::CherryPick(ids), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        },
                        on_cherry_abort: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_history_action(api.read().clone(), current, HistoryAction::CherryAbort, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            } else {
                                notice.set("Open a repository before cherry-pick abort".to_string());
                            }
                        },
                        on_run_rebase: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before rebase".to_string());
                                return;
                            };
                            let base = rebase_base.read().trim().to_string();
                            if base.is_empty() {
                                notice.set("Base commit is required for rebase".to_string());
                                return;
                            }
                            run_history_action(api.read().clone(), current, HistoryAction::Rebase(base, rebase_steps.read().clone()), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        },
                        on_tool_action: move |action: ToolAction| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before using Git tools".to_string());
                                return;
                            };
                            run_repository_tool(
                                api.read().clone(),
                                current,
                                action,
                                selected_file.read().clone(),
                                tool_revision.read().clone(),
                                tool_branch.read().clone(),
                                tool_tag.read().clone(),
                                tool_file.read().clone(),
                                tool_remote_name.read().clone(),
                                tool_remote_url.read().clone(),
                                tool_flow_name.read().clone(),
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_delete_repository: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before removing it".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id.clone();
                            spawn(async move {
                                match api_client.delete_repository(&repository_id).await {
                                    Ok(()) => {
                                        workspace.set(None);
                                        git_status.set(Vec::new());
                                        branches.set(Vec::new());
                                        commits.set(Vec::new());
                                        stashes.set(Vec::new());
                                        conflicts.set(Vec::new());
                                        diff.set(String::new());
                                        notice.set("Repository removed from Zync".to_string());
                                        load_repositories(api_client, repositories, notice);
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                        on_stage: move |path: String| {
                            run_file_action_from_workspace(
                                api.read().clone(),
                                workspace.read().as_ref().cloned(),
                                vec![path],
                                FileAction::Stage,
                                workspace,
                                git_status,
                                branches,
                                commits,
                                stashes,
                                conflicts,
                                diff,
                                notice,
                            );
                        },
                        on_diff: move |path: String| {
                            blame_view.set(None);
                            if git_status.read().is_empty() {
                                selected_file.set(String::new());
                                diff.set(String::new());
                                notice.set("No local changes to inspect".to_string());
                                return;
                            }
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before viewing diff".to_string());
                                return;
                            };
                            selected_file.set(path.clone());
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id;
                            spawn(async move {
                                match api_client.diff_workdir_file(&repository_id, &path).await {
                                    Ok(patch) => diff.set(patch),
                                    Err(error) => notice.set(error),
                                }
                            });
                        }
                    }
                    HistoryToolsPanel {
                        stashes: stashes.read().clone(),
                        commits: commits.read().clone(),
                        stash_message: stash_message.read().clone(),
                        cherry_pick_input: cherry_pick_input.read().clone(),
                        rebase_base: rebase_base.read().clone(),
                        rebase_steps: rebase_steps.read().clone(),
                        on_stash_message: move |message: String| stash_message.set(message),
                        on_cherry_pick_input: move |value: String| cherry_pick_input.set(value),
                        on_rebase_base: move |value: String| rebase_base.set(value),
                        on_load_rebase: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before loading rebase plan".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            spawn(async move {
                                match api_client.rebase_plan(&current.repository.id, 20).await {
                                    Ok(plan) => {
                                        let steps = plan.into_iter().map(|commit| api::RebaseStepRequest {
                                            commit: commit.id,
                                            action: "pick".to_string(),
                                            message: None,
                                        }).collect::<Vec<_>>();
                                        rebase_steps.set(steps);
                                        notice.set("Rebase todo loaded".to_string());
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                        on_rebase_action: move |(commit, action): (String, String)| {
                            let mut next = rebase_steps.read().clone();
                            if let Some(step) = next.iter_mut().find(|step| step.commit == commit) {
                                step.action = action;
                            }
                            rebase_steps.set(next);
                        },
                        on_rebase_move: move |(commit, direction): (String, i32)| {
                            let next = move_rebase_step(rebase_steps.read().clone(), &commit, direction);
                            rebase_steps.set(next);
                        },
                        on_rebase_drop: move |(dragged, target): (String, String)| {
                            let next = drop_rebase_step(rebase_steps.read().clone(), &dragged, &target);
                            rebase_steps.set(next);
                        },
                        on_create_stash: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, StashAction::Create(stash_message.read().clone()), workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            }
                        },
                        on_apply_stash: move |index: usize| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, StashAction::Apply(index), workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            }
                        },
                        on_pop_stash: move |index: usize| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, StashAction::Pop(index), workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            }
                        },
                        on_drop_stash: move |index: usize| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, StashAction::Drop(index), workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                            }
                        },
                        on_cherry_pick: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before cherry-pick".to_string());
                                return;
                            };
                            let ids = cherry_pick_input.read().split_whitespace().map(ToOwned::to_owned).collect::<Vec<_>>();
                            if ids.is_empty() {
                                notice.set("Enter commit ids to cherry-pick".to_string());
                                return;
                            }
                            run_history_action(api.read().clone(), current, HistoryAction::CherryPick(ids), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        },
                        on_cherry_abort: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_history_action(api.read().clone(), current, HistoryAction::CherryAbort, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_run_rebase: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before rebase".to_string());
                                return;
                            };
                            let base = rebase_base.read().trim().to_string();
                            if base.is_empty() {
                                notice.set("Base commit is required for interactive rebase".to_string());
                                return;
                            }
                            let steps = rebase_steps.read().clone();
                            run_history_action(api.read().clone(), current, HistoryAction::Rebase(base, steps), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        }
                    }
                    ConflictEditorPanel {
                        conflicts: conflicts.read().clone(),
                        detail: conflict_detail.read().clone(),
                        manual_content: manual_conflict_content.read().clone(),
                        on_select: move |path: String| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before conflict detail".to_string());
                                return;
                            };
                            let api_client = api.read().clone();
                            spawn(async move {
                                match api_client.conflict_detail(&current.repository.id, &path).await {
                                    Ok(detail) => {
                                        manual_conflict_content.set(detail.ours_content.clone());
                                        conflict_detail.set(detail);
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                        on_manual_change: move |content: String| manual_conflict_content.set(content),
                        on_save_manual: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before resolving conflicts".to_string());
                                return;
                            };
                            let path = conflict_detail.read().path.clone();
                            if path.is_empty() {
                                notice.set("Select a conflicted file first".to_string());
                                return;
                            }
                            let content = manual_conflict_content.read().clone();
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id.clone();
                            let workspace_id = current.workspace.id.clone();
                            spawn(async move {
                                match api_client.write_file(&workspace_id, &path, content).await {
                                    Ok(()) => {
                                        match api_client.stage_files(&repository_id, vec![path]).await {
                                            Ok(()) => {
                                                notice.set("Manual conflict resolution saved".to_string());
                                                load_workspace(
                                                    api_client,
                                                    repository_id,
                                                    workspace_id,
                                                    workspace,
                                                    git_status,
                                                    branches,
                                                    commits,
                                                    stashes,
                                                    conflicts,
                                                    diff,
                                                    notice
                                                );
                                            }
                                            Err(error) => notice.set(error),
                                        }
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        },
                        on_accept: move |(path, side): (String, String)| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before resolving conflicts".to_string());
                                return;
                            };
                            run_history_action(api.read().clone(), current, HistoryAction::Resolve(path, side), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        }
                    }
                    RepositoryToolsPanel {
                        selected_file: selected_file.read().clone(),
                        revision: tool_revision.read().clone(),
                        branch_name: tool_branch.read().clone(),
                        tag_name: tool_tag.read().clone(),
                        file_path: tool_file.read().clone(),
                        remote_name: tool_remote_name.read().clone(),
                        remote_url: tool_remote_url.read().clone(),
                        flow_name: tool_flow_name.read().clone(),
                        on_revision: move |value: String| tool_revision.set(value),
                        on_branch_name: move |value: String| tool_branch.set(value),
                        on_tag_name: move |value: String| tool_tag.set(value),
                        on_file_path: move |value: String| tool_file.set(value),
                        on_remote_name: move |value: String| tool_remote_name.set(value),
                        on_remote_url: move |value: String| tool_remote_url.set(value),
                        on_flow_name: move |value: String| tool_flow_name.set(value),
                        on_action: move |action: ToolAction| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before using repository tools".to_string());
                                return;
                            };
                            run_repository_tool(
                                api.read().clone(),
                                current,
                                action,
                                selected_file.read().clone(),
                                tool_revision.read().clone(),
                                tool_branch.read().clone(),
                                tool_tag.read().clone(),
                                tool_file.read().clone(),
                                tool_remote_name.read().clone(),
                                tool_remote_url.read().clone(),
                                tool_flow_name.read().clone(),
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
                    }
                }

                if let Some(dialog) = branch_dialog.read().clone() {
                    BranchActionDialog {
                        dialog,
                        value: branch_dialog_value.read().clone(),
                        target: branch_dialog_target.read().clone(),
                        checkout: *branch_dialog_checkout.read(),
                        local_mode: *branch_dialog_local_mode.read(),
                        has_local_changes: git_status
                            .read()
                            .iter()
                            .any(|file| file.staged || file.unstaged || file.untracked || file.conflicted),
                        rebase_steps: branch_dialog_rebase_steps.read().clone(),
                        on_value: move |value: String| branch_dialog_value.set(value),
                        on_target: move |value: String| branch_dialog_target.set(value),
                        on_checkout: move |value: bool| branch_dialog_checkout.set(value),
                        on_local_mode: move |mode: LocalChangesMode| branch_dialog_local_mode.set(mode),
                        on_rebase_action: move |(commit, action): (String, String)| {
                            let mut next = branch_dialog_rebase_steps.read().clone();
                            if let Some(step) = next.iter_mut().find(|step| step.commit == commit) {
                                step.action = action;
                            }
                            branch_dialog_rebase_steps.set(next);
                        },
                        on_reload_rebase: move |_| {
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before loading rebase todo".to_string());
                                return;
                            };
                            branch_dialog_rebase_steps.set(Vec::new());
                            load_branch_rebase_steps(
                                api.read().clone(),
                                current.repository.id,
                                branch_dialog_rebase_steps,
                                notice,
                            );
                        },
                        on_cancel: move |_| branch_dialog.set(None),
                        on_submit: move |_| {
                            let Some(dialog) = branch_dialog.read().clone() else {
                                return;
                            };
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before running branch action".to_string());
                                return;
                            };
                            let value = branch_dialog_value.read().trim().to_string();
                            let target = branch_dialog_target.read().trim().to_string();
                            let checkout = *branch_dialog_checkout.read();
                            let steps = branch_dialog_rebase_steps.read().clone();
                            branch_dialog.set(None);
                            match dialog {
                                BranchDialog::Checkout { branch } => run_branch_action(api.read().clone(), current, BranchAction::Checkout(branch), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::Merge { branch } => run_branch_action(api.read().clone(), current, BranchAction::Merge(branch), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::Delete { branch } => run_branch_action(api.read().clone(), current, BranchAction::Delete(branch), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::Rename { branch } => run_branch_action(api.read().clone(), current, BranchAction::Rename(branch, value), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::NewBranch { branch: _, target: _ } => {
                                    let changed_files = git_status
                                        .read()
                                        .iter()
                                        .filter(|file| file.staged || file.unstaged || file.untracked || file.conflicted)
                                        .map(|file| file.path.clone())
                                        .collect::<Vec<_>>();
                                    run_create_branch_action(
                                        api.read().clone(),
                                        current,
                                        value,
                                        target,
                                        checkout,
                                        *branch_dialog_local_mode.read(),
                                        changed_files,
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
                                BranchDialog::NewTag { branch: _, target: _ } => run_tag_action(api.read().clone(), current, TagAction::Create(value, target), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::Rebase { branch, .. } => run_history_action(api.read().clone(), current, HistoryAction::Rebase(branch, steps), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::RewordCommit { commit } => {
                                    if value.is_empty() {
                                        notice.set("Commit message is required".to_string());
                                    } else {
                                        match quick_rebase_plan(&commits.read(), &commit, "pick", Some(value)) {
                                            Ok((base, plan)) => run_interactive_rebase_plan(
                                                api.read().clone(), current, base, plan,
                                                format!("Reworded {}", short_id(&commit)),
                                                workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                            ),
                                            Err(error) => notice.set(error),
                                        }
                                    }
                                }
                                BranchDialog::ResetToCommit { commit } => run_commit_quick_action(
                                    api.read().clone(), current,
                                    CommitQuickAction::Reset(commit, target == "hard"),
                                    workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                ),
                                BranchDialog::DropCommit { commit } => {
                                    match quick_rebase_plan(&commits.read(), &commit, "drop", None) {
                                        Ok((base, plan)) => run_interactive_rebase_plan(
                                            api.read().clone(), current, base, plan,
                                            format!("Dropped {}", short_id(&commit)),
                                            workspace, git_status, branches, commits, stashes, conflicts, diff, notice,
                                        ),
                                        Err(error) => notice.set(error),
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(stash) = stash_apply_dialog.read().clone() {
                    StashApplyDialog {
                        stash,
                        delete_after_apply: *stash_apply_delete.read(),
                        on_delete_after_apply: move |checked: bool| stash_apply_delete.set(checked),
                        on_cancel: move |_| stash_apply_dialog.set(None),
                        on_submit: move |_| {
                            let Some(stash) = stash_apply_dialog.read().clone() else {
                                return;
                            };
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before stash action".to_string());
                                stash_apply_dialog.set(None);
                                return;
                            };
                            let action = if *stash_apply_delete.read() {
                                StashAction::Pop(stash.index)
                            } else {
                                StashAction::Apply(stash.index)
                            };
                            stash_apply_dialog.set(None);
                            run_stash_action(api.read().clone(), current, action, workspace, git_status, branches, commits, stashes, conflicts, diff, notice, toast);
                        }
                    }
                }

                footer { class: "status-bar h-7 shrink-0 border-t border-zinc-800 px-3 flex items-center gap-2 text-xs text-zinc-400 bg-zinc-950",
                    span { class: if *live_sync_ok.read() { "status-dot" } else { "status-dot status-dot-offline" } }
                    span { class: "min-w-0 truncate", "{notice}" }
                }
            }
        }
    }
}
