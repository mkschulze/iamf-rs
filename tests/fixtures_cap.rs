//! D-14's enforcement half: the vendored-fixture size cap and the count.
//!
//! `tests/fixtures/MANIFEST.md` states a per-file size cap and a count of
//! vendored `.iamf` files. Both are enforced here rather than trusted, because
//! Parallax consumes this crate as a **path dependency** — every byte under
//! `tests/fixtures/` lands in every Parallax developer's clone, and a fixture
//! committed by mistake cannot be un-committed without a history rewrite.
//!
//! This runs offline on all four byte-identity targets with no reference binary
//! present (CONF-10). It reads only files already in the repository.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// GUARD-04's carve-out for `unwrap`/`expect`/`panic` applies inside `#[test]`
// functions only, so every helper below is total: it returns `Option`/`Result`
// and the tests do the asserting. That is the right shape anyway — a helper
// that panics reports the failure at the wrong place.

fn manifest_text() -> Result<String, String> {
    let path = repo_root().join("tests/fixtures/MANIFEST.md");
    std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "cannot read {} ({e}) — D-14 requires a manifest alongside the fixtures",
            path.display()
        )
    })
}

/// The first `N` or `N_NNN` following the first mention of "size cap" in
/// `MANIFEST.md`. The manifest is the single source of the cap; duplicating the
/// number here would let the two drift.
fn stated_cap(manifest: &str) -> Option<u64> {
    let lower = manifest.to_ascii_lowercase();
    let idx = lower.find("size cap")?;
    let tail = lower.get(idx..).unwrap_or_default();
    let digits: String = tail
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '_')
        .filter(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The count `MANIFEST.md` claims, from the line that states it explicitly.
fn stated_iamf_count(manifest: &str) -> Option<usize> {
    let line = manifest
        .lines()
        .find(|l| l.to_ascii_lowercase().contains("`.iamf` files vendored:"))?;
    let digits: String = line
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Every regular file under `dir`, recursively.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

fn vendored_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(&repo_root().join("tests/fixtures/reference"), &mut out);
    out
}

#[test]
fn no_vendored_fixture_exceeds_the_stated_cap() {
    let manifest = manifest_text().expect("MANIFEST.md must be readable");
    let cap = stated_cap(&manifest).expect("MANIFEST.md must state a numeric per-file size cap");
    assert!(cap > 0, "the stated size cap must be positive");

    let mut over: Vec<String> = Vec::new();
    for path in vendored_files() {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > cap {
            over.push(format!("{} ({size} bytes)", path.display()));
        }
    }
    assert!(
        over.is_empty(),
        "these vendored fixtures exceed the {cap}-byte per-file cap stated in \
         tests/fixtures/MANIFEST.md: {over:?}. Either fetch the file on demand \
         instead (the manifest gives the pinned `git show` command) or raise the \
         cap deliberately in MANIFEST.md — and note that raising it puts the \
         difference into every Parallax developer's clone (D-14)."
    );
}

#[test]
fn the_manifest_accounts_for_every_vendored_iamf_file() {
    let manifest = manifest_text().expect("MANIFEST.md must be readable");
    let claimed =
        stated_iamf_count(&manifest).expect("MANIFEST.md must state how many `.iamf` are vendored");
    let on_disk: Vec<PathBuf> = vendored_files()
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "iamf"))
        .collect();
    assert_eq!(
        on_disk.len(),
        claimed,
        "tests/fixtures/MANIFEST.md claims {claimed} vendored `.iamf` files but \
         {} are on disk. A fixture nobody wrote down is a fixture nobody can \
         justify keeping.",
        on_disk.len()
    );
    for path in &on_disk {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        assert!(
            manifest.contains(&name),
            "{name} is vendored but is not named in tests/fixtures/MANIFEST.md"
        );
    }
}

/// The invalid fixture must stay flagged as invalid. If someone moves it out of
/// `negative/` or drops its warning from the manifest, the "clean decode, wrong
/// result" trap it exists to demonstrate becomes a golden by accident.
#[test]
fn the_invalid_fixture_is_documented_as_negative_only() {
    let manifest = manifest_text().expect("MANIFEST.md must be readable");
    let negative = repo_root().join("tests/fixtures/reference/negative/tones_256samp_5p1_pcm.iamf");
    assert!(
        negative.is_file(),
        "the negative fixture must live under tests/fixtures/reference/negative/"
    );
    assert!(
        manifest.contains("tones_256samp_5p1_pcm"),
        "MANIFEST.md must name the invalid fixture"
    );
    assert!(
        manifest.contains("num_samples_per_frame = 0"),
        "MANIFEST.md must record WHY tones_256samp_5p1_pcm.iamf is invalid \
         (num_samples_per_frame = 0), not merely that it is"
    );
}
