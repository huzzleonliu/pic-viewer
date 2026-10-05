use crate::function::crop::{
    image_norm_to_stage, move_rect, resize_corner, snap_moved_rect, snap_resized_rect,
    stage_box_to_image_norm, visual_rect_from_layout, CropCorner, CropRect, SNAP_THRESHOLD,
};
use leptos::ev;
use leptos::html;
use leptos::prelude::*;

#[derive(Clone, Copy)]
enum CropDrag {
    None,
    Pan {
        last_x: f64,
        last_y: f64,
    },
    Move {
        start: CropRect,
        origin_x: f64,
        origin_y: f64,
    },
    Resize {
        corner: CropCorner,
        start: CropRect,
    },
}

#[component]
pub fn CropOverlay(
    stage_ref: NodeRef<html::Div>,
    host_ref: NodeRef<html::Div>,
    norm: RwSignal<Option<CropRect>>,
    ratio: Signal<(u32, u32)>,
    zoom: RwSignal<f64>,
    pan: RwSignal<(f64, f64)>,
) -> impl IntoView {
    let drag = RwSignal::new(CropDrag::None);

    let image_now = move || {
        let z = zoom.get_untracked();
        let p = pan.get_untracked();
        predicted_image_in_stage(stage_ref, host_ref, z, p)
    };

    let local_pos = move |ev: &ev::MouseEvent| -> Option<(f64, f64)> {
        let (left, top, _, _) = stage_metrics(stage_ref)?;
        Some((ev.client_x() as f64 - left, ev.client_y() as f64 - top))
    };

    let on_move = move |ev: ev::MouseEvent| {
        let Some((x, y)) = local_pos(&ev) else {
            return;
        };
        let Some((_, _, stage_w, stage_h)) = stage_metrics(stage_ref) else {
            return;
        };
        let img = image_now();
        match drag.get_untracked() {
            CropDrag::None => {}
            CropDrag::Pan { last_x, last_y } => {
                let cx = ev.client_x() as f64;
                let cy = ev.client_y() as f64;
                let _ = pan.try_update(|(px, py)| {
                    *px += cx - last_x;
                    *py += cy - last_y;
                });
                drag.set(CropDrag::Pan {
                    last_x: cx,
                    last_y: cy,
                });
            }
            CropDrag::Move {
                start,
                origin_x,
                origin_y,
            } => {
                let mut next = move_rect(start, x - origin_x, y - origin_y, stage_w, stage_h);
                if let Some((ix, iy, iw, ih)) = img {
                    next = snap_moved_rect(next, ix, iy, iw, ih, stage_w, stage_h, SNAP_THRESHOLD);
                    if let Some((nx, ny, nw, nh)) = stage_box_to_image_norm(next, ix, iy, iw, ih) {
                        norm.set(Some(CropRect {
                            x: nx,
                            y: ny,
                            w: nw,
                            h: nh,
                        }));
                    }
                }
            }
            CropDrag::Resize { corner, start } => {
                let (rw, rh) = ratio.get_untracked();
                let aspect = rw as f64 / rh as f64;
                let mut next = resize_corner(start, corner, x, y, aspect);
                next = move_rect(next, 0.0, 0.0, stage_w, stage_h);
                if let Some((ix, iy, iw, ih)) = img {
                    next = snap_resized_rect(
                        next,
                        corner,
                        aspect,
                        ix,
                        iy,
                        iw,
                        ih,
                        stage_w,
                        stage_h,
                        SNAP_THRESHOLD,
                    );
                    next = move_rect(next, 0.0, 0.0, stage_w, stage_h);
                    if let Some((nx, ny, nw, nh)) = stage_box_to_image_norm(next, ix, iy, iw, ih) {
                        norm.set(Some(CropRect {
                            x: nx,
                            y: ny,
                            w: nw,
                            h: nh,
                        }));
                    }
                }
            }
        }
    };

    let end_drag = move |_| drag.set(CropDrag::None);

    let start_move = move |ev: ev::MouseEvent| {
        ev.stop_propagation();
        if ev.button() != 0 {
            return;
        }
        let Some(n) = norm.get_untracked() else {
            return;
        };
        let Some((ix, iy, iw, ih)) = image_now() else {
            return;
        };
        let start = image_norm_to_stage(n.x, n.y, n.w, n.h, ix, iy, iw, ih);
        if let Some((x, y)) = local_pos(&ev) {
            drag.set(CropDrag::Move {
                start,
                origin_x: x,
                origin_y: y,
            });
        }
    };

    view! {
        <div
            class="crop-layer"
            class:is-panning=move || matches!(drag.get(), CropDrag::Pan { .. })
            on:mousemove=on_move
            on:mouseup=end_drag
            on:mouseleave=end_drag
            on:mousedown=move |ev| {
                ev.stop_propagation();
                if ev.button() != 0 {
                    return;
                }
                drag.set(CropDrag::Pan {
                    last_x: ev.client_x() as f64,
                    last_y: ev.client_y() as f64,
                });
            }
            on:wheel=move |ev: ev::WheelEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                let factor = if ev.delta_y() < 0.0 { 1.12 } else { 1.0 / 1.12 };
                super::apply_viewer_zoom(zoom, factor);
            }
        >
            <Show when=move || norm.get().is_some()>
                <div
                    class="crop-box"
                    style=move || {
                        let Some(n) = norm.get() else {
                            return String::new();
                        };
                        let z = zoom.get();
                        let p = pan.get();
                        predicted_image_in_stage(stage_ref, host_ref, z, p)
                            .map(|(ix, iy, iw, ih)| {
                                let r = image_norm_to_stage(n.x, n.y, n.w, n.h, ix, iy, iw, ih);
                                format!(
                                    "left:{}px;top:{}px;width:{}px;height:{}px",
                                    r.x, r.y, r.w, r.h
                                )
                            })
                            .unwrap_or_default()
                    }
                    on:mousedown=start_move
                >
                    <button
                        type="button"
                        class="crop-handle nw"
                        aria-label="左上角"
                        on:mousedown=move |ev| {
                            start_handle(
                                CropCorner::Nw,
                                ev,
                                norm,
                                drag,
                                stage_ref,
                                host_ref,
                                zoom,
                                pan,
                            )
                        }
                    ></button>
                    <button
                        type="button"
                        class="crop-handle ne"
                        aria-label="右上角"
                        on:mousedown=move |ev| {
                            start_handle(
                                CropCorner::Ne,
                                ev,
                                norm,
                                drag,
                                stage_ref,
                                host_ref,
                                zoom,
                                pan,
                            )
                        }
                    ></button>
                    <button
                        type="button"
                        class="crop-handle sw"
                        aria-label="左下角"
                        on:mousedown=move |ev| {
                            start_handle(
                                CropCorner::Sw,
                                ev,
                                norm,
                                drag,
                                stage_ref,
                                host_ref,
                                zoom,
                                pan,
                            )
                        }
                    ></button>
                    <button
                        type="button"
                        class="crop-handle se"
                        aria-label="右下角"
                        on:mousedown=move |ev| {
                            start_handle(
                                CropCorner::Se,
                                ev,
                                norm,
                                drag,
                                stage_ref,
                                host_ref,
                                zoom,
                                pan,
                            )
                        }
                    ></button>
                </div>
            </Show>
        </div>
    }
}

fn start_handle(
    corner: CropCorner,
    ev: ev::MouseEvent,
    norm: RwSignal<Option<CropRect>>,
    drag: RwSignal<CropDrag>,
    stage_ref: NodeRef<html::Div>,
    host_ref: NodeRef<html::Div>,
    zoom: RwSignal<f64>,
    pan: RwSignal<(f64, f64)>,
) {
    ev.stop_propagation();
    ev.prevent_default();
    if ev.button() != 0 {
        return;
    }
    let Some(n) = norm.get_untracked() else {
        return;
    };
    let Some((ix, iy, iw, ih)) = predicted_image_in_stage(
        stage_ref,
        host_ref,
        zoom.get_untracked(),
        pan.get_untracked(),
    ) else {
        return;
    };
    drag.set(CropDrag::Resize {
        corner,
        start: image_norm_to_stage(n.x, n.y, n.w, n.h, ix, iy, iw, ih),
    });
}

fn stage_metrics(stage_ref: NodeRef<html::Div>) -> Option<(f64, f64, f64, f64)> {
    #[cfg(feature = "hydrate")]
    {
        let stage = stage_ref.get()?;
        let sr = stage.get_bounding_client_rect();
        Some((sr.left(), sr.top(), sr.width(), sr.height()))
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = stage_ref;
        None
    }
}

fn predicted_image_in_stage(
    stage_ref: NodeRef<html::Div>,
    host_ref: NodeRef<html::Div>,
    zoom: f64,
    pan: (f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    if !zoom.is_finite() || zoom <= 0.0 {
        return None;
    }
    let (left, top, lw, lh) = untransformed_host_in_stage(stage_ref, host_ref)?;
    let vis = visual_rect_from_layout(left, top, lw, lh, zoom, pan.0, pan.1);
    Some((vis.x, vis.y, vis.w, vis.h))
}

fn untransformed_host_in_stage(
    stage_ref: NodeRef<html::Div>,
    host_ref: NodeRef<html::Div>,
) -> Option<(f64, f64, f64, f64)> {
    #[cfg(feature = "hydrate")]
    {
        let stage = stage_ref.get()?;
        let host = host_ref.get()?;
        let sr = stage.get_bounding_client_rect();
        let hr = host.get_bounding_client_rect();
        let w = hr.width();
        let h = hr.height();
        if w < 1.0 || h < 1.0 {
            None
        } else {
            Some((hr.left() - sr.left(), hr.top() - sr.top(), w, h))
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (stage_ref, host_ref);
        None
    }
}
