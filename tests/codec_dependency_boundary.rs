//! CODEC-07 must remain true even on Windows, where the shell boundary gate is
//! not available. These canaries exercise the same root manifest/source rule
//! using only Rust's standard library.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

const FORBIDDEN: &[&str] = &[
    "claxon",
    "flacenc",
    "opus",
    "opusic-sys",
    "audiopus",
    "rubato",
];

#[test]
fn lexical_import_canaries_have_the_expected_polarity() {
    let mut failures = Vec::new();
    for (source, expected) in [
        ("use {opus};\n", Some("opus")),
        ("extern crate opus;\n", Some("opus")),
        ("fn harmless() {}\nuse opus::Encoder;\n", Some("opus")),
        ("pub(crate) use ::opus::Encoder;\n", Some("opus")),
        (
            "pub(in crate::private) use {std::fmt, opusic_sys};\n",
            Some("opusic-sys"),
        ),
        ("use{claxon, flacenc};\n", Some("claxon")),
        ("extern crate audiopus as codec;\n", Some("audiopus")),
        ("mod inner { use rubato::Resampler; }\n", Some("rubato")),
        ("use/* comment */opus::Encoder;\n", Some("opus")),
        ("const HELP: &str = r#\"; use opus::Encoder;\"#;\n", None),
        ("const HELP: &str = r##\"use opus::Encoder;\"##;\n", None),
        (
            "const HELP: &str = r##\"\"#; use opus::Encoder;\"##;\n",
            None,
        ),
        (
            "const HELP: &[u8] = br##\"; extern crate opus;\"##;\n",
            None,
        ),
        ("const HELP: &[u8] = b\"\\\"; use opus;\";\n", None),
        ("const HELP: &str = \"\\\"; use opus;\";\n", None),
        (
            "/* nested /* use opus; */ extern crate opus; */\n// use opus;\n",
            None,
        ),
        (
            "const QUOTE: char = '\"'; use opus::Encoder;\n",
            Some("opus"),
        ),
        (
            "const QUOTE: u8 = b'\"'; extern crate opus;\n",
            Some("opus"),
        ),
        ("const QUOTE: char = '\\''; use opus;\n", Some("opus")),
        (
            "const SYMBOL: char = '\\u{1f600}'; use opus;\n",
            Some("opus"),
        ),
        (
            "fn borrow<'a>(s: &'a str) { use opus::Encoder; }\n",
            Some("opus"),
        ),
        ("use opus_extra::Encoder; fn reuse() {}\n", None),
        (
            "const HELP: &str = r#\"use opus;\"#; use flacenc::Encoder;\n",
            Some("flacenc"),
        ),
    ] {
        let canary = TemporaryCanary::new("lexical-import");
        write_canary(canary.path(), "[dependencies]\n", source);
        let result = assert_src_has_no_codec_imports(&canary.path().join("src"));
        match (expected, result) {
            (None, Ok(())) => {}
            (Some(package), Err(error))
                if error.contains("src import") && error.contains(&format!("`{package}`")) => {}
            (expected, actual) => {
                failures.push(format!("{source:?}: expected {expected:?}, got {actual:?}"))
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn workspace_exclusion_canaries_are_limited_to_the_exclude_value() {
    let mut failures = Vec::new();
    for (manifest, accepted) in [
        (
            "[workspace]\nexclude = [\n    \"fuzz\",\n    \"tools/codec-fixtures\",\n]\n",
            true,
        ),
        (
            "[workspace]\n# exclude = [\"tools/codec-fixtures\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"fuzz\"]\nmembers = [\"tools/codec-fixtures\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\n \"fuzz\", # \"tools/codec-fixtures\"\n]\nmembers = [\"tools/codec-fixtures\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"fuzz\"]\n[package.metadata]\nexclude = [\"tools/codec-fixtures\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"name#with]bracket\", \"tools/codec-fixtures\"] # comment\n",
            true,
        ),
        (
            "[workspace] # comment\nexclude = ['tools/codec-fixtures']\n",
            true,
        ),
        (
            "[workspace]\nexclude = [\"fuzz\"] # \"tools/codec-fixtures\"\n",
            false,
        ),
        (
            "[workspace]\nexclude_more = [\"tools/codec-fixtures\"]\n",
            false,
        ),
    ] {
        let canary = TemporaryCanary::new("workspace-value");
        write_canary(canary.path(), manifest, "");
        let result = assert_root_workspace_excludes_codec_fixtures(canary.path());
        if result.is_ok() != accepted {
            failures.push(format!(
                "{manifest:?}: expected accepted={accepted}, got {result:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

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
    assert!(
        dependency_error.contains("[dependencies]"),
        "{dependency_error}"
    );
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
        ("grouped-branch", "use { std::fmt, opus::Encoder };\n"),
        ("grouped-name", "use { opus };\n"),
        (
            "multiline-group",
            "use {\n    std::fmt,\n    opus::Encoder,\n};\n",
        ),
        ("visible-import", "pub use opus::Encoder;\n"),
        ("hyphen-normalized", "use opusic_sys::Encoder;\n"),
        (
            "raw-string",
            "const TOKEN: &str = r#\"use opus::Encoder;\"#;\n",
        ),
        (
            "string-then-import",
            "const TOKEN: &str = \"/*\"; use opus::Encoder;\n",
        ),
        (
            "two-comments-then-import",
            "/* one */ /* two */ use opus;\n",
        ),
    ] {
        let canary = TemporaryCanary::new(kind);
        write_canary(
            canary.path(),
            "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\nthiserror = \"2\"\n",
            source,
        );
        let result = assert_manifest_and_src_boundary(canary.path());
        if matches!(kind, "block-comment" | "raw-string") {
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
    assert!(
        workspace_error.contains("[workspace] exclude"),
        "{workspace_error}"
    );

    let canary = TemporaryCanary::new("workspace-multiline");
    write_canary(
        canary.path(),
        "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\nexclude = [\n    \"fuzz\",\n    \"tools/codec-fixtures\",\n]\n",
        "pub const CODEC_ID_OPUS: u32 = 1;\n",
    );
    assert_root_workspace_excludes_codec_fixtures(canary.path())
        .expect("a multiline workspace exclusion must be accepted");

    let canary = TemporaryCanary::new("workspace-comment");
    write_canary(
        canary.path(),
        "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n# exclude = [\"tools/codec-fixtures\"]\n",
        "pub const CODEC_ID_OPUS: u32 = 1;\n",
    );
    assert_root_workspace_excludes_codec_fixtures(canary.path())
        .expect_err("a comment-only workspace exclusion must not count");

    let canary = TemporaryCanary::new("lockfile");
    write_canary(
        canary.path(),
        "[package]\nname = \"boundary-canary\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\nexclude = [\"tools/codec-fixtures\"]\n",
        "pub const CODEC_ID_OPUS: u32 = 1;\n",
    );
    fs::write(
        canary.path().join("Cargo.lock"),
        "[[package]]\nname = \"opus\"\n",
    )
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
    let manifest = strip_toml_comments(&manifest);
    let mut lines = manifest.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace = trimmed == "[workspace]";
            continue;
        }
        if in_workspace {
            if let Some((key, value)) = trimmed.split_once('=') {
                if key.trim() == "exclude" && value.trim_start().starts_with('[') {
                    // The parser returns at this array's closing bracket, even when
                    // later members or tables mention the fixture generator.
                    let array = value.trim_start()[1..]
                        .chars()
                        .chain(std::iter::once('\n'))
                        .chain(
                            lines
                                .by_ref()
                                .flat_map(|line| line.chars().chain(std::iter::once('\n'))),
                        );
                    if toml_array_contains_fixture(array) {
                        return Ok(());
                    }
                    break;
                }
            }
        }
    }
    Err("root [workspace] exclude must contain tools/codec-fixtures".to_owned())
}

fn strip_toml_comments(manifest: &str) -> String {
    let mut output = String::with_capacity(manifest.len());
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;
    for ch in manifest.chars() {
        if comment && ch != '\n' {
            output.push(' ');
            continue;
        }
        comment = false;
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if delimiter == '"' && ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
        } else if matches!(ch, '"' | '\'') {
            quote = Some(ch);
        } else if ch == '#' {
            comment = true;
        }
        output.push(if comment { ' ' } else { ch });
    }
    output
}

fn toml_array_contains_fixture(array: impl Iterator<Item = char>) -> bool {
    let mut quote = None;
    let mut escaped = false;
    let mut entry = String::new();
    let mut found = false;
    for ch in array {
        if let Some(delimiter) = quote {
            if escaped {
                // Escaped entries are not needed for this literal path. Keep
                // the escape so a different path cannot accidentally match.
                entry.push(ch);
                escaped = false;
            } else if delimiter == '"' && ch == '\\' {
                entry.push(ch);
                escaped = true;
            } else if ch == delimiter {
                found |= entry == "tools/codec-fixtures";
                quote = None;
            } else {
                entry.push(ch);
            }
        } else if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            entry.clear();
        } else if ch == ']' {
            return found;
        }
    }
    false
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
                    return Err(format!(
                        "root Cargo.lock must not contain codec package `{codec}`"
                    ));
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
                return Err(format!(
                    "{section} must not contain codec dependency `{codec}`"
                ));
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
        if let Some(codec) = imported_codec(&strip_rust_comments_and_strings(&source)) {
            return Err(format!(
                "src import must not name codec `{codec}` ({})",
                path.display()
            ));
        }
    }
    Ok(())
}

fn imported_codec(source: &str) -> Option<&'static str> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !is_identifier_byte(bytes[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && is_identifier_byte(bytes[index]) {
            index += 1;
        }
        let mut rest = &source[index..];
        match &source[start..index] {
            // Visibility tokens before `use` do not affect the import body.
            "use" => {}
            "extern" => {
                rest = rest.trim_start();
                let Some(body) = rest.strip_prefix("crate") else {
                    continue;
                };
                if body
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| is_identifier_byte(*byte))
                {
                    continue;
                }
                rest = body;
            }
            _ => continue,
        }
        let Some((statement, _)) = rest.split_once(';') else {
            continue;
        };
        for codec in FORBIDDEN {
            let identifier = codec.replace('-', "_");
            for branch in statement.split(['{', ',', '}']) {
                let branch = branch.trim_start();
                let branch = branch.strip_prefix("::").unwrap_or(branch).trim_start();
                let end = branch
                    .as_bytes()
                    .iter()
                    .take_while(|byte| is_identifier_byte(**byte))
                    .count();
                if branch[..end] == identifier {
                    return Some(codec);
                }
            }
        }
        index = source.len() - rest.len() + statement.len() + 1;
    }
    None
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

fn strip_rust_comments_and_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = bytes.to_vec();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        if bytes[index..].starts_with(b"/*") {
            let mut depth = 1;
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
        } else if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if let Some((content, hashes)) = raw_string_start(bytes, index) {
            index = content;
            while index < bytes.len() {
                if bytes[index] == b'"'
                    && bytes
                        .get(index + 1..index + 1 + hashes)
                        .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
                {
                    index += hashes + 1;
                    break;
                }
                index += 1;
            }
        } else if bytes[index] == b'"' || bytes[index..].starts_with(b"b\"") {
            if bytes[index] == b'b' {
                index += 1;
            }
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
        } else if let Some(end) = character_literal_end(source, index) {
            index = end;
        } else {
            index += 1;
            continue;
        }
        // Preserve token separation and line positions; never retain literal
        // content or merge identifiers separated by a comment.
        for byte in &mut output[start..index] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    String::from_utf8(output).expect("sanitizing complete literals preserves UTF-8")
}

fn raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    if index > 0 && is_identifier_byte(bytes[index - 1]) {
        return None;
    }
    let mut cursor = index;
    if bytes[cursor] == b'b' {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let hashes = bytes[cursor..]
        .iter()
        .take_while(|byte| **byte == b'#')
        .count();
    cursor += hashes;
    (bytes.get(cursor) == Some(&b'"')).then_some((cursor + 1, hashes))
}

fn character_literal_end(source: &str, index: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut cursor = index;
    if bytes[cursor] == b'b' {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'\'') {
        return None;
    }
    cursor += 1;
    if bytes.get(cursor) == Some(&b'\\') {
        cursor += 1;
        match bytes.get(cursor)? {
            b'u' if bytes.get(cursor + 1) == Some(&b'{') => {
                cursor += 2;
                cursor += bytes[cursor..].iter().position(|byte| *byte == b'}')? + 1;
            }
            b'x' => cursor += 3,
            _ => cursor += 1,
        }
    } else {
        let ch = source.get(cursor..)?.chars().next()?;
        if matches!(ch, '\n' | '\r' | '\'') {
            return None;
        }
        cursor += ch.len_utf8();
    }
    // A lifetime such as `'a` lacks the immediate closing quote.
    (bytes.get(cursor) == Some(&b'\'')).then_some(cursor + 1)
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
        let path = base.join(format!(
            "iamf-codec-boundary-{kind}-{}-{nonce}",
            process::id()
        ));
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
        assert_eq!(
            self.path.parent(),
            Some(base.as_path()),
            "refusing unsafe canary cleanup"
        );
        assert!(valid_name, "refusing unsafe canary cleanup");
        assert!(
            !fs::symlink_metadata(&self.path)
                .expect("inspect canary")
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(&self.path).expect("remove isolated canary directory");
    }
}
