//! Compile-time API-identity evidence for the narrow Resolution operator.

use dclutch_resolution_core_v3_operator as narrow;

#[test]
fn monolith_reexports_the_exact_narrow_types_and_builder() {
    let observation = dclutch_operator::Observation {
        slot: 7,
        unix_timestamp: 11,
        finality: dclutch_operator::Finality::Finalized,
    };
    let identical: narrow::Observation = observation;
    assert_eq!(identical.slot, 7);

    let monolith_builder: fn(
        &dclutch_operator::resolution_core_v3::ResolutionCloseFundSnapshotV3,
    ) -> Result<
        dclutch_operator::resolution_core_v3::ResolutionCloseFundReportV3,
        dclutch_operator::resolution_core_v3::ResolutionCoreOperatorErrorV3,
    > = dclutch_operator::resolution_core_v3::build_resolution_close_fund_v3;
    let _: fn(
        &narrow::ResolutionCloseFundSnapshotV3,
    ) -> Result<narrow::ResolutionCloseFundReportV3, narrow::ResolutionCoreOperatorErrorV3> =
        monolith_builder;

    assert_eq!(
        dclutch_operator::resolution_core_v3::RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3,
        narrow::RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3
    );
}
