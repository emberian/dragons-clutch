//! Exact expiry rollback for one Prepared Resolution funding ledger.

use dclutch_sha256_adapter::digestv;

use crate::resolution::{Error, Result};

/// Resolution funding-abort request magic.
pub const PRE_MARKET_FUNDING_ABORT_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLRPAQ1";
/// Exact Resolution funding-abort request width.
pub const PRE_MARKET_FUNDING_ABORT_REQUEST_BYTES_V1: usize = 368;
/// Resolution funding-abort receipt magic.
pub const PRE_MARKET_FUNDING_ABORT_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLRPAR1";
/// Exact Resolution funding-abort receipt width.
pub const PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1: usize = 488;

const VERSION_V1: u16 = 1;
const LEDGER_ACCOUNT_DIGEST_DOMAIN_V1: &[u8] = b"dclutch/resolution-ledger-account-state/v1";

/// Exact expiry rollback request for one Resolution-owned Pending ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreMarketFundingAbortRequestV1 {
    /// Durable checkpoint phase: `1` Prepared, `3` CustodyAborted, or `4`/`5`
    /// after the canonical first controller ledger closed.
    pub checkpoint_phase: u8,
    /// Exact checkpoint revision, equal to its phase in V1.
    pub checkpoint_revision: u64,
    /// Activated execution-release-set identity.
    pub release_set: [u8; 32],
    /// Trading-owned controller-funding checkpoint PDA.
    pub checkpoint: [u8; 32],
    /// SHA-256 of the exact checkpoint prestate.
    pub checkpoint_digest: [u8; 32],
    /// Future Core Market.
    pub market: [u8; 32],
    /// Future Market generation.
    pub generation: u64,
    /// Finalized capability-manifest identity.
    pub manifest: [u8; 32],
    /// Ordered two-controller funding-list identity.
    pub funding_list: [u8; 32],
    /// Exact three-row Resolution subset.
    pub selected_mask: u16,
    /// Resolution-owned funding-ledger PDA.
    pub ledger: [u8; 32],
    /// Digest of the exact ledger account prestate, including lamports and data.
    pub ledger_account_digest: [u8; 32],
    /// Original native-principal funding source.
    pub funding_source: [u8; 32],
    /// Canonical ledger-Rent refund account.
    pub rent_credit: [u8; 32],
    /// Last slot at which staging or opening was allowed.
    pub expiry_slot: u64,
}

impl PreMarketFundingAbortRequestV1 {
    fn validate(self) -> Result<Self> {
        if !valid_phase_revision(self.checkpoint_phase, self.checkpoint_revision)
            || self.generation == 0
            || self.expiry_slot == 0
            || self.selected_mask.count_ones() != 3
            || self.funding_source == self.rent_credit
            || required_ids(self).iter().any(is_zero)
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Ok(self)
    }

    /// Encode the sole canonical abort request.
    pub fn encode(self) -> Result<[u8; PRE_MARKET_FUNDING_ABORT_REQUEST_BYTES_V1]> {
        let value = self.validate()?;
        let mut output = [0_u8; PRE_MARKET_FUNDING_ABORT_REQUEST_BYTES_V1];
        put(&mut output, 0, &PRE_MARKET_FUNDING_ABORT_REQUEST_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        output[10] = value.checkpoint_phase;
        put(&mut output, 16, &value.checkpoint_revision.to_le_bytes())?;
        for (offset, field) in [
            (24, value.release_set),
            (56, value.checkpoint),
            (88, value.checkpoint_digest),
            (120, value.market),
            (160, value.manifest),
            (192, value.funding_list),
            (232, value.ledger),
            (264, value.ledger_account_digest),
            (296, value.funding_source),
            (328, value.rent_credit),
        ] {
            put(&mut output, offset, &field)?;
        }
        put(&mut output, 152, &value.generation.to_le_bytes())?;
        put(&mut output, 224, &value.selected_mask.to_le_bytes())?;
        put(&mut output, 360, &value.expiry_slot.to_le_bytes())?;
        Ok(output)
    }

    /// Hostile-decode one exact abort request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(
            input,
            PRE_MARKET_FUNDING_ABORT_REQUEST_BYTES_V1,
            &PRE_MARKET_FUNDING_ABORT_REQUEST_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1
            || any_nonzero(input, 11, 5)?
            || any_nonzero(input, 226, 6)?
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Self {
            checkpoint_phase: read_u8(input, 10)?,
            checkpoint_revision: read_u64(input, 16)?,
            release_set: read_array(input, 24)?,
            checkpoint: read_array(input, 56)?,
            checkpoint_digest: read_array(input, 88)?,
            market: read_array(input, 120)?,
            generation: read_u64(input, 152)?,
            manifest: read_array(input, 160)?,
            funding_list: read_array(input, 192)?,
            selected_mask: read_u16(input, 224)?,
            ledger: read_array(input, 232)?,
            ledger_account_digest: read_array(input, 264)?,
            funding_source: read_array(input, 296)?,
            rent_credit: read_array(input, 328)?,
            expiry_slot: read_u64(input, 360)?,
        }
        .validate()
    }
}

/// Exact receipt proving one Resolution Pending ledger was canonically closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreMarketFundingAbortReceiptV1 {
    /// Durable checkpoint phase authenticated before close.
    pub checkpoint_phase: u8,
    /// Exact checkpoint revision authenticated before close.
    pub checkpoint_revision: u64,
    /// SHA-256 of the exact abort request bytes.
    pub request_digest: [u8; 32],
    /// Activated execution-release-set identity.
    pub release_set: [u8; 32],
    /// Trading-owned checkpoint PDA.
    pub checkpoint: [u8; 32],
    /// Exact checkpoint prestate digest.
    pub checkpoint_digest: [u8; 32],
    /// Future Core Market.
    pub market: [u8; 32],
    /// Future Market generation.
    pub generation: u64,
    /// Finalized capability-manifest identity.
    pub manifest: [u8; 32],
    /// Ordered two-controller funding-list identity.
    pub funding_list: [u8; 32],
    /// Exact three-row Resolution subset.
    pub selected_mask: u16,
    /// Closed Resolution funding-ledger PDA.
    pub ledger: [u8; 32],
    /// Exact funded account-state digest before close.
    pub ledger_account_digest: [u8; 32],
    /// Original principal funding source.
    pub funding_source: [u8; 32],
    /// Canonical ledger-Rent refund account.
    pub rent_credit: [u8; 32],
    /// Checkpoint expiry slot proven exceeded.
    pub expiry_slot: u64,
    /// Exact native principal refunded to `funding_source`.
    pub native_principal_refund_lamports: u64,
    /// Exact ledger Rent refunded to `rent_credit`.
    pub rent_refund_lamports: u64,
    /// Exact sum of the two classified refunds.
    pub total_refund_lamports: u64,
    /// Digest of the zero-lamport, zero-data System-owned poststate.
    pub closed_account_digest: [u8; 32],
    /// Resolution program that produced this receipt.
    pub producer: [u8; 32],
}

impl PreMarketFundingAbortReceiptV1 {
    fn validate(self) -> Result<Self> {
        let total = self
            .native_principal_refund_lamports
            .checked_add(self.rent_refund_lamports)
            .ok_or(Error::InvalidPreMarketFunding)?;
        if !valid_phase_revision(self.checkpoint_phase, self.checkpoint_revision)
            || self.generation == 0
            || self.expiry_slot == 0
            || self.selected_mask.count_ones() != 3
            || self.rent_refund_lamports == 0
            || total != self.total_refund_lamports
            || self.funding_source == self.rent_credit
            || receipt_ids(self).iter().any(is_zero)
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Ok(self)
    }

    /// Encode the sole canonical abort receipt.
    pub fn encode(self) -> Result<[u8; PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1]> {
        let value = self.validate()?;
        let mut output = [0_u8; PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1];
        put(&mut output, 0, &PRE_MARKET_FUNDING_ABORT_RECEIPT_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        output[10] = value.checkpoint_phase;
        put(&mut output, 16, &value.checkpoint_revision.to_le_bytes())?;
        for (offset, field) in [
            (24, value.request_digest),
            (56, value.release_set),
            (88, value.checkpoint),
            (120, value.checkpoint_digest),
            (152, value.market),
            (192, value.manifest),
            (224, value.funding_list),
            (264, value.ledger),
            (296, value.ledger_account_digest),
            (328, value.funding_source),
            (360, value.rent_credit),
            (424, value.closed_account_digest),
            (456, value.producer),
        ] {
            put(&mut output, offset, &field)?;
        }
        put(&mut output, 184, &value.generation.to_le_bytes())?;
        put(&mut output, 256, &value.selected_mask.to_le_bytes())?;
        put(&mut output, 392, &value.expiry_slot.to_le_bytes())?;
        put(
            &mut output,
            400,
            &value.native_principal_refund_lamports.to_le_bytes(),
        )?;
        put(&mut output, 408, &value.rent_refund_lamports.to_le_bytes())?;
        put(&mut output, 416, &value.total_refund_lamports.to_le_bytes())?;
        Ok(output)
    }

    /// Hostile-decode one exact abort receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        exact(
            input,
            PRE_MARKET_FUNDING_ABORT_RECEIPT_BYTES_V1,
            &PRE_MARKET_FUNDING_ABORT_RECEIPT_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1
            || any_nonzero(input, 11, 5)?
            || any_nonzero(input, 258, 6)?
        {
            return Err(Error::InvalidPreMarketFunding);
        }
        Self {
            checkpoint_phase: read_u8(input, 10)?,
            checkpoint_revision: read_u64(input, 16)?,
            request_digest: read_array(input, 24)?,
            release_set: read_array(input, 56)?,
            checkpoint: read_array(input, 88)?,
            checkpoint_digest: read_array(input, 120)?,
            market: read_array(input, 152)?,
            generation: read_u64(input, 184)?,
            manifest: read_array(input, 192)?,
            funding_list: read_array(input, 224)?,
            selected_mask: read_u16(input, 256)?,
            ledger: read_array(input, 264)?,
            ledger_account_digest: read_array(input, 296)?,
            funding_source: read_array(input, 328)?,
            rent_credit: read_array(input, 360)?,
            expiry_slot: read_u64(input, 392)?,
            native_principal_refund_lamports: read_u64(input, 400)?,
            rent_refund_lamports: read_u64(input, 408)?,
            total_refund_lamports: read_u64(input, 416)?,
            closed_account_digest: read_array(input, 424)?,
            producer: read_array(input, 456)?,
        }
        .validate()
    }
}

/// Digest one exact Resolution ledger account state before or after close.
#[must_use]
pub fn pre_market_funding_ledger_account_digest_v1(
    key: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data: &[u8],
) -> [u8; 32] {
    let lamports = lamports.to_le_bytes();
    let data_len = u64::try_from(data.len()).unwrap_or(u64::MAX).to_le_bytes();
    digestv(&[
        LEDGER_ACCOUNT_DIGEST_DOMAIN_V1,
        &key,
        &owner,
        &lamports,
        &data_len,
        data,
    ])
}

fn required_ids(value: PreMarketFundingAbortRequestV1) -> [[u8; 32]; 10] {
    [
        value.release_set,
        value.checkpoint,
        value.checkpoint_digest,
        value.market,
        value.manifest,
        value.funding_list,
        value.ledger,
        value.ledger_account_digest,
        value.funding_source,
        value.rent_credit,
    ]
}

fn receipt_ids(value: PreMarketFundingAbortReceiptV1) -> [[u8; 32]; 13] {
    [
        value.request_digest,
        value.release_set,
        value.checkpoint,
        value.checkpoint_digest,
        value.market,
        value.manifest,
        value.funding_list,
        value.ledger,
        value.ledger_account_digest,
        value.funding_source,
        value.rent_credit,
        value.closed_account_digest,
        value.producer,
    ]
}

fn valid_phase_revision(phase: u8, revision: u64) -> bool {
    matches!((phase, revision), (1, 1) | (3, 3) | (4, 4) | (5, 5))
}

fn is_zero(value: &[u8; 32]) -> bool {
    *value == [0; 32]
}

fn exact(input: &[u8], width: usize, magic: &[u8; 8]) -> Result<()> {
    if input.len() != width || input.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidPreMarketFunding);
    }
    Ok(())
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(Error::InvalidPreMarketFunding)?,
        )
        .ok_or(Error::InvalidPreMarketFunding)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidPreMarketFunding)
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(Error::InvalidPreMarketFunding)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn any_nonzero(input: &[u8], offset: usize, width: usize) -> Result<bool> {
    Ok(slice(input, offset, width)?.iter().any(|byte| *byte != 0))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
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

    fn request() -> PreMarketFundingAbortRequestV1 {
        PreMarketFundingAbortRequestV1 {
            checkpoint_phase: 1,
            checkpoint_revision: 1,
            release_set: [1; 32],
            checkpoint: [2; 32],
            checkpoint_digest: [3; 32],
            market: [4; 32],
            generation: 5,
            manifest: [6; 32],
            funding_list: [7; 32],
            selected_mask: 0b111,
            ledger: [8; 32],
            ledger_account_digest: [9; 32],
            funding_source: [10; 32],
            rent_credit: [11; 32],
            expiry_slot: 12,
        }
    }

    #[test]
    fn request_roundtrip_and_phase_partition_are_exact() {
        let exact = request();
        let bytes = exact.encode().expect("request");
        assert_eq!(PreMarketFundingAbortRequestV1::decode(&bytes), Ok(exact));
        for (phase, revision) in [(1, 1), (3, 3), (4, 4), (5, 5)] {
            let candidate = PreMarketFundingAbortRequestV1 {
                checkpoint_phase: phase,
                checkpoint_revision: revision,
                ..exact
            };
            assert_eq!(
                PreMarketFundingAbortRequestV1::decode(
                    &candidate.encode().expect("accepted cleanup phase")
                ),
                Ok(candidate)
            );
        }
        for (phase, revision) in [
            (1, 2),
            (2, 2),
            (2, 1),
            (3, 4),
            (4, 5),
            (5, 4),
            (0, 0),
            (6, 6),
        ] {
            assert!(
                PreMarketFundingAbortRequestV1 {
                    checkpoint_phase: phase,
                    checkpoint_revision: revision,
                    ..exact
                }
                .encode()
                .is_err()
            );
        }
    }

    #[test]
    fn receipt_refuses_substitution_and_refund_mismatch() {
        let exact = PreMarketFundingAbortReceiptV1 {
            checkpoint_phase: 3,
            checkpoint_revision: 3,
            request_digest: [1; 32],
            release_set: [2; 32],
            checkpoint: [3; 32],
            checkpoint_digest: [4; 32],
            market: [5; 32],
            generation: 6,
            manifest: [7; 32],
            funding_list: [8; 32],
            selected_mask: 0b111,
            ledger: [9; 32],
            ledger_account_digest: [10; 32],
            funding_source: [11; 32],
            rent_credit: [12; 32],
            expiry_slot: 13,
            native_principal_refund_lamports: 14,
            rent_refund_lamports: 15,
            total_refund_lamports: 29,
            closed_account_digest: [16; 32],
            producer: [17; 32],
        };
        let bytes = exact.encode().expect("receipt");
        assert_eq!(PreMarketFundingAbortReceiptV1::decode(&bytes), Ok(exact));
        assert!(
            PreMarketFundingAbortReceiptV1 {
                total_refund_lamports: 28,
                ..exact
            }
            .encode()
            .is_err()
        );
        assert!(
            PreMarketFundingAbortReceiptV1 {
                producer: [0; 32],
                ..exact
            }
            .encode()
            .is_err()
        );
    }

    #[test]
    fn account_digest_binds_every_physical_coordinate() {
        let exact = pre_market_funding_ledger_account_digest_v1([1; 32], [2; 32], 3, &[4, 5]);
        assert_ne!(
            exact,
            pre_market_funding_ledger_account_digest_v1([9; 32], [2; 32], 3, &[4, 5])
        );
        assert_ne!(
            exact,
            pre_market_funding_ledger_account_digest_v1([1; 32], [9; 32], 3, &[4, 5])
        );
        assert_ne!(
            exact,
            pre_market_funding_ledger_account_digest_v1([1; 32], [2; 32], 9, &[4, 5])
        );
        assert_ne!(
            exact,
            pre_market_funding_ledger_account_digest_v1([1; 32], [2; 32], 3, &[4, 6])
        );
    }
}
