use harfrust::{Font, ShaperFont};
use sha2::{Digest, Sha256};
use skrifa::bitmap::BitmapStrikes;
use skrifa::raw::TableProvider;
use skrifa::FontRef;
use std::fmt;
use std::sync::Arc;

pub type FaceId = u16;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Pack {
    Core,
    Emoji,
    Cjk,
    Arabic,
    Hebrew,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Family {
    NotoSans,
    LiberationMono,
    NotoColorEmoji,
    NotoSansCjk,
    NotoSansArabic,
    NotoSansHebrew,
}

pub(crate) struct Spec {
    pub(crate) file: &'static str,
    pub(crate) sha256: &'static str,
    pub(crate) pack: Pack,
    pub(crate) family: Family,
    pub(crate) weight: u16,
    pub(crate) italic: bool,
    embedded: Option<&'static [u8]>,
}

macro_rules! core_face {
    ($file:literal, $sha:literal, $family:expr, $weight:literal, $italic:literal) => {
        Spec {
            file: $file,
            sha256: $sha,
            pack: Pack::Core,
            family: $family,
            weight: $weight,
            italic: $italic,
            embedded: Some(include_bytes!(concat!("../../../assets/fonts/", $file))),
        }
    };
}

macro_rules! lazy_face {
    ($file:literal, $sha:literal, $pack:expr, $family:expr) => {
        Spec {
            file: $file,
            sha256: $sha,
            pack: $pack,
            family: $family,
            weight: 400,
            italic: false,
            embedded: None,
        }
    };
}

pub(crate) static SPECS: [Spec; 10] = [
    core_face!(
        "NotoSans-Regular.ttf",
        "89c3c497f618fdaa0b2d1e98fef93582f28c71debd2c4a8cdf41f190ced2909d",
        Family::NotoSans,
        400,
        false
    ),
    core_face!(
        "NotoSans-Bold.ttf",
        "e83493c945848ecd4a9ad0f6d19164541a0d3e23a9c952304a00a46e00272ac5",
        Family::NotoSans,
        700,
        false
    ),
    core_face!(
        "NotoSans-Italic.ttf",
        "5d7618dda819443fd07301c84adcee9fe8960c873ea5e5e1f0f59db384b78082",
        Family::NotoSans,
        400,
        true
    ),
    core_face!(
        "NotoSans-BoldItalic.ttf",
        "4315b26e5afb62f70f5ffe0edaa3363e57765ff2af2beda12b6fef197e3e5f5a",
        Family::NotoSans,
        700,
        true
    ),
    core_face!(
        "LiberationMono-Regular.ttf",
        "395fa5ab8d40c8eba390ced528744ea75a7f69aabf3e68b6f925ca0e39a27370",
        Family::LiberationMono,
        400,
        false
    ),
    core_face!(
        "LiberationMono-Bold.ttf",
        "626655e94dd82f3f42549daf995c921b0915fa8ab1f4b839559e8892ea41d240",
        Family::LiberationMono,
        700,
        false
    ),
    lazy_face!(
        "NotoColorEmoji.ttf",
        "93cdc4ee9aa40e2afceecc63da0ca05ec7aab4bec991ece51a6b52389f48a477",
        Pack::Emoji,
        Family::NotoColorEmoji
    ),
    lazy_face!(
        "NotoSansCJK-Regular.ttc",
        "b76b0433203017ca80401b2ee0dd69350349871c4b19d504c34dbdd80541690a",
        Pack::Cjk,
        Family::NotoSansCjk
    ),
    lazy_face!(
        "NotoSansArabic-Regular.ttf",
        "504d7407d86875acf7d04dfaa0fd7524d0b8797723bc4aa18022f29db25b0b6e",
        Pack::Arabic,
        Family::NotoSansArabic
    ),
    lazy_face!(
        "NotoSansHebrew-Regular.ttf",
        "436900d5ad77d33e4234247f3076eaecd25b92b9bea0519514f684140e3566a7",
        Pack::Hebrew,
        Family::NotoSansHebrew
    ),
];

const SYSTEM_FALLBACK: [Family; 6] = [
    Family::NotoSans,
    Family::NotoSansCjk,
    Family::NotoSansArabic,
    Family::NotoSansHebrew,
    Family::LiberationMono,
    Family::NotoColorEmoji,
];

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FamilyList(Vec<Family>);

impl FamilyList {
    pub fn parse(css: &str) -> FamilyList {
        let mut out = Vec::new();
        for name in css.split(',') {
            let name = name.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            let found: &[Family] = match name.to_ascii_lowercase().as_str() {
                "noto sans" => &[Family::NotoSans],
                "liberation mono" => &[Family::LiberationMono],
                "noto color emoji" | "emoji" => &[Family::NotoColorEmoji],
                "noto sans cjk jp" => &[Family::NotoSansCjk],
                "noto sans arabic" => &[Family::NotoSansArabic],
                "noto sans hebrew" => &[Family::NotoSansHebrew],
                "sans-serif" => &[
                    Family::NotoSans,
                    Family::NotoSansCjk,
                    Family::NotoColorEmoji,
                ],
                "serif" => &[Family::NotoSans],
                "monospace" => &[Family::LiberationMono],
                _ => &[],
            };
            for f in found {
                if !out.contains(f) {
                    out.push(*f);
                }
            }
        }
        FamilyList(out)
    }

    pub fn families(&self) -> &[Family] {
        &self.0
    }
}

#[derive(Clone)]
enum Bytes {
    Static(&'static [u8]),
    Owned(Arc<Vec<u8>>),
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        match self {
            Bytes::Static(b) => b,
            Bytes::Owned(v) => v,
        }
    }
}

pub struct Face {
    pub id: FaceId,
    pub(crate) font: Font,
    bytes: Bytes,
    pub upem: f32,
    pub ascent: f32,
    pub descent: f32,
    pub(crate) strike_ppem: Option<f32>,
}

impl Face {
    fn load(id: FaceId, bytes: Bytes) -> Result<Face, FontError> {
        let spec = &SPECS[id as usize];
        let invalid = || FontError::Invalid(spec.file);
        let font = match &bytes {
            Bytes::Static(b) => Font::new(*b, 0),
            Bytes::Owned(v) => Font::new(Arc::clone(v) as Arc<dyn AsRef<[u8]> + Send + Sync>, 0),
        }
        .ok_or_else(invalid)?;
        let sk = FontRef::from_index(bytes.as_ref(), 0).map_err(|_| invalid())?;
        let head = sk.head().map_err(|_| invalid())?;
        let hhea = sk.hhea().map_err(|_| invalid())?;
        let strike_ppem = BitmapStrikes::new(&sk).iter().next().map(|s| s.ppem());
        Ok(Face {
            id,
            font,
            upem: head.units_per_em() as f32,
            ascent: hhea.ascender().to_i16() as f32,
            descent: -(hhea.descender().to_i16() as f32),
            strike_ppem,
            bytes,
        })
    }

    pub(crate) fn font_ref(&self) -> FontRef<'_> {
        match FontRef::from_index(self.bytes.as_ref(), 0) {
            Ok(f) => f,
            Err(_) => unreachable!("face bytes were parsed at load"),
        }
    }

    pub fn family(&self) -> Family {
        SPECS[self.id as usize].family
    }

    pub fn file(&self) -> &'static str {
        SPECS[self.id as usize].file
    }

    pub fn weight(&self) -> u16 {
        SPECS[self.id as usize].weight
    }

    pub fn is_emoji(&self) -> bool {
        self.strike_ppem.is_some()
    }

    pub(crate) fn covers(&self, c: char) -> bool {
        ShaperFont::new(&self.font)
            .nominal_glyph(c as u32)
            .is_some()
    }

    pub fn rounded_metrics(&self, size: f32) -> (f32, f32) {
        (
            (self.ascent * size / self.upem).round(),
            (self.descent * size / self.upem).round(),
        )
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FontError {
    Unknown { sha256: String },
    Invalid(&'static str),
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontError::Unknown { sha256 } => {
                write!(
                    f,
                    "font file with SHA-256 {sha256} is not in assets/fonts.lock.json"
                )
            }
            FontError::Invalid(file) => write!(f, "font file {file} does not parse"),
        }
    }
}

impl std::error::Error for FontError {}

pub struct Fonts {
    faces: Vec<Option<Face>>,
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts::new()
    }
}

impl Fonts {
    pub fn new() -> Fonts {
        let faces = SPECS
            .iter()
            .enumerate()
            .map(|(i, spec)| {
                spec.embedded
                    .map(|b| match Face::load(i as FaceId, Bytes::Static(b)) {
                        Ok(face) => face,
                        Err(e) => unreachable!("embedded core face does not load: {e}"),
                    })
            })
            .collect();
        Fonts { faces }
    }

    pub fn register(&mut self, data: Vec<u8>) -> Result<Pack, FontError> {
        let digest = hex(&Sha256::digest(&data));
        let Some(id) = SPECS.iter().position(|s| s.sha256 == digest) else {
            return Err(FontError::Unknown { sha256: digest });
        };
        let face = Face::load(id as FaceId, Bytes::Owned(Arc::new(data)))?;
        self.faces[id] = Some(face);
        Ok(SPECS[id].pack)
    }

    pub fn loaded(&self, pack: Pack) -> bool {
        SPECS
            .iter()
            .zip(&self.faces)
            .filter(|(s, _)| s.pack == pack)
            .all(|(_, f)| f.is_some())
    }

    pub fn face(&self, id: FaceId) -> Option<&Face> {
        self.faces.get(id as usize).and_then(Option::as_ref)
    }

    pub(crate) fn expect(&self, id: FaceId) -> &Face {
        match self.face(id) {
            Some(f) => f,
            None => unreachable!("face {id} was selected while unloaded"),
        }
    }

    fn matching(&self, family: Family, weight: u16, italic: bool) -> Option<FaceId> {
        let candidates: Vec<(FaceId, &Spec)> = SPECS
            .iter()
            .enumerate()
            .filter(|(i, s)| s.family == family && self.faces[*i].is_some())
            .map(|(i, s)| (i as FaceId, s))
            .collect();
        let style: Vec<&(FaceId, &Spec)> = {
            let same: Vec<_> = candidates
                .iter()
                .filter(|(_, s)| s.italic == italic)
                .collect();
            if same.is_empty() {
                candidates.iter().collect()
            } else {
                same
            }
        };
        let weights: Vec<u16> = style.iter().map(|(_, s)| s.weight).collect();
        let w = css_weight(weight, &weights)?;
        style.iter().find(|(_, s)| s.weight == w).map(|(id, _)| *id)
    }

    pub(crate) fn primary(&self, families: &FamilyList, weight: u16, italic: bool) -> FaceId {
        families
            .0
            .iter()
            .find_map(|f| self.matching(*f, weight, italic))
            .unwrap_or(0)
    }

    pub(crate) fn select(
        &self,
        primary: FaceId,
        families: &FamilyList,
        weight: u16,
        italic: bool,
        cluster: &str,
    ) -> FaceId {
        if is_emoji_cluster(cluster) {
            if let Some(e) = self.matching(Family::NotoColorEmoji, 400, false) {
                return e;
            }
        }
        let base: Vec<char> = cluster
            .chars()
            .filter(|&c| {
                !matches!(c, '\u{200d}' | '\u{fe0e}' | '\u{fe0f}')
                    && !('\u{300}'..='\u{36f}').contains(&c)
            })
            .collect();
        let covers = |id: FaceId| base.iter().all(|&c| self.expect(id).covers(c));
        if base.is_empty() || covers(primary) {
            return primary;
        }
        families
            .0
            .iter()
            .chain(SYSTEM_FALLBACK.iter())
            .filter_map(|f| self.matching(*f, weight, italic))
            .find(|&id| covers(id))
            .unwrap_or(primary)
    }
}

fn css_weight(desired: u16, available: &[u16]) -> Option<u16> {
    if available.contains(&desired) {
        return Some(desired);
    }
    let mut sorted = available.to_vec();
    sorted.sort_unstable();
    let above = |lo: u16| sorted.iter().copied().find(|&w| w > lo);
    let below = |hi: u16| sorted.iter().rev().copied().find(|&w| w < hi);
    match desired {
        400..=500 => sorted
            .iter()
            .copied()
            .find(|&w| w > desired && w <= 500)
            .or_else(|| below(desired))
            .or_else(|| above(500)),
        d if d < 400 => below(d).or_else(|| above(d)),
        d => above(d).or_else(|| below(d)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_emoji_presentation(c: char) -> bool {
    use icu_properties::props::EmojiPresentation;
    use icu_properties::CodePointSetData;
    CodePointSetData::new::<EmojiPresentation>().contains(c)
}

pub(crate) fn is_emoji_cluster(cluster: &str) -> bool {
    let Some(first) = cluster.chars().next() else {
        return false;
    };
    if cluster.contains('\u{fe0f}') || cluster.contains('\u{20e3}') {
        return true;
    }
    if cluster.contains('\u{fe0e}') {
        return false;
    }
    if ('\u{1f1e6}'..='\u{1f1ff}').contains(&first) {
        return true;
    }
    is_emoji_presentation(first)
        || (cluster.contains('\u{200d}') && cluster.chars().any(is_emoji_presentation))
}
