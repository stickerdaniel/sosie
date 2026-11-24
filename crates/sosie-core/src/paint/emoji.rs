use super::glyph::quantize;
use crate::fonts::Face;
use skrifa::bitmap::{BitmapData, BitmapStrikes};
use skrifa::instance::Size;
use skrifa::GlyphId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct Sprite {
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
    pub data: Vec<u8>,
}

struct Level {
    w: u32,
    h: u32,
    texels: Vec<f32>,
}

struct Strike {
    ppem: f32,
    bearing_x: f32,
    bearing_y: f32,
    w: u32,
    h: u32,
    levels: Mutex<Vec<Arc<Level>>>,
}

const CACHE_BYTES: usize = 64 << 20;

#[derive(Default)]
struct StrikeCache {
    map: HashMap<(u16, u32), Option<Arc<Strike>>>,
    bytes: usize,
}

fn strikes() -> &'static Mutex<StrikeCache> {
    static CACHE: OnceLock<Mutex<StrikeCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(StrikeCache::default()))
}

fn strike(face: &Face, glyph: u32, size_dev: f32) -> Option<Arc<Strike>> {
    let key = (face.id, glyph);
    if let Some(s) = strikes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map
        .get(&key)
    {
        return s.clone();
    }
    let decoded = decode(face, glyph, size_dev).map(Arc::new);
    let bytes = decoded
        .as_ref()
        .map_or(0, |s| 2 * s.w as usize * s.h as usize * 16);
    let mut cache = strikes().lock().unwrap_or_else(|e| e.into_inner());
    if cache.bytes + bytes > CACHE_BYTES {
        cache.map.clear();
        cache.bytes = 0;
    }
    cache.bytes += bytes;
    cache.map.insert(key, decoded.clone());
    decoded
}

fn decode(face: &Face, glyph: u32, size_dev: f32) -> Option<Strike> {
    let font = face.font_ref();
    let strikes = BitmapStrikes::new(&font);
    let bg = strikes.glyph_for_size(Size::new(size_dev), GlyphId::new(glyph))?;
    let BitmapData::Png(data) = bg.data else {
        return None;
    };
    let mut dec = png::Decoder::new(std::io::Cursor::new(data));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut rd = dec.read_info().ok()?;
    let mut buf = vec![0; rd.output_buffer_size()?];
    let info = rd.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let mut texels = Vec::with_capacity(info.width as usize * info.height as usize * 4);
    for p in buf[..info.buffer_size()].as_chunks::<4>().0 {
        let a = p[3] as f32 / 255.0;
        texels.extend_from_slice(&[
            p[0] as f32 * a,
            p[1] as f32 * a,
            p[2] as f32 * a,
            p[3] as f32,
        ]);
    }
    Some(Strike {
        ppem: bg.ppem_y,
        bearing_x: bg.inner_bearing_x,
        bearing_y: bg.inner_bearing_y,
        w: info.width,
        h: info.height,
        levels: Mutex::new(vec![Arc::new(Level {
            w: info.width,
            h: info.height,
            texels,
        })]),
    })
}

impl Strike {
    fn level(&self, n: usize) -> Arc<Level> {
        let mut levels = self.levels.lock().unwrap_or_else(|e| e.into_inner());
        while levels.len() <= n {
            let prev = Arc::clone(&levels[levels.len() - 1]);
            levels.push(Arc::new(halve(&prev)));
        }
        Arc::clone(&levels[n])
    }
}

fn halve(l: &Level) -> Level {
    let (nw, nh) = ((l.w / 2).max(1), (l.h / 2).max(1));
    let mut texels = vec![0f32; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            for k in 0..4 {
                let g = |xx: u32, yy: u32| {
                    l.texels[((yy.min(l.h - 1) * l.w + xx.min(l.w - 1)) * 4 + k) as usize]
                };
                texels[((y * nw + x) * 4 + k) as usize] = (g(2 * x, 2 * y)
                    + g(2 * x + 1, 2 * y)
                    + g(2 * x, 2 * y + 1)
                    + g(2 * x + 1, 2 * y + 1))
                    / 4.0;
            }
        }
    }
    Level {
        w: nw,
        h: nh,
        texels,
    }
}

fn bilerp(l: &Level, u: f32, v: f32) -> [f32; 4] {
    let x0 = u.floor();
    let y0 = v.floor();
    let (fx, fy) = (u - x0, v - y0);
    let get = |x: i32, y: i32, k: usize| -> f32 {
        if x < 0 || y < 0 || x >= l.w as i32 || y >= l.h as i32 {
            0.0
        } else {
            l.texels[((y as u32 * l.w + x as u32) * 4) as usize + k]
        }
    };
    let (x, y) = (x0 as i32, y0 as i32);
    std::array::from_fn(|k| {
        get(x, y, k) * (1.0 - fx) * (1.0 - fy)
            + get(x + 1, y, k) * fx * (1.0 - fy)
            + get(x, y + 1, k) * (1.0 - fx) * fy
            + get(x + 1, y + 1, k) * fx * fy
    })
}

pub(crate) fn sprite(face: &Face, glyph: u32, size_dev: f32, x: f32, y: f32) -> Option<Sprite> {
    if !size_dev.is_finite() || size_dev <= 0.0 || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let st = strike(face, glyph, size_dev)?;
    let s = size_dev / st.ppem;
    let ox = quantize(x) + st.bearing_x * s;
    let oy = y.round() - st.bearing_y * s;
    let (dw, dh) = (st.w as f32 * s, st.h as f32 * s);
    let left = ox.floor();
    let top = oy.floor();
    let w = ((ox + dw).ceil() - left) as u32;
    let h = ((oy + dh).ceil() - top) as u32;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return None;
    }
    let mut n = 0;
    while n < 16 && s * (2u32 << n) as f32 <= 1.0 {
        n += 1;
    }
    let lvl = st.level(n);
    let (ls, lsy) = (lvl.w as f32 / st.w as f32, lvl.h as f32 / st.h as f32);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for py in 0..h {
        for px in 0..w {
            let dx = left + px as f32 + 0.5 - ox;
            let dy = top + py as f32 + 0.5 - oy;
            let acc = bilerp(&lvl, dx / s * ls - 0.5, dy / s * lsy - 0.5);
            let o = ((py * w + px) * 4) as usize;
            for k in 0..4 {
                data[o + k] = acc[k].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some(Sprite {
        w,
        h,
        left: left as i32,
        top: top as i32,
        data,
    })
}
