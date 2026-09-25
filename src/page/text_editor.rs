use crate::function::{read_text_file, write_text_file};
use crate::structure::ExplorerState;
use leptos::html;
use leptos::prelude::*;

const FONT_MIN: u32 = 12;
const FONT_MAX: u32 = 28;

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        1
    } else {
        text.matches('\n').count() + 1
    }
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
                <label class="orig-check" title="在左侧显示行号">
                    <input
                        type="checkbox"
                        prop:checked=move || state.editor_line_numbers.get()
                        on:change=move |ev| {
                            state.editor_line_numbers.set(event_target_checked(&ev));
                        }
                    />
                    "行号"
                </label>
            </div>
            <Suspense fallback=|| view! { <div class="editor-empty">"加载中…"</div> }>
                {move || match (load_error.get(), source.get()) {
                    (Some(err), _) => view! {
                        <div class="editor-empty editor-error">{err}</div>
                    }.into_any(),
                    (_, Some(Ok(_))) => view! {
                        <div class="editor-body">
                            <Show when=move || state.editor_line_numbers.get()>
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
                                style=font_style
                                spellcheck="false"
                                wrap="off"
                                prop:value=move || draft.get()
                                on:input=move |ev| draft.set(event_target_value(&ev))
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
