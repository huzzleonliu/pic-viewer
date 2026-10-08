mod crop_overlay;
mod export_bar;
mod filmstrip;
mod filter_bar;
mod mark_bar;

#[cfg(feature = "hydrate")]
use crate::function::crop::contain_content_rect;
use crate::function::crop::{fitted_norm, CropRect};
use crate::function::explorer::gallery_dir_from;
use crate::function::{
    list_images, media_url, preview_url, rel_name, save_cropped_image, save_rotated_image,
    start_meta_index, with_file_rev, Nav,
};
use crate::structure::{ExplorerState, ImageList, ImageRef, SelectedItem};
use crop_overlay::CropOverlay;
use export_bar::ExportBar;
use filmstrip::FilmstripRail;
use filter_bar::FilterBar;
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use mark_bar::MarkBar;
use std::collections::HashSet;

pub(crate) const FILMSTRIP_PAGE: usize = 100;

fn apply_path_filter(items: Vec<ImageRef>, filter: Option<HashSet<String>>) -> Vec<ImageRef> {
    match filter {
        Some(set) => items
            .into_iter()
            .filter(|item| set.contains(&item.path))
            .collect(),
        None => items,
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
    let filmstrip_rail = NodeRef::<html::Div>::new();
    let thumb_page = RwSignal::new(0usize);
    let crop_ratio = RwSignal::new(None::<(u32, u32)>);
    let crop_norm = RwSignal::new(None::<CropRect>);
    let img_ref = NodeRef::<html::Img>::new();
    let stage_ref = NodeRef::<html::Div>::new();
    let host_ref = NodeRef::<html::Div>::new();
    let crop_aspect = Signal::derive(move || crop_ratio.get().unwrap_or((1, 1)));
    let paused = move || state.is_text_mode();

    Effect::new(move |_| {
        let _ = state.viewed.try_get();
        if state
            .selected
            .try_with_untracked(|s| s.as_ref().is_some_and(|item| item.is_text))
            .unwrap_or(false)
        {
            return;
        }
        if zoom.try_get_untracked() != Some(1.0) {
            let _ = zoom.try_update(|v| *v = 1.0);
        }
        if rotate.try_get_untracked() != Some(0) {
            let _ = rotate.try_update(|v| *v = 0);
        }
        if pan.try_get_untracked() != Some((0.0, 0.0)) {
            let _ = pan.try_update(|v| *v = (0.0, 0.0));
        }
        if loaded.try_get_untracked() != Some(false) {
            let _ = loaded.try_update(|v| *v = false);
        }
        if failed.try_get_untracked() != Some(false) {
            let _ = failed.try_update(|v| *v = false);
        }
        if crop_ratio.try_get_untracked().flatten().is_some() {
            let _ = crop_ratio.try_update(|v| *v = None);
        }
        if crop_norm.try_get_untracked().flatten().is_some() {
            let _ = crop_norm.try_update(|v| *v = None);
        }
    });

    let gallery_dir = Memo::new(move |_| {
        let browse = state.browse_dir.get();
        if let Some(dir) = browse.as_deref() {
            return dir.to_string();
        }
        gallery_dir_from(
            None,
            state.selected.get().as_ref(),
            state.viewed.get().as_deref(),
        )
    });

    let gallery = Resource::new(
        move || {
            if paused() {
                return (false, String::new(), 0u64, false);
            }
            (
                true,
                gallery_dir.try_get().unwrap_or_default(),
                state.gallery_epoch.try_get().unwrap_or(0),
                state.include_subdirs.try_get().unwrap_or(false),
            )
        },
        |(active, dir, epoch, recursive)| async move {
            if !active {
                return Ok(ImageList {
                    truncated: false,
                    items: Vec::new(),
                });
            }
            list_images(dir, recursive, epoch).await
        },
    );

    Effect::new(move |_| {
        if paused() || !state.panels.filter.try_get().unwrap_or(false) {
            return;
        }
        let dir = gallery_dir.try_get().unwrap_or_default();
        let epoch = state.gallery_epoch.try_get().unwrap_or(0);
        let recursive = state.include_subdirs.try_get().unwrap_or(false);
        leptos::task::spawn_local(async move {
            if let Err(e) = start_meta_index(dir, epoch, recursive).await {
                let _ = state
                    .status
                    .try_update(|s| *s = format!("读取元数据失败：{e}"));
            }
        });
    });

    Effect::new(move |_| {
        if paused() {
            return;
        }
        let _ = gallery_dir.try_get();
        let _ = state.gallery_epoch.try_get();
        let _ = state.include_subdirs.try_get();
        if state.filter_paths.try_get_untracked().flatten().is_some() {
            let _ = state.filter_paths.try_update(|v| *v = None);
        }
    });

    Effect::new(move |_| {
        if paused() {
            return;
        }
        let _ = gallery_dir.try_get();
        let _ = state.filter_paths.try_get();
        let _ = state.gallery_epoch.try_get();
        let _ = state.include_subdirs.try_get();
        if thumb_page.try_get_untracked() != Some(0) {
            let _ = thumb_page.try_update(|v| *v = 0);
        }
    });

    Effect::new(move |_| {
        if paused() {
            return;
        }
        let Some(dir) = state.browse_dir.try_get().flatten() else {
            return;
        };
        let Some(Ok(list)) = gallery.try_get().flatten() else {
            return;
        };
        let shown = apply_path_filter(list.items, state.filter_paths.try_get().flatten());
        state.navigate(Nav::ApplyGallery {
            dir,
            shown: shown.into_iter().map(|item| item.path).collect(),
        });
    });

    Effect::new(move |_| {
        if paused() {
            return;
        }
        let Some(current) = state.viewed.try_get().flatten() else {
            return;
        };
        let Some(Ok(list)) = gallery.try_get().flatten() else {
            return;
        };
        if list.truncated {
            const MSG: &str = "图片列表已截断到 10000 张";
            if state.status.try_get_untracked().as_deref() != Some(MSG) {
                let _ = state.status.try_update(|s| *s = MSG.into());
            }
        }
        let shown = apply_path_filter(list.items, state.filter_paths.try_get().flatten());
        let Some(idx) = shown.iter().position(|item| item.path == current) else {
            return;
        };
        let page = idx / FILMSTRIP_PAGE;
        if thumb_page.try_get_untracked() != Some(page) {
            let _ = thumb_page.try_update(|v| *v = page);
        }
    });

    Effect::new(move |_| {
        let _ = thumb_page.try_get();
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
                    let _ = rotate.try_update(|v| *v = 0);
                    let _ = state.media_rev.try_update(|n| *n += 1);
                    let _ = state.status.try_update(|s| *s = "已保存旋转".into());
                }
                Err(e) => {
                    let _ = state.status.try_update(|s| *s = format!("保存失败：{e}"));
                }
            }
            let _ = saving.try_update(|v| *v = false);
        });
    };

    Effect::new(move |_| {
        if !state.panels.crop.try_get().unwrap_or(false) {
            if crop_ratio.try_get_untracked().flatten().is_some() {
                let _ = crop_ratio.try_update(|v| *v = None);
            }
            if crop_norm.try_get_untracked().flatten().is_some() {
                let _ = crop_norm.try_update(|v| *v = None);
            }
        }
    });

    Effect::new(move |_| {
        let Some((rw, rh)) = crop_ratio.try_get().flatten() else {
            if crop_norm.try_get_untracked().flatten().is_some() {
                let _ = crop_norm.try_update(|v| *v = None);
            }
            return;
        };
        if !loaded.try_get().unwrap_or(false) || failed.try_get().unwrap_or(true) {
            return;
        }
        let apply = move || {
            if crop_ratio.try_get_untracked().flatten() != Some((rw, rh)) {
                return;
            }
            if let Some(next) = measure_initial_crop(host_ref, img_ref, rw, rh) {
                let _ = crop_norm.try_update(|v| *v = Some(next));
            }
        };
        apply();
        #[cfg(feature = "hydrate")]
        {
            use gloo_timers::callback::Timeout;
            Timeout::new(0, apply).forget();
        }
    });

    let begin_crop = move |rw: u32, rh: u32| {
        zoom.set(1.0);
        pan.set((0.0, 0.0));
        rotate.set(0);
        crop_ratio.set(Some((rw, rh)));
    };

    let save_crop = move || {
        let Some(path) = state.viewed.get() else {
            return;
        };
        let Some(r) = crop_norm.get() else {
            return;
        };
        if crop_ratio.get().is_none() || saving.get() {
            return;
        }
        let (x, y, w, h) = (r.x, r.y, r.w, r.h);
        saving.set(true);
        leptos::task::spawn_local(async move {
            match save_cropped_image(path.clone(), x, y, w, h).await {
                Ok(new_path) => {
                    if new_path != path {
                        state.retarget_path(&path, &new_path);
                    }
                    let _ = crop_ratio.try_update(|v| *v = None);
                    let _ = crop_norm.try_update(|v| *v = None);
                    state.bump_listings();
                    let _ = state.media_rev.try_update(|n| *n += 1);
                    let _ = state.status.try_update(|s| *s = "已保存裁切".into());
                }
                Err(e) => {
                    let _ = state
                        .status
                        .try_update(|s| *s = format!("裁切保存失败：{e}"));
                }
            }
            let _ = saving.try_update(|v| *v = false);
        });
    };

    let go_relative = move |delta: isize| {
        let Some(current) = state.viewed.get() else {
            return;
        };
        let Some(Ok(list)) = gallery.get() else {
            return;
        };
        let shown = apply_path_filter(list.items, state.filter_paths.get());
        let Some(idx) = shown.iter().position(|item| item.path == current) else {
            return;
        };
        let next = idx as isize + delta;
        if next >= 0 && (next as usize) < shown.len() {
            state.navigate(Nav::OpenImage(shown[next as usize].path.clone()));
        }
    };

    let apply_zoom = move |factor: f64| {
        apply_viewer_zoom(zoom, factor);
    };

    let zoom_by = move |factor: f64| {
        apply_zoom(factor);
    };

    let reset = move |_| {
        if crop_ratio.get().is_some() {
            return;
        }
        zoom.set(1.0);
        rotate.set(0);
        pan.set((0.0, 0.0));
    };

    view! {
        <section class="viewer" class:panel-off=move || state.is_text_mode()>
            <div class="viewer-toolbar" class:panel-off=move || !state.panels.crop.get()>
                <button
                    class="btn"
                    class:is-active=move || crop_ratio.get() == Some((3, 4))
                    disabled=move || state.viewed.get().is_none()
                    title="竖图 3:4"
                    on:click=move |_| begin_crop(3, 4)
                >
                    "3:4"
                </button>
                <button
                    class="btn"
                    class:is-active=move || crop_ratio.get() == Some((1, 1))
                    disabled=move || state.viewed.get().is_none()
                    title="正方形 1:1"
                    on:click=move |_| begin_crop(1, 1)
                >
                    "1:1"
                </button>
                <button
                    class="btn"
                    class:is-active=move || crop_ratio.get() == Some((4, 3))
                    disabled=move || state.viewed.get().is_none()
                    title="横图 4:3"
                    on:click=move |_| begin_crop(4, 3)
                >
                    "4:3"
                </button>
                <button
                    class="btn"
                    title="取消裁切框"
                    disabled=move || crop_ratio.get().is_none()
                    on:click=move |_| {
                        crop_ratio.set(None);
                        crop_norm.set(None);
                    }
                >
                    "重置"
                </button>
                <button
                    class="btn"
                    title="裁切后保存为 WebP；原文件已是 WebP 则覆盖"
                    disabled=move || {
                        state.viewed.get().is_none()
                            || crop_norm.get().is_none()
                            || saving.get()
                    }
                    on:click=move |_| save_crop()
                >
                    "保存"
                </button>
            </div>
            <div class="viewer-toolbar" class:panel-off=move || !state.panels.adjust.get()>
                <button class="btn" on:click=move |_| zoom_by(1.0 / 1.2) title="缩小">"−"</button>
                <span class="zoom-label">{move || format!("{}%", (zoom.get() * 100.0).round())}</span>
                <button class="btn" on:click=move |_| zoom_by(1.2) title="放大">"+"</button>
                <button
                    class="btn"
                    disabled=move || crop_ratio.get().is_some()
                    on:click=move |_| rotate.update(|r| *r = (*r - 90).rem_euclid(360))
                    title="左转"
                >
                    "↺"
                </button>
                <button
                    class="btn"
                    disabled=move || crop_ratio.get().is_some()
                    on:click=move |_| rotate.update(|r| *r = (*r + 90) % 360)
                    title="右转"
                >
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
                        prop:checked=move || state.include_subdirs.get()
                        on:change=move |ev| {
                            let _ = state
                                .include_subdirs
                                .try_update(|v| *v = event_target_checked(&ev));
                        }
                    />
                    "包括子目录"
                </label>
            </div>
            <div
                class="stage"
                node_ref=stage_ref
                class:is-dragging=move || dragging.get()
                class:is-picked=move || {
                    state.viewed.get().is_some_and(|p| state.is_checked(&p))
                }
                class:is-cropping=move || crop_ratio.get().is_some()
                on:wheel=move |ev: ev::WheelEvent| {
                    ev.prevent_default();
                    let factor = if ev.delta_y() < 0.0 { 1.12 } else { 1.0 / 1.12 };
                    apply_zoom(factor);
                }
                on:mousedown=move |ev: ev::MouseEvent| {
                    if crop_ratio.get().is_some() || ev.button() != 0 {
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
                                let mtime = gallery
                                    .get()
                                    .and_then(|res| res.ok())
                                    .and_then(|list| {
                                        list.items
                                            .iter()
                                            .find(|item| item.path == path)
                                            .map(|item| item.mtime)
                                    })
                                    .unwrap_or(0);
                                with_file_rev(url, mtime, state.media_rev.get())
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
                                <div class="crop-anchor">
                                    <div
                                        class="crop-host"
                                        node_ref=host_ref
                                        style=move || {
                                            let (x, y) = pan.get();
                                            format!(
                                                "transform: translate({x}px, {y}px) scale({}) rotate({}deg)",
                                                zoom.get(),
                                                rotate.get(),
                                            )
                                        }
                                    >
                                        <img
                                            node_ref=img_ref
                                            src=src
                                            alt=path.clone()
                                            draggable="false"
                                            class=move || {
                                                if loaded.get() {
                                                    "viewer-img is-ready"
                                                } else {
                                                    "viewer-img"
                                                }
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
                                </div>
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
                            <Show when=move || crop_ratio.get().is_some()>
                                <CropOverlay
                                    stage_ref=stage_ref
                                    host_ref=host_ref
                                    img_ref=img_ref
                                    norm=crop_norm
                                    ratio=crop_aspect
                                    zoom=zoom
                                    pan=pan
                                />
                            </Show>
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
                        Ok(list) if list.items.is_empty() => None,
                        Ok(list) => {
                            let shown = apply_path_filter(list.items, state.filter_paths.get());
                            if shown.is_empty() {
                                None
                            } else {
                                Some(
                                    view! {
                                        <Show when=move || state.panels.thumbnails.get()>
                                            <FilmstripRail
                                                items=shown.clone()
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

pub(crate) fn apply_viewer_zoom(zoom: RwSignal<f64>, factor: f64) {
    let Some(old) = zoom.try_get() else {
        return;
    };
    let new_z = (old * factor).clamp(0.1, 8.0);
    if old <= 0.0 || (new_z - old).abs() < 1e-12 {
        return;
    }
    zoom.set(new_z);
}

fn measure_initial_crop(
    host_ref: NodeRef<html::Div>,
    img_ref: NodeRef<html::Img>,
    ratio_w: u32,
    ratio_h: u32,
) -> Option<CropRect> {
    let (w, h) = host_layout_size(host_ref, img_ref)?;
    Some(fitted_norm(w, h, ratio_w as f64, ratio_h as f64))
}

fn host_layout_size(
    host_ref: NodeRef<html::Div>,
    img_ref: NodeRef<html::Img>,
) -> Option<(f64, f64)> {
    #[cfg(feature = "hydrate")]
    {
        let host = host_ref.get()?;
        let w = f64::from(host.offset_width());
        let h = f64::from(host.offset_height());
        if w < 1.0 || h < 1.0 {
            return None;
        }
        let (nw, nh) = img_ref
            .get()
            .map(|img| {
                (
                    f64::from(img.natural_width()),
                    f64::from(img.natural_height()),
                )
            })
            .unwrap_or((0.0, 0.0));
        let r = contain_content_rect(w, h, nw, nh);
        if r.w >= 1.0 && r.h >= 1.0 {
            Some((r.w, r.h))
        } else {
            Some((w, h))
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (host_ref, img_ref);
        None
    }
}
