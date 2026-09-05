//! Resolution-owned subset-ledger initialization before Market creation.

use dclutch_market::{PROJECT_FOUND_REQUEST_BYTES_V2, ProjectFoundRequestV2};
use dclutch_sha256_adapter::digestv;

use crate::resolution::Error;

/// Exact pre-Market initializer request magic.
pub const PRE_MARKET_FUNDING_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLRPFQ1";
/// Exact pre-Market initializer request width.
pub const PRE_MARKET_FUNDING_REQUEST_BYTES_V1: usize = 240;
/// Exact dust-tolerant pre-Market initializer request magic.
pub const PRE_MARKET_FUNDING_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLRPFQ2";
/// Exact dust-tolerant pre-Market initializer request width.
pub const PRE_MARKET_FUNDING_REQUEST_BYTES_V2: usize = 272;
/// Exact pre-Market initializer receipt magic.
pub const PRE_MARKET_FUNDING_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLRPFR1";
/// Exact pre-Market initializer receipt width.
pub const PRE_MARKET_FUNDING_RECEIPT_BYTES_V1: usize = 304;
/// Exact dust-tolerant pre-Market initializer receipt magic.
pub const PRE_MARKET_FUNDING_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLRPFR2";
/// Exact dust-tolerant pre-Market initializer receipt width.
pub const PRE_MARKET_FUNDING_RECEIPT_BYTES_V2: usize = 368;

const PRE_MARKET_FUNDING_PRESTATE_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch/resolution-ledger-prestate/v1";

const VERSION_V1: u16 = 1;
const VERSION_V2: u16 = 2;
const PROJECT_FOUND_OFFSET: usize = 16;
const MANIFEST_OFFSET: usize = PROJECT_FOUND_OFFSET + PROJECT_FOUND_REQUEST_BYTES_V2;
const MASK_OFFSET: usize = MANIFEST_OFFSET + 32;
const FUNDING_SOURCE_OFFSET: usize = MASK_OFFSET + 8;
const LEDGER_OFFSET: usize = FUNDING_SOURCE_OFFSET + 32;
const PRESTATE_DIGEST_OFFSET: usize = LEDGER_OFFSET + 32;
const PROJECT_FOUND_RECEIPT_DIGEST_OFFSET_V2: usize = PRESTATE_DIGEST_OFFSET + 32;

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

/// Exact request for one dust-tolerant Resolution-owned pre-Market subset ledger.
///
/// The physical prestate digest binds any harmless System-owned lamport dust.
/// V2 never accepts V1 wire bytes, so callers cannot silently receive the
/// wider dust-accounting receipt under the old action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreMarketFundingRequestV2 {
    /// Exact Core ProjectFound request that owns the future Market projection.
    pub project_found: ProjectFoundRequestV2,
    /// Finalized capability-manifest content identity.
    pub manifest: [u8; 32],
    /// Canonical Resolution-controller manifest-index subset.
    pub selected_mask: u16,
    /// Signer funding only the exact ledger shortfall.
    pub funding_source: [u8; 32],
    /// Canonical Resolution-owned subset-ledger PDA.
    pub ledger: [u8; 32],
    /// Digest of the exact System-owned, zero-data prestate, including dust.
    pub prestate_digest: [u8; 32],
    /// Expected digest of Core's exact immediate ProjectFound receipt bytes.
    pub expected_project_found_receipt_digest: [u8; 32],
}

impl PreMarketFundingRequestV2 {
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
                self.expected_project_found_receipt_digest,
            ]
            .iter()
            .any(|value| value.iter().all(|byte| *byte == 0))
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Ok(self)
    }

    /// Encode the sole canonical V2 request bytes.
    pub fn encode(self) -> Result<[u8; PRE_MARKET_FUNDING_REQUEST_BYTES_V2], Error> {
        let value = self.validate()?;
        let mut output = [0_u8; PRE_MARKET_FUNDING_REQUEST_BYTES_V2];
        put(&mut output, 0, &PRE_MARKET_FUNDING_REQUEST_MAGIC_V2)?;
        put(&mut output, 8, &VERSION_V2.to_le_bytes())?;
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
        put(
            &mut output,
            PROJECT_FOUND_RECEIPT_DIGEST_OFFSET_V2,
            &value.expected_project_found_receipt_digest,
        )?;
        Ok(output)
    }

    /// Hostile-decode one exact V2 request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            PRE_MARKET_FUNDING_REQUEST_BYTES_V2,
            &PRE_MARKET_FUNDING_REQUEST_MAGIC_V2,
        )?;
        if read_u16(input, 8)? != VERSION_V2
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
            expected_project_found_receipt_digest: read_array(
                input,
                PROJECT_FOUND_RECEIPT_DIGEST_OFFSET_V2,
            )?,
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

/// Immediate receipt for one dust-tolerant initialized Resolution subset ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreMarketFundingReceiptV2 {
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
    /// Exact System-owned, zero-data prestate digest, including dust.
    pub prestate_digest: [u8; 32],
    /// SHA-256 of the exact initialized ledger bytes.
    pub poststate_digest: [u8; 32],
    /// Exact current ledger Rent reserve.
    pub exact_rent_lamports: u64,
    /// Exact aggregate native principal initially held.
    pub exact_native_principal: u64,
    /// SHA-256 of the exact embedded ordinary Core Found request.
    pub found_request_digest: [u8; 32],
    /// Exact signer funding only the ledger shortfall.
    pub funding_source: [u8; 32],
    /// Core-authenticated future Market RentCredit and sole dust-refund account.
    pub rent_credit: [u8; 32],
    /// Digest of Core's exact immediate ProjectFound receipt bytes.
    pub project_found_receipt_digest: [u8; 32],
    /// Lamports observed on the System-owned zero-data ledger before mutation.
    pub observed_dust_lamports: u64,
    /// Exact shortfall transferred from `funding_source`.
    pub top_up_lamports: u64,
    /// Exact excess dust returned to `rent_credit`.
    pub refund_lamports: u64,
    /// Exact lamports held by the initialized Resolution ledger.
    pub exact_post_lamports: u64,
}

impl PreMarketFundingReceiptV2 {
    fn validate(self) -> Result<Self, Error> {
        if self.generation == 0
            || self.selected_mask.count_ones() != 3
            || self.exact_rent_lamports == 0
            || [
                self.market,
                self.manifest,
                self.ledger,
                self.prestate_digest,
                self.poststate_digest,
                self.found_request_digest,
                self.funding_source,
                self.rent_credit,
                self.project_found_receipt_digest,
            ]
            .iter()
            .any(|value| value.iter().all(|byte| *byte == 0))
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        let target = self
            .exact_rent_lamports
            .checked_add(self.exact_native_principal)
            .ok_or(Error::InvalidPreMarketFunding)?;
        let (top_up, refund) = if self.observed_dust_lamports < target {
            (target - self.observed_dust_lamports, 0)
        } else {
            (0, self.observed_dust_lamports - target)
        };
        if self.exact_post_lamports != target
            || self.top_up_lamports != top_up
            || self.refund_lamports != refund
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Ok(self)
    }

    /// Encode the sole canonical V2 receipt bytes.
    pub fn encode(self) -> Result<[u8; PRE_MARKET_FUNDING_RECEIPT_BYTES_V2], Error> {
        let value = self.validate()?;
        let mut output = [0_u8; PRE_MARKET_FUNDING_RECEIPT_BYTES_V2];
        put(&mut output, 0, &PRE_MARKET_FUNDING_RECEIPT_MAGIC_V2)?;
        put(&mut output, 8, &VERSION_V2.to_le_bytes())?;
        put(&mut output, 16, &value.market)?;
        put(&mut output, 48, &value.generation.to_le_bytes())?;
        put(&mut output, 56, &value.manifest)?;
        put(&mut output, 88, &value.selected_mask.to_le_bytes())?;
        put(&mut output, 96, &value.ledger)?;
        put(&mut output, 128, &value.prestate_digest)?;
        put(&mut output, 160, &value.poststate_digest)?;
        put(&mut output, 192, &value.exact_rent_lamports.to_le_bytes())?;
        put(
            &mut output,
            200,
            &value.exact_native_principal.to_le_bytes(),
        )?;
        put(&mut output, 208, &value.found_request_digest)?;
        put(&mut output, 240, &value.funding_source)?;
        put(&mut output, 272, &value.rent_credit)?;
        put(&mut output, 304, &value.project_found_receipt_digest)?;
        put(
            &mut output,
            336,
            &value.observed_dust_lamports.to_le_bytes(),
        )?;
        put(&mut output, 344, &value.top_up_lamports.to_le_bytes())?;
        put(&mut output, 352, &value.refund_lamports.to_le_bytes())?;
        put(&mut output, 360, &value.exact_post_lamports.to_le_bytes())?;
        Ok(output)
    }

    /// Hostile-decode one exact V2 receipt.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            PRE_MARKET_FUNDING_RECEIPT_BYTES_V2,
            &PRE_MARKET_FUNDING_RECEIPT_MAGIC_V2,
        )?;
        if read_u16(input, 8)? != VERSION_V2
            || any_nonzero(input, 10, 6)?
            || any_nonzero(input, 90, 6)?
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Self {
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
            project_found_receipt_digest: read_array(input, 304)?,
            observed_dust_lamports: read_u64(input, 336)?,
            top_up_lamports: read_u64(input, 344)?,
            refund_lamports: read_u64(input, 352)?,
            exact_post_lamports: read_u64(input, 360)?,
        }
        .validate()
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
    use dclutch_market::{Action, Identity, Request};

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

    fn request_v2() -> PreMarketFundingRequestV2 {
        let legacy = request();
        PreMarketFundingRequestV2 {
            project_found: legacy.project_found,
            manifest: legacy.manifest,
            selected_mask: legacy.selected_mask,
            funding_source: legacy.funding_source,
            ledger: legacy.ledger,
            prestate_digest: legacy.prestate_digest,
            expected_project_found_receipt_digest: [6; 32],
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
    fn v2_request_binds_project_found_receipt_and_refuses_v1_bytes() {
        let exact = request_v2();
        let bytes = exact.encode().expect("V2 request");
        assert_eq!(PreMarketFundingRequestV2::decode(&bytes), Ok(exact));
        assert!(
            PreMarketFundingRequestV2::decode(&request().encode().expect("legacy request"))
                .is_err()
        );
        let mut omitted = exact;
        omitted.expected_project_found_receipt_digest = [0; 32];
        assert!(omitted.encode().is_err());
    }

    #[test]
    fn v2_receipt_authenticates_dust_reconciliation_and_refuses_v1() {
        let exact = PreMarketFundingReceiptV2 {
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
            project_found_receipt_digest: [11; 32],
            observed_dust_lamports: 17,
            top_up_lamports: 0,
            refund_lamports: 4,
            exact_post_lamports: 13,
        };
        let bytes = exact.encode().expect("V2 receipt");
        assert_eq!(PreMarketFundingReceiptV2::decode(&bytes), Ok(exact));
        assert!(
            PreMarketFundingReceiptV2::decode(
                &PreMarketFundingReceiptV1 {
                    market: exact.market,
                    generation: exact.generation,
                    manifest: exact.manifest,
                    selected_mask: exact.selected_mask,
                    ledger: exact.ledger,
                    prestate_digest: exact.prestate_digest,
                    poststate_digest: exact.poststate_digest,
                    exact_rent_lamports: exact.exact_rent_lamports,
                    exact_native_principal: exact.exact_native_principal,
                    found_request_digest: exact.found_request_digest,
                    funding_source: exact.funding_source,
                    rent_credit: exact.rent_credit,
                }
                .encode()
                .expect("legacy receipt")
            )
            .is_err()
        );
        for hostile in [
            PreMarketFundingReceiptV2 {
                refund_lamports: 3,
                ..exact
            },
            PreMarketFundingReceiptV2 {
                top_up_lamports: 1,
                ..exact
            },
            PreMarketFundingReceiptV2 {
                exact_post_lamports: 14,
                ..exact
            },
            PreMarketFundingReceiptV2 {
                project_found_receipt_digest: [0; 32],
                ..exact
            },
        ] {
            assert!(hostile.encode().is_err());
        }
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
