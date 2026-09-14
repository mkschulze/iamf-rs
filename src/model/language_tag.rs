//! BCP-47 well-formedness for Mix Presentation `annotations_language` (DESC-05).
//!
//! IAMF v1.1.0 requires every `annotations_language` to conform to BCP-47. By
//! user decision (quick 260914-m62) this crate checks the RFC 5646 section 2.1
//! ABNF — the "well-formed" conformance class — and nothing more.
//!
//! # Well-formed, not valid
//!
//! RFC 5646 section 2.2.9 defines a second, stricter class, "valid", which also
//! requires every subtag to be in the IANA Language Subtag Registry and forbids
//! duplicate variants and singletons. It also says validity "depends on the
//! date of the registry used". A pinned registry table goes stale and a live
//! one would make `validate()` output drift from run to run, and determinism is
//! a project constraint. So an unregistered but well-formed tag such as `qaa`
//! is accepted, and no duplicate-variant or duplicate-singleton rule applies.
//!
//! # Case and alphabet
//!
//! Tags are US-ASCII only, so any non-ASCII or non-UTF-8 byte fails the
//! grammar. Subtags are matched case-insensitively (section 2.1.1); the tag is
//! never canonicalised.
//!
//! # Where it is used
//!
//! `MixPresentation::validate` reports a finding and `EncoderBuilder::build`
//! refuses the tag. Parsing and writing never call this, so a malformed tag
//! still round-trips byte-exactly (PARSE-04).

use core::iter::Peekable;

// ref: IAMF v1.1.0 index.bs:1273 (annotations_language SHALL conform to BCP-47)
// ref: RFC 5646 section 2.1 (Language-Tag ABNF), section 2.1.1 (case, 8-character subtags), section 2.2.8 (irregular grandfathered tags)
// DISAGREEMENT: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:471-479, 523-530 and libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:749 carry the bytes unchecked; the spec is stricter and wins (diagnosed by validate(), refused only by EncoderBuilder::build)

/// The irregular grandfathered tags of RFC 5646 section 2.2.8. They do not
/// match `langtag`, so they are accepted as whole, case-insensitive literals.
/// The regular grandfathered tags (`art-lojban`, `zh-min-nan`, ...) already
/// match `langtag` and need no entry.
const IRREGULAR_GRANDFATHERED: [&[u8]; 17] = [
    b"en-GB-oed",
    b"i-ami",
    b"i-bnn",
    b"i-default",
    b"i-enochian",
    b"i-hak",
    b"i-klingon",
    b"i-lux",
    b"i-mingo",
    b"i-navajo",
    b"i-pwn",
    b"i-tao",
    b"i-tay",
    b"i-tsu",
    b"sgn-BE-FR",
    b"sgn-BE-NL",
    b"sgn-CH-DE",
];

/// Whether `tag` is a well-formed BCP-47 language tag (RFC 5646 section 2.1).
///
/// One greedy forward pass with no allocation and no backtracking: every
/// alternative in the grammar is disjoint by length or character class, so the
/// first match is the only match. An empty subtag (leading, trailing or doubled
/// `-`, or an empty tag) fails every class test.
pub(crate) fn is_well_formed_language_tag(tag: &[u8]) -> bool {
    if IRREGULAR_GRANDFATHERED
        .iter()
        .any(|irregular| irregular.eq_ignore_ascii_case(tag))
    {
        return true;
    }
    let mut subtags = tag.split(|byte| *byte == b'-').peekable();
    let Some(first) = subtags.next() else {
        return false;
    };
    // privateuse = "x" 1*("-" (1*8alphanum))
    if is_private_use_marker(first) {
        return private_use_tail(&mut subtags);
    }
    // language = 2*3ALPHA ["-" extlang] / 4ALPHA / 5*8ALPHA
    if !is_alpha(first, 2, 8) {
        return false;
    }
    // extlang = 3ALPHA *2("-" 3ALPHA), only after a 2-3 letter language
    if first.len() <= 3 {
        for _ in 0..3 {
            if subtags.next_if(|subtag| is_alpha(subtag, 3, 3)).is_none() {
                break;
            }
        }
    }
    // script = 4ALPHA
    let _script = subtags.next_if(|subtag| is_alpha(subtag, 4, 4));
    // region = 2ALPHA / 3DIGIT
    let _region = subtags.next_if(|subtag| is_alpha(subtag, 2, 2) || is_digits(subtag, 3));
    // variant = 5*8alphanum / (DIGIT 3alphanum)
    while subtags.next_if(|subtag| is_variant(subtag)).is_some() {}
    // extension = singleton 1*("-" (2*8alphanum))
    while subtags.next_if(|subtag| is_singleton(subtag)).is_some() {
        if subtags
            .next_if(|subtag| is_alphanumeric(subtag, 2, 8))
            .is_none()
        {
            return false;
        }
        while subtags
            .next_if(|subtag| is_alphanumeric(subtag, 2, 8))
            .is_some()
        {}
    }
    // ["-" privateuse]
    if subtags
        .next_if(|subtag| is_private_use_marker(subtag))
        .is_some()
    {
        return private_use_tail(&mut subtags);
    }
    subtags.peek().is_none()
}

/// The `1*("-" (1*8alphanum))` tail after an `x` subtag: at least one subtag,
/// and every remaining subtag 1-8 alphanumerics.
fn private_use_tail<'a, I: Iterator<Item = &'a [u8]>>(subtags: &mut Peekable<I>) -> bool {
    if subtags.peek().is_none() {
        return false;
    }
    subtags.all(|subtag| is_alphanumeric(subtag, 1, 8))
}

/// `x` or `X`, the private-use marker.
fn is_private_use_marker(subtag: &[u8]) -> bool {
    subtag.eq_ignore_ascii_case(b"x")
}

/// A single alphanumeric other than `x`/`X`.
fn is_singleton(subtag: &[u8]) -> bool {
    is_alphanumeric(subtag, 1, 1) && !is_private_use_marker(subtag)
}

/// `5*8alphanum / (DIGIT 3alphanum)`.
fn is_variant(subtag: &[u8]) -> bool {
    is_alphanumeric(subtag, 5, 8)
        || (is_alphanumeric(subtag, 4, 4) && subtag.first().is_some_and(u8::is_ascii_digit))
}

/// `lo` to `hi` ASCII letters.
fn is_alpha(subtag: &[u8], lo: usize, hi: usize) -> bool {
    (lo..=hi).contains(&subtag.len()) && subtag.iter().all(u8::is_ascii_alphabetic)
}

/// Exactly `len` ASCII digits.
fn is_digits(subtag: &[u8], len: usize) -> bool {
    subtag.len() == len && subtag.iter().all(u8::is_ascii_digit)
}

/// `lo` to `hi` ASCII letters or digits.
fn is_alphanumeric(subtag: &[u8], lo: usize, hi: usize) -> bool {
    (lo..=hi).contains(&subtag.len()) && subtag.iter().all(u8::is_ascii_alphanumeric)
}
