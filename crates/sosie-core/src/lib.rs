#![forbid(unsafe_code)]

mod blink_ascii;
pub mod color;
pub mod display;
pub mod fonts;
pub mod limits;
mod linebreak;
pub mod paint;
mod script;
mod shape;
pub mod text;
#[cfg(feature = "count-work")]
#[doc(hidden)]
pub mod work;
#[cfg(not(feature = "count-work"))]
mod work;

pub use color::Rgba;
pub use display::{DisplayList, GlyphPos, Item, Radii, Rect, Shape};
pub use fonts::{FaceId, Family, FamilyList, FontError, Fonts, Pack};
pub use limits::{LimitError, Limits};
pub use paint::{encode_png, rasterize, rasterize_png, Pixmap, RenderError};
pub use text::{
    layout, Direction, Inline, InlineBackground, InlineBox, LayoutError, Line, OverflowWrap,
    Paragraph, PlacedBox, TextLayout, TextStyle, Warning, WhiteSpace, MAX_FONT_SIZE, MIN_FONT_SIZE,
};
