use serde::{Deserialize, Serialize};

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "avif", "ico", "tif", "tiff",
];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FsKind {
    Dir,
    Image,
    Text,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsItem {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_image: bool,
    pub is_text: bool,
}

pub type FsEntry = FsItem;
pub type SelectedItem = FsItem;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageRef {
    pub path: String,
    pub mtime: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageList {
    pub truncated: bool,
    pub items: Vec<ImageRef>,
}

impl ImageList {
    pub fn paths(&self) -> Vec<String> {
        self.items.iter().map(|item| item.path.clone()).collect()
    }
}

impl FsItem {
    pub fn kind(&self) -> FsKind {
        if self.is_dir {
            FsKind::Dir
        } else if self.is_image {
            FsKind::Image
        } else if self.is_text {
            FsKind::Text
        } else {
            FsKind::Other
        }
    }

    pub fn from_kind(name: String, path: String, kind: FsKind) -> Self {
        Self {
            name,
            path,
            is_dir: kind == FsKind::Dir,
            is_image: kind == FsKind::Image,
            is_text: kind == FsKind::Text,
        }
    }

    pub fn image_from_path(path: String) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
        Self::from_kind(name, path, FsKind::Image)
    }

    pub fn from_image_path(path: String) -> Self {
        Self::image_from_path(path)
    }

    pub fn root() -> Self {
        Self::from_kind("/".into(), String::new(), FsKind::Dir)
    }
}

impl From<&FsItem> for FsItem {
    fn from(item: &FsItem) -> Self {
        item.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{FsItem, FsKind, ImageList, ImageRef};

    #[test]
    fn kind_matches_flags() {
        assert_eq!(FsItem::root().kind(), FsKind::Dir);
        assert_eq!(
            FsItem::image_from_path("a.jpg".into()).kind(),
            FsKind::Image
        );
        assert_eq!(
            FsItem::from_kind("n.txt".into(), "n.txt".into(), FsKind::Text).kind(),
            FsKind::Text
        );
    }

    #[test]
    fn image_list_paths_follow_items() {
        let list = ImageList {
            truncated: false,
            items: vec![ImageRef {
                path: "a.jpg".into(),
                mtime: 1_710_000_000,
            }],
        };
        assert_eq!(list.paths(), ["a.jpg"]);
    }
}
