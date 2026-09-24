---
phase: quick-260924-tvf
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - tests/refvectors.rs
  - tests/support/vector_ledger.rs
  - CONFORMANCE-GATE.md
  - .github/workflows/reference.yml
autonomous: true
requirements: [CONF-10, CONF-11, OBU-08, PARSE-04, GUARD-06]

estimate:
  tokens: 95000
  raw_tokens: 95000
  tasks: 3
  confidence: low

must_haves:
  truths:
    - "`cargo test --locked --test refvectors -- --nocapture` passes offline on this working tree (no `.reference/` present) and prints BOTH a `refvectors[vendored]: walked 35 .iamf, 34 paired` line and a `SKIP refvectors[corpus]:` line — it never passes silently with nothing walked"
    - "The harness classifies every file through ONE walker used at two roots: the vendored root `tests/fixtures/reference` (always on, all four targets) and the fetched corpus root `.reference/libiamf/tests` (gated). The gated branch is therefore not dead code — the same `walk()` is exercised offline against 35 real foreign files"
    - "`is_valid: false` in a paired textproto is recorded, never enforced: the harness's only hard accept/reject failure is the RED cell `is_valid: true AND we reject structurally`. On the vendored 34 pairs that cell is empty today (12 declare `is_valid: false`; 11 of them parse here; the one structural reject, test_000129, declares `is_valid: false`)"
    - "A parse-then-write byte difference is classified, not failed: identical / canonicalized (a re-parse of our output yields the same debug-plus-findings projection — the documented non-minimal obu_size caveat) / diverged (projection differs — HARD FAIL)"
    - "A file that parses cleanly must also walk to its own length (`find_obu_boundaries().last() == Some(len)`), so the parse path and the framing path disagreeing about the same bytes is a named failure rather than two independently green tests"
    - "`tests/support/vector_ledger.rs` commits one row per top-level vendored `.iamf` (35 rows) and `tests/refvectors.rs` asserts that table is an exact bijection with what the walker produced — a disposition drift is a reviewable red diff, in the style of `tests/support/reference_expectations.rs`"
    - "Every path in `STRICTER_THAN_REFERENCE` has a matching `CONFORMANCE-GATE.md` subsection under `## Reference limits diagnosed, not enforced`; `DIFF-LEDGER.md` is not touched (its rows are asserted as an exact set in both directions at tests/conformance.rs:2963)"
    - "`.github/workflows/reference.yml` fails the Linux job when the corpus harness skips there, and that guard is PROVEN to fire: running its exact grep against the offline log (which does contain `SKIP refvectors[corpus]`) succeeds, so the failure branch is demonstrably reachable"
    - "Zero new dependencies, zero new vendored bytes, one `[[test]]` entry in Cargo.toml (unchanged), no ignore attribute, no change under `src/`. `cargo clippy --locked --all-targets -- -D warnings` is clean and `bash tools/prove-guards.sh` still reports 9 PASS"
  artifacts:
    - path: "tests/refvectors.rs"
      provides: "new auto-discovered test binary: corpus discovery, textproto flag scanner, four-cell disposition classifier, boundary and round-trip cross-checks, printed table plus counts"
      contains: "refvectors[corpus]: walked"
    - path: "tests/support/vector_ledger.rs"
      provides: "committed per-vector disposition table plus the STRICTER_THAN_REFERENCE exemption list"
      contains: "VENDORED_DISPOSITIONS"
    - path: "CONFORMANCE-GATE.md"
      provides: "the stricter-than-libiamf policy and the measured result, under the existing 'Reference limits diagnosed, not enforced' section"
      contains: "260924-tvf"
    - path: ".github/workflows/reference.yml"
      provides: "the non-skip assertion for the corpus harness in the Linux reference job"
      contains: "refvectors"
  key_links:
    - "tests/refvectors.rs -> tests/support/vector_ledger.rs via a path-attribute module include; support/ is not a crate, so a helper exists only in the binaries that include it"
    - "tests/refvectors.rs -> tools/build-reference.sh: the corpus root constant `.reference/libiamf/tests` is owned by that script (REPO_ROOT at :36, REF_DIR at :39, SRC at :40, and `$SRC/tests/test_000003.iamf` at :267). If the script moves REF_DIR or SRC, the constant moves with it"
    - "tests/refvectors.rs -> .github/workflows/reference.yml: the workflow greps the exact printed markers `refvectors[corpus]: walked` and `SKIP refvectors[corpus]`. Changing either string breaks the CI gate silently — they are a contract"
    - "STRICTER_THAN_REFERENCE -> CONFORMANCE-GATE.md: one section per listed path; the list is the machine-readable half of that document"
---

<objective>
Close the "a corpus harness that skips is a green harness" hole for the pinned `libiamf@v1.1.0`
reference vectors: a new gated test binary that walks every reachable reference vector, joins each
one against its paired upstream `.textproto` disposition, classifies it into a four-cell matrix,
proves parse-then-write byte handling, commits the result as a drift-detecting ledger, and is
asserted in CI to have actually run.

Purpose: today the suite proves round-trip over 40 vendored files and framing over the same 40, and
**nothing reads the upstream `is_valid` declaration at all**. This slice adds that join, and makes it
reusable over the ~221-file fetched corpus where the real coverage is.

Output: `tests/refvectors.rs`, `tests/support/vector_ledger.rs`, a `CONFORMANCE-GATE.md` section and
one CI step. Nothing under `src/`. No new dependency. No new vendored byte.
</objective>

<execution_context>
@~/.claude/gsd-core/workflows/execute-plan.md
@~/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@/Users/cell/talea/iamf-rs/CLAUDE.md
@/Users/cell/talea/iamf-rs/tests/CLAUDE.md
@/Users/cell/talea/iamf-rs/.planning/quick/260924-tvf-libiamf-vector-harness-run-the-full-pinn/260924-tvf-RESEARCH.md

Read these for shape before writing a line — they are the patterns this plan copies:
@/Users/cell/talea/iamf-rs/tests/refcorpus.rs
@/Users/cell/talea/iamf-rs/tests/support/reference_expectations.rs
@/Users/cell/talea/iamf-rs/tests/parse_reference.rs
</context>

<facts_verified_at_planning_time>
Live observations made against this working tree on 2026-09-25. These are the edit and verification
authority for this plan. Do not re-derive them, and do not trust an older note that disagrees.

| Fact | How it was observed |
|---|---|
| `.reference/` does not exist here — `ls -d .reference` fails. The corpus is unreachable at execution time, so every corpus-touching assertion must be gated | `ls -d .reference` |
| `tests/fixtures/reference/` top level: **35** `.iamf` and **34** `.textproto`; the 34 pair 1:1 by stem; `test_000076_aac_lc.iamf` is the one unpaired `.iamf`; every `.textproto` has an `.iamf` | per-stem shell loop over the directory |
| Subdirectories are NOT parallel: `iamf-tools/` holds 4 `.iamf` plus `test_000003.textproto` and `test_000134.textproto` (the **new**-dialect copies, one of them with no sibling `.iamf`); `negative/` holds 1 `.iamf` and no textproto. 35 + 4 + 1 = 40 total | `ls iamf-tools/ negative/` |
| Across the 34 vendored textprotos: 22 `is_valid: true`, **12** `is_valid: false`, 28 `is_valid_to_decode: true`, 6 false. Both keys are flat two-space-indented scalars inside the leading `test_vector_metadata` block | `grep -h is_valid` piped through `sort \| uniq -c`; head of `test_000003.textproto` |
| The corpus root is `<repo>/.reference/libiamf/tests`, owned by `tools/build-reference.sh`: `REPO_ROOT` at `:36`, `REF_DIR="$REPO_ROOT/.reference"` at `:39`, `SRC="$REF_DIR/libiamf"` at `:40`, `SMOKE_IAMF="$SRC/tests/test_000003.iamf"` at `:267` | read of the script |
| `tests/fixtures/MANIFEST.md:343` records a counted table: `libiamf@v1.1.0 tests/` holds **221** `.iamf`, **221** `.textproto` and 403 `.wav`. That is a committed in-tree count, not a number copied from the todo | read of MANIFEST.md |
| `.github/workflows/reference.yml` has exactly **12** step lines matching `^      - name:`, plus one `- uses:` at `:45`. The FLAC/Opus non-skip step is `:84-92`; the manifest assertion is `:98-99`; the reference-gated suite runs at `:101-102`; `target/reference-out/` is already in the uploaded artifact list at `:217` | read of the workflow |
| `CONFORMANCE-GATE.md` `## Reference limits diagnosed, not enforced` starts at `:646`; the complete `kMaxNumParameters` template entry is `:652-690`; `## Waivers` follows at `:692` | `grep -n '^#'` plus read |
| `tools/prove-guards.sh` sets `EXPECTED_PASS=9` (`:58`), asserts a pass count of 9 (`:334`), and copies only `Cargo.toml`, `Cargo.lock`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml` and `src/` into the proof crate | read of the script |
| `tests/reference_manifest.rs` asserts manifest keys by substring containment for `iamfdec_sha256`, `host_triple`, `dep_codecs_disabled`, `aac_reference_enabled`, and scans SHAs line-wise | read of the test |
| `python3 -c "import yaml"` fails here — there is no PyYAML, so the workflow edit cannot be validated by a YAML parser. Use the structural step-count check in task 3 instead | executed |
| Working tree is clean apart from this untracked planning directory | `git status --short` |
</facts_verified_at_planning_time>

<planner_decisions>
Two places where this plan deliberately departs from `260924-tvf-RESEARCH.md`. Both are recorded here
so the executor does not "restore" them.

1. **No `libiamf_src` key is added to `.reference-manifest.json`, and `tools/build-reference.sh` is not
   touched.** The research suggested deriving the corpus root from an additive manifest key. The path
   is already deterministic and single-owner (`REPO_ROOT/.reference/libiamf`), the harness would need
   a hardcoded fallback for older manifests anyway, and two resolution paths where one suffices is
   complexity bought for nothing. Keeping the 368-line reference build script out of the blast radius
   is worth more than the indirection. The constant carries a comment naming the script lines that own
   the path.

2. **The harness runs its walker at the vendored root too, always on, on all four targets.** The
   locked scope describes the corpus-gated layer only. Without the vendored layer the entire classifier
   would be code that never executes outside one Linux job, the committed ledger of task 2 would have
   no data to commit at execution time (the corpus is unreachable here), and a zero-row ledger is
   exactly the "green because it sees nothing" defect this repository forbids. Same walker, second
   root, about thirty extra lines. It duplicates nothing: `refcorpus.rs` asserts framing only and
   `parse_reference.rs` asserts a semantic digest; neither reads the upstream `is_valid` field.
</planner_decisions>

<tasks>

<task type="tracer">
  <name>Task 1: One walker, two roots — the refvectors harness, green offline</name>
  <files>tests/refvectors.rs</files>
  <read_first>
`tests/refcorpus.rs` (the corpus-walk shape, its total-helper discipline and its module-doc voice);
`tests/parse_reference.rs` lines 92-128 (the debug-plus-findings projection at `:96` and the
`write_parsed_sequence` round-trip at `:120-126`); `tests/conformance.rs` lines 179-211 (the
print-SKIP-and-return convention and the narrow local clippy allow with a comment, for non-test
helpers); `tests/conformance.rs` line 1251 (`manifest_raw_value`, the line-scanner shape to copy);
`tests/CLAUDE.md` section "Lints in tests".
  </read_first>
  <action>
Create `tests/refvectors.rs` as a new integration-test binary. Rely on Cargo auto-discovery and leave
`Cargo.toml` alone: `tools/prove-guards.sh` copies only `src/` into its proof crate and then runs
clippy with all targets there, so an unconditional manifest test entry pointing at a `tests/` file
the proof directory does not have would break all nine guard cases.

Open with a module doc in the voice of `tests/refcorpus.rs`. It must state: the purpose (joining each
reference vector against its paired upstream textproto disposition); the requirement and decision IDs
served (CONF-10, CONF-11, OBU-08, PARSE-04, GUARD-06, D-14, D-25); why this is a new binary rather
than an addition to `tests/conformance.rs` (that file is the decode-oracle concern and is already the
largest in the repo), to `tests/refcorpus.rs` (framing only) or to `tests/vectors.rs` (hand-computed
bit primitives under D-25 — a name trap, not the vector corpus); why it prints a skip line and returns
rather than using the ignore attribute (there is none anywhere in `tests/*.rs`; the convention is
`tests/conformance.rs:179-203`); and that a harness which skips is indistinguishable from one that
passes, which is why `.github/workflows/reference.yml` greps the printed markers.

CONSTANTS. `VENDORED_ROOT` = "tests/fixtures/reference" and `CORPUS_ROOT` = ".reference/libiamf/tests",
both joined onto the cargo manifest directory. Comment on `CORPUS_ROOT` that `tools/build-reference.sh`
owns that path (REPO_ROOT at :36, REF_DIR at :39, SRC at :40, smoke input at :267) and that a new
environment variable was deliberately not introduced for it. `MIN_CORPUS_VECTORS` = 200usize, with a
comment citing `tests/fixtures/MANIFEST.md:343` (221 `.iamf` and 221 `.textproto` counted at the
pinned tag), stating that this is a deliberately conservative non-skip floor rather than a census, and
that it should be tightened to the observed count after the first green reference-job run.

TYPES. Three small enums, each deriving Debug, Clone, Copy, PartialEq and Eq, each with a total
`tag()` returning a static string: `Parse` with Clean, Findings, Rejected tagged "clean", "findings",
"rejected"; `Walk` with EndsOnLength, Mismatch, Error tagged "ends-on-length", "mismatch", "error";
`RoundTrip` with Identical, Canonicalized, Diverged, NotApplicable tagged "identical",
"canonicalized", "diverged", "n-a". One `Verdict` struct holding parse, `findings: usize`, walk,
round_trip, `is_valid: Option<bool>` and `is_valid_to_decode: Option<bool>`.

HELPERS. Every one total — returning Option, Vec or Verdict, never panicking — because clippy.toml's
unwrap, expect and panic carve-out reaches inside test function bodies only. Where one genuinely
cannot be total, give it a narrow local allow with a comment stating the reason, in the style of
`tests/conformance.rs:206-211`. Do not put a file-level allow at the top of this file.

  - `iamf_files_in(dir)` — NON-recursive, `iamf` extension, sorted, empty vector when the directory is
    unreadable. Comment why non-recursive: under the vendored root the `iamf-tools/` subdirectory holds
    new-dialect textprotos whose stems collide with different top-level `.iamf` files, so a recursive
    pairing would join a file to the wrong metadata, and `negative/` carries no metadata at all.
  - `paired_textproto(path)` — the sibling with the `textproto` extension, Some only when it is a file.
  - `scan_flag(text, key)` — the field scanner, in the spirit of `manifest_raw_value`
    (`tests/conformance.rs:1251`): the first line whose trimmed form strips the exact prefix formed by
    the key followed by a colon, then trimmed and matched against the two boolean spellings, else None.
    Comment that this exact-colon strip is what stops a lookup of the shorter key from being satisfied
    by the longer `is_valid_to_decode` key — the single most load-bearing detail in the scanner.
  - `projection(sequence)` — the debug-plus-findings string, the same shape as the semantic projection
    at `tests/parse_reference.rs:96`, used ONLY to tell a canonicalisation from a divergence.
  - `verdict_for(bytes, metadata)` — total. Runs `find_obu_boundaries` for the Walk value: an error
    gives Error; a first boundary of 0 with a last boundary equal to the byte length gives
    EndsOnLength; anything else gives Mismatch. Runs `parse_sequence`: an error gives Parse::Rejected
    with RoundTrip::NotApplicable; success gives Clean or Findings from the length of `validate()`,
    then `write_parsed_sequence` into a fresh vector. Equal bytes give Identical. Unequal bytes compute
    the projection of the original parse and of a re-parse of our own output: equal projections give
    Canonicalized (PARSE-04's documented legal foreign non-minimal `obu_size` width caveat, recorded as
    the Phase 02 decision in `.planning/STATE.md:117`); unequal projections, a failed re-parse or a
    write error give Diverged. Build the projections ONLY on the unequal branch — the corpus holds
    files above 14 MB and formatting a whole parsed sequence per file would dominate the runtime. Read
    the two flags from the metadata text with `scan_flag`, leaving both None when there is no paired
    textproto.
  - `walk(root)` — returns None when the root is not a directory (that is the skip signal), otherwise a
    vector of file-name and Verdict pairs in sorted order.
  - `print_table(label, rows)` — one deterministic line per file carrying the name, both flags (a dash
    for None), the parse tag, the finding count, the walk tag and the round-trip tag; then a summary
    block crossing the `is_valid` flag against the parse tag, which is the four-cell matrix; then the
    count line. The two count lines are a CI contract and must read exactly
    `refvectors[vendored]: walked {n} .iamf, {p} paired` and
    `refvectors[corpus]: walked {n} reference vectors`. Comment that
    `.github/workflows/reference.yml` greps these literals.
  - `violations(rows, stricter_allowed)` — returns a vector of messages, one per breach, each naming
    the file and the rule. Exactly three rules and no others:
      V1, the RED cell — the `is_valid` flag is present and true and the parse was rejected, unless the
          file name appears in `stricter_allowed`. The message must say this is either a real parser
          bug or a deliberate stricter-than-libiamf decision that needs a `CONFORMANCE-GATE.md` entry
          and a `STRICTER_THAN_REFERENCE` row.
      V2 — the round trip diverged.
      V3 — the parse succeeded (clean or with findings) while the walk did not end on the file length,
          meaning the parse path and the framing path disagree about the same bytes.
    Comment explicitly that a false `is_valid` flag is NOT a rule: it is a semantic declaration about
    a deliberately generated vector, not an instruction to reject. 12 of the 34 vendored textprotos
    declare it and 11 of those parse cleanly here, so an equality rule would be red on 11 files on day
    one and would pressure toward loosening a correct parser.

TESTS, three of them.
  - `every_vendored_pair_is_classified` — walk the vendored root; assert the result is present; assert
    35 files with 34 of them paired; print the table with the label "vendored"; assert the violations
    vector is empty, putting the messages in the assert payload.
  - `the_reference_corpus_agrees_with_its_own_textprotos` — walk the corpus root. Absent: print
    `SKIP refvectors[corpus]: ` followed by a reason naming `bash tools/build-reference.sh` as the way
    to fetch it and saying this is the expected offline state (CONF-10), then return. Present: print
    the table with the label "corpus", assert the row count is at least `MIN_CORPUS_VECTORS`, assert
    the violations vector is empty.
  - `the_flag_scanner_reads_the_paired_metadata_block` — a pure unit test of `scan_flag` over an inline
    metadata snippet where the shorter key is false and the longer one is true, asserting the two are
    read independently and that an absent key yields None.

Thread `stricter_allowed` as an empty slice at both call sites; task 2 replaces it with the committed
list. Do not modify `tests/vectors.rs`, `tests/refcorpus.rs`, `tests/conformance.rs`, `DIFF-LEDGER.md`,
`Cargo.toml` or anything under `src/`.
  </action>
  <verify>
    <automated>cd /Users/cell/talea/iamf-rs && SCRATCH=/private/tmp/claude-501/-Users-cell-talea-iamf-rs/92e01600-baa5-499c-887e-d727d0406ee2/scratchpad && cargo test --locked --test refvectors -- --nocapture --test-threads=1 2>&1 | tee "$SCRATCH/refvectors-offline.log" | tail -40 && grep -q 'refvectors\[vendored\]: walked 35 .iamf, 34 paired' "$SCRATCH/refvectors-offline.log" && grep -q 'SKIP refvectors\[corpus\]' "$SCRATCH/refvectors-offline.log" && grep -q '^\[\[test\]\]' Cargo.toml && test -z "$(grep -n '^\[\[test\]\]' Cargo.toml | sed -n '2p')" && cargo clippy --locked --all-targets -- -D warnings</automated>
  </verify>
  <done>
`cargo test --locked --test refvectors -- --nocapture` passes offline and prints the vendored table,
the four-cell summary, `refvectors[vendored]: walked 35 .iamf, 34 paired` and `SKIP refvectors[corpus]:`.
Clippy with all targets and denied warnings is clean. `Cargo.toml` still holds exactly one `[[test]]`
entry. No file under `src/` changed.
  </done>
</task>

<task type="auto">
  <name>Task 2: Commit the dispositions — vector_ledger.rs plus the CONFORMANCE-GATE section</name>
  <files>tests/support/vector_ledger.rs, tests/refvectors.rs, CONFORMANCE-GATE.md</files>
  <read_first>
`tests/support/reference_expectations.rs` (the committed-table style, its module doc about provenance,
and the static-string disposition field at `:210-217`); `tests/parse_reference.rs:45-81` (the bijection
assertion to mirror); `CONFORMANCE-GATE.md:646-690` (the section opener and the complete
`kMaxNumParameters` template: Spec, iamf-tools, libiamf, Decision, Consequence for consumers, What
asserts it, Revisit when).
  </read_first>
  <action>
First run the task 1 harness with output uncaptured and READ the printed vendored table. The ledger is
transcribed from that output plus the upstream textprotos. Do not invent a row, and do not reorder the
walker's sorted output.

Create `tests/support/vector_ledger.rs`. Module doc must state, plainly: the two flag columns are
transcribed from the paired upstream textprotos and are authoritative; the parse, walk and round-trip
columns are this crate's OWN behaviour, committed as a change detector under D-25 and therefore never
conformance evidence on their own, so that a drift becomes a reviewable diff instead of a silently
changed printout; and that every entry in `STRICTER_THAN_REFERENCE` must have a matching
`CONFORMANCE-GATE.md` subsection. Contents: a `VectorDisposition` struct with `path`, `is_valid` and
`is_valid_to_decode` as optional booleans, and `parse`, `walk` and `round_trip` as static strings
holding the tag values from task 1, plus a `findings` count; `VENDORED_DISPOSITIONS`, one row per
top-level vendored `.iamf`, 35 of them, in the walker's sorted order; and `STRICTER_THAN_REFERENCE`, a
static-string slice naming every file in the RED cell. Expect that list to be EMPTY on this tree — the
one structural reject among the vendored files, `test_000129.iamf`, declares the false flag, so it is
not a red cell. Keep helpers out of this file entirely; it is data.

Then edit `tests/refvectors.rs`: include the ledger with a path-attribute module declaration next to
the other test-support includes, pass `STRICTER_THAN_REFERENCE` instead of the empty slice at both
`violations` call sites, and add a fourth test,
`the_vendored_ledger_is_a_bijection_with_what_the_walker_produced`. It asserts the committed row count
is 35, that the sorted set of ledger paths equals the sorted set of walked file names (so an added or
dropped fixture fails here rather than shrinking the check), and that every column of every row equals
the walked verdict, naming the file and the column in the failure message. Also assert that every entry
of `STRICTER_THAN_REFERENCE` is a path that exists in the ledger, so the exemption list cannot name a
file that is not walked.

Then add one subsection to `CONFORMANCE-GATE.md` under the existing `## Reference limits diagnosed, not
enforced` heading, placed after the `kMaxNumParameters` entry and before `## Waivers`. Title it for the
reverse direction, naming quick task 260924-tvf and the date. Open with one sentence extending the
section's scope: the section already records a pinned reference being stricter than the spec; this
entry records the mirror case, where this crate is stricter than the permissive `libiamf@v1.1.0`
reader. Then cover, using the template fields: the four-cell matrix and the policy that a false
`is_valid` is a semantic declaration about a deliberately generated vector and never an instruction to
reject, with the measured evidence (12 of 34 vendored textprotos declare it; 11 of those parse here;
the single structural reject also declares it); the measured result at this commit, naming the number
of red cells found over the vendored 34 and stating that the ~221-file corpus is measured in the Linux
reference job; one fully templated entry for each red cell IF the run found any, each with its Spec,
iamf-tools, libiamf, Decision, Consequence for consumers, What asserts it and Revisit when fields;
"What asserts it" naming `tests/refvectors.rs` and `tests/support/vector_ledger.rs` by test name; and
"Revisit when" pointing at a move of the `libiamf` pin. Do NOT add a row to `DIFF-LEDGER.md` — its rows
are asserted as an exact set in both directions at `tests/conformance.rs:2963`, so an extra row fails
that test.

If the vendored run turns up a surprise — a file whose `is_valid` flag is true while we emit findings,
or a canonicalized round trip — record it as a row like any other AND describe it in one sentence in
the new `CONFORMANCE-GATE.md` subsection. Do not loosen a check to make a surprise disappear.
  </action>
  <verify>
    <automated>cd /Users/cell/talea/iamf-rs && cargo test --locked --test refvectors -- --nocapture --test-threads=1 2>&1 | tail -40 && test "$(awk '/VectorDisposition \{/{n++} END{print n+0}' tests/support/vector_ledger.rs)" -ge 35 && grep -q '260924-tvf' CONFORMANCE-GATE.md && grep -q '^## Reference limits diagnosed, not enforced' CONFORMANCE-GATE.md && test -z "$(grep -n '^## Reference limits diagnosed, not enforced' CONFORMANCE-GATE.md | sed -n '2p')" && test -z "$(git diff --name-only -- DIFF-LEDGER.md)" && cargo test --locked && cargo clippy --locked --all-targets -- -D warnings</automated>
  </verify>
  <done>
All four `refvectors` tests pass offline. `tests/support/vector_ledger.rs` holds 35 committed rows and
an exemption list whose every entry exists in the ledger. `CONFORMANCE-GATE.md` carries the new
subsection under the existing "Reference limits diagnosed, not enforced" heading, naming 260924-tvf.
`DIFF-LEDGER.md` is untouched. The whole suite and clippy are green.
  </done>
</task>

<task type="auto">
  <name>Task 3: The CI non-skip gate, proven to fire</name>
  <files>.github/workflows/reference.yml</files>
  <read_first>
`.github/workflows/reference.yml` lines 79-103 — the "CONF-05 may not skip here" step at `:84-92` is
the model to mirror, the manifest assertion at `:98-99` must stay first, and the artifact upload at
`:208-219` already collects `target/reference-out/`.
  </read_first>
  <action>
Insert exactly one new step into the single `reference` job, between the "Assert the manifest matches
REFERENCES.md" step (`:98-99`) and the "Run the reference-gated test suite" step (`:101-102`). That
position is deliberate: the manifest drift assertion must remain the first thing that runs, because a
suite green against the wrong reference is worse than no suite.

Above the step, write a comment block, in the voice of the `:79-83` comment, saying: the corpus
harness prints a skip line and passes when `.reference/libiamf/tests` is absent, which is correct
offline (CONF-10) and is also indistinguishable from a pass, so this job — the one place the corpus
exists — must fail if it skips; and that the walked-count floor itself lives in the
`MIN_CORPUS_VECTORS` constant in `tests/refvectors.rs` so there is exactly one owner of that number,
with this step guarding only the skip path.

The step itself: name it for the assertion it makes, mirroring the wording of the CONF-05 step. Its
script sets the strict shell options including pipefail, makes `target/reference-out/` if needed, runs
the harness with `cargo test --locked --test refvectors -- --nocapture --test-threads=1` piped through
`tee` into `target/reference-out/refvectors.log`, then fails with a named diagnostic if the log lacks
the `refvectors[corpus]: walked` marker, fails with a named diagnostic if the log carries the
`SKIP refvectors[corpus]` marker, and otherwise echoes the matched walked-count line so the number
lands in the job log. Note in the comment that the log file is picked up by the existing artifact
upload. Escape the square brackets in every grep pattern so they are literals, not a character class.

Match the file's existing indentation exactly: step keys at six spaces, and the run block as a literal
scalar. There is no YAML parser on this machine, so the structural check in the verify block (the step
count rising from 12 to 13) is what proves the step is a sibling of the others rather than nested
inside one.
  </action>
  <verify>
    <automated>cd /Users/cell/talea/iamf-rs && SCRATCH=/private/tmp/claude-501/-Users-cell-talea-iamf-rs/92e01600-baa5-499c-887e-d727d0406ee2/scratchpad && test "$(awk '/^      - name:/{n++} END{print n+0}' .github/workflows/reference.yml)" -eq 13 && grep -n 'refvectors' .github/workflows/reference.yml && awk '/Assert the manifest matches REFERENCES.md/{m=NR} /refvectors/{if(!r)r=NR} /Run the reference-gated test suite/{s=NR} END{exit !(m<r && r<s)}' .github/workflows/reference.yml && cargo test --locked --test refvectors -- --nocapture --test-threads=1 2>&1 | tee "$SCRATCH/refvectors-offline.log" >/dev/null && grep -q 'SKIP refvectors\[corpus\]' "$SCRATCH/refvectors-offline.log" && echo "GUARD-FIRES: the CI skip-detection pattern matches the offline log, so its failure branch is reachable" && cargo test --locked && cargo clippy --locked --all-targets -- -D warnings && bash tools/prove-guards.sh 2>&1 | tail -3</automated>
  </verify>
  <done>
`.github/workflows/reference.yml` has 13 named steps, the new one sitting between the manifest
assertion and the reference-gated suite. The exact grep the new step uses to detect a skip is proven
to match the offline log, so the guard's failure branch is reachable rather than theoretical. The full
suite, clippy with denied warnings, and `tools/prove-guards.sh` at 9 PASS are all green.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| network -> `.reference/libiamf/tests` | `tools/build-reference.sh` clones a foreign repository at a pinned SHA; every byte the new harness reads from that tree is untrusted input |
| corpus file -> `parse_sequence` | ~221 foreign `.iamf` files, including ones deliberately generated as invalid, cross into this crate's parser |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-tvf-01 | Denial of Service | `verdict_for` over a 14 MB corpus file | low | mitigate | The debug projection is built only when the written bytes differ from the input, so the expensive formatting path is not taken per file; each parsed model is dropped before the next file is read |
| T-tvf-02 | Tampering | corpus provenance | medium | mitigate | The corpus is materialised only by `tools/build-reference.sh` at the SHA pinned in `REFERENCES.md`, and `tests/reference_manifest.rs` runs before this harness in the reference job (workflow `:98-99`), so a drifted checkout fails first |
| T-tvf-03 | Information Disclosure | committed ledger | low | accept | The ledger records file names, upstream boolean flags and our own disposition tags — no vendored bytes, so `tests/fixtures_cap.rs` and D-14 are untouched |
| T-tvf-SC | Tampering | package-manager installs | n/a | accept | This slice adds ZERO dependencies. Nothing is installed, so the package legitimacy gate has no input; the linked graph stays `iamf` plus `thiserror` |
</threat_model>

<verification>
Run all of these from the repository root before committing:

1. `cargo test --locked --test refvectors -- --nocapture --test-threads=1` — passes, prints the
   vendored table, `refvectors[vendored]: walked 35 .iamf, 34 paired`, and `SKIP refvectors[corpus]:`.
   A run that prints neither count line is the defect this task exists to prevent.
2. `cargo test --locked` — the whole suite green offline, with no reference binary and no container.
3. `cargo clippy --locked --all-targets -- -D warnings` — clean. Watch for unwrap, expect, panic,
   indexing and arithmetic in non-test helpers: the carve-out reaches inside test bodies only.
4. `bash tools/prove-guards.sh` — final line reads `all 9 guardrails fired.`
5. `bash tools/check-float-escape-census.sh` — still exactly one hit, in `src/model/loudness.rs`.
6. `cargo tree -e normal,no-proc-macro` — lists only `iamf` and `thiserror`.
7. `awk '/^      - name:/{n++} END{print n+0}' .github/workflows/reference.yml` — prints 13 (it was 12).
8. `git diff --stat` — touches only `tests/refvectors.rs`, `tests/support/vector_ledger.rs`,
   `CONFORMANCE-GATE.md` and `.github/workflows/reference.yml`. Nothing under `src/`, nothing in
   `Cargo.toml`, nothing in `DIFF-LEDGER.md`, no new vendored fixture.

Committing: stage explicit paths only. Never `git add -A` or `git add .` — `docs/` holds untracked
upstream clones that must not be staged. Check `git diff --cached --name-only` before every commit.
Suggested atomic commits: one per task, scoped `test(260924-tvf):`, `test(260924-tvf):` and
`ci(260924-tvf):`.
</verification>

<success_criteria>
- A new test binary walks every reachable reference vector, joins it to its paired upstream textproto
  disposition, and classifies it into the four-cell matrix.
- The classifier runs offline on all four targets against 35 real foreign files, so the corpus-gated
  branch is exercised code rather than a promise.
- The upstream `is_valid` flag is recorded and never enforced as equality; only the red cell fails.
- Parse-then-write is classified as identical, canonicalized or diverged, with only divergence fatal.
- The dispositions are committed, so a drift is a reviewable diff.
- The Linux reference job fails if the corpus harness skips there, and that guard is proven reachable.
- Zero new dependencies, zero new vendored bytes, no change under `src/`.
</success_criteria>

<notes>
## Follow-up work this slice deliberately does NOT do

These were split out by the research and confirmed by the user's locked scope. They are NOT started,
not stubbed and not prepared for. The executor must carry this list verbatim into `SUMMARY.md` so it
survives (the 1-3 task budget is fully spent, so they are not written into `.planning/todos/pending/`
here; file them from the SUMMARY afterwards).

1. **Encoder conformance from each `.textproto`** — phase-sized, not a quick task. It needs a
   hand-rolled reader for the OLD `UserMetadata` proto dialect (libiamf's textprotos still carry
   `count_label`, `num_substreams`, `num_layers`, `num_sub_mixes`, `num_audio_elements`, `num_layouts`
   and `param_definition_size`, per `tests/fixtures/MANIFEST.md:65-71`), no textproto crate is on the
   `deny.toml` allow-list, the existing `textproto_for` writer handles exactly one Codec Config, one
   Audio Element and one Mix Presentation, and every non-LPCM vector is permanently unencodeable here
   because this crate frames pre-encoded access units and never encodes them.
2. **The `coverage.csv` per-spec-field report** — a new quick task, and only after this ledger exists,
   so the report has a committed floor to assert against instead of being a printout nobody reads.
3. **Test-scaffolding consolidation** (the originating todo's items 2 and 3) — its own quick task, and
   never in the same commit as this harness: a diff that both ADDS and MOVES test surface makes any
   regression ambiguous between "the new harness found a real bug" and "the refactor dropped an
   assertion". Start with the five true duplicates (`sha256_hex`, `rust_files_under`, `repo_root`,
   `sha_on_line_naming`, `PartialThenFail`), and budget for the clippy test carve-out boundary: a
   panicking assertion moved out of a test body into a shared helper trips denied warnings.

## Things that will bite

- The floor of 200 is conservative on purpose. The first green Linux reference run prints the true
  walked count; tighten `MIN_CORPUS_VECTORS` to it in a follow-up, with the observed number cited.
- The first corpus run may surface dispositions nobody has seen (canonicalized round trips, findings on
  files declared valid). Those are outcomes to record, never reasons to loosen a check.
- The printed markers are a contract with the CI step. Renaming either string silently disarms the
  gate.
</notes>

<output>
Create `.planning/quick/260924-tvf-libiamf-vector-harness-run-the-full-pinn/260924-tvf-SUMMARY.md`
when done, including the three follow-up items from the notes section verbatim and the vendored
four-cell counts the harness printed.
</output>
