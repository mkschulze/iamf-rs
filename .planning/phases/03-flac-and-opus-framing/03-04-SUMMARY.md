# Phase 03 Plan 04 Summary

Completed the committed, codec-excluded Opus corpus and standalone decode oracle.

- Measured `L=312`, `S=1607`, `P=2`, `E=1`: two raw packets decode to 1,920 frames before trimming and 1,607 stereo frames after it.
- Expected PCM comes from fresh packet decoding, never the lossy source PCM; verifier authenticates lookahead against a configured encoder.
- Root tests authenticate immutable inventory, exact digests, metadata, packet lengths, arithmetic, trims, and raw framing.
- Codec dependencies remain excluded; root normal graph is `iamf -> thiserror`.

Key commits: `df0b37c`, `4defec4`, `25a3294`, `1f6ca8c`.
