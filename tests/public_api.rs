#[test]
fn encoder_public_surface_is_available_without_default_features() {
    use iamf::encoder::{EncoderBuilder, FrameInput};
    use iamf::obu::CodecConfig;

    let _ = EncoderBuilder::new();
    let _ = CodecConfig::aac_lc(7, 48_000);
    let _ = FrameInput::AacLc(vec![0x21]);
}
