use crate::function::{read_text_file, write_text_file};
use crate::structure::ExplorerState;
use leptos::html;
use leptos::prelude::*;

const FONT_MIN: u32 = 12;
const FONT_MAX: u32 = 28;

#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
mod indent_edit {
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

    fn unindent_line(line: &str) -> (String, usize) {
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

    fn indent_lines(
        text: &str,
        start: usize,
        end: usize,
        unindent: bool,
    ) -> (String, usize, usize) {
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

    pub fn apply_tab(
        text: &str,
        start: usize,
        end: usize,
        unindent: bool,
    ) -> (String, usize, usize) {
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
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        1
    } else {
        text.matches('\n').count() + 1
    }
}

fn handle_editor_key(
    ev: &leptos::ev::KeyboardEvent,
    textarea_ref: NodeRef<html::Textarea>,
    draft: RwSignal<String>,
) {
    let key = ev.key();
    let is_tab = key == "Tab";
    let is_enter = key == "Enter";
    if !is_tab && !is_enter {
        return;
    }
    if ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
        return;
    }
    if ev.is_composing() || ev.key_code() == 229 {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (textarea_ref, draft);
        if is_tab {
            ev.prevent_default();
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let Some(el) = textarea_ref.get().or_else(|| {
            ev.target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        }) else {
            return;
        };
        ev.prevent_default();
        let start = el.selection_start().ok().flatten().unwrap_or(0);
        let end = el.selection_end().ok().flatten().unwrap_or(start);
        let text = el.value();
        if is_enter || (!ev.shift_key() && !indent_edit::utf16_range_has_newline(&text, start, end))
        {
            let insert = if is_enter {
                indent_edit::enter_insert(&text, start as usize)
            } else {
                indent_edit::UNIT.to_string()
            };
            let _ = el.set_range_text_with_start_and_end_and_mode(&insert, start, end, "end");
            let caret = el.selection_end().ok().flatten().unwrap_or(end);
            draft.set(el.value());
            restore_textarea_caret(&el, caret);
            return;
        }
        let sel_start = indent_edit::utf16_to_byte(&text, start as usize);
        let sel_end = indent_edit::utf16_to_byte(&text, end as usize);
        let (new_text, new_start, new_end) =
            indent_edit::apply_tab(&text, sel_start, sel_end, ev.shift_key());
        el.set_value(&new_text);
        let caret_start = indent_edit::byte_to_utf16(&new_text, new_start) as u32;
        let caret_end = indent_edit::byte_to_utf16(&new_text, new_end) as u32;
        draft.set(new_text);
        let _ = el.set_selection_range(caret_start, caret_end);
        restore_textarea_caret_range(&el, caret_start, caret_end);
    }
}

#[cfg(target_arch = "wasm32")]
fn restore_textarea_caret(el: &web_sys::HtmlTextAreaElement, caret: u32) {
    restore_textarea_caret_range(el, caret, caret);
}

#[cfg(target_arch = "wasm32")]
fn restore_textarea_caret_range(el: &web_sys::HtmlTextAreaElement, start: u32, end: u32) {
    let _ = el.set_selection_range(start, end);
    let el = el.clone();
    leptos::task::spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(0).await;
        let _ = el.set_selection_range(start, end);
    });
}

#[component]
pub fn TextEditor() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let draft = RwSignal::new(String::new());
    let saved = RwSignal::new(String::new());
    let saving = RwSignal::new(false);
    let load_error = RwSignal::new(None::<String>);
    let textarea_ref = NodeRef::<html::Textarea>::new();
    let gutter_ref = NodeRef::<html::Pre>::new();

    let text_path = Memo::new(move |_| state.selected.get().filter(|s| s.is_text).map(|s| s.path));
    let text_name = Memo::new(move |_| {
        state
            .selected
            .get()
            .filter(|s| s.is_text)
            .map(|s| s.name)
            .unwrap_or_default()
    });

    let source = Resource::new(
        move || text_path.get(),
        |path| async move {
            match path {
                Some(path) => read_text_file(path).await,
                None => Ok(String::new()),
            }
        },
    );

    Effect::new(move |_| match source.get() {
        Some(Ok(text)) => {
            load_error.set(None);
            saved.set(text.clone());
            draft.set(text);
        }
        Some(Err(err)) => {
            load_error.set(Some(err.to_string()));
            saved.set(String::new());
            draft.set(String::new());
        }
        None => {}
    });

    let dirty = move || draft.get() != saved.get();
    let can_write =
        move || text_path.get().is_some() && load_error.get().is_none() && !saving.get();

    let reset = move |_| {
        if !can_write() {
            return;
        }
        draft.set(saved.get());
        if let Some(el) = textarea_ref.get() {
            el.set_value(&saved.get());
        }
        state.status.set("已重置未保存的修改".into());
    };

    let save = move |_| {
        let Some(path) = text_path.get() else {
            return;
        };
        if !can_write() || !dirty() {
            return;
        }
        let content = draft.get();
        let name = text_name.get();
        saving.set(true);
        leptos::task::spawn_local(async move {
            match write_text_file(path, content.clone()).await {
                Ok(()) => {
                    saved.set(content);
                    state.status.set(format!("已保存：{name}"));
                }
                Err(err) => state.status.set(format!("保存失败：{err}")),
            }
            saving.set(false);
        });
    };

    let bump_font = move |delta: i32| {
        state.editor_font_size.update(|size| {
            let next = i32::try_from(*size).unwrap_or(15) + delta;
            *size = next.clamp(FONT_MIN as i32, FONT_MAX as i32) as u32;
        });
    };

    let font_style = move || format!("font-size:{}px", state.editor_font_size.get());

    let sync_gutter = move |_| {
        let Some(ta) = textarea_ref.get() else {
            return;
        };
        let Some(gutter) = gutter_ref.get() else {
            return;
        };
        gutter.set_scroll_top(ta.scroll_top());
    };

    view! {
        <section
            class="viewer text-editor"
            class:is-light=move || state.editor_light.get()
        >
            <div class="viewer-toolbar" class:panel-off=move || !state.show_adjust.get()>
                <button
                    class="btn"
                    title="丢弃未保存的修改"
                    disabled=move || !can_write() || !dirty()
                    on:click=reset
                >
                    "重置"
                </button>
                <button
                    class="btn"
                    title="保存到文件"
                    disabled=move || !can_write() || !dirty()
                    on:click=save
                >
                    {move || if saving.get() { "保存中…" } else { "保存" }}
                </button>
                <span class="viewer-name">{move || text_name.get()}</span>
            </div>
            <div
                class="viewer-toolbar editor-settings-bar"
                class:panel-off=move || !state.show_editor_settings.get()
            >
                <span class="mark-label">"字号"</span>
                <button
                    class="btn"
                    title="减小字号"
                    disabled=move || { state.editor_font_size.get() <= FONT_MIN }
                    on:click=move |_| bump_font(-1)
                >
                    "−"
                </button>
                <span class="zoom-label">{move || format!("{}px", state.editor_font_size.get())}</span>
                <button
                    class="btn"
                    title="增大字号"
                    disabled=move || { state.editor_font_size.get() >= FONT_MAX }
                    on:click=move |_| bump_font(1)
                >
                    "+"
                </button>
                <button
                    class="btn"
                    class:is-active=move || state.editor_light.get()
                    title="切换亮暗模式"
                    on:click=move |_| state.editor_light.update(|v| *v = !*v)
                >
                    {move || if state.editor_light.get() { "亮色" } else { "暗色" }}
                </button>
                <label class="orig-check" title="在左侧显示行号。自动换行开启时行号无法与折行对齐，会暂时隐藏">
                    <input
                        type="checkbox"
                        prop:checked=move || state.editor_line_numbers.get()
                        on:change=move |ev| {
                            state.editor_line_numbers.set(event_target_checked(&ev));
                        }
                    />
                    "行号"
                </label>
                <label class="orig-check" title="按编辑区宽度折行；长行不再左右滚动">
                    <input
                        type="checkbox"
                        prop:checked=move || state.editor_word_wrap.get()
                        on:change=move |ev| {
                            state.editor_word_wrap.set(event_target_checked(&ev));
                        }
                    />
                    "自动换行"
                </label>
            </div>
            <Suspense fallback=|| view! { <div class="editor-empty">"加载中…"</div> }>
                {move || match (load_error.get(), source.get()) {
                    (Some(err), _) => view! {
                        <div class="editor-empty editor-error">{err}</div>
                    }.into_any(),
                    (_, Some(Ok(_))) => view! {
                        <div class="editor-body">
                            <Show when=move || {
                                state.editor_line_numbers.get() && !state.editor_word_wrap.get()
                            }>
                                <pre
                                    node_ref=gutter_ref
                                    class="editor-gutter"
                                    style=font_style
                                    aria-hidden="true"
                                >
                                    {move || {
                                        let n = line_count(&draft.get());
                                        (1..=n)
                                            .map(|i| i.to_string())
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    }}
                                </pre>
                            </Show>
                            <textarea
                                node_ref=textarea_ref
                                class="editor-textarea"
                                class:is-wrap=move || state.editor_word_wrap.get()
                                style=font_style
                                spellcheck="false"
                                wrap=move || {
                                    if state.editor_word_wrap.get() {
                                        "soft"
                                    } else {
                                        "off"
                                    }
                                }
                                prop:value=move || draft.get()
                                on:input=move |ev| draft.set(event_target_value(&ev))
                                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                    handle_editor_key(&ev, textarea_ref, draft);
                                }
                                on:scroll=sync_gutter
                            />
                        </div>
                    }.into_any(),
                    _ => view! { <div class="editor-empty">"加载中…"</div> }.into_any(),
                }}
            </Suspense>
        </section>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::indent_edit::{
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
