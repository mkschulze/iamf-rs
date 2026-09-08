# Phase 1: Conformant LPCM Bitstream - Context

**Gathered:** 2026-09-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 delivers the bit-level I/O layer, the OBU and descriptor type model, the LPCM encode
path, all thirteen guardrails, and the seven-clause conformance gate — such that a standalone
`.iamf` file this crate writes is accepted by **both** reference implementations, is
byte-explicable against `iamf-tools` output, and is reproducible byte-for-byte on macOS arm64,
macOS x86_64, Windows MSVC and Linux x64.

63 of the project's 88 v1 requirements land here. That is deliberate: the guardrails are cheaper
to install before there is code than to retrofit after, and every later phase inherits Phase 1's
byte-level understanding silently.

**Not in this phase:** the sequence-level parser, `ParamDefinitionRegistry`, round-trip
properties and the fuzzer (Phase 2); FLAC and Opus framing (Phase 3); `EncoderBuilder` and the
Parallax-facing API surface (Phase 4).

**Repo state at discussion time:** greenfield. No source files exist. Only `HANDOFF.md`,
`README.md`, `LICENSE-MIT`, `LICENSE-APACHE`, `.gitignore` and `.planning/`.

</domain>

<decisions>
## Implementation Decisions

### Bit layer and the dependency floor

- **D-01:** Hand-roll `BitCursor`/`BitWriter` over `&[u8]` (~150–300 lines) rather than wrapping
  `bitstream-io`. `bitstream-io` is demoted to a **dev-dependency**, used in a proptest that
  asserts our primitives agree with it bit-for-bit on random inputs — a differential oracle that
  never enters the shipping graph. This amends BITS-01's wording ("wrapping `bitstream-io`") while
  satisfying its intent, and resolves BITS-01's collision with API-05.
  — **Reversibility:** costly — the hand-rolled cursor produces byte offsets natively, and every
  `Error.at` value in the crate flows from it. Adopting `bitstream-io` later means reintroducing
  the `std::io::Error` mapping layer that BITS-07 exists to contain, at every primitive.

- **D-02:** API-05's "dependency-free" means **zero linked dependencies — proc-macro derive crates
  excepted**. `thiserror` stays unconditional (PROJECT.md constraint) despite pulling
  `thiserror-impl → syn → proc-macro2 → quote → unicode-ident`; none of those contribute code to
  the linked artifact. This definition MUST be written into `REFERENCES.md` and a `lib.rs` doc
  comment so Phase 4 does not rediscover it as a contradiction.

- **D-03:** The public uleb128 writer always emits **minimal form** (BITS-03 unchanged). A
  `pub(crate)` fixed-size encoder exists solely so the conformance harness can reproduce reference
  files exactly (CONF-08). No caller-visible `LebMode` — the Out-of-Scope exclusion is about the
  *public encoder*, and byte-identity must stay caller-independent.

### Type model and error shape

- **D-04:** `audio_element_type` is modelled as
  `#[non_exhaustive] enum AudioElementType { ChannelBased(..), SceneBased(..), Reserved { value: u8, raw: Vec<u8> } }`
  from the first commit. Only `ChannelBased` is constructible through the **public encoder path**;
  `SceneBased` stays constructible internally or behind a test-only constructor, because PARSE-07
  needs to build scene-based fixtures and "asserted understood" is unmeetable otherwise. Decode is
  a superset of encode — the encoder emits beds only, but Parallax's import adapter meets foreign
  files.
  Rationale the user weighted most: `Reserved { value, raw }` buys a **testable property**, not just
  forward compatibility — decode→encode of an element we do not understand is byte-identical, which
  is CI-checkable against fixtures nothing else can assert on, and strictly stronger than "we
  parsed it". It also matches Parallax's house rule — refuse with the reason named, never
  approximate: the adapter can say "this file carries element type 3, which we do not model"
  rather than rejecting the whole file or letting it pass silently as opaque bytes.
  — **Reversibility:** one-way — this is a published public enum that Parallax's import adapter
  matches on. Adding a variant is additive under `#[non_exhaustive]`, but changing `Reserved`'s
  shape, or making `SceneBased` publicly constructible later, is a breaking contract change.

- **D-05:** **Invariant to assert in the type's own docs:** `Reserved` can only round-trip if the
  parser can skip an unknown element's payload without understanding it, which requires the
  remaining length to be derivable. IAMF OBUs carry `obu_size`, so it is
  (`payload = obu_size − bytes consumed by the common fields`). This is the thing that silently
  breaks if a future field is added before the type-specific config.
  **Wiring constraint for plan 01-05:** `Reserved.raw` and OBU-07's `trailing: Vec<u8>` are two
  mechanisms both claiming "everything after the fields I understood". They need a defined
  precedence — `raw` consumes to the end of the Audio Element payload and `trailing` stays empty —
  or the boundary is ambiguous and the round-trip property becomes untestable.

- **D-06:** For fields the writer **derives** (`audio_roll_distance`, `obu_size`, implicit
  substream IDs), the reader **preserves the wire value**. The model stores what was read. A
  separate `validate()` reports every derived-field mismatch by name — "audio_roll_distance is 2,
  expected 0 for LPCM" — without failing the parse. Fresh construction through the encoder derives
  the value as DESC-02 requires. Never silently normalise.
  — **Reversibility:** costly — every descriptor struct gains a field for a value it could
  otherwise compute; removing them later is a public-API break.

- **D-07:** Three roles, no overlap: the **writer is faithful** (serialises whatever the model
  holds, so `serialize(parse(bytes)) == bytes` survives for foreign files), **`validate()` is
  explicit** and never runs implicitly, and Phase 4's **`build()` is the mandatory validation
  point** for freshly-constructed models (API-01).

- **D-08:** The error type is `struct Error { kind: ErrorKind, at: Location }` where
  `Location = InputOffset(u64) | OutputOffset(u64) | Field(&'static str) | Unlocated`. One `Result`
  type everywhere; position attached once rather than repeated into every variant; the three error
  sources (read / write / validate) stay distinguishable rather than conflated. Satisfies GUARD-13's
  intent structurally and keeps `size_of::<Error>() <= 32` tractable as variants accumulate over
  four milestones.
  — **Reversibility:** one-way — this is the public error surface Parallax matches on.

- **D-09:** `validate()` returns **all findings**, each naming its field path via
  `Location::Field` — `fn validate(&self) -> Vec<Finding>`, not `Result<(), Error>`. One run tells
  you everything wrong with a foreign file, which is what an import adapter needs to explain a
  rejection. Matches Parallax's "refuse with the reason named" rule.
  — **Reversibility:** costly — public signature; the `Finding` type becomes part of the contract.

### Reader scope in Phase 1

- **D-10:** **Full read/write pairing from the first commit.** Every OBU type gets `read` and
  `write` in the same file, in that order, in the same commit — STACK.md's stated architecture
  applied without exception, so asymmetry is visually obvious. Phase 2 then becomes exactly what
  its description claims: sequence-level parser, `ParamDefinitionRegistry`, round-trip properties,
  fuzzer.
  **Guard against the closed loop:** round-trip tests are **supplementary evidence only**.
  BITS-06's hand-computed vectors and the reference oracles remain the primary proof. The roadmap's
  own justification for Phase 2 depending on Phase 1 is that "a parser that mirrors a
  misunderstanding passes its own round-trips forever" — pairing is for auditability, not evidence.

### Reference oracles and the conformance gate

- **D-11:** **Split the oracles by build cost.** `tools/build-reference.sh` builds `libiamf`
  natively on macOS at its pinned SHA (CMake, one submodule, yields `iamfdec`), so
  `IAMF_REF_DECODER` is set locally and CONF-05 runs in seconds while iterating. `iamf-tools`
  (Bazel + abseil + protobuf + fdk-aac) runs **only in a digest-pinned container**, used for
  CONF-11's one-time fixture generation and the Linux CI job enforcing CONF-06/07. Matches how
  often each oracle is actually needed. CONF-10 still holds: `cargo test` is green offline on all
  four targets with no reference binary present.

- **D-12:** CONF-07's byte-diff writeup is an **executable ledger** — a committed table of known
  differences (offset, field, our bytes, theirs, why) that a test asserts against **exactly**. A
  new difference fails; a difference that *disappears* also fails, forcing the ledger to be
  updated. The prose explanation and the enforcement are one artifact, so success criterion 3 can
  never pass on a stale document.

- **D-13:** GUARD-06 is enforced at runtime, not just by script. `tools/build-reference.sh` writes
  `.reference-manifest.json` recording the SHA it actually checked out plus a hash of each binary
  produced; the `iamf-tools` container image is pinned **by digest, not tag**; a test asserts the
  manifest matches `REFERENCES.md` **before any conformance clause runs**. Silent drift becomes a
  named error rather than a green gate against the wrong reference.

- **D-14:** CONF-11 fixture policy — one Bazel run generates everything; commit every output under
  a stated **per-file size cap**, plus a `MANIFEST.md` listing all 338 textprotos, which produced
  output, which were committed, which were skipped and why, and the exact pinned-container command
  to regenerate any of them.
  — **Reversibility:** costly — fixtures committed to git history cannot be un-committed without a
  history rewrite, and Parallax consumes this crate as a path dependency, so they land in every
  Parallax developer's clone.

- **D-15:** **FFmpeg is an oracle, never a reference.** ffmpeg 9.0.1 is present on the dev machine
  with both an IAMF demuxer and muxer for the raw container (verified 2026-09-08; that build is
  `--enable-gpl --enable-version3`).
  - **Phase 1 (now):** write the boundary down. GUARD-08's `CONTRIBUTING.md` checkbox is extended
    to name **FFmpeg / libavformat** alongside `gpac` and `libspatialaudio`, and `REFERENCES.md`
    records the read-vs-invoke distinction explicitly.
  - **Phase 2:** wire ffmpeg in as a third, independent-lineage read oracle.
  **The distinction, stated for the record:** *reading* `libavformat/iamf*.c` is contamination —
  `libavformat` is LGPL-2.1+, the same licence class as `gpac`, which PROJECT.md forbids reading or
  porting. *Invoking* the `ffmpeg` binary from a test is not: separate process, nothing links,
  nothing distributed together — the identical pattern already blessed for `libiamf`. The GPL build
  flags do not change this, because the GPL's reach is over a combined work. This must be written
  down because the trap is a developer debugging a byte-diff opening `iamf_writer.c` "just to
  check"; contamination is irreversible and relicensing needs every contributor's agreement.

- **D-16:** GUARD-09's matrix runs **everything on all four targets on every PR** — not a reduced
  golden-hash-only job.
  **Known operational risk to carry into plan 01-01:** GitHub's `macos-13` is the last x86_64 macOS
  image and is on the retirement path (`macos-14`/`15` are arm64). Plan 01-01 must carry a
  documented fallback — cross-compile x86_64 and run under Rosetta on an arm64 runner, or drop to
  three targets with the reason written down — rather than discovering it as a red gate.

- **D-17:** **Gate waiver policy.** If a clause of the seven-clause gate proves unachievable for a
  reason that is not a defect, it may be downgraded **only** by a committed `CONFORMANCE-GATE.md`
  entry naming the clause, what was attempted, why it cannot be met, what weaker property is
  asserted instead, and what would let it be restored. The phase then exits with the waiver
  visible. Decided now, while nothing is at stake, because two clauses are partly outside our
  control: CONF-08 requires reproducing `test_000003.iamf` "from an equivalent configuration" that
  is not published and must be inferred from the file, and CONF-06/07 depend on an `iamf-tools`
  v1.x tag that DEC-04 has not yet identified.

### Conformance fixture design

- **D-18:** The Phase 1 fixture is **two Codec Configs and one 5.1 Audio Element**, 48 kHz, 24-bit
  **big-endian**, length a deliberate non-multiple of the frame size, signal a per-channel
  deterministic index pattern.
  - **Two Codec Configs, one Audio Element** — not two elements. `iamfdec` decodes a *mix
    presentation*, rendering every element in the sub-mix into the target layout and summing them,
    so a two-element fixture makes the decoded PCM a **mix** that we could only predict by
    rendering — and rendering is explicitly out of scope (Parallax owns it under `D-40`). One
    sub-mix, one element, unity gain, target layout 5.1 ⇒ the decoder's output *is* our input and
    sample-identity is a direct comparison. CONF-04's stated purpose ("two Codec Configs **or** two
    Audio Elements, so ordering is observable") is met by the repeated descriptor type.
  - **5.1, not stereo** — the phase's highest silent-failure risk is BCG channel→substream packing
    (coupled pairs first, then mono), which produces a clean decode with scrambled channels: no
    error, no crash, correct byte count. Stereo cannot detect it (one coupled pair, zero mono
    channels). 5.1 packs as L/R coupled, Ls/Rs coupled, then C and LFE mono — two pairs, two monos,
    ordering fully observable.
  - **24-bit big-endian** — DESC-03 warns `sample_format_flags == 0` means *big*-endian, the
    opposite of a WAV-shaped assumption. A 3-byte big-endian sample is where that gets written
    wrong; 16-bit hides half the mistake.
  - **Non-multiple length** — forces `trim_at_end > 0` (CONF-03). Note this also covers OBU-05's
    "a test where the two trim values differ is mandatory": LPCM has no priming, so
    `trim_at_start = 0` and `trim_at_end = N`. A swapped write order would trim the wrong end and
    CONF-05's sample-identity catches it.
  — **Reversibility:** costly — the golden hash, the annotated structural dump and the diff ledger
  are all derived from this exact configuration; changing it invalidates all three.
  — **AMENDED 2026-09-08 (user decision), after the research item below was executed.**
  The two-Codec-Config design does not survive contact: `encoder_main` accepts it and
  `libiamf`'s `iamfdec` decodes it **byte-identically** to the golden, but `iamf-tools`'
  `decoder_main` — the binary CONF-06 must run through, since v2.1.0 ships no `probe_main` —
  **aborts on an absl `CHECK`**. Causality isolated by control: the same textproto with the
  second `codec_config_metadata` block deleted decodes to `63 temporal units`; with it, the
  decoder dies. Fallback **(b)** adopted: sample-identity (CONF-02/03/05) stays on the
  **single-element 5.1 fixture**, so D-19's no-permutation-constant premise is untouched, and
  CONF-04's ordering observability moves to a **second, structure-only fixture with two Audio
  Elements** (77 of the 226 reference files have ≥2, so the route is well precedented).
  Fallback (a) — the `arbitrary_obu` injection — was rejected on its own recorded caveat:
  `INSERTION_HOOK_AFTER_CODEC_CONFIGS` makes ordering positional rather than ID-sorted, so
  DESC-08's ascending-ID property would no longer be what is observed. Accepted cost: two
  elements push the minimum profile Simple → **Base**, which PROF-02 must exercise anyway.
  Full evidence in `CONFORMANCE-GATE.md` § Experiment 2. **Plan 01-08 inherits two fixtures.**
  — **Research item for plan 01-02 — EXECUTED 2026-09-08, see the amendment above:** confirm `iamf-tools`' strict parser accepts a Codec Config
  that no Audio Element references. If it does not, fall back to differing the two configs and
  having the element use whichever is legal.

- **D-19:** **No permutation constant anywhere in the harness.** The fixture's input PCM is authored
  in the reference decoder's documented output order for the requested layout, cited in the
  harness, so comparison is direct sample-for-sample equality with nothing to tune. On mismatch, a
  diagnostic checks whether *some* permutation would have matched and reports "channels scrambled:
  ch2↔ch4" rather than "PCM differs".
  **Why this matters:** TIME-02 says BCG packing order is distinct from presentation channel order,
  so input order, packing order and `iamfdec`'s WAV order are three different things. A permutation
  constant is a knob, and the cheapest-looking fix for a failing comparison is to turn the knob
  until it goes green — which absorbs a real BCG packing bug into the harness and ships it. No
  knob, no knob to turn.
  — **Reversibility:** costly — the fixture's authored channel order and the golden artifacts all
  depend on it.

- **D-20:** GUARD-09's committed golden is **three artifacts**: the `.iamf` fixture, a hash for the
  four-target comparison, and a **human-readable annotated structural dump** (offset, field name,
  value, hex) generated by our own dumper. The dump is what makes an output change a reviewable PR
  diff — which is GUARD-09's whole stated rationale, and which neither a bare hash (one-line diff,
  explains nothing) nor a bare binary blob delivers. The dumper doubles as the debugging tool for
  the CONF-07 ledger work.
  **Caveat to write down:** the dump is self-consistent with the writer, so it aids **review**,
  never correctness.

### Guardrails, hygiene and sequencing

- **D-21:** GUARD-11's no-DSP guard is **clippy `disallowed-types` on `f32`/`f64`**, extending the
  same `clippy.toml` that already carries GUARD-02's `HashMap`/`HashSet` ban, plus
  `disallowed-methods` for transcendentals (`sin`, `cos`, `powf`, …), each with a `reason` string.
  Compiler-enforced, so local `cargo clippy` and CI agree. PROF-03's Q7.8 helper genuinely needs
  `f64` and carries the **single** documented `#[allow]` — which turns the guard into a census:
  `rg 'allow.*disallowed_types'` should list exactly one entry.

- **D-22:** **Phase 1 ships no Cargo features at all.** There is nothing to gate — no codec crates
  until Phase 3 (dev-only), `bitstream-io` is a dev-dependency (D-01), `thiserror` is unconditional
  (D-02). API-05's gating arrives in plan 04-03 where the drivers and codec libraries it describes
  actually exist. A cfg matrix would also multiply the four-target byte-identity gate that D-16
  makes mandatory on every PR.
  **Note for the doc pass:** PROJECT.md's Key Decisions line "One crate, feature-gated
  `encode`/`decode`" reads as a standing description and will be implemented faithfully in 01-01 by
  an executor unless amended to read as a Phase 4 intention.

- **D-23:** STACK.md's `// ref: iamf/obu/obu_header.cc ObuHeader::ValidateAndWrite` citation
  discipline gets a **mechanical test**: a test walks `src/`, finds every `fn read_*` / `fn write_*`,
  and fails if one lacks a preceding `// ref:` line. ~30 lines, runs on all four targets offline,
  no new dependency. Citations rot silently in exactly the way the byte-diff writeup would have —
  same fix as D-12.

- **D-24:** Plans **01-01 → 01-02 run sequentially**, both before any bitstream code (01-03
  onward). The guardrails land first — `deny.toml`, `clippy.toml`, hardening lints,
  `rust-toolchain.toml`, `REFERENCES.md` — then the external-tooling track. This satisfies the
  roadmap's real intent ("start it on day one rather than discover it late": the Bazel long-pole is
  confronted immediately) without two agents contending over `Cargo.toml`, `.github/workflows/` and
  `REFERENCES.md` — the two plans that create the most root-level files, where a conflict would
  corrupt the guardrails before any code exists to protect. The genuinely slow part of 01-02 is
  container and Bazel wall-clock, which parallel agents do not speed up.

- **D-25:** With `tdd_mode: true`, expected bytes come from **hand-decoded vectors first, reference
  capture as a second, independent assert**. For each OBU type, write the expected bytes by hand
  from the spec and the reference's *source* before the encoder exists — BITS-06 already mandates
  this for the bit primitives, extended to OBUs — then separately assert the reference's own output
  matches. Two independent derivations that must agree: when they disagree you have found either a
  misreading or a real divergence, and you know which. This is the activity that already resolved
  the OBU header three independent ways.
  **Rejected:** capturing reference output as the expected bytes and hand-decoding only on failure.
  That is a regression test, not a specification — it proves you match a blob you never understood,
  and cannot catch the case where you and the reference both do something the spec forbids.

- **D-26:** **Worktree cleanup — done during this discussion** (commit `41eaa4e`). The staged
  `LICENSE` deletion from the DEC-02 dual-licence split was committed, and `.vscode/` was added to
  `.gitignore` (it held only personal editor config — a `files.exclude` list). Plan 01-01 starts
  from a clean tree, and the licence change is attributed to a licence commit rather than swept
  into scaffolding.

### Claude's Discretion

The user explicitly delegated these:

- Module and file layout under `src/` (constrained only by BITS-07 and D-10's pairing rule)
- CI provider specifics and workflow file structure
- The `iamf-tools` container base image and whether it is built in-repo or pulled
- The annotated dump's output format (D-20)
- The `Finding` type's exact fields (D-09)
- The golden-hash file's format and location
- Plan-level task breakdown within each of the 8 plans
- DEC-04 (`iamf-tools` v1.x tag identification) and DEC-05 (`libiamf` payload rejection rules) go
  to the researcher, not back to the user

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Parallax consumer constraints (user-cited during discussion — highest priority)

- `/Users/cell/local/Parallax/docs/BUFFERS.md` §0 — line 17, Media row: *"decoded to `f32` on read,
  cached at session rate; peaks and analysis on the `f32` cache"*. Grounds the offline-only
  finding.
- `/Users/cell/local/Parallax/docs/IO.md` §1 — line 19, Lossy row: *"decoded once into the `f32`
  cache (`BUFFERS.md` §0), never re-decoded"*. Line 22, IAMF row: the import mapping — *"object
  elements → tracks with sources and position automation, scene elements → Ambisonic clips, channel
  elements → layout-mapped clips; mix presentations → scenes with snapshots"*. This is the source
  of the decode-is-a-superset finding behind D-04.
- `/Users/cell/local/Parallax/docs/eclipsa/04-PARALLAX-INTEGRATION.md` — features **5.1 IAMF
  Output** (real-time monitoring; a Standard Output on an IAMF target layout plus metadata — **does
  not touch this crate at all**), **5.2 Playback/verify** (decode an `.iamf` once into the cache),
  **5.3 Exporter** (offline file writing; encode).

### This repo

- `/Users/cell/local/iamf-rs/.planning/PROJECT.md` — Core Value, Context (the OBU header resolved
  three ways; `libiamf` is a permissive reader; the minimum viable file), Constraints, Key
  Decisions, Open Questions
- `/Users/cell/local/iamf-rs/.planning/REQUIREMENTS.md` — the 63 Phase 1 requirements and the Out
  of Scope table
- `/Users/cell/local/iamf-rs/.planning/ROADMAP.md` — Phase 1 goal, five success criteria, the
  8-plan breakdown
- `/Users/cell/local/iamf-rs/HANDOFF.md` — the original handoff, including the five claims whose
  provenance research corrected
- `/Users/cell/local/iamf-rs/.planning/research/STACK.md` — bit-I/O decision and alternatives,
  parser architecture, error handling, fuzzing, determinism, codec framing, `deny.toml`, the
  three-tier test strategy
- `/Users/cell/local/iamf-rs/.planning/research/ARCHITECTURE.md`
- `/Users/cell/local/iamf-rs/.planning/research/PITFALLS.md`
- `/Users/cell/local/iamf-rs/.planning/research/FEATURES.md`
- `/Users/cell/local/iamf-rs/.planning/research/SUMMARY.md`
- `/Users/cell/local/Parallax/docs/eclipsa/06-SOURCES.md` — marks which facts are verified-from-code
  and which are from prose

### Reference implementations (to be pinned by SHA in `REFERENCES.md`, GUARD-06)

- `github.com/AOMediaCodec/iamf-tools` at a **v1.x tag** — `iamf/common/read_bit_buffer.h`,
  `write_bit_buffer.h`, `iamf/common/leb_generator.h`, `iamf/obu/obu_header.cc`,
  `iamf/obu/types.h`, `iamf/obu/decoder_config/lpcm_decoder_config.h`, `profile_filter.cc`,
  `iamf/cli/testdata/` (338 textprotos). **HEAD is a draft-v2.0.0 tree — do not mirror it.**
- `github.com/AOMediaCodec/libiamf` at a pinned SHA — `code/src/iamf_dec/obu/iamf_obu.c`,
  `codec_config_obu.c`, `audio_frame_obu.c` (DEC-05: payload-level rejection rules),
  `tests/test_000003.iamf` (the golden, CONF-08)

### Forbidden sources (GUARD-08 — may NOT be read or ported)

- `gpac` — LGPL-2.1
- `libspatialaudio` — LGPL-2.1+
- **FFmpeg / libavformat** — LGPL-2.1+ (added by D-15). Invoking the binary is permitted; reading
  the source is not.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

**None — this is a greenfield repository.** No source files exist. The codebase scout found only
`HANDOFF.md`, `README.md`, `LICENSE-MIT`, `LICENSE-APACHE`, `.gitignore` and `.planning/`.

The inverse of the usual scout finding applies: nothing constrains the design, so every convention
established in Phase 1 becomes the pattern all later phases inherit. That raises the stakes on
module boundaries, the error shape (D-08) and the read/write pairing rule (D-10) rather than
lowering them.

### Established Patterns

To be established here, not discovered:

- Read/write pairs in the same file, in that order, with a `// ref:` citation on each —
  mechanically enforced (D-10, D-23)
- `Vec` in bitstream order plus a `by_id()` accessor for all descriptor collections; never
  `BTreeMap`, which reorders bitstream-visible order (DESC-09, PROJECT.md Constraints)
- No `HashMap`/`HashSet` anywhere — `clippy.toml` `disallowed-types` with a `reason` string
  (GUARD-02)
- Hand-written recursive descent mirroring the reference's ~22-function primitive surface; no
  derive macros, because the byte layout must be auditable against C++

### Integration Points

- **Parallax** consumes this crate as a **path dependency** during development. Anything committed
  to `tests/fixtures/` lands in every Parallax developer's clone (see D-14's reversibility note).
- The public surface Parallax matches on: `AudioElementType` (D-04), `Error`/`ErrorKind`/`Location`
  (D-08), `Finding` (D-09). These are the one-way decisions.

</code_context>

<specifics>
## Specific Ideas

### Project-level decision made during this discussion — closes PROJECT.md Open Question 2

The user closed Open Question 2 decisively, in their own words, grounded in two Parallax spec lines
that predate the question:

> **Offline-only, end to end.** Parallax decodes every format once at import into an f32 cache at
> session rate and streams the cache, never the codec (`BUFFERS.md` §0, `IO.md` §1) — so neither
> direction of the crate ever runs on or near the audio callback, and no allocation or locking
> discipline applies to it.

The decoder does not merely avoid the audio callback — it does not run during playback at all, not
even on a deadline thread. There is no soft-realtime middle ground to design for.

**Three constraints the user gave for shaping the API:**

1. **Not on the real-time path.** Allocate freely, use `Vec`, none of the RT rules apply. Designing
   an audio library defensively for RT when it never runs there costs ergonomics for nothing.
   *(Note: SEQ-02's streaming primitive survives unchanged, for a different reason — it is a memory
   ceiling so an hour of 7.1.4 24-bit is never resident, not an RT-safety measure.)*
2. **Same input → same bytes, on four targets.** The constraint that actually binds: no `HashMap`
   iteration reaching the output, no platform `libm`, and a defined float→fixed path. *(PROF-03 is
   the only place a float touches the encode path — one named, range-checked, unit-tested function.
   `round_ties_even` is correct precisely because it is IEEE-754-exact rather than a transcendental,
   so it is identical on every target without `libm`. `as i16` would truncate toward zero and bias
   every negative value upward by up to 1 LSB.)*
3. **Accept a gain curve, not a callback.** The adapter hands over mix-gain automation already
   sampled at the chosen tick rate. Taking a plain slice keeps the decimation decision on
   Parallax's side of the boundary, where it belongs and where the ADM BWF exporter can share it.
   *(Phase 4 detail deferred: whether the slice element is a bare gain value or a
   `(value, animation_type)` pair. Either is a plain slice; neither is a callback.)*

### Correction to PROJECT.md's framing (user-supplied)

PROJECT.md says Parallax needs IAMF for *"both a monitoring/playback output path and an export
format"*. Those are **three** features in Parallax's integration doc, and the split matters:

| Feature | What it is | Touches iamf-rs? |
|---|---|---|
| 5.1 IAMF Output | Real-time monitoring — a Standard Output on an IAMF target layout, rendered every block from the same Sources, carrying mix-presentation gain structure and BS.1770 loudness. No encoding happens. | **No. Not at all.** |
| 5.2 Playback/verify | Play an actual `.iamf` to catch bitstream and profile errors a render cannot | Yes — decode, once, into the cache |
| 5.3 Exporter | Offline file writing | Yes — encode |

The monitoring path — the thing a mixing engineer uses to answer "is my IAMF mix right" — never
touches this crate. It is a render target plus a metadata model. So the half of PROJECT.md's
motivation that says "monitoring/playback" is doing no work.

### Parallax house rule adopted for this crate's API

**Refuse with the reason named, never approximate.** This drove D-04 (`Reserved { value, raw }` so
the adapter can say "this file carries element type 3, which we do not model"), D-06 (validate
reports mismatches by name, never silently normalises) and D-09 (`validate()` returns all findings,
each with its field path).

</specifics>

<deferred>
## Deferred Ideas

### PROJECT.md / REQUIREMENTS.md doc pass (not this workflow's scope — run separately)

1. **Close Open Question 2** with the user's one-line formulation and its `BUFFERS.md` §0 /
   `IO.md` §1 grounding. Move it to Settled.
2. **Correct the motivation** — replace *"both a monitoring/playback output path and an export
   format"* with the 5.1 / 5.2 / 5.3 split above. Feature 5.1 never touches this crate.
3. **Soften DEC-03's recorded cost.** PROJECT.md records *"the decimation policy lives in Parallax
   and must be shared with the ADM BWF exporter, or the two exports will disagree"* as though it
   covers positions. It does not — position is baked into the render at full sample rate for IAMF
   regardless of DEC-03, while ADM BWF carries positions as `audioBlockFormat` with its own
   interpolation. Those exporters diverge on positions because the *formats* differ. The
   shared-policy obligation reduces to **mix-gain automation only**.
4. **REQUIREMENTS.md: DECO-02 is now dead text** — *"if the decoder is ever used for in-DAW
   playback, allocation and locking rules apply to that path and the API must be shaped for it from
   the start"* is closed out by the offline-only decision.
5. **Amend the Key Decisions line** "One crate, feature-gated `encode`/`decode`" to read as a
   Phase 4 intention rather than a standing description (see D-22).

### Research-pass items (Phase 1, for the researcher — not user questions)

6. **Verify at the pinned tree:** does `MixGainParameterData` carry per-subblock animation types
   (step / linear / bezier)? If so, DEC-03 costs even less than recorded — a ramp is endpoints plus
   a type, not N samples, so "pre-decimated blocks" need not lose resolution.
   **Labelled needs-confirming, not verified** — this came from the general shape of the v1.1 spec
   and `parameter_block.cc`'s `GetMixGainAtTime`, not from a read of the pinned tree.
7. **Confirm** `iamf-tools`' strict parser accepts a Codec Config that no Audio Element references
   (D-18's fallback depends on it).

### Phase 2

8. **Wire ffmpeg in as a third, independent-lineage read oracle** (D-15). The licence boundary is
   written down in Phase 1; the oracle itself waits.

### v2 / decoder

9. **Spec-version collision to confront before the v2 element-type enum is set.** `IO.md` §1
   specifies importing IAMF **object elements** with position automation. But IAMF v1.1.0's
   `audio_element_type` is a 3-bit field with two defined values — `CHANNEL_BASED` (0) and
   `SCENE_BASED` (1); 2–7 reserved. Object-based is a **draft-v2.0 addition**, which is why
   `profile_filter.cc` erases Simple/Base/Base-Enhanced the moment an element is object-based and
   why `libiamf` cannot decode one. So a v1.1.0-pinned decoder cannot satisfy `IO.md` §1's object
   mapping at all. Not a gap in this crate — a spec-version consequence the v2 decoder work must
   confront. **Labelled needs-confirming against the pinned tree.**
   D-04's `Reserved { value, raw }` variant means such files still round-trip byte-identically in
   the meantime, and the adapter can name the reason it will not model them.

### Out of scope, raised and redirected

None — the discussion stayed within the phase boundary. The API-shaping constraints the user gave
(offline-only, byte-determinism, gain curve as a slice) are Phase 4 material and are recorded above
rather than acted on here.

</deferred>

---

*Phase: 1-Conformant LPCM Bitstream*
*Context gathered: 2026-09-08*
