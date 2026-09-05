//! The Claims-side authentication of a Market-selected Product Runtime V2 graph.

use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_svm_reader::{
    AuthenticatedProductRuntimeV2, ProductRuntimeFrameV2,
    authenticate_product_runtime_v2 as authenticate_graph,
};
use solana_program::pubkey::Pubkey;

use crate::ClaimsSbfError;

/// Independently authenticate a Market-selected Product graph.
pub(crate) fn authenticate_product_runtime_v2(
    registry_program: &Pubkey,
    expected_product_record_digest: [u8; 32],
    frame: ProductRuntimeFrameV2<'_, '_>,
) -> Result<AuthenticatedProductRuntimeV2, ClaimsSbfError> {
    let expected =
        ContentId::new(expected_product_record_digest).map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_graph(registry_program, expected, frame).map_err(|_| ClaimsSbfError::Accounts)
}
