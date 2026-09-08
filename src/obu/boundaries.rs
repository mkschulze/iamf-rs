//! OBU-08 — the structural walk over a whole `.iamf` byte stream.
//!
//! Written **before the first encoder test**, deliberately: it is the cheapest
//! possible structural check on our own output later, and today it has the
//! whole vendored reference corpus to prove itself against with no reference
//! binary present at all (CONF-10, `tests/refcorpus.rs`).

use crate::bits::BitCursor;
use crate::error::{Error, ErrorKind, Location, Result};

/// Every OBU start offset in `bytes`, plus the final end offset.
///
/// The returned vector's **last element is `bytes.len()` for a well-formed
/// sequence**, which makes OBU-08's property a single equality check rather
/// than a length comparison the caller has to derive.
pub fn find_obu_boundaries(bytes: &[u8]) -> Result<Vec<usize>> {
    let _ = bytes;
    Ok(Vec::new())
}

/// Unused in the skeleton; the walk uses it in GREEN.
fn _unused(_: &BitCursor<'_>) -> Option<Error> {
    Some(Error::new(
        ErrorKind::TruncatedObu,
        Location::InputOffset(0),
    ))
}
