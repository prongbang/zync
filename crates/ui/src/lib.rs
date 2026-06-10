use dioxus::prelude::*;

pub mod api;

pub fn app() -> Element {
    let api = use_signal(api::ZyncApi::default);
    let api_base = api.read().base_url.clone();

    let mut repositories = use_signal(Vec::<api::RepositoryRecord>::new);
    let workspace = use_signal(|| None::<api::WorkspaceResponse>);
    let git_status = use_signal(Vec::<api::FileStatus>::new);
    let branches = use_signal(Vec::<api::BranchSummary>::new);
    let commits = use_signal(Vec::<api::CommitSummary>::new);
    let mut diff = use_signal(String::new);
    let mut selected_file = use_signal(String::new);
    let mut editor_content = use_signal(String::new);
    let mut repo_path = use_signal(String::new);
    let mut repo_name = use_signal(String::new);
    let mut commit_message = use_signal(String::new);
    let mut notice = use_signal(|| "Ready".to_string());

    {
        let api = api.read().clone();
        use_effect(move || {
            load_repositories(api.clone(), repositories, notice);
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

    rsx! {
        script { src: "https://cdn.tailwindcss.com" }
        main { class: "min-h-screen bg-zinc-950 text-zinc-100 flex flex-col lg:flex-row overflow-x-hidden lg:overflow-hidden",
            aside { class: "w-full lg:w-[340px] lg:h-screen shrink-0 border-b lg:border-b-0 lg:border-r border-zinc-800 bg-zinc-950/95 p-3 sm:p-4 flex flex-col gap-3 sm:gap-4",
                header { class: "space-y-1 flex items-start justify-between gap-3 lg:block",
                    h1 { class: "text-xl font-semibold tracking-tight", "Zync" }
                    p { class: "max-w-[220px] sm:max-w-none text-xs text-zinc-500 truncate", "API {api_base}" }
                }

                section { class: "rounded-lg border border-zinc-800 bg-zinc-900/60 p-3 space-y-3",
                    input {
                        class: "w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                        placeholder: "Repository path mounted on server",
                        value: "{repo_path}",
                        oninput: move |event| repo_path.set(event.value())
                    }
                    input {
                        class: "w-full rounded-md border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100 placeholder:text-zinc-500 outline-none focus:border-cyan-500",
                        placeholder: "Name",
                        value: "{repo_name}",
                        oninput: move |event| repo_name.set(event.value())
                    }
                    div { class: "flex flex-col sm:flex-row gap-2",
                        button {
                            class: "flex-1 rounded-md bg-cyan-500 px-3 py-2 text-sm font-medium text-zinc-950 hover:bg-cyan-400 disabled:opacity-50",
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
                            class: "rounded-md border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-800",
                            onclick: move |_| load_repositories(api.read().clone(), repositories, notice),
                            "Refresh"
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

            section { class: "min-w-0 flex-1 flex flex-col bg-zinc-950",
                header { class: "shrink-0 border-b border-zinc-800 px-3 sm:px-5 py-3 flex flex-col sm:flex-row sm:items-center justify-between gap-3 sm:gap-4 bg-zinc-950/95",
                    if let Some(current) = workspace.read().as_ref() {
                        div { class: "min-w-0",
                            h2 { class: "text-lg font-semibold truncate", "{current.repository.name}" }
                            p { class: "text-xs text-zinc-500 truncate", "{current.repository.path}" }
                        }
                        div { class: "hidden xl:flex flex-col items-end gap-1 text-[11px] text-zinc-500 min-w-0",
                            span { class: "truncate max-w-[420px]", "Workspace {current.workspace.id}" }
                            span { class: "truncate max-w-[420px]", "WS {websocket_url}" }
                        }
                    } else {
                        div { class: "min-w-0",
                            h2 { class: "text-lg font-semibold", "Open a mounted Git repository" }
                            p { class: "text-xs text-zinc-500", "Mount a project into the server, add its server-side path, then open it here." }
                        }
                    }
                    button {
                        class: "w-full sm:w-auto rounded-md border border-zinc-700 px-3 py-2 text-sm text-zinc-200 hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-40",
                        disabled: current_repository_id.is_empty(),
                        onclick: move |_| {
                            if let Some(current) = workspace.read().as_ref() {
                                load_workspace(
                                    api.read().clone(),
                                    current.repository.id.clone(),
                                    current.workspace.id.clone(),
                                    workspace,
                                    git_status,
                                    branches,
                                    commits,
                                    diff,
                                    notice
                                );
                            }
                        },
                        "Refresh workspace"
                    }
                }

                div { class: "flex-1 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-[280px_minmax(0,1.35fr)_minmax(360px,0.9fr)] xl:grid-rows-[minmax(0,1fr)_290px] gap-3 p-3 overflow-y-auto lg:overflow-hidden",
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
                        }
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
                                let staged = api_client.diff_staged_file(&repository_id, &path).await.unwrap_or_default();
                                let workdir = api_client.diff_workdir_file(&repository_id, &path).await.unwrap_or_default();
                                let patch = if staged.is_empty() && workdir.is_empty() {
                                    format!("No diff for {path}")
                                } else {
                                    format!("--- staged: {path} ---\n{staged}\n--- working tree: {path} ---\n{workdir}")
                                };
                                diff.set(patch);
                                notice.set(format!("Showing diff for {path}"));
                            });
                        }
                    }

                    DiffViewer { diff: diff.read().clone() }

                    CommitPanel {
                        message: commit_message.read().clone(),
                        on_message: move |message: String| commit_message.set(message),
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
                            spawn(async move {
                                let request = api::CommitRequest {
                                    message,
                                    author_name: "Zync".to_string(),
                                    author_email: "zync@local".to_string(),
                                    amend: false,
                                    sign_off: false,
                                };
                                match api_client.commit(&repository_id, &request).await {
                                    Ok(_) => {
                                        notice.set("Committed".to_string());
                                        commit_message.set(String::new());
                                        load_workspace(
                                            api_client,
                                            repository_id,
                                            workspace_id,
                                            workspace,
                                            git_status,
                                            branches,
                                            commits,
                                            diff,
                                            notice
                                        );
                                    }
                                    Err(error) => notice.set(error),
                                }
                            });
                        }
                    }

                    BranchPanel { branches: branches.read().clone() }
                    CommitGraph { commits: commits.read().clone() }
                }

                footer { class: "h-8 shrink-0 border-t border-zinc-800 px-4 flex items-center text-xs text-zinc-400 bg-zinc-950", "{notice}" }
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
                    diff,
                    notice,
                );
            }
            Err(error) => notice.set(error),
        }
    });
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
    _diff: Signal<String>,
    _notice: Signal<String>,
) {
}

#[component]
fn RepositoryList(
    repositories: Vec<api::RepositoryRecord>,
    on_open: EventHandler<String>,
) -> Element {
    rsx! {
        section { class: "max-h-64 lg:max-h-none lg:min-h-0 lg:flex-1 overflow-y-auto space-y-2 pr-1",
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
) -> Element {
    rsx! {
        article { class: "min-h-[260px] md:min-h-[320px] xl:min-h-0 xl:row-span-2 rounded-lg border border-zinc-800 bg-zinc-900/55 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Files" }
            ul { class: "min-h-0 flex-1 overflow-y-auto p-2 space-y-1",
                for file in files.into_iter().filter(|file| !file.is_dir).take(300) {
                    li {
                        button {
                            class: if file.path == selected { "w-full rounded-md bg-cyan-500/15 px-2 py-1.5 text-left text-xs text-cyan-200 border border-cyan-500/30 truncate" } else { "w-full rounded-md px-2 py-1.5 text-left text-xs text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100 truncate" },
                            onclick: move |_| on_select.call(file.path.clone()),
                            "{file.path}"
                        }
                    }
                }
            }
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
        article { class: "min-h-[420px] md:min-h-[520px] xl:min-h-0 rounded-lg border border-zinc-800 bg-zinc-900/55 flex flex-col overflow-hidden",
            header { class: "shrink-0 border-b border-zinc-800 px-3 py-2 flex items-center justify-between gap-3",
                h3 { class: "min-w-0 truncate text-sm font-semibold", if path.is_empty() { "Editor" } else { "{path}" } }
                button { class: "rounded-md bg-cyan-500 px-3 py-1.5 text-xs font-medium text-zinc-950 hover:bg-cyan-400", onclick: move |_| on_save.call(()), "Save" }
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
        article { class: "min-h-[320px] md:min-h-[420px] xl:min-h-0 rounded-lg border border-zinc-800 bg-zinc-900/55 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Git Status" }
            div { class: "min-h-0 flex-1 overflow-y-auto p-3 space-y-4",
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
        section { class: "space-y-2",
            div { class: "flex items-center justify-between gap-2",
                h4 { class: "text-xs font-semibold uppercase tracking-wide text-zinc-500", "{title}" }
                button {
                    class: "rounded-md border border-zinc-700 px-2 py-1 text-[11px] text-zinc-300 hover:bg-zinc-800 disabled:opacity-40",
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
        div { class: "rounded-md border border-zinc-800 bg-zinc-950/45 p-2 space-y-2",
            code { class: "block truncate text-xs text-zinc-300", "{path}" }
            div { class: "flex flex-wrap gap-2",
                button { class: "rounded-md border border-zinc-700 px-2 py-1 text-[11px] text-zinc-200 hover:bg-zinc-800", onclick: move |_| on_diff.call(diff_path.clone()), "Diff" }
                button { class: "rounded-md border border-cyan-700/60 px-2 py-1 text-[11px] text-cyan-200 hover:bg-cyan-500/10", onclick: move |_| on_primary.call(primary_path.clone()), "{primary_label}" }
                button { class: "rounded-md border border-red-800/70 px-2 py-1 text-[11px] text-red-200 hover:bg-red-500/10", onclick: move |_| on_discard.call(discard_path.clone()), "Discard" }
            }
        }
    }
}

#[component]
fn DiffViewer(diff: String) -> Element {
    rsx! {
        article { class: "min-h-[320px] md:min-h-[420px] xl:min-h-0 rounded-lg border border-zinc-800 bg-zinc-900/55 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Working Tree Diff" }
            pre { class: "min-h-0 flex-1 overflow-auto bg-zinc-950/70 p-3 font-mono text-xs leading-5 text-zinc-300", "{diff}" }
        }
    }
}

#[component]
fn CommitPanel(
    message: String,
    on_message: EventHandler<String>,
    on_commit: EventHandler<()>,
) -> Element {
    rsx! {
        article { class: "min-h-[260px] rounded-lg border border-zinc-800 bg-zinc-900/55 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Commit" }
            textarea {
                class: "min-h-0 flex-1 resize-none bg-zinc-950/70 p-3 text-sm text-zinc-100 outline-none placeholder:text-zinc-600",
                value: "{message}",
                placeholder: "Commit message",
                oninput: move |event| on_message.call(event.value())
            }
            div { class: "border-t border-zinc-800 p-3",
                button { class: "w-full rounded-md bg-emerald-500 px-3 py-2 text-sm font-medium text-zinc-950 hover:bg-emerald-400", onclick: move |_| on_commit.call(()), "Commit staged changes" }
            }
        }
    }
}

#[component]
fn BranchPanel(branches: Vec<api::BranchSummary>) -> Element {
    rsx! {
        article { class: "min-h-[240px] rounded-lg border border-zinc-800 bg-zinc-900/55 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Branches" }
            ul { class: "min-h-0 flex-1 overflow-y-auto p-3 space-y-1",
                for branch in branches {
                    li { class: "flex items-center justify-between gap-2 rounded-md px-2 py-1.5 text-xs hover:bg-zinc-800",
                        if branch.is_head {
                            strong { class: "truncate text-cyan-300", "{branch.name}" }
                        } else {
                            span { class: "truncate text-zinc-300", "{branch.name}" }
                        }
                        small { class: "shrink-0 text-zinc-600", " {branch.kind}" }
                    }
                }
            }
        }
    }
}

#[component]
fn CommitGraph(commits: Vec<api::CommitSummary>) -> Element {
    rsx! {
        article { class: "min-h-[240px] rounded-lg border border-zinc-800 bg-zinc-900/55 flex flex-col overflow-hidden",
            h3 { class: "shrink-0 border-b border-zinc-800 px-3 py-2 text-sm font-semibold", "Commit Graph" }
            ol { class: "min-h-0 flex-1 overflow-y-auto p-3 space-y-2",
                for commit in commits {
                    li { class: "grid grid-cols-[70px_1fr] gap-2 rounded-md border border-zinc-800 bg-zinc-950/40 p-2 text-xs",
                        code { class: "text-cyan-300", "{short_id(&commit.id)}" }
                        div { class: "min-w-0",
                            span { class: "block truncate text-zinc-200", "{commit.summary}" }
                            small { class: "text-zinc-600", "{commit.author}" }
                        }
                    }
                }
            }
        }
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
