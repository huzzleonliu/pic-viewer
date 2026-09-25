use crate::function::{apply_meta_filter, get_meta_index_status};
use crate::structure::ExplorerState;
use leptos::prelude::*;

#[component]
pub(crate) fn FilterBar(dir: Memo<String>) -> impl IntoView {
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
        if !state.show_filter.get() {
            return;
        }
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
                gloo_timers::future::TimeoutFuture::new(500).await;
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
                                    state.filter_paths.set(Some(paths.into_iter().collect()));
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
