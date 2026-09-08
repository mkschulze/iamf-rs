//! OBU framing — the header every OBU type shares, and the structural walk over
//! a whole `.iamf` byte stream.

mod header;

pub use header::{ObuHeader, ObuType, Trimming, TypeSpecific, read_obu_header, write_obu};
