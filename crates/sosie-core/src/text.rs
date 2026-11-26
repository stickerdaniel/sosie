// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use crate::color::Rgba;
use crate::display::{DisplayList, GlyphPos, Item};
use crate::fonts::{FaceId, Family, FamilyList, Fonts};
use crate::{linebreak, script, shape};
use std::ops::Range;
use unicode_bidi::{BidiClass, BidiInfo, Level};

mod breaker;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WhiteSpace {
    Normal,
    Pre,
    PreWrap,
    BreakSpaces,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverflowWrap {
    Normal,
    Anywhere,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Ltr,
    Rtl,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineBackground {
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: f32,
    pub color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FamilyList,
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
    pub italic: bool,
    pub color: Rgba,
    pub white_space: WhiteSpace,
    pub tab_size: u32,
    pub background: Option<InlineBackground>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineBox {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text { text: String, style: TextStyle },
    Box(InlineBox),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Paragraph {
    pub width: f32,
    pub strut: TextStyle,
    pub overflow_wrap: OverflowWrap,
    pub direction: Direction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedBox {
    pub inline: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct Run {
    face: FaceId,
    size: f32,
    color: Rgba,
    glyphs: Vec<GlyphPos>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Background {
    x0: f32,
    x1: f32,
    ascent: f32,
    descent: f32,
    padding_y: f32,
    radius: f32,
    color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub range: Range<usize>,
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
    pub ink_width: f32,
    pub advance: f32,
    pub boxes: Vec<PlacedBox>,
    runs: Vec<Run>,
    backgrounds: Vec<Background>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Warning {
    SyntheticBoldUnsupported,
}

impl Warning {
    pub fn code(self) -> &'static str {
        match self {
            Warning::SyntheticBoldUnsupported => "synthetic-bold-unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    pub text: String,
    pub lines: Vec<Line>,
    pub height: f32,
    pub warnings: Vec<Warning>,
}

impl TextLayout {
    pub fn line_text(&self, line: &Line) -> &str {
        &self.text[line.range.clone()]
    }

    pub fn paint(&self, list: &mut DisplayList, x: f32, y: f32) {
        for line in &self.lines {
            let baseline = y + line.baseline;
            for b in &line.backgrounds {
                list.push(Item::TextBackground {
                    x0: x + b.x0,
                    x1: x + b.x1,
                    baseline,
                    ascent: b.ascent,
                    descent: b.descent,
                    padding_y: b.padding_y,
                    radius: b.radius,
                    color: b.color,
                });
            }
            for r in &line.runs {
                list.push(Item::Glyphs {
                    face: r.face,
                    size: r.size,
                    color: r.color,
                    x,
                    baseline,
                    glyphs: r.glyphs.clone(),
                });
            }
        }
    }
}

const OBJECT: char = '\u{fffc}';
const ZWSP: char = '\u{200b}';
const RESHAPES_PER_LINE: usize = 4;

#[derive(Clone)]
struct Style {
    family: FamilyList,
    primary: FaceId,
    weight: u16,
    italic: bool,
    size: f32,
    line_height: f32,
    color: Rgba,
    ws: WhiteSpace,
    pad: f32,
    tab: u32,
    background: Option<InlineBackground>,
    atom: Option<(usize, InlineBox)>,
}

fn clamp_trim(a: f32, (lead, tail): &mut (f32, f32)) -> f32 {
    let room = a.max(0.0);
    if *lead + *tail > room {
        let scale = room / (*lead + *tail);
        *lead *= scale;
        *tail *= scale;
    }
    (a - *lead - *tail).max(0.0)
}

struct Remeasure {
    t: usize,
    reshapes: usize,
    width: f32,
    raw: Vec<f32>,
    safe: Vec<bool>,
    pens: Vec<Pen>,
}

fn ceil64(w: f32) -> f32 {
    (w * 64.0).ceil() / 64.0
}

fn floor64(w: f32) -> f32 {
    (w * 64.0).floor() / 64.0
}

fn fits(width: f32, avail: f32) -> bool {
    ceil64(width) <= avail + 1.0 / 64.0
}

struct G {
    start: usize,
    end: usize,
    span: usize,
    face: FaceId,
    level: u8,
    script: u16,
    space: bool,
    tab: bool,
    nl: bool,
    cr: bool,
    hang: bool,
    atom: bool,
    pad_before: f32,
    pad_after: f32,
    gap_before: f32,
}

#[derive(Clone, Copy)]
struct Pen {
    ls: usize,
    k: usize,
    total: f32,
    item: f32,
}

impl Pen {
    fn new(ls: usize) -> Self {
        Pen {
            ls,
            k: ls,
            total: 0.0,
            item: 0.0,
        }
    }

    fn width(&self) -> f32 {
        self.total + ceil64(self.item)
    }
}

struct Para<'a> {
    fonts: &'a Fonts,
    text: String,
    styles: Vec<Style>,
    g: Vec<G>,
    adv: Vec<f32>,
    item_break: Vec<bool>,
    shape_break: Vec<bool>,
    wrap: bool,
    strut_ws: WhiteSpace,
    anywhere: bool,
    avail: f32,
    rtl_base: bool,
    strut: (f32, f32),
    tab_space: f32,
}

fn space_width(fonts: &Fonts, face: FaceId, size: f32) -> f32 {
    let mut gl = Vec::new();
    shape::shape(fonts.expect(face), " ", size, false, None, &mut gl);
    gl.iter().map(|g| g.adv).sum()
}

fn half_leading(fonts: &Fonts, face: FaceId, size: f32, line_height: f32) -> (f32, f32) {
    let f = fonts.expect(face);
    let (asc, desc) = (f.ascent * size / f.upem, f.descent * size / f.upem);
    let above = asc.round() + ((line_height - (asc + desc)) / 2.0).floor();
    (above, line_height - above)
}

fn resolve(fonts: &Fonts, s: &TextStyle) -> Style {
    Style {
        family: s.family.clone(),
        primary: fonts.primary(&s.family, s.weight, s.italic),
        weight: s.weight,
        italic: s.italic,
        size: s.size,
        line_height: s.line_height,
        color: s.color,
        ws: s.white_space,
        pad: s.background.map_or(0.0, |b| floor64(b.padding_x)),
        tab: s.tab_size,
        background: s.background,
        atom: None,
    }
}

#[derive(Clone, Copy)]
enum End {
    Start,
    Kept,
    Collapsible { newline: bool, space: bool },
    Removed { newline: bool, span: usize },
}

struct Collapsed {
    text: String,
    span_of: Vec<usize>,
}

impl Collapsed {
    fn push(&mut self, c: char, span: usize) {
        self.text.push(c);
        self.span_of.extend(std::iter::repeat_n(span, c.len_utf8()));
    }

    fn remove_space(&mut self, newline: bool) -> End {
        self.text.pop();
        let span = self.span_of.pop().unwrap_or_default();
        End::Removed { newline, span }
    }

    fn restore(&mut self, last: End) -> End {
        match last {
            End::Removed { newline, span } => {
                self.push(' ', span);
                End::Collapsible {
                    newline,
                    space: true,
                }
            }
            _ => last,
        }
    }

    fn removes_newline(&self, before_space: bool, next: Option<char>) -> bool {
        let mut rev = self.text.chars().rev();
        if before_space {
            rev.next();
        }
        rev.next() == Some(ZWSP) || next == Some(ZWSP)
    }

    fn normal(&mut self, t: &str, span: usize, last: End) -> End {
        let cs: Vec<char> = t.chars().collect();
        let run_end = |i: usize| i + cs[i..].iter().take_while(|&&c| is_collapsible(c)).count();
        let mut end = last;
        let mut i = 0;
        if cs.first().is_some_and(|&c| is_collapsible(c)) {
            let e = run_end(0);
            let mut newline = cs[..e].contains(&'\n');
            let next = cs.get(e).copied();
            if let End::Collapsible {
                newline: last_newline,
                space: true,
            } = last
            {
                if (newline || last_newline) && self.removes_newline(true, next) {
                    end = self.remove_space(last_newline);
                    newline = false;
                }
            }
            let insert =
                matches!(last, End::Kept) && !(newline && self.removes_newline(false, next));
            if insert {
                self.push(' ', span);
                end = End::Collapsible {
                    newline,
                    space: true,
                };
            }
            i = e;
        } else if let (
            Some(&c),
            End::Collapsible {
                newline: true,
                space: true,
            },
        ) = (cs.first(), last)
        {
            if self.removes_newline(true, Some(c)) {
                self.remove_space(true);
            }
        }
        while i < cs.len() {
            if !is_collapsible(cs[i]) {
                self.push(cs[i], span);
                end = End::Kept;
                i += 1;
                continue;
            }
            let e = run_end(i);
            let newline = cs[i..e].contains(&'\n');
            if newline && self.removes_newline(false, cs.get(e).copied()) {
                end = End::Kept;
            } else {
                self.push(' ', span);
                end = End::Collapsible {
                    newline,
                    space: true,
                };
            }
            i = e;
        }
        end
    }
}

fn is_collapsible(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

fn collapse(
    inlines: &[Inline],
    strut: &TextStyle,
    fonts: &Fonts,
) -> (String, Vec<usize>, Vec<Style>) {
    let mut out = Collapsed {
        text: String::new(),
        span_of: Vec::new(),
    };
    let mut styles = Vec::new();
    let mut last = End::Start;
    for (i, inline) in inlines.iter().enumerate() {
        let idx = styles.len();
        match inline {
            Inline::Text { text: t, style } => {
                styles.push(resolve(fonts, style));
                if t.is_empty() {
                    continue;
                }
                last = out.restore(last);
                if style.white_space == WhiteSpace::Normal {
                    last = out.normal(t, idx, last);
                    continue;
                }
                for c in t.chars() {
                    out.push(c, idx);
                    last = if c == '\n' {
                        End::Collapsible {
                            newline: false,
                            space: false,
                        }
                    } else {
                        End::Kept
                    };
                }
            }
            Inline::Box(b) => {
                let mut st = resolve(fonts, strut);
                st.background = None;
                st.pad = 0.0;
                st.atom = Some((i, *b));
                styles.push(st);
                out.restore(last);
                out.push(OBJECT, idx);
                last = End::Kept;
            }
        }
    }
    let Collapsed { text, mut span_of } = out;
    span_of.push(styles.len().saturating_sub(1));
    (text, span_of, styles)
}

impl<'a> Para<'a> {
    fn new(fonts: &'a Fonts, para: &Paragraph, inlines: &[Inline]) -> Self {
        let (text, span_of, styles) = collapse(inlines, &para.strut, fonts);
        let mut span_ranges = vec![(usize::MAX, 0usize); styles.len()];
        for (b, &s) in span_of.iter().enumerate().take(text.len()) {
            span_ranges[s].0 = span_ranges[s].0.min(b);
            span_ranges[s].1 = span_ranges[s].1.max(b + 1);
        }
        let base = match para.direction {
            Direction::Ltr => Some(Level::ltr()),
            Direction::Rtl => Some(Level::rtl()),
            Direction::Auto => None,
        };
        let bidi = BidiInfo::new(&text, base);
        let rtl_base = bidi.paragraphs.first().is_some_and(|p| p.level.is_rtl());
        let mut levels = line_levels(&bidi);
        boundary_neutral_levels(&bidi, &mut levels);
        carriage_return_levels(&text, &span_of, &mut levels);
        let scripts = script::resolve(&text);
        let mut gbs = linebreak::grapheme_boundaries(&text);
        gbs.extend(
            text.match_indices("\r\n")
                .map(|(i, _)| i + 1)
                .filter(|&i| span_of[i - 1] != span_of[i]),
        );
        gbs.sort_unstable();
        let mut g = Vec::new();
        for w in gbs.windows(2) {
            let (a, b) = (w[0], w[1]);
            let c = &text[a..b];
            let span = span_of[a];
            let st = &styles[span];
            let space = c == " ";
            let tab = c == "\t";
            let nl = c == "\n" || c == "\r\n";
            let cr = c == "\r";
            let atom = st.atom.is_some();
            let hang = match st.ws {
                WhiteSpace::Normal => space,
                WhiteSpace::PreWrap => space || tab,
                _ => false,
            };
            let (ss, se) = span_ranges[span];
            g.push(G {
                start: a,
                end: b,
                span,
                face: if space || tab || nl || cr || atom {
                    st.primary
                } else {
                    fonts.select(st.primary, &st.family, st.weight, st.italic, c)
                },
                level: levels[a].number(),
                script: scripts[a],
                space,
                tab,
                nl,
                cr,
                hang,
                atom,
                pad_before: if a == ss { st.pad } else { 0.0 },
                pad_after: if b == se { st.pad } else { 0.0 },
                gap_before: 0.0,
            });
        }
        let n = g.len();
        for (span, st) in styles.iter().enumerate() {
            if span_ranges[span].0 == usize::MAX && st.pad > 0.0 {
                let k = g.partition_point(|x| x.span < span);
                if let Some(next) = g.get_mut(k) {
                    next.gap_before += 2.0 * st.pad;
                }
            }
        }
        let mut item_break = vec![false; n];
        let mut shape_break = vec![false; n];
        for i in 1..n {
            let (p, q) = (&g[i - 1], &g[i]);
            let hard = p.level != q.level || p.tab || q.tab || p.nl || q.nl || p.cr || q.cr;
            item_break[i] = hard || p.span != q.span;
            shape_break[i] = hard
                || p.atom
                || q.atom
                || p.face != q.face
                || p.script != q.script
                || p.pad_after > 0.0
                || q.pad_before > 0.0
                || q.gap_before > 0.0
                || styles[p.span].size != styles[q.span].size;
        }
        let strut_style = resolve(fonts, &para.strut);
        let strut = half_leading(
            fonts,
            strut_style.primary,
            para.strut.size,
            para.strut.line_height,
        );
        let mut p = Para {
            fonts,
            text,
            styles,
            g,
            adv: Vec::new(),
            item_break,
            shape_break,
            wrap: para.strut.white_space != WhiteSpace::Pre,
            strut_ws: para.strut.white_space,
            anywhere: para.overflow_wrap == OverflowWrap::Anywhere,
            avail: para.width,
            rtl_base,
            strut,
            tab_space: space_width(fonts, strut_style.primary, para.strut.size),
        };
        p.adv = p.shape_range(0, n).0;
        p
    }

    fn items(&self, ls: usize, le: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut s = ls;
        for i in ls + 1..le {
            if self.shape_break[i] {
                out.push((s, i));
                s = i;
            }
        }
        if s < le {
            out.push((s, le));
        }
        out
    }

    fn shape_into(&self, a: usize, b: usize, glyphs: &mut Vec<shape::ShapedGlyph>) {
        let first = &self.g[a];
        let st = &self.styles[first.span];
        let sub = &self.text[first.start..self.g[b - 1].end];
        glyphs.clear();
        shape::shape(
            self.fonts.expect(first.face),
            sub,
            st.size,
            first.level % 2 == 1,
            script::hb_script(first.script),
            glyphs,
        );
    }

    fn shape_range(&self, ls: usize, le: usize) -> (Vec<f32>, Vec<(f32, f32)>) {
        let mut adv = self.raw_range(ls, le, None);
        let mut trims = self.han_trim(ls, le);
        for (a, trim) in adv.iter_mut().zip(&mut trims) {
            *a = clamp_trim(*a, trim);
        }
        (adv, trims)
    }

    fn raw_range(&self, ls: usize, le: usize, mut safe: Option<&mut Vec<bool>>) -> Vec<f32> {
        let mut adv = vec![0.0f32; le - ls];
        if let Some(safe) = safe.as_deref_mut() {
            *safe = vec![true; le - ls + 1];
        }
        let mut glyphs = Vec::new();
        for (a, b) in self.items(ls, le) {
            let first = &self.g[a];
            if first.tab || first.nl || first.cr {
                continue;
            }
            if let Some((_, bx)) = self.styles[first.span].atom {
                adv[a - ls] = bx.width;
                continue;
            }
            let (ta, tb) = (first.start, self.g[b - 1].end);
            self.shape_into(a, b, &mut glyphs);
            if let Some(safe) = safe.as_deref_mut() {
                let mut starts: Vec<(usize, bool)> = glyphs
                    .iter()
                    .map(|gl| (gl.cluster as usize, gl.unsafe_to_break))
                    .collect();
                starts.sort_unstable_by_key(|&(c, _)| c);
                starts.dedup_by(|next, kept| {
                    let same = next.0 == kept.0;
                    if same {
                        kept.1 |= next.1;
                    }
                    same
                });
                let guessed = script::hb_script(first.script).is_none();
                for k in a + 1..b {
                    let at = self.g[k].start - ta;
                    let clean = starts
                        .binary_search_by_key(&at, |&(c, _)| c)
                        .is_ok_and(|i| !starts[i].1);
                    safe[k - ls] = clean && !guessed;
                }
            }
            let mut clusters: Vec<(usize, f32)> = Vec::new();
            let mut push = |gl: &shape::ShapedGlyph| match clusters.last_mut() {
                Some((c, sum)) if *c == gl.cluster as usize => *sum += gl.adv,
                _ => clusters.push((gl.cluster as usize, gl.adv)),
            };
            if first.level % 2 == 1 {
                glyphs.iter().rev().for_each(&mut push);
            } else {
                glyphs.iter().for_each(&mut push);
            }
            let bounds: Vec<usize> = (a..=b)
                .map(|k| {
                    if k == b {
                        tb - ta
                    } else {
                        self.g[k].start - ta
                    }
                })
                .collect();
            let m = b - a;
            for ci in 0..clusters.len() {
                let (c, sum) = clusters[ci];
                let next = clusters.get(ci + 1).map_or(tb - ta, |p| p.0);
                let g0 = bounds.partition_point(|&o| o < c).min(m - 1);
                let g1 = bounds.partition_point(|&o| o < next).max(g0 + 1).min(m);
                let share = sum / (g1 - g0) as f32;
                for k in g0..g1 {
                    adv[a - ls + k] += share;
                }
            }
        }
        adv
    }

    fn is_cjk(&self, k: usize) -> bool {
        let g = &self.g[k];
        !g.atom && self.fonts.expect(g.face).family() == Family::NotoSansCjk
    }

    fn last_trim(&self, ls: usize, k: usize) -> (f32, f32) {
        let from = k.saturating_sub(1).max(ls);
        self.han_trim(from, k + 1)[k - from]
    }

    fn han_trim(&self, ls: usize, le: usize) -> Vec<(f32, f32)> {
        #[derive(PartialEq, Clone, Copy)]
        enum K {
            Open,
            Close,
            Dot,
            Middle,
            Other,
        }
        let kind = |k: usize| -> K {
            if !self.is_cjk(k) {
                return K::Other;
            }
            let g = &self.g[k];
            match &self.text[g.start..g.end] {
                "「" | "『" | "（" | "【" | "〔" | "〈" | "《" | "［" | "｛" | "〘" | "〖" => {
                    K::Open
                }
                "」" | "』" | "）" | "】" | "〕" | "〉" | "》" | "］" | "｝" | "〙" | "〗" => {
                    K::Close
                }
                "、" | "。" | "，" | "．" => K::Dot,
                "・" | "：" | "；" => K::Middle,
                _ => K::Other,
            }
        };
        let mut trim = vec![(0.0f32, 0.0f32); le.saturating_sub(ls)];
        for k in ls + 1..le {
            let (a, b) = (kind(k - 1), kind(k));
            let half = |k: usize| self.styles[self.g[k].span].size / 2.0;
            if b == K::Open && a != K::Other {
                trim[k - ls].0 += half(k);
            } else if matches!(a, K::Close | K::Dot) && matches!(b, K::Close | K::Dot | K::Middle) {
                trim[k - 1 - ls].1 += half(k - 1);
            }
        }
        for k in ls..le {
            if self.g[k].level % 2 == 1 && matches!(kind(k), K::Open | K::Close) {
                let (lead, tail) = trim[k - ls];
                trim[k - ls] = (tail, lead);
            }
        }
        trim
    }

    fn tab_advance(&self, span: usize, x: f32) -> f32 {
        let sw = self.tab_space;
        let tw = sw * self.styles[span].tab as f32;
        if !(tw > 0.0 && tw.is_finite() && x.is_finite()) {
            return 0.0;
        }
        let mut d = tw - x.rem_euclid(tw);
        if d < sw / 2.0 {
            d += tw;
        }
        d
    }

    fn step(&self, pen: &mut Pen, adv: &[f32]) -> (f32, f32) {
        let ls = pen.ls;
        self.step_by(pen, |k| adv[k - ls])
    }

    fn step_by(&self, pen: &mut Pen, adv: impl Fn(usize) -> f32) -> (f32, f32) {
        let k = pen.k;
        if k > pen.ls && self.item_break[k] {
            pen.total += ceil64(pen.item);
            pen.item = 0.0;
        }
        let g = &self.g[k];
        if g.gap_before > 0.0 {
            pen.total += ceil64(pen.item) + g.gap_before;
            pen.item = 0.0;
        }
        if g.pad_before > 0.0 {
            pen.total += ceil64(pen.item) + g.pad_before;
            pen.item = 0.0;
        }
        let a = if g.tab {
            self.tab_advance(g.span, pen.total + pen.item)
        } else {
            adv(k)
        };
        let x0 = pen.total + pen.item;
        pen.item += a;
        let x1 = pen.total + pen.item;
        if g.pad_after > 0.0 {
            pen.total += ceil64(pen.item) + g.pad_after;
            pen.item = 0.0;
        }
        pen.k += 1;
        (x0, x1)
    }

    fn walk(&self, ls: usize, t: usize, adv: &[f32]) -> (f32, Vec<(f32, f32)>) {
        let mut pen = Pen::new(ls);
        let pos = (ls..t).map(|_| self.step(&mut pen, adv)).collect();
        (pen.width(), pos)
    }

    fn trail(&self, ls: usize, mut le: usize) -> usize {
        while le > ls && self.g[le - 1].hang {
            le -= 1;
        }
        le
    }

    fn end_trim(&self, t: usize) -> f32 {
        if t == 0 || !self.is_cjk(t - 1) {
            return 0.0;
        }
        let g = &self.g[t - 1];
        match &self.text[g.start..g.end] {
            "」" | "』" | "）" | "】" | "〕" | "〉" | "》" | "］" | "｝" | "〙" | "〗" => {
                self.styles[g.span].size / 2.0
            }
            _ => 0.0,
        }
    }

    fn remeasure(&self, ls: usize, le: usize, memo: &mut Option<Remeasure>) -> f32 {
        let t = self.trail(ls, le);
        if t == ls {
            return 0.0;
        }
        if let Some(r) = memo
            .as_ref()
            .filter(|r| t <= r.t && (t == r.t || r.safe[t - ls] || r.reshapes >= RESHAPES_PER_LINE))
        {
            if t == r.t {
                return r.width;
            }
            let k = t - 1;
            let mut trim = self.last_trim(ls, k);
            let a = clamp_trim(r.raw[k - ls], &mut trim);
            let mut pen = r.pens[k - ls];
            self.step_by(&mut pen, |_| a);
            return self.trimmed_width(pen.width(), ls, t);
        }
        let mut safe = Vec::new();
        let raw = self.raw_range(ls, t, Some(&mut safe));
        let mut trims = self.han_trim(ls, t);
        let adv: Vec<f32> = raw
            .iter()
            .zip(&mut trims)
            .map(|(&a, trim)| clamp_trim(a, trim))
            .collect();
        let mut pen = Pen::new(ls);
        let mut pens = Vec::with_capacity(t - ls + 1);
        pens.push(pen);
        for _ in ls..t {
            self.step(&mut pen, &adv);
            pens.push(pen);
        }
        let width = self.trimmed_width(pen.width(), ls, t);
        let reshapes = memo.as_ref().map_or(0, |r| r.reshapes + 1);
        *memo = Some(Remeasure {
            t,
            reshapes,
            width,
            raw,
            safe,
            pens,
        });
        width
    }

    fn trimmed_width(&self, w: f32, ls: usize, t: usize) -> f32 {
        if w > self.avail && t > ls {
            w - self.end_trim(t)
        } else {
            w
        }
    }

    fn place_gaps(&mut self, breaks: &[breaker::Break]) {
        for b in breaks {
            let ls = b.start;
            let Some(first) = self.g.get(ls) else {
                continue;
            };
            if first.gap_before == 0.0 {
                continue;
            }
            let lo = ls.checked_sub(1).map_or(0, |k| self.g[k].span + 1);
            let gap = (lo..first.span)
                .filter(|s| b.spans.binary_search(s).is_ok())
                .map(|s| 2.0 * self.styles[s].pad)
                .sum();
            self.g[ls].gap_before = gap;
        }
    }

    fn breaks(&self) -> Vec<breaker::Break> {
        if self.g.is_empty() {
            return Vec::new();
        }
        breaker::Breaker::new(self).run()
    }

    fn line(&self, b: &breaker::Break, top: f32) -> Line {
        let (ls, le, spans) = (b.start, b.end, &b.spans);
        let t = self.trail(ls, le);
        let skip = b.skip.filter(|&k| k < t);
        let (mut adv, mut trims) = self.shape_range(ls, t);
        if let Some(k) = skip {
            adv[k - ls] = 0.0;
        }
        let (full, mut pos) = self.walk(ls, t, &adv);
        let hang_pad: f32 = (t..le)
            .filter(|&k| {
                t > ls
                    && self.g[k].span == self.g[t - 1].span
                    && self.styles[self.g[k].span].ws == WhiteSpace::Normal
            })
            .map(|k| self.g[k].pad_after)
            .sum();
        let full = full + hang_pad;
        let soft = le < self.g.len() && !self.g[le].nl;
        let trim = if (self.wrap || soft) && full > self.avail && t > ls {
            self.end_trim(t)
        } else {
            0.0
        };
        if let Some(p) = pos.last_mut() {
            p.1 -= trim;
        }
        let vpos = self.visual(ls, t, le, hang_pad, skip, &pos);
        let ink: Vec<usize> = (ls..t)
            .filter(|&k| !self.g[k].space && !self.g[k].tab && !self.g[k].cr)
            .collect();
        let ink_width = match (ink.first(), ink.last()) {
            (Some(&a), Some(&b)) => pos[b - ls].1 - pos[a - ls].0,
            _ => 0.0,
        };
        let shift = if self.rtl_base {
            self.avail - (full - trim)
        } else {
            0.0
        };
        let (mut above, mut below) = self.strut;
        let mut prev = None;
        let graphemes = (ls..le).map(|k| self.g[k].span);
        for span in graphemes.chain(spans.iter().copied()) {
            if prev == Some(span) {
                continue;
            }
            prev = Some(span);
            let st = &self.styles[span];
            let (a, b) = match st.atom {
                Some((_, bx)) => (bx.height, 0.0),
                None => half_leading(self.fonts, st.primary, st.size, st.line_height),
            };
            above = above.max(a);
            below = below.max(b);
        }
        if let Some(last) = trims.last_mut() {
            if self.g[t - 1].level % 2 == 1 {
                last.0 += trim;
            } else {
                last.1 += trim;
            }
        }
        let mut runs = Vec::new();
        let mut boxes = Vec::new();
        let mut glyphs = Vec::new();
        let items = self.items(ls, t).into_iter().flat_map(|(a, b)| match skip {
            Some(k) if (a..b).contains(&k) => [(a, k), (k + 1, b)],
            _ => [(a, b), (b, b)],
        });
        for (a, b) in items.filter(|(a, b)| a < b) {
            let first = &self.g[a];
            if first.tab || first.nl || first.cr {
                continue;
            }
            let st = &self.styles[first.span];
            if let Some((inline, bx)) = st.atom {
                boxes.push(PlacedBox {
                    inline,
                    x: shift + vpos[a - ls].0,
                    y: top + above - bx.height,
                    width: bx.width,
                    height: bx.height,
                });
                continue;
            }
            self.shape_into(a, b, &mut glyphs);
            let origin = first.start;
            let end = self.g[b - 1].end - origin;
            let mut anchors: Vec<usize> = glyphs.iter().map(|g| g.cluster as usize).collect();
            anchors.sort_unstable();
            anchors.dedup();
            let index = |at: usize| a + self.g[a..b].partition_point(|g| g.start - origin < at);
            let graphemes = |cluster: u32| {
                let c = cluster as usize;
                let next = anchors[anchors.partition_point(|&x| x <= c)..]
                    .first()
                    .copied()
                    .unwrap_or(end);
                let first = (index(c + 1) - 1).max(a);
                first..index(next).max(first + 1)
            };
            let mut i = 0;
            while i < glyphs.len() {
                let span = self.g[graphemes(glyphs[i].cluster).start].span;
                let mut j = i;
                let mut start = f32::INFINITY;
                while j < glyphs.len() {
                    let ks = graphemes(glyphs[j].cluster);
                    if self.g[ks.start].span != span {
                        break;
                    }
                    if j == i || glyphs[j - 1].cluster != glyphs[j].cluster {
                        for k in ks {
                            start = start.min(vpos[k - ls].0);
                        }
                    }
                    j += 1;
                }
                let mut x = shift + start;
                let mut out = Vec::with_capacity(j - i);
                let mut after = 0.0;
                for g in i..j {
                    let gl = &glyphs[g];
                    if g == 0 || glyphs[g - 1].cluster != gl.cluster {
                        let before;
                        (before, after) = graphemes(gl.cluster).fold((0.0, 0.0), |(b, a), k| {
                            (b + trims[k - ls].0, a + trims[k - ls].1)
                        });
                        x -= before;
                    }
                    out.push(GlyphPos {
                        id: gl.id,
                        x: x + gl.dx,
                        y: -gl.dy,
                    });
                    x += gl.adv;
                    if glyphs.get(g + 1).is_none_or(|h| h.cluster != gl.cluster) {
                        x -= after;
                    }
                }
                runs.push(Run {
                    face: first.face,
                    size: st.size,
                    color: self.styles[span].color,
                    glyphs: out,
                });
                i = j;
            }
        }
        let mut backgrounds = Vec::new();
        let mut k = ls;
        while k < t {
            let span = self.g[k].span;
            let s = k;
            while k < t && self.g[k].span == span {
                k += 1;
            }
            let st = &self.styles[span];
            if let Some(bg) = st.background {
                let face = self.fonts.expect(st.primary);
                let (ascent, descent) = face.rounded_metrics(st.size);
                let x0 = (s..k).map(|j| vpos[j - ls].0).fold(f32::INFINITY, f32::min)
                    - self.g[s].pad_before;
                let x1 = (s..k)
                    .map(|j| vpos[j - ls].1)
                    .fold(f32::NEG_INFINITY, f32::max)
                    + self.g[k - 1].pad_after;
                backgrounds.push(Background {
                    x0: shift + x0,
                    x1: shift + x1,
                    ascent,
                    descent,
                    padding_y: bg.padding_y,
                    radius: bg.radius,
                    color: bg.color,
                });
            }
        }
        let range = if le > ls {
            self.g[ls].start..self.g[le - 1].end
        } else {
            let at = self.g.get(ls).map_or(self.text.len(), |g| g.start);
            at..at
        };
        Line {
            range,
            top,
            height: above + below,
            baseline: top + above,
            ink_width,
            advance: full,
            boxes,
            runs,
            backgrounds,
        }
    }

    fn visual(
        &self,
        ls: usize,
        t: usize,
        le: usize,
        hang_pad: f32,
        skip: Option<usize>,
        pos: &[(f32, f32)],
    ) -> Vec<(f32, f32)> {
        if !self.rtl_base && (ls..t).all(|k| self.g[k].level.is_multiple_of(2)) {
            return pos.to_vec();
        }
        let mut groups: Vec<(usize, usize)> = Vec::new();
        for k in ls..t {
            if k == ls || self.item_break[k] {
                groups.push((k, k + 1));
            } else if let Some(g) = groups.last_mut() {
                g.1 = k + 1;
            }
        }
        let base = u8::from(self.rtl_base);
        let tail = (ls..le)
            .rev()
            .take_while(|&k| self.g[k].cr)
            .last()
            .unwrap_or(t);
        let mut levels: Vec<u8> = groups
            .iter()
            .map(|&(a, _)| if a >= tail { base } else { self.g[a].level })
            .collect();
        if let Some(gi) = groups
            .iter()
            .position(|&(a, b)| Some(a) == skip && b == a + 1)
        {
            if let Some(&next) = levels.get(gi + 1) {
                levels[gi] = next;
                for j in (0..gi).rev().take_while(|&j| self.g[groups[j].0].cr) {
                    levels[j] = next;
                }
            }
        }
        let mut vpos = pos.to_vec();
        let mut cursor = 0.0f32;
        let order = visual_order(&levels);
        let span = |gi: usize| self.g[groups[gi].0].span;
        for run in order.chunk_by(|&x, &y| span(x) == span(y)) {
            let start: f32 = run.iter().map(|&gi| self.g[groups[gi].0].pad_before).sum();
            let end: f32 = run
                .iter()
                .map(|&gi| {
                    let b = groups[gi].1;
                    self.g[b - 1].pad_after + if b == t { hang_pad } else { 0.0 }
                })
                .sum();
            let (pad_left, pad_right) = if self.rtl_base {
                (end, start)
            } else {
                (start, end)
            };
            cursor += pad_left;
            for &gi in run {
                let (a, b) = groups[gi];
                let odd = levels[gi] % 2 == 1;
                if !odd {
                    cursor += self.g[a].gap_before;
                }
                let left = pos[a - ls].0;
                let right = pos[b - 1 - ls].1;
                for k in a..b {
                    let (x0, x1) = pos[k - ls];
                    vpos[k - ls] = if odd {
                        (cursor + (right - x1), cursor + (right - x0))
                    } else {
                        (cursor + (x0 - left), cursor + (x1 - left))
                    };
                }
                cursor += ceil64(right - left);
                if odd {
                    cursor += self.g[a].gap_before;
                }
            }
            cursor += pad_right;
        }
        vpos
    }
}

fn line_levels(bidi: &BidiInfo) -> Vec<Level> {
    use BidiClass::{B, BN, FSI, LRE, LRI, LRO, PDF, PDI, RLE, RLI, RLO, S, WS};
    let mut levels = bidi.levels.clone();
    for para in &bidi.paragraphs {
        let range = para.range.clone();
        let mut reset_from = Some(range.start);
        let mut prev = para.level;
        for (offset, c) in bidi.text[range.clone()].char_indices() {
            let i = range.start + offset;
            let end = i + c.len_utf8();
            match bidi.original_classes[i] {
                B | S => {
                    levels[reset_from.unwrap_or(i)..end].fill(para.level);
                    reset_from = None;
                }
                WS | FSI | LRI | RLI | PDI => {
                    reset_from.get_or_insert(i);
                }
                RLE | LRE | RLO | LRO | PDF | BN => {
                    reset_from.get_or_insert(i);
                    levels[i..end].fill(prev);
                }
                _ => reset_from = None,
            }
            prev = levels[i];
        }
        if let Some(from) = reset_from {
            levels[from..range.end].fill(para.level);
        }
    }
    levels
}

fn boundary_neutral_levels(bidi: &BidiInfo, levels: &mut [Level]) {
    let mut next = None;
    for (i, _) in bidi.text.char_indices().rev() {
        match bidi.original_classes[i] {
            BidiClass::BN => {
                if let Some(level) = next {
                    levels[i] = level;
                }
            }
            BidiClass::B | BidiClass::S => next = None,
            _ => next = Some(levels[i]),
        }
    }
}

fn carriage_return_levels(text: &str, span_of: &[usize], levels: &mut [Level]) {
    let mut next = None;
    let mut found = Vec::new();
    for (i, c) in text.char_indices().rev() {
        if c == '\r' {
            found.push((i, next));
        } else {
            next = Some((i, c));
        }
    }
    for (i, next) in found.into_iter().rev() {
        let inner = next
            .filter(|&(k, c)| c != '\n' && span_of[k] == span_of[i])
            .map(|(k, _)| k);
        let before = text[..i]
            .char_indices()
            .next_back()
            .filter(|&(_, c)| c != '\n')
            .map(|(j, _)| j);
        let own = before.filter(|&j| span_of[j] == span_of[i]);
        if let Some(j) = inner.or(own).or(next.map(|(k, _)| k)).or(before) {
            levels[i] = levels[j];
        }
    }
}

fn visual_order(levels: &[u8]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..levels.len()).collect();
    let Some(&max) = levels.iter().max() else {
        return order;
    };
    let min_odd = levels
        .iter()
        .copied()
        .filter(|l| l % 2 == 1)
        .min()
        .unwrap_or(max + 1);
    let mut lvl = max;
    while lvl >= min_odd && lvl > 0 {
        let mut i = 0;
        while i < order.len() {
            if levels[order[i]] >= lvl {
                let s = i;
                while i < order.len() && levels[order[i]] >= lvl {
                    i += 1;
                }
                order[s..i].reverse();
            } else {
                i += 1;
            }
        }
        lvl -= 1;
    }
    order
}

pub const MIN_FONT_SIZE: f32 = 0.0;
pub const MAX_FONT_SIZE: f32 = 256.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayoutError {
    FontSize { size: f32, min: f32, max: f32 },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::FontSize { size, min, max } => {
                write!(f, "font size {size} px is outside {min} to {max} px")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

fn check_size(size: f32) -> Result<(), LayoutError> {
    if (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&size) {
        Ok(())
    } else {
        Err(LayoutError::FontSize {
            size,
            min: MIN_FONT_SIZE,
            max: MAX_FONT_SIZE,
        })
    }
}

pub fn layout(
    fonts: &Fonts,
    para: &Paragraph,
    inlines: &[Inline],
) -> Result<TextLayout, LayoutError> {
    check_size(para.strut.size)?;
    for inline in inlines {
        if let Inline::Text { style, .. } = inline {
            check_size(style.size)?;
        }
    }
    let mut p = Para::new(fonts, para, inlines);
    let mut lines = Vec::new();
    let mut top = 0.0f32;
    let breaks = p.breaks();
    p.place_gaps(&breaks);
    for b in &breaks {
        let line = p.line(b, top);
        top += line.height;
        lines.push(line);
    }
    let synthetic_bold = p.g.iter().any(|g| {
        let face = fonts.expect(g.face);
        !(g.space || g.tab || g.nl || g.cr || g.atom)
            && p.styles[g.span].weight >= 600
            && face.weight() < 600
            && !face.is_emoji()
    });
    Ok(TextLayout {
        text: p.text,
        lines,
        height: top,
        warnings: synthetic_bold
            .then_some(Warning::SyntheticBoldUnsupported)
            .into_iter()
            .collect(),
    })
}
