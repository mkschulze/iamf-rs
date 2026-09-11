//! Immutable high-level IAMF descriptor authoring.
//!
//! The builder owns caller-local handles and assigns all wire ids only after
//! static validation has accepted the complete declaration set.

use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::layout::{AmbisonicsConfig, AmbisonicsMonoConfig};
use crate::model::{select_minimum_profile, DescriptorSet, Profile};
use crate::obu::{
    AudioElement, AudioElementType, AudioFrame, CodecConfig, DecoderConfig, IaSequenceHeader,
    MixGainParamDefinition, MixPresentation, Obu, ObuHeader, ObuType, ParamDefinitionRegistry,
    ParameterBlock, Trimming, CODEC_ID_FLAC, CODEC_ID_LPCM, CODEC_ID_OPUS,
};
use core::sync::atomic::{AtomicU64, Ordering};
use std::io::Write;

use crate::sequence::{SequenceWriter, TemporalUnit};

static NEXT_BUILDER_GENERATION: AtomicU64 = AtomicU64::new(0);

/// A caller-local reference to a declared codec configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecConfigHandle {
    generation: u64,
    index: usize,
}

/// A caller-local reference to a declared audio element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioElementHandle {
    generation: u64,
    index: usize,
}

/// A caller-local reference to a declared mix presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixPresentationHandle {
    generation: u64,
    index: usize,
}

/// A caller-local reference to one audio substream, optionally shared by elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubstreamHandle {
    generation: u64,
    index: usize,
}

/// A caller-local reference to a declared mix-gain parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterHandle {
    generation: u64,
    index: usize,
}

/// The deterministic mapping from caller-local handles to IAMF wire ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdManifest {
    codec_configs: Vec<(CodecConfigHandle, u32)>,
    audio_elements: Vec<(AudioElementHandle, u32)>,
    mix_presentations: Vec<(MixPresentationHandle, u32)>,
    substream_ids: Vec<(SubstreamHandle, u32)>,
    parameter_ids: Vec<(ParameterHandle, u32)>,
    element_substreams: Vec<(AudioElementHandle, Vec<SubstreamHandle>)>,
    presentation_parameters: Vec<(MixPresentationHandle, Vec<ParameterHandle>)>,
    sequence_profile: Profile,
}

impl IdManifest {
    /// The wire id of a substream, including an explicitly shared substream.
    #[must_use]
    pub fn substream_id(&self, handle: SubstreamHandle) -> Option<u32> {
        lookup_id(&self.substream_ids, handle)
    }

    /// The wire id of a mix-gain parameter.
    #[must_use]
    pub fn parameter_id(&self, handle: ParameterHandle) -> Option<u32> {
        lookup_id(&self.parameter_ids, handle)
    }

    /// An element's substream handles in frame order, including implicit declarations.
    #[must_use]
    pub fn substreams(&self, handle: AudioElementHandle) -> Option<&[SubstreamHandle]> {
        self.element_substreams
            .iter()
            .find(|(candidate, _)| *candidate == handle)
            .map(|(_, handles)| handles.as_slice())
    }

    /// A presentation's parameter handles: element gains then output gain for each sub-mix.
    #[must_use]
    pub fn parameters(&self, handle: MixPresentationHandle) -> Option<&[ParameterHandle]> {
        self.presentation_parameters
            .iter()
            .find(|(candidate, _)| *candidate == handle)
            .map(|(_, handles)| handles.as_slice())
    }
    /// The wire id allocated to a codec configuration handle.
    #[must_use]
    pub fn codec_config_id(&self, handle: CodecConfigHandle) -> Option<u32> {
        lookup_id(&self.codec_configs, handle)
    }

    /// The wire id allocated to an audio element handle.
    #[must_use]
    pub fn audio_element_id(&self, handle: AudioElementHandle) -> Option<u32> {
        lookup_id(&self.audio_elements, handle)
    }

    /// The wire id allocated to a mix presentation handle.
    #[must_use]
    pub fn mix_presentation_id(&self, handle: MixPresentationHandle) -> Option<u32> {
        lookup_id(&self.mix_presentations, handle)
    }

    /// The minimum profile required by the most demanding Mix Presentation.
    #[must_use]
    pub const fn sequence_profile(&self) -> Profile {
        self.sequence_profile
    }
}

fn lookup_id<H: Copy + PartialEq>(entries: &[(H, u32)], handle: H) -> Option<u32> {
    entries
        .iter()
        .find(|(candidate, _)| *candidate == handle)
        .map(|(_, id)| *id)
}

/// A completed, immutable descriptor configuration.
#[derive(Debug, Clone)]
pub struct Encoder {
    descriptors: DescriptorSet,
    generation: u64,
}

impl PartialEq for Encoder {
    fn eq(&self, other: &Self) -> bool {
        self.descriptors == other.descriptors
    }
}

impl Eq for Encoder {}

impl Encoder {
    /// The frozen descriptor model this encoder will write.
    #[must_use]
    pub const fn descriptors(&self) -> &DescriptorSet {
        &self.descriptors
    }

    /// Begin writing this frozen configuration to `sink`.
    ///
    /// The descriptor prologue is written immediately. Subsequent temporal
    /// input is validated as a complete unit before the underlying sequence
    /// writer receives it, so an input error cannot emit a partial unit.
    pub fn start<W: Write>(self, sink: W) -> Result<EncodingWriter<W>> {
        let mut sequence = SequenceWriter::new(sink);
        sequence.push_descriptors(&self.descriptors)?;
        Ok(EncodingWriter {
            sequence,
            descriptors: self.descriptors,
            generation: self.generation,
        })
    }
}

/// One caller-owned, pre-encoded Audio Frame payload.
///
/// This type deliberately has no encoder, decoder, resampler, or PCM
/// conversion operation. FLAC and Opus access units are handed through
/// verbatim; LPCM bytes are only checked against the frozen frame plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameInput {
    /// IAMF-LPCM sample bytes in the codec configuration's declared format.
    Lpcm(Vec<u8>),
    /// One already encoded FLAC access unit.
    Flac(Vec<u8>),
    /// One already encoded Opus access unit.
    Opus(Vec<u8>),
}

/// A Parameter Block submitted against its frozen caller-local definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedParameterBlock {
    /// The definition accepted during [`EncoderBuilder::build`].
    pub parameter: ParameterHandle,
    /// The already decimated IAMF parameter data to write.
    pub block: ParameterBlock,
}

/// One complete temporal unit supplied to an [`EncodingWriter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalUnitInput {
    /// Frames in the frozen substream declaration order.
    pub frames: Vec<(SubstreamHandle, FrameInput)>,
    /// Parameter Blocks in their requested wire order.
    pub parameter_blocks: Vec<SubmittedParameterBlock>,
    /// The single trim plan shared by every frame in the unit.
    pub trimming: Option<Trimming>,
}

/// A high-level temporal writer backed by exactly one [`SequenceWriter`].
#[derive(Debug)]
pub struct EncodingWriter<W: Write> {
    sequence: SequenceWriter<W>,
    descriptors: DescriptorSet,
    generation: u64,
}

impl<W: Write> EncodingWriter<W> {
    /// Bytes successfully handed to the sink, including the descriptor prologue.
    #[must_use]
    pub const fn bytes_written(&self) -> u64 {
        self.sequence.bytes_written()
    }

    /// Validate and append one complete temporal unit.
    ///
    /// All caller input is lowered before the underlying writer is called.
    /// Therefore a typed input failure leaves the sink exactly at the previous
    /// temporal-unit boundary.
    pub fn push_temporal_unit(&mut self, input: TemporalUnitInput) -> Result<()> {
        let unit = self.preflight(input)?;
        self.sequence.push_temporal_unit(&unit)
    }

    /// Flush the append-only sequence and return its sink.
    ///
    /// This consumes the writer, preserving the underlying writer's poisoned
    /// state and its compile-time prohibition on submissions after finishing.
    pub fn finish(self) -> Result<W> {
        self.sequence.finish()
    }

    fn preflight(&self, input: TemporalUnitInput) -> Result<TemporalUnit> {
        let mut submitted_handles = Vec::with_capacity(input.frames.len());
        for (handle, _) in &input.frames {
            if submitted_handles.contains(handle) {
                return Err(temporal_input(
                    ErrorKind::DuplicateTemporalSubstream,
                    "frames",
                ));
            }
            submitted_handles.push(*handle);
        }

        let expected = self.declared_substreams();
        if input.frames.len() != expected.len() {
            return Err(temporal_input(
                ErrorKind::MissingTemporalSubstream,
                "frames",
            ));
        }

        let mut frames = Vec::with_capacity(input.frames.len());
        for ((handle, frame), expected_id) in input.frames.into_iter().zip(expected) {
            let id = self.substream_id(handle)?;
            if id != expected_id {
                return Err(temporal_input(
                    ErrorKind::TemporalSubstreamOrderMismatch,
                    "frames",
                ));
            }
            let (config, channels) = self.substream_plan(id)?;
            self.validate_frame(&config, channels, &frame)?;
            frames.push(AudioFrame::new(id, frame_payload(frame)).into_obu(input.trimming));
        }

        let mut parameter_blocks = Vec::with_capacity(input.parameter_blocks.len());
        for submitted in input.parameter_blocks {
            let parameter_id = self.parameter_id(submitted.parameter)?;
            if submitted.block.parameter_id != parameter_id {
                return Err(temporal_input(
                    ErrorKind::ParameterIdMismatch,
                    "parameter_id",
                ));
            }
            parameter_blocks.push(Obu::new(
                ObuHeader::new(ObuType::ParameterBlock),
                submitted.block,
            ));
        }

        Ok(TemporalUnit {
            temporal_delimiter: None,
            parameter_blocks,
            audio_frames: frames,
        })
    }

    fn declared_substreams(&self) -> Vec<u32> {
        let mut ids = Vec::new();
        for element in &self.descriptors.audio_elements {
            for id in &element.audio_substream_ids {
                if !ids.contains(id) {
                    ids.push(*id);
                }
            }
        }
        ids
    }

    fn substream_id(&self, handle: SubstreamHandle) -> Result<u32> {
        if handle.generation != self.generation {
            return Err(temporal_input(
                ErrorKind::UnknownTemporalSubstreamHandle,
                "substream_handle",
            ));
        }
        let id = u32::try_from(handle.index).map_err(|_| {
            temporal_input(
                ErrorKind::UnknownTemporalSubstreamHandle,
                "substream_handle",
            )
        })?;
        if self.declared_substreams().contains(&id) {
            Ok(id)
        } else {
            Err(temporal_input(
                ErrorKind::UnknownTemporalSubstreamHandle,
                "substream_handle",
            ))
        }
    }

    fn parameter_id(&self, handle: ParameterHandle) -> Result<u32> {
        if handle.generation != self.generation {
            return Err(temporal_input(
                ErrorKind::UnknownTemporalParameterHandle,
                "parameter_handle",
            ));
        }
        let id = u32::try_from(handle.index).map_err(|_| {
            temporal_input(
                ErrorKind::UnknownTemporalParameterHandle,
                "parameter_handle",
            )
        })?;
        if ParamDefinitionRegistry::from_descriptors(&self.descriptors)?
            .get(id)
            .is_some()
        {
            Ok(id)
        } else {
            Err(temporal_input(
                ErrorKind::UnknownTemporalParameterHandle,
                "parameter_handle",
            ))
        }
    }

    fn substream_plan(&self, id: u32) -> Result<(CodecConfig, u8)> {
        for element in &self.descriptors.audio_elements {
            for (position, candidate) in element.audio_substream_ids.iter().enumerate() {
                if *candidate == id {
                    let config = self
                        .descriptors
                        .codec_configs
                        .iter()
                        .find(|config| config.codec_config_id == element.codec_config_id)
                        .ok_or_else(|| {
                            temporal_input(ErrorKind::FrameCodecMismatch, "codec_config")
                        })?;
                    return Ok((config.clone(), substream_channels(element, position)));
                }
            }
        }
        Err(temporal_input(
            ErrorKind::UnknownTemporalSubstreamHandle,
            "substream_handle",
        ))
    }

    fn validate_frame(&self, config: &CodecConfig, channels: u8, frame: &FrameInput) -> Result<()> {
        match (&config.decoder_config, frame) {
            (DecoderConfig::Lpcm(lpcm), FrameInput::Lpcm(payload)) => {
                let bytes_per_sample = u64::from(lpcm.sample_size)
                    .checked_div(8)
                    .filter(|bytes| *bytes > 0)
                    .ok_or_else(|| {
                        temporal_input(ErrorKind::LpcmFrameByteAlignment, "frame.payload")
                    })?;
                let payload_length = u64::try_from(payload.len()).map_err(|_| {
                    temporal_input(ErrorKind::LpcmFrameSampleCountMismatch, "frame.payload")
                })?;
                if payload_length.checked_rem(bytes_per_sample) != Some(0) {
                    return Err(temporal_input(
                        ErrorKind::LpcmFrameByteAlignment,
                        "frame.payload",
                    ));
                }
                let expected = u64::from(config.num_samples_per_frame)
                    .checked_mul(u64::from(channels))
                    .and_then(|samples| samples.checked_mul(bytes_per_sample))
                    .ok_or_else(|| {
                        temporal_input(ErrorKind::LpcmFrameSampleCountMismatch, "frame.payload")
                    })?;
                if payload_length != expected {
                    return Err(temporal_input(
                        ErrorKind::LpcmFrameSampleCountMismatch,
                        "frame.payload",
                    ));
                }
                Ok(())
            }
            (DecoderConfig::Flac(_), FrameInput::Flac(_))
            | (DecoderConfig::Opus(_), FrameInput::Opus(_)) => Ok(()),
            (DecoderConfig::Lpcm(_), FrameInput::Flac(_))
            | (DecoderConfig::Lpcm(_), FrameInput::Opus(_))
            | (DecoderConfig::Flac(_), FrameInput::Lpcm(_))
            | (DecoderConfig::Flac(_), FrameInput::Opus(_))
            | (DecoderConfig::Opus(_), FrameInput::Lpcm(_))
            | (DecoderConfig::Opus(_), FrameInput::Flac(_))
            | (DecoderConfig::Raw { .. }, _) => {
                Err(temporal_input(ErrorKind::FrameCodecMismatch, "frame.codec"))
            }
        }
    }
}

fn frame_payload(frame: FrameInput) -> Vec<u8> {
    match frame {
        FrameInput::Lpcm(payload) | FrameInput::Flac(payload) | FrameInput::Opus(payload) => {
            payload
        }
    }
}

fn temporal_input(kind: ErrorKind, field: &'static str) -> Error {
    Error::new(kind, Location::Field(field))
}

#[derive(Debug, Clone)]
struct AudioElementDeclaration {
    codec_config: CodecConfigHandle,
    substreams: Vec<SubstreamHandle>,
    element: AudioElement,
}

#[derive(Debug, Clone)]
struct MixPresentationDeclaration {
    audio_elements: Vec<AudioElementHandle>,
    parameters: Vec<ParameterHandle>,
    presentation: MixPresentation,
}

#[derive(Debug)]
struct ParameterDeclaration {
    implicit_id: Option<u32>,
    gain: MixGainParamDefinition,
}

/// Ordered static declarations waiting to be validated and frozen.
#[derive(Debug)]
pub struct EncoderBuilder {
    generation: u64,
    codec_configs: Vec<CodecConfig>,
    audio_elements: Vec<AudioElementDeclaration>,
    mix_presentations: Vec<MixPresentationDeclaration>,
    substreams: Vec<Option<u32>>,
    parameters: Vec<ParameterDeclaration>,
}

impl EncoderBuilder {
    /// Start an empty static configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            generation: NEXT_BUILDER_GENERATION.fetch_add(1, Ordering::Relaxed),
            codec_configs: Vec::new(),
            audio_elements: Vec::new(),
            mix_presentations: Vec::new(),
            substreams: Vec::new(),
            parameters: Vec::new(),
        }
    }

    /// Declare a codec configuration.
    pub fn add_codec_config(&mut self, config: CodecConfig) -> CodecConfigHandle {
        let handle = CodecConfigHandle {
            generation: self.generation,
            index: self.codec_configs.len(),
        };
        self.codec_configs.push(config);
        handle
    }

    /// Declare an audio element and fresh substreams using the template ids as collision labels.
    ///
    /// Template ids are never emitted. Repeated labels are rejected at build;
    /// use explicit substream handles to share audio between elements.
    pub fn add_audio_element(
        &mut self,
        codec_config: CodecConfigHandle,
        element: AudioElement,
    ) -> AudioElementHandle {
        let substreams = element
            .audio_substream_ids
            .iter()
            .map(|id| self.declare_substream(Some(*id)))
            .collect();
        self.add_audio_element_with_substreams(codec_config, substreams, element)
    }

    /// Declare an element with explicit ordered substreams. Template substream ids
    /// are ignored; their count must match the supplied handles and layer topology.
    pub fn add_audio_element_with_substreams(
        &mut self,
        codec_config: CodecConfigHandle,
        substreams: Vec<SubstreamHandle>,
        element: AudioElement,
    ) -> AudioElementHandle {
        let handle = AudioElementHandle {
            generation: self.generation,
            index: self.audio_elements.len(),
        };
        self.audio_elements.push(AudioElementDeclaration {
            codec_config,
            substreams,
            element,
        });
        handle
    }

    /// Declare one substream. Reusing its handle explicitly shares encoded audio.
    /// Every declared substream must be referenced by at least one element at build.
    pub fn add_substream(&mut self) -> SubstreamHandle {
        self.declare_substream(None)
    }

    fn declare_substream(&mut self, implicit_id: Option<u32>) -> SubstreamHandle {
        let handle = SubstreamHandle {
            generation: self.generation,
            index: self.substreams.len(),
        };
        self.substreams.push(implicit_id);
        handle
    }

    /// Declare a mix-gain parameter. Its template id is ignored; all other fields
    /// define the parameter used by exactly one emitted gain definition.
    /// Unused parameter declarations are rejected at build.
    pub fn add_mix_gain_parameter(&mut self, gain: MixGainParamDefinition) -> ParameterHandle {
        self.declare_parameter(None, gain)
    }

    fn declare_parameter(
        &mut self,
        implicit_id: Option<u32>,
        gain: MixGainParamDefinition,
    ) -> ParameterHandle {
        let handle = ParameterHandle {
            generation: self.generation,
            index: self.parameters.len(),
        };
        self.parameters
            .push(ParameterDeclaration { implicit_id, gain });
        handle
    }

    /// Author the supported scene-based form without exposing the low-level scene constructor.
    pub fn add_ambisonics_mono(
        &mut self,
        codec_config: CodecConfigHandle,
        substreams: Vec<SubstreamHandle>,
        config: AmbisonicsMonoConfig,
    ) -> AudioElementHandle {
        let element = AudioElement {
            audio_element_id: 0,
            reserved: 0,
            codec_config_id: 0,
            audio_element_type: AudioElementType::scene_based(AmbisonicsConfig::Mono(config)),
            audio_substream_ids: vec![0; substreams.len()],
            params: Vec::new(),
            trailing: Vec::new(),
        };
        self.add_audio_element_with_substreams(codec_config, substreams, element)
    }

    /// Declare a presentation and fresh parameters, using template parameter ids
    /// as collision labels. Retrieve their opaque handles from the manifest.
    pub fn add_mix_presentation(
        &mut self,
        audio_elements: Vec<AudioElementHandle>,
        presentation: MixPresentation,
    ) -> MixPresentationHandle {
        let parameters = presentation
            .sub_mixes
            .iter()
            .flat_map(|sub_mix| {
                sub_mix
                    .elements
                    .iter()
                    .map(|element| &element.element_mix_gain)
                    .chain(core::iter::once(&sub_mix.output_mix_gain))
            })
            .map(|gain| self.declare_parameter(Some(gain.definition.parameter_id), gain.clone()))
            .collect();
        self.add_mix_presentation_with_parameters(audio_elements, parameters, presentation)
    }

    /// Declare a presentation referencing explicitly declared gains. References
    /// follow wire order: element gains then output gain for each sub-mix.
    /// The template's complete gain fields are replaced by these declarations.
    /// A parameter handle may appear only once across all presentations.
    pub fn add_mix_presentation_with_parameters(
        &mut self,
        audio_elements: Vec<AudioElementHandle>,
        parameters: Vec<ParameterHandle>,
        presentation: MixPresentation,
    ) -> MixPresentationHandle {
        let handle = MixPresentationHandle {
            generation: self.generation,
            index: self.mix_presentations.len(),
        };
        self.mix_presentations.push(MixPresentationDeclaration {
            audio_elements,
            parameters,
            presentation,
        });
        handle
    }

    /// Validate every static declaration, allocate deterministic ids, and freeze it.
    pub fn build(mut self) -> Result<(Encoder, IdManifest)> {
        self.resolve_parameter_references()?;
        self.validate_declarations()?;

        let mut manifest = IdManifest {
            codec_configs: allocate_ids(self.codec_configs.len(), |index| CodecConfigHandle {
                generation: self.generation,
                index,
            })?,
            audio_elements: allocate_ids(self.audio_elements.len(), |index| AudioElementHandle {
                generation: self.generation,
                index,
            })?,
            mix_presentations: allocate_ids(self.mix_presentations.len(), |index| {
                MixPresentationHandle {
                    generation: self.generation,
                    index,
                }
            })?,
            substream_ids: allocate_ids(self.substreams.len(), |index| SubstreamHandle {
                generation: self.generation,
                index,
            })?,
            parameter_ids: allocate_ids(self.parameters.len(), |index| ParameterHandle {
                generation: self.generation,
                index,
            })?,
            element_substreams: self
                .audio_elements
                .iter()
                .enumerate()
                .map(|(index, declaration)| {
                    (
                        AudioElementHandle {
                            generation: self.generation,
                            index,
                        },
                        declaration.substreams.clone(),
                    )
                })
                .collect(),
            presentation_parameters: self
                .mix_presentations
                .iter()
                .enumerate()
                .map(|(index, declaration)| {
                    (
                        MixPresentationHandle {
                            generation: self.generation,
                            index,
                        },
                        declaration.parameters.clone(),
                    )
                })
                .collect(),
            sequence_profile: Profile::Simple,
        };

        let mut descriptors = DescriptorSet::new(IaSequenceHeader::new(0, 0));
        descriptors.codec_configs = self.codec_configs;
        for (index, config) in descriptors.codec_configs.iter_mut().enumerate() {
            config.codec_config_id = u32::try_from(index).map_err(allocation_error)?;
        }

        for (index, declaration) in self.audio_elements.into_iter().enumerate() {
            let codec_config_id =
                u32::try_from(declaration.codec_config.index).map_err(allocation_error)?;
            let mut element = declaration.element;
            element.audio_element_id = u32::try_from(index).map_err(allocation_error)?;
            element.codec_config_id = codec_config_id;
            element.audio_substream_ids = declaration
                .substreams
                .iter()
                .map(|handle| u32::try_from(handle.index).map_err(allocation_error))
                .collect::<Result<_>>()?;
            descriptors.audio_elements.push(element);
        }

        for (index, declaration) in self.mix_presentations.into_iter().enumerate() {
            let mut presentation = declaration.presentation;
            let mut handles = declaration.audio_elements.into_iter();
            let mut parameters = declaration.parameters.into_iter();
            for sub_mix in &mut presentation.sub_mixes {
                for element in &mut sub_mix.elements {
                    let handle = handles.next().ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidDescriptorReference,
                            Location::Field("mix_presentation.audio_elements"),
                        )
                    })?;
                    element.audio_element_id =
                        u32::try_from(handle.index).map_err(allocation_error)?;
                    element.element_mix_gain.definition.parameter_id =
                        parameter_wire_id(parameters.next())?;
                }
                sub_mix.output_mix_gain.definition.parameter_id =
                    parameter_wire_id(parameters.next())?;
            }
            presentation.mix_presentation_id = u32::try_from(index).map_err(allocation_error)?;
            descriptors.mix_presentations.push(presentation);
        }

        let (primary_profile, additional_profile) =
            select_sequence_profile(&descriptors.mix_presentations, &descriptors.audio_elements)?;
        descriptors.sequence_header =
            IaSequenceHeader::new(primary_profile.to_wire(), additional_profile.to_wire());
        manifest.sequence_profile = primary_profile;

        Ok((
            Encoder {
                descriptors,
                generation: self.generation,
            },
            manifest,
        ))
    }

    fn validate_declarations(&self) -> Result<()> {
        validate_implicit_ids(self.substreams.iter().copied(), "audio_substream_id")?;
        validate_implicit_ids(
            self.parameters
                .iter()
                .map(|declaration| declaration.implicit_id),
            "parameter_id",
        )?;
        for parameter in &self.parameters {
            validate_parameter_definition(&parameter.gain.definition)?;
            validate_findings(parameter.gain.definition.validate())?;
        }
        for config in &self.codec_configs {
            let expected = match &config.decoder_config {
                DecoderConfig::Lpcm(_) => CODEC_ID_LPCM,
                DecoderConfig::Flac(_) => CODEC_ID_FLAC,
                DecoderConfig::Opus(_) => CODEC_ID_OPUS,
                DecoderConfig::Raw { .. } => return Err(invalid("decoder_config")),
            };
            if config.codec_id != expected {
                return Err(invalid("codec_id"));
            }
            validate_findings(config.validate())?;
        }
        for declaration in &self.audio_elements {
            self.codec_config(declaration.codec_config)?;
            validate_element_topology(&declaration.element)?;
            validate_findings(declaration.element.validate())?;
            if declaration.substreams.len() != declaration.element.audio_substream_ids.len() {
                return Err(invalid("audio_substream_ids"));
            }
            for (position, handle) in declaration.substreams.iter().enumerate() {
                if handle.generation != self.generation
                    || self.substreams.get(handle.index).is_none()
                {
                    return Err(Error::new(
                        ErrorKind::UnknownSubstreamHandle,
                        Location::Unlocated,
                    ));
                }
                if declaration
                    .substreams
                    .iter()
                    .take(position)
                    .any(|other| other == handle)
                {
                    return Err(Error::new(
                        ErrorKind::DuplicateDeclaration,
                        Location::Field("audio_substream_id"),
                    ));
                }
                for other in &self.audio_elements {
                    for (other_position, other_handle) in other.substreams.iter().enumerate() {
                        if other_handle == handle
                            && (other.codec_config != declaration.codec_config
                                || substream_channels(&other.element, other_position)
                                    != substream_channels(&declaration.element, position))
                        {
                            return Err(invalid("shared_substream"));
                        }
                    }
                }
            }
        }
        for declaration in &self.mix_presentations {
            let expected_references: usize = declaration
                .presentation
                .sub_mixes
                .iter()
                .map(|sub_mix| sub_mix.elements.len())
                .sum();
            if expected_references != declaration.audio_elements.len() {
                return Err(Error::new(
                    ErrorKind::InvalidDescriptorReference,
                    Location::Field("mix_presentation.audio_elements"),
                ));
            }
            for handle in &declaration.audio_elements {
                self.audio_element(*handle)?;
            }
            validate_findings(declaration.presentation.validate())?;
            for sub_mix in &declaration.presentation.sub_mixes {
                for layout in &sub_mix.layouts {
                    use crate::model::layout::SoundSystem;
                    use crate::obu::Layout;
                    if matches!(
                        layout.layout,
                        Layout::Reserved(_) | Layout::SoundSystem(SoundSystem::Reserved(_))
                    ) {
                        return Err(Error::new(
                            ErrorKind::UnsupportedLayout,
                            Location::Field("sub_mix.layouts"),
                        ));
                    }
                    if let Some(extension) = &layout.loudness.extension {
                        if extension.info_type_bits & 0xfc == 0
                            || extension.info_type_bits & 0x03 != 0
                        {
                            return Err(invalid("loudness.extension.info_type_bits"));
                        }
                    }
                }
            }
        }

        for index in 0..self.substreams.len() {
            if !self.audio_elements.iter().any(|declaration| {
                declaration
                    .substreams
                    .iter()
                    .any(|handle| handle.index == index)
            }) {
                return Err(invalid("unused_substream"));
            }
        }

        let descriptors = DescriptorSet {
            sequence_header: IaSequenceHeader::new(0, 0),
            codec_configs: self.codec_configs.clone(),
            audio_elements: self
                .audio_elements
                .iter()
                .map(|declaration| declaration.element.clone())
                .collect(),
            mix_presentations: self
                .mix_presentations
                .iter()
                .map(|declaration| declaration.presentation.clone())
                .collect(),
        };
        let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)?;
        for entry in registry.entries() {
            validate_parameter_definition(&entry.definition)?;
        }
        if self
            .audio_elements
            .iter()
            .any(|declaration| !declaration.element.params.is_empty())
        {
            return Err(Error::new(
                ErrorKind::UnsupportedParameterData,
                Location::Field("audio_element_params"),
            ));
        }
        // Exercise every exact wire encoder before allocating the manifest or
        // accepting a sink. Lowering only replaces validated ids afterward.
        crate::model::write_descriptors(&mut crate::bits::BitWriter::new(), &descriptors)?;
        Ok(())
    }

    fn resolve_parameter_references(&mut self) -> Result<()> {
        let mut used_parameters = Vec::new();
        for declaration in &mut self.mix_presentations {
            let gains = declaration
                .presentation
                .sub_mixes
                .iter_mut()
                .flat_map(|sub_mix| {
                    sub_mix
                        .elements
                        .iter_mut()
                        .map(|element| &mut element.element_mix_gain)
                        .chain(core::iter::once(&mut sub_mix.output_mix_gain))
                });
            let expected = gains.count();
            if declaration.parameters.len() != expected {
                return Err(invalid("mix_presentation.parameters"));
            }
            let gains = declaration
                .presentation
                .sub_mixes
                .iter_mut()
                .flat_map(|sub_mix| {
                    sub_mix
                        .elements
                        .iter_mut()
                        .map(|element| &mut element.element_mix_gain)
                        .chain(core::iter::once(&mut sub_mix.output_mix_gain))
                });
            for (gain, handle) in gains.zip(&declaration.parameters) {
                let parameter = self
                    .parameters
                    .get(handle.index)
                    .filter(|_| handle.generation == self.generation)
                    .ok_or_else(|| {
                        Error::new(ErrorKind::UnknownParameterHandle, Location::Unlocated)
                    })?;
                if used_parameters.contains(handle) {
                    return Err(Error::new(
                        ErrorKind::DuplicateDeclaration,
                        Location::Field("parameter_id"),
                    ));
                }
                used_parameters.push(*handle);
                *gain = parameter.gain.clone();
            }
        }
        if used_parameters.len() != self.parameters.len() {
            return Err(invalid("unused_parameter"));
        }
        Ok(())
    }

    fn codec_config(&self, handle: CodecConfigHandle) -> Result<&CodecConfig> {
        if handle.generation != self.generation {
            return Err(Error::new(
                ErrorKind::UnknownCodecConfigHandle,
                Location::Unlocated,
            ));
        }
        self.codec_configs
            .get(handle.index)
            .ok_or_else(|| Error::new(ErrorKind::UnknownCodecConfigHandle, Location::Unlocated))
    }

    fn audio_element(&self, handle: AudioElementHandle) -> Result<&AudioElementDeclaration> {
        if handle.generation != self.generation {
            return Err(Error::new(
                ErrorKind::UnknownAudioElementHandle,
                Location::Unlocated,
            ));
        }
        self.audio_elements
            .get(handle.index)
            .ok_or_else(|| Error::new(ErrorKind::UnknownAudioElementHandle, Location::Unlocated))
    }
}

/// Select a sequence profile from Presentation-local minima.
///
/// Audio Elements are resolved once for each sub-mix reference, so a shared
/// declaration contributes to every Presentation that uses it. Presentations
/// are never unioned: their concurrent-element and channel limits apply one
/// Presentation at a time, and the IA Sequence Header receives the highest
/// resulting pair.
fn select_sequence_profile(
    presentations: &[MixPresentation],
    elements: &[AudioElement],
) -> Result<(Profile, Profile)> {
    let mut selected = (Profile::Simple, Profile::Simple);

    for presentation in presentations {
        let presentation_elements: Result<Vec<&AudioElement>> = presentation
            .sub_mixes
            .iter()
            .flat_map(|sub_mix| &sub_mix.elements)
            .map(|reference| {
                let index = usize::try_from(reference.audio_element_id).map_err(|_| {
                    Error::new(
                        ErrorKind::InvalidDescriptorReference,
                        Location::Field("mix_presentation.audio_elements"),
                    )
                })?;
                elements.get(index).ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidDescriptorReference,
                        Location::Field("mix_presentation.audio_elements"),
                    )
                })
            })
            .collect();
        let candidate = select_minimum_profile(&presentation_elements?)?;
        if candidate.0 > selected.0 {
            selected = candidate;
        }
    }

    Ok(selected)
}

impl Default for EncoderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_findings(findings: Vec<crate::error::Finding>) -> Result<()> {
    if findings.is_empty() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::InvalidDescriptorReference,
            Location::Field("descriptors"),
        ))
    }
}

fn allocate_ids<H>(length: usize, make_handle: impl Fn(usize) -> H) -> Result<Vec<(H, u32)>> {
    (0..length)
        .map(|index| {
            u32::try_from(index)
                .map(|id| (make_handle(index), id))
                .map_err(allocation_error)
        })
        .collect()
}

fn allocation_error(_: core::num::TryFromIntError) -> Error {
    Error::new(ErrorKind::WireIdAllocationExhausted, Location::Unlocated)
}

fn invalid(field: &'static str) -> Error {
    Error::new(
        ErrorKind::InvalidDescriptorReference,
        Location::Field(field),
    )
}

fn parameter_wire_id(handle: Option<ParameterHandle>) -> Result<u32> {
    u32::try_from(
        handle
            .ok_or_else(|| invalid("mix_presentation.parameters"))?
            .index,
    )
    .map_err(allocation_error)
}

fn validate_implicit_ids(
    ids: impl Iterator<Item = Option<u32>>,
    field: &'static str,
) -> Result<()> {
    let mut seen = Vec::new();
    for id in ids.flatten() {
        if seen.contains(&id) {
            return Err(Error::new(
                ErrorKind::DuplicateDeclaration,
                Location::Field(field),
            ));
        }
        seen.push(id);
    }
    Ok(())
}

fn substream_channels(element: &AudioElement, position: usize) -> u8 {
    match &element.audio_element_type {
        AudioElementType::ChannelBased(config)
            if config
                .scalable_channel_layout
                .layers
                .first()
                .is_some_and(|layer| position < usize::from(layer.coupled_substream_count)) =>
        {
            2
        }
        _ => 1,
    }
}

fn validate_element_topology(element: &AudioElement) -> Result<()> {
    match &element.audio_element_type {
        AudioElementType::ChannelBased(config) => {
            let layers = &config.scalable_channel_layout.layers;
            for layer in layers {
                if layer.loudspeaker_layout.channel_count().is_none() {
                    return Err(Error::new(
                        ErrorKind::UnsupportedLayout,
                        Location::Field("loudspeaker_layout"),
                    ));
                }
            }
            let [layer] = layers.as_slice() else {
                return Err(invalid("num_layers"));
            };
            if layer.recon_gain_is_present {
                return Err(Error::new(
                    ErrorKind::UnsupportedParameterData,
                    Location::Field("recon_gain_is_present"),
                ));
            }
            if layer.coupled_substream_count > layer.substream_count
                || layer.loudspeaker_layout.channel_count()
                    != Some(
                        u32::from(layer.substream_count)
                            .saturating_add(u32::from(layer.coupled_substream_count)),
                    )
            {
                return Err(Error::new(
                    ErrorKind::ChannelCountMismatch,
                    Location::Field("substream_count"),
                ));
            }
        }
        AudioElementType::SceneBased(AmbisonicsConfig::Mono(config)) => {
            let channels = config.output_channel_count;
            if !(1_u8..=15).any(|root| root.checked_mul(root) == Some(channels))
                || config.substream_count == 0
                || config.substream_count > channels
                || usize::from(config.substream_count) != element.audio_substream_ids.len()
                || config.channel_mapping.len() != usize::from(channels)
                || config
                    .channel_mapping
                    .iter()
                    .any(|mapping| *mapping != 255 && *mapping >= config.substream_count)
                || (0..config.substream_count)
                    .any(|stream| !config.channel_mapping.contains(&stream))
            {
                return Err(invalid("ambisonics_mono_config"));
            }
        }
        _ => return Err(invalid("audio_element_type")),
    }
    Ok(())
}

fn validate_parameter_definition(definition: &crate::obu::ParamDefinition) -> Result<()> {
    if definition.parameter_rate == 0 {
        return Err(invalid("parameter_rate"));
    }
    if let Some(fields) = &definition.duration_fields {
        let valid = fields.duration != 0
            && if fields.constant_subblock_duration == 0 {
                !fields.subblock_durations.is_empty()
                    && !fields.subblock_durations.contains(&0)
                    && fields
                        .subblock_durations
                        .iter()
                        .try_fold(0_u32, |sum, duration| sum.checked_add(*duration))
                        == Some(fields.duration)
            } else {
                fields.constant_subblock_duration <= fields.duration
                    && fields.subblock_durations.is_empty()
            };
        if !valid {
            return Err(Error::new(
                ErrorKind::SubblockDurationMismatch,
                Location::Field("duration_fields"),
            ));
        }
    }
    Ok(())
}
