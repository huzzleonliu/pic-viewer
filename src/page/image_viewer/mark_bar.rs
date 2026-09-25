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
                                state.status.set(format!("tag 写入失败：{e}"));
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
                            let still_here =
                                state.viewed.get_untracked().as_deref() == Some(path.as_str());
                            if still_here && tag_epoch.get_untracked() != my {
                                return;
                            }
                            if let Err(e) = set_image_tags(path, value).await {
                                state.status.set(format!("tag 写入失败：{e}"));
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
