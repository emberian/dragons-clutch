//! Exact wire for the permissionless Direct fee-settlement transaction.
//!
//! This is the second transaction of `docs/design/FEE_SECOND_TRANSACTION_V1.md`.
//! The fill records `fee_owed` on the buyer's maker replay and leaves the
//! residual SPL delegation standing; this wire asks Trading to move exactly
//! that obligation to the Market's configured fee recipient and clear the
//! field.
//!
//! **The request carries no economic value at all** -- no amount, no
//! destination address, no revision, no digest. Every one of those is read out
//! of program-owned state by the route (§1.4 of that design), so a stranger's
//! submission is effect-free beyond the outcome the fill already fixed. What
//! the wire carries is the COORDINATE -- which market, which maker -- plus
//! three bump hints the submitter mined off chain, each of which is reproduced
//! and checked rather than trusted.
//!
//! It also, deliberately, carries **no expected-state digest**. The two
//! sibling permissionless routes (`replay_setup_v1`, `token_setup_v1`) both pin
//! the Market and root bytes they read. Those routes run once, at first use,
//! against state nothing else is moving. This one runs against a Direct root
//! that every fill in the market rewrites, so a pinned digest would make the
//! crank re-derivable-only-between-blocks and griefable by any unrelated
//! trade -- and the obligation it settles is undeadlined, so a route that can
//! be raced forever is a route that can strand a maker forever.

use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReplayV1, CustodyRequestV1,
    DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_sha256_adapter::digestv;

/// High selector reserved for Direct fee settlement.
pub const DIRECT_FEE_SETTLEMENT_SELECTOR_V1: u32 = 0xffff_ff03;
/// Canonical request magic.
pub const DIRECT_FEE_SETTLEMENT_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTDFS1";
/// Canonical receipt magic.
pub const DIRECT_FEE_SETTLEMENT_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTDFR1";
/// Exact request width.
pub const DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1: usize = 96;
/// Exact receipt width.
pub const DIRECT_FEE_SETTLEMENT_RECEIPT_BYTES_V1: usize = 360;
/// Implemented wire version.
pub const DIRECT_FEE_SETTLEMENT_VERSION_V1: u16 = 1;
/// Domain separating the synthetic order tag carried by the fee request.
pub const DIRECT_FEE_SETTLEMENT_ORDER_DOMAIN_V1: &[u8] = b"dclutch/direct/fee-settlement-order/v1";

/// The design's §4.4 ceiling for this wire, asserted rather than described.
const _: () = assert!(DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1 < 128);

/// Width of one identity field.
const IDENTITY_BYTES: usize = 32;
/// Width of one little-endian u64 field.
const SCALAR_BYTES: usize = 8;

/// Width of the header every fee-settlement record starts with: magic, version, two
/// reserved bytes, selector.
///
/// Pinned to the first field's coordinate below rather than written twice -- a
/// header that grew without the fields moving would overwrite the first one.
const HEADER_BYTES: usize = 16;
const MARKET_OFFSET: usize = 16;
const MAKER_OFFSET: usize = 48;
const GENERATION_OFFSET: usize = 80;
const CALLER_AUTHORITY_BUMP_OFFSET: usize = 88;
const CUSTODY_REPLAY_BUMP_OFFSET: usize = 89;
const CUSTODY_TRANSFER_BUMP_OFFSET: usize = 90;
const REQUEST_RESERVED_OFFSET: usize = 91;
const REQUEST_RESERVED_BYTES: usize = 5;

const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 16;
const RECEIPT_MARKET_OFFSET: usize = 48;
const RECEIPT_MAKER_OFFSET: usize = 80;
const RECEIPT_MAKER_ROOT_OFFSET: usize = 112;
const RECEIPT_CUSTODY_REPLAY_OFFSET: usize = 144;
const RECEIPT_FEE_SOURCE_OFFSET: usize = 176;
const RECEIPT_FEE_DESTINATION_OFFSET: usize = 208;
const RECEIPT_FEE_RECIPIENT_OFFSET: usize = 240;
const RECEIPT_CUSTODY_REQUEST_DIGEST_OFFSET: usize = 272;
const RECEIPT_CUSTODY_POSTSTATE_OFFSET: usize = 304;
const RECEIPT_SETTLED_AMOUNT_OFFSET: usize = 336;
const RECEIPT_EXPECTED_REVISION_OFFSET: usize = 344;
const RECEIPT_RESULTING_REVISION_OFFSET: usize = 352;

/// Stable hostile-decode refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectFeeSettlementErrorV1 {
    /// A wire had another exact width.
    InvalidLength,
    /// Magic, version, or selector selected another route.
    InvalidHeader,
    /// Reserved bytes were nonzero.
    NonCanonical,
    /// A required identity was zero.
    ZeroIdentity,
    /// The settled amount was zero, or the revision did not step by one.
    InvalidSettlement,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, DirectFeeSettlementErrorV1>;

/// Exact permissionless fee-settlement request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeeSettlementRequestV1 {
    /// Canonical Core Market PDA whose config priced the fee.
    pub market: [u8; 32],
    /// The debtor: the maker whose replay carries the obligation.
    pub maker: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Mined bump for the Trading caller authority; zero searches.
    pub caller_authority_bump: u8,
    /// Mined bump for Custody's own replay PDA; zero searches.
    pub custody_replay_bump: u8,
    /// Mined bump for Custody's transfer authority PDA; zero searches.
    pub custody_transfer_bump: u8,
}

impl DirectFeeSettlementRequestV1 {
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
            DIRECT_FEE_SETTLEMENT_REQUEST_MAGIC_V1,
            DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1,
        )?;
        if input.get(REQUEST_RESERVED_OFFSET..REQUEST_RESERVED_OFFSET + REQUEST_RESERVED_BYTES)
            != Some(&[0; REQUEST_RESERVED_BYTES])
        {
            return Err(DirectFeeSettlementErrorV1::NonCanonical);
        }
        Self {
            market: array(input, MARKET_OFFSET)?,
            maker: array(input, MAKER_OFFSET)?,
            generation: u64_at(input, GENERATION_OFFSET)?,
            caller_authority_bump: byte_at(input, CALLER_AUTHORITY_BUMP_OFFSET)?,
            custody_replay_bump: byte_at(input, CUSTODY_REPLAY_BUMP_OFFSET)?,
            custody_transfer_bump: byte_at(input, CUSTODY_TRANSFER_BUMP_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical request.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1];
        output[..HEADER_BYTES].copy_from_slice(&header(DIRECT_FEE_SETTLEMENT_REQUEST_MAGIC_V1));
        output[MARKET_OFFSET..MARKET_OFFSET + IDENTITY_BYTES].copy_from_slice(&self.market);
        output[MAKER_OFFSET..MAKER_OFFSET + IDENTITY_BYTES].copy_from_slice(&self.maker);
        output[GENERATION_OFFSET..GENERATION_OFFSET + SCALAR_BYTES]
            .copy_from_slice(&self.generation.to_le_bytes());
        output[CALLER_AUTHORITY_BUMP_OFFSET] = self.caller_authority_bump;
        output[CUSTODY_REPLAY_BUMP_OFFSET] = self.custody_replay_bump;
        output[CUSTODY_TRANSFER_BUMP_OFFSET] = self.custody_transfer_bump;
        Ok(output)
    }

    /// The caller-authority bump as the hint type the derivation takes.
    ///
    /// Zero is not a legal PDA bump, so it is the wire's way of saying
    /// "unmined, search for it" -- the same convention `HotBumpHintsV1` and
    /// `CustodyBumpRelayV1` use, and the reason no separate presence flag
    /// exists on this record.
    #[must_use]
    pub const fn caller_authority_hint(self) -> Option<u8> {
        if self.caller_authority_bump == 0 {
            None
        } else {
            Some(self.caller_authority_bump)
        }
    }

    /// The two bumps Custody derives for itself, in relay order.
    #[must_use]
    pub const fn custody_relay(self) -> [u8; 2] {
        [self.custody_replay_bump, self.custody_transfer_bump]
    }
}

/// Exact Trading acknowledgment of one settled Direct fee.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeeSettlementReceiptV1 {
    /// SHA-256 of the complete top-level request.
    pub request_digest: [u8; 32],
    /// Canonical Core Market.
    pub market: [u8; 32],
    /// The debtor whose obligation this cleared.
    pub maker: [u8; 32],
    /// The debtor's Trading maker replay, which is also the Custody context.
    pub maker_root: [u8; 32],
    /// The Custody replay this settlement advanced.
    pub custody_replay: [u8; 32],
    /// Token account the fee left, owned by the debtor.
    pub fee_source: [u8; 32],
    /// Token account the fee entered, owned by the configured recipient.
    pub fee_destination: [u8; 32],
    /// The immutable config's fee recipient.
    pub fee_recipient: [u8; 32],
    /// SHA-256 of the exact derived Custody request.
    pub custody_request_digest: [u8; 32],
    /// Custody poststate commitment stored in the replay.
    pub custody_poststate: [u8; 32],
    /// Exactly the amount the maker replay recorded, never "whatever is delegated".
    pub settled_amount: u64,
    /// Custody replay revision this settlement consumed.
    pub expected_revision: u64,
    /// Custody replay revision it left behind.
    pub resulting_revision: u64,
}

impl DirectFeeSettlementReceiptV1 {
    /// Validate all required identities and the settlement's own arithmetic.
    pub fn new(self) -> Result<Self> {
        for value in [
            self.request_digest,
            self.market,
            self.maker,
            self.maker_root,
            self.custody_replay,
            self.fee_source,
            self.fee_destination,
            self.fee_recipient,
            self.custody_request_digest,
            self.custody_poststate,
        ] {
            require_nonzero(value)?;
        }
        if self.settled_amount == 0
            || self.expected_revision.checked_add(1) != Some(self.resulting_revision)
        {
            return Err(DirectFeeSettlementErrorV1::InvalidSettlement);
        }
        Ok(self)
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_header(
            input,
            DIRECT_FEE_SETTLEMENT_RECEIPT_MAGIC_V1,
            DIRECT_FEE_SETTLEMENT_RECEIPT_BYTES_V1,
        )?;
        Self {
            request_digest: array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            market: array(input, RECEIPT_MARKET_OFFSET)?,
            maker: array(input, RECEIPT_MAKER_OFFSET)?,
            maker_root: array(input, RECEIPT_MAKER_ROOT_OFFSET)?,
            custody_replay: array(input, RECEIPT_CUSTODY_REPLAY_OFFSET)?,
            fee_source: array(input, RECEIPT_FEE_SOURCE_OFFSET)?,
            fee_destination: array(input, RECEIPT_FEE_DESTINATION_OFFSET)?,
            fee_recipient: array(input, RECEIPT_FEE_RECIPIENT_OFFSET)?,
            custody_request_digest: array(input, RECEIPT_CUSTODY_REQUEST_DIGEST_OFFSET)?,
            custody_poststate: array(input, RECEIPT_CUSTODY_POSTSTATE_OFFSET)?,
            settled_amount: u64_at(input, RECEIPT_SETTLED_AMOUNT_OFFSET)?,
            expected_revision: u64_at(input, RECEIPT_EXPECTED_REVISION_OFFSET)?,
            resulting_revision: u64_at(input, RECEIPT_RESULTING_REVISION_OFFSET)?,
        }
        .new()
    }

    /// Encode one canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; DIRECT_FEE_SETTLEMENT_RECEIPT_BYTES_V1]> {
        Self::new(self)?;
        let mut output = [0_u8; DIRECT_FEE_SETTLEMENT_RECEIPT_BYTES_V1];
        output[..HEADER_BYTES].copy_from_slice(&header(DIRECT_FEE_SETTLEMENT_RECEIPT_MAGIC_V1));
        // Two gapless runs, exactly as `replay_setup_v1`'s receipt is written:
        // ten identities, then three little-endian u64s to the record's end.
        // The debug asserts are what keep `zip`'s truncation loud -- a field
        // added without widening the record would silently vanish rather than
        // panic -- and they compile out of the SBF release build.
        let identities = [
            self.request_digest,
            self.market,
            self.maker,
            self.maker_root,
            self.custody_replay,
            self.fee_source,
            self.fee_destination,
            self.fee_recipient,
            self.custody_request_digest,
            self.custody_poststate,
        ];
        debug_assert!(
            RECEIPT_SETTLED_AMOUNT_OFFSET.saturating_sub(RECEIPT_REQUEST_DIGEST_OFFSET)
                == identities.len().saturating_mul(IDENTITY_BYTES)
        );
        for (slot, value) in output[RECEIPT_REQUEST_DIGEST_OFFSET..RECEIPT_SETTLED_AMOUNT_OFFSET]
            .chunks_exact_mut(IDENTITY_BYTES)
            .zip(identities.iter())
        {
            slot.copy_from_slice(value);
        }
        let scalars = [
            self.settled_amount,
            self.expected_revision,
            self.resulting_revision,
        ];
        debug_assert!(
            DIRECT_FEE_SETTLEMENT_RECEIPT_BYTES_V1.saturating_sub(RECEIPT_SETTLED_AMOUNT_OFFSET)
                == scalars.len().saturating_mul(SCALAR_BYTES)
        );
        for (slot, value) in output
            [RECEIPT_SETTLED_AMOUNT_OFFSET..DIRECT_FEE_SETTLEMENT_RECEIPT_BYTES_V1]
            .chunks_exact_mut(SCALAR_BYTES)
            .zip(scalars.iter())
        {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        Ok(output)
    }
}

/// Detect the exact fee-settlement request family without a partial header.
#[must_use]
pub fn is_direct_fee_settlement_v1(input: &[u8]) -> bool {
    input.len() == DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1
        && input.get(..8) == Some(DIRECT_FEE_SETTLEMENT_REQUEST_MAGIC_V1.as_slice())
}

/// Derive the synthetic order tag the fee request carries.
///
/// `semantic.parent_request_digest` is the seller leg's digest, read straight
/// off `replay.last_request_digest` (design §1.4): tx2 continues that leg and
/// says so. `semantic.order` cannot be the same value, because two Custody
/// requests differing in nothing else would then differ in nothing at all --
/// and this one is not the same act. So the order tag is that parent digest
/// domain-separated by the settled coordinate, which makes each settlement's
/// request digest distinct from the leg it continues without inventing a fact.
#[must_use]
pub fn direct_fee_settlement_order_v1(
    parent_request_digest: [u8; 32],
    maker_root: [u8; 32],
    amount: u64,
) -> [u8; 32] {
    digestv(&[
        DIRECT_FEE_SETTLEMENT_ORDER_DOMAIN_V1,
        &parent_request_digest,
        &maker_root,
        &amount.to_le_bytes(),
    ])
}

/// The chain facts one fee settlement is projected from.
///
/// Every field is read off a program-owned account: the Custody replay carries
/// all seven binding values and the revision, the buyer's maker replay carries
/// the amount and the debtor, and the token accounts carry their own owners.
/// Nothing here can come from a caller's instruction data, which is what makes
/// a stranger's submission effect-free beyond the outcome the fill fixed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFeeProjectionV1 {
    /// The Custody replay as it stands after the fill, decoded.
    pub replay: CustodyReplayV1,
    /// Exactly what the buyer's maker replay records.
    pub fee_owed: u64,
    /// The debtor's collateral account.
    pub source: [u8; 32],
    /// The debtor, which owns `source`.
    pub source_owner: [u8; 32],
    /// A collateral account of the configured recipient.
    pub destination: [u8; 32],
    /// The immutable config's `fee_recipient`, which owns `destination`.
    pub destination_owner: [u8; 32],
    /// The Realm's collateral mint.
    pub mint: [u8; 32],
    /// The Realm's token program.
    pub token_program: [u8; 32],
    /// The Market's Custody transfer authority, which holds the delegation.
    pub custody_authority: [u8; 32],
}

/// Project the exact fee request one settlement presents to Custody.
///
/// This is `FEE_SECOND_TRANSACTION_V1` §1.4's field table, and it lives here
/// rather than inside the Trading route so that **the caller and the program
/// build the same bytes from the same function**. They have to: the Custody
/// caller authority's sixth seed is the digest of these bytes, so a caller that
/// reproduced the projection separately would be addressing a PDA nothing signs
/// the moment the two drifted by a byte -- and the refusal would name the
/// authority, not the drift.
///
/// **`total_debit` is the one delegated-allowance field no state a second
/// transaction reads can supply.** The fill's `buyer_debit` is spent and gone,
/// and the maker replay records the residue rather than the original. So the
/// settlement declares an atomic debit of exactly the obligation, which is the
/// truth from where it stands: `starts_atomic_debit` and `terminal` are derived
/// facts about the allowance arithmetic and not assertions about a transaction
/// (§1.3), and the number that decides whether anything moves --
/// `allowance_before` -- is compared by Custody against the LIVE delegation.
pub fn project_direct_fee_request_v1(
    projection: DirectFeeProjectionV1,
) -> Result<DelegatedCustodyRequestV2> {
    if projection.fee_owed == 0 {
        return Err(DirectFeeSettlementErrorV1::InvalidSettlement);
    }
    let replay = projection.replay;
    let resulting_revision = replay
        .next_revision
        .checked_add(1)
        .ok_or(DirectFeeSettlementErrorV1::InvalidSettlement)?;
    let request = DelegatedCustodyRequestV2 {
        custody: CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Trading,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::External,
            release_set: replay.release_set,
            market: replay.market,
            realm: replay.realm,
            context: replay.context,
            caller_program: replay.caller_program,
            semantic: ContextV1 {
                candidate: [0; 32],
                source_owner: projection.source_owner,
                destination_owner: projection.destination_owner,
                order: direct_fee_settlement_order_v1(
                    replay.last_request_digest,
                    replay.context,
                    projection.fee_owed,
                ),
                // The leg this settlement continues, written by the fill and
                // program-owned, which is what keeps `require_nonzero` on the
                // parent digest satisfied without a caller supplying anything.
                parent_request_digest: replay.last_request_digest,
                order_nonce: 0,
                generation: replay.generation,
                page_index: 0,
                execution_index: 0,
                transfer_index: 0,
            },
            source: projection.source,
            destination: projection.destination,
            source_vault_context: [0; 32],
            destination_vault_context: [0; 32],
            mint: projection.mint,
            token_program: projection.token_program,
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: replay.next_revision,
            resulting_revision,
            amount: projection.fee_owed,
            rent_lamports: 0,
        },
        starts_atomic_debit: true,
        terminal: true,
        delegate_before: projection.custody_authority,
        delegate_after: [0; 32],
        total_debit: projection.fee_owed,
        allowance_before: projection.fee_owed,
        allowance_after: 0,
    };
    request
        .validate()
        .map_err(|_| DirectFeeSettlementErrorV1::InvalidSettlement)?;
    Ok(request)
}

/// The shared sixteen-byte fee-settlement record header, as a value.
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
    head[8..10].copy_from_slice(&DIRECT_FEE_SETTLEMENT_VERSION_V1.to_le_bytes());
    head[12..16].copy_from_slice(&DIRECT_FEE_SETTLEMENT_SELECTOR_V1.to_le_bytes());
    head
}

fn require_header(input: &[u8], magic: [u8; 8], width: usize) -> Result<()> {
    if input.len() != width {
        return Err(DirectFeeSettlementErrorV1::InvalidLength);
    }
    if input.get(..8) != Some(magic.as_slice())
        || u16_at(input, 8)? != DIRECT_FEE_SETTLEMENT_VERSION_V1
        || input.get(10..12) != Some(&[0, 0])
        || u32_at(input, 12)? != DIRECT_FEE_SETTLEMENT_SELECTOR_V1
    {
        return Err(DirectFeeSettlementErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(DirectFeeSettlementErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    input
        .get(offset..offset + 32)
        .ok_or(DirectFeeSettlementErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| DirectFeeSettlementErrorV1::InvalidLength)
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(DirectFeeSettlementErrorV1::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    input
        .get(offset..offset + 2)
        .ok_or(DirectFeeSettlementErrorV1::InvalidLength)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| DirectFeeSettlementErrorV1::InvalidLength)
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    input
        .get(offset..offset + 4)
        .ok_or(DirectFeeSettlementErrorV1::InvalidLength)?
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| DirectFeeSettlementErrorV1::InvalidLength)
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    input
        .get(offset..offset + 8)
        .ok_or(DirectFeeSettlementErrorV1::InvalidLength)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| DirectFeeSettlementErrorV1::InvalidLength)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn request() -> DirectFeeSettlementRequestV1 {
        DirectFeeSettlementRequestV1 {
            market: id(1),
            maker: id(2),
            generation: 9,
            caller_authority_bump: 254,
            custody_replay_bump: 253,
            custody_transfer_bump: 252,
        }
    }

    fn receipt() -> DirectFeeSettlementReceiptV1 {
        DirectFeeSettlementReceiptV1 {
            request_digest: id(3),
            market: id(1),
            maker: id(2),
            maker_root: id(4),
            custody_replay: id(5),
            fee_source: id(6),
            fee_destination: id(7),
            fee_recipient: id(8),
            custody_request_digest: id(9),
            custody_poststate: id(10),
            settled_amount: 2,
            expected_revision: 8,
            resulting_revision: 9,
        }
    }

    #[test]
    fn request_and_receipt_are_exact_hostile_decodable_wires() {
        let request = request();
        let bytes = request.to_bytes().expect("request");
        assert_eq!(DirectFeeSettlementRequestV1::decode(&bytes), Ok(request));
        assert!(is_direct_fee_settlement_v1(&bytes));

        let receipt = receipt();
        let receipt_bytes = receipt.to_bytes().expect("receipt");
        assert_eq!(
            DirectFeeSettlementReceiptV1::decode(&receipt_bytes),
            Ok(receipt)
        );

        for offset in [0, 8, 10, 12] {
            let mut hostile = bytes;
            hostile[offset] ^= 1;
            assert!(DirectFeeSettlementRequestV1::decode(&hostile).is_err());
        }
        assert_eq!(
            DirectFeeSettlementRequestV1::decode(
                bytes
                    .get(..DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1 - 1)
                    .expect("prefix")
            ),
            Err(DirectFeeSettlementErrorV1::InvalidLength)
        );
    }

    /// The reserved tail is canonical zero, and a decoder that ignored it would
    /// admit two byte strings for one request -- two digests for one caller
    /// authority.
    #[test]
    fn a_nonzero_reserved_tail_refuses() {
        let bytes = request().to_bytes().expect("request");
        for offset in REQUEST_RESERVED_OFFSET..DIRECT_FEE_SETTLEMENT_REQUEST_BYTES_V1 {
            let mut hostile = bytes;
            hostile[offset] = 1;
            assert_eq!(
                DirectFeeSettlementRequestV1::decode(&hostile),
                Err(DirectFeeSettlementErrorV1::NonCanonical),
                "reserved byte {offset}",
            );
        }
    }

    /// Zero means "unmined, search", which is the only reading a PDA bump of
    /// zero can have; every other value is reproduced and checked.
    #[test]
    fn a_zero_bump_is_absence_and_not_a_bump() {
        assert_eq!(request().caller_authority_hint(), Some(254));
        assert_eq!(
            DirectFeeSettlementRequestV1 {
                caller_authority_bump: 0,
                ..request()
            }
            .caller_authority_hint(),
            None
        );
        assert_eq!(request().custody_relay(), [253, 252]);
    }

    /// A receipt that claims a zero settlement, or a revision that did not step
    /// by exactly one, is not a receipt for anything this route can have done.
    #[test]
    fn the_receipt_refuses_a_settlement_it_could_not_have_produced() {
        assert!(receipt().new().is_ok());
        assert_eq!(
            DirectFeeSettlementReceiptV1 {
                settled_amount: 0,
                ..receipt()
            }
            .new(),
            Err(DirectFeeSettlementErrorV1::InvalidSettlement)
        );
        assert_eq!(
            DirectFeeSettlementReceiptV1 {
                resulting_revision: 10,
                ..receipt()
            }
            .new(),
            Err(DirectFeeSettlementErrorV1::InvalidSettlement)
        );
    }

    /// The order tag separates a settlement from the leg it continues, and
    /// moves on every axis that identifies the settlement.
    #[test]
    fn the_order_tag_is_distinct_from_its_parent_and_from_every_neighbour() {
        let parent = id(20);
        let root = id(21);
        let baseline = direct_fee_settlement_order_v1(parent, root, 2);
        assert_ne!(baseline, parent);
        for changed in [
            direct_fee_settlement_order_v1(id(22), root, 2),
            direct_fee_settlement_order_v1(parent, id(23), 2),
            direct_fee_settlement_order_v1(parent, root, 3),
        ] {
            assert_ne!(changed, baseline);
        }
    }
}
