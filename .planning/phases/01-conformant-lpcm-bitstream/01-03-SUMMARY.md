---
phase: 01-conformant-lpcm-bitstream
plan: 03
subsystem: bitstream
tags: [rust, bits, uleb128, proptest, bitstream-io, tdd, citations, iamf]

requires:
  - "01-01 — the lint set, the D-08/D-09 error surface (Error/ErrorKind/Location), the `// ref:` citation rule, tools/cargo-test-tap.sh"
  - "01-02 — the vendored reference fixture corpus, in particular tests/fixtures/reference/test_000003.iamf"
provides:
  - "`iamf::bits::BitCursor<'a>` — the hand-rolled read half: read_unsigned, read_signed, read_bool, read_string, read_uint8_span, read_uleb128, bits_remaining, bytes_remaining, is_byte_aligned, byte_position, sub_reader"
  - "`iamf::bits::BitWriter` — the write half, mirroring the reader method for method: write_unsigned, write_signed, write_bool, write_string, write_bytes, write_uleb128_minimal, len_bytes, is_byte_aligned, finish"
  - "`bits::leb128` — minimal_len, write_uleb128_minimal, write_uleb128_fixed (pub(crate), D-03), read_uleb128 with both reference caps enforced"
  - "tests/vectors.rs — 31 hand-computed BITS-06/D-25 vectors keyed to real offsets in test_000003.iamf, written before the encoder existed"
  - "tests/bits_oracle.rs — D-01's differential oracle: four proptest properties plus the exhaustive uleb128 group boundaries"
  - "tests/citations.rs — D-23's mechanical `// ref:` walker, which now polices every later plan"
  - "Six new ErrorKind variants consumed, three added (UnsupportedWidth, ValueExceedsWidth, StringHasInteriorNul, Leb128SizeInvalid)"
affects:
  - "01-04 onward — every OBU read/write is expressed in these primitives, and every Error.at value flows out of them"
  - "01-05/01-06 — sub_reader is Pattern 3's bounded payload view; write_uleb128_minimal is Pattern 1's two-pass obu_size emitter"
  - "Phase 2 PARSE-04 — the decoder already accepts non-minimal uleb128, proven by an in-crate vector"

actuals:
  tokens: 19089
  tasks: 3
  commits: 6

plan_head_before: 4ca3ed20e2becd9bd2c4cc5f222f12e2d1f9d33c

tech-stack:
  added:
    - "hex-literal 1.1.0 (MIT OR Apache-2.0) — dev-only, hand-computed vectors"
    - "proptest 1.11.0 (MIT OR Apache-2.0) — dev-only, MSRV 1.85 exactly at GUARD-12's floor"
    - "bitstream-io 4.10.0 (MIT/Apache-2.0) — dev-only, the differential oracle and nothing else"
  patterns:
    - "One private `read_bit`/`write_bit` per direction: bit order is decided once per direction and cannot drift between primitives"
    - "Minimal uleb128 is fixed-size uleb128 at minimal_len — one encoder loop, and minimality is structural rather than emergent from a loop condition"
    - "sub_reader is expressed in terms of read_uint8_span, so the bounds-check-before-reserve rule has one implementation"
    - "Two independent derivations that must agree: hand-computed vectors as the primary proof, the oracle as the second assert"
    - "A citation is the plain-comment block above a function, `// ref:` plus an optional `// NOTE:` rider where the reference's own comment misleads"

key-files:
  created:
    - src/bits/mod.rs
    - src/bits/reader.rs
    - src/bits/writer.rs
    - src/bits/leb128.rs
    - tests/vectors.rs
    - tests/bits_oracle.rs
    - tests/citations.rs
    - .planning/phases/01-conformant-lpcm-bitstream/deferred-items.md
  modified:
    - src/lib.rs
    - src/error.rs
    - Cargo.toml
    - Cargo.lock

key-decisions:
  - "The plan's `cargo tree -e normal` gate is the wrong instrument for the second time in this phase; `-e normal,no-proc-macro` is what measures D-02's claim, and it returns exactly `iamf` + `thiserror`."
  - "`minimal_len` got a production caller by making `write_uleb128_minimal` call `write_uleb128_fixed` at the minimal length. This is what LebGenerator itself does, removes a duplicated loop, and resolved a dead-code error without an `#[allow]`."
  - "`read_string` returns the NUL and `write_string` appends it. The asymmetry is deliberate and documented: the reader shows what was on the wire, the writer makes an unterminated string unrepresentable."
  - "`read_uint8_span` and `sub_reader` require byte alignment. A borrowed `&[u8]` has no sub-byte start, so this is forced rather than chosen, and it matches the reference's own byte-aligned payloads."
  - "The `// ref:` walker searches the whole comment block above a function, not only the last line, because Pitfall F's `// NOTE:` rider sits after the citation and wraps."
  - "`read_bit`/`write_bit` cite the reference functions that CONTAIN the per-bit logic, with a NOTE saying the reference inlines what this crate factors out. Inventing a same-named reference symbol would have been the exact failure D-23 exists to prevent."

patterns-established:
  - "In a compiled language the RED commit lands the type shape plus argument validation, so the tests compile, discover and fail on assertions rather than on a build error — which proves nothing. Established in 01-01, applied three times here."
  - "A mechanical check must test its own failure mode. tests/citations.rs asserts that the walker catches an uncited function and names it, and that it finds a non-zero number of functions at all."
  - "Verification instruments get corrected in the summary when they are wrong, rather than worked around silently."

requirements-completed:
  - BITS-01
  - BITS-02
  - BITS-03
  - BITS-04
  - BITS-05
  - BITS-06
  - BITS-07

coverage:
  - id: D1
    description: "One value travels the whole layer: write_unsigned(31,5) plus three clear flags emits 0xF8, and the cursor reads it back"
    requirement: BITS-01
    verification:
      - kind: unit
        ref: "tests/vectors.rs#offset_0x00_ia_sequence_header_header_byte_writes_as_f8 / _reads_back"
        status: pass
      - kind: integration
        ref: "xxd tests/fixtures/reference/test_000003.iamf byte 0 == f8"
        status: pass
    human_judgment: false
  - id: D2
    description: "The reference's primitive surface is mirrored one for one, read and write adjacent in the same order"
    requirement: BITS-02
    verification:
      - kind: unit
        ref: "tests/vectors.rs — 31 tests covering unsigned, signed 9/16, bool, string, span, sub_reader, uleb128"
        status: pass
      - kind: integration
        ref: "cargo test --release — all 8 targets green, so overflow behaviour is identical in both profiles"
        status: pass
    human_judgment: false
  - id: D3
    description: "uleb128 emits minimal form only through the public path, and both reference caps are typed located errors"
    requirement: BITS-03
    verification:
      - kind: unit
        ref: "tests/vectors.rs#read_uleb128_rejects_a_ninth_continuation_byte / _a_value_above_u32_max / uleb128_round_trips_at_the_u32_boundary"
        status: pass
      - kind: unit
        ref: "src/bits/leb128.rs tests — minimal_len at every seven-bit boundary; fixed-size 6-in-5-bytes still decodes to 6"
        status: pass
    human_judgment: false
  - id: D4
    description: "The fixed-size encoder exists, is pub(crate), and the crate exposes no public leb128 mode argument"
    requirement: BITS-04
    verification:
      - kind: integration
        ref: "grep -rn 'pub fn write_uleb128_fixed\\|LebMode' src/ -> no matches; the fn is `pub(crate)`"
        status: pass
    human_judgment: false
  - id: D5
    description: "Alignment is observable and enforced: is_byte_aligned, NotByteAligned on finish and on an unaligned sub_reader"
    requirement: BITS-05
    verification:
      - kind: unit
        ref: "tests/vectors.rs#writer_byte_alignment_tracks_the_partial_byte, #sub_reader_refuses_an_unaligned_parent"
        status: pass
    human_judgment: false
  - id: D6
    description: "Hand-computed vectors exist and pass BEFORE any OBU type exists, and were written before the implementation"
    requirement: BITS-06
    verification:
      - kind: integration
        ref: "git log: test(01-03) e9dd7f7 and 4dfdffe precede feat(01-03) 5408fe4 and debd741; RED gate RED_EVIDENCE_OK on both"
        status: pass
      - kind: unit
        ref: "cargo test --test vectors -> 31 passed"
        status: pass
    human_judgment: false
  - id: D7
    description: "No foreign error type crosses the bits boundary, and the oracle crate never reaches the linked graph"
    requirement: BITS-07
    verification:
      - kind: integration
        ref: "cargo tree -e normal,no-proc-macro --prefix none | sort -u -> 2 lines, both matching ^(iamf|thiserror)"
        status: pass
      - kind: integration
        ref: "grep -rlE '^\\s*use\\s+bitstream_io' src/ | wc -l -> 0"
        status: pass
      - kind: integration
        ref: "cargo deny check licenses -> licenses ok, with all three dev-dependencies present"
        status: pass
    human_judgment: false
  - id: D8
    description: "Our primitives agree with a 50M-download reference implementation bit for bit on random input"
    requirement: BITS-01
    verification:
      - kind: unit
        ref: "tests/bits_oracle.rs -> 5 passed (write equality incl. padding, stepwise read equality, signed both directions, uleb128 length + round trip)"
        status: pass
    human_judgment: false
  - id: D9
    description: "Every fn read_*/write_* under src/ carries a `// ref:` citation, enforced mechanically and provably able to fail"
    requirement: BITS-02
    verification:
      - kind: unit
        ref: "tests/citations.rs -> 3 passed; 17 functions checked, 0 missing; the walker's own failure mode is asserted"
        status: pass
    human_judgment: false
  - id: D10
    description: "The module a fuzzer reaches first carries no panic path, no raw index and no unchecked arithmetic"
    requirement: BITS-07
    verification:
      - kind: integration
        ref: "production code under src/bits/ grepped for unwrap()/expect(/..] -> 0; grep -rn 'allow(' src/ -> 0"
        status: pass
      - kind: integration
        ref: "cargo clippy --all-targets -- -D warnings -> clean (indexing_slicing, arithmetic_side_effects, unwrap_used, expect_used, panic all at deny)"
        status: pass
    human_judgment: false

duration: 47min
completed: 2026-09-08
status: complete
---

# Phase 1 Plan 03: Bit-Level I/O Layer Summary

**A hand-rolled `BitCursor`/`BitWriter` over `&[u8]` plus uleb128 both ways, proven first against 31 hand-computed vectors decoded from `test_000003.iamf` before the encoder existed, then a second time against `bitstream-io` as a proptest differential oracle that never enters the linked graph — with D-23's citation walker now live and policing every later plan.**

## Performance

- **Duration:** 47 min
- **Tasks:** 3 of 3
- **Commits:** 6 (measured — see below)
- **Files created/modified:** 12

## Accomplishments

- **D-01's justification actually holds, because the oracle exists.** Hand-rolling a bit cursor is only defensible if it is made to agree with something that has millions of downloads of exercise behind it. `tests/bits_oracle.rs` does that on random input: our writer's bytes must equal `bitstream-io`'s **including the zero padding of the final partial byte**, our reads must match at every step of a random width schedule, signed widths 2..=16 must agree in both directions, and every `u32` must round-trip through uleb128 at a length derived by counting seven-bit groups rather than by the range table the encoder itself uses. All five pass. The oracle is imported from exactly one file and `cargo tree -e normal,no-proc-macro` is still exactly `iamf` + `thiserror`.

- **The vectors were written before the code, and they are hand-decoded, not captured.** D-25 rejects capture-first goldens because they prove you match a blob you never understood. Every expected byte in `tests/vectors.rs` was worked out from the two's-complement, the seven-bit groups, or the bit packing, and keyed to a real offset: `0xF8` at `0x00` is an IA Sequence Header with all three flags clear; `c8 01` at `0x0A` is `codec_config_id` 200; `ca 5b` at `0x74` is `integrated_loudness` −13733; `65 6e 2d 75 73 00` at `0x2C` is `annotations_language[0]`. The vendored fixture was then read with `xxd` as an independent confirmation, and it agrees at every offset.

- **The citation walker found a real gap on its very first run** — `read_bit` and `write_bit`, the two most important functions in the module, had no `// ref:` line. That failure *was* this task's RED. Fixing it required deciding what to cite when the reference has no same-named symbol: the reference inlines the per-bit step inside its literal reader/writer, so the citation points at the function that **contains** the logic and a `// NOTE:` rider says so. Inventing `WriteBitBuffer::WriteBit` would have been exactly the rot D-23 exists to prevent.

- **D-03's recorded rationale is corrected where a future reader will hit it.** D-03 says the fixed-size uleb128 encoder exists so the harness can reproduce reference files for CONF-08. Research correction 4 shows that is false — 524 674 `obu_size` fields across all 226 reference `.iamf` files, zero non-minimal — and `src/bits/mod.rs` now says so in the module doc comment, along with what the encoder is *actually* for: Phase 2's PARSE-04 foreign-file caveat and reproducing `test_000134`, the single textproto in the corpus that sets `GENERATE_LEB_FIXED_SIZE`. An in-crate vector proves the decoder already accepts a non-minimal encoding.

- **Nothing under `src/bits/` can panic on hostile input, and that is measured rather than asserted.** Zero `unwrap()`/`expect()`/raw index in production code, zero `#[allow]` anywhere in `src/`, and `clippy --all-targets -- -D warnings` clean with `indexing_slicing`, `arithmetic_side_effects`, `unwrap_used`, `expect_used` and `panic` all at deny. The V5 bounds-check-before-allocate rule is stated as a module-level comment on `reader.rs` where it is first enforced, because it governs every count-driven loop in every later plan.

## Task Commits

| Task | Phase | Commit | What |
|---|---|---|---|
| 1 (tracer) | RED | `e9dd7f7` | 6 hand-computed vectors + `src/bits/` type shape and bounds-check paths. Gate: exit 101, 6 tests, 0 pass, 6 fail, `RED_EVIDENCE_OK`. |
| 1 (tracer) | GREEN | `5408fe4` | `read_bit`/`write_bit`, MSB-first accumulation, minimal uleb128 both ways. 6/6. |
| 1 (tracer) | gate | — | Tracer feedback gate re-ran all three `<verify>` commands end-to-end under auto mode; all passed, so expansion proceeded. |
| 2 | RED | `4dfdffe` | 31 vectors + the remaining signatures. Gate: exit 101, 31 tests, 14 pass, 17 fail, `RED_EVIDENCE_OK`. |
| 2 | GREEN | `debd741` | Signed widths, strings, spans, sub-readers, the leb128 caps and the fixed-size encoder. 31/31 + 5 in-crate. |
| 3 | RED | `20dbf54` | The oracle and the citation walker + `proptest`/`bitstream-io`. Gate: exit 101, 3 tests, 2 pass, 1 fail, `RED_EVIDENCE_OK` — the walker's own finding. |
| 3 | GREEN | `eb40f1c` | The two missing citations, the walker's block-scanning fix, and the containment proof in `mod.rs`. |

**Measured:** `git rev-list --count 4ca3ed2..HEAD` = **6** commits (`plan_head_before: 4ca3ed20e2becd9bd2c4cc5f222f12e2d1f9d33c`). No REFACTOR commits: the one cleanup that arose (deduplicating the two encoder loops) was folded into task 2's GREEN so that every commit on the branch is green under `cargo clippy -D warnings`.

## Files Created/Modified

| File | What it does |
|---|---|
| `src/bits/mod.rs` | BITS-07's containment rule in the imperative, D-01's rationale, D-03's **corrected** rationale, the citation convention including the `// NOTE:` rider, and the containment proof — the public `Error` wraps no standard-library I/O error type, so the boundary does not exist at all. |
| `src/bits/reader.rs` | `BitCursor<'a>`. Eleven public methods plus the private `read_bit`/`advance_one_bit`. Carries the module-level "one rule, every count" bounds-check policy. |
| `src/bits/writer.rs` | `BitWriter`. Nine public methods mirroring the reader in the same order, plus the private `write_bit`. `finish()` refuses a pending partial byte rather than zero-padding it. |
| `src/bits/leb128.rs` | `minimal_len`, `write_uleb128_fixed` (`pub(crate)`), `write_uleb128_minimal` expressed in terms of it, and `read_uleb128` with both caps. Five in-crate vectors for the `pub(crate)` surface `tests/` cannot reach. |
| `src/lib.rs` | `pub mod bits;`. |
| `src/error.rs` | Four new scalar-payload variants: `UnsupportedWidth`, `ValueExceedsWidth`, `StringHasInteriorNul`, `Leb128SizeInvalid`. The 32-byte `const` budget still holds. |
| `tests/vectors.rs` | 31 named hand-computed vectors, each keyed to the `test_000003.iamf` offset it reproduces where one exists. |
| `tests/bits_oracle.rs` | Four proptest properties plus the exhaustive seven-bit group boundaries. The only file in the repository that names `bitstream_io`. |
| `tests/citations.rs` | The D-23 walker, plus two tests of the walker itself. |
| `Cargo.toml` / `Cargo.lock` | Three dev-dependencies, each with its licence on the line that adds it, and a comment recording that `bitstream-io` is dev-only by decision. |
| `.planning/.../deferred-items.md` | One out-of-scope discovery (pre-existing `cargo fmt` drift in two earlier plans' test files). |

## Decisions Made

**`minimal_len` needed a production caller, and giving it one improved the code.** A `pub(crate)` helper used only from `#[cfg(test)]` is dead code in a normal build, and `#[allow(dead_code)]` is forbidden under `src/bits/`. Rather than widening the API, `write_uleb128_minimal` now calls `write_uleb128_fixed(w, value, minimal_len(value))`. That is what `LebGenerator` itself does — `kMinimum` and `kFixedSize` differ only in where the length comes from — it deletes a duplicated loop, and it makes minimality structural: the loop never decides when to stop, so it cannot append a trailing `0x80 0x00`.

**`read_string` returns the NUL; `write_string` appends it.** Asymmetric on purpose. The reader shows exactly what was on the wire and the field's byte count includes the terminator; the writer takes the payload without one so a caller cannot emit an unterminated string at all. Round-tripping a read value means dropping its last byte, and both sides say so in their doc comments. `write_string` also refuses an interior NUL (`StringHasInteriorNul`), because such a string reads back shorter than it was written.

**`read_uint8_span` and therefore `sub_reader` require byte alignment.** A borrowed `&[u8]` has no sub-byte start, so this is forced rather than chosen — and it is also correct: OBU payloads are byte-aligned by construction and the reference pads nothing. `sub_reader` is expressed in terms of `read_uint8_span` so the alignment check, the bounds-check-before-reserve rule and the parent advance have one implementation rather than two that can drift.

**The values are byte strings, never UTF-8-validated.** Validating on read would make this parser stricter than the reference, and rejecting a file the reference accepts is as much a conformance failure as accepting one it rejects.

**Errors report the offset of the field's first byte, not the byte the violation was noticed at.** Both uleb128 caps carry `Location::InputOffset(start)`: the thing that is wrong is the whole field. Read errors carry `InputOffset` and write errors `OutputOffset` throughout, including from `leb128.rs`, which is why `BitWriter::output_offset` is `pub(super)` — a write error's offset must not depend on which module noticed it.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] The plan's `cargo tree -e normal` gate is the wrong instrument, for the second time in this phase**

- **Found during:** Task 3 verification.
- **Issue:** Task 3's `<verify>` block asserts that `cargo tree -e normal --prefix none | sort -u | grep -cE '^(iamf|thiserror)'` equals the total line count. Run as written it fails on a correct crate: `-e normal` selects normal dependency *edges* and includes the whole `thiserror-impl → syn → proc-macro2 → quote → unicode-ident` proc-macro chain. Measured here: 9 lines total, 3 matching. This is the identical finding plan 01-01 recorded as its deviation 2.
- **Fix:** Used `cargo tree -e normal,no-proc-macro --prefix none | sort -u`, which returns exactly two lines, both matching — `iamf v0.1.0` and `thiserror v2.0.20`. This is what `.github/workflows/ci.yml` already asserts and what `REFERENCES.md` already documents; no file needed changing.
- **Files modified:** none.
- **Verification:** `total=2 matched=2` with the correct instrument; `total=9 matched=3` with the plan's.

**2. [Rule 1 — Bug] Task 1's panic-path grep cannot distinguish production code from the `#[cfg(test)]` module**

- **Found during:** final verification, after task 2 added in-crate vectors to `leb128.rs`.
- **Issue:** The gate is `grep -v '^[[:space:]]*//' src/bits/*.rs | grep -cE 'unwrap\(\)|expect\(|\.\.\]'`, and it must print 0. It prints 5 — all five inside `leb128.rs`'s `#[cfg(test)] mod tests`, which is precisely where GUARD-04's carve-out permits `expect()` (`allow-expect-in-tests = true`, and clippy agrees). A line-based grep cannot see module boundaries.
- **Fix:** Truncated each file at its `#[cfg(test)]` line before grepping, which is the same scope clippy and D-21's `#[allow]` census use. The corrected instrument prints **0**.
- **Files modified:** none.
- **Verification:** `for f in src/bits/*.rs; do sed '/^#\[cfg(test)\]$/,$d' "$f"; done | grep -v '^\s*//' | grep -cE 'unwrap\(\)|expect\(|\.\.\]'` → `0`.

**3. [Rule 3 — Blocking] Let-chains are Rust 1.88; GUARD-12 pins 1.85**

- **Found during:** Task 1 GREEN.
- **Issue:** `if let Some(limit) = 1_u64.checked_shl(bits) && value >= limit` is the natural way to write the width check, and it does not compile on the pinned toolchain — let-chains stabilised in 1.88.
- **Fix:** A nested `if`, with a comment naming the reason so nobody "tidies" it back. `clippy::collapsible_if` does not fire on `if let` + `if` under 1.85, so the lint set is satisfied.
- **Committed in:** `5408fe4`.

**4. [Rule 3 — Blocking] `minimal_len` and later `write_uleb128_fixed` were dead code, and `#[allow(dead_code)]` is forbidden here**

- **Found during:** Task 1 and Task 2 GREEN.
- **Issue:** Both are `pub(crate)` and, as planned, had no non-test caller in this plan — their real users are Phase 2's PARSE-04 and the `test_000134` reproduction. `-D warnings` turns `dead_code` into a build error, and the acceptance criteria forbid an `#[allow]` under `src/bits/`.
- **Fix:** Gave them real callers instead of silencing the lint, by expressing the minimal encoder in terms of the fixed-size one (see Decisions). Better code, not a workaround.
- **Committed in:** `5408fe4` and `debd741`.

**5. [Rule 3 — Blocking] The citation walker's original single-line rule could not see a wrapped `// NOTE:` rider**

- **Found during:** Task 3 GREEN.
- **Issue:** The walker checked the first preceding non-blank, non-attribute, non-doc line. Pitfall F's convention puts a `// NOTE:` after the `// ref:` line, and a note long enough to be worth writing wraps — so the line immediately above the function is a plain `//` continuation and the citation was not found.
- **Fix:** The walker now searches the whole plain-comment block above the function and stops at the first line of code, so a citation on the *previous* function is still never credited to this one. Both the file's doc comment and the walker's self-test were updated to match.
- **Committed in:** `eb40f1c`.

**6. [Rule 3 — Blocking] `cargo fmt` reformatted two files this plan does not own**

- **Found during:** Task 2 and Task 3.
- **Issue:** `cargo fmt` has no path filter that respects task scope; running it touched `tests/error_shape.rs` and `tests/reference_manifest.rs`, authored by plans 01-01 and 01-02.
- **Fix:** Reverted both with `git checkout --` before staging, twice. The drift is logged in `deferred-items.md` rather than fixed. CI does not currently run `cargo fmt --check`, so this is not a red gate today.
- **Files modified:** none (reverted).

---

**Total deviations:** 6 auto-fixed (2 Rule 1 corrections to verification instruments, 4 Rule 3 blocking). **No scope creep.** Two files not listed in the plan were produced: `deferred-items.md` (required by the executor's scope-boundary rule) and the in-crate `#[cfg(test)]` module in `leb128.rs` (forced — `tests/` cannot reach a `pub(crate)` item, and the plan's own behaviour list requires vectors for `minimal_len` and `write_uleb128_fixed`).

## Issues Encountered

**Three `ErrorKind` variants beyond the six the plan named.** `UnsupportedWidth { bits: u8 }`, `ValueExceedsWidth { bits: u8 }` and `Leb128SizeInvalid { size: u8 }`, plus `StringHasInteriorNul`. All are scalar-payload so the 32-byte `const` budget is untouched, and all name genuinely distinct failures the plan's own `<behavior>` blocks require (a width outside 0..=64, a value that does not fit its field, a fixed-size request outside 1..=8, a payload that would read back shorter than it was written). `ErrorKind` is `#[non_exhaustive]`, so these are additive.

**The oracle passed on its first run.** That is the desired outcome and not a sign the test is weak — the RED for those properties was earned by the two GREEN commits before it, and the properties do fail if the implementation is wrong (the citation walker in the same commit demonstrates a genuine first-run failure). Worth knowing for a reviewer: the oracle's value is as a *standing* gate over every later change to `src/bits/`, not as a one-off discovery.

**BITS-01's requirement text is still superseded.** `REQUIREMENTS.md` says `BitReader<'a>`/`BitWriter` "wrapping `bitstream-io`". D-01 amended that and this plan implemented the amendment — the type is `BitCursor`, the crate is dev-only. The requirement text was **not** edited, as the plan's flagged assumption says. Recorded in the broken-windows ledger as entry 5 so it is visible at ship time.

## Known Stubs

None. `src/`, `tests/*.rs` and `tools/*.sh` scanned for `TODO`, `FIXME`, `unimplemented`, `placeholder`, "coming soon" and "not available" — no matches. `grep -rn 'allow(' src/` returns nothing, so D-21's escape census is still zero entries.

The RED commits `e9dd7f7`, `4dfdffe` and `20dbf54` each passed through a deliberately incomplete state; none of those states survives into `HEAD`.

## Threat Flags

None. The plan's `<threat_model>` covers exactly the surface this plan built, and every `mitigate` disposition landed:

| Threat | Where it landed |
|---|---|
| T-01-12 unbounded allocation from a parsed length | `read_uint8_span` bounds-checks before slicing; the policy is a module-level comment on `reader.rs`; `read_string` grows one byte at a time behind a 128-byte cap |
| T-01-13 uleb128 continuation chain | 1..=8 bytes; a ninth is `Leb128TooLong`; over-`u32` is `Leb128ValueTooLarge` before the narrowing |
| T-01-14 integer overflow in width/position arithmetic | `checked_*`/`saturating_*` throughout; `arithmetic_side_effects` at deny; zero `#[allow]` |
| T-01-15 slice-index panic | `indexing_slicing` at deny; every read goes through the bounds-checked cursor; the grep gate prints 0 for production code |
| T-01-16 unterminated string | 128-byte cap **including** the NUL, checked before the next read so exhaustion cannot masquerade as it |
| T-01-17 silently truncating `sub_reader` | errors on both over-length and unaligned parents, and leaves the parent inert on failure |
| T-01-SC crates.io installs | all three dev-dependencies carry OK/Approved verdicts in RESEARCH.md's Package Legitimacy Audit; `cargo deny check licenses` ok; `cargo tree -e normal,no-proc-macro` proves none reach the linked graph |

## User Setup Required

None. Everything in this plan runs offline with `cargo test`.

The two items plan 01-01 raised are still open and still the user's call: the work remains on `gsd/phase-01-conformant-lpcm-bitstream` rather than `main`, and `.gsd/` plus `.planning/milestone.lock` are still untracked.

## Next Phase Readiness

Ready for plan **01-04** (the OBU header):

- `BitWriter::write_unsigned(31, 5)` plus three `write_bool` is literally byte 0 of an IA Sequence Header, already proven.
- `write_uleb128_minimal` is the `obu_size` emitter Pattern 1's two-pass serialisation needs, and `minimal_len` sizes the header for the first pass.
- `sub_reader` is Pattern 3's bounded payload view, with the "assert fully consumed" check available as `bits_remaining() == 0`.
- `read_signed(16)` is `audio_roll_distance`, `default_mix_gain`, `integrated_loudness` and `digital_peak`, all verified against real fixture bytes.
- `tests/citations.rs` will fail any new `fn read_*`/`fn write_*` that ships without a `// ref:` line, so the discipline is now enforced rather than remembered.

**Carried concern:** the oracle covers the *primitives*. It says nothing about OBU-level framing, and it never will — `bitstream-io` has no opinion about `obu_size`. From 01-04 onward the only oracles are the hand-computed vectors and the reference binaries, which is exactly why D-25's two-independent-derivations rule matters more from here, not less.

---
*Phase: 01-conformant-lpcm-bitstream*
*Completed: 2026-09-08*

## Self-Check: PASSED

All 12 claimed files exist on disk. All 6 claimed commits (`e9dd7f7`, `5408fe4`, `4dfdffe`, `debd741`, `20dbf54`, `eb40f1c`) exist in git. `actuals.commits: 6` is measured with `git rev-list --count 4ca3ed2..HEAD`, not narrated. Frontmatter is `status: complete` with 7 requirement IDs and 10 coverage entries.
