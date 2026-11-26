// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use crate::display::{Radii, Rect, Shape};
use tiny_skia::{Path, PathBuilder, Rect as SkRect};

pub(crate) const KAPPA: f32 = 0.552_284_8;

pub(crate) fn scale_radii(w: f32, h: f32, r: Radii) -> Radii {
    let r = Radii {
        tl: r.tl.max(0.0),
        tr: r.tr.max(0.0),
        br: r.br.max(0.0),
        bl: r.bl.max(0.0),
    };
    let mut f = 1.0_f32;
    let check = |sum: f32, len: f32, f: &mut f32| {
        if sum > len && sum > 0.0 {
            *f = f.min(len / sum);
        }
    };
    check(r.tl + r.tr, w, &mut f);
    check(r.bl + r.br, w, &mut f);
    check(r.tl + r.bl, h, &mut f);
    check(r.tr + r.br, h, &mut f);
    Radii {
        tl: r.tl * f,
        tr: r.tr * f,
        br: r.br * f,
        bl: r.bl * f,
    }
}

pub(crate) fn path(shape: &Shape) -> Option<Path> {
    let rect = shape.rect;
    if !rect.w.is_finite() || !rect.h.is_finite() || rect.w <= 0.0 || rect.h <= 0.0 {
        return None;
    }
    round_rect(rect, scale_radii(rect.w, rect.h, shape.radii))
}

pub(crate) fn scale_rect(r: Rect, dpr: f32) -> Rect {
    Rect {
        x: r.x * dpr,
        y: r.y * dpr,
        w: r.w * dpr,
        h: r.h * dpr,
    }
}

pub(crate) fn rect_path(r: Rect) -> Option<Path> {
    SkRect::from_xywh(r.x, r.y, r.w, r.h).map(PathBuilder::from_rect)
}

fn round_rect(r: Rect, radii: Radii) -> Option<Path> {
    if radii.tl == 0.0 && radii.tr == 0.0 && radii.br == 0.0 && radii.bl == 0.0 {
        return rect_path(r);
    }
    let (l, t, rr, b) = (r.x, r.y, r.x + r.w, r.y + r.h);
    let mut p = PathBuilder::new();
    p.move_to(l + radii.tl, t);
    p.line_to(rr - radii.tr, t);
    if radii.tr > 0.0 {
        let k = radii.tr * KAPPA;
        p.cubic_to(rr - radii.tr + k, t, rr, t + radii.tr - k, rr, t + radii.tr);
    }
    p.line_to(rr, b - radii.br);
    if radii.br > 0.0 {
        let k = radii.br * KAPPA;
        p.cubic_to(rr, b - radii.br + k, rr - radii.br + k, b, rr - radii.br, b);
    }
    p.line_to(l + radii.bl, b);
    if radii.bl > 0.0 {
        let k = radii.bl * KAPPA;
        p.cubic_to(l + radii.bl - k, b, l, b - radii.bl + k, l, b - radii.bl);
    }
    p.line_to(l, t + radii.tl);
    if radii.tl > 0.0 {
        let k = radii.tl * KAPPA;
        p.cubic_to(l, t + radii.tl - k, l + radii.tl - k, t, l + radii.tl, t);
    }
    p.close();
    p.finish()
}
