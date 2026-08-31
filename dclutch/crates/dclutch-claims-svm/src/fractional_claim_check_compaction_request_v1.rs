//! The fractional compaction request: the same terminal header, plus the four
//! coordinates the Fractional family adds and the one number it promises.
//!
//! [`crate::claim_check_compaction_request_v1`] establishes the rule this type
//! keeps: **compaction must call the payout derivation, never re-implement it.**
//! So this wire does not restate the terminal header either. It carries one
//! verbatim, decoded by [`TerminalSettlementRequestV3::decode`][decode] and by
//! nothing else, and adds only what the Fractional family needs that the
//! terminal header does not already carry.
//!
//! [decode]: crate::terminal_settlement_v3::TerminalSettlementRequestV3::decode
//!
//! # What is added, and what is deliberately not
//!
//! Added: the finalized Fractional exposure `terms` (which authors the
//! denominator and names the shard Mint for a coordinate), the selected
//! `token_behavior` record, the root's `expected_root_revision`, the
//! `representation_coordinate` being compacted, and `payout_per_claim`.
//!
//! Not added, because the terminal header already carries them and a second
//! author is how two halves of one feature drift: `release_set`, `market`,
//! `realm`, `generation`, `collateral_mint`, `token_program`, `position`, every
//! expected revision, and the recipient pair.
//!
//! **Not added, and this one is worth stating: the Fractional capability root.**
//! It is [`Self::root`], which reads `settlement.input().owner`. The reserve
//! Position's owner *is* the root -- `fractional_retirement_v3` joins its
//! admission on exactly that equality -- so a separate field would be a second
//! place to name one thing, and the only way the two could ever differ is if one
//! of them were wrong.
//!
//! # Why `payout_per_claim` may be on the wire at all
//!
//! It is the rate every returning holder will apply forever, so a caller naming
//! it looks like the whole attack. It is not, because **the wire carries a claim
//! and the conservation plan authenticates it.**
//!
//! [`FractionalClaimCheckCompactionPlanV1`][plan] refuses to exist unless
//!
//! ```text
//! observed vault credit == (observed shard supply / authenticated denominator)
//!                          * payout_per_claim
//! ```
//!
//! where the credit is what the terminal derivation actually moved, the supply
//! is read off the Mint, and the denominator comes from the authenticated terms.
//! Two of the three are chain observations and the third is authenticated, so
//! for any nonzero whole-claim count the equation pins `payout_per_claim`
//! uniquely: a caller who names a rate one atom off gets `RateNotCovered`.
//!
//! The one place the equation does not pin it is where the outstanding supply
//! forms no whole claim at all, since `0 == 0` holds for every rate. Nothing is
//! promised there either: the plan's funding conjunct declines to mint whenever
//! the escrowed total is zero, so no record is written and no rate is persisted.
//! Stated rather than left for a reader to find, because it is exactly the hole
//! somebody would go looking for.
//!
//! [plan]: crate::fractional_claim_check_conservation_v1::FractionalClaimCheckCompactionPlanV1

use core::convert::TryInto;

use crate::CallerRole;
use crate::claim_check_request_v1::ClaimCheckActionV1;
use crate::claim_check_v1::{CLAIM_CHECK_WIRE_VERSION_V1, ClaimCheckErrorV1, ClaimCheckResultV1};
use crate::fractional_claim_check_v1::{
    FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1, FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1,
};
use crate::terminal_settlement_v3::{
    TERMINAL_SETTLEMENT_REQUEST_BYTES_V3, TerminalSettlementRequestInputV3,
    TerminalSettlementRequestV3,
};

const VERSION_OFFSET: usize = 8;
const ACTION_OFFSET: usize = 10;
const RESERVED_HEADER_OFFSET: usize = 11;
const RESERVED_HEADER_BYTES: usize = 5;
const TERMS_OFFSET: usize = 16;
const TOKEN_BEHAVIOR_OFFSET: usize = 48;
const REVISION_OFFSET: usize = 80;
const COORDINATE_OFFSET: usize = 88;
const PAYOUT_PER_CLAIM_OFFSET: usize = 92;
const RESERVED_BODY_OFFSET: usize = 100;
const RESERVED_BODY_BYTES: usize = 4;
const SETTLEMENT_OFFSET: usize = 104;

/// Exact width of a fractional compaction request.
pub const FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1: usize =
    SETTLEMENT_OFFSET + TERMINAL_SETTLEMENT_REQUEST_BYTES_V3;

/// The Fractional coordinates a compaction adds to the terminal header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCompactionCoordinatesV1 {
    /// Finalized Fractional exposure terms: the denominator's sole author.
    pub terms: [u8; 32],
    /// Terms-selected TokenBehavior record.
    pub token_behavior: [u8; 32],
    /// Optimistic Fractional capability root revision.
    pub expected_root_revision: u64,
    /// Claims representation coordinate whose shard Mint is being compacted.
    pub representation_coordinate: u32,
    /// Promised collateral atoms per whole Claims coordinate.
    ///
    /// A claim the conservation plan authenticates against two chain
    /// observations and one authenticated number; see this module's header for
    /// why carrying it here is safe and where it is not pinned.
    pub payout_per_claim: u64,
}

/// Trading-composed request to compact one Fractional reserve into a record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalCompactToClaimCheckRequestV1 {
    coordinates: FractionalCompactionCoordinatesV1,
    settlement: TerminalSettlementRequestV3,
}

impl FractionalCompactToClaimCheckRequestV1 {
    /// Construct and canonicalize one fractional compaction request.
    pub fn new(
        coordinates: FractionalCompactionCoordinatesV1,
        settlement: TerminalSettlementRequestV3,
    ) -> ClaimCheckResultV1<Self> {
        let value = Self {
            coordinates,
            settlement,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact fractional compaction request.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        if input.len() != FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1 {
            return Err(ClaimCheckErrorV1::InvalidLength);
        }
        exact(input, 0, &FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1)?;
        if read_u16(input, VERSION_OFFSET)? != CLAIM_CHECK_WIRE_VERSION_V1 {
            return Err(ClaimCheckErrorV1::InvalidHeader);
        }
        if read_byte(input, ACTION_OFFSET)? != ClaimCheckActionV1::FractionalCompact as u8 {
            return Err(ClaimCheckErrorV1::UnknownTag);
        }
        require_zero(input, RESERVED_HEADER_OFFSET, RESERVED_HEADER_BYTES)?;
        require_zero(input, RESERVED_BODY_OFFSET, RESERVED_BODY_BYTES)?;
        let settlement_bytes = input
            .get(SETTLEMENT_OFFSET..)
            .ok_or(ClaimCheckErrorV1::InvalidLength)?;
        // One author, exactly as the native request insists: the terminal
        // header's own decoder owns every refusal it has ever made, including
        // the ones added after this line was written.
        let settlement = TerminalSettlementRequestV3::decode(settlement_bytes)
            .map_err(|_| ClaimCheckErrorV1::InvalidHeader)?;
        Self::new(
            FractionalCompactionCoordinatesV1 {
                terms: read_array(input, TERMS_OFFSET)?,
                token_behavior: read_array(input, TOKEN_BEHAVIOR_OFFSET)?,
                expected_root_revision: read_u64(input, REVISION_OFFSET)?,
                representation_coordinate: read_u32(input, COORDINATE_OFFSET)?,
                payout_per_claim: read_u64(input, PAYOUT_PER_CLAIM_OFFSET)?,
            },
            settlement,
        )
    }

    /// Encode one exact canonical fractional compaction request.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1]> {
        self.validate()?;
        let mut output = [0; FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1];
        write(&mut output, 0, &FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1)?;
        write(
            &mut output,
            VERSION_OFFSET,
            &CLAIM_CHECK_WIRE_VERSION_V1.to_le_bytes(),
        )?;
        write(
            &mut output,
            ACTION_OFFSET,
            &[ClaimCheckActionV1::FractionalCompact as u8],
        )?;
        write(&mut output, TERMS_OFFSET, &self.coordinates.terms)?;
        write(
            &mut output,
            TOKEN_BEHAVIOR_OFFSET,
            &self.coordinates.token_behavior,
        )?;
        write(
            &mut output,
            REVISION_OFFSET,
            &self.coordinates.expected_root_revision.to_le_bytes(),
        )?;
        write(
            &mut output,
            COORDINATE_OFFSET,
            &self.coordinates.representation_coordinate.to_le_bytes(),
        )?;
        write(
            &mut output,
            PAYOUT_PER_CLAIM_OFFSET,
            &self.coordinates.payout_per_claim.to_le_bytes(),
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

    /// Return the Fractional coordinates this request adds.
    #[must_use]
    pub const fn coordinates(self) -> FractionalCompactionCoordinatesV1 {
        self.coordinates
    }

    /// Return the Fractional capability root: the reserve Position's owner.
    ///
    /// Not a field. The reserve Position is owned by the root, and this is the
    /// only place that fact is read, so there is nothing for a caller to forge
    /// and nothing for two fields to disagree about.
    #[must_use]
    pub const fn root(self) -> [u8; 32] {
        self.settlement.input().owner
    }

    /// Require this request to pay the market's own escrow and nothing else.
    ///
    /// The native rule, unchanged and for the same reason: the recipient pair
    /// is derived by the route from the market's aggregate, and a caller who
    /// could name it would redirect every shard holder's collateral at once.
    /// Fractionally the stake is larger than natively -- one reserve backs an
    /// entire coordinate's outstanding supply, not one sleeper's payout.
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

    fn validate(self) -> ClaimCheckResultV1<()> {
        let input = self.settlement.input();
        // The same role the native compaction admits, and for the same reason:
        // this route stands in for a top-level settlement whose signer is
        // absent. Being Trading-composed changes who holds account zero -- the
        // release-pinned caller authority rather than an arbitrary cranker --
        // and changes nothing about which settlement is being stood in for.
        if input.caller_role != CallerRole::Claims {
            return Err(ClaimCheckErrorV1::UnknownTag);
        }
        // Paying the reserve's own owner directly is not compaction. It is the
        // root taking the collateral that backs every outstanding shard, which
        // is the fractional form of the native hostile and strictly worse.
        if input.recipient_owner == input.owner
            || input.recipient_token_account == input.owner
            || input.recipient_owner == input.position
            || input.recipient_token_account == input.position
        {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        // The terms author the denominator and name the shard Mint; the
        // behavior record selects the Token program the Mint must be owned by.
        // A request aliasing either onto a coordinate the terminal header
        // already carries is naming one account twice with two meanings.
        require_distinct(&[
            self.coordinates.terms,
            self.coordinates.token_behavior,
            input.release_set,
            input.market,
            input.owner,
            input.position,
        ])?;
        // A coordinate no exposure terms could ever declare. Refused here
        // rather than deeper, exactly as the record refuses it: the record has
        // to be self-authenticating after the terms are gone, so a request that
        // could mint an unreadable record must not be admitted in the first
        // place.
        if self.coordinates.representation_coordinate >= FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        // A rate of zero promises a record nobody would ever redeem, which the
        // record type refuses and the conservation plan declines to mint. A
        // request naming it is asking for an act with no possible outcome.
        if self.coordinates.payout_per_claim == 0 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        Ok(())
    }
}

fn require_distinct(identities: &[[u8; 32]]) -> ClaimCheckResultV1<()> {
    for (index, left) in identities.iter().enumerate() {
        if left.iter().all(|byte| *byte == 0) {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        let rest = index.checked_add(1).ok_or(ClaimCheckErrorV1::Arithmetic)?;
        if identities.iter().skip(rest).any(|right| right == left) {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
    }
    Ok(())
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

fn read_u32(input: &[u8], offset: usize) -> ClaimCheckResultV1<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    let bytes: [u8; 4] = input
        .get(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| ClaimCheckErrorV1::InvalidLength)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(input: &[u8], offset: usize) -> ClaimCheckResultV1<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    let bytes: [u8; 8] = input
        .get(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| ClaimCheckErrorV1::InvalidLength)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_array(input: &[u8], offset: usize) -> ClaimCheckResultV1<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(ClaimCheckErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| ClaimCheckErrorV1::InvalidLength)
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
    use crate::claim_check_compaction_request_v1::CompactPositionToClaimCheckRequestV1;

    const ESCROW: [u8; 32] = [40; 32];
    const VAULT: [u8; 32] = [41; 32];
    /// The Fractional capability root, which is the reserve Position's owner.
    const ROOT: [u8; 32] = [9; 32];

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
            owner: ROOT,
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

    fn coordinates() -> FractionalCompactionCoordinatesV1 {
        FractionalCompactionCoordinatesV1 {
            terms: [20; 32],
            token_behavior: [21; 32],
            expected_root_revision: 11,
            representation_coordinate: 6,
            payout_per_claim: 4_000,
        }
    }

    fn settlement() -> TerminalSettlementRequestV3 {
        TerminalSettlementRequestV3::new(settlement_input()).expect("settlement")
    }

    fn compaction() -> FractionalCompactToClaimCheckRequestV1 {
        FractionalCompactToClaimCheckRequestV1::new(coordinates(), settlement())
            .expect("fractional compaction")
    }

    #[test]
    fn a_fractional_compaction_request_round_trips_at_its_one_exact_width() {
        let value = compaction();
        let bytes = value.to_bytes().expect("bytes");
        assert_eq!(bytes.len(), FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1);
        assert_eq!(
            FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1,
            104 + TERMINAL_SETTLEMENT_REQUEST_BYTES_V3
        );
        assert_eq!(
            FractionalCompactToClaimCheckRequestV1::decode(&bytes),
            Ok(value)
        );
        assert_eq!(value.coordinates(), coordinates());
    }

    #[test]
    fn the_embedded_header_is_the_terminal_header_byte_for_byte() {
        // The same claim the native request makes, and it has to be checkable
        // the same way: the bytes the payout derivation reads are the exact
        // bytes an ordinary redemption would have carried, with nothing this
        // wrapper added interleaved into them.
        let value = compaction();
        let bytes = value.to_bytes().expect("bytes");
        assert_eq!(
            bytes.get(SETTLEMENT_OFFSET..),
            Some(settlement().to_bytes().as_slice())
        );
        assert_eq!(value.settlement(), settlement());
        assert_eq!(value.input(), settlement_input());
    }

    #[test]
    fn the_root_is_read_from_the_settlement_and_is_never_a_second_field() {
        // The reserve Position's owner IS the Fractional capability root, and
        // this is where that is read. A separate field could disagree with it,
        // and the only way it ever would is by being wrong.
        let value = compaction();
        assert_eq!(value.root(), ROOT);
        assert_eq!(value.root(), value.input().owner);
        // And there is nowhere in the bytes for a second copy to live: the
        // whole fractional body is the four coordinates plus its reserved run.
        let bytes = value.to_bytes().expect("bytes");
        let body = bytes
            .get(TERMS_OFFSET..SETTLEMENT_OFFSET)
            .expect("fractional body");
        assert_eq!(body.len(), 32 + 32 + 8 + 4 + 8 + RESERVED_BODY_BYTES);
        assert!(
            !body.windows(32).any(|window| window == ROOT),
            "the root must not appear anywhere in the fractional body"
        );
    }

    #[test]
    fn the_two_compaction_requests_can_never_be_decoded_as_each_other() {
        // Different magics, different widths, different actions -- and asserted
        // rather than assumed, because one of these routes pays one sleeper and
        // the other resolves the collateral behind an entire coordinate's
        // outstanding supply.
        let fractional = compaction().to_bytes().expect("bytes");
        let native = CompactPositionToClaimCheckRequestV1::new(settlement())
            .expect("native compaction")
            .to_bytes()
            .expect("bytes");
        assert_ne!(fractional.len(), native.len());
        assert!(FractionalCompactToClaimCheckRequestV1::decode(&native).is_err());
        assert!(CompactPositionToClaimCheckRequestV1::decode(&fractional).is_err());
        // Same width, wrong magic: the one case a length check cannot catch.
        let mut disguised = fractional;
        write(
            &mut disguised,
            0,
            &crate::claim_check_v1::CLAIM_CHECK_COMPACT_MAGIC_V1,
        )
        .expect("write");
        assert_eq!(
            FractionalCompactToClaimCheckRequestV1::decode(&disguised),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
        // Same width, right magic, wrong action byte.
        let mut wrong_action = fractional;
        write(
            &mut wrong_action,
            ACTION_OFFSET,
            &[ClaimCheckActionV1::Compact as u8],
        )
        .expect("write");
        assert_eq!(
            FractionalCompactToClaimCheckRequestV1::decode(&wrong_action),
            Err(ClaimCheckErrorV1::UnknownTag)
        );
    }

    #[test]
    fn every_reserved_byte_and_every_width_but_one_is_refused() {
        let canonical = compaction().to_bytes().expect("bytes");
        for offset in RESERVED_HEADER_OFFSET
            ..RESERVED_HEADER_OFFSET
                .checked_add(RESERVED_HEADER_BYTES)
                .expect("range")
        {
            let mut bytes = canonical;
            write(&mut bytes, offset, &[1]).expect("write");
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
        for offset in RESERVED_BODY_OFFSET
            ..RESERVED_BODY_OFFSET
                .checked_add(RESERVED_BODY_BYTES)
                .expect("range")
        {
            let mut bytes = canonical;
            write(&mut bytes, offset, &[1]).expect("write");
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
        let mut short = canonical.to_vec();
        short.pop();
        assert_eq!(
            FractionalCompactToClaimCheckRequestV1::decode(&short),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        let mut long = canonical.to_vec();
        long.push(0);
        assert_eq!(
            FractionalCompactToClaimCheckRequestV1::decode(&long),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        let mut version = canonical;
        write(&mut version, VERSION_OFFSET, &2_u16.to_le_bytes()).expect("write");
        assert_eq!(
            FractionalCompactToClaimCheckRequestV1::decode(&version),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
    }

    #[test]
    fn paying_the_reserves_own_owner_is_refused_in_all_four_ways() {
        // Fractionally this is worse than the native hostile it mirrors: the
        // reserve backs every outstanding shard of one coordinate, so a
        // recipient the caller chose takes all of them at once.
        for input in [
            TerminalSettlementRequestInputV3 {
                recipient_owner: ROOT,
                ..settlement_input()
            },
            TerminalSettlementRequestInputV3 {
                recipient_token_account: ROOT,
                ..settlement_input()
            },
            TerminalSettlementRequestInputV3 {
                recipient_owner: settlement_input().position,
                ..settlement_input()
            },
            TerminalSettlementRequestInputV3 {
                recipient_token_account: settlement_input().position,
                ..settlement_input()
            },
        ] {
            let settlement = TerminalSettlementRequestV3::new(input).expect("settlement");
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::new(coordinates(), settlement),
                Err(ClaimCheckErrorV1::InvalidIdentity)
            );
        }
        // And the escrow pair is required to be exactly the derived one.
        let value = compaction();
        assert_eq!(value.require_escrow_recipient(ESCROW, VAULT), Ok(()));
        assert_eq!(
            value.require_escrow_recipient(VAULT, ESCROW),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        assert_eq!(
            value.require_escrow_recipient([42; 32], VAULT),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
    }

    #[test]
    fn a_role_that_did_not_lose_its_signer_is_refused() {
        for role in [CallerRole::Core, CallerRole::Trading] {
            let settlement = TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
                caller_role: role,
                ..settlement_input()
            })
            .expect("settlement");
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::new(coordinates(), settlement),
                Err(ClaimCheckErrorV1::UnknownTag),
                "{role:?} reaches a settlement by CPI with its own authority signing"
            );
        }
    }

    #[test]
    fn the_fractional_coordinates_are_nonzero_distinct_and_bounded() {
        for terms in [[0; 32], [21; 32], [1; 32], [2; 32], ROOT, [10; 32]] {
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::new(
                    FractionalCompactionCoordinatesV1 {
                        terms,
                        ..coordinates()
                    },
                    settlement(),
                ),
                Err(ClaimCheckErrorV1::InvalidIdentity),
                "terms may not be zero, the behavior record, or a settlement coordinate"
            );
        }
        for token_behavior in [[0; 32], [20; 32], [1; 32], [2; 32], ROOT, [10; 32]] {
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::new(
                    FractionalCompactionCoordinatesV1 {
                        token_behavior,
                        ..coordinates()
                    },
                    settlement(),
                ),
                Err(ClaimCheckErrorV1::InvalidIdentity)
            );
        }
        // The coordinate bound is the exposure terms' own, restated because the
        // record it mints has to be readable after the terms are gone.
        for representation_coordinate in [
            FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1,
            FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1.wrapping_add(1),
            u32::MAX,
        ] {
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::new(
                    FractionalCompactionCoordinatesV1 {
                        representation_coordinate,
                        ..coordinates()
                    },
                    settlement(),
                ),
                Err(ClaimCheckErrorV1::InvalidEntitlement)
            );
        }
        assert!(
            FractionalCompactToClaimCheckRequestV1::new(
                FractionalCompactionCoordinatesV1 {
                    representation_coordinate: FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1
                        .saturating_sub(1),
                    ..coordinates()
                },
                settlement(),
            )
            .is_ok(),
            "the last admissible coordinate is admissible"
        );
        // A rate of zero asks for a record the record type itself refuses.
        assert_eq!(
            FractionalCompactToClaimCheckRequestV1::new(
                FractionalCompactionCoordinatesV1 {
                    payout_per_claim: 0,
                    ..coordinates()
                },
                settlement(),
            ),
            Err(ClaimCheckErrorV1::InvalidEntitlement)
        );
    }

    #[test]
    fn the_promised_rate_survives_the_wire_exactly() {
        // The record persists this number and every returning holder multiplies
        // by it forever, so a wire that rounded, truncated or sign-extended it
        // anywhere would be a slow leak rather than a refusal.
        for payout_per_claim in [1_u64, 2, 4_000, u64::MAX.wrapping_sub(1), u64::MAX] {
            let value = FractionalCompactToClaimCheckRequestV1::new(
                FractionalCompactionCoordinatesV1 {
                    payout_per_claim,
                    ..coordinates()
                },
                settlement(),
            )
            .expect("rate");
            let bytes = value.to_bytes().expect("bytes");
            assert_eq!(
                FractionalCompactToClaimCheckRequestV1::decode(&bytes)
                    .expect("round trip")
                    .coordinates()
                    .payout_per_claim,
                payout_per_claim
            );
        }
    }
}
