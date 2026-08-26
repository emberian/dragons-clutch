//! Registry-finalized admission for Product Runtime V2 records.
//!
//! The persisted receipt contains only finalized-record coordinates. It does
//! not copy Product facts. Core Found and Claims must independently authenticate
//! Registry ownership, raw/staging PDAs, exact hashes, rent, and staging vacancy,
//! then decode the referenced bytes again at their own trust boundary.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_product_runtime_v2::{
    ContentId, Error as ProductError, PortfolioV2, ProductJoinV2, ResultDomainV2, join_product_v2,
};

/// Product record schema label hashed into [`PRODUCT_RECORD_SCHEMA_ID_V2`].
pub const PRODUCT_RECORD_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/product-runtime-v2-product-record";
/// SHA-256 of [`PRODUCT_RECORD_SCHEMA_PREIMAGE_V2`].
pub const PRODUCT_RECORD_SCHEMA_ID_V2: [u8; 32] = [
    0xd9, 0xc3, 0x9f, 0xb6, 0x0c, 0x7d, 0xb7, 0x79, 0xa7, 0x84, 0x4d, 0xe7, 0x85, 0x05, 0x73, 0x8a,
    0x58, 0x99, 0x26, 0x4f, 0x86, 0x83, 0xdb, 0x4c, 0x6a, 0xe6, 0x1c, 0x9e, 0xf0, 0xe3, 0xcf, 0xf8,
];
/// Result-domain schema label hashed into [`RESULT_DOMAIN_SCHEMA_ID_V2`].
pub const RESULT_DOMAIN_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/product-runtime-v2-result-domain";
/// SHA-256 of [`RESULT_DOMAIN_SCHEMA_PREIMAGE_V2`].
pub const RESULT_DOMAIN_SCHEMA_ID_V2: [u8; 32] = [
    0x39, 0x9c, 0xc5, 0x74, 0x0f, 0x62, 0x1e, 0xa5, 0xc3, 0x0f, 0x96, 0x0a, 0x14, 0xaf, 0x83, 0x9b,
    0x0b, 0x5c, 0xfd, 0x58, 0xa9, 0x30, 0x5d, 0xcc, 0x09, 0xc6, 0x1f, 0xd1, 0x67, 0x81, 0xb7, 0xc2,
];
/// Portfolio schema label hashed into [`PORTFOLIO_SCHEMA_ID_V2`].
pub const PORTFOLIO_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/product-runtime-v2-portfolio";
/// SHA-256 of [`PORTFOLIO_SCHEMA_PREIMAGE_V2`].
pub const PORTFOLIO_SCHEMA_ID_V2: [u8; 32] = [
    0x76, 0x70, 0x6d, 0xdf, 0x08, 0x91, 0x7b, 0xb3, 0xdf, 0x08, 0x6b, 0x8c, 0x65, 0x04, 0x92, 0x83,
    0xbb, 0xab, 0x69, 0x75, 0x9c, 0x5b, 0x24, 0xb0, 0x75, 0x29, 0x7c, 0x47, 0x0f, 0xe3, 0xd6, 0x65,
];
/// Reference-only receipt schema label.
pub const ADMISSION_RECEIPT_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/product-runtime-v2-admission-receipt";
/// SHA-256 of [`ADMISSION_RECEIPT_SCHEMA_PREIMAGE_V2`].
pub const ADMISSION_RECEIPT_SCHEMA_ID_V2: [u8; 32] = [
    0xb7, 0x24, 0x54, 0x93, 0x39, 0x06, 0xb8, 0xb7, 0x7f, 0x0d, 0x48, 0xa8, 0xf3, 0x63, 0xf9, 0xd9,
    0x2b, 0xa1, 0xa2, 0x75, 0x34, 0x07, 0xff, 0xed, 0x39, 0x7e, 0x42, 0x00, 0x14, 0xae, 0xa4, 0x7b,
];

/// Exact fixed Product record width.
pub const PRODUCT_RECORD_BYTES_V2: usize = 112;
/// Exact reference-only admission receipt width.
pub const ADMISSION_RECEIPT_BYTES_V2: usize = 400;
/// Exact admission request width.
pub const ADMISSION_REQUEST_BYTES_V2: usize = 112;
/// Product record magic.
pub const PRODUCT_RECORD_MAGIC_V2: [u8; 8] = *b"DCLTPRM2";
/// Admission receipt magic.
pub const ADMISSION_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLTPRA2";
/// Admission request magic.
pub const ADMISSION_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLTPRQ2";
/// Shared admission wire version.
pub const ADMISSION_VERSION_V2: u16 = 2;
/// Number of finalized records in one complete admission.
pub const ADMISSION_RECORD_COUNT_V2: u8 = 3;
/// Admission-program PDA domain for one exact reference-only receipt.
pub const ADMISSION_RECEIPT_PDA_DOMAIN_V2: &[u8] = b"dclutch/product-v2/admission";

const PRODUCT_ID_OFFSET: usize = 16;
const PRODUCT_DOMAIN_DIGEST_OFFSET: usize = 48;
const PRODUCT_PORTFOLIO_DIGEST_OFFSET: usize = 80;
const RECEIPT_COUNT_OFFSET: usize = 10;
const RECEIPT_RECORDS_OFFSET: usize = 16;
const RECORD_COORDINATE_BYTES: usize = 128;
const REQUEST_PRODUCT_DIGEST_OFFSET: usize = 16;
const REQUEST_DOMAIN_DIGEST_OFFSET: usize = 48;
const REQUEST_PORTFOLIO_DIGEST_OFFSET: usize = 80;

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
        if array::<8>(bytes, 0)? != ADMISSION_REQUEST_MAGIC_V2
            || read_u16(bytes, 8)? != ADMISSION_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 10, 6)?;
        Ok(Self {
            product_digest: read_id(bytes, REQUEST_PRODUCT_DIGEST_OFFSET)?,
            result_domain_digest: read_id(bytes, REQUEST_DOMAIN_DIGEST_OFFSET)?,
            portfolio_digest: read_id(bytes, REQUEST_PORTFOLIO_DIGEST_OFFSET)?,
        })
    }

    /// Encode into one exact caller buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != ADMISSION_REQUEST_BYTES_V2 {
            return Err(Error::OutputLength);
        }
        output.fill(0);
        put(output, 0, &ADMISSION_REQUEST_MAGIC_V2)?;
        put(output, 8, &ADMISSION_VERSION_V2.to_le_bytes())?;
        put(
            output,
            REQUEST_PRODUCT_DIGEST_OFFSET,
            &self.product_digest.to_bytes(),
        )?;
        put(
            output,
            REQUEST_DOMAIN_DIGEST_OFFSET,
            &self.result_domain_digest.to_bytes(),
        )?;
        put(
            output,
            REQUEST_PORTFOLIO_DIGEST_OFFSET,
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
        if array::<8>(bytes, 0)? != PRODUCT_RECORD_MAGIC_V2
            || read_u16(bytes, 8)? != ADMISSION_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, 10, 6)?;
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
        put(output, 0, &PRODUCT_RECORD_MAGIC_V2)?;
        put(output, 8, &ADMISSION_VERSION_V2.to_le_bytes())?;
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
        if array::<8>(bytes, 0)? != ADMISSION_RECEIPT_MAGIC_V2
            || read_u16(bytes, 8)? != ADMISSION_VERSION_V2
        {
            return Err(Error::UnsupportedSchema);
        }
        if byte(bytes, RECEIPT_COUNT_OFFSET)? != ADMISSION_RECORD_COUNT_V2 {
            return Err(Error::NonCanonical);
        }
        require_zero(bytes, 11, 5)?;
        let product = decode_coordinate(bytes, RECEIPT_RECORDS_OFFSET)?;
        let result_domain =
            decode_coordinate(bytes, RECEIPT_RECORDS_OFFSET + RECORD_COORDINATE_BYTES)?;
        let portfolio =
            decode_coordinate(bytes, RECEIPT_RECORDS_OFFSET + 2 * RECORD_COORDINATE_BYTES)?;
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
        put(output, 0, &ADMISSION_RECEIPT_MAGIC_V2)?;
        put(output, 8, &ADMISSION_VERSION_V2.to_le_bytes())?;
        put(output, RECEIPT_COUNT_OFFSET, &[ADMISSION_RECORD_COUNT_V2])?;
        encode_coordinate(output, RECEIPT_RECORDS_OFFSET, self.product)?;
        encode_coordinate(
            output,
            RECEIPT_RECORDS_OFFSET + RECORD_COORDINATE_BYTES,
            self.result_domain,
        )?;
        encode_coordinate(
            output,
            RECEIPT_RECORDS_OFFSET + 2 * RECORD_COORDINATE_BYTES,
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
    if product.result_domain_digest != receipt.result_domain.content_digest
        || product.portfolio_digest != receipt.portfolio.content_digest
    {
        return Err(Error::ProductMismatch);
    }
    let domain = ResultDomainV2::decode(result_domain_bytes).map_err(map_product)?;
    let portfolio = PortfolioV2::decode(portfolio_bytes).map_err(map_product)?;
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
        schema_id: read_id(bytes, offset)?,
        content_digest: read_id(bytes, offset + 32)?,
        raw_account: read_id(bytes, offset + 64)?,
        staging_account: read_id(bytes, offset + 96)?,
    })
}

fn encode_coordinate(
    output: &mut [u8],
    offset: usize,
    coordinate: FinalizedRecordCoordinateV2,
) -> Result<()> {
    put(output, offset, &coordinate.schema_id.to_bytes())?;
    put(output, offset + 32, &coordinate.content_digest.to_bytes())?;
    put(output, offset + 64, &coordinate.raw_account.to_bytes())?;
    put(output, offset + 96, &coordinate.staging_account.to_bytes())?;
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
