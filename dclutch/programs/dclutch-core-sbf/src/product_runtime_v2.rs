//! The Core-side authentication of a Product Runtime V2 graph, at founding and
//! on every later route that re-reads the selected Product.

use dclutch_market::Product;
use dclutch_product::ContentId;
use dclutch_product::svm_reader::{
    AuthenticatedProductRuntimeV2, ProductRuntimeFrameV2,
    authenticate_content_addressed_product_runtime_v2,
    authenticate_product_runtime_v2 as authenticate_selected_product_runtime_v2,
};
use solana_program::pubkey::Pubkey;

use crate::{CoreSbfError, release::identity};

/// Authenticate a newly selected content-addressed Product graph. Child
/// identities come from the authenticated Product record, never instruction
/// fields or a receipt.
pub(crate) fn authenticate_product_runtime_v2(
    registry_program: &Pubkey,
    frame: ProductRuntimeFrameV2<'_, '_>,
) -> Result<AuthenticatedProductRuntimeV2, CoreSbfError> {
    authenticate_content_addressed_product_runtime_v2(registry_program, frame)
        .map_err(|_| CoreSbfError::Reference)
}

/// Reauthenticate the exact Product graph already persisted by a Core Market.
pub(crate) fn authenticate_selected_runtime_v2(
    registry_program: &Pubkey,
    expected_product_record: [u8; 32],
    frame: ProductRuntimeFrameV2<'_, '_>,
) -> Result<AuthenticatedProductRuntimeV2, CoreSbfError> {
    let expected = ContentId::new(expected_product_record).map_err(|_| CoreSbfError::Reference)?;
    authenticate_selected_product_runtime_v2(registry_program, expected, frame)
        .map_err(|_| CoreSbfError::Reference)
}

/// Project independently authenticated runtime facts into the Core semantic waist.
pub(crate) fn project_core_product_v2(
    runtime: AuthenticatedProductRuntimeV2,
) -> Result<Product, CoreSbfError> {
    Ok(Product {
        product_record: identity(runtime.product_record.content_digest.to_bytes())?,
        product_id: identity(runtime.product_id.to_bytes())?,
        result_domain: identity(runtime.result_domain_record.content_digest.to_bytes())?,
        portfolio: identity(runtime.portfolio_record.content_digest.to_bytes())?,
        coordinate_domain: identity(runtime.coordinate_domain_id.to_bytes())?,
        result_unit: identity(runtime.result_unit_id.to_bytes())?,
        claim_basis: identity(runtime.claim_basis_id.to_bytes())?,
        liability_basis: identity(runtime.liability_basis_id.to_bytes())?,
        representation_release: identity(runtime.representation_release_id.to_bytes())?,
        mapping_release: identity(runtime.mapping_release_id.to_bytes())?,
        outcome_count: runtime.outcome_count,
    })
}
