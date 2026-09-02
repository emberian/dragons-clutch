//! Fixed request and receipt for first-use Direct Custody replay setup.
//!
//! The wallet names only the Market, immutable generation, buyer maker, and
//! exact pre-Market digest. Trading derives every Custody request coordinate
//! from those values and authenticated accounts; the caller cannot submit a
//! Custody request or caller authority.

use dclutch_sha256_adapter::digestv;

/// High selector reserved for Direct Custody replay setup.
pub const DIRECT_REPLAY_SETUP_SELECTOR_V1: u32 = 0xffff_ff01;
/// Canonical request magic.
pub const DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTDRS1";
/// Canonical receipt magic.
pub const DIRECT_REPLAY_SETUP_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDSR1";
/// Exact request width.
pub const DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1: usize = 120;
/// Exact receipt width.
pub const DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1: usize = 376;
/// Implemented wire version.
pub const DIRECT_REPLAY_SETUP_VERSION_V1: u16 = 1;
/// Domain separating the synthetic Custody parent digest.
pub const DIRECT_REPLAY_SETUP_PARENT_DOMAIN_V1: &[u8] = b"dclutch/direct/replay-setup-parent/v1";

/// Width of one identity field. Every 32-byte field in both records is one.
const IDENTITY_BYTES: usize = 32;
/// Width of one little-endian u64 field.
const SCALAR_BYTES: usize = 8;

const MARKET_OFFSET: usize = 16;
const MAKER_OFFSET: usize = 48;
const MARKET_DIGEST_OFFSET: usize = 80;
const GENERATION_OFFSET: usize = 112;

const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 16;
const RECEIPT_MARKET_OFFSET: usize = 48;
const RECEIPT_MAKER_OFFSET: usize = 80;
const RECEIPT_MAKER_ROOT_OFFSET: usize = 112;
const RECEIPT_CUSTODY_REPLAY_OFFSET: usize = 144;
const RECEIPT_RENT_REFUND_OFFSET: usize = 176;
const RECEIPT_PAYER_OFFSET: usize = 208;
const RECEIPT_CUSTODY_REQUEST_DIGEST_OFFSET: usize = 240;
const RECEIPT_CUSTODY_POSTSTATE_OFFSET: usize = 272;
const RECEIPT_CUSTODY_REPLAY_DIGEST_OFFSET: usize = 304;
const RECEIPT_OBSERVED_LAMPORTS_OFFSET: usize = 336;
const RECEIPT_PAYER_TOP_UP_OFFSET: usize = 344;
const RECEIPT_REFUNDED_EXCESS_OFFSET: usize = 352;
const RECEIPT_EXACT_RENT_OFFSET: usize = 360;
const RECEIPT_POST_LAMPORTS_OFFSET: usize = 368;

/// Stable hostile-decode refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReplaySetupErrorV1 {
    /// A wire had another width.
    InvalidLength,
    /// Magic, version, or selector selected another route.
    InvalidHeader,
    /// Reserved bytes were nonzero.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// Lamport normalization facts were not exact.
    InvalidNormalization,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, DirectReplaySetupErrorV1>;

/// Exact permissionless setup request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReplaySetupRequestV1 {
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// Buyer maker wallet that determines the Trading maker root.
    pub maker: [u8; 32],
    /// SHA-256 of the exact canonical pre-Market bytes.
    pub expected_market_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
}

impl DirectReplaySetupRequestV1 {
    /// Validate required identities.
    pub fn new(self) -> Result<Self> {
        require_nonzero(self.market)?;
        require_nonzero(self.maker)?;
        require_nonzero(self.expected_market_digest)?;
        Ok(self)
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1)?;
        Self {
            market: array(input, MARKET_OFFSET)?,
            maker: array(input, MAKER_OFFSET)?,
            expected_market_digest: array(input, MARKET_DIGEST_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical request.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1];
        output[..8].copy_from_slice(&DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1);
        output[8..10].copy_from_slice(&DIRECT_REPLAY_SETUP_VERSION_V1.to_le_bytes());
        output[12..16].copy_from_slice(&DIRECT_REPLAY_SETUP_SELECTOR_V1.to_le_bytes());
        output[MARKET_OFFSET..MARKET_OFFSET + 32].copy_from_slice(&self.market);
        output[MAKER_OFFSET..MAKER_OFFSET + 32].copy_from_slice(&self.maker);
        output[MARKET_DIGEST_OFFSET..MARKET_DIGEST_OFFSET + 32]
            .copy_from_slice(&self.expected_market_digest);
        output[GENERATION_OFFSET..GENERATION_OFFSET + 8]
            .copy_from_slice(&self.generation.to_le_bytes());
        Ok(output)
    }
}

/// Exact Trading acknowledgment of Custody replay setup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectReplaySetupReceiptV1 {
    /// SHA-256 of the complete top-level request.
    pub request_digest: [u8; 32],
    /// Canonical Core Market.
    pub market: [u8; 32],
    /// Buyer maker wallet.
    pub maker: [u8; 32],
    /// Derived Trading maker-root context.
    pub maker_root: [u8; 32],
    /// Derived Custody replay PDA.
    pub custody_replay: [u8; 32],
    /// Canonical lifecycle RentCredit beneficiary.
    pub rent_refund: [u8; 32],
    /// Transaction-independent protocol rent payer.
    pub payer: [u8; 32],
    /// SHA-256 of the exact derived Custody request.
    pub custody_request_digest: [u8; 32],
    /// Custody poststate commitment stored in the replay.
    pub custody_poststate: [u8; 32],
    /// SHA-256 of the complete canonical replay bytes.
    pub custody_replay_digest: [u8; 32],
    /// Lamports observed on the vacant system-owned replay before setup.
    pub observed_lamports: u64,
    /// Exact shortfall transferred from the payer.
    pub payer_top_up: u64,
    /// Exact dust excess returned to the lifecycle RentCredit.
    pub refunded_excess: u64,
    /// Exact Rent minimum for the replay width.
    pub exact_rent: u64,
    /// Exact replay lamports after initialization.
    pub post_lamports: u64,
}

impl DirectReplaySetupReceiptV1 {
    /// Validate all required identities and exact normalization arithmetic.
    pub fn new(self) -> Result<Self> {
        for value in [
            self.request_digest,
            self.market,
            self.maker,
            self.maker_root,
            self.custody_replay,
            self.rent_refund,
            self.payer,
            self.custody_request_digest,
            self.custody_poststate,
            self.custody_replay_digest,
        ] {
            require_nonzero(value)?;
        }
        if self.post_lamports != self.exact_rent
            || (self.observed_lamports <= self.exact_rent
                && (self.payer_top_up != self.exact_rent - self.observed_lamports
                    || self.refunded_excess != 0))
            || (self.observed_lamports > self.exact_rent
                && (self.payer_top_up != 0
                    || self.refunded_excess != self.observed_lamports - self.exact_rent))
        {
            return Err(DirectReplaySetupErrorV1::InvalidNormalization);
        }
        Ok(self)
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(input, DIRECT_REPLAY_SETUP_RECEIPT_MAGIC_V1)?;
        Self {
            request_digest: array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            market: array(input, RECEIPT_MARKET_OFFSET)?,
            maker: array(input, RECEIPT_MAKER_OFFSET)?,
            maker_root: array(input, RECEIPT_MAKER_ROOT_OFFSET)?,
            custody_replay: array(input, RECEIPT_CUSTODY_REPLAY_OFFSET)?,
            rent_refund: array(input, RECEIPT_RENT_REFUND_OFFSET)?,
            payer: array(input, RECEIPT_PAYER_OFFSET)?,
            custody_request_digest: array(input, RECEIPT_CUSTODY_REQUEST_DIGEST_OFFSET)?,
            custody_poststate: array(input, RECEIPT_CUSTODY_POSTSTATE_OFFSET)?,
            custody_replay_digest: array(input, RECEIPT_CUSTODY_REPLAY_DIGEST_OFFSET)?,
            observed_lamports: u64_at(input, RECEIPT_OBSERVED_LAMPORTS_OFFSET)?,
            payer_top_up: u64_at(input, RECEIPT_PAYER_TOP_UP_OFFSET)?,
            refunded_excess: u64_at(input, RECEIPT_REFUNDED_EXCESS_OFFSET)?,
            exact_rent: u64_at(input, RECEIPT_EXACT_RENT_OFFSET)?,
            post_lamports: u64_at(input, RECEIPT_POST_LAMPORTS_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1];
        output[..8].copy_from_slice(&DIRECT_REPLAY_SETUP_RECEIPT_MAGIC_V1);
        output[8..10].copy_from_slice(&DIRECT_REPLAY_SETUP_VERSION_V1.to_le_bytes());
        output[12..16].copy_from_slice(&DIRECT_REPLAY_SETUP_SELECTOR_V1.to_le_bytes());
        // The receipt is two gapless runs: ten identities from the request
        // digest up to the lamport block, then five little-endian u64s filling
        // the record to its end. Each run's offsets were the previous plus the
        // field width, so listing the fields in order tiles the run exactly.
        // The asserts are what keep that quiet failure loud: zip truncates, so
        // a field added without widening the record would silently vanish from
        // the receipt rather than panic. They compile out of the SBF release
        // build, so the ELF pays nothing for them.
        let identities = [
            self.request_digest,
            self.market,
            self.maker,
            self.maker_root,
            self.custody_replay,
            self.rent_refund,
            self.payer,
            self.custody_request_digest,
            self.custody_poststate,
            self.custody_replay_digest,
        ];
        debug_assert!(
            RECEIPT_OBSERVED_LAMPORTS_OFFSET.saturating_sub(RECEIPT_REQUEST_DIGEST_OFFSET)
                == identities.len().saturating_mul(IDENTITY_BYTES)
        );
        for (slot, value) in output[RECEIPT_REQUEST_DIGEST_OFFSET..RECEIPT_OBSERVED_LAMPORTS_OFFSET]
            .chunks_exact_mut(IDENTITY_BYTES)
            .zip(identities.iter())
        {
            slot.copy_from_slice(value);
        }
        let scalars = [
            self.observed_lamports,
            self.payer_top_up,
            self.refunded_excess,
            self.exact_rent,
            self.post_lamports,
        ];
        debug_assert!(
            DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1.saturating_sub(RECEIPT_OBSERVED_LAMPORTS_OFFSET)
                == scalars.len().saturating_mul(SCALAR_BYTES)
        );
        for (slot, value) in output
            [RECEIPT_OBSERVED_LAMPORTS_OFFSET..DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1]
            .chunks_exact_mut(SCALAR_BYTES)
            .zip(scalars.iter())
        {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        Ok(output)
    }
}

/// Detect the exact setup request family without accepting a partial header.
#[must_use]
pub fn is_direct_replay_setup_v1(input: &[u8]) -> bool {
    input.len() == DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1
        && input.get(..8) == Some(DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1.as_slice())
}

/// Derive the exact synthetic parent digest used in the child Custody request.
#[must_use]
pub fn direct_replay_setup_parent_digest_v1(
    top_request_digest: [u8; 32],
    maker_root: [u8; 32],
    rent_refund: [u8; 32],
    payer: [u8; 32],
    exact_rent: u64,
) -> [u8; 32] {
    digestv(&[
        DIRECT_REPLAY_SETUP_PARENT_DOMAIN_V1,
        &top_request_digest,
        &maker_root,
        &rent_refund,
        &payer,
        &exact_rent.to_le_bytes(),
    ])
}

fn require_header(input: &[u8], magic: [u8; 8]) -> Result<()> {
    if input.len()
        != if magic == DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1 {
            DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1
        } else {
            DIRECT_REPLAY_SETUP_RECEIPT_BYTES_V1
        }
    {
        return Err(DirectReplaySetupErrorV1::InvalidLength);
    }
    if input.get(..8) != Some(magic.as_slice())
        || u16_at(input, 8)? != DIRECT_REPLAY_SETUP_VERSION_V1
        || input.get(10..12) != Some(&[0, 0])
        || u32_at(input, 12)? != DIRECT_REPLAY_SETUP_SELECTOR_V1
    {
        return Err(DirectReplaySetupErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(DirectReplaySetupErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    input
        .get(offset..offset + 32)
        .ok_or(DirectReplaySetupErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| DirectReplaySetupErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    input
        .get(offset..offset + 2)
        .ok_or(DirectReplaySetupErrorV1::InvalidLength)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| DirectReplaySetupErrorV1::InvalidLength)
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    input
        .get(offset..offset + 4)
        .ok_or(DirectReplaySetupErrorV1::InvalidLength)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| DirectReplaySetupErrorV1::InvalidLength)
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    input
        .get(offset..offset + 8)
        .ok_or(DirectReplaySetupErrorV1::InvalidLength)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| DirectReplaySetupErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    #[test]
    fn request_and_receipt_are_exact_hostile_decodable_wires() {
        let request = DirectReplaySetupRequestV1 {
            market: id(1),
            maker: id(2),
            expected_market_digest: id(3),
            generation: 7,
        };
        let request_bytes = request.to_bytes().expect("request");
        assert_eq!(
            DirectReplaySetupRequestV1::decode(&request_bytes),
            Ok(request)
        );
        assert!(is_direct_replay_setup_v1(&request_bytes));

        let receipt = DirectReplaySetupReceiptV1 {
            request_digest: id(4),
            market: request.market,
            maker: request.maker,
            maker_root: id(5),
            custody_replay: id(6),
            rent_refund: id(7),
            payer: id(8),
            custody_request_digest: id(9),
            custody_poststate: id(10),
            custody_replay_digest: id(11),
            observed_lamports: 3,
            payer_top_up: 97,
            refunded_excess: 0,
            exact_rent: 100,
            post_lamports: 100,
        };
        let receipt_bytes = receipt.to_bytes().expect("receipt");
        assert_eq!(
            DirectReplaySetupReceiptV1::decode(&receipt_bytes),
            Ok(receipt)
        );

        for offset in [0, 8, 10, 12] {
            let mut hostile = request_bytes;
            hostile[offset] ^= 1;
            assert!(DirectReplaySetupRequestV1::decode(&hostile).is_err());
        }
        assert_eq!(
            DirectReplaySetupRequestV1::decode(&request_bytes[..119]),
            Err(DirectReplaySetupErrorV1::InvalidLength)
        );
    }

    #[test]
    fn normalization_refuses_inconsistent_or_overflowed_facts() {
        let base = DirectReplaySetupReceiptV1 {
            request_digest: id(1),
            market: id(2),
            maker: id(3),
            maker_root: id(4),
            custody_replay: id(5),
            rent_refund: id(6),
            payer: id(7),
            custody_request_digest: id(8),
            custody_poststate: id(9),
            custody_replay_digest: id(10),
            observed_lamports: 101,
            payer_top_up: 0,
            refunded_excess: 1,
            exact_rent: 100,
            post_lamports: 100,
        };
        assert!(base.new().is_ok());
        assert_eq!(
            DirectReplaySetupReceiptV1 {
                payer_top_up: 1,
                ..base
            }
            .new(),
            Err(DirectReplaySetupErrorV1::InvalidNormalization)
        );
        assert_eq!(
            DirectReplaySetupReceiptV1 {
                post_lamports: 101,
                ..base
            }
            .new(),
            Err(DirectReplaySetupErrorV1::InvalidNormalization)
        );
    }
}
