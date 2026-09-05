//! Shared finalized Product-graph authentication for successor operators.

use crate::ObservedAccount;
use dclutch_product::ContentId;
use dclutch_product::admission::{
    AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

/// Product facts reauthenticated from one finalized Runtime V2 graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductGraphObservationV3 {
    /// Exact Product-owned outcome count, including explicit failure.
    pub outcome_count: u32,
    /// SHA-256 identity of the authenticated Product root record.
    pub product_record: [u8; 32],
    /// Stable Product identity joined by all three records.
    pub product_id: [u8; 32],
    /// ResultDomain content identity selected by the Product record.
    pub result_domain_id: [u8; 32],
    /// Semantic liability-basis identity selected by the Product graph.
    pub liability_basis_id: [u8; 32],
}

/// Same-snapshot finalized Product, ResultDomain, and Portfolio coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedProductGraphAccountsV3<'a> {
    /// Registry program owning every finalized raw record.
    pub registry_program: Pubkey,
    /// Finalized Product root bytes.
    pub product_raw: &'a ObservedAccount,
    /// Vacant Product staging cursor.
    pub product_staging: &'a ObservedAccount,
    /// Finalized ResultDomain bytes.
    pub domain_raw: &'a ObservedAccount,
    /// Vacant ResultDomain staging cursor.
    pub domain_staging: &'a ObservedAccount,
    /// Finalized Portfolio bytes.
    pub portfolio_raw: &'a ObservedAccount,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: &'a ObservedAccount,
}

/// Refusal from a malformed finalized coordinate or inconsistent Product graph.
///
/// The eight coordinate causes below were one `InvalidRecord` until 2026-09-05.
/// One code over eight disjuncts makes every test that names it a test of
/// "something about this coordinate was wrong", which is the bare `is_err()`
/// the refusal vocabulary forbids wearing a name: a fixture written to corrupt
/// the staging cursor passes just as green when it corrupts nothing and the
/// raw address is what refuses. Each disjunct is a different accusation --
/// wrong address, foreign owner, a program where a record should be, an empty
/// record, a cursor that is not vacant -- so each gets its own name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductGraphObservationErrorV3 {
    /// The raw record is not at the address its own content digest derives
    /// under the Registry program.
    RawRecordAddress,
    /// The raw record is not owned by the Registry program.
    RawRecordOwner,
    /// The raw record account is executable, so it is a program and not a
    /// finalized record.
    RawRecordExecutable,
    /// The raw record carries no bytes, so nothing is finalized at this
    /// coordinate.
    RawRecordEmpty,
    /// The staging cursor is not at the address this record's digest derives
    /// under the Registry program.
    StagingCursorAddress,
    /// The staging cursor is not system-owned, so this record's staging was
    /// never returned.
    StagingCursorOwner,
    /// The staging cursor account is executable.
    StagingCursorExecutable,
    /// The staging cursor still holds bytes, so this record is still being
    /// staged and is not finalized.
    StagingCursorNotVacant,
    /// A content identity this route derives is not a valid `ContentId`.
    ContentIdentity,
    /// `dclutch_product::admission` refused; the cause is its own.
    ProductRuntimeV2Admission(dclutch_product::admission::Error),
}

/// Reauthenticate one exact finalized Product graph without trusting a client DTO.
pub fn authenticate_product_graph_observation_v3(
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
    .map_err(ProductGraphObservationErrorV3::ProductRuntimeV2Admission)?;
    Ok(AuthenticatedProductGraphObservationV3 {
        outcome_count: admitted.join.outcome_count,
        product_record: admitted.product_record_digest.to_bytes(),
        product_id: admitted.join.product_id.to_bytes(),
        result_domain_id: admitted.join.result_domain_id.to_bytes(),
        liability_basis_id: admitted.join.liability_basis_id.to_bytes(),
    })
}

fn finalized_coordinate(
    registry: Pubkey,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    schema: [u8; 32],
) -> Result<FinalizedRecordCoordinateV2, ProductGraphObservationErrorV3> {
    let digest = ContentId::new(hash(&raw.data).to_bytes())
        .map_err(|_| ProductGraphObservationErrorV3::ContentIdentity)?;
    let (expected_raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest.to_bytes()],
        &registry,
    );
    let (expected_staging, _) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest.to_bytes()],
        &registry,
    );
    // ONE DISJUNCT, ONE NAME, in the order the conjunction was written.
    if raw.key != expected_raw {
        return Err(ProductGraphObservationErrorV3::RawRecordAddress);
    }
    if raw.owner != registry {
        return Err(ProductGraphObservationErrorV3::RawRecordOwner);
    }
    if raw.executable {
        return Err(ProductGraphObservationErrorV3::RawRecordExecutable);
    }
    if raw.data.is_empty() {
        return Err(ProductGraphObservationErrorV3::RawRecordEmpty);
    }
    if staging.key != expected_staging {
        return Err(ProductGraphObservationErrorV3::StagingCursorAddress);
    }
    if staging.owner != system_program::ID {
        return Err(ProductGraphObservationErrorV3::StagingCursorOwner);
    }
    if staging.executable {
        return Err(ProductGraphObservationErrorV3::StagingCursorExecutable);
    }
    if !staging.data.is_empty() {
        return Err(ProductGraphObservationErrorV3::StagingCursorNotVacant);
    }
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema)
            .map_err(|_| ProductGraphObservationErrorV3::ContentIdentity)?,
        content_digest: digest,
        raw_account: ContentId::new(expected_raw.to_bytes())
            .map_err(|_| ProductGraphObservationErrorV3::ContentIdentity)?,
        staging_account: ContentId::new(expected_staging.to_bytes())
            .map_err(|_| ProductGraphObservationErrorV3::ContentIdentity)?,
    })
}
