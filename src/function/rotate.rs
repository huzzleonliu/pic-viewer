use leptos::prelude::*;

#[cfg(feature = "ssr")]
mod io {
    use image::{DynamicImage, ImageEncoder, ImageFormat};
    use little_exif::exif_tag::ExifTag;
    use little_exif::filetype::get_file_type;
    use little_exif::ifd::ExifTagGroup;
    use little_exif::metadata::Metadata;
    use std::io::Cursor;
    use std::path::Path;

    const ORIENTATION_TAG: u16 = 0x0112;

    fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
        let tmp = path.with_file_name(format!(
            ".{}.rot.tmp",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "image".into())
        ));
        if let Err(e) = std::fs::write(&tmp, bytes) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(())
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

    fn apply_metadata(path: &Path, buf: &mut Vec<u8>, original: &Vec<u8>) {
        let Ok(file_type) = get_file_type(path) else {
            return;
        };
        let Ok(mut metadata) = Metadata::new_from_vec(original, file_type) else {
            return;
        };
        metadata.set_tag(ExifTag::UnknownINT16U(
            vec![1],
            ORIENTATION_TAG,
            ExifTagGroup::GENERIC,
        ));
        let _ = metadata.write_to_vec(buf, file_type);
    }

    pub fn rotate_file(path: &Path, degrees: i32) -> Result<(), String> {
        let degrees = degrees.rem_euclid(360);
        if degrees == 0 {
            return Ok(());
        }
        if !matches!(degrees, 90 | 180 | 270) {
            return Err("只支持按 90° 的倍数旋转".into());
        }
        let format = ImageFormat::from_path(path).map_err(|_| "无法识别图片格式".to_string())?;
        let original = std::fs::read(path).map_err(|e| e.to_string())?;
        let img = image::load_from_memory(&original).map_err(|e| format!("无法解码图片：{e}"))?;
        let img = match degrees {
            90 => img.rotate90(),
            180 => img.rotate180(),
            270 => img.rotate270(),
            _ => img,
        };
        let mut buf = encode_image(&img, format)?;
        apply_metadata(path, &mut buf, &original);
        if buf.is_empty() {
            return Err("编码结果为空".into());
        }
        atomic_write(path, &buf)?;
        Ok(())
    }
}

#[server]
pub async fn save_rotated_image(path: String, degrees: i32) -> Result<(), ServerFnError> {
    if path.is_empty() {
        return Err(ServerFnError::new("未选择图片"));
    }
    let degrees = degrees.rem_euclid(360);
    crate::function::rating::with_image_path(path, move |p| io::rotate_file(p, degrees)).await
}
