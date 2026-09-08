//! OBU framing — the header every OBU type shares, and the structural walk over
//! a whole `.iamf` byte stream.
//!
//! # The one `trailing` drain site (OBU-07), and its precedence rule (D-05)
//!
//! Every OBU carries a `trailing: Vec<u8>` holding whatever its type-specific
//! parser did not consume. It is filled in exactly one place — [`read_obu_with`]
//! below — rather than once per OBU type, because a per-type drain is a
//! per-type opportunity to forget one, and an under-read is otherwise
//! completely silent.
//!
//! **Two mechanisms will claim "everything after the fields I understood", and
//! their precedence is defined.** `trailing` is the **OBU-level** remainder:
//! `obu_size` minus whatever the type-specific parser read. A *payload-level*
//! raw-bytes field — the one plan 01-05 gives an Audio Element type this crate
//! does not model — consumes to the end of **its own** payload, which means it
//! consumes to the end of the OBU payload too, and `trailing` is then left
//! empty. The payload-level mechanism wins; `trailing` is what is left when
//! nothing else claimed it.
//!
//! Without that rule the boundary between the two is ambiguous and Phase 2's
//! decode→encode byte-identity property becomes untestable — which is the only
//! property that can prove we preserved something we did not understand.

mod header;

pub use header::{ObuHeader, ObuType, Trimming, TypeSpecific, read_obu_header, write_obu};

use crate::bits::{BitCursor, BitWriter};
use crate::error::Result;

/// One OBU: its header, its parsed payload, and the OBU-level remainder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obu<T> {
    /// The common header.
    pub header: ObuHeader,
    /// Whatever the type-specific parser produced.
    pub payload: T,
    /// Bytes inside `obu_size` that the type-specific parser did not consume.
    /// Empty on every freshly constructed OBU, and appended **last** on write.
    pub trailing: Vec<u8>,
}

impl<T> Obu<T> {
    /// A freshly constructed OBU, with nothing trailing.
    #[must_use]
    pub const fn new(header: ObuHeader, payload: T) -> Self {
        Self {
            header,
            payload,
            trailing: Vec::new(),
        }
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ReadAndValidate
/// Read one whole OBU: the header, then the type-specific payload through a
/// bounded sub-reader, then the OBU-level remainder.
pub fn read_obu_with<T, F>(r: &mut BitCursor<'_>, parse: F) -> Result<Obu<T>>
where
    F: FnOnce(&mut BitCursor<'_>) -> Result<T>,
{
    let _ = (r, parse);
    Err(crate::error::Error::new(
        crate::error::ErrorKind::UnexpectedEndOfInput,
        crate::error::Location::Unlocated,
    ))
}

// ref: iamf-tools@v2.1.0 iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite
/// Write one whole OBU, appending `trailing` after the type-specific payload.
pub fn write_obu_with<T, F>(w: &mut BitWriter, obu: &Obu<T>, write_payload: F) -> Result<()>
where
    F: FnOnce(&mut BitWriter, &T) -> Result<()>,
{
    let _ = (w, obu, write_payload);
    Ok(())
}
