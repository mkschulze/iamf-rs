use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Cargo executes integration tests in the tool package. Relative fixture
/// arguments are intentionally resolved against the repository root instead.
pub fn fixture_dir(variable: &str) -> PathBuf {
    let path =
        PathBuf::from(std::env::var_os(variable).expect("explicit fixture path is required"));
    assert!(
        !path.as_os_str().is_empty(),
        "fixture path must not be empty"
    );
    if path.is_absolute() {
        path
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(path)
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn manifest(dir: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(dir.join("MANIFEST.md")).unwrap();
    let mut fields = BTreeMap::new();
    for (key, value) in text.lines().filter_map(|line| line.split_once(" = ")) {
        assert!(fields.insert(key.to_owned(), value.to_owned()).is_none());
    }
    fields
}

pub fn pcm_s16le(bytes: &[u8]) -> Vec<i32> {
    assert_eq!(bytes.len() % 2, 0);
    bytes
        .chunks_exact(2)
        .map(|pair| i32::from(i16::from_le_bytes(pair.try_into().unwrap())))
        .collect()
}
