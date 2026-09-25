use crate::function::list_dir;
use crate::structure::{ClipboardMode, ExplorerState, FsEntry, SelectedItem};
use leptos::prelude::*;

#[component]
pub fn FilePane() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let no_tree_target = move || {
        state
            .selected
            .get()
            .map(|s| s.path.is_empty())
            .unwrap_or(true)
    };
    let no_checked = move || state.checked.with(|list| list.is_empty());
    let clipboard_empty = move || state.clipboard.get().is_none() || state.busy.get();

    view! {
        <aside class="sidebar" class:panel-off=move || !state.show_file_manager.get()>
            <div class="toolbar-stack">
                <div class="toolbar">
                    <span class="toolbar-label">"当前项"</span>
                    <button
                        class="btn"
                        title="在当前目录新建文件夹"
                        disabled=move || state.busy.get()
                        on:click=move |_| state.request_mkdir()
                    >
                        "新建目录"
                    </button>
                    <button
                        class="btn"
                        title="在当前目录新建文本文件"
                        disabled=move || state.busy.get()
                        on:click=move |_| state.request_mkfile()
                    >
                        "新建文件"
                    </button>
                    <button
                        class="btn"
                        title="复制相对托管目录的路径，可粘贴到导出目录"
                        on:click=move |_| {
                            let path = state.selected_dir_rel();
                            if path.is_empty() {
                                state.status.set("当前是托管根目录，相对路径为空".into());
                                return;
                            }
                            if crate::page::copy_text::copy_plain_text(&path) {
                                state.status.set(format!("已复制路径：{path}"));
                            } else {
                                state.status.set("复制失败".into());
                            }
                        }
                    >
                        "复制路径"
                    </button>
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
                            let n = state.checked.with(|list| list.len());
                            if n == 0 {
                                "已选".into()
                            } else {
                                format!("已选（{n}）")
                            }
                        }}
                    </span>
                    <button
                        class="btn"
                        title="勾选当前目录全部项目"
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
                        title="复制勾选的项目"
                        disabled=no_checked
                        on:click=move |_| state.copy_checked()
                    >
                        "复制"
                    </button>
                    <button
                        class="btn"
                        title="剪切勾选的项目"
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
                        title="删除勾选的项目"
                        disabled=move || no_checked() || state.busy.get()
                        on:click=move |_| state.request_delete_checked()
                    >
                        "删除"
                    </button>
                    <button
                        class="btn btn-ghost"
                        title="刷新并清空勾选"
                        on:click=move |_| {
                            state.checked.update(|list| list.clear());
                            state.refresh.update(|n| *n += 1);
                            state.status.set("已刷新，已清空勾选".into());
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
    }
    .into_any()
}

#[component]
fn RootTree() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let root = FsEntry {
        name: "/".into(),
        path: String::new(),
        is_dir: true,
        is_image: false,
        is_text: false,
    };

    Effect::new(move |_| {
        if state.selected.get().is_none() {
            state.selected.set(Some(SelectedItem::root()));
        }
    });

    view! { <ul class="tree"><TreeNode entry=root depth=0/></ul> }
}

#[component]
fn TreeNode(entry: FsEntry, depth: u32) -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let item = SelectedItem::from(&entry);
    let path = item.path.clone();
    let is_dir = item.is_dir;
    let is_image = item.is_image;
    let is_text = item.is_text;
    let name_for_view = item.name.clone();
    let path_expanded = path.clone();

    let children = Resource::new(
        move || (state.is_expanded(&path_expanded), state.refresh.get()),
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
        let item = item.clone();
        move |_| {
            if item.is_image {
                state.viewed.set(Some(item.path.clone()));
            }
            state.selected.set(Some(item.clone()));
        }
    };

    let toggle_row = expand_and_select(state, item.clone());
    let toggle_chevron = expand_and_select(state, item.clone());

    let indent = 10 + depth * 14;
    let can_check = !path.is_empty();
    let path_for_class = path.clone();
    let path_open = path.clone();
    let path_show = path.clone();
    let path_box = path.clone();
    let item_check = item.clone();

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
                    if is_text {
                        class.push_str(" is-text");
                    }
                    class
                }
                on:click=select
                on:dblclick=toggle_row
            >
                <label
                    class="tree-check"
                    class:is-root=!can_check
                    on:click=move |ev| ev.stop_propagation()
                    on:mousedown=move |ev| ev.stop_propagation()
                    on:dblclick=move |ev| ev.stop_propagation()
                >
                    <input
                        type="checkbox"
                        prop:disabled=!can_check
                        prop:checked=move || can_check && state.is_checked(&path_box)
                        on:change=move |ev| {
                            if !can_check {
                                return;
                            }
                            state.set_checked(item_check.clone(), event_target_checked(&ev));
                        }
                    />
                </label>
                <div class="tree-item-body" style=format!("padding-left:{indent}px")>
                    <button
                        class="chevron"
                        class:is-hidden=!is_dir
                        class:is-open=move || state.is_expanded(&path_open)
                        on:click=toggle_chevron
                        aria-label="展开"
                    >
                        "▸"
                    </button>
                    <span class="icon">{if is_dir { "📁" } else if is_image { "🖼" } else if is_text { "📝" } else { "📄" }}</span>
                    <span class="name" title=name_for_view.clone()>{name_for_view.clone()}</span>
                </div>
            </div>
            <Show when=move || is_dir && state.is_expanded(&path_show)>
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

fn expand_and_select(
    state: ExplorerState,
    item: SelectedItem,
) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        if item.is_dir {
            state.toggle_expanded(&item.path);
            state.selected.set(Some(item.clone()));
        }
    }
}
