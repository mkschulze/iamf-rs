# Phase 03 Plan 07: Offline codec regression closure summary

## Delivered

- Named offline regressions cover CODEC-01 through CODEC-07, including typed
  FLAC/Opus configs, raw/trailing fidelity, rate/roll/trim behavior, parser
  truncation boundaries, streaming and parsed round trips, and stable fuzz
  replay with codec-enriched shapes.
- `tests/fixtures/codecs/README.md` maps every requirement to tests and
  records explicit excluded-workspace generation and verification commands.
- `tests/codec_fixtures.rs` creates a sorted SHA-256 inventory of exactly the
  immutable artifact set: manifests, packet files, and actual `.s16le` PCM.

## Evidence

- 156 targeted offline tests and five fuzz regression tests passed.
- Every FLAC prefix below 38 bytes and Opus prefix below 11 bytes remains Raw;
  truncating the enclosing OBU produces positioned structural errors.
- Before/after inventories list eleven artifacts and are byte-identical across
  `--all-targets` and `--include-ignored` root test runs.
- No root test invokes fixture generation or changes packets, PCM, or
  manifests.

## Next

Plan 03-08 wires this proof into the four-target root CI matrix, retains
generator verification as a Linux-only excluded-package action, and archives
pinned reference conformance evidence.
