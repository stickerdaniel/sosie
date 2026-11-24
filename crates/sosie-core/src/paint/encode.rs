// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use crate::paint::Pixmap;

pub fn encode_png(p: &Pixmap) -> Vec<u8> {
    encode(p, png::Compression::Fast, png::Filter::Sub)
}

fn encode(p: &Pixmap, compression: png::Compression, filter: png::Filter) -> Vec<u8> {
    let mut out = Vec::with_capacity(1024 + p.rgba.len() / 8);
    {
        let mut enc = png::Encoder::new(&mut out, p.width, p.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(compression);
        enc.set_filter(filter);
        let Ok(mut w) = enc.write_header() else {
            return Vec::new();
        };
        if w.write_image_data(&p.rgba).is_err() {
            return Vec::new();
        }
    }
    out
}
