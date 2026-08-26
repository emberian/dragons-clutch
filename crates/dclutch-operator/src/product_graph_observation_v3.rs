//! Shared finalized Product-graph authentication for successor operators.

use crate::ObservedAccount;
use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_admission::{
    AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductGraphObservationV3 {
    pub(crate) outcome_count: u32,
    pub(crate) product_record: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FinalizedProductGraphAccountsV3<'a> {
    pub(crate) registry_program: Pubkey,
    pub(crate) product_raw: &'a ObservedAccount,
    pub(crate) product_staging: &'a ObservedAccount,
    pub(crate) domain_raw: &'a ObservedAccount,
    pub(crate) domain_staging: &'a ObservedAccount,
    pub(crate) portfolio_raw: &'a ObservedAccount,
    pub(crate) portfolio_staging: &'a ObservedAccount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductGraphObservationErrorV3 {
    InvalidRecord,
    InvalidGraph,
}

pub(crate) fn authenticate_product_graph_observation_v3(
    accounts: FinalizedProductGraphAccountsV3<'_>,
) -> Result<AuthenticatedProductGraphObservationV3, ProductGraphObservationErrorV3> {
    let product = finalized_coordinate(
        accounts.registry_program,
        accounts.product_raw,
        accounts.product_staging,
        PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let domain = finalized_coordinate(
        accounts.registry_program,
        accounts.domain_raw,
        accounts.domain_staging,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio = finalized_coordinate(
        accounts.registry_program,
        accounts.portfolio_raw,
        accounts.portfolio_staging,
        PORTFOLIO_SCHEMA_ID_V2,
    )?;
    let admitted = admit_authenticated_records_v2(
        AdmissionReceiptV2 {
            product,
            result_domain: domain,
            portfolio,
        },
        &accounts.product_raw.data,
        &accounts.domain_raw.data,
        &accounts.portfolio_raw.data,
    )
    .map_err(|_| ProductGraphObservationErrorV3::InvalidGraph)?;
    Ok(AuthenticatedProductGraphObservationV3 {
        outcome_count: admitted.join.outcome_count,
        product_record: admitted.product_record_digest.to_bytes(),
    })
}

fn finalized_coordinate(
    registry: Pubkey,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    schema: [u8; 32],
) -> Result<FinalizedRecordCoordinateV2, ProductGraphObservationErrorV3> {
    let digest = ContentId::new(hash(&raw.data).to_bytes())
        .map_err(|_| ProductGraphObservationErrorV3::InvalidRecord)?;
    let (expected_raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest.to_bytes()],
        &registry,
    );
    let (expected_staging, _) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest.to_bytes()],
        &registry,
    );
    if raw.key != expected_raw
        || raw.owner != registry
        || raw.executable
        || raw.data.is_empty()
        || staging.key != expected_staging
        || staging.owner != system_program::ID
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(ProductGraphObservationErrorV3::InvalidRecord);
    }
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema)
            .map_err(|_| ProductGraphObservationErrorV3::InvalidRecord)?,
        content_digest: digest,
        raw_account: ContentId::new(expected_raw.to_bytes())
            .map_err(|_| ProductGraphObservationErrorV3::InvalidRecord)?,
        staging_account: ContentId::new(expected_staging.to_bytes())
            .map_err(|_| ProductGraphObservationErrorV3::InvalidRecord)?,
    })
}
