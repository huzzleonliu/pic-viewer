use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

static PIC_ROOT: OnceLock<PathBuf> = OnceLock::new();

pub fn pic_root() -> &'static Path {
    PIC_ROOT
        .get_or_init(|| {
            let raw = std::env::var("PIC_ROOT").unwrap_or_else(|_| "./pics".into());
            let path = PathBuf::from(&raw);
            if !path.exists() {
                fs::create_dir_all(&path)
                    .unwrap_or_else(|e| panic!("无法创建 PIC_ROOT ({raw}): {e}"));
            }
            path.canonicalize().unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(&path)
            })
        })
        .as_path()
}

pub fn to_rel(full: &Path) -> String {
    full.strip_prefix(pic_root())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

pub fn resolve_path(rel: &str) -> Result<PathBuf, String> {
    resolve_under(pic_root(), rel)
}

pub fn resolve_under(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim_start_matches('/');
    if rel.contains('\0') {
        return Err("非法路径".into());
    }
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err("非法路径".into()),
        }
    }

    let joined = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };

    if joined.exists() {
        let canon = joined.canonicalize().map_err(|e| e.to_string())?;
        if !canon.starts_with(root) {
            return Err("路径越界".into());
        }
        return Ok(canon);
    }

    let file_name = joined
        .file_name()
        .ok_or_else(|| "非法路径".to_string())?
        .to_os_string();
    let parent = joined.parent().ok_or_else(|| "非法路径".to_string())?;
    if !parent.exists() {
        return Err("目标目录不存在".into());
    }
    let canon_parent = parent.canonicalize().map_err(|e| e.to_string())?;
    if !canon_parent.starts_with(root) {
        return Err("路径越界".into());
    }
    Ok(canon_parent.join(file_name))
}

pub fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let dest = dir.join(name);
    if !dest.exists() {
        return dest;
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for i in 1..10_000 {
        let candidate = dir.join(format!("{stem} ({i}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}-copy{ext}"))
}

pub fn copy_recursively(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_recursively(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to)?;
    }
    Ok(())
}

pub fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("tif") | Some("tiff") => "image/tiff",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_recursively, resolve_under, unique_dest};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn scratch() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "pic-viewer-sandbox-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn resolve_rejects_parent_and_null() {
        let root = scratch();
        assert!(resolve_under(&root, "..").is_err());
        assert!(resolve_under(&root, "a/../b").is_err());
        assert!(resolve_under(&root, "a\0b").is_err());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn resolve_allows_existing_child_and_new_name() {
        let root = scratch();
        fs::create_dir(root.join("album")).unwrap();
        fs::write(root.join("album/a.txt"), b"ok").unwrap();
        let got = resolve_under(&root, "album/a.txt").unwrap();
        assert_eq!(got, root.join("album/a.txt"));
        let pending = resolve_under(&root, "album/new.txt").unwrap();
        assert_eq!(pending, root.join("album/new.txt"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn unique_dest_adds_numeric_suffix() {
        let root = scratch();
        fs::write(root.join("a.txt"), b"1").unwrap();
        let dest = unique_dest(&root, "a.txt");
        assert_eq!(dest.file_name().unwrap(), "a (1).txt");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn copy_recursively_copies_tree() {
        let root = scratch();
        fs::create_dir(root.join("from")).unwrap();
        fs::write(root.join("from/a.txt"), b"hi").unwrap();
        copy_recursively(&root.join("from"), &root.join("to")).unwrap();
        assert_eq!(fs::read(root.join("to/a.txt")).unwrap(), b"hi");
        fs::remove_dir_all(&root).ok();
    }
}
