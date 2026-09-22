use leptos::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_image: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
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

#[derive(Clone, Copy)]
pub struct ExplorerState {
    pub selected: RwSignal<Option<SelectedItem>>,
    pub clipboard: RwSignal<Option<Clipboard>>,
    pub checked: RwSignal<Vec<SelectedItem>>,
    pub viewed: RwSignal<Option<String>>,
    pub refresh: RwSignal<u64>,
    pub media_rev: RwSignal<u64>,
    pub status: RwSignal<String>,
    pub confirm_delete: RwSignal<Option<Vec<SelectedItem>>>,
    pub rename_target: RwSignal<Option<SelectedItem>>,
    pub rename_draft: RwSignal<String>,
    pub busy: RwSignal<bool>,
    pub show_thumbnails: RwSignal<bool>,
    pub show_adjust: RwSignal<bool>,
    pub show_file_manager: RwSignal<bool>,
    pub show_stars: RwSignal<bool>,
    pub show_filter: RwSignal<bool>,
    pub filter_paths: RwSignal<Option<Vec<String>>>,
}

impl ExplorerState {
    pub fn new() -> Self {
        Self {
            selected: RwSignal::new(None),
            clipboard: RwSignal::new(None),
            checked: RwSignal::new(Vec::new()),
            viewed: RwSignal::new(None),
            refresh: RwSignal::new(0),
            media_rev: RwSignal::new(0),
            status: RwSignal::new("就绪".into()),
            confirm_delete: RwSignal::new(None),
            rename_target: RwSignal::new(None),
            rename_draft: RwSignal::new(String::new()),
            busy: RwSignal::new(false),
            show_thumbnails: RwSignal::new(true),
            show_adjust: RwSignal::new(true),
            show_file_manager: RwSignal::new(true),
            show_stars: RwSignal::new(false),
            show_filter: RwSignal::new(false),
            filter_paths: RwSignal::new(None),
        }
    }
}
