/// Classify a file from its first bytes. Does not look at the name or extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Image,
    Video,
    Text,
    Other,
}

pub fn decode_text(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\xff\xfe") {
        utf16_lossy(&bytes[2..], false)
    } else if bytes.starts_with(b"\xfe\xff") {
        utf16_lossy(&bytes[2..], true)
    } else {
        let rest = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
        String::from_utf8_lossy(rest).into_owned()
    }
}

fn utf16_lossy(bytes: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if big_endian {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

pub fn kind_from_header(buf: &[u8]) -> FileKind {
    if let Some(kind) = binary_kind(buf) {
        return kind;
    }
    if looks_like_text(buf) {
        FileKind::Text
    } else {
        FileKind::Other
    }
}

fn binary_kind(buf: &[u8]) -> Option<FileKind> {
    if buf.len() >= 8 && buf.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(FileKind::Image);
    }
    if buf.len() >= 3 && buf.starts_with(b"\xff\xd8\xff") {
        return Some(FileKind::Image);
    }
    if buf.len() >= 6 && (buf.starts_with(b"GIF87a") || buf.starts_with(b"GIF89a")) {
        return Some(FileKind::Image);
    }
    if buf.len() >= 2 && buf.starts_with(b"BM") {
        return Some(FileKind::Image);
    }
    if buf.len() >= 4 && (buf.starts_with(b"II*\0") || buf.starts_with(b"MM\0*")) {
        return Some(FileKind::Image);
    }
    if buf.len() >= 4 && buf.starts_with(b"\0\0\x01\0") {
        return Some(FileKind::Image);
    }
    if buf.len() >= 12 && buf.starts_with(b"RIFF") {
        return Some(match &buf[8..12] {
            b"WEBP" => FileKind::Image,
            b"AVI " => FileKind::Video,
            _ => FileKind::Other,
        });
    }
    if buf.len() >= 12 && &buf[4..8] == b"ftyp" {
        let brand = &buf[8..12];
        if matches!(
            brand,
            b"avif" | b"avis" | b"heic" | b"heif" | b"mif1" | b"msf1"
        ) {
            return Some(FileKind::Image);
        }
        return Some(FileKind::Video);
    }
    if buf.len() >= 4 && buf.starts_with(b"\x1a\x45\xdf\xa3") {
        return Some(FileKind::Video);
    }
    if buf.len() >= 4 && buf.starts_with(b"%PDF") {
        return Some(FileKind::Other);
    }
    if buf.len() >= 4 && buf.starts_with(b"PK\x03\x04") {
        return Some(FileKind::Other);
    }
    if buf.len() >= 3 && buf.starts_with(b"ID3") {
        return Some(FileKind::Other);
    }
    None
}

fn looks_like_text(buf: &[u8]) -> bool {
    if buf.is_empty() {
        return true;
    }
    if buf.starts_with(b"\xff\xfe") || buf.starts_with(b"\xfe\xff") {
        return true;
    }
    let rest = if buf.starts_with(b"\xef\xbb\xbf") {
        &buf[3.min(buf.len())..]
    } else {
        buf
    };
    if rest.iter().any(|&b| b == 0) {
        return false;
    }
    let usable = match std::str::from_utf8(rest) {
        Ok(_) => rest,
        Err(err) => {
            let up = err.valid_up_to();
            if up == 0 || rest.len() - up > 3 {
                return false;
            }
            &rest[..up]
        }
    };
    std::str::from_utf8(usable)
        .map(|s| {
            s.chars()
                .all(|c| matches!(c, '\t' | '\n' | '\r') || !c.is_control())
        })
        .unwrap_or(false)
}

pub fn kind_from_ext(ext: &str) -> Option<FileKind> {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "avif" | "ico" => {
            Some(FileKind::Image)
        }
        "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" => Some(FileKind::Video),
        "txt" | "md" | "markdown" | "json" | "csv" | "toml" | "yaml" | "yml" | "rs" | "js"
        | "ts" | "css" | "html" | "htm" | "xml" | "log" | "ini" | "conf" | "sh" => {
            Some(FileKind::Text)
        }
        _ => None,
    }
}

#[cfg(feature = "ssr")]
pub fn sniff_file(path: &std::path::Path) -> FileKind {
    use std::io::Read;
    let mut buf = [0u8; 64];
    let Ok(mut file) = std::fs::File::open(path) else {
        return FileKind::Other;
    };
    let n = file.read(&mut buf).unwrap_or(0);
    kind_from_header(&buf[..n])
}

#[cfg(feature = "ssr")]
pub fn classify_file(path: &std::path::Path) -> FileKind {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        if let Some(kind) = kind_from_ext(ext) {
            return kind;
        }
    }
    sniff_file(path)
}

#[cfg(test)]
mod tests {
    use super::{decode_text, kind_from_ext, kind_from_header, FileKind};

    #[test]
    fn png_jpeg_gif_webp() {
        assert_eq!(kind_from_header(b"\x89PNG\r\n\x1a\nrest"), FileKind::Image);
        assert_eq!(kind_from_header(b"\xff\xd8\xff\xe0...."), FileKind::Image);
        assert_eq!(kind_from_header(b"GIF89a...."), FileKind::Image);
        let mut webp = [0u8; 16];
        webp[..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        assert_eq!(kind_from_header(&webp), FileKind::Image);
    }

    #[test]
    fn mp4_is_video_not_text() {
        let mut ftyp = [0u8; 16];
        ftyp[4..8].copy_from_slice(b"ftyp");
        ftyp[8..12].copy_from_slice(b"isom");
        assert_eq!(kind_from_header(&ftyp), FileKind::Video);
    }

    #[test]
    fn utf8_and_svg_are_text() {
        assert_eq!(kind_from_header(b""), FileKind::Text);
        assert_eq!(kind_from_header("hello\nworld".as_bytes()), FileKind::Text);
        assert_eq!(kind_from_header("备注：测试".as_bytes()), FileKind::Text);
        assert_eq!(
            kind_from_header(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"),
            FileKind::Text
        );
        assert_eq!(kind_from_header(b"{\"ok\": true}\n"), FileKind::Text);
    }

    #[test]
    fn nul_and_zip_not_text() {
        assert_eq!(kind_from_header(b"hello\0world"), FileKind::Other);
        assert_eq!(kind_from_header(b"PK\x03\x04...."), FileKind::Other);
        assert_eq!(kind_from_header(b"%PDF-1.7...."), FileKind::Other);
    }

    #[test]
    fn utf16_bom_is_text() {
        assert_eq!(kind_from_header(b"\xff\xfeh\0i\0"), FileKind::Text);
    }

    #[test]
    fn extension_fast_path() {
        assert_eq!(kind_from_ext("JPG"), Some(FileKind::Image));
        assert_eq!(kind_from_ext("png"), Some(FileKind::Image));
        assert_eq!(kind_from_ext("mp4"), Some(FileKind::Video));
        assert_eq!(kind_from_ext("txt"), Some(FileKind::Text));
        assert_eq!(kind_from_ext("svg"), None);
        assert_eq!(kind_from_ext("bin"), None);
    }

    #[test]
    fn decode_utf8_utf16_and_bom() {
        assert_eq!(decode_text("备注".as_bytes()), "备注");
        assert_eq!(decode_text(b"\xef\xbb\xbfhi"), "hi");
        assert_eq!(decode_text(b"\xff\xfeh\0i\0"), "hi");
        assert_eq!(decode_text(b"\xfe\xff\0h\0i"), "hi");
    }
}
