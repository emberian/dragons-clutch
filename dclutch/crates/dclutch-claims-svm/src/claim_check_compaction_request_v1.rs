//! The compaction request, which is redemption's own request with the
//! recipient swapped.
//!
//! This is the single most important instruction in the design, expressed as a
//! type rather than as a comment: **compaction must call the payout derivation,
//! never re-implement it.** A second author for the payoff function is how a
//! compaction that pays a different number than redemption would have gets
//! built and passes its own tests, and the number is somebody's money.
//!
//! So the wire does not restate the terminal header. It carries one verbatim,
//! at its own exact width, decoded by
//! [`TerminalSettlementRequestV3::decode`][decode] and by nothing else. Every
//! coordinate the derivation reads therefore has exactly one author, and a
//! future edit to the terminal header reaches compaction automatically instead
//! of silently leaving it behind.
//!
//! # What compaction is allowed to change, and what it is not
//!
//! Exactly two fields differ from the redemption the sleeping holder would have
//! sent: `recipient_owner` and `recipient_token_account`. Everything else --
//! the market, the position, the outcome, the quantity, every expected revision
//! -- is the holder's own settlement, unaltered.
//!
//! Those two fields are the whole attack surface, because a cranker who could
//! choose them would redirect a sleeping holder's collateral to themselves.
//! They are therefore **derived, not accepted**: the route computes the escrow
//! PDA and its vault from the market's own aggregate and requires the request
//! to name exactly those, through [`CompactPositionToClaimCheckRequestV1::require_escrow_recipient`].
//! The check lives here, where it is pure and testable; the derivation lives in
//! the route, where the program id does.
//!
//! [decode]: crate::terminal_settlement_v3::TerminalSettlementRequestV3::decode
//!
//! # Why this route exists at all, given the header is identical
//!
//! Because of the signature. Redemption under `CallerRole::Claims` binds
//! account zero to the position owner's own wallet -- that is what makes it
//! GREEN-SELF, and nothing about it needs fixing. A right, though, is not a
//! liveness guarantee: a route only its beneficiary can call stalls when its
//! beneficiary is absent. Compaction relaxes that one requirement, and only
//! after a release-fixed deadline has passed, while keeping every other
//! coordinate identical.

use core::convert::TryInto;

use crate::CallerRole;
use crate::claim_check_request_v1::ClaimCheckActionV1;
use crate::claim_check_v1::{
    CLAIM_CHECK_COMPACT_MAGIC_V1, CLAIM_CHECK_WIRE_VERSION_V1, ClaimCheckErrorV1,
    ClaimCheckResultV1,
};
use crate::terminal_settlement_v3::{
    TERMINAL_SETTLEMENT_REQUEST_BYTES_V3, TerminalSettlementRequestInputV3,
    TerminalSettlementRequestV3,
};

const VERSION_OFFSET: usize = 8;
const ACTION_OFFSET: usize = 10;
const RESERVED_HEADER_OFFSET: usize = 11;
const RESERVED_HEADER_BYTES: usize = 5;
const SETTLEMENT_OFFSET: usize = 16;

/// Exact width of a compaction request: the claim-check header plus one
/// verbatim terminal settlement request.
pub const COMPACT_POSITION_TO_CLAIM_CHECK_BYTES_V1: usize =
    SETTLEMENT_OFFSET + TERMINAL_SETTLEMENT_REQUEST_BYTES_V3;

/// Permissionless request to compact one sleeping position into a claim-check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactPositionToClaimCheckRequestV1 {
    settlement: TerminalSettlementRequestV3,
}

impl CompactPositionToClaimCheckRequestV1 {
    /// Wrap one terminal settlement request as a compaction request.
    pub fn new(settlement: TerminalSettlementRequestV3) -> ClaimCheckResultV1<Self> {
        let value = Self { settlement };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact compaction request.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        if input.len() != COMPACT_POSITION_TO_CLAIM_CHECK_BYTES_V1 {
            return Err(ClaimCheckErrorV1::InvalidLength);
        }
        exact(input, 0, &CLAIM_CHECK_COMPACT_MAGIC_V1)?;
        if read_u16(input, VERSION_OFFSET)? != CLAIM_CHECK_WIRE_VERSION_V1 {
            return Err(ClaimCheckErrorV1::InvalidHeader);
        }
        if read_byte(input, ACTION_OFFSET)? != ClaimCheckActionV1::Compact as u8 {
            return Err(ClaimCheckErrorV1::UnknownTag);
        }
        require_zero(input, RESERVED_HEADER_OFFSET, RESERVED_HEADER_BYTES)?;
        let settlement_bytes = input
            .get(SETTLEMENT_OFFSET..)
            .ok_or(ClaimCheckErrorV1::InvalidLength)?;
        // One author. The terminal header's own decoder owns every refusal it
        // has ever made, including the ones added after this line was written.
        let settlement = TerminalSettlementRequestV3::decode(settlement_bytes)
            .map_err(|_| ClaimCheckErrorV1::InvalidHeader)?;
        Self::new(settlement)
    }

    /// Encode one exact canonical compaction request.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; COMPACT_POSITION_TO_CLAIM_CHECK_BYTES_V1]> {
        self.validate()?;
        let mut output = [0; COMPACT_POSITION_TO_CLAIM_CHECK_BYTES_V1];
        write(&mut output, 0, &CLAIM_CHECK_COMPACT_MAGIC_V1)?;
        write(
            &mut output,
            VERSION_OFFSET,
            &CLAIM_CHECK_WIRE_VERSION_V1.to_le_bytes(),
        )?;
        write(
            &mut output,
            ACTION_OFFSET,
            &[ClaimCheckActionV1::Compact as u8],
        )?;
        write(&mut output, SETTLEMENT_OFFSET, &self.settlement.to_bytes())?;
        Ok(output)
    }

    /// Borrow the verbatim terminal settlement request the derivation reads.
    #[must_use]
    pub const fn settlement(self) -> TerminalSettlementRequestV3 {
        self.settlement
    }

    /// Return the settlement's coordinates.
    #[must_use]
    pub const fn input(self) -> TerminalSettlementRequestInputV3 {
        self.settlement.input()
    }

    /// Require this request to pay the market's own escrow and nothing else.
    ///
    /// The two values are derived by the route from the market's aggregate;
    /// this is where they are enforced. A cranker who could name the recipient
    /// would redirect a sleeping holder's collateral, so a mismatch here is the
    /// difference between a crank and a theft.
    pub fn require_escrow_recipient(
        self,
        escrow: [u8; 32],
        vault: [u8; 32],
    ) -> ClaimCheckResultV1<()> {
        let input = self.settlement.input();
        if input.recipient_owner != escrow || input.recipient_token_account != vault {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        Ok(())
    }

    /// Project the coordinates a compaction's claim-check is derived from.
    ///
    /// The aggregate is not a field of the terminal header -- it is derived
    /// from the market by the route -- so it is supplied here rather than
    /// guessed, and the owner comes from the settlement the holder would have
    /// sent. That pairing is what makes the claim-check's address a proof of
    /// its holder.
    #[must_use]
    pub const fn holder(self) -> [u8; 32] {
        self.settlement.input().owner
    }

    fn validate(self) -> ClaimCheckResultV1<()> {
        let input = self.settlement.input();
        // Compaction stands in for the top-level, owner-signed redemption and
        // for no other caller. A Core- or Trading-role settlement reaches this
        // route only by CPI, where its own caller authority already signed and
        // no deadline is being waived.
        if input.caller_role != CallerRole::Claims {
            return Err(ClaimCheckErrorV1::UnknownTag);
        }
        // Paying the holder directly is not compaction; it is the holder's own
        // redemption with the signature removed, which is the entire hostile.
        if input.recipient_owner == input.owner
            || input.recipient_token_account == input.owner
            || input.recipient_owner == input.position
            || input.recipient_token_account == input.position
        {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        Ok(())
    }
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> ClaimCheckResultV1<()> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    if input.get(offset..end) != Some(expected) {
        return Err(ClaimCheckErrorV1::InvalidHeader);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> ClaimCheckResultV1<()> {
    let end = offset
        .checked_add(width)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ClaimCheckErrorV1::NonCanonical);
    }
    Ok(())
}

fn read_byte(input: &[u8], offset: usize) -> ClaimCheckResultV1<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(ClaimCheckErrorV1::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> ClaimCheckResultV1<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    let bytes: [u8; 2] = input
        .get(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| ClaimCheckErrorV1::InvalidLength)?;
    Ok(u16::from_le_bytes(bytes))
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) -> ClaimCheckResultV1<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESCROW: [u8; 32] = [40; 32];
    const VAULT: [u8; 32] = [41; 32];

    fn settlement_input() -> TerminalSettlementRequestInputV3 {
        TerminalSettlementRequestInputV3 {
            caller_role: CallerRole::Claims,
            release_set: [1; 32],
            market: [2; 32],
            realm: [3; 32],
            parent_context: [4; 32],
            product_record_digest: [5; 32],
            exposure_id: [6; 32],
            exposure_digest: [7; 32],
            terminal_record_digest: [8; 32],
            owner: [9; 32],
            position: [10; 32],
            recipient_owner: ESCROW,
            recipient_token_account: VAULT,
            claims_program: [12; 32],
            custody_program: [13; 32],
            collateral_mint: [14; 32],
            token_program: [15; 32],
            semantic_basis_id: [16; 32],
            linked_basis_record_digest: [17; 32],
            generation: 9,
            expected_market_revision: 3,
            expected_position_revision: 4,
            expected_custody_revision: 5,
            quantity: 700,
            claim_index: 1,
            transfer_index: 0,
        }
    }

    fn compaction() -> CompactPositionToClaimCheckRequestV1 {
        CompactPositionToClaimCheckRequestV1::new(
            TerminalSettlementRequestV3::new(settlement_input()).expect("settlement"),
        )
        .expect("compaction")
    }

    #[test]
    fn a_compaction_request_round_trips_at_its_one_exact_width() {
        let value = compaction();
        let bytes = value.to_bytes().expect("bytes");
        assert_eq!(bytes.len(), COMPACT_POSITION_TO_CLAIM_CHECK_BYTES_V1);
        assert_eq!(
            COMPACT_POSITION_TO_CLAIM_CHECK_BYTES_V1,
            16 + TERMINAL_SETTLEMENT_REQUEST_BYTES_V3
        );
        assert_eq!(
            CompactPositionToClaimCheckRequestV1::decode(&bytes),
            Ok(value)
        );
    }

    #[test]
    fn the_embedded_header_is_the_terminal_header_byte_for_byte() {
        // The claim that the payout derivation is CALLED and not re-implemented
        // is only as good as this: the bytes the derivation reads are the exact
        // bytes an ordinary redemption would have carried.
        let value = compaction();
        let bytes = value.to_bytes().expect("bytes");
        let settlement = TerminalSettlementRequestV3::new(settlement_input()).expect("settlement");
        assert_eq!(
            bytes.get(SETTLEMENT_OFFSET..),
            Some(settlement.to_bytes().as_slice())
        );
        assert_eq!(value.settlement(), settlement);
        assert_eq!(value.input(), settlement_input());
    }

    #[test]
    fn compaction_differs_from_the_holders_own_redemption_in_exactly_two_fields() {
        // Stated as a test rather than as a claim in a comment: everything the
        // derivation reads is the sleeping holder's own settlement, and only
        // the destination moves.
        let compacted = compaction().input();
        let mut redeemed = settlement_input();
        redeemed.recipient_owner = redeemed.owner;
        redeemed.recipient_token_account = [11; 32];

        assert_ne!(compacted.recipient_owner, redeemed.recipient_owner);
        assert_ne!(
            compacted.recipient_token_account,
            redeemed.recipient_token_account
        );
        // And every other coordinate is identical.
        assert_eq!(compacted.market, redeemed.market);
        assert_eq!(compacted.owner, redeemed.owner);
        assert_eq!(compacted.position, redeemed.position);
        assert_eq!(compacted.quantity, redeemed.quantity);
        assert_eq!(compacted.claim_index, redeemed.claim_index);
        assert_eq!(compacted.generation, redeemed.generation);
        assert_eq!(
            compacted.expected_market_revision,
            redeemed.expected_market_revision
        );
        assert_eq!(
            compacted.expected_position_revision,
            redeemed.expected_position_revision
        );
        assert_eq!(
            compacted.expected_custody_revision,
            redeemed.expected_custody_revision
        );
        assert_eq!(compacted.collateral_mint, redeemed.collateral_mint);
        assert_eq!(compacted.token_program, redeemed.token_program);
    }

    #[test]
    fn a_cranker_may_not_name_the_recipient() {
        // The whole attack surface. A crank that could choose where the payout
        // lands is a theft with a deadline attached.
        let value = compaction();
        assert_eq!(value.require_escrow_recipient(ESCROW, VAULT), Ok(()));
        assert_eq!(
            value.require_escrow_recipient([99; 32], VAULT),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        assert_eq!(
            value.require_escrow_recipient(ESCROW, [99; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        // Swapping the pair is not admissible either.
        assert_eq!(
            value.require_escrow_recipient(VAULT, ESCROW),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
    }

    #[test]
    fn paying_the_holder_directly_is_refused_because_that_is_the_hostile() {
        // Compaction with the holder as recipient is the holder's redemption
        // with the signature deleted, which is precisely what the deadline and
        // the escrow exist to prevent.
        for mutate in [
            |input: &mut TerminalSettlementRequestInputV3| input.recipient_owner = input.owner,
            |input: &mut TerminalSettlementRequestInputV3| {
                input.recipient_token_account = input.owner
            },
            |input: &mut TerminalSettlementRequestInputV3| input.recipient_owner = input.position,
            |input: &mut TerminalSettlementRequestInputV3| {
                input.recipient_token_account = input.position
            },
        ] {
            let mut input = settlement_input();
            mutate(&mut input);
            let settlement = TerminalSettlementRequestV3::new(input).expect("settlement");
            assert_eq!(
                CompactPositionToClaimCheckRequestV1::new(settlement),
                Err(ClaimCheckErrorV1::InvalidIdentity)
            );
        }
    }

    #[test]
    fn only_the_owner_signed_role_may_be_compacted() {
        // Core and Trading reach terminal settlement by CPI, where their own
        // caller authority already signed and no deadline is being waived.
        for role in [CallerRole::Core, CallerRole::Trading] {
            let mut input = settlement_input();
            input.caller_role = role;
            let settlement = TerminalSettlementRequestV3::new(input).expect("settlement");
            assert_eq!(
                CompactPositionToClaimCheckRequestV1::new(settlement),
                Err(ClaimCheckErrorV1::UnknownTag)
            );
        }
    }

    #[test]
    fn a_hostile_header_is_refused_by_the_terminal_decoder_not_by_a_copy_of_it() {
        let good = compaction().to_bytes().expect("bytes");

        let mut wrong_magic = good;
        write(&mut wrong_magic, 0, b"DCLTCCO1").expect("magic");
        assert_eq!(
            CompactPositionToClaimCheckRequestV1::decode(&wrong_magic),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );

        let mut wrong_version = good;
        write(&mut wrong_version, VERSION_OFFSET, &2_u16.to_le_bytes()).expect("version");
        assert_eq!(
            CompactPositionToClaimCheckRequestV1::decode(&wrong_version),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );

        let mut wrong_action = good;
        write(
            &mut wrong_action,
            ACTION_OFFSET,
            &[ClaimCheckActionV1::Redeem as u8],
        )
        .expect("action");
        assert_eq!(
            CompactPositionToClaimCheckRequestV1::decode(&wrong_action),
            Err(ClaimCheckErrorV1::UnknownTag)
        );

        // A corrupted byte anywhere inside the embedded header is caught by the
        // terminal decoder, which is the point of embedding it verbatim.
        let mut corrupt = good;
        write(&mut corrupt, SETTLEMENT_OFFSET, &[0xFF]).expect("corrupt");
        assert_eq!(
            CompactPositionToClaimCheckRequestV1::decode(&corrupt),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );

        assert_eq!(
            CompactPositionToClaimCheckRequestV1::decode(
                good.get(..COMPACT_POSITION_TO_CLAIM_CHECK_BYTES_V1 - 1)
                    .expect("truncate")
            ),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
    }

    #[test]
    fn every_reserved_header_byte_must_be_zero() {
        for offset in RESERVED_HEADER_OFFSET..(RESERVED_HEADER_OFFSET + RESERVED_HEADER_BYTES) {
            let mut bytes = compaction().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[1]).expect("dirty");
            assert_eq!(
                CompactPositionToClaimCheckRequestV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
    }

    #[test]
    fn the_holder_is_read_from_the_settlement_never_from_a_field_of_our_own() {
        let value = compaction();
        assert_eq!(value.holder(), settlement_input().owner);
    }
}
