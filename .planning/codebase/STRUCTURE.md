# Codebase Structure

**Analysis Date:** 2026-09-13

## Directory Layout

```
iamf-rs/
├── src/                    # The `iamf` library crate
│   ├── lib.rs              # SPEC_VERSION, module list, error re-exports
│   ├── error.rs            # Error, ErrorKind, Location, Finding, Result
│   ├── bits/               # BitCursor, BitWriter, uleb128 (only bit-aware code)
│   ├── obu/                # One file per OBU type + header + framing + boundaries
│   ├── model/              # DescriptorSet, layout, loudness, profile
│   ├── sequence.rs         # parse_sequence, ParsedSequence, SequenceWriter
│   ├── encoder.rs          # EncoderBuilder, Encoder, EncodingWriter, IdManifest
│   ├── packing.rs          # Layout -> substream channel packing
│   ├── dump.rs             # Annotated human-readable OBU dump
│   └── fuzzing.rs          # Bounded arbitrary generator (feature "fuzzing")
├── tests/                  # Integration tests (one file per concern)
│   ├── support/            # Shared test helpers (included via #[path]/mod)
│   └── fixtures/           # golden/, reference/, codecs/ + MANIFEST.md
├── fuzz/                   # Excluded cargo-fuzz workspace (own Cargo.lock, deny.toml)
│   ├── fuzz_targets/       # parse_sequence.rs, obu_roundtrip.rs
│   └── corpus/             # Committed seed corpora per target
├── tools/                  # Shell scripts, Dockerfile, experiments
│   └── codec-fixtures/     # Excluded workspace: FLAC/Opus fixture generation
├── .github/workflows/      # ci.yml, fuzz.yml, reference.yml
├── .planning/              # GSD planning artifacts (phases, research, codebase)
├── .superpowers/sdd/       # SDD task reports
├── docs/                   # assets/ (tracked) + untracked upstream reference clones
├── Cargo.toml / Cargo.lock # Root package + lints
├── clippy.toml             # disallowed types/methods, test carve-outs
├── deny.toml               # Licence allow-list
├── rust-toolchain.toml     # Pinned toolchain
└── *.md                    # HANDOFF, REFERENCES, CONFORMANCE-GATE, DIFF-LEDGER, CONTRIBUTING, ...
```

## Directory Purposes

**`src/bits/`:**
- Purpose: Bit-level primitives; the only module permitted to handle bit positions.
- Key files: `src/bits/reader.rs`, `src/bits/writer.rs` (same method order), `src/bits/leb128.rs`, `src/bits/mod.rs`.

**`src/obu/`:**
- Purpose: Wire syntax per OBU. Submodules are private; everything public is re-exported from `src/obu/mod.rs`.
- Key files: `header.rs`, `sequence_header.rs`, `codec_config.rs`, `audio_element.rs`, `mix_presentation.rs`, `param_definition.rs`, `parameter_block.rs`, `audio_frame.rs`, `temporal_delimiter.rs`, `boundaries.rs`.

**`src/model/`:**
- Purpose: Cross-OBU semantics. `mod.rs` (DescriptorSet, validate, write_descriptors), `layout.rs`, `loudness.rs`, `profile.rs`.

**`tests/`:**
- Purpose: Integration tests against the public API, golden bytes, reference files, guards.
- Key files: `golden.rs`, `conformance.rs`, `parse_reference.rs`, `refcorpus.rs`, `round_trip.rs`, `sequence.rs`, `sequence_parse.rs`, `encoder_builder.rs`, `encoder_streaming.rs`, `bits_oracle.rs`, `citations.rs`, `error_shape.rs`, `public_api.rs`, `parallax_contract.rs`, `fuzz_regression.rs` (requires `fuzzing` feature), `codec_dependency_boundary.rs`.

**`tests/fixtures/`:**
- `golden/`: `phase1_sample_identity.iamf` + `.sha256` + `.dump.txt` (cross-target byte identity).
- `reference/`: `test_NNNNNN.iamf` + `.textproto` pairs from iamf-tools; `reference/iamf-tools/` encoder outputs; `reference/negative/` expected-rejection files.
- `codecs/{flac,opus}/`: `packet-NNN.bin`, `source.s16le`, `expected.s16le`, `MANIFEST.md`.

**`fuzz/`:** separate workspace; `CORPUS.md` documents seeds.

**`tools/`:** `build-reference.sh` (builds libiamf/iamf-tools), `prove-guards.sh` (proves lints fire), `check-codec-*.sh`, `red-evidence.sh`, `cargo-test-tap.sh`, `iamf-tools.Dockerfile`, `experiments/`.

## Key File Locations

**Entry Points:**
- `src/lib.rs`: library root.
- `fuzz/fuzz_targets/parse_sequence.rs`, `fuzz/fuzz_targets/obu_roundtrip.rs`: fuzzers.

**Configuration:**
- `Cargo.toml`: package, features (`fuzzing`), `[lints]` hardening.
- `clippy.toml`, `deny.toml`, `fuzz/deny.toml`, `tools/codec-fixtures/deny.toml`, `rust-toolchain.toml`.
- `REFERENCES.md`: pinned reference SHAs.

**Core Logic:**
- `src/obu/mod.rs`: OBU framing and single trailing drain.
- `src/sequence.rs`, `src/encoder.rs`.

**Testing:**
- `tests/*.rs`, `tests/support/*.rs`, `tests/fixtures/`.

## Naming Conventions

**Files:**
- snake_case, named after the IAMF OBU/concept: `src/obu/mix_presentation.rs`.
- Integration tests: snake_case by concern, flat in `tests/`: `tests/encoder_streaming.rs`.
- Fixtures: reference name preserved (`test_000003.iamf` + `test_000003.textproto`).

**Functions:**
- `read_<obu>` / `write_<obu>` pairs, e.g. `read_codec_config` / `write_codec_config`.

**Directories:**
- Module dirs with `mod.rs` (`src/bits/mod.rs`, `src/obu/mod.rs`, `src/model/mod.rs`).

## Where to Add New Code

**New OBU type or payload field:**
- Implementation: `src/obu/<name>.rs` with adjacent `read_`/`write_` fns and `// ref:` citations; re-export from `src/obu/mod.rs`; dispatch in `src/sequence.rs` (parse + `SequenceWriter`) and `src/dump.rs`.
- Tests: `tests/descriptors.rs` / `tests/vectors.rs` (hex vectors), `tests/round_trip.rs`; extend `src/fuzzing.rs` generator.

**New bit primitive:**
- `src/bits/reader.rs` and `src/bits/writer.rs` in matching order; oracle test in `tests/bits_oracle.rs`.

**New error:**
- Variant in `ErrorKind` in `src/error.rs`; keep `tests/error_shape.rs` size budget.

**Cross-descriptor validation / profile rules:**
- `src/model/mod.rs` (`DescriptorSet::validate`), `src/model/profile.rs`; tests in `tests/profile.rs`.

**Encoder API:**
- `src/encoder.rs` (builder methods + `build()`); layout packing in `src/packing.rs`; tests `tests/encoder_builder.rs`, `tests/packing.rs`, update `tests/public_api.rs` / `tests/parallax_contract.rs`.

**Codec-dependent tests (FLAC/Opus encode/decode):**
- `tools/codec-fixtures/tests/` only — never add codec crates to root `Cargo.toml`.

**Shared test helpers:**
- `tests/support/`.

## Special Directories

**`target/`:** build output. Generated: Yes. Committed: No.

**`docs/libiamf/`, `docs/iamf-tools/`, `docs/eclipsa-audio-plugin/`, `docs/iamf/`, `docs/oar/`:**
- Purpose: Untracked local checkouts of upstream reference implementations/spec for reading. Generated: No. Committed: No (untracked). `docs/assets/` is tracked.

**`fuzz/`, `tools/codec-fixtures/`:**
- Excluded from the root workspace (`[workspace] exclude`), each with its own lockfile and deny config. Committed: Yes.

**`.planning/`:** GSD artifacts. Committed: Yes.

---

*Structure analysis: 2026-09-13*
