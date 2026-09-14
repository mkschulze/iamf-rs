# Deferred Items — Quick Task 260914-pyd

## FYI (not a task): REQUIREMENTS.md text is now stale re: AAC-LC scope

`260914-pyd-CONTEXT.md`'s locked doc scope for this merge was `.planning/PROJECT.md` and
`HANDOFF.md` only — it explicitly did not mention `.planning/REQUIREMENTS.md`. Now that
`origin/feature/aac-lc-framing` is merged, three places in `REQUIREMENTS.md` describe a
codec set or scope boundary that predates AAC-LC framing landing in this crate:

- **`DECO-03`** (line ~169): "Add AAC-LC decoder-config wire syntax here only when required by
  `iamf-decode-rs`; the AAC codec implementation remains outside this crate." AAC-LC decoder-config
  wire syntax (typed read/write) now exists in this crate as of this merge — this requirement's own
  trigger condition has occurred, independent of any `iamf-decode-rs` request.
- **`API-06`** (line ~140): "The frame API accepts LPCM frame payloads or already encoded FLAC/Opus
  access units..." — lists three codecs; AAC-LC access units are now a fourth accepted `FrameInput`
  variant (`FrameInput::AacLc`), exercised by `tests/encoder_streaming.rs`'s
  `lpcm_flac_opus_and_aac_lc_inputs_follow_the_frozen_codec_kind` and the encoder's exhaustive
  `FrameCodecMismatch` match.
- **`API-09`** (line ~143): "...and LPCM/FLAC/Opus framing." — same three-codec list; AAC-LC framing
  is now covered by the same public-API-only contract-fixture pattern this requirement describes.

Both `API-06` and `API-09` are also still marked `Pending` in the traceability table (lines ~285,
~288) despite `260914-pyd-PLAN.md`'s own frontmatter listing `requirements: [API-06, API-09,
CODEC-07]` — this executor did **not** run `requirements mark-complete` for them, per this task's
explicit constraint not to touch `REQUIREMENTS.md`.

**This is flagged as an FYI for the orchestrator/user to route, not something this quick task
executed.** Per `260914-pyd-CONTEXT.md`'s locked doc scope and this task's own explicit
constraint ("Do NOT touch, merge, cherry-pick, or reference `origin/wip/delivery-validator-conformance`"
and the broader instruction to leave `REQUIREMENTS.md`/`ROADMAP.md` alone), no edit to
`REQUIREMENTS.md` was made. A follow-up quick task (or a `requirements mark-complete` call once
the wording is updated) would close this out.
