//! Refusals and route admission for fractional claim-checks.
//!
//! [`crate::claim_check_compaction_v1`] compacts a sleeping *person's* Position
//! into a record only that person can open, and refuses every owner kind that
//! cannot sign for its own payout. That refusal is correct and is not relaxed
//! here. What this module adds is the second destination the refusal implies:
//! a Position whose owner cannot sign, but whose claimants hold an instrument,
//! goes to a record addressed by the instrument instead of by a payee.
//!
//! So the two gates are complementary rather than competing, and the point of
//! stating them next to each other is that every owner kind now has to answer
//! *which* claim-check it gets, rather than only whether it gets the native one.
//! A fourth owner kind is a compile error in [`claim_check_route_for`] and not a
//! silent inheritance of whichever arm it was written beside -- C0's lesson from
//! the compaction design's §15.5, applied a third time.
//!
//! # The refusal a shard burn cannot get around
//!
//! Worth stating where the codes live, because it is the fact that shapes the
//! whole fractional route and it is not visible from the design.
//!
//! A shard Mint carries Token-2022's `PermissionedBurn` extension, pinned by
//! `Token2022BehaviorProfileV2::read_mint` to the Mint's controller. In the
//! Fractional family that controller is the capability root: a **Trading**-
//! derived PDA. Token-2022 refuses a *standard* burn outright while that
//! extension is present, and its permissioned burn demands the configured
//! authority as a second signer. So no signature a shard holder can produce will
//! ever burn a shard on its own, and Claims cannot produce the other one.
//!
//! Compaction therefore has to re-point the Mint's burn authority to the escrow
//! -- a PDA Claims *can* sign -- while the root is still alive to authorize it.
//! That is why `Authority` and `ShardMint` are separate codes below: a route
//! that reached the burn without having done the re-point fails for a reason
//! that is worth telling apart from a frame error.

use dclutch_claims_svm::claim_check_conservation_v1::ClaimCheckCompactionObservationV1;
use dclutch_claims_svm::claim_check_v1::{
    COMPACTION_CRANK_REWARD_LAMPORTS_V1, COMPACTION_DEADLINE_SLOTS_V1, ClaimCheckEscrowSeedsV1,
    ClaimCheckEscrowV1, ClaimCheckVaultSeedsV1,
};
use dclutch_claims_svm::fractional_claim_check_compaction_receipt_v1::{
    FractionalClaimCheckCompactionReceiptInputV1, FractionalClaimCheckCompactionReceiptV1,
};
use dclutch_claims_svm::fractional_claim_check_compaction_request_v1::FractionalCompactToClaimCheckRequestV1;
use dclutch_claims_svm::fractional_claim_check_conservation_v1::{
    FractionalClaimCheckCompactionObservationV1, FractionalClaimCheckCompactionPlanV1,
};
use dclutch_claims_svm::fractional_claim_check_v1::{
    FRACTIONAL_CLAIM_CHECK_BYTES_V1, FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1,
    FRACTIONAL_COMPACT_TERMINAL_FRAME_V1, FractionalClaimCheckSeedsV1, FractionalClaimCheckV1,
    FractionalCompactionRoleV1,
};
use dclutch_claims_svm::protocol_position_v2::{
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
};
use dclutch_claims_svm::terminal_settlement_v3::{
    TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3, TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsV2,
};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_token_svm::{
    TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, Token2022BehaviorProfileV2, TokenBehaviorSelectionV2,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    program::{invoke, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::system_program;
use spl_token_2022_interface::instruction as token_instruction;

use crate::claim_check_compaction_v1::{
    COMPACT_TERMINAL_POSITION_ACCOUNT_V1, allocate_and_assign, close_and_split, observation,
    token_balance,
};
use crate::rational_representation_v2::authenticate_finalized_rational_record;

/// Stable fractional claim-check compaction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FractionalClaimCheckCompactionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5640,
    /// A signer the route does not admit was present, or a required one absent.
    ///
    /// The Fractional capability root is a required signer here and only here:
    /// re-pointing the shard Mint's burn authority needs the authority it is
    /// being moved away from, and after this instruction nobody can produce it.
    Authority = 0x5641,
    /// Coordinates did not derive the passed account, or aliased, or were zero.
    Identity = 0x5642,
    /// The compaction deadline had not elapsed at the observed slot.
    Deadline = 0x5643,
    /// The Core phase, or the absence of a terminal receipt, refused.
    Phase = 0x5644,
    /// The fractional claim-check address was already occupied.
    AlreadyCompacted = 0x5645,
    /// A plan's atoms, shards or lamports did not balance.
    Conservation = 0x5646,
    /// The terminal payout derivation refused.
    Economic = 0x5647,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5648,
    /// The escrow was absent, or its mint or token program did not match.
    Escrow = 0x5649,
    /// A position kind this route does not fractionally compact.
    Scope = 0x564A,
    /// The finalized exposure terms, or the coordinate they declare, refused.
    Terms = 0x564B,
    /// The shard Mint's profile, supply, or burn-authority hand-off refused.
    ShardMint = 0x564C,
}

impl FractionalClaimCheckCompactionSbfErrorV1 {
    /// Every refusal this request family can raise, in discriminant order.
    ///
    /// This is what the sub-band assertions below read. It is kept honest by
    /// [`FractionalClaimCheckCompactionSbfErrorV1::ordinal`], whose match is exhaustive: a variant
    /// added to the enum does not compile until its author writes an arm here, and the only arm
    /// that satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 13] = [
        Self::Accounts,
        Self::Authority,
        Self::Identity,
        Self::Deadline,
        Self::Phase,
        Self::AlreadyCompacted,
        Self::Conservation,
        Self::Economic,
        Self::Receipt,
        Self::Escrow,
        Self::Scope,
        Self::Terms,
        Self::ShardMint,
    ];

    /// This refusal's position in [`FractionalClaimCheckCompactionSbfErrorV1::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a fourteenth variant is
    /// a COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Accounts => 0,
            Self::Authority => 1,
            Self::Identity => 2,
            Self::Deadline => 3,
            Self::Phase => 4,
            Self::AlreadyCompacted => 5,
            Self::Conservation => 6,
            Self::Economic => 7,
            Self::Receipt => 8,
            Self::Escrow => 9,
            Self::Scope => 10,
            Self::Terms => 11,
            Self::ShardMint => 12,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing about
// the variants after it and goes stale silently every single time the family
// grows -- the failure is not that the name is wrong, it is that nothing can
// notice. Claims' own top-level band proved it the expensive way: its bound went
// on naming `ReleaseSuperseded` after a later variant landed, so for as long as
// that stood, the newest refusal in the program was checked by nothing.
//
// So the sub-band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    const SUB_BAND: u32 = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x640;
    assert!(
        FractionalClaimCheckCompactionSbfErrorV1::ALL[0] as u32 == SUB_BAND,
        "FractionalClaimCheckCompactionSbfErrorV1 must start at its registered sub-band offset"
    );
    let mut index = 0;
    while index < FractionalClaimCheckCompactionSbfErrorV1::ALL.len() {
        let variant = FractionalClaimCheckCompactionSbfErrorV1::ALL[index];
        assert!(
            variant.ordinal() == index,
            "FractionalClaimCheckCompactionSbfErrorV1::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == SUB_BAND + index as u32,
            "FractionalClaimCheckCompactionSbfErrorV1 discriminants are not the contiguous run from the sub-band offset that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "FractionalClaimCheckCompactionSbfErrorV1 must not run past its registered refusal band"
        );
        index += 1;
    }
};
// Four sub-bands now: native compaction 0x600, native redemption 0x620,
// fractional compaction 0x640, fractional redemption 0x660. Each is
// independently versioned, so none may grow into the next. These assertions are
// what would catch it, and they stay necessary after the weld above: the
// exhaustive loop proves a family is a contiguous run from ITS OWN offset and
// says nothing about whether that run has reached the next family's.
//
// Both endpoints are read off `ALL` rather than named. A separation assertion
// that names the variants by hand is the same defect it is here to prevent.
const _: () = {
    const NATIVE_REDEMPTION_TOP: u32 = {
        use crate::claim_check_redemption_v1::ClaimCheckRedemptionSbfErrorV1 as Native;
        Native::ALL[Native::ALL.len() - 1] as u32
    };
    const FRACTIONAL_COMPACTION_BASE: u32 = FractionalClaimCheckCompactionSbfErrorV1::ALL[0] as u32;
    assert!(
        NATIVE_REDEMPTION_TOP < FRACTIONAL_COMPACTION_BASE,
        "the native redemption sub-band must not run into fractional compaction"
    );
};

impl From<FractionalClaimCheckCompactionSbfErrorV1> for ProgramError {
    fn from(value: FractionalClaimCheckCompactionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

/// Stable fractional claim-check redemption refusal.
///
/// Short, and the shortness is the feature: what a shard holder touches when
/// they finally come back is a market that no longer exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FractionalClaimCheckRedemptionSbfErrorV1 {
    /// The fixed account frame, ownership, or writability refused.
    Accounts = 0x5660,
    /// The signer was not the presented shard account's own owner.
    ///
    /// Not "was not the record's holder": this record names no holder. The only
    /// authority a redemption proves is over the shards being burned.
    Authority = 0x5661,
    /// The record was not at its derived address, or a mint did not match.
    Identity = 0x5662,
    /// The vault debit, the shard burn, or the pay-down did not balance.
    Conservation = 0x5663,
    /// Observed post-balances did not match the admitted plan.
    Receipt = 0x5664,
    /// The shard balance presented forms no whole Claims coordinate.
    ///
    /// Its own code, and not folded into `Conservation`, because it is the one
    /// refusal an honest holder can hit: sub-denominator dust was never a claim
    /// on collateral and does not become one here. A holder seeing this needs to
    /// be told to consolidate, not to suspect the escrow.
    NoWholeClaim = 0x5665,
    /// An escrow close was attempted while fractional claim-checks were live.
    Vault = 0x5666,
}

impl FractionalClaimCheckRedemptionSbfErrorV1 {
    /// Every refusal this request family can raise, in discriminant order.
    ///
    /// This is what the sub-band assertions below read. It is kept honest by
    /// [`FractionalClaimCheckRedemptionSbfErrorV1::ordinal`], whose match is exhaustive: a variant
    /// added to the enum does not compile until its author writes an arm here, and the only arm
    /// that satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 7] = [
        Self::Accounts,
        Self::Authority,
        Self::Identity,
        Self::Conservation,
        Self::Receipt,
        Self::NoWholeClaim,
        Self::Vault,
    ];

    /// This refusal's position in [`FractionalClaimCheckRedemptionSbfErrorV1::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: an eighth variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Accounts => 0,
            Self::Authority => 1,
            Self::Identity => 2,
            Self::Conservation => 3,
            Self::Receipt => 4,
            Self::NoWholeClaim => 5,
            Self::Vault => 6,
        }
    }
}
// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing about
// the variants after it and goes stale silently every single time the family
// grows -- the failure is not that the name is wrong, it is that nothing can
// notice. Claims' own top-level band proved it the expensive way: its bound went
// on naming `ReleaseSuperseded` after a later variant landed, so for as long as
// that stood, the newest refusal in the program was checked by nothing.
//
// So the sub-band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    const SUB_BAND: u32 = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x660;
    assert!(
        FractionalClaimCheckRedemptionSbfErrorV1::ALL[0] as u32 == SUB_BAND,
        "FractionalClaimCheckRedemptionSbfErrorV1 must start at its registered sub-band offset"
    );
    let mut index = 0;
    while index < FractionalClaimCheckRedemptionSbfErrorV1::ALL.len() {
        let variant = FractionalClaimCheckRedemptionSbfErrorV1::ALL[index];
        assert!(
            variant.ordinal() == index,
            "FractionalClaimCheckRedemptionSbfErrorV1::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == SUB_BAND + index as u32,
            "FractionalClaimCheckRedemptionSbfErrorV1 discriminants are not the contiguous run from the sub-band offset that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "FractionalClaimCheckRedemptionSbfErrorV1 must not run past its registered refusal band"
        );
        index += 1;
    }
};
// Endpoints read off `ALL` for the same reason as the assertion above.
const _: () = {
    const COMPACTION_TOP: u32 = FractionalClaimCheckCompactionSbfErrorV1::ALL
        [FractionalClaimCheckCompactionSbfErrorV1::ALL.len() - 1]
        as u32;
    const REDEMPTION_BASE: u32 = FractionalClaimCheckRedemptionSbfErrorV1::ALL[0] as u32;
    assert!(
        COMPACTION_TOP < REDEMPTION_BASE,
        "the fractional compaction sub-band must not run into fractional redemption"
    );
};

impl From<FractionalClaimCheckRedemptionSbfErrorV1> for ProgramError {
    fn from(value: FractionalClaimCheckRedemptionSbfErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

/// Which claim-check, if any, a Position's owner kind is entitled to.
///
/// The question the native weld only half asked. `owner_kind_can_open_a_claim_check`
/// answers "can this address sign for its own payout", which is exactly right
/// and is why the Fractional reserve is refused there. But "cannot sign" is not
/// the same as "has no claimants", and conflating the two is what turned a delay
/// into a total loss the first time. So the routing is stated once, exhaustively,
/// and both gates read it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimCheckRouteV1 {
    /// A wallet: it signs, so a record may name it as the payee.
    Native,
    /// A program-derived owner whose claimants hold a Fractional shard Mint.
    Fractional,
    /// Neither route can represent this owner's claimants today.
    Neither,
}

/// Route one Position owner kind to the claim-check that can represent it.
///
/// Exhaustive on purpose, and the arms carry their reasons because the reasons
/// are the whole content of the function.
#[must_use]
pub const fn claim_check_route_for(kind: ProtocolPositionOwnerKindV2) -> ClaimCheckRouteV1 {
    match kind {
        // A wallet. It signs, so the native record can name it and be opened.
        ProtocolPositionOwnerKindV2::User => ClaimCheckRouteV1::Native,
        // Trading-owned resting inventory, and the Fractional reserve. The
        // address cannot sign, so it can never be a payee -- but the collateral
        // it holds backs every outstanding shard of one coordinate, and those
        // shards have holders who can sign for themselves. The record is
        // addressed by the Mint, so no payee is named and none has to sign.
        ProtocolPositionOwnerKindV2::TradingRecord => ClaimCheckRouteV1::Fractional,
        // A Claims capability PDA. Structurally the same shape -- its claimants
        // hold a Mint too -- but a rational representation's Mint, whose terms
        // live in a different family. The fractional record's two numbers,
        // `denominator` and `payout_per_claim`, come from the Fractional
        // exposure terms specifically, and nothing authors them for a rational
        // representation. Debt, named rather than absolved: this owner kind
        // still has no compaction route, and a market carrying one still cannot
        // retire past a sleeping holder.
        ProtocolPositionOwnerKindV2::ClaimsCapability => ClaimCheckRouteV1::Neither,
    }
}

/// Whether a Position's owner kind may be compacted into a fractional record.
///
/// The mirror of `claim_check_compaction_v1::owner_kind_can_open_a_claim_check`,
/// reading the same routing so the two can never both admit one kind.
#[must_use]
pub const fn owner_kind_may_open_a_fractional_claim_check(
    kind: ProtocolPositionOwnerKindV2,
) -> bool {
    matches!(claim_check_route_for(kind), ClaimCheckRouteV1::Fractional)
}

// ---------------------------------------------------------------------------
// The route
// ---------------------------------------------------------------------------

/// Coordinates this route borrows from the wrapped terminal frame.
///
/// The terminal frame's roles, order and privileges belong to
/// [`crate::terminal_settlement_v3`] and are never re-enumerated here -- the
/// frame declaration says so and the payout derivation is *called* for the same
/// reason. These four are the accounts this route reads for its own purposes,
/// named because `dclutch-claims-svm` exports constants for the collateral
/// coordinates (hoard, recipient) and not for these. The Position's index has
/// one author in [`crate::claim_check_compaction_v1`] and is imported from
/// there rather than restated.
mod terminal {
    /// The Claims aggregate: the escrow's and the record's sole shared seed.
    pub(super) const AGGREGATE: usize = 1;
    /// The Rent sysvar the terminal frame already carries.
    pub(super) const RENT_SYSVAR: usize = 10;
    /// The Registry program owning every finalized record pair.
    pub(super) const REGISTRY: usize = 13;
    /// The collateral mint the escrow's vault is denominated in.
    pub(super) const COLLATERAL_MINT: usize = 31;
}

/// Everything one fractional compaction proves before it is entitled to turn.
struct FractionalCompactionPreparedV1 {
    aggregate: [u8; 32],
    escrow: ClaimCheckEscrowV1,
    vault: Pubkey,
    record_seeds: FractionalClaimCheckSeedsV1,
    record_bump: u8,
    shard_mint: [u8; 32],
    denominator: u64,
    shard_supply: u64,
    payout_per_claim: u64,
    representation_coordinate: u32,
}

/// Observed collateral either side of the payout this crank performed.
#[derive(Clone, Copy)]
struct FractionalCollateralMovementV1 {
    hoard_before: u64,
    hoard_after: u64,
    vault_before: u64,
    vault_after: u64,
}

/// Resolve one admitted role's absolute index in the frame.
///
/// A refused role has no index, which is what makes an accidental reach
/// impossible to spell: there is no number to write down for an account this
/// route must never touch. `TradingCallerAuthority` is the live case -- design
/// §17.8 ruling 2 dropped it, and this function is why dropping it cannot be
/// undone by a stray literal.
fn role_index(role: FractionalCompactionRoleV1) -> Result<usize, ProgramError> {
    role.index()
        .ok_or_else(|| FractionalClaimCheckCompactionSbfErrorV1::Accounts.into())
}

/// Borrow one admitted role's account, with the privileges the frame declares.
///
/// The privileges are read from [`FractionalCompactionRoleV1::privileges`]
/// rather than written out again here, so the declaration is load-bearing
/// instead of documentary. Equality on both halves, not implication: an account
/// arriving *more* privileged than declared is refused too, because a writable
/// account this route only reads is a lock somebody else could have needed and
/// a signature nobody checked.
fn role_account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    role: FractionalCompactionRoleV1,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    let account = accounts
        .get(role_index(role)?)
        .ok_or(FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let (signer, writable) = role.privileges();
    if account.is_signer != signer {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Authority.into());
    }
    if account.is_writable != writable {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Accounts.into());
    }
    Ok(account)
}

/// Compact one sleeping Fractional reserve into a record its shard holders own.
///
/// The fractional half of the crank the compaction design exists to make
/// possible, and it differs from its native sibling
/// ([`crate::claim_check_compaction_v1::process_compaction`]) in exactly one
/// fact: the Position's owner is a Trading PDA that can never sign for a payout,
/// while the claimants on the collateral it holds are the holders of a shard
/// Mint, who can sign for themselves. So the record this mints is addressed by
/// the *instrument* rather than by a payee, and names no payee at all.
///
/// **What is called rather than re-implemented**, because a second author for
/// any of these is how two routes come to pay two different numbers:
///
/// - the payout, by
///   [`crate::terminal_settlement_v3::execute_claim_check_compaction`] -- the
///   holder's own redemption with one proof relaxed at coordinate 0;
/// - the lamport sweep and its amended order, by
///   [`crate::claim_check_compaction_v1::close_and_split`], shared unmodified
///   through [`FractionalClaimCheckCompactionPlanV1::shared`];
/// - the conservation, by [`FractionalClaimCheckCompactionPlanV1`], which
///   embeds the native plan rather than restating it.
///
/// **Trading-composed means composed for signature, not for authority** (design
/// §17.8). What authorizes this crank is the elapsed deadline and the records --
/// the same authorizer as the native sibling, which requires nothing from
/// Trading. The one thing Trading supplies is the capability root's signature,
/// spent on the one act no other key can perform: `SetAuthority`
/// (`PermissionedBurn`), root -> escrow, while the root is still alive.
pub fn process_fractional_compaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Accounts.into());
    }
    let request = decode_compaction_request(instruction_data)?;
    // The digest of the EXACT bytes this route was handed, which is what binds
    // the receipt to the parent's request and nothing else. Hashed here, over
    // the whole instruction data, because that is what the parent hashed -- and
    // it is why this route reads no receipt suffix (`VerifiedOnly`): a byte
    // appended after the request would change this digest and the child would
    // refuse.
    let request_digest = hash(instruction_data).to_bytes();
    let prepared = authenticate_fractional_compaction(program_id, accounts, &request)?;
    let terminal = accounts
        .get(..FRACTIONAL_COMPACT_TERMINAL_FRAME_V1)
        .ok_or(FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let vault_account = accounts
        .get(TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3)
        .ok_or(FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let hoard_account = accounts
        .get(TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3)
        .ok_or(FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let vault_before = token_balance(vault_account)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let hoard_before = token_balance(hoard_account)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;

    // CALLED, never re-implemented. Everything the holder's own redemption
    // authenticates, this authenticates, because it is the same code -- and a
    // fractional compaction that re-derived the payout could pay a different
    // number than the redemption it stands in for and still pass its own tests.
    crate::terminal_settlement_v3::execute_claim_check_compaction(
        program_id,
        terminal,
        request.settlement(),
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Economic)?;

    commit_fractional_compaction(
        program_id,
        accounts,
        prepared.as_ref(),
        &request,
        request_digest,
        FractionalCollateralMovementV1 {
            hoard_before,
            hoard_after: token_balance(hoard_account)
                .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?,
            vault_before,
            vault_after: token_balance(vault_account)
                .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?,
        },
    )
}

/// Decode the 744-byte compaction wire, off its caller's frame.
///
/// **`#[inline(never)]` and the `Box` are both load-bearing, and the reason is
/// measured rather than assumed** (design §17.6, confirmed a second time in
/// §17.9). This request is 744 bytes and decodes into a struct embedding the
/// whole `TerminalSettlementRequestV3`. Written inline it puts that struct on
/// its caller's frame: FRACCHECK-3 watched the identical wire move
/// `route_authority` from 3,072 to 3,712 bytes, and `cargo build-sbf` reports a
/// frame under 4,096 exactly as it reports one far below it, so the growth is
/// invisible to the gate CI runs. The Claims link's deepest function sits at
/// 3,776 of 4,096 -- 320 bytes of headroom for a 744-byte struct.
///
/// Measure with `tools/sbf-frame-sizes.py`; never trust a zero-diagnostic build.
#[inline(never)]
fn decode_compaction_request(
    instruction_data: &[u8],
) -> Result<Box<FractionalCompactToClaimCheckRequestV1>, ProgramError> {
    FractionalCompactToClaimCheckRequestV1::decode(instruction_data)
        .map(Box::new)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Identity.into())
}

#[inline(never)]
fn authenticate_fractional_compaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalCompactToClaimCheckRequestV1,
) -> Result<Box<FractionalCompactionPreparedV1>, ProgramError> {
    let input = request.input();
    let coordinates = request.coordinates();
    let at = |index: usize| {
        accounts
            .get(index)
            .ok_or(FractionalClaimCheckCompactionSbfErrorV1::Accounts)
    };
    let escrow_account = role_account(accounts, FractionalCompactionRoleV1::Escrow)?;
    let record_account = role_account(
        accounts,
        FractionalCompactionRoleV1::FractionalClaimCheckRecord,
    )?;
    let admission_account = role_account(accounts, FractionalCompactionRoleV1::ReserveAdmission)?;
    let rent_credit_account = role_account(accounts, FractionalCompactionRoleV1::RentCredit)?;
    let system_account = role_account(accounts, FractionalCompactionRoleV1::SystemProgram)?;
    let root_account = role_account(
        accounts,
        FractionalCompactionRoleV1::FractionalCapabilityRoot,
    )?;
    let shard_mint_account = role_account(accounts, FractionalCompactionRoleV1::ShardMint)?;
    let shard_token_program =
        role_account(accounts, FractionalCompactionRoleV1::ShardTokenProgram)?;
    let terms_raw = role_account(accounts, FractionalCompactionRoleV1::ExposureTerms)?;
    let terms_staging = role_account(accounts, FractionalCompactionRoleV1::ExposureTermsStaging)?;
    let behavior_raw = role_account(accounts, FractionalCompactionRoleV1::TokenBehavior)?;
    let behavior_staging =
        role_account(accounts, FractionalCompactionRoleV1::TokenBehaviorStaging)?;
    // `Opener` is borrowed for its privileges only; the sweep credits it by
    // account rather than by identity, exactly as the native sibling does.
    role_account(accounts, FractionalCompactionRoleV1::Opener)?;

    if system_account.key != &system_program::ID || escrow_account.owner != program_id {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Accounts.into());
    }

    // EVERY SIGNER IN THE FRAME IS NAMED. Two, and only two, and the second is
    // the whole of what "Trading-composed" buys. Coordinate 0 is the cranker --
    // anybody, and required to sign because `(Claims, ClaimCheckCrank)` asks of
    // it only that somebody stood behind the transaction, which the terminal
    // derivation checks as its own. The capability root is the other. A third
    // presented signature is REFUSED rather than ignored: a route that merely
    // does not read a signature still lets a caller present one, and a presented
    // signature is a privilege somebody can be induced to grant.
    let root_index = role_index(FractionalCompactionRoleV1::FractionalCapabilityRoot)?;
    if accounts
        .iter()
        .enumerate()
        .any(|(index, account)| index != 0 && index != root_index && account.is_signer)
    {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Authority.into());
    }

    // The reserve Position's owner IS the root, and the root signs this frame.
    // That is what entitles this route's close, and it is why §17.8 ruling 2
    // could drop `TradingCallerAuthority`: the close is owner-signed, and the
    // owner's own signature is not a second job needing a second signer.
    if root_account.key.to_bytes() != request.root() {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Authority.into());
    }

    let aggregate = at(terminal::AGGREGATE)?.key.to_bytes();
    let escrow_seeds = ClaimCheckEscrowSeedsV1::new(aggregate)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Identity)?;
    if escrow_account.key != &Pubkey::find_program_address(&escrow_seeds.as_slices(), program_id).0
    {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    let escrow = ClaimCheckEscrowV1::decode(
        &escrow_account
            .try_borrow_data()
            .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?,
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Escrow)?;

    // The deadline, with checked arithmetic and the native's inclusive `>=`: a
    // wrapping add would turn a far-future origin into an already-elapsed one,
    // which is exactly the premature crank this gate refuses.
    let horizon = escrow
        .opened_slot
        .checked_add(COMPACTION_DEADLINE_SLOTS_V1)
        .ok_or(FractionalClaimCheckCompactionSbfErrorV1::Deadline)?;
    if solana_program::clock::Clock::get()?.slot < horizon {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Deadline.into());
    }

    // THE RECIPIENT IS DERIVED, NEVER ACCEPTED. The single check separating a
    // crank from a theft, and fractionally the stake is larger than natively:
    // one reserve backs an entire coordinate's outstanding supply rather than
    // one sleeper's payout.
    let vault_seeds = ClaimCheckVaultSeedsV1::new(aggregate)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Identity)?;
    let vault = Pubkey::find_program_address(&vault_seeds.as_slices(), program_id).0;
    request
        .require_escrow_recipient(escrow_account.key.to_bytes(), vault.to_bytes())
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Identity)?;
    if at(TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3)?.key != &vault
        || escrow.vault != vault.to_bytes()
        || escrow.collateral_mint != at(terminal::COLLATERAL_MINT)?.key.to_bytes()
        || escrow.market != input.market
        || escrow.release_set != input.release_set
        || escrow.generation != input.generation
    {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Escrow.into());
    }

    // The reserve Position's admission, and the owner-kind gate this whole
    // module exists to complete. `TradingRecord` is the one kind the native
    // weld had to refuse -- an unsignable payee -- and the one this route
    // admits, because the record it mints names no payee.
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(aggregate, input.owner)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Identity)?;
    if admission_account.key
        != &Pubkey::find_program_address(&admission_seeds.as_slices(), program_id).0
        || admission_account.owner != program_id
    {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    let admission = ProtocolPositionAdmissionV2::decode(
        &admission_account
            .try_borrow_data()
            .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?,
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Scope)?;
    if !owner_kind_may_open_a_fractional_claim_check(admission.owner_kind()) {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Scope.into());
    }

    // What this route can prove about the RentCredit, and what it cannot.
    //
    // The residual beneficiary is bound to THIS market: the record must decode
    // and must carry the market, release set and generation the request already
    // pinned everywhere else, so a caller naming their own wallet (which holds
    // no such record) or another market's credit is refused here. What is NOT
    // proved is the PDA derivation under the Rent program's id, because this
    // frame declares no Rent program account to derive it against --
    // `authenticate_rent_credit` needs one and retirement's frame carries it.
    // Adding a fiftieth account is a frame-declaration change and a design
    // decision, not a lane's; the residual is named as debt rather than
    // absolved. The native sibling checks strictly less here (writability only).
    let rent_credit_bytes = rent_credit_account
        .try_borrow_data()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let rent_credit = LifecycleRentCreditV2::decode(&rent_credit_bytes)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    if rent_credit.market().to_bytes() != input.market
        || rent_credit.release_set().to_bytes() != input.release_set
        || rent_credit.generation() != input.generation
    {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    drop(rent_credit_bytes);

    // The finalized terms: raw AND staging, together. The raw half must hash to
    // the expected digest and the staging cursor must be VACANT, which is what
    // proves nobody is part-way through replacing the bytes. On the account that
    // authors the denominator every holder's payout is divided by, carrying only
    // the raw half would leave a route that still looks authenticated and is
    // reading a number mid-rewrite (design §17.7 finding 1).
    let rent = Rent::from_account_info(at(terminal::RENT_SYSVAR)?)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let registry = at(terminal::REGISTRY)?;
    let terms_bytes = terms_raw
        .try_borrow_data()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        terms_raw,
        terms_staging,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        coordinates.terms,
        &terms_bytes,
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Terms)?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: coordinates.terms,
            finalized_terms_id: coordinates.terms,
            recomputed_terms_digest: coordinates.terms,
            finalized_terms_digest: coordinates.terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Terms)?;
    if terms.market() != input.market || terms.release_set() != input.release_set {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Terms.into());
    }
    let denominator = terms.denominator();
    let expected_mint = terms
        .shard_mint(coordinates.representation_coordinate)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Terms)?;
    let terms_token_program = terms.token_program();
    let terms_behavior = terms.token_behavior();
    drop(terms_bytes);

    // The behavior record selects the Token program the shard Mint must be
    // owned by, and the terms select the behavior record. Reading the program
    // from anywhere else would be a second author for the same fact.
    if terms_behavior != coordinates.token_behavior {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Terms.into());
    }
    let behavior_bytes = behavior_raw
        .try_borrow_data()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    authenticate_finalized_rational_record(
        registry.key,
        &rent,
        behavior_raw,
        behavior_staging,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        coordinates.token_behavior,
        &behavior_bytes,
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Terms)?;
    let behavior = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        &behavior_bytes,
        input.realm,
        input.release_set,
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::ShardMint)?;
    if behavior.token_program() != terms_token_program
        || shard_token_program.key.to_bytes() != terms_token_program
    {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::ShardMint.into());
    }
    drop(behavior_bytes);

    // The shard Mint, read BEFORE the hand-off, while its permissioned-burn
    // authority is still the root -- which is what `read_mint` requires and is
    // therefore also a proof that the hand-off has not already happened.
    //
    // `read_mint` and not `check_mint`, and this is the third caller class its
    // sibling's doc names rather than a hole in that discipline: the outstanding
    // shard supply IS the durable claim, and there is no independent expectation
    // of it to pin. Any holder's redemption lowers it between the moment a
    // request is built and the moment it lands, so a pinned supply would refuse
    // an honest compaction because somebody else redeemed first.
    if shard_mint_account.key.to_bytes() != expected_mint
        || shard_mint_account.owner != shard_token_program.key
    {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::ShardMint.into());
    }
    let mint_bytes = shard_mint_account
        .try_borrow_data()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
    let shard_supply = Token2022BehaviorProfileV2::read_mint(
        terms_token_program,
        expected_mint,
        &mint_bytes,
        root_account.key.to_bytes(),
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::ShardMint)?
    .base_supply();
    drop(mint_bytes);

    // The record's coordinates are the aggregate and the INSTRUMENT, so a caller
    // naming the wrong Mint derives an address that is not the account they
    // passed. There is no claimant field on any wire to forge.
    let record_seeds = FractionalClaimCheckSeedsV1::new(aggregate, expected_mint)
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Identity)?;
    let (expected_record, record_bump) =
        Pubkey::find_program_address(&record_seeds.as_slices(), program_id);
    if record_account.key != &expected_record {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::Identity.into());
    }
    // Anti-replay is the account's own existence: a fractional claim-check that
    // exists is a coordinate already compacted.
    if !record_account.data_is_empty() || record_account.owner != &system_program::ID {
        return Err(FractionalClaimCheckCompactionSbfErrorV1::AlreadyCompacted.into());
    }

    Ok(Box::new(FractionalCompactionPreparedV1 {
        aggregate,
        escrow,
        vault,
        record_seeds,
        record_bump,
        shard_mint: expected_mint,
        denominator,
        shard_supply,
        payout_per_claim: coordinates.payout_per_claim,
        representation_coordinate: coordinates.representation_coordinate,
    }))
}

#[inline(never)]
fn commit_fractional_compaction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    prepared: &FractionalCompactionPreparedV1,
    request: &FractionalCompactToClaimCheckRequestV1,
    request_digest: [u8; 32],
    movement: FractionalCollateralMovementV1,
) -> ProgramResult {
    let at = |index: usize| {
        accounts
            .get(index)
            .ok_or(FractionalClaimCheckCompactionSbfErrorV1::Accounts)
    };
    let escrow_account = at(role_index(FractionalCompactionRoleV1::Escrow)?)?;
    let record_account = at(role_index(
        FractionalCompactionRoleV1::FractionalClaimCheckRecord,
    )?)?;
    let admission_account = at(role_index(FractionalCompactionRoleV1::ReserveAdmission)?)?;
    let rent_credit_account = at(role_index(FractionalCompactionRoleV1::RentCredit)?)?;
    let opener_account = at(role_index(FractionalCompactionRoleV1::Opener)?)?;
    let system_account = at(role_index(FractionalCompactionRoleV1::SystemProgram)?)?;
    let root_account = at(role_index(
        FractionalCompactionRoleV1::FractionalCapabilityRoot,
    )?)?;
    let shard_mint_account = at(role_index(FractionalCompactionRoleV1::ShardMint)?)?;
    let shard_token_program = at(role_index(FractionalCompactionRoleV1::ShardTokenProgram)?)?;
    let position_account = at(COMPACT_TERMINAL_POSITION_ACCOUNT_V1)?;
    let cranker_account = at(0)?;
    let escrow = prepared.escrow;
    let FractionalCollateralMovementV1 {
        hoard_before,
        hoard_after,
        vault_before,
        vault_after,
    } = movement;

    let rent = Rent::get()?;
    let mints_record = vault_after > vault_before;
    let record_rent = if mints_record {
        rent.minimum_balance(FRACTIONAL_CLAIM_CHECK_BYTES_V1)
    } else {
        0
    };
    let plan =
        FractionalClaimCheckCompactionPlanV1::new(FractionalClaimCheckCompactionObservationV1 {
            shared: ClaimCheckCompactionObservationV1 {
                payout_atoms: hoard_before.saturating_sub(hoard_after),
                hoard_before,
                hoard_after,
                vault_before,
                vault_after,
                position: observation(position_account),
                admission: observation(admission_account),
                claim_check: observation(record_account),
                cranker: observation(cranker_account),
                opener: observation(opener_account),
                rent_credit: observation(rent_credit_account),
                claim_check_rent: record_rent,
                opener_debt: escrow.opener_outlay,
                crank_reward_cap: COMPACTION_CRANK_REWARD_LAMPORTS_V1,
            },
            denominator: prepared.denominator,
            payout_per_claim: prepared.payout_per_claim,
            shard_supply: prepared.shard_supply,
        })
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Conservation)?;

    if plan.mints_claim_check() {
        // THE HAND-OFF, and it happens here or it never happens. Token-2022
        // refuses `SetAuthority(PermissionedBurn)` without the CURRENT
        // authority's signature; that authority is the capability root, nothing
        // but a Trading `invoke_signed` can produce its signature, and nothing
        // can produce it once the market retires. `invoke` and not
        // `invoke_signed`: Claims cannot sign for a Trading PDA, and does not
        // need to -- the signature is already on this frame, propagated by the
        // parent that authenticated the root's bytes before marking it a signer.
        //
        // Welded to `mints_claim_check` in both directions, exactly as the
        // record write is. A coordinate that escrows nothing mints no record,
        // so no holder will ever burn against this Mint through the escrow, and
        // moving a live authority to serve a claim that does not exist would be
        // an authority moved for nothing.
        let hand_off = token_instruction::set_authority(
            shard_token_program.key,
            shard_mint_account.key,
            Some(escrow_account.key),
            token_instruction::AuthorityType::PermissionedBurn,
            root_account.key,
            &[],
        )
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::ShardMint)?;
        invoke(
            &hand_off,
            &[
                shard_mint_account.clone(),
                root_account.clone(),
                shard_token_program.clone(),
            ],
        )
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Authority)?;

        // The post-hand-off re-read, and it is not ceremony: the two entry
        // points are DISJOINT over all Mint bytes, so this admitting is proof
        // the `SetAuthority` actually landed and the burn authority is now the
        // escrow. A Mint the hand-off never touched is a refusal here rather
        // than a pass. The supply is re-checked equal because nothing in this
        // transaction may have moved it.
        let mint_bytes = shard_mint_account
            .try_borrow_data()
            .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?;
        let handed_over = Token2022BehaviorProfileV2::read_compacted_shard_mint(
            shard_token_program.key.to_bytes(),
            prepared.shard_mint,
            &mint_bytes,
            root_account.key.to_bytes(),
            escrow_account.key.to_bytes(),
        )
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::ShardMint)?;
        if handed_over.base_supply() != prepared.shard_supply {
            return Err(FractionalClaimCheckCompactionSbfErrorV1::ShardMint.into());
        }
        drop(mint_bytes);

        write_fractional_claim_check(
            program_id,
            record_account,
            system_account,
            &prepared.record_seeds,
            prepared.record_bump,
            FractionalClaimCheckV1 {
                aggregate: prepared.aggregate,
                shard_mint: prepared.shard_mint,
                market: escrow.market,
                release_set: escrow.release_set,
                vault: prepared.vault.to_bytes(),
                collateral_mint: escrow.collateral_mint,
                position_atoms_digest: hash(
                    &position_account
                        .try_borrow_data()
                        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?,
                )
                .to_bytes(),
                escrowed_atoms: plan.escrowed_atoms(),
                denominator: prepared.denominator,
                payout_per_claim: prepared.payout_per_claim,
                compacted_shard_supply: prepared.shard_supply,
                compacted_slot: solana_program::clock::Clock::get()?.slot,
                generation: escrow.generation,
                representation_coordinate: prepared.representation_coordinate,
                bump: prepared.record_bump,
            },
            plan.shared().claim_check_top_up(),
        )?;
    }

    // The sweep, SHARED rather than restated: rent first because it is
    // mandatory, then the crank, then the opener's debt, then the residue.
    // `close_and_split` takes the native plan by reference and `shared()`
    // returns exactly that type by value, so the amended four-credit order has
    // ONE author across both routes (design §17.9's correction to hazard 2).
    // Its refusals are the native `0x5600` band, which is a visible consequence
    // in a validator log and is accepted deliberately: it *is* the native close.
    close_and_split(
        position_account,
        admission_account,
        cranker_account,
        opener_account,
        rent_credit_account,
        &plan.shared(),
    )?;

    let mut updated = ClaimCheckEscrowV1 {
        opener_outlay: plan.shared().opener_debt_after(),
        ..escrow
    };
    if plan.mints_claim_check() {
        updated = updated
            .admit_claim_check()
            .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Escrow)?;
    }
    escrow_account
        .try_borrow_mut_data()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?
        .copy_from_slice(
            &updated
                .to_bytes()
                .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Escrow)?,
        );

    plan.validate_post(
        dclutch_claims_svm::claim_check_conservation_v1::ClaimCheckCompactionPostV1 {
            position_lamports: position_account.lamports(),
            admission_lamports: admission_account.lamports(),
            claim_check_lamports: record_account.lamports(),
            cranker_lamports: cranker_account.lamports(),
            opener_lamports: opener_account.lamports(),
            rent_credit_lamports: rent_credit_account.lamports(),
            hoard_lamports_of_collateral: hoard_after,
            vault_lamports_of_collateral: vault_after,
        },
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Receipt)?;

    // The evidence, emitted last and only once everything above has moved. A
    // receipt written before the acts it describes would be a promise; written
    // here it is a report, and the conservation plan has already refused every
    // way the acts could have gone differently.
    let receipt = FractionalClaimCheckCompactionReceiptV1::new(
        request,
        FractionalClaimCheckCompactionReceiptInputV1 {
            request_digest,
            aggregate: prepared.aggregate,
            shard_mint: prepared.shard_mint,
            escrow: escrow_account.key.to_bytes(),
            record: record_account.key.to_bytes(),
            escrowed_atoms: plan.escrowed_atoms(),
            denominator: prepared.denominator,
            payout_per_claim: prepared.payout_per_claim,
            compacted_shard_supply: prepared.shard_supply,
            minted: plan.mints_claim_check(),
        },
    )
    .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Receipt)?;
    set_return_data(
        &receipt
            .to_bytes()
            .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Receipt)?,
    );
    Ok(())
}

/// Allocate, fund and write one 320-byte fractional claim-check.
///
/// **Its own writer, and deliberately not a widened `write_claim_check`**
/// (design §17.9's correction to hazard 2). That function is hard-typed to
/// `ClaimCheckSeedsV1`, `ClaimCheckV1` and the native `CLAIM_CHECK_BYTES_V1` of
/// **288**; this record is `FRACTIONAL_CLAIM_CHECK_BYTES_V1` of **320** under
/// `FractionalClaimCheckSeedsV1`. Both seed types return `[&[u8]; 3]`, so the
/// body shape matches and widening looks tempting -- but a writer generic over
/// two record types and two widths is a function whose callers can pass the
/// wrong pairing. The honest share is [`allocate_and_assign`], which
/// `write_claim_check` already delegates to and which this calls; re-deriving
/// the seed order here is what would have been duplication.
fn write_fractional_claim_check<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    seeds: &FractionalClaimCheckSeedsV1,
    bump: u8,
    record: FractionalClaimCheckV1,
    top_up: u64,
) -> Result<(), ProgramError> {
    let record = record
        .new()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Conservation)?;
    let bump_seed = [bump];
    let [domain, aggregate, shard_mint] = seeds.as_slices();
    allocate_and_assign(
        account,
        system,
        FRACTIONAL_CLAIM_CHECK_BYTES_V1,
        program_id,
        &[domain, aggregate, shard_mint, &bump_seed],
    )?;
    // Credited directly rather than transferred, because the lamports come from
    // accounts this program owns and is about to close: a System transfer cannot
    // move them, their owner is Claims and not System.
    **account
        .try_borrow_mut_lamports()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)? += top_up;
    account
        .try_borrow_mut_data()
        .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Accounts)?
        .copy_from_slice(
            &record
                .to_bytes()
                .map_err(|_| FractionalClaimCheckCompactionSbfErrorV1::Conservation)?,
        );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_check_compaction_v1::ClaimCheckCompactionSbfErrorV1;
    use crate::claim_check_redemption_v1::ClaimCheckRedemptionSbfErrorV1;
    use dclutch_claims_svm::claim_check_v1::{
        CLAIM_CHECK_ESCROW_SEED_V1, CLAIM_CHECK_SEED_V1, CLAIM_CHECK_VAULT_SEED_V1,
    };
    use dclutch_claims_svm::fractional_claim_check_v1::{
        FRACTIONAL_CLAIM_CHECK_SEED_V1, FractionalClaimCheckSeedsV1, MAX_PDA_SEED_BYTES_V1,
    };
    use solana_program::pubkey::{Pubkey, PubkeyError};

    const KINDS: [ProtocolPositionOwnerKindV2; 3] = [
        ProtocolPositionOwnerKindV2::TradingRecord,
        ProtocolPositionOwnerKindV2::User,
        ProtocolPositionOwnerKindV2::ClaimsCapability,
    ];

    /// The dispatcher routes on a magic prefix, so the magics must not shadow.
    ///
    /// `process_remaining_instruction` compares a leading slice of the
    /// instruction data against each family magic in written order, and returns
    /// on the first match. That is only sound if no magic is a prefix of another
    /// and no two are equal -- otherwise the arm written first would swallow the
    /// other family's traffic, and the swallowed route would be unreachable with
    /// nothing to say so. The fractional compaction magic (`DCLTFCC1`) joined
    /// that comparison beside the native one (`DCLTCCC1`), which differ in one
    /// byte at index 5, so the property is worth pinning rather than eyeballing.
    ///
    /// Stated over every ORDERED pair, because "is a prefix of" is not
    /// symmetric: checking each unordered pair once would test one direction and
    /// leave the other to luck.
    #[test]
    fn no_claim_check_family_magic_shadows_another_in_the_dispatcher() {
        use dclutch_claims_svm::claim_check_v1::{
            CLAIM_CHECK_COMPACT_MAGIC_V1, CLAIM_CHECK_ESCROW_MAGIC_V1, CLAIM_CHECK_OPEN_MAGIC_V1,
            CLAIM_CHECK_RECORD_MAGIC_V1, CLAIM_CHECK_REDEEM_MAGIC_V1,
        };
        use dclutch_claims_svm::fractional_claim_check_compaction_receipt_v1::FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_MAGIC_V1;
        use dclutch_claims_svm::fractional_claim_check_v1::{
            FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1, FRACTIONAL_CLAIM_CHECK_RECORD_MAGIC_V1,
            FRACTIONAL_CLAIM_CHECK_REDEEM_MAGIC_V1,
        };

        let family: [(&str, [u8; 8]); 8] = [
            ("native open", CLAIM_CHECK_OPEN_MAGIC_V1),
            ("native compact", CLAIM_CHECK_COMPACT_MAGIC_V1),
            ("native redeem", CLAIM_CHECK_REDEEM_MAGIC_V1),
            ("native record", CLAIM_CHECK_RECORD_MAGIC_V1),
            ("native escrow", CLAIM_CHECK_ESCROW_MAGIC_V1),
            (
                "fractional compact",
                FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1,
            ),
            ("fractional record", FRACTIONAL_CLAIM_CHECK_RECORD_MAGIC_V1),
            ("fractional redeem", FRACTIONAL_CLAIM_CHECK_REDEEM_MAGIC_V1),
        ];
        for (left_name, left) in family {
            for (right_name, right) in family {
                if left_name == right_name {
                    continue;
                }
                assert_ne!(
                    left, right,
                    "{left_name} and {right_name} share a magic; the dispatcher would route both to whichever arm is written first"
                );
                assert!(
                    !left.starts_with(&right),
                    "{right_name} is a prefix of {left_name}; a prefix match would swallow it"
                );
            }
        }
        // And the receipt magic is not an instruction magic at all. It travels
        // as return data, never as instruction input -- but it lives in the same
        // family and a later reader could reach for it, so it is held distinct
        // from every route magic here rather than by nobody.
        for (name, magic) in family {
            assert_ne!(
                magic, FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_MAGIC_V1,
                "the compaction receipt magic must not collide with {name}"
            );
        }
    }

    #[test]
    fn every_code_is_contiguous_and_unique_within_its_sub_band() {
        for (index, code) in FractionalClaimCheckCompactionSbfErrorV1::ALL
            .iter()
            .enumerate()
        {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x640 + index as u32;
            assert_eq!(*code as u32, expected);
            assert!(
                !FractionalClaimCheckCompactionSbfErrorV1::ALL
                    .iter()
                    .skip(index + 1)
                    .any(|other| other == code)
            );
        }
        for (index, code) in FractionalClaimCheckRedemptionSbfErrorV1::ALL
            .iter()
            .enumerate()
        {
            let expected = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x660 + index as u32;
            assert_eq!(*code as u32, expected);
            assert!(
                !FractionalClaimCheckRedemptionSbfErrorV1::ALL
                    .iter()
                    .skip(index + 1)
                    .any(|other| other == code)
            );
        }
    }

    #[test]
    fn the_four_claim_check_sub_bands_are_ordered_and_never_overlap() {
        // Native compaction 0x600, native redemption 0x620, fractional
        // compaction 0x640, fractional redemption 0x660. The gaps are deliberate
        // room for any one family to grow without the others renumbering, which
        // would break every hostile test naming a literal.
        let boundaries = [
            ClaimCheckCompactionSbfErrorV1::Accounts as u32,
            ClaimCheckRedemptionSbfErrorV1::Accounts as u32,
            FractionalClaimCheckCompactionSbfErrorV1::Accounts as u32,
            FractionalClaimCheckRedemptionSbfErrorV1::Accounts as u32,
        ];
        for (index, base) in boundaries.iter().enumerate() {
            if let Some(next) = boundaries.get(index + 1) {
                assert!(base < next, "sub-band bases must ascend");
            }
        }
        assert!(
            (ClaimCheckCompactionSbfErrorV1::Scope as u32)
                < ClaimCheckRedemptionSbfErrorV1::Accounts as u32
        );
        assert!(
            (ClaimCheckRedemptionSbfErrorV1::Vault as u32)
                < FractionalClaimCheckCompactionSbfErrorV1::Accounts as u32
        );
        assert!(
            (FractionalClaimCheckCompactionSbfErrorV1::ShardMint as u32)
                < FractionalClaimCheckRedemptionSbfErrorV1::Accounts as u32
        );
        // And every occupied Claims sub-band predating this lane still sits
        // below all four, so an addition to one of those that ran 0x100 wide
        // would be caught here.
        for occupied in [
            0x000_u32, 0x100, 0x140, 0x160, 0x180, 0x200, 0x210, 0x260, 0x500,
        ] {
            let base = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + occupied;
            for code in FractionalClaimCheckCompactionSbfErrorV1::ALL {
                assert!(code as u32 > base);
            }
            for code in FractionalClaimCheckRedemptionSbfErrorV1::ALL {
                assert!(code as u32 > base);
            }
        }
    }

    #[test]
    fn a_refusal_reaches_the_runtime_as_the_literal_a_log_line_shows() {
        assert_eq!(
            ProgramError::from(FractionalClaimCheckCompactionSbfErrorV1::Scope),
            ProgramError::Custom(0x564A)
        );
        assert_eq!(
            ProgramError::from(FractionalClaimCheckCompactionSbfErrorV1::ShardMint),
            ProgramError::Custom(0x564C)
        );
        assert_eq!(
            ProgramError::from(FractionalClaimCheckRedemptionSbfErrorV1::NoWholeClaim),
            ProgramError::Custom(0x5665)
        );
    }

    #[test]
    fn no_owner_kind_is_admitted_by_both_gates() {
        // The weld's own property, extended. Refusing `TradingRecord` at the
        // native gate was the fix; admitting it at the fractional gate is the
        // completion. What must never happen is both, because a Position
        // compacted twice would resolve one pot of collateral into two records.
        for kind in KINDS {
            let native = crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(kind);
            let fractional = owner_kind_may_open_a_fractional_claim_check(kind);
            assert!(
                !(native && fractional),
                "{kind:?} may not be entitled to two claim-checks for one Position"
            );
        }
    }

    #[test]
    fn the_native_gate_is_not_relaxed_by_this_lane() {
        // Stated against the shipped function rather than restated, so a future
        // edit to the weld has to argue with this test. `TradingRecord` is the
        // Fractional reserve: giving it a native record would mint collateral to
        // an address that can never sign, which is the exposure the weld closed.
        assert!(
            crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::User
            )
        );
        assert!(
            !crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::TradingRecord
            )
        );
        assert!(
            !crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::ClaimsCapability
            )
        );
    }

    #[test]
    fn the_pda_claimant_design_is_what_admits_the_kind_the_weld_refuses() {
        // The whole argument of this lane, as an assertion. The native gate asks
        // "can this address sign for its own payout" and `TradingRecord` answers
        // no, permanently. The fractional record never asks: it names no payee,
        // because its address IS the instrument. So the one kind the weld had to
        // refuse is exactly the one this route admits, and neither gate had to
        // move.
        assert_eq!(
            claim_check_route_for(ProtocolPositionOwnerKindV2::TradingRecord),
            ClaimCheckRouteV1::Fractional
        );
        assert!(owner_kind_may_open_a_fractional_claim_check(
            ProtocolPositionOwnerKindV2::TradingRecord
        ));
        assert!(
            !crate::claim_check_compaction_v1::owner_kind_can_open_a_claim_check(
                ProtocolPositionOwnerKindV2::TradingRecord
            )
        );
    }

    #[test]
    fn every_owner_kind_is_routed_and_exactly_one_is_still_stranded() {
        // The routing is total, and the coverage is stated as a count so the
        // named debt cannot quietly grow or quietly vanish.
        let native = KINDS
            .into_iter()
            .filter(|kind| matches!(claim_check_route_for(*kind), ClaimCheckRouteV1::Native))
            .count();
        let fractional = KINDS
            .into_iter()
            .filter(|kind| matches!(claim_check_route_for(*kind), ClaimCheckRouteV1::Fractional))
            .count();
        let neither = KINDS
            .into_iter()
            .filter(|kind| matches!(claim_check_route_for(*kind), ClaimCheckRouteV1::Neither))
            .count();
        assert_eq!(native, 1);
        assert_eq!(fractional, 1);
        // `ClaimsCapability`. Its claimants hold a rational representation's
        // Mint, whose terms author neither of this record's two numbers. Still
        // open, still named in the census.
        assert_eq!(neither, 1);
        assert_eq!(native + fractional + neither, KINDS.len());
        assert_eq!(
            claim_check_route_for(ProtocolPositionOwnerKindV2::ClaimsCapability),
            ClaimCheckRouteV1::Neither
        );
    }

    #[test]
    fn the_fractional_gate_agrees_with_the_routing_and_reads_no_second_table() {
        for kind in KINDS {
            assert_eq!(
                owner_kind_may_open_a_fractional_claim_check(kind),
                matches!(claim_check_route_for(kind), ClaimCheckRouteV1::Fractional),
                "{kind:?} must be admitted by the routing and by nothing else"
            );
        }
    }

    /// The record's address derives, which it could not until 2026-08-31.
    ///
    /// `FRACTIONAL_CLAIM_CHECK_SEED_V1` shipped at 33 bytes — one over Solana's
    /// per-seed maximum — so `create_program_address` refused
    /// `MaxSeedLengthExceeded` for all 255 bumps, `find_program_address` found
    /// none, and every route naming this record would have **panicked** rather
    /// than refused. Nothing caught it because nothing had ever derived it:
    /// `dclutch-claims-svm` is `no_std` with no SDK dependency, so its own tests
    /// cannot call this. This crate can, and does.
    ///
    /// **The test discriminates**, which is the part that matters: the same call
    /// on the exact 33-byte string it used to hold returns `None`. A derivation
    /// test that could not tell the old value from the new one would pass either
    /// way and prove nothing about the fix.
    #[test]
    fn the_record_address_derives_and_the_old_domain_still_could_not() {
        let seeds = FractionalClaimCheckSeedsV1::new([3; 32], [4; 32]).expect("coordinates");
        let program = Pubkey::new_from_array([5; 32]);

        let (address, bump) = Pubkey::try_find_program_address(&seeds.as_slices(), &program)
            .expect("the fractional claim-check address must derive");
        assert_ne!(address, Pubkey::default());
        // And the bump round-trips: the address the record persists a bump for
        // is the one `create_program_address` reproduces from that bump.
        let with_bump: [&[u8]; 4] = [
            FRACTIONAL_CLAIM_CHECK_SEED_V1,
            &seeds.aggregate(),
            &seeds.shard_mint(),
            core::slice::from_ref(&bump),
        ];
        assert_eq!(
            Pubkey::create_program_address(&with_bump, &program).expect("canonical bump"),
            address
        );

        // The exact string this domain held until it was shortened. Restated as
        // a literal rather than reconstructed, so this stays a statement about
        // the value that actually shipped.
        const SHIPPED_UNDERIVABLE: &[u8] = b"dclutch:fractional-claim-check:v1";
        assert_eq!(SHIPPED_UNDERIVABLE.len(), 33);
        assert_eq!(
            Pubkey::try_find_program_address(
                &[SHIPPED_UNDERIVABLE, &seeds.aggregate(), &seeds.shard_mint()],
                &program,
            ),
            None,
            "the 33-byte domain has no derivable address for any bump"
        );
        assert_eq!(
            Pubkey::create_program_address(
                &[
                    SHIPPED_UNDERIVABLE,
                    &seeds.aggregate(),
                    &seeds.shard_mint(),
                    &[255]
                ],
                &program,
            ),
            Err(PubkeyError::MaxSeedLengthExceeded),
            "and the reason is the seed length, not an unlucky bump"
        );

        // Every domain this family derives with is inside the maximum, so the
        // fix is the family's and not one constant's.
        for domain in [
            FRACTIONAL_CLAIM_CHECK_SEED_V1,
            CLAIM_CHECK_SEED_V1,
            CLAIM_CHECK_ESCROW_SEED_V1,
            CLAIM_CHECK_VAULT_SEED_V1,
        ] {
            assert!(domain.len() <= MAX_PDA_SEED_BYTES_V1);
        }
    }
}

/// Frame-guard witnesses, driven against real `AccountInfo`s.
///
/// These run the shipped entry point over a synthetic 49-account frame. They do
/// not and cannot prove the payout, the hand-off, the record write or
/// conservation -- those need a market, real Token-2022 bytes and a Custody
/// composition, which is the campaign's job. What they DO prove is the security
/// surface that sits in front of all of it: which frames the route admits, and
/// which it refuses before it has read a single account's data.
///
/// The frame walk is what makes this reachable without a market. Every refusal
/// below fires inside `role_account` or the signer sweep, both of which run
/// before any derivation, any decode and any CPI -- so a synthetic frame is
/// sufficient evidence for exactly these properties and no others. That
/// boundary is stated here so nobody later reads these as the campaign.
#[cfg(test)]
mod frame_guard_tests {
    use super::*;
    use dclutch_claims_svm::CallerRole;
    use dclutch_claims_svm::fractional_claim_check_compaction_request_v1::{
        FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1, FractionalCompactionCoordinatesV1,
    };
    use dclutch_claims_svm::terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TerminalSettlementRequestInputV3,
        TerminalSettlementRequestV3,
    };
    use solana_program::account_info::AccountInfo;

    const PROGRAM: Pubkey = Pubkey::new_from_array([9; 32]);
    const ESCROW: [u8; 32] = [40; 32];
    const VAULT: [u8; 32] = [41; 32];
    const ROOT: [u8; 32] = [77; 32];

    fn account(key: Pubkey, owner: Pubkey, signer: bool, writable: bool) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(1_u64)),
            Box::leak(Vec::new().into_boxed_slice()),
            Box::leak(Box::new(owner)),
            false,
        )
    }

    fn request_bytes() -> Vec<u8> {
        let settlement = TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
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
        })
        .expect("settlement");
        FractionalCompactToClaimCheckRequestV1::new(
            FractionalCompactionCoordinatesV1 {
                terms: [20; 32],
                token_behavior: [21; 32],
                expected_root_revision: 11,
                representation_coordinate: 6,
                payout_per_claim: 4_000,
            },
            settlement,
        )
        .expect("request")
        .to_bytes()
        .expect("bytes")
        .to_vec()
    }

    /// A frame whose thirteen roles carry exactly the privileges declared.
    ///
    /// Built by asking [`FractionalCompactionRoleV1::privileges`] rather than by
    /// writing the flags out again, so the fixture cannot drift away from the
    /// declaration the route enforces -- if it did, these tests would be
    /// checking the route against a second opinion instead of against the frame.
    fn admitted_frame() -> Vec<AccountInfo<'static>> {
        let mut accounts = Vec::with_capacity(FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1);
        // The wrapped terminal frame. Coordinate 0 is the cranker, who signs
        // because `(Claims, ClaimCheckCrank)` asks that somebody did.
        for index in 0..TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 {
            let key = Pubkey::new_from_array([u8::try_from(index).expect("index") + 100; 32]);
            accounts.push(account(key, PROGRAM, index == 0, index == 0));
        }
        for (position, role) in FractionalCompactionRoleV1::frame().into_iter().enumerate() {
            let (signer, writable) = role.privileges();
            let key = match role {
                FractionalCompactionRoleV1::SystemProgram => system_program::ID,
                FractionalCompactionRoleV1::FractionalCapabilityRoot => {
                    Pubkey::new_from_array(ROOT)
                }
                _ => Pubkey::new_from_array([u8::try_from(position).expect("position") + 200; 32]),
            };
            accounts.push(account(key, PROGRAM, signer, writable));
        }
        assert_eq!(accounts.len(), FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1);
        accounts
    }

    fn refusal(accounts: &[AccountInfo<'_>]) -> ProgramError {
        process_fractional_compaction(&PROGRAM, accounts, &request_bytes())
            .expect_err("this frame must be refused")
    }

    /// The admitted frame gets PAST the guard, which is what makes the rest mean
    /// something.
    ///
    /// It cannot succeed -- these accounts hold no records, so the escrow decode
    /// refuses. The point is precisely WHICH refusal: anything but `Accounts`
    /// (0x5640) or `Authority` (0x5641) proves the walk cleared the frame guard
    /// and died later, on data. Without this control, every test below would
    /// pass against a route that refused every frame for some unrelated reason.
    #[test]
    fn the_declared_frame_clears_the_guard_and_fails_later_on_data() {
        let refused = refusal(&admitted_frame());
        assert_ne!(
            refused,
            ProgramError::from(FractionalClaimCheckCompactionSbfErrorV1::Accounts),
            "the frame the declaration describes must not be refused as a frame"
        );
        assert_ne!(
            refused,
            ProgramError::from(FractionalClaimCheckCompactionSbfErrorV1::Authority),
            "the frame the declaration describes must not be refused as an authority"
        );
    }

    /// **Witness w8, at the frame guard.**
    ///
    /// Design §17.8 predicted a direct entry without Trading would die at the
    /// `SetAuthority` hand-off with Token-2022's `MissingRequiredSignature`. It
    /// dies EARLIER and BETTER than that: the frame guard reads the root's
    /// declared privileges, sees an unsigned root, and refuses `0x5641` -- a
    /// code in this route's own band, greppable in a validator log, raised
    /// before the route has touched an account's data or sent a single CPI.
    ///
    /// The design's prediction is not wrong, it is a fallback that is now
    /// unreachable from this direction, and that is strictly the better outcome:
    /// a refusal that names a code beats a refusal borrowed from another
    /// program's error space.
    #[test]
    fn a_root_that_did_not_sign_is_refused_before_the_hand_off_and_names_a_code() {
        let mut accounts = admitted_frame();
        let index = FractionalCompactionRoleV1::FractionalCapabilityRoot
            .index()
            .expect("the root is admitted");
        let root = accounts.get(index).expect("root").clone();
        // Identical in every respect except the signature. Same key, same
        // writability, same owner -- so what this isolates is the signature and
        // nothing else.
        *accounts.get_mut(index).expect("root") =
            account(*root.key, *root.owner, false, root.is_writable);
        assert_eq!(
            refusal(&accounts),
            ProgramError::Custom(0x5641),
            "an unsigned capability root must be refused as an authority"
        );
    }

    /// The writability inversion, enforced one level down from the gate.
    ///
    /// `fractional_root_signer`'s w3 pins it in Trading; this pins it in Claims.
    /// The root is `(signer, NOT writable)` here while its three exposure
    /// neighbours require a writable root, so the plausible future edit is not
    /// malice but tidiness -- a reader "restoring consistency". Two programs now
    /// have to be talked out of it.
    #[test]
    fn a_writable_root_is_refused_by_the_frame_as_well_as_by_the_gate() {
        let mut accounts = admitted_frame();
        let index = FractionalCompactionRoleV1::FractionalCapabilityRoot
            .index()
            .expect("the root is admitted");
        let root = accounts.get(index).expect("root").clone();
        *accounts.get_mut(index).expect("root") = account(*root.key, *root.owner, true, true);
        assert_eq!(
            refusal(&accounts),
            ProgramError::Custom(0x5640),
            "a compaction revises nothing about the root, so a writable one is a frame error"
        );
    }

    /// A read-only role arriving writable is refused too, not merely tolerated.
    ///
    /// The frame guard compares privileges for EQUALITY, not implication. An
    /// account this route only reads, arriving writable, is a lock somebody else
    /// could have needed and a write nobody checked -- so "more privileged than
    /// declared" is an error in the same way "less" is.
    #[test]
    fn an_account_more_privileged_than_declared_is_refused() {
        for role in [
            FractionalCompactionRoleV1::ExposureTerms,
            FractionalCompactionRoleV1::TokenBehaviorStaging,
            FractionalCompactionRoleV1::ShardTokenProgram,
        ] {
            let mut accounts = admitted_frame();
            let index = role.index().expect("admitted role");
            let declared = accounts.get(index).expect("role").clone();
            assert!(!declared.is_writable, "{role:?} is declared read-only");
            *accounts.get_mut(index).expect("role") =
                account(*declared.key, *declared.owner, false, true);
            assert_eq!(
                refusal(&accounts),
                ProgramError::Custom(0x5640),
                "{role:?} arriving writable must be refused"
            );
        }
    }

    /// Every signer is named: the cranker and the root, and nobody else.
    ///
    /// A third presented signature is REFUSED rather than ignored. A route that
    /// merely did not read a signature would still let a caller present one, and
    /// a presented signature is a privilege somebody can be induced to grant.
    #[test]
    fn a_signature_the_route_does_not_admit_is_refused_rather_than_ignored() {
        // Every terminal-frame coordinate except the cranker at 0.
        for index in 1..TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 {
            let mut accounts = admitted_frame();
            let existing = accounts.get(index).expect("terminal account").clone();
            *accounts.get_mut(index).expect("terminal account") =
                account(*existing.key, *existing.owner, true, existing.is_writable);
            assert_eq!(
                refusal(&accounts),
                ProgramError::Custom(0x5641),
                "a signature at terminal coordinate {index} must be refused"
            );
        }
    }

    /// The frame is exactly 49, and neither 48 nor 50 is admitted.
    #[test]
    fn only_the_exact_declared_width_is_admitted() {
        let mut short = admitted_frame();
        short.pop();
        assert_eq!(refusal(&short), ProgramError::Custom(0x5640));

        let mut long = admitted_frame();
        let last = long.last().expect("last").clone();
        long.push(account(*last.key, *last.owner, false, false));
        assert_eq!(refusal(&long), ProgramError::Custom(0x5640));

        assert_eq!(FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1, 49);
        assert_eq!(
            FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 - TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
            13
        );
    }

    /// **Witness w7, at the frame guard.**
    ///
    /// The route runs its whole authentication walk over a frame that contains
    /// no caller-authority account anywhere, because there is no coordinate at
    /// which one could sit: `TradingCallerAuthority.index()` is `None`, and the
    /// frame is exactly the thirteen admitted roles. Design §17.8 ruling 2 as an
    /// observation over the shipped entry point rather than an assertion about
    /// the declaration.
    ///
    /// It is the frame-guard half of w7. The other half -- that a compaction
    /// SUCCEEDS with no caller authority in frame -- needs the campaign, and is
    /// named as not proved rather than implied by this.
    #[test]
    fn the_route_walks_a_frame_with_no_caller_authority_at_any_coordinate() {
        assert_eq!(
            FractionalCompactionRoleV1::TradingCallerAuthority.index(),
            None
        );
        let accounts = admitted_frame();
        assert_eq!(accounts.len(), FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1);
        // The walk clears the guard, so every coordinate it reads is one of the
        // thirteen the declaration admits plus the terminal frame's own.
        let refused = refusal(&accounts);
        assert_ne!(refused, ProgramError::Custom(0x5640));
        assert_ne!(refused, ProgramError::Custom(0x5641));
        // And the declaration keeps the role refused with its stated reason, so
        // the account cannot return by being forgotten about.
        assert_eq!(
            FractionalCompactionRoleV1::TradingCallerAuthority.admission(),
            dclutch_claims_svm::fractional_claim_check_v1::FractionalCompactionAdmissionV1::RefusedTakesNoParentAuthority
        );
    }

    /// A wire that is not this route's wire is refused as an identity.
    #[test]
    fn a_request_that_is_not_a_fractional_compaction_is_refused() {
        let accounts = admitted_frame();
        let mut wrong = request_bytes();
        *wrong.first_mut().expect("magic") = b'X';
        assert_eq!(
            process_fractional_compaction(&PROGRAM, &accounts, &wrong)
                .expect_err("a foreign wire must be refused"),
            ProgramError::Custom(0x5642)
        );
        assert_eq!(
            request_bytes().len(),
            FRACTIONAL_COMPACT_TO_CLAIM_CHECK_BYTES_V1
        );
    }
}
