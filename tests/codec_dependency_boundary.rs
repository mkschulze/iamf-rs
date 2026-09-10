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
    let dependency_error = assert_manifest_and_src_boundary(canary.path())
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
    let import_error = assert_manifest_and_src_boundary(canary.path())
        .expect_err("a codec import from src must be rejected")
        .to_string();
    assert!(import_error.contains("src import"), "{import_error}");
    assert!(import_error.contains("opus"), "{import_error}");

    for (kind, source) in [
        ("simple-import", "use opus;\n"),
        ("aliased-import", "use opus as codec;\n"),
        ("grouped-import", "use { opus::Encoder };\n"),
        ("inline-block-import", "/* comment */ use opus;\n"),
        ("block-comment", "/* use opus::Encoder; */\n"),
    ] {
        let canary = TemporaryCanary::new(kind);
        write_canary(
            canary.path(),
            "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nthiserror = \"2\"\n",
            source,
        );
        let result = assert_manifest_and_src_boundary(canary.path());
        if kind == "block-comment" {
            result.expect("a block comment mentioning a codec must remain harmless");
        } else {
            let error = result
                .expect_err("a simple or aliased codec import must be rejected")
                .to_string();
            assert!(error.contains("src import"), "{error}");
            assert!(error.contains("opus"), "{error}");
        }
    }

    let canary = TemporaryCanary::new("workspace-exclude");
    write_canary(
        canary.path(),
        "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\nmembers = [\".\"]\n",
        "pub const CODEC_ID_OPUS: u32 = 1;\n",
    );
    let workspace_error = assert_root_workspace_excludes_codec_fixtures(canary.path())
        .expect_err("the root workspace must exclude the codec fixture generator")
        .to_string();
    assert!(workspace_error.contains("[workspace] exclude"), "{workspace_error}");

    let canary = TemporaryCanary::new("lockfile");
    write_canary(
        canary.path(),
        "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\nexclude = [\"tools/codec-fixtures\"]\n",
        "pub const CODEC_ID_OPUS: u32 = 1;\n",
    );
    fs::write(canary.path().join("Cargo.lock"), "[[package]]\nname = \"opus\"\n")
        .expect("write codec lockfile canary");
    let lock_error = assert_lock_has_no_codec_packages(canary.path())
        .expect_err("the root lockfile must not resolve opus")
        .to_string();
    assert!(lock_error.contains("Cargo.lock"), "{lock_error}");
    assert!(lock_error.contains("opus"), "{lock_error}");
}

fn assert_boundary(root: &Path) -> Result<(), String> {
    assert_root_workspace_excludes_codec_fixtures(root)?;
    assert_lock_has_no_codec_packages(root)?;
    assert_manifest_and_src_boundary(root)
}

fn assert_manifest_and_src_boundary(root: &Path) -> Result<(), String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("cannot read root Cargo.toml: {error}"))?;
    assert_manifest_has_no_codec_dependencies(&manifest)?;
    assert_src_has_no_codec_imports(&root.join("src"))
}

fn assert_root_workspace_excludes_codec_fixtures(root: &Path) -> Result<(), String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("cannot read root Cargo.toml: {error}"))?;
    let mut in_workspace = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace = trimmed == "[workspace]";
            continue;
        }
        if in_workspace && trimmed.starts_with("exclude") && trimmed.contains("\"tools/codec-fixtures\"") {
            return Ok(());
        }
    }
    Err("root [workspace] exclude must contain tools/codec-fixtures".to_owned())
}

fn assert_lock_has_no_codec_packages(root: &Path) -> Result<(), String> {
    let lock = fs::read_to_string(root.join("Cargo.lock"))
        .map_err(|error| format!("cannot read root Cargo.lock: {error}"))?;
    let mut in_package = false;
    for line in lock.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            in_package = true;
            continue;
        }
        if in_package {
            for codec in FORBIDDEN {
                if trimmed == format!("name = \"{codec}\"") {
                    return Err(format!("root Cargo.lock must not contain codec package `{codec}`"));
                }
            }
        }
    }
    Ok(())
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
        for line in strip_comments(&source).lines() {
            let code = line.trim_start();
            for codec in FORBIDDEN {
                let use_import = import_starts_with_codec(code, "use", codec);
                let extern_crate = import_starts_with_codec(code, "extern crate", codec);
                if use_import || extern_crate {
                    return Err(format!("src import must not name codec `{codec}` ({})", path.display()));
                }
            }
        }
    }
    Ok(())
}

fn import_starts_with_codec(code: &str, keyword: &str, codec: &str) -> bool {
    let Some(rest) = code.strip_prefix(keyword).and_then(|rest| rest.strip_prefix(char::is_whitespace)) else {
        return false;
    };
    let rest = rest.trim_start().strip_prefix("::").unwrap_or(rest.trim_start());
    let rest = rest.strip_prefix('{').map(str::trim_start).unwrap_or(rest);
    let Some(after_codec) = rest.strip_prefix(codec) else {
        return false;
    };
    matches!(after_codec.chars().next(), None | Some(':' | ';'))
        || after_codec.chars().next().is_some_and(char::is_whitespace)
}

fn strip_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut block_depth = 0_u32;
    while index < bytes.len() {
        let pair = bytes.get(index..index.saturating_add(2));
        if block_depth > 0 {
            if pair == Some(b"/*") {
                block_depth += 1;
                index += 2;
            } else if pair == Some(b"*/") {
                block_depth -= 1;
                index += 2;
            } else {
                if bytes[index] == b'\n' {
                    output.push('\n');
                }
                index += 1;
            }
        } else if pair == Some(b"/*") {
            block_depth = 1;
            index += 2;
        } else if pair == Some(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else {
            output.push(bytes[index] as char);
            index += 1;
        }
    }
    output
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
