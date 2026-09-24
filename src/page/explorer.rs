use crate::function::get_root_info;
use crate::page::copy_text::copy_plain_text;
use crate::page::file_tree::FilePane;
use crate::page::image_viewer::ImageViewer;
use crate::page::text_editor::TextEditor;
use crate::structure::ExplorerState;
use leptos::prelude::*;

#[component]
pub fn Explorer() -> impl IntoView {
    let state = ExplorerState::new();
    provide_context(state);

    view! {
        <div class="explorer">
            <header class="app-header">
                <div class="brand">
                    <span class="brand-mark">"▣"</span>
                    <span>"Pic Viewer"</span>
                </div>
                <RootLabel/>
                <div class="header-toggles">
                    <span class="header-mode">
                        {move || if state.is_text_mode() { "文本" } else { "图片" }}
                    </span>
                    <span class="header-sep">" | "</span>
                    <Show when=move || !state.is_text_mode()>
                        <span class="header-toggle-group">
                            <PanelToggle label="缩略图" on=state.show_thumbnails/>
                            <PanelToggle label="标记" on=state.show_stars/>
                            <PanelToggle label="筛选" on=state.show_filter/>
                            <PanelToggle label="导出" on=state.show_export/>
                            <PanelToggle label="简单调整" on=state.show_adjust/>
                        </span>
                    </Show>
                    <Show when=move || state.is_text_mode()>
                        <span class="header-toggle-group">
                            <PanelToggle label="简单调整" on=state.show_adjust/>
                            <PanelToggle label="界面设置" on=state.show_editor_settings/>
                        </span>
                    </Show>
                    <span class="header-sep">" | "</span>
                    <PanelToggle label="文件管理器" on=state.show_file_manager/>
                </div>
            </header>
            <div class="workspace">
                <FilePane/>
                <ImageViewer/>
                <Show when=move || state.is_text_mode()>
                    <TextEditor/>
                </Show>
            </div>
            <footer class="status-bar">
                <span class="status-text">{move || state.status.get()}</span>
                <span class="status-sel">
                    {move || {
                        let current = state
                            .selected
                            .get()
                            .map(|s| {
                                if s.path.is_empty() {
                                    "当前：/".into()
                                } else {
                                    format!("当前：/{}", s.path)
                                }
                            })
                            .unwrap_or_else(|| "未选择".into());
                        let n = state.checked.get().len();
                        if n == 0 {
                            current
                        } else {
                            format!("{current} · 已勾选 {n} 项")
                        }
                    }}
                </span>
            </footer>
            <ConfirmDelete/>
            <RenameDialog/>
            <MkdirDialog/>
            <MkfileDialog/>
            <FailureDialog/>
        </div>
    }.into_any()
}

#[component]
fn RootLabel() -> impl IntoView {
    let info = Resource::new(|| (), |_| async move { get_root_info().await });
    view! {
        <div class="root-label">
            <span class="muted">"托管目录"</span>
            <Suspense fallback=|| view! { <code>"…"</code> }>
                {move || {
                    info.get().map(|res| match res {
                        Ok(path) => view! { <code>{path}</code> }.into_any(),
                        Err(_) => view! { <code>"PIC_ROOT"</code> }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn ConfirmDelete() -> impl IntoView {
    let state = expect_context::<ExplorerState>();

    view! {
        <Show when=move || state.confirm_delete.get().is_some()>
            <div class="modal-backdrop" on:click=move |_| state.confirm_delete.set(None)>
                <div class="modal" on:click=move |ev| ev.stop_propagation()>
                    <h2>"确认删除"</h2>
                    <p>
                        "将永久删除 "
                        <strong>
                            {move || {
                                match state.confirm_delete.get().as_deref() {
                                    Some([one]) => one.name.clone(),
                                    Some(items) => format!("{} 项", items.len()),
                                    None => String::new(),
                                }
                            }}
                        </strong>
                        " ，此操作不可恢复。"
                    </p>
                    <div class="modal-actions">
                        <button class="btn" on:click=move |_| state.confirm_delete.set(None)>
                            "取消"
                        </button>
                        <button class="btn btn-danger" on:click=move |_| state.confirm_delete_selected()>
                            "删除"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }.into_any()
}

#[component]
fn RenameDialog() -> impl IntoView {
    let state = expect_context::<ExplorerState>();

    view! {
        <Show when=move || state.rename_target.get().is_some()>
            <div class="modal-backdrop" on:click=move |_| state.rename_target.set(None)>
                <div class="modal" on:click=move |ev| ev.stop_propagation()>
                    <h2>"重命名"</h2>
                    <p>
                        {move || {
                            state
                                .rename_target
                                .get()
                                .map(|item| {
                                    if item.is_dir {
                                        format!("文件夹：{}", item.name)
                                    } else {
                                        format!("文件：{}", item.name)
                                    }
                                })
                                .unwrap_or_default()
                        }}
                    </p>
                    <input
                        class="modal-input"
                        type="text"
                        autofocus
                        prop:value=move || state.rename_draft.get()
                        on:input=move |ev| state.rename_draft.set(event_target_value(&ev))
                    />
                    <div class="modal-actions">
                        <button class="btn" on:click=move |_| state.rename_target.set(None)>
                            "取消"
                        </button>
                        <button class="btn" on:click=move |_| state.confirm_rename()>
                            "确定"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }.into_any()
}

#[component]
fn MkdirDialog() -> impl IntoView {
    let state = expect_context::<ExplorerState>();

    view! {
        <Show when=move || state.mkdir_parent.get().is_some()>
            <div class="modal-backdrop" on:click=move |_| state.mkdir_parent.set(None)>
                <div class="modal" on:click=move |ev| ev.stop_propagation()>
                    <h2>"新建目录"</h2>
                    <p>
                        {move || {
                            match state.mkdir_parent.get().as_deref() {
                                Some("") => "位置：/".into(),
                                Some(path) => format!("位置：/{path}"),
                                None => String::new(),
                            }
                        }}
                    </p>
                    <input
                        class="modal-input"
                        type="text"
                        autofocus
                        prop:value=move || state.mkdir_draft.get()
                        on:input=move |ev| state.mkdir_draft.set(event_target_value(&ev))
                    />
                    <div class="modal-actions">
                        <button class="btn" on:click=move |_| state.mkdir_parent.set(None)>
                            "取消"
                        </button>
                        <button class="btn" on:click=move |_| state.confirm_mkdir()>
                            "创建"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
    .into_any()
}

#[component]
fn MkfileDialog() -> impl IntoView {
    let state = expect_context::<ExplorerState>();

    view! {
        <Show when=move || state.mkfile_parent.get().is_some()>
            <div class="modal-backdrop" on:click=move |_| state.mkfile_parent.set(None)>
                <div class="modal" on:click=move |ev| ev.stop_propagation()>
                    <h2>"新建文件"</h2>
                    <p>
                        {move || {
                            match state.mkfile_parent.get().as_deref() {
                                Some("") => "位置：/".into(),
                                Some(path) => format!("位置：/{path}"),
                                None => String::new(),
                            }
                        }}
                    </p>
                    <input
                        class="modal-input"
                        type="text"
                        autofocus
                        prop:value=move || state.mkfile_draft.get()
                        on:input=move |ev| state.mkfile_draft.set(event_target_value(&ev))
                    />
                    <div class="modal-actions">
                        <button class="btn" on:click=move |_| state.mkfile_parent.set(None)>
                            "取消"
                        </button>
                        <button class="btn" on:click=move |_| state.confirm_mkfile()>
                            "创建"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
    .into_any()
}

#[component]
fn FailureDialog() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let copied = RwSignal::new(false);

    Effect::new(move |_| {
        state.failure_report.track();
        copied.set(false);
    });

    view! {
        <Show when=move || state.failure_report.get().is_some()>
            <div
                class="modal-backdrop"
                on:click=move |_| {
                    copied.set(false);
                    state.failure_report.set(None);
                }
            >
                <div class="modal modal-wide" on:click=move |ev| ev.stop_propagation()>
                    <h2>
                        {move || {
                            state
                                .failure_report
                                .get()
                                .map(|r| r.title)
                                .unwrap_or_else(|| "失败列表".into())
                        }}
                    </h2>
                    <p>
                        {move || {
                            state
                                .failure_report
                                .get()
                                .map(|r| format!("共 {} 项失败", r.failures.len()))
                                .unwrap_or_default()
                        }}
                    </p>
                    <ul class="fail-list">
                        {move || {
                            state
                                .failure_report
                                .get()
                                .map(|r| {
                                    r.failures
                                        .into_iter()
                                        .map(|item| {
                                            let file = if item.file.is_empty() {
                                                "(未命名)".to_string()
                                            } else {
                                                item.file
                                            };
                                            view! {
                                                <li>
                                                    <code>{file}</code>
                                                    <span class="fail-error">{item.error}</span>
                                                </li>
                                            }
                                        })
                                        .collect_view()
                                })
                        }}
                    </ul>
                    <div class="modal-actions">
                        <button
                            class="btn"
                            type="button"
                            on:click=move |_| {
                                let Some(report) = state.failure_report.get() else {
                                    return;
                                };
                                let list = report
                                    .failures
                                    .into_iter()
                                    .map(|item| item.file)
                                    .filter(|f| !f.is_empty())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                if copy_plain_text(&list) {
                                    copied.set(true);
                                    state.status.set("已复制失败文件列表".into());
                                } else {
                                    state.status.set("复制失败，请手动选择列表".into());
                                }
                            }
                        >
                            {move || if copied.get() { "已复制" } else { "复制文件列表" }}
                        </button>
                        <button
                            class="btn"
                            on:click=move |_| {
                                copied.set(false);
                                state.failure_report.set(None);
                            }
                        >
                            "关闭"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
    .into_any()
}

#[component]
fn PanelToggle(label: &'static str, on: RwSignal<bool>) -> impl IntoView {
    view! {
        <button
            type="button"
            class="btn"
            class:is-active=move || on.get()
            aria-pressed=move || on.get()
            on:click=move |_| on.update(|v| *v = !*v)
        >
            {label}
        </button>
    }
}
