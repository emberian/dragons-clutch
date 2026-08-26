//! Product Runtime V2 reader prepared for Claims admission consumers.
//!
//! This module deliberately has no dispatch. It will be exported only when
//! the current Claims tranche converges, then replaces the V1 Product Instance
//! decoder in LBV2 and protocol-position admission.

use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_svm_reader::{
    AuthenticatedProductRuntimeV2, ProductRuntimeFrameV2,
    authenticate_product_runtime_v2 as authenticate_graph,
};
use solana_program::{pubkey::Pubkey, rent::Rent};

use crate::ClaimsSbfError;

/// Independently authenticate a Market-selected Product graph and only then
/// cross-check the optional admission receipt as a non-authoritative cache.
pub(crate) fn authenticate_product_runtime_v2(
    registry_program: &Pubkey,
    rent: &Rent,
    expected_product_record_digest: [u8; 32],
    admission_receipt_bytes: &[u8],
    frame: ProductRuntimeFrameV2<'_, '_>,
) -> Result<AuthenticatedProductRuntimeV2, ClaimsSbfError> {
    let expected =
        ContentId::new(expected_product_record_digest).map_err(|_| ClaimsSbfError::Accounts)?;
    let authenticated = authenticate_graph(registry_program, rent, expected, frame)
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticated
        .recheck_reference_receipt(admission_receipt_bytes)
        .map_err(|_| ClaimsSbfError::Accounts)?;
    Ok(authenticated)
}
