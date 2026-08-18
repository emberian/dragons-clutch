//! `Intent::CreateMarket` — validated initialization-write.
//!
//! This module brings a market into existence: it authenticates a fixed
//! twelve-account plane, re-composes the whole of
//! [`clutch_solana_reference::validate_market_init`] on-chain, writes the eight
//! initial account states, and then re-runs that same validation over the bytes
//! it just wrote.  A refusal anywhere aborts the instruction, and SVM
//! transaction semantics — not this program — discard the partial write.
//!
//! It contains no economic logic.  The founding market is empty by
//! construction, so the only kernel call is
//! [`clutch_kernel::MarketState::check_invariants`] over the state being
//! founded; byte ownership stays in [`clutch_solana_layout`] and in the
//! reference-only codecs of [`clutch_solana_reference`], and metadata
//! authentication stays in [`crate::accounts`].
//!
//! ## Authority model — **PROPOSED**
//!
//! Market creation here is **permissionless**, and that is a proposal this lane
//! is making, not a frozen rule.  Concretely:
//!
//! - the account at index [`IX_CREATOR`] must present an authenticated
//!   signature, and it is the fee payer the runtime already charges;
//! - **there is no privileged key of any kind** — no protocol admin, no Realm
//!   authority, no deploy authority, and no allow-list.  `PROJECT.md` §4 puts
//!   permissionless work at the edge and gives no market-creation gate, and the
//!   frozen [`clutch_solana_layout::RealmAccount`] carries no authority field to
//!   check one against, so inventing an authority here would be inventing an
//!   ABI;
//! - the creator becomes the owner of the founding position triple.  That is
//!   the same owner interpretation [`crate::seeds`] proposes and
//!   [`super::split`] already enforces: the 32-byte owner identity is the raw
//!   bytes of the signing wallet address;
//! - the only real gates are **structural**: the Realm and Profile accounts
//!   must already exist at their canonical addresses, the Profile must have
//!   **frozen** its collateral policy, and the immutable terms artifact the new
//!   market binds must already exist and already bind this Realm, Profile,
//!   feed, and outcome count.  A Realm that has not decided which collateral
//!   policy it commits to must not mint liabilities, which is
//!   `require_frozen_collateral_policy` in the offline reference adapter.
//!
//! Two alternatives were considered and rejected in the same breath.  A Realm
//! authority signature has nowhere to live in the frozen layout.  A program
//! upgrade-authority gate would centralize creation on the deployer, which is
//! the opposite of the charter.
//!
//! ### Residues this authority model does not close
//!
//! Market identity is `canonical_market_id(realm, profile, market_nonce)` and
//! is **not creator-bound**, so a nonce is a first-come address: an observer of
//! a pending transaction can create the same market first.  It cannot create a
//! *different* market at that address — every field is a function of the
//! identity, the terms artifact, and the signer — but it does become the
//! founding position owner.  Naming it is not fixing it.
//!
//! ## Account creation is out of scope this wave — the named deferred step
//!
//! **This instruction creates no account.**  All twelve accounts arrive
//! already created, program-owned, rent-funded, correctly sized, and — for the
//! eight it writes — **all-zero**.  That mirrors exactly how the bring-up
//! harness loads its plane at genesis, and it is deferred checks 2, 4, and 6 of
//! `docs/implementation/SBF_BRINGUP.md`: the `system_instruction::create_account`
//! CPI, the rent-exemption computation, and the `invoke_signed` seed plumbing
//! are unwritten and untested, and this lane does not pretend otherwise.  What
//! *is* written is the half that has an oracle — the validated
//! initialization-write — and the all-zero precondition is what makes the
//! missing half detectable rather than assumed.
//!
//! ## Where the collateral cap comes from, and the field still unsourced
//!
//! - [`MarketAccount::collateral_cap`] is written from
//!   [`clutch_solana_layout::TermsAccount::collateral_cap`] — the immutable,
//!   digest-committed terms field the v3 revision added for exactly this
//!   (`RESOLUTION_EVIDENCE_PLAN.md` §3.5's finding: the cap needs a terms
//!   field, not a policy field).  **Markets founded here are fundable**: the
//!   terms codec refuses a zero cap, so "cap 0 refuses at market init" is
//!   structural — a terms artifact with no cap decision cannot exist, and
//!   the old residue ("a market created today exists and cannot accept
//!   collateral") is closed.  [`validate_initial_plane`] re-checks that the
//!   written cap equals the terms' cap, so a founding write cannot invent
//!   one.  What this instruction still cannot check is the *ceiling*: the
//!   offline reference's `validate_market_init` now takes the 266
//!   collateral-policy bytes as an evidence input and refuses a cap above
//!   `check_market_cap`'s mint ceiling, but no account in this frozen
//!   twelve-account plane carries those bytes, so the on-chain half of that
//!   check is an obligation on whoever adds a policy-bytes account to the
//!   schema.  This program keeps the freeze-discipline gate
//!   (`require_frozen_collateral_policy`) and names the gap rather than
//!   pretending the binding was checked.
//! - [`MarketAccount::created_slot`] is written **`0`**.  The honest value is
//!   the `Clock` sysvar slot, and this crate has no clock plane: no sysvar
//!   dependency, no sysvar account role, and adding either is a shared-file
//!   decision.  No check in the layout crate, in the reference adapter, or here
//!   reads the field, so a zero is inert rather than load-bearing — but it is a
//!   placeholder and is listed as such.
//!
//! ## Free initial values this lane chose — **PROPOSED**
//!
//! - `position.generation` and the external/replay `position_generation` are
//!   **`0`**: a market's founding triple enters at generation zero, and the
//!   external and replay PDAs are seeded on it.
//! - `supply.generation` is **`0`**: one accounting era per ledger lifetime,
//!   per `docs/implementation/MULTI_POSITION_CLOSURE.md` §4.  Nothing writes it
//!   again, so an era bump is structurally impossible.
//! - `hoard.authority` is the **Hoard PDA's own address bytes**.  The frozen
//!   codec only requires it nonzero; making it the account's own address is the
//!   one choice that is checkable from the account list rather than asserted.
//! - The kernel payout set is **copied from the immutable terms artifact**, and
//!   the resolution record is initialized **unresolved**
//!   ([`clutch_solana_layout::PAYOUT_INDEX_UNRESOLVED`], zero window, zero
//!   cursors).
//! - The request envelope's `sequence` must be **`0`**.  Creation consumes no
//!   replay sequence — the founding replay account is written at zero, so the
//!   first `Split` uses sequence zero — and a nonzero creation sequence is a
//!   replay-plane claim this instruction cannot honour.
//!
//! ## What is checked, and where it comes from
//!
//! [`validate_initial_plane`] is a re-composition of
//! `clutch_solana_reference::validate_market_init`, which cannot be called from
//! a program: the SBF backend reports it as overflowing the 4 KiB call frame.
//! Every check it performs is one of that function's, in that function's
//! order.  Two of what used to be this module's "named strengthenings" — the
//! terms artifact binding the new market, and the kernel payout set equalling
//! the terms payout set — are now the reference's own creation checks (the
//! terms artifact became an input of `validate_market_init` when the cap flow
//! landed), so they are parity rather than strengthenings.  One strengthening
//! remains, and one reference check has no on-chain counterpart:
//!
//! | divergence | direction | why |
//! | --- | --- | --- |
//! | the resolution record must be present, bound, and unresolved | stricter here | the reference's `validate_market_init` has no resolution account; this instruction writes one, so it validates one |
//! | the collateral policy is recomputed and bound, and the cap checked against its ceiling | reference only | the reference takes the 266 policy bytes as an evidence input; no account in this frozen plane carries them, so this program checks freeze discipline only — fail-closed but weaker, and named in the cap section above |
//!
//! ## Refusal codes
//!
//! `error.rs` is unfrozen this wave and carries the four appends this table
//! used to reserve.  What this instruction emits:
//!
//! | check | emitted | code |
//! | --- | --- | --- |
//! | a target account was not all-zero (re-initialization) | [`ClutchError::AlreadyInitialized`] | `0x0040` |
//! | the Profile's collateral policy is not frozen | [`ClutchError::CollateralPolicyNotFrozen`] | `0x0041` |
//! | the kernel payout set is not the terms payout set | [`ClutchError::PayoutSetMismatch`] | `0x0042` |
//! | the terms artifact does not bind this market, or the written cap is not the terms' cap | [`ClutchError::TermsBindingMismatch`] | `0x0043` |
//! | an initial value was nonzero | `Reference(NonEmptyInitialization)` | `0x3010` |
//! | every other check | the [`ClutchError`] the check already has | `0x0001..=0x0017` |
//!
//! ## Frame discipline
//!
//! Every function holding a whole decoded account is `#[inline(never)]`, for
//! the reason [`crate::accounts`] gives: the kernel account and the terms
//! artifact are over a kilobyte each, and the 4 KiB SBF frame does not hold two
//! of them plus a caller.  Each of the eight target accounts is decoded exactly
//! once during validation, into a small facts structure; the payout set is
//! never carried into [`process`]'s frame.
//!
//! This is **measured, not reasoned**: `cargo-build-sbf` emits its frame-space
//! diagnostic for `clutch_solana_reference::validate_market_init` (estimated
//! 10496 bytes) and for six other functions in the layout and reference crates
//! that this program never calls, and for **no** function in `clutch_sbf` —
//! this module's included.  Re-running that build is how the discipline stays
//! true; a frame overflow is undefined behaviour the loader will happily
//! execute, so it is not something to discover from a failing transaction.
//!
//! The compute-unit cost is a different question and is **not** answered here.
//! Zeroing checks scan every target account, and validation decodes eight
//! accounts plus the kernel twice more and the terms artifact twice more.  No
//! budget has been measured for this instruction; that is obligation 10 of
//! `docs/implementation/SOLANA_REFERENCE_ADAPTER.md` and it stays open.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_representation_bound,
    require_signer, require_two_term_closure, MarketFacts, Outcome, RealmFacts, StateRole,
    SupplyFacts, TermsFacts,
};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_kernel::{
    MarketState, PayoutSet, PayoutVector, Phase, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES,
};
use clutch_solana_layout::{
    account_len, canonical_market_id, canonical_outcome_id, Hash32, HoardAccount, Intent,
    MarketAccount, PositionAccount, ProfileAccount, ResolutionAccount, SupplyLedgerAccount,
    TermsAccount, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_INDEX_UNRESOLVED, PROFILE_FLAG_POLICY_FROZEN,
};
use clutch_solana_reference::{
    Action, Error as ReferenceError, ExternalAccount, KernelAccount, ReplayAccount, Request,
    EXTERNAL_ACCOUNT_LEN, KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/* ------------------------------------------------------------------------ */
/* Account plane                                                             */
/* ------------------------------------------------------------------------ */

/// Exact number of accounts this instruction accepts.
///
/// A fixed count is itself a check: a remaining-account shuffle cannot append
/// an extra writable account for a later instruction to reuse.
pub const ACCOUNT_COUNT: usize = 12;

/// Authenticated creator; pays, signs, and owns the founding position.
pub const IX_CREATOR: usize = 0;
/// Realm configuration account (read-only).
pub const IX_REALM: usize = 1;
/// Profile identity account (read-only).
pub const IX_PROFILE: usize = 2;
/// Immutable terms artifact the new market binds (read-only).
pub const IX_TERMS: usize = 3;
/// Market account to initialize.
pub const IX_MARKET: usize = 4;
/// Hoard collateral account to initialize.
pub const IX_HOARD: usize = 5;
/// Founding position account to initialize.
pub const IX_POSITION: usize = 6;
/// Reference-only kernel-aggregate account to initialize.
pub const IX_KERNEL: usize = 7;
/// Reference-only external-shadow account to initialize.
pub const IX_EXTERNAL: usize = 8;
/// Reference-only replay-sequence account to initialize.
pub const IX_REPLAY: usize = 9;
/// Market-wide supply-ledger account to initialize.
pub const IX_SUPPLY: usize = 10;
/// Resolution-record account to initialize, unresolved.
pub const IX_RESOLUTION: usize = 11;

/// The program-owned state roles of this instruction, in account-list order.
const STATE_ROLES: [StateRole; 11] = [
    StateRole::read_only(IX_REALM, account_len::REALM),
    StateRole::read_only(IX_PROFILE, account_len::PROFILE),
    StateRole::read_only(IX_TERMS, account_len::TERMS),
    StateRole::writable(IX_MARKET, account_len::MARKET),
    StateRole::writable(IX_HOARD, account_len::HOARD),
    StateRole::writable(IX_POSITION, account_len::POSITION),
    StateRole::writable(IX_KERNEL, KERNEL_ACCOUNT_LEN),
    StateRole::writable(IX_EXTERNAL, EXTERNAL_ACCOUNT_LEN),
    StateRole::writable(IX_REPLAY, REPLAY_ACCOUNT_LEN),
    StateRole::writable(IX_SUPPLY, account_len::SUPPLY_LEDGER),
    StateRole::writable(IX_RESOLUTION, account_len::RESOLUTION),
];

/// The eight roles this instruction initializes, which must arrive all-zero.
const TARGET_ROLES: [usize; 8] = [
    IX_MARKET,
    IX_HOARD,
    IX_POSITION,
    IX_KERNEL,
    IX_EXTERNAL,
    IX_REPLAY,
    IX_SUPPLY,
    IX_RESOLUTION,
];

/* ------------------------------------------------------------------------ */
/* Request, bumps, and plane views                                           */
/* ------------------------------------------------------------------------ */

/// One already-matched `CreateMarket` intent.
///
/// [`crate::dispatch`] hands this module the whole envelope, so the match lives
/// here.  The fallback arm is not decoration: it is the same
/// `_ => Err(UnsupportedIntent)` the offline reference adapter keeps, and it is
/// what stops a future routing edit from delivering another intent into an
/// initializer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateMarketIntent {
    /// Realm namespace the market is created under.
    pub realm: Hash32,
    /// Profile identity the Realm commits to.
    pub profile: Hash32,
    /// Nonce distinguishing markets within one `(realm, profile)` pair.
    pub market_nonce: u64,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Immutable terms digest the market binds.
    pub terms: Hash32,
    /// Feed identity the market resolves against.
    pub feed: Hash32,
}

/// The canonical bumps of every account this instruction initializes.
///
/// The reference-only kernel aggregate has no stored bump field, so it is
/// absent here and its derivation check is address-only; that gap is deferred
/// check 5 of `docs/implementation/SBF_BRINGUP.md`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaneBumps {
    /// Market PDA bump.
    pub market: u8,
    /// Hoard PDA bump.
    pub hoard: u8,
    /// Founding position PDA bump.
    pub position: u8,
    /// External-shadow PDA bump.
    pub external: u8,
    /// Replay-sequence PDA bump.
    pub replay: u8,
    /// Supply-ledger PDA bump.
    pub supply: u8,
    /// Resolution-record PDA bump.
    pub resolution: u8,
}

/// Read-only view of the eight initialized accounts.
#[derive(Clone, Copy, Debug)]
pub struct PlaneBytes<'a> {
    /// Market account bytes.
    pub market: &'a [u8],
    /// Hoard account bytes.
    pub hoard: &'a [u8],
    /// Founding position account bytes.
    pub position: &'a [u8],
    /// Reference-only kernel-aggregate bytes.
    pub kernel: &'a [u8],
    /// Reference-only external-shadow bytes.
    pub external: &'a [u8],
    /// Reference-only replay-sequence bytes.
    pub replay: &'a [u8],
    /// Supply-ledger bytes.
    pub supply: &'a [u8],
    /// Resolution-record bytes.
    pub resolution: &'a [u8],
}

/// Writable view of the eight initialized accounts.
#[derive(Debug)]
pub struct PlaneWrite<'a> {
    /// Market account bytes.
    pub market: &'a mut [u8],
    /// Hoard account bytes.
    pub hoard: &'a mut [u8],
    /// Founding position account bytes.
    pub position: &'a mut [u8],
    /// Reference-only kernel-aggregate bytes.
    pub kernel: &'a mut [u8],
    /// Reference-only external-shadow bytes.
    pub external: &'a mut [u8],
    /// Reference-only replay-sequence bytes.
    pub replay: &'a mut [u8],
    /// Supply-ledger bytes.
    pub supply: &'a mut [u8],
    /// Resolution-record bytes.
    pub resolution: &'a mut [u8],
}

/// The identities a founding write is parameterized by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingIdentities {
    /// Canonical market identity, derived from the intent.
    pub market: Hash32,
    /// Founding position owner; the creator's raw address bytes.
    pub owner: Hash32,
    /// Hoard authority; the Hoard PDA's own address bytes.
    pub hoard_authority: Hash32,
}

/* ------------------------------------------------------------------------ */
/* Local refusal helpers                                                     */
/* ------------------------------------------------------------------------ */

/// Refuse with a reference-adapter class unless `condition` holds.
///
/// Two of this instruction's checks are the reference adapter's own named
/// refusals and have no adapter-vocabulary equivalent; see the refusal table in
/// the module docs.
fn require_reference(condition: bool, error: ReferenceError) -> Outcome<()> {
    if condition {
        Ok(())
    } else {
        Err(Refusal::Reference(error))
    }
}

/// Refuse with the terms-binding append unless `condition` holds.
///
/// The layout crate's own `binds_market` raises `Codec(MismatchedBinding)`
/// for the same disagreement; this instruction emits the allocated
/// first-class code [`ClutchError::TermsBindingMismatch`] (`0x0043`) so a
/// transaction log names the check, not the codec that happened to run it.
fn require_binding(condition: bool) -> Outcome<()> {
    require(condition, ClutchError::TermsBindingMismatch)
}

/* ------------------------------------------------------------------------ */
/* Local plane readers                                                       */
/* ------------------------------------------------------------------------ */

/// Profile facts including the freeze discipline.
///
/// [`crate::accounts::read_profile`] carries neither the flags nor the
/// collateral-policy digest, and the freeze gate needs both.  The frozen codec
/// already refuses every combination except "flag set exactly when the digest
/// is nonzero", so the two fields are carried rather than pre-judged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProfileInitFacts {
    profile: Hash32,
    realm: Hash32,
    version: u8,
    collateral_policy_digest: Hash32,
    flags: u8,
}

/// Hoard facts a founding write binds against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HoardFacts {
    market: Hash32,
    realm: Hash32,
    authority: Hash32,
    collateral_atoms: u64,
    stored_bump: u8,
}

/// Founding-position facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PositionFacts {
    market: Hash32,
    owner: Hash32,
    generation: u64,
    internal: [u64; MAX_OUTCOMES],
    cash_atoms: u64,
    reserved_cash_atoms: u64,
    stored_bump: u8,
    close_state: u8,
}

/// External-shadow facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExternalFacts {
    market: Hash32,
    owner: Hash32,
    position_generation: u64,
    balances: [u64; MAX_OUTCOMES],
    stored_bump: u8,
}

/// Replay-sequence facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReplayFacts {
    market: Hash32,
    owner: Hash32,
    position_generation: u64,
    sequence: u64,
    stored_bump: u8,
}

/// Decode a Profile account, carrying the freeze discipline fields.
#[inline(never)]
fn read_profile_init(data: &[u8]) -> Outcome<ProfileInitFacts> {
    let value = ProfileAccount::decode(data)?;
    Ok(ProfileInitFacts {
        profile: value.profile,
        realm: value.realm,
        version: value.version,
        collateral_policy_digest: value.collateral_policy_digest,
        flags: value.flags,
    })
}

/// Decode a Hoard account.
#[inline(never)]
fn read_hoard(data: &[u8]) -> Outcome<HoardFacts> {
    let value = HoardAccount::decode(data)?;
    Ok(HoardFacts {
        market: value.market,
        realm: value.realm,
        authority: value.authority,
        collateral_atoms: value.collateral_atoms,
        stored_bump: value.stored_bump,
    })
}

/// Decode a Position account.
#[inline(never)]
fn read_position(data: &[u8]) -> Outcome<PositionFacts> {
    let value = PositionAccount::decode(data)?;
    Ok(PositionFacts {
        market: value.market,
        owner: value.owner,
        generation: value.generation,
        internal: value.internal,
        cash_atoms: value.cash_atoms,
        reserved_cash_atoms: value.reserved_cash_atoms,
        stored_bump: value.stored_bump,
        close_state: value.close_state,
    })
}

/// Decode a reference-only external-shadow account.
#[inline(never)]
fn read_external(data: &[u8]) -> Outcome<ExternalFacts> {
    let value = ExternalAccount::decode(data)?;
    Ok(ExternalFacts {
        market: value.market,
        owner: value.owner,
        position_generation: value.position_generation,
        balances: value.balances,
        stored_bump: value.stored_bump,
    })
}

/// Decode a reference-only replay-sequence account.
#[inline(never)]
fn read_replay(data: &[u8]) -> Outcome<ReplayFacts> {
    let value = ReplayAccount::decode(data)?;
    Ok(ReplayFacts {
        market: value.market,
        owner: value.owner,
        position_generation: value.position_generation,
        sequence: value.sequence,
        stored_bump: value.stored_bump,
    })
}

/* ------------------------------------------------------------------------ */
/* Payout-set plumbing                                                       */
/* ------------------------------------------------------------------------ */

/// Lift the immutable terms payout vectors into the kernel's payout set.
///
/// The terms artifact is the only committed source of "what this market pays":
/// [`MarketAccount::terms`] is the digest of the vectors' body.  The whole set
/// is over a kilobyte, so this is the only place it is materialized and it
/// never crosses into [`process`]'s frame.
///
/// `decode_unchecked`: the terms bytes were already fully decoded — digest
/// recomputation included — earlier in this same instruction (the address
/// plane in [`process`], and `validate_market_wide`'s own full read before
/// `require_payout_set_binding` runs), and the account is presented
/// read-only, so re-paying the SHA-256 here would be a second copy of a fact
/// this transaction already established.
#[inline(never)]
fn terms_payout_set(terms_data: &[u8]) -> Outcome<PayoutSet> {
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_unchecked_into(terms_data, &mut terms)?;
    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    let mut index = 0_usize;
    while index < usize::from(terms.payout_count) {
        vectors[index] = PayoutVector::new(
            terms.payouts[index].denominator,
            terms.payouts[index].weights,
        );
        index += 1;
    }
    Ok(PayoutSet::new(
        terms.payout_count,
        terms.outcome_count,
        vectors,
    ))
}

/// The terms' digest-committed collateral cap, in its own frame.
///
/// Same `decode_unchecked` soundness argument as [`terms_payout_set`].
#[inline(never)]
fn terms_collateral_cap(terms_data: &[u8]) -> Outcome<u64> {
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_unchecked_into(terms_data, &mut terms)?;
    Ok(terms.collateral_cap)
}

/// Compare an encoded kernel account's payout set against an expected set.
#[inline(never)]
fn require_kernel_payouts(kernel_data: &[u8], expected: &PayoutSet) -> Outcome<()> {
    let kernel = KernelAccount::decode(kernel_data)?;
    require(
        kernel.payouts.count == expected.count && kernel.payouts.outcomes == expected.outcomes,
        ClutchError::PayoutSetMismatch,
    )?;
    let mut index = 0_usize;
    while index < MAX_PAYOUTS {
        require(
            kernel.payouts.vectors[index] == expected.vectors[index],
            ClutchError::PayoutSetMismatch,
        )?;
        index += 1;
    }
    Ok(())
}

/// The reference adapter's `require_payout_set_binding`, hoisted to creation.
#[inline(never)]
fn require_payout_set_binding(kernel_data: &[u8], terms_data: &[u8]) -> Outcome<()> {
    let expected = terms_payout_set(terms_data)?;
    require_kernel_payouts(kernel_data, &expected)
}

/// The reference adapter's `kernel_market` plus `check_invariants`.
///
/// The whole `KernelAccount`/`MarketState` working set lives in this frame and
/// nowhere else, exactly as [`super::split`]'s `kernel_split` does.
#[inline(never)]
fn require_kernel_invariants(
    kernel_data: &[u8],
    outcome_count: u8,
    collateral: u64,
) -> Outcome<()> {
    let kernel = KernelAccount::decode(kernel_data)?;
    require(
        usize::from(outcome_count) <= KERNEL_MAX_OUTCOMES
            && kernel.payouts.outcomes == outcome_count,
        ClutchError::MismatchedState,
    )?;
    let phase = match kernel.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(ClutchError::NonCanonical.into()),
    };
    let market = MarketState {
        outcomes: outcome_count,
        phase,
        resolved_payout: kernel.resolved_payout,
        collateral,
        total_supply: kernel.total_supply,
        payouts: kernel.payouts,
    };
    market.check_invariants()?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Preconditions                                                             */
/* ------------------------------------------------------------------------ */

/// Match the routed envelope, refusing any intent that is not `CreateMarket`.
pub fn create_market_intent(request: &Request) -> Outcome<CreateMarketIntent> {
    match request.action {
        Action::Layout(Intent::CreateMarket {
            realm,
            profile,
            market_nonce,
            outcome_count,
            terms,
            feed,
        }) => Ok(CreateMarketIntent {
            realm,
            profile,
            market_nonce,
            outcome_count,
            terms,
            feed,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Creation consumes no replay sequence, so the envelope must carry zero.
pub fn require_creation_sequence(sequence: u64) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)
}

/// Refuse an account that is not entirely zero.
///
/// This is the idempotence gate and it is the whole of it: a market that
/// already exists has nonzero bytes at its canonical address, so a second
/// `CreateMarket` at that address refuses here before deriving anything,
/// reading any identity, or writing any byte.  It is also what makes the
/// missing account-creation CPI *detectable*: an account that was never created
/// has no data at all and fails the role length check instead.
pub fn require_zeroed(data: &[u8]) -> Outcome<()> {
    let mut index = 0_usize;
    while index < data.len() {
        if data[index] != 0 {
            return Err(Refusal::Adapter(ClutchError::AlreadyInitialized));
        }
        index += 1;
    }
    Ok(())
}

/// Refuse a Realm whose Profile has not frozen its collateral policy.
///
/// This is a freeze-discipline check, not a binding check: a well-formed
/// frozen Profile can still commit to another Realm's collateral policy and
/// nothing here would notice.  The recompute-and-compare exists now —
/// `collateral::verify_collateral_binding`, consumed by the offline
/// reference's `validate_market_init` — but it needs the 266 policy bytes,
/// which no account in this frozen twelve-account plane carries; see the cap
/// section of the module docs.
fn require_frozen_collateral_policy(profile: &ProfileInitFacts) -> Outcome<()> {
    require(
        profile.flags & PROFILE_FLAG_POLICY_FROZEN != 0
            && profile.collateral_policy_digest != Hash32::ZERO,
        ClutchError::CollateralPolicyNotFrozen,
    )
}

/* ------------------------------------------------------------------------ */
/* The initialization write                                                  */
/* ------------------------------------------------------------------------ */

/// Encode the initial Market account.
#[inline(never)]
fn write_market(
    data: &mut [u8],
    intent: &CreateMarketIntent,
    market: Hash32,
    bumps: &PlaneBumps,
    collateral_cap: u64,
) -> Outcome<()> {
    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    let mut index = 0_usize;
    while index < usize::from(intent.outcome_count) && index < MAX_OUTCOMES {
        outcomes[index] = canonical_outcome_id(market, index as u8);
        index += 1;
    }
    let account = MarketAccount {
        market,
        realm: intent.realm,
        profile: intent.profile,
        terms: intent.terms,
        outcome_count: intent.outcome_count,
        lifecycle: 0,
        stored_bump: bumps.market,
        hoard_bump: bumps.hoard,
        outcomes,
        feed: intent.feed,
        /* The terms' digest-committed cap; `created_slot` stays the named
         * zero placeholder.  See the cap section of the module docs. */
        collateral_cap,
        created_slot: 0,
        reserved: Hash32::ZERO,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial Hoard account.
#[inline(never)]
fn write_hoard(
    data: &mut [u8],
    identities: &FoundingIdentities,
    realm: Hash32,
    bump: u8,
) -> Outcome<()> {
    let account = HoardAccount {
        market: identities.market,
        realm,
        authority: identities.hoard_authority,
        collateral_atoms: 0,
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the founding Position account, provably zero (C0).
#[inline(never)]
fn write_position(data: &mut [u8], market: Hash32, owner: Hash32, bump: u8) -> Outcome<()> {
    let account = PositionAccount {
        market,
        owner,
        generation: 0,
        internal: [0; MAX_OUTCOMES],
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        stored_bump: bump,
        close_state: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial reference-only kernel aggregate.
#[inline(never)]
fn write_kernel(data: &mut [u8], terms_data: &[u8], market: Hash32) -> Outcome<()> {
    let payouts = terms_payout_set(terms_data)?;
    let account = KernelAccount {
        market,
        phase: 0,
        resolved_payout: 0,
        payouts,
        total_supply: [0; MAX_OUTCOMES],
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the founding external shadow, provably zero (C0).
#[inline(never)]
fn write_external(data: &mut [u8], market: Hash32, owner: Hash32, bump: u8) -> Outcome<()> {
    let account = ExternalAccount {
        market,
        owner,
        position_generation: 0,
        balances: [0; MAX_OUTCOMES],
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the founding replay sequence, provably zero (C0).
#[inline(never)]
fn write_replay(data: &mut [u8], market: Hash32, owner: Hash32, bump: u8) -> Outcome<()> {
    let account = ReplayAccount {
        market,
        owner,
        position_generation: 0,
        sequence: 0,
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial market-wide supply ledger, both terms zero.
#[inline(never)]
fn write_supply(
    data: &mut [u8],
    market: Hash32,
    realm: Hash32,
    outcome_count: u8,
    bump: u8,
) -> Outcome<()> {
    let account = SupplyLedgerAccount {
        market,
        realm,
        generation: 0,
        outcome_count,
        internal_supply: [0; MAX_OUTCOMES],
        external_supply: [0; MAX_OUTCOMES],
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Encode the initial resolution record, unresolved.
#[inline(never)]
fn write_resolution(
    data: &mut [u8],
    market: Hash32,
    intent: &CreateMarketIntent,
    bump: u8,
) -> Outcome<()> {
    let account = ResolutionAccount {
        market,
        terms: intent.terms,
        feed: intent.feed,
        window: Hash32::ZERO,
        feed_cursor: 0,
        sealed_end_bucket_exclusive: 0,
        repair_generation: 0,
        resolved_slot: 0,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        stored_bump: bump,
        flags: 0,
    };
    account.encode(data)?;
    Ok(())
}

/// Write all eight initial account states.
///
/// Every `encode` runs the frozen codec's own `validate` first, so a malformed
/// account never reaches an account's data.  Nothing here checks anything
/// *across* accounts; that is [`validate_initial_plane`], which runs afterwards
/// over exactly these bytes.
#[inline(never)]
pub fn write_initial_plane(
    terms_data: &[u8],
    plane: PlaneWrite<'_>,
    intent: &CreateMarketIntent,
    identities: &FoundingIdentities,
    bumps: &PlaneBumps,
) -> Outcome<()> {
    let market = identities.market;
    write_market(
        plane.market,
        intent,
        market,
        bumps,
        terms_collateral_cap(terms_data)?,
    )?;
    write_hoard(plane.hoard, identities, intent.realm, bumps.hoard)?;
    write_position(plane.position, market, identities.owner, bumps.position)?;
    write_kernel(plane.kernel, terms_data, market)?;
    write_external(plane.external, market, identities.owner, bumps.external)?;
    write_replay(plane.replay, market, identities.owner, bumps.replay)?;
    write_supply(
        plane.supply,
        market,
        intent.realm,
        intent.outcome_count,
        bumps.supply,
    )?;
    write_resolution(plane.resolution, market, intent, bumps.resolution)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* The validation                                                            */
/* ------------------------------------------------------------------------ */

/// The market-wide half of `validate_market_init`.
///
/// Returns the decoded market facts so the founding-triple half does not decode
/// the market a second time.  The check order is the reference's: stored bumps,
/// cross-account linkage, the freeze gate, the intent-to-state identity
/// conjunction, the ledger binding, emptiness, kernel invariants, padding, and
/// the two-term closure.
#[inline(never)]
fn validate_market_wide(
    realm_data: &[u8],
    profile_data: &[u8],
    terms_data: &[u8],
    plane: PlaneBytes<'_>,
    intent: &CreateMarketIntent,
    bumps: &PlaneBumps,
) -> Outcome<MarketFacts> {
    let realm: RealmFacts = accounts::read_realm(realm_data)?;
    let profile = read_profile_init(profile_data)?;
    let terms: TermsFacts = accounts::read_terms(terms_data)?;
    let market: MarketFacts = accounts::read_market(plane.market)?;
    let hoard = read_hoard(plane.hoard)?;
    let kernel = accounts::read_kernel(plane.kernel)?;
    let supply: SupplyFacts = accounts::read_supply(plane.supply)?;
    let resolution = accounts::read_resolution(plane.resolution)?;

    /* Stored bumps, before anything reads a balance: an account presented at a
     * canonical address but carrying another address's bump is a mislinked
     * account whatever its contents say. */
    require(
        market.stored_bump == bumps.market
            && market.hoard_bump == bumps.hoard
            && hoard.stored_bump == bumps.hoard
            && supply.stored_bump == bumps.supply
            && resolution.stored_bump == bumps.resolution,
        ClutchError::WrongBump,
    )?;

    /* Cross-account linkage, mirroring `validate_links`. */
    require(
        market.market == hoard.market
            && market.realm == hoard.realm
            && market.market == kernel.market
            && market.market == supply.market
            && market.realm == supply.realm
            && market.outcome_count == supply.outcome_count
            && market.lifecycle == 0
            && kernel.phase == 0,
        ClutchError::MismatchedState,
    )?;

    require_frozen_collateral_policy(&profile)?;

    /* The intent-to-state conjunction of `validate_market_init`, including the
     * canonical market identity and the Realm/Profile edges. */
    let expected_market = canonical_market_id(intent.realm, intent.profile, intent.market_nonce);
    require(
        realm.realm == intent.realm
            && realm.profile == intent.profile
            && profile.profile == intent.profile
            && profile.realm == intent.realm
            && realm.profile_version == profile.version
            && usize::from(realm.max_outcomes) == MAX_OUTCOMES
            && intent.outcome_count <= realm.max_outcomes
            && market.market == expected_market
            && market.realm == intent.realm
            && market.profile == intent.profile
            && market.outcome_count == intent.outcome_count
            && market.terms == intent.terms
            && market.feed == intent.feed,
        ClutchError::MismatchedState,
    )?;

    /* NAMED STRENGTHENING: the presented terms artifact must be the one this
     * market's digest binds.  `TermsAccount::binds_market` is exactly this
     * comparison, and its refusal class is reproduced rather than reclassified.
     * The digest is self-certifying inside the codec, so equality of the digest
     * plus these five fields is equality of the whole artifact. */
    require_binding(
        terms.terms == market.terms
            && terms.realm == market.realm
            && terms.profile == market.profile
            && terms.feed == market.feed
            && terms.outcome_count == market.outcome_count,
    )?;

    /* The written cap must be the terms' digest-committed cap — the cap flow
     * of RESOLUTION_EVIDENCE_PLAN §3.5.  The terms codec refuses a zero cap,
     * so a founded market is never the unfundable cap-0 residue. */
    require_binding(market.collateral_cap == terms.collateral_cap)?;

    /* NAMED STRENGTHENING: the resolution record is present, bound, and
     * unresolved.  A market founded beside a record that already selects a
     * payout would be resolved before it existed. */
    require(
        resolution.market == market.market
            && resolution.terms == market.terms
            && resolution.feed == market.feed
            && !resolution.resolved,
        ClutchError::MismatchedState,
    )?;

    /* Emptiness.  This is the market-wide half of C0 and of the reference's
     * `NonEmptyInitialization`; the founding triple's half follows. */
    let mut outcome = 0_usize;
    while outcome < MAX_OUTCOMES {
        require_reference(
            kernel.total_supply[outcome] == 0
                && supply.internal_supply[outcome] == 0
                && supply.external_supply[outcome] == 0,
            ReferenceError::NonEmptyInitialization,
        )?;
        outcome += 1;
    }
    require_reference(
        hoard.collateral_atoms == 0,
        ReferenceError::NonEmptyInitialization,
    )?;

    /* Kernel invariants over the founded state, then the payout-set binding. */
    require_kernel_invariants(plane.kernel, market.outcome_count, hoard.collateral_atoms)?;
    require_payout_set_binding(plane.kernel, terms_data)?;

    /* C1: the two-term ledger closes against the kernel aggregate. */
    require_two_term_closure(&supply, &kernel, market.outcome_count)?;
    Ok(market)
}

/// The founding-triple half of `validate_market_init`: C0.
///
/// The triple must be mutually bound to one market, one owner, and one
/// generation, and must be provably zero — internal balances, external shadow
/// balances, position cash and reserved cash, replay sequence, and an open
/// close-state.  This is the base case the multi-position closure induction
/// starts from (`MULTI_POSITION_CLOSURE.md` C0), so a market founded around a
/// triple that already holds anything is refused rather than reconciled.
#[inline(never)]
fn validate_founding_triple(
    plane: PlaneBytes<'_>,
    market: &MarketFacts,
    bumps: &PlaneBumps,
) -> Outcome<()> {
    let position = read_position(plane.position)?;
    let external = read_external(plane.external)?;
    let replay = read_replay(plane.replay)?;
    let supply: SupplyFacts = accounts::read_supply(plane.supply)?;

    require(
        position.stored_bump == bumps.position
            && external.stored_bump == bumps.external
            && replay.stored_bump == bumps.replay,
        ClutchError::WrongBump,
    )?;
    require(
        market.market == position.market
            && market.market == external.market
            && market.market == replay.market
            && position.owner == external.owner
            && position.owner == replay.owner
            && position.generation == external.position_generation
            && position.generation == replay.position_generation,
        ClutchError::MismatchedState,
    )?;

    /* C0 proper. */
    require_reference(
        position.close_state == 0
            && position.cash_atoms == 0
            && position.reserved_cash_atoms == 0
            && replay.sequence == 0,
        ReferenceError::NonEmptyInitialization,
    )?;
    let mut outcome = 0_usize;
    while outcome < MAX_OUTCOMES {
        require_reference(
            position.internal[outcome] == 0 && external.balances[outcome] == 0,
            ReferenceError::NonEmptyInitialization,
        )?;
        outcome += 1;
    }

    /* Padding beyond the active outcome count, and C2 against the ledger terms.
     *
     * Both are *redundant here* and neither is load-bearing: the emptiness loop
     * above already proved every one of the `MAX_OUTCOMES` entries zero, which
     * implies canonical padding, and an all-zero triple is bounded by any
     * ledger.  They are kept because `validate_market_init` keeps them --
     * `validate_padding` and `validate_aggregate_closure` run on the same
     * initial state there -- and this function's contract is to be that
     * function, not a minimized version of it.  Claiming they catch something
     * at initialization would be false. */
    let count = usize::from(market.outcome_count);
    let mut padding = count;
    while padding < MAX_OUTCOMES {
        require(
            position.internal[padding] == 0 && external.balances[padding] == 0,
            ClutchError::NonCanonical,
        )?;
        padding += 1;
    }
    require_representation_bound(
        &supply,
        &position.internal,
        &external.balances,
        market.outcome_count,
    )?;
    Ok(())
}

/// Re-compose `clutch_solana_reference::validate_market_init` over one plane.
///
/// The oracle for every check here is that function; the oracle for every byte
/// [`write_initial_plane`] produces is that function plus the frozen layout
/// codecs.  It is deliberately callable on already-encoded bytes with
/// separately supplied bumps, exactly as [`crate::accounts::expect_pda`] takes
/// an already-derived address: that is what lets the whole refusal table be
/// exercised on a host where program-address derivation is not compiled.
pub fn validate_initial_plane(
    realm_data: &[u8],
    profile_data: &[u8],
    terms_data: &[u8],
    plane: PlaneBytes<'_>,
    intent: &CreateMarketIntent,
    bumps: &PlaneBumps,
) -> Outcome<()> {
    let market = validate_market_wide(realm_data, profile_data, terms_data, plane, intent, bumps)?;
    validate_founding_triple(plane, &market, bumps)
}

/* ------------------------------------------------------------------------ */
/* The instruction                                                           */
/* ------------------------------------------------------------------------ */

/// Validate hostile accounts and initialize exactly one market.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    let intent = create_market_intent(request)?;

    require_count(accounts, ACCOUNT_COUNT)?;

    /* The authority model, in three lines: the creator signs, nothing else is
     * privileged, and the creator's address is the founding position owner. */
    let creator = &accounts[IX_CREATOR];
    require_signer(creator)?;

    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &STATE_ROLES)?;
    require_creation_sequence(request.sequence)?;

    /* Idempotence, before any identity is derived or read: every account this
     * instruction initializes must arrive all-zero. */
    let mut target = 0_usize;
    while target < TARGET_ROLES.len() {
        require_zeroed(&accounts[TARGET_ROLES[target]].data.borrow())?;
        target += 1;
    }

    /* Identities.  The market identity is a function of the intent alone, and
     * caller-supplied expected keys are never accepted: every address below is
     * recomputed from the frozen seed schema and compared. */
    let market_id = canonical_market_id(intent.realm, intent.profile, intent.market_nonce);
    let realm_bytes = intent.realm.bytes();
    let profile_bytes = intent.profile.bytes();
    let terms_bytes = intent.terms.bytes();
    let market_bytes = market_id.bytes();
    let owner_bytes = creator.key.to_bytes();

    let realm_stored_bump = accounts::read_realm(&accounts[IX_REALM].data.borrow())?.stored_bump;
    let terms_stored_bump = accounts::read_terms(&accounts[IX_TERMS].data.borrow())?.stored_bump;
    expect_pda(
        accounts[IX_REALM].key,
        seeds::realm_pda(program_id, &realm_bytes),
        Some(realm_stored_bump),
    )?;
    expect_pda(
        accounts[IX_PROFILE].key,
        seeds::profile_pda(program_id, &realm_bytes, &profile_bytes),
        None,
    )?;
    expect_pda(
        accounts[IX_TERMS].key,
        seeds::terms_pda(program_id, &realm_bytes, &terms_bytes),
        Some(terms_stored_bump),
    )?;

    /* The eight target accounts carry no stored bump yet -- they are zeroed --
     * so the derived bump is compared against nothing here and is *written*
     * below, then re-checked against these same derivations by
     * `validate_initial_plane`. */
    let market_derived = seeds::market_pda(program_id, &realm_bytes, &market_bytes);
    let hoard_derived = seeds::hoard_pda(program_id, &market_bytes);
    let position_derived = seeds::position_pda(program_id, &market_bytes, &owner_bytes);
    let kernel_derived = seeds::kernel_pda(program_id, &market_bytes);
    let external_derived = seeds::external_pda(program_id, &market_bytes, &owner_bytes, 0);
    let replay_derived = seeds::replay_pda(program_id, &market_bytes, &owner_bytes, 0);
    let supply_derived = seeds::supply_pda(program_id, &market_bytes);
    let resolution_derived = seeds::resolution_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_MARKET].key, market_derived, None)?;
    expect_pda(accounts[IX_HOARD].key, hoard_derived, None)?;
    expect_pda(accounts[IX_POSITION].key, position_derived, None)?;
    expect_pda(accounts[IX_KERNEL].key, kernel_derived, None)?;
    expect_pda(accounts[IX_EXTERNAL].key, external_derived, None)?;
    expect_pda(accounts[IX_REPLAY].key, replay_derived, None)?;
    expect_pda(accounts[IX_SUPPLY].key, supply_derived, None)?;
    expect_pda(accounts[IX_RESOLUTION].key, resolution_derived, None)?;

    let bumps = PlaneBumps {
        market: market_derived.1,
        hoard: hoard_derived.1,
        position: position_derived.1,
        external: external_derived.1,
        replay: replay_derived.1,
        supply: supply_derived.1,
        resolution: resolution_derived.1,
    };
    let identities = FoundingIdentities {
        market: market_id,
        owner: Hash32::from_bytes(owner_bytes),
        hoard_authority: Hash32::from_bytes(accounts[IX_HOARD].key.to_bytes()),
    };

    /* Everything below this line writes. */
    {
        let borrow = |index: usize| {
            accounts[index]
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
        };
        let mut market_data = borrow(IX_MARKET)?;
        let mut hoard_data = borrow(IX_HOARD)?;
        let mut position_data = borrow(IX_POSITION)?;
        let mut kernel_data = borrow(IX_KERNEL)?;
        let mut external_data = borrow(IX_EXTERNAL)?;
        let mut replay_data = borrow(IX_REPLAY)?;
        let mut supply_data = borrow(IX_SUPPLY)?;
        let mut resolution_data = borrow(IX_RESOLUTION)?;
        write_initial_plane(
            &accounts[IX_TERMS].data.borrow(),
            PlaneWrite {
                market: &mut market_data,
                hoard: &mut hoard_data,
                position: &mut position_data,
                kernel: &mut kernel_data,
                external: &mut external_data,
                replay: &mut replay_data,
                supply: &mut supply_data,
                resolution: &mut resolution_data,
            },
            &intent,
            &identities,
            &bumps,
        )?;
    }

    /* ...and this re-reads exactly what was written and runs the whole of the
     * offline `validate_market_init` over it.  A refusal here aborts the
     * instruction; SVM transaction semantics discard the write. */
    validate_initial_plane(
        &accounts[IX_REALM].data.borrow(),
        &accounts[IX_PROFILE].data.borrow(),
        &accounts[IX_TERMS].data.borrow(),
        PlaneBytes {
            market: &accounts[IX_MARKET].data.borrow(),
            hoard: &accounts[IX_HOARD].data.borrow(),
            position: &accounts[IX_POSITION].data.borrow(),
            kernel: &accounts[IX_KERNEL].data.borrow(),
            external: &accounts[IX_EXTERNAL].data.borrow(),
            replay: &accounts[IX_REPLAY].data.borrow(),
            supply: &accounts[IX_SUPPLY].data.borrow(),
            resolution: &accounts[IX_RESOLUTION].data.borrow(),
        },
        &intent,
        &bumps,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        canonical_profile_hash, canonical_realm_id, CodecError, PayoutVectorBytes, RealmAccount,
        PROFILE_PARENT_BYTES,
    };

    /* These tests run on the host, where `seeds::find` is deliberately not
     * compiled (see the module docs of `crate::seeds`).  They therefore cover
     * everything this instruction is except the one thing an address syscall
     * decides: the initialization write, byte for byte, and every refusal in
     * `validate_market_init`.  `process` itself is exercised only up to its
     * first derivation; the SVM leg is a follow-up wave. */

    const REALM_NONCE: u64 = 7;
    const MARKET_NONCE: u64 = 9;
    const OUTCOME_COUNT: u8 = 2;
    const PAYOUT_COUNT: u8 = 2;
    /// The terms' digest-committed collateral cap the founding write copies.
    const FIXTURE_CAP: u64 = 5_000;

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn plane_bumps() -> PlaneBumps {
        PlaneBumps {
            market: 3,
            hoard: 4,
            position: 5,
            external: 6,
            replay: 7,
            supply: 10,
            resolution: 9,
        }
    }

    fn profile_hash() -> Hash32 {
        canonical_profile_hash(&[0xc0; PROFILE_PARENT_BYTES]).expect("exact parent preimage")
    }

    fn realm_hash() -> Hash32 {
        canonical_realm_id(profile_hash(), REALM_NONCE)
    }

    fn market_id() -> Hash32 {
        canonical_market_id(realm_hash(), profile_hash(), MARKET_NONCE)
    }

    fn unit_vector(index: usize) -> [u64; MAX_OUTCOMES] {
        let mut weights = [0; MAX_OUTCOMES];
        weights[index] = 1;
        weights
    }

    /// The immutable terms artifact the fixture market binds.
    ///
    /// Its window policy is the offline reference adapter's own resolution
    /// fixture, so the payout set this instruction lifts into the kernel is the
    /// set the reference would have expected to find there.
    fn terms_account(profile: Hash32) -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        payouts[0] = PayoutVectorBytes {
            denominator: 1,
            weights: unit_vector(0),
        };
        payouts[1] = PayoutVectorBytes {
            denominator: 1,
            weights: unit_vector(1),
        };
        let mut knots = [0u128; clutch_solana_layout::MAX_KNOTS];
        knots[0] = 1;
        let mut payout_map = [clutch_solana_layout::PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        payout_map[0] = 0;
        payout_map[1] = 1;
        let mut value = TermsAccount {
            terms: Hash32::ZERO,
            realm: realm_hash(),
            profile,
            feed: h(9),
            price_grid: h(0x9a),
            outcome_count: OUTCOME_COUNT,
            payout_count: PAYOUT_COUNT,
            payouts,
            grid_family_id: 7,
            grid_version: 1,
            bucket_seconds: 60,
            expected_start_bucket: 100,
            expected_end_bucket_exclusive: 130,
            maturity_horizon_buckets: 30,
            coverage_policy_id: 11,
            repair_policy_id: 12,
            failure_policy_id: 13,
            statistic_id: 1,
            ambiguity_policy_id: 1,
            edge_policy_id: 1,
            basis_degree: 0,
            knot_count: 1,
            uniform_log2_spacing: clutch_solana_layout::UNIFORM_SPACING_NONE,
            failure_payout_index: 0,
            coverage_policy_parameter: 0,
            repair_generation: 0,
            source_version: 1,
            evaluator_version: 1,
            source_adapter_id: h(9),
            payout_map,
            knots,
            collateral_cap: FIXTURE_CAP,
            stored_bump: 8,
            flags: 0,
        };
        value.terms = value.recomputed_terms_digest().expect("terms body digests");
        value
    }

    fn realm_account() -> RealmAccount {
        RealmAccount {
            realm: realm_hash(),
            profile: profile_hash(),
            max_outcomes: MAX_OUTCOMES as u8,
            profile_version: 1,
            stored_bump: 200,
            flags: 0,
        }
    }

    fn profile_account() -> ProfileAccount {
        ProfileAccount {
            profile: profile_hash(),
            realm: realm_hash(),
            collateral_policy_digest: h(0xd0),
            version: 1,
            flags: PROFILE_FLAG_POLICY_FROZEN,
        }
    }

    fn encoded<F>(len: usize, encode: F) -> Vec<u8>
    where
        F: FnOnce(&mut [u8]) -> core::result::Result<usize, CodecError>,
    {
        let mut bytes = vec![0; len];
        encode(&mut bytes).expect("fixture encodes");
        bytes
    }

    /// One founded market, as [`write_initial_plane`] actually writes it.
    struct Founded {
        realm: Vec<u8>,
        profile: Vec<u8>,
        terms: Vec<u8>,
        intent: CreateMarketIntent,
        bumps: PlaneBumps,
        market: Vec<u8>,
        hoard: Vec<u8>,
        position: Vec<u8>,
        kernel: Vec<u8>,
        external: Vec<u8>,
        replay: Vec<u8>,
        supply: Vec<u8>,
        resolution: Vec<u8>,
    }

    impl Founded {
        fn plane(&self) -> PlaneBytes<'_> {
            PlaneBytes {
                market: &self.market,
                hoard: &self.hoard,
                position: &self.position,
                kernel: &self.kernel,
                external: &self.external,
                replay: &self.replay,
                supply: &self.supply,
                resolution: &self.resolution,
            }
        }

        fn validate(&self) -> Outcome<()> {
            validate_initial_plane(
                &self.realm,
                &self.profile,
                &self.terms,
                self.plane(),
                &self.intent,
                &self.bumps,
            )
        }
    }

    fn owner() -> Hash32 {
        h(31)
    }

    fn hoard_authority() -> Hash32 {
        h(0x40)
    }

    fn founded() -> Founded {
        let profile = profile_hash();
        let terms_value = terms_account(profile);
        let intent = CreateMarketIntent {
            realm: realm_hash(),
            profile,
            market_nonce: MARKET_NONCE,
            outcome_count: OUTCOME_COUNT,
            terms: terms_value.terms,
            feed: terms_value.feed,
        };
        let identities = FoundingIdentities {
            market: market_id(),
            owner: owner(),
            hoard_authority: hoard_authority(),
        };
        let bumps = plane_bumps();

        let terms = encoded(account_len::TERMS, |out| terms_value.encode(out));
        let mut market = vec![0; account_len::MARKET];
        let mut hoard = vec![0; account_len::HOARD];
        let mut position = vec![0; account_len::POSITION];
        let mut kernel = vec![0; KERNEL_ACCOUNT_LEN];
        let mut external = vec![0; EXTERNAL_ACCOUNT_LEN];
        let mut replay = vec![0; REPLAY_ACCOUNT_LEN];
        let mut supply = vec![0; account_len::SUPPLY_LEDGER];
        let mut resolution = vec![0; account_len::RESOLUTION];
        write_initial_plane(
            &terms,
            PlaneWrite {
                market: &mut market,
                hoard: &mut hoard,
                position: &mut position,
                kernel: &mut kernel,
                external: &mut external,
                replay: &mut replay,
                supply: &mut supply,
                resolution: &mut resolution,
            },
            &intent,
            &identities,
            &bumps,
        )
        .expect("the founding write must succeed");

        Founded {
            realm: encoded(account_len::REALM, |out| realm_account().encode(out)),
            profile: encoded(account_len::PROFILE, |out| profile_account().encode(out)),
            terms,
            intent,
            bumps,
            market,
            hoard,
            position,
            kernel,
            external,
            replay,
            supply,
            resolution,
        }
    }

    /* -------------------------------------------------------------------- */
    /* Happy path: byte-exact initialization                                 */
    /* -------------------------------------------------------------------- */

    #[test]
    fn the_founding_write_is_byte_exact_against_independently_encoded_accounts() {
        /* The expectation is the *structs*, encoded by the frozen codecs, not a
         * hex transcript: a change to any field this lane chose shows up here
         * as a struct that no longer matches, with the field named. */
        let founded = founded();
        let market = market_id();
        let realm = realm_hash();
        let profile = profile_hash();
        let terms_value = terms_account(profile);
        let bumps = plane_bumps();

        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        outcomes[0] = canonical_outcome_id(market, 0);
        outcomes[1] = canonical_outcome_id(market, 1);
        let expected_market = MarketAccount {
            market,
            realm,
            profile,
            terms: terms_value.terms,
            outcome_count: OUTCOME_COUNT,
            lifecycle: 0,
            stored_bump: bumps.market,
            hoard_bump: bumps.hoard,
            outcomes,
            feed: terms_value.feed,
            // The terms' digest-committed cap; the slot stays the named zero
            // placeholder.  See the module docs.
            collateral_cap: FIXTURE_CAP,
            created_slot: 0,
            reserved: Hash32::ZERO,
        };
        let expected_hoard = HoardAccount {
            market,
            realm,
            authority: hoard_authority(),
            collateral_atoms: 0,
            stored_bump: bumps.hoard,
            flags: 0,
        };
        let expected_position = PositionAccount {
            market,
            owner: owner(),
            generation: 0,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: bumps.position,
            close_state: 0,
        };
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = PayoutVector::new(1, unit_vector(0));
        vectors[1] = PayoutVector::new(1, unit_vector(1));
        let expected_kernel = KernelAccount {
            market,
            phase: 0,
            resolved_payout: 0,
            payouts: PayoutSet::new(PAYOUT_COUNT, OUTCOME_COUNT, vectors),
            total_supply: [0; MAX_OUTCOMES],
        };
        let expected_external = ExternalAccount {
            market,
            owner: owner(),
            position_generation: 0,
            balances: [0; MAX_OUTCOMES],
            stored_bump: bumps.external,
            flags: 0,
        };
        let expected_replay = ReplayAccount {
            market,
            owner: owner(),
            position_generation: 0,
            sequence: 0,
            stored_bump: bumps.replay,
            flags: 0,
        };
        let expected_supply = SupplyLedgerAccount {
            market,
            realm,
            generation: 0,
            outcome_count: OUTCOME_COUNT,
            internal_supply: [0; MAX_OUTCOMES],
            external_supply: [0; MAX_OUTCOMES],
            stored_bump: bumps.supply,
            flags: 0,
        };
        let expected_resolution = ResolutionAccount {
            market,
            terms: terms_value.terms,
            feed: terms_value.feed,
            window: Hash32::ZERO,
            feed_cursor: 0,
            sealed_end_bucket_exclusive: 0,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            stored_bump: bumps.resolution,
            flags: 0,
        };

        assert_eq!(
            founded.market,
            encoded(account_len::MARKET, |out| expected_market.encode(out))
        );
        assert_eq!(
            founded.hoard,
            encoded(account_len::HOARD, |out| expected_hoard.encode(out))
        );
        assert_eq!(
            founded.position,
            encoded(account_len::POSITION, |out| expected_position.encode(out))
        );
        let mut kernel_bytes = vec![0; KERNEL_ACCOUNT_LEN];
        expected_kernel
            .encode(&mut kernel_bytes)
            .expect("kernel encodes");
        assert_eq!(founded.kernel, kernel_bytes);
        let mut external_bytes = vec![0; EXTERNAL_ACCOUNT_LEN];
        expected_external
            .encode(&mut external_bytes)
            .expect("external encodes");
        assert_eq!(founded.external, external_bytes);
        let mut replay_bytes = vec![0; REPLAY_ACCOUNT_LEN];
        expected_replay
            .encode(&mut replay_bytes)
            .expect("replay encodes");
        assert_eq!(founded.replay, replay_bytes);
        assert_eq!(
            founded.supply,
            encoded(account_len::SUPPLY_LEDGER, |out| expected_supply
                .encode(out))
        );
        assert_eq!(
            founded.resolution,
            encoded(account_len::RESOLUTION, |out| expected_resolution
                .encode(out))
        );
        assert_eq!(founded.validate(), Ok(()));
    }

    #[test]
    fn the_founded_resolution_record_is_unresolved_and_the_market_is_active() {
        let founded = founded();
        let resolution = accounts::read_resolution(&founded.resolution).expect("decodes");
        assert!(!resolution.resolved);
        assert_eq!(resolution.payout_index, PAYOUT_INDEX_UNRESOLVED);
        let market = accounts::read_market(&founded.market).expect("decodes");
        assert_eq!(market.lifecycle, 0);
        /* Fundable by construction: the cap is the terms' own, never zero. */
        assert_eq!(market.collateral_cap, FIXTURE_CAP);
        assert_eq!(
            accounts::read_terms(&founded.terms)
                .expect("decodes")
                .collateral_cap,
            FIXTURE_CAP
        );
        let kernel = accounts::read_kernel(&founded.kernel).expect("decodes");
        assert_eq!(kernel.phase, 0);
        assert_eq!(kernel.total_supply, [0; MAX_OUTCOMES]);
    }

    /* -------------------------------------------------------------------- */
    /* Idempotence                                                           */
    /* -------------------------------------------------------------------- */

    #[test]
    fn re_initializing_a_founded_market_refuses() {
        /* The whole idempotence gate: the account a second `CreateMarket` would
         * write is exactly the account the first one wrote, and it is no longer
         * zero. */
        let founded = founded();
        assert_eq!(require_zeroed(&vec![0; account_len::MARKET]), Ok(()));
        for account in [
            &founded.market,
            &founded.hoard,
            &founded.position,
            &founded.kernel,
            &founded.external,
            &founded.replay,
            &founded.supply,
            &founded.resolution,
        ] {
            assert_eq!(
                require_zeroed(account),
                Err(Refusal::Adapter(ClutchError::AlreadyInitialized))
            );
        }
    }

    #[test]
    fn a_single_nonzero_byte_anywhere_in_a_target_refuses() {
        let mut data = vec![0; account_len::MARKET];
        data[account_len::MARKET - 1] = 1;
        assert_eq!(
            require_zeroed(&data),
            Err(Refusal::Adapter(ClutchError::AlreadyInitialized))
        );
    }

    /* -------------------------------------------------------------------- */
    /* Envelope discipline                                                   */
    /* -------------------------------------------------------------------- */

    fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
        let mut body = [0_u8; clutch_solana_layout::MAX_INTENT_BYTES];
        let len = intent.encode(&mut body).expect("intent encodes");
        let mut out = vec![0xd1, 1];
        out.extend_from_slice(&sequence.to_le_bytes());
        out.push(0);
        out.extend_from_slice(&(len as u16).to_le_bytes());
        out.extend_from_slice(&body[..len]);
        out
    }

    fn create_request(sequence: u64) -> Request {
        let terms_value = terms_account(profile_hash());
        let bytes = layout_request(
            sequence,
            Intent::CreateMarket {
                realm: realm_hash(),
                profile: profile_hash(),
                market_nonce: MARKET_NONCE,
                outcome_count: OUTCOME_COUNT,
                terms: terms_value.terms,
                feed: terms_value.feed,
            },
        );
        Request::decode(&bytes).expect("the create envelope decodes")
    }

    #[test]
    fn the_intent_match_accepts_only_create_market() {
        let request = create_request(0);
        let intent = create_market_intent(&request).expect("create matches");
        assert_eq!(intent.market_nonce, MARKET_NONCE);
        assert_eq!(intent.outcome_count, OUTCOME_COUNT);

        let split = Request::decode(&layout_request(
            0,
            Intent::Split {
                market: market_id(),
                owner: owner(),
                quantity: 1,
            },
        ))
        .expect("split envelope decodes");
        assert_eq!(
            create_market_intent(&split),
            Err(ClutchError::UnsupportedInstruction.into())
        );
        /* And `process` refuses it before touching an account, which is why it
         * can be called here with no accounts at all. */
        assert_eq!(
            process(&Pubkey::new_from_array([1; 32]), &[], &split),
            Err(ClutchError::UnsupportedInstruction.into())
        );
    }

    #[test]
    fn creation_consumes_no_replay_sequence() {
        assert_eq!(require_creation_sequence(0), Ok(()));
        assert_eq!(
            require_creation_sequence(1),
            Err(ClutchError::Replay.into())
        );
        assert_eq!(
            require_creation_sequence(u64::MAX),
            Err(ClutchError::Replay.into())
        );
    }

    /* -------------------------------------------------------------------- */
    /* The refusal table                                                     */
    /* -------------------------------------------------------------------- */

    /// Rewrite one plane account from a mutated struct.
    macro_rules! rewrite {
        ($founded:expr, $field:ident, $ty:ty, $len:expr, $mutate:expr) => {{
            let mut value = <$ty>::decode(&$founded.$field).expect("plane account decodes");
            #[allow(clippy::redundant_closure_call)]
            ($mutate)(&mut value);
            let mut bytes = vec![0; $len];
            value.encode(&mut bytes).expect("mutated account encodes");
            $founded.$field = bytes;
        }};
    }

    #[test]
    fn non_canonical_outcome_ids_refuse() {
        /* The outcome identities are `canonical_outcome_id(market, i)` and the
         * frozen codec owns that rule, so the tamper has to be at the byte
         * level: `MarketAccount::encode` would refuse to produce it. */
        let mut founded = founded();
        let offset = 2 + (4 * 32) + 4;
        founded.market[offset] ^= 0xff;
        assert_eq!(
            founded.validate(),
            Err(Refusal::Codec(CodecError::NonCanonicalIdentity))
        );
    }

    #[test]
    fn a_non_canonical_market_identity_refuses() {
        /* A market whose identity is not `canonical_market_id(realm, profile,
         * nonce)`, but whose outcome identities are canonical *for that wrong
         * identity*, so the codec is satisfied and only the initializer's own
         * derivation check can catch it. */
        let mut founded = founded();
        let forged = canonical_market_id(realm_hash(), profile_hash(), MARKET_NONCE + 1);
        rewrite!(
            founded,
            market,
            MarketAccount,
            account_len::MARKET,
            |market: &mut MarketAccount| {
                market.market = forged;
                market.outcomes[0] = canonical_outcome_id(forged, 0);
                market.outcomes[1] = canonical_outcome_id(forged, 1);
            }
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn every_stored_bump_is_compared_against_the_derivation() {
        for (name, mutate) in [
            ("market", 0_usize),
            ("hoard", 1),
            ("supply", 2),
            ("resolution", 3),
            ("position", 4),
            ("external", 5),
            ("replay", 6),
        ] {
            let mut founded = founded();
            match mutate {
                0 => rewrite!(
                    founded,
                    market,
                    MarketAccount,
                    account_len::MARKET,
                    |v: &mut MarketAccount| v.stored_bump ^= 1
                ),
                1 => rewrite!(
                    founded,
                    hoard,
                    HoardAccount,
                    account_len::HOARD,
                    |v: &mut HoardAccount| v.stored_bump ^= 1
                ),
                2 => rewrite!(
                    founded,
                    supply,
                    SupplyLedgerAccount,
                    account_len::SUPPLY_LEDGER,
                    |v: &mut SupplyLedgerAccount| v.stored_bump ^= 1
                ),
                3 => rewrite!(
                    founded,
                    resolution,
                    ResolutionAccount,
                    account_len::RESOLUTION,
                    |v: &mut ResolutionAccount| v.stored_bump ^= 1
                ),
                4 => rewrite!(
                    founded,
                    position,
                    PositionAccount,
                    account_len::POSITION,
                    |v: &mut PositionAccount| v.stored_bump ^= 1
                ),
                5 => rewrite!(
                    founded,
                    external,
                    ExternalAccount,
                    EXTERNAL_ACCOUNT_LEN,
                    |v: &mut ExternalAccount| v.stored_bump ^= 1
                ),
                _ => rewrite!(
                    founded,
                    replay,
                    ReplayAccount,
                    REPLAY_ACCOUNT_LEN,
                    |v: &mut ReplayAccount| v.stored_bump ^= 1
                ),
            }
            assert_eq!(
                founded.validate(),
                Err(ClutchError::WrongBump.into()),
                "{name} bump must be compared"
            );
        }
    }

    #[test]
    fn a_hoard_bound_to_another_realm_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            hoard,
            HoardAccount,
            account_len::HOARD,
            |v: &mut HoardAccount| v.realm = h(0x77)
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_supply_ledger_bound_to_another_market_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            supply,
            SupplyLedgerAccount,
            account_len::SUPPLY_LEDGER,
            |v: &mut SupplyLedgerAccount| v.market = h(0x78)
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_triple_that_is_not_mutually_bound_refuses() {
        for index in 0..3_usize {
            let mut founded = founded();
            match index {
                0 => rewrite!(
                    founded,
                    external,
                    ExternalAccount,
                    EXTERNAL_ACCOUNT_LEN,
                    |v: &mut ExternalAccount| v.owner = h(0x79)
                ),
                1 => rewrite!(
                    founded,
                    replay,
                    ReplayAccount,
                    REPLAY_ACCOUNT_LEN,
                    |v: &mut ReplayAccount| v.position_generation = 1
                ),
                _ => rewrite!(
                    founded,
                    position,
                    PositionAccount,
                    account_len::POSITION,
                    |v: &mut PositionAccount| v.market = h(0x7a)
                ),
            }
            assert_eq!(
                founded.validate(),
                Err(ClutchError::MismatchedState.into()),
                "triple linkage case {index}"
            );
        }
    }

    #[test]
    fn nonzero_initial_supplies_refuse() {
        let empty = Err(Refusal::Reference(ReferenceError::NonEmptyInitialization));

        let mut ledger = founded();
        rewrite!(
            ledger,
            supply,
            SupplyLedgerAccount,
            account_len::SUPPLY_LEDGER,
            |v: &mut SupplyLedgerAccount| v.internal_supply[0] = 1
        );
        assert_eq!(ledger.validate(), empty);

        let mut external_term = founded();
        rewrite!(
            external_term,
            supply,
            SupplyLedgerAccount,
            account_len::SUPPLY_LEDGER,
            |v: &mut SupplyLedgerAccount| v.external_supply[1] = 1
        );
        assert_eq!(external_term.validate(), empty);

        let mut aggregate = founded();
        rewrite!(
            aggregate,
            kernel,
            KernelAccount,
            KERNEL_ACCOUNT_LEN,
            |v: &mut KernelAccount| v.total_supply[0] = 1
        );
        assert_eq!(aggregate.validate(), empty);

        let mut collateral = founded();
        rewrite!(
            collateral,
            hoard,
            HoardAccount,
            account_len::HOARD,
            |v: &mut HoardAccount| v.collateral_atoms = 1
        );
        assert_eq!(collateral.validate(), empty);
    }

    #[test]
    fn c0_refuses_a_founding_triple_that_is_not_provably_zero() {
        let empty = Err(Refusal::Reference(ReferenceError::NonEmptyInitialization));

        let mut claims = founded();
        rewrite!(
            claims,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| v.internal[0] = 1
        );
        assert_eq!(claims.validate(), empty);

        let mut cash = founded();
        rewrite!(
            cash,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| v.cash_atoms = 1
        );
        assert_eq!(cash.validate(), empty);

        let mut reserved = founded();
        rewrite!(
            reserved,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| {
                v.cash_atoms = 5;
                v.reserved_cash_atoms = 5;
            }
        );
        assert_eq!(reserved.validate(), empty);

        let mut closing = founded();
        rewrite!(
            closing,
            position,
            PositionAccount,
            account_len::POSITION,
            |v: &mut PositionAccount| v.close_state = 1
        );
        assert_eq!(closing.validate(), empty);

        let mut shadow = founded();
        rewrite!(
            shadow,
            external,
            ExternalAccount,
            EXTERNAL_ACCOUNT_LEN,
            |v: &mut ExternalAccount| v.balances[1] = 1
        );
        assert_eq!(shadow.validate(), empty);

        let mut replayed = founded();
        rewrite!(
            replayed,
            replay,
            ReplayAccount,
            REPLAY_ACCOUNT_LEN,
            |v: &mut ReplayAccount| v.sequence = 1
        );
        assert_eq!(replayed.validate(), empty);
    }

    #[test]
    fn an_unfrozen_collateral_policy_refuses_market_initialization() {
        let mut founded = founded();
        let unfrozen = ProfileAccount {
            profile: profile_hash(),
            realm: realm_hash(),
            collateral_policy_digest: Hash32::ZERO,
            version: 1,
            flags: 0,
        };
        founded.profile = encoded(account_len::PROFILE, |out| unfrozen.encode(out));
        assert_eq!(
            founded.validate(),
            Err(Refusal::Adapter(ClutchError::CollateralPolicyNotFrozen))
        );
    }

    #[test]
    fn the_realm_width_gate_is_unreachable_because_the_frozen_codec_pins_it() {
        /* `validate_initial_plane` carries the reference's two width checks --
         * `realm.max_outcomes == MAX_OUTCOMES` and
         * `intent.outcome_count <= realm.max_outcomes` -- and neither can fire,
         * because the frozen codecs refuse both halves first.  That is worth an
         * assertion rather than a comment: if either codec ever loosens, this
         * test fails and the checks above it stop being dead weight. */
        let narrow = RealmAccount {
            realm: realm_hash(),
            profile: profile_hash(),
            max_outcomes: 8,
            profile_version: 1,
            stored_bump: 200,
            flags: 0,
        };
        let mut realm_bytes = [0; account_len::REALM];
        assert_eq!(
            narrow.encode(&mut realm_bytes),
            Err(CodecError::InvalidCount)
        );
        let mut intent_bytes = [0; clutch_solana_layout::MAX_INTENT_BYTES];
        assert_eq!(
            Intent::CreateMarket {
                realm: realm_hash(),
                profile: profile_hash(),
                market_nonce: MARKET_NONCE,
                outcome_count: (MAX_OUTCOMES as u8) + 1,
                terms: h(1),
                feed: h(2),
            }
            .encode(&mut intent_bytes),
            Err(CodecError::InvalidCount)
        );
    }

    #[test]
    fn a_profile_version_the_realm_does_not_expect_refuses() {
        let mut founded = founded();
        let drifted = RealmAccount {
            realm: realm_hash(),
            profile: profile_hash(),
            max_outcomes: MAX_OUTCOMES as u8,
            profile_version: 2,
            stored_bump: 200,
            flags: 0,
        };
        founded.realm = encoded(account_len::REALM, |out| drifted.encode(out));
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn the_kernel_invariant_checker_runs_over_the_founded_state() {
        /* At initialization the only reachable `check_invariants` failure is the
         * outcome-count disagreement: the frozen kernel codec already refuses a
         * malformed payout set at decode, and required collateral over a zero
         * supply is zero.  Both halves are asserted, so neither can silently
         * stop being checked. */
        let mut disagreeing = founded();
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = PayoutVector::new(1, unit_vector(0));
        vectors[1] = PayoutVector::new(1, unit_vector(1));
        rewrite!(
            disagreeing,
            kernel,
            KernelAccount,
            KERNEL_ACCOUNT_LEN,
            |v: &mut KernelAccount| v.payouts = PayoutSet::new(PAYOUT_COUNT, 3, vectors)
        );
        assert_eq!(
            disagreeing.validate(),
            Err(ClutchError::MismatchedState.into())
        );

        /* A payout set that does not sum to its denominator never reaches the
         * invariant checker: `KernelAccount::encode` does not validate, so the
         * bytes exist, and `decode` is what refuses them. */
        let mut malformed = founded();
        let mut bad = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut weights = [0; MAX_OUTCOMES];
        weights[0] = 2;
        bad[0] = PayoutVector::new(1, weights);
        bad[1] = PayoutVector::new(1, unit_vector(1));
        let broken = KernelAccount {
            market: market_id(),
            phase: 0,
            resolved_payout: 0,
            payouts: PayoutSet::new(PAYOUT_COUNT, OUTCOME_COUNT, bad),
            total_supply: [0; MAX_OUTCOMES],
        };
        let mut bytes = vec![0; KERNEL_ACCOUNT_LEN];
        broken.encode(&mut bytes).expect("unvalidated encode");
        malformed.kernel = bytes;
        /* The reference-only kernel codec raises the kernel's own class through
         * its own error type; `error.rs` maps both to the same `0x2004`. */
        assert_eq!(
            malformed.validate(),
            Err(Refusal::Reference(ReferenceError::Kernel(
                clutch_kernel::Error::InvalidPayoutWeights
            )))
        );
    }

    #[test]
    fn a_kernel_payout_set_that_is_not_the_terms_payout_set_refuses() {
        /* Valid on its own, and a valid set for this outcome count -- it is
         * simply not the set the market's own terms digest commits to. */
        let mut founded = founded();
        let mut swapped = [PayoutVector::ZERO; MAX_PAYOUTS];
        swapped[0] = PayoutVector::new(1, unit_vector(1));
        swapped[1] = PayoutVector::new(1, unit_vector(0));
        rewrite!(
            founded,
            kernel,
            KernelAccount,
            KERNEL_ACCOUNT_LEN,
            |v: &mut KernelAccount| v.payouts =
                PayoutSet::new(PAYOUT_COUNT, OUTCOME_COUNT, swapped)
        );
        assert_eq!(
            founded.validate(),
            Err(ClutchError::PayoutSetMismatch.into())
        );
    }

    #[test]
    fn a_terms_artifact_that_does_not_bind_this_market_refuses() {
        let mut founded = founded();
        let other = terms_account(h(0x51));
        founded.terms = encoded(account_len::TERMS, |out| other.encode(out));
        assert_eq!(
            founded.validate(),
            Err(Refusal::Adapter(ClutchError::TermsBindingMismatch))
        );
    }

    #[test]
    fn a_resolution_record_that_is_already_resolved_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            resolution,
            ResolutionAccount,
            account_len::RESOLUTION,
            |v: &mut ResolutionAccount| {
                v.payout_index = 0;
                v.window = h(0x30);
                v.feed_cursor = 130;
                v.sealed_end_bucket_exclusive = 130;
                v.repair_generation = 1;
                v.resolved_slot = 900;
            }
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_resolution_record_bound_to_other_terms_refuses() {
        let mut founded = founded();
        rewrite!(
            founded,
            resolution,
            ResolutionAccount,
            account_len::RESOLUTION,
            |v: &mut ResolutionAccount| v.terms = h(0x52)
        );
        assert_eq!(founded.validate(), Err(ClutchError::MismatchedState.into()));
    }

    #[test]
    fn a_market_cap_that_is_not_the_terms_cap_refuses() {
        /* The cap flow: the founding write copies the terms'
         * digest-committed cap, and the re-validation refuses any other
         * value — a writer cannot invent a risk limit.  The zero case is not
         * writable at all: the terms codec refuses a zero cap, so the old
         * "exists and cannot accept collateral" residue is unfoundable. */
        let mut mismatched = founded();
        rewrite!(
            mismatched,
            market,
            MarketAccount,
            account_len::MARKET,
            |v: &mut MarketAccount| v.collateral_cap = FIXTURE_CAP + 1
        );
        assert_eq!(
            mismatched.validate(),
            Err(Refusal::Adapter(ClutchError::TermsBindingMismatch))
        );

        let mut zeroed = founded();
        rewrite!(
            zeroed,
            market,
            MarketAccount,
            account_len::MARKET,
            |v: &mut MarketAccount| v.collateral_cap = 0
        );
        assert_eq!(
            zeroed.validate(),
            Err(Refusal::Adapter(ClutchError::TermsBindingMismatch))
        );

        let mut undecided = terms_account(profile_hash());
        undecided.collateral_cap = 0;
        undecided.terms = undecided.recomputed_terms_digest().expect("digest");
        assert_eq!(undecided.validate(), Err(CodecError::ZeroValue));
    }

    #[test]
    fn an_intent_that_does_not_describe_the_written_market_refuses() {
        for index in 0..4_usize {
            let mut founded = founded();
            match index {
                0 => founded.intent.market_nonce += 1,
                1 => founded.intent.outcome_count = 3,
                2 => founded.intent.feed = h(0x53),
                _ => founded.intent.realm = h(0x54),
            }
            assert_eq!(
                founded.validate(),
                Err(ClutchError::MismatchedState.into()),
                "intent case {index}"
            );
        }
    }
}
