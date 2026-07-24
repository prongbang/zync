use dioxus::prelude::*;
use crate::*;

#[component]
pub(crate) fn FileExplorer(
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
pub(crate) fn PaneStepSplitter(
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
pub(crate) fn PaneGridSplitters(
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
pub(crate) fn PaneSizeControls(
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
pub(crate) fn PaneSlider(
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
pub(crate) fn WorkspaceToolbar(
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
pub(crate) fn EditorPanel(
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
pub(crate) fn GitStatusPanel(
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
pub(crate) fn StatusGroup(
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
pub(crate) fn StatusRow(
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
pub(crate) fn CommitPanel(
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
pub(crate) fn BranchPanel(
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
pub(crate) fn BranchRow(
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
pub(crate) fn BranchContextMenu(
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
pub(crate) fn ToolbarIcon(icon: ToolbarGlyph) -> Element {
    let path_data = match icon {
        ToolbarGlyph::Fetch => "M8 2.5v7 M4.8 6.7L8 9.9l3.2-3.2 M3 13h10",
        ToolbarGlyph::Pull => "M8 2.5V12 M4.8 8.8L8 12l3.2-3.2",
        ToolbarGlyph::Push => "M8 13.5V4 M4.8 7.2L8 4l3.2 3.2",
        ToolbarGlyph::Stash => "M3 6.5h10v6.5H3z M4.6 3.5h6.8L13 6.5H3z M6.3 9.5h3.4",
    };
    rsx! {
        svg {
            class: "toolbar-icon",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.5",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "{path_data}" }
        }
    }
}

#[component]
pub(crate) fn BasicGitToolsPanel(
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
                    ToolButton { label: "Tree at Revision".to_string(), action: ToolAction::TreeAtRevision, on_action: on_tool_action }
                    ToolButton { label: "Reflog".to_string(), action: ToolAction::Reflog, on_action: on_tool_action }
                    ToolButton { label: "Reset Mixed".to_string(), action: ToolAction::ResetMixed, on_action: on_tool_action }
                    ToolButton { label: "Reset Hard".to_string(), action: ToolAction::ResetHard, on_action: on_tool_action }
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
                    ToolButton { label: "Prune Remote".to_string(), action: ToolAction::PruneRemote, on_action: on_tool_action }
                    ToolButton { label: "Delete Remote Branch".to_string(), action: ToolAction::DeleteRemoteBranch, on_action: on_tool_action }
                    ToolButton { label: "Set Upstream".to_string(), action: ToolAction::SetUpstream, on_action: on_tool_action }
                    ToolButton { label: "Force Lease Push".to_string(), action: ToolAction::PushForceWithLease, on_action: on_tool_action }
                    ToolButton { label: "GitHub Links".to_string(), action: ToolAction::GithubLinks, on_action: on_tool_action }
                    ToolButton { label: "Submodules".to_string(), action: ToolAction::Submodules, on_action: on_tool_action }
                    ToolButton { label: "Submodule Init".to_string(), action: ToolAction::SubmoduleInit, on_action: on_tool_action }
                    ToolButton { label: "Submodule Update".to_string(), action: ToolAction::SubmoduleUpdate, on_action: on_tool_action }
                    ToolButton { label: "Submodule Sync".to_string(), action: ToolAction::SubmoduleSync, on_action: on_tool_action }
                    ToolButton { label: "LFS".to_string(), action: ToolAction::Lfs, on_action: on_tool_action }
                    ToolButton { label: "LFS Install".to_string(), action: ToolAction::LfsInstall, on_action: on_tool_action }
                    ToolButton { label: "LFS Track".to_string(), action: ToolAction::LfsTrack, on_action: on_tool_action }
                    ToolButton { label: "LFS Untrack".to_string(), action: ToolAction::LfsUntrack, on_action: on_tool_action }
                    ToolButton { label: "LFS Pull".to_string(), action: ToolAction::LfsPull, on_action: on_tool_action }
                    ToolButton { label: "LFS Push".to_string(), action: ToolAction::LfsPush, on_action: on_tool_action }
                    ToolButton { label: "Develop".to_string(), action: ToolAction::GitFlowDevelop, on_action: on_tool_action }
                    ToolButton { label: "Feature".to_string(), action: ToolAction::GitFlowFeature, on_action: on_tool_action }
                    ToolButton { label: "Release".to_string(), action: ToolAction::GitFlowRelease, on_action: on_tool_action }
                    ToolButton { label: "Hotfix".to_string(), action: ToolAction::GitFlowHotfix, on_action: on_tool_action }
                    button { class: "basic-tool-button basic-tool-danger", onclick: move |_| on_delete_repository.call(()), "Remove Repo from Zync" }
                }
            }
        }
    }
}

#[component]
pub(crate) fn HistoryToolsPanel(
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
pub(crate) fn RebaseStepRow(
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
pub(crate) fn ConflictEditorPanel(
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
pub(crate) fn ConflictPane(title: String, path: String, content: String) -> Element {
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
pub(crate) fn RepositoryToolsPanel(
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
pub(crate) fn ToolButton(label: String, action: ToolAction, on_action: EventHandler<ToolAction>) -> Element {
    rsx! {
        button {
            class: "rounded-md border border-zinc-700 px-2 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800",
            onclick: move |_| on_action.call(action),
            "{label}"
        }
    }
}
