#![forbid(unsafe_code)]

mod blink_ascii;
pub mod chart;
pub mod color;
pub mod display;
pub mod fonts;
pub mod limits;
mod linebreak;
pub mod paint;
mod script;
mod shape;
pub mod text;

pub use chart::{
    Chart, ChartError, Metric, MetricError, Palette, Rendered, Theme, Tone, WrappedName,
};
pub use color::Rgba;
pub use display::{DisplayList, GlyphPos, Item, Radii, Rect, Shape};
pub use fonts::{FaceId, Family, FamilyList, FontError, Fonts, Pack};
pub use limits::{LimitError, Limits};
pub use paint::{
    encode, encode_png, rasterize, rasterize_png, EncodeError, Format, Pixmap, RenderError,
    WEBP_MAX_SIDE,
};
pub use text::{
    layout, Direction, Inline, InlineBackground, InlineBox, LayoutError, Line, OverflowWrap,
    Paragraph, PlacedBox, TextLayout, TextStyle, Warning, WhiteSpace, MAX_FONT_SIZE, MIN_FONT_SIZE,
};
