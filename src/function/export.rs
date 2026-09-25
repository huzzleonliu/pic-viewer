use crate::structure::FailureItem;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportReport {
    pub ok: u32,
    pub total: u32,
    pub failures: Vec<FailureItem>,
    pub download_url: Option<String>,
}

#[cfg(feature = "ssr")]
mod io {
    use crate::function::path::{rel_name, validate_file_name};
    use crate::function::{mime_for, pic_root, unique_dest};
    use crate::structure::FailureItem;
    use image::{DynamicImage, ImageEncoder, ImageFormat};
    use little_exif::filetype::get_file_type;
    use little_exif::metadata::Metadata;
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    use zip::ZipWriter;

    pub struct StoredExport {
        pub filename: String,
        pub mime: &'static str,
        pub path: PathBuf,
        pub delete_after: bool,
        pub created: Instant,
    }

    pub fn store() -> &'static Mutex<HashMap<u64, StoredExport>> {
        static STORE: OnceLock<Mutex<HashMap<u64, StoredExport>>> = OnceLock::new();
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn next_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(1);
        SEQ.fetch_add(1, Ordering::Relaxed)
    }

    fn temp_path() -> PathBuf {
        std::env::temp_dir().join(format!("pic-viewer-export-{}", next_id()))
    }

    pub fn put_download_file(
        filename: String,
        mime: &'static str,
        path: PathBuf,
        delete_after: bool,
    ) -> u64 {
        let id = next_id();
        let mut map = store().lock().unwrap_or_else(|p| p.into_inner());
        let now = Instant::now();
        map.retain(|_, v| {
            let keep = now.duration_since(v.created) < Duration::from_secs(15 * 60);
            if !keep && v.delete_after {
                let _ = std::fs::remove_file(&v.path);
            }
            keep
        });
        map.insert(
            id,
            StoredExport {
                filename,
                mime,
                path,
                delete_after,
                created: now,
            },
        );
        id
    }

    pub fn take_download(id: u64) -> Option<StoredExport> {
        let mut map = store().lock().unwrap_or_else(|p| p.into_inner());
        map.remove(&id)
    }

    enum ExportKind {
        Convert(ImageFormat, &'static str),
        Original,
    }

    fn parse_kind(fmt: &str) -> Result<ExportKind, String> {
        match fmt.trim().to_ascii_lowercase().as_str() {
            "original" | "orig" => Ok(ExportKind::Original),
            "jpg" | "jpeg" => Ok(ExportKind::Convert(ImageFormat::Jpeg, "jpg")),
            "png" => Ok(ExportKind::Convert(ImageFormat::Png, "png")),
            "webp" => Ok(ExportKind::Convert(ImageFormat::WebP, "webp")),
            "gif" => Ok(ExportKind::Convert(ImageFormat::Gif, "gif")),
            "bmp" => Ok(ExportKind::Convert(ImageFormat::Bmp, "bmp")),
            "tif" | "tiff" => Ok(ExportKind::Convert(ImageFormat::Tiff, "tiff")),
            _ => Err("不支持的导出格式".into()),
        }
    }

    pub fn sanitize_rel_dir(rel: &str) -> Result<String, String> {
        let rel = rel.trim().trim_matches('/').replace('\\', "/");
        if rel.is_empty() {
            return Ok(String::new());
        }
        for part in rel.split('/') {
            if part.is_empty() {
                continue;
            }
            validate_file_name(part)?;
        }
        Ok(rel)
    }

    pub fn resolve_dest_dir(rel: &str) -> Result<PathBuf, String> {
        let rel = sanitize_rel_dir(rel)?;
        let root = pic_root();
        let dest = if rel.is_empty() {
            root.to_path_buf()
        } else {
            root.join(rel)
        };
        std::fs::create_dir_all(&dest).map_err(|e| format!("无法创建目录：{e}"))?;
        let canon = dest.canonicalize().map_err(|e| e.to_string())?;
        if !canon.starts_with(root) {
            return Err("路径越界".into());
        }
        if !canon.is_dir() {
            return Err("导出目标不是目录".into());
        }
        Ok(canon)
    }

    fn file_stem_name(rel: &str) -> String {
        let name = rel_name(rel);
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => name.to_string(),
        }
    }

    fn unique_out_name(used: &mut HashMap<String, u32>, name: &str) -> String {
        if !used.contains_key(name) {
            used.insert(name.to_string(), 0);
            return name.to_string();
        }
        let (stem, suffix) = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
            _ => (name, String::new()),
        };
        let mut i = 1;
        loop {
            let candidate = format!("{stem} ({i}){suffix}");
            if !used.contains_key(&candidate) {
                used.insert(candidate.clone(), 0);
                return candidate;
            }
            i += 1;
        }
    }

    fn encode_image(img: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, String> {
        let mut buf = Cursor::new(Vec::new());
        if format == ImageFormat::Jpeg {
            let rgb = img.to_rgb8();
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 92);
            enc.write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| e.to_string())?;
            return Ok(buf.into_inner());
        }
        img.write_to(&mut buf, format)
            .map_err(|e| format!("无法编码该格式：{e}"))?;
        Ok(buf.into_inner())
    }

    fn apply_exif(original: &[u8], original_path: &Path, dest_name: &str, buf: &mut Vec<u8>) {
        let dummy = Path::new(dest_name);
        let Ok(file_type) = get_file_type(dummy).or_else(|_| get_file_type(original_path)) else {
            return;
        };
        let original_vec = original.to_vec();
        let Ok(metadata) = Metadata::new_from_vec(&original_vec, file_type)
            .or_else(|_| Metadata::new_from_path(original_path))
        else {
            return;
        };
        let _ = metadata.write_to_vec(buf, file_type);
    }

    pub fn convert_file(
        full: &Path,
        rel: &str,
        format: ImageFormat,
        ext: &str,
    ) -> Result<(String, Vec<u8>), String> {
        let original = std::fs::read(full).map_err(|e| e.to_string())?;
        let img = image::load_from_memory(&original).map_err(|e| format!("无法解码：{e}"))?;
        let mut buf = encode_image(&img, format)?;
        let stem = file_stem_name(rel);
        let name = format!("{stem}.{ext}");
        apply_exif(&original, full, &name, &mut buf);
        if buf.is_empty() {
            return Err("编码结果为空".into());
        }
        Ok((name, buf))
    }

    fn zip_paths(files: &[(String, PathBuf)]) -> Result<PathBuf, String> {
        let zip_path = temp_path();
        let file = std::fs::File::create(&zip_path).map_err(|e| e.to_string())?;
        {
            let mut zip = ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            for (name, path) in files {
                zip.start_file(name, opts)
                    .map_err(|e| format!("打包失败：{e}"))?;
                let mut src = std::fs::File::open(path).map_err(|e| format!("打包失败：{e}"))?;
                std::io::copy(&mut src, &mut zip).map_err(|e| format!("打包失败：{e}"))?;
            }
            zip.finish().map_err(|e| format!("打包失败：{e}"))?;
        }
        Ok(zip_path)
    }

    fn write_temp_bytes(bytes: &[u8]) -> Result<PathBuf, String> {
        let path = temp_path();
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn export_batch(
        paths: Vec<String>,
        dest: String,
        dest_dir: String,
        format: String,
    ) -> Result<super::ExportReport, String> {
        let kind = parse_kind(&format)?;
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for path in paths {
            if path.is_empty() || !seen.insert(path.clone()) {
                continue;
            }
            unique.push(path);
        }
        let total = unique.len() as u32;
        if total == 0 {
            return Ok(super::ExportReport {
                ok: 0,
                total: 0,
                failures: vec![FailureItem {
                    file: String::new(),
                    error: "没有已勾选的图片".into(),
                }],
                download_url: None,
            });
        }

        let to_download = dest != "dir";
        let out_dir = if to_download {
            None
        } else {
            Some(resolve_dest_dir(&dest_dir)?)
        };

        let mut ok = 0u32;
        let mut failures = Vec::new();
        let mut staged: Vec<(String, PathBuf, bool)> = Vec::new();
        let mut used_names: HashMap<String, u32> = HashMap::new();

        for rel in unique {
            let name = rel_name(&rel).to_string();
            match crate::function::resolve_path(&rel) {
                Ok(full) if full.is_file() => {
                    let staged_file = match &kind {
                        ExportKind::Original => Ok((name.clone(), full.clone(), false)),
                        ExportKind::Convert(format, ext) => convert_file(&full, &rel, *format, ext)
                            .and_then(|(out_name, bytes)| {
                                let path = write_temp_bytes(&bytes)?;
                                Ok((out_name, path, true))
                            }),
                    };
                    match staged_file {
                        Ok((out_name, file_path, delete_after)) => {
                            if let Some(dir) = &out_dir {
                                let dest = unique_dest(dir, &out_name);
                                let copied = std::fs::copy(&file_path, &dest);
                                if delete_after {
                                    let _ = std::fs::remove_file(&file_path);
                                }
                                if let Err(e) = copied {
                                    failures.push(FailureItem {
                                        file: rel,
                                        error: format!("写入失败：{e}"),
                                    });
                                    continue;
                                }
                            } else {
                                staged.push((
                                    unique_out_name(&mut used_names, &out_name),
                                    file_path,
                                    delete_after,
                                ));
                            }
                            ok += 1;
                        }
                        Err(e) => failures.push(FailureItem {
                            file: rel,
                            error: e,
                        }),
                    }
                }
                Ok(_) => failures.push(FailureItem {
                    file: name,
                    error: "不是文件".into(),
                }),
                Err(e) => failures.push(FailureItem {
                    file: name,
                    error: e,
                }),
            }
        }

        let download_url = if to_download && !staged.is_empty() {
            if staged.len() == 1 {
                let (filename, path, delete_after) = staged.remove(0);
                let mime = match &kind {
                    ExportKind::Original => mime_for(Path::new(&filename)),
                    ExportKind::Convert(_, "jpg") => "image/jpeg",
                    ExportKind::Convert(_, "png") => "image/png",
                    ExportKind::Convert(_, "webp") => "image/webp",
                    ExportKind::Convert(_, "gif") => "image/gif",
                    ExportKind::Convert(_, "bmp") => "image/bmp",
                    ExportKind::Convert(_, "tiff") => "image/tiff",
                    ExportKind::Convert(_, _) => "application/octet-stream",
                };
                let id = put_download_file(filename, mime, path, delete_after);
                Some(format!("/export/{id}"))
            } else {
                let zip_entries: Vec<(String, PathBuf)> = staged
                    .iter()
                    .map(|(name, path, _)| (name.clone(), path.clone()))
                    .collect();
                let zip_path = zip_paths(&zip_entries);
                for (_, path, delete_after) in staged {
                    if delete_after {
                        let _ = std::fs::remove_file(path);
                    }
                }
                let zip_path = zip_path?;
                let id =
                    put_download_file("pic-export.zip".into(), "application/zip", zip_path, true);
                Some(format!("/export/{id}"))
            }
        } else {
            None
        };

        Ok(super::ExportReport {
            ok,
            total,
            failures,
            download_url,
        })
    }
}

#[server]
pub async fn export_checked(
    paths: Vec<String>,
    dest: String,
    dest_dir: String,
    format: String,
) -> Result<ExportReport, ServerFnError> {
    tokio::task::spawn_blocking(move || io::export_batch(paths, dest, dest_dir, format))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
struct CleanupStream {
    inner: tokio_util::io::ReaderStream<tokio::fs::File>,
    cleanup: Option<std::path::PathBuf>,
}

#[cfg(feature = "ssr")]
impl futures_util::Stream for CleanupStream {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let inner = std::pin::Pin::new(&mut this.inner);
        match futures_util::Stream::poll_next(inner, cx) {
            std::task::Poll::Ready(None) => {
                if let Some(path) = this.cleanup.take() {
                    let _ = std::fs::remove_file(path);
                }
                std::task::Poll::Ready(None)
            }
            other => other,
        }
    }
}

#[cfg(feature = "ssr")]
impl Drop for CleanupStream {
    fn drop(&mut self) {
        if let Some(path) = self.cleanup.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn serve_export(
    axum::extract::Path(id): axum::extract::Path<u64>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::IntoResponse;
    use tokio_util::io::ReaderStream;

    let Some(file) = io::take_download(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let reader = match tokio::fs::File::open(&file.path).await {
        Ok(reader) => reader,
        Err(_) => {
            if file.delete_after {
                let _ = std::fs::remove_file(&file.path);
            }
            return StatusCode::NOT_FOUND.into_response();
        }
    };
    let cleanup = if file.delete_after {
        Some(file.path)
    } else {
        None
    };
    let mut res = Body::from_stream(CleanupStream {
        inner: ReaderStream::new(reader),
        cleanup,
    })
    .into_response();
    if let Ok(val) = HeaderValue::from_str(file.mime) {
        res.headers_mut().insert(header::CONTENT_TYPE, val);
    }
    let disp = format!(
        "attachment; filename=\"{}\"",
        file.filename.replace('"', "")
    );
    if let Ok(val) = HeaderValue::from_str(&disp) {
        res.headers_mut().insert(header::CONTENT_DISPOSITION, val);
    }
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}
