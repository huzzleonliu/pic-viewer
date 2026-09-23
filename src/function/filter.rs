use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "ssr", test))]
pub fn normalize_tag_text(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\0')
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(any(feature = "ssr", test))]
fn tag_tokens(s: &str) -> Vec<String> {
    normalize_tag_text(s)
        .split(|c: char| matches!(c, ',' | ';' | '，' | '、'))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(any(feature = "ssr", test))]
pub fn merge_tag_lists(existing: &str, incoming: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for token in tag_tokens(existing)
        .into_iter()
        .chain(tag_tokens(incoming))
    {
        let key = token.to_lowercase();
        if seen.iter().any(|s| s == &key) {
            continue;
        }
        seen.push(key);
        out.push(token);
    }
    out.join(", ")
}

#[cfg(any(feature = "ssr", test))]
fn tags_contain(tags: &str, query: &str) -> bool {
    let query = normalize_tag_text(query);
    if query.is_empty() {
        return true;
    }
    let normalized = normalize_tag_text(tags);
    if normalized.contains(&query) {
        return true;
    }
    tag_tokens(tags)
        .iter()
        .any(|t| t == &query || t.contains(&query))
}

#[cfg(any(feature = "ssr", test))]
fn tags_match(tags: &str, mode: &str, query: &str) -> bool {
    let query = normalize_tag_text(query);
    if query.is_empty() {
        return true;
    }
    let normalized = normalize_tag_text(tags);
    let tokens = tag_tokens(tags);
    match mode {
        "eq" => normalized == query || tokens.iter().any(|t| t == &query),
        "excludes" => !tags_contain(tags, &query),
        _ => tags_contain(tags, &query),
    }
}

#[cfg(any(feature = "ssr", test))]
fn star_match(rating: u8, op: &str, val: u8) -> bool {
    match op {
        "any" | "" => true,
        "lt" => rating < val,
        "le" => rating <= val,
        "ge" => rating >= val,
        "gt" => rating > val,
        _ => rating == val,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetaScanStatus {
    pub ready: bool,
    pub done: u32,
    pub total: u32,
}

#[cfg(feature = "ssr")]
mod store {
    use rusqlite::Connection;
    use std::sync::{Mutex, OnceLock};

    pub struct IndexState {
        pub conn: Connection,
        pub dir: String,
        pub epoch: u64,
        pub gen: u64,
        pub total: u32,
        pub ready: bool,
    }

    pub fn state() -> &'static Mutex<IndexState> {
        static STATE: OnceLock<Mutex<IndexState>> = OnceLock::new();
        STATE.get_or_init(|| {
            let conn = Connection::open_in_memory().expect("open in-memory sqlite");
            conn.execute(
                "CREATE TABLE images (
                    path TEXT PRIMARY KEY,
                    rating INTEGER NOT NULL,
                    tags TEXT NOT NULL,
                    seq INTEGER NOT NULL
                )",
                [],
            )
            .expect("create images table");
            Mutex::new(IndexState {
                conn,
                dir: String::new(),
                epoch: u64::MAX,
                gen: 0,
                total: 0,
                ready: false,
            })
        })
    }

    pub fn lock() -> std::sync::MutexGuard<'static, IndexState> {
        state()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

#[cfg(feature = "ssr")]
fn list_image_rels(dir: &str) -> Result<Vec<String>, String> {
    use crate::function::is_image_name;
    use crate::function::path::join_rel;
    use std::fs;

    let full = crate::function::resolve_path(dir)?;
    if !full.is_dir() {
        return Err("不是目录".into());
    }
    let mut rels = Vec::new();
    let read = fs::read_dir(&full).map_err(|e| e.to_string())?;
    for item in read {
        let item = match item {
            Ok(v) => v,
            Err(_) => continue,
        };
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir || !is_image_name(&name) {
            continue;
        }
        rels.push(join_rel(dir, &name));
    }
    rels.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(rels)
}

#[cfg(feature = "ssr")]
struct ReadyOnDrop {
    gen: u64,
    dir: String,
}

#[cfg(feature = "ssr")]
impl Drop for ReadyOnDrop {
    fn drop(&mut self) {
        let mut st = store::lock();
        if st.gen == self.gen && st.dir == self.dir {
            st.ready = true;
        }
    }
}

#[cfg(feature = "ssr")]
fn spawn_scan(dir: String, paths: Vec<String>, gen: u64) {
    tokio::task::spawn_blocking(move || {
        let _guard = ReadyOnDrop {
            gen,
            dir: dir.clone(),
        };
        for (seq, rel) in paths.into_iter().enumerate() {
            {
                let st = store::lock();
                if st.gen != gen {
                    return;
                }
            }
            let Ok(full) = crate::function::resolve_path(&rel) else {
                continue;
            };
            let (rating, tags) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let rating = crate::function::rating::read_file_rating(&full).unwrap_or(0);
                let tags = normalize_tag_text(
                    &crate::function::rating::read_file_tags(&full).unwrap_or_default(),
                );
                (rating, tags)
            }))
            .unwrap_or_else(|_| (0, String::new()));
            let st = store::lock();
            if st.gen != gen {
                return;
            }
            let _ = st.conn.execute(
                "INSERT OR REPLACE INTO images (path, rating, tags, seq) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![rel, rating, tags, seq as i64],
            );
        }
    });
}

#[cfg(feature = "ssr")]
pub fn touch_index_rating(rel: &str, rating: u8) {
    let st = store::lock();
    let _ = st.conn.execute(
        "UPDATE images SET rating = ?1 WHERE path = ?2",
        rusqlite::params![rating, rel],
    );
}

#[cfg(feature = "ssr")]
pub fn touch_index_tags(rel: &str, tags: &str) {
    let tags = normalize_tag_text(tags);
    let st = store::lock();
    let _ = st.conn.execute(
        "UPDATE images SET tags = ?1 WHERE path = ?2",
        rusqlite::params![tags, rel],
    );
}

#[server]
pub async fn start_meta_index(dir: String, epoch: u64) -> Result<(), ServerFnError> {
    let gen = {
        let mut st = store::lock();
        if st.gen > 0 && st.dir == dir && st.epoch == epoch {
            return Ok(());
        }
        st.gen += 1;
        let gen = st.gen;
        st.dir = dir.clone();
        st.epoch = epoch;
        st.total = 0;
        st.ready = false;
        st.conn
            .execute("DELETE FROM images", [])
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        gen
    };
    let scan_dir = dir.clone();
    let paths = match tokio::task::spawn_blocking(move || list_image_rels(&scan_dir))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        Ok(paths) => paths,
        Err(e) => {
            let mut st = store::lock();
            if st.gen == gen {
                st.ready = true;
            }
            return Err(ServerFnError::new(e));
        }
    };
    {
        let mut st = store::lock();
        if st.gen != gen {
            return Ok(());
        }
        st.total = paths.len() as u32;
        if paths.is_empty() {
            st.ready = true;
            return Ok(());
        }
    }
    spawn_scan(dir, paths, gen);
    Ok(())
}

#[server]
pub async fn get_meta_index_status(dir: String) -> Result<MetaScanStatus, ServerFnError> {
    let st = store::lock();
    if st.dir != dir {
        return Ok(MetaScanStatus {
            ready: false,
            done: 0,
            total: 0,
        });
    }
    let done: u32 = st
        .conn
        .query_row("SELECT COUNT(*) FROM images", [], |row| row.get(0))
        .unwrap_or(0);
    Ok(MetaScanStatus {
        ready: st.ready,
        done,
        total: st.total,
    })
}

#[server]
pub async fn apply_meta_filter(
    dir: String,
    star_op: String,
    star_val: u8,
    tag_mode: String,
    tag_query: String,
    complement: bool,
) -> Result<Vec<String>, ServerFnError> {
    let rows = {
        let st = store::lock();
        if st.dir != dir {
            return Err(ServerFnError::new("目录已切换，请等待元数据读取"));
        }
        if !st.ready {
            return Err(ServerFnError::new("元数据尚未读完"));
        }
        let mut stmt = st
            .conn
            .prepare("SELECT path, rating, tags FROM images ORDER BY seq")
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let mapped = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let mut out = Vec::new();
        for row in mapped {
            out.push(row.map_err(|e| ServerFnError::new(e.to_string()))?);
        }
        out
    };

    let query = normalize_tag_text(&tag_query);
    let star_val = star_val.min(5);
    Ok(rows
        .into_iter()
        .filter(|(_, rating, tags)| {
            let rating = (*rating).clamp(0, 5) as u8;
            let matched =
                star_match(rating, &star_op, star_val) && tags_match(tags, &tag_mode, &query);
            if complement {
                !matched
            } else {
                matched
            }
        })
        .map(|(path, _, _)| path)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{merge_tag_lists, star_match, tags_match};

    #[test]
    fn merge_appends_and_dedups() {
        assert_eq!(merge_tag_lists("aaa", "aaa"), "aaa");
        assert_eq!(merge_tag_lists("AAA", "aaa"), "AAA");
        assert_eq!(merge_tag_lists("aaa", "bbb"), "aaa, bbb");
        assert_eq!(merge_tag_lists("aaa, bbb", "bbb, ccc"), "aaa, bbb, ccc");
        assert_eq!(merge_tag_lists("aaa, aaa", "bbb"), "aaa, bbb");
        assert_eq!(merge_tag_lists("", "aaa"), "aaa");
        assert_eq!(merge_tag_lists("aaa", ""), "aaa");
    }

    #[test]
    fn contains_plain_tag() {
        assert!(tags_match("aaa", "contains", "aaa"));
        assert!(tags_match("foo, aaa, bar", "contains", "aaa"));
        assert!(tags_match("aaabbb", "contains", "aaa"));
    }

    #[test]
    fn contains_strips_utf16_nuls() {
        assert!(tags_match("a\0a\0a\0", "contains", "aaa"));
        assert!(tags_match("foo, a\0a\0a, bar", "contains", "aaa"));
    }

    #[test]
    fn contains_does_not_drop_matching() {
        assert!(!tags_match("bbb", "contains", "aaa"));
        assert!(tags_match("bbb,aaa", "excludes", "zzz"));
        assert!(!tags_match("aaa", "excludes", "aaa"));
    }

    #[test]
    fn star_any_ignores_value() {
        assert!(star_match(5, "any", 0));
        assert!(star_match(0, "eq", 0));
        assert!(!star_match(3, "eq", 0));
    }
}
