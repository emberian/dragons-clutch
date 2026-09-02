//! Durable claim-check records that outlive the market that minted them.
//!
//! A terminal market cannot retire while any holder still owes themselves a
//! redemption, and no route but the holder's own can perform it. A holder who
//! never returns therefore holds the market, its rent, and every downstream
//! recovery open forever. Claim-check compaction replaces the perpetual account
//! with a perpetual *claim*: after a release-fixed deadline anyone may resolve a
//! sleeping holder's payout into an escrow only that holder can open, and the
//! market's own machinery closes.
//!
//! The payout is resolved **at compaction**, in collateral atoms, and never
//! re-derived. It cannot be otherwise: every input to the payout function --
//! the Claims aggregate, the Position, the linked basis record, the composition
//! graph record, and the Hoard -- is destroyed by the retirement the
//! claim-check exists to permit. A record storing raw per-outcome atoms would
//! be an IOU denominated in a function nobody can evaluate.
//!
//! Two consequences are visible in the layout below. The record is
//! **fixed-width**, independent of the market's runtime outcome width: it keeps
//! a digest of the position's atom vector as evidence rather than the vector
//! itself, so a 256-outcome market's claim-check costs the rent, and the
//! redemption compute, of a binary market's. And it carries its own `vault`,
//! `collateral_mint` and `release_set`, because at redemption time there is no
//! aggregate left to read them from.
//!
//! This module is pure wire and identity. Conservation lives in the plan
//! structs; account authority lives in the program.

use core::convert::TryInto;

/// Exact width of one durable claim-check record.
pub const CLAIM_CHECK_BYTES_V1: usize = 288;
/// Exact width of one per-market claim-check escrow record.
pub const CLAIM_CHECK_ESCROW_BYTES_V1: usize = 256;

/// Open-escrow request magic.
pub const CLAIM_CHECK_OPEN_MAGIC_V1: [u8; 8] = *b"DCLTCCO1";
/// Compaction request magic.
pub const CLAIM_CHECK_COMPACT_MAGIC_V1: [u8; 8] = *b"DCLTCCC1";
/// Claim-check redemption request magic.
pub const CLAIM_CHECK_REDEEM_MAGIC_V1: [u8; 8] = *b"DCLTCCR1";
/// Persisted claim-check record magic.
pub const CLAIM_CHECK_RECORD_MAGIC_V1: [u8; 8] = *b"DCLTCCK1";
/// Persisted claim-check escrow magic.
pub const CLAIM_CHECK_ESCROW_MAGIC_V1: [u8; 8] = *b"DCLTCCV1";

/// Implemented claim-check wire version.
pub const CLAIM_CHECK_WIRE_VERSION_V1: u16 = 1;
/// Persisted-record kind discriminant.
pub const CLAIM_CHECK_RECORD_KIND_V1: u8 = 1;
/// Persisted-escrow kind discriminant.
pub const CLAIM_CHECK_ESCROW_KIND_V1: u8 = 2;

/// Canonical claim-check PDA seed domain.
pub const CLAIM_CHECK_SEED_V1: &[u8] = b"dclutch:claim-check:v1";
/// Canonical claim-check escrow PDA seed domain.
pub const CLAIM_CHECK_ESCROW_SEED_V1: &[u8] = b"dclutch:claim-check-escrow:v1";
/// Canonical claim-check escrow vault PDA seed domain.
///
/// The vault is a Claims-derived PDA rather than the escrow's associated token
/// account, which is a deliberate departure from the design.
///
/// An associated token account lives at an address derived under the
/// *associated-token* program, so this program cannot sign for it and could not
/// create it with the tree's own `allocate`/`assign` idiom -- it would have to
/// CPI the associated-token program, putting a third-party program in a frame
/// that otherwise needs none. Deriving the vault here instead keeps creation on
/// the house pattern, keeps the frame smaller, and makes the vault's address
/// recoverable from the aggregate alone, which is exactly what a holder needs
/// once the market is gone.
///
/// Nothing about the design's reasoning is lost. The point of §4.2 was that the
/// vault is an ordinary `External` token account rather than a new Custody
/// compartment, and it still is: Custody authenticates a transfer destination
/// by its mint and its *owner*, never by how its address was derived, so from
/// Custody's side this is the same kind of account a holder's own wallet token
/// account already is.
pub const CLAIM_CHECK_VAULT_SEED_V1: &[u8] = b"dclutch:claim-check-vault:v1";

/// Slots a market must remain redeemable before any position may be compacted.
///
/// 38_880_000 slots is approximately 180 days at Solana's observed ~2.5
/// slots/second, or 216_000 slots/day. The arithmetic is stated so it can be
/// argued with. The job of the value is that "no honest holder is plausibly
/// asleep" holds for a person who checks their positions twice a year, and
/// being generous costs the holder almost nothing: compaction does not take
/// their value, it moves their already-resolved payout into an escrow only
/// they can open.
///
/// **This is a release constant, and that is the whole guarantee.** A founder
/// pins a release set at founding, which pins the claims ELF digest, which pins
/// this number: the founder reads their deadline by reading the release they
/// found on. A founder-set field was rejected under the same rationale that
/// motivates the feature — a founder choosing when *other people's* markets may
/// retire is itself an arbitrary actor inserting an arbitrary delay, landing on
/// parties who never agreed to it.
///
/// **Never shortenable post-founding.** Changing this needs a new ELF, hence a
/// new release set, and a live market's selected release set is write-once. A
/// shortened deadline applied to a live market would be confiscation with extra
/// steps. Should a release-set re-point route ever exist, it must refuse a
/// target whose deadline is shorter than the market's founding value;
/// lengthening is permitted, shortening is not.
///
/// **There is deliberately no test override and no feature flag.** A flag that
/// shortens the deadline is a shortening authority travelling with the build,
/// and the guarantee above rests on the deadline being a property of the
/// artifact everyone verifies. This mirrors the doctrine the record contract
/// already states for `CANONICAL_RECORD_MAX_STAGING_LIFETIME_SLOTS_V1`: a
/// successor release defines a new profile rather than silently changing an
/// in-progress bound. A campaign that needs a shorter wait builds a different
/// source revision, which the checked-release manifest already distinguishes by
/// `source_revision`, `source_digest` and `artifact_digest` — visible to
/// everyone who reads it, which a build flag would not be. A harness needs none
/// of this: it warps the clock.
pub const COMPACTION_DEADLINE_SLOTS_V1: u64 = 38_880_000;

/// Ceiling on the lamports one compaction crank pays the caller.
///
/// The reward is a *residual*, capped here, never a demand: a thin position
/// yields a small reward rather than a refusal. That distinction is
/// load-bearing — a compaction that could refuse for lack of funds would
/// reintroduce the sleeping-holder deadlock through the funding door.
///
/// Nothing new is taken from the Hoard. The crank is funded entirely from
/// lamports already leaving the position and admission accounts, which today
/// flow in full to a creation-fixed refund wallet — an identified party, and
/// the party that benefits most from retirement actually happening.
pub const COMPACTION_CRANK_REWARD_LAMPORTS_V1: u64 = 200_000;

const RECORD_VERSION_OFFSET: usize = 8;
const RECORD_KIND_OFFSET: usize = 10;
const RECORD_BUMP_OFFSET: usize = 11;
const RECORD_RESERVED_HEADER_OFFSET: usize = 12;
const RECORD_AGGREGATE_OFFSET: usize = 16;
const RECORD_OWNER_OFFSET: usize = 48;
const RECORD_MARKET_OFFSET: usize = 80;
const RECORD_RELEASE_SET_OFFSET: usize = 112;
const RECORD_VAULT_OFFSET: usize = 144;
const RECORD_COLLATERAL_MINT_OFFSET: usize = 176;
const RECORD_ATOMS_DIGEST_OFFSET: usize = 208;
const RECORD_ENTITLEMENT_OFFSET: usize = 240;
const RECORD_COMPACTED_SLOT_OFFSET: usize = 248;
const RECORD_GENERATION_OFFSET: usize = 256;
const RECORD_RESERVED_BODY_OFFSET: usize = 264;

const ESCROW_VERSION_OFFSET: usize = 8;
const ESCROW_KIND_OFFSET: usize = 10;
const ESCROW_BUMP_OFFSET: usize = 11;
const ESCROW_RESERVED_HEADER_OFFSET: usize = 12;
const ESCROW_AGGREGATE_OFFSET: usize = 16;
const ESCROW_MARKET_OFFSET: usize = 48;
const ESCROW_RELEASE_SET_OFFSET: usize = 80;
const ESCROW_VAULT_OFFSET: usize = 112;
const ESCROW_COLLATERAL_MINT_OFFSET: usize = 144;
const ESCROW_OPENER_OFFSET: usize = 176;
const ESCROW_OPENED_SLOT_OFFSET: usize = 208;
const ESCROW_OPENER_OUTLAY_OFFSET: usize = 216;
const ESCROW_OUTSTANDING_OFFSET: usize = 224;
const ESCROW_GENERATION_OFFSET: usize = 232;
const ESCROW_RESERVED_BODY_OFFSET: usize = 240;

const RESERVED_HEADER_BYTES: usize = 4;
const RECORD_RESERVED_BODY_BYTES: usize = 24;
const ESCROW_RESERVED_BODY_BYTES: usize = 16;

/// Stable hostile-decode or claim-check binding refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckErrorV1 {
    /// Bytes did not have the exact selected width.
    InvalidLength,
    /// Magic or version selected another wire family.
    InvalidHeader,
    /// Reserved bytes were not zero.
    NonCanonical,
    /// A persisted kind discriminant was not implemented.
    UnknownTag,
    /// A required identity was zero or two distinct identities aliased.
    InvalidIdentity,
    /// A claim-check promised no collateral, or a generation was zero.
    InvalidEntitlement,
    /// Checked arithmetic overflowed or underflowed.
    Arithmetic,
}

/// Result alias for claim-check wire operations.
pub type ClaimCheckResultV1<T> = core::result::Result<T, ClaimCheckErrorV1>;

/// Canonical claim-check PDA coordinates.
///
/// The coordinates are the position's own seeds. A claim-check's *address* is
/// therefore a proof of its holder, and the compaction route accepts no holder
/// identity as a wire field: a caller naming the wrong owner derives an address
/// that is not the account they passed. The refusals mirror
/// [`crate::protocol_position_v2::ProtocolPositionSeedsV2::new`] exactly rather
/// than restating a weaker set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckSeedsV1 {
    aggregate: [u8; 32],
    owner: [u8; 32],
}

impl ClaimCheckSeedsV1 {
    /// Construct the unique claim-check coordinates for one holder.
    pub fn new(aggregate: [u8; 32], owner: [u8; 32]) -> ClaimCheckResultV1<Self> {
        require_nonzero(aggregate)?;
        require_nonzero(owner)?;
        if aggregate == owner {
            return Err(ClaimCheckErrorV1::InvalidIdentity);
        }
        Ok(Self { aggregate, owner })
    }

    /// Borrow the sole exact claim-check PDA seed order, excluding its bump.
    pub fn as_slices(&self) -> [&[u8]; 3] {
        [CLAIM_CHECK_SEED_V1, &self.aggregate, &self.owner]
    }

    /// Return the Claims aggregate coordinate.
    pub const fn aggregate(self) -> [u8; 32] {
        self.aggregate
    }

    /// Return the holder coordinate.
    pub const fn owner(self) -> [u8; 32] {
        self.owner
    }
}

/// Canonical per-market claim-check escrow PDA coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckEscrowSeedsV1 {
    aggregate: [u8; 32],
}

impl ClaimCheckEscrowSeedsV1 {
    /// Construct the unique escrow coordinates for one market.
    pub fn new(aggregate: [u8; 32]) -> ClaimCheckResultV1<Self> {
        require_nonzero(aggregate)?;
        Ok(Self { aggregate })
    }

    /// Borrow the sole exact escrow PDA seed order, excluding its bump.
    pub fn as_slices(&self) -> [&[u8]; 2] {
        [CLAIM_CHECK_ESCROW_SEED_V1, &self.aggregate]
    }

    /// Return the Claims aggregate coordinate.
    pub const fn aggregate(self) -> [u8; 32] {
        self.aggregate
    }

    /// Bind a persisted bump, producing the exact signer projection.
    pub const fn with_bump(self, bump: u8) -> ClaimCheckEscrowSignerSeedsV1 {
        ClaimCheckEscrowSignerSeedsV1 {
            aggregate: self.aggregate,
            bump: [bump],
        }
    }
}

/// Canonical escrow vault PDA coordinates.
///
/// One vault per market, addressed by the same aggregate the escrow is, so a
/// holder returning to a market that no longer exists can find both from the
/// one coordinate their claim-check carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckVaultSeedsV1 {
    aggregate: [u8; 32],
}

impl ClaimCheckVaultSeedsV1 {
    /// Construct the unique vault coordinates for one market.
    pub fn new(aggregate: [u8; 32]) -> ClaimCheckResultV1<Self> {
        require_nonzero(aggregate)?;
        Ok(Self { aggregate })
    }

    /// Borrow the sole exact vault PDA seed order, excluding its bump.
    pub fn as_slices(&self) -> [&[u8]; 2] {
        [CLAIM_CHECK_VAULT_SEED_V1, &self.aggregate]
    }

    /// Return the Claims aggregate coordinate.
    pub const fn aggregate(self) -> [u8; 32] {
        self.aggregate
    }
}

/// Exact escrow signer-seed projection for a vault-authority invocation.
///
/// The escrow PDA is the vault's token authority: it, and nothing else, moves
/// collateral out of the vault. The bump is the one persisted in the escrow
/// record, so the signer recipe has exactly one author.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckEscrowSignerSeedsV1 {
    aggregate: [u8; 32],
    bump: [u8; 1],
}

impl ClaimCheckEscrowSignerSeedsV1 {
    /// Borrow the sole exact escrow signer seed order, bump included.
    pub fn as_slices(&self) -> [&[u8]; 3] {
        [CLAIM_CHECK_ESCROW_SEED_V1, &self.aggregate, &self.bump]
    }

    /// Return the bound bump.
    pub const fn bump(self) -> u8 {
        self.bump[0]
    }
}

/// One durable, fixed-width, permanently redeemable claim-check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckV1 {
    /// Claims aggregate the position was admitted against; a PDA seed.
    pub aggregate: [u8; 32],
    /// Sole holder entitled to redeem; a PDA seed, never a wire field.
    pub owner: [u8; 32],
    /// Logical Core Market identity, retained for audit after closure.
    pub market: [u8; 32],
    /// Release set the market pinned at founding.
    pub release_set: [u8; 32],
    /// Escrow vault token account holding this entitlement.
    pub vault: [u8; 32],
    /// Collateral mint the entitlement is denominated in.
    pub collateral_mint: [u8; 32],
    /// Digest of the position's per-outcome atom vector, kept as evidence.
    pub position_atoms_digest: [u8; 32],
    /// Collateral atoms owed, as the credit the vault was *observed* to take.
    pub entitlement_atoms: u64,
    /// Clock slot at which compaction resolved this payout.
    pub compacted_slot: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Persisted claim-check PDA bump.
    pub bump: u8,
}

impl ClaimCheckV1 {
    /// Construct and canonicalize one claim-check.
    pub fn new(self) -> ClaimCheckResultV1<Self> {
        self.validate()?;
        Ok(self)
    }

    /// Hostile-decode one exact persisted claim-check.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        exact_width(input, CLAIM_CHECK_BYTES_V1)?;
        exact(input, 0, &CLAIM_CHECK_RECORD_MAGIC_V1)?;
        if read_u16(input, RECORD_VERSION_OFFSET)? != CLAIM_CHECK_WIRE_VERSION_V1 {
            return Err(ClaimCheckErrorV1::InvalidHeader);
        }
        if read_byte(input, RECORD_KIND_OFFSET)? != CLAIM_CHECK_RECORD_KIND_V1 {
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
            owner: read_array(input, RECORD_OWNER_OFFSET)?,
            market: read_array(input, RECORD_MARKET_OFFSET)?,
            release_set: read_array(input, RECORD_RELEASE_SET_OFFSET)?,
            vault: read_array(input, RECORD_VAULT_OFFSET)?,
            collateral_mint: read_array(input, RECORD_COLLATERAL_MINT_OFFSET)?,
            position_atoms_digest: read_array(input, RECORD_ATOMS_DIGEST_OFFSET)?,
            entitlement_atoms: read_u64(input, RECORD_ENTITLEMENT_OFFSET)?,
            compacted_slot: read_u64(input, RECORD_COMPACTED_SLOT_OFFSET)?,
            generation: read_u64(input, RECORD_GENERATION_OFFSET)?,
            bump: read_byte(input, RECORD_BUMP_OFFSET)?,
        }
        .new()
    }

    /// Encode canonical persisted bytes.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; CLAIM_CHECK_BYTES_V1]> {
        self.validate()?;
        let mut output = [0; CLAIM_CHECK_BYTES_V1];
        write(&mut output, 0, &CLAIM_CHECK_RECORD_MAGIC_V1)?;
        write(
            &mut output,
            RECORD_VERSION_OFFSET,
            &CLAIM_CHECK_WIRE_VERSION_V1.to_le_bytes(),
        )?;
        write(
            &mut output,
            RECORD_KIND_OFFSET,
            &[CLAIM_CHECK_RECORD_KIND_V1],
        )?;
        write(&mut output, RECORD_BUMP_OFFSET, &[self.bump])?;
        for (offset, value) in [
            (RECORD_AGGREGATE_OFFSET, self.aggregate),
            (RECORD_OWNER_OFFSET, self.owner),
            (RECORD_MARKET_OFFSET, self.market),
            (RECORD_RELEASE_SET_OFFSET, self.release_set),
            (RECORD_VAULT_OFFSET, self.vault),
            (RECORD_COLLATERAL_MINT_OFFSET, self.collateral_mint),
            (RECORD_ATOMS_DIGEST_OFFSET, self.position_atoms_digest),
        ] {
            write(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (RECORD_ENTITLEMENT_OFFSET, self.entitlement_atoms),
            (RECORD_COMPACTED_SLOT_OFFSET, self.compacted_slot),
            (RECORD_GENERATION_OFFSET, self.generation),
        ] {
            write(&mut output, offset, &value.to_le_bytes())?;
        }
        Ok(output)
    }

    /// Return the exact PDA coordinates this record must live at.
    pub fn seeds(self) -> ClaimCheckResultV1<ClaimCheckSeedsV1> {
        ClaimCheckSeedsV1::new(self.aggregate, self.owner)
    }

    fn validate(self) -> ClaimCheckResultV1<()> {
        require_distinct(&[
            self.aggregate,
            self.owner,
            self.market,
            self.release_set,
            self.vault,
            self.collateral_mint,
        ])?;
        require_nonzero(self.position_atoms_digest)?;
        // A claim-check that promises nothing is not a claim-check. A position
        // whose terminal payout resolves to zero atoms -- every holder of a
        // losing outcome -- is compacted and closed without one, which is what
        // keeps the escrow's own close reachable: an unredeemable zero record
        // would pin `outstanding_claim_checks` above zero forever, and rebuild
        // the perpetual account this design exists to remove.
        if self.entitlement_atoms == 0 || self.generation == 0 {
            return Err(ClaimCheckErrorV1::InvalidEntitlement);
        }
        Ok(())
    }
}

/// One per-market claim-check escrow: the clock origin and the vault authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimCheckEscrowV1 {
    /// Claims aggregate this escrow serves; its sole PDA seed.
    pub aggregate: [u8; 32],
    /// Logical Core Market identity, retained for audit after closure.
    pub market: [u8; 32],
    /// Release set the market pinned at founding.
    pub release_set: [u8; 32],
    /// Escrow vault token account this record authorizes.
    pub vault: [u8; 32],
    /// Collateral mint the vault holds.
    pub collateral_mint: [u8; 32],
    /// Party who paid to open the escrow, repaid in full by the first crank.
    pub opener: [u8; 32],
    /// Clock slot at which the escrow opened: the compaction deadline's origin.
    ///
    /// Stamping here rather than at the market's terminal transition is a
    /// *lengthening*, never a shortening: the wait runs from when someone
    /// noticed, which is at or after terminal, so it can only ever be more
    /// generous to the holder. There is no uniformly readable terminal slot on
    /// chain, and adding one would mean a Lean edit and a second ELF.
    pub opened_slot: u64,
    /// Lamports the opener advanced, repaid before any crank pays itself.
    pub opener_outlay: u64,
    /// Live claim-checks; the escrow may close only at zero.
    pub outstanding_claim_checks: u64,
    /// Immutable Market generation.
    pub generation: u64,
    /// Persisted escrow PDA bump, sole author of the vault signer recipe.
    pub bump: u8,
}

impl ClaimCheckEscrowV1 {
    /// Construct and canonicalize one escrow record.
    pub fn new(self) -> ClaimCheckResultV1<Self> {
        self.validate()?;
        Ok(self)
    }

    /// Hostile-decode one exact persisted escrow record.
    pub fn decode(input: &[u8]) -> ClaimCheckResultV1<Self> {
        exact_width(input, CLAIM_CHECK_ESCROW_BYTES_V1)?;
        exact(input, 0, &CLAIM_CHECK_ESCROW_MAGIC_V1)?;
        if read_u16(input, ESCROW_VERSION_OFFSET)? != CLAIM_CHECK_WIRE_VERSION_V1 {
            return Err(ClaimCheckErrorV1::InvalidHeader);
        }
        if read_byte(input, ESCROW_KIND_OFFSET)? != CLAIM_CHECK_ESCROW_KIND_V1 {
            return Err(ClaimCheckErrorV1::UnknownTag);
        }
        require_zero(input, ESCROW_RESERVED_HEADER_OFFSET, RESERVED_HEADER_BYTES)?;
        require_zero(
            input,
            ESCROW_RESERVED_BODY_OFFSET,
            ESCROW_RESERVED_BODY_BYTES,
        )?;
        Self {
            aggregate: read_array(input, ESCROW_AGGREGATE_OFFSET)?,
            market: read_array(input, ESCROW_MARKET_OFFSET)?,
            release_set: read_array(input, ESCROW_RELEASE_SET_OFFSET)?,
            vault: read_array(input, ESCROW_VAULT_OFFSET)?,
            collateral_mint: read_array(input, ESCROW_COLLATERAL_MINT_OFFSET)?,
            opener: read_array(input, ESCROW_OPENER_OFFSET)?,
            opened_slot: read_u64(input, ESCROW_OPENED_SLOT_OFFSET)?,
            opener_outlay: read_u64(input, ESCROW_OPENER_OUTLAY_OFFSET)?,
            outstanding_claim_checks: read_u64(input, ESCROW_OUTSTANDING_OFFSET)?,
            generation: read_u64(input, ESCROW_GENERATION_OFFSET)?,
            bump: read_byte(input, ESCROW_BUMP_OFFSET)?,
        }
        .new()
    }

    /// Encode canonical persisted bytes.
    pub fn to_bytes(self) -> ClaimCheckResultV1<[u8; CLAIM_CHECK_ESCROW_BYTES_V1]> {
        self.validate()?;
        let mut output = [0; CLAIM_CHECK_ESCROW_BYTES_V1];
        write(&mut output, 0, &CLAIM_CHECK_ESCROW_MAGIC_V1)?;
        write(
            &mut output,
            ESCROW_VERSION_OFFSET,
            &CLAIM_CHECK_WIRE_VERSION_V1.to_le_bytes(),
        )?;
        write(
            &mut output,
            ESCROW_KIND_OFFSET,
            &[CLAIM_CHECK_ESCROW_KIND_V1],
        )?;
        write(&mut output, ESCROW_BUMP_OFFSET, &[self.bump])?;
        for (offset, value) in [
            (ESCROW_AGGREGATE_OFFSET, self.aggregate),
            (ESCROW_MARKET_OFFSET, self.market),
            (ESCROW_RELEASE_SET_OFFSET, self.release_set),
            (ESCROW_VAULT_OFFSET, self.vault),
            (ESCROW_COLLATERAL_MINT_OFFSET, self.collateral_mint),
            (ESCROW_OPENER_OFFSET, self.opener),
        ] {
            write(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (ESCROW_OPENED_SLOT_OFFSET, self.opened_slot),
            (ESCROW_OPENER_OUTLAY_OFFSET, self.opener_outlay),
            (ESCROW_OUTSTANDING_OFFSET, self.outstanding_claim_checks),
            (ESCROW_GENERATION_OFFSET, self.generation),
        ] {
            write(&mut output, offset, &value.to_le_bytes())?;
        }
        Ok(output)
    }

    /// Return the exact PDA coordinates this record must live at.
    pub fn seeds(self) -> ClaimCheckResultV1<ClaimCheckEscrowSeedsV1> {
        ClaimCheckEscrowSeedsV1::new(self.aggregate)
    }

    /// Return the exact vault signer projection under the persisted bump.
    pub fn signer_seeds(self) -> ClaimCheckResultV1<ClaimCheckEscrowSignerSeedsV1> {
        Ok(self.seeds()?.with_bump(self.bump))
    }

    /// Record one further live claim-check.
    pub fn admit_claim_check(self) -> ClaimCheckResultV1<Self> {
        Self {
            outstanding_claim_checks: self
                .outstanding_claim_checks
                .checked_add(1)
                .ok_or(ClaimCheckErrorV1::Arithmetic)?,
            ..self
        }
        .new()
    }

    /// Retire one redeemed claim-check.
    pub fn retire_claim_check(self) -> ClaimCheckResultV1<Self> {
        Self {
            outstanding_claim_checks: self
                .outstanding_claim_checks
                .checked_sub(1)
                .ok_or(ClaimCheckErrorV1::Arithmetic)?,
            ..self
        }
        .new()
    }

    /// Whether every minted claim-check has been redeemed.
    pub const fn is_settled(self) -> bool {
        self.outstanding_claim_checks == 0
    }

    fn validate(self) -> ClaimCheckResultV1<()> {
        require_distinct(&[
            self.aggregate,
            self.market,
            self.release_set,
            self.vault,
            self.collateral_mint,
            self.opener,
        ])?;
        if self.generation == 0 {
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

    fn record() -> ClaimCheckV1 {
        ClaimCheckV1 {
            aggregate: [1; 32],
            owner: [2; 32],
            market: [3; 32],
            release_set: [4; 32],
            vault: [5; 32],
            collateral_mint: [6; 32],
            position_atoms_digest: [7; 32],
            entitlement_atoms: 900_001,
            compacted_slot: 12_345,
            generation: 9,
            bump: 254,
        }
        .new()
        .expect("record")
    }

    fn escrow() -> ClaimCheckEscrowV1 {
        ClaimCheckEscrowV1 {
            aggregate: [1; 32],
            market: [3; 32],
            release_set: [4; 32],
            vault: [5; 32],
            collateral_mint: [6; 32],
            opener: [8; 32],
            opened_slot: 12_000,
            opener_outlay: 2_039_280,
            outstanding_claim_checks: 0,
            generation: 9,
            bump: 253,
        }
        .new()
        .expect("escrow")
    }

    #[test]
    fn record_round_trips_at_its_one_exact_width() {
        let value = record();
        let bytes = value.to_bytes().expect("bytes");
        assert_eq!(bytes.len(), CLAIM_CHECK_BYTES_V1);
        assert_eq!(ClaimCheckV1::decode(&bytes), Ok(value));
    }

    #[test]
    fn escrow_round_trips_at_its_one_exact_width() {
        let value = escrow();
        let bytes = value.to_bytes().expect("bytes");
        assert_eq!(bytes.len(), CLAIM_CHECK_ESCROW_BYTES_V1);
        assert_eq!(ClaimCheckEscrowV1::decode(&bytes), Ok(value));
    }

    #[test]
    fn record_width_is_independent_of_market_outcome_width() {
        // The whole point of the digest: a 256-outcome market's claim-check is
        // byte-identical in size to a binary market's, so its rent is provably
        // covered by the position's own rent at any width.
        let narrow = record().to_bytes().expect("narrow");
        let mut wide_source = record();
        wide_source.position_atoms_digest = [200; 32];
        let wide = wide_source.to_bytes().expect("wide");
        assert_eq!(narrow.len(), wide.len());
    }

    #[test]
    fn record_refuses_a_truncated_or_extended_input() {
        let bytes = record().to_bytes().expect("bytes");
        assert_eq!(
            ClaimCheckV1::decode(&bytes[..CLAIM_CHECK_BYTES_V1 - 1]),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        let mut long = [0_u8; CLAIM_CHECK_BYTES_V1 + 1];
        long.get_mut(..CLAIM_CHECK_BYTES_V1)
            .expect("prefix")
            .copy_from_slice(&bytes);
        assert_eq!(
            ClaimCheckV1::decode(&long),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
        assert_eq!(
            ClaimCheckV1::decode(&[]),
            Err(ClaimCheckErrorV1::InvalidLength)
        );
    }

    #[test]
    fn record_refuses_another_wire_family() {
        let mut bytes = record().to_bytes().expect("bytes");
        write(&mut bytes, 0, &CLAIM_CHECK_ESCROW_MAGIC_V1).expect("swap magic");
        assert_eq!(
            ClaimCheckV1::decode(&bytes),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );

        let mut versioned = record().to_bytes().expect("bytes");
        write(&mut versioned, RECORD_VERSION_OFFSET, &2_u16.to_le_bytes()).expect("version");
        assert_eq!(
            ClaimCheckV1::decode(&versioned),
            Err(ClaimCheckErrorV1::InvalidHeader)
        );

        let mut kinded = record().to_bytes().expect("bytes");
        write(
            &mut kinded,
            RECORD_KIND_OFFSET,
            &[CLAIM_CHECK_ESCROW_KIND_V1],
        )
        .expect("kind");
        assert_eq!(
            ClaimCheckV1::decode(&kinded),
            Err(ClaimCheckErrorV1::UnknownTag)
        );
    }

    #[test]
    fn record_refuses_nonzero_reserved_runs() {
        for offset in [RECORD_RESERVED_HEADER_OFFSET, RECORD_RESERVED_BODY_OFFSET] {
            let mut bytes = record().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[1]).expect("dirty reserved");
            assert_eq!(
                ClaimCheckV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
        // Every reserved byte, not merely the first of each run.
        for offset in RECORD_RESERVED_BODY_OFFSET..CLAIM_CHECK_BYTES_V1 {
            let mut bytes = record().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[0xFF]).expect("dirty reserved");
            assert_eq!(
                ClaimCheckV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
    }

    #[test]
    fn escrow_refuses_nonzero_reserved_runs() {
        for offset in [ESCROW_RESERVED_HEADER_OFFSET, ESCROW_RESERVED_BODY_OFFSET] {
            let mut bytes = escrow().to_bytes().expect("bytes");
            write(&mut bytes, offset, &[1]).expect("dirty reserved");
            assert_eq!(
                ClaimCheckEscrowV1::decode(&bytes),
                Err(ClaimCheckErrorV1::NonCanonical)
            );
        }
    }

    #[test]
    fn record_refuses_a_zero_or_aliased_identity() {
        for mutate in [
            |value: &mut ClaimCheckV1| value.aggregate = [0; 32],
            |value: &mut ClaimCheckV1| value.owner = [0; 32],
            |value: &mut ClaimCheckV1| value.market = [0; 32],
            |value: &mut ClaimCheckV1| value.release_set = [0; 32],
            |value: &mut ClaimCheckV1| value.vault = [0; 32],
            |value: &mut ClaimCheckV1| value.collateral_mint = [0; 32],
        ] {
            let mut value = record();
            mutate(&mut value);
            assert_eq!(value.new(), Err(ClaimCheckErrorV1::InvalidIdentity));
            assert_eq!(value.to_bytes(), Err(ClaimCheckErrorV1::InvalidIdentity));
        }

        // The aggregate-equals-owner refusal mirrors ProtocolPositionSeedsV2.
        let mut aliased = record();
        aliased.owner = aliased.aggregate;
        assert_eq!(aliased.new(), Err(ClaimCheckErrorV1::InvalidIdentity));

        let mut vault_alias = record();
        vault_alias.vault = vault_alias.collateral_mint;
        assert_eq!(vault_alias.new(), Err(ClaimCheckErrorV1::InvalidIdentity));

        let mut zero_digest = record();
        zero_digest.position_atoms_digest = [0; 32];
        assert_eq!(zero_digest.new(), Err(ClaimCheckErrorV1::InvalidIdentity));
    }

    #[test]
    fn seeds_refuse_exactly_what_the_position_seeds_refuse() {
        assert_eq!(
            ClaimCheckSeedsV1::new([0; 32], [2; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        assert_eq!(
            ClaimCheckSeedsV1::new([1; 32], [0; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        assert_eq!(
            ClaimCheckSeedsV1::new([1; 32], [1; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
        let seeds = ClaimCheckSeedsV1::new([1; 32], [2; 32]).expect("seeds");
        assert_eq!(seeds.as_slices(), [CLAIM_CHECK_SEED_V1, &[1; 32], &[2; 32]]);
        assert_eq!(
            ClaimCheckEscrowSeedsV1::new([0; 32]),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
    }

    #[test]
    fn the_record_address_is_the_positions_own_address_recipe() {
        // A claim-check's coordinates are the position's coordinates under a
        // different domain, so a caller naming the wrong owner derives an
        // address that is not the account they passed. There is no holder field
        // to forge.
        let value = record();
        let seeds = value.seeds().expect("seeds");
        assert_eq!(seeds.aggregate(), value.aggregate);
        assert_eq!(seeds.owner(), value.owner);
        assert_ne!(CLAIM_CHECK_SEED_V1, CLAIM_CHECK_ESCROW_SEED_V1);
    }

    #[test]
    fn a_claim_check_promising_nothing_is_refused() {
        let mut empty = record();
        empty.entitlement_atoms = 0;
        assert_eq!(empty.new(), Err(ClaimCheckErrorV1::InvalidEntitlement));
        assert_eq!(empty.to_bytes(), Err(ClaimCheckErrorV1::InvalidEntitlement));

        let mut ungenerated = record();
        ungenerated.generation = 0;
        assert_eq!(
            ungenerated.new(),
            Err(ClaimCheckErrorV1::InvalidEntitlement)
        );
    }

    #[test]
    fn a_decoded_record_carrying_a_zero_entitlement_is_still_refused() {
        // The refusal must survive the wire, not merely the constructor: bytes
        // are what an adversary controls.
        let mut bytes = record().to_bytes().expect("bytes");
        write(&mut bytes, RECORD_ENTITLEMENT_OFFSET, &0_u64.to_le_bytes()).expect("zero");
        assert_eq!(
            ClaimCheckV1::decode(&bytes),
            Err(ClaimCheckErrorV1::InvalidEntitlement)
        );

        let mut aliased = record().to_bytes().expect("bytes");
        write(&mut aliased, RECORD_OWNER_OFFSET, &[1; 32]).expect("alias owner to aggregate");
        assert_eq!(
            ClaimCheckV1::decode(&aliased),
            Err(ClaimCheckErrorV1::InvalidIdentity)
        );
    }

    #[test]
    fn outstanding_claim_checks_only_move_by_one_and_never_below_zero() {
        let opened = escrow();
        assert!(opened.is_settled());
        assert_eq!(
            opened.retire_claim_check(),
            Err(ClaimCheckErrorV1::Arithmetic)
        );

        let one = opened.admit_claim_check().expect("admit");
        assert_eq!(one.outstanding_claim_checks, 1);
        assert!(!one.is_settled());
        let two = one.admit_claim_check().expect("admit");
        assert_eq!(two.outstanding_claim_checks, 2);
        let back = two
            .retire_claim_check()
            .expect("retire")
            .retire_claim_check()
            .expect("retire");
        assert_eq!(back, opened);
        assert!(back.is_settled());

        let saturated = ClaimCheckEscrowV1 {
            outstanding_claim_checks: u64::MAX,
            ..opened
        };
        assert_eq!(
            saturated.admit_claim_check(),
            Err(ClaimCheckErrorV1::Arithmetic)
        );
    }

    #[test]
    fn the_vault_signer_recipe_has_exactly_one_author() {
        let value = escrow();
        let signer = value.signer_seeds().expect("signer");
        assert_eq!(signer.bump(), value.bump);
        assert_eq!(
            signer.as_slices(),
            [
                CLAIM_CHECK_ESCROW_SEED_V1,
                &value.aggregate[..],
                &[value.bump][..]
            ]
        );
    }

    #[test]
    fn the_deadlines_stated_arithmetic_is_the_arithmetic() {
        // 216_000 slots/day at ~2.5 slots/second, for 180 days. The value is
        // written out so a reader can argue with the reasoning rather than
        // reverse-engineer it from a magic number.
        const SLOTS_PER_DAY: u64 = 216_000;
        assert_eq!(COMPACTION_DEADLINE_SLOTS_V1, SLOTS_PER_DAY * 180);
        // Generous by construction: a holder who checks twice a year is never
        // compacted out of their familiar route.
        const _: () = assert!(COMPACTION_DEADLINE_SLOTS_V1 > SLOTS_PER_DAY * 120);
    }

    #[test]
    fn the_deadline_horizon_must_be_computed_with_checked_arithmetic() {
        // A route computing `opened_slot + DEADLINE` on a wrapping add would
        // turn a far-future origin into an already-elapsed deadline, which is
        // the premature crank the whole gate exists to refuse.
        assert_eq!(u64::MAX.checked_add(COMPACTION_DEADLINE_SLOTS_V1), None);
        assert_eq!(
            12_000_u64.checked_add(COMPACTION_DEADLINE_SLOTS_V1),
            Some(38_892_000)
        );
    }

    #[test]
    fn the_redemption_frame_contains_nothing_the_market_takes_with_it() {
        // The assertion is on the SPEC, not on an inspection of the route. A
        // later edit that reaches for an aggregate, a Core state, a basis or
        // graph record, a Hoard or a Custody replay cursor has to add a role
        // here and answer `survives_retirement` for it, and the honest answer
        // fails this test.
        for role in ClaimCheckRedemptionRoleV1::frame() {
            assert!(
                role.survives_retirement(),
                "{role:?} does not outlive the market and cannot be in this frame"
            );
        }
    }

    #[test]
    fn the_redemption_frame_is_pinned_exactly() {
        // Width and order are both load-bearing: the route indexes this frame,
        // and a silent insertion would shift every account after it.
        assert_eq!(
            ClaimCheckRedemptionRoleV1::frame(),
            [
                ClaimCheckRedemptionRoleV1::Holder,
                ClaimCheckRedemptionRoleV1::ClaimCheckRecord,
                ClaimCheckRedemptionRoleV1::Escrow,
                ClaimCheckRedemptionRoleV1::Vault,
                ClaimCheckRedemptionRoleV1::HolderTokens,
                ClaimCheckRedemptionRoleV1::CollateralMint,
                ClaimCheckRedemptionRoleV1::TokenProgram,
            ]
        );
        assert_eq!(
            ClaimCheckRedemptionRoleV1::frame().len(),
            CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1
        );
    }

    #[test]
    fn exactly_one_account_signs_and_it_is_the_person_being_paid() {
        // GREEN-SELF stated as arithmetic: no third party stands between a
        // holder and their collateral, and there is no second signature for
        // anyone to be induced to provide.
        let signers: usize = ClaimCheckRedemptionRoleV1::frame()
            .iter()
            .filter(|role| role.privileges().0)
            .count();
        assert_eq!(signers, 1);
        assert!(ClaimCheckRedemptionRoleV1::Holder.privileges().0);
        assert!(ClaimCheckRedemptionRoleV1::Holder.privileges().1);
        // Nothing the route only reads is writable.
        for role in [
            ClaimCheckRedemptionRoleV1::CollateralMint,
            ClaimCheckRedemptionRoleV1::TokenProgram,
        ] {
            assert_eq!(role.privileges(), (false, false));
        }
    }

    #[test]
    fn a_redemption_frame_is_far_smaller_than_the_compaction_that_made_it() {
        // The residue claim, as a number. A crank needs the whole market; a
        // holder coming back needs seven accounts and none of the market's.
        assert_eq!(CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1, 7);
        const _: () = assert!(CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1 < 36);
    }

    #[test]
    fn the_five_wire_magics_are_pairwise_distinct() {
        let magics = [
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

/// Exact claim-check redemption frame width.
pub const CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1: usize = 7;

/// One account role in the claim-check redemption frame.
///
/// This frame is the point of the whole design, so it is declared rather than
/// left implicit. What a holder touches when they finally come back is a market
/// that no longer exists: the aggregate closed, the Core state closed, the
/// linked basis and composition graph records closed, the Hoard emptied and
/// closed, the Custody replay cursor gone. A redemption route that needed any
/// of them would not be a claim-check; it would be a promise that stops working
/// the moment the thing it depends on is cleaned up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckRedemptionRoleV1 {
    /// The holder, who signs and is paid. The only signer this frame admits.
    Holder,
    /// The claim-check record, closed into the holder.
    ClaimCheckRecord,
    /// The per-market escrow, whose outstanding count this redemption retires.
    Escrow,
    /// The escrow vault, debited by exactly the record's entitlement.
    Vault,
    /// The holder's own token account, credited.
    HolderTokens,
    /// The collateral mint, for a checked transfer.
    CollateralMint,
    /// The collateral token program.
    TokenProgram,
}

impl ClaimCheckRedemptionRoleV1 {
    /// The exact ordered frame.
    #[must_use]
    pub const fn frame() -> [Self; CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1] {
        [
            Self::Holder,
            Self::ClaimCheckRecord,
            Self::Escrow,
            Self::Vault,
            Self::HolderTokens,
            Self::CollateralMint,
            Self::TokenProgram,
        ]
    }

    /// Whether an account in this role still exists after the market retires.
    ///
    /// **Every role in [`Self::frame`] must answer `true`, and a test enforces
    /// it.** The method exists so that adding an account to this route is not a
    /// silent act: a later edit must state, here, whether the thing it is
    /// reaching for outlives the market. The honest answer for an aggregate, a
    /// Core state, a basis or graph record, a Hoard or a Custody replay cursor
    /// is `false`, and the test fails on it. Answering `true` for one of those
    /// would be a deliberate false statement rather than an oversight, which is
    /// the most a type can do about it.
    #[must_use]
    pub const fn survives_retirement(self) -> bool {
        match self {
            // Created by compaction, or independent of the market entirely.
            Self::Holder
            | Self::ClaimCheckRecord
            | Self::Escrow
            | Self::Vault
            | Self::HolderTokens
            | Self::CollateralMint
            | Self::TokenProgram => true,
        }
    }

    /// Exact privileges this role carries.
    #[must_use]
    pub const fn privileges(self) -> (bool, bool) {
        match self {
            // signer, writable
            Self::Holder => (true, true),
            Self::ClaimCheckRecord | Self::Escrow | Self::Vault | Self::HolderTokens => {
                (false, true)
            }
            Self::CollateralMint | Self::TokenProgram => (false, false),
        }
    }
}
