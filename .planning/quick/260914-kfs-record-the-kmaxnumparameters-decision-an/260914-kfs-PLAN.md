---
phase: quick-260914-kfs
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/obu/audio_element.rs
  - tests/sequence_parse.rs
  - tests/encoder_builder.rs
  - CONFORMANCE-GATE.md
  - HANDOFF.md
  - .planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md
autonomous: true
requirements: [PARSE-01, PARSE-04, DESC-04]

estimate:
  tokens: 45000
  raw_tokens: 45000
  tasks: 2
  confidence: low

must_haves:
  truths:
    - "User decision 2026-09-14 (1): an Audio Element with 257 params (each AudioElementParam::Extension { param_definition_type: 3, bytes: [] }) still parses with parse_sequence, keeps all 257 params, and write_parsed_sequence reproduces the input bytes exactly"
    - "User decision 2026-09-14 (2): AudioElement::validate, DescriptorSet::validate and ParsedSequence::validate each report exactly one Finding at Location::Field(\"num_parameters\") for that 257-param element, with the exact message 'audio element 300 has 257 parameters; iamf-tools@v2.1.0 refuses an Audio Element with more than 256 (kMaxNumParameters) on read and write, although IAMF v1.1.0 requires parsers to support any num_parameters'; on HEAD there is none"
    - "The boundary matches iamf-tools `num_parameters > kMaxNumParameters`: with exactly 256 params DescriptorSet::validate equals the unmodified published set's findings and ParsedSequence::validate has no Field(\"num_parameters\") finding"
    - "User decision 2026-09-14 (3): the num_parameters read site in read_audio_element and the finding carry `// ref:` citations to the spec (index.bs:751-754), iamf-tools@v2.1.0 ValidateNumParameters / kMaxNumParameters and libiamf@v1.1.0 iamf_element_new"
    - "EncoderBuilder source is unchanged. It already refused every Audio Element param; for 257 params the element finding now fires first at encoder.rs validate_findings, so build returns InvalidDescriptorReference at Field(\"descriptors\") instead of UnsupportedParameterData at Field(\"audio_element_params\"); the 1-param test stays UnsupportedParameterData"
    - "User decision 2026-09-14 (4): CONFORMANCE-GATE.md records the decision and evidence, HANDOFF.md records the consumer-visible finding and the builder kind precedence, and 260914-hoa-deferred-items.md item 2 is marked resolved"
    - "No semantic_sha256 in tests/support/reference_expectations.rs, golden artifact, fixture, corpus seed or ErrorKind changes; fmt, build, clippy, cargo test --locked, fuzz replay, prove-guards (9 PASS), float census (1 hit) and cargo tree (iamf + thiserror) pass"
  artifacts:
    - path: "src/obu/audio_element.rs"
      provides: "IAMF_TOOLS_MAX_NUM_PARAMETERS const, the num_parameters Finding in AudioElement::validate, cited comments at the read and write sites"
      contains: "kMaxNumParameters"
    - path: "tests/sequence_parse.rs"
      provides: "RED-first 257-param finding test through all three validate surfaces plus the 256 control"
      contains: "audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit"
    - path: "tests/encoder_builder.rs"
      provides: "pins the builder outcome for 257 Audio Element params"
      contains: "build_rejects_257_audio_element_params_through_the_element_finding"
    - path: "CONFORMANCE-GATE.md"
      provides: "decision record for kMaxNumParameters"
      contains: "kMaxNumParameters"
    - path: "HANDOFF.md"
      provides: "consumer note on the num_parameters finding"
      contains: "num_parameters"
  key_links:
    - from: "src/model/mod.rs DescriptorSet::validate (element.validate())"
      to: "AudioElement::validate"
      via: "existing per-element loop; mod.rs is not edited"
      pattern: "findings.extend\\(element.validate\\(\\)\\)"
    - from: "src/sequence.rs ParsedSequence::validate (SequenceObu::AudioElement arm)"
      to: "AudioElement::validate"
      via: "existing obu.payload.validate() call; sequence.rs is not edited"
      pattern: "SequenceObu::AudioElement\\(obu\\)"
    - from: "src/encoder.rs EncoderBuilder::build validate_findings(declaration.element.validate())"
      to: "AudioElement::validate"
      via: "runs before the audio_element_params gate, so the new finding decides the kind for > 256 params; encoder.rs is not edited"
      pattern: "validate_findings\\(declaration.element.validate\\(\\)\\)"
---

<objective>
Record the `kMaxNumParameters` decision (deferred item 2 of quick 260914-hoa) and implement it exactly as the user
locked it on 2026-09-14:

1. **Read stays unrestricted.** The parser keeps accepting any `num_parameters`. IAMF v1.1.0 `index.bs:754`: "Parsers
   SHALL support any value of num_parameters."
2. **`validate()` reports it.** When an Audio Element has more than 256 params, `AudioElement::validate` pushes a
   `Finding` at `Location::Field("num_parameters")`. The finding says that `iamf-tools@v2.1.0` refuses such an Audio
   Element (`absl::UnimplementedError`, "Number of parameters exceeds the maximum supported by the decoder"). It sits in
   `AudioElement::validate`, the single site that `DescriptorSet::validate` (`src/model/mod.rs:127-129`) and
   `ParsedSequence::validate` (`src/sequence.rs` `SequenceObu::AudioElement` arm) already call, so all three surfaces
   report it with no second copy of the rule.
3. **Cited comments** go at the read site and at the finding.
4. **Evidence docs:** CONFORMANCE-GATE.md, HANDOFF.md and the hoa deferred-items entry.

Boundary: `> 256`, matching iamf-tools `num_parameters > AudioElementObu::kMaxNumParameters`. 256 is accepted and 257 is
refused.

The writer is not changed (Claude's discretion, consistent with the decision). iamf-tools also refuses > 256 on write
(`ValidateAndWritePayload`, `audio_element.cc:759`). Refusing on write here would break parse-then-write byte exactness
for a spec-legal foreign file (PARSE-04). The Phase 02 / quick 260914-5c5 precedent for `param_definition_type` 0 is the
same: a stricter reference rule is diagnosed by `validate()`, not refused. A `// NOTE:` at the write site records this.

EncoderBuilder is affected in error kind only, and its source is not edited. `build()` already rejects every Audio Element
param (`src/encoder.rs:1314-1322`, UnsupportedParameterData at `Field("audio_element_params")`). But
`validate_findings(declaration.element.validate())?` at `src/encoder.rs:1103` runs first and maps any element finding to
`InvalidDescriptorReference` at `Field("descriptors")`. So for > 256 params the new finding decides the kind. The existing
type-0 finding already has the same precedence. A test pins it and HANDOFF.md states it.

Output: one code commit and one docs commit. The deferred-items file is edited but not staged.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md
@tests/CLAUDE.md
@.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md

Working directory: `/Volumes/lab/talea/iamf-rs`. Base commit: `2b4c058`. Work sequentially on `main`, with no worktrees.

The working tree already contains orchestrator- and user-owned changes. Never stage any of them:
- untracked `.DS_Store`, `docs/`, `iamf-rs.code-workspace`
- modified `.planning/STATE.md` (if present)
- this quick-task directory, including its PLAN and SUMMARY
- `.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md` (edit it in Task 2, but the orchestrator commits it)

Never use `git add -A`, `git add .` or `git commit -a`. Stage explicit paths and check `git diff --cached --name-only`
before every commit.

Interfaces verified this session (do not re-explore):

- **`src/obu/audio_element.rs`**
  - `:4` `use crate::bits::{BitCursor, BitWriter, bounded_vec};`
  - `:352-356` `pub fn num_parameters(&self) -> usize { self.params.len() }`
  - `:358-453` `pub fn validate(&self) -> Vec<Finding>`, doc "Findings this element can carry, reported and never raised
    (D-07/D-09)". Element-local findings already use `Location::Field("num_substreams")` / `Field("num_layers")` and
    `.to_owned()` / `format!` messages. The `for param in &self.params` loop ends at `:450`, then `findings` is returned
    at `:451`. `Finding`, `Location` are already imported.
  - `:464-471` `// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc AudioElementObu::ReadAndValidatePayloadDerived`
    above `pub fn read_audio_element`. Its doc says every count is checked against `bytes_remaining()` (T-01-24).
  - `:478` `let params = read_counted(r, read_audio_element_param)?;` is the num_parameters read site.
  - `:542` `write_count(w, v.params.len(), "num_parameters")?;` is the write site.
  - `:568-587` `fn read_counted`: rejects `count > r.bytes_remaining()` with UnexpectedEndOfInput, then `bounded_vec(count)`.
- **`src/encoder.rs`** (not edited): `:1100-1103` loop over `self.audio_elements` calls `validate_element_topology`, then
  `validate_findings(declaration.element.validate())?`. `:1424-1433` `validate_findings` returns
  `Error::new(ErrorKind::InvalidDescriptorReference, Location::Field("descriptors"))` for any non-empty findings. `:1310`
  `from_descriptors`. `:1314-1322` params gate.
- **`tests/sequence_parse.rs`**: imports at `:6-22` already include `ErrorKind, Finding, Location`, `DescriptorSet`,
  `AudioElementParam`, `SequenceObu`, `parse_sequence`, `write_parsed_sequence` and `support::published_descriptor_set`.
  `support::descriptor_bytes(&set) -> Vec<u8>` exists (`tests/support/test_000003.rs`, included as `support`). The
  published set has one channel-based Audio Element, id 300, codec 200, substream `[0]`, no params. Its
  `validate()` already yields exactly one finding, the nested `parameter_id 100` duplicate (`tests/descriptors.rs:1371`).
  Existing exact-finding style: `assert_eq!(parsed.validate(), vec![iamf::error::Finding { at: Location::Field(..),
  message: "...".to_owned() }])` (`:523-529`).
- **`tests/encoder_builder.rs`**: `:143-162` `build_rejects_audio_element_params_even_when_an_extension_definition_is_opaque`
  pushes one `Extension{3, []}` onto `stereo_element()` and asserts UnsupportedParameterData at
  `Field("audio_element_params")`. That test is the control and stays unchanged. Helpers `lpcm_config()` and
  `stereo_element()` (`:976`) exist, and `iamf::{ErrorKind, Location}` is imported at `:12`.
- **Pinned evidence** (orchestrator-verified; re-read only through `git -C docs/iamf-tools show 848c6ff4:<path>` and
  `git -C .reference/libiamf show f06e919e:<path>`):
  - iamf-tools@v2.1.0 `iamf/obu/audio_element.h:282-287`: "This class has stricter limits than the specification:
    Maximum number parameters is limited to `kMaxNumParameters`." and
    `static constexpr uint32_t kMaxNumParameters = 256;`, "Artificial limit on the maximum number of parameters."
  - iamf-tools@v2.1.0 `iamf/obu/audio_element.cc:97-113` `ValidateNumParameters`. It cites §3.6 "SHALL support any
    value". It gives the memory rationale ("To reduce the risk of allocating massive amounts of memory, we limit the
    number of parameters.") and returns `absl::UnimplementedError` when `num_parameters > kMaxNumParameters`. It is called on
    write at `:759` (`ValidateAndWritePayload`) and on read at `:811` (`ReadAndValidatePayloadDerived`, before
    `reserve`).
  - The limit was introduced by iamf-tools commit `feb873de` (2025-11-03), "`AudioElementObu`: Limit number of
    parameters, to fix cases of excessive fuzzer memory."
  - libiamf@v1.1.0 `code/src/iamf_dec/IAMF_OBU.c:455-480` `iamf_element_new` has no limit. It does
    `IAMF_MALLOCZ(ParameterBase *, val)` for the read count and skips unknown types with `bs_skipABytes`.
  - Spec `docs/iamf/index.bs:751-754` (`num_parameters` semantics; `:754` "Parsers SHALL support any value of
    [=audio_element_obu/num_parameters=]."). The `docs/iamf` checkout is untracked, so grep that line text before citing
    it. The same checkout is behind the existing `index.bs:680-689` / `:772` citations in this file.
- **Evidence docs**
  - `DIFF-LEDGER.md` is a test-asserted CONF-07 offset table (`tests/conformance.rs`). Do NOT touch it.
  - `CONFORMANCE-GATE.md` holds decisions and research facts (for example `#### ADOPTED: fallback (b) — user decision,
    2026-09-08` and `### Experiment B`, which says "Being stricter than the pinned reference is its own defect class").
    `## Waivers` starts at `:646`, directly after `## Cross-target byte-identity evidence`. No test parses this file.
  - `HANDOFF.md` `### Deferred responsibilities` (`:82-95`) ends with the 5c5 bullets, the last being
    "- `EncoderBuilder` output and its error kinds are unchanged."
- **Memory mitigations already in place**: `read_counted`'s `bytes_remaining()` bound plus `bounded_vec` 64 KiB
  preallocation cap (quick 260913-js8), the 2 MiB OBU cap, and streaming `SequenceReader` (quick 260914-hoa).

Project constraints: Rust 1.85 (no let-chains). In `src/`: no `unwrap`/`expect`/indexing, no bare arithmetic, no new
`#[allow]`. In `tests/*.rs`: `unwrap`/`expect`/`panic` only inside `#[test]` bodies. Helper fns must be panic-free and
index-free (`.first_mut()`, `.get()`). Add no new `fn read_*`/`fn write_*`. No `ErrorKind` change.

Out of scope, not planned (note in SUMMARY deferred items): spec `index.bs:752-753` also says channel-based elements
SHALL carry 0, 1 or 2 params and scene-based elements 0. This crate has no finding for that today, and the user decision
covers only the iamf-tools 256 limit.
</context>

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: A >256-param Audio Element parses and round-trips, and every validate() surface reports the iamf-tools limit (RED first)</name>
  <files>tests/sequence_parse.rs, tests/encoder_builder.rs, src/obu/audio_element.rs</files>
  <precondition>git diff --quiet HEAD -- src tests HANDOFF.md CONFORMANCE-GATE.md succeeds at base 2b4c058</precondition>
  <behavior>
    - K1 `audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit` (tests/sequence_parse.rs). Take published_descriptor_set() and set audio_elements[0].params to 257 copies of AudioElementParam::Extension { param_definition_type: 3, bytes: vec![] }. Hand decode: num_parameters 257 is uleb128 `81 02`, each param is `03 00`. The expected Finding is at Location::Field("num_parameters") with the exact message "audio element 300 has 257 parameters; iamf-tools@v2.1.0 refuses an Audio Element with more than 256 (kMaxNumParameters) on read and write, although IAMF v1.1.0 requires parsers to support any num_parameters". Assert:
      (a) the element's own validate(), filtered to at == Field("num_parameters"), equals vec![expected];
      (b) set.validate() filtered the same way equals vec![expected], and its len equals published_descriptor_set().validate().len() + 1;
      (c) bytes = support::descriptor_bytes(&set). parse_sequence(&bytes) is Ok (decision 1: no reject on read), its SequenceObu::AudioElement payload has params.len() == 257, and write_parsed_sequence(Vec::new(), &parsed) == Ok(bytes);
      (d) parsed.validate() filtered to Field("num_parameters") equals vec![expected].
      HEAD: (a), (b) and (d) are empty, so the test FAILS. (c) already holds on HEAD.
    - K2 control `audio_element_with_256_params_has_no_num_parameters_finding` (tests/sequence_parse.rs), same construction with 256. set.validate() == published_descriptor_set().validate() exactly. parse_sequence(descriptor_bytes(set)) is Ok with 256 params, and parsed.validate() has no finding at Field("num_parameters"). It must be ok on HEAD and after.
    - K3 `build_rejects_257_audio_element_params_through_the_element_finding` (tests/encoder_builder.rs): stereo_element() with 257 Extension{3, []} params is added to a builder with lpcm_config(). build() is Err with kind InvalidDescriptorReference at Location::Field("descriptors"). HEAD: UnsupportedParameterData at Field("audio_element_params"), so it FAILS.
    - Control K4 (existing, unchanged): `build_rejects_audio_element_params_even_when_an_extension_definition_is_opaque` stays ok, with 1 param and UnsupportedParameterData.
  </behavior>
  <action>
    Step 1 (RED, tests only).
    - In `tests/sequence_parse.rs`, add a panic-free, index-free helper `fn published_set_with_extension_params(count: usize) -> DescriptorSet`. It clones published_descriptor_set(), and if `audio_elements.first_mut()` is Some, sets `params` to `core::iter::repeat_with(|| AudioElementParam::Extension { param_definition_type: 3, bytes: Vec::new() }).take(count).collect()`. repeat_with avoids depending on a Clone derive.
    - Add a panic-free helper `fn num_parameters_findings(findings: Vec<Finding>) -> Vec<Finding>` that keeps `at == Location::Field("num_parameters")`.
    - Add a panic-free helper `fn audio_element_payloads(parsed: &ParsedSequence) -> Vec<&AudioElement>` via `filter_map` over `SequenceObu::AudioElement(obu)`. Add `AudioElement` to the existing `iamf::obu` import only if it is not already there. It is at `:12`, so no change is expected.
    - Write K1 and K2 exactly as in behavior. Build the expected Finding inline with `Finding { at: Location::Field("num_parameters"), message: "...".to_owned() }` and compare with `assert_eq!`. Put a one-line hand-decode comment above K1 (D-25 style, per the behavior). Cite user decision 2026-09-14 (quick 260914-kfs) in a short comment on each test.
    - In `tests/encoder_builder.rs`, add K3 directly after the test at `:143-162`. Build 257 params with the same repeat_with pattern and assert `error.kind() == &ErrorKind::InvalidDescriptorReference` and `error.at() == Location::Field("descriptors")`. Comment that build() never accepts Audio Element params, and that the element's own num_parameters finding (quick 260914-kfs) runs first at `validate_findings(declaration.element.validate())`, the same precedence as the existing type-0 finding.
    - Run `mkdir -p target && cargo test --locked --no-fail-fast --test sequence_parse --test encoder_builder > target/kfs-t1-red.log 2>&1 || true`. The log must show `FAILED` for K1 and K3 and `ok` for K2 and K4. K1's panic must come from a finding assertion, not from parse or write: the log must not contain `UnexpectedEndOfInput` or `ObuTooLarge` for K1. If anything differs, stop and report the excerpt. Do not adapt expectations to HEAD.

    Step 2 (GREEN, `src/obu/audio_element.rs` only).
    - Add a private constant near `validate` (above `impl AudioElement` or directly before the method's impl block): `const IAMF_TOOLS_MAX_NUM_PARAMETERS: usize = 256;`. Give it a `///` doc: iamf-tools' `AudioElementObu::kMaxNumParameters`, an artificial limit stricter than the spec. Put a plain-comment block above it:
      `// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.h AudioElementObu::kMaxNumParameters`
      `// NOTE: diagnosed by AudioElement::validate, never enforced on read or write (user decision 2026-09-14, quick 260914-kfs; CONFORMANCE-GATE.md).`
      Keep it private: no public API is added.
    - In `AudioElement::validate`, after the `for param in &self.params` loop and before returning `findings`, push the Finding when `self.params.len() > IAMF_TOOLS_MAX_NUM_PARAMETERS`. The comparison is `>`, not `>=`, to match iamf-tools. Use `Location::Field("num_parameters")` and `format!("audio element {} has {} parameters; iamf-tools@v2.1.0 refuses an Audio Element with more than 256 (kMaxNumParameters) on read and write, although IAMF v1.1.0 requires parsers to support any num_parameters", self.audio_element_id, self.params.len())`. Put a plain-comment block directly above the `if`:
      `// ref: IAMF v1.1.0 index.bs:754 ("Parsers SHALL support any value of num_parameters")`
      `// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc ValidateNumParameters (UnimplementedError when num_parameters > kMaxNumParameters; called on write :759 and read :811)`
      Then one comment line: the finding warns a consumer that iamf-tools refuses this Audio Element, while this crate follows the spec and keeps reading and writing it (quick 260914-kfs). Extend the method's `///` doc by one sentence naming this finding.
    - At the read site `:478` (`let params = read_counted(r, read_audio_element_param)?;`), add a plain-comment block directly above the statement, not above the fn, so the fn's citation block is untouched:
      `// No num_parameters cap on read (user decision 2026-09-14, quick 260914-kfs).`
      `// ref: IAMF v1.1.0 index.bs:754 ("Parsers SHALL support any value of num_parameters")`
      `// ref: iamf-tools@v2.1.0 iamf/obu/audio_element.cc ValidateNumParameters (refuses > kMaxNumParameters = 256 before reserve, :811)`
      `// ref: libiamf@v1.1.0 code/src/iamf_dec/IAMF_OBU.c iamf_element_new (no cap; IAMF_MALLOCZ for the read count)`
      Then one line: memory is bounded here by read_counted's bytes_remaining check and bounded_vec's 64 KiB preallocation cap inside a 2 MiB OBU, and AudioElement::validate reports the iamf-tools refusal instead.
    - At the write site `:542` (`write_count(w, v.params.len(), "num_parameters")?;`), add a `// NOTE:` directly above it. iamf-tools@v2.1.0 `ValidateAndWritePayload` also refuses > 256 on write (`audio_element.cc:759`). This writer does not, so parse-then-write stays byte-exact for a spec-legal file (PARSE-04), matching the type-0 precedent. The condition is diagnosed by `AudioElement::validate` (quick 260914-kfs).
    - Grep `docs/iamf/index.bs` for the line "Parsers SHALL support any value of" and use the line number it reports in both spec citations. The plan expects 754; if the number differs, use the real one.
    - Change nothing in `src/model`, `src/sequence.rs`, `src/encoder.rs` or `src/error.rs`, and add no ErrorKind.

    Run `cargo fmt --all`. Commit by explicit path: `git add src/obu/audio_element.rs tests/sequence_parse.rs tests/encoder_builder.rs`. Confirm `git diff --cached --name-only` lists exactly those three.
    Message: `feat(quick-260914-kfs): report Audio Elements with more than 256 params in validate()`.
    Body:
    - user decision 2026-09-14 points (1)-(3)
    - the iamf-tools and libiamf evidence lines
    - why the writer does not refuse (PARSE-04, type-0 precedent)
    - the builder kind precedence for > 256 params
    - the line `No semantic_sha256, golden artifact, fixture, corpus seed or ErrorKind changed.`
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && for t in audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit build_rejects_257_audio_element_params_through_the_element_finding; do grep -q "^test $t \.\.\. FAILED" target/kfs-t1-red.log || { echo "RED missing: $t"; exit 1; }; done && for t in audio_element_with_256_params_has_no_num_parameters_finding build_rejects_audio_element_params_even_when_an_extension_definition_is_opaque; do grep -q "^test $t \.\.\. ok" target/kfs-t1-red.log || { echo "control not green on HEAD: $t"; exit 1; }; done && cargo test --locked --test sequence_parse && cargo test --locked --test encoder_builder && cargo test --locked --test descriptors && cargo test --locked --test parse_reference && cargo test --locked --test golden && cargo test --locked --test citations && cargo test --locked --test public_api && cargo test --locked --lib && grep -q 'IAMF_TOOLS_MAX_NUM_PARAMETERS' src/obu/audio_element.rs && grep -q 'AudioElementObu::kMaxNumParameters' src/obu/audio_element.rs && grep -q 'ValidateNumParameters' src/obu/audio_element.rs && grep -q 'iamf_element_new' src/obu/audio_element.rs && grep -q 'params.len() > IAMF_TOOLS_MAX_NUM_PARAMETERS' src/obu/audio_element.rs && git diff --quiet 2b4c058 -- tests/support/reference_expectations.rs tests/fixtures tests/golden.rs fuzz src/model src/sequence.rs src/encoder.rs src/error.rs DIFF-LEDGER.md && git diff --quiet HEAD -- src tests && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings</automated>
  </verify>
  <done>K1 and K3 failed on HEAD because the finding was missing and K2 and K4 stayed green; all four now pass. A 257-param Audio Element parses, round-trips byte-exactly, and gets exactly one num_parameters Finding from AudioElement, DescriptorSet and ParsedSequence validate. 256 params gets none. The read and write sites and the finding carry cited comments. parse_reference (all semantic_sha256 unchanged), golden, citations, public_api and both clippy runs pass. Exactly three files are committed.</done>
</task>

<task type="auto">
  <name>Task 2: Record the decision in CONFORMANCE-GATE.md and HANDOFF.md, resolve the hoa deferred item, and run the release gates</name>
  <files>CONFORMANCE-GATE.md, HANDOFF.md, .planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md</files>
  <action>
    Use CONFORMANCE-GATE.md for the decision record, not DIFF-LEDGER.md. DIFF-LEDGER.md is the test-asserted CONF-07 byte-offset table and must not be edited.

    (1) `CONFORMANCE-GATE.md`: insert a new top-level section immediately before `## Waivers`, titled `## Reference limits diagnosed, not enforced`. Give it a one-paragraph lead: a pinned reference may impose a resource limit stricter than the spec; such a limit is recorded here with its disposition; it is not a waiver (no CONF clause is downgraded) and not a diff-ledger entry (no byte differs). Then a subsection `### `kMaxNumParameters = 256` — reported by `validate()`, not refused (user decision, 2026-09-14, quick 260914-kfs)` with these parts:
      - **Spec.** `index.bs:751-754`: parsers SHALL support any value of `num_parameters` (use the line number verified in Task 1).
      - **iamf-tools@v2.1.0 (`848c6ff`).** `iamf/obu/audio_element.h:282-287` (the "stricter limits than the specification" class comment and `kMaxNumParameters = 256`). `iamf/obu/audio_element.cc:97-113` `ValidateNumParameters`, with its quoted memory rationale and `absl::UnimplementedError` "Number of parameters exceeds the maximum supported by the decoder" for `num_parameters > 256`. It is called on write (`:759`) and on read (`:811`). It was introduced by commit `feb873de` (2025-11-03), "Limit number of parameters, to fix cases of excessive fuzzer memory."
      - **libiamf@v1.1.0 (`f06e919`).** `code/src/iamf_dec/IAMF_OBU.c:455-480` `iamf_element_new`: no limit, `IAMF_MALLOCZ` for the read count, unknown types skipped.
      - **Decision.** (1) No reject on read. (2) `AudioElement::validate` pushes a Finding at `Field("num_parameters")` for more than 256 params, which also reaches `DescriptorSet::validate` and `ParsedSequence::validate`. (3) Cited comments at the read site, the write site and the finding. (4) The writer does not refuse either, so parse-then-write stays byte-exact (PARSE-04).
      - **Why "keep the stricter rule" does not apply here.** The carried-over policy from 260914-1mq is to satisfy both the spec and the pinned references. This limit is an artificial resource limit, not a format rule. Refusing would reject spec-legal files that libiamf reads. This crate's memory is already bounded without it: `read_counted` checks `bytes_remaining()`, `bounded_vec` caps preallocation at 64 KiB, OBUs are capped at 2 MiB, and `SequenceReader` streams. The user decided the question on 2026-09-14.
      - **Consequence for consumers.** A file with such an Audio Element parses and writes here and in libiamf, but iamf-tools (the CONF-06 oracle) refuses it. The finding is the signal. `EncoderBuilder` never authors Audio Element params, so this crate's encoder cannot emit one.
      - **What asserts it.** `tests/sequence_parse.rs` `audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit` and `audio_element_with_256_params_has_no_num_parameters_finding`; `tests/encoder_builder.rs` `build_rejects_257_audio_element_params_through_the_element_finding`.
      - **Revisit when.** The iamf-tools pin moves and the limit changes or disappears (update the constant, message and tests), or a consumer needs a hard refusal (an opt-in, caller-owned policy, per the HANDOFF.md parse-side memory contract).

    (2) `HANDOFF.md`: in `### Deferred responsibilities`, directly after the bullet "- `EncoderBuilder` output and its error kinds are unchanged.", add a bullet. An Audio Element with more than 256 params still parses, writes and round-trips (IAMF v1.1.0 requires parsers to accept any `num_parameters`). `AudioElement::validate`, `DescriptorSet::validate` and `ParsedSequence::validate` report a `Finding` at `Location::Field("num_parameters")`, because `iamf-tools@v2.1.0` refuses such an Audio Element (`kMaxNumParameters = 256`). `EncoderBuilder::build` already refused every Audio Element param. For more than 256 the element finding is checked first, so the error is `InvalidDescriptorReference` at `Field("descriptors")` rather than `UnsupportedParameterData` at `Field("audio_element_params")`. No `ErrorKind` was added or changed (quick 260914-kfs; decision in `CONFORMANCE-GATE.md`). Also append "(quick 260914-5c5)" to the end of the preceding "- `EncoderBuilder` output and its error kinds are unchanged." bullet, so its scope is explicit and it no longer reads as contradicting the new bullet. Portfolio change control does not apply: no ownership moves and no cross-library object is added.

    (3) `.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md` item 2: prefix it with `**RESOLVED by quick 260914-kfs (user decision 2026-09-14).**`. Append one sentence: not enforced on read or write; `AudioElement::validate` reports more than 256 params at `Field("num_parameters")`; no reference hash changed. Keep the original text. Do NOT stage this file.

    (4) Release gates, all from the repository root:
    - `cargo fmt --check`
    - `cargo build --locked --all-targets`
    - `cargo clippy --locked --all-targets -- -D warnings`
    - `cargo test --locked`
    - `cargo test --locked --features fuzzing --test fuzz_regression`
    - `bash tools/prove-guards.sh` (expect 9 PASS)
    - `bash tools/check-float-escape-census.sh` (expect exactly 1 hit, `lufs_to_q7_8`)
    - `cargo tree -e normal,no-proc-macro` (expect only `iamf` and `thiserror`)
    If any semantic_sha256 or golden assertion fails, stop and report. Do not regenerate.

    Commit by explicit path: `git add CONFORMANCE-GATE.md HANDOFF.md`. Confirm `git diff --cached --name-only` lists exactly those two.
    Message: `docs(quick-260914-kfs): record the kMaxNumParameters decision and the num_parameters finding`.
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && grep -q '^## Reference limits diagnosed, not enforced' CONFORMANCE-GATE.md && [ "$(grep -n '^## Reference limits diagnosed, not enforced' CONFORMANCE-GATE.md | cut -d: -f1)" -lt "$(grep -n '^## Waivers' CONFORMANCE-GATE.md | cut -d: -f1)" ] && grep -q 'feb873de' CONFORMANCE-GATE.md && grep -q 'iamf_element_new' CONFORMANCE-GATE.md && grep -q 'audio_element_with_257_params_parses_and_every_validate_reports_the_iamf_tools_limit' CONFORMANCE-GATE.md && grep -q 'quick 260914-kfs' HANDOFF.md && grep -q 'Field("num_parameters")' HANDOFF.md && grep -q 'RESOLVED by quick 260914-kfs' .planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md && git diff --quiet 2b4c058 -- DIFF-LEDGER.md tests/support/reference_expectations.rs tests/fixtures && git diff --quiet HEAD -- CONFORMANCE-GATE.md HANDOFF.md src tests && cargo fmt --check && cargo build --locked --all-targets && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked && cargo test --locked --features fuzzing --test fuzz_regression && [ "$(bash tools/prove-guards.sh 2>&1 | grep -c PASS)" -ge 9 ] && bash tools/check-float-escape-census.sh && [ "$(cargo tree -e normal,no-proc-macro --prefix none | sed 's/ .*//' | sort -u | tr '\n' ' ')" = "iamf thiserror " ]</automated>
  </verify>
  <done>CONFORMANCE-GATE.md carries the decision record before `## Waivers` with spec, iamf-tools (including feb873de), libiamf evidence, the disposition and the asserting tests. HANDOFF.md states the finding and the builder kind precedence. The hoa deferred item 2 is marked resolved but not staged. Every release gate passes, DIFF-LEDGER.md and the reference expectations are untouched, and exactly two docs files are committed.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| untrusted `.iamf` bytes → `read_audio_element` | attacker-controlled `num_parameters` drives an allocation |
| this crate's model → a consumer exporting for iamf-tools | a file this crate accepts may be refused by the stricter reference |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-kfs-01 | Denial of Service | `read_audio_element` / `read_counted` num_parameters | medium | accept | Deliberately uncapped per user decision (spec SHALL). Allocation is already bounded by the `bytes_remaining()` check, the `bounded_vec` 64 KiB preallocation cap and the 2 MiB OBU cap. `SequenceReader` lets the caller own policy. Recorded in CONFORMANCE-GATE.md |
| T-kfs-02 | Tampering | consumer pipeline passing a > 256-param Audio Element to iamf-tools | low | mitigate | `AudioElement::validate` Finding at `Field("num_parameters")`, reachable from DescriptorSet and ParsedSequence validate, asserted by K1; HANDOFF.md note |
| T-kfs-03 | Repudiation | silent drift if the iamf-tools pin changes the limit | low | mitigate | constant cites `AudioElementObu::kMaxNumParameters`; CONFORMANCE-GATE.md "Revisit when" names the pin move as the trigger |
</threat_model>

<verification>
- K1 and K3 were RED on HEAD (target/kfs-t1-red.log), and K2 and K4 were green on HEAD and after.
- parse_reference passes: no semantic_sha256 changed, because no committed fixture has more than 256 params.
- golden passes, and DIFF-LEDGER.md, src/model, src/sequence.rs, src/encoder.rs and src/error.rs are unchanged from 2b4c058.
- Full gate set: fmt, build, clippy (with and without the fuzzing feature), cargo test, fuzz replay, prove-guards (9 PASS), float census (1 hit), cargo tree (iamf + thiserror).
</verification>

<success_criteria>
- A spec-legal Audio Element with any number of params is still read and written byte-exactly (decision 1).
- More than 256 params yields exactly one `num_parameters` Finding on all three validate surfaces, and 256 yields none (decision 2).
- Read site, write site and finding carry `// ref:` citations to spec, iamf-tools@v2.1.0 and libiamf@v1.1.0 (decision 3).
- CONFORMANCE-GATE.md and HANDOFF.md record the decision, and the hoa deferred item is resolved (decision 4).
- Two commits: `feat(quick-260914-kfs)` (3 files) and `docs(quick-260914-kfs)` (2 files).
</success_criteria>

<output>
Create `.planning/quick/260914-kfs-record-the-kmaxnumparameters-decision-an/260914-kfs-SUMMARY.md` when done (not staged; the
orchestrator commits it). Include a deferred note: spec `index.bs:752-753` num_parameters SHALL be 0-2 for
channel-based and 0 for scene-based elements has no finding today, and it is out of scope for this decision.
</output>
