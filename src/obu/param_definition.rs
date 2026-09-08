//! `ParamDefinition` — the block of fields every parameter definition in the
//! format starts with, shared by the Audio Element (demixing, recon gain) and
//! the Mix Presentation (element and output mix gain).
//!
//! It lives in its own module because it is genuinely shared: putting it in
//! either owner would make the other reach across an OBU boundary for it, and
//! the two owners land in different commits.
//!
//! # `param_definition_mode = 1` is the zero-Parameter-Block path
//!
//! When the mode bit is set there are **no** `duration`,
//! `constant_subblock_duration` or `num_subblocks` fields: the next field
//! follows immediately. That is how `test_000003` carries two mandatory Mix
//! Gain param definitions while containing zero Parameter Block OBUs (DESC-06).
//! The duration fields are therefore an `Option` gated by the mode, not fields
//! with a "not applicable" value.

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Error, ErrorKind, Location, Result};

/// The `duration` / `constant_subblock_duration` / subblock block, present on
/// the wire **only** when `param_definition_mode == 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationFields {
    /// `duration`.
    pub duration: u32,
    /// `constant_subblock_duration`. When non-zero the subblock durations are
    /// implied and `subblock_durations` is empty.
    pub constant_subblock_duration: u32,
    /// `subblock_durations`, present only when
    /// `constant_subblock_duration == 0`. `num_subblocks` is derived from this
    /// length, never stored (Pattern 2).
    pub subblock_durations: Vec<u32>,
}

/// The fields every parameter definition begins with.
// ref: iamf-tools@v2.1.0 iamf/obu/param_definitions.h ParamDefinition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDefinition {
    /// `parameter_id`.
    pub parameter_id: u32,
    /// `parameter_rate`, in ticks per second.
    pub parameter_rate: u32,
    /// The duration block, present exactly when `param_definition_mode` is
    /// `false`.
    ///
    /// `param_definition_mode` itself is **derived** from this `Option`
    /// ([`ParamDefinition::param_definition_mode`]) rather than stored beside
    /// it (Pattern 2). A stored bit plus the fields it gates is precisely the
    /// disagreement that produces a file the reference mis-frames: mode 1 with
    /// duration fields present would write a bit saying "no duration fields"
    /// and then three of them.
    pub duration_fields: Option<DurationFields>,
}

impl ParamDefinition {
    /// A mode-1 definition: no duration fields, so no Parameter Block OBU is
    /// implied by its presence.
    #[must_use]
    pub const fn mode_1(parameter_id: u32, parameter_rate: u32) -> Self {
        Self {
            parameter_id,
            parameter_rate,
            duration_fields: None,
        }
    }

    /// `param_definition_mode`, derived from the data it gates.
    ///
    /// `true` (1) means the duration fields are absent and the parameter is
    /// carried entirely by Parameter Block OBUs — of which a file may contain
    /// none (DESC-06).
    #[must_use]
    pub const fn param_definition_mode(&self) -> bool {
        self.duration_fields.is_none()
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/param_definitions.cc ParamDefinition::ReadAndValidate
/// Read the shared parameter-definition fields.
pub fn read_param_definition(r: &mut BitCursor<'_>) -> Result<ParamDefinition> {
    let parameter_id = r.read_uleb128()?;
    let parameter_rate = r.read_uleb128()?;
    let param_definition_mode = r.read_bool()?;
    let reserved = u8::try_from(r.read_unsigned(7)?).unwrap_or(0);
    let _ = reserved; // Preserved as zero on write; reported by no rule here.

    let duration_fields = if param_definition_mode {
        None
    } else {
        let duration = r.read_uleb128()?;
        let constant_subblock_duration = r.read_uleb128()?;
        let subblock_durations = if constant_subblock_duration == 0 {
            let start = r.byte_position();
            let num_subblocks = r.read_uleb128()?;
            let num_subblocks = usize::try_from(num_subblocks)
                .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
            // Bounds-check before reserving (T-01-24): the smallest possible
            // subblock duration is one byte, so more subblocks than bytes left
            // is unreadable by construction.
            if num_subblocks > r.bytes_remaining() {
                return Err(Error::new(
                    ErrorKind::UnexpectedEndOfInput,
                    Location::InputOffset(start),
                ));
            }
            let mut durations = Vec::with_capacity(num_subblocks);
            for _ in 0..num_subblocks {
                durations.push(r.read_uleb128()?);
            }
            durations
        } else {
            Vec::new()
        };
        Some(DurationFields {
            duration,
            constant_subblock_duration,
            subblock_durations,
        })
    };

    Ok(ParamDefinition {
        parameter_id,
        parameter_rate,
        duration_fields,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/param_definitions.cc ParamDefinition::ValidateAndWrite
/// Write the shared parameter-definition fields.
///
/// `param_definition_mode` is emitted from `duration_fields.is_none()` rather
/// than from the stored bit, so the two can never disagree (Pattern 2).
pub fn write_param_definition(w: &mut BitWriter, v: &ParamDefinition) -> Result<()> {
    w.write_uleb128_minimal(v.parameter_id)?;
    w.write_uleb128_minimal(v.parameter_rate)?;
    w.write_bool(v.param_definition_mode())?;
    w.write_unsigned(0, 7)?;
    if let Some(fields) = v.duration_fields.as_ref() {
        w.write_uleb128_minimal(fields.duration)?;
        w.write_uleb128_minimal(fields.constant_subblock_duration)?;
        if fields.constant_subblock_duration == 0 {
            let count = u32::try_from(fields.subblock_durations.len()).map_err(|_| {
                Error::new(ErrorKind::ObuTooLarge, Location::Field("num_subblocks"))
            })?;
            w.write_uleb128_minimal(count)?;
            for duration in &fields.subblock_durations {
                w.write_uleb128_minimal(*duration)?;
            }
        }
    }
    Ok(())
}
