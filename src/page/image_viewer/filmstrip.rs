use super::FILMSTRIP_PAGE;
use crate::function::{thumb_url, with_file_rev, Nav};
use crate::structure::{ExplorerState, ImageRef, SelectedItem};
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
    items: Vec<ImageRef>,
    thumb_page: RwSignal<usize>,
    rail_ref: NodeRef<html::Div>,
) -> impl IntoView {
    let items = StoredValue::new(items);

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
                let n = items.with_value(|e| e.len());
                let pages = filmstrip_pages(n);
                let last = pages.saturating_sub(1);
                let page = thumb_page.get().min(last);
                let start = page * FILMSTRIP_PAGE;
                let end = (start + FILMSTRIP_PAGE).min(n);
                let slice = items.with_value(|e| e[start..end].to_vec());
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
                            .map(|item| view! { <Thumb item=item/> })
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
fn Thumb(item: ImageRef) -> impl IntoView {
    let state = expect_context::<ExplorerState>();
    let selected = SelectedItem::from_image_path(item.path.clone());
    let path = selected.path.clone();
    let name = selected.name.clone();
    let mtime = item.mtime;
    let src = {
        let path = path.clone();
        move || with_file_rev(thumb_url(&path), mtime, state.media_rev.get())
    };
    let path_active = path.clone();
    let path_checked_class = path.clone();
    let path_checked_box = path.clone();
    let item_check = selected.clone();
    let open_path = item.path.clone();

    let open = move |_| {
        state.navigate(Nav::OpenImage(open_path.clone()));
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
