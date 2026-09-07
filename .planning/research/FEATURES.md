# Feature Research

**Domain:** IAMF bitstream serialiser / parser / encoder library (Rust)
**Researched:** 2026-09-07
**Confidence:** HIGH for everything marked `[ENC]` / `[DEC]` / `[WIRE]`; MEDIUM–LOW for `[SPEC]`

---

## 0. Provenance key — read this first

Every claim below carries one of these tags. The project has been burned by prose, so nothing is
asserted without saying where it came from.

| Tag | Meaning | Trust |
|-----|---------|-------|
| `[ENC]` | Read from **`AOMediaCodec/iamf-tools`** source (BSD-3-Clause-Clear + AOM Patent 1.0). Path and symbol given. | HIGH — this is the reference encoder / OBU library |
| `[DEC]` | Read from **`AOMediaCodec/libiamf`** source (BSD-3-Clause-Clear). | HIGH — this is the reference decoder, i.e. the thing that must accept our bytes |
| `[WIRE]` | Verified by hex-dumping a **golden conformance bitstream** shipped in `libiamf/tests/*.iamf` and decoding it by hand. | HIGHEST — this is ground truth |
| `[ECL]` | Read from **`google/eclipsa-audio-plugin`** (Apache-2.0). A *consumer* of IAMF, not the format. | MEDIUM — tells you what one product does, not what the format requires |
| `[SPEC]` | Specification prose or a code comment paraphrasing it, **not** independently verified against a byte stream. | **UNVERIFIED** |
| `[MKT]` | Product/marketing documentation. | **UNVERIFIED — treat as false until checked** |

**Sources actually consulted.** `iamf-tools` @ `901a86e` (2026-09-02) plus tag `v1.0.0` (`ce54b9a`,
2024-01-26); `libiamf` @ `e55e183` (2026-08-21) including its `tests/*.iamf` golden vectors;
`eclipsa-audio-plugin` @ HEAD. **No LGPL or GPL source was opened.** `libspatialaudio` and `gpac` were
not cloned, read, grepped or referenced.

Local checkouts, if you want to re-verify:
`/private/tmp/claude-501/-Users-cell-local-iamf-rs/0fe94b60-790f-4e8c-98a8-3cadeacd03fa/scratchpad/{iamf-tools,libiamf,eclipsa-audio-plugin}`

---

## 1. Findings that change the project brief

Four things surfaced that PROJECT.md and HANDOFF.md do not currently say. Take these before the
feature tables.

### 1.1 Pin **IAMF v1.1.0**, not v1.0 — the decoder decides this

`libiamf`'s own README states: *"The 'code' directory contains the reference decoder for AOM IAMF
v1.1.0 … updated to implement the changes including the base-enhanced profile introduced in the
v1.1.0 specification since the release of v1.0.1."* `[DEC: README.md]`

PROJECT.md's Constraint *"Pin an IAMF specification version… the certification programme cites v1.0"*
is inconsistent with its own Correction #4, which lists **Base-Enhanced** — a profile that **does not
exist in v1.0**. The `ProfileVersion` enum at `iamf-tools` v1.0.0 has only Simple and Base. Since the
Core Value is *"libiamf reads it back"*, the version to pin is **v1.1.0**.

**Action for the roadmap: change the pinned version to IAMF v1.1.0 and name it in the code.**

### 1.2 `iamf-tools` HEAD is ahead of the decoder — do not mirror it blindly

`iamf-tools` HEAD is tracking a **draft IAMF v2.0.0**. Its own source says so: a TODO reads
*"TODO(b/461488730): Ensure these agree with final v2.0.0 limits."* `[ENC: iamf/cli/profile_filter.cc]`

Constructs present at `iamf-tools` HEAD that **`libiamf` (v1.1.0) will not accept**:

| HEAD-only construct | Where | Risk if copied |
|---|---|---|
| `kObuIaMetadata = 24` (new OBU type) | `iamf/obu/obu_header.h` | Unknown OBU in a v1.1.0 stream |
| `kAudioElementObjectBased = 2` + `ObjectsConfig` | `iamf/obu/audio_element.h` | Explicitly filtered out of Simple/Base/Base-Enhanced by `profile_filter.cc` |
| Param definition types **3–8** (Polar, Cart8, Cart16, DualPolar, DualCart8, DualCart16) | `iamf/obu/param_definitions/param_definition_base.h` | v1.1.0 knows only 0/1/2 |
| `RenderingConfig` re-layout: 2 b mode + 1 b `element_gain_offset_flag` + 2 b `binaural_filter_profile` + 3 b reserved | `iamf/obu/rendering_config.cc` | v1.1.0 is 2 b mode + **6 b reserved**. Same byte width; **all-zero is identical on the wire**, non-zero is not |
| Profiles 3/4/5 (Base-Advanced, Advanced-1, Advanced-2) | `iamf/obu/ia_sequence_header.h` | Rejected by a v1.1.0 decoder |
| Expanded layouts 13–19 (10.2.9.3, LFE pair, bottom-3, 7.1.5.4, bottom-4, top-1, top-5) | `iamf/obu/audio_element.h` | `profile_filter.cc` removes Base-Enhanced for exactly these |
| `MixPresentationOptionalFields` behind the Mix Presentation `optional_fields_flag` | `iamf/obu/mix_presentation.cc` | Header bit 1 gets a *third* meaning; v1.1.0 has no such field |

**The v1.1.0 subset happens to be the same subset the project already wants.** Building only what
PROJECT.md scopes in automatically stays v1.1.0-clean. Building "everything the reference type model
has" does not.

### 1.3 Corrections #1 and #2 came from Eclipsa, not from `iamf-tools` — they are true, but about a *consumer*

- The comment *"Keeping things simple with 1 layer for now"* **does not exist anywhere in
  `iamf-tools`** (grepped the whole tree). It is Eclipsa's. `iamf-tools` itself fully implements
  multi-layer `ScalableChannelLayoutConfig`, recon gain and output gain `[ENC:
  iamf/obu/audio_element.cc]`.
- Correction #1 (no object-based path) was true of `iamf-tools` at v1.0/v1.1 and remains true *for
  the profiles we target* — `profile_filter.cc` erases Simple, Base and Base-Enhanced the moment an
  element is object-based `[ENC]`. It is **no longer true of `iamf-tools` HEAD in general**, which
  now has `ObjectsConfig` for the v2.0 draft.

**Both corrections stand as scope decisions.** They are just weaker evidence than "the reference
can't do it" — they are "the reference product chose not to, and the profiles we target forbid it."
That is still a good reason not to build them.

### 1.4 Correction #5's layout numbering (0–30) is Eclipsa's internal enum, **not an IAMF field**

`[ECL: common/substream_rdr/substream_rdr_utils/Speakers.h:133–186]` — `kMono(0) … kHOA3(12)`,
`kExplLFE(13) … kExpl9Point1Point6Top(25)`, `k22p2(26) … kHOA7(30)`. That is a plugin-internal index.
**No IAMF bitstream field is numbered 0–30.** Three separate wire fields are involved:

| Eclipsa index | Actual IAMF encoding | Field |
|---|---|---|
| 0–9 (Mono…Binaural) | `loudspeaker_layout` = 0–9, 4 bits `[ENC/WIRE]` | Audio Element → `ChannelAudioLayerConfig` |
| 10–12 (HOA1–3) | `audio_element_type` = **1** (scene-based) + `AmbisonicsConfig`. **Not a loudspeaker_layout value at all** `[ENC: audio_element.cc]` | Audio Element |
| 13–25 (expanded) | `loudspeaker_layout` = **15** (`kLayoutExpanded`) **+ an 8-bit `expanded_loudspeaker_layout`** = 0–12. Note the offset: Eclipsa 13 → IAMF expanded 0 `[ENC]` | Audio Element |
| 26 (22.2) | Not an Audio Element. It is `SoundSystemH_9_10_3` = 7 in a **Mix Presentation** `Layout` `[ENC: mix_presentation.h:153]` | Mix Presentation |
| 27–30 (HOA4–7) | Not encodable. Render-only. | — |

Also confirmed from code: **9.1.6 is indeed not a base layout** — it is `expanded_loudspeaker_layout
= 8`, whose comment reads *"Subset of Sound System H [ITU-2051-3]"*, with 16 named channels
(`FLc FRc FL FR SiL SiR BL BR TpFL TpFR TpSiL TpSiR TpBL TpBR` coupled + `FC LFE` non-coupled)
drawn from the 24 of Sound System H `[ENC: obu_with_data_generator.cc:196–204]`. Correction #5's
substance is right; its numbering needs the table above.

**Design consequence:** the Rust type model must not have one flat `Layout` enum of 31 variants. It
needs `LoudspeakerLayout` (0–15), `ExpandedLoudspeakerLayout` (u8), `AmbisonicsMode`, and
`SoundSystem` (0–14) as **four distinct types** in three different OBUs.

---

## 2. The OBU header encoding — **RESOLVED**

PROJECT.md flags this as the thing not to guess. It is now settled, three ways: the encoder writes
it, the decoder reads it, and a golden file proves it.

### 2.1 The layout

```
byte 0:  [ obu_type : 5 ][ obu_redundant_copy : 1 ][ type_specific_flag : 1 ][ obu_extension_flag : 1 ]
         MSB-first: obu_type occupies bits 7..3.
then:    obu_size                        : uleb128 (unsigned LEB128, 1..8 bytes)
then:    if (audio-frame type && type_specific_flag):
             num_samples_to_trim_at_end   : uleb128
             num_samples_to_trim_at_start : uleb128
then:    if (obu_extension_flag):
             extension_header_size        : uleb128
             extension_header_bytes       : that many bytes
then:    payload                          : obu_size − (all conditional bytes above)
```

Citations, all three independent:

- **Write path** `[ENC: iamf/obu/obu_header.cc, ObuHeader::ValidateAndWrite]`
  `WriteUnsignedLiteral(obu_type, 5)` → `WriteBoolean(obu_redundant_copy)` →
  `WriteBoolean(type_specific_flag)` → `WriteBoolean(GetExtensionHeaderFlag())` →
  `WriteUleb128(obu_size)` → `WriteFieldsAfterObuSize(...)`.
- **v1.0 write path is byte-identical** `[ENC: iamf-tools v1.0.0 (ce54b9a), iamf/obu_header.cc]` —
  same five writes, with the third flag literally named `obu_trimming_status_flag`. **The header wire
  format has not changed between v1.0 and HEAD.**
- **Read path** `[DEC: code/src/iamf_dec/obu/iamf_obu.c:56–95]`
  ```c
  h->obu_type              = (val >> 3) & 0x1f;
  h->obu_redundant_copy    = (val >> 2) & 0x01;
  h->obu_trimming_status_flag = (val >> 1) & 0x01;
  obu_extension_flag       =  val       & 0x01;
  obu_size = ior_leb128_u32(r);
  ```
  with the header comment `// obu_type(5) + obu_redundant_copy(1) + obu_trimming_status_flag(1) + obu_extension_flag(1) + obu_size(min:8)`.

### 2.2 `[WIRE]` proof from a golden conformance file

`libiamf/tests/test_000003.iamf`, offset 0:

```
f8 06 69 61 6d 66 00 00
```
`0xF8 = 0b11111_0_0_0` → obu_type 31 (IA Sequence Header), all three flags 0. `obu_size = 0x06`.
Payload: `"iamf"` + `primary_profile=0` + `additional_profile=0`. Exactly 6 bytes. ✔

Same file, offset `0x7D32` — the final audio frame, which the test metadata says trims 64 samples
from the end:

```
32 82 04 40 00 <512 bytes of PCM>
```
`0x32 = 0b00110_0_1_0` → obu_type **6** (Audio Frame, implicit substream ID 0), redundant 0,
**trimming_status 1**, extension 0. `obu_size = 82 04` = uleb128 → `2 | (4<<7)` = **514**.
Then `40` = `num_samples_to_trim_at_end` = **64** ✔, `00` = trim-at-start = 0.
Payload = 514 − 2 = **512** bytes = 128 samples × 2 ch × 16 bit. ✔

**This settles `obu_size` semantics beyond doubt: `obu_size` counts every byte after `obu_size`
itself — the trim fields, the extension header, and the payload.** `[ENC: GetObuPayloadSize()]`

### 2.3 The third flag is polymorphic — this is the trap

At HEAD the field is called `type_specific_flag` and its meaning depends on `obu_type`
`[ENC: obu_header.cc, GetObuTrimmingStatusFlag/GetIsKeyFrame/GetOptionalFieldsFlag]`:

| `obu_type` | Meaning of bit 1 |
|---|---|
| Audio Frame (5, 6–23) | `obu_trimming_status_flag` |
| Temporal Delimiter (4) | `is_not_key_frame` |
| Mix Presentation (2) | `optional_fields_flag` **(v2.0 draft only — do not set for v1.1.0)** |
| everything else | reserved, **SHALL be 0** — `Validate()` rejects it otherwise |

`libiamf` reads it as `obu_trimming_status_flag` unconditionally and only *acts* on it for audio
frames `[DEC: iamf_obu.c:79]`.

**For the v1.1.0 encoder: set bit 1 only on audio frames that carry trimming. Zero everywhere else.**

### 2.4 `obu_redundant_copy`

`Validate()` **rejects** `obu_redundant_copy` on audio frames, temporal delimiters and parameter
blocks `[ENC: IsRedundantCopyAllowed()]`. It is legal only on descriptors (0, 1, 2, 31). Its use is
re-emitting descriptors mid-stream for random access — a streaming feature, not needed for a
standalone `.iamf`. Set it to `false` and model it as an `Option`/flag you never assert in v1.

### 2.5 Byte alignment and padding

There is **no padding mechanism and no alignment bits**. `[ENC: iamf/obu/obu_base.cc,
ObuBase::ValidateAndWriteObu]` serialises the payload to a scratch buffer and then:

```cpp
if (!temp_wb.IsByteAligned()) {
  return absl::InvalidArgumentError("Expected the OBU payload to be byte-aligned: ...");
}
```

Every OBU payload's bit-fields **must sum to a whole number of bytes by construction**. The
serialiser errors out rather than padding. In Rust this means: a `BitWriter` that asserts
byte-alignment at each OBU boundary, and no `align()` call anywhere.

**Forward-compatibility mechanism (worth mirroring):** on read, if the payload parser consumes fewer
bytes than `obu_size` promised, the remainder is captured verbatim into `footer_` and written back
out on re-serialisation `[ENC: ObuBase::ReadAndValidatePayload]`. This is how round-trip fidelity
survives future spec versions. **Recommend the Rust model carries a `trailing: Vec<u8>` on every
OBU** — it makes M2's round-trip-equality requirement achievable against files we do not fully
understand.

### 2.6 `uleb128` details

- Standard unsigned LEB128, little-endian 7-bit groups, continuation bit in MSB `[ENC:
  iamf/common/leb_generator.cc]`.
- Decoded values are **`u32`**, not `u64` — `typedef uint32_t DecodedUleb128` `[ENC: iamf/obu/types.h]`.
- **Maximum 8 bytes** on the wire: `constexpr int kMaxLeb128Size = 8` with the comment *"IAMF spec
  requires a ULEB128 or a SLEB128 be encoded in <= 8 bytes"* `[ENC: types.h]`. So non-minimal
  (zero-padded) encodings are legal and must be *parsed*, up to 8 bytes.
- Two generation modes exist: `kMinimum` (minimal bytes) and `kFixedSize` (pad to N ∈ [1,8]) `[ENC:
  LebGenerator]`. **Use `kMinimum` — it is what the golden files use** `[WIRE]` — but a parser must
  accept padded forms.
- **Determinism note (PROJECT.md constraint):** minimal-form uleb128 is a pure function of the value.
  Safe. Do not expose fixed-size mode in the public API, or byte-identity becomes caller-dependent.
- Signed 16-bit fields (`audio_roll_distance`, `integrated_loudness`, `digital_peak`, `true_peak`,
  `output_gain`, `default_mix_gain`, mix-gain values) are **fixed 16-bit big-endian two's
  complement**, not sleb128 `[ENC: WriteSigned16]`, `[WIRE: ca 5b = −13733]`.

### 2.7 Size ceilings

- Entire OBU ≤ **2 MB** (`1 << 21`) `[ENC: kEntireObuSizeMaxTwoMegabytes]`, `[DEC:
  def_iamf_obu_max_size 2097152]` — both agree.
- Enforced as `obu_size ≤ 2^21 − 1 − sizeof(obu_size)` `[ENC: ValidateObuIsUnderTwoMegabytes]`.
- Strings ≤ **128 bytes including the NUL** `[ENC: kIamfMaxStringSize]`.
- Strings are **NUL-terminated UTF-8 with no length prefix** `[WIRE: 65 6e 2d 75 73 00 = "en-us\0"]`.

---

## 3. The OBU type set

`[ENC: iamf/obu/obu_header.h, enum ObuType]` — 5 bits, so 0–31.

| Type | Name | Required for minimal `.iamf`? | Does the reference emit it? | Notes |
|---|---|---|---|---|
| **0** | Codec Config | **YES — ≥1** | always | `[WIRE]` present in every golden vector |
| **1** | Audio Element | **YES — ≥1** | always | |
| **2** | Mix Presentation | **YES — ≥1** | always | `libiamf` selects one to render; without it there is nothing to decode `[DEC: iamf_decoder.c:292]` |
| **3** | Parameter Block | **NO** | only when a param definition uses `param_definition_mode = 1` and values vary | `[WIRE: test_000003.iamf has none and is `is_valid_to_decode: true`]` |
| **4** | Temporal Delimiter | **NO** | **off by default** — `enable_temporal_delimiters: false` in the vectors `[ENC: testdata/*.textproto]` | Zero-length payload `[ENC: temporal_delimiter.h]` |
| **5** | Audio Frame (explicit ID) | only if a substream ID > 17 | rarely | Payload = `uleb128 substream_id` + codec bytes |
| **6–23** | Audio Frame, implicit substream ID 0–17 | **YES — ≥1** | **this is the default** | `GetObuType(id) = 6 + id` when `id ≤ 17` `[ENC: audio_frame.cc:33]`, `[WIRE: 0x32 → type 6]` |
| **24** | Metadata | **NO** | HEAD only | **v2.0 draft. Do not emit.** Also present in `libiamf` HEAD's parser |
| 25–30 | Reserved | no | no | Parser must skip gracefully; `libiamf` logs `"Reserved OBU type %u"` and continues `[DEC]` |
| **31** | IA Sequence Header | **YES — exactly 1, first** | always | Carries the `"iamf"` 4CC that syncs the stream |

**Implicit-ID audio frames are the default, not an optimisation.** A Rust encoder that always emits
type 5 will produce valid but non-idiomatic files, and byte-comparison against `iamf-tools` output
will fail. Implement type selection: `id <= 17 → 6 + id`, else `5`.

---

## 4. Descriptor field inventory (v1.1.0 wire order)

This is the Rust type model. Field order **is** the serialisation order.

### 4.1 IA Sequence Header — OBU type 31

`[ENC: iamf/obu/ia_sequence_header.cc]`, `[WIRE]`

| Field | Width | Mandatory | Fixed by the reference? |
|---|---|---|---|
| `ia_code` | u32 = `0x69616D66` (`"iamf"`) | yes | **Always this.** Not a field the caller sets — `iamf-tools` removed `ia_code` from its user metadata "in favor of always inserting the correct one" `[ENC: CHANGELOG]`. Model it as a constant. |
| `primary_profile` | u8 | yes | caller-chosen |
| `additional_profile` | u8 | yes | caller-chosen; vectors set it equal to primary `[ENC: testdata]` |

Payload is exactly **6 bytes**. `libiamf` hard-fails if `ia_code` mismatches `[ENC:
ValidateEqual(ia_code, kIaCode)]`.

**v1.1.0 valid values: 0 = Simple, 1 = Base, 2 = Base-Enhanced.** 3/4/5 exist only at `iamf-tools`
HEAD.

### 4.2 Codec Config — OBU type 0

`[ENC: iamf/obu/codec_config.cc, ValidateAndWritePayload]`, `[WIRE]`

| Field | Encoding | Mandatory | Notes |
|---|---|---|---|
| `codec_config_id` | uleb128 | yes | caller-chosen; vectors use 200 |
| `codec_id` | u32 4CC | yes | `"ipcm"`=0x6970636D, `"fLaC"`=0x664C6143, `"Opus"`=0x4F707573, `"mp4a"`=0x6D703461 `[ENC: CodecConfig::CodecId]` |
| `num_samples_per_frame` | uleb128 | yes | must be ≥ 1; `iamf-tools` caps it for sanity, spec has no upper bound for LPCM `[ENC: ValidateNumSamplesPerFrame]` |
| `audio_roll_distance` | **int16 BE** | yes | **derived, not free** — see below |
| `decoder_config` | codec-specific | yes | see §6 |

**`audio_roll_distance` is a function of the codec, not a caller choice:**
- LPCM → **0**, validated by equality `[ENC: LpcmDecoderConfig::GetRequiredAudioRollDistance]`
- FLAC → `FlacStreamInfoStrictConstraints::kAudioRollDistance`, also validated by equality `[ENC]`
- Opus → `−ceil(3840 / num_samples_per_frame)` `[ENC: OpusDecoderConfig::GetRequiredAudioRollDistance]`

The Rust API should **compute** it and refuse to let the caller set it. `iamf-tools` goes further and
silently corrects it (*"Copy the codec config, it may be modified to correct the roll distance"*).

### 4.3 Audio Element — OBU type 1

`[ENC: iamf/obu/audio_element.cc, AudioElementObu::ValidateAndWritePayload]`, `[WIRE]`

| Field | Encoding | Mandatory | Notes |
|---|---|---|---|
| `audio_element_id` | uleb128 | yes | |
| `audio_element_type` | **3 bits** | yes | 0 = channel-based, 1 = scene-based, 2 = object-based (**v2.0 draft; erased from Simple/Base/Base-Enhanced by `profile_filter.cc`**), 3–7 reserved |
| `reserved` | **5 bits** | yes | always 0 in the vectors `[WIRE: byte 0x00]` |
| `codec_config_id` | uleb128 | yes | must match a Codec Config OBU |
| `num_substreams` | uleb128 | yes | **derived from the vector length** — `iamf-tools` dropped it from user metadata `[ENC: CHANGELOG]`. Model as `Vec::len()`, not a field. |
| `audio_substream_ids[]` | uleb128 × N | yes | |
| `num_parameters` | uleb128 | yes | also derived; ≤ 256 `[ENC: kMaxNumParameters]` |
| `audio_element_params[]` | see §7 | conditional | **Mix Gain (type 0) is explicitly forbidden here** — `iamf-tools` returns `InvalidArgumentError("Mix Gain parameter type is explicitly forbidden for Audio Element OBUs")` `[ENC]` |
| `config` | variant on type | yes | |

**Channel-based config (`ScalableChannelLayoutConfig`)** `[ENC:
ValidateAndWriteScalableChannelLayout]`:

| Field | Encoding | Notes |
|---|---|---|
| `num_layers` | 3 bits | 1–6. Derived from vec length. **v1 target: 1** |
| `reserved` | 5 bits | 0 |
| `channel_audio_layer_configs[]` | × `num_layers` | |

**Per-layer (`ChannelAudioLayerConfig::Write`)** `[ENC]`, `[WIRE: 10 01 01]`:

| Field | Encoding | Mandatory | Reference value in the minimal file |
|---|---|---|---|
| `loudspeaker_layout` | **4 bits** | yes | 1 (Stereo). 15 = `kLayoutExpanded` |
| `output_gain_is_present_flag` | 1 bit | yes | **0** |
| `recon_gain_is_present_flag` | 1 bit | yes | **0** |
| `reserved_a` | 2 bits | yes | 0 |
| `substream_count` | **u8** | yes | 1 |
| `coupled_substream_count` | **u8** | yes | 1 |
| `output_gain_flag` | 6 bits | **only if** `output_gain_is_present_flag` | absent |
| `reserved_b` | 2 bits | ditto | absent |
| `output_gain` | int16 BE | ditto | absent |
| `expanded_loudspeaker_layout` | **u8** | **only if** `loudspeaker_layout == 15` | absent |

**Confirms Correction #2's wire consequence:** the minimal conformant element writes one layer with
both gain flags clear, so the conditional blocks vanish entirely. `[WIRE: `20 10 01 01` — four
bytes total for the whole scalable config]`

**Scene-based config (`AmbisonicsConfig`)** `[ENC: iamf/obu/ambisonics_config.cc]`: `ambisonics_mode`
uleb128, then mono config (`output_channel_count` u8, `substream_count` u8, `channel_mapping[]` u8×N)
or projection config. Deferred per PROJECT.md.

### 4.4 Mix Presentation — OBU type 2

`[ENC: iamf/obu/mix_presentation.cc]`, `[WIRE: fully hand-decoded from test_000003.iamf]`

| Field | Encoding | Mandatory | Notes |
|---|---|---|---|
| `mix_presentation_id` | uleb128 | yes | |
| `count_label` | uleb128 | yes | number of annotation languages |
| `annotations_language[]` | NUL-terminated string × `count_label` | yes | must be **unique**; ISO-639-2 validated only for the `content_language` tag `[ENC]` |
| `localized_presentation_annotations[]` | string × `count_label` | yes | count must equal `count_label` |
| `num_sub_mixes` | uleb128 | yes | **must be ≥ 1** `[ENC: ValidateNumSubMixes]`. Derived from vec len |
| `sub_mixes[]` | below | yes | audio element IDs must be unique across sub-mixes |
| `mix_presentation_tags` | optional block | **no** | absent in the minimal golden file `[WIRE]`. When present: `num_tags` u8 then (name, value) string pairs; at most one `content_language` |
| `MixPresentationOptionalFields` | — | **v2.0 draft only** | gated on header bit 1. **Do not emit for v1.1.0** |

**Sub-mix** `[ENC: ValidateAndWriteSubMix]`:

| Field | Encoding | Notes |
|---|---|---|
| `num_audio_elements` | uleb128 | **must be ≠ 0** `[ENC: ValidateNotEqual(0, ...)]` |
| `audio_elements[]` | below | |
| `output_mix_gain` | Mix Gain param definition | **mandatory, always written** |
| `num_layouts` | uleb128 | |
| `layouts[]` | below | |

> **Hard encoder rule:** `"Every sub-mix must have a stereo layout."` `[ENC:
> ValidateAndWriteSubMix + HasStereoLayout]` — at least one layout must be
> `layout_type = 2` with `sound_system = 0` (`kSoundSystemA_0_2_0`). This is not optional and is a
> classic "almost works" failure.

**Sub-mix audio element** `[ENC: ValidateAndWriteSubMixAudioElement]`:

| Field | Encoding | Notes |
|---|---|---|
| `audio_element_id` | uleb128 | |
| `localized_element_annotations[]` | string × `count_label` | count must match |
| `rendering_config` | 1 byte + uleb128 ext size (+ bytes) | **v1.1.0: 2 bits `headphones_rendering_mode` + 6 bits reserved**, then `rendering_config_extension_size` uleb128. `[WIRE: 00 00]` |
| `element_mix_gain` | Mix Gain param definition | **mandatory, always written** |

`headphones_rendering_mode`: 0 = stereo, 1 = binaural (world-locked), 2 = binaural (head-locked),
3 = reserved `[ENC: RenderingConfig::HeadphonesRenderingMode]`. The vectors use 0.

**Layout** `[ENC: ValidateAndWriteLayout]`, `[WIRE: 80 00 ca5b cdb1]`:

| Field | Encoding | Notes |
|---|---|---|
| `layout_type` | **2 bits** | 0,1 reserved · **2 = SS convention** · 3 = binaural |
| if type 2: `sound_system` | **4 bits** | 0–14, see §9 |
| if type 2: `reserved` | 2 bits | 0 |
| if type 0/1/3: `reserved` | 6 bits | 0 |
| `info_type` | **u8 bitmask** | `0x01` true peak, `0x02` anchored loudness, `0xFC` any extension |
| `integrated_loudness` | int16 BE | **caller supplies** (PROJECT.md: this crate carries, does not compute) |
| `digital_peak` | int16 BE | caller supplies |
| `true_peak` | int16 BE | only if `info_type & 0x01` |
| anchored loudness block | u8 count + (u8 anchor, int16 value)× | only if `info_type & 0x02`; anchors must be unique |
| layout extension | uleb128 size + bytes | only if `info_type & 0xFC` |

`info_type = 0` in the minimal file — so only two int16s. Loudness units are Q7.8 dB
(`−13733 ≈ −53.6 LKFS`).

---

## 5. Minimum viable conformant file

**This is not a guess.** `libiamf/tests/test_000003.iamf` is a shipped conformance vector whose
metadata is `is_valid: true`, `is_valid_to_decode: true`, and whose description reads *"A simple
example of a stereo IAMF stream with 1 substream and no parameter blocks."* `[ENC:
iamf/cli/testdata/test_000003.textproto]`. It is byte-verified `[WIRE]` and is precisely the M1
target: single-layer channel-based, LPCM, one Mix Presentation, standalone `.iamf`.

### 5.1 OBU order

```
IA Sequence Header  (31)   ← exactly one, first
Codec Config        (0)    ← ascending codec_config_id
Audio Element       (1)    ← ascending audio_element_id
Mix Presentation    (2)    ← original order preserved (selection order matters downstream)
─────────── descriptors end / data begins ───────────
[Temporal Delimiter (4)]   ← optional, default OFF
[Parameter Blocks   (3)]   ← per temporal unit, before the frames
Audio Frames        (6+id) ← one per substream per temporal unit
… repeat per temporal unit …
```

`[ENC: iamf/cli/obu_sequencer_base.cc, WriteDescriptorObus + WriteTemporalUnit]`. The comment there
cites spec §5.1.1 and notes: *"For Codec Config OBUs and Audio Element OBUs, the order is arbitrary.
For determinism this implementation orders them by ascending ID."* — **which is exactly the
determinism discipline PROJECT.md requires. Copy it: sort by ID, use `BTreeMap`.**

Within a temporal unit: parameter blocks **then** audio frames `[ENC: WriteTemporalUnit]`.

### 5.2 Exact field values that make it decode

| OBU | Field | Value |
|---|---|---|
| IA Seq Hdr | `ia_code` / profiles | `"iamf"` / 0 / 0 (Simple) |
| Codec Config | id / codec / frame / roll | 200 / `"ipcm"` / 128 / 0 |
| | LPCM decoder config | `flags=1` (LE), `sample_size=16`, `sample_rate=16000` |
| Audio Element | id / type / codec_config_id | 300 / 0 (channel-based) / 200 |
| | substream ids / num_parameters | `[0]` / **0** |
| | scalable config | `num_layers=1`, layout=Stereo(1), **output_gain=0, recon_gain=0**, `substream_count=1`, `coupled_substream_count=1` |
| Mix Presentation | id / count_label | 42 / 1 |
| | languages / annotations | `["en-us"]` / `["test_mix_pres"]` |
| | sub-mix | 1 element (id 300), `headphones_rendering_mode=0`, ext size 0 |
| | `element_mix_gain` | `parameter_id=100`, `parameter_rate=16000`, **`param_definition_mode=1`**, `default_mix_gain=0` |
| | `output_mix_gain` | identical |
| | layouts | **exactly one**, `layout_type=2`, `sound_system=0` (**stereo — mandatory**) |
| | loudness | `info_type=0`, integrated `−13733`, peak `−12879` |
| Audio Frames | type | **6** (implicit substream 0) |
| | payload | 128 samples × 2 ch × 16-bit LE, interleaved tick-major |
| | last frame | trimming flag set, `trim_at_end=64`, `trim_at_start=0` |
| Parameter Blocks | — | **none** |
| Temporal Delimiters | — | **none** |

### 5.3 The three non-obvious requirements

1. **Mix gain param definitions are mandatory structure even with no Parameter Blocks.** Both
   `element_mix_gain` and `output_mix_gain` are written unconditionally. Setting
   `param_definition_mode = 1` means *"the schedule lives in Parameter Blocks"*, and if no Parameter
   Blocks appear, `default_mix_gain` applies. That is how the minimal file gets away with zero
   Parameter Block OBUs. `[ENC: ParamDefinition::ValidateAndWrite + MixGainParamDefinition]`, `[WIRE]`
2. **A stereo layout in every sub-mix.** §4.4.
3. **Trimming.** Input length is almost never a multiple of `num_samples_per_frame`, so the final
   frame is zero-padded and `num_samples_to_trim_at_end` is set. Not optional if you want the PCM to
   come back sample-identical — which is literally the project's Core Value.

The complete descriptor prologue of this file is **0x76 = 118 bytes**. That is the whole of M1's
output header.

---

## 6. Codec framing

### 6.1 LPCM (`"ipcm"`) — table stakes, no dependency

**Codec Config `decoder_config`** `[ENC: lpcm_decoder_config.cc]` — 6 bytes:

| Field | Width | Legal values |
|---|---|---|
| `sample_format_flags` | u8 | **0 = big-endian, 1 = little-endian** — nothing else accepted |
| `sample_size` | u8 | **16, 24, 32** only |
| `sample_rate` | u32 BE | **16000, 32000, 44100, 48000, 96000** only |

`audio_roll_distance` must be **0**.

**Audio Frame payload** `[ENC: iamf/cli/cli_util.cc, WritePcmFrameToBuffer + lpcm_encoder.cc]`:

```
for t in 0..num_samples_per_frame:
    for c in 0..channels_in_this_substream:
        write sample as sample_size/8 bytes, LE or BE per format flag
```

i.e. **interleaved, tick-major / channel-minor**. Frame length is exactly
`num_samples_per_frame × channels × sample_size/8` — fixed, so trimming is the only variable.
`sample_size % 8 != 0` is rejected outright.

**Channel-to-substream packing (BCG rule)** `[ENC: obu_with_data_generator.cc,
CollectBaseChannelGroupLabels]` — **coupled (stereo) substreams first, then non-coupled (mono)**:

| Layout | Coupled pairs, in order | Then mono |
|---|---|---|
| Mono | — | `C` |
| Stereo | `L2 R2` | — |
| 3.1.2 | `L3 R3`, `Ltf3 Rtf3` | `C`, `LFE` |
| 5.1 | `L5 R5`, `Ls5 Rs5` | `C`, `LFE` |
| 5.1.2 | `L5 R5`, `Ls5 Rs5`, `Ltf2 Rtf2` | `C`, `LFE` |
| 5.1.4 | `L5 R5`, `Ls5 Rs5`, `Ltf4 Rtf4`, `Ltb4 Rtb4` | `C`, `LFE` |
| 7.1 | `L7 R7`, `Lss7 Rss7`, `Lrs7 Rrs7` | `C`, `LFE` |
| 7.1.2 | 7.1 pairs + `Ltf2 Rtf2` | `C`, `LFE` |
| 7.1.4 | 7.1 pairs + `Ltf4 Rtf4`, `Ltb4 Rtb4` | `C`, `LFE` |

So for 7.1.4: `substream_count = 7`, `coupled_substream_count = 5`, substreams 0–4 carry the pairs in
that order, 5 = `C`, 6 = `LFE`. **Getting this order wrong produces a file that decodes cleanly with
the channels scrambled — the archetypal "almost works" bug.** Note this order is *not* the same as
the presentation channel order (`L R C LFE Lss Rss Lrs Rrs Ltf Rtf Ltb Rtb`) used elsewhere `[ENC:
channel_label.cc:56]`.

### 6.2 FLAC (`"fLaC"`) — M3

**`decoder_config` is a sequence of FLAC metadata blocks** `[ENC: flac_decoder_config.cc]`:

```
per block: [ last_metadata_block_flag : 1 ][ block_type : 7 ][ length : 24 ] payload
```

The STREAMINFO block (type 0, length fixed at **34**) `[ENC: WriteStreamInfo]`:

| Field | Width | `iamf-tools` constraint |
|---|---|---|
| `minimum_block_size` | 16 | ≥ 16, and **must equal `num_samples_per_frame`** |
| `maximum_block_size` | 16 | ditto |
| `minimum_frame_size` | 24 | **must be 0** (encoder validated) |
| `maximum_frame_size` | 24 | **must be 0** |
| `sample_rate` | 20 | 8000–192000 |
| `number_of_channels` | 3 | **must be 1** — *"In IAMF the number_of_channels is fixed to 1, but can be ignored when reading. The actual number of channels is determined on a per-substream basis based on the audio element."* `[ENC]` |
| `bits_per_sample` | 5 | serialised as **value − 1**; 16/24/32-bit → 15/23/31. Confirmed by `bit_depth_to_measure_loudness = bits_per_sample + 1` `[ENC:315–319]` |
| `total_samples_in_stream` | 36 | ≤ `0xFFFFFFFFF` |
| `md5_signature` | 128 | **must be all zeros** |

`audio_roll_distance` fixed at `FlacStreamInfoStrictConstraints::kAudioRollDistance`.

**Audio Frame payload** = one raw FLAC frame per substream per temporal unit. Additional metadata
blocks (non-STREAMINFO) are written verbatim as opaque bytes.

### 6.3 Opus (`"Opus"`) — M3

**`decoder_config` = the OpusHead identification header body** `[ENC: opus_decoder_config.cc]` — 11
bytes, **and note the byte-order mix**:

| Field | Width | Constraint |
|---|---|---|
| `version` | u8 | |
| `output_channel_count` | u8 | **fixed at 2** `[ENC: kOutputChannelCount]` |
| `pre_skip` | u16 **BE** | encoder delay |
| `input_sample_rate` | u32 **BE** | informational — decoder output is **always 48000** `[ENC: GetOutputSampleRate() { return 48000; }]` |
| `output_gain` | int16 BE | **fixed at 0** |
| `mapping_family` | u8 | **fixed at 0** |

Three of six fields are constants. `audio_roll_distance` = `−ceil(3840 / num_samples_per_frame)` —
e.g. 960 samples → −4; 480 → −8.

Bit depth for loudness measurement is reported as 32 `[ENC]`.

**Audio Frame payload** = one raw Opus packet per substream per temporal unit (no Ogg framing, no
TOC wrapper beyond Opus's own).

### 6.4 AAC-LC (`"mp4a"`)

`aac_decoder_config.{h,cc}` exists in `iamf-tools`. PROJECT.md scopes it out. The CHANGELOG confirms
the direction of travel: *"Deprecate `AAC_SAMPLE_FREQUENCY_INDEX_ESCAPE_VALUE` and explicit
`sampling_frequency`, these are usually forbidden by the IAMF spec."* `[ENC]` — **Correction #3 holds
for the encoder; note that `libiamf` does decode AAC, so a future decoder would need it.**

---

## 7. Parameter blocks — and the open question that blocks API design

### 7.1 Parameter definition types

`[ENC: iamf/obu/param_definitions/param_definition_base.h:47–58]`

| Value | Type | v1.1.0? | Where it may appear |
|---|---|---|---|
| **0** | Mix Gain | yes | **Mix Presentation only** — explicitly forbidden in Audio Element OBUs `[ENC]` |
| **1** | Demixing | yes | Audio Element only |
| **2** | Recon Gain | yes | Audio Element only |
| 3–8 | Polar, Cart8, Cart16, DualPolar, DualCart8, DualCart16 | **NO — v2.0 draft** | Rendering Config (object positions) |
| 9+ | reserved / extension | — | `ExtendedParamDefinition` (opaque bytes) |

Types 3–8 are the v2.0-draft object-position machinery. **They are the mechanism by which IAMF might
one day stop being a bed format.** Not in v1.1.0; not in scope.

### 7.2 Param definition wire format

`[ENC: ParamDefinition::ValidateAndWrite]`, `[WIRE: 64 80 7d 80 | 00 00]`

```
parameter_id            : uleb128
parameter_rate          : uleb128
param_definition_mode   : 1 bit
reserved                : 7 bits
if (param_definition_mode == 0):        // schedule is IN the definition
    duration                  : uleb128
    constant_subblock_duration: uleb128
    if (constant_subblock_duration == 0):
        num_subblocks   : uleb128
        subblock_durations[] : uleb128 × num_subblocks
// then subclass-specific:
//   Mix Gain    → default_mix_gain : int16 BE (Q7.8 dB)
//   Demixing    → dmixp_mode : 3 bits, reserved : 5 bits, default_w : 4 bits, reserved : 4 bits
//   Recon Gain  → nothing
```

`param_definition_mode` is **derived**, not set: `schedule.has_value() ? 0 : 1` `[ENC:
GetParamDefinitionMode]`. Mode **1** = schedule lives in the Parameter Blocks. Mode **0** = schedule
is fixed here and the blocks carry only values.

### 7.3 Parameter Block payload — OBU type 3

`[ENC: ParameterBlockObu::ValidateAndWritePayload]`

```
parameter_id : uleb128
if (param_definition_mode == 1):   // schedule travels with the block
    duration                  : uleb128
    constant_subblock_duration: uleb128
    if (constant_subblock_duration == 0):
        num_subblocks     : uleb128
        for each subblock: subblock_duration : uleb128, then parameter_data
    else:
        for each subblock: parameter_data
else:                               // schedule came from the definition
    for each subblock: parameter_data
```

**`parameter_data` by type:**
- **Mix Gain**: `animation_type` uleb128, then per type `[ENC: animated_parameter_data.h]`:
  `Step`(0) → `start` int16 · `Linear`(1) → `start`,`end` · `Bezier`(2) → `start`,`end`,`control`,
  `control_time` u8 · `InterLinear`(3) → `end` · `InterBezier`(4) → `end`,`control`,`control_time`.
  Values are Q7.8 dB.
- **Demixing**: `dmixp_mode` 3 bits + reserved 5 bits. Modes 0,1,2,4,5,6 valid; 3 and 7 reserved and
  **rejected** `[ENC]`.
- **Recon Gain**: per layer where `recon_gain_is_present_flags[i]`: `recon_gain_flag` uleb128 bitmask,
  then one u8 per set bit `[ENC: recon_gain_info_parameter_data.cc]`.

Note `InterLinear` / `InterBezier` omit the start value — they continue from the previous subblock.
That is a stateful decode and a real trap for a naive round-trip test.

### 7.4 **The parameter tick rate — facts for Open Question #1**

This is PROJECT.md's blocking question. Here is everything the code says.

1. **`parameter_rate` is a per-definition uleb128 field, independent of the audio sample rate.**
   `[ENC: ParamDefinition]` Confirmed.
2. **In practice the reference sets `parameter_rate == sample_rate`.** In `test_000003`: sample rate
   16000, both mix-gain `parameter_rate` = 16000. Across the vectors, 48000/48000 and 16000/16000
   `[ENC: testdata/*.textproto]`, `[WIRE]`.
3. **`iamf-tools` has never implemented the case where they differ.** Two open TODOs, same bug ID:
   - `global_timing_module.cc:44` — `// TODO(b/283281856): Handle cases where parameter_rate and sample_rate differ.`
   - `parameter_block_partitioner.cc:344` — `// TODO(b/283281856): Set the duration to a different value when parameter_rate != sample rate.`
   `[ENC]`
4. **Parameter block duration is set equal to `num_samples_per_frame`.**
   `partition_duration = codec_config.num_samples_per_frame()` `[ENC: FindPartitionDuration]`.
5. **The only validation is `parameter_rate != 0`** `[ENC: global_timing_module.cc:76]`.
6. **Durations are in parameter-rate ticks**, and timing is tracked as
   `{rate, timestamp}` pairs advanced per unit `[ENC: TimingData]`.
7. `[SPEC — unverified]` A code comment for LPCM cites *"IAMF v1.1.0 section 3.11.4: The sample rate
   used for computing offsets SHALL be sample_rate."*

**What this means for the API design decision.** The reference implementation, the golden vectors and
the conformance suite all live in the world where **1 parameter tick = 1 audio sample**, and one
parameter block spans exactly one audio frame. Nothing in the format forbids a lower parameter rate,
but nothing in either reference *exercises* it, so a file that uses one is untested territory for
`libiamf`.

**Recommendation for Parallax's decision:** take **pre-decimated blocks at `parameter_rate =
sample_rate`, one block per audio frame**, and let Parallax own curve→block decimation. Rationale:
(a) it is the only path with conformance-vector coverage; (b) it makes the crate's job purely
mechanical, consistent with "receives rendered PCM plus metadata and produces bytes"; (c) accepting
curves would put interpolation — i.e. maths, i.e. a determinism hazard under PROJECT.md's `libm`
constraint — inside this crate. The animation types (`Bezier`, `InterBezier`) mean the format can
*express* a curve, but expressing it correctly requires the encoder to decide sample points, which is
a Parallax decision.

**This is a recommendation, not a finding. Flag it for the phase that designs the API.**

---

## 8. Profiles

`[ENC: iamf/cli/profile_filter.cc:490–540]` — **Correction #4 confirmed verbatim from code.**

| Profile | Value | Max audio elements | Max channels | Notes |
|---|---|---|---|---|
| **Simple** | 0 | **1** | **16** | No expanded layouts, no object-based |
| **Base** | 1 | **2** | **18** | No expanded layouts, no object-based |
| **Base-Enhanced** | 2 | **28** | **28** | Expanded layouts 0–12 only; **not** 13–19 |
| Base-Advanced | 3 | 18 | 18 | **v2.0 draft — do not emit** |
| Advanced-1 | 4 | 18 | 18 | v2.0 draft |
| Advanced-2 | 5 | 28 | 28 | v2.0 draft |

Constraints enforced at encode time `[ENC]`:
- Channels are counted **per Mix Presentation**, summed over its audio elements.
- Object-based elements erase Simple, Base and Base-Enhanced.
- `loudspeaker_layout == 15` (expanded) erases Simple and Base.
- Expanded layouts 13–19 erase Base-Enhanced.
- Reserved loudspeaker layouts 10–14 erase everything.
- Ambisonics: mono and projection modes are allowed by all profiles; other modes erase everything.

**The "pick the minimum profile" requirement in PROJECT.md is directly implementable**: start with
the full profile set, run each of these filters, take the lowest survivor. That is exactly the
reference algorithm and it is ~150 lines.

---

## 9. Layouts — the three enumerations

### 9.1 `LoudspeakerLayout` — Audio Element, 4 bits `[ENC: audio_element.h]`

`0` Mono · `1` Stereo · `2` 5.1 · `3` 5.1.2 · `4` 5.1.4 · `5` 7.1 · `6` 7.1.2 · `7` 7.1.4 ·
`8` 3.1.2 · `9` Binaural · `10–14` reserved · **`15` Expanded**

### 9.2 `ExpandedLoudspeakerLayout` — u8, present only when layout == 15 `[ENC]`

`0` LFE · `1` StereoS (Ls/Rs of 5.1.4) · `2` StereoSS · `3` StereoRS · `4` StereoTF · `5` StereoTB ·
`6` Top4Ch · `7` 3.0 (L/C/R of 7.1.4) · **`8` 9.1.6 (subset of Sound System H)** · `9` StereoF ·
`10` StereoSi · `11` StereoTpSi · `12` Top6Ch · `13–19` **v2.0 draft** (10.2.9.3, LFE pair,
bottom-3, 7.1.5.4, bottom-4, top-1, top-5) · `20–255` reserved

**Base-Enhanced supports 0–12 only.**

### 9.3 `SoundSystem` — Mix Presentation `Layout`, 4 bits `[ENC: mix_presentation.h:145–161]`

| Value | Sound system | Channels `[ENC: GetNumChannelsFromLayout]` |
|---|---|---|
| 0 | A (0+2+0) **stereo — mandatory in every sub-mix** | 2 |
| 1 | B (0+5+0) | 6 |
| 2 | C (2+5+0) | 8 |
| 3 | D (4+5+0) | 10 |
| 4 | E (4+5+1) | 11 |
| 5 | F (3+7+0) | 12 |
| 6 | G (4+9+0) | 14 |
| 7 | **H (9+10+3) — this is 22.2** | 24 |
| 8 | I (0+7+0) | 8 |
| 9 | J (4+7+0) | 12 |
| 10 | IAMF 7.1.2 | 10 |
| 11 | IAMF 3.1.2 | 6 |
| 12 | IAMF Mono | 1 |
| 13 | **IAMF 9.1.6** | 16 |
| 14 | IAMF 7.1.5.4 | 17 |
| 15 | reserved | — |

Plus `layout_type = 3` → binaural, 2 channels.

**Note the direct evidence for Correction #5's 9.1.6 claim: `SoundSystem 13 = 16 channels`, while
`SoundSystem 7 (H, 9+10+3) = 24 channels`. 9.1.6 is a named 16-of-24 subset, exactly as stated.**

---

## 10. Table stakes (must exist for a usable v1 encoder)

| Feature | Why it is an entry fee | Complexity | Notes / provenance |
|---|---|---|---|
| OBU header write + read (5-bit type, 3 flags, uleb128 size, conditional trim & extension) | Every byte of every OBU goes through it | **LOW** | Fully specified in §2. `[ENC]`+`[DEC]`+`[WIRE]` |
| uleb128 encode (minimal) / decode (accept ≤ 8 bytes, padded) | Pervasive | LOW | `u32` values only |
| Byte-aligned `BitWriter` / `BitReader` with per-OBU alignment assertion | The reference errors rather than pads | LOW | `[ENC: ObuBase]` |
| `trailing: Vec<u8>` forward-compat footer on every OBU | Makes M2's round-trip equality achievable against unknown fields | LOW | `[ENC: footer_]` |
| IA Sequence Header (31) | Sync word; without it nothing is an IA Sequence | **TRIVIAL** | 6 bytes |
| Codec Config (0) + LPCM decoder config | No frames without it | LOW | `audio_roll_distance` must be **derived** |
| Audio Element (1), channel-based, single layer | The thing being encoded | **MEDIUM** | 3/5-bit split, 4-bit layout, conditional blocks |
| Mix Presentation (2), one sub-mix, one stereo layout | Mandatory; the decoder selects one to render | **MEDIUM** | Strings, `count_label` cross-checks, mandatory stereo layout, loudness |
| Mix Gain param definitions (element + output), mode 1, `default_mix_gain` | **Mandatory structure** even with zero Parameter Blocks | LOW | `[WIRE]` |
| Audio Frame with **implicit substream ID** (types 6–23) | This is what the reference emits; type 5 is the fallback | LOW | `id ≤ 17 → 6+id` |
| LPCM framing: interleaved tick-major, LE/BE, 16/24/32-bit | The payload | LOW | `[ENC: WritePcmFrameToBuffer]` |
| **Channel→substream BCG packing (coupled pairs first, then mono)** | Wrong order = clean decode, scrambled channels | **MEDIUM** | §6.1. Highest silent-failure risk in M1 |
| **Trimming: pad final frame, set `num_samples_to_trim_at_end`** | Without it the PCM is not sample-identical — the Core Value | **MEDIUM** | `[WIRE]` |
| Descriptor ordering: 31, then 0s, then 1s, then 2s | Spec §5.1.1 | TRIVIAL | Sort by ID for determinism `[ENC]` |
| Temporal unit ordering: parameter blocks then audio frames | | TRIVIAL | `[ENC: WriteTemporalUnit]` |
| Profile selection incl. minimum-fit | PROJECT.md requirement; ~150 lines of filters | **MEDIUM** | §8, directly portable |
| Layout enumerations as four distinct label types | §1.4 — one flat enum is wrong | LOW | |
| Loudness metadata carried through (`info_type`, integrated, peak, optional true-peak/anchored) | PROJECT.md requirement | LOW | Values from the caller |
| `thiserror` typed errors; no `unwrap`/`expect` | PROJECT.md constraint; the reference returns `absl::Status` everywhere | LOW | Mirror the validation points — they are the error taxonomy |
| Byte-identity test vs. `libiamf/tests/*.iamf` goldens | Cheapest possible conformance signal, and it already exists | **LOW** | ~40 LPCM vectors sitting in the repo |

**M1's full descriptor prologue is 118 bytes.** The work is in getting each of those bytes right, not
in volume.

### Table stakes that arrive with the parser (M2)

| Feature | Complexity | Notes |
|---|---|---|
| OBU parser mirroring every write path | MEDIUM | Every `ValidateAndWrite` has a `ReadAndValidate` twin to mirror |
| Reserved OBU types 25–30 skipped by `obu_size`, not by guessing | LOW | `libiamf` logs and continues `[DEC]` |
| Peek header without consuming (`obu_type` + total size) | LOW | `[ENC: PeekObuTypeAndTotalObuSize]` — needed for streaming/resync |
| Reject `obu_size` > 2 MB, negative residual payload | LOW | Both refs check `[ENC]`+`[DEC]` |
| Reject `type_specific_flag` set on a type that reserves it | LOW | `[ENC: Validate()]` |
| Fuzz target on the OBU parser | MEDIUM | PROJECT.md hard requirement for M2 |
| Round-trip equality against our own output **and** `iamf-tools` goldens | MEDIUM | The `trailing` footer is what makes the second half possible |

---

## 11. Later quality features (deferred — not entry fees)

| Feature | Why deferred | Complexity | Depends on |
|---|---|---|---|
| **FLAC framing** | Codec dependency; M1 proves the chain without it | MEDIUM | LPCM path proven |
| **Opus framing** | Ditto; roll distance is derived, three config fields are constants | MEDIUM | LPCM path proven |
| **Multi-layer scalable channel layouts (BCG/DCG ladder)** | `iamf-tools` implements it fully, but Eclipsa ships one layer and the profiles do not require it `[ENC]`+`[ECL]` | **HIGH** | single-layer element |
| **Recon gain** | Only meaningful with ≥ 2 layers; per-layer bitmask of u8 gains | HIGH | multi-layer |
| **Demixing parameters** | Only meaningful with a layer ladder | MEDIUM | multi-layer |
| **Output gain** (`output_gain_is_present_flag`) | Conditional 4 bytes per layer; reference writes it absent | LOW | — |
| **Parameter Blocks (type 3) at all** | The minimal file has none | MEDIUM | Answering §7.4 first |
| **Mix gain automation (Step/Linear/Bezier/Inter\*)** | Stateful `Inter*` variants make round-trip non-trivial | MEDIUM | Parameter Blocks |
| **Temporal Delimiter OBUs** | Default-off in the reference; ~3 lines when wanted | TRIVIAL | — |
| **Ambisonics mono mode (scene-based elements)** | HOA1–3 as an element; `AmbisonicsConfig` mono is small | MEDIUM | — |
| **Ambisonics projection mode** | PROJECT.md defers explicitly | HIGH | mono mode |
| **Expanded loudspeaker layouts 0–12 (incl. 9.1.6)** | One extra u8 + a channel table per layout; needs Base-Enhanced | MEDIUM | profile selection |
| **`obu_redundant_copy` / mid-stream descriptor re-emission** | Streaming/random-access feature; meaningless for a file | LOW | — |
| **Mix Presentation tags** | Optional block; `content_language` needs ISO-639-2 validation | LOW | — |
| **Anchored loudness / true peak / layout extension** | Conditional on `info_type` bits; caller supplies values | LOW | loudness path |
| **Multiple sub-mixes / multiple Mix Presentations** | Structurally supported; each needs its own stereo layout | MEDIUM | — |
| **ISO-BMFF muxing** | PROJECT.md: written by us, `gpac` is LGPL and excluded | **HIGH** | standalone path proven |
| **Native decoder** | Open Question #3 — may be better served by shelling out to `libiamf` | HIGH | parser |
| **AAC-LC** | Encoder does not need it; `libiamf` decodes it, so a decoder would | MEDIUM | decoder decision |

---

## 12. Anti-features — deliberately do not build

| Anti-feature | Why it gets requested | Why it is wrong here | Instead |
|---|---|---|---|
| **Any rendering / panning / spatial DSP** | "It's an audio format crate, surely it renders" | PROJECT.md `D-40`: Parallax owns the renderer in `f64` over `libm`. DSP here would duplicate it and break determinism ownership. A PR adding DSP is in the wrong repo | Accept rendered PCM + metadata |
| **Loudness measurement (BS.1770)** | The Mix Presentation carries loudness, so the crate "should" compute it | Parallax's meter shares code with export normalisation (`MON-05`). Two implementations = two answers | Carry the caller's numbers |
| **Object-based Audio Elements** | The spec and the marketing both promise objects `[SPEC]`/`[MKT]` | `profile_filter.cc` erases Simple, Base **and** Base-Enhanced the moment an element is object-based `[ENC]`. It is a v2.0-draft feature `libiamf` cannot decode | Bed format. Bake motion into a 7.1.4/9.1.6 bed (PROJECT.md Open Q #5) |
| **Position parameter definitions (types 3–8)** | They look like the way to express per-source motion | v2.0 draft. Emitting them makes the file undecodable by the v1.1.0 reference decoder | Mix gain only, for now |
| **Metadata OBU (type 24)** | Present in `iamf-tools` HEAD | v2.0 draft | Parse-and-skip; never emit |
| **Profiles 3/4/5 (Base-Advanced, Advanced-1/2)** | Present in the HEAD enum | v2.0 draft, with `TODO(b/461488730)` saying the limits are not final `[ENC]` | Simple / Base / Base-Enhanced |
| **Expanded layouts 13–19** | Present in the HEAD enum | `profile_filter.cc` erases Base-Enhanced for them `[ENC]` | 0–12 |
| **Mirroring `iamf-tools` HEAD's type model wholesale** | It is the reference; copying it feels safest | HEAD is a draft-v2.0 tree. A faithful port produces files `libiamf` rejects — the exact "almost works" failure PROJECT.md warns about | Port the **v1.1.0 subset**, verify against `libiamf/tests/*.iamf` |
| **A flat 31-variant `Layout` enum** | Correction #5's 0–30 numbering invites it | That numbering is Eclipsa's plugin index `[ECL]`, not an IAMF field. It conflates three wire fields across two OBUs (§1.4) | Four types: `LoudspeakerLayout`, `ExpandedLoudspeakerLayout`, `AmbisonicsMode`, `SoundSystem` |
| **`gpac` for ISO-BMFF** | It is the obvious muxer | LGPL-2.1, rejected by Parallax's `deny.toml`. **Not read for this research** | Write our own when ISO-BMFF lands |
| **`libspatialaudio`** | Renderer reference | LGPL-2.1+, contamination is irreversible. **Not read** | `libear` / `obr` are permissive if ever needed — but see anti-feature #1 |
| **Porting the Eclipsa plugin suite** | It is Google's own IAMF tooling | Rejected 2026-09-07: ~1/3 JUCE UI, architecture works around not being the host, and the encoder/decoder/muxer are not in that repo | Library against `iamf-tools` + `libiamf` |
| **Caller-settable `audio_roll_distance`** | It is a field, so expose it | It is a pure function of codec + frame size, validated by equality; `iamf-tools` silently corrects it `[ENC]` | Compute it; do not accept it |
| **Caller-settable `num_substreams` / `num_layouts` / `num_subblocks` / `count_label` cross-counts** | They are wire fields | `iamf-tools` deleted every one of them from its user metadata in favour of deriving from the related collection `[ENC: CHANGELOG]` | Derive from `Vec::len()`; make the invalid state unrepresentable |
| **`HashMap` keyed collections in the model** | Natural for ID lookup | PROJECT.md determinism constraint. The reference itself sorts (`SortedKeys(..., std::less<>)`) *specifically for determinism* `[ENC]` | `BTreeMap` / `Vec` |
| **Fixed-size (padded) uleb128 in the public API** | `LebGenerator` offers it | Makes output byte-identity caller-dependent, defeating the CI byte-identity test | `kMinimum` internally; accept padded on parse only |
| **Publishing to crates.io to reserve `iamf`** | The name is free | PROJECT.md: `publish = false` until it does something real | Path dependency |

---

## 13. Feature dependencies

```
uleb128 + byte-aligned BitWriter
    └──requires──> nothing
            │
            ▼
OBU header (type/flags/size/trim/extension)
    ├──enables──> IA Sequence Header (31)
    ├──enables──> Codec Config (0) ──requires──> LPCM decoder config
    ├──enables──> Audio Element (1) ──requires──> Codec Config id
    │                     └──requires──> ScalableChannelLayoutConfig (1 layer)
    ├──enables──> Mix Presentation (2)
    │                     ├──requires──> Audio Element id
    │                     ├──requires──> Mix Gain param definitions (element + output)
    │                     └──requires──> ≥1 layout, one of which MUST be stereo
    └──enables──> Audio Frame (6+id)
                          ├──requires──> LPCM interleave + BCG channel→substream packing
                          └──requires──> trimming (final frame)
                                    │
                                    ▼
                        M1: standalone .iamf that libiamf decodes
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        ▼                           ▼                           ▼
   OBU parser              FLAC / Opus framing          Profile selection
        ├──requires──> trailing-bytes footer          (needs channel counts
        ├──enables──> round-trip equality              per layout — already
        └──REQUIRES──> fuzz target (M2, non-negotiable)  built for M1)
                                    │
                                    ▼
                            Parameter Blocks (3)
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
            Mix gain automation          Demixing + Recon Gain
                                                    │
                                    ──requires──> multi-layer scalable element
                                                    │
                                                    ▼
                                        Expanded layouts (Base-Enhanced)

Ambisonics mono mode ──enables──> Ambisonics projection mode
Standalone .iamf     ──enables──> ISO-BMFF (written by us)
Parser               ──enables──> native decoder (if wanted at all — Open Q #3)
```

### Dependency notes

- **Mix Presentation requires Mix Gain param definitions, but not Parameter Blocks.** This is the
  single most useful dependency fact for M1: the structure is mandatory, the OBUs are not.
  `param_definition_mode = 1` + `default_mix_gain = 0` + zero Parameter Blocks is a shipped
  conformance vector `[WIRE]`.
- **Trimming is a hard dependency of the Core Value**, not a polish item. Without it the returned PCM
  is longer than the input and "sample-identical" fails.
- **Recon gain and demixing depend on multi-layer**, which depends on nothing being wrong in the
  single-layer path. They are strictly downstream; deferring them costs nothing.
- **The fuzz target depends on the parser and nothing else** — which is why PROJECT.md can pin it to
  M2 and why it genuinely cannot land in M1.
- **Profile selection is nearly free once channel counting exists**, and channel counting is needed
  anyway to validate `substream_count`/`coupled_substream_count`. Consider pulling it into M1.
- **The `trailing` footer conflicts with a naive "parse into exhaustive enums" model.** Decide early:
  unknown trailing bytes must survive a round trip, so every OBU struct needs the field from day one.
  Retrofitting it after M2's equality test is written is painful.

---

## 14. MVP definition

### Launch with (M1 — a file `libiamf` decodes)

- [ ] **uleb128 codec + byte-aligned bit writer** — everything sits on it
- [ ] **OBU header serialise** (§2) — the thing PROJECT.md said not to guess; now specified
- [ ] **IA Sequence Header (31)** — 6 bytes, Simple/Simple
- [ ] **Codec Config (0) + LPCM** — with **derived** `audio_roll_distance`
- [ ] **Audio Element (1)**, channel-based, single layer, gain flags clear
- [ ] **Mix Presentation (2)**, one sub-mix, mandatory stereo layout, caller-supplied loudness
- [ ] **Mix Gain param definitions**, mode 1, `default_mix_gain = 0` — mandatory structure
- [ ] **Audio Frames**, implicit substream ID (types 6–23), LPCM interleave, **BCG packing**
- [ ] **Trimming on the final frame** — required for sample-identical round-trip
- [ ] **Descriptor + temporal-unit ordering**, IDs sorted ascending, `BTreeMap` everywhere
- [ ] **Byte-comparison test against `libiamf/tests/test_000003.iamf`** — reproduce a golden file
      byte-for-byte before trying anything else. It is 32567 bytes and it is already on disk.
- [ ] **`libiamf` decode-and-compare** — the actual Core Value gate

**Deliberately excluded from M1:** Parameter Blocks, Temporal Delimiters, multi-layer, recon gain,
output gain, expanded layouts, ambisonics, FLAC, Opus, ISO-BMFF, the parser.

### Add after validation (M2 / M3)

- [ ] **OBU parser + `trailing` footer + fuzz target** — fuzzer ships with the parser, non-negotiable
- [ ] **Round-trip equality** on our output and on `iamf-tools` goldens
- [ ] **Profile selection incl. minimum-fit** — cheap, and unblocks larger beds
- [ ] **FLAC framing** — trigger: LPCM decode-and-compare is green
- [ ] **Opus framing** — trigger: same
- [ ] **Expanded layouts 0–12 (9.1.6)** — trigger: Parallax needs a bed larger than 7.1.4

### Future consideration (M4+)

- [ ] **Shaped exporter API** — blocked on §7.4 (parameter tick rate)
- [ ] **Parameter Blocks + mix gain automation** — same blocker
- [ ] **Multi-layer + demixing + recon gain** — only once single-layer is proven in the field
- [ ] **Ambisonics mono, then projection**
- [ ] **ISO-BMFF, written by us**
- [ ] **Native decoder** — Open Q #3; shelling out to `libiamf` in tests may be permanently sufficient

---

## 15. Prioritisation matrix

| Feature | User value | Cost | Priority |
|---|---|---|---|
| OBU header + uleb128 + bit writer | HIGH | LOW | **P1** |
| IA Sequence Header / Codec Config / Audio Element / Mix Presentation (minimal forms) | HIGH | MEDIUM | **P1** |
| LPCM framing + BCG channel packing | HIGH | MEDIUM | **P1** |
| Trimming | HIGH | MEDIUM | **P1** |
| Byte-comparison against shipped golden vectors | HIGH | LOW | **P1** |
| `libiamf` decode-and-compare harness | HIGH | MEDIUM | **P1** |
| Deterministic ordering (`BTreeMap`, sorted IDs, minimal uleb128) | HIGH | LOW | **P1** |
| `trailing` forward-compat footer | MEDIUM | LOW | **P1** (cheap now, expensive later) |
| OBU parser | HIGH | MEDIUM | **P2** |
| Fuzz target | HIGH | MEDIUM | **P2** (hard-pinned to M2) |
| Round-trip equality | HIGH | MEDIUM | **P2** |
| Profile selection | MEDIUM | LOW | **P2** |
| FLAC framing | MEDIUM | MEDIUM | **P2** |
| Opus framing | MEDIUM | MEDIUM | **P2** |
| Expanded layouts 0–12 | MEDIUM | MEDIUM | **P2** |
| Parameter Blocks + mix gain automation | MEDIUM | MEDIUM | **P3** (blocked on §7.4) |
| Temporal Delimiters | LOW | TRIVIAL | **P3** |
| Ambisonics mono | LOW | MEDIUM | **P3** |
| Multi-layer + recon gain + demixing | LOW | HIGH | **P3** |
| ISO-BMFF | MEDIUM | HIGH | **P3** |
| Native decoder | LOW | HIGH | **P3** |
| AAC-LC | LOW | MEDIUM | **P3** |
| Object-based elements / position params / Metadata OBU / profiles 3–5 | — | — | **NEVER (v1.1.0 target)** |

---

## 16. Reference implementation comparison

| Capability | `iamf-tools` (encoder, HEAD) | `libiamf` (decoder) | `eclipsa-audio-plugin` | **iamf-rs plan** |
|---|---|---|---|---|
| Spec version | draft **v2.0.0** | **v1.1.0** | v1.1.0-era | **v1.1.0** — match the decoder |
| Language | C++20 + Abseil + protobuf | C99 | C++ + JUCE | Rust 2024, no runtime deps beyond core |
| OBU types emitted | 0–5, 6–23, 24, 31 | parses all | via `iamf-tools` | 0,1,2,31 + 6–23 in M1; 3,4,5 later |
| Codecs (encode) | LPCM, FLAC, Opus, AAC | decodes all four | — | LPCM → FLAC → Opus. No AAC |
| Element types | channel, scene, **object** | all | channel, scene | channel (M1), scene (later). **No object** |
| Scalable layers | full multi-layer + recon gain | full | **one layer** | one layer; multi-layer deferred |
| Profiles | 0–5 | 0–2 | 0–2 | 0–2 |
| Config input | protobuf text | — | plugin state | typed Rust builder |
| Determinism | sorts IDs explicitly for it `[ENC]` | n/a | n/a | **enforced** — `BTreeMap`, minimal uleb128, CI byte-identity |
| Error handling | `absl::Status` | int codes | JUCE | `thiserror`, no `unwrap` |
| Fuzzing | — | — | — | **required on the parser (M2)** |

**The competitive position is unusual and worth stating plainly:** there is no Rust IAMF crate, both
references are permissively licensed, and the decoder ships ~340 conformance vectors as `.iamf` files
plus matching `.textproto` descriptions. That is an exceptionally good test oracle for a bitstream
library — better than ADM BWF, which has normative prose and no reference implementation.

---

## 17. Gaps and open items

1. **Parameter tick rate (PROJECT.md Open Q #1)** — facts gathered in §7.4; a recommendation is
   offered but the decision is Parallax's and belongs to the API-design phase.
2. **`libiamf` HEAD also contains a Metadata OBU parser and audio frame IDs beyond v1.1.0.** Its
   README claims v1.1.0. The exact v1.1.0/v2.0 boundary *inside `libiamf`* was not fully traced.
   **Mitigation: use the shipped `tests/*.iamf` vectors as the definition of "accepted", not the
   HEAD source.** Those vectors are tagged `github/aomediacodec/libiamf/v1.0.0-errata` and
   `.../main`.
3. **AAC decoder config layout** — not read; out of scope, but a future decoder needs it.
4. **Ambisonics projection config wire format** — only the mono path was read in detail.
5. **`iamf-tools` tags `v2.0.0` / `v2.1.0` exist** but their trees were not fetched (shallow clone).
   If a v1.1.0-exact reference tree is wanted, one of those is probably it — worth 5 minutes before
   the type model is written.
6. **The `.iamf` file extension has no magic beyond the IA Sequence Header.** Sync is on the `"iamf"`
   4CC inside OBU 31 `[ENC]`+`[WIRE]`. No container header, no footer, no index.
7. **PROJECT.md edits recommended:** pin **v1.1.0** (§1.1); reword Correction #5's numbering (§1.4);
   note that Corrections #1/#2 describe Eclipsa and the target profiles rather than `iamf-tools`
   generally (§1.3); add "do not mirror `iamf-tools` HEAD" as an explicit constraint (§1.2).

---

## Sources

**Reference implementation source (read under permissive licences):**
- `AOMediaCodec/iamf-tools` @ `901a86e` (2026-09-02) and tag `v1.0.0` @ `ce54b9a` (2024-01-26) —
  BSD-3-Clause-Clear + AOM Patent License 1.0. Files cited inline: `iamf/obu/obu_header.{h,cc}`,
  `obu_base.cc`, `types.h`, `ia_sequence_header.cc`, `codec_config.{h,cc}`, `audio_element.{h,cc}`,
  `mix_presentation.{h,cc}`, `rendering_config.{h,cc}`, `audio_frame.cc`, `temporal_delimiter.h`,
  `parameter_block.cc`, `demixing_info_parameter_data.cc`, `recon_gain_info_parameter_data.cc`,
  `animated_parameter_data.h`, `ambisonics_config.cc`,
  `decoder_config/{lpcm,flac,opus}_decoder_config.{h,cc}`,
  `param_definitions/{param_definition_base,subblock_schedule,mix_gain,demixing,recon_gain}*.cc`,
  `common/leb_generator.{h,cc}`, `cli/{profile_filter,obu_sequencer_base,obu_sequencer_iamf,
  global_timing_module,parameter_block_partitioner,cli_util,channel_label,obu_with_data_generator}.cc`,
  `cli/codec/lpcm_encoder.cc`, `cli/testdata/*.textproto`, `CHANGELOG.md`.
- `AOMediaCodec/libiamf` @ `e55e183` (2026-08-21) — BSD-3-Clause-Clear. Files cited:
  `README.md`, `code/src/iamf_dec/obu/iamf_obu.{c,h}`, `code/src/iamf_dec/iamf_decoder.c`,
  and the golden bitstreams `tests/test_000003.iamf`, `tests/test_000031.iamf`.
- `google/eclipsa-audio-plugin` @ HEAD — Apache-2.0. Cited:
  `common/substream_rdr/substream_rdr_utils/Speakers.h:133–186`.

**Deliberately NOT consulted (licence hygiene):** `libspatialaudio` (LGPL-2.1+) and `gpac`
(LGPL-2.1) were not cloned, opened, searched or referenced at any point in this research.

**Specification:** `aomediacodec.github.io/iamf` was **not** fetched. Every `[SPEC]`-tagged claim in
this document comes from a code comment paraphrasing the spec and is marked unverified. Everything
load-bearing is `[ENC]`, `[DEC]` or `[WIRE]`.

---
*Feature research for: IAMF bitstream encoder/serialiser library*
*Researched: 2026-09-07*
