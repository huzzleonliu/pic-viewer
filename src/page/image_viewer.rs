use crate::function::{
    get_image_rating, get_image_tags, list_dir, media_url, parent_path, set_image_rating,
    set_image_tags,
};
use crate::structure::{ExplorerState, FsEntry, SelectedItem};
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use leptos::server_fn::ServerFnError;

#[component]
pub fn ImageViewer() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let zoom = RwSignal::new(1.0_f64);
    let rotate = RwSignal::new(0_i32);
    let pan = RwSignal::new((0.0_f64, 0.0_f64));
    let dragging = RwSignal::new(false);
    let last_pos = RwSignal::new((0.0_f64, 0.0_f64));
    let loaded = RwSignal::new(false);
    let failed = RwSignal::new(false);

    Effect::new(move |_| {
        state.viewed.track();
        zoom.set(1.0);
        rotate.set(0);
        pan.set((0.0, 0.0));
        loaded.set(false);
        failed.set(false);
    });

    let gallery = Resource::new(
        move || {
            let viewed = state.viewed.get();
            let selected = state.selected.get();
            let refresh = state.refresh.get();
            let dir = if let Some(path) = viewed {
                parent_path(&path)
            } else if let Some(s) = selected {
                if s.is_dir {
                    s.path
                } else {
                    parent_path(&s.path)
                }
            } else {
                String::new()
            };
            (dir, refresh)
        },
        |(dir, _)| async move {
            let entries = list_dir(dir).await?;
            Ok::<Vec<FsEntry>, ServerFnError>(
                entries.into_iter().filter(|e| e.is_image).collect(),
            )
        },
    );

    let go_relative = move |delta: isize| {
        let Some(current) = state.viewed.get() else {
            return;
        };
        let Some(Ok(list)) = gallery.get() else {
            return;
        };
        let Some(idx) = list.iter().position(|e| e.path == current) else {
            return;
        };
        let next = idx as isize + delta;
        if next >= 0 && (next as usize) < list.len() {
            let entry = &list[next as usize];
            state.viewed.set(Some(entry.path.clone()));
            state.selected.set(Some(SelectedItem {
                path: entry.path.clone(),
                name: entry.name.clone(),
                is_dir: false,
                is_image: true,
            }));
        }
    };

    let zoom_by = move |factor: f64| {
        zoom.update(|z| {
            *z = (*z * factor).clamp(0.1, 8.0);
        });
    };

    let reset = move |_| {
        zoom.set(1.0);
        rotate.set(0);
        pan.set((0.0, 0.0));
    };

    view! {
        <section class="viewer">
            <div class="viewer-toolbar" class:panel-off=move || !state.show_adjust.get()>
                <button class="btn" on:click=move |_| zoom_by(1.0 / 1.2) title="缩小">"−"</button>
                <span class="zoom-label">{move || format!("{}%", (zoom.get() * 100.0).round())}</span>
                <button class="btn" on:click=move |_| zoom_by(1.2) title="放大">"+"</button>
                <button class="btn" on:click=move |_| rotate.update(|r| *r = (*r - 90).rem_euclid(360)) title="左转">
                    "↺"
                </button>
                <button class="btn" on:click=move |_| rotate.update(|r| *r = (*r + 90) % 360) title="右转">
                    "↻"
                </button>
                <button class="btn" on:click=reset title="重置缩放与旋转">"重置"</button>
                <button class="btn" on:click=move |_| go_relative(-1) title="上一张">"‹"</button>
                <button class="btn" on:click=move |_| go_relative(1) title="下一张">"›"</button>
                <span class="viewer-name">
                    {move || {
                        state
                            .viewed
                            .get()
                            .map(|p| p.rsplit('/').next().unwrap_or(&p).to_string())
                            .unwrap_or_else(|| "未选择图片".into())
                    }}
                </span>
            </div>
            <div
                class="stage"
                class:is-dragging=move || dragging.get()
                class:is-picked=move || {
                    state.viewed.get().is_some_and(|p| state.is_checked(&p))
                }
                on:wheel=move |ev: ev::WheelEvent| {
                    ev.prevent_default();
                    let factor = if ev.delta_y() < 0.0 { 1.12 } else { 1.0 / 1.12 };
                    zoom.update(|z| {
                        *z = (*z * factor).clamp(0.1, 8.0);
                    });
                }
                on:mousedown=move |ev: ev::MouseEvent| {
                    if ev.button() != 0 {
                        return;
                    }
                    dragging.set(true);
                    last_pos.set((ev.client_x() as f64, ev.client_y() as f64));
                }
                on:mousemove=move |ev: ev::MouseEvent| {
                    if !dragging.get_untracked() {
                        return;
                    }
                    let (lx, ly) = last_pos.get_untracked();
                    let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
                    pan.update(|(px, py)| {
                        *px += x - lx;
                        *py += y - ly;
                    });
                    last_pos.set((x, y));
                }
                on:mouseup=move |_| dragging.set(false)
                on:mouseleave=move |_| dragging.set(false)
            >
                {move || match state.viewed.get() {
                    Some(path) => {
                        let src = media_url(&path);
                        let checked_path = path.clone();
                        let change_path = path.clone();
                        let change_name = path
                            .rsplit('/')
                            .next()
                            .unwrap_or(path.as_str())
                            .to_string();
                        view! {
                            <label
                                class="img-check"
                                on:mousedown=move |ev| ev.stop_propagation()
                                on:click=move |ev| ev.stop_propagation()
                            >
                                <input
                                    type="checkbox"
                                    prop:checked=move || state.is_checked(&checked_path)
                                    on:change=move |ev| {
                                        state.set_image_checked(
                                            change_path.clone(),
                                            change_name.clone(),
                                            event_target_checked(&ev),
                                        );
                                    }
                                />
                                "选择"
                            </label>
                            <div class="stage-inner">
                                <Show when=move || !loaded.get() && !failed.get()>
                                    <div class="stage-msg">"加载图片…"</div>
                                </Show>
                                <Show when=move || failed.get()>
                                    <div class="stage-msg error">"无法加载图片"</div>
                                </Show>
                                <img
                                    src=src
                                    alt=path.clone()
                                    draggable="false"
                                    class=move || {
                                        if loaded.get() { "viewer-img is-ready" } else { "viewer-img" }
                                    }
                                    style=move || {
                                        let (x, y) = pan.get();
                                        format!(
                                            "transform: translate({x}px, {y}px) scale({}) rotate({}deg)",
                                            zoom.get(),
                                            rotate.get(),
                                        )
                                    }
                                    on:load=move |_| {
                                        loaded.set(true);
                                        failed.set(false);
                                    }
                                    on:error=move |_| {
                                        failed.set(true);
                                        loaded.set(false);
                                    }
                                />
                            </div>
                        }
                            .into_any()
                    }
                    None => {
                        view! {
                            <div class="stage-empty">
                                <p class="empty-title">"选择一张图片开始浏览"</p>
                                <p class="muted">"勾选「选择」后可在左侧高亮，并用第二排按钮批量操作"</p>
                            </div>
                        }
                            .into_any()
                    }
                }}
            </div>
            <MarkBar/>
            <Suspense fallback=|| ()>
                {move || {
                    gallery.get().and_then(|res| match res {
                        Ok(entries) if entries.is_empty() => None,
                        Ok(entries) => Some(
                            view! {
                                <div class="filmstrip-rail" class:panel-off=move || !state.show_thumbnails.get()>
                                    <div class="filmstrip">
                                        {entries
                                            .into_iter()
                                            .map(|entry| view! { <Thumb entry/> })
                                            .collect_view()}
                                    </div>
                                </div>
                            },
                        ),
                        Err(_) => None,
                    })
                }}
            </Suspense>
        </section>
    }.into_any()
}

#[component]
fn MarkBar() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let stars = RwSignal::new(0u8);
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0u64);
    let tags = RwSignal::new(String::new());
    let tag_tall = RwSignal::new(false);
    let tag_writing = RwSignal::new(false);
    let tag_pending = RwSignal::new(None::<(String, String)>);
    let tag_input = NodeRef::<html::Textarea>::new();

    let rating = Resource::new(
        move || (state.viewed.get(), reload.get()),
        |(path, _)| async move {
            match path {
                Some(p) => get_image_rating(p).await,
                None => Ok(0u8),
            }
        },
    );

    let tag_res = Resource::new(
        move || state.viewed.get(),
        |path| async move {
            match path {
                Some(p) => get_image_tags(p).await,
                None => Ok(String::new()),
            }
        },
    );

    Effect::new(move |_| match rating.get() {
        Some(Ok(v)) => stars.set(v),
        Some(Err(_)) => stars.set(0),
        None => {}
    });

    Effect::new(move |_| match (state.viewed.get(), tag_res.get()) {
        (None, _) => {
            tags.set(String::new());
            tag_tall.set(false);
        }
        (_, Some(Ok(v))) => {
            tags.set(v);
            if let Some(el) = tag_input.get() {
                tag_tall.set(el.scroll_height() > el.client_height() + 2);
            }
        }
        (_, Some(Err(_))) => {
            tags.set(String::new());
            tag_tall.set(false);
        }
        (_, None) => {
            tags.set(String::new());
            tag_tall.set(false);
        }
    });

    view! {
        <div class="mark-bar" class:panel-off=move || !state.show_stars.get()>
            <div class="mark-stars">
                <span class="mark-label">"星标"</span>
                <StarButton n=1 stars busy reload/>
                <StarButton n=2 stars busy reload/>
                <StarButton n=3 stars busy reload/>
                <StarButton n=4 stars busy reload/>
                <StarButton n=5 stars busy reload/>
            </div>
            <div class="mark-tags">
                <span class="mark-label">"tag"</span>
                <textarea
                    node_ref=tag_input
                    class="tag-input"
                    class:is-tall=move || tag_tall.get()
                    rows=move || if tag_tall.get() { 2 } else { 1 }
                    placeholder="用逗号分隔"
                    disabled=move || state.viewed.get().is_none()
                    prop:value=move || tags.get()
                    on:keydown=move |ev: ev::KeyboardEvent| {
                        if ev.key() == "Enter" {
                            ev.prevent_default();
                        }
                    }
                    on:input=move |ev| {
                        let mut value = event_target_value(&ev);
                        if value.contains('\n') {
                            value = value.replace(['\n', '\r'], "");
                        }
                        tags.set(value.clone());
                        if let Some(el) = tag_input.get() {
                            tag_tall.set(el.scroll_height() > el.client_height() + 2);
                        }
                        let Some(path) = state.viewed.get() else {
                            return;
                        };
                        if tag_writing.get() {
                            tag_pending.set(Some((path, value)));
                            return;
                        }
                        tag_writing.set(true);
                        leptos::task::spawn_local(async move {
                            let mut path = path;
                            let mut value = value;
                            loop {
                                if let Err(e) = set_image_tags(path, value).await {
                                    state.status.set(format!("tag 写入失败：{e}"));
                                }
                                match tag_pending.get_untracked() {
                                    Some((next_path, next_value)) => {
                                        tag_pending.set(None);
                                        path = next_path;
                                        value = next_value;
                                    }
                                    None => {
                                        tag_writing.set(false);
                                        break;
                                    }
                                }
                            }
                        });
                    }
                />
            </div>
        </div>
    }
}

#[component]
fn StarButton(
    n: u8,
    stars: RwSignal<u8>,
    busy: RwSignal<bool>,
    reload: RwSignal<u64>,
) -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    view! {
        <button
            type="button"
            class=move || {
                if stars.get() >= n {
                    "star-btn is-on"
                } else {
                    "star-btn"
                }
            }
            title=format!("{n} 星")
            disabled=move || state.viewed.get().is_none() || busy.get()
            on:click=move |_| {
                let Some(path) = state.viewed.get() else {
                    return;
                };
                if busy.get() {
                    return;
                }
                let next = if stars.get() == n { 0 } else { n };
                busy.set(true);
                stars.set(next);
                leptos::task::spawn_local(async move {
                    match set_image_rating(path, next).await {
                        Ok(_) => {
                            state.status.set(if next == 0 {
                                "已清除星标".into()
                            } else {
                                format!("已标记 {next} 星")
                            });
                        }
                        Err(e) => {
                            state.status.set(format!("星标写入失败：{e}"));
                            reload.update(|v| *v += 1);
                        }
                    }
                    busy.set(false);
                });
            }
        >
            "★"
        </button>
    }
}

#[component]
fn Thumb(entry: FsEntry) -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let path = entry.path.clone();
    let name = entry.name.clone();
    let src = media_url(&path);
    let path_active = path.clone();
    let path_checked_class = path.clone();
    let path_checked_box = path.clone();
    let path_change = path.clone();
    let name_change = name.clone();

    let open = {
        let path = path.clone();
        let name = name.clone();
        move |_| {
            state.viewed.set(Some(path.clone()));
            state.selected.set(Some(SelectedItem {
                path: path.clone(),
                name: name.clone(),
                is_dir: false,
                is_image: true,
            }));
        }
    };

    view! {
        <div
            class="thumb"
            class:is-active=move || state.viewed.get().as_deref() == Some(path_active.as_str())
            class:is-checked=move || state.is_checked(&path_checked_class)
            title=name.clone()
        >
            <label
                class="thumb-check"
                on:click=move |ev| ev.stop_propagation()
                on:mousedown=move |ev| ev.stop_propagation()
            >
                <input
                    type="checkbox"
                    prop:checked=move || state.is_checked(&path_checked_box)
                    on:change=move |ev| {
                        state.set_image_checked(
                            path_change.clone(),
                            name_change.clone(),
                            event_target_checked(&ev),
                        );
                    }
                />
            </label>
            <button class="thumb-btn" type="button" on:click=open>
                <img src=src alt=name.clone() draggable="false"/>
                <span class="thumb-name">{name.clone()}</span>
            </button>
        </div>
    }
}
