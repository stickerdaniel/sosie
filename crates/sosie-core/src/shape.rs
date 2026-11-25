// Adapted from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use crate::fonts::Face;
use crate::fonts::Family;
use harfrust::{Buffer, Direction, Feature, ShapeOptions, ShapePlan, ShaperFont, Tag};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::hash::BuildHasherDefault;
use std::rc::Rc;

#[derive(Clone, Copy, Debug)]
pub struct ShapedGlyph {
    pub id: u32,
    pub cluster: u32,
    pub adv: f32,
    pub dx: f32,
    pub dy: f32,
    pub unsafe_to_break: bool,
}

thread_local! {
    static BUFFER: RefCell<Option<Buffer>> = const { RefCell::new(None) };
    static PLANS: RefCell<PlanCache> = RefCell::new(PlanCache::empty());
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PlanKey {
    face: u16,
    rtl: bool,
    script: Option<harfrust::Script>,
}

#[derive(Default)]
struct PlanHasher(u64);

impl std::hash::Hasher for PlanHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (words, tail) = bytes.as_chunks::<8>();
        for &w in words {
            self.write_u64(u64::from_ne_bytes(w));
        }
        for &b in tail {
            self.write_u64(b as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, v: u8) {
        self.write_u64(v as u64);
    }

    #[inline]
    fn write_u16(&mut self, v: u16) {
        self.write_u64(v as u64);
    }

    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

struct PlanCache {
    plans: HashMap<PlanKey, Rc<ShapePlan>, BuildHasherDefault<PlanHasher>>,
    order: VecDeque<PlanKey>,
}

impl PlanCache {
    const MAX: usize = 512;

    fn empty() -> Self {
        PlanCache {
            plans: HashMap::default(),
            order: VecDeque::new(),
        }
    }

    fn get(&self, key: &PlanKey) -> Option<Rc<ShapePlan>> {
        self.plans.get(key).map(Rc::clone)
    }

    fn insert(&mut self, key: PlanKey, plan: Rc<ShapePlan>) {
        while self.plans.len() >= Self::MAX {
            match self.order.pop_front() {
                Some(oldest) => {
                    self.plans.remove(&oldest);
                }
                None => break,
            }
        }
        if self.plans.insert(key, plan).is_none() {
            self.order.push_back(key);
        }
    }
}

fn features(face: &Face) -> &'static [Feature] {
    static CHWS: [Feature; 1] = [Feature {
        tag: Tag::new(b"chws"),
        value: 1,
        start: 0,
        end: u32::MAX,
    }];
    if face.family() == Family::NotoSansCjk {
        &CHWS
    } else {
        &[]
    }
}

fn plan_for(face: &Face, rtl: bool, script: Option<harfrust::Script>) -> Rc<ShapePlan> {
    let key = PlanKey {
        face: face.id,
        rtl,
        script,
    };
    PLANS.with(|plans| {
        if let Some(plan) = plans.borrow().get(&key) {
            return plan;
        }
        let plan = Rc::new(ShapePlan::new(
            &face.font,
            if rtl {
                Direction::RightToLeft
            } else {
                Direction::LeftToRight
            },
            script,
            None,
            features(face),
        ));
        plans.borrow_mut().insert(key, Rc::clone(&plan));
        plan
    })
}

pub fn shape(
    face: &Face,
    text: &str,
    size: f32,
    rtl: bool,
    script: Option<harfrust::Script>,
    out: &mut Vec<ShapedGlyph>,
) {
    if text.is_empty() {
        return;
    }
    let mut buffer = BUFFER.with(|b| b.borrow_mut().take()).unwrap_or_default();
    buffer.clear();
    buffer.push_str(text);
    match script {
        Some(s) => buffer.set_script(Some(s)),
        None => buffer.guess_segment_properties(),
    }
    buffer.set_direction(if rtl {
        Direction::RightToLeft
    } else {
        Direction::LeftToRight
    });

    let shaper = ShaperFont::new(&face.font);
    let plan = plan_for(face, rtl, buffer.script());
    let options = ShapeOptions::new()
        .features(features(face))
        .plan(Some(&*plan));
    if harfrust::shape(&shaper, &mut buffer, options).is_ok() {
        let upem = face.upem as i64;
        let x_scale = (size * 65536.0) as i64;
        let hb_scale = |v: i64| -> i64 {
            let scaled = v * x_scale;
            (scaled + if scaled >= 0 { upem / 2 } else { -upem / 2 }) / upem
        };
        let own_scale = match face.strike_ppem {
            Some(ppem) => OwnScale::Strike(ppem as f64),
            None => OwnScale::Outline((size as f64 * 64.0).floor() / 64.0),
        };
        let infos = buffer.glyph_infos();
        let positions = buffer.glyph_positions();
        out.reserve(infos.len());
        for (info, pos) in infos.iter().zip(positions) {
            let own = shaper.glyph_h_advance(harfrust::GlyphId::new(info.glyph_id)) as i64;
            let own_px = match own_scale {
                OwnScale::Strike(ppem) => {
                    ((own as f64 * ppem / face.upem as f64).round() * size as f64 / ppem) as f32
                }
                OwnScale::Outline(ft_size) => (own as f64 * ft_size / face.upem as f64) as f32,
            };
            let own_fixed = (own_px * 65536.0) as i64;
            let adjust_fixed = hb_scale(pos.x_advance as i64 - own);
            out.push(ShapedGlyph {
                id: info.glyph_id,
                cluster: info.cluster,
                adv: (own_fixed + adjust_fixed) as f32 / 65536.0,
                dx: hb_scale(pos.x_offset as i64) as f32 / 65536.0,
                dy: hb_scale(pos.y_offset as i64) as f32 / 65536.0,
                unsafe_to_break: info.unsafe_to_break(),
            });
        }
    }
    BUFFER.with(|b| *b.borrow_mut() = Some(buffer));
}

#[derive(Clone, Copy)]
enum OwnScale {
    Strike(f64),
    Outline(f64),
}
