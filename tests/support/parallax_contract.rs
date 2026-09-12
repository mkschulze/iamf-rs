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

#[derive(Debug, Clone, Copy)]
enum CandidateKind {
    LpcmStereo,
    FlacStereo,
    OpusStereo,
    AmbisonicsMono,
}

/// Bytes and stable observables produced by the filtered public authoring path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryArtifact {
    pub bytes: Vec<u8>,
    pub retained_names: Vec<Vec<u8>>,
    pub excluded_names: Vec<Vec<u8>>,
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
    let flac = builder.add_codec_config(CodecConfig::flac(89, 128, 48_000, 16)?);
    let opus = builder.add_codec_config(CodecConfig::opus(90, 960, 48_000, 312)?);

    let mut lpcm_element = None;
    let mut flac_element = None;
    let mut opus_element = None;
    let mut ambisonics_element = None;
    for candidate in retained.iter().copied() {
        let element = match candidate.kind {
            CandidateKind::LpcmStereo => builder.add_audio_element(lpcm, stereo_element(10)),
            CandidateKind::FlacStereo => builder.add_audio_element(flac, stereo_element(20)),
            CandidateKind::OpusStereo => builder.add_audio_element(opus, stereo_element(30)),
            CandidateKind::AmbisonicsMono => {
                let streams = (0..4).map(|_| builder.add_substream()).collect();
                builder.add_ambisonics_mono(
                    lpcm,
                    streams,
                    AmbisonicsMonoConfig {
                        output_channel_count: 4,
                        substream_count: 4,
                        channel_mapping: vec![0, 1, 2, 3],
                    },
                )
            }
        };
        match candidate.kind {
            CandidateKind::LpcmStereo => lpcm_element = Some(element),
            CandidateKind::FlacStereo => flac_element = Some(element),
            CandidateKind::OpusStereo => opus_element = Some(element),
            CandidateKind::AmbisonicsMono => ambisonics_element = Some(element),
        }
    }
    let lpcm_element = lpcm_element.expect("included LPCM candidate");
    let flac_element = flac_element.expect("included FLAC candidate");
    let opus_element = opus_element.expect("included Opus candidate");
    let ambisonics_element = ambisonics_element.expect("included Ambisonics candidate");

    let primary = builder.add_mix_presentation(
        vec![lpcm_element, flac_element, opus_element, ambisonics_element],
        presentation(
            b"Parallax primary",
            &[b"program", b"archive", b"stream", b"bed"],
            10,
        ),
    );
    let alternate = builder.add_mix_presentation(
        vec![lpcm_element],
        presentation(b"Parallax alternate", &[b"program"], 30),
    );
    let (encoder, manifest) = builder.build()?;

    let lpcm_stream = only_substream(&manifest, lpcm_element);
    let flac_stream = only_substream(&manifest, flac_element);
    let opus_stream = only_substream(&manifest, opus_element);
    let ambisonics_streams = manifest
        .substreams(ambisonics_element)
        .expect("ambisonics manifest entry")
        .to_vec();
    let mut parameters = manifest
        .parameters(primary)
        .expect("primary presentation parameters")
        .to_vec();
    parameters.extend_from_slice(
        manifest
            .parameters(alternate)
            .expect("alternate presentation parameters"),
    );
    let parameter = parameters.first().copied().expect("supplied gain handle");
    let parameter_id = manifest.parameter_id(parameter).expect("supplied gain id");

    // These are committed opaque access units. The fixture never encodes FLAC
    // or Opus; it only submits their existing corpus packets through the
    // public streaming boundary.
    let flac_payload = include_bytes!("../fixtures/codecs/flac/packet-000.bin").to_vec();
    let opus_payload = include_bytes!("../fixtures/codecs/opus/packet-000.bin").to_vec();
    let mut frames = vec![
        (lpcm_stream, FrameInput::Lpcm(vec![0; 512])),
        (flac_stream, FrameInput::Flac(flac_payload)),
        (opus_stream, FrameInput::Opus(opus_payload)),
    ];
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

    Ok(DeliveryArtifact {
        bytes: writer.finish()?,
        retained_names: retained
            .iter()
            .map(|candidate| candidate.name.to_vec())
            .collect(),
        excluded_names,
    })
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
