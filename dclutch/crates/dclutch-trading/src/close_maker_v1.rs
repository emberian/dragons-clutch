//! Fixed-layout request and receipt for the permissionless maker-replay close.
//!
//! This is the missing decrement (wall 22): the ONLY route that can ever
//! reduce `open_maker_root_count`, and therefore the only thing standing
//! between a filled market and `CloseCapability`'s zero-count gate. It runs
//! inside Retiring -- `consume_nonce_v2` refuses every non-Open phase, so the
//! count can only fall once retirement begins -- and it moves exactly one
//! maker replay from open to closed per invocation.
//!
//! **The request carries no economic value**: like `fee_settlement_v1`, every
//! lamport destination and amount is read out of program-owned state (the
//! replay's immutable `rent_owner` and `rent_principal`), so a stranger's
//! submission is effect-free beyond the outcome the replay already fixed. The
//! wire carries only the COORDINATE -- which market, which maker -- and, like
//! that sibling, **no expected-state digest**: sibling closes rewrite the
//! Direct root's count word, so a pinned digest would let each close grief the
//! next submission for no protection the route's own derivations do not
//! already give.

use dclutch_sha256_adapter::digestv;

/// High selector reserved for the maker-replay close.
///
/// `0xffff_ff03` is `DIRECT_FEE_SETTLEMENT_SELECTOR_V1` -- a wire discriminant
/// that never became a ProgramSet entry -- and is skipped here so the two
/// namespaces can never alias.
pub const DIRECT_CLOSE_MAKER_SELECTOR_V1: u32 = 0xffff_ff04;
/// Exact permissionless close-maker request width.
pub const DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1: usize = 96;
/// Exact close-maker receipt width.
///
/// Widened from 240 on 2026-09-04 for `closer_reward`. The receipt is
/// `set_return_data` only -- no released record, descriptor or profile digests
/// it -- so the width is a codec fact and not a release identity.
pub const DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1: usize = 248;
/// Close-maker request magic.
pub const DIRECT_CLOSE_MAKER_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTDMC1";
/// Close-maker receipt magic.
pub const DIRECT_CLOSE_MAKER_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDMX1";
/// Implemented close-maker wire version.
pub const DIRECT_CLOSE_MAKER_VERSION_V1: u16 = 1;
/// Canonical `u32` selector byte offset shared with the Direct ProgramSet.
pub const DIRECT_CLOSE_MAKER_SELECTOR_OFFSET_V1: usize = 12;
/// Finalized schema label for the close-maker request.
pub const DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/direct-close-maker-request-v1";
/// SHA-256 of [`DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0xca, 0xa2, 0x0c, 0x4f, 0xd5, 0xe2, 0x9e, 0xa4, 0xf9, 0x1d, 0x85, 0xfb, 0xcc, 0x6d, 0xbe, 0x17,
    0x42, 0xa3, 0x87, 0xd9, 0xac, 0x0a, 0xae, 0x61, 0x8f, 0x7a, 0x90, 0x71, 0x5c, 0xe7, 0x19, 0x36,
];

/// The carve ceiling this route passes to `close_maker_replay_v2`.
///
/// **RULED 2026-09-04 (C-11 D1 item 4): a permissionless closer's reward is
/// carved from the donation slice alone, capped at the funded-crank floor.**
/// The rule is in the kernel and proved in Lean
/// (`the_closer_carve_never_touches_principal`,
/// `the_closer_carve_is_capped_and_bounded_by_the_donation`), and the governed
/// value is `closer_reward_cap_lamports` in
/// `dclutch-market::protocol_parameters`'s record, whose genesis this constant
/// projects so the two can never disagree.
///
/// **It is zero, and the reason is the FRAME, not the ruling.**
/// `direct_close_maker_v1.rs` refuses any signer at all
/// (`accounts.iter().any(|account| account.is_signer)`), so there is no closer
/// in the twenty-two-account frame to pay. Paying one needs a twenty-third
/// account with a signer conjunct -- `FUNDED_CRANK_V1.md` section 6's "the
/// caller signs only to own the reward, never to be authorized" -- which moves
/// the released AccountProfile, the close descriptor's digest, and therefore
/// every derived identity. That is a cohort cut's work and it is OWED, named
/// here rather than left as a zero a reader would take for a policy.
///
/// What did NOT change: refusing a nonzero donation is still rejected, on
/// `CloseSeal`'s own documented lesson -- anyone can transfer one lamport into
/// a Trading-owned PDA, so a refusal would let a griefer strand any replay, and
/// the market behind it, permanently, for nothing.
pub const DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1: u64 =
    dclutch_market::protocol_parameters::PROTOCOL_GENESIS_CLOSER_REWARD_CAP_LAMPORTS_V1;

/// Exact number of scalar registers in the authenticated close artifacts.
pub const DIRECT_CLOSE_MAKER_SCALAR_COUNT_V1: u16 = 10;
/// Exact number of identity registers in the authenticated close artifacts.
pub const DIRECT_CLOSE_MAKER_IDENTITY_COUNT_V1: u16 = 2;
/// Profile-relative account index of the composite Direct root.
pub const DIRECT_CLOSE_MAKER_ROOT_ACCOUNT_V1: u16 = 0;
/// Scalar register carrying the caller-selected request selector.
pub const DIRECT_CLOSE_MAKER_SELECTOR_SCALAR_V1: u16 = 0;
/// Scalar register carrying the root tail magic word.
pub const DIRECT_CLOSE_MAKER_ROOT_MAGIC_SCALAR_V1: u16 = 1;
/// Scalar register carrying the root version/phase/reserved header word.
pub const DIRECT_CLOSE_MAKER_ROOT_HEADER_SCALAR_V1: u16 = 2;
/// Scalar register carrying the open maker-root count.
pub const DIRECT_CLOSE_MAKER_MAKER_COUNT_SCALAR_V1: u16 = 3;
/// Scalar register carrying the root's observed lamports.
pub const DIRECT_CLOSE_MAKER_ROOT_LAMPORTS_SCALAR_V1: u16 = 4;
/// Scalar register holding the expected close selector constant.
pub const DIRECT_CLOSE_MAKER_EXPECTED_SELECTOR_SCALAR_V1: u16 = 5;
/// Scalar register holding the expected root magic constant.
pub const DIRECT_CLOSE_MAKER_EXPECTED_MAGIC_SCALAR_V1: u16 = 6;
/// Scalar register holding the expected RETIRING header word constant.
pub const DIRECT_CLOSE_MAKER_RETIRING_HEADER_SCALAR_V1: u16 = 7;
/// Scalar register holding the constant one.
pub const DIRECT_CLOSE_MAKER_ONE_SCALAR_V1: u16 = 8;
/// Scalar register receiving the decremented maker-root count.
///
/// This register is the missing decrement itself: the released transition
/// computes it with `sub_into` after `nonzero` refuses an already-drained
/// count, and the released effect writes it back to the count word. The chain
/// executable cross-checks the same number through
/// `close_maker_replay_v2` -- two authors, one equality.
pub const DIRECT_CLOSE_MAKER_POST_COUNT_SCALAR_V1: u16 = 9;
/// Identity register carrying this Trading Program.
pub const DIRECT_CLOSE_MAKER_TRADING_IDENTITY_V1: u16 = 0;
/// Identity register carrying the composite root address.
pub const DIRECT_CLOSE_MAKER_ROOT_IDENTITY_V1: u16 = 1;

/// Exact top-level account count.
pub const DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1: usize = 22;

/// Top-level index of the composite Direct root (writable).
pub const DIRECT_CLOSE_MAKER_ROOT_TOP_ACCOUNT_V1: usize = 0;
/// Top-level index of the canonical Core Market.
pub const DIRECT_CLOSE_MAKER_MARKET_ACCOUNT_V1: usize = 1;
/// Top-level index of the persisted capability-manifest raw record.
pub const DIRECT_CLOSE_MAKER_MANIFEST_RAW_ACCOUNT_V1: usize = 2;
/// Top-level index of the finalized ProgramSet raw record.
pub const DIRECT_CLOSE_MAKER_PROGRAM_SET_RAW_ACCOUNT_V1: usize = 3;
/// Top-level index of the ProgramSet staging cursor.
pub const DIRECT_CLOSE_MAKER_PROGRAM_SET_STAGING_ACCOUNT_V1: usize = 4;
/// Top-level index of the selected close descriptor raw record.
pub const DIRECT_CLOSE_MAKER_DESCRIPTOR_RAW_ACCOUNT_V1: usize = 5;
/// Top-level index of the close descriptor staging cursor.
pub const DIRECT_CLOSE_MAKER_DESCRIPTOR_STAGING_ACCOUNT_V1: usize = 6;
/// Top-level index of the Direct config raw record.
pub const DIRECT_CLOSE_MAKER_CONFIG_RAW_ACCOUNT_V1: usize = 7;
/// Top-level index of the Direct config staging cursor.
pub const DIRECT_CLOSE_MAKER_CONFIG_STAGING_ACCOUNT_V1: usize = 8;
/// Top-level index of the close AccountProfile raw record.
pub const DIRECT_CLOSE_MAKER_PROFILE_RAW_ACCOUNT_V1: usize = 9;
/// Top-level index of the close AccountProfile staging cursor.
pub const DIRECT_CLOSE_MAKER_PROFILE_STAGING_ACCOUNT_V1: usize = 10;
/// Top-level index of the close EffectProgram raw record.
pub const DIRECT_CLOSE_MAKER_EFFECT_RAW_ACCOUNT_V1: usize = 11;
/// Top-level index of the close EffectProgram staging cursor.
pub const DIRECT_CLOSE_MAKER_EFFECT_STAGING_ACCOUNT_V1: usize = 12;
/// Top-level index of the Registry-owned activation cache.
pub const DIRECT_CLOSE_MAKER_ACTIVATION_CACHE_ACCOUNT_V1: usize = 13;
/// Top-level index of the Core program executable.
pub const DIRECT_CLOSE_MAKER_CORE_PROGRAM_ACCOUNT_V1: usize = 14;
/// Top-level index of the Core ProgramData account.
pub const DIRECT_CLOSE_MAKER_CORE_PROGRAMDATA_ACCOUNT_V1: usize = 15;
/// Top-level index of this Trading program executable.
pub const DIRECT_CLOSE_MAKER_TRADING_PROGRAM_ACCOUNT_V1: usize = 16;
/// Top-level index of the Trading ProgramData account.
pub const DIRECT_CLOSE_MAKER_TRADING_PROGRAMDATA_ACCOUNT_V1: usize = 17;
/// Top-level index of the Registry program executable.
pub const DIRECT_CLOSE_MAKER_REGISTRY_ACCOUNT_V1: usize = 18;
/// Top-level index of the Rent sysvar.
pub const DIRECT_CLOSE_MAKER_RENT_ACCOUNT_V1: usize = 19;
/// Top-level index of the maker replay being closed (writable).
pub const DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1: usize = 20;
/// Top-level index of the recorded rent-owner destination wallet (writable).
pub const DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1: usize = 21;

/// Return the exact `(writable, executable)` membrane for one account index.
#[must_use]
pub const fn direct_close_maker_account_privileges_v1(index: usize) -> Option<(bool, bool)> {
    if index >= DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1 {
        None
    } else {
        Some((
            index == DIRECT_CLOSE_MAKER_ROOT_TOP_ACCOUNT_V1
                || index == DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1
                || index == DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1,
            index == DIRECT_CLOSE_MAKER_CORE_PROGRAM_ACCOUNT_V1
                || index == DIRECT_CLOSE_MAKER_TRADING_PROGRAM_ACCOUNT_V1
                || index == DIRECT_CLOSE_MAKER_REGISTRY_ACCOUNT_V1,
        ))
    }
}

/// Width of the header every close-maker record starts with: magic, version, two
/// reserved bytes, selector.
///
/// Pinned to the first field's coordinate below rather than written twice -- a
/// header that grew without the fields moving would overwrite the first one.
const HEADER_BYTES: usize = 16;
const MARKET_OFFSET: usize = 16;
const MAKER_OFFSET: usize = 48;
const GENERATION_OFFSET: usize = 80;
const REQUEST_RESERVED_OFFSET: usize = 88;
const REQUEST_RESERVED_BYTES: usize = 8;

const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 16;
const RECEIPT_MARKET_OFFSET: usize = 48;
const RECEIPT_MAKER_OFFSET: usize = 80;
const RECEIPT_MAKER_ROOT_OFFSET: usize = 112;
const RECEIPT_RENT_OWNER_OFFSET: usize = 144;
const RECEIPT_POST_ROOT_DIGEST_OFFSET: usize = 176;
const RECEIPT_RENT_PRINCIPAL_OFFSET: usize = 208;
const RECEIPT_DONATION_OFFSET: usize = 216;
const RECEIPT_CLOSER_REWARD_OFFSET: usize = 224;
const RECEIPT_TOTAL_CREDIT_OFFSET: usize = 232;
const RECEIPT_REMAINING_COUNT_OFFSET: usize = 240;

/// Stable request/receipt refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCloseMakerErrorV1 {
    /// A wire did not have its one exact width.
    InvalidLength,
    /// Magic, version, or selector selected another route.
    InvalidHeader,
    /// Reserved bytes were noncanonical.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// The receipt's refund arithmetic did not conserve the observed balance.
    InvalidRefund,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, DirectCloseMakerErrorV1>;

/// Exact permissionless close-maker request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerRequestV1 {
    /// Canonical Core Market PDA whose Direct root the close drains.
    pub market: [u8; 32],
    /// The maker whose replay closes.
    pub maker: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
}

impl DirectCloseMakerRequestV1 {
    /// Validate required identities.
    pub fn new(self) -> Result<Self> {
        require_nonzero(self.market)?;
        require_nonzero(self.maker)?;
        Ok(self)
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(
            input,
            DIRECT_CLOSE_MAKER_REQUEST_MAGIC_V1,
            DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1,
        )?;
        if input.get(REQUEST_RESERVED_OFFSET..REQUEST_RESERVED_OFFSET + REQUEST_RESERVED_BYTES)
            != Some(&[0; REQUEST_RESERVED_BYTES])
        {
            return Err(DirectCloseMakerErrorV1::NonCanonical);
        }
        Self {
            market: array(input, MARKET_OFFSET)?,
            maker: array(input, MAKER_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical request.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1];
        output[..HEADER_BYTES].copy_from_slice(&header(DIRECT_CLOSE_MAKER_REQUEST_MAGIC_V1));
        output[MARKET_OFFSET..MARKET_OFFSET + 32].copy_from_slice(&self.market);
        output[MAKER_OFFSET..MAKER_OFFSET + 32].copy_from_slice(&self.maker);
        output[GENERATION_OFFSET..GENERATION_OFFSET + 8]
            .copy_from_slice(&self.generation.to_le_bytes());
        Ok(output)
    }
}

/// Exact Trading acknowledgment of one closed maker replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCloseMakerReceiptV1 {
    /// SHA-256 of the complete top-level request.
    pub request_digest: [u8; 32],
    /// Canonical Core Market.
    pub market: [u8; 32],
    /// The maker whose replay closed.
    pub maker: [u8; 32],
    /// The closed Trading maker replay address.
    pub maker_root: [u8; 32],
    /// The immutably recorded beneficiary the balance reached.
    pub rent_owner: [u8; 32],
    /// SHA-256 of the exact root bytes after the count decrement.
    pub post_root_digest: [u8; 32],
    /// Historical account-rent principal, exactly as recorded at first use.
    pub rent_principal: u64,
    /// Lamports above principal, explicitly not fees or reserves.
    pub unclassified_donation: u64,
    /// The permissionless closer's carve, out of the donation slice alone.
    pub closer_reward: u64,
    /// Exact total lamports credited to the beneficiary.
    pub total_credit: u64,
    /// Open maker roots still standing after this close.
    pub remaining_open_maker_roots: u64,
}

impl DirectCloseMakerReceiptV1 {
    /// Validate identities and the refund's own conservation arithmetic.
    pub fn new(self) -> Result<Self> {
        for value in [
            self.request_digest,
            self.market,
            self.maker,
            self.maker_root,
            self.rent_owner,
            self.post_root_digest,
        ] {
            require_nonzero(value)?;
        }
        // Conservation, and the ruling's own bound, in one place: the whole
        // observed balance is exactly the carve plus what the beneficiary
        // received, and the carve came out of the donation slice, so the
        // beneficiary still received at least everything the maker put in. A
        // receipt claiming a larger carve than the donation is a receipt for a
        // close that took principal, and it refuses here.
        if self.rent_principal == 0
            || self.closer_reward > self.unclassified_donation
            || self.rent_principal.checked_add(self.unclassified_donation)
                != self.closer_reward.checked_add(self.total_credit)
        {
            return Err(DirectCloseMakerErrorV1::InvalidRefund);
        }
        Ok(self)
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(
            input,
            DIRECT_CLOSE_MAKER_RECEIPT_MAGIC_V1,
            DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1,
        )?;
        Self {
            request_digest: array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            market: array(input, RECEIPT_MARKET_OFFSET)?,
            maker: array(input, RECEIPT_MAKER_OFFSET)?,
            maker_root: array(input, RECEIPT_MAKER_ROOT_OFFSET)?,
            rent_owner: array(input, RECEIPT_RENT_OWNER_OFFSET)?,
            post_root_digest: array(input, RECEIPT_POST_ROOT_DIGEST_OFFSET)?,
            rent_principal: u64_at(input, RECEIPT_RENT_PRINCIPAL_OFFSET)?,
            unclassified_donation: u64_at(input, RECEIPT_DONATION_OFFSET)?,
            closer_reward: u64_at(input, RECEIPT_CLOSER_REWARD_OFFSET)?,
            total_credit: u64_at(input, RECEIPT_TOTAL_CREDIT_OFFSET)?,
            remaining_open_maker_roots: u64_at(input, RECEIPT_REMAINING_COUNT_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1];
        output[..HEADER_BYTES].copy_from_slice(&header(DIRECT_CLOSE_MAKER_RECEIPT_MAGIC_V1));
        let identities = [
            self.request_digest,
            self.market,
            self.maker,
            self.maker_root,
            self.rent_owner,
            self.post_root_digest,
        ];
        debug_assert!(
            RECEIPT_RENT_PRINCIPAL_OFFSET - RECEIPT_REQUEST_DIGEST_OFFSET == identities.len() * 32
        );
        for (slot, value) in output[RECEIPT_REQUEST_DIGEST_OFFSET..RECEIPT_RENT_PRINCIPAL_OFFSET]
            .chunks_exact_mut(32)
            .zip(identities.iter())
        {
            slot.copy_from_slice(value);
        }
        let scalars = [
            self.rent_principal,
            self.unclassified_donation,
            self.closer_reward,
            self.total_credit,
            self.remaining_open_maker_roots,
        ];
        debug_assert!(
            DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1 - RECEIPT_RENT_PRINCIPAL_OFFSET
                == scalars.len() * 8
        );
        for (slot, value) in output
            [RECEIPT_RENT_PRINCIPAL_OFFSET..DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1]
            .chunks_exact_mut(8)
            .zip(scalars.iter())
        {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        Ok(output)
    }
}

/// Detect the exact close-maker request family without a partial header.
#[must_use]
pub fn is_direct_close_maker_v1(input: &[u8]) -> bool {
    input.len() == DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1
        && input.get(..8) == Some(DIRECT_CLOSE_MAKER_REQUEST_MAGIC_V1.as_slice())
}

/// Derive the canonical close-maker route context tag.
#[must_use]
pub fn direct_close_maker_context_v1(
    market: [u8; 32],
    maker: [u8; 32],
    generation: u64,
) -> [u8; 32] {
    digestv(&[
        b"dclutch/direct/close-maker-context/v1",
        &market,
        &maker,
        &generation.to_le_bytes(),
    ])
}

/// The shared sixteen-byte close-maker record header, as a value.
///
/// It used to be three fixed-offset writes into a `&mut [u8]` the caller
/// promised was wide enough. Every caller passes a fixed-width array, so the
/// promise was always kept -- but the SLICE parameter threw the width away, and
/// three writes that cannot be checked is what `indexing_slicing` is naming.
/// Building the header at its own width instead restores the bound: the array
/// below is exactly as long as the ranges written into it, the compiler sees
/// that, and each record copies a whole header into a constant range of its own
/// constant-width buffer.
fn header(magic: [u8; 8]) -> [u8; HEADER_BYTES] {
    let mut head = [0_u8; HEADER_BYTES];
    head[..8].copy_from_slice(&magic);
    head[8..10].copy_from_slice(&DIRECT_CLOSE_MAKER_VERSION_V1.to_le_bytes());
    head[12..16].copy_from_slice(&DIRECT_CLOSE_MAKER_SELECTOR_V1.to_le_bytes());
    head
}

fn require_header(input: &[u8], magic: [u8; 8], width: usize) -> Result<()> {
    if input.len() != width {
        return Err(DirectCloseMakerErrorV1::InvalidLength);
    }
    if input.get(..8) != Some(magic.as_slice())
        || u16_at(input, 8)? != DIRECT_CLOSE_MAKER_VERSION_V1
        || input.get(10..12) != Some(&[0, 0])
        || u32_at(input, 12)? != DIRECT_CLOSE_MAKER_SELECTOR_V1
    {
        return Err(DirectCloseMakerErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(DirectCloseMakerErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    input
        .get(offset..offset + 32)
        .ok_or(DirectCloseMakerErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| DirectCloseMakerErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    input
        .get(offset..offset + 2)
        .ok_or(DirectCloseMakerErrorV1::InvalidLength)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| DirectCloseMakerErrorV1::InvalidLength)
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    input
        .get(offset..offset + 4)
        .ok_or(DirectCloseMakerErrorV1::InvalidLength)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| DirectCloseMakerErrorV1::InvalidLength)
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    input
        .get(offset..offset + 8)
        .ok_or(DirectCloseMakerErrorV1::InvalidLength)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| DirectCloseMakerErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    extern crate std;

    use super::*;
    use dclutch_sha256_adapter::digest;

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn request() -> DirectCloseMakerRequestV1 {
        DirectCloseMakerRequestV1 {
            market: id(1),
            maker: id(2),
            generation: 9,
        }
    }

    fn receipt() -> DirectCloseMakerReceiptV1 {
        DirectCloseMakerReceiptV1 {
            request_digest: id(3),
            market: id(1),
            maker: id(2),
            maker_root: id(4),
            rent_owner: id(5),
            post_root_digest: id(6),
            rent_principal: 100,
            unclassified_donation: 11,
            closer_reward: 0,
            total_credit: 111,
            remaining_open_maker_roots: 0,
        }
    }

    #[test]
    fn schema_id_and_selector_are_frozen() {
        assert_eq!(
            digest(DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_PREIMAGE_V1),
            DIRECT_CLOSE_MAKER_REQUEST_SCHEMA_ID_V1
        );
        assert_eq!(DIRECT_CLOSE_MAKER_SELECTOR_V1, 0xffff_ff04);
        // The fee-settlement wire owns 0xffff_ff03 and must never alias.
        assert_ne!(
            DIRECT_CLOSE_MAKER_SELECTOR_V1,
            crate::fee_settlement_v1::DIRECT_FEE_SETTLEMENT_SELECTOR_V1
        );
    }

    #[test]
    fn request_and_receipt_are_exact_hostile_decodable_wires() {
        let request = request();
        let bytes = request.to_bytes().expect("request");
        assert_eq!(DirectCloseMakerRequestV1::decode(&bytes), Ok(request));
        assert!(is_direct_close_maker_v1(&bytes));
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().expect("selector")),
            DIRECT_CLOSE_MAKER_SELECTOR_V1
        );

        let receipt = receipt();
        let receipt_bytes = receipt.to_bytes().expect("receipt");
        assert_eq!(
            DirectCloseMakerReceiptV1::decode(&receipt_bytes),
            Ok(receipt)
        );

        for offset in [0, 8, 10, 12] {
            let mut hostile = bytes;
            hostile[offset] ^= 1;
            assert!(DirectCloseMakerRequestV1::decode(&hostile).is_err());
        }
        assert_eq!(
            DirectCloseMakerRequestV1::decode(
                bytes
                    .get(..DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1 - 1)
                    .expect("prefix")
            ),
            Err(DirectCloseMakerErrorV1::InvalidLength)
        );
    }

    #[test]
    fn a_nonzero_reserved_tail_refuses() {
        let bytes = request().to_bytes().expect("request");
        for offset in REQUEST_RESERVED_OFFSET..DIRECT_CLOSE_MAKER_REQUEST_BYTES_V1 {
            let mut hostile = bytes;
            hostile[offset] = 1;
            assert_eq!(
                DirectCloseMakerRequestV1::decode(&hostile),
                Err(DirectCloseMakerErrorV1::NonCanonical),
                "reserved byte {offset}",
            );
        }
    }

    /// A receipt claiming a refund that does not conserve the observed balance
    /// is not a receipt for anything this route can have done -- the Lean twin
    /// is `maker_close_refund_conserved`.
    #[test]
    fn the_receipt_refuses_a_refund_it_could_not_have_produced() {
        assert!(receipt().new().is_ok());
        assert_eq!(
            DirectCloseMakerReceiptV1 {
                total_credit: 110,
                ..receipt()
            }
            .new(),
            Err(DirectCloseMakerErrorV1::InvalidRefund)
        );
        assert_eq!(
            DirectCloseMakerReceiptV1 {
                rent_principal: 0,
                unclassified_donation: 111,
                ..receipt()
            }
            .new(),
            Err(DirectCloseMakerErrorV1::InvalidRefund)
        );
    }

    /// RULING D1 ITEM 4 on the wire: a receipt may report a carve, and it may
    /// not report one larger than the donation it claims to have come from.
    ///
    /// The hostile is the one the ruling names -- a larger carve refuses -- and
    /// it is checked at the exact discriminant with a control one lamport away,
    /// so it cannot pass on the conservation conjunct beside it.
    #[test]
    fn a_receipt_carving_more_than_the_donation_refuses() {
        // The carve at exactly the donation: legal, and the beneficiary
        // receives exactly the principal.
        let whole = DirectCloseMakerReceiptV1 {
            closer_reward: 11,
            total_credit: 100,
            ..receipt()
        };
        assert!(whole.new().is_ok());
        assert_eq!(whole.rent_principal, whole.total_credit);
        assert_eq!(
            DirectCloseMakerReceiptV1::decode(&whole.to_bytes().expect("encode")),
            Ok(whole),
        );

        // One lamport more: the carve has reached into principal and it
        // refuses. Both conjuncts would catch it, and the bound is stated
        // separately so the refusal names the right accusation.
        assert_eq!(
            DirectCloseMakerReceiptV1 {
                closer_reward: 12,
                total_credit: 99,
                ..receipt()
            }
            .new(),
            Err(DirectCloseMakerErrorV1::InvalidRefund)
        );
        // And a carve that does not come out of anything: conservation holds
        // for the numbers, but the carve exceeds the donation.
        assert_eq!(
            DirectCloseMakerReceiptV1 {
                unclassified_donation: 0,
                closer_reward: 11,
                total_credit: 100,
                rent_principal: 111,
                ..receipt()
            }
            .new(),
            Err(DirectCloseMakerErrorV1::InvalidRefund)
        );
    }

    /// The carve ceiling this route passes is zero because its FRAME admits no
    /// closer, not because ruling D1 item 4 says zero -- and the frame fact is
    /// asserted beside it so the two cannot be confused.
    #[test]
    fn the_carve_ceiling_is_zero_because_the_frame_admits_no_closer() {
        assert_eq!(DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1, 0);
        // The governed record's genesis is its single author.
        assert_eq!(
            DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1,
            dclutch_market::protocol_parameters::PROTOCOL_GENESIS_CLOSER_REWARD_CAP_LAMPORTS_V1,
        );
        // Twenty-two accounts, none of them a closer: indices 20 and 21 are the
        // replay and the recorded rent owner, and there is no index 22.
        assert_eq!(DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1, 22);
        assert_eq!(
            direct_close_maker_account_privileges_v1(DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1),
            None,
        );
    }

    #[test]
    fn account_privileges_are_the_exact_close_membrane() {
        let mut writable = 0;
        let mut executable = 0;
        for index in 0..DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1 {
            let (w, x) = direct_close_maker_account_privileges_v1(index).expect("privileges");
            writable += usize::from(w);
            executable += usize::from(x);
            assert!(!(w && x), "no account is both writable and executable");
        }
        assert_eq!(writable, 3, "root, replay, and rent owner");
        assert_eq!(executable, 3, "core, trading, registry");
        assert!(
            direct_close_maker_account_privileges_v1(DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1).is_none()
        );
    }
}
