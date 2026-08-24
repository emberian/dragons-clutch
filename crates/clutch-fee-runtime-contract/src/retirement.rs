//! Exact hash boundary for durable streaming fee-retirement receipts.

use crate::projection::SelectedOwnerFeeBookHashV1;

/// Commitment domain for the accumulator's terminal authority receipt.
pub const FEE_RETIREMENT_AUTHORITY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fee-retirement-authority/v1\0";

/// Minimal exact SHA-256 seam shared by fee retirement and its terminal pair.
pub trait FeeRetirementHashV1: SelectedOwnerFeeBookHashV1 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32];
}
