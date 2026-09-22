use crate::function::list_dir;
use crate::structure::{ClipboardMode, ExplorerState, FsEntry, SelectedItem};
use leptos::prelude::*;

#[component]
pub fn FilePane() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let no_tree_target = move || state.selected.get().map(|s| s.path.is_empty()).unwrap_or(true);
    let no_checked = move || state.checked.get().is_empty();
    let clipboard_empty = move || state.clipboard.get().is_none() || state.busy.get();

    view! {
        <aside class="sidebar" class:panel-off=move || !state.show_file_manager.get()>
            <div class="toolbar-stack">
                <div class="toolbar">
                    <span class="toolbar-label">"当前项"</span>
                    <button
                        class="btn"
                        title="复制当前项"
                        disabled=no_tree_target
                        on:click=move |_| state.copy_selected()
                    >
                        "复制"
                    </button>
                    <button
                        class="btn"
                        title="剪切当前项"
                        disabled=no_tree_target
                        on:click=move |_| state.cut_selected()
                    >
                        "剪切"
                    </button>
                    <button
                        class="btn"
                        title="粘贴到当前目录"
                        disabled=clipboard_empty
                        on:click=move |_| state.paste_clipboard()
                    >
                        "粘贴"
                    </button>
                    <button
                        class="btn"
                        title="重命名当前项"
                        disabled=no_tree_target
                        on:click=move |_| state.request_rename()
                    >
                        "重命名"
                    </button>
                    <button
                        class="btn btn-danger-ghost"
                        title="删除当前项"
                        disabled=move || no_tree_target() || state.busy.get()
                        on:click=move |_| state.request_delete()
                    >
                        "删除"
                    </button>
                    <button
                        class="btn btn-ghost"
                        title="刷新"
                        on:click=move |_| {
                            state.refresh.update(|n| *n += 1);
                            state.status.set("已刷新".into());
                        }
                    >
                        "刷新"
                    </button>
                </div>
                <div class="toolbar toolbar-checked">
                    <span class="toolbar-label">
                        {move || {
                            let n = state.checked.get().len();
                            if n == 0 {
                                "已选图片".into()
                            } else {
                                format!("已选图片（{n}）")
                            }
                        }}
                    </span>
                    <button
                        class="btn"
                        title="勾选当前目录全部图片"
                        disabled=move || state.busy.get()
                        on:click=move |_| state.check_all_current()
                    >
                        "全选"
                    </button>
                    <button
                        class="btn"
                        title="取消全部勾选"
                        disabled=no_checked
                        on:click=move |_| state.uncheck_all()
                    >
                        "全不选"
                    </button>
                    <button
                        class="btn"
                        title="复制勾选的图片"
                        disabled=no_checked
                        on:click=move |_| state.copy_checked()
                    >
                        "复制"
                    </button>
                    <button
                        class="btn"
                        title="剪切勾选的图片"
                        disabled=no_checked
                        on:click=move |_| state.cut_checked()
                    >
                        "剪切"
                    </button>
                    <button
                        class="btn"
                        title="粘贴到当前目录"
                        disabled=clipboard_empty
                        on:click=move |_| state.paste_clipboard()
                    >
                        "粘贴"
                    </button>
                    <button
                        class="btn btn-danger-ghost"
                        title="删除勾选的图片"
                        disabled=move || no_checked() || state.busy.get()
                        on:click=move |_| state.request_delete_checked()
                    >
                        "删除"
                    </button>
                    <button
                        class="btn btn-ghost"
                        title="刷新"
                        on:click=move |_| {
                            state.refresh.update(|n| *n += 1);
                            state.status.set("已刷新".into());
                        }
                    >
                        "刷新"
                    </button>
                </div>
            </div>
            <div class="clipboard-hint">
                {move || match state.clipboard.get() {
                    None => "剪贴板为空".into(),
                    Some(clip) => {
                        let verb = match clip.mode {
                            ClipboardMode::Copy => "复制",
                            ClipboardMode::Cut => "剪切",
                        };
                        if clip.items.len() == 1 {
                            format!("剪贴板（{verb}）：{}", clip.items[0].name)
                        } else {
                            format!("剪贴板（{verb}）：{} 项", clip.items.len())
                        }
                    }
                }}
            </div>
            <div class="tree-scroll">
                <RootTree/>
            </div>
        </aside>
    }.into_any()
}

#[component]
fn RootTree() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let root = FsEntry {
        name: "/".into(),
        path: String::new(),
        is_dir: true,
        is_image: false,
    };

    Effect::new(move |_| {
        if state.selected.get().is_none() {
            state.selected.set(Some(SelectedItem {
                path: String::new(),
                name: "/".into(),
                is_dir: true,
                is_image: false,
            }));
        }
    });

    view! { <ul class="tree"><TreeNode entry=root depth=0 start_open=true/></ul> }
}

#[component]
fn TreeNode(entry: FsEntry, depth: u32, #[prop(optional)] start_open: bool) -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let expanded = RwSignal::new(start_open);
    let path = entry.path.clone();
    let name = entry.name.clone();
    let is_dir = entry.is_dir;
    let is_image = entry.is_image;

    let children = Resource::new(
        move || (expanded.get(), state.refresh.get()),
        {
            let path = path.clone();
            move |(open, _)| {
                let path = path.clone();
                async move {
                    if open && is_dir {
                        list_dir(path).await
                    } else {
                        Ok(Vec::new())
                    }
                }
            }
        },
    );

    let select = {
        let path = path.clone();
        let name = name.clone();
        move |_| {
            state.selected.set(Some(SelectedItem {
                path: path.clone(),
                name: name.clone(),
                is_dir,
                is_image,
            }));
            if is_image {
                state.viewed.set(Some(path.clone()));
            }
        }
    };

    let indent = 10 + depth * 14;
    let path_for_class = path.clone();
    let name_for_view = name.clone();

    let toggle_row = {
        let path = path.clone();
        let name = name.clone();
        move |ev: leptos::ev::MouseEvent| {
            ev.stop_propagation();
            if is_dir {
                expanded.update(|v| *v = !*v);
                state.selected.set(Some(SelectedItem {
                    path: path.clone(),
                    name: name.clone(),
                    is_dir,
                    is_image,
                }));
            }
        }
    };
    let toggle_chevron = {
        let path = path.clone();
        let name = name.clone();
        move |ev: leptos::ev::MouseEvent| {
            ev.stop_propagation();
            if is_dir {
                expanded.update(|v| *v = !*v);
                state.selected.set(Some(SelectedItem {
                    path: path.clone(),
                    name: name.clone(),
                    is_dir,
                    is_image,
                }));
            }
        }
    };

    view! {
        <li class="tree-li">
            <div
                class=move || {
                    let mut class = "tree-item".to_string();
                    if state.selected.get().as_ref().map(|s| s.path.as_str())
                        == Some(path_for_class.as_str())
                    {
                        class.push_str(" is-selected");
                    }
                    if state.is_checked(&path_for_class) {
                        class.push_str(" is-checked");
                    }
                    if state
                        .clipboard
                        .get()
                        .filter(|c| c.mode == ClipboardMode::Cut)
                        .is_some_and(|c| c.items.iter().any(|i| i.path == path_for_class))
                    {
                        class.push_str(" is-cut");
                    }
                    if is_dir {
                        class.push_str(" is-dir");
                    }
                    if is_image {
                        class.push_str(" is-image");
                    }
                    class
                }
                style=format!("padding-left:{indent}px")
                on:click=select
                on:dblclick=toggle_row
            >
                <button
                    class="chevron"
                    class:is-hidden=!is_dir
                    class:is-open=move || expanded.get()
                    on:click=toggle_chevron
                    aria-label="展开"
                >
                    "▸"
                </button>
                <span class="icon">{if is_dir { "📁" } else if is_image { "🖼" } else { "📄" }}</span>
                <span class="name" title=name_for_view.clone()>{name_for_view.clone()}</span>
            </div>
            <Show when=move || is_dir && expanded.get()>
                <Suspense fallback=|| view! { <div class="tree-loading">"加载中…"</div> }>
                    {move || {
                        children.get().map(|res| match res {
                            Ok(entries) if entries.is_empty() => view! {
                                <div class="tree-empty" style=format!("padding-left:{}px", indent + 22)>
                                    "空目录"
                                </div>
                            }.into_any(),
                            Ok(entries) => {
                                let next = depth + 1;
                                view! {
                                    <ul class="tree">
                                        {entries
                                            .into_iter()
                                            .map(|child| {
                                                view! { <TreeNode entry=child depth=next/> }.into_any()
                                            })
                                            .collect_view()}
                                    </ul>
                                }.into_any()
                            }
                            Err(e) => view! {
                                <div class="tree-error" style=format!("padding-left:{}px", indent + 22)>
                                    {e.to_string()}
                                </div>
                            }.into_any(),
                        })
                    }}
                </Suspense>
            </Show>
        </li>
    }.into_any()
}
