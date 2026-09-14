#![no_main]

use iamf::sequence::{SequenceObu, SequenceReader, parse_sequence};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let eager = parse_sequence(data).map(|sequence| sequence.obus);
    // The committed corpus replays this differential on stable in
    // tests/sequence_reader.rs; the nightly discovery job adds the hostile
    // inputs that corpus does not have, so a divergence aborts like a crash.
    let streamed = SequenceReader::new(data).collect::<Result<Vec<SequenceObu>, iamf::Error>>();
    assert_eq!(
        streamed, eager,
        "quick 260914-hoa: SequenceReader and parse_sequence disagree"
    );
});
