//! Shared hostile-account validation plane.
//!
//! Every instruction family in this program reaches its transition through this
//! module.  It owns exactly three things, and deliberately owns nothing else:
//!
//! 1. authentication of runtime-supplied [`AccountInfo`] metadata — account
//!    count, signatures, role aliasing, program ownership, the executable bit,
//!    declared writability, and exact data length;
//! 2. comparison of a supplied account key and stored bump against a
//!    program-derived address computed from [`crate::seeds`]; and
//! 3. decoding each account through its frozen codec and returning the small
//!    set of facts a transition needs.
//!
//! It contains **no** semantic or economic logic.  Balances, supplies,
//! collateral, and invariants belong to [`clutch_kernel`]; byte ownership
//! belongs to [`clutch_solana_layout`] and to the reference-only codecs in
//! [`clutch_solana_reference`].  Nothing here decides whether a transition is
//! allowed; it decides only whether the bytes and the metadata are what they
//! claim to be.
//!
//! ## Why the readers return facts instead of accounts
//!
//! SBF gives each call frame a hard 4 KiB budget, and several of these decoded
//! accounts are large fixed-size values: `clutch_solana_layout::account_len`
//! puts the order page and the immutable terms artifact in the kilobytes, and
//! the order page alone is within the same order of magnitude as the whole
//! frame.  Every reader is therefore `#[inline(never)]`, so that the large
//! decoded value lives in its own frame and only a small facts structure
//! crosses back to the caller.  A transition that holds two or three whole
//! accounts by value at once does not fit, which is the same finding
//! `docs/implementation/SBF_BRINGUP.md` records against the offline adapter's
//! `apply`.
//!
//! ## Reader coverage is not check coverage
//!
//! A reader existing for an account means the account can be decoded and
//! authenticated.  It does **not** mean any instruction binds it, writes it, or
//! checks it against anything else.  The wave-3 readers below are foundation
//! for the per-instruction lanes; today nothing in [`crate::dispatch`] calls
//! them.

use crate::error::{ClutchError, Refusal};
use clutch_kernel::BasisMode;
use clutch_solana_layout::{
    direct_selection::{DirectEpochV3Account, DIRECT_EPOCH_BYTES},
    stream, CandidateRecord, EpochAccount, FeedAccount, FinalPotAccount, Hash32, MarketAccount,
    PriceGridAccount, ProfileAccount, RealmAccount, ResolutionAccount, SettlementReceiptAccount,
    SupplyLedgerAccount, TermsAccount, MAX_OUTCOMES,
};
use clutch_solana_reference::KernelAccount;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// The refusal-carrying result type used throughout this program.
pub type Outcome<T> = core::result::Result<T, Refusal>;

/// Refuse with `error` unless `condition` holds.
pub fn require(condition: bool, error: ClutchError) -> Outcome<()> {
    if condition {
        Ok(())
    } else {
        Err(error.into())
    }
}

/// One program-owned state account in an instruction's fixed account list.
///
/// A role is a *declaration*, not a request: `writable` is what the instruction
/// requires the runtime to have been told, so a read-only role that arrives
/// declared writable is refused rather than quietly accepted.  Declaring a
/// wider account list than an instruction needs is how a remaining-account
/// shuffle smuggles a writable account into a later instruction, so the role
/// table is exhaustive and the account count is exact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateRole {
    /// Position in the instruction's account list.
    pub index: usize,
    /// Exact data length this role must have.
    pub len: usize,
    /// Whether the instruction requires the account to be declared writable.
    pub writable: bool,
}

impl StateRole {
    /// A role the instruction reads and must not be allowed to write.
    pub const fn read_only(index: usize, len: usize) -> Self {
        Self {
            index,
            len,
            writable: false,
        }
    }

    /// A role the instruction mutates.
    pub const fn writable(index: usize, len: usize) -> Self {
        Self {
            index,
            len,
            writable: true,
        }
    }
}

/// Refuse an account list whose length is not exactly `expected`.
pub fn require_count(accounts: &[AccountInfo], expected: usize) -> Outcome<()> {
    require(accounts.len() == expected, ClutchError::AccountCount)
}

/// Refuse an actor the runtime did not authenticate a signature for.
pub fn require_signer(account: &AccountInfo) -> Outcome<()> {
    require(account.is_signer, ClutchError::MissingSignature)
}

/// Refuse any two roles filled by one account key.
///
/// A writable alias would let one logical debit or credit land twice, which is
/// obligation 3 of `docs/implementation/SOLANA_REFERENCE_ADAPTER.md`.  The
/// comparison is over the whole list, actor included, because an actor that is
/// also a state account is the same aliasing problem.
pub fn require_distinct(accounts: &[AccountInfo]) -> Outcome<()> {
    let count = accounts.len();
    let mut left = 0_usize;
    while left < count {
        let mut right = left + 1;
        while right < count {
            require(
                accounts[left].key != accounts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Authenticate the metadata of every program-owned state role.
///
/// Two passes, in this order: program ownership, the executable bit, and
/// declared writability for every role; then exact data length for every role.
/// Length is checked after ownership on purpose — a wrong-length account that
/// this program does not even own should be reported as the ownership failure
/// it is.  Both passes run in the order of the `roles` slice, so a caller that
/// lists roles in account-index order gets refusals in account-index order.
///
/// The codecs re-check length, discriminator, and version, but checking length
/// here first means a wrong-length account never reaches a codec at all.
pub fn validate_state_roles(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    roles: &[StateRole],
) -> Outcome<()> {
    for role in roles {
        let account = &accounts[role.index];
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.executable, ClutchError::ExecutableAccount)?;
        if role.writable {
            require(account.is_writable, ClutchError::NotWritable)?;
        } else {
            require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        }
    }
    for role in roles {
        require(
            accounts[role.index].data_len() == role.len,
            ClutchError::WrongDataLength,
        )?;
    }
    Ok(())
}

/// Authenticate one program-owned role whose exact layout version selects one
/// of a small, explicitly listed account lengths.
pub fn validate_state_role_lengths(
    program_id: &Pubkey,
    account: &AccountInfo,
    writable: bool,
    lengths: &[usize],
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    if writable {
        require(account.is_writable, ClutchError::NotWritable)?;
    } else {
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    }
    require(
        lengths.iter().any(|length| *length == account.data_len()),
        ClutchError::WrongDataLength,
    )
}

/// Compare a supplied account key, and optionally a stored bump, against a
/// canonical derivation.
///
/// Caller-supplied expected keys are never accepted anywhere in this program:
/// `derived` must come from [`crate::seeds`], which recomputes the address from
/// the frozen seed schema.  `stored_bump` is `None` only for the two accounts
/// whose frozen layout carries no bump field — Profile and the reference-only
/// kernel aggregate — and that gap is listed in `SBF_BRINGUP.md`.
pub fn expect_pda(actual: &Pubkey, derived: (Pubkey, u8), stored_bump: Option<u8>) -> Outcome<()> {
    require(*actual == derived.0, ClutchError::WrongPda)?;
    match stored_bump {
        Some(bump) => require(bump == derived.1, ClutchError::WrongBump),
        None => Ok(()),
    }
}

/* ------------------------------------------------------------------------ */
/* Core plane readers                                                        */
/* ------------------------------------------------------------------------ */

/// Realm facts a transition binds against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmFacts {
    /// Realm identity.
    pub realm: Hash32,
    /// Profile identity this Realm commits to.
    pub profile: Hash32,
    /// Largest outcome count this Realm admits.
    pub max_outcomes: u8,
    /// Profile version this Realm expects.
    pub profile_version: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Profile facts a transition binds against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileFacts {
    /// Profile identity.
    pub profile: Hash32,
    /// Realm namespace.
    pub realm: Hash32,
    /// Exact CollateralPolicy V2 content identity.
    pub collateral_policy_id: Hash32,
    /// Exact compiled AdapterRelease V2 content identity.
    pub adapter_release_id: Hash32,
    /// Profile version.
    pub version: u8,
}

/// Market facts a transition binds against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFacts {
    /// Market identity.
    pub market: Hash32,
    /// Realm namespace.
    pub realm: Hash32,
    /// Profile identity.
    pub profile: Hash32,
    /// Immutable terms digest this market binds.
    pub terms: Hash32,
    /// Feed identity this market resolves against.
    pub feed: Hash32,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Lifecycle: zero active, one resolved.
    pub lifecycle: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Canonical Hoard bump this market claims.
    pub hoard_bump: u8,
    /// Immutable collateral cap.
    pub collateral_cap: u64,
}

/// Reference-only kernel-aggregate facts a transition binds against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelFacts {
    /// Market identity.
    pub market: Hash32,
    /// Kernel phase: zero active, one resolved.
    pub phase: u8,
    /// Immutable resolution mode copied from validated Terms at creation.
    pub basis_mode: BasisMode,
    /// Outcome count the frozen payout set was built over.
    pub payout_outcomes: u8,
    /// Aggregate internal plus external supply by outcome.
    pub total_supply: [u64; MAX_OUTCOMES],
}

/// Decode a Realm account.
#[inline(never)]
pub fn read_realm(data: &[u8]) -> Outcome<RealmFacts> {
    let value = RealmAccount::decode(data)?;
    Ok(RealmFacts {
        realm: value.realm,
        profile: value.profile,
        max_outcomes: value.max_outcomes,
        profile_version: value.profile_version,
        stored_bump: value.stored_bump,
    })
}

/// Decode a Profile account.
#[inline(never)]
pub fn read_profile(data: &[u8]) -> Outcome<ProfileFacts> {
    let value = ProfileAccount::decode(data)?;
    Ok(ProfileFacts {
        profile: value.profile,
        realm: value.realm,
        collateral_policy_id: value.collateral_policy_id,
        adapter_release_id: value.adapter_release_id,
        version: value.version,
    })
}

/// Decode a Market account.
#[inline(never)]
pub fn read_market(data: &[u8]) -> Outcome<MarketFacts> {
    let value = MarketAccount::decode(data)?;
    Ok(MarketFacts {
        market: value.market,
        realm: value.realm,
        profile: value.profile,
        terms: value.terms,
        feed: value.feed,
        outcome_count: value.outcome_count,
        lifecycle: value.lifecycle,
        stored_bump: value.stored_bump,
        hoard_bump: value.hoard_bump,
        collateral_cap: value.collateral_cap,
    })
}

/// Decode a reference-only kernel-aggregate account.
#[inline(never)]
pub fn read_kernel(data: &[u8]) -> Outcome<KernelFacts> {
    let value = KernelAccount::decode(data)?;
    Ok(KernelFacts {
        market: value.market,
        phase: value.phase,
        basis_mode: value.basis_mode,
        payout_outcomes: value.payouts.outcomes,
        total_supply: value.total_supply,
    })
}

/* ------------------------------------------------------------------------ */
/* CLO-DELTA-V1 closure primitives                                           */
/* ------------------------------------------------------------------------ */

/* The three checked obligations of `docs/implementation/MULTI_POSITION_CLOSURE.md`
 * section 3, in the shapes `clutch_solana_reference` uses, so that an
 * instruction lane composes the same checks under the same names instead of
 * re-deriving them.  They are *validation*, not semantics: none of them decides
 * what a transition may do, only whether the market aggregate and the one
 * presented position triple can both be true at once.
 *
 * `split.rs` (Split/Merge/Materialize/Dematerialize) and `market_init.rs`
 * compose them; `observe_resolve.rs` currently carries its own inline
 * transcription of C1+C2 (`check_closure`) rather than composing these —
 * one function, two in-program transcriptions, held together by the SVM
 * differential.  See `SBF_BRINGUP.md`. */

/// C1: the two-term ledger closes against the kernel aggregate.
///
/// `internal_supply[o] + external_supply[o] == total_supply[o]` for every active
/// outcome.  Checked before a transition and again over the post-state, which
/// is what gives C3 its teeth: a kernel whose aggregate effect diverged from
/// its per-position effect refuses here rather than corrupting the ledger.
///
/// Padding beyond the active outcome count is not re-checked; the frozen codec
/// already refuses a ledger with non-canonical padding.
pub fn require_two_term_closure(
    supply: &SupplyFacts,
    kernel: &KernelFacts,
    outcome_count: u8,
) -> Outcome<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        let aggregate = supply.internal_supply[outcome]
            .checked_add(supply.external_supply[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            aggregate == kernel.total_supply[outcome],
            ClutchError::AggregateClosureMismatch,
        )?;
        outcome += 1;
    }
    Ok(())
}

/// C2: the presented position triple is bounded by the market aggregate.
///
/// `position.internal[o] <= internal_supply[o]` and
/// `external.balances[o] <= external_supply[o]`.  This is the part of "every
/// balance is represented in the ledger" that a single transition can decide
/// about the one triple it sees; the full sum over all positions is a statement
/// about histories, maintained by induction and never scanned.
///
/// It deliberately replaces the older equality against the aggregate.  An
/// equality makes a second position unrepresentable, which is exactly why it
/// had to go.
pub fn require_representation_bound(
    supply: &SupplyFacts,
    internal: &[u64; MAX_OUTCOMES],
    external: &[u64; MAX_OUTCOMES],
    outcome_count: u8,
) -> Outcome<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        require(
            internal[outcome] <= supply.internal_supply[outcome]
                && external[outcome] <= supply.external_supply[outcome],
            ClutchError::AggregateClosureMismatch,
        )?;
        outcome += 1;
    }
    Ok(())
}

/// C3: move one ledger term by exactly the delta applied to the position.
///
/// `ledger' = ledger - pre + post` per active outcome, with checked arithmetic.
/// The ledger is **never** overwritten with the presented position: overwriting
/// is what made the single-position model single-position.  Call it once for the
/// internal term and once for the external term.
///
/// An underflow means the ledger did not contain the balance the position
/// claims to be giving up, which is a closure failure and not an arithmetic
/// accident, so it refuses [`ClutchError::AggregateClosureMismatch`]; a genuine
/// overflow refuses [`ClutchError::Arithmetic`].  Both distinctions mirror the
/// offline reference adapter exactly.
pub fn apply_ledger_delta(
    ledger_term: &mut [u64; MAX_OUTCOMES],
    outcome_count: u8,
    pre: &[u64; MAX_OUTCOMES],
    post: &[u64; MAX_OUTCOMES],
) -> Outcome<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        ledger_term[outcome] = ledger_term[outcome]
            .checked_sub(pre[outcome])
            .ok_or(Refusal::Adapter(ClutchError::AggregateClosureMismatch))?
            .checked_add(post[outcome])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        outcome += 1;
    }
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Wave-3 plane readers                                                      */
/* ------------------------------------------------------------------------ */

/// Feed-head facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeedFacts {
    /// Feed identity.
    pub feed: Hash32,
    /// Realm namespace.
    pub realm: Hash32,
    /// Accepted cursor.
    pub cursor: u64,
    /// Next logical boundary.
    pub next_boundary: u64,
    /// Archive page count.
    pub archive_pages: u64,
    /// Digest of the checked accumulator summary.
    pub summary: Hash32,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Market-wide supply-ledger facts.
///
/// This is the two-term ledger: claims credited inside the internal ledger and
/// claims materialized outside it.  It is the market-wide aggregate and is
/// never authority over any single position's balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupplyFacts {
    /// Market identity.
    pub market: Hash32,
    /// Realm namespace.
    pub realm: Hash32,
    /// Accounting-era generation.
    pub generation: u64,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Claims credited inside the internal ledger, per outcome.
    pub internal_supply: [u64; MAX_OUTCOMES],
    /// Claims materialized externally and accounted, per outcome.
    pub external_supply: [u64; MAX_OUTCOMES],
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Immutable-terms facts.
///
/// The payout vectors themselves are deliberately not carried out of the
/// reader: the whole `MAX_PAYOUTS`-wide set does not belong in a caller's
/// frame, and a transition that needs one vector should decode inside its own
/// `#[inline(never)]` helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TermsFacts {
    /// Terms digest; equals the canonical digest of the terms body.
    pub terms: Hash32,
    /// Realm the terms were authored under.
    pub realm: Hash32,
    /// Immutable collateral/profile identity.
    pub profile: Hash32,
    /// Feed identity the window is evaluated against.
    pub feed: Hash32,
    /// Frozen price-grid identity the order limit domain lives on.
    pub price_grid: Hash32,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Active payout-vector count.
    pub payout_count: u8,
    /// Frozen B-spline degree; zero selects finite presets, one through three native basis mode.
    pub basis_degree: u8,
    /// Digest-bound resolution statistic identity.
    pub statistic_id: u16,
    /// Per-market collateral cap the terms digest commits to.
    pub collateral_cap: u64,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Frozen price-grid facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PriceGridFacts {
    /// Grid identity; equals the canonical digest of the grid body.
    pub grid: Hash32,
    /// Realm namespace.
    pub realm: Hash32,
    /// Exact integer price scale.
    pub price_scale: u64,
    /// Active tick count.
    pub tick_count: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Resolution-record facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionFacts {
    /// Market identity.
    pub market: Hash32,
    /// Immutable terms digest whose payout set the index refers to.
    pub terms: Hash32,
    /// Feed identity the sealed window came from.
    pub feed: Hash32,
    /// Digest of the sealed window result.
    pub window: Hash32,
    /// Accepted feed cursor at seal.
    pub feed_cursor: u64,
    /// Exclusive end bucket of the sealed window.
    pub sealed_end_bucket_exclusive: u64,
    /// Repair generation of the sealed window.
    pub repair_generation: u64,
    /// Slot the resolution was recorded in.
    pub resolved_slot: u64,
    /// Selected payout index, or `PAYOUT_INDEX_UNRESOLVED`.
    pub payout_index: u8,
    /// Whether a payout vector has been selected.
    pub resolved: bool,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Epoch/book-domain facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochFacts {
    /// Canonical epoch identity.
    pub epoch: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Book identity within the market.
    pub book: Hash32,
    /// Immutable terms digest this epoch clears under.
    pub terms: Hash32,
    /// Frozen price-grid identity.
    pub price_grid: Hash32,
    /// Opaque frozen-policy identity.
    pub policy: Hash32,
    /// Set-wide order-set digest; zero until the page set is frozen.
    pub order_set: Hash32,
    /// Epoch index within the market.
    pub epoch_index: u64,
    /// Exact integer price scale.
    pub price_scale: u64,
    /// Frozen page count; zero until frozen.
    pub page_count: u16,
    /// Frozen order count; zero until frozen.
    pub order_count: u16,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Lifecycle phase; see the layout crate's `EPOCH_PHASE_*` constants.
    pub phase: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Order-page facts.
///
/// The 16 order records are deliberately not carried out of the reader; see
/// [`TermsFacts`] for the same reason.  What is carried is the whole page-set
/// commitment, which is what makes cross-page range, uniqueness, and closure
/// checkable without holding the records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderPageFacts {
    /// Market identity.
    pub market: Hash32,
    /// Epoch identity.
    pub epoch: Hash32,
    /// Set-wide order-set digest; zero until the page set is frozen.
    pub order_set: Hash32,
    /// Digest of this page's position and record bytes.
    pub page_digest: Hash32,
    /// This page's lowest order id; zero exactly when the page is empty.
    pub first_order_id: Hash32,
    /// This page's highest order id; zero exactly when the page is empty.
    pub last_order_id: Hash32,
    /// The previous page's last order id; zero exactly on page zero.
    pub prev_page_last_order_id: Hash32,
    /// Zero-based page index.
    pub page_index: u16,
    /// Total frozen page count.
    pub page_count: u16,
    /// Orders across the whole page set; zero until the set is frozen.
    pub set_order_count: u16,
    /// Number of populated records on this page.
    pub order_count: u8,
    /// Freeze state: zero open, one frozen.
    pub frozen: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Candidate-record facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFacts {
    /// Candidate identity; equals the digest of its free coordinates.
    pub candidate: Hash32,
    /// Epoch identity.
    pub epoch: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Slot the candidate was submitted in.
    pub submitted_slot: u64,
    /// Distinct participating owners.
    pub distinct_owners: u16,
    /// Orders this candidate binds; must equal the frozen book length.
    pub order_len: u8,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Status; see the layout crate's `CANDIDATE_STATUS_*` constants.
    pub status: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Final-pot facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotFacts {
    /// Epoch identity.
    pub epoch: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Selected candidate identity.
    pub candidate: Hash32,
    /// Pot cash in exact price units.
    pub pot_cash_price_units: u128,
    /// Remainder atoms of the one named rounding boundary, in price units.
    pub rounding_pot_price_units: u128,
    /// Active outcome count.
    pub outcome_count: u8,
    /// Pot phase; see the layout crate's `POT_PHASE_*` constants.
    pub phase: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Settlement-receipt facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptFacts {
    /// Epoch identity.
    pub epoch: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Selected candidate identity.
    pub candidate: Hash32,
    /// Buy-side order id; zero exactly when the buy end is the virtual merge.
    pub buy_order_id: Hash32,
    /// Sell-side order id; zero exactly when the sell end is the virtual split.
    pub sell_order_id: Hash32,
    /// Exact consideration in price units.
    pub consideration_price_units: u128,
    /// Slice quantity in claim atoms.
    pub quantity: u64,
    /// Cumulative settled quantity, never above `quantity`.
    pub settled_quantity: u64,
    /// The outcome's scaled price, frozen at clear time.
    pub price: u64,
    /// Monotone settlement sequence for this candidate.
    pub sequence: u64,
    /// Index of the frozen slice this receipt settles.
    pub slice_index: u16,
    /// Bound outcome of both ends.
    pub outcome: u8,
    /// Leg kind; see the layout crate's `RECEIPT_LEG_*` constants.
    pub leg_kind: u8,
    /// Consumption flags; see the layout crate's `RECEIPT_FLAG_*` constants.
    pub consumed_flags: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

/// Decode a feed-head account.
#[inline(never)]
pub fn read_feed(data: &[u8]) -> Outcome<FeedFacts> {
    let value = FeedAccount::decode(data)?;
    Ok(FeedFacts {
        feed: value.feed,
        realm: value.realm,
        cursor: value.cursor,
        next_boundary: value.next_boundary,
        archive_pages: value.archive_pages,
        summary: value.summary,
        stored_bump: value.stored_bump,
    })
}

/// Decode a market-wide supply-ledger account.
#[inline(never)]
pub fn read_supply(data: &[u8]) -> Outcome<SupplyFacts> {
    let value = SupplyLedgerAccount::decode(data)?;
    Ok(SupplyFacts {
        market: value.market,
        realm: value.realm,
        generation: value.generation,
        outcome_count: value.outcome_count,
        internal_supply: value.internal_supply,
        external_supply: value.external_supply,
        stored_bump: value.stored_bump,
    })
}

/// Boxed [`read_market`]: the facts live in this helper's frame and only a
/// heap pointer crosses back into the caller's bounded SBF frame (the
/// `direct_selection_v3::common` discipline).
#[inline(never)]
pub fn read_market_boxed(data: &[u8]) -> Outcome<Box<MarketFacts>> {
    Ok(Box::new(read_market(data)?))
}

/// Boxed [`read_terms`] for the same SBF frame discipline.
#[inline(never)]
pub fn read_terms_boxed(data: &[u8]) -> Outcome<Box<TermsFacts>> {
    Ok(Box::new(read_terms(data)?))
}

/// Decode an immutable-terms account.
#[inline(never)]
pub fn read_terms(data: &[u8]) -> Outcome<TermsFacts> {
    /* The one FULL terms decode of a transaction: digest recomputation
     * included.  The `_into` shape keeps this frame at one account copy. */
    let mut value = TermsAccount::ZEROED;
    TermsAccount::decode_into(data, &mut value)?;
    Ok(TermsFacts {
        terms: value.terms,
        realm: value.realm,
        profile: value.profile,
        feed: value.feed,
        price_grid: value.price_grid,
        outcome_count: value.outcome_count,
        payout_count: value.payout_count,
        basis_degree: value.basis_degree,
        statistic_id: value.statistic_id,
        collateral_cap: value.collateral_cap,
        stored_bump: value.stored_bump,
    })
}

/// Decode a frozen price-grid account.
#[inline(never)]
pub fn read_price_grid(data: &[u8]) -> Outcome<PriceGridFacts> {
    let value = PriceGridAccount::decode(data)?;
    Ok(PriceGridFacts {
        grid: value.grid,
        realm: value.realm,
        price_scale: value.price_scale,
        tick_count: value.tick_count,
        stored_bump: value.stored_bump,
    })
}

/// Decode a resolution-record account.
#[inline(never)]
pub fn read_resolution(data: &[u8]) -> Outcome<ResolutionFacts> {
    let value = ResolutionAccount::decode(data)?;
    Ok(ResolutionFacts {
        market: value.market,
        terms: value.terms,
        feed: value.feed,
        window: value.window,
        feed_cursor: value.feed_cursor,
        sealed_end_bucket_exclusive: value.sealed_end_bucket_exclusive,
        repair_generation: value.repair_generation,
        resolved_slot: value.resolved_slot,
        payout_index: value.payout_index,
        resolved: value.is_resolved(),
        stored_bump: value.stored_bump,
    })
}

/// Decode an epoch/book-domain account.
#[inline(never)]
pub fn read_epoch(data: &[u8]) -> Outcome<EpochFacts> {
    #[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
    {
        let _ = data;
        return Err(ClutchError::UnsupportedInstruction.into());
    }
    #[cfg(feature = "profile-full")]
    if data.len() == DIRECT_EPOCH_BYTES {
        let value = DirectEpochV3Account::decode(data)?;
        return Ok(epoch_facts(&value.common));
    }
    #[cfg(feature = "profile-direct-v3-source-v2-point")]
    {
        let value = DirectEpochV3Account::decode(data)?;
        return Ok(epoch_facts(&value.common));
    }
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    let value = EpochAccount::decode(data)?;
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    Ok(epoch_facts(&value))
}

fn epoch_facts(value: &EpochAccount) -> EpochFacts {
    EpochFacts {
        epoch: value.epoch,
        market: value.market,
        book: value.book,
        terms: value.terms,
        price_grid: value.price_grid,
        policy: value.policy,
        order_set: value.order_set,
        epoch_index: value.epoch_index,
        price_scale: value.price_scale,
        page_count: value.page_count,
        order_count: value.order_count,
        outcome_count: value.outcome_count,
        phase: value.phase,
        stored_bump: value.stored_bump,
    }
}

/// Decode exactly a version-three direct Epoch.
#[inline(never)]
pub fn read_direct_epoch(data: &[u8]) -> Outcome<DirectEpochV3Account> {
    Ok(DirectEpochV3Account::decode(data)?)
}

/// Boxed [`read_direct_epoch`]: the decoded Epoch lives in this helper's
/// frame and only a heap pointer crosses back into the caller's bounded SBF
/// frame (the `direct_selection_v3::common` discipline).
#[inline(never)]
pub fn read_direct_epoch_boxed(data: &[u8]) -> Outcome<Box<DirectEpochV3Account>> {
    Ok(Box::new(read_direct_epoch(data)?))
}

/* The generic Epoch reader above intentionally returns only the common facts
 * used by placement, cancellation and page construction. Direct candidate
 * actions must call `read_direct_epoch` so a V2 account cannot gain a schedule
 * by defaulting absent fields. */

/// Decode an order-page account, streaming.  **This now runs on SBF.**
///
/// The order page is the largest account in the protocol —
/// `MAX_ORDERS_PER_PAGE` slots of `ORDER_SLOT_BYTES` each plus the page-set
/// commitment — and the *buffered* decoder does not fit an SBF call frame.
/// That was measured, not assumed: `clutch_solana_layout::OrderPageAccount`'s
/// own `decode` is reported at an estimated 8640-byte frame and
/// `decode_on_grid` at 8320, because both build a whole page by value before
/// returning it, and a wrapper around either was reported at 8192.  This
/// function was therefore compiled off-chain only, and the whole
/// batch-auction instruction family was blocked behind it.
///
/// It no longer is.  `clutch_solana_layout::stream::verify_page` returns
/// **exactly** what `OrderPageAccount::decode` returns — the identical
/// `CodecError`, or the same page's header fields — while never holding more
/// than one `ORDER_SLOT_BYTES` slot: the header is read on its own, the slot
/// array is swept once structurally while the page digest is folded, and it is
/// walked again for record semantics and the order-id chain.  Every field of
/// [`OrderPageFacts`] is a header field, so the mapping below is one-to-one
/// and this reader loses nothing by never materializing the records.
///
/// The 16 records are still deliberately not carried out; a caller that needs
/// them opens a `clutch_solana_layout::stream::OrderSlotCursor` over the same
/// bytes, which costs one slot of stack per step.  A caller that needs only
/// the addressing fields — `epoch`, `page_index`, `stored_bump` — should read
/// `stream::OrderPageHeader::decode` instead, which reads the fixed 235-byte
/// header and folds no digest at all.
#[inline(never)]
pub fn read_order_page(data: &[u8]) -> Outcome<OrderPageFacts> {
    let value = stream::verify_page(data)?;
    Ok(OrderPageFacts {
        market: value.market,
        epoch: value.epoch,
        order_set: value.order_set,
        page_digest: value.page_digest,
        first_order_id: value.first_order_id,
        last_order_id: value.last_order_id,
        prev_page_last_order_id: value.prev_page_last_order_id,
        page_index: value.page_index,
        page_count: value.page_count,
        set_order_count: value.set_order_count,
        order_count: value.order_count,
        frozen: value.frozen,
        stored_bump: value.stored_bump,
    })
}

/// Decode a candidate-record account.
#[inline(never)]
pub fn read_candidate(data: &[u8]) -> Outcome<CandidateFacts> {
    let value = CandidateRecord::decode(data)?;
    Ok(CandidateFacts {
        candidate: value.candidate,
        epoch: value.epoch,
        market: value.market,
        submitted_slot: value.submitted_slot,
        distinct_owners: value.distinct_owners,
        order_len: value.order_len,
        outcome_count: value.outcome_count,
        status: value.status,
        stored_bump: value.stored_bump,
    })
}

/// Decode a final-pot account.
#[inline(never)]
pub fn read_final_pot(data: &[u8]) -> Outcome<FinalPotFacts> {
    let value = FinalPotAccount::decode(data)?;
    Ok(FinalPotFacts {
        epoch: value.epoch,
        market: value.market,
        candidate: value.candidate,
        pot_cash_price_units: value.pot_cash_price_units,
        rounding_pot_price_units: value.rounding_pot_price_units,
        outcome_count: value.outcome_count,
        phase: value.phase,
        stored_bump: value.stored_bump,
    })
}

/// Decode a settlement-receipt account.
#[inline(never)]
pub fn read_receipt(data: &[u8]) -> Outcome<ReceiptFacts> {
    let value = SettlementReceiptAccount::decode(data)?;
    Ok(ReceiptFacts {
        epoch: value.epoch,
        market: value.market,
        candidate: value.candidate,
        buy_order_id: value.buy_order_id,
        sell_order_id: value.sell_order_id,
        consideration_price_units: value.consideration_price_units,
        quantity: value.quantity,
        settled_quantity: value.settled_quantity,
        price: value.price,
        sequence: value.sequence,
        slice_index: value.slice_index,
        outcome: value.outcome,
        leg_kind: value.leg_kind,
        consumed_flags: value.consumed_flags,
        stored_bump: value.stored_bump,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        account_len, canonical_epoch_id, CodecError, OrderPageAccount, OrderSlot,
        PayoutVectorBytes, MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, MAX_PAYOUTS,
        PAYOUT_INDEX_UNRESOLVED,
    };

    /* These tests run on the host, where `seeds::find` is not compiled (see the
     * module docs of `crate::seeds`).  They therefore cover exactly the part of
     * this module that needs no address derivation: metadata authentication,
     * the bump/key comparison given an already-derived address, and every
     * decoder.  The transition itself is still only exercised by the SVM
     * differential; see deferred check 12 in `SBF_BRINGUP.md`. */

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    /// Owned backing store for one host-side `AccountInfo`.
    struct Cell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        is_signer: bool,
        is_writable: bool,
        executable: bool,
    }

    impl Cell {
        fn state(key_byte: u8, owner: Pubkey, len: usize, is_writable: bool) -> Self {
            Self {
                key: key(key_byte),
                owner,
                lamports: 1,
                data: vec![0; len],
                is_signer: false,
                is_writable,
                executable: false,
            }
        }

        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                self.is_signer,
                self.is_writable,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                self.executable,
            )
        }
    }

    #[test]
    fn state_roles_refuse_a_foreign_owner_an_executable_and_a_wrong_length() {
        let program = key(0xaa);
        let roles = [StateRole::writable(0, 8)];

        let mut good = Cell::state(1, program, 8, true);
        assert_eq!(
            validate_state_roles(&program, &[good.info()], &roles),
            Ok(())
        );

        let mut foreign = Cell::state(1, key(0xbb), 8, true);
        assert_eq!(
            validate_state_roles(&program, &[foreign.info()], &roles),
            Err(ClutchError::WrongProgramOwner.into())
        );

        let mut executable = Cell::state(1, program, 8, true);
        executable.executable = true;
        assert_eq!(
            validate_state_roles(&program, &[executable.info()], &roles),
            Err(ClutchError::ExecutableAccount.into())
        );

        let mut short = Cell::state(1, program, 7, true);
        assert_eq!(
            validate_state_roles(&program, &[short.info()], &roles),
            Err(ClutchError::WrongDataLength.into())
        );
    }

    #[test]
    fn state_roles_refuse_both_directions_of_a_writability_mismatch() {
        let program = key(0xaa);

        let mut read_only_arrived_writable = Cell::state(1, program, 8, true);
        assert_eq!(
            validate_state_roles(
                &program,
                &[read_only_arrived_writable.info()],
                &[StateRole::read_only(0, 8)],
            ),
            Err(ClutchError::UnexpectedWritable.into())
        );

        let mut writable_arrived_read_only = Cell::state(1, program, 8, false);
        assert_eq!(
            validate_state_roles(
                &program,
                &[writable_arrived_read_only.info()],
                &[StateRole::writable(0, 8)],
            ),
            Err(ClutchError::NotWritable.into())
        );
    }

    #[test]
    fn ownership_is_reported_before_length() {
        /* A wrong-length account this program does not own is an ownership
         * failure, not a length failure.  Getting this backwards would leak
         * "the length I expect" for accounts belonging to other programs. */
        let program = key(0xaa);
        let mut foreign_and_short = Cell::state(1, key(0xbb), 3, true);
        assert_eq!(
            validate_state_roles(
                &program,
                &[foreign_and_short.info()],
                &[StateRole::writable(0, 8)],
            ),
            Err(ClutchError::WrongProgramOwner.into())
        );
    }

    #[test]
    fn aliasing_and_signature_and_count_refuse() {
        let program = key(0xaa);
        let mut first = Cell::state(1, program, 8, true);
        let mut second = Cell::state(1, program, 8, true);
        let mut third = Cell::state(2, program, 8, true);

        assert_eq!(
            require_distinct(&[first.info(), second.info()]),
            Err(ClutchError::AccountAlias.into())
        );
        assert_eq!(require_distinct(&[first.info(), third.info()]), Ok(()));
        assert_eq!(
            require_count(&[first.info()], 2),
            Err(ClutchError::AccountCount.into())
        );
        assert_eq!(
            require_signer(&first.info()),
            Err(ClutchError::MissingSignature.into())
        );
        first.is_signer = true;
        assert_eq!(require_signer(&first.info()), Ok(()));
    }

    #[test]
    fn expect_pda_refuses_a_wrong_key_and_a_wrong_bump() {
        let derived = (key(7), 254);
        assert_eq!(expect_pda(&key(7), derived, Some(254)), Ok(()));
        assert_eq!(expect_pda(&key(7), derived, None), Ok(()));
        assert_eq!(
            expect_pda(&key(8), derived, Some(254)),
            Err(ClutchError::WrongPda.into())
        );
        assert_eq!(
            expect_pda(&key(7), derived, Some(253)),
            Err(ClutchError::WrongBump.into())
        );
    }

    /// Every reader must refuse a short, long, mistagged, or misversioned
    /// account, mirroring the layout crate's own hostile-header battery.
    fn hostile_header<T: core::fmt::Debug>(bytes: &[u8], read: fn(&[u8]) -> Outcome<T>) {
        assert!(read(bytes).is_ok(), "canonical bytes must decode");

        let mut truncated = bytes.to_vec();
        truncated.pop();
        assert_eq!(
            read(&truncated).unwrap_err(),
            Refusal::Codec(CodecError::Truncated)
        );

        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert_eq!(
            read(&trailing).unwrap_err(),
            Refusal::Codec(CodecError::TrailingBytes)
        );

        let mut mistagged = bytes.to_vec();
        mistagged[0] ^= 0xff;
        assert_eq!(
            read(&mistagged).unwrap_err(),
            Refusal::Codec(CodecError::WrongTag)
        );

        let mut misversioned = bytes.to_vec();
        misversioned[1] = misversioned[1].wrapping_add(1);
        assert_eq!(
            read(&misversioned).unwrap_err(),
            Refusal::Codec(CodecError::WrongVersion)
        );
    }

    fn grid() -> PriceGridAccount {
        let mut ticks = [0; MAX_GRID_TICKS];
        ticks[1] = 2_500;
        ticks[2] = 5_000;
        ticks[3] = 7_500;
        ticks[4] = 10_000;
        let mut value = PriceGridAccount {
            grid: Hash32::ZERO,
            realm: h(2),
            price_scale: 10_000,
            tick_count: 5,
            ticks,
            stored_bump: 3,
            flags: 0,
        };
        value.grid = value.recomputed_grid_id().expect("grid body");
        value
    }

    fn terms() -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut first = [0; MAX_OUTCOMES];
        first[0] = 1;
        let mut second = [0; MAX_OUTCOMES];
        second[1] = 1;
        payouts[0] = PayoutVectorBytes {
            denominator: 1,
            weights: first,
        };
        payouts[1] = PayoutVectorBytes {
            denominator: 1,
            weights: second,
        };
        let mut knots = [0u128; clutch_solana_layout::MAX_KNOTS];
        knots[0] = 1;
        let mut payout_map = [clutch_solana_layout::PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        payout_map[0] = 0;
        payout_map[1] = 1;
        let mut value = TermsAccount {
            terms: Hash32::ZERO,
            realm: h(2),
            profile: h(3),
            feed: h(5),
            price_grid: grid().grid,
            outcome_count: 2,
            payout_count: 2,
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
            source_adapter_id: h(5),
            payout_map,
            knots,
            collateral_cap: 1_000,
            stored_bump: 9,
            flags: 0,
        };
        value.terms = value.recomputed_terms_digest().expect("terms body");
        value
    }

    fn epoch() -> EpochAccount {
        EpochAccount {
            epoch: canonical_epoch_id(h(1), 4),
            market: h(1),
            book: h(21),
            terms: terms().terms,
            price_grid: grid().grid,
            policy: h(22),
            order_set: Hash32::ZERO,
            first_order_id: Hash32::ZERO,
            last_order_id: Hash32::ZERO,
            epoch_index: 4,
            relation_version: clutch_solana_layout::RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: 99,
            owner_count: 3,
            page_count: 0,
            order_count: 0,
            outcome_count: 2,
            basis_degree: 0,
            phase: clutch_solana_layout::EPOCH_PHASE_OPEN,
            stored_bump: 6,
            flags: 0,
        }
    }

    #[cfg(feature = "profile-direct-v3-source-v2-point")]
    fn direct_epoch() -> DirectEpochV3Account {
        DirectEpochV3Account::open(
            h(1),
            terms().terms,
            grid().grid,
            h(22),
            4,
            10_000,
            0,
            100,
            110,
            6,
        )
        .expect("direct epoch")
    }

    /// One empty, unfrozen page.
    ///
    /// The slots are canonical padding rather than records on purpose: this
    /// test is about the account header and the page-set commitment fields,
    /// and an empty page keeps the fixture independent of which order families
    /// `clutch_solana_layout::OrderSlot` currently admits.
    fn order_page() -> OrderPageAccount {
        let mut value = OrderPageAccount {
            market: h(1),
            epoch: canonical_epoch_id(h(1), 4),
            order_set: Hash32::ZERO,
            page_digest: Hash32::ZERO,
            first_order_id: Hash32::ZERO,
            last_order_id: Hash32::ZERO,
            prev_page_last_order_id: Hash32::ZERO,
            page_index: 0,
            page_count: 1,
            set_order_count: 0,
            order_count: 0,
            tombstone_count: 0,
            frozen: 0,
            stored_bump: 5,
            orders: [OrderSlot::Empty; MAX_ORDERS_PER_PAGE],
        };
        value.page_digest = value.recomputed_page_digest().expect("page digest");
        value
    }

    fn candidate() -> CandidateRecord {
        let epoch = epoch();
        let mut prices = [0; MAX_OUTCOMES];
        prices[0] = 10_000;
        let mut value = CandidateRecord {
            candidate: Hash32::ZERO,
            epoch: epoch.epoch,
            market: epoch.market,
            prices,
            virtual_split: 5,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: -3,
            limit_surplus_price_units: 7,
            // v3: a VERIFIED record carries its verified tie digest.
            score_digest: Hash32([0x5c; 32]),
            churn: 5,
            submitted_slot: 42,
            distinct_owners: 3,
            order_len: 1,
            outcome_count: 2,
            status: clutch_solana_layout::CANDIDATE_STATUS_VERIFIED,
            stored_bump: 4,
            flags: 0,
        };
        value.candidate = value
            .recomputed_candidate_digest()
            .expect("candidate digest");
        value
    }

    macro_rules! header_case {
        ($name:ident, $len:expr, $value:expr, $reader:path) => {
            #[test]
            fn $name() {
                let mut bytes = [0_u8; $len];
                $value.encode(&mut bytes).expect("fixture encodes");
                hostile_header(&bytes, $reader);
            }
        };
    }

    header_case!(
        supply_reader_refuses_hostile_headers,
        account_len::SUPPLY_LEDGER,
        SupplyLedgerAccount {
            market: h(1),
            realm: h(2),
            generation: 3,
            outcome_count: 2,
            internal_supply: [0; MAX_OUTCOMES],
            external_supply: [0; MAX_OUTCOMES],
            stored_bump: 8,
            flags: 0,
        },
        read_supply
    );
    header_case!(
        terms_reader_refuses_hostile_headers,
        account_len::TERMS,
        terms(),
        read_terms
    );
    header_case!(
        price_grid_reader_refuses_hostile_headers,
        account_len::PRICE_GRID,
        grid(),
        read_price_grid
    );
    #[cfg(any(feature = "profile-full", feature = "profile-general-source-v2-point"))]
    header_case!(
        epoch_reader_refuses_hostile_headers,
        account_len::EPOCH,
        epoch(),
        read_epoch
    );
    #[cfg(feature = "profile-direct-v3-source-v2-point")]
    header_case!(
        epoch_reader_refuses_hostile_headers,
        DIRECT_EPOCH_BYTES,
        direct_epoch(),
        read_epoch
    );
    header_case!(
        order_page_reader_refuses_hostile_headers,
        account_len::ORDER_PAGE,
        order_page(),
        read_order_page
    );
    header_case!(
        candidate_reader_refuses_hostile_headers,
        account_len::CANDIDATE,
        candidate(),
        read_candidate
    );
    header_case!(
        final_pot_reader_refuses_hostile_headers,
        account_len::FINAL_POT,
        {
            let c = candidate();
            let mut pot_internal = [0; MAX_OUTCOMES];
            pot_internal[0] = 5;
            FinalPotAccount {
                epoch: c.epoch,
                market: c.market,
                candidate: c.candidate,
                pot_internal,
                pot_cash_price_units: 50_000,
                rounding_pot_price_units: 3,
                outcome_count: 2,
                phase: clutch_solana_layout::POT_PHASE_OPEN,
                stored_bump: 1,
                flags: 0,
            }
        },
        read_final_pot
    );
    header_case!(
        receipt_reader_refuses_hostile_headers,
        account_len::SETTLEMENT_RECEIPT,
        {
            let c = candidate();
            SettlementReceiptAccount {
                epoch: c.epoch,
                market: c.market,
                candidate: c.candidate,
                buy_order_id: h(1),
                sell_order_id: h(2),
                consideration_price_units: 3 * 10_000,
                quantity: 3,
                settled_quantity: 0,
                price: 10_000,
                sequence: 1,
                slice_index: 0,
                outcome: 0,
                leg_kind: clutch_solana_layout::RECEIPT_LEG_DIRECT,
                consumed_flags: 0,
                stored_bump: 2,
                flags: 0,
            }
        },
        read_receipt
    );
    header_case!(
        resolution_reader_refuses_hostile_headers,
        account_len::RESOLUTION,
        ResolutionAccount {
            market: h(1),
            terms: terms().terms,
            feed: h(5),
            window: Hash32::ZERO,
            feed_cursor: 0,
            sealed_end_bucket_exclusive: 0,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            stored_bump: 3,
            flags: 0,
        },
        read_resolution
    );
    header_case!(
        feed_reader_refuses_hostile_headers,
        account_len::FEED,
        FeedAccount {
            feed: h(5),
            realm: h(2),
            cursor: 0,
            next_boundary: 0,
            archive_pages: 0,
            summary: h(6),
            stored_bump: 4,
            flags: 0,
        },
        read_feed
    );

    #[test]
    fn the_resolution_reader_reports_the_unresolved_state_as_unresolved() {
        let mut bytes = [0_u8; account_len::RESOLUTION];
        let mut record = ResolutionAccount {
            market: h(1),
            terms: terms().terms,
            feed: h(5),
            window: Hash32::ZERO,
            feed_cursor: 0,
            sealed_end_bucket_exclusive: 0,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            stored_bump: 3,
            flags: 0,
        };
        record.encode(&mut bytes).expect("unresolved encodes");
        let facts = read_resolution(&bytes).expect("unresolved decodes");
        assert!(!facts.resolved);
        assert_eq!(facts.payout_index, PAYOUT_INDEX_UNRESOLVED);

        record.payout_index = 1;
        record.window = h(30);
        record.feed_cursor = 130;
        record.sealed_end_bucket_exclusive = 130;
        record.repair_generation = 2;
        record.resolved_slot = 900;
        record.encode(&mut bytes).expect("resolved encodes");
        let facts = read_resolution(&bytes).expect("resolved decodes");
        assert!(facts.resolved);
        assert_eq!(facts.payout_index, 1);
    }

    fn ledger(internal: [u64; MAX_OUTCOMES], external: [u64; MAX_OUTCOMES]) -> SupplyFacts {
        SupplyFacts {
            market: h(1),
            realm: h(2),
            generation: 3,
            outcome_count: 2,
            internal_supply: internal,
            external_supply: external,
            stored_bump: 8,
        }
    }

    fn amounts(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
        let mut out = [0; MAX_OUTCOMES];
        out[0] = first;
        out[1] = second;
        out
    }

    fn aggregate(total: [u64; MAX_OUTCOMES]) -> KernelFacts {
        KernelFacts {
            market: h(1),
            phase: 0,
            basis_mode: BasisMode::FinitePreset,
            payout_outcomes: 2,
            total_supply: total,
        }
    }

    #[test]
    fn c1_closes_the_two_term_ledger_against_the_kernel_aggregate() {
        let supply = ledger(amounts(40, 10), amounts(60, 0));
        assert_eq!(
            require_two_term_closure(&supply, &aggregate(amounts(100, 10)), 2),
            Ok(())
        );
        assert_eq!(
            require_two_term_closure(&supply, &aggregate(amounts(101, 10)), 2),
            Err(ClutchError::AggregateClosureMismatch.into())
        );
        // A two-term sum that does not fit is arithmetic, not disagreement.
        let overflowing = ledger(amounts(u64::MAX, 0), amounts(1, 0));
        assert_eq!(
            require_two_term_closure(&overflowing, &aggregate(amounts(0, 0)), 2),
            Err(ClutchError::Arithmetic.into())
        );
    }

    #[test]
    fn c2_bounds_one_triple_without_making_a_second_one_unrepresentable() {
        let supply = ledger(amounts(40, 10), amounts(60, 0));
        // A position holding strictly less than the aggregate is fine, which is
        // exactly the state the old equality refused.
        assert_eq!(
            require_representation_bound(&supply, &amounts(25, 4), &amounts(11, 0), 2),
            Ok(())
        );
        assert_eq!(
            require_representation_bound(&supply, &amounts(40, 10), &amounts(60, 0), 2),
            Ok(())
        );
        assert_eq!(
            require_representation_bound(&supply, &amounts(41, 0), &amounts(0, 0), 2),
            Err(ClutchError::AggregateClosureMismatch.into())
        );
        assert_eq!(
            require_representation_bound(&supply, &amounts(0, 0), &amounts(61, 0), 2),
            Err(ClutchError::AggregateClosureMismatch.into())
        );
    }

    #[test]
    fn c3_moves_the_ledger_by_the_delta_and_never_overwrites_it() {
        /* Two positions share the ledger.  One of them splits 5 more sets; the
         * ledger must gain 5, not be replaced by that one position's balance. */
        let mut internal = amounts(40, 10);
        assert_eq!(
            apply_ledger_delta(&mut internal, 2, &amounts(25, 4), &amounts(30, 9)),
            Ok(())
        );
        assert_eq!(internal, amounts(45, 15));

        // Giving up more than the ledger holds is a closure failure.
        let mut short = amounts(3, 0);
        assert_eq!(
            apply_ledger_delta(&mut short, 2, &amounts(4, 0), &amounts(0, 0)),
            Err(ClutchError::AggregateClosureMismatch.into())
        );

        // A genuine overflow is reported as arithmetic.
        let mut wide = amounts(u64::MAX, 0);
        assert_eq!(
            apply_ledger_delta(&mut wide, 2, &amounts(0, 0), &amounts(1, 0)),
            Err(ClutchError::Arithmetic.into())
        );

        // Padding beyond the active outcome count is never touched.
        let mut padded = [7_u64; MAX_OUTCOMES];
        padded[0] = 10;
        padded[1] = 10;
        assert_eq!(
            apply_ledger_delta(&mut padded, 2, &amounts(1, 1), &amounts(2, 2)),
            Ok(())
        );
        assert_eq!(padded[0], 11);
        assert_eq!(padded[1], 11);
        assert_eq!(padded[2], 7);
    }

    #[test]
    fn the_supply_reader_carries_both_ledger_terms() {
        /* The two terms are separate facts, not a sum: a reader that folded
         * them would make the market-wide internal/external split
         * unobservable, which is exactly the drift deferred check 13 of
         * `SBF_BRINGUP.md` names. */
        let mut internal = [0; MAX_OUTCOMES];
        let mut external = [0; MAX_OUTCOMES];
        internal[0] = 40;
        internal[1] = 10;
        external[0] = 60;
        let ledger = SupplyLedgerAccount {
            market: h(1),
            realm: h(2),
            generation: 3,
            outcome_count: 2,
            internal_supply: internal,
            external_supply: external,
            stored_bump: 8,
            flags: 0,
        };
        let mut bytes = [0_u8; account_len::SUPPLY_LEDGER];
        ledger.encode(&mut bytes).expect("ledger encodes");
        let facts = read_supply(&bytes).expect("ledger decodes");
        assert_eq!(facts.internal_supply, internal);
        assert_eq!(facts.external_supply, external);
        assert_eq!(facts.outcome_count, 2);
        assert_eq!(facts.generation, 3);
    }
}
