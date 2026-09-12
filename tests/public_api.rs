#[test]
fn encoder_public_surface_is_available_without_default_features() {
    let _ = iamf::encoder::EncoderBuilder::new();
}
