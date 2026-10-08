use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::sandbox::{copy_recursively, pic_root, resolve_path, to_rel, unique_dest};

#[server]
pub async fn delete_entry(path: String) -> Result<(), ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = path;
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        block(move || io::delete_entry(path)).await
    }
}

#[server]
pub async fn rename_entry(path: String, new_name: String) -> Result<String, ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (path, new_name);
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        block(move || io::rename_entry(path, new_name)).await
    }
}

#[server]
pub async fn rename_entries_numbered(
    paths: Vec<String>,
    prefix: String,
    start: i32,
    end: i32,
) -> Result<crate::structure::BatchRenameReport, ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (paths, prefix, start, end);
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        block(move || io::rename_entries_numbered(paths, prefix, start, end)).await
    }
}

#[server]
pub async fn create_dir(parent: String, name: String) -> Result<String, ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (parent, name);
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        block(move || io::create_dir(parent, name)).await
    }
}

#[server]
pub async fn create_file(parent: String, name: String) -> Result<String, ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (parent, name);
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        block(move || io::create_file(parent, name)).await
    }
}

#[server]
pub async fn paste_entry(
    source: String,
    dest_dir: String,
    cut: bool,
) -> Result<String, ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (source, dest_dir, cut);
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        block(move || io::paste_entry(source, dest_dir, cut)).await
    }
}

#[cfg(feature = "ssr")]
async fn block<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, ServerFnError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
mod io {
    use super::{copy_recursively, pic_root, resolve_path, to_rel, unique_dest};

    pub fn delete_entry(path: String) -> Result<(), String> {
        if path.is_empty() {
            return Err("不能删除根目录".into());
        }
        let target = resolve_path(&path)?;
        if target == pic_root() {
            return Err("不能删除根目录".into());
        }
        if !target.exists() {
            return Err("文件不存在".into());
        }
        if target.is_dir() {
            std::fs::remove_dir_all(&target)
        } else {
            std::fs::remove_file(&target)
        }
        .map_err(|e| e.to_string())
    }

    pub fn rename_entry(path: String, new_name: String) -> Result<String, String> {
        if path.is_empty() {
            return Err("不能重命名根目录".into());
        }
        let new_name = new_name.trim().to_string();
        crate::function::path::validate_file_name(&new_name)?;

        let from = resolve_path(&path)?;
        if from == pic_root() {
            return Err("不能重命名根目录".into());
        }
        if !from.exists() {
            return Err("文件不存在".into());
        }
        let parent = from.parent().ok_or_else(|| "非法路径".to_string())?;
        let to = parent.join(&new_name);
        if !to.starts_with(pic_root()) {
            return Err("路径越界".into());
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
            return Err("目标名称已存在".into());
        }
        std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
        Ok(to_rel(&to))
    }

    pub fn rename_entries_numbered(
        paths: Vec<String>,
        prefix: String,
        start: i32,
        end: i32,
    ) -> Result<crate::structure::BatchRenameReport, String> {
        use crate::function::path::{
            number_run, numbered_file_name, rel_name, validate_file_name, validate_rename_prefix,
        };
        use crate::structure::{BatchRenameReport, FailureItem};

        let prefix = validate_rename_prefix(&prefix)?;
        let nums = number_run(start, end);
        if nums.len() != paths.len() {
            return Err(format!(
                "序号个数（{}）与文件数（{}）不一致",
                nums.len(),
                paths.len()
            ));
        }
        if paths.is_empty() {
            return Ok(BatchRenameReport {
                ok: 0,
                renamed: Vec::new(),
                failures: Vec::new(),
            });
        }

        let mut desired = Vec::with_capacity(paths.len());
        for (path, n) in paths.iter().zip(nums.iter().copied()) {
            if path.is_empty() {
                return Err("不能重命名根目录".into());
            }
            let from = resolve_path(path)?;
            if from == pic_root() {
                return Err("不能重命名根目录".into());
            }
            if !from.is_file() {
                return Err(format!("{} 不是文件", rel_name(path)));
            }
            let orig = from
                .file_name()
                .ok_or_else(|| "非法路径".to_string())?
                .to_string_lossy()
                .into_owned();
            let new_name = numbered_file_name(&prefix, n, &orig);
            validate_file_name(&new_name)?;
            let parent = from.parent().ok_or_else(|| "非法路径".to_string())?;
            let dest = parent.join(&new_name);
            if !dest.starts_with(pic_root()) {
                return Err("路径越界".into());
            }
            desired.push((from, new_name));
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut parked: Vec<(std::path::PathBuf, std::path::PathBuf, String, String)> = Vec::new();
        for (i, (from, new_name)) in desired.into_iter().enumerate() {
            let parent = from.parent().ok_or_else(|| "非法路径".to_string())?;
            let old_rel = to_rel(&from);
            let tmp = unique_dest(parent, &format!(".__pv_ren_{stamp}_{i}"));
            if let Err(e) = std::fs::rename(&from, &tmp) {
                for (orig, temp, _, _) in parked.iter().rev() {
                    let _ = std::fs::rename(temp, orig);
                }
                return Err(e.to_string());
            }
            parked.push((from, tmp, new_name, old_rel));
        }

        let mut renamed = Vec::new();
        let mut failures = Vec::new();
        for (orig, tmp, new_name, old_rel) in parked {
            let parent = match tmp.parent() {
                Some(p) => p.to_path_buf(),
                None => {
                    failures.push(FailureItem {
                        file: old_rel,
                        error: "非法路径".into(),
                    });
                    continue;
                }
            };
            let dest = unique_dest(&parent, &new_name);
            match std::fs::rename(&tmp, &dest) {
                Ok(()) => renamed.push((old_rel, to_rel(&dest))),
                Err(e) => {
                    let _ = std::fs::rename(&tmp, &orig);
                    failures.push(FailureItem {
                        file: old_rel,
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(BatchRenameReport {
            ok: renamed.len() as u32,
            renamed,
            failures,
        })
    }

    pub fn create_dir(parent: String, name: String) -> Result<String, String> {
        let name = name.trim().to_string();
        crate::function::path::validate_file_name(&name)?;
        let parent_full = resolve_path(&parent)?;
        if !parent_full.is_dir() {
            return Err("父路径不是目录".into());
        }
        let dest = parent_full.join(&name);
        if !dest.starts_with(pic_root()) {
            return Err("路径越界".into());
        }
        if dest.exists() {
            return Err("目标名称已存在".into());
        }
        std::fs::create_dir(&dest).map_err(|e| e.to_string())?;
        Ok(to_rel(&dest))
    }

    pub fn create_file(parent: String, name: String) -> Result<String, String> {
        let name = name.trim().to_string();
        crate::function::path::validate_file_name(&name)?;
        let parent_full = resolve_path(&parent)?;
        if !parent_full.is_dir() {
            return Err("父路径不是目录".into());
        }
        let dest = parent_full.join(&name);
        if !dest.starts_with(pic_root()) {
            return Err("路径越界".into());
        }
        if dest.exists() {
            return Err("目标名称已存在".into());
        }
        std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        Ok(to_rel(&dest))
    }

    pub fn paste_entry(source: String, dest_dir: String, cut: bool) -> Result<String, String> {
        if source.is_empty() {
            return Err("不能复制根目录".into());
        }
        let from = resolve_path(&source)?;
        if !from.exists() {
            return Err("源文件不存在".into());
        }
        let dest_parent = resolve_path(&dest_dir)?;
        if !dest_parent.is_dir() {
            return Err("粘贴目标不是目录".into());
        }

        if from == dest_parent {
            return Err("不能粘贴到自身".into());
        }
        if from.is_dir() {
            let dest_str = dest_parent.to_string_lossy();
            let from_str = from.to_string_lossy();
            if dest_str.starts_with(&format!("{}{}", from_str, std::path::MAIN_SEPARATOR)) {
                return Err("不能粘贴到自身内部".into());
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
            .ok_or_else(|| "非法源路径".to_string())?
            .to_string_lossy()
            .into_owned();
        let to = unique_dest(&dest_parent, &name);
        if to.exists() && to.canonicalize().ok().as_deref() == Some(from.as_path()) {
            return Err("不能覆盖源文件".into());
        }

        if cut {
            match std::fs::rename(&from, &to) {
                Ok(()) => return Ok(to_rel(&to)),
                Err(_) => {
                    copy_recursively(&from, &to).map_err(|e| e.to_string())?;
                    let remove = if from.is_dir() {
                        std::fs::remove_dir_all(&from)
                    } else {
                        std::fs::remove_file(&from)
                    };
                    remove.map_err(|e| format!("已复制但删除源失败：{e}"))?;
                    return Ok(to_rel(&to));
                }
            }
        }

        copy_recursively(&from, &to).map_err(|e| e.to_string())?;
        Ok(to_rel(&to))
    }
}
