use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_image: bool,
    pub is_text: bool,
}

impl SelectedItem {
    pub fn from_image_path(path: String) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
        Self {
            path,
            name,
            is_dir: false,
            is_image: true,
            is_text: false,
        }
    }

    pub fn root() -> Self {
        Self {
            path: String::new(),
            name: "/".into(),
            is_dir: true,
            is_image: false,
            is_text: false,
        }
    }
}

impl From<&super::fs::FsEntry> for SelectedItem {
    fn from(entry: &super::fs::FsEntry) -> Self {
        Self {
            path: entry.path.clone(),
            name: entry.name.clone(),
            is_dir: entry.is_dir,
            is_image: entry.is_image,
            is_text: entry.is_text,
        }
    }
}

impl From<super::fs::FsEntry> for SelectedItem {
    fn from(entry: super::fs::FsEntry) -> Self {
        Self {
            path: entry.path,
            name: entry.name,
            is_dir: entry.is_dir,
            is_image: entry.is_image,
            is_text: entry.is_text,
        }
    }
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

impl From<&SelectedItem> for ClipboardItem {
    fn from(item: &SelectedItem) -> Self {
        Self {
            path: item.path.clone(),
            name: item.name.clone(),
            is_dir: item.is_dir,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clipboard {
    pub items: Vec<ClipboardItem>,
    pub mode: ClipboardMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureItem {
    pub file: String,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureReport {
    pub title: String,
    pub failures: Vec<FailureItem>,
}

#[derive(Clone, Copy)]
pub struct ExplorerState {
    pub selected: RwSignal<Option<SelectedItem>>,
    pub clipboard: RwSignal<Option<Clipboard>>,
    pub checked: RwSignal<CheckedList>,
    pub viewed: RwSignal<Option<String>>,
    pub refresh: RwSignal<u64>,
    pub media_rev: RwSignal<u64>,
    pub status: RwSignal<String>,
    pub confirm_delete: RwSignal<Option<Vec<SelectedItem>>>,
    pub rename_target: RwSignal<Option<SelectedItem>>,
    pub rename_draft: RwSignal<String>,
    pub mkdir_parent: RwSignal<Option<String>>,
    pub mkdir_draft: RwSignal<String>,
    pub mkfile_parent: RwSignal<Option<String>>,
    pub mkfile_draft: RwSignal<String>,
    pub busy: RwSignal<bool>,
    pub show_thumbnails: RwSignal<bool>,
    pub show_adjust: RwSignal<bool>,
    pub show_file_manager: RwSignal<bool>,
    pub show_stars: RwSignal<bool>,
    pub show_filter: RwSignal<bool>,
    pub show_export: RwSignal<bool>,
    pub show_editor_settings: RwSignal<bool>,
    pub editor_font_size: RwSignal<u32>,
    pub editor_light: RwSignal<bool>,
    pub editor_line_numbers: RwSignal<bool>,
    pub filter_paths: RwSignal<Option<HashSet<String>>>,
    pub failure_report: RwSignal<Option<FailureReport>>,
    pub expanded_dirs: RwSignal<HashSet<String>>,
}

impl ExplorerState {
    pub fn new() -> Self {
        Self {
            selected: RwSignal::new(None),
            clipboard: RwSignal::new(None),
            checked: RwSignal::new(CheckedList::default()),
            viewed: RwSignal::new(None),
            refresh: RwSignal::new(0),
            media_rev: RwSignal::new(0),
            status: RwSignal::new("就绪".into()),
            confirm_delete: RwSignal::new(None),
            rename_target: RwSignal::new(None),
            rename_draft: RwSignal::new(String::new()),
            mkdir_parent: RwSignal::new(None),
            mkdir_draft: RwSignal::new(String::new()),
            mkfile_parent: RwSignal::new(None),
            mkfile_draft: RwSignal::new(String::new()),
            busy: RwSignal::new(false),
            show_thumbnails: RwSignal::new(true),
            show_adjust: RwSignal::new(true),
            show_file_manager: RwSignal::new(true),
            show_stars: RwSignal::new(false),
            show_filter: RwSignal::new(false),
            show_export: RwSignal::new(false),
            show_editor_settings: RwSignal::new(false),
            editor_font_size: RwSignal::new(15),
            editor_light: RwSignal::new(false),
            editor_line_numbers: RwSignal::new(true),
            filter_paths: RwSignal::new(None),
            failure_report: RwSignal::new(None),
            expanded_dirs: RwSignal::new(HashSet::from([String::new()])),
        }
    }

    pub fn is_text_mode(self) -> bool {
        self.selected
            .with(|s| s.as_ref().is_some_and(|item| item.is_text))
    }

    pub fn is_expanded(self, path: &str) -> bool {
        self.expanded_dirs.with(|set| set.contains(path))
    }

    pub fn toggle_expanded(self, path: &str) {
        let path = path.to_string();
        self.expanded_dirs.update(|set| {
            if !set.remove(&path) {
                set.insert(path);
            }
        });
    }

    pub fn report_failures(self, title: &str, failures: Vec<FailureItem>) {
        if failures.is_empty() {
            return;
        }
        self.failure_report.set(Some(FailureReport {
            title: title.to_string(),
            failures,
        }));
    }
}
