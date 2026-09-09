# Phase 2 expectation provenance: unpaired iamf-tools fixtures

The four files below have no paired configuration proto in
`iamf/cli/testdata/` at the pinned iamf-tools revision.  Their Phase 2
expectations therefore come directly from the committed bytes, not from a
filename summary and not from this crate's parser.

All four were checked at iamf-tools commit
`848c6ff4968ff8cc6f728259892ab4f90cb83256`.  For each file the procedure was:

1. compare the local file with the pinned git blob and record both object IDs;
2. walk the common headers from `xxd -g1`, adding `1 + sizeof(obu_size) +
   obu_size` at every boundary;
3. decode fields using the pinned `ObuHeader::ReadAndValidate`,
   `IASequenceHeaderObu::ReadAndValidatePayloadDerived`,
   `CodecConfigObu::ReadAndValidatePayloadDerived`,
   `AudioElementObu::ReadAndValidatePayloadDerived`,
   `MixPresentationObu::ReadAndValidatePayloadDerived`, and
   `AudioFrameObu::ReadAndValidatePayloadDerived` layouts; and
4. cross-check only the boundary arithmetic with the Phase 1 walker and the
   semantic transcription with the pinned iamf-tools decoder's verbose log.

The common header byte is `type:5 | redundant:1 | trimming:1 | extension:1`.
Every header below has redundant = 0, extension = 0, and no extension bytes.
All descriptor trailing-byte vectors are empty.  `AFn` means the compact Audio
Frame OBU type whose substream ID is `n`; audio payload bytes remain opaque.
No file contains a Temporal Delimiter or Parameter Block, so there is no
parameter data to interpret and canonical temporal units are determined by
the repeated-frame-ID rule.

## `noise_1024samp_5p1_opus.iamf`

| Provenance | Value |
|---|---|
| Upstream path | `iamf/cli/testdata/iamf/noise_1024samp_5p1_opus.iamf` |
| Git blob | `d11c26eb8ed492eda1a07c6e6b87504aa0d0febd` |
| Local bytes | 2,228; SHA-256 `2115fd08eee9791f1e35fe4a4bfb268ab02cc096ad9a5cc950aa911999fd1cae` |
| Raw codec bytes | offset 19, length 11; SHA-256 `6f16678e7c8864729b653873d8536abb564ec78c80ef557c7b3638351c3cc53b` |

| OBU index | Offset | Header/type | `obu_size` | End | Frame trim `(end,start)` |
|---:|---:|---|---:|---:|---|
| 0 | 0 | `f8` / IA Sequence Header | 6 | 8 | — |
| 1 | 8 | `00` / Codec Config | 20 | 30 | — |
| 2 | 30 | `08` / Audio Element | 13 | 45 | — |
| 3 | 45 | `10` / Mix Presentation | 63 | 110 | — |
| 4 | 110 | `30` / AF0 | 242 | 355 | (0,0) |
| 5 | 355 | `38` / AF1 | 242 | 600 | (0,0) |
| 6 | 600 | `40` / AF2 | 250 | 853 | (0,0) |
| 7 | 853 | `48` / AF3 | 250 | 1,106 | (0,0) |
| 8 | 1,106 | `32` / AF0, trimming | 266 | 1,375 | (584,0) |
| 9 | 1,375 | `3a` / AF1, trimming | 266 | 1,644 | (584,0) |
| 10 | 1,644 | `42` / AF2, trimming | 289 | 1,936 | (584,0) |
| 11 | 1,936 | `4a` / AF3, trimming | 289 | 2,228 | (584,0) |

- Sequence header: `ia_code = "iamf"`, primary/additional profiles `(0,0)`.
- Codec 0: FourCC `Opus`, 960 samples/frame, roll distance -4; raw config is
  `01 02 01 38 00 00 bb 80 00 00 00` at the position above.
- Element 1: channel based, reserved 0, codec 0, substreams `[0,1,2,3]`, no
  element parameters; one layer, layout 2 (5.1), layer flags/reserved 0,
  substream count 4, coupled count 2, no expanded layout.
- Mix 3: languages `["en-us"]`, presentation annotation
  `["default_mix_presentation"]`; one submix referring to element 1 with
  annotation `"5.1"`, headphones mode 1 and empty rendering extension.
  Element gain is Mix Gain `(id=101, rate=48000, mode=1, reserved=0,
  default=0)`; output gain is the same shape with id 100.  Its sole layout is
  loudspeakers-SS, sound system 0 (stereo), reserved 0, loudness
  `(info=0, integrated=0, digital_peak=0)`.
- Temporal units: two groups, each ordered `[AF0,AF1,AF2,AF3]`; no reserved or
  trailing data.  The pinned reader reports no semantic error.

## `noise_1024samp_stereo_flac.iamf`

| Provenance | Value |
|---|---|
| Upstream path | `iamf/cli/testdata/iamf/noise_1024samp_stereo_flac.iamf` |
| Git blob | `bb4f3589da46e81cc8a32887b0b8e38eb9136ca4` |
| Local bytes | 4,293; SHA-256 `4bcc7b0ba187da62915a209e1e8ebc02978b4d35cbd392a13d4a4ce9340a269a` |
| Raw codec bytes | offset 19, length 38; SHA-256 `14bc8e30154e84e9f22afaa27f0a4d7e87b09f25a9ff23ac768aa165c8436e09` |

| OBU index | Offset | Header/type | `obu_size` | End | Frame trim |
|---:|---:|---|---:|---:|---|
| 0 | 0 | `f8` / IA Sequence Header | 6 | 8 | — |
| 1 | 8 | `00` / Codec Config | 47 | 57 | — |
| 2 | 57 | `08` / Audio Element | 10 | 69 | — |
| 3 | 69 | `10` / Mix Presentation | 66 | 137 | — |
| 4 | 137 | `30` / AF0 | 4,153 | 4,293 | (0,0) |

- Sequence header profiles are `(0,0)`.
- Codec 0: FourCC `fLaC`, 4,608 samples/frame, roll distance 0.  The 38-byte
  raw STREAMINFO-shaped remainder begins `80 00 00 80 00 00 22 12` and is
  asserted only as opaque bytes in Phase 2.
- Element 1 is channel based with reserved 0, codec 0, substream `[0]`, no
  parameters; one layout-1 (stereo) layer, zero flags/reserved, substream and
  coupled counts both 1.
- Mix 3 is the same one-element structure as above except the element
  annotation is `"stereo"`; both gain definitions and the loudness layout are
  otherwise identical.
- One delimiter-free temporal unit contains AF0; no reserved/trailing data or
  semantic finding.

## `noise_3s_stereo_opus.iamf`

| Provenance | Value |
|---|---|
| Upstream path | `iamf/cli/testdata/iamf/noise_3s_stereo_opus.iamf` |
| Git blob | `574e2bf75724486189fb2ecf5e605b674102f0aa` |
| Local bytes | 2,319; SHA-256 `a91b89d0691fa3e92e29ae2805019bb355081b9ae84cdd715269f4c5f99c5693` |
| Raw codec bytes | offset 18, length 11; SHA-256 `62111fe8161f0727d8dac66e5e5e8a61751fda1b1d1df24211e6d9e91791f758` |

Descriptor boundaries are `0,8,29,41,109`.  The remaining 87 OBUs are AF0.
Their start offsets, copied from the byte walk, are:

```text
109,139,165,191,216,244,271,297,322,346,372,397,420,447,473,500,
525,551,574,598,624,649,674,701,726,750,777,802,829,854,879,905,
931,957,984,1011,1037,1062,1087,1113,1137,1165,1191,1217,1241,
1267,1291,1316,1340,1365,1390,1414,1440,1466,1491,1514,1539,
1564,1587,1612,1635,1659,1684,1708,1732,1757,1782,1806,1831,
1856,1883,1910,1936,1960,1985,2010,2033,2057,2081,2107,2132,
2156,2181,2206,2232,2257,2284
```

The corresponding `obu_size` values are:

```text
28,24,24,23,26,25,24,23,22,24,23,21,25,24,25,23,24,21,22,24,
23,23,25,23,22,25,23,25,23,23,24,24,24,25,25,24,23,23,24,22,
26,24,24,22,24,22,23,22,23,23,22,24,24,23,21,23,23,21,23,21,
22,23,22,22,23,23,22,23,23,25,25,24,22,23,23,21,22,22,24,23,
22,23,23,24,23,25,33
```

Every frame header is `30` except the last (`32`, trimming enabled).  Frames
0..85 have trim `(0,0)`; frame 86 has `(96,0)`.  Adding each header byte,
one-byte size, and size above yields the next listed offset and finally 2,319.

- Sequence profiles `(0,0)`; Codec 0 is `Opus`, 120 samples/frame, roll -32,
  raw config `01 02 00 78 00 00 bb 80 00 00 00`.
- Element 1 is the same channel-based stereo shape as the FLAC file.
- Mix 3 is the same `"stereo"` mix, with gain ids 101 and 100 at rate 48,000
  and one sound-system-0 loudness layout.
- Repeated AF0 begins a new canonical temporal unit, giving 87 units.  There
  are no reserved/trailing bytes or pinned-reader semantic findings.

## `tones_100ms_3OA_stereo_opus.iamf`

| Provenance | Value |
|---|---|
| Upstream path | `iamf/cli/testdata/iamf/tones_100ms_3OA_stereo_opus.iamf` |
| Git blob | `1c68c507f034b9ad45537dac5cf906660a37eea3` |
| Local bytes | 4,715; SHA-256 `d3d1405cd2ea4b93e6c7cabda110037983862f5b4755573f203b84b8b1d49cdf` |
| Raw codec bytes | offset 19, length 11; SHA-256 `6f16678e7c8864729b653873d8536abb564ec78c80ef557c7b3638351c3cc53b` |

The complete start-offset vector is:

```text
0,8,30,48,60,142,333,543,733,929,1029,1149,1287,1414,1580,1676,
1827,1953,2098,2276,2374,2517,2636,2795,2974,3078,3209,3358,3505,
3681,3809,4005,4214,4424,4634,4715
```

OBUs 0..4 are sequence header, codec, Audio Element 1, Audio Element 2, and
Mix Presentation.  OBUs 5..34 are six repeated groups of `[AF0,AF1,AF2,AF3,
AF4]`.  Their `obu_size` values are:

```text
188,207,187,193,98, 118,135,125,163,94, 148,124,142,175,96,
140,117,156,176,102, 128,146,144,173,126, 193,206,207,207,79
```

The first five groups have trim `(0,0)` and header bytes
`30,38,40,48,50`; the last has trim `(648,0)` and headers
`32,3a,42,4a,52`.  Size fields are two bytes exactly where the value is at
least 128; manual addition lands on every offset and final length above.

- Sequence profiles `(1,1)`; Codec 0 is `Opus`, 960 samples/frame, roll -4,
  with the same 11 raw bytes/digest as the 5.1 file.
- Element 1 is scene based, reserved 0, codec 0, substreams `[0,1,2,3]`, no
  parameters; ambisonics mode 0 (mono), output/substream counts `(4,4)` and
  channel mapping `[0,1,2,3]`.
- Element 2 is channel based, reserved 0, codec 0, substream `[4]`, no
  parameters; one layout-1 stereo layer, zero flags/reserved, substream and
  coupled counts both 1.
- Mix 3 has one submix referring in order to elements `[1,2]`, annotations
  `["FOA","stereo"]`, headphone modes `[1,0]`, empty rendering extensions,
  and element Mix Gain definitions `(id=101,rate=48000,mode=1,reserved=0,
  default=0)` for both references.  Output Mix Gain uses id 100 with the same
  remaining fields.  Its sole layout is sound system 0 with zero loudness.
- Six delimiter-free temporal units contain the five frame IDs in order.
  Reserved/trailing bytes are empty.  The pinned reader accepts the stream;
  the repeated id 101 is retained exactly rather than deduplicated.

These hand-decodes are the sole independent source for the four unpaired Rust
expectation rows.  The other 34 positive rows are grounded in their paired
`libiamf@v1.1.0` textprotos.
