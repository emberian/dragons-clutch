//! Registry-finalized admission for Product Runtime V2 records.
//!
//! The persisted receipt contains only finalized-record coordinates. It does
//! not copy Product facts. Core Found and Claims must independently authenticate
//! Registry ownership, raw/staging PDAs, exact hashes, rent, and staging vacancy,
//! then decode the referenced bytes again at their own trust boundary.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use crate::{
    ContentId, Error as ProductError, PortfolioV2, ProductJoinV2, ResultDomainV2, join_product_v2,
};




#[allow(missing_docs)]
#[rustfmt::skip]
mod generated_admission_v2;

// Four layouts, from `DClutch.ProductAdmissionV2Abi`. The crate wrote them as
// seventeen bare constants, six of which were not constants at all but
// `offset + 32`, `offset + 64` and `offset + 96` spelled twice inside
// `decode_coordinate` and `encode_coordinate`.
//
// The Product record and the admission request are ONE SHAPE, which is why
// there is no `REQUEST_*` set any more: `the_record_and_the_request_are_one_shape`
// says in Lean what six parallel declarations used to say by agreeing.
use generated_admission_v2::{
    ADMISSION_BODY_MAGIC_OFFSET_V2, ADMISSION_BODY_RESERVED_BYTES_V2,
    ADMISSION_BODY_RESERVED_OFFSET_V2, ADMISSION_BODY_VERSION_OFFSET_V2, ADMISSION_MAGIC_BYTES_V2,
    ADMISSION_RECEIPT_COUNT_OFFSET_V2, ADMISSION_RECEIPT_MAGIC_OFFSET_V2,
    ADMISSION_RECEIPT_PORTFOLIO_OFFSET_V2, ADMISSION_RECEIPT_RECORDS_OFFSET_V2,
    ADMISSION_RECEIPT_RESERVED_BYTES_V2, ADMISSION_RECEIPT_RESERVED_OFFSET_V2,
    ADMISSION_RECEIPT_RESULT_DOMAIN_OFFSET_V2, ADMISSION_RECEIPT_VERSION_OFFSET_V2,
    PRODUCT_DOMAIN_DIGEST_OFFSET, PRODUCT_ID_OFFSET, PRODUCT_PORTFOLIO_DIGEST_OFFSET,
    RECORD_COORDINATE_BYTES, RECORD_COORDINATE_CONTENT_DIGEST_OFFSET_V2,
    RECORD_COORDINATE_RAW_ACCOUNT_OFFSET_V2, RECORD_COORDINATE_SCHEMA_ID_OFFSET_V2,
    RECORD_COORDINATE_STAGING_ACCOUNT_OFFSET_V2,
};
pub use generated_admission_v2::{
    ADMISSION_RECEIPT_BYTES_V2, ADMISSION_RECEIPT_MAGIC_V2, ADMISSION_RECEIPT_PDA_DOMAIN_V2,
    ADMISSION_RECEIPT_SCHEMA_ID_V2, ADMISSION_RECEIPT_SCHEMA_PREIMAGE_V2,
    ADMISSION_RECORD_COUNT_V2, ADMISSION_REQUEST_BYTES_V2, ADMISSION_REQUEST_MAGIC_V2,
    ADMISSION_VERSION_V2, PORTFOLIO_SCHEMA_ID_V2, PORTFOLIO_SCHEMA_PREIMAGE_V2,
    PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_MAGIC_V2, PRODUCT_RECORD_PRODUCT_ID_OFFSET_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_PREIMAGE_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_PREIMAGE_V2,
};

// The three receipt coordinates are one stride apart. Lean states this as
// `the_three_coordinates_are_one_stride`; this is the independent check, and it
// is what goes red if a coordinate's placement and the declared stride ever
// part. Two authorities that must agree, neither derived from the other.
const _: () = assert!(
    ADMISSION_RECEIPT_RESULT_DOMAIN_OFFSET_V2
        == ADMISSION_RECEIPT_RECORDS_OFFSET_V2 + RECORD_COORDINATE_BYTES
);
const _: () = assert!(
    ADMISSION_RECEIPT_PORTFOLIO_OFFSET_V2
        == ADMISSION_RECEIPT_RECORDS_OFFSET_V2 + 2 * RECORD_COORDINATE_BYTES
);
const _: () = assert!(
    ADMISSION_RECEIPT_RECORDS_OFFSET_V2
        + (ADMISSION_RECORD_COUNT_V2 as usize) * RECORD_COORDINATE_BYTES
        == ADMISSION_RECEIPT_BYTES_V2
);

/// Product V2 admission refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An exact fixed-layout value had the wrong width.
    InvalidLength,
    /// Magic or schema version selected another protocol.
    UnsupportedSchema,
    /// Reserved bytes or record ordering were noncanonical.
    NonCanonical,
    /// A Product record identity or child digest differed.
    ProductMismatch,
    /// The runtime Product/domain/portfolio kernel refused.
    RuntimeProduct,
    /// A caller output buffer had the wrong exact width.
    OutputLength,
    /// Checked offset arithmetic overflowed.
    ArithmeticOverflow,
}

/// Admission result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Immutable expected content digests selecting the three finalized records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionRequestV2 {
    /// Expected Product record digest.
    pub product_digest: ContentId,
    /// Expected result-domain record digest.
    pub result_domain_digest: ContentId,
    /// Expected portfolio record digest.
    pub portfolio_digest: ContentId,
}

impl AdmissionRequestV2 {
    /// Decode one exact admission request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ADMISSION_REQUEST_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<ADMISSION_MAGIC_BYTES_V2>(bytes, ADMISSION_BODY_MAGIC_OFFSET_V2)?
            != ADMISSION_REQUEST_MAGIC_V2
            || read_u16(bytes, ADMISSION_BODY_VERSION_OFFSET_V2)? != ADMISSION_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            ADMISSION_BODY_RESERVED_OFFSET_V2,
            ADMISSION_BODY_RESERVED_BYTES_V2,
        )?;
        Ok(Self {
            product_digest: read_id(bytes, PRODUCT_ID_OFFSET)?,
            result_domain_digest: read_id(bytes, PRODUCT_DOMAIN_DIGEST_OFFSET)?,
            portfolio_digest: read_id(bytes, PRODUCT_PORTFOLIO_DIGEST_OFFSET)?,
        })
    }

    /// Encode into one exact caller buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != ADMISSION_REQUEST_BYTES_V2 {
            return Err(Error::OutputLength);
        }
        output.fill(0);
        put(
            output,
            ADMISSION_BODY_MAGIC_OFFSET_V2,
            &ADMISSION_REQUEST_MAGIC_V2,
        )?;
        put(
            output,
            ADMISSION_BODY_VERSION_OFFSET_V2,
            &ADMISSION_VERSION_V2.to_le_bytes(),
        )?;
        put(output, PRODUCT_ID_OFFSET, &self.product_digest.to_bytes())?;
        put(
            output,
            PRODUCT_DOMAIN_DIGEST_OFFSET,
            &self.result_domain_digest.to_bytes(),
        )?;
        put(
            output,
            PRODUCT_PORTFOLIO_DIGEST_OFFSET,
            &self.portfolio_digest.to_bytes(),
        )?;
        Ok(())
    }
}

/// Minimal Product record: one stable Product identity and two exact child
/// content digests. All other facts remain owned by the child records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductRecordV2 {
    product_id: ContentId,
    result_domain_digest: ContentId,
    portfolio_digest: ContentId,
}

impl ProductRecordV2 {
    /// Construct one exact Product→domain→portfolio binding.
    pub const fn new(
        product_id: ContentId,
        result_domain_digest: ContentId,
        portfolio_digest: ContentId,
    ) -> Self {
        Self {
            product_id,
            result_domain_digest,
            portfolio_digest,
        }
    }

    /// Hostile-decode one fixed Product record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PRODUCT_RECORD_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<ADMISSION_MAGIC_BYTES_V2>(bytes, ADMISSION_BODY_MAGIC_OFFSET_V2)?
            != PRODUCT_RECORD_MAGIC_V2
            || read_u16(bytes, ADMISSION_BODY_VERSION_OFFSET_V2)? != ADMISSION_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(
            bytes,
            ADMISSION_BODY_RESERVED_OFFSET_V2,
            ADMISSION_BODY_RESERVED_BYTES_V2,
        )?;
        Ok(Self {
            product_id: read_id(bytes, PRODUCT_ID_OFFSET)?,
            result_domain_digest: read_id(bytes, PRODUCT_DOMAIN_DIGEST_OFFSET)?,
            portfolio_digest: read_id(bytes, PRODUCT_PORTFOLIO_DIGEST_OFFSET)?,
        })
    }

    /// Encode into an exact caller buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != PRODUCT_RECORD_BYTES_V2 {
            return Err(Error::OutputLength);
        }
        output.fill(0);
        put(
            output,
            ADMISSION_BODY_MAGIC_OFFSET_V2,
            &PRODUCT_RECORD_MAGIC_V2,
        )?;
        put(
            output,
            ADMISSION_BODY_VERSION_OFFSET_V2,
            &ADMISSION_VERSION_V2.to_le_bytes(),
        )?;
        put(output, PRODUCT_ID_OFFSET, &self.product_id.to_bytes())?;
        put(
            output,
            PRODUCT_DOMAIN_DIGEST_OFFSET,
            &self.result_domain_digest.to_bytes(),
        )?;
        put(
            output,
            PRODUCT_PORTFOLIO_DIGEST_OFFSET,
            &self.portfolio_digest.to_bytes(),
        )?;
        Ok(())
    }

    /// Stable Product identity.
    pub const fn product_id(self) -> ContentId {
        self.product_id
    }
    /// Exact result-domain record digest.
    pub const fn result_domain_digest(self) -> ContentId {
        self.result_domain_digest
    }
    /// Exact portfolio record digest.
    pub const fn portfolio_digest(self) -> ContentId {
        self.portfolio_digest
    }
}

/// Registry-finalized record coordinates. These are references, not a claim
/// that finalization has already been checked by this pure crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedRecordCoordinateV2 {
    /// Exact schema/validator identity.
    pub schema_id: ContentId,
    /// SHA-256 digest of the entire semantic byte sequence.
    pub content_digest: ContentId,
    /// Derived Registry-owned raw account.
    pub raw_account: ContentId,
    /// Derived vacant staging account.
    pub staging_account: ContentId,
}

/// Reference-only receipt containing Product, domain, and portfolio record
/// coordinates in canonical order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionReceiptV2 {
    /// Product record coordinate.
    pub product: FinalizedRecordCoordinateV2,
    /// Result-domain record coordinate.
    pub result_domain: FinalizedRecordCoordinateV2,
    /// Portfolio record coordinate.
    pub portfolio: FinalizedRecordCoordinateV2,
}

impl AdmissionReceiptV2 {
    /// Decode and validate canonical record ordering and schema identities.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ADMISSION_RECEIPT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<ADMISSION_MAGIC_BYTES_V2>(bytes, ADMISSION_RECEIPT_MAGIC_OFFSET_V2)?
            != ADMISSION_RECEIPT_MAGIC_V2
            || read_u16(bytes, ADMISSION_RECEIPT_VERSION_OFFSET_V2)? != ADMISSION_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        if byte(bytes, ADMISSION_RECEIPT_COUNT_OFFSET_V2)? != ADMISSION_RECORD_COUNT_V2 {
            return Err(Error::NonCanonical);
        }
        require_zero(
            bytes,
            ADMISSION_RECEIPT_RESERVED_OFFSET_V2,
            ADMISSION_RECEIPT_RESERVED_BYTES_V2,
        )?;
        let product = decode_coordinate(bytes, ADMISSION_RECEIPT_RECORDS_OFFSET_V2)?;
        let result_domain = decode_coordinate(bytes, ADMISSION_RECEIPT_RESULT_DOMAIN_OFFSET_V2)?;
        let portfolio = decode_coordinate(bytes, ADMISSION_RECEIPT_PORTFOLIO_OFFSET_V2)?;
        if product.schema_id.to_bytes() != PRODUCT_RECORD_SCHEMA_ID_V2
            || result_domain.schema_id.to_bytes() != RESULT_DOMAIN_SCHEMA_ID_V2
            || portfolio.schema_id.to_bytes() != PORTFOLIO_SCHEMA_ID_V2
        {
            return Err(Error::NonCanonical);
        }
        Ok(Self {
            product,
            result_domain,
            portfolio,
        })
    }

    /// Encode the reference-only receipt into an exact caller buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        validate_coordinate_schema(self.product, PRODUCT_RECORD_SCHEMA_ID_V2)?;
        validate_coordinate_schema(self.result_domain, RESULT_DOMAIN_SCHEMA_ID_V2)?;
        validate_coordinate_schema(self.portfolio, PORTFOLIO_SCHEMA_ID_V2)?;
        if output.len() != ADMISSION_RECEIPT_BYTES_V2 {
            return Err(Error::OutputLength);
        }
        output.fill(0);
        put(
            output,
            ADMISSION_RECEIPT_MAGIC_OFFSET_V2,
            &ADMISSION_RECEIPT_MAGIC_V2,
        )?;
        put(
            output,
            ADMISSION_RECEIPT_VERSION_OFFSET_V2,
            &ADMISSION_VERSION_V2.to_le_bytes(),
        )?;
        put(
            output,
            ADMISSION_RECEIPT_COUNT_OFFSET_V2,
            &[ADMISSION_RECORD_COUNT_V2],
        )?;
        encode_coordinate(output, ADMISSION_RECEIPT_RECORDS_OFFSET_V2, self.product)?;
        encode_coordinate(
            output,
            ADMISSION_RECEIPT_RESULT_DOMAIN_OFFSET_V2,
            self.result_domain,
        )?;
        encode_coordinate(
            output,
            ADMISSION_RECEIPT_PORTFOLIO_OFFSET_V2,
            self.portfolio,
        )?;
        Ok(())
    }
}

/// Ephemeral decoded admission projection. This is never a persisted second
/// truth; consumers independently recreate it from authenticated raw records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionProjectionV2 {
    /// Exact Product/domain/basis/representation identity join.
    pub join: ProductJoinV2,
    /// Product record digest, distinct from stable Product identity.
    pub product_record_digest: ContentId,
    /// Portfolio record digest, distinct from semantic release/basis IDs.
    pub portfolio_record_digest: ContentId,
}

/// Decode and join three already Registry-authenticated exact record bodies.
///
/// This pure step does not trust the receipt as finalization evidence. The
/// caller supplies the three coordinates whose owner/PDA/hash/rent/vacancy
/// checks it performed at the adapter boundary.
pub fn admit_authenticated_records_v2(
    receipt: AdmissionReceiptV2,
    product_bytes: &[u8],
    result_domain_bytes: &[u8],
    portfolio_bytes: &[u8],
) -> Result<AdmissionProjectionV2> {
    let product = ProductRecordV2::decode(product_bytes)?;
    let domain = ResultDomainV2::decode(result_domain_bytes).map_err(map_product)?;
    let portfolio = PortfolioV2::decode(portfolio_bytes).map_err(map_product)?;
    admit_authenticated_views_v2(receipt, product, domain, portfolio)
}

/// Join already decoded views after an adapter independently authenticated
/// their exact raw bodies and coordinates. Long runtime tails are therefore
/// decoded once without changing composition authority.
pub fn admit_authenticated_views_v2(
    receipt: AdmissionReceiptV2,
    product: ProductRecordV2,
    domain: ResultDomainV2<'_>,
    portfolio: PortfolioV2<'_>,
) -> Result<AdmissionProjectionV2> {
    if product.result_domain_digest != receipt.result_domain.content_digest
        || product.portfolio_digest != receipt.portfolio.content_digest
    {
        return Err(Error::ProductMismatch);
    }
    if domain.product_id() != product.product_id {
        return Err(Error::ProductMismatch);
    }
    let join = join_product_v2(
        receipt.result_domain.content_digest,
        receipt.portfolio.content_digest,
        domain,
        portfolio,
    )
    .map_err(map_product)?;
    Ok(AdmissionProjectionV2 {
        join,
        product_record_digest: receipt.product.content_digest,
        portfolio_record_digest: receipt.portfolio.content_digest,
    })
}

fn map_product(_: ProductError) -> Error {
    Error::RuntimeProduct
}

fn validate_coordinate_schema(
    coordinate: FinalizedRecordCoordinateV2,
    expected: [u8; 32],
) -> Result<()> {
    if coordinate.schema_id.to_bytes() != expected {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

fn decode_coordinate(bytes: &[u8], offset: usize) -> Result<FinalizedRecordCoordinateV2> {
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: read_id(bytes, offset + RECORD_COORDINATE_SCHEMA_ID_OFFSET_V2)?,
        content_digest: read_id(bytes, offset + RECORD_COORDINATE_CONTENT_DIGEST_OFFSET_V2)?,
        raw_account: read_id(bytes, offset + RECORD_COORDINATE_RAW_ACCOUNT_OFFSET_V2)?,
        staging_account: read_id(bytes, offset + RECORD_COORDINATE_STAGING_ACCOUNT_OFFSET_V2)?,
    })
}

fn encode_coordinate(
    output: &mut [u8],
    offset: usize,
    coordinate: FinalizedRecordCoordinateV2,
) -> Result<()> {
    put(
        output,
        offset + RECORD_COORDINATE_SCHEMA_ID_OFFSET_V2,
        &coordinate.schema_id.to_bytes(),
    )?;
    put(
        output,
        offset + RECORD_COORDINATE_CONTENT_DIGEST_OFFSET_V2,
        &coordinate.content_digest.to_bytes(),
    )?;
    put(
        output,
        offset + RECORD_COORDINATE_RAW_ACCOUNT_OFFSET_V2,
        &coordinate.raw_account.to_bytes(),
    )?;
    put(
        output,
        offset + RECORD_COORDINATE_STAGING_ACCOUNT_OFFSET_V2,
        &coordinate.staging_account.to_bytes(),
    )?;
    Ok(())
}

fn read_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?).map_err(map_product)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::OutputLength)?
        .copy_from_slice(value);
    Ok(())
}

fn require_zero(bytes: &[u8], offset: usize, length: usize) -> Result<()> {
    let end = offset
        .checked_add(length)
        .ok_or(Error::ArithmeticOverflow)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonical);
    }
    Ok(())
}
