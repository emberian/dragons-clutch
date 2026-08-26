//! Direct successor projections behind the sole Registry-selected Trading role.

/// Record-keyed Custody lifecycle for registered Buy liquidity.
pub mod buy_escrow;
/// Runtime-width complementary Custody request projection.
pub mod complementary;
/// Ordinary registered-fill Claims/Custody planning and receipt verification.
pub mod physical;

#[cfg(test)]
mod tests;
