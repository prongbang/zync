use crate::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ResizeDragTarget {
    Sidebar,
    LeftPane,
    Inspector,
    History,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum RepoAddMode {
    Folder,
    GitUrl,
}

#[derive(Clone, PartialEq)]
pub(crate) enum SidebarBranchCommand {
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
pub(crate) enum SidebarStashCommand {
    Apply(api::StashSummary),
    Drop(usize),
}

#[derive(Clone, PartialEq)]
pub(crate) enum CommitMenuCommand {
    NewBranch(String),
    NewTag(String),
    RebaseToHere(String),
    InteractiveRebase(String),
    Reword(String),
    EditCommit(String),
    SquashIntoParent(String),
    FixupIntoParent(String),
    DropCommit(String),
    ResetToHere(String),
    CheckoutCommit(String),
    CherryPick(String),
    Revert(String),
    SaveAsPatch(String),
    CompareToLocal(String),
    CopySha(String),
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LocalChangesMode {
    DontChange,
    StashReapply,
    Discard,
}

#[derive(Clone, PartialEq)]
pub(crate) enum BranchDialog {
    Checkout {
        branch: String,
    },
    Merge {
        branch: String,
    },
    Rebase {
        branch: String,
        interactive: bool,
    },
    NewBranch {
        branch: String,
        target: Option<String>,
    },
    RewordCommit {
        commit: String,
    },
    ResetToCommit {
        commit: String,
    },
    DropCommit {
        commit: String,
    },
    NewTag {
        branch: String,
        target: Option<String>,
    },
    Rename {
        branch: String,
    },
    Delete {
        branch: String,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ToastKind {
    Success,
    Error,
}

#[derive(Clone, PartialEq)]
pub(crate) struct ToastMessage {
    pub(crate) kind: ToastKind,
    pub(crate) title: String,
    pub(crate) detail: String,
}

impl BranchDialog {
    pub(crate) fn title(&self) -> &'static str {
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
            BranchDialog::RewordCommit { .. } => "Reword Commit",
            BranchDialog::ResetToCommit { .. } => "Reset to Commit",
            BranchDialog::DropCommit { .. } => "Drop Commit",
        }
    }

    pub(crate) fn branch(&self) -> &str {
        match self {
            BranchDialog::Checkout { branch }
            | BranchDialog::Merge { branch }
            | BranchDialog::Rebase { branch, .. }
            | BranchDialog::NewBranch { branch, .. }
            | BranchDialog::NewTag { branch, .. }
            | BranchDialog::Rename { branch }
            | BranchDialog::Delete { branch } => branch,
            BranchDialog::RewordCommit { commit }
            | BranchDialog::ResetToCommit { commit }
            | BranchDialog::DropCommit { commit } => commit,
        }
    }

    pub(crate) fn is_dangerous(&self) -> bool {
        matches!(
            self,
            BranchDialog::Delete { .. }
                | BranchDialog::ResetToCommit { .. }
                | BranchDialog::DropCommit { .. }
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FileAction {
    Stage,
    Unstage,
    Discard,
}

pub(crate) enum FileTreeAction {
    Create(String, bool),
    Rename(String, String),
    Delete(String),
}

pub(crate) enum BranchAction {
    Create(String),
    Checkout(String),
    Merge(String),
    Delete(String),
    Rename(String, String),
}

pub(crate) enum TagAction {
    Create(String, String),
}

pub(crate) enum RemoteAction {
    Fetch,
    Pull,
    Push,
}

pub(crate) enum StashAction {
    Create(String),
    Apply(usize),
    Pop(usize),
    Drop(usize),
}

pub(crate) enum HistoryAction {
    CherryPick(Vec<String>),
    CherryAbort,
    Rebase(String, Vec<api::RebaseStepRequest>),
    Resolve(String, String),
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ToolAction {
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

pub(crate) const SCOPE_WORKSPACE: u8 = 1 << 0;
pub(crate) const SCOPE_STATUS: u8 = 1 << 1;
pub(crate) const SCOPE_BRANCHES: u8 = 1 << 2;
pub(crate) const SCOPE_GRAPH: u8 = 1 << 3;
pub(crate) const SCOPE_STASHES: u8 = 1 << 4;
pub(crate) const SCOPE_CONFLICTS: u8 = 1 << 5;
pub(crate) const SCOPE_DIFF: u8 = 1 << 6;
pub(crate) const SCOPE_ALL: u8 = 0x7f;
// A workdir file change can only move status, the workdir diff, and conflicts.
pub(crate) const SCOPE_WORKDIR: u8 = SCOPE_STATUS | SCOPE_DIFF | SCOPE_CONFLICTS;

#[derive(Clone, PartialEq)]
pub(crate) enum CommitQuickAction {
    Checkout(String),
    CherryPick(String),
    Revert(String),
    Reset(String, bool),
}

#[derive(Clone, PartialEq)]
pub(crate) struct DiffSegment {
    pub(crate) text: String,
    pub(crate) changed: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct SplitDiffLine {
    pub(crate) old: Vec<DiffSegment>,
    pub(crate) new: Vec<DiffSegment>,
    pub(crate) old_class: &'static str,
    pub(crate) new_class: &'static str,
}

pub(crate) const SPLIT_REMOVED_CLASS: &str = "bg-red-500/10 text-red-200";
pub(crate) const SPLIT_ADDED_CLASS: &str = "bg-emerald-500/10 text-emerald-200";
pub(crate) const SPLIT_EMPTY_CLASS: &str = "text-zinc-700";

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum CommitSectionMode {
    LocalChanges,
    Commits,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ToolbarGlyph {
    Fetch,
    Pull,
    Push,
    Stash,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ForkDetailTab {
    Commit,
    Changes,
    FileTree,
    GitTools,
    Repository,
}

#[derive(Clone)]
pub(crate) struct ChangedTreeEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) depth: usize,
    pub(crate) is_file: bool,
    pub(crate) status: String,
}

#[derive(Clone, PartialEq)]
pub(crate) struct BlameView {
    pub(crate) path: String,
    pub(crate) rows: Vec<BlameRow>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct BlameRow {
    pub(crate) line: usize,
    pub(crate) commit: String,
    pub(crate) author: String,
    pub(crate) code: String,
}

#[derive(Clone, PartialEq)]
pub(crate) struct GraphRow {
    pub(crate) commit: api::CommitSummary,
    pub(crate) lane: usize,
    pub(crate) lane_count: usize,
    // Lanes connected to the row above / below; the commit's own lane appears
    // in top_lanes only when it was already tracked (not a branch tip) and in
    // bottom_lanes only when the commit has a parent (not a root).
    pub(crate) top_lanes: HashSet<usize>,
    pub(crate) bottom_lanes: HashSet<usize>,
    pub(crate) merge_lanes: HashSet<usize>,
}

#[derive(Clone)]
pub(crate) struct DiffHunk {
    pub(crate) title: String,
    pub(crate) header: Vec<String>,
    pub(crate) old_start: usize,
    pub(crate) new_start: usize,
    pub(crate) lines: Vec<DiffLine>,
    pub(crate) patch: String,
}

#[derive(Clone, PartialEq)]
pub(crate) struct DiffLine {
    pub(crate) key: String,
    pub(crate) index: usize,
    pub(crate) text: String,
    pub(crate) selectable: bool,
    pub(crate) row_class: &'static str,
}
