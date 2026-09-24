# Delivery Conformance and Profile Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Make the public encoder able to reject incomplete delivery sequences before writing, validate a complete finite timeline before emission, and lock IAMF v1.1 profile selection to the pinned iamf-tools v2.1.0 reference matrix.

**Architecture:** Keep the append-only Encoder::start streaming API unchanged because it deliberately permits descriptor fragments. Add an explicit finite-delivery validation path which works on an immutable Encoder plus a slice of TemporalUnitInput values before a sink is touched. Factor shared temporal checks so the normal writer and the preflight validator cannot drift. Keep profile selection in encoder.rs but drive it from a table-based test matrix whose expected values are derived only from the pinned v1.1 reference.

**Tech Stack:** Rust 1.85, existing iamf public API, no new normal dependencies, existing Rust integration tests and Docker reference gate.

**Spec:** docs/IAMF-V1.1-COMPLETENESS-AUDIT.md, especially P1/P2 and §§2.4, 4 and 5; docs/iamf/v1.1.0.html §§2.4, 4 and 5; REFERENCES.md pins iamf-tools@848c6ff4968ff8cc6f728259892ab4f90cb83256 and libiamf@f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63.

## Global Constraints

- Rust MSRV is 1.85; retain edition 2024 and forbid unsafe code.
- Keep zero linked dependencies other than thiserror; do not add an ISO-BMFF, codec, DSP or media-framework dependency.
- Do not break Encoder::start or EncodingWriter: streaming descriptor fragments remain valid for their existing callers.
- All input validation must be checked arithmetic, panic-free, and return Error with a static Location::Field path.
- Treat the pinned reference commits above as the only v1.1 behavioral oracle; docs/iamf-tools HEAD is a newer draft line and must not define expected results.
- Do not claim a decoder, renderer, OAR implementation, ISO-BMFF packager, or codec encoder as part of this work.
- Every task ends with cargo test --locked, cargo clippy --locked --all-targets -- -D warnings, cargo fmt --check, and git diff --check before commit.

---

## File structure

| File | Change | Responsibility |
|---|---|---|
| src/error.rs | Modify | Add additive, typed delivery-validation error variants. |
| src/encoder.rs | Modify | Public finite delivery validation API, shared temporal preflight, timeline state machine, and profile code only where test fixes require it. |
| tests/encoder_delivery.rs | Create | Public delivery API tests: no presentation, valid finite sequence, no partial output, trim ordering and duration behavior. |
| tests/profile_matrix.rs | Create | Pinned iamf-tools v2.1.0 profile-filter matrix expressed through the public builder. |
| tests/encoder_builder.rs | Modify | Retain focused builder regressions and move only duplicated profile coverage to the matrix suite. |
| REFERENCES.md | Modify | Cite the exact pinned source file/function and explain that the matrix is a behavioral snapshot, not a HEAD comparison. |
| README.md | Modify | State the distinction between streaming descriptor fragments and the explicit complete-delivery validator. |
| docs/IAMF-V1.1-COMPLETENESS-AUDIT.md | Modify | Mark P1/P2 resolved only after the implementation and reference gate pass; retain deferred authoring/renderer scope. |

The plan deliberately does not create a CompleteIaSequence byte container. The new finite validator is a public preflight seam required by a later iamf-isobmff-rs adapter, while bytes remain owned by the existing caller-selected sink.

## Delivery API contract

Add these public methods to Encoder:

~~~
pub fn validate_delivery_conformance(&self) -> Result<()>;

pub fn validate_delivery_timeline(
    &self,
    units: &[TemporalUnitInput],
) -> Result<()>;
~~~

validate_delivery_conformance checks frozen descriptors only. It requires at
least one Mix Presentation, re-runs descriptor reference validation and checks
that the v1.1 profile pair produced by the builder is internally valid. It does
not require any temporal data because an empty duration is not itself a reason
to reinterpret the existing fragment API.

validate_delivery_timeline first calls validate_delivery_conformance. It then
validates every unit with the same rules used by EncodingWriter::preflight,
without writing bytes. It enforces the additional finite-delivery invariants:

- every Audio Frame in a unit has the frozen samples-per-frame duration;
- all frames of a unit have one shared trimming value;
- nonzero start trimming occurs only in the first unit;
- nonzero end trimming occurs only in the final unit;
- trim sums remain within the frozen frame duration;
- parameter blocks are validated against their governing definitions and their
  declared duration fields are internally consistent;
- checked cumulative decode and presentation sample counters never overflow.

Do not make finish on the streaming writer retroactively reject already-written
bytes. A finite caller must call validate_delivery_timeline before start and
push_temporal_unit. This preserves both no-partial-unit semantics and the
existing incremental API.

### Task 1: Typed descriptor-level delivery boundary

**Files:**
- Modify: src/error.rs
- Modify: src/encoder.rs
- Create: tests/encoder_delivery.rs
- Modify: README.md
- Modify: docs/IAMF-V1.1-COMPLETENESS-AUDIT.md

**Interfaces:**
- Consumes: Encoder::descriptors(), DescriptorSet, Error and Location.
- Produces: Encoder::validate_delivery_conformance(&self) -> Result<()>.
- Error kinds: MissingDeliveryMixPresentation and InvalidDeliveryDescriptorSet.
- Later task dependency: validate_delivery_timeline calls this method first.

- [ ] **Step 1: Write failing public tests**

Create tests/encoder_delivery.rs with a minimal helper that constructs one
LPCM codec config, one channel Audio Element, two substream handles and one
valid Mix Presentation. Add these tests:

~~~
#[test]
fn delivery_validation_rejects_a_descriptor_fragment_without_a_mix_presentation() {
    let mut builder = EncoderBuilder::new();
    let config = builder.add_codec_config(lpcm_config());
    builder.add_audio_element(config, mono_element());
    let (encoder, _) = builder.build().unwrap();

    let error = encoder.validate_delivery_conformance().unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::MissingDeliveryMixPresentation);
    assert_eq!(error.at(), Location::Field("mix_presentations"));
}

#[test]
fn delivery_validation_accepts_a_complete_static_descriptor_set() {
    let (encoder, _) = complete_lpcm_builder().build().unwrap();
    encoder.validate_delivery_conformance().unwrap();
}

#[test]
fn normal_streaming_start_still_allows_a_descriptor_fragment() {
    let (encoder, _) = fragment_builder().build().unwrap();
    let writer = encoder.start(Vec::new()).unwrap();
    assert!(writer.bytes_written() > 0);
}
~~~

- [ ] **Step 2: Run the new tests to verify failure**

Run:
~~~
cargo test --locked --test encoder_delivery -- delivery_validation
~~~

Expected: compilation failure because validate_delivery_conformance and
MissingDeliveryMixPresentation do not exist.

- [ ] **Step 3: Add minimal typed errors**

In src/error.rs add non-payload variants:

~~~
#[error("a complete delivery sequence requires at least one Mix Presentation")]
MissingDeliveryMixPresentation,
#[error("the frozen descriptors are not valid for a complete delivery sequence")]
InvalidDeliveryDescriptorSet,
~~~

Use Location::Field("mix_presentations") for the missing-presentation case and
Location::Field("descriptors") when static descriptor validation reports any
finding. Do not use String payloads or a new error wrapper; Error remains
within its existing size assertion.

- [ ] **Step 4: Implement static validation**

In the Encoder impl in src/encoder.rs add
validate_delivery_conformance. Its order is:

1. Reject descriptors.mix_presentations.is_empty().
2. Call validate() on the frozen DescriptorSet and translate a nonempty finding
   list to InvalidDeliveryDescriptorSet at descriptors.
3. Recompute the profile candidate pair with select_sequence_profile over the
   frozen codec configs, presentations and elements.
4. Reject if that pair does not equal the pair in sequence_header.
5. Return Ok otherwise.

Keep Encoder::start untouched. The new method is opt-in and finite-delivery
specific.

- [ ] **Step 5: Run focused tests and documentation check**

Run:
~~~
cargo test --locked --test encoder_delivery
cargo test --locked --test encoder_streaming
~~~

Expected: all pass; the third test proves streaming compatibility.

Update README consumer-boundary text to say the normal writer permits frozen
descriptor fragments, whereas a file-delivery caller must invoke the explicit
delivery validator. Update the audit P1 wording from “open” to “implemented,
pending timeline and reference gate” only after the tests are green.

- [ ] **Step 6: Commit**

~~~
git add src/error.rs src/encoder.rs tests/encoder_delivery.rs README.md docs/IAMF-V1.1-COMPLETENESS-AUDIT.md
git commit -m "feat: validate complete IAMF delivery descriptors"
~~~

### Task 2: Shared finite timeline preflight

**Files:**
- Modify: src/error.rs
- Modify: src/encoder.rs
- Modify: tests/encoder_delivery.rs
- Modify: tests/encoder_streaming.rs

**Interfaces:**
- Consumes: Encoder::validate_delivery_conformance, TemporalUnitInput,
  EncodingWriter temporal preflight, CodecConfig and Trimming.
- Produces: Encoder::validate_delivery_timeline(&self, units:
  &[TemporalUnitInput]) -> Result<()>.
- Error kinds: DeliveryStartTrimNotFirst, DeliveryEndTrimNotFinal and
  DeliveryTimelineOverflow.
- Invariant: no bytes are written by the new method.

- [ ] **Step 1: Write failing timeline tests**

Append these cases to tests/encoder_delivery.rs using the complete LPCM helper:

~~~
#[test]
fn delivery_timeline_accepts_untrimmed_units_and_a_final_end_trim() {
    let (encoder, handles) = complete_lpcm_encoder();
    let units = vec![
        stereo_unit(handles, None),
        stereo_unit(handles, Some(Trimming { at_start: 0, at_end: 4 })),
    ];
    encoder.validate_delivery_timeline(&units).unwrap();
}

#[test]
fn delivery_timeline_rejects_start_trim_after_the_first_unit() {
    let (encoder, handles) = complete_lpcm_encoder();
    let units = vec![
        stereo_unit(handles, None),
        stereo_unit(handles, Some(Trimming { at_start: 1, at_end: 0 })),
    ];
    let error = encoder.validate_delivery_timeline(&units).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::DeliveryStartTrimNotFirst);
    assert_eq!(error.at(), Location::Field("temporal_units.trimming.at_start"));
}

#[test]
fn delivery_timeline_rejects_end_trim_before_the_final_unit() {
    let (encoder, handles) = complete_lpcm_encoder();
    let units = vec![
        stereo_unit(handles, Some(Trimming { at_start: 0, at_end: 1 })),
        stereo_unit(handles, None),
    ];
    let error = encoder.validate_delivery_timeline(&units).unwrap_err();
    assert_eq!(error.kind(), &ErrorKind::DeliveryEndTrimNotFinal);
    assert_eq!(error.at(), Location::Field("temporal_units.trimming.at_end"));
}
~~~

Also add one test with u32::MAX-compatible unit count/duration inputs through a
small internal timeline-state unit test in encoder.rs. It must assert
DeliveryTimelineOverflow instead of wraparound.

- [ ] **Step 2: Run tests to verify failure**

Run:
~~~
cargo test --locked --test encoder_delivery -- delivery_timeline
~~~

Expected: compilation failure because validate_delivery_timeline and its three
error kinds do not exist.

- [ ] **Step 3: Factor non-writing temporal validation**

In src/encoder.rs extract the validation portion of
EncodingWriter::preflight into a private helper taking:

~~~
fn validate_temporal_input(
    descriptors: &DescriptorSet,
    generation: u64,
    input: &TemporalUnitInput,
) -> Result<ValidatedTemporalUnit>;
~~~

ValidatedTemporalUnit is private and contains only duration and normalized trim
metadata needed by the timeline pass. Existing EncodingWriter::preflight calls
this helper before it lowers owned FrameInput payloads into AudioFrame OBUs.
The helper must retain the existing checks for duplicate/missing/order-mismatched
substreams, handle generations, frame codec kind, LPCM byte/sample count,
parameter handle/ID/context, parameter subblock duration validity and trim
versus codec frame size.

This prevents the delivery checker from becoming a second incomplete temporal
implementation.

- [ ] **Step 4: Implement finite timeline validation**

Add Encoder::validate_delivery_timeline. It:

1. calls validate_delivery_conformance;
2. visits the supplied units by index and invokes validate_temporal_input;
3. derives one checked unit decode duration from frozen codec config;
4. accepts nonzero start trim only at index zero;
5. accepts nonzero end trim only at the final index;
6. checked-adds decode duration and presentation duration
   (duration minus both trims) to u64 counters;
7. returns the typed overflow error at temporal_units.timeline.

The method must borrow the unit slice and must not call SequenceWriter, Write,
or allocate frame payload copies.

- [ ] **Step 5: Prove no regression and no write side effects**

Add this test:

~~~
#[test]
fn rejected_delivery_timeline_does_not_change_a_later_sink() {
    let (encoder, handles) = complete_lpcm_encoder();
    let invalid = vec![
        stereo_unit(handles, Some(Trimming { at_start: 0, at_end: 1 })),
        stereo_unit(handles, None),
    ];
    assert!(encoder.validate_delivery_timeline(&invalid).is_err());

    let writer = encoder.start(Vec::new()).unwrap();
    assert!(writer.bytes_written() > 0);
}
~~~

Run:
~~~
cargo test --locked --test encoder_delivery
cargo test --locked --test encoder_streaming
cargo test --locked --test temporal
~~~

Expected: all pass, including existing incremental writer tests.

- [ ] **Step 6: Commit**

~~~
git add src/error.rs src/encoder.rs tests/encoder_delivery.rs tests/encoder_streaming.rs
git commit -m "feat: preflight complete IAMF delivery timelines"
~~~

### Task 3: Pinned v1.1 profile-filter matrix

**Files:**
- Create: tests/profile_matrix.rs
- Modify: tests/encoder_builder.rs
- Modify: REFERENCES.md
- Modify: docs/IAMF-V1.1-COMPLETENESS-AUDIT.md

**Interfaces:**
- Consumes: public EncoderBuilder, public descriptor types, IdManifest::sequence_profile.
- Produces: a table-driven set of v1.1 profile expectations and explicit
  unsupported cases.
- Reference source: iamf-tools commit 848c6ff4968ff8cc6f728259892ab4f90cb83256,
  profile_filter.cc and its associated tests at that exact tree.

- [ ] **Step 1: Freeze the reference case table before coding**

In tests/profile_matrix.rs define a local Case struct:

~~~
struct Case {
    name: &'static str,
    codec_configs: usize,
    elements: usize,
    channels_per_element: u8,
    expanded_layout: bool,
    submixes: usize,
    headphones_reserved: bool,
    expected: Option<Profile>,
}
~~~

Populate at least these named cases:

| Name | Expected |
|---|---|
| one element, stereo, one submix | Some(Simple) |
| two elements, 5.1 total output, one submix | Some(Base) |
| one element, expanded 9.1.6, one submix | Some(BaseEnhanced) |
| 29 output channels | None |
| two submixes | None |
| reserved headphones rendering mode | None |
| no submixes | Some(Simple) |
| two codec configs | None |

Add a source comment on every table group identifying the pinned commit and
reference test/function. Do not derive expected values from docs/iamf-tools.

- [ ] **Step 2: Run the matrix to establish its first failing case**

Run:
~~~
cargo test --locked --test profile_matrix
~~~

Expected: compile failure because the test target does not exist, then one
behavioral failure if a current builder/profile mismatch is discovered. Record
the failing case before changing production code.

- [ ] **Step 3: Implement only demonstrated filter corrections**

Run every case through EncoderBuilder and inspect the result:

~~~
let result = build_case(case);
assert_eq!(result.map(|(_, ids)| ids.sequence_profile()), case.expected);
~~~

If a case fails, make the smallest correction in select_sequence_profile or
filter_profiles_for_audio_element. Preserve these deliberate v1.1 rules:

- exactly one unique Codec Config is required;
- zero submixes are not an automatic profile failure;
- more than one submix has no supported v1.1 profile in this writer;
- Expanded layouts require Base Enhanced;
- reserved element or headphones mode has no supported profile;
- primary and additional profile remain equal.

Do not add Base Advanced or any later draft profile.

- [ ] **Step 4: Add differential reference coverage**

Add a conformance fixture that serializes one representative accepted Simple,
Base and Base Enhanced case. Run the existing Docker reference gate so
iamf-tools and libiamf accept the outputs. Keep profile-matrix unit tests
offline; external references remain in the existing optional gate.

Run:
~~~
cargo test --locked --test profile_matrix
cargo test --locked --test encoder_builder
cargo test --locked --test conformance
~~~

When Docker/reference tooling is available, also run the documented targeted
reference command from CONTRIBUTING.md or the CI workflow, and record the
pinned image/commit in the test report.

- [ ] **Step 5: Update provenance and audit**

In REFERENCES.md add a “Profile matrix” paragraph naming the exact
iamf-tools pin, source path and the rule that local HEAD is not an oracle.
In the audit change P2 from “matrix missing” to “pinned matrix covered,” while
leaving advanced authoring APIs as deferred.

- [ ] **Step 6: Commit**

~~~
git add src/encoder.rs tests/profile_matrix.rs tests/encoder_builder.rs tests/conformance.rs REFERENCES.md docs/IAMF-V1.1-COMPLETENESS-AUDIT.md
git commit -m "test: lock IAMF v1.1 profile selection to reference matrix"
~~~

### Task 4: Final verification and delivery documentation

**Files:**
- Modify: README.md
- Modify: docs/IAMF-V1.1-COMPLETENESS-AUDIT.md
- Modify: CHANGELOG.md, if the repository maintains unreleased entries

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces: user-facing statement of the streaming-versus-delivery boundary and
  a verified audit status.

- [ ] **Step 1: Write documentation acceptance assertions**

Add a small docs-focused test only if the repository already uses source-text
contract tests; otherwise review the exact README snippets manually. The
required wording must state:

- Encoder::start remains incremental and may write a descriptor fragment.
- validate_delivery_conformance and validate_delivery_timeline are required
  before treating bytes as a complete deliverable sequence.
- The validators do not decode, render or ISO-BMFF encapsulate audio.

- [ ] **Step 2: Run the full local gate**

Run:
~~~
cargo test --locked -q
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
git diff --check
cargo tree -e normal,no-proc-macro
~~~

Expected: all Rust tests and lint/format/diff checks pass; the normal linked
tree remains iamf plus thiserror.

- [ ] **Step 3: Run the pinned reference gate**

Run the repository’s documented Docker/reference conformance command. Confirm:

- the invoked iamf-tools tree is 848c6ff4968ff8cc6f728259892ab4f90cb83256;
- the invoked libiamf tree is f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63;
- Simple, Base, Base Enhanced and AAC-LC representative outputs are accepted;
- failure output, if any, is preserved as a fixture/reproduction rather than
  papered over with a relaxed test.

- [ ] **Step 4: Review and commit**

Review the audit with each P1/P2 item mapped to a test target. Ensure no item
claims renderer, decoder or ISO-BMFF implementation. Then commit:

~~~
git add README.md docs/IAMF-V1.1-COMPLETENESS-AUDIT.md CHANGELOG.md
git commit -m "docs: document IAMF delivery conformance boundary"
~~~

## Plan self-review

| Audit requirement | Task coverage |
|---|---|
| P1: complete deliverable versus descriptor fragment | Task 1 |
| P1: pre-write global timeline/trimming validation | Task 2 |
| P2: pinned profile-filter behavior | Task 3 |
| Reference-gated proof and honest documentation | Task 4 |
| Decoder, renderer, OAR and ISO-BMFF remain out of scope | Global constraints, Tasks 1–4 |

No task relies on a draft source tree, adds a linked dependency, or requires
changing the streaming API. The two new public methods are additive and use
the existing Error/Location contract.
