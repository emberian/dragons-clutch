//! Exact first-use setup wire for Direct's seller and fee Token-2022 accounts.
//!
//! The request names no token-account address. Trading authenticates the
//! Market-selected Direct config and the seller's canonical Claims Position,
//! then derives both accounts under its own program ID. This keeps destination
//! selection out of an untrusted operator while leaving setup permissionless.

use dclutch_sha256_adapter::digestv;

/// High selector reserved for Direct token-account setup.
pub const DIRECT_TOKEN_SETUP_SELECTOR_V1: u32 = 0xffff_ff02;
/// Canonical request magic.
pub const DIRECT_TOKEN_SETUP_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTDTS1";
/// Canonical receipt magic.
pub const DIRECT_TOKEN_SETUP_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDTR1";
/// Implemented wire version.
pub const DIRECT_TOKEN_SETUP_VERSION_V1: u16 = 1;
use crate::successor::DIRECT_MAX_FEE_BASIS_POINTS_V1;

/// Exact request width.
pub const DIRECT_TOKEN_SETUP_REQUEST_BYTES_V1: usize = 216;
/// Exact receipt width.
pub const DIRECT_TOKEN_SETUP_RECEIPT_BYTES_V1: usize = 680;
/// Exact top-level account count owned by the adapter route.
pub const DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1: usize = 23;
/// The rate the canonical bootstrap founds a Direct market at.
///
/// IT IS NOT A PROTOCOL BOUND AND THIS MODULE NO LONGER REFUSES ON IT. The
/// protocol band is `DIRECT_MAX_FEE_BASIS_POINTS_V1`, enforced once, by
/// `DirectExecutionConfigV1::new`. This constant is what the local-validator
/// bootstrap chooses, and a founder may choose otherwise inside the band.
pub const DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1: u16 = 50;
/// Domain separating the exact ordered account frame commitment.
pub const DIRECT_TOKEN_SETUP_FRAME_DOMAIN_V1: &[u8] = b"dclutch/direct/token-setup-frame/v1";
/// PDA domain shared by the two role-separated token accounts.
pub const DIRECT_TOKEN_ACCOUNT_PDA_DOMAIN_V1: &[u8] = b"dclutch:direct-token:v1";

/// Width of the fixed magic/version/reserved/selector header both records open
/// with, and so the offset of each record's first field.
const HEADER_BYTES: usize = 16;
/// Width of one identity field. Every 32-byte field in both records is one.
const IDENTITY_BYTES: usize = 32;
/// Width of one encoded [`DirectTokenRentNormalizationV1`]: five u64s.
const NORMALIZATION_BYTES: usize = 40;

const MARKET_OFFSET: usize = 16;
const MARKET_DIGEST_OFFSET: usize = 48;
const ROOT_DIGEST_OFFSET: usize = 80;
const CLAIMS_AGGREGATE_DIGEST_OFFSET: usize = 112;
const SELLER_OWNER_OFFSET: usize = 144;
const SELLER_POSITION_DIGEST_OFFSET: usize = 176;
const GENERATION_OFFSET: usize = 208;

const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 16;
const RECEIPT_FRAME_DIGEST_OFFSET: usize = 48;
const RECEIPT_MARKET_OFFSET: usize = 80;
const RECEIPT_RELEASE_SET_OFFSET: usize = 112;
const RECEIPT_REALM_OFFSET: usize = 144;
const RECEIPT_DIRECT_CONFIG_OFFSET: usize = 176;
const RECEIPT_CLAIMS_AGGREGATE_OFFSET: usize = 208;
const RECEIPT_SELLER_POSITION_OFFSET: usize = 240;
const RECEIPT_COLLATERAL_MINT_OFFSET: usize = 272;
const RECEIPT_TOKEN_PROGRAM_OFFSET: usize = 304;
const RECEIPT_SELLER_OWNER_OFFSET: usize = 336;
const RECEIPT_FEE_RECIPIENT_OFFSET: usize = 368;
const RECEIPT_SELLER_TOKEN_OFFSET: usize = 400;
const RECEIPT_FEE_TOKEN_OFFSET: usize = 432;
const RECEIPT_RENT_REFUND_OFFSET: usize = 464;
const RECEIPT_PAYER_OFFSET: usize = 496;
const RECEIPT_SELLER_POSTSTATE_DIGEST_OFFSET: usize = 528;
const RECEIPT_FEE_POSTSTATE_DIGEST_OFFSET: usize = 560;
const RECEIPT_FEE_BASIS_POINTS_OFFSET: usize = 592;
const RECEIPT_RESERVED_OFFSET: usize = 594;
const RECEIPT_RESERVED_BYTES: usize = 6;
const RECEIPT_SELLER_OBSERVED_OFFSET: usize = 600;
const RECEIPT_FEE_OBSERVED_OFFSET: usize = 640;

/// Stable hostile-decode refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectTokenSetupErrorV1 {
    /// A wire had another exact width.
    InvalidLength,
    /// Magic, version, selector, or reserved bytes selected another wire.
    InvalidHeader,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// A role or pair of token coordinates aliased.
    Alias,
    /// The selected fee rate was outside the protocol band.
    InvalidFee,
    /// Rent normalization facts were inconsistent or overflowed.
    InvalidNormalization,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, DirectTokenSetupErrorV1>;

/// Exact permissionless setup request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTokenSetupRequestV1 {
    /// Canonical Core Market PDA.
    pub market: [u8; 32],
    /// SHA-256 of the complete canonical pre-Market bytes.
    pub expected_market_digest: [u8; 32],
    /// SHA-256 of the complete Direct root account bytes.
    pub expected_root_digest: [u8; 32],
    /// SHA-256 of the complete canonical Claims aggregate bytes.
    pub expected_claims_aggregate_digest: [u8; 32],
    /// Seller identity that must own the authenticated Claims Position.
    pub seller_owner: [u8; 32],
    /// SHA-256 of the complete canonical seller Position bytes.
    pub expected_seller_position_digest: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
}

impl DirectTokenSetupRequestV1 {
    /// Validate all request identities.
    pub fn new(self) -> Result<Self> {
        for value in [
            self.market,
            self.expected_market_digest,
            self.expected_root_digest,
            self.expected_claims_aggregate_digest,
            self.seller_owner,
            self.expected_seller_position_digest,
        ] {
            require_nonzero(value)?;
        }
        Ok(self)
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(
            input,
            DIRECT_TOKEN_SETUP_REQUEST_MAGIC_V1,
            DIRECT_TOKEN_SETUP_REQUEST_BYTES_V1,
        )?;
        Self {
            market: array(input, MARKET_OFFSET)?,
            expected_market_digest: array(input, MARKET_DIGEST_OFFSET)?,
            expected_root_digest: array(input, ROOT_DIGEST_OFFSET)?,
            expected_claims_aggregate_digest: array(input, CLAIMS_AGGREGATE_DIGEST_OFFSET)?,
            seller_owner: array(input, SELLER_OWNER_OFFSET)?,
            expected_seller_position_digest: array(input, SELLER_POSITION_DIGEST_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical request.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_TOKEN_SETUP_REQUEST_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0; DIRECT_TOKEN_SETUP_REQUEST_BYTES_V1];
        output[..HEADER_BYTES].copy_from_slice(&header(DIRECT_TOKEN_SETUP_REQUEST_MAGIC_V1));
        // The six identities occupy MARKET_OFFSET..GENERATION_OFFSET with no
        // gaps, so the offsets were never independent facts -- each was the
        // previous plus 32. In order, they tile that region exactly.
        let identities = [
            self.market,
            self.expected_market_digest,
            self.expected_root_digest,
            self.expected_claims_aggregate_digest,
            self.seller_owner,
            self.expected_seller_position_digest,
        ];
        debug_assert!(
            GENERATION_OFFSET.saturating_sub(MARKET_OFFSET)
                == identities.len().saturating_mul(IDENTITY_BYTES)
        );
        for (slot, value) in output[MARKET_OFFSET..GENERATION_OFFSET]
            .chunks_exact_mut(IDENTITY_BYTES)
            .zip(identities.iter())
        {
            slot.copy_from_slice(value);
        }
        output[GENERATION_OFFSET..GENERATION_OFFSET + 8]
            .copy_from_slice(&self.generation.to_le_bytes());
        Ok(output)
    }
}

/// One account's exact lamport normalization facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTokenRentNormalizationV1 {
    /// Lamports present before setup.
    pub observed_lamports: u64,
    /// Exact payer-funded shortfall.
    pub payer_top_up: u64,
    /// Exact excess returned to the Market RentCredit.
    pub refunded_excess: u64,
    /// Exact Token Account rent minimum.
    pub exact_rent: u64,
    /// Exact post-initialization lamports.
    pub post_lamports: u64,
}

impl DirectTokenRentNormalizationV1 {
    /// Validate the sole canonical normalization of one observed balance.
    pub fn new(self) -> Result<Self> {
        let expected =
            direct_token_rent_normalization_v1(self.observed_lamports, self.exact_rent, 0)?;
        if self.payer_top_up != expected.payer_top_up
            || self.refunded_excess != expected.refunded_excess
            || self.post_lamports != self.exact_rent
        {
            return Err(DirectTokenSetupErrorV1::InvalidNormalization);
        }
        Ok(self)
    }
}

/// Exact Trading acknowledgment of both initialized token accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTokenSetupReceiptV1 {
    /// SHA-256 of the complete top-level request.
    pub request_digest: [u8; 32],
    /// SHA-256 of the exact ordered frame including privileges.
    pub frame_digest: [u8; 32],
    /// Canonical Core Market.
    pub market: [u8; 32],
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Finalized Realm content identity.
    pub realm: [u8; 32],
    /// Finalized Direct config content identity.
    pub direct_config: [u8; 32],
    /// Canonical Claims aggregate PDA.
    pub claims_aggregate: [u8; 32],
    /// Canonical seller Position PDA.
    pub seller_position: [u8; 32],
    /// Realm-selected collateral Mint.
    pub collateral_mint: [u8; 32],
    /// Realm-selected Token-2022 program.
    pub token_program: [u8; 32],
    /// Position-authenticated seller owner.
    pub seller_owner: [u8; 32],
    /// Config-selected fee recipient.
    pub fee_recipient: [u8; 32],
    /// Derived seller token account.
    pub seller_token: [u8; 32],
    /// Derived fee token account.
    pub fee_token: [u8; 32],
    /// Core Market's canonical lifecycle RentCredit.
    pub rent_refund: [u8; 32],
    /// Sole transaction signer and shortfall payer.
    pub payer: [u8; 32],
    /// SHA-256 of the complete seller Token Account poststate.
    pub seller_poststate_digest: [u8; 32],
    /// SHA-256 of the complete fee Token Account poststate.
    pub fee_poststate_digest: [u8; 32],
    /// Release-pinned Direct fee rate.
    pub fee_basis_points: u16,
    /// Seller account normalization facts.
    pub seller_normalization: DirectTokenRentNormalizationV1,
    /// Fee account normalization facts.
    pub fee_normalization: DirectTokenRentNormalizationV1,
}

impl DirectTokenSetupReceiptV1 {
    /// Validate all receipt identities and arithmetic.
    pub fn new(self) -> Result<Self> {
        for value in [
            self.request_digest,
            self.frame_digest,
            self.market,
            self.release_set,
            self.realm,
            self.direct_config,
            self.claims_aggregate,
            self.seller_position,
            self.collateral_mint,
            self.token_program,
            self.seller_owner,
            self.fee_recipient,
            self.seller_token,
            self.fee_token,
            self.rent_refund,
            self.payer,
            self.seller_poststate_digest,
            self.fee_poststate_digest,
        ] {
            require_nonzero(value)?;
        }
        // The BAND, which is the protocol's, not a point, which was this
        // module's alone. See `DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1`.
        if self.fee_basis_points > DIRECT_MAX_FEE_BASIS_POINTS_V1 {
            return Err(DirectTokenSetupErrorV1::InvalidFee);
        }
        if self.seller_token == self.fee_token
            || self.seller_token == self.payer
            || self.seller_token == self.rent_refund
            || self.fee_token == self.payer
            || self.fee_token == self.rent_refund
            || self.payer == self.rent_refund
        {
            return Err(DirectTokenSetupErrorV1::Alias);
        }
        self.seller_normalization.new()?;
        self.fee_normalization.new()?;
        Ok(self)
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(
            input,
            DIRECT_TOKEN_SETUP_RECEIPT_MAGIC_V1,
            DIRECT_TOKEN_SETUP_RECEIPT_BYTES_V1,
        )?;
        if input
            .get(RECEIPT_RESERVED_OFFSET..RECEIPT_RESERVED_OFFSET + RECEIPT_RESERVED_BYTES)
            .is_none_or(|bytes| bytes.iter().any(|byte| *byte != 0))
        {
            return Err(DirectTokenSetupErrorV1::InvalidHeader);
        }
        Self {
            request_digest: array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            frame_digest: array(input, RECEIPT_FRAME_DIGEST_OFFSET)?,
            market: array(input, RECEIPT_MARKET_OFFSET)?,
            release_set: array(input, RECEIPT_RELEASE_SET_OFFSET)?,
            realm: array(input, RECEIPT_REALM_OFFSET)?,
            direct_config: array(input, RECEIPT_DIRECT_CONFIG_OFFSET)?,
            claims_aggregate: array(input, RECEIPT_CLAIMS_AGGREGATE_OFFSET)?,
            seller_position: array(input, RECEIPT_SELLER_POSITION_OFFSET)?,
            collateral_mint: array(input, RECEIPT_COLLATERAL_MINT_OFFSET)?,
            token_program: array(input, RECEIPT_TOKEN_PROGRAM_OFFSET)?,
            seller_owner: array(input, RECEIPT_SELLER_OWNER_OFFSET)?,
            fee_recipient: array(input, RECEIPT_FEE_RECIPIENT_OFFSET)?,
            seller_token: array(input, RECEIPT_SELLER_TOKEN_OFFSET)?,
            fee_token: array(input, RECEIPT_FEE_TOKEN_OFFSET)?,
            rent_refund: array(input, RECEIPT_RENT_REFUND_OFFSET)?,
            payer: array(input, RECEIPT_PAYER_OFFSET)?,
            seller_poststate_digest: array(input, RECEIPT_SELLER_POSTSTATE_DIGEST_OFFSET)?,
            fee_poststate_digest: array(input, RECEIPT_FEE_POSTSTATE_DIGEST_OFFSET)?,
            fee_basis_points: u16_at(input, RECEIPT_FEE_BASIS_POINTS_OFFSET)?,
            seller_normalization: normalization_at(input, RECEIPT_SELLER_OBSERVED_OFFSET)?,
            fee_normalization: normalization_at(input, RECEIPT_FEE_OBSERVED_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_TOKEN_SETUP_RECEIPT_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0; DIRECT_TOKEN_SETUP_RECEIPT_BYTES_V1];
        output[..HEADER_BYTES].copy_from_slice(&header(DIRECT_TOKEN_SETUP_RECEIPT_MAGIC_V1));
        // Eighteen identities tile RECEIPT_REQUEST_DIGEST_OFFSET up to the
        // fee-basis-points scalar with no gaps, so listing them in order says
        // once what eighteen separate offsets said eighteen times.
        let identities = [
            self.request_digest,
            self.frame_digest,
            self.market,
            self.release_set,
            self.realm,
            self.direct_config,
            self.claims_aggregate,
            self.seller_position,
            self.collateral_mint,
            self.token_program,
            self.seller_owner,
            self.fee_recipient,
            self.seller_token,
            self.fee_token,
            self.rent_refund,
            self.payer,
            self.seller_poststate_digest,
            self.fee_poststate_digest,
        ];
        debug_assert!(
            RECEIPT_FEE_BASIS_POINTS_OFFSET.saturating_sub(RECEIPT_REQUEST_DIGEST_OFFSET)
                == identities.len().saturating_mul(IDENTITY_BYTES)
        );
        for (slot, value) in output[RECEIPT_REQUEST_DIGEST_OFFSET..RECEIPT_FEE_BASIS_POINTS_OFFSET]
            .chunks_exact_mut(IDENTITY_BYTES)
            .zip(identities.iter())
        {
            slot.copy_from_slice(value);
        }
        output[RECEIPT_FEE_BASIS_POINTS_OFFSET..RECEIPT_FEE_BASIS_POINTS_OFFSET + 2]
            .copy_from_slice(&self.fee_basis_points.to_le_bytes());
        output
            [RECEIPT_SELLER_OBSERVED_OFFSET..RECEIPT_SELLER_OBSERVED_OFFSET + NORMALIZATION_BYTES]
            .copy_from_slice(&normalization_bytes(self.seller_normalization));
        output[RECEIPT_FEE_OBSERVED_OFFSET..RECEIPT_FEE_OBSERVED_OFFSET + NORMALIZATION_BYTES]
            .copy_from_slice(&normalization_bytes(self.fee_normalization));
        Ok(output)
    }
}

/// One of the two immutable token-account PDA roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DirectTokenAccountRoleV1 {
    /// Seller collateral destination.
    Seller = 0,
    /// Venue-fee collateral destination.
    Fee = 1,
}

/// Owned exact Trading PDA seed projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTokenAccountSeedsV1 {
    market: [u8; 32],
    generation: [u8; 8],
    owner: [u8; 32],
    role: [u8; 1],
}

impl DirectTokenAccountSeedsV1 {
    /// Validate and construct one role-separated token-account coordinate.
    pub fn new(
        market: [u8; 32],
        generation: u64,
        owner: [u8; 32],
        role: DirectTokenAccountRoleV1,
    ) -> Result<Self> {
        require_nonzero(market)?;
        require_nonzero(owner)?;
        Ok(Self {
            market,
            generation: generation.to_le_bytes(),
            owner,
            role: [role as u8],
        })
    }

    /// Return exact PDA seeds excluding the bump.
    pub fn as_slices(&self) -> [&[u8]; 5] {
        [
            DIRECT_TOKEN_ACCOUNT_PDA_DOMAIN_V1,
            &self.market,
            &self.generation,
            &self.owner,
            &self.role,
        ]
    }
}

/// Compute the one canonical lamport normalization and preflight refund overflow.
pub fn direct_token_rent_normalization_v1(
    observed_lamports: u64,
    exact_rent: u64,
    refund_lamports: u64,
) -> Result<DirectTokenRentNormalizationV1> {
    let (payer_top_up, refunded_excess) = if observed_lamports > exact_rent {
        let excess = observed_lamports
            .checked_sub(exact_rent)
            .ok_or(DirectTokenSetupErrorV1::InvalidNormalization)?;
        refund_lamports
            .checked_add(excess)
            .ok_or(DirectTokenSetupErrorV1::InvalidNormalization)?;
        (0, excess)
    } else {
        (
            exact_rent
                .checked_sub(observed_lamports)
                .ok_or(DirectTokenSetupErrorV1::InvalidNormalization)?,
            0,
        )
    };
    Ok(DirectTokenRentNormalizationV1 {
        observed_lamports,
        payer_top_up,
        refunded_excess,
        exact_rent,
        post_lamports: exact_rent,
    })
}

/// Commit the exact ordered account addresses and fixed privilege vector.
#[must_use]
pub fn direct_token_setup_frame_digest_v1(
    accounts: [[u8; 32]; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1],
) -> [u8; 32] {
    // bit 0 signer, bit 1 writable, bit 2 executable.
    const PRIVILEGES: [u8; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1] = [
        0, 4, 4, 0, 4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 3, 2, 0, 4, 4,
    ];
    let privileges: &'static [u8; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1] = &PRIVILEGES;
    // After the domain, the preimage is one address then its one privilege
    // byte, per account, in frame order -- which is exactly what flattening the
    // zipped pairs produces. Filling the tail in order writes the same parts the
    // computed `1 + index * 2` / `2 + index * 2` slots did, with no index.
    let mut parts: [&[u8]; 1 + DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1 * 2] =
        [DIRECT_TOKEN_SETUP_FRAME_DOMAIN_V1; 1 + DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1 * 2];
    let tail = accounts
        .iter()
        .zip(privileges.iter())
        .flat_map(|(account, privilege)| [account.as_slice(), core::slice::from_ref(privilege)]);
    for (slot, part) in parts.iter_mut().skip(1).zip(tail) {
        *slot = part;
    }
    digestv(&parts)
}

/// Detect only the complete canonical request family width and magic.
#[must_use]
pub fn is_direct_token_setup_v1(input: &[u8]) -> bool {
    input.len() == DIRECT_TOKEN_SETUP_REQUEST_BYTES_V1
        && input.get(..8) == Some(DIRECT_TOKEN_SETUP_REQUEST_MAGIC_V1.as_slice())
}

fn require_header(input: &[u8], magic: [u8; 8], width: usize) -> Result<()> {
    if input.len() != width {
        return Err(DirectTokenSetupErrorV1::InvalidLength);
    }
    if input.get(..8) != Some(magic.as_slice())
        || u16_at(input, 8)? != DIRECT_TOKEN_SETUP_VERSION_V1
        || input.get(10..12) != Some(&[0, 0])
        || u32_at(input, 12)? != DIRECT_TOKEN_SETUP_SELECTOR_V1
    {
        return Err(DirectTokenSetupErrorV1::InvalidHeader);
    }
    Ok(())
}

/// Build the fixed record header: magic, version, two reserved zero bytes,
/// selector.
///
/// Written into a local buffer of the header's exact width rather than into a
/// caller's slice, so the compiler can see that every range is in bounds. Bytes
/// 10..12 stay zero exactly as they did when the caller's zeroed record was
/// written in place.
fn header(magic: [u8; 8]) -> [u8; HEADER_BYTES] {
    let mut head = [0_u8; HEADER_BYTES];
    head[..8].copy_from_slice(&magic);
    head[8..10].copy_from_slice(&DIRECT_TOKEN_SETUP_VERSION_V1.to_le_bytes());
    head[12..16].copy_from_slice(&DIRECT_TOKEN_SETUP_SELECTOR_V1.to_le_bytes());
    head
}

fn normalization_at(input: &[u8], offset: usize) -> Result<DirectTokenRentNormalizationV1> {
    Ok(DirectTokenRentNormalizationV1 {
        observed_lamports: u64_at(input, offset)?,
        payer_top_up: u64_at(input, offset + 8)?,
        refunded_excess: u64_at(input, offset + 16)?,
        exact_rent: u64_at(input, offset + 24)?,
        post_lamports: u64_at(input, offset + 32)?,
    })
}

/// Build one normalization block: five little-endian u64s that tile its exact
/// width, so `chunks_exact_mut` yields exactly five slots and the zip is total.
///
/// The assert is what stops the tiling from going quiet: zip truncates, so
/// adding a scalar without widening the block would silently drop it from the
/// receipt rather than panic. It compiles out of the SBF release build.
fn normalization_bytes(value: DirectTokenRentNormalizationV1) -> [u8; NORMALIZATION_BYTES] {
    let scalars = [
        value.observed_lamports,
        value.payer_top_up,
        value.refunded_excess,
        value.exact_rent,
        value.post_lamports,
    ];
    let mut block = [0_u8; NORMALIZATION_BYTES];
    debug_assert!(block.len() == scalars.len().saturating_mul(8));
    for (slot, scalar) in block.chunks_exact_mut(8).zip(scalars.iter()) {
        slot.copy_from_slice(&scalar.to_le_bytes());
    }
    block
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(DirectTokenSetupErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    input
        .get(offset..offset + 32)
        .ok_or(DirectTokenSetupErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| DirectTokenSetupErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    input
        .get(offset..offset + 2)
        .ok_or(DirectTokenSetupErrorV1::InvalidLength)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| DirectTokenSetupErrorV1::InvalidLength)
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    input
        .get(offset..offset + 4)
        .ok_or(DirectTokenSetupErrorV1::InvalidLength)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| DirectTokenSetupErrorV1::InvalidLength)
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    input
        .get(offset..offset + 8)
        .ok_or(DirectTokenSetupErrorV1::InvalidLength)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| DirectTokenSetupErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn request() -> DirectTokenSetupRequestV1 {
        DirectTokenSetupRequestV1 {
            market: id(1),
            expected_market_digest: id(2),
            expected_root_digest: id(3),
            expected_claims_aggregate_digest: id(4),
            seller_owner: id(5),
            expected_seller_position_digest: id(6),
            generation: 7,
        }
    }

    fn receipt() -> DirectTokenSetupReceiptV1 {
        DirectTokenSetupReceiptV1 {
            request_digest: id(1),
            frame_digest: id(2),
            market: id(3),
            release_set: id(4),
            realm: id(5),
            direct_config: id(6),
            claims_aggregate: id(7),
            seller_position: id(8),
            collateral_mint: id(9),
            token_program: id(10),
            seller_owner: id(11),
            fee_recipient: id(12),
            seller_token: id(13),
            fee_token: id(14),
            rent_refund: id(15),
            payer: id(16),
            seller_poststate_digest: id(17),
            fee_poststate_digest: id(18),
            fee_basis_points: DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1,
            seller_normalization: direct_token_rent_normalization_v1(99, 100, 0)
                .expect("seller normalization"),
            fee_normalization: direct_token_rent_normalization_v1(101, 100, 0)
                .expect("fee normalization"),
        }
    }

    #[test]
    fn request_and_receipt_round_trip_and_refuse_hostile_headers() {
        let request = request();
        let bytes = request.to_bytes().expect("request");
        assert_eq!(DirectTokenSetupRequestV1::decode(&bytes), Ok(request));
        assert!(is_direct_token_setup_v1(&bytes));
        assert_eq!(
            DirectTokenSetupRequestV1::decode(&bytes[..bytes.len() - 1]),
            Err(DirectTokenSetupErrorV1::InvalidLength)
        );
        for offset in [0, 8, 10, 12] {
            let mut hostile = bytes;
            hostile[offset] ^= 1;
            assert!(DirectTokenSetupRequestV1::decode(&hostile).is_err());
        }

        let receipt = receipt();
        let bytes = receipt.to_bytes().expect("receipt");
        assert_eq!(DirectTokenSetupReceiptV1::decode(&bytes), Ok(receipt));
        let mut reserved = bytes;
        reserved[RECEIPT_RESERVED_OFFSET] = 1;
        assert_eq!(
            DirectTokenSetupReceiptV1::decode(&reserved),
            Err(DirectTokenSetupErrorV1::InvalidHeader)
        );
    }

    #[test]
    fn normalization_covers_every_dust_boundary_and_refund_overflow() {
        let rent = 100;
        for (observed, top_up, refund) in [
            (0, 100, 0),
            (1, 99, 0),
            (rent - 1, 1, 0),
            (rent, 0, 0),
            (rent + 1, 0, 1),
            (u64::MAX, 0, u64::MAX - rent),
        ] {
            let value =
                direct_token_rent_normalization_v1(observed, rent, 0).expect("normalization");
            assert_eq!(value.payer_top_up, top_up);
            assert_eq!(value.refunded_excess, refund);
            assert_eq!(value.post_lamports, rent);
            assert!(value.new().is_ok());
        }
        assert_eq!(
            direct_token_rent_normalization_v1(rent + 1, rent, u64::MAX),
            Err(DirectTokenSetupErrorV1::InvalidNormalization)
        );
    }

    #[test]
    fn receipt_refuses_alias_wrong_fee_and_wrong_poststate_facts() {
        let base = receipt();
        assert_eq!(
            DirectTokenSetupReceiptV1 {
                fee_token: base.seller_token,
                ..base
            }
            .new(),
            Err(DirectTokenSetupErrorV1::Alias)
        );
        // 49 is INSIDE the band and is admitted now. The receipt records the
        // rate the finalized config states; it does not have an opinion about
        // which rate a market chose. A market at 30 basis points is the reason
        // this changed -- cohort-11's config says 30, this module used to demand
        // exactly 50, and since this route is the sole creator of the seller's
        // and the venue's token accounts that market could never trade and its
        // record could never be repaired.
        for admitted in [0, 30, 49, 50, DIRECT_MAX_FEE_BASIS_POINTS_V1] {
            assert!(
                DirectTokenSetupReceiptV1 {
                    fee_basis_points: admitted,
                    ..base
                }
                .new()
                .is_ok(),
                "{admitted} basis points is inside the protocol band",
            );
        }
        // The BAND still refuses, by name, one point above it.
        assert_eq!(
            DirectTokenSetupReceiptV1 {
                fee_basis_points: DIRECT_MAX_FEE_BASIS_POINTS_V1 + 1,
                ..base
            }
            .new(),
            Err(DirectTokenSetupErrorV1::InvalidFee)
        );
        assert_eq!(
            DirectTokenSetupReceiptV1 {
                seller_normalization: DirectTokenRentNormalizationV1 {
                    post_lamports: 101,
                    ..base.seller_normalization
                },
                ..base
            }
            .new(),
            Err(DirectTokenSetupErrorV1::InvalidNormalization)
        );
    }

    #[test]
    fn role_and_frame_order_are_digest_authority() {
        let seller =
            DirectTokenAccountSeedsV1::new(id(1), 2, id(3), DirectTokenAccountRoleV1::Seller)
                .expect("seller");
        let fee = DirectTokenAccountSeedsV1::new(id(1), 2, id(3), DirectTokenAccountRoleV1::Fee)
            .expect("fee");
        assert_ne!(seller.as_slices(), fee.as_slices());

        let mut accounts = [[0; 32]; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1];
        for (index, account) in accounts.iter_mut().enumerate() {
            *account = [u8::try_from(index + 1).expect("small index"); 32];
        }
        let baseline = direct_token_setup_frame_digest_v1(accounts);
        accounts.swap(0, 1);
        assert_ne!(direct_token_setup_frame_digest_v1(accounts), baseline);
    }
}
