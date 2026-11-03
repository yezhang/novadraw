// Copyright 2026 the Novadraw Contributors
// SPDX-License-Identifier: MIT

//! Ordered rules extracted from the locked generated language table.
//! Ordinary and admitted callers evaluate the same rules and byte kernels.

use super::face::cold_admission::{ColdAdmission, ColdOperation};
use super::tag::find_private_prefix_with;
use read_fonts::types::Tag;
use smallvec::SmallVec;

enum Condition {
    Subtag(&'static str),
    Exact(&'static str),
    Language(&'static str),
    Both(&'static str, &'static str),
}

struct Rule {
    condition: Condition,
    tags: &'static [&'static [u8; 4]],
}

macro_rules! condition {
    (subtag $a:literal) => {
        Condition::Subtag($a)
    };
    (exact $a:literal) => {
        Condition::Exact($a)
    };
    (lang $a:literal) => {
        Condition::Language($a)
    };
    (both $a:literal / $b:literal) => {
        Condition::Both($a, $b)
    };
}

macro_rules! rules {
    ($($kind:ident $a:literal $(/ $b:literal)? => [$($tag:literal),+],)*) => {
        &[$(Rule {
            condition: condition!($kind $a $(/ $b)?),
            tags: &[$($tag),+],
        }),*]
    };
}

const GLOBAL: &[Rule] = rules! {
    subtag "-fonnapa" => [b"APPH"],
    subtag "-polyton" => [b"PGR "],
    subtag "-arevmda" => [b"HYE "],
    subtag "-provenc" => [b"PRO "],
    subtag "-fonipa" => [b"IPPH"],
    subtag "-geok" => [b"KGE "],
    subtag "-syre" => [b"SYRE"],
    subtag "-syrj" => [b"SYRJ"],
    subtag "-syrn" => [b"SYRN"],
};
const A: &[Rule] = rules! {
    exact "rt-lojban" => [b"JBO "],
};
const C: &[Rule] = rules! {
    lang "do-hant-hk" => [b"ZHH "],
    lang "do-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "jy-hant-hk" => [b"ZHH "],
    lang "jy-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "mn-hant-hk" => [b"ZHH "],
    lang "mn-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "np-hant-hk" => [b"ZHH "],
    lang "np-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "px-hant-hk" => [b"ZHH "],
    lang "px-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "sp-hant-hk" => [b"ZHH "],
    lang "sp-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "zh-hant-hk" => [b"ZHH "],
    lang "zh-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "zo-hant-hk" => [b"ZHH "],
    lang "zo-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "do-hans" => [b"ZHS "],
    lang "do-hant" => [b"ZHT "],
    lang "jy-hans" => [b"ZHS "],
    lang "jy-hant" => [b"ZHT "],
    lang "mn-hans" => [b"ZHS "],
    lang "mn-hant" => [b"ZHT "],
    lang "np-hans" => [b"ZHS "],
    lang "np-hant" => [b"ZHT "],
    lang "px-hans" => [b"ZHS "],
    lang "px-hant" => [b"ZHT "],
    lang "sp-hans" => [b"ZHS "],
    lang "sp-hant" => [b"ZHT "],
    lang "zh-hans" => [b"ZHS "],
    lang "zh-hant" => [b"ZHT "],
    lang "zo-hans" => [b"ZHS "],
    lang "zo-hant" => [b"ZHT "],
    both "do-" / "-hk" => [b"ZHH "],
    both "do-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "do-" / "-tw" => [b"ZHT "],
    both "jy-" / "-hk" => [b"ZHH "],
    both "jy-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "jy-" / "-tw" => [b"ZHT "],
    both "mn-" / "-hk" => [b"ZHH "],
    both "mn-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "mn-" / "-tw" => [b"ZHT "],
    both "np-" / "-hk" => [b"ZHH "],
    both "np-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "np-" / "-tw" => [b"ZHT "],
    both "px-" / "-hk" => [b"ZHH "],
    both "px-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "px-" / "-tw" => [b"ZHT "],
    both "sp-" / "-hk" => [b"ZHH "],
    both "sp-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "sp-" / "-tw" => [b"ZHT "],
    both "zh-" / "-hk" => [b"ZHH "],
    both "zh-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "zh-" / "-tw" => [b"ZHT "],
    both "zo-" / "-hk" => [b"ZHH "],
    both "zo-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "zo-" / "-tw" => [b"ZHT "],
};
const G: &[Rule] = rules! {
    lang "an-hant-hk" => [b"ZHH "],
    lang "an-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "an-hans" => [b"ZHS "],
    lang "an-hant" => [b"ZHT "],
    lang "a-latg" => [b"IRT "],
    both "an-" / "-hk" => [b"ZHH "],
    both "an-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "an-" / "-tw" => [b"ZHT "],
};
const H: &[Rule] = rules! {
    lang "ak-hant-hk" => [b"ZHH "],
    lang "ak-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "nm-hant-hk" => [b"ZHH "],
    lang "nm-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "sn-hant-hk" => [b"ZHH "],
    lang "sn-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "ak-hans" => [b"ZHS "],
    lang "ak-hant" => [b"ZHT "],
    lang "nm-hans" => [b"ZHS "],
    lang "nm-hant" => [b"ZHT "],
    lang "sn-hans" => [b"ZHS "],
    lang "sn-hant" => [b"ZHT "],
    both "ak-" / "-hk" => [b"ZHH "],
    both "ak-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "ak-" / "-tw" => [b"ZHT "],
    both "nm-" / "-hk" => [b"ZHH "],
    both "nm-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "nm-" / "-tw" => [b"ZHT "],
    both "sn-" / "-hk" => [b"ZHH "],
    both "sn-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "sn-" / "-tw" => [b"ZHT "],
};
const I: &[Rule] = rules! {
    exact "-navajo" => [b"NAV ", b"ATH "],
    exact "-hak" => [b"ZHS "],
    exact "-lux" => [b"LTZ "],
};
const L: &[Rule] = rules! {
    lang "uh-hant-hk" => [b"ZHH "],
    lang "uh-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "uh-hans" => [b"ZHS "],
    lang "uh-hant" => [b"ZHT "],
    lang "zh-hans" => [b"ZHS "],
    both "uh-" / "-hk" => [b"ZHH "],
    both "uh-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "uh-" / "-tw" => [b"ZHT "],
};
const M: &[Rule] = rules! {
    lang "np-hant-hk" => [b"ZHH "],
    lang "np-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "np-hans" => [b"ZHS "],
    lang "np-hant" => [b"ZHT "],
    both "np-" / "-hk" => [b"ZHH "],
    both "np-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "np-" / "-tw" => [b"ZHT "],
    both "nw-" / "-th" => [b"MONT"],
};
const N: &[Rule] = rules! {
    lang "an-hant-hk" => [b"ZHH "],
    lang "an-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "an-hans" => [b"ZHS "],
    lang "an-hant" => [b"ZHT "],
    both "an-" / "-hk" => [b"ZHH "],
    both "an-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "an-" / "-tw" => [b"ZHT "],
    exact "o-bok" => [b"NOR "],
    exact "o-nyn" => [b"NYN "],
};
const R: &[Rule] = rules! {
    both "o-" / "-md" => [b"MOL ", b"ROM "],
};
const S: &[Rule] = rules! {
    lang "jc-hant-hk" => [b"ZHH "],
    lang "jc-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "jc-hans" => [b"ZHS "],
    lang "jc-hant" => [b"ZHT "],
    both "jc-" / "-hk" => [b"ZHH "],
    both "jc-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "jc-" / "-tw" => [b"ZHT "],
};
const W: &[Rule] = rules! {
    lang "uu-hant-hk" => [b"ZHH "],
    lang "uu-hant-mo" => [b"ZHTM", b"ZHH "],
    lang "uu-hans" => [b"ZHS "],
    lang "uu-hant" => [b"ZHT "],
    both "uu-" / "-hk" => [b"ZHH "],
    both "uu-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "uu-" / "-tw" => [b"ZHT "],
};
const Y: &[Rule] = rules! {
    lang "ue-hans" => [b"ZHS "],
};
const Z: &[Rule] = rules! {
    lang "h-hant-hk" => [b"ZHH "],
    lang "h-hant-mo" => [b"ZHTM", b"ZHH "],
    exact "h-min-nan" => [b"ZHS "],
    lang "h-hans" => [b"ZHS "],
    lang "h-hant" => [b"ZHT "],
    exact "h-min" => [b"ZHS "],
    both "h-" / "-hk" => [b"ZHH "],
    both "h-" / "-mo" => [b"ZHTM", b"ZHH "],
    both "h-" / "-tw" => [b"ZHT "],
};

fn subtag_with<P: ColdAdmission>(
    mut language: &[u8],
    subtag: &[u8],
    policy: &mut P,
) -> Result<bool, P::Error> {
    while let Some(i) = find_private_prefix_with(language, subtag, policy)? {
        // Like match_indices, resume after the complete nonoverlapping match.
        policy.charge(ColdOperation::TableRead, 1, 0)?;
        if language
            .get(i + subtag.len())
            .is_none_or(|c| !c.is_ascii_alphanumeric())
        {
            return Ok(true);
        }
        language = &language[i + subtag.len()..];
    }
    Ok(false)
}

fn starts_with<P: ColdAdmission>(
    language: &[u8],
    spec: &[u8],
    policy: &mut P,
) -> Result<bool, P::Error> {
    policy.charge(ColdOperation::Planning, 2, 0)?;
    if language.len() < spec.len() {
        return Ok(false);
    }
    policy.charge(ColdOperation::TableRead, spec.len(), 0)?;
    policy.charge(ColdOperation::TableRead, spec.len(), 0)?;
    Ok(&language[..spec.len()] == spec)
}

fn apply_with<P: ColdAdmission>(
    rules: &[Rule],
    language: &[u8],
    suffix: &[u8],
    tags: &mut SmallVec<[Tag; 3]>,
    policy: &mut P,
) -> Result<bool, P::Error> {
    for rule in rules {
        // Visit the rule, its discriminator and string views before matching.
        policy.charge(ColdOperation::TableRead, 4, 0)?;
        let matched = match rule.condition {
            Condition::Subtag(spec) => subtag_with(language, spec.as_bytes(), policy)?,
            Condition::Exact(spec) => {
                policy.charge(ColdOperation::Planning, 2, 0)?;
                suffix.len() == spec.len() && starts_with(suffix, spec.as_bytes(), policy)?
            }
            Condition::Language(spec) => {
                if starts_with(suffix, spec.as_bytes(), policy)? {
                    policy.charge(ColdOperation::TableRead, 1, 0)?;
                    suffix.len() == spec.len() || suffix.get(spec.len()) == Some(&b'-')
                } else {
                    false
                }
            }
            Condition::Both(prefix, subtag) => {
                starts_with(suffix, prefix.as_bytes(), policy)?
                    && subtag_with(language, subtag.as_bytes(), policy)?
            }
        };
        if matched {
            policy.charge(ColdOperation::Planning, 1, 0)?;
            // Every rule produces one or two inline tags. No temporary tag
            // slice is constructed and no heap growth can occur here.
            for bytes in rule.tags {
                policy.charge(ColdOperation::TableRead, 1 + 4, 0)?;
                policy.charge(ColdOperation::Initialize, 4 + 4, 0)?;
                tags.push(Tag::new(bytes));
            }
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn tags_from_complex_language_with<P: ColdAdmission>(
    language: &str,
    tags: &mut SmallVec<[Tag; 3]>,
    policy: &mut P,
) -> Result<bool, P::Error> {
    let bytes = language.as_bytes();
    if apply_with(GLOBAL, bytes, bytes, tags, policy)? {
        return Ok(true);
    }
    policy.charge(ColdOperation::TableRead, 1, 0)?;
    let Some(first) = bytes.first() else {
        return Ok(false);
    };
    let rules = match first {
        b'a' => A,
        b'c' => C,
        b'g' => G,
        b'h' => H,
        b'i' => I,
        b'l' => L,
        b'm' => M,
        b'n' => N,
        b'r' => R,
        b's' => S,
        b'w' => W,
        b'y' => Y,
        b'z' => Z,
        _ => &[],
    };
    apply_with(rules, bytes, &bytes[1..], tags, policy)
}

#[cfg(test)]
mod tests {
    use super::super::face::cold_admission::OrdinaryCold;
    use super::*;
    use alloc::{format, string::String, vec::Vec};

    #[test]
    fn every_locked_rule_matches_the_generated_reference() {
        let groups = [
            (b'?', GLOBAL),
            (b'a', A),
            (b'c', C),
            (b'g', G),
            (b'h', H),
            (b'i', I),
            (b'l', L),
            (b'm', M),
            (b'n', N),
            (b'r', R),
            (b's', S),
            (b'w', W),
            (b'y', Y),
            (b'z', Z),
        ];
        for (first, rules) in groups {
            for rule in rules {
                let base = match rule.condition {
                    Condition::Subtag(spec) => format!("und{spec}"),
                    Condition::Exact(spec) | Condition::Language(spec) => {
                        format!("{}{spec}", char::from(first))
                    }
                    Condition::Both(prefix, subtag) => {
                        format!("{}{prefix}abc{subtag}", char::from(first))
                    }
                };
                let mut cases: Vec<String> = Vec::new();
                for suffix in ["", "-", "-abc", "a", "0", "é", "-fonipa", "-hant-mo"] {
                    cases.push(format!("{base}{suffix}"));
                }
                for i in 0..base.len() {
                    cases.push(String::from(&base[..i]));
                }
                for input in cases.into_iter().filter(|s| !s.is_empty()) {
                    let mut expected = SmallVec::new();
                    let found =
                        super::super::tag_table::tags_from_complex_language(&input, &mut expected);
                    let mut actual = SmallVec::new();
                    assert_eq!(
                        tags_from_complex_language_with(&input, &mut actual, &mut OrdinaryCold,)
                            .unwrap(),
                        found,
                        "{input}"
                    );
                    assert_eq!(actual, expected, "{input}");
                }
            }
        }
    }
}
