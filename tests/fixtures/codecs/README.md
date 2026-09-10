# Committed codec framing fixtures

Root tests consume these artifacts offline. They never invoke a codec encoder,
regenerate packets, or overwrite expected PCM or manifests. The fixture tool is
a separate workspace at `tools/codec-fixtures`, explicitly excluded from the
root workspace. Neither root `cargo test --locked` nor root
`cargo test --locked -- --include-ignored` reaches that package, including its
ignored generators and independent decoder verifiers.

Immutable artifacts are `packet-*.bin`, PCM files (the committed files are
`source.s16le` and `expected.s16le`; also include `*.pcm` if added), and
`MANIFEST.md` below both codec directories. This README is editable documentation,
not a generated immutable artifact. Scratch output is not part of the inventory.

## Requirement-to-test matrix

Names below are exact test names; the suite column identifies the integration
target. Inventory them with `cargo test --locked -- --list` and
`cargo test --locked --features fuzzing --test fuzz_regression -- --list`.
Unless explicitly marked otherwise, these tests run at the root using committed
artifacts only.

| Requirement | Contract | Suite | Named tests |
| --- | --- | --- | --- |
| CODEC-01 | Hand-derived FLAC38 and every STREAMINFO field | `descriptors` | `canonical_flac_streaminfo_matches_the_hand_vector_and_exposes_every_field` |
| CODEC-01 | Short FLAC stays Raw; complete prefix owns exactly 38 bytes | `descriptors` | `a_thirty_seven_byte_flac_decoder_config_stays_raw_and_byte_exact`; `complete_flac_prefix_claims_only_thirty_eight_bytes_and_preserves_trailing` |
| CODEC-01 | Foreign fixed fields, rate, roll, and raw values survive read/write and produce findings | `descriptors` | `parsed_flac_contradictions_are_preserved_and_all_diagnosed` |
| CODEC-01 | Own typed FLAC model and byte round trip | `round_trip` | `typed_flac_codec_configs_round_trip` |
| CODEC-01 | All 0..37 bounded lengths remain Raw; 38 is typed; no following-OBU read-through; actual OBU truncation has an absolute error | `sequence_parse` | `every_flac_prefix_length_in_a_sequence_is_raw_until_38_and_truncation_is_positioned` |
| CODEC-02 | Hand-derived Opus11, big-endian fields, no Ogg header | `descriptors` | `canonical_opus_config_is_exactly_the_eleven_iamf_bytes` |
| CODEC-02 | Short Opus stays Raw; complete prefix owns exactly 11 bytes | `descriptors` | `a_ten_byte_opus_decoder_config_stays_raw_and_byte_exact`; `complete_opus_prefix_claims_only_eleven_bytes_and_preserves_trailing` |
| CODEC-02 | Own typed and foreign byte round trips | `round_trip` | `typed_opus_codec_configs_round_trip`; `foreign_typed_opus_config_round_trips_without_normalization` |
| CODEC-02, CODEC-05 | Foreign version, channels, pre-skip, rate, gain, mapping and roll are preserved and diagnosed | `descriptors` | `parsed_opus_contradictions_are_preserved_and_all_diagnosed` |
| CODEC-02 | All 0..10 bounded lengths remain Raw; 11 is typed; no following-OBU read-through; actual OBU truncation has an absolute error | `sequence_parse` | `every_opus_prefix_length_in_a_sequence_is_raw_until_11_and_truncation_is_positioned` |
| CODEC-01, CODEC-02 | Bounded legal typed codecs and 1..16 trailing bytes in whole streaming/parsed sequences | `round_trip` | `canonical_codec_sequences_round_trip` |
| CODEC-01, CODEC-02 | Stable corpus replay, typed codec enrichment, unknown/redundant/parameter shapes, and exact seed copies | `fuzz_regression` (feature `fuzzing`) | `parse_replay_covers_every_seed_and_minimized_artifact`; `roundtrip_replay_covers_every_seed_and_minimized_artifact`; `codec_enriched_fuzz_replay_preserves_typed_configs_trailing_and_sequence_shapes`; `pinned_positive_parse_seeds_are_exact_fixture_copies`; `documented_roundtrip_seed_shapes_are_present` |
| CODEC-03 | Checked `-ceil(3840/n)` at every pinned boundary, including zero rejection | `temporal` | `opus_roll_distance_matches_every_pinned_ceiling_boundary`; `opus_roll_distance_rejects_zero_before_dividing` |
| CODEC-03 | Constructor derives roll; foreign wrong roll survives and yields a finding | `descriptors` | `canonical_opus_config_is_exactly_the_eleven_iamf_bytes`; `parsed_opus_contradictions_are_preserved_and_all_diagnosed`; `parsed_flac_contradictions_are_preserved_and_all_diagnosed` |
| CODEC-04 | Unequal end/start trims retain END-before-START wire order | `temporal` | `unequal_opus_end_and_start_trim_are_written_end_before_start` |
| CODEC-04 | Measured Opus L and final E, exact packet order and stereo frame count | `codec_fixtures`; `fixture` | `opus_manifest_authenticates_arithmetic_packets_trims_and_exact_stereo_output`; `opus_fixture_keeps_manifest_packets_and_asymmetric_priming_on_the_wire` |
| CODEC-04 | FLAC final-only trim, opaque packets, exact 384-to-300 frame arithmetic | `codec_fixtures` | `flac_adapter_writes_exact_packets_and_only_final_end_trim`; `flac_three_headers_and_samples_prove_384_frames_then_trim_84_to_exact_300` |
| CODEC-05 | FLAC rate, frame-size and depth errors have exact kinds/locations; accepted lower block boundary | `descriptors` | `flac_constructor_rejects_exact_rate_frame_and_depth_boundaries`; `flac_constructor_accepts_sixteen_samples_without_validation_findings` |
| CODEC-05 | Opus 44.1 kHz typed error and compact public error surface | `error_shape` | `fresh_opus_rejects_44_1_khz_with_the_published_typed_error`; `codec_capability_errors_are_exact_compact_kinds` |
| CODEC-01, CODEC-06 | FLAC pinned tool versions, exact inventory/order, every packet and PCM SHA-256 | `codec_fixtures` | `flac_manifest_authenticates_exactly_three_packets_and_both_pcm_files` |
| CODEC-02, CODEC-06 | Opus pinned metadata, source/expected/packet digests, inventory/order; reject malformed provenance | `codec_fixtures` | `opus_manifest_authenticates_arithmetic_packets_trims_and_exact_stereo_output`; `opus_manifest_contract_rejects_missing_source_provenance_and_bad_metadata` |
| CODEC-01, CODEC-06 | Independent Claxon decode of committed FLAC packets equals original PCM | excluded tool `flac_fixtures` (explicit, ignored) | `verify_flac_corpus` |
| CODEC-02, CODEC-04, CODEC-06 | Fresh standalone Opus decoder consumes exact committed packets, trims L/E, equals expected PCM | excluded tool `opus_fixtures` (explicit, ignored) | `verify_opus_corpus` |
| CODEC-01, CODEC-02, CODEC-04 | Shared libiamf exact decode/strict-parser conformance gates | `conformance` (reference-dependent) | `the_flac_fixture_is_conformant`; `the_opus_fixture_is_conformant_to_its_standalone_decode` |
| CODEC-07 | Root manifest/lock/source excludes codec dependencies; workspace exclusion and import canaries | `codec_dependency_boundary` | `codec_dependencies_and_src_imports_remain_outside_the_root_crate`; `workspace_exclusion_canaries_are_limited_to_the_exclude_value`; `import_canaries_have_the_expected_polarity` |

The root FLAC sample check only decodes the committed fixed-block Verbatim
shape. Root Opus replay authenticates the committed standalone decode by digest;
it does not run an Opus decoder. Independent decoder verification uses the
explicit excluded-tool commands below. Reference conformance needs the pinned
local libiamf/iamf-tools setup and reports a skip when unavailable. A passing
offline test run does not claim that these external checks ran. CODEC-06 licence
and advisory evidence additionally comes from the isolated preflight and each
workspace's `cargo deny` policy, not from an audio round-trip assertion.

The additional property and fuzz strategies live in their integration tests.
They enrich existing canonical cases with legal typed codecs and bounded
trailing bytes while preserving original LPCM strategy behavior, seed bytes,
and the shared libFuzzer input format. Every existing model seed is still
replayed unchanged, then also replayed with each typed codec.

## Explicit regeneration and verification

The excluded tool pins `flacenc = 0.5.1` (default features disabled),
`claxon = 0.4.3`, `opus = 0.4.0`, and `sha2 = 0.10.9`.
The locked Opus backend is `opusic-sys = 0.7.5`, bundling `libopus 1.6.1`.
Both packages declare edition 2024 and MSRV Rust 1.85. Use the excluded tool's
own lockfile and licence policy; codec dependencies never belong in the root
manifest/lock. The manifests record codec settings, packet order, sample counts,
measured lookahead, trims and source provenance.

Only regenerate as an intentional artifact update. From the repository root,
the exact generation commands are:

```sh
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact generate_flac_corpus
CODEC_FIXTURE_OUTPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact generate_opus_corpus
```

Verify the committed or intentionally regenerated corpus without regenerating:

```sh
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/flac cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test flac_fixtures -- --ignored --exact verify_flac_corpus
CODEC_FIXTURE_INPUT=tests/fixtures/codecs/opus cargo test --locked --manifest-path tools/codec-fixtures/Cargo.toml --test opus_fixtures -- --ignored --exact verify_opus_corpus
```

Set `CARGO_NET_OFFLINE=true` when dependencies are already cached. Generation
may require the excluded tool's C/CMake build prerequisites for bundled Opus;
ordinary root replay does not. Review the tool versions and lockfile before an
intentional generation run. Afterwards:

1. Run both independent verifiers above. Check packet inventory/order and all
   packet/source/expected SHA-256 values against each manifest; review L, S, P,
   E, padded frame counts and channel order. FLAC expected PCM must equal source
   PCM; Opus expected PCM must equal the standalone decode of those exact packets.
2. Run `cargo test --locked --test codec_fixtures` for the root artifact contract,
   and the named descriptor/property/parser/fuzz tests for framing regressions.
3. Inspect `git diff --binary -- tests/fixtures/codecs` alongside the textual
   manifests. Review every changed packet, PCM file, digest, version and setting;
   do not accept binary or expected-output changes solely because tests pass.
4. Run the root dependency boundary test and the excluded tool's scoped licence
   policy. Retain the pinned reference conformance evidence for an artifact
   update. Commit reviewed artifacts and their manifests together.
