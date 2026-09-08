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
`ObuProcessor`/`DescriptorObuParser` and so exercises the strict parser. A
non-zero exit or an error on stderr is the CONF-06 signal.

## Recorded experiments

Experiments run against the gate that are worth keeping even though they are not
waivers — a byte diff explained, a decode compared, a configuration ruled out.
Plans 01-02 and 01-08 append here.

*(none yet)*

## Waivers

*(none)*

Any reduction to the four-target byte-identity matrix — for example dropping
`x86_64-apple-darwin` if GitHub retires its last x86_64 macOS runner image — is
recorded here as a waiver, with its reason, rather than appearing as a silently
shrunk matrix in `.github/workflows/ci.yml`. See that file's runner-availability
fallback block.
