//! CODEC-07 must remain true even on Windows, where the shell boundary gate is
//! not available. These canaries exercise the same root manifest/source rule
//! using portable Rust, syn's AST, and decoded TOML values.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use syn::{ext::IdentExt, visit::Visit};

const FORBIDDEN: &[&str] = &[
    "claxon",
    "flacenc",
    "opus",
    "opusic-sys",
    "audiopus",
    "rubato",
];

#[test]
fn parser_valid_imports_are_distinguished_from_literal_content() {
    let mut failures = Vec::new();
    for (source, forbidden) in [
        ("extern crate opus;", true),
        ("use r#opus::Encoder;", true),
        (
            "fn f() -> impl Sized + use<> { use opus::Encoder; () }",
            true,
        ),
        (
            "const HELP: &std::ffi::CStr = cr#\"\"\"#; use opus::Encoder;",
            true,
        ),
        ("use\u{0085}opus::Encoder;", true),
        (
            "const HELP: &std::ffi::CStr = cr#\"\"; use opus;\"#;",
            false,
        ),
        ("const LABEL: &str = r#\"use opus::Encoder;\"#;", false),
        ("use std::{opus, nested::{opusic_sys}};", false),
        (
            "mod local { fn f() { extern crate r#opus as codec; } }",
            true,
        ),
        ("use {std::fmt, {opus as codec}};", true),
    ] {
        syn::parse_file(source).expect("the import canary must be valid Rust syntax");
        let canary = TemporaryCanary::new("parser-import");
        write_complete_canary(canary.path(), "", source);
        let result = assert_boundary(canary.path());
        match (forbidden, result) {
            (false, Ok(())) => {}
            (true, Err(error)) if error.contains("src import") && error.contains("`opus`") => {}
            (expected, actual) => {
                failures.push(format!("{source:?}: forbidden={expected}, got {actual:?}"))
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn decoded_manifest_dependency_tables_define_the_boundary() {
    let mut failures = Vec::new();
    for (manifest, forbidden) in [
        (
            "[dependencies] # legal table comment\n\"opus\" = \"0.4\"\n",
            true,
        ),
        (
            "[target.'cfg(unix)'.dev-dependencies]\nrenamed = { package = 'opus', version = \"0.4\" }\n",
            true,
        ),
        (
            "[dependencies.renamed]\npackage = 'opus'\nversion = '0.4'\n",
            true,
        ),
        ("[build-dependencies]\nopus = '0.4'\n", true),
        (
            "[target.'cfg(windows)'.build-dependencies.renamed]\npackage = \"op\\u0075s\"\n",
            true,
        ),
        ("[workspace.dependencies]\nopus = '0.4'\n", true),
        (
            "[dependencies]\n# package = \"opus\"\nthiserror = '2' # opus = '0.4'\n",
            false,
        ),
        (
            "[package.metadata]\nnote = '''\n[dependencies]\nopus = '0.4'\n'''\n",
            false,
        ),
    ] {
        let canary = TemporaryCanary::new("parser-dependency");
        write_complete_canary(canary.path(), manifest, "");
        let text = fs::read_to_string(canary.path().join("Cargo.toml")).unwrap();
        text.parse::<toml::Value>()
            .expect("the manifest canary must be valid TOML");
        let result = assert_boundary(canary.path());
        match (forbidden, result) {
            (false, Ok(())) => {}
            (true, Err(error)) if error.contains("dependencies") && error.contains("`opus`") => {}
            (expected, actual) => failures.push(format!(
                "{manifest:?}: forbidden={expected}, got {actual:?}"
            )),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn decoded_lock_package_names_define_the_boundary() {
    let mut failures = Vec::new();
    for (lock, forbidden) in [
        ("[[package]] # legal comment\n'name' = 'opus'\n", true),
        ("[[\"package\"]]\nname = \"op\\u0075s\"\n", true),
        ("[[package]]\nname = 'iamf'\n# name = \"opus\"\n", false),
        (
            "[[package]]\nname = 'iamf'\n[metadata]\nname = \"opus\"\n",
            false,
        ),
    ] {
        lock.parse::<toml::Value>()
            .expect("the lock canary must be valid TOML");
        let canary = TemporaryCanary::new("parser-lock");
        write_complete_canary(canary.path(), "", "");
        fs::write(canary.path().join("Cargo.lock"), lock).unwrap();
        let result = assert_boundary(canary.path());
        match (forbidden, result) {
            (false, Ok(())) => {}
            (true, Err(error)) if error.contains("Cargo.lock") && error.contains("`opus`") => {}
            (expected, actual) => {
                failures.push(format!("{lock:?}: forbidden={expected}, got {actual:?}"))
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn malformed_inputs_fail_with_their_path() {
    for (file, content, diagnostic) in [
        ("Cargo.toml", "[workspace", "cannot parse"),
        ("Cargo.lock", "[[package]", "cannot parse"),
        ("src/lib.rs", "fn broken( {", "cannot parse Rust source"),
    ] {
        let canary = TemporaryCanary::new("malformed");
        write_complete_canary(canary.path(), "", "");
        let path = canary.path().join(file);
        fs::write(&path, content).unwrap();
        let error = assert_boundary(canary.path()).expect_err("malformed input must fail closed");
        assert!(
            error.contains(diagnostic) && error.contains(&path.display().to_string()),
            "{error}"
        );
    }
}

fn write_complete_canary(root: &Path, manifest: &str, source: &str) {
    write_canary(
        root,
        &format!("[workspace]\nexclude = [\"tools/codec-fixtures\"]\n{manifest}"),
        source,
    );
}

#[test]
fn import_canaries_have_the_expected_polarity() {
    let mut failures = Vec::new();
    for (source, expected) in [
        ("use r#opus::Encoder;\n", Some("opus")),
        ("extern crate r#opus;\n", Some("opus")),
        ("use {std::fmt, r#opusic_sys};\n", Some("opusic-sys")),
        ("extern crate r#opusic_sys as codec;\n", Some("opusic-sys")),
        ("fn opus() {} fn r#use() { opus(); }\n", None),
        (
            "fn main() { let r#use = { use opus::Encoder; }; }\n",
            Some("opus"),
        ),
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
        syn::parse_file(source).expect("the import canary must be valid Rust syntax");
        write_complete_canary(canary.path(), "[dependencies]\n", source);
        let result = assert_boundary(canary.path());
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
            "[\"workspace\"]\n\"exclude\" = [\"tools/codec-fixtures\"]\n",
            true,
        ),
        (
            "[workspace]\nexclude = [\"tools\\u002fcodec-fixtures\"]\n",
            true,
        ),
        (
            "[package.metadata]\nnote = \"\"\"\n[workspace]\nexclude = [\"tools/codec-fixtures\"]\n\"\"\"\n[workspace]\nexclude = [\"fuzz\"]\n",
            false,
        ),
        (
            "[workspace]\nmetadata.note = '''\nexclude = [\"tools/codec-fixtures\"]\n'''\nexclude = [\"fuzz\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"\"\"prefix\"\"tools/codec-fixtures\"\"suffix\"\"\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = ['''prefix''tools/codec-fixtures''suffix''']\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"\"\"prefix\n\"tools/codec-fixtures\"\nsuffix\"\"\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"\"\"tools/codec-fixtures\"\"\"]\n",
            true,
        ),
        (
            "[workspace]\nexclude = [\"\"\"\ntools/codec-fixtures\"\"\"]\n",
            true,
        ),
        (
            "[workspace]\nexclude = ['''\ntools/codec-fixtures''']\n",
            true,
        ),
        (
            "[workspace]\nexclude = [\"\"\"tools/codec-fixtures\"\"\"\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"\"\"tools/codec-fixtures\"\"\"\"\"]\n",
            false,
        ),
        (
            "[workspace]\nexclude = [\"\"\"prefix\n# ] \"\"suffix\"\"\", \"tools/codec-fixtures\"]\n",
            true,
        ),
        (
            "[workspace]\nexclude = [\"\"\"prefix\"\"tools/codec-fixtures\"\"suffix\"\"\"]\nmembers = [\"tools/codec-fixtures\"]\n",
            false,
        ),
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
        manifest
            .parse::<toml::Value>()
            .expect("the workspace canary must be valid TOML");
        let canary = TemporaryCanary::new("workspace-value");
        write_canary(canary.path(), manifest, "");
        let result = assert_boundary(canary.path());
        if result.is_ok() != accepted {
            failures.push(format!(
                "{manifest:?}: expected accepted={accepted}, got {result:?}"
            ));
        }
        if let Err(error) = result {
            assert!(
                error.contains("[workspace] exclude") && error.contains("tools/codec-fixtures"),
                "{error}"
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn codec_dependencies_and_src_imports_remain_outside_the_root_crate() {
    let root = boundary_root().expect("the CODEC-07 boundary root must be a directory");
    assert_boundary(&root).expect("the root tree must preserve the CODEC-07 boundary");
}

fn boundary_root() -> Result<PathBuf, String> {
    let root = env::var_os("CODEC_BOUNDARY_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    if !root.is_dir() {
        return Err(format!(
            "boundary root is not a directory: {}",
            root.display()
        ));
    }
    Ok(root)
}

fn assert_boundary(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("Cargo.toml");
    let manifest = read_toml(&manifest_path)?;
    assert_root_workspace_excludes_codec_fixtures(&manifest, &manifest_path)?;
    assert_manifest_has_no_codec_dependencies(&manifest, &mut Vec::new(), &manifest_path)?;
    assert_lock_has_no_codec_packages(root)?;
    assert_src_has_no_codec_imports(&root.join("src"))
}

fn read_toml(path: &Path) -> Result<toml::Value, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    contents
        .parse()
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn assert_root_workspace_excludes_codec_fixtures(
    manifest: &toml::Value,
    path: &Path,
) -> Result<(), String> {
    let excluded = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("exclude"))
        .and_then(toml::Value::as_array)
        .is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry.as_str() == Some("tools/codec-fixtures"))
        });
    if !excluded {
        return Err(format!(
            "root [workspace] exclude must contain tools/codec-fixtures ({})",
            path.display()
        ));
    }
    Ok(())
}

fn assert_manifest_has_no_codec_dependencies(
    value: &toml::Value,
    key_path: &mut Vec<String>,
    manifest_path: &Path,
) -> Result<(), String> {
    match value {
        toml::Value::Table(table) => {
            if key_path.last().is_some_and(|key| {
                matches!(
                    key.as_str(),
                    "dependencies" | "dev-dependencies" | "build-dependencies"
                )
            }) {
                for (name, dependency) in table {
                    let package = dependency
                        .as_table()
                        .and_then(|table| table.get("package"))
                        .and_then(toml::Value::as_str);
                    if let Some(codec) = FORBIDDEN
                        .iter()
                        .find(|codec| name == **codec || package == Some(**codec))
                    {
                        return Err(format!(
                            "[{}] must not contain codec dependency `{codec}` ({})",
                            key_path.join("."),
                            manifest_path.display()
                        ));
                    }
                }
            }
            for (key, child) in table {
                key_path.push(key.clone());
                assert_manifest_has_no_codec_dependencies(child, key_path, manifest_path)?;
                key_path.pop();
            }
        }
        toml::Value::Array(entries) => {
            for entry in entries {
                assert_manifest_has_no_codec_dependencies(entry, key_path, manifest_path)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn assert_lock_has_no_codec_packages(root: &Path) -> Result<(), String> {
    let path = root.join("Cargo.lock");
    let lock = read_toml(&path)?;
    if let Some(packages) = lock.get("package").and_then(toml::Value::as_array) {
        for package in packages {
            if let Some(name) = package.get("name").and_then(toml::Value::as_str) {
                if FORBIDDEN.contains(&name) {
                    return Err(format!(
                        "root Cargo.lock must not contain codec package `{name}` ({})",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn assert_src_has_no_codec_imports(src: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Ok(());
    }
    for entry in
        fs::read_dir(src).map_err(|error| format!("cannot read {}: {error}", src.display()))?
    {
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
        let file = syn::parse_file(&source)
            .map_err(|error| format!("cannot parse Rust source {}: {error}", path.display()))?;
        let mut imports = CodecImports::default();
        imports.visit_file(&file);
        if let Some(codec) = imports.forbidden {
            return Err(format!(
                "src import must not name codec `{codec}` ({})",
                path.display()
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct CodecImports {
    forbidden: Option<&'static str>,
}

impl<'ast> Visit<'ast> for CodecImports {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if self.forbidden.is_none() {
            self.forbidden = imported_codec(&item.tree);
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if self.forbidden.is_none() {
            self.forbidden = codec_ident(&item.ident);
        }
        syn::visit::visit_item_extern_crate(self, item);
    }
}

fn imported_codec(tree: &syn::UseTree) -> Option<&'static str> {
    // Only each branch's leading segment identifies an external package.
    // In `use std::{opus, opusic_sys}`, both names belong to `std`.
    match tree {
        syn::UseTree::Path(path) => codec_ident(&path.ident),
        syn::UseTree::Name(name) => codec_ident(&name.ident),
        syn::UseTree::Rename(rename) => codec_ident(&rename.ident),
        syn::UseTree::Group(group) => group.items.iter().find_map(imported_codec),
        syn::UseTree::Glob(_) => None,
    }
}

fn codec_ident(ident: &syn::Ident) -> Option<&'static str> {
    let package = ident.unraw().to_string().replace('_', "-");
    FORBIDDEN.iter().copied().find(|codec| *codec == package)
}

fn write_canary(root: &Path, manifest: &str, source: &str) {
    fs::create_dir_all(root.join("src")).expect("create canary src directory");
    fs::write(root.join("Cargo.toml"), manifest).expect("write canary manifest");
    fs::write(root.join("Cargo.lock"), "version = 4\n").expect("write canary lockfile");
    fs::write(root.join("src/lib.rs"), source).expect("write canary source");
}

struct TemporaryCanary {
    base: PathBuf,
    path: PathBuf,
}

impl TemporaryCanary {
    fn new(kind: &str) -> Self {
        let base = env::temp_dir()
            .canonicalize()
            .expect("resolve temporary base");
        assert!(base.is_dir(), "temporary base must be a directory");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = base.join(format!(
            "iamf-codec-boundary-{kind}-{}-{nonce}",
            process::id()
        ));
        fs::create_dir(&path).expect("create isolated canary directory");
        Self { base, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryCanary {
    fn drop(&mut self) {
        let valid_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("iamf-codec-boundary-"));
        assert_eq!(
            self.path.parent(),
            Some(self.base.as_path()),
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
