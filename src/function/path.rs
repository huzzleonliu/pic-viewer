use crate::structure::fs::IMAGE_EXTS;

pub fn is_image_name(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| IMAGE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

pub fn rel_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
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

pub fn validate_rename_prefix(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name == "." || name == ".." {
        return Err("非法名称".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("名称不能包含路径分隔符".into());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("名称包含非法字符".into());
    }
    Ok(name.to_string())
}

pub fn name_extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[i..],
        _ => "",
    }
}

pub fn numbered_file_name(prefix: &str, n: i32, original_name: &str) -> String {
    format!("{prefix}{n}{}", name_extension(original_name))
}

pub fn number_run(start: i32, end: i32) -> Vec<i32> {
    if start <= end {
        (start..=end).collect()
    } else {
        (end..=start).rev().collect()
    }
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

#[cfg(test)]
mod tests {
    use super::{
        join_rel, number_run, numbered_file_name, parent_path, rel_name, rewrite_prefix,
        validate_file_name, validate_rename_prefix,
    };

    #[test]
    fn rewrite_replaces_exact_path() {
        assert_eq!(rewrite_prefix("album/a", "album/a", "dest/a"), "dest/a");
    }

    #[test]
    fn rewrite_replaces_children_only_with_slash() {
        assert_eq!(
            rewrite_prefix("album/a/b.jpg", "album/a", "dest/a"),
            "dest/a/b.jpg"
        );
        assert_eq!(rewrite_prefix("album/ab", "album/a", "dest/a"), "album/ab");
        assert_eq!(rewrite_prefix("other", "album/a", "dest/a"), "other");
    }

    #[test]
    fn rewrite_empty_old_does_not_prefix() {
        assert_eq!(rewrite_prefix("album/a", "", "x"), "album/a");
    }

    #[test]
    fn rel_parent_join() {
        assert_eq!(rel_name("album/a/b.jpg"), "b.jpg");
        assert_eq!(rel_name("b.jpg"), "b.jpg");
        assert_eq!(parent_path("album/a/b.jpg"), "album/a");
        assert_eq!(parent_path("b.jpg"), "");
        assert_eq!(join_rel("", "x"), "x");
        assert_eq!(join_rel("album", "x"), "album/x");
    }

    #[test]
    fn validate_name_rejects_separators() {
        assert!(validate_file_name("ok.txt").is_ok());
        assert!(validate_file_name("").is_err());
        assert!(validate_file_name("a/b").is_err());
        assert!(validate_file_name("..").is_err());
    }

    #[test]
    fn numbered_names_keep_ext_no_pad() {
        assert_eq!(numbered_file_name("img", 1, "a.jpg"), "img1.jpg");
        assert_eq!(numbered_file_name("", 12, "a.jpg"), "12.jpg");
        assert_eq!(numbered_file_name("x", 2, "noext"), "x2");
        assert_eq!(numbered_file_name("", 1, ".bashrc"), "1");
        assert_eq!(numbered_file_name("a", 3, "b.tar.gz"), "a3.gz");
    }

    #[test]
    fn number_run_inclusive() {
        assert_eq!(number_run(1, 3), vec![1, 2, 3]);
        assert_eq!(number_run(3, 1), vec![3, 2, 1]);
        assert_eq!(number_run(5, 5), vec![5]);
    }

    #[test]
    fn rename_prefix_allows_empty() {
        assert_eq!(validate_rename_prefix("  ").unwrap(), "");
        assert!(validate_rename_prefix("a/b").is_err());
    }
}
