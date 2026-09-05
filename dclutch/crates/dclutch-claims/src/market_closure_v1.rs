//! Claims-owned aggregate-empty Market closure request and receipt.
//!
//! The SBF adapter must authenticate the selected Core caller and current
//! Claims release, decode the runtime-width aggregate, prove every supply is
//! zero, close the aggregate to the immutable RentCredit, and only then emit
//! [`ClaimsMarketClosureReceiptV1`].

use core::convert::TryInto;

#[allow(missing_docs)]
mod generated {
    include!("generated_market_closure_v1.rs");
}

pub use generated::{
    CLAIMS_MARKET_CLOSURE_ACTION_V1, CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
    CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1, CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1,
    CLAIMS_MARKET_CLOSURE_RECEIPT_MAGIC_V1, CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1,
    CLAIMS_MARKET_CLOSURE_REQUEST_MAGIC_V1, CLAIMS_MARKET_CLOSURE_VERSION_V1,
};

use generated::{
    CLAIMS_CLOSURE_RECEIPT_AGGREGATE_OFFSET, CLAIMS_CLOSURE_RECEIPT_CLAIM_COUNT_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_GENERATION_OFFSET, CLAIMS_CLOSURE_RECEIPT_KIND_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_LIABILITY_UNITS_OFFSET, CLAIMS_CLOSURE_RECEIPT_MAGIC_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_MARKET_OFFSET, CLAIMS_CLOSURE_RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_POST_REVISION_OFFSET, CLAIMS_CLOSURE_RECEIPT_PRE_RESOURCE_DIGEST_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_PRE_REVISION_OFFSET, CLAIMS_CLOSURE_RECEIPT_PRODUCER_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_REFUND_LAMPORTS_OFFSET, CLAIMS_CLOSURE_RECEIPT_RELEASE_SET_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_RENT_CREDIT_OFFSET, CLAIMS_CLOSURE_RECEIPT_REQUEST_DIGEST_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_RESERVED_BODY_OFFSET, CLAIMS_CLOSURE_RECEIPT_RESERVED_HEADER_OFFSET,
    CLAIMS_CLOSURE_RECEIPT_VERSION_OFFSET, CLAIMS_CLOSURE_REQUEST_ACTION_OFFSET,
    CLAIMS_CLOSURE_REQUEST_AGGREGATE_OFFSET, CLAIMS_CLOSURE_REQUEST_CLAIM_COUNT_OFFSET,
    CLAIMS_CLOSURE_REQUEST_CORE_PROGRAM_OFFSET, CLAIMS_CLOSURE_REQUEST_EXPECTED_REVISION_OFFSET,
    CLAIMS_CLOSURE_REQUEST_GENERATION_OFFSET, CLAIMS_CLOSURE_REQUEST_MAGIC_OFFSET,
    CLAIMS_CLOSURE_REQUEST_MARKET_OFFSET, CLAIMS_CLOSURE_REQUEST_PARENT_REQUEST_DIGEST_OFFSET,
    CLAIMS_CLOSURE_REQUEST_RELEASE_SET_OFFSET, CLAIMS_CLOSURE_REQUEST_RENT_CREDIT_OFFSET,
    CLAIMS_CLOSURE_REQUEST_RESERVED_BODY_OFFSET, CLAIMS_CLOSURE_REQUEST_RESERVED_HEADER_OFFSET,
    CLAIMS_CLOSURE_REQUEST_RESULTING_REVISION_OFFSET, CLAIMS_CLOSURE_REQUEST_VERSION_OFFSET,
};

const RECEIPT_KIND_V1: u8 = 1;

/// Stable hostile-decode or receipt-join refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsMarketClosureErrorV1 {
    /// Input width was not exact.
    InvalidLength,
    /// Magic, version, action, or kind selected another wire family.
    InvalidHeader,
    /// Reserved bytes were nonzero.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// Required distinct accounts aliased.
    AccountAlias,
    /// Generation, revision, width, or refund coordinates were invalid.
    InvalidCoordinate,
    /// A receipt did not bind the exact request and resource digests.
    ReceiptMismatch,
}

/// Result alias for Claims Market closure.
pub type ClaimsMarketClosureResultV1<T> = core::result::Result<T, ClaimsMarketClosureErrorV1>;

/// Construction input for one exact aggregate closure request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsMarketClosureRequestInputV1 {
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Canonical logical Core Market.
    pub market: [u8; 32],
    /// Canonical Claims aggregate account.
    pub aggregate: [u8; 32],
    /// Immutable RentCredit beneficiary.
    pub rent_credit: [u8; 32],
    /// SHA-256 of the exact parent Core retirement request and bundle.
    pub parent_request_digest: [u8; 32],
    /// Current Core program authorized to request closure.
    pub core_program: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Aggregate revision before closure.
    pub expected_revision: u64,
    /// Exact closure revision.
    pub resulting_revision: u64,
    /// Runtime Product/aggregate width.
    pub claim_count: u32,
}

/// One exact hostile-decodable aggregate closure request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsMarketClosureRequestV1(ClaimsMarketClosureRequestInputV1);

impl ClaimsMarketClosureRequestV1 {
    /// Construct and validate a closure request.
    pub fn new(input: ClaimsMarketClosureRequestInputV1) -> ClaimsMarketClosureResultV1<Self> {
        require_nonzero(&[
            input.release_set,
            input.market,
            input.aggregate,
            input.rent_credit,
            input.parent_request_digest,
            input.core_program,
        ])?;
        require_distinct(&[input.market, input.aggregate, input.rent_credit])?;
        if input.generation == 0
            || input.claim_count < 2
            || input.expected_revision.checked_add(1) != Some(input.resulting_revision)
        {
            return Err(ClaimsMarketClosureErrorV1::InvalidCoordinate);
        }
        Ok(Self(input))
    }

    /// Hostile-decode an exact request.
    pub fn decode(input: &[u8]) -> ClaimsMarketClosureResultV1<Self> {
        require_header(
            input,
            &CLAIMS_MARKET_CLOSURE_REQUEST_MAGIC_V1,
            CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1,
            CLAIMS_CLOSURE_REQUEST_VERSION_OFFSET,
        )?;
        if byte(input, CLAIMS_CLOSURE_REQUEST_ACTION_OFFSET)? != CLAIMS_MARKET_CLOSURE_ACTION_V1 {
            return Err(ClaimsMarketClosureErrorV1::InvalidHeader);
        }
        require_zero(input, CLAIMS_CLOSURE_REQUEST_RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, CLAIMS_CLOSURE_REQUEST_RESERVED_BODY_OFFSET, 20)?;
        Self::new(ClaimsMarketClosureRequestInputV1 {
            release_set: array(input, CLAIMS_CLOSURE_REQUEST_RELEASE_SET_OFFSET)?,
            market: array(input, CLAIMS_CLOSURE_REQUEST_MARKET_OFFSET)?,
            aggregate: array(input, CLAIMS_CLOSURE_REQUEST_AGGREGATE_OFFSET)?,
            rent_credit: array(input, CLAIMS_CLOSURE_REQUEST_RENT_CREDIT_OFFSET)?,
            parent_request_digest: array(
                input,
                CLAIMS_CLOSURE_REQUEST_PARENT_REQUEST_DIGEST_OFFSET,
            )?,
            core_program: array(input, CLAIMS_CLOSURE_REQUEST_CORE_PROGRAM_OFFSET)?,
            generation: u64_at(input, CLAIMS_CLOSURE_REQUEST_GENERATION_OFFSET)?,
            expected_revision: u64_at(input, CLAIMS_CLOSURE_REQUEST_EXPECTED_REVISION_OFFSET)?,
            resulting_revision: u64_at(input, CLAIMS_CLOSURE_REQUEST_RESULTING_REVISION_OFFSET)?,
            claim_count: u32_at(input, CLAIMS_CLOSURE_REQUEST_CLAIM_COUNT_OFFSET)?,
        })
    }

    /// Encode exact canonical request bytes.
    pub fn to_bytes(self) -> [u8; CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1] {
        let mut output = [0; CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1];
        put(
            &mut output,
            CLAIMS_CLOSURE_REQUEST_MAGIC_OFFSET,
            &CLAIMS_MARKET_CLOSURE_REQUEST_MAGIC_V1,
        );
        put_u16(
            &mut output,
            CLAIMS_CLOSURE_REQUEST_VERSION_OFFSET,
            CLAIMS_MARKET_CLOSURE_VERSION_V1,
        );
        output[CLAIMS_CLOSURE_REQUEST_ACTION_OFFSET] = CLAIMS_MARKET_CLOSURE_ACTION_V1;
        for (offset, value) in [
            (
                CLAIMS_CLOSURE_REQUEST_RELEASE_SET_OFFSET,
                self.0.release_set,
            ),
            (CLAIMS_CLOSURE_REQUEST_MARKET_OFFSET, self.0.market),
            (CLAIMS_CLOSURE_REQUEST_AGGREGATE_OFFSET, self.0.aggregate),
            (
                CLAIMS_CLOSURE_REQUEST_RENT_CREDIT_OFFSET,
                self.0.rent_credit,
            ),
            (
                CLAIMS_CLOSURE_REQUEST_PARENT_REQUEST_DIGEST_OFFSET,
                self.0.parent_request_digest,
            ),
            (
                CLAIMS_CLOSURE_REQUEST_CORE_PROGRAM_OFFSET,
                self.0.core_program,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (CLAIMS_CLOSURE_REQUEST_GENERATION_OFFSET, self.0.generation),
            (
                CLAIMS_CLOSURE_REQUEST_EXPECTED_REVISION_OFFSET,
                self.0.expected_revision,
            ),
            (
                CLAIMS_CLOSURE_REQUEST_RESULTING_REVISION_OFFSET,
                self.0.resulting_revision,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        put_u32(
            &mut output,
            CLAIMS_CLOSURE_REQUEST_CLAIM_COUNT_OFFSET,
            self.0.claim_count,
        );
        output
    }

    /// Borrow all validated request coordinates.
    pub const fn input(self) -> ClaimsMarketClosureRequestInputV1 {
        self.0
    }
}

/// Construction input for one Claims-owned aggregate-empty receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsMarketClosureReceiptInputV1 {
    /// Current Claims program producing the receipt.
    pub producer: [u8; 32],
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Canonical logical Market.
    pub market: [u8; 32],
    /// Closed aggregate account.
    pub aggregate: [u8; 32],
    /// Immutable RentCredit beneficiary.
    pub rent_credit: [u8; 32],
    /// SHA-256 of exact request bytes.
    pub request_digest: [u8; 32],
    /// SHA-256 of exact pre-closure aggregate bytes.
    pub pre_resource_digest: [u8; 32],
    /// Domain-separated closure/RentCredit poststate digest.
    pub post_resource_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Aggregate revision before closure.
    pub pre_revision: u64,
    /// Exact closure revision.
    pub post_revision: u64,
    /// Must be zero: total outstanding liability units.
    pub liability_units: u64,
    /// Exact aggregate lamports refunded.
    pub refund_lamports: u64,
    /// Runtime Product/aggregate width checked empty.
    pub claim_count: u32,
}

/// Immediate Claims-owned aggregate-empty closure evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsMarketClosureReceiptV1(ClaimsMarketClosureReceiptInputV1);

impl ClaimsMarketClosureReceiptV1 {
    /// Construct and validate an aggregate-empty receipt.
    pub fn new(input: ClaimsMarketClosureReceiptInputV1) -> ClaimsMarketClosureResultV1<Self> {
        require_nonzero(&[
            input.producer,
            input.release_set,
            input.market,
            input.aggregate,
            input.rent_credit,
            input.request_digest,
            input.pre_resource_digest,
            input.post_resource_digest,
        ])?;
        require_distinct(&[input.market, input.aggregate, input.rent_credit])?;
        if input.generation == 0
            || input.claim_count < 2
            || input.pre_revision.checked_add(1) != Some(input.post_revision)
            || input.liability_units != 0
            || input.refund_lamports == 0
        {
            return Err(ClaimsMarketClosureErrorV1::InvalidCoordinate);
        }
        Ok(Self(input))
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> ClaimsMarketClosureResultV1<Self> {
        require_header(
            input,
            &CLAIMS_MARKET_CLOSURE_RECEIPT_MAGIC_V1,
            CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1,
            CLAIMS_CLOSURE_RECEIPT_VERSION_OFFSET,
        )?;
        if byte(input, CLAIMS_CLOSURE_RECEIPT_KIND_OFFSET)? != RECEIPT_KIND_V1 {
            return Err(ClaimsMarketClosureErrorV1::InvalidHeader);
        }
        require_zero(input, CLAIMS_CLOSURE_RECEIPT_RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, CLAIMS_CLOSURE_RECEIPT_RESERVED_BODY_OFFSET, 4)?;
        Self::new(ClaimsMarketClosureReceiptInputV1 {
            producer: array(input, CLAIMS_CLOSURE_RECEIPT_PRODUCER_OFFSET)?,
            release_set: array(input, CLAIMS_CLOSURE_RECEIPT_RELEASE_SET_OFFSET)?,
            market: array(input, CLAIMS_CLOSURE_RECEIPT_MARKET_OFFSET)?,
            aggregate: array(input, CLAIMS_CLOSURE_RECEIPT_AGGREGATE_OFFSET)?,
            rent_credit: array(input, CLAIMS_CLOSURE_RECEIPT_RENT_CREDIT_OFFSET)?,
            request_digest: array(input, CLAIMS_CLOSURE_RECEIPT_REQUEST_DIGEST_OFFSET)?,
            pre_resource_digest: array(input, CLAIMS_CLOSURE_RECEIPT_PRE_RESOURCE_DIGEST_OFFSET)?,
            post_resource_digest: array(input, CLAIMS_CLOSURE_RECEIPT_POST_RESOURCE_DIGEST_OFFSET)?,
            generation: u64_at(input, CLAIMS_CLOSURE_RECEIPT_GENERATION_OFFSET)?,
            pre_revision: u64_at(input, CLAIMS_CLOSURE_RECEIPT_PRE_REVISION_OFFSET)?,
            post_revision: u64_at(input, CLAIMS_CLOSURE_RECEIPT_POST_REVISION_OFFSET)?,
            liability_units: u64_at(input, CLAIMS_CLOSURE_RECEIPT_LIABILITY_UNITS_OFFSET)?,
            refund_lamports: u64_at(input, CLAIMS_CLOSURE_RECEIPT_REFUND_LAMPORTS_OFFSET)?,
            claim_count: u32_at(input, CLAIMS_CLOSURE_RECEIPT_CLAIM_COUNT_OFFSET)?,
        })
    }

    /// Encode exact canonical receipt bytes.
    pub fn to_bytes(self) -> [u8; CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1] {
        let mut output = [0; CLAIMS_MARKET_CLOSURE_RECEIPT_BYTES_V1];
        put(
            &mut output,
            CLAIMS_CLOSURE_RECEIPT_MAGIC_OFFSET,
            &CLAIMS_MARKET_CLOSURE_RECEIPT_MAGIC_V1,
        );
        put_u16(
            &mut output,
            CLAIMS_CLOSURE_RECEIPT_VERSION_OFFSET,
            CLAIMS_MARKET_CLOSURE_VERSION_V1,
        );
        output[CLAIMS_CLOSURE_RECEIPT_KIND_OFFSET] = RECEIPT_KIND_V1;
        for (offset, value) in [
            (CLAIMS_CLOSURE_RECEIPT_PRODUCER_OFFSET, self.0.producer),
            (
                CLAIMS_CLOSURE_RECEIPT_RELEASE_SET_OFFSET,
                self.0.release_set,
            ),
            (CLAIMS_CLOSURE_RECEIPT_MARKET_OFFSET, self.0.market),
            (CLAIMS_CLOSURE_RECEIPT_AGGREGATE_OFFSET, self.0.aggregate),
            (
                CLAIMS_CLOSURE_RECEIPT_RENT_CREDIT_OFFSET,
                self.0.rent_credit,
            ),
            (
                CLAIMS_CLOSURE_RECEIPT_REQUEST_DIGEST_OFFSET,
                self.0.request_digest,
            ),
            (
                CLAIMS_CLOSURE_RECEIPT_PRE_RESOURCE_DIGEST_OFFSET,
                self.0.pre_resource_digest,
            ),
            (
                CLAIMS_CLOSURE_RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
                self.0.post_resource_digest,
            ),
        ] {
            put(&mut output, offset, &value);
        }
        for (offset, value) in [
            (CLAIMS_CLOSURE_RECEIPT_GENERATION_OFFSET, self.0.generation),
            (
                CLAIMS_CLOSURE_RECEIPT_PRE_REVISION_OFFSET,
                self.0.pre_revision,
            ),
            (
                CLAIMS_CLOSURE_RECEIPT_POST_REVISION_OFFSET,
                self.0.post_revision,
            ),
            (
                CLAIMS_CLOSURE_RECEIPT_LIABILITY_UNITS_OFFSET,
                self.0.liability_units,
            ),
            (
                CLAIMS_CLOSURE_RECEIPT_REFUND_LAMPORTS_OFFSET,
                self.0.refund_lamports,
            ),
        ] {
            put_u64(&mut output, offset, value);
        }
        put_u32(
            &mut output,
            CLAIMS_CLOSURE_RECEIPT_CLAIM_COUNT_OFFSET,
            self.0.claim_count,
        );
        output
    }

    /// Verify the receipt against one exact request and observed digests.
    pub fn verify_for(
        self,
        request: ClaimsMarketClosureRequestV1,
        request_digest: [u8; 32],
        pre_resource_digest: [u8; 32],
        post_resource_digest: [u8; 32],
    ) -> ClaimsMarketClosureResultV1<()> {
        let request = request.input();
        if self.0.release_set != request.release_set
            || self.0.market != request.market
            || self.0.aggregate != request.aggregate
            || self.0.rent_credit != request.rent_credit
            || self.0.request_digest != request_digest
            || self.0.pre_resource_digest != pre_resource_digest
            || self.0.post_resource_digest != post_resource_digest
            || self.0.generation != request.generation
            || self.0.pre_revision != request.expected_revision
            || self.0.post_revision != request.resulting_revision
            || self.0.claim_count != request.claim_count
        {
            return Err(ClaimsMarketClosureErrorV1::ReceiptMismatch);
        }
        Ok(())
    }

    /// Borrow all validated receipt coordinates.
    pub const fn input(self) -> ClaimsMarketClosureReceiptInputV1 {
        self.0
    }
}

fn require_nonzero(values: &[[u8; 32]]) -> ClaimsMarketClosureResultV1<()> {
    if values
        .iter()
        .any(|value| value.iter().all(|byte| *byte == 0))
    {
        Err(ClaimsMarketClosureErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_distinct(values: &[[u8; 32]]) -> ClaimsMarketClosureResultV1<()> {
    for (index, value) in values.iter().enumerate() {
        if values
            .get(index + 1..)
            .is_some_and(|tail| tail.contains(value))
        {
            return Err(ClaimsMarketClosureErrorV1::AccountAlias);
        }
    }
    Ok(())
}

fn require_header(
    input: &[u8],
    magic: &[u8; 8],
    width: usize,
    version_offset: usize,
) -> ClaimsMarketClosureResultV1<()> {
    if input.len() != width || input.get(..8) != Some(magic.as_slice()) {
        return Err(ClaimsMarketClosureErrorV1::InvalidLength);
    }
    if u16_at(input, version_offset)? != CLAIMS_MARKET_CLOSURE_VERSION_V1 {
        return Err(ClaimsMarketClosureErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> ClaimsMarketClosureResultV1<()> {
    if input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?,
        )
        .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(ClaimsMarketClosureErrorV1::NonCanonical)
    } else {
        Ok(())
    }
}

fn byte(input: &[u8], offset: usize) -> ClaimsMarketClosureResultV1<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)
}

fn array(input: &[u8], offset: usize) -> ClaimsMarketClosureResultV1<[u8; 32]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(32)
                    .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?,
        )
        .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| ClaimsMarketClosureErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> ClaimsMarketClosureResultV1<u16> {
    Ok(u16::from_le_bytes(
        input
            .get(
                offset
                    ..offset
                        .checked_add(2)
                        .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?,
            )
            .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| ClaimsMarketClosureErrorV1::InvalidLength)?,
    ))
}

fn u32_at(input: &[u8], offset: usize) -> ClaimsMarketClosureResultV1<u32> {
    Ok(u32::from_le_bytes(
        input
            .get(
                offset
                    ..offset
                        .checked_add(4)
                        .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?,
            )
            .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| ClaimsMarketClosureErrorV1::InvalidLength)?,
    ))
}

fn u64_at(input: &[u8], offset: usize) -> ClaimsMarketClosureResultV1<u64> {
    Ok(u64::from_le_bytes(
        input
            .get(
                offset
                    ..offset
                        .checked_add(8)
                        .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?,
            )
            .ok_or(ClaimsMarketClosureErrorV1::InvalidLength)?
            .try_into()
            .map_err(|_| ClaimsMarketClosureErrorV1::InvalidLength)?,
    ))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset + value.len()) {
        target.copy_from_slice(value);
    }
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ClaimsMarketClosureRequestV1 {
        ClaimsMarketClosureRequestV1::new(ClaimsMarketClosureRequestInputV1 {
            release_set: [1; 32],
            market: [2; 32],
            aggregate: [3; 32],
            rent_credit: [4; 32],
            parent_request_digest: [5; 32],
            core_program: [6; 32],
            generation: 9,
            expected_revision: 11,
            resulting_revision: 12,
            claim_count: 258,
        })
        .expect("request")
    }

    #[test]
    fn request_and_receipt_round_trip_at_runtime_width() {
        let request = request();
        assert_eq!(
            ClaimsMarketClosureRequestV1::decode(&request.to_bytes()),
            Ok(request)
        );
        let request_input = request.input();
        let receipt = ClaimsMarketClosureReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
            producer: [7; 32],
            release_set: request_input.release_set,
            market: request_input.market,
            aggregate: request_input.aggregate,
            rent_credit: request_input.rent_credit,
            request_digest: [8; 32],
            pre_resource_digest: [9; 32],
            post_resource_digest: [10; 32],
            generation: request_input.generation,
            pre_revision: request_input.expected_revision,
            post_revision: request_input.resulting_revision,
            liability_units: 0,
            refund_lamports: 100,
            claim_count: request_input.claim_count,
        })
        .expect("receipt");
        assert_eq!(
            ClaimsMarketClosureReceiptV1::decode(&receipt.to_bytes()),
            Ok(receipt)
        );
        assert_eq!(
            receipt.verify_for(request, [8; 32], [9; 32], [10; 32]),
            Ok(())
        );
    }

    #[test]
    fn residual_liability_and_digest_substitution_refuse() {
        let mut receipt = ClaimsMarketClosureReceiptInputV1 {
            producer: [7; 32],
            release_set: [1; 32],
            market: [2; 32],
            aggregate: [3; 32],
            rent_credit: [4; 32],
            request_digest: [8; 32],
            pre_resource_digest: [9; 32],
            post_resource_digest: [10; 32],
            generation: 9,
            pre_revision: 11,
            post_revision: 12,
            liability_units: 1,
            refund_lamports: 100,
            claim_count: 258,
        };
        assert_eq!(
            ClaimsMarketClosureReceiptV1::new(receipt),
            Err(ClaimsMarketClosureErrorV1::InvalidCoordinate)
        );
        receipt.liability_units = 0;
        let receipt = ClaimsMarketClosureReceiptV1::new(receipt).expect("empty receipt");
        assert_eq!(
            receipt.verify_for(request(), [8; 32], [9; 32], [11; 32]),
            Err(ClaimsMarketClosureErrorV1::ReceiptMismatch)
        );
    }
}
