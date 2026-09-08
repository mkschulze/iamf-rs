//! D-23 — the `// ref:` citation discipline, enforced mechanically.
//!
//! STACK.md's convention is that every `read`/`write` function names the
//! reference file and symbol it was derived from. A convention that is only
//! written down rots silently, in exactly the way research found `libiamf`'s
//! LPCM endianness comment had rotted — the comment says one thing and the code
//! one line below it does the opposite. The citation is what tells a reviewer
//! *which* code to go read; without it the review has nowhere to start.
//!
//! This test walks `src/`, finds every `fn read_*` and `fn write_*`, and fails
//! naming the file, line and function when one has no `// ref:` line above it.
//! Roughly thirty lines, no dependency, runs offline on all four targets, and
//! it polices every later plan in this phase and every later phase.
//!
//! The citation is looked for in the plain-comment block immediately above the
//! function, skipping blank lines, attributes and doc comments and stopping at
//! the first line of code — so a citation on the previous function is never
//! credited to this one. The house layout is:
//!
//! ```text
//! // ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h Class::Method
//! // NOTE: optional rider, e.g. that the reference's own comment is wrong
//! //       here and the code is authoritative. May wrap over several lines.
//! /// What this function does.
//! #[must_use]
//! pub fn read_something(..)
//! ```

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn every_read_and_write_function_in_src_cites_its_reference() {
    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0_usize;

    for file in rust_files_under(Path::new("src")) {
        let text = fs::read_to_string(&file).expect("src file is readable");
        let lines: Vec<&str> = text.lines().collect();

        for (index, line) in lines.iter().enumerate() {
            if in_test_module(&lines, index) {
                continue;
            }
            let Some(name) = function_name_needing_a_citation(line) else {
                continue;
            };
            checked = checked.saturating_add(1);
            if !has_citation_above(&lines, index) {
                let line_number = index.saturating_add(1);
                missing.push(format!(
                    "{}:{} — `fn {}` has no `// ref:` line above it",
                    file.display(),
                    line_number,
                    name
                ));
            }
        }
    }

    assert!(
        checked > 0,
        "the walker found no `fn read_*`/`fn write_*` at all under src/ — \
         it has stopped measuring anything and would pass on an empty crate"
    );
    assert!(
        missing.is_empty(),
        "{} of {checked} read/write functions are missing a `// ref:` citation \
         (D-23):\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

/// Every `.rs` file under `root`, recursively.
fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rust_files_under(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// The name of the function defined on `line`, if it is one this rule governs.
///
/// A definition, not a call: the line must contain `fn ` followed immediately
/// by `read_` or `write_`, and must not itself be a comment.
fn function_name_needing_a_citation(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return None;
    }
    let after_fn = trimmed.split("fn ").nth(1)?;
    if !(after_fn.starts_with("read_") || after_fn.starts_with("write_")) {
        return None;
    }
    let name: String = after_fn
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

/// Whether the plain-comment block immediately above the function contains a
/// `// ref:` line.
///
/// Scans backwards over blank lines, attributes and doc comments, then over the
/// consecutive `//` lines that form the citation block, and stops at the first
/// line of actual code — so a citation belonging to the *previous* function is
/// never credited to this one.
///
/// The block may hold more than the citation itself. The convention puts a
/// `// NOTE:` rider *after* the `// ref:` line, wrapped over as many lines as
/// it needs, for the case where a reference comment misleads: `libiamf`'s LPCM
/// endianness comment says `0x01 - big endian` while the line below it does
/// `param->big_endian = !ior_8(r)`. The note is part of the citation, not a
/// replacement for it, which is why the whole block is searched.
fn has_citation_above(lines: &[&str], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 {
        cursor = cursor.saturating_sub(1);
        let Some(candidate) = lines.get(cursor).map(|l| l.trim()) else {
            return false;
        };
        if candidate.is_empty() || candidate.starts_with('#') || candidate.starts_with("///") {
            continue;
        }
        if candidate.starts_with("// ref:") {
            return true;
        }
        if candidate.starts_with("//") {
            continue;
        }
        // Code. The comment block above this function is exhausted.
        return false;
    }
    false
}

/// Whether the line at `index` sits inside a `#[cfg(test)]` module.
///
/// Counts braces from the `#[cfg(test)]` attribute forward: while the depth is
/// above the level the attribute appeared at, we are still inside that module.
/// Test helpers are exempt from D-23 — a citation belongs on the code being
/// audited against the reference, not on the scaffolding that exercises it.
fn in_test_module(lines: &[&str], index: usize) -> bool {
    let mut depth = 0_i32;
    let mut test_mod_depth: Option<i32> = None;

    for (n, line) in lines.iter().enumerate() {
        if n == index {
            return test_mod_depth.is_some();
        }
        if test_mod_depth.is_none() && line.trim() == "#[cfg(test)]" {
            test_mod_depth = Some(depth);
        }
        let opens = i32::try_from(line.matches('{').count()).unwrap_or(0);
        let closes = i32::try_from(line.matches('}').count()).unwrap_or(0);
        depth = depth.saturating_add(opens).saturating_sub(closes);
        if let Some(entry_depth) = test_mod_depth {
            if depth <= entry_depth && closes > 0 {
                test_mod_depth = None;
            }
        }
    }
    false
}

/// The walker itself must be able to fail. A test that can only pass is a test
/// that has stopped testing, and this one polices every later plan.
#[test]
fn the_walker_reports_a_missing_citation_by_name() {
    let sample = vec![
        "// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h R::ReadX",
        "// NOTE: the reference's comment here is inverted; the code below the",
        "// comment is authoritative. This rider wraps over two lines.",
        "/// Cited.",
        "pub fn read_cited(&mut self) -> Result<u8> {",
        "}",
        "",
        "/// Not cited.",
        "pub fn write_uncited(&mut self) -> Result<()> {",
        "}",
    ];
    let line = |n: usize| sample.get(n).copied().unwrap_or_default();

    assert_eq!(
        function_name_needing_a_citation(line(4)).as_deref(),
        Some("read_cited")
    );
    assert!(
        has_citation_above(&sample, 4),
        "a citation followed by a NOTE rider still counts"
    );

    assert_eq!(
        function_name_needing_a_citation(line(8)).as_deref(),
        Some("write_uncited")
    );
    assert!(
        !has_citation_above(&sample, 8),
        "the uncited one is caught, and it is named in the failure"
    );
}

/// Calls are not definitions, and comments are not code.
#[test]
fn the_walker_ignores_call_sites_and_comments() {
    assert!(function_name_needing_a_citation("        self.read_unsigned(8)?;").is_none());
    assert!(function_name_needing_a_citation("    // fn read_pretend()").is_none());
    assert!(function_name_needing_a_citation("    /// See `fn write_thing`.").is_none());
    assert!(function_name_needing_a_citation("    fn helper(&self) {}").is_none());
}
