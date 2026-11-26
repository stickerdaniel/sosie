// Copied from xeet (github.com/rauchg/xeet) at 095ba15, Apache-2.0; changes are listed in NOTICE.

use icu_properties::props::{BidiPairedBracketType, Script};
use icu_properties::CodePointMapData;

#[allow(deprecated)]
pub fn resolve(text: &str) -> Vec<u16> {
    let scripts = CodePointMapData::<Script>::new();
    let extensions = icu_properties::script::ScriptWithExtensions::new();
    let mirrors = CodePointMapData::<icu_properties::props::BidiMirroringGlyph>::new();
    const UNRESOLVED: u16 = u16::MAX;
    let common = Script::Common.to_icu4c_value();

    let mut out = vec![UNRESOLVED; text.len()];
    let mut cur = UNRESOLVED;
    let mut pending_from: Option<(usize, u16)> = None;
    let mut stack: Vec<(char, usize)> = Vec::new();
    for (i, c) in text.char_indices() {
        let mut sc = scripts.get(c);
        if sc == Script::Common || sc == Script::Inherited {
            let ext = extensions.get_script_extensions_val(c);
            let mut real = ext
                .iter()
                .filter(|x| *x != Script::Common && *x != Script::Inherited);
            if let Some(first) = real.next() {
                if cur == UNRESOLVED || !ext.iter().any(|x| x.to_icu4c_value() == cur) {
                    sc = first;
                }
            }
        }
        let resolved = if sc == Script::Common || sc == Script::Inherited {
            let m = mirrors.get(c);
            match m.paired_bracket_type {
                BidiPairedBracketType::Open => {
                    if stack.len() < 64 {
                        stack.push((m.mirroring_glyph.unwrap_or(c), i));
                    }
                    pending_from = Some((i + c.len_utf8(), cur));
                    cur
                }
                BidiPairedBracketType::Close => {
                    match stack.iter().rposition(|&(close, _)| close == c) {
                        Some(k) => {
                            let open_at = stack[k].1;
                            stack.truncate(k);
                            let s = out[open_at];
                            if s != UNRESOLVED {
                                cur = s;
                            }
                            if let Some((p, _)) = pending_from.take() {
                                resolve_pending(&mut out[p..i], s);
                            }
                            s
                        }
                        None => {
                            if pending_from.is_some() {
                                UNRESOLVED
                            } else {
                                cur
                            }
                        }
                    }
                }
                _ => {
                    if pending_from.is_some() {
                        UNRESOLVED
                    } else {
                        cur
                    }
                }
            }
        } else {
            let s = sc.to_icu4c_value();
            if let Some((p, bracket)) = pending_from.take() {
                resolve_pending(
                    &mut out[p..i],
                    if bracket == s || bracket == UNRESOLVED {
                        s
                    } else {
                        common
                    },
                );
            }
            if cur == UNRESOLVED {
                for v in out[..i].iter_mut() {
                    if *v == UNRESOLVED {
                        *v = s;
                    }
                }
            }
            cur = s;
            s
        };
        let end = i + c.len_utf8();
        for v in &mut out[i..end] {
            *v = resolved;
        }
    }
    if let Some((p, bracket)) = pending_from {
        resolve_pending(&mut out[p..], bracket);
    }
    for v in &mut out {
        if *v == UNRESOLVED {
            *v = common;
        }
    }
    out
}

#[allow(deprecated)]
pub fn hb_script(code: u16) -> Option<harfrust::Script> {
    use icu_properties::PropertyNamesShort;
    if code == 0 {
        return None;
    }
    let script = Script::from_icu4c_value(code);
    if script == Script::Common || script == Script::Inherited {
        return None;
    }
    let name = PropertyNamesShort::<Script>::new().get(script)?;
    let bytes: [u8; 4] = name.as_bytes().try_into().ok()?;
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&bytes))
}

fn resolve_pending(bytes: &mut [u16], script: u16) {
    for v in bytes {
        if *v == u16::MAX {
            *v = script;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs(text: &str) -> Vec<String> {
        let s = resolve(text);
        let mut out: Vec<String> = Vec::new();
        let mut prev = None;
        for (i, c) in text.char_indices() {
            if prev == Some(s[i]) {
                out.last_mut().unwrap().push(c);
            } else {
                out.push(c.to_string());
                prev = Some(s[i]);
            }
        }
        out
    }

    #[test]
    fn common_joins_the_preceding_script() {
        assert_eq!(runs("a חדש (beta)"), ["a ", "חדש (", "beta", ")"]);
        assert_eq!(runs("the الله #AI"), ["the ", "الله #", "AI"]);
    }

    #[test]
    fn leading_common_joins_the_first_script() {
        assert_eq!(runs("2026 שלום"), ["2026 שלום"]);
        assert_eq!(runs("🚀 launch"), ["🚀 launch"]);
    }

    #[test]
    fn closing_bracket_takes_the_opening_script() {
        assert_eq!(runs("שלום (the) עולם"), ["שלום (", "the", ") עולם"]);
        assert_eq!(runs("x השקה (#AI)"), ["x ", "השקה (", "#", "AI", ")"]);
        assert_eq!(runs("x השקה #AI"), ["x ", "השקה #", "AI"]);
        assert_eq!(runs("é (#AI)"), ["é (#AI)"]);
        assert_eq!(runs("API ، (API)"), ["API ", "، (", "API", ")"]);
        assert_eq!(runs("مرحبا، شكرا"), ["مرحبا، شكرا"]);
        assert_eq!(runs("é (100%) جديد"), ["é (100%) ", "جديد"]);
    }
}
