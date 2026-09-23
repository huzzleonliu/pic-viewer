use crate::structure::fs::IMAGE_EXTS;

pub fn is_image_name(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| IMAGE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

pub fn parent_path(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

pub fn join_rel(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

pub fn validate_file_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("名称不能为空".into());
    }
    if name == "." || name == ".." {
        return Err("非法名称".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("名称不能包含路径分隔符".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("名称包含非法字符".into());
    }
    Ok(())
}

pub fn rewrite_prefix(path: &str, old: &str, new: &str) -> String {
    if path == old {
        new.to_string()
    } else if !old.is_empty() && path.starts_with(old) && path[old.len()..].starts_with('/') {
        format!("{new}{}", &path[old.len()..])
    } else {
        path.to_string()
    }
}

fn encode_rel(rel: &str) -> String {
    rel.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| urlencoding::encode(s).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn media_url(rel: &str) -> String {
    format!("/media/{}", encode_rel(rel))
}

pub fn thumb_url(rel: &str) -> String {
    format!("/thumb/{}", encode_rel(rel))
}

pub fn preview_url(rel: &str) -> String {
    format!("/preview/{}", encode_rel(rel))
}
