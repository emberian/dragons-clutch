//! Product Runtime V2 reader prepared for canonical Core Found replacement.
//!
//! This module deliberately has no parallel dispatch. It is exported only in
//! the coordinated Found ABI migration that deletes the V1 fixed-domain path.

use dclutch_product_runtime_v2_svm_reader::{
    AuthenticatedProductRuntimeV2, ProductRuntimeFrameV2,
    authenticate_content_addressed_product_runtime_v2,
};
use solana_program::{pubkey::Pubkey, rent::Rent};

use crate::CoreSbfError;

/// Authenticate a newly selected content-addressed Product graph. Child
/// identities come from the authenticated Product record, never instruction
/// fields or a receipt.
pub(crate) fn authenticate_product_runtime_v2(
    registry_program: &Pubkey,
    rent: &Rent,
    frame: ProductRuntimeFrameV2<'_, '_>,
) -> Result<AuthenticatedProductRuntimeV2, CoreSbfError> {
    authenticate_content_addressed_product_runtime_v2(registry_program, rent, frame)
        .map_err(|_| CoreSbfError::Reference)
}
