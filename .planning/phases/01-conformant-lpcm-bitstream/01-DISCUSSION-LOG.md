# Phase 1: Conformant LPCM Bitstream - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-09-08
**Phase:** 1-Conformant LPCM Bitstream
**Areas discussed:** Bit-layer dependency, Reference-oracle loop, Reader in Phase 1, Conformance
fixture design, Guardrails & sequencing, Test discipline & gate policy

---

## Bit-layer dependency

### Q1 — BITS-01 says wrap `bitstream-io`; API-05 says `--no-default-features` must be dependency-free

| Option | Description | Selected |
|--------|-------------|----------|
| Hand-roll + keep bitstream-io as test oracle | Hand-rolled `BitCursor`/`BitWriter` over `&[u8]`; `bitstream-io` demoted to dev-dependency used in a differential proptest | ✓ |
| Hand-roll, drop bitstream-io entirely | Same layer, no differential oracle; correctness rests on BITS-06 vectors plus the reference gate | |
| bitstream-io only, relax API-05 | Keep BITS-01 verbatim, reinterpret "dependency-free" as "no codec/driver deps" | |
| Feature-gate both implementations | `bitstream-io` by default, hand-rolled under no-default-features | |

**User's choice:** Hand-roll + keep as test oracle
**Notes:** Gives the zero-dependency core API-05 wants *and* the 50.6M-download exercise as an
independent oracle, without either in the shipping graph. Amends BITS-01's wording, satisfies its
intent. The feature-gate option was rejected as duplicating 22 primitives across a cfg matrix —
the read/write asymmetry class of bug this project exists to catch, doubled.

### Q2 — What does API-05's "dependency-free" mean, given `thiserror` pulls a 6-crate proc-macro chain?

| Option | Description | Selected |
|--------|-------------|----------|
| Zero linked deps; derive crates excepted | `thiserror` unconditional; definition written into REFERENCES.md and a lib.rs doc comment | ✓ |
| Literal zero crates under no-default-features | `thiserror` behind default feature; hand-written Display under no-default-features | |
| Defer — decide it in Phase 4 | Leave API-05's wording unresolved until plan 04-03 | |

**User's choice:** Zero linked deps; derive crates excepted
**Notes:** `thiserror` is locked by PROJECT.md's constraints, so the question was what
"dependency-free" means, not whether to keep it. None of the six crates contribute code to the
linked artifact. Deferring was rejected because the decision would then land with four milestones
of error variants already accumulated.

### Q3 — If reproducing golden `test_000003.iamf` requires non-minimal (fixed-size) uleb128?

| Option | Description | Selected |
|--------|-------------|----------|
| Crate-internal fixed-size path, no public knob | Public writer stays minimal; `pub(crate)` fixed-size path for the harness only | ✓ |
| Minimal only — enumerate the diff instead | CONF-08 becomes "reproduced except at enumerated offsets" | |
| Decide after the DEC-04 research | Have plan 01-02 report `LebGenerator`'s default mode first | |

**User's choice:** Crate-internal fixed-size path
**Notes:** The Out-of-Scope exclusion on a `LebMode` knob is about the *public encoder* — byte
identity must stay caller-independent. Nothing stops the test harness expressing fixed-size form.

### Q4 — Given decode is a superset of encode, how should `audio_element_type` be modelled?

| Option | Description | Selected |
|--------|-------------|----------|
| Non-exhaustive enum, all wire values representable, one constructible | `ChannelBased` / `SceneBased` / `Reserved{value,raw}`; only `ChannelBased` via the public encoder | ✓ |
| Channel-based only, with a raw catch-all | `ChannelBased` plus one `Unparsed{element_type, raw}` | |
| Channel-based only, error on anything else | Parser rejects unmodellable element types | |
| Defer to Phase 2 | Model channel-based concretely; leave enum shape to the parser phase | |

**User's choice:** Non-exhaustive enum, all wire values representable, one constructible
**Notes (user, verbatim in substance):**
- `Reserved { value, raw }` buys a **testable property**, not just forward compatibility:
  decode→encode of an element you do not understand is byte-identical. Checkable in CI against
  fixtures you cannot otherwise assert anything about, and strictly stronger than "we parsed it".
  Options 2 and 3 cannot state that property; option 3 turns legal files into errors, poisoning the
  fuzz corpus with false positives.
- It is also the shape the Parallax import adapter needs. House rule: **refuse with the reason
  named, never approximate.** Option 1 lets the adapter say "this file carries element type 3,
  which we do not model" — precise, actionable, not a whole-file rejection. Option 2 would let an
  unmodelled element pass silently as opaque bytes, the failure mode catalogued in Ardour and
  maolan.
- **Invariant to make explicit in the type's docs:** `Reserved` can only round-trip if the parser
  can skip an unknown element's payload without understanding it, which requires the remaining
  length to be derivable. IAMF OBUs carry `obu_size`, so it is. This silently breaks if a future
  field is added before the type-specific config.
- **API detail:** "parse-only" must mean *not constructible through the public encoder path*, not
  *not constructible*. PARSE-07 needs to build scene-based fixtures, so keep it constructible
  internally or behind a test-only constructor — otherwise "asserted understood" cannot be met.

**Claude's follow-on (recorded as a wiring constraint, not a new decision):** `Reserved.raw` and
OBU-07's `trailing: Vec<u8>` both claim "everything after the fields I understood" and need a
defined precedence.

---

## Reference-oracle loop

### Q1 — Where does the reference gate run: macOS, container, or Linux CI only?

| Option | Description | Selected |
|--------|-------------|----------|
| Split by cost: libiamf local, iamf-tools containerised | `libiamf` native on macOS for the fast CONF-05 loop; `iamf-tools` only in a pinned container | ✓ |
| Both containerised, one image | Single pinned Dockerfile for both, run locally and in CI | |
| Both native on macOS, CI mirrors on Linux | Fastest local loop; owns the macOS Bazel + fdk-aac build | |
| CI only — nothing local | Golden fixtures offline; reference verdict on push | |

**User's choice:** Split by cost
**Notes:** The two references have wildly asymmetric build costs, and that asymmetry maps onto how
often each is needed — CONF-05 runs on every edit, CONF-06/07 when a file already decodes, CONF-11
exactly once. CONF-10 keeps `cargo test` green offline on all four targets regardless.

### Q2 — How is CONF-07's byte-diff writeup enforced so it cannot rot?

| Option | Description | Selected |
|--------|-------------|----------|
| Executable ledger — the writeup IS the test | Committed diff table asserted exactly; new *and* vanished diffs fail | ✓ |
| Zero-diff gate — any difference is a bug | Binary exit criterion, no ledger | |
| Executable ledger keyed by field not offset | Survives offset shifts; coarser | |
| Prose writeup reviewed at phase exit | `docs/BYTE-DIFF.md`, human-reviewed | |

**User's choice:** Executable ledger
**Notes:** "Enumerated in writing" describes an artifact, not an enforcement mechanism. A prose
table rots silently because nothing recomputes it, and the gate would pass on a document rather
than on the bytes.

### Q3 — CONF-11 fixture generation: what gets committed?

| Option | Description | Selected |
|--------|-------------|----------|
| Commit under a size cap, MANIFEST the rest | One Bazel run; commit under a stated cap plus a full MANIFEST with regeneration recipe | ✓ |
| Commit everything that builds | Maximum breadth for PARSE-07 and the FUZZ-04 corpus seed | |
| Phase-scoped — only what Phase 1 needs | Re-run the pinned container in Phases 2 and 3 | |
| Commit all via Git LFS | Full coverage without bloating normal clones | |

**User's choice:** Commit under a size cap, MANIFEST the rest
**Notes:** Fixtures are a permanent repo asset — Parallax consumes this crate as a path dependency,
so they land in every Parallax developer's clone. The MANIFEST makes "why isn't fixture X here"
answerable without re-deriving it, and distinguishes "negative test" from "build failed".

### Q4 — FFmpeg 9.0.1 with IAMF demux/mux is installed. Add it to the gate?

| Option | Description | Selected |
|--------|-------------|----------|
| Third read-oracle now, opt-in; source declared off-limits | Optional CONF-05-shaped clause; CONTRIBUTING.md extended | |
| Read-oracle AND second byte-diff target | Also mux with ffmpeg and diff against it | |
| Record the finding, wire it in Phase 2 | Licence boundary written in Phase 1; oracle in Phase 2 | ✓ |
| Don't use it | Keep the gate to the two AOM references | |

**User's choice:** Record the finding, wire it in Phase 2
**Notes:** Raised by the user mid-discussion. Verified locally: ffmpeg 9.0.1 at
`/usr/local/bin/ffmpeg`, IAMF demuxer **and** muxer present, built `--enable-gpl --enable-version3`.
The load-bearing distinction: **reading** `libavformat/iamf*.c` is contamination (LGPL-2.1+, same
class as `gpac`, which PROJECT.md forbids); **invoking** the binary from a test is not — separate
process, nothing links, the pattern already blessed for `libiamf`. GUARD-08's CONTRIBUTING.md
checkbox is extended to name FFmpeg/libavformat in Phase 1 regardless, because the trap is a
developer debugging a byte-diff opening `iamf_writer.c` "just to check".

### Q5 — How is the pinned-SHA guarantee (GUARD-06) enforced across a split build?

| Option | Description | Selected |
|--------|-------------|----------|
| Stamped manifest + digest-pinned image, asserted by a test | `.reference-manifest.json` + digest pin, asserted before any clause runs | ✓ |
| Stamped manifest only | Verify the local build; trust the container by tag | |
| Digest-pinned image only | Both references in one immutable image | |
| REFERENCES.md + CI checkout, no runtime assertion | GUARD-06 as literally written | |

**User's choice:** Stamped manifest + digest pin + test
**Notes:** GUARD-06 as written is a statement about the build script; nothing verified the binary
actually tested against was built from the pinned SHA. A rebuilt image on the same tag is exactly
how `iamf-tools` HEAD's draft-v2.0 type model could reach the strict oracle unnoticed.

### Q6 — GUARD-09's four-target matrix: what runs where, how often?

| Option | Description | Selected |
|--------|-------------|----------|
| Golden-hash on all four every PR; full suite on two | Cheap claim continuously proven; full suite on Linux + macOS arm64 | |
| Everything on all four, every PR | Maximum confidence, simplest matrix | ✓ |
| Linux + macOS arm64 per PR, all four on main | Fastest PR gate | |

**User's choice:** Everything on all four, every PR
**Notes:** Recorded operational risk — GitHub's `macos-13` is the last x86_64 macOS image and is on
the retirement path, so plan 01-01 must carry a documented fallback rather than discovering it as a
red gate.

---

## Reader in Phase 1

### Q1 — How far does the reader come forward?

| Option | Description | Selected |
|--------|-------------|----------|
| Full pairing from the first commit | Every OBU type read+write in one file, one commit | ✓ |
| Pair only where a requirement names it | DESC-01, TIME-05, OBU-07, OBU-08 only | |
| Writer-only plus the read-wrapper skeleton | No per-type readers until Phase 2 | |
| Full pairing AND pull the round-trip property forward | Also move PARSE-03/04 into Phase 1 (roadmap amendment) | |

**User's choice:** Full pairing from the first commit
**Notes:** The requirements already mandate substantial reader work — DESC-01 ("serialised *and
parsed*"), TIME-05 ("*parsed* with its `ParamDefinition`"), OBU-07's shared read wrapper, OBU-08,
BITS-01's `BitReader`. The `Reserved{value,raw}` round-trip property is unobservable without one.
**Trap named and guarded:** the roadmap's justification for Phase 2 depending on Phase 1 is that "a
parser that mirrors a misunderstanding passes its own round-trips forever" — so pairing is for
auditability, and round-trip tests are supplementary evidence only.

### Q2 — What does the reader do with fields the writer derives?

| Option | Description | Selected |
|--------|-------------|----------|
| Preserve the wire value; validate separately, never silently | Model stores what was read; `validate()` names mismatches | ✓ |
| Re-derive and error on mismatch | Parser is the enforcement point | |
| Re-derive and overwrite silently | Reader normalises to the derived value | |
| Don't model derived fields at all | Computed on write, discarded on read | |

**User's choice:** Preserve the wire value; validate separately
**Notes:** Keeps `serialize(parse(bytes)) == bytes` exact for foreign files and matches the
refuse-with-the-reason-named rule. Silent normalisation is the failure mode this project exists to
avoid — the round-trip would pass by having quietly changed the bytes.
**Consequence recorded rather than re-asked:** the writer must not validate. Faithful writer,
explicit `validate()`, `build()` as the mandatory point in Phase 4.

### Q3 — Error type shape, serving reads, writes and validation

| Option | Description | Selected |
|--------|-------------|----------|
| Struct with `kind` + typed `Location` | `Error { kind, at: InputOffset/OutputOffset/Field/Unlocated }` | ✓ |
| Flat non-exhaustive enum, `offset` per variant | GUARD-13 exactly as written | |
| Three error types unified at the top | `ReadError` / `WriteError` / `ValidationError` | |

**User's choice:** Struct with `kind` + typed `Location`
**Notes:** Input offset and output offset are different numbers meaning different things, and
validation errors have a field path rather than an offset. Repeating `offset: Option<u64>` costs
16 bytes before each variant's payload against STACK.md's 32-byte `const` assert, and gives
validation variants a permanently-`None` field.

---

## Conformance fixture design

### Q1 — What is the fixture, concretely?

| Option | Description | Selected |
|--------|-------------|----------|
| Two elements: 5.1 + stereo, 48 kHz, 24-bit BE | Six descriptor OBUs, two Codec Configs, two Audio Elements | ✓ (superseded below) |
| Single 7.1.4 element, 48 kHz, 24-bit BE | Matches Parallax's target layout | |
| Two fixtures: minimal stereo + full 5.1 | Fast loop plus full gate | |
| Minimal: stereo, 48 kHz, 16-bit | Smallest file satisfying the letter of CONF-02/03/04 | |

**User's choice:** Two elements: 5.1 + stereo — **subsequently revised, see Q3**
**Notes:** Stereo cannot detect BCG packing order (one coupled pair, zero mono channels). 5.1 packs
as L/R coupled, Ls/Rs coupled, then C and LFE mono. 24-bit big-endian catches DESC-03's inverted
`sample_format_flags` sense.

### Q2 — How is input PCM compared against the decoder's output, given three channel orderings?

| Option | Description | Selected |
|--------|-------------|----------|
| Define the fixture in the decoder's output order; no permutation constant anywhere | Direct equality; scrambling diagnosed by name on mismatch | ✓ |
| Explicit named mapping constant, cited and asserted | Permutation kept but treated as fixed test input | |
| Multiset assert first, then ordered assert | Two named failure modes | |
| Compare in whatever order works; tune once and freeze | Establish empirically against a known-good file | |

**User's choice:** Define the fixture in the decoder's output order; no permutation constant
**Notes:** TIME-02 makes input order, BCG packing order and `iamfdec`'s WAV order three different
things. A permutation constant is a knob, and the cheapest-looking fix for a failing comparison is
to turn it until green — absorbing a real BCG packing bug into the harness. No knob, no knob to
turn.

### Q3 — The two-element fixture makes the decoded PCM a mix we cannot predict without rendering

| Option | Description | Selected |
|--------|-------------|----------|
| Two Codec Configs, one 5.1 Audio Element | One sub-mix, one element, unity gain — decoder output *is* our input | ✓ |
| Two Audio Elements, only one in the sub-mix | Stronger ordering coverage, same legality question on a bigger descriptor | |
| Two elements, second muted to silence | Depends on the reference renderer's arithmetic, not on our bytes | |
| Single element; satisfy CONF-04 via OBU count alone | No repeated descriptor type | |

**User's choice:** Two Codec Configs, one 5.1 Audio Element
**Notes:** Correction to Q1's recommendation. `iamfdec` decodes a *mix presentation*, rendering
every element in the sub-mix into the target layout and summing them — so two elements make the
decoded PCM a mix, and predicting it would require rendering, which is explicitly out of scope
(Parallax owns it under `D-40`). CONF-04's stated purpose — "two Codec Configs **or** two Audio
Elements, so ordering is observable" — is met by the repeated descriptor type.
**Research item:** confirm `iamf-tools`' strict parser accepts a Codec Config no element
references; fall back to differing the two configs if not.

---

## Guardrails, hygiene and sequencing

### Q1 — GUARD-11's no-DSP guard mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| clippy `disallowed-types` on f32/f64, one documented allow | Extends the clippy.toml already carrying GUARD-02 | ✓ |
| Grep script in CI | `tools/no-dsp-check.sh` scanning `src/` | |
| Both — clippy plus a grep sweep | Belt and braces | |

**User's choice:** clippy `disallowed-types`, one documented allow
**Notes:** Compiler-enforced so local clippy and CI agree. PROF-03's Q7.8 helper carries the single
`#[allow]`, turning the guard into a census where the correct answer is exactly one entry.

### Q2 — Plans 01-01 and 01-02: same wave or sequential?

| Option | Description | Selected |
|--------|-------------|----------|
| Sequential 01-01 → 01-02, both before any bitstream code | Guardrails first, then external tooling | ✓ |
| Same wave, 01-01 owns REFERENCES.md | Literal reading of "in parallel on day one" | |
| Split 01-01: pins first, then parallel | Nine plans; roadmap amendment | |

**User's choice:** Sequential 01-01 → 01-02
**Notes:** The roadmap's intent — confront the Bazel long-pole immediately rather than in month two
— is satisfied by sequential-but-second. Both plans create the most root-level files
(`Cargo.toml`, `.github/workflows/`, `REFERENCES.md`), and a conflict there would corrupt the
guardrails before any code exists to protect. The slow part of 01-02 is container and Bazel
wall-clock, which parallel agents do not speed up.

### Q3 — Worktree: staged `LICENSE` deletion and untracked `.vscode/`

| Option | Description | Selected |
|--------|-------------|----------|
| Clean up now, before planning | Separate housekeeping commit; decide `.vscode/` deliberately | ✓ |
| Fold into plan 01-01 | Land alongside the DEC-02 licence work | |
| Clean up now, and gitignore `.vscode/` | Same, with editor config kept personal | |

**User's choice:** Clean up now, before planning
**Notes:** Done during the discussion — commit `41eaa4e`. `.vscode/settings.json` held only
personal editor config (a `files.exclude` list and a hide-files extension setting), so it was
gitignored rather than committed. A dangling staged deletion is the kind of thing an agent's first
commit sweeps up silently, attributing a licence change to a scaffolding commit.

---

## Test discipline and gate policy

### Q1 — With TDD mode on, where do expected bytes come from?

| Option | Description | Selected |
|--------|-------------|----------|
| Hand-decoded vectors first, reference capture as a second assert | Two independent derivations that must agree | ✓ |
| Reference capture first, hand-decode only on disagreement | Faster to a green suite | |
| Hand-decoded only for header and descriptors | Concentrate expensive work where misreading is costly | |

**User's choice:** Hand-decoded vectors first, reference capture second
**Notes:** Capturing reference output as the expected bytes is a regression test, not a
specification — it proves you match a blob you never understood, and cannot catch the case where
you and the reference both do something the spec forbids. Hand-decoding is the activity that
already resolved the OBU header three independent ways.

### Q2 — What does `validate()` return?

| Option | Description | Selected |
|--------|-------------|----------|
| All findings, each naming its field path | `fn validate(&self) -> Vec<Finding>` | ✓ |
| First error only, `Result<(), Error>` | Composes with `?` | |
| Both — collecting and short-circuiting forms | Two entry points | |

**User's choice:** All findings, each naming its field path
**Notes:** One run tells you everything wrong with a foreign file, which is what the import adapter
needs to explain a rejection. Matches the refuse-with-the-reason-named rule.

### Q3 — Does Phase 1 ship any Cargo features?

| Option | Description | Selected |
|--------|-------------|----------|
| None — featureless until Phase 4 | No `[features]` table at all | ✓ |
| Define the skeleton now, empty | Declare `encode`/`decode`, both default-on | |
| Featureless, and amend PROJECT.md's line | Also correct the Key Decisions entry | |

**User's choice:** None — featureless until Phase 4
**Notes:** Nothing exists to gate. A cfg matrix would also multiply the four-target byte-identity
gate now mandatory on every PR. PROJECT.md's "One crate, feature-gated `encode`/`decode`" line is
flagged in CONTEXT.md's deferred section as a trap an executor would otherwise implement faithfully.

### Q4 — How is the `// ref:` citation discipline enforced?

| Option | Description | Selected |
|--------|-------------|----------|
| Mechanical test over the source | Walks `src/`, fails on any uncited `read_*`/`write_*` | ✓ |
| Review convention in CONTRIBUTING.md | Enforced by review | |
| Test for presence, plus a periodic manual audit | Both | |

**User's choice:** Mechanical test over the source
**Notes:** Citations rot silently in exactly the way the byte-diff writeup would have — same fix as
the executable ledger.

### Q5 — What gets committed as the byte-identity golden?

| Option | Description | Selected |
|--------|-------------|----------|
| Fixture + annotated structural dump + hash | Dump makes an output change a reviewable diff | ✓ |
| Fixture + hash only | Smallest | |
| Hash only, fixture regenerated in-test | Nothing binary in the repo | |

**User's choice:** Fixture + annotated structural dump + hash
**Notes:** GUARD-09's stated rationale is that byte-identity as a committed fixture "makes an output
change a reviewable PR diff rather than a red job with no explanation" — which neither a bare hash
nor a bare binary blob delivers. Caveat recorded: the dump is self-consistent with the writer, so
it aids review, never correctness.

### Q6 — If a gate clause proves unachievable for a reason that is not a defect?

| Option | Description | Selected |
|--------|-------------|----------|
| Written waiver, recorded in the repo, phase exits | `CONFORMANCE-GATE.md` entry; phase exits with the waiver visible | ✓ |
| No waivers — the phase does not exit | All seven clauses, no exceptions | |
| Waiver, and the clause becomes a tracked v2 requirement | Plus a numbered REQUIREMENTS.md entry | |
| Decide when it happens | Judge with full information | |

**User's choice:** Written waiver, recorded in the repo
**Notes:** Decided while nothing was at stake. Two clauses are partly outside our control — CONF-08
requires reproducing `test_000003.iamf` from a configuration that is not published and must be
inferred, and CONF-06/07 depend on an `iamf-tools` v1.x tag DEC-04 has not identified. The
alternative is a gate quietly softened at 2am by whoever is closest to it.

---

## Claude's Discretion

Explicitly delegated by the user:

- Module and file layout under `src/`
- CI provider specifics and workflow file structure
- The `iamf-tools` container base image, and whether it is built in-repo or pulled
- The annotated structural dump's output format
- The `Finding` type's exact fields
- The golden-hash file's format and location
- Plan-level task breakdown within each of the 8 plans
- DEC-04 and DEC-05 routed to the researcher rather than back to the user

## Deferred Ideas

Full detail in CONTEXT.md's `<deferred>` section. Summary:

1. **PROJECT.md doc pass** — close Open Question 2 as offline-only end to end; correct the
   "monitoring/playback" motivation to the 5.1/5.2/5.3 split; soften DEC-03's recorded cost to
   mix-gain automation only; amend the feature-gating Key Decisions line.
2. **REQUIREMENTS.md** — DECO-02 is now dead text.
3. **Research pass** — verify whether `MixGainParameterData` carries per-subblock animation types
   (needs-confirming, not verified); confirm an unreferenced Codec Config is accepted by
   `iamf-tools`' strict parser.
4. **Phase 2** — wire ffmpeg in as a third, independent-lineage read oracle.
5. **v2 decoder** — `IO.md` §1 specifies importing IAMF object elements, but v1.1.0 has no
   object-based `audio_element_type` (draft-v2.0 only; `libiamf` cannot decode them). A
   v1.1.0-pinned decoder cannot satisfy that mapping. Needs-confirming against the pinned tree.

**Scope creep:** none. The discussion stayed within the phase boundary; the API-shaping constraints
the user supplied are Phase 4 material and were recorded rather than acted on.
