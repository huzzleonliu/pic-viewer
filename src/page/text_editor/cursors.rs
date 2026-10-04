use super::indent::{byte_to_utf16, unindent_line, utf16_to_byte};

pub fn normalize(carets: &mut Vec<u32>) {
    carets.sort_unstable();
    carets.dedup();
}

pub fn add_caret(carets: &mut Vec<u32>, pos: u32) {
    carets.push(pos);
    normalize(carets);
}

pub fn topmost(carets: &[u32]) -> Option<u32> {
    carets.iter().copied().min()
}

fn utf16_len(s: &str) -> usize {
    s.chars().map(|c| c.len_utf16()).sum()
}

pub fn insert_same(text: &str, carets: &[u32], insert: &str) -> (String, Vec<u32>) {
    map_insert(text, carets, |_, _| insert.to_string())
}

pub fn map_insert<F>(text: &str, carets: &[u32], mut insert_at: F) -> (String, Vec<u32>)
where
    F: FnMut(&str, u32) -> String,
{
    let mut unique = carets.to_vec();
    normalize(&mut unique);
    if unique.is_empty() {
        return (text.to_string(), unique);
    }
    let mut out = String::new();
    let mut last_byte = 0usize;
    let mut new_carets = Vec::with_capacity(unique.len());
    let mut delta_u16 = 0u32;
    for &c in &unique {
        let byte = utf16_to_byte(text, c as usize);
        out.push_str(&text[last_byte..byte]);
        let ins = insert_at(text, c);
        let ins_u16 = utf16_len(&ins) as u32;
        out.push_str(&ins);
        last_byte = byte;
        new_carets.push(c + delta_u16 + ins_u16);
        delta_u16 += ins_u16;
    }
    out.push_str(&text[last_byte..]);
    (out, new_carets)
}

fn prev_char_start(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    if byte == 0 {
        return 0;
    }
    let mut i = byte - 1;
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_end(text: &str, byte: usize) -> usize {
    let mut i = byte.min(text.len());
    while i < text.len() && !text.is_char_boundary(i) {
        i += 1;
    }
    if i >= text.len() {
        return text.len();
    }
    i + text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(0)
}

fn merge_ranges(ranges: &mut Vec<(usize, usize)>) {
    if ranges.len() < 2 {
        return;
    }
    ranges.sort_unstable();
    let mut merged = Vec::with_capacity(ranges.len());
    let (mut a, mut b) = ranges[0];
    for &(c, d) in &ranges[1..] {
        if c <= b {
            b = b.max(d);
        } else {
            merged.push((a, b));
            a = c;
            b = d;
        }
    }
    merged.push((a, b));
    *ranges = merged;
}

fn apply_deletes(text: &str, carets: &[u32], ranges: &[(usize, usize)]) -> (String, Vec<u32>) {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for &(a, b) in ranges {
        out.push_str(&text[last..a]);
        last = b;
    }
    out.push_str(&text[last..]);

    let mut unique = carets.to_vec();
    normalize(&mut unique);
    let mut new_carets = Vec::with_capacity(unique.len());
    for &c in &unique {
        let byte = utf16_to_byte(text, c as usize);
        let mut shift = 0usize;
        for &(a, b) in ranges {
            if b <= byte {
                shift += b - a;
            } else if a < byte {
                shift += byte - a;
                break;
            } else {
                break;
            }
        }
        new_carets.push(byte_to_utf16(&out, byte.saturating_sub(shift)) as u32);
    }
    normalize(&mut new_carets);
    (out, new_carets)
}

pub fn delete_backward(text: &str, carets: &[u32]) -> (String, Vec<u32>) {
    let mut unique = carets.to_vec();
    normalize(&mut unique);
    let mut ranges = Vec::new();
    for &c in &unique {
        let end = utf16_to_byte(text, c as usize);
        let start = prev_char_start(text, end);
        if start < end {
            ranges.push((start, end));
        }
    }
    merge_ranges(&mut ranges);
    apply_deletes(text, &unique, &ranges)
}

pub fn delete_forward(text: &str, carets: &[u32]) -> (String, Vec<u32>) {
    let mut unique = carets.to_vec();
    normalize(&mut unique);
    let mut ranges = Vec::new();
    for &c in &unique {
        let start = utf16_to_byte(text, c as usize);
        let end = next_char_end(text, start);
        if start < end {
            ranges.push((start, end));
        }
    }
    merge_ranges(&mut ranges);
    apply_deletes(text, &unique, &ranges)
}

pub fn move_left(text: &str, carets: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = carets
        .iter()
        .map(|&c| {
            let byte = utf16_to_byte(text, c as usize);
            byte_to_utf16(text, prev_char_start(text, byte)) as u32
        })
        .collect();
    normalize(&mut out);
    out
}

pub fn move_right(text: &str, carets: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = carets
        .iter()
        .map(|&c| {
            let byte = utf16_to_byte(text, c as usize);
            byte_to_utf16(text, next_char_end(text, byte)) as u32
        })
        .collect();
    normalize(&mut out);
    out
}

fn line_start_byte(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn line_end_byte(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    text[byte..]
        .find('\n')
        .map(|i| byte + i)
        .unwrap_or(text.len())
}

fn place_col(text: &str, line_start: usize, line_end: usize, col: usize) -> usize {
    line_start + utf16_to_byte(&text[line_start..line_end], col)
}

pub fn move_vertical(text: &str, carets: &[u32], up: bool) -> Vec<u32> {
    let mut out = Vec::with_capacity(carets.len());
    for &c in carets {
        let byte = utf16_to_byte(text, c as usize);
        let start = line_start_byte(text, byte);
        let col = byte_to_utf16(text, byte) - byte_to_utf16(text, start);
        let dest = if up {
            if start == 0 {
                byte
            } else {
                let prev_end = start - 1;
                let prev_start = line_start_byte(text, prev_end);
                place_col(text, prev_start, prev_end, col)
            }
        } else {
            let end = line_end_byte(text, byte);
            if end >= text.len() {
                byte
            } else {
                let next_start = end + 1;
                let next_end = line_end_byte(text, next_start);
                place_col(text, next_start, next_end, col)
            }
        };
        out.push(byte_to_utf16(text, dest) as u32);
    }
    normalize(&mut out);
    out
}

pub fn move_line_edge(text: &str, carets: &[u32], to_end: bool) -> Vec<u32> {
    let mut out = Vec::with_capacity(carets.len());
    for &c in carets {
        let byte = utf16_to_byte(text, c as usize);
        let dest = if to_end {
            line_end_byte(text, byte)
        } else {
            line_start_byte(text, byte)
        };
        out.push(byte_to_utf16(text, dest) as u32);
    }
    normalize(&mut out);
    out
}

pub fn unindent_caret_lines(text: &str, carets: &[u32]) -> (String, Vec<u32>) {
    let mut unique = carets.to_vec();
    normalize(&mut unique);
    let mut begins: Vec<usize> = unique
        .iter()
        .map(|&c| line_start_byte(text, utf16_to_byte(text, c as usize)))
        .collect();
    begins.sort_unstable();
    begins.dedup();

    let mut edits: Vec<(usize, usize)> = Vec::new();
    for &begin in &begins {
        let end = line_end_byte(text, begin);
        let (_, removed) = unindent_line(&text[begin..end]);
        if removed > 0 {
            edits.push((begin, removed));
        }
    }

    let mut out = text.to_string();
    for &(at, n) in edits.iter().rev() {
        out.replace_range(at..at + n, "");
    }

    let mut new_carets = Vec::with_capacity(unique.len());
    for &c in &unique {
        let orig = utf16_to_byte(text, c as usize);
        let mut shift = 0usize;
        for &(at, n) in &edits {
            if orig >= at + n {
                shift += n;
            } else if orig > at {
                shift += orig - at;
            }
        }
        new_carets.push(byte_to_utf16(&out, orig - shift) as u32);
    }
    normalize(&mut new_carets);
    (out, new_carets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::text_editor::indent::enter_insert;

    #[test]
    fn add_caret_sorts_and_dedups() {
        let mut cs = vec![8, 2];
        add_caret(&mut cs, 2);
        add_caret(&mut cs, 5);
        assert_eq!(cs, vec![2, 5, 8]);
        assert_eq!(topmost(&cs), Some(2));
    }

    #[test]
    fn insert_same_at_ascii_carets() {
        let (out, carets) = insert_same("ace", &[1, 2], "X");
        assert_eq!(out, "aXcXe");
        assert_eq!(carets, vec![2, 4]);
    }

    #[test]
    fn insert_same_at_cjk_utf16_carets() {
        let text = "甲乙丙";
        let (out, carets) = insert_same(text, &[1, 2], "X");
        assert_eq!(out, "甲X乙X丙");
        assert_eq!(carets, vec![2, 4]);
    }

    #[test]
    fn insert_enter_uses_each_line_indent() {
        let text = "  aa\n    bb";
        let end = utf16_len(text) as u32;
        let (out, carets) = map_insert(text, &[4, end], |t, c| enter_insert(t, c as usize));
        assert_eq!(out, "  aa\n  \n    bb\n    ");
        assert_eq!(carets, vec![7, utf16_len(&out) as u32]);
    }

    #[test]
    fn backspace_at_two_carets() {
        let (out, carets) = delete_backward("abcde", &[3, 5]);
        assert_eq!(out, "abd");
        assert_eq!(carets, vec![2, 3]);
    }

    #[test]
    fn backspace_cjk_deletes_whole_char() {
        let (out, carets) = delete_backward("甲乙丙", &[2]);
        assert_eq!(out, "甲丙");
        assert_eq!(carets, vec![1]);
    }

    #[test]
    fn delete_forward_at_two_carets() {
        let (out, carets) = delete_forward("abcde", &[1, 3]);
        assert_eq!(out, "ace");
        assert_eq!(carets, vec![1, 2]);
    }

    #[test]
    fn overlapping_backspace_merges() {
        let (out, carets) = delete_backward("ab", &[1, 1]);
        assert_eq!(out, "b");
        assert_eq!(carets, vec![0]);
    }

    #[test]
    fn move_left_right_and_edges() {
        let text = "ab\ncd";
        assert_eq!(move_left(text, &[1, 4]), vec![0, 3]);
        assert_eq!(move_right(text, &[0, 3]), vec![1, 4]);
        assert_eq!(move_line_edge(text, &[1, 4], false), vec![0, 3]);
        assert_eq!(move_line_edge(text, &[0, 3], true), vec![2, 5]);
    }

    #[test]
    fn move_vertical_keeps_column() {
        let text = "ab\ncd\nef";
        assert_eq!(move_vertical(text, &[1], false), vec![4]);
        assert_eq!(move_vertical(text, &[4], true), vec![1]);
        assert_eq!(move_vertical(text, &[2], false), vec![5]);
        assert_eq!(move_vertical(text, &[0], true), vec![0]);
    }

    #[test]
    fn unindent_each_caret_line_once() {
        let (out, carets) = unindent_caret_lines("\ta\n\tb\n\tc", &[1, 4]);
        assert_eq!(out, "a\nb\n\tc");
        assert_eq!(carets, vec![0, 2]);
    }

    #[test]
    fn escape_keeps_topmost() {
        assert_eq!(topmost(&[9, 2, 5]), Some(2));
    }
}
