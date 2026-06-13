use dioxus::prelude::*;
use std::collections::HashSet;

pub mod api;

const TAILWIND_CSS: &str = include_str!("tailwind.min.css");
const APP_CSS: &str = include_str!("style.css");

#[derive(Clone, Copy, PartialEq)]
enum ResizeDragTarget {
    Sidebar,
    LeftPane,
    Inspector,
    History,
}

#[derive(Clone, Copy, PartialEq)]
enum RepoAddMode {
    Folder,
    GitUrl,
}

#[derive(Clone, PartialEq)]
enum SidebarBranchCommand {
    Checkout(String),
    Merge(String),
    Rebase(String),
    InteractiveRebase(String),
    NewBranch(String),
    NewTag(String),
    Rename(String),
    Delete(String),
    CopyName(String),
}

#[derive(Clone, PartialEq)]
enum BranchDialog {
    Checkout { branch: String },
    Merge { branch: String },
    Rebase { branch: String, interactive: bool },
    NewBranch { branch: String, target: Option<String> },
    NewTag { branch: String, target: Option<String> },
    Rename { branch: String },
    Delete { branch: String },
}

impl BranchDialog {
    fn title(&self) -> &'static str {
        match self {
            BranchDialog::Checkout { .. } => "Checkout Branch",
            BranchDialog::Merge { .. } => "Merge Branch",
            BranchDialog::Rebase {
                interactive: true, ..
            } => "Interactive Rebase",
            BranchDialog::Rebase { .. } => "Rebase Branch",
            BranchDialog::NewBranch { .. } => "New Branch",
            BranchDialog::NewTag { .. } => "New Tag",
            BranchDialog::Rename { .. } => "Rename Branch",
            BranchDialog::Delete { .. } => "Delete Branch",
        }
    }

    fn branch(&self) -> &str {
        match self {
            BranchDialog::Checkout { branch }
            | BranchDialog::Merge { branch }
            | BranchDialog::Rebase { branch, .. }
            | BranchDialog::NewBranch { branch, .. }
            | BranchDialog::NewTag { branch, .. }
            | BranchDialog::Rename { branch }
            | BranchDialog::Delete { branch } => branch,
        }
    }

    fn is_dangerous(&self) -> bool {
        matches!(self, BranchDialog::Delete { .. })
    }
}

fn clamp_pane_size(value: f64, min: u16, max: u16) -> u16 {
    value.round().clamp(f64::from(min), f64::from(max)) as u16
}

#[cfg(target_arch = "wasm32")]
fn viewport_width() -> Option<f64> {
    web_sys::window()?.inner_width().ok()?.as_f64()
}

#[cfg(not(target_arch = "wasm32"))]
fn viewport_width() -> Option<f64> {
    None
}

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
    let mut branch_dialog = use_signal(|| None::<BranchDialog>);
    let mut branch_dialog_value = use_signal(String::new);
    let mut branch_dialog_target = use_signal(String::new);
    let mut branch_dialog_checkout = use_signal(|| true);
    let mut branch_dialog_rebase_steps = use_signal(Vec::<api::RebaseStepRequest>::new);
    let mut commit_section_mode = use_signal(|| CommitSectionMode::Commits);
    let mut notice = use_signal(|| "Ready".to_string());

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
                        let api_client = api.read().clone();
                        spawn(async move {
                            match api_client.open_repository(&repository_id).await {
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
                                        notice
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
                                                notice
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
                    on_open_menu: move |name: String| sidebar_open_menu.set(Some(name)),
                    on_close_menu: move |_| sidebar_open_menu.set(None),
                    on_checkout: move |name: String| {
                        mobile_sidebar_open.set(false);
                        sidebar_open_menu.set(None);
                        if let Some(current) = workspace.read().as_ref().cloned() {
                            run_branch_action(api.read().clone(), current, BranchAction::Checkout(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        }
                    },
                    on_branch_command: move |command: SidebarBranchCommand| {
                        mobile_sidebar_open.set(false);
                        sidebar_open_menu.set(None);
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
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Fetch, workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { "Fetch" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Pull, workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { "Pull" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Push, workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { "Push" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_stash_action(api.read().clone(), current, StashAction::Create(stash_message.read().clone()), workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { "Stash" } }
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
                                run_remote_action(api.read().clone(), current, RemoteAction::Fetch, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_pull: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_remote_action(api.read().clone(), current, RemoteAction::Pull, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_push: move |_| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_remote_action(api.read().clone(), current, RemoteAction::Push, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
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

                    DiffViewer {
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
                        commits: commits.read().clone(),
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
                                run_remote_action(api.read().clone(), current, action, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            } else {
                                notice.set("Open a repository before remote action".to_string());
                            }
                        },
                        on_stash_action: move |action: StashAction| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, action, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
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
                                run_stash_action(api.read().clone(), current, StashAction::Create(stash_message.read().clone()), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_apply_stash: move |index: usize| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, StashAction::Apply(index), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_pop_stash: move |index: usize| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, StashAction::Pop(index), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                        },
                        on_drop_stash: move |index: usize| {
                            if let Some(current) = workspace.read().as_ref().cloned() {
                                run_stash_action(api.read().clone(), current, StashAction::Drop(index), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
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
                        rebase_steps: branch_dialog_rebase_steps.read().clone(),
                        on_value: move |value: String| branch_dialog_value.set(value),
                        on_target: move |value: String| branch_dialog_target.set(value),
                        on_checkout: move |value: bool| branch_dialog_checkout.set(value),
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
                                BranchDialog::NewBranch { branch: _, target: _ } => run_branch_action(api.read().clone(), current, BranchAction::CreateAt(value, target, checkout), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::NewTag { branch: _, target: _ } => run_tag_action(api.read().clone(), current, TagAction::Create(value, target), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                                BranchDialog::Rebase { branch, .. } => run_history_action(api.read().clone(), current, HistoryAction::Rebase(branch, steps), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                            }
                        }
                    }
                }

                footer { class: "h-7 shrink-0 border-t border-zinc-800 px-3 flex items-center text-xs text-zinc-400 bg-zinc-950", "{notice}" }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum FileAction {
    Stage,
    Unstage,
    Discard,
}

enum FileTreeAction {
    Create(String, bool),
    Rename(String, String),
    Delete(String),
}

enum BranchAction {
    Create(String),
    CreateAt(String, String, bool),
    Checkout(String),
    Merge(String),
    Delete(String),
    Rename(String, String),
}

enum TagAction {
    Create(String, String),
}

enum RemoteAction {
    Fetch,
    Pull,
    Push,
}

enum StashAction {
    Create(String),
    Apply(usize),
    Pop(usize),
    Drop(usize),
}

enum HistoryAction {
    CherryPick(Vec<String>),
    CherryAbort,
    Rebase(String, Vec<api::RebaseStepRequest>),
    Resolve(String, String),
}

#[derive(Clone, Copy, PartialEq)]
enum ToolAction {
    CheckoutRevision,
    BranchFromRevision,
    RevertCommit,
    CreateTag,
    DeleteTag,
    Tags,
    Blame,
    FileHistory,
    TreeAtRevision,
    Reflog,
    ResetMixed,
    ResetHard,
    Submodules,
    Lfs,
    Remotes,
    AddRemote,
    DeleteRemote,
    PruneRemote,
    DeleteRemoteBranch,
    SetUpstream,
    PushForceWithLease,
    SubmoduleInit,
    SubmoduleUpdate,
    SubmoduleSync,
    LfsInstall,
    LfsTrack,
    LfsUntrack,
    LfsPull,
    LfsPush,
    RebaseContinue,
    RebaseAbort,
    RebaseSkip,
    GitFlowDevelop,
    GitFlowFeature,
    GitFlowRelease,
    GitFlowHotfix,
    GithubLinks,
}

fn load_repositories(
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

fn load_workspace(
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
) {
    spawn(async move {
        match api.workspace(&workspace_id).await {
            Ok(next_workspace) => workspace.set(Some(next_workspace)),
            Err(error) => notice.set(error),
        }
        match api.status(&repository_id).await {
            Ok(items) => git_status.set(items),
            Err(error) => notice.set(error),
        }
        match api.branches(&repository_id).await {
            Ok(items) => branches.set(items),
            Err(error) => notice.set(error),
        }
        match api.graph(&repository_id).await {
            Ok(items) => commits.set(items),
            Err(error) => notice.set(error),
        }
        match api.stashes(&repository_id).await {
            Ok(items) => stashes.set(items),
            Err(error) => notice.set(error),
        }
        match api.conflicts(&repository_id).await {
            Ok(items) => conflicts.set(items),
            Err(error) => notice.set(error),
        }
        match api.diff_workdir(&repository_id).await {
            Ok(patch) => diff.set(patch),
            Err(error) => notice.set(error),
        }
    });
}

fn run_file_action_from_workspace(
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

fn run_file_tree_action(
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

fn run_commit_action(
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
                            if output.trim().is_empty() {
                                notice.set("Committed and pushed".to_string());
                            } else {
                                notice.set(format!("Committed and pushed: {}", output.trim()));
                            }
                        }
                        Err(error) => notice.set(format!("Committed, push failed: {error}")),
                    }
                } else {
                    notice.set("Committed".to_string());
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

fn run_branch_action(
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
            BranchAction::CreateAt(name, revision, checkout) => {
                if name.trim().is_empty() {
                    Err("Branch name is required".to_string())
                } else if revision.trim().is_empty() {
                    api.create_branch(&repository_id, &name, checkout).await
                } else {
                    api.create_branch_at(&repository_id, &name, &revision, checkout)
                        .await
                }
            }
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

fn run_tag_action(
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
                        if target.is_empty() { None } else { Some(target) },
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

fn load_branch_rebase_steps(
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
                        })
                        .collect(),
                );
                notice.set("Rebase todo loaded".to_string());
            }
            Err(error) => notice.set(error),
        }
    });
}

fn copy_to_clipboard(value: String, mut notice: Signal<String>) {
    #[cfg(target_arch = "wasm32")]
    {
        spawn(async move {
            let Some(window) = web_sys::window() else {
                notice.set(format!("Branch name: {value}"));
                return;
            };
            let clipboard = window.navigator().clipboard();
            match wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&value)).await {
                Ok(_) => notice.set(format!("Branch name copied: {value}")),
                Err(_) => notice.set(format!("Branch name: {value}")),
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    notice.set(format!("Branch name copied: {value}"));
}

fn run_remote_action(
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
                let detail = if output.trim().is_empty() {
                    format!("{label} complete")
                } else {
                    format!("{label} complete: {}", output.trim())
                };
                notice.set(detail);
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

fn run_stash_action(
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
    mut notice: Signal<String>,
) {
    let repository_id = current.repository.id;
    let workspace_id = current.workspace.id;
    spawn(async move {
        let result = match action {
            StashAction::Create(message) => api.create_stash(&repository_id, &message).await,
            StashAction::Apply(index) => api.apply_stash(&repository_id, index, false).await,
            StashAction::Pop(index) => api.apply_stash(&repository_id, index, true).await,
            StashAction::Drop(index) => api.drop_stash(&repository_id, index).await,
        };
        match result {
            Ok(()) => {
                notice.set("Stash action complete".to_string());
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

fn run_history_action(
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
fn run_repository_tool(
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

fn github_repo_url(remote_url: &str) -> Option<String> {
    let trimmed = remote_url.trim().trim_end_matches(".git");
    if let Some(path) = trimmed.strip_prefix("git@github.com:") {
        return Some(format!("https://github.com/{path}"));
    }
    if let Some(path) = trimmed.strip_prefix("ssh://git@github.com/") {
        return Some(format!("https://github.com/{path}"));
    }
    if trimmed.starts_with("https://github.com/") || trimmed.starts_with("http://github.com/") {
        return Some(trimmed.replacen("http://", "https://", 1));
    }
    None
}

fn pretty_json<T: serde::Serialize>(value: T) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

fn move_rebase_step(
    mut steps: Vec<api::RebaseStepRequest>,
    commit: &str,
    direction: i32,
) -> Vec<api::RebaseStepRequest> {
    let Some(index) = steps.iter().position(|step| step.commit == commit) else {
        return steps;
    };
    let target = if direction < 0 {
        index.saturating_sub(1)
    } else {
        (index + 1).min(steps.len().saturating_sub(1))
    };
    steps.swap(index, target);
    steps
}

fn drop_rebase_step(
    mut steps: Vec<api::RebaseStepRequest>,
    dragged: &str,
    target: &str,
) -> Vec<api::RebaseStepRequest> {
    if dragged == target {
        return steps;
    }
    let Some(from) = steps.iter().position(|step| step.commit == dragged) else {
        return steps;
    };
    let Some(to) = steps.iter().position(|step| step.commit == target) else {
        return steps;
    };
    let step = steps.remove(from);
    let insert_at = if from < to { to.saturating_sub(1) } else { to };
    steps.insert(insert_at, step);
    steps
}

#[cfg(target_arch = "wasm32")]
fn start_live_events(
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
) {
    use futures_util::StreamExt;
    use gloo_net::websocket::futures::WebSocket;

    let url = api.websocket_url(&workspace_id);
    spawn(async move {
        match WebSocket::open(&url) {
            Ok(mut socket) => {
                notice.set("Live sync connected".to_string());
                while let Some(message) = socket.next().await {
                    match message {
                        Ok(_) => load_workspace(
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
                        ),
                        Err(error) => {
                            notice.set(format!("Live sync disconnected: {error}"));
                            break;
                        }
                    }
                }
            }
            Err(error) => notice.set(format!("Live sync unavailable: {error}")),
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn start_live_events(
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
) {
}

#[component]
fn ForkSidebarNavigation(
    branches: Vec<api::BranchSummary>,
    stashes: Vec<api::StashSummary>,
    open_menu: Option<String>,
    on_open_menu: EventHandler<String>,
    on_close_menu: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_branch_command: EventHandler<SidebarBranchCommand>,
) -> Element {
    let has_stashes = !stashes.is_empty();
    let locals = branches
        .iter()
        .filter(|branch| branch.kind == "local")
        .cloned()
        .collect::<Vec<_>>();
    let remotes = branches
        .iter()
        .filter(|branch| branch.kind != "local")
        .cloned()
        .collect::<Vec<_>>();

    rsx! {
        section { class: "fork-nav-tree min-h-0 flex-1 overflow-y-auto",
            div { class: "fork-sidebar-search",
                input { class: "fork-filter-input", placeholder: "Filter" }
            }
            ForkSidebarSection {
                title: "Branches".to_string(),
                rows: locals,
                open_menu: open_menu.clone(),
                on_open_menu,
                on_close_menu,
                on_checkout,
                on_branch_command
            }
            ForkRemoteSection {
                title: "Remotes".to_string(),
                rows: remotes,
                open_menu,
                on_open_menu,
                on_close_menu,
                on_checkout,
                on_branch_command
            }
            section { class: "fork-sidebar-section",
                div { class: "fork-section-title",
                    span { "Tags" }
                }
                div { class: "fork-sidebar-row fork-sidebar-leaf fork-sidebar-muted-row",
                    span { class: "min-w-0 truncate", "No tags loaded" }
                }
            }
            section { class: "fork-sidebar-section",
                div { class: "fork-section-title",
                    span { "Stashes" }
                }
                for stash in stashes.clone() {
                    div { class: "fork-sidebar-row fork-sidebar-leaf",
                        span { class: "min-w-0 truncate", if stash.message.is_empty() { "#{stash.index} {stash.name}" } else { "{stash.message}" } }
                    }
                }
                if !has_stashes {
                    div { class: "fork-sidebar-empty", "No stashes" }
                }
            }
            section { class: "fork-sidebar-section",
                div { class: "fork-section-title",
                    span { "Submodules" }
                }
                div { class: "fork-sidebar-empty", "No submodules loaded" }
            }
        }
    }
}

fn branch_group_rows(rows: Vec<api::BranchSummary>) -> Vec<(String, Vec<api::BranchSummary>)> {
    let mut grouped = Vec::<(String, Vec<api::BranchSummary>)>::new();
    for branch in rows {
        let group_name = branch
            .name
            .split_once('/')
            .map(|(group, _)| group.to_string())
            .unwrap_or_default();
        if group_name.is_empty() {
            grouped.push((String::new(), vec![branch]));
            continue;
        }
        if let Some((_, items)) = grouped.iter_mut().find(|(name, _)| name == &group_name) {
            items.push(branch);
        } else {
            grouped.push((group_name, vec![branch]));
        }
    }
    grouped
}

fn branch_leaf_label(branch: &api::BranchSummary, group: &str) -> String {
    if group.is_empty() {
        branch.name.clone()
    } else {
        branch
            .name
            .strip_prefix(&format!("{group}/"))
            .unwrap_or(&branch.name)
            .to_string()
    }
}

#[component]
fn ForkSidebarSection(
    title: String,
    rows: Vec<api::BranchSummary>,
    open_menu: Option<String>,
    on_open_menu: EventHandler<String>,
    on_close_menu: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_branch_command: EventHandler<SidebarBranchCommand>,
) -> Element {
    let grouped = branch_group_rows(rows);
    rsx! {
        section { class: "fork-sidebar-section",
            div { class: "fork-section-title",
                span { "{title}" }
            }
            for (group, branches) in grouped {
                if group.is_empty() {
                    for branch in branches {
                        ForkSidebarBranchRow {
                            branch: branch.clone(),
                            label: String::new(),
                            indent: false,
                            menu_open: open_menu.as_ref() == Some(&branch.name),
                            on_open_menu,
                            on_close_menu,
                            on_checkout,
                            on_branch_command
                        }
                    }
                } else {
                    div { class: "fork-sidebar-row fork-sidebar-group-row",
                        span { class: "min-w-0 truncate", "{group}" }
                    }
                    for branch in branches {
                        ForkSidebarBranchRow {
                            branch: branch.clone(),
                            label: branch_leaf_label(&branch, &group),
                            indent: true,
                            menu_open: open_menu.as_ref() == Some(&branch.name),
                            on_open_menu,
                            on_close_menu,
                            on_checkout,
                            on_branch_command
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ForkRemoteSection(
    title: String,
    rows: Vec<api::BranchSummary>,
    open_menu: Option<String>,
    on_open_menu: EventHandler<String>,
    on_close_menu: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_branch_command: EventHandler<SidebarBranchCommand>,
) -> Element {
    let grouped = branch_group_rows(rows);
    rsx! {
        section { class: "fork-sidebar-section",
            div { class: "fork-section-title",
                span { "{title}" }
            }
            for (remote, branches) in grouped {
                if remote.is_empty() {
                    for branch in branches {
                        ForkSidebarBranchRow {
                            branch: branch.clone(),
                            label: String::new(),
                            indent: false,
                            menu_open: open_menu.as_ref() == Some(&branch.name),
                            on_open_menu,
                            on_close_menu,
                            on_checkout,
                            on_branch_command
                        }
                    }
                } else {
                    div { class: "fork-sidebar-row fork-sidebar-group-row",
                        span { class: "min-w-0 truncate", "{remote}" }
                    }
                    for branch in branches {
                        ForkSidebarBranchRow {
                            branch: branch.clone(),
                            label: branch_leaf_label(&branch, &remote),
                            indent: true,
                            menu_open: open_menu.as_ref() == Some(&branch.name),
                            on_open_menu,
                            on_close_menu,
                            on_checkout,
                            on_branch_command
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ForkSidebarBranchRow(
    branch: api::BranchSummary,
    label: String,
    indent: bool,
    menu_open: bool,
    on_open_menu: EventHandler<String>,
    on_close_menu: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_branch_command: EventHandler<SidebarBranchCommand>,
) -> Element {
    let display = if label.is_empty() {
        branch.name.clone()
    } else {
        label
    };
    let row_class = if branch.is_head {
        "fork-sidebar-row fork-sidebar-row-active fork-sidebar-leaf"
    } else if indent {
        "fork-sidebar-row fork-sidebar-leaf fork-sidebar-row-indent"
    } else {
        "fork-sidebar-row fork-sidebar-leaf"
    };
    let branch_for_context = branch.name.clone();
    let branch_for_click = branch.name.clone();
    let branch_for_more = branch.name.clone();
    rsx! {
        div { class: "fork-sidebar-row-wrap",
            div {
                class: "{row_class}",
                prevent_default: "oncontextmenu",
                oncontextmenu: move |_| {
                    on_open_menu.call(branch_for_context.clone());
                },
                onclick: move |_| on_checkout.call(branch_for_click.clone()),
                span { class: "min-w-0 truncate", "{display}" }
                if branch.is_head {
                    span { class: "fork-row-badge", "1↑" }
                }
                button {
                    class: "fork-row-more",
                    title: "Branch actions",
                    onclick: move |event| {
                        event.stop_propagation();
                        on_open_menu.call(branch_for_more.clone());
                    },
                    span { "More" }
                }
            }
            if menu_open {
                ForkBranchContextMenu {
                    branch: branch.name.clone(),
                    is_head: branch.is_head,
                    on_close: on_close_menu,
                    on_command: on_branch_command
                }
            }
        }
    }
}

#[component]
fn ForkBranchContextMenu(
    branch: String,
    is_head: bool,
    on_close: EventHandler<()>,
    on_command: EventHandler<SidebarBranchCommand>,
) -> Element {
    let mut drag_start_y = use_signal(|| None::<f64>);
    let mut drag_offset = use_signal(|| 0.0_f64);
    let sheet_style = format!("--sheet-drag-y: {}px;", (*drag_offset.read()).min(180.0));

    rsx! {
        button {
            class: "fork-context-scrim",
            title: "Close menu",
            onclick: move |_| on_close.call(())
        }
        div {
            class: "fork-context-menu",
            style: "{sheet_style}",
            onpointerdown: move |event| {
                drag_start_y.set(Some(event.client_coordinates().y));
                drag_offset.set(0.0);
            },
            onpointermove: move |event| {
                let Some(start_y) = *drag_start_y.read() else {
                    return;
                };
                let delta = event.client_coordinates().y - start_y;
                drag_offset.set(delta.max(0.0));
            },
            onpointerup: move |_| {
                if *drag_offset.read() > 86.0 {
                    on_close.call(());
                }
                drag_start_y.set(None);
                drag_offset.set(0.0);
            },
            onpointercancel: move |_| {
                drag_start_y.set(None);
                drag_offset.set(0.0);
            },
            ContextMenuItem { label: "Checkout...".to_string(), disabled: is_head, command: SidebarBranchCommand::Checkout(branch.clone()), on_command, on_close }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "Merge into 'main'...".to_string(), disabled: is_head, command: SidebarBranchCommand::Merge(branch.clone()), on_command, on_close }
            ContextMenuItem { label: format!("Rebase on '{branch}'..."), disabled: is_head, command: SidebarBranchCommand::Rebase(branch.clone()), on_command, on_close }
            ContextMenuItem { label: format!("Interactively Rebase on '{branch}'..."), disabled: is_head, command: SidebarBranchCommand::InteractiveRebase(branch.clone()), on_command, on_close }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "New Branch...".to_string(), command: SidebarBranchCommand::NewBranch(branch.clone()), on_command, on_close, shortcut: "⇧⌘B".to_string() }
            ContextMenuItem { label: "New Tag...".to_string(), command: SidebarBranchCommand::NewTag(branch.clone()), on_command, on_close, shortcut: "⇧⌘T".to_string() }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "Rename...".to_string(), disabled: is_head, command: SidebarBranchCommand::Rename(branch.clone()), on_command, on_close }
            ContextMenuItem { label: "Delete...".to_string(), disabled: is_head, command: SidebarBranchCommand::Delete(branch.clone()), on_command, on_close }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "Copy Branch Name".to_string(), command: SidebarBranchCommand::CopyName(branch), on_command, on_close }
        }
    }
}

#[component]
fn BranchActionDialog(
    dialog: BranchDialog,
    value: String,
    target: String,
    checkout: bool,
    rebase_steps: Vec<api::RebaseStepRequest>,
    on_value: EventHandler<String>,
    on_target: EventHandler<String>,
    on_checkout: EventHandler<bool>,
    on_rebase_action: EventHandler<(String, String)>,
    on_reload_rebase: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_submit: EventHandler<()>,
) -> Element {
    let branch = dialog.branch().to_string();
    let title = dialog.title();
    let submit_label = match &dialog {
        BranchDialog::Checkout { .. } => "Checkout",
        BranchDialog::Merge { .. } => "Merge",
        BranchDialog::Rebase {
            interactive: true, ..
        } => "Run Interactive Rebase",
        BranchDialog::Rebase { .. } => "Run Rebase",
        BranchDialog::NewBranch { .. } => "Create Branch",
        BranchDialog::NewTag { .. } => "Create Tag",
        BranchDialog::Rename { .. } => "Rename",
        BranchDialog::Delete { .. } => "Delete",
    };
    let submit_class = if dialog.is_dangerous() {
        "branch-dialog-primary branch-dialog-danger"
    } else {
        "branch-dialog-primary"
    };

    rsx! {
        div { class: "branch-dialog-layer",
            button {
                class: "branch-dialog-scrim",
                title: "Close dialog",
                onclick: move |_| on_cancel.call(())
            }
            section { class: "branch-dialog",
                header { class: "branch-dialog-header",
                    div { class: "min-w-0",
                        h3 { "{title}" }
                        p { class: "truncate", "{branch}" }
                    }
                    button {
                        class: "branch-dialog-close",
                        title: "Close",
                        onclick: move |_| on_cancel.call(()),
                        "x"
                    }
                }

                div { class: "branch-dialog-body",
                    match dialog.clone() {
                        BranchDialog::Checkout { .. } => rsx! {
                            p { "Switch working copy to this branch." }
                            code { class: "branch-dialog-code", "{branch}" }
                        },
                        BranchDialog::Merge { .. } => rsx! {
                            p { "Merge this branch into the current branch." }
                            code { class: "branch-dialog-code", "{branch}" }
                        },
                        BranchDialog::Delete { .. } => rsx! {
                            p { "Delete this local branch. This cannot be undone from Zync." }
                            code { class: "branch-dialog-code", "{branch}" }
                        },
                        BranchDialog::Rename { .. } => rsx! {
                            label { class: "branch-dialog-field",
                                span { "New branch name" }
                                input {
                                    value: "{value}",
                                    oninput: move |event| on_value.call(event.value())
                                }
                            }
                        },
                        BranchDialog::NewBranch { target: base_target, .. } => rsx! {
                            label { class: "branch-dialog-field",
                                span { "Branch name" }
                                input {
                                    placeholder: "feature/name",
                                    value: "{value}",
                                    oninput: move |event| on_value.call(event.value())
                                }
                            }
                            label { class: "branch-dialog-field",
                                span { "Start point" }
                                input {
                                    placeholder: base_target.unwrap_or_else(|| branch.clone()),
                                    value: "{target}",
                                    oninput: move |event| on_target.call(event.value())
                                }
                            }
                            label { class: "branch-dialog-check",
                                input {
                                    r#type: "checkbox",
                                    checked: checkout,
                                    onchange: move |event| on_checkout.call(event.checked())
                                }
                                span { "Checkout after create" }
                            }
                        },
                        BranchDialog::NewTag { target: tag_target, .. } => rsx! {
                            label { class: "branch-dialog-field",
                                span { "Tag name" }
                                input {
                                    placeholder: "v1.0.0",
                                    value: "{value}",
                                    oninput: move |event| on_value.call(event.value())
                                }
                            }
                            label { class: "branch-dialog-field",
                                span { "Target" }
                                input {
                                    placeholder: tag_target.unwrap_or_else(|| branch.clone()),
                                    value: "{target}",
                                    oninput: move |event| on_target.call(event.value())
                                }
                            }
                        },
                        BranchDialog::Rebase { interactive, .. } => rsx! {
                            p {
                                if interactive {
                                    "Edit the todo then rebase the current branch on this branch."
                                } else {
                                    "Rebase the current branch on this branch using the loaded todo."
                                }
                            }
                            div { class: "branch-dialog-rebase-head",
                                code { "{branch}" }
                                button {
                                    class: "branch-dialog-secondary",
                                    onclick: move |_| on_reload_rebase.call(()),
                                    "Reload todo"
                                }
                            }
                            div { class: "branch-dialog-rebase-list",
                                if rebase_steps.is_empty() {
                                    p { class: "branch-dialog-muted", "No todo loaded yet." }
                                }
                                for step in rebase_steps.clone() {
                                    div { class: "branch-dialog-rebase-row",
                                        code { "{short_id(&step.commit)}" }
                                        if interactive {
                                            div { class: "branch-dialog-action-pills",
                                                for action in ["pick", "squash", "fixup", "drop", "edit"] {
                                                    button {
                                                        class: if step.action == action { "branch-dialog-pill branch-dialog-pill-active" } else { "branch-dialog-pill" },
                                                        onclick: {
                                                            let commit = step.commit.clone();
                                                            move |_| on_rebase_action.call((commit.clone(), action.to_string()))
                                                        },
                                                        "{action}"
                                                    }
                                                }
                                            }
                                        } else {
                                            span { class: "branch-dialog-muted", "{step.action}" }
                                        }
                                    }
                                }
                            }
                        },
                    }
                }

                footer { class: "branch-dialog-footer",
                    button {
                        class: "branch-dialog-secondary",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "{submit_class}",
                        onclick: move |_| on_submit.call(()),
                        "{submit_label}"
                    }
                }
            }
        }
    }
}

#[component]
fn ContextMenuItem(
    label: String,
    command: SidebarBranchCommand,
    on_command: EventHandler<SidebarBranchCommand>,
    on_close: EventHandler<()>,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] active: bool,
    #[props(default = false)] chevron: bool,
    #[props(default)] shortcut: String,
) -> Element {
    let class_name = if active {
        "fork-context-item fork-context-item-active"
    } else {
        "fork-context-item"
    };
    rsx! {
        button {
            class: "{class_name}",
            disabled,
            onclick: move |_| {
                on_command.call(command.clone());
                on_close.call(());
            },
            span { class: "min-w-0 truncate", "{label}" }
            if !shortcut.is_empty() {
                span { class: "fork-context-shortcut", "{shortcut}" }
            } else if chevron {
                span { class: "fork-context-chevron", "More" }
            }
        }
    }
}

#[component]
fn RepositorySelector(
    repositories: Vec<api::RepositoryRecord>,
    selected_repository_id: String,
    current_branch: String,
    on_open: EventHandler<String>,
) -> Element {
    let selected_repository = repositories
        .iter()
        .find(|repository| repository.id == selected_repository_id)
        .cloned();
    let selected_path = selected_repository
        .as_ref()
        .map(|repository| repository.path.as_str())
        .unwrap_or("No repository selected");
    let selected_repository_id_for_change = selected_repository_id.clone();

    rsx! {
        section { class: "fork-repository-selector shrink-0 border-b border-zinc-800",
            label { class: "fork-repository-label", "Repository" }
            div { class: "fork-repository-select-wrap",
                select {
                    class: "fork-repository-select",
                    value: "{selected_repository_id}",
                    onchange: move |event| {
                        let repository_id = event.value();
                        if !repository_id.is_empty()
                            && repository_id != selected_repository_id_for_change
                        {
                            on_open.call(repository_id);
                        }
                    },
                    option { value: "", disabled: true, selected: selected_repository_id.is_empty(), "Select repository" }
                    for repository in repositories {
                        option {
                            value: "{repository.id}",
                            selected: repository.id == selected_repository_id,
                            "{repository.name}"
                        }
                    }
                }
            }
            p { class: "fork-repository-path", "{selected_path}" }
            p { class: "fork-repository-branch",
                span { "Current branch" }
                strong { "{current_branch}" }
            }
        }
    }
}

#[component]
fn FileExplorer(
    files: Vec<api::FileNode>,
    selected: String,
    on_select: EventHandler<String>,
    on_create: EventHandler<(String, bool)>,
    on_rename: EventHandler<(String, String)>,
    on_delete: EventHandler<String>,
    on_search: EventHandler<String>,
) -> Element {
    let mut search = use_signal(String::new);
    let mut draft_path = use_signal(String::new);
    let mut rename_path = use_signal(|| selected.clone());
    let rename_selected = selected.clone();
    let delete_selected = selected.clone();
    let has_selection = !selected.is_empty();
    rsx! {
        article { class: "file-explorer-panel min-h-[260px] md:min-h-[320px] xl:min-h-0 xl:col-start-1 xl:row-start-2 xl:row-span-2 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "shrink-0 border-b border-zinc-800 px-2 py-2 space-y-2",
                h3 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-400", "Files" }
                input {
                    class: "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500",
                    placeholder: "search files",
                    value: "{search}",
                    oninput: move |event| {
                        let value = event.value();
                        search.set(value.clone());
                        on_search.call(value);
                    }
                }
                div { class: "flex gap-2",
                    input {
                        class: "min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500",
                        placeholder: "path/to/file.rs",
                        value: "{draft_path}",
                        oninput: move |event| draft_path.set(event.value())
                    }
                    button { class: "rounded-md border border-cyan-700/60 px-2 py-1.5 text-xs text-cyan-200 hover:bg-cyan-500/10", onclick: move |_| on_create.call((draft_path.read().trim().to_string(), false)), "File" }
                    button { class: "rounded-md border border-zinc-700 px-2 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800", onclick: move |_| on_create.call((draft_path.read().trim().to_string(), true)), "Dir" }
                }
                div { class: "flex gap-2",
                    input {
                        class: "min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500",
                        placeholder: "rename selected to",
                        value: "{rename_path}",
                        oninput: move |event| rename_path.set(event.value())
                    }
                    button {
                        class: "rounded-md border border-amber-800/70 px-2 py-1.5 text-xs text-amber-200 hover:bg-amber-500/10 disabled:opacity-40",
                        disabled: !has_selection,
                        onclick: move |_| on_rename.call((rename_selected.clone(), rename_path.read().trim().to_string())),
                        "Rename"
                    }
                    button {
                        class: "rounded-md border border-red-800/70 px-2 py-1.5 text-xs text-red-200 hover:bg-red-500/10 disabled:opacity-40",
                        disabled: !has_selection,
                        onclick: move |_| on_delete.call(delete_selected.clone()),
                        "Delete"
                    }
                }
            }
            ul { class: "min-h-0 flex-1 overflow-y-auto p-2 space-y-1",
                for file in files.into_iter().take(500) {
                    li {
                        button {
                            class: if file.path == selected { "w-full rounded-md bg-cyan-500/15 px-2 py-1.5 text-left text-xs text-cyan-200 border border-cyan-500/30 truncate" } else { "w-full rounded-md px-2 py-1.5 text-left text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 truncate" },
                            disabled: file.is_dir,
                            onclick: move |_| {
                                rename_path.set(file.path.clone());
                                if !file.is_dir {
                                    on_select.call(file.path.clone());
                                }
                            },
                            if file.is_dir { "[dir] {file.path}" } else { "{file.path}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PaneStepSplitter(
    label: String,
    class_name: String,
    on_decrease: EventHandler<()>,
    on_increase: EventHandler<()>,
    on_drag_start: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "{class_name}",
            onpointerdown: move |_| on_drag_start.call(()),
            button { title: "Shrink {label}", onclick: move |_| on_decrease.call(()), "-" }
            span { "{label}" }
            button { title: "Grow {label}", onclick: move |_| on_increase.call(()), "+" }
        }
    }
}

#[component]
fn PaneGridSplitters(
    on_left_decrease: EventHandler<()>,
    on_left_increase: EventHandler<()>,
    on_left_drag_start: EventHandler<()>,
    on_right_decrease: EventHandler<()>,
    on_right_increase: EventHandler<()>,
    on_right_drag_start: EventHandler<()>,
    on_history_decrease: EventHandler<()>,
    on_history_increase: EventHandler<()>,
    on_history_drag_start: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "grid-splitter grid-splitter-left",
            onpointerdown: move |_| on_left_drag_start.call(()),
            button { title: "Narrow left pane", onclick: move |_| on_left_decrease.call(()), "-" }
            button { title: "Widen left pane", onclick: move |_| on_left_increase.call(()), "+" }
        }
        div {
            class: "grid-splitter grid-splitter-right",
            onpointerdown: move |_| on_right_drag_start.call(()),
            button { title: "Narrow inspector", onclick: move |_| on_right_decrease.call(()), "-" }
            button { title: "Widen inspector", onclick: move |_| on_right_increase.call(()), "+" }
        }
        div {
            class: "grid-splitter grid-splitter-history",
            onpointerdown: move |_| on_history_drag_start.call(()),
            button { title: "Shorter history", onclick: move |_| on_history_decrease.call(()), "-" }
            button { title: "Taller history", onclick: move |_| on_history_increase.call(()), "+" }
        }
    }
}

#[component]
fn PaneSizeControls(
    sidebar_width: u16,
    left_pane_width: u16,
    inspector_width: u16,
    history_height: u16,
    on_sidebar: EventHandler<u16>,
    on_left_pane: EventHandler<u16>,
    on_inspector: EventHandler<u16>,
    on_history: EventHandler<u16>,
    on_reset: EventHandler<()>,
) -> Element {
    rsx! {
        details { class: "pane-size-controls",
            summary { "Layout" }
            div { class: "pane-size-popover",
                PaneSlider {
                    label: "Sidebar".to_string(),
                    value: sidebar_width,
                    min: 220,
                    max: 420,
                    on_change: on_sidebar
                }
                PaneSlider {
                    label: "Left".to_string(),
                    value: left_pane_width,
                    min: 220,
                    max: 420,
                    on_change: on_left_pane
                }
                PaneSlider {
                    label: "Inspector".to_string(),
                    value: inspector_width,
                    min: 320,
                    max: 560,
                    on_change: on_inspector
                }
                PaneSlider {
                    label: "History".to_string(),
                    value: history_height,
                    min: 240,
                    max: 520,
                    on_change: on_history
                }
                button { class: "pane-reset-button", onclick: move |_| on_reset.call(()), "Reset layout" }
            }
        }
    }
}

#[component]
fn PaneSlider(
    label: String,
    value: u16,
    min: u16,
    max: u16,
    on_change: EventHandler<u16>,
) -> Element {
    rsx! {
        label { class: "pane-slider",
            span { "{label}" }
            input {
                r#type: "range",
                min: "{min}",
                max: "{max}",
                value: "{value}",
                oninput: move |event| {
                    if let Ok(value) = event.value().parse::<u16>() {
                        on_change.call(value);
                    }
                }
            }
            output { "{value}px" }
        }
    }
}

#[component]
fn WorkspaceToolbar(
    disabled: bool,
    on_refresh: EventHandler<()>,
    on_fetch: EventHandler<()>,
    on_pull: EventHandler<()>,
    on_push: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "flex w-full flex-wrap gap-1 xl:w-auto",
            button { class: "rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-40", disabled, onclick: move |_| on_fetch.call(()), "Fetch" }
            button { class: "rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-40", disabled, onclick: move |_| on_pull.call(()), "Pull" }
            button { class: "rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-200 hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-40", disabled, onclick: move |_| on_push.call(()), "Push" }
            button { class: "rounded border border-cyan-700/60 bg-cyan-500/10 px-2 py-1 text-xs text-cyan-200 hover:bg-cyan-500/20 disabled:cursor-not-allowed disabled:opacity-40", disabled, onclick: move |_| on_refresh.call(()), "Refresh" }
        }
    }
}

#[component]
fn EditorPanel(
    path: String,
    content: String,
    on_change: EventHandler<String>,
    on_save: EventHandler<()>,
) -> Element {
    rsx! {
        article { class: "editor-panel min-h-[420px] md:min-h-[520px] xl:min-h-0 xl:col-start-3 xl:row-start-3 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "shrink-0 border-b border-zinc-800 px-3 py-2 flex items-center justify-between gap-3",
                h3 { class: "min-w-0 truncate text-xs font-semibold uppercase tracking-wide text-zinc-400", if path.is_empty() { "File Preview" } else { "{path}" } }
                button { class: "rounded bg-cyan-500 px-2 py-1 text-xs font-medium text-zinc-950 hover:bg-cyan-400", onclick: move |_| on_save.call(()), "Save" }
            }
            textarea {
                class: "min-h-0 flex-1 resize-none bg-zinc-950/70 p-3 font-mono text-xs leading-5 text-zinc-100 outline-none placeholder:text-zinc-600",
                value: "{content}",
                placeholder: "Select a file",
                oninput: move |event| on_change.call(event.value())
            }
        }
    }
}

#[component]
fn GitStatusPanel(
    files: Vec<api::FileStatus>,
    on_stage_all: EventHandler<Vec<String>>,
    on_stage: EventHandler<String>,
    on_unstage_all: EventHandler<Vec<String>>,
    on_unstage: EventHandler<String>,
    on_discard: EventHandler<String>,
    on_diff: EventHandler<String>,
) -> Element {
    let staged = files
        .iter()
        .filter(|file| file.staged)
        .cloned()
        .collect::<Vec<_>>();
    let unstaged = files
        .iter()
        .filter(|file| file.unstaged || file.untracked || file.conflicted)
        .cloned()
        .collect::<Vec<_>>();

    rsx! {
        article { class: "working-copy-panel min-h-[320px] md:min-h-[420px] xl:min-h-0 xl:col-start-3 xl:row-start-1 bg-zinc-950 flex flex-col overflow-hidden",
            h3 { class: "h-9 shrink-0 border-b border-zinc-800 px-3 flex items-center text-xs font-semibold uppercase tracking-wide text-zinc-400", "Working Copy" }
            div { class: "min-h-0 flex-1 overflow-y-auto p-2 space-y-3",
            StatusGroup {
                title: "Staged".to_string(),
                files: staged,
                primary_label: "Unstage".to_string(),
                bulk_label: "Unstage all".to_string(),
                on_bulk: on_unstage_all,
                on_primary: on_unstage,
                on_discard,
                on_diff
            }
            StatusGroup {
                title: "Unstaged".to_string(),
                files: unstaged,
                primary_label: "Stage".to_string(),
                bulk_label: "Stage all".to_string(),
                on_bulk: on_stage_all,
                on_primary: on_stage,
                on_discard,
                on_diff
            }
            }
        }
    }
}

#[component]
fn StatusGroup(
    title: String,
    files: Vec<api::FileStatus>,
    primary_label: String,
    bulk_label: String,
    on_bulk: EventHandler<Vec<String>>,
    on_primary: EventHandler<String>,
    on_discard: EventHandler<String>,
    on_diff: EventHandler<String>,
) -> Element {
    let bulk_paths = files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    rsx! {
        section { class: "space-y-1.5",
            div { class: "flex items-center justify-between gap-2",
                h4 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-500", "{title}" }
                button {
                    class: "rounded border border-zinc-700 px-1.5 py-0.5 text-[11px] text-zinc-300 hover:bg-zinc-800 disabled:opacity-40",
                    disabled: bulk_paths.is_empty(),
                    onclick: move |_| on_bulk.call(bulk_paths.clone()),
                    "{bulk_label}"
                }
            }
            for file in files {
                StatusRow {
                    path: file.path,
                    primary_label: primary_label.clone(),
                    on_primary,
                    on_discard,
                    on_diff
                }
            }
        }
    }
}

#[component]
fn StatusRow(
    path: String,
    primary_label: String,
    on_primary: EventHandler<String>,
    on_discard: EventHandler<String>,
    on_diff: EventHandler<String>,
) -> Element {
    let primary_path = path.clone();
    let discard_path = path.clone();
    let diff_path = path.clone();
    rsx! {
        div { class: "grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border-b border-zinc-900 py-1.5",
            code { class: "min-w-0 truncate text-xs text-zinc-300", "{path}" }
            div { class: "flex shrink-0 gap-1",
                button { class: "rounded border border-zinc-700 px-1.5 py-0.5 text-[11px] text-zinc-200 hover:bg-zinc-800", onclick: move |_| on_diff.call(diff_path.clone()), "Diff" }
                button { class: "rounded border border-cyan-700/60 px-1.5 py-0.5 text-[11px] text-cyan-200 hover:bg-cyan-500/10", onclick: move |_| on_primary.call(primary_path.clone()), "{primary_label}" }
                button { class: "rounded border border-red-800/70 px-1.5 py-0.5 text-[11px] text-red-200 hover:bg-red-500/10", onclick: move |_| on_discard.call(discard_path.clone()), "Discard" }
            }
        }
    }
}

#[component]
fn DiffViewer(
    diff: String,
    image_path: String,
    image_before_url: String,
    image_after_url: String,
    on_stage_patch: EventHandler<String>,
) -> Element {
    let mut selected_lines = use_signal(HashSet::<String>::new);
    let hunks = diff_hunks(&diff);
    let split_lines = split_diff_lines(&hunks);
    let stage_all_patch = diff.clone();
    let show_image_diff =
        is_image_path(&image_path) && !image_before_url.is_empty() && !image_after_url.is_empty();
    rsx! {
        article { class: "diff-viewer-panel min-h-[320px] md:min-h-[420px] xl:min-h-0 xl:col-start-2 xl:row-start-2 xl:row-span-2 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "shrink-0 border-b border-zinc-800 px-3 py-2 flex items-center justify-between gap-2",
                h3 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-400", "Side-by-side Diff / Partial Staging" }
                button {
                    class: "rounded-md border border-cyan-700/60 px-2 py-1 text-[11px] text-cyan-200 hover:bg-cyan-500/10 disabled:opacity-40",
                    disabled: !diff_is_patch(&stage_all_patch),
                    onclick: move |_| on_stage_patch.call(stage_all_patch.clone()),
                    "Stage patch"
                }
            }
            div { class: "min-h-0 flex-1 overflow-auto bg-zinc-950/70 p-3 space-y-3",
                if show_image_diff {
                    ImageDiffPreview {
                        path: image_path.clone(),
                        before_url: image_before_url.clone(),
                        after_url: image_after_url.clone(),
                    }
                }
                if !split_lines.is_empty() {
                    section { class: "rounded-md border border-zinc-800 bg-zinc-950/80 overflow-hidden",
                        div { class: "grid grid-cols-2 border-b border-zinc-800 text-[11px] font-semibold uppercase tracking-wide text-zinc-500",
                            span { class: "px-2 py-1.5", "Old" }
                            span { class: "border-l border-zinc-800 px-2 py-1.5", "New" }
                        }
                        div { class: "max-h-72 overflow-auto font-mono text-xs leading-5",
                            for line in split_lines {
                                div { class: "grid grid-cols-2",
                                    pre { class: format!("min-w-0 whitespace-pre-wrap break-words px-2 {}", line.old_class), "{line.old}" }
                                    pre { class: format!("min-w-0 whitespace-pre-wrap break-words border-l border-zinc-800 px-2 {}", line.new_class), "{line.new}" }
                                }
                            }
                        }
                    }
                }
                if hunks.is_empty() {
                    pre { class: "font-mono text-xs leading-5 text-zinc-300 whitespace-pre-wrap", "{diff}" }
                } else {
                    for hunk in hunks.clone() {
                        {
                            let selected_for_hunk = hunk
                                .lines
                                .iter()
                                .filter(|line| selected_lines.read().contains(&line.key))
                                .map(|line| line.index)
                                .collect::<HashSet<_>>();
                            let selected_patch = selected_patch_for_hunk(&hunk, &selected_for_hunk);
                            rsx! {
                        article { class: "rounded-md border border-zinc-800 bg-zinc-950/80 overflow-hidden",
                            div { class: "flex items-center justify-between gap-2 border-b border-zinc-800 px-2 py-1.5",
                                code { class: "min-w-0 truncate text-[11px] text-zinc-400", "{hunk.title}" }
                                div { class: "flex shrink-0 gap-1.5",
                                    button {
                                        class: "rounded-md border border-zinc-700 px-2 py-1 text-[11px] text-zinc-300 hover:bg-zinc-800 disabled:opacity-40",
                                        disabled: selected_patch.is_none(),
                                        onclick: move |_| {
                                            if let Some(patch) = selected_patch.clone() {
                                                on_stage_patch.call(patch);
                                            }
                                        },
                                        "Stage selected"
                                    }
                                    button {
                                        class: "rounded-md border border-cyan-700/60 px-2 py-1 text-[11px] text-cyan-200 hover:bg-cyan-500/10",
                                        onclick: move |_| on_stage_patch.call(hunk.patch.clone()),
                                        "Stage hunk"
                                    }
                                }
                            }
                            div { class: "max-h-72 overflow-auto p-2 font-mono text-xs leading-5",
                                for line in hunk.lines.clone() {
                                    {
                                        let selected = selected_lines.read().contains(&line.key);
                                        rsx! {
                                    DiffLineRow {
                                        line,
                                        selected,
                                        on_toggle: move |key: String| {
                                            let mut next = selected_lines.read().clone();
                                            if !next.insert(key.clone()) {
                                                next.remove(&key);
                                            }
                                            selected_lines.set(next);
                                        }
                                    }
                                        }
                                    }
                                }
                            }
                        }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ImageDiffPreview(path: String, before_url: String, after_url: String) -> Element {
    rsx! {
        section { class: "rounded-md border border-zinc-800 bg-zinc-950/80 overflow-hidden",
            div { class: "border-b border-zinc-800 px-2 py-1.5",
                h4 { class: "text-xs font-semibold text-zinc-300", "Image Diff" }
                p { class: "mt-0.5 break-all text-[11px] text-zinc-500", "{path}" }
            }
            div { class: "grid grid-cols-1 md:grid-cols-2",
                div { class: "min-w-0 p-2",
                    div { class: "mb-1 text-[11px] font-semibold uppercase tracking-wide text-zinc-500", "HEAD" }
                    img { class: "max-h-80 w-full rounded border border-zinc-800 object-contain bg-zinc-900", src: "{before_url}", alt: "HEAD image" }
                }
                div { class: "min-w-0 border-t border-zinc-800 p-2 md:border-l md:border-t-0",
                    div { class: "mb-1 text-[11px] font-semibold uppercase tracking-wide text-zinc-500", "Working Tree" }
                    img { class: "max-h-80 w-full rounded border border-zinc-800 object-contain bg-zinc-900", src: "{after_url}", alt: "Working tree image" }
                }
            }
        }
    }
}

fn is_image_path(path: &str) -> bool {
    matches!(
        path.rsplit('.')
            .next()
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "apng" | "avif" | "gif" | "jpg" | "jpeg" | "png" | "svg" | "webp"
    )
}

fn status_label(file: &api::FileStatus) -> &'static str {
    if file.conflicted {
        "!"
    } else if file.untracked {
        "?"
    } else if file.staged {
        "+"
    } else if file.unstaged {
        "~"
    } else {
        "•"
    }
}

fn status_class(file: &api::FileStatus) -> &'static str {
    if file.conflicted {
        "fork-status fork-status-conflict"
    } else if file.untracked {
        "fork-status fork-status-untracked"
    } else if file.staged {
        "fork-status fork-status-added"
    } else if file.unstaged {
        "fork-status fork-status-modified"
    } else {
        "fork-status"
    }
}

#[derive(Clone, PartialEq)]
struct SplitDiffLine {
    old: String,
    new: String,
    old_class: &'static str,
    new_class: &'static str,
}

fn split_diff_lines(hunks: &[DiffHunk]) -> Vec<SplitDiffLine> {
    let mut rows = Vec::new();
    for hunk in hunks {
        rows.push(SplitDiffLine {
            old: hunk.title.clone(),
            new: hunk.title.clone(),
            old_class: "bg-cyan-500/10 text-cyan-200",
            new_class: "bg-cyan-500/10 text-cyan-200",
        });
        for line in hunk.lines.iter().skip(1) {
            if line.text.starts_with('-') && !line.text.starts_with("--- ") {
                rows.push(SplitDiffLine {
                    old: line.text.clone(),
                    new: String::new(),
                    old_class: "bg-red-500/10 text-red-200",
                    new_class: "text-zinc-700",
                });
            } else if line.text.starts_with('+') && !line.text.starts_with("+++ ") {
                rows.push(SplitDiffLine {
                    old: String::new(),
                    new: line.text.clone(),
                    old_class: "text-zinc-700",
                    new_class: "bg-emerald-500/10 text-emerald-200",
                });
            } else {
                rows.push(SplitDiffLine {
                    old: line.text.clone(),
                    new: line.text.clone(),
                    old_class: "text-zinc-400",
                    new_class: "text-zinc-400",
                });
            }
        }
    }
    rows
}

#[component]
fn DiffLineRow(line: DiffLine, selected: bool, on_toggle: EventHandler<String>) -> Element {
    let key = line.key.clone();
    rsx! {
        div { class: format!("grid grid-cols-[28px_1fr] gap-2 rounded px-1 {}", line.row_class),
            if line.selectable {
                button {
                    class: if selected { "my-0.5 h-5 rounded border border-cyan-500 bg-cyan-500 text-[10px] text-zinc-950" } else { "my-0.5 h-5 rounded border border-zinc-700 text-[10px] text-zinc-500 hover:border-cyan-500" },
                    onclick: move |_| on_toggle.call(key.clone()),
                    if selected { "x" } else { "+" }
                }
            } else {
                span {}
            }
            pre { class: "overflow-visible whitespace-pre-wrap break-words", "{line.text}" }
        }
    }
}

#[component]
fn CommitPanel(
    message: String,
    amend: bool,
    sign_off: bool,
    push_after: bool,
    on_message: EventHandler<String>,
    on_amend: EventHandler<bool>,
    on_sign_off: EventHandler<bool>,
    on_push_after: EventHandler<bool>,
    on_commit: EventHandler<()>,
) -> Element {
    rsx! {
        article { class: "commit-panel min-h-[260px] xl:min-h-0 xl:col-start-3 xl:row-start-2 bg-zinc-950 flex flex-col overflow-hidden",
            h3 { class: "h-9 shrink-0 border-b border-zinc-800 px-3 flex items-center text-xs font-semibold uppercase tracking-wide text-zinc-400", "Commit Panel" }
            textarea {
                class: "min-h-0 flex-1 resize-none bg-zinc-950/70 p-3 text-sm text-zinc-100 outline-none placeholder:text-zinc-600",
                value: "{message}",
                placeholder: "Commit message",
                oninput: move |event| on_message.call(event.value())
            }
            div { class: "border-t border-zinc-800 p-3 space-y-3",
                div { class: "grid grid-cols-1 gap-2 text-xs text-zinc-300",
                    label { class: "flex items-center gap-2",
                        input { r#type: "checkbox", checked: amend, onchange: move |event| on_amend.call(event.checked()) }
                        "Amend previous commit"
                    }
                    label { class: "flex items-center gap-2",
                        input { r#type: "checkbox", checked: sign_off, onchange: move |event| on_sign_off.call(event.checked()) }
                        "Sign off"
                    }
                    label { class: "flex items-center gap-2",
                        input { r#type: "checkbox", checked: push_after, onchange: move |event| on_push_after.call(event.checked()) }
                        "Push after commit"
                    }
                }
                button { class: "w-full rounded-md bg-emerald-500 px-3 py-2 text-sm font-medium text-zinc-950 hover:bg-emerald-400", onclick: move |_| on_commit.call(()), "Commit staged changes" }
            }
        }
    }
}

#[component]
fn BranchPanel(
    branches: Vec<api::BranchSummary>,
    new_branch_name: String,
    on_new_branch_name: EventHandler<String>,
    on_create: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_merge: EventHandler<String>,
    on_delete: EventHandler<String>,
    on_rename: EventHandler<(String, String)>,
) -> Element {
    let mut open_menu = use_signal(|| None::<String>);
    rsx! {
        article { class: "branch-panel min-h-[240px] xl:min-h-0 xl:col-start-1 xl:row-start-1 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "shrink-0 border-b border-zinc-800 px-2 py-2 space-y-2",
                h3 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-400", "Repository Navigator" }
                div { class: "flex gap-2",
                    input {
                        class: "min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500",
                        placeholder: "new branch",
                        value: "{new_branch_name}",
                        oninput: move |event| on_new_branch_name.call(event.value())
                    }
                    button {
                        class: "rounded-md border border-cyan-700/60 px-2 py-1.5 text-xs text-cyan-200 hover:bg-cyan-500/10",
                        onclick: move |_| on_create.call(()),
                        "Create"
                    }
                }
            }
            ul { class: "min-h-0 flex-1 overflow-y-auto p-2 space-y-1",
                for branch in branches {
                    BranchRow {
                        menu_open: open_menu.read().as_ref() == Some(&branch.name),
                        branch,
                        on_open_menu: move |name: String| open_menu.set(Some(name)),
                        on_close_menu: move |_| open_menu.set(None),
                        on_checkout,
                        on_merge,
                        on_delete,
                        on_rename
                    }
                }
            }
        }
    }
}

#[component]
fn BranchRow(
    menu_open: bool,
    branch: api::BranchSummary,
    on_open_menu: EventHandler<String>,
    on_close_menu: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_merge: EventHandler<String>,
    on_delete: EventHandler<String>,
    on_rename: EventHandler<(String, String)>,
) -> Element {
    let mut rename_value = use_signal(|| branch.name.clone());
    let checkout_name = branch.name.clone();
    let merge_name = branch.name.clone();
    let delete_name = branch.name.clone();
    let menu_name = branch.name.clone();
    rsx! {
        li {
            class: "relative rounded-md border border-zinc-800 bg-zinc-950/35 p-2 text-xs",
            oncontextmenu: move |_| on_open_menu.call(menu_name.clone()),
            div { class: "flex items-center justify-between gap-2",
                if branch.is_head {
                    strong { class: "truncate text-cyan-300", "{branch.name}" }
                } else {
                    span { class: "truncate text-zinc-300", "{branch.name}" }
                }
                div { class: "flex shrink-0 items-center gap-2",
                    small { class: "text-zinc-600", " {branch.kind}" }
                    button {
                        class: "rounded border border-zinc-700 px-1.5 py-0.5 text-[11px] text-zinc-300 hover:bg-zinc-800",
                        onclick: move |_| on_open_menu.call(branch.name.clone()),
                        "..."
                    }
                }
            }
            div { class: "mt-2 flex flex-wrap gap-1.5",
                button { class: "rounded border border-zinc-700 px-1.5 py-0.5 text-[11px] text-zinc-300 hover:bg-zinc-800 disabled:opacity-40", disabled: branch.is_head, onclick: move |_| on_checkout.call(checkout_name.clone()), "Checkout" }
                button { class: "rounded border border-emerald-800/70 px-1.5 py-0.5 text-[11px] text-emerald-200 hover:bg-emerald-500/10 disabled:opacity-40", disabled: branch.is_head, onclick: move |_| on_merge.call(merge_name.clone()), "Merge" }
                button { class: "rounded border border-red-800/70 px-1.5 py-0.5 text-[11px] text-red-200 hover:bg-red-500/10 disabled:opacity-40", disabled: branch.is_head, onclick: move |_| on_delete.call(delete_name.clone()), "Delete" }
            }
            if menu_open {
                BranchContextMenu {
                    branch: branch.name.clone(),
                    is_head: branch.is_head,
                    on_close: on_close_menu,
                    on_checkout,
                    on_merge,
                    on_delete,
                    rename_value: rename_value.read().clone(),
                    on_rename_value: move |value: String| rename_value.set(value),
                    on_rename
                }
            }
        }
    }
}

#[component]
fn BranchContextMenu(
    branch: String,
    is_head: bool,
    on_close: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_merge: EventHandler<String>,
    on_delete: EventHandler<String>,
    rename_value: String,
    on_rename_value: EventHandler<String>,
    on_rename: EventHandler<(String, String)>,
) -> Element {
    let checkout_name = branch.clone();
    let merge_name = branch.clone();
    let delete_name = branch.clone();
    rsx! {
        div { class: "absolute right-2 top-8 z-20 w-48 overflow-hidden rounded-md border border-zinc-700 bg-zinc-950 shadow-xl shadow-black/40",
            button { class: "block w-full px-3 py-2 text-left text-xs text-zinc-300 hover:bg-zinc-800 disabled:opacity-40", disabled: is_head, onclick: move |_| { on_checkout.call(checkout_name.clone()); on_close.call(()); }, "Checkout" }
            button { class: "block w-full px-3 py-2 text-left text-xs text-emerald-200 hover:bg-emerald-500/10 disabled:opacity-40", disabled: is_head, onclick: move |_| { on_merge.call(merge_name.clone()); on_close.call(()); }, "Merge into HEAD" }
            button { class: "block w-full px-3 py-2 text-left text-xs text-red-200 hover:bg-red-500/10 disabled:opacity-40", disabled: is_head, onclick: move |_| { on_delete.call(delete_name.clone()); on_close.call(()); }, "Delete" }
            div { class: "border-t border-zinc-800 p-2 space-y-2",
                input {
                    class: "w-full rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs text-zinc-100 outline-none focus:border-cyan-500",
                    value: "{rename_value}",
                    oninput: move |event| on_rename_value.call(event.value())
                }
                button {
                    class: "w-full rounded border border-cyan-700/60 px-2 py-1 text-left text-xs text-cyan-200 hover:bg-cyan-500/10 disabled:opacity-40",
                    disabled: is_head,
                    onclick: move |_| {
                        on_rename.call((branch.clone(), rename_value.clone()));
                        on_close.call(());
                    },
                    "Rename"
                }
            }
            button { class: "block w-full border-t border-zinc-800 px-3 py-2 text-left text-xs text-zinc-500 hover:bg-zinc-800", onclick: move |_| on_close.call(()), "Close" }
        }
    }
}

#[component]
fn CommitGraph(
    commits: Vec<api::CommitSummary>,
    files: Vec<api::FileStatus>,
    changed_count: usize,
    selected_file: String,
    selected_commit_id: String,
    mode: CommitSectionMode,
    on_local_changes: EventHandler<()>,
    on_all_commits: EventHandler<()>,
    on_select_local_file: EventHandler<String>,
    on_stage_local_file: EventHandler<String>,
    on_unstage_local_file: EventHandler<String>,
    on_select_commit: EventHandler<String>,
    on_load_more: EventHandler<()>,
) -> Element {
    let rows = graph_rows(&commits);
    rsx! {
        article { class: "commit-graph-panel min-h-[240px] xl:min-h-0 xl:col-start-2 xl:row-start-1 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "commit-section-header shrink-0 border-b border-zinc-800 px-3 flex items-center justify-between gap-2",
                div { class: "commit-section-tabs",
                    button {
                        class: commit_section_tab_class(mode, CommitSectionMode::LocalChanges),
                        onclick: move |_| on_local_changes.call(()),
                        "Local Changes ({changed_count})"
                    }
                    button {
                        class: commit_section_tab_class(mode, CommitSectionMode::Commits),
                        onclick: move |_| on_all_commits.call(()),
                        "All Commits"
                    }
                }
                if mode == CommitSectionMode::Commits {
                    button { class: "rounded border border-zinc-700 px-2 py-1 text-[11px] text-zinc-300 hover:bg-zinc-800", onclick: move |_| on_load_more.call(()), "Load more" }
                }
            }
            if mode == CommitSectionMode::LocalChanges {
                div { class: "local-changes-list min-h-0 flex-1 overflow-y-auto",
                    if files.is_empty() {
                        div { class: "local-changes-empty", "No local changes" }
                    } else {
                        div { class: "local-changes-header",
                            span { "Status" }
                            span { "File" }
                            span { "Action" }
                        }
                        for file in files {
                            div {
                                class: if file.path == selected_file { "local-change-row local-change-row-active" } else { "local-change-row" },
                                button {
                                    class: "local-change-main",
                                    onclick: {
                                        let path = file.path.clone();
                                        move |_| on_select_local_file.call(path.clone())
                                    },
                                    span { class: status_class(&file), "{status_label(&file)}" }
                                    code { class: "min-w-0 truncate", "{file.path}" }
                                }
                                div { class: "local-change-actions",
                                    if file.unstaged || file.untracked || file.conflicted {
                                        button {
                                            class: "local-change-action",
                                            onclick: {
                                                let path = file.path.clone();
                                                move |_| on_stage_local_file.call(path.clone())
                                            },
                                            "Stage"
                                        }
                                    }
                                    if file.staged {
                                        button {
                                            class: "local-change-action",
                                            onclick: {
                                                let path = file.path.clone();
                                                move |_| on_unstage_local_file.call(path.clone())
                                            },
                                            "Unstage"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "commit-list-header",
                    span { "Graph" }
                    span { "Commit" }
                    span { "Message" }
                    span { "Author" }
                }
                ol { class: "min-h-0 flex-1 overflow-y-auto",
                    for row in rows {
                        li {
                            class: if row.commit.id == selected_commit_id { "commit-list-row commit-list-row-active" } else { "commit-list-row" },
                            onclick: {
                                let commit_id = row.commit.id.clone();
                                move |_| on_select_commit.call(commit_id.clone())
                            },
                            GraphLaneStrip { row: row.clone() }
                            code { class: "commit-list-sha", "{short_id(&row.commit.id)}" }
                            span { class: "commit-list-message", "{row.commit.summary}" }
                            span { class: "commit-list-author", "{row.commit.author}" }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum CommitSectionMode {
    LocalChanges,
    Commits,
}

fn commit_section_tab_class(active: CommitSectionMode, tab: CommitSectionMode) -> &'static str {
    if active == tab {
        "commit-section-tab commit-section-tab-active"
    } else {
        "commit-section-tab"
    }
}

#[component]
fn GraphLaneStrip(row: GraphRow) -> Element {
    rsx! {
        div { class: "graph-lane-strip",
            for lane in 0..row.lane_count {
                div { class: "graph-lane",
                    if row.active_lanes.contains(&lane) {
                        div { class: "graph-lane-line", style: lane_line_style(lane) }
                    }
                    if row.merge_lanes.contains(&lane) {
                        div { class: "graph-lane-merge", style: lane_line_style(lane) }
                    }
                    if lane == row.lane {
                        span { class: "graph-lane-dot", style: lane_dot_style(lane) }
                    }
                }
            }
        }
    }
}

#[component]
fn ForkCommitDetailPanel(
    selected: Option<api::CommitSummary>,
    files: Vec<api::FileStatus>,
    stashes: Vec<api::StashSummary>,
    diff: String,
    selected_file: String,
    commit_mode: CommitSectionMode,
    commit_message: String,
    stash_message: String,
    cherry_pick_input: String,
    rebase_base: String,
    rebase_steps: Vec<api::RebaseStepRequest>,
    tool_revision: String,
    tool_branch: String,
    tool_tag: String,
    tool_file: String,
    tool_remote_name: String,
    tool_remote_url: String,
    tool_flow_name: String,
    on_commit_message: EventHandler<String>,
    on_commit: EventHandler<()>,
    on_stash_message: EventHandler<String>,
    on_cherry_pick_input: EventHandler<String>,
    on_rebase_base: EventHandler<String>,
    on_rebase_action: EventHandler<(String, String)>,
    on_tool_revision: EventHandler<String>,
    on_tool_branch: EventHandler<String>,
    on_tool_tag: EventHandler<String>,
    on_tool_file: EventHandler<String>,
    on_tool_remote_name: EventHandler<String>,
    on_tool_remote_url: EventHandler<String>,
    on_tool_flow_name: EventHandler<String>,
    on_remote_action: EventHandler<RemoteAction>,
    on_stash_action: EventHandler<StashAction>,
    on_load_rebase: EventHandler<()>,
    on_cherry_pick: EventHandler<()>,
    on_cherry_abort: EventHandler<()>,
    on_run_rebase: EventHandler<()>,
    on_tool_action: EventHandler<ToolAction>,
    on_delete_repository: EventHandler<()>,
    on_stage: EventHandler<String>,
    on_diff: EventHandler<String>,
) -> Element {
    let mut active_tab = use_signal(|| ForkDetailTab::Commit);
    let selected_tab = if commit_mode == CommitSectionMode::LocalChanges
        && *active_tab.read() == ForkDetailTab::Commit
    {
        ForkDetailTab::Changes
    } else {
        *active_tab.read()
    };
    let additions = diff
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count();
    let deletions = diff
        .lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .count();
    rsx! {
        article { class: "fork-detail-panel bg-zinc-950 flex flex-col overflow-hidden",
            div { class: "fork-detail-tabs",
                if commit_mode == CommitSectionMode::Commits {
                    button {
                        class: detail_tab_class(selected_tab, ForkDetailTab::Commit),
                        onclick: move |_| active_tab.set(ForkDetailTab::Commit),
                        "Commit"
                    }
                }
                button {
                    class: detail_tab_class(selected_tab, ForkDetailTab::Changes),
                    onclick: move |_| active_tab.set(ForkDetailTab::Changes),
                    "Changes"
                }
                button {
                    class: detail_tab_class(selected_tab, ForkDetailTab::FileTree),
                    onclick: move |_| active_tab.set(ForkDetailTab::FileTree),
                    "File Tree"
                }
                button {
                    class: detail_tab_class(selected_tab, ForkDetailTab::GitTools),
                    onclick: move |_| active_tab.set(ForkDetailTab::GitTools),
                    "Git Tools"
                }
                div { class: "fork-detail-commit-box",
                    input {
                        class: "fork-detail-commit-input",
                        value: "{commit_message}",
                        placeholder: "Commit message",
                        oninput: move |event| on_commit_message.call(event.value())
                    }
                    button {
                        class: "fork-detail-commit-button",
                        onclick: move |_| on_commit.call(()),
                        "Commit"
                    }
                }
            }
            if selected_tab == ForkDetailTab::Commit {
                div { class: "fork-detail-body",
                    if let Some(commit) = selected.clone() {
                        section { class: "fork-commit-summary",
                            div { class: "fork-person-card",
                                div { class: "fork-avatar", "{commit.author.chars().next().unwrap_or('Z')}" }
                                div { class: "min-w-0",
                                    div { class: "fork-label", "AUTHOR" }
                                    div { class: "fork-person-name", "{commit.author}" }
                                    div { class: "fork-muted", "Commit time {commit.time}" }
                                }
                            }
                            div { class: "fork-sha-card",
                                div { class: "fork-label", "SHA" }
                                code { class: "fork-sha", "{commit.id}" }
                                div { class: "fork-label mt-2", "PARENTS" }
                                div { class: "fork-parent-list",
                                    for parent in commit.parents {
                                        code { class: "fork-parent", "{short_id(&parent)}" }
                                    }
                                }
                            }
                        }
                        section { class: "fork-message-block",
                            h3 { " {commit.summary}" }
                            p { class: "fork-muted", "{additions} additions, {deletions} deletions in current diff" }
                        }
                    } else {
                        section { class: "fork-message-block",
                            h3 { "No commit selected" }
                            p { class: "fork-muted", "Open a repository and select a row in the commit graph." }
                        }
                    }
                    ForkChangedFilesList {
                        files: files.clone(),
                        selected_file: selected_file.clone(),
                        on_stage,
                        on_diff
                    }
                }
            } else if selected_tab == ForkDetailTab::Changes {
                ForkChangesTab {
                    selected,
                    files,
                    diff,
                    selected_file,
                    additions,
                    deletions,
                    on_stage,
                    on_diff
                }
            } else if selected_tab == ForkDetailTab::GitTools {
                BasicGitToolsPanel {
                    stashes,
                    selected_file,
                    stash_message,
                    cherry_pick_input,
                    rebase_base,
                    rebase_steps,
                    tool_revision,
                    tool_branch,
                    tool_tag,
                    tool_file,
                    tool_remote_name,
                    tool_remote_url,
                    tool_flow_name,
                    on_stash_message,
                    on_cherry_pick_input,
                    on_rebase_base,
                    on_rebase_action,
                    on_tool_revision,
                    on_tool_branch,
                    on_tool_tag,
                    on_tool_file,
                    on_tool_remote_name,
                    on_tool_remote_url,
                    on_tool_flow_name,
                    on_remote_action,
                    on_stash_action,
                    on_load_rebase,
                    on_cherry_pick,
                    on_cherry_abort,
                    on_run_rebase,
                    on_tool_action,
                    on_delete_repository
                }
            } else {
                div { class: "fork-detail-body fork-file-tree-tab",
                    div { class: "fork-file-tree-header",
                        span { "Changed file tree" }
                        span { class: "fork-muted", "{files.len()} item(s)" }
                    }
                    div { class: "fork-file-tree-list",
                        for entry in changed_tree_entries(&files) {
                            if entry.is_file {
                                button {
                                    class: if entry.path == selected_file { "fork-tree-entry fork-tree-entry-file fork-tree-entry-active" } else { "fork-tree-entry fork-tree-entry-file" },
                                    style: "padding-left: {entry.depth * 18 + 10}px",
                                    onclick: {
                                        let path = entry.path.clone();
                                        move |_| on_diff.call(path.clone())
                                    },
                                    span { class: "fork-tree-file-icon", "{entry.status}" }
                                    span { class: "truncate", "{entry.name}" }
                                }
                            } else {
                                div {
                                    class: "fork-tree-entry fork-tree-entry-dir",
                                    style: "padding-left: {entry.depth * 18 + 10}px",
                                    span { class: "fork-tree-folder-icon", "" }
                                    span { class: "truncate", "{entry.name}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ForkDetailTab {
    Commit,
    Changes,
    FileTree,
    GitTools,
}

fn detail_tab_class(active: ForkDetailTab, tab: ForkDetailTab) -> &'static str {
    if active == tab {
        "fork-detail-tab fork-detail-tab-active"
    } else {
        "fork-detail-tab"
    }
}

#[component]
fn BasicGitToolsPanel(
    stashes: Vec<api::StashSummary>,
    selected_file: String,
    stash_message: String,
    cherry_pick_input: String,
    rebase_base: String,
    rebase_steps: Vec<api::RebaseStepRequest>,
    tool_revision: String,
    tool_branch: String,
    tool_tag: String,
    tool_file: String,
    tool_remote_name: String,
    tool_remote_url: String,
    tool_flow_name: String,
    on_stash_message: EventHandler<String>,
    on_cherry_pick_input: EventHandler<String>,
    on_rebase_base: EventHandler<String>,
    on_rebase_action: EventHandler<(String, String)>,
    on_tool_revision: EventHandler<String>,
    on_tool_branch: EventHandler<String>,
    on_tool_tag: EventHandler<String>,
    on_tool_file: EventHandler<String>,
    on_tool_remote_name: EventHandler<String>,
    on_tool_remote_url: EventHandler<String>,
    on_tool_flow_name: EventHandler<String>,
    on_remote_action: EventHandler<RemoteAction>,
    on_stash_action: EventHandler<StashAction>,
    on_load_rebase: EventHandler<()>,
    on_cherry_pick: EventHandler<()>,
    on_cherry_abort: EventHandler<()>,
    on_run_rebase: EventHandler<()>,
    on_tool_action: EventHandler<ToolAction>,
    on_delete_repository: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "fork-detail-body basic-git-tools",
            section { class: "basic-tool-section basic-tool-section-compact",
                h3 { "Remote" }
                div { class: "basic-tool-actions",
                    button { class: "basic-tool-button", onclick: move |_| on_remote_action.call(RemoteAction::Fetch), "Fetch" }
                    button { class: "basic-tool-button", onclick: move |_| on_remote_action.call(RemoteAction::Pull), "Pull" }
                    button { class: "basic-tool-button", onclick: move |_| on_remote_action.call(RemoteAction::Push), "Push" }
                }
            }

            section { class: "basic-tool-section",
                h3 { "Revision / Tags" }
                div { class: "basic-tool-grid basic-tool-grid-3",
                    input { class: "basic-tool-input", value: "{tool_revision}", placeholder: "revision / commit id", oninput: move |event| on_tool_revision.call(event.value()) }
                    input { class: "basic-tool-input", value: "{tool_branch}", placeholder: "new branch name", oninput: move |event| on_tool_branch.call(event.value()) }
                    input { class: "basic-tool-input", value: "{tool_tag}", placeholder: "tag name", oninput: move |event| on_tool_tag.call(event.value()) }
                }
                div { class: "basic-tool-actions",
                    ToolButton { label: "Checkout Revision".to_string(), action: ToolAction::CheckoutRevision, on_action: on_tool_action }
                    ToolButton { label: "Branch from Revision".to_string(), action: ToolAction::BranchFromRevision, on_action: on_tool_action }
                    ToolButton { label: "Revert".to_string(), action: ToolAction::RevertCommit, on_action: on_tool_action }
                    ToolButton { label: "Create Tag".to_string(), action: ToolAction::CreateTag, on_action: on_tool_action }
                    ToolButton { label: "Delete Tag".to_string(), action: ToolAction::DeleteTag, on_action: on_tool_action }
                    ToolButton { label: "List Tags".to_string(), action: ToolAction::Tags, on_action: on_tool_action }
                }
            }

            section { class: "basic-tool-section",
                h3 { "Cherry-pick / Rebase" }
                div { class: "basic-tool-grid basic-tool-grid-2",
                    input { class: "basic-tool-input", value: "{cherry_pick_input}", placeholder: "commit ids to cherry-pick", oninput: move |event| on_cherry_pick_input.call(event.value()) }
                    input { class: "basic-tool-input", value: "{rebase_base}", placeholder: "rebase base", oninput: move |event| on_rebase_base.call(event.value()) }
                }
                div { class: "basic-tool-actions",
                    button { class: "basic-tool-button", onclick: move |_| on_cherry_pick.call(()), "Cherry-pick" }
                    button { class: "basic-tool-button", onclick: move |_| on_cherry_abort.call(()), "Abort Cherry-pick" }
                    button { class: "basic-tool-button", onclick: move |_| on_load_rebase.call(()), "Load Rebase Todo" }
                    button { class: "basic-tool-button", onclick: move |_| on_run_rebase.call(()), "Run Rebase" }
                    ToolButton { label: "Rebase Continue".to_string(), action: ToolAction::RebaseContinue, on_action: on_tool_action }
                    ToolButton { label: "Rebase Abort".to_string(), action: ToolAction::RebaseAbort, on_action: on_tool_action }
                    ToolButton { label: "Rebase Skip".to_string(), action: ToolAction::RebaseSkip, on_action: on_tool_action }
                }
                if !rebase_steps.is_empty() {
                    div { class: "basic-rebase-list",
                        for step in rebase_steps.clone() {
                            div { class: "basic-rebase-row",
                                code { "{short_id(&step.commit)}" }
                                div { class: "branch-dialog-action-pills",
                                    for action in ["pick", "squash", "fixup", "drop", "edit"] {
                                        button {
                                            class: if step.action == action { "branch-dialog-pill branch-dialog-pill-active" } else { "branch-dialog-pill" },
                                            onclick: {
                                                let commit = step.commit.clone();
                                                move |_| on_rebase_action.call((commit.clone(), action.to_string()))
                                            },
                                            "{action}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "basic-tool-section",
                h3 { "Stashes" }
                div { class: "basic-tool-grid basic-tool-grid-action",
                    input { class: "basic-tool-input", value: "{stash_message}", placeholder: "stash message", oninput: move |event| on_stash_message.call(event.value()) }
                    button {
                        class: "basic-tool-button",
                        onclick: {
                            let message = stash_message.clone();
                            move |_| on_stash_action.call(StashAction::Create(message.clone()))
                        },
                        "Create Stash"
                    }
                }
                div { class: "basic-stash-list",
                    if stashes.is_empty() {
                        p { class: "fork-muted", "No stashes" }
                    }
                    for stash in stashes {
                        div { class: "basic-stash-row",
                            div { class: "min-w-0",
                                strong { "#{stash.index} {stash.name}" }
                                p { class: "truncate", "{stash.message}" }
                            }
                            div { class: "basic-tool-actions basic-tool-actions-tight",
                                button { class: "basic-tool-button", onclick: move |_| on_stash_action.call(StashAction::Apply(stash.index)), "Apply" }
                                button { class: "basic-tool-button", onclick: move |_| on_stash_action.call(StashAction::Pop(stash.index)), "Pop" }
                                button { class: "basic-tool-button basic-tool-danger", onclick: move |_| on_stash_action.call(StashAction::Drop(stash.index)), "Drop" }
                            }
                        }
                    }
                }
            }

            section { class: "basic-tool-section",
                h3 { "Files / Remotes / Submodules" }
                div { class: "basic-tool-grid basic-tool-grid-3",
                    input { class: "basic-tool-input", value: "{tool_file}", placeholder: if selected_file.is_empty() { "file path" } else { "{selected_file}" }, oninput: move |event| on_tool_file.call(event.value()) }
                    input { class: "basic-tool-input", value: "{tool_remote_name}", placeholder: "remote", oninput: move |event| on_tool_remote_name.call(event.value()) }
                    input { class: "basic-tool-input", value: "{tool_remote_url}", placeholder: "remote url", oninput: move |event| on_tool_remote_url.call(event.value()) }
                }
                input { class: "basic-tool-input", value: "{tool_flow_name}", placeholder: "remote branch / upstream branch / LFS pattern", oninput: move |event| on_tool_flow_name.call(event.value()) }
                div { class: "basic-tool-actions",
                    ToolButton { label: "Blame".to_string(), action: ToolAction::Blame, on_action: on_tool_action }
                    ToolButton { label: "File History".to_string(), action: ToolAction::FileHistory, on_action: on_tool_action }
                    ToolButton { label: "List Remotes".to_string(), action: ToolAction::Remotes, on_action: on_tool_action }
                    ToolButton { label: "Add Remote".to_string(), action: ToolAction::AddRemote, on_action: on_tool_action }
                    ToolButton { label: "Delete Remote".to_string(), action: ToolAction::DeleteRemote, on_action: on_tool_action }
                    ToolButton { label: "Delete Remote Branch".to_string(), action: ToolAction::DeleteRemoteBranch, on_action: on_tool_action }
                    ToolButton { label: "Set Upstream".to_string(), action: ToolAction::SetUpstream, on_action: on_tool_action }
                    ToolButton { label: "Submodules".to_string(), action: ToolAction::Submodules, on_action: on_tool_action }
                    ToolButton { label: "Submodule Init".to_string(), action: ToolAction::SubmoduleInit, on_action: on_tool_action }
                    ToolButton { label: "Submodule Update".to_string(), action: ToolAction::SubmoduleUpdate, on_action: on_tool_action }
                    ToolButton { label: "Submodule Sync".to_string(), action: ToolAction::SubmoduleSync, on_action: on_tool_action }
                    button { class: "basic-tool-button basic-tool-danger", onclick: move |_| on_delete_repository.call(()), "Remove Repo from Zync" }
                }
            }
        }
    }
}

#[component]
fn ForkChangedFilesList(
    files: Vec<api::FileStatus>,
    selected_file: String,
    on_stage: EventHandler<String>,
    on_diff: EventHandler<String>,
) -> Element {
    rsx! {
        section { class: "fork-changed-files",
            div { class: "fork-changed-header",
                span { "Changed Files" }
                span { class: "fork-muted", "{files.len()} item(s)" }
            }
            for file in files.into_iter().take(120) {
                div { class: if file.path == selected_file { "fork-file-row fork-file-row-active" } else { "fork-file-row" },
                    button { class: "fork-file-main", onclick: {
                        let path = file.path.clone();
                        move |_| on_diff.call(path.clone())
                    },
                        span { class: status_class(&file), "{status_label(&file)}" }
                        code { "{file.path}" }
                    }
                    button { class: "fork-file-action", onclick: {
                        let path = file.path.clone();
                        move |_| on_stage.call(path.clone())
                    }, "Stage" }
                }
            }
        }
    }
}

#[component]
fn ForkChangesTab(
    selected: Option<api::CommitSummary>,
    files: Vec<api::FileStatus>,
    diff: String,
    selected_file: String,
    additions: usize,
    deletions: usize,
    on_stage: EventHandler<String>,
    on_diff: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "fork-changes-view",
            header { class: "fork-changes-commit-bar",
                div { class: "fork-avatar fork-avatar-small",
                    "{selected.as_ref().and_then(|commit| commit.author.chars().next()).unwrap_or('Z')}"
                }
                if let Some(commit) = selected {
                    strong { class: "truncate", "{commit.author}" }
                    code { "{short_id(&commit.id)}" }
                    span { class: "fork-muted", "{commit.time}" }
                    span { class: "fork-changes-summary", "{commit.summary}" }
                } else {
                    strong { "Working tree" }
                    span { class: "fork-muted", "Select a commit or file to inspect changes." }
                }
            }
            div { class: "fork-changes-grid",
                aside { class: "fork-changes-files",
                    div { class: "fork-changes-files-toolbar",
                        span { class: "fork-search-dot", "" }
                        span { class: "fork-muted", "{files.len()} files" }
                    }
                    div { class: "fork-changes-tree",
                        for entry in changed_tree_entries(&files) {
                            if entry.is_file {
                                div {
                                    class: if entry.path == selected_file { "fork-change-tree-row fork-change-tree-row-active" } else { "fork-change-tree-row" },
                                    style: "padding-left: {entry.depth * 18 + 10}px",
                                    button {
                                        class: "fork-change-tree-main",
                                        onclick: {
                                            let path = entry.path.clone();
                                            move |_| on_diff.call(path.clone())
                                        },
                                        span { class: status_class_from_label(&entry.status), "{entry.status}" }
                                        span { class: "truncate", "{entry.name}" }
                                    }
                                    button {
                                        class: "fork-change-tree-stage",
                                        title: "Stage file",
                                        onclick: {
                                            let path = entry.path.clone();
                                            move |_| on_stage.call(path.clone())
                                        },
                                        "+"
                                    }
                                }
                            } else {
                                div {
                                    class: "fork-change-tree-row fork-change-tree-dir",
                                    style: "padding-left: {entry.depth * 18 + 10}px",
                                    span { class: "fork-tree-folder-icon", "" }
                                    span { class: "truncate", "{entry.name}" }
                                }
                            }
                        }
                    }
                }
                section { class: "fork-changes-diff",
                    div { class: "fork-changes-diff-toolbar",
                        span { class: "fork-file-doc-icon", "" }
                        code { class: "truncate", if selected_file.is_empty() { "Select a file" } else { "{selected_file}" } }
                        span { class: "fork-muted", "+{additions} -{deletions}" }
                    }
                    ForkCompactDiff { diff }
                }
            }
        }
    }
}

#[component]
fn ForkCompactDiff(diff: String) -> Element {
    let hunks = diff_hunks(&diff);
    rsx! {
        div { class: "fork-compact-diff",
            if diff.trim().is_empty() {
                div { class: "fork-diff-empty", "Select a changed file to show its diff." }
            } else if hunks.is_empty() {
                pre { class: "fork-compact-diff-raw", "{diff}" }
            } else {
                for hunk in hunks {
                    article { class: "fork-compact-hunk",
                        div { class: "fork-compact-hunk-title", "{hunk.title}" }
                        for line in hunk.lines {
                            div { class: format!("fork-compact-line {}", compact_diff_class(line.text.as_str())),
                                span { class: "fork-compact-line-marker", "{compact_diff_marker(line.text.as_str())}" }
                                pre { "{line.text}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
struct ChangedTreeEntry {
    name: String,
    path: String,
    depth: usize,
    is_file: bool,
    status: String,
}

fn changed_tree_entries(files: &[api::FileStatus]) -> Vec<ChangedTreeEntry> {
    let mut entries = Vec::<ChangedTreeEntry>::new();
    let mut seen_dirs = HashSet::<String>::new();
    let mut sorted = files.to_vec();
    sorted.sort_by(|left, right| left.path.cmp(&right.path));

    for file in sorted {
        let parts = file.path.split('/').collect::<Vec<_>>();
        let mut prefix = String::new();
        for (index, part) in parts.iter().enumerate() {
            let is_file = index == parts.len().saturating_sub(1);
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if is_file {
                entries.push(ChangedTreeEntry {
                    name: (*part).to_string(),
                    path: file.path.clone(),
                    depth: index,
                    is_file: true,
                    status: status_label(&file).to_string(),
                });
            } else if seen_dirs.insert(prefix.clone()) {
                entries.push(ChangedTreeEntry {
                    name: (*part).to_string(),
                    path: prefix.clone(),
                    depth: index,
                    is_file: false,
                    status: String::new(),
                });
            }
        }
    }

    entries
}

fn status_class_from_label(label: &str) -> &'static str {
    match label {
        "A" => "fork-status fork-status-added",
        "U" => "fork-status fork-status-untracked",
        "!" => "fork-status fork-status-conflict",
        _ => "fork-status fork-status-modified",
    }
}

fn compact_diff_class(line: &str) -> &'static str {
    if line.starts_with('+') && !line.starts_with("+++") {
        "fork-compact-line-added"
    } else if line.starts_with('-') && !line.starts_with("---") {
        "fork-compact-line-removed"
    } else if line.starts_with("@@") {
        "fork-compact-line-hunk"
    } else {
        "fork-compact-line-context"
    }
}

fn compact_diff_marker(line: &str) -> &'static str {
    if line.starts_with('+') && !line.starts_with("+++") {
        "+"
    } else if line.starts_with('-') && !line.starts_with("---") {
        "-"
    } else {
        ""
    }
}

#[component]
fn HistoryToolsPanel(
    stashes: Vec<api::StashSummary>,
    commits: Vec<api::CommitSummary>,
    stash_message: String,
    cherry_pick_input: String,
    rebase_base: String,
    rebase_steps: Vec<api::RebaseStepRequest>,
    on_stash_message: EventHandler<String>,
    on_cherry_pick_input: EventHandler<String>,
    on_rebase_base: EventHandler<String>,
    on_load_rebase: EventHandler<()>,
    on_rebase_action: EventHandler<(String, String)>,
    on_rebase_move: EventHandler<(String, i32)>,
    on_rebase_drop: EventHandler<(String, String)>,
    on_create_stash: EventHandler<()>,
    on_apply_stash: EventHandler<usize>,
    on_pop_stash: EventHandler<usize>,
    on_drop_stash: EventHandler<usize>,
    on_cherry_pick: EventHandler<()>,
    on_cherry_abort: EventHandler<()>,
    on_run_rebase: EventHandler<()>,
) -> Element {
    let mut dragging_commit = use_signal(|| None::<String>);
    rsx! {
        article { class: "history-tools-panel min-h-[360px] xl:min-h-0 xl:col-start-1 xl:row-start-4 bg-zinc-950 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Workflow: Stash / Cherry-pick / Rebase" }
            div { class: "min-h-0 flex-1 overflow-y-auto p-3 space-y-4",
                section { class: "space-y-2",
                    div { class: "flex gap-2",
                        input {
                            class: "min-w-0 flex-1 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500",
                            value: "{stash_message}",
                            oninput: move |event| on_stash_message.call(event.value())
                        }
                        button { class: "rounded-md border border-cyan-700/60 px-2 py-1.5 text-xs text-cyan-200 hover:bg-cyan-500/10", onclick: move |_| on_create_stash.call(()), "Stash" }
                    }
                    for stash in stashes {
                        div { class: "rounded-md border border-zinc-800 bg-zinc-950/40 p-2 text-xs",
                            div { class: "truncate text-zinc-300", "#{stash.index} {stash.name}" }
                            code { class: "block truncate text-[11px] text-zinc-600", "{short_id(&stash.message)}" }
                            div { class: "mt-2 flex flex-wrap gap-1.5",
                                button { class: "rounded border border-zinc-700 px-1.5 py-0.5 text-[11px] text-zinc-300 hover:bg-zinc-800", onclick: move |_| on_apply_stash.call(stash.index), "Apply" }
                                button { class: "rounded border border-emerald-800/70 px-1.5 py-0.5 text-[11px] text-emerald-200 hover:bg-emerald-500/10", onclick: move |_| on_pop_stash.call(stash.index), "Pop" }
                                button { class: "rounded border border-red-800/70 px-1.5 py-0.5 text-[11px] text-red-200 hover:bg-red-500/10", onclick: move |_| on_drop_stash.call(stash.index), "Drop" }
                            }
                        }
                    }
                }

                section { class: "space-y-2 border-t border-zinc-800 pt-3",
                    h4 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-500", "Cherry-pick" }
                    textarea {
                        class: "h-16 w-full resize-none rounded-md border border-zinc-700 bg-zinc-950 p-2 font-mono text-xs text-zinc-100 outline-none focus:border-cyan-500",
                        placeholder: "commit ids separated by space",
                        value: "{cherry_pick_input}",
                        oninput: move |event| on_cherry_pick_input.call(event.value())
                    }
                    div { class: "flex gap-2",
                        button { class: "flex-1 rounded-md border border-emerald-800/70 px-2 py-1.5 text-xs text-emerald-200 hover:bg-emerald-500/10", onclick: move |_| on_cherry_pick.call(()), "Cherry-pick" }
                        button { class: "rounded-md border border-red-800/70 px-2 py-1.5 text-xs text-red-200 hover:bg-red-500/10", onclick: move |_| on_cherry_abort.call(()), "Abort" }
                    }
                }

                section { class: "space-y-2 border-t border-zinc-800 pt-3",
                    h4 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-500", "Interactive Rebase" }
                    div { class: "grid grid-cols-1 sm:grid-cols-[1fr_auto] gap-2",
                        input {
                            class: "min-w-0 rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 font-mono text-xs text-zinc-100 outline-none focus:border-cyan-500",
                            placeholder: "base commit",
                            value: "{rebase_base}",
                            oninput: move |event| on_rebase_base.call(event.value())
                        }
                        button { class: "rounded-md border border-zinc-700 px-2 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800", onclick: move |_| on_load_rebase.call(()), "Load todo" }
                    }
                    div { class: "space-y-1",
                        for step in rebase_steps.clone() {
                            RebaseStepRow {
                                step,
                                dragging: dragging_commit.read().clone(),
                                on_drag_start: move |commit: String| dragging_commit.set(Some(commit)),
                                on_drop_commit: move |target: String| {
                                    if let Some(dragged) = dragging_commit.read().clone() {
                                        on_rebase_drop.call((dragged, target));
                                    }
                                    dragging_commit.set(None);
                                },
                                on_rebase_action,
                                on_rebase_move
                            }
                        }
                    }
                    if !commits.is_empty() && rebase_steps.is_empty() {
                        p { class: "text-xs text-zinc-500", "Load todo to prepare the latest commits." }
                    }
                    button { class: "w-full rounded-md bg-cyan-500 px-3 py-2 text-sm font-medium text-zinc-950 hover:bg-cyan-400", onclick: move |_| on_run_rebase.call(()), "Run rebase todo" }
                }
            }
        }
    }
}

#[component]
fn RebaseStepRow(
    step: api::RebaseStepRequest,
    dragging: Option<String>,
    on_drag_start: EventHandler<String>,
    on_drop_commit: EventHandler<String>,
    on_rebase_action: EventHandler<(String, String)>,
    on_rebase_move: EventHandler<(String, i32)>,
) -> Element {
    let commit_for_drag = step.commit.clone();
    let commit_for_drop = step.commit.clone();
    let move_up_commit = step.commit.clone();
    let move_down_commit = step.commit.clone();
    let is_drop_target = dragging
        .as_ref()
        .map(|commit| commit != &step.commit)
        .unwrap_or(false);
    rsx! {
        div {
            class: if is_drop_target { "grid grid-cols-[86px_1fr] gap-2 rounded-md border border-cyan-500/50 bg-cyan-500/10 p-2 text-xs" } else { "grid grid-cols-[86px_1fr] gap-2 rounded-md border border-zinc-800 bg-zinc-950/40 p-2 text-xs" },
            draggable: "true",
            "data-commit": "{step.commit}",
            ondragstart: move |_| on_drag_start.call(commit_for_drag.clone()),
            ondragover: move |_| {},
            ondrop: move |_| on_drop_commit.call(commit_for_drop.clone()),
            div { class: "flex items-center gap-1",
                div { class: "flex flex-col gap-1",
                    button { class: "h-4 rounded border border-zinc-700 px-1 text-[10px] text-zinc-400 hover:bg-zinc-800", onclick: move |_| on_rebase_move.call((move_up_commit.clone(), -1)), "Up" }
                    button { class: "h-4 rounded border border-zinc-700 px-1 text-[10px] text-zinc-400 hover:bg-zinc-800", onclick: move |_| on_rebase_move.call((move_down_commit.clone(), 1)), "Dn" }
                }
            code { class: "text-cyan-300", "{short_id(&step.commit)}" }
            }
            div { class: "flex flex-wrap gap-1.5",
                for action in ["pick", "squash", "fixup", "drop", "edit"] {
                    button {
                        class: if step.action == action { "rounded bg-cyan-500 px-1.5 py-0.5 text-[11px] text-zinc-950" } else { "rounded border border-zinc-700 px-1.5 py-0.5 text-[11px] text-zinc-300 hover:bg-zinc-800" },
                        onclick: {
                            let commit = step.commit.clone();
                            move |_| on_rebase_action.call((commit.clone(), action.to_string()))
                        },
                        "{action}"
                    }
                }
            }
        }
    }
}

#[component]
fn ConflictEditorPanel(
    conflicts: Vec<api::ConflictSummary>,
    detail: api::ConflictDetail,
    manual_content: String,
    on_select: EventHandler<String>,
    on_manual_change: EventHandler<String>,
    on_save_manual: EventHandler<()>,
    on_accept: EventHandler<(String, String)>,
) -> Element {
    let selected_path = detail.path.clone();
    let accept_local_path = detail.path.clone();
    let accept_remote_path = detail.path.clone();
    let accept_both_content = format!(
        "{}\n{}",
        detail.ours_content.trim_end(),
        detail.theirs_content.trim_start()
    );
    rsx! {
        article { class: "conflict-editor-panel min-h-[360px] xl:min-h-0 xl:col-start-2 xl:row-start-4 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "shrink-0 border-b border-zinc-800 px-3 py-2 flex items-center justify-between gap-2",
                h3 { class: "text-sm font-semibold", "3-way Conflict Editor" }
                span { class: "text-[11px] text-zinc-500", "{conflicts.len()} conflict(s)" }
            }
            div { class: "min-h-0 flex-1 grid grid-cols-1 lg:grid-cols-[220px_1fr] overflow-hidden",
                aside { class: "border-b lg:border-b-0 lg:border-r border-zinc-800 p-2 overflow-y-auto",
                    for conflict in conflicts {
                        if let Some(path) = conflict.ours.clone().or(conflict.theirs.clone()).or(conflict.ancestor.clone()) {
                            button {
                                class: if path == selected_path { "mb-1 w-full rounded-md border border-cyan-500/40 bg-cyan-500/10 px-2 py-1.5 text-left text-xs text-cyan-200 truncate" } else { "mb-1 w-full rounded-md px-2 py-1.5 text-left text-xs text-zinc-400 hover:bg-zinc-800 truncate" },
                                onclick: move |_| on_select.call(path.clone()),
                                "{path}"
                            }
                        }
                    }
                }
                section { class: "min-h-0 overflow-y-auto p-3 space-y-3",
                    if detail.path.is_empty() {
                        p { class: "text-sm text-zinc-500", "Select a conflicted file." }
                    } else {
                        div { class: "flex flex-col sm:flex-row sm:items-center justify-between gap-2",
                            code { class: "min-w-0 truncate text-xs text-cyan-300", "{detail.path}" }
                            div { class: "flex gap-2",
                                button { class: "rounded-md border border-emerald-800/70 px-2 py-1 text-xs text-emerald-200 hover:bg-emerald-500/10", onclick: move |_| on_accept.call((accept_local_path.clone(), "local".to_string())), "Accept Local" }
                                button { class: "rounded-md border border-amber-800/70 px-2 py-1 text-xs text-amber-200 hover:bg-amber-500/10", onclick: move |_| on_accept.call((accept_remote_path.clone(), "remote".to_string())), "Accept Remote" }
                                button { class: "rounded-md border border-cyan-700/70 px-2 py-1 text-xs text-cyan-200 hover:bg-cyan-500/10", onclick: move |_| on_manual_change.call(accept_both_content.clone()), "Accept Both" }
                            }
                        }
                        div { class: "grid grid-cols-1 xl:grid-cols-3 gap-3",
                            ConflictPane { title: "LOCAL".to_string(), path: detail.ours_path.clone().unwrap_or_default(), content: detail.ours_content.clone() }
                            ConflictPane { title: "BASE".to_string(), path: detail.ancestor_path.clone().unwrap_or_default(), content: detail.ancestor_content.clone() }
                            ConflictPane { title: "REMOTE".to_string(), path: detail.theirs_path.clone().unwrap_or_default(), content: detail.theirs_content.clone() }
                        }
                        section { class: "rounded-md border border-cyan-900/70 bg-cyan-950/20 flex flex-col overflow-hidden",
                            div { class: "flex items-center justify-between gap-2 border-b border-cyan-900/60 px-2 py-1.5",
                                h4 { class: "text-xs font-semibold text-cyan-200", "MANUAL MERGE" }
                                button {
                                    class: "rounded-md bg-cyan-500 px-2 py-1 text-xs font-medium text-zinc-950 hover:bg-cyan-400",
                                    onclick: move |_| on_save_manual.call(()),
                                    "Save + Mark Resolved"
                                }
                            }
                            textarea {
                                class: "min-h-[220px] resize-y bg-zinc-950/70 p-2 font-mono text-xs leading-5 text-zinc-100 outline-none",
                                value: "{manual_content}",
                                oninput: move |event| on_manual_change.call(event.value())
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ConflictPane(title: String, path: String, content: String) -> Element {
    rsx! {
        section { class: "min-h-[220px] rounded-md border border-zinc-800 bg-zinc-950/60 flex flex-col overflow-hidden",
            div { class: "border-b border-zinc-800 px-2 py-1.5",
                h4 { class: "text-xs font-semibold text-zinc-300", "{title}" }
                code { class: "block truncate text-[11px] text-zinc-600", "{path}" }
            }
            textarea {
                class: "min-h-0 flex-1 resize-none bg-transparent p-2 font-mono text-xs leading-5 text-zinc-200 outline-none",
                readonly: true,
                value: "{content}"
            }
        }
    }
}

#[component]
fn RepositoryToolsPanel(
    selected_file: String,
    revision: String,
    branch_name: String,
    tag_name: String,
    file_path: String,
    remote_name: String,
    remote_url: String,
    flow_name: String,
    on_revision: EventHandler<String>,
    on_branch_name: EventHandler<String>,
    on_tag_name: EventHandler<String>,
    on_file_path: EventHandler<String>,
    on_remote_name: EventHandler<String>,
    on_remote_url: EventHandler<String>,
    on_flow_name: EventHandler<String>,
    on_action: EventHandler<ToolAction>,
) -> Element {
    rsx! {
        article { class: "repository-tools-panel min-h-[420px] xl:min-h-0 xl:col-start-3 xl:row-start-4 bg-zinc-950 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Repository Tools" }
            div { class: "min-h-0 flex-1 overflow-y-auto p-3",
                div { class: "space-y-4",
                    section { class: "space-y-2 rounded-md border border-zinc-800 bg-zinc-950/35 p-3",
                        h4 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-500", "Revision / Tags" }
                        div { class: "grid grid-cols-1 sm:grid-cols-3 gap-2",
                            input { class: "rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 font-mono text-xs text-zinc-100 outline-none focus:border-cyan-500", value: "{revision}", placeholder: "revision", oninput: move |event| on_revision.call(event.value()) }
                            input { class: "rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500", value: "{branch_name}", placeholder: "branch from revision", oninput: move |event| on_branch_name.call(event.value()) }
                            input { class: "rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500", value: "{tag_name}", placeholder: "tag name", oninput: move |event| on_tag_name.call(event.value()) }
                        }
                        div { class: "flex flex-wrap gap-2",
                            ToolButton { label: "Checkout Rev".to_string(), action: ToolAction::CheckoutRevision, on_action }
                            ToolButton { label: "Branch From Rev".to_string(), action: ToolAction::BranchFromRevision, on_action }
                            ToolButton { label: "Revert".to_string(), action: ToolAction::RevertCommit, on_action }
                            ToolButton { label: "Create Tag".to_string(), action: ToolAction::CreateTag, on_action }
                            ToolButton { label: "Delete Tag".to_string(), action: ToolAction::DeleteTag, on_action }
                            ToolButton { label: "List Tags".to_string(), action: ToolAction::Tags, on_action }
                        }
                    }

                    section { class: "space-y-2 rounded-md border border-zinc-800 bg-zinc-950/35 p-3",
                        h4 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-500", "History / Browse" }
                        input { class: "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500", value: "{file_path}", placeholder: if selected_file.is_empty() { "file path" } else { "{selected_file}" }, oninput: move |event| on_file_path.call(event.value()) }
                        div { class: "flex flex-wrap gap-2",
                            ToolButton { label: "Blame".to_string(), action: ToolAction::Blame, on_action }
                            ToolButton { label: "File History".to_string(), action: ToolAction::FileHistory, on_action }
                            ToolButton { label: "Tree at Rev".to_string(), action: ToolAction::TreeAtRevision, on_action }
                            ToolButton { label: "Reflog".to_string(), action: ToolAction::Reflog, on_action }
                            ToolButton { label: "Reset Mixed".to_string(), action: ToolAction::ResetMixed, on_action }
                            ToolButton { label: "Reset Hard".to_string(), action: ToolAction::ResetHard, on_action }
                        }
                    }

                    section { class: "space-y-2 rounded-md border border-zinc-800 bg-zinc-950/35 p-3",
                        h4 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-500", "Remotes / Submodules / LFS / Git-flow" }
                        div { class: "grid grid-cols-1 sm:grid-cols-3 gap-2",
                            input { class: "rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500", value: "{remote_name}", placeholder: "remote", oninput: move |event| on_remote_name.call(event.value()) }
                            input { class: "rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500 sm:col-span-2", value: "{remote_url}", placeholder: "remote url", oninput: move |event| on_remote_url.call(event.value()) }
                        }
                        input { class: "w-full rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 outline-none focus:border-cyan-500", value: "{flow_name}", placeholder: "branch name / LFS pattern / git-flow name", oninput: move |event| on_flow_name.call(event.value()) }
                        div { class: "flex flex-wrap gap-2",
                            ToolButton { label: "List Remotes".to_string(), action: ToolAction::Remotes, on_action }
                            ToolButton { label: "Add Remote".to_string(), action: ToolAction::AddRemote, on_action }
                            ToolButton { label: "Delete Remote".to_string(), action: ToolAction::DeleteRemote, on_action }
                            ToolButton { label: "Prune Remote".to_string(), action: ToolAction::PruneRemote, on_action }
                            ToolButton { label: "Set Upstream".to_string(), action: ToolAction::SetUpstream, on_action }
                            ToolButton { label: "Delete Remote Branch".to_string(), action: ToolAction::DeleteRemoteBranch, on_action }
                            ToolButton { label: "Force Lease Push".to_string(), action: ToolAction::PushForceWithLease, on_action }
                            ToolButton { label: "GitHub Links".to_string(), action: ToolAction::GithubLinks, on_action }
                            ToolButton { label: "Submodules".to_string(), action: ToolAction::Submodules, on_action }
                            ToolButton { label: "Submodule Init".to_string(), action: ToolAction::SubmoduleInit, on_action }
                            ToolButton { label: "Submodule Update".to_string(), action: ToolAction::SubmoduleUpdate, on_action }
                            ToolButton { label: "Submodule Sync".to_string(), action: ToolAction::SubmoduleSync, on_action }
                            ToolButton { label: "LFS".to_string(), action: ToolAction::Lfs, on_action }
                            ToolButton { label: "LFS Install".to_string(), action: ToolAction::LfsInstall, on_action }
                            ToolButton { label: "LFS Track".to_string(), action: ToolAction::LfsTrack, on_action }
                            ToolButton { label: "LFS Untrack".to_string(), action: ToolAction::LfsUntrack, on_action }
                            ToolButton { label: "LFS Pull".to_string(), action: ToolAction::LfsPull, on_action }
                            ToolButton { label: "LFS Push".to_string(), action: ToolAction::LfsPush, on_action }
                            ToolButton { label: "Rebase Continue".to_string(), action: ToolAction::RebaseContinue, on_action }
                            ToolButton { label: "Rebase Abort".to_string(), action: ToolAction::RebaseAbort, on_action }
                            ToolButton { label: "Rebase Skip".to_string(), action: ToolAction::RebaseSkip, on_action }
                            ToolButton { label: "Develop".to_string(), action: ToolAction::GitFlowDevelop, on_action }
                            ToolButton { label: "Feature".to_string(), action: ToolAction::GitFlowFeature, on_action }
                            ToolButton { label: "Release".to_string(), action: ToolAction::GitFlowRelease, on_action }
                            ToolButton { label: "Hotfix".to_string(), action: ToolAction::GitFlowHotfix, on_action }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ToolButton(label: String, action: ToolAction, on_action: EventHandler<ToolAction>) -> Element {
    rsx! {
        button {
            class: "rounded-md border border-zinc-700 px-2 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800",
            onclick: move |_| on_action.call(action),
            "{label}"
        }
    }
}

#[derive(Clone, PartialEq)]
struct GraphRow {
    commit: api::CommitSummary,
    lane: usize,
    lane_count: usize,
    active_lanes: HashSet<usize>,
    merge_lanes: HashSet<usize>,
}

fn graph_rows(commits: &[api::CommitSummary]) -> Vec<GraphRow> {
    let mut lanes = Vec::<Option<String>>::new();
    let mut rows = Vec::new();

    for commit in commits {
        let lane = lanes
            .iter()
            .position(|id| id.as_ref() == Some(&commit.id))
            .unwrap_or_else(|| {
                let next = lanes
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(lanes.len());
                if next == lanes.len() {
                    lanes.push(None);
                }
                next
            });

        let active_lanes = lanes
            .iter()
            .enumerate()
            .filter_map(|(index, id)| id.as_ref().map(|_| index))
            .chain(std::iter::once(lane))
            .collect::<HashSet<_>>();
        let merge_lanes = if commit.parents.len() > 1 {
            (lane..(lane + commit.parents.len()).min(lanes.len().max(lane + 1))).collect()
        } else {
            HashSet::new()
        };

        rows.push(GraphRow {
            commit: commit.clone(),
            lane,
            lane_count: lanes.len().max(lane + commit.parents.len()).max(1),
            active_lanes,
            merge_lanes,
        });

        if let Some(first_parent) = commit.parents.first() {
            lanes[lane] = Some(first_parent.clone());
        } else {
            lanes[lane] = None;
        }

        for parent in commit.parents.iter().skip(1) {
            let target = lanes
                .iter()
                .position(Option::is_none)
                .unwrap_or(lanes.len());
            if target == lanes.len() {
                lanes.push(Some(parent.clone()));
            } else {
                lanes[target] = Some(parent.clone());
            }
        }

        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        if lanes.is_empty() {
            lanes.push(None);
        }
    }

    rows
}

fn lane_dot_style(lane: usize) -> &'static str {
    let colors = [
        "background:#2dd4bf;",
        "background:#f59e0b;",
        "background:#a78bfa;",
        "background:#fb7185;",
        "background:#38bdf8;",
        "background:#34d399;",
        "background:#f472b6;",
    ];
    colors[lane % colors.len()]
}

fn lane_line_style(lane: usize) -> &'static str {
    let colors = [
        "background:#0f766e;",
        "background:#b45309;",
        "background:#7c3aed;",
        "background:#be123c;",
        "background:#0284c7;",
        "background:#059669;",
        "background:#be185d;",
    ];
    colors[lane % colors.len()]
}

#[derive(Clone)]
struct DiffHunk {
    title: String,
    header: Vec<String>,
    old_start: usize,
    new_start: usize,
    lines: Vec<DiffLine>,
    patch: String,
}

#[derive(Clone, PartialEq)]
struct DiffLine {
    key: String,
    index: usize,
    text: String,
    selectable: bool,
    row_class: &'static str,
}

fn diff_is_patch(diff: &str) -> bool {
    diff.contains("diff --git") && diff.contains("@@")
}

fn diff_hunks(diff: &str) -> Vec<DiffHunk> {
    if !diff_is_patch(diff) {
        return Vec::new();
    }
    let mut file_header = Vec::<String>::new();
    let mut current = Vec::<String>::new();
    let mut title = String::new();
    let mut old_start = 0usize;
    let mut new_start = 0usize;
    let mut hunks = Vec::new();
    let mut hunk_index = 0usize;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if !current.is_empty() {
                hunks.push(DiffHunk {
                    title: title.clone(),
                    header: file_header.clone(),
                    old_start,
                    new_start,
                    lines: diff_lines(hunk_index, &current),
                    patch: build_patch(&file_header, &current),
                });
                hunk_index += 1;
                current.clear();
            }
            file_header.clear();
            file_header.push(line.to_string());
            title = line.to_string();
        } else if line.starts_with("@@") {
            if !current.is_empty() {
                hunks.push(DiffHunk {
                    title: title.clone(),
                    header: file_header.clone(),
                    old_start,
                    new_start,
                    lines: diff_lines(hunk_index, &current),
                    patch: build_patch(&file_header, &current),
                });
                hunk_index += 1;
                current.clear();
            }
            title = line.to_string();
            if let Some((old, new)) = parse_hunk_starts(line) {
                old_start = old;
                new_start = new;
            }
            current.push(line.to_string());
        } else if current.is_empty() {
            file_header.push(line.to_string());
        } else {
            current.push(line.to_string());
        }
    }

    if !current.is_empty() {
        hunks.push(DiffHunk {
            title,
            header: file_header.clone(),
            old_start,
            new_start,
            lines: diff_lines(hunk_index, &current),
            patch: build_patch(&file_header, &current),
        });
    }
    hunks
}

fn diff_lines(hunk_index: usize, lines: &[String]) -> Vec<DiffLine> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let selectable = index > 0
                && (line.starts_with('+') || line.starts_with('-'))
                && !line.starts_with("+++ ")
                && !line.starts_with("--- ");
            let row_class = if line.starts_with('+') && !line.starts_with("+++ ") {
                "bg-emerald-500/10 text-emerald-200"
            } else if line.starts_with('-') && !line.starts_with("--- ") {
                "bg-red-500/10 text-red-200"
            } else if line.starts_with("@@") {
                "bg-cyan-500/10 text-cyan-200"
            } else {
                "text-zinc-400"
            };
            DiffLine {
                key: format!("{hunk_index}:{index}"),
                index,
                text: line.clone(),
                selectable,
                row_class,
            }
        })
        .collect()
}

fn parse_hunk_starts(header: &str) -> Option<(usize, usize)> {
    let mut parts = header.split_whitespace();
    parts.next()?;
    let old_part = parts.next()?.trim_start_matches('-');
    let new_part = parts.next()?.trim_start_matches('+');
    Some((parse_range_start(old_part)?, parse_range_start(new_part)?))
}

fn parse_range_start(value: &str) -> Option<usize> {
    value.split(',').next()?.parse().ok()
}

fn selected_patch_for_hunk(hunk: &DiffHunk, selected: &HashSet<usize>) -> Option<String> {
    if selected.is_empty() {
        return None;
    }

    let mut body = Vec::<String>::new();
    let mut old_count = 0usize;
    let mut new_count = 0usize;
    for line in hunk.lines.iter().skip(1) {
        let is_context = line.text.starts_with(' ') || line.text.starts_with('\\');
        let is_selected = selected.contains(&line.index);
        if is_context || is_selected {
            if line.text.starts_with('+') && !line.text.starts_with("+++ ") {
                new_count += 1;
            } else if line.text.starts_with('-') && !line.text.starts_with("--- ") {
                old_count += 1;
            } else if line.text.starts_with(' ') {
                old_count += 1;
                new_count += 1;
            }
            body.push(line.text.clone());
        }
    }

    if body
        .iter()
        .all(|line| line.starts_with(' ') || line.starts_with('\\'))
    {
        return None;
    }

    let mut patch = hunk.header.join("\n");
    if !patch.is_empty() {
        patch.push('\n');
    }
    patch.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        hunk.old_start, old_count, hunk.new_start, new_count
    ));
    patch.push_str(&body.join("\n"));
    patch.push('\n');
    Some(patch)
}

fn build_patch(header: &[String], hunk: &[String]) -> String {
    let mut patch = header.join("\n");
    if !patch.is_empty() {
        patch.push('\n');
    }
    patch.push_str(&hunk.join("\n"));
    patch.push('\n');
    patch
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
