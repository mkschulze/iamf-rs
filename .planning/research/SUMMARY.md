# Project Research Summary

**Project:** iamf-rs — IAMF bitstream serialiser/parser (Rust), consumed by Parallax
**Domain:** Bit-level binary format library; licence-constrained, deterministic, fuzzed
**Researched:** 2026-09-07
**Confidence:** HIGH for everything read from `iamf-tools` / `libiamf` source and from golden `.iamf` bytes; MEDIUM for Rust-ecosystem convention; LOW for anything tagged `[SPEC]` (spec prose / code comments)

## Executive Summary

This is a hand-written bit-level codec library, not an application. All four researchers converged on the same shape: two runtime dependencies (`bitstream-io`, `thiserror`), a hand-written recursive-descent reader/writer whose functions mirror `iamf-tools`' C++ one-for-one, and read/write co-located per OBU type so asymmetry is visible in a single diff. The reference implementations are permissively licensed (BSD-3-Clause-Clear + AOM Patent License 1.0) and were read directly; the decoder `libiamf` additionally ships ~340 golden `.iamf` conformance vectors, which is an unusually good oracle for a bitstream library and should be the primary M1 test artefact — `libiamf/tests/test_000003.iamf` is byte-for-byte the M1 target file, and its descriptor prologue is 118 bytes.

**The research contradicts the project's own handoff in several places, and those contradictions are the highest-value output.** The OBU header encoding — which PROJECT.md explicitly said not to guess — is now resolved three independent ways (encoder write path, decoder read path, hand-decoded golden bytes) and is stated once below. The spec version should almost certainly move from v1.0 to **v1.1.0**, because Base-Enhanced (already in the handoff's Correction #4) does not exist in v1.0 and because `libiamf` — the thing whose acceptance *is* the Core Value — implements v1.1.0. `iamf-tools` HEAD is a **draft-v2.0.0 tree** and mirroring its type model produces files `libiamf` rejects. Three of the handoff's own "verified facts" (Corrections #1, #2, #5) have weaker or different provenance than claimed. And the obvious fix for the `HashMap` ban — `BTreeMap` — is *wrong* for descriptor collections, because it reorders bitstream-visible order.

The dominant risk is not difficulty; it is **latency of detection**. Pitfalls 1–8 (`obu_size` origin, the polymorphic bit-6 flag, end-before-start trimming order, leb128 non-minimality, descriptor ordering, exact-valued Codec Config fields, flag/field decoupling, bit-level I/O) all produce files that *almost* work, and `libiamf` is permissive enough to accept several of them. Discovering any one of them after M2 invalidates every fixture, every golden, the parser's mirrored misunderstanding, and the entire accumulated fuzz corpus. The mitigation is a multi-clause M1 exit criterion (below) rather than "libiamf returned OK", plus running `iamf-tools`' strict parser alongside `libiamf`'s permissive one.

## Key Findings

### Recommended Stack

Two runtime dependencies, nine crates in the resolved graph, all satisfied by `allow = ["MIT", "Unicode-3.0"]`. This was executed locally against `cargo-deny 0.20.2` and `clippy 1.92.0`, not inferred. The `deny.toml` in STACK.md §7 passes today (`advisories ok, bans ok, licenses ok, sources ok`), and every one of the six proposed clippy hardening lints was verified to fire.

**Core technologies:**
- **`bitstream-io` 4.10.0** (MIT/Apache-2.0) — bit-level reader/writer. Chosen because `BitsWritten` solves the two-pass `obu_size` problem for free, `byte_align()` is first-class, and const-checked reads cannot panic. Wrap it in `BitReader`/`BitWriter` newtypes whose method names mirror `read_bit_buffer.h` exactly — that wrapper is the audit seam, and the only module that touches the crate.
- **`thiserror` 2.0.20** — typed errors, `#[non_exhaustive]`, `offset` on every variant, and `Clone + PartialEq + Eq` (possible only because `std::io::Error` is mapped at the `bits` boundary and never escapes).
- **Rust 1.85 / edition 2024**, pinned via `rust-toolchain.toml` so the byte-identity matrix compiles identically everywhere. `proptest 1.11` sets the effective MSRV at exactly 1.85.
- **Deliberately zero codec dependencies in the shipping crate.** IAMF's FLAC and Opus `decoder_config` fields are not codec data — they are fixed bit layouts (FLAC STREAMINFO ~60 lines; OpusHead 11 bytes, three of six fields constant). Codec crates are needed only as *dev*-dependencies for M3 test material, and Opus needs none at all if packets are committed as fixtures.
- **Write leb128 by hand.** The `leb128` crate cannot emit non-minimal/fixed-size ULEB128 (which `LebGenerator::kFixedSize` requires) and does not enforce IAMF's 8-byte / `u32` caps. ~30 lines each way.

### The OBU header — RESOLVED, and the agreed answer

FEATURES.md and PITFALLS.md resolved this independently and **agree completely**. Provenance: `iamf-tools` `obu_header.cc::ValidateAndWrite` (write), `iamf-tools` v1.0.0 `ce54b9a` (byte-identical — the header has not changed since v1.0), `libiamf` `iamf_obu.c:56–95` (read), and a hand-decoded golden file.

```
byte 0:  [ obu_type : 5 ][ obu_redundant_copy : 1 ][ type_specific_flag : 1 ][ obu_extension_flag : 1 ]
         MSB-first; obu_type occupies bits 7..3.
then:    obu_size : uleb128 (1..8 bytes)
then:    if (audio-frame && type_specific_flag):
             num_samples_to_trim_at_end   : uleb128   <- END FIRST
             num_samples_to_trim_at_start : uleb128
then:    if (obu_extension_flag): extension_header_size : uleb128, then that many bytes
then:    payload
```

**`obu_size` counts every byte after the `obu_size` field itself** — including the trim fields and the extension header, excluding byte 0 and the size bytes. `libiamf` reconstructs the total as `obu_size + len(obu_size) + 1`. `[WIRE]` proof: `0x32 0x82 0x04 0x40 0x00 <512 bytes>` → type 6, trimming set, `obu_size = 514`, trim_end = 64, trim_start = 0, payload 512.

No disagreement between the two researchers on any clause. Points of nuance both raised:

- **leb128 need NOT be minimal on the wire.** Non-minimal (zero-padded) encodings up to 8 bytes are legal and the reference *emits them deliberately* in `kFixedSize` mode. **Encoder: hard-code minimal, do not expose a `LebMode` knob** (it makes byte-identity caller-dependent). **Parser: accept 1–8 bytes; typed error on a 9th continuation byte and on `> u32::MAX`.** Note the reference asymmetry — `iamf-tools` rejects both; `libiamf` loops exactly 8 times without checking continuation and *clamps* to `UINT32_MAX`. Do not use `libiamf` as a well-formedness oracle.
- **Bit 6 is polymorphic.** Audio frames (5, 6–23) → `obu_trimming_status_flag`; Temporal Delimiter (4) → `is_not_key_frame` (inverted sense); Mix Presentation (2) → `optional_fields_flag` (**v2.0 draft — never set for v1.1.0**); everything else reserved, SHALL be 0. `iamf-tools` rejects a set reserved bit; **`libiamf` does not validate it at all**, so passing M1's `libiamf` gate does not prove conformance on this axis. Model it as an enum carried by the payload variant, not a `bool` on a shared header.
- **Trim order is END before START.** Byte-identical whenever the two values are equal — which is almost always — so the bug is invisible without a fixture whose sample count is not a multiple of the frame size.
- `obu_redundant_copy` is forbidden on audio frames, temporal delimiters and parameter blocks; legal only on descriptors (0, 1, 2, 31).
- **No padding, no alignment bits.** Every payload's bit fields must sum to whole bytes by construction; the reference errors rather than pads. `BitWriter` must expose `is_byte_aligned()` and assert it at every OBU boundary — this one assertion catches nearly every payload bit-count error cheaply.
- **Forward-compat:** on read, bytes between the end of the parsed payload and `obu_size` are kept verbatim in `footer_`. **Every Rust OBU struct needs `trailing: Vec<u8>` from day one**, drained centrally by the shared `read_obu` wrapper — retrofitting after M2's equality test is written is painful.

### Spec version: v1.0 vs v1.1.0 — evidence on each side

Flagged by STACK, FEATURES and PITFALLS independently. **Unresolved; it is a decision, and it is cheap now and expensive later.**

| For v1.0 | For v1.1.0 |
|---|---|
| The certification programme cites v1.0 (PROJECT.md constraint) | `libiamf`'s README: the reference decoder implements **v1.1.0**, "including the base-enhanced profile introduced in the v1.1.0 specification since the release of v1.0.1" `[DEC]` |
| | PROJECT.md's own Correction #4 lists **Base-Enhanced**, a profile that does not exist in v1.0. `ProfileVersion` at `iamf-tools` v1.0.0 has only Simple and Base. The brief is internally inconsistent |
| | The Core Value is "`libiamf` reads it back", and `libiamf` is a v1.1.0 decoder |
| | `iamf-tools` source comments say "as of IAMF v1.1.0"; the spec is published at `v1.1.0.html` |

**What it blocks:** the `SPEC_VERSION` constant, the profile enum's legal range, whether expanded loudspeaker layouts exist at all, and whether the conformance claim is falsifiable. FEATURES.md recommends pinning v1.1.0 and editing PROJECT.md. PITFALLS.md is more cautious: "if they differ, target v1.0 and record which v1.1 features are deliberately unemitted." **They differ in emphasis, not in facts.** Note also that the v1.1.0 subset happens to be the subset PROJECT.md already scopes — building only what is scoped stays v1.1.0-clean automatically.

### Which reference tree to read, and how to pin it

Three researchers touched this and the reconciliation is clean:

- **`iamf-tools` `main`/HEAD is a draft-v2.0.0 tree.** Its own source says so (`TODO(b/461488730): Ensure these agree with final v2.0.0 limits`). HEAD-only constructs that a v1.1.0 decoder rejects: OBU type 24 (Metadata), object-based audio elements + `ObjectsConfig`, param definition types 3–8 (Polar/Cart8/Cart16/Dual*), profiles 3/4/5 (Base-Advanced, Advanced-1/2), expanded layouts 13–19, `MixPresentationOptionalFields`, and a re-laid-out `RenderingConfig` (same byte width — **all-zero is identical on the wire, non-zero is not**).
- PITFALLS.md's observation that HEAD now has six profiles and an `aac_encoder.cc` is **the same finding from the other side**: it is evidence the handoff's facts are already a version behind, not evidence that six profiles are targetable.
- STACK.md's note that HEAD contains `polar_parameter_data` / `metadata_obu` is likewise the v2.0 draft surfacing. It does **not** overturn the "IAMF as practised is a bed format" conclusion for v1.1.0 — those are exactly the v2.0-draft object-position machinery.

**Single instruction:** read `iamf-tools` for *mechanism* (how a field is written), and treat **`libiamf/tests/*.iamf` golden vectors as the definition of "accepted"**, not HEAD source. Pin exact SHAs for both repos in a `REFERENCES.md` and have CI check out those SHAs, never `main`. Bumping a pin is a deliberate task that re-runs the conformance suite in the same commit. Tags `v2.0.0` / `v2.1.0` of `iamf-tools` exist but were not fetched — worth five minutes before the type model is written, since one of them may be a v1.1.0-exact tree.

### Corrections to the handoff's corrections

FEATURES.md re-derived PROJECT.md's "verified facts" and three did not survive as stated. **All three still stand as scope decisions; the evidence behind them is different from what the document claims.**

1. **Correction #1 (no object-based path)** — true of `iamf-tools` at v1.0/v1.1 and true *for the profiles we target* (`profile_filter.cc` erases Simple, Base and Base-Enhanced the moment an element is object-based). **No longer true of `iamf-tools` HEAD in general**, which has `ObjectsConfig` for the v2.0 draft. The reason to skip objects is "the target profiles forbid it and `libiamf` cannot decode it", not "the reference cannot do it".
2. **Correction #2 ("Keeping things simple with 1 layer for now")** — that comment **does not exist anywhere in `iamf-tools`**. It is Eclipsa's. `iamf-tools` fully implements multi-layer `ScalableChannelLayoutConfig`, recon gain and output gain. The correct framing: "the reference *product* chose one layer, and the profiles do not require more."
3. **Correction #5's layout numbering 0–30** — that is Eclipsa's internal plugin enum (`Speakers.h:133–186`), **not an IAMF field**. No IAMF wire field is numbered 0–30. It conflates three fields across two OBUs: `loudspeaker_layout` (4 bits, 0–15), `expanded_loudspeaker_layout` (u8, present only when layout == 15, **offset by 13** from Eclipsa's index), `audio_element_type = 1` + `AmbisonicsConfig` for HOA, and `SoundSystem` (4 bits, in Mix Presentation) for 22.2. Correction #5's *substance* is right — 9.1.6 is genuinely not a base layout; it is `expanded_loudspeaker_layout = 8`, "subset of Sound System H", 16 of 24 channels, and `SoundSystem 13 = 16 channels` corroborates it.
   **Design consequence: four distinct Rust types, not one flat 31-variant enum.**

Also worth folding into PROJECT.md: pin v1.1.0; add "do not mirror `iamf-tools` HEAD" as an explicit constraint.

### Determinism — one reconciled prescription

Three partial answers that fit together:

- **STACK.md:** the IAMF wire format has **no floating-point fields** (exhaustive grep of `iamf/obu/**`). Loudness and mix gain are `int16` Q7.8; object positions are `int16`/`int8`/`uint8`. **Therefore `libm` is not a day-one dependency**, and the determinism problem reduces to ordered containers plus integer arithmetic.
- **ARCHITECTURE.md:** `BTreeMap` is the **wrong** fix for the `HashMap` ban in the one place it is most tempting. `HashMap` is non-deterministic; `BTreeMap` is deterministic but silently reorders descriptors by ID, and **descriptor order is bitstream-visible**, so the swap trades a determinism bug for a round-trip bug that looks like a fix. Descriptors must be `Vec<T>` in bitstream order plus a `by_id()` accessor (N ≤ 28 by profile, so a linear scan is free).
- **PITFALLS.md:** the reference solves the same problem by storing in `absl::flat_hash_map` and calling `SortedKeys(...)` explicitly before writing — i.e. it separates storage from write order. The reference contains the fix for the bug you are about to write.

**Prescription:** `Vec` in bitstream order for anything whose order reaches the wire (descriptors, substream IDs, layers, layouts, sub-mixes). `BTreeMap` **only** for lookup-only structures that never drive output order — `ParamDefinitionRegistry` is the canonical example. `HashMap`/`HashSet` banned outright via `clippy.toml` `disallowed-types` (verified firing, with the `reason` string surfaced). On the *encode* path, sort by ascending ID before writing, matching `obu_sequencer_base.cc:WriteDescriptorObus`; on the *parse* path, preserve observed order. Add the same-process double-encode test (the only cheap check that catches per-process hash seeding) and the four-target SHA-256 hash matrix. The remaining float hazard is a single one: caller-supplied LUFS → Q7.8, which needs one range-checked helper using `round_ties_even` (not `as i16`, which truncates toward zero and biases every negative loudness value upward by up to 1 LSB).

### Architecture Approach

Four horizontal layers, matching both `iamf-tools` (`common` → `obu` → `cli` → `include`) and the `mp4` crate. Types own their serialisation: `read_payload`/`write_payload` live in the same file as the struct, behind two crate traits — **not** parallel `model/`/`read/`/`write/` trees, because a missed field in a split layout compiles fine and fails only as a runtime round-trip mismatch.

**Major components:**
1. **`bits`** — `BitReader<'a>` borrowing `&[u8]`, `BitWriter`, `LebPolicy`, `remaining()`, `is_byte_aligned()`, `sub_reader(len)`. Knows nothing about IAMF. The only place bit offsets exist.
2. **`obu::<type>`** — one file per OBU type, struct + both directions + its round-trip test. `ReadObu` uses a GAT `type Ctx<'a>`: `()` for everything except Parameter Block, which takes `&'a ParamDefinitionRegistry`. This puts in the type system what the C++ can only express as a static factory the caller must remember to call.
3. **`sequence` / `encode`** — drivers: ordering, timing, profile choice, ID assignment. Push/pull streaming is the primitive (`push_descriptors` → `push_temporal_unit` → `finish`); whole-file is a thin wrapper, because an hour of 7.1.4 24-bit is ~4 GB.
4. **`codec`** — framing only (PCM ↔ substream bytes). LPCM in the ungated core. **Gate the drivers and the codec libraries, never the bitstream** — `--no-default-features` then yields a dependency-free probe/inspect build, which is the ideal fuzz target and `cargo deny` baseline.

Two non-obvious calls: loudness supplied **up front at `build()`** (append-only writer, `W: Write` suffices, trivially deterministic) with the seek-back variant available but not built in M1; and no trait/factory pair for the public API — the C++ split exists for ABI stability across a shared library boundary, which Rust with one path-dependency consumer does not have.

### Critical Pitfalls

1. **`obu_size` measured from the wrong origin** — four plausible wrong answers, all producing files that nearly work, and a one-OBU test file cannot detect any of them. Serialise the payload to a `Vec` first, compute the size, then write the header. Write `find_obu_boundaries(&[u8])` *before* the first encoder test and assert the last boundary lands exactly on `bytes.len()`.
2. **Bit 6 modelled as one shared `bool`** — makes reserved-bit misuse representable, and `libiamf` will not catch it. Enum carried by the payload variant; `iamf-tools`' parser is the check that catches this class.
3. **Trimming written start-then-end** — byte-identical whenever both values are equal. Only a fixture whose sample count is *not* a multiple of the frame size turns it into a test failure.
4. **Testing that proves only self-consistency** — round-trip passes forever when serialiser and parser share a misunderstanding; silence as the test signal hides channel order, endianness, trimming and frame-count errors simultaneously; goldens generated by the code under test freeze bugs. Per-channel-distinguishable non-silent signal, ≥6 OBUs, two Codec Configs or two Audio Elements.
5. **BCG channel→substream packing** (coupled stereo pairs first, then mono) — wrong order gives a file that decodes cleanly with the channels scrambled. Note this order is *not* the presentation channel order. Highest silent-failure risk in M1.
6. **Building the parser, codecs or API before M1 is verified** — a late `obu_size` fix costs the encoder function (hours, the part people estimate) plus every fixture, every literal-byte test, the parser that mirrored the misunderstanding while passing its round-trips, **the entire fuzz corpus** (all accumulated coverage explored a grammar that does not exist), and every prior conformance claim.
7. **`sample_format_flags = 0` means BIG-endian** — the opposite of a WAV-shaped mental model. Backwards produces a file that decodes without error and sounds like white noise, i.e. looks like a DSP bug.

## Implications for Roadmap

The milestone spine M1→M5 in PROJECT.md survives the research intact. What changes is **what M1 contains**, which is substantially more than its one-line description suggests.

### Phase 1 (M1): A standalone `.iamf` that `libiamf` decodes sample-identically

**Rationale:** M1 is not "get one boring LPCM file out". It is the milestone that must *discover and pin* every fact in Pitfalls 1–8, install every guardrail before there is code to retrofit, and start the external-tooling work whose latency is invisible on the code critical path. PROJECT.md already says M1 may not be reordered; the research says the reason is that everything downstream inherits M1's misunderstandings silently.

**Delivers:**
- `bits` first commit: `BitReader`/`BitWriter` with `is_byte_aligned()`, `bits_remaining()`, separate signed/unsigned methods, hand-computed unit vectors — before any OBU exists.
- Hand-written leb128 both directions (minimal on write; 1–8 bytes accepted on read, with typed errors for a 9th continuation byte and `u32` overflow).
- `obu::header` with the resolved encoding and the polymorphic bit-6 enum.
- IA Sequence Header (31), Codec Config (0) + LPCM decoder config with **derived** `audio_roll_distance`, Audio Element (1) single-layer channel-based, Mix Presentation (2) with one sub-mix and the **mandatory stereo layout**, and the **mandatory** element/output Mix Gain param definitions (mode 1, `default_mix_gain = 0`) — mandatory structure even with zero Parameter Block OBUs.
- Audio Frames with implicit substream IDs (`id ≤ 17 → 6 + id`), LPCM interleave, BCG packing, trimming on the final frame.
- `trailing: Vec<u8>` on every OBU struct from day one.
- **Guardrails, all day 1:** `deny.toml` copied **verbatim** from Parallax (not re-authored, or the two drift and the check becomes theatre) and *proven to fail* on a deliberately added LGPL crate; `clippy.toml` `disallowed-types`; the six hardening lints; `SPEC_VERSION` in code; `REFERENCES.md` with pinned SHAs; `NOTICE`; the `CONTRIBUTING.md` "did not consult gpac/libspatialaudio" checkbox; the cross-target byte-identity matrix ("from the first release" is a stated constraint, and a matrix added later merely documents whatever the code already does).
- **Started day 1, in parallel, off the code critical path:** building `iamf-tools` with Bazel once and generating `tests/fixtures/*.iamf`. **`iamf-tools` does not ship a `.iamf` corpus** — `iamf/cli/testdata/` has 338 `.textproto` and exactly **one** `.iamf`. This is a hidden long-pole and a hard prerequisite for M2's "parse `iamf-tools` output". Also day 1: the pinned `libiamf` CI job.

**Reference verification mechanism (settled):** `Command`-in-tests against pre-built binaries discovered via `IAMF_REF_DECODER`, plus `tools/build-reference.sh` at pinned commits, in **one** Linux CI job. **Not `build.rs`** — a build script would force CMake, C++20, abseil, protobuf and fdk-aac onto every `cargo build` including Parallax's and docs.rs's, unconditionally by construction. Not FFI/bindgen yet. Golden fixtures are the always-on layer so `cargo test` is green offline on all four targets.

**Exit criterion — multi-clause, not "libiamf returned OK", and built as a reusable `assert_conformant(config, pcm)` function so M3 reuses it unchanged:**
1. Test signal is per-channel-distinguishable and non-silent.
2. Total sample count is **not** a multiple of the frame size (forces `trim_at_end > 0`).
3. ≥6 OBUs, including two Codec Configs or two Audio Elements (ordering is unobservable with one of each).
4. `libiamf` at a pinned commit: decode OK **and sample count equals what was encoded** (this clause is what catches silent truncation from an oversized `obu_size`) **and** PCM sample-identical.
5. `iamf-tools`' own parser accepts the file (the only check that catches reserved-bit misuse and leb128 strictness — `libiamf` is permissive where `iamf-tools` is strict).
6. Byte-diff against an `iamf-tools`-produced file for an identical configuration: either identical, or **every** difference enumerated in writing with a reason. "Mostly the same" is not a result.
7. Byte-comparison against `libiamf/tests/test_000003.iamf` — reproduce a shipped golden before attempting anything else.

**Avoids:** Pitfalls 1–9, 11, 12(a/c), 13, 14, 15.

### Phase 2 (M2): Parser, round-trip, fuzz

**Rationale:** the parser mirrors the writer, so it must not be written until the writer is verified. The fuzz target depends on the parser and nothing else, which is why PROJECT.md can hard-pin it here.
**Delivers:** sequence-level parser; `ParamDefinitionRegistry` passed as an explicit argument (never hidden parser state — a stateful parser makes the fuzz target either miss type 3 entirely or fabricate a registry, looking healthy while testing nothing); model round-trip `parse(serialize(m)) == m` promised universally; byte round-trip promised only for own output, with the `LebPolicy` caveat documented for foreign files; `parse_sequence` + `obu_roundtrip` fuzz targets; corpus committed and seeded from real `iamf-tools` output; a stable-toolchain corpus-regression test on all four targets so every crash ever found becomes a permanent cross-platform test.
**Nuance from ARCHITECTURE.md worth adopting:** do not read "M1 encoder, M2 parser" as "no reader code before M2". Land the reader alongside the writer per OBU type wherever it is cheap; let M2 be about the *sequence*-level parser, the registry, the round-trip property and the fuzzer, which is the genuinely new work — and give M1 far better tests for free.
**Fuzz placement:** independent `fuzz/` workspace (`cargo fuzz init --fuzzing-workspace=true`), excluded from the root workspace. NCSA has since been accepted into the allow-list by user decision, so **the `libfuzzer-sys` licence issue is resolved** and this is now a hygiene preference (keeps the root `cargo deny` honest about what actually reaches a consumer, keeps `cargo build` fast) rather than a blocker.

### Phase 3 (M3): FLAC and Opus framing

**Rationale:** the LPCM chain must be proven first; the same conformance harness is then reused unchanged.
**Delivers:** FLAC STREAMINFO (hand-written bit fields; channels pinned to 1, frame sizes 0, MD5 zero, `bits_per_sample` stored as value−1) and OpusHead (11 bytes, `output_channel_count` fixed 2, `output_gain` 0, `mapping_family` 0, output rate always 48 kHz), with `audio_roll_distance` derived (`−ceil(3840 / num_samples_per_frame)` for Opus). Opus priming makes `trim_at_start` non-zero for the first time — the second chance to catch a swapped trim order.
**Licence exposure:** this is the **real `deny.toml` exposure milestone**. `libopus` and `libFLAC` are both BSD-3-Clause, but the wrapper crate's own licence, whatever it vendors (the FLAC *command-line tools* are GPLv2), and its build-dependencies are three separate questions. Run `cargo deny check licenses` on a throwaway branch that merely *adds* the candidate crates, before M3 starts. Ten minutes, and it decides the M3 approach. The recommended answer avoids the question entirely: pure-Rust `flacenc`/`claxon` as dev-dependencies (verify `claxon` builds on edition 2024 — 2020 release, no declared edition), and **committed Opus packet fixtures with no dependency at all**.
**Scope pressure to resist:** sample-rate mismatch ("we just need a little resampler") is the single most likely first DSP breach. The correct answer is a typed `Error::SampleRateNotSupportedByCodec`.

### Phase 4 (M4): The Parallax-facing API

**Rationale:** shape depends on the tick-rate answer, which is a Parallax decision (below). Answering it after M4 starts means a breaking change to the only consumer's surface.
**Delivers:** `EncoderBuilder` with a single `build() -> Result<Encoder>` validation point that also assigns IDs and picks the minimum profile; profile selection (~150 lines, directly portable from `profile_filter.cc`, and nearly free once channel counting exists — consider pulling it into M1); the loudness quantisation helper (write it in M1 so the rule exists before there is pressure to inline it).

### Phase 5 (M5+): ISO-BMFF, scalable layers, possibly a decoder

**Licence contamination milestone.** ISO-BMFF is where `gpac` is the obvious reference and it is LGPL-2.1 and forbidden. The realistic breach is not deliberate — it is a search result someone reads twenty lines of, and **contamination is irreversible** (relicensing needs every contributor's agreement). Use the IAMF spec's ISO-BMFF binding section and `iamf-tools`' own permissively-licensed muxer; confirm that muxer exists in the repo before committing to M5. Recon gain and demixing weights are genuinely DSP-adjacent and are exactly why multi-layer is deferred: this crate serialises values it is given, and if nobody supplies them the feature is not ready.

### The blocking open question — parameter tick rate (curves vs pre-decimated blocks)

**Presented, not decided. This is the user's / Parallax's call, and it blocks M4's API shape.**

Facts both researchers agree on:
- `parameter_rate` is a per-definition uleb128, formally independent of the audio sample rate.
- **`parameter_rate` lives in the `param_definition`, inside the Audio Element / Mix Presentation descriptors, which are written before any audio.** So the tick rate is a **`build()`-time input under either option** — never a per-block choice. That is where the decision actually bites.
- In practice the reference always sets `parameter_rate == sample_rate` (16000/16000 and 48000/48000 across the vectors), sets parameter block duration equal to `num_samples_per_frame`, validates only `parameter_rate != 0`, and has **never implemented the case where they differ** — two open TODOs on the same bug ID say so. A file with a lower parameter rate is untested territory for `libiamf`.
- The reference's own decimation lives in `parameter_block_partitioner.cc`, in the **CLI layer, above the public API**.

| Option A — crate takes pre-decimated blocks | Option B — crate takes curves |
|---|---|
| No time model, no float arithmetic on the encode path; determinism inherited from Parallax; `libm` unneeded | Crate acquires a time model, a sampling policy and interpolation (step/linear/Bézier); `libm` becomes a core dependency and byte-identity now depends on this crate's maths across four targets |
| `encode/` stays thin: validate tiling, serialise | One implementation of decimation, chosen by the party that knows the format's constraints; can collapse a static curve to one subblock and pick a rate dividing the frame size cleanly |
| Round-trip stays near-identity at the API boundary | Input ≠ output at the API boundary, so the round-trip invariant has to be asserted one layer down where it is easier to forget |
| Matches the reference's own layering | Parallax's export code gets simpler and cannot get the tiling wrong |
| **Cost:** decimation policy lives in Parallax and must be shared with ADM BWF, or it gets written twice and the two exports disagree | Sits uncomfortably beside the no-DSP constraint; would need an explicit scope carve-out |

FEATURES.md **recommends Option A** (only path with conformance-vector coverage; keeps the crate mechanical; keeps interpolation maths out). ARCHITECTURE.md offers a **structure that defers the decision at no cost**: core API takes blocks, with `encode::curve` behind a `curves` feature strictly above it. That reversibility is real, but **it is not the same as answering the question** — the tick-rate policy itself must still be decided before `build()` has a sensible signature.

### Phase Ordering Rationale

- **Dependency-forced:** `bits` → `obu::header` → descriptors in reference order (Audio Element references a Codec Config ID; Mix Presentation references Audio Element IDs, and the reference parser resolves **forward-only**, so ordering is load-bearing) → param definitions → time-varying OBUs → LPCM framing → sequence writer. Nothing above `bits` can be written or tested without it, and `bits` is the module whose errors are cheapest to make and latest to discover.
- **Verification-forced:** M1 before M2 because every M2 artefact inherits M1's byte-level understanding, and the fuzz corpus in particular becomes worthless if that understanding was wrong.
- **Latency-forced:** fixture generation (Bazel build of `iamf-tools`) and the pinned `libiamf` CI job start on day 1 in parallel with everything, because they are external tooling with their own setup latency and are hard prerequisites for M1's and M2's acceptance tests.
- **Cheapest-now:** every guardrail (`deny.toml`, clippy config, `SPEC_VERSION`, `REFERENCES.md`, the CI matrix) lands in M1 because installing them later means fixing violations rather than never writing them.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1 (M1):** read `libiamf`'s `codec_config_obu.c` and `audio_frame_obu.c` before writing the LPCM path; their payload-level rejection rules were only partly traced. Also resolve the v1.0-vs-v1.1.0 decision and read AOM Patent License 1.0 §1.2 (both one commit now, archaeology later).
- **Phase 3 (M3):** throwaway-branch `cargo deny` run on candidate codec crates before committing to an approach; verify `claxon` builds on edition 2024.
- **Phase 5 (M5):** the ISO-BMFF binding was not researched at all, and the `gpac` prohibition removes the obvious reference. Confirm `iamf-tools`' own muxer exists and is usable before committing.
- **Phase 4 (M4):** blocked on the tick-rate *decision* rather than on missing research.

Phases with standard patterns (skip `--research-phase`):
- **Phase 2 (M2):** the parser mirrors the writer one-for-one, and the fuzzing setup is fully specified in STACK.md §4 and ARCHITECTURE.md. The work is mechanical once M1 is verified.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | **HIGH** | Versions and licences from the crates.io registry API; `cargo-deny 0.20.2`, `cargo tree` and `clippy 1.92.0` **executed locally** against scratch crates. The `deny.toml` passes and every lint was observed to fire. MEDIUM only for `BitsWritten` (docs.rs API listing, not yet exercised) and LOW for `claxon` on edition 2024 (not attempted) |
| Features | **HIGH** for `[ENC]`/`[DEC]`/`[WIRE]`; **LOW** for `[SPEC]` | Read from `iamf-tools` @ `901a86e` + tag `v1.0.0`, `libiamf` @ `e55e183`, and golden `.iamf` bytes hand-decoded. The spec itself was deliberately **not** fetched; every prose-derived claim is tagged unverified |
| Architecture | **HIGH** for reference-derived structure; **MEDIUM** for Rust conventions | File paths and header contents read from `raw.githubusercontent.com` at named paths, not summaries. Comparisons to `mp4`, `binrw`, Symphonia are structural and MEDIUM |
| Pitfalls | **HIGH** for IAMF-specific; **MEDIUM** for Rust-ecosystem mitigations | Each critical pitfall cites the reference function that proves it. Detection strategies and Clippy guidance are MEDIUM (web) |

**Overall confidence:** HIGH — with the explicit caveat that the highest-value findings are *corrections to the brief*, so PROJECT.md should be amended before the roadmap hardens around it.

### Gaps to Address

- **Spec version v1.0 vs v1.1.0** — unresolved; researchers agree on the facts and differ on emphasis. Decide in M1 planning; it sets `SPEC_VERSION`, the profile range and the falsifiability of the conformance claim.
- **Parameter tick rate** — the blocking open question. Facts gathered, recommendation offered, decision deferred to Parallax. Must be answered before M4 begins; ARCHITECTURE.md's blocks-core-plus-curves-feature layering buys reversibility, not an answer.
- **AOM Patent License 1.0 §1.2 unread** — bears on MIT vs `MIT OR Apache-2.0` (MIT grants no patent licence; both references are BSD-3-Clause-**Clear**, which explicitly withholds patent rights, hence the separate `PATENTS` file). One commit now; needs every contributor's consent later.
- **`iamf-tools` tags `v2.0.0` / `v2.1.0` not fetched** — one may be a v1.1.0-exact tree. Five minutes, worth spending before the type model is written.
- **`libiamf`'s exact v1.1.0/v2.0 internal boundary not traced** — its HEAD parses a Metadata OBU despite a v1.1.0 README. **Mitigation: treat the shipped `tests/*.iamf` vectors as the definition of "accepted", not HEAD source.**
- **ISO-BMFF binding not researched** (M5); **AAC decoder config not read** (out of scope for the encoder, needed by any future decoder); **ambisonics projection config** — only the mono path was read in detail.
- **PROJECT.md edits recommended:** pin v1.1.0; reword Correction #5's numbering; note Corrections #1/#2 describe Eclipsa and the target profiles rather than `iamf-tools` generally; add "do not mirror `iamf-tools` HEAD" as an explicit constraint; restate the `HashMap` guidance as `Vec`-for-wire-order rather than `BTreeMap`.

## Sources

### Primary (HIGH confidence)
- `AOMediaCodec/iamf-tools` @ `901a86e` / `main` (v2.1.0) and tag `v1.0.0` @ `ce54b9a` — BSD-3-Clause-Clear + AOM Patent License 1.0. `obu/obu_header.{h,cc}`, `obu_base.cc`, `types.h`, `ia_sequence_header.cc`, `codec_config.cc`, `audio_element.cc`, `mix_presentation.cc`, `rendering_config.cc`, `audio_frame.cc`, `parameter_block.cc`, `arbitrary_obu.h`, `decoder_config/{lpcm,flac,opus}_*.{h,cc}`, `param_definitions/*`, `common/{leb_generator,read_bit_buffer,write_bit_buffer}.*`, `cli/{profile_filter,obu_sequencer_base,descriptor_obu_parser,temporal_unit_view,global_timing_module,parameter_block_partitioner,channel_label,obu_with_data_generator}.cc`, `cli/testdata/*.textproto`, `CHANGELOG.md`
- `AOMediaCodec/libiamf` @ `e55e183` / `main` — BSD-3-Clause-Clear. `README.md`, `code/src/iamf_dec/obu/iamf_obu.c`, `iamf_decoder.c`, `common/iorw.c`, and the golden bitstreams `tests/test_000003.iamf`, `tests/test_000031.iamf` (`[WIRE]` ground truth)
- **Local execution**, 2026-09-07 — `cargo-deny 0.20.2`, `cargo tree -e normal`, `cargo clippy 1.92.0`, `cargo generate-lockfile` against scratch crates
- crates.io registry API — all versions, licences, MSRVs, editions
- `google/eclipsa-audio-plugin` @ HEAD (Apache-2.0) — `Speakers.h:133–186`, the source of Correction #5's numbering

### Secondary (MEDIUM confidence)
- `alfg/mp4-rust` (MIT), `jam1garner/binrw` (MIT), `pdeljanov/Symphonia` (MPL-2.0, **structure only, nothing ported**) — module-layout comparisons
- docs.rs `bitstream-io`; embarkstudios cargo-deny docs (the `[licenses]` allow-list-only schema change); rust-fuzz book; The Cargo Book (additive features)

### Tertiary (LOW confidence — needs validation)
- Anything tagged `[SPEC]` in FEATURES.md — spec prose or a code comment paraphrasing it. **The IAMF specification itself was deliberately not fetched.** Everything load-bearing is `[ENC]`, `[DEC]` or `[WIRE]`
- `aomediacodec.github.io/iamf/v1.1.0.html`, cited in PITFALLS.md's web sources for the OBU header syntax table — superseded by the source-verified encoding above

**Deliberately NOT consulted, by constraint, in any of the four research passes:** `libspatialaudio` (LGPL-2.1+) and `gpac` (LGPL-2.1) were not cloned, opened, searched or referenced.

---
*Research completed: 2026-09-07*
*Ready for roadmap: yes*
