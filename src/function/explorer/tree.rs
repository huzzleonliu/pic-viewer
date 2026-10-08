use crate::function::path::parent_path;
use crate::structure::{FsEntry, SelectedItem};
use std::collections::{HashMap, HashSet};

pub(crate) fn visible_tree_items(
    expanded: &HashSet<String>,
    listings: &HashMap<String, Vec<FsEntry>>,
) -> Vec<SelectedItem> {
    let mut out = vec![SelectedItem::root()];
    walk_visible("", expanded, listings, &mut out);
    out
}

fn walk_visible(
    path: &str,
    expanded: &HashSet<String>,
    listings: &HashMap<String, Vec<FsEntry>>,
    out: &mut Vec<SelectedItem>,
) {
    if !expanded.contains(path) {
        return;
    }
    let Some(children) = listings.get(path) else {
        return;
    };
    for child in children {
        out.push(child.clone());
        if child.is_dir {
            walk_visible(&child.path, expanded, listings, out);
        }
    }
}

pub(crate) fn checkable_range(visible: &[SelectedItem], from: &str, to: &str) -> Vec<SelectedItem> {
    let find = |p: &str| visible.iter().position(|item| item.path == p);
    match (find(from), find(to)) {
        (Some(i), Some(j)) => {
            let (a, b) = if i <= j { (i, j) } else { (j, i) };
            visible[a..=b]
                .iter()
                .filter(|item| !item.path.is_empty())
                .cloned()
                .collect()
        }
        (_, Some(_)) => visible
            .iter()
            .filter(|item| item.path == to && !item.path.is_empty())
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn gallery_dir_from(
    browse_dir: Option<&str>,
    selected: Option<&SelectedItem>,
    viewed: Option<&str>,
) -> String {
    if let Some(dir) = browse_dir {
        return dir.to_string();
    }
    if let Some(path) = viewed {
        return parent_path(path);
    }
    match selected {
        Some(s) if s.is_dir => s.path.clone(),
        Some(s) => parent_path(&s.path),
        None => String::new(),
    }
}

#[cfg(test)]
mod tree_select_tests {
    use super::{checkable_range, gallery_dir_from, visible_tree_items};
    use crate::function::nav::next_viewed_for_dir;
    use crate::structure::{FsEntry, SelectedItem};
    use std::collections::{HashMap, HashSet};

    fn entry(path: &str, is_dir: bool) -> FsEntry {
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        FsEntry {
            name,
            path: path.to_string(),
            is_dir,
            is_image: !is_dir && path.ends_with(".jpg"),
            is_text: !is_dir && path.ends_with(".txt"),
        }
    }

    fn sample() -> (HashSet<String>, HashMap<String, Vec<FsEntry>>) {
        let mut listings = HashMap::new();
        listings.insert(
            String::new(),
            vec![entry("photos", true), entry("readme.txt", false)],
        );
        listings.insert(
            "photos".into(),
            vec![entry("photos/a.jpg", false), entry("photos/b.jpg", false)],
        );
        (HashSet::from([String::new()]), listings)
    }

    #[test]
    fn flatten_skips_collapsed_children() {
        let (expanded, listings) = sample();
        let paths: Vec<_> = visible_tree_items(&expanded, &listings)
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(paths, ["", "photos", "readme.txt"]);
    }

    #[test]
    fn flatten_includes_expanded_children() {
        let (mut expanded, listings) = sample();
        expanded.insert("photos".into());
        let paths: Vec<_> = visible_tree_items(&expanded, &listings)
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(
            paths,
            ["", "photos", "photos/a.jpg", "photos/b.jpg", "readme.txt"]
        );
    }

    #[test]
    fn range_is_inclusive_and_skips_root() {
        let (mut expanded, listings) = sample();
        expanded.insert("photos".into());
        let visible = visible_tree_items(&expanded, &listings);
        let paths: Vec<_> = checkable_range(&visible, "", "photos/b.jpg")
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(paths, ["photos", "photos/a.jpg", "photos/b.jpg"]);
    }

    #[test]
    fn range_works_backwards() {
        let (expanded, listings) = sample();
        let visible = visible_tree_items(&expanded, &listings);
        let paths: Vec<_> = checkable_range(&visible, "readme.txt", "photos")
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert_eq!(paths, ["photos", "readme.txt"]);
    }

    #[test]
    fn gallery_follows_browse_dir_over_viewed() {
        let image = SelectedItem::from_image_path("photos/a.jpg".into());
        assert_eq!(
            gallery_dir_from(Some("animals"), Some(&image), Some("photos/a.jpg")),
            "animals"
        );
        assert_eq!(
            gallery_dir_from(None, Some(&image), Some("photos/a.jpg")),
            "photos"
        );
        assert_eq!(gallery_dir_from(Some(""), None, None), "");
    }

    #[test]
    fn dir_without_images_clears_viewed() {
        assert_eq!(next_viewed_for_dir(&[], Some("a.jpg")), None);
        assert_eq!(next_viewed_for_dir(&[], None), None);
    }

    #[test]
    fn dir_with_images_keeps_or_opens_first() {
        let shown = vec!["photos/a.jpg".into(), "photos/b.jpg".into()];
        assert_eq!(
            next_viewed_for_dir(&shown, Some("photos/b.jpg")).as_deref(),
            Some("photos/b.jpg")
        );
        assert_eq!(
            next_viewed_for_dir(&shown, Some("other.jpg")).as_deref(),
            Some("photos/a.jpg")
        );
        assert_eq!(
            next_viewed_for_dir(&shown, None).as_deref(),
            Some("photos/a.jpg")
        );
    }
}
