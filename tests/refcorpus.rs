//! CONF-10's always-on offline layer: the OBU boundary walk over the whole
//! vendored reference corpus.
//!
//! **No reference binary, no container, no network.** This file runs on all
//! four CI targets in the ordinary PR gate with `IAMF_REF_DECODER` unset, which
//! is exactly what keeps that gate fast and cross-platform while the expensive
//! `libiamf` decode-and-compare stays a Linux-only job.
//!
//! What it proves is small and load-bearing: for every `.iamf` file
//! `iamf-tools` produced, our structural walk lands its **final boundary
//! exactly on the file length**. A walk that stops one OBU short, or one byte
//! long, is indistinguishable from a correct one on any single OBU — it only
//! shows up at the end of the file, which is why the equality is asserted there.
//!
//! The corpus includes `negative/tones_256samp_5p1_pcm.iamf`, which is invalid
//! by construction (`num_samples_per_frame = 0`). That is a *payload-level*
//! defect, not a framing one, so it walks cleanly like everything else and is
//! included here deliberately. It must never become a golden.

use std::fs;
use std::path::{Path, PathBuf};

use iamf::error::ErrorKind;
use iamf::obu::find_obu_boundaries;

const CORPUS: &str = "tests/fixtures/reference";
const MANIFEST: &str = "tests/fixtures/MANIFEST.md";

/// Every `.iamf` under the corpus, in **sorted path order** so a failure names
/// the same file on every target and in every run.
fn vendored_iamf_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(Path::new(CORPUS), &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "iamf") {
            out.push(path);
        }
    }
}

/// The `.iamf` count `MANIFEST.md` declares, so a fixture silently dropped from
/// the corpus fails this test rather than quietly shrinking it.
fn manifest_iamf_count() -> usize {
    let text = fs::read_to_string(MANIFEST).expect("the fixture manifest is readable");
    for line in text.lines() {
        if let Some(tail) = line.split("files vendored:").nth(1) {
            let digits: String = tail
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(n) = digits.parse::<usize>() {
                return n;
            }
        }
    }
    panic!("MANIFEST.md no longer declares an `.iamf` files vendored: N count");
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// OBU-08's property, over every file we vendored: the last boundary is the
/// file length, exactly.
#[test]
fn every_vendored_iamf_walks_to_its_own_length() {
    let files = vendored_iamf_files();
    println!("refcorpus: walked {} vendored .iamf files", files.len());

    assert!(
        !files.is_empty(),
        "the corpus walk found no .iamf files under {CORPUS} — it has stopped \
         measuring anything and would pass on an empty checkout"
    );
    assert_eq!(
        files.len(),
        manifest_iamf_count(),
        "MANIFEST.md declares a different number of vendored .iamf files than \
         are on disk — a fixture was added or dropped without recording it"
    );

    for path in &files {
        let bytes = fs::read(path).expect("a vendored fixture is readable");
        let boundaries = find_obu_boundaries(&bytes)
            .unwrap_or_else(|e| panic!("{} failed to walk: {e}", path.display()));

        assert_eq!(
            boundaries.last().copied(),
            Some(bytes.len()),
            "{}: the final boundary must land exactly on the file length",
            path.display()
        );
        assert_eq!(
            boundaries.first().copied(),
            Some(0),
            "{}: the first boundary is the start of the file",
            path.display()
        );
    }
}

/// The hand-decoded file, asserted against the numbers `01-RESEARCH.md` derived
/// by reading it byte by byte.
///
/// The descriptor prologue is **120 bytes**, which corrects `PROJECT.md`'s 118.
#[test]
fn test_000003_walks_67_obus_ending_exactly_on_32567() {
    let bytes = fs::read("tests/fixtures/reference/test_000003.iamf").expect("vendored fixture");
    assert_eq!(bytes.len(), 32567, "0x7F37");

    let boundaries = find_obu_boundaries(&bytes).expect("the file walks cleanly");

    assert_eq!(boundaries.last().copied(), Some(32567));
    assert_eq!(
        boundaries.len(),
        68,
        "67 OBU start offsets plus the final end offset"
    );
    assert_eq!(
        boundaries.len().saturating_sub(1),
        67,
        "67 OBUs: 4 descriptors and 63 audio frames"
    );

    // IA Sequence Header, Codec Config, Audio Element, Mix Presentation.
    assert_eq!(boundaries.get(..4), Some([0_usize, 8, 26, 40].as_slice()));
    // The first Audio Frame — and therefore the descriptor prologue length.
    assert_eq!(boundaries.get(4).copied(), Some(120));
    // The trimmed frame hand-decoded at 0x7D32.
    assert!(boundaries.contains(&32050));
}

// ---------------------------------------------------------------------------
// Negative cases, all built by mutating a vendored file **in memory**. A
// corrupted fixture committed to git is a fixture someone eventually treats as
// a golden.
// ---------------------------------------------------------------------------

/// T-01-20. `libiamf@v1.1.0`'s splitter returns 0 when the last OBU runs past
/// the buffer, and callers read that as end-of-stream — so an oversized final
/// size manifests there as a *shorter file*, not a decode failure. Ours must be
/// a typed error, because a boundary list that merely stops short is
/// indistinguishable from a correct walk of a legitimately shorter file.
#[test]
fn a_file_truncated_mid_obu_is_an_error_not_a_short_boundary_list() {
    let mut bytes = fs::read("tests/fixtures/reference/test_000003.iamf").expect("vendored fixture");
    bytes.truncate(bytes.len().saturating_sub(1));

    let err = find_obu_boundaries(&bytes)
        .expect_err("the final OBU is one byte short")
        .kind()
        .clone();

    assert_eq!(err, ErrorKind::TruncatedObu);
}

/// An `obu_size` overwritten with a value that pushes the OBU past the end of
/// the buffer: a typed error, not a panic and not a silent stop.
#[test]
fn an_obu_size_past_the_end_of_the_buffer_is_an_error_not_a_panic() {
    let bytes = fs::read("tests/fixtures/reference/test_000003.iamf").expect("vendored fixture");
    // The IA Sequence Header alone, with its obu_size raised from 6 to 127.
    let mut head = bytes.get(..8).expect("at least eight bytes").to_vec();
    if let Some(size) = head.get_mut(1) {
        *size = 0x7f;
    }

    let err = find_obu_boundaries(&head)
        .expect_err("129 bytes claimed inside an 8-byte buffer")
        .kind()
        .clone();

    assert_eq!(err, ErrorKind::TruncatedObu);
}

/// `kEntireObuSizeMaxTwoMegabytes` is `1 << 21`. `80 80 80 01` is exactly that
/// value as a uleb128, and it must be refused on its size alone — before any
/// bounds check against the buffer, so the error names the real defect.
#[test]
fn an_obu_size_above_the_two_megabyte_ceiling_is_refused_on_its_size_alone() {
    let bytes = [0xf8_u8, 0x80, 0x80, 0x80, 0x01];

    let err = find_obu_boundaries(&bytes)
        .expect_err("2 MiB is the ceiling, not a legal size")
        .kind()
        .clone();

    assert_eq!(err, ErrorKind::ObuTooLarge);
}

/// An empty input is a zero-OBU sequence, and its only boundary is `0` — which
/// is also `bytes.len()`, so OBU-08's property holds degenerately rather than
/// being a special case a caller has to know about.
#[test]
fn an_empty_input_yields_a_single_boundary_at_zero() {
    assert_eq!(find_obu_boundaries(&[]).expect("trivially valid"), vec![0]);
}
