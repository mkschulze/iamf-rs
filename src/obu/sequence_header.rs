//! The IA Sequence Header OBU (type 31) — DESC-01. RED skeleton: types and
//! signatures only; the wire behaviour lands in the GREEN commit.

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Finding, Result};

/// `ia_code` — the four bytes `69 61 6d 66`, `"iamf"`.
pub const IA_CODE: u32 = 0x6961_6d66;

/// `IAMF_PROFILE_COUNT` — Simple, Base, Base-Enhanced.
pub const PROFILE_COUNT: u8 = 3;

/// The IA Sequence Header payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IaSequenceHeader {
    /// SHALL be [`IA_CODE`]. Preserved as read.
    pub ia_code: u32,
    /// `primary_profile`.
    pub primary_profile: u8,
    /// `additional_profile` — must be >= `primary_profile`.
    pub additional_profile: u8,
    /// Payload bytes past the six this type understands.
    pub trailing: Vec<u8>,
}

impl IaSequenceHeader {
    /// A header through the encoder path.
    #[must_use]
    pub const fn new(primary_profile: u8, additional_profile: u8) -> Self {
        Self {
            ia_code: IA_CODE,
            primary_profile,
            additional_profile,
            trailing: Vec::new(),
        }
    }

    /// Every rule this OBU can break.
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        Vec::new() // STUB(GREEN)
    }
}

// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c iamf_sequence_header_new
/// Read the IA Sequence Header payload.
pub fn read_ia_sequence_header(r: &mut BitCursor<'_>) -> Result<IaSequenceHeader> {
    let _ = r; // STUB(GREEN)
    Ok(placeholder())
}

fn placeholder() -> IaSequenceHeader {
    IaSequenceHeader {
        ia_code: 0,
        primary_profile: 0,
        additional_profile: 0,
        trailing: Vec::new(),
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/ia_sequence_header.cc IASequenceHeaderObu::ValidateAndWriteObu
/// Write the IA Sequence Header payload.
pub fn write_ia_sequence_header(w: &mut BitWriter, v: &IaSequenceHeader) -> Result<()> {
    let _ = (w, v); // STUB(GREEN)
    Ok(())
}
