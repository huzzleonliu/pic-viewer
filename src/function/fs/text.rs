use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::sandbox::resolve_path;
#[cfg(feature = "ssr")]
use crate::function::sniff::{decode_text, kind_from_header, sniff_file, FileKind};

#[server]
pub async fn read_text_file(path: String) -> Result<String, ServerFnError> {
    if path.is_empty() {
        return Err(ServerFnError::new("不能打开根目录"));
    }
    let target = resolve_path(&path).map_err(ServerFnError::new)?;
    if !target.is_file() {
        return Err(ServerFnError::new("不是文件"));
    }
    let meta = std::fs::metadata(&target).map_err(|e| ServerFnError::new(e.to_string()))?;
    if meta.len() > 2 * 1024 * 1024 {
        return Err(ServerFnError::new("文件过大，无法在编辑器中打开"));
    }
    let bytes = std::fs::read(&target).map_err(|e| ServerFnError::new(e.to_string()))?;
    let header = &bytes[..bytes.len().min(64)];
    if kind_from_header(header) != FileKind::Text {
        return Err(ServerFnError::new("不是文本文件"));
    }
    Ok(decode_text(&bytes))
}

#[server]
pub async fn write_text_file(path: String, content: String) -> Result<(), ServerFnError> {
    if path.is_empty() {
        return Err(ServerFnError::new("不能写入根目录"));
    }
    if content.len() > 8 * 1024 * 1024 {
        return Err(ServerFnError::new("内容过大，无法保存"));
    }
    let target = resolve_path(&path).map_err(ServerFnError::new)?;
    if target.is_dir() {
        return Err(ServerFnError::new("不能写入目录"));
    }
    if target.exists() && sniff_file(&target) != FileKind::Text {
        return Err(ServerFnError::new("不是文本文件"));
    }
    std::fs::write(&target, content.as_bytes()).map_err(|e| ServerFnError::new(e.to_string()))
}
