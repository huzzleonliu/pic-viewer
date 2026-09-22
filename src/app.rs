use crate::file_tree::FilePane;
use crate::fs_api::{
    delete_entry, is_image_name, parent_path, paste_entry, rename_entry, rewrite_prefix,
    validate_file_name,
};
use crate::image_viewer::ImageViewer;
use leptos::ev;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::StaticSegment;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_image: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clipboard {
    pub items: Vec<ClipboardItem>,
    pub mode: ClipboardMode,
}

#[derive(Clone, Copy)]
pub struct ExplorerState {
    pub selected: RwSignal<Option<SelectedItem>>,
    pub clipboard: RwSignal<Option<Clipboard>>,
    pub checked: RwSignal<Vec<SelectedItem>>,
    pub viewed: RwSignal<Option<String>>,
    pub refresh: RwSignal<u64>,
    pub status: RwSignal<String>,
    pub confirm_delete: RwSignal<Option<Vec<SelectedItem>>>,
    pub rename_target: RwSignal<Option<SelectedItem>>,
    pub rename_draft: RwSignal<String>,
    pub busy: RwSignal<bool>,
}

impl ExplorerState {
    pub fn is_checked(self, path: &str) -> bool {
        self.checked.get().iter().any(|i| i.path == path)
    }

    pub fn set_image_checked(self, path: String, name: String, on: bool) {
        self.checked.update(|list| {
            let idx = list.iter().position(|x| x.path == path);
            match (on, idx) {
                (true, None) => list.push(SelectedItem {
                    path,
                    name,
                    is_dir: false,
                    is_image: true,
                }),
                (false, Some(i)) => {
                    list.remove(i);
                }
                _ => {}
            }
        });
    }

    fn set_clipboard(self, items: Vec<ClipboardItem>, mode: ClipboardMode) {
        if items.is_empty() {
            return;
        }
        let label = match (mode, items.len()) {
            (ClipboardMode::Copy, 1) => format!("已复制：{}", items[0].name),
            (ClipboardMode::Cut, 1) => format!("已剪切：{}", items[0].name),
            (ClipboardMode::Copy, n) => format!("已复制 {n} 项"),
            (ClipboardMode::Cut, n) => format!("已剪切 {n} 项"),
        };
        self.clipboard.set(Some(Clipboard { items, mode }));
        self.status.set(label);
    }

    pub fn copy_selected(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能复制根目录".into());
                return;
            }
            self.set_clipboard(
                vec![ClipboardItem {
                    path: item.path,
                    name: item.name,
                    is_dir: item.is_dir,
                }],
                ClipboardMode::Copy,
            );
        }
    }

    pub fn cut_selected(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能剪切根目录".into());
                return;
            }
            self.set_clipboard(
                vec![ClipboardItem {
                    path: item.path,
                    name: item.name,
                    is_dir: item.is_dir,
                }],
                ClipboardMode::Cut,
            );
        }
    }

    pub fn copy_checked(self) {
        let items = self.checked_clipboard_items();
        if items.is_empty() {
            self.status.set("未勾选图片".into());
            return;
        }
        self.set_clipboard(items, ClipboardMode::Copy);
    }

    pub fn cut_checked(self) {
        let items = self.checked_clipboard_items();
        if items.is_empty() {
            self.status.set("未勾选图片".into());
            return;
        }
        self.set_clipboard(items, ClipboardMode::Cut);
    }

    fn checked_clipboard_items(self) -> Vec<ClipboardItem> {
        self.checked
            .get()
            .into_iter()
            .map(|i| ClipboardItem {
                path: i.path,
                name: i.name,
                is_dir: i.is_dir,
            })
            .collect()
    }

    pub fn paste_clipboard(self) {
        let Some(clip) = self.clipboard.get() else {
            self.status.set("剪贴板为空".into());
            return;
        };
        if self.busy.get() {
            return;
        }
        let dest_dir = match self.selected.get() {
            Some(s) if s.is_dir => s.path,
            Some(s) => parent_path(&s.path),
            None => String::new(),
        };
        let cut = clip.mode == ClipboardMode::Cut;
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            let mut ok = 0usize;
            let mut last_path = None;
            let mut last_item = None;
            let mut first_err = None;
            for item in &clip.items {
                match paste_entry(item.path.clone(), dest_dir.clone(), cut).await {
                    Ok(new_path) => {
                        ok += 1;
                        last_path = Some(new_path.clone());
                        last_item = Some(item.clone());
                        if cut && new_path != item.path {
                            self.forget_path(&item.path);
                        }
                    }
                    Err(e) => {
                        first_err = Some(e.to_string());
                        break;
                    }
                }
            }
            if let Some(err) = first_err {
                self.status.set(format!("粘贴失败（已完成 {ok} 项）：{err}"));
            } else if let (Some(new_path), Some(item)) = (last_path, last_item) {
                let name = new_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&item.name)
                    .to_string();
                self.selected.set(Some(SelectedItem {
                    is_image: !item.is_dir && is_image_name(&name),
                    path: new_path,
                    name,
                    is_dir: item.is_dir,
                }));
                let verb = if cut { "已移动" } else { "已粘贴" };
                self.status.set(if clip.items.len() == 1 {
                    format!("{verb}：{}", item.name)
                } else {
                    format!("{verb} {ok} 项")
                });
            }
            if cut && ok == clip.items.len() {
                self.clipboard.set(None);
            }
            self.refresh.update(|n| *n += 1);
            self.busy.set(false);
        });
    }

    pub fn request_rename(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能重命名根目录".into());
                return;
            }
            self.rename_draft.set(item.name.clone());
            self.rename_target.set(Some(item));
        }
    }

    pub fn confirm_rename(self) {
        let Some(target) = self.rename_target.get() else {
            return;
        };
        if self.busy.get() {
            return;
        }
        let new_name = self.rename_draft.get();
        if let Err(err) = validate_file_name(&new_name) {
            self.status.set(err);
            return;
        }
        let new_name = new_name.trim().to_string();
        if new_name == target.name {
            self.rename_target.set(None);
            return;
        }
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            match rename_entry(target.path.clone(), new_name).await {
                Ok(new_path) => {
                    self.retarget_path(&target.path, &new_path);
                    self.rename_target.set(None);
                    self.refresh.update(|n| *n += 1);
                    let name = new_path
                        .rsplit('/')
                        .next()
                        .unwrap_or(new_path.as_str())
                        .to_string();
                    self.status.set(format!("已重命名为：{name}"));
                }
                Err(e) => self.status.set(format!("重命名失败：{e}")),
            }
            self.busy.set(false);
        });
    }

    fn retarget_path(self, old: &str, new: &str) {
        let map = |path: &str| rewrite_prefix(path, old, new);
        let file_name = |path: &str| {
            path.rsplit('/')
                .next()
                .unwrap_or(path)
                .to_string()
        };

        if let Some(v) = self.viewed.get() {
            let nv = map(&v);
            if nv != v {
                if is_image_name(&file_name(&nv)) {
                    self.viewed.set(Some(nv));
                } else {
                    self.viewed.set(None);
                }
            }
        }
        if let Some(mut selected) = self.selected.get() {
            let np = map(&selected.path);
            if np != selected.path {
                selected.name = file_name(&np);
                selected.is_image = !selected.is_dir && is_image_name(&selected.name);
                selected.path = np;
                self.selected.set(Some(selected));
            }
        }
        self.checked.update(|list| {
            list.retain_mut(|item| {
                let np = map(&item.path);
                if np != item.path {
                    item.name = file_name(&np);
                    item.is_image = is_image_name(&item.name);
                    item.path = np;
                }
                item.is_image
            });
        });
        if let Some(mut clip) = self.clipboard.get() {
            for item in &mut clip.items {
                let np = map(&item.path);
                if np != item.path {
                    item.name = file_name(&np);
                    item.path = np;
                }
            }
            self.clipboard.set(Some(clip));
        }
    }

    pub fn request_delete(self) {
        if let Some(item) = self.selected.get() {
            if item.path.is_empty() {
                self.status.set("不能删除根目录".into());
                return;
            }
            self.confirm_delete.set(Some(vec![item]));
        }
    }

    pub fn request_delete_checked(self) {
        let items = self.checked.get();
        if items.is_empty() {
            self.status.set("未勾选图片".into());
            return;
        }
        self.confirm_delete.set(Some(items));
    }

    pub fn confirm_delete_selected(self) {
        let Some(items) = self.confirm_delete.get() else {
            return;
        };
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        leptos::task::spawn_local(async move {
            let mut ok = 0usize;
            let mut first_err = None;
            let total = items.len();
            for item in items {
                match delete_entry(item.path.clone()).await {
                    Ok(()) => {
                        ok += 1;
                        self.forget_path(&item.path);
                    }
                    Err(e) => {
                        first_err = Some((item.name, e.to_string()));
                        break;
                    }
                }
            }
            if let Some((name, err)) = first_err {
                self.status
                    .set(format!("删除失败（已完成 {ok} 项，{name}）：{err}"));
            } else {
                self.status.set(if total == 1 {
                    "已删除".into()
                } else {
                    format!("已删除 {ok} 项")
                });
            }
            self.confirm_delete.set(None);
            self.refresh.update(|n| *n += 1);
            self.busy.set(false);
        });
    }

    fn forget_path(self, path: &str) {
        if self.viewed.get().as_deref() == Some(path) {
            self.viewed.set(None);
        }
        if self.selected.get().as_ref().map(|s| s.path.as_str()) == Some(path) {
            self.selected.set(None);
        }
        self.checked
            .update(|list| list.retain(|i| i.path != path));
        if let Some(clip) = self.clipboard.get() {
            let items: Vec<_> = clip
                .items
                .into_iter()
                .filter(|i| i.path != path)
                .collect();
            if items.is_empty() {
                self.clipboard.set(None);
            } else {
                self.clipboard.set(Some(Clipboard {
                    mode: clip.mode,
                    items,
                }));
            }
        }
    }
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/pic-viewer.css"/>
        <Title text="Pic Viewer"/>
        <Router>
            <main>
                <Routes fallback=|| view! { <p class="not-found">"页面不存在"</p> }>
                    <Route path=StaticSegment("") view=Explorer/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Explorer() -> impl IntoView {
    let state = ExplorerState {
        selected: RwSignal::new(None),
        clipboard: RwSignal::new(None),
        checked: RwSignal::new(Vec::new()),
        viewed: RwSignal::new(None),
        refresh: RwSignal::new(0),
        status: RwSignal::new("就绪".into()),
        confirm_delete: RwSignal::new(None),
        rename_target: RwSignal::new(None),
        rename_draft: RwSignal::new(String::new()),
        busy: RwSignal::new(false),
    };
    provide_context(state);

    let handle = window_event_listener(ev::keydown, move |ev: ev::KeyboardEvent| {
        if state.rename_target.get_untracked().is_some() {
            if ev.key() == "Escape" {
                state.rename_target.set(None);
            }
            return;
        }
        if state.confirm_delete.get_untracked().is_some() {
            if ev.key() == "Escape" {
                state.confirm_delete.set(None);
            }
            return;
        }
        let ctrl = ev.ctrl_key() || ev.meta_key();
        match (ctrl, ev.key().as_str()) {
            (true, "c") => {
                ev.prevent_default();
                state.copy_selected();
            }
            (true, "x") => {
                ev.prevent_default();
                state.cut_selected();
            }
            (true, "v") => {
                ev.prevent_default();
                state.paste_clipboard();
            }
            (false, "F2") => {
                ev.prevent_default();
                state.request_rename();
            }
            (false, "Delete") => state.request_delete(),
            _ => {}
        }
    });
    on_cleanup(move || drop(handle));

    view! {
        <div class="explorer">
            <header class="app-header">
                <div class="brand">
                    <span class="brand-mark">"▣"</span>
                    <span>"Pic Viewer"</span>
                </div>
                <RootLabel/>
                <div class="header-hint">"Ctrl+C 复制 · Ctrl+X 剪切 · Ctrl+V 粘贴 · F2 重命名 · Delete 删除"</div>
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
    }
}

#[component]
fn RootLabel() -> impl IntoView {
    let info = Resource::new(|| (), |_| async move { crate::fs_api::get_root_info().await });
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
    }
}

#[component]
fn RenameDialog() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if state.rename_target.get().is_none() {
            return;
        }
        if let Some(input) = input_ref.get() {
            let _ = input.focus();
            input.select();
        }
    });

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
                        node_ref=input_ref
                        class="modal-input"
                        type="text"
                        prop:value=move || state.rename_draft.get()
                        on:input=move |ev| state.rename_draft.set(event_target_value(&ev))
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                state.confirm_rename();
                            }
                        }
                    />
                    <div class="modal-actions">
                        <button class="btn" on:click=move |_| state.rename_target.set(None)>
                            "取消"
                        </button>
                        <button
                            class="btn"
                            disabled=move || state.busy.get()
                            on:click=move |_| state.confirm_rename()
                        >
                            "确定"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
