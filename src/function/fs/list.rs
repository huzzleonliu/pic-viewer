use crate::structure::{FsEntry, ImageList};
#[cfg(feature = "ssr")]
use crate::structure::ImageRef;
use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::sandbox::{pic_root, resolve_path, to_rel};
#[cfg(feature = "ssr")]
use crate::function::sniff::{classify_file, FileKind};

#[cfg(feature = "ssr")]
const MAX_GALLERY_IMAGES: usize = 10_000;

#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct ImageRecord {
    pub path: String,
    pub mtime: u64,
    pub size: u64,
}

#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct ImageScan {
    pub truncated: bool,
    pub records: Vec<ImageRecord>,
}

#[cfg(feature = "ssr")]
fn file_mtime_size(path: &std::path::Path) -> (u64, u64) {
    use std::time::UNIX_EPOCH;
    let Ok(meta) = std::fs::metadata(path) else {
        return (0, 0);
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (mtime, meta.len())
}

#[cfg(feature = "ssr")]
fn list_dir_sync(path: &str) -> Result<Vec<FsEntry>, String> {
    let dir = resolve_path(path)?;
    if !dir.is_dir() {
        return Err("不是目录".into());
    }
    let mut entries = Vec::new();
    let read = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for item in read {
        let item = match item {
            Ok(v) => v,
            Err(_) => continue,
        };
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let rel = to_rel(&item.path());
        let (is_image, is_text) = if is_dir {
            (false, false)
        } else {
            match classify_file(&item.path()) {
                FileKind::Image => (true, false),
                FileKind::Text => (false, true),
                _ => (false, false),
            }
        };
        entries.push(FsEntry {
            is_image,
            is_text,
            name,
            path: rel,
            is_dir,
        });
    }
    sort_fs_entries(&mut entries);
    Ok(entries)
}

#[cfg(any(test, feature = "ssr"))]
pub(crate) fn sort_fs_entries(entries: &mut [FsEntry]) {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

#[cfg(feature = "ssr")]
pub fn collect_image_scan(rel_dir: &str, recursive: bool) -> Result<ImageScan, String> {
    use std::path::PathBuf;

    let root = resolve_path(rel_dir)?;
    if !root.is_dir() {
        return Err("不是目录".into());
    }

    let mut out = Vec::new();
    let mut truncated = false;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        if out.len() >= MAX_GALLERY_IMAGES {
            truncated = true;
            break;
        }
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut subdirs: Vec<PathBuf> = Vec::new();
        let mut images: Vec<(String, PathBuf)> = Vec::new();
        for item in read {
            let Ok(item) = item else {
                continue;
            };
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = item.path();
            let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if recursive {
                    subdirs.push(path);
                }
                continue;
            }
            if classify_file(&path) == FileKind::Image {
                images.push((name, path));
            }
        }
        images.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        for (_, path) in images {
            if out.len() >= MAX_GALLERY_IMAGES {
                truncated = true;
                break;
            }
            let (mtime, size) = file_mtime_size(&path);
            out.push(ImageRecord {
                path: to_rel(&path),
                mtime,
                size,
            });
        }
        if recursive {
            subdirs.sort_by(|a, b| {
                let an = a
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let bn = b
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                an.cmp(&bn)
            });
            stack.extend(subdirs.into_iter().rev());
        }
    }
    if !stack.is_empty() {
        truncated = true;
    }
    Ok(ImageScan {
        truncated,
        records: out,
    })
}

#[cfg(feature = "ssr")]
mod listing_cache {
    use super::ImageScan;
    use crate::function::lru::LruCache;
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone)]
    struct ScanVal {
        epoch: u64,
        scan: ImageScan,
    }

    fn cache() -> &'static Mutex<LruCache<(String, bool), ScanVal>> {
        static CACHE: OnceLock<Mutex<LruCache<(String, bool), ScanVal>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(LruCache::new(16)))
    }

    pub fn get_or_collect(dir: &str, recursive: bool, epoch: u64) -> Result<ImageScan, String> {
        let key = (dir.to_string(), recursive);
        let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(cached) = guard.get(&key) {
            if cached.epoch == epoch {
                return Ok(cached.scan.clone());
            }
        }
        drop(guard);
        let scan = super::collect_image_scan(dir, recursive)?;
        let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
        guard.insert(
            key,
            ScanVal {
                epoch,
                scan: scan.clone(),
            },
        );
        Ok(scan)
    }
}

#[cfg(feature = "ssr")]
mod dir_list_cache {
    use super::FsEntry;
    use crate::function::lru::LruCache;
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone)]
    struct DirVal {
        epoch: u64,
        entries: Vec<FsEntry>,
    }

    fn cache() -> &'static Mutex<LruCache<String, DirVal>> {
        static CACHE: OnceLock<Mutex<LruCache<String, DirVal>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(LruCache::new(32)))
    }

    pub fn get_or_list(path: &str, epoch: u64) -> Result<Vec<FsEntry>, String> {
        let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(cached) = guard.get(&path.to_string()) {
            if cached.epoch == epoch {
                return Ok(cached.entries.clone());
            }
        }
        drop(guard);
        let entries = super::list_dir_sync(path)?;
        let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
        guard.insert(
            path.to_string(),
            DirVal {
                epoch,
                entries: entries.clone(),
            },
        );
        Ok(entries)
    }
}

#[cfg(feature = "ssr")]
pub fn cached_image_scan(dir: &str, recursive: bool, epoch: u64) -> Result<ImageScan, String> {
    listing_cache::get_or_collect(dir, recursive, epoch)
}

#[server]
pub async fn get_root_info() -> Result<String, ServerFnError> {
    Ok(pic_root().display().to_string())
}

#[server]
pub async fn list_dir(path: String, epoch: u64) -> Result<Vec<FsEntry>, ServerFnError> {
    tokio::task::spawn_blocking(move || dir_list_cache::get_or_list(&path, epoch))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .map_err(ServerFnError::new)
}

#[server]
pub async fn list_images(
    path: String,
    recursive: bool,
    epoch: u64,
) -> Result<ImageList, ServerFnError> {
    tokio::task::spawn_blocking(move || {
        let scan = cached_image_scan(&path, recursive, epoch)?;
        Ok::<ImageList, String>(ImageList {
            truncated: scan.truncated,
            items: scan
                .records
                .into_iter()
                .map(|r| ImageRef {
                    path: r.path,
                    mtime: r.mtime,
                })
                .collect(),
        })
    })
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .map_err(ServerFnError::new)
}

#[cfg(test)]
mod tests {
    use super::sort_fs_entries;
    use crate::structure::FsEntry;

    fn item(name: &str, is_dir: bool) -> FsEntry {
        FsEntry {
            name: name.into(),
            path: name.into(),
            is_dir,
            is_image: !is_dir && name.ends_with(".jpg"),
            is_text: false,
        }
    }

    #[test]
    fn dirs_first_then_case_insensitive_name() {
        let mut entries = vec![item("b.jpg", false), item("A", true), item("a.jpg", false)];
        sort_fs_entries(&mut entries);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["A", "a.jpg", "b.jpg"]);
    }
}
