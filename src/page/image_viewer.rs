use crate::function::{
    apply_meta_filter, batch_mark_images, export_checked, get_image_rating, get_image_tags,
    get_meta_index_status, list_dir, media_url, parent_path, preview_url, save_rotated_image, set_image_rating,
    set_image_tags, start_meta_index, thumb_url,
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
    let view_original = RwSignal::new(false);

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
        move || (gallery_dir.get(), state.refresh.get()),
        |(dir, _)| async move {
            let entries = list_dir(dir).await?;
            Ok::<Vec<FsEntry>, ServerFnError>(
                entries.into_iter().filter(|e| e.is_image).collect(),
            )
        },
    );

    Effect::new(move |_| {
        let dir = gallery_dir.get();
        let epoch = state.refresh.get();
        state.filter_paths.set(None);
        leptos::task::spawn_local(async move {
            if let Err(e) = start_meta_index(dir, epoch).await {
                state.status.set(format!("读取元数据失败：{e}"));
            }
        });
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
                is_text: false,
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
                            .map(|p| p.rsplit('/').next().unwrap_or(&p).to_string())
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
            <FilterBar dir=gallery_dir/>
            <MarkBar/>
            <ExportBar/>
            <Suspense fallback=|| ()>
                {move || {
                    gallery.get().and_then(|res| match res {
                        Ok(entries) if entries.is_empty() => None,
                        Ok(entries) => {
                            let shown = match state.filter_paths.get() {
                                Some(paths) => entries
                                    .into_iter()
                                    .filter(|e| paths.iter().any(|p| p == &e.path))
                                    .collect::<Vec<_>>(),
                                None => entries,
                            };
                            Some({
                                let shown = shown.clone();
                                view! {
                                    <Show when=move || state.show_thumbnails.get()>
                                        <div class="filmstrip-rail">
                                            <div class="filmstrip">
                                                {shown
                                                    .clone()
                                                    .into_iter()
                                                    .map(|entry| view! { <Thumb entry/> })
                                                    .collect_view()}
                                            </div>
                                        </div>
                                    </Show>
                                }
                            })
                        }
                        Err(_) => None,
                    })
                }}
            </Suspense>
        </section>
    }.into_any()
}

fn start_download(url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.location().set_href(url);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
    }
}

#[component]
fn ExportBar() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let dest = RwSignal::new("download".to_string());
    let dest_dir = RwSignal::new(String::new());
    let format = RwSignal::new("jpeg".to_string());
    let exporting = RwSignal::new(false);

    view! {
        <div class="export-bar" class:panel-off=move || !state.show_export.get()>
            <span class="mark-label">"导出勾选"</span>
            <span class="mark-label">"导出到"</span>
            <select
                class="filter-select"
                prop:value=move || dest.get()
                on:change=move |ev| dest.set(event_target_value(&ev))
            >
                <option value="download" selected>"下载到本机"</option>
                <option value="current">"当前目录"</option>
                <option value="dir">"特定目录"</option>
            </select>
            <Show when=move || dest.get() == "dir">
                <input
                    class="filter-tag-input export-dir-input"
                    type="text"
                    placeholder="相对托管目录，如 export/out"
                    prop:value=move || dest_dir.get()
                    on:input=move |ev| dest_dir.set(event_target_value(&ev))
                />
            </Show>
            <span class="mark-label">"导出格式"</span>
            <select
                class="filter-select"
                prop:value=move || format.get()
                on:change=move |ev| format.set(event_target_value(&ev))
            >
                <option value="jpeg" selected>"JPEG"</option>
                <option value="png">"PNG"</option>
                <option value="webp">"WebP"</option>
                <option value="gif">"GIF"</option>
                <option value="bmp">"BMP"</option>
                <option value="tiff">"TIFF"</option>
            </select>
            <button
                class="btn filter-apply"
                type="button"
                title="导出已勾选图片"
                disabled=move || {
                    !state.checked.get().iter().any(|i| i.is_image)
                        || exporting.get()
                        || state.busy.get()
                        || (dest.get() == "dir" && dest_dir.get().trim().is_empty())
                }
                on:click=move |_| {
                    if exporting.get() || state.busy.get() {
                        return;
                    }
                    let items: Vec<_> = state
                        .checked
                        .get()
                        .into_iter()
                        .filter(|i| i.is_image)
                        .collect();
                    if items.is_empty() {
                        state.status.set("请先勾选图片".into());
                        return;
                    }
                    let dest_mode = dest.get();
                    let dest_dir_val = if dest_mode == "current" {
                        state.current_dir_rel()
                    } else {
                        dest_dir.get()
                    };
                    if dest_mode == "dir" && dest_dir_val.trim().is_empty() {
                        state.status.set("请填写导出目录".into());
                        return;
                    }
                    let dest_api = if dest_mode == "download" {
                        "download".to_string()
                    } else {
                        "dir".to_string()
                    };
                    let format = format.get();
                    let n = items.len();
                    let paths: Vec<String> = items.into_iter().map(|i| i.path).collect();
                    exporting.set(true);
                    state.busy.set(true);
                    state.status.set(format!("正在导出 {n} 张…"));
                    leptos::task::spawn_local(async move {
                        match export_checked(paths, dest_api.clone(), dest_dir_val, format).await
                        {
                            Ok(report) => {
                                if report.total == 0 {
                                    state.status.set("请先勾选图片".into());
                                } else if report.failures.is_empty() {
                                    state.status.set(format!("已导出 {} 张", report.ok));
                                } else {
                                    let failed = report.failures.len();
                                    state.status.set(format!(
                                        "已导出 {}/{} 张，失败 {failed}",
                                        report.ok, report.total
                                    ));
                                    state.report_failures("导出失败", report.failures);
                                }
                                if let Some(url) = report.download_url {
                                    start_download(&url);
                                }
                                if dest_api == "dir" && report.ok > 0 {
                                    state.refresh.update(|v| *v += 1);
                                }
                            }
                            Err(e) => state.status.set(format!("导出失败：{e}")),
                        }
                        exporting.set(false);
                        state.busy.set(false);
                    });
                }
            >
                "导出"
            </button>
        </div>
    }
}

#[component]
fn FilterBar(dir: Memo<String>) -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let star_op = RwSignal::new("any".to_string());
    let star_val = RwSignal::new("0".to_string());
    let tag_mode = RwSignal::new("contains".to_string());
    let tag_query = RwSignal::new(String::new());
    let complement = RwSignal::new(false);
    let applying = RwSignal::new(false);
    let ready = RwSignal::new(false);
    let progress = RwSignal::new((0u32, 0u32));
    let poll_gen = RwSignal::new(0u64);

    Effect::new(move |_| {
        let dir = dir.get();
        ready.set(false);
        progress.set((0, 0));
        poll_gen.update(|n| *n += 1);
        let my = poll_gen.get_untracked();
        leptos::task::spawn_local(async move {
            loop {
                if poll_gen.get_untracked() != my {
                    return;
                }
                match get_meta_index_status(dir.clone()).await {
                    Ok(s) => {
                        progress.set((s.done, s.total));
                        if s.ready {
                            ready.set(true);
                            return;
                        }
                    }
                    Err(_) => {}
                }
                #[cfg(target_arch = "wasm32")]
                gloo_timers::future::TimeoutFuture::new(250).await;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    return;
                }
            }
        });
    });

    view! {
        <div class="filter-bar" class:panel-off=move || !state.show_filter.get()>
            <span class="mark-label">"星标"</span>
            <select
                class="filter-select"
                prop:value=move || star_op.get()
                on:change=move |ev| star_op.set(event_target_value(&ev))
            >
                <option value="any" selected>"不限"</option>
                <option value="lt">"<"</option>
                <option value="le">"<="</option>
                <option value="eq">"="</option>
                <option value="ge">">="</option>
                <option value="gt">">"</option>
            </select>
            <select
                class="filter-select"
                prop:value=move || star_val.get()
                disabled=move || star_op.get() == "any"
                on:change=move |ev| star_val.set(event_target_value(&ev))
            >
                <option value="0" selected>"0"</option>
                <option value="1">"1"</option>
                <option value="2">"2"</option>
                <option value="3">"3"</option>
                <option value="4">"4"</option>
                <option value="5">"5"</option>
            </select>
            <span class="mark-label">"tag"</span>
            <select
                class="filter-select"
                prop:value=move || tag_mode.get()
                on:change=move |ev| tag_mode.set(event_target_value(&ev))
            >
                <option value="eq">"全等"</option>
                <option value="contains" selected>"包含"</option>
                <option value="excludes">"不包含"</option>
            </select>
            <input
                class="filter-tag-input"
                type="text"
                placeholder="筛选 tag"
                prop:value=move || tag_query.get()
                on:input=move |ev| tag_query.set(event_target_value(&ev))
            />
            <label class="filter-check">
                <input
                    type="checkbox"
                    prop:checked=move || complement.get()
                    on:change=move |ev| complement.set(event_target_checked(&ev))
                />
                "补集"
            </label>
            <div class="filter-actions">
                <button
                    class="btn filter-apply"
                    type="button"
                    disabled=move || !ready.get() || applying.get()
                    title=move || {
                        if ready.get() {
                            "应用筛选".into()
                        } else {
                            let (done, total) = progress.get();
                            if total == 0 {
                                "读取元数据…".into()
                            } else {
                                format!("读取元数据 {done}/{total}")
                            }
                        }
                    }
                    on:click=move |_| {
                        if !ready.get() || applying.get() {
                            return;
                        }
                        let dir = dir.get();
                        let star_op = star_op.get();
                        let star_val = star_val.get().parse::<u8>().unwrap_or(0);
                        let tag_mode = tag_mode.get();
                        let tag_query = tag_query.get();
                        let complement = complement.get();
                        applying.set(true);
                        leptos::task::spawn_local(async move {
                            match apply_meta_filter(
                                dir,
                                star_op,
                                star_val,
                                tag_mode,
                                tag_query,
                                complement,
                            )
                            .await
                            {
                                Ok(paths) => {
                                    let n = paths.len();
                                    state.filter_paths.set(Some(paths));
                                    state.status.set(format!("已筛选 {n} 张"));
                                }
                                Err(e) => state.status.set(format!("筛选失败：{e}")),
                            }
                            applying.set(false);
                        });
                    }
                >
                    "确认"
                </button>
                <button
                    class="btn"
                    type="button"
                    title="去掉所有筛选条件"
                    on:click=move |_| {
                        star_op.set("any".into());
                        star_val.set("0".into());
                        tag_mode.set("contains".into());
                        tag_query.set(String::new());
                        complement.set(false);
                        state.filter_paths.set(None);
                        state.status.set("已重置筛选".into());
                    }
                >
                    "重置"
                </button>
            </div>
        </div>
    }
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
            <button
                class="btn mark-batch"
                type="button"
                title="把当前星标写入勾选图片，并把当前 tag 追加到各文件（文件里已有的不重复）"
                disabled=move || {
                    !state.checked.get().iter().any(|i| i.is_image)
                        || busy.get()
                        || state.busy.get()
                }
                on:click=move |_| {
                    if busy.get() || state.busy.get() {
                        return;
                    }
                    let items: Vec<_> = state
                        .checked
                        .get()
                        .into_iter()
                        .filter(|i| i.is_image)
                        .collect();
                    if items.is_empty() {
                        state.status.set("请先勾选图片".into());
                        return;
                    }
                    let rating = stars.get();
                    let tags = tags.get();
                    let n = items.len();
                    let paths: Vec<String> = items.into_iter().map(|i| i.path).collect();
                    busy.set(true);
                    state.busy.set(true);
                    state.status.set(format!("正在批量标记 {n} 张…"));
                    leptos::task::spawn_local(async move {
                        match batch_mark_images(paths, rating, tags).await {
                            Ok(report) => {
                                if report.total == 0 {
                                    state.status.set("请先勾选图片".into());
                                } else if report.failures.is_empty() {
                                    state.status.set(format!("已批量标记 {} 张", report.ok));
                                } else {
                                    let failed = report.failures.len();
                                    state.status.set(format!(
                                        "已标记 {}/{} 张，失败 {failed}",
                                        report.ok, report.total
                                    ));
                                    state.report_failures("批量标记失败", report.failures);
                                }
                                reload.update(|v| *v += 1);
                            }
                            Err(e) => state.status.set(format!("批量标记失败：{e}")),
                        }
                        busy.set(false);
                        state.busy.set(false);
                    });
                }
            >
                "批量标记"
            </button>
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
    let src = {
        let path = path.clone();
        move || format!("{}?v={}", thumb_url(&path), state.media_rev.get())
    };
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
                is_text: false,
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
                <img src=src alt=name.clone() draggable="false" loading="lazy" decoding="async"/>
                <span class="thumb-name">{name.clone()}</span>
            </button>
        </div>
    }
}
