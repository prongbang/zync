use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{mpsc::channel, Arc, Mutex},
    thread,
    time::Duration,
};

use crate::websocket::{WorkspaceEvent, WorkspaceHub};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Clone, Default)]
pub struct WorkspaceSync {
    watchers: Arc<Mutex<HashSet<String>>>,
}

impl WorkspaceSync {
    pub fn watch(&self, workspace_id: String, root: PathBuf, hub: WorkspaceHub) {
        let key = format!("{}:{}", workspace_id, root.display());
        let mut watchers = self.watchers.lock().expect("workspace sync lock");
        if !watchers.insert(key) {
            return;
        }
        drop(watchers);
        spawn_workspace_watcher(workspace_id, root, hub);
    }

    /// Number of active (workspace, root) file watchers — a gauge for
    /// `/metrics` (P5.3, `observability::Metrics::render_prometheus`).
    pub fn watcher_count(&self) -> usize {
        self.watchers.lock().expect("workspace sync lock").len()
    }
}

pub fn list_workspace_files(root: impl AsRef<Path>) -> anyhow::Result<Vec<FileNode>> {
    let root = root.as_ref();
    let mut nodes = Vec::new();
    visit(root, root, &mut nodes)?;
    nodes.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.path.cmp(&b.path)));
    Ok(nodes)
}

fn spawn_workspace_watcher(workspace_id: String, root: PathBuf, hub: WorkspaceHub) {
    // Resolve symlinks up front. On macOS the FSEvents backend reports every event
    // path in canonical form (e.g. `/private/tmp/...` for a repo registered under
    // `/tmp/...`), so if we strip against the raw, un-canonicalized root the prefix
    // never matches and every raw file change is silently dropped. Canonicalizing the
    // root here keeps `strip_prefix` in `event_for_path` aligned with what the watcher
    // actually delivers (also covers `/var`, symlinked homes, and Docker mounts).
    let root = resolve_watch_root(&root);
    thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(watcher) => watcher,
            Err(error) => {
                tracing::error!("failed to create file watcher: {error}");
                return;
            }
        };
        if let Err(error) = watcher.watch(&root, RecursiveMode::Recursive) {
            tracing::error!("failed to watch workspace {}: {error}", root.display());
            return;
        }

        let mut pending: HashMap<String, WorkspaceEvent> = HashMap::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(120)) {
                Ok(Ok(event)) => {
                    for path in event.paths {
                        if path.components().any(|part| part.as_os_str() == ".git") {
                            continue;
                        }
                        if let Some(workspace_event) = event_for_path(&root, &event.kind, path) {
                            let key = workspace_event.path.clone().unwrap_or_default();
                            pending.insert(key, workspace_event);
                        }
                    }
                }
                Ok(Err(error)) => {
                    tracing::warn!("workspace watcher event error: {error}");
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !pending.is_empty() {
                        broadcast_batch(&workspace_id, &hub, &mut pending);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    if !pending.is_empty() {
                        broadcast_batch(&workspace_id, &hub, &mut pending);
                    }
                    break;
                }
            }
        }
    });
}

/// Canonicalize the watch root so it matches the (symlink-resolved) paths the notify
/// backend reports. Falls back to the raw path when canonicalization fails (e.g. the
/// directory does not exist yet) so watching still starts and fails loudly at `watch()`
/// for that non-existent-dir case.
fn resolve_watch_root(root: &Path) -> PathBuf {
    fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
}

fn event_for_path(root: &Path, kind: &EventKind, path: PathBuf) -> Option<WorkspaceEvent> {
    let relative = path
        .strip_prefix(root)
        .ok()
        .map(|relative| relative.to_string_lossy().to_string())
        .filter(|relative| !relative.is_empty())?;
    let event_kind = match kind {
        EventKind::Create(_) => {
            if path.is_dir() {
                "folder_created"
            } else {
                "file_created"
            }
        }
        EventKind::Remove(_) => {
            if relative.ends_with('/') {
                "folder_deleted"
            } else {
                "file_deleted"
            }
        }
        EventKind::Modify(_) => "file_changed",
        _ => "file_changed",
    };

    let mut event = WorkspaceEvent::new(event_kind);
    event.path = Some(relative);
    if matches!(event_kind, "file_created" | "file_changed") {
        event.payload = file_payload(&path);
    }
    Some(event)
}

fn file_payload(path: &Path) -> serde_json::Value {
    const MAX_INLINE_FILE: u64 = 256 * 1024;
    let metadata = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_INLINE_FILE => metadata,
        _ => return serde_json::json!({ "inline": false }),
    };
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return serde_json::json!({ "inline": false }),
    };
    let revision = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().to_string());
    serde_json::json!({
        "inline": true,
        "content": content,
        "revision": revision,
        "size": metadata.len()
    })
}

fn broadcast_batch(
    workspace_id: &str,
    hub: &WorkspaceHub,
    pending: &mut HashMap<String, WorkspaceEvent>,
) {
    let events = pending.drain().map(|(_, event)| event).collect::<Vec<_>>();
    let mut batch = WorkspaceEvent::new("workspace_batch");
    batch.payload = serde_json::json!({ "events": events });
    hub.broadcast(workspace_id, batch);
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind};

    #[cfg(unix)]
    #[test]
    fn resolve_watch_root_canonicalizes_symlinked_prefix() {
        // A repo registered under a symlinked path (on macOS `/tmp` -> `/private/tmp`,
        // but reproduced explicitly here so it also exercises symlink resolution on
        // Linux CI where `temp_dir()` has no symlinked prefix): the FSEvents backend
        // reports events under the canonical path, so the watch root must be
        // canonicalized to keep `strip_prefix` matching. This is the regression that
        // dropped every raw file-change event.
        let base = std::env::temp_dir().join(format!("zync-sync-test-{}", std::process::id()));
        fs::remove_dir_all(&base).ok();
        let real = base.join("real-repo");
        fs::create_dir_all(&real).unwrap();
        let link = base.join("link-repo");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        // Resolving the symlinked path must yield the canonical real target, not the
        // link path notify was handed.
        let resolved = resolve_watch_root(&link);
        let canonical_real = fs::canonicalize(&real).unwrap();
        assert_eq!(resolved, canonical_real);
        assert_ne!(resolved, link);

        // A file event delivered under the canonical (real) root must strip against the
        // resolved root and yield the correct relative path — the end-to-end effect of
        // the fix.
        let file = canonical_real.join("changed.txt");
        let event = event_for_path(
            &resolved,
            &EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            file,
        )
        .expect("canonical event path should strip against resolved root");
        assert_eq!(event.kind, "file_changed");
        assert_eq!(event.path.as_deref(), Some("changed.txt"));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn resolve_watch_root_falls_back_when_missing() {
        let missing = PathBuf::from("/this/path/does/not/exist/zync");
        assert_eq!(resolve_watch_root(&missing), missing);
    }

    #[test]
    fn event_for_path_skips_the_root_itself() {
        let root = PathBuf::from("/private/tmp/repo");
        assert!(
            event_for_path(&root, &EventKind::Create(CreateKind::Folder), root.clone()).is_none(),
            "an event for the watched root itself must not produce an empty-path event"
        );
    }

    #[test]
    fn event_for_path_ignores_paths_outside_root() {
        let root = PathBuf::from("/private/tmp/repo");
        let outside = PathBuf::from("/private/tmp/other/file.txt");
        assert!(event_for_path(
            &root,
            &EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Content)),
            outside
        )
        .is_none());
    }
}

fn visit(root: &Path, current: &Path, nodes: &mut Vec<FileNode>) -> anyhow::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.components().any(|part| part.as_os_str() == ".git") {
            continue;
        }
        let metadata = entry.metadata()?;
        let relative = path.strip_prefix(root)?.to_string_lossy().to_string();
        nodes.push(FileNode {
            name: entry.file_name().to_string_lossy().to_string(),
            path: relative,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
        });
        if metadata.is_dir() {
            visit(root, &path, nodes)?;
        }
    }
    Ok(())
}
