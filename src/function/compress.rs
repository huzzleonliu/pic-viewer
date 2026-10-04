use crate::structure::CompressImagesReport;
use leptos::prelude::*;

#[server]
pub async fn compress_checked_images(
    paths: Vec<String>,
) -> Result<CompressImagesReport, ServerFnError> {
    #[cfg(not(feature = "ssr"))]
    {
        let _ = paths;
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        io::compress_batch(paths).await.map_err(ServerFnError::new)
    }
}

#[cfg(feature = "ssr")]
mod io {
    use crate::function::fs::{imgproxy_webp_bytes, pic_root, resolve_path, to_rel, unique_dest};
    use crate::function::path::rel_name;
    use crate::function::sniff::{
        is_jpeg_bytes, kind_from_ext, kind_from_header, skip_compress_to_webp, FileKind,
    };
    use crate::structure::{CompressImagesReport, FailureItem};
    use image::{DynamicImage, ExtendedColorType};
    use std::collections::HashSet;
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    const WEBP_QUALITY: f32 = 95.0;

    enum Outcome {
        Converted(String, String),
        Skipped,
    }

    pub async fn compress_batch(paths: Vec<String>) -> Result<CompressImagesReport, String> {
        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        for path in paths {
            if path.is_empty() || !seen.insert(path.clone()) {
                continue;
            }
            unique.push(path);
        }

        let mut report = CompressImagesReport {
            ok: 0,
            skipped: 0,
            converted: Vec::new(),
            failures: Vec::new(),
        };
        for (i, path) in unique.into_iter().enumerate() {
            match compress_one(&path, i).await {
                Ok(Outcome::Converted(old, new)) => {
                    report.ok += 1;
                    report.converted.push((old, new));
                }
                Ok(Outcome::Skipped) => report.skipped += 1,
                Err(error) => report.failures.push(FailureItem {
                    file: rel_name(&path).to_string(),
                    error,
                }),
            }
        }
        Ok(report)
    }

    async fn compress_one(rel: &str, idx: usize) -> Result<Outcome, String> {
        let full = resolve_path(rel)?;
        if full == pic_root() {
            return Ok(Outcome::Skipped);
        }
        if !full.is_file() {
            return Ok(Outcome::Skipped);
        }
        if skip_by_name(rel) {
            return Ok(Outcome::Skipped);
        }
        let original = tokio::fs::read(&full)
            .await
            .map_err(|e| format!("读取失败：{e}"))?;
        if skip_compress_to_webp(rel, &original) {
            return Ok(Outcome::Skipped);
        }
        let header = &original[..original.len().min(64)];
        if kind_from_header(header) != FileKind::Image {
            return Ok(Outcome::Skipped);
        }

        let encoded = match encode_webp(original.clone()).await {
            Ok(buf) => buf,
            Err(inner) => match imgproxy_webp_bytes(rel).await {
                Ok(buf) => buf,
                Err(_) => return Err(inner),
            },
        };

        let parent = full.parent().ok_or_else(|| "非法路径".to_string())?;
        let stem = file_stem(rel_name(rel));
        let dest = unique_dest(parent, &format!("{stem}.webp"));
        atomic_replace(&full, &dest, &encoded, idx)?;
        Ok(Outcome::Converted(rel.to_string(), to_rel(&dest)))
    }

    fn skip_by_name(rel: &str) -> bool {
        let name = rel_name(rel);
        let ext = name
            .rsplit_once('.')
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        matches!(ext.as_str(), "gif" | "webp")
            || kind_from_ext(&ext).is_some_and(|k| k != FileKind::Image)
    }

    fn file_stem(name: &str) -> String {
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => name.to_string(),
        }
    }

    fn atomic_replace(from: &Path, dest: &Path, bytes: &[u8], idx: usize) -> Result<(), String> {
        let parent = dest.parent().ok_or_else(|| "非法路径".to_string())?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tmp = unique_dest(parent, &format!(".__pv_cwebp_{stamp}_{idx}.webp"));
        if let Err(e) = std::fs::write(&tmp, bytes) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        if let Err(e) = std::fs::rename(&tmp, dest) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        if from != dest {
            let _ = std::fs::remove_file(from);
        }
        Ok(())
    }

    async fn encode_webp(original: Vec<u8>) -> Result<Vec<u8>, String> {
        tokio::task::spawn_blocking(move || encode_webp_sync(&original))
            .await
            .map_err(|e| e.to_string())?
    }

    fn encode_webp_sync(original: &[u8]) -> Result<Vec<u8>, String> {
        let img = image::load_from_memory(original).map_err(|e| format!("无法解码：{e}"))?;
        if is_jpeg_bytes(original) {
            encode_lossy(&img, WEBP_QUALITY)
        } else {
            let lossless = encode_lossless(&img)?;
            if lossless.len() < original.len() {
                Ok(lossless)
            } else {
                encode_lossy(&img, WEBP_QUALITY)
            }
        }
    }

    fn encode_lossless(img: &DynamicImage) -> Result<Vec<u8>, String> {
        let mut buf = Cursor::new(Vec::new());
        if img.color().has_alpha() {
            let rgba = img.to_rgba8();
            image::codecs::webp::WebPEncoder::new_lossless(&mut buf)
                .encode(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .map_err(|e| format!("无法编码 WebP：{e}"))?;
        } else {
            let rgb = img.to_rgb8();
            image::codecs::webp::WebPEncoder::new_lossless(&mut buf)
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|e| format!("无法编码 WebP：{e}"))?;
        }
        let out = buf.into_inner();
        if out.is_empty() {
            Err("编码结果为空".into())
        } else {
            Ok(out)
        }
    }

    fn encode_lossy(img: &DynamicImage, quality: f32) -> Result<Vec<u8>, String> {
        let mem = if img.color().has_alpha() {
            let rgba = img.to_rgba8();
            webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height()).encode(quality)
        } else {
            let rgb = img.to_rgb8();
            webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height()).encode(quality)
        };
        if mem.is_empty() {
            Err("编码结果为空".into())
        } else {
            Ok(mem.to_vec())
        }
    }

    #[cfg(test)]
    pub(super) fn encode_webp_sync_for_test(original: &[u8]) -> Result<Vec<u8>, String> {
        encode_webp_sync(original)
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::io::encode_webp_sync_for_test;
    use crate::function::sniff::is_webp_bytes;
    use image::{ExtendedColorType, ImageEncoder, Rgb};

    fn sample_png() -> Vec<u8> {
        let mut img = image::RgbImage::new(48, 48);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = Rgb([(x * 5) as u8, (y * 5) as u8, 180]);
        }
        let mut buf = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(img.as_raw(), 48, 48, ExtendedColorType::Rgb8)
            .expect("png");
        buf
    }

    fn sample_jpeg() -> Vec<u8> {
        let mut img = image::RgbImage::new(64, 64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = Rgb([((x + y) % 256) as u8, (x * 3) as u8, (y * 4) as u8]);
        }
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95)
            .write_image(img.as_raw(), 64, 64, ExtendedColorType::Rgb8)
            .expect("jpeg");
        buf
    }

    #[test]
    fn png_encodes_to_smaller_webp() {
        let png = sample_png();
        let webp = encode_webp_sync_for_test(&png).expect("encode png");
        assert!(is_webp_bytes(&webp));
        assert!(webp.len() < png.len());
    }

    #[test]
    fn jpeg_encodes_to_webp_without_crushing() {
        let jpeg = sample_jpeg();
        let webp = encode_webp_sync_for_test(&jpeg).expect("encode jpeg");
        assert!(is_webp_bytes(&webp));
        // 小图 JPEG 的容器开销大，体积比不能代表实拍照片，这里只断言产出合法 WebP。
    }
}
