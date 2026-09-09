//! Field-level expectations for every committed foreign IAMF fixture.

#[path = "support/reference_expectations.rs"]
mod reference_expectations;

use std::path::{Path, PathBuf};

use reference_expectations::{NEGATIVE_EXPECTATION, POSITIVE_EXPECTATIONS};

fn collect_iamf(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir).expect("reference fixture directory");
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_iamf(&path, base, out);
        } else if path.extension().is_some_and(|extension| extension == "iamf") {
            out.push(
                path.strip_prefix(base)
                    .expect("fixture below reference root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[test]
fn expectation_inventory_is_a_bijection() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference");
    let mut actual = Vec::new();
    collect_iamf(&root, &root, &mut actual);
    actual.sort();

    let mut expected: Vec<String> = POSITIVE_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.path.to_owned())
        .collect();
    expected.push(NEGATIVE_EXPECTATION.path.to_owned());
    let original_len = expected.len();
    expected.sort();
    expected.dedup();

    assert_eq!(POSITIVE_EXPECTATIONS.len(), 38, "positive ledger count");
    assert_eq!(original_len, 39, "38 positives plus one negative");
    assert_eq!(expected.len(), original_len, "duplicate expectation path");
    assert_eq!(actual, expected, "missing or unexpected reference fixture");
    assert!(
        POSITIVE_EXPECTATIONS
            .iter()
            .all(|expectation| !expectation.path.is_empty()),
        "positive paths must not be empty"
    );
    assert!(!NEGATIVE_EXPECTATION.path.is_empty(), "negative path must not be empty");
    assert_eq!(NEGATIVE_EXPECTATION.zero_frame_size, 0);
    assert_eq!(NEGATIVE_EXPECTATION.finding_field, "num_samples_per_frame");
    assert!(!NEGATIVE_EXPECTATION.finding_message.is_empty());
}
