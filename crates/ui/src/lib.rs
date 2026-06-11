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
enum IconName {
    Folder,
    FolderOpen,
    GitBranch,
    Search,
    Archive,
    FileDiff,
    Check,
    GitCommit,
    ChevronRight,
    ChevronDown,
    Tag,
    Remote,
    More,
}

#[derive(Clone, PartialEq)]
enum SidebarBranchCommand {
    Checkout(String),
    FastForward(String),
    Push(String),
    Pull(String),
    CreatePullRequest(String),
    Merge(String),
    Rebase(String),
    InteractiveRebase(String),
    NewBranch(String),
    NewTag(String),
    Tracking(String),
    Rename(String),
    Delete(String),
    Ai(String),
    CopyName(String),
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
    let git_status = use_signal(Vec::<api::FileStatus>::new);
    let branches = use_signal(Vec::<api::BranchSummary>::new);
    let mut commits = use_signal(Vec::<api::CommitSummary>::new);
    let mut selected_commit = use_signal(|| None::<api::CommitSummary>);
    let stashes = use_signal(Vec::<api::StashSummary>::new);
    let conflicts = use_signal(Vec::<api::ConflictSummary>::new);
    let mut conflict_detail = use_signal(api::ConflictDetail::default);
    let mut manual_conflict_content = use_signal(String::new);
    let mut diff = use_signal(String::new);
    let mut selected_file = use_signal(String::new);
    let mut editor_content = use_signal(String::new);
    let mut repo_path = use_signal(String::new);
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
    let tool_output = use_signal(String::new);
    let mut sidebar_width = use_signal(|| 320u16);
    let mut left_pane_width = use_signal(|| 260u16);
    let mut inspector_width = use_signal(|| 380u16);
    let mut history_height = use_signal(|| 320u16);
    let mut active_resize = use_signal(|| None::<ResizeDragTarget>);
    let mut auto_opened_first_repo = use_signal(|| false);
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
    let current_workspace_id = workspace
        .read()
        .as_ref()
        .map(|item| item.workspace.id.clone())
        .unwrap_or_default();
    let websocket_url = if current_workspace_id.is_empty() {
        String::new()
    } else {
        api.read().websocket_url(&current_workspace_id)
    };
    let changed_count = git_status.read().len();
    let conflict_count = conflicts.read().len();
    let current_branch = branches
        .read()
        .iter()
        .find(|branch| branch.is_head)
        .map(|branch| branch.name.clone())
        .unwrap_or_else(|| "no branch".to_string());
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
            aside { class: "workspace-sidebar fork-sidebar w-full xl:w-[280px] xl:h-screen shrink-0 border-b xl:border-b-0 xl:border-r border-zinc-800 bg-zinc-950 flex flex-col",
                header { class: "fork-sidebar-title h-12 shrink-0 border-b border-zinc-800 px-3 flex items-center justify-between gap-3",
                    div { class: "min-w-0",
                        h1 { class: "text-sm font-semibold tracking-tight", if let Some(current) = workspace.read().as_ref() { "{current.repository.name}" } else { "Zync" } }
                        p { class: "min-w-0 truncate text-[11px] text-zinc-500", "API {api_base}" }
                    }
                    span { class: "text-zinc-500", "..." }
                }

                details { class: "fork-mount-panel shrink-0 border-b border-zinc-800 bg-zinc-900/40",
                    summary { class: "fork-mount-summary",
                        Icon { name: IconName::Folder }
                        span { "Add mounted repository" }
                    }
                    div { class: "fork-mount-body space-y-2",
                    input {
                        class: "w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                        placeholder: "Repository path mounted on server",
                        value: "{repo_path}",
                        oninput: move |event| repo_path.set(event.value())
                    }
                    input {
                        class: "w-full rounded border border-zinc-700 bg-zinc-950 px-2 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                        placeholder: "Name",
                        value: "{repo_name}",
                        oninput: move |event| repo_name.set(event.value())
                    }
                    div { class: "grid grid-cols-[1fr_auto] gap-2",
                        button {
                            class: "rounded bg-cyan-500 px-2 py-1.5 text-xs font-medium text-zinc-950 hover:bg-cyan-400 disabled:opacity-50",
                            onclick: move |_| {
                                let api_client = api.read().clone();
                                let path = repo_path.read().trim().to_string();
                                let name = repo_name.read().trim().to_string();
                                spawn(async move {
                                    if path.is_empty() {
                                        notice.set("Repository path is required".to_string());
                                        return;
                                    }
                                    let request = api::CreateRepositoryRequest {
                                        name: if name.is_empty() { None } else { Some(name) },
                                        path: Some(path),
                                        remote_url: None,
                                        clone_to: None,
                                    };
                                    match api_client.create_repository(&request).await {
                                        Ok(opened) => {
                                            notice.set("Repository added and watcher started".to_string());
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
                            "Add mounted repo"
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
                    changed_count,
                    branches: branches.read().clone(),
                    stashes: stashes.read().clone(),
                    on_local_changes: move |_| notice.set("Open Working Copy in the lower pane".to_string()),
                    on_all_commits: move |_| notice.set("Commit graph focused".to_string()),
                    on_checkout: move |name: String| {
                        if let Some(current) = workspace.read().as_ref().cloned() {
                            run_branch_action(api.read().clone(), current, BranchAction::Checkout(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                        }
                    },
                    on_branch_command: move |command: SidebarBranchCommand| {
                        let branch_name = match &command {
                            SidebarBranchCommand::Checkout(name)
                            | SidebarBranchCommand::FastForward(name)
                            | SidebarBranchCommand::Push(name)
                            | SidebarBranchCommand::Pull(name)
                            | SidebarBranchCommand::CreatePullRequest(name)
                            | SidebarBranchCommand::Merge(name)
                            | SidebarBranchCommand::Rebase(name)
                            | SidebarBranchCommand::InteractiveRebase(name)
                            | SidebarBranchCommand::NewBranch(name)
                            | SidebarBranchCommand::NewTag(name)
                            | SidebarBranchCommand::Tracking(name)
                            | SidebarBranchCommand::Rename(name)
                            | SidebarBranchCommand::Delete(name)
                            | SidebarBranchCommand::Ai(name)
                            | SidebarBranchCommand::CopyName(name) => name.clone(),
                        };

                        let Some(current) = workspace.read().as_ref().cloned() else {
                            notice.set(format!("Open a repository before using {branch_name}"));
                            return;
                        };

                        match command {
                            SidebarBranchCommand::Checkout(name) => run_branch_action(api.read().clone(), current, BranchAction::Checkout(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                            SidebarBranchCommand::Merge(name) => run_branch_action(api.read().clone(), current, BranchAction::Merge(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                            SidebarBranchCommand::Delete(name) => run_branch_action(api.read().clone(), current, BranchAction::Delete(name), workspace, git_status, branches, commits, stashes, conflicts, diff, notice),
                            SidebarBranchCommand::Push(name) => {
                                notice.set(format!("Pushing current branch; selected branch: {name}"));
                                run_remote_action(api.read().clone(), current, RemoteAction::Push, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                            SidebarBranchCommand::Pull(name) | SidebarBranchCommand::FastForward(name) => {
                                notice.set(format!("Pulling current branch; selected branch: {name}"));
                                run_remote_action(api.read().clone(), current, RemoteAction::Pull, workspace, git_status, branches, commits, stashes, conflicts, diff, notice);
                            }
                            SidebarBranchCommand::CopyName(name) => notice.set(format!("Branch name copied target: {name}")),
                            SidebarBranchCommand::CreatePullRequest(name) => notice.set(format!("Create Pull Request for {name} is not connected to a Git provider yet")),
                            SidebarBranchCommand::Rebase(name) => notice.set(format!("Open Rebase workflow and use {name} as base")),
                            SidebarBranchCommand::InteractiveRebase(name) => notice.set(format!("Open Interactive Rebase workflow for {name}")),
                            SidebarBranchCommand::NewBranch(name) => notice.set(format!("Use New Branch with base {name}")),
                            SidebarBranchCommand::NewTag(name) => notice.set(format!("Use New Tag with target {name}")),
                            SidebarBranchCommand::Tracking(name) => notice.set(format!("Tracking options for {name}")),
                            SidebarBranchCommand::Rename(name) => notice.set(format!("Open Repository Navigator to rename {name}")),
                            SidebarBranchCommand::Ai(name) => notice.set(format!("AI actions for {name} will run after AI tools are enabled")),
                        }
                    }
                }

                RepositoryList {
                    repositories: repositories.read().clone(),
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
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref() { load_workspace(api.read().clone(), current.repository.id.clone(), current.workspace.id.clone(), workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { class: "fork-toolbar-symbol fork-toolbar-symbol-folder" } span { "Quick Launch" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Fetch, workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { class: "fork-toolbar-symbol fork-toolbar-symbol-fetch" } span { "Fetch" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Pull, workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { class: "fork-toolbar-symbol fork-toolbar-symbol-pull" } span { "Pull" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_remote_action(api.read().clone(), current, RemoteAction::Push, workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { class: "fork-toolbar-symbol fork-toolbar-symbol-push" } span { "Push" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| { if let Some(current) = workspace.read().as_ref().cloned() { run_stash_action(api.read().clone(), current, StashAction::Create(stash_message.read().clone()), workspace, git_status, branches, commits, stashes, conflicts, diff, notice); } }, span { class: "fork-toolbar-symbol fork-toolbar-symbol-stash" } span { "Stash" } }
                    }
                    if let Some(current) = workspace.read().as_ref() {
                        div { class: "fork-repo-switcher min-w-0",
                            h2 { class: "text-sm font-semibold truncate", "{current.repository.name}" }
                            p { class: "text-xs text-zinc-500 truncate", "{current_branch}" }
                        }
                        div { class: "workspace-meta hidden xl:flex flex-wrap items-center justify-end gap-1 text-[11px] text-zinc-500 min-w-0",
                            span { class: "workspace-pill", "{current_branch}" }
                            span { class: "workspace-pill", "{changed_count} changes" }
                            span { class: "workspace-pill", "{conflict_count} conflicts" }
                            span { class: "truncate max-w-[420px]", "WS {websocket_url}" }
                        }
                    } else {
                        div { class: "fork-repo-switcher min-w-0",
                            h2 { class: "text-lg font-semibold", "Open a mounted Git repository" }
                            p { class: "text-xs text-zinc-500", "Mount a project into the server, add its server-side path, then open it here." }
                        }
                    }
                    div { class: "fork-toolbar-right",
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| notice.set("New Branch is available from Repository Navigator".to_string()), span { class: "fork-toolbar-symbol fork-toolbar-symbol-branch" } span { "New Branch" } }
                        button { class: "fork-toolbar-button", disabled: current_repository_id.is_empty(), onclick: move |_| notice.set("Open in server mounted path".to_string()), span { class: "fork-toolbar-symbol fork-toolbar-symbol-open" } span { "Open in" } }
                        button { class: "fork-toolbar-button", onclick: move |_| notice.set("Feedback noted".to_string()), span { class: "fork-toolbar-symbol fork-toolbar-symbol-feedback" } span { "Feedback" } }
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
                    PaneSizeControls {
                        sidebar_width: *sidebar_width.read(),
                        left_pane_width: *left_pane_width.read(),
                        inspector_width: *inspector_width.read(),
                        history_height: *history_height.read(),
                        on_sidebar: move |value: u16| sidebar_width.set(value),
                        on_left_pane: move |value: u16| left_pane_width.set(value),
                        on_inspector: move |value: u16| inspector_width.set(value),
                        on_history: move |value: u16| history_height.set(value),
                        on_reset: move |_| {
                            sidebar_width.set(320);
                            left_pane_width.set(260);
                            inspector_width.set(380);
                            history_height.set(320);
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
                            let Some(current) = workspace.read().as_ref().cloned() else {
                                notice.set("Open a repository before committing".to_string());
                                return;
                            };
                            let message = commit_message.read().trim().to_string();
                            if message.is_empty() {
                                notice.set("Commit message is required".to_string());
                                return;
                            }
                            let api_client = api.read().clone();
                            let repository_id = current.repository.id.clone();
                            let workspace_id = current.workspace.id.clone();
                            let amend = *commit_amend.read();
                            let sign_off = *commit_sign_off.read();
                            let push_after = *commit_push_after.read();
                            spawn(async move {
                                let request = api::CommitRequest {
                                    message,
                                    author_name: "Zync".to_string(),
                                    author_email: "zync@local".to_string(),
                                    amend,
                                    sign_off,
                                };
                                match api_client.commit(&repository_id, &request).await {
                                    Ok(_) => {
                                        if push_after {
                                            match api_client.push(&repository_id).await {
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
                        selected: selected_commit.read().clone().or_else(|| commits.read().first().cloned()),
                        files: git_status.read().clone(),
                        diff: diff.read().clone(),
                        selected_file: selected_file.read().clone(),
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
                        output: tool_output.read().clone(),
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
                                tool_output,
                                notice,
                            );
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
    Checkout(String),
    Merge(String),
    Delete(String),
    Rename(String, String),
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
            BranchAction::Checkout(name) => api.checkout_branch(&repository_id, &name).await,
            BranchAction::Merge(name) => api.merge_branch(&repository_id, &name).await,
            BranchAction::Delete(name) => api.delete_branch(&repository_id, &name).await,
            BranchAction::Rename(name, new_name) => {
                api.rename_branch(&repository_id, &name, &new_name).await
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
    mut output: Signal<String>,
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
                output.set(message.clone());
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
                output.set(error.clone());
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
fn Icon(name: IconName) -> Element {
    let path_data = match name {
        IconName::Folder => {
            "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"
        }
        IconName::FolderOpen => {
            "M6 14l1.45-2.9A2 2 0 0 1 9.24 10H21a1 1 0 0 1 .9 1.45l-3.1 6.2A2 2 0 0 1 17 19H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4.5a2 2 0 0 1 1.6.8L12 6h7a2 2 0 0 1 2 2v2"
        }
        IconName::GitBranch => {
            "M6 3v12 M18 6a3 3 0 1 1-6 0a3 3 0 0 1 6 0 M9 18a3 3 0 1 1-6 0a3 3 0 0 1 6 0 M18 9a9 9 0 0 1-9 9"
        }
        IconName::Search => {
            "M11 19a8 8 0 1 1 5.657-13.657A8 8 0 0 1 11 19 M21 21l-4.35-4.35"
        }
        IconName::Archive => {
            "M21 8v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8 M1 3h22v5H1z M10 12h4"
        }
        IconName::FileDiff => {
            "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6 M12 13H8 M16 17H8 M10 9H8"
        }
        IconName::Check => "M20 6L9 17l-5-5",
        IconName::GitCommit => {
            "M12 3v6 M12 15v6 M8 12a4 4 0 1 1 8 0a4 4 0 0 1-8 0 M3 12h5 M16 12h5"
        }
        IconName::ChevronRight => "M9 18l6-6l-6-6",
        IconName::ChevronDown => "M6 9l6 6l6-6",
        IconName::Tag => "M20.6 13.4l-7.2 7.2a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8Z M7.5 7.5h.01",
        IconName::Remote => "M18 18.5a3.5 3.5 0 1 0 0-7a3.5 3.5 0 0 0 0 7Z M6 12.5a3.5 3.5 0 1 0 0-7a3.5 3.5 0 0 0 0 7Z M15 14.5l-6-4 M9 7.5l6-3",
        IconName::More => "M12 8h.01 M12 12h.01 M12 16h.01",
    };

    rsx! {
        svg {
            class: "zync-icon",
            view_box: "0 0 24 24",
            path {
                d: "{path_data}",
                fill: "none",
                stroke: "currentColor",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2"
            }
        }
    }
}

#[component]
fn ForkSidebarNavigation(
    changed_count: usize,
    branches: Vec<api::BranchSummary>,
    stashes: Vec<api::StashSummary>,
    on_local_changes: EventHandler<()>,
    on_all_commits: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_branch_command: EventHandler<SidebarBranchCommand>,
) -> Element {
    let mut open_menu = use_signal(|| None::<String>);
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
            div { class: "fork-sidebar-primary",
                button { class: "fork-sidebar-row fork-sidebar-row-strong", onclick: move |_| on_local_changes.call(()),
                    span { class: "fork-row-icon", Icon { name: IconName::FileDiff } }
                    span { "Local Changes ({changed_count})" }
                }
                button { class: "fork-sidebar-row fork-sidebar-row-strong", onclick: move |_| on_all_commits.call(()),
                    span { class: "fork-row-icon", Icon { name: IconName::GitCommit } }
                    span { "All Commits" }
                }
            }
            div { class: "fork-sidebar-view-tabs",
                button { class: "fork-sidebar-view-tab fork-sidebar-view-tab-active", title: "Repository tree", Icon { name: IconName::GitBranch } }
                button { class: "fork-sidebar-view-tab", title: "Search", Icon { name: IconName::Search } }
            }
            div { class: "fork-sidebar-search",
                span { class: "fork-search-icon", Icon { name: IconName::Search } }
                input { class: "fork-filter-input", placeholder: "Filter" }
            }
            ForkSidebarSection {
                title: "Branches".to_string(),
                rows: locals,
                open_menu: open_menu.read().clone(),
                on_open_menu: move |name: String| open_menu.set(Some(name)),
                on_close_menu: move |_| open_menu.set(None),
                on_checkout,
                on_branch_command
            }
            ForkRemoteSection {
                title: "Remotes".to_string(),
                rows: remotes,
                open_menu: open_menu.read().clone(),
                on_open_menu: move |name: String| open_menu.set(Some(name)),
                on_close_menu: move |_| open_menu.set(None),
                on_checkout,
                on_branch_command
            }
            section { class: "fork-sidebar-section",
                div { class: "fork-section-title",
                    span { class: "fork-section-caret", Icon { name: IconName::ChevronDown } }
                    span { "Tags" }
                }
                div { class: "fork-sidebar-row fork-sidebar-leaf fork-sidebar-muted-row",
                    span { class: "fork-row-icon", Icon { name: IconName::Tag } }
                    span { class: "min-w-0 truncate", "No tags loaded" }
                }
            }
            section { class: "fork-sidebar-section",
                div { class: "fork-section-title",
                    span { class: "fork-section-caret", Icon { name: IconName::ChevronDown } }
                    span { "Stashes" }
                }
                for stash in stashes.clone() {
                    div { class: "fork-sidebar-row fork-sidebar-leaf",
                        span { class: "fork-row-icon", Icon { name: IconName::Archive } }
                        span { class: "min-w-0 truncate", if stash.message.is_empty() { "#{stash.index} {stash.name}" } else { "{stash.message}" } }
                    }
                }
                if !has_stashes {
                    div { class: "fork-sidebar-empty", "No stashes" }
                }
            }
            section { class: "fork-sidebar-section",
                div { class: "fork-section-title",
                    span { class: "fork-section-caret", Icon { name: IconName::ChevronDown } }
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
                span { class: "fork-section-caret", Icon { name: IconName::ChevronDown } }
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
                        span { class: "fork-section-caret", Icon { name: IconName::ChevronDown } }
                        span { class: "fork-row-icon", Icon { name: IconName::FolderOpen } }
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
                span { class: "fork-section-caret", Icon { name: IconName::ChevronDown } }
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
                        span { class: "fork-section-caret", Icon { name: IconName::ChevronDown } }
                        span { class: "fork-row-icon", Icon { name: IconName::Remote } }
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
                span { class: "fork-row-icon", if branch.is_head { Icon { name: IconName::Check } } else { Icon { name: IconName::GitBranch } } }
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
                    Icon { name: IconName::More }
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
    rsx! {
        div { class: "fork-context-menu",
            ContextMenuItem { label: "Checkout...".to_string(), disabled: is_head, command: SidebarBranchCommand::Checkout(branch.clone()), on_command, on_close, active: true }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: format!("Fast-Forward to '{branch}'"), command: SidebarBranchCommand::FastForward(branch.clone()), on_command, on_close }
            ContextMenuItem { label: "Pull from".to_string(), command: SidebarBranchCommand::Pull(branch.clone()), on_command, on_close, chevron: true }
            ContextMenuItem { label: "Push to".to_string(), command: SidebarBranchCommand::Push(branch.clone()), on_command, on_close, chevron: true }
            ContextMenuItem { label: "Create Pull Request".to_string(), command: SidebarBranchCommand::CreatePullRequest(branch.clone()), on_command, on_close, chevron: true }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "Merge into 'main'...".to_string(), disabled: is_head, command: SidebarBranchCommand::Merge(branch.clone()), on_command, on_close }
            ContextMenuItem { label: format!("Rebase on '{branch}'..."), disabled: is_head, command: SidebarBranchCommand::Rebase(branch.clone()), on_command, on_close }
            ContextMenuItem { label: format!("Interactively Rebase on '{branch}'..."), disabled: is_head, command: SidebarBranchCommand::InteractiveRebase(branch.clone()), on_command, on_close }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "New Branch...".to_string(), command: SidebarBranchCommand::NewBranch(branch.clone()), on_command, on_close, shortcut: "⇧⌘B".to_string() }
            ContextMenuItem { label: "New Tag...".to_string(), command: SidebarBranchCommand::NewTag(branch.clone()), on_command, on_close, shortcut: "⇧⌘T".to_string() }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "Tracking".to_string(), command: SidebarBranchCommand::Tracking(branch.clone()), on_command, on_close, chevron: true }
            ContextMenuItem { label: "Rename...".to_string(), disabled: is_head, command: SidebarBranchCommand::Rename(branch.clone()), on_command, on_close }
            ContextMenuItem { label: "Delete...".to_string(), disabled: is_head, command: SidebarBranchCommand::Delete(branch.clone()), on_command, on_close }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "AI".to_string(), command: SidebarBranchCommand::Ai(branch.clone()), on_command, on_close, chevron: true }
            div { class: "fork-context-separator" }
            ContextMenuItem { label: "Copy Branch Name".to_string(), command: SidebarBranchCommand::CopyName(branch), on_command, on_close }
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
                span { class: "fork-context-chevron", Icon { name: IconName::ChevronRight } }
            }
        }
    }
}

#[component]
fn RepositoryList(
    repositories: Vec<api::RepositoryRecord>,
    on_open: EventHandler<String>,
) -> Element {
    rsx! {
        section { class: "min-h-0 flex-1 overflow-y-auto p-2 space-y-1",
            for repository in repositories {
                article { class: "group rounded-lg border border-zinc-800 bg-zinc-900/40 hover:border-cyan-700/80 hover:bg-zinc-900",
                    button {
                        class: "w-full min-w-0 p-3 text-left",
                        onclick: move |_| on_open.call(repository.id.clone()),
                        div { class: "font-medium text-sm text-zinc-100 truncate group-hover:text-cyan-300", "{repository.name}" }
                        div { class: "mt-1 text-xs text-zinc-500 truncate", "{repository.path}" }
                    }
                }
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
    on_select_commit: EventHandler<String>,
    on_load_more: EventHandler<()>,
) -> Element {
    let rows = graph_rows(&commits);
    rsx! {
        article { class: "commit-graph-panel min-h-[240px] xl:min-h-0 xl:col-start-2 xl:row-start-1 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "h-9 shrink-0 border-b border-zinc-800 px-3 flex items-center justify-between gap-2",
                h3 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-400", "Commit Graph" }
                button { class: "rounded border border-zinc-700 px-2 py-1 text-[11px] text-zinc-300 hover:bg-zinc-800", onclick: move |_| on_load_more.call(()), "Load more" }
            }
            div { class: "grid grid-cols-[128px_76px_minmax(0,1fr)_150px] border-b border-zinc-800 bg-zinc-900/60 px-2 py-1 text-[11px] font-medium uppercase tracking-wide text-zinc-500",
                span { "Graph" }
                span { "Commit" }
                span { "Message" }
                span { "Author" }
            }
            ol { class: "min-h-0 flex-1 overflow-y-auto",
                for row in rows {
                    li {
                        class: "grid grid-cols-[128px_76px_minmax(0,1fr)_150px] gap-2 border-b border-zinc-900 px-2 py-1.5 text-xs hover:bg-cyan-500/10",
                        onclick: {
                            let commit_id = row.commit.id.clone();
                            move |_| on_select_commit.call(commit_id.clone())
                        },
                        GraphLaneStrip { row: row.clone() }
                        code { class: "self-center text-cyan-300", "{short_id(&row.commit.id)}" }
                        span { class: "min-w-0 truncate self-center text-zinc-200", "{row.commit.summary}" }
                        span { class: "min-w-0 truncate self-center text-zinc-500", "{row.commit.author}" }
                    }
                }
            }
        }
    }
}

#[component]
fn GraphLaneStrip(row: GraphRow) -> Element {
    rsx! {
        div { class: "relative grid h-full min-h-10 grid-flow-col auto-cols-[16px] items-stretch justify-start",
            for lane in 0..row.lane_count {
                div { class: "relative h-full w-4",
                    if row.active_lanes.contains(&lane) {
                        div { class: format!("absolute left-1/2 top-0 h-full w-px -translate-x-1/2 {}", lane_color(lane, false)) }
                    }
                    if row.merge_lanes.contains(&lane) {
                        div { class: "absolute left-1/2 top-1/2 h-px w-4 bg-zinc-600" }
                    }
                    if lane == row.lane {
                        span { class: format!("absolute left-1/2 top-1/2 z-10 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full ring-4 ring-zinc-950 {}", lane_color(lane, true)) }
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
    diff: String,
    selected_file: String,
    on_stage: EventHandler<String>,
    on_diff: EventHandler<String>,
) -> Element {
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
                button { class: "fork-detail-tab fork-detail-tab-active", "Commit" }
                button { class: "fork-detail-tab", "Changes" }
                button { class: "fork-detail-tab", "File Tree" }
            }
            div { class: "fork-detail-body",
                if let Some(commit) = selected {
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
                section { class: "fork-changed-files",
                    div { class: "fork-changed-header",
                        span { "Changed Files" }
                        span { class: "fork-muted", "{files.len()} item(s)" }
                    }
                    for file in files.into_iter().take(80) {
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
    output: String,
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
            div { class: "min-h-0 flex-1 grid grid-cols-1 xl:grid-cols-[1.2fr_1fr] gap-3 overflow-y-auto p-3",
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
                pre { class: "min-h-[300px] overflow-auto rounded-md border border-zinc-800 bg-zinc-950/70 p-3 font-mono text-xs leading-5 text-zinc-300 whitespace-pre-wrap", "{output}" }
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

fn lane_color(lane: usize, dot: bool) -> &'static str {
    let colors = if dot {
        [
            "bg-teal-400",
            "bg-amber-400",
            "bg-violet-400",
            "bg-rose-400",
            "bg-sky-400",
            "bg-amber-400",
            "bg-emerald-400",
        ]
    } else {
        [
            "bg-teal-700",
            "bg-amber-700",
            "bg-violet-700",
            "bg-rose-700",
            "bg-sky-700",
            "bg-amber-700",
            "bg-emerald-700",
        ]
    };
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
