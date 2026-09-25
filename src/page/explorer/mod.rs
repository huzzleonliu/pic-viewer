mod dialogs;

use crate::function::get_root_info;
use crate::page::file_tree::FilePane;
use crate::page::image_viewer::ImageViewer;
use crate::page::text_editor::TextEditor;
use crate::structure::ExplorerState;
use dialogs::{ConfirmDelete, FailureDialog, MkdirDialog, MkfileDialog, RenameDialog};
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
                        let n = state.checked.with(|list| list.len());
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
    }
    .into_any()
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
