//! Immutable high-level IAMF descriptor authoring.
//!
//! The builder owns caller-local handles and assigns all wire ids only after
//! static validation has accepted the complete declaration set.

use crate::error::{Error, ErrorKind, Location, Result};
use crate::model::DescriptorSet;
use crate::obu::{AudioElement, CodecConfig, IaSequenceHeader, MixPresentation};

/// A caller-local reference to a declared codec configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecConfigHandle(usize);

/// A caller-local reference to a declared audio element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioElementHandle(usize);

/// A caller-local reference to a declared mix presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixPresentationHandle(usize);

/// The deterministic mapping from caller-local handles to IAMF wire ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdManifest {
    codec_configs: Vec<(CodecConfigHandle, u32)>,
    audio_elements: Vec<(AudioElementHandle, u32)>,
    mix_presentations: Vec<(MixPresentationHandle, u32)>,
}

impl IdManifest {
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
}

fn lookup_id<H: Copy + PartialEq>(entries: &[(H, u32)], handle: H) -> Option<u32> {
    entries
        .iter()
        .find(|(candidate, _)| *candidate == handle)
        .map(|(_, id)| *id)
}

/// A completed, immutable descriptor configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoder {
    descriptors: DescriptorSet,
}

impl Encoder {
    /// The frozen descriptor model this encoder will write.
    #[must_use]
    pub const fn descriptors(&self) -> &DescriptorSet {
        &self.descriptors
    }
}

#[derive(Debug, Clone)]
struct AudioElementDeclaration {
    codec_config: CodecConfigHandle,
    element: AudioElement,
}

#[derive(Debug, Clone)]
struct MixPresentationDeclaration {
    audio_elements: Vec<AudioElementHandle>,
    presentation: MixPresentation,
}

/// Ordered static declarations waiting to be validated and frozen.
#[derive(Debug, Default)]
pub struct EncoderBuilder {
    codec_configs: Vec<CodecConfig>,
    audio_elements: Vec<AudioElementDeclaration>,
    mix_presentations: Vec<MixPresentationDeclaration>,
}

impl EncoderBuilder {
    /// Start an empty static configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            codec_configs: Vec::new(),
            audio_elements: Vec::new(),
            mix_presentations: Vec::new(),
        }
    }

    /// Declare a codec configuration.
    pub fn add_codec_config(&mut self, config: CodecConfig) -> CodecConfigHandle {
        let handle = CodecConfigHandle(self.codec_configs.len());
        self.codec_configs.push(config);
        handle
    }

    /// Declare an audio element referring to a previously declared codec configuration.
    pub fn add_audio_element(
        &mut self,
        codec_config: CodecConfigHandle,
        element: AudioElement,
    ) -> AudioElementHandle {
        let handle = AudioElementHandle(self.audio_elements.len());
        self.audio_elements.push(AudioElementDeclaration {
            codec_config,
            element,
        });
        handle
    }

    /// Declare a mix presentation and its ordered element references.
    pub fn add_mix_presentation(
        &mut self,
        audio_elements: Vec<AudioElementHandle>,
        presentation: MixPresentation,
    ) -> MixPresentationHandle {
        let handle = MixPresentationHandle(self.mix_presentations.len());
        self.mix_presentations.push(MixPresentationDeclaration {
            audio_elements,
            presentation,
        });
        handle
    }

    /// Validate every static declaration, allocate deterministic ids, and freeze it.
    pub fn build(self) -> Result<(Encoder, IdManifest)> {
        let manifest = IdManifest {
            codec_configs: allocate_ids(self.codec_configs.len(), CodecConfigHandle)?,
            audio_elements: allocate_ids(self.audio_elements.len(), AudioElementHandle)?,
            mix_presentations: allocate_ids(self.mix_presentations.len(), MixPresentationHandle)?,
        };

        let mut descriptors = DescriptorSet::new(IaSequenceHeader::new(0, 0));
        descriptors.codec_configs = self.codec_configs;
        for (index, config) in descriptors.codec_configs.iter_mut().enumerate() {
            config.codec_config_id = u32::try_from(index).map_err(allocation_error)?;
        }

        for (index, declaration) in self.audio_elements.into_iter().enumerate() {
            let codec_config_id = manifest
                .codec_config_id(declaration.codec_config)
                .ok_or_else(|| {
                    Error::new(ErrorKind::UnknownCodecConfigHandle, Location::Unlocated)
                })?;
            let mut element = declaration.element;
            element.audio_element_id = u32::try_from(index).map_err(allocation_error)?;
            element.codec_config_id = codec_config_id;
            descriptors.audio_elements.push(element);
        }

        for (index, declaration) in self.mix_presentations.into_iter().enumerate() {
            let mut presentation = declaration.presentation;
            let expected_references: usize = presentation
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
            let mut handles = declaration.audio_elements.into_iter();
            for sub_mix in &mut presentation.sub_mixes {
                for element in &mut sub_mix.elements {
                    let handle = handles.next().ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidDescriptorReference,
                            Location::Field("mix_presentation.audio_elements"),
                        )
                    })?;
                    element.audio_element_id =
                        manifest.audio_element_id(handle).ok_or_else(|| {
                            Error::new(ErrorKind::UnknownAudioElementHandle, Location::Unlocated)
                        })?;
                }
            }
            presentation.mix_presentation_id = u32::try_from(index).map_err(allocation_error)?;
            descriptors.mix_presentations.push(presentation);
        }

        if descriptors.validate().is_empty() {
            Ok((Encoder { descriptors }, manifest))
        } else {
            Err(Error::new(
                ErrorKind::InvalidDescriptorReference,
                Location::Field("descriptors"),
            ))
        }
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
