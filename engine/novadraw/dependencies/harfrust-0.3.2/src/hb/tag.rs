use smallvec::SmallVec;

use super::common::TagExt;
use super::{hb_tag_t, script, tag_table, Language, Script};
use super::face::cold_admission::{ColdAdmission, ColdOperation};
#[cfg(test)]
use super::face::cold_admission::OrdinaryCold;

type ThreeTags = SmallVec<[hb_tag_t; 3]>;

trait SmallVecExt {
    fn left(&self) -> usize;
    fn is_full(&self) -> bool;
}

impl<A: smallvec::Array> SmallVecExt for SmallVec<A> {
    fn left(&self) -> usize {
        self.inline_size() - self.len()
    }

    fn is_full(&self) -> bool {
        self.len() == self.inline_size()
    }
}

/// Converts an `Script` and an `Language` to script and language tags.
#[cfg(test)]
pub fn tags_from_script_and_language(
    script: Option<Script>,
    language: Option<&Language>,
) -> (ThreeTags, ThreeTags) {
    match tags_from_script_and_language_with(script, language, &mut OrdinaryCold) {
        Ok(Some(tags)) => tags,
        Ok(None) => unreachable!(),
        Err(error) => match error {},
    }
}

pub(crate) fn tags_from_script_and_language_with<P: ColdAdmission>(
    script: Option<Script>, language: Option<&Language>, policy: &mut P,
) -> Result<Option<(ThreeTags, ThreeTags)>, P::Error> {
    let mut needs_script = true;
    let mut scripts = SmallVec::new();
    let mut languages = SmallVec::new();

    let mut private_use_subtag = None;
    let mut prefix = "";
    if let Some(language) = language {
        // as_str validates UTF-8. The stored byte count can be read without
        // validation; charge that pass before entering the public getter.
        policy.charge(ColdOperation::Planning, 1, 0)?;
        policy.charge(ColdOperation::TableRead, language.byte_len(), 0)?;
        let language = language.as_str();
        policy.charge(ColdOperation::TableRead, 2, 0)?;
        if language.starts_with("x-") {
            private_use_subtag = Some(language);
        } else {
            let bytes = language.as_bytes();
            let mut i = 1;
            while i < bytes.len() {
                // Previous, next and current byte, with conditional reread
                // for the x branch. Charge before each iteration.
                policy.charge(ColdOperation::TableRead, 4, 0)?;
                if bytes.get(i - 1) == Some(&b'-') && bytes.get(i + 1) == Some(&b'-') {
                    if bytes[i] == b'x' {
                        private_use_subtag = Some(&language[i..]);
                        if prefix.is_empty() {
                            prefix = &language[..i - 1];
                        }

                        break;
                    } else {
                        prefix = &language[..i - 1];
                    }
                }

                i += 1;
            }

            if prefix.is_empty() {
                prefix = &language[..i];
            }
        }

        needs_script = !parse_private_use_subtag_with(
            private_use_subtag,
            "-hbsc",
            u8::to_ascii_lowercase,
            &mut scripts,
            policy,
        )?;

        let needs_language = !parse_private_use_subtag_with(
            private_use_subtag,
            "-hbot",
            u8::to_ascii_uppercase,
            &mut languages,
            policy,
        )?;

        if needs_language && !prefix.is_empty() {
            // Language already normalizes ASCII case and underscores when
            // constructed. Its nonempty prefix needs no temporary allocation.
            tags_from_language_with(prefix, &mut languages, policy)?;
        }
    }

    if needs_script {
        // Fixed-size script conversion produces at most three inline tags.
        policy.charge(ColdOperation::TableRead, 4 + 4 + 4, 0)?;
        policy.charge(ColdOperation::Initialize, 3 + 3, 0)?;
        all_tags_from_script(script, &mut scripts);
    }

    Ok(Some((scripts, languages)))
}

fn parse_private_use_subtag_with<P: ColdAdmission>(
    private_use_subtag: Option<&str>,
    prefix: &str,
    normalize: fn(&u8) -> u8,
    tags: &mut ThreeTags,
    policy: &mut P,
) -> Result<bool, P::Error> {
    let Some(private_use_subtag) = private_use_subtag else {
        return Ok(false);
    };

    // Explicit bounded substring scan on both paths, avoiding an opaque str
    // searcher. Reproduce find's earliest byte match for these ASCII needles.
    let found = find_private_prefix_with(private_use_subtag.as_bytes(), prefix.as_bytes(), policy)?;
    let private_use_subtag = match found {
        Some(idx) => &private_use_subtag[idx + prefix.len()..],
        None => return Ok(false),
    };

    let mut tag = SmallVec::<[u8; 4]>::new();
    for i in 0..core::cmp::min(4, private_use_subtag.len()) {
        policy.charge(ColdOperation::TableRead, 1, 0)?;
        let c = private_use_subtag.as_bytes()[i];
        if c.is_ascii_alphanumeric() {
            // Inline byte write and its eventual cleanup visit.
            policy.charge(ColdOperation::Initialize, 2, 0)?;
            tag.push((normalize)(&c));
        } else {
            break;
        }
    }

    if tag.is_empty() {
        return Ok(false);
    }

    // TagExt pads a four-byte stack array, copies the supplied bytes into it,
    // then copies that array into Tag. Cover the temporary writes and cleanup,
    // not only the input byte visits.
    policy.charge(ColdOperation::TableRead, tag.len() + 4, 0)?;
    policy.charge(ColdOperation::Initialize, 4 + tag.len() + 4, 0)?;
    let mut tag = hb_tag_t::from_bytes_lossy(tag.as_slice());

    // Some bits magic from HarfBuzz...
    // Include repeated tag reads even on the conditional case below.
    policy.charge(ColdOperation::TableRead, 4 + 4 + 4, 0)?;
    if tag.as_u32() & 0xDFDF_DFDF == hb_tag_t::default_script().as_u32() {
        tag = hb_tag_t::from_u32(tag.as_u32() ^ !0xDFDF_DFDF);
    }

    policy.charge(ColdOperation::Initialize, 2, 0)?;
    tags.push(tag);

    Ok(true)
}

pub(crate) fn find_private_prefix_with<P: ColdAdmission>(
    bytes: &[u8], prefix: &[u8], policy: &mut P,
) -> Result<Option<usize>, P::Error> {
    // Both needles are nonempty ASCII literals. A fixed word skip avoids
    // comparing a five-byte needle at every byte of a long private subtag.
    const WORD_BYTES: usize = 8;
    const BYTE_ONES: u64 = 0x0101_0101_0101_0101;
    const BYTE_HIGH_BITS: u64 = 0x8080_8080_8080_8080;
    policy.charge(ColdOperation::TableRead, 1, 0)?;
    let first = prefix[0];
    let repeated = u64::from(first) * BYTE_ONES;
    let mut i = 0;
    while bytes.len() - i >= prefix.len() {
        if bytes.len() - i >= WORD_BYTES {
            // Fixed byte copy into a stack word, including temporary writes
            // and cleanup. No unproved string searcher or heap allocation.
            policy.charge(ColdOperation::TableRead, WORD_BYTES, 0)?;
            policy.charge(ColdOperation::Initialize, WORD_BYTES + WORD_BYTES, 0)?;
            let word = u64::from_le_bytes(bytes[i..i + WORD_BYTES].try_into().unwrap());
            let diff = word ^ repeated;
            // The standard zero-byte predicate has no false negatives.
            // Any possible match falls through to the exact scalar comparison.
            if diff.wrapping_sub(BYTE_ONES) & !diff & BYTE_HIGH_BITS == 0 {
                i += WORD_BYTES;
                continue;
            }
        }
        policy.charge(ColdOperation::TableRead, 1, 0)?;
        if bytes[i] == first {
            policy.charge(ColdOperation::TableRead, prefix.len(), 0)?;
            policy.charge(ColdOperation::TableRead, prefix.len(), 0)?;
            if &bytes[i..i + prefix.len()] == prefix {
                return Ok(Some(i));
            }
        }
        i += 1;
    }
    Ok(None)
}

#[cfg(test)]
fn lang_cmp(s1: &str, s2: &str) -> core::cmp::Ordering {
    let da = s1.find('-').unwrap_or(s1.len());
    let db = s2.find('-').unwrap_or(s2.len());
    let n = core::cmp::max(da, db);
    let ea = core::cmp::min(n, s1.len());
    let eb = core::cmp::min(n, s2.len());
    s1[..ea].cmp(&s2[..eb])
}

#[cfg(test)]
fn tags_from_language(language: &Language, tags: &mut ThreeTags) {
    let language = language.as_str();

    // Check for matches of multiple subtags.
    if tag_table::tags_from_complex_language(language, tags) {
        return;
    }

    let mut sublang = language;

    // Find a language matching in the first component.
    if let Some(i) = language.find('-') {
        // If there is an extended language tag, use it.
        if language.len() >= 6 {
            let extlang = match language[i + 1..].find('-') {
                Some(idx) => idx == 3,
                None => language.len() - i - 1 == 3,
            };

            if extlang && language.as_bytes()[i + 1].is_ascii_alphabetic() {
                sublang = &language[i + 1..];
            }
        }
    }

    use tag_table::OPEN_TYPE_LANGUAGES as LANGUAGES;

    if let Ok(mut idx) = LANGUAGES.binary_search_by(|v| lang_cmp(v.language, sublang)) {
        while idx != 0 && LANGUAGES[idx].language == LANGUAGES[idx - 1].language {
            idx -= 1;
        }

        let len = core::cmp::min(tags.left(), LANGUAGES.len() - idx - 1);
        for i in 0..len {
            if LANGUAGES[idx + i].language != LANGUAGES[idx].language {
                break;
            }

            if LANGUAGES[idx + i].tag.is_null() {
                break;
            }

            if tags.is_full() {
                break;
            }

            tags.push(LANGUAGES[idx + i].tag);
        }

        return;
    }

    if language.len() == 3 {
        tags.push(hb_tag_t::from_bytes_lossy(language.as_bytes()).to_uppercase());
    }
}

fn lang_cmp_with<P: ColdAdmission>(
    s1: &str, s2: &str, policy: &mut P,
) -> Result<core::cmp::Ordering, P::Error> {
    let da = find_private_prefix_with(s1.as_bytes(), b"-", policy)?.unwrap_or(s1.len());
    let db = find_private_prefix_with(s2.as_bytes(), b"-", policy)?.unwrap_or(s2.len());
    let n = core::cmp::max(da, db);
    let ea = core::cmp::min(n, s1.len());
    let eb = core::cmp::min(n, s2.len());
    // Compare visits both byte ranges; no opaque binary_search closure.
    policy.charge(ColdOperation::TableRead, ea, 0)?;
    policy.charge(ColdOperation::TableRead, eb, 0)?;
    Ok(s1.as_bytes()[..ea].cmp(&s2.as_bytes()[..eb]))
}

fn language_equal_with<P: ColdAdmission>(
    a: &str, b: &str, policy: &mut P,
) -> Result<bool, P::Error> {
    policy.charge(ColdOperation::Planning, 2, 0)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    policy.charge(ColdOperation::TableRead, a.len(), 0)?;
    policy.charge(ColdOperation::TableRead, b.len(), 0)?;
    Ok(a == b)
}

fn tags_from_language_with<P: ColdAdmission>(
    language: &str, tags: &mut ThreeTags, policy: &mut P,
) -> Result<(), P::Error> {
    if super::tag_rules::tags_from_complex_language_with(language, tags, policy)? {
        return Ok(());
    }
    let mut sublang = language;
    if let Some(i) = find_private_prefix_with(language.as_bytes(), b"-", policy)? {
        if language.len() >= 6 {
            let remaining = &language[i + 1..];
            let extlang = match find_private_prefix_with(remaining.as_bytes(), b"-", policy)? {
                Some(idx) => idx == 3,
                None => remaining.len() == 3,
            };
            if extlang {
                policy.charge(ColdOperation::TableRead, 1, 0)?;
                if language.as_bytes()[i + 1].is_ascii_alphabetic() {
                    sublang = remaining;
                }
            }
        }
    }
    use tag_table::OPEN_TYPE_LANGUAGES as LANGUAGES;
    let mut start = 0;
    let mut end = LANGUAGES.len();
    let mut found = None;
    while start < end {
        let mid = start + (end - start) / 2;
        policy.charge(ColdOperation::TableRead, 1, 0)?;
        match lang_cmp_with(LANGUAGES[mid].language, sublang, policy)? {
            core::cmp::Ordering::Less => start = mid + 1,
            core::cmp::Ordering::Greater => end = mid,
            core::cmp::Ordering::Equal => {
                found = Some(mid);
                break;
            }
        }
    }
    if let Some(mut idx) = found {
        while idx != 0 {
            policy.charge(ColdOperation::TableRead, 2, 0)?;
            if !language_equal_with(
                LANGUAGES[idx].language, LANGUAGES[idx - 1].language, policy,
            )? {
                break;
            }
            idx -= 1;
        }
        // Preserve the locked table's trailing-slot and inline-capacity limit.
        let len = core::cmp::min(tags.left(), LANGUAGES.len() - idx - 1);
        for i in 0..len {
            policy.charge(ColdOperation::TableRead, 2, 0)?;
            if !language_equal_with(
                LANGUAGES[idx + i].language, LANGUAGES[idx].language, policy,
            )? {
                break;
            }
            policy.charge(ColdOperation::TableRead, 4, 0)?;
            if LANGUAGES[idx + i].tag.is_null() || tags.is_full() {
                break;
            }
            policy.charge(ColdOperation::TableRead, 4, 0)?;
            policy.charge(ColdOperation::Initialize, 4 + 4, 0)?;
            tags.push(LANGUAGES[idx + i].tag);
        }
        return Ok(());
    }
    if language.len() == 3 {
        policy.charge(ColdOperation::TableRead, 3 + 4, 0)?;
        // Lossy padding/copy, ASCII tag normalization and eventual cleanup.
        policy.charge(ColdOperation::Initialize, 4 + 3 + 4 + 4 + 4, 0)?;
        tags.push(hb_tag_t::from_bytes_lossy(language.as_bytes()).to_uppercase());
    }
    Ok(())
}

#[cfg(test)]
mod general_language_reference_tests {
    use super::*;
    use alloc::format;
    use core::str::FromStr;

    #[test]
    fn all_primary_table_languages_and_extended_forms_match_locked_kernel() {
        for entry in tag_table::OPEN_TYPE_LANGUAGES {
            for suffix in ["", "-abc", "-hans", "-hant-mo", "-fonipa", "-x-private"] {
                for input in [format!("{}{suffix}", entry.language),
                    format!("und-{}{suffix}", entry.language)] {
                    let language = Language::from_str(&input).unwrap();
                    let mut expected = ThreeTags::new();
                    tags_from_language(&language, &mut expected);
                    let mut actual = ThreeTags::new();
                    tags_from_language_with(language.as_str(), &mut actual, &mut OrdinaryCold)
                        .unwrap();
                    assert_eq!(actual, expected, "{input}");
                }
            }
        }
    }
}

fn all_tags_from_script(script: Option<Script>, tags: &mut ThreeTags) {
    if let Some(script) = script {
        if let Some(tag) = new_tag_from_script(script) {
            // Script::Myanmar maps to 'mym2', but there is no 'mym3'.
            if tag != hb_tag_t::new(b"mym2") {
                let mut tag3 = tag.to_be_bytes();
                tag3[3] = b'3';
                tags.push(hb_tag_t::new(&tag3));
            }

            if !tags.is_full() {
                tags.push(tag);
            }
        }

        if !tags.is_full() {
            tags.push(old_tag_from_script(script));
        }
    }
}

fn new_tag_from_script(script: Script) -> Option<hb_tag_t> {
    match script {
        script::BENGALI => Some(hb_tag_t::new(b"bng2")),
        script::DEVANAGARI => Some(hb_tag_t::new(b"dev2")),
        script::GUJARATI => Some(hb_tag_t::new(b"gjr2")),
        script::GURMUKHI => Some(hb_tag_t::new(b"gur2")),
        script::KANNADA => Some(hb_tag_t::new(b"knd2")),
        script::MALAYALAM => Some(hb_tag_t::new(b"mlm2")),
        script::ORIYA => Some(hb_tag_t::new(b"ory2")),
        script::TAMIL => Some(hb_tag_t::new(b"tml2")),
        script::TELUGU => Some(hb_tag_t::new(b"tel2")),
        script::MYANMAR => Some(hb_tag_t::new(b"mym2")),
        _ => None,
    }
}

fn old_tag_from_script(script: Script) -> hb_tag_t {
    // This seems to be accurate as of end of 2012.
    match script {
        script::MATH => hb_tag_t::new(b"math"),

        // Katakana and Hiragana both map to 'kana'.
        script::HIRAGANA => hb_tag_t::new(b"kana"),

        // Spaces at the end are preserved, unlike ISO 15924.
        script::LAO => hb_tag_t::new(b"lao "),
        script::YI => hb_tag_t::new(b"yi  "),
        // Unicode-5.0 additions.
        script::NKO => hb_tag_t::new(b"nko "),
        // Unicode-5.1 additions.
        script::VAI => hb_tag_t::new(b"vai "),

        // Else, just change first char to lowercase and return.
        _ => hb_tag_t::from_u32(script.tag().as_u32() | 0x2000_0000),
    }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]

    use super::*;
    use core::str::FromStr;
    use alloc::vec::Vec;

    #[test]
    fn private_prefix_word_skip_matches_every_boundary_and_rejects_before_read() {
        use super::super::face::cold_admission::{AdmittedCold, ColdOperationError};
        for len in 0..64 {
            for fill in [b'a', b'-', 0, 0xff] {
                for needle in [b"-hbot".as_slice(), b"-hbsc".as_slice()] {
                    for position in 0..=len {
                        let mut bytes = alloc::vec![fill; len];
                        bytes.splice(position..position, needle.iter().copied());
                        let expected = bytes.windows(needle.len()).position(|window| window == needle);
                        let mut complete = Vec::new();
                        let result = find_private_prefix_with(&bytes, needle, &mut AdmittedCold(|op, cost| {
                            complete.push((op, cost));
                            Ok::<_, usize>(())
                        })).unwrap();
                        assert_eq!(result, expected);
                        // Exercise the first, middle and last operation in each
                        // boundary case; public tests reject every full plan point.
                        for reject in [0, complete.len() / 2, complete.len() - 1] {
                            let mut prefix = Vec::new();
                            let result = find_private_prefix_with(&bytes, needle, &mut AdmittedCold(|op, cost| {
                                let index = prefix.len();
                                prefix.push((op, cost));
                                if index == reject { Err(index) } else { Ok(()) }
                            }));
                            assert!(matches!(result, Err(ColdOperationError::Admission(index)) if index == reject));
                            assert_eq!(prefix, complete[..=reject]);
                        }
                    }
                }
            }
        }
    }

    fn new_tag_to_script(tag: hb_tag_t) -> Option<Script> {
        match &tag.to_be_bytes() {
            b"bng2" => Some(script::BENGALI),
            b"dev2" => Some(script::DEVANAGARI),
            b"gjr2" => Some(script::GUJARATI),
            b"gur2" => Some(script::GURMUKHI),
            b"knd2" => Some(script::KANNADA),
            b"mlm2" => Some(script::MALAYALAM),
            b"ory2" => Some(script::ORIYA),
            b"tml2" => Some(script::TAMIL),
            b"tel2" => Some(script::TELUGU),
            b"mym2" => Some(script::MYANMAR),
            _ => Some(script::UNKNOWN),
        }
    }

    fn old_tag_to_script(tag: hb_tag_t) -> Option<Script> {
        if tag == hb_tag_t::default_script() {
            return None;
        }

        let mut bytes = tag.to_be_bytes();

        // This side of the conversion is fully algorithmic.

        // Any spaces at the end of the tag are replaced by repeating the last
        // letter.  Eg 'nko ' -> 'Nkoo'
        if bytes[2] == b' ' {
            bytes[2] = bytes[1];
        }
        if bytes[3] == b' ' {
            bytes[3] = bytes[2];
        }

        // Change first char to uppercase.
        bytes[0] = bytes[0].to_ascii_uppercase();

        Some(Script(hb_tag_t::new(&bytes)))
    }

    fn tag_to_script(tag: hb_tag_t) -> Option<Script> {
        let bytes = tag.to_be_bytes();
        if bytes[3] == b'2' || bytes[3] == b'3' {
            let mut tag2 = bytes;
            tag2[3] = b'2';
            return new_tag_to_script(hb_tag_t::new(&tag2));
        }

        old_tag_to_script(tag)
    }

    fn test_simple_tags(tag: &str, script: Script) {
        let tag = hb_tag_t::from_bytes_lossy(tag.as_bytes());

        let (scripts, _) = tags_from_script_and_language(Some(script), None);
        if !scripts.is_empty() {
            assert_eq!(tag, scripts[0]);
        } else {
            assert_eq!(tag, hb_tag_t::default_script());
        }

        assert_eq!(tag_to_script(tag), Some(script));
    }

    #[test]
    fn tag_to_uppercase() {
        assert_eq!(hb_tag_t::new(b"abcd").to_uppercase(), hb_tag_t::new(b"ABCD"));
        assert_eq!(hb_tag_t::new(b"abc ").to_uppercase(), hb_tag_t::new(b"ABC "));
        assert_eq!(hb_tag_t::new(b"ABCD").to_uppercase(), hb_tag_t::new(b"ABCD"));
    }

    #[test]
    fn tag_to_lowercase() {
        assert_eq!(hb_tag_t::new(b"abcd").to_lowercase(), hb_tag_t::new(b"abcd"));
        assert_eq!(hb_tag_t::new(b"abc ").to_lowercase(), hb_tag_t::new(b"abc "));
        assert_eq!(hb_tag_t::new(b"ABCD").to_lowercase(), hb_tag_t::new(b"abcd"));
    }

    #[test]
    fn script_degenerate() {
        assert_eq!(hb_tag_t::new(b"DFLT"), hb_tag_t::default_script());

        // Hiragana and Katakana both map to 'kana'.
        test_simple_tags("kana", script::KATAKANA);

        let (scripts, _) = tags_from_script_and_language(Some(script::HIRAGANA), None);
        assert_eq!(scripts.as_slice(), &[hb_tag_t::new(b"kana")]);

        // Spaces are replaced
        assert_eq!(tag_to_script(hb_tag_t::new(b"be  ")), Script::from_iso15924_tag(hb_tag_t::new(b"Beee")));
    }

    #[test]
    fn script_simple() {
        // Arbitrary non-existent script.
        test_simple_tags("wwyz", Script::from_iso15924_tag(hb_tag_t::new(b"wWyZ")).unwrap());

        // These we don't really care about.
        test_simple_tags("zyyy", script::COMMON);
        test_simple_tags("zinh", script::INHERITED);
        test_simple_tags("zzzz", script::UNKNOWN);

        test_simple_tags("arab", script::ARABIC);
        test_simple_tags("copt", script::COPTIC);
        test_simple_tags("kana", script::KATAKANA);
        test_simple_tags("latn", script::LATIN);

        // These are trickier since their OT script tags have space.
        test_simple_tags("lao ", script::LAO);
        test_simple_tags("yi  ", script::YI);
        // Unicode-5.0 additions.
        test_simple_tags("nko ", script::NKO);
        // Unicode-5.1 additions.
        test_simple_tags("vai ", script::VAI);

        // https://docs.microsoft.com/en-us/typography/opentype/spec/scripttags

        // Unicode-5.2 additions.
        test_simple_tags("mtei", script::MEETEI_MAYEK);
        // Unicode-6.0 additions.
        test_simple_tags("mand", script::MANDAIC);
    }

    macro_rules! test_script_from_language {
        ($name:ident, $tag:expr, $lang:expr, $script:expr) => {
            #[test]
            fn $name() {
                let tag = hb_tag_t::from_bytes_lossy($tag.as_bytes());
                let (scripts, _) = tags_from_script_and_language(
                    $script, Language::from_str($lang).ok().as_ref(),
                );
                if !scripts.is_empty() {
                    assert_eq!(scripts.as_slice(), &[tag]);
                }
            }
        };
    }

    test_script_from_language!(script_from_language_01, "", "", None);
    test_script_from_language!(script_from_language_02, "", "en", None);
    test_script_from_language!(script_from_language_03, "copt", "en", Some(script::COPTIC));
    test_script_from_language!(script_from_language_04, "", "x-hbsc", None);
    test_script_from_language!(script_from_language_05, "copt", "x-hbsc", Some(script::COPTIC));
    test_script_from_language!(script_from_language_06, "abc ", "x-hbscabc", None);
    test_script_from_language!(script_from_language_07, "deva", "x-hbscdeva", None);
    test_script_from_language!(script_from_language_08, "dev2", "x-hbscdev2", None);
    test_script_from_language!(script_from_language_09, "dev3", "x-hbscdev3", None);
    test_script_from_language!(script_from_language_10, "copt", "x-hbotpap0-hbsccopt", None);
    test_script_from_language!(script_from_language_11, "", "en-x-hbsc", None);
    test_script_from_language!(script_from_language_12, "copt", "en-x-hbsc", Some(script::COPTIC));
    test_script_from_language!(script_from_language_13, "abc ", "en-x-hbscabc", None);
    test_script_from_language!(script_from_language_14, "deva", "en-x-hbscdeva", None);
    test_script_from_language!(script_from_language_15, "dev2", "en-x-hbscdev2", None);
    test_script_from_language!(script_from_language_16, "dev3", "en-x-hbscdev3", None);
    test_script_from_language!(script_from_language_17, "copt", "en-x-hbotpap0-hbsccopt", None);

    #[test]
    fn script_indic() {
        fn check(tag1: &str, tag2: &str, tag3: &str, script: Script) {
            let tag1 = hb_tag_t::from_bytes_lossy(tag1.as_bytes());
            let tag2 = hb_tag_t::from_bytes_lossy(tag2.as_bytes());
            let tag3 = hb_tag_t::from_bytes_lossy(tag3.as_bytes());

            let (scripts, _) = tags_from_script_and_language(Some(script), None);
            assert_eq!(scripts.as_slice(), &[tag1, tag2, tag3]);
            assert_eq!(tag_to_script(tag1), Some(script));
            assert_eq!(tag_to_script(tag2), Some(script));
            assert_eq!(tag_to_script(tag3), Some(script));
        }

        check("bng3", "bng2", "beng", script::BENGALI);
        check("dev3", "dev2", "deva", script::DEVANAGARI);
        check("gjr3", "gjr2", "gujr", script::GUJARATI);
        check("gur3", "gur2", "guru", script::GURMUKHI);
        check("knd3", "knd2", "knda", script::KANNADA);
        check("mlm3", "mlm2", "mlym", script::MALAYALAM);
        check("ory3", "ory2", "orya", script::ORIYA);
        check("tml3", "tml2", "taml", script::TAMIL);
        check("tel3", "tel2", "telu", script::TELUGU);
    }

    // TODO: swap tag and lang
    macro_rules! test_tag_from_language {
        ($name:ident, $tag:expr, $lang:expr) => {
            #[test]
            fn $name() {
                let tag = hb_tag_t::from_bytes_lossy($tag.as_bytes());
                let (_, languages) = tags_from_script_and_language(
                    None, Language::from_str(&$lang.to_lowercase()).ok().as_ref(),
                );
                if !languages.is_empty() {
                    assert_eq!(languages[0], tag);
                }
            }
        };
    }

    test_tag_from_language!(tag_from_language_dflt, "dflt", "");
    test_tag_from_language!(tag_from_language_ALT, "ALT", "alt");
    test_tag_from_language!(tag_from_language_ARA, "ARA", "ar");
    test_tag_from_language!(tag_from_language_AZE, "AZE", "az");
    test_tag_from_language!(tag_from_language_az_ir, "AZE", "az-ir");
    test_tag_from_language!(tag_from_language_az_az, "AZE", "az-az");
    test_tag_from_language!(tag_from_language_ENG, "ENG", "en");
    test_tag_from_language!(tag_from_language_en_US, "ENG", "en_US");
    test_tag_from_language!(tag_from_language_CJA, "CJA", "cja"); /* Western Cham */
    test_tag_from_language!(tag_from_language_CJM, "CJM", "cjm"); /* Eastern Cham */
    test_tag_from_language!(tag_from_language_ENV, "EVN", "eve");
    test_tag_from_language!(tag_from_language_HAL, "HAL", "cfm"); /* BCP47 and current ISO639-3 code for Halam/Falam Chin */
    test_tag_from_language!(tag_from_language_flm, "HAL", "flm"); /* Retired ISO639-3 code for Halam/Falam Chin */
    test_tag_from_language!(tag_from_language_hy, "HYE0", "hy");
    test_tag_from_language!(tag_from_language_hyw, "HYE", "hyw");
    test_tag_from_language!(tag_from_language_bgr, "QIN", "bgr"); /* Bawm Chin */
    test_tag_from_language!(tag_from_language_cbl, "QIN", "cbl"); /* Bualkhaw Chin */
    test_tag_from_language!(tag_from_language_cka, "QIN", "cka"); /* Khumi Awa Chin */
    test_tag_from_language!(tag_from_language_cmr, "QIN", "cmr"); /* Mro-Khimi Chin */
    test_tag_from_language!(tag_from_language_cnb, "QIN", "cnb"); /* Chinbon Chin */
    test_tag_from_language!(tag_from_language_cnh, "QIN", "cnh"); /* Hakha Chin */
    test_tag_from_language!(tag_from_language_cnk, "QIN", "cnk"); /* Khumi Chin */
    test_tag_from_language!(tag_from_language_cnw, "QIN", "cnw"); /* Ngawn Chin */
    test_tag_from_language!(tag_from_language_csh, "QIN", "csh"); /* Asho Chin */
    test_tag_from_language!(tag_from_language_csy, "QIN", "csy"); /* Siyin Chin */
    test_tag_from_language!(tag_from_language_ctd, "QIN", "ctd"); /* Tedim Chin */
    test_tag_from_language!(tag_from_language_czt, "QIN", "czt"); /* Zotung Chin */
    test_tag_from_language!(tag_from_language_dao, "QIN", "dao"); /* Daai Chin */
    test_tag_from_language!(tag_from_language_htl, "QIN", "hlt"); /* Matu Chin */
    test_tag_from_language!(tag_from_language_mrh, "QIN", "mrh"); /* Mara Chin */
    test_tag_from_language!(tag_from_language_pck, "QIN", "pck"); /* Paite Chin */
    test_tag_from_language!(tag_from_language_sez, "QIN", "sez"); /* Senthang Chin */
    test_tag_from_language!(tag_from_language_tcp, "QIN", "tcp"); /* Tawr Chin */
    test_tag_from_language!(tag_from_language_tcz, "QIN", "tcz"); /* Thado Chin */
    test_tag_from_language!(tag_from_language_yos, "QIN", "yos"); /* Yos, deprecated by IANA in favor of Zou [zom] */
    test_tag_from_language!(tag_from_language_zom, "QIN", "zom"); /* Zou */
    test_tag_from_language!(tag_from_language_FAR, "FAR", "fa");
    test_tag_from_language!(tag_from_language_fa_IR, "FAR", "fa_IR");
    test_tag_from_language!(tag_from_language_man, "MNK", "man");
    test_tag_from_language!(tag_from_language_SWA, "SWA", "aii"); /* Swadaya Aramaic */
    test_tag_from_language!(tag_from_language_SYR, "SYR", "syr"); /* Syriac [macrolanguage] */
    test_tag_from_language!(tag_from_language_amw, "SYR", "amw"); /* Western Neo-Aramaic */
    test_tag_from_language!(tag_from_language_cld, "SYR", "cld"); /* Chaldean Neo-Aramaic */
    test_tag_from_language!(tag_from_language_syc, "SYR", "syc"); /* Classical Syriac */
    test_tag_from_language!(tag_from_language_TUA, "TUA", "tru"); /* Turoyo Aramaic */
    test_tag_from_language!(tag_from_language_zh, "ZHS", "zh"); /* Chinese */
    test_tag_from_language!(tag_from_language_zh_cn, "ZHS", "zh-cn"); /* Chinese (China) */
    test_tag_from_language!(tag_from_language_zh_sg, "ZHS", "zh-sg"); /* Chinese (Singapore) */
    test_tag_from_language!(tag_from_language_zh_mo, "ZHTM", "zh-mo"); /* Chinese (Macao) */
    test_tag_from_language!(tag_from_language_zh_hant_mo, "ZHTM", "zh-hant-mo"); /* Chinese (Macao) */
    test_tag_from_language!(tag_from_language_zh_hans_mo, "ZHS", "zh-hans-mo"); /* Chinese (Simplified, Macao) */
    test_tag_from_language!(tag_from_language_ZHH, "ZHH", "zh-HK"); /* Chinese (Hong Kong) */
    test_tag_from_language!(tag_from_language_zh_HanT_hK, "ZHH", "zH-HanT-hK"); /* Chinese (Hong Kong) */
    test_tag_from_language!(tag_from_language_zh_HanS_hK, "ZHS", "zH-HanS-hK"); /* Chinese (Simplified, Hong Kong) */
    test_tag_from_language!(tag_from_language_zh_tw, "ZHT", "zh-tw"); /* Chinese (Taiwan) */
    test_tag_from_language!(tag_from_language_ZHS, "ZHS", "zh-Hans"); /* Chinese (Simplified) */
    test_tag_from_language!(tag_from_language_ZHT, "ZHT", "zh-Hant"); /* Chinese (Traditional) */
    test_tag_from_language!(tag_from_language_zh_xx, "ZHS", "zh-xx"); /* Chinese (Other) */
    test_tag_from_language!(tag_from_language_zh_Hans_TW, "ZHS", "zh-Hans-TW");
    test_tag_from_language!(tag_from_language_yue, "ZHH", "yue");
    test_tag_from_language!(tag_from_language_yue_Hant, "ZHH", "yue-Hant");
    test_tag_from_language!(tag_from_language_yue_Hans, "ZHS", "yue-Hans");
    test_tag_from_language!(tag_from_language_ABC, "ABC", "abc");
    test_tag_from_language!(tag_from_language_ABCD, "ABCD", "x-hbotabcd");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbotabc_zxc, "ABC", "asdf-asdf-wer-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbotabc, "ABC", "asdf-asdf-wer-x-hbotabc");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbotabcd, "ABCD", "asdf-asdf-wer-x-hbotabcd");
    test_tag_from_language!(tag_from_language_asdf_asdf_wer_x_hbot_zxc, "dflt", "asdf-asdf-wer-x-hbot-zxc");
    test_tag_from_language!(tag_from_language_xy, "dflt", "xy");
    test_tag_from_language!(tag_from_language_xyz, "XYZ", "xyz"); /* Unknown ISO 639-3 */
    test_tag_from_language!(tag_from_language_xyz_qw, "XYZ", "xyz-qw"); /* Unknown ISO 639-3 */

    /*
     * Invalid input. The precise answer does not matter, as long as it
     * does not crash or get into an infinite loop.
     */
    test_tag_from_language!(tag_from_language__fonipa, "IPPH", "-fonipa");

    /*
     * Tags that contain "-fonipa" as a substring but which do not contain
     * the subtag "fonipa".
     */
    test_tag_from_language!(tag_from_language_en_fonipax, "ENG", "en-fonipax");
    test_tag_from_language!(tag_from_language_en_x_fonipa, "ENG", "en-x-fonipa");
    test_tag_from_language!(tag_from_language_en_a_fonipa, "ENG", "en-a-fonipa");
    test_tag_from_language!(tag_from_language_en_a_qwe_b_fonipa, "ENG", "en-a-qwe-b-fonipa");

    /* International Phonetic Alphabet */
    test_tag_from_language!(tag_from_language_en_fonipa, "IPPH", "en-fonipa");
    test_tag_from_language!(tag_from_language_en_fonipax_fonipa, "IPPH", "en-fonipax-fonipa");
    test_tag_from_language!(tag_from_language_rm_ch_fonipa_sursilv_x_foobar, "IPPH", "rm-CH-fonipa-sursilv-x-foobar");
    test_tag_from_language!(tag_from_language_IPPH, "IPPH", "und-fonipa");
    test_tag_from_language!(tag_from_language_zh_fonipa, "IPPH", "zh-fonipa");

    /* North American Phonetic Alphabet (Americanist Phonetic Notation) */
    test_tag_from_language!(tag_from_language_en_fonnapa, "APPH", "en-fonnapa");
    test_tag_from_language!(tag_from_language_chr_fonnapa, "APPH", "chr-fonnapa");
    test_tag_from_language!(tag_from_language_APPH, "APPH", "und-fonnapa");

    /* Khutsuri Georgian */
    test_tag_from_language!(tag_from_language_ka_geok, "KGE", "ka-Geok");
    test_tag_from_language!(tag_from_language_KGE, "KGE", "und-Geok");

    /* Irish Traditional */
    test_tag_from_language!(tag_from_language_IRT, "IRT", "ga-Latg");

    /* Moldavian */
    test_tag_from_language!(tag_from_language_MOL, "MOL", "ro-MD");

    /* Polytonic Greek */
    test_tag_from_language!(tag_from_language_PGR, "PGR", "el-polyton");
    test_tag_from_language!(tag_from_language_el_CY_polyton, "PGR", "el-CY-polyton");

    /* Estrangela Syriac */
    test_tag_from_language!(tag_from_language_aii_Syre, "SYRE", "aii-Syre");
    test_tag_from_language!(tag_from_language_de_Syre, "SYRE", "de-Syre");
    test_tag_from_language!(tag_from_language_syr_Syre, "SYRE", "syr-Syre");
    test_tag_from_language!(tag_from_language_und_Syre, "SYRE", "und-Syre");

    /* Western Syriac */
    test_tag_from_language!(tag_from_language_aii_Syrj, "SYRJ", "aii-Syrj");
    test_tag_from_language!(tag_from_language_de_Syrj, "SYRJ", "de-Syrj");
    test_tag_from_language!(tag_from_language_syr_Syrj, "SYRJ", "syr-Syrj");
    test_tag_from_language!(tag_from_language_SYRJ, "SYRJ", "und-Syrj");

    /* Eastern Syriac */
    test_tag_from_language!(tag_from_language_aii_Syrn, "SYRN", "aii-Syrn");
    test_tag_from_language!(tag_from_language_de_Syrn, "SYRN", "de-Syrn");
    test_tag_from_language!(tag_from_language_syr_Syrn, "SYRN", "syr-Syrn");
    test_tag_from_language!(tag_from_language_SYRN, "SYRN", "und-Syrn");

    /* Test that x-hbot overrides the base language */
    test_tag_from_language!(tag_from_language_fa_x_hbotabc_zxc, "ABC", "fa-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_fa_ir_x_hbotabc_zxc, "ABC", "fa-ir-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_zh_x_hbotabc_zxc, "ABC", "zh-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_zh_cn_x_hbotabc_zxc, "ABC", "zh-cn-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_zh_xy_x_hbotabc_zxc, "ABC", "zh-xy-x-hbotabc-zxc");
    test_tag_from_language!(tag_from_language_xyz_xy_x_hbotabc_zxc, "ABC", "xyz-xy-x-hbotabc-zxc");

    /* Unnormalized BCP 47 tags */
    test_tag_from_language!(tag_from_language_ar_aao, "ARA", "ar-aao");
    test_tag_from_language!(tag_from_language_art_lojban, "JBO", "art-lojban");
    test_tag_from_language!(tag_from_language_kok_gom, "KOK", "kok-gom");
    test_tag_from_language!(tag_from_language_i_lux, "LTZ", "i-lux");
    test_tag_from_language!(tag_from_language_drh, "MNG", "drh");
    test_tag_from_language!(tag_from_language_ar_ary1, "MOR", "ar-ary");
    test_tag_from_language!(tag_from_language_ar_ary_DZ, "MOR", "ar-ary-DZ");
    test_tag_from_language!(tag_from_language_no_bok, "NOR", "no-bok");
    test_tag_from_language!(tag_from_language_no_nyn, "NYN", "no-nyn");
    test_tag_from_language!(tag_from_language_i_hak, "ZHS", "i-hak");
    test_tag_from_language!(tag_from_language_zh_guoyu, "ZHS", "zh-guoyu");
    test_tag_from_language!(tag_from_language_zh_min, "ZHS", "zh-min");
    test_tag_from_language!(tag_from_language_zh_min_nan, "ZHS", "zh-min-nan");
    test_tag_from_language!(tag_from_language_zh_xiang, "ZHS", "zh-xiang");

    /* BCP 47 tags that look similar to unrelated language system tags */
    test_tag_from_language!(tag_from_language_als, "SQI", "als");
    test_tag_from_language!(tag_from_language_far, "dflt", "far");

    /* A UN M.49 region code, not an extended language subtag */
    test_tag_from_language!(tag_from_language_ar_001, "ARA", "ar-001");

    /* An invalid tag */
    test_tag_from_language!(tag_from_language_invalid, "TRK", "tr@foo=bar");

    macro_rules! test_tags {
        ($name:ident, $script:expr, $lang:expr, $scripts:expr, $langs:expr) => {
            #[test]
            fn $name() {
                let (scripts, languages) = tags_from_script_and_language(
                    $script, Language::from_str($lang).ok().as_ref(),
                );

                let exp_scripts: Vec<hb_tag_t> = $scripts.iter().map(|v| hb_tag_t::from_bytes_lossy(*v)).collect();
                let exp_langs: Vec<hb_tag_t> = $langs.iter().map(|v| hb_tag_t::from_bytes_lossy(*v)).collect();

                assert_eq!(exp_scripts, scripts.as_slice());
                assert_eq!(exp_langs, languages.as_slice());
            }
        };
    }

    test_tags!(tag_full_en, None, "en", &[], &[b"ENG"]);
    test_tags!(tag_full_en_x_hbscdflt, None, "en-x-hbscdflt", &[b"DFLT"], &[b"ENG"]);
    test_tags!(tag_full_en_latin, Some(script::LATIN), "en", &[b"latn"], &[b"ENG"]);
    test_tags!(tag_full_und_fonnapa, None, "und-fonnapa", &[], &[b"APPH"]);
    test_tags!(tag_full_en_fonnapa, None, "en-fonnapa", &[], &[b"APPH"]);
    test_tags!(tag_full_x_hbot1234_hbsc5678, None, "x-hbot1234-hbsc5678", &[b"5678"], &[b"1234"]);
    test_tags!(tag_full_x_hbsc5678_hbot1234, None, "x-hbsc5678-hbot1234", &[b"5678"], &[b"1234"]);
    test_tags!(tag_full_ml, Some(script::MALAYALAM), "ml", &[b"mlm3", b"mlm2", b"mlym"], &[b"MAL", b"MLR"]);
    test_tags!(tag_full_xyz, None, "xyz", &[], &[b"XYZ"]);
    test_tags!(tag_full_xy, None, "xy", &[], &[]);
}
