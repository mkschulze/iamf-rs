# Quick 260914-1mq: Bound Parameter Block subblock memory amplification - Research

**Researched:** 2026-09-14
**Domain:** Parameter Block parsing (`src/obu/parameter_block.rs`), parse-side memory bounds
**Confidence:** HIGH. I prototyped the fix in a scratch copy of HEAD. The full offline suite passed there (555 tests), as did `clippy -D warnings` and the `fuzzing` replay. A before/after differential scan of all 110 committed fixture and corpus files showed no difference at all.

<user_constraints>
## User Constraints (binding, from `.planning/.continue-here.md` BLOCKING CONSTRAINTS)

- **Conformance rule:** satisfy BOTH the IAMF v1.1.0 spec AND the pinned references. Where either one is stricter, keep the stricter rule. Record every spec/reference disagreement in a `// ref:` comment.
- **Reference hashes:** a `semantic_sha256` in `tests/support/reference_expectations.rs` may change ONLY for a fixture that the pinned `iamf-tools` testdata marks `is_valid: false`. Predict the new hash, then verify it exactly.
- **Pinned trees only:** `git -C docs/iamf-tools show 848c6ff4:<path>`, `git -C docs/iamf show v1.1.0:index.bs`, `git -C docs/libiamf show f06e919e:<path>`.
- **Staging discipline:** never `git add -A` / `.` / `commit -a`. Stage explicit paths and check `git diff --cached --name-only`.
</user_constraints>

## Project Constraints (from CLAUDE.md)

- Clippy denies `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used` and `panic`. The test exemption covers only `#[test]` fn bodies. Helper fns in test files are **not** exempt, which I hit in the prototype.
- `// ref:` citations are required above `read_*`/`write_*`. Existing `ErrorKind` variants may not change; adding one is non-breaking.
- Never silently normalise. Rust 1.85 has no let-chains. Golden bytes stay unchanged. The linked graph stays `iamf` + `thiserror`.

## Summary

The review's diagnosis is correct, and the real picture is a bit wider than it says. `read_parameter_block` (`src/obu/parameter_block.rs:395-450`) bounds `num_subblocks` only by `r.bytes_remaining()` (`:445`). A Recon Gain subblock whose `recon_gain_is_present` flags are all false consumes **0 bytes** (`:736-739`). Each such subblock still costs 40 bytes, plus a heap `Vec<Option<ReconGainElement>>` of 20 bytes × `num_layers`. The padding the attacker needs to pass the byte rule is never consumed; it ends up as `trailing`. I measured a single 2 MiB OBU at up to **204× model bytes per input byte** (7 absent layers), before allocator overhead.

The spec and `iamf-tools` give a principled bound. Demixing and Recon Gain definitions **SHALL have `num_subblocks` = 1** (`index.bs:776-782`, `:786-792`). `iamf-tools` enforces this by requiring `constant_subblock_duration == duration` in `ValidateSpecificParamDefinition`, which runs on every Parameter Block read (`parameter_block.cc:376`) and write (`:350`). Rejecting any other count cannot reject a file that is spec-legal or that iamf-tools accepts. The prototype also turned up a **pre-existing false rejection**. A legal single Recon Gain block with every layer absent and no trailing bytes (`01 c0 07 c0 07`) fails today with `UnexpectedEndOfInput @0`, because `1 > bytes_remaining() == 0`.

**Primary recommendation:** add `ErrorKind::SubblockCountNotOne`. In `read_parameter_block`, reject a Demixing or Recon Gain context with `num_subblocks != 1` at `InputOffset(start)`, before the byte rule. Skip the byte rule for Recon Gain, whose count is now exactly 1. Mirror the count check in `validate_parameter_block` at `Field("subblocks")`. No hash changes, no golden change, no user decision required. `read_strings` and the Raw/Mix Gain residuals are linear, 1+ byte per element, and are **deferred**, as recorded in the CONCERNS residual.

## Q1 - Current code, per `param_definition_type`

Where the count comes from (`parameter_block.rs:395-432`):

| Mode | `constant_subblock_duration` (csd) | `num_subblocks` source | `subblock_duration` on wire? |
|---|---|---|---|
| 1 (block carries durations) | 0 | read uleb (`:399`) | yes, each ≥1 byte (`:455-465`); sum must == `duration` (`:475-482`) |
| 1 | ≠0 | `ceil(duration/csd)` (`:401-405`), **up to u32::MAX from 2 wire bytes** | no |
| 0 (from definition) | 0 | `subblock_durations.len()` of the definition (`:421-423`) | no |
| 0 | ≠0 | `ceil(def.duration/def.csd)` (`:425-429`) | no |

So a zero-byte subblock needs mode 1 with csd≠0 or mode 0. In both, the count is derived rather than paid for with wire bytes, and the only guard is `:445`.

Minimum wire size per subblock, from `read_parameter_data` `:694-772`, and the size of the resulting model:

| Kind | Min wire bytes | Model per subblock (measured `size_of`) | Worst ratio @2 MiB (capacity × size, measured) |
|---|---|---|---|
| Recon Gain, all layers absent | **0** | 40 + heap 20×num_layers (0..7) | 64× (0 layers) … **204×** (7 layers) |
| Demixing | 1 | 40 | 64× |
| Raw (extension type ≥3, `parameter_data_size`=0) | 1 | 40 (+0 heap) | 64× |
| Mix Gain Step | 3 | 40 | 16× |
| `read_strings` NUL string | 1 | 24 + heap cap 8 | ~32× |

`size_of::<ParameterSubblock>() = 40`, `ParameterData = 32`, `Option<ReconGainElement> = 20`, `Vec<u8> = 24`; empty string cap after pop = 8. The "64×" figures include the up-to-2× `Vec` growth slack; the exact len×size ratio is 40×. Probe: scratchpad `probe_keep/zz_1mq_probe.rs`. In a full sequence the unread padding is also copied into `Obu::trailing` (+1×).

## Q2 - Spec and reference evidence (pinned trees)

- **Spec** `index.bs:776-782` (DemixingParamDefinition) and `:786-792` (ReconGainParamDefinition), verbatim: "`param_definition_mode` SHALL be set to 0." / "`duration` SHALL be the same as `num_samples_per_frame` …" / "`num_subblocks` SHALL be set to 1." / "`constant_subblock_duration` SHALL be same as `duration`." [CITED: index.bs@v1.1.0]
- **Spec** `:1603`: "`subblock_duration` … SHALL NOT be set to 0"; `:1591`: block `duration` SHALL NOT be 0. So `num_subblocks ≤ duration` always holds, but `duration` is a u32. That bound does not help.
- **A zero-byte Recon Gain subblock is legal.** `ReconGainInfoParameterData` (`:1713-1724`) reads only layers with `recon_gain_is_present_flag(i) == 1`. No normative text forbids a Recon Gain definition whose flags are all 0. Annex `:3038`/`:3044` is informative: layer 0 and single-layer elements get flag 0, and "no Parameter Block OBUs for recon gain info" are emitted when all flags are 0. **Therefore we must not reject zero-byte subblocks as such, only counts ≠ 1.** [CITED]
- **iamf-tools@848c6ff4** `iamf/obu/param_definitions.cc:37-60 ValidateSpecificParamDefinition`: for `kParameterDefinitionDemixing`/`kParameterDefinitionReconGain` it requires `param_definition_mode_ == 0`, `duration_ != 0` and `constant_subblock_duration_ == duration_`. It is called from `ParamDefinition::Validate` (`:241`), which `ParameterBlockObu::ReadAndValidatePayloadDerived` calls first (`parameter_block.cc:376`), and likewise `ValidateAndWritePayload` (`:350`). `obu_processor.cc:170-174` propagates the failure, aborting the decode. Otherwise it does `subblocks_.resize(num_subblocks)` unbounded (`:396`). Note that `ParamDefinition::ReadAndValidate` (`:132-158`) returns early for mode 1 or csd≠0, so the descriptor itself is accepted and the rejection happens at the block. [VERIFIED: pinned source read]
- **libiamf@f06e919e** `code/src/iamf_dec/IAMF_OBU.c:1103-1129` (`iamf_parameter_new`): no count check. `IAMF_MALLOCZ(ParameterSegment *, para->nb_segments)` is unbounded, and `:383-411` `iamf_parameter_base_init` is the same. This is a permissive reader, recorded as a DISAGREEMENT. [VERIFIED: pinned source read]

Citation symbols: `iamf-tools@v2.1.0 iamf/obu/param_definitions.cc ValidateSpecificParamDefinition`, `iamf-tools@v2.1.0 iamf/obu/parameter_block.cc ParameterBlockObu::ReadAndValidatePayloadDerived`, `libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c iamf_parameter_new`.

## Q3 - Fix options

| Option | Bounds zero-byte class? | Legal-file risk | Verdict |
|---|---|---|---|
| **(a) Demixing/Recon Gain `num_subblocks == 1` (spec + iamf-tools)** | Yes, count becomes 1 | None: spec SHALL, and iamf-tools rejects otherwise | **Recommended** |
| (a+) Full iamf-tools rule (mode 0, csd == duration) at parse | Yes | None for conformant files, but it breaks the crate's own mode-1 models (`src/fuzzing.rs:103-110`, `tests/support/sequence_cases.rs:119-123`, `tests/temporal.rs` 836-975) | Out of scope; see "Optional follow-up" |
| (b) Ratio cap derived from `obu_size` | Only partly: still linear | Arbitrary, not reference-derived | Reject |
| (c) Compact storage of absent layers | Reduces 204× to 64× only | None | Unnecessary once (a) is in |
| (d) Per-parse memory budget | Yes, all classes | An API change (`parse_sequence_with_limits`) | Defer; user call if a hard RSS bound is ever wanted |

Rejecting counts **≠ 1** rather than **> 1** is deliberate. The spec says "SHALL be set to 1". Count 0 needs `duration` 0, which the spec forbids and iamf-tools rejects. No committed file has either.

**Relaxing the byte rule for Recon Gain** is safe after (a), because the count is exactly 1 and the allocation is ≤ 1 subblock plus ≤ 7 layer slots. It also fixes the false rejection of a legal zero-byte block. Keep the byte rule for Demixing, Raw and Mix Gain: every one of those subblocks is ≥ 1 byte, and keeping it leaves existing error kinds and offsets unchanged.

**`read_strings`** (`src/obu/mix_presentation.rs:688-703`): each string is ≥ 1 wire byte (NUL) and costs ~32 bytes of model. This is linear, bounded by the 2 MiB OBU cap, and already accepted as CONCERNS.md residual (1). **Deferred.** Be honest about what remains after the fix: Raw extension subblocks at 1 byte still give 40× exact / 64× with slack. So the review's 38 MB → 1.94 GB RSS scenario is still reachable through `param_definition_type ≥ 3` blocks. That is the same linear class as strings, so record it in the CONCERNS residual and leave (d) for later.

**User decision:** not required for this task. Optional follow-up to put to the user:
> *EN:* Should the full iamf-tools rule for Demixing/Recon Gain definitions (`param_definition_mode` = 0, `constant_subblock_duration` = `duration` ≠ 0) become a `validate()` finding? That means test churn in the fuzz model and `sequence_cases`, which use mode 1.
> *DE:* Soll die vollständige iamf-tools-Regel für Demixing-/Recon-Gain-Definitionen (Modus 0, `constant_subblock_duration` = `duration` ≠ 0) zusätzlich als `validate()`-Finding gemeldet werden? Diese Aufgabe erzwingt nur die Folge daraus (genau 1 Subblock). Option A: jetzt nicht (empfohlen, eigener Quick-Task). Option B: als Finding im selben Task.

## Q4 - Error kind and location

- Existing kinds do not fit. `SubblockDurationMismatch` means "subblock durations do not sum to the block duration" (`src/error.rs:111-112`), and `UnsupportedParameterData` means "this parameter data shape is not modelled". **Add `SubblockCountNotOne`** with the message "a Demixing or Recon Gain Parameter Block does not have exactly one subblock". It is a unit variant, so the `size_of::<Error>() <= 32` budget is unaffected.
- Parallax does not match on `iamf::ErrorKind`. `rg ErrorKind /Users/cell/local/Parallax --glob '*.rs'` finds only `std::io::ErrorKind`. HANDOFF.md:52-60 lists encoder-path kinds. The new kind cannot reach the encoder, because `EncoderBuilder::build` rejects any Audio Element params (`src/encoder.rs:1314-1322`). **HANDOFF.md needs no change.**
- Read location is `Location::InputOffset(start)`, where `start` is the Parameter Block payload start, i.e. `parameter_id`. The count is often derived, so there is no single field to point at. This matches the `ParameterModeMismatch` (`:416-419`) and byte-rule (`:445-449`) convention. In `parse_sequence`, `with_input_base(payload_base)` makes it absolute (`src/sequence.rs:576-579`). Write location is `Location::Field("subblocks")`, matching `:585-591`.

## Q5 - Impact scan

- **Differential scan:** 110 of 110 files under `tests/fixtures`, `fuzz/corpus` and `fuzz/artifacts` gave identical results before and after the change. That covers parse Ok/Err with kind and offset, the sha256 of `{seq:#?}\nfindings=…` (the `semantic_sha256` projection) and the sha256 of `dump_annotated`. Per-file subblock inventory: the only Recon Gain block is `test_000059` (`recon:1`). There are no Demixing blocks. Multi-subblock blocks are all Mix Gain (`test_000016` 22, `test_000071` 3/2, `test_000088` 3), which the change does not touch.
- **Hash prediction: no `semantic_sha256` changes. Golden unchanged.** Verified: `parse_reference` and `golden` pass in the prototype.
- **Write side:** today `validate_parameter_block` (`:529-620`) does not check the count, and neither does the encoder (it never emits Demixing/Recon Gain). `src/fuzzing.rs:287-299` and `sequence_cases.rs:302-314` build one-subblock blocks, so the mirrored write check does not affect them (`fuzz_regression` passes).
- **Parse error, not `validate()` finding:** this follows the precedent of `SubblockDurationMismatch`, a parse error that mirrors `ReadAndValidatePayloadDerived`. It also has to be at parse time, because a finding comes after the allocation. That timing is the reason for the fix.

## Q6 - Test plan

RED evidence on HEAD: the 2 MiB vector parses `Ok` with 2_097_147 subblocks (probe), and `01 c0 07 c0 07` fails `UnexpectedEndOfInput@0` (prototype run). Prototype tests: scratchpad `1mq/tests/zz_1mq_fix.rs` (8 tests, all green).

Put the parameter-block tests in **`tests/temporal.rs`** next to `a_subblock_count_larger_than_the_bytes_remaining_is_refused_before_reserving`. Put the 2 MiB test in **`tests/allocation_bounds.rs`**. All use `registry.register(ParamDefinition::mode_1(id, 48_000), ctx)`:

| Test | Context | Bytes | Expect |
|---|---|---|---|
| 2 MiB hostile (allocation_bounds) | ReconGain `[false;7]` | `01 FB FF 7F 01` zero-extended to 1<<21 (duration 2_097_147, csd 1, count == bytes left) | `SubblockCountNotOne @ InputOffset(0)` |
| small hostile | ReconGain `[false;2]` | `01 FF FF FF 7F 01` | same (new kind wins over the byte rule) |
| explicit count 2 | ReconGain `[false]` | `01 02 00 02 01 01` | same |
| mode-0 definition implies 2 | ReconGain `[false;2]`, def `DurationFields{duration:2, csd:1, []}` | `01 00 00` | same |
| demixing ×2 | Demixing | `05 02 01 bb bb` | same |
| **positive:** single zero-byte recon gain | ReconGain `[false;2]` | `01 c0 07 c0 07` | Ok, 1 subblock `layers: [None, None]`, write round-trips exactly |
| **positive:** multi-subblock mix gain | MixGain | `01 02 01 00 00 01 00 ff ff` | Ok, 2 Step subblocks (second `-1`), round-trips |
| writer | Demixing, block duration 2 / csd 1, 2 subblocks | model | `SubblockCountNotOne @ Field("subblocks")` |

The existing `test_000059_two_layer_recon_gain_shape_round_trips_exact_bytes` and `demixing_data_uses_exactly_three_mode_bits_and_five_reserved_bits` stay as positive controls. The memory-bound assertion needs no `unsafe`: the rejection happens before `bounded_vec` (`:452`), so an `Err` means nothing was allocated, and positive tests assert `subblocks.len() == 1`. Test helpers must return `Option<(ErrorKind, Location)>` via `.err().map(..)`, because `expect_err` and `[i]` fail clippy outside `#[test]` bodies.

## Q7 - Pitfalls

1. **Order:** put `check_single_subblock` after the count is known and **before** `usize::try_from` / the byte rule. Otherwise a small hostile Recon Gain vector reports `UnexpectedEndOfInput`.
2. **Don't drop the byte rule wholesale:** exempt only `ParameterDataContext::ReconGain`. `corrupt_known_demixing_syntax_is_not_reclassified_as_raw_data` and the MixGain hostile test rely on it.
3. **Clippy in test helpers:** see Q6. `matches!` without let-chains is fine on 1.85.
4. **Citations:** the new helper is not `read_*`/`write_*`, but give it `// ref:` lines (spec :776-782, :786-792; iamf-tools `ValidateSpecificParamDefinition`), a `NOTE:` that the reference is stricter (mode/csd), and `DISAGREEMENT: libiamf … iamf_parameter_new`. Error-variant comments follow the `DuplicateAnchorElement` style (`src/error.rs:180-184`).
5. **Gates run on the prototype:** `cargo clippy --locked --all-targets -- -D warnings` clean. `cargo test --locked` 555 passed, 0 failed. `cargo test --locked --features fuzzing --test fuzz_regression` 5 passed. `rg -U -c 'allow\(\s*clippy::disallowed_types' src/` → `src/model/loudness.rs:1`.
6. **Docs:** update `.planning/codebase/CONCERNS.md`. The count-driven section should record the zero-byte class as closed and name the remaining linear residuals: Raw extension subblocks 40×, Mix Gain 13×, strings ~32×, and per-parse budget (d) as the only hard bound. Mark the review finding resolved.

## Security Domain (ASVS L1)

V5 Input Validation / DoS (CWE-400, CWE-789): this is a bounded allocation driven by a derived count. The mitigation is the spec-derived structural rule, checked before allocation. It is not a heuristic cap. No other ASVS category applies: the crate has no auth, sessions or crypto.

## Assumptions Log

| # | Claim | Risk if wrong |
|---|---|---|
| A1 | iamf-tools/libiamf *run* (not just read) accept a single zero-byte Recon Gain block. This comes from reading the source; no reference binary was run | Low. The relaxation only changes an error into `Ok` for count 1, which the spec syntax permits |

## Sources

- Pinned spec `docs/iamf` v1.1.0 `index.bs` :776-792, :1543-1605, :1705-1724, :3038-3044
- `iamf-tools@848c6ff4`: `iamf/obu/param_definitions.cc` :37-60, :132-158, :189-243; `iamf/obu/parameter_block.cc` :39-49, :70-118, :202-225, :345-352, :373-420; `iamf/obu/recon_gain_info_parameter_data.cc` ReadAndValidate; `iamf/cli/obu_processor.cc` :153-196
- `libiamf@f06e919e`: `code/src/iamf_dec/IAMF_OBU.c` :383-411, :1073-1260
- Repo HEAD `64d2eb3`: `src/obu/parameter_block.rs`, `src/obu/param_definition.rs`, `src/error.rs`, `src/encoder.rs:1314-1322`, `src/fuzzing.rs`, `tests/temporal.rs`, `tests/support/sequence_cases.rs`
- Scratch prototype: `scratchpad/1mq/` (diff in `scratchpad/1mq.diff`), scans `scratchpad/scan_before.tsv` / `scan_after.tsv`
