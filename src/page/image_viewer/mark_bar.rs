use crate::function::{
    batch_mark_images, get_image_rating, get_image_tags, set_image_rating, set_image_tags,
};
use crate::structure::ExplorerState;
use leptos::ev;
use leptos::html;
use leptos::prelude::*;

#[component]
pub(crate) fn MarkBar() -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let stars = RwSignal::new(0u8);
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0u64);
    let tags = RwSignal::new(String::new());
    let tag_tall = RwSignal::new(false);
    let tag_epoch = RwSignal::new(0u64);
    let tag_input = NodeRef::<html::Textarea>::new();

    let rating = Resource::new(
        move || {
            if state.is_text_mode() {
                return (None, 0u64);
            }
            (
                state.viewed.try_get().flatten(),
                reload.try_get().unwrap_or(0),
            )
        },
        |(path, _)| async move {
            match path {
                Some(p) => get_image_rating(p).await,
                None => Ok(0u8),
            }
        },
    );

    let tag_res = Resource::new(
        move || {
            if state.is_text_mode() {
                None
            } else {
                state.viewed.try_get().flatten()
            }
        },
        |path| async move {
            match path {
                Some(p) => get_image_tags(p).await,
                None => Ok(String::new()),
            }
        },
    );

    Effect::new(move |_| {
        if state.is_text_mode() {
            return;
        }
        match rating.try_get() {
            Some(Some(Ok(v))) => {
                let _ = stars.try_update(|s| *s = v);
            }
            Some(Some(Err(_))) => {
                let _ = stars.try_update(|s| *s = 0);
            }
            _ => {}
        }
    });

    Effect::new(move |_| {
        if state.is_text_mode() {
            return;
        }
        let Some(viewed) = state.viewed.try_get() else {
            return;
        };
        let Some(res) = tag_res.try_get() else {
            return;
        };
        match (viewed, res) {
            (None, _) | (_, None) | (_, Some(Err(_))) => {
                let _ = tags.try_update(|v| *v = String::new());
                let _ = tag_tall.try_update(|v| *v = false);
            }
            (_, Some(Ok(v))) => {
                let _ = tags.try_update(|t| *t = v);
                if let Some(el) = tag_input.get() {
                    let _ =
                        tag_tall.try_update(|t| *t = el.scroll_height() > el.client_height() + 2);
                }
            }
        }
    });

    view! {
        <div class="mark-bar" class:panel-off=move || !state.panels.stars.get()>
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
                        if ev.key() != "Enter" {
                            return;
                        }
                        ev.prevent_default();
                        let Some(path) = state.viewed.get() else {
                            return;
                        };
                        tag_epoch.update(|n| *n += 1);
                        let value = tags.get();
                        leptos::task::spawn_local(async move {
                            if let Err(e) = set_image_tags(path, value).await {
                                let _ = state
                                    .status
                                    .try_update(|s| *s = format!("tag 写入失败：{e}"));
                            }
                        });
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
                        tag_epoch.update(|n| *n += 1);
                        let my = tag_epoch.get_untracked();
                        leptos::task::spawn_local(async move {
                            #[cfg(target_arch = "wasm32")]
                            gloo_timers::future::TimeoutFuture::new(400).await;
                            let still_here = state.viewed.try_get_untracked().flatten().as_deref()
                                == Some(path.as_str());
                            if still_here && tag_epoch.try_get_untracked() != Some(my) {
                                return;
                            }
                            if let Err(e) = set_image_tags(path, value).await {
                                let _ = state
                                    .status
                                    .try_update(|s| *s = format!("tag 写入失败：{e}"));
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
                    !state.checked.with(|list| list.iter().any(|i| i.is_image))
                        || busy.get()
                        || state.busy.get()
                }
                on:click=move |_| {
                    if busy.get() || state.busy.get() {
                        return;
                    }
                    let items: Vec<_> = state
                        .checked
                        .with(|list| list.iter().filter(|i| i.is_image).cloned().collect());
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
                                    let _ = state.status.try_update(|s| *s = "请先勾选图片".into());
                                } else if report.failures.is_empty() {
                                    let _ = state
                                        .status
                                        .try_update(|s| *s = format!("已批量标记 {} 张", report.ok));
                                } else {
                                    let failed = report.failures.len();
                                    let _ = state.status.try_update(|s| {
                                        *s = format!(
                                            "已标记 {}/{} 张，失败 {failed}",
                                            report.ok, report.total
                                        )
                                    });
                                    state.report_failures("批量标记失败", report.failures);
                                }
                                let _ = reload.try_update(|v| *v += 1);
                            }
                            Err(e) => {
                                let _ = state
                                    .status
                                    .try_update(|s| *s = format!("批量标记失败：{e}"));
                            }
                        }
                        let _ = busy.try_update(|v| *v = false);
                        let _ = state.busy.try_update(|v| *v = false);
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
                            let _ = state.status.try_update(|s| {
                                *s = if next == 0 {
                                    "已清除星标".into()
                                } else {
                                    format!("已标记 {next} 星")
                                }
                            });
                        }
                        Err(e) => {
                            let _ = state
                                .status
                                .try_update(|s| *s = format!("星标写入失败：{e}"));
                            let _ = reload.try_update(|v| *v += 1);
                        }
                    }
                    let _ = busy.try_update(|v| *v = false);
                });
            }
        >
            "★"
        </button>
    }
}
