use crate::function::path::parent_path;
use crate::structure::SelectedItem;

pub(crate) fn next_viewed_for_dir(shown: &[String], current: Option<&str>) -> Option<String> {
    if shown.is_empty() {
        None
    } else if current.is_some_and(|p| shown.iter().any(|x| x == p)) {
        current.map(str::to_string)
    } else {
        shown.first().cloned()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavSnapshot {
    pub selected: Option<SelectedItem>,
    pub viewed: Option<String>,
    pub browse_dir: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Nav {
    TreePlain(SelectedItem),
    Focus(SelectedItem),
    OpenImage(String),
    ApplyGallery { dir: String, shown: Vec<String> },
}

pub fn apply_nav(prev: NavSnapshot, nav: Nav) -> NavSnapshot {
    match nav {
        Nav::TreePlain(item) => {
            let mut next = prev;
            if item.is_dir {
                let path = item.path.clone();
                let keep = next.viewed.as_ref().is_some_and(|p| parent_path(p) == path);
                if next.browse_dir.as_deref() != Some(path.as_str()) && !keep {
                    next.viewed = None;
                }
                next.browse_dir = Some(path);
            } else {
                next.browse_dir = None;
                if item.is_image {
                    next.viewed = Some(item.path.clone());
                }
            }
            next.selected = Some(item);
            next
        }
        Nav::Focus(item) => {
            let mut next = prev;
            next.selected = Some(item);
            next
        }
        Nav::OpenImage(path) => NavSnapshot {
            selected: Some(SelectedItem::from_image_path(path.clone())),
            viewed: Some(path),
            browse_dir: prev.browse_dir,
        },
        Nav::ApplyGallery { dir, shown } => {
            if prev.browse_dir.as_deref() != Some(dir.as_str()) {
                return prev;
            }
            let mut next = prev;
            next.viewed = next_viewed_for_dir(&shown, next.viewed.as_deref());
            next
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_nav, Nav, NavSnapshot};
    use crate::structure::SelectedItem;

    fn dir(path: &str) -> SelectedItem {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        SelectedItem {
            path: path.to_string(),
            name,
            is_dir: true,
            is_image: false,
            is_text: false,
        }
    }

    fn image(path: &str) -> SelectedItem {
        SelectedItem::from_image_path(path.to_string())
    }

    fn text(path: &str) -> SelectedItem {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        SelectedItem {
            path: path.to_string(),
            name,
            is_dir: false,
            is_image: false,
            is_text: true,
        }
    }

    fn snap() -> NavSnapshot {
        NavSnapshot {
            selected: Some(SelectedItem::root()),
            viewed: Some("photos/a.jpg".into()),
            browse_dir: Some("photos".into()),
        }
    }

    #[test]
    fn dir_click_sets_browse_and_clears_foreign_viewed() {
        let next = apply_nav(snap(), Nav::TreePlain(dir("animals")));
        assert_eq!(next.browse_dir.as_deref(), Some("animals"));
        assert_eq!(next.viewed, None);
        assert_eq!(
            next.selected.as_ref().map(|s| s.path.as_str()),
            Some("animals")
        );
    }

    #[test]
    fn dir_click_keeps_viewed_in_that_dir() {
        let next = apply_nav(snap(), Nav::TreePlain(dir("photos")));
        assert_eq!(next.browse_dir.as_deref(), Some("photos"));
        assert_eq!(next.viewed.as_deref(), Some("photos/a.jpg"));
    }

    #[test]
    fn image_click_opens_file_and_clears_browse() {
        let next = apply_nav(snap(), Nav::TreePlain(image("other/b.jpg")));
        assert_eq!(next.browse_dir, None);
        assert_eq!(next.viewed.as_deref(), Some("other/b.jpg"));
    }

    #[test]
    fn text_click_keeps_viewed() {
        let next = apply_nav(snap(), Nav::TreePlain(text("notes.txt")));
        assert_eq!(next.browse_dir, None);
        assert_eq!(next.viewed.as_deref(), Some("photos/a.jpg"));
        assert!(next.selected.is_some_and(|s| s.is_text));
    }

    #[test]
    fn expand_only_changes_focus() {
        let next = apply_nav(snap(), Nav::Focus(dir("animals")));
        assert_eq!(next.browse_dir.as_deref(), Some("photos"));
        assert_eq!(next.viewed.as_deref(), Some("photos/a.jpg"));
        assert_eq!(
            next.selected.as_ref().map(|s| s.path.as_str()),
            Some("animals")
        );
    }

    #[test]
    fn open_image_keeps_browse_dir() {
        let next = apply_nav(snap(), Nav::OpenImage("photos/b.jpg".into()));
        assert_eq!(next.browse_dir.as_deref(), Some("photos"));
        assert_eq!(next.viewed.as_deref(), Some("photos/b.jpg"));
    }

    #[test]
    fn apply_gallery_is_idempotent_when_viewed_already_in_list() {
        let shown = vec!["photos/a.jpg".into(), "photos/b.jpg".into()];
        let first = apply_nav(
            snap(),
            Nav::ApplyGallery {
                dir: "photos".into(),
                shown: shown.clone(),
            },
        );
        assert_eq!(first, snap());
        let second = apply_nav(
            first.clone(),
            Nav::ApplyGallery {
                dir: "photos".into(),
                shown,
            },
        );
        assert_eq!(second, first);
    }

    #[test]
    fn apply_gallery_opens_first_when_viewed_cleared() {
        let mut empty = snap();
        empty.viewed = None;
        let next = apply_nav(
            empty,
            Nav::ApplyGallery {
                dir: "photos".into(),
                shown: vec!["photos/a.jpg".into(), "photos/b.jpg".into()],
            },
        );
        assert_eq!(next.viewed.as_deref(), Some("photos/a.jpg"));
        assert_eq!(next.browse_dir.as_deref(), Some("photos"));
    }

    #[test]
    fn apply_gallery_only_when_browse_matches() {
        let shown = vec!["animals/x.jpg".into()];
        let skipped = apply_nav(
            snap(),
            Nav::ApplyGallery {
                dir: "animals".into(),
                shown: shown.clone(),
            },
        );
        assert_eq!(skipped.viewed.as_deref(), Some("photos/a.jpg"));
        let applied = apply_nav(
            snap(),
            Nav::ApplyGallery {
                dir: "photos".into(),
                shown,
            },
        );
        assert_eq!(applied.viewed.as_deref(), Some("animals/x.jpg"));
    }
}
