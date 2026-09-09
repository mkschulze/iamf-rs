#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "support/sequence_cases.rs"]
mod sequence_cases;

use iamf::sequence::{ParsedSequence, parse_sequence, write_parsed_sequence};
use proptest::prelude::*;
use sequence_cases::canonical_parsed_strategy;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn model_round_trip(sequence in canonical_parsed_strategy()) {
        let bytes = write_parsed_sequence(Vec::new(), &sequence).expect("canonical model writes");
        let parsed = parse_sequence(&bytes).expect("own bytes parse");

        prop_assert_eq!(parsed, sequence);
    }
}

#[test]
fn from_parts_is_an_ordering_adapter() {
    let case = sequence_cases::rich_case(2, true, vec![1, 2, 3], -7);
    let parsed = ParsedSequence::from_parts(&case.descriptors, &case.units)
        .expect("canonical parts have a flat ordering");

    assert!(!parsed.obus.is_empty());
}
