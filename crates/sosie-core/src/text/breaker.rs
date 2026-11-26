use super::{ceil64, fits, Para, Pen, Remeasure, WhiteSpace, G};
use crate::linebreak;
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
    Text,
    Tab,
    Cr,
    Zwsp,
    Atom,
    Lf,
}

#[derive(Clone, Copy)]
struct Item {
    kind: Kind,
    a: usize,
    b: usize,
    span: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Continue,
    Trailing,
    Overflow,
    Done,
}

pub(super) struct Break {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) skip: Option<usize>,
    pub(super) spans: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Res {
    item: usize,
    a: usize,
    b: usize,
    w: f32,
    cba: bool,
    trailing: bool,
    shaped: bool,
    inside: bool,
    bao: bool,
    lb: bool,
}

struct Shaped {
    end: usize,
    width: f32,
    overflow: bool,
    trailing: bool,
}

fn sp(g: &G) -> bool {
    g.space || g.tab || g.nl
}

fn bsp(g: &G) -> bool {
    g.space || g.tab
}

pub(super) struct Breaker<'b, 'a> {
    p: &'b Para<'a>,
    items: Vec<Item>,
    pad: Vec<f32>,
    zw: Vec<bool>,
    hang: Vec<usize>,
    icu: Vec<bool>,
    fit: f32,
    item: usize,
    off: usize,
    style: Option<usize>,
    auto_wrap: bool,
    bao: bool,
    every: bool,
    char_break: bool,
    over: bool,
    state: State,
    leading: bool,
    skip: Option<(usize, usize)>,
    forced: bool,
    res: Vec<Res>,
    position: f32,
    ls: usize,
    start: (usize, usize),
    line_style: Option<usize>,
    pens: RefCell<Vec<Pen>>,
    memo: Option<Remeasure>,
}

impl<'b, 'a> Breaker<'b, 'a> {
    pub(super) fn new(p: &'b Para<'a>) -> Self {
        let n = p.g.len();
        let mut count = vec![0usize; p.styles.len()];
        for g in &p.g {
            count[g.span] += 1;
        }
        let mut items = Vec::new();
        let mut zw = vec![false; n + 1];
        let mut pos = 0;
        for (s, st) in p.styles.iter().enumerate() {
            let end = pos + count[s];
            if st.atom.is_some() {
                items.extend((pos..end).map(|k| Item {
                    kind: Kind::Atom,
                    a: k,
                    b: k + 1,
                    span: s,
                }));
                pos = end;
                continue;
            }
            let item = |kind, a, b| Item {
                kind,
                a,
                b,
                span: s,
            };
            items.push(item(Kind::Open, pos, pos));
            let leading = matches!(st.ws, WhiteSpace::PreWrap | WhiteSpace::BreakSpaces);
            let mut k = pos;
            while k < end {
                let g = &p.g[k];
                if g.nl {
                    if p.text[g.start..g.end].starts_with('\r') {
                        items.push(item(Kind::Cr, k, k));
                    }
                    items.push(item(Kind::Lf, k, k + 1));
                    k += 1;
                    continue;
                }
                if g.cr {
                    items.push(item(Kind::Cr, k, k + 1));
                    k += 1;
                    continue;
                }
                if g.tab {
                    let mut e = k + 1;
                    while e < end && p.g[e].tab && p.g[e].level == g.level {
                        e += 1;
                    }
                    items.push(item(Kind::Tab, k, e));
                    k = e;
                    continue;
                }
                if leading && g.space && (k == 0 || p.g[k - 1].nl) {
                    let run = (k..end).find(|&e| !p.g[e].space).unwrap_or(end);
                    while k < run {
                        let e = (k + 1..run).find(|&e| p.item_break[e]).unwrap_or(run);
                        items.push(item(Kind::Text, k, e));
                        k = e;
                    }
                    zw[run] = true;
                    items.push(item(Kind::Zwsp, run, run));
                    continue;
                }
                let e = (k + 1..end).find(|&e| p.item_break[e]).unwrap_or(end);
                items.push(item(Kind::Text, k, e));
                k = e;
            }
            items.push(item(Kind::Close, end, end));
            pos = end;
        }
        let mut icu = vec![false; n + 1];
        let mut hs = 0;
        for k in 0..=n {
            if k < n && !p.g[k].nl {
                continue;
            }
            if hs < k {
                let a = p.g[hs].start;
                for o in linebreak::line_breaks(&p.text[a..p.g[k - 1].end]) {
                    if let Ok(i) = p.g[hs..k].binary_search_by_key(&(a + o), |g| g.start) {
                        icu[hs + i] = true;
                    }
                }
            }
            hs = k + 1;
        }
        let mut hang = vec![0; n + 1];
        for e in 1..=n {
            hang[e] = if zw[e] || !sp(&p.g[e - 1]) {
                e
            } else {
                hang[e - 1]
            };
        }
        let pad = p.styles.iter().zip(&count).map(|(st, _)| st.pad).collect();
        Breaker {
            p,
            items,
            pad,
            zw,
            hang,
            icu,
            fit: p.avail + 1.0 / 64.0,
            item: 0,
            off: 0,
            style: None,
            auto_wrap: true,
            bao: false,
            every: false,
            char_break: false,
            over: false,
            state: State::Continue,
            leading: true,
            skip: None,
            forced: false,
            res: Vec::new(),
            position: 0.0,
            ls: 0,
            start: (0, 0),
            line_style: None,
            pens: RefCell::new(vec![Pen::new(0)]),
            memo: None,
        }
    }

    pub(super) fn run(mut self) -> Vec<Break> {
        let n = self.p.g.len();
        self.set_style(None);
        let mut lines: Vec<(Option<usize>, Break)> = Vec::new();
        while self.item < self.items.len() {
            let start = (self.item, self.off);
            self.res.clear();
            self.position = 0.0;
            self.start = start;
            self.ls = self.off;
            self.pens = RefCell::new(vec![Pen::new(self.ls)]);
            self.memo = None;
            self.over = false;
            self.forced = false;
            self.line_style = self.style;
            self.set_style(self.style);
            self.break_line();
            if !self.forced {
                self.rewind_trailing_open_tags();
            }
            let stuck = (self.item, self.off) == start;
            debug_assert!(!stuck, "line breaker made no progress");
            if stuck {
                let it = self.items[self.item];
                self.res.push(Res {
                    item: self.item,
                    a: self.off,
                    b: it.b,
                    w: 0.0,
                    cba: false,
                    trailing: false,
                    shaped: true,
                    inside: false,
                    bao: false,
                    lb: true,
                });
                self.move_item(self.item);
            }
            if !self.res.last().is_some_and(|r| r.lb) {
                continue;
            }
            let lf = self
                .res
                .iter()
                .map(|r| self.items[r.item])
                .find(|it| it.kind == Kind::Lf)
                .map(|it| it.a);
            let mut spans: Vec<usize> = self.res.iter().map(|r| self.items[r.item].span).collect();
            spans.dedup();
            lines.push((
                lf,
                Break {
                    start: self.ls,
                    end: n,
                    skip: self.skip.map(|(_, k)| k),
                    spans,
                },
            ));
        }
        let starts: Vec<usize> = lines.iter().map(|(_, b)| b.start).skip(1).collect();
        let mut s = 0;
        lines
            .into_iter()
            .enumerate()
            .map(|(i, (lf, b))| {
                let start = s;
                let end = lf.unwrap_or_else(|| starts.get(i).copied().unwrap_or(n));
                s = lf.map_or(end, |k| k + 1);
                Break {
                    start,
                    end,
                    skip: b.skip.filter(|&k| k >= start),
                    spans: b.spans,
                }
            })
            .collect()
    }

    fn ws(&self, st: Option<usize>) -> WhiteSpace {
        st.map_or(self.p.strut_ws, |s| self.p.styles[s].ws)
    }

    fn set_style(&mut self, st: Option<usize>) {
        self.style = st;
        let ws = self.ws(st);
        self.auto_wrap = ws != WhiteSpace::Pre;
        if self.auto_wrap {
            self.bao = self.p.anywhere;
            self.char_break = self.bao && self.over;
            self.every = ws == WhiteSpace::BreakSpaces;
        }
    }

    fn base(&self, k: usize) -> f32 {
        let mut pens = self.pens.borrow_mut();
        while pens.len() <= k - self.ls {
            let mut pen = *pens.last().expect("pens start with the line start");
            self.p.step(&mut pen, &self.p.adv[self.ls..]);
            pens.push(pen);
        }
        pens[k - self.ls].width()
    }

    fn rel(&self, a: usize, b: usize) -> f32 {
        if a >= b {
            return 0.0;
        }
        let g = &self.p.g;
        self.base(b) - self.base(a) - g[a].gap_before - g[a].pad_before - g[b - 1].pad_after
    }

    fn breakable(&self, k: usize) -> bool {
        let g = &self.p.g;
        if k <= self.ls {
            return false;
        }
        if k < g.len() && g[k].nl && g[k - 1].cr {
            return false;
        }
        if k >= g.len() || self.char_break {
            return true;
        }
        let (last, cur) = (sp(&g[k - 1]), sp(&g[k]));
        if self.every {
            if last {
                return true;
            }
            if cur {
                return false;
            }
        } else {
            if cur {
                return false;
            }
            if last {
                return true;
            }
        }
        !g[k - 1].cr && !g[k].cr && self.icu[k]
    }

    fn hang_start(&self, e: usize) -> usize {
        self.hang[e]
    }

    fn with_hang(&self, k: usize, first: usize) -> (usize, Option<usize>) {
        let space = k > 0 && sp(&self.p.g[k - 1]) && !(self.zw[k] && k == first);
        (k, space.then(|| self.hang_start(k - 1)))
    }

    fn prev_opp(&self, offset: usize, min: usize) -> usize {
        let mut k = offset.min(self.p.g.len());
        while k > min {
            if self.breakable(k) {
                return k;
            }
            k -= 1;
        }
        min
    }

    fn next_opp(&self, offset: usize, len: usize) -> usize {
        (offset..len).find(|&k| self.breakable(k)).unwrap_or(len)
    }

    fn exact_fits(&mut self, end: usize) -> bool {
        let w = self.p.remeasure(self.ls, end, &mut self.memo);
        fits(w, self.p.avail)
    }

    fn shaped(
        &self,
        start: usize,
        kern: Option<usize>,
        end: usize,
        overflow: bool,
        trailing: bool,
    ) -> Shaped {
        let w = self.rel(start, end);
        let width = if kern == Some(end) {
            w - self.p.end_trim(end)
        } else {
            w
        };
        Shaped {
            end,
            width,
            overflow,
            trailing,
        }
    }

    fn shape_line(&mut self, ii: usize, start: usize, avail: f32, exact: bool) -> Option<Shaped> {
        let it = self.items[ii];
        let (rs, re) = (it.a, it.b);
        let avail = avail.max(0.0);
        let no_result = self.bao && !self.over;
        if start == rs && avail >= ceil64(self.rel(rs, re)) {
            return Some(self.shaped(start, None, re, false, false));
        }
        let mut cand = if start == rs && avail <= 0.0 {
            rs
        } else {
            (start..re)
                .find(|&k| self.rel(start, k + 1) > avail)
                .unwrap_or(re)
        };
        let mut kern = None;
        if cand < re {
            let trim = self.p.end_trim(cand + 1);
            if trim > 0.0 && self.breakable(cand + 1) && self.rel(start, cand + 1) - trim <= avail {
                cand += 1;
                kern = Some(cand);
            }
        }
        if cand >= re {
            return Some(self.shaped(start, kern, re, false, false));
        }
        let g = &self.p.g;
        let every = self.every;
        let mut overflow = false;
        let mut bo;
        if !sp(&g[cand]) || every {
            bo = self.with_hang(self.prev_opp(cand, start), rs);
            overflow = bo.0 <= start;
            if overflow {
                if no_result {
                    return None;
                }
                bo = self.with_hang(self.next_opp(cand.max(start + 1), re), rs);
            }
        } else {
            bo = self.with_hang(self.next_opp(cand.max(start + 1), re), rs);
            if bo.0 > cand && bo.1.is_none_or(|e| e > cand) {
                let prev = self.with_hang(self.prev_opp(cand, start), rs);
                if prev.0 > start {
                    bo = prev;
                } else {
                    overflow = true;
                    if no_result {
                        return None;
                    }
                }
            }
            if let Some(e) = bo.1 {
                if e <= start {
                    let end = re.min(bo.0);
                    return Some(self.shaped(start, kern, end, overflow, true));
                }
            }
        }
        if bo.0 >= re {
            if overflow {
                return Some(self.shaped(start, kern, re, true, false));
            }
            bo.0 = re;
            if bo.1.is_some_and(|e| re < e) {
                bo.1 = None;
            }
            if sp(&g[re - 1]) {
                bo.1 = Some(self.hang_start(re - 1));
            }
        } else if exact && !overflow {
            loop {
                let end = match bo.1 {
                    Some(e) if !every => (start + 1).max(e),
                    _ => bo.0,
                };
                if self.exact_fits(end) {
                    break;
                }
                let prev = self.with_hang(self.prev_opp(end - 1, start), rs);
                if prev.0 > start {
                    bo = prev;
                    continue;
                }
                overflow = true;
                bo = self.with_hang(self.prev_opp(cand, start), rs);
                if bo.0 <= start {
                    bo = self.with_hang(self.next_opp(cand.max(start + 1), re), rs);
                    if bo.0 >= re {
                        return Some(self.shaped(start, kern, re, true, false));
                    }
                }
                break;
            }
        }
        if !every {
            if let Some(e) = bo.1 {
                bo.0 = (start + 1).max(e);
            }
        }
        Some(self.shaped(start, kern, bo.0, overflow, false))
    }

    fn can_break_after_item(&self, ii: usize) -> bool {
        let it = self.items[ii];
        self.breakable(it.b) || it.kind == Kind::Text && self.p.g.get(it.b).is_some_and(|g| g.atom)
    }

    fn break_text(&mut self, ri: usize, avail: f32, avail_h: f32, exact: bool) -> bool {
        let ii = self.res[ri].item;
        let it = self.items[ii];
        let start = self.res[ri].a;
        match self.shape_line(ii, start, avail, exact) {
            None => {
                let r = &mut self.res[ri];
                r.w = avail_h + 1.0;
                r.b = it.b;
                r.shaped = false;
                false
            }
            Some(s) => {
                let cba = s.end < it.b || self.can_break_after_item(ii);
                let r = &mut self.res[ri];
                r.b = s.end;
                r.w = s.width;
                r.trailing = s.trailing;
                r.shaped = true;
                r.cba = cba;
                r.inside = !s.overflow;
                self.leading = false;
                s.width <= avail_h
            }
        }
    }

    fn add(&mut self, ii: usize, end: usize) -> usize {
        self.res.push(Res {
            item: ii,
            a: self.off,
            b: end,
            w: 0.0,
            cba: false,
            trailing: false,
            shaped: true,
            inside: false,
            bao: self.bao,
            lb: self.res.last().is_some_and(|r| r.lb),
        });
        self.res.len() - 1
    }

    fn add_empty(&mut self, ii: usize) {
        let ri = self.add(ii, self.off);
        if ri >= 1 && self.res[ri - 1].cba {
            self.res[ri - 1].cba = false;
            self.res[ri].cba = true;
        }
    }

    fn move_item(&mut self, ii: usize) {
        self.item = ii + 1;
        self.off = self.items[ii].b;
    }

    fn move_result(&mut self, ri: usize) {
        let r = self.res[ri];
        self.item = r.item + usize::from(r.b == self.items[r.item].b);
        self.off = r.b;
    }

    fn can_fit(&self) -> bool {
        self.position <= self.fit
    }

    fn handle_empty(&mut self, ii: usize) {
        self.add_empty(ii);
        self.move_item(ii);
    }

    fn handle_text(&mut self, ii: usize) {
        let it = self.items[ii];
        if self.state == State::Trailing {
            self.handle_trailing(ii);
            return;
        }
        if self.leading && self.ws(Some(it.span)) == WhiteSpace::Normal && self.p.g[self.off].space
        {
            self.skip = Some((self.res.len(), self.off));
            self.off += 1;
            if self.off == it.b {
                self.handle_empty(ii);
                return;
            }
        }
        if self.state == State::Continue && !self.can_fit() {
            if self.auto_wrap && sp(&self.p.g[self.off]) {
                self.handle_trailing(ii);
                if self.state != State::Done {
                    self.state = State::Continue;
                    return;
                }
            }
            self.handle_overflow();
            return;
        }
        let ri = self.add(ii, it.b);
        self.res[ri].lb = true;
        if !self.auto_wrap {
            let w = self.rel(self.res[ri].a, it.b);
            self.res[ri].w = w;
            self.leading = false;
            self.position += w;
            self.move_item(ii);
            return;
        }
        let avail = self.fit - self.position;
        let fitted = self.break_text(ri, avail, avail, true);
        self.position += self.res[ri].w;
        self.move_result(ri);
        let r = self.res[ri];
        if fitted {
            if r.b < it.b {
                self.handle_trailing(ii);
            }
            return;
        }
        if !r.shaped {
            self.handle_overflow();
            return;
        }
        if r.trailing {
            self.state = State::Trailing;
            if self.ws(Some(it.span)) != WhiteSpace::Normal && sp(&self.p.g[r.b - 1]) {
                self.rewind(ri);
            }
            return;
        }
        if self.state == State::Overflow {
            if r.cba {
                self.state = State::Trailing;
            }
            return;
        }
        if (r.a..r.b).all(|k| sp(&self.p.g[k])) {
            return;
        }
        self.handle_overflow();
    }

    fn handle_trailing(&mut self, ii: usize) {
        let it = self.items[ii];
        if !self.auto_wrap {
            self.state = State::Done;
            return;
        }
        let g = &self.p.g;
        let c0 = self.off;
        match self.p.styles[it.span].ws {
            WhiteSpace::Normal => {
                if !g[c0].space {
                    if c0 > 0 && bsp(&g[c0 - 1]) {
                        self.leading = false;
                    }
                    self.state = State::Done;
                    return;
                }
                self.off = c0 + 1;
                self.leading = false;
                if let Some(last) = self.res.last_mut() {
                    last.cba = true;
                }
            }
            WhiteSpace::BreakSpaces => {}
            _ => {
                let end = (c0..it.b).find(|&k| !bsp(&g[k])).unwrap_or(it.b);
                if end == c0 {
                    if c0 > 0 && bsp(&g[c0 - 1]) {
                        self.leading = false;
                    }
                    self.state = State::Done;
                    return;
                }
                let cba = self.zw[end] || end < g.len() && !bsp(&g[end]);
                let ri = self.add(ii, end);
                let w = self.rel(c0, end);
                let r = &mut self.res[ri];
                r.trailing = true;
                r.w = w;
                r.cba = cba;
                r.lb = true;
                self.position += w;
                self.off = end;
                self.leading = false;
            }
        }
        if self.off < it.b {
            self.state = State::Done;
            return;
        }
        if self.res.last().is_none_or(|r| r.item != ii) {
            self.add_empty(ii);
        }
        self.item = ii + 1;
        self.state = State::Trailing;
    }

    fn handle_overflow_if_needed(&mut self) -> bool {
        if self.state == State::Continue && !self.can_fit() {
            self.handle_overflow();
            return true;
        }
        false
    }

    fn handle_forced(&mut self, ii: usize) {
        if self.handle_overflow_if_needed() {
            return;
        }
        let ri = self.add(ii, self.items[ii].b);
        self.res[ri].trailing = true;
        self.res[ri].cba = true;
        self.res[ri].lb = true;
        self.move_item(ii);
        while self
            .items
            .get(self.item)
            .is_some_and(|it| it.kind == Kind::Close)
        {
            self.handle_close(self.item);
        }
        self.forced = true;
        self.state = State::Done;
    }

    fn handle_control(&mut self, ii: usize) {
        match self.items[ii].kind {
            Kind::Lf => self.handle_forced(ii),
            Kind::Tab => self.handle_text(ii),
            Kind::Zwsp => {
                let ri = self.add(ii, self.items[ii].b);
                self.res[ri].cba = true;
                self.move_item(ii);
            }
            _ => self.handle_empty(ii),
        }
    }

    fn handle_atomic(&mut self, ii: usize) {
        if self.state == State::Continue && self.fit - self.position < 0.0 {
            self.handle_overflow();
            return;
        }
        let it = self.items[ii];
        let ri = self.add(ii, it.b);
        let w = self.rel(it.a, it.b);
        self.res[ri].w = w;
        self.res[ri].cba = self.auto_wrap;
        self.res[ri].lb = true;
        self.position += w;
        self.leading = false;
        self.move_item(ii);
    }

    fn handle_open(&mut self, ii: usize) {
        let span = self.items[ii].span;
        let ri = self.add(ii, self.items[ii].b);
        let w = self.pad[span];
        self.res[ri].w = w;
        self.res[ri].lb |= w > 0.0;
        self.position += w;
        self.set_style(Some(span));
        self.move_item(ii);
    }

    fn handle_close(&mut self, ii: usize) {
        let it = self.items[ii];
        let ri = self.add(ii, it.b);
        let w = self.pad[it.span];
        self.res[ri].w = w;
        self.res[ri].lb |= w > 0.0;
        self.position += w;
        let was = self.auto_wrap;
        self.set_style(None);
        self.move_item(ii);
        if ri == 0 {
            return;
        }
        if self.res[ri - 1].cba {
            self.res[ri].cba = true;
            self.res[ri - 1].cba = false;
            return;
        }
        let g = &self.p.g;
        let k = it.b;
        let after_space = k > 0 && bsp(&g[k - 1]) && !self.zw[k];
        if was {
            let only_after_space = matches!(
                self.p.strut_ws,
                WhiteSpace::PreWrap | WhiteSpace::BreakSpaces
            );
            self.res[ri].cba = g.get(k).is_some_and(bsp) && (!only_after_space || after_space);
            return;
        }
        if self.auto_wrap && !after_space {
            self.res[ri].cba = self.breakable(k);
        }
    }

    fn compute_style(&self, idx: usize) -> Option<usize> {
        let it = self.items[self.res[idx].item];
        if matches!(it.kind, Kind::Text | Kind::Close) {
            return Some(it.span);
        }
        for r in self.res[..idx].iter().rev() {
            let it = self.items[r.item];
            match it.kind {
                Kind::Text | Kind::Open => return Some(it.span),
                Kind::Close => return None,
                _ => {}
            }
        }
        self.line_style
    }

    fn rewind(&mut self, new_end: usize) {
        if new_end > 0 {
            self.move_result(new_end - 1);
            self.leading = false;
        } else {
            (self.item, self.off) = self.start;
            self.leading = true;
        }
        if new_end < self.res.len() {
            let st = self.compute_style(new_end);
            self.set_style(st);
        }
        self.res.truncate(new_end);
        self.skip = self.skip.filter(|&(j, _)| j < new_end);
        self.position = self.res.iter().map(|r| r.w).sum();
    }

    fn rewind_overflow(&mut self, mut new_end: usize) {
        let mut open = 0usize;
        let mut index = new_end;
        while index < self.res.len() {
            let r = self.res[index];
            let it = self.items[r.item];
            let ws = self.p.styles[it.span].ws;
            let wraps = !matches!(ws, WhiteSpace::Pre | WhiteSpace::BreakSpaces);
            let trailable = match it.kind {
                Kind::Text if r.a == r.b => true,
                Kind::Text => {
                    if (r.shaped || self.bao && !self.over) && wraps && sp(&self.p.g[r.a]) {
                        if r.shaped && (r.a + 1..r.b).all(|k| sp(&self.p.g[k])) {
                            true
                        } else {
                            self.state = State::Trailing;
                            self.rewind(index);
                            return;
                        }
                    } else {
                        false
                    }
                }
                Kind::Tab | Kind::Cr | Kind::Zwsp | Kind::Lf => wraps,
                Kind::Open => {
                    if open == 0 {
                        new_end = index;
                    }
                    open += 1;
                    true
                }
                Kind::Close => {
                    open = open.saturating_sub(1);
                    true
                }
                Kind::Atom => false,
            };
            if !trailable {
                if open > 0 {
                    index = new_end;
                }
                self.state = State::Done;
                self.rewind(index);
                return;
            }
            index += 1;
        }
        if open > 0 {
            self.state = State::Done;
            self.rewind(new_end);
            return;
        }
        self.leading = false;
        self.position = self.res.iter().map(|r| r.w).sum();
        self.state = State::Done;
    }

    fn handle_overflow(&mut self) {
        let avail = self.fit;
        let mut wtr = self.position - avail;
        let mut break_before = 0;
        let mut has_bao = self.bao;
        let mut i = self.res.len();
        while i > 0 {
            i -= 1;
            let r = self.res[i];
            has_bao |= r.bao;
            if i + 1 < self.res.len() && r.cba {
                if wtr <= 0.0 {
                    self.position = avail + wtr;
                    self.rewind_overflow(i + 1);
                    return;
                }
                break_before = i + 1;
            }
            wtr -= r.w;
            if wtr > 0.0 {
                continue;
            }
            let it = self.items[r.item];
            if it.kind != Kind::Text || r.a == r.b || wtr >= 0.0 || !r.inside {
                continue;
            }
            let item_avail = -wtr;
            let min_avail = r.w - 1.0;
            if min_avail <= 0.0 {
                let k = self.prev_opp(r.b - 1, r.a);
                if k > r.a {
                    let w = self.rel(r.a, k);
                    let r = &mut self.res[i];
                    r.b = k;
                    r.w = w;
                    r.cba = true;
                    self.rewind_overflow(i + 1);
                    return;
                }
                continue;
            }
            let saved = (self.res[i], self.style, self.leading);
            self.set_style(Some(it.span));
            self.break_text(i, item_avail.min(min_avail), item_avail, false);
            let nr = self.res[i];
            if nr.cba && nr.w <= item_avail && nr.b < r.b {
                if i + 1 == self.res.len() {
                    self.position = avail + wtr + nr.w;
                    self.item = nr.item;
                    self.off = nr.b;
                    self.handle_trailing(nr.item);
                    return;
                }
                self.state = State::Trailing;
                self.rewind(i + 1);
                return;
            }
            self.res[i] = saved.0;
            self.set_style(saved.1);
            self.leading = saved.2;
        }
        if !self.over && has_bao {
            self.over = true;
            self.state = State::Continue;
            let st = if self.res.is_empty() {
                self.style
            } else {
                self.compute_style(0)
            };
            self.set_style(st);
            if !self.res.is_empty() {
                self.rewind(0);
            }
            return;
        }
        if break_before > 0 {
            self.rewind_overflow(break_before);
            return;
        }
        if self.res.last().is_some_and(|r| r.cba) {
            self.state = State::Trailing;
            return;
        }
        self.state = State::Overflow;
    }

    fn rewind_trailing_open_tags(&mut self) {
        let mut k = self.res.len();
        while k > 0 && self.items[self.res[k - 1].item].kind == Kind::Open {
            k -= 1;
        }
        if k > 0 && k < self.res.len() {
            let r = self.res[k];
            self.rewind(k);
            self.item = r.item;
            self.off = r.a;
        }
    }

    fn break_line(&mut self) {
        self.state = State::Continue;
        self.leading = true;
        self.skip = None;
        while self.state != State::Done {
            if self.item >= self.items.len() {
                if self.handle_overflow_if_needed() && self.item < self.items.len() {
                    continue;
                }
                return;
            }
            if self.state == State::Overflow && self.res.last().is_some_and(|r| r.cba) {
                self.state = State::Trailing;
            }
            let ii = self.item;
            match self.items[ii].kind {
                Kind::Text => self.handle_text(ii),
                Kind::Open => self.handle_open(ii),
                Kind::Close => self.handle_close(ii),
                Kind::Tab | Kind::Cr | Kind::Zwsp | Kind::Lf => self.handle_control(ii),
                Kind::Atom => {
                    if self.state == State::Trailing {
                        return;
                    }
                    self.handle_atomic(ii);
                }
            }
        }
    }
}
