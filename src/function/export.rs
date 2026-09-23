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
    use crate::function::path::validate_file_name;
    use crate::function::{pic_root, unique_dest};
    use crate::structure::FailureItem;
    use image::{DynamicImage, ImageEncoder, ImageFormat};
    use little_exif::filetype::get_file_type;
    use little_exif::metadata::Metadata;
    use std::collections::HashMap;
    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    use zip::ZipWriter;

    pub struct StoredExport {
        pub filename: String,
        pub mime: &'static str,
        pub bytes: Vec<u8>,
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

    pub fn put_download(filename: String, mime: &'static str, bytes: Vec<u8>) -> u64 {
        let id = next_id();
        let mut map = store().lock().unwrap_or_else(|p| p.into_inner());
        let now = Instant::now();
        map.retain(|_, v| now.duration_since(v.created) < Duration::from_secs(15 * 60));
        map.insert(
            id,
            StoredExport {
                filename,
                mime,
                bytes,
                created: now,
            },
        );
        id
    }

    pub fn take_download(id: u64) -> Option<StoredExport> {
        let mut map = store().lock().unwrap_or_else(|p| p.into_inner());
        map.remove(&id)
    }

    pub fn parse_format(fmt: &str) -> Result<(ImageFormat, &'static str), String> {
        match fmt.trim().to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Ok((ImageFormat::Jpeg, "jpg")),
            "png" => Ok((ImageFormat::Png, "png")),
            "webp" => Ok((ImageFormat::WebP, "webp")),
            "gif" => Ok((ImageFormat::Gif, "gif")),
            "bmp" => Ok((ImageFormat::Bmp, "bmp")),
            "tif" | "tiff" => Ok((ImageFormat::Tiff, "tiff")),
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
        let name = rel.rsplit('/').next().unwrap_or(rel);
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => name.to_string(),
        }
    }

    fn unique_zip_name(used: &mut HashMap<String, u32>, stem: &str, ext: &str) -> String {
        let base = format!("{stem}.{ext}");
        if !used.contains_key(&base) {
            used.insert(base.clone(), 0);
            return base;
        }
        let mut i = 1;
        loop {
            let name = format!("{stem} ({i}).{ext}");
            if !used.contains_key(&name) {
                used.insert(name.clone(), 0);
                return name;
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

    fn zip_files(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            for (name, data) in files {
                zip.start_file(name, opts)
                    .map_err(|e| format!("打包失败：{e}"))?;
                zip.write_all(data).map_err(|e| format!("打包失败：{e}"))?;
            }
            zip.finish().map_err(|e| format!("打包失败：{e}"))?;
        }
        Ok(cursor.into_inner())
    }

    pub fn export_batch(
        paths: Vec<String>,
        dest: String,
        dest_dir: String,
        format: String,
    ) -> Result<super::ExportReport, String> {
        let (format, ext) = parse_format(&format)?;
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
        let mut packed: Vec<(String, Vec<u8>)> = Vec::new();
        let mut used_names: HashMap<String, u32> = HashMap::new();

        for rel in unique {
            let name = rel.rsplit('/').next().unwrap_or(rel.as_str()).to_string();
            match crate::function::resolve_path(&rel) {
                Ok(full) if full.is_file() => match convert_file(&full, &rel, format, ext) {
                    Ok((out_name, bytes)) => {
                        if let Some(dir) = &out_dir {
                            let dest = unique_dest(dir, &out_name);
                            if let Err(e) = std::fs::write(&dest, &bytes) {
                                failures.push(FailureItem {
                                    file: rel,
                                    error: format!("写入失败：{e}"),
                                });
                                continue;
                            }
                        } else {
                            let zip_name = unique_zip_name(
                                &mut used_names,
                                &file_stem_name(&out_name),
                                ext,
                            );
                            packed.push((zip_name, bytes));
                        }
                        ok += 1;
                    }
                    Err(e) => failures.push(FailureItem { file: rel, error: e }),
                },
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

        let download_url = if to_download && !packed.is_empty() {
            if packed.len() == 1 {
                let (filename, bytes) = packed.remove(0);
                let mime = match ext {
                    "jpg" => "image/jpeg",
                    "png" => "image/png",
                    "webp" => "image/webp",
                    "gif" => "image/gif",
                    "bmp" => "image/bmp",
                    "tiff" => "image/tiff",
                    _ => "application/octet-stream",
                };
                let id = put_download(filename, mime, bytes);
                Some(format!("/export/{id}"))
            } else {
                let zip_bytes = zip_files(&packed)?;
                let id = put_download(
                    "pic-export.zip".into(),
                    "application/zip",
                    zip_bytes,
                );
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
pub async fn serve_export(
    axum::extract::Path(id): axum::extract::Path<u64>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::IntoResponse;

    let Some(file) = io::take_download(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut res = Body::from(file.bytes).into_response();
    if let Ok(val) = HeaderValue::from_str(file.mime) {
        res.headers_mut().insert(header::CONTENT_TYPE, val);
    }
    let disp = format!("attachment; filename=\"{}\"", file.filename.replace('"', ""));
    if let Ok(val) = HeaderValue::from_str(&disp) {
        res.headers_mut().insert(header::CONTENT_DISPOSITION, val);
    }
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    res
}
