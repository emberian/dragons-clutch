//! The fractional compaction receipt: what the route proves it did.
//!
//! # Why this type exists at all, and why it did not exist until now
//!
//! Trading's receipt verifier refused
//! [`ReceiptKindV3::FractionalClaimCheckCompaction`][kind] outright for three
//! lanes, and the refusal was correct rather than a stub: a receipt verifier's
//! whole job is to prove the child did what the request asked, and there was
//! nothing to prove it against. FRACCHECK-2 declined to invent one on the
//! ground that a receipt fixed to what a route *would be guessed to* produce is
//! a shape nobody can check, and being the lane that also writes the route does
//! not make that guess safer -- only more convincing.
//!
//! So this type is written against a route that exists, in the same commit that
//! makes the route emit it. Every field below is a value
//! `fractional_claim_check_v1::process_fractional_compaction` holds at the
//! moment it returns, and not one of them is a number a caller supplied
//! unchecked.
//!
//! [kind]: (Trading's `claims_composition_v3`)
//!
//! # The one field that is not evidence of the payout
//!
//! [`FractionalClaimCheckCompactionReceiptV1::root`] is the capability root, and
//! it is here because Trading compares it against the root account **in the
//! frame it built**, at the coordinate the frame declaration names. That is what
//! closes the loop the three sibling Fractional receipts already close: a child
//! that reported a different root than the parent authenticated would be a child
//! that ran against a market the parent did not select.
//!
//! # What is deliberately NOT here
//!
//! No holder, no payee, no claimant. The record this route mints is addressed by
//! the instrument, and a receipt naming one holder would be the first place the
//! "positions are never enumerated" premise leaked -- the same refusal the frame
//! declaration states as `RefusedNamesOneHolder`.
//!
//! No lamport figures. The sweep is authenticated by the conservation plan's own
//! `validate_post` against observed post-balances *inside* the route, which is a
//! strictly stronger check than a number copied onto a wire for a parent to
//! re-read. A receipt field restating it would be a second author for a fact
//! that already has one.

use crate::claim_check_v1::{CLAIM_CHECK_WIRE_VERSION_V1, ClaimCheckErrorV1, ClaimCheckResultV1};
use crate::fractional_claim_check_compaction_request_v1::FractionalCompactToClaimCheckRequestV1;
use crate::fractional_claim_check_v1::FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1;

/// Exact width of a fractional compaction receipt.
pub const FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1: usize = 256;

/// Magic prefix of a fractional compaction receipt.
///
/// The family's fourth, and the letter follows the terminal settlement's own
/// question/answer convention (`DCLTSQ03` -> `DCLTSA03`): `DCLTFCC1` asks and
/// `DCLTFCA1` answers.
pub const FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTFCA1";

const VERSION_OFFSET: usize = 8;
const RESERVED_HEADER_OFFSET: usize = 10;
const RESERVED_HEADER_BYTES: usize = 6;
const REQUEST_DIGEST_OFFSET: usize = 16;
const ROOT_OFFSET: usize = 48;
const AGGREGATE_OFFSET: usize = 80;
const SHARD_MINT_OFFSET: usize = 112;
const ESCROW_OFFSET: usize = 144;
const RECORD_OFFSET: usize = 176;
const ESCROWED_OFFSET: usize = 208;
const DENOMINATOR_OFFSET: usize = 216;
const PAYOUT_PER_CLAIM_OFFSET: usize = 224;
const COMPACTED_SUPPLY_OFFSET: usize = 232;
const COORDINATE_OFFSET: usize = 240;
const MINTED_OFFSET: usize = 244;
const RESERVED_BODY_OFFSET: usize = 245;
const RESERVED_BODY_BYTES: usize = 11;

// The layout is exact and the trailing reserve is what proves it. A field added
// later must take bytes from the reserve and shrink this constant, which does
// not compile until its author has looked at every offset above.
const _: () = assert!(
    RESERVED_BODY_OFFSET + RESERVED_BODY_BYTES == FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1,
    "the fractional compaction receipt's reserved tail must close the record exactly"
);

/// Exact evidence a fractional compaction emits after it has moved everything.
///
/// Every field is read from chain state or from an authenticated record by the
/// route that emits it. Two of them -- `denominator` and `payout_per_claim` --
/// are the pair the minted record persists forever, and they are on this wire so
/// that the transaction which created a permanent obligation says on its face
/// what that obligation's rate is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckCompactionReceiptV1 {
    request_digest: [u8; 32],
    root: [u8; 32],
    aggregate: [u8; 32],
    shard_mint: [u8; 32],
    escrow: [u8; 32],
    record: [u8; 32],
    escrowed_atoms: u64,
    denominator: u64,
    payout_per_claim: u64,
    compacted_shard_supply: u64,
    representation_coordinate: u32,
    minted: bool,
}

/// Everything one fractional compaction receipt is built from.
///
/// A struct rather than eleven positional arguments, because a constructor whose
/// arguments are eleven `[u8; 32]`s and four integers is a constructor whose
/// call sites can silently transpose two of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckCompactionReceiptInputV1 {
    /// Digest of the exact instruction data this route was handed.
    pub request_digest: [u8; 32],
    /// The capability root, as it sat in the frame the parent built.
    pub aggregate: [u8; 32],
    /// The shard Mint whose burn authority this route handed to the escrow.
    pub shard_mint: [u8; 32],
    /// The per-market escrow, derived from the aggregate and never accepted.
    pub escrow: [u8; 32],
    /// The fractional claim-check address, derived from aggregate and Mint.
    pub record: [u8; 32],
    /// Collateral atoms the record opened escrowed.
    pub escrowed_atoms: u64,
    /// Shard atoms per whole Claims coordinate, from the finalized terms.
    pub denominator: u64,
    /// Collateral atoms per whole Claims coordinate.
    pub payout_per_claim: u64,
    /// Outstanding shard supply observed at compaction.
    pub compacted_shard_supply: u64,
    /// Whether a record was actually minted.
    pub minted: bool,
}

impl FractionalClaimCheckCompactionReceiptV1 {
    /// Construct and canonicalize one fractional compaction receipt.
    ///
    /// The root and the representation coordinate are taken from the REQUEST
    /// rather than from the input struct, for the reason the request type takes
    /// the root off the terminal header rather than carrying a field: a value
    /// with one author cannot be made to disagree with itself.
    pub fn new(
        request: &FractionalCompactToClaimCheckRequestV1,
        input: FractionalClaimCheckCompactionReceiptInputV1,
    ) -> ClaimCheckResultV1<Self> {
        let value = Self {
            request_digest: input.request_digest,
            root: request.root(),
            aggregate: input.aggregate,
            shard_mint: input.shard_mint,
            escrow: input.escrow,
            record: input.record,
            escrowed_atoms: input.escrowed_atoms,
            denominator: input.denominator,
            payout_per_claim: input.payout_per_claim,
            compacted_shard_supply: input.compacted_shard_supply,
            representation_coordinate: request.coordinates().representation_coordinate,
            minted: input.minted,
        };
        value.validate()?;
        Ok(value)
    }

    /// Hostile-decode one exact fractional compaction receipt.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        if input.len() != FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1 {
            return Err(ClaimCheckErrorV1::InvalidLength);
        }
        exact(input, 0, &FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_MAGIC_V1)?;
        if read_u16(input, VERSION_OFFSET)? != CLAIM_CHECK_WIRE_VERSION_V1 {
            return Err(ClaimCheckErrorV1::InvalidHeader);
        }
        require_zero(input, RESERVED_HEADER_OFFSET, RESERVED_HEADER_BYTES)?;
        require_zero(input, RESERVED_BODY_OFFSET, RESERVED_BODY_BYTES)?;
        let minted = match read_byte(input, MINTED_OFFSET)? {
            0 => false,
            1 => true,
            // A bool is one bit and the wire gives it eight. Every other value
            // is refused rather than coerced: `!= 0` would let 255 distinct
            // encodings mean `true`, and a receipt with more than one valid
            // encoding is a receipt whose digest a caller can choose.
            _ => return Err(ClaimCheckErrorV1::NonCanonical),
        };
        Self {
            request_digest: read_array(input, REQUEST_DIGEST_OFFSET)?,
            root: read_array(input, ROOT_OFFSET)?,
            aggregate: read_array(input, AGGREGATE_OFFSET)?,
            shard_mint: read_array(input, SHARD_MINT_OFFSET)?,
            escrow: read_array(input, ESCROW_OFFSET)?,
            record: read_array(input, RECORD_OFFSET)?,
            escrowed_atoms: read_u64(input, ESCROWED_OFFSET)?,
            denominator: read_u64(input, DENOMINATOR_OFFSET)?,
            payout_per_claim: read_u64(input, PAYOUT_PER_CLAIM_OFFSET)?,
            compacted_shard_supply: read_u64(input, COMPACTED_SUPPLY_OFFSET)?,
            representation_coordinate: read_u32(input, COORDINATE_OFFSET)?,
            minted,
        }
        .canonical()
    }

    /// Encode one exact canonical fractional compaction receipt.
    pub fn to_bytes(
        self,
    ) -> ClaimCheckResultV1<[u8; FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1]> {
        self.validate()?;
        let mut output = [0; FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1];
        write(
            &mut output,
            0,
            &FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_MAGIC_V1,
        )?;
        write(
            &mut output,
            VERSION_OFFSET,
            &CLAIM_CHECK_WIRE_VERSION_V1.to_le_bytes(),
        )?;
        for (offset, value) in [
            (REQUEST_DIGEST_OFFSET, self.request_digest),
            (ROOT_OFFSET, self.root),
            (AGGREGATE_OFFSET, self.aggregate),
            (SHARD_MINT_OFFSET, self.shard_mint),
            (ESCROW_OFFSET, self.escrow),
            (RECORD_OFFSET, self.record),
        ] {
            write(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (ESCROWED_OFFSET, self.escrowed_atoms),
            (DENOMINATOR_OFFSET, self.denominator),
            (PAYOUT_PER_CLAIM_OFFSET, self.payout_per_claim),
            (COMPACTED_SUPPLY_OFFSET, self.compacted_shard_supply),
        ] {
            write(&mut output, offset, &value.to_le_bytes())?;
        }
        write(
            &mut output,
            COORDINATE_OFFSET,
            &self.representation_coordinate.to_le_bytes(),
        )?;
        write(&mut output, MINTED_OFFSET, &[u8::from(self.minted)])?;
        Ok(output)
    }

    /// Bind this receipt to the exact parent request and its digest.
    ///
    /// This is what makes the receipt evidence rather than an announcement. The
    /// parent hashed the bytes it sent; a receipt reporting a different digest
    /// is a receipt for some other transaction, whatever else it says.
    pub fn verify_for(
        self,
        request: FractionalCompactToClaimCheckRequestV1,
        request_digest: [u8; 32],
    ) -> ClaimCheckResultV1<()> {
        let coordinates = request.coordinates();
        if self.request_digest != request_digest
            || self.root != request.root()
            || self.representation_coordinate != coordinates.representation_coordinate
            // The rate the record will pay forever must be the rate the request
            // promised. The route already refuses a mismatch through the
            // conservation plan; restating it here is what lets the PARENT
            // refuse it too, without trusting the child to have done so.
            || self.payout_per_claim != coordinates.payout_per_claim
        {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        // The escrow this route pays is the one the request named as recipient,
        // and the vault is derived. Checking the pair here closes the same loop
        // `require_escrow_recipient` closes inside the route.
        request.require_escrow_recipient(self.escrow, request.input().recipient_token_account)?;
        Ok(())
    }

    /// The capability root this compaction ran against.
    ///
    /// Read by Trading and compared against the root account in the frame it
    /// built, which is why it is on the wire at all.
    #[must_use]
    pub const fn root(self) -> [u8; 32] {
        self.root
    }

    /// The shard Mint whose burn authority now belongs to the escrow.
    #[must_use]
    pub const fn shard_mint(self) -> [u8; 32] {
        self.shard_mint
    }

    /// The fractional claim-check address, minted or vacant.
    #[must_use]
    pub const fn record(self) -> [u8; 32] {
        self.record
    }

    /// Collateral atoms the record opened escrowed.
    #[must_use]
    pub const fn escrowed_atoms(self) -> u64 {
        self.escrowed_atoms
    }

    /// Outstanding shard supply observed at compaction.
    #[must_use]
    pub const fn compacted_shard_supply(self) -> u64 {
        self.compacted_shard_supply
    }

    /// Whether this compaction actually minted a record.
    #[must_use]
    pub const fn minted(self) -> bool {
        self.minted
    }

    /// Whether the escrowed opening balance is what this rate and supply form.
    ///
    /// The receipt's own arithmetic claim, checkable by a parent that holds
    /// nothing but these bytes. It is the same statement
    /// `FractionalClaimCheckV1::opening_escrow_is_consistent` makes about the
    /// persisted record, restated over the receipt so the parent does not have
    /// to read the record to check the child.
    #[must_use]
    pub const fn opening_escrow_is_consistent(self) -> bool {
        if self.denominator == 0 {
            return false;
        }
        match (self.compacted_shard_supply / self.denominator).checked_mul(self.payout_per_claim) {
            Some(expected) => expected == self.escrowed_atoms,
            None => false,
        }
    }

    fn canonical(self) -> ClaimCheckResultV1<Self> {
        self.validate()?;
        Ok(self)
    }

    fn validate(self) -> ClaimCheckResultV1<()> {
        require_distinct(&[
            self.root,
            self.aggregate,
            self.shard_mint,
            self.escrow,
            self.record,
        ])?;
        require_nonzero(self.request_digest)?;
        // The terms refuse a denominator of one or zero at decode, so a receipt
        // claiming one is a receipt for a compaction the terms could not have
        // produced -- the same refusal the record itself makes, restated where
        // a parent can reach it.
        if self.denominator <= 1 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        if self.representation_coordinate >= FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        // Minting and escrowing are welded in BOTH directions, exactly as the
        // conservation plan welds them: a record that promises nothing is a
        // record nobody would redeem and an outstanding count that never falls,
        // and atoms escrowed against no record are atoms nobody can reach.
        if self.minted != (self.escrowed_atoms != 0) {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        // And the arithmetic must close. A receipt whose rate and supply do not
        // form its own escrowed balance is describing a compaction that did not
        // conserve, whoever emitted it.
        if !self.opening_escrow_is_consistent() {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        Ok(())
    }
}

fn require_nonzero(value: [u8; 32]) -> ClaimCheckResultV1<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(ClaimCheckErrorV1::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn require_distinct(identities: &[[u8; 32]]) -> ClaimCheckResultV1<()> {
    for (index, left) in identities.iter().enumerate() {
        require_nonzero(*left)?;
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
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use crate::CallerRole;
    use crate::fractional_claim_check_compaction_request_v1::FractionalCompactionCoordinatesV1;
    use crate::terminal_settlement_v3::{
        TerminalSettlementRequestInputV3, TerminalSettlementRequestV3,
    };

    const ESCROW: [u8; 32] = [40; 32];
    const VAULT: [u8; 32] = [41; 32];
    const ROOT: [u8; 32] = [9; 32];
    const AGGREGATE: [u8; 32] = [30; 32];
    const SHARD_MINT: [u8; 32] = [31; 32];
    const RECORD: [u8; 32] = [42; 32];
    const DIGEST: [u8; 32] = [55; 32];

    /// 1,000 shards at 100 per claim is ten claims, at 4,000 atoms each.
    const DENOMINATOR: u64 = 100;
    const SUPPLY: u64 = 1_000;
    const PAYOUT_PER_CLAIM: u64 = 4_000;
    const ESCROWED: u64 = 40_000;

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
            payout_per_claim: PAYOUT_PER_CLAIM,
        }
    }

    fn request() -> FractionalCompactToClaimCheckRequestV1 {
        FractionalCompactToClaimCheckRequestV1::new(
            coordinates(),
            TerminalSettlementRequestV3::new(settlement_input()).expect("settlement"),
        )
        .expect("request")
    }

    fn input() -> FractionalClaimCheckCompactionReceiptInputV1 {
        FractionalClaimCheckCompactionReceiptInputV1 {
            request_digest: DIGEST,
            aggregate: AGGREGATE,
            shard_mint: SHARD_MINT,
            escrow: ESCROW,
            record: RECORD,
            escrowed_atoms: ESCROWED,
            denominator: DENOMINATOR,
            payout_per_claim: PAYOUT_PER_CLAIM,
            compacted_shard_supply: SUPPLY,
            minted: true,
        }
    }

    fn receipt() -> FractionalClaimCheckCompactionReceiptV1 {
        FractionalClaimCheckCompactionReceiptV1::new(&request(), input()).expect("receipt")
    }

    #[test]
    fn a_compaction_receipt_round_trips_at_its_one_exact_width() {
        let value = receipt();
        let bytes = value.to_bytes().expect("bytes");
        assert_eq!(bytes.len(), FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1);
        assert_eq!(
            bytes.get(..8),
            Some(FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_MAGIC_V1.as_slice())
        );
        assert_eq!(
            FractionalClaimCheckCompactionReceiptV1::decode(&bytes),
            Ok(value)
        );
        // Every accessor a parent reads, pinned to the literal it was built with.
        assert_eq!(value.root(), ROOT);
        assert_eq!(value.shard_mint(), SHARD_MINT);
        assert_eq!(value.record(), RECORD);
        assert_eq!(value.escrowed_atoms(), ESCROWED);
        assert_eq!(value.compacted_shard_supply(), SUPPLY);
        assert!(value.minted());
    }

    #[test]
    fn the_root_and_the_coordinate_come_from_the_request_and_not_from_the_caller() {
        // The one-author rule, as an assertion. The input struct carries no root
        // and no coordinate, so there is no second field for a caller to set to
        // something the request disagrees with.
        let value = receipt();
        assert_eq!(value.root(), request().root());
        assert_eq!(value.root(), settlement_input().owner);
    }

    #[test]
    fn a_receipt_for_some_other_request_is_refused_four_different_ways() {
        let value = receipt();
        assert_eq!(value.verify_for(request(), DIGEST), Ok(()));

        // (a) a different request digest -- the receipt is for another
        // transaction, whatever else it says.
        assert!(value.verify_for(request(), [56; 32]).is_err());

        // (b) a different representation coordinate.
        let mut other = coordinates();
        other.representation_coordinate = 7;
        let shifted = FractionalCompactToClaimCheckRequestV1::new(
            other,
            TerminalSettlementRequestV3::new(settlement_input()).expect("settlement"),
        )
        .expect("request");
        assert!(value.verify_for(shifted, DIGEST).is_err());

        // (c) a different promised rate. The record pays this rate forever, so
        // the parent refuses a child that recorded a different one.
        let mut rerated = coordinates();
        rerated.payout_per_claim = PAYOUT_PER_CLAIM + 1;
        let rerated_request = FractionalCompactToClaimCheckRequestV1::new(
            rerated,
            TerminalSettlementRequestV3::new(settlement_input()).expect("settlement"),
        )
        .expect("request");
        assert!(value.verify_for(rerated_request, DIGEST).is_err());

        // (d) a different root -- the reserve Position's owner.
        let mut other_owner = settlement_input();
        other_owner.owner = [99; 32];
        let other_root = FractionalCompactToClaimCheckRequestV1::new(
            coordinates(),
            TerminalSettlementRequestV3::new(other_owner).expect("settlement"),
        )
        .expect("request");
        assert!(value.verify_for(other_root, DIGEST).is_err());
    }

    #[test]
    fn minting_and_escrowing_are_welded_in_both_directions() {
        // The native plan's weld, restated where a PARENT can reach it. A record
        // promising nothing is a record nobody redeems and an outstanding count
        // that never falls; atoms escrowed against no record are atoms nobody
        // can reach. Both halves are refused, not just the one a route is
        // likelier to get wrong.
        let mut promises_nothing = input();
        promises_nothing.escrowed_atoms = 0;
        assert!(
            FractionalClaimCheckCompactionReceiptV1::new(&request(), promises_nothing).is_err(),
            "a minted record escrowing nothing must be refused"
        );

        let mut unreachable = input();
        unreachable.minted = false;
        assert!(
            FractionalClaimCheckCompactionReceiptV1::new(&request(), unreachable).is_err(),
            "atoms escrowed against no record must be refused"
        );

        // And the honest empty compaction: nothing formed, nothing escrowed,
        // nothing minted. Below the denominator, so the quotient floors to zero.
        let empty = FractionalClaimCheckCompactionReceiptInputV1 {
            escrowed_atoms: 0,
            compacted_shard_supply: DENOMINATOR - 1,
            minted: false,
            ..input()
        };
        let value = FractionalClaimCheckCompactionReceiptV1::new(&request(), empty)
            .expect("an empty compaction is a legitimate outcome");
        assert!(!value.minted());
        assert_eq!(value.escrowed_atoms(), 0);
    }

    #[test]
    fn a_receipt_whose_arithmetic_does_not_close_is_refused() {
        // The receipt's own claim, checkable by a parent holding nothing else:
        // ten whole claims at 4,000 atoms is 40,000, and any other escrowed
        // balance describes a compaction that did not conserve.
        for wrong in [ESCROWED - 1, ESCROWED + 1, 1] {
            let mut broken = input();
            broken.escrowed_atoms = wrong;
            assert!(
                FractionalClaimCheckCompactionReceiptV1::new(&request(), broken).is_err(),
                "escrowed {wrong} must not pass against a rate that forms {ESCROWED}"
            );
        }
        assert!(receipt().opening_escrow_is_consistent());
    }

    #[test]
    fn a_denominator_no_terms_could_have_produced_is_refused() {
        // The exposure terms refuse zero and one at decode
        // (`NonFractionalDenominator`), and after retirement the terms are gone
        // -- so a receipt claiming one has to be refused here or nowhere.
        for degenerate in [0, 1] {
            let mut broken = input();
            broken.denominator = degenerate;
            broken.escrowed_atoms = 0;
            broken.minted = false;
            assert!(
                FractionalClaimCheckCompactionReceiptV1::new(&request(), broken).is_err(),
                "denominator {degenerate} is not a fractionalization"
            );
        }
    }

    #[test]
    fn the_minted_flag_has_exactly_two_valid_encodings_and_not_two_hundred_and_fifty_six() {
        // A bool is one bit and the wire gives it eight. Under a `!= 0` decode,
        // 255 distinct byte strings would all mean `true` -- and a receipt with
        // more than one valid encoding is a receipt whose DIGEST a caller can
        // choose, which is the property the parent's request/receipt binding
        // rests on. Stated over every byte rather than over the two a reader
        // would think to try.
        let mut bytes = receipt().to_bytes().expect("bytes");
        for candidate in 0..=u8::MAX {
            bytes[MINTED_OFFSET] = candidate;
            let decoded = FractionalClaimCheckCompactionReceiptV1::decode(&bytes);
            match candidate {
                // `true` is the round-trip; `false` fails the weld here because
                // this fixture escrows atoms, which is itself the weld working.
                1 => assert!(decoded.is_ok()),
                _ => assert!(
                    decoded.is_err(),
                    "byte {candidate} must not decode as a boolean"
                ),
            }
        }
    }

    #[test]
    fn every_reserved_byte_must_be_zero_and_the_tail_closes_the_record() {
        let base = receipt().to_bytes().expect("bytes");
        for offset in (RESERVED_HEADER_OFFSET..RESERVED_HEADER_OFFSET + RESERVED_HEADER_BYTES)
            .chain(RESERVED_BODY_OFFSET..RESERVED_BODY_OFFSET + RESERVED_BODY_BYTES)
        {
            let mut bytes = base;
            bytes[offset] = 1;
            assert!(
                FractionalClaimCheckCompactionReceiptV1::decode(&bytes).is_err(),
                "reserved byte {offset} must refuse a non-zero value"
            );
        }
        // The reserve is where a later field must come from, so it has to
        // actually reach the end of the record.
        assert_eq!(
            RESERVED_BODY_OFFSET + RESERVED_BODY_BYTES,
            FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1
        );
    }

    #[test]
    fn a_receipt_naming_one_account_twice_is_refused() {
        // Five identities, all distinct and all non-zero. An aggregate equal to
        // the shard Mint, or a record equal to the escrow, is one account
        // standing in two roles -- which is how a compaction would report
        // paying into the thing it claims to have created.
        for (label, broken) in [
            (
                "aggregate == shard mint",
                FractionalClaimCheckCompactionReceiptInputV1 {
                    shard_mint: AGGREGATE,
                    ..input()
                },
            ),
            (
                "record == escrow",
                FractionalClaimCheckCompactionReceiptInputV1 {
                    record: ESCROW,
                    ..input()
                },
            ),
            (
                "zero record",
                FractionalClaimCheckCompactionReceiptInputV1 {
                    record: [0; 32],
                    ..input()
                },
            ),
            (
                "zero request digest",
                FractionalClaimCheckCompactionReceiptInputV1 {
                    request_digest: [0; 32],
                    ..input()
                },
            ),
        ] {
            assert!(
                FractionalClaimCheckCompactionReceiptV1::new(&request(), broken).is_err(),
                "{label} must be refused"
            );
        }
    }

    #[test]
    fn a_width_that_is_not_the_exact_width_is_refused() {
        let bytes = receipt().to_bytes().expect("bytes");
        assert!(FractionalClaimCheckCompactionReceiptV1::decode(&bytes[..255]).is_err());
        let mut longer = bytes.to_vec();
        longer.push(0);
        assert!(FractionalClaimCheckCompactionReceiptV1::decode(&longer).is_err());
        // And a receipt from another family at the same width is refused on its
        // magic rather than accidentally admitted on its shape.
        let mut wrong_magic = bytes;
        wrong_magic[0] = b'X';
        assert!(FractionalClaimCheckCompactionReceiptV1::decode(&wrong_magic).is_err());
    }
}
