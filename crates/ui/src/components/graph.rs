use dioxus::prelude::*;
use crate::*;

#[component]
pub(crate) fn CommitContextMenu(
    commit_id: String,
    on_close: EventHandler<()>,
    on_command: EventHandler<CommitMenuCommand>,
) -> Element {
    let run = move |command: CommitMenuCommand| {
        on_command.call(command);
        on_close.call(());
    };
    let id = commit_id;
    rsx! {
        button { class: "commit-menu-scrim", onclick: move |event| { event.stop_propagation(); on_close.call(()); } }
        div { class: "commit-context-menu",
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::NewBranch(id.clone())); } }, "New Branch..." }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::NewTag(id.clone())); } }, "New Tag..." }
            div { class: "commit-menu-divider" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::RebaseToHere(id.clone())); } }, "Rebase to Here..." }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::InteractiveRebase(id.clone())); } }, "Interactive Rebase..." }
            div { class: "commit-menu-section", "Quick Actions" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::Reword(id.clone())); } }, "Reword Message..." }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::EditCommit(id.clone())); } }, "Edit..." }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::SquashIntoParent(id.clone())); } }, "Squash into Parent" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::FixupIntoParent(id.clone())); } }, "Fixup into Parent" }
            button { class: "commit-menu-item commit-menu-danger", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::DropCommit(id.clone())); } }, "Drop..." }
            div { class: "commit-menu-divider" }
            button { class: "commit-menu-item commit-menu-danger", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::ResetToHere(id.clone())); } }, "Reset to Here..." }
            div { class: "commit-menu-divider" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::CheckoutCommit(id.clone())); } }, "Checkout Commit" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::CherryPick(id.clone())); } }, "Cherry-pick Commit" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::Revert(id.clone())); } }, "Revert Commit" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::SaveAsPatch(id.clone())); } }, "Save as Patch" }
            div { class: "commit-menu-divider" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::CompareToLocal(id.clone())); } }, "Compare to Local Changes" }
            button { class: "commit-menu-item", onclick: { let id = id.clone(); move |event| { event.stop_propagation(); run(CommitMenuCommand::CopySha(id.clone())); } }, "Copy Commit SHA" }
        }
    }
}

#[component]
pub(crate) fn CommitGraph(
    rows: Vec<GraphRow>,
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
    open_menu: Option<String>,
    on_open_menu: EventHandler<String>,
    on_close_menu: EventHandler<()>,
    on_menu_command: EventHandler<CommitMenuCommand>,
) -> Element {
    // Windowed rendering: rows are a fixed 34px tall, so only the slice near
    // the viewport is materialized; spacer items keep the scrollbar honest.
    const COMMIT_ROW_HEIGHT: f64 = 34.0;
    const OVERSCAN_ROWS: usize = 10;
    // Written from the (wasm-only) scroll handler; plain bindings on native.
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
    let mut scroll_top = use_signal(|| 0.0f64);
    #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
    let mut viewport_height = use_signal(|| 720.0f64);
    let mut list_element = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let total_rows = rows.len();
    let first_row = ((scroll_top() / COMMIT_ROW_HEIGHT) as usize)
        .saturating_sub(OVERSCAN_ROWS)
        .min(total_rows);
    let visible_rows = ((viewport_height() / COMMIT_ROW_HEIGHT).ceil() as usize
        + 2 * OVERSCAN_ROWS)
        .min(total_rows - first_row);
    let last_row = first_row + visible_rows;
    let top_spacer = first_row as f64 * COMMIT_ROW_HEIGHT;
    let bottom_spacer = (total_rows - last_row) as f64 * COMMIT_ROW_HEIGHT;
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
                    span { "Description" }
                    span { "Author" }
                    span { "Commit" }
                    span { "Date" }
                }
                ol { class: "min-h-0 flex-1 overflow-y-auto",
                    onmounted: move |event| {
                        let element = event.data();
                        #[cfg(target_arch = "wasm32")]
                        if let Some(node) = element.downcast::<web_sys::Element>() {
                            viewport_height.set(f64::from(node.client_height()).max(200.0));
                        }
                        list_element.set(Some(element));
                    },
                    onscroll: move |_| {
                        #[cfg(target_arch = "wasm32")]
                        if let Some(element) = list_element.read().as_ref() {
                            if let Some(node) = element.downcast::<web_sys::Element>() {
                                scroll_top.set(f64::from(node.scroll_top()));
                                viewport_height.set(f64::from(node.client_height()).max(200.0));
                            }
                        }
                    },
                    if first_row > 0 {
                        li { style: "height:{top_spacer}px;" }
                    }
                    for row in rows[first_row..last_row].iter().cloned() {
                        li {
                            class: if row.commit.id == selected_commit_id { "commit-list-row commit-list-row-active" } else { "commit-list-row" },
                            oncontextmenu: {
                                let commit_id = row.commit.id.clone();
                                move |event: Event<MouseData>| {
                                    event.prevent_default();
                                    on_open_menu.call(commit_id.clone());
                                }
                            },
                            onclick: {
                                let commit_id = row.commit.id.clone();
                                move |_| on_select_commit.call(commit_id.clone())
                            },
                            div { class: "commit-graph-cell", GraphLaneStrip { row: row.clone() } }
                            span { class: "commit-list-message",
                                for commit_ref in row.commit.refs.iter().take(4).cloned() {
                                    span { class: commit_ref_class(&commit_ref.kind), "{commit_ref.name}" }
                                }
                                if row.commit.refs.len() > 4 {
                                    span { class: "commit-ref commit-ref-more", "+{row.commit.refs.len() - 4}" }
                                }
                                span { class: "commit-message-text", "{row.commit.summary}" }
                            }
                            span { class: "commit-list-author",
                                if let Some(url) = gravatar_url(&row.commit.author_email, 32) {
                                    img { class: "commit-avatar", src: "{url}", alt: "", loading: "lazy" }
                                }
                                span { class: "commit-author-name", "{row.commit.author}" }
                            }
                            code { class: "commit-list-sha", "{short_id(&row.commit.id)}" }
                            span { class: "commit-list-date", "{format_commit_time(row.commit.time)}" }
                            if open_menu.as_deref() == Some(row.commit.id.as_str()) {
                                CommitContextMenu {
                                    commit_id: row.commit.id.clone(),
                                    on_close: on_close_menu,
                                    on_command: on_menu_command,
                                }
                            }
                        }
                    }
                    if last_row < total_rows {
                        li { style: "height:{bottom_spacer}px;" }
                    }
                }
            }
        }
    }
}

#[component]
pub(crate) fn GraphLaneStrip(row: GraphRow) -> Element {
    const LANE_WIDTH: f64 = 13.0;
    const ROW_HEIGHT: f64 = 34.0;
    let lane_center = |lane: usize| lane as f64 * LANE_WIDTH + LANE_WIDTH / 2.0;
    let width = row.lane_count as f64 * LANE_WIDTH;
    let dot_x = lane_center(row.lane);
    let mid_y = ROW_HEIGHT / 2.0;
    rsx! {
        svg {
            class: "graph-lane-svg",
            width: "{width}",
            height: "{ROW_HEIGHT}",
            view_box: "0 0 {width} {ROW_HEIGHT}",
            for lane in 0..row.lane_count {
                {
                    let top = row.top_lanes.contains(&lane);
                    let bottom = row.bottom_lanes.contains(&lane);
                    let is_commit_lane = lane == row.lane;
                    let is_merge_lane = !is_commit_lane && row.merge_lanes.contains(&lane);
                    rsx! {
                        // Straight rail segments: full-height for pass-through
                        // lanes, half-height at tips (no top) and roots (no
                        // bottom). Merge-born lanes get a curve instead.
                        if top {
                            line {
                                x1: "{lane_center(lane)}",
                                y1: "0",
                                x2: "{lane_center(lane)}",
                                y2: if bottom && !is_commit_lane { "{ROW_HEIGHT}" } else { "{mid_y}" },
                                stroke: lane_color(lane),
                                stroke_width: "2",
                                stroke_linecap: "round",
                            }
                        }
                        if bottom && (is_commit_lane || (!top && !is_merge_lane)) {
                            line {
                                x1: "{lane_center(lane)}",
                                y1: "{mid_y}",
                                x2: "{lane_center(lane)}",
                                y2: "{ROW_HEIGHT}",
                                stroke: lane_color(lane),
                                stroke_width: "2",
                                stroke_linecap: "round",
                            }
                        }
                        if is_merge_lane {
                            path {
                                d: "M {dot_x} {mid_y} Q {lane_center(lane)} {mid_y} {lane_center(lane)} {ROW_HEIGHT}",
                                fill: "none",
                                stroke: lane_color(lane),
                                stroke_width: "2",
                                stroke_linecap: "round",
                            }
                        }
                    }
                }
            }
            circle {
                cx: "{dot_x}",
                cy: "{mid_y}",
                r: "3.6",
                fill: lane_color(row.lane),
                stroke: "#0b0d12",
                stroke_width: "1.6",
            }
        }
    }
}
