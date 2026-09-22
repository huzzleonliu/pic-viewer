use crate::function::get_root_info;
use crate::page::file_tree::FilePane;
use crate::page::image_viewer::ImageViewer;
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
                    <PanelToggle label="缩略图" on=state.show_thumbnails/>
                    <PanelToggle label="简单调整" on=state.show_adjust/>
                    <PanelToggle label="文件管理器" on=state.show_file_manager/>
                </div>
            </header>
            <div class="workspace">
                <FilePane/>
                <ImageViewer/>
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
                            format!("{current} · 已勾选 {n} 张")
                        }
                    }}
                </span>
            </footer>
            <ConfirmDelete/>
            <RenameDialog/>
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
