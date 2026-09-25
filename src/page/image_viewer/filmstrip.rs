use super::FILMSTRIP_PAGE;
use crate::function::thumb_url;
use crate::structure::{ExplorerState, FsEntry, SelectedItem};
use leptos::ev;
use leptos::html;
use leptos::prelude::*;

fn filmstrip_pages(n: usize) -> usize {
    if n == 0 {
        0
    } else {
        n.div_ceil(FILMSTRIP_PAGE)
    }
}

#[component]
pub(crate) fn FilmstripRail(
    paths: Vec<String>,
    thumb_page: RwSignal<usize>,
    rail_ref: NodeRef<html::Div>,
) -> impl IntoView {
    let paths = StoredValue::new(paths);

    view! {
        <div
            class="filmstrip-rail"
            node_ref=rail_ref
            on:wheel=move |ev: ev::WheelEvent| {
                let dx = if ev.delta_x().abs() > ev.delta_y().abs() {
                    ev.delta_x()
                } else {
                    ev.delta_y()
                };
                if dx == 0.0 {
                    return;
                }
                ev.prevent_default();
                if let Some(el) = rail_ref.get() {
                    el.set_scroll_left(((el.scroll_left() as f64) + dx).round() as i32);
                }
            }
        >
            {move || {
                let n = paths.with_value(|e| e.len());
                let pages = filmstrip_pages(n);
                let last = pages.saturating_sub(1);
                let page = thumb_page.get().min(last);
                let start = page * FILMSTRIP_PAGE;
                let end = (start + FILMSTRIP_PAGE).min(n);
                let slice = paths.with_value(|e| e[start..end].to_vec());
                let page_disp = page + 1;
                view! {
                    <div class="filmstrip">
                        {if page > 0 {
                            Some(view! {
                                <FilmstripPageBtn
                                    label="上一页"
                                    current=page_disp
                                    total=pages
                                    on_click=move || {
                                        thumb_page.update(|p| *p = p.saturating_sub(1));
                                    }
                                />
                            })
                        } else {
                            None
                        }}
                        {slice
                            .into_iter()
                            .map(|path| view! { <Thumb entry=FsEntry::image_from_path(path)/> })
                            .collect_view()}
                        {if pages > 1 && page < last {
                            Some(view! {
                                <FilmstripPageBtn
                                    label="下一页"
                                    current=page_disp
                                    total=pages
                                    on_click=move || {
                                        thumb_page.update(|p| *p = (*p + 1).min(last));
                                    }
                                />
                            })
                        } else {
                            None
                        }}
                    </div>
                }
            }}
        </div>
    }
}

#[component]
fn FilmstripPageBtn<F>(
    label: &'static str,
    current: usize,
    total: usize,
    on_click: F,
) -> impl IntoView
where
    F: Fn() + 'static + Copy,
{
    view! {
        <button
            type="button"
            class="filmstrip-page"
            title=label
            on:click=move |_| on_click()
        >
            <span class="filmstrip-page-label">{label}</span>
            <span class="filmstrip-page-num">{format!("{current}/{total}")}</span>
        </button>
    }
}

#[component]
fn Thumb(entry: FsEntry) -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let item = SelectedItem::from(&entry);
    let path = item.path.clone();
    let name = item.name.clone();
    let src = {
        let path = path.clone();
        move || format!("{}?v={}", thumb_url(&path), state.media_rev.get())
    };
    let path_active = path.clone();
    let path_checked_class = path.clone();
    let path_checked_box = path.clone();
    let item_check = item.clone();

    let open = move |_| {
        state.viewed.set(Some(item.path.clone()));
        state.selected.set(Some(item.clone()));
    };

    view! {
        <div
            class="thumb"
            class:is-active=move || state.viewed.get().as_deref() == Some(path_active.as_str())
            class:is-checked=move || state.is_checked(&path_checked_class)
            title=path.clone()
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
                        state.set_checked(item_check.clone(), event_target_checked(&ev));
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
