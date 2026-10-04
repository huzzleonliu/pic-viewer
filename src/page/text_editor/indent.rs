#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub const UNIT: &str = "\t";

fn leading_ws(s: &str) -> &str {
    let n = s.bytes().take_while(|&b| b == b' ' || b == b'\t').count();
    &s[..n]
}

fn floor_boundary(text: &str, mut i: usize) -> usize {
    i = i.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn clamp_range(text: &str, start: usize, end: usize) -> (usize, usize) {
    let start = floor_boundary(text, start);
    let end = floor_boundary(text, end);
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

fn enter_insert_at_byte(text: &str, start: usize) -> String {
    let start = floor_boundary(text, start);
    let line_begin = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let prefix = &text[line_begin..start];
    let pad = leading_ws(prefix);
    let extra = if prefix.trim_end().ends_with(['{', '[', '(', ':']) {
        UNIT
    } else {
        ""
    };
    format!("\n{pad}{extra}")
}

pub fn enter_insert(text: &str, utf16_start: usize) -> String {
    enter_insert_at_byte(text, utf16_to_byte(text, utf16_start))
}

#[cfg(test)]
pub fn apply_enter(text: &str, start: usize, end: usize) -> (String, usize) {
    let (start, end) = clamp_range(text, start, end);
    let insert = enter_insert_at_byte(text, start);
    let mut out = String::with_capacity(text.len() - (end - start) + insert.len());
    out.push_str(&text[..start]);
    out.push_str(&insert);
    out.push_str(&text[end..]);
    let caret = start + insert.len();
    (out, caret)
}

pub(super) fn unindent_line(line: &str) -> (String, usize) {
    if let Some(rest) = line.strip_prefix('\t') {
        return (rest.to_string(), 1);
    }
    let spaces = line.bytes().take_while(|&b| b == b' ').count().min(4);
    if spaces == 0 {
        (line.to_string(), 0)
    } else {
        (line[spaces..].to_string(), spaces)
    }
}

fn map_pos(p: usize, edits: &[(usize, i32)]) -> usize {
    let mut delta = 0i32;
    for &(at, d) in edits {
        if d > 0 {
            if p > at {
                delta += d;
            }
        } else if d < 0 {
            let n = (-d) as usize;
            if p >= at + n {
                delta += d;
            } else if p > at {
                delta += at as i32 - p as i32;
            }
        }
    }
    (p as i32 + delta).max(0) as usize
}

fn indent_lines(text: &str, start: usize, end: usize, unindent: bool) -> (String, usize, usize) {
    let (start, end) = clamp_range(text, start, end);
    let from = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let last = if end > start && text.as_bytes()[end - 1] == b'\n' {
        end - 1
    } else {
        end
    };
    let to = text[last..]
        .find('\n')
        .map(|i| last + i)
        .unwrap_or(text.len());
    let block = &text[from..to];
    let mut new_block = String::new();
    let mut edits = Vec::new();
    let mut rel = 0usize;
    for (i, line) in block.split('\n').enumerate() {
        if i > 0 {
            new_block.push('\n');
            rel += 1;
        }
        let abs = from + rel;
        if unindent {
            let (next, removed) = unindent_line(line);
            new_block.push_str(&next);
            if removed > 0 {
                edits.push((abs, -(removed as i32)));
            }
        } else {
            new_block.push_str(UNIT);
            new_block.push_str(line);
            edits.push((abs, UNIT.len() as i32));
        }
        rel += line.len();
    }
    let mut out = String::with_capacity(text.len() + new_block.len());
    out.push_str(&text[..from]);
    out.push_str(&new_block);
    out.push_str(&text[to..]);
    (out, map_pos(start, &edits), map_pos(end, &edits))
}

pub fn apply_tab(text: &str, start: usize, end: usize, unindent: bool) -> (String, usize, usize) {
    let (start, end) = clamp_range(text, start, end);
    if !unindent && !text[start..end].contains('\n') {
        let mut out = String::with_capacity(text.len() + UNIT.len());
        out.push_str(&text[..start]);
        out.push_str(UNIT);
        out.push_str(&text[end..]);
        let caret = start + UNIT.len();
        return (out, caret, caret);
    }
    indent_lines(text, start, end, unindent)
}

pub fn utf16_to_byte(s: &str, utf16_pos: usize) -> usize {
    let mut seen = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if seen >= utf16_pos {
            return byte_idx;
        }
        seen += ch.len_utf16();
    }
    s.len()
}

pub fn byte_to_utf16(s: &str, byte_pos: usize) -> usize {
    let mut i = byte_pos.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    s[..i].chars().map(|c| c.len_utf16()).sum()
}

pub fn utf16_range_has_newline(text: &str, start: u32, end: u32) -> bool {
    let a = utf16_to_byte(text, start as usize);
    let b = utf16_to_byte(text, end as usize);
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    text.get(a..b).is_some_and(|s| s.contains('\n'))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_enter, apply_tab, byte_to_utf16, enter_insert, utf16_range_has_newline, utf16_to_byte,
    };

    fn replace_at_utf16(text: &str, start: usize, end: usize, insert: &str) -> String {
        let a = utf16_to_byte(text, start);
        let b = utf16_to_byte(text, end);
        format!("{}{}{}", &text[..a], insert, &text[b..])
    }

    #[test]
    fn utf16_offsets_match_cjk() {
        let s = "备注：test";
        assert_eq!(utf16_to_byte(s, 0), 0);
        assert_eq!(utf16_to_byte(s, 3), "备注：".len());
        assert_eq!(utf16_to_byte(s, 7), s.len());
        assert_eq!(byte_to_utf16(s, "备注：".len()), 3);
        assert_eq!(byte_to_utf16(s, s.len()), 7);
        let (out, caret) = apply_enter(s, utf16_to_byte(s, 3), utf16_to_byte(s, 3));
        assert_eq!(out, "备注：\ntest");
        assert_eq!(caret, "备注：\n".len());
        assert_eq!(byte_to_utf16(&out, caret), 4);
    }

    #[test]
    fn utf16_offsets_match_emoji() {
        let s = "😀a";
        assert_eq!(utf16_to_byte(s, 2), "😀".len());
        assert_eq!(byte_to_utf16(s, "😀".len()), 2);
    }

    #[test]
    fn treating_utf16_as_bytes_inserts_two_lines_too_early() {
        let text = "一二三四五六七八九十\n甲乙丙丁戊己庚辛壬癸\n光标在这里";
        let caret_byte = text.find('在').expect("caret char");
        let caret_u16 = byte_to_utf16(text, caret_byte);
        assert_eq!(text[..caret_byte].matches('\n').count(), 2);
        assert_eq!(text[..caret_u16].matches('\n').count(), 0);
        let (wrong, _) = apply_enter(text, caret_u16, caret_u16);
        assert!(
            wrong.contains("一二三四五六七八\n九十"),
            "old indexing splits the first line: {wrong:?}"
        );
        assert!(
            !wrong.contains("光标\n在这里"),
            "old indexing must not split at the caret: {wrong:?}"
        );
    }

    #[test]
    fn enter_at_utf16_caret_splits_cjk_line_not_two_lines_up() {
        let text = "一二三四五六七八九十\n甲乙丙丁戊己庚辛壬癸\n光标在这里";
        let caret_byte = text.find('在').expect("caret char");
        let caret_u16 = byte_to_utf16(text, caret_byte);
        let insert = enter_insert(text, caret_u16);
        assert_eq!(insert, "\n");
        let out = replace_at_utf16(text, caret_u16, caret_u16, &insert);
        assert!(
            out.contains("光标\n在这里"),
            "newline should land at the caret: {out:?}"
        );
        assert!(
            !out.contains("一二三四五六七八\n九十"),
            "must not split two lines above: {out:?}"
        );
        assert_eq!(out.matches('\n').count(), 3);
    }

    #[test]
    fn enter_insert_keeps_indent_of_caret_line() {
        let text = "fn main() {\n    中文\n    光标";
        let caret_u16 = byte_to_utf16(text, text.len());
        assert_eq!(enter_insert(text, caret_u16), "\n    ");
        assert!(utf16_range_has_newline(text, 0, caret_u16 as u32));
        assert!(!utf16_range_has_newline("abc", 0, 3));
    }

    #[test]
    fn enter_copies_indent() {
        let (out, caret) = apply_enter("  abc", 5, 5);
        assert_eq!(out, "  abc\n  ");
        assert_eq!(caret, 8);
    }

    #[test]
    fn enter_extra_after_brace() {
        let (out, caret) = apply_enter("  foo {", 7, 7);
        assert_eq!(out, "  foo {\n  \t");
        assert_eq!(caret, 11);
    }

    #[test]
    fn enter_replaces_selection() {
        let (out, caret) = apply_enter("ab", 0, 2);
        assert_eq!(out, "\n");
        assert_eq!(caret, 1);
    }

    #[test]
    fn tab_inserts_at_caret() {
        let (out, a, b) = apply_tab("ab", 1, 1, false);
        assert_eq!(out, "a\tb");
        assert_eq!((a, b), (2, 2));
    }

    #[test]
    fn tab_indents_block() {
        let (out, a, b) = apply_tab("a\nb", 0, 3, false);
        assert_eq!(out, "\ta\n\tb");
        assert_eq!((a, b), (0, 5));
    }

    #[test]
    fn shift_tab_unindents_tab_and_spaces() {
        let (out, ..) = apply_tab("\ta\n    b", 0, 8, true);
        assert_eq!(out, "a\nb");
    }
}
