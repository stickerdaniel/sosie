// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba { r, g, b, a: 255 }
    }

    pub const fn hex(v: u32) -> Rgba {
        Rgba::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    pub fn alpha(self, alpha: f32) -> Rgba {
        Rgba {
            a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
            ..self
        }
    }

    pub fn parse_hex(s: &str) -> Option<Rgba> {
        let s = s.trim().strip_prefix('#')?;
        let n = |i: usize, len: usize| u8::from_str_radix(&s[i..i + len], 16).ok();
        match s.len() {
            3 => {
                let d = |i| n(i, 1).map(|v| v * 17);
                Some(Rgba::rgb(d(0)?, d(1)?, d(2)?))
            }
            6 => Some(Rgba::rgb(n(0, 2)?, n(2, 2)?, n(4, 2)?)),
            8 => Some(Rgba {
                r: n(0, 2)?,
                g: n(2, 2)?,
                b: n(4, 2)?,
                a: n(6, 2)?,
            }),
            _ => None,
        }
    }

    pub fn css(self) -> String {
        if self.a == 255 {
            format!("rgb({}, {}, {})", self.r, self.g, self.b)
        } else {
            format!(
                "rgba({}, {}, {}, {})",
                self.r,
                self.g,
                self.b,
                (self.a as f32 / 255.0 * 1000.0).round() / 1000.0
            )
        }
    }
}
