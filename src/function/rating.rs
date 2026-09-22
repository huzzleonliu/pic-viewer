use leptos::prelude::*;

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

    fn with_metadata(path: &Path, edit: impl FnOnce(&mut Metadata)) -> Result<(), String> {
        let file_type = get_file_type(path).map_err(|_| "该格式不支持写入元数据".to_string())?;
        let mut buf = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut metadata = match Metadata::new_from_vec(&buf, file_type) {
            Ok(m) => m,
            Err(_) => Metadata::new(),
        };
        edit(&mut metadata);
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
        Ok(())
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

    fn decode_user_comment(bytes: &[u8], endian: &Endian) -> String {
        if bytes.len() < 8 {
            return String::from_utf8_lossy(bytes)
                .trim_end_matches('\0')
                .to_string();
        }
        let header = &bytes[..8];
        let rest = &bytes[8..];
        if header.starts_with(b"UNICODE") {
            let units: Vec<u16> = rest
                .chunks_exact(2)
                .map(|c| match endian {
                    Endian::Little => u16::from_le_bytes([c[0], c[1]]),
                    Endian::Big => u16::from_be_bytes([c[0], c[1]]),
                })
                .collect();
            String::from_utf16_lossy(&units)
                .trim_end_matches('\0')
                .to_string()
        } else if header.starts_with(b"ASCII") || header.iter().all(|&b| b == 0) {
            String::from_utf8_lossy(rest)
                .trim_end_matches('\0')
                .to_string()
        } else {
            String::from_utf8_lossy(rest)
                .trim_end_matches('\0')
                .to_string()
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
            return String::from_utf8_lossy(&tag.value_as_u8_vec(&endian))
                .trim_end_matches('\0')
                .to_string();
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

    pub fn write_rating(path: &Path, rating: u8) -> Result<(), String> {
        with_metadata(path, |metadata| {
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
        })
    }

    pub fn write_tags(path: &Path, tags: &str) -> Result<(), String> {
        with_metadata(path, |metadata| {
            if tags.is_empty() {
                metadata.remove_tag(ExifTag::UserComment(Vec::new()));
                metadata.remove_tag(ExifTag::ImageDescription(String::new()));
            } else {
                let endian = metadata.get_endian();
                metadata.set_tag(ExifTag::UserComment(encode_user_comment(tags, &endian)));
                metadata.set_tag(ExifTag::ImageDescription(tags.to_string()));
            }
        })
    }
}

#[cfg(feature = "ssr")]
async fn with_image_path<T, F>(path: String, work: F) -> Result<T, ServerFnError>
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
    with_image_path(path, move |p| io::write_rating(p, rating).map(|()| rating)).await
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
    with_image_path(path, move |p| io::write_tags(p, &tags)).await
}
