use leptos::prelude::*;
use crate::structure::FailureItem;

#[cfg(feature = "ssr")]
mod io {
    use little_exif::endian::Endian;
    use little_exif::exif_tag::ExifTag;
    use little_exif::filetype::get_file_type;
    use little_exif::ifd::ExifTagGroup;
    use little_exif::metadata::Metadata;
    use std::path::Path;

    const RATING_TAG: u16 = 0x4746;
    const RATING_PERCENT_TAG: u16 = 0x4749;
    const DESCRIPTION_TAG: u16 = 0x010e;
    const USER_COMMENT_TAG: u16 = 0x9286;

    fn percent_for(rating: u8) -> u16 {
        match rating {
            0 => 0,
            1 => 1,
            2 => 25,
            3 => 50,
            4 => 75,
            _ => 99,
        }
    }

    fn no_exif(err: &std::io::Error) -> bool {
        err.to_string().contains("No EXIF")
    }

    fn load_metadata(path: &Path) -> Result<Metadata, String> {
        if get_file_type(path).is_err() {
            return Err("该格式不支持读取元数据".into());
        }
        match Metadata::new_from_path(path) {
            Ok(metadata) => Ok(metadata),
            Err(e) if no_exif(&e) => Ok(Metadata::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn with_metadata<T>(path: &Path, edit: impl FnOnce(&mut Metadata) -> T) -> Result<T, String> {
        let file_type = get_file_type(path).map_err(|_| "该格式不支持写入元数据".to_string())?;
        let mut buf = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut metadata = match Metadata::new_from_vec(&buf, file_type) {
            Ok(m) => m,
            Err(_) => Metadata::new(),
        };
        let result = edit(&mut metadata);
        metadata
            .write_to_vec(&mut buf, file_type)
            .map_err(|e| e.to_string())?;
        let tmp = path.with_file_name(format!(
            ".{}.meta.tmp",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "image".into())
        ));
        if let Err(e) = std::fs::write(&tmp, &buf) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(result)
    }

    fn parse_rating(metadata: &Metadata) -> u8 {
        let endian = metadata.get_endian();
        let Some(tag) = metadata.get_tag_by_hex(RATING_TAG, None).next() else {
            return 0;
        };
        let bytes = tag.value_as_u8_vec(&endian);
        if bytes.len() >= 2 {
            let v = match endian {
                Endian::Little => u16::from_le_bytes([bytes[0], bytes[1]]),
                Endian::Big => u16::from_be_bytes([bytes[0], bytes[1]]),
            };
            v.min(5) as u8
        } else {
            bytes.first().copied().unwrap_or(0).min(5)
        }
    }

    fn decode_utf16(bytes: &[u8], endian: &Endian) -> String {
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| match endian {
                Endian::Little => u16::from_le_bytes([c[0], c[1]]),
                Endian::Big => u16::from_be_bytes([c[0], c[1]]),
            })
            .collect();
        crate::function::filter::normalize_tag_text(&String::from_utf16_lossy(&units))
    }

    fn looks_like_utf16(bytes: &[u8]) -> bool {
        bytes.len() >= 2
            && bytes.len() % 2 == 0
            && bytes
                .chunks_exact(2)
                .filter(|c| (c[0] == 0) != (c[1] == 0))
                .count()
                * 2
                >= bytes.len() / 2
    }

    fn decode_user_comment(bytes: &[u8], endian: &Endian) -> String {
        if bytes.len() >= 8 && bytes.starts_with(b"UNICODE") {
            decode_utf16(&bytes[8..], endian)
        } else if bytes.len() >= 8
            && (bytes.starts_with(b"ASCII") || bytes[..8].iter().all(|&b| b == 0))
        {
            crate::function::filter::normalize_tag_text(&String::from_utf8_lossy(&bytes[8..]))
        } else if looks_like_utf16(bytes) {
            decode_utf16(bytes, endian)
        } else {
            crate::function::filter::normalize_tag_text(&String::from_utf8_lossy(bytes))
        }
    }

    fn encode_user_comment(text: &str, endian: &Endian) -> Vec<u8> {
        let mut out = b"UNICODE\0".to_vec();
        for unit in text.encode_utf16() {
            let bytes = match endian {
                Endian::Little => unit.to_le_bytes(),
                Endian::Big => unit.to_be_bytes(),
            };
            out.extend_from_slice(&bytes);
        }
        out
    }

    fn parse_tags(metadata: &Metadata) -> String {
        let endian = metadata.get_endian();
        if let Some(tag) = metadata.get_tag_by_hex(USER_COMMENT_TAG, None).next() {
            let text = decode_user_comment(&tag.value_as_u8_vec(&endian), &endian);
            if !text.is_empty() {
                return text;
            }
        }
        if let Some(tag) = metadata.get_tag_by_hex(DESCRIPTION_TAG, None).next() {
            return crate::function::filter::normalize_tag_text(&String::from_utf8_lossy(
                &tag.value_as_u8_vec(&endian),
            ));
        }
        String::new()
    }

    pub fn read_rating(path: &Path) -> Result<u8, String> {
        if get_file_type(path).is_err() {
            return Ok(0);
        }
        Ok(parse_rating(&load_metadata(path)?))
    }

    pub fn read_tags(path: &Path) -> Result<String, String> {
        if get_file_type(path).is_err() {
            return Ok(String::new());
        }
        Ok(parse_tags(&load_metadata(path)?))
    }

    fn write_rating_into(metadata: &mut Metadata, rating: u8) {
        metadata.set_tag(ExifTag::UnknownINT16U(
            vec![u16::from(rating)],
            RATING_TAG,
            ExifTagGroup::GENERIC,
        ));
        metadata.set_tag(ExifTag::UnknownINT16U(
            vec![percent_for(rating)],
            RATING_PERCENT_TAG,
            ExifTagGroup::GENERIC,
        ));
    }

    fn write_tags_into(metadata: &mut Metadata, tags: &str) {
        if tags.is_empty() {
            metadata.remove_tag(ExifTag::UserComment(Vec::new()));
            metadata.remove_tag(ExifTag::ImageDescription(String::new()));
        } else {
            let endian = metadata.get_endian();
            metadata.set_tag(ExifTag::UserComment(encode_user_comment(tags, &endian)));
            metadata.set_tag(ExifTag::ImageDescription(tags.to_string()));
        }
    }

    pub fn write_rating(path: &Path, rating: u8) -> Result<(), String> {
        with_metadata(path, |metadata| {
            write_rating_into(metadata, rating);
        })
    }

    pub fn write_tags(path: &Path, tags: &str) -> Result<(), String> {
        with_metadata(path, |metadata| {
            write_tags_into(metadata, tags);
        })
    }

    pub fn apply_mark(path: &Path, rating: u8, incoming_tags: &str) -> Result<String, String> {
        with_metadata(path, |metadata| {
            let existing = parse_tags(metadata);
            let merged = crate::function::filter::merge_tag_lists(&existing, incoming_tags);
            write_rating_into(metadata, rating);
            write_tags_into(metadata, &merged);
            merged
        })
    }
}

#[cfg(feature = "ssr")]
pub fn read_file_rating(path: &std::path::Path) -> Result<u8, String> {
    io::read_rating(path)
}

#[cfg(feature = "ssr")]
pub fn read_file_tags(path: &std::path::Path) -> Result<String, String> {
    io::read_tags(path)
}

#[cfg(feature = "ssr")]
pub async fn with_image_path<T, F>(path: String, work: F) -> Result<T, ServerFnError>
where
    T: Send + 'static,
    F: FnOnce(&std::path::Path) -> Result<T, String> + Send + 'static,
{
    if path.is_empty() {
        return Err(ServerFnError::new("未选择图片"));
    }
    let full = crate::function::resolve_path(&path).map_err(ServerFnError::new)?;
    if !full.is_file() {
        return Err(ServerFnError::new("文件不存在"));
    }
    tokio::task::spawn_blocking(move || work(&full))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .map_err(ServerFnError::new)
}

#[server]
pub async fn get_image_rating(path: String) -> Result<u8, ServerFnError> {
    if path.is_empty() {
        return Ok(0);
    }
    with_image_path(path, |p| io::read_rating(p)).await
}

#[server]
pub async fn set_image_rating(path: String, rating: u8) -> Result<u8, ServerFnError> {
    if rating > 5 {
        return Err(ServerFnError::new("星标须为 0–5"));
    }
    with_image_path(path.clone(), move |p| io::write_rating(p, rating).map(|()| rating)).await?;
    crate::function::filter::touch_index_rating(&path, rating);
    Ok(rating)
}

#[server]
pub async fn get_image_tags(path: String) -> Result<String, ServerFnError> {
    if path.is_empty() {
        return Ok(String::new());
    }
    with_image_path(path, |p| io::read_tags(p)).await
}

#[server]
pub async fn set_image_tags(path: String, tags: String) -> Result<(), ServerFnError> {
    with_image_path(path.clone(), {
        let tags = tags.clone();
        move |p| io::write_tags(p, &tags)
    })
    .await?;
    crate::function::filter::touch_index_tags(&path, &tags);
    Ok(())
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct BatchMarkReport {
    pub ok: u32,
    pub total: u32,
    pub failures: Vec<FailureItem>,
}

#[server]
pub async fn batch_mark_images(
    paths: Vec<String>,
    rating: u8,
    tags: String,
) -> Result<BatchMarkReport, ServerFnError> {
    if rating > 5 {
        return Err(ServerFnError::new("星标须为 0–5"));
    }
    let mut unique = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        if path.is_empty() || !seen.insert(path.clone()) {
            continue;
        }
        unique.push(path);
    }
    if unique.is_empty() {
        return Ok(BatchMarkReport {
            ok: 0,
            total: 0,
            failures: vec![FailureItem {
                file: String::new(),
                error: "没有已勾选的图片".into(),
            }],
        });
    }
    tokio::task::spawn_blocking(move || {
        let total = unique.len() as u32;
        let mut ok = 0u32;
        let mut failures = Vec::new();
        for rel in unique {
            let name = rel
                .rsplit('/')
                .next()
                .unwrap_or(rel.as_str())
                .to_string();
            match crate::function::resolve_path(&rel) {
                Ok(full) if full.is_file() => match io::apply_mark(&full, rating, &tags) {
                    Ok(merged) => {
                        crate::function::filter::touch_index_rating(&rel, rating);
                        crate::function::filter::touch_index_tags(&rel, &merged);
                        ok += 1;
                    }
                    Err(e) => {
                        failures.push(FailureItem { file: rel, error: e });
                    }
                },
                Ok(_) => {
                    failures.push(FailureItem {
                        file: name,
                        error: "不是文件".into(),
                    });
                }
                Err(e) => {
                    failures.push(FailureItem {
                        file: name,
                        error: e,
                    });
                }
            }
        }
        BatchMarkReport { ok, total, failures }
    })
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))
}
