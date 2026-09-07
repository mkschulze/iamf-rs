# Pitfalls Research

**Domain:** Binary bitstream serialiser/parser for a standardised audio container (IAMF), consumed by a determinism-constrained DAW
**Researched:** 2026-09-07
**Confidence:** HIGH for the IAMF-specific findings (read directly from the permissively-licensed reference implementations), MEDIUM for the Rust-ecosystem findings

## Provenance and Licence Hygiene of This Document

Every IAMF-specific claim below was read from source on 2026-09-07:

| Source | Licence | What was read |
|---|---|---|
| `AOMediaCodec/iamf-tools` @ `main` | BSD-3-Clause-Clear + AOM Patent License 1.0 | `iamf/obu/obu_header.{cc,h}`, `iamf/obu/ia_sequence_header.{cc,h}`, `iamf/obu/codec_config.cc`, `iamf/obu/decoder_config/lpcm_decoder_config.cc`, `iamf/obu/mix_presentation.cc`, `iamf/common/read_bit_buffer.cc`, `iamf/common/leb_generator.{cc,h}`, `iamf/cli/descriptor_obu_parser.cc`, `iamf/cli/obu_sequencer_base.cc`, `iamf/cli/temporal_unit_view.cc`, `iamf/cli/profile_filter.cc` |
| `AOMediaCodec/libiamf` @ `main` | BSD-3-Clause-Clear + AOM Patent License 1.0 | `code/src/iamf_dec/obu/iamf_obu.c`, `code/src/iamf_dec/common/iorw.c` |

**`libspatialaudio` and `gpac` were not consulted.** Nothing in this document derives from an LGPL or GPL source.

**Anything below labelled HIGH was read from code.** Anything labelled MEDIUM or LOW came from web search and should be re-verified before it drives a decision.

---

## Critical Pitfalls

### Pitfall 1: `obu_size` measured from the wrong origin

**Confidence: HIGH — read from `obu_header.cc` and `iamf_obu.c`.**

**What goes wrong:**

The OBU header is one byte of bit-packed fields (`obu_type` 5 bits, `obu_redundant_copy` 1, a type-specific flag 1, `obu_extension_flag` 1), then `obu_size` as a leb128, then — conditionally — the trimming fields and the extension header, then the payload.

`obu_size` counts **everything after the `obu_size` field itself**. Concretely, from `GetObuSizeAndValidate()`:

```
obu_size = size_of(trimming fields, if present)
         + size_of(extension_header_size + extension_header_bytes, if present)
         + payload_serialized_size
```

It excludes the first header byte and it excludes the bytes of `obu_size` itself. `libiamf` reconstructs the whole OBU as `obu_size + size_of(obu_size) + 1`.

There are four plausible wrong answers, and all four produce a file that *nearly* works:

| Wrong choice | Byte error per OBU | How it presents |
|---|---|---|
| Includes the whole header | `+1 + len(obu_size)` | Splitter over-reads; each OBU boundary drifts forward |
| Payload only, excluding trimming/extension fields | `−(trim + ext bytes)` | Correct until the first trimmed or extended OBU, then drifts |
| Includes `obu_size` but not the first byte | `+len(obu_size)` | Drifts by 1–2 bytes per OBU |
| Fixed-width (e.g. always 4 bytes) instead of minimal | 0 — value is right, encoding differs | Decodes fine; fails byte-identity against `iamf-tools` |

**Why it happens:**

`obu_size` cannot be written until the payload is serialised, so it is always computed by a second pass or a backfill. Whichever mechanism you choose, the *origin* of the measurement is a free parameter that the code does not force you to get right, and a single-OBU test file cannot detect an error in it. The brief's own instinct is correct: this is exactly where a guess produces a file that almost works.

**The specific reason it hides:** in a file containing one OBU, the size field is never used to find a boundary — the decoder reads to EOF. The error only appears at the *second* OBU. And `libiamf`'s splitter, on finding `obu_size > remaining bytes`, returns `0`, which callers commonly read as "end of stream" rather than as an error. **A size field that is too large on the final OBU therefore manifests as a shorter file, not as a decode failure.**

**How to avoid:**

1. Serialise the payload into a `Vec<u8>` first; compute `obu_size` from `payload.len() + fields_after_obu_size.len()`; write the header; concatenate. This mirrors the reference and removes the backfill question entirely. Optimise later, if ever — this is an offline encoder.
2. Put the origin in a doc comment on the one function that computes it, citing `obu_header.cc:GetObuSizeAndValidate` and the spec section, so the next reader does not re-derive it.
3. Assert byte-alignment before writing `obu_size`. The reference does this (`WriteTemporalUnit` returns an error if `!wb.IsByteAligned()`), because a size in bytes is meaningless if the writer is mid-byte.
4. Enforce the two IAMF ceilings at the same point: whole OBU ≤ 2 MB (2 097 152) and `obu_size` ≤ 2²¹ − 4.

**Warning signs:**

- Your first test file contains one or two OBUs. **This is the warning sign.** A conformance fixture must contain at least six: IA Sequence Header, Codec Config, Audio Element, Mix Presentation, and several Audio Frames.
- `libiamf` decodes but returns fewer samples than you encoded → size too large somewhere, splitter truncated.
- `libiamf` logs `Reserved OBU type N` → size too small; the splitter landed inside a payload and read a payload byte as an `obu_type`.
- `iamf-tools`' own parser reports `obu_size not valid for OBU flags. Negative remaining payload size.` → your size excludes the trimming or extension fields.

**Detection strategy that actually works:** write a `find_obu_boundaries(&[u8]) -> Vec<(usize, u8)>` helper *before* the first encoder test. Run it over your own output and assert the last boundary lands exactly on `bytes.len()`. A framing error is then a one-line assertion failure rather than an eight-hour hunt through a decoder's logs.

**Phase to address:** M1, before any OBU type beyond the IA Sequence Header exists.

---

### Pitfall 2: Treating the third header bit as `obu_trimming_status_flag` for every OBU type

**Confidence: HIGH — read from `obu_header.cc` (`Validate`, `GetObuTrimmingStatusFlag`, `GetIsKeyFrame`, `GetOptionalFieldsFlag`) and `iamf_obu.c`.**

**What goes wrong:**

The spec's syntax table names bit 6 `obu_trimming_status_flag`, and the handoff repeats that name. **It is not one flag.** In the reference it is called `type_specific_flag` and its meaning is a function of `obu_type`:

| `obu_type` | Meaning of bit 6 |
|---|---|
| 5–23 (Audio Frame) | `obu_trimming_status_flag` |
| 4 (Temporal Delimiter) | `is_not_key_frame` — **inverted sense** |
| 2 (Mix Presentation) | `optional_fields_flag` |
| everything else (0, 1, 3, 24–31) | **reserved, SHALL be 0** |

`obu_redundant_copy` has a matching rule: it is **forbidden** on Audio Frames, Temporal Delimiter and Parameter Block. `iamf-tools` returns `InvalidArgumentError` for either violation.

**Why it happens:**

A tidy Rust model wants one `ObuHeader` struct with a `trimming: Option<Trimming>` field, and that struct is naturally shared by all OBU types. The moment `Trimming` is representable on a Codec Config OBU, someone will eventually set it, and the type system will not object.

**Compounding this:** `libiamf` does *not* validate the reserved case — `_iamf_obu_raw_parse_header` reads `(val >> 1) & 0x01` unconditionally and only *acts* on it for audio frame types. So a file with a reserved bit set decodes perfectly in `libiamf` and is rejected by `iamf-tools`. **Passing M1's `libiamf` gate does not prove conformance on this axis.**

**How to avoid:**

Do not model bit 6 as a `bool` on a shared header. Model it as an enum carried by the payload variant:

```rust
enum TypeSpecific {
    Trimming(Option<Trimming>),   // audio frames only
    IsNotKeyFrame(bool),          // temporal delimiter only
    OptionalFields(bool),         // mix presentation only
    Reserved,                     // serialises as 0, cannot be constructed otherwise
}
```

Make the illegal state unrepresentable rather than validated. Same for `obu_redundant_copy`: if the header is constructed from the payload enum, the three forbidden types simply have no constructor that sets it.

**Warning signs:**

- `ObuHeader` is `#[derive(Default)]` and shared verbatim across all OBU types.
- A test asserts a header round-trips without knowing the OBU type.
- `libiamf` accepts your file but you have never run `iamf-tools`' parser over it.

**Phase to address:** M1 for the model shape; M2 must add the `iamf-tools`-parses-our-output check, because `libiamf` alone cannot catch this class.

---

### Pitfall 3: Trimming fields written start-then-end

**Confidence: HIGH — read from `WriteFieldsAfterObuSize` in `obu_header.cc` and `_iamf_obu_raw_parse_header` in `iamf_obu.c`; both write/read END first.**

**What goes wrong:**

When `obu_trimming_status_flag` is set, the wire order is:

```
leb128 num_samples_to_trim_at_end
leb128 num_samples_to_trim_at_start
```

**End first.** Every prose description, every struct field ordering instinct, and the English phrase "trim the start and the end" push the opposite way.

**Why it happens:** it is counter-intuitive, and it is invisible in the common case. Almost every temporal unit has `trim_at_start = 0` and `trim_at_end = 0`, and both encode to the same single byte `0x00`. **A swapped pair is byte-identical whenever both values are equal — which is almost always.** The bug only surfaces on the first and last frame of a real encode.

**How to avoid:**

- Name the struct fields in wire order and add `// wire order: end before start — verified against iamf-tools obu_header.cc` on the line.
- Make the M1 fixture have a total sample count that is **not** a multiple of the frame size, so the final frame genuinely requires `num_samples_to_trim_at_end > 0`. This single fixture property is what turns a latent bug into a test failure.
- Additionally exercise a non-zero `trim_at_start` (codec priming delay — mandatory for Opus at M3, optional for LPCM). If M1 only ever produces zeros here, M3 will discover the bug.

**Warning signs:**

- The encoder's fixture is an exact multiple of the frame size.
- Decoded output has the right sample count but N samples of the wrong content at one end.
- `libiamf` decodes with correct sample count when both trims are zero, and drifts by exactly `trim_end − trim_start` samples when they differ.

**Additional constraint, same area:** within one temporal unit, `iamf-tools` requires **every** audio frame to carry identical `num_samples_to_trim_at_start`, identical `num_samples_to_trim_at_end`, and identical timestamps. Per-substream trimming is rejected as of v1.1.0. If your model puts trimming on the frame rather than on the temporal unit, you have made per-substream divergence representable.

**Phase to address:** M1 (fixture design and wire order); re-verified in M3 when Opus priming makes `trim_at_start` non-zero for the first time.

---

### Pitfall 4: Assuming leb128 is minimal

**Confidence: HIGH — read from `leb_generator.{cc,h}`, `read_bit_buffer.cc`, `iorw.c`.**

**What goes wrong:**

IAMF's `leb128` is ULEB128: 7 bits per byte, little-endian, `0x80` continuation, **maximum 8 bytes**, decoded value must fit in `u32`. But **non-minimal encodings are legal and the reference encoder emits them on purpose**: `LebGenerator` has a `kFixedSize` mode with `fixed_size ∈ 1..=8`, used so that `obu_size` can be reserved and backfilled at a known width.

Three separate bugs follow from assuming minimality:

1. **Parser rejects valid files.** You error on `0x80 0x00` (padded zero). `iamf-tools` accepts it.
2. **Round-trip test fails on third-party input.** You parse an `iamf-tools` file that used fixed-size mode, re-serialise minimally, and the bytes differ — even though the model is identical. Your "round trip" assertion is now testing something you did not intend.
3. **Byte-identity across targets silently depends on which mode you chose.** If the mode is a runtime config rather than a compile-time constant, two callers get different bytes for the same input.

**How to avoid:**

- **Encoder:** pick minimal encoding, hard-code it, and document why. A `LebMode` knob is a determinism hazard for no benefit in an offline encoder.
- **Parser:** accept 1–8 bytes; return a typed error if the 8th byte still has the continuation bit set; return a typed error if the accumulated value exceeds `u32::MAX`. Both are what `iamf-tools` does.
- **If you ever need byte-exact re-serialisation of a parsed file** (a "remux" or a `probe --rewrite` mode), the parsed model must carry the *encoded length*, not only the value. Decide at M2 whether that is in scope; if not, state in the docs that re-serialisation is value-preserving, not byte-preserving.

**Note the reference asymmetry — this is a genuine trap:** `iamf-tools` errors on a 9th continuation byte and on `u32` overflow. `libiamf`'s `ior_leb128` loops exactly 8 times, does **not** check whether the 8th byte requested continuation, and `ior_leb128_u32` **clamps** an oversized value to `UINT32_MAX` rather than failing. So a malformed length can decode "successfully" in `libiamf` as `0xFFFFFFFF`. If you use `libiamf` as your oracle you will not see this. Do not treat "the reference decoder accepted it" as "it is well-formed".

**Warning signs:**

- Your leb128 decoder has no explicit byte-count bound.
- Your round-trip test is `parse(serialise(model)) == model` only, never `serialise(parse(bytes)) == bytes` — the second direction is where the minimality assumption bites.
- A parsed `iamf-tools` file re-serialises to a different length.

**Phase to address:** M2 (parser); the encoder-side decision in M1.

---

### Pitfall 5: Descriptor ordering and referential integrity

**Confidence: HIGH — read from `descriptor_obu_parser.cc` and `obu_sequencer_base.cc`.**

**What goes wrong:**

A standalone `.iamf` is "descriptors then data", but that is not the whole rule. The reference sequencer writes descriptors in a fixed order:

1. IA Sequence Header
2. Metadata OBUs (type 24)
3. Codec Config OBUs — **sorted ascending by `codec_config_id`**
4. Audio Element OBUs — **sorted ascending by `audio_element_id`**
5. Mix Presentation OBUs — in list order

And the reference *parser* enforces:

- The descriptor run **must begin with an IA Sequence Header**, or parsing fails with `An IA Sequence and/or descriptor OBUs must always start with an IA Header`.
- `ia_code` must be exactly `0x69616D66` (`"iamf"`), checked with `ValidateEqual`.
- `primary_profile` must be a known value. **`additional_profile` is not validated** — see Pitfall 11.
- A duplicate `audio_element_id` is rejected.
- A Mix Presentation referencing an `audio_element_id` not already parsed is rejected: `Mix Presentation OBU references an audio element ID which is not found in the audio element map`. **This makes the ordering load-bearing:** Audio Elements must precede the Mix Presentations that reference them, because the parser resolves forward-only.
- A sub-mix with `num_audio_elements == 0` is rejected.
- A Codec Config payload smaller than 8 bytes is **silently skipped** as presumed corruption, not rejected. Your under-populated Codec Config will vanish, and the Audio Element referencing it will then fail for an unrelated-looking reason.

Within a temporal unit the order is: Temporal Delimiter (if enabled) → Parameter Blocks (sorted by `parameter_id`) → Audio Frames (sorted by audio element ID then substream ID). Substream IDs must be unique within a temporal unit; parameter IDs must be unique within a temporal unit.

**Why it happens:**

Rust's natural model is a struct of `Vec`s, serialised in field order. That is fine — until someone stores the descriptors in a `HashMap<u32, CodecConfig>` for lookup and iterates it to write. Then the order is nondeterministic *and* wrong. Note that `iamf-tools` uses `absl::flat_hash_map` for storage and explicitly calls `SortedKeys(...)` before writing, for exactly this reason. **The reference implementation contains the fix for the determinism bug you are about to write.**

**How to avoid:**

- `BTreeMap<u32, _>` for anything keyed by ID that reaches the byte stream. This satisfies the project's determinism constraint and produces the reference's ordering for free.
- One `serialise_descriptors()` function with the five steps in order and a comment naming `obu_sequencer_base.cc:WriteDescriptorObus` as the source.
- Validate referential integrity **at model construction**, not at serialisation: a `MixPresentation` referencing an unknown `audio_element_id` should be a constructor error, so the caller gets a typed error at the API boundary rather than a decoder error three tools downstream.

**Warning signs:**

- Any `HashMap` or `HashSet` in the crate at all. Make this a `grep` in CI: `grep -rn 'HashMap\|HashSet' src/` must return nothing outside tests.
- IDs assigned in caller order rather than sorted before write.
- No test with two Codec Configs or two Audio Elements — with one of each, ordering is unobservable.

**Phase to address:** M1 (write order, `BTreeMap` choice); M2 (parser-side referential validation and the multi-element fixture).

---

### Pitfall 6: Codec Config fields that must hold exact values

**Confidence: HIGH — read from `codec_config.cc` and `lpcm_decoder_config.cc`.**

**What goes wrong:**

`audio_roll_distance` is a signed 16-bit field in the Codec Config whose value is **not free** — it is determined by the codec and, for Opus, by the frame size. `iamf-tools` validates it with `ValidateEqual` against a per-codec required value. For LPCM the required value is **0**. Get it wrong and the OBU is rejected outright, with an error that talks about roll distance rather than about the field you actually mis-set.

The LPCM decoder config is similarly closed:

| Field | Legal values | Width |
|---|---|---|
| `sample_format_flags` | `0` (big-endian) or `1` (little-endian) — **nothing else** | 8 bits |
| `sample_size` | `16`, `24`, `32` — **not 8, not 64** | 8 bits |
| `sample_rate` | `16000`, `32000`, `44100`, `48000`, `96000` — **not 88200, not 192000** | 32 bits |
| `audio_roll_distance` | exactly `0` for LPCM | 16 bits, signed |
| `num_samples_per_frame` | non-zero; the reference caps it at 1 s at 96 kHz | leb128 |

**Note `sample_format_flags = 0` means big-endian.** LPCM samples in an IAMF Audio Frame are big-endian unless you say otherwise, which is the opposite of what a WAV-shaped mental model expects. Getting this backwards produces a file that decodes without error and sounds like white noise — the most expensive kind of bug, because it looks like a DSP problem.

Additionally, `iamf-tools` refuses a set of Codec Configs with differing sample rates or bit depths: *"Codec Config OBUs with different bit-depths and/or sample rates are not in base-enhanced/base/simple profile; they are not allowed in ISOBMFF."* If the API lets a caller build a project with two elements at different rates, that is an error to raise at the API, not at serialisation.

**How to avoid:**

- Model each of these as a Rust enum (`SampleSize::Bits16 | Bits24 | Bits32`, `SampleRate::Hz48000 | ...`), not as `u8`/`u32`. Illegal values then cannot be constructed, and the "what does 0 mean" question is answered by the variant name: `Endianness::Big` / `Endianness::Little`.
- Derive `audio_roll_distance` from the codec rather than accepting it as a parameter. The reference has an `automatically_override_roll_distance` flag; you have no reason to expose the manual path.
- Validate the common-sample-rate constraint once, in the function that assembles the sequence.

**Warning signs:**

- Any of these fields typed as a bare integer in the public API.
- The M1 fixture is silence or a constant — see Pitfall 13; endianness inversion is inaudible in silence and invisible in a DC signal.
- No test at a non-48 kHz rate and no test at 24-bit.

**Phase to address:** M1 for LPCM; M3 repeats the whole exercise for FLAC and Opus, where roll distance is frame-size-dependent for Opus.

---

### Pitfall 7: Flag bits decoupled from the fields they gate

**Confidence: HIGH — read from `mix_presentation.cc` and `obu_header.cc`.**

**What goes wrong:**

IAMF is full of bitmask-gated optional fields. The Mix Presentation loudness block is the clearest example: an 8-bit `info_type` bitmask, then two mandatory `signed(16)` values, then `true_peak` **only if** `info_type & kTruePeak`, then anchored loudness **only if** `info_type & kAnchoredLoudness`, then an extension blob **only if** `info_type & kAnyLayoutExtension`.

If the flag and the field can disagree in your model, every byte after the disagreement shifts. And because the shift happens *inside* a payload, `obu_size` is still self-consistent — the OBU boundary is correct, so the framing checks all pass, and the corruption is confined to a field the decoder interprets as garbage.

The same coupling exists in the OBU header (`obu_extension_flag` gates `extension_header_size` + bytes) and throughout the Audio Element and Parameter Block.

**Why it happens:**

`struct Loudness { info_type: u8, true_peak: i16, ... }` compiles. Nothing forces `info_type` to agree with which fields are meaningful. The reference has the same weakness in C++ and papers over it with validation; Rust does not have to.

**How to avoid:**

Derive the flag from the data, never store it:

```rust
struct Loudness {
    integrated: FixedQ7_8,
    digital_peak: FixedQ7_8,
    true_peak: Option<FixedQ7_8>,
    anchored: Option<AnchoredLoudness>,
    extension: Option<Vec<u8>>,
}
impl Loudness { fn info_type(&self) -> u8 { /* computed */ } }
```

The flag byte becomes a function of the model. It cannot disagree. Apply the same rule to `obu_extension_flag` (derive from `Option<Vec<u8>>` — this is what `iamf-tools` does in `GetExtensionHeaderFlag()`) and to every other gate.

**Warning signs:**

- A public struct has both a flag field and the field it gates.
- A serialiser writes `if self.flags & X != 0 { write(self.y) }` where `y` is not an `Option`.
- Round-trip passes but a field comes back as a default value — a sign the write was skipped and the read consumed something else.

**Phase to address:** M1 for the header and Mix Presentation; the rule must be stated once and applied to every OBU as it is added.

---

### Pitfall 8: Bit-level I/O errors that byte-level tests cannot see

**Confidence: HIGH for the IAMF specifics; MEDIUM for the general detection strategies.**

**What goes wrong:**

Five distinct failure classes, all of which produce plausible-looking bytes:

1. **Bit order within a byte.** IAMF packs the header MSB-first: `obu_type` occupies bits 7–3. `libiamf` extracts it as `(val >> 3) & 0x1f`. A LSB-first writer produces a valid-looking byte with the wrong type. Detection: assert the exact first byte of a known OBU against a hand-computed constant.
2. **Endianness of multi-byte fields vs. bit order.** `sample_rate` is `unsigned int (32)`, written big-endian (network order) by the bit writer, while LPCM *sample data* endianness is controlled by `sample_format_flags` and defaults to big-endian. Two different endianness questions, one struct apart. Detection: separate types for "bitstream integer" and "sample datum" so a conversion is required to cross between them.
3. **Off-by-one in bit counts.** `obu_type` is 5 bits, not 4 or 6. Reading 6 steals the redundant-copy bit; every subsequent field is shifted by one bit for the rest of the OBU. Detection: a table-driven test that asserts each field's bit width against the spec table, in one place, so the widths are data rather than scattered literals.
4. **Alignment after a non-aligned field.** IAMF payloads are byte-aligned at OBU boundaries, but internal structures pack sub-byte fields and then rely on explicit reserved-bit padding to re-align. Forgetting the padding leaves the writer mid-byte; the next `write_u8` then straddles two bytes. `iamf-tools` guards this with `IsByteAligned()` checks. Detection: your `BitWriter` must expose `is_byte_aligned()`, and the OBU-level serialiser must assert it before computing `obu_size` and after finishing the payload. **This assertion catches nearly every bit-count error in the payload, cheaply.**
5. **Signed vs unsigned extraction.** Loudness fields are `signed(16)` in two's complement. Reading them into a `u16` and casting with `as i16` happens to work in Rust; reading into a `u32` first and then casting loses the sign. `audio_roll_distance` is likewise `i16` and is normally *negative* for real codecs. Detection: type the reader as `read_i16()` returning `i16` — never expose a "read N bits into u64 and cast yourself" API for signed fields.

**How to avoid:**

- Build the `BitReader`/`BitWriter` as the first commit of M1, with `is_byte_aligned()`, `bits_remaining()`, and separate signed/unsigned methods. Unit-test it in isolation with hand-computed vectors before any OBU exists.
- Keep every field's bit width in one table, referenced by both the reader and the writer, so a width can only be wrong in both directions at once (which the round trip then hides — hence the next point).
- **Assert absolute byte offsets, not just round-trip equality.** For the M1 fixture, commit a test that says "after writing the IA Sequence Header, the writer is at byte offset 8" and "the Codec Config OBU starts at byte 8 and its `obu_type` byte is `0x00`". Offsets are the only bit-level property a round trip cannot fake.

**Warning signs:**

- The bit writer has no alignment query.
- Field widths appear as magic numbers at each call site.
- Every test is a round trip; no test asserts a literal byte.

**Phase to address:** M1, first commit.

---

### Pitfall 9: Determinism leaks

**Confidence: HIGH for the IAMF-specific vectors; MEDIUM for the general Rust mechanisms.**

**What goes wrong:**

The crate must produce byte-identical output on macOS arm64, macOS x86_64, Windows MSVC and Linux x64, run to run. The realistic leak vectors, in descending order of likelihood for *this* crate:

| Vector | How it reaches output | How it is caught |
|---|---|---|
| **`HashMap`/`HashSet` iteration** | Descriptors keyed by ID, substream IDs, parameter IDs. `RandomState` is seeded per process, so order differs **run to run on the same machine** | `grep -rn 'HashMap\|HashSet' src/` in CI; encode the same fixture twice in one process and diff |
| **Float → fixed-point rounding** | Loudness arrives from the caller as `f32`/`f64` LUFS and must become `signed(16)` Q7.8. `(x * 256.0) as i16` **truncates toward zero**, which is asymmetric about zero — and every integrated loudness value is negative, so truncation biases every value upward by up to 1 LSB | Property test: quantise ±values, assert symmetry; assert `-23.0 LUFS` maps to exactly `-5888` |
| **`sort_unstable` on a non-unique key** | Sorting audio frames by audio element ID when two frames share one | Sort by the full `(element_id, substream_id)` tuple, which is unique; or use `sort_by_key` (stable) |
| **Platform transcendentals** | Only if dB↔linear conversion appears (`10f64.powf(db/20.0)`). This is the single most likely place maths sneaks in | CI grep for `powf|powi|log10|exp|sin|cos|sqrt` in `src/`; use `libm` if any is genuinely needed |
| **Timestamps / paths / tool strings** | IAMF has a Metadata OBU carrying free-form `name`/`value` tag pairs, and Mix Presentation annotations. A "created by iamf-rs 0.1.2 at <time>" tag is byte-identity poison across versions and runs | Forbid the crate from originating any string not supplied by the caller |
| **`#[derive(Hash)]` + hash-ordered container** | Only matters if the hash feeds a `HashMap` that reaches output — the derive itself is fine | Covered by the `HashMap` grep |
| **Iteration over `std::fs::read_dir`** | Only in tests/fixtures, but a fixture-ordering difference makes CI flaky and wastes a day | Collect and sort before use |

`f64::to_string` and `format!` use a deterministic algorithm in `core` and are safe. Integer arithmetic and IEEE-754 `+ - * / sqrt` are bit-reproducible across these targets. The danger is confined to the table above.

**How to avoid — the CI shape that actually catches these:**

1. **Same-process double-encode.** In the unit test suite: encode a fixture twice in one process and `assert_eq!` the bytes. This is the only cheap check that catches `HashMap` seeding, because the seed is per-process and CI often runs one target.
2. **Cross-target hash job.** A matrix over the four targets; each builds and encodes a fixed fixture set and emits `sha256`; a final job asserts all four sets are identical. **This must exist from the first release** — it is a stated constraint, and it is worthless if added later, because by then it just documents whatever the code does.
3. **Static tripwires.** `grep` bans on `HashMap`, `HashSet`, and unguarded transcendentals, run as a CI step. Crude, and they work.
4. **Explicit quantisation helper.** One `fn lufs_to_q7_8(x: f32) -> Result<i16, Error>` using `round_ties_even`, range-checked, unit-tested at the boundaries. Not `as i16` at three call sites.

**Warning signs:**

- Determinism is asserted in the README but not in `.github/workflows/`.
- Any test that is flaky in CI and passes locally.
- A cross-target job exists but only compares "it built".

**Phase to address:** M1 for `BTreeMap` and the CI matrix; M4 for the loudness quantisation rule, since that is where the caller's floats enter — but write the helper in M1 so the rule exists before there is pressure to inline it.

---

### Pitfall 10: A parser written for well-formed input

**Confidence: HIGH for the attack surface (enumerated from the reference's own bounds checks); MEDIUM for the Rust mitigations.**

**What goes wrong:**

Every length field in IAMF is attacker-controlled: `obu_size`, `extension_header_size`, `info_type_size`, `num_samples_per_frame`, `num_anchor_elements`, `num_substreams`, `num_layers`, `num_audio_elements`, `num_labels`, and every count that opens a loop. The classes:

1. **Unbounded allocation.** `Vec::with_capacity(n)` or `vec![0u8; n]` where `n` is a parsed leb128 up to `u32::MAX` → a 4 GB allocation from a 5-byte input. **The reference shows the correct pattern:** in `mix_presentation.cc` it calls `ValidateInRange(info_type_size, {0, kEntireObuSizeMaxTwoMegabytes})` *before* `resize`. In `obu_header.cc` it explicitly comments *"Avoid reading in the (possibly large) extension, until we know it is plausible."*
2. **Integer overflow in size arithmetic.** `header_len + obu_size`, `offset + payload_len`, `obu_size − trim_bytes − ext_bytes`. The reference computes the last one in `int64_t` and then checks `< 0`, because the subtraction genuinely can go negative on hostile input. In Rust, release-mode `+`/`-` wraps silently, and a wrapped length then indexes a slice.
3. **Loops driven by a count with no per-element cost check.** `for _ in 0..num_anchor_elements` with `num_anchor_elements: u32` and a 1-byte body: 4 billion iterations, each pushing to a `Vec`. Not an infinite loop, but indistinguishable from one.
4. **Zero-length elements.** A container loop that advances by a parsed length can fail to progress if the length is 0. IAMF's minimum OBU is 2 bytes, so the top-level OBU loop always advances — but any *inner* loop over variable-length items needs an explicit progress assertion.
5. **Slice indexing panics.** `data[i]`, `&data[a..b]`, `copy_from_slice` with mismatched lengths. A panic in a library is a denial of service for the host DAW.
6. **Stack overflow from nesting.** IAMF's grammar is not deeply recursive — Mix Presentation → sub-mix → audio element → layout is bounded. This is the *least* likely class here. But if extension parsing is ever made recursive, cap the depth.

**How to avoid — beyond the `unwrap()`/`expect()` ban:**

The `unwrap()` ban is necessary and nowhere near sufficient. Add, in the crate root:

```rust
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing)]        // no data[i], no &data[a..b]
#![deny(clippy::arithmetic_side_effects)] // no bare +/-/* on parsed values
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![deny(clippy::integer_division, clippy::cast_possible_truncation)]
```

These are `restriction`-category Clippy lints — deliberately noisy, meant to be chosen individually. `indexing_slicing` and `arithmetic_side_effects` are the two that matter most here, and they are exactly the two people disable first because they are annoying. Scope them to the parser module rather than disabling them, and allow them in `#[cfg(test)]`.

Then the structural rules:

- **The reader owns the bound.** Every read goes through a `BitReader` that knows `bits_remaining()` and returns `Err(UnexpectedEof)` — no call site ever sees a raw slice.
- **Never allocate from a parsed length directly.** Cap against remaining input: an `n`-element list where each element costs ≥1 byte cannot have `n > reader.bytes_remaining()`. Check this *before* reserving. This one rule kills classes 1 and 3 together.
- **`checked_add`/`checked_sub`/`checked_mul` on every size computation**, or a newtype `ByteLen(u32)` whose arithmetic is checked by construction. `arithmetic_side_effects` will force this.
- **A crate-level configurable cap** on total allocation for one parse, defaulting to something like 64 MB, so a hostile file cannot exhaust the DAW's memory even with individually-plausible fields.

**What a good fuzz harness for this looks like:**

```rust
// fuzz_targets/obu_parse.rs — robustness
fuzz_target!(|data: &[u8]| {
    let _ = iamf::parse_sequence(data);  // must not panic, must terminate
});
```

That is the floor, not the target. The harness that finds real bugs is the **round-trip differential**:

```rust
// fuzz_targets/obu_roundtrip.rs
fuzz_target!(|data: &[u8]| {
    if let Ok(model) = iamf::parse_sequence(data) {
        let bytes = model.serialise().expect("a parsed model must serialise");
        let reparsed = iamf::parse_sequence(&bytes).expect("our own output must parse");
        assert_eq!(model, reparsed);   // parse ∘ serialise ∘ parse == parse
    }
});
```

This asserts an invariant, so it fails on *wrong* behaviour rather than only on crashes — which is the difference between a fuzzer that finds one panic in week one and then nothing, and one that keeps earning its CPU time.

Plus a structure-aware generator target using `arbitrary::Arbitrary` on the model, which reaches deep validation paths that random bytes never will, since random bytes almost never produce a valid `ia_code`.

**What makes a fuzz harness bad:**

| Anti-pattern | Why it fails |
|---|---|
| No seed corpus | Random bytes essentially never produce `0x69616d66`, so the fuzzer never gets past the IA Sequence Header and explores 0.1% of the code |
| Discarding the result (`let _ = parse(data);` and nothing else) | Only finds panics; misses every logic error |
| `-max_len` left at a default below the interesting lengths | The multi-byte leb128 paths are never reached |
| Only run manually, before a release | A fuzz target that is not in CI is a fuzz target that stops working and nobody notices |
| No `-rss_limit`/OOM handling | The first OOM finding masks everything behind it |
| Harness in the same crate as the code, gated behind a feature that CI does not build | Bit-rots within one milestone |

**Seed the corpus with:** the M1 output files, every `iamf-tools` test vector you can obtain, and hand-crafted near-misses (correct `ia_code`, one field wrong). Commit the corpus. It is the harness's most valuable component.

**Warning signs:**

- `cargo fuzz` exists but is not a CI job.
- The fuzz target compiles but `fuzz/corpus/` is empty or gitignored.
- The parser takes `&[u8]` and indexes it directly anywhere.
- `#[allow(clippy::indexing_slicing)]` appears in the parser.

**Phase to address:** M2 — the constraint is explicit that the fuzz target ships with the parser and may not slip. The *lint configuration* should land in M1 so the encoder is written under it too.

---

### Pitfall 11: Spec-version drift

**Confidence: HIGH — the drift is already observable between `HANDOFF.md` and `iamf-tools` `main` as of 2026-09-07.**

**What goes wrong:**

`HANDOFF.md` §4 records facts read from `iamf-tools` on 2026-09-07, including "three codecs shipped, not four" and profiles Simple/Base/Base-Enhanced. Reading `iamf-tools` `main` today:

- `ProfileVersion` has **six** values: `Simple = 0`, `Base = 1`, `BaseEnhanced = 2`, `BaseAdvanced = 3`, `Advanced1 = 4`, `Advanced2 = 5`. `libiamf`'s profile limit table confirms: `{1,16,…}`, `{2,18,…}`, `{28,28,…}`, `{18,18,…}`, `{18,18,…}`, `{28,28,…}`.
- `iamf/cli/codec/aac_encoder.cc` **exists** in the tree.
- Source comments say "IAMF v1.1.0 imposes a maximum size…" and "All frames must have the same trimming information and timestamps **as of IAMF v1.1.0**".
- The spec is published at `v1.1.0.html`.

So the handoff's verified facts are already a version behind, and it does not say which version they were verified against. **That is the failure mode in miniature.**

What goes wrong downstream when a version is not pinned:

- You write against `main`, emit `primary_profile = 3` (Base-Advanced), and a v1.0 decoder returns `Unexpected profile_version` — a file that is conformant to one version and rejected by another, with no way to tell which you meant.
- You validate `additional_profile` against a v1.0 list and reject a valid v1.1 file. (Note: `iamf-tools` does **not** validate `additional_profile` at all. Do not be stricter than the reference on a field whose purpose is forward compatibility — that is precisely the field designed to carry a value you do not recognise.)
- Your conformance claim ("`libiamf` accepts it") is unfalsifiable because "`libiamf`" is a moving target.

**How to avoid:**

1. **`pub const SPEC_VERSION: &str` in a `spec` module**, with every spec-derived constant defined in that one module, each with a doc comment citing its spec section. Decide now whether the target is v1.0 (what the certification programme cites) or v1.1.0 (what the reference implements) — and if they differ, target v1.0 and record which v1.1 features are deliberately unemitted.
2. **Pin the reference commits.** A `REFERENCES.md` recording the exact `iamf-tools` and `libiamf` git SHAs verified against, and the CI job that builds `libiamf` for M1 must check out that SHA — not `main`. Otherwise a green CI can turn red from an upstream commit and you will debug your own code for a day.
3. **Bumping a pin is a task, not a drive-by.** The bump commit re-runs the whole conformance suite and updates `REFERENCES.md` in the same commit.
4. **Forward compatibility in the parser:** reserved OBU types (25–30) and unknown `additional_profile` values must be skipped or carried, not rejected. `iamf-tools` bypasses reserved OBUs with a warning and seeks past them using `total_obu_size`; copy that behaviour.

**Warning signs:**

- A version number appears in the README but nowhere in the code.
- CI clones `iamf-tools`/`libiamf` at `main`.
- The parser rejects an unknown enum value anywhere the spec says a decoder should ignore it.

**Phase to address:** M1 — this is one commit's work at the start and a multi-day archaeology exercise later.

---

### Pitfall 12: Licence contamination and the patent gap

**Confidence: HIGH for the licence facts (read from the source headers); MEDIUM for the tooling specifics.**

**What goes wrong:**

Three distinct risks, commonly conflated:

**(a) Reading forbidden source.** `libspatialaudio` (LGPL-2.1+) and `gpac` (LGPL-2.1) may not be read. The realistic breach is not deliberate — it is a search result. Someone searches "ISO-BMFF IA sample entry" at M5, lands in a `gpac` file on GitHub, reads twenty lines to understand the box layout, and the contamination is done and undocumented. **Contamination is irreversible: relicensing later requires every contributor's agreement.**

**(b) A transitive dependency that violates Parallax's `deny.toml`.** The concrete exposure is M3. `libopus` is BSD-3-Clause and `libFLAC` (the library) is BSD-3-Clause — both fine. The risk is one layer up: the `*-sys` wrapper crate's *own* licence, whatever it vendors, and its build-dependencies. A wrapper that vendors the FLAC *command-line tools* (GPLv2) alongside the library, or that pulls a copyleft build helper, fails the allow-list. Discovering this at Parallax integration means M3 is wasted.

**(c) The patent gap.** Both references are **BSD-3-Clause-Clear** — the "Clear" variant explicitly withholds patent rights. It is a copyright licence and nothing more, which is why the AOM Patent License 1.0 ships as a separate `PATENTS` file. Meanwhile **MIT grants no patent licence**, which is the whole reason the Rust ecosystem's convention is `MIT OR Apache-2.0`. Shipping an implementation of a patent-pooled standard under MIT alone gives downstream users no patent position from you at all.

**How to avoid:**

| Risk | Mechanism | Where |
|---|---|---|
| (a) | `CONTRIBUTING.md` with a **named** allow/deny list of readable repositories, and a PR template checkbox: "I did not consult `libspatialaudio` or `gpac`" | M1 |
| (a) | `REFERENCES.md` recording, per module, which reference file informed it — this is also the attribution BSD-3-Clause-Clear requires | M1, updated per milestone |
| (a) | A `NOTICE` file reproducing the AOM copyright notice and the BSD-3-Clause-Clear text | M1 |
| (b) | `cargo deny check` in CI from the first commit, using **Parallax's `deny.toml` verbatim** — copy it, do not write a new one, or the two will drift and the check becomes theatre | M1 |
| (b) | State each dependency's licence in a comment on the line that adds it in `Cargo.toml` | every milestone |
| (b) | Before M3 starts, run `cargo deny check licenses` on a throwaway branch that merely *adds* the candidate codec crates. Ten minutes, and it decides the M3 approach | M3 planning |
| (c) | Read AOM Patent License 1.0 §1.2 and record the finding; adopt `MIT OR Apache-2.0` unless §1.2 gives a reason not to | M1 — it is one commit now, and needs every contributor's consent later |

**On `cargo deny` specifically:** the default configuration does not necessarily cover `build-dependencies` and `dev-dependencies` the way you would assume, and crates with no SPDX expression (`NOASSERTION`) require explicit `clarify` entries rather than being silently allowed. Verify the configuration actually fails on a known-bad crate before trusting it — add an LGPL crate temporarily, confirm CI goes red, remove it. **A licence check that has never failed is not known to work.**

**Warning signs:**

- `cargo deny` is in CI but has never once failed.
- `deny.toml` was written here rather than copied from Parallax.
- A PR adds a dependency with no licence noted.
- Anyone mentions "I found how gpac does it".

**Phase to address:** M1 for all the mechanisms; M3 is the milestone with real dependency exposure; M5 (ISO-BMFF) is the milestone with real reading-contamination exposure, since that is where `gpac` is the obvious reference.

---

### Pitfall 13: Testing that proves only self-consistency

**Confidence: HIGH — this is the failure mode the brief names, and the reference's own test structure supports the prescription.**

**What goes wrong:**

Four tests that look like coverage and prove nothing:

1. **Round-trip only.** `parse(serialise(model)) == model` passes perfectly when the serialiser and parser share a misunderstanding. If both put `trim_at_start` before `trim_at_end`, the round trip is green forever. Round-trip tests measure internal consistency, which is not the property under test.
2. **Per-field unit tests that never assemble a file.** Testing that `CodecConfig` serialises to the right 12 bytes proves nothing about `obu_size`, nothing about ordering, nothing about alignment across OBU boundaries. All of Pitfalls 1–5 live between the fields, not in them.
3. **Golden files generated by the code under test.** The classic: run the encoder, eyeball the hex, commit it as `expected.iamf`. This freezes the current behaviour, bug included, and converts every future correct fix into a test failure that someone will "fix" by regenerating the golden.
4. **Silence as the test signal.** A silent or DC fixture decodes sample-identically even when the channel order is wrong, the endianness is inverted, the trimming is swapped, or the frame count is off by a frame of zeros. It tests almost nothing, and it is the default choice.

**The minimum test that actually proves conformance** — this is M1's exit criterion, and each clause exists because it catches something the others miss:

1. **Encode a per-channel-distinguishable, non-silent signal.** Channel *n* carries a tone at a distinct frequency, or a distinct DC offset, or a distinct PRNG stream seeded by *n*. This makes channel order, endianness, and interleaving errors visible. A single tone on all channels does not.
2. **Total sample count that is not a multiple of the frame size**, so the final frame requires `num_samples_to_trim_at_end > 0` and the trimming path is exercised at all.
3. **At least six OBUs**, including two Codec Configs or two Audio Elements, so ordering and framing are observable.
4. **Decode with `libiamf` at a pinned commit, and assert three things:** the decode returned OK, **the returned sample count equals what was encoded** (this is what catches silent truncation from an oversized `obu_size`), and the PCM is sample-identical.
5. **Parse the same file with `iamf-tools`' own parser** and assert it reports no error. This is the only check that catches reserved-bit misuse and non-minimal-leb128 rejection, because `libiamf` is permissive where `iamf-tools` is strict.
6. **Byte-diff against an `iamf-tools`-produced file for an identical configuration.** This is the strongest available signal and it is *achievable*, because `iamf-tools`' descriptor write order is fixed and documented (Pitfall 5) and its default leb128 mode is minimal. The exit bar: either the files are identical, or **every** difference is enumerated in writing with a reason. "Mostly the same" is not a result.
7. **Parse an `iamf-tools`-produced file into our model** and assert the model matches what we would have built. This is M2, but design the M1 fixture so the same configuration is reproducible in `iamf-tools`' proto input format.

Everything else — round trips, property tests, fuzzing — is valuable *on top of* this, and worthless as a substitute.

**Warning signs:**

- The test suite is green and no test invokes an external binary.
- `tests/fixtures/*.iamf` were produced by this crate.
- The test signal is silence, a constant, or the same tone on every channel.
- The fixture's sample count is a round multiple of 1024.

**Phase to address:** M1 defines the conformance harness; M2 adds direction (7); M3 reuses the identical harness for FLAC and Opus, which is only possible if M1 built it as a reusable function rather than as one inline test.

---

### Pitfall 14: Building the parser, the codecs or the API before M1 is verified

**Confidence: HIGH — this is the direct consequence of Pitfalls 1–5 being latent.**

**What goes wrong:**

Suppose the `obu_size` origin is wrong (Pitfall 1) and it is discovered at M4. The *code* fix is small — one function, an afternoon. The cost is everything downstream of it:

| Artefact | Cost of a late header-encoding fix |
|---|---|
| `write_obu()` | Hours. This is the cheap part, and it is the part people estimate |
| Every committed golden/fixture file | Regenerate and re-verify all of them |
| Every unit test asserting literal bytes | Every expected array is wrong |
| The M2 parser | It mirrors the encoder's misunderstanding, so it was **passing** its round-trip tests. Both sides change |
| The M2 fuzz corpus | Seeded with invalid files. The fuzzer has spent its entire runtime exploring a grammar that does not exist — all accumulated coverage is worthless |
| M3 FLAC/Opus fixtures | Regenerate, re-verify against `libiamf`, twice |
| The M4 API | May have exposed a size type that turns out to need to be "encoded length" rather than "value" (Pitfall 4). A breaking change to the Parallax-facing surface |
| Confidence | Every previously-passing conformance claim is now unproven and must be re-run |

And the **silent** version is worse. Because `libiamf` is permissive — it ignores reserved bits, clamps oversized leb128s, and returns `0` from the splitter in a way callers read as end-of-stream — a subtly wrong file can pass M1's gate and fail at YouTube ingest or at certification, after M2–M4 are built on it.

**Why the temptation is strong:** the parser is the more interesting problem; the API is the part Parallax actually calls, so it feels like the highest-value work; and codecs are concrete and bounded. Meanwhile M1 is "get one boring LPCM file to decode", which sounds like a day's work and is not, because it is the milestone that has to discover every fact in Pitfalls 1–8.

**How to avoid:**

- Treat M1's exit criterion as Pitfall 13's seven clauses, not as "it compiles" or "libiamf returned 0".
- **Build the conformance harness as a reusable function in M1, not as a test body.** `assert_conformant(config, pcm)` invoked once in M1 is invoked four more times in M3 for free. Written as an inline test, it gets copy-pasted and diverges.
- Explicitly forbid starting M2 until M1's byte-diff against `iamf-tools` is either identical or fully explained in writing.
- If M1 turns out to need three weeks, that is M1 telling you something true about the problem. Reordering does not make the work smaller; it makes it discoverable later, when it is more expensive.

**Warning signs:**

- A parser exists before a `libiamf`-verified file exists.
- M1 was declared done on "`libiamf` returned success" without a sample-count assertion.
- The word "temporarily" appears next to a decision about the header encoding.

**Phase to address:** M1, by definition. The constraint is already recorded in `PROJECT.md` ("M1 may not be reordered") — the value here is knowing the *size* of the bill, so the constraint survives contact with a deadline.

---

### Pitfall 15: Scope creep into DSP

**Confidence: HIGH for the pressure points (derived from the milestone structure and the reference's own scope); MEDIUM for the boundary mechanisms.**

**What goes wrong:**

The crate acquires a resampler, then a gain stage, then a downmixer, then it is a renderer — in a repository whose brief says a PR adding DSP is in the wrong place.

**Where the pressure actually comes from, in the order it will arrive:**

| Milestone | The pressure | Why it feels reasonable |
|---|---|---|
| M1 | Layout enumeration. `Layout::Surround7_1_4` looks like it should know it has 12 channels *at these positions* | It legitimately needs the channel **count** and the channel **label order**. It does not need a position. Countable ≠ placeable |
| M3 | Sample-rate mismatch. Opus is natively 48 kHz; the caller hands you 44.1 kHz | "We just need a little resampler." This is the single most likely first breach |
| M3 | Opus priming delay. Correct `num_samples_to_trim_at_start` requires knowing the encoder's delay | Reading the delay from the codec is fine. *Compensating* for it by shifting samples is DSP |
| M4 | Loudness. The Mix Presentation must carry integrated loudness; you already hold the PCM | `MON-05` says Parallax owns the BS.1770 meter, sharing code with export normalisation. Two meters means two answers |
| M5 | Scalable layers. Recon gain and demixing weights **are** DSP, unavoidably | This is the real boundary test, and it is the reason the feature is deferred |
| M5 | The decoder. Decoded substreams have to be rendered to *some* layout to be listenable | The decoder's job is to produce substreams plus metadata. Rendering is `D-40`'s |

**The boundary that holds:**

State it as a property of the data, not as a rule about intent, so it is checkable:

> **No coefficient may exist in this crate.** A layout is a label and a channel count. A gain is a number the caller supplied that we quantise and write. No array of filter taps, no gain table, no window function, no interpolation between two values, no transcendental.

Enforced by:

- A CI grep over `src/` for `powf|powi|log10|log2|exp|sin|cos|tan|sqrt|hypot`, allow-listed only in the one loudness-quantisation module (which needs none of them — `x * 256.0` and `round_ties_even` suffice, so the allow-list can be empty).
- A `CONTRIBUTING.md` section: "This crate does not render. If your change computes a value from audio samples, it belongs in Parallax."
- **The correct answer to the M3 resampler pressure:** a typed error. `Error::SampleRateNotSupportedByCodec { got, supported }`. The caller resamples with Parallax's resampler, which is already deterministic and already `libm`-based. Two resamplers means two results and a determinism failure across the Parallax/iamf-rs boundary.
- **The correct answer to the M5 scalable-layers pressure:** demixing weights and recon gain are *values carried in the bitstream*. This crate serialises them; whoever computes them supplies them. If nobody supplies them, the feature is not ready — which is exactly why single-layer is the shipping configuration and the reference itself writes one layer.

**Warning signs:**

- A `dsp`, `render`, `mix` or `resample` module appears.
- `libm` is added as a dependency for something other than a quantisation helper.
- A layout enum grows an `azimuth`/`elevation` field.
- The public API accepts per-source positions (see `PROJECT.md` open question 5 — the API must not pretend to express something IAMF-as-practised cannot).

**Phase to address:** M1 states the rule and the grep; M3 and M5 are where it is tested.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|---|---|---|---|
| `HashMap` for descriptor lookup "just internally" | Familiar, slightly faster | It leaks to output the moment someone iterates it; violates a stated constraint; the failure is intermittent and target-dependent | **Never.** `BTreeMap` costs nothing at these sizes |
| Fixed-width leb128 so `obu_size` can be backfilled in place | No second serialisation pass | Bytes differ from `iamf-tools` for the same input, so the strongest conformance check (byte-diff) is unavailable forever | Never for the encoder. Fine as a *parser* capability |
| Golden files generated by this crate | Instant regression coverage | Freezes bugs; every real fix looks like a regression | Only for *stability* tests explicitly labelled "asserts output has not changed", never for *conformance* |
| `unwrap()` behind "this can't fail here" | Shorter code | A parser panic is a DoS in the host DAW | Never outside `#[cfg(test)]` — already a stated constraint |
| `#[allow(clippy::indexing_slicing)]` in the parser | Unblocks an afternoon | Reintroduces the entire panic class the lint exists to prevent | Never in the parser. Acceptable in test code |
| Silence as the M1 test signal | Trivial to generate | Hides channel order, endianness, trimming and frame-count errors — four of the six most likely bugs | Never as the *only* signal; fine as an additional case |
| Deferring the cross-target CI matrix "until there's something to test" | Faster first release | Added later, it merely documents whatever the code already does; a determinism bug introduced in M1 is baselined as correct | Never — the constraint says "from the first release" |
| Skipping `cargo deny` until dependencies exist | Nothing to check yet | The check is never exercised, so it is not known to work when M3 adds real dependencies | Add it at M1 *and* prove it fails on a known-bad crate |
| One `ObuHeader` struct shared by all types with a `bool` flag | Simple, mirrors the spec table | Makes reserved-bit misuse representable; `libiamf` will not catch it | Never — the enum costs twenty lines |
| Taking `f32` LUFS at the API and quantising at the call site | Fewer types | Rounding rule diverges between call sites; asymmetric truncation biases negative values | Never — one helper, unit-tested at boundaries |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|---|---|---|
| `libiamf` as the M1 oracle | Treating "decoded OK" as "conformant". It ignores reserved bits, clamps overlong leb128 to `UINT32_MAX`, and its splitter returns `0` on `obu_size > remaining` — which callers read as end-of-stream, so an oversized size field silently truncates | Assert decode-OK **and** sample count **and** PCM equality; then separately run `iamf-tools`' parser, which is the strict one |
| `libiamf` / `iamf-tools` in CI | Cloning `main`. An upstream commit turns CI red and you debug your own code | Pin exact SHAs in `REFERENCES.md`; the CI job checks out those SHAs. Bumping is a deliberate task |
| `iamf-tools` for byte-comparison | Comparing against output generated with a non-default `LebGenerator` mode, then concluding your encoder is wrong | Confirm the reference ran in `kMinimum` mode before treating a length difference as your bug |
| Parallax (path dependency) | Discovering a licence violation at integration | Run Parallax's `deny.toml` **verbatim** here; a locally-authored copy drifts |
| Parallax (determinism) | Two resamplers, or two loudness meters, one on each side of the boundary | This crate resamples nothing and measures nothing. Mismatched input is a typed error |
| Parallax (API shape) | Designing the parameter-block API before the tick-rate question is answered (`PROJECT.md` open question 1) | This is the one open question that blocks M4. It decides curves-vs-blocks. Do not guess it in M1 — but do not let M4 start without it |
| FLAC / Opus `-sys` crates (M3) | Assuming `libFLAC` BSD-3-Clause and `libopus` BSD-3-Clause means the wrapper crate is clean | The wrapper's own licence, its vendored contents (the FLAC *tools* are GPLv2), and its build-dependencies are all separate questions. Run `cargo deny` on a throwaway branch before committing to a crate |
| ISO-BMFF (M5) | Reading `gpac` for the box layout | Forbidden. Use the IAMF spec's ISO-BMFF binding section and `iamf-tools`' own muxer, which is permissively licensed |

---

## Performance Traps

Scale here means file duration and channel count, not users.

| Trap | Symptoms | Prevention | When It Breaks |
|---|---|---|---|
| Buffering the whole encode in memory | Memory grows linearly with duration; a feature-length 9.1.6 24-bit export exhausts RAM | Stream temporal units to a `Write` sink, as `obu_sequencer_base` does. Descriptors are written once, then each temporal unit is serialised and flushed | ~10 min of 16-channel 48 kHz 24-bit ≈ 1.4 GB of PCM alone |
| Per-OBU `Vec` allocation for the size-computation pass | Allocator churn: one or more allocations per frame per substream | Reuse a scratch buffer on the serialiser. Keep the two-pass structure (it is what makes `obu_size` correct); just stop reallocating | Thousands of frames × tens of substreams |
| The 2 MB OBU ceiling vs. large LPCM frames | Encode fails partway through a long file with a size error | Validate at Codec Config construction: `num_samples_per_frame × channels × bytes_per_sample` must fit under 2 MB with header overhead. Fail at configuration, not at frame 4 000 | 32-bit, 28 channels, >18 000 samples/frame |
| Fuzzer OOM masking real findings | The fuzzer reports one OOM repeatedly and finds nothing else | Allocation caps (Pitfall 10) plus an explicit `-rss_limit_mb`; treat an OOM finding as a parser bug to fix, not a fuzzer setting to raise | Within hours of the first fuzz run |
| Byte-identity CI job re-encoding a long fixture on four targets | CI time grows; the job gets disabled | Keep determinism fixtures short (a few frames) but *structurally* rich — many OBUs, many elements, non-round sample counts. Structure is what varies, not duration | As soon as the job exceeds patience |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---|---|---|
| `Vec::with_capacity(n)` / `vec![0; n]` from a parsed length | 4 GB allocation from a handful of bytes; OOM-kills the host DAW | Cap `n` against `reader.bytes_remaining()` before reserving. The reference does exactly this — `ValidateInRange(info_type_size, {0, 2MB})` before `resize`, and an explicit comment about not reading a large extension until it is plausible |
| Unchecked size arithmetic (`obu_size − trim − ext`) | Wrapping subtraction yields a huge length, which then drives an allocation or an index | `checked_sub` returning a typed error. `iamf-tools` computes this in `int64_t` and explicitly rejects a negative result — that check exists because hostile input reaches it |
| `data[i]` / `&data[a..b]` in the parser | Panic = denial of service inside the DAW process | `#![deny(clippy::indexing_slicing)]`; all reads through a bounds-checked `BitReader` |
| Loop count from a `u32` field with no per-element floor | 4 billion iterations; indistinguishable from a hang | `n ≤ bytes_remaining()` when each element costs ≥1 byte. One rule, applies to every count in the format |
| Trusting `obu_size` to bound the payload read | A payload reader that runs past its OBU into the next one corrupts framing without erroring | Give each OBU payload a *sub-reader* limited to exactly `payload_len`, and assert it is fully consumed at the end — this also catches under-reads, which are silent otherwise |
| Treating `libiamf`'s acceptance as validation | It clamps rather than rejects; it ignores reserved bits | `libiamf` is a compatibility oracle, not a validator |
| `unsafe` anywhere | Any memory-safety bug becomes exploitable rather than a panic | `#![forbid(unsafe_code)]` — and note this constrains the M3 codec bindings, which are FFI. Isolate FFI in a separate module or crate with its own justification |
| Unbounded total allocation across one parse | Many individually-plausible fields sum to exhaustion | A crate-level cap on total allocation per parse, with a configurable limit |

---

## API Pitfalls

The library's users are Parallax's exporter and, later, other Rust callers.

| Pitfall | Impact | Better Approach |
|---|---|---|
| Accepting per-source positions | Promises something IAMF-as-practised cannot express — it is a bed format (`PROJECT.md` correction 1). The caller builds on a lie and discovers it at export | Accept rendered PCM plus a layout label. Make the API's shape state the truth |
| Bare integers for closed-set fields (`sample_rate: u32`, `sample_size: u8`) | The caller can construct an invalid file and only learns at serialisation | Enums. Illegal states unrepresentable |
| `anyhow` in a library | Callers cannot match on failure kinds | `thiserror` — already a stated constraint |
| Errors that name the field the *reference* complains about | "audio_roll_distance mismatch" when the user set the wrong codec | Errors phrased in the caller's vocabulary, with the wire-level detail as context |
| Deciding curves-vs-blocks for parameter blocks by guessing the tick rate | The one open question that blocks M4. Guessing means an API break, and audible motion artefacts | Answer `PROJECT.md` open question 1 before M4 begins. It is shared with ADM BWF, so Parallax should answer it once |
| Exposing a `LebMode` knob | Two callers get different bytes for the same input; determinism becomes caller-dependent | Minimal encoding, hard-coded, documented |
| Feature-gating `encode`/`decode` without testing the combinations | `--no-default-features` or `--features decode` fails to build, discovered by a downstream user | CI job matrix over the feature powerset from the moment the second feature exists |

---

## "Looks Done But Isn't" Checklist

- [ ] **OBU framing:** often missing a boundary walk — verify `find_obu_boundaries()` over your own output lands the final boundary exactly on `len()`, on a file with ≥6 OBUs
- [ ] **`obu_size`:** often measured from the wrong origin — verify it excludes the first byte and the size field itself, and *includes* trimming and extension bytes
- [ ] **Header flags:** often modelled as one shared `bool` — verify the reserved case is unrepresentable for types 0, 1, 3, 24–31, and that `obu_redundant_copy` cannot be set on audio frames, temporal delimiter or parameter block
- [ ] **Trimming:** often start-before-end and never exercised — verify wire order is END then START, and that the fixture's sample count is *not* a multiple of the frame size
- [ ] **leb128:** often assumes minimality — verify the parser accepts padded encodings up to 8 bytes and rejects a 9th continuation byte and `> u32::MAX`
- [ ] **Descriptor order:** often unsorted — verify Codec Configs and Audio Elements are written ascending by ID, and that Audio Elements precede the Mix Presentations referencing them
- [ ] **LPCM config:** often defaults endianness wrong — verify `sample_format_flags = 0` means **big**-endian and that this matches the bytes you actually wrote
- [ ] **`audio_roll_distance`:** often left at a default — verify it is exactly `0` for LPCM and derived (not user-supplied) for every codec
- [ ] **Flag/field coupling:** often stored rather than derived — verify `info_type` and `obu_extension_flag` are computed from `Option` fields
- [ ] **Byte alignment:** often unasserted — verify the writer asserts alignment at every OBU boundary and the payload sub-reader is fully consumed
- [ ] **Determinism:** often claimed, not tested — verify the same-process double-encode test exists *and* the four-target hash job exists and has actually run
- [ ] **`cargo deny`:** often never exercised — verify it fails on a deliberately-added LGPL crate before trusting a green result
- [ ] **Fuzzing:** often a target with no corpus — verify `fuzz/corpus/` is committed, non-empty, and that the target runs in CI with a time budget
- [ ] **Clippy hardening:** often present but overridden — verify no `#[allow(clippy::indexing_slicing)]` or `arithmetic_side_effects` in non-test code
- [ ] **Spec version:** often only in prose — verify `SPEC_VERSION` exists in code and reference SHAs are pinned in CI
- [ ] **Conformance:** often "libiamf returned OK" — verify sample count, PCM equality, `iamf-tools` parse, and the byte-diff-or-explanation against an `iamf-tools`-produced file
- [ ] **No DSP:** often crept in — verify the transcendental grep passes and no module computes a value from audio samples

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---|---|---|
| `obu_size` origin wrong, caught in M1 | **LOW** | Fix one function, regenerate the handful of M1 fixtures, re-run the conformance harness |
| `obu_size` origin wrong, caught after M2 | **HIGH** | Fix the function; regenerate every fixture and golden; fix the parser's mirror assumption; **discard and re-seed the fuzz corpus** (all accumulated coverage was for a nonexistent grammar); re-verify M2's round trips, which were passing while wrong |
| Reserved bit set, caught by certification after shipping | **HIGH** | One-line encoder fix, but every file ever exported is non-conformant and there is no way to identify which downstream copies exist. Mitigated only by having run `iamf-tools`' parser in M1 |
| Trimming order swapped, caught in M3 | **MEDIUM** | One-line fix; but every M1/M2 fixture had equal trim values and so was byte-identical either way — meaning no existing test detects the fix either. Add the non-multiple-of-frame-size fixture first, watch it fail, then fix |
| `HashMap` reached output, caught by the CI matrix | **LOW** | Swap to `BTreeMap`, confirm the ordering matches the reference's `SortedKeys`, re-run |
| `HashMap` reached output, caught by a Parallax user | **MEDIUM** | Same code fix, but every previously-exported file may differ from a re-export, so any downstream byte-identity claim is void |
| LGPL source read | **VERY HIGH / IRRECOVERABLE** | Identify every affected file; rewrite clean-room by a contributor who has not read the source; or relicense, which needs every contributor's agreement. There is no cheap path — which is why the prevention is a written policy and a PR checkbox |
| A dependency violates `deny.toml`, caught at Parallax integration | **MEDIUM–HIGH** | Replace the dependency, which for a codec binding may mean a different FFI surface and re-doing M3's decode-and-compare work |
| DSP merged | **MEDIUM** | Revert is easy; the hard part is that a caller may now depend on it. Prevention (the grep, the CONTRIBUTING rule) is what keeps this LOW |
| Wrong spec version targeted | **MEDIUM** | Mostly constant changes if `SPEC_VERSION` and the spec module exist; a full audit of scattered literals if they do not |
| Parameter tick-rate question answered after M4 | **HIGH** | A breaking change to the Parallax-facing API. Answer it before M4 starts |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---|---|---|
| 1. `obu_size` origin | **M1** | Boundary walk lands exactly on `len()` for a ≥6-OBU file; byte-diff against `iamf-tools` |
| 2. Type-specific flag bit | **M1** model, **M2** verification | Reserved case unrepresentable in the type system; `iamf-tools` parser accepts our output |
| 3. Trimming order | **M1** | Fixture with a non-multiple-of-frame-size sample count decodes with the correct sample count and content |
| 4. leb128 minimality | **M1** encoder, **M2** parser | Parser accepts a hand-written padded encoding; rejects 9-byte and `> u32::MAX` |
| 5. Descriptor order / referential integrity | **M1** | Two-Codec-Config, two-Audio-Element fixture; assert ascending ID order in the output bytes |
| 6. Codec Config exact values | **M1** LPCM, **M3** FLAC/Opus | Enums in the API; a test at 24-bit and at a non-48 kHz rate |
| 7. Flag/field coupling | **M1**, rule applied per OBU | No public struct holds both a flag and the field it gates |
| 8. Bit-level I/O | **M1**, first commit | `BitWriter::is_byte_aligned()` asserted at every OBU boundary; absolute-offset assertions in tests |
| 9. Determinism | **M1** (`BTreeMap`, CI matrix), **M4** (loudness quantisation) | Same-process double-encode test; four-target hash equality job; `HashMap`/transcendental greps |
| 10. Parser hostility | **M1** (lints), **M2** (parser + fuzz) | Clippy hardening lints deny-level with no non-test overrides; fuzz target in CI with a committed corpus; round-trip differential harness |
| 11. Spec-version drift | **M1** | `SPEC_VERSION` in code; reference SHAs pinned in `REFERENCES.md` and used by CI |
| 12. Licence / patent | **M1** (mechanisms), **M3** (deps), **M5** (ISO-BMFF reading) | `cargo deny` proven to fail on a known-bad crate; `NOTICE` + `REFERENCES.md` present; AOM §1.2 read and recorded; licence decision made |
| 13. Testing that proves nothing | **M1** | The seven-clause conformance harness exists as a **reusable function**, invoked again unchanged in M3 |
| 14. Milestone sequencing | **M1** | No parser code merged before M1's byte-diff is identical or fully explained in writing |
| 15. Scope creep into DSP | **M1** (rule + grep), tested at **M3** and **M5** | Transcendental grep passes; sample-rate mismatch is a typed error, not a resampler |

---

## Sources

**Read from source, 2026-09-07 (HIGH confidence):**

- `AOMediaCodec/iamf-tools` @ `main`, BSD-3-Clause-Clear + AOM Patent License 1.0 — `iamf/obu/obu_header.{cc,h}` (header layout, `obu_size` semantics, flag-bit type dependence, redundant-copy restrictions, trimming field order, 2 MB ceilings), `iamf/common/leb_generator.{cc,h}` and `iamf/common/read_bit_buffer.cc` (leb128 modes, 8-byte cap, `u32` bound, overflow rejection), `iamf/obu/ia_sequence_header.{cc,h}` (`ia_code`, profile enumeration, unvalidated `additional_profile`), `iamf/obu/codec_config.cc` and `iamf/obu/decoder_config/lpcm_decoder_config.cc` (roll distance, sample rate/size/format enumerations, frame-size bounds), `iamf/obu/mix_presentation.cc` (`info_type` bitmask gating, `signed(16)` loudness, pre-`resize` range validation), `iamf/cli/descriptor_obu_parser.cc` (descriptor ordering enforcement, referential integrity, reserved-OBU bypass, small-Codec-Config skip), `iamf/cli/obu_sequencer_base.cc` (`SortedKeys` write order, temporal-unit order, byte-alignment assertions, common sample rate/bit depth), `iamf/cli/temporal_unit_view.cc` (uniform trimming and timestamps, unique substream and parameter IDs), `iamf/cli/profile_filter.cc` (profile channel/element counting)
- `AOMediaCodec/libiamf` @ `main`, BSD-3-Clause-Clear + AOM Patent License 1.0 — `code/src/iamf_dec/obu/iamf_obu.c` (splitter arithmetic confirming `obu_size` excludes the header; 2 MB and 2-byte bounds; the six-profile limit table; unvalidated reserved bits), `code/src/iamf_dec/common/iorw.c` (leb128 saturation to `UINT32_MAX`, silent 8-byte truncation)

**Web sources (LOW–MEDIUM confidence, per the classify-confidence seam):**

- [Immersive Audio Model and Formats v1.1.0 — AOM](https://aomediacodec.github.io/iamf/v1.1.0.html) — OBU header syntax table; `obu_size` normative wording
- [Structure-Aware Fuzzing — Rust Fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz/structure-aware-fuzzing.html) — `arbitrary` derive vs `fuzz_mutator!`, coverage comparison
- [Writing harnesses — Trail of Bits Testing Handbook](https://appsec.guide/docs/fuzzing/rust/techniques/writing-harnesses/) — harness structure, library-crate requirement, `init` parameter
- [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) — corpus and target mechanics
- [Clippy Lints index](https://rust-lang.github.io/rust-clippy/master/index.html) and [Lint Configuration](https://doc.rust-lang.org/clippy/lint_configuration.html) — `restriction` category semantics; `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`
- [Your Clippy Config Should Be Stricter — Evan Schwartz](https://emschwartz.me/your-clippy-config-should-be-stricter/) — restriction-lint selection rationale

**Not consulted, by constraint:** `libspatialaudio` (LGPL-2.1+), `gpac` (LGPL-2.1).

---

## Gaps

- **`libiamf`'s payload-level rejection rules** were only partly read (OBU-level framing and leb128). The per-OBU parsers (`codec_config_obu.c`, `audio_element_obu.c`, `mix_presentation_obu.c`) were not read in detail. M1 should read `codec_config_obu.c` and `audio_frame_obu.c` before writing the LPCM path.
- **The ISO-BMFF binding** (M5) was not researched. The `gpac` prohibition makes `iamf-tools`' own muxer the only permissible reference; confirm it exists in that repository before committing to M5.
- **AOM Patent License 1.0 §1.2** remains unread — this is `PROJECT.md` open question 2 and it is cheapest to resolve now.
- **The parameter tick-rate question** (`PROJECT.md` open question 1) is outside this document's scope but blocks M4's API. It is flagged in Integration Gotchas and API Pitfalls.
- **Whether the target is IAMF v1.0 or v1.1.0** is unresolved and affects Pitfall 11's prescription directly. The certification programme cites v1.0; the reference implements v1.1.0.

---
*Pitfalls research for: IAMF bitstream serialiser/parser in Rust*
*Researched: 2026-09-07*
