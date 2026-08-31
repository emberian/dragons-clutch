//! Durable claim-checks whose claimant is an instrument rather than a person.
//!
//! [`crate::claim_check_v1`] closes R3 for a native Position: the payout is
//! resolved at compaction into a record only its one holder can open. That
//! shape refuses the Fractional reserve Position outright, and must
//! (`claim_check_compaction_v1::owner_kind_can_open_a_claim_check`), because the
//! reserve is owned by a program-derived address that can never sign for its own
//! payout. Its real claimants are the holders of a shard Mint: plural, unknown
//! to the Position, and holding an instrument rather than an account.
//!
//! This module is the second record type, and the reason it is a second type
//! rather than a second use of the first is that the two differ in the one place
//! a shared width would hide. A native record's `entitlement_atoms` means "the
//! payout, paid once, then the record closes". Here the analogous field means
//! "the collateral still escrowed, paid down across many burns". One width
//! meaning two field layouts would turn the house decode order -- exact width,
//! magic, version, kind, every reserved run, then fields -- into a union whose
//! arms diverge after the kind byte, and the hostile decode stops being cheap to
//! audit.
//!
//! # The claimant is the Mint, and that is what dissolves the unsignable owner
//!
//! The record's coordinates are `[FRACTIONAL_CLAIM_CHECK_SEED_V1, aggregate,
//! shard_mint]`. The address therefore proves which *instrument* the record
//! answers to, exactly as the native record's address proves which *person* it
//! answers to. Nobody is named as the payee, so nobody has to sign as the payee,
//! so the fact that the reserve Position's owner is a PDA stops being a defect
//! and becomes irrelevant: the record was never going to be paid to that owner.
//! Entitlement is proved by presenting shards, which only their holder can move.
//!
//! # The arithmetic, and why there is no pro-rata
//!
//! `divide_exposure_shards_v2` is the sole quotient/remainder boundary in the
//! Fractional family, and the payout below it is a multiplication by a
//! per-coordinate constant the terminal evaluator produces once:
//!
//! ```text
//! whole_claims     = shard_atoms / denominator      (floor, the ONLY division)
//! consumed         = whole_claims * denominator     (burned)
//! change           = shard_atoms - consumed         (stays with the holder)
//! collateral_atoms = whole_claims * payout_per_claim
//! ```
//!
//! A record storing `denominator` and `payout_per_claim` therefore pays, to the
//! atom, what on-time redemption would have paid -- from the same two numbers
//! and the same two operations, with no second rounding boundary to get wrong
//! and no last-burner remainder to dispose of. Sub-denominator dust is not a
//! claim on collateral before compaction (`NoWholeClaim` is a refusal, not a
//! zero payout) and does not become one after.
//!
//! # The lifetime, argued rather than inherited
//!
//! The native discipline is absence-based anti-replay: the record is created
//! once at a vacant address and closed on its one redemption, so a closed
//! account is an unrepeatable one and there is no cursor, revision or counter to
//! get wrong. A paid-down record cannot be absence-based *in the middle*, so the
//! question is what replaces it there, and whether absence returns at the end.
//!
//! **In the middle, the shards are the anti-replay.** A redemption burns
//! `whole_claims * denominator` shards, and burned shards cannot be burned
//! twice. The record is a budget, not a ticket: it does not authorize a payment,
//! it bounds the total of them. Presenting the same shards again fails at the
//! Token program, not here.
//!
//! **At the end, absence returns exactly.** [`Self::pay_down`] reports
//! [`FractionalPayDownV1::Settled`] when the escrowed balance reaches zero, and
//! the route closes the record there rather than persisting it -- so a record
//! promising nothing never exists, in the constructor or across the wire, which
//! is the same rule [`crate::claim_check_v1::ClaimCheckV1`] enforces for the
//! same reason: a record nobody has any motive to redeem would pin the escrow's
//! outstanding count above zero forever and rebuild the perpetual account this
//! family exists to remove.
//!
//! **And the remainder goes nowhere, which is a decision, not an omission.**
//! Compaction escrows `floor(supply / denominator) * payout_per_claim` -- every
//! whole claim the outstanding supply could form. Holders can only redeem
//! `sum_i floor(shards_i / denominator)`, which is smaller whenever the supply
//! is spread across accounts carrying sub-denominator dust. That gap is not
//! stranded: shard *transfer* is ordinary, only burning is permissioned, so any
//! two dust holders can consolidate and redeem what neither could alone. So the
//! remainder stays escrowed against a claim that can still be formed, and the
//! record can stay open forever if nobody ever forms it. That is the same ruling
//! the escrow's own close already makes -- an escrow still holding a live
//! claim-check is holding somebody's collateral, and closing it would be taking
//! their money -- applied one level down. A sweep-to-a-beneficiary rule was
//! considered and rejected: it would pay a third party out of collateral whose
//! claimants had merely not coordinated yet.
//!
//! This module is pure wire and identity. Conservation lives in the plan
//! structs; account authority lives in the program.

use core::convert::TryInto;

use crate::claim_check_v1::{ClaimCheckErrorV1, ClaimCheckResultV1};

/// Exact width of one durable fractional claim-check record.
pub const FRACTIONAL_CLAIM_CHECK_BYTES_V1: usize = 320;

/// Persisted fractional claim-check record magic.
pub const FRACTIONAL_CLAIM_CHECK_RECORD_MAGIC_V1: [u8; 8] = *b"DCLTFCK1";

/// Fractional compaction request magic.
pub const FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1: [u8; 8] = *b"DCLTFCC1";

/// Fractional claim-check redemption request magic.
pub const FRACTIONAL_CLAIM_CHECK_REDEEM_MAGIC_V1: [u8; 8] = *b"DCLTFCR1";

/// Persisted fractional-record kind discriminant.
///
/// Third in the family, after the native record (1) and the escrow (2). The
/// discriminant is what makes a hostile decode of one type against another's
/// bytes fail at a named byte rather than in a field.
pub const FRACTIONAL_CLAIM_CHECK_RECORD_KIND_V1: u8 = 3;

/// Solana's maximum length for one PDA seed segment.
///
/// A domain longer than this is not merely unusual, it is **underivable**:
/// `create_program_address` refuses `MaxSeedLengthExceeded` for every bump, so
/// `find_program_address` finds none and panics. This crate is `no_std` and does
/// not depend on the SDK, so the limit is restated here and enforced below.
/// `dclutch-dealer-codec` restates it the same way for the same reason.
pub const MAX_PDA_SEED_BYTES_V1: usize = 32;

/// Canonical fractional claim-check PDA seed domain.
///
/// `frac-` rather than `fractional-`: the spelled-out form is 33 bytes, one
/// over the maximum, so no address could ever be derived for this record and
/// every route naming it would have aborted. The abbreviation is on the
/// qualifier and never on `claim-check`, which is the family word the sibling
/// domains (`dclutch:claim-check:v1`, `:claim-check-escrow:v1`,
/// `:claim-check-vault:v1`) all spell in full.
pub const FRACTIONAL_CLAIM_CHECK_SEED_V1: &[u8] = b"dclutch:frac-claim-check:v1";

// The assertion is the actual fix; the shorter string is only a shorter string
// until something stops the next one from growing. `dclutch-dealer-codec` says
// the same thing beside its own domains, and this family had no such guard,
// which is exactly how a 33-byte seed shipped.
const _: () = assert!(
    FRACTIONAL_CLAIM_CHECK_SEED_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the fractional claim-check domain must be a usable PDA seed"
);
const _: () = assert!(
    crate::claim_check_v1::CLAIM_CHECK_SEED_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the native claim-check domain must be a usable PDA seed"
);
const _: () = assert!(
    crate::claim_check_v1::CLAIM_CHECK_ESCROW_SEED_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the claim-check escrow domain must be a usable PDA seed"
);
const _: () = assert!(
    crate::claim_check_v1::CLAIM_CHECK_VAULT_SEED_V1.len() <= MAX_PDA_SEED_BYTES_V1,
    "the claim-check vault domain must be a usable PDA seed"
);

const RECORD_VERSION_OFFSET: usize = 8;
const RECORD_KIND_OFFSET: usize = 10;
const RECORD_BUMP_OFFSET: usize = 11;
const RECORD_RESERVED_HEADER_OFFSET: usize = 12;
const RECORD_AGGREGATE_OFFSET: usize = 16;
const RECORD_SHARD_MINT_OFFSET: usize = 48;
const RECORD_MARKET_OFFSET: usize = 80;
const RECORD_RELEASE_SET_OFFSET: usize = 112;
const RECORD_VAULT_OFFSET: usize = 144;
const RECORD_COLLATERAL_MINT_OFFSET: usize = 176;
const RECORD_ATOMS_DIGEST_OFFSET: usize = 208;
const RECORD_ESCROWED_OFFSET: usize = 240;
const RECORD_DENOMINATOR_OFFSET: usize = 248;
const RECORD_PAYOUT_PER_CLAIM_OFFSET: usize = 256;
const RECORD_COMPACTED_SUPPLY_OFFSET: usize = 264;
const RECORD_COMPACTED_SLOT_OFFSET: usize = 272;
const RECORD_GENERATION_OFFSET: usize = 280;
const RECORD_COORDINATE_OFFSET: usize = 288;
const RECORD_RESERVED_BODY_OFFSET: usize = 292;

const RESERVED_HEADER_BYTES: usize = 4;
const RECORD_RESERVED_BODY_BYTES: usize = 28;

/// Largest Claims representation width the exposure terms admit.
///
/// Mirrored from `dclutch_fractional_claim_kernel::exposure_v2`, which refuses
/// `representation_width == 0 || > 256` at decode. A record naming a coordinate
/// no terms could ever declare is refused here rather than deeper, where the
/// terms are no longer available to compare against: after retirement they are
/// gone, and this record has to be self-authenticating.
pub const FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1: u32 = 256;

/// Canonical fractional claim-check PDA coordinates.
///
/// The claimant is the shard Mint, so the Mint is a seed. A caller naming the
/// wrong instrument derives an address that is not the account they passed, and
/// there is no claimant field on any wire to forge. This is the native record's
/// own identity discipline with the holder replaced by the instrument, which is
/// the whole of the fractional correction: the native record could not name a
/// payee that was able to sign, and this one names no payee at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckSeedsV1 {
    aggregate: [u8; 32],
    shard_mint: [u8; 32],
}

impl FractionalClaimCheckSeedsV1 {
    /// Construct the unique fractional claim-check coordinates for one Mint.
    pub fn new(aggregate: [u8; 32], shard_mint: [u8; 32]) -> ClaimCheckResultV1<Self> {
        require_nonzero(aggregate)?;
        require_nonzero(shard_mint)?;
        if aggregate == shard_mint {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        Ok(Self {
            aggregate,
            shard_mint,
        })
    }

    /// Borrow the sole exact fractional claim-check PDA seed order, no bump.
    pub fn as_slices(&self) -> [&[u8]; 3] {
        [
            FRACTIONAL_CLAIM_CHECK_SEED_V1,
            &self.aggregate,
            &self.shard_mint,
        ]
    }

    /// Return the Claims aggregate coordinate.
    pub const fn aggregate(self) -> [u8; 32] {
        self.aggregate
    }

    /// Return the shard Mint coordinate: the claimant.
    pub const fn shard_mint(self) -> [u8; 32] {
        self.shard_mint
    }
}

/// One durable, fixed-width, permanently redeemable fractional claim-check.
///
/// Fixed width for the same reason the native record is: the market's runtime
/// outcome width is gone by the time anyone reads this, so the record keeps a
/// digest of the reserve Position's atom vector as evidence rather than the
/// vector itself. A 256-outcome market's fractional claim-check costs the rent,
/// and the redemption compute, of a binary market's.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimCheckV1 {
    /// Claims aggregate the reserve Position was admitted against; a PDA seed.
    pub aggregate: [u8; 32],
    /// The shard Mint this record answers to; a PDA seed, never a wire field.
    pub shard_mint: [u8; 32],
    /// Logical Core Market identity, retained for audit after closure.
    pub market: [u8; 32],
    /// Release set the market pinned at founding.
    pub release_set: [u8; 32],
    /// Escrow vault token account holding this coordinate's collateral.
    pub vault: [u8; 32],
    /// Collateral mint the escrowed atoms are denominated in.
    pub collateral_mint: [u8; 32],
    /// Digest of the reserve Position's per-outcome atom vector, as evidence.
    pub position_atoms_digest: [u8; 32],
    /// Collateral atoms still escrowed: the balance this record pays down.
    pub escrowed_atoms: u64,
    /// Exact shard atoms per whole Claims coordinate; the sole divisor.
    pub denominator: u64,
    /// Exact collateral atoms per whole Claims coordinate; the sole multiplier.
    pub payout_per_claim: u64,
    /// Shard supply observed at compaction, as evidence for the escrowed total.
    ///
    /// Not read by any arithmetic. It is what makes the compaction's own claim
    /// checkable by anyone holding only this record: the escrowed balance it
    /// opened with must have been
    /// `(compacted_shard_supply / denominator) * payout_per_claim`, and
    /// [`Self::opening_escrow_is_consistent`] states exactly that.
    pub compacted_shard_supply: u64,
    /// Clock slot at which compaction resolved this coordinate.
    pub compacted_slot: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Claims representation coordinate this Mint stands for.
    pub representation_coordinate: u32,
    /// Persisted fractional claim-check PDA bump.
    pub bump: u8,
}

/// What one pay-down left behind.
///
/// Stated as a sum rather than as a record with a zero field, because the two
/// outcomes are different acts: one rewrites the record, the other closes it.
/// A route that received a zero-balance record and had to remember to close it
/// is a route that can forget.
///
/// The variants differ in size by the width of a record, and that is the point
/// rather than an oversight: this crate is `no_std` with no allocator, so the
/// alternative to carrying the record in the live arm is an `Option` whose
/// `None` silently means "settled" -- exactly the implicit meaning the named sum
/// exists to remove.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalPayDownV1 {
    /// Collateral remains escrowed; persist this record.
    Remaining(FractionalClaimCheckV1),
    /// The escrowed balance reached zero; close the record instead.
    Settled,
}

impl FractionalPayDownV1 {
    /// The record still to be persisted, or `None` once it has settled.
    #[must_use]
    pub const fn remaining(self) -> Option<FractionalClaimCheckV1> {
        match self {
            Self::Remaining(record) => Some(record),
            Self::Settled => None,
        }
    }

    /// Whether this pay-down exhausted the escrowed balance.
    #[must_use]
    pub const fn is_settled(self) -> bool {
        matches!(self, Self::Settled)
    }
}

impl FractionalClaimCheckV1 {
    /// Construct and canonicalize one fractional claim-check.
    pub fn new(self) -> ClaimCheckResultV1<Self> {
        self.validate()?;
        Ok(self)
    }

    /// Hostile-decode one exact persisted fractional claim-check.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        exact_width(input, FRACTIONAL_CLAIM_CHECK_BYTES_V1)?;
        exact(input, 0, &FRACTIONAL_CLAIM_CHECK_RECORD_MAGIC_V1)?;
        if read_u16(input, RECORD_VERSION_OFFSET)?
            != crate::claim_check_v1::CLAIM_CHECK_WIRE_VERSION_V1
        {
            return Err(ClaimCheckErrorV1::InvalidHeader);
        }
        if read_byte(input, RECORD_KIND_OFFSET)? != FRACTIONAL_CLAIM_CHECK_RECORD_KIND_V1 {
            return Err(ClaimCheckErrorV1::UnknownTag);
        }
        require_zero(input, RECORD_RESERVED_HEADER_OFFSET, RESERVED_HEADER_BYTES)?;
        require_zero(
            input,
            RECORD_RESERVED_BODY_OFFSET,
            RECORD_RESERVED_BODY_BYTES,
        )?;
        Self {
            aggregate: read_array(input, RECORD_AGGREGATE_OFFSET)?,
            shard_mint: read_array(input, RECORD_SHARD_MINT_OFFSET)?,
            market: read_array(input, RECORD_MARKET_OFFSET)?,
            release_set: read_array(input, RECORD_RELEASE_SET_OFFSET)?,
            vault: read_array(input, RECORD_VAULT_OFFSET)?,
            collateral_mint: read_array(input, RECORD_COLLATERAL_MINT_OFFSET)?,
            position_atoms_digest: read_array(input, RECORD_ATOMS_DIGEST_OFFSET)?,
            escrowed_atoms: read_u64(input, RECORD_ESCROWED_OFFSET)?,
            denominator: read_u64(input, RECORD_DENOMINATOR_OFFSET)?,
            payout_per_claim: read_u64(input, RECORD_PAYOUT_PER_CLAIM_OFFSET)?,
            compacted_shard_supply: read_u64(input, RECORD_COMPACTED_SUPPLY_OFFSET)?,
            compacted_slot: read_u64(input, RECORD_COMPACTED_SLOT_OFFSET)?,
            generation: read_u64(input, RECORD_GENERATION_OFFSET)?,
            representation_coordinate: read_u32(input, RECORD_COORDINATE_OFFSET)?,
            bump: read_byte(input, RECORD_BUMP_OFFSET)?,
        }
        .new()
    }

    /// Encode canonical persisted bytes.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; FRACTIONAL_CLAIM_CHECK_BYTES_V1]> {
        self.validate()?;
        let mut output = [0; FRACTIONAL_CLAIM_CHECK_BYTES_V1];
        write(&mut output, 0, &FRACTIONAL_CLAIM_CHECK_RECORD_MAGIC_V1)?;
        write(
            &mut output,
            RECORD_VERSION_OFFSET,
            &crate::claim_check_v1::CLAIM_CHECK_WIRE_VERSION_V1.to_le_bytes(),
        )?;
        write(
            &mut output,
            RECORD_KIND_OFFSET,
            &[FRACTIONAL_CLAIM_CHECK_RECORD_KIND_V1],
        )?;
        write(&mut output, RECORD_BUMP_OFFSET, &[self.bump])?;
        for (offset, value) in [
            (RECORD_AGGREGATE_OFFSET, self.aggregate),
            (RECORD_SHARD_MINT_OFFSET, self.shard_mint),
            (RECORD_MARKET_OFFSET, self.market),
            (RECORD_RELEASE_SET_OFFSET, self.release_set),
            (RECORD_VAULT_OFFSET, self.vault),
            (RECORD_COLLATERAL_MINT_OFFSET, self.collateral_mint),
            (RECORD_ATOMS_DIGEST_OFFSET, self.position_atoms_digest),
        ] {
            write(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (RECORD_ESCROWED_OFFSET, self.escrowed_atoms),
            (RECORD_DENOMINATOR_OFFSET, self.denominator),
            (RECORD_PAYOUT_PER_CLAIM_OFFSET, self.payout_per_claim),
            (RECORD_COMPACTED_SUPPLY_OFFSET, self.compacted_shard_supply),
            (RECORD_COMPACTED_SLOT_OFFSET, self.compacted_slot),
            (RECORD_GENERATION_OFFSET, self.generation),
        ] {
            write(&mut output, offset, &value.to_le_bytes())?;
        }
        write(
            &mut output,
            RECORD_COORDINATE_OFFSET,
            &self.representation_coordinate.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Return the exact PDA coordinates this record must live at.
    pub fn seeds(self) -> ClaimCheckResultV1<FractionalClaimCheckSeedsV1> {
        FractionalClaimCheckSeedsV1::new(self.aggregate, self.shard_mint)
    }

    /// Whole Claims coordinates a holder's shard balance can form.
    ///
    /// **The only division in this module, and it floors.** It is
    /// `divide_exposure_shards_v2`'s own quotient, restated over the two numbers
    /// this record persists rather than over terms that no longer exist. A
    /// balance below the denominator forms nothing, which is a refusal upstream
    /// (`NoWholeClaim`) and is zero here; either way it is not a claim on
    /// collateral.
    #[must_use]
    pub const fn whole_claims_for(self, shard_atoms: u64) -> u64 {
        shard_atoms / self.denominator
    }

    /// Shard atoms exactly `whole_claims` coordinates consume. Never rounds.
    pub fn consumed_shards(self, whole_claims: u64) -> ClaimCheckResultV1<u64> {
        whole_claims
            .checked_mul(self.denominator)
            .ok_or(ClaimCheckErrorV1::Arithmetic)
    }

    /// Collateral atoms exactly `whole_claims` coordinates are owed.
    ///
    /// A multiplication by a per-coordinate constant, which is why a fractional
    /// claim-check pays to the atom what on-time redemption would have paid:
    /// same two numbers, same two operations, no second rounding boundary.
    pub fn claim_payout(self, whole_claims: u64) -> ClaimCheckResultV1<u64> {
        whole_claims
            .checked_mul(self.payout_per_claim)
            .ok_or(ClaimCheckErrorV1::Arithmetic)
    }

    /// Reduce the escrowed balance by one redemption's collateral.
    ///
    /// Refuses an overdraw rather than saturating: the escrowed balance is what
    /// every other holder's claim is paid out of, so paying more than it holds
    /// is theft from them, not an accounting slip. Reaching exactly zero is
    /// [`FractionalPayDownV1::Settled`], and the route closes the record there.
    pub fn pay_down(self, collateral_atoms: u64) -> ClaimCheckResultV1<FractionalPayDownV1> {
        self.validate()?;
        if collateral_atoms == 0 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        let escrowed_atoms = self
            .escrowed_atoms
            .checked_sub(collateral_atoms)
            .ok_or(ClaimCheckErrorV1::Arithmetic)?;
        if escrowed_atoms == 0 {
            return Ok(FractionalPayDownV1::Settled);
        }
        Ok(FractionalPayDownV1::Remaining(
            Self {
                escrowed_atoms,
                ..self
            }
            .new()?,
        ))
    }

    /// Whether the opening balance matches what the observed supply could form.
    ///
    /// The compaction's whole claim, checkable from this record alone once every
    /// other account is gone. It is deliberately *not* an invariant of a live
    /// record -- a paid-down record fails it, and must -- so it takes the
    /// opening balance as an argument rather than reading the current one.
    #[must_use]
    pub fn opening_escrow_is_consistent(self, opening_escrowed_atoms: u64) -> bool {
        self.denominator != 0
            && self.claim_payout(self.whole_claims_for(self.compacted_shard_supply))
                == Ok(opening_escrowed_atoms)
    }

    fn validate(self) -> ClaimCheckResultV1<()> {
        require_distinct(&[
            self.aggregate,
            self.shard_mint,
            self.market,
            self.release_set,
            self.vault,
            self.collateral_mint,
        ])?;
        require_nonzero(self.position_atoms_digest)?;
        // A denominator of one is not a fractionalization, and the exposure
        // terms refuse it at decode (`NonFractionalDenominator`). Restating the
        // refusal here is what keeps a record from claiming a denominator no
        // terms could have produced -- and after retirement the terms are gone,
        // so this record is the only thing left to argue with.
        if self.denominator <= 1 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        // A coordinate whose terminal payout is zero mints no record at all,
        // for the reason the native record refuses a zero entitlement: nobody
        // would ever have a motive to redeem it, and it would pin the escrow's
        // outstanding count above zero forever.
        if self.payout_per_claim == 0 || self.escrowed_atoms == 0 || self.generation == 0 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        if self.representation_coordinate >= FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        Ok(())
    }
}

/// Exact fractional claim-check redemption frame width.
pub const FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1: usize = 9;

/// One account role in the fractional claim-check redemption frame.
///
/// The declared set is deliberately *larger* than [`Self::frame`]. Every role
/// answers [`Self::survives_retirement`], and the frame admits exactly the ones
/// that answer `true` -- so an account this route must never reach for is stated
/// here and excluded by a test, rather than being absent for no recorded reason.
///
/// The excluded role is the one that matters, and it is the correction this
/// module exists to carry. Burning a shard is not an ordinary Token act: the
/// shard Mint carries Token-2022's `PermissionedBurn` extension, and its
/// configured authority is a second required signer on every burn. A *standard*
/// burn is refused outright while that extension is present, so a holder's own
/// signature can never burn a shard by itself. At founding that authority is the
/// Fractional capability root -- a **Trading**-derived PDA. Claims cannot sign
/// it, and it does not outlive the market, so a redemption frame containing it
/// would be a promise that stops working exactly when it is needed.
///
/// Compaction therefore re-points the Mint's burn authority to the escrow, whose
/// PDA Claims *can* sign, and the redemption frame that results carries the
/// escrow twice over: once as the outstanding-count bookkeeper and once as the
/// burn approver. That is why [`Self::FractionalCapabilityRoot`] is declared and
/// refused rather than simply forgotten.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalClaimCheckRedemptionRoleV1 {
    /// The shard holder, who signs as their token account's owner and is paid.
    Holder,
    /// The fractional claim-check record, paid down or closed.
    FractionalClaimCheckRecord,
    /// The per-market escrow: outstanding-count bookkeeper and burn approver.
    Escrow,
    /// The escrow vault, debited by exactly this redemption's payout.
    Vault,
    /// The holder's own collateral token account, credited.
    HolderCollateralTokens,
    /// The collateral mint, for a checked transfer.
    CollateralMint,
    /// The shard mint, whose supply this redemption burns down.
    ShardMint,
    /// The holder's own shard token account, burned from.
    HolderShardTokens,
    /// The Token program owning both mints.
    TokenProgram,
    /// The Fractional capability root: declared, and refused.
    ///
    /// A Trading-derived PDA, and the shard Mint's `PermissionedBurn` authority
    /// at founding. It is named here so that the reason it is not in the frame
    /// is a stated answer rather than an absence, and so that an edit reaching
    /// for it has to change [`Self::survives_retirement`] first.
    FractionalCapabilityRoot,
}

impl FractionalClaimCheckRedemptionRoleV1 {
    /// The exact ordered frame: every role that outlives the market, in order.
    #[must_use]
    pub const fn frame() -> [Self; FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1] {
        [
            Self::Holder,
            Self::FractionalClaimCheckRecord,
            Self::Escrow,
            Self::Vault,
            Self::HolderCollateralTokens,
            Self::CollateralMint,
            Self::ShardMint,
            Self::HolderShardTokens,
            Self::TokenProgram,
        ]
    }

    /// Every role this route has an opinion about, admitted or refused.
    #[must_use]
    pub const fn declared() -> [Self; FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1 + 1] {
        [
            Self::Holder,
            Self::FractionalClaimCheckRecord,
            Self::Escrow,
            Self::Vault,
            Self::HolderCollateralTokens,
            Self::CollateralMint,
            Self::ShardMint,
            Self::HolderShardTokens,
            Self::TokenProgram,
            Self::FractionalCapabilityRoot,
        ]
    }

    /// Whether an account in this role still exists after the market retires.
    ///
    /// **Every role in [`Self::frame`] must answer `true`, and a test enforces
    /// it. Every role that answers `false` must be absent from the frame, and
    /// the same test enforces that.** The method exists so that adding an
    /// account to this route is not a silent act.
    ///
    /// The shard Mint answers `true`, and that answer is a debt this family owes
    /// elsewhere: `RetireCoordinate` closes the Mint today and requires its
    /// supply to be zero to do so. A compacted coordinate has nonzero supply by
    /// construction -- the outstanding shards *are* the durable claim -- so the
    /// Mint must survive retirement, which adds one perpetual Mint per
    /// unredeemed coordinate to the residue. Named, not absolved.
    #[must_use]
    pub const fn survives_retirement(self) -> bool {
        match self {
            // Created by compaction, kept alive by it, or independent of the
            // market entirely.
            Self::Holder
            | Self::FractionalClaimCheckRecord
            | Self::Escrow
            | Self::Vault
            | Self::HolderCollateralTokens
            | Self::CollateralMint
            | Self::ShardMint
            | Self::HolderShardTokens
            | Self::TokenProgram => true,
            // A Trading-derived PDA, and the shard Mint's burn authority at
            // founding. It does not outlive the market and Claims could not sign
            // it if it did, which is precisely why compaction has to re-point
            // the burn authority to the escrow before the market goes away.
            Self::FractionalCapabilityRoot => false,
        }
    }

    /// Exact privileges this role carries in the frame.
    #[must_use]
    pub const fn privileges(self) -> (bool, bool) {
        match self {
            // signer, writable
            Self::Holder => (true, true),
            Self::FractionalClaimCheckRecord
            | Self::Escrow
            | Self::Vault
            | Self::HolderCollateralTokens
            | Self::ShardMint
            | Self::HolderShardTokens => (false, true),
            Self::CollateralMint | Self::TokenProgram => (false, false),
            // Not in the frame; stated so a later edit that adds it has to
            // delete this arm rather than merely append an account.
            Self::FractionalCapabilityRoot => (false, false),
        }
    }
}

/// Terminal-settlement frame width one fractional compaction wraps verbatim.
///
/// Named here and never re-enumerated. The terminal frame's roles, order and
/// privileges belong to [`crate::terminal_settlement_v3`], which is the crate
/// that decodes the header this route carries; a second enumeration of them
/// here would be a second author for one frame, and the design's rule about the
/// payout derivation -- *call it, never re-implement it* -- is the same rule one
/// level up. The native compaction wraps the same frame the same way
/// (`claim_check_compaction_v1::TERMINAL_FRAME_V1`).
pub const FRACTIONAL_COMPACT_TERMINAL_FRAME_V1: usize =
    crate::terminal_settlement_v3::TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3;

/// Accounts one fractional compaction adds past the terminal frame.
///
/// **Fourteen, and the number moved three times -- twice for opposite reasons,
/// and once because a check the route could not run turned out to be cheap.**
///
/// *Up, from twelve to fourteen: what authentication costs.* The first
/// declaration gave `ExposureTerms` and `TokenBehavior` one account each. Both
/// are *finalized Registry records*, and this tree authenticates a finalized
/// record by a raw/staging **pair** -- `authenticate_finalized_rational_record`
/// derives both PDAs, requires the raw one to hash to the expected digest, and
/// requires the staging cursor to be vacant, which is what proves the record is
/// not mid-update. Every sibling route carries the pair:
/// `fractional_retirement_v3` does it three times over, in its begin,
/// coordinate and finish frames, and the terminal frame this one wraps carries
/// its own exposure raw/staging at indices 21 and 22. A frame holding only the
/// raw halves cannot run that check at all -- it could derive the raw address
/// and compare a digest, silently dropping the "not mid-update" proof on a
/// route that resolves an entire coordinate's outstanding collateral. That is
/// the shape of weakening that is invisible afterwards, because the route still
/// looks like it authenticates its terms. So the frame grew by two rather than
/// the check shrinking by one.
///
/// *Down, from fourteen to thirteen: what ceremony cost.* Design §17.8 ruling 2
/// dropped [`FractionalCompactionRoleV1::TradingCallerAuthority`]. It was
/// declared a required signer for a close that turns out to be owner-signed
/// without it: the reserve Position's owner **is** the capability root, the root
/// signs this frame for the burn hand-off, and a caller-authority PDA proves
/// only that Trading processed this request -- which the root's own signature
/// already proves, strictly more strongly, because Trading marks the root a
/// signer only after `fractional_root_signer` authenticates the root's bytes
/// against that same request. Two Trading signers where one carries the proof is
/// O-016's exact shape of ceremony: inclusion mistaken for authority. The role
/// stays *declared*, refused with its own reason, so the account cannot come
/// back by being forgotten about.
///
/// *Up again, from thirteen to fourteen: what an unrunnable check cost.* The
/// route decodes a real `LifecycleRentCreditV2`
/// and binds it to this market by its content -- market, release set,
/// generation -- which refuses a caller naming their own wallet or another
/// market's credit. What content alone cannot prove is that the account is the
/// **derived** RentCredit: `create_program_address` over the record's own
/// persisted seeds needs the Rent program's id to derive *under*, and a frame
/// with no Rent program account has no id to offer that is not the caller's own
/// word. This is the same lesson the raw/staging pair above taught one
/// paragraph earlier, arriving at a different account: a record is not
/// authenticated by its content alone.
///
/// So [`FractionalCompactionRoleV1::RentProgram`] joins the frame, read-only,
/// and `authenticate_rent_credit` runs -- the one this crate's siblings all run
/// and this route alone could not. It is the *eighth* account the Fractional
/// family adds over the native crank, and the native crank is the reason it can
/// be added at no cost to anyone: that route checks the RentCredit's writability
/// and nothing else, so nothing downstream was relying on this frame being the
/// narrower of the two.
pub const FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1: usize = 14;

/// Exact fractional compaction frame width.
pub const FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1: usize =
    FRACTIONAL_COMPACT_TERMINAL_FRAME_V1 + FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1;

/// Largest account count one transaction may lock without an address table.
///
/// Restated rather than imported because this crate is `no_std` and holds no
/// SDK: the number is the runtime's, and a frame at or over it is not a tight
/// frame but an unusable one.
pub const MAX_TRANSACTION_LOCKS_V1: usize = 64;

// A frame that cannot be locked is not a frame. This route already serialises
// through the address lookup table the fractional campaigns use, and the
// assertion is here so that a further account has to argue with the limit at
// compile time rather than at a validator.
const _: () = assert!(
    FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 < MAX_TRANSACTION_LOCKS_V1,
    "the fractional compaction frame must fit inside one transaction's locks"
);

/// Why one role is or is not in the fractional compaction frame.
///
/// Every declared role answers this, exhaustively, so that a further account
/// has to state its reason rather than inherit the arm it was written beside --
/// C0's lesson (compaction design §15.5), which
/// [`crate::fractional_claim_check_v1`]'s route table and FRACR3's owner-kind
/// weld both already apply. The refusals are the point: an absent account with
/// no recorded reason is indistinguishable from one nobody thought of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalCompactionAdmissionV1 {
    /// In the frame, at the stated index.
    Admitted,
    /// Refused: it names one shard holder, and compaction names none.
    ///
    /// The premise that lets a single transaction stand in for every holder at
    /// once (design §1.3: positions are never enumerated on chain). A frame
    /// carrying a holder's account would make this route per-holder, unbounded
    /// in the number of transactions a coordinate needs, and would burn one
    /// holder's shards at a moment they did not choose.
    RefusedNamesOneHolder,
    /// Refused: the record it names answers to a payee who cannot sign.
    ///
    /// FRACR3's weld, from the other side. The reserve Position's owner kind is
    /// `TradingRecord`, `owner_kind_can_open_a_claim_check` refuses it, and
    /// that refusal is correct -- a native claim-check minted here would be
    /// collateral written to a PDA with no private key. This whole route exists
    /// because the refusal stands, so reaching for the native record inside it
    /// would be undoing the reason it was written.
    RefusedUnsignablePayee,
    /// Refused: it belongs to the ordered-retirement walk, not to compaction.
    ///
    /// Compaction is permissionless and unordered by design; retirement's
    /// cursor is ordered and stateful. A frame carrying the cursor would let a
    /// stalled or absent walk block a crank whose whole purpose is to run when
    /// nobody is minding the market.
    RefusedNotThisRoute,
    /// Refused: a deadline-entitled crank takes no parent's authority.
    ///
    /// Design §17.8 ruling 2, and the reason is that this signer would refuse
    /// nothing. The crank is *anybody* -- under
    /// `(CallerRole::Claims, ParentAuthorityV3::ClaimCheckCrank)` coordinate 0
    /// is asked only that somebody signed, which is the deliberate relaxation
    /// that makes the crank permissionless. A caller-authority PDA derived from
    /// the caller's own request digest proves that Trading processed this exact
    /// request; the capability root's signature proves the same thing and more,
    /// because Trading marks the root a signer only after
    /// `fractional_root_signer` authenticates the root's bytes against that
    /// request. A stranger arriving without Trading dies at the `SetAuthority`
    /// hand-off with `MissingRequiredSignature`; a cranker arriving through
    /// Trading is legitimate by design. Neither outcome moves if a second
    /// Trading signer stands beside the first.
    ///
    /// And the close it was declared for does not need it. The reserve
    /// Position's owner **is** [`FractionalCompactionRoleV1::FractionalCapabilityRoot`],
    /// which signs this frame -- so the compaction close is owner-signed,
    /// deadline-entitled and record-authenticated, strictly stronger than the
    /// native sibling's `close_and_split`, which authenticates nothing inside
    /// itself because entitlement was proved before it is called.
    /// `execute_parent_authenticated_close` and `authenticate_parent_authority`
    /// are untouched and remain retirement's own, where the caller-PDA-plus-root
    /// pair is real because the close is *ordered by Trading's retirement walk*
    /// rather than entitled by an elapsed deadline.
    RefusedTakesNoParentAuthority,
}

/// One account role the fractional compaction frame has an opinion about.
///
/// The enumeration covers **only the accounts compaction adds**; indices below
/// [`FRACTIONAL_COMPACT_TERMINAL_FRAME_V1`] are the terminal frame's, and are
/// its to name. [`Self::index`] states where each one sits, so the constants a
/// route indexes by have one author here rather than one per program.
///
/// # The eight the Fractional family adds over the native crank
///
/// The native compaction needs six accounts past the terminal frame: the
/// escrow, the record it mints, the admission it reads the owner kind off, the
/// RentCredit, the opener it repays, and the System program. Every one of those
/// is here too, for the same reason. **Seven** of the other eight are what
/// §17.4's hand-off costs; the eighth,
/// [`Self::RentProgram`], is not a hand-off cost at all but the price of
/// authenticating a record this route was previously only able to read:
///
/// - [`Self::FractionalCapabilityRoot`], alone, because it is what
///   "Trading-composed" means for this route: composed **for signature, not for
///   authority** (design §17.8). The root's signature exists only inside
///   Trading, and the route needs it for exactly one thing no other key can do.
///   What *authorizes* the crank is the elapsed deadline and the records -- the
///   same authorizer as the native sibling, which requires nothing from Trading
///   at all. [`Self::TradingCallerAuthority`] was declared here beside the root
///   and is now refused; see its own arm.
/// - [`Self::ShardMint`] and [`Self::ShardTokenProgram`], because the burn
///   authority hand-off is a `SetAuthority` against the Mint, and the shard
///   Mint's Token program is not necessarily the collateral's.
/// - [`Self::ExposureTerms`], because the denominator has exactly one author
///   and the record persists it forever.
/// - [`Self::TokenBehavior`], because the profile the split-controller reader
///   runs is terms-selected, and reading it from anywhere else would be a
///   second author for the same fact.
/// - [`Self::ExposureTermsStaging`] and [`Self::TokenBehaviorStaging`], because
///   the two records above are *finalized Registry records* and this tree
///   authenticates one by its raw/staging pair -- the raw half must hash to the
///   expected digest and the staging cursor must be vacant. Without the cursors
///   the route could compare a digest but could not prove the record is settled,
///   which on the terms account means reading a denominator mid-rewrite.
/// - [`Self::RentProgram`] -- the eighth, and the odd one out. Not the
///   hand-off's cost but the RentCredit's: without a Rent program id to derive
///   under, the residual beneficiary is checked by its content alone, and
///   content is what a caller supplies. It is the raw/staging lesson arriving at
///   a second account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalCompactionRoleV1 {
    /// The per-market escrow: outstanding-count bookkeeper and future approver.
    Escrow,
    /// The fractional claim-check this crank mints, addressed by the Mint.
    FractionalClaimCheckRecord,
    /// The reserve Position's admission, carrying the persisted owner kind.
    ReserveAdmission,
    /// The market's RentCredit, residual beneficiary of the sweep.
    RentCredit,
    /// The escrow's opener, repaid from the sweep after the crank is paid.
    Opener,
    /// System program, for the record's allocation.
    SystemProgram,
    /// The Trading-derived Fractional capability root, signing for itself.
    ///
    /// **One job, and its once is the hand-off** (design §17.8 ruling 1). The
    /// root is the shard Mint's `PermissionedBurn` authority, and
    /// `SetAuthority(PermissionedBurn)`, root -> escrow, is refused by
    /// Token-2022 without the *current* authority's signature. Nothing but a
    /// Trading `invoke_signed` can produce that signature and nothing can
    /// produce it after the market retires, which is the entire reason the
    /// hand-off happens at compaction and not later -- and the reason §17.4
    /// made this route Trading-composed at all. After the hand-off the root
    /// signs nothing ever again: redemption is holder plus escrow.
    ///
    /// The root is *also* the reserve Position's owner, which is what entitles
    /// this route's close -- but that is the owner's own signature doing an
    /// owner's work, not a second job needing a second signer. §17.7 read it as
    /// two jobs and declared [`Self::TradingCallerAuthority`] beside it; §17.8
    /// ruling 2 refused that role for exactly this reason.
    FractionalCapabilityRoot,
    /// The Trading caller-authority PDA: declared, and refused.
    ///
    /// It has no program to derive it against and nothing to refuse.
    /// `CallerAuthoritySeedsV1` must be derived under the **Trading program
    /// id**, and no Trading program account is in this frame: the terminal
    /// frame's caller program is read only on the `CallerRole::Trading` path,
    /// and a fractional compaction request pins `CallerRole::Claims`. So the
    /// frame would have carried a signer nothing could check. See
    /// [`FractionalCompactionAdmissionV1::RefusedTakesNoParentAuthority`] for
    /// why adding the program account to check it was refused too.
    TradingCallerAuthority,
    /// The shard Mint: `SetAuthority` target, and the supply the record pins.
    ShardMint,
    /// The Token program owning the shard Mint.
    ///
    /// Separate from the terminal frame's Token program on purpose: that one is
    /// the *collateral* mint's, and a market may hold collateral under a
    /// different Token program than the one its shards are minted by. Folding
    /// the two would work until the first market where they differ.
    ShardTokenProgram,
    /// The finalized exposure terms: the denominator's sole author.
    ExposureTerms,
    /// The exposure terms' staging cursor, proving the record is not mid-update.
    ///
    /// Not decoration, and not symmetry for its own sake. A finalized Registry
    /// record is authenticated by the raw/staging pair together: the raw half
    /// carries the bytes and must hash to the expected digest, and the staging
    /// half must be **vacant**, which is what proves nobody is part-way through
    /// replacing those bytes. Carrying only the raw half would leave a route
    /// that reads a denominator from a record mid-rewrite, and the denominator
    /// is the sole divisor every holder's payout is computed with.
    ExposureTermsStaging,
    /// The terms-selected TokenBehavior record the shard profile is read under.
    TokenBehavior,
    /// The TokenBehavior record's staging cursor, for the same reason.
    TokenBehaviorStaging,
    /// The executable Rent program the RentCredit is derived and owned under.
    ///
    /// **The account that turns a content check into an authentication.** Every
    /// other conjunct on the RentCredit reads the record's own bytes -- market,
    /// release set, generation -- and bytes are what a caller supplies. The
    /// missing proof is that the account presented is the *derived* credit:
    /// `create_program_address` over the record's own persisted seeds, under a
    /// program id, must reproduce the account's own address. That needs a
    /// program id the route did not take from whoever built the transaction.
    ///
    /// It gets one from a place the caller cannot reach: the reserve Position's
    /// **admission**, which Claims wrote at admission time and which persists
    /// both the RentCredit and its Rent program
    /// (`ProtocolPositionAdmissionV2::rent_credit`/`rent_program`). So this
    /// frame pins *both* halves of the identity from one Claims-authored record.
    /// That is strictly stronger than `fractional_retirement_v3`'s finish, where
    /// the address is fixed by the cursor and the program account then names
    /// itself -- safe there, because a substituted program would have to already
    /// own the one address the root chose, but one conjunct short of this.
    ///
    /// Read-only and never a signer: a program account this route only derives
    /// against needs no more, and a write lock it does not need is one some
    /// other transaction could have used.
    ///
    /// Last in the frame rather than beside its credit; [`Self::frame`] says why.
    RentProgram,
    /// A shard holder's own shard token account: declared, and refused.
    HolderShardTokens,
    /// A shard holder's own collateral token account: declared, and refused.
    HolderCollateralTokens,
    /// The native claim-check record for the reserve's owner: declared, refused.
    NativeClaimCheckRecord,
    /// The ordered-retirement cursor: declared, and refused.
    RetirementCursor,
}

impl FractionalCompactionRoleV1 {
    /// The exact ordered frame past the wrapped terminal accounts.
    #[must_use]
    pub const fn frame() -> [Self; FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1] {
        [
            Self::Escrow,
            Self::FractionalClaimCheckRecord,
            Self::ReserveAdmission,
            Self::RentCredit,
            Self::Opener,
            Self::SystemProgram,
            Self::FractionalCapabilityRoot,
            Self::ShardMint,
            Self::ShardTokenProgram,
            Self::ExposureTerms,
            Self::ExposureTermsStaging,
            Self::TokenBehavior,
            Self::TokenBehaviorStaging,
            // LAST, and not beside the credit it authenticates, which is where a
            // reader would put it. The first six of this frame are the native
            // crank's own six in the native crank's own order, so the two
            // routes' tails can be read side by side -- a property this module
            // asserts and one the whole thread depends on, since the payout, the
            // sweep and the conservation all have a single author across both
            // routes. The Rent program is an account the native crank does not
            // have, so it belongs with the Fractional additions and nowhere
            // else. Adjacency with the credit would have been prettier and would
            // have cost the parity.
            Self::RentProgram,
        ]
    }

    /// Every role this route has an opinion about, admitted or refused.
    #[must_use]
    pub const fn declared() -> [Self; FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1 + 5] {
        [
            Self::Escrow,
            Self::FractionalClaimCheckRecord,
            Self::ReserveAdmission,
            Self::RentCredit,
            Self::Opener,
            Self::SystemProgram,
            Self::FractionalCapabilityRoot,
            Self::TradingCallerAuthority,
            Self::ShardMint,
            Self::ShardTokenProgram,
            Self::ExposureTerms,
            Self::ExposureTermsStaging,
            Self::TokenBehavior,
            Self::TokenBehaviorStaging,
            Self::RentProgram,
            Self::HolderShardTokens,
            Self::HolderCollateralTokens,
            Self::NativeClaimCheckRecord,
            Self::RetirementCursor,
        ]
    }

    /// Whether this role is in the frame, and if not, why not.
    #[must_use]
    pub const fn admission(self) -> FractionalCompactionAdmissionV1 {
        match self {
            Self::Escrow
            | Self::FractionalClaimCheckRecord
            | Self::ReserveAdmission
            | Self::RentCredit
            | Self::RentProgram
            | Self::Opener
            | Self::SystemProgram
            | Self::FractionalCapabilityRoot
            | Self::ShardMint
            | Self::ShardTokenProgram
            | Self::ExposureTerms
            | Self::ExposureTermsStaging
            | Self::TokenBehavior
            | Self::TokenBehaviorStaging => FractionalCompactionAdmissionV1::Admitted,
            Self::HolderShardTokens | Self::HolderCollateralTokens => {
                FractionalCompactionAdmissionV1::RefusedNamesOneHolder
            }
            Self::NativeClaimCheckRecord => FractionalCompactionAdmissionV1::RefusedUnsignablePayee,
            Self::RetirementCursor => FractionalCompactionAdmissionV1::RefusedNotThisRoute,
            Self::TradingCallerAuthority => {
                FractionalCompactionAdmissionV1::RefusedTakesNoParentAuthority
            }
        }
    }

    /// Absolute index of an admitted role in the whole compaction frame.
    ///
    /// `None` for a refused role, which is what makes an accidental index
    /// impossible to spell: there is no number to write down for an account the
    /// route must never reach for.
    #[must_use]
    pub fn index(self) -> Option<usize> {
        let mut position = 0;
        while position < FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1 {
            // `get` rather than an index: this crate denies `indexing_slicing`,
            // and a frame lookup is exactly where a silent panic would hide.
            if Self::frame().get(position) == Some(&self) {
                return FRACTIONAL_COMPACT_TERMINAL_FRAME_V1.checked_add(position);
            }
            position = position.checked_add(1)?;
        }
        None
    }

    /// Exact privileges an admitted role carries in the frame.
    ///
    /// `(signer, writable)`, and there is exactly **one** signer: the
    /// capability root, which is what "Trading-composed" means for this route
    /// (design §17.8 -- composed for signature, not for authority). Nothing a
    /// human holds signs a fractional compaction; the one signature is
    /// program-derived, which is what keeps the crank permissionless in the
    /// sense that matters -- anybody may *send* it, and nobody may direct where
    /// it pays.
    ///
    /// The root is a signer and **not writable**: a compaction revises nothing
    /// about the root. That is the inversion `fractional_root_signer`'s
    /// compaction arm enforces, against the exposure arms next to it, which
    /// require a writable root because their effect program commits a revision.
    #[must_use]
    pub const fn privileges(self) -> (bool, bool) {
        match self {
            Self::FractionalCapabilityRoot => (true, false),
            Self::Escrow
            | Self::FractionalClaimCheckRecord
            | Self::ReserveAdmission
            | Self::RentCredit
            | Self::Opener
            | Self::ShardMint => (false, true),
            // Every one of these is read and never written, and the two staging
            // cursors are read precisely to confirm they hold nothing. The Rent
            // program is here rather than beside its credit: the credit receives
            // the residue and is written, the program is only derived against.
            Self::SystemProgram
            | Self::RentProgram
            | Self::ShardTokenProgram
            | Self::ExposureTerms
            | Self::ExposureTermsStaging
            | Self::TokenBehavior
            | Self::TokenBehaviorStaging => (false, false),
            // Not in the frame. Stated so an edit that admits one has to delete
            // its arm here rather than merely append an account.
            Self::HolderShardTokens
            | Self::HolderCollateralTokens
            | Self::NativeClaimCheckRecord
            | Self::RetirementCursor
            | Self::TradingCallerAuthority => (false, false),
        }
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

fn exact_width(input: &[u8], width: usize) -> ClaimCheckResultV1<()> {
    if input.len() == width {
        Ok(())
    } else {
        Err(ClaimCheckErrorV1::InvalidLength)
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
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> ClaimCheckResultV1<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> ClaimCheckResultV1<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> ClaimCheckResultV1<[u8; N]> {
    let end = offset
        .checked_add(N)
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
    use crate::claim_check_v1::{
        CLAIM_CHECK_BYTES_V1, CLAIM_CHECK_ESCROW_MAGIC_V1, CLAIM_CHECK_RECORD_KIND_V1,
        CLAIM_CHECK_RECORD_MAGIC_V1, CLAIM_CHECK_SEED_V1, ClaimCheckV1,
    };

    const DENOMINATOR: u64 = 1_000;
    const PAYOUT_PER_CLAIM: u64 = 7_500;

    fn record() -> FractionalClaimCheckV1 {
        FractionalClaimCheckV1 {
            aggregate: [1; 32],
            shard_mint: [2; 32],
            market: [3; 32],
            release_set: [4; 32],
            vault: [5; 32],
            collateral_mint: [6; 32],
            position_atoms_digest: [7; 32],
            // 12_345 shards outstanding form 12 whole claims.
            escrowed_atoms: 12 * PAYOUT_PER_CLAIM,
            denominator: DENOMINATOR,
            payout_per_claim: PAYOUT_PER_CLAIM,
            compacted_shard_supply: 12_345,
            compacted_slot: 38_892_000,
            generation: 9,
            representation_coordinate: 3,
            bump: 251,
        }
        .new()
        .expect("record")
    }

    #[test]
    fn record_round_trips_at_its_one_exact_width() {
        let value = record();
        let bytes = value.to_bytes().expect("bytes");
        assert_eq!(bytes.len(), FRACTIONAL_CLAIM_CHECK_BYTES_V1);
        assert_eq!(FractionalClaimCheckV1::decode(&bytes), Ok(value));
    }

    #[test]
    fn the_two_record_types_can_never_be_decoded_as_each_other() {
        // The whole argument for a second record type rather than a second use
        // of the reserved run: one width meaning two field layouts is a union
        // whose arms diverge after the kind byte. Here neither width nor kind
        // nor magic agrees, so a hostile decode fails at a named byte.
        assert_ne!(FRACTIONAL_CLAIM_CHECK_BYTES_V1, CLAIM_CHECK_BYTES_V1);
        assert_ne!(
            FRACTIONAL_CLAIM_CHECK_RECORD_MAGIC_V1,
            CLAIM_CHECK_RECORD_MAGIC_V1
        );
        assert_ne!(
            FRACTIONAL_CLAIM_CHECK_RECORD_KIND_V1,
            CLAIM_CHECK_RECORD_KIND_V1
        );
        assert_ne!(FRACTIONAL_CLAIM_CHECK_SEED_V1, CLAIM_CHECK_SEED_V1);

        let fractional = record().to_bytes().expect("bytes");
        assert_eq!(
            ClaimCheckV1::decode(&fractional),
            Err(ClaimCheckErrorV1::InvalidLength)
        );

        // And the kind byte alone stops a same-width forgery: a record whose
        // every other byte is canonical still refuses if it claims to be the
        // native kind, or the escrow kind, or anything unallocated.
        for kind in [0_u8, CLAIM_CHECK_RECORD_KIND_V1, 2, 4, 255] {
            let mut bytes = record().to_bytes().expect("bytes");
            write(&mut bytes, RECORD_KIND_OFFSET, &[kind]).expect("kind");
            assert_eq!(
                FractionalClaimCheckV1::decode(&bytes),
                Err(ClaimCheckErrorV1::UnknownTag)
            );
        }
    }

    #[test]
    fn record_refuses_a_truncated_or_extended_input() {
        let bytes = record().to_bytes().expect("bytes");
        assert_eq!(
            FractionalClaimCheckV1::decode(
                bytes
                    .get(..FRACTIONAL_CLAIM_CHECK_BYTES_V1 - 1)
                    .expect("short")
            ),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        let mut long = [0_u8; FRACTIONAL_CLAIM_CHECK_BYTES_V1 + 1];
        long.get_mut(..FRACTIONAL_CLAIM_CHECK_BYTES_V1)
            .expect("prefix")
            .copy_from_slice(&bytes);
        assert_eq!(
            FractionalClaimCheckV1::decode(&long),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        assert_eq!(
            FractionalClaimCheckV1::decode(&[]),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
    }

    #[test]
    fn record_refuses_another_wire_family() {
        let mut bytes = record().to_bytes().expect("bytes");
        write(&mut bytes, 0, &CLAIM_CHECK_ESCROW_MAGIC_V1).expect("swap magic");
        assert_eq!(
            FractionalClaimCheckV1::decode(&bytes),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );

        let mut versioned = record().to_bytes().expect("bytes");
        write(&mut versioned, RECORD_VERSION_OFFSET, &2_u16.to_le_bytes()).expect("version");
        assert_eq!(
            FractionalClaimCheckV1::decode(&versioned),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );
    }

    #[test]
    fn record_refuses_every_nonzero_reserved_byte() {
        for offset in RECORD_RESERVED_HEADER_OFFSET
            ..RECORD_RESERVED_HEADER_OFFSET.saturating_add(RESERVED_HEADER_BYTES)
        {
            let mut bytes = record().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[0xFF]).expect("dirty reserved");
            assert_eq!(
                FractionalClaimCheckV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
        // Every reserved body byte, not merely the first: the reserved run is
        // where a later version's fields will live, and a decoder that only
        // checked the first byte would admit a partially-populated successor.
        for offset in RECORD_RESERVED_BODY_OFFSET..FRACTIONAL_CLAIM_CHECK_BYTES_V1 {
            let mut bytes = record().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[0xFF]).expect("dirty reserved");
            assert_eq!(
                FractionalClaimCheckV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
    }

    #[test]
    fn record_refuses_a_zero_or_aliased_identity() {
        for mutate in [
            |value: &mut FractionalClaimCheckV1| value.aggregate = [0; 32],
            |value: &mut FractionalClaimCheckV1| value.shard_mint = [0; 32],
            |value: &mut FractionalClaimCheckV1| value.market = [0; 32],
            |value: &mut FractionalClaimCheckV1| value.release_set = [0; 32],
            |value: &mut FractionalClaimCheckV1| value.vault = [0; 32],
            |value: &mut FractionalClaimCheckV1| value.collateral_mint = [0; 32],
        ] {
            let mut value = record();
            mutate(&mut value);
            assert_eq!(value.new(), Err(ClaimCheckErrorV1::InvalidIdentity));
            assert_eq!(value.to_bytes(), Err(ClaimCheckErrorV1::InvalidIdentity));
        }

        // The shard Mint is the claimant and the collateral Mint is what the
        // payout is denominated in. A record aliasing them would pay a holder
        // in the instrument they just burned.
        let mut aliased_mints = record();
        aliased_mints.collateral_mint = aliased_mints.shard_mint;
        assert_eq!(aliased_mints.new(), Err(ClaimCheckErrorV1::InvalidIdentity));

        let mut aliased_seed = record();
        aliased_seed.shard_mint = aliased_seed.aggregate;
        assert_eq!(aliased_seed.new(), Err(ClaimCheckErrorV1::InvalidIdentity));

        let mut zero_digest = record();
        zero_digest.position_atoms_digest = [0; 32];
        assert_eq!(zero_digest.new(), Err(ClaimCheckErrorV1::InvalidIdentity));
    }

    #[test]
    fn seeds_prove_the_instrument_the_way_the_native_seeds_prove_the_holder() {
        // The correction, as arithmetic. There is no claimant field to forge:
        // the claimant is a seed, so a caller naming the wrong Mint derives an
        // address that is not the account they passed.
        let value = record();
        let seeds = value.seeds().expect("seeds");
        assert_eq!(seeds.aggregate(), value.aggregate);
        assert_eq!(seeds.shard_mint(), value.shard_mint);
        assert_eq!(
            seeds.as_slices(),
            [FRACTIONAL_CLAIM_CHECK_SEED_V1, &[1; 32], &[2; 32]]
        );

        assert_eq!(
            FractionalClaimCheckSeedsV1::new([0; 32], [2; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        assert_eq!(
            FractionalClaimCheckSeedsV1::new([1; 32], [0; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        assert_eq!(
            FractionalClaimCheckSeedsV1::new([1; 32], [1; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
    }

    #[test]
    fn a_record_promising_nothing_is_refused_across_the_wire_too() {
        for mutate in [
            |value: &mut FractionalClaimCheckV1| value.escrowed_atoms = 0,
            |value: &mut FractionalClaimCheckV1| value.payout_per_claim = 0,
            |value: &mut FractionalClaimCheckV1| value.generation = 0,
            // A denominator of one is not a fractionalization; the exposure
            // terms refuse it and so must anything claiming to descend from
            // them.
            |value: &mut FractionalClaimCheckV1| value.denominator = 1,
            |value: &mut FractionalClaimCheckV1| value.denominator = 0,
            |value: &mut FractionalClaimCheckV1| {
                value.representation_coordinate = FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1;
            },
        ] {
            let mut value = record();
            mutate(&mut value);
            assert_eq!(value.new(), Err(ClaimCheckErrorV1::InvalidEntitlement));
        }

        // Bytes are what an adversary controls, so the refusal must survive the
        // wire and not merely the constructor.
        for (offset, forged) in [
            (RECORD_ESCROWED_OFFSET, 0_u64),
            (RECORD_PAYOUT_PER_CLAIM_OFFSET, 0),
            (RECORD_DENOMINATOR_OFFSET, 1),
            (RECORD_GENERATION_OFFSET, 0),
        ] {
            let mut bytes = record().to_bytes().expect("bytes");
            write(&mut bytes, offset, &forged.to_le_bytes()).expect("forge");
            assert_eq!(
                FractionalClaimCheckV1::decode(&bytes),
                Err(ClaimCheckErrorV1::InvalidEntitlement)
            );
        }
    }

    #[test]
    fn the_only_division_floors_and_sub_denominator_dust_claims_nothing() {
        // Rounding pinned per leg. The quotient floors, and it is the only
        // division in the module; both multiplications are exact.
        let value = record();
        for (shards, claims) in [
            (0_u64, 0_u64),
            (1, 0),
            (DENOMINATOR - 1, 0),
            (DENOMINATOR, 1),
            (DENOMINATOR + 1, 1),
            (2 * DENOMINATOR - 1, 1),
            (12_345, 12),
            (u64::MAX, u64::MAX / DENOMINATOR),
        ] {
            assert_eq!(value.whole_claims_for(shards), claims);
            // Never rounds up, at any input.
            assert!(
                value.consumed_shards(claims).expect("consumed") <= shards,
                "{shards} shards must never consume more than the holder holds"
            );
        }

        // The dust that forms no claim is left with the holder, to the atom.
        for shards in [1_u64, 999, 1_001, 12_345] {
            let claims = value.whole_claims_for(shards);
            let consumed = value.consumed_shards(claims).expect("consumed");
            let change = shards - consumed;
            assert!(change < DENOMINATOR);
            assert_eq!(consumed + change, shards);
        }
    }

    #[test]
    fn a_burn_is_paid_exactly_what_on_time_redemption_would_have_paid() {
        // The claim the whole record exists to make, as arithmetic over the two
        // persisted numbers. `divide_exposure_shards_v2` plus the terminal
        // evaluator's per-coordinate constant is
        //   collateral = (shard_atoms / denominator) * payout_per_claim
        // and this record evaluates the same expression from its own fields.
        let value = record();
        for shards in [DENOMINATOR, 3 * DENOMINATOR, 12_345, 12_000] {
            let claims = value.whole_claims_for(shards);
            assert_eq!(
                value.claim_payout(claims),
                Ok(claims * PAYOUT_PER_CLAIM),
                "the payout is a multiplication, never a second rounding"
            );
        }
        // Overflow is a refusal, not a wrap.
        let mut huge = record();
        huge.payout_per_claim = u64::MAX;
        assert_eq!(huge.claim_payout(2), Err(ClaimCheckErrorV1::Arithmetic));
        assert_eq!(
            huge.consumed_shards(u64::MAX),
            Err(ClaimCheckErrorV1::Arithmetic)
        );
    }

    #[test]
    fn the_escrowed_balance_pays_down_to_the_atom_and_settles_exactly_at_zero() {
        // Conservation across the FULL pay-down, not merely per burn: the sum
        // of every payout equals the opening balance, and the record settles on
        // the burn that exhausts it rather than one either side.
        let opening = record();
        let mut live = opening;
        let mut paid = 0_u64;
        // Twelve whole claims, taken as 5 + 4 + 2 + 1.
        for claims in [5_u64, 4, 2] {
            let payout = live.claim_payout(claims).expect("payout");
            paid += payout;
            live = live
                .pay_down(payout)
                .expect("pay down")
                .remaining()
                .expect("settled before the balance was exhausted");
            assert_eq!(live.escrowed_atoms, opening.escrowed_atoms - paid);
            // Everything except the balance is immutable across a pay-down.
            assert_eq!(
                FractionalClaimCheckV1 {
                    escrowed_atoms: opening.escrowed_atoms,
                    ..live
                },
                opening
            );
        }
        let last = live.claim_payout(1).expect("payout");
        paid += last;
        assert_eq!(live.pay_down(last), Ok(FractionalPayDownV1::Settled));
        assert_eq!(paid, opening.escrowed_atoms);
        assert_eq!(paid, 12 * PAYOUT_PER_CLAIM);
    }

    #[test]
    fn a_pay_down_that_would_overdraw_another_holder_is_refused() {
        // The escrowed balance is what every other holder's claim is paid out
        // of. Overpaying one of them is theft from the rest, so it refuses
        // rather than saturating.
        let value = record();
        assert_eq!(
            value.pay_down(value.escrowed_atoms + 1),
            Err(ClaimCheckErrorV1::Arithmetic)
        );
        assert_eq!(value.pay_down(u64::MAX), Err(ClaimCheckErrorV1::Arithmetic));
        // Exactly the balance settles; one atom less leaves a live record.
        assert_eq!(
            value.pay_down(value.escrowed_atoms),
            Ok(FractionalPayDownV1::Settled)
        );
        assert!(
            value
                .pay_down(value.escrowed_atoms - 1)
                .expect("one atom short of settling")
                .remaining()
                .is_some()
        );
        assert!(
            value
                .pay_down(value.escrowed_atoms)
                .expect("settling")
                .is_settled()
        );
        // A payout of nothing is not a redemption, and must not retire a shard
        // burn's worth of nothing against the record.
        assert_eq!(
            value.pay_down(0),
            Err(ClaimCheckErrorV1::InvalidEntitlement)
        );
    }

    #[test]
    fn the_opening_balance_is_checkable_from_the_record_alone() {
        // Once the market is gone this record is the only evidence left, so the
        // compaction's own claim -- that it escrowed every whole claim the
        // outstanding supply could form -- has to be checkable from it.
        let value = record();
        assert!(value.opening_escrow_is_consistent(value.escrowed_atoms));
        assert!(!value.opening_escrow_is_consistent(value.escrowed_atoms + 1));
        assert!(!value.opening_escrow_is_consistent(value.escrowed_atoms - 1));

        // Sub-denominator dust across the supply is not escrowed, because it is
        // not a claim: 12_345 shards form 12 claims, not 12.345.
        assert_eq!(value.whole_claims_for(value.compacted_shard_supply), 12);
        assert_eq!(value.escrowed_atoms, 12 * PAYOUT_PER_CLAIM);

        // And a paid-down record fails the opening check against its CURRENT
        // balance, which is why the opening balance is an argument.
        let live = value
            .pay_down(PAYOUT_PER_CLAIM)
            .expect("pay down")
            .remaining()
            .expect("one claim cannot settle twelve");
        assert!(!live.opening_escrow_is_consistent(live.escrowed_atoms));
        assert!(live.opening_escrow_is_consistent(value.escrowed_atoms));
    }

    #[test]
    fn every_role_in_the_frame_outlives_the_market_and_every_refused_role_is_absent() {
        // The assertion is on the SPEC, not on an inspection of the route. A
        // later edit that reaches for an aggregate, a Core state, a basis or
        // graph record, a Hoard, a Custody replay cursor -- or the Fractional
        // capability root -- has to add a role here and answer
        // `survives_retirement` for it, and the honest answer fails this test.
        for role in FractionalClaimCheckRedemptionRoleV1::frame() {
            assert!(
                role.survives_retirement(),
                "{role:?} does not outlive the market and cannot be in this frame"
            );
        }
        for role in FractionalClaimCheckRedemptionRoleV1::declared() {
            assert_eq!(
                FractionalClaimCheckRedemptionRoleV1::frame().contains(&role),
                role.survives_retirement(),
                "{role:?} is in the frame exactly when it outlives the market"
            );
        }
    }

    #[test]
    fn the_shard_mints_burn_authority_is_never_the_trading_root_in_this_frame() {
        // The correction this module carries, stated where a future edit will
        // trip over it. A shard burn needs the Mint's PermissionedBurn authority
        // as a second signer -- a standard burn is refused outright while the
        // extension is present -- and at founding that authority is the
        // Fractional capability root, a Trading-derived PDA that Claims cannot
        // sign and that does not outlive the market. So the root is declared and
        // refused, and the escrow, which Claims CAN sign, is in the frame to be
        // the burn approver.
        assert!(
            !FractionalClaimCheckRedemptionRoleV1::FractionalCapabilityRoot.survives_retirement()
        );
        assert!(
            !FractionalClaimCheckRedemptionRoleV1::frame()
                .contains(&FractionalClaimCheckRedemptionRoleV1::FractionalCapabilityRoot)
        );
        assert!(
            FractionalClaimCheckRedemptionRoleV1::frame()
                .contains(&FractionalClaimCheckRedemptionRoleV1::Escrow)
        );
        assert!(
            FractionalClaimCheckRedemptionRoleV1::frame()
                .contains(&FractionalClaimCheckRedemptionRoleV1::ShardMint)
        );
    }

    #[test]
    fn exactly_one_account_signs_and_it_is_the_holder_presenting_shards() {
        // GREEN-SELF, fractional edition: no third party stands between a shard
        // holder and their collateral, and there is no second signature for
        // anyone to be induced to provide. The burn's other required signer is
        // the escrow PDA, which the program signs for itself and which no human
        // holds.
        let signers: usize = FractionalClaimCheckRedemptionRoleV1::frame()
            .iter()
            .filter(|role| role.privileges().0)
            .count();
        assert_eq!(signers, 1);
        assert!(FractionalClaimCheckRedemptionRoleV1::Holder.privileges().0);
        assert!(FractionalClaimCheckRedemptionRoleV1::Holder.privileges().1);
        for role in [
            FractionalClaimCheckRedemptionRoleV1::CollateralMint,
            FractionalClaimCheckRedemptionRoleV1::TokenProgram,
        ] {
            assert_eq!(role.privileges(), (false, false));
        }
    }

    #[test]
    fn the_frame_is_pinned_exactly_and_stays_far_below_the_lock_limit() {
        assert_eq!(
            FractionalClaimCheckRedemptionRoleV1::frame(),
            [
                FractionalClaimCheckRedemptionRoleV1::Holder,
                FractionalClaimCheckRedemptionRoleV1::FractionalClaimCheckRecord,
                FractionalClaimCheckRedemptionRoleV1::Escrow,
                FractionalClaimCheckRedemptionRoleV1::Vault,
                FractionalClaimCheckRedemptionRoleV1::HolderCollateralTokens,
                FractionalClaimCheckRedemptionRoleV1::CollateralMint,
                FractionalClaimCheckRedemptionRoleV1::ShardMint,
                FractionalClaimCheckRedemptionRoleV1::HolderShardTokens,
                FractionalClaimCheckRedemptionRoleV1::TokenProgram,
            ]
        );
        assert_eq!(
            FractionalClaimCheckRedemptionRoleV1::frame().len(),
            FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1
        );
        // The residue claim, as a number. A fractional terminal settlement is
        // 44 accounts; a shard holder coming back needs nine, and none of the
        // market's.
        assert_eq!(FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1, 9);
        assert!(FRACTIONAL_CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1 < 44);
    }

    #[test]
    fn the_compaction_frame_wraps_the_terminal_frame_and_adds_exactly_fourteen() {
        // The shape §17.5 names, as arithmetic rather than a "~". The terminal
        // half is not re-enumerated here on purpose -- it has one author, and
        // this asserts that this module agrees with that author rather than
        // restating it.
        assert_eq!(
            FRACTIONAL_COMPACT_TERMINAL_FRAME_V1,
            crate::terminal_settlement_v3::TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3
        );
        assert_eq!(FRACTIONAL_COMPACT_TERMINAL_FRAME_V1, 36);
        // WITNESS w6 (design §17.8 ruling 2), carried forward to the ruled
        // fiftieth account (WAVE `b4546291`): the frame is 50, and the dropped
        // role STILL has no index. Pinned as literals, not derived from the enum
        // -- deriving 50 from `frame().len()` would agree with any frame at all.
        assert_eq!(FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1, 14);
        assert_eq!(FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1, 50);
        // The account the ruling added, pinned as a literal and placed LAST --
        // the fiftieth is the fiftieth. It is deliberately not beside the credit
        // it authenticates: the first six of this frame are the native crank's
        // own six in the native crank's own order, the Rent program is an
        // account the native crank does not have, and keeping that prefix intact
        // is worth more than adjacency. The assertion below on `SystemProgram`
        // is the other half of that pair, and an edit that moved this account
        // for tidiness would red it.
        assert_eq!(FractionalCompactionRoleV1::RentProgram.index(), Some(49));
        // READ-ONLY, and its credit is not. A program account this route only
        // derives against needs neither a signature nor a write lock; the credit
        // receives the residue and must be written. An edit granting the program
        // either privilege has to argue with these two lines.
        assert_eq!(
            FractionalCompactionRoleV1::RentProgram.privileges(),
            (false, false)
        );
        assert_eq!(
            FractionalCompactionRoleV1::RentCredit.privileges(),
            (false, true)
        );
        assert_eq!(
            FractionalCompactionRoleV1::TradingCallerAuthority.index(),
            None,
            "ruling 2 dropped the caller authority; an index for it is the frame \
             growing the account back"
        );
        assert_eq!(
            FractionalCompactionRoleV1::frame().len(),
            FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1
        );
        // Six of the fourteen are the native crank's own; seven of the other
        // eight are what the §17.4 hand-off costs, and the eighth is the Rent
        // program the ruled authentication derives under. The native frame is
        // 42; this one is 50, and both fit one transaction's locks without an
        // address table.
        assert_eq!(FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 - 8, 42);
        assert!(FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 < MAX_TRANSACTION_LOCKS_V1);
        // Both finalized Registry records are carried as raw/staging PAIRS, the
        // way every sibling route carries them. Stated as a property over the
        // frame rather than as a count, so a later edit that drops one staging
        // half to save an account has to argue with this line -- the failure it
        // would cause is silent, because a route missing a staging cursor still
        // authenticates its raw record and merely stops proving it is settled.
        for (record, staging) in [
            (
                FractionalCompactionRoleV1::ExposureTerms,
                FractionalCompactionRoleV1::ExposureTermsStaging,
            ),
            (
                FractionalCompactionRoleV1::TokenBehavior,
                FractionalCompactionRoleV1::TokenBehaviorStaging,
            ),
        ] {
            let raw = record.index().expect("a finalized record is in the frame");
            let cursor = staging.index().expect("its staging cursor is too");
            assert_eq!(
                cursor,
                raw + 1,
                "{record:?} and its staging cursor must be adjacent, as the pair every \
                 authenticate_finalized_rational_record caller passes"
            );
            assert_eq!(record.privileges(), (false, false));
            assert_eq!(staging.privileges(), (false, false));
        }
    }

    #[test]
    fn every_declared_role_is_either_indexed_or_refused_with_a_reason() {
        // The property that keeps the enum honest in both directions: an
        // admitted role has exactly one index inside the compaction half, and a
        // refused role has none at all -- so there is no number to write down
        // for an account this route must never reach for.
        let mut admitted = 0;
        for role in FractionalCompactionRoleV1::declared() {
            match role.admission() {
                FractionalCompactionAdmissionV1::Admitted => {
                    admitted += 1;
                    let index = role.index().expect("an admitted role sits somewhere");
                    assert!(
                        index >= FRACTIONAL_COMPACT_TERMINAL_FRAME_V1,
                        "{role:?} claims an index inside the terminal frame's half"
                    );
                    assert!(index < FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1);
                    assert!(FractionalCompactionRoleV1::frame().contains(&role));
                }
                FractionalCompactionAdmissionV1::RefusedNamesOneHolder
                | FractionalCompactionAdmissionV1::RefusedUnsignablePayee
                | FractionalCompactionAdmissionV1::RefusedNotThisRoute
                | FractionalCompactionAdmissionV1::RefusedTakesNoParentAuthority => {
                    assert_eq!(role.index(), None, "{role:?} is refused and yet indexed");
                    assert!(!FractionalCompactionRoleV1::frame().contains(&role));
                    assert_eq!(role.privileges(), (false, false));
                }
            }
        }
        assert_eq!(admitted, FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1);
        // Non-vacuous in the direction that matters: there ARE refusals, and
        // all three reasons are exercised rather than one standing in for the
        // set. A refusal nobody reaches proves nothing about the frame.
        for reason in [
            FractionalCompactionAdmissionV1::RefusedNamesOneHolder,
            FractionalCompactionAdmissionV1::RefusedUnsignablePayee,
            FractionalCompactionAdmissionV1::RefusedNotThisRoute,
            FractionalCompactionAdmissionV1::RefusedTakesNoParentAuthority,
        ] {
            assert!(
                FractionalCompactionRoleV1::declared()
                    .iter()
                    .any(|role| role.admission() == reason),
                "{reason:?} is declared and never reached"
            );
        }
    }

    #[test]
    fn the_indices_are_consecutive_from_the_terminal_frame_and_never_collide() {
        // The route indexes accounts by these numbers, so a duplicate or a gap
        // is a route reading the wrong account rather than a tidiness defect.
        let mut expected = FRACTIONAL_COMPACT_TERMINAL_FRAME_V1;
        for role in FractionalCompactionRoleV1::frame() {
            assert_eq!(role.index(), Some(expected), "{role:?} is out of order");
            expected += 1;
        }
        assert_eq!(expected, FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1);

        // AND THE ORDER ITSELF, SPELLED OUT. The loop above derives its
        // expectation from `frame()`, so it holds under any permutation --
        // proved by mutation: swapping two entries left it green. The order is
        // load-bearing (a caller builds the account vector from it and the
        // route reads by index), so a swap has to fail somewhere, and this is
        // the only place that can see one.
        assert_eq!(
            FractionalCompactionRoleV1::frame(),
            [
                FractionalCompactionRoleV1::Escrow,
                FractionalCompactionRoleV1::FractionalClaimCheckRecord,
                FractionalCompactionRoleV1::ReserveAdmission,
                FractionalCompactionRoleV1::RentCredit,
                FractionalCompactionRoleV1::Opener,
                FractionalCompactionRoleV1::SystemProgram,
                FractionalCompactionRoleV1::FractionalCapabilityRoot,
                FractionalCompactionRoleV1::ShardMint,
                FractionalCompactionRoleV1::ShardTokenProgram,
                FractionalCompactionRoleV1::ExposureTerms,
                FractionalCompactionRoleV1::ExposureTermsStaging,
                FractionalCompactionRoleV1::TokenBehavior,
                FractionalCompactionRoleV1::TokenBehaviorStaging,
                FractionalCompactionRoleV1::RentProgram,
            ]
        );
        // The first six are the native crank's own six, in the native crank's
        // own order, so the two routes' tails can be read side by side. This
        // pair is why the ruled fiftieth account went to the END of the frame
        // instead of beside the credit it authenticates: an insertion at the
        // readable place would have pushed `SystemProgram` off the native six
        // and quietly ended the parity, which is a property the whole thread
        // leans on -- the payout, the sweep and the conservation each have one
        // author across both routes.
        assert_eq!(
            FractionalCompactionRoleV1::Escrow.index(),
            Some(FRACTIONAL_COMPACT_TERMINAL_FRAME_V1)
        );
        assert_eq!(
            FractionalCompactionRoleV1::SystemProgram.index(),
            Some(FRACTIONAL_COMPACT_TERMINAL_FRAME_V1 + 5)
        );
    }

    #[test]
    fn compaction_names_no_holder_and_its_one_signer_is_program_derived() {
        // §1.3, as a frame property. One transaction stands in for every holder
        // of a coordinate, which is only true while it names none of them; a
        // frame that grew a holder account would make this route per-holder and
        // would burn one holder's shards at a moment they did not choose.
        for role in [
            FractionalCompactionRoleV1::HolderShardTokens,
            FractionalCompactionRoleV1::HolderCollateralTokens,
        ] {
            assert_eq!(
                role.admission(),
                FractionalCompactionAdmissionV1::RefusedNamesOneHolder
            );
        }
        // And the mirror of the redemption frame, which is the pairing worth
        // asserting: redemption names exactly one holder and no market account;
        // compaction names every market account and no holder.
        assert!(
            FractionalClaimCheckRedemptionRoleV1::frame()
                .contains(&FractionalClaimCheckRedemptionRoleV1::HolderShardTokens)
        );

        // WITNESS w5 (design §17.8 ruling 2): among the thirteen added roles
        // there is EXACTLY ONE signer, and it is the capability root. Written
        // as a scan of the whole frame rather than a check of the root alone,
        // because the property that ruling 2 bought is the *absence* of a
        // second Trading signer -- a test naming only the root would stay green
        // if the caller authority came back.
        let mut signers = [None; FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1];
        let mut found = 0;
        for role in FractionalCompactionRoleV1::frame() {
            if role.privileges().0 {
                *signers.get_mut(found).expect("a frame-sized signer table") = Some(role);
                found += 1;
            }
        }
        assert_eq!(
            found, 1,
            "a fractional compaction has one program-derived signature, not two: \
             the root's, which Trading adds only after authenticating the root \
             against this same request"
        );
        assert_eq!(
            signers.first().copied().flatten(),
            Some(FractionalCompactionRoleV1::FractionalCapabilityRoot)
        );
        // And it is a signer that is NOT writable -- the inversion the gate's
        // compaction arm enforces, because a compaction revises nothing.
        assert_eq!(
            FractionalCompactionRoleV1::FractionalCapabilityRoot.privileges(),
            (true, false)
        );
        // The dropped role is still declared, still refused, and refused for
        // its own stated reason rather than by falling into a neighbour's arm.
        assert_eq!(
            FractionalCompactionRoleV1::TradingCallerAuthority.admission(),
            FractionalCompactionAdmissionV1::RefusedTakesNoParentAuthority
        );
        assert!(
            FractionalCompactionRoleV1::declared()
                .contains(&FractionalCompactionRoleV1::TradingCallerAuthority),
            "refused is not the same as forgotten: the role stays declared so the \
             account cannot return by nobody having written down why it left"
        );
    }

    #[test]
    fn the_native_record_is_declared_here_and_refused_for_frac_r3s_reason() {
        // The two routes' welds, joined. FRACR3 refused a `TradingRecord`-owned
        // Position a native claim-check because its payee could not sign; this
        // route exists because that refusal is right. Reaching for the native
        // record from inside it would undo the reason it was written, so the
        // role is named and refused rather than merely absent.
        assert_eq!(
            FractionalCompactionRoleV1::NativeClaimCheckRecord.admission(),
            FractionalCompactionAdmissionV1::RefusedUnsignablePayee
        );
        assert_eq!(
            FractionalCompactionRoleV1::RetirementCursor.admission(),
            FractionalCompactionAdmissionV1::RefusedNotThisRoute
        );
    }

    #[test]
    fn the_three_new_magics_are_distinct_from_each_other_and_from_the_native_five() {
        use crate::claim_check_v1::{
            CLAIM_CHECK_COMPACT_MAGIC_V1, CLAIM_CHECK_OPEN_MAGIC_V1, CLAIM_CHECK_REDEEM_MAGIC_V1,
        };
        let magics = [
            FRACTIONAL_CLAIM_CHECK_RECORD_MAGIC_V1,
            FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1,
            FRACTIONAL_CLAIM_CHECK_REDEEM_MAGIC_V1,
            CLAIM_CHECK_OPEN_MAGIC_V1,
            CLAIM_CHECK_COMPACT_MAGIC_V1,
            CLAIM_CHECK_REDEEM_MAGIC_V1,
            CLAIM_CHECK_RECORD_MAGIC_V1,
            CLAIM_CHECK_ESCROW_MAGIC_V1,
        ];
        for (index, left) in magics.iter().enumerate() {
            assert!(!magics.iter().skip(index + 1).any(|right| right == left));
        }
    }
}
