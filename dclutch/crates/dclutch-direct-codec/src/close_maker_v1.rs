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
pub const DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1: usize = 240;
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

/// The permissionless closer's reward, named while ruling 1 of the cohort-9
/// review (`COHORT9_PLAN_REVIEW_2026_08_31.md` section 8) is pending.
///
/// Until ruled, the whole observed balance follows the landed Lean plan
/// (`MakerClosePlan`: `totalCredit` to `rentOwner`, refund conservation
/// proved), so the `unclassified_donation` slice reaches the recorded
/// `rent_owner` rather than being refused: refusing a nonzero donation would
/// hand a griefer a 1-lamport transfer that strands the replay -- and the
/// market behind it -- permanently, the exact outcome `CloseSeal`'s own cap
/// commentary documents against. A ruled closer reward is a later carve out of
/// the donation slice alone; the principal is the maker's own money and never
/// part of it.
pub const DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1: u64 = 0;

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
const RECEIPT_TOTAL_CREDIT_OFFSET: usize = 224;
const RECEIPT_REMAINING_COUNT_OFFSET: usize = 232;

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
        write_header(&mut output, DIRECT_CLOSE_MAKER_REQUEST_MAGIC_V1);
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
        if self.rent_principal == 0
            || self.rent_principal.checked_add(self.unclassified_donation)
                != Some(self.total_credit)
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
            total_credit: u64_at(input, RECEIPT_TOTAL_CREDIT_OFFSET)?,
            remaining_open_maker_roots: u64_at(input, RECEIPT_REMAINING_COUNT_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_CLOSE_MAKER_RECEIPT_BYTES_V1];
        write_header(&mut output, DIRECT_CLOSE_MAKER_RECEIPT_MAGIC_V1);
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

fn write_header(output: &mut [u8], magic: [u8; 8]) {
    output[..8].copy_from_slice(&magic);
    output[8..10].copy_from_slice(&DIRECT_CLOSE_MAKER_VERSION_V1.to_le_bytes());
    output[12..16].copy_from_slice(&DIRECT_CLOSE_MAKER_SELECTOR_V1.to_le_bytes());
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

    /// The closer reward is deliberately zero until ruling 1 lands; the whole
    /// balance follows the landed Lean plan to the recorded rent owner.
    #[test]
    fn the_closer_reward_is_named_and_zero_until_ruled() {
        assert_eq!(DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1, 0);
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
