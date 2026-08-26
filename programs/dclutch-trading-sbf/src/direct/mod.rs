//! Direct successor projections behind the sole Registry-selected Trading role.

/// Record-keyed Custody lifecycle for registered Buy liquidity.
pub mod buy_escrow;
/// Runtime-width complementary Custody request projection.
pub mod complementary;
/// Commit-last ordinary Direct state candidates and shared physical facts.
pub mod physical;
/// Claims-owned record Position lifecycle for registered Sell liquidity.
pub mod sell_escrow;

#[cfg(test)]
mod tests;
