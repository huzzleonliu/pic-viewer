mod export_bar;
mod filmstrip;
mod filter_bar;
mod mark_bar;

use crate::function::{
    list_images, media_url, parent_path, preview_url, rel_name, save_rotated_image,
    start_meta_index,
};
use crate::structure::{ExplorerState, SelectedItem};
use export_bar::ExportBar;
use filmstrip::FilmstripRail;
use filter_bar::FilterBar;
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use mark_bar::MarkBar;
use std::collections::HashSet;

pub(crate) const FILMSTRIP_PAGE: usize = 100;

fn apply_path_filter(paths: Vec<String>, filter: Option<HashSet<String>>) -> Vec<String> {
    match filter {
        Some(set) => paths.into_iter().filter(|p| set.contains(p)).collect(),
        None => paths,
    }
}

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
    let view_original = RwSignal::new(false);
    let include_subdirs = RwSignal::new(false);
    let filmstrip_rail = NodeRef::<html::Div>::new();
    let thumb_page = RwSignal::new(0usize);

    Effect::new(move |_| {
        state.viewed.track();
        zoom.set(1.0);
        rotate.set(0);
        pan.set((0.0, 0.0));
        loaded.set(false);
        failed.set(false);
    });

    let gallery_dir = Memo::new(move |_| {
        if let Some(path) = state.viewed.get() {
            parent_path(&path)
        } else if let Some(s) = state.selected.get() {
            if s.is_dir {
                s.path
            } else {
                parent_path(&s.path)
            }
        } else {
            String::new()
        }
    });

    let gallery = Resource::new(
        move || {
            (
                gallery_dir.get(),
                state.refresh.get(),
                include_subdirs.get(),
            )
        },
        |(dir, epoch, recursive)| async move { list_images(dir, recursive, epoch).await },
    );

    Effect::new(move |_| {
        let dir = gallery_dir.get();
        let epoch = state.refresh.get();
        let recursive = include_subdirs.get();
        state.filter_paths.set(None);
        leptos::task::spawn_local(async move {
            if let Err(e) = start_meta_index(dir, epoch, recursive).await {
                state.status.set(format!("读取元数据失败：{e}"));
            }
        });
    });

    Effect::new(move |_| {
        gallery_dir.track();
        state.filter_paths.track();
        state.refresh.track();
        include_subdirs.track();
        thumb_page.set(0);
    });

    Effect::new(move |_| {
        let Some(current) = state.viewed.get() else {
            return;
        };
        let Some(Ok(list)) = gallery.get() else {
            return;
        };
        if list.truncated {
            state.status.set("图片列表已截断到 10000 张".into());
        }
        let shown = apply_path_filter(list.paths, state.filter_paths.get());
        let Some(idx) = shown.iter().position(|p| p == &current) else {
            return;
        };
        let page = idx / FILMSTRIP_PAGE;
        if thumb_page.get_untracked() != page {
            thumb_page.set(page);
        }
    });

    Effect::new(move |_| {
        thumb_page.track();
        if let Some(el) = filmstrip_rail.get() {
            el.set_scroll_left(0);
        }
    });

    let saving = RwSignal::new(false);

    let save_rotation = move |_| {
        let Some(path) = state.viewed.get() else {
            return;
        };
        let degrees = rotate.get().rem_euclid(360);
        if degrees == 0 || saving.get() {
            return;
        }
        saving.set(true);
        leptos::task::spawn_local(async move {
            match save_rotated_image(path, degrees).await {
                Ok(()) => {
                    rotate.set(0);
                    state.media_rev.update(|n| *n += 1);
                    state.status.set("已保存旋转".into());
                }
                Err(e) => state.status.set(format!("保存失败：{e}")),
            }
            saving.set(false);
        });
    };

    let go_relative = move |delta: isize| {
        let Some(current) = state.viewed.get() else {
            return;
        };
        let Some(Ok(list)) = gallery.get() else {
            return;
        };
        let shown = apply_path_filter(list.paths, state.filter_paths.get());
        let Some(idx) = shown.iter().position(|p| p == &current) else {
            return;
        };
        let next = idx as isize + delta;
        if next >= 0 && (next as usize) < shown.len() {
            let path = shown[next as usize].clone();
            state.viewed.set(Some(path.clone()));
            state
                .selected
                .set(Some(SelectedItem::from_image_path(path)));
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
        <section class="viewer" class:panel-off=move || state.is_text_mode()>
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
                <button
                    class="btn"
                    title="把当前旋转写入文件"
                    disabled=move || {
                        state.viewed.get().is_none()
                            || saving.get()
                            || rotate.get().rem_euclid(360) == 0
                    }
                    on:click=save_rotation
                >
                    "保存"
                </button>
                <button class="btn" on:click=move |_| go_relative(-1) title="上一张">"‹"</button>
                <button class="btn" on:click=move |_| go_relative(1) title="下一张">"›"</button>
                <span class="viewer-name">
                    {move || {
                        state
                            .viewed
                            .get()
                            .map(|p| rel_name(&p).to_string())
                            .unwrap_or_else(|| "未选择图片".into())
                    }}
                </span>
                <label class="orig-check" title="勾选后加载本地原图；不勾选则由 imgproxy 压缩预览">
                    <input
                        type="checkbox"
                        prop:checked=move || view_original.get()
                        on:change=move |ev| {
                            view_original.set(event_target_checked(&ev));
                            loaded.set(false);
                            failed.set(false);
                        }
                    />
                    "原图"
                </label>
                <label
                    class="orig-check"
                    title="勾选后缩略图与筛选包含当前目录下所有子目录中的图片"
                >
                    <input
                        type="checkbox"
                        prop:checked=move || include_subdirs.get()
                        on:change=move |ev| {
                            include_subdirs.set(event_target_checked(&ev));
                        }
                    />
                    "包括子目录"
                </label>
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
                        let src = {
                            let path = path.clone();
                            move || {
                                let url = if view_original.get() {
                                    media_url(&path)
                                } else {
                                    preview_url(&path)
                                };
                                format!("{}?v={}", url, state.media_rev.get())
                            }
                        };
                        let checked_path = path.clone();
                        let change_item = SelectedItem::from_image_path(path.clone());
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
                                        state.set_checked(
                                            change_item.clone(),
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
                            <button
                                type="button"
                                class="stage-nav stage-nav-prev"
                                title="上一张"
                                aria-label="上一张"
                                on:mousedown=move |ev| ev.stop_propagation()
                                on:click=move |ev| {
                                    ev.stop_propagation();
                                    go_relative(-1);
                                }
                            ></button>
                            <button
                                type="button"
                                class="stage-nav stage-nav-next"
                                title="下一张"
                                aria-label="下一张"
                                on:mousedown=move |ev| ev.stop_propagation()
                                on:click=move |ev| {
                                    ev.stop_propagation();
                                    go_relative(1);
                                }
                            ></button>
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
            <FilterBar dir=gallery_dir/>
            <MarkBar/>
            <ExportBar/>
            <Suspense fallback=|| ()>
                {move || {
                    gallery.get().and_then(|res| match res {
                        Ok(list) if list.paths.is_empty() => None,
                        Ok(list) => {
                            let shown = apply_path_filter(list.paths, state.filter_paths.get());
                            if shown.is_empty() {
                                None
                            } else {
                                Some(
                                    view! {
                                        <Show when=move || state.show_thumbnails.get()>
                                            <FilmstripRail
                                                paths=shown.clone()
                                                thumb_page=thumb_page
                                                rail_ref=filmstrip_rail
                                            />
                                        </Show>
                                    }
                                    .into_any(),
                                )
                            }
                        }
                        Err(_) => None,
                    })
                }}
            </Suspense>
        </section>
    }.into_any()
}
