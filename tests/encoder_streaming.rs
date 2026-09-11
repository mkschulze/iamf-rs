//! Public streaming tests for the immutable high-level encoder.

use iamf::encoder::{EncoderBuilder, FrameInput, SubmittedParameterBlock, TemporalUnitInput};
use iamf::model::layout::{LoudspeakerLayout, SoundSystem};
use iamf::obu::{
    AudioElement, BlockDurationFields, ChannelAudioLayerConfig, CodecConfig, Layout,
    LayoutWithLoudness, Loudness, LpcmDecoderConfig, MixGainParamDefinition, MixGainParameterData,
    MixPresentation, ObuType, ParameterBlock, ParameterData, ParameterSubblock, RenderingConfig,
    SampleFormatFlags, ScalableChannelLayoutConfig, SubMix, SubMixAudioElement,
};
use iamf::sequence::{parse_sequence, SequenceObu};
use iamf::ErrorKind;
use std::io::{self, Write};

#[test]
fn invalid_complete_unit_writes_no_temporal_bytes() -> iamf::Result<()> {
    let (encoder, left, _right) = stereo_builder()?;
    let mut writer = encoder.start(Vec::new())?;
    let before = writer.bytes_written();

    let error = writer
        .push_temporal_unit(TemporalUnitInput {
            frames: vec![(left, FrameInput::Lpcm(stereo_pcm_frame()))],
            parameter_blocks: Vec::new(),
            trimming: None,
        })
        .unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::MissingTemporalSubstream);
    assert_eq!(writer.bytes_written(), before);
    Ok(())
}

#[test]
fn duplicate_frame_handle_writes_no_temporal_bytes() -> iamf::Result<()> {
    let (encoder, left, _right) = stereo_builder()?;
    let mut writer = encoder.start(Vec::new())?;
    let before = writer.bytes_written();

    let error = writer
        .push_temporal_unit(TemporalUnitInput {
            frames: vec![
                (left, FrameInput::Lpcm(stereo_pcm_frame())),
                (left, FrameInput::Lpcm(stereo_pcm_frame())),
                (_right, FrameInput::Lpcm(stereo_pcm_frame())),
            ],
            parameter_blocks: Vec::new(),
            trimming: None,
        })
        .unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::DuplicateTemporalSubstream);
    assert_eq!(writer.bytes_written(), before);
    Ok(())
}

#[test]
fn temporal_frames_follow_first_descriptor_substream_order() -> iamf::Result<()> {
    let (encoder, first_declared, second_declared) = reverse_substream_order_builder()?;
    let mut writer = encoder.start(Vec::new())?;

    writer.push_temporal_unit(TemporalUnitInput {
        frames: vec![
            (first_declared, FrameInput::Lpcm(stereo_pcm_frame())),
            (second_declared, FrameInput::Lpcm(stereo_pcm_frame())),
        ],
        parameter_blocks: Vec::new(),
        trimming: None,
    })?;

    Ok(())
}

#[test]
fn wrong_codec_frame_writes_no_temporal_bytes() -> iamf::Result<()> {
    let (encoder, left, right) = stereo_builder()?;
    let mut writer = encoder.start(Vec::new())?;
    let before = writer.bytes_written();

    let error = writer
        .push_temporal_unit(TemporalUnitInput {
            frames: vec![
                (left, FrameInput::Opus(vec![0xf8])),
                (right, FrameInput::Lpcm(stereo_pcm_frame())),
            ],
            parameter_blocks: Vec::new(),
            trimming: None,
        })
        .unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::FrameCodecMismatch);
    assert_eq!(writer.bytes_written(), before);
    Ok(())
}

#[test]
fn lpcm_flac_and_opus_inputs_follow_the_frozen_codec_kind() -> iamf::Result<()> {
    for result in [
        mono_builder(CodecConfig::lpcm(
            0,
            128,
            LpcmDecoderConfig {
                sample_format_flags: SampleFormatFlags::LittleEndian,
                sample_size: 16,
                sample_rate: 16_000,
            },
        )),
        mono_builder(CodecConfig::flac(0, 128, 16_000, 16)?),
        mono_builder(CodecConfig::opus(0, 960, 48_000, 312)?),
    ] {
        let (encoder, frame) = result?;
        let mut writer = encoder.start(Vec::new())?;
        let expected_payload = match &frame.1 {
            FrameInput::Lpcm(payload) | FrameInput::Flac(payload) | FrameInput::Opus(payload) => {
                payload.clone()
            }
        };
        writer.push_temporal_unit(TemporalUnitInput {
            frames: vec![(frame.0, frame.1)],
            parameter_blocks: Vec::new(),
            trimming: None,
        })?;
        let written = writer.finish()?;
        let frames: Vec<_> = parse_sequence(&written)?
            .obus
            .into_iter()
            .filter_map(|obu| match obu {
                SequenceObu::AudioFrame(frame) => Some(frame),
                _ => None,
            })
            .collect();
        let [frame] = frames.as_slice() else {
            panic!("a matching typed input writes exactly one Audio Frame OBU");
        };
        assert_eq!(frame.header.obu_type, ObuType::AudioFrameId0);
        assert_eq!(frame.payload.substream_id, 0);
        assert_eq!(frame.payload.payload, expected_payload);
    }
    Ok(())
}

#[test]
fn ungoverned_parameter_block_writes_no_temporal_bytes() -> iamf::Result<()> {
    let (encoder, left, right) = stereo_builder()?;
    let mut foreign = EncoderBuilder::new();
    let foreign_parameter =
        foreign.add_mix_gain_parameter(MixGainParamDefinition::mode_1(9, 16_000));
    let mut writer = encoder.start(Vec::new())?;
    let before = writer.bytes_written();

    let error = writer
        .push_temporal_unit(TemporalUnitInput {
            frames: vec![
                (left, FrameInput::Lpcm(stereo_pcm_frame())),
                (right, FrameInput::Lpcm(stereo_pcm_frame())),
            ],
            parameter_blocks: vec![SubmittedParameterBlock {
                parameter: foreign_parameter,
                block: ParameterBlock {
                    parameter_id: 0,
                    duration_fields: None,
                    subblocks: Vec::new(),
                },
            }],
            trimming: None,
        })
        .unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::UnknownTemporalParameterHandle);
    assert_eq!(writer.bytes_written(), before);
    Ok(())
}

#[test]
fn partial_sink_failure_poisoned_high_level_writer() -> iamf::Result<()> {
    #[derive(Debug)]
    struct PartialThenFail {
        accepted: usize,
        limit: usize,
    }

    impl Write for PartialThenFail {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.accepted == self.limit {
                return Err(io::Error::other("terminal failure"));
            }
            let remaining = self.limit.saturating_sub(self.accepted);
            let written = remaining.min(bytes.len());
            self.accepted = self.accepted.saturating_add(written);
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let (encoder, left, right) = stereo_builder()?;
    let prologue_len = encoder.clone().start(Vec::new())?.finish()?.len();
    let mut writer = encoder.start(PartialThenFail {
        accepted: 0,
        limit: prologue_len + 1,
    })?;

    let first = writer
        .push_temporal_unit(stereo_temporal_unit(left, right))
        .expect_err("a temporal write partially reaches the refusing sink");
    assert_eq!(first.kind(), &ErrorKind::SinkWrite);
    assert_eq!(
        writer.bytes_written(),
        u64::try_from(prologue_len + 1).expect("test sink limit fits in u64")
    );

    let retry = writer
        .push_temporal_unit(stereo_temporal_unit(left, right))
        .expect_err("a poisoned high-level writer cannot emit another unit");
    assert_eq!(retry.kind(), &ErrorKind::SequenceWriterPoisoned);

    let finish = writer
        .finish()
        .expect_err("a poisoned high-level writer cannot return its sink");
    assert_eq!(finish.kind(), &ErrorKind::SequenceWriterPoisoned);
    Ok(())
}

#[test]
fn parameter_subblocks_must_tile_the_declared_duration() -> iamf::Result<()> {
    let (encoder, left, right, parameter) = stereo_builder_with_parameter()?;
    let mut writer = encoder.start(Vec::new())?;
    let before = writer.bytes_written();

    let error = writer
        .push_temporal_unit(TemporalUnitInput {
            frames: vec![
                (left, FrameInput::Lpcm(stereo_pcm_frame())),
                (right, FrameInput::Lpcm(stereo_pcm_frame())),
            ],
            parameter_blocks: vec![SubmittedParameterBlock {
                parameter,
                block: ParameterBlock {
                    parameter_id: 0,
                    duration_fields: Some(BlockDurationFields {
                        duration: 10,
                        constant_subblock_duration: 0,
                    }),
                    subblocks: vec![
                        ParameterSubblock {
                            subblock_duration: Some(4),
                            data: ParameterData::MixGain(MixGainParameterData::Step {
                                start_point_value: 0,
                            }),
                        },
                        ParameterSubblock {
                            subblock_duration: Some(5),
                            data: ParameterData::MixGain(MixGainParameterData::Step {
                                start_point_value: 0,
                            }),
                        },
                    ],
                },
            }],
            trimming: None,
        })
        .expect_err("subblock durations must exactly tile the declared duration");
    assert_eq!(error.kind(), &ErrorKind::SubblockDurationMismatch);
    assert_eq!(writer.bytes_written(), before);
    Ok(())
}

fn mono_builder(
    config: CodecConfig,
) -> iamf::Result<(
    iamf::encoder::Encoder,
    (iamf::encoder::SubstreamHandle, FrameInput),
)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(config.clone());
    let substream = builder.add_substream();
    builder.add_audio_element_with_substreams(
        codec,
        vec![substream],
        AudioElement::channel_based(
            0,
            0,
            vec![0],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Mono,
                1,
                0,
            )),
        ),
    );
    let input = match config.decoder_config {
        iamf::obu::DecoderConfig::Lpcm(_) => FrameInput::Lpcm(vec![0; 256]),
        iamf::obu::DecoderConfig::Flac(_) => FrameInput::Flac(vec![0]),
        iamf::obu::DecoderConfig::Opus(_) => FrameInput::Opus(vec![0xf8]),
        iamf::obu::DecoderConfig::Raw { .. } | _ => unreachable!(),
    };
    builder
        .build()
        .map(|(encoder, _)| (encoder, (substream, input)))
}

fn stereo_builder() -> iamf::Result<(
    iamf::encoder::Encoder,
    iamf::encoder::SubstreamHandle,
    iamf::encoder::SubstreamHandle,
)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(CodecConfig::lpcm(
        0,
        128,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::LittleEndian,
            sample_size: 16,
            sample_rate: 16_000,
        },
    ));
    let left = builder.add_substream();
    let right = builder.add_substream();
    let element = builder.add_audio_element_with_substreams(
        codec,
        vec![left, right],
        AudioElement::channel_based(
            0,
            0,
            vec![0, 1],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Stereo,
                2,
                0,
            )),
        ),
    );
    builder.add_mix_presentation(vec![element], presentation());
    builder.build().map(|(encoder, _)| (encoder, left, right))
}

fn stereo_builder_with_parameter() -> iamf::Result<(
    iamf::encoder::Encoder,
    iamf::encoder::SubstreamHandle,
    iamf::encoder::SubstreamHandle,
    iamf::encoder::ParameterHandle,
)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(CodecConfig::lpcm(
        0,
        128,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::LittleEndian,
            sample_size: 16,
            sample_rate: 16_000,
        },
    ));
    let left = builder.add_substream();
    let right = builder.add_substream();
    let element = builder.add_audio_element_with_substreams(
        codec,
        vec![left, right],
        AudioElement::channel_based(
            0,
            0,
            vec![0, 1],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Stereo,
                2,
                0,
            )),
        ),
    );
    let parameter = builder.add_mix_gain_parameter(MixGainParamDefinition::mode_1(0, 16_000));
    let output_parameter =
        builder.add_mix_gain_parameter(MixGainParamDefinition::mode_1(1, 16_000));
    builder.add_mix_presentation_with_parameters(
        vec![element],
        vec![parameter, output_parameter],
        presentation(),
    );
    builder
        .build()
        .map(|(encoder, _)| (encoder, left, right, parameter))
}

fn stereo_temporal_unit(
    left: iamf::encoder::SubstreamHandle,
    right: iamf::encoder::SubstreamHandle,
) -> TemporalUnitInput {
    TemporalUnitInput {
        frames: vec![
            (left, FrameInput::Lpcm(stereo_pcm_frame())),
            (right, FrameInput::Lpcm(stereo_pcm_frame())),
        ],
        parameter_blocks: Vec::new(),
        trimming: None,
    }
}

fn reverse_substream_order_builder() -> iamf::Result<(
    iamf::encoder::Encoder,
    iamf::encoder::SubstreamHandle,
    iamf::encoder::SubstreamHandle,
)> {
    let mut builder = EncoderBuilder::new();
    let codec = builder.add_codec_config(CodecConfig::lpcm(
        0,
        128,
        LpcmDecoderConfig {
            sample_format_flags: SampleFormatFlags::LittleEndian,
            sample_size: 16,
            sample_rate: 16_000,
        },
    ));
    let first_allocated = builder.add_substream();
    let second_allocated = builder.add_substream();
    let element = builder.add_audio_element_with_substreams(
        codec,
        vec![second_allocated, first_allocated],
        AudioElement::channel_based(
            0,
            0,
            vec![0, 1],
            ScalableChannelLayoutConfig::single_layer(ChannelAudioLayerConfig::new(
                LoudspeakerLayout::Stereo,
                2,
                0,
            )),
        ),
    );
    builder.add_mix_presentation(vec![element], presentation());
    builder
        .build()
        .map(|(encoder, _)| (encoder, second_allocated, first_allocated))
}

fn presentation() -> MixPresentation {
    MixPresentation {
        mix_presentation_id: 0,
        annotations_language: vec![b"en".to_vec()],
        localized_presentation_annotations: vec![b"stereo".to_vec()],
        sub_mixes: vec![SubMix {
            elements: vec![SubMixAudioElement {
                audio_element_id: 0,
                localized_element_annotations: vec![b"bed".to_vec()],
                rendering_config: RenderingConfig::stereo(),
                element_mix_gain: MixGainParamDefinition::mode_1(0, 16_000),
            }],
            output_mix_gain: MixGainParamDefinition::mode_1(1, 16_000),
            layouts: vec![LayoutWithLoudness {
                layout: Layout::SoundSystem(SoundSystem::A0_2_0),
                reserved: 0,
                loudness: Loudness::new(0, 0),
            }],
        }],
        trailing: Vec::new(),
    }
}

fn stereo_pcm_frame() -> Vec<u8> {
    vec![0; 256]
}
