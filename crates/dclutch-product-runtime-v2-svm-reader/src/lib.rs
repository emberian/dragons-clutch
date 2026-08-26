//! Independent SVM authentication and decoding for Product Runtime V2 graphs.
//!
//! The exact Product-record digest is the graph root. The Product body selects
//! the domain and portfolio digests; neither an admission receipt nor a caller
//! may select those children independently. A receipt can be rechecked after
//! authentication as a coordinate cache, but it is never authority.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_product_runtime_v2::{ContentId, PortfolioV2, ResultDomainV2};
use dclutch_product_runtime_v2_admission::{
    AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2, RESULT_DOMAIN_SCHEMA_ID_V2,
    admit_authenticated_views_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::system_program;

/// Product Runtime V2 SVM-reader refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Raw/staging accounts aliased or carried forbidden privileges.
    AccountFrame,
    /// Product record owner, PDA, digest, rent, or staging vacancy refused.
    ProductRecord,
    /// Result-domain record owner, PDA, digest, rent, or staging vacancy refused.
    ResultDomainRecord,
    /// Portfolio record owner, PDA, digest, rent, or staging vacancy refused.
    PortfolioRecord,
    /// Product/domain/portfolio decoding or exact identity composition refused.
    Composition,
    /// Optional receipt coordinates differed from independently authenticated facts.
    ReceiptMismatch,
    /// Account data could not be borrowed.
    Borrow,
}

/// Reader result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// One read-only finalized raw/staging account pair.
#[derive(Clone, Copy)]
pub struct FinalizedRecordFrameV2<'accounts, 'info> {
    /// Registry-owned exact raw body.
    pub raw: &'accounts AccountInfo<'info>,
    /// System-owned vacant staging cursor PDA.
    pub staging: &'accounts AccountInfo<'info>,
}

/// Exact Product/domain/portfolio read-only account frame.
#[derive(Clone, Copy)]
pub struct ProductRuntimeFrameV2<'accounts, 'info> {
    /// Product graph-root record.
    pub product: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Product-selected result domain.
    pub result_domain: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Product-selected exact rational portfolio.
    pub portfolio: FinalizedRecordFrameV2<'accounts, 'info>,
}

/// One independently authenticated finalized coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRecordV2 {
    /// Canonical record schema.
    pub schema_id: ContentId,
    /// SHA-256 digest of the complete raw body.
    pub content_digest: ContentId,
    /// Canonical Registry raw PDA.
    pub raw_account: Pubkey,
    /// Canonical vacant staging PDA.
    pub staging_account: Pubkey,
}

impl AuthenticatedRecordV2 {
    /// Project the exact finalized coordinate after authentication. This is a
    /// reference cache, not a substitute for repeating authentication.
    pub fn coordinate(self) -> Result<FinalizedRecordCoordinateV2> {
        Ok(FinalizedRecordCoordinateV2 {
            schema_id: self.schema_id,
            content_digest: self.content_digest,
            raw_account: content(self.raw_account.to_bytes())?,
            staging_account: content(self.staging_account.to_bytes())?,
        })
    }
}

/// Ephemeral, independently authenticated Product Runtime V2 projection.
///
/// These fixed-size fields are copied from decoded authenticated raw bodies,
/// never from receipt bytes. Persisted Product facts remain owned by those raw
/// records and every consumer repeats authentication and decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductRuntimeV2 {
    /// Authenticated Product graph-root record.
    pub product_record: AuthenticatedRecordV2,
    /// Authenticated Product-selected domain record.
    pub result_domain_record: AuthenticatedRecordV2,
    /// Authenticated Product-selected portfolio record.
    pub portfolio_record: AuthenticatedRecordV2,
    /// Stable semantic Product identity inside the graph-root record.
    pub product_id: ContentId,
    /// Coordinate/statistic domain selected by Product.
    pub coordinate_domain_id: ContentId,
    /// Exact result unit selected by Product.
    pub result_unit_id: ContentId,
    /// Native claim basis selected by the portfolio.
    pub claim_basis_id: ContentId,
    /// Liability basis shared by domain and portfolio.
    pub liability_basis_id: ContentId,
    /// Representation semantic release shared by domain and portfolio.
    pub representation_release_id: ContentId,
    /// Coordinate mapping semantic release selected by the domain.
    pub mapping_release_id: ContentId,
    /// Runtime native outcome count including explicit failure.
    pub outcome_count: u32,
}

impl AuthenticatedProductRuntimeV2 {
    /// Recheck an optional admission receipt after all facts and coordinates
    /// have already been independently authenticated.
    pub fn recheck_reference_receipt(self, receipt_bytes: &[u8]) -> Result<()> {
        let receipt =
            AdmissionReceiptV2::decode(receipt_bytes).map_err(|_| Error::ReceiptMismatch)?;
        let expected = AdmissionReceiptV2 {
            product: self.product_record.coordinate()?,
            result_domain: self.result_domain_record.coordinate()?,
            portfolio: self.portfolio_record.coordinate()?,
        };
        if receipt != expected {
            return Err(Error::ReceiptMismatch);
        }
        Ok(())
    }
}

/// Derive the Product graph-root digest from the exact raw body, authenticate
/// the entire graph, and return its ephemeral projection. Core Found uses this
/// form because creation selects a new content-addressed Product graph.
pub fn authenticate_content_addressed_product_runtime_v2<'accounts, 'info>(
    registry_program: &Pubkey,
    rent: &Rent,
    frame: ProductRuntimeFrameV2<'accounts, 'info>,
) -> Result<AuthenticatedProductRuntimeV2> {
    let product_data = frame
        .product
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let digest = content(hash(&product_data).to_bytes())?;
    drop(product_data);
    authenticate_product_runtime_v2(registry_program, rent, digest, frame)
}

/// Authenticate and decode the exact Product graph already selected by a
/// Market or linked Claims record. The expected Product digest is the only
/// external graph authority; child identities come exclusively from the
/// authenticated Product record.
pub fn authenticate_product_runtime_v2<'accounts, 'info>(
    registry_program: &Pubkey,
    rent: &Rent,
    expected_product_digest: ContentId,
    frame: ProductRuntimeFrameV2<'accounts, 'info>,
) -> Result<AuthenticatedProductRuntimeV2> {
    require_distinct(frame)?;
    let product_record = authenticate_record(
        registry_program,
        rent,
        frame.product,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        expected_product_digest,
        Error::ProductRecord,
    )?;
    let product_data = frame
        .product
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let product = ProductRecordV2::decode(&product_data).map_err(|_| Error::Composition)?;
    let result_domain_record = authenticate_record(
        registry_program,
        rent,
        frame.result_domain,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product.result_domain_digest(),
        Error::ResultDomainRecord,
    )?;
    let portfolio_record = authenticate_record(
        registry_program,
        rent,
        frame.portfolio,
        PORTFOLIO_SCHEMA_ID_V2,
        product.portfolio_digest(),
        Error::PortfolioRecord,
    )?;
    let domain_data = frame
        .result_domain
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let portfolio_data = frame
        .portfolio
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let receipt = AdmissionReceiptV2 {
        product: product_record.coordinate()?,
        result_domain: result_domain_record.coordinate()?,
        portfolio: portfolio_record.coordinate()?,
    };
    let domain = ResultDomainV2::decode(&domain_data).map_err(|_| Error::Composition)?;
    let portfolio = PortfolioV2::decode(&portfolio_data).map_err(|_| Error::Composition)?;
    let projection = admit_authenticated_views_v2(receipt, product, domain, portfolio)
        .map_err(|_| Error::Composition)?;
    if projection.product_record_digest != product_record.content_digest
        || projection.portfolio_record_digest != portfolio_record.content_digest
        || projection.join.product_id != product.product_id()
        || projection.join.result_domain_id != result_domain_record.content_digest
        || projection.join.representation_id != portfolio_record.content_digest
    {
        return Err(Error::Composition);
    }
    Ok(AuthenticatedProductRuntimeV2 {
        product_record,
        result_domain_record,
        portfolio_record,
        product_id: projection.join.product_id,
        coordinate_domain_id: domain.coordinate_domain_id(),
        result_unit_id: domain.result_unit_id(),
        claim_basis_id: projection.join.claim_basis_id,
        liability_basis_id: projection.join.liability_basis_id,
        representation_release_id: portfolio.representation_release_id(),
        mapping_release_id: domain.mapping_release_id(),
        outcome_count: projection.join.outcome_count,
    })
}

fn authenticate_record(
    registry_program: &Pubkey,
    rent: &Rent,
    frame: FinalizedRecordFrameV2<'_, '_>,
    schema: [u8; 32],
    expected_digest: ContentId,
    refusal: Error,
) -> Result<AuthenticatedRecordV2> {
    let digest = expected_digest.to_bytes();
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        registry_program,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        registry_program,
    )
    .0;
    let raw_data = frame.raw.try_borrow_data().map_err(|_| Error::Borrow)?;
    if frame.raw.key != &expected_raw
        || frame.raw.owner != registry_program
        || frame.raw.is_signer
        || frame.raw.is_writable
        || frame.raw.executable
        || hash(&raw_data).to_bytes() != digest
        || !rent.is_exempt(frame.raw.lamports(), raw_data.len())
        || frame.staging.key != &expected_staging
        || frame.staging.owner != &system_program::ID
        || frame.staging.is_signer
        || frame.staging.is_writable
        || frame.staging.executable
        || frame.staging.data_len() != 0
    {
        return Err(refusal);
    }
    Ok(AuthenticatedRecordV2 {
        schema_id: content(schema)?,
        content_digest: expected_digest,
        raw_account: *frame.raw.key,
        staging_account: *frame.staging.key,
    })
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| Error::Composition)
}

fn require_distinct(frame: ProductRuntimeFrameV2<'_, '_>) -> Result<()> {
    let accounts = [
        frame.product.raw,
        frame.product.staging,
        frame.result_domain.raw,
        frame.result_domain.staging,
        frame.portfolio.raw,
        frame.portfolio.staging,
    ];
    for (left_index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left_index.saturating_add(1))
            .any(|right| left.key == right.key)
        {
            return Err(Error::AccountFrame);
        }
    }
    Ok(())
}
