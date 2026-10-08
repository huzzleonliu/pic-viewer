use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::fs::FsEntry;

pub const DIR_LISTING_CAP: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedDir {
    pub epoch: u64,
    pub entries: Option<Result<Vec<FsEntry>, String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirListingStore {
    by_path: HashMap<String, ListedDir>,
    recency: Vec<String>,
}

impl DirListingStore {
    pub fn get(&self, path: &str) -> Option<&ListedDir> {
        self.by_path.get(path)
    }

    pub fn ok_entries(&self) -> HashMap<String, Vec<FsEntry>> {
        self.by_path
            .iter()
            .filter_map(|(k, v)| {
                v.entries
                    .as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .map(|e| (k.clone(), e.clone()))
            })
            .collect()
    }

    fn touch(&mut self, path: &str) {
        self.recency.retain(|p| p != path);
        self.recency.push(path.to_string());
    }

    pub fn begin_fetch(&mut self, path: &str, epoch: u64) -> bool {
        if let Some(cur) = self.by_path.get(path) {
            if cur.epoch == epoch {
                self.touch(path);
                return false;
            }
        }
        self.by_path.insert(
            path.to_string(),
            ListedDir {
                epoch,
                entries: None,
            },
        );
        self.touch(path);
        true
    }

    pub fn finish(
        &mut self,
        path: String,
        epoch: u64,
        result: Result<Vec<FsEntry>, String>,
        keep: &HashSet<String>,
    ) {
        match self.by_path.get(&path) {
            Some(cur) if cur.epoch == epoch => {}
            _ => return,
        }
        self.by_path.insert(
            path.clone(),
            ListedDir {
                epoch,
                entries: Some(result),
            },
        );
        self.touch(&path);
        self.evict(keep);
    }

    pub fn forget_path(&mut self, path: &str) {
        if path.is_empty() {
            return;
        }
        let prefix = format!("{path}/");
        self.by_path
            .retain(|k, _| k != path && !k.starts_with(&prefix));
        self.recency
            .retain(|k| k != path && !k.starts_with(&prefix));
    }

    pub fn retarget_with(&mut self, map: impl Fn(&str) -> String) {
        let mut next = HashMap::new();
        for (k, mut listed) in std::mem::take(&mut self.by_path) {
            if let Some(Ok(entries)) = listed.entries.as_mut() {
                for entry in entries.iter_mut() {
                    entry.path = map(&entry.path);
                }
            }
            next.insert(map(&k), listed);
        }
        self.by_path = next;
        for path in &mut self.recency {
            *path = map(path);
        }
    }

    fn evict(&mut self, keep: &HashSet<String>) {
        while self.by_path.len() > DIR_LISTING_CAP {
            let Some(victim) = self
                .recency
                .iter()
                .find(|p| !keep.contains(*p) && !p.is_empty())
                .cloned()
            else {
                break;
            };
            self.by_path.remove(&victim);
            self.recency.retain(|p| p != &victim);
        }
    }
}

pub type SelectedItem = super::fs::FsItem;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckedList {
    items: Vec<SelectedItem>,
    index: HashSet<String>,
}

impl CheckedList {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.index.contains(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SelectedItem> {
        self.items.iter()
    }

    pub fn to_vec(&self) -> Vec<SelectedItem> {
        self.items.clone()
    }

    pub fn set_item(&mut self, item: SelectedItem, on: bool) {
        let path = item.path.clone();
        let idx = self.items.iter().position(|x| x.path == path);
        match (on, idx) {
            (true, None) => {
                self.index.insert(path);
                self.items.push(item);
            }
            (true, Some(i)) => {
                self.items[i] = item;
            }
            (false, Some(i)) => {
                self.index.remove(&path);
                self.items.remove(i);
            }
            _ => {}
        }
    }

    pub fn insert_missing(&mut self, item: SelectedItem) {
        if self.index.insert(item.path.clone()) {
            self.items.push(item);
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.index.clear();
    }

    pub fn retain(&mut self, mut pred: impl FnMut(&SelectedItem) -> bool) {
        self.items.retain(|item| pred(item));
        self.reindex();
    }

    pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut SelectedItem)) {
        for item in &mut self.items {
            f(item);
        }
        self.reindex();
    }

    fn reindex(&mut self) {
        self.index = self.items.iter().map(|item| item.path.clone()).collect();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clipboard {
    pub items: Vec<super::fs::FsItem>,
    pub mode: ClipboardMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureItem {
    pub file: String,
    pub error: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchRenameReport {
    pub ok: u32,
    pub renamed: Vec<(String, String)>,
    pub failures: Vec<FailureItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompressImagesReport {
    pub ok: u32,
    pub skipped: u32,
    pub converted: Vec<(String, String)>,
    pub failures: Vec<FailureItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureReport {
    pub title: String,
    pub failures: Vec<FailureItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchRenameDraft {
    pub name: String,
    pub start: String,
    pub end: String,
    pub file_count: usize,
}

#[derive(Clone, Copy)]
pub struct PanelFlags {
    pub thumbnails: RwSignal<bool>,
    pub adjust: RwSignal<bool>,
    pub file_manager: RwSignal<bool>,
    pub stars: RwSignal<bool>,
    pub filter: RwSignal<bool>,
    pub export: RwSignal<bool>,
    pub crop: RwSignal<bool>,
    pub editor_settings: RwSignal<bool>,
}

#[derive(Clone, Copy)]
pub struct EditorPrefs {
    pub font_size: RwSignal<u32>,
    pub light: RwSignal<bool>,
    pub line_numbers: RwSignal<bool>,
    pub word_wrap: RwSignal<bool>,
}

#[derive(Clone, Copy)]
pub struct ExplorerState {
    pub selected: RwSignal<Option<SelectedItem>>,
    pub clipboard: RwSignal<Option<Clipboard>>,
    pub checked: RwSignal<CheckedList>,
    pub check_anchor: RwSignal<Option<String>>,
    pub dir_listings: RwSignal<DirListingStore>,
    pub viewed: RwSignal<Option<String>>,
    pub browse_dir: RwSignal<Option<String>>,
    pub include_subdirs: RwSignal<bool>,
    pub tree_epoch: RwSignal<u64>,
    pub gallery_epoch: RwSignal<u64>,
    pub media_rev: RwSignal<u64>,
    pub status: RwSignal<String>,
    pub confirm_delete: RwSignal<Option<Vec<SelectedItem>>>,
    pub rename_target: RwSignal<Option<SelectedItem>>,
    pub rename_draft: RwSignal<String>,
    pub batch_rename: RwSignal<Option<BatchRenameDraft>>,
    pub mkdir_parent: RwSignal<Option<String>>,
    pub mkdir_draft: RwSignal<String>,
    pub mkfile_parent: RwSignal<Option<String>>,
    pub mkfile_draft: RwSignal<String>,
    pub busy: RwSignal<bool>,
    pub panels: PanelFlags,
    pub editor: EditorPrefs,
    pub filter_paths: RwSignal<Option<HashSet<String>>>,
    pub failure_report: RwSignal<Option<FailureReport>>,
    pub expanded_dirs: RwSignal<HashSet<String>>,
}

impl ExplorerState {
    pub fn new() -> Self {
        Self {
            selected: RwSignal::new(Some(SelectedItem::root())),
            clipboard: RwSignal::new(None),
            checked: RwSignal::new(CheckedList::default()),
            check_anchor: RwSignal::new(None),
            dir_listings: RwSignal::new(DirListingStore::default()),
            viewed: RwSignal::new(None),
            browse_dir: RwSignal::new(None),
            include_subdirs: RwSignal::new(false),
            tree_epoch: RwSignal::new(0),
            gallery_epoch: RwSignal::new(0),
            media_rev: RwSignal::new(0),
            status: RwSignal::new("就绪".into()),
            confirm_delete: RwSignal::new(None),
            rename_target: RwSignal::new(None),
            rename_draft: RwSignal::new(String::new()),
            batch_rename: RwSignal::new(None),
            mkdir_parent: RwSignal::new(None),
            mkdir_draft: RwSignal::new(String::new()),
            mkfile_parent: RwSignal::new(None),
            mkfile_draft: RwSignal::new(String::new()),
            busy: RwSignal::new(false),
            panels: PanelFlags {
                thumbnails: RwSignal::new(true),
                adjust: RwSignal::new(true),
                file_manager: RwSignal::new(true),
                stars: RwSignal::new(false),
                filter: RwSignal::new(false),
                export: RwSignal::new(false),
                crop: RwSignal::new(false),
                editor_settings: RwSignal::new(false),
            },
            editor: EditorPrefs {
                font_size: RwSignal::new(15),
                light: RwSignal::new(false),
                line_numbers: RwSignal::new(true),
                word_wrap: RwSignal::new(false),
            },
            filter_paths: RwSignal::new(None),
            failure_report: RwSignal::new(None),
            expanded_dirs: RwSignal::new(HashSet::from([String::new()])),
        }
    }

    pub fn bump_tree(self) {
        let _ = self.tree_epoch.try_update(|n| *n += 1);
    }

    pub fn bump_gallery(self) {
        let _ = self.gallery_epoch.try_update(|n| *n += 1);
    }

    pub fn bump_listings(self) {
        self.bump_tree();
        self.bump_gallery();
    }

    pub fn is_text_mode(self) -> bool {
        self.selected
            .try_with(|s| s.as_ref().is_some_and(|item| item.is_text))
            .unwrap_or(false)
    }

    pub fn is_expanded(self, path: &str) -> bool {
        self.expanded_dirs
            .try_with(|set| set.contains(path))
            .unwrap_or(false)
    }

    pub fn toggle_expanded(self, path: &str) {
        let path = path.to_string();
        self.expanded_dirs.try_update(|set| {
            if !set.remove(&path) {
                set.insert(path);
            }
        });
    }

    pub fn report_failures(self, title: &str, failures: Vec<FailureItem>) {
        if failures.is_empty() {
            return;
        }
        let _ = self.failure_report.try_update(|v| {
            *v = Some(FailureReport {
                title: title.to_string(),
                failures,
            });
        });
    }
}

#[cfg(test)]
mod listing_store_tests {
    use super::{DirListingStore, DIR_LISTING_CAP};
    use crate::structure::FsEntry;
    use std::collections::HashSet;

    fn file(path: &str) -> FsEntry {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        FsEntry {
            name,
            path: path.to_string(),
            is_dir: false,
            is_image: path.ends_with(".jpg"),
            is_text: path.ends_with(".txt"),
        }
    }

    #[test]
    fn same_epoch_does_not_refetch() {
        let mut store = DirListingStore::default();
        let keep = HashSet::from([String::new()]);
        assert!(store.begin_fetch("", 1));
        store.finish(String::new(), 1, Ok(vec![file("a.txt")]), &keep);
        assert!(!store.begin_fetch("", 1));
        assert!(store.begin_fetch("", 2));
    }

    #[test]
    fn forget_drops_path_and_children() {
        let mut store = DirListingStore::default();
        let keep = HashSet::from([String::new(), "photos".into()]);
        store.begin_fetch("", 1);
        store.finish(String::new(), 1, Ok(vec![file("photos/a.jpg")]), &keep);
        store.begin_fetch("photos", 1);
        store.finish("photos".into(), 1, Ok(vec![file("photos/a.jpg")]), &keep);
        store.forget_path("photos");
        assert!(store.get("photos").is_none());
        assert!(store.get("").is_some());
    }

    #[test]
    fn evicts_unexpanded_when_over_cap() {
        let mut store = DirListingStore::default();
        let keep = HashSet::from([String::new()]);
        store.begin_fetch("", 1);
        store.finish(String::new(), 1, Ok(Vec::new()), &keep);
        for i in 0..=DIR_LISTING_CAP {
            let path = format!("d{i}");
            store.begin_fetch(&path, 1);
            store.finish(path, 1, Ok(Vec::new()), &keep);
        }
        assert!(store.by_path.len() <= DIR_LISTING_CAP);
        assert!(store.get("").is_some());
    }
}

#[cfg(test)]
mod checked_list_tests {
    use super::CheckedList;
    use crate::structure::FsItem;

    fn item(path: &str) -> FsItem {
        FsItem::from_kind(
            path.rsplit('/').next().unwrap_or(path).into(),
            path.into(),
            crate::structure::FsKind::Other,
        )
    }

    #[test]
    fn keeps_order_and_dedups() {
        let mut list = CheckedList::default();
        list.insert_missing(item("b"));
        list.insert_missing(item("a"));
        list.insert_missing(item("b"));
        let paths: Vec<_> = list.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["b", "a"]);
        list.set_item(item("b"), false);
        let paths: Vec<_> = list.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["a"]);
    }
}
