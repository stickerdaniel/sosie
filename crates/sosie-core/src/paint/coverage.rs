// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use tiny_skia::{Path, PathSegment, Point, Transform};

pub(crate) const FLATTEN: f32 = 0.005;

#[cfg(test)]
const SUBSAMPLES: u32 = 64;

const MAX_DEPTH: u32 = 8;

type Edge = (f32, f32, f32, f32);

#[derive(Clone, Copy)]
struct Seg {
    lo_y: f32,
    hi_y: f32,
    x0: f32,
    y0: f32,
    inv_dy: f32,
    dx: f32,
    dir: i32,
    vertical: bool,
}

#[cfg(test)]
pub(crate) fn mask(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    mask_with(path, w, h, SUBSAMPLES, false)
}

const SMALL_SUBSAMPLES: u32 = 16;

pub(crate) fn mask_small(
    path: &Path,
    w: u32,
    h: u32,
    rule: tiny_skia::FillRule,
    origin_x: f32,
    origin_y: f32,
) -> Option<Vec<u8>> {
    let moved = path
        .clone()
        .transform(Transform::from_translate(-origin_x, -origin_y))?;
    mask_with(
        &moved,
        w,
        h,
        SMALL_SUBSAMPLES,
        matches!(rule, tiny_skia::FillRule::EvenOdd),
    )
}

fn mask_with(path: &Path, w: u32, h: u32, subsamples: u32, even_odd: bool) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || subsamples == 0 {
        return None;
    }
    let mut edges: Vec<Edge> = Vec::new();
    collect_edges(path, &mut edges);
    if edges.is_empty() {
        return None;
    }
    let segs = prepare(&edges, h as f32);
    if segs.is_empty() {
        return None;
    }
    let wu = w as usize;
    let weight = 1.0 / subsamples as f32;
    let count = subsamples as i32;
    let mut out = vec![0u8; wu * h as usize];
    let mut acc = vec![0.0f32; wu];
    let mut full = vec![0i32; wu + 1];
    let mut xs: Vec<(f32, i32)> = Vec::with_capacity(16);
    let mut active: Vec<usize> = Vec::with_capacity(16);
    let mut next = 0usize;
    let mut nonempty = false;
    for iy in 0..h {
        let ry0 = iy as f32;
        let ry1 = ry0 + 1.0;
        while next < segs.len() && segs[next].lo_y < ry1 {
            active.push(next);
            next += 1;
        }
        active.retain(|&i| segs[i].hi_y > ry0);
        if active.is_empty() {
            continue;
        }
        let mut simple = true;
        for &i in &active {
            let s = &segs[i];
            if !s.vertical || s.lo_y > ry0 || s.hi_y < ry1 {
                simple = false;
                break;
            }
        }
        let mut touched = (usize::MAX, 0usize);
        if simple {
            xs.clear();
            for &i in &active {
                xs.push((segs[i].x0, segs[i].dir));
            }
            if xs.len() >= 2 {
                sort_xs(&mut xs);
                add_spans(
                    &mut acc,
                    &mut full,
                    wu,
                    &mut touched,
                    &xs,
                    1.0,
                    count,
                    even_odd,
                );
            }
        } else {
            for k in 0..subsamples {
                let y = ry0 + (k as f32 + 0.5) * weight;
                xs.clear();
                for &i in &active {
                    let s = &segs[i];
                    if y < s.lo_y || y >= s.hi_y {
                        continue;
                    }
                    let x = s.x0 + s.dx * ((y - s.y0) * s.inv_dy);
                    xs.push((x, s.dir));
                }
                if xs.len() < 2 {
                    continue;
                }
                sort_xs(&mut xs);
                add_spans(
                    &mut acc,
                    &mut full,
                    wu,
                    &mut touched,
                    &xs,
                    weight,
                    1,
                    even_odd,
                );
            }
        }
        if touched.0 > touched.1 {
            continue;
        }
        let row = iy as usize * wu;
        let mut running = 0i32;
        for c in touched.0..=touched.1 {
            running += full[c];
            let v = if running == count && acc[c] == 0.0 {
                255
            } else {
                let a = acc[c] + (running as f32) * weight;
                (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
            };
            out[row + c] = v;
            nonempty |= v != 0;
            acc[c] = 0.0;
            full[c] = 0;
        }
        full[touched.1 + 1] = 0;
    }
    nonempty.then_some(out)
}

fn sort_xs(xs: &mut [(f32, i32)]) {
    xs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
}

fn inside(winding: i32, even_odd: bool) -> bool {
    if even_odd {
        winding & 1 != 0
    } else {
        winding != 0
    }
}

#[allow(clippy::too_many_arguments)]
fn add_spans(
    acc: &mut [f32],
    full: &mut [i32],
    w: usize,
    touched: &mut (usize, usize),
    xs: &[(f32, i32)],
    scale: f32,
    count: i32,
    even_odd: bool,
) {
    let mut winding = 0i32;
    let mut span_start = 0.0f32;
    for &(x, dir) in xs {
        let before = inside(winding, even_odd);
        winding += dir;
        let after = inside(winding, even_odd);
        if !before && after {
            span_start = x;
        } else if before && !after {
            add_span(acc, full, w, touched, span_start, x, scale, count);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_span(
    acc: &mut [f32],
    full: &mut [i32],
    w: usize,
    touched: &mut (usize, usize),
    xa: f32,
    xb: f32,
    scale: f32,
    count: i32,
) {
    if !(xa.is_finite() && xb.is_finite()) || xb <= xa {
        return;
    }
    let first = xa.floor().max(0.0) as usize;
    let last = (xb.floor().max(0.0) as usize).min(w - 1);
    if first > last {
        return;
    }
    touched.0 = touched.0.min(first);
    touched.1 = touched.1.max(last);
    if first == last {
        let lo = (first as f32).max(xa);
        let hi = ((first + 1) as f32).min(xb);
        if hi > lo {
            acc[first] += (hi - lo) * scale;
        }
        return;
    }
    let lo = (first as f32).max(xa);
    let hi = (first + 1) as f32;
    if hi > lo {
        acc[first] += (hi - lo) * scale;
    }
    let lo = last as f32;
    let hi = ((last + 1) as f32).min(xb);
    if hi > lo {
        acc[last] += (hi - lo) * scale;
    }
    if last > first + 1 {
        full[first + 1] += count;
        full[last] -= count;
    }
}

fn prepare(edges: &[Edge], h: f32) -> Vec<Seg> {
    let mut segs: Vec<Seg> = Vec::with_capacity(edges.len());
    for &(x0, y0, x1, y1) in edges {
        if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
            continue;
        }
        let (lo_y, hi_y) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        if hi_y <= lo_y || hi_y <= 0.0 || lo_y >= h {
            continue;
        }
        segs.push(Seg {
            lo_y,
            hi_y,
            x0,
            y0,
            inv_dy: 1.0 / (y1 - y0),
            dx: x1 - x0,
            dir: if y1 > y0 { 1 } else { -1 },
            vertical: x1 == x0,
        });
    }
    segs.sort_by(|a, b| {
        a.lo_y
            .partial_cmp(&b.lo_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    segs
}

fn collect_edges(path: &Path, out: &mut Vec<Edge>) {
    let mut cur = Point::zero();
    let mut start = Point::zero();
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                cur = p;
                start = p;
            }
            PathSegment::LineTo(p) => {
                out.push((cur.x, cur.y, p.x, p.y));
                cur = p;
            }
            PathSegment::QuadTo(p1, p2) => {
                flatten_quad(out, cur, p1, p2, 0);
                cur = p2;
            }
            PathSegment::CubicTo(p1, p2, p3) => {
                flatten_cubic(out, cur, p1, p2, p3, 0);
                cur = p3;
            }
            PathSegment::Close => {
                out.push((cur.x, cur.y, start.x, start.y));
                cur = start;
            }
        }
    }
}

fn dist_to_line(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    let (qx, qy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - qx).powi(2) + (p.y - qy).powi(2)).sqrt()
}

fn flatten_quad(out: &mut Vec<Edge>, p0: Point, p1: Point, p2: Point, depth: u32) {
    if depth >= MAX_DEPTH || dist_to_line(p1, p0, p2) <= FLATTEN {
        out.push((p0.x, p0.y, p2.x, p2.y));
        return;
    }
    let mid = |a: Point, c: Point| Point::from_xy((a.x + c.x) * 0.5, (a.y + c.y) * 0.5);
    let (a, c) = (mid(p0, p1), mid(p1, p2));
    let m = mid(a, c);
    flatten_quad(out, p0, a, m, depth + 1);
    flatten_quad(out, m, c, p2, depth + 1);
}

fn flatten_cubic(out: &mut Vec<Edge>, p0: Point, p1: Point, p2: Point, p3: Point, depth: u32) {
    let flat = dist_to_line(p1, p0, p3).max(dist_to_line(p2, p0, p3)) <= FLATTEN;
    if depth >= MAX_DEPTH || flat {
        out.push((p0.x, p0.y, p3.x, p3.y));
        return;
    }
    let mid = |a: Point, c: Point| Point::from_xy((a.x + c.x) * 0.5, (a.y + c.y) * 0.5);
    let (p01, p12, p23) = (mid(p0, p1), mid(p1, p2), mid(p2, p3));
    let (p012, p123) = (mid(p01, p12), mid(p12, p23));
    let m = mid(p012, p123);
    flatten_cubic(out, p0, p01, p012, m, depth + 1);
    flatten_cubic(out, m, p123, p23, p3, depth + 1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::PathBuilder;

    fn at(px: &[u8], w: u32, x: u32, y: u32) -> u8 {
        px[(y * w + x) as usize]
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
        let mut b = PathBuilder::new();
        b.move_to(x0, y0);
        b.line_to(x1, y0);
        b.line_to(x1, y1);
        b.line_to(x0, y1);
        b.close();
        b.finish().unwrap()
    }

    #[test]
    fn an_axis_aligned_rectangle_is_exactly_one_or_zero() {
        let c = mask(&rect(2.0, 3.0, 6.0, 8.0), 10, 12).unwrap();
        for y in 0..12 {
            for x in 0..10 {
                let want = if (2..6).contains(&x) && (3..8).contains(&y) {
                    255
                } else {
                    0
                };
                assert_eq!(at(&c, 10, x, y), want, "at {x},{y}");
            }
        }
    }

    #[test]
    fn a_half_pixel_edge_is_half_coverage() {
        let c = mask(&rect(2.0, 1.0, 6.5, 4.0), 10, 6).unwrap();
        assert_eq!(at(&c, 10, 2, 2), 255);
        assert!(
            (at(&c, 10, 6, 2) as i32 - 128).abs() <= 2,
            "{}",
            at(&c, 10, 6, 2)
        );
        assert_eq!(at(&c, 10, 7, 2), 0);
    }

    #[test]
    fn a_diagonal_edge_is_linear() {
        let mut b = PathBuilder::new();
        b.move_to(0.0, 0.0);
        b.line_to(8.0, 0.0);
        b.line_to(4.0, 4.0);
        b.line_to(0.0, 4.0);
        b.close();
        let c = mask(&b.finish().unwrap(), 8, 4).unwrap();
        assert_eq!(at(&c, 8, 0, 0), 255);
        let row3: u32 = (0..8).map(|x| at(&c, 8, x, 3) as u32).sum();
        assert!(
            (row3 as i32 - (4.5 * 255.0) as i32).abs() <= 8,
            "row 3 covers {row3}/255 px"
        );
    }

    #[test]
    fn a_circle_matches_analytic_area() {
        let (cx, cy, r) = (24.0f32, 24.0f32, 20.0f32);
        let mut b = PathBuilder::new();
        b.push_circle(cx, cy, r);
        let path = b.finish().unwrap();
        let c = mask(&path, 48, 48).unwrap();
        let s = 24u32;
        let mut worst = 0f32;
        for y in 0..48u32 {
            for x in 0..48u32 {
                let mut hits = 0u32;
                for sy in 0..s {
                    for sx in 0..s {
                        let px = x as f32 + (sx as f32 + 0.5) / s as f32;
                        let py = y as f32 + (sy as f32 + 0.5) / s as f32;
                        if (px - cx).powi(2) + (py - cy).powi(2) <= r * r {
                            hits += 1;
                        }
                    }
                }
                let want = 255.0 * hits as f32 / (s * s) as f32;
                worst = worst.max((at(&c, 48, x, y) as f32 - want).abs());
            }
        }
        assert!(worst <= 4.0, "worst deviation {worst}");
    }

    #[test]
    fn a_row_with_two_spans_adds_both() {
        let mut b = PathBuilder::new();
        b.move_to(0.0, 0.0);
        b.line_to(3.0, 0.0);
        b.line_to(3.0, 4.0);
        b.line_to(0.0, 4.0);
        b.close();
        b.move_to(6.0, 0.0);
        b.line_to(9.0, 0.0);
        b.line_to(9.0, 4.0);
        b.line_to(6.0, 4.0);
        b.close();
        let c = mask(&b.finish().unwrap(), 10, 4).unwrap();
        for y in 0..4 {
            assert_eq!(at(&c, 10, 1, y), 255, "left span at row {y}");
            assert_eq!(at(&c, 10, 2, y), 255, "left span at row {y}");
            assert_eq!(at(&c, 10, 4, y), 0, "the gap at row {y}");
            assert_eq!(at(&c, 10, 5, y), 0, "the gap at row {y}");
            assert_eq!(at(&c, 10, 7, y), 255, "right span at row {y}");
            assert_eq!(at(&c, 10, 8, y), 255, "right span at row {y}");
        }
    }
}
