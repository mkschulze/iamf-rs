//! The Parameter Block OBU (type 3) — **the OBU whose parsing is
//! context-dependent**, and the reason TIME-05 exists.
//!
//! Read lives immediately before write (D-10).
//!
//! # The context argument is the point
//!
//! A Parameter Block cannot be parsed from its own bytes. Whether it carries
//! `duration`, `constant_subblock_duration` and `num_subblocks` — and whether
//! each subblock carries its own `subblock_duration` — is decided by the
//! `param_definition_mode` of a [`ParamDefinition`] that lives in a
//! **descriptor** OBU somewhere earlier in the file. Which *shape* the
//! per-subblock parameter data takes is decided by that definition's
//! `param_definition_type`, which is not on the wire in the block either.
//!
//! C++ cannot express that dependency through a virtual read path, so
//! `iamf-tools` routes around it: `ParameterBlockObu::PeekParameterId` reads
//! the id, seeks back, and a static `CreateMode0`/`CreateMode1` factory is
//! then chosen by the caller. Rust can make the dependency a **parameter**,
//! which deletes the whole "called the wrong factory" class of bug — the
//! compiler will not let you parse a Parameter Block without saying which
//! definition governs it.
//!
//! So: [`read_parameter_block`] takes a [`ParamDefinitionRegistry`], peeks the
//! block's ID through a cloned cursor, and looks up both the shared definition
//! and its owner-specific data context. Omitting that registry is a compile
//! error. There is no static factory and **no hidden parser state**.
//!
//! # `param_definition_type` is a second argument, and it has to be
//!
//! The reference's `ParamDefinition` is a class hierarchy —
//! `MixGainParamDefinition`, `DemixingParamDefinition`,
//! `ReconGainParamDefinition`, `ExtendedParamDefinition` — so
//! `ParamDefinition::GetType()` is a method on the object. This crate's
//! `ParamDefinition` is the **shared wire prefix** those four have in common,
//! because that prefix is what the Audio Element and the Mix Presentation both
//! serialise; the type is carried by the *owner* (the Audio Element's
//! `param_definition_type` field, or "mix gain" implicitly for a Mix
//! Presentation). Passing it separately keeps the shared struct honest about
//! what is actually on the wire in it, and keeps the dependency visible in the
//! signature rather than smuggled into a field.
//!
//! # The animation type answers a question CONTEXT.md deferred
//!
//! IAMF v1.1.0 **does** carry a per-subblock `animation_type`, so a mix-gain
//! ramp is *endpoints plus a type* rather than N sampled values. That reduces
//! what DEC-03's pre-decimated-blocks choice costs. Recorded here; **Phase 4
//! material, not acted on.**

use crate::bits::{BitCursor, BitWriter};
use crate::error::{Error, ErrorKind, Finding, Location, Result};
use crate::obu::param_definition::{
    ParamDefinition, ParamDefinitionRegistry, ParameterDataContext,
};

/// `param_definition_type` — which parameter definition governs a block, and
/// therefore what shape each subblock's parameter data takes.
///
/// Not a field of [`ParamDefinition`]: it is not part of the shared wire
/// prefix. See the module comment.
// ref: iamf-tools@v2.1.0 iamf/obu/param_definitions.h ParamDefinition::ParameterDefinitionType
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamDefinitionType {
    /// `PARAMETER_DEFINITION_MIX_GAIN = 0`.
    MixGain,
    /// `PARAMETER_DEFINITION_DEMIXING = 1`.
    Demixing,
    /// `PARAMETER_DEFINITION_RECON_GAIN = 2`.
    ReconGain,
    /// Any other value: 3 and above.
    Reserved(u32),
}

impl ParamDefinitionType {
    /// The uleb128 value this type carries on the wire.
    #[must_use]
    pub const fn value(self) -> u32 {
        match self {
            Self::MixGain => 0,
            Self::Demixing => 1,
            Self::ReconGain => 2,
            Self::Reserved(raw) => raw,
        }
    }

    /// The type a `param_definition_type` field carries.
    #[must_use]
    pub const fn from_value(value: u32) -> Self {
        match value {
            0 => Self::MixGain,
            1 => Self::Demixing,
            2 => Self::ReconGain,
            other => Self::Reserved(other),
        }
    }
}

/// `MixGainParameterData::AnimationType`, a uleb128.
// ref: iamf-tools@v2.1.0 iamf/obu/mix_gain_parameter_data.h MixGainParameterData::AnimationType
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    /// `kAnimateStep = 0`.
    Step,
    /// `kAnimateLinear = 1`.
    Linear,
    /// `kAnimateBezier = 2`.
    Bezier,
    /// 3 and above — reserved.
    Reserved(u32),
}

impl AnimationType {
    /// The uleb128 value this animation carries.
    #[must_use]
    pub const fn value(self) -> u32 {
        match self {
            Self::Step => 0,
            Self::Linear => 1,
            Self::Bezier => 2,
            Self::Reserved(raw) => raw,
        }
    }

    /// The animation an `animation_type` field selects.
    #[must_use]
    pub const fn from_value(value: u32) -> Self {
        match value {
            0 => Self::Step,
            1 => Self::Linear,
            2 => Self::Bezier,
            other => Self::Reserved(other),
        }
    }
}

/// `mix_gain_parameter_data` — the animation and the points it needs.
///
/// The `animation_type` field is **derived** from the variant
/// ([`MixGainParameterData::animation_type`]) rather than stored beside it
/// (Pattern 2): a stored type that disagreed with the point set would write a
/// selector claiming one shape and then emit another, and the reference would
/// mis-frame every byte after it.
// ref: iamf-tools@v2.1.0 iamf/obu/mix_gain_parameter_data.h AnimationStepInt16
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MixGainParameterData {
    /// `AnimationStepInt16`.
    Step {
        /// `start_point_value`, Q7.8.
        start_point_value: i16,
    },
    /// `AnimationLinearInt16`.
    Linear {
        /// `start_point_value`, Q7.8.
        start_point_value: i16,
        /// `end_point_value`, Q7.8.
        end_point_value: i16,
    },
    /// `AnimationBezierInt16`.
    Bezier {
        /// `start_point_value`, Q7.8.
        start_point_value: i16,
        /// `end_point_value`, Q7.8.
        end_point_value: i16,
        /// `control_point_value`, Q7.8.
        control_point_value: i16,
        /// `control_point_relative_time`, Q0.8 in a `u8`.
        control_point_relative_time: u8,
    },
}

impl MixGainParameterData {
    /// `animation_type`, derived from the data it gates.
    #[must_use]
    pub const fn animation_type(&self) -> AnimationType {
        match self {
            Self::Step { .. } => AnimationType::Step,
            Self::Linear { .. } => AnimationType::Linear,
            Self::Bezier { .. } => AnimationType::Bezier,
        }
    }
}

/// One subblock's parameter data.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterData {
    /// `PARAMETER_DEFINITION_MIX_GAIN`.
    MixGain(MixGainParameterData),
    /// `PARAMETER_DEFINITION_DEMIXING`.
    Demixing(DemixingInfoParameterData),
    /// `PARAMETER_DEFINITION_RECON_GAIN`.
    ReconGain(ReconGainInfoParameterData),
    /// An unmodelled `param_definition_type`: `parameter_data_size` plus
    /// exactly that many bytes, kept verbatim so the block re-serialises
    /// unchanged.
    ///
    /// This is the same mechanism the Audio Element uses for a reserved
    /// element type, and it works for the same reason: the length is **on the
    /// wire**, so the payload can be skipped without being understood.
    Raw(Vec<u8>),
}

/// The exact 3+5-bit Demixing Info parameter-data byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemixingInfoParameterData {
    /// `dmixp_mode`, 3 bits. Reserved values are retained and diagnosed.
    pub dmixp_mode: u8,
    /// Reserved, 5 bits.
    pub reserved: u8,
}

/// One present Recon Gain layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconGainElement {
    /// ULEB bitmask selecting which of the twelve gain bytes are present.
    pub recon_gain_flag: u32,
    /// Gains indexed by flag bit; unselected entries are zero after parsing.
    pub recon_gain: [u8; 12],
}

/// Recon Gain data aligned one-for-one with the associated channel layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconGainInfoParameterData {
    /// `None` for a layer whose descriptor flag is clear.
    pub layers: Vec<Option<ReconGainElement>>,
}

impl ParameterData {
    /// Semantic findings that do not prevent faithful parsing or writing.
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        match self {
            Self::Demixing(data) => {
                if matches!(data.dmixp_mode, 3 | 7) {
                    findings.push(Finding {
                        at: Location::Field("dmixp_mode"),
                        message: format!("dmixp_mode {} is reserved", data.dmixp_mode),
                    });
                }
                if data.reserved != 0 {
                    findings.push(Finding {
                        at: Location::Field("reserved"),
                        message: format!(
                            "demixing reserved field carries non-zero value {}",
                            data.reserved
                        ),
                    });
                }
            }
            Self::ReconGain(data) => {
                for element in data.layers.iter().flatten() {
                    if element.recon_gain_flag & !0x0fff != 0 {
                        findings.push(Finding {
                            at: Location::Field("recon_gain_flag"),
                            message: format!(
                                "recon_gain_flag {} sets reserved bits above bit 11",
                                element.recon_gain_flag
                            ),
                        });
                    }
                }
            }
            Self::MixGain(_) | Self::Raw(_) => {}
        }
        findings
    }
}

/// One entry of `subblocks`.
///
/// `subblock_duration` is an `Option` rather than a value with a
/// "not applicable" reading, because whether it is on the wire is decided by
/// the governing definition and the block's `constant_subblock_duration`
/// together — the gate and the field travel as one (Pattern 2).
// ref: iamf-tools@v2.1.0 iamf/obu/parameter_block.h ParameterSubblock
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterSubblock {
    /// `subblock_duration`, present only when `param_definition_mode == 1` and
    /// `constant_subblock_duration == 0`.
    pub subblock_duration: Option<u32>,
    /// The parameter data itself.
    pub data: ParameterData,
}

impl ParameterSubblock {
    /// The mix-gain data, if this subblock carries any.
    #[must_use]
    pub const fn mix_gain(&self) -> Option<&MixGainParameterData> {
        match &self.data {
            ParameterData::MixGain(data) => Some(data),
            ParameterData::Demixing(_) | ParameterData::ReconGain(_) | ParameterData::Raw(_) => {
                None
            }
        }
    }

    /// The verbatim bytes, if this subblock carries an unmodelled parameter
    /// type.
    #[must_use]
    pub fn raw(&self) -> Option<&[u8]> {
        match &self.data {
            ParameterData::Raw(bytes) => Some(bytes),
            ParameterData::MixGain(_)
            | ParameterData::Demixing(_)
            | ParameterData::ReconGain(_) => None,
        }
    }
}

/// The `duration` / `constant_subblock_duration` block a Parameter Block
/// carries **in itself** when `param_definition_mode == 1`.
///
/// Distinct from `param_definition::DurationFields`, which is the same two
/// fields plus a `subblock_durations` list, in the *definition*. The block's
/// per-subblock durations live inside each [`ParameterSubblock`], so carrying a
/// second list here would give the same wire bytes two owners.
///
/// `num_subblocks` is **derived** from `subblocks.len()` and never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDurationFields {
    /// `duration`.
    pub duration: u32,
    /// `constant_subblock_duration`. When non-zero, `num_subblocks` is implied
    /// as `ceil(duration / constant_subblock_duration)` and is not on the wire.
    pub constant_subblock_duration: u32,
}

/// The Parameter Block OBU payload.
// ref: iamf-tools@v2.1.0 iamf/obu/parameter_block.h ParameterBlockObu
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterBlock {
    /// `parameter_id`. Must equal the governing definition's.
    pub parameter_id: u32,
    /// Present exactly when the governing definition's
    /// `param_definition_mode` is 1.
    pub duration_fields: Option<BlockDurationFields>,
    /// `subblocks`, in bitstream order.
    pub subblocks: Vec<ParameterSubblock>,
}

impl ParameterBlock {
    /// Semantic findings carried by its parameter-data subblocks.
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        self.subblocks
            .iter()
            .flat_map(|subblock| subblock.data.validate())
            .collect()
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/parameter_block.cc ParameterBlockObu::ReadAndValidatePayloadDerived
// ref: iamf-tools@v2.1.0 iamf/obu/mix_gain_parameter_data.cc MixGainParameterData::ReadAndValidate
/// Read a Parameter Block payload, given the **descriptor-resident context**
/// that governs it.
///
/// `def` decides whether the block carries its own duration fields; `kind`
/// decides the shape of each subblock's parameter data. Both are explicit
/// arguments — see the module comment for why that is the requirement rather
/// than a convenience.
pub fn read_parameter_block(
    r: &mut BitCursor<'_>,
    registry: &ParamDefinitionRegistry,
) -> Result<ParameterBlock> {
    let start = r.byte_position();
    let mut peek = r.clone();
    let peeked_parameter_id = peek.read_uleb128()?;
    let registered = registry.get(peeked_parameter_id).ok_or_else(|| {
        Error::new(
            ErrorKind::NoGoverningParamDefinition,
            Location::InputOffset(start),
        )
    })?;
    let def = &registered.definition;
    let context = &registered.context;
    let parameter_id = r.read_uleb128()?;
    // The reference checks that the id in the bitstream agrees with the
    // definition it was handed. Ours can too, and for the same reason: a
    // disagreement means the caller looked up the wrong definition, and every
    // field after this point would then be shaped by the wrong rules.
    if parameter_id != def.parameter_id {
        return Err(Error::new(
            ErrorKind::ParameterIdMismatch,
            Location::InputOffset(start),
        ));
    }

    // Under mode 1 the block carries its own duration fields; under mode 0 they
    // come from the definition and the block omits them. This is the single
    // fact that makes the definition an argument rather than a convenience.
    let (duration_fields, num_subblocks) = if def.param_definition_mode() {
        let duration = r.read_uleb128()?;
        let constant_subblock_duration = r.read_uleb128()?;
        let num_subblocks = if constant_subblock_duration == 0 {
            r.read_uleb128()?
        } else {
            subblocks_implied_by(
                duration,
                constant_subblock_duration,
                Location::InputOffset(start),
            )?
        };
        (
            Some(BlockDurationFields {
                duration,
                constant_subblock_duration,
            }),
            num_subblocks,
        )
    } else {
        let fields = def.duration_fields.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::ParameterModeMismatch,
                Location::InputOffset(start),
            )
        })?;
        let count = if fields.constant_subblock_duration == 0 {
            u32::try_from(fields.subblock_durations.len())
                .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?
        } else {
            subblocks_implied_by(
                fields.duration,
                fields.constant_subblock_duration,
                Location::InputOffset(start),
            )?
        };
        (None, count)
    };

    // `subblock_duration` is on the wire exactly when mode is 1 AND
    // `constant_subblock_duration` is 0.
    let include_subblock_duration =
        duration_fields.is_some_and(|fields| fields.constant_subblock_duration == 0);

    // T-01-32: `num_subblocks` is attacker-controlled and drives an allocation.
    // The smallest possible subblock is one byte, so more subblocks than bytes
    // left is unreadable by construction — checked before a single element is
    // reserved (the one rule every count in this format obeys).
    let num_subblocks = usize::try_from(num_subblocks)
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
    if num_subblocks > r.bytes_remaining() {
        return Err(Error::new(
            ErrorKind::UnexpectedEndOfInput,
            Location::InputOffset(start),
        ));
    }

    let mut subblocks = Vec::with_capacity(num_subblocks);
    let mut total_subblock_duration = 0_u64;
    for _ in 0..num_subblocks {
        let subblock_duration = if include_subblock_duration {
            let value = r.read_uleb128()?;
            total_subblock_duration = total_subblock_duration
                .checked_add(u64::from(value))
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::SubblockDurationMismatch,
                        Location::InputOffset(start),
                    )
                })?;
            Some(value)
        } else {
            None
        };
        subblocks.push(ParameterSubblock {
            subblock_duration,
            data: read_parameter_data(r, context)?,
        });
    }

    if include_subblock_duration {
        let declared = duration_fields.map_or(0, |fields| u64::from(fields.duration));
        if total_subblock_duration != declared {
            return Err(Error::new(
                ErrorKind::SubblockDurationMismatch,
                Location::InputOffset(start),
            ));
        }
    }

    Ok(ParameterBlock {
        parameter_id,
        duration_fields,
        subblocks,
    })
}

// ref: iamf-tools@v2.1.0 iamf/obu/parameter_block.cc ParameterBlockObu::ValidateAndWritePayload
/// Write a Parameter Block payload, given the definition that governs it.
///
/// A block whose `duration_fields` presence disagrees with the definition's
/// `param_definition_mode` is a typed error, not a silent choice: writing the
/// fields under mode 0 (or omitting them under mode 1) produces a block the
/// reference re-frames from the wrong offset.
pub fn write_parameter_block(
    w: &mut BitWriter,
    def: &ParamDefinition,
    context: &ParameterDataContext,
    block: &ParameterBlock,
) -> Result<()> {
    validate_parameter_block(def, context, block)?;

    w.write_uleb128_minimal(block.parameter_id)?;
    if let Some(fields) = block.duration_fields {
        w.write_uleb128_minimal(fields.duration)?;
        w.write_uleb128_minimal(fields.constant_subblock_duration)?;
        if fields.constant_subblock_duration == 0 {
            // `num_subblocks` is derived from the list it counts (Pattern 2).
            let count = u32::try_from(block.subblocks.len()).map_err(|_| {
                Error::new(ErrorKind::ObuTooLarge, Location::Field("num_subblocks"))
            })?;
            w.write_uleb128_minimal(count)?;
        }
    }

    for subblock in &block.subblocks {
        if let Some(duration) = subblock.subblock_duration {
            w.write_uleb128_minimal(duration)?;
        }
        write_parameter_data(w, context, &subblock.data)?;
    }
    Ok(())
}

pub(crate) fn validate_parameter_block(
    def: &ParamDefinition,
    context: &ParameterDataContext,
    block: &ParameterBlock,
) -> Result<()> {
    if block.parameter_id != def.parameter_id {
        return Err(Error::new(
            ErrorKind::ParameterIdMismatch,
            Location::Field("parameter_id"),
        ));
    }
    if block.duration_fields.is_some() != def.param_definition_mode() {
        return Err(Error::new(
            ErrorKind::ParameterModeMismatch,
            Location::Field("param_definition_mode"),
        ));
    }
    for subblock in &block.subblocks {
        validate_parameter_data_kind(&subblock.data, context)?;
    }

    let (expected_count, carries_subblock_duration, declared_duration) =
        if let Some(fields) = block.duration_fields {
            let count = if fields.constant_subblock_duration == 0 {
                u32::try_from(block.subblocks.len()).map_err(|_| {
                    Error::new(ErrorKind::ObuTooLarge, Location::Field("num_subblocks"))
                })?
            } else {
                subblocks_implied_by(
                    fields.duration,
                    fields.constant_subblock_duration,
                    Location::Field("subblocks"),
                )?
            };
            let carries = fields.constant_subblock_duration == 0;
            (count, carries, carries.then_some(fields.duration))
        } else {
            let fields = def.duration_fields.as_ref().ok_or_else(|| {
                Error::new(
                    ErrorKind::ParameterModeMismatch,
                    Location::Field("duration_fields"),
                )
            })?;
            let count = if fields.constant_subblock_duration == 0 {
                u32::try_from(fields.subblock_durations.len()).map_err(|_| {
                    Error::new(ErrorKind::ObuTooLarge, Location::Field("subblocks"))
                })?
            } else {
                subblocks_implied_by(
                    fields.duration,
                    fields.constant_subblock_duration,
                    Location::Field("subblocks"),
                )?
            };
            (count, false, None)
        };

    let actual_count = u32::try_from(block.subblocks.len())
        .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::Field("subblocks")))?;
    if actual_count != expected_count {
        return Err(Error::new(
            ErrorKind::SubblockDurationMismatch,
            Location::Field("subblocks"),
        ));
    }

    let mut duration_sum = 0_u64;
    for subblock in &block.subblocks {
        if subblock.subblock_duration.is_some() != carries_subblock_duration {
            return Err(Error::new(
                ErrorKind::SubblockDurationMismatch,
                Location::Field("subblock_duration"),
            ));
        }
        if let Some(duration) = subblock.subblock_duration {
            duration_sum = duration_sum.checked_add(u64::from(duration)).ok_or_else(|| {
                Error::new(
                    ErrorKind::SubblockDurationMismatch,
                    Location::Field("subblock_duration"),
                )
            })?;
        }
    }
    if declared_duration.is_some_and(|duration| duration_sum != u64::from(duration)) {
        return Err(Error::new(
            ErrorKind::SubblockDurationMismatch,
            Location::Field("subblock_duration"),
        ));
    }
    Ok(())
}

fn validate_parameter_data_kind(
    data: &ParameterData,
    context: &ParameterDataContext,
) -> Result<()> {
    let matches = match (context, data) {
        (ParameterDataContext::MixGain, ParameterData::MixGain(_))
        | (ParameterDataContext::Demixing, ParameterData::Demixing(_))
        | (ParameterDataContext::Reserved(3..), ParameterData::Raw(_)) => true,
        (
            ParameterDataContext::ReconGain {
                recon_gain_is_present,
            },
            ParameterData::ReconGain(data),
        ) => {
            data.layers.len() == recon_gain_is_present.len()
                && data
                    .layers
                    .iter()
                    .zip(recon_gain_is_present)
                    .all(|(layer, present)| layer.is_some() == *present)
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::UnsupportedParameterData,
            if matches!(context, ParameterDataContext::ReconGain { .. }) {
                Location::Field("recon_gain_is_present")
            } else {
                Location::Field("parameter_data")
            },
        ))
    }
}

/// `ceil(duration / constant_subblock_duration)`, with the division checked.
///
/// Integer division with a ceiling correction, exactly as `GetNumSubblocks`
/// computes it. The divisor is non-zero at every call site here, and it is
/// still `checked_div`: GUARD-03 denies bare arithmetic because a bound that
/// holds only by argument stops holding when the argument is edited.
// ref: iamf-tools@v2.1.0 iamf/obu/parameter_block.cc ParameterBlockObu::GetNumSubblocks
fn subblocks_implied_by(
    duration: u32,
    constant_subblock_duration: u32,
    at: Location,
) -> Result<u32> {
    let overflow = || {
        Error::new(ErrorKind::SubblockDurationMismatch, at)
    };
    let whole = duration
        .checked_div(constant_subblock_duration)
        .ok_or_else(overflow)?;
    let remainder = duration
        .checked_rem(constant_subblock_duration)
        .ok_or_else(overflow)?;
    if remainder == 0 {
        Ok(whole)
    } else {
        whole.checked_add(1).ok_or_else(overflow)
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_gain_parameter_data.cc MixGainParameterData::ReadAndValidate
// ref: iamf-tools@v2.1.0 iamf/obu/extension_parameter_data.cc ExtensionParameterData::ReadAndValidate
// NOTE: an animation type above 2 is `absl::UnimplementedError` in the
// reference, and it has to be: the wire carries no length for an unrecognised
// animation, so there is no boundary to preserve verbatim without inventing
// one. Matching the reference exactly is the rule — being looser here would
// mean guessing where the next subblock starts. An unmodelled
// `param_definition_type` is a different case entirely: it has an explicit
// `parameter_data_size` on the wire, so "verbatim" is well defined.
fn read_parameter_data(
    r: &mut BitCursor<'_>,
    context: &ParameterDataContext,
) -> Result<ParameterData> {
    let start = r.byte_position();
    match context {
        ParameterDataContext::MixGain => {
            let animation = AnimationType::from_value(r.read_uleb128()?);
            let data = match animation {
                AnimationType::Step => MixGainParameterData::Step {
                    start_point_value: read_i16(r)?,
                },
                AnimationType::Linear => MixGainParameterData::Linear {
                    start_point_value: read_i16(r)?,
                    end_point_value: read_i16(r)?,
                },
                AnimationType::Bezier => MixGainParameterData::Bezier {
                    start_point_value: read_i16(r)?,
                    end_point_value: read_i16(r)?,
                    control_point_value: read_i16(r)?,
                    control_point_relative_time: u8::try_from(r.read_unsigned(8)?).unwrap_or(0),
                },
                AnimationType::Reserved(_) => {
                    return Err(Error::new(
                        ErrorKind::UnsupportedParameterData,
                        Location::InputOffset(start),
                    ));
                }
            };
            Ok(ParameterData::MixGain(data))
        }
        // `parameter_data_size` then exactly that many bytes. The length goes
        // through `read_uint8_span`, which caps it against `bytes_remaining()`
        // before reserving.
        ParameterDataContext::Demixing => Ok(ParameterData::Demixing(
            DemixingInfoParameterData {
                dmixp_mode: u8::try_from(r.read_unsigned(3)?).unwrap_or(0),
                reserved: u8::try_from(r.read_unsigned(5)?).unwrap_or(0),
            },
        )),
        ParameterDataContext::ReconGain {
            recon_gain_is_present,
        } => {
            let mut layers = Vec::with_capacity(recon_gain_is_present.len());
            for present in recon_gain_is_present {
                if !present {
                    layers.push(None);
                    continue;
                }
                let recon_gain_flag = r.read_uleb128()?;
                let mut recon_gain = [0_u8; 12];
                for index in 0_u32..12 {
                    if recon_gain_flag & (1_u32 << index) != 0 {
                        let value = u8::try_from(r.read_unsigned(8)?).unwrap_or(0);
                        if let Some(slot) = recon_gain.get_mut(usize::try_from(index).unwrap_or(0)) {
                            *slot = value;
                        }
                    }
                }
                layers.push(Some(ReconGainElement {
                    recon_gain_flag,
                    recon_gain,
                }));
            }
            Ok(ParameterData::ReconGain(ReconGainInfoParameterData {
                layers,
            }))
        }
        ParameterDataContext::Reserved(0..=2) => Err(Error::new(
            ErrorKind::UnsupportedParameterData,
            Location::InputOffset(start),
        )),
        ParameterDataContext::Reserved(_) => {
            let size = r.read_uleb128()?;
            let size = usize::try_from(size)
                .map_err(|_| Error::new(ErrorKind::ObuTooLarge, Location::InputOffset(start)))?;
            Ok(ParameterData::Raw(r.read_uint8_span(size)?.to_vec()))
        }
    }
}

// ref: iamf-tools@v2.1.0 iamf/obu/mix_gain_parameter_data.cc MixGainParameterData::Write
// ref: iamf-tools@v2.1.0 iamf/obu/extension_parameter_data.cc ExtensionParameterData::Write
fn write_parameter_data(
    w: &mut BitWriter,
    context: &ParameterDataContext,
    data: &ParameterData,
) -> Result<()> {
    match data {
        ParameterData::MixGain(mix_gain) => {
            // The selector is derived from the variant, never stored (Pattern 2).
            w.write_uleb128_minimal(mix_gain.animation_type().value())?;
            match *mix_gain {
                MixGainParameterData::Step { start_point_value } => {
                    w.write_signed(i64::from(start_point_value), 16)
                }
                MixGainParameterData::Linear {
                    start_point_value,
                    end_point_value,
                } => {
                    w.write_signed(i64::from(start_point_value), 16)?;
                    w.write_signed(i64::from(end_point_value), 16)
                }
                MixGainParameterData::Bezier {
                    start_point_value,
                    end_point_value,
                    control_point_value,
                    control_point_relative_time,
                } => {
                    w.write_signed(i64::from(start_point_value), 16)?;
                    w.write_signed(i64::from(end_point_value), 16)?;
                    w.write_signed(i64::from(control_point_value), 16)?;
                    w.write_unsigned(u64::from(control_point_relative_time), 8)
                }
            }
        }
        ParameterData::Demixing(data) => {
            w.write_unsigned(u64::from(data.dmixp_mode), 3)?;
            w.write_unsigned(u64::from(data.reserved), 5)
        }
        ParameterData::ReconGain(data) => {
            let ParameterDataContext::ReconGain {
                recon_gain_is_present,
            } = context
            else {
                return Err(Error::new(
                    ErrorKind::UnsupportedParameterData,
                    Location::Field("parameter_data"),
                ));
            };
            for (layer, present) in data.layers.iter().zip(recon_gain_is_present) {
                if !present {
                    continue;
                }
                let element = layer.as_ref().ok_or_else(|| {
                    Error::new(
                        ErrorKind::UnsupportedParameterData,
                        Location::Field("recon_gain_is_present"),
                    )
                })?;
                w.write_uleb128_minimal(element.recon_gain_flag)?;
                for (index, gain) in element.recon_gain.iter().enumerate() {
                    let index = u32::try_from(index).unwrap_or(0);
                    if element.recon_gain_flag & (1_u32 << index) != 0 {
                        w.write_unsigned(u64::from(*gain), 8)?;
                    }
                }
            }
            Ok(())
        }
        ParameterData::Raw(bytes) => {
            let size = u32::try_from(bytes.len()).map_err(|_| {
                Error::new(
                    ErrorKind::ObuTooLarge,
                    Location::Field("parameter_data_size"),
                )
            })?;
            w.write_uleb128_minimal(size)?;
            w.write_bytes(bytes)
        }
    }
}

/// A signed 16-bit big-endian field, the width every Q7.8 point uses.
// ref: iamf-tools@v2.1.0 iamf/common/read_bit_buffer.h ReadBitBuffer::ReadSigned16
fn read_i16(r: &mut BitCursor<'_>) -> Result<i16> {
    i16::try_from(r.read_signed(16)?).map_err(|_| {
        Error::new(
            ErrorKind::ValueExceedsWidth { bits: 16 },
            Location::Unlocated,
        )
    })
}
