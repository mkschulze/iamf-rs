# Phase 3: FLAC and Opus Framing - Research

**Researched:** 2026-09-09  
**Domain:** IAMF v1.1.0 codec configuration, pre-encoded access-unit framing, and codec-oracle tests  
**Confidence:** HIGH for wire layouts and validation rules read from the pinned sources; HIGH for the local Rust 1.85/licence preflight; MEDIUM for four-target behavior of the C-backed Opus test dependency until CI executes it.

---

## Executive recommendation

Implement codec framing without putting an encoder or decoder in `src/`:

1. Add typed `FlacDecoderConfig` and `OpusDecoderConfig` variants in
   `src/obu/codec_config.rs`. Constructors derive the 4CC and roll distance;
   readers preserve the wire value and `validate()` reports contradictions.
2. Keep `AudioFrame` payloads opaque. Extend the fixture adapter so a test may
   supply already encoded access units and independently decoded expected PCM,
   while every codec still traverses the same `SequenceWriter` and
   `assert_conformant` clauses.
3. Pin `claxon = 0.4.3`, `flacenc = 0.5.1` with default features disabled, and
   `opus = 0.4.0` as dev-only tools. Never import them from `src/`. Commit the
   packets and expected PCM so ordinary tests are offline and deterministic.
4. Accept FLAC rates in the pinned STREAMINFO range. Accept only 48 kHz on the
   fresh Opus encoder path: IAMF timing output is always 48 kHz and this crate
   does not resample. Return the new typed sample-rate error before writing.

The pinned reference identities are
`iamf-tools@848c6ff4968ff8cc6f728259892ab4f90cb83256` (`v2.1.0`) and
`libiamf@f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63` (`v1.1.0`), as recorded in
[`REFERENCES.md`](../../../REFERENCES.md). Source citations below refer to the
local trees under `.reference/` at exactly those SHAs.

## Source and preflight findings

The dependency preflight has already been executed in an isolated Cargo
project with `edition = "2024"`, `rust-version = "1.85"`, and the repository's
`deny.toml`. `claxon 0.4.3`, `flacenc 0.5.1` with
`default-features = false`, and `opus 0.4.0` compile, and the candidate graph
passes `cargo deny check licenses` once the throwaway root declares
`MIT OR Apache-2.0`. This satisfies CODEC-06 as research evidence; the real
lockfile and all four CI targets must still be checked after the dependencies
are added.

| Tool | Role | Licence/graph | Important risk |
|---|---|---|---|
| `claxon = 0.4.3` | independent FLAC decode oracle | Apache-2.0; no normal dependency | Old crate and no declared `rust-version`; compilation on 1.85 was therefore tested rather than inferred. |
| `flacenc = { version = 0.5.1, default-features = false }` | deterministic FLAC fixture production | Apache-2.0; pure Rust graph, but has `build.rs` | Default features add parallelism/logging/serde; keep them off. Fixture regeneration must be explicit, not part of normal tests. |
| `opus = 0.4.0` | Opus packet production and standalone packet decode | MIT/Apache-2.0; pulls `opusic-sys 0.7.5` (BSD-3-Clause) and bundled libopus/CMake | Unsafe FFI and a C toolchain exist in the **test** graph. Prove Windows/macOS/Linux builds; never let this edge enter `cargo tree -e normal,no-proc-macro`. |

Primary package evidence is in the locally cached crate manifests and the
throwaway manifest at `/private/tmp/iamf-codec-precheck.5WKZg2/Cargo.toml`.
`opusic-sys 0.7.5` declares Rust 1.82 and defaults to its `bundled` CMake build.
The shipping crate remains `unsafe_code = "forbid"`; that lint does not and
cannot certify transitive dev dependencies.

## Exact wire contracts

### FLAC decoder configuration (CODEC-01)

The canonical configuration is 38 bytes: a four-byte FLAC metadata-block
header followed by the 34-byte STREAMINFO payload. There is no `fLaC` stream
marker in IAMF's `decoder_config`.

| Field | Width | Encoder rule |
|---|---:|---|
| `last_metadata_block_flag` | 1 bit | `1` for the canonical one-block config |
| `block_type` | 7 bits | `0` (STREAMINFO) |
| `metadata_data_block_length` | 24 bits | `34` |
| `minimum_block_size` | 16 bits | `num_samples_per_frame`, at least 16 and at most `u16::MAX` |
| `maximum_block_size` | 16 bits | same as minimum |
| `minimum_frame_size` | 24 bits | `0` |
| `maximum_frame_size` | 24 bits | `0` |
| `sample_rate` | 20 bits | `1..=655_350` Hz |
| `number_of_channels` | 3 bits | raw field fixed to `1`; do not derive it from the Audio Element |
| `bits_per_sample` | 5 bits | raw `actual_bits_per_sample - 1`; pinned strict range is `3..=31`, corresponding to actual 4..=32 bits |
| `total_samples_in_stream` | 36 bits | use `0` (unknown) for the fresh streaming/framing constructor |
| `md5_signature` | 128 bits | all zero |

All fields are written MSB-first with the existing `BitWriter`. The packed
20/3/5/36-bit tail is deliberately not assembled with shifts in calling code;
the bit writer already supplies checked width and alignment handling.

The first block must be STREAMINFO; its block sizes must equal
`num_samples_per_frame`; FLAC `audio_roll_distance` is zero. The pinned encoder
also treats zero frame sizes and an all-zero MD5 as mandatory for output, even
though the source labels these IAMF `SHOULD` constraints. Sources:
`.reference/iamf-tools/iamf/obu/decoder_config/flac_decoder_config.{h,cc}` at
the pinned SHA, especially `FlacStreamInfoStrictConstraints`,
`WriteStreamInfo`, and `ValidateEncodingRestrictions`.

The typed parser should claim the canonical 38-byte prefix only when it is
structurally complete. A shorter known config remains `Raw` so Phase 2's
byte-reproduction contract survives. Bytes after the named prefix stay in the
existing `CodecConfig.trailing` path. Preserve the header's last-block bit on
read; validate a contradictory canonical shape rather than silently changing
it.

Audio Frame payloads contain complete FLAC frame bytes, not a standalone FLAC
file header or STREAMINFO. Fixture generation may serialize a full temporary
FLAC stream and split out its metadata and frame records, but committed IAMF
packet fixtures must contain only the access-unit frame bytes.

### Opus decoder configuration (CODEC-02)

IAMF calls this an OpusHead layout, but it is exactly 11 bytes and does **not**
contain the eight ASCII bytes `OpusHead` used by an Ogg identification packet.
Every multibyte field is big-endian in IAMF.

| Offset | Field | Width | Fresh-output rule |
|---:|---|---:|---|
| 0 | `version` | 8 | `1` (non-zero; upper-nibble major version must be zero) |
| 1 | `output_channel_count` | 8 | fixed `2` |
| 2 | `pre_skip` | 16 | measured encoder lookahead; non-zero in the committed fixture |
| 4 | `input_sample_rate` | 32 | `48_000` |
| 8 | `output_gain` | signed 16 | fixed `0` |
| 10 | `mapping_family` | 8 | fixed `0` |

This is transcribed from
`.reference/iamf-tools/iamf/obu/decoder_config/opus_decoder_config.{h,cc}`;
the pinned test's 48 kHz bytes are `01 02 00 00 00 00 bb 80 00 00 00` before a
non-zero pre-skip is inserted. `libiamf` independently reads offsets 2, 4, 8,
and 10 as big-endian in
`.reference/libiamf/code/src/iamf_dec/vlogging_tool_sr.c` and initializes its
decoder from the rate at offset 4 in
`.reference/libiamf/code/src/iamf_dec/opus/IAMF_opus_decoder.c`.

The typed parser claims exactly 11 bytes when present and leaves any bounded
remainder in `CodecConfig.trailing`. A shorter `Opus` config remains `Raw`.
Validation covers all fixed fields, version, non-zero pre-skip for a fresh
framing fixture, the stored roll distance, and rate consistency.

## Sample rates, roll distance, and trimming

### Sample-rate policy (CODEC-05)

- FLAC's STREAMINFO rate is a 20-bit field, but the pinned FLAC validation
  narrows fresh/valid values to `1..=655_350`. Reject zero, out-of-range rates,
  and a frame size that cannot fit the 16-bit min/max block-size fields.
- Opus has a 48 kHz IAMF output clock regardless of the informational input
  rate (`OpusDecoderConfig::GetOutputSampleRate()` returns 48,000). Because the
  fixture path maps input samples, packet durations, trims, and output samples
  without resampling, the fresh constructor accepts only 48,000. A requested
  44.1 kHz (or any other mismatch) returns
  `Error::SampleRateNotSupportedByCodec` before bytes are emitted.
- Parsing remains faithful: foreign input rates are stored and diagnosed by
  `validate()`; they are not normalized to 48 kHz.

This matches the pinned `audio_frame_generator.cc` guard that rejects differing
encoder-input and decoder-output rates rather than resampling. The generic
Opus codec itself supports output rates 8/12/16/24/48 kHz (see
[RFC 6716 §2.1.1](https://www.rfc-editor.org/rfc/rfc6716#section-2.1.1)), but the
IAMF v1.1.0 output timing rule used here is 48 kHz; generic codec capability is
not permission to widen this constructor.

### Roll distance (CODEC-03)

For Opus:

```text
R = ceil(3840 / num_samples_per_frame)
audio_roll_distance = -R
```

Implement ceiling division without overflow as `q = 3840 / n`, then add one
iff `3840 % n != 0`; reject `n == 0`, checked-convert `q` to `i16`, then checked
negate. Examples: `n=960 -> -4`, `n=120 -> -32`. Constructors derive this
value. Parsers preserve the signed 16-bit wire value and validation recomputes
the expected value. FLAC remains zero. Source:
`.reference/iamf-tools/iamf/obu/decoder_config/opus_decoder_config.cc`,
`GetRequiredAudioRollDistance`.

### Pre-skip and trim (CODEC-04)

`pre_skip` is codec configuration; `trim_at_start` is the IAMF Audio Frame
instruction that removes the priming samples from the rendered timeline. The
fixture generator must query Opus lookahead and record it (the pinned tool uses
`OPUS_GET_LOOKAHEAD`; its test fixture observes 312 at 48 kHz), not hard-code
312 as a protocol constant. Require:

- `pre_skip > 0` and first temporal unit `trim_at_start == pre_skip`;
- the same start/end trims on every substream frame in a temporal unit;
- `trim_at_end` independently represents final zero padding;
- a fixture where start and end trim differ, proving END-before-START wire
  order cannot pass by symmetry.

The pinned flow is documented by
`.reference/iamf-tools/iamf/cli/codec/opus_encoder.cc` and
`proto_conversion/proto_to_obu/audio_frame_generator.cc`. The latter adds the
encoder-required delay to the start-trim state and distributes trim across
frames. Existing `src/obu/header.rs` already writes end trim before start trim;
do not add another header representation.

For the standalone oracle, decode the exact committed packet sequence from a
fresh decoder state, concatenate interleaved PCM, then apply the IAMF frame
headers' start/end trims. Compare that result to `libiamf` output exactly. Do
not compare Opus output with its pre-encode source PCM and do not introduce a
tolerance.

## Fixture and shared-harness design

The current `Fixture::encode()` in `tests/support/fixture.rs` converts LPCM
samples into bytes itself, while `assert_conformant()` in
`tests/conformance.rs` owns the actual gate. Preserve that ownership split:

- Generalize fixture preparation to return per-temporal-unit,
  per-substream encoded payloads plus explicit `Trimming` and expected
  interleaved PCM. LPCM uses the existing packing code; FLAC and Opus read
  committed access-unit files. `SequenceWriter` remains the only IAMF writer.
- Generalize `ElementSpec` to obtain sample rate and comparison depth from any
  typed decoder config. Avoid an LPCM-only accessor hidden behind a generic
  name.
- Keep one `assert_conformant` path for manifest pinning, distinguishable
  signal, forced trim, structural walk, two `libiamf` decodes, limiter drift,
  exact PCM equality, and strict `iamf-tools` parsing. Codec-specific code may
  prepare expected PCM, but may not skip or weaken clauses.
- For FLAC, the expected PCM is the deterministic non-silent source and Claxon
  must also decode the committed frame material to it. For Opus, expected PCM
  comes from a standalone decode of the same committed packets after applying
  IAMF trims.
- Commit a provenance manifest beside packets: generator crate/version,
  settings, input digest, packet ordering, frame size, rate, channels,
  pre-skip, end padding, and SHA-256 for each packet and expected PCM artifact.
  Normal tests must not invoke an encoder, network, Docker, or system codec.

Existing upstream fixtures under `tests/fixtures/reference/iamf-tools/` are
excellent parser regressions and byte-layout cross-checks, but they are not a
replacement for the Phase 3 authored fixture: several have no paired source
PCM/config, and the Opus oracle must know the exact standalone expected decode.

## Validation Architecture

Validation is deliberately layered so a shared misunderstanding cannot make
all evidence green.

| Requirement | Red/green evidence | Final command or artifact | Wave |
|---|---|---|---:|
| CODEC-01 | Hand-derived 38-byte vector first; typed read/write and malformed-short/raw preservation; committed FLAC packets decode through Claxon and `libiamf` to identical PCM | `cargo test --locked --test descriptors --test round_trip --test conformance -- --nocapture --test-threads=1` with `IAMF_REF_DECODER=.reference/libiamf/code/test/tools/iamfdec/iamfdec` | 1 |
| CODEC-02 | Hand-derived 11-byte vector, fixed-field negatives, typed/raw round trip, exact committed packet replay | same focused descriptor/round-trip tests, then conformance command | 2 |
| CODEC-03 | Table tests at `n={0,1,120,960,3839,3840,3841,96000}`; constructor derivation and parsed-mismatch Finding | `cargo test --locked --test descriptors --test temporal` | 2 |
| CODEC-04 | First frame has non-zero start trim, start differs from end, serialized header proves END-before-START, independent decoded PCM equals `libiamf` output | `cargo test --locked --test obu_header --test conformance -- --nocapture --test-threads=1` | 2 |
| CODEC-05 | Constructors reject mismatched Opus rate and invalid FLAC rate/frame-size with the exact typed `ErrorKind`; parser still preserves foreign bytes | `cargo test --locked --test error_shape --test descriptors` and `bash tools/prove-guards.sh` | 1–3 |
| CODEC-06 | Preserve the isolated preflight record; repeat policy against the real lockfile | `cargo deny check licenses && cargo deny check advisories bans sources` | 0, then 3 |
| CODEC-07 | Mechanical source-import ban and exact normal graph; codec names allowed only in dev graph | `cargo tree --locked -e normal,no-proc-macro --prefix none`; `rg -n 'claxon|flacenc|opus' src Cargo.toml`; inspect `[dev-dependencies]` hits only | every wave, final in 3 |

Additional phase-wide gates after each wave:

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo deny check licenses
cargo deny check advisories bans sources
bash tools/prove-guards.sh
```

Run the reference-dependent conformance suite serially because it writes
fixture outputs and invokes external tools. Offline four-target CI runs the
same fixture/parser/vector tests with `IAMF_REF_DECODER` unset and must print
the established skip reasons rather than silently claiming reference coverage.
Reference CI supplies the pinned decoder and pinned `iamf-tools` image.

### Failure localization

| Failure | Most likely owner |
|---|---|
| Hand vector differs before any packet is framed | codec-config bit layout/endian/width |
| Typed round trip differs, but raw round trip passes | typed parser's claimed length or trailing-byte drain |
| Standalone decode differs from committed expected PCM | fixture provenance, packet order, or codec-version drift |
| Standalone decode matches, `libiamf` differs | IAMF packet framing, substream order, roll, pre-skip, or trim application |
| `libiamf` matches but `decoder_main` rejects | strict IAMF semantic validation/fixed fields |
| Normal graph contains a codec crate | manifest feature/dependency placement defect, regardless of passing audio tests |

## Recommended plan decomposition

### 03-01 — Typed codec configs and FLAC proof (Wave 1)

- Add failing hand-byte vectors for canonical STREAMINFO, truncation/raw
  fallback, trailing preservation, invalid block/rate/bit-depth/frame size, and
  zero FLAC roll.
- Add the public non-exhaustive FLAC type/variant, constructor, adjacent
  read/write functions, accessors, validation, dump support, and public
  re-export. Add `SampleRateNotSupportedByCodec` with compact fields that keep
  the established error-size test green.
- Extend fixture preparation for committed encoded access units; create the
  deterministic FLAC packet/source/manifest and run the unchanged gate
  semantics plus Claxon decode.
- Owns CODEC-01 and the FLAC half of CODEC-05. Recheck the normal graph before
  proceeding.

### 03-02 — Opus layout, roll, priming, and exact oracle (Wave 2)

- Add the 11-byte hand vector and fixed-field/truncation/trailing tests before
  implementation.
- Add the Opus type/variant, 48 kHz constructor, checked roll derivation, faithful
  read, validation Findings, dump support, and re-export.
- Generate and commit small deterministic stereo packets, their measured
  non-zero lookahead, unequal final padding, standalone decoded-and-trimmed PCM,
  and provenance. Route them through the shared fixture adapter and gate.
- Owns CODEC-02, CODEC-03, CODEC-04, and the Opus half of CODEC-05.

### 03-03 — Dependency and regression closure (Wave 3)

- Pin the selected dev tools and lockfile only after the isolated licence
  result is recorded. Add a mechanical CI assertion that `src/` contains no
  codec-crate imports and the normal graph contains none.
- Replay both committed packet sets offline, retain foreign/raw round trips,
  run all negative rate/config/roll/trim cases, and execute all guard, lint,
  deny, MSRV, four-target, parser, fuzz-regression, and reference gates.
- Document reproducible fixture regeneration separately from ordinary tests.
- Owns CODEC-06 and CODEC-07 and closes cross-platform risk.

Wave 2 depends on the codec-neutral fixture adapter from Wave 1. Wave 3 is a
closure wave, not a place to discover wire behavior. If C-backed `opus` fails a
target, keep shipping code unchanged and move fixture generation out of the
ordinary test path; do not add a platform codec feature to the library.

## Risks and non-goals

- **No runtime codec API.** Public Phase 3 constructors describe framing and
  accept encoded bytes; encoding/decoding, bitrate, quality, complexity, and
  resampling remain outside the crate.
- **No normalization on read.** A foreign wrong roll/rate/fixed field is
  preserved and reported, not repaired.
- **Do not confuse Ogg and IAMF headers.** Adding `OpusHead` ASCII or using
  little-endian Ogg fields yields an invalid IAMF config.
- **Do not frame whole FLAC files.** STREAMINFO belongs in Codec Config and
  FLAC frames belong in Audio Frame payloads.
- **Do not let fixture tooling define expected bytes.** Hand vectors land
  first; codec/reference output is an independent second oracle.
- **Cross-platform confidence is pending CI.** Local macOS compilation and
  cargo-deny success establish feasibility, not Windows/Linux closure.

## Planning checklist

- Every plan names the exact requirement IDs it owns.
- Every typed reader has a preceding pinned-source citation and a paired writer.
- Every constructor derives codec ID and roll; no public contradictory roll
  parameter exists.
- Every short known config has a raw-preservation regression.
- Both fixtures are non-silent, committed, provenance-labelled, and offline.
- Opus expected PCM is decoded from the same packets and trimmed by IAMF header
  values.
- `cargo tree -e normal,no-proc-macro` remains codec-free after every plan.

---

*Phase: 03-flac-and-opus-framing*  
*Research status: complete — ready for planning*
