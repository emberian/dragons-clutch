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
