//! Phase 2 sequence-parser context tests.

#[path = "support/test_000003.rs"]
mod support;

use iamf::model::DescriptorSet;
use iamf::model::layout::LoudspeakerLayout;
use iamf::obu::{
    AudioElementParam, AudioElementType, ChannelAudioLayerConfig, ParamDefinition,
    ParamDefinitionRegistry, ParameterDataContext,
};
use support::published_descriptor_set;

fn definition(parameter_id: u32) -> ParamDefinition {
    ParamDefinition::mode_1(parameter_id, 48_000)
}

#[test]
fn an_empty_registry_has_no_parameter_definition() {
    let registry = ParamDefinitionRegistry::default();

    assert!(registry.get(7).is_none());
    assert!(registry.is_empty());
}

#[test]
fn duplicate_parameter_ids_are_retained_and_lookup_binds_to_the_first() {
    let mut registry = ParamDefinitionRegistry::default();
    registry.register(definition(7), ParameterDataContext::MixGain);
    registry.register(definition(7), ParameterDataContext::Demixing);

    assert_eq!(registry.len(), 2, "duplicates remain observable");
    assert_eq!(
        registry.get(7).map(|entry| &entry.context),
        Some(&ParameterDataContext::MixGain),
        "lookup follows the first definition in wire order"
    );
}

#[test]
fn descriptor_observation_preserves_nested_wire_order() {
    let mut descriptors = published_descriptor_set();
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![
        AudioElementParam::Demixing {
            definition: definition(11),
            default_dmixp_mode: 0,
            default_reserved: 0,
            default_w: 0,
            default_w_reserved: 0,
        },
        AudioElementParam::ReconGain {
            definition: definition(12),
        },
    ];

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the descriptor definitions are structurally readable");
    let ids: Vec<u32> = registry
        .entries()
        .iter()
        .map(|entry| entry.definition.parameter_id)
        .collect();

    assert_eq!(ids, vec![11, 12, 100, 100]);
}

#[test]
fn registry_lookup_follows_the_audio_element_order_emitted_by_the_writer() {
    let mut later = support::published_audio_element().payload;
    later.audio_element_id = 20;
    later.params = vec![AudioElementParam::Demixing {
        definition: definition(55),
        default_dmixp_mode: 0,
        default_reserved: 0,
        default_w: 0,
        default_w_reserved: 0,
    }];
    let mut earlier = later.clone();
    earlier.audio_element_id = 10;
    earlier.params = vec![AudioElementParam::ReconGain {
        definition: definition(55),
    }];
    let mut descriptors = published_descriptor_set();
    descriptors.audio_elements = vec![later, earlier];
    descriptors.mix_presentations.clear();

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the definitions are structurally readable");

    assert_eq!(
        registry.get(55).map(|entry| &entry.context),
        Some(&ParameterDataContext::ReconGain {
            recon_gain_is_present: vec![false],
        }),
        "Audio Element 10 is emitted before Audio Element 20"
    );
}

#[test]
fn an_extension_definition_registers_its_shared_prefix_and_reserved_context() {
    let mut descriptors = published_descriptor_set();
    // ParamDefinition { id: 7, rate: 1, mode: 1 }, followed by extension data.
    descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element")
        .params = vec![AudioElementParam::Extension {
        param_definition_type: 9,
        bytes: vec![0x07, 0x01, 0x80, 0xaa],
    }];

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the shared ParamDefinition prefix is present");
    let registered = registry.get(7).expect("extension parameter id 7");

    assert_eq!(registered.definition, ParamDefinition::mode_1(7, 1));
    assert_eq!(registered.context, ParameterDataContext::Reserved(9));
}

#[test]
fn recon_gain_context_has_exactly_one_presence_flag_per_channel_layer() {
    let mut descriptors = published_descriptor_set();
    let element = descriptors
        .audio_elements
        .first_mut()
        .expect("published fixture has one Audio Element");
    element.params = vec![AudioElementParam::ReconGain {
        definition: definition(12),
    }];
    let AudioElementType::ChannelBased(config) = &mut element.audio_element_type else {
        unreachable!("published fixture is channel based");
    };
    config.scalable_channel_layout.layers = vec![
        ChannelAudioLayerConfig::new(LoudspeakerLayout::Stereo, 1, 1),
        ChannelAudioLayerConfig {
            recon_gain_is_present: true,
            reserved: 0,
            ..ChannelAudioLayerConfig::new(LoudspeakerLayout::Ch5_1, 3, 1)
        },
    ];

    let registry = ParamDefinitionRegistry::from_descriptors(&descriptors)
        .expect("the descriptor definitions are structurally readable");

    assert_eq!(
        registry.get(12).map(|entry| &entry.context),
        Some(&ParameterDataContext::ReconGain {
            recon_gain_is_present: vec![false, true],
        })
    );
}

#[test]
fn descriptor_validation_reports_duplicate_nested_parameter_ids() {
    let descriptors: DescriptorSet = published_descriptor_set();

    assert!(descriptors.validate().iter().any(|finding| {
        finding.at == iamf::error::Location::Field("parameter_id")
            && finding.message.contains("parameter_id 100")
    }));
}
