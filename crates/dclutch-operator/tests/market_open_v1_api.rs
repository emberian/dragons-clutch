//! Compile-time identity of the monolith re-export and narrow market-open API.

use dclutch_market_open_v1_operator::{
    RegistryOpenMarketContinuationErrorV1, RegistryOpenMarketContinuationReportV1,
    RegistryOpenMarketContinuationStateV1,
};
use solana_program::instruction::Instruction;

type Builder =
    fn(
        &RegistryOpenMarketContinuationStateV1,
        &Instruction,
    )
        -> Result<RegistryOpenMarketContinuationReportV1, RegistryOpenMarketContinuationErrorV1>;

#[test]
fn monolith_reexport_is_the_narrow_builder() {
    let narrow: Builder =
        dclutch_market_open_v1_operator::build_registry_open_market_continuation_v1;
    let monolith: Builder = dclutch_operator::registry::open_market_continuation_v1::build_registry_open_market_continuation_v1;
    let _same_public_function_type = (narrow, monolith);
}
