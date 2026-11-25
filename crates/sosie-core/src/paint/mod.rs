// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

mod coverage;
mod emoji;
mod encode;
mod glyph;
mod shapes;

use crate::color::Rgba;
use crate::display::{DisplayList, GlyphPos, Item, Radii, Rect, Shape};
use crate::fonts::{FaceId, Fonts};
use crate::limits::{LimitError, Limits};
pub use encode::encode_png;
use std::fmt;
use tiny_skia::{FillRule, Path, PathBuilder};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Pixmap {
    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgba> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = &self.rgba[i..i + 4];
        Some(Rgba {
            r: p[0],
            g: p[1],
            b: p[2],
            a: p[3],
        })
    }

    pub fn to_png(&self) -> Vec<u8> {
        encode_png(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderError {
    Limit(LimitError),
    TranslucentBackground,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Limit(e) => e.fmt(f),
            RenderError::TranslucentBackground => {
                write!(f, "the surface background must be opaque")
            }
        }
    }
}

impl std::error::Error for RenderError {}

impl From<LimitError> for RenderError {
    fn from(e: LimitError) -> Self {
        RenderError::Limit(e)
    }
}

pub fn rasterize(
    fonts: &Fonts,
    list: &DisplayList,
    dpr: f32,
    limits: &Limits,
) -> Result<Pixmap, RenderError> {
    let (width, height, bytes) = limits.output(list.width, list.height, dpr)?;
    let bg = list.background;
    if bg.a != 255 {
        return Err(RenderError::TranslucentBackground);
    }
    let mut rgba = vec![0u8; bytes];
    for px in rgba.as_chunks_mut::<4>().0 {
        px.copy_from_slice(&[bg.r, bg.g, bg.b, 255]);
    }
    let mut r = Renderer {
        pm: Pixmap {
            width,
            height,
            rgba,
        },
        dpr,
        fonts,
    };
    for item in &list.items {
        r.item(item);
    }
    Ok(r.pm)
}

pub fn rasterize_png(
    fonts: &Fonts,
    list: &DisplayList,
    dpr: f32,
    limits: &Limits,
) -> Result<Vec<u8>, RenderError> {
    rasterize(fonts, list, dpr, limits).map(|p| encode_png(&p))
}

struct Renderer<'a> {
    pm: Pixmap,
    dpr: f32,
    fonts: &'a Fonts,
}

fn floor64(v: f32) -> f32 {
    (v * 64.0).floor() / 64.0
}

fn scale_shape(s: &Shape, dpr: f32) -> Shape {
    Shape {
        rect: shapes::scale_rect(s.rect, dpr),
        radii: Radii {
            tl: s.radii.tl * dpr,
            tr: s.radii.tr * dpr,
            br: s.radii.br * dpr,
            bl: s.radii.bl * dpr,
        },
    }
}

impl Renderer<'_> {
    fn item(&mut self, item: &Item) {
        match item {
            Item::Fill { shape, color } => {
                if let Some(p) = shapes::path(&scale_shape(shape, self.dpr)) {
                    self.fill(&p, *color, FillRule::Winding);
                }
            }
            Item::Border {
                shape,
                width,
                color,
            } => self.border(shape, *width, *color),
            Item::Glyphs {
                face,
                size,
                color,
                x,
                baseline,
                glyphs,
            } => self.glyphs(*face, *size, *color, *x, *baseline, glyphs),
            Item::TextBackground {
                x0,
                x1,
                baseline,
                ascent,
                descent,
                padding_y,
                radius,
                color,
            } => {
                let d = self.dpr;
                let base = (baseline * d).round();
                let pad = floor64(padding_y * d);
                let (l, r) = ((x0 * d).round(), (x1 * d).round());
                let (t, b) = (
                    (base - ascent * d - pad).ceil(),
                    (base + descent * d + pad).floor(),
                );
                let shape = Shape {
                    rect: Rect {
                        x: l,
                        y: t,
                        w: r - l,
                        h: b - t,
                    },
                    radii: Radii::all(radius * d),
                };
                if let Some(p) = shapes::path(&shape) {
                    self.fill(&p, *color, FillRule::Winding);
                }
            }
        }
    }

    fn border(&mut self, shape: &Shape, width: f32, color: Rgba) {
        if !width.is_finite() || width <= 0.0 {
            return;
        }
        let outer = scale_shape(shape, self.dpr);
        let bw = width * self.dpr;
        let rect = outer.rect;
        let fitted = shapes::scale_radii(rect.w, rect.h, outer.radii);
        let inner = Shape {
            rect: Rect {
                x: rect.x + bw,
                y: rect.y + bw,
                w: rect.w - 2.0 * bw,
                h: rect.h - 2.0 * bw,
            },
            radii: Radii {
                tl: (fitted.tl - bw).max(0.0),
                tr: (fitted.tr - bw).max(0.0),
                br: (fitted.br - bw).max(0.0),
                bl: (fitted.bl - bw).max(0.0),
            },
        };
        let Some(o) = shapes::path(&outer) else {
            return;
        };
        let mut pb = PathBuilder::new();
        pb.push_path(&o);
        if let Some(i) = shapes::path(&inner) {
            pb.push_path(&i);
        }
        if let Some(ring) = pb.finish() {
            self.fill(&ring, color, FillRule::EvenOdd);
        }
    }

    fn fill(&mut self, path: &Path, color: Rgba, rule: FillRule) {
        if color.a == 0 {
            return;
        }
        let b = path.bounds();
        let visible = b.right() > 0.0
            && b.bottom() > 0.0
            && b.left() < self.pm.width as f32
            && b.top() < self.pm.height as f32;
        if !visible {
            return;
        }
        let left = b.left().floor().max(0.0);
        let top = b.top().floor().max(0.0);
        let w = (b.right().ceil().min(self.pm.width as f32) - left).max(0.0) as u32;
        let h = (b.bottom().ceil().min(self.pm.height as f32) - top).max(0.0) as u32;
        if w == 0 || h == 0 {
            return;
        }
        if let Some(cov) = coverage::mask_small(path, w, h, rule, left, top) {
            self.blit(&cov, w, h, left as i64, top as i64, color, None);
        }
    }

    fn glyphs(
        &mut self,
        face: FaceId,
        size: f32,
        color: Rgba,
        x: f32,
        baseline: f32,
        glyphs: &[GlyphPos],
    ) {
        if !size.is_finite()
            || size <= 0.0
            || !x.is_finite()
            || !baseline.is_finite()
            || color.a == 0
        {
            return;
        }
        let Some(f) = self.fonts.face(face) else {
            return;
        };
        let d = self.dpr;
        let base_x = x * d;
        let base_y = (baseline * d).round();
        if f.is_emoji() {
            let size_dev = size * d;
            for g in glyphs {
                if let Some(s) =
                    emoji::sprite(f, g.id, size_dev, base_x + g.x * d, base_y + g.y * d)
                {
                    self.blit_premul(&s.data, s.w, s.h, s.left as i64, s.top as i64);
                }
            }
            return;
        }
        let size_dev = floor64(size * d);
        if size_dev > 1024.0 {
            return;
        }
        let lut = glyph::gamma_table(color);
        for g in glyphs {
            if !g.x.is_finite() || !g.y.is_finite() {
                continue;
            }
            let xq = glyph::quantize(base_x + g.x * d);
            let yq = (base_y + g.y * d).round();
            let Some(b) = glyph::bitmap(f, g.id, size_dev, xq) else {
                continue;
            };
            let left = (xq.floor() as i64).saturating_add(b.left as i64);
            let top = (yq as i64).saturating_add(b.top as i64);
            self.blit(&b.data, b.w, b.h, left, top, color, Some(lut));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn blit(
        &mut self,
        cov: &[u8],
        w: u32,
        h: u32,
        left: i64,
        top: i64,
        c: Rgba,
        lut: Option<&[u8; 256]>,
    ) {
        let pw = self.pm.width as i64;
        let (xs, ys) = (visible(left, w, pw), visible(top, h, self.pm.height as i64));
        for y in ys {
            let py = top + y;
            for x in xs.clone() {
                let px = left + x;
                let raw = cov[(y * w as i64 + x) as usize];
                let cv = lut.map_or(raw, |t| t[raw as usize]) as u32;
                let a = (c.a as u32 * cv + 127) / 255;
                if a == 0 {
                    continue;
                }
                let i = ((py * pw + px) * 4) as usize;
                let inv = 255 - a;
                for (k, s) in [c.r, c.g, c.b].into_iter().enumerate() {
                    let d = self.pm.rgba[i + k] as u32;
                    self.pm.rgba[i + k] = ((s as u32 * a + d * inv + 127) / 255).min(255) as u8;
                }
            }
        }
    }

    fn blit_premul(&mut self, src: &[u8], w: u32, h: u32, left: i64, top: i64) {
        let pw = self.pm.width as i64;
        let (xs, ys) = (visible(left, w, pw), visible(top, h, self.pm.height as i64));
        for y in ys {
            let py = top + y;
            for x in xs.clone() {
                let px = left + x;
                let s = ((y * w as i64 + x) * 4) as usize;
                let a = src[s + 3] as u32;
                if a == 0 {
                    continue;
                }
                let i = ((py * pw + px) * 4) as usize;
                let inv = 255 - a;
                for k in 0..3 {
                    let d = self.pm.rgba[i + k] as u32;
                    self.pm.rgba[i + k] =
                        (src[s + k] as u32 + (d * inv + 127) / 255).min(255) as u8;
                }
            }
        }
    }
}

fn visible(origin: i64, len: u32, extent: i64) -> std::ops::Range<i64> {
    let start = origin.saturating_neg().clamp(0, len as i64);
    let end = extent.saturating_sub(origin).clamp(0, len as i64);
    start..end.max(start)
}
