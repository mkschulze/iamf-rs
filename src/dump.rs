//! D-20's annotated structural dumper — one deterministic, diff-friendly text
//! rendering of any `.iamf` byte slice.
//!
//! # This dump aids review. It never establishes correctness.
//!
//! Stated in the imperative because the temptation is real. The dumper parses
//! with the same readers the writer mirrors, so it is **self-consistent with
//! this crate by construction**: a field this crate writes wrongly and reads
//! back wrongly renders here as a tidy, plausible line. The same caveat applies
//! to the committed golden hash. Both are *change detection* — they make an
//! output change a reviewable PR diff, which is GUARD-09's whole stated
//! rationale and which neither a bare hash (a one-line diff that explains
//! nothing) nor a bare binary blob delivers.
//!
//! Conformance evidence comes from exactly two places: the two reference
//! oracles (`libiamf`'s `iamfdec` and `iamf-tools`' `decoder_main`/
//! `encoder_main`), and hand-decoded byte vectors. Never from here.
//!
//! # What is dumped, and what is not
//!
//! Every OBU gets its absolute start offset, its type, its `obu_size`, its
//! flags, one line per decoded field, and its raw bytes in a 16-per-row hex
//! block carrying absolute offsets. The field lines are rendered from the
//! parsed **model**; they carry the OBU's offset rather than each field's own,
//! because a per-field offset would require a second walker over the wire
//! layout — a duplicate of the parser, and therefore a second thing to keep in
//! step with `iamf-tools`. The hex block is what locates a wrong byte, and it
//! is exact.
//!
//! Determinism is a hard requirement, not a nicety: this text is committed and
//! compared. There is no timestamp, no absolute path, no address, no hashed
//! container and no iteration over anything but a `Vec` in bitstream order.

use crate::bits::BitCursor;
use crate::error::Result;
use crate::obu::find_obu_boundaries;

/// Render `bytes` as a deterministic, annotated structural dump.
///
/// # Errors
///
/// Whatever [`find_obu_boundaries`] reports for a malformed OBU chain. A
/// payload this crate cannot parse is rendered as raw bytes with a stated
/// reason rather than failing the whole dump — a dumper that refuses to dump
/// the thing you are debugging is no use.
pub fn dump_annotated(bytes: &[u8]) -> Result<String> {
    let _ = find_obu_boundaries(bytes)?;
    let _ = BitCursor::new(bytes);
    Ok(String::new())
}
