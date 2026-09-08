# Vendored reference fixtures — manifest (D-14)

Every byte in `tests/fixtures/reference/` was copied out of one of the two
pinned reference trees. Parallax consumes this crate as a **path dependency**,
so each file here lands in every Parallax developer's clone; that is why there
is a cap, why the corpus is curated rather than complete, and why this file
exists at all. D-14's reversibility note is blunt about it: fixtures committed
to git history cannot be un-committed without a history rewrite.

## Sources and licence

| Repo | Tag | Commit | Licence |
|---|---|---|---|
| [`AOMediaCodec/libiamf`](https://github.com/AOMediaCodec/libiamf) | `v1.1.0` | `f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63` | BSD-3-Clause-Clear + AOM Patent License 1.0 |
| [`AOMediaCodec/iamf-tools`](https://github.com/AOMediaCodec/iamf-tools) | `v2.1.0` | `848c6ff4968ff8cc6f728259892ab4f90cb83256` | BSD-3-Clause-Clear + AOM Patent License 1.0 |

Attribution is in [`NOTICE`](../../NOTICE); the pins are in
[`REFERENCES.md`](../../REFERENCES.md) and are the same SHAs
`tools/build-reference.sh` checks out and `tests/reference_manifest.rs` asserts.
A copy taken from either project's development tip does not belong here — both
have drifted toward the draft-v2.0.0 tree, and a fixture from there is a file
the pinned decoder rejects, which is a divergence no passing test would reveal.

## `golden/` is not vendored — it is ours (D-20, GUARD-09)

`tests/fixtures/golden/` holds three artifacts **this crate generates**, not
copies of anything: `phase1_sample_identity.iamf` (7 073 bytes), its SHA-256,
and an annotated structural dump produced by `iamf::dump::dump_annotated`. They
are the committed golden every one of GUARD-09's four targets regenerates and
compares against, which makes an output change a reviewable PR diff rather than
a red job with no explanation.

Nothing in that directory is licensed by AOM and nothing there is conformance
evidence: a golden this crate generated is self-consistent with this crate by
construction. It is change detection. Regenerate with

```sh
IAMF_REGENERATE_GOLDEN=1 cargo test --test golden regenerate_golden_artifacts -- --nocapture
```

and then **read the diff** — that is the whole reason the dump is committed
alongside the bytes.

## The per-file size cap

**65_536 bytes** (64 KiB) per file.

It admits `test_000003.iamf` (32 567 bytes) with room to spare and excludes the
multi-minute vectors, whose median is 310 KB and whose largest is 14.9 MB.
Skipping the 154 files over the cap keeps 594 MB out of every clone.

The cap is enforced, not merely stated — `tests/fixtures_cap.rs` walks
`tests/fixtures/reference/` and fails on any file above it, so a future "just
this one big file" is a red test rather than a silent 6 MB.

## Layout

```
tests/fixtures/reference/
  *.iamf, *.textproto, *.wav      from libiamf@v1.1.0 tests/
  iamf-tools/                     from iamf-tools@v2.1.0 iamf/cli/testdata/
  negative/                       files that are INVALID by construction
```

The split by directory is provenance, and it also keeps the two proto dialects
apart: `libiamf@v1.1.0`'s textprotos are written in the **old** dialect and
still carry the fields v2.0.0 deprecated (`count_label`, `num_substreams`,
`num_layers`, `num_sub_mixes`, `num_audio_elements`, `num_layouts`,
`param_definition_size`). Do not feed them to `encoder_main`; use the
`iamf-tools/` copies.

## Vendored — 77 files, 39 of them `.iamf`, 1,203,066 bytes total

**`.iamf` files vendored: 39.** (`find tests/fixtures/reference -name '*.iamf' | wc -l`
must print exactly that.)

### 1. The `test_000003` quartet — CONF-08's target and the smoke-test pair

| Path (under `tests/fixtures/`) | Source repo | Bytes | Vendored | Reason |
|---|---|---:|:--:|---|
| `reference/test_000003.iamf` | `libiamf` | 32 567 | yes | CONF-08's target: the 120-byte descriptor prologue  67 OBUs  minimal leb128 throughout |
| `reference/test_000003.textproto` | `libiamf` | 3 910 | yes | the published configuration for the file above (retires D-17's CONF-08 waiver). OLD proto dialect - do NOT feed to encoder_main |
| `reference/sawtooth_100_stereo.wav` | `libiamf` | 32 044 | yes | the source PCM test_000003 was encoded from; the build script's smoke decode compares against it |
| `reference/test_000003_rendered_id_42_sub_mix_0_layout_0.wav` | `libiamf` | 32 044 | yes | the reference's own rendered output for mix presentation 42 - D-25's second  independent derivation |
| `reference/iamf-tools/test_000003.textproto` | `iamf-tools` | 3 967 | yes | the SAME configuration in the CURRENT proto dialect - this is the copy encoder_main accepts (Experiment 2's base) |

`test_000003.iamf` is stereo, 16 kHz, 16-bit little-endian, `PROFILE_VERSION_SIMPLE`,
`codec_config_id: 200`, `num_samples_per_frame: 128`, `audio_element_id: 300`,
`mix_presentation_id: 42`, `samples_to_trim_at_end: 64`. Its descriptor prologue
is **120 bytes** (not 118 — see `CONFORMANCE-GATE.md`) and it walks 67 OBUs with
its final boundary landing exactly on `len()`. It is **not** the D-18 Phase 1
fixture (5.1, 48 kHz, 24-bit big-endian); those are two different files.

Two of these four exist so D-25's "two independent derivations that must agree"
has both halves on disk: `sawtooth_100_stereo.wav` is what the file was encoded
*from*, and `test_000003_rendered_id_42_sub_mix_0_layout_0.wav` is what the
reference renders it *to*.

### 2. Every LPCM `.iamf` under the cap, with its configuration

Phase 1 is LPCM-only, so this is the working corpus. `codec_id == "ipcm"` and
nothing else in the file.

| Path (under `tests/fixtures/`) | Source repo | Bytes | Vendored | Reason |
|---|---|---:|:--:|---|
| `reference/test_000000_3.iamf` | `libiamf` | 32 309 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000000_3.textproto` | `libiamf` | 3 851 | yes | configuration for test_000000_3.iamf |
| `reference/test_000002.iamf` | `libiamf` | 33 197 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000002.textproto` | `libiamf` | 4 280 | yes | configuration for test_000002.iamf |
| `reference/test_000005.iamf` | `libiamf` | 33 494 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000005.textproto` | `libiamf` | 3 920 | yes | configuration for test_000005.iamf |
| `reference/test_000006.iamf` | `libiamf` | 33 744 | yes | LPCM under the cap; stereo  base profile |
| `reference/test_000006.textproto` | `libiamf` | 4 093 | yes | configuration for test_000006.iamf |
| `reference/test_000007.iamf` | `libiamf` | 33 494 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000007.textproto` | `libiamf` | 3 986 | yes | configuration for test_000007.iamf |
| `reference/test_000012.iamf` | `libiamf` | 33 496 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000012.textproto` | `libiamf` | 3 971 | yes | configuration for test_000012.iamf |
| `reference/test_000013.iamf` | `libiamf` | 33 496 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000013.textproto` | `libiamf` | 3 973 | yes | configuration for test_000013.iamf |
| `reference/test_000015.iamf` | `libiamf` | 33 500 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000015.textproto` | `libiamf` | 4 319 | yes | configuration for test_000015.iamf |
| `reference/test_000016.iamf` | `libiamf` | 33 549 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000016.textproto` | `libiamf` | 4 254 | yes | configuration for test_000016.iamf |
| `reference/test_000017.iamf` | `libiamf` | 33 496 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000017.textproto` | `libiamf` | 4 082 | yes | configuration for test_000017.iamf |
| `reference/test_000018.iamf` | `libiamf` | 33 619 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000018.textproto` | `libiamf` | 4 004 | yes | configuration for test_000018.iamf |
| `reference/test_000019.iamf` | `libiamf` | 33 494 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000019.textproto` | `libiamf` | 4 323 | yes | configuration for test_000019.iamf |
| `reference/test_000060.iamf` | `libiamf` | 33 539 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000060.textproto` | `libiamf` | 4 186 | yes | configuration for test_000060.iamf |
| `reference/test_000062.iamf` | `libiamf` | 33 501 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000062.textproto` | `libiamf` | 4 343 | yes | configuration for test_000062.iamf |
| `reference/test_000063.iamf` | `libiamf` | 33 501 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000063.textproto` | `libiamf` | 4 369 | yes | configuration for test_000063.iamf |
| `reference/test_000067.iamf` | `libiamf` | 33 499 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000067.textproto` | `libiamf` | 4 053 | yes | configuration for test_000067.iamf |
| `reference/test_000071.iamf` | `libiamf` | 33 058 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000071.textproto` | `libiamf` | 10 413 | yes | configuration for test_000071.iamf |
| `reference/test_000077.iamf` | `libiamf` | 33 509 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000077.textproto` | `libiamf` | 4 215 | yes | configuration for test_000077.iamf |
| `reference/test_000078.iamf` | `libiamf` | 32 575 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000078.textproto` | `libiamf` | 3 905 | yes | configuration for test_000078.iamf |
| `reference/test_000079.iamf` | `libiamf` | 32 575 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000079.textproto` | `libiamf` | 3 907 | yes | configuration for test_000079.iamf |
| `reference/test_000085.iamf` | `libiamf` | 33 494 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000085.textproto` | `libiamf` | 4 030 | yes | configuration for test_000085.iamf |
| `reference/test_000088.iamf` | `libiamf` | 33 026 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000088.textproto` | `libiamf` | 11 149 | yes | configuration for test_000088.iamf |
| `reference/test_000097.iamf` | `libiamf` | 33 279 | yes | LPCM under the cap; mono  simple profile |
| `reference/test_000097.textproto` | `libiamf` | 4 048 | yes | configuration for test_000097.iamf |
| `reference/test_000119.iamf` | `libiamf` | 225 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000119.textproto` | `libiamf` | 6 502 | yes | configuration for test_000119.iamf |
| `reference/test_000120.iamf` | `libiamf` | 204 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000120.textproto` | `libiamf` | 5 682 | yes | configuration for test_000120.iamf |
| `reference/test_000121.iamf` | `libiamf` | 32 503 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000121.textproto` | `libiamf` | 3 808 | yes | configuration for test_000121.iamf |
| `reference/test_000122.iamf` | `libiamf` | 207 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000122.textproto` | `libiamf` | 5 894 | yes | configuration for test_000122.iamf |
| `reference/test_000129.iamf` | `libiamf` | 221 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000129.textproto` | `libiamf` | 6 358 | yes | configuration for test_000129.iamf |
| `reference/test_000130.iamf` | `libiamf` | 204 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000130.textproto` | `libiamf` | 5 661 | yes | configuration for test_000130.iamf |
| `reference/test_000501.iamf` | `libiamf` | 32 522 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000501.textproto` | `libiamf` | 4 090 | yes | configuration for test_000501.iamf |
| `reference/test_000503.iamf` | `libiamf` | 33 500 | yes | LPCM under the cap; stereo  simple profile |
| `reference/test_000503.textproto` | `libiamf` | 4 139 | yes | configuration for test_000503.iamf |

### 3. Structurally interesting — paths Phase 1 writes

| Path (under `tests/fixtures/`) | Source repo | Bytes | Vendored | Reason |
|---|---|---:|:--:|---|
| `reference/test_000124.iamf` | `libiamf` | 13 383 | yes | TWO Audio Elements - the only multi-element file under the cap. Descriptor ordering is observable (CONF-04 fallback (b) material). Opus frames; Phase 1 reads descriptors only |
| `reference/test_000124.textproto` | `libiamf` | 5 719 | yes | configuration for test_000124.iamf |
| `reference/test_000059.iamf` | `libiamf` | 20 204 | yes | NON-STEREO loudspeaker_layout (5.1 plus a stereo layout in the mix presentation) - exercises the multi-layout descriptor path. Opus frames |
| `reference/test_000059.textproto` | `libiamf` | 22 269 | yes | configuration for test_000059.iamf |
| `reference/iamf-tools/test_000134.textproto` | `iamf-tools` | 4 378 | yes | the ONLY reference configuration setting GENERATE_LEB_FIXED_SIZE - 1 of 226. Textproto ONLY: no test_000134.iamf is committed in either tree  so the bytes must be produced by encoder_main in the pinned container. Named now for Phase 2's PARSE-04 |

`test_000134` deserves the extra sentence it gets in the table. It is the
**only** one of the 226 reference configurations that sets
`GENERATE_LEB_FIXED_SIZE`; the other 225 use the `kMinimum` default, and a
mechanical walk of all 226 committed `.iamf` files found 524 674 `obu_size`
fields, **every one of them minimal**. It is therefore the only non-minimal
leb128 material that exists anywhere in the reference corpus — and no
`test_000134.iamf` is committed in either tree, so its bytes have to be produced
by `encoder_main` in the pinned container. It is named here now, while the fact
is known, for Phase 2's PARSE-04.

### 4. `iamf-tools` fixtures — the four valid ones

| Path (under `tests/fixtures/`) | Source repo | Bytes | Vendored | Reason |
|---|---|---:|:--:|---|
| `reference/iamf-tools/noise_1024samp_5p1_opus.iamf` | `iamf-tools` | 2 228 | yes | valid iamf-tools-produced fixture: 5.1 Opus  num_samples_per_frame = 960 |
| `reference/iamf-tools/noise_1024samp_stereo_flac.iamf` | `iamf-tools` | 4 293 | yes | valid iamf-tools-produced fixture: stereo FLAC  num_samples_per_frame = 4608 |
| `reference/iamf-tools/noise_3s_stereo_opus.iamf` | `iamf-tools` | 2 319 | yes | valid iamf-tools-produced fixture: stereo Opus  num_samples_per_frame = 120 |
| `reference/iamf-tools/tones_100ms_3OA_stereo_opus.iamf` | `iamf-tools` | 4 715 | yes | valid iamf-tools-produced fixture: third-order ambisonics + stereo  num_samples_per_frame = 960 |

### 5. Negative fixtures — INVALID by construction, never goldens

| Path (under `tests/fixtures/`) | Source repo | Bytes | Vendored | Reason |
|---|---|---:|:--:|---|
| `reference/negative/tones_256samp_5p1_pcm.iamf` | `iamf-tools` | 3 188 | yes | INVALID: num_samples_per_frame = 0. NEGATIVE FIXTURE ONLY - never a golden |

`tones_256samp_5p1_pcm.iamf` carries **`num_samples_per_frame = 0`**, read
directly from its Codec Config payload
(`00 69 70 63 6d 00 00 00 01 10 00 00 bb 80` — `codec_config_id` 0, `codec_id`
`"ipcm"`, then the zero). The three reference implementations disagree about it,
which is exactly what makes it valuable:

- `iamf-tools`' own `ValidateNumSamplesPerFrame` requires `[1, 96000]`, so its
  parser would **reject its own fixture**.
- `libiamf`'s development tip rejects it.
- `libiamf@v1.1.0` — our pinned decoder — **accepts it and produces zero
  samples**, reporting `Get 0 frames` / `Get 0 samples` and exiting 0.

That last line is the whole point. It is the canonical "clean decode, wrong
result" case, and it is precisely what CONF-05's decoded-sample-count clause
exists to catch — the clause is not redundant with PCM equality, because a
zero-sample decode is trivially PCM-equal to nothing.

**This file must never be promoted to a golden.** Its Audio Element payload is
still sound evidence for 5.1 BCG channel-to-substream packing, and plan 01-06
uses it for that. Reading its structure is fine; asserting our output matches it
is not.

## Skipped candidates

187 of `libiamf@v1.1.0`'s 221 `.iamf` files are not vendored: 154 are over the
cap and 33 are under it but add nothing Phase 1 needs. Named samples, with sizes
and reasons:

| Path (under `tests/fixtures/`) | Source repo | Bytes | Vendored | Reason |
|---|---|---:|:--:|---|
| `test_000711.iamf` | `libiamf` | 14 942 170 | no | over the cap by 14 876 634 bytes; LPCM |
| `test_000710.iamf` | `libiamf` | 13 982 621 | no | over the cap by 13 917 085 bytes; LPCM |
| `test_000707.iamf` | `libiamf` | 13 500 190 | no | over the cap by 13 434 654 bytes; LPCM |
| `test_000700.iamf` | `libiamf` | 13 497 365 | no | over the cap by 13 431 829 bytes; LPCM |
| `test_000614.iamf` | `libiamf` | 13 497 309 | no | over the cap by 13 431 773 bytes; LPCM |
| `test_000708.iamf` | `libiamf` | 13 493 523 | no | over the cap by 13 427 987 bytes; LPCM |
| `test_000619.iamf` | `libiamf` | 13 016 312 | no | over the cap by 12 950 776 bytes; LPCM |
| `test_000621.iamf` | `libiamf` | 13 016 312 | no | over the cap by 12 950 776 bytes; LPCM |
| `test_000622.iamf` | `libiamf` | 13 016 312 | no | over the cap by 12 950 776 bytes; LPCM |
| `test_000623.iamf` | `libiamf` | 13 016 312 | no | over the cap by 12 950 776 bytes; LPCM |
| `test_000618.iamf` | `libiamf` | 13 015 085 | no | over the cap by 12 949 549 bytes; LPCM |
| `test_000620.iamf` | `libiamf` | 13 015 085 | no | over the cap by 12 949 549 bytes; LPCM |
| `test_000058.iamf` | `libiamf` | 65 924 | no | smallest file over the cap — 388 bytes over; LPCM |
| `test_000014.iamf` | `libiamf` | 32 912 | no | under the cap  but OPUS — Phase 1 is LPCM-only and no structural property here is unique |
| `test_000020.iamf` | `libiamf` | 6 977 | no | under the cap  but OPUS — Phase 1 is LPCM-only and no structural property here is unique |
| `test_000021.iamf` | `libiamf` | 6 808 | no | under the cap  but OPUS — Phase 1 is LPCM-only and no structural property here is unique |
| `test_000022.iamf` | `libiamf` | 6 977 | no | under the cap  but OPUS — Phase 1 is LPCM-only and no structural property here is unique |
| `test_000023.iamf` | `libiamf` | 7 641 | no | under the cap  but OPUS — Phase 1 is LPCM-only and no structural property here is unique |
| `test_000024.iamf` | `libiamf` | 7 006 | no | under the cap  but OPUS — Phase 1 is LPCM-only and no structural property here is unique |

103 of the over-cap files are LPCM — they are skipped on size
alone, not relevance, and any of them can be fetched on demand with the command
below when a specific one is wanted.

## Regenerating or fetching any file

Every file above is reproducible from a pinned SHA, byte-for-byte, with no build
step. Run `bash tools/build-reference.sh` once to populate `.reference/`, then:

```sh
# from libiamf@v1.1.0 -- any of the 221 .iamf, 221 .textproto or 403 .wav files
git -C .reference/libiamf show \
  f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63:tests/test_000711.iamf \
  > /tmp/test_000711.iamf

# from iamf-tools@v2.1.0
git -C .reference/iamf-tools show \
  848c6ff4968ff8cc6f728259892ab4f90cb83256:iamf/cli/testdata/test_000134.textproto \
  > /tmp/test_000134.textproto
```

`.reference/iamf-tools` is a sparse, blobless checkout; if it is absent:

```sh
git clone --filter=blob:none --no-checkout \
  https://github.com/AOMediaCodec/iamf-tools.git .reference/iamf-tools
git -C .reference/iamf-tools sparse-checkout set --cone iamf/cli/testdata iamf/cli docs
git -C .reference/iamf-tools checkout -q 848c6ff4968ff8cc6f728259892ab4f90cb83256
```

The `.github/workflows/reference.yml` job fetches on demand rather than reading
this directory, so a fixture nobody vendored is still testable in CI.

Producing a `.iamf` that no tree commits — `test_000134` is the only one this
phase wants — needs the digest-pinned container:

```sh
docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/
docker run --rm -v "$PWD:/work" iamf-tools:v2.1.0 \
  bazel-bin/iamf/cli/encoder_main \
    --user_metadata_filename=/work/tests/fixtures/reference/iamf-tools/test_000134.textproto \
    --output_iamf_directory=/work/target
```

## Scope change — CONF-11 no longer needs a Bazel run (research correction 6)

CONF-11 was written on the premise that
`iamf-tools/iamf/cli/testdata/` holds "338 `.textproto` files and **one**
`.iamf`", and that "M2's parse-`iamf-tools`-produced-files therefore needs a
one-time Bazel build" to generate and commit `tests/fixtures/*.iamf`.

**That premise is false at the pinned tags.** Counted directly:

| Source | `.textproto` | `.iamf` | rendered `.wav` |
|---|---:|---:|---:|
| `iamf-tools@v2.1.0 iamf/cli/testdata/` | 226 | 5 | 0 |
| `libiamf@v1.1.0 tests/` | 221 | **221** | 403 |

338 is the development tip's count, not v2.1.0's. 221 `iamf-tools`-produced
`.iamf` files, each with its own configuration textproto **and** its rendered
output WAV, are already committed to `libiamf@v1.1.0` under BSD-3-Clause-Clear.

**Fixture supply therefore needs no Bazel run at all.** It is a `git show` from
a pinned SHA. Every one of the 226 committed `.iamf` files was walked
OBU-by-OBU and every one lands its final boundary exactly on `len()`, so the
corpus is also a day-one regression suite for `find_obu_boundaries()` (OBU-08)
that runs with **no reference binary present** — which is CONF-10.

The digest-pinned container is still wanted, but for different work than CONF-11
described: **CONF-07** (producing a companion file for *our own* configuration)
and **CONF-06** (`iamf-tools`' parse path). Neither is fixture supply, and
neither is on this plan's critical path.

## Research open question 2 — resolved

> Should the 221 `libiamf@v1.1.0` `.iamf` fixtures be vendored into this repo?

**A curated subset is vendored; the remainder is fetched on demand.** The
deciding constraint is the one D-14 already names: Parallax consumes this crate
as a path dependency, so vendoring all 221 would put 623 MB into every Parallax
developer's clone to serve a phase that reads LPCM descriptors. The subset above
is 1.2 MB and covers CONF-08's target, the whole LPCM working set, a
two-Audio-Element file, a non-stereo layout, the only fixed-size-leb128
configuration, and the one file that decodes cleanly to the wrong answer.

The cost of being wrong is asymmetric and that is why the line is drawn here:
adding a file later is a commit, removing one is a history rewrite.
