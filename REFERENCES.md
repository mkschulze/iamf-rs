# Reference implementations — pinned (GUARD-06)

Everything this crate claims about the IAMF bitstream is measured against these
two trees at these two commits. Not tags alone, and never `main`: a tag can be
moved, and both projects' `main` branches have moved on to a draft the pinned
decoder cannot read.

| Project | Tag | Commit | Licence | Pinned |
|---|---|---|---|---|
| [`AOMediaCodec/iamf-tools`](https://github.com/AOMediaCodec/iamf-tools) | `v2.1.0` | `848c6ff4968ff8cc6f728259892ab4f90cb83256` | BSD-3-Clause-Clear + AOM Patent License 1.0 | 2026-09-08 |
| [`AOMediaCodec/libiamf`](https://github.com/AOMediaCodec/libiamf) | `v1.1.0` | `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63` | BSD-3-Clause-Clear + AOM Patent License 1.0 | 2026-09-08 |

`iamf::SPEC_VERSION` is `"1.1.0"`. `libiamf@v1.1.0` implements IAMF v1.1.0, and
`libiamf` accepting our output is this crate's Core Value.

Bumping either pin is a deliberate, reviewable commit that must re-run the whole
conformance suite in the same change. A fact read at a pinned SHA does not
expire; that is what pinning buys.

## The version-number caveat — read this before hunting for a v1.1.x tag

PROJECT.md says to pin "an `iamf-tools` **v1.x** tag and SHA". **There is no
`v1.1.x` tag in `iamf-tools`.** The available tags are `v1.0.0` (2024-01-26),
`v2.0.0` (2025-08-18) and `v2.1.0` (2025-11-06). The version number tracks the
**tool**, not the spec: `v2.1.0`'s changelog records that v2.0.0 added "support
for encoding Standalone IAMF Representation for Base-Enhanced profile based on
[IAMF v1.1.0]".

So "a v1.x tag" resolves to a tag numbered **v2.1.0**, and that is not a
mistake. Do not go looking for a v1.1.x tag; it does not exist.

## The four discriminating checks

Each of these separates a v1.1.0-exact tree from the draft-v2.0.0 tree.
`v2.1.0` passes all four. `main` fails all four.

1. **`ProfileVersion`** is exactly `kIamfSimpleProfile = 0`,
   `kIamfBaseProfile = 1`, `kIamfBaseEnhancedProfile = 2`,
   `kIamfReserved255Profile = 255`. `main` adds `kIamfBaseAdvancedProfile = 3`,
   `kIamfAdvanced1Profile = 4`, `kIamfAdvanced2Profile = 5`.
   (`iamf/obu/ia_sequence_header.h`)
2. **OBU type 24** is `kObuIaReserved24 = 24`. `main` has
   `kObuIaMetadata = 24`. (`iamf/obu/obu_header.h`)
3. **`AudioElementType`** is `{0 = channel-based, 1 = scene-based}` with
   "Values in the range of [2 - 7] are reserved". `main` adds
   `kAudioElementObjectBased = 2` and an `ObjectsConfig` variant.
   (`iamf/obu/audio_element.h`)
4. **`ParameterDefinitionType`** is `{0 = mix gain, 1 = demixing,
   2 = recon gain}` with 3 and above reserved. (`iamf/obu/param_definitions.h`)

If a future contributor bumps a pin, re-run all four before anything else. A
tree that fails any of them models a different format, and mirroring it produces
files `libiamf@v1.1.0` rejects.

## The proto-dialect caveat

`iamf-tools@v2.1.0` changed the encoder API to take serialised protos, and
v2.0.0 deprecated `count_label`, `num_substreams`, `num_layers`,
`num_sub_mixes`, `num_audio_elements`, `num_layouts` and
`param_definition_size` in favour of deriving them.

`libiamf@v1.1.0 tests/*.textproto` are written in the **old** dialect and still
carry those fields. **Do not feed them to `encoder_main`.** Use the copies under
`iamf-tools@v2.1.0 iamf/cli/testdata/` instead. Diffing `test_000003.textproto`
between the two trees, the only differences are the deprecated-field removals,
the `channel_ids`/`channel_labels` → `channel_metadatas` migration, and an added
`encoder_control_metadata` block — the configuration itself is identical.

## What "dependency-free" means (D-02)

API-05 calls this crate dependency-free. That means **zero *linked*
dependencies — proc-macro derive crates excepted.**

`thiserror` is unconditional (PROJECT.md: never `anyhow` in a library). It pulls
`thiserror-impl → syn → proc-macro2 → quote → unicode-ident`; none of those
contribute code to the linked artifact. The check is:

```sh
cargo tree -e normal,no-proc-macro
```

which lists `iamf` and `thiserror` and nothing else. Plain `cargo tree -e normal`
is **not** the right instrument — it includes proc-macro edges and will show the
whole derive chain.

This definition is also written into `src/lib.rs`'s crate documentation, so a
later milestone does not rediscover it as a contradiction and "fix" it by
dropping `thiserror`.

## Reading versus invoking — the contamination boundary (D-15, GUARD-08)

**Reading** the source of an LGPL project and re-expressing what you learned in
this crate is contamination. It is irreversible: relicensing afterwards needs
every contributor's agreement.

**Invoking** a compiled binary from a test is not. Separate process, nothing
links, nothing is distributed together. This is the identical pattern already
used for `libiamf`'s `iamfdec`.

| Project | Licence | May we read the source? | May we invoke the binary? |
|---|---|---|---|
| `AOMediaCodec/iamf-tools` | BSD-3-Clause-Clear | **yes** | yes |
| `AOMediaCodec/libiamf` | BSD-3-Clause-Clear | **yes** | yes |
| `gpac` | LGPL-2.1 | **no** | yes |
| `libspatialaudio` | LGPL-2.1+ | **no** | n/a — no DSP in this crate |
| FFmpeg / `libavformat` | LGPL-2.1+ | **no** | yes |

FFmpeg is on that list deliberately. ffmpeg 9.0.1 has both an IAMF demuxer and
muxer for the raw container, which makes it a genuinely useful third,
independent-lineage read oracle — and that is exactly why the trap is real. The
failure mode is a developer debugging a byte diff who opens
`libavformat/iamf_writer.c` "just to check". The GPL build flags some
distributions use do not change the analysis either way, because the GPL's reach
is over a combined work and there is no combined work here.

Attribution for the reference material this crate ports field layouts and
constants from is in [`NOTICE`](NOTICE). The AOM Patent License 1.0 required at
the repository root by §1.2.1(a) is in [`PATENTS`](PATENTS).
