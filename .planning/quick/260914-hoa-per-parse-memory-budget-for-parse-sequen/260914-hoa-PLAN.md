---
phase: quick-260914-hoa
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/sequence.rs
  - tests/sequence_reader.rs
  - fuzz/fuzz_targets/parse_sequence.rs
  - fuzz/CLAUDE.md
  - HANDOFF.md
  - .planning/codebase/CONCERNS.md
autonomous: true
requirements: [PARSE-01, PARSE-02, SEQ-02, FUZZ-02]

estimate:
  tokens: 150000
  raw_tokens: 150000
  tasks: 3
  confidence: low

must_haves:
  truths:
    - "iamf::sequence::SequenceReader::new(&[0x20,0x00,0x20,0x00,0x20,0x05]) yields Ok(TemporalDelimiter), Ok(TemporalDelimiter), Err(Error::new(UnexpectedEndOfInput, InputOffset(6))), then None and None; byte_position() reads 0, 2, 4, 4, 4; parse_sequence on the same bytes returns that same Err"
    - "On tests/fixtures/reference/test_000129.iamf the reader yields exactly three Ok items (IaSequenceHeader, CodecConfig, AudioElement), then Err(UnexpectedEndOfInput @ InputOffset(53)), then None; byte_position() is 37 after the error, and parse_sequence(&bytes[..37]) is Ok with those same three OBUs"
    - "For every tests/fixtures/**/*.iamf file and every file in fuzz/corpus/parse_sequence and fuzz/artifacts/parse_sequence, SequenceReader::new(b).collect::<Result<Vec<_>>>() == parse_sequence(b).map(|s| s.obus); for Ok inputs the reader's position sequence equals find_obu_boundaries; the test asserts >= 44 inputs, >= 5000 OBUs and that test_000129 is among the Err inputs"
    - "parse_sequence is SequenceReader::new(input) collected into ParsedSequence: no semantic_sha256, golden artifact, fixture, corpus seed, ErrorKind variant or error offset changes; parse_reference, golden, sequence_parse, round_trip and fuzz_regression pass unchanged"
    - "An eager reader, a reader that is not fused after an error, and a reader that does not restore its cursor on error each turn at least one named sequence_reader test red. The logs are recorded, and none of these variants is committed"
    - "32 MiB of 20 00 held in memory measures roughly 3.3 GB max RSS through parse_sequence and stays under 200 MB through SequenceReader when each item is dropped (/usr/bin/time -l, scratch probe outside the repo, recorded in SUMMARY)"
    - "The fuzz target parse_sequence asserts that the reader and the eager parse agree, and the fuzz workspace builds on nightly-2026-09-01"
    - "The parse_sequence and SequenceReader rustdoc, HANDOFF.md and CONCERNS.md state the measured factors: 192-byte SequenceObu, 96x live for 2-byte OBU streams, per-OBU ceiling about 55x of <= 2 MiB, committed fixtures 2.45x aggregate"
  artifacts:
    - path: "src/sequence.rs"
      provides: "pub struct SequenceReader<'a> (new, byte_position, Iterator<Item = Result<SequenceObu>>, FusedIterator); parse_sequence as a collect over it; module doc reader section; memory contract rustdoc"
      contains: "FusedIterator for SequenceReader"
    - path: "tests/sequence_reader.rs"
      provides: "laziness, position, fusing, definition-carry, test_000129 prefix and whole-corpus differential tests with anti-vacuity asserts"
      contains: "fn reader_equals_parse_sequence_on_every_committed_input"
    - path: "fuzz/fuzz_targets/parse_sequence.rs"
      provides: "streamed-vs-eager differential assertion"
      contains: "SequenceReader"
    - path: "HANDOFF.md"
      provides: "Parse-side memory subsection naming SequenceReader and the measured factors"
      contains: "SequenceReader"
    - path: ".planning/codebase/CONCERNS.md"
      provides: "residual (1) with the 96x generic per-OBU class, its resolution by streaming and the updated in-payload classes"
      contains: "260914-hoa"
  key_links:
    - from: "src/sequence.rs parse_sequence"
      to: "SequenceReader::next"
      via: "collect::<Result<Vec<SequenceObu>>>() short-circuits on the first Err and drops the prefix, so the no-public-prefix contract and every error offset are unchanged"
      pattern: "SequenceReader::new\\(input\\)"
    - from: "SequenceReader::next_obu"
      to: "ParamDefinitionRegistry observe_audio_element / observe_mix_presentation / get"
      via: "the registry is a reader field that survives across next() calls, so a Parameter Block after its Mix Presentation is still typed as SequenceObu::ParameterBlock"
      pattern: "registry"
    - from: "tests/sequence_reader.rs differential"
      to: "iamf::obu::find_obu_boundaries"
      via: "an independent header-only walk checks every byte_position() the reader reports"
      pattern: "find_obu_boundaries"
---

<objective>
Bound parse-side memory without changing `parse_sequence`. Research (`260914-hoa-RESEARCH.md`) measured that the largest
amplification is the flat `Vec<SequenceObu>` itself, not any payload class. Each `SequenceObu` is 192 bytes, and the
smallest legal OBU is 2 bytes (`20 00`, `30 00`, `c0 00`). That is 96× live, measured at 3.27-3.66 GB max RSS for 32 MiB of input.
The 2 MiB OBU cap does not limit it. The user delegated the design to research. Research chose option (d′): add a streaming
`SequenceReader` (an additive public API) and re-implement `parse_sequence` as a `collect` over it, with identical
results, errors and offsets. Measured factors are documented alongside it. This is how the pinned references work:
`iamf-tools@v2.1.0` `iamf/api/decoder/iamf_decoder.cc` `DecodeOneTemporalUnit` flushes per temporal unit, and
`libiamf@v1.1.0` `code/include/IAMF_decoder.h` `IAMF_decoder_decode` streams. Neither has a memory budget.

Decisions taken here, all delegated to research and adopted unchanged:
- There is no `ParseLimits` type and no new `ErrorKind`. `parse_sequence` remains unlimited. No accept or reject decision depends
  on `size_of`.
- Streaming is per OBU, not per temporal unit, because `ParsedSequence::temporal_unit_ranges` decides `uses_delimiters`
  over the whole sequence (`src/sequence.rs:413-416`).
- After the first `Err`, the reader restores its cursor to the start of the failing OBU and fuses. `byte_position()` then
  reports that start. This is Claude's discretion: the research sketch says the position stays unchanged after an error,
  and the probe confirmed that the real cursor has already advanced past the header when `read_obu_with` fails.
- `README.md` is not edited. Its "Consumer boundary" section describes only the export seam and never mentions
  `parse_sequence` or `ParsedSequence` (verified by grep), so no statement there goes stale. The orchestrator's condition
  ("if it discusses parsing") is not met. The consumer contract lives in `HANDOFF.md`.
- The fuzz target gains a streamed-vs-eager differential. Stable replay of the corpus and artifacts through the same
  differential lives in `tests/sequence_reader.rs`, so `tests/fuzz_regression.rs` and `src/fuzzing.rs` stay untouched.
- Deferred, not planned: a whole-file `ParseLimits` budget, the `kMaxNumParameters = 256` read limit (a user question),
  registry de-duplication, boxing `SequenceObu` variants, a temporal-unit-grouping reader, and the O(n·m) `registry.get`.

Output: 3 commits (feat, test, docs), plus an uncommitted SUMMARY and deferred-items file.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.claude/CLAUDE.md
@tests/CLAUDE.md
@.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-RESEARCH.md
@src/sequence.rs

Working directory: `/Volumes/lab/talea/iamf-rs`. Base commit: `f24a095`. Work sequentially on `main` with no worktrees.
Toolchain is 1.85.0, so let-chains are not available.

The working tree already contains orchestrator- and user-owned files. Never stage any of them: untracked `.DS_Store`,
`docs/`, `iamf-rs.code-workspace`, `.planning/STATE.md` and everything in this quick-task directory (PLAN, RESEARCH,
SUMMARY, deferred-items). Never run `git add -A`, `git add .` or `git commit -a`. Stage explicit paths, and check
`git diff --cached --name-only` before every commit. Put all logs under `target/`, which is ignored.

Facts verified on HEAD by the planner's probe (scratch crate with a path dependency on this repo):
- `test_000129.iamf` (221 B): `find_obu_boundaries` is Ok `[0, 8, 24, 37, 53, 135, 165, 201, 211, 221]`. `parse_sequence`
  on the whole file gives `Err(UnexpectedEndOfInput @ InputOffset(53))`, on `bytes[..37]` gives Ok with 3 OBUs, and on
  `bytes[..53]` gives the same Err @ 53.
- `20 00 20 00 20 05` → `Err(UnexpectedEndOfInput @ InputOffset(6))`. `[]` → Ok with 0 OBUs.
- `10 0a 01 00 01 00 05 01 80 00 00 00 18 06 05 01 01 00 00 00` → Ok `[MixPresentation, ParameterBlock]`.
- 44 inputs: 40 `tests/fixtures/**/*.iamf` files (golden 1, reference 34, reference/iamf-tools 4, reference/negative 1)
  plus 4 in `fuzz/corpus/parse_sequence`. `fuzz/artifacts/parse_sequence` exists and is empty. Together they hold 5843 OBUs,
  and exactly one input errs (`test_000129`). For every Ok input, `find_obu_boundaries(b).len() == obus.len() + 1`.
- `size_of::<SequenceObu>()` is 192 on x86_64-apple-darwin, and research reports 192 on aarch64-apple-darwin.

API facts: `BitCursor` derives `Debug, Clone`. `bytes_remaining() -> usize` and `byte_position() -> u64`.
`ParamDefinitionRegistry` derives `Debug, Clone, PartialEq, Eq, Default`, and `new()` is `const`. `Error::new(kind, at)` is
`pub const`, `Error` implements `PartialEq`, and its fields are private. `iamf::obu::find_obu_boundaries(&[u8]) ->
Result<Vec<usize>>`. `Error::with_input_base` is `pub(crate)`.
</context>

<source_audit>
| Source | Item | Covered by |
|---|---|---|
| GOAL | Per-parse memory bound for `parse_sequence` (resolved to streaming reader + documented factors) | Tasks 1-3 |
| REQ | PARSE-01 / PARSE-02 (sequence parser, explicit registry) unchanged in behaviour; SEQ-02 streaming shape mirrored on read | Task 1 |
| REQ | FUZZ-02 `parse_sequence` fuzz target | Task 2 |
| RESEARCH | `SequenceReader` + `FusedIterator` + `byte_position`; `parse_sequence` = collect | Task 1 |
| RESEARCH | Q4 tests (laziness, position, definitions carry, differential with anti-vacuity, empty) + guard mutation | Task 1 |
| RESEARCH | Memory evidence: `/usr/bin/time -l` before/after probe | Task 2 |
| RESEARCH | Optional fuzz differential | Task 2 |
| RESEARCH | Docs: rustdoc, module doc, HANDOFF, CONCERNS residual (1) | Task 1 (rustdoc), Task 3 |
| RESEARCH | Docs: README Consumer boundary paragraph | EXCLUDED per orchestrator condition (README does not discuss parsing; see objective) |
| RESEARCH | Deferred list (6 items) | Task 3 deferred-items file (not implemented) |
| CONTEXT | No CONTEXT.md; user delegated to research; deferred items stay deferred | Objective |
</source_audit>

<tasks>

<task type="tracer" tdd="true">
  <name>Task 1: SequenceReader streams parse_sequence one OBU at a time, and parse_sequence collects it (RED first, three mutation proofs)</name>
  <files>tests/sequence_reader.rs, src/sequence.rs</files>
  <behavior>
    - reader_yields_each_obu_before_a_later_error: bytes 20 00 20 00 20 05. Five next() calls return Some(Ok(TemporalDelimiter)), Some(Ok(TemporalDelimiter)), Some(Err(Error::new(ErrorKind::UnexpectedEndOfInput, Location::InputOffset(6)))), None, None. parse_sequence on the same bytes equals Err of that same Error.
    - reader_reports_the_position_of_the_next_obu: same bytes. byte_position() is 0 before any call, 2 after the first, 4 after the second, 4 after the Err and 4 after the following None.
    - reader_streams_the_valid_prefix_of_test_000129: the reader yields Ok IaSequenceHeader, Ok CodecConfig, Ok AudioElement (checked with matches!), then Err(UnexpectedEndOfInput @ InputOffset(53)), then None. byte_position() is 37 after the Err. parse_sequence(bytes.get(..37)) is Ok, and its obus equal the three yielded items.
    - reader_carries_parameter_definitions_across_obus: bytes 10 0a 01 00 01 00 05 01 80 00 00 00 18 06 05 01 01 00 00 00. Collected items are [MixPresentation, ParameterBlock], with the second NOT UngovernedParameterBlock. This is a control that stays green under the eager stand-in.
    - reader_equals_parse_sequence_on_every_committed_input: the differential and anti-vacuity test, specified in the action.
    - reader_on_empty_input_yields_nothing: next() is None, byte_position() is 0, and parse_sequence(&[]) is Ok with 0 OBUs. This is a control.
  </behavior>
  <action>
    Step 1 (RED-1, test file). Create `tests/sequence_reader.rs` with a `//!` header. It covers quick 260914-hoa,
    PARSE-01 and SEQ-02, says the reader must equal `parse_sequence` on everything committed, and names the anti-pattern it
    guards against: a differential that is green because it saw nothing. Import `iamf::sequence::{SequenceReader, SequenceObu,
    parse_sequence}`, `iamf::obu::find_obu_boundaries` and `iamf::{Error, ErrorKind, Location}`. Resolve paths with
    `env!("CARGO_MANIFEST_DIR")`. Write the six tests in the behavior block.

    Build the differential test like this:
    - Collect inputs with a recursive, sorted directory walk. Take files with extension `iamf` under `tests/fixtures`, and every
      regular file under `fuzz/corpus/parse_sequence` and `fuzz/artifacts/parse_sequence`. A missing or empty artifacts directory
      contributes nothing.
    - Helpers return `std::io::Result<Vec<PathBuf>>` and the `#[test]` body calls `expect`. tests/CLAUDE.md allows
      `expect` only inside `#[test]`.
    - For each input, first assert `SequenceReader::new(&bytes).collect::<Result<Vec<SequenceObu>, Error>>()` equals
      `parse_sequence(&bytes).map(|s| s.obus)`, with the path in the message.
    - Then drive a fresh reader by hand. Record `byte_position()` before the loop and after every Ok item, and break on
      the first Err.
    - For an Ok input, the recorded positions, converted with `usize::try_from`, equal `find_obu_boundaries(&bytes)` Ok.
    - For an Err input, `parse_sequence(bytes.get(..pos))` is Ok with obus equal to the yielded Ok items, where `pos` is
      `byte_position()` read after the Err.
    - Count inputs, OBUs and Err inputs with `checked_add`/`saturating_add`. `arithmetic_side_effects` and `indexing_slicing`
      are denied in tests too.
    - After the loop, assert: inputs seen >= 44, `.iamf` files seen >= 40, total OBUs >= 5000, and at least one Err input
      whose path ends with `test_000129.iamf`.
    Record the actual counts in SUMMARY. The probe expects 44, 40, 5843 and 1.

    Run `mkdir -p target && cargo test --locked --no-fail-fast --test sequence_reader > target/hoa-t1-red1.log 2>&1 || true`.
    The log must show `error[E0432]` naming `SequenceReader`. Any other failure is a stop-and-report.

    Step 2 (RED-2, eager stand-in, never committed). This shows that an eager `next()` fails the laziness test. Add a
    temporary `pub struct SequenceReader<'a>` to `src/sequence.rs` that holds the input slice, a position `u64` and a buffered
    queue of `Result<SequenceObu>`. On the first `next()` it calls today's unchanged `parse_sequence(input)`. On Ok it buffers
    every OBU and sets the position to `input.len()`. On Err it buffers only that Err. `next()` then pops from the front, and
    `byte_position()` returns the stored position. Run
    `cargo test --locked --no-fail-fast --test sequence_reader > target/hoa-t1-red2.log 2>&1 || true`. Expected results:
    - FAILED: `reader_yields_each_obu_before_a_later_error`, `reader_reports_the_position_of_the_next_obu`,
      `reader_streams_the_valid_prefix_of_test_000129` and `reader_equals_parse_sequence_on_every_committed_input` (the
      position check).
    - ok: `reader_carries_parameter_definitions_across_obus` and `reader_on_empty_input_yields_nothing`.
    If anything differs, stop and report the excerpt. Do not adapt tests to the stand-in. Step 3 replaces the stand-in
    entirely.

    Step 3 (GREEN) in `src/sequence.rs`.
    - Directly above `parse_sequence`, add `pub struct SequenceReader<'a>` deriving `Debug, Clone`. Its private fields are
      `reader: BitCursor<'a>`, `registry: ParamDefinitionRegistry` and `finished: bool`.
    - Put a plain-comment block above the struct:
      `// ref: iamf-tools@v2.1.0 iamf/api/decoder/iamf_decoder.cc DecodeOneTemporalUnit`
      `// ref: libiamf@v1.1.0 code/include/IAMF_decoder.h IAMF_decoder_decode`
      Follow with a `// NOTE:` saying both references stream and hold only descriptors plus the current unit. This reader
      streams per OBU instead, because unit grouping needs whole-sequence lookahead (`ParsedSequence::temporal_unit_ranges`).
    - Add `impl<'a> SequenceReader<'a>` with `#[must_use] pub const fn new(input: &'a [u8]) -> Self` (drop `const` if
      `BitCursor::new` is not const) and `#[must_use] pub fn byte_position(&self) -> u64`, which delegates to the cursor.
    - Add a private `fn next_obu(&mut self) -> Result<SequenceObu>`. Do not name it `read_*`, so `tests/citations.rs` is
      unaffected. Its body is today's `while` loop body from `parse_sequence`, moved verbatim: the two inspection clones,
      `payload_base`, every `with_input_base` mapping, the registry observe/get calls and the same dispatch arms. The only
      change is that the block returns the `SequenceObu` instead of pushing it. Destructure `let Self { reader, registry, .. } =
      self;` at the top so the moved body keeps its local names.
    - Implement `Iterator for SequenceReader<'_>` with `type Item = Result<SequenceObu>`. `next()` returns `None` when
      `finished` is set or `reader.bytes_remaining() == 0`. Otherwise it clones the cursor as the OBU start and calls
      `next_obu()`. On Ok it returns `Some(Ok(obu))`. On Err it restores the cursor to the saved start, sets `finished = true`
      and returns `Some(Err(error))`.
    - Do not override `size_hint`. The default lower bound of 0 keeps the `collect` growth profile equal to the old push loop.
      Add `impl core::iter::FusedIterator for SequenceReader<'_> {}`.
    - Replace the body of `parse_sequence` with `SequenceReader::new(input)` collected into
      `Result<Vec<SequenceObu>>` and mapped into `ParsedSequence { obus }`. Keep the signature and the existing doc
      paragraphs.

    Rustdoc (same commit, same file):
    - `SequenceReader`: what it yields, in exact wire order, with the same kinds and absolute `InputOffset`s as
      `parse_sequence`. It is fused after the first error, and `byte_position()` then reports the start of the failing OBU.
      `parse_sequence(input)` is exactly this reader collected, so the reader is the primitive.
    - `SequenceReader` memory: the reader holds a borrowed input, a cursor and the parameter-definition registry. The registry
      grows with every definition an Audio Element or Mix Presentation publishes, including redundant copies. One yielded
      item is one OBU of at most 2 MiB `obu_size` (`ENTIRE_OBU_SIZE_MAX`). Its model measured at most about 55× that size
      (about 110 MB, the `read_strings` worst case). Items the caller drops are freed immediately.
    - `SequenceReader` example: a short runnable doc example that iterates `[0x20, 0x00, 0x20, 0x00]` and counts two
      Temporal Delimiters.
    - `parse_sequence` gets a `# Memory` section. It retains every OBU. At the time of writing, each `SequenceObu` is 192
      bytes inline on the 64-bit targets probed (the largest variant is `Obu<AudioElement>`), plus payload copies. This is a
      layout fact, not a guarantee, and no behaviour depends on it. The 40 committed `.iamf` fixtures measure 2.45× input in
      aggregate: 1.2-4.0× for files of at least 2 KB, and up to about 17× for files of 225 B or less. Hostile streams of
      2-byte OBUs (`20 00`, `30 00`, `c0 00`) retain 96× input, measured at 3.27-3.66 GB max RSS for 32 MiB on macOS (quick
      260914-hoa). For untrusted or large input, use `SequenceReader`.
    - `parse_sequence` gets a `# Errors` section: the first error the reader yields, and no partial model.
    - Module doc: change the title line to cover the IA Sequence writer and reader. Add a `# Reading: SequenceReader`
      section citing PARSE-01, PARSE-02, SEQ-02 (the streaming shape, mirrored on read) and quick 260914-hoa. It says the
      streaming reader is the read-side memory ceiling for the same reason the writer is streaming, and gives the per-OBU
      rather than per-unit rationale.

    Run `cargo fmt --all`. Run `cargo test --locked --test sequence_reader` (all 6 ok), `--test parse_reference`,
    `--test golden`, `--test sequence_parse`, `--test round_trip`, `--test citations`, `--doc`, and both clippy runs (with and
    without `--features fuzzing`). If `parse_reference` reports any `semantic_sha256` or negative-expectation mismatch, or
    `golden` fails, stop and report. Research and the probe predict no change.

    Commit by explicit path: `git add src/sequence.rs tests/sequence_reader.rs`, and confirm `git diff --cached --name-only`
    lists exactly those two. Message: `feat(quick-260914-hoa): stream sequence parsing one OBU at a time with
    SequenceReader`. The body gives the research finding (192-byte enum, 96× for 2-byte OBUs), the reference streaming model,
    the reason streaming is per OBU and not per unit, the reason there is no budget and no ErrorKind, and the line
    `No semantic_sha256, golden artifact, fixture, corpus seed, ErrorKind or error offset changed.`

    Step 4 (mutation proofs after the commit; restore after each, commit none).
    (M-fuse) In `next()`, remove only the `finished = true` assignment on the Err path. Run
    `cargo test --locked --no-fail-fast --test sequence_reader > target/hoa-t1-mut-fuse.log 2>&1 || true`. Expected FAILED:
    `reader_yields_each_obu_before_a_later_error` and `reader_streams_the_valid_prefix_of_test_000129`. Then run
    `git restore src/sequence.rs`.
    (M-restore) Remove only the cursor restore on the Err path. Run
    `cargo test --locked --no-fail-fast --test sequence_reader > target/hoa-t1-mut-restore.log 2>&1 || true`. Expected
    FAILED: `reader_reports_the_position_of_the_next_obu` (the cursor sits at 6, not 4). Then run
    `git restore src/sequence.rs`.
    After both, `git diff --quiet HEAD -- src tests` must hold. If a mutation leaves every expected test green, stop and
    report: the guard does not fire. Record the RED-1, RED-2 and both mutation outcomes in SUMMARY. RED-2 is the "eager
    `next()` fails the laziness test" proof.
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && grep -q 'error\[E0432\]' target/hoa-t1-red1.log && grep -q 'SequenceReader' target/hoa-t1-red1.log && for t in reader_yields_each_obu_before_a_later_error reader_reports_the_position_of_the_next_obu reader_streams_the_valid_prefix_of_test_000129 reader_equals_parse_sequence_on_every_committed_input; do grep -q "^test $t \.\.\. FAILED" target/hoa-t1-red2.log || { echo "RED-2 missing: $t"; exit 1; }; done && for t in reader_carries_parameter_definitions_across_obus reader_on_empty_input_yields_nothing; do grep -q "^test $t \.\.\. ok" target/hoa-t1-red2.log || { echo "RED-2 control not ok: $t"; exit 1; }; done && grep -q '^test reader_yields_each_obu_before_a_later_error \.\.\. FAILED' target/hoa-t1-mut-fuse.log && grep -q '^test reader_streams_the_valid_prefix_of_test_000129 \.\.\. FAILED' target/hoa-t1-mut-fuse.log && grep -q '^test reader_reports_the_position_of_the_next_obu \.\.\. FAILED' target/hoa-t1-mut-restore.log && git diff --quiet HEAD -- src tests && cargo test --locked --test sequence_reader && cargo test --locked --test parse_reference && cargo test --locked --test golden && cargo test --locked --test sequence_parse && cargo test --locked --test round_trip && cargo test --locked --test citations && cargo test --locked --doc && grep -q 'FusedIterator for SequenceReader' src/sequence.rs && grep -q '// ref: iamf-tools@v2.1.0 iamf/api/decoder/iamf_decoder.cc DecodeOneTemporalUnit' src/sequence.rs && grep -q '// ref: libiamf@v1.1.0 code/include/IAMF_decoder.h IAMF_decoder_decode' src/sequence.rs && [ "$(awk '/^pub fn parse_sequence/,/^}/' src/sequence.rs | grep -c 'SequenceReader::new(input)')" = "1" ] && [ "$(awk '/^pub fn parse_sequence/,/^}/' src/sequence.rs | wc -l | tr -d ' ')" -le 8 ] && grep -q '96' src/sequence.rs && grep -q '192' src/sequence.rs && git diff --quiet f24a095 -- tests/support/reference_expectations.rs tests/fixtures tests/golden.rs fuzz/corpus fuzz/artifacts src/obu src/bits src/model src/error.rs src/lib.rs src/fuzzing.rs Cargo.toml Cargo.lock && C=$(git log -1 --format=%H --grep='^feat(quick-260914-hoa): stream sequence parsing') && [ -n "$C" ] && F=$(git diff-tree --no-commit-id --name-only -r "$C") && [ "$(printf '%s\n' "$F" | sort | tr '\n' ' ')" = "src/sequence.rs tests/sequence_reader.rs " ] && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings</automated>
  </verify>
  <done>RED-1 (compile failure naming SequenceReader) and RED-2 (the eager stand-in fails the four laziness, position, prefix and differential tests while both controls stay green) are logged. The M-fuse and M-restore mutations each turned their named tests red and were restored. SequenceReader is committed with new, byte_position, Iterator and FusedIterator, and parse_sequence is a collect over it. All six sequence_reader tests pass with >= 44 inputs, >= 5000 OBUs and test_000129 among the Err inputs. parse_reference, golden, sequence_parse, round_trip, citations, doctests and both clippy runs pass. Exactly two files are committed, and the protected paths are unchanged since f24a095.</done>
</task>

<task type="auto">
  <name>Task 2: The fuzz target checks the reader against the eager parse, with measured RSS evidence</name>
  <files>fuzz/fuzz_targets/parse_sequence.rs, fuzz/CLAUDE.md</files>
  <action>
    `fuzz/fuzz_targets/parse_sequence.rs`: keep `#![no_main]` and the `fuzz_target!` shape. In the body, compute the eager
    result with `iamf::sequence::parse_sequence(data)` and the streamed result with
    `iamf::sequence::SequenceReader::new(data)` collected into `Result<Vec<SequenceObu>, iamf::Error>`. Then `assert_eq!`
    the streamed result against the eager result mapped to its `obus`, with a short message naming quick 260914-hoa. A
    divergence therefore aborts the run like a crash. Add one `//` comment explaining why: the nightly discovery job finds
    hostile inputs that the committed corpus does not have.

    `fuzz/CLAUDE.md` `:13`: the `parse_sequence` bullet now says raw bytes go to both `parse_sequence` and `SequenceReader`,
    and any panic or disagreement between them counts as a failure. Leave the seeds sentence and everything else unchanged.

    Fuzz workspace checks:
    1. `cargo +nightly-2026-09-01 check --locked --manifest-path fuzz/Cargo.toml --features roundtrip-model --bins > target/hoa-fuzz-check.log 2>&1`.
       An `iamf` or target compile error is a stop-and-report. A toolchain or C++ environment failure unrelated to this crate
       is recorded as "fuzz workspace build: CI-verified" in SUMMARY.
    2. Best effort, local 60 s run that never touches committed corpus or artifacts:
       - Copy `fuzz/corpus/parse_sequence` into a fresh directory in your session scratchpad, outside the repo.
       - Run `cargo +nightly-2026-09-01 fuzz run parse_sequence <scratch-corpus> -- -max_total_time=60 -artifact_prefix=<scratch>/artifacts/ > target/hoa-fuzz-run.log 2>&1`.
       - A reported crash or assertion failure is a stop-and-report: attach the artifact bytes as hex in the report and do not
         commit them.
       - If the run cannot start for environment reasons (sanitizer or platform), record that in SUMMARY and in deferred item 7.
       - Afterwards, `git status --porcelain -- fuzz` must show only the two edited files.

    RSS evidence, in a scratch crate in your session scratchpad (never in the repo):
    - `Cargo.toml` has an empty `[workspace]` table and `iamf = { path = "/Volumes/lab/talea/iamf-rs" }`, plus a copy of the
      repo's `rust-toolchain.toml`. Build in release.
    - `main` builds 32 MiB in memory as `20 00` repeated 16_777_216 times. Mode `eager` calls `parse_sequence` and prints the
      OBU count. Mode `stream` iterates `SequenceReader`, counts Ok items, drops each immediately, stops on Err and prints
      the count.
    - Run each mode under `/usr/bin/time -l` and record the "maximum resident set size" lines for both. Expected: eager about
      3.3 GB (research: 3.27 GB) and stream under 200 MB, where the input itself is about 32 MB. Both counts must be
      16_777_216.
    - If stream RSS is 200 MB or more, stop and report. That falsifies research assumption A3 and the docs in Task 3 would
      be wrong.
    - Put both numbers, the machine arch (`uname -m`) and the commands in SUMMARY. Task 3 quotes the stream number.

    Run `cargo test --locked --features fuzzing --test fuzz_regression` (unchanged, must pass). Commit by explicit path:
    `git add fuzz/fuzz_targets/parse_sequence.rs fuzz/CLAUDE.md`, and confirm `git diff --cached --name-only` is exactly
    those two. Message: `test(quick-260914-hoa): fuzz SequenceReader against parse_sequence`. The body records the local
    check and run outcome and the measured RSS pair.
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && grep -q 'SequenceReader::new(data)' fuzz/fuzz_targets/parse_sequence.rs && grep -q 'parse_sequence(data)' fuzz/fuzz_targets/parse_sequence.rs && grep -q 'assert_eq!' fuzz/fuzz_targets/parse_sequence.rs && grep -q 'SequenceReader' fuzz/CLAUDE.md && test -f target/hoa-fuzz-check.log && ! grep -q 'error\[E' target/hoa-fuzz-check.log && git diff --quiet HEAD -- fuzz && git diff --quiet f24a095 -- fuzz/corpus fuzz/artifacts fuzz/Cargo.toml fuzz/Cargo.lock fuzz/fuzz_targets/obu_roundtrip.rs fuzz/CORPUS.md tests/fuzz_regression.rs src/fuzzing.rs && C=$(git log -1 --format=%H --grep='^test(quick-260914-hoa): fuzz SequenceReader') && [ -n "$C" ] && F=$(git diff-tree --no-commit-id --name-only -r "$C") && [ "$(printf '%s\n' "$F" | sort | tr '\n' ' ')" = "fuzz/CLAUDE.md fuzz/fuzz_targets/parse_sequence.rs " ] && cargo test --locked --features fuzzing --test fuzz_regression</automated>
  </verify>
  <done>The parse_sequence fuzz target asserts that the streamed and eager parses agree, and fuzz/CLAUDE.md says so. The fuzz workspace check passed or is recorded as CI-verified, with no iamf compile error. The 60 s run either found nothing or its environment blocker is recorded, and committed corpus and artifacts are untouched. Eager and stream RSS for 32 MiB of 20 00 are measured and recorded, with stream under 200 MB. fuzz_regression passes. Exactly two files are committed.</done>
</task>

<task type="auto">
  <name>Task 3: Document the parse-side memory contract, write deferred items, run every release gate</name>
  <files>HANDOFF.md, .planning/codebase/CONCERNS.md, .planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md</files>
  <action>
    `HANDOFF.md`: insert a new `### Parse-side memory` subsection immediately before `### Public-example audit`. Edit nothing
    else. Content, as short prose and bullets:
    - The additive API is `iamf::sequence::SequenceReader` (`new`, `byte_position`, `Iterator<Item = Result<SequenceObu>>`,
      fused after the first error). `parse_sequence` is unchanged in signature, results, error kinds and offsets, and is now
      the reader collected. No `ErrorKind` was added or changed.
    - Measured factors (quick 260914-hoa):
      - `parse_sequence` retains one 192-byte `SequenceObu` per OBU on the 64-bit targets probed, plus payload copies.
      - Committed fixtures measure 2.45× input in aggregate (1.2-4.0× for files of at least 2 KB).
      - Hostile 2-byte OBU streams retain 96× input, 3.27-3.66 GB max RSS for 32 MiB.
      - Through `SequenceReader` with items dropped, the same 32 MiB measured `<Task 2 stream RSS>`.
      - One item is at most about 55× of an OBU of up to 2 MiB (about 110 MB).
      - The parameter-definition registry still grows with every published definition, including redundant copies.
    - Guidance for consumers reading untrusted or large files (`iamf-decode-rs`, `iamf-isobmff-rs`, a Parallax import):
      use `SequenceReader` and keep only what is needed. The caller owns resource policy, for example stopping on an OBU
      count or on `byte_position()`, just as the pinned `iamf-tools` and `libiamf` decoders have no library-side budget.
      `ParsedSequence::validate()` and `temporal_unit_ranges()` still need the whole collected model.
    - Portfolio change control does not apply: the addition is local, moves no ownership and adds no cross-library object.

    `.planning/codebase/CONCERNS.md`, section "Allocation amplification from count fields", `Residuals` bullet (1) (`:56`).
    Edit only that bullet and keep residual (2) as it is. Rewrite (1) to say:
    - The flat model stores one `SequenceObu` (192 B) per OBU. This generic per-OBU class is the dominant amplification:
      2-byte OBUs (`20 00`, `30 00`, `c0 00`) give 96× live and 3.27-3.66 GB max RSS at 32 MiB. Nothing but input length
      bounds it, because the 2 MiB cap is per OBU. It was not in earlier residual lists.
    - Resolved by quick 260914-hoa as streaming, not as a budget. `SequenceReader` yields one OBU at a time, and the same
      32 MiB measured `<Task 2 stream RSS>` with items dropped. `parse_sequence` keeps the whole-file factor and documents it.
    - The in-payload classes are linear and capped per OBU. Measured live factors: `read_strings` about 42× (about 50× with
      malloc rounding), AE extension params about 34×, Mix Gain Step subblocks and layouts about 17×, substream ids about 4×.
      Single 2 MiB OBU peaks reach about 55× (`read_strings`).
    - Keep the existing sentences about extension definitions never governing a block (260914-5c5) and about
      `ParameterData::Raw` being reachable only through a caller-registered `ParameterDataContext::Reserved`.
    - Replace the "not re-measured" sentence and the "per-parse memory budget ... open user decision (260914-1mq deferred
      item 1)" sentence with this: a whole-file `ParseLimits` budget is deferred (260914-hoa deferred item 1), and the
      registry still grows with redundant descriptor copies (260914-hoa deferred item 3).

    Commit by explicit path: `git add HANDOFF.md .planning/codebase/CONCERNS.md`, and confirm `git diff --cached --name-only`
    is exactly those two. Message: `docs(quick-260914-hoa): document the parse-side memory contract and SequenceReader`.

    Create, and do NOT commit, `.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md`
    headed `# 260914-hoa deferred items`, numbered:
    1. **Whole-file `ParseLimits` / `parse_sequence_with_limits`.** A unit-table byte budget (never `size_of`) with a new
       `ErrorKind::ParseBudgetExceeded`, charged at about 20 allocation sites across `src/obu/*`. Only worth doing if a consumer
       needs a hard cap on a retained whole-file model. Reference bases for numbers: `kEntireObuSizeMaxTwoMegabytes` (`iamf/obu/types.h:32`)
       and the 4 MiB `StreamBasedReadBitBuffer` source cap (`iamf/common/read_bit_buffer.cc:509-512`).
    2. **`kMaxNumParameters = 256` on Audio Element read.** iamf-tools@v2.1.0 enforces it (`iamf/obu/audio_element.h:287`,
       `ValidateNumParameters` `audio_element.cc:97-113`, called at `:811`), while the spec says parsers SHALL support any value. It
       would cap the AE-param classes (34-50× per OBU). This is a user question: whether "stricter wins" applies to a resource limit.
       It also needs a reference-hash check.
    3. **Registry de-duplication.** `ParamDefinitionRegistry::register` pushes duplicates (`src/obu/param_definition.rs:89-94`),
       so redundant descriptor copies grow the registry linearly even inside `SequenceReader`, by about 72 B per 12 wire bytes.
       `entries()` is public and documented as "including duplicates".
    4. **Boxing `SequenceObu` variants** (192 → about 16 B inline, about 96× → about 40×). Breaking public change.
    5. **Temporal-unit streaming reader** mirroring `SequenceWriter`. Blocked by whole-sequence `uses_delimiters` in
       `temporal_unit_ranges` (`src/sequence.rs:413-416`).
    6. **O(n·m) `registry.get` linear scan** (`param_definition.rs:177-181`). CPU cost, not memory.
    7. **Evidence limits.** The 192-byte figure was probed only on macOS (aarch64 per research, x86_64 per planner). It is
       documentation, not asserted by a test, because asserting a layout would make a test depend on `size_of`. RSS was
       measured only on macOS. A copying `realloc` (e.g. Windows) could transiently touch about 192× (research A2, ASSUMED).
       Record here whether Task 2's local 60 s fuzz run ran or was left to CI.

    Gates, from `/Volumes/lab/talea/iamf-rs`, with logs under `target/`:
    1. `cargo fmt --all -- --check`
    2. `cargo build --locked --all-targets`
    3. `cargo clippy --locked --all-targets -- -D warnings`
    4. `cargo clippy --locked --all-targets --features fuzzing -- -D warnings`
    5. `cargo test --locked > target/hoa-full.log 2>&1; echo "full_exit=$?" >> target/hoa-full.log`
    6. `cargo test --locked --features fuzzing --test fuzz_regression`
    7. `bash tools/prove-guards.sh > target/hoa-guards.log 2>&1`, which must print `guardrail proof: 9 passed, 0 failed`
    8. `bash tools/check-float-escape-census.sh` exits 0 (exactly the `src/model/loudness.rs` hit)
    9. `cargo tree --locked -e normal,no-proc-macro --prefix none | cut -d' ' -f1 | sort -u` is exactly `iamf` and `thiserror`
    10. Protected paths unchanged since base: `git diff --quiet f24a095 -- tests/support/reference_expectations.rs tests/fixtures tests/golden.rs DIFF-LEDGER.md README.md fuzz/corpus fuzz/artifacts fuzz/Cargo.toml fuzz/Cargo.lock tools src/bits src/obu src/model src/encoder.rs src/dump.rs src/error.rs src/lib.rs src/fuzzing.rs Cargo.toml Cargo.lock`
    11. Committed scope: `git diff --name-only f24a095 HEAD > target/hoa-committed.txt` lists only the six planned files.

    If fmt or clippy needs a fix, make a follow-up commit by explicit path titled `style(quick-260914-hoa): ...`, and amend
    nothing. If any gate fails for another reason, stop and report the log excerpt. Do not edit fixtures, goldens, reference
    expectations, DIFF-LEDGER.md, README.md or the portfolio document. Do not stage `.planning/STATE.md` or any quick-directory file.
  </action>
  <verify>
    <automated>cd /Volumes/lab/talea/iamf-rs && D=.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md && test -f "$D" && grep -q 'kMaxNumParameters' "$D" && grep -q 'ParseLimits' "$D" && grep -q 'temporal_unit_ranges' "$D" && grep -q 'realloc' "$D" && grep -q '### Parse-side memory' HANDOFF.md && grep -q 'SequenceReader' HANDOFF.md && grep -q '96' HANDOFF.md && grep -q '260914-hoa' .planning/codebase/CONCERNS.md && grep -q 'SequenceReader' .planning/codebase/CONCERNS.md && grep -q '192' .planning/codebase/CONCERNS.md && git diff --quiet HEAD -- HANDOFF.md .planning/codebase/CONCERNS.md src tests fuzz && cargo fmt --all -- --check && cargo build --locked --all-targets && cargo clippy --locked --all-targets -- -D warnings && cargo clippy --locked --all-targets --features fuzzing -- -D warnings && { cargo test --locked > target/hoa-full.log 2>&1; echo "full_exit=$?" >> target/hoa-full.log; } && grep -q '^full_exit=0$' target/hoa-full.log && [ -z "$(grep '^test result: ' target/hoa-full.log | grep -v '^test result: ok')" ] && cargo test --locked --features fuzzing --test fuzz_regression && bash tools/prove-guards.sh > target/hoa-guards.log 2>&1 && grep -q 'guardrail proof: 9 passed, 0 failed' target/hoa-guards.log && bash tools/check-float-escape-census.sh && [ "$(cargo tree --locked -e normal,no-proc-macro --prefix none | cut -d' ' -f1 | sort -u | tr '\n' ' ')" = "iamf thiserror " ] && git diff --quiet f24a095 -- tests/support/reference_expectations.rs tests/fixtures tests/golden.rs DIFF-LEDGER.md README.md fuzz/corpus fuzz/artifacts fuzz/Cargo.toml fuzz/Cargo.lock tools src/bits src/obu src/model src/encoder.rs src/dump.rs src/error.rs src/lib.rs src/fuzzing.rs Cargo.toml Cargo.lock && git diff --name-only f24a095 HEAD > target/hoa-committed.txt && [ -z "$(grep -vE '^(src/sequence\.rs|tests/sequence_reader\.rs|fuzz/fuzz_targets/parse_sequence\.rs|fuzz/CLAUDE\.md|HANDOFF\.md|\.planning/codebase/CONCERNS\.md)$' target/hoa-committed.txt)" ]</automated>
  </verify>
  <done>HANDOFF.md has a Parse-side memory subsection naming SequenceReader, the measured factors (including the Task 2 stream RSS) and consumer guidance. CONCERNS residual (1) records the 96x generic per-OBU class, its resolution by streaming and the updated in-payload factors, with residual (2) unchanged. Both files are committed by explicit path. The deferred-items file exists with seven items and is not committed. fmt, build, both clippy runs, the full offline suite (every binary ok), fuzz replay, prove-guards (9 passed), the float census and the cargo tree check pass. Protected paths are unchanged since f24a095, and only the six planned files are committed across the three commits.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| foreign .iamf bytes -> parse_sequence / SequenceReader | Attacker-controlled OBU count, obu_size and payload bytes decide how much model memory is retained |
| iamf -> consumers (iamf-decode-rs, iamf-isobmff-rs, Parallax import) | Parse results, error kinds/offsets and the new streaming API are part of the consumer contract |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-hoa-01 | Denial of Service (memory amplification, CWE-400/770) | `parse_sequence` flat `Vec<SequenceObu>` (96x for 2-byte OBUs) | high | mitigate | Task 1 adds `SequenceReader`, which retains nothing the caller drops. Task 2 measures stream RSS for 32 MiB of `20 00` below 200 MB. Rustdoc and HANDOFF steer untrusted input to the reader |
| T-hoa-02 | Denial of Service (residual per-OBU and registry growth) | one OBU of at most 2 MiB up to about 55x; registry duplicates | medium | accept | Bounded per item by the existing 2 MiB cap. Registry growth is linear in published definitions and recorded in CONCERNS and deferred items 1-3. Resource policy is the consumer's, as in the pinned references |
| T-hoa-03 | Tampering (silent behaviour drift in parse results/offsets) | `parse_sequence` re-implemented as collect | high | mitigate | The whole-corpus differential compares Ok models and Err kind+location on 44 inputs, with anti-vacuity asserts. parse_reference semantic hashes, golden, sequence_parse and round_trip are unchanged, and a mismatch is stop-and-report. The fuzz target asserts agreement on novel inputs |
| T-hoa-04 | Tampering (garbage items after an error) | reader continuing from an undefined cursor | medium | mitigate | Fused on the first Err, with the cursor restored to the failing OBU start. The M-fuse and M-restore mutations prove the tests catch both omissions |
| T-hoa-05 | Denial of Service (panic) | `next_obu`, `next`, test helpers | medium | mitigate | The body moves verbatim from checked code. No indexing, bare arithmetic or unwrap, enforced by both clippy runs and prove-guards |
| T-hoa-06 | Information disclosure / contract drift | `ErrorKind`, `size_of::<Error>()` | low | mitigate | No ErrorKind added or changed and `src/error.rs` is a protected path (gate 10). No accept/reject decision depends on `size_of` |
| T-hoa-SC | Tampering (supply chain) | package installs | low | accept | No dependency added. `Cargo.toml`, `Cargo.lock` and the fuzz lock are protected paths. The RSS probe lives outside the repo |
</threat_model>

<verification>
- RED evidence: `target/hoa-t1-red1.log` (E0432 naming `SequenceReader`) and `target/hoa-t1-red2.log` (eager stand-in: 4 FAILED, 2 controls ok). Mutation evidence: `target/hoa-t1-mut-fuse.log` and `target/hoa-t1-mut-restore.log`.
- `cargo fmt --all -- --check`, `cargo build --locked --all-targets`, and `cargo clippy --locked --all-targets -- -D warnings` with and without `--features fuzzing`.
- `cargo test --locked` passes in every binary, including `sequence_reader`, `parse_reference`, `golden`, `sequence_parse`, `round_trip`, `citations`, `public_api`, `error_shape` and doctests. `cargo test --locked --features fuzzing --test fuzz_regression` passes.
- `bash tools/prove-guards.sh` reports 9 passed. `bash tools/check-float-escape-census.sh` exits 0. `cargo tree -e normal,no-proc-macro` lists only `iamf` and `thiserror`.
- `cargo +nightly-2026-09-01 check --locked --manifest-path fuzz/Cargo.toml --features roundtrip-model --bins` passes, or is recorded as CI-verified for an environment reason.
- RSS pair recorded: eager about 3.3 GB, stream under 200 MB for 32 MiB of `20 00`.
- Protected paths unchanged since `f24a095`, and only six files committed.
</verification>

<success_criteria>
- A consumer can parse an arbitrarily large or hostile IA Sequence while holding only the registry plus the OBUs it chooses to keep, and the measured RSS for the 96x class confirms it.
- `parse_sequence` returns byte-for-byte identical models, error kinds and error offsets on every committed input, and all reference hashes and the golden fixture are unchanged.
- The laziness, fusing and cursor-restore properties are guarded by tests proven to fire against an eager, an unfused and an unrestored implementation.
- The measured amplification factors and the streaming guidance are written down in rustdoc, HANDOFF.md and CONCERNS.md, and every deferred alternative is recorded.
</success_criteria>

<output>
Create, and do NOT commit (the orchestrator handles them):
- `.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-SUMMARY.md`: RED-1/RED-2 and mutation outcomes, differential counts (inputs, `.iamf` files, OBUs, Err inputs), the RSS pair with arch and commands, the fuzz check and run status, and the gate results
- `.planning/quick/260914-hoa-per-parse-memory-budget-for-parse-sequen/260914-hoa-deferred-items.md` (content specified in Task 3)
</output>
