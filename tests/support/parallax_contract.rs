//! Public-API-only delivery fixture used by the Parallax contract tests.
//!
//! This is intentionally a test adapter, not an application-facing Parallax
//! type.  The candidate filter runs before this module lowers anything into
//! `iamf::encoder` declarations.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test-only contract fixture invariants should stop the test loudly"
)]

use iamf::encoder::{EncoderBuilder, FrameInput, SubmittedParameterBlock, TemporalUnitInput};
use iamf::model::layout::{AmbisonicsMonoConfig, LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AudioElement, BlockDurationFields, ChannelAudioLayerConfig, CodecConfig, Layout,
    LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition, MixGainParameterData,
    MixPresentation, ParameterBlock, ParameterData, ParameterSubblock, RenderingConfig,
    SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
};

/// Local delivery metadata which deliberately does not enter the encoder API.
#[derive(Debug, Clone, Copy)]
pub struct DeliveryCandidate {
    pub name: &'static [u8],
    pub included: bool,
    pub draft: bool,
    kind: CandidateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateKind {
    LpcmStereo,
    FlacStereo,
    OpusStereo,
    AacLcStereo,
    AmbisonicsMono,
}

/// Bytes and stable observables produced by the filtered public authoring path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryArtifact {
    pub deliveries: Vec<DeliveryFile>,
    pub retained_names: Vec<Vec<u8>>,
    pub excluded_names: Vec<Vec<u8>>,
}

/// One independently decodable IAMF delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryFile {
    pub name: Vec<u8>,
    pub bytes: Vec<u8>,
}

/// Construct the delivery from eligible candidates only.
pub fn build_delivery() -> iamf::Result<DeliveryArtifact> {
    let candidates = [
        DeliveryCandidate {
            name: b"program stereo",
            included: true,
            draft: false,
            kind: CandidateKind::LpcmStereo,
        },
        DeliveryCandidate {
            name: b"flac archive",
            included: true,
            draft: false,
            kind: CandidateKind::FlacStereo,
        },
        DeliveryCandidate {
            name: b"opus stream",
            included: true,
            draft: false,
            kind: CandidateKind::OpusStereo,
        },
        DeliveryCandidate {
            name: b"aac-lc stream",
            included: true,
            draft: false,
            kind: CandidateKind::AacLcStereo,
        },
        DeliveryCandidate {
            name: b"ambisonics bed",
            included: true,
            draft: false,
            kind: CandidateKind::AmbisonicsMono,
        },
        DeliveryCandidate {
            name: b"excluded alternate",
            included: false,
            draft: false,
            kind: CandidateKind::LpcmStereo,
        },
        DeliveryCandidate {
            name: b"draft alternate",
            included: true,
            draft: true,
            kind: CandidateKind::LpcmStereo,
        },
    ];
    let retained: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.included && !candidate.draft)
        .collect();
    let excluded_names = candidates
        .iter()
        .filter(|candidate| !candidate.included || candidate.draft)
        .map(|candidate| candidate.name.to_vec())
        .collect();

    let includes = |kind| retained.iter().any(|candidate| candidate.kind == kind);
    let mut deliveries = Vec::new();
    if includes(CandidateKind::LpcmStereo) && includes(CandidateKind::AmbisonicsMono) {
        deliveries.push(primary_delivery()?);
    }
    if includes(CandidateKind::FlacStereo) {
        deliveries.push(archive_delivery()?);
    }
    if includes(CandidateKind::OpusStereo) {
        deliveries.push(stream_delivery()?);
    }
    if includes(CandidateKind::AacLcStereo) {
        deliveries.push(aac_lc_delivery()?);
    }

    Ok(DeliveryArtifact {
        deliveries,
        retained_names: retained
            .iter()
            .map(|candidate| candidate.name.to_vec())
            .collect(),
        excluded_names,
    })
}

fn primary_delivery() -> iamf::Result<DeliveryFile> {
    let mut builder = EncoderBuilder::new();
    let lpcm = builder.add_codec_config(CodecConfig::lpcm(
        88,
        128,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::LittleEndian,
            sample_size: 16,
            sample_rate: 48_000,
        },
    ));
    let lpcm_element = builder.add_audio_element(lpcm, stereo_element(10));
    let streams = (0..4).map(|_| builder.add_substream()).collect();
    let ambisonics_element = builder.add_ambisonics_mono(
        lpcm,
        streams,
        AmbisonicsMonoConfig {
            output_channel_count: 4,
            substream_count: 4,
            channel_mapping: vec![0, 1, 2, 3],
        },
    );

    let primary = builder.add_mix_presentation(
        vec![lpcm_element, ambisonics_element],
        presentation(b"Parallax primary", &[b"program", b"bed"], 10),
    );
    let (encoder, manifest) = builder.build()?;

    let lpcm_stream = only_substream(&manifest, lpcm_element);
    let ambisonics_streams = manifest
        .substreams(ambisonics_element)
        .expect("ambisonics manifest entry")
        .to_vec();
    let parameters = manifest
        .parameters(primary)
        .expect("primary presentation parameters");
    let parameter = parameters.first().copied().expect("supplied gain handle");
    let parameter_id = manifest.parameter_id(parameter).expect("supplied gain id");

    let mut frames = vec![(lpcm_stream, FrameInput::Lpcm(vec![0; 512]))];
    frames.extend(
        ambisonics_streams
            .into_iter()
            .map(|stream| (stream, FrameInput::Lpcm(vec![0; 256]))),
    );
    let mut writer = encoder.start(Vec::new())?;
    writer.push_temporal_unit(TemporalUnitInput {
        frames,
        parameter_blocks: vec![SubmittedParameterBlock {
            parameter,
            block: supplied_parameter_block(parameter_id),
        }],
        trimming: None,
    })?;

    Ok(DeliveryFile {
        name: b"Parallax primary".to_vec(),
        bytes: writer.finish()?,
    })
}

fn archive_delivery() -> iamf::Result<DeliveryFile> {
    let mut builder = EncoderBuilder::new();
    let flac = builder.add_codec_config(CodecConfig::flac(89, 128, 48_000, 16)?);
    let element = builder.add_audio_element(flac, stereo_element(20));
    builder.add_mix_presentation(
        vec![element],
        presentation(b"Parallax archive", &[b"archive"], 30),
    );
    let (encoder, manifest) = builder.build()?;
    let stream = only_substream(&manifest, element);
    let mut writer = encoder.start(Vec::new())?;
    writer.push_temporal_unit(TemporalUnitInput {
        frames: vec![(
            stream,
            FrameInput::Flac(include_bytes!("../fixtures/codecs/flac/packet-000.bin").to_vec()),
        )],
        parameter_blocks: Vec::new(),
        trimming: None,
    })?;
    Ok(DeliveryFile {
        name: b"Parallax archive".to_vec(),
        bytes: writer.finish()?,
    })
}

fn stream_delivery() -> iamf::Result<DeliveryFile> {
    let mut builder = EncoderBuilder::new();
    let opus = builder.add_codec_config(CodecConfig::opus(90, 960, 48_000, 312)?);
    let element = builder.add_audio_element(opus, stereo_element(30));
    builder.add_mix_presentation(
        vec![element],
        presentation(b"Parallax stream", &[b"stream"], 40),
    );
    let (encoder, manifest) = builder.build()?;
    let stream = only_substream(&manifest, element);
    let mut writer = encoder.start(Vec::new())?;
    writer.push_temporal_unit(TemporalUnitInput {
        frames: vec![(
            stream,
            FrameInput::Opus(include_bytes!("../fixtures/codecs/opus/packet-000.bin").to_vec()),
        )],
        parameter_blocks: Vec::new(),
        trimming: None,
    })?;
    Ok(DeliveryFile {
        name: b"Parallax stream".to_vec(),
        bytes: writer.finish()?,
    })
}

fn aac_lc_delivery() -> iamf::Result<DeliveryFile> {
    let mut builder = EncoderBuilder::new();
    let aac_lc = builder.add_codec_config(CodecConfig::aac_lc(91, 48_000)?);
    let element = builder.add_audio_element(aac_lc, stereo_element(40));
    builder.add_mix_presentation(
        vec![element],
        presentation(b"Parallax AAC-LC", &[b"aac-lc"], 50),
    );
    let (encoder, manifest) = builder.build()?;
    let stream = only_substream(&manifest, element);
    let mut writer = encoder.start(Vec::new())?;
    writer.push_temporal_unit(TemporalUnitInput {
        frames: vec![(stream, FrameInput::AacLc(pinned_aac_lc_access_unit()))],
        parameter_blocks: Vec::new(),
        trimming: None,
    })?;
    Ok(DeliveryFile {
        name: b"Parallax AAC-LC".to_vec(),
        bytes: writer.finish()?,
    })
}

fn pinned_aac_lc_access_unit() -> Vec<u8> {
    let sequence = iamf::sequence::parse_sequence(include_bytes!(
        "../fixtures/reference/test_000076_aac_lc.iamf"
    ))
    .expect("the pinned AAC-LC source vector parses");
    sequence
        .obus
        .into_iter()
        .find_map(|obu| match obu {
            iamf::sequence::SequenceObu::AudioFrame(frame) => Some(frame.payload.payload),
            _ => None,
        })
        .expect("the pinned AAC-LC source vector contains an access unit")
}

fn only_substream(
    manifest: &iamf::encoder::IdManifest,
    element: iamf::encoder::AudioElementHandle,
) -> iamf::encoder::SubstreamHandle {
    let streams = manifest
        .substreams(element)
        .expect("element manifest entry");
    assert_eq!(streams.len(), 1, "stereo fixture has one coded substream");
    streams[0]
}

fn stereo_element(substream_label: u32) -> AudioElement {
    AudioElement::channel_based(
        0,
        0,
        vec![substream_label],
        ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
            LoudspeakerLayout::Stereo,
            1,
            1,
        )),
    )
}

fn presentation(annotation: &[u8], labels: &[&[u8]], parameter_base: u32) -> MixPresentation {
    MixPresentation {
        mix_presentation_id: 0,
        annotations_language: vec![b"en".to_vec()],
        localized_presentation_annotations: vec![annotation.to_vec()],
        sub_mixes: vec![SubMix {
            elements: labels
                .iter()
                .enumerate()
                .map(|(index, label)| SubMixAudioElement {
                    audio_element_id: 0,
                    localized_element_annotations: vec![label.to_vec()],
                    rendering_config: RenderingConfig::stereo(),
                    element_mix_gain: MixGainParamDefinition::mode_1(
                        parameter_base
                            .checked_add(u32::try_from(index).expect("small parameter offset"))
                            .expect("small parameter id"),
                        48_000,
                    ),
                })
                .collect(),
            output_mix_gain: MixGainParamDefinition::mode_1(
                parameter_base
                    .checked_add(u32::try_from(labels.len()).expect("small parameter count"))
                    .expect("small output parameter id"),
                48_000,
            ),
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(-5888, -1408),
            }],
        }],
        trailing: Vec::new(),
    }
}

fn supplied_parameter_block(parameter_id: u32) -> ParameterBlock {
    ParameterBlock {
        parameter_id,
        duration_fields: Some(BlockDurationFields {
            duration: 128,
            constant_subblock_duration: 0,
        }),
        subblocks: vec![
            ParameterSubblock {
                subblock_duration: Some(64),
                data: ParameterData::MixGain(MixGainParameterData::Linear {
                    start_point_value: -256,
                    end_point_value: 128,
                }),
            },
            ParameterSubblock {
                subblock_duration: Some(64),
                data: ParameterData::MixGain(MixGainParameterData::Step {
                    start_point_value: 128,
                }),
            },
        ],
    }
}
