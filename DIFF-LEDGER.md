# Byte-diff ledger — our encoder vs `iamf-tools`' `encoder_main` (CONF-07, D-12)

**This file is asserted by `cargo test`, not merely read.**
`tests/conformance.rs::conf_07_byte_diff_matches_the_committed_ledger` computes
the byte-by-byte difference between the `.iamf` this crate writes for the
sample-identity fixture and the one `iamf-tools`' `encoder_main` writes from the
same configuration and the same PCM, and asserts that the set of differing
offsets equals the table below **exactly, in both directions**:

- a **new** difference fails — the encoder changed and nobody explained it;
- a difference that has **vanished** fails too, forcing this file to be updated.

The second half is the one that matters. Without it this document silently
becomes a description of a file that no longer exists, and Phase 1's success
criterion 3 — "every byte difference against `iamf-tools` output is either
absent or explained in writing" — passes on a stale writeup. That is exactly the
failure D-12 exists to prevent, and it is why the prose explanation and the
enforcement are one artifact rather than two.

Rows are compared as a **set keyed by offset**, sorted ascending, so reordering
this file does not change the verdict. A row whose `ours` and `theirs` are equal
is a **ledger error** and fails: a row that records no difference cannot be
describing one. A row with an empty `why` is rejected too — "differs at offset
N" is not an explanation, and neither is a blank cell.

## Why the comparison is meaningful at all

`libiamf` is a *permissive* reader. Experiment A flipped a reserved bit in a
descriptor and got a **byte-identical** WAV back; Experiment 1 showed
`iamf-tools`' `decoder_main` is blind to that same bit. So CONF-05 and CONF-06
together still cannot see reserved-bit misuse, and this diff is the only clause
that can. "Passing the `libiamf` gate is necessary and nowhere near sufficient"
is the first line of `CONFORMANCE-GATE.md`; this table is what makes the rest of
that sentence true.

## What is compared

| | |
|---|---|
| fixture | `tests/support/fixture.rs::sample_identity` — one 5.1 Audio Element, one Codec Config, 48 kHz, 24-bit little-endian, 300 sample frames (`trim_at_end = 84`) |
| ours | `iamf::sequence::SequenceWriter`, the same path the public API uses |
| theirs | `iamf-tools@v2.1.0` `encoder_main`, in the digest-pinned container |
| configuration | **generated** from the same `DescriptorSet` our encoder wrote, in `iamf-tools`' `UserMetadata` proto dialect, by `tests/conformance.rs::textproto_for` |
| loudness | `validate_user_loudness: true`, so the reference uses the fixture's numbers rather than measuring its own — a loudness difference here would be a real encoding difference, not a measurement one |

The textproto is **generated rather than committed**, deliberately. A committed
copy would be a second description of one configuration; the two would drift, and
CONF-07 would then be comparing two files built from *different* configurations
and reporting the difference as an encoder defect.

## The ledger

<!-- gsd:ledger-table -->

| offset | field | ours | theirs | why |
|---|---|---|---|---|

**The table is empty, and that is a result, not a placeholder.** Every byte of
the sample-identity fixture that this crate writes is identical to the byte
`iamf-tools`' own encoder writes for the same configuration — the descriptor
prologue, the 5.1 BCG channel→substream packing, the implicit substream ids, the
END-before-START trim order, the two-pass `obu_size`, and all four Audio Frames
of all three temporal units.

An empty ledger that is *asserted* is meaningfully different from no ledger at
all: if a difference appears, this file fails until someone writes down what it
is and why it is legal.
