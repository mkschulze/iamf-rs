//! D-13's runtime half: the reference-drift assertion.
//!
//! GUARD-06 pins `iamf-tools` and `libiamf` by SHA in `REFERENCES.md`.
//! `tools/build-reference.sh` records the SHA it *actually* checked out into
//! `.reference-manifest.json`. This test asserts the two agree, and it is meant
//! to run **before any conformance clause** — a conformance suite that is green
//! against the wrong reference is worse than no suite at all, because it reports
//! confidence it has not earned.
//!
//! CONF-10 holds regardless: with `IAMF_REF_DECODER` unset this test prints a
//! one-line reason and returns without failing, so `cargo test` stays green
//! offline on all four byte-identity targets with no reference binary present.
//!
//! No TOML or JSON dependency is added for this. Both files are parsed by
//! scanning for 40-hex-character tokens on lines that name the project, which
//! is enough for two SHAs and keeps the shipping graph at `iamf` + `thiserror`.
//! This file lives in `tests/`, so nothing here reaches the library.

use std::path::{Path, PathBuf};

/// Absolute path to the repository root, derived from `CARGO_MANIFEST_DIR`.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The first 40-hex-character token on the first line that contains `needle`
/// (case-insensitively) and also contains such a token.
///
/// `REFERENCES.md` puts each project and its commit on one table row, and
/// `.reference-manifest.json` puts each key and its value on one line, so a
/// line-scoped scan is unambiguous for both.
fn sha_on_line_naming(haystack: &str, needle: &str) -> Option<String> {
    let needle_lc = needle.to_ascii_lowercase();
    for line in haystack.lines() {
        if !line.to_ascii_lowercase().contains(&needle_lc) {
            continue;
        }
        if let Some(sha) = first_sha40(line) {
            return Some(sha);
        }
    }
    None
}

/// The first maximal run of exactly 40 lowercase hex characters in `line`,
/// bounded on both sides by a non-hex character (so a 41-character run is not
/// silently truncated to a false match).
fn first_sha40(line: &str) -> Option<String> {
    // Splitting on "not a hex digit" yields exactly the maximal hex runs, so a
    // 41-character run is one 41-character token and is rejected — it can never
    // be truncated into a false 40-character match.
    line.split(|c: char| !c.is_ascii_hexdigit())
        .find(|run| {
            run.len() == 40
                && run
                    .chars()
                    .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
        })
        .map(str::to_owned)
}

/// Reads a file, or returns a message naming the path when it cannot be read.
fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// The manifest assertion. Fails with a message naming *both* values when the
/// manifest and `REFERENCES.md` disagree.
///
/// Skips — prints a reason and returns — when `IAMF_REF_DECODER` is unset,
/// which is the offline, no-reference-binary case CONF-10 requires to be green.
#[test]
fn reference_manifest_matches_pinned_shas() {
    let Ok(decoder) = std::env::var("IAMF_REF_DECODER") else {
        println!(
            "SKIP reference_manifest_matches_pinned_shas: IAMF_REF_DECODER is unset. \
             Run `bash tools/build-reference.sh` and export the path it prints to enable \
             the reference-gated checks. This is the expected offline state (CONF-10)."
        );
        return;
    };

    let root = repo_root();
    let manifest_path = root.join(".reference-manifest.json");
    let references_path = root.join("REFERENCES.md");

    assert!(
        Path::new(&decoder).is_file(),
        "IAMF_REF_DECODER is set to {decoder}, which is not a file. \
         Re-run `bash tools/build-reference.sh` and export the path it prints."
    );

    let manifest = match read(&manifest_path) {
        Ok(s) => s,
        Err(e) => panic!(
            "{e}\nIAMF_REF_DECODER is set, so the reference build is expected to have \
             stamped .reference-manifest.json. Re-run `bash tools/build-reference.sh`."
        ),
    };
    let references = read(&references_path).expect("REFERENCES.md must exist at the repo root");

    // Two projects, two SHAs, asserted independently so the failure message
    // says which pin drifted.
    for (project, manifest_key) in [("libiamf", "libiamf_sha"), ("iamf-tools", "iamf_tools_sha")] {
        let pinned = sha_on_line_naming(&references, project).unwrap_or_else(|| {
            panic!("REFERENCES.md carries no 40-hex commit on a line naming `{project}`")
        });
        let built = sha_on_line_naming(&manifest, manifest_key).unwrap_or_else(|| {
            panic!(".reference-manifest.json carries no 40-hex value for `{manifest_key}`")
        });
        assert_eq!(
            built, pinned,
            "manifest disagrees with REFERENCES.md for `{project}`: \
             .reference-manifest.json says {built}, REFERENCES.md pins {pinned}. \
             The reference in this environment is NOT the pinned one, so no conformance \
             verdict from it is meaningful. Re-run `bash tools/build-reference.sh`, or \
             update the pin deliberately and re-run the whole conformance suite in the \
             same commit (GUARD-06)."
        );
    }

    // The manifest must also record what was built, not merely which source was
    // checked out — a Phase 3 build with real opus/FLAC libraries produces a
    // different iamfdec from the same SHA (D-13).
    for key in ["iamfdec_sha256", "host_triple", "dep_codecs_disabled"] {
        assert!(
            manifest.contains(key),
            ".reference-manifest.json is missing the `{key}` field; \
             it was written by an older tools/build-reference.sh. Re-run it."
        );
    }
}

/// The SHA scanner must not accept a 39- or 41-character hex run, or the
/// drift assertion could pass against a truncated value.
#[test]
fn sha_scanner_requires_exactly_forty_hex_characters() {
    let forty = "f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63";
    assert_eq!(forty.len(), 40);

    assert_eq!(
        first_sha40(&format!("| `libiamf` | `{forty}` |")).as_deref(),
        Some(forty)
    );
    let thirty_nine: String = forty.chars().take(39).collect();
    assert_eq!(first_sha40(&thirty_nine), None);
    assert_eq!(first_sha40(&format!("{forty}a")), None);
    assert_eq!(first_sha40("no commit on this line"), None);
}

/// The line scanner must attribute a SHA to the project named on its own line,
/// not to whichever SHA appears first in the file.
#[test]
fn line_scanner_attributes_each_sha_to_its_own_project() {
    let doc = "\
| Project | Commit |
| iamf-tools | `848c6ff4968ff8cc6f728259892ab4f90cb83256` |
| libiamf | `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63` |
";
    assert_eq!(
        sha_on_line_naming(doc, "iamf-tools").as_deref(),
        Some("848c6ff4968ff8cc6f728259892ab4f90cb83256")
    );
    assert_eq!(
        sha_on_line_naming(doc, "libiamf").as_deref(),
        Some("f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63")
    );
    assert_eq!(sha_on_line_naming(doc, "gpac"), None);
}

/// The real `REFERENCES.md` must parse — this runs offline, with no reference
/// binary, so a malformed pin table is caught on every target on every PR
/// rather than only when someone has a reference build.
#[test]
fn references_md_carries_both_pins() {
    let references = read(&repo_root().join("REFERENCES.md"))
        .expect("REFERENCES.md must exist at the repo root");
    assert!(
        sha_on_line_naming(&references, "libiamf").is_some(),
        "REFERENCES.md carries no 40-hex commit on a line naming `libiamf`"
    );
    assert!(
        sha_on_line_naming(&references, "iamf-tools").is_some(),
        "REFERENCES.md carries no 40-hex commit on a line naming `iamf-tools`"
    );
}
