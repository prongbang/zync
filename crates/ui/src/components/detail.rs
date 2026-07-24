use dioxus::prelude::*;
use crate::*;

#[component]
pub(crate) fn ForkCommitDetailPanel(
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
    repo_stats: Option<api::RepoStats>,
    on_load_stats: EventHandler<()>,
    blame: Option<BlameView>,
    on_blame: EventHandler<()>,
    on_close_blame: EventHandler<()>,
    on_stage_patch: EventHandler<String>,
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
                button {
                    class: detail_tab_class(selected_tab, ForkDetailTab::Repository),
                    onclick: move |_| {
                        active_tab.set(ForkDetailTab::Repository);
                        on_load_stats.call(());
                    },
                    "Repository"
                }
            }
            if selected_tab == ForkDetailTab::Commit {
                div { class: "fork-detail-body",
                    if let Some(commit) = selected.clone() {
                        section { class: "fork-commit-summary",
                            div { class: "fork-person-card",
                                if let Some(url) = gravatar_url(&commit.author_email, 96) {
                                    img { class: "fork-avatar fork-avatar-img", src: "{url}", alt: "" }
                                } else {
                                    div { class: "fork-avatar", "{commit.author.chars().next().unwrap_or('Z')}" }
                                }
                                div { class: "min-w-0",
                                    div { class: "fork-label", "AUTHOR" }
                                    div { class: "fork-person-name", "{commit.author}" }
                                    if !commit.author_email.is_empty() {
                                        div { class: "fork-muted", "{commit.author_email}" }
                                    }
                                    div { class: "fork-muted", "{format_commit_time(commit.time)}" }
                                }
                            }
                            if !commit.committer.is_empty()
                                && (commit.committer != commit.author
                                    || commit.committer_email != commit.author_email)
                            {
                                div { class: "fork-person-card",
                                    if let Some(url) = gravatar_url(&commit.committer_email, 96) {
                                        img { class: "fork-avatar fork-avatar-img", src: "{url}", alt: "" }
                                    } else {
                                        div { class: "fork-avatar", "{commit.committer.chars().next().unwrap_or('Z')}" }
                                    }
                                    div { class: "min-w-0",
                                        div { class: "fork-label", "COMMITTER" }
                                        div { class: "fork-person-name", "{commit.committer}" }
                                        if !commit.committer_email.is_empty() {
                                            div { class: "fork-muted", "{commit.committer_email}" }
                                        }
                                    }
                                }
                            }
                            div { class: "fork-sha-card",
                                if !commit.refs.is_empty() {
                                    div { class: "fork-label", "REFS" }
                                    div { class: "fork-ref-list",
                                        for commit_ref in commit.refs.iter().cloned() {
                                            span { class: commit_ref_class(&commit_ref.kind), "{commit_ref.name}" }
                                        }
                                    }
                                    div { class: "fork-label mt-2", "SHA" }
                                } else {
                                    div { class: "fork-label", "SHA" }
                                }
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
                    blame: blame.clone(),
                    on_stage,
                    on_diff,
                    on_stage_patch,
                    on_blame,
                    on_close_blame
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
            } else if selected_tab == ForkDetailTab::Repository {
                RepoStatsPanel { stats: repo_stats.clone() }
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
            footer { class: "fork-detail-commit-footer",
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
        }
    }
}

#[component]
pub(crate) fn RepoStatsPanel(stats: Option<api::RepoStats>) -> Element {
    let contributor_max = stats
        .as_ref()
        .and_then(|stats| stats.contributors.first())
        .map(|author| author.commits)
        .unwrap_or(1)
        .max(1);
    rsx! {
        div { class: "fork-detail-body",
            if let Some(stats) = stats {
                section { class: "repo-stats-overview",
                    div { class: "repo-stats-card",
                        div { class: "fork-label", "COMMITS" }
                        div { class: "repo-stats-number", "{stats.commit_count}" }
                    }
                    div { class: "repo-stats-card",
                        div { class: "fork-label", "CONTRIBUTORS" }
                        div { class: "repo-stats-number", "{stats.contributors.len()}" }
                    }
                    div { class: "repo-stats-card",
                        div { class: "fork-label", "FIRST COMMIT" }
                        div { class: "repo-stats-date", "{format_commit_time(stats.first_commit_time)}" }
                    }
                    div { class: "repo-stats-card",
                        div { class: "fork-label", "LAST COMMIT" }
                        div { class: "repo-stats-date", "{format_commit_time(stats.last_commit_time)}" }
                    }
                }
                section { class: "repo-stats-block",
                    h3 { class: "repo-stats-heading", "Commits per month" }
                    RepoStatsChart { monthly: stats.monthly.clone() }
                }
                section { class: "repo-stats-block",
                    h3 { class: "repo-stats-heading", "Top contributors" }
                    for author in stats.contributors.iter().take(8).cloned() {
                        div { class: "repo-stats-author-row",
                            span { class: "repo-stats-author-name", "{author.name}" }
                            div { class: "repo-stats-author-track",
                                div {
                                    class: "repo-stats-author-bar",
                                    style: "width:{(author.commits as f64 / contributor_max as f64 * 100.0).max(2.0)}%;",
                                }
                            }
                            span { class: "repo-stats-author-count", "{author.commits}" }
                        }
                    }
                }
            } else {
                section { class: "fork-message-block",
                    h3 { "Loading repository statistics..." }
                    p { class: "fork-muted", "Crunching commit history in the background." }
                }
            }
        }
    }
}

#[component]
pub(crate) fn RepoStatsChart(monthly: Vec<api::MonthStat>) -> Element {
    const CHART_WIDTH: f64 = 640.0;
    const CHART_HEIGHT: f64 = 170.0;
    const BASELINE: f64 = 148.0;
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let max_total = monthly
        .iter()
        .map(|month| month.total)
        .max()
        .unwrap_or(1)
        .max(1);
    let count = monthly.len().max(1);
    let slot = CHART_WIDTH / count as f64;
    let bar_width = (slot - 3.0).clamp(1.0, 42.0);
    let bar_inset = (slot - bar_width) / 2.0;
    let label_step = count.div_ceil(8).max(1);
    rsx! {
        if monthly.is_empty() {
            p { class: "fork-muted", "No commits in range." }
        } else {
            svg {
                class: "repo-stats-chart",
                view_box: "0 0 {CHART_WIDTH} {CHART_HEIGHT}",
                line {
                    x1: "0",
                    y1: "{BASELINE}",
                    x2: "{CHART_WIDTH}",
                    y2: "{BASELINE}",
                    stroke: "rgba(255,255,255,0.14)",
                    stroke_width: "1",
                }
                for (index, month) in monthly.iter().enumerate() {
                    {
                        let bar_height =
                            (month.total as f64 / max_total as f64 * (BASELINE - 12.0)).max(1.5);
                        let x = index as f64 * slot + bar_inset;
                        let y = BASELINE - bar_height;
                        let title = format!(
                            "{} {}: {} commit(s)",
                            MONTHS[(month.month.clamp(1, 12) - 1) as usize],
                            month.year,
                            month.total,
                        );
                        rsx! {
                            rect {
                                x: "{x}",
                                y: "{y}",
                                width: "{bar_width}",
                                height: "{bar_height}",
                                rx: "1.5",
                                fill: "#26d0bd",
                                opacity: "0.85",
                                title { "{title}" }
                            }
                        }
                    }
                }
                for (index, month) in monthly.iter().enumerate() {
                    if index % label_step == 0 {
                        text {
                            x: "{index as f64 * slot + slot / 2.0}",
                            y: "{CHART_HEIGHT - 6.0}",
                            fill: "#8b8d9a",
                            font_size: "9",
                            text_anchor: "middle",
                            "{MONTHS[(month.month.clamp(1, 12) - 1) as usize]} {month.year % 100}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub(crate) fn ForkChangedFilesList(
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
pub(crate) fn ForkChangesTab(
    selected: Option<api::CommitSummary>,
    files: Vec<api::FileStatus>,
    diff: String,
    selected_file: String,
    additions: usize,
    deletions: usize,
    blame: Option<BlameView>,
    on_stage: EventHandler<String>,
    on_diff: EventHandler<String>,
    on_stage_patch: EventHandler<String>,
    on_blame: EventHandler<()>,
    on_close_blame: EventHandler<()>,
) -> Element {
    let mut split_view = use_signal(|| false);
    let stage_enabled = selected.is_none();
    let blame_active = blame.is_some();
    let split_active = *split_view.read();
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
                        div { class: "fork-diff-controls",
                            button {
                                class: if !split_active && !blame_active { "fork-diff-toggle fork-diff-toggle-active" } else { "fork-diff-toggle" },
                                onclick: move |_| {
                                    split_view.set(false);
                                    on_close_blame.call(());
                                },
                                "Inline"
                            }
                            button {
                                class: if split_active && !blame_active { "fork-diff-toggle fork-diff-toggle-active" } else { "fork-diff-toggle" },
                                onclick: move |_| {
                                    split_view.set(true);
                                    on_close_blame.call(());
                                },
                                "Split"
                            }
                            button {
                                class: if blame_active { "fork-diff-toggle fork-diff-toggle-active" } else { "fork-diff-toggle" },
                                disabled: selected_file.is_empty(),
                                onclick: move |_| {
                                    if blame_active {
                                        on_close_blame.call(());
                                    } else {
                                        on_blame.call(());
                                    }
                                },
                                "Blame"
                            }
                        }
                    }
                    if let Some(view) = blame {
                        div { class: "fork-blame-wrap", BlameTable { view } }
                    } else if split_active {
                        SplitDiffSection { diff }
                    } else {
                        ForkCompactDiff { diff, stage_enabled, on_stage_patch }
                    }
                }
            }
        }
    }
}
