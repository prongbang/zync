use dioxus::prelude::*;
use crate::*;

#[component]
pub(crate) fn StashApplyDialog(
    stash: api::StashSummary,
    delete_after_apply: bool,
    on_delete_after_apply: EventHandler<bool>,
    on_cancel: EventHandler<()>,
    on_submit: EventHandler<()>,
) -> Element {
    let label = stash_label(&stash);
    rsx! {
        div { class: "branch-dialog-layer",
            button {
                class: "branch-dialog-scrim",
                title: "Close dialog",
                onclick: move |_| on_cancel.call(())
            }
            section { class: "branch-dialog stash-apply-dialog",
                div { class: "stash-apply-layout",
                    div { class: "stash-apply-icon", "" }
                    div { class: "stash-apply-content",
                        header { class: "stash-apply-header",
                            h3 { "Apply Stash" }
                            p { "Apply changes of the stash to your working directory" }
                        }
                        div { class: "stash-apply-row",
                            strong { "Stash:" }
                            code { "{label}" }
                        }
                        label { class: "branch-dialog-check stash-apply-check",
                            input {
                                r#type: "checkbox",
                                checked: delete_after_apply,
                                onchange: move |event| on_delete_after_apply.call(event.checked())
                            }
                            span { "Delete stash after applying" }
                        }
                        p { class: "branch-dialog-muted stash-apply-note", "Stash will not be deleted if a conflict occurs" }
                    }
                }
                footer { class: "branch-dialog-footer stash-apply-footer",
                    button {
                        class: "branch-dialog-secondary",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "branch-dialog-primary",
                        onclick: move |_| on_submit.call(()),
                        "Apply"
                    }
                }
            }
        }
    }
}

#[component]
pub(crate) fn BranchActionDialog(
    dialog: BranchDialog,
    value: String,
    target: String,
    checkout: bool,
    local_mode: LocalChangesMode,
    has_local_changes: bool,
    rebase_steps: Vec<api::RebaseStepRequest>,
    on_value: EventHandler<String>,
    on_target: EventHandler<String>,
    on_checkout: EventHandler<bool>,
    on_local_mode: EventHandler<LocalChangesMode>,
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
        BranchDialog::NewBranch { .. } => {
            if checkout {
                "Create and Checkout"
            } else {
                "Create Branch"
            }
        }
        BranchDialog::NewTag { .. } => "Create Tag",
        BranchDialog::Rename { .. } => "Rename",
        BranchDialog::Delete { .. } => "Delete",
        BranchDialog::RewordCommit { .. } => "Reword",
        BranchDialog::ResetToCommit { .. } => "Reset",
        BranchDialog::DropCommit { .. } => "Drop",
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
                        BranchDialog::RewordCommit { .. } => rsx! {
                            label { class: "branch-dialog-field",
                                span { "New commit message" }
                                input {
                                    value: "{value}",
                                    oninput: move |event| on_value.call(event.value())
                                }
                            }
                            p { class: "fork-muted", "Rewrites this commit and re-applies every later commit." }
                        },
                        BranchDialog::ResetToCommit { .. } => rsx! {
                            p { "Move the current branch to this commit." }
                            code { class: "branch-dialog-code", "{branch}" }
                            div { class: "branch-dialog-field",
                                span { "Mode" }
                                label { class: "branch-dialog-radio",
                                    input {
                                        r#type: "radio",
                                        name: "reset-mode",
                                        checked: target != "hard",
                                        onchange: move |_| on_target.call("mixed".to_string())
                                    }
                                    span { "Mixed - keep changes in the working tree" }
                                }
                                label { class: "branch-dialog-radio",
                                    input {
                                        r#type: "radio",
                                        name: "reset-mode",
                                        checked: target == "hard",
                                        onchange: move |_| on_target.call("hard".to_string())
                                    }
                                    span { class: "branch-dialog-radio-danger", "Hard - discard all changes after this commit" }
                                }
                            }
                        },
                        BranchDialog::DropCommit { .. } => rsx! {
                            p { "Remove this commit from history and re-apply every later commit. This cannot be undone from Zync." }
                            code { class: "branch-dialog-code", "{branch}" }
                        },
                        BranchDialog::NewBranch { target: base_target, .. } => rsx! {
                            div { class: "branch-dialog-field",
                                span { "Create branch at" }
                                code { class: "branch-dialog-code", "{base_target.clone().unwrap_or_else(|| branch.clone())}" }
                            }
                            label { class: "branch-dialog-field",
                                span { "Branch name" }
                                input {
                                    placeholder: "feature/name",
                                    value: "{value}",
                                    oninput: move |event| on_value.call(event.value())
                                }
                            }
                            label { class: "branch-dialog-field",
                                span { "Start point (optional override)" }
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
                                span { "Check out after create" }
                            }
                            div { class: "branch-dialog-field",
                                span {
                                    if has_local_changes {
                                        "Local changes"
                                    } else {
                                        "Local changes (working tree is clean)"
                                    }
                                }
                                label { class: "branch-dialog-radio",
                                    input {
                                        r#type: "radio",
                                        name: "branch-local-changes",
                                        disabled: !checkout,
                                        checked: local_mode == LocalChangesMode::DontChange,
                                        onchange: move |_| on_local_mode.call(LocalChangesMode::DontChange)
                                    }
                                    span { "Don't change" }
                                }
                                label { class: "branch-dialog-radio",
                                    input {
                                        r#type: "radio",
                                        name: "branch-local-changes",
                                        disabled: !checkout,
                                        checked: local_mode == LocalChangesMode::StashReapply,
                                        onchange: move |_| on_local_mode.call(LocalChangesMode::StashReapply)
                                    }
                                    span { "Stash and reapply" }
                                }
                                label { class: "branch-dialog-radio",
                                    input {
                                        r#type: "radio",
                                        name: "branch-local-changes",
                                        disabled: !checkout,
                                        checked: local_mode == LocalChangesMode::Discard,
                                        onchange: move |_| on_local_mode.call(LocalChangesMode::Discard)
                                    }
                                    span { class: "branch-dialog-radio-danger", "Discard" }
                                }
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
pub(crate) fn ContextMenuItem(
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
pub(crate) fn RepositorySelector(
    repositories: Vec<api::RepositoryRecord>,
    selected_repository_id: String,
    current_branch: String,
    on_open: EventHandler<String>,
    on_favorite: EventHandler<(String, bool)>,
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
            div { class: "fork-repository-selector-head",
                label { class: "fork-repository-label", "Repository" }
                if let Some(repository) = selected_repository.clone() {
                    button {
                        class: if repository.favorite { "fork-repository-favorite fork-repository-favorite-active" } else { "fork-repository-favorite" },
                        title: if repository.favorite { "Remove from favorites" } else { "Add to favorites" },
                        onclick: move |_| on_favorite.call((repository.id.clone(), !repository.favorite)),
                        if repository.favorite { "Favorite" } else { "Mark favorite" }
                    }
                } else {
                    button {
                        class: "fork-repository-favorite",
                        disabled: true,
                        "Mark favorite"
                    }
                }
            }
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
