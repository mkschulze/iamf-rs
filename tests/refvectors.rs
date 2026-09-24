//! CONF-10/CONF-11's disposition layer: every reachable reference vector,
//! joined against the `is_valid` declaration in its paired upstream
//! `.textproto`.
//!
//! Serves CONF-10, CONF-11, OBU-08, PARSE-04, GUARD-06, D-14 and D-25.
//!
//! Nothing in this repository reads the upstream disposition fields. The
//! vendored corpus is walked for framing (`tests/refcorpus.rs`) and pinned by a
//! semantic digest (`tests/parse_reference.rs`), but the declaration the
//! reference project itself attaches to each vector — "this file is valid" /
//! "this file is not" — has never been joined to what this crate does with the
//! bytes. This file makes that join, and makes it reusable at a second root so
//! the same classifier runs over the ~221-file fetched corpus where the real
//! codec and layout coverage is.
//!
//! **Why a new test binary.** `tests/conformance.rs` is the decode-oracle
//! concern and is already the largest file in the repository;
//! `tests/refcorpus.rs` asserts framing only and deliberately parses nothing;
//! and `tests/vectors.rs` is a name trap — it is BITS-06/D-25 hand-computed bit
//! primitives, not the vector corpus. A fourth concern gets a fourth binary.
//! There is no `[[test]]` entry for it: Cargo auto-discovers `tests/*.rs`, and
//! `tools/prove-guards.sh` copies only `src/` into its proof crate before
//! running `cargo clippy --all-targets` there, so an unconditional manifest
//! entry pointing at a `tests/` file that directory does not have would break
//! all nine guard cases.
//!
//! **Why a printed skip rather than `#[ignore]`.** There is no `#[ignore]`
//! anywhere in `tests/*.rs`. The established convention is to print a `SKIP`
//! line and return — `tests/conformance.rs:179-203`. It is kept here because a
//! skipped test that passes and a real test that passes are the same green
//! tick, and the *skip* is the thing CI has to be able to see.
//!
//! **Which is why the printed counts are a contract.** A harness that skips is
//! indistinguishable from a harness that measured something, so
//! `.github/workflows/reference.yml` greps this file's printed markers in the
//! one job where the corpus exists and fails there if the corpus layer skipped.
//! Renaming either marker silently disarms that gate.

use std::fs;
use std::path::{Path, PathBuf};

use iamf::obu::find_obu_boundaries;
use iamf::sequence::{ParsedSequence, parse_sequence, write_parsed_sequence};

// ---------------------------------------------------------------------------
// Roots
// ---------------------------------------------------------------------------

/// The committed foreign corpus. Always present, all four targets — which is
/// what keeps the classifier below exercised code rather than a promise about
/// one Linux job.
const VENDORED_ROOT: &str = "tests/fixtures/reference";

/// The fetched `libiamf@v1.1.0` vector tree.
///
/// `tools/build-reference.sh` owns this path and is its single source of truth:
/// `REPO_ROOT` at `:36`, `REF_DIR="$REPO_ROOT/.reference"` at `:39`,
/// `SRC="$REF_DIR/libiamf"` at `:40`, and the smoke input
/// `"$SRC/tests/test_000003.iamf"` at `:267`. A new environment variable was
/// deliberately *not* introduced for it: the path is already deterministic and
/// single-owner, and a second resolution path would need a hardcoded fallback
/// anyway. If that script moves `REF_DIR` or `SRC`, this constant moves with it.
const CORPUS_ROOT: &str = ".reference/libiamf/tests";

/// The non-skip floor for the fetched corpus.
///
/// `tests/fixtures/MANIFEST.md:343` records a counted table: the pinned
/// `libiamf@v1.1.0` `tests/` tree holds 221 `.iamf` and 221 `.textproto`. This
/// is a deliberately conservative floor, not a census — its job is to catch a
/// corpus that arrived half-checked-out, not to pin the upstream count. Tighten
/// it to the number the first green reference-job run prints, citing that run.
const MIN_CORPUS_VECTORS: usize = 200;

// ---------------------------------------------------------------------------
// The disposition cells
// ---------------------------------------------------------------------------

/// What `parse_sequence` plus `validate()` did with the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parse {
    Clean,
    Findings,
    Rejected,
}

impl Parse {
    fn tag(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Findings => "findings",
            Self::Rejected => "rejected",
        }
    }
}

/// What `find_obu_boundaries` did with the same bytes (OBU-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walk {
    EndsOnLength,
    Mismatch,
    Error,
}

impl Walk {
    fn tag(self) -> &'static str {
        match self {
            Self::EndsOnLength => "ends-on-length",
            Self::Mismatch => "mismatch",
            Self::Error => "error",
        }
    }
}

/// What parse-then-write did (PARSE-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoundTrip {
    Identical,
    Canonicalized,
    Diverged,
    NotApplicable,
}

impl RoundTrip {
    fn tag(self) -> &'static str {
        match self {
            Self::Identical => "identical",
            Self::Canonicalized => "canonicalized",
            Self::Diverged => "diverged",
            Self::NotApplicable => "n-a",
        }
    }
}

/// One vector's complete disposition: the two upstream declarations, and the
/// three things this crate did with the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Verdict {
    parse: Parse,
    findings: usize,
    walk: Walk,
    round_trip: RoundTrip,
    /// `true` when a sibling `.textproto` was found at all. Kept separate from
    /// the two flags so "no paired metadata" and "paired metadata that does not
    /// carry the key" stay distinguishable instead of both reading as `None`.
    paired: bool,
    is_valid: Option<bool>,
    is_valid_to_decode: Option<bool>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
//
// Every helper below is TOTAL: it returns an `Option`, a `Vec` or a `Verdict`
// and never panics. `clippy.toml`'s `allow-unwrap-in-tests` /
// `allow-expect-in-tests` / `allow-panic-in-tests` carve-out reaches inside
// `#[test]` function bodies only, so the asserting is done by the tests and the
// measuring is done here. There is deliberately no file-level `#[allow]`.

/// Every `.iamf` directly inside `dir`, sorted, empty when it is unreadable.
///
/// **Non-recursive, on purpose.** Under the vendored root the `iamf-tools/`
/// subdirectory holds the *new*-dialect textproto copies, whose stems
/// (`test_000003`, `test_000134`) collide with different top-level `.iamf`
/// files — a recursive pairing would join a file to another file's metadata,
/// which is worse than no metadata. `negative/` carries no metadata at all.
fn iamf_files_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.extension().is_some_and(|ext| ext == "iamf"))
        .collect();
    paths.sort();
    paths
}

/// The sibling `.textproto`, `Some` only when it is a readable file.
fn paired_textproto(path: &Path) -> Option<PathBuf> {
    let candidate = path.with_extension("textproto");
    candidate.is_file().then_some(candidate)
}

/// The first `key: true` / `key: false` line in a vector textproto.
///
/// A line scanner in the spirit of `manifest_raw_value`
/// (`tests/conformance.rs:1251`), not a textproto parser: these are flat
/// two-space-indented scalars inside the leading `test_vector_metadata` block
/// and no dependency is worth a shape this project already reads by hand.
///
/// The exact-colon strip is the load-bearing detail. Without it a lookup of
/// `is_valid` would be satisfied by the `is_valid_to_decode` line — the two keys
/// disagree on 5 of the 34 vendored vectors, so that confusion would not even
/// be visible as a failure, only as a wrong answer.
fn scan_flag(text: &str, key: &str) -> Option<bool> {
    let prefix = format!("{key}:");
    text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix(prefix.as_str())?;
        match rest.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    })
}

/// The semantic projection: derived `Debug` over the whole model plus the
/// ordered findings, the same shape as `tests/parse_reference.rs:96`.
///
/// Used for exactly one question — is a byte difference a canonicalisation or a
/// divergence — and never as evidence on its own.
fn projection(sequence: &ParsedSequence) -> String {
    format!("{sequence:#?}\nfindings={:#?}", sequence.validate())
}

/// Classify one file. Total.
fn verdict_for(bytes: &[u8], metadata: Option<&str>) -> Verdict {
    let walk = match find_obu_boundaries(bytes) {
        Err(_) => Walk::Error,
        Ok(boundaries) => {
            if boundaries.first().copied() == Some(0)
                && boundaries.last().copied() == Some(bytes.len())
            {
                Walk::EndsOnLength
            } else {
                Walk::Mismatch
            }
        }
    };

    let (parse, findings, round_trip) = match parse_sequence(bytes) {
        Err(_) => (Parse::Rejected, 0, RoundTrip::NotApplicable),
        Ok(sequence) => {
            let findings = sequence.validate().len();
            let parse = if findings == 0 {
                Parse::Clean
            } else {
                Parse::Findings
            };
            let round_trip = match write_parsed_sequence(Vec::new(), &sequence) {
                Err(_) => RoundTrip::Diverged,
                Ok(written) if written == bytes => RoundTrip::Identical,
                Ok(written) => {
                    // Only on the unequal branch: the corpus holds files above
                    // 14 MB and formatting a whole parsed sequence per file
                    // would dominate the run (T-tvf-01).
                    match parse_sequence(&written) {
                        Ok(reparsed) if projection(&reparsed) == projection(&sequence) => {
                            RoundTrip::Canonicalized
                        }
                        _ => RoundTrip::Diverged,
                    }
                }
            };
            (parse, findings, round_trip)
        }
    };

    Verdict {
        parse,
        findings,
        walk,
        round_trip,
        paired: metadata.is_some(),
        is_valid: metadata.and_then(|text| scan_flag(text, "is_valid")),
        is_valid_to_decode: metadata.and_then(|text| scan_flag(text, "is_valid_to_decode")),
    }
}

/// Classify every `.iamf` directly under `root`.
///
/// `None` is the skip signal: the root is not a directory, so there is nothing
/// to measure. An unreadable file is announced and dropped, which makes the
/// count assertions in the tests fail rather than letting the walk quietly
/// shrink.
fn walk(root: &Path) -> Option<Vec<(String, Verdict)>> {
    if !root.is_dir() {
        return None;
    }
    let mut rows = Vec::new();
    for path in iamf_files_in(root) {
        let Some(name) = path.file_name().map(|name| name.to_string_lossy().into_owned()) else {
            continue;
        };
        let Ok(bytes) = fs::read(&path) else {
            println!("refvectors: UNREADABLE {}", path.display());
            continue;
        };
        let metadata = paired_textproto(&path).and_then(|meta| fs::read_to_string(meta).ok());
        rows.push((name, verdict_for(&bytes, metadata.as_deref())));
    }
    Some(rows)
}

fn flag_cell(flag: Option<bool>) -> &'static str {
    match flag {
        Some(true) => "true",
        Some(false) => "false",
        None => "-",
    }
}

fn cell_count(rows: &[(String, Verdict)], flag: Option<bool>, parse: Parse) -> usize {
    rows.iter()
        .filter(|(_, verdict)| verdict.is_valid == flag && verdict.parse == parse)
        .count()
}

/// One deterministic line per file, then the four-cell matrix, then the count
/// line the CI step greps.
fn print_table(label: &str, rows: &[(String, Verdict)]) {
    for (name, verdict) in rows {
        println!(
            "refvectors[{label}] {name:<40} valid={:<5} decode={:<5} parse={:<8} findings={:<3} \
             walk={:<14} roundtrip={}",
            flag_cell(verdict.is_valid),
            flag_cell(verdict.is_valid_to_decode),
            verdict.parse.tag(),
            verdict.findings,
            verdict.walk.tag(),
            verdict.round_trip.tag(),
        );
    }

    println!("refvectors[{label}] summary: upstream is_valid x our parse disposition");
    println!("refvectors[{label}]   is_valid | clean | findings | rejected");
    for flag in [Some(true), Some(false), None] {
        println!(
            "refvectors[{label}]   {:<8} | {:<5} | {:<8} | {}",
            flag_cell(flag),
            cell_count(rows, flag, Parse::Clean),
            cell_count(rows, flag, Parse::Findings),
            cell_count(rows, flag, Parse::Rejected),
        );
    }

    // These two lines are a contract with `.github/workflows/reference.yml`,
    // which greps them literally. Renaming either disarms the CI gate.
    if label == "vendored" {
        println!(
            "refvectors[vendored]: walked {} .iamf, {} paired",
            rows.len(),
            rows.iter().filter(|(_, verdict)| verdict.paired).count(),
        );
    } else {
        println!(
            "refvectors[corpus]: walked {} reference vectors",
            rows.len()
        );
    }
}

/// Every breach of the three rules this harness enforces. Everything else it
/// merely records.
///
/// A **false** `is_valid` is deliberately NOT a rule. It is a semantic
/// declaration about a vector the reference project generated on purpose, not
/// an instruction to reject the bytes: 12 of the 34 vendored textprotos declare
/// it and 11 of those parse cleanly here, so an equality rule would be red on 11
/// files on day one and would apply steady pressure toward loosening a correct
/// parser.
fn violations(rows: &[(String, Verdict)], stricter_allowed: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for (name, verdict) in rows {
        // V1 — the RED cell.
        if verdict.is_valid == Some(true)
            && verdict.parse == Parse::Rejected
            && !stricter_allowed.contains(&name.as_str())
        {
            out.push(format!(
                "{name}: RED CELL — the paired textproto declares is_valid: true and we reject it \
                 structurally. That is either a real parser bug, or a deliberate \
                 stricter-than-libiamf decision, which needs a CONFORMANCE-GATE.md subsection \
                 under 'Reference limits diagnosed, not enforced' and a STRICTER_THAN_REFERENCE \
                 row naming this file."
            ));
        }
        // V2 — parse-then-write lost or changed information.
        if verdict.round_trip == RoundTrip::Diverged {
            out.push(format!(
                "{name}: parse-then-write diverged — the written bytes differ from the input and a \
                 re-parse of our own output does not project to the same model plus findings, so \
                 this is not the documented non-minimal obu_size canonicalisation (PARSE-04)."
            ));
        }
        // V3 — the two readers disagree about the same bytes.
        if matches!(verdict.parse, Parse::Clean | Parse::Findings)
            && verdict.walk != Walk::EndsOnLength
        {
            out.push(format!(
                "{name}: parse_sequence accepted the file but the boundary walk is '{}' — the parse \
                 path and the framing path disagree about the same bytes, which neither test would \
                 catch on its own (OBU-08).",
                verdict.walk.tag()
            ));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The always-on layer: 35 real foreign files, on all four targets, with no
/// reference build and no network.
#[test]
fn every_vendored_pair_is_classified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(VENDORED_ROOT);
    let rows = walk(&root).expect("the vendored corpus is committed to this repository");

    print_table("vendored", &rows);

    assert_eq!(
        rows.len(),
        35,
        "the vendored root holds 35 top-level .iamf files; a different number means a fixture was \
         added or dropped without updating this harness"
    );
    assert_eq!(
        rows.iter().filter(|(_, verdict)| verdict.paired).count(),
        34,
        "34 of the 35 have a paired .textproto; test_000076_aac_lc.iamf is the one that does not"
    );

    let breaches = violations(&rows, &[]);
    assert!(
        breaches.is_empty(),
        "vendored vector dispositions breached the harness rules:\n{}",
        breaches.join("\n")
    );
}

/// The gated layer: the same walker over the fetched `libiamf@v1.1.0` vector
/// tree, which is where the codec and layout coverage the vendored subset
/// deliberately excludes actually lives.
#[test]
fn the_reference_corpus_agrees_with_its_own_textprotos() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS_ROOT);
    let Some(rows) = walk(&root) else {
        println!(
            "SKIP refvectors[corpus]: {CORPUS_ROOT} is absent. Run `bash \
             tools/build-reference.sh` to fetch the pinned libiamf tree, whose checkout \
             materialises the vector corpus as a by-product. This is the expected offline state \
             (CONF-10)."
        );
        return;
    };

    print_table("corpus", &rows);

    assert!(
        rows.len() >= MIN_CORPUS_VECTORS,
        "the corpus walk found {} vectors, below the floor of {MIN_CORPUS_VECTORS} — a \
         half-materialised checkout measures almost nothing while still passing",
        rows.len()
    );

    let breaches = violations(&rows, &[]);
    assert!(
        breaches.is_empty(),
        "reference corpus dispositions breached the harness rules:\n{}",
        breaches.join("\n")
    );
}

/// The scanner's one real hazard, pinned directly: `is_valid` must not be
/// answered by the `is_valid_to_decode` line.
#[test]
fn the_flag_scanner_reads_the_paired_metadata_block() {
    let metadata = "\
test_vector_metadata {
  human_readable_description:
    \"anchor elements must be unique\"
  file_name_prefix: \"test_000063\"
  is_valid: false
  is_valid_to_decode: true
  validate_user_loudness: true
}
";

    assert_eq!(scan_flag(metadata, "is_valid"), Some(false));
    assert_eq!(scan_flag(metadata, "is_valid_to_decode"), Some(true));
    assert_eq!(scan_flag(metadata, "validate_user_loudness"), Some(true));
    assert_eq!(scan_flag(metadata, "mp4_fixed_timestamp"), None);
    assert_eq!(scan_flag(metadata, "file_name_prefix"), None);
}
