# Quick 260913-qk3: enforce IAMF v1.1.0 profile restrictions — Research

**Researched:** 2026-09-13
**Base commit:** `6622a09` (repo `src/` and `tests/` unmodified by this research)
**Domain:** IAMF profile conformance (spec v1.1.0 plus v1.0.0-errata as adopted; `iamf-tools@848c6ff4` = `v2.1.0`; `libiamf@f06e919e` = `v1.1.0`)
**Confidence:** HIGH for the spec text, the reference behaviour and the crate state (all read this session). MEDIUM for the design, which was prototyped on a scratch copy and run against the full suite and both pinned decoders.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Directive refinement (user, 2026-09-13):** "3 wundert mich, wir sollten uns an die spec halten" ("3 surprises me, we should follow the spec"), "4 bitte fixen" ("4 please fix"), "5 bitte fixen" ("5 please fix").
- **Satisfy BOTH the spec and the pinned references.** Where the spec is stricter than `iamf-tools@v2.1.0` or `libiamf@v1.1.0`, the spec wins (items 3 and 4). Where a pinned reference is stricter (e.g. `parameter_rate` must equal the sample rate), the reference rule stays. Record each spec/reference disagreement in a `// ref:` comment.
- `SPEC_VERSION = "1.1.0"`. v1.1.0 is binding; errata text binds only as far as v1.1.0 adopts it.
- **Items 3 and 4 change profile selection; they do not add rejections.** `select_minimum_profile` must raise the profile when the Simple/Base limits are exceeded across the whole sequence, or when the Base scene-based or multi-layer limits are exceeded. Reject only when no profile allows the configuration: for Base-Enhanced that means the 28-element and 28-channel ceilings, scoped exactly as v1.1.0 states them. Invert or rename `unrelated_presentations_do_not_sum_their_element_or_channel_limits`.
- **Items 1 and 2 are `build()` rejections.** Use payload-free `ErrorKind` variants, keep the 32-byte `Error` assertion, and use `Location::Field`.
- **Item 5 is a finding in `DescriptorSet::validate()`, not a rejection.** `SequenceWriter`, `write_sequence` and `write_parsed_sequence` must keep round-tripping foreign files byte-exactly. No write-time rejection for foreign files.
- **No golden changes:** golden fixtures and `DIFF-LEDGER.md` stay byte-identical. CONF-06 and CONF-05 must still pass.
- **Leave alone:** foreign working-tree changes and the `docs/` clones.

### Claude's Discretion
- Finding and error names, and the test layout.
- Whether items 1 and 2 also get parse-side findings.

### Deferred Ideas (OUT OF SCOPE)
- Item 6: Binaural rendering abort in `decoder_main`.
</user_constraints>

## Summary

1. **v1.1.0 hands Simple and Base to the errata, and the errata scope differs by limit.** The element limits are **sequence-wide**: "only one unique Audio Element OBU" and "at most two unique". The Simple and Base **channel** limits stay **per Mix Presentation**. The Base-Enhanced **channel** ceiling is **sequence-wide** (28). The Base-Enhanced **element** ceiling is per Mix Presentation (28). CONTEXT's phrase "element and channel limits across the whole sequence" is only half right: the Simple/Base channel limits are per presentation.
2. **Both references are per-presentation, and neither enforces any Base combination rule.** So the spec is stricter on four points: the sequence-wide unique-element counts, the sequence-wide Base-Enhanced channel total, at most one scene-based element under Base, and at most one multi-layer channel-based element under Base. The references are stricter, or equal, on `num_sub_mixes` (both reject it outright), on reserved headphones modes (equal), and on channel counting for scene-based elements (the crate already uses libiamf's larger `output_channel_count`).
3. **Gaps 1 and 2 are confirmed on a scratch copy.** `build()` accepts two sub-mixes and returns `Ok(Base)`. It also accepts **zero** sub-mixes and returns `Ok(Simple)`. It accepts headphones modes `Reserved(2)` and `Reserved(3)` and returns `Ok(Simple)`. All of these are reachable through the public API, because `MixPresentation.sub_mixes` and `RenderingConfig.headphones_rendering_mode` are public fields.
4. **The prototype is spec-scoped selection plus two rejections plus the item-5 finding.** Against the full suite it breaks exactly two tests, both expected. Golden, `parse_reference` hashes, the Parallax delivery (already Base-Enhanced), `tests/profile.rs` and every conformance test stay green. Files whose profile bytes change under the new rule decode 1 frame / 1 temporal unit with pinned `iamfdec` and `decoder_main`.

**Primary recommendation:** split `select_minimum_profile` into a per-presentation floor and a sequence floor over slice inputs. Add `num_sub_mixes != 1` and reserved-headphones rejections in `validate_declarations` **before** `validate_findings(presentation.validate())`. Put the item-5 compliance finding in `DescriptorSet::validate()`, reusing the per-presentation floor.

## A. v1.1.0 profile text (VERIFIED: `git -C docs/iamf show v1.1.0:index.bs`)

**Common restrictions, all profiles** (`v1.1.0:1908-1929`, verbatim excerpts):
- `:1912` "There SHALL be only one unique [=Codec Config OBU=]." (already enforced)
- `:1920` "[=num_sub_mixes=] SHOULD be set to 1. [=Mix Presentation OBU=]s with [=num_sub_mixes=] > 1 SHOULD be ignored."
- `:1921` "[=num_audio_elements=] SHOULD be set to at most 28. [=Mix Presentation OBU=]s with [=num_audio_elements=] > 28 SHOULD be ignored."
- `:1927` "The limit on the number of channels, which profiles MAY define, applies to the sum of channels across all [=Audio Element=]s in a [=Mix Presentation=] before mixing."
- `:1929` "There SHALL be at least one [=Mix Presentation OBU=] that complies with the conformance points of the [=primary_profile=] set in the [=IA Sequence=]."
- `:1906` "the meaning of a unique OBU is that it is still unique if it only varies by the [=obu_redundant_copy=] flag."

**(a) Simple and Base defer to the errata.**
- `v1.1.0:1936`: "The simple profile complies with that of <a href=…v1.0.0-errata.html>IAMF specification v1.0.0-errata</a>."
- `:1943` says the same for Base.
- v1.1.0 replaces the errata's *common* section with its own (`:1908-1929`). What it adopts is the errata **Simple and Base sections**:
  - `v1.0.0-errata:1852-1853` "When the [=primary_profile=] field is set to 0, the following constraints apply to the [=IA Sequence=]: - There SHALL be only one unique [=Audio Element OBU=]." Capabilities, `:1857`: "handle up to 16 channels".
  - `v1.0.0-errata:1865-1873` "When the [=primary_profile=] field is set to 1, … There SHALL be at most two unique [=Audio Element OBU=]s. - There SHALL be at most one Channel-based [=Audio Element=] having [=num_layers=] > 1 at any one time. - There SHALL be at most one Scene-based [=Audio Element=] at any one time." It then lists the four allowed pairs: CB1+CB1, CB1+CBn, Scene+CB1, Scene+CBn.
  - `:1878-1879` "handle up to 18 channels. - The 18 channels limit applies to the sum of channels across all [=Audio Element=]s in a [=Mix Presentation=] before mixing."

**Base-Enhanced** (`v1.1.0:1950-1953`):
- "When the [=primary_profile=] field is set to 2 … There SHALL be at least one [=Mix Presentation OBU=] for at most 28 [=Audio Element=]s that parsers complying with the base-enhanced profile recognize."
- "If the [=additional_profile=] is set to 2 and the [=primary_profile=] is set to less than or equal to 2, there SHALL be at most 28 channels in total across all [=Audio Element=]s in the [=IA Sequence=] …"
- Capabilities (`:1958`): 28 channels, 28 elements.

**Headphones mode** (`v1.1.0:1337-1341`): "0: … Stereo. 1: … binaural renderer. 2~3: Reserved for future use. Parsers encountering a reserved value of [=headphones_rendering_mode=] SHALL ignore the [=Mix Presentation OBU=] that contains this [=rendering_config=]."

**Sub-mix count** (`v1.1.0:1278`): "[=num_sub_mixes=] … SHALL NOT be set to 0."

### Rules and their scope

| # | Rule | Scope | Source |
|---|------|-------|--------|
| R1 | Simple: ≤1 unique Audio Element | **sequence** | errata:1853 via v1.1.0:1936 |
| R2 | Simple: ≤16 channels | per MP | errata:1857 + v1.1.0:1927 |
| R3 | Base: ≤2 unique Audio Elements | **sequence** | errata:1866 via v1.1.0:1943 |
| R4 | Base: ≤1 scene-based element | sequence (see A1) | errata:1868 |
| R5 | Base: ≤1 channel-based element with `num_layers > 1` | sequence (see A1) | errata:1867 |
| R6 | Base: ≤18 channels | per MP | errata:1878-1879 |
| R7 | BE: ≤28 elements | per MP ("at least one MP") | v1.1.0:1951, :1921 |
| R8 | BE: ≤28 channels | **sequence** (`additional_profile = 2`) | v1.1.0:1953 |
| R9 | `num_sub_mixes` = 1 (0 SHALL NOT; >1 SHOULD be ignored) | per MP, all profiles | v1.1.0:1278, :1920 |
| R10 | `headphones_rendering_mode` ∈ {0, 1} | per MP, all profiles | v1.1.0:1339-1341 |
| R11 | ≥1 MP complies with `primary_profile` | sequence | v1.1.0:1929 |

**(b) Channel counting.** The spec does not define how channels are counted for scene-based or scalable elements (INFERRED: no definition found in the Profiles section). The crate counts the highest layer's layout for channel-based elements and `output_channel_count` for scene-based ones, which matches libiamf (see B).

**(g) Other profile points.**
- There are no codec-type or parameter-type profile rules (`v1.1.0:1896` NOTE: profiles constrain "how many codecs … but … not … the actual codec").
- The layout-15 floor is already implemented (p28).
- Out of scope: the reserved-OBU placement rules (`:1900-1902`) and loudness.

## B. Reference comparison

**iamf-tools@848c6ff4 `iamf/cli/profile_filter.cc`**, all per Mix Presentation:
- `:35-41` constants 1/2/28 elements, 16/18/28 channels.
- `:220-238` `num_sub_mixes > 1` erases all profiles.
- `:240-271` `Reserved2`/`Reserved3` erase all profiles.
- `:273-280` channels = the sum of substream labels (coded channels).
- `:282-311` counts references over **all sub-mixes of one MP**.
- `:313-362` element/channel tiers.
- There is no scene-based or `num_layers` rule anywhere in the file.

`obu_sequencer_base.cc:306-313` filters **every** MP against `{primary, additional}` before writing it, so every MP must pass the header's profile.

`iamf/obu/mix_presentation.cc:195-197` `ValidateNumSubMixes` rejects `num_sub_mixes == 0` on read and write (`:495`, `:549`).

**libiamf@f06e919e:**
- `IAMF_OBU.c:760-782`: `num_sub_mixes == 0` or `> IAMF_MIX_PRESENTATION_MAX_SUBS` (=1, `IAMF_types.h:168`) fails the Mix Presentation parse.
- `IAMF_decoder.c:628-632` `_profile_limit`: `{1,16,16}`, `{2,18,16}`, `{28,28,25}`.
- `:1284-1337` `iamf_database_mix_presentation_is_valid`, per MP, first sub-mix only:
  - element count check;
  - `headphones_rendering_mode > 1` invalidates the MP;
  - channels = the last layer's layout channels, or `output_channel_count` for scene-based elements.
- `:1402-1410` effective profile = min(`additional_profile`, Base-Enhanced).
- No scene-based or multi-layer combination rule.

| Rule | iamf-tools | libiamf | Class | Crate must implement |
|------|-----------|---------|-------|----------------------|
| R1/R3 element counts | per MP | per MP | **spec stricter** (sequence) | sequence-wide unique count **and** per-MP reference count |
| R2/R6 channels | per MP (coded) | per MP (output) | equal scope; libiamf count ≥ iamf-tools | per MP, libiamf count (current) |
| R4/R5 Base combos | none | none | **spec stricter** | raise to BE when exceeded |
| R7 BE elements | per MP | per MP | equal | per MP (current); >28 unique also caught by R8 |
| R8 BE channels | per MP | per MP | **spec stricter** (sequence) | sequence-wide total ≤28, else error |
| R9 sub-mixes | >1 rejected; 0 rejected on read/write | ≠1 fails parse | **reference stricter** (>1 is SHOULD in spec) | `build()` rejects `!= 1` |
| R10 headphones | reserved rejected | reserved drops MP | equal | `build()` rejects `Reserved(_)` |
| R11 compliance | every MP must pass header (write) | — | reference stricter on write | encoder already satisfies it; `DescriptorSet::validate()` finding |

## C. Crate state (VERIFIED: read this session, and probed on a scratch copy)

- `src/model/profile.rs:115-125` quoted:
  - `pub const SIMPLE_MAX_AUDIO_ELEMENTS: usize = 1;`
  - `BASE_MAX_AUDIO_ELEMENTS: usize = 2;`
  - `BASE_ENHANCED_MAX_AUDIO_ELEMENTS: usize = 28;`
  - `SIMPLE_MAX_CHANNELS: u32 = 16;`
  - `BASE_MAX_CHANNELS: u32 = 18;`
  - `BASE_ENHANCED_MAX_CHANNELS: u32 = 28;`
- `select_minimum_profile(elements: &[&AudioElement]) -> Result<(Profile, Profile)>` (`:154-215`):
  - It checks both 28-ceilings, then the expanded first-layer floor, then the tiers. It always returns an equal pair.
  - Doc at `:130`: "**Both counts are per Mix Presentation, not per sequence.**" This must be rewritten.
  - Channel count (`:252-274`): the last layer's `channel_count()`, or `output_channel_count()`; `UnsupportedLayout` for reserved values.
- `src/encoder.rs:1235-1268` `select_sequence_profile(presentations, elements)`: runs one `select_minimum_profile` per MP over sub-mix references (indexing `elements.get(id)`) and takes the max. It is called at `:932-933`.
- `validate_declarations` (`:947-1148`):
  - `:1064-1080` checks reference counts, then `validate_findings(declaration.presentation.validate())?`, which maps **any** finding to `InvalidDescriptorReference` at `Field("descriptors")` (`:1276-1285`).
  - There is no sub-mix-count or headphones check.
  - `validate_element_topology` (`:1367-1432`) requires exactly one layer (`let [layer] = layers.as_slice() else { return Err(invalid("num_layers")) }`) and only `AmbisonicsConfig::Mono` scene-based elements. **R5 is therefore unreachable through the builder**; it stays reachable through `select_minimum_profile` and parsed sets.
- `src/obu/mix_presentation.rs:35-42`: `pub enum HeadphonesRenderingMode { Stereo, Binaural, Reserved(u8) }`. `MixPresentation.sub_mixes: Vec<SubMix>` is a public field (`:364`). `MixPresentation::validate` (`:378-424`) has no sub-mix-count or headphones finding.
- `src/model/mod.rs:122-199` `DescriptorSet::validate()`: per-descriptor findings, duplicate ids, nested `parameter_id` duplicates, and unresolved references. There is no profile finding. `Finding { at: Location, message: String }` (`src/error.rs:262-267`).
- `src/sequence.rs:195` `ParsedSequence::validate()` is a **separate** validator. It does not call `DescriptorSet::validate()`, and there is no ParsedSequence→DescriptorSet conversion. `tests/parse_reference.rs:95` hashes `ParsedSequence::validate()` output into `semantic_sha256`.
- `src/error.rs`: `ProfileNotFound` (`:84-85`, "no profile permits this configuration") is used only in `tests/error_shape.rs:97`. `ElementCountExceedsProfile` and `ChannelCountExceedsProfile` are at `:114-117`. `const _: () = assert!(size_of::<Error>() <= 32);` is at `:258`.
- `src/obu/sequence_header.rs:55-87` header `validate()` already reports `primary_profile >= PROFILE_COUNT` (3) and `additional < primary`.

**Scratch probe (`scratchpad/qk3/repo/tests/qk3_probe.rs`, unmodified crate) — VERIFIED output:**
```
PROBE two_sub_mixes: Ok((Base, 2))
PROBE zero_sub_mixes: Ok(Simple)
PROBE headphones Reserved(2): Ok(Simple)
PROBE headphones Reserved(3): Ok(Simple)
PROBE headphones Reserved(4): Err(Error { kind: ValueExceedsWidth { bits: 2 }, at: OutputOffset(19) })
PROBE unreferenced element: Ok((Simple, 2))     # 2 unique AEs under a Simple header
PROBE no presentation: Ok((Simple, 0))
PROBE duplicate element in sub-mix: Ok(Base)
```
Gaps 1 and 2 are confirmed. Three more builder gaps were found; see F (deferred).

## D. Design

### Items 1+2: `build()` rejections (in `validate_declarations`, placed **before** `validate_findings(declaration.presentation.validate())?` at `encoder.rs:1080`)
- New payload-free variants in `src/error.rs`:
  - `SubMixCountNotOne` — `#[error("a Mix Presentation must carry exactly one sub-mix")]`, `Location::Field("num_sub_mixes")`. Covers 0 as well as >1.
  - `ReservedHeadphonesRenderingMode` — `#[error("headphones_rendering_mode is reserved")]`, `Location::Field("rendering_config.headphones_rendering_mode")`. Rejects any `Reserved(_)` (2 and 3; 4+ already fails the width check later), for **every** element type: the spec ignores the field for non-channel-based elements but still drops the whole MP (`:1341`).
- Payload-free variants keep the 32-byte assertion.
- Citations:
  - `// ref: IAMF v1.1.0 index.bs:1278, :1920`
  - `// ref: iamf-tools@v2.1.0 iamf/cli/profile_filter.cc:220-238 FilterProfileForNumSubmixes`
  - `// ref: iamf-tools@v2.1.0 iamf/obu/mix_presentation.cc:195-197 ValidateNumSubMixes`
  - `// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c:760-782`
  - `// DISAGREEMENT: spec says >1 SHOULD be ignored; both references reject, so the reference rule stays.`
  - For headphones: `index.bs:1337-1341`, `profile_filter.cc:240-271`, `IAMF_decoder.c:1309-1315`.
- **Do not** add these as `MixPresentation::validate()` findings. That would (a) make `validate_findings` pre-empt the dedicated kinds, and (b) change `test_000124.iamf`'s `semantic_sha256` in `tests/support/reference_expectations.rs:172`, because it carries `num_sub_mixes: 2` under a Base header (`test_000124.textproto:111`, `is_valid: false`). Item 5 covers these shapes on the model side.

### Items 3+4: spec-scoped selection (`src/model/profile.rs`)
The shared core uses **slice inputs**, so both `DescriptorSet` and (later) `ParsedSequence` can call it:
```rust
// presentation floor: one MP's resolved references (duplicates counted, as iamf-tools does)
fn presentation_floor(elements: &[&AudioElement]) -> Result<Profile>
  // >28 refs -> ElementCountExceedsProfile; >28 ch (libiamf count) -> ChannelCountExceedsProfile
  // expanded first layer -> BE; ≤1 & ≤16ch -> Simple; ≤2 & ≤18ch & base_combination_allowed -> Base; else BE
// sequence floor: every unique Audio Element (distinct audio_element_id, first binding)
fn sequence_floor(elements: &[&AudioElement]) -> Result<Profile>
  // >28 unique -> ElementCountExceedsProfile (implied by R8); total ch >28 -> ChannelCountExceedsProfile (R8)
  // expanded -> BE; ≤1 -> Simple; ≤2 & base_combination_allowed -> Base; else BE   (no Simple/Base channel test: R2/R6 are per MP)
fn base_combination_allowed(e) = scene_based_count ≤ 1 && channel_based_with_layers_len>1_count ≤ 1
pub fn select_minimum_profile(elements) = max(presentation_floor(e), sequence_floor(e)), equal pair   // signature unchanged
pub fn presentation_minimum_profile(presentation: &MixPresentation, elements: &[&AudioElement]) -> Result<Profile>
  // sub_mixes.len()!=1 or any Reserved headphones -> Err(ProfileNotFound); resolve ids first-match; missing -> Err(InvalidDescriptorReference)
pub fn select_sequence_profile(elements: &[&AudioElement], presentations: &[MixPresentation]) -> Result<(Profile, Profile)>
  // sequence_floor(elements) max over presentation_minimum_profile(p, elements)
```
- Error order: sequence ceilings run first, then per-MP ceilings, then the floors. The expanded floor stays after every ceiling (p28 invariant). An over-ceiling configuration is still an error, never Base-Enhanced.
- `encoder.rs` replaces its private `select_sequence_profile` and calls the model function with `descriptors.audio_elements` and `mix_presentations`. Ids equal indices after lowering, so first-match lookup gives the same result.
- Citations:
  - `// ref: IAMF v1.0.0-errata index.bs:1852-1853, :1865-1873 (adopted by v1.1.0 index.bs:1936, :1943)`
  - `// ref: IAMF v1.1.0 index.bs:1927, :1951, :1953`
  - `// DISAGREEMENT: iamf-tools@v2.1.0 profile_filter.cc:282-362 and libiamf@v1.1.0 IAMF_decoder.c:1284-1337 count per Mix Presentation and have no scene-based/num_layers rule; the spec is stricter, so the spec wins (qk3 directive).`
- Rewrite the doc at `profile.rs:130-134` and the module doc for `tests/profile.rs:16`.

**Prototype:** `scratchpad/qk3/prototype-src.diff`, 215 lines. It uses `ProfileNotFound`/`ReservedValue` as placeholders; use the new variants instead.

### Item 5: `DescriptorSet::validate()` finding
- After the reference checks, let `primary = Profile::from_wire(self.sequence_header.primary_profile)`.
- If the profile is not `Reserved(_)` (the header finding already covers ≥3) and no MP satisfies `presentation_minimum_profile(mp, &elements).is_ok_and(|p| p <= primary)`, push:
  `Finding { at: Location::Field("primary_profile"), message: "no Mix Presentation complies with primary_profile {n} (IAMF v1.1.0 index.bs:1929)" }`
- "Complies" reuses the per-MP floor: the MP's own counts, channels, expanded floor and Base combination, plus `num_sub_mixes == 1` and no reserved headphones mode.
- This is not a rejection: the writers are untouched.
- **Recommended in the same task (discretion):** a second finding when `sequence_floor(elements)` exceeds `primary`, e.g. a Simple header over 2 unique elements, which violates `errata:1853`. It uses the same `Field("primary_profile")` and a different message, and it is cheap because it reuses the same function.

## E. Test impact (VERIFIED: prototype on the scratch copy, `cargo test --no-fail-fast`, docker stubbed)

Only 2 of ~500 tests fail, both expected:
- `tests/encoder_builder.rs:185` `unrelated_presentations_do_not_sum_their_element_or_channel_limits` now gets `Err(ChannelCountExceedsProfile at Field("loudspeaker_layout"))`. Two 16-channel TOA elements make 32 channels sequence-wide (R8), and two scene-based elements rule out Base (R4). **Rename** it to e.g. `two_presentations_sum_channels_against_the_sequence_wide_base_enhanced_ceiling` and assert that error. Keep `two_presentation_builder` (`:1156`) as its fixture.
- `tests/encoder_builder.rs:198` `shared_element_is_counted_in_each_presentation_that_references_it` now gets `BaseEnhanced` instead of `Base`: 3 unique elements (R3). Rename it and assert BE. Add a companion test where a shared element plus one other gives 2 unique elements → **Base**, which proves distinct counting.

Unaffected, all green:
- `tests/profile.rs` (34 tests)
- `golden.rs`
- `parse_reference.rs` (no hash change: `ParsedSequence::validate` untouched)
- `parallax_contract.rs`, `conformance.rs`, `descriptors.rs`, `fixture.rs`, `sequence_parse.rs`
- the `encoder_builder.rs:1077` `descriptors().validate().is_empty()` check

**Parallax delivery:** its primary MP has 4 elements and 3×2+4 = 10 channels, so it is already Base-Enhanced. Sequence-wide it is still 4 unique elements and 10 ≤ 28 channels, so its **profile bytes do not change**. No committed fixture's bytes change.

**Pinned-decoder check of the new selections** (scratch files, 16 kHz LPCM, 1 temporal unit), VERIFIED:
| File | Old → new profile | `iamfdec -s0` | `decoder_main` |
|------|------|------|------|
| two presentations, one stereo each | Simple → Base (`01 01`) | exit 0, "Get 1 frames", 556-byte WAV | "Decoded 1 temporal units." |
| 3 unique stereo, shared across 2 MPs | Base → BE (`02 02`) | exit 0, "Get 1 frames", 556-byte WAV | "Decoded 1 temporal units." |

**New tests:**
- **Rejections** (`tests/encoder_builder.rs`):
  - two sub-mixes → `SubMixCountNotOne`/`Field("num_sub_mixes")`
  - zero sub-mixes → the same error
  - `Reserved(2)` and `Reserved(3)` → `ReservedHeadphonesRenderingMode`
  - `Binaural` still builds
- **Selections:**
  - two one-stereo presentations → Base (`01 01`)
  - two FOA mono elements in separate MPs → BE (R4)
  - two FOA in one MP → BE (was Base)
  - FOA + stereo → Base
- **Direct `select_minimum_profile` in `tests/profile.rs` (R5, not builder-reachable):**
  - two 2-layer channel-based elements → BE
  - one 2-layer + one FOA → Base
  - an unreferenced second element through the builder → Base
- **Finding** (`tests/descriptors.rs` or `sequence_parse.rs`):
  - Simple header + 2-element MP → finding
  - same set with a Base header → none
  - two MPs where one complies → none
  - a single MP with two sub-mixes → finding
  - Simple header + expanded element → finding
  - `primary_profile = 3` → only the existing header finding
- **Doc rows:** update `error_shape`, if the inventory is listed anywhere.

## F. Recommendation and deferred items

**Recommendation:** implement D as three commits.
1. Rejections for R9/R10 plus tests. These are TDD: the probes above are the failing tests.
2. Spec-scoped selection plus the renamed tests plus the R4/R5 tests.
3. The item-5 finding plus tests.

Then run full `cargo test`, clippy (`indexing_slicing`, `arithmetic_side_effects` are deny: use `.count()`, `.max()`, `checked_add`), CONF-06 with docker, and CONF-05 with `IAMF_REF_DECODER=…/iamfdec`. Golden and `DIFF-LEDGER.md` must stay unchanged.

**Deferred (with reasons):**
1. **`build()` accepts zero Mix Presentations** (`Ok((Simple, 0))`). A sequence needs ≥1 MP (`v1.1.0:1902`, `:1929`). After item 5, `descriptors().validate()` on such output will truthfully report the compliance finding. Rejecting it is a separate `build()` rule.
2. **`build()` accepts the same element twice in one sub-mix** (`Ok(Base)`), which violates `v1.1.0:1280` "no duplicate values of audio_element_id within one Mix Presentation". It is out of this task's profile scope.
3. **`ParsedSequence::validate()` compliance finding.** This is the true parse-side validator. Adding it changes `test_000124.iamf`'s `semantic_sha256` (INFERRED: its only MP has 2 sub-mixes). It needs a separate expectation update, and redundant-copy filtering, because ParsedSequence keeps `obu_redundant_copy` Audio Elements. The slice-based API in D makes it a small follow-up.
4. **Separate R9/R10 findings in `MixPresentation::validate()`.** See D: pre-emption and the `test_000124` hash.
5. Item 6 (Binaural `decoder_main` abort): out of scope per CONTEXT.
6. Unmodified reminder: `docs/IAMF-V1.1-COMPLETENESS-AUDIT.md` is foreign and must not be edited.

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | Base's "at any one time" (errata:1867-1868) is read sequence-wide. It sits under "constraints apply to the [=IA Sequence=]" next to "at most two unique", and the stricter reading wins under the directive. | A, D | If the user means per MP, two scene-based elements in *separate* MPs would be Base rather than BE. Both decode either way (BE ⊇ Base); only the header bytes differ. |
| A2 | Unique elements are counted as distinct `audio_element_id`s (first binding), including elements no MP references. | D | Builder ids are unique, so this is only relevant for foreign sets carrying duplicate ids (already a finding). |
| A3 | Checking >28 unique elements sequence-wide as `ElementCountExceedsProfile`, before the R8 channel check, is a design choice. R8 alone implies it. | D | Only the error kind for >28 mono elements spread across MPs changes. |
| A4 | Error variant names `SubMixCountNotOne` and `ReservedHeadphonesRenderingMode`. | D | Naming only (discretion). |

## Open Questions
1. A1 wording: confirm the sequence-wide reading at plan-review. The recommendation is sequence-wide.
2. Should item 5 also get the sequence-scope finding ("Simple header, 2 unique elements")? The recommendation is yes, as a second message.

## Validation Architecture
- Framework: `cargo test` (stable, edition 2024). Quick run: `cargo test --test encoder_builder --test profile --test descriptors`. Full suite: `cargo test`.
- CONF-06: `PATH=/Applications/Docker.app/Contents/Resources/bin:$PATH cargo test --test conformance`. Wrap docker probes in `perl -e 'alarm N; exec @ARGV'`.
- CONF-05: `IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec cargo test --test conformance`.
- Wave 0: none. All test files exist; the new tests are added to `tests/encoder_builder.rs`, `tests/profile.rs` and `tests/descriptors.rs`.

## Security Domain
- V5 Input Validation applies: the new selection must use checked sums (the existing `total_channel_count` uses `checked_add`), and the `by_id`-style lookups must stay linear over bounded, non-index access (`indexing_slicing` deny).
- The new finding path runs on foreign descriptor sets: it must never panic on a missing element or a reserved type. Return `Err` and treat the MP as non-compliant.

## Environment Availability
| Dependency | Available | Note |
|---|---|---|
| `iamf-tools:v2.1.0` docker image | ✓ `sha256:4a011a6d…` | `decoder_main` ran this session |
| libiamf `iamfdec` | ✓ `.reference/libiamf/code/test/tools/iamfdec/iamfdec` | ran this session |

## Sources
- IAMF spec `docs/iamf` tags `v1.1.0` (`e131550`) `index.bs:1278, 1337-1341, 1892-1962` and `v1.0.0-errata` `index.bs:1811-1882`.
- `iamf-tools@848c6ff4968ff8cc6f728259892ab4f90cb83256` (= tag v2.1.0): `iamf/cli/profile_filter.cc` (full), `iamf/cli/obu_sequencer_base.cc:302-318`, `iamf/cli/obu_processor.cc:198-219`, `iamf/obu/mix_presentation.cc:195-197,491-549`, `iamf/api/decoder/iamf_decoder.h:60`.
- `libiamf@f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63` (= tag v1.1.0): `code/src/iamf_dec/IAMF_decoder.c:621-668, 1284-1361, 1402-1410`, `IAMF_OBU.c:750-790`, `IAMF_types.h:143,168`.
- Crate: `src/model/profile.rs`, `src/model/mod.rs`, `src/encoder.rs:770-1432`, `src/obu/mix_presentation.rs:1-434`, `src/obu/sequence_header.rs`, `src/error.rs`, `src/sequence.rs:195-330`, `tests/encoder_builder.rs`, `tests/parse_reference.rs`, `tests/support/parallax_contract.rs`, `tests/fixtures/reference/*.textproto`.
- Scratch evidence: `/private/tmp/claude-501/-Users-cell-local-iamf-rs/ffcacbe6-b061-4445-a28e-fe0219356ad0/scratchpad/qk3/` (`repo/tests/qk3_probe.rs`, `prototype-src.diff`, `out/*.log`).

**Valid until:** the pins or `src/model/profile.rs` change.
