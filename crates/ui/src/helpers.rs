use crate::*;
use std::collections::HashSet;

pub(crate) fn clamp_pane_size(value: f64, min: u16, max: u16) -> u16 {
    value.round().clamp(f64::from(min), f64::from(max)) as u16
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn viewport_width() -> Option<f64> {
    web_sys::window()?.inner_width().ok()?.as_f64()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn viewport_width() -> Option<f64> {
    None
}

pub(crate) fn scope_bit(name: &str) -> u8 {
    match name {
        "workspace" => SCOPE_WORKSPACE,
        "status" => SCOPE_STATUS,
        "branches" => SCOPE_BRANCHES,
        "commits" | "graph" => SCOPE_GRAPH,
        "stashes" => SCOPE_STASHES,
        "conflicts" => SCOPE_CONFLICTS,
        "diff" => SCOPE_DIFF,
        _ => SCOPE_ALL,
    }
}

// Map an incoming websocket event to the smallest refresh that covers it.
// Only the wasm websocket loop calls this; the native build has no live sync.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn scope_for_event(text: &str) -> u8 {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return SCOPE_ALL;
    };
    match value.get("kind").and_then(|kind| kind.as_str()).unwrap_or("") {
        "workspace_batch" | "file_created" | "file_changed" | "file_deleted"
        | "folder_created" | "folder_deleted" | "file_renamed" => SCOPE_WORKDIR,
        "git_changed" => value
            .get("payload")
            .and_then(|payload| payload.get("scopes"))
            .and_then(|scopes| scopes.as_array())
            .map(|scopes| {
                scopes
                    .iter()
                    .filter_map(|scope| scope.as_str())
                    .fold(0u8, |acc, name| acc | scope_bit(name))
            })
            .filter(|scope| *scope != 0)
            .unwrap_or(SCOPE_ALL),
        _ => SCOPE_ALL,
    }
}

// Build an interactive-rebase plan for a Fork-style quick action on a single
// commit: reset to the commit's parent, apply the action to the commit, then
// re-pick every descendant already loaded in the graph (linear history only).
pub(crate) fn quick_rebase_plan(
    commits: &[api::CommitSummary],
    target_id: &str,
    action: &str,
    message: Option<String>,
) -> Result<(String, Vec<api::RebaseStepRequest>), String> {
    let index = commits
        .iter()
        .position(|commit| commit.id == target_id)
        .ok_or_else(|| "Commit is not in the loaded graph".to_string())?;
    let target = &commits[index];
    if target.parents.len() != 1 {
        return Err("Quick actions need a commit with exactly one parent".to_string());
    }
    let base = target.parents[0].clone();
    let mut steps = vec![api::RebaseStepRequest {
        commit: target_id.to_string(),
        action: action.to_string(),
        message,
    }];
    for descendant in commits[..index].iter().rev() {
        if descendant.parents.len() > 1 {
            return Err("Quick actions across merge commits are not supported".to_string());
        }
        steps.push(api::RebaseStepRequest {
            commit: descendant.id.clone(),
            action: "pick".to_string(),
            message: None,
        });
    }
    Ok((base, steps))
}

pub(crate) fn github_repo_url(remote_url: &str) -> Option<String> {
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

pub(crate) fn pretty_json<T: serde::Serialize>(value: T) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

pub(crate) fn move_rebase_step(
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

pub(crate) fn drop_rebase_step(
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

pub(crate) fn branch_group_rows(rows: Vec<api::BranchSummary>) -> Vec<(String, Vec<api::BranchSummary>)> {
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

pub(crate) fn branch_leaf_label(branch: &api::BranchSummary, group: &str) -> String {
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

pub(crate) fn stash_label(stash: &api::StashSummary) -> String {
    if stash.message.trim().is_empty() {
        format!("#{} {}", stash.index, stash.name)
    } else {
        format!("stash@{{{}}} {}", stash.index, stash.message.trim())
    }
}

pub(crate) fn is_image_path(path: &str) -> bool {
    matches!(
        path.rsplit('.')
            .next()
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "apng" | "avif" | "gif" | "jpg" | "jpeg" | "png" | "svg" | "webp"
    )
}

pub(crate) fn status_label(file: &api::FileStatus) -> &'static str {
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

pub(crate) fn status_class(file: &api::FileStatus) -> &'static str {
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

pub(crate) fn plain_segments(text: &str) -> Vec<DiffSegment> {
    vec![DiffSegment {
        text: text.to_string(),
        changed: false,
    }]
}

// Pair a run of removals with the following run of additions so both sides of
// the split view stay aligned, and mark the changed part of each paired line.
pub(crate) fn flush_change_run(
    rows: &mut Vec<SplitDiffLine>,
    removed: &mut Vec<String>,
    added: &mut Vec<String>,
) {
    let pairs = removed.len().max(added.len());
    for index in 0..pairs {
        match (removed.get(index), added.get(index)) {
            (Some(old_text), Some(new_text)) => {
                let (old_segments, new_segments) = intra_line_segments(old_text, new_text);
                rows.push(SplitDiffLine {
                    old: old_segments,
                    new: new_segments,
                    old_class: SPLIT_REMOVED_CLASS,
                    new_class: SPLIT_ADDED_CLASS,
                });
            }
            (Some(old_text), None) => rows.push(SplitDiffLine {
                old: plain_segments(old_text),
                new: Vec::new(),
                old_class: SPLIT_REMOVED_CLASS,
                new_class: SPLIT_EMPTY_CLASS,
            }),
            (None, Some(new_text)) => rows.push(SplitDiffLine {
                old: Vec::new(),
                new: plain_segments(new_text),
                old_class: SPLIT_EMPTY_CLASS,
                new_class: SPLIT_ADDED_CLASS,
            }),
            (None, None) => {}
        }
    }
    removed.clear();
    added.clear();
}

// Word-level highlight: split the line into the common prefix, the changed
// middle, and the common suffix. The leading +/- marker always stays plain.
pub(crate) fn intra_line_segments(old_text: &str, new_text: &str) -> (Vec<DiffSegment>, Vec<DiffSegment>) {
    let old_marker = &old_text[..1.min(old_text.len())];
    let new_marker = &new_text[..1.min(new_text.len())];
    let old_chars: Vec<char> = old_text.chars().skip(1).collect();
    let new_chars: Vec<char> = new_text.chars().skip(1).collect();

    let mut prefix = 0usize;
    while prefix < old_chars.len()
        && prefix < new_chars.len()
        && old_chars[prefix] == new_chars[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < old_chars.len() - prefix
        && suffix < new_chars.len() - prefix
        && old_chars[old_chars.len() - 1 - suffix] == new_chars[new_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let build = |marker: &str, chars: &[char]| {
        let mut segments = Vec::new();
        let head: String = chars[..prefix].iter().collect();
        let middle: String = chars[prefix..chars.len() - suffix].iter().collect();
        let tail: String = chars[chars.len() - suffix..].iter().collect();
        segments.push(DiffSegment {
            text: format!("{marker}{head}"),
            changed: false,
        });
        if !middle.is_empty() {
            segments.push(DiffSegment {
                text: middle,
                changed: true,
            });
        }
        if !tail.is_empty() {
            segments.push(DiffSegment {
                text: tail,
                changed: false,
            });
        }
        segments
    };
    (
        build(old_marker, &old_chars),
        build(new_marker, &new_chars),
    )
}

pub(crate) fn split_diff_lines(hunks: &[DiffHunk]) -> Vec<SplitDiffLine> {
    let mut rows = Vec::new();
    for hunk in hunks {
        rows.push(SplitDiffLine {
            old: plain_segments(&hunk.title),
            new: plain_segments(&hunk.title),
            old_class: "bg-cyan-500/10 text-cyan-200",
            new_class: "bg-cyan-500/10 text-cyan-200",
        });
        let mut removed = Vec::new();
        let mut added = Vec::new();
        for line in hunk.lines.iter().skip(1) {
            if line.text.starts_with('-') && !line.text.starts_with("--- ") {
                removed.push(line.text.clone());
            } else if line.text.starts_with('+') && !line.text.starts_with("+++ ") {
                added.push(line.text.clone());
            } else {
                flush_change_run(&mut rows, &mut removed, &mut added);
                rows.push(SplitDiffLine {
                    old: plain_segments(&line.text),
                    new: plain_segments(&line.text),
                    old_class: "text-zinc-400",
                    new_class: "text-zinc-400",
                });
            }
        }
        flush_change_run(&mut rows, &mut removed, &mut added);
    }
    rows
}

pub(crate) fn commit_section_tab_class(active: CommitSectionMode, tab: CommitSectionMode) -> &'static str {
    if active == tab {
        "commit-section-tab commit-section-tab-active"
    } else {
        "commit-section-tab"
    }
}

pub(crate) fn gravatar_url(email: &str, size: u32) -> Option<String> {
    let normalized = email.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let digest = md5::compute(normalized.as_bytes());
    // "mp" (mystery person) keeps unknown authors neutral instead of loud
    // generated identicons; real Gravatar photos still come through.
    Some(format!(
        "https://www.gravatar.com/avatar/{digest:x}?s={size}&d=mp"
    ))
}

pub(crate) fn commit_ref_class(kind: &str) -> &'static str {
    match kind {
        "head" => "commit-ref commit-ref-head",
        "local" => "commit-ref commit-ref-local",
        "remote" => "commit-ref commit-ref-remote",
        "tag" => "commit-ref commit-ref-tag",
        _ => "commit-ref",
    }
}

pub(crate) fn format_commit_time(seconds: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{} {}, {} {:02}:{:02}",
        MONTHS[(month - 1) as usize],
        day,
        year,
        second_of_day / 3600,
        (second_of_day % 3600) / 60,
    )
}

// Howard Hinnant's civil-from-days algorithm; dates rendered in UTC.
pub(crate) fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

pub(crate) fn detail_tab_class(active: ForkDetailTab, tab: ForkDetailTab) -> &'static str {
    if active == tab {
        "fork-detail-tab fork-detail-tab-active"
    } else {
        "fork-detail-tab"
    }
}

pub(crate) fn changed_tree_entries(files: &[api::FileStatus]) -> Vec<ChangedTreeEntry> {
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

pub(crate) fn status_class_from_label(label: &str) -> &'static str {
    match label {
        "A" => "fork-status fork-status-added",
        "U" => "fork-status fork-status-untracked",
        "!" => "fork-status fork-status-conflict",
        _ => "fork-status fork-status-modified",
    }
}

pub(crate) fn compact_diff_class(line: &str) -> &'static str {
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

pub(crate) fn compact_diff_marker(line: &str) -> &'static str {
    if line.starts_with('+') && !line.starts_with("+++") {
        "+"
    } else if line.starts_with('-') && !line.starts_with("---") {
        "-"
    } else {
        ""
    }
}

pub(crate) fn compact_diff_text(line: &str) -> &str {
    if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
        line
    } else {
        line.strip_prefix(['+', '-', ' ']).unwrap_or(line)
    }
}

pub(crate) fn build_blame_rows(ranges: &[api::BlameLine], content: &str) -> Vec<BlameRow> {
    let mut owner = std::collections::HashMap::new();
    for range in ranges {
        for offset in 0..range.line_count {
            owner.insert(
                range.start_line + offset,
                (range.commit.clone(), range.author.clone()),
            );
        }
    }
    content
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line_number = index + 1;
            let (commit, author) = owner.get(&line_number).cloned().unwrap_or_default();
            BlameRow {
                line: line_number,
                commit,
                author,
                code: line.to_string(),
            }
        })
        .collect()
}

pub(crate) fn graph_rows(commits: &[api::CommitSummary]) -> Vec<GraphRow> {
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

        let top_lanes = lanes
            .iter()
            .enumerate()
            .filter_map(|(index, id)| id.as_ref().map(|_| index))
            .collect::<HashSet<_>>();

        if let Some(first_parent) = commit.parents.first() {
            lanes[lane] = Some(first_parent.clone());
        } else {
            lanes[lane] = None;
        }

        let mut merge_lanes = HashSet::new();
        for parent in commit.parents.iter().skip(1) {
            // Merge into a lane that already tracks this parent so merge lines
            // converge like Fork's; otherwise take the first free lane.
            let target = lanes
                .iter()
                .position(|id| id.as_ref() == Some(parent))
                .unwrap_or_else(|| {
                    let next = lanes
                        .iter()
                        .position(Option::is_none)
                        .unwrap_or(lanes.len());
                    if next == lanes.len() {
                        lanes.push(Some(parent.clone()));
                    } else {
                        lanes[next] = Some(parent.clone());
                    }
                    next
                });
            merge_lanes.insert(target);
        }

        let bottom_lanes = lanes
            .iter()
            .enumerate()
            .filter_map(|(index, id)| id.as_ref().map(|_| index))
            .collect::<HashSet<_>>();
        let lane_count = top_lanes
            .iter()
            .chain(bottom_lanes.iter())
            .copied()
            .chain(std::iter::once(lane))
            .max()
            .unwrap_or(0)
            + 1;

        rows.push(GraphRow {
            commit: commit.clone(),
            lane,
            lane_count,
            top_lanes,
            bottom_lanes,
            merge_lanes,
        });

        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
    }

    rows
}

pub(crate) fn lane_color(lane: usize) -> &'static str {
    let colors = [
        "#2dd4bf", "#f59e0b", "#a78bfa", "#fb7185", "#38bdf8", "#34d399", "#f472b6",
    ];
    colors[lane % colors.len()]
}

pub(crate) fn diff_is_patch(diff: &str) -> bool {
    diff.contains("diff --git") && diff.contains("@@")
}

pub(crate) fn diff_hunks(diff: &str) -> Vec<DiffHunk> {
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

pub(crate) fn diff_lines(hunk_index: usize, lines: &[String]) -> Vec<DiffLine> {
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

pub(crate) fn parse_hunk_starts(header: &str) -> Option<(usize, usize)> {
    let mut parts = header.split_whitespace();
    parts.next()?;
    let old_part = parts.next()?.trim_start_matches('-');
    let new_part = parts.next()?.trim_start_matches('+');
    Some((parse_range_start(old_part)?, parse_range_start(new_part)?))
}

pub(crate) fn parse_range_start(value: &str) -> Option<usize> {
    value.split(',').next()?.parse().ok()
}

pub(crate) fn selected_patch_for_hunk(hunk: &DiffHunk, selected: &HashSet<usize>) -> Option<String> {
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

pub(crate) fn build_patch(header: &[String], hunk: &[String]) -> String {
    let mut patch = header.join("\n");
    if !patch.is_empty() {
        patch.push('\n');
    }
    patch.push_str(&hunk.join("\n"));
    patch.push('\n');
    patch
}

pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
