//! Resolution-owned subset-ledger initialization before Market creation.

use dclutch_market_core_codec::{PROJECT_FOUND_REQUEST_BYTES_V2, ProjectFoundRequestV2};
use dclutch_sha256_adapter::digestv;

use crate::Error;

/// Exact pre-Market initializer request magic.
pub const PRE_MARKET_FUNDING_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLRPFQ1";
/// Exact pre-Market initializer request width.
pub const PRE_MARKET_FUNDING_REQUEST_BYTES_V1: usize = 240;
/// Exact pre-Market initializer receipt magic.
pub const PRE_MARKET_FUNDING_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLRPFR1";
/// Exact pre-Market initializer receipt width.
pub const PRE_MARKET_FUNDING_RECEIPT_BYTES_V1: usize = 304;

const PRE_MARKET_FUNDING_PRESTATE_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch/resolution-ledger-prestate/v1";

const VERSION_V1: u16 = 1;
const PROJECT_FOUND_OFFSET: usize = 16;
const MANIFEST_OFFSET: usize = PROJECT_FOUND_OFFSET + PROJECT_FOUND_REQUEST_BYTES_V2;
const MASK_OFFSET: usize = MANIFEST_OFFSET + 32;
const FUNDING_SOURCE_OFFSET: usize = MASK_OFFSET + 8;
const LEDGER_OFFSET: usize = FUNDING_SOURCE_OFFSET + 32;
const PRESTATE_DIGEST_OFFSET: usize = LEDGER_OFFSET + 32;

/// Exact request for one Resolution-owned pre-Market subset ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreMarketFundingRequestV1 {
    /// Exact Core ProjectFound request that owns the future Market projection.
    pub project_found: ProjectFoundRequestV2,
    /// Finalized capability-manifest content identity.
    pub manifest: [u8; 32],
    /// Canonical Resolution-controller manifest-index subset.
    pub selected_mask: u16,
    /// Signer funding exact ledger Rent and native principal.
    pub funding_source: [u8; 32],
    /// Canonical Resolution-owned subset-ledger PDA.
    pub ledger: [u8; 32],
    /// Digest of the exact vacant System-account prestate.
    pub prestate_digest: [u8; 32],
}

impl PreMarketFundingRequestV1 {
    /// Validate one canonical request.
    pub fn validate(self) -> Result<Self, Error> {
        self.project_found
            .encode()
            .map_err(|_| Error::InvalidPreMarketFunding)?;
        if self.selected_mask.count_ones() != 3
            || [
                self.manifest,
                self.funding_source,
                self.ledger,
                self.prestate_digest,
            ]
            .iter()
            .any(|value| value.iter().all(|byte| *byte == 0))
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Ok(self)
    }

    /// Encode the sole canonical request bytes.
    pub fn encode(self) -> Result<[u8; PRE_MARKET_FUNDING_REQUEST_BYTES_V1], Error> {
        let value = self.validate()?;
        let mut output = [0_u8; PRE_MARKET_FUNDING_REQUEST_BYTES_V1];
        put(&mut output, 0, &PRE_MARKET_FUNDING_REQUEST_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        put(
            &mut output,
            PROJECT_FOUND_OFFSET,
            &value
                .project_found
                .encode()
                .map_err(|_| Error::InvalidPreMarketFunding)?,
        )?;
        put(&mut output, MANIFEST_OFFSET, &value.manifest)?;
        put(&mut output, MASK_OFFSET, &value.selected_mask.to_le_bytes())?;
        put(&mut output, FUNDING_SOURCE_OFFSET, &value.funding_source)?;
        put(&mut output, LEDGER_OFFSET, &value.ledger)?;
        put(&mut output, PRESTATE_DIGEST_OFFSET, &value.prestate_digest)?;
        Ok(output)
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            PRE_MARKET_FUNDING_REQUEST_BYTES_V1,
            &PRE_MARKET_FUNDING_REQUEST_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1
            || any_nonzero(input, 10, 6)?
            || any_nonzero(input, MASK_OFFSET + 2, 6)?
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Self {
            project_found: ProjectFoundRequestV2::decode(slice(
                input,
                PROJECT_FOUND_OFFSET,
                PROJECT_FOUND_REQUEST_BYTES_V2,
            )?)
            .map_err(|_| Error::InvalidPreMarketFunding)?,
            manifest: read_array(input, MANIFEST_OFFSET)?,
            selected_mask: read_u16(input, MASK_OFFSET)?,
            funding_source: read_array(input, FUNDING_SOURCE_OFFSET)?,
            ledger: read_array(input, LEDGER_OFFSET)?,
            prestate_digest: read_array(input, PRESTATE_DIGEST_OFFSET)?,
        }
        .validate()
    }
}

/// Immediate receipt for one initialized Resolution-owned subset ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreMarketFundingReceiptV1 {
    /// Core-authenticated future Market.
    pub market: [u8; 32],
    /// Core-authenticated Market generation.
    pub generation: u64,
    /// Finalized capability-manifest identity.
    pub manifest: [u8; 32],
    /// Canonical Resolution subset.
    pub selected_mask: u16,
    /// Resolution-owned ledger PDA.
    pub ledger: [u8; 32],
    /// Exact vacant prestate digest.
    pub prestate_digest: [u8; 32],
    /// SHA-256 of the exact initialized ledger bytes.
    pub poststate_digest: [u8; 32],
    /// Exact current ledger Rent reserve.
    pub exact_rent_lamports: u64,
    /// Exact aggregate native principal initially held.
    pub exact_native_principal: u64,
    /// SHA-256 of the exact embedded ordinary Core Found request.
    pub found_request_digest: [u8; 32],
    /// Exact signer that funded ledger Rent and native principal.
    pub funding_source: [u8; 32],
    /// Core-authenticated future Market RentCredit account.
    ///
    /// The future `CoreState.rent_beneficiary` is the sole semantic owner of
    /// this fact; the ledger header deliberately does not duplicate it.
    pub rent_credit: [u8; 32],
}

impl PreMarketFundingReceiptV1 {
    /// Encode the sole canonical receipt bytes.
    pub fn encode(self) -> Result<[u8; PRE_MARKET_FUNDING_RECEIPT_BYTES_V1], Error> {
        if self.generation == 0
            || self.selected_mask.count_ones() != 3
            || [
                self.market,
                self.manifest,
                self.ledger,
                self.prestate_digest,
                self.poststate_digest,
                self.found_request_digest,
                self.funding_source,
                self.rent_credit,
            ]
            .iter()
            .any(|value| value.iter().all(|byte| *byte == 0))
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        let mut output = [0_u8; PRE_MARKET_FUNDING_RECEIPT_BYTES_V1];
        put(&mut output, 0, &PRE_MARKET_FUNDING_RECEIPT_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        put(&mut output, 16, &self.market)?;
        put(&mut output, 48, &self.generation.to_le_bytes())?;
        put(&mut output, 56, &self.manifest)?;
        put(&mut output, 88, &self.selected_mask.to_le_bytes())?;
        put(&mut output, 96, &self.ledger)?;
        put(&mut output, 128, &self.prestate_digest)?;
        put(&mut output, 160, &self.poststate_digest)?;
        put(&mut output, 192, &self.exact_rent_lamports.to_le_bytes())?;
        put(&mut output, 200, &self.exact_native_principal.to_le_bytes())?;
        put(&mut output, 208, &self.found_request_digest)?;
        put(&mut output, 240, &self.funding_source)?;
        put(&mut output, 272, &self.rent_credit)?;
        Ok(output)
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            PRE_MARKET_FUNDING_RECEIPT_BYTES_V1,
            &PRE_MARKET_FUNDING_RECEIPT_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1
            || any_nonzero(input, 10, 6)?
            || any_nonzero(input, 90, 6)?
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        let value = Self {
            market: read_array(input, 16)?,
            generation: read_u64(input, 48)?,
            manifest: read_array(input, 56)?,
            selected_mask: read_u16(input, 88)?,
            ledger: read_array(input, 96)?,
            prestate_digest: read_array(input, 128)?,
            poststate_digest: read_array(input, 160)?,
            exact_rent_lamports: read_u64(input, 192)?,
            exact_native_principal: read_u64(input, 200)?,
            found_request_digest: read_array(input, 208)?,
            funding_source: read_array(input, 240)?,
            rent_credit: read_array(input, 272)?,
        };
        value.encode()?;
        Ok(value)
    }
}

/// Digest one exact prospective ledger-account prestate.
///
/// The caller supplies only the physical account facts read from the runtime.
/// This pure seam is shared by the Resolution adapter and the composing
/// Trading builder, so there is no second digest domain or preimage layout.
#[must_use]
pub fn pre_market_funding_prestate_digest_v1(
    key: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data_len: u64,
) -> [u8; 32] {
    let lamports = lamports.to_le_bytes();
    let data_len = data_len.to_le_bytes();
    digestv(&[
        PRE_MARKET_FUNDING_PRESTATE_DIGEST_DOMAIN_V1,
        &key,
        &owner,
        &lamports,
        &data_len,
    ])
}

fn exact_header(input: &[u8], width: usize, magic: &[u8; 8]) -> Result<(), Error> {
    if input.len() != width || input.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidPreMarketFunding);
    }
    Ok(())
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], Error> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(Error::InvalidPreMarketFunding)?,
        )
        .ok_or(Error::InvalidPreMarketFunding)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidPreMarketFunding)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn any_nonzero(input: &[u8], offset: usize, width: usize) -> Result<bool, Error> {
    Ok(slice(input, offset, width)?.iter().any(|byte| *byte != 0))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidPreMarketFunding)?,
        )
        .ok_or(Error::InvalidPreMarketFunding)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market_core_codec::{Action, Identity, Request};

    fn request() -> PreMarketFundingRequestV1 {
        PreMarketFundingRequestV1 {
            project_found: ProjectFoundRequestV2::new(Request::administrative(
                Action::Found,
                7,
                Identity::new([1; 32]).expect("market"),
            ))
            .expect("ProjectFound"),
            manifest: [2; 32],
            selected_mask: 0b111,
            funding_source: [3; 32],
            ledger: [4; 32],
            prestate_digest: [5; 32],
        }
    }

    #[test]
    fn request_is_exact_and_reserved_bytes_refuse() {
        let exact = request();
        let bytes = exact.encode().expect("request");
        assert_eq!(PreMarketFundingRequestV1::decode(&bytes), Ok(exact));
        let mut reserved = bytes;
        reserved[10] = 1;
        assert!(PreMarketFundingRequestV1::decode(&reserved).is_err());
        let mut partial_mask = exact;
        partial_mask.selected_mask = 0b11;
        assert!(partial_mask.encode().is_err());
    }

    #[test]
    fn receipt_roundtrips_exact_commitments() {
        let receipt = PreMarketFundingReceiptV1 {
            market: [1; 32],
            generation: 7,
            manifest: [2; 32],
            selected_mask: 0b111,
            ledger: [3; 32],
            prestate_digest: [4; 32],
            poststate_digest: [5; 32],
            exact_rent_lamports: 6,
            exact_native_principal: 7,
            found_request_digest: [8; 32],
            funding_source: [9; 32],
            rent_credit: [10; 32],
        };
        let bytes = receipt.encode().expect("receipt");
        assert_eq!(PreMarketFundingReceiptV1::decode(&bytes), Ok(receipt));
    }

    #[test]
    fn prestate_digest_has_one_exact_vector_and_binds_every_field() {
        let exact = pre_market_funding_prestate_digest_v1([1; 32], [2; 32], 3, 4);
        assert_eq!(
            exact,
            [
                0x60, 0xdd, 0x54, 0x52, 0x3d, 0x15, 0xc6, 0x10, 0x27, 0x21, 0x42, 0x67, 0xd8, 0xc0,
                0xed, 0xe7, 0x79, 0x1a, 0x2b, 0xa8, 0x6b, 0x7b, 0x40, 0x2b, 0x85, 0x1f, 0x1c, 0xba,
                0x1f, 0x95, 0x9b, 0x9e,
            ]
        );
        assert_ne!(
            pre_market_funding_prestate_digest_v1([9; 32], [2; 32], 3, 4),
            exact
        );
        assert_ne!(
            pre_market_funding_prestate_digest_v1([1; 32], [9; 32], 3, 4),
            exact
        );
        assert_ne!(
            pre_market_funding_prestate_digest_v1([1; 32], [2; 32], 9, 4),
            exact
        );
        assert_ne!(
            pre_market_funding_prestate_digest_v1([1; 32], [2; 32], 3, 9),
            exact
        );
    }
}
