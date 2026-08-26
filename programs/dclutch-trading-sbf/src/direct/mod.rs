//! Direct successor projections behind the sole Registry-selected Trading role.

/// Record-keyed Custody lifecycle for registered Buy liquidity.
pub mod buy_escrow;
/// Runtime-width complementary Custody request projection.
pub mod complementary;
/// Existing-root inline ordinary Claims/Custody projection.
pub mod inline;
/// Generic Trading-owned lifecycle joins for Direct root, maker, and record state.
pub mod lifecycle;
/// Commit-last ordinary Direct state candidates and shared physical facts.
pub mod physical;
/// Claims-owned record Position lifecycle for registered Sell liquidity.
pub mod sell_escrow;

#[cfg(test)]
mod tests;
