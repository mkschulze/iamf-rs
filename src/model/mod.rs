//! The descriptor model — the layout vocabulary the OBUs share, and the
//! ordered descriptor set itself (DESC-08, DESC-09).
//!
//! # Why these collections are `Vec` and not `BTreeMap`
//!
//! Almost every IAMF collection is an **ordered sequence in the bitstream**,
//! not a map. Modelling one as a map makes the wire order a property of the
//! keys rather than of the data, which trades a determinism bug for a
//! round-trip bug: `BTreeMap` would reorder Mix Presentations, and Mix
//! Presentations are written in **list order** on purpose (see
//! [`write_descriptors`]). A `Vec` plus [`by_id`] keeps the wire order the
//! type's order, and a duplicate id becomes a reported Finding rather than the
//! silent overwrite a map would give.
//!
//! `HashMap` is banned outright by GUARD-02 — its iteration order is seeded per
//! process, so any hashed container whose order can reach the output silently
//! breaks the four-target byte-identity guarantee.

pub mod layout;
pub mod loudness;
pub mod profile;

pub use loudness::{Q7_8, lufs_to_q7_8};
pub use profile::{Profile, select_minimum_profile};

use crate::bits::BitWriter;
use crate::error::{Finding, Location, Result};
use crate::obu::{
    AudioElement, CodecConfig, IaSequenceHeader, MixPresentation, Obu, ObuHeader, ObuType,
    write_audio_element, write_codec_config, write_ia_sequence_header, write_mix_presentation,
    write_obu_with,
};

/// A descriptor that carries an id other descriptors reference.
pub trait Identified {
    /// The id, as it appears on the wire.
    fn id(&self) -> u32;
}

impl Identified for CodecConfig {
    fn id(&self) -> u32 {
        self.codec_config_id
    }
}

impl Identified for AudioElement {
    fn id(&self) -> u32 {
        self.audio_element_id
    }
}

impl Identified for MixPresentation {
    fn id(&self) -> u32 {
        self.mix_presentation_id
    }
}

/// The **first** descriptor in bitstream order carrying `id`, or `None`.
///
/// A linear scan, deliberately. The alternative — a map — would make a
/// duplicate id a silent overwrite, and the wire can carry duplicates: this
/// returns the first, which is what a decoder reading forward would bind to,
/// and `validate()` reports the duplicate by naming both indices.
///
/// Returns `None` on an empty slice.
#[must_use]
pub fn by_id<T: Identified>(items: &[T], id: u32) -> Option<&T> {
    items.iter().find(|item| item.id() == id)
}

/// Every descriptor OBU of one IA Sequence, in model form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorSet {
    /// The IA Sequence Header, always written first.
    pub sequence_header: IaSequenceHeader,
    /// Codec Configs. Written **ascending by `codec_config_id`**.
    pub codec_configs: Vec<CodecConfig>,
    /// Audio Elements. Written **ascending by `audio_element_id`**.
    pub audio_elements: Vec<AudioElement>,
    /// Mix Presentations. Written in **list order**, never sorted.
    pub mix_presentations: Vec<MixPresentation>,
}

impl DescriptorSet {
    /// An empty set carrying only its sequence header.
    #[must_use]
    pub const fn new(sequence_header: IaSequenceHeader) -> Self {
        Self {
            sequence_header,
            codec_configs: Vec::new(),
            audio_elements: Vec::new(),
            mix_presentations: Vec::new(),
        }
    }

    /// The first Codec Config with this id.
    #[must_use]
    pub fn codec_config_by_id(&self, id: u32) -> Option<&CodecConfig> {
        by_id(&self.codec_configs, id)
    }

    /// The first Audio Element with this id.
    #[must_use]
    pub fn audio_element_by_id(&self, id: u32) -> Option<&AudioElement> {
        by_id(&self.audio_elements, id)
    }

    /// The first Mix Presentation with this id.
    #[must_use]
    pub fn mix_presentation_by_id(&self, id: u32) -> Option<&MixPresentation> {
        by_id(&self.mix_presentations, id)
    }

    /// Every finding across the set: each descriptor's own, plus duplicate ids
    /// and unresolvable cross-references.
    ///
    /// Reference resolution is **forward-only and reported, never enforced**:
    /// the parser does not require a referent to have been seen already,
    /// because being stricter than the reference on read would break Phase 2's
    /// foreign-file round-trip.
    #[must_use]
    pub fn validate(&self) -> Vec<Finding> {
        let mut findings = self.sequence_header.validate();
        for config in &self.codec_configs {
            findings.extend(config.validate());
        }
        for element in &self.audio_elements {
            findings.extend(element.validate());
        }
        for presentation in &self.mix_presentations {
            findings.extend(presentation.validate());
        }

        findings.extend(duplicate_id_findings(&self.codec_configs, "codec_config_id"));
        findings.extend(duplicate_id_findings(&self.audio_elements, "audio_element_id"));
        findings.extend(duplicate_id_findings(
            &self.mix_presentations,
            "mix_presentation_id",
        ));

        if let Ok(registry) = crate::obu::ParamDefinitionRegistry::from_descriptors(self) {
            for (index, entry) in registry.entries().iter().enumerate() {
                for (other_index, other) in registry
                    .entries()
                    .iter()
                    .enumerate()
                    .skip(index.saturating_add(1))
                {
                    if entry.definition.parameter_id == other.definition.parameter_id {
                        findings.push(Finding {
                            at: Location::Field("parameter_id"),
                            message: format!(
                                "parameter_id {} appears at nested definition indices {index} and \
                                 {other_index}; lookup binds to index {index}, the first in \
                                 bitstream order",
                                entry.definition.parameter_id
                            ),
                        });
                    }
                }
            }
        }

        for element in &self.audio_elements {
            if self.codec_config_by_id(element.codec_config_id).is_none() {
                findings.push(Finding {
                    at: Location::Field("codec_config_id"),
                    message: format!(
                        "audio element {} references codec_config_id {}, which no Codec Config \
                         in this sequence carries",
                        element.audio_element_id, element.codec_config_id
                    ),
                });
            }
        }
        for presentation in &self.mix_presentations {
            for sub_mix in &presentation.sub_mixes {
                for element in &sub_mix.elements {
                    if self.audio_element_by_id(element.audio_element_id).is_none() {
                        findings.push(Finding {
                            at: Location::Field("audio_element_id"),
                            message: format!(
                                "mix presentation {} references audio_element_id {}, which no \
                                 Audio Element in this sequence carries",
                                presentation.mix_presentation_id, element.audio_element_id
                            ),
                        });
                    }
                }
            }
        }
        findings
    }
}

/// One Finding per duplicated id, naming **both** indices.
fn duplicate_id_findings<T: Identified>(items: &[T], field: &'static str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, item) in items.iter().enumerate() {
        for (other_index, other) in items.iter().enumerate().skip(index.saturating_add(1)) {
            if item.id() == other.id() {
                findings.push(Finding {
                    at: Location::Field(field),
                    message: format!(
                        "{field} {} appears at indices {index} and {other_index}; by_id() binds \
                         to index {index}, the first in bitstream order",
                        item.id()
                    ),
                });
            }
        }
    }
    findings
}

// ref: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc ObuSequencerBase::WriteDescriptorObus
// NOTE: Codec Configs and Audio Elements are sorted ascending by id
// (`SortedKeys(..., std::less<uint32_t>())`) and Mix Presentations are NOT.
// The reference's own comment gives the reason: *"Because the original
// ordering may be used downstream when selecting the mix presentation."*
// Sorting Mix Presentations by id would therefore be WRONG, and this is the
// exact place a future reader will "fix" the inconsistency. Do not.
/// Write the whole descriptor prologue in the reference's order: IA Sequence
/// Header, Codec Configs ascending by id, Audio Elements ascending by id, then
/// Mix Presentations in list order.
pub fn write_descriptors(w: &mut BitWriter, set: &DescriptorSet) -> Result<()> {
    write_obu_with(
        w,
        &Obu::new(
            ObuHeader::new(ObuType::IaSequenceHeader),
            set.sequence_header.clone(),
        ),
        write_ia_sequence_header,
    )?;

    for config in sorted_by_id(&set.codec_configs) {
        write_obu_with(
            w,
            &Obu::new(ObuHeader::new(ObuType::CodecConfig), config.clone()),
            write_codec_config,
        )?;
    }
    for element in sorted_by_id(&set.audio_elements) {
        write_obu_with(
            w,
            &Obu::new(ObuHeader::new(ObuType::AudioElement), element.clone()),
            write_audio_element,
        )?;
    }
    for presentation in &set.mix_presentations {
        write_obu_with(
            w,
            &Obu::new(
                ObuHeader::new(ObuType::MixPresentation),
                presentation.clone(),
            ),
            write_mix_presentation,
        )?;
    }
    Ok(())
}

/// The items, ascending by id, without disturbing the caller's `Vec`.
///
/// A **stable** sort, so two descriptors sharing an id keep their bitstream
/// order relative to one another — the same order `by_id` binds to.
fn sorted_by_id<T: Identified>(items: &[T]) -> Vec<&T> {
    let mut sorted: Vec<&T> = items.iter().collect();
    sorted.sort_by_key(|item| item.id());
    sorted
}
