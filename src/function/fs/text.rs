use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::sandbox::resolve_path;
#[cfg(feature = "ssr")]
use crate::function::sniff::{decode_text, kind_from_header, sniff_file, FileKind};

#[server]
pub async fn read_text_file(path: String) -> Result<String, ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = path;
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        tokio::task::spawn_blocking(move || io::read_text_file(path))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .map_err(ServerFnError::new)
    }
}

#[server]
pub async fn write_text_file(path: String, content: String) -> Result<(), ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (path, content);
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        tokio::task::spawn_blocking(move || io::write_text_file(path, content))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .map_err(ServerFnError::new)
    }
}

#[cfg(feature = "ssr")]
mod io {
    use super::{decode_text, kind_from_header, resolve_path, sniff_file, FileKind};

    pub fn read_text_file(path: String) -> Result<String, String> {
        if path.is_empty() {
            return Err("不能打开根目录".into());
        }
        let target = resolve_path(&path)?;
        if !target.is_file() {
            return Err("不是文件".into());
        }
        let meta = std::fs::metadata(&target).map_err(|e| e.to_string())?;
        if meta.len() > 2 * 1024 * 1024 {
            return Err("文件过大，无法在编辑器中打开".into());
        }
        let bytes = std::fs::read(&target).map_err(|e| e.to_string())?;
        let header = &bytes[..bytes.len().min(64)];
        if kind_from_header(header) != FileKind::Text {
            return Err("不是文本文件".into());
        }
        Ok(decode_text(&bytes))
    }

    pub fn write_text_file(path: String, content: String) -> Result<(), String> {
        if path.is_empty() {
            return Err("不能写入根目录".into());
        }
        if content.len() > 8 * 1024 * 1024 {
            return Err("内容过大，无法保存".into());
        }
        let target = resolve_path(&path)?;
        if target.is_dir() {
            return Err("不能写入目录".into());
        }
        if target.exists() && sniff_file(&target) != FileKind::Text {
            return Err("不是文本文件".into());
        }
        std::fs::write(&target, content.as_bytes()).map_err(|e| e.to_string())
    }
}
