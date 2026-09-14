# 260914-5c5 deferred items

1. **Other reserved aliases not probed.** `LoudspeakerLayout::Reserved`, `ExpandedLoudspeakerLayout::Reserved`,
   `SoundSystem::Reserved` and `HeadphonesRenderingMode::Reserved` may hold defined values and write
   non-round-tripping bytes (research A3, ASSUMED). A follow-up quick task should probe them and extend
   `tests/writer_round_trip.rs`.
2. **Audio Element `param_definition_type` 0.** Spec `index.bs:680-689` gives it no size field. iamf-tools@v2.1.0
   rejects it on read (`audio_element.cc:393-396`) and write (`ValidateAndWriteAudioElementParam`). libiamf@v1.1.0
   and this crate read size plus bytes. The crate diagnoses it in `validate()` and keeps it writable for byte-exact
   re-emission (a `// NOTE:` in `write_audio_element_param` records the disagreement). Refusing it needs a
   reference-hash check and a user decision.
3. **Infallible signatures kept fallible.** `ParamDefinitionRegistry::observe_audio_element`, `from_descriptors` and
   `ParsedSequence::from_parts` can no longer fail but keep `-> Result`. The dead `is_err()` / `if let Ok` branches in
   `src/sequence.rs` and `src/model/mod.rs` remain. Parallax usage was not observed (A2). Simplifying them is a
   breaking API change.
4. **`SceneBased` is mutable through `AudioElementType::SceneBased { 0: c, .. }`.** It is not constructible outside
   the crate, but a caller can rewrite a parsed config. Task 2's guards make such edits fail on write instead of
   writing mis-framed bytes. No action unless the Parallax contract should mention it.
5. **Memory claim unmeasured (A4).** Removing parse-time Raw subblocks for extension-governed blocks was reasoned, not
   measured. It belongs with the per-parse budget decision (260914-1mq deferred item 1).
6. **Fuzz workspace build.** Gate 10 ran locally and passed:
   `cargo +nightly-2026-09-01 check --locked --manifest-path fuzz/Cargo.toml --features roundtrip-model --bins`
   exited 0 (`target/5c5-fuzz-ws.log`). No CI dependency for this item.
7. **Low-level writers may still leave partial bytes in a caller's `BitWriter` on error (new).** The new guards sit
   at the top of their owning writer as planned, but some owners are nested (`write_ambisonics_config`,
   `write_audio_element_param`, `write_layout_with_loudness`, `write_loudness`, and `write_param_definition` when
   called from an Audio Element or Mix Presentation). By the time they run, the enclosing public writer has already
   emitted earlier fields. This matches the existing `ValueExceedsWidth` behaviour. `write_obu_with` and the sequence
   writers use scratch buffers, so no sink sees a partial OBU. A pre-pass validator at the top of each public writer
   would make direct low-level calls transactional as well.
8. **Proptest regression files (new).** RED runs of `round_trip` and `writer_round_trip` created
   `tests/*.proptest-regressions`. They were deleted as run artifacts. The repo has no `.gitignore` rule for them, so a
   future failing proptest run will leave them untracked again.
