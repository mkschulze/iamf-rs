# Phase 1: Conformant LPCM Bitstream - Research

**Researched:** 2026-09-08
**Domain:** IAMF v1.1.0 bitstream serialisation in Rust; reference-implementation conformance oracles
**Confidence:** HIGH for every reference-implementation fact (read from source at named tags, and in several cases executed locally); MEDIUM for the container recipe; LOW for nothing that gates a plan.

---

## Provenance conventions used in this document

Per PROJECT.md's standing rule (five earlier claims were labelled verified-from-code and were not), every claim below carries one of:

- **`[VERIFIED-FROM-CODE: <repo>@<tag> <path>:<lines>]`** — file opened this session at that exact revision; values quoted verbatim.
- **`[VERIFIED-BY-EXECUTION]`** — I ran it on this machine and pasted the output.
- **`[VERIFIED-FROM-DATA]`** — computed from a shipped binary fixture this session.
- **`[CITED: <repo>@<tag> <path>]`** — the reference project's own prose/docs (README, changelog), not its code.
- **`[ASSUMED]`** — training knowledge or inference. Needs confirmation.

**Licence hygiene statement.** Sources read this session: `AOMediaCodec/iamf-tools` (BSD-3-Clause-Clear + AOM Patent License 1.0) and `AOMediaCodec/libiamf` (same). **`gpac`, `libspatialaudio` and FFmpeg/`libavformat` were NOT read.** The `ffmpeg` binary was *invoked* once (see Environment Availability); no FFmpeg source was opened. No search result led into `libavformat/iamf*.c`.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

Copied verbatim from `01-CONTEXT.md` `<decisions>`. All 26 are binding on the planner.

**Bit layer and the dependency floor**

- **D-01:** Hand-roll `BitCursor`/`BitWriter` over `&[u8]` (~150–300 lines) rather than wrapping `bitstream-io`. `bitstream-io` is demoted to a **dev-dependency**, used in a proptest that asserts our primitives agree with it bit-for-bit on random inputs — a differential oracle that never enters the shipping graph. This amends BITS-01's wording ("wrapping `bitstream-io`") while satisfying its intent, and resolves BITS-01's collision with API-05. — **Reversibility:** costly — the hand-rolled cursor produces byte offsets natively, and every `Error.at` value in the crate flows from it. Adopting `bitstream-io` later means reintroducing the `std::io::Error` mapping layer that BITS-07 exists to contain, at every primitive.
- **D-02:** API-05's "dependency-free" means **zero linked dependencies — proc-macro derive crates excepted**. `thiserror` stays unconditional (PROJECT.md constraint) despite pulling `thiserror-impl → syn → proc-macro2 → quote → unicode-ident`; none of those contribute code to the linked artifact. This definition MUST be written into `REFERENCES.md` and a `lib.rs` doc comment so Phase 4 does not rediscover it as a contradiction.
- **D-03:** The public uleb128 writer always emits **minimal form** (BITS-03 unchanged). A `pub(crate)` fixed-size encoder exists solely so the conformance harness can reproduce reference files exactly (CONF-08). No caller-visible `LebMode` — the Out-of-Scope exclusion is about the *public encoder*, and byte-identity must stay caller-independent.

**Type model and error shape**

- **D-04:** `audio_element_type` is modelled as `#[non_exhaustive] enum AudioElementType { ChannelBased(..), SceneBased(..), Reserved { value: u8, raw: Vec<u8> } }` from the first commit. Only `ChannelBased` is constructible through the **public encoder path**; `SceneBased` stays constructible internally or behind a test-only constructor, because PARSE-07 needs to build scene-based fixtures and "asserted understood" is unmeetable otherwise. Decode is a superset of encode — the encoder emits beds only, but Parallax's import adapter meets foreign files. Rationale the user weighted most: `Reserved { value, raw }` buys a **testable property**, not just forward compatibility — decode→encode of an element we do not understand is byte-identical, which is CI-checkable against fixtures nothing else can assert on, and strictly stronger than "we parsed it". It also matches Parallax's house rule — refuse with the reason named, never approximate. — **Reversibility:** one-way — this is a published public enum that Parallax's import adapter matches on.
- **D-05:** **Invariant to assert in the type's own docs:** `Reserved` can only round-trip if the parser can skip an unknown element's payload without understanding it, which requires the remaining length to be derivable. IAMF OBUs carry `obu_size`, so it is (`payload = obu_size − bytes consumed by the common fields`). **Wiring constraint for plan 01-05:** `Reserved.raw` and OBU-07's `trailing: Vec<u8>` are two mechanisms both claiming "everything after the fields I understood". They need a defined precedence — `raw` consumes to the end of the Audio Element payload and `trailing` stays empty — or the boundary is ambiguous and the round-trip property becomes untestable.
- **D-06:** For fields the writer **derives** (`audio_roll_distance`, `obu_size`, implicit substream IDs), the reader **preserves the wire value**. The model stores what was read. A separate `validate()` reports every derived-field mismatch by name — "audio_roll_distance is 2, expected 0 for LPCM" — without failing the parse. Fresh construction through the encoder derives the value as DESC-02 requires. Never silently normalise. — **Reversibility:** costly.
- **D-07:** Three roles, no overlap: the **writer is faithful** (serialises whatever the model holds, so `serialize(parse(bytes)) == bytes` survives for foreign files), **`validate()` is explicit** and never runs implicitly, and Phase 4's **`build()` is the mandatory validation point** for freshly-constructed models (API-01).
- **D-08:** The error type is `struct Error { kind: ErrorKind, at: Location }` where `Location = InputOffset(u64) | OutputOffset(u64) | Field(&'static str) | Unlocated`. One `Result` type everywhere; position attached once rather than repeated into every variant; the three error sources (read / write / validate) stay distinguishable rather than conflated. Satisfies GUARD-13's intent structurally and keeps `size_of::<Error>() <= 32` tractable. — **Reversibility:** one-way.
- **D-09:** `validate()` returns **all findings**, each naming its field path via `Location::Field` — `fn validate(&self) -> Vec<Finding>`, not `Result<(), Error>`. — **Reversibility:** costly.

**Reader scope in Phase 1**

- **D-10:** **Full read/write pairing from the first commit.** Every OBU type gets `read` and `write` in the same file, in that order, in the same commit. **Guard against the closed loop:** round-trip tests are **supplementary evidence only**. BITS-06's hand-computed vectors and the reference oracles remain the primary proof.

**Reference oracles and the conformance gate**

- **D-11:** **Split the oracles by build cost.** `tools/build-reference.sh` builds `libiamf` natively on macOS at its pinned SHA (CMake, one submodule, yields `iamfdec`), so `IAMF_REF_DECODER` is set locally and CONF-05 runs in seconds while iterating. `iamf-tools` (Bazel + abseil + protobuf + fdk-aac) runs **only in a digest-pinned container**, used for CONF-11's one-time fixture generation and the Linux CI job enforcing CONF-06/07. CONF-10 still holds: `cargo test` is green offline on all four targets with no reference binary present.
- **D-12:** CONF-07's byte-diff writeup is an **executable ledger** — a committed table of known differences (offset, field, our bytes, theirs, why) that a test asserts against **exactly**. A new difference fails; a difference that *disappears* also fails.
- **D-13:** GUARD-06 is enforced at runtime, not just by script. `tools/build-reference.sh` writes `.reference-manifest.json` recording the SHA it actually checked out plus a hash of each binary produced; the `iamf-tools` container image is pinned **by digest, not tag**; a test asserts the manifest matches `REFERENCES.md` **before any conformance clause runs**.
- **D-14:** CONF-11 fixture policy — one Bazel run generates everything; commit every output under a stated **per-file size cap**, plus a `MANIFEST.md` listing all textprotos, which produced output, which were committed, which were skipped and why, and the exact pinned-container command to regenerate any of them. — **Reversibility:** costly.
- **D-15:** **FFmpeg is an oracle, never a reference.** Phase 1 writes the boundary down: GUARD-08's `CONTRIBUTING.md` checkbox names **FFmpeg / libavformat** alongside `gpac` and `libspatialaudio`, and `REFERENCES.md` records the read-vs-invoke distinction explicitly. Phase 2 wires ffmpeg in as a third, independent-lineage read oracle.
- **D-16:** GUARD-09's matrix runs **everything on all four targets on every PR**. **Known operational risk for plan 01-01:** GitHub's `macos-13` is the last x86_64 macOS image and is on the retirement path. Plan 01-01 must carry a documented fallback rather than discovering it as a red gate.
- **D-17:** **Gate waiver policy.** A clause may be downgraded **only** by a committed `CONFORMANCE-GATE.md` entry naming the clause, what was attempted, why it cannot be met, what weaker property is asserted instead, and what would let it be restored.

**Conformance fixture design**

- **D-18:** The Phase 1 fixture is **two Codec Configs and one 5.1 Audio Element**, 48 kHz, 24-bit **big-endian**, length a deliberate non-multiple of the frame size, signal a per-channel deterministic index pattern. Two Codec Configs, one Audio Element — not two elements, because `iamfdec` decodes a *mix presentation* and a two-element fixture makes the decoded PCM a **mix**. 5.1, not stereo — the phase's highest silent-failure risk is BCG channel→substream packing. 24-bit big-endian — DESC-03's `sample_format_flags == 0` means *big*-endian. Non-multiple length — forces `trim_at_end > 0`. — **Reversibility:** costly. — **Research item for plan 01-02:** confirm `iamf-tools`' strict parser accepts a Codec Config that no Audio Element references. If it does not, fall back to differing the two configs and having the element use whichever is legal.
- **D-19:** **No permutation constant anywhere in the harness.** The fixture's input PCM is authored in the reference decoder's documented output order for the requested layout, cited in the harness. On mismatch, a diagnostic checks whether *some* permutation would have matched and reports "channels scrambled: ch2↔ch4". — **Reversibility:** costly.
- **D-20:** GUARD-09's committed golden is **three artifacts**: the `.iamf` fixture, a hash for the four-target comparison, and a **human-readable annotated structural dump** (offset, field name, value, hex) generated by our own dumper. **Caveat:** the dump is self-consistent with the writer, so it aids **review**, never correctness.

**Guardrails, hygiene and sequencing**

- **D-21:** GUARD-11's no-DSP guard is **clippy `disallowed-types` on `f32`/`f64`**, extending the same `clippy.toml` that already carries GUARD-02's `HashMap`/`HashSet` ban, plus `disallowed-methods` for transcendentals, each with a `reason` string. PROF-03's Q7.8 helper carries the **single** documented `#[allow]`.
- **D-22:** **Phase 1 ships no Cargo features at all.** **Note for the doc pass:** PROJECT.md's Key Decisions line "One crate, feature-gated `encode`/`decode`" reads as a standing description and will be implemented faithfully in 01-01 by an executor unless amended to read as a Phase 4 intention.
- **D-23:** STACK.md's `// ref:` citation discipline gets a **mechanical test**: a test walks `src/`, finds every `fn read_*` / `fn write_*`, and fails if one lacks a preceding `// ref:` line.
- **D-24:** Plans **01-01 → 01-02 run sequentially**, both before any bitstream code (01-03 onward).
- **D-25:** With `tdd_mode: true`, expected bytes come from **hand-decoded vectors first, reference capture as a second, independent assert**. **Rejected:** capturing reference output as the expected bytes and hand-decoding only on failure.
- **D-26:** **Worktree cleanup — done during the discussion** (commit `41eaa4e`). Plan 01-01 starts from a clean tree.

### Claude's Discretion

The user explicitly delegated these:

- Module and file layout under `src/` (constrained only by BITS-07 and D-10's pairing rule)
- CI provider specifics and workflow file structure
- The `iamf-tools` container base image and whether it is built in-repo or pulled
- The annotated dump's output format (D-20)
- The `Finding` type's exact fields (D-09)
- The golden-hash file's format and location
- Plan-level task breakdown within each of the 8 plans
- DEC-04 (`iamf-tools` v1.x tag identification) and DEC-05 (`libiamf` payload rejection rules) go to the researcher, not back to the user

### Deferred Ideas (OUT OF SCOPE)

- **PROJECT.md / REQUIREMENTS.md doc pass** (run separately): close Open Question 2 with the offline-only formulation; correct the 5.1/5.2/5.3 motivation split; soften DEC-03's recorded cost to mix-gain automation only; mark DECO-02 as dead text; amend the "One crate, feature-gated `encode`/`decode`" Key Decision to read as a Phase 4 intention.
- **Phase 2:** wire ffmpeg in as a third, independent-lineage read oracle (D-15).
- **v2 / decoder:** the `IO.md` §1 object-element import mapping collides with IAMF v1.1.0's two-value `audio_element_type`. Not a gap in this crate — a spec-version consequence the v2 decoder work must confront.
- Rendering, panning, spatial DSP; a resampler; loudness measurement; UI; object-based elements; mirroring `iamf-tools` HEAD's type model; `gpac`; a `LebMode` knob on the public encoder; `build.rs`-driven reference builds; publishing to crates.io.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

63 requirement IDs land in this phase. Grouped by the plan that owns them, with the research finding that unblocks each group.

| Plan | Requirement IDs | Research Support |
|------|-----------------|------------------|
| 01-01 | GUARD-01, GUARD-02, GUARD-03, GUARD-04, GUARD-05, GUARD-07, GUARD-08, GUARD-09, GUARD-10, GUARD-11, GUARD-12, GUARD-13, DEC-01, DEC-02 | §Standard Stack (verified crate versions/licences); §Security Domain; §AOM Patent License 1.0 §1.2 — **a `PATENTS` file at repo root is a condition of the inbound patent grant**; §Environment Availability |
| 01-01/01-02 | GUARD-06, DEC-04 | §DEC-04 — pin `iamf-tools` **v2.1.0** `848c6ff…` and `libiamf` **v1.1.0** `f06e919…`, with the evidence that both are v1.1.0-exact trees |
| 01-02 | CONF-09, CONF-11 | §Reference Build Recipes — a *working, executed* macOS `libiamf` + `iamfdec` recipe including the non-obvious `dep_codecs/lib` step; §Fixture Supply — 226 reference `.iamf` files already exist, correcting "338 textprotos and exactly one `.iamf`" |
| 01-03 | BITS-01, BITS-02, BITS-03, BITS-04, BITS-05, BITS-06, BITS-07 | §leb128 minimality — 524 674 `obu_size` fields across 226 reference files, **zero non-minimal**; §Verified byte layouts; §Reference primitive surface |
| 01-04 | OBU-01, OBU-02, OBU-03, OBU-04, OBU-05, OBU-06, OBU-07, OBU-08 | §CORRECTION 1 — **OBU-03 as written mirrors a draft-v2.0.0 tree**; the v1.1.0 model has two variants, not four. §Verified byte layouts (hand-decoded `test_000003.iamf` prologue, all 67 OBUs walked) |
| 01-05 | DESC-01…DESC-09 | §Verified byte layouts (full 120-byte descriptor prologue field-by-field); §Codec Config rejection rules; §Mix Presentation stereo-layout rule with exact source line; §Layout enums quoted verbatim |
| 01-06 | TIME-01, TIME-02, TIME-03, TIME-04, TIME-05 | §BCG packing for 5.1 — resolved three independent ways, including from a shipped 5.1 PCM file's own OBU sizes; §MixGainParameterData animation types |
| 01-07 | SEQ-01, SEQ-02, SEQ-03, PROF-01, PROF-02, PROF-03 | §Profile limits quoted from `profile_filter.cc`; §Descriptor write order from `obu_sequencer_base.cc` |
| 01-08 | CONF-01…CONF-08, CONF-10, DEC-05 | §DEC-05 — full rejection-rule enumeration; §**The peak limiter** — a decision-changing constraint on the fixture's amplitude; §CONF-08 — **the configuration IS published**, which retires D-17's stated risk for that clause |
</phase_requirements>

---

## Summary

Three findings change plans, and one retires a risk the phase was carrying.

**DEC-04 is settled with strong evidence, and the answer is `v2.1.0`.** `iamf-tools` version numbers track the *tool*, not the spec. Both `v2.0.0` and `v2.1.0` are IAMF v1.1.0-exact trees: three profiles, no Metadata OBU, `audio_element_type` ∈ {0,1} with 2–7 reserved, parameter definition types {0,1,2} with 3+ reserved. `main` is the draft-v2.0.0 tree with six profiles, `kObuIaMetadata = 24`, `kAudioElementObjectBased = 2` and an `ObjectsConfig`. Pin **`v2.1.0` = `848c6ff4968ff8cc6f728259892ab4f90cb83256`**; it is v1.1.0-exact *and* carries v2.1.0's security fixes.

**A requirement is wrong, and it is wrong in exactly the way PROJECT.md predicted.** OBU-03 requires bit 6 be "a polymorphic enum … trimming status for audio frames, inverted `is_not_key_frame` for temporal delimiter, reserved-SHALL-be-0 elsewhere", and PITFALLS.md §2 adds `optional_fields_flag` for Mix Presentation. At `iamf-tools` **v2.1.0** the field is called `obu_trimming_status_flag` and `IsTrimmingStatusFlagAllowed()` returns `true` **only for Audio Frames**; setting it on a Temporal Delimiter is an `InvalidArgumentError`. `GetIsKeyFrame`/`GetOptionalFieldsFlag`/`type_specific_flag` **do not exist anywhere in `iamf/obu/` at v2.1.0** — they are `main`-only, and `main`'s own comment says "In IAMF v2.0.0, if `optional_fields_flag` is true…". PITFALLS.md read `main`. Under DEC-01 the correct v1.1.0 model has **two** variants: `Trimming(Option<Trimming>)` for types 5 and 6–23, and `Reserved` for everything else.

**`libiamf` always applies a peak limiter, and that constrains the fixture's amplitude.** `IAMF_decoder_open()` unconditionally creates a limiter at −1.0 dBTP with 1 ms attack, 200 ms release and 240 samples of look-ahead. Below threshold the gain is exactly `1.0` so the path is bit-exact — I confirmed this by building `iamfdec` on this machine and decoding `test_000003.iamf` to a WAV sample-identical with its source, 8000 frames, with the limiter *on*. Above −1 dBFS the gain leaves 1.0 for up to 201 ms and CONF-05's sample-identity assertion fails with a diffuse amplitude error that looks like a DSP bug. **D-18's per-channel index pattern must be scaled to peak ≤ −6 dBFS, and the harness should pass `-disable_limiter` belt-and-braces.** (`iamfdec` at v1.1.0 has that flag.)

**And one risk is retired.** D-17 pre-authorised a waiver for CONF-08 because `test_000003.iamf`'s "equivalent configuration is not published and must be inferred from the file". It *is* published: `libiamf@v1.1.0 tests/test_000003.textproto` sits beside the binary, and `iamf-tools@v2.1.0 iamf/cli/testdata/test_000003.textproto` is the same configuration in the current proto dialect. CONF-08 is now a mechanical reproduction from a written spec, not archaeology. I also hand-decoded the whole 120-byte descriptor prologue field by field (PROJECT.md says 118 — see CORRECTION 5) and walked all 67 OBUs to `bytes.len()` exactly.

**Primary recommendation:** pin `iamf-tools@v2.1.0` and `libiamf@v1.1.0`; rewrite OBU-03's model to the two-variant v1.1.0 shape before plan 01-04 is written; cap the conformance fixture at −6 dBFS; and build `libiamf` on macOS with `dep_codecs/lib/*.a` moved aside, which yields a dependency-free LPCM-only decoder in under a minute.

---

## Architectural Responsibility Map

This is a library with no tiers in the web sense. The equivalent axis is *which artifact owns a capability*, and the phase's real risk is capability leaking across the crate/harness/reference boundary.

| Capability | Primary Owner | Secondary Owner | Rationale |
|------------|---------------|-----------------|-----------|
| Bit-level read/write primitives | `iamf` crate, `src/bits/` only | — | BITS-07: `std::io::Error` (if `bitstream-io` ever returns) is mapped at this boundary and never escapes. Nothing else in the crate touches raw bit offsets. |
| uleb128 encode/decode | `src/bits/` | — | D-03: public writer is minimal-only; the fixed-size encoder is `pub(crate)` and reachable only from the conformance harness. |
| OBU framing (`obu_size`, header byte, trim, extension) | `iamf` crate, one module | — | Pitfall 1: the `obu_size` origin must be computed in exactly one function so it can be cited once. |
| Descriptor/type model + `validate()` | `iamf` crate | — | D-06/D-07/D-09: the writer is faithful; `validate()` is explicit and returns all findings. |
| Derived-value computation (`audio_roll_distance`, `obu_size`, implicit substream IDs) | `iamf` crate encoder path | `validate()` reports mismatch on the read path | D-06. Never normalise on read. |
| BCG channel→substream packing | `iamf` crate | — | TIME-02. It is a *permutation*, not DSP: no coefficient, no arithmetic on sample values. Stays inside the scope boundary. |
| Rendering / mixing / summing | **`libiamf` only** (in the harness), never this crate | Parallax `D-40` in production | The fixture is designed (D-18) so `iamfdec`'s renderer is a 6×6 identity matrix — see §BCG. |
| Loudness *measurement* | **Nobody in this repo** | Parallax `MON-05` | Out of Scope. The crate carries caller-supplied numbers and quantises them (PROF-03). |
| Float→fixed quantisation (Q7.8) | `iamf` crate, one function, one `#[allow]` | — | D-21 makes the guard a census: `rg 'allow.*disallowed_types'` must return exactly one hit. |
| Sample-identity assertion, byte-diff ledger, golden hash | `tests/` harness (`assert_conformant`) | — | CONF-01: a reusable function, not a test body, so Phase 3 reuses it unchanged. |
| Reference binaries | `tools/build-reference.sh` + `IAMF_REF_DECODER` | Digest-pinned container for `iamf-tools` | CONF-09/D-11. Never a `build.rs`. |

---

## DEC-04 — Settled: pin `iamf-tools` v2.1.0

### The available tags

```
ce54b9a8db8cb382b073736f9f54e795afbb02fb  refs/tags/v1.0.0   (2024-01-26)
bee0f286f7d3928551e2fb37b5961b709eb1f714  refs/tags/v2.0.0   (2025-08-18)
848c6ff4968ff8cc6f728259892ab4f90cb83256  refs/tags/v2.1.0   (2025-11-06)
901a86e19e32e20dbdf0268984ae75f3f481249c  refs/heads/main    (2026-09-02)
```
`[VERIFIED-BY-EXECUTION: git ls-remote --tags https://github.com/AOMediaCodec/iamf-tools, 2026-09-08]`

There is **no `v1.1.x` tag.** The version number is the tool's, not the spec's. `v2.1.0`'s changelog says v2.0.0 added "support for encoding Standalone IAMF Representation for Base-Enhanced profile based on **[IAMF v1.1.0]**" and "Update Simple and Base profile to be based on **[IAMF v1.0.0-errata]**". `[CITED: iamf-tools@v2.1.0 CHANGELOG.md]`

### The four discriminating checks

Every one of these separates a v1.1.0-exact tree from the draft-v2.0.0 tree. `v2.0.0` and `v2.1.0` pass all four; `main` fails all four.

| Check | `v2.0.0` | `v2.1.0` | `main` |
|---|---|---|---|
| `ProfileVersion` | `kIamfSimpleProfile = 0, kIamfBaseProfile = 1, kIamfBaseEnhancedProfile = 2, kIamfReserved255Profile = 255` | identical | adds `kIamfBaseAdvancedProfile = 3, kIamfAdvanced1Profile = 4, kIamfAdvanced2Profile = 5` |
| OBU type 24 | `kObuIaReserved24 = 24,` | identical | `kObuIaMetadata = 24,` |
| `AudioElementType` | `kAudioElementChannelBased = 0, kAudioElementSceneBased = 1,` + `// Values in the range of [2 - 7] are reserved.` | identical | `kAudioElementObjectBased = 2,` + `// Values in the range of [3 - 7] are reserved.` and `ObjectsConfig` in the variant |
| `ParameterDefinitionType` | `kParameterDefinitionMixGain = 0, kParameterDefinitionDemixing = 1, kParameterDefinitionReconGain = 2,` + `// Values in the range of [3, (1 << 32) - 1] are reserved.` | identical | (header restructured; `main` is not v1.1.0 here either) |

`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/ia_sequence_header.h:27-32, iamf/obu/obu_header.h:53, iamf/obu/audio_element.h:290-296, iamf/obu/param_definitions.h:39-46]` and `[VERIFIED-FROM-CODE: iamf-tools@v2.0.0 / @main, same paths, via git show]`

### Recommendation

**Pin `v2.1.0` = `848c6ff4968ff8cc6f728259892ab4f90cb83256`.**

Reasons, in order:
1. It is v1.1.0-exact by all four checks.
2. Its changelog's `### Security` entry reads "Fix potential buffer overflows, invalid memory access, or excessive memory usage for certain bitstreams." `[CITED: iamf-tools@v2.1.0 CHANGELOG.md]` A reference we run over our own output in CI should be the hardened one.
3. Nothing in v2.1.0's changelog changes encoder *output* bytes. The changes are API shape (serialized protos), deprecated redundant size fields (values still computed identically), logging macros, and decode-side fixes.
4. It ships an example decoder (`decoder_main`) that v1.0.0 does not.

**Caveat to record in `REFERENCES.md`:** v2.1.0 "Changed encoder API to take in serialized protos", and v2.0.0 deprecated `count_label`, `num_substreams`, `num_layers`, `num_sub_mixes`, `num_audio_elements`, `num_layouts`, `param_definition_size` and others in favour of deriving them. `libiamf@v1.1.0 tests/*.textproto` are written in the *old* dialect and still contain those fields. If plan 01-02 feeds a textproto to `encoder_main`, use the copy from **`iamf-tools@v2.1.0 iamf/cli/testdata/`**, not the one in `libiamf/tests/`. I diffed `test_000003.textproto` between the two: the only differences are the deprecated-field removals, the `channel_ids`/`channel_labels` → `channel_metadatas` migration, and an added `encoder_control_metadata` block. `[VERIFIED-BY-EXECUTION: diff, 2026-09-08]`

### `libiamf` pin

```
f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63  refs/tags/v1.1.0   (2024-11-01)
e55e1832a608affe602de2ee39929bd7759a75ab  refs/heads/main    (2026-08-21)
tags also present: v1.0.0, v1.0.0-errata, v1.0.1, v2.0.0-wga-draft
```
`[VERIFIED-BY-EXECUTION: git log/tag, 2026-09-08]`

**Pin the `v1.1.0` tag = `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63`.** It matches DEC-01 exactly, it is the AOM Final Deliverable release, and it builds and runs correctly on this machine (§Reference Build Recipes).

**Do not pin `main`.** Although `main`'s README still says "reference decoder for AOM IAMF v1.1.0" `[CITED: libiamf@main README.md]`, its code has drifted toward the v2 draft: `iamf_obu.c` dispatches `case ck_iamf_obu_metadata:` and `iamf_obu_raw_is_reserved_obu()` tests `profile > ck_iamf_profile_base_enhanced`. `[VERIFIED-FROM-CODE: libiamf@main code/src/iamf_dec/obu/iamf_obu.c:123, 186]` That is the same drift PROJECT.md forbids mirroring.

**Consequence for the roadmap's Research line.** The roadmap says to read `libiamf`'s `codec_config_obu.c` and `audio_frame_obu.c`. **Those files do not exist at `v1.1.0`** — the decoder is one monolithic `code/src/iamf_dec/IAMF_OBU.c` there; the `obu/` split is a `main` refactor. I read **both**: `main`'s split files (clearer, and the roadmap's target) and `v1.1.0`'s monolith (the file we will actually pin). Differences are called out below.

---

## DEC-05 — Settled: payload-level rejection rules

Two columns, because `libiamf@v1.1.0` (what we pin) and `libiamf@main` (what the roadmap named) differ, and `iamf-tools@v2.1.0` is strictly stricter than both. **The union is what the encoder must satisfy.**

### Codec Config OBU (type 0)

Wire order, confirmed by hand-decode: `codec_config_id` (uleb128) · `codec_id` (4 bytes, 4CC) · `num_samples_per_frame` (uleb128) · `audio_roll_distance` (**signed 16, big-endian**) · `decoder_config` (**the entire remainder of the OBU payload**).

Both `libiamf` revisions compute `decoder_config_size = payload_size − bytes_consumed_so_far` and read that many bytes. `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:337-345]` / `[VERIFIED-FROM-CODE: libiamf@main code/src/iamf_dec/obu/codec_config_obu.c:51-64]` There is **no length field** for the decoder config — get `obu_size` wrong and the LPCM config silently absorbs or loses bytes.

| Rejection | `libiamf@v1.1.0` | `libiamf@main` | `iamf-tools@v2.1.0` |
|---|---|---|---|
| `codec_id` not one of `Opus` / `mp4a` / `fLaC` / `ipcm` | **reject** (`_valid_codec` → `iamf_codec_check`) | **reject** (`_obu_cc_codec_id_check`) | reject (`Unknown codec_id`) |
| `num_samples_per_frame == 0` | **accepted** — decoder then yields 0 samples | **reject**: `"number of samples per frame should not be zero."` | reject — `ValidateInRange(nspf, {1, 96000})` |
| `num_samples_per_frame > 96000` | accepted | accepted | **reject** — `kMaxPracticalFrameSize = 96000` |
| Opus `decoder_config[0] > 15` | reject | reject | (separate Opus validation) |
| `audio_roll_distance != 0` for LPCM | **not checked** | **not checked** | **reject** — `ValidateEqual(audio_roll_distance, LpcmDecoderConfig::GetRequiredAudioRollDistance())`, which returns `0` |
| `sample_format_flags ∉ {0, 1}` | **not checked** | **not checked** | **reject** — `absl::UnimplementedError` |
| `sample_size ∉ {16, 24, 32}` | **not checked**; PCM init silently falls through to a 16-bit LE reader | **not checked** | **reject** — `ValidateSampleSize` |
| `sample_rate ∉ {16000, 32000, 44100, 48000, 96000}` | **not checked** | **not checked** | **reject** — `ValidateSampleRate` |
| Two Codec Configs with different sample rate or bit depth | n/a (decoder) | n/a | **reject** — `"Codec Config OBUs with different bit-depths and/or sample rates are not in base-enhanced/base/simple profile; they are not allowed in ISOBMFF."` |
| Two Codec Configs with different `num_samples_per_frame` | n/a | n/a | **reject** — `"The encoder does not support Codec Config OBUs with a different number of samples per frame yet."` |

`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:250-252, 305-320, 322-400; code/src/iamf_dec/pcm/IAMF_pcm_decoder.c:36-70]`
`[VERIFIED-FROM-CODE: libiamf@main code/src/iamf_dec/obu/codec_config_obu.c:109-148, 246-260]`
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/codec_config.cc:38-56; iamf/obu/codec_config.h:91; iamf/obu/decoder_config/lpcm_decoder_config.cc:27-90; iamf/cli/cli_util.cc:224-243; iamf/cli/obu_sequencer_base.cc:178-202]`

**The endianness question, settled from the code that acts on it.** `libiamf@main`'s comment is *wrong* and the code is right:

```c
  /**
   * @brief LPCM Specific
   * b08: sample_format_flags: 0x01 - big endian, 0x00 - little endian   <-- COMMENT IS INVERTED
   * b08: sample_size
   * b32: sample_rate
   */
  param->big_endian = !ior_8(r);
```
`[VERIFIED-FROM-CODE: libiamf@main code/src/iamf_dec/obu/codec_config_obu.c:250-256]`

`v1.1.0` agrees, in the reader-selection logic:
```c
  ctx->func = reads16le;
  ctx->scale_i2f = 1 << 15;
  if (ths->sample_size == 16) {
    if (!ths->flags) ctx->func = reads16be;
  } else if (ths->sample_size == 24) {
    ctx->scale_i2f = 1 << 23;
    if (!ths->flags)
      ctx->func = reads24be;
    else
      ctx->func = reads24le;
  } else if (ths->sample_size == 32) { ... }
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/pcm/IAMF_pcm_decoder.c:51-68]`

And `iamf-tools` names the values:
```cpp
  enum LpcmFormatFlagsBitmask : uint8_t {
    kLpcmBigEndian = 0x00,
    kLpcmLittleEndian = 0x01,
    kLpcmBeginReserved = 0x02,
    kLpcmEndReserved = 0xff,
  };
```
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/decoder_config/lpcm_decoder_config.h:30-35]`

**DESC-03 is confirmed three ways. `sample_format_flags == 0` means big-endian.** Also note the trap in the same snippet: an out-of-range `sample_size` falls through to `reads16le` with `scale_i2f = 1 << 15` and **no error**. A `sample_size` of 8 produces plausible-sounding garbage.

### Audio Frame OBU (types 5, 6–23)

`libiamf` performs **no payload-level rejection at all**. It reads an explicit `audio_substream_id` uleb128 only when `obu_type == 5`, derives `substream_id = obu_type − 6` for types 6–23, copies the trim values in from the header, and takes the whole remainder as the frame. `[VERIFIED-FROM-CODE: libiamf@main code/src/iamf_dec/obu/audio_frame_obu.c:47-63]` / `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:1310-1336]` This confirms TIME-01's implicit-ID rule from both sides.

Everything that can go wrong with an audio frame is caught (or silently tolerated) *downstream*, in the PCM decoder:

| Condition | `libiamf@v1.1.0` behaviour |
|---|---|
| Number of frames in a temporal unit ≠ `substream_count` | `if (!count \|\| count != ths->streams) return IAMF_ERR_BAD_ARG;` — **hard error** |
| A coupled substream's byte length disagrees with substream 0's | `ia_loge(...)` then `return IAMF_ERR_INTERNAL` — **hard error** |
| Decoded sample count ≠ `num_samples_per_frame` | `ia_logw("real samples and frame size are different: %d vs %d", ...)` — **warning only** |

`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/pcm/IAMF_pcm_decoder.c:80-112]`

`iamf-tools` adds, on top: within one temporal unit **every** audio frame must carry identical `num_samples_to_trim_at_start`, identical `num_samples_to_trim_at_end` and identical timestamps; substream IDs must be unique within the temporal unit. `[CITED: PITFALLS.md §3, read from iamf-tools cli/temporal_unit_view.cc — I did not re-open that file this session, so treat the wording as CITED not VERIFIED]`

### IA Sequence Header (type 31) — a rejection rule neither research document recorded

```c
static int _valid_profile(uint8_t primary, uint8_t addional) {
  return primary < IAMF_PROFILE_COUNT && primary <= addional;
}
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:250-252]` — and identically at `main` as `_obu_sh_valid_profile`, `code/src/iamf_dec/obu/ia_sequence_header_obu.c:69-71`.

**`additional_profile` MUST be ≥ `primary_profile`,** or `libiamf` rejects the whole sequence. `IAMF_PROFILE_COUNT == 3` at v1.1.0 (`IAMF_PROFILE_SIMPLE, IAMF_PROFILE_BASE, IAMF_PROFILE_BASE_ENHANCED, IAMF_PROFILE_COUNT`) `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_types.h:135-141]`. `ia_code` must equal `0x69616d66` in both. Note this cuts against PITFALLS.md §11's "do not be stricter than the reference on `additional_profile`" — `iamf-tools` does not validate it, but **`libiamf` does**, and `libiamf`'s acceptance is the Core Value. Emitting `primary = additional = 0` (Simple), as `test_000003` does, is safe.

### The one thing `libiamf@v1.1.0` does that `main` does not

`IAMF_OBU_split` reads the trim fields whenever the header bit is set, **for any OBU type**:
```c
  if (obu->trimming) {
    obu->trim_end = bs_getAleb128(&b);    // num_samples_to_trim_at_end;
    obu->trim_start = bs_getAleb128(&b);  // num_samples_to_trim_at_start;
  }
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:64-120]`

`main` guards it with `_iamf_obu_type_is_audio_frame(h->obu_type) && h->obu_trimming_status_flag`. `[VERIFIED-FROM-CODE: libiamf@main code/src/iamf_dec/obu/iamf_obu.c:78-86]`

So on the pinned v1.1.0 decoder, setting bit 6 on a non-audio-frame OBU shifts its entire payload by two bytes and corrupts the descriptor — a silent, catastrophic failure. This is a second, independent reason for CORRECTION 1 below.

---

## CORRECTIONS to project documents

These are the highest-value output of this research. Each contradicts something currently written down; each is verified from the pinned tree.

### CORRECTION 1 — OBU-03 mirrors a draft-v2.0.0 tree (**decision-changing, blocks plan 01-04**)

REQUIREMENTS.md OBU-03: *"Bit 6 modelled as a polymorphic enum carried by the payload variant, not a shared `bool` — trimming status for audio frames, inverted `is_not_key_frame` for temporal delimiter, reserved-SHALL-be-0 elsewhere."* PITFALLS.md §2 adds `optional_fields_flag` for Mix Presentation and gives a four-variant enum. PROJECT.md's Context repeats it.

At the pinned tag, the field is called `obu_trimming_status_flag` and is legal **only on audio frames**:
```cpp
// Returns `true` if this `ObuType` is allowed to have the
// `obu_trimming_status_flag` flag set. `false` otherwise.
bool IsTrimmingStatusFlagAllowed(ObuType type) {
  if (kObuIaAudioFrameId0 <= type && type <= kObuIaAudioFrameId17) {
    return true;
  }
  switch (type) {
    case kObuIaAudioFrame:
      return true;
    default:
      return false;
  }
}
```
…and `Validate()` turns a violation into an error:
```cpp
  if (header.obu_trimming_status_flag &&
      !IsTrimmingStatusFlagAllowed(header.obu_type)) {
    return absl::InvalidArgumentError(absl::StrCat(
        "The trimming status flag flag is not allowed to be set for "
        "obu_type= ", header.obu_type));
  }
```
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/obu_header.cc:60-70, 95-101]`

A repository-wide grep for `type_specific_flag|GetIsKeyFrame|GetOptionalFieldsFlag|is_not_key_frame|optional_fields_flag` over `iamf/obu/` at **v2.1.0 returns nothing**. The same grep at **`main`** returns `obu_header.cc:344 GetOptionalFieldsFlag`, `obu_header.cc:353 GetIsKeyFrame`, and `mix_presentation.cc:653` whose comment reads *"In IAMF v2.0.0, if `optional_fields_flag` is true, then we do not have…"*. `[VERIFIED-BY-EXECUTION: grep at both revisions, 2026-09-08]`

**The v1.1.0-correct model:**
```rust
/// ref: iamf/obu/obu_header.cc IsTrimmingStatusFlagAllowed (iamf-tools v2.1.0)
#[non_exhaustive]
pub enum TypeSpecific {
    /// Audio Frame OBUs only (types 5, 6..=23). Bit 6 = obu_trimming_status_flag.
    Trimming(Option<Trimming>),
    /// Every other OBU type. Reserved, SHALL be 0. Serialises as 0; no other constructor.
    Reserved,
}
```
`IsNotKeyFrame` and `OptionalFields` must **not** exist. Adding them under `#[non_exhaustive]` makes a state representable that `iamf-tools@v2.1.0` rejects on write and that `libiamf@v1.1.0` mis-frames on read.

`obu_redundant_copy`, by contrast, is exactly as documented: forbidden on types 3, 4, 5 and 6–23, allowed elsewhere. `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/obu_header.cc:43-57]` OBU-04 stands unchanged.

**Action:** amend OBU-03 in REQUIREMENTS.md, and annotate PITFALLS.md §2 with the provenance (it was read from `main`). Plan 01-04 must implement the two-variant enum.

### CORRECTION 2 — `test_000003.iamf`'s configuration IS published (**retires a D-17 waiver**)

`libiamf@v1.1.0` ships **221 `.iamf` files and 221 matching `.textproto` files** in `tests/`, plus 403 `.wav` files (inputs and per-mix rendered outputs). `[VERIFIED-BY-EXECUTION: ls | wc -l, 2026-09-08]`

`tests/test_000003.textproto` gives the exact configuration. The salient fields:

| Field | Value |
|---|---|
| `primary_profile` / `additional_profile` | `PROFILE_VERSION_SIMPLE` (0/0) |
| `codec_config_id` | `200` |
| `codec_id` | `CODEC_ID_LPCM` |
| `num_samples_per_frame` | `128` |
| `audio_roll_distance` | `0` |
| `sample_format_flags` | `LPCM_LITTLE_ENDIAN` |
| `sample_size` / `sample_rate` | `16` / `16000` |
| `audio_element_id` / `audio_element_type` | `300` / `AUDIO_ELEMENT_CHANNEL_BASED` |
| substreams | `num_substreams: 1`, `audio_substream_ids: [0]`, `num_parameters: 0` |
| layer config | `num_layers: 1`, `LOUDSPEAKER_LAYOUT_STEREO`, `substream_count: 1`, `coupled_substream_count: 1` |
| `mix_presentation_id` | `42`, `annotations_language: ["en-us"]`, `localized_presentation_annotations: ["test_mix_pres"]` |
| element/output mix gain | `parameter_id: 100`, `parameter_rate: 16000`, `param_definition_mode: 1`, `default_mix_gain: 0` |
| layout | `SOUND_SYSTEM_A_0_2_0`, `info_type_bit_masks: []`, `integrated_loudness: -13733`, `digital_peak: -12879` |
| audio | `sawtooth_100_stereo.wav`, `samples_to_trim_at_end: 64`, `samples_to_trim_at_start: 0` |
| temporal delimiters | `enable_temporal_delimiters: false` |

`[VERIFIED-FROM-CODE: libiamf@v1.1.0 tests/test_000003.textproto]`

**Note it is stereo, 16 kHz, 16-bit little-endian — not the D-18 fixture.** CONF-08 and CONF-02..05 are two different files. The plan must not conflate them.

**Recommendation:** update D-17's `CONFORMANCE-GATE.md` preamble. The pre-authorised waiver for CONF-08 is no longer needed; only CONF-06/07 retain a defensible waiver path (and DEC-04 has now removed most of that risk too).

### CORRECTION 3 — the fixture supply is far larger than "338 textprotos and exactly one `.iamf`"

PROJECT.md: *"`iamf-tools/iamf/cli/testdata/` holds 338 `.textproto` files and **one** `.iamf`; no release ships a test-vector bundle. M2's 'parse `iamf-tools`-produced files' therefore needs a one-time Bazel build."*

At the pinned tags:

| Source | `.textproto` | `.iamf` | rendered `.wav` |
|---|---|---|---|
| `iamf-tools@v2.1.0 iamf/cli/testdata/` | **226** | **5** (in `testdata/iamf/`) | 0 |
| `libiamf@v1.1.0 tests/` | **221** | **221** | 403 |

`[VERIFIED-BY-EXECUTION: ls counts, 2026-09-08]` (338 is `main`'s count, not v2.1.0's.)

**This substantially de-risks CONF-11 and Phase 2's PARSE-07.** 221 `iamf-tools`-produced `.iamf` files, each with its own configuration textproto **and** its rendered output WAV, are already committed to `libiamf@v1.1.0` under BSD-3-Clause-Clear. They can be consumed directly by the harness (or vendored under D-14's size cap with a `NOTICE` entry) with **no Bazel run at all**. The Bazel container is still wanted for CONF-07 (producing a companion file for *our own* configuration), but it stops being a blocking long-pole for fixture supply.

I validated the whole corpus mechanically:
```
files=226  obu_size fields=524674  non-minimal=0
```
Every one of the 226 files (221 from libiamf + 5 from iamf-tools) walks OBU-by-OBU and lands its final boundary **exactly** on `len()`, and every `obu_size` is minimal ULEB128. `[VERIFIED-BY-EXECUTION: python OBU walker, 2026-09-08]` That corpus is an excellent day-one regression suite for `find_obu_boundaries()` (OBU-08) that needs no reference binary — i.e. it satisfies CONF-10.

### CORRECTION 4 — the `LebGenerator` default is `kMinimum` and the reference corpus contains zero non-minimal encodings

```cpp
  static std::unique_ptr<LebGenerator> Create(
      GenerationMode generation_mode = GenerationMode::kMinimum,
      int8_t fixed_size = 0);
```
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/common/leb_generator.h:39-41]`

The CLI's generator comes from the proto, whose default is also minimum:
```proto
message Leb128Generator {
  Leb128GeneratorMode mode = 1 [default = GENERATE_LEB_MINIMUM];
  int32 fixed_size = 2 [default = 5];
}
```
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/proto/test_vector_metadata.proto:29-35]`

Exactly **one** of the 226 textprotos sets `GENERATE_LEB_FIXED_SIZE`: `test_000134.textproto`. `[VERIFIED-BY-EXECUTION: grep -l, 2026-09-08]`

**Consequence for D-03:** the `pub(crate)` fixed-size encoder is **not needed for CONF-08**. `test_000003.iamf` is minimal throughout, as is every other reference file. Keep the fixed-size path (it costs ~15 lines, it is needed for Phase 2's PARSE-04 foreign-file caveat, and `test_000134` exercises it), but the plan should not schedule it as a CONF-08 blocker, and D-03's stated justification should be corrected: it exists for *parsing* and for reproducing `test_000134`, not for reproducing `test_000003`.

### CORRECTION 5 — the descriptor prologue is 120 bytes, not 118

PROJECT.md says `test_000003.iamf` has "a 118-byte descriptor prologue". The first Audio Frame OBU begins at offset **`0x78` = 120**. `[VERIFIED-BY-EXECUTION: OBU walk — descriptors at 0, 8, 26, 40; first audio frame at 120]` The field-by-field decode below accounts for all 120 bytes.

### CORRECTION 6 — `iamf-tools@v2.1.0` has no `probe_main`

STACK.md §8 says `iamf-tools` "Produces `encoder_main`, `decoder_main`, `probe_main` CLIs". At v2.1.0 `iamf/cli/BUILD` declares exactly **two** `cc_binary` targets: `decoder_main` and `encoder_main`. `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/BUILD:724, 745]`

**Consequence for CONF-06** ("`iamf-tools`' own parser accepts the file"). There is no dedicated probe/validate binary. The options, in order of cost:
1. **Use `decoder_main`** — it goes through `ObuProcessor`/`DescriptorObuParser` and therefore exercises the strict parser. Flags: `--input_filename`, `--output_filename`, optional `--mix_id`, `--output_layout` (default `2.0`). `[CITED: iamf-tools@v2.1.0 docs/iamf_decoder_main.md]` A non-zero exit or an error on stderr is the CONF-06 signal.
2. Add a ~40-line `cc_binary` in the container build that calls `ObuProcessor` and returns its status. More precise, more machinery, and it means maintaining C++ in this repo.

Recommend (1) for Phase 1, and record in `CONFORMANCE-GATE.md` that CONF-06 is enforced *through* `decoder_main`'s parse path rather than by a dedicated validator.

### CORRECTION 7 — `iamf-tools@v2.1.0` ships one invalid `.iamf` fixture

`iamf/cli/testdata/iamf/tones_256samp_5p1_pcm.iamf` has `num_samples_per_frame = 0`:

```
codec config payload: 00 69 70 63 6d 00 00 00 01 10 00 00 bb 80   (14 bytes)
  codec_config_id       = 0x00      -> 0
  codec_id              = "ipcm"
  num_samples_per_frame = 0x00      -> 0     <-- invalid
  audio_roll_distance   = 00 00     -> 0
  sample_format_flags   = 0x01      -> little-endian
  sample_size           = 0x10      -> 16
  sample_rate           = 00 00 bb 80 -> 48000
```
`[VERIFIED-FROM-DATA: iamf-tools@v2.1.0 iamf/cli/testdata/iamf/tones_256samp_5p1_pcm.iamf, bytes 10..23]`

The other four files in that directory are fine (`nspf` = 960, 4608, 120, 960). `[VERIFIED-FROM-DATA]` `iamf-tools`' own `ValidateNumSamplesPerFrame` requires `[1, 96000]`, so its parser would reject its own fixture; `libiamf@main` rejects it; `libiamf@v1.1.0` accepts it and produces **zero samples** — which I reproduced:

```
===================== Get 0 frames
===================== Get 0 samples
```
`[VERIFIED-BY-EXECUTION: iamfdec -i0 -o3 out/x.wav -r 48000 -s1 -d 16 tones_256samp_5p1_pcm.iamf, 2026-09-08]`

**Do not use this file as a golden.** Its *structure* is still sound evidence for BCG packing (below), and it is a perfect negative fixture for the "clean decode, wrong result" failure class.

---

## The peak limiter — a constraint on the conformance fixture (**decision-changing**)

`IAMF_decoder_open()` creates a limiter unconditionally:
```c
    handle->ctx.threshold_db = LIMITER_MaximumTruePeak;
    ...
    handle->limiter = audio_effect_peak_limiter_create();
```
and it is initialised at configure time with:
```c
      audio_effect_peak_limiter_init(
          handle->limiter, handle->ctx.threshold_db, OUTPUT_SAMPLERATE,
          iamf_layout_channels_count(&handle->ctx.output_layout->layout),
          LIMITER_AttackSec, LIMITER_ReleaseSec, LIMITER_LookAhead);
```
with constants
```c
#define LIMITER_MaximumTruePeak -1.0f
#define LIMITER_AttackSec 0.001f
#define LIMITER_ReleaseSec 0.200f
#define LIMITER_LookAhead 240
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:4153-4159, 4231-4236; code/src/common/audio_defines.h:23-26]`

`compute_target_gain()` returns exactly `1.0` while no look-ahead peak exceeds `linearThreashold`; once `peak * currentGain > linearThreashold` it enters a 1 ms attack ramp to `linearThreashold / peak` followed by a 200 ms release back to 1.0. `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/audio_effect_peak_limiter.c, compute_target_gain]`

`−1.0 dBTP` ⇒ linear threshold ≈ **0.8913**.

**Empirically, below threshold the path is bit-exact and the sample count is preserved:**
```
out/t3_lim.wav    ch 2 frames 8000   differing samples: 0 of 16000
out/t3_nolim.wav  ch 2 frames 8000   differing samples: 0 of 16000
  first 8 out: (-100, 100, -99, 99, -98, 98, -97, 97)
  src:         (-100, 100, -99, 99, -98, 98, -97, 97)
```
`[VERIFIED-BY-EXECUTION: iamfdec on test_000003.iamf vs sawtooth_100_stereo.wav, 2026-09-08]` — peak there is 100/32768 ≈ 0.003 FS.

**Guidance for plan 01-08 / D-18:**
1. **Cap the fixture's per-channel index pattern at ≤ −6 dBFS** (|sample| ≤ 0.5 × full scale). A pattern that walks toward full scale — the obvious way to write "a per-channel deterministic index pattern" for 24-bit — will cross −1 dBFS and the limiter will engage.
2. **Pass `-disable_limiter` in the harness.** `iamfdec` at v1.1.0 supports it (`-disable_limiter : Disable peak limiter.`) and `IAMF_decoder_peak_limiter_enable(dec, 0)` *destroys* the limiter, removing the 240-sample look-ahead path entirely. `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/test/tools/iamfdec/src/test_iamfdec.c:110, 378; code/src/iamf_dec/IAMF_decoder.c:4457-4469]`
3. **Document in the harness *why* both are done**, or a later maintainer will raise the amplitude "to make the signal more distinguishable" and reintroduce the failure.
4. **`-p [dB]` is an alternative** (`IAMF_decoder_peak_limiter_set_threshold`): `-p 0` makes the linear threshold 1.0, unreachable from integer PCM. Prefer `-disable_limiter`.

### Other `iamfdec` harness gotchas, all verified by execution

| Gotcha | Detail |
|---|---|
| `-o3` takes a **file** path, not a directory | Passing `out/` prints `out/ can't opened.` and writes nothing, **with exit code 0**. The harness must assert the output file exists and is non-empty, not just check the exit status. `[VERIFIED-BY-EXECUTION]` |
| `-r` defaults to **48000** and drives a resampler | `IAMF_decoder_set_sampling_rate` accepts `{8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000, 64000, 88200, 96000}`. Decoding a 16 kHz file without `-r 16000` silently resamples. D-18's fixture is 48 kHz so the default is correct there, but CONF-08's `test_000003` reproduction needs `-r 16000`. `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:4483-4498]` `[VERIFIED-BY-EXECUTION]` |
| `-s` selects the output layout | `-s0` = Sound System A (0+2+0); **`-s1` = Sound System B (0+5+0) = 5.1**; `-s12` = mono; `-sb` = binaural. `[VERIFIED-FROM-CODE: code/test/tools/iamfdec/src/test_iamfdec.c:87-103]` |
| `-d [bit]` sets output WAV bit depth | Set it explicitly to the fixture's `sample_size` so the comparison is like-for-like. |
| A "successful" run can still yield zero samples | See CORRECTION 7. **CONF-05's decoded-sample-count assertion is what catches this** — it is not redundant with PCM equality. |

---

## BCG channel→substream packing for 5.1 — resolved three independent ways

PROJECT.md names this the highest silent-failure risk in the phase. It is now closed.

### (a) The rule, quoted from `libiamf`'s own comment

```
   * In ChannelGroup for Channel audio: The order conforms to following rules:
   *
   * @ Coupled Substream(s) comes first and followed by non-coupled
   * Substream(s).
   * @ Coupled Substream(s) for surround channels comes first and followed by
   * one(s) for top channels.
   * @ Coupled Substream(s) for front channels comes first and followed by
   * one(s) for side, rear and back channels.
   * @ Coupled Substream(s) for side channels comes first and followed by one(s)
   * for rear channels.
   * @ Center channel comes first and followed by LFE and followed by the other
   * one.
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:365-378]`

### (b) The 5.1 answer, computed from the layout table

```c
    {
        .id = IAMF_LAYOUT_ID_5_1,
        .name = "5.1",
        .channels = 6,
        .height = 0,
        .surround = 5,
        .lfe1 = 3,
        .sound_system = SOUND_SYSTEM_B,
        .type = IA_CHANNEL_LAYOUT_510,
        .rendering_id_in = IAMF_51,
        .rendering_id_out = BS2051_B,
        .decoding_map = {0, 1, 4, 5, 2, 3},
        .channel_layout = {IA_CH_L5, IA_CH_R5, IA_CH_C, IA_CH_LFE, IA_CH_SL5,
                           IA_CH_SR5},
    },
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_layout.c:57-74]`

and the function that consumes `decoding_map`:
```c
int iamf_audio_layer_layout_get_decoding_channels(IAChannelLayoutType type,
                                                  IAChannel *channels,
                                                  uint32_t count) {
  const IAMF_LayoutInfo *info = iamf_audio_layer_get_layout_info(type);
  for (uint32_t i = 0; i < info->channels; ++i)
    channels[i] = info->channel_layout[info->decoding_map[i]];
  return info->channels;
}
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_layout.c:432-438]`

Applying it: `channels = [L, R, Ls, Rs, C, LFE]`. The inverse scatter confirms it:
```c
  for (int i = 0; i < info->channels; i++)
    memcpy(&out[info->decoding_map[i] * size], &in[i * size],
           size * sizeof(float));
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_decoder.c:439-446]` — mapping decoding order `[L,R,Ls,Rs,C,LFE]` to presentation order `[L,R,C,LFE,Ls,Rs]`.

### (c) The same answer read out of a shipped 5.1 PCM file

```
@     0 type=31 SeqHeader        size=6
@     8 type= 0 CodecConfig      size=14
@    24 type= 1 AudioElement     size=13
@    39 type= 2 MixPresentation  size=63
@   104 type= 6 AudioFrame       size=1024    <- 256 samples x 2 ch x 2 B  => COUPLED
@  1131 type= 7 AudioFrame       size=1024    <- COUPLED
@  2158 type= 8 AudioFrame       size=512     <- 256 samples x 1 ch x 2 B  => MONO
@  2673 type= 9 AudioFrame       size=512     <- MONO
total 3188
```
and its Audio Element payload:
```
AE payload: 01 00 00 04 00 01 02 03 00 20 20 04 02
  audio_element_id          = 0x01 -> 1
  audio_element_type(3)/reserved(5) = 0x00 -> channel-based
  codec_config_id           = 0x00 -> 0
  num_substreams            = 0x04 -> 4
  audio_substream_ids       = 00 01 02 03
  num_parameters            = 0x00 -> 0
  num_layers(3)/reserved(5) = 0x20 -> 1 layer
  layer cfg byte            = 0x20 -> loudspeaker_layout=2 (5.1), output_gain=0, recon_gain=0, reserved_a=0
  substream_count           = 0x04 -> 4
  coupled_substream_count   = 0x02 -> 2
```
`[VERIFIED-FROM-DATA: iamf-tools@v2.1.0 iamf/cli/testdata/iamf/tones_256samp_5p1_pcm.iamf]`

### The resulting specification for plan 01-06

For a single-layer channel-based 5.1 element:

| Substream | OBU type | Contents | Frame payload layout |
|---|---|---|---|
| 0 | 6 (`kObuIaAudioFrameId0`) | **coupled (L, R)** | interleaved: `L₀ R₀ L₁ R₁ …` |
| 1 | 7 | **coupled (Ls, Rs)** | interleaved: `Ls₀ Rs₀ …` |
| 2 | 8 | **mono C** | `C₀ C₁ …` |
| 3 | 9 | **mono LFE** | `LFE₀ LFE₁ …` |

`substream_count = 4`, `coupled_substream_count = 2`, `loudspeaker_layout = 2`.

The interleaving inside a coupled substream is explicit in the decoder:
```c
  for (; c < ths->coupled_streams; ++c) {
    for (int s = 0; s < samples; ++s) {
      for (int lf = 0; lf < 2; ++lf) {
        fpcm[samples * (c * 2 + lf) + s] =
            ctx->func(buf[c], (s * 2 + lf) * sample_size_bytes) / ctx->scale_i2f;
      }
    }
  }
  cc = ths->coupled_streams;
  for (; c < ths->streams; ++c) {
    for (int s = 0; s < samples; ++s) {
      fpcm[samples * (cc + c) + s] =
          ctx->func(buf[c], s * sample_size_bytes) / ctx->scale_i2f;
    }
  }
```
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/pcm/IAMF_pcm_decoder.c:113-131]`

---

## The decoder's 5.1 output channel order (D-19)

**`L, R, C, LFE, Ls, Rs`** — for Sound System B (0+5+0), per ITU-R BS.2051-3.

Two independent confirmations:

1. **From `libiamf` code:** `.channel_layout = {IA_CH_L5, IA_CH_R5, IA_CH_C, IA_CH_LFE, IA_CH_SL5, IA_CH_SR5}` `[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/IAMF_layout.c:72-73]`
2. **From `iamf-tools`' own documentation table:** `Sound System B (0+5+0) | ITU-2051-3 | L, R, C, LFE, Ls, Rs` `[CITED: iamf-tools@v2.1.0 iamf/cli/testdata/README.md, "Output WAV files"]`

The full table, for later phases (cite it in the harness rather than re-deriving):

| Layout | Channel order |
|---|---|
| Sound System A (0+2+0) | L, R |
| Sound System B (0+5+0) | **L, R, C, LFE, Ls, Rs** |
| Sound System C (2+5+0) | L, R, C, LFE, Ls, Rs, Ltf, Rtf |
| Sound System D (4+5+0) | L, R, C, LFE, Ls, Rs, Ltf, Rtf, Ltr, Rtr |
| Sound System I (0+7+0) | L, R, C, LFE, Lss, Rss, Lrs, Rrs |
| Sound System J (4+7+0) | L, R, C, LFE, Lss, Rss, Lrs, Rrs, Ltf, Rtf, Ltb, Rtb |

`[CITED: iamf-tools@v2.1.0 iamf/cli/testdata/README.md]`

**D-18's "the decoder's output *is* our input" premise holds, and here is the proof.** The 5.1→Sound System B render matrix in `libiamf` is the 6×6 identity:
```c
float iamf51_bs050[][6] = {{1.0, 0, 0, 0, 0, 0}, {0, 1.0, 0, 0, 0, 0},
                           {0, 0, 1.0, 0, 0, 0}, {0, 0, 0, 1.0, 0, 0},
                           {0, 0, 0, 0, 1.0, 0}, {0, 0, 0, 0, 0, 1.0}};
```
with the table entry `{IAMF_51, BS2051_B, (float *)iamf51_bs050, 6, 6},`.
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/m2m_rdr.c:106-108, 1222]`

Multiplication by exactly `1.0f` and summation of one term are IEEE-754-exact, so the only remaining transforms are int→float (`/ 2^23` for 24-bit) and float→int on the WAV write. Both are exact for the round trip provided the limiter gain is 1.0 — which is the §Peak Limiter constraint, and which the `test_000003` run demonstrated empirically at 16-bit.

**Residual risk for 24-bit that plan 01-08 should confront early:** the `test_000003` empirical proof was 16-bit. `iamf_decoder_plane2stride_out(pcm, f->data, real_frame_size, channels, ctx->bit_depth)` performs the float→int conversion; I did not read its rounding rule this session. Run the 24-bit round trip as the *first* thing 01-08 does, before the fixture design is frozen. If 24-bit float→int is not exact, D-18's "24-bit big-endian" choice needs a `CONFORMANCE-GATE.md` note, not a silent 16-bit downgrade — the DESC-03 endianness coverage is why 24-bit was chosen. `[ASSUMED: that 24-bit is exact — not verified]`

---

## Verified byte layouts (`test_000003.iamf`, hand-decoded)

D-25 requires hand-decoded vectors as the primary source. This is that work, done once, and it is directly reusable as plan 01-04/01-05 test data.

```
00000000: f806 6961 6d66 0000 000e ... (see below)
```

| Offset | Bytes | Field | Value |
|---|---|---|---|
| `0x00` | `f8` | `obu_type(5)=11111`=31, `redundant(1)=0`, `trimming(1)=0`, `ext(1)=0` | IA Sequence Header |
| `0x01` | `06` | `obu_size` uleb128 | 6 |
| `0x02` | `69 61 6d 66` | `ia_code` | `"iamf"` = `0x69616D66` |
| `0x06` | `00` | `primary_profile` | 0 (Simple) |
| `0x07` | `00` | `additional_profile` | 0 (Simple) |
| `0x08` | `00` | header byte | Codec Config (type 0), all flags 0 |
| `0x09` | `10` | `obu_size` | 16 |
| `0x0A` | `c8 01` | `codec_config_id` uleb128 | 200 |
| `0x0C` | `69 70 63 6d` | `codec_id` | `"ipcm"` |
| `0x10` | `80 01` | `num_samples_per_frame` uleb128 | 128 |
| `0x12` | `00 00` | `audio_roll_distance` i16 BE | 0 |
| `0x14` | `01` | `sample_format_flags` | 1 = little-endian |
| `0x15` | `10` | `sample_size` | 16 |
| `0x16` | `00 00 3e 80` | `sample_rate` u32 BE | 16000 |
| `0x1A` | `08` | header byte | Audio Element (type 1) |
| `0x1B` | `0c` | `obu_size` | 12 |
| `0x1C` | `ac 02` | `audio_element_id` uleb128 | 300 |
| `0x1E` | `00` | `audio_element_type(3)` / `reserved(5)` | 0 = channel-based |
| `0x1F` | `c8 01` | `codec_config_id` uleb128 | 200 |
| `0x21` | `01` | `num_substreams` uleb128 | 1 |
| `0x22` | `00` | `audio_substream_ids[0]` uleb128 | 0 |
| `0x23` | `00` | `num_parameters` uleb128 | 0 |
| `0x24` | `20` | `num_layers(3)=001` / `reserved(5)=0` | 1 layer |
| `0x25` | `10` | `loudspeaker_layout(4)=0001` / `output_gain_is_present(1)=0` / `recon_gain_is_present(1)=0` / `reserved_a(2)=0` | Stereo |
| `0x26` | `01` | `substream_count` | 1 |
| `0x27` | `01` | `coupled_substream_count` | 1 |
| `0x28` | `10` | header byte | Mix Presentation (type 2), **bit 6 = 0** |
| `0x29` | `4e` | `obu_size` | 78 |
| `0x2A` | `2a` | `mix_presentation_id` uleb128 | 42 |
| `0x2B` | `01` | `count_label` uleb128 | 1 |
| `0x2C` | `65 6e 2d 75 73 00` | `annotations_language[0]` | `"en-us\0"` |
| `0x32` | `74 … 00` | `localized_presentation_annotations[0]` | `"test_mix_pres\0"` (14 B) |
| `0x40` | `01` | `num_sub_mixes` uleb128 | 1 |
| `0x41` | `01` | `num_audio_elements` uleb128 | 1 |
| `0x42` | `ac 02` | `audio_element_id` uleb128 | 300 |
| `0x44` | `74 … 00` | `localized_element_annotations[0]` | `"test_sub_mix_0_audio_element_0\0"` (31 B) |
| `0x63` | `00` | `headphones_rendering_mode(2)=00` / `reserved(6)=0` | STEREO |
| `0x64` | `00` | `rendering_config_extension_size` uleb128 | 0 |
| `0x65` | `64` | element mix gain `parameter_id` uleb128 | 100 |
| `0x66` | `80 7d` | `parameter_rate` uleb128 | 16000 |
| `0x68` | `80` | `param_definition_mode(1)=1` / `reserved(7)=0` | mode 1 ⇒ no duration/subblock fields |
| `0x69` | `00 00` | `default_mix_gain` i16 BE | 0 |
| `0x6B` | `64` | output mix gain `parameter_id` | 100 |
| `0x6C` | `80 7d` | `parameter_rate` | 16000 |
| `0x6E` | `80` | `param_definition_mode` | 1 |
| `0x6F` | `00 00` | `default_mix_gain` | 0 |
| `0x71` | `01` | `num_layouts` uleb128 | 1 |
| `0x72` | `80` | `layout_type(2)=10`=2 SS-convention / `sound_system(4)=0000` / `reserved(2)=00` | Sound System A (0+2+0) |
| `0x73` | `00` | `info_type` | 0 (no true peak, no anchored, no extension) |
| `0x74` | `ca 5b` | `integrated_loudness` i16 BE | −13733 |
| `0x76` | `cd b1` | `digital_peak` i16 BE | −12879 |
| `0x78` | `30` | header byte | Audio Frame ID0 (type 6), trimming=0 |
| `0x79` | `80 04` | `obu_size` uleb128 | 512 = 128 samples × 2 ch × 2 B |
| … | | 62 more untrimmed frames | |
| `0x7D32` | `32` | header byte | type 6, **trimming flag = 1** |
| `0x7D33` | `82 04` | `obu_size` | 514 |
| `0x7D35` | `40` | `num_samples_to_trim_at_end` uleb128 | **64 — written FIRST** |
| `0x7D36` | `00` | `num_samples_to_trim_at_start` uleb128 | 0 |
| `0x7D37` | 512 B | payload | |
| | | **file length** | 32567 = `0x7F37`; 67 OBUs; final boundary lands exactly on `len()` |

`[VERIFIED-FROM-DATA: libiamf@v1.1.0 tests/test_000003.iamf, hand-decoded and cross-checked against tests/test_000003.textproto, 2026-09-08]`

Every value above matches the textproto. This is the two-independent-derivations agreement D-25 asks for, already achieved for the descriptor set.

**Load-bearing confirmations from this decode:**
- `obu_size` at `0x7D33` is **514 = 2 trim bytes + 512 payload** ⇒ `obu_size` includes the trim fields and excludes byte 0 and the size bytes. (Pitfall 1.)
- Trim order is **END then START**. (OBU-05.)
- After the IA Sequence Header the writer is at byte offset **8** — the absolute-offset assertion PITFALLS.md §8 asks for.
- `param_definition_mode = 1` ⇒ `default_mix_gain` follows immediately with no `duration`/`constant_subblock_duration`/`num_subblocks`. (DESC-06.)
- `sound_system` is 4 bits inside a 1-byte field with `layout_type` in the top 2 bits and 2 reserved bits. (DESC-07.)
- Zero Parameter Block OBUs despite two mandatory Mix Gain param definitions. (DESC-06.)

### The `obu_size` origin, in the reference's own words

```cpp
// `obu_size` represents the size of all fields after itself and
// serialized_size. Sum them to get `obu_size`, while ensuring they fit into ...
```
and the write path:
```cpp
absl::Status ObuHeader::ValidateAndWrite(int64_t payload_serialized_size,
                                         WriteBitBuffer& wb) const {
  DecodedUleb128 obu_size;
  RETURN_IF_NOT_OK(GetObuSizeAndValidate(wb.leb_generator_, *this,
                                         payload_serialized_size, obu_size));
  RETURN_IF_NOT_OK(wb.WriteUnsignedLiteral(obu_type, 5));
  RETURN_IF_NOT_OK(wb.WriteBoolean(obu_redundant_copy));
  RETURN_IF_NOT_OK(wb.WriteBoolean(obu_trimming_status_flag));
  RETURN_IF_NOT_OK(wb.WriteBoolean(obu_extension_flag));
  RETURN_IF_NOT_OK(wb.WriteUleb128(obu_size));
  RETURN_IF_NOT_OK(WriteFieldsAfterObuSize(*this, wb));
  return absl::OkStatus();
}
```
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/obu_header.cc:196-197, 278-294]`

`WriteFieldsAfterObuSize` writes, in order: `num_samples_to_trim_at_end`, `num_samples_to_trim_at_start` (both only if the trimming flag is set), then `extension_header_size` + `extension_header_bytes` (only if the extension flag is set). `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/obu_header.cc:106-124]`

Caps:
```cpp
inline constexpr int kMaxLeb128Size = 8;
constexpr uint32_t kEntireObuSizeMaxTwoMegabytes = (1 << 21);
```
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/types.h:21, 32]` and the derived bound `max_obu_size = kEntireObuSizeMaxTwoMegabytes - 1 - size_of_obu_size` `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/obu_header.cc:141-142]`.

`GetObuSizeAndValidate` also asserts byte-alignment of the after-size fields before summing (`if (!temp_wb_after_obu_size.IsByteAligned() || …)`) `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/obu_header.cc:184-190]` — BITS-05's precedent.

---

## Enumerations to model, quoted verbatim

Plan 01-05 needs these as Rust enums (Pitfall 6: never a bare integer).

**`LoudspeakerLayout` (4 bits)** — `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/audio_element.h:76-93]`
```cpp
  enum LoudspeakerLayout : uint8_t {
    kLayoutMono = 0,      // C.
    kLayoutStereo = 1,    // L/R
    kLayout5_1_ch = 2,    // L/C/R/Ls/Rs/LFE.
    kLayout5_1_2_ch = 3,  // L/C/R/Ls/Rs/Ltf/Rtf/LFE.
    kLayout5_1_4_ch = 4,  // L/C/R/Ls/Rs/Ltf/Rtf/Ltr/Rtr/LFE.
    kLayout7_1_ch = 5,    // L/C/R/Lss/Rss/Lrs/Rrs/LFE.
    kLayout7_1_2_ch = 6,  // L/C/R/Lss/Rss/Lrs/Rrs/Ltf/Rtf/LFE.
    kLayout7_1_4_ch = 7,  // L/C/R/Lss/Rss/Lrs/Rrs/Ltf/Rtf/Ltb/Rtb/LFE.
    kLayout3_1_2_ch = 8,  // L/C/R//Ltf/Rtf/LFE.
    kLayoutBinaural = 9,  // L/R.
    kLayoutReserved10 = 10,
    kLayoutReserved11 = 11,
    kLayoutReserved12 = 12,
    kLayoutReserved13 = 13,
    kLayoutReserved14 = 14,
    kLayoutExpanded = 15,
  };
```

**`ExpandedLoudspeakerLayout` (u8, present only when layout == 15)** — `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/audio_element.h:96-114]`
```cpp
    kExpandedLayoutLFE = 0,        kExpandedLayoutStereoS = 1,
    kExpandedLayoutStereoSS = 2,   kExpandedLayoutStereoRS = 3,
    kExpandedLayoutStereoTF = 4,   kExpandedLayoutStereoTB = 5,
    kExpandedLayoutTop4Ch = 6,     kExpandedLayout3_0_ch = 7,
    kExpandedLayout9_1_6_ch = 8,   kExpandedLayoutStereoF = 9,
    kExpandedLayoutStereoSi = 10,  kExpandedLayoutStereoTpSi = 11,
    kExpandedLayoutTop6Ch = 12,    kExpandedLayoutReserved13 = 13,
    kExpandedLayoutReserved255 = 255,
```

**`SoundSystem` (4 bits, in the Mix Presentation)** — `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/mix_presentation.h:155-172]`
```cpp
    kSoundSystemA_0_2_0 = 0,   kSoundSystemB_0_5_0 = 1,
    kSoundSystemC_2_5_0 = 2,   kSoundSystemD_4_5_0 = 3,
    kSoundSystemE_4_5_1 = 4,   kSoundSystemF_3_7_0 = 5,
    kSoundSystemG_4_9_0 = 6,   kSoundSystemH_9_10_3 = 7,
    kSoundSystemI_0_7_0 = 8,   kSoundSystemJ_4_7_0 = 9,
    kSoundSystem10_2_7_0 = 10, kSoundSystem11_2_3_0 = 11,
    kSoundSystem12_0_1_0 = 12, kSoundSystem13_6_9_0 = 13,
    kSoundSystemBeginReserved = 14, kSoundSystemEndReserved = 15,
```

DESC-07's four distinct types are confirmed correct: `LoudspeakerLayout` (4 bits, Audio Element), `ExpandedLoudspeakerLayout` (u8, Audio Element, gated on `== 15`), `AmbisonicsConfig` (Audio Element), `SoundSystem` (4 bits, Mix Presentation). They live in **two different OBUs** and must not be one enum.

**Profile limits** — `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc:34-40]`
```cpp
constexpr int kSimpleProfileMaxAudioElements = 1;
constexpr int kBaseProfileMaxAudioElements = 2;
constexpr int kBaseEnhancedProfileMaxAudioElements = 28;
constexpr int kSimpleProfileMaxChannels = 16;
constexpr int kBaseProfileMaxChannels = 18;
constexpr int kBaseEnhancedProfileMaxChannels = 28;
```
Both counts are **per Mix Presentation**. For PROF-02: a single 5.1 (6-channel) element ⇒ minimum profile is **Simple (0)**.

**`MixGainParameterData` animation types** (answers CONTEXT.md deferred item 6, and it is present in v1.1.0) — `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/mix_gain_parameter_data.h:26-114]`
```cpp
  enum AnimationType : DecodedUleb128 {
    kAnimateStep = 0,
    kAnimateLinear = 1,
    kAnimateBezier = 2,
  };
struct AnimationStepInt16   { int16_t start_point_value; };
struct AnimationLinearInt16 { int16_t start_point_value; int16_t end_point_value; };
struct AnimationBezierInt16 { int16_t start_point_value; int16_t end_point_value;
                              int16_t control_point_value;
                              uint8_t control_point_relative_time;  // Q0.8 format. };
```
**Yes — IAMF v1.1.0 carries a per-subblock animation type.** DEC-03 costs less than PROJECT.md records: a ramp is endpoints plus a type, not N samples. This is Phase 4 material (the `(value, animation_type)` slice element question). Recorded, not acted on.

---

## Descriptor write order and the mandatory stereo layout

**Write order** — `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc:262-310]`
1. IA Sequence Header
2. Codec Config OBUs, **ascending by `codec_config_id`** (`SortedKeys(codec_config_obus, std::less<uint32_t>())`)
3. Audio Element OBUs, **ascending by `audio_element_id`**
4. Mix Presentation OBUs, **in original list order** (the source comment: *"Because the original ordering may be used downstream when selecting the mix presentation"*)

DESC-09's `Vec`-in-bitstream-order + `by_id()` is the right model: sorting Mix Presentations by ID would be *wrong*.

**Mandatory stereo layout (DESC-05)** — enforced on write, and the error string is exact:
```cpp
  bool found_stereo_layout = false;
  for (const auto& layout : sub_mix.layouts) {
    RETURN_IF_NOT_OK(ValidateAndWriteLayout(layout, found_stereo_layout, wb));
  }
  if (!found_stereo_layout) {
    return absl::InvalidArgumentError(
        "Every sub-mix must have a stereo layout.");
  }
```
with `found_stereo_layout` set only by `if (sound_system == kSoundSystemA_0_2_0)`.
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:183-191, 388-396]`

**Read/write asymmetry worth knowing** — the reference does **not** check it on read:
```cpp
// TODO(b/339855338): Set `found_stereo_layout` to true if it is a stereo layout
// and check that its been found in MixPresentationSubMix::Read.
```
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:399-400]` Our `validate()` (D-09) should report it as a Finding; the *parser* must not reject it, or we are stricter than the reference on read.

**Consequence for D-18's 5.1 fixture:** the sub-mix needs **two** layouts — Sound System B (5.1, the one we compare against) *and* Sound System A (0+2+0, mandatory). Two `Loudness` blocks are therefore required, both with caller-supplied numbers. Plan 01-08's fixture design must include this; it is easy to miss because `test_000003` is stereo and its single mandatory layout doubles as its target.

---

## D-18's unreferenced Codec Config — empirical finding and recommendation

**Parser side: legal.** `DescriptorObuParser` inserts each Codec Config into a map keyed by `codec_config_id`; Audio Elements look their ID up. Nothing requires every Codec Config be referenced. `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/descriptor_obu_parser.cc:46-68]` `WriteDescriptorObus` writes every entry in the map unconditionally. `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc:276-284]`

**But there is no precedent, and there are hard constraints.** Across all 226 reference `.iamf` files:
```
files with >1 CodecConfig or >1 AudioElement: 77
of which have an UNREFERENCED codec config:    0
```
`[VERIFIED-BY-EXECUTION: reference-corpus scan, 2026-09-08]`

Exactly one file has two Codec Configs — `test_000119.iamf` — and its second one is injected as an **`ArbitraryObu`** (raw hex bytes in the textproto, with an imaginary `codec_id: "fake"`), referenced by an equally arbitrary second Audio Element. `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/testdata/test_000119.textproto]`

**Hard constraints on any two-Codec-Config configuration** (both are `encoder_main`-fatal):
- Different sample rate or bit depth ⇒ `absl::UnimplementedError("Codec Config OBUs with different bit-depths and/or sample rates are not in base-enhanced/base/simple profile; they are not allowed in ISOBMFF.")` `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/obu_sequencer_base.cc:187-195]`
- Different `num_samples_per_frame` ⇒ `absl::UnknownError("The encoder does not support Codec Config OBUs with a different number of samples per frame yet.")` `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/cli_util.cc:224-243]`

### Recommendation for D-18

1. **Make the two Codec Configs identical in every field except `codec_config_id`.** LPCM has no free field once rate/size/format are fixed, and the constraints above forbid varying the ones that exist. Two IDs (say `200` and `201`) with the element referencing `200` still makes DESC-08's ascending-ID write order observable in our own output.
2. **Plan 01-02 must settle this empirically, not by inference.** Add a task: in the pinned container, copy `iamf/cli/testdata/test_000003.textproto`, add a second `codec_config_metadata` block that differs only by `codec_config_id`, run `encoder_main`, and record whether it succeeds. This is a 10-minute experiment that either confirms the design or triggers the fallback. Record the result in `CONFORMANCE-GATE.md`.
3. **Fallback if `encoder_main` refuses**, in order of preference:
   - **(a)** Use `arbitrary_obu_metadata` with `INSERTION_HOOK_AFTER_CODEC_CONFIGS` to inject the second Codec Config as raw bytes — the mechanism `test_000119` uses, so it is proven. **Caveat:** that hook forces the arbitrary OBU *after* all normal ones, so ordering becomes positional rather than ID-sorted, and DESC-08's ascending-ID property is no longer what is being observed. Note this in the ledger. `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/proto/arbitrary_obu.proto:64]`
   - **(b)** Satisfy CONF-04 with **two Audio Elements** and accept that the decoded PCM is a mix — then assert only the *structure* (OBU count, ordering, boundary walk) on that file, and keep sample-identity on the single-element 5.1 file. Two fixtures, two properties, no rendering on our side. 77 reference files have ≥2 Audio Elements, so this route is well-precedented; note that two elements pushes the minimum profile from Simple to Base.

---

## Standard Stack

Phase 1's dependency graph is deliberately tiny. D-01 demotes `bitstream-io`; D-22 forbids features.

### Core (shipping)

| Crate | Version | Licence | Purpose | Why standard |
|---|---|---|---|---|
| `thiserror` | **2.0.20** (published 2026-08-08) | `MIT OR Apache-2.0` | Typed library errors (D-08, GUARD-13) | PROJECT.md constraint (never `anyhow` in a library). 1 424 796 704 downloads; `dtolnay`; MSRV 1.71, edition 2021. Its `thiserror-impl → syn → proc-macro2 → quote → unicode-ident` chain is proc-macro-only, which D-02 exempts. |

`[VERIFIED: crates.io registry API, 2026-09-08]` — but see the provenance rule: `thiserror` was named by PROJECT.md's own constraint, not discovered from a slopsquat-prone search, and its repository is `https://github.com/dtolnay/thiserror`.

**That is the entire shipping graph for Phase 1.** No `bitstream-io`, no `libm`, no codec crates.

### Supporting (dev-dependencies only)

| Crate | Version | Licence | Purpose | When |
|---|---|---|---|---|
| `bitstream-io` | **4.10.0** (2026-04-14) | `MIT/Apache-2.0` (slash form; cargo-deny 0.20.2 resolves as `OR`) | **Differential oracle** for D-01's hand-rolled `BitCursor`/`BitWriter` in a proptest | 01-03. MSRV 1.83, edition 2018, 50 657 892 downloads, repo `github.com/tuffy/bitstream-io`. |
| `hex-literal` | **1.1.0** (2025-10-29) | `MIT OR Apache-2.0` | `hex!("f8 06 69 61 6d 66 …")` in BITS-06 / D-25 hand-computed vectors | 01-03 onward. MSRV **1.85**, edition **2024** — matches GUARD-12 exactly. RustCrypto. |
| `pretty_assertions` | **1.4.1** (2024-09-15) | `MIT OR Apache-2.0` | Readable diffs on a failed byte-identity assertion | Optional, 01-08. A failing `assert_eq!` on a 32 KB buffer is unreadable without it. |

`[VERIFIED: crates.io registry API, 2026-09-08]`

**Deferred to Phase 2, not Phase 1:** `proptest` 1.11.0 (`MIT OR Apache-2.0`, MSRV **1.85**) and `arbitrary` 1.4.2. D-01's differential-oracle proptest is the one exception that pulls `proptest` into 01-03 — plan 01-03 must decide whether to land the oracle now (adds `proptest`) or defer the oracle to Phase 2 and rely on BITS-06's hand-computed vectors in Phase 1. **Recommendation: land it in 01-03.** D-01's entire justification for hand-rolling rests on the oracle existing; deferring it means the hand-rolled cursor ships unchecked through the phase that matters most. `proptest`'s MSRV of 1.85 is exactly GUARD-12's floor, so it costs nothing.

**Installation**
```toml
[dependencies]
thiserror = "2.0.20"          # MIT OR Apache-2.0

[dev-dependencies]
bitstream-io = "4.10.0"       # MIT/Apache-2.0 — differential oracle only (D-01)
hex-literal = "1.1.0"         # MIT OR Apache-2.0 — hand-computed vectors (BITS-06)
proptest = "1.11.0"           # MIT OR Apache-2.0 — differential oracle (D-01); sets MSRV 1.85
pretty_assertions = "1.4.1"   # MIT OR Apache-2.0 — readable byte diffs
```

### Alternatives considered

| Instead of | Could use | Tradeoff |
|---|---|---|
| Hand-rolled `BitCursor` (D-01) | `bitstream-io` as a real dependency | STACK.md's original recommendation. Overridden by D-01: the `io::Error` boundary and the loss of native byte offsets for `Error.at` cost more than `BitsWritten` saves. `BitsWritten` is also unnecessary — the reference itself does the two-pass `WriteBitBuffer` dance (`GetObuSizeAndValidate`), which is ~10 lines over a `Vec<u8>`. |
| `pretty_assertions` | `similar-asserts 2.0.0` (Apache-2.0) | Unified diff instead of side-by-side. Either is fine; pick one and note the licence on the line that adds it. |
| Hand-written uleb128 | `leb128 0.2.7` | Rejected in STACK.md and still correct, though the reason narrows: minimal-only is now known to be sufficient for CONF-08 (CORRECTION 4), so the argument is the 8-byte / `u32::MAX` caps and typed errors, not fixed-size mode. |

---

## Package Legitimacy Audit

Rust/crates.io. Each package was confirmed against the crates.io registry API this session, and each was *named by an existing project document* (PROJECT.md, STACK.md or CONTEXT.md) rather than discovered by web search — so the slopsquat vector is not in play, but the version and licence needed re-verification.

| Package | Registry | First published | Downloads | Source repo | Verdict | Disposition |
|---|---|---|---|---|---|---|
| `thiserror` 2.0.20 | crates.io | 2019-10-09 | 1 424 796 704 | github.com/dtolnay/thiserror | OK | Approved (shipping) |
| `bitstream-io` 4.10.0 | crates.io | 2017-02-17 | 50 657 892 | github.com/tuffy/bitstream-io | OK | Approved (dev-dependency) |
| `hex-literal` 1.1.0 | crates.io | 2018-01-29 | 72 403 454 | github.com/RustCrypto/utils | OK | Approved (dev-dependency) |
| `proptest` 1.11.0 | crates.io | 2017-06-18 | 182 508 354 | github.com/proptest-rs/proptest | OK | Approved (dev-dependency) |
| `pretty_assertions` 1.4.1 | crates.io | 2017-03-26 | 202 352 676 | github.com/rust-pretty-assertions/rust-pretty-assertions | OK | Approved (optional dev-dependency) |

`[VERIFIED: crates.io registry API `https://crates.io/api/v1/crates/<name>`, 2026-09-08]`

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

**Licence-allow-list note for GUARD-01.** PROJECT.md records `Unicode-3.0` as mandatory (`unicode-ident ← syn ← thiserror-impl`). That still holds — `thiserror` 2.0.20's proc-macro chain is unchanged. Phase 1's `deny.toml`, copied verbatim from Parallax, needs `allow = [..., "Unicode-3.0"]` or `cargo deny check licenses` fails on a clean tree. `NCSA` is **not** needed in Phase 1 (no `libfuzzer-sys` until Phase 2, and D-01 keeps `fuzz/` an independent workspace regardless).

---

## Architecture Patterns

### System architecture — the Phase 1 data flow

```
                                caller-supplied
                             ┌─────────────────────┐
                             │ config + PCM (i32)  │
                             │ + loudness (f64)    │
                             └──────────┬──────────┘
                                        │
                    ┌───────────────────▼───────────────────┐
                    │  PROF-03  lufs_to_q7_8()              │  the ONLY float in
                    │  round_ties_even, range-checked       │  the encode path;
                    │  ONE #[allow(disallowed_types)]       │  D-21's census = 1
                    └───────────────────┬───────────────────┘
                                        │ i16 Q7.8
   ┌────────────────────────────────────▼─────────────────────────────────────┐
   │                          descriptor model  (Vec in bitstream order)      │
   │  IaSequenceHeader → CodecConfig[] → AudioElement[] → MixPresentation[]   │
   │  derived on construct: audio_roll_distance=0, implicit substream ids     │
   │  preserved on parse:   whatever the wire said  (D-06)                    │
   └────────────────────────────────────┬─────────────────────────────────────┘
                                        │
             ┌──────────────────────────┴──────────────────────────┐
             │                                                     │
   ┌─────────▼──────────┐                              ┌───────────▼───────────┐
   │  BCG packer        │  channel-order permutation   │  validate() -> Vec<   │
   │  L,R,C,LFE,Ls,Rs   │  NO arithmetic on samples    │  Finding>  (D-09)     │
   │        ↓           │                              │  explicit, never      │
   │  [L,R] [Ls,Rs]     │                              │  implicit  (D-07)     │
   │  [C]   [LFE]       │                              └───────────────────────┘
   └─────────┬──────────┘
             │ substream 0..3, coupled first
   ┌─────────▼───────────────────────────────────────────────────────────────┐
   │  SEQ-02 streaming writer over W: Write                                  │
   │  push_descriptors → push_temporal_unit(×N) → finish                     │
   │      per OBU: serialise payload to scratch Vec                          │
   │               assert is_byte_aligned()            (BITS-05)             │
   │               obu_size = fields_after_size.len() + payload.len()        │
   │               write byte0 | uleb128(obu_size) | after-size | payload    │
   └─────────┬───────────────────────────────────────────────────────────────┘
             │ bytes
   ┌─────────▼──────────┐   ┌──────────────────────┐   ┌──────────────────────┐
   │ find_obu_boundaries│   │ annotated dump (D-20)│   │  golden hash (D-20)  │
   │  last == len()     │   │  offset/field/hex    │   │  4 targets           │
   │  (OBU-08)          │   │  reviewable PR diff  │   │                      │
   └─────────┬──────────┘   └──────────────────────┘   └──────────────────────┘
             │
   ┌─────────▼───────────────────────────────────────────────────────────────┐
   │        assert_conformant(config, pcm)          (CONF-01, reusable)      │
   ├──────────────────────┬────────────────────┬─────────────────────────────┤
   │ always-on, offline   │ IAMF_REF_DECODER   │ pinned container (Linux CI) │
   │ 4 targets (CONF-10)  │ macOS + Linux      │                             │
   │ • golden hash        │ • iamfdec           │ • decoder_main parse        │
   │ • boundary walk      │   -disable_limiter  │   (CONF-06)                 │
   │ • double-encode      │ • sample count ==   │ • encoder_main companion    │
   │   (GUARD-10)         │ • PCM identical     │   → byte-diff ledger        │
   │ • ref-corpus walk    │   (CONF-05)         │   (CONF-07, D-12)           │
   └──────────────────────┴────────────────────┴─────────────────────────────┘
                                     │
                        .reference-manifest.json checked FIRST (D-13)
```

### Recommended project structure

`src/` layout is Claude's discretion; this satisfies BITS-07 and D-10.

```
src/
├── lib.rs           # SPEC_VERSION, crate lints, D-02's dependency-free doc comment
├── error.rs         # Error { kind, at }, ErrorKind, Location, Finding  (D-08, D-09)
├── bits/            # THE ONLY module allowed to know about bit offsets  (BITS-07)
│   ├── mod.rs
│   ├── reader.rs    # BitCursor: is_byte_aligned, bits_remaining, sub_reader
│   ├── writer.rs    # BitWriter: mirrors reader method-for-method  (BITS-02)
│   └── leb128.rs    # minimal writer (pub), fixed-size writer (pub(crate)), reader
├── obu/
│   ├── header.rs    # byte 0, obu_size origin, TypeSpecific, trim, extension
│   ├── boundaries.rs# find_obu_boundaries  (OBU-08) — written FIRST
│   ├── sequence_header.rs
│   ├── codec_config.rs
│   ├── audio_element.rs
│   ├── mix_presentation.rs
│   ├── audio_frame.rs
│   ├── temporal_delimiter.rs
│   └── parameter_block.rs
├── model/
│   ├── layout.rs    # the FOUR distinct types (DESC-07)
│   ├── profile.rs   # PROF-01, PROF-02
│   └── loudness.rs  # PROF-03 — the single #[allow]
├── packing.rs       # BCG channel -> substream  (TIME-02)
├── sequence.rs      # SEQ-01/02/03
└── dump.rs          # D-20's annotated structural dumper
tests/
├── conformance.rs   # assert_conformant  (CONF-01)
├── vectors.rs       # BITS-06 / D-25 hand-computed vectors
├── refcorpus.rs     # boundary walk over vendored reference .iamf  (offline, CONF-10)
└── citations.rs     # D-23's mechanical // ref: check
```

### Pattern 1 — Two-pass OBU serialisation, mirroring `GetObuSizeAndValidate`

**What:** serialise the payload into a scratch buffer, compute `obu_size` from it, then emit the header.
**When:** every OBU, always. Never backfill.
**Why:** it removes the `obu_size` origin as a free parameter (Pitfall 1) and it is literally what the reference does.

```rust
// ref: iamf/obu/obu_header.cc GetObuSizeAndValidate / ObuHeader::ValidateAndWrite
//      (iamf-tools v2.1.0, 848c6ff)
// obu_size counts every byte AFTER the obu_size field itself: the trim fields,
// the extension header, and the payload. It excludes byte 0 and the size bytes.
fn write_obu(
    w: &mut BitWriter,
    header: &ObuHeader,
    payload: &[u8],          // already serialised into a scratch Vec
    scratch: &mut Vec<u8>,   // reused across frames — see Performance Traps
) -> Result<()> {
    debug_assert!(w.is_byte_aligned());          // BITS-05

    scratch.clear();
    let mut after = BitWriter::new(scratch);
    header.write_fields_after_obu_size(&mut after)?;   // END trim, then START trim, then ext
    if !after.is_byte_aligned() {
        return Err(Error::new(ErrorKind::UnalignedAfterSizeFields, Location::Unlocated));
    }

    let obu_size = (after.len_bytes())
        .checked_add(payload.len())
        .ok_or_else(|| Error::new(ErrorKind::ObuSizeOverflow, Location::Unlocated))?;

    // kEntireObuSizeMaxTwoMegabytes = 1 << 21   (iamf/obu/types.h:32)
    let size_of_obu_size = leb128::minimal_len(obu_size as u32);
    let max = (1usize << 21) - 1 - size_of_obu_size;
    if obu_size > max { return Err(Error::new(ErrorKind::ObuTooLarge, Location::Unlocated)); }

    w.write_unsigned(header.obu_type as u64, 5)?;   // MSB-first, 5 bits  (OBU-01)
    w.write_bool(header.obu_redundant_copy)?;
    w.write_bool(header.trimming_status_flag())?;   // derived from TypeSpecific  (CORRECTION 1)
    w.write_bool(header.extension.is_some())?;      // derived, never stored  (Pitfall 7)
    w.write_uleb128_minimal(obu_size as u32)?;      // D-03
    w.write_bytes(scratch)?;
    w.write_bytes(payload)?;
    debug_assert!(w.is_byte_aligned());
    Ok(())
}
```

### Pattern 2 — Derive every gate flag from the data it gates

**What:** `obu_extension_flag`, `info_type`, `output_gain_is_present_flag`, `recon_gain_is_present_flag` are all *computed*, never stored.
**Why:** the reference does exactly this (`GetExtensionHeaderFlag()`), and it makes the flag/field disagreement of Pitfall 7 unrepresentable.

```rust
// ref: iamf/obu/mix_presentation.cc ValidateAndWriteLayout (iamf-tools v2.1.0)
pub struct Loudness {
    pub integrated: Q7_8,           // signed(16)
    pub digital_peak: Q7_8,         // signed(16)
    pub true_peak: Option<Q7_8>,    // info_type & 0x01
    pub anchored: Option<AnchoredLoudness>,   // info_type & 0x02
    pub extension: Option<Vec<u8>>, // info_type & 0xFC
}
impl Loudness {
    fn info_type(&self) -> u8 { /* computed from the three Options */ }
}
```

### Pattern 3 — Payload sub-readers, asserted fully consumed

**What:** each OBU payload gets a `sub_reader(payload_len)`; at the end, assert `bits_remaining() == 0` **or** drain the remainder into OBU-07's `trailing: Vec<u8>`.
**Why:** it catches under-reads, which are otherwise silent, and it gives OBU-07 a single central drain point (D-05's precedence rule: `trailing` is the *OBU-level* remainder; `Reserved.raw` is the *Audio-Element-payload-level* remainder and consumes to the end of that payload, leaving `trailing` empty).

### Anti-patterns to avoid

- **A shared `ObuHeader` with a `bool` flag.** Makes reserved-bit misuse representable; `libiamf@v1.1.0` will mis-frame it (it reads trim fields for *any* type with bit 6 set) and `iamf-tools@v2.1.0` will reject it.
- **A four-variant `TypeSpecific`.** See CORRECTION 1. `IsNotKeyFrame`/`OptionalFields` are draft-v2.0.0.
- **`BTreeMap` for descriptor collections.** Mix Presentations are written in *list* order, not ID order. `BTreeMap` would reorder them.
- **Golden files generated by this crate and then treated as conformance evidence.** D-20 already labels the dump "aids review, never correctness"; the same caveat must sit on the golden hash.
- **A permutation constant in the harness** (D-19). The channel order is now *known* — `L, R, C, LFE, Ls, Rs` — so there is nothing to tune.
- **Reading `libiamf@main`'s `codec_config_obu.c` comment as authoritative.** It is inverted (§DEC-05). Read the code.

---

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---|---|---|---|
| Bit-level primitive correctness | Confidence in your own `BitCursor` | `bitstream-io` as a **proptest differential oracle** (D-01) | 50M downloads of exercise against ~300 lines written this week. The oracle is the entire justification for hand-rolling. |
| A test corpus of valid `.iamf` files | Generate them with your own encoder | The **226 committed reference `.iamf` files** (CORRECTION 3) | Self-generated goldens freeze bugs (Pitfall 13 §3). These are `iamf-tools` output with matching configs and rendered WAVs, already in git, no Bazel needed. |
| Knowing `test_000003.iamf`'s configuration | Infer it from the bytes | `libiamf@v1.1.0 tests/test_000003.textproto` (CORRECTION 2) | It is published. Hand-decoding remains valuable as the *second* independent derivation (D-25) — which is exactly how I found CORRECTION 5. |
| A conformance decoder | A native Rust decoder | `iamfdec` via `Command` (CONF-09) | Builds in under a minute on macOS with zero dependencies (§Build Recipes). A native decoder is a v2 product decision (DECO-01). |
| A WAV reader/writer for the harness | Hand-rolled RIFF parsing | Python's `wave` in a test script, or a dev-only crate | The harness compares PCM, not containers. Do not add a WAV crate to the shipping graph. |
| Deciding the 5.1 channel order | Reasoning about ITU-R BS.2051 | `libiamf`'s `IAMF_layout.c` table + `iamf-tools`' README table | Both cited above. This is D-19's "cited in the harness". |
| A resampler for `iamfdec`'s `-r` mismatch | Anything | Pass `-r <the fixture's rate>` | The single most likely first DSP breach (Pitfall 15). The flag exists. |

**Key insight:** almost every "hard" fact this phase needs is already written down inside the two reference repositories — in test vectors, in README tables, in enum definitions, in comment blocks. The expensive failure mode is not writing the code; it is *deriving* a fact that was published, and deriving it wrong. Read the pinned tree first, every time.

---

## Common Pitfalls

The 15 pitfalls in `PITFALLS.md` still apply, with §2 amended by CORRECTION 1 and §4/§11 nuanced by CORRECTIONS 4 and 1. Six pitfalls are **new to this research**.

### Pitfall A — The peak limiter turns a loud fixture into a diffuse PCM mismatch

**What goes wrong:** `assert_conformant` reports "PCM differs" with thousands of small differences, concentrated after loud transients. It looks like a rounding or endianness bug.
**Root cause:** `libiamf` applies a −1 dBTP limiter unconditionally with a 200 ms release.
**Prevention:** peak ≤ −6 dBFS in the fixture, plus `-disable_limiter`. Both, and document why.
**Warning signs:** the diff is amplitude-only and channel-uniform; scaling the input down makes it disappear.

### Pitfall B — `iamfdec` exits 0 while writing nothing

**What goes wrong:** `-o3 out/` (a directory) prints `out/ can't opened.` to stderr and **returns 0**. So does a zero-sample decode.
**Prevention:** the harness asserts the output file exists, is larger than a 44-byte WAV header, and that the decoded sample count equals the encoded count *before* comparing PCM. **This is what CONF-05's sample-count clause is for.**
**Warning signs:** a green conformance test that got faster.

### Pitfall C — The `-r` default silently resamples

**What goes wrong:** `iamfdec` defaults to 48000. Decoding `test_000003.iamf` (16 kHz) without `-r 16000` runs the speex resampler and the PCM comparison fails for a reason unrelated to the bitstream.
**Prevention:** the harness always passes `-r` derived from the Codec Config it wrote.

### Pitfall D — The mandatory stereo layout is easy to forget on a 5.1 fixture

**What goes wrong:** `encoder_main` fails with `"Every sub-mix must have a stereo layout."` while producing the CONF-07 companion file, after the crate has already written a 5.1-only sub-mix that `libiamf` happily decoded.
**Root cause:** `libiamf` does not enforce it; `iamf-tools` enforces it **on write only** (there is a `TODO` for the read path).
**Prevention:** the 5.1 fixture carries **two** layouts (Sound System B and Sound System A) with two `Loudness` blocks. `validate()` reports a missing stereo layout as a Finding but the parser does not reject it.

### Pitfall E — `additional_profile < primary_profile` is rejected by `libiamf` and unchecked by `iamf-tools`

**What goes wrong:** the reverse of the usual asymmetry. `libiamf`'s `_valid_profile` requires `primary <= additional`; `iamf-tools` does not validate `additional_profile` at all. A file that passes CONF-06 fails CONF-05.
**Prevention:** PROF-02's minimum-profile selection must set `additional_profile >= primary_profile`. Emitting both equal is always safe.

### Pitfall F — Trusting a comment in the reference

**What goes wrong:** `libiamf@main`'s LPCM comment says `0x01 - big endian, 0x00 - little endian`; the code one line below does `param->big_endian = !ior_8(r)`. Following the comment inverts DESC-03 and produces white noise that looks like a DSP bug — exactly the failure PITFALLS.md §6 names as "the most expensive kind".
**Prevention:** D-23's `// ref:` citations must point at the **function**, and the reviewer's job is to read the code at that citation, not its docstring. Consider extending the citation convention to `// ref: <path> <symbol>` (which it already is) and adding, where a comment misleads, a `// NOTE: the comment above this line in the reference is inverted; the code is authoritative.`

---

## Reference Build Recipes

### `libiamf` @ `v1.1.0` on macOS — executed, working

**Corrections to CONTEXT.md / STACK.md, all `[VERIFIED-FROM-CODE: libiamf@v1.1.0]`:**
- **CMake ≥ 3.6**, not 3.28. (3.28 is a `main` requirement.)
- **No git submodules.** `.gitmodules` is absent at `v1.1.0`. The `AOMediaCodec/oar` submodule is a `main` fact.
- **No `-DIAMF_TEST_TOOL=ON` option.** The options at `v1.1.0` are `BUILD_SHARED_LIBS`, `SUPPORT_VERIFIER`, `CODEC_CAP`, `MULTICHANNEL_BINAURALIZER`, `HOA_BINAURALIZER`. `iamfdec` is built by a **separate CMake project** under `code/test/tools/iamfdec/`.
- **The codec libraries are not needed for LPCM.** `CODEC_CAP=ON` (default) does `find_library(... PATHS dep_codecs/lib NO_DEFAULT_PATH)` for opus/fdk-aac/FLAC; on failure it emits a `WARNING`, skips the corresponding `src/iamf_dec/<codec>` source directory, and links nothing. The LPCM path is always compiled.

**The non-obvious step.** `code/dep_codecs/lib/` ships **x86_64-Linux** `.a` files (`libopus.a`, `libFLAC.a`, `libfdk-aac.a`) alongside Windows `.lib` files. CMake on macOS *finds* the `.a` files, compiles the codec sources in, and then the link of `iamfdec` fails with hundreds of `ld: symbol(s) not found for architecture x86_64`. I reproduced this. `[VERIFIED-BY-EXECUTION]` The fix is to move them aside so `find_library` genuinely fails.

**The recipe, as executed on macOS 14.8.8 / x86_64 / AppleClang 16.0.0 / CMake 4.4.3:**

```bash
# tools/build-reference.sh  (libiamf portion)
set -euo pipefail
LIBIAMF_SHA=f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63   # tag v1.1.0
git clone --filter=blob:none --no-checkout https://github.com/AOMediaCodec/libiamf.git "$SRC"
git -C "$SRC" checkout -q "$LIBIAMF_SHA"

cd "$SRC/code"
# Phase 1 is LPCM-only. The shipped dep_codecs/lib/*.a are x86_64-Linux and will
# be found by find_library() but cannot link on macOS. Move them aside so the
# CMake probe fails cleanly and the codec sources are excluded.
# (Phase 3 will need real opus/FLAC libs here — see dep_codecs/README.md.)
mkdir -p dep_codecs/lib_disabled
mv dep_codecs/lib/*.a dep_codecs/lib/*.lib dep_codecs/lib_disabled/ 2>/dev/null || true

cmake -DCMAKE_INSTALL_PREFIX="$PREFIX" -DBUILD_SHARED_LIBS=OFF .
make -j"$(sysctl -n hw.ncpu)"
make install

cd test/tools/iamfdec
cmake -DCMAKE_INSTALL_PREFIX="$PREFIX" .
make -j"$(sysctl -n hw.ncpu)"
# => ./iamfdec   (291 760 bytes here)
```

Observed configure output confirming the codec exclusion:
```
  the opus library was not found
  the fdk-aac library was not found
  the FLAC library was not found
```
`[VERIFIED-BY-EXECUTION: full build + install + iamfdec link succeeded, 2026-09-08]`

`BUILD_SHARED_LIBS=OFF` is recommended so `iamfdec` needs no `DYLD_LIBRARY_PATH` at test time. Two harmless `ranlib: file: libiamf.a(h2b_rdr.c.o) has no symbols` warnings appear (the binauralizer stubs); they are not errors.

**Smoke test to put in `build-reference.sh` (D-13's manifest step):**
```bash
"$PREFIX/../iamfdec" -i0 -o3 /tmp/t3.wav -r 16000 -s0 -d 16 -disable_limiter "$SRC/tests/test_000003.iamf"
# assert /tmp/t3.wav has 8000 frames and is sample-identical with
#        "$SRC/tests/sawtooth_100_stereo.wav"
```
This is a self-validating build check: it proves the binary works *and* it proves the harness's comparison logic, before any of our own bytes exist. `[VERIFIED-BY-EXECUTION: 0 differing samples of 16000, 2026-09-08]`

**`.reference-manifest.json` (D-13) should record:** the two SHAs actually checked out, `sha256` of `libiamf.a` and `iamfdec`, the host triple, the CMake version, and whether `dep_codecs` was disabled (because a Phase 3 build with codecs *will* produce a different `iamfdec` hash and that must be a visible manifest change, not a mystery).

### `iamf-tools` @ `v2.1.0` in a digest-pinned container

Prerequisites, from the project's own docs: **Bazelisk**, **CMake** (some deps build with it), **Clang 13+ or GCC 10+**. `[CITED: iamf-tools@v2.1.0 docs/build_instructions.md]` No Bazelisk is installed on this machine, so the recipe below is **`[ASSUMED]` in its details** and must be validated by plan 01-02.

```dockerfile
# tools/iamf-tools.Dockerfile
# ubuntu:24.04 multi-arch index digest, verified against registry-1.docker.io 2026-09-08
FROM ubuntu@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517

ARG IAMF_TOOLS_SHA=848c6ff4968ff8cc6f728259892ab4f90cb83256   # tag v2.1.0
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates git curl build-essential clang cmake python3 unzip zip \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /usr/local/bin/bazel \
      https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-amd64 \
    && chmod +x /usr/local/bin/bazel
WORKDIR /src
RUN git clone --filter=blob:none --no-checkout \
      https://github.com/AOMediaCodec/iamf-tools.git . \
 && git checkout -q "${IAMF_TOOLS_SHA}"
RUN bazel build -c opt //iamf/cli:encoder_main //iamf/cli:decoder_main
```

`[VERIFIED-BY-EXECUTION: the ubuntu:24.04 digest, via registry-1.docker.io manifest HEAD, 2026-09-08]`
`[ASSUMED: that this apt set is sufficient and that bazelisk-latest resolves a Bazel version v2.1.0 accepts]`

**Notes for plan 01-02:**
- D-13 requires the image be pinned **by digest**. Pin *both* the base image digest (above) **and**, once built, the digest of your own resulting image, recording it in `REFERENCES.md`.
- `bazelisk-linux-amd64` on an Apple Silicon dev machine needs `--platform=linux/amd64` or the `arm64` asset. Prefer building the image in CI (Linux x64) and pulling it locally.
- Pin the Bazelisk release too — `latest` is exactly the moving target GUARD-06 exists to prevent. Replace with a versioned URL once 01-02 has picked one.
- `encoder_main` has `data = ["//iamf/cli/testdata:input_wav_files"]`, so the input WAVs are already inside the build. Our own fixture WAV has to be mounted in. `[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/BUILD:750-752]`
- Usage, from the docs: `bazelisk build -c opt //iamf/cli:encoder_main`, and `bazel-bin/iamf/cli/decoder_main --input_filename=… --output_filename=…`. `[CITED: iamf-tools@v2.1.0 docs/build_instructions.md, docs/iamf_decoder_main.md]`

---

## State of the Art

| Old (as written in this repo's docs) | Current (at the pinned tags) | Impact |
|---|---|---|
| "read `iamf-tools` at a **v1.x** tag" | There is no v1.1.x tag; **v2.1.0** is the v1.1.0-exact tree | DEC-04 resolved. Update `REFERENCES.md` wording so a future reader does not go hunting. |
| Bit 6 is polymorphic across four OBU types | v1.1.0: legal on audio frames only; the polymorphism is draft-v2.0.0 | CORRECTION 1. Blocks plan 01-04. |
| "338 textprotos and exactly one `.iamf`" | 226 textprotos + 5 `.iamf` at v2.1.0; **221 `.iamf` + 221 textprotos + 403 WAVs** in `libiamf@v1.1.0 tests/` | CORRECTION 3. Bazel is no longer a fixture-supply blocker. |
| `test_000003.iamf`'s configuration must be inferred | Published as `tests/test_000003.textproto` | CORRECTION 2. D-17's CONF-08 waiver is unnecessary. |
| `probe_main` exists | Only `encoder_main` and `decoder_main` at v2.1.0 | CORRECTION 6. CONF-06 goes through `decoder_main`. |
| `libiamf` needs CMake ≥ 3.28 and a submodule | v1.1.0: CMake ≥ 3.6, no submodules, no codec deps for LPCM | Build recipe simplifies; the real obstacle is `dep_codecs/lib`. |
| The reference encoder may emit non-minimal leb128 | It **can** (`kFixedSize`), but its default is `kMinimum` and 524 674/524 674 reference `obu_size` fields are minimal | CORRECTION 4. D-03's fixed-size path is for parsing and `test_000134`, not CONF-08. |
| `libiamf` clamps an overlong leb128 to `UINT32_MAX` | True of `main`'s `ior_leb128_u32`. At **v1.1.0**, `bs_getAleb128` returns `UINT64_MAX` as a sentinel and `IAMF_OBU_split` returns 0 | The asymmetry PITFALLS.md §4 warns about is `main`-flavoured. The v1.1.0 behaviour is different but equally permissive (returns 0 = "end of stream"). |

**Deprecated / do not use:**
- `iamf-tools@main` for anything (draft v2.0.0).
- `libiamf@main` as the pinned decoder (Metadata OBU + profiles > base-enhanced have leaked in).
- `iamf-tools@v2.1.0 iamf/cli/testdata/iamf/tones_256samp_5p1_pcm.iamf` as a golden (`num_samples_per_frame = 0`).
- `libiamf@v1.1.0 tests/*.textproto` as `encoder_main` input (old proto dialect; use `iamf-tools@v2.1.0 iamf/cli/testdata/` copies).

---

## Assumptions Log

Claims tagged `[ASSUMED]` above. Each needs confirmation before it becomes a locked decision.

| # | Claim | Section | Risk if wrong |
|---|---|---|---|
| A1 | The 24-bit float→int conversion in `iamf_decoder_plane2stride_out` is exact, so D-18's 24-bit fixture round-trips sample-identically | Decoder output channel order | **HIGH.** If inexact, D-18's 24-bit big-endian choice (the whole DESC-03 coverage) needs a gate waiver or a redesign. **Plan 01-08 must test this first, before the fixture is frozen.** Cheap: encode a 24-bit BE fixture, decode with `-d 24 -disable_limiter`, compare. |
| A2 | The Dockerfile's apt package set is sufficient to build `iamf-tools@v2.1.0` and Bazelisk-latest resolves an acceptable Bazel | Reference Build Recipes | MEDIUM. Plan 01-02's first task. Failure costs container-iteration time, not a design change. |
| A3 | `iamf-tools@v2.1.0`'s `encoder_main` accepts a Codec Config that no Audio Element references | D-18 unreferenced Codec Config | MEDIUM. The parser side is verified; the encoder side is not. D-18 already flags this as a plan 01-02 research item and two fallbacks are specified. |
| A4 | PITFALLS.md §3's "within one temporal unit, every audio frame must carry identical trim values and timestamps" still holds at v2.1.0 | DEC-05, Audio Frame | LOW. Read from `main` originally; I did not re-open `temporal_unit_view.cc` this session. It only bites a multi-substream fixture, which D-18's 5.1 fixture is — so plan 01-06 should re-verify it, cheaply, by grep at the pinned tag. |
| A5 | GitHub's `macos-13` x86_64 runner is on the retirement path (D-16's premise) | — | MEDIUM. Not verified this session. D-16 already requires a documented fallback in plan 01-01, which covers it either way. |
| A6 | `cargo-deny 0.20.2` resolves `bitstream-io`'s slash-form `MIT/Apache-2.0` as `OR` | Standard Stack | LOW. Recorded in STACK.md as verified on 2026-09-07; not re-run today. GUARD-01's "prove it fails on a deliberately-added LGPL crate" step will surface any surprise. |

---

## Open Questions

1. **Does `iamf-tools`' `decoder_main` return a non-zero exit code on a parse failure?**
   - What we know: it goes through `ObuProcessor`/`DescriptorObuParser` and the docs say "If successful, the decoder will produce an `output_file.wav`".
   - What's unclear: whether failure is signalled by exit code, stderr, or only by a missing output file.
   - Recommendation: plan 01-02 runs it against a deliberately corrupted `.iamf` and records the observed signal in `CONFORMANCE-GATE.md`. Do not assume exit codes — `iamfdec` already demonstrated that a reference tool can fail and return 0 (Pitfall B).

2. **Should the 221 `libiamf@v1.1.0` `.iamf` fixtures be vendored into this repo?**
   - What we know: they are BSD-3-Clause-Clear, they total a few MB, and they would satisfy CONF-10's "green offline on all four targets" for the boundary-walk and (in Phase 2) parse tests without any reference binary.
   - What's unclear: D-14's per-file size cap, and the fact that Parallax consumes this crate as a path dependency so every fixture lands in every Parallax clone.
   - Recommendation: vendor a **curated subset** (the LPCM ones, plus `test_000003.*`, plus a handful of structurally interesting ones), with a `MANIFEST.md` per D-14 and a `NOTICE` entry per GUARD-07. Fetch the rest on demand in the reference CI job.

3. **Which `iamfdec` invocation is canonical for `assert_conformant`?**
   - What we know: `-disable_limiter` is correct for sample-identity; the default (limiter on) is closer to what a real consumer does.
   - Recommendation: run **both**. `-disable_limiter` for the sample-identity assertion, default for a second, weaker assertion that the decode succeeds and the sample count matches. That way a future limiter-engaging fixture fails loudly rather than being masked.

4. **Does `test_000134`'s fixed-size leb128 file round-trip through our parser?** Phase 2 question (PARSE-04), but it is the *only* non-minimal reference file and it should be named in Phase 2's plan now, while it is known.

---

## Environment Availability

Probed on this machine, 2026-09-08.

| Dependency | Required by | Available | Version | Fallback |
|---|---|---|---|---|
| `rustc` | everything | ✓ | 1.92.0 (2025-12-08) | — (GUARD-12 pins 1.85 via `rust-toolchain.toml`; 1.92 satisfies it) |
| `cargo` | everything | ✓ | 1.92.0 | — |
| `cargo-deny` | GUARD-01 | ✓ | 0.20.2 | — |
| CMake | `libiamf` build | ✓ | 4.4.3 | Emits a deprecation warning for `cmake_minimum_required(VERSION 3.6)`; **configures and builds successfully** `[VERIFIED-BY-EXECUTION]` |
| AppleClang / `make` | `libiamf` build | ✓ | AppleClang 16.0.0.16000026 | — |
| `git` | reference checkout | ✓ | — | — |
| `docker` | `iamf-tools` container (D-11) | ✓ (CLI present) | — | Daemon not exercised this session; CI is the primary venue |
| `bazelisk` / `bazel` | `iamf-tools` native build | ✗ | — | **Intended** — D-11 says `iamf-tools` runs only in the container |
| `ffmpeg` | D-15 Phase 2 oracle | ✓ | 9.0.1 | — |
| Network to github.com / crates.io / registry-1.docker.io | reference fetch | ✓ | — | — |

**FFmpeg IAMF support, confirmed empirically (D-15's premise):**
```
 DE  iamf            Raw Immersive Audio Model and Formats
```
and a full demux of `test_000003.iamf` to a 32 078-byte WAV: `Stream #0:0 -> #0:0 (pcm_s16le (native) -> pcm_s16le (native))`, `16000 Hz, stereo, s16`. `[VERIFIED-BY-EXECUTION: ffmpeg -formats | grep iamf; ffmpeg -i … -f wav, 2026-09-08]` `D` and `E` = both demuxer and muxer. **No FFmpeg source was read.**

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `bazelisk` — by design, it lives in the container.

---

## Security Domain

`security_enforcement: true`, `security_asvs_level: 1`. This is an offline library with no network, no auth, no persistence, and no user identity. Most ASVS categories are structurally inapplicable; V5 and V6 map onto real work.

### Applicable ASVS categories

| ASVS category | Applies | Standard control |
|---|---|---|
| V2 Authentication | no | No principals. |
| V3 Session Management | no | No sessions. |
| V4 Access Control | no | No resources to guard. |
| V5 Input Validation | **yes** | Every parsed length field is attacker-controlled. Controls: bounds-checked `BitCursor` (`bits_remaining()`), `sub_reader(len)` per OBU payload, `checked_add`/`checked_sub`/`checked_mul` enforced by `clippy::arithmetic_side_effects`, no raw indexing enforced by `clippy::indexing_slicing`, and **never allocate from a parsed length without capping it against `bytes_remaining()`**. Hard ceilings taken from the reference: `kMaxLeb128Size = 8`, decoded uleb128 ≤ `u32::MAX`, whole OBU ≤ `1 << 21` bytes. |
| V6 Cryptography | **yes, narrowly** | The only cryptographic primitive is the SHA-256 in GUARD-09's golden hash. **Do not hand-roll it** and do not add a hashing crate to the shipping graph — compute it in the CI job (`shasum -a 256` / `sha256sum` / `certutil -hashfile`) or in a dev-dependency. It is a change-detection digest, not a security boundary. |
| V7 Error Handling & Logging | **yes** | D-08's `Location` carries a byte offset into every error. Offsets are not sensitive here (the input is the caller's own file). No `panic!` on malformed input — GUARD-04 plus `clippy::panic`. |
| V12 File & Resources | **yes** | The parser must terminate and must not OOM on hostile input. Phase 1 installs the lints; Phase 2's fuzzer proves it. |
| V14 Configuration | **yes** | GUARD-06/D-13: reference SHAs pinned, container pinned by digest, manifest asserted before any conformance clause runs. An unpinned reference is a supply-chain hole, not just a reproducibility one. |

### Known threat patterns for this stack

| Pattern | STRIDE | Standard mitigation |
|---|---|---|
| Unbounded allocation from a parsed length (`obu_size`, `extension_header_size`, `info_type_size`, `num_substreams`, `num_layouts`, …) | Denial of Service | Cap `n` against `reader.bytes_remaining()` **before** reserving. The reference does exactly this: `ValidateInRange(info_type_size, {0, kEntireObuSizeMaxTwoMegabytes})` before `resize`, with an explicit comment about not reading a large extension until it is plausible. |
| Integer overflow in size arithmetic (`obu_size − trim − ext`) | Tampering → DoS | `checked_*` everywhere; `clippy::arithmetic_side_effects` at `deny`. The reference computes this in `int64_t` and rejects a negative result, which tells you hostile input reaches it. |
| Slice-index panic in a library embedded in a DAW | Denial of Service | `#![deny(clippy::indexing_slicing)]`, all reads through the bounds-checked cursor. **No `#[allow]` in non-test code** — this is the lint people disable first. |
| Loop driven by a `u32` count with a ≥1-byte body | Denial of Service | `n <= bytes_remaining()`. One rule, every count in the format. |
| Supply-chain: an unpinned reference or container | Tampering | GUARD-06 + D-13's runtime manifest assertion. |
| Supply-chain: a hallucinated or typosquatted crate | Tampering | §Package Legitimacy Audit; five crates, all registry-verified with source repos. |
| Licence contamination from an LGPL source | (legal, not STRIDE) | GUARD-08's `CONTRIBUTING.md` checkbox, extended by D-15 to name FFmpeg/`libavformat`. Irreversible; prevention is the only control. |
| `unsafe` code | Elevation of Privilege | `#![forbid(unsafe_code)]` in `lib.rs`. Phase 1 has no FFI. |

---

## Project Constraints (from CLAUDE.md)

Actionable directives extracted from `./.claude/CLAUDE.md`. The planner must verify compliance; nothing in this research contradicts any of them.

| Directive | Where it binds in Phase 1 |
|---|---|
| **No `HashMap`/`HashSet` anywhere output byte order can see them. Prefer `Vec` in bitstream order plus a `by_id` lookup — not `BTreeMap`.** | DESC-09, GUARD-02. Confirmed correct by `obu_sequencer_base.cc`: Mix Presentations are written in *list* order, so `BTreeMap` would reorder them. |
| **Byte-identity as a committed golden fixture**, not a cross-job artifact comparison. | GUARD-09, D-20. |
| **No platform transcendentals** in anything affecting output bytes; `libm` if any appear. | GUARD-11, D-21. Confirmed non-binding: the IAMF wire format has no float fields; PROF-03 needs only `* 256.0` and `round_ties_even`, both IEEE-754-exact. |
| **Licence allow-list**; `Unicode-3.0` mandatory; state each dependency's licence on the line that adds it. | GUARD-01. §Standard Stack states every licence inline. |
| **`cargo-deny` config shape** — allow-list only; `deny`/`copyleft`/`allow-osi-fsf-free`/`default`/`version` keys are removed and now error; rejection by omission, recorded in a comment. | GUARD-01, plan 01-01. |
| **Source licence hygiene** — `libspatialaudio`, `gpac` may not be read or ported; `iamf-tools`, `libiamf`, `eclipsa-audio-plugin`, `libear`, `obr` may. Extended by D-15 to forbid FFmpeg/`libavformat`. | GUARD-08. **This session read only `iamf-tools` and `libiamf`.** |
| **Pin the references** — an `iamf-tools` v1.x tag and SHA, and a `libiamf` SHA, named in the code. | GUARD-06, DEC-04. §DEC-04 supplies both, with the wording caveat that "v1.x tag" resolves to `v2.1.0`. |
| **Pin the spec version** — a `SPEC_VERSION` constant named in the code. | GUARD-05, DEC-01 (`"1.1.0"`). |
| **`thiserror` for errors, never `anyhow` in a library.** | D-08, GUARD-13. |
| **Parser hardening** — no `unwrap()`/`expect()` outside tests, plus `clippy::indexing_slicing` and `clippy::arithmetic_side_effects`. | GUARD-03, GUARD-04, §Security Domain. |
| **Rust edition 2024.** | GUARD-12. All five crates are compatible; `proptest` and `hex-literal` set the effective MSRV at 1.85, which is the edition-2024 floor. |
| **Fuzzing ships in M2, in an independent `fuzz/` workspace.** | Not Phase 1. Phase 1 installs the lints the fuzz target will depend on. |
| **Milestone ordering: M1 may not be reordered.** | The whole phase. |
| **GSD workflow enforcement** — no direct repo edits outside a GSD workflow. | This research wrote only its own output file. |

---

## Sources

### Primary — read from source at a named revision (HIGH confidence)

**`AOMediaCodec/iamf-tools` @ `v2.1.0` = `848c6ff4968ff8cc6f728259892ab4f90cb83256`** — BSD-3-Clause-Clear + AOM Patent License 1.0
- `iamf/obu/obu_header.{cc,h}` — byte-0 layout, `obu_size` origin, `IsRedundantCopyAllowed`, `IsTrimmingStatusFlagAllowed`, `WriteFieldsAfterObuSize`, 2 MB ceiling
- `iamf/obu/types.h` — `kMaxLeb128Size`, `kEntireObuSizeMaxTwoMegabytes`
- `iamf/obu/ia_sequence_header.h` — `ProfileVersion`, `kIaCode`
- `iamf/obu/audio_element.h` — `AudioElementType`, `LoudspeakerLayout`, `ExpandedLoudspeakerLayout`
- `iamf/obu/codec_config.{cc,h}` — `ValidateNumSamplesPerFrame`, `kMaxPracticalFrameSize`, `OverrideAudioRollDistance`
- `iamf/obu/decoder_config/lpcm_decoder_config.{cc,h}` — `LpcmFormatFlagsBitmask`, `ValidateSampleSize`, `ValidateSampleRate`, `GetRequiredAudioRollDistance`
- `iamf/obu/mix_presentation.{cc,h}` — mandatory-stereo-layout rule, `SoundSystem`, `LoudspeakersSsConventionLayout::Write/Read`
- `iamf/obu/param_definitions.h` — `ParameterDefinitionType`
- `iamf/obu/mix_gain_parameter_data.h` — `AnimationType`, the three animation structs
- `iamf/common/leb_generator.h` — `GenerationMode`, default `kMinimum`
- `iamf/cli/obu_sequencer_base.cc` — `WriteDescriptorObus`, `FillDescriptorStatistics`, `WriteTemporalUnit`
- `iamf/cli/cli_util.cc` — `GetCommonSamplesPerFrame`
- `iamf/cli/descriptor_obu_parser.cc` — Codec Config map insertion
- `iamf/cli/profile_filter.cc` — profile element/channel limits, `FilterAudioElementType`, `FilterExpandedLoudspeakerLayout`
- `iamf/cli/proto/test_vector_metadata.proto`, `iamf/cli/proto/arbitrary_obu.proto`
- `iamf/cli/BUILD` — the two `cc_binary` targets
- `iamf/cli/testdata/test_000003.textproto`, `test_000119.textproto`
- `iamf/cli/testdata/iamf/*.iamf` (5 files, byte-level)

**`AOMediaCodec/iamf-tools` @ `v2.0.0` = `bee0f286…` and @ `main` = `901a86e1…`** — read only for the four-way v1.1.0-vs-draft-v2 comparison and the `type_specific_flag` grep.

**`AOMediaCodec/libiamf` @ `v1.1.0` = `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63`** — BSD-3-Clause-Clear + AOM Patent License 1.0
- `code/src/iamf_dec/IAMF_OBU.c` — `IAMF_OBU_split`, `_valid_profile`, `_valid_codec`, `_valid_decoder_config`, `iamf_version_new`, `iamf_codec_conf_new`, `iamf_frame_new`, `iamf_parameter_base_init`
- `code/src/iamf_dec/IAMF_types.h` — `IAChannel`, `IAReconChannel`, `IAMFExpandedLayoutType`, profile enum
- `code/src/iamf_dec/IAMF_layout.c` — the layout table, `decoding_map`, `iamf_audio_layer_layout_get_decoding_channels`
- `code/src/iamf_dec/IAMF_decoder.c` — the BCG ordering comment block, limiter lifecycle, `iamf_delay_buffer_handle`, `IAMF_decoder_set_sampling_rate`, `IAMF_decoder_peak_limiter_*`
- `code/src/iamf_dec/pcm/IAMF_pcm_decoder.c` — endianness selection, coupled/mono planar layout, length checks
- `code/src/iamf_dec/m2m_rdr.c` — `iamf51_bs050` identity matrix, `m2m_rdr_tab`
- `code/src/iamf_dec/audio_effect_peak_limiter.c`, `code/src/common/audio_defines.h` — limiter constants and gain law
- `code/test/tools/iamfdec/src/test_iamfdec.c`, `code/test/tools/iamfdec/CMakeLists.txt` — CLI flags, output naming
- `code/CMakeLists.txt`, `code/README.md`, `code/build.sh`, `code/dep_codecs/` — build recipe
- `PATENTS` — AOM Patent License 1.0, read in full
- `tests/test_000003.{iamf,textproto}`, `tests/sawtooth_100_stereo.wav`, `tests/test_000003_rendered_id_42_sub_mix_0_layout_0.wav`, and 221 `.iamf` files scanned byte-level

**`AOMediaCodec/libiamf` @ `main` = `e55e1832a608affe602de2ee39929bd7759a75ab`** — `code/src/iamf_dec/obu/{iamf_obu.c,codec_config_obu.c,audio_frame_obu.c,ia_sequence_header_obu.c}`, `README.md`. Read because the roadmap named these files; **not** the recommended pin.

### Executed locally, 2026-09-08 (HIGH confidence)

- `git ls-remote --tags` on both repositories — the SHA table
- CMake 4.4.3 + AppleClang 16 build of `libiamf@v1.1.0` and `iamfdec` on macOS 14.8.8 x86_64, twice (once reproducing the `dep_codecs` link failure, once with the fix)
- `iamfdec` decode of `test_000003.iamf` with and without `-disable_limiter`; PCM compared against `sawtooth_100_stereo.wav` — 0 differing samples of 16 000, 8000 frames
- `iamfdec` decode of `tones_256samp_5p1_pcm.iamf` — 0 frames, 0 samples
- Python OBU walker over 226 `.iamf` files — 524 674 `obu_size` fields, 0 non-minimal, 0 walk mismatches
- Python descriptor scanner over the same corpus — 77 files with >1 Codec Config or >1 Audio Element, 0 with an unreferenced Codec Config
- Hand-decode of `test_000003.iamf`'s 120-byte prologue, cross-checked against its textproto
- `diff` of `libiamf@v1.1.0 tests/test_000003.textproto` vs `iamf-tools@v2.1.0 iamf/cli/testdata/test_000003.textproto`
- crates.io registry API for five crates — versions, licences, MSRVs, download counts, repositories
- `registry-1.docker.io` manifest HEAD for `ubuntu:24.04` — digest
- `ffmpeg -formats | grep iamf` and a full IAMF→WAV demux
- `rustc --version`, `cargo --version`, `cargo deny --version`, `cmake --version`, `sw_vers`, `uname -m`

### Secondary — reference-project prose (MEDIUM confidence)

- `iamf-tools@v2.1.0 CHANGELOG.md` — the "based on IAMF v1.1.0" / "IAMF v1.0.0-errata" statements
- `iamf-tools@v2.1.0 iamf/cli/testdata/README.md` — the output-WAV channel-order table
- `iamf-tools@v2.1.0 iamf/cli/testdata/iamf/README.md` — the five small-file descriptions
- `iamf-tools@v2.1.0 docs/build_instructions.md`, `docs/iamf_decoder_main.md`
- `libiamf@v1.1.0 code/README.md`, `tests/README.md`; `libiamf@main README.md`

### Not consulted, by constraint

`gpac` (LGPL-2.1), `libspatialaudio` (LGPL-2.1+), **FFmpeg / `libavformat` (LGPL-2.1+)**. No search result was followed into `libavformat/iamf*.c`. The `ffmpeg` binary was invoked; its source was not opened.

### Appendix — AOM Patent License 1.0 §1.2, read in full

Read from `libiamf@v1.1.0 PATENTS` (identical text ships in `iamf-tools@v2.1.0 PATENTS`). `[VERIFIED-FROM-CODE: libiamf@v1.1.0 PATENTS]`

**§1.2.1 Availability** — *as a condition* of the §1.1 grant to make/sell/offer for sale/import/distribute an Implementation, the Licensee must (i) make **its own** Necessary Claims available under this License, and (ii) **reproduce this License with any Implementation**: *"a. For distribution in source code, by including this License in the root directory of the source code with its Implementation."*

**§1.2.2 Additional Conditions** — the licence is directly Licensor→Licensee; no rights flow through suppliers or distributors.

**How this lands on `iamf-rs`:**
- `iamf-rs` is an **Encoder** (§2.4) and therefore an **Implementation** (§2.6). Distributing it makes us a **Licensor** (§2.9(i)).
- **Actionable for plan 01-01: add a `PATENTS` file at the repository root** containing the AOM Patent License 1.0 verbatim. This is §1.2.1(a)'s literal requirement and it is the condition on the *inbound* grant, so omitting it costs us the patent licence we are relying on.
- It sits **alongside**, not instead of, `LICENSE-MIT` + `LICENSE-APACHE` + `NOTICE`. Apache-2.0 §3's patent grant and AOM §1.1 are independent; DEC-02 is unaffected and needs no revisiting.
- §1.3 Defensive Termination is standard and imposes no packaging obligation.
- §2.10(b)(ii) defines Necessary Claims to include claims infringed by the Reference Implementation — relevant only if this project ever accepts a contributor with a patent portfolio; nothing to do now.
- **Packaging note:** when `publish = false` is eventually lifted, ensure `Cargo.toml`'s `include`/`exclude` keeps `PATENTS` in the published `.crate`, since §1.2.1(a) speaks about "the root directory of the source code".

---

## Metadata

**Confidence breakdown:**

| Area | Level | Reason |
|---|---|---|
| DEC-04 (`iamf-tools` tag) | **HIGH** | Four independent discriminating checks, each a verbatim enum quoted at three revisions. |
| DEC-05 (payload rejection rules) | **HIGH** | Read at both `libiamf` revisions and cross-checked against `iamf-tools`' validators; endianness confirmed three ways including from the code that acts on it. |
| leb128 minimality (CORRECTION 4) | **HIGH** | 524 674 fields across 226 files, plus the generator default and the proto default. |
| BCG packing for 5.1 (TIME-02) | **HIGH** | Rule comment + `decoding_map` arithmetic + a shipped file's own OBU sizes; three independent derivations agreeing. |
| Output channel order (D-19) | **HIGH** | Layout table + `iamf-tools` README table. |
| OBU-03 correction | **HIGH** | Positive code at v2.1.0 plus a repo-wide grep returning nothing, contrasted with the same grep at `main`. |
| Peak limiter constraint | **HIGH** for the mechanism and constants; **HIGH** for 16-bit exactness (executed); **MEDIUM** for 24-bit (A1) | Constants and gain law read from source; 16-bit round trip executed; 24-bit not yet tested. |
| `test_000003` byte layout | **HIGH** | Hand-decoded and independently confirmed against the published textproto — two derivations, agreeing, per D-25. |
| `libiamf` macOS build recipe | **HIGH** | Executed twice on this machine, including reproducing the failure mode it fixes. |
| `iamf-tools` container recipe | **MEDIUM** | Base-image digest verified; the apt set and Bazelisk pin are `[ASSUMED]` (A2). |
| Standard stack versions/licences | **HIGH** | crates.io registry API today. |
| Unreferenced Codec Config legality | **MEDIUM** | Parser side verified; encoder side inferred, with two specified fallbacks (A3). |

**Research date:** 2026-09-08
**Valid until:** 2026-10-08 for the crates.io versions (stable crates, 30 days). **Indefinite** for every fact read at a pinned SHA — that is what pinning buys, and it is why GUARD-06 exists. Re-verify only when a pin is deliberately bumped, and re-run the whole conformance suite in the same commit (PITFALLS.md §11).
