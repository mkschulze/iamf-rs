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
**not** the D-18 Phase 1 fixture (5.1, 48 kHz, 24-bit big-endian). CONF-08 and
CONF-02..05 are two different files. Do not conflate them.

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

**What the CONF-06 signal actually is has NOT been established.** An earlier
revision of this section asserted "a non-zero exit or an error on stderr is the
CONF-06 signal". That was an inference, and Experiment A below refutes the
general form of it by execution: `libiamf`'s `iamfdec` returns **exit 0 on every
one of five inputs**, including two that decode to zero samples. Whether
`decoder_main` behaves differently is Experiment 1, and Experiment 1 has not run
(the container could not be built on the authoring machine). Until it does,
CONF-06 has a route but no asserted observable, and a harness written against
the exit code would be a gate that passes on a failed parse.

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

### Experiment 1 — how does `decoder_main` signal a parse failure? (NOT RUN)

Research open question 1, and the thing CONF-06's value rests on.

**Why it did not run.** The `iamf-tools` oracle only exists inside the
digest-pinned container (D-11: Bazel + abseil + protobuf + fdk-aac is not a
build to run natively). On the authoring machine the `docker` CLI is present at
`/usr/local/bin/docker` but the **daemon is not running** —
`Cannot connect to the Docker daemon at unix:///Users/cell/.docker/run/docker.sock` —
and Docker Desktop could not be started from this session. `bazel`/`bazelisk`
are absent by design. So the image could be *authored* and pinned, and it is
(`tools/iamf-tools.Dockerfile`), but it could not be *built* or *run*.

**This is unverified, not blocked.** `.github/workflows/reference.yml` builds the
image on `ubuntu-latest`, so the first run of that workflow is where this
executes. It is recorded in `.planning/WINDOWS.md` so it surfaces at ship time
rather than being rediscovered.

**The exact procedure, ready to run.** Nothing here needs deciding; it needs a
daemon.

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

**Record three observations per case, separately:** the exit code, the verbatim
stderr, and whether an output file appeared and how large it is. **Do not assume
the exit code is the signal** — Experiment A above is the counter-example, in
this repository, on the sibling tool.

**The conclusion this experiment owes the project:** the name of the observable
`.github/workflows/reference.yml`'s CONF-06 step asserts on. That step currently
asserts only `test -s <output>.wav`, which is the weakest defensible claim and
is deliberately marked as such until this runs.

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

### Experiment 2 — does `encoder_main` accept an unreferenced Codec Config? (NOT RUN)

Research assumption A3, and D-18's stated research item for this plan. D-18's
Phase 1 fixture is **two Codec Configs and one 5.1 Audio Element**, so the
second Codec Config is referenced by nothing. The parser side is verified legal
from source; the encoder side is not, and that is what this settles.

**Why it did not run.** Same cause as Experiment 1: the Docker daemon was not
running on the authoring machine.

**The exact procedure, ready to run.** The input is already prepared and
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

**If `encoder_main` refuses, the fallback is already chosen and must be recorded
here when adopted**, in this order:

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

## Waivers

*(none)*

Any reduction to the four-target byte-identity matrix — for example dropping
`x86_64-apple-darwin` if GitHub retires its last x86_64 macOS runner image — is
recorded here as a waiver, with its reason, rather than appearing as a silently
shrunk matrix in `.github/workflows/ci.yml`. See that file's runner-availability
fallback block.
