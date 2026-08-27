use clutch_retirement_adapter::RetirementAdapterErrorV1;

fn exhaustive_v1_match(error: RetirementAdapterErrorV1) -> u8 {
    match error {
        RetirementAdapterErrorV1::Retirement(_) => 0,
        RetirementAdapterErrorV1::BaseCodec(_) => 1,
        RetirementAdapterErrorV1::BaseLengthMismatch => 2,
        RetirementAdapterErrorV1::WrongOwner => 3,
        RetirementAdapterErrorV1::WrongPda => 4,
        RetirementAdapterErrorV1::NotWritable => 5,
        RetirementAdapterErrorV1::WrongBump => 6,
        RetirementAdapterErrorV1::InvalidSchema => 7,
    }
}

#[test]
fn downstream_adapter_v1_exhaustive_match_still_compiles() {
    assert_eq!(
        exhaustive_v1_match(RetirementAdapterErrorV1::InvalidSchema),
        7
    );
}
