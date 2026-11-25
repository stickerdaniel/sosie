// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use crate::color::Rgba;
use crate::display::{DisplayList, GlyphPos, Item};
use crate::fonts::{FaceId, Family, FamilyList, Fonts};
use crate::{linebreak, script, shape};
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};

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
    break_before_spaces: bool,
    anywhere: bool,
    avail: f32,
    rtl_base: bool,
    strut: (f32, f32),
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

fn collapse(
    inlines: &[Inline],
    strut: &TextStyle,
    fonts: &Fonts,
) -> (String, Vec<usize>, Vec<Style>) {
    let mut text = String::new();
    let mut span_of = Vec::new();
    let mut styles = Vec::new();
    let mut prev_collapsible_space = true;
    for (i, inline) in inlines.iter().enumerate() {
        match inline {
            Inline::Text { text: t, style } => {
                let idx = styles.len();
                styles.push(resolve(fonts, style));
                for c in t.chars() {
                    let c = if style.white_space == WhiteSpace::Normal {
                        if matches!(c, ' ' | '\t' | '\n' | '\r') {
                            if prev_collapsible_space {
                                continue;
                            }
                            prev_collapsible_space = true;
                            ' '
                        } else {
                            prev_collapsible_space = false;
                            c
                        }
                    } else {
                        prev_collapsible_space = c == '\n';
                        c
                    };
                    let mut buf = [0u8; 4];
                    let s = c.encode_utf8(&mut buf);
                    text.push_str(s);
                    span_of.extend(std::iter::repeat_n(idx, s.len()));
                }
            }
            Inline::Box(b) => {
                let mut st = resolve(fonts, strut);
                st.background = None;
                st.pad = 0.0;
                st.atom = Some((i, *b));
                let idx = styles.len();
                styles.push(st);
                text.push(OBJECT);
                span_of.extend(std::iter::repeat_n(idx, OBJECT.len_utf8()));
                prev_collapsible_space = false;
            }
        }
    }
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
        let mut levels = bidi.levels.clone();
        for para in &bidi.paragraphs {
            let range = para.range.clone();
            levels[range.clone()]
                .copy_from_slice(&bidi.reordered_levels(para, range.clone())[range]);
        }
        let scripts = script::resolve(&text);
        let gbs = linebreak::grapheme_boundaries(&text);
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
            });
        }
        let n = g.len();
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
            break_before_spaces: matches!(
                para.strut.white_space,
                WhiteSpace::Normal | WhiteSpace::Pre
            ),
            anywhere: para.overflow_wrap == OverflowWrap::Anywhere,
            avail: para.width,
            rtl_base,
            strut,
        };
        p.adv = p.shape_range(0, n);
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

    fn shape_range(&self, ls: usize, le: usize) -> Vec<f32> {
        let mut adv = vec![0.0f32; le - ls];
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
        for (a, (lead, tail)) in adv.iter_mut().zip(self.han_trim(ls, le)) {
            *a -= lead + tail;
        }
        adv
    }

    fn is_cjk(&self, k: usize) -> bool {
        let g = &self.g[k];
        !g.atom && self.fonts.expect(g.face).family() == Family::NotoSansCjk
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

    fn space_width(&self, span: usize) -> f32 {
        let st = &self.styles[span];
        let mut gl = Vec::new();
        shape::shape(
            self.fonts.expect(st.primary),
            " ",
            st.size,
            false,
            None,
            &mut gl,
        );
        gl.iter().map(|g| g.adv).sum()
    }

    fn tab_advance(&self, span: usize, x: f32) -> f32 {
        let sw = self.space_width(span);
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

    fn walk(&self, ls: usize, t: usize, adv: &[f32]) -> (f32, Vec<(f32, f32)>) {
        let mut total = 0.0f32;
        let mut item = 0.0f32;
        let mut pos = Vec::with_capacity(t - ls);
        for k in ls..t {
            if k > ls && self.item_break[k] {
                total += ceil64(item);
                item = 0.0;
            }
            let g = &self.g[k];
            if g.pad_before > 0.0 {
                total += ceil64(item) + g.pad_before;
                item = 0.0;
            }
            let a = if g.tab {
                self.tab_advance(g.span, total + item)
            } else {
                adv[k - ls]
            };
            let x0 = total + item;
            item += a;
            pos.push((x0, total + item));
            if g.pad_after > 0.0 {
                total += ceil64(item) + g.pad_after;
                item = 0.0;
            }
        }
        (total + ceil64(item), pos)
    }

    fn trail(&self, ls: usize, le: usize) -> usize {
        let mut t = le;
        while t > ls && self.g[t - 1].hang {
            t -= 1;
        }
        t
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

    fn fit_width(&self, ls: usize, le: usize, reshape: bool) -> f32 {
        let t = self.trail(ls, le);
        let w = if reshape {
            let adv = self.shape_range(ls, t);
            self.walk(ls, t, &adv).0
        } else {
            self.walk(ls, t, &self.adv[ls..t]).0
        };
        if w > self.avail && t > ls {
            w - self.end_trim(t)
        } else {
            w
        }
    }

    fn breaks(&self) -> Vec<(usize, usize)> {
        let n = self.g.len();
        let mut out = Vec::new();
        if n == 0 {
            return out;
        }
        let mut hs = 0;
        for k in 0..n {
            if self.g[k].nl {
                self.break_hard(hs, k, &mut out);
                hs = k + 1;
            }
        }
        if hs < n {
            self.break_hard(hs, n, &mut out);
        }
        out
    }

    fn break_hard(&self, hs: usize, he: usize, out: &mut Vec<(usize, usize)>) {
        if hs >= he {
            out.push((hs, hs));
            return;
        }
        let a = self.g[hs].start;
        let b = self.g[he - 1].end;
        let mut opps: Vec<usize> = linebreak::line_breaks(&self.text[a..b])
            .into_iter()
            .filter_map(|o| {
                self.g[hs..he]
                    .binary_search_by_key(&(a + o), |g| g.start)
                    .ok()
                    .map(|i| hs + i)
            })
            .collect();
        for k in hs + 1..he {
            let (p, q) = (&self.g[k - 1], &self.g[k]);
            let (pw, qw) = (self.styles[p.span].ws, self.styles[q.span].ws);
            let blank = |g: &G| g.space || g.tab;
            let after_space = match pw {
                WhiteSpace::BreakSpaces => blank(p),
                WhiteSpace::PreWrap => (p.space && !q.space) || (blank(p) && blank(q) && qw != pw),
                WhiteSpace::Normal => p.space && blank(q) && qw != pw,
                _ => false,
            };
            let before_pre_space = self.break_before_spaces
                && qw == WhiteSpace::Pre
                && pw != WhiteSpace::Pre
                && blank(q)
                && !blank(p)
                && p.span != q.span;
            if after_space || before_pre_space {
                opps.push(k);
            }
        }
        let wraps = |k: usize| {
            let (p, q) = (&self.g[k - 1], &self.g[k]);
            self.styles[p.span].ws != WhiteSpace::Pre
                || (self.wrap && p.span != q.span && !p.space && !p.tab)
        };
        opps.retain(|&k| wraps(k));
        opps.sort_unstable();
        opps.dedup();
        let mut ls = hs;
        while ls < he {
            let first = opps.partition_point(|&o| o <= ls);
            let cand = |k: usize| opps.get(first + k).copied().unwrap_or(he);
            let count = opps.len() - first + 1;
            let mut best = None;
            for k in 0..count {
                if fits(self.fit_width(ls, cand(k), false), self.avail) {
                    best = Some(k);
                } else {
                    break;
                }
            }
            let mut chosen = None;
            if let Some(mut k) = best {
                loop {
                    let le = cand(k);
                    if fits(self.fit_width(ls, le, true), self.avail) {
                        chosen = Some(le);
                        break;
                    }
                    if k == 0 {
                        break;
                    }
                    k -= 1;
                }
            }
            let mut le = match chosen {
                Some(le) => le,
                None if !self.anywhere => cand(0),
                None => {
                    let mut g = ls + 1;
                    while g < he {
                        let (_, pos) = self.walk(ls, g + 1, &self.adv[ls..g + 1]);
                        if pos.last().is_some_and(|p| fits(p.1, self.avail)) {
                            g += 1;
                        } else {
                            break;
                        }
                    }
                    loop {
                        let allowed = g == he || wraps(g);
                        if allowed && (fits(self.fit_width(ls, g, true), self.avail) || g <= ls + 1)
                        {
                            break g;
                        }
                        if g <= ls + 1 {
                            break (ls + 1..he).find(|&k| wraps(k)).unwrap_or(he);
                        }
                        g -= 1;
                    }
                }
            };
            let pre_overflow = chosen.is_none()
                && self.anywhere
                && self.styles[self.g[le - 1].span].ws == WhiteSpace::Pre
                && !fits(self.fit_width(ls, le, true), self.avail);
            let after_tab = self.g[le - 1].tab;
            while le < he && self.g[le].hang {
                let span = self.g[le].span;
                let end = (le..self.g.len())
                    .find(|&k| self.g[k].span != span)
                    .unwrap_or(self.g.len());
                let run = (le..end.min(he)).take_while(|&k| self.g[k].hang).count();
                let collapsible = self.styles[span].ws == WhiteSpace::Normal;
                if !collapsible && (pre_overflow || (run < end - le && after_tab)) {
                    break;
                }
                le += run;
            }
            out.push((ls, le));
            ls = le;
        }
    }

    fn line(&self, ls: usize, le: usize, top: f32) -> Line {
        let t = self.trail(ls, le);
        let adv = self.shape_range(ls, t);
        let (full, mut pos) = self.walk(ls, t, &adv);
        if self.wrap && full > self.avail && t > ls {
            let trim = self.end_trim(t);
            if let Some(p) = pos.last_mut() {
                p.1 -= trim;
            }
        }
        let vpos = self.visual(ls, t, &pos);
        let ink: Vec<usize> = (ls..t)
            .filter(|&k| !self.g[k].space && !self.g[k].tab && !self.g[k].cr)
            .collect();
        let ink_width = match (ink.first(), ink.last()) {
            (Some(&a), Some(&b)) => pos[b - ls].1 - pos[a - ls].0,
            _ => 0.0,
        };
        let shift = if self.rtl_base {
            self.avail - full
        } else {
            0.0
        };
        let (mut above, mut below) = self.strut;
        let mut seen = Vec::new();
        for k in ls..le {
            let span = self.g[k].span;
            if seen.contains(&span) {
                continue;
            }
            seen.push(span);
            let st = &self.styles[span];
            let (a, b) = match st.atom {
                Some((_, bx)) => (bx.height, 0.0),
                None => half_leading(self.fonts, st.primary, st.size, st.line_height),
            };
            above = above.max(a);
            below = below.max(b);
        }
        let trims = self.han_trim(ls, t);
        let mut runs = Vec::new();
        let mut boxes = Vec::new();
        let mut glyphs = Vec::new();
        for (a, b) in self.items(ls, t) {
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
                    for k in ks {
                        start = start.min(vpos[k - ls].0);
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

    fn visual(&self, ls: usize, t: usize, pos: &[(f32, f32)]) -> Vec<(f32, f32)> {
        if (ls..t).all(|k| self.g[k].level.is_multiple_of(2)) {
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
        let levels: Vec<u8> = groups.iter().map(|&(a, _)| self.g[a].level).collect();
        let mut vpos = pos.to_vec();
        let mut cursor = 0.0f32;
        for gi in visual_order(&levels) {
            let (a, b) = groups[gi];
            cursor += self.g[a].pad_before;
            let left = pos[a - ls].0;
            let right = pos[b - 1 - ls].1;
            for k in a..b {
                let (x0, x1) = pos[k - ls];
                vpos[k - ls] = if levels[gi] % 2 == 1 {
                    (cursor + (right - x1), cursor + (right - x0))
                } else {
                    (cursor + (x0 - left), cursor + (x1 - left))
                };
            }
            cursor += ceil64(right - left) + self.g[b - 1].pad_after;
        }
        vpos
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
    let p = Para::new(fonts, para, inlines);
    let mut lines = Vec::new();
    let mut top = 0.0f32;
    for (ls, le) in p.breaks() {
        let line = p.line(ls, le, top);
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
