//! Committed per-vector disposition ledger for the vendored reference corpus.
//!
//! Two kinds of column live here and they have different standing.
//!
//! - `is_valid` and `is_valid_to_decode` are **transcribed from the paired
//!   upstream `.textproto`** and are authoritative. They say what the reference
//!   project declares about the vector, not what this crate thinks of it.
//! - `parse`, `findings`, `walk` and `round_trip` are **this crate's own
//!   behaviour**, committed under D-25 as a change detector and therefore never
//!   conformance evidence on their own. Their only job is to turn a drift into
//!   a reviewable red diff instead of a silently changed printout — the same
//!   contract `tests/support/reference_expectations.rs` holds for the semantic
//!   digests.
//!
//! Every entry in `STRICTER_THAN_REFERENCE` must have a matching
//! `CONFORMANCE-GATE.md` subsection under "Reference limits diagnosed, not
//! enforced". The slice is the machine-readable half of that document; a name
//! here with no section there is an undocumented divergence from the pinned
//! reference.
//!
//! This file is data. Helpers belong in `tests/refvectors.rs`, which is the
//! only binary that includes it.

/// One vendored `.iamf` and everything the harness observed about it.
#[derive(Debug, Clone, Copy)]
pub struct VectorDisposition {
    /// File name relative to `tests/fixtures/reference`, top level only.
    pub path: &'static str,
    /// Upstream `test_vector_metadata.is_valid`, `None` when unpaired.
    pub is_valid: Option<bool>,
    /// Upstream `test_vector_metadata.is_valid_to_decode`, `None` when unpaired.
    pub is_valid_to_decode: Option<bool>,
    /// `Parse::tag()` — "clean", "findings" or "rejected".
    pub parse: &'static str,
    /// `ParsedSequence::validate().len()`.
    pub findings: usize,
    /// `Walk::tag()` — "ends-on-length", "mismatch" or "error".
    pub walk: &'static str,
    /// `RoundTrip::tag()` — "identical", "canonicalized", "diverged" or "n-a".
    pub round_trip: &'static str,
}

/// One row per top-level vendored `.iamf`, in the walker's sorted order.
///
/// Measured on 2026-09-25 by `cargo test --locked --test refvectors --
/// --nocapture`. The four-cell summary over the 34 paired vectors at that
/// commit: `is_valid: true` → 2 clean, 20 findings, **0 rejected**;
/// `is_valid: false` → 0 clean, 11 findings, 1 rejected. The RED cell
/// (`is_valid: true` and we reject) is empty, which is why
/// `STRICTER_THAN_REFERENCE` is empty below.
pub const VENDORED_DISPOSITIONS: &[VectorDisposition] = &[
    VectorDisposition {
        path: "test_000000_3.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(false),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000002.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000003.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000005.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000006.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000007.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(false),
        parse: "findings",
        findings: 2,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000012.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000013.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000015.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 2,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000016.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(false),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000017.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000018.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000019.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000059.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000060.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000062.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000063.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(false),
        parse: "findings",
        findings: 2,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000067.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000071.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "clean",
        findings: 0,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000076_aac_lc.iamf",
        is_valid: None,
        is_valid_to_decode: None,
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000077.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000078.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000079.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000085.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(false),
        parse: "findings",
        findings: 2,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000088.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "clean",
        findings: 0,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000097.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000119.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 4,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000120.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 3,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000121.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000122.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 3,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000124.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(false),
        parse: "findings",
        findings: 8,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000129.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(true),
        parse: "rejected",
        findings: 0,
        walk: "ends-on-length",
        round_trip: "n-a",
    },
    VectorDisposition {
        path: "test_000130.iamf",
        is_valid: Some(false),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 3,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000501.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
    VectorDisposition {
        path: "test_000503.iamf",
        is_valid: Some(true),
        is_valid_to_decode: Some(true),
        parse: "findings",
        findings: 1,
        walk: "ends-on-length",
        round_trip: "identical",
    },
];

/// Vendored vectors the upstream textproto declares `is_valid: true` and this
/// crate nevertheless rejects structurally — a deliberate stricter-than-libiamf
/// position, one `CONFORMANCE-GATE.md` subsection each.
///
/// **Empty at this commit, and that is the measured result, not a placeholder.**
/// The one structural reject among the vendored 35 is `test_000129.iamf`
/// (`UnexpectedEndOfInput` at offset 53, `reference_expectations.rs:210-217`),
/// and its own textproto declares `is_valid: false` — so it is the acceptable
/// cell, not the red one. A name added here without its section is a divergence
/// nobody wrote down.
pub const STRICTER_THAN_REFERENCE: &[&str] = &[];
