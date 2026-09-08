//! GUARD-09's committed golden and GUARD-10's same-process double-encode.
//!
//! Both are **offline**: no reference binary, no container, no network. That is
//! what puts them in CONF-10's always-on layer and lets them run on all four
//! byte-identity targets in the PR gate, while the expensive reference clauses
//! run on Linux only.
//!
//! # Three artifacts, not one (D-20)
//!
//! `tests/fixtures/golden/` holds exactly three files for the sample-identity
//! fixture: the `.iamf` itself, its SHA-256, and the annotated structural dump.
//! The dump is what makes an output change a **reviewable PR diff** — GUARD-09's
//! whole stated rationale — which neither a bare hash (a one-line diff that
//! explains nothing) nor a bare binary blob delivers.
//!
//! # What the golden proves, and what it does not
//!
//! It proves the four targets agree with each other and with the last reviewed
//! output. It is **change detection**. A golden this crate generated cannot be
//! evidence that the bytes are *correct*: it is self-consistent with the writer
//! by construction. Conformance evidence comes from the two reference oracles
//! and from hand-decoded vectors. See `src/dump.rs`'s module documentation,
//! which states the same caveat where a reader of the dumper will meet it.

#[path = "support/fixture.rs"]
mod fixture;

use std::path::{Path, PathBuf};

use iamf::dump::dump_annotated;
use iamf::obu::find_obu_boundaries;

/// The committed golden directory.
fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden")
}

/// Read a committed golden artifact, failing with a message that says how to
/// regenerate it rather than just that a file is missing.
// GUARD-04's `allow-unwrap-in-tests` / `allow-expect-in-tests` /
// `allow-panic-in-tests` carve-out (clippy.toml) applies to `#[test]` functions
// only — a helper reachable from tests is not one. These helpers exist solely to
// build or inspect a fixture, and a failure in them is an environment or
// programming error that must stop the run loudly rather than be swallowed.
// Kept as narrow, per-function allows so a future helper does not inherit the
// exemption silently.
#[allow(clippy::panic)]
fn read_golden(name: &str) -> Vec<u8> {
    let path = golden_dir().join(name);
    match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => panic!(
            "cannot read the committed golden {}: {e}\n\
             Regenerate with:\n\
             \x20 IAMF_REGENERATE_GOLDEN=1 cargo test --test golden regenerate -- --nocapture\n\
             then READ the diff before committing it. A missing golden is not a reason to delete \
             this test — the golden is what makes an output change a reviewable PR diff.",
            path.display()
        ),
    }
}

/// Lowercase hex of a byte slice.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        text.push_str(&format!("{byte:02x}"));
        text
    })
}

/// The SHA-256 of `bytes`, as lowercase hex.
///
/// `sha2` is a **dev-dependency** and reaches nothing shipped:
/// `cargo tree -e normal,no-proc-macro` still lists exactly `iamf` and
/// `thiserror`. It is here rather than a platform tool because GUARD-09 runs
/// this test on Windows MSVC too, where neither `shasum` nor `sha256sum`
/// exists — and rather than hand-rolled, because a hand-rolled digest is a
/// second thing that can be wrong in a test whose job is to detect change.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

/// The first differing offset between two byte slices, with a hex window.
fn describe_first_difference(produced: &[u8], expected: &[u8]) -> Option<String> {
    if produced == expected {
        return None;
    }
    let first = produced
        .iter()
        .zip(expected.iter())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| produced.len().min(expected.len()));
    let start = first.saturating_sub(16);
    let end = first.saturating_add(16).min(produced.len().max(expected.len()));
    let window = |bytes: &[u8]| -> String {
        let end = end.min(bytes.len());
        let start = start.min(end);
        bytes
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    Some(format!(
        "first difference at offset {first} (0x{first:x})\n\
         produced len {} expected len {}\n\
         produced[0x{start:x}..0x{end:x}] = {}\n\
         expected[0x{start:x}..0x{end:x}] = {}",
        produced.len(),
        expected.len(),
        window(produced),
        window(expected),
    ))
}

/// The lines of the annotated dump around `offset`, so a golden mismatch names
/// the field rather than only the byte.
fn dump_window(dump: &str, offset: usize) -> String {
    let needle = format!("{offset:08x}");
    let lines: Vec<&str> = dump.lines().collect();
    let anchor = lines
        .iter()
        .position(|line| line.contains(&needle))
        .or_else(|| {
            // Fall back to the last OBU header at or before the offset.
            lines.iter().enumerate().fold(None, |best, (index, line)| {
                let at = line.split_whitespace().next()?;
                let parsed = usize::from_str_radix(at, 16).ok()?;
                if parsed <= offset { Some(index) } else { best }
            })
        })
        .unwrap_or(0);
    let start = anchor.saturating_sub(6);
    let end = anchor.saturating_add(10).min(lines.len());
    lines
        .get(start..end)
        .unwrap_or_default()
        .join("\n")
}

// ---------------------------------------------------------------------------
// GUARD-09 — the committed golden reproduces on every target
// ---------------------------------------------------------------------------

#[test]
fn the_golden_iamf_reproduces_byte_for_byte() {
    let produced = fixture::sample_identity()
        .encode()
        .expect("the sample-identity fixture encodes");
    let expected = read_golden("phase1_sample_identity.iamf");

    if let Some(difference) = describe_first_difference(&produced, &expected) {
        let dump = dump_annotated(&expected).unwrap_or_default();
        let offset = produced
            .iter()
            .zip(expected.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        panic!(
            "the regenerated fixture does not match the committed golden.\n{difference}\n\n\
             annotated dump of the committed golden around that offset:\n{}\n\n\
             If this change is intended, regenerate all three golden artifacts and review the \
             dump diff — that diff is the point of committing it.",
            dump_window(&dump, offset)
        );
    }
}

#[test]
fn the_golden_hash_matches_the_golden_bytes() {
    let bytes = read_golden("phase1_sample_identity.iamf");
    let committed = String::from_utf8(read_golden("phase1_sample_identity.iamf.sha256"))
        .unwrap_or_default();
    let committed = committed.split_whitespace().next().unwrap_or("").to_owned();
    assert_eq!(
        sha256_hex(&bytes),
        committed,
        "the committed SHA-256 does not describe the committed .iamf — the two golden artifacts \
         have drifted apart, which means one of them was regenerated and the other was not"
    );
}

#[test]
fn the_regenerated_hash_matches_the_committed_hash() {
    let produced = fixture::sample_identity()
        .encode()
        .expect("the sample-identity fixture encodes");
    let committed = String::from_utf8(read_golden("phase1_sample_identity.iamf.sha256"))
        .unwrap_or_default();
    let committed = committed.split_whitespace().next().unwrap_or("").to_owned();
    assert_eq!(
        sha256_hex(&produced),
        committed,
        "this target produced a different fixture from the committed golden. That is exactly the \
         cross-target byte-identity failure GUARD-09's four-target matrix exists to catch; look \
         for a hashed container, a platform-dependent sort, or a float."
    );
}

#[test]
fn the_golden_dump_reproduces() {
    let bytes = read_golden("phase1_sample_identity.iamf");
    let produced = dump_annotated(&bytes).expect("the golden dumps");
    let committed = String::from_utf8(read_golden("phase1_sample_identity.dump.txt"))
        .expect("the committed dump is UTF-8");
    assert_eq!(
        produced, committed,
        "the annotated dump of the committed .iamf differs from the committed dump. Either the \
         dumper changed (regenerate and review) or it is not deterministic (fix that first — a \
         non-deterministic dump cannot serve as a reviewable diff)."
    );
}

#[test]
fn the_dump_is_deterministic_across_repeated_runs() {
    let bytes = read_golden("phase1_sample_identity.iamf");
    let first = dump_annotated(&bytes).expect("the golden dumps");
    let second = dump_annotated(&bytes).expect("the golden dumps");
    let third = dump_annotated(&bytes).expect("the golden dumps");
    assert_eq!(first, second);
    assert_eq!(second, third);
    assert!(
        !first.is_empty(),
        "the dump is empty, so 'deterministic' is vacuous"
    );
}

#[test]
fn the_dump_carries_no_timestamp_and_no_absolute_path() {
    let bytes = read_golden("phase1_sample_identity.iamf");
    let dump = dump_annotated(&bytes).expect("the golden dumps");
    assert!(
        !dump.contains(env!("CARGO_MANIFEST_DIR")),
        "the dump names this machine's checkout path, so it can never match on another target"
    );
    // A path separator following a drive letter or a home root, and the two
    // shapes an RFC-3339 timestamp takes. Any of them makes the dump
    // machine-dependent and the committed copy unreproducible.
    for marker in ["/Users/", "/home/", "C:\\", "Z-", "+00:00"] {
        assert!(
            !dump.contains(marker),
            "the dump contains {marker:?}, which looks like a timestamp or an absolute path"
        );
    }
}

#[test]
fn the_golden_directory_holds_exactly_the_three_d20_artifacts() {
    let mut names: Vec<String> = std::fs::read_dir(golden_dir())
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    assert_eq!(
        names,
        vec![
            "phase1_sample_identity.dump.txt".to_owned(),
            "phase1_sample_identity.iamf".to_owned(),
            "phase1_sample_identity.iamf.sha256".to_owned(),
        ],
        "D-20 requires exactly three golden artifacts: the .iamf, its hash, and the annotated \
         dump"
    );
}

// ---------------------------------------------------------------------------
// GUARD-10 — the same-process double-encode
// ---------------------------------------------------------------------------

/// Two full encodes in one process must produce identical bytes.
///
/// The cheap check that catches **per-process hash seeding** — `HashMap`'s
/// `RandomState` is seeded once per process, so a hashed container whose
/// iteration order reaches the output produces stable bytes within a run and
/// different bytes between runs. A cross-target golden would catch it too, but
/// only intermittently and only in CI; this catches it locally and every time,
/// which is why `HashMap` is banned crate-wide by GUARD-02 *and* checked here.
#[test]
fn double_encode_is_byte_identical() {
    for name in ["sample_identity", "endianness", "structure_only"] {
        let build = || match name {
            "endianness" => fixture::endianness(),
            "structure_only" => fixture::structure_only(),
            _ => fixture::sample_identity(),
        };
        let first = build().encode().expect("the fixture encodes");
        let second = build().encode().expect("the fixture encodes");
        assert!(
            describe_first_difference(&first, &second).is_none(),
            "{name}: two encodes in ONE process differ. That is per-process nondeterminism — a \
             hashed container, an address-dependent sort, or an uninitialised byte — and a \
             cross-target golden would only catch it intermittently.\n{}",
            describe_first_difference(&first, &second).unwrap_or_default()
        );
    }
}

/// The whole-file wrapper and the streaming primitive agree, and the boundary
/// walk lands exactly on `bytes.len()` for every fixture (OBU-08).
#[test]
fn every_fixture_walks_to_its_own_length() {
    for fixture in fixture::all() {
        let bytes = fixture.encode().expect("the fixture encodes");
        let boundaries = find_obu_boundaries(&bytes).expect("the fixture's OBU chain walks");
        assert_eq!(
            boundaries.last().copied(),
            Some(bytes.len()),
            "{}: the final OBU boundary is {:?}, not bytes.len() = {}",
            fixture.name,
            boundaries.last(),
            bytes.len()
        );
    }
}

/// A guard on the guard: `golden_dir` must point inside the repository, or every
/// assertion above is comparing against nothing.
#[test]
fn the_golden_directory_is_inside_the_repository() {
    let dir = golden_dir();
    assert!(
        dir.starts_with(Path::new(env!("CARGO_MANIFEST_DIR"))),
        "the golden directory {} is outside the checkout",
        dir.display()
    );
}

// ---------------------------------------------------------------------------
// Regeneration — deliberately env-gated, deliberately not a script
// ---------------------------------------------------------------------------

/// Rewrite the three golden artifacts. **Only** with `IAMF_REGENERATE_GOLDEN=1`.
///
/// It lives here rather than in a `tools/` script for one reason: the thing that
/// regenerates the golden and the thing that checks it must build the fixture
/// through exactly the same code, or a regeneration can "fix" a mismatch that
/// the check would have caught. Being env-gated rather than automatic is the
/// other half: a golden that regenerates itself on mismatch detects nothing.
///
/// After running it, **read the diff**. That is the whole point of committing
/// the annotated dump alongside the bytes.
#[test]
fn regenerate_golden_artifacts() {
    if std::env::var_os("IAMF_REGENERATE_GOLDEN").is_none() {
        println!(
            "SKIP regenerate_golden_artifacts: set IAMF_REGENERATE_GOLDEN=1 to rewrite \
             tests/fixtures/golden/. This is the expected state — a golden that regenerates \
             itself detects nothing."
        );
        return;
    }
    let bytes = fixture::sample_identity()
        .encode()
        .expect("the sample-identity fixture encodes");
    let dump = dump_annotated(&bytes).expect("the fixture dumps");
    let digest = sha256_hex(&bytes);

    let dir = golden_dir();
    std::fs::create_dir_all(&dir).expect("the golden directory is creatable");
    std::fs::write(dir.join("phase1_sample_identity.iamf"), &bytes).expect("the .iamf is writable");
    std::fs::write(
        dir.join("phase1_sample_identity.iamf.sha256"),
        format!("{digest}  phase1_sample_identity.iamf\n"),
    )
    .expect("the hash is writable");
    std::fs::write(dir.join("phase1_sample_identity.dump.txt"), &dump)
        .expect("the dump is writable");
    println!(
        "regenerated {} bytes, sha256 {digest}, {} dump lines — NOW READ THE DIFF",
        bytes.len(),
        dump.lines().count()
    );
}
