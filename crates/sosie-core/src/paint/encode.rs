// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use crate::paint::Pixmap;
use std::fmt;

pub const WEBP_MAX_SIDE: u32 = 16_383;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Webp,
    Png,
}

impl Format {
    pub const fn extension(self) -> &'static str {
        match self {
            Format::Webp => "webp",
            Format::Png => "png",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodeError {
    Empty,
    Buffer {
        expected: Option<usize>,
        actual: usize,
    },
    TooLarge {
        width: u32,
        height: u32,
        max: u32,
    },
    Encoder(String),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::Empty => write!(f, "the pixmap has no pixels"),
            EncodeError::Buffer {
                expected: Some(expected),
                actual,
            } => write!(
                f,
                "the pixmap holds {actual} bytes, its size needs {expected}"
            ),
            EncodeError::Buffer {
                expected: None,
                actual,
            } => write!(
                f,
                "the pixmap holds {actual} bytes, its size exceeds the address space"
            ),
            EncodeError::TooLarge { width, height, max } => write!(
                f,
                "WebP allows at most {max} px per side, the pixmap is {width} by {height} px"
            ),
            EncodeError::Encoder(e) => write!(f, "the encoder failed: {e}"),
        }
    }
}

impl std::error::Error for EncodeError {}

pub fn encode(p: &Pixmap, format: Format) -> Result<Vec<u8>, EncodeError> {
    if p.width == 0 || p.height == 0 {
        return Err(EncodeError::Empty);
    }
    let expected = (p.width as usize)
        .checked_mul(p.height as usize)
        .and_then(|n| n.checked_mul(4));
    if expected != Some(p.rgba.len()) {
        return Err(EncodeError::Buffer {
            expected,
            actual: p.rgba.len(),
        });
    }
    let out = match format {
        Format::Png => encode_png(p),
        Format::Webp => encode_webp(p)?,
    };
    if out.is_empty() {
        return Err(EncodeError::Encoder("no bytes written".to_string()));
    }
    Ok(out)
}

fn encode_webp(p: &Pixmap) -> Result<Vec<u8>, EncodeError> {
    if p.width > WEBP_MAX_SIDE || p.height > WEBP_MAX_SIDE {
        return Err(EncodeError::TooLarge {
            width: p.width,
            height: p.height,
            max: WEBP_MAX_SIDE,
        });
    }
    let mut params = webp::EncoderParams::default();
    params.use_lossy = false;
    params.lossless_method = 4;
    params.lossless_quality = 75;
    params.lossless_exact = true;
    params.near_lossless = 100;
    params.fast_mode = false;
    let mut out = Vec::with_capacity(1024 + p.rgba.len() / 8);
    let mut enc = webp::WebPEncoder::new(&mut out);
    enc.set_params(params);
    enc.encode(&p.rgba, p.width, p.height, webp::ColorType::Rgba8)
        .map_err(|e| EncodeError::Encoder(e.to_string()))?;
    Ok(out)
}

pub fn encode_png(p: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(1024 + p.rgba.len() / 8);
    {
        let mut enc = png::Encoder::new(&mut out, p.width, p.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Balanced);
        enc.set_filter(png::Filter::Adaptive);
        let Ok(mut w) = enc.write_header() else {
            return Vec::new();
        };
        if w.write_image_data(&p.rgba).is_err() {
            return Vec::new();
        }
    }
    out
}
