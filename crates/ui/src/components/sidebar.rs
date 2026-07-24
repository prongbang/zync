use dioxus::prelude::*;
use crate::*;

#[component]
pub(crate) fn ForkSidebarNavigation(
    branches: Vec<api::BranchSummary>,
    stashes: Vec<api::StashSummary>,
    open_menu: Option<String>,
    open_stash_menu: Option<usize>,
    on_open_menu: EventHandler<String>,
    on_open_stash_menu: EventHandler<usize>,
    on_close_menu: EventHandler<()>,
    on_checkout: EventHandler<String>,
    on_branch_command: EventHandler<SidebarBranchCommand>,
    on_stash_command: EventHandler<SidebarStashCommand>,
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
                    ForkSidebarStashRow {
                        stash: stash.clone(),
                        menu_open: open_stash_menu == Some(stash.index),
                        on_open_menu: on_open_stash_menu,
                        on_close_menu,
                        on_command: on_stash_command
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

#[component]
pub(crate) fn ForkSidebarSection(
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
pub(crate) fn ForkRemoteSection(
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
pub(crate) fn ForkSidebarBranchRow(
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
                oncontextmenu: move |event: Event<MouseData>| {
                    event.prevent_default();
                    on_open_menu.call(branch_for_context.clone());
                },
                onclick: move |_| on_checkout.call(branch_for_click.clone()),
                span { class: "min-w-0 truncate", "{display}" }
                if let Some(ahead) = branch.ahead.filter(|count| *count > 0) {
                    span { class: "fork-row-badge", "{ahead}↑" }
                }
                if let Some(behind) = branch.behind.filter(|count| *count > 0) {
                    span { class: "fork-row-badge fork-row-badge-behind", "{behind}↓" }
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
pub(crate) fn ForkBranchContextMenu(
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
pub(crate) fn ForkSidebarStashRow(
    stash: api::StashSummary,
    menu_open: bool,
    on_open_menu: EventHandler<usize>,
    on_close_menu: EventHandler<()>,
    on_command: EventHandler<SidebarStashCommand>,
) -> Element {
    let stash_for_context = stash.index;
    let stash_for_more = stash.index;
    let label = stash_label(&stash);
    rsx! {
        div { class: "fork-sidebar-row-wrap",
            div {
                class: "fork-sidebar-row fork-sidebar-leaf",
                oncontextmenu: move |event: Event<MouseData>| {
                    event.prevent_default();
                    on_open_menu.call(stash_for_context);
                },
                span { class: "fork-stash-icon", "" }
                span { class: "min-w-0 truncate", "{label}" }
                button {
                    class: "fork-row-more",
                    title: "Stash actions",
                    onclick: move |event| {
                        event.stop_propagation();
                        on_open_menu.call(stash_for_more);
                    },
                    span { "More" }
                }
            }
            if menu_open {
                ForkStashContextMenu {
                    stash: stash.clone(),
                    on_close: on_close_menu,
                    on_command
                }
            }
        }
    }
}

#[component]
pub(crate) fn ForkStashContextMenu(
    stash: api::StashSummary,
    on_close: EventHandler<()>,
    on_command: EventHandler<SidebarStashCommand>,
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
            StashContextMenuItem { label: "Apply...".to_string(), command: SidebarStashCommand::Apply(stash.clone()), on_command, on_close }
            div { class: "fork-context-separator" }
            StashContextMenuItem { label: "Drop Stash".to_string(), command: SidebarStashCommand::Drop(stash.index), on_command, on_close }
        }
    }
}

#[component]
pub(crate) fn StashContextMenuItem(
    label: String,
    command: SidebarStashCommand,
    on_command: EventHandler<SidebarStashCommand>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "fork-context-item",
            onclick: move |_| {
                on_command.call(command.clone());
                on_close.call(());
            },
            span { class: "min-w-0 truncate", "{label}" }
        }
    }
}
