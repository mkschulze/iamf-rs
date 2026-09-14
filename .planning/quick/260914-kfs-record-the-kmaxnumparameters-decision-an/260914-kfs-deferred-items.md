# 260914-kfs deferred items

1. **Spec per-type `num_parameters` rule has no finding.** IAMF v1.1.0 `index.bs:752-753`: when
   `audio_element_type` = 0 (channel-based) `num_parameters` SHALL be 0, 1 or 2; when it is 1 or 2
   (scene-based) it SHALL be 0. `AudioElement::validate` does not report either today. Out of scope for the
   2026-09-14 user decision, which covers only the iamf-tools `kMaxNumParameters = 256` limit. A future
   finding would need its own decision on whether it also reaches `EncoderBuilder` (which already refuses every
   Audio Element param) and on its interaction with the `> 256` finding (both would fire at 257+ on a
   channel-based element, at the same `Field("num_parameters")` location).
