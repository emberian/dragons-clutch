use clutch_retirement::{
    AuthenticatedEpochChildV1, ChildSlotV1, EpochChildKindV1, RetirementErrorV1, RetirementErrorV2,
};

fn committed_v1_variant_index(error: RetirementErrorV1) -> u8 {
    match error {
        RetirementErrorV1::Truncated => 0,
        RetirementErrorV1::TrailingBytes => 1,
        RetirementErrorV1::WrongTag => 2,
        RetirementErrorV1::WrongVersion => 3,
        RetirementErrorV1::ZeroIdentity => 4,
        RetirementErrorV1::WrongGeneration => 5,
        RetirementErrorV1::InvalidEnum => 6,
        RetirementErrorV1::NonCanonicalState => 7,
        RetirementErrorV1::ArithmeticOverflow => 8,
        RetirementErrorV1::CounterUnderflow => 9,
        RetirementErrorV1::NonmonotoneEpoch => 10,
        RetirementErrorV1::EpochIndexExhausted => 11,
        RetirementErrorV1::EconomicBalanceOutstanding => 12,
        RetirementErrorV1::ReservationOutstanding => 13,
        RetirementErrorV1::ChildOutstanding => 14,
        RetirementErrorV1::AlreadyTerminal => 15,
        RetirementErrorV1::WrongPhase => 16,
        RetirementErrorV1::ChildAlreadyPresent => 17,
        RetirementErrorV1::ChildAbsent => 18,
        RetirementErrorV1::WrongChildKind => 19,
        RetirementErrorV1::ClearWorkOutstanding => 20,
        RetirementErrorV1::AccountBalanceShortfall => 21,
        RetirementErrorV1::PayerIsNeutralSink => 22,
    }
}

#[test]
fn downstream_exhaustive_match_and_historical_child_name_still_compile() {
    let variants = [
        RetirementErrorV1::Truncated,
        RetirementErrorV1::TrailingBytes,
        RetirementErrorV1::WrongTag,
        RetirementErrorV1::WrongVersion,
        RetirementErrorV1::ZeroIdentity,
        RetirementErrorV1::WrongGeneration,
        RetirementErrorV1::InvalidEnum,
        RetirementErrorV1::NonCanonicalState,
        RetirementErrorV1::ArithmeticOverflow,
        RetirementErrorV1::CounterUnderflow,
        RetirementErrorV1::NonmonotoneEpoch,
        RetirementErrorV1::EpochIndexExhausted,
        RetirementErrorV1::EconomicBalanceOutstanding,
        RetirementErrorV1::ReservationOutstanding,
        RetirementErrorV1::ChildOutstanding,
        RetirementErrorV1::AlreadyTerminal,
        RetirementErrorV1::WrongPhase,
        RetirementErrorV1::ChildAlreadyPresent,
        RetirementErrorV1::ChildAbsent,
        RetirementErrorV1::WrongChildKind,
        RetirementErrorV1::ClearWorkOutstanding,
        RetirementErrorV1::AccountBalanceShortfall,
        RetirementErrorV1::PayerIsNeutralSink,
    ];
    for (expected, error) in (0u8..=22).zip(variants) {
        assert_eq!(committed_v1_variant_index(error), expected);
    }

    let conversions = [
        (RetirementErrorV1::Truncated, RetirementErrorV2::Truncated),
        (
            RetirementErrorV1::TrailingBytes,
            RetirementErrorV2::TrailingBytes,
        ),
        (RetirementErrorV1::WrongTag, RetirementErrorV2::WrongTag),
        (
            RetirementErrorV1::WrongVersion,
            RetirementErrorV2::WrongVersion,
        ),
        (
            RetirementErrorV1::ZeroIdentity,
            RetirementErrorV2::ZeroIdentity,
        ),
        (
            RetirementErrorV1::WrongGeneration,
            RetirementErrorV2::WrongGeneration,
        ),
        (
            RetirementErrorV1::InvalidEnum,
            RetirementErrorV2::InvalidEnum,
        ),
        (
            RetirementErrorV1::NonCanonicalState,
            RetirementErrorV2::NonCanonicalState,
        ),
        (
            RetirementErrorV1::ArithmeticOverflow,
            RetirementErrorV2::ArithmeticOverflow,
        ),
        (
            RetirementErrorV1::CounterUnderflow,
            RetirementErrorV2::CounterUnderflow,
        ),
        (
            RetirementErrorV1::NonmonotoneEpoch,
            RetirementErrorV2::NonmonotoneEpoch,
        ),
        (
            RetirementErrorV1::EpochIndexExhausted,
            RetirementErrorV2::EpochIndexExhausted,
        ),
        (
            RetirementErrorV1::EconomicBalanceOutstanding,
            RetirementErrorV2::EconomicBalanceOutstanding,
        ),
        (
            RetirementErrorV1::ReservationOutstanding,
            RetirementErrorV2::ReservationOutstanding,
        ),
        (
            RetirementErrorV1::ChildOutstanding,
            RetirementErrorV2::ChildOutstanding,
        ),
        (
            RetirementErrorV1::AlreadyTerminal,
            RetirementErrorV2::AlreadyTerminal,
        ),
        (RetirementErrorV1::WrongPhase, RetirementErrorV2::WrongPhase),
        (
            RetirementErrorV1::ChildAlreadyPresent,
            RetirementErrorV2::ChildAlreadyPresent,
        ),
        (
            RetirementErrorV1::ChildAbsent,
            RetirementErrorV2::ChildAbsent,
        ),
        (
            RetirementErrorV1::WrongChildKind,
            RetirementErrorV2::WrongChildKind,
        ),
        (
            RetirementErrorV1::ClearWorkOutstanding,
            RetirementErrorV2::ClearWorkOutstanding,
        ),
        (
            RetirementErrorV1::AccountBalanceShortfall,
            RetirementErrorV2::AccountBalanceShortfall,
        ),
        (
            RetirementErrorV1::PayerIsNeutralSink,
            RetirementErrorV2::PayerIsNeutralSink,
        ),
    ];
    for (frozen, successor) in conversions {
        assert_eq!(RetirementErrorV2::from(frozen), successor);
    }

    let child = AuthenticatedEpochChildV1 {
        epoch_generation: 1,
        kind: EpochChildKindV1::OrderPage,
        candidate_status: None,
    };
    assert_eq!(ChildSlotV1::Present(child), ChildSlotV1::Present(child));
}
