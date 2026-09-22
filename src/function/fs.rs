use crate::structure::FsEntry;
use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::function::path::is_image_name;

#[cfg(feature = "ssr")]
mod server_fs {
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
        let root = pic_root();
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
}

#[cfg(feature = "ssr")]
pub use server_fs::{mime_for, pic_root, resolve_path};

#[cfg(feature = "ssr")]
use server_fs::{copy_recursively, to_rel, unique_dest};

#[server]
pub async fn get_root_info() -> Result<String, ServerFnError> {
    Ok(pic_root().display().to_string())
}

#[server]
pub async fn list_dir(path: String) -> Result<Vec<FsEntry>, ServerFnError> {
    let dir = resolve_path(&path).map_err(ServerFnError::new)?;
    if !dir.is_dir() {
        return Err(ServerFnError::new("不是目录"));
    }
    let mut entries = Vec::new();
    let read = std::fs::read_dir(&dir).map_err(|e| ServerFnError::new(e.to_string()))?;
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
        entries.push(FsEntry {
            is_image: !is_dir && is_image_name(&name),
            name,
            path: rel,
            is_dir,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[server]
pub async fn delete_entry(path: String) -> Result<(), ServerFnError> {
    if path.is_empty() {
        return Err(ServerFnError::new("不能删除根目录"));
    }
    let target = resolve_path(&path).map_err(ServerFnError::new)?;
    if target == pic_root() {
        return Err(ServerFnError::new("不能删除根目录"));
    }
    if !target.exists() {
        return Err(ServerFnError::new("文件不存在"));
    }
    let result = if target.is_dir() {
        std::fs::remove_dir_all(&target)
    } else {
        std::fs::remove_file(&target)
    };
    result.map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn rename_entry(path: String, new_name: String) -> Result<String, ServerFnError> {
    if path.is_empty() {
        return Err(ServerFnError::new("不能重命名根目录"));
    }
    let new_name = new_name.trim().to_string();
    crate::function::path::validate_file_name(&new_name).map_err(ServerFnError::new)?;

    let from = resolve_path(&path).map_err(ServerFnError::new)?;
    if from == pic_root() {
        return Err(ServerFnError::new("不能重命名根目录"));
    }
    if !from.exists() {
        return Err(ServerFnError::new("文件不存在"));
    }
    let parent = from
        .parent()
        .ok_or_else(|| ServerFnError::new("非法路径"))?;
    let to = parent.join(&new_name);
    if !to.starts_with(pic_root()) {
        return Err(ServerFnError::new("路径越界"));
    }
    if to.exists() {
        let same = from
            .canonicalize()
            .ok()
            .zip(to.canonicalize().ok())
            .is_some_and(|(a, b)| a == b);
        if same {
            return Ok(to_rel(&from));
        }
        return Err(ServerFnError::new("目标名称已存在"));
    }
    std::fs::rename(&from, &to).map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(to_rel(&to))
}

#[server]
pub async fn paste_entry(
    source: String,
    dest_dir: String,
    cut: bool,
) -> Result<String, ServerFnError> {
    if source.is_empty() {
        return Err(ServerFnError::new("不能复制根目录"));
    }
    let from = resolve_path(&source).map_err(ServerFnError::new)?;
    if !from.exists() {
        return Err(ServerFnError::new("源文件不存在"));
    }
    let dest_parent = resolve_path(&dest_dir).map_err(ServerFnError::new)?;
    if !dest_parent.is_dir() {
        return Err(ServerFnError::new("粘贴目标不是目录"));
    }

    if from == dest_parent {
        return Err(ServerFnError::new("不能粘贴到自身"));
    }
    if from.is_dir() {
        let dest_str = dest_parent.to_string_lossy();
        let from_str = from.to_string_lossy();
        if dest_str.starts_with(&format!("{}{}", from_str, std::path::MAIN_SEPARATOR)) {
            return Err(ServerFnError::new("不能粘贴到自身内部"));
        }
    }

    if cut {
        if let Some(parent) = from.parent() {
            if parent == dest_parent.as_path() {
                return Ok(to_rel(&from));
            }
        }
    }

    let name = from
        .file_name()
        .ok_or_else(|| ServerFnError::new("非法源路径"))?
        .to_string_lossy()
        .into_owned();
    let to = unique_dest(&dest_parent, &name);
    if to.exists() && to.canonicalize().ok().as_deref() == Some(from.as_path()) {
        return Err(ServerFnError::new("不能覆盖源文件"));
    }

    if cut {
        match std::fs::rename(&from, &to) {
            Ok(()) => return Ok(to_rel(&to)),
            Err(_) => {
                copy_recursively(&from, &to).map_err(|e| ServerFnError::new(e.to_string()))?;
                let remove = if from.is_dir() {
                    std::fs::remove_dir_all(&from)
                } else {
                    std::fs::remove_file(&from)
                };
                remove.map_err(|e| ServerFnError::new(format!("已复制但删除源失败：{e}")))?;
                return Ok(to_rel(&to));
            }
        }
    }

    copy_recursively(&from, &to).map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(to_rel(&to))
}

#[cfg(feature = "ssr")]
pub async fn serve_media(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::IntoResponse;

    let Ok(safe) = resolve_path(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !safe.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mime = mime_for(&safe);
    match tokio::fs::read(&safe).await {
        Ok(bytes) => {
            let mut res = Body::from(bytes).into_response();
            res.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
            res.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=120"),
            );
            res
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
