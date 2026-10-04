#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
mod cursors;
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
mod indent;

use crate::function::{read_text_file, write_text_file};
use crate::structure::ExplorerState;
use leptos::html;
use leptos::prelude::*;

const FONT_MIN: u32 = 12;
const FONT_MAX: u32 = 28;

#[derive(Clone, Copy, PartialEq)]
struct CaretMark {
    offset: u32,
    left: f64,
    top: f64,
    height: f64,
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
    cursors: RwSignal<Vec<u32>>,
) {
    let key = ev.key();
    if ev.is_composing() || ev.key_code() == 229 {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (textarea_ref, draft, cursors);
        if key == "Tab" && !ev.alt_key() {
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

        if key == "Escape" {
            let cs = cursors.get_untracked();
            if cs.len() > 1 {
                ev.prevent_default();
                let top = cursors::topmost(&cs).unwrap_or(0);
                cursors.set(Vec::new());
                restore_textarea_caret(&el, top);
            }
            return;
        }

        let multi = cursors.with_untracked(|cs| cs.len() > 1);
        if multi {
            handle_multi_key(ev, &el, draft, cursors);
            return;
        }

        let is_tab = key == "Tab";
        let is_enter = key == "Enter";
        if !is_tab && !is_enter {
            return;
        }
        if ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
            return;
        }
        ev.prevent_default();
        let start = el.selection_start().ok().flatten().unwrap_or(0);
        let end = el.selection_end().ok().flatten().unwrap_or(start);
        let text = el.value();
        if is_enter || (!ev.shift_key() && !indent::utf16_range_has_newline(&text, start, end)) {
            let insert = if is_enter {
                indent::enter_insert(&text, start as usize)
            } else {
                indent::UNIT.to_string()
            };
            let _ = el.set_range_text_with_start_and_end_and_mode(&insert, start, end, "end");
            let caret = el.selection_end().ok().flatten().unwrap_or(end);
            draft.set(el.value());
            restore_textarea_caret(&el, caret);
            return;
        }
        let sel_start = indent::utf16_to_byte(&text, start as usize);
        let sel_end = indent::utf16_to_byte(&text, end as usize);
        let (new_text, new_start, new_end) =
            indent::apply_tab(&text, sel_start, sel_end, ev.shift_key());
        el.set_value(&new_text);
        let caret_start = indent::byte_to_utf16(&new_text, new_start) as u32;
        let caret_end = indent::byte_to_utf16(&new_text, new_end) as u32;
        draft.set(new_text);
        restore_textarea_caret_range(&el, caret_start, caret_end);
    }
}

#[cfg(target_arch = "wasm32")]
fn handle_multi_key(
    ev: &leptos::ev::KeyboardEvent,
    el: &web_sys::HtmlTextAreaElement,
    draft: RwSignal<String>,
    cursors: RwSignal<Vec<u32>>,
) {
    if ev.ctrl_key() || ev.meta_key() {
        return;
    }
    let key = ev.key();
    let carets = cursors.get_untracked();
    let text = el.value();
    let edited = if key == "Backspace" {
        Some(cursors::delete_backward(&text, &carets))
    } else if key == "Delete" {
        Some(cursors::delete_forward(&text, &carets))
    } else if key == "Enter" {
        Some(cursors::map_insert(&text, &carets, |t, c| {
            indent::enter_insert(t, c as usize)
        }))
    } else if key == "Tab" {
        if ev.shift_key() {
            Some(cursors::unindent_caret_lines(&text, &carets))
        } else {
            Some(cursors::insert_same(&text, &carets, indent::UNIT))
        }
    } else if key == "ArrowLeft" {
        ev.prevent_default();
        commit_carets_only(el, cursors, cursors::move_left(&text, &carets));
        return;
    } else if key == "ArrowRight" {
        ev.prevent_default();
        commit_carets_only(el, cursors, cursors::move_right(&text, &carets));
        return;
    } else if key == "ArrowUp" {
        ev.prevent_default();
        commit_carets_only(el, cursors, cursors::move_vertical(&text, &carets, true));
        return;
    } else if key == "ArrowDown" {
        ev.prevent_default();
        commit_carets_only(el, cursors, cursors::move_vertical(&text, &carets, false));
        return;
    } else if key == "Home" {
        ev.prevent_default();
        commit_carets_only(el, cursors, cursors::move_line_edge(&text, &carets, false));
        return;
    } else if key == "End" {
        ev.prevent_default();
        commit_carets_only(el, cursors, cursors::move_line_edge(&text, &carets, true));
        return;
    } else if is_printable_key(&key) && !ev.alt_key() {
        Some(cursors::insert_same(&text, &carets, &key))
    } else {
        None
    };
    let Some((new_text, new_carets)) = edited else {
        return;
    };
    ev.prevent_default();
    commit_multi(el, draft, cursors, new_text, new_carets);
}

#[cfg(target_arch = "wasm32")]
fn is_printable_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if !c.is_control()) && chars.next().is_none()
}

#[cfg(target_arch = "wasm32")]
fn commit_carets_only(
    el: &web_sys::HtmlTextAreaElement,
    cursors: RwSignal<Vec<u32>>,
    new_carets: Vec<u32>,
) {
    let primary = new_carets.last().copied().unwrap_or(0);
    if new_carets.len() <= 1 {
        cursors.set(Vec::new());
    } else {
        cursors.set(new_carets);
    }
    restore_textarea_caret(el, primary);
}

#[cfg(target_arch = "wasm32")]
fn commit_multi(
    el: &web_sys::HtmlTextAreaElement,
    draft: RwSignal<String>,
    cursors: RwSignal<Vec<u32>>,
    new_text: String,
    new_carets: Vec<u32>,
) {
    el.set_value(&new_text);
    draft.set(new_text);
    let primary = new_carets.last().copied().unwrap_or(0);
    if new_carets.len() <= 1 {
        cursors.set(Vec::new());
    } else {
        cursors.set(new_carets);
    }
    restore_textarea_caret(el, primary);
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

#[cfg(target_arch = "wasm32")]
const MIRROR_STYLES: &[&str] = &[
    "direction",
    "box-sizing",
    "width",
    "height",
    "overflow-x",
    "overflow-y",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "border-style",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "font-style",
    "font-variant",
    "font-weight",
    "font-stretch",
    "font-size",
    "line-height",
    "font-family",
    "text-align",
    "text-transform",
    "text-indent",
    "letter-spacing",
    "word-spacing",
    "tab-size",
    "white-space",
    "word-wrap",
    "overflow-wrap",
];

#[cfg(target_arch = "wasm32")]
fn measure_carets(el: &web_sys::HtmlTextAreaElement, carets: &[u32]) -> Vec<CaretMark> {
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else {
        return Vec::new();
    };
    let Ok(computed) = window.get_computed_style(el) else {
        return Vec::new();
    };
    let Some(computed) = computed else {
        return Vec::new();
    };
    let Some(document) = window.document() else {
        return Vec::new();
    };
    let Ok(div) = document.create_element("div") else {
        return Vec::new();
    };
    let Ok(div) = div.dyn_into::<web_sys::HtmlElement>() else {
        return Vec::new();
    };
    let style = div.style();
    let _ = style.set_property("position", "absolute");
    let _ = style.set_property("visibility", "hidden");
    let _ = style.set_property("white-space", "pre");
    for prop in MIRROR_STYLES {
        if let Ok(val) = computed.get_property_value(prop) {
            let _ = style.set_property(prop, &val);
        }
    }
    let _ = style.set_property("overflow", "hidden");
    let _ = style.set_property("top", "0");
    let _ = style.set_property("left", "0");
    let body = match document.body() {
        Some(body) => body,
        None => return Vec::new(),
    };
    if body.append_child(&div).is_err() {
        return Vec::new();
    }
    let value = el.value();
    let scroll_top = f64::from(el.scroll_top());
    let scroll_left = f64::from(el.scroll_left());
    let mut marks = Vec::with_capacity(carets.len());
    for &offset in carets {
        let split = indent::utf16_to_byte(&value, offset as usize);
        div.set_text_content(Some(&value[..split]));
        let Ok(span) = document.create_element("span") else {
            continue;
        };
        span.set_text_content(Some("\u{200b}"));
        if div.append_child(&span).is_err() {
            continue;
        }
        let Ok(span) = span.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        marks.push(CaretMark {
            offset,
            left: f64::from(span.offset_left()) - scroll_left,
            top: f64::from(span.offset_top()) - scroll_top,
            height: f64::from(span.offset_height()).max(1.0),
        });
    }
    div.remove();
    marks
}

#[component]
pub fn TextEditor() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let draft = RwSignal::new(String::new());
    let saved = RwSignal::new(String::new());
    let saving = RwSignal::new(false);
    let load_error = RwSignal::new(None::<String>);
    let cursors = RwSignal::new(Vec::<u32>::new());
    let caret_marks = RwSignal::new(Vec::<CaretMark>::new());
    let alt_anchor = StoredValue::new(Vec::<u32>::new());
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
            cursors.set(Vec::new());
        }
        Some(Err(err)) => {
            load_error.set(Some(err.to_string()));
            saved.set(String::new());
            draft.set(String::new());
            cursors.set(Vec::new());
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
        cursors.set(Vec::new());
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

    let refresh_overlay = move || {
        let Some(el) = textarea_ref.get() else {
            caret_marks.set(Vec::new());
            return;
        };
        let cs = cursors.get_untracked();
        if cs.len() <= 1 {
            caret_marks.set(Vec::new());
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            caret_marks.set(measure_carets(&el, &cs));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = el;
            caret_marks.set(Vec::new());
        }
    };

    Effect::new(move |_| {
        let _ = (
            cursors.get(),
            draft.get(),
            state.editor_font_size.get(),
            state.editor_word_wrap.get(),
        );
        refresh_overlay();
    });

    let sync_scroll = move |_| {
        if let (Some(ta), Some(gutter)) = (textarea_ref.get(), gutter_ref.get()) {
            gutter.set_scroll_top(ta.scroll_top());
        }
        refresh_overlay();
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
                <span class="muted editor-hint" title="在编辑区按住 Alt 再点击可添加光标，Esc 回到最上方那个">
                    "Alt+点击 多光标"
                </span>
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
                            <div class="editor-input-wrap">
                                <textarea
                                    node_ref=textarea_ref
                                    class="editor-textarea"
                                    class:is-wrap=move || state.editor_word_wrap.get()
                                    class:has-multi-caret=move || cursors.with(|cs| cs.len() > 1)
                                    style=font_style
                                    spellcheck="false"
                                    title="Alt+点击添加光标，Esc 回到最上光标"
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
                                        handle_editor_key(&ev, textarea_ref, draft, cursors);
                                    }
                                    on:mousedown=move |ev: leptos::ev::MouseEvent| {
                                        if ev.button() != 0 {
                                            return;
                                        }
                                        let Some(el) = textarea_ref.get() else {
                                            return;
                                        };
                                        if ev.alt_key() {
                                            let mut cs = cursors.get_untracked();
                                            if cs.is_empty() {
                                                cs.push(
                                                    el.selection_start()
                                                        .ok()
                                                        .flatten()
                                                        .unwrap_or(0),
                                                );
                                            }
                                            alt_anchor.set_value(cs);
                                        } else {
                                            cursors.set(Vec::new());
                                            alt_anchor.set_value(Vec::new());
                                        }
                                    }
                                    on:click=move |ev: leptos::ev::MouseEvent| {
                                        if ev.button() != 0 || !ev.alt_key() {
                                            return;
                                        }
                                        let Some(el) = textarea_ref.get() else {
                                            return;
                                        };
                                        let pos = el.selection_start().ok().flatten().unwrap_or(0);
                                        let mut cs = alt_anchor.get_value();
                                        if cs.is_empty() {
                                            cs.push(pos);
                                        }
                                        cursors::add_caret(&mut cs, pos);
                                        if cs.len() > 1 {
                                            cursors.set(cs);
                                            let _ = el.set_selection_range(pos, pos);
                                        }
                                    }
                                    on:paste=move |ev: leptos::ev::ClipboardEvent| {
                                        if cursors.with_untracked(|cs| cs.len() <= 1) {
                                            return;
                                        }
                                        #[cfg(target_arch = "wasm32")]
                                        {
                                            let Some(el) = textarea_ref.get() else {
                                                return;
                                            };
                                            let Some(data) = ev.clipboard_data() else {
                                                return;
                                            };
                                            let Ok(paste) = data.get_data("text/plain") else {
                                                return;
                                            };
                                            if paste.is_empty() {
                                                return;
                                            }
                                            ev.prevent_default();
                                            let (new_text, new_carets) = cursors::insert_same(
                                                &el.value(),
                                                &cursors.get_untracked(),
                                                &paste,
                                            );
                                            commit_multi(&el, draft, cursors, new_text, new_carets);
                                        }
                                        #[cfg(not(target_arch = "wasm32"))]
                                        {
                                            let _ = ev;
                                        }
                                    }
                                    on:scroll=sync_scroll
                                />
                                <div class="editor-caret-layer" aria-hidden="true">
                                    <For
                                        each=move || caret_marks.get()
                                        key=|m| m.offset
                                        children=move |m| {
                                            view! {
                                                <span
                                                    class="editor-caret"
                                                    style=format!(
                                                        "left:{}px;top:{}px;height:{}px",
                                                        m.left, m.top, m.height
                                                    )
                                                />
                                            }
                                        }
                                    />
                                </div>
                            </div>
                        </div>
                    }.into_any(),
                    _ => view! { <div class="editor-empty">"加载中…"</div> }.into_any(),
                }}
            </Suspense>
        </section>
    }
    .into_any()
}
