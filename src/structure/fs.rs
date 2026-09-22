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
}
