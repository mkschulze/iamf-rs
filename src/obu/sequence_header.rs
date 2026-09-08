//! The IA Sequence Header OBU (type 31) — DESC-01.
//!
//! Four bytes of magic and two profile bytes, and one rejection rule that only
//! the *decoder* enforces. `read` sits immediately before `write` (D-10).

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Error, ErrorKind, Finding, Location, Result};

/// `ia_code` — the four bytes `69 61 6d 66`, `"iamf"`.
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_types.h IAMF_CODE
pub const IA_CODE: u32 = 0x6961_6d66;

/// `IAMF_PROFILE_COUNT` — Simple, Base, Base-Enhanced.
// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_types.h IAMF_PROFILE_COUNT
pub const PROFILE_COUNT: u8 = 3;

/// The IA Sequence Header payload.
///
/// `ia_code` is stored rather than assumed: D-06 says the reader preserves what
/// the wire carried and `validate()` reports the mismatch by name. A foreign
/// file with a wrong magic stays reproducible, and the defect stays visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IaSequenceHeader {
    /// SHALL be [`IA_CODE`]. Preserved as read.
    pub ia_code: u32,
    /// `primary_profile` — the profile a decoder must support to play the
    /// sequence at all.
    pub primary_profile: u8,
    /// `additional_profile` — **must be >= `primary_profile`**; see
    /// [`write_ia_sequence_header`].
    pub additional_profile: u8,
    /// Payload bytes past the six this type understands (OBU-06 / D-05).
    pub trailing: Vec<u8>,
}

impl IaSequenceHeader {
    /// A header through the encoder path: `ia_code` is derived, never supplied.
    #[must_use]
    pub const fn new(primary_profile: u8, additional_profile: u8) -> Self {
        Self {
            ia_code: IA_CODE,
            primary_profile,
            additional_profile,
            trailing: Vec::new(),
        }
    }

    /// Every rule this OBU can break, reported at once and never implicitly
    /// (D-07, D-09).
    ///
    /// `additional_profile < primary_profile` is reported here **and** refused
    /// on write, because it is the one rule `libiamf` enforces and `iamf-tools`
    /// does not: a file carrying it passes CONF-06 and fails CONF-05.
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if self.ia_code != IA_CODE {
            findings.push(Finding {
                at: Location::Field("ia_code"),
                message: format!(
                    "ia_code is 0x{:08x}, expected 0x{IA_CODE:08x} (\"iamf\")",
                    self.ia_code
                ),
            });
        }
        if self.primary_profile >= PROFILE_COUNT {
            findings.push(Finding {
                at: Location::Field("primary_profile"),
                message: format!(
                    "primary_profile is {}, and libiamf's `_valid_profile` requires it below \
                     IAMF_PROFILE_COUNT ({PROFILE_COUNT})",
                    self.primary_profile
                ),
            });
        }
        if self.additional_profile < self.primary_profile {
            findings.push(Finding {
                at: Location::Field("additional_profile"),
                message: format!(
                    "additional_profile is {}, below primary_profile {}; libiamf rejects the \
                     whole sequence",
                    self.additional_profile, self.primary_profile
                ),
            });
        }
        findings
    }
}

// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c iamf_sequence_header_new
/// Read the IA Sequence Header payload, draining anything past it into
/// `trailing`.
pub fn read_ia_sequence_header(r: &mut BitCursor<'_>) -> Result<IaSequenceHeader> {
    let ia_code = u32::try_from(r.read_unsigned(32)?).unwrap_or(0);
    let primary_profile = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    let additional_profile = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
    let remaining = r.bytes_remaining();
    let trailing = r.read_uint8_span(remaining)?.to_vec();
    Ok(IaSequenceHeader {
        ia_code,
        primary_profile,
        additional_profile,
        trailing,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/ia_sequence_header.cc IASequenceHeaderObu::ValidateAndWriteObu
// NOTE: the profile-ordering rule below comes from libiamf, NOT from the
// function cited above — `iamf-tools` does not validate `additional_profile`
// at all. This is the reverse of the usual strictness asymmetry, and it means a
// file carrying the violation is written happily by the reference encoder and
// then rejected wholesale by the reference decoder:
//   ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c _valid_profile
//   `return primary < IAMF_PROFILE_COUNT && primary <= addional;`
/// Write the IA Sequence Header payload.
///
/// Refuses `additional_profile < primary_profile` before emitting a byte.
/// Emitting both equal is always safe.
pub fn write_ia_sequence_header(w: &mut BitWriter, v: &IaSequenceHeader) -> Result<()> {
    if v.additional_profile < v.primary_profile {
        return Err(Error::new(
            ErrorKind::AdditionalProfileBelowPrimary,
            Location::Field("additional_profile"),
        ));
    }
    w.write_unsigned(u64::from(v.ia_code), 32)?;
    w.write_unsigned(u64::from(v.primary_profile), 8)?;
    w.write_unsigned(u64::from(v.additional_profile), 8)?;
    w.write_bytes(&v.trailing)
}
