use crate::color::Rgba;
use crate::fonts::FaceId;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Radii {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Radii {
    pub const fn all(r: f32) -> Radii {
        Radii {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Shape {
    pub rect: Rect,
    pub radii: Radii,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphPos {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Fill {
        shape: Shape,
        color: Rgba,
    },
    Border {
        shape: Shape,
        width: f32,
        color: Rgba,
    },
    Glyphs {
        face: FaceId,
        size: f32,
        color: Rgba,
        x: f32,
        baseline: f32,
        glyphs: Vec<GlyphPos>,
    },
    TextBackground {
        x0: f32,
        x1: f32,
        baseline: f32,
        ascent: f32,
        descent: f32,
        padding_y: f32,
        radius: f32,
        color: Rgba,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayList {
    pub width: f32,
    pub height: f32,
    pub background: Rgba,
    pub items: Vec<Item>,
}

impl DisplayList {
    pub fn new(width: f32, height: f32, background: Rgba) -> DisplayList {
        DisplayList {
            width,
            height,
            background,
            items: Vec::new(),
        }
    }

    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }
}
