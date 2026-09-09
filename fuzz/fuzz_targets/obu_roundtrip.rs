#![no_main]

use libfuzzer_sys::arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut input = Unstructured::new(data);
    let Ok(sequence) = iamf::fuzzing::sequence_from_fuzz_bytes(&mut input) else {
        return;
    };
    let bytes = iamf::sequence::write_parsed_sequence(Vec::new(), &sequence)
        .expect("the bounded generator must only produce writer-valid models");
    let parsed = iamf::sequence::parse_sequence(&bytes)
        .expect("bytes from the production writer must parse");
    assert_eq!(parsed, sequence);
    let rewritten = iamf::sequence::write_parsed_sequence(Vec::new(), &parsed)
        .expect("a parsed writer-valid model must remain writer-valid");
    assert_eq!(rewritten, bytes);
});
