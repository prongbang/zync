use dioxus::prelude::*;
use crate::*;
use std::collections::HashSet;

#[component]
pub(crate) fn DiffViewer(
    diff: String,
    image_path: String,
    image_before_url: String,
    image_after_url: String,
    on_stage_patch: EventHandler<String>,
    blame_available: bool,
    on_blame: EventHandler<()>,
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
                div { class: "flex shrink-0 gap-1.5",
                    button {
                        class: "rounded-md border border-zinc-700 px-2 py-1 text-[11px] text-zinc-300 hover:bg-zinc-800 disabled:opacity-40",
                        disabled: !blame_available,
                        onclick: move |_| on_blame.call(()),
                        "Blame"
                    }
                    button {
                        class: "rounded-md border border-cyan-700/60 px-2 py-1 text-[11px] text-cyan-200 hover:bg-cyan-500/10 disabled:opacity-40",
                        disabled: !diff_is_patch(&stage_all_patch),
                        onclick: move |_| on_stage_patch.call(stage_all_patch.clone()),
                        "Stage patch"
                    }
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
                                    pre { class: format!("min-w-0 whitespace-pre-wrap break-words px-2 {}", line.old_class),
                                        for segment in line.old.iter().cloned() {
                                            span { class: if segment.changed { "diff-word-removed" } else { "" }, "{segment.text}" }
                                        }
                                    }
                                    pre { class: format!("min-w-0 whitespace-pre-wrap break-words border-l border-zinc-800 px-2 {}", line.new_class),
                                        for segment in line.new.iter().cloned() {
                                            span { class: if segment.changed { "diff-word-added" } else { "" }, "{segment.text}" }
                                        }
                                    }
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
pub(crate) fn ImageDiffPreview(path: String, before_url: String, after_url: String) -> Element {
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

#[component]
pub(crate) fn DiffLineRow(line: DiffLine, selected: bool, on_toggle: EventHandler<String>) -> Element {
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
pub(crate) fn ForkCompactDiff(
    diff: String,
    #[props(default = false)] stage_enabled: bool,
    #[props(default)] on_stage_patch: EventHandler<String>,
) -> Element {
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
                        div { class: "fork-compact-hunk-title",
                            span { class: "min-w-0 truncate", "{hunk.title}" }
                            if stage_enabled && diff_is_patch(&hunk.patch) {
                                button {
                                    class: "fork-hunk-stage",
                                    title: "Stage this hunk",
                                    onclick: {
                                        let patch = hunk.patch.clone();
                                        move |_| on_stage_patch.call(patch.clone())
                                    },
                                    "Stage hunk"
                                }
                            }
                        }
                        for line in hunk.lines {
                            div { class: format!("fork-compact-line {}", compact_diff_class(line.text.as_str())),
                                span { class: "fork-compact-line-marker", "{compact_diff_marker(line.text.as_str())}" }
                                pre { "{compact_diff_text(line.text.as_str())}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub(crate) fn SplitDiffSection(diff: String) -> Element {
    let hunks = diff_hunks(&diff);
    let split_lines = split_diff_lines(&hunks);
    rsx! {
        div { class: "fork-compact-diff",
            if split_lines.is_empty() {
                div { class: "fork-diff-empty", "Select a changed file to show its diff." }
            } else {
                section { class: "rounded-md border border-zinc-800 bg-zinc-950/80 overflow-hidden",
                    div { class: "grid grid-cols-2 border-b border-zinc-800 text-[11px] font-semibold uppercase tracking-wide text-zinc-500",
                        span { class: "px-2 py-1.5", "Old" }
                        span { class: "border-l border-zinc-800 px-2 py-1.5", "New" }
                    }
                    div { class: "font-mono text-xs leading-5",
                        for line in split_lines {
                            div { class: "grid grid-cols-2",
                                pre { class: format!("min-w-0 whitespace-pre-wrap break-words px-2 {}", line.old_class),
                                    for segment in line.old.iter().cloned() {
                                        span { class: if segment.changed { "diff-word-removed" } else { "" }, "{segment.text}" }
                                    }
                                }
                                pre { class: format!("min-w-0 whitespace-pre-wrap break-words border-l border-zinc-800 px-2 {}", line.new_class),
                                    for segment in line.new.iter().cloned() {
                                        span { class: if segment.changed { "diff-word-added" } else { "" }, "{segment.text}" }
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
pub(crate) fn BlameTable(view: BlameView) -> Element {
    rsx! {
        div { class: "min-h-0 flex-1 overflow-auto font-mono text-xs leading-5",
            for row in view.rows.iter().cloned() {
                div { class: "blame-row",
                    span { class: "blame-line-no", "{row.line}" }
                    code { class: "blame-commit", "{short_id(&row.commit)}" }
                    span { class: "blame-author", "{row.author}" }
                    pre { class: "blame-code", "{row.code}" }
                }
            }
        }
    }
}

#[component]
pub(crate) fn BlameViewer(view: BlameView, on_close: EventHandler<()>) -> Element {
    rsx! {
        article { class: "diff-viewer-panel min-h-[320px] md:min-h-[420px] xl:min-h-0 xl:col-start-2 xl:row-start-2 xl:row-span-2 bg-zinc-950 flex flex-col overflow-hidden",
            header { class: "shrink-0 border-b border-zinc-800 px-3 py-2 flex items-center justify-between gap-2",
                h3 { class: "min-w-0 truncate text-xs font-semibold uppercase tracking-wide text-zinc-400",
                    "Blame - {view.path}"
                }
                button {
                    class: "rounded-md border border-zinc-700 px-2 py-1 text-[11px] text-zinc-300 hover:bg-zinc-800",
                    onclick: move |_| on_close.call(()),
                    "Back to diff"
                }
            }
            BlameTable { view }
        }
    }
}
