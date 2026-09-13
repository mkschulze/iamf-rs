# Deferred items — quick 260913-lta

None of these is implemented in this task. Each entry gives its source and why it is out of scope.

## 1. FLAC and Opus Parallax deliveries, each as its own IA sequence

- **Source:** IAMF v1.1.0 `index.bs:1912` ("There SHALL be only one unique Codec Config OBU").
  `iamf-tools@v2.1.0` `iamf/cli/obu_processor.cc` `GetSampleRateAndFrameSize` fails on two configs.
- **What changed here:** the Parallax delivery fixture (`tests/support/parallax_contract.rs`) now
  lowers the `flac archive` and `opus stream` candidates onto the shared LPCM Codec Config.
- **Follow-up:** add one delivery build per codec, each with exactly one Codec Config. This restores
  API-09's contract-fixture codec coverage.
- **Still covered meanwhile:** builder-level FLAC/Opus framing is exercised by the single-config
  builders at `tests/encoder_streaming.rs:153-154`.
- **Why out of scope:** the LOCKED decision limits this task to the single-config rule and the
  fixture repair.

## 2. Parameter Block duration equals Audio Frame duration

- **Source:** IAMF v1.1.0 `index.bs:1915-1916`.
- **Current state:** temporal preflight checks only that the duration is non-zero.
- **Why out of scope:** it is a separate spec rule, listed as a follow-up in the LOCKED block.

## 3. Every Audio Substream carries the same trimming information

- **Source:** IAMF v1.1.0 `index.bs:1913`.
- **Related to:** the trim-position finding in `.planning/reviews/2026-09-13-CODEBASE-REVIEWS.md`.
- **Why out of scope:** it is a separate spec rule, listed as a follow-up in the LOCKED block.

## 4. `DescriptorSet::validate()` finding for more than one Codec Config

- **Source:** the same spec line, `index.bs:1912`, applied to the low-level model (`src/model/mod.rs`).
- **Check first:** `tests/descriptors.rs:1313` pushes a duplicate config.
- **Constraint:** the low-level writers (`SequenceWriter`, `write_sequence`, `write_parsed_sequence`)
  must keep round-tripping foreign files byte-exactly. A finding must not become a write-time
  rejection.
- **Why out of scope:** this task's discretion choice (RESEARCH) keeps the rule in `EncoderBuilder::build()` only.

## 5. Profile selection for expanded layouts

- **Source:** `.planning/reviews/2026-09-13-CODEBASE-REVIEWS.md`.
- **Why out of scope:** it is only noted here, and unchanged.

## 6. Informational, upstream: libiamf buffer growth loop

- **Where:** `libiamf@f06e919e` `code/src/iamf_dec/IAMF_decoder.c:2261`, `iamf_set_stream_info`.
- **Defect:** the loop `for (int n = 0; i < DEC_BUF_CNT; ++n)` tests `i` instead of `n`.
  `DEC_BUF_CNT` is 3.
- [INFERRED] With unequal frame sizes, `n` can index past `DEC_BUF_CNT`.
- **Consequence here:** one more reason `decoder_main`, not `libiamf`, is the gate. The builder can no
  longer emit unequal frame sizes, because one Codec Config fixes the timing.
- **Why out of scope:** it is an upstream defect.

## 7. Disagreement note: Eclipsa vs iamf-tools

- **Finding:** Eclipsa always emits one Codec Config (id 200), which agrees with `iamf-tools@v2.1.0`.
  Nothing to reconcile.

## 8. Resolution note

- **Resolved:** this task resolves the CONF-06 item deferred in
  `.planning/quick/260913-js8-fix-the-count-field-preallocation-memory/deferred-items.md`. That file
  is not edited.

## 9. Discovered during execution: rustdoc broken intra-doc link (pre-existing)

- **Where:** `src/sequence.rs:669`. The link `` [`ParamDefinition`] `` does not resolve, so
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` fails.
- **Status:** it predates this task, and the file was not touched here.
- **Why out of scope:** it is unrelated to this task, and not part of the plan's gate. The new doc
  links on `add_codec_config` and `build` resolve.
