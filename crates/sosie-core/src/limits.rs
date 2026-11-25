use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub min_dpr: f32,
    pub max_dpr: f32,
    pub max_width_px: u32,
    pub max_height_px: u32,
    pub max_pixels: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            min_dpr: 1.0,
            max_dpr: 4.0,
            max_width_px: 8_192,
            max_height_px: 32_768,
            max_pixels: 64_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LimitError {
    Dpr { dpr: f32, min: f32, max: f32 },
    Size { width: f32, height: f32 },
    Width { px: u64, max: u32 },
    Height { px: u64, max: u32 },
    Pixels { pixels: u64, max: u64 },
    Bytes { pixels: u64 },
}

impl fmt::Display for LimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LimitError::Dpr { dpr, min, max } => write!(f, "dpr {dpr} is outside {min} to {max}"),
            LimitError::Size { width, height } => write!(
                f,
                "surface size {width} by {height} is not a positive finite size"
            ),
            LimitError::Width { px, max } => write!(f, "output width {px} px exceeds {max} px"),
            LimitError::Height { px, max } => write!(f, "output height {px} px exceeds {max} px"),
            LimitError::Pixels { pixels, max } => {
                write!(f, "output of {pixels} pixels exceeds {max} pixels")
            }
            LimitError::Bytes { pixels } => {
                write!(
                    f,
                    "output of {pixels} RGBA pixels exceeds the address space"
                )
            }
        }
    }
}

impl std::error::Error for LimitError {}

impl Limits {
    pub(crate) fn output(
        &self,
        width: f32,
        height: f32,
        dpr: f32,
    ) -> Result<(u32, u32, usize), LimitError> {
        if !(dpr.is_finite() && dpr >= self.min_dpr && dpr <= self.max_dpr) {
            return Err(LimitError::Dpr {
                dpr,
                min: self.min_dpr,
                max: self.max_dpr,
            });
        }
        if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
            return Err(LimitError::Size { width, height });
        }
        let device = |v: f32| (v as f64 * dpr as f64).round().max(1.0);
        let (w, h) = (device(width), device(height));
        if w > self.max_width_px as f64 {
            return Err(LimitError::Width {
                px: w as u64,
                max: self.max_width_px,
            });
        }
        if h > self.max_height_px as f64 {
            return Err(LimitError::Height {
                px: h as u64,
                max: self.max_height_px,
            });
        }
        let (w, h) = (w as u64, h as u64);
        let pixels = w.saturating_mul(h);
        if pixels > self.max_pixels {
            return Err(LimitError::Pixels {
                pixels,
                max: self.max_pixels,
            });
        }
        let bytes = usize::try_from(pixels)
            .ok()
            .and_then(|p| p.checked_mul(4))
            .ok_or(LimitError::Bytes { pixels })?;
        Ok((w as u32, h as u32, bytes))
    }
}
