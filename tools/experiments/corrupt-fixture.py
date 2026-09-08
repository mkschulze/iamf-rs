#!/usr/bin/env python3
"""Produce the three corrupted variants used by the failure-signal experiments.

CONFORMANCE-GATE.md records what each reference tool does with these. The
corruptions live in a script rather than in prose so the experiment is
reproducible byte-for-byte by whoever runs it next -- in particular by the
`reference` CI job, which is where Experiment 1 (`decoder_main`'s failure
signal) will finally execute.

Defaults are for `tests/fixtures/reference/test_000003.iamf`, whose descriptor
OBUs sit at offsets 0 (IA Sequence Header), 8 (Codec Config), 26 (Audio Element)
and 40 (Mix Presentation), with the first Audio Frame OBU at 120.

    python3 tools/experiments/corrupt-fixture.py \\
        tests/fixtures/reference/test_000003.iamf /tmp/corrupt

Writes valid.iamf, bitflip_reserved.iamf, bitflip_sample_rate.iamf,
obusize.iamf and truncated.iamf into the output directory.
"""

import pathlib
import sys


def main(argv: list[str]) -> int:
    src_path = pathlib.Path(
        argv[1] if len(argv) > 1 else "tests/fixtures/reference/test_000003.iamf"
    )
    out_dir = pathlib.Path(argv[2] if len(argv) > 2 else "/tmp/corrupt")
    out_dir.mkdir(parents=True, exist_ok=True)

    src = src_path.read_bytes()
    if len(src) < 200:
        print(f"{src_path} is too small to corrupt meaningfully", file=sys.stderr)
        return 1

    written = []

    def emit(name: str, data: bytes, note: str) -> None:
        path = out_dir / name
        path.write_bytes(data)
        written.append((name, len(data), note))

    # 0. Baseline. Every experiment needs the uncorrupted control, or a "failure"
    #    signal cannot be distinguished from the tool's ordinary behaviour.
    emit("valid.iamf", src, "unmodified control")

    # 1. Flip one bit inside a descriptor payload -- specifically a RESERVED bit.
    #    Byte 30 is the audio_element_type (3 bits) + reserved (5 bits) octet of
    #    the Audio Element OBU at offset 26. Bit 0 is in the reserved field, so
    #    this is reserved-bit misuse and nothing else.
    a = bytearray(src)
    a[30] ^= 0x01
    emit(
        "bitflip_reserved.iamf",
        bytes(a),
        "byte 30 bit 0: a RESERVED bit of the Audio Element's type octet, set to 1",
    )

    # 2. Flip one bit inside a descriptor payload that CHANGES MEANING. Byte 23
    #    is inside the Codec Config's 4-byte sample_rate (00 00 3e 80 = 16000);
    #    setting bit 0 makes it 0x00013e80 = 81536.
    b = bytearray(src)
    b[23] ^= 0x01
    emit(
        "bitflip_sample_rate.iamf",
        bytes(b),
        "byte 23 bit 0: Codec Config sample_rate 16000 -> 81536",
    )

    # 3. Overwrite one obu_size byte with a LARGER value. Byte 1 is the
    #    single-byte leb128 obu_size of the IA Sequence Header OBU at offset 0.
    c = bytearray(src)
    original = c[1]
    c[1] = 0x7F
    emit(
        "obusize.iamf",
        bytes(c),
        f"byte 1: IA Sequence Header obu_size {original} -> 127, so every later "
        f"OBU boundary is wrong",
    )

    # 4. Truncate mid-OBU. 120 is the first Audio Frame boundary; 160 lands 40
    #    bytes inside that frame.
    emit("truncated.iamf", src[:160], "truncated 40 bytes into the first Audio Frame OBU")

    for name, size, note in written:
        print(f"{out_dir / name}  {size} bytes  -- {note}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
