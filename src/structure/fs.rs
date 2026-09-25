use serde::{Deserialize, Serialize};

pub const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "avif", "ico", "tif", "tiff",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_image: bool,
    pub is_text: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageList {
    pub truncated: bool,
    pub paths: Vec<String>,
}

impl FsEntry {
    pub fn image_from_path(path: String) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
        Self {
            name,
            path,
            is_dir: false,
            is_image: true,
            is_text: false,
        }
    }
}
