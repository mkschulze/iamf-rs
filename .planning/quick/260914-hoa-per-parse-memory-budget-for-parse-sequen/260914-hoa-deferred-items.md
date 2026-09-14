# 260914-hoa deferred items

1. **Whole-file `ParseLimits` / `parse_sequence_with_limits`.** A unit-table byte budget (never `size_of`) with a new
   `ErrorKind::ParseBudgetExceeded`, charged at about 20 allocation sites across `src/obu/*`. Only worth doing if a
   consumer needs a hard cap on a retained whole-file model. Reference bases for numbers: `kEntireObuSizeMaxTwoMegabytes`
   (`iamf/obu/types.h:32`) and the 4 MiB `StreamBasedReadBitBuffer` source cap (`iamf/common/read_bit_buffer.cc:509-512`).
2. **`kMaxNumParameters = 256` on Audio Element read.** iamf-tools@v2.1.0 enforces it (`iamf/obu/audio_element.h:287`,
   `ValidateNumParameters` `audio_element.cc:97-113`, called at `:811`), while the spec says parsers SHALL support any
   value. It would cap the AE-param classes (34-50x per OBU). This is a user question: whether "stricter wins" applies to
   a resource limit. It also needs a reference-hash check.
3. **Registry de-duplication.** `ParamDefinitionRegistry::register` pushes duplicates
   (`src/obu/param_definition.rs:89-94`), so redundant descriptor copies grow the registry linearly even inside
   `SequenceReader`, by about 72 B per 12 wire bytes. `entries()` is public and documented as "including duplicates".
4. **Boxing `SequenceObu` variants** (192 -> about 16 B inline, about 96x -> about 40x). Breaking public change.
5. **Temporal-unit streaming reader** mirroring `SequenceWriter`. Blocked by whole-sequence `uses_delimiters` in
   `temporal_unit_ranges` (`src/sequence.rs`, `ParsedSequence::temporal_unit_ranges`).
6. **O(n·m) `registry.get` linear scan** (`param_definition.rs:177-181`). CPU cost, not memory.
7. **Evidence limits.** The 192-byte figure was probed only on macOS (aarch64 per research, x86_64 per planner). It is
   documentation, not asserted by a test, because asserting a layout would make a test depend on `size_of`. RSS was
   measured only on macOS (this run: x86_64, eager 3,251,961,856 B, stream 34,299,904 B). A copying `realloc`
   (e.g. Windows) could transiently touch about 192x (research A2, ASSUMED). The Task 2 local 60 s fuzz run **did run**
   locally (nightly-2026-09-01, cargo-fuzz 0.13.2): 465,862 runs in 61 s from a scratch corpus copy, no crash or
   assertion failure. It is not a substitute for the CI nightly discovery job.
