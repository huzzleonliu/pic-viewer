use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropCorner {
    Nw,
    Ne,
    Sw,
    Se,
}

const MIN_SIDE: f64 = 24.0;
pub const SNAP_THRESHOLD: f64 = 8.0;

/// 在区域内放得下的、给定宽高比的最大居中矩形，坐标为 0–1。
pub fn fitted_crop(img_w: f64, img_h: f64, ratio_w: f64, ratio_h: f64) -> (f64, f64, f64, f64) {
    if img_w <= 0.0 || img_h <= 0.0 || ratio_w <= 0.0 || ratio_h <= 0.0 {
        return (0.0, 0.0, 1.0, 1.0);
    }
    let img_a = img_w / img_h;
    let tgt_a = ratio_w / ratio_h;
    if (img_a - tgt_a).abs() < 1e-6 {
        (0.0, 0.0, 1.0, 1.0)
    } else if img_a > tgt_a {
        let w = tgt_a / img_a;
        ((1.0 - w) / 2.0, 0.0, w, 1.0)
    } else {
        let h = img_a / tgt_a;
        (0.0, (1.0 - h) / 2.0, 1.0, h)
    }
}

pub fn initial_box_in_stage(
    img_x: f64,
    img_y: f64,
    img_w: f64,
    img_h: f64,
    ratio_w: f64,
    ratio_h: f64,
) -> CropRect {
    let (fx, fy, fw, fh) = fitted_crop(img_w, img_h, ratio_w, ratio_h);
    CropRect {
        x: img_x + fx * img_w,
        y: img_y + fy * img_h,
        w: (fw * img_w).max(MIN_SIDE),
        h: (fh * img_h).max(MIN_SIDE),
    }
}

pub fn move_rect(r: CropRect, dx: f64, dy: f64, stage_w: f64, stage_h: f64) -> CropRect {
    let min_vis = MIN_SIDE.min(r.w).min(r.h);
    CropRect {
        x: (r.x + dx).clamp(min_vis - r.w, stage_w - min_vis),
        y: (r.y + dy).clamp(min_vis - r.h, stage_h - min_vis),
        w: r.w,
        h: r.h,
    }
}

pub fn resize_corner(
    start: CropRect,
    corner: CropCorner,
    mouse_x: f64,
    mouse_y: f64,
    aspect: f64,
) -> CropRect {
    if aspect <= 0.0 {
        return start;
    }
    let min_w = MIN_SIDE;
    let min_h = MIN_SIDE / aspect;
    match corner {
        CropCorner::Se => size_from_fixed(
            start.x, start.y, mouse_x, mouse_y, aspect, min_w, min_h, true, true,
        ),
        CropCorner::Nw => size_from_fixed(
            start.x + start.w,
            start.y + start.h,
            mouse_x,
            mouse_y,
            aspect,
            min_w,
            min_h,
            false,
            false,
        ),
        CropCorner::Ne => size_from_fixed(
            start.x,
            start.y + start.h,
            mouse_x,
            mouse_y,
            aspect,
            min_w,
            min_h,
            true,
            false,
        ),
        CropCorner::Sw => size_from_fixed(
            start.x + start.w,
            start.y,
            mouse_x,
            mouse_y,
            aspect,
            min_w,
            min_h,
            false,
            true,
        ),
    }
}

fn size_from_fixed(
    fx: f64,
    fy: f64,
    mx: f64,
    my: f64,
    aspect: f64,
    min_w: f64,
    min_h: f64,
    from_left: bool,
    from_top: bool,
) -> CropRect {
    let raw_w = if from_left { mx - fx } else { fx - mx };
    let raw_h = if from_top { my - fy } else { fy - my };
    let mut w = raw_w.max(min_w);
    let mut h = w / aspect;
    let h_y = raw_h.max(min_h);
    let w_y = h_y * aspect;
    if w_y > w {
        w = w_y;
        h = h_y;
    }
    rect_from_fixed(fx, fy, w, h, from_left, from_top)
}

fn rect_from_fixed(fx: f64, fy: f64, w: f64, h: f64, from_left: bool, from_top: bool) -> CropRect {
    CropRect {
        x: if from_left { fx } else { fx - w },
        y: if from_top { fy } else { fy - h },
        w,
        h,
    }
}

pub fn scale_rect_around(r: CropRect, cx: f64, cy: f64, factor: f64) -> CropRect {
    if !factor.is_finite() || factor <= 0.0 {
        return r;
    }
    CropRect {
        x: cx + (r.x - cx) * factor,
        y: cy + (r.y - cy) * factor,
        w: (r.w * factor).max(MIN_SIDE),
        h: (r.h * factor).max(MIN_SIDE),
    }
}

fn edge_guides(
    img_x: f64,
    img_y: f64,
    img_w: f64,
    img_h: f64,
    stage_w: f64,
    stage_h: f64,
) -> ([f64; 4], [f64; 4]) {
    (
        [img_x, img_x + img_w, 0.0, stage_w],
        [img_y, img_y + img_h, 0.0, stage_h],
    )
}

fn nearest_delta(value: f64, guides: &[f64], threshold: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for &guide in guides {
        let delta = guide - value;
        if delta.abs() <= threshold && best.is_none_or(|b| delta.abs() < b.abs()) {
            best = Some(delta);
        }
    }
    best
}

fn closer(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if x.abs() <= y.abs() { x } else { y }),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

pub fn snap_moved_rect(
    r: CropRect,
    img_x: f64,
    img_y: f64,
    img_w: f64,
    img_h: f64,
    stage_w: f64,
    stage_h: f64,
    threshold: f64,
) -> CropRect {
    let (xs, ys) = edge_guides(img_x, img_y, img_w, img_h, stage_w, stage_h);
    let dx = closer(
        nearest_delta(r.x, &xs, threshold),
        nearest_delta(r.x + r.w, &xs, threshold),
    )
    .unwrap_or(0.0);
    let dy = closer(
        nearest_delta(r.y, &ys, threshold),
        nearest_delta(r.y + r.h, &ys, threshold),
    )
    .unwrap_or(0.0);
    move_rect(r, dx, dy, stage_w, stage_h)
}

pub fn snap_resized_rect(
    r: CropRect,
    corner: CropCorner,
    aspect: f64,
    img_x: f64,
    img_y: f64,
    img_w: f64,
    img_h: f64,
    stage_w: f64,
    stage_h: f64,
    threshold: f64,
) -> CropRect {
    if aspect <= 0.0 {
        return r;
    }
    let (xs, ys) = edge_guides(img_x, img_y, img_w, img_h, stage_w, stage_h);
    let (fx, fy, from_left, from_top) = match corner {
        CropCorner::Se => (r.x, r.y, true, true),
        CropCorner::Nw => (r.x + r.w, r.y + r.h, false, false),
        CropCorner::Ne => (r.x, r.y + r.h, true, false),
        CropCorner::Sw => (r.x + r.w, r.y, false, true),
    };
    let moving_v = if from_left { r.x + r.w } else { r.x };
    let moving_h = if from_top { r.y + r.h } else { r.y };
    let min_w = MIN_SIDE;
    let min_h = MIN_SIDE / aspect;

    let mut best: Option<(f64, CropRect)> = None;
    for &guide in &xs {
        let w = if from_left { guide - fx } else { fx - guide };
        if w < min_w {
            continue;
        }
        let delta = guide - moving_v;
        if delta.abs() > threshold {
            continue;
        }
        let cand = rect_from_fixed(fx, fy, w, w / aspect, from_left, from_top);
        consider_snap(&mut best, delta.abs(), cand);
    }
    for &guide in &ys {
        let h = if from_top { guide - fy } else { fy - guide };
        if h < min_h {
            continue;
        }
        let delta = guide - moving_h;
        if delta.abs() > threshold {
            continue;
        }
        let cand = rect_from_fixed(fx, fy, h * aspect, h, from_left, from_top);
        consider_snap(&mut best, delta.abs(), cand);
    }
    best.map(|(_, snapped)| snapped).unwrap_or(r)
}

fn consider_snap(best: &mut Option<(f64, CropRect)>, dist: f64, cand: CropRect) {
    match best {
        Some((d, _)) if *d <= dist => {}
        _ => *best = Some((dist, cand)),
    }
}

pub fn stage_box_to_image_norm(
    r: CropRect,
    img_x: f64,
    img_y: f64,
    img_w: f64,
    img_h: f64,
) -> Option<(f64, f64, f64, f64)> {
    if img_w < 1.0 || img_h < 1.0 || r.w < 1.0 || r.h < 1.0 {
        return None;
    }
    Some((
        (r.x - img_x) / img_w,
        (r.y - img_y) / img_h,
        r.w / img_w,
        r.h / img_h,
    ))
}

pub fn image_norm_to_stage(
    nx: f64,
    ny: f64,
    nw: f64,
    nh: f64,
    img_x: f64,
    img_y: f64,
    img_w: f64,
    img_h: f64,
) -> CropRect {
    CropRect {
        x: img_x + nx * img_w,
        y: img_y + ny * img_h,
        w: (nw * img_w).max(1.0),
        h: (nh * img_h).max(1.0),
    }
}

/// 与 CSS `transform: translate(pan) scale(zoom)`、`transform-origin: center` 一致。
pub fn visual_rect_from_layout(
    left: f64,
    top: f64,
    w: f64,
    h: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
) -> CropRect {
    let cx = left + w * 0.5 + pan_x;
    let cy = top + h * 0.5 + pan_y;
    let vw = w * zoom;
    let vh = h * zoom;
    CropRect {
        x: cx - vw * 0.5,
        y: cy - vh * 0.5,
        w: vw,
        h: vh,
    }
}

pub fn fitted_norm(img_w: f64, img_h: f64, ratio_w: f64, ratio_h: f64) -> CropRect {
    let (x, y, w, h) = fitted_crop(img_w, img_h, ratio_w, ratio_h);
    CropRect { x, y, w, h }
}

/// `object-fit: contain` 时，图片内容在元素盒里的像素矩形。
pub fn contain_content_rect(box_w: f64, box_h: f64, nat_w: f64, nat_h: f64) -> CropRect {
    if box_w <= 0.0 || box_h <= 0.0 || nat_w <= 0.0 || nat_h <= 0.0 {
        return CropRect {
            x: 0.0,
            y: 0.0,
            w: box_w.max(0.0),
            h: box_h.max(0.0),
        };
    }
    let scale = (box_w / nat_w).min(box_h / nat_h);
    let w = nat_w * scale;
    let h = nat_h * scale;
    CropRect {
        x: (box_w - w) * 0.5,
        y: (box_h - h) * 0.5,
        w,
        h,
    }
}

pub fn canvas_placement(
    src_w: u32,
    src_h: u32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Option<(u32, u32, i64, i64)> {
    if src_w == 0 || src_h == 0 || !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return None;
    }
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let iw = src_w as f64;
    let ih = src_h as f64;
    let out_w = (w * iw).round().clamp(1.0, 50_000.0) as u32;
    let out_h = (h * ih).round().clamp(1.0, 50_000.0) as u32;
    let dx = (-x * iw).round() as i64;
    let dy = (-y * ih).round() as i64;
    Some((out_w, out_h, dx, dy))
}

#[server]
pub async fn save_cropped_image(
    path: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<String, ServerFnError> {
    if path.is_empty() {
        return Err(ServerFnError::new("未选择图片"));
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (path, x, y, w, h);
        Err(ServerFnError::new("仅服务端可用"))
    }
    #[cfg(feature = "ssr")]
    {
        io::crop_and_save(&path, x, y, w, h)
            .await
            .map_err(ServerFnError::new)
    }
}

#[cfg(feature = "ssr")]
mod io {
    use super::canvas_placement;
    use crate::function::fs::{pic_root, resolve_path, to_rel, unique_dest};
    use crate::function::path::rel_name;
    use image::{DynamicImage, Rgba, RgbaImage};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    pub async fn crop_and_save(
        rel: &str,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Result<String, String> {
        let full = resolve_path(rel)?;
        if full == pic_root() || !full.is_file() {
            return Err("不是可裁切的图片".into());
        }
        let original = tokio::fs::read(&full)
            .await
            .map_err(|e| format!("读取失败：{e}"))?;
        let rel = rel.to_string();
        tokio::task::spawn_blocking(move || {
            let img =
                image::load_from_memory(&original).map_err(|e| format!("无法解码图片：{e}"))?;
            let framed = crop_with_alpha(&img, x, y, w, h)?;
            let encoded = encode_webp_rgba(&framed)?;
            let parent = full.parent().ok_or_else(|| "非法路径".to_string())?;
            let name = rel_name(&rel);
            let dest = if is_webp_name(name) {
                full.clone()
            } else {
                let stem = file_stem(name);
                unique_dest(parent, &format!("{stem}.webp"))
            };
            atomic_write(&dest, &encoded)?;
            Ok(to_rel(&dest))
        })
        .await
        .map_err(|e| e.to_string())?
    }

    fn is_webp_name(name: &str) -> bool {
        name.rsplit_once('.')
            .map(|(_, ext)| ext.eq_ignore_ascii_case("webp"))
            .unwrap_or(false)
    }

    fn file_stem(name: &str) -> String {
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => name.to_string(),
        }
    }

    fn crop_with_alpha(
        img: &DynamicImage,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Result<RgbaImage, String> {
        let (out_w, out_h, dx, dy) = canvas_placement(img.width(), img.height(), x, y, w, h)
            .ok_or_else(|| "裁切区域无效".to_string())?;
        let mut canvas = RgbaImage::from_pixel(out_w, out_h, Rgba([0, 0, 0, 0]));
        image::imageops::overlay(&mut canvas, &img.to_rgba8(), dx, dy);
        Ok(canvas)
    }

    fn encode_webp_rgba(img: &RgbaImage) -> Result<Vec<u8>, String> {
        let encoder = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height());
        let mem = encoder.encode_lossless();
        if mem.is_empty() {
            Err("编码结果为空".into())
        } else {
            Ok(mem.to_vec())
        }
    }

    fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
        let parent = path.parent().ok_or_else(|| "非法路径".to_string())?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tmp = unique_dest(parent, &format!(".__pv_crop_{stamp}.webp"));
        if let Err(e) = std::fs::write(&tmp, bytes) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn crop_with_alpha_for_test(
        img: &DynamicImage,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Result<RgbaImage, String> {
        crop_with_alpha(img, x, y, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canvas_placement, contain_content_rect, fitted_crop, fitted_norm, image_norm_to_stage,
        initial_box_in_stage, move_rect, resize_corner, scale_rect_around, snap_moved_rect,
        snap_resized_rect, stage_box_to_image_norm, visual_rect_from_layout, CropCorner, CropRect,
    };

    #[test]
    fn same_ratio_is_full_frame() {
        assert_eq!(fitted_crop(3000.0, 4000.0, 3.0, 4.0), (0.0, 0.0, 1.0, 1.0));
        assert_eq!(fitted_crop(100.0, 100.0, 1.0, 1.0), (0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn landscape_to_square_is_centered() {
        let (x, y, w, h) = fitted_crop(200.0, 100.0, 1.0, 1.0);
        assert!((y - 0.0).abs() < 1e-9);
        assert!((h - 1.0).abs() < 1e-9);
        assert!((w - 0.5).abs() < 1e-9);
        assert!((x - 0.25).abs() < 1e-9);
    }

    #[test]
    fn se_resize_keeps_aspect() {
        let start = CropRect {
            x: 10.0,
            y: 10.0,
            w: 100.0,
            h: 50.0,
        };
        let next = resize_corner(start, CropCorner::Se, 210.0, 10.0, 2.0);
        assert!((next.x - 10.0).abs() < 1e-9);
        assert!((next.y - 10.0).abs() < 1e-9);
        assert!((next.w / next.h - 2.0).abs() < 1e-6);
        assert!(next.w >= 200.0 - 1e-6);
    }

    #[test]
    fn move_keeps_box_on_stage() {
        let r = CropRect {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 40.0,
        };
        let moved = move_rect(r, 1000.0, 0.0, 200.0, 200.0);
        assert!(moved.x + 24.0 <= 200.0 + 1e-6);
    }

    #[test]
    fn placement_pads_outside_image() {
        let (out_w, out_h, dx, dy) =
            canvas_placement(100, 100, -0.1, 0.0, 1.2, 1.0).expect("place");
        assert_eq!((out_w, out_h), (120, 100));
        assert_eq!((dx, dy), (10, 0));
    }

    #[test]
    fn stage_box_maps_back_to_image() {
        let r = initial_box_in_stage(50.0, 20.0, 200.0, 100.0, 1.0, 1.0);
        let (x, y, w, h) = stage_box_to_image_norm(r, 50.0, 20.0, 200.0, 100.0).unwrap();
        assert!((y - 0.0).abs() < 1e-6);
        assert!((h - 1.0).abs() < 1e-6);
        assert!((w - 0.5).abs() < 1e-6);
        assert!((x - 0.25).abs() < 1e-6);
    }

    #[test]
    fn zooming_image_keeps_normalized_crop() {
        let n = fitted_norm(200.0, 100.0, 1.0, 1.0);
        let before = image_norm_to_stage(n.x, n.y, n.w, n.h, 50.0, 20.0, 200.0, 100.0);
        let after = image_norm_to_stage(n.x, n.y, n.w, n.h, -50.0, -30.0, 400.0, 200.0);
        let back = stage_box_to_image_norm(after, -50.0, -30.0, 400.0, 200.0).unwrap();
        assert!((back.0 - n.x).abs() < 1e-9);
        assert!((back.1 - n.y).abs() < 1e-9);
        assert!((back.2 - n.w).abs() < 1e-9);
        assert!((back.3 - n.h).abs() < 1e-9);
        let scaled = scale_rect_around(before, 150.0, 70.0, 2.0);
        assert!((scaled.x - after.x).abs() < 1e-9);
        assert!((scaled.y - after.y).abs() < 1e-9);
        assert!((scaled.w - after.w).abs() < 1e-9);
        assert!((scaled.h - after.h).abs() < 1e-9);
    }

    #[test]
    fn layout_zoom_matches_css_center_origin() {
        let vis = visual_rect_from_layout(50.4, 20.7, 200.6, 100.2, 2.0, 10.0, 5.0);
        assert!((vis.x - (-39.9)).abs() < 1e-9);
        assert!((vis.y - (-24.4)).abs() < 1e-9);
        assert!((vis.w - 401.2).abs() < 1e-9);
        assert!((vis.h - 200.4).abs() < 1e-9);
        let vis0 = visual_rect_from_layout(50.4, 20.7, 200.6, 100.2, 1.0, 0.0, 0.0);
        assert!((vis0.x - 50.4).abs() < 1e-9);
        assert!((vis0.y - 20.7).abs() < 1e-9);
    }

    #[test]
    fn snap_moves_left_edge_to_image() {
        let r = CropRect {
            x: 54.0,
            y: 20.0,
            w: 40.0,
            h: 40.0,
        };
        let snapped = snap_moved_rect(r, 50.0, 10.0, 200.0, 100.0, 400.0, 300.0, 8.0);
        assert!((snapped.x - 50.0).abs() < 1e-9);
        assert!((snapped.y - 20.0).abs() < 1e-9);
    }

    #[test]
    fn snap_ignores_far_edges() {
        let r = CropRect {
            x: 80.0,
            y: 20.0,
            w: 40.0,
            h: 40.0,
        };
        let snapped = snap_moved_rect(r, 50.0, 10.0, 200.0, 100.0, 400.0, 300.0, 8.0);
        assert!((snapped.x - 80.0).abs() < 1e-9);
    }

    #[test]
    fn snap_resize_se_to_image_right_keeps_aspect() {
        let r = CropRect {
            x: 50.0,
            y: 10.0,
            w: 194.0,
            h: 97.0,
        };
        let snapped = snap_resized_rect(
            r,
            CropCorner::Se,
            2.0,
            50.0,
            10.0,
            200.0,
            100.0,
            400.0,
            300.0,
            8.0,
        );
        assert!((snapped.x - 50.0).abs() < 1e-9);
        assert!(((snapped.x + snapped.w) - 250.0).abs() < 1e-9);
        assert!((snapped.w / snapped.h - 2.0).abs() < 1e-6);
    }

    #[test]
    fn contain_letterbox_is_centered() {
        let r = contain_content_rect(200.0, 100.0, 50.0, 50.0);
        assert!((r.x - 50.0).abs() < 1e-9);
        assert!((r.y - 0.0).abs() < 1e-9);
        assert!((r.w - 100.0).abs() < 1e-9);
        assert!((r.h - 100.0).abs() < 1e-9);
    }

    #[test]
    fn contain_matching_aspect_fills_box() {
        let r = contain_content_rect(300.0, 400.0, 3000.0, 4000.0);
        assert!(r.x.abs() < 1e-9);
        assert!(r.y.abs() < 1e-9);
        assert!((r.w - 300.0).abs() < 1e-9);
        assert!((r.h - 400.0).abs() < 1e-9);
    }

    #[test]
    fn already_scaled_rect_must_not_be_scaled_again() {
        let layout = visual_rect_from_layout(40.0, 80.0, 100.0, 200.0, 1.0, 0.0, 0.0);
        let once = visual_rect_from_layout(layout.x, layout.y, layout.w, layout.h, 2.0, 0.0, 0.0);
        let twice = visual_rect_from_layout(once.x, once.y, once.w, once.h, 2.0, 0.0, 0.0);
        assert!((once.y - (-20.0)).abs() < 1e-9);
        assert!(twice.y < once.y);
    }

    #[test]
    fn scale_around_center_keeps_relative_pos() {
        let r = CropRect {
            x: 50.0,
            y: 20.0,
            w: 100.0,
            h: 50.0,
        };
        let next = scale_rect_around(r, 100.0, 70.0, 2.0);
        assert!((next.x - 0.0).abs() < 1e-9);
        assert!((next.y - (-30.0)).abs() < 1e-9);
        assert!((next.w - 200.0).abs() < 1e-9);
        assert!((next.h - 100.0).abs() < 1e-9);
    }
}

#[cfg(all(test, feature = "ssr"))]
mod alpha_tests {
    use super::io::crop_with_alpha_for_test;
    use image::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn outside_image_is_transparent() {
        let mut img = RgbImage::new(4, 2);
        for p in img.pixels_mut() {
            *p = Rgb([255, 0, 0]);
        }
        let out = crop_with_alpha_for_test(&DynamicImage::ImageRgb8(img), -0.5, 0.0, 1.5, 1.0)
            .expect("crop");
        assert_eq!(out.dimensions(), (6, 2));
        assert_eq!(out.get_pixel(0, 0).0[3], 0);
        assert_eq!(out.get_pixel(2, 0).0, [255, 0, 0, 255]);
    }
}
