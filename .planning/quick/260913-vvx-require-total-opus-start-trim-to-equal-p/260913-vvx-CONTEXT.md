# Quick Task 260913-vvx: require total Opus start trim to equal pre_skip in the encoder - Context

**Gathered:** 2026-09-13
**Status:** Ready for research

<domain>
## Task Boundary

Resolve deferred item 1 of `.planning/quick/260913-n56-enforce-parameter-block-duration-and-uni/260913-n56-deferred-items.md`
(Opus `pre_skip` compared with the total start trim).

</domain>

<decisions>
## Implementation Decisions

- **User decision (2026-09-13): "ja, mit genau gleich umsetzen"** ("yes, implement with exactly
  equal").
  - The total number of samples trimmed at the start of an Opus IA sequence SHALL equal the Codec
    Config's `pre_skip`.
  - Source: IAMF v1.1.0 `index.bs:1819`, "Pre-skip SHALL be the same as the number of audio samples
    to be trimmed at the start of coded Audio Substreams".
  - This satisfies iamf-tools@v2.1.0 `audio_frame_generator.cc:141-156`, which only requires the
    trim to be at least `pre_skip`. The project rule is to satisfy both the spec and the references,
    and the spec wins where it is stricter (see
    `.planning/quick/260913-qk3-enforce-iamf-v1-1-0-profile-restrictions/260913-qk3-CONTEXT.md`).
- **Scope:** Opus Codec Configs only. LPCM and FLAC have no `pre_skip` and are unaffected. Research
  checks whether AAC-LC (if present in this crate) has an analogous rule. If it does, record it; do
  not implement it.
- **Placement:** the rule belongs in the high-level encoder path (`EncodingWriter` / `finish()`).
  - The low-level writers must keep round-tripping foreign files byte-exactly.
  - Research decides whether `ParsedSequence::validate()` / `DescriptorSet::validate()` should also
    report a finding, and what it would cost in reference `semantic_sha256` changes. A hash may
    change only for a fixture that the pinned iamf-tools testdata marks `is_valid: false`, or with a
    full explanation; otherwise defer it.
- **Errors:** payload-free `ErrorKind` variants, keeping the 32-byte assertion.

### Claude's Discretion
- Exactly when a mismatch is detected: at the first unit without a start trim, at `finish()`, or both.
  Also how to handle streams that are trimmed entirely or are empty.
- Test layout.

</decisions>

<specifics>
## Specific Ideas

- `tests/encoder_streaming.rs:141-185` (Opus, `pre_skip` 312, no trim) must be migrated so it trims
  312 samples at the start.
- The committed Opus codec fixtures and the `phase3_opus` conformance fixture must still pass
  CONF-06 (`decoder_main`). Research checks whether their start trims already equal `pre_skip`; if
  not, stop and report before changing fixture bytes.
- Gates: fmt, clippy (with and without the fuzzing feature), full `cargo test --locked`, fuzz replay,
  conformance with
  `IAMF_REF_DECODER=/Users/cell/local/iamf-rs/.reference/libiamf/code/test/tools/iamfdec/iamfdec`.
  FLAC/Opus CONF-05 skips on this host by manifest design; CONF-06 still runs.
- Golden fixtures and `DIFF-LEDGER.md` stay unchanged.

</specifics>

<canonical_refs>
## Canonical References

- IAMF spec `v1.1.0` `index.bs:1819` (Opus pre-skip) and `:541-542` (start and end trim placement).
- `iamf-tools@848c6ff4` `iamf/cli/proto_conversion/proto_to_obu/audio_frame_generator.cc:141-156`.
- `libiamf@f06e919e` Opus decoder pre-skip handling.
- `.planning/quick/260913-n56-enforce-parameter-block-duration-and-uni/260913-n56-RESEARCH.md` (T3/T4 trim state in `EncodingWriter`).

</canonical_refs>
