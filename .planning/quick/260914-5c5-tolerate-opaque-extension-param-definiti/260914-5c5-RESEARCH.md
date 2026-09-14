# Quick 260914-5c5: Tolerate opaque extension param definitions; reject non-round-tripping writer states - Research

**Researched:** 2026-09-14
**Domain:** `ParamDefinitionRegistry` (`src/obu/param_definition.rs`), sequence parse/write (`src/sequence.rs`), low-level OBU writers
**Confidence:** HIGH. I read both references at their pinned commits and the spec at `v1.1.0`. A probe crate in the scratchpad confirmed every Part B state against HEAD. A scratch copy of HEAD with the Part A fix passed `parse_reference` (all 37 `semantic_sha256` unchanged), `golden`, and `clippy -D warnings`. The failures it produced are listed below; all of them are expected.

<user_constraints>
## User Constraints (binding, from `.planning/.continue-here.md` BLOCKING CONSTRAINTS and `260913-qk3-CONTEXT.md`)

- **Conformance rule:** satisfy BOTH the IAMF v1.1.0 spec AND the pinned references (`iamf-tools` v2.1.0 `848c6ff`, `libiamf` v1.1.0 `f06e919`). Where either one is stricter, keep the stricter rule. Record every spec/reference disagreement in a `// ref:` comment.
- **Reference hashes:** a `semantic_sha256` in `tests/support/reference_expectations.rs` may change ONLY for a fixture the pinned testdata marks `is_valid: false`. Predict it; any other change means stop and report.
- **Pinned trees only** (`git show <pin>:<path>`). **Staging discipline:** explicit paths only.
- Prior decision "low-level writers byte-exact" and Phase 02 "Bounded ungoverned Parameter Blocks preserve raw bytes and validate missing context" (`STATE.md:119`).

No `CONTEXT.md` exists for this quick task.
</user_constraints>

## Project Constraints (from CLAUDE.md)

- Clippy denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap/expect/panic` in `src/`. `clippy.toml` exempts only unwrap/expect/panic in tests, so **test code must still use `.get()`**. Toolchain 1.85.0 has no let-chains.
- Every `read_*`/`write_*` needs a `// ref:` line. Existing `ErrorKind` variants may not change; adding one is fine. `size_of::<Error>() <= 32` (the probe measured exactly 32 today: `ErrorKind` = 2 bytes, `Location` = 24). **Only add unit or `u8`-payload variants.**
- Never silently normalise. Golden bytes stay. Linked graph stays `iamf` + `thiserror`. There are no new packages, so no legitimacy audit is needed.

## Summary

**Part A.** For `param_definition_type > 2`, the spec gives only `param_definition_size` plus opaque `param_definition_bytes` that "Parsers SHOULD ignore" (`index.bs:686-689`, `:772`, `:796`). Neither reference reads a `ParamDefinition` prefix from those bytes, and neither registers them:
- `iamf-tools` `ExtendedParamDefinition` is documented as "reserved for future use; should be ignored". It "does not read the base class's data".
- `CollectAndValidateParamDefinitions` logs "Ignoring parameter definition of type=" and `continue`s.
- A Parameter Block with no definition is skipped as a "stray".
- `libiamf` does `bs_skipABytes` and `continue`.

The crate's prefix registration was a Phase 02 invention with no source behind it. It fails in three ways:
1. A legal empty or short extension aborts parse, write, `from_parts`, `SequenceWriter` and builder paths.
2. A legal extension whose opaque bytes happen to decode **shadows** a real Mix Gain id, so a valid file fails. I confirmed this on HEAD.
3. It silently suppresses the nested-duplicate findings in `DescriptorSet::validate`.

**Recommend (b): never register extension definitions.** Blocks whose id is known only from extension bytes become `UngovernedParameterBlock`, which already exists, preserves raw bytes and round-trips.

**Part B.** Six of the review's states are confirmed by the probe, plus one extra alias it missed (`AmbisonicsConfig::Reserved { mode: 0|1 }`). One claim is refuted: `Extension { type: 0 }` round-trips, so only types 1 and 2 break. No reader ever produces any of these states, so rejecting them on write cannot affect a parsed fixture, the golden file, the corpus or the fuzz generator.

**Primary recommendation:** 2 plan tasks.
- **T1 (Part A):** replace the Extension arm in the registry with a no-op. Update 4 test sites and the fuzz/proptest generator.
- **T2 (Part B):** add writer guards backed by two new unit `ErrorKind` variants, plus red unit tests and one "write Ok ⇒ re-read equal" proptest file.
- Predicted hash impact: **none**. I verified this for all 37 positives.

## Part A - Evidence

### What the sources say

| Source (pinned) | Behaviour for `param_definition_type >= 3` |
|---|---|
| Spec `index.bs:686-689` | `leb128() param_definition_size; unsigned int (8 x param_definition_size) param_definition_bytes;` [VERIFIED: git show v1.1.0:index.bs] |
| Spec `index.bs:772` | "An OBU parser SHALL be able to parse [=param_definition_type=] = P (where P > 2) and [=param_definition_size=]. The OBU parser SHOULD ignore the bytes indicated by [=param_definition_size=] that it does not recognize." |
| Spec `index.bs:796` | "param_definition_bytes represents reserved bytes for future use ... Parsers SHOULD ignore these bytes when they don't understand the parameter definition." |
| Spec `index.bs:1585`, `:1591` | If nothing refers to a `parameter_id`, "parsers SHOULD ignore [=Parameter Block OBU=]s with this identifier". Parsers "SHOULD ignore the Parameter Block OBU with a param_definition_type that they don't recognize." |
| iamf-tools `iamf/obu/param_definitions.h:292-293` | `/* !\brief Parameter definition reserved for future use; should be ignored. */ class ExtendedParamDefinition` |
| iamf-tools `iamf/obu/param_definitions.cc:335-344` | `// This class does not read the base class's data, i.e. it doesn't call ParamDefinition::ReadAndWrite(wb)`. Reads size and bytes only, so `parameter_id_` is never read. |
| iamf-tools `iamf/cli/cli_util.cc:130-135` | `default: ABSL_LOG(WARNING) << "Ignoring parameter definition of type= " ...; continue;`. Not inserted into the definition map. The encoder uses the same collector (`iamf_encoder.cc:323`). |
| iamf-tools `iamf/cli/obu_processor.cc:164-167` | "Found a stray parameter block OBU (no matching parameter definition)." `// The spec prefers skipping over unknown parameter blocks.` `return read_bit_buffer.IgnoreBytes(payload_size);` |
| libiamf `code/src/iamf_dec/IAMF_OBU.c:475-483` | `else { uint64_t size = bs_getAleb128(&b); bs_skipABytes(&b, size); ia_logw("Don't support parameter type ..."); continue; }`. No parameter object is created. |
| libiamf `IAMF_OBU.c:1093-1099` (`iamf_parameter_new`) | `if (!objParam \|\| !objParam->param_base) { ... goto parameter_fail; }`. The block is dropped. That `param_base` comes from the definition lookup is [ASSUMED]. |

All three sources agree, so there is no disagreement to record. Registering an extension definition is stricter than every source, and it is wrong.

### Current crate behaviour (HEAD, read this session)

- `param_definition.rs:119-129`: `let definition = read_param_definition(&mut reader)?;` on the opaque `bytes`, then `register(definition, ParameterDataContext::Reserved(*param_definition_type))`. The doc at `:94-96` wrongly claims "Unknown definition types begin with the same shared prefix".
- Every caller propagates or swallows that error:
  - `sequence.rs:124` `from_parts`
  - `:555` `parse_sequence`
  - `:652` `write_parsed_sequence`
  - `:837` `SequenceWriter`
  - `encoder.rs:1310` (build)
  - `encoder.rs:401` (push)
  - `sequence.rs:498` (`validate`: `if observe(registry).is_err() { return; }`)
  - `model/mod.rs:147` (`if let Ok(registry)`: **all nested duplicate-id findings silently vanish**)
- Ungoverned blocks: parse routes on `registry.get(id).is_some()` (`sequence.rs:575-593`). Otherwise the whole payload is kept as `UngovernedParameterBlock`, written verbatim (`:674-676`), and `validate()` pushes "parameter block references parameter_id {id}, which no definition in this sequence carries" (`:346-356`).

### Probe results (HEAD, scratch crate `probe/`)

- `Extension{7, []}` in test_000002's Audio Element:
  - `parse_sequence` gives `Err(UnexpectedEndOfInput @ InputOffset(28))`
  - `write_parsed_sequence` gives `Err(UnexpectedEndOfInput @ InputOffset(0))`
  - `from_descriptors` gives `Err(... @ InputOffset(1))`
- **Shadowing (conformance bug):** start from test_000121 (`is_valid: true`, extension bytes `"ignored"`). Add a Mix Gain block for id 100, which is legal and re-parses equal. Patch `'i'` to `0x64` so the opaque bytes now decode to `parameter_id` 100. `parse_sequence` then gives `Err(UnexpectedEndOfInput @ 132)`: the block binds to the extension, which is first in wire order, and is misparsed as Raw data. With fix (b) in the scratch copy, the same bytes parse and the block is `MixGain(Step{256})`.

### Decision: (b) never register

- **(a) best-effort prefix decode** is rejected. It still decodes opaque bytes, so it keeps the shadowing false rejection, and it produces spurious duplicate-id findings. Whether a file parses would depend on the content of bytes the spec calls opaque.
- **(c) register + Finding** is rejected. It has the same shadowing problem. A new Finding would also change test_000121's `semantic_sha256`, because the hash includes `validate()` output (`tests/parse_reference.rs:91-99`) and the fixture is `is_valid: true`. The constraint forbids that.
- **(b)** matches the spec (SHOULD ignore) and both references, and it closes all three failure modes. It also removes the parse-time route to `ParameterData::Raw`. The `.continue-here.md` memory residual says "38 MB → 1.94 GB RSS still reachable via `param_definition_type >= 3`", and that route is now 1× raw bytes. Deriving the exact RSS change is out of scope; this is stated by reading.
- **No new Finding for extension definitions.** Refs only log a warning, and a Finding would break the hash constraint. A block whose id appears only in extension bytes keeps the existing ungoverned-block finding. That behaviour is unchanged and true, because no *recognised* definition carries the id.

### Offset bug (Q4)

The error comes from a fresh `BitCursor::new(bytes)` at `param_definition.rs:124`, so its offset is relative to the extension slice. `sequence.rs:556` then does `.with_input_base(payload_base)`. The reported offset is therefore the Audio Element **payload start**, not the extension bytes. The probe showed payload_base = 28 and error offset = 28. The review's 26 is the same effect on its own input. On the write path it is even an `InputOffset`. After (b) there is no error path at all, so the bug is **moot**. No separate fix is needed.

### T1 integration points and required test updates

1. `param_definition.rs:119-129`: make the arm `AudioElementParam::Extension { .. } => {}`, with `// ref:` lines citing `cli_util.cc CollectAndValidateParamDefinitions`, `param_definitions.h ExtendedParamDefinition`, and `libiamf IAMF_OBU.c iamf_element_new`.
   - Rewrite the doc at `:94-96`.
   - **Keep the `-> Result<()>` signatures** of `observe_audio_element` and `from_descriptors`. They are public, and I could not check Parallax usage because `/Users/cell/local/Parallax` is absent here.
   - Document that the result is currently always `Ok`.
2. `sequence.rs:118-121`: remove the "# Errors ... extension parameter definition" doc, or reword it.
3. The scratch copy with only change 1 applied gives this exact failing set (nothing else fails; clippy is clean):
   - `tests/sequence_parse.rs:192` `an_extension_definition_registers_its_shared_prefix_and_reserved_context`: **replace** with the inverse test (red test 2 below).
   - `tests/encoder_builder.rs:141-159` now gets `UnsupportedParameterData` (from `encoder.rs:1317-1322`) instead of `UnexpectedEndOfInput @0`. Rename it to "build rejects audio element params" and assert `UnsupportedParameterData` at `Field("audio_element_params")`. The builder never supported params anyway.
   - `tests/round_trip.rs`: 5 tests fail, all from shared `tests/support/sequence_cases.rs`. `rich_case` declares `Extension{7, definition_bytes(id 12)}` (`:136-139`) and `rich_unit` pushes a Raw block id 12 (`:238-245`, `raw_subblock` `:316`), so `SequenceWriter` now returns `NoGoverningParamDefinition`.
     - Fix: drop block 12 from the typed units.
     - To keep raw-byte coverage, insert a `SequenceObu::UngovernedParameterBlock { header: ParameterBlock, payload: [0x0c, …raw bytes] }` into the flat model after `from_parts`.
     - `unknown_obu_and_raw_parameter_data_keep_exact_positions_and_offsets` (`round_trip.rs:180-296`) asserts index 6 as `ParameterBlock` with `ParameterData::Raw`, plus a boundary vector. Re-derive both; the sentinel becomes `UngovernedParameterBlock`.
   - `tests/fuzz_regression.rs`: `roundtrip_replay_covers_every_seed_and_minimized_artifact` and `codec_enriched_fuzz_replay…` fail because the same shape lives in `src/fuzzing.rs:121-124` (Extension), `:223-230` (block 12), `:301-306` (`raw_subblock`). Apply the same generator change.
     - Corpus seed *files* stay byte-identical, since they are generator inputs.
     - Update `fuzz/CORPUS.md:27`, `:42` ("raw extension data", `bounded-raw-parameter-data`) to say "ungoverned raw Parameter Block".
4. `tests/temporal.rs:593-665` registers `ParameterDataContext::Reserved(7)` manually. It is unaffected and keeps the public `Raw` read/write path tested.

## Part B - Writer states (all line numbers re-verified at HEAD)

| # | State / writer location | Probe result (HEAD) | Writer must reject | Reader can produce it? |
|---|---|---|---|---|
| B1 | `LoudnessExtension.info_type_bits`, from `Loudness::info_type` `mix_presentation.rs:193-205` (masks `& 0xfc` at `:202`) and `write_loudness` `:931-956` (ext at `:951-954`) | `0x00`/`0x01`: write Ok, re-read ≠ (bytes land in `trailing=[1,170]`). `0x85`: write Ok, re-read ≠ (low bits dropped) | `bits & 0xfc == 0 \|\| bits & 0x03 != 0`, the same predicate as the builder guard `encoder.rs:1274-1279` | No. Read sets `info_type & 0xfc` only when non-zero (`:906-916`) |
| B2 | `AudioElementParam::Extension` type 1/2, `write_audio_element_param` `audio_element.rs:624-636` | type 1 and 2: re-read `Err(UnexpectedEndOfInput)`. **type 0: re-read equal. type 3: equal** | `param_definition_type == 1 \|\| == 2` only. Keep 0 writable: the reader accepts it as Extension (`:600-608`, like libiamf's `else` skip), `validate()` already flags it (`:442-449`), and byte-exact parse→write must still re-emit it | No. 1 and 2 dispatch to typed arms (`:583-599`) |
| B3 | `AudioElementType::Reserved{value}`, `write_audio_element` `:513-536` (`:515`, `:533`) | value 0: re-read ≠. value 1: re-read Err. value 2: equal. value 8: already `ValueExceedsWidth{bits:3}` | `value == 0 \|\| value == 1`. Leave >7 to the existing width error | No. Only `2..=7` (`:485-494`) |
| B4 | Ambisonics, `write_ambisonics_config` `:808-827` | Mono with `channel_mapping` 1 short: re-read Err. `Reserved{mode:0}`: re-read Err. **Reachable publicly:** `SceneBased(..)` tuple patterns are private, but the struct pattern `AudioElementType::SceneBased { 0: c, .. }` compiled on 1.85.0 and gave `&mut` access to a parsed model | Mono: `channel_mapping.len() != output_channel_count`. Projection: `demixing_matrix.len() != (substream + coupled) * output_channel_count` (checked arithmetic, as in read `:776-786`). `Reserved{mode: 0\|1}` | No (read `:751-806`) |
| B5 | `DurationFields`, `write_param_definition` `param_definition.rs:328-349` (`:333-344`) | `{8, 8, [4,4]}`: write Ok, re-read ≠ | `constant_subblock_duration != 0 && !subblock_durations.is_empty()` | No. The list is empty unless constant == 0 (`:285-305`) |
| B6 | `Layout::Reserved`, `write_layout_with_loudness` `mix_presentation.rs:858-868`, `layout_type()` `:228-234` | 2 and 3: re-read ≠. 4: already `ValueExceedsWidth{bits:2}` | `Layout::Reserved(2 \| 3)` | No. Only 0/1 (`:838-846`) |
| B7 | `ObuType::Reserved`, `value()` `header.rs:62-91` (`:89`), gate `validate_header` `:296-310` (called by `write_obu` `:376`) | 3 and 23: reparse ≠. 31: reparse Err. 24: equal. 32: already `ValueExceedsWidth{bits:5}` | `Reserved(v)` with `v <= 23 \|\| v == 31`. Keep >31 as `ValueExceedsWidth` (documented at `:95-98`) | No. `from_value` gives Reserved only for 24..=30 (`:100-129`) |

**Existing producers (grep + read):**
- `src/fuzzing.rs` and `tests/support/sequence_cases.rs` use `info_type_bits: 0x84`, `ObuType::Reserved(27)` and mode-1 definitions only.
- Tests use `ObuType::Reserved(24..=30)` (`sequence_parse.rs:329,368,538,549`, `obu_header.rs:524`), `info_type_bits: 0x04` (`descriptors.rs:1126`) and `0` (`encoder_builder.rs:1275`, which the builder rejects before any write, at `encoder.rs:1274` versus `:1325`).
- `encoder_builder.rs:1318-1338` expects `is_err()` for `(4, 2, [2,2])`, which still errors.
- Every AmbisonicsMono construction is length-consistent, and the builder already checks the mapping (`encoder.rs:1566`).

**No test, golden file, corpus seed or generator produces a rejected state.** B1-B7 do not conflict with "low-level writers byte-exact": that decision means parsed input re-emits exactly, and no parsed model reaches these guards.

**Error kinds.** No existing variant fits. `ReservedValue{value}` means "reserved field carries non-zero value"; `InvalidDescriptorReference` is builder-scoped; `SubblockDurationMismatch` means sums. Add two **unit** variants (non-breaking, keeps the 32-byte budget):
- `ReservedAliasesDefinedValue`, message "a reserved variant carries a value this spec version defines". Used for B2, B3, B4-`Reserved{mode}`, B6 and B7, with `Location::Field("param_definition_type" | "audio_element_type" | "ambisonics_mode" | "layout_type" | "obu_type")`.
- `GatedFieldMismatch`, message "a stored gate or length disagrees with the data it gates". Used for B1 `Field("info_type")`, B4 `Field("channel_mapping" | "demixing_matrix")` and B5 `Field("subblock_durations")`.

The names are at Claude's discretion. Add `// ref:` lines with the `ValidateAndWrite` counterparts: `iamf-tools` `ExtensionParameterData::Write` `ValidateContainerSizeEqual` is the reference precedent for length checks (`extension_parameter_data.cc:33-40`).

**Scope:** keep all seven in T2. Each is a 3-6 line guard at the top of an existing writer, and all share the two variants and one proptest file. Deferred, same alias class but not in the review, to record in `260914-5c5-deferred-items.md`:
- `LoudspeakerLayout::Reserved`, `ExpandedLoudspeakerLayout::Reserved`, `SoundSystem::Reserved` and `HeadphonesRenderingMode::Reserved` with defined values [ASSUMED alias, not probed].
- The spec gives `param_definition_type == 0` in an Audio Element **no** size field (`index.bs:680-689`). iamf-tools rejects it on read (`audio_element.cc:393-396`), while the crate and libiamf read size + bytes. This is a spec/ref disagreement; a `// ref:` note is enough for now.

## Red tests (fail at HEAD, pass after)

**T1 (new tests in `tests/sequence_parse.rs`):**
1. `opaque_extension_definitions_without_a_param_definition_prefix_round_trip`. Take `published_descriptor_set()` (tests/support/test_000003.rs) with `Extension{7, []}`, `Extension{3, [0x80]}` and `Extension{9, [0x05, 0x01]}`. Build via `ParsedSequence::from_parts`, then `write_parsed_sequence` gives `Ok`, and `parse_sequence(bytes)` equals the model. At HEAD this is `Err(UnexpectedEndOfInput)`.
2. `a_parameter_block_whose_id_only_appears_inside_extension_bytes_is_ungoverned`. Use `Extension{9, [0x07, 0x01, 0x80, 0xaa]}` plus an `UngovernedParameterBlock{payload: [0x07, …]}`. Parse gives `SequenceObu::UngovernedParameterBlock`, bytes round-trip, and `ParamDefinitionRegistry::from_descriptors(..).get(7)` is `None`. At HEAD this is governed `ParameterBlock`.
3. `extension_bytes_that_decode_to_a_mix_gain_id_do_not_shadow_it`. Use the probe construction: 3-byte extension `[0x64, 0x01, 0x80]` plus Mix Gain definition id 100 plus a Mix Gain Step block for id 100. Parse gives `ParameterBlock` with `ParameterData::MixGain`. At HEAD this is an `Err` or `Raw`.
4. `descriptor_validation_keeps_nested_duplicate_findings_beside_an_opaque_extension`. Take `published_descriptor_set()` (which already has duplicate id 100) plus `Extension{7, []}`. `validate()` still contains the `parameter_id 100` finding. At HEAD the finding is suppressed by `model/mod.rs:147`.

**T2 (new file `tests/writer_round_trip.rs`):**
- One `#[test]` per B1-B7 asserting the exact `ErrorKind` and `Field`, plus positive controls: `Extension{0}` and `{3}`, `Reserved{value: 2}`, `ObuType::Reserved(24)`, `info_type_bits 0x84`.
- Proptest `proptest! { #![proptest_config(ProptestConfig::with_cases(256))] }`. Property: **if a write returns `Ok`, the re-read is `Ok` and equal, and `bytes_remaining() == 0`**. Do not filter aliases out of the strategy; the implication is the whole point.
  - `ObuType`: `ObuType::Reserved(0u8..=40)` in `write_obu(&header, &[])` against `read_obu_header`.
  - Mix Presentation: from `published_mix_presentation()`. Mutate `layouts[0].layout` over `{SoundSystem(from_value(0..16)), Binaural, Reserved(0..=5)}` and `loudness.extension` over `Option<(any u8, vec(any u8, 0..4))>`. Check `write_mix_presentation` against `read_mix_presentation`.
  - Audio Element: from `published_audio_element()`. Use type `Reserved{value: 0u8..=9, raw}` and `params` of `Extension{0u32..=6, vec(..)}`. Check `write_audio_element` against `read_audio_element`.
  - `ParamDefinition`: id and rate `any::<u32>()`, `reserved 0u8..=127`, `duration_fields` `Option<(duration, constant 0..3, vec(any u32, 0..4))>`. Check `write_param_definition` against `read_param_definition`.
  - Ambisonics: parse `tests/fixtures/reference/iamf-tools/tones_100ms_3OA_stereo_opus.iamf`, then mutate via `SceneBased { 0: c, .. }` with mapping length `0..20` and `mode 0..4`.
- **Helper fns and strategies need `.get()` instead of indexing:** the lint exemption covers only unwrap/expect/panic.

## Reference-hash constraint (Q3)

- **Method:** probe `scan()` walked every `.iamf` under `tests/fixtures/` (40 files: 37 positive, 2 negative, 1 golden) plus `fuzz/corpus/parse_sequence/` (4). It used `find_obu_boundaries`, then `read_audio_element` per Audio Element OBU (so negatives are covered too), and listed `param_definition_type`s.
- **Result:** exactly **one** extension param in all 44 files: `tests/fixtures/reference/test_000121.iamf`, `param_types=[3]`, bytes `"ignored"`. Its textproto says `is_valid: true` and "uses a reserved parameter type that must be ignored". At HEAD those bytes decode to phantom id 105, mode 0. No Parameter Block uses id 105, and there are 0 `Raw` blocks in any file. test_000059 has types `[1, 2]`, which are typed.
- **Prediction:** no hash changes, including `test_000121` staying at `04737ff74968d56e326367a2dd2bbe58c4d3b4c5675a3af2fd62d63df9eb3366`. A phantom registry entry never reaches Debug output or findings unless its id collides.
- **Verified:** in the scratch copy with fix (b), `cargo test --locked --test parse_reference` gave 7 passed, which includes `every_positive_matches_its_complete_modeled_field_ledger` and flat write byte-identity. `golden` gave 11 passed. Part B cannot move a hash, because no reader produces a guarded state.

## Common Pitfalls

1. **Check before emitting.** Low-level writers called directly on a caller's `BitWriter` otherwise leave partial bytes. Put each guard at the top of the function.
2. **Do not reject `Extension{type: 0}` or `ObuType::Reserved(>31)` with the new variant.** The first round-trips; the second already has `ValueExceedsWidth`, which is documented.
3. **Test helpers are not lint-exempt** for indexing or arithmetic.
4. **Do not "fix" T1 by catching the error** in `observe_parameter_duplicates` or parse. The registration itself is what is wrong: shadowing happens exactly when the decode *succeeds*.
5. **Re-derive the boundary vector** at `round_trip.rs:287-290` rather than editing numbers by hand; check the new vector with `find_obu_boundaries`.
6. **Run `--features fuzzing`.** Plain `cargo test` does not compile the `fuzz_regression` failures.

## Security Domain (security_enforcement: true)

Only V5 Input Validation applies; auth, session, access control and crypto are N/A.
- T1 removes a parse-time attacker-controlled route into typed Raw subblock allocation. Extension-governed blocks stay one bounded `Vec<u8>`.
- T1 removes a denial of service where a legal file is rejected because of its opaque bytes.
- T2 prevents a writer from emitting mis-framed bitstreams that `libiamf` would read as different OBUs, which is a tampering-class integrity issue. There are no new allocations and no new dependencies.

## Assumptions Log

| # | Claim | Risk if wrong |
|---|---|---|
| A1 | libiamf's `objParam->param_base` is null for ids without a recognised definition | Low. iamf-tools and the spec already justify (b) |
| A2 | Parallax does not call `observe_audio_element` expecting `Err` (checkout absent, so no observation) | Low. The signature is kept |
| A3 | `LoudspeakerLayout`/`SoundSystem`/`HeadphonesRenderingMode` reserved variants alias defined values (not probed) | Deferred only |
| A4 | The memory residual via type >= 3 is closed by (b) (reasoned, not measured) | Low. No regression either way |

## Sources

- **Primary (read at pins this session):**
  - `docs/iamf` `v1.1.0:index.bs` lines 670-700, 746-796, 1540-1595
  - `docs/iamf-tools` `848c6ff4`: `iamf/obu/audio_element.cc:190-236,383-418`, `iamf/obu/param_definitions.{h:285-340,cc:60-200,324-358}`, `iamf/cli/cli_util.cc:40-150`, `iamf/cli/obu_processor.cc:120-175`, `iamf/cli/iamf_encoder.cc:318-328`, `iamf/obu/extension_parameter_data.cc:20-45`
  - `.reference/libiamf` (HEAD == `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63`, verified): `code/src/iamf_dec/IAMF_OBU.c:180-200,455-530,1073-1100`, `IAMF_decoder.c:1150-1160`
- **Codebase:** the files and line ranges cited inline, all opened with Read/sed this session.
- **Experiments** (scratchpad `probe/`, `probe_b/`, `copy/`): outputs are quoted above.

**Valid until:** 2026-10-14. Pins are fixed; line numbers drift with commits.
