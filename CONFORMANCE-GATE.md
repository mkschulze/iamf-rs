# Conformance gate — waiver ledger

Phase 1 exits through a seven-clause gate (CONF-02..CONF-08), not through
"`libiamf` returned OK". `libiamf` is a *permissive* reader: it ignores
reserved-bit misuse and clamps an overlong leb128 rather than erroring. Passing
it is necessary and nowhere near sufficient.

## Waiver policy (D-17)

A clause may be downgraded **only** by a committed entry in `## Waivers` below,
and only for a reason that is not a defect. Every entry must name:

1. **The clause** being downgraded.
2. **What was attempted** — the specific thing that was tried and did not work.
3. **Why it cannot be met** — the reason, stated so a reader can disagree with it.
4. **What weaker property is asserted instead** — there is always something, and
   an entry that asserts nothing is a deletion pretending to be a waiver.
5. **What would let it be restored** — the condition under which this entry gets
   removed.

The phase then exits with the waiver visible. This policy was written down while
nothing was at stake, precisely so that the first time a clause looks
unachievable the bar is already set.

A waiver is **not** the route around a failing test. It is the route around a
clause that cannot be evaluated. If the clause runs and reports a difference,
that is a defect or a diff-ledger entry, never a waiver.

## Facts established by research, 2026-09-08

These change the gate as it was originally written. They are recorded here
because the gate is where they bite.

### CONF-08's pre-authorised waiver is **retired**

D-17 pre-authorised a waiver for CONF-08 on the grounds that
`test_000003.iamf`'s "equivalent configuration is not published and must be
inferred from the file". **It is published.** `libiamf@v1.1.0` ships 221 `.iamf`
files and 221 matching `.textproto` files in `tests/`, and
`tests/test_000003.textproto` gives the exact configuration —
`PROFILE_VERSION_SIMPLE`, `codec_config_id: 200`, `CODEC_ID_LPCM`,
`num_samples_per_frame: 128`, `audio_roll_distance: 0`,
`LPCM_LITTLE_ENDIAN`, 16-bit / 16 kHz, `audio_element_id: 300`, one substream,
stereo, `mix_presentation_id: 42`, `samples_to_trim_at_end: 64`, temporal
delimiters disabled. The same configuration in the current proto dialect is
`iamf-tools@v2.1.0 iamf/cli/testdata/test_000003.textproto`.

CONF-08 is therefore a mechanical reproduction from a written specification, not
archaeology, and **no waiver is available for it**. Anyone opening one must
first explain why the published textproto is insufficient.

Note that `test_000003.iamf` is stereo, 16 kHz, 16-bit little-endian — it is
**not** the D-18 Phase 1 fixture (5.1, 48 kHz, 24-bit; see Experiment 3 for why
its endianness is little and not big). CONF-08 and CONF-02..05 are two different
files. Do not conflate them.

### The descriptor prologue is **120 bytes, not 118**

PROJECT.md describes `test_000003.iamf` as having "a 118-byte descriptor
prologue". The first Audio Frame OBU begins at offset `0x78` = **120**.
Descriptor OBUs sit at offsets 0, 8, 26 and 40. The file walks **67 OBUs** and
its final OBU boundary lands on `bytes.len()` exactly.

Use 120. A harness written against 118 will fail in a way that looks like an
encoder bug.

### Only CONF-06 and CONF-07 retain a defensible waiver path

Both depend on running `iamf-tools` at its pinned tag, which requires Bazel,
abseil, protobuf and fdk-aac and therefore runs only in a digest-pinned
container. If that container cannot be built or run, those two clauses cannot be
evaluated — which is what a waiver is for. DEC-04 has now removed most of the
risk by identifying the tag (`v2.1.0` =
`848c6ff4968ff8cc6f728259892ab4f90cb83256`; see `REFERENCES.md`).

Related, and not a waiver: `iamf-tools@v2.1.0` ships no `probe_main`. Its
`iamf/cli/BUILD` declares exactly two `cc_binary` targets, `decoder_main` and
`encoder_main`. CONF-06 ("`iamf-tools`' own parser accepts the file") is
therefore enforced *through* `decoder_main`'s parse path, which goes via
`ObuProcessor`/`DescriptorObuParser` and so exercises the strict parser.

**The CONF-06 signal is `Decoded <N> temporal units.` on stderr, N matched against
the expected temporal-unit count.** Established by Experiment 1, executed
2026-09-08.

An earlier revision of this section asserted "a non-zero exit or an error on
stderr is the CONF-06 signal". That was an inference, and it is wrong twice over.
Experiment A refuted the general form: `libiamf`'s `iamfdec` returns **exit 0 on
every one of five inputs**, including two that decode to zero samples. Experiment 1
then showed `decoder_main` is no better — of the same five inputs it returns **0 on
three**, one of which is a file truncated mid-OBU that yields an 80-byte WAV and
**zero** decoded temporal units.

That truncated case is why `test -s <output>.wav` is also insufficient: 80 bytes is
a 44-byte WAV header plus 36 bytes of nothing, and `test -s` passes it. The
temporal-unit count is the only one of the three recorded observations — exit code,
stderr, output file — that separates a truncated parse from a successful one.
`.github/workflows/reference.yml` asserts on it.

## Recorded experiments

Experiments run against the gate that are worth keeping even though they are not
waivers — a byte diff explained, a decode compared, a configuration ruled out.
Plans 01-02 and 01-08 append here.

Each entry states whether it was **EXECUTED** or **NOT RUN**. An entry that was
not run still belongs here, with its exact commands and the reason it could not
execute, because the question it leaves open is the thing a later reader needs
to know about. What must never appear is an entry that reads as though it ran.

---

### Experiment A — how `iamfdec` signals failure (EXECUTED, 2026-09-08)

Not one of the two experiments plan 01-02 was asked for. It is the one that
could be run on the authoring machine, and it is what makes the other two's
framing correct: it establishes by execution, in this repository, that a
reference tool can fail and still return 0, so no harness in this project may
assert on an exit code without having tested that it means something.

**Setup.** `tools/experiments/corrupt-fixture.py` produces the control plus four
corruptions of `tests/fixtures/reference/test_000003.iamf` (descriptor OBUs at
offsets 0, 8, 26, 40; first Audio Frame at 120):

```sh
python3 tools/experiments/corrupt-fixture.py \
  tests/fixtures/reference/test_000003.iamf /tmp/corrupt
for f in valid bitflip_reserved bitflip_sample_rate obusize truncated; do
  "$IAMF_REF_DECODER" -i0 -o3 "/tmp/corrupt/$f.wav" \
    -r 16000 -s0 -d 16 -disable_limiter "/tmp/corrupt/$f.iamf"
done
```

**Observed** (`libiamf@v1.1.0` `iamfdec`, macOS x86_64, exact output):

| Input | Corruption | exit | WAV bytes | WAV sha256 (12) | reported |
|---|---|---:|---:|---|---|
| `valid.iamf` | none (control) | **0** | 32 044 | `ff8c67981380` | `Get 63 frames` / `Get 8000 samples` |
| `bitflip_reserved.iamf` | byte 30 bit 0 — a **reserved** bit of the Audio Element type octet | **0** | 32 044 | `ff8c67981380` | `Get 63 frames` / `Get 8000 samples` |
| `bitflip_sample_rate.iamf` | byte 23 bit 0 — Codec Config `sample_rate` 16000 → 81536 | **0** | 6 324 | `07e189ed9ae4` | `Get 63 frames` / `Get 1570 samples` |
| `obusize.iamf` | byte 1 — IA Sequence Header `obu_size` 6 → 127 | **0** | 44 | `79bfb4abe83f` | `Get 0 frames` / `Get 0 samples` |
| `truncated.iamf` | cut 40 bytes into the first Audio Frame OBU | **0** | 44 | `79bfb4abe83f` | `Get 0 frames` / `Get 0 samples` |

**Three conclusions, each load-bearing.**

1. **The exit code is never the signal.** All five runs returned 0, including the
   two that produced nothing but a 44-byte WAV header. The observables are: the
   output file exists and exceeds 44 bytes; the frame count; the decoded-sample
   count. `tools/build-reference.sh` asserts exactly those three and says so.

2. **`libiamf` ignores reserved-bit misuse — reproduced, not cited.** Flipping a
   reserved bit produced a **byte-identical** WAV: same size, same sha256, same
   frame and sample counts. This is the concrete form of "passing the `libiamf`
   gate is necessary and nowhere near sufficient". A reserved-bit bug in our
   encoder would sail through CONF-05 without a murmur, which is why CONF-07's
   byte diff against `iamf-tools` output is the clause that actually catches it.

3. **"Clean decode, wrong result" is the failure class to design against.** The
   `sample_rate` flip changed one bit in a descriptor and yielded a successful
   decode of the wrong length — 1570 samples instead of 8000, no error, exit 0.
   Together with `tones_256samp_5p1_pcm.iamf` (zero samples, exit 0; see
   `tests/fixtures/MANIFEST.md`) this is why CONF-05's decoded-sample-count
   assertion is **not** redundant with PCM equality: a wrong-length decode is
   never compared sample-for-sample at all unless the count is checked first.

---

### Experiment 1 — how does `decoder_main` signal a parse failure? (EXECUTED 2026-09-08)

Research open question 1, and the thing CONF-06's value rests on. It could not run
when this plan was written — the Docker daemon was down on the authoring machine —
and ran unchanged once the daemon was started. The image built in 275.5 s
(1 273 Bazel actions, build-time assertion passed), which also closed research
assumption A2.

**Results.** Five inputs from `tools/experiments/corrupt-fixture.py`, three
observations recorded separately per case as this section requires:

| case | exit | WAV | sha256 (first 16) | stderr |
|---|---|---|---|---|
| `valid` (control) | 0 | 64 080 B | `01edcf2f45132977` | `Decoded 63 temporal units.` |
| `bitflip_reserved` | 0 | 64 080 B | `01edcf2f45132977` — **identical to control** | `Decoded 63 temporal units.` |
| `bitflip_sample_rate` | 139 | absent | — | `*** Check failure stack trace: ***` |
| `obusize` | 139 | absent | — | `*** Check failure stack trace: ***` |
| `truncated` | **0** | **80 B** | `42e6693d278a1e39` | **`Decoded 0 temporal units.`** |

**The conclusion this experiment owed the project — the name of the observable:**

> CONF-06 asserts that stderr carries `Decoded <N> temporal units.` with N equal to
> the expected temporal-unit count. Not the exit code. Not `test -s`.

Three of five corrupted inputs return exit 0, so an exit-code gate passes on three
failures. The truncated file additionally defeats `test -s`: 80 bytes is a WAV
header plus 36 bytes, written while decoding nothing at all.

**Two secondary findings, both worth keeping.**

1. **The reserved-bit flip is invisible to `decoder_main` as well** — byte-identical
   WAV, same sha256 as the control. Reserved-bit misuse is therefore invisible to
   *both* oracles. This is not a weakness in CONF-06; it is the reason CONF-07's
   byte-diff exists, now demonstrated rather than assumed. It does contradict
   PROJECT.md's claim that "`iamf-tools` errors on reserved-bit misuse" — for this
   field, at v2.1.0, it does not.
2. **The two malformed-header cases die on an absl `CHECK` abort**, not a graceful
   rejection. Loud and usable as a signal, but `decoder_main` has no clean error
   path for a bad `sample_rate` or a wrong `obu_size`. `reference.yml` greps for
   `Check failure stack trace` explicitly, because an abort produces no
   temporal-unit line to match against.

**The exact procedure, as run:**

```sh
docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/
python3 tools/experiments/corrupt-fixture.py \
  tests/fixtures/reference/test_000003.iamf /tmp/corrupt

for f in valid bitflip_reserved bitflip_sample_rate obusize truncated; do
  rm -f "/tmp/corrupt/$f.decoder_main.wav"
  docker run --rm -v /tmp/corrupt:/work -w /src iamf-tools:v2.1.0 \
    bazel-bin/iamf/cli/decoder_main \
      --input_filename="/work/$f.iamf" \
      --output_filename="/work/$f.decoder_main.wav"
  echo "$f: exit=$?  out=$(ls -l "/tmp/corrupt/$f.decoder_main.wav" 2>&1)"
done
```

The `echo` above is a convenience; the results table was produced by capturing
stdout and stderr separately per case, because the exit code, the verbatim stderr
and the output file's size are three independent observations and conflating them
is how the wrong one gets chosen as the signal.

Note for anyone re-running this: on macOS `docker build` may first fail with
`exec: "docker-credential-desktop": executable file not found in $PATH`. That is a
`credsStore` lookup, not a build failure — the image is public. Prepend
`/Applications/Docker.app/Contents/Resources/bin` to `PATH`.

**The conclusion this experiment owed the project has been delivered:** the
observable is `Decoded <N> temporal units.`, and
`.github/workflows/reference.yml`'s CONF-06 step now asserts on it — including an
explicit grep for `Check failure stack trace`, since an absl abort produces no
temporal-unit line at all.

**Recorded alongside, per research correction 8:** CONF-06 goes through
`decoder_main` because `iamf-tools@v2.1.0` ships **no `probe_main`** and no other
dedicated validator. `iamf/cli/BUILD` at that SHA declares exactly two
`cc_binary` targets, `decoder_main` and `encoder_main`. STACK.md §8's claim that
the project "Produces `encoder_main`, `decoder_main`, `probe_main` CLIs" is a
development-tip fact, not a v2.1.0 one. `decoder_main` is the right route anyway
— it runs `ObuProcessor` / `DescriptorObuParser`, which is the strict parser —
but it is a route chosen because the alternative does not exist, and that should
read as a deliberate decision rather than as a missing validator.

---

### Experiment 2 — does `encoder_main` accept an unreferenced Codec Config? (EXECUTED 2026-09-08)

Research assumption A3, and D-18's stated research item for this plan. D-18's
Phase 1 fixture is **two Codec Configs and one 5.1 Audio Element**, so the
second Codec Config is referenced by nothing. The parser side is verified legal
from source; the encoder side was not, and that is what this settled.

**It settled more than it was asked.** The question was framed as "does the
encoder refuse?", and the pre-recorded fallbacks below assume that failure mode.
The encoder does **not** refuse. The break is one layer further down, in the
binary CONF-06 must actually run through.

| tool | verdict on the two-Codec-Config file |
|---|---|
| `iamf-tools` `encoder_main` | **accepts** — exit 0, `Success. Test case expected to pass.`, `status= OK`; the sequencer log shows two `after Codec Config` lines (bit_offset 208, then 352), so both are written |
| `libiamf` `iamfdec` (the Core Value oracle) | **accepts** — 8 000 samples, WAV **byte-identical** to the golden's decode (sha256 `ff8c679813805b3b…`, `cmp -l` reports 0 differing bytes) |
| `iamf-tools` `decoder_main` | **aborts** — `*** Check failure stack trace: ***`, no output file |

Output was 32 585 bytes against the golden's 32 567: exactly the 18-byte second
Codec Config OBU inserted (`00 10 c901 6970636d 8001 0000 0110 0000 3e80` — type 0,
size 16, id 201, `ipcm`), everything else byte-identical.

**Causality isolated, not inferred.** The same textproto with the second
`codec_config_metadata` block deleted, through the same encoder and the same
decoder:

- one Codec Config → `Decoded 63 temporal units.`, 64 080-byte WAV
- two Codec Configs → `*** Check failure stack trace: ***`, no output

Nothing else differs. The unreferenced Codec Config is the cause.

**Interpretation.** The file is conformant by every available measure except one.
`DescriptorObuParser` permits unreferenced Codec Configs by source,
`WriteDescriptorObus` writes them, `encoder_main` produces them, and the decoder
whose acceptance *is* the Core Value reads them byte-identically. Only
`decoder_main` aborts, on a hard `CHECK` rather than a diagnostic — which reads as
an upstream robustness bug rather than a spec violation. That interpretation does
not rescue the fixture: CONF-06 must run through `decoder_main` (research
correction 8 — v2.1.0 ships no `probe_main`), so the clause cannot pass regardless
of who is at fault.

**The exact procedure, as run.** The input is already prepared and
committed — `tools/experiments/two-codec-configs.textproto` is
`iamf-tools@v2.1.0 iamf/cli/testdata/test_000003.textproto` (the **current**
proto dialect, not `libiamf@v1.1.0`'s deprecated-field copy) with one added
`codec_config_metadata` block carrying `codec_config_id: 201` and **identical**
`sample_rate`, `sample_size`, `sample_format_flags` and `num_samples_per_frame`.
No `audio_element_metadata` references 201.

```sh
docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/
mkdir -p target/experiment2
docker run --rm -v "$PWD:/work" -w /src iamf-tools:v2.1.0 \
  bazel-bin/iamf/cli/encoder_main \
    --user_metadata_filename=/work/tools/experiments/two-codec-configs.textproto \
    --output_iamf_directory=/work/target/experiment2
ls -l target/experiment2
```

Every field but the ID is held identical **because it has to be**, not for
tidiness. Two Codec Configs differing in sample rate or bit depth are fatal
(`obu_sequencer_base.cc`: *"Codec Config OBUs with different bit-depths and/or
sample rates are not in base-enhanced/base/simple profile; they are not allowed
in ISOBMFF."*), and a differing `num_samples_per_frame` is fatal too
(`cli_util.cc`: *"The encoder does not support Codec Config OBUs with a
different number of samples per frame yet."*). LPCM has no remaining free field.

**What is already known from source, and what is not:**

- **Parser side — legal.** `DescriptorObuParser` inserts each Codec Config into
  a map keyed by `codec_config_id` and Audio Elements look their ID up; nothing
  requires every entry be referenced. `WriteDescriptorObus` writes every entry
  unconditionally.
- **Precedent — none.** Across all 226 reference `.iamf` files, 77 have more
  than one Codec Config or more than one Audio Element and **zero** have an
  unreferenced Codec Config. The single file with two Codec Configs,
  `test_000119`, injects its second one as an `ArbitraryObu` — raw hex in the
  textproto with an imaginary `codec_id: "fake"` — referenced by an equally
  arbitrary second Audio Element.
- **Encoder side — unknown.** That is this experiment.

**The fallback was pre-chosen in this order, and must be recorded here when
adopted:**

- **(a)** Inject the second Codec Config via `arbitrary_obu_metadata` with
  `INSERTION_HOOK_AFTER_CODEC_CONFIGS` — the mechanism `test_000119` uses, so it
  is proven to work. **Caveat that must be written into the ledger if this is
  taken:** that hook forces the arbitrary OBU *after* all normal ones, so
  ordering becomes positional rather than ID-sorted, and DESC-08's ascending-ID
  property is then no longer what is being observed. The clause would still
  pass; it would just be measuring something else, which is worse than failing.
- **(b)** Satisfy CONF-04 with **two Audio Elements** on a second,
  structure-only fixture — assert OBU count, ordering and the boundary walk
  there, and keep sample-identity on the single-element 5.1 file. 77 reference
  files have ≥ 2 Audio Elements so the route is well-precedented. Note that two
  elements push the minimum profile from Simple to **Base**.

#### ADOPTED: fallback (b) — user decision, 2026-09-08

D-18 is amended. The Phase 1 fixture becomes **two fixtures**:

| fixture | shape | what it carries |
|---|---|---|
| sample-identity | one 5.1 Audio Element, one Codec Config | CONF-02, CONF-03, CONF-05 — the decoder's output IS our input, so D-19's "no permutation constant anywhere" premise is untouched |
| structure-only | two Audio Elements | CONF-04 — OBU count, descriptor ordering, and the `find_obu_boundaries` walk landing exactly on `bytes.len()` |

**Why (b) and not (a).** (a) was ordered first, but its own recorded caveat is
disqualifying: forcing the OBU after all normal ones makes ordering positional,
so DESC-08's ascending-ID property stops being the thing under test. A clause that
passes while measuring something other than what it claims is worse than a clause
that fails, because nothing downstream ever learns the difference.

**Accepted cost.** Two Audio Elements push the minimum profile from Simple to
**Base**. PROF-02 (minimum-profile selection) has to exercise that path anyway, so
this is added coverage rather than lost coverage — the fixture now proves the
selector picks Base when Base is what the configuration fits.

**Consequence for plan 01-08.** It inherits two fixtures, not one, and the
seven-clause gate is split across them. The coupling that made "one fixture, seven
clauses" valuable is genuinely weakened; what preserves the gate's meaning is that
the *sample-identity* half — the half `libiamf`'s acceptance defines — stays whole
and un-permuted.

### Experiment B — research assumption A4, re-verified at the pinned tag (EXECUTED, 2026-09-08)

`PITFALLS.md` §3 claimed that within one temporal unit `iamf-tools` requires every
audio frame to carry identical `num_samples_to_trim_at_start`, identical
`num_samples_to_trim_at_end` and identical timestamps, and that substream IDs must
be unique within the unit. `01-RESEARCH.md` recorded that as **`[CITED]`, not
`[VERIFIED]`** — it was read from the reference's development tip and not re-opened
at `v2.1.0`. Being stricter than the pinned reference is its own defect class, so
plan 01-06 re-opened the file before enforcing anything.

**Command.** The pinned tree lives at `/src` inside the digest-pinned container
(`REFERENCES.md` § "The `iamf-tools` container"):

```sh
docker run --rm --entrypoint /bin/sh iamf-tools:v2.1.0 \
  -c 'cd /src && git rev-parse HEAD && cat -n iamf/cli/temporal_unit_view.cc'
```

`git rev-parse HEAD` echoed `848c6ff4968ff8cc6f728259892ab4f90cb83256` — the SHA
`REFERENCES.md` pins for tag `v2.1.0`, so the lines below are from the pinned tree
and not from `main`.

**Result: A4 is CONFIRMED at `v2.1.0`.** `ValidateAllAudioFramesMatchStatistics`,
`iamf/cli/temporal_unit_view.cc:128-164`, enforces all four clauses:

| Clause | File and line | Enforcement |
|---|---|---|
| substream IDs unique within the unit | `temporal_unit_view.cc:131-140` | `InvalidArgumentError("A temporal unit must not have multiple audio with the same substream ID.")` |
| identical `num_samples_to_trim_at_end` | `temporal_unit_view.cc:147-150` | `ValidateEqual(..., "`num_samples_to_trim_at_end` must be the same for all audio frames")` |
| identical `num_samples_to_trim_at_start` | `temporal_unit_view.cc:151-155` | `ValidateEqual(..., "`num_samples_to_trim_at_start` must be the same for all audio frames")` |
| identical start and end timestamps | `temporal_unit_view.cc:156-161` | `ValidateEqual(..., "must be the same for all audio frames")` |

One clause research did **not** record, found in the same read and worth having:
`ComputeTemporalUnitStatisticsFromAudioFrame`, `temporal_unit_view.cc:71-79`, requires
`num_samples_to_trim_at_start + num_samples_to_trim_at_end <= num_samples_per_frame`
("cumulative trim is <= `num_samples_per_frame`"), explicitly to prevent an underflow
in the subtraction that follows.

**Enforced.** `iamf::obu::validate_temporal_unit` checks the trim-equality and
substream-uniqueness clauses, and `plan_frames` produces trim values that satisfy the
cumulative-trim bound by construction (`0 <= trim_at_end < num_samples_per_frame`,
`trim_at_start = 0` for LPCM). The timestamp clauses are not enforced here because
timestamps are not a wire field of the Audio Frame OBU — they are `iamf-tools`'
internal bookkeeping — and this crate has no temporal-unit timeline until Phase 2's
sequence-level parser.

Assumption A4's status in `01-RESEARCH.md` moves from `[CITED]` to
`[VERIFIED-FROM-CODE: iamf-tools@v2.1.0 iamf/cli/temporal_unit_view.cc:128-164]`.

### Experiment 3 — research assumption A1: does a 24-bit round trip survive the reference exactly? (EXECUTED, 2026-09-08)

Research's highest-risk `[ASSUMED]` claim, and plan 01-08's **gating** first task:
the empirical proof that `iamf_decoder_plane2stride_out`'s float→int conversion is
exact was run at **16 bit**, and D-18 chose 24-bit **big-endian** specifically to
cover DESC-03's endianness sense. It ran before the fixture was frozen, because the
golden hash, the annotated dump and the diff ledger are all derived from the fixture.

**A1 is REFUTED — and not where it was expected.** The output conversion is exact.
The *input* read is not.

```c
/* libiamf@v1.1.0 code/src/iamf_dec/bitstream.c:202-210 */
uint32_t readu24be(uint8_t *data, int offset) {
  return readu16be(data, offset) << 8 | data[offset + 2];   /* correct */
}

int reads24be(uint8_t *data, int offset) {
  uint32_t ret = readu16le(data, offset) << 8 | data[offset + 2];
  /*             ^^^^^^^^^ readu16be was meant */
  int iret = ret << 8;
  return (iret >> 8);
}
```

`readu16le(data, offset) << 8 | data[offset + 2]` composes `b0<<8 | b1<<16 | b2`
where big-endian means `b0<<16 | b1<<8 | b2`, so **the top two bytes of every
24-bit big-endian sample are transposed**. `IAMF_pcm_decoder.c:57-61` selects
`reads24be` for exactly `sample_size == 24 && !flags`, so nothing else is affected:
`reads16be` uses `readu16be`, `reads24le`/`reads16le`/`reads32le` use `readu16le`,
and `reads32be` uses `readu32be`. All of those are correct.
`[VERIFIED-FROM-CODE: libiamf@v1.1.0 code/src/iamf_dec/bitstream.c:184-228;
code/src/iamf_dec/pcm/IAMF_pcm_decoder.c:52-67]`

**Three probes, all executed, all now committed as tests in `tests/conformance.rs`.**
Each encodes a stereo 48 kHz fixture of 300 sample frames — a deliberate
non-multiple of `num_samples_per_frame` 128, so the final frame carries
`trim_at_end = 84` — through the real `SequenceWriter`, with a per-channel ramp
capped at half full scale (−6 dBFS), and decodes it twice.

| probe | sample format | decoded frames | differing samples | limiter delta |
|---|---|---:|---:|---:|
| `probe_24bit_little_endian_round_trips_exactly` | 24-bit LE | 300 of 300 | **0 of 600** | 0 |
| `probe_16bit_big_endian_round_trips_exactly` | 16-bit BE | 300 of 300 | **0 of 600** | 0 |
| `probe_24bit_big_endian_hits_the_upstream_reads24be_defect` | 24-bit BE | 300 of 300 | **597 of 600** | 600 |

Verbatim, from `cargo test --test conformance -- --nocapture --test-threads=1` with
`IAMF_REF_DECODER` pointing at the pinned `iamfdec`:

```
A1 RESULT (16-bit BE): 300 frames in, 300 out, 0 of 600 samples differ, limiter delta 0
A1 RESULT (24-bit BE): 300 frames in, 300 out, 597 of 600 samples differ, limiter delta 600
probe24be limiter delta 600 (expected non-zero: the misread values exceed -1 dBTP)
A1 RESULT (24-bit LE): 300 frames in, 300 out, 0 of 600 samples differ, limiter delta 0
```

The exact command each probe runs, as printed:

```sh
"$IAMF_REF_DECODER" -i0 -o3 <out>.nolimiter.wav -r 48000 -s0 -d <16|24> \
  -disable_limiter <in>.iamf
"$IAMF_REF_DECODER" -i0 -o3 <out>.limiter.wav    -r 48000 -s0 -d <16|24> <in>.iamf
```

**Four things this establishes, each load-bearing.**

1. **The 24-bit depth itself is exact.** 24-bit little-endian round-trips with zero
   differing samples, so `scale_i2f = 1 << 23`, `FLOAT2INT24`'s
   `lrintf(x * 8388608.f)` and the three-byte little-endian write in
   `iamf_decoder_plane2stride_out` are all fine. A1's *stated* risk — float→int
   rounding — was not the problem.
2. **Big-endian is exact at 16 bits.** So `sample_format_flags == 0` is evaluable by
   the reference; only its 24-bit read is not.
3. **The failure is diagnosed, not merely observed.** All 600 decoded samples of the
   24-bit big-endian run equal what the defective `reads24be` produces from the bytes
   we wrote — asserted in `probe_24bit_big_endian_hits_the_upstream_reads24be_defect`.
   That proves our encoder wrote correct big-endian bytes and the reference misread
   them, which is the opposite conclusion from "our 24-bit big-endian writer is
   broken", and it is the conclusion a bare "the PCM differs" would not have
   supported. Only 597 of 600 *differ* because a sample whose top two bytes are equal
   is invariant under a transposition — which is also why a quiet or slowly-varying
   fixture would have hidden this entirely. (An earlier run of this same probe read
   595 rather than 597: the signal generator's step was widened after
   `the_ramp_spans_its_range` found the original step too small to cross zero in 300
   frames, so more samples now have differing top bytes. The count is re-measured, not
   re-narrated.)
4. **The limiter delta of 600 is a consequence, not a fixture defect.** The input
   peaks at −6 dBFS; the *misread* values do not, so the limiter engages on samples
   that are already wrong. `assert_limiter_is_transparent` is therefore asserted by
   the callers that expect exactness and deliberately not inside the shared
   round-trip helper — otherwise this case would have reported "the fixture is too
   loud", sending a reader to lower the amplitude instead of finding the misread.

**Consequence for the fixture design — decided here, before the freeze.**

| fixture | shape | why |
|---|---|---|
| sample-identity | one 5.1 Audio Element, one Codec Config, 48 kHz, **24-bit little-endian** | CONF-02/03/05. Keeps the whole of D-18 except the endianness sense: 5.1 BCG packing, three-byte samples, non-multiple length, `trim_at_end > 0` with `trim_at_start = 0` |
| endianness | one stereo Audio Element, 48 kHz, **16-bit big-endian** | DESC-03's `sample_format_flags == 0` sample identity, kept rather than lost, at the depth the reference can evaluate |
| structure-only | two Audio Elements | CONF-04, per Experiment 2's adopted fallback (b) — unchanged by this experiment |

**What is retained and what is lost, stated plainly.** DESC-03's endianness *sense*
is retained: at 16 bits through the reference, and at 24 bits by hand-computed byte
vectors (`store_sample_writes_the_declared_byte_order` asserts that `0x123456` stores
as `12 34 56` big-endian and `56 34 12` little-endian, and that `-1` stores as
`FF FF FF`) — evidence that involves no permissive reader at all and is therefore
stronger than the round trip it replaces. What is lost is exactly one thing: the
*combination* of a three-byte sample and big-endian order, verified end to end
through `libiamf`. That is the waiver below.

**What was NOT done.** The 24-bit big-endian combination was not silently dropped,
no tolerance window was introduced, and nothing in the harness compensates for the
transposition. The defect probe asserts that the misread is *complete and exactly
explained*; it never applies the transposition to make a comparison pass.


## Phase 1 exit

The phase exits through this table, not through "`libiamf` returned OK". Every
verdict below was produced by `cargo test --test conformance -- --nocapture` on
macOS x86_64 on 2026-09-08, with `IAMF_REF_DECODER` pointing at the pinned
`iamfdec` and the digest-pinned `iamf-tools:v2.1.0` image available locally. The
per-clause lines the harness prints are the same strings quoted here.

| clause | what it asserts | verdict | evidence |
|---|---|---|---|
| **clause 0** (D-13) | the reference on disk is the one `REFERENCES.md` pins, asserted **before** any other clause | **PASS** | `clause 0 (D-13 manifest): reference pin confirmed`, both SHAs matched |
| **CONF-02** | the signal is non-silent and every channel differs from every other at every index | **PASS** | `6 channels, each distinguishable at every index, peak <= -6 dBFS` |
| **CONF-03** | the length forces `0 < trim_at_end < num_samples_per_frame` with `trim_at_start = 0`, computed in checked integers | **PASS** | `trim_at_end = 84, trim_at_start = 0, and the two differ` |
| **CONF-04** | structure observable in our own output: OBU count, descriptor write order, boundary walk landing on `len()` | **PASS** | sample-identity: `16 OBUs … final boundary lands exactly on len() = 7073`; structure-only: `11 OBUs, 1 Codec Config(s), 2 Audio Element(s)` |
| **CONF-05** | `libiamf` decodes it, the sample count matches, the PCM is **identical** | **PASS** | `300 sample frames, 0 of 1800 samples differ, limiter delta 0` — the 5.1 fixture, so BCG packing is proven correct as well |
| **CONF-06** | `iamf-tools`' stricter parser accepts it | **PASS** | `decoder_main reported "Decoded 3 temporal units."` on all three fixtures |
| **CONF-07** | the byte diff against `encoder_main` output equals `DIFF-LEDGER.md` exactly, both directions | **PASS, and the diff is EMPTY** | `ours 7073 bytes, theirs 7073 bytes, 0 differing offset(s), ledger 0 row(s)` |
| **CONF-08** | plan 01-07's whole-file reproduction of `test_000003.iamf` still holds | **PASS** | 32567 bytes, 68 boundaries, first Audio Frame at 120; the reproduction itself is `tests/sequence.rs` |
| **CONF-09** | reference tools invoked by `Command`, discovered by env var or pinned image, never by a build script | **PASS** | asserted at source level: no `build.rs`, no `[build-dependencies]`, no `bindgen`/`cmake`/`cc`/`pkg-config` |
| **CONF-10** | green offline with no reference binary and no container | **PASS** | `env -u IAMF_REF_DECODER cargo test --locked` green; every reference-gated clause prints a skip reason |

### The result CONF-07 actually produced

**Our output is byte-identical to `iamf-tools@v2.1.0`'s `encoder_main` output**
for the same configuration and the same PCM — 7073 bytes each, zero differing
offsets, SHA-256 `3e53f10babd78b721d524c0e41fbb0806a5ad37d2f5b4dd247a4336e0be1282c`.

That is the real exit criterion PROJECT.md names — "either identical or fully
explained in writing" — met by identity rather than by explanation, and it is a
materially stronger result than passing `libiamf`, which Experiment A showed
accepts a file with a flipped reserved bit without a murmur.

It was checked for the obvious false pass. `run_encoder_main` refuses to run if
the companion path already exists, and requires `encoder_main`'s per-layout
rendered WAVs to be present afterwards, so a stale or planted `.iamf` cannot pass
as a fresh reference encode. The scratch directory holds
`phase1_sample_identity_rendered_id_42_sub_mix_0_layout_0.wav` and `…layout_1.wav`
— two layouts, which is the 5.1 fixture's Sound System B and its mandatory
Sound System A.

### Waivers in force at exit

**One: W-1**, below — CONF-05 for the 24-bit **big-endian** sample format
specifically, because `libiamf@v1.1.0`'s `reads24be` misreads it. CONF-05 itself
is not waived; it passes on the 24-bit little-endian sample-identity fixture and
again on the 16-bit big-endian endianness fixture. The waiver names all five
D-17 fields and its restoration condition is **executable**: the day the round
trip becomes exact, `probe_24bit_big_endian_hits_the_upstream_reads24be_defect`
fails and points back at the entry.

**No other clause carries a waiver.** Note in particular that CONF-08's
pre-authorised waiver was retired by research — the configuration is published —
and that DEC-04's settlement of the `iamf-tools` pin removed most of the
CONF-06/07 risk, both of which are now demonstrated rather than merely expected.


## Cross-target byte-identity evidence (GUARD-09 / ROADMAP criterion 4)

**Outcome:** normal-four-target

**CI-tested candidate SHA:** `5fea02a00d23665353fed8e317d7cb7ab8325694`

- **Workflow run:** [CI run 34287100850](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850)
- **Run state:** `completed` / `success`
- **Completed:** 2026-09-08T22:42:13Z
- **Committed golden SHA-256:** `3e53f10babd78b721d524c0e41fbb0806a5ad37d2f5b4dd247a4336e0be1282c`

The run's `headSha` is the exact candidate above. Every target executed the full
test suite and then the separately named `Golden fixture reproduces on this target
(GUARD-09, GUARD-10)` step. A successful named step means that target regenerated
the IAMF output, matched the committed golden artifact and hash byte-for-byte, and
passed the same-process double-encode assertion.

| target | job | runner route | job conclusion | golden status | golden conclusion |
|---|---|---|---|---|---|
| aarch64-apple-darwin | [102265091867](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265091867) | native `macos-latest` arm64 | success | completed | success |
| x86_64-apple-darwin | [102265092177](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092177) | x86_64 binaries executed under Rosetta 2 on `macos-latest` arm64 | success | completed | success |
| x86_64-pc-windows-msvc | [102265092145](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092145) | native `windows-latest`, committed LF bytes preserved before checkout | success | completed | success |
| x86_64-unknown-linux-gnu | [102265092207](https://github.com/mkschulze/iamf-rs/actions/runs/34287100850/job/102265092207) | native `ubuntu-latest` x86_64 | success | completed | success |

The predecessor run
[34286069472](https://github.com/mkschulze/iamf-rs/actions/runs/34286069472),
for candidate `61ba098413b48751838035c41a068b270a7a1ba0`, reproduced two
infrastructure failures: Windows checkout converted the committed golden dump from
LF to CRLF, and GitHub no longer offered the native `macos-13` x86_64 runner. Commit
`5fea02a00d23665353fed8e317d7cb7ab8325694` changed only the workflow: it disables
checkout conversion before Windows checkout and executes the x86_64 macOS test
binaries through Rosetta. No successful jobs were combined across SHAs; every row
above comes from the new, complete run 34287100850.

The later closure HEAD may contain this evidence and Phase 1 planning/state records,
but the candidate remains the tested code identity. Closure therefore requires an
ancestry check plus the explicit documentation/state descendant-path allowlist from
plan 01-10; any source, test, fixture, workflow, toolchain or dependency change after
this candidate invalidates this evidence and requires a new matrix run.


## Waivers

### W-1 — CONF-05 for the 24-bit **big-endian** sample format (opened 2026-09-08, plan 01-08)

1. **The clause.** CONF-05 — "`libiamf` decodes the file and the PCM is
   sample-identical" — as applied to a fixture with `sample_size == 24` and
   `sample_format_flags == 0` (big-endian). CONF-05 itself is **not** waived: it is
   asserted in full on the 24-bit little-endian sample-identity fixture and again on
   the 16-bit big-endian endianness fixture. What cannot be evaluated is the
   combination.

2. **What was attempted.** A stereo 48 kHz 24-bit big-endian file, written through
   the real `SequenceWriter` with a −6 dBFS per-channel ramp and 300 sample frames,
   decoded by the pinned `iamfdec` with `-r 48000 -s0 -d 24 -disable_limiter`. The
   decode succeeded: 300 of 300 sample frames, correct channel count, structurally
   sound. 597 of 600 samples came back wrong. See Experiment 3 for the full run.

3. **Why it cannot be met.** `libiamf@v1.1.0`'s `reads24be`
   (`code/src/iamf_dec/bitstream.c:206-210`) calls `readu16le` where `readu16be` was
   meant, transposing the top two bytes of every 24-bit big-endian sample. This is a
   defect in the pinned reference, not in this crate: every one of the 600 decoded
   samples equals what that code computes from the bytes we wrote — all 600, not just
   the 597 that differ — which is asserted by
   `probe_24bit_big_endian_hits_the_upstream_reads24be_defect`. No conformant
   encoder can produce a 24-bit big-endian file that this decoder reads correctly, so
   the clause is unevaluable rather than failing.

4. **What weaker property is asserted instead.** Three, together covering both halves
   of what 24-bit big-endian was chosen for:
   - **24-bit depth, end to end:** `probe_24bit_little_endian_round_trips_exactly`
     and the sample-identity fixture — zero differing samples through the reference.
   - **Big-endian sense, end to end:** `probe_16bit_big_endian_round_trips_exactly`
     and the endianness fixture — zero differing samples through the reference.
   - **24-bit big-endian storage, by hand:** `store_sample_writes_the_declared_byte_order`
     asserts the exact three bytes for a positive and a negative sample in both
     orders, against values computed by hand and not captured from our own output.
   - **The defect is bounded:** the misread is asserted to be *complete* — all 600
     samples explained — so "24-bit big-endian is wrong somewhere else too" is ruled
     out rather than assumed.

5. **What would restore it.** An upstream fix to `reads24be`, and `REFERENCES.md`'s
   `libiamf` pin moving to a revision carrying it. **This condition is executable:**
   `probe_24bit_big_endian_hits_the_upstream_reads24be_defect` asserts
   `trip.differing > 0`, so the day the round trip becomes exact that test fails with
   a message pointing back at this waiver. Remove this entry and move the
   sample-identity fixture to 24-bit big-endian, which is what D-18 originally
   specified.



Any reduction to the four-target byte-identity matrix — for example dropping
`x86_64-apple-darwin` if GitHub retires its last x86_64 macOS runner image — is
recorded here as a waiver, with its reason, rather than appearing as a silently
shrunk matrix in `.github/workflows/ci.yml`. See that file's runner-availability
fallback block.
