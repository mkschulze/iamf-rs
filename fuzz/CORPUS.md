# Permanent fuzz corpus

Every file below is committed replay input. `tests/fuzz_regression.rs` recursively sorts and reads
both corpus trees and any committed `fuzz/artifacts/<target>/` trees; an empty, unreadable, skipped,
or symlinked input fails the stable test instead of silently reducing coverage.

## `parse_sequence`

These are byte-for-byte copies of the four **positive** IAMF files vendored from
`iamf-tools@v2.1.0` (`848c6ff4968ff8cc6f728259892ab4f90cb83256`). The stable test checks the
copy, original, and pinned SHA-256 before invoking the production parser.

| Seed | SHA-256 |
|---|---|
| `noise_1024samp_5p1_opus.iamf` | `2115fd08eee9791f1e35fe4a4bfb268ab02cc096ad9a5cc950aa911999fd1cae` |
| `noise_1024samp_stereo_flac.iamf` | `4bcc7b0ba187da62915a209e1e8ebc02978b4d35cbd392a13d4a4ce9340a269a` |
| `noise_3s_stereo_opus.iamf` | `a91b89d0691fa3e92e29ae2805019bb355081b9ae84cdd715269f4c5f99c5693` |
| `tones_100ms_3OA_stereo_opus.iamf` | `d3d1405cd2ea4b93e6c7cabda110037983862f5b4755573f203b84b8b1d49cdf` |

The evidence-backed vendored-corpus ruling remains 37 positive and two negative IAMFs. Only valid
positive inputs seed fuzzing: `test_000129` is structurally truncated at offset 53 and the zero-frame
fixture is a semantic validation negative, so neither is copied here.

## `obu_roundtrip`

These inputs drive the exact shared `Unstructured` grammar in `iamf::fuzzing`. Rich models always
contain two audio substreams, all parameter contexts (demixing, recon gain, raw extension data,
element mix gain, and output mix gain), reserved values, known trailing bytes, and bounded payloads.

| Seed | Shape selected |
|---|---|
| `empty` | Empty sequence |
| `descriptors-only` | IA Sequence Header only |
| `no-delimiter` | Rich temporal unit without a Temporal Delimiter |
| `delimiter` | Rich temporal unit with a Temporal Delimiter |
| `multi-substream-all-parameters` | Three units, two substreams, and every parameter context |
| `redundant-descriptor` | Redundant Audio Element descriptor copy |
| `reserved-trailing` | Reserved fields and descriptor/parameter trailing bytes |
| `unknown-before` | Unknown OBU before the canonical sequence |
| `unknown-between` | Unknown OBU between known OBUs |
| `unknown-after` | Unknown OBU after the canonical sequence |
| `bounded-raw-parameter-data` | Maximum 32-byte generated frame payload split across raw subblocks |

## Promoting a failure

1. Preserve the CI artifact and its log before changing the input.
2. Minimize with `cargo +nightly-2026-09-01 fuzz tmin <target> <artifact>`, adding
   `--features roundtrip-model` for `obu_roundtrip`.
3. Commit the minimized file under `fuzz/artifacts/<target>/`; never commit only the larger original.
4. If the failure identifies a specific invariant, add a focused, named stable regression as well.
5. Verify `cargo test --locked --features fuzzing --test fuzz_regression -- --nocapture` locally.
