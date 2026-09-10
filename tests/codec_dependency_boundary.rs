//! CODEC-07 must remain true even on Windows, where the shell boundary gate is
//! not available. These canaries exercise the same root manifest/source rule
//! using only Rust's standard library.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

const FORBIDDEN: &[&str] = &["claxon", "flacenc", "opus", "opusic-sys", "audiopus", "rubato"];

#[test]
fn codec_dependencies_and_src_imports_remain_outside_the_root_crate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_boundary(root).expect("the committed root tree must preserve the CODEC-07 boundary");

    let canary = TemporaryCanary::new("dependency");
    write_canary(
        canary.path(),
        "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nopus = \"0.4\"\n",
        "pub const CODEC_ID_OPUS: u32 = 1;\n",
    );
    let dependency_error = assert_boundary(canary.path())
        .expect_err("a root [dependencies] opus entry must be rejected")
        .to_string();
    assert!(dependency_error.contains("[dependencies]"), "{dependency_error}");
    assert!(dependency_error.contains("opus"), "{dependency_error}");

    let canary = TemporaryCanary::new("import");
    write_canary(
        canary.path(),
        "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nthiserror = \"2\"\n",
        "use opus::Encoder;\n",
    );
    let import_error = assert_boundary(canary.path())
        .expect_err("a codec import from src must be rejected")
        .to_string();
    assert!(import_error.contains("src import"), "{import_error}");
    assert!(import_error.contains("opus"), "{import_error}");
}

fn assert_boundary(root: &Path) -> Result<(), String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("cannot read root Cargo.toml: {error}"))?;
    assert_manifest_has_no_codec_dependencies(&manifest)?;
    assert_src_has_no_codec_imports(&root.join("src"))
}

fn assert_manifest_has_no_codec_dependencies(manifest: &str) -> Result<(), String> {
    let mut section = "";
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed;
            continue;
        }
        if !matches!(section, "[dependencies]" | "[dev-dependencies]") {
            continue;
        }
        let dependency = trimmed.split_once('=').map(|(name, _)| name.trim());
        for codec in FORBIDDEN {
            if dependency == Some(*codec) || trimmed.contains(&format!("package = \"{codec}\"")) {
                return Err(format!("{section} must not contain codec dependency `{codec}`"));
            }
        }
    }
    Ok(())
}

fn assert_src_has_no_codec_imports(src: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(src).map_err(|error| format!("cannot read src: {error}"))? {
        let entry = entry.map_err(|error| format!("cannot read src entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            assert_src_has_no_codec_imports(&path)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        for line in source.lines() {
            let code = line.split_once("//").map_or(line, |(code, _)| code).trim_start();
            for codec in FORBIDDEN {
                let use_import = code.starts_with("use ")
                    && (code[4..].starts_with(&format!("{codec}::"))
                        || code[4..].starts_with(&format!("::{codec}::")));
                let extern_crate = code.starts_with(&format!("extern crate {codec}"));
                if use_import || extern_crate {
                    return Err(format!("src import must not name codec `{codec}` ({})", path.display()));
                }
            }
        }
    }
    Ok(())
}

fn write_canary(root: &Path, manifest: &str, source: &str) {
    fs::create_dir_all(root.join("src")).expect("create canary src directory");
    fs::write(root.join("Cargo.toml"), manifest).expect("write canary manifest");
    fs::write(root.join("src/lib.rs"), source).expect("write canary source");
}

struct TemporaryCanary {
    path: PathBuf,
}

impl TemporaryCanary {
    fn new(kind: &str) -> Self {
        let base = env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = base.join(format!("iamf-codec-boundary-{kind}-{}-{nonce}", process::id()));
        fs::create_dir(&path).expect("create isolated canary directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryCanary {
    fn drop(&mut self) {
        let base = env::temp_dir();
        let valid_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("iamf-codec-boundary-"));
        assert_eq!(self.path.parent(), Some(base.as_path()), "refusing unsafe canary cleanup");
        assert!(valid_name, "refusing unsafe canary cleanup");
        assert!(!fs::symlink_metadata(&self.path).expect("inspect canary").file_type().is_symlink());
        fs::remove_dir_all(&self.path).expect("remove isolated canary directory");
    }
}
