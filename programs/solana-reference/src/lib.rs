#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Offline reference adapter joining hostile-byte layouts to the pure kernel.
//!
//! This is deliberately not a Solana program. It has no entrypoint, account
//! runtime, PDA derivation, CPI, token implementation, clock, signatures, or
//! transaction atomicity. Callers provide explicit account metadata and fixed
//! account bytes. The adapter authenticates the facts it can represent, refuses
//! the facts it cannot, runs [`clutch_kernel`] on local copies, and returns exact
//! post-state bytes.
//!
//! The extra kernel, external-balance, and replay accounts are reference-only
//! state needed to expose the missing semantic seams. Their layouts are not a
//! deployment ABI.
//!
//! # Multi-position closure is inductive, not scanned
//!
//! Aggregate closure follows CLO-DELTA-V1
//! (`docs/implementation/MULTI_POSITION_CLOSURE.md`): the supply ledger is the
//! only counted truth, every transition moves it by exactly the delta it
//! applies to the one presented position triple, a position enters the system
//! only provably zero ([`validate_position_init`]), and the per-transition
//! checks are the ledger's two-term closure against the kernel aggregate plus
//! a one-sided bound of the presented triple by the ledger terms. The full
//! `sum over positions == ledger` invariant is a theorem about histories;
//! that every position's history goes through these transitions is the
//! runtime's obligation (PDA uniqueness, ownership, write locks), named in
//! `SOLANA_REFERENCE_ADAPTER.md` obligations 1 through 3 and 9.
//!
//! # Resolution is evidence-gated, not authenticated
//!
//! `Action::Resolve` and `Action::RedeemInternal` no longer refuse
//! unconditionally: they refuse unless the caller supplies a
//! [`ResolutionEvidence`] whose every element checks out. **Evidence-gated is
//! not authenticated.** The feed identity bytes a window is bound to are
//! opaque, nothing here proves an observation came from any source, and the
//! adapter still has no runtime, PDA derivation, clock, or signature checking.
//! What the gate establishes is narrower and exact: the payout index is the
//! one a sealed, complete, mature, correctly-domained fold of the supplied
//! observations selects under terms the market's own digest commits to. Absent
//! or unusable evidence still returns [`Error::ResolutionEvidenceUnavailable`],
//! byte-identically to the previous unconditional refusal.

use clutch_accumulator::{
    CoveragePolicy, FeedIdentity, Grid, Observation, WindowAccumulator, WindowDomain, WindowResult,
    IDENTITY_BYTES, WINDOW_DOMAIN_BYTES,
};
use clutch_kernel::{
    BasisMode, Error as KernelError, MarketState, PayoutSet, PayoutVector, Phase, Position,
    MAX_OUTCOMES as KERNEL_MAX_OUTCOMES, MAX_PAYOUTS,
};
use clutch_solana_layout::{
    account_len, canonical_market_id, collateral, direct_selection_v3::DirectV3Intent, CodecError,
    Hash32, HoardAccount, Intent, MarketAccount, PositionAccount, ProfileAccount, RealmAccount,
    ResolutionAccount, SupplyLedgerAccount, TermsAccount, MAX_OUTCOMES, PROFILE_FLAG_POLICY_FROZEN,
};

mod resolution;

pub use clutch_accumulator::WindowError;
pub use resolution::{
    derive_payout, derive_payout_vector, ResolutionRefusal, ResolutionTerms,
    AMBIG_COMPATIBLE_SET_02, AMBIG_REFUSE_01, EDGE_CLAMP_01, EDGE_REFUSE_02,
    FAIL_EXTENDED_WINDOW_02, FAIL_UNIFORM_REFUND_01, GEN_EXACT_01, GEN_FINAL_AT_MATURITY_02,
    MAX_BOUNDARIES, MAX_CELLS, PAYOUT_MAP_UNUSED, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
    STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07, STAT_RELATIVE_TERMINAL_TWAP_05,
    STAT_SAMPLED_MAX_03, STAT_SAMPLED_MIN_02, STAT_TERMINAL_01, STAT_TWAP_04, V1_EVALUATOR_VERSION,
    V1_EXACT_GENERATION, V1_SOURCE_VERSION,
};

const KERNEL_TAG: u8 = 0x41;
const EXTERNAL_TAG: u8 = 0x42;
const REPLAY_TAG: u8 = 0x43;
const WINDOW_EVIDENCE_TAG: u8 = 0x45;
const REQUEST_TAG: u8 = 0xd1;
const REFERENCE_VERSION: u8 = 1;
const KERNEL_ACCOUNT_VERSION: u8 = 2;
const ACTION_LAYOUT: u8 = 0;
const ACTION_RESOLVE: u8 = 1;
const ACTION_REDEEM_INTERNAL: u8 = 2;
const PAYOUT_VECTOR_BYTES: usize = 8 + (8 * MAX_OUTCOMES);
const OBSERVATION_ACCEPTED: u8 = 1;
const OBSERVATION_MISSING: u8 = 0;

/// Fixed bytes of one observation record in the window-evidence blob.
pub const OBSERVATION_RECORD_BYTES: usize = 1 + 8 + 16 + 16;
/// Fixed bytes of the window-evidence header, including the record count.
pub const WINDOW_EVIDENCE_HEADER_BYTES: usize =
    2 + (2 * IDENTITY_BYTES) + 4 + 4 + 4 + 2 + 8 + 8 + 8 + 8 + 8 + 2 + 8 + 2;
/// Largest observation count this reference adapter folds in one call.
///
/// This is a stack bound of the offline lab, not a protocol bound: the
/// accumulator admits up to `MAX_BUCKETS` and a real adapter would fold pages
/// across transactions. Obligation 10 still owns the real resource question.
pub const MAX_OBSERVATIONS: usize = 32;
/// Largest window-evidence blob accepted by this reference adapter.
pub const MAX_WINDOW_EVIDENCE_LEN: usize =
    WINDOW_EVIDENCE_HEADER_BYTES + (MAX_OBSERVATIONS * OBSERVATION_RECORD_BYTES);

/// Exact length of the reference-only kernel account.
pub const KERNEL_ACCOUNT_LEN: usize =
    2 + 32 + 1 + 1 + 1 + 1 + 1 + (8 * MAX_OUTCOMES) + (MAX_PAYOUTS * PAYOUT_VECTOR_BYTES);
/// Exact length of the reference-only external-balance account.
pub const EXTERNAL_ACCOUNT_LEN: usize = 2 + 32 + 32 + 8 + (8 * MAX_OUTCOMES) + 1 + 1;
/// Exact length of the reference-only replay account.
pub const REPLAY_ACCOUNT_LEN: usize = 2 + 32 + 32 + 8 + 8 + 1 + 1;
/// Largest request accepted by this reference adapter.
pub const MAX_REQUEST_LEN: usize = 2 + 8 + 1 + 2 + clutch_solana_layout::MAX_INTENT_BYTES;

/// Errors from metadata checks, codecs, or pure transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A frozen layout codec rejected hostile bytes.
    Layout(CodecError),
    /// The pure semantic kernel rejected a state or transition.
    Kernel(KernelError),
    /// A reference-only account or request had the wrong exact length.
    WrongLength,
    /// A reference-only discriminator was wrong.
    WrongTag,
    /// A reference-only version was unsupported.
    WrongVersion,
    /// A reference-only enum, flag, or padding field was invalid.
    NonCanonical,
    /// A checked arithmetic operation overflowed or underflowed.
    Arithmetic,
    /// A supplied account was not owned by the expected program identity.
    WrongProgramOwner,
    /// Two logical account roles shared one key.
    AccountAlias,
    /// An account key did not match the trusted binding supplied by the caller.
    WrongAccountKey,
    /// A state account required for a transition was not writable.
    NotWritable,
    /// The actor did not present a signature assertion.
    MissingSignature,
    /// The signed actor was not authorized for the requested action.
    UnauthorizedActor,
    /// No authority policy exists for this action, so it fails closed.
    AuthorizationUnavailable,
    /// No typed maturity, sealed-window, source, terms, and payout evidence
    /// was supplied at all, so the transition fails closed.
    ///
    /// This is the pre-evidence-plane refusal, retained byte-identically: a
    /// caller that supplies no [`ResolutionEvidence`] gets exactly this class
    /// for both `Action::Resolve` and `Action::RedeemInternal`, whatever the
    /// account bytes claim.
    ResolutionEvidenceUnavailable,
    /// The typed window evidence was refused by the accumulator state machine.
    ///
    /// Carries the accumulator's own named reason, so a truncated prefix, an
    /// immature window, a non-contiguous page, a backwards feed cursor, a
    /// refused coverage policy, and a wrong domain field stay distinguishable.
    Window(WindowError),
    /// The terms-to-payout derivation refused. Carries its `R-nn` class.
    Resolution(ResolutionRefusal),
    /// The supplied terms artifact is not the one the market's digest binds.
    TermsBindingMismatch,
    /// The reference kernel payout set is not the immutable terms payout set.
    PayoutSetMismatch,
    /// The resolution record is not bound to this market's immutable terms.
    ResolutionBindingMismatch,
    /// A payout was already recorded for this market, so it cannot resolve again.
    ResolutionAlreadyRecorded,
    /// Redemption requires a resolution record that already selected a payout.
    ResolutionNotRecorded,
    /// The requested payout index is not the one the evidence derives.
    PayoutIndexMismatch,
    /// An immutable evidence account was presented as writable.
    ImmutableAccountWritable,
    /// Resolution evidence was supplied for an action that admits none.
    UnexpectedEvidence,
    /// The trusted window identity binding was absent.
    WindowIdentityUnavailable,
    /// The Realm profile's collateral policy digest is not frozen.
    CollateralPolicyNotFrozen,
    /// A stored bump differed from the separately supplied expected bump.
    WrongBump,
    /// Account identities, generations, phases, or immutable fields disagreed.
    MismatchedState,
    /// A presented position exceeded, or the supply ledger disagreed with, the
    /// market aggregate it must be represented in.
    ///
    /// Raised by the CLO-DELTA-V1 closure checks of
    /// `docs/implementation/MULTI_POSITION_CLOSURE.md`: the ledger's two terms
    /// must sum to the kernel aggregate per outcome, and the presented
    /// position's internal and external balances must each be bounded by the
    /// matching ledger term. The full multi-position sum invariant is
    /// inductive — zero at initialization, preserved by the checked delta
    /// write-back — so this class is what a counterfeit claim, a tampered
    /// ledger, or a diverging aggregate effect refuses with.
    AggregateClosureMismatch,
    /// Market initialization contained pre-existing claims or a closing position.
    NonEmptyInitialization,
    /// A request sequence was stale, skipped, or exhausted.
    Replay,
    /// The operation is outside this deliberately small reference subset.
    UnsupportedIntent,
    /// The market collateral cap would be exceeded.
    CollateralCap,
}

impl From<CodecError> for Error {
    fn from(value: CodecError) -> Self {
        Self::Layout(value)
    }
}

impl From<KernelError> for Error {
    fn from(value: KernelError) -> Self {
        Self::Kernel(value)
    }
}

impl From<WindowError> for Error {
    fn from(value: WindowError) -> Self {
        Self::Window(value)
    }
}

impl From<ResolutionRefusal> for Error {
    fn from(value: ResolutionRefusal) -> Self {
        Self::Resolution(value)
    }
}

/// Result returned by this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Runtime metadata asserted for one account by a future adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMetadata {
    /// Account key.
    pub key: Hash32,
    /// Runtime-reported owner program.
    pub owner_program: Hash32,
    /// Whether the instruction declared the account writable.
    pub writable: bool,
}

/// Runtime metadata asserted for a transaction signer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorMetadata {
    /// Signer account key.
    pub key: Hash32,
    /// Whether the runtime authenticated a signature for this actor.
    pub signer: bool,
}

/// Metadata for every state role in a reference transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionMetadata {
    /// Frozen market account metadata.
    pub market: AccountMetadata,
    /// Collateral hoard account metadata.
    pub hoard: AccountMetadata,
    /// Owner position account metadata.
    pub position: AccountMetadata,
    /// Reference kernel-state account metadata.
    pub kernel: AccountMetadata,
    /// Reference external-balance account metadata.
    pub external: AccountMetadata,
    /// Reference replay account metadata.
    pub replay: AccountMetadata,
    /// Market-wide supply ledger account metadata.
    pub supply: AccountMetadata,
    /// Authenticated action actor.
    pub actor: ActorMetadata,
}

/// Trusted account bindings a real SVM adapter must derive rather than accept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedBindings {
    /// Program identity expected to own all state accounts.
    pub program_id: Hash32,
    /// Expected market account key.
    pub market: Hash32,
    /// Expected hoard account key.
    pub hoard: Hash32,
    /// Expected position account key.
    pub position: Hash32,
    /// Expected reference kernel account key.
    pub kernel: Hash32,
    /// Expected reference external account key.
    pub external: Hash32,
    /// Expected reference replay account key.
    pub replay: Hash32,
    /// Expected supply ledger account key.
    pub supply: Hash32,
    /// Expected market PDA bump.
    pub market_bump: u8,
    /// Expected hoard PDA bump.
    pub hoard_bump: u8,
    /// Expected position PDA bump.
    pub position_bump: u8,
    /// Expected reference external-account bump.
    pub external_bump: u8,
    /// Expected reference replay-account bump.
    pub replay_bump: u8,
    /// Expected supply ledger bump.
    pub supply_bump: u8,
}

/// Runtime metadata for the market-global resolution transition.
///
/// The absence of Position, external-balance, and owner Replay roles is the
/// replay-domain boundary: resolution is a fact about one Market, never one
/// wallet's current position generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionTransitionMetadata {
    /// Market account metadata.
    pub market: AccountMetadata,
    /// Collateral Hoard metadata.
    pub hoard: AccountMetadata,
    /// Kernel aggregate metadata.
    pub kernel: AccountMetadata,
    /// Market-wide SupplyLedger metadata.
    pub supply: AccountMetadata,
    /// Authenticated fee payer; no key is privileged.
    pub actor: ActorMetadata,
}

/// Trusted bindings for the market-global resolution transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionExpectedBindings {
    /// Program expected to own all state accounts.
    pub program_id: Hash32,
    /// Expected Market account key.
    pub market: Hash32,
    /// Expected Hoard account key.
    pub hoard: Hash32,
    /// Expected kernel aggregate key.
    pub kernel: Hash32,
    /// Expected SupplyLedger key.
    pub supply: Hash32,
    /// Expected Market PDA bump.
    pub market_bump: u8,
    /// Expected Hoard PDA bump.
    pub hoard_bump: u8,
    /// Expected SupplyLedger PDA bump.
    pub supply_bump: u8,
}

/// Immutable byte slices consumed by one reference transition.
#[derive(Clone, Copy, Debug)]
pub struct StateBytes<'a> {
    /// Market layout bytes.
    pub market: &'a [u8],
    /// Hoard layout bytes.
    pub hoard: &'a [u8],
    /// Position layout bytes.
    pub position: &'a [u8],
    /// Reference kernel-state bytes.
    pub kernel: &'a [u8],
    /// Reference external-balance bytes.
    pub external: &'a [u8],
    /// Reference replay bytes.
    pub replay: &'a [u8],
    /// Market-wide supply ledger bytes.
    pub supply: &'a [u8],
}

/// Immutable market-global state consumed by resolution.
#[derive(Clone, Copy, Debug)]
pub struct ResolutionStateBytes<'a> {
    /// Market layout bytes.
    pub market: &'a [u8],
    /// Hoard layout bytes.
    pub hoard: &'a [u8],
    /// Kernel aggregate bytes.
    pub kernel: &'a [u8],
    /// Market-wide SupplyLedger bytes.
    pub supply: &'a [u8],
}

/// Immutable byte slices of the typed resolution evidence plane.
///
/// These are separate from [`StateBytes`] on purpose. A transition that needs
/// no resolution evidence must be callable with no way to supply any, so that
/// the fail-closed default stays the *absence* of a code path rather than a
/// flag somebody can set.
#[derive(Clone, Copy, Debug)]
pub struct EvidenceBytes<'a> {
    /// Immutable [`TermsAccount`] bytes whose digest the market committed to.
    pub terms: &'a [u8],
    /// [`ResolutionAccount`] bytes: unresolved before a resolve, resolved
    /// before a redemption.
    pub resolution: &'a [u8],
    /// Window evidence blob: one declared domain plus its observation records.
    pub window: &'a [u8],
}

/// Runtime metadata for the two evidence-plane accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceMetadata {
    /// Immutable terms account metadata.
    pub terms: AccountMetadata,
    /// Resolution record account metadata.
    pub resolution: AccountMetadata,
}

/// Trusted bindings for the evidence plane that a real adapter must derive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceBindings {
    /// Expected immutable terms account key.
    pub terms: Hash32,
    /// Expected resolution record account key.
    pub resolution: Hash32,
    /// Expected terms account bump.
    pub terms_bump: u8,
    /// Expected resolution account bump.
    pub resolution_bump: u8,
    /// Trusted `WindowId` of the expected window domain.
    ///
    /// This crate owns no hash primitive, exactly as
    /// `clutch_accumulator` owns none: the accumulator publishes
    /// `WINDOW_DOMAIN_TAG` and the exact 144 canonical preimage bytes so a
    /// hashing adapter and an independent recomputation cannot disagree. A
    /// real adapter must derive this value as
    /// `HASH(WINDOW_DOMAIN_TAG || WindowDomain::encode_canonical())`; here it
    /// arrives as a trusted binding beside the PDA keys and bumps, is refused
    /// when zero, and is recorded rather than believed — no gate decision
    /// depends on it. Use [`expected_window_preimage`] to recompute the exact
    /// bytes it must be the digest of.
    pub window_id: Hash32,
}

/// One caller-supplied typed resolution evidence bundle.
///
/// Supplying this does not make resolution succeed. Every element is checked
/// against terms the market's own digest commits to, and the sealed
/// [`WindowResult`] is constructed here by driving the accumulator's state
/// machine over the observation records: there is no "sealed" flag on the wire
/// for a caller to set.
#[derive(Clone, Copy, Debug)]
pub struct ResolutionEvidence<'a> {
    /// Evidence account bytes.
    pub bytes: EvidenceBytes<'a>,
    /// Evidence account runtime metadata.
    pub metadata: EvidenceMetadata,
    /// Trusted evidence bindings.
    pub bindings: EvidenceBindings,
    /// Authenticated next-bucket cursor witnessed for the underlying feed.
    pub feed_cursor: u64,
    /// Slot recorded into the resolution account, supplied by the adapter.
    pub resolved_slot: u64,
}

/// Exact post-state bytes returned only after every check succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionOutput {
    /// Market account post-state.
    pub market: [u8; account_len::MARKET],
    /// Hoard account post-state.
    pub hoard: [u8; account_len::HOARD],
    /// Position account post-state.
    pub position: [u8; account_len::POSITION],
    /// Reference kernel-state post-state.
    pub kernel: [u8; KERNEL_ACCOUNT_LEN],
    /// Reference external-balance post-state.
    pub external: [u8; EXTERNAL_ACCOUNT_LEN],
    /// Reference replay post-state.
    pub replay: [u8; REPLAY_ACCOUNT_LEN],
    /// Supply ledger post-state.
    pub supply: [u8; account_len::SUPPLY_LEDGER],
    /// Resolution record post-state; `None` when no evidence plane was used.
    ///
    /// A resolve writes the selected payout and the sealed window facts here.
    /// A redemption returns the record unchanged, so a caller can compare the
    /// exact bytes and see that redemption never edits its own authority.
    pub resolution: Option<[u8; account_len::RESOLUTION]>,
    /// Collateral atoms paid by a redemption; zero for every other action.
    pub redemption_payout: u64,
}

/// Exact market-global post-state of one resolution attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionTransitionOutput {
    /// Market account post-state.
    pub market: [u8; account_len::MARKET],
    /// Hoard account post-state; byte-identical because Resolve moves no value.
    pub hoard: [u8; account_len::HOARD],
    /// Kernel aggregate post-state.
    pub kernel: [u8; KERNEL_ACCOUNT_LEN],
    /// SupplyLedger post-state; byte-identical in this reference relation.
    pub supply: [u8; account_len::SUPPLY_LEDGER],
    /// Canonical Resolution record post-state.
    pub resolution: [u8; account_len::RESOLUTION],
    /// True only when the input was the already-recorded exact resolution fact.
    pub repeated: bool,
}

/// Kernel-only facts not present in the frozen Solana layout prototype.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelAccount {
    /// Associated market identity.
    pub market: Hash32,
    /// Kernel phase: zero active, one resolved.
    pub phase: u8,
    /// Immutable resolution mode selected from the validated market terms.
    pub basis_mode: BasisMode,
    /// Selected payout index after resolution.
    pub resolved_payout: u8,
    /// Immutable finite payout set.
    pub payouts: PayoutSet,
    /// Aggregate internal plus external supply by outcome.
    pub total_supply: [u64; MAX_OUTCOMES],
}

/// Reference shadow for claims materialized outside the internal ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAccount {
    /// Associated market identity.
    pub market: Hash32,
    /// Associated owner identity.
    pub owner: Hash32,
    /// Position generation this shadow belongs to.
    pub position_generation: u64,
    /// External claim balances by outcome.
    pub balances: [u64; MAX_OUTCOMES],
    /// Stored bump checked against caller-supplied trusted derivation.
    pub stored_bump: u8,
    /// Reserved flags; must be zero.
    pub flags: u8,
}

/// Reference replay sequence, namespaced by position generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayAccount {
    /// Associated market identity.
    pub market: Hash32,
    /// Associated owner identity.
    pub owner: Hash32,
    /// Position generation this sequence belongs to.
    pub position_generation: u64,
    /// Exact next request sequence.
    pub sequence: u64,
    /// Stored bump checked against caller-supplied trusted derivation.
    pub stored_bump: u8,
    /// Reserved flags; must be zero.
    pub flags: u8,
}

/// A decoded reference request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    /// Exact replay sequence.
    pub sequence: u64,
    /// Requested semantic action.
    pub action: Action,
}

/// Dedicated request envelope for the versioned Direct V3 lifecycle.
///
/// This decoder deliberately does not widen [`Intent`] or [`Request`]. Until
/// the SBF adapter routes this exact type to the complete V3 handler family,
/// tags 36 through 46 continue to fail closed through the legacy request
/// decoder. Keeping the envelope separate also prevents a partially added V3
/// tag from falling into a legacy direct handler with different account
/// versions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectV3Request {
    /// Exact replay sequence supplied to the routed lifecycle action.
    pub sequence: u64,
    /// One exact V3 lifecycle intent.
    pub intent: DirectV3Intent,
}

/// Actions supported by the offline reference adapter.
///
/// The size spread between `Layout` and the two narrow variants is deliberate
/// and cannot be closed here.  `Layout` carries the frozen
/// [`clutch_solana_layout::Intent`], whose widest arm is a portfolio placement
/// with a `[u64; MAX_OUTCOMES]` coefficient vector, and the only refactor
/// `clippy::large_enum_variant` proposes is boxing — indirection this crate is
/// forbidden to have.  It is `no_std`, `no_alloc`, and fixed-layout, and the
/// lint itself notes that boxing would also cost the `Copy` every caller here
/// relies on.  So the spread is stated rather than removed.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    /// A frozen layout intent; only a strict subset can transition state.
    Layout(Intent),
    /// Wire request for resolution; execution currently refuses fail-closed.
    Resolve {
        /// Payout-vector index.
        payout_index: u8,
    },
    /// Wire request for redemption; execution currently refuses fail-closed.
    RedeemInternal {
        /// Outcome index.
        outcome: u8,
        /// Claim atoms to redeem exactly.
        quantity: u64,
    },
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], expected: usize, tag: u8) -> Result<Self> {
        Self::new_version(bytes, expected, tag, REFERENCE_VERSION)
    }
    fn new_version(bytes: &'a [u8], expected: usize, tag: u8, version: u8) -> Result<Self> {
        if bytes.len() != expected {
            return Err(Error::WrongLength);
        }
        if bytes[0] != tag {
            return Err(Error::WrongTag);
        }
        if bytes[1] != version {
            return Err(Error::WrongVersion);
        }
        Ok(Self { bytes, at: 2 })
    }
    fn raw<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(Error::WrongLength)?;
        if end > self.bytes.len() {
            return Err(Error::WrongLength);
        }
        let mut out = [0; N];
        out.copy_from_slice(&self.bytes[self.at..end]);
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.raw::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.raw::<2>()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.raw::<4>()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.raw::<8>()?))
    }
    fn u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.raw::<16>()?))
    }
    fn hash(&mut self) -> Result<Hash32> {
        Ok(Hash32::from_bytes(self.raw::<32>()?))
    }
    fn done(self) -> Result<()> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}

struct Writer<'a> {
    bytes: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    fn new(bytes: &'a mut [u8], tag: u8) -> Result<Self> {
        Self::new_version(bytes, tag, REFERENCE_VERSION)
    }
    fn new_version(bytes: &'a mut [u8], tag: u8, version: u8) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(Error::WrongLength);
        }
        bytes[0] = tag;
        bytes[1] = version;
        Ok(Self { bytes, at: 2 })
    }
    fn raw(&mut self, value: &[u8]) -> Result<()> {
        let end = self.at.checked_add(value.len()).ok_or(Error::WrongLength)?;
        if end > self.bytes.len() {
            return Err(Error::WrongLength);
        }
        self.bytes[self.at..end].copy_from_slice(value);
        self.at = end;
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<()> {
        self.raw(&[value])
    }
    fn u64(&mut self, value: u64) -> Result<()> {
        self.raw(&value.to_le_bytes())
    }
    fn hash(&mut self, value: Hash32) -> Result<()> {
        self.raw(&value.bytes())
    }
    fn done(self) -> Result<()> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::WrongLength)
        }
    }
}

impl KernelAccount {
    /// Encode the exact reference-only kernel account layout.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        if out.len() != KERNEL_ACCOUNT_LEN {
            return Err(Error::WrongLength);
        }
        let mut writer = Writer::new_version(out, KERNEL_TAG, KERNEL_ACCOUNT_VERSION)?;
        writer.hash(self.market)?;
        writer.u8(self.phase)?;
        writer.u8(self.basis_mode as u8)?;
        writer.u8(self.resolved_payout)?;
        writer.u8(self.payouts.count)?;
        writer.u8(self.payouts.outcomes)?;
        for amount in self.total_supply {
            writer.u64(amount)?;
        }
        for vector in self.payouts.vectors {
            writer.u64(vector.denominator)?;
            for weight in vector.weights {
                writer.u64(weight)?;
            }
        }
        writer.done()?;
        Ok(KERNEL_ACCOUNT_LEN)
    }

    /// Decode the exact reference-only kernel account layout.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new_version(
            bytes,
            KERNEL_ACCOUNT_LEN,
            KERNEL_TAG,
            KERNEL_ACCOUNT_VERSION,
        )?;
        let market = reader.hash()?;
        let phase = reader.u8()?;
        let basis_mode = match reader.u8()? {
            0 => BasisMode::FinitePreset,
            1 => BasisMode::DerivedBasis,
            _ => return Err(Error::NonCanonical),
        };
        let resolved_payout = reader.u8()?;
        let count = reader.u8()?;
        let outcomes = reader.u8()?;
        let mut total_supply = [0; MAX_OUTCOMES];
        for amount in &mut total_supply {
            *amount = reader.u64()?;
        }
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        for vector in &mut vectors {
            let denominator = reader.u64()?;
            let mut weights = [0; MAX_OUTCOMES];
            for weight in &mut weights {
                *weight = reader.u64()?;
            }
            *vector = PayoutVector::new(denominator, weights);
        }
        reader.done()?;
        let value = Self {
            market,
            phase,
            basis_mode,
            resolved_payout,
            payouts: PayoutSet::new(count, outcomes, vectors),
            total_supply,
        };
        value.validate_shape()?;
        Ok(value)
    }

    fn validate_shape(&self) -> Result<()> {
        if self.market == Hash32::ZERO || self.phase > 1 {
            return Err(Error::NonCanonical);
        }
        self.payouts.validate()?;
        if self.phase == 0 && self.resolved_payout != 0 {
            return Err(Error::NonCanonical);
        }
        if self.phase == 1 {
            match self.basis_mode {
                BasisMode::FinitePreset if self.resolved_payout >= self.payouts.count => {
                    return Err(Error::NonCanonical);
                }
                BasisMode::DerivedBasis if self.resolved_payout != 0 => {
                    return Err(Error::NonCanonical);
                }
                BasisMode::FinitePreset | BasisMode::DerivedBasis => {}
            }
        }
        let count = usize::from(self.payouts.outcomes);
        if self.total_supply[count..].iter().any(|amount| *amount != 0) {
            return Err(Error::NonCanonical);
        }
        Ok(())
    }
}

impl ExternalAccount {
    /// Encode the exact reference-only external-balance layout.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        if out.len() != EXTERNAL_ACCOUNT_LEN || self.flags != 0 {
            return Err(Error::NonCanonical);
        }
        let mut writer = Writer::new(out, EXTERNAL_TAG)?;
        writer.hash(self.market)?;
        writer.hash(self.owner)?;
        writer.u64(self.position_generation)?;
        for balance in self.balances {
            writer.u64(balance)?;
        }
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.done()?;
        Ok(EXTERNAL_ACCOUNT_LEN)
    }

    /// Decode the exact reference-only external-balance layout.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes, EXTERNAL_ACCOUNT_LEN, EXTERNAL_TAG)?;
        let market = reader.hash()?;
        let owner = reader.hash()?;
        let position_generation = reader.u64()?;
        let mut balances = [0; MAX_OUTCOMES];
        for balance in &mut balances {
            *balance = reader.u64()?;
        }
        let value = Self {
            market,
            owner,
            position_generation,
            balances,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.done()?;
        if value.market == Hash32::ZERO || value.owner == Hash32::ZERO || value.flags != 0 {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }
}

impl ReplayAccount {
    /// Encode the exact reference-only replay layout.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        if out.len() != REPLAY_ACCOUNT_LEN || self.flags != 0 {
            return Err(Error::NonCanonical);
        }
        let mut writer = Writer::new(out, REPLAY_TAG)?;
        writer.hash(self.market)?;
        writer.hash(self.owner)?;
        writer.u64(self.position_generation)?;
        writer.u64(self.sequence)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.done()?;
        Ok(REPLAY_ACCOUNT_LEN)
    }

    /// Decode the exact reference-only replay layout.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(bytes, REPLAY_ACCOUNT_LEN, REPLAY_TAG)?;
        let value = Self {
            market: reader.hash()?,
            owner: reader.hash()?,
            position_generation: reader.u64()?,
            sequence: reader.u64()?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.done()?;
        if value.market == Hash32::ZERO || value.owner == Hash32::ZERO || value.flags != 0 {
            return Err(Error::NonCanonical);
        }
        Ok(value)
    }
}

impl Request {
    /// Decode a strict replay envelope and, where applicable, a frozen layout intent.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 11 || bytes.len() > MAX_REQUEST_LEN {
            return Err(Error::WrongLength);
        }
        if bytes[0] != REQUEST_TAG {
            return Err(Error::WrongTag);
        }
        if bytes[1] != REFERENCE_VERSION {
            return Err(Error::WrongVersion);
        }
        let sequence = u64::from_le_bytes(bytes[2..10].try_into().map_err(|_| Error::WrongLength)?);
        let action = match bytes[10] {
            ACTION_LAYOUT => {
                if bytes.len() < 13 {
                    return Err(Error::WrongLength);
                }
                let len = usize::from(u16::from_le_bytes(
                    bytes[11..13].try_into().map_err(|_| Error::WrongLength)?,
                ));
                if len > clutch_solana_layout::MAX_INTENT_BYTES || bytes.len() != 13 + len {
                    return Err(Error::WrongLength);
                }
                Action::Layout(Intent::decode(&bytes[13..])?)
            }
            ACTION_RESOLVE => {
                if bytes.len() != 12 {
                    return Err(Error::WrongLength);
                }
                Action::Resolve {
                    payout_index: bytes[11],
                }
            }
            ACTION_REDEEM_INTERNAL => {
                if bytes.len() != 20 {
                    return Err(Error::WrongLength);
                }
                Action::RedeemInternal {
                    outcome: bytes[11],
                    quantity: u64::from_le_bytes(
                        bytes[12..20].try_into().map_err(|_| Error::WrongLength)?,
                    ),
                }
            }
            _ => return Err(Error::NonCanonical),
        };
        Ok(Self { sequence, action })
    }
}

impl DirectV3Request {
    /// Encode the strict reference envelope and exact Direct V3 inner wire.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        let inner_len = self.intent.encoded_len();
        let exact = 13usize.checked_add(inner_len).ok_or(Error::Arithmetic)?;
        if out.len() < exact {
            return Err(Error::WrongLength);
        }
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&self.sequence.to_le_bytes());
        out[10] = ACTION_LAYOUT;
        out[11..13].copy_from_slice(
            &u16::try_from(inner_len)
                .map_err(|_| Error::WrongLength)?
                .to_le_bytes(),
        );
        let written = self.intent.encode(&mut out[13..exact])?;
        if written != inner_len {
            return Err(Error::WrongLength);
        }
        Ok(exact)
    }

    /// Decode only the strict Direct V3 request family.
    ///
    /// Legacy layout tags, resolution actions, hostile lengths, and trailing
    /// bytes all refuse before a lifecycle handler can inspect accounts.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 13 || bytes.len() > MAX_REQUEST_LEN {
            return Err(Error::WrongLength);
        }
        if bytes[0] != REQUEST_TAG {
            return Err(Error::WrongTag);
        }
        if bytes[1] != REFERENCE_VERSION {
            return Err(Error::WrongVersion);
        }
        if bytes[10] != ACTION_LAYOUT {
            return Err(Error::WrongTag);
        }
        let inner_len = usize::from(u16::from_le_bytes(
            bytes[11..13].try_into().map_err(|_| Error::WrongLength)?,
        ));
        if inner_len > clutch_solana_layout::MAX_INTENT_BYTES
            || bytes.len() != 13usize.checked_add(inner_len).ok_or(Error::Arithmetic)?
        {
            return Err(Error::WrongLength);
        }
        Ok(Self {
            sequence: u64::from_le_bytes(bytes[2..10].try_into().map_err(|_| Error::WrongLength)?),
            intent: DirectV3Intent::decode(&bytes[13..])?,
        })
    }
}

/// Validate an already encoded initial market against a create intent, its
/// immutable terms artifact, and the Realm's frozen collateral policy.
///
/// This proves only local byte/identity coherence. It deliberately does not
/// authorize creation; [`apply`] refuses `CreateMarket` until an SVM authority
/// model exists.
///
/// `policy_bytes` are the Realm's 266 collateral-policy bytes, an **evidence
/// input** per `RESOLUTION_EVIDENCE_PLAN.md` §3.5: the Profile's freeze
/// discipline alone cannot tell whether the frozen digest is the *right*
/// one, so the child digest is recomputed from these bytes and compared
/// ([`collateral::verify_collateral_binding`]). `terms_bytes` are the
/// immutable terms artifact the new market binds: the founding
/// `collateral_cap` is the terms' own cap — nonzero by the terms codec, so a
/// market with no cap decision cannot be founded — and it must not exceed
/// the ceiling the bound collateral policy admits
/// ([`collateral::CollateralPolicy::check_market_cap`]).
#[allow(clippy::too_many_arguments)] // one argument per evidence artifact, deliberately unbundled
pub fn validate_market_init(
    realm_bytes: &[u8],
    profile_bytes: &[u8],
    policy_bytes: &[u8],
    terms_bytes: &[u8],
    state: StateBytes<'_>,
    create_intent_bytes: &[u8],
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
) -> Result<()> {
    validate_metadata(metadata, bindings, false)?;
    let realm = RealmAccount::decode(realm_bytes)?;
    let profile = ProfileAccount::decode(profile_bytes)?;
    let mut terms_account = TermsAccount::ZEROED;
    TermsAccount::decode_into(terms_bytes, &mut terms_account)?;
    let decoded = DecodedState::decode(state)?;
    let DecodedState {
        market,
        hoard,
        position,
        kernel,
        external,
        replay,
        supply,
    } = &decoded;
    validate_links(&decoded, bindings)?;
    let policy = require_collateral_binding(policy_bytes, &profile)?;
    let intent = Intent::decode(create_intent_bytes)?;
    let (intent_realm, intent_profile, nonce, outcomes, terms, feed) = match intent {
        Intent::CreateMarket {
            realm,
            profile,
            market_nonce,
            outcome_count,
            terms,
            feed,
        } => (realm, profile, market_nonce, outcome_count, terms, feed),
        _ => return Err(Error::UnsupportedIntent),
    };
    let expected_market = canonical_market_id(intent_realm, intent_profile, nonce);
    if realm.realm != intent_realm
        || realm.profile != intent_profile
        || profile.profile != intent_profile
        || profile.realm != intent_realm
        || realm.profile_version != profile.version
        || usize::from(realm.max_outcomes) != MAX_OUTCOMES
        || outcomes > realm.max_outcomes
        || market.market != expected_market
        || market.realm != intent_realm
        || market.profile != intent_profile
        || market.outcome_count != outcomes
        || market.terms != terms
        || market.feed != feed
        || market.lifecycle != 0
        || hoard.market != market.market
        || hoard.realm != market.realm
        || position.market != market.market
        || kernel.market != market.market
        || kernel.phase != 0
        || external.market != market.market
        || external.owner != position.owner
        || external.position_generation != position.generation
        || replay.market != market.market
        || replay.owner != position.owner
        || replay.position_generation != position.generation
    {
        return Err(Error::MismatchedState);
    }
    supply
        .binds_market(market)
        .map_err(|_| Error::MismatchedState)?;
    /* The presented terms artifact must be the one this market's digest
     * binds; the artifact is self-certifying inside the codec, so digest
     * equality plus these field comparisons is equality of the whole
     * artifact. */
    terms_account
        .binds_market(market)
        .map_err(|_| Error::TermsBindingMismatch)?;
    /* The cap flow: the founding market's immutable collateral cap is the
     * terms' own — a digest-committed decision, never a writer's choice —
     * and the bound collateral policy must not refute it.  The terms codec
     * refuses a zero cap, so "cap 0 refuses at market init" is structural:
     * an unfundable-forever market cannot be founded. */
    if market.collateral_cap != terms_account.collateral_cap {
        return Err(Error::TermsBindingMismatch);
    }
    policy
        .check_market_cap(market.collateral_cap)
        .map_err(|_| Error::CollateralCap)?;
    if hoard.collateral_atoms != 0
        || position.close_state != 0
        || position.internal.iter().any(|amount| *amount != 0)
        || position.cash_atoms != 0
        || position.reserved_cash_atoms != 0
        || kernel.total_supply.iter().any(|amount| *amount != 0)
        || external.balances.iter().any(|amount| *amount != 0)
        || replay.sequence != 0
        || supply.internal_supply.iter().any(|amount| *amount != 0)
        || supply.external_supply.iter().any(|amount| *amount != 0)
    {
        return Err(Error::NonEmptyInitialization);
    }
    let pure = kernel_market(&decoded)?;
    pure.check_invariants()?;
    /* A market whose kernel pays something its own terms digest does not
     * commit to can never resolve; the binding is checked at creation, not
     * merely at resolution. */
    require_payout_set_binding(kernel, &terms_account)?;
    validate_padding(&decoded)?;
    validate_aggregate_closure(&decoded)?;
    Ok(())
}

/// Validate that a position triple enters an existing market provably zero.
///
/// This is the base case C0 of the multi-position closure scheme
/// (`docs/implementation/MULTI_POSITION_CLOSURE.md`): the represented-balances
/// invariant `sum over positions == ledger term` is inductive, so a position
/// (with its external shadow and replay accounts) may join the set it ranges
/// over only in the state that leaves every sum unchanged. Like
/// [`validate_market_init`], this validates and does not authorize or execute
/// creation; who may create a position, and that these bytes are the ones a
/// fresh account actually holds, are runtime facts
/// (`SOLANA_REFERENCE_ADAPTER.md` obligations 1 through 3).
///
/// The market-wide accounts (market, hoard, kernel, supply ledger) are
/// presented mid-life and checked for linkage, padding, closure, and kernel
/// invariants, not emptiness. The triple itself must be all zero: internal
/// balances, external shadow balances, position cash and reserved cash, replay
/// sequence, and an open close-state, with the three accounts mutually bound
/// to one owner and one generation. A market that is no longer active refuses:
/// new positions in a resolved market are outside this reference subset.
pub fn validate_position_init(
    state: StateBytes<'_>,
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
) -> Result<()> {
    validate_metadata(metadata, bindings, false)?;
    let decoded = DecodedState::decode(state)?;
    validate_links(&decoded, bindings)?;
    validate_padding(&decoded)?;
    validate_aggregate_closure(&decoded)?;
    let pure = kernel_market(&decoded)?;
    pure.check_invariants()?;
    if decoded.market.lifecycle != 0 || decoded.kernel.phase != 0 {
        return Err(Error::MismatchedState);
    }
    let DecodedState {
        position,
        external,
        replay,
        ..
    } = &decoded;
    if position.close_state != 0
        || position.internal.iter().any(|amount| *amount != 0)
        || position.cash_atoms != 0
        || position.reserved_cash_atoms != 0
        || external.balances.iter().any(|amount| *amount != 0)
        || replay.sequence != 0
    {
        return Err(Error::NonEmptyInitialization);
    }
    Ok(())
}

/// Refuse unless the Realm profile is frozen to **exactly** this policy.
///
/// The §3.5 wiring of `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md`:
/// the 266 policy bytes arrive as an evidence input, and
/// [`collateral::verify_collateral_binding`] recomputes the child digest
/// `D_col` from them and compares it against the Profile's stored digest —
/// decoding alone authenticates nothing, and a well-formed frozen Profile
/// can commit to another Realm's collateral policy.
///
/// The taxonomy does not move: an *unfrozen* Profile still refuses
/// [`Error::CollateralPolicyNotFrozen`], checked here before the binding
/// runs; every other refusal — a hostile policy blob, a foreign well-formed
/// policy, a bit-flipped stored digest — surfaces as the decoder's or
/// binding's own [`Error::Layout`] class.
fn require_collateral_binding(
    policy_bytes: &[u8],
    profile: &ProfileAccount,
) -> Result<collateral::CollateralPolicy> {
    if profile.flags & PROFILE_FLAG_POLICY_FROZEN == 0
        || profile.collateral_policy_digest == Hash32::ZERO
    {
        return Err(Error::CollateralPolicyNotFrozen);
    }
    /* The unfrozen case was refused above, so the binding function's own
     * unfrozen refusal (`ZeroIdentity`) is unreachable here and a remaining
     * `ZeroIdentity` is the policy decoder's currency refusal — a hostile
     * blob, not a freeze fault — surfaced as the layout class it is. */
    collateral::verify_collateral_binding(policy_bytes, profile).map_err(Error::Layout)
}

/// The seven decoded state accounts of one reference transition.
///
/// Grouping them keeps every check below reading named roles rather than a
/// positional argument list, which is also what stops a future field from
/// silently sliding into the wrong slot at one call site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedState {
    market: MarketAccount,
    hoard: HoardAccount,
    position: PositionAccount,
    kernel: KernelAccount,
    external: ExternalAccount,
    replay: ReplayAccount,
    supply: SupplyLedgerAccount,
}

impl DecodedState {
    fn decode(bytes: StateBytes<'_>) -> Result<Self> {
        Ok(Self {
            market: MarketAccount::decode(bytes.market)?,
            hoard: HoardAccount::decode(bytes.hoard)?,
            position: PositionAccount::decode(bytes.position)?,
            kernel: KernelAccount::decode(bytes.kernel)?,
            external: ExternalAccount::decode(bytes.external)?,
            replay: ReplayAccount::decode(bytes.replay)?,
            supply: SupplyLedgerAccount::decode(bytes.supply)?,
        })
    }
}

/// Apply one strict request to local copies and return exact post-state bytes.
///
/// No caller-provided output is mutated on error. The returned state is an
/// offline transition witness, not evidence of SVM execution or token movement.
///
/// This entry point supplies no evidence plane, so `Action::Resolve` and
/// `Action::RedeemInternal` refuse with
/// [`Error::ResolutionEvidenceUnavailable`] whatever the account bytes claim.
/// That is the same refusal class the crate returned unconditionally before
/// the evidence plane existed, and it is what makes the fail-closed default a
/// missing code path rather than a flag. See [`apply_with_evidence`].
pub fn apply(
    request_bytes: &[u8],
    state: StateBytes<'_>,
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
) -> Result<TransitionOutput> {
    apply_inner(request_bytes, state, None, metadata, bindings)
}

/// Apply one strict request together with a typed resolution evidence plane.
///
/// Supplying evidence does not authorize anything. Every element is checked
/// against terms the market's own digest commits to, and the sealed
/// [`WindowResult`] is built here by driving the accumulator's state machine
/// over the caller's observation records, so no "sealed" assertion crosses the
/// wire. Any missing, mismatched, immature, incomplete, unsealed,
/// wrong-generation, or ambiguous input is a distinct refusal.
///
/// A layout intent admits no evidence plane and refuses
/// [`Error::UnexpectedEvidence`] rather than silently ignoring it.
pub fn apply_with_evidence(
    request_bytes: &[u8],
    state: StateBytes<'_>,
    evidence: &ResolutionEvidence<'_>,
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
) -> Result<TransitionOutput> {
    apply_inner(request_bytes, state, Some(evidence), metadata, bindings)
}

/// Apply one market-global, evidence-gated Resolve transition.
///
/// This is the canonical resolution oracle. Its input type cannot carry a
/// Position, per-holder external shadow, or owner Replay account. The request
/// sequence is the immutable repair generation selected by Terms and the
/// sealed window, not an incrementing owner nonce. The Resolution account is
/// the sole persisted replay fact: an exact repeat is accepted byte-for-byte
/// without advancing any counter, while any conflicting repeat refuses.
pub fn apply_market_resolution_with_evidence(
    request_bytes: &[u8],
    state: ResolutionStateBytes<'_>,
    evidence: &ResolutionEvidence<'_>,
    metadata: &ResolutionTransitionMetadata,
    bindings: &ResolutionExpectedBindings,
) -> Result<ResolutionTransitionOutput> {
    validate_resolution_metadata(metadata, bindings, evidence)?;
    let request = Request::decode(request_bytes)?;
    let requested_payout = match request.action {
        Action::Resolve { payout_index } => payout_index,
        _ => return Err(Error::UnsupportedIntent),
    };

    let mut market = MarketAccount::decode(state.market)?;
    let hoard = HoardAccount::decode(state.hoard)?;
    let mut kernel = KernelAccount::decode(state.kernel)?;
    let supply = SupplyLedgerAccount::decode(state.supply)?;
    if market.stored_bump != bindings.market_bump
        || market.hoard_bump != bindings.hoard_bump
        || hoard.stored_bump != bindings.hoard_bump
        || supply.stored_bump != bindings.supply_bump
    {
        return Err(Error::WrongBump);
    }
    if market.market != hoard.market
        || market.realm != hoard.realm
        || market.market != kernel.market
        || market.market != supply.market
        || market.realm != supply.realm
        || market.outcome_count != supply.outcome_count
        || kernel.payouts.outcomes != market.outcome_count
        || (market.lifecycle == 0 && kernel.phase != 0)
        || (market.lifecycle == 1 && kernel.phase != 1)
        || market.lifecycle > 1
    {
        return Err(Error::MismatchedState);
    }
    let mut outcome = 0_usize;
    while outcome < usize::from(market.outcome_count) {
        if supply
            .aggregate_supply(outcome as u8)
            .map_err(Error::Layout)?
            != kernel.total_supply[outcome]
        {
            return Err(Error::AggregateClosureMismatch);
        }
        outcome += 1;
    }
    while outcome < MAX_OUTCOMES {
        if kernel.total_supply[outcome] != 0 {
            return Err(Error::NonCanonical);
        }
        outcome += 1;
    }

    if !metadata.actor.signer {
        return Err(Error::MissingSignature);
    }
    let mut terms = TermsAccount::ZEROED;
    TermsAccount::decode_into(evidence.bytes.terms, &mut terms)?;
    if terms.stored_bump != evidence.bindings.terms_bump {
        return Err(Error::WrongBump);
    }
    terms
        .binds_market(&market)
        .map_err(|_| Error::TermsBindingMismatch)?;
    require_payout_set_binding(&kernel, &terms)?;
    let record = ResolutionAccount::decode(evidence.bytes.resolution)?;
    if record.stored_bump != evidence.bindings.resolution_bump {
        return Err(Error::WrongBump);
    }
    if record.market != market.market {
        return Err(Error::ResolutionBindingMismatch);
    }
    record
        .binds_terms(&terms)
        .map_err(|_| Error::ResolutionBindingMismatch)?;

    let derived = ResolutionTerms::from_market_terms(&market, &terms)?;
    let window = fold_window_evidence(evidence.bytes.window, evidence.feed_cursor)?;
    /* The persisted account pair still carries only a finite-preset index.
     * A native derived-basis resolution must go through
     * `derive_payout_vector` + `resolve_with_vector`; searching the preset set
     * here would silently turn a shaped claim back into portfolio sugar. */
    if derived.basis_degree != 0 {
        return Err(Error::Resolution(ResolutionRefusal::WrongResolutionMode));
    }
    let payout_index = derive_payout(&derived, &terms.payouts, &window)?;
    if payout_index != requested_payout {
        return Err(Error::PayoutIndexMismatch);
    }
    if request.sequence != terms.repair_generation
        || request.sequence != window.domain().generation()
    {
        return Err(Error::Replay);
    }

    let repeated = record.is_resolved();
    let expected = ResolutionAccount {
        market: market.market,
        terms: terms.terms,
        feed: terms.feed,
        window: evidence.bindings.window_id,
        feed_cursor: if repeated {
            record.feed_cursor
        } else {
            window.sealed_cursor()
        },
        sealed_end_bucket_exclusive: window.domain().end_bucket_exclusive(),
        repair_generation: window.domain().generation(),
        resolved_slot: if repeated {
            record.resolved_slot
        } else {
            evidence.resolved_slot
        },
        payout_index,
        stored_bump: evidence.bindings.resolution_bump,
        flags: 0,
    };

    match (market.lifecycle, kernel.phase, repeated) {
        (0, 0, false) => {
            let mut pure = MarketState {
                outcomes: market.outcome_count,
                phase: Phase::Active,
                resolved_payout: kernel.resolved_payout,
                basis_mode: BasisMode::FinitePreset,
                resolved_vector: PayoutVector::ZERO,
                collateral: hoard.collateral_atoms,
                total_supply: kernel.total_supply,
                payouts: kernel.payouts,
            };
            pure.check_invariants()?;
            pure.resolve(requested_payout)?;
            market.lifecycle = 1;
            kernel.phase = 1;
            kernel.resolved_payout = pure.resolved_payout;
            kernel.total_supply = pure.total_supply;
        }
        (1, 1, true) => {
            if kernel.resolved_payout != requested_payout || record != expected {
                return Err(Error::ResolutionBindingMismatch);
            }
        }
        (0, 0, true) => return Err(Error::ResolutionAlreadyRecorded),
        _ => return Err(Error::MismatchedState),
    }

    let mut output = ResolutionTransitionOutput {
        market: [0; account_len::MARKET],
        hoard: [0; account_len::HOARD],
        kernel: [0; KERNEL_ACCOUNT_LEN],
        supply: [0; account_len::SUPPLY_LEDGER],
        resolution: [0; account_len::RESOLUTION],
        repeated,
    };
    market.encode(&mut output.market)?;
    hoard.encode(&mut output.hoard)?;
    kernel.encode(&mut output.kernel)?;
    supply.encode(&mut output.supply)?;
    expected.encode(&mut output.resolution)?;
    Ok(output)
}

fn apply_inner(
    request_bytes: &[u8],
    state: StateBytes<'_>,
    evidence: Option<&ResolutionEvidence<'_>>,
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
) -> Result<TransitionOutput> {
    validate_metadata(metadata, bindings, true)?;
    let request = Request::decode(request_bytes)?;
    let mut decoded = DecodedState::decode(state)?;
    validate_links(&decoded, bindings)?;
    validate_padding(&decoded)?;
    validate_aggregate_closure(&decoded)?;
    if request.sequence != decoded.replay.sequence {
        return Err(Error::Replay);
    }
    let next_sequence = decoded
        .replay
        .sequence
        .checked_add(1)
        .ok_or(Error::Replay)?;
    let mut pure_market = kernel_market(&decoded)?;
    let mut pure_position = Position {
        internal: decoded.position.internal,
        external: decoded.external.balances,
    };
    let mut resolution_bytes = None;
    let payout = match request.action {
        Action::Layout(intent) => {
            if evidence.is_some() {
                return Err(Error::UnexpectedEvidence);
            }
            match intent {
                Intent::Split {
                    market: intent_market,
                    owner,
                    quantity,
                } => {
                    authorize_owner(metadata.actor, decoded.position.owner)?;
                    require_intent_binding(
                        intent_market,
                        owner,
                        &decoded.market,
                        &decoded.position,
                    )?;
                    if decoded.market.lifecycle != 0 || decoded.position.close_state != 0 {
                        return Err(Error::MismatchedState);
                    }
                    let next_collateral = decoded
                        .hoard
                        .collateral_atoms
                        .checked_add(quantity)
                        .ok_or(Error::Arithmetic)?;
                    if next_collateral > decoded.market.collateral_cap {
                        return Err(Error::CollateralCap);
                    }
                    if decoded.position.free_cash_atoms().map_err(Error::Layout)? < quantity {
                        return Err(Error::Arithmetic);
                    }
                    decoded.position.cash_atoms = decoded
                        .position
                        .cash_atoms
                        .checked_sub(quantity)
                        .ok_or(Error::Arithmetic)?;
                    pure_market.split(&mut pure_position, quantity)?;
                    0
                }
                Intent::Merge {
                    market: intent_market,
                    owner,
                    quantity,
                } => {
                    authorize_owner(metadata.actor, decoded.position.owner)?;
                    require_intent_binding(
                        intent_market,
                        owner,
                        &decoded.market,
                        &decoded.position,
                    )?;
                    /* Same phase discipline as `Split`.  `MarketState::merge`
                     * already refuses a resolved market through
                     * `require_active`, so the lifecycle half is redundant with
                     * the kernel and the `close_state` half is not: a closing
                     * position must not recombine its way back into cash. */
                    if decoded.market.lifecycle != 0 || decoded.position.close_state != 0 {
                        return Err(Error::MismatchedState);
                    }
                    /* NO COLLATERAL-CAP CHECK, deliberately.  `Split` checks
                     * the cap because it is the only transition that raises
                     * `hoard.collateral_atoms`; `merge` lowers it
                     * (`MarketState::merge` refuses `InsufficientCollateral`
                     * and then subtracts), so the post-state collateral is
                     * strictly below the pre-state collateral and cannot cross
                     * a ceiling it was under.  A cap check here would be worse
                     * than redundant: a market already above its cap — a cap
                     * lowered by some future governance path, say — would be
                     * unable to unwind, which is the one direction that always
                     * has to stay open. */
                    pure_market.merge(&mut pure_position, quantity)?;
                    /* The cash credit is the *consequence* of the burn, so it
                     * lands after the kernel step that justified it. That is
                     * the mirror image of `Split`, where the debit is the
                     * precondition and precedes the mint, and it is the order
                     * `Action::RedeemInternal` already credits a payout in.
                     * `quantity` is the released collateral because a complete
                     * set is worth exactly one atom of collateral: the same
                     * one-to-one the kernel enforces on both sides. */
                    decoded.position.cash_atoms = decoded
                        .position
                        .cash_atoms
                        .checked_add(quantity)
                        .ok_or(Error::Arithmetic)?;
                    0
                }
                Intent::Materialize {
                    market: intent_market,
                    owner,
                    destination,
                    outcome,
                    quantity,
                } => {
                    authorize_owner(metadata.actor, decoded.position.owner)?;
                    require_intent_binding(
                        intent_market,
                        owner,
                        &decoded.market,
                        &decoded.position,
                    )?;
                    if destination != metadata.external.key {
                        return Err(Error::WrongAccountKey);
                    }
                    pure_market.materialize(&mut pure_position, outcome, quantity)?;
                    0
                }
                Intent::Dematerialize {
                    market: intent_market,
                    owner,
                    source,
                    outcome,
                    quantity,
                } => {
                    authorize_owner(metadata.actor, decoded.position.owner)?;
                    require_intent_binding(
                        intent_market,
                        owner,
                        &decoded.market,
                        &decoded.position,
                    )?;
                    if source != metadata.external.key {
                        return Err(Error::WrongAccountKey);
                    }
                    pure_market.dematerialize(&mut pure_position, outcome, quantity)?;
                    0
                }
                Intent::CreateMarket { .. } => return Err(Error::AuthorizationUnavailable),
                _ => return Err(Error::UnsupportedIntent),
            }
        }
        Action::Resolve { payout_index } => {
            let evidence = evidence.ok_or(Error::ResolutionEvidenceUnavailable)?;
            validate_evidence_metadata(metadata, bindings, evidence, true)?;
            // Resolution is non-discretionary: the typed evidence authorizes
            // it, no key does. A signature is still required because a
            // transaction has a fee payer, but no signer is privileged and no
            // signer can substitute for any element of the gate.
            if !metadata.actor.signer {
                return Err(Error::MissingSignature);
            }
            let record = resolve_from_evidence(&decoded, evidence, payout_index)?;
            pure_market.resolve(payout_index)?;
            decoded.market.lifecycle = 1;
            let mut bytes = [0; account_len::RESOLUTION];
            record.encode(&mut bytes)?;
            resolution_bytes = Some(bytes);
            0
        }
        Action::RedeemInternal { outcome, quantity } => {
            let evidence = evidence.ok_or(Error::ResolutionEvidenceUnavailable)?;
            validate_evidence_metadata(metadata, bindings, evidence, false)?;
            authorize_owner(metadata.actor, decoded.position.owner)?;
            let record = redeem_from_evidence(&decoded, evidence)?;
            let paid = pure_market.redeem_internal(&mut pure_position, outcome, quantity)?;
            decoded.position.cash_atoms = decoded
                .position
                .cash_atoms
                .checked_add(paid)
                .ok_or(Error::Arithmetic)?;
            let mut bytes = [0; account_len::RESOLUTION];
            record.encode(&mut bytes)?;
            resolution_bytes = Some(bytes);
            paid
        }
    };
    decoded.hoard.collateral_atoms = pure_market.collateral;
    apply_position_delta_to_ledger(
        &mut decoded.supply,
        decoded.market.outcome_count,
        &decoded.position.internal,
        &decoded.external.balances,
        &pure_position,
    )?;
    decoded.position.internal = pure_position.internal;
    decoded.external.balances = pure_position.external;
    decoded.kernel.phase = match pure_market.phase {
        Phase::Active => 0,
        Phase::Resolved => 1,
    };
    decoded.kernel.resolved_payout = pure_market.resolved_payout;
    decoded.kernel.total_supply = pure_market.total_supply;
    decoded.replay.sequence = next_sequence;
    validate_aggregate_closure(&decoded)?;
    encode_output(&decoded, resolution_bytes, payout)
}

/// The exact canonical preimage a market's frozen terms name for its window.
///
/// This crate publishes the preimage rather than a digest for the same reason
/// `clutch_accumulator` does: it owns no hash primitive, so an independent
/// recomputation and a hashing adapter cannot disagree about what was hashed.
/// A `WindowId` is `HASH(WINDOW_DOMAIN_TAG || these bytes)`.
pub fn expected_window_preimage(
    market: &MarketAccount,
    terms: &TermsAccount,
) -> Result<[u8; WINDOW_DOMAIN_BYTES]> {
    let derived = ResolutionTerms::from_market_terms(market, terms)?;
    let mut out = [0; WINDOW_DOMAIN_BYTES];
    derived.window.encode_canonical(&mut out);
    Ok(out)
}

/// Resolve a derived-basis market from sealed evidence, with no preset bridge.
///
/// The design's §4 and §5.1 seams joined end to end: the derivation produces
/// the validated weight vector at the resolved value, and the kernel installs
/// exactly that vector through `MarketState::resolve_with_vector`.  No step
/// searches the frozen preset set, so the reachable lattice is no longer
/// capped at `MAX_PAYOUTS` members. For a two-outcome degree-1 market the whole
/// `D + 1` member lattice resolves, not the eight vectors an enumeration could
/// hold.
///
/// The division of labour is unchanged from resolve-by-index: the derivation
/// binds the vector to digest-bound terms and one sealed `WindowResult`, and
/// the kernel checks only (H1)/(H2) shape against the frozen `D`.  This
/// function reads no account, clock, or signer and writes nothing but the
/// market it is handed; it is pure and total, exactly like the two seams it
/// composes.
///
/// Both mode gates are live and independent.  Degree-0 terms refuse
/// [`ResolutionRefusal::WrongResolutionMode`] in the derivation, and a
/// `FinitePreset` market refuses `KernelError::WrongResolutionMode` in the
/// kernel, so a caller cannot cross the seams by supplying a matched pair of
/// the wrong kind.
///
/// What this is *not*: it is not the `Action::Resolve` account path. The SBF
/// adapter owns that account binding, including the native-resolution record;
/// this helper remains the pure derivation-to-kernel seam.
pub fn resolve_derived_market(
    market: &mut MarketState,
    terms: &ResolutionTerms,
    window: &WindowResult,
) -> Result<PayoutVector> {
    let derived = derive_payout_vector(terms, window)?;
    // The two crates agree on `MAX_OUTCOMES` by construction: this line does
    // not compile if the weight arrays ever stop being the same type.
    let vector = PayoutVector::new(derived.denominator, derived.weights);
    market.resolve_with_vector(vector)?;
    Ok(vector)
}

/// Fold a caller-supplied window-evidence blob into a sealed [`WindowResult`].
///
/// The blob declares its own domain, and the fold runs the accumulator's
/// `Open -> Mature -> Sealed` state machine over its observation records. The
/// caller cannot assert maturity, completeness, contiguity, or a seal: each is
/// a consequence of the records and the witnessed feed cursor, and each failure
/// is the accumulator's own named refusal. Checking the declared domain against
/// the market's expected one happens later, in [`derive_payout`], so a wrong
/// feed, window, maturity, generation, grid, or coverage policy is reported as
/// the field that differed rather than as a decode failure.
fn fold_window_evidence(bytes: &[u8], feed_cursor: u64) -> Result<WindowResult> {
    if bytes.len() < WINDOW_EVIDENCE_HEADER_BYTES || bytes.len() > MAX_WINDOW_EVIDENCE_LEN {
        return Err(Error::WrongLength);
    }
    let count = usize::from(u16::from_le_bytes([
        bytes[WINDOW_EVIDENCE_HEADER_BYTES - 2],
        bytes[WINDOW_EVIDENCE_HEADER_BYTES - 1],
    ]));
    if count > MAX_OBSERVATIONS {
        return Err(Error::WrongLength);
    }
    let expected = WINDOW_EVIDENCE_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(OBSERVATION_RECORD_BYTES)
                .ok_or(Error::WrongLength)?,
        )
        .ok_or(Error::WrongLength)?;
    let mut reader = Reader::new(bytes, expected, WINDOW_EVIDENCE_TAG)?;
    let source_adapter_id = reader.raw::<IDENTITY_BYTES>()?;
    let feed_spec_id = reader.raw::<IDENTITY_BYTES>()?;
    let source_version = reader.u32()?;
    let evaluator_version = reader.u32()?;
    let grid_family_id = reader.u32()?;
    let grid_version = reader.u16()?;
    let bucket_seconds = reader.u64()?;
    let start_bucket = reader.u64()?;
    let end_bucket_exclusive = reader.u64()?;
    let maturity_bucket_exclusive = reader.u64()?;
    let generation = reader.u64()?;
    let coverage_policy_id = reader.u16()?;
    let coverage_policy_parameter = reader.u64()?;
    let declared_count = reader.u16()?;
    if usize::from(declared_count) != count {
        return Err(Error::WrongLength);
    }
    let feed = FeedIdentity::new(
        source_adapter_id,
        feed_spec_id,
        source_version,
        evaluator_version,
    )?;
    let grid = Grid::new(grid_family_id, grid_version, bucket_seconds)
        .map_err(|error| Error::Window(WindowError::Summary(error)))?;
    let coverage = CoveragePolicy::from_registry(coverage_policy_id, coverage_policy_parameter)?;
    let domain = WindowDomain::new(
        feed,
        grid,
        start_bucket,
        end_bucket_exclusive,
        maturity_bucket_exclusive,
        generation,
        coverage,
    )?;
    let mut window = WindowAccumulator::open(domain);
    let mut index = 0usize;
    while index < count {
        let kind = reader.u8()?;
        let bucket = reader.u64()?;
        let low = reader.u128()?;
        let high = reader.u128()?;
        let observation = match kind {
            OBSERVATION_MISSING => {
                if low != 0 || high != 0 {
                    return Err(Error::NonCanonical);
                }
                Observation::missing(bucket)
            }
            OBSERVATION_ACCEPTED => Observation::accepted(bucket, low, high),
            _ => return Err(Error::NonCanonical),
        };
        window.observe(observation)?;
        index += 1;
    }
    reader.done()?;
    window.witness_feed_cursor(feed_cursor)?;
    window.seal()?;
    Ok(window.result()?)
}

/// Bind the reference-only kernel payout set to the immutable terms artifact.
///
/// The kernel account is reference-only state, so its payout set used to be
/// unbound caller bytes. `MarketAccount.terms` is the digest of the terms body,
/// and that body contains the payout vectors, so requiring equality here is
/// what makes "the payouts this market pays" a committed fact rather than an
/// assertion of whoever assembled the transaction.
fn require_payout_set_binding(kernel: &KernelAccount, terms: &TermsAccount) -> Result<()> {
    let expected_mode = if terms.basis_degree == 0 {
        BasisMode::FinitePreset
    } else {
        BasisMode::DerivedBasis
    };
    if kernel.basis_mode != expected_mode {
        return Err(Error::Kernel(KernelError::WrongResolutionMode));
    }
    if kernel.payouts.count != terms.payout_count || kernel.payouts.outcomes != terms.outcome_count
    {
        return Err(Error::PayoutSetMismatch);
    }
    let mut index = 0usize;
    while index < MAX_PAYOUTS {
        let vector = kernel.payouts.vectors[index];
        let frozen = terms.payouts[index];
        if vector.denominator != frozen.denominator || vector.weights != frozen.weights {
            return Err(Error::PayoutSetMismatch);
        }
        index += 1;
    }
    Ok(())
}

/// Decode and bind the immutable terms artifact for one market.
///
/// Writes into a caller-owned slot (`TermsAccount::decode_into` discipline):
/// the account is over 1.6 KiB, so the by-value form cost every evidence
/// gate two account-sized frame copies.  On `Err`, `out` holds an
/// unspecified partial decode and must not be read.
fn bind_terms(
    market: &MarketAccount,
    kernel: &KernelAccount,
    evidence: &ResolutionEvidence<'_>,
    out: &mut TermsAccount,
) -> Result<()> {
    TermsAccount::decode_into(evidence.bytes.terms, out)?;
    if out.stored_bump != evidence.bindings.terms_bump {
        return Err(Error::WrongBump);
    }
    out.binds_market(market)
        .map_err(|_| Error::TermsBindingMismatch)?;
    require_payout_set_binding(kernel, out)?;
    Ok(())
}

/// Decode and bind the resolution record for one market's immutable terms.
fn bind_resolution(
    market: &MarketAccount,
    terms: &TermsAccount,
    evidence: &ResolutionEvidence<'_>,
) -> Result<ResolutionAccount> {
    let record = ResolutionAccount::decode(evidence.bytes.resolution)?;
    if record.stored_bump != evidence.bindings.resolution_bump {
        return Err(Error::WrongBump);
    }
    if record.market != market.market {
        return Err(Error::ResolutionBindingMismatch);
    }
    record
        .binds_terms(terms)
        .map_err(|_| Error::ResolutionBindingMismatch)?;
    Ok(record)
}

/// The full `Action::Resolve` evidence gate.
///
/// Every step is a distinct refusal, in this order: the market must be active;
/// the terms artifact must be the one the market's digest binds; the frozen
/// payout set must be the kernel's; the resolution record must be bound and
/// still unresolved; the terms must derive a V1-admissible
/// [`ResolutionTerms`]; the observation records must fold into a complete,
/// mature, sealed [`WindowResult`]; that result must be bound to exactly the
/// terms' domain; the derivation must select one payout unambiguously; and the
/// request must be asking for exactly that payout.
fn resolve_from_evidence(
    state: &DecodedState,
    evidence: &ResolutionEvidence<'_>,
    requested_payout: u8,
) -> Result<ResolutionAccount> {
    let market = &state.market;
    if market.lifecycle != 0 || state.kernel.phase != 0 {
        return Err(Error::Resolution(ResolutionRefusal::MarketNotActive));
    }
    let mut terms = TermsAccount::ZEROED;
    bind_terms(market, &state.kernel, evidence, &mut terms)?;
    let record = bind_resolution(market, &terms, evidence)?;
    if record.is_resolved() {
        return Err(Error::ResolutionAlreadyRecorded);
    }
    let derived = ResolutionTerms::from_market_terms(market, &terms)?;
    let window = fold_window_evidence(evidence.bytes.window, evidence.feed_cursor)?;
    if derived.basis_degree != 0 {
        return Err(Error::Resolution(ResolutionRefusal::WrongResolutionMode));
    }
    let payout_index = derive_payout(&derived, &terms.payouts, &window)?;
    if payout_index != requested_payout {
        return Err(Error::PayoutIndexMismatch);
    }
    Ok(ResolutionAccount {
        market: market.market,
        terms: terms.terms,
        feed: terms.feed,
        window: evidence.bindings.window_id,
        feed_cursor: window.sealed_cursor(),
        sealed_end_bucket_exclusive: window.domain().end_bucket_exclusive(),
        repair_generation: window.domain().generation(),
        resolved_slot: evidence.resolved_slot,
        payout_index,
        stored_bump: evidence.bindings.resolution_bump,
        flags: 0,
    })
}

/// The full `Action::RedeemInternal` evidence gate.
///
/// Redemption's authority is the recorded resolution, not a re-fold, so the
/// window blob must be empty: re-deriving a payout at redemption time would
/// create a second place a payout can be decided. The record must be bound to
/// the market's immutable terms, must have selected a payout inside that
/// frozen set, and must agree with the resolved kernel state exactly. Forged
/// resolved market/kernel bytes without this chain therefore still refuse.
fn redeem_from_evidence(
    state: &DecodedState,
    evidence: &ResolutionEvidence<'_>,
) -> Result<ResolutionAccount> {
    if !evidence.bytes.window.is_empty() {
        return Err(Error::UnexpectedEvidence);
    }
    let market = &state.market;
    let mut terms = TermsAccount::ZEROED;
    bind_terms(market, &state.kernel, evidence, &mut terms)?;
    let record = bind_resolution(market, &terms, evidence)?;
    if !record.is_resolved() {
        return Err(Error::ResolutionNotRecorded);
    }
    if record.window != evidence.bindings.window_id {
        return Err(Error::ResolutionBindingMismatch);
    }
    if record.payout_index >= terms.payout_count {
        return Err(Error::Resolution(ResolutionRefusal::PayoutIndexOutOfRange));
    }
    if market.lifecycle != 1
        || state.kernel.phase != 1
        || state.kernel.resolved_payout != record.payout_index
    {
        return Err(Error::MismatchedState);
    }
    Ok(record)
}

fn kernel_market(state: &DecodedState) -> Result<MarketState> {
    let DecodedState {
        market,
        hoard,
        kernel,
        ..
    } = state;
    if usize::from(market.outcome_count) > KERNEL_MAX_OUTCOMES
        || kernel.payouts.outcomes != market.outcome_count
    {
        return Err(Error::MismatchedState);
    }
    let phase = match kernel.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(Error::NonCanonical),
    };
    if phase == Phase::Resolved && kernel.basis_mode == BasisMode::DerivedBasis {
        /* KernelAccount owns only the immutable mode.  The v3 Resolution
         * record remains the sole persisted owner of a native payout vector,
         * and this legacy account-shaped adapter is not passed that record. */
        return Err(Error::Kernel(KernelError::WrongResolutionMode));
    }
    let pure = MarketState {
        outcomes: market.outcome_count,
        phase,
        resolved_payout: kernel.resolved_payout,
        basis_mode: kernel.basis_mode,
        resolved_vector: PayoutVector::ZERO,
        collateral: hoard.collateral_atoms,
        total_supply: kernel.total_supply,
        payouts: kernel.payouts,
    };
    pure.check_invariants()?;
    Ok(pure)
}

fn validate_metadata(
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
    writable: bool,
) -> Result<()> {
    let accounts = [
        metadata.market,
        metadata.hoard,
        metadata.position,
        metadata.kernel,
        metadata.external,
        metadata.replay,
        metadata.supply,
    ];
    let expected = [
        bindings.market,
        bindings.hoard,
        bindings.position,
        bindings.kernel,
        bindings.external,
        bindings.replay,
        bindings.supply,
    ];
    for (index, account) in accounts.iter().enumerate() {
        if account.owner_program != bindings.program_id {
            return Err(Error::WrongProgramOwner);
        }
        if account.key != expected[index] {
            return Err(Error::WrongAccountKey);
        }
        if writable && !account.writable {
            return Err(Error::NotWritable);
        }
        if account.key == metadata.actor.key {
            return Err(Error::AccountAlias);
        }
        for other in &accounts[index + 1..] {
            if account.key == other.key {
                return Err(Error::AccountAlias);
            }
        }
    }
    Ok(())
}

fn validate_resolution_metadata(
    metadata: &ResolutionTransitionMetadata,
    bindings: &ResolutionExpectedBindings,
    evidence: &ResolutionEvidence<'_>,
) -> Result<()> {
    let accounts = [
        metadata.market,
        metadata.hoard,
        metadata.kernel,
        metadata.supply,
        evidence.metadata.terms,
        evidence.metadata.resolution,
    ];
    let expected = [
        bindings.market,
        bindings.hoard,
        bindings.kernel,
        bindings.supply,
        evidence.bindings.terms,
        evidence.bindings.resolution,
    ];
    for (index, account) in accounts.iter().enumerate() {
        if account.owner_program != bindings.program_id {
            return Err(Error::WrongProgramOwner);
        }
        if account.key != expected[index] {
            return Err(Error::WrongAccountKey);
        }
        if account.key == metadata.actor.key {
            return Err(Error::AccountAlias);
        }
        for other in &accounts[index + 1..] {
            if account.key == other.key {
                return Err(Error::AccountAlias);
            }
        }
    }
    if !metadata.market.writable || !metadata.kernel.writable || !metadata.supply.writable {
        return Err(Error::NotWritable);
    }
    if metadata.hoard.writable || evidence.metadata.terms.writable {
        return Err(Error::ImmutableAccountWritable);
    }
    if !evidence.metadata.resolution.writable {
        return Err(Error::NotWritable);
    }
    if evidence.bindings.window_id == Hash32::ZERO {
        return Err(Error::WindowIdentityUnavailable);
    }
    Ok(())
}

/// Apply the same metadata discipline to the two evidence-plane accounts.
///
/// The immutable terms artifact must never be presented writable, and the
/// resolution record must be writable exactly when the action writes it. An
/// account presented with the wrong mutability is a refusal rather than a
/// tolerated over-permission, because a real runtime would let that write land.
fn validate_evidence_metadata(
    metadata: &TransitionMetadata,
    bindings: &ExpectedBindings,
    evidence: &ResolutionEvidence<'_>,
    resolution_writable: bool,
) -> Result<()> {
    let state = [
        metadata.market,
        metadata.hoard,
        metadata.position,
        metadata.kernel,
        metadata.external,
        metadata.replay,
        metadata.supply,
    ];
    let extra = [
        (evidence.metadata.terms, evidence.bindings.terms),
        (evidence.metadata.resolution, evidence.bindings.resolution),
    ];
    for (index, (account, expected)) in extra.iter().enumerate() {
        if account.owner_program != bindings.program_id {
            return Err(Error::WrongProgramOwner);
        }
        if account.key != *expected {
            return Err(Error::WrongAccountKey);
        }
        if account.key == metadata.actor.key {
            return Err(Error::AccountAlias);
        }
        for other in &state {
            if account.key == other.key {
                return Err(Error::AccountAlias);
            }
        }
        for (other, _) in &extra[index + 1..] {
            if account.key == other.key {
                return Err(Error::AccountAlias);
            }
        }
    }
    if evidence.metadata.terms.writable {
        return Err(Error::ImmutableAccountWritable);
    }
    if resolution_writable {
        if !evidence.metadata.resolution.writable {
            return Err(Error::NotWritable);
        }
    } else if evidence.metadata.resolution.writable {
        return Err(Error::ImmutableAccountWritable);
    }
    if evidence.bindings.window_id == Hash32::ZERO {
        return Err(Error::WindowIdentityUnavailable);
    }
    Ok(())
}

/// Cross-account identity, bump, and lifecycle linkage.
///
/// `position.generation` is the per-position close/reopen era and stays bound
/// to the triple: the external shadow and replay accounts must carry it
/// exactly. `supply.generation` is the market accounting era and is
/// deliberately *not* identified with any position's generation (the retired
/// closed single-position model equated the two): the reference admits one
/// era per ledger lifetime — no instruction writes it — and the proposed SVM
/// rule derives the ledger PDA from the market with no close path, so an era
/// bump is structurally impossible. See
/// `docs/implementation/MULTI_POSITION_CLOSURE.md` §4.
fn validate_links(state: &DecodedState, bindings: &ExpectedBindings) -> Result<()> {
    let DecodedState {
        market,
        hoard,
        position,
        kernel,
        external,
        replay,
        supply,
    } = state;
    if market.stored_bump != bindings.market_bump
        || market.hoard_bump != bindings.hoard_bump
        || hoard.stored_bump != bindings.hoard_bump
        || position.stored_bump != bindings.position_bump
        || external.stored_bump != bindings.external_bump
        || replay.stored_bump != bindings.replay_bump
        || supply.stored_bump != bindings.supply_bump
    {
        return Err(Error::WrongBump);
    }
    if market.market != hoard.market
        || market.realm != hoard.realm
        || market.market != position.market
        || market.market != kernel.market
        || market.market != external.market
        || market.market != replay.market
        || market.market != supply.market
        || market.realm != supply.realm
        || market.outcome_count != supply.outcome_count
        || position.owner != external.owner
        || position.owner != replay.owner
        || position.generation != external.position_generation
        || position.generation != replay.position_generation
        || (market.lifecycle == 0 && kernel.phase != 0)
        || (market.lifecycle == 1 && kernel.phase != 1)
        || market.lifecycle > 1
    {
        return Err(Error::MismatchedState);
    }
    Ok(())
}

fn validate_padding(state: &DecodedState) -> Result<()> {
    let DecodedState {
        market,
        position,
        kernel,
        external,
        ..
    } = state;
    let count = usize::from(market.outcome_count);
    if position.internal[count..].iter().any(|amount| *amount != 0)
        || kernel.total_supply[count..]
            .iter()
            .any(|amount| *amount != 0)
        || external.balances[count..].iter().any(|amount| *amount != 0)
    {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

/// The multi-position closure checks C1 and C2 of CLO-DELTA-V1.
///
/// [`SupplyLedgerAccount`] persists the market-wide aggregate as the two terms
/// whose sum it is: claims still credited internally and claims materialized
/// outside the internal ledger and accounted for. Under
/// `docs/implementation/MULTI_POSITION_CLOSURE.md` this checks, per active
/// outcome, both of:
///
/// ```text
/// C1  supply.internal[o] + supply.external[o] == kernel.total_supply[o]
/// C2  position.internal[o] <= supply.internal[o]
///     external.balances[o] <= supply.external[o]
/// ```
///
/// C1 is the ledger's own two-term closure and holds for any number of
/// positions. C2 replaces the retired closed single-position equalities: it is
/// the part of the represented-balances invariant
/// `sum over positions == ledger term` that one transition can check about the
/// one triple it sees. The full sum invariant is inductive — established at
/// initialization ([`validate_market_init`], [`validate_position_init`]) and
/// preserved by the checked delta write-back
/// ([`apply_position_delta_to_ledger`]) — never scanned. The bound is
/// deliberately one-sided: a ledger term exceeding the presented position is
/// another position's business (or stranded, conservatively locked claims),
/// while a position exceeding the ledger term is a counterfeit claim and
/// refuses.
fn validate_aggregate_closure(state: &DecodedState) -> Result<()> {
    let DecodedState {
        market,
        position,
        kernel,
        external,
        supply,
        ..
    } = state;
    let mut outcome = 0_usize;
    while outcome < usize::from(market.outcome_count) {
        let index = outcome as u8;
        let aggregate = supply
            .aggregate_supply(index)
            .map_err(|_| Error::Arithmetic)?;
        if aggregate != kernel.total_supply[outcome] {
            return Err(Error::AggregateClosureMismatch);
        }
        if position.internal[outcome] > supply.internal_supply[outcome]
            || external.balances[outcome] > supply.external_supply[outcome]
        {
            return Err(Error::AggregateClosureMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

/// The inductive step C3 of CLO-DELTA-V1: `ledger' = ledger - pre + post`.
///
/// The ledger is never overwritten with the presented position; it is moved by
/// exactly the delta the transition applied to that position, per term, per
/// active outcome, with checked arithmetic. Padding beyond the active outcome
/// count is untouched (it is validated zero on both sides). An underflow means
/// the position exceeded the ledger term it must be represented in — a closure
/// violation, unreachable after the C2 pre-check — and an overflow means the
/// ledger term left `u64`. Together with the two-term closure re-checked over
/// the post-state, this forces the kernel's aggregate supply effect to equal
/// its per-position effect: a divergence refuses rather than corrupting the
/// ledger.
fn apply_position_delta_to_ledger(
    supply: &mut SupplyLedgerAccount,
    outcome_count: u8,
    pre_internal: &[u64; MAX_OUTCOMES],
    pre_external: &[u64; MAX_OUTCOMES],
    post: &Position,
) -> Result<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        supply.internal_supply[outcome] = supply.internal_supply[outcome]
            .checked_sub(pre_internal[outcome])
            .ok_or(Error::AggregateClosureMismatch)?
            .checked_add(post.internal[outcome])
            .ok_or(Error::Arithmetic)?;
        supply.external_supply[outcome] = supply.external_supply[outcome]
            .checked_sub(pre_external[outcome])
            .ok_or(Error::AggregateClosureMismatch)?
            .checked_add(post.external[outcome])
            .ok_or(Error::Arithmetic)?;
        outcome += 1;
    }
    Ok(())
}

fn authorize(actor: ActorMetadata, expected: Hash32) -> Result<()> {
    if !actor.signer {
        return Err(Error::MissingSignature);
    }
    if actor.key != expected {
        return Err(Error::UnauthorizedActor);
    }
    Ok(())
}

fn authorize_owner(actor: ActorMetadata, owner: Hash32) -> Result<()> {
    authorize(actor, owner)
}

fn require_intent_binding(
    intent_market: Hash32,
    owner: Hash32,
    market: &MarketAccount,
    position: &PositionAccount,
) -> Result<()> {
    if intent_market != market.market || owner != position.owner {
        return Err(Error::MismatchedState);
    }
    Ok(())
}

fn encode_output(
    state: &DecodedState,
    resolution: Option<[u8; account_len::RESOLUTION]>,
    redemption_payout: u64,
) -> Result<TransitionOutput> {
    let mut output = TransitionOutput {
        market: [0; account_len::MARKET],
        hoard: [0; account_len::HOARD],
        position: [0; account_len::POSITION],
        kernel: [0; KERNEL_ACCOUNT_LEN],
        external: [0; EXTERNAL_ACCOUNT_LEN],
        replay: [0; REPLAY_ACCOUNT_LEN],
        supply: [0; account_len::SUPPLY_LEDGER],
        resolution,
        redemption_payout,
    };
    state.market.encode(&mut output.market)?;
    state.hoard.encode(&mut output.hoard)?;
    state.position.encode(&mut output.position)?;
    state.kernel.encode(&mut output.kernel)?;
    state.external.encode(&mut output.external)?;
    state.replay.encode(&mut output.replay)?;
    state.supply.encode(&mut output.supply)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use clutch_accumulator::{
        COVERAGE_POLICY_BOUNDED_GAPS, COVERAGE_POLICY_COMPLETE_REQUIRED, MAX_VALUE,
    };
    use clutch_solana_layout::{
        canonical_outcome_id, canonical_profile_hash, canonical_realm_id, FeedId,
        PayoutVectorBytes, MAX_KNOTS, PAYOUT_INDEX_UNRESOLVED, PROFILE_PARENT_BYTES,
        UNIFORM_SPACING_NONE,
    };

    const RESOLVED_SLOT: u64 = 4_242;
    const FEED_CURSOR: u64 = 104;
    const START_BUCKET: u64 = 100;
    const END_BUCKET: u64 = 103;
    const MATURITY_HORIZON: u64 = 4;
    const GRID_FAMILY: u32 = 7;
    const GRID_VERSION: u16 = 1;
    const BUCKET_SECONDS: u64 = 60;

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    /// The 64-byte parent Profile preimage of RESOLUTION_EVIDENCE_PLAN 3.2.
    fn parent_profile_bytes(child_digest: Hash32) -> [u8; PROFILE_PARENT_BYTES] {
        let mut parent = [0; PROFILE_PARENT_BYTES];
        parent[..8].copy_from_slice(b"DCPROF1\0");
        parent[8..10].copy_from_slice(&1_u16.to_le_bytes());
        parent[12..14].copy_from_slice(&1_u16.to_le_bytes());
        parent[14..16].copy_from_slice(&1_u16.to_le_bytes());
        parent[16..48].copy_from_slice(&child_digest.bytes());
        parent
    }

    /// The Realm's frozen collateral policy: a real, decodable 266-byte
    /// policy whose recomputed child digest the fixture Profile freezes, so
    /// every market-init call site binds an actual policy rather than a
    /// placeholder digest (RESOLUTION_EVIDENCE_PLAN §3.5).
    fn fixture_policy() -> collateral::CollateralPolicy {
        let backing = collateral::CurrencyRef::spl(collateral::TOKEN_2022_PROGRAM, [0x9d; 32], 9);
        collateral::CollateralPolicy {
            schema_version: collateral::COLLATERAL_POLICY_SCHEMA,
            flags: collateral::COLLATERAL_POLICY_STRICT_FLAGS,
            collateral: backing,
            fee: backing,
            liveness: collateral::CurrencyRef::NATIVE_SOL,
            max_supply_atoms: 1_000_000_000,
            allowed_mint_extensions: 0,
            required_mint_extensions: 0,
            allowed_account_extensions: collateral::EXTENSION_IMMUTABLE_OWNER,
            required_account_extensions: 0,
        }
    }

    fn payout_set() -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut left = [0; MAX_OUTCOMES];
        left[0] = 1;
        vectors[0] = PayoutVector::new(1, left);
        let mut right = [0; MAX_OUTCOMES];
        right[1] = 1;
        vectors[1] = PayoutVector::new(1, right);
        PayoutSet::new(2, 2, vectors)
    }

    fn frozen_terms(realm: Hash32, profile: Hash32, feed: FeedId) -> TermsAccount {
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut left = [0; MAX_OUTCOMES];
        left[0] = 1;
        payouts[0] = PayoutVectorBytes {
            denominator: 1,
            weights: left,
        };
        let mut right = [0; MAX_OUTCOMES];
        right[1] = 1;
        payouts[1] = PayoutVectorBytes {
            denominator: 1,
            weights: right,
        };
        /* The degree-0 boundary table equivalent to V1's pinned ordinal
         * partition for two outcomes: one interior boundary at 1, cells
         * [0, 1) and [1, MAX], identity payout map.  The v3 source identity
         * keeps the v2 "feed doubles as both" shape by storing the feed as
         * the source-adapter id, so every window fixture is unchanged. */
        let mut knots = [0u128; MAX_KNOTS];
        knots[0] = 1;
        let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        payout_map[0] = 0;
        payout_map[1] = 1;
        let mut terms = TermsAccount {
            terms: Hash32::ZERO,
            realm,
            profile,
            feed,
            price_grid: h(12),
            outcome_count: 2,
            payout_count: 2,
            payouts,
            grid_family_id: GRID_FAMILY,
            grid_version: GRID_VERSION,
            bucket_seconds: BUCKET_SECONDS,
            expected_start_bucket: START_BUCKET,
            expected_end_bucket_exclusive: END_BUCKET,
            maturity_horizon_buckets: MATURITY_HORIZON,
            coverage_policy_id: u32::from(COVERAGE_POLICY_COMPLETE_REQUIRED),
            repair_policy_id: u32::from(GEN_EXACT_01),
            failure_policy_id: u32::from(FAIL_UNIFORM_REFUND_01),
            statistic_id: STAT_TERMINAL_01,
            ambiguity_policy_id: AMBIG_REFUSE_01,
            edge_policy_id: EDGE_CLAMP_01,
            basis_degree: 0,
            knot_count: 1,
            uniform_log2_spacing: UNIFORM_SPACING_NONE,
            failure_payout_index: 0,
            coverage_policy_parameter: 0,
            repair_generation: V1_EXACT_GENERATION,
            source_version: V1_SOURCE_VERSION,
            evaluator_version: V1_EVALUATOR_VERSION,
            source_adapter_id: feed,
            payout_map,
            knots,
            collateral_cap: 1_000,
            stored_bump: 8,
            flags: 0,
        };
        terms.terms = terms.recomputed_terms_digest().expect("terms body");
        terms
    }

    /// The declared window domain of one evidence blob, mutable field by field
    /// so an adversarial test can name exactly which one it corrupted.
    #[derive(Clone, Copy, Debug)]
    struct WindowSpec {
        source_adapter_id: [u8; IDENTITY_BYTES],
        feed_spec_id: [u8; IDENTITY_BYTES],
        source_version: u32,
        evaluator_version: u32,
        grid_family_id: u32,
        grid_version: u16,
        bucket_seconds: u64,
        start_bucket: u64,
        end_bucket_exclusive: u64,
        maturity_bucket_exclusive: u64,
        generation: u64,
        coverage_policy_id: u16,
        coverage_policy_parameter: u64,
    }

    impl WindowSpec {
        fn expected(feed: FeedId) -> Self {
            Self {
                source_adapter_id: feed.bytes(),
                feed_spec_id: feed.bytes(),
                source_version: V1_SOURCE_VERSION,
                evaluator_version: V1_EVALUATOR_VERSION,
                grid_family_id: GRID_FAMILY,
                grid_version: GRID_VERSION,
                bucket_seconds: BUCKET_SECONDS,
                start_bucket: START_BUCKET,
                end_bucket_exclusive: END_BUCKET,
                maturity_bucket_exclusive: START_BUCKET + MATURITY_HORIZON,
                generation: V1_EXACT_GENERATION,
                coverage_policy_id: COVERAGE_POLICY_COMPLETE_REQUIRED,
                coverage_policy_parameter: 0,
            }
        }
    }

    fn encode_window(
        spec: &WindowSpec,
        records: &[(u8, u64, u128, u128)],
    ) -> ([u8; MAX_WINDOW_EVIDENCE_LEN], usize) {
        let mut out = [0; MAX_WINDOW_EVIDENCE_LEN];
        out[0] = WINDOW_EVIDENCE_TAG;
        out[1] = REFERENCE_VERSION;
        let mut at = 2;
        let mut put = |bytes: &[u8], at: &mut usize| {
            out[*at..*at + bytes.len()].copy_from_slice(bytes);
            *at += bytes.len();
        };
        put(&spec.source_adapter_id, &mut at);
        put(&spec.feed_spec_id, &mut at);
        put(&spec.source_version.to_le_bytes(), &mut at);
        put(&spec.evaluator_version.to_le_bytes(), &mut at);
        put(&spec.grid_family_id.to_le_bytes(), &mut at);
        put(&spec.grid_version.to_le_bytes(), &mut at);
        put(&spec.bucket_seconds.to_le_bytes(), &mut at);
        put(&spec.start_bucket.to_le_bytes(), &mut at);
        put(&spec.end_bucket_exclusive.to_le_bytes(), &mut at);
        put(&spec.maturity_bucket_exclusive.to_le_bytes(), &mut at);
        put(&spec.generation.to_le_bytes(), &mut at);
        put(&spec.coverage_policy_id.to_le_bytes(), &mut at);
        put(&spec.coverage_policy_parameter.to_le_bytes(), &mut at);
        put(&(records.len() as u16).to_le_bytes(), &mut at);
        assert_eq!(at, WINDOW_EVIDENCE_HEADER_BYTES);
        for (kind, bucket, low, high) in records {
            put(&[*kind], &mut at);
            put(&bucket.to_le_bytes(), &mut at);
            put(&low.to_le_bytes(), &mut at);
            put(&high.to_le_bytes(), &mut at);
        }
        (out, at)
    }

    /// The complete, mature, one-hot-selecting observation page: buckets 100
    /// and 101 sit in cell 0 and bucket 102 terminates in cell 1.
    fn winning_records() -> [(u8, u64, u128, u128); 3] {
        [
            (OBSERVATION_ACCEPTED, 100, 0, 0),
            (OBSERVATION_ACCEPTED, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 102, 1, 1),
        ]
    }

    struct Fixture {
        state: TransitionOutput,
        metadata: TransitionMetadata,
        bindings: ExpectedBindings,
        evidence_metadata: EvidenceMetadata,
        evidence_bindings: EvidenceBindings,
        realm: [u8; account_len::REALM],
        profile: [u8; account_len::PROFILE],
        policy: [u8; collateral::COLLATERAL_POLICY_BYTES],
        terms: [u8; account_len::TERMS],
        terms_account: TermsAccount,
        resolution: [u8; account_len::RESOLUTION],
        create: [u8; 139],
    }

    impl Fixture {
        fn window_spec(&self) -> WindowSpec {
            WindowSpec::expected(self.terms_account.feed)
        }
    }

    fn fixture() -> Fixture {
        let policy = fixture_policy();
        let policy_bytes = policy.canonical_bytes().expect("fixture policy encodes");
        let policy_digest = policy.digest().expect("fixture policy digests");
        let profile_hash = canonical_profile_hash(&parent_profile_bytes(policy_digest))
            .expect("exact parent preimage");
        let realm_hash = canonical_realm_id(profile_hash, 7);
        let market_id = canonical_market_id(realm_hash, profile_hash, 9);
        let owner = h(31);
        let feed = FeedId::from_bytes([9; 32]);
        let terms_account = frozen_terms(realm_hash, profile_hash, feed);
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        outcomes[0] = canonical_outcome_id(market_id, 0);
        outcomes[1] = canonical_outcome_id(market_id, 1);
        let market = MarketAccount {
            market: market_id,
            realm: realm_hash,
            profile: profile_hash,
            terms: terms_account.terms,
            outcome_count: 2,
            lifecycle: 0,
            stored_bump: 3,
            hoard_bump: 4,
            outcomes,
            feed,
            collateral_cap: 1_000,
            created_slot: 55,
            reserved: Hash32::ZERO,
        };
        let hoard = HoardAccount {
            market: market_id,
            realm: realm_hash,
            authority: h(10),
            collateral_atoms: 0,
            stored_bump: 4,
            flags: 0,
        };
        let position = PositionAccount {
            market: market_id,
            owner,
            generation: 2,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 100,
            reserved_cash_atoms: 7,
            stored_bump: 5,
            close_state: 0,
        };
        let kernel = KernelAccount {
            market: market_id,
            phase: 0,
            basis_mode: BasisMode::FinitePreset,
            resolved_payout: 0,
            payouts: payout_set(),
            total_supply: [0; MAX_OUTCOMES],
        };
        let external = ExternalAccount {
            market: market_id,
            owner,
            position_generation: 2,
            balances: [0; MAX_OUTCOMES],
            stored_bump: 6,
            flags: 0,
        };
        let replay = ReplayAccount {
            market: market_id,
            owner,
            position_generation: 2,
            sequence: 0,
            stored_bump: 7,
            flags: 0,
        };
        let supply = SupplyLedgerAccount {
            market: market_id,
            realm: realm_hash,
            generation: 2,
            outcome_count: 2,
            internal_supply: [0; MAX_OUTCOMES],
            external_supply: [0; MAX_OUTCOMES],
            stored_bump: 10,
            flags: 0,
        };
        let resolution = ResolutionAccount {
            market: market_id,
            terms: terms_account.terms,
            feed,
            window: Hash32::ZERO,
            feed_cursor: 0,
            sealed_end_bucket_exclusive: 0,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            stored_bump: 9,
            flags: 0,
        };
        let mut state = TransitionOutput {
            market: [0; account_len::MARKET],
            hoard: [0; account_len::HOARD],
            position: [0; account_len::POSITION],
            kernel: [0; KERNEL_ACCOUNT_LEN],
            external: [0; EXTERNAL_ACCOUNT_LEN],
            replay: [0; REPLAY_ACCOUNT_LEN],
            supply: [0; account_len::SUPPLY_LEDGER],
            resolution: None,
            redemption_payout: 0,
        };
        market.encode(&mut state.market).unwrap();
        hoard.encode(&mut state.hoard).unwrap();
        position.encode(&mut state.position).unwrap();
        kernel.encode(&mut state.kernel).unwrap();
        external.encode(&mut state.external).unwrap();
        replay.encode(&mut state.replay).unwrap();
        supply.encode(&mut state.supply).unwrap();
        let mut terms_bytes = [0; account_len::TERMS];
        terms_account.encode(&mut terms_bytes).unwrap();
        let mut resolution_bytes = [0; account_len::RESOLUTION];
        resolution.encode(&mut resolution_bytes).unwrap();
        let program = h(50);
        let keys = [h(51), h(52), h(53), h(54), h(55), h(56), h(57)];
        let am = |key| AccountMetadata {
            key,
            owner_program: program,
            writable: true,
        };
        let metadata = TransitionMetadata {
            market: am(keys[0]),
            hoard: am(keys[1]),
            position: am(keys[2]),
            kernel: am(keys[3]),
            external: am(keys[4]),
            replay: am(keys[5]),
            supply: am(keys[6]),
            actor: ActorMetadata {
                key: owner,
                signer: true,
            },
        };
        let bindings = ExpectedBindings {
            program_id: program,
            market: keys[0],
            hoard: keys[1],
            position: keys[2],
            kernel: keys[3],
            external: keys[4],
            replay: keys[5],
            supply: keys[6],
            market_bump: 3,
            hoard_bump: 4,
            position_bump: 5,
            external_bump: 6,
            replay_bump: 7,
            supply_bump: 10,
        };
        let evidence_metadata = EvidenceMetadata {
            terms: AccountMetadata {
                key: h(58),
                owner_program: program,
                writable: false,
            },
            resolution: AccountMetadata {
                key: h(59),
                owner_program: program,
                writable: true,
            },
        };
        let evidence_bindings = EvidenceBindings {
            terms: h(58),
            resolution: h(59),
            terms_bump: 8,
            resolution_bump: 9,
            window_id: h(77),
        };
        let realm = RealmAccount {
            realm: realm_hash,
            profile: profile_hash,
            max_outcomes: 16,
            profile_version: 2,
            stored_bump: 2,
            flags: 0,
        };
        let profile = ProfileAccount {
            profile: profile_hash,
            realm: realm_hash,
            version: 2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
            collateral_policy_digest: policy_digest,
        };
        let mut realm_bytes = [0; account_len::REALM];
        let mut profile_bytes = [0; account_len::PROFILE];
        realm.encode(&mut realm_bytes).unwrap();
        profile.encode(&mut profile_bytes).unwrap();
        let create_intent = Intent::CreateMarket {
            realm: realm_hash,
            profile: profile_hash,
            market_nonce: 9,
            outcome_count: 2,
            terms: terms_account.terms,
            feed,
        };
        let mut create = [0; 139];
        assert_eq!(create_intent.encode(&mut create), Ok(139));
        Fixture {
            state,
            metadata,
            bindings,
            evidence_metadata,
            evidence_bindings,
            realm: realm_bytes,
            profile: profile_bytes,
            policy: policy_bytes,
            terms: terms_bytes,
            terms_account,
            resolution: resolution_bytes,
            create,
        }
    }

    fn state_bytes(state: &TransitionOutput) -> StateBytes<'_> {
        StateBytes {
            market: &state.market,
            hoard: &state.hoard,
            position: &state.position,
            kernel: &state.kernel,
            external: &state.external,
            replay: &state.replay,
            supply: &state.supply,
        }
    }

    fn resolution_state_bytes(state: &TransitionOutput) -> ResolutionStateBytes<'_> {
        ResolutionStateBytes {
            market: &state.market,
            hoard: &state.hoard,
            kernel: &state.kernel,
            supply: &state.supply,
        }
    }

    fn resolution_metadata(f: &Fixture) -> ResolutionTransitionMetadata {
        let mut hoard = f.metadata.hoard;
        hoard.writable = false;
        ResolutionTransitionMetadata {
            market: f.metadata.market,
            hoard,
            kernel: f.metadata.kernel,
            supply: f.metadata.supply,
            actor: f.metadata.actor,
        }
    }

    fn resolution_bindings(f: &Fixture) -> ResolutionExpectedBindings {
        ResolutionExpectedBindings {
            program_id: f.bindings.program_id,
            market: f.bindings.market,
            hoard: f.bindings.hoard,
            kernel: f.bindings.kernel,
            supply: f.bindings.supply,
            market_bump: f.bindings.market_bump,
            hoard_bump: f.bindings.hoard_bump,
            supply_bump: f.bindings.supply_bump,
        }
    }

    fn clear_init_cash(state: &mut TransitionOutput) {
        let mut position = PositionAccount::decode(&state.position).unwrap();
        position.cash_atoms = 0;
        position.reserved_cash_atoms = 0;
        position.encode(&mut state.position).unwrap();
    }

    fn layout_request(sequence: u64, intent: Intent) -> [u8; MAX_REQUEST_LEN] {
        let mut intent_bytes = [0; clutch_solana_layout::MAX_INTENT_BYTES];
        let len = intent.encode(&mut intent_bytes).unwrap();
        let mut out = [0; MAX_REQUEST_LEN];
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&sequence.to_le_bytes());
        out[10] = ACTION_LAYOUT;
        out[11..13].copy_from_slice(&(len as u16).to_le_bytes());
        out[13..13 + len].copy_from_slice(&intent_bytes[..len]);
        out
    }

    fn layout_request_len(request: &[u8; MAX_REQUEST_LEN]) -> usize {
        13 + usize::from(u16::from_le_bytes([request[11], request[12]]))
    }

    fn direct_v3_intents() -> [DirectV3Intent; 11] {
        let rewards = clutch_solana_layout::direct_selection_v3::DirectKeeperRewardsV3 {
            begin_verification: 1,
            verify_candidate: 2,
            finalize_selection: 3,
            settle: 4,
            lapse: 5,
        };
        [
            DirectV3Intent::InitEpoch {
                market: h(1),
                epoch_index: 7,
                policy: h(2),
                submission_opens_slot: 10,
                submission_closes_slot: 20,
                selection_deadline_slot: 25,
                settlement_deadline_slot: 27,
                neutral_lamport_sink: h(3),
            },
            DirectV3Intent::FreezeEpoch {
                market: h(1),
                epoch: h(4),
                reward_deposit: 18,
                rewards,
            },
            DirectV3Intent::AbortUnfrozen {
                market: h(1),
                epoch: h(4),
            },
            DirectV3Intent::SubmitCandidate {
                market: h(1),
                epoch: h(4),
                outcome_price: 5,
            },
            DirectV3Intent::BeginVerification {
                market: h(1),
                epoch: h(4),
            },
            DirectV3Intent::VerifyCandidate {
                market: h(1),
                epoch: h(4),
                retained_index: 2,
            },
            DirectV3Intent::FinalizeSelection {
                market: h(1),
                epoch: h(4),
            },
            DirectV3Intent::Settle {
                market: h(1),
                epoch: h(4),
            },
            DirectV3Intent::LapseEmpty {
                market: h(1),
                epoch: h(4),
            },
            DirectV3Intent::LapseUnselected {
                market: h(1),
                epoch: h(4),
            },
            DirectV3Intent::LapseSelected {
                market: h(1),
                epoch: h(4),
            },
        ]
    }

    #[test]
    fn dedicated_direct_v3_request_envelope_is_exact_and_legacy_refuses() {
        for (sequence, intent) in direct_v3_intents().into_iter().enumerate() {
            let request = DirectV3Request {
                sequence: sequence as u64,
                intent,
            };
            let mut bytes = [0xa5; MAX_REQUEST_LEN];
            let written = request.encode(&mut bytes).unwrap();
            assert_eq!(written, 13 + intent.encoded_len());
            assert_eq!(DirectV3Request::decode(&bytes[..written]), Ok(request));
            assert_eq!(
                Request::decode(&bytes[..written]),
                Err(Error::Layout(CodecError::WrongTag))
            );
            assert_eq!(
                DirectV3Request::decode(&bytes[..written - 1]),
                Err(Error::WrongLength)
            );
            assert_eq!(
                DirectV3Request::decode(&bytes[..written + 1]),
                Err(Error::WrongLength)
            );

            let mut hostile = bytes;
            hostile[0] ^= 1;
            assert_eq!(
                DirectV3Request::decode(&hostile[..written]),
                Err(Error::WrongTag)
            );
            let mut hostile = bytes;
            hostile[1] ^= 1;
            assert_eq!(
                DirectV3Request::decode(&hostile[..written]),
                Err(Error::WrongVersion)
            );
            let mut hostile = bytes;
            hostile[10] = ACTION_RESOLVE;
            assert_eq!(
                DirectV3Request::decode(&hostile[..written]),
                Err(Error::WrongTag)
            );
            let mut hostile = bytes;
            hostile[11..13].copy_from_slice(&0_u16.to_le_bytes());
            assert_eq!(
                DirectV3Request::decode(&hostile[..written]),
                Err(Error::WrongLength)
            );
        }
    }

    fn resolve_request(sequence: u64, payout: u8) -> [u8; 12] {
        let mut out = [0; 12];
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&sequence.to_le_bytes());
        out[10] = ACTION_RESOLVE;
        out[11] = payout;
        out
    }

    fn redeem_request(sequence: u64, outcome: u8, quantity: u64) -> [u8; 20] {
        let mut out = [0; 20];
        out[0] = REQUEST_TAG;
        out[1] = REFERENCE_VERSION;
        out[2..10].copy_from_slice(&sequence.to_le_bytes());
        out[10] = ACTION_REDEEM_INTERNAL;
        out[11] = outcome;
        out[12..20].copy_from_slice(&quantity.to_le_bytes());
        out
    }

    /// Split `quantity` complete sets out of the fixture's opening position.
    fn split_state(f: &Fixture, quantity: u64) -> TransitionOutput {
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let request = layout_request(
            0,
            Intent::Split {
                market,
                owner,
                quantity,
            },
        );
        apply(
            &request[..layout_request_len(&request)],
            state_bytes(&f.state),
            &f.metadata,
            &f.bindings,
        )
        .unwrap()
    }

    /// Recombine `quantity` complete sets back into collateral, from a state
    /// an earlier transition already produced.
    fn merge_from(
        f: &Fixture,
        state: &TransitionOutput,
        sequence: u64,
        quantity: u64,
    ) -> Result<TransitionOutput> {
        let market = MarketAccount::decode(&state.market).unwrap().market;
        let owner = PositionAccount::decode(&state.position).unwrap().owner;
        apply_layout(
            state,
            &f.metadata,
            &f.bindings,
            sequence,
            Intent::Merge {
                market,
                owner,
                quantity,
            },
        )
    }

    /// A second owner's position/external/replay triple joined to the
    /// fixture's market-wide accounts, with its own keys, bumps, owner, and a
    /// generation (5) that deliberately differs from the supply ledger's era
    /// (2), pinning the CLO-DELTA-V1 decoupling.
    fn second_triple(
        f: &Fixture,
        cash_atoms: u64,
    ) -> (TransitionOutput, TransitionMetadata, ExpectedBindings) {
        let market_id = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = h(32);
        let position = PositionAccount {
            market: market_id,
            owner,
            generation: 5,
            internal: [0; MAX_OUTCOMES],
            cash_atoms,
            reserved_cash_atoms: 0,
            stored_bump: 11,
            close_state: 0,
        };
        let external = ExternalAccount {
            market: market_id,
            owner,
            position_generation: 5,
            balances: [0; MAX_OUTCOMES],
            stored_bump: 12,
            flags: 0,
        };
        let replay = ReplayAccount {
            market: market_id,
            owner,
            position_generation: 5,
            sequence: 0,
            stored_bump: 13,
            flags: 0,
        };
        let mut state = f.state.clone();
        position.encode(&mut state.position).unwrap();
        external.encode(&mut state.external).unwrap();
        replay.encode(&mut state.replay).unwrap();
        let mut metadata = f.metadata;
        metadata.position.key = h(61);
        metadata.external.key = h(62);
        metadata.replay.key = h(63);
        metadata.actor = ActorMetadata {
            key: owner,
            signer: true,
        };
        let mut bindings = f.bindings;
        bindings.position = h(61);
        bindings.external = h(62);
        bindings.replay = h(63);
        bindings.position_bump = 11;
        bindings.external_bump = 12;
        bindings.replay_bump = 13;
        (state, metadata, bindings)
    }

    /// Copy the market-wide accounts (market, hoard, kernel, supply ledger)
    /// from one owner's transition output into another owner's working state.
    fn sync_shared(from: &TransitionOutput, to: &mut TransitionOutput) {
        to.market = from.market;
        to.hoard = from.hoard;
        to.kernel = from.kernel;
        to.supply = from.supply;
    }

    fn apply_layout(
        state: &TransitionOutput,
        metadata: &TransitionMetadata,
        bindings: &ExpectedBindings,
        sequence: u64,
        intent: Intent,
    ) -> Result<TransitionOutput> {
        let request = layout_request(sequence, intent);
        apply(
            &request[..layout_request_len(&request)],
            state_bytes(state),
            metadata,
            bindings,
        )
    }

    /// The test-side scan the adapter never performs: the ledger terms must
    /// equal the componentwise sums over the known triples, and the two-term
    /// aggregate must equal the kernel supply.
    fn assert_ledger_is_position_sum(shared: &TransitionOutput, triples: &[&TransitionOutput]) {
        let ledger = SupplyLedgerAccount::decode(&shared.supply).unwrap();
        let kernel = KernelAccount::decode(&shared.kernel).unwrap();
        let mut outcome = 0_usize;
        while outcome < 2 {
            let mut internal = 0_u64;
            let mut external = 0_u64;
            for triple in triples {
                internal += PositionAccount::decode(&triple.position).unwrap().internal[outcome];
                external += ExternalAccount::decode(&triple.external).unwrap().balances[outcome];
            }
            assert_eq!(ledger.internal_supply[outcome], internal);
            assert_eq!(ledger.external_supply[outcome], external);
            assert_eq!(
                ledger.aggregate_supply(outcome as u8),
                Ok(kernel.total_supply[outcome])
            );
            outcome += 1;
        }
    }

    #[test]
    fn initialized_market_validation_runs_kernel_invariants() {
        let mut f = fixture();
        clear_init_cash(&mut f.state);
        assert_eq!(
            validate_market_init(
                &f.realm,
                &f.profile,
                &f.policy,
                &f.terms,
                state_bytes(&f.state),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Ok(())
        );
    }

    #[test]
    fn kernel_v2_persists_mode_and_refuses_hostile_or_legacy_mode_bytes() {
        let f = fixture();
        let mut kernel = KernelAccount::decode(&f.state.kernel).unwrap();
        kernel.basis_mode = BasisMode::DerivedBasis;
        let mut encoded = [0_u8; KERNEL_ACCOUNT_LEN];
        kernel.encode(&mut encoded).unwrap();
        assert_eq!(
            KernelAccount::decode(&encoded).unwrap().basis_mode,
            BasisMode::DerivedBasis
        );

        let mut hostile = encoded;
        hostile[35] = 2;
        assert_eq!(KernelAccount::decode(&hostile), Err(Error::NonCanonical));

        let mut legacy_version = encoded;
        legacy_version[1] = REFERENCE_VERSION;
        assert_eq!(
            KernelAccount::decode(&legacy_version),
            Err(Error::WrongVersion)
        );
    }

    #[test]
    fn market_initialization_refuses_a_hostile_mode_flip_exactly() {
        let mut f = fixture();
        clear_init_cash(&mut f.state);
        let mut kernel = KernelAccount::decode(&f.state.kernel).unwrap();
        kernel.basis_mode = BasisMode::DerivedBasis;
        kernel.encode(&mut f.state.kernel).unwrap();
        assert_eq!(
            validate_market_init(
                &f.realm,
                &f.profile,
                &f.policy,
                &f.terms,
                state_bytes(&f.state),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Kernel(KernelError::WrongResolutionMode))
        );
    }

    #[test]
    fn active_derived_mode_split_is_solvent_and_preserves_mode() {
        let mut f = fixture();
        let mut kernel = KernelAccount::decode(&f.state.kernel).unwrap();
        kernel.basis_mode = BasisMode::DerivedBasis;
        kernel.encode(&mut f.state.kernel).unwrap();
        let market = MarketAccount::decode(&f.state.market).unwrap();
        let position = PositionAccount::decode(&f.state.position).unwrap();
        let split = apply_layout(
            &f.state,
            &f.metadata,
            &f.bindings,
            0,
            Intent::Split {
                market: market.market,
                owner: position.owner,
                quantity: 6,
            },
        )
        .unwrap();
        let after = KernelAccount::decode(&split.kernel).unwrap();
        assert_eq!(after.basis_mode, BasisMode::DerivedBasis);
        assert_eq!(after.total_supply[0], 6);
        assert_eq!(after.total_supply[1], 6);
        assert_eq!(
            HoardAccount::decode(&split.hoard).unwrap().collateral_atoms,
            6
        );
    }

    #[test]
    fn initialized_market_refuses_preexisting_position_claims() {
        let mut f = fixture();
        clear_init_cash(&mut f.state);
        let mut hoard = HoardAccount::decode(&f.state.hoard).unwrap();
        hoard.collateral_atoms = 1;
        hoard.encode(&mut f.state.hoard).unwrap();
        let mut position = PositionAccount::decode(&f.state.position).unwrap();
        position.internal[0] = 1;
        position.internal[1] = 1;
        position.encode(&mut f.state.position).unwrap();
        let mut kernel = KernelAccount::decode(&f.state.kernel).unwrap();
        kernel.total_supply[0] = 1;
        kernel.total_supply[1] = 1;
        kernel.encode(&mut f.state.kernel).unwrap();
        let mut supply = SupplyLedgerAccount::decode(&f.state.supply).unwrap();
        supply.internal_supply[0] = 1;
        supply.internal_supply[1] = 1;
        supply.encode(&mut f.state.supply).unwrap();
        assert_eq!(
            validate_market_init(
                &f.realm,
                &f.profile,
                &f.policy,
                &f.terms,
                state_bytes(&f.state),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::NonEmptyInitialization)
        );
    }

    #[test]
    fn unfrozen_collateral_policy_refuses_market_initialization() {
        let mut f = fixture();
        clear_init_cash(&mut f.state);
        let mut profile = ProfileAccount::decode(&f.profile).unwrap();
        profile.flags = 0;
        profile.collateral_policy_digest = Hash32::ZERO;
        let mut unfrozen = [0; account_len::PROFILE];
        profile.encode(&mut unfrozen).unwrap();
        assert_eq!(
            validate_market_init(
                &f.realm,
                &unfrozen,
                &f.policy,
                &f.terms,
                state_bytes(&f.state),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::CollateralPolicyNotFrozen)
        );
    }

    #[test]
    fn forged_position_cannot_materialize_claims_absent_from_aggregate() {
        let mut f = fixture();
        let mut position = PositionAccount::decode(&f.state.position).unwrap();
        position.internal[0] = 1;
        position.encode(&mut f.state.position).unwrap();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let request = layout_request(
            0,
            Intent::Materialize {
                market,
                owner: position.owner,
                destination: f.metadata.external.key,
                outcome: 0,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::AggregateClosureMismatch)
        );
        assert_eq!(
            KernelAccount::decode(&f.state.kernel).unwrap().total_supply[0],
            0
        );
        assert_eq!(
            ExternalAccount::decode(&f.state.external).unwrap().balances[0],
            0
        );
        assert_eq!(
            SupplyLedgerAccount::decode(&f.state.supply)
                .unwrap()
                .internal_supply[0],
            0
        );
    }

    #[test]
    fn split_has_exact_full_account_pre_and_post_vectors() {
        let f = fixture();
        let output = split_state(&f, 11);

        let expected_market = f.state.market;
        let mut expected_hoard = f.state.hoard;
        expected_hoard[98..106].copy_from_slice(&11_u64.to_le_bytes());
        let mut expected_position = f.state.position;
        expected_position[74..82].copy_from_slice(&11_u64.to_le_bytes());
        expected_position[82..90].copy_from_slice(&11_u64.to_le_bytes());
        expected_position[202..210].copy_from_slice(&89_u64.to_le_bytes());
        let mut expected_kernel = f.state.kernel;
        expected_kernel[39..47].copy_from_slice(&11_u64.to_le_bytes());
        expected_kernel[47..55].copy_from_slice(&11_u64.to_le_bytes());
        let expected_external = f.state.external;
        let mut expected_replay = f.state.replay;
        expected_replay[74..82].copy_from_slice(&1_u64.to_le_bytes());
        let mut expected_supply = f.state.supply;
        expected_supply[75..83].copy_from_slice(&11_u64.to_le_bytes());
        expected_supply[83..91].copy_from_slice(&11_u64.to_le_bytes());

        assert_eq!(output.market, expected_market);
        assert_eq!(output.hoard, expected_hoard);
        assert_eq!(output.position, expected_position);
        assert_eq!(output.kernel, expected_kernel);
        assert_eq!(output.external, expected_external);
        assert_eq!(output.replay, expected_replay);
        assert_eq!(output.supply, expected_supply);
        assert_eq!(output.resolution, None);
        assert_eq!(output.redemption_payout, 0);
    }

    #[test]
    fn merge_has_exact_full_account_pre_and_post_vectors() {
        /* The named little-endian field deltas of one merge, read against the
         * split-11 state rather than the fixture, so every number below is the
         * inverse of the vector directly above: collateral 11 -> 7, both
         * internal balances 11 -> 7, cash 89 -> 93 (credited, not debited),
         * both kernel aggregates 11 -> 7, both ledger internal terms 11 -> 7,
         * sequence 1 -> 2, and the market and the external shadow untouched. */
        let f = fixture();
        let split = split_state(&f, 11);
        let output = merge_from(&f, &split, 1, 4).unwrap();

        let expected_market = split.market;
        let mut expected_hoard = split.hoard;
        expected_hoard[98..106].copy_from_slice(&7_u64.to_le_bytes());
        let mut expected_position = split.position;
        expected_position[74..82].copy_from_slice(&7_u64.to_le_bytes());
        expected_position[82..90].copy_from_slice(&7_u64.to_le_bytes());
        expected_position[202..210].copy_from_slice(&93_u64.to_le_bytes());
        let mut expected_kernel = split.kernel;
        expected_kernel[39..47].copy_from_slice(&7_u64.to_le_bytes());
        expected_kernel[47..55].copy_from_slice(&7_u64.to_le_bytes());
        let expected_external = split.external;
        let mut expected_replay = split.replay;
        expected_replay[74..82].copy_from_slice(&2_u64.to_le_bytes());
        let mut expected_supply = split.supply;
        expected_supply[75..83].copy_from_slice(&7_u64.to_le_bytes());
        expected_supply[83..91].copy_from_slice(&7_u64.to_le_bytes());

        assert_eq!(output.market, expected_market);
        assert_eq!(output.hoard, expected_hoard);
        assert_eq!(output.position, expected_position);
        assert_eq!(output.kernel, expected_kernel);
        assert_eq!(output.external, expected_external);
        assert_eq!(output.replay, expected_replay);
        assert_eq!(output.supply, expected_supply);
        assert_eq!(output.resolution, None);
        assert_eq!(output.redemption_payout, 0);

        /* The reserved cash the position parked is untouched by both legs: a
         * merge credits `cash_atoms` and never `reserved_cash_atoms`. */
        let position = PositionAccount::decode(&output.position).unwrap();
        assert_eq!(position.reserved_cash_atoms, 7);
        assert_eq!(position.cash_atoms, 93);
    }

    #[test]
    fn split_then_merge_returns_every_account_to_its_pre_split_bytes() {
        /* PROJECT.md's central recombination promise, at byte resolution: a
         * complete set goes back into its collateral and leaves no residue.
         * The replay sequence is the one field that must differ, because two
         * transitions were consumed and a state machine that forgot them would
         * be replayable. */
        let f = fixture();
        let split = split_state(&f, 11);
        let round_trip = merge_from(&f, &split, 1, 11).unwrap();

        assert_eq!(round_trip.market, f.state.market);
        assert_eq!(round_trip.hoard, f.state.hoard);
        assert_eq!(round_trip.position, f.state.position);
        assert_eq!(round_trip.kernel, f.state.kernel);
        assert_eq!(round_trip.external, f.state.external);
        assert_eq!(round_trip.supply, f.state.supply);
        assert_eq!(round_trip.resolution, None);
        assert_eq!(round_trip.redemption_payout, 0);

        let mut expected_replay = f.state.replay;
        expected_replay[74..82].copy_from_slice(&2_u64.to_le_bytes());
        assert_eq!(round_trip.replay, expected_replay);
        assert_ne!(round_trip.replay, f.state.replay);
        assert_eq!(
            ReplayAccount::decode(&round_trip.replay).unwrap().sequence,
            2
        );

        /* And the round trip is not an artifact of one quantity.  93 is the
         * largest one the fixture can split at all: `PositionAccount::validate`
         * refuses `reserved_cash_atoms > cash_atoms`, so the reserved 7 atoms
         * are a floor under the cash a split may spend, not merely an
         * annotation. */
        for quantity in [1_u64, 7, 93] {
            let split = split_state(&f, quantity);
            let back = merge_from(&f, &split, 1, quantity).unwrap();
            assert_eq!(back.hoard, f.state.hoard);
            assert_eq!(back.position, f.state.position);
            assert_eq!(back.kernel, f.state.kernel);
            assert_eq!(back.supply, f.state.supply);
        }
    }

    #[test]
    fn merge_refuses_insufficient_claims_a_closing_position_and_a_resolved_market() {
        let f = fixture();
        let split = split_state(&f, 11);

        /* Merging more than the market holds as collateral.  The kernel tests
         * collateral before per-outcome balances (`MarketState::merge`), which
         * is the only reason `InsufficientCollateral` is reachable at all, so
         * a single-position market over-merging reports the collateral fault. */
        assert_eq!(
            merge_from(&f, &split, 1, 12),
            Err(Error::Kernel(KernelError::InsufficientCollateral))
        );

        /* Merging more than *this position* holds, in a market whose
         * collateral covers the request because a second position funded it.
         * This is the counterfeit direction a single-position market cannot
         * express — there, `collateral >= quantity` already implies the
         * balances — and it reports the balance fault instead. */
        let market_id = MarketAccount::decode(&f.state.market).unwrap().market;
        let (second_state, second_metadata, second_bindings) = second_triple(&f, 100);
        let mut second = second_state;
        sync_shared(&split, &mut second);
        let second_owner = PositionAccount::decode(&second.position).unwrap().owner;
        let second = apply_layout(
            &second,
            &second_metadata,
            &second_bindings,
            0,
            Intent::Split {
                market: market_id,
                owner: second_owner,
                quantity: 30,
            },
        )
        .unwrap();
        assert_eq!(
            HoardAccount::decode(&second.hoard)
                .unwrap()
                .collateral_atoms,
            41,
            "the hoard now covers a 31-atom merge that this position cannot back"
        );
        assert_eq!(
            apply_layout(
                &second,
                &second_metadata,
                &second_bindings,
                1,
                Intent::Merge {
                    market: market_id,
                    owner: second_owner,
                    quantity: 31,
                },
            ),
            Err(Error::Kernel(KernelError::InsufficientBalance)),
            "a position cannot merge against another position's claims"
        );

        // A closing position cannot recombine its way back into cash.
        let mut closing = split.clone();
        let mut position = PositionAccount::decode(&closing.position).unwrap();
        position.close_state = 1;
        position.encode(&mut closing.position).unwrap();
        assert_eq!(merge_from(&f, &closing, 1, 1), Err(Error::MismatchedState));

        /* A resolved market refuses too, and by two independent checks: the
         * adapter's own lifecycle discipline reports first, and
         * `MarketState::require_active` stands behind it. */
        let mut resolved = split.clone();
        let mut market = MarketAccount::decode(&resolved.market).unwrap();
        market.lifecycle = 1;
        market.encode(&mut resolved.market).unwrap();
        let mut kernel = KernelAccount::decode(&resolved.kernel).unwrap();
        kernel.phase = 1;
        kernel.encode(&mut resolved.kernel).unwrap();
        assert_eq!(merge_from(&f, &resolved, 1, 1), Err(Error::MismatchedState));

        // Every refusal above left the split state exactly as it was.
        assert_eq!(split, split_state(&f, 11));
    }

    #[test]
    fn merge_refuses_a_counterfeit_claim_and_a_tampered_ledger() {
        let f = fixture();

        /* CLO-DELTA-V1 C2 over the pre-state: a position claiming internal
         * balance the market-wide ledger does not carry cannot melt it into
         * collateral, which is the counterfeit path a merge would otherwise
         * open — `Split` mints against cash, but `Merge` pays cash out. */
        let mut forged = f.state.clone();
        let mut position = PositionAccount::decode(&forged.position).unwrap();
        position.internal[0] = 5;
        position.internal[1] = 5;
        position.encode(&mut forged.position).unwrap();
        assert_eq!(
            merge_from(&f, &forged, 0, 5),
            Err(Error::AggregateClosureMismatch)
        );

        /* C1 over the pre-state: a ledger whose two terms no longer sum to the
         * kernel aggregate refuses before the kernel is even built. */
        let split = split_state(&f, 11);
        let mut tampered = split.clone();
        let mut supply = SupplyLedgerAccount::decode(&tampered.supply).unwrap();
        supply.internal_supply[0] += 1;
        supply.encode(&mut tampered.supply).unwrap();
        assert_eq!(
            merge_from(&f, &tampered, 1, 1),
            Err(Error::AggregateClosureMismatch)
        );

        // Neither refusal moved the hoard, the aggregate, or the ledger.
        assert_eq!(
            HoardAccount::decode(&forged.hoard)
                .unwrap()
                .collateral_atoms,
            0
        );
        assert_eq!(
            SupplyLedgerAccount::decode(&f.state.supply)
                .unwrap()
                .internal_supply[0],
            0
        );
    }

    #[test]
    fn materialize_and_dematerialize_are_supply_neutral() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let split = split_state(&f, 20);
        let materialize = layout_request(
            1,
            Intent::Materialize {
                market,
                owner,
                destination: f.metadata.external.key,
                outcome: 1,
                quantity: 7,
            },
        );
        let materialized = apply(
            &materialize[..layout_request_len(&materialize)],
            state_bytes(&split),
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let kernel_before = KernelAccount::decode(&split.kernel).unwrap();
        let kernel_after = KernelAccount::decode(&materialized.kernel).unwrap();
        assert_eq!(kernel_after.total_supply, kernel_before.total_supply);
        assert_eq!(
            PositionAccount::decode(&materialized.position)
                .unwrap()
                .internal[1],
            13
        );
        assert_eq!(
            ExternalAccount::decode(&materialized.external)
                .unwrap()
                .balances[1],
            7
        );
        let ledger = SupplyLedgerAccount::decode(&materialized.supply).unwrap();
        assert_eq!(ledger.internal_supply[1], 13);
        assert_eq!(ledger.external_supply[1], 7);
        assert_eq!(ledger.aggregate_supply(1), Ok(20));

        let dematerialize = layout_request(
            2,
            Intent::Dematerialize {
                market,
                owner,
                source: f.metadata.external.key,
                outcome: 1,
                quantity: 7,
            },
        );
        let restored = apply(
            &dematerialize[..layout_request_len(&dematerialize)],
            state_bytes(&materialized),
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        assert_eq!(
            PositionAccount::decode(&restored.position)
                .unwrap()
                .internal[1],
            20
        );
        assert_eq!(
            ExternalAccount::decode(&restored.external)
                .unwrap()
                .balances[1],
            0
        );
        assert_eq!(
            SupplyLedgerAccount::decode(&restored.supply)
                .unwrap()
                .external_supply[1],
            0
        );
    }

    #[test]
    fn bounded_closed_traces_preserve_position_aggregate_equality() {
        let mut quantity = 1_u64;
        while quantity <= 16 {
            let f = fixture();
            let market = MarketAccount::decode(&f.state.market).unwrap().market;
            let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
            let split = split_state(&f, quantity);
            let moved = quantity / 2;
            let state = if moved == 0 {
                split
            } else {
                let materialize = layout_request(
                    1,
                    Intent::Materialize {
                        market,
                        owner,
                        destination: f.metadata.external.key,
                        outcome: 0,
                        quantity: moved,
                    },
                );
                apply(
                    &materialize[..layout_request_len(&materialize)],
                    state_bytes(&split),
                    &f.metadata,
                    &f.bindings,
                )
                .unwrap()
            };
            let position = PositionAccount::decode(&state.position).unwrap();
            let external = ExternalAccount::decode(&state.external).unwrap();
            let kernel = KernelAccount::decode(&state.kernel).unwrap();
            let ledger = SupplyLedgerAccount::decode(&state.supply).unwrap();
            let mut outcome = 0_usize;
            while outcome < 2 {
                assert_eq!(
                    position.internal[outcome] + external.balances[outcome],
                    kernel.total_supply[outcome]
                );
                assert_eq!(
                    ledger.aggregate_supply(outcome as u8),
                    Ok(kernel.total_supply[outcome])
                );
                assert_eq!(ledger.internal_supply[outcome], position.internal[outcome]);
                assert_eq!(ledger.external_supply[outcome], external.balances[outcome]);
                outcome += 1;
            }
            quantity += 1;
        }
    }

    #[test]
    fn multi_position_lifecycle_tracks_ledger_sums() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner_a = PositionAccount::decode(&f.state.position).unwrap().owner;
        let (b_start, metadata_b, bindings_b) = second_triple(&f, 100);
        let owner_b = PositionAccount::decode(&b_start.position).unwrap().owner;

        // A splits 20 against the shared market-wide accounts.
        let a = split_state(&f, 20);
        let mut b = b_start.clone();
        sync_shared(&a, &mut b);

        // B splits 5 against the same evolving aggregate. B's generation (5)
        // differs from the ledger era (2): the retired single-position
        // identification is gone and the triple still validates.
        let b = apply_layout(
            &b,
            &metadata_b,
            &bindings_b,
            0,
            Intent::Split {
                market,
                owner: owner_b,
                quantity: 5,
            },
        )
        .unwrap();
        let mut a = a;
        sync_shared(&b, &mut a);
        assert_ledger_is_position_sum(&b, &[&a, &b]);
        assert_eq!(HoardAccount::decode(&b.hoard).unwrap().collateral_atoms, 25);

        // A materializes 7 of outcome 1: the ledger terms move by exactly A's
        // delta while B's holdings stay represented.
        let a = apply_layout(
            &a,
            &f.metadata,
            &f.bindings,
            1,
            Intent::Materialize {
                market,
                owner: owner_a,
                destination: f.metadata.external.key,
                outcome: 1,
                quantity: 7,
            },
        )
        .unwrap();
        let mut b = b;
        sync_shared(&a, &mut b);
        assert_ledger_is_position_sum(&a, &[&a, &b]);
        let ledger = SupplyLedgerAccount::decode(&a.supply).unwrap();
        assert_eq!(ledger.internal_supply[1], 18);
        assert_eq!(ledger.external_supply[1], 7);

        // A dematerializes the 7 back; the aggregate is unchanged throughout.
        let a = apply_layout(
            &a,
            &f.metadata,
            &f.bindings,
            2,
            Intent::Dematerialize {
                market,
                owner: owner_a,
                source: f.metadata.external.key,
                outcome: 1,
                quantity: 7,
            },
        )
        .unwrap();
        sync_shared(&a, &mut b);
        assert_ledger_is_position_sum(&a, &[&a, &b]);

        // Resolve payout 1 through A's triple; sums are untouched.
        let (window, len) = encode_window(&f.window_spec(), &winning_records());
        let a = apply_with_evidence(
            &resolve_request(3, 1),
            state_bytes(&a),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &f.resolution,
                    window: &window[..len],
                },
                metadata: f.evidence_metadata,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let record = a.resolution.unwrap();
        sync_shared(&a, &mut b);
        assert_ledger_is_position_sum(&a, &[&a, &b]);

        let mut readonly = f.evidence_metadata;
        readonly.resolution.writable = false;

        // A redeems its 20 winning claims; B's 5 stay represented.
        let a = apply_with_evidence(
            &redeem_request(4, 1, 20),
            state_bytes(&a),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &record,
                    window: &[],
                },
                metadata: readonly,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        assert_eq!(a.redemption_payout, 20);
        sync_shared(&a, &mut b);
        assert_ledger_is_position_sum(&a, &[&a, &b]);
        assert_eq!(
            SupplyLedgerAccount::decode(&a.supply)
                .unwrap()
                .internal_supply[1],
            5
        );

        // B redeems its 5: the winning aggregate drains to zero exactly.
        let b = apply_with_evidence(
            &redeem_request(1, 1, 5),
            state_bytes(&b),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &record,
                    window: &[],
                },
                metadata: readonly,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &metadata_b,
            &bindings_b,
        )
        .unwrap();
        assert_eq!(b.redemption_payout, 5);
        let mut a = a;
        sync_shared(&b, &mut a);
        assert_ledger_is_position_sum(&b, &[&a, &b]);
        let ledger = SupplyLedgerAccount::decode(&b.supply).unwrap();
        assert_eq!(ledger.internal_supply[1], 0);
        assert_eq!(KernelAccount::decode(&b.kernel).unwrap().total_supply[1], 0);
        assert_eq!(HoardAccount::decode(&b.hoard).unwrap().collateral_atoms, 0);
        // The losing outcome's claims remain outstanding and represented.
        assert_eq!(ledger.internal_supply[0], 25);
    }

    #[test]
    fn position_init_forgery_refuses() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner_a = PositionAccount::decode(&f.state.position).unwrap().owner;

        // Mid-life market: A holds 20 split claims, 3 of them materialized,
        // so both ledger terms are nonzero.
        let a = split_state(&f, 20);
        let a = apply_layout(
            &a,
            &f.metadata,
            &f.bindings,
            1,
            Intent::Materialize {
                market,
                owner: owner_a,
                destination: f.metadata.external.key,
                outcome: 0,
                quantity: 3,
            },
        )
        .unwrap();
        let (mut b, metadata_b, bindings_b) = second_triple(&f, 0);
        sync_shared(&a, &mut b);

        // A provably-zero triple joins.
        assert_eq!(
            validate_position_init(state_bytes(&b), &metadata_b, &bindings_b),
            Ok(())
        );

        // Every nonzero field of the entering triple refuses, even when the
        // ledger would cover the forged claims.
        let mut internal = b.clone();
        let mut position = PositionAccount::decode(&internal.position).unwrap();
        position.internal[0] = 1;
        position.encode(&mut internal.position).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&internal), &metadata_b, &bindings_b),
            Err(Error::NonEmptyInitialization)
        );

        let mut external = b.clone();
        let mut shadow = ExternalAccount::decode(&external.external).unwrap();
        shadow.balances[0] = 1;
        shadow.encode(&mut external.external).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&external), &metadata_b, &bindings_b),
            Err(Error::NonEmptyInitialization)
        );

        let mut cash = b.clone();
        let mut position = PositionAccount::decode(&cash.position).unwrap();
        position.cash_atoms = 1;
        position.encode(&mut cash.position).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&cash), &metadata_b, &bindings_b),
            Err(Error::NonEmptyInitialization)
        );

        // Reserved cash cannot exceed total cash at the codec, so the
        // encodable reserved forgery carries both; init refuses it whole.
        let mut reserved = b.clone();
        let mut position = PositionAccount::decode(&reserved.position).unwrap();
        position.cash_atoms = 1;
        position.reserved_cash_atoms = 1;
        position.encode(&mut reserved.position).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&reserved), &metadata_b, &bindings_b),
            Err(Error::NonEmptyInitialization)
        );

        let mut sequence = b.clone();
        let mut replay = ReplayAccount::decode(&sequence.replay).unwrap();
        replay.sequence = 1;
        replay.encode(&mut sequence.replay).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&sequence), &metadata_b, &bindings_b),
            Err(Error::NonEmptyInitialization)
        );

        let mut closing = b.clone();
        let mut position = PositionAccount::decode(&closing.position).unwrap();
        position.close_state = 1;
        position.encode(&mut closing.position).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&closing), &metadata_b, &bindings_b),
            Err(Error::NonEmptyInitialization)
        );

        // Entering claims exceeding the ledger term refuse as counterfeits
        // before the emptiness check is even reached.
        let mut counterfeit = b.clone();
        let mut position = PositionAccount::decode(&counterfeit.position).unwrap();
        position.internal[0] = 18;
        position.encode(&mut counterfeit.position).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&counterfeit), &metadata_b, &bindings_b),
            Err(Error::AggregateClosureMismatch)
        );

        // A resolved market admits no new positions.
        let (window, len) = encode_window(&f.window_spec(), &winning_records());
        let resolved = apply_with_evidence(
            &resolve_request(2, 1),
            state_bytes(&a),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &f.resolution,
                    window: &window[..len],
                },
                metadata: f.evidence_metadata,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let mut late = b.clone();
        sync_shared(&resolved, &mut late);
        assert_eq!(
            validate_position_init(state_bytes(&late), &metadata_b, &bindings_b),
            Err(Error::MismatchedState)
        );
    }

    #[test]
    fn generation_replay_after_close_reopen_refuses() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let a = split_state(&f, 20);

        // Reopen: the position restarts zeroed at generation 3, but the
        // external shadow and replay accounts are still the retired
        // generation's. The triple binding refuses.
        let mut reopened = a.clone();
        let mut position = PositionAccount::decode(&reopened.position).unwrap();
        position.generation = 3;
        position.internal = [0; MAX_OUTCOMES];
        position.cash_atoms = 0;
        position.reserved_cash_atoms = 0;
        position.encode(&mut reopened.position).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&reopened), &f.metadata, &f.bindings),
            Err(Error::MismatchedState)
        );
        assert_eq!(
            apply_layout(
                &reopened,
                &f.metadata,
                &f.bindings,
                0,
                Intent::Split {
                    market,
                    owner,
                    quantity: 1,
                },
            ),
            Err(Error::MismatchedState)
        );

        // A fresh external and replay at generation 3 complete the reopen.
        // The retired position's 20 claims stay counted in the ledger — the
        // conservative over-count — and the zeroed triple still validates.
        let mut shadow = ExternalAccount::decode(&reopened.external).unwrap();
        shadow.position_generation = 3;
        shadow.balances = [0; MAX_OUTCOMES];
        shadow.encode(&mut reopened.external).unwrap();
        let mut replay = ReplayAccount::decode(&reopened.replay).unwrap();
        replay.position_generation = 3;
        replay.sequence = 0;
        replay.encode(&mut reopened.replay).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&reopened), &f.metadata, &f.bindings),
            Ok(())
        );

        // Balances surviving into the reopened triple refuse: reopen is an
        // initialization event and must re-establish the base case.
        let mut resurrected = reopened.clone();
        let mut position = PositionAccount::decode(&resurrected.position).unwrap();
        position.internal[0] = 20;
        position.encode(&mut resurrected.position).unwrap();
        assert_eq!(
            validate_position_init(state_bytes(&resurrected), &f.metadata, &f.bindings),
            Err(Error::NonEmptyInitialization)
        );

        // The retired generation's next sequence (1) does not replay against
        // the restarted triple.
        assert_eq!(
            apply_layout(
                &reopened,
                &f.metadata,
                &f.bindings,
                1,
                Intent::Split {
                    market,
                    owner,
                    quantity: 1,
                },
            ),
            Err(Error::Replay)
        );
    }

    #[test]
    fn aliased_position_keys_refuse() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let a = split_state(&f, 20);
        let (mut b, metadata_b, bindings_b) = second_triple(&f, 100);
        sync_shared(&a, &mut b);
        let owner_b = PositionAccount::decode(&b.position).unwrap().owner;
        let split_b = Intent::Split {
            market,
            owner: owner_b,
            quantity: 5,
        };

        // B's transition presented with A's position key refuses against B's
        // trusted bindings.
        let mut foreign_key = metadata_b;
        foreign_key.position.key = f.metadata.position.key;
        let request = layout_request(0, split_b);
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&b),
                &foreign_key,
                &bindings_b,
            ),
            Err(Error::WrongAccountKey)
        );

        // A's position bytes presented inside B's triple. Verbatim, A's
        // stored bump already refuses against B's derivation; with the bump
        // forged to match, the owner binding across position, external
        // shadow, and replay is what refuses.
        let mut cross = b.clone();
        cross.position = a.position;
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&cross),
                &metadata_b,
                &bindings_b,
            ),
            Err(Error::WrongBump)
        );
        let mut rebumped = PositionAccount::decode(&a.position).unwrap();
        rebumped.stored_bump = bindings_b.position_bump;
        rebumped.encode(&mut cross.position).unwrap();
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&cross),
                &metadata_b,
                &bindings_b,
            ),
            Err(Error::MismatchedState)
        );

        // One key claimed for two roles refuses as an alias even when the
        // bindings agree with the metadata.
        let mut aliased_metadata = metadata_b;
        aliased_metadata.position.key = metadata_b.supply.key;
        let mut aliased_bindings = bindings_b;
        aliased_bindings.position = bindings_b.supply;
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&b),
                &aliased_metadata,
                &aliased_bindings,
            ),
            Err(Error::AccountAlias)
        );

        // The actor aliased onto the position account refuses.
        let mut actor_alias = metadata_b;
        actor_alias.actor.key = metadata_b.position.key;
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&b),
                &actor_alias,
                &bindings_b,
            ),
            Err(Error::AccountAlias)
        );
    }

    #[test]
    fn donation_and_direct_burn_accounting_is_one_sided() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let a = split_state(&f, 20);

        // Over-counting — the ledger carries 5 claims per outcome of a
        // position this transition never sees — is the accepted, conservative
        // direction. A's transition moves the ledger by A's delta only and
        // the unpresented claims stay represented.
        let mut over = a.clone();
        let mut ledger = SupplyLedgerAccount::decode(&over.supply).unwrap();
        ledger.internal_supply[0] += 5;
        ledger.internal_supply[1] += 5;
        ledger.encode(&mut over.supply).unwrap();
        let mut kernel = KernelAccount::decode(&over.kernel).unwrap();
        kernel.total_supply[0] += 5;
        kernel.total_supply[1] += 5;
        kernel.encode(&mut over.kernel).unwrap();
        let mut hoard = HoardAccount::decode(&over.hoard).unwrap();
        hoard.collateral_atoms = 25;
        hoard.encode(&mut over.hoard).unwrap();
        let materialized = apply_layout(
            &over,
            &f.metadata,
            &f.bindings,
            1,
            Intent::Materialize {
                market,
                owner,
                destination: f.metadata.external.key,
                outcome: 0,
                quantity: 4,
            },
        )
        .unwrap();
        let ledger = SupplyLedgerAccount::decode(&materialized.supply).unwrap();
        assert_eq!(ledger.internal_supply[0], 21);
        assert_eq!(ledger.external_supply[0], 4);
        assert_eq!(ledger.aggregate_supply(0), Ok(25));

        // Under-counting — the position exceeding the ledger's internal term
        // — is the refused direction: a burned ledger term with an intact
        // position is a counterfeit claim.
        let mut under = a.clone();
        let mut ledger = SupplyLedgerAccount::decode(&under.supply).unwrap();
        ledger.internal_supply[0] = 15;
        ledger.encode(&mut under.supply).unwrap();
        let mut kernel = KernelAccount::decode(&under.kernel).unwrap();
        kernel.total_supply[0] = 15;
        kernel.encode(&mut under.kernel).unwrap();
        assert_eq!(
            apply_layout(
                &under,
                &f.metadata,
                &f.bindings,
                1,
                Intent::Materialize {
                    market,
                    owner,
                    destination: f.metadata.external.key,
                    outcome: 0,
                    quantity: 1,
                },
            ),
            Err(Error::AggregateClosureMismatch)
        );

        // A forged external shadow exceeding the ledger's external term
        // refuses the same way.
        let mut minted = a.clone();
        let mut shadow = ExternalAccount::decode(&minted.external).unwrap();
        shadow.balances[0] = 1;
        shadow.encode(&mut minted.external).unwrap();
        assert_eq!(
            apply_layout(
                &minted,
                &f.metadata,
                &f.bindings,
                1,
                Intent::Dematerialize {
                    market,
                    owner,
                    source: f.metadata.external.key,
                    outcome: 0,
                    quantity: 1,
                },
            ),
            Err(Error::AggregateClosureMismatch)
        );
    }

    #[test]
    fn concurrent_same_slot_interleavings_commute_on_the_ledger() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner_a = PositionAccount::decode(&f.state.position).unwrap().owner;
        let (b_start, metadata_b, bindings_b) = second_triple(&f, 100);
        let owner_b = PositionAccount::decode(&b_start.position).unwrap().owner;
        let split_a = Intent::Split {
            market,
            owner: owner_a,
            quantity: 20,
        };
        let split_b = Intent::Split {
            market,
            owner: owner_b,
            quantity: 5,
        };

        // Order one: A then B.
        let a1 = split_state(&f, 20);
        let mut b1 = b_start.clone();
        sync_shared(&a1, &mut b1);
        let b1 = apply_layout(&b1, &metadata_b, &bindings_b, 0, split_b).unwrap();

        // Order two: B then A.
        let b2 = apply_layout(&b_start, &metadata_b, &bindings_b, 0, split_b).unwrap();
        let mut a2 = f.state.clone();
        sync_shared(&b2, &mut a2);
        let a2 = apply_layout(&a2, &f.metadata, &f.bindings, 0, split_a).unwrap();

        // Both serializations preserve the sums and land on the identical
        // market-wide post-state, and each owner's triple is byte-identical
        // either way. The runtime's writable-account lock on the one ledger
        // is what forces some serialization to exist (obligation 3); each
        // serialized transition preserves the invariant, so any order does.
        assert_ledger_is_position_sum(&b1, &[&a1, &b1]);
        assert_ledger_is_position_sum(&a2, &[&a2, &b2]);
        assert_eq!(b1.supply, a2.supply);
        assert_eq!(b1.kernel, a2.kernel);
        assert_eq!(b1.hoard, a2.hoard);
        assert_eq!(a1.position, a2.position);
        assert_eq!(a1.replay, a2.replay);
        assert_eq!(b1.position, b2.position);
        assert_eq!(b1.replay, b2.replay);

        // Within one position the replay sequence is the concurrency control:
        // re-submitting A's split against the serialized post-state refuses.
        let mut a_replay = a1.clone();
        sync_shared(&b1, &mut a_replay);
        assert_eq!(
            apply_layout(&a_replay, &f.metadata, &f.bindings, 0, split_a),
            Err(Error::Replay)
        );

        // A stale ledger snapshot that cannot cover a position's holdings
        // refuses; that a *live* ledger is presented at all is the runtime's
        // writable-account guarantee, which the offline model names rather
        // than checks.
        let mut stale = b1.clone();
        stale.supply = f.state.supply;
        stale.kernel = f.state.kernel;
        stale.hoard = f.state.hoard;
        assert_eq!(
            apply_layout(
                &stale,
                &metadata_b,
                &bindings_b,
                1,
                Intent::Materialize {
                    market,
                    owner: owner_b,
                    destination: metadata_b.external.key,
                    outcome: 0,
                    quantity: 1,
                },
            ),
            Err(Error::AggregateClosureMismatch)
        );
    }

    #[test]
    fn market_global_resolution_is_exactly_idempotent_without_owner_replay() {
        let f = fixture();
        let split = split_state(&f, 15);
        let (window, len) = encode_window(&f.window_spec(), &winning_records());
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        let metadata = resolution_metadata(&f);
        let bindings = resolution_bindings(&f);

        /* Split advanced the owner's replay sequence to one. Resolution uses
         * the immutable repair generation (zero) instead, demonstrating that
         * the owner nonce is neither read nor consumed. */
        assert_eq!(ReplayAccount::decode(&split.replay).unwrap().sequence, 1);
        let first = apply_market_resolution_with_evidence(
            &resolve_request(V1_EXACT_GENERATION, 1),
            resolution_state_bytes(&split),
            &evidence,
            &metadata,
            &bindings,
        )
        .unwrap();
        assert!(!first.repeated);
        assert_eq!(first.hoard, split.hoard);
        assert_eq!(first.supply, split.supply);

        let repeated_evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &first.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR + 50,
            /* A retry in a later slot cannot rewrite the first recorded slot. */
            resolved_slot: RESOLVED_SLOT + 99,
        };
        let repeated = apply_market_resolution_with_evidence(
            &resolve_request(V1_EXACT_GENERATION, 1),
            ResolutionStateBytes {
                market: &first.market,
                hoard: &first.hoard,
                kernel: &first.kernel,
                supply: &first.supply,
            },
            &repeated_evidence,
            &metadata,
            &bindings,
        )
        .unwrap();
        assert!(repeated.repeated);
        assert_eq!(repeated.market, first.market);
        assert_eq!(repeated.hoard, first.hoard);
        assert_eq!(repeated.kernel, first.kernel);
        assert_eq!(repeated.supply, first.supply);
        assert_eq!(repeated.resolution, first.resolution);

        assert_eq!(
            apply_market_resolution_with_evidence(
                &resolve_request(V1_EXACT_GENERATION + 1, 1),
                ResolutionStateBytes {
                    market: &first.market,
                    hoard: &first.hoard,
                    kernel: &first.kernel,
                    supply: &first.supply,
                },
                &repeated_evidence,
                &metadata,
                &bindings,
            ),
            Err(Error::Replay)
        );

        let mut conflicting = repeated_evidence;
        conflicting.bindings.window_id = h(78);
        assert_eq!(
            apply_market_resolution_with_evidence(
                &resolve_request(V1_EXACT_GENERATION, 1),
                ResolutionStateBytes {
                    market: &first.market,
                    hoard: &first.hoard,
                    kernel: &first.kernel,
                    supply: &first.supply,
                },
                &conflicting,
                &metadata,
                &bindings,
            ),
            Err(Error::ResolutionBindingMismatch)
        );

        let mut aliased = metadata;
        aliased.actor.key = aliased.market.key;
        assert_eq!(
            apply_market_resolution_with_evidence(
                &resolve_request(V1_EXACT_GENERATION, 1),
                resolution_state_bytes(&split),
                &evidence,
                &aliased,
                &bindings,
            ),
            Err(Error::AccountAlias)
        );
    }

    #[test]
    fn market_global_resolution_rejects_wrong_window_generation() {
        let f = fixture();
        let split = split_state(&f, 15);
        let mut wrong = f.window_spec();
        wrong.generation += 1;
        let (window, len) = encode_window(&wrong, &winning_records());
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_market_resolution_with_evidence(
                &resolve_request(V1_EXACT_GENERATION, 1),
                resolution_state_bytes(&split),
                &evidence,
                &resolution_metadata(&f),
                &resolution_bindings(&f),
            ),
            Err(Error::Resolution(ResolutionRefusal::WindowDomainMismatch(
                WindowError::MismatchedGeneration
            )))
        );
    }

    #[test]
    fn signer_cannot_bypass_missing_resolution_evidence() {
        let mut f = fixture();
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let split = split_state(&f, 15);
        f.metadata.actor = ActorMetadata {
            key: h(60),
            signer: true,
        };
        assert_eq!(
            apply(
                &resolve_request(1, 1),
                state_bytes(&split),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionEvidenceUnavailable)
        );

        let mut forged_resolved = split;
        let mut market_account = MarketAccount::decode(&forged_resolved.market).unwrap();
        market_account.lifecycle = 1;
        market_account.encode(&mut forged_resolved.market).unwrap();
        let mut kernel = KernelAccount::decode(&forged_resolved.kernel).unwrap();
        kernel.phase = 1;
        kernel.resolved_payout = 1;
        kernel.encode(&mut forged_resolved.kernel).unwrap();
        f.metadata.actor = ActorMetadata {
            key: owner,
            signer: true,
        };
        assert_eq!(
            apply(
                &redeem_request(1, 1, 15),
                state_bytes(&forged_resolved),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionEvidenceUnavailable)
        );
    }

    #[test]
    fn adapter_refuses_resolution_and_redemption_without_typed_evidence() {
        // The pre-evidence-plane fixture, unchanged: no terms artifact, no
        // resolution record, no observation page. Both actions must still land
        // in exactly the old refusal class, so the fail-closed default is a
        // missing code path rather than a check somebody can satisfy.
        let f = fixture();
        let split = split_state(&f, 12);
        assert_eq!(
            apply(
                &resolve_request(1, 0),
                state_bytes(&split),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionEvidenceUnavailable)
        );
        assert_eq!(
            apply(
                &resolve_request(1, 1),
                state_bytes(&split),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionEvidenceUnavailable)
        );
        assert_eq!(
            apply(
                &redeem_request(1, 1, 12),
                state_bytes(&split),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionEvidenceUnavailable)
        );

        // A layout intent admits no evidence plane at all.
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let (window, len) = encode_window(&f.window_spec(), &winning_records());
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        let split_request = layout_request(
            1,
            Intent::Split {
                market,
                owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply_with_evidence(
                &split_request[..layout_request_len(&split_request)],
                state_bytes(&split),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::UnexpectedEvidence)
        );
    }

    #[test]
    fn resolution_rejects_prefix_before_exact_window_seal() {
        let f = fixture();
        let split = split_state(&f, 12);
        let spec = f.window_spec();
        let all = winning_records();

        // A truncated prefix cannot seal: the fold never reaches the domain's
        // exclusive end, so no WindowResult exists to derive from.
        for prefix in 0..all.len() {
            let (window, len) = encode_window(&spec, &all[..prefix]);
            let evidence = ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &f.resolution,
                    window: &window[..len],
                },
                metadata: f.evidence_metadata,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            };
            assert_eq!(
                apply_with_evidence(
                    &resolve_request(1, 1),
                    state_bytes(&split),
                    &evidence,
                    &f.metadata,
                    &f.bindings,
                ),
                Err(Error::Window(WindowError::IncompleteDomain))
            );
        }

        // A complete fold whose feed cursor has not reached the maturity bound
        // is a different refusal: covered is not the same fact as mature.
        let (window, len) = encode_window(&spec, &all);
        for cursor in [END_BUCKET, FEED_CURSOR - 1] {
            let evidence = ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &f.resolution,
                    window: &window[..len],
                },
                metadata: f.evidence_metadata,
                bindings: f.evidence_bindings,
                feed_cursor: cursor,
                resolved_slot: RESOLVED_SLOT,
            };
            assert_eq!(
                apply_with_evidence(
                    &resolve_request(1, 1),
                    state_bytes(&split),
                    &evidence,
                    &f.metadata,
                    &f.bindings,
                ),
                Err(Error::Window(WindowError::NotMature))
            );
        }

        // An explicit gap is refused by the terms' registered coverage policy
        // even though the bare summary would happily answer `terminal`.
        let gapped = [
            (OBSERVATION_ACCEPTED, 100, 0, 0),
            (OBSERVATION_MISSING, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 102, 1, 1),
        ];
        let (window, len) = encode_window(&spec, &gapped);
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Window(WindowError::CoverageRefused))
        );

        // Reordered and duplicated buckets are not a fold of this window.
        let reordered = [
            (OBSERVATION_ACCEPTED, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 100, 0, 0),
            (OBSERVATION_ACCEPTED, 102, 1, 1),
        ];
        let (window, len) = encode_window(&spec, &reordered);
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Window(WindowError::NonContiguous))
        );
    }

    #[test]
    fn resolution_rejects_wrong_window_source_version_and_repair_generation() {
        let f = fixture();
        let split = split_state(&f, 12);
        let records = winning_records();
        let base = f.window_spec();

        let mut wrong_source = base;
        wrong_source.source_version = V1_SOURCE_VERSION + 1;
        let mut wrong_evaluator = base;
        wrong_evaluator.evaluator_version = V1_EVALUATOR_VERSION + 1;
        let mut wrong_adapter = base;
        wrong_adapter.source_adapter_id = [0x5a; IDENTITY_BYTES];
        let mut wrong_spec = base;
        wrong_spec.feed_spec_id = [0x5a; IDENTITY_BYTES];
        let mut wrong_generation = base;
        wrong_generation.generation = V1_EXACT_GENERATION + 1;
        let mut wrong_grid = base;
        wrong_grid.grid_version = GRID_VERSION + 1;
        let mut wrong_maturity = base;
        wrong_maturity.maturity_bucket_exclusive = START_BUCKET + MATURITY_HORIZON + 1;
        let mut wrong_coverage = base;
        wrong_coverage.coverage_policy_id = COVERAGE_POLICY_BOUNDED_GAPS;
        wrong_coverage.coverage_policy_parameter = 1;

        let cases = [
            (wrong_source, WindowError::MismatchedFeed),
            (wrong_evaluator, WindowError::MismatchedFeed),
            (wrong_adapter, WindowError::MismatchedFeed),
            (wrong_spec, WindowError::MismatchedFeed),
            (wrong_generation, WindowError::MismatchedGeneration),
            (wrong_grid, WindowError::MismatchedGrid),
            (wrong_maturity, WindowError::MismatchedMaturity),
            (wrong_coverage, WindowError::MismatchedCoveragePolicy),
        ];
        for (spec, reason) in cases {
            let cursor = if spec.maturity_bucket_exclusive > FEED_CURSOR {
                spec.maturity_bucket_exclusive
            } else {
                FEED_CURSOR
            };
            let (window, len) = encode_window(&spec, &records);
            let evidence = ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &f.resolution,
                    window: &window[..len],
                },
                metadata: f.evidence_metadata,
                bindings: f.evidence_bindings,
                feed_cursor: cursor,
                resolved_slot: RESOLVED_SLOT,
            };
            assert_eq!(
                apply_with_evidence(
                    &resolve_request(1, 1),
                    state_bytes(&split),
                    &evidence,
                    &f.metadata,
                    &f.bindings,
                ),
                Err(Error::Resolution(ResolutionRefusal::WindowDomainMismatch(
                    reason
                )))
            );
        }

        // A window over a different bucket range is a wrong window even when
        // every other field matches.
        let mut shifted = base;
        shifted.start_bucket = START_BUCKET + 1;
        shifted.end_bucket_exclusive = END_BUCKET + 1;
        let shifted_records = [
            (OBSERVATION_ACCEPTED, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 102, 0, 0),
            (OBSERVATION_ACCEPTED, 103, 1, 1),
        ];
        let (window, len) = encode_window(&shifted, &shifted_records);
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Resolution(ResolutionRefusal::WindowDomainMismatch(
                WindowError::WrongWindow
            )))
        );
    }

    #[test]
    fn adapter_binds_payout_set_to_immutable_terms_artifact() {
        let f = fixture();
        let split = split_state(&f, 12);
        let (window, len) = encode_window(&f.window_spec(), &winning_records());

        // The reference kernel account's payout set is caller-supplied bytes.
        // Substituting a payout vector there, without touching the immutable
        // terms artifact, must refuse: the market's digest committed to the
        // frozen set, not to whatever the transaction assembled.
        let mut forged = split.clone();
        let mut kernel = KernelAccount::decode(&forged.kernel).unwrap();
        let mut weights = [0; MAX_OUTCOMES];
        weights[0] = 1;
        kernel.payouts.vectors[1] = PayoutVector::new(1, weights);
        kernel.encode(&mut forged.kernel).unwrap();
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&forged),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::PayoutSetMismatch)
        );

        // Editing the terms artifact instead changes its self-certifying
        // digest, so the market no longer binds it at all.
        let mut swapped = f.terms_account;
        swapped.payouts[1].weights[0] = 1;
        swapped.payouts[1].weights[1] = 0;
        swapped.terms = swapped.recomputed_terms_digest().unwrap();
        let mut swapped_bytes = [0; account_len::TERMS];
        swapped.encode(&mut swapped_bytes).unwrap();
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &swapped_bytes,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::TermsBindingMismatch)
        );

        // A terms artifact whose digest field is simply forged to the market's
        // value fails its own re-encode check inside the frozen codec.
        let mut lying = f.terms_account;
        lying.payouts[1].weights[0] = 1;
        lying.payouts[1].weights[1] = 0;
        let mut lying_bytes = [0; account_len::TERMS];
        assert_eq!(
            lying.encode(&mut lying_bytes),
            Err(CodecError::NonCanonicalIdentity)
        );

        // The payout the evidence derives is the only one that may be
        // requested; asking for the other index refuses.
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 0),
                state_bytes(&split),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::PayoutIndexMismatch)
        );
    }

    #[test]
    fn ambiguous_interval_and_wrong_actor_mutability_refuse() {
        let f = fixture();
        let split = split_state(&f, 12);
        let spec = f.window_spec();

        // A terminal interval straddling the cell boundary is AMBIG-REFUSE-01.
        let straddling = [
            (OBSERVATION_ACCEPTED, 100, 0, 0),
            (OBSERVATION_ACCEPTED, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 102, 0, 1),
        ];
        let (window, len) = encode_window(&spec, &straddling);
        let evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &evidence,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Resolution(ResolutionRefusal::AmbiguousInterval))
        );

        let (window, len) = encode_window(&spec, &winning_records());
        let good = EvidenceBytes {
            terms: &f.terms,
            resolution: &f.resolution,
            window: &window[..len],
        };

        // The immutable terms artifact must never be presented writable.
        let mut writable_terms = f.evidence_metadata;
        writable_terms.terms.writable = true;
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &ResolutionEvidence {
                    bytes: good,
                    metadata: writable_terms,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ImmutableAccountWritable)
        );

        // A resolve writes the record, so a read-only record refuses.
        let mut readonly_record = f.evidence_metadata;
        readonly_record.resolution.writable = false;
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &ResolutionEvidence {
                    bytes: good,
                    metadata: readonly_record,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::NotWritable)
        );

        // An evidence account aliased onto a state account refuses.
        let mut aliased = f.evidence_metadata;
        aliased.terms.key = f.metadata.kernel.key;
        let mut aliased_bindings = f.evidence_bindings;
        aliased_bindings.terms = f.metadata.kernel.key;
        aliased.terms.writable = false;
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &ResolutionEvidence {
                    bytes: good,
                    metadata: aliased,
                    bindings: aliased_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::AccountAlias)
        );

        // The trusted window identity is refused when absent.
        let mut zero_window = f.evidence_bindings;
        zero_window.window_id = Hash32::ZERO;
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &ResolutionEvidence {
                    bytes: good,
                    metadata: f.evidence_metadata,
                    bindings: zero_window,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::WindowIdentityUnavailable)
        );

        // An unsigned actor still cannot resolve: evidence authorizes the
        // transition, but a transaction still has to have been submitted.
        let mut unsigned = f.metadata;
        unsigned.actor.signer = false;
        assert_eq!(
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &ResolutionEvidence {
                    bytes: good,
                    metadata: f.evidence_metadata,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &unsigned,
                &f.bindings,
            ),
            Err(Error::MissingSignature)
        );
    }

    #[test]
    fn unimplemented_policies_and_inadmissible_statistics_refuse() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap();

        // GEN-FINAL-AT-MATURITY-02 is blocked on a sealed feed-epoch object.
        let mut repaired = f.terms_account;
        repaired.repair_policy_id = u32::from(GEN_FINAL_AT_MATURITY_02);
        repaired.terms = repaired.recomputed_terms_digest().unwrap();
        let mut repaired_market = market;
        repaired_market.terms = repaired.terms;
        assert_eq!(
            ResolutionTerms::from_market_terms(&repaired_market, &repaired),
            Err(ResolutionRefusal::TermsMalformed)
        );

        // The v3 terms carry the coverage parameter, so a bounded-gap policy
        // is expressible — but only with a real bound: the registry refuses
        // a zero gap bound rather than defaulting one.
        let mut gapped = f.terms_account;
        gapped.coverage_policy_id = u32::from(COVERAGE_POLICY_BOUNDED_GAPS);
        gapped.terms = gapped.recomputed_terms_digest().unwrap();
        let mut gapped_market = market;
        gapped_market.terms = gapped.terms;
        assert_eq!(
            ResolutionTerms::from_market_terms(&gapped_market, &gapped),
            Err(ResolutionRefusal::TermsMalformed)
        );
        let mut bounded = gapped;
        bounded.coverage_policy_parameter = 1;
        bounded.terms = bounded.recomputed_terms_digest().unwrap();
        let mut bounded_market = market;
        bounded_market.terms = bounded.terms;
        assert!(ResolutionTerms::from_market_terms(&bounded_market, &bounded).is_ok());
        // ...and COMPLETE_REQUIRED still refuses a stray nonzero parameter.
        let mut stray = f.terms_account;
        stray.coverage_policy_parameter = 1;
        stray.terms = stray.recomputed_terms_digest().unwrap();
        let mut stray_market = market;
        stray_market.terms = stray.terms;
        assert_eq!(
            ResolutionTerms::from_market_terms(&stray_market, &stray),
            Err(ResolutionRefusal::TermsMalformed)
        );

        // An unregistered failure policy refuses at derivation, not at use.
        let mut held = f.terms_account;
        held.failure_policy_id = 99;
        held.terms = held.recomputed_terms_digest().unwrap();
        let mut held_market = market;
        held_market.terms = held.terms;
        assert_eq!(
            ResolutionTerms::from_market_terms(&held_market, &held),
            Err(ResolutionRefusal::TermsMalformed)
        );

        // R-01: a terms artifact for another market's digest.
        let mut other = market;
        other.terms = h(0x77);
        assert_eq!(
            ResolutionTerms::from_market_terms(&other, &f.terms_account),
            Err(ResolutionRefusal::TermsDigestMismatch)
        );

        let derived = ResolutionTerms::from_market_terms(&market, &f.terms_account).unwrap();
        assert_eq!(derived.statistic, STAT_TERMINAL_01);
        assert_eq!(derived.ambiguity_policy, AMBIG_REFUSE_01);
        assert_eq!(derived.generation_policy, GEN_EXACT_01);
        assert_eq!(derived.cell_count, 2);
        assert_eq!(derived.basis_degree, 0);
        assert_eq!(derived.knot_count, 1);
        assert_eq!(derived.knots[0], 1);
        assert_eq!(derived.payout_map[0], 0);
        assert_eq!(derived.payout_map[1], 1);
        assert_eq!(derived.payout_map[2], PAYOUT_MAP_UNUSED);
        assert_eq!(derived.window.generation(), V1_EXACT_GENERATION);
        assert_eq!(derived.window.start_bucket(), START_BUCKET);
        assert_eq!(derived.window.end_bucket_exclusive(), END_BUCKET);
        assert_eq!(
            derived.window.maturity_bucket_exclusive(),
            START_BUCKET + MATURITY_HORIZON
        );

        // R-02 and R-05 on the statistic registry.
        let mut relative = derived;
        relative.statistic = STAT_RELATIVE_TERMINAL_TWAP_05;
        assert_eq!(
            relative.validate(),
            Err(ResolutionRefusal::StatisticUnsupported)
        );
        let mut unregistered = derived;
        unregistered.statistic = 9;
        assert_eq!(
            unregistered.validate(),
            Err(ResolutionRefusal::TermsMalformed)
        );
        let mut compatible = derived;
        compatible.ambiguity_policy = AMBIG_COMPATIBLE_SET_02;
        assert_eq!(
            compatible.validate(),
            Err(ResolutionRefusal::TermsMalformed)
        );
        let mut uniform = derived;
        uniform.failure_policy = FAIL_EXTENDED_WINDOW_02;
        assert_eq!(uniform.validate(), Ok(()));

        // R-03: a partition with an empty cell, a non-increasing boundary, or
        // live padding is refused before any statistic is read.
        let mut zero_first = derived;
        zero_first.cell_count = 3;
        zero_first.knot_count = 2;
        zero_first.payout_map[2] = 1;
        zero_first.knots[0] = 0;
        zero_first.knots[1] = 2;
        assert_eq!(
            zero_first.validate(),
            Err(ResolutionRefusal::PartitionMalformed)
        );
        let mut flat = derived;
        flat.cell_count = 3;
        flat.knot_count = 2;
        flat.payout_map[2] = 1;
        flat.knots[0] = 5;
        flat.knots[1] = 5;
        assert_eq!(flat.validate(), Err(ResolutionRefusal::PartitionMalformed));
        let mut over_max = derived;
        over_max.knots[0] = MAX_VALUE + 1;
        assert_eq!(
            over_max.validate(),
            Err(ResolutionRefusal::PartitionMalformed)
        );
        let mut live_padding = derived;
        live_padding.knots[1] = 3;
        assert_eq!(
            live_padding.validate(),
            Err(ResolutionRefusal::PartitionMalformed)
        );
        let mut single_cell = derived;
        single_cell.cell_count = 1;
        single_cell.knot_count = 0;
        assert_eq!(
            single_cell.validate(),
            Err(ResolutionRefusal::PartitionMalformed)
        );

        // R-09: a live cell pointing outside the frozen payout set.
        let mut out_of_range = derived;
        out_of_range.payout_map[1] = 2;
        assert_eq!(
            out_of_range.validate(),
            Err(ResolutionRefusal::PayoutIndexOutOfRange)
        );

        // The R-nn classes are exactly the plan's registry.
        assert_eq!(ResolutionRefusal::TermsDigestMismatch.class(), 1);
        assert_eq!(ResolutionRefusal::TermsMalformed.class(), 2);
        assert_eq!(ResolutionRefusal::PartitionMalformed.class(), 3);
        assert_eq!(
            ResolutionRefusal::WindowDomainMismatch(WindowError::WrongWindow).class(),
            4
        );
        assert_eq!(ResolutionRefusal::StatisticUnsupported.class(), 5);
        assert_eq!(ResolutionRefusal::AmbiguousInterval.class(), 6);
        assert_eq!(ResolutionRefusal::NoAcceptedCoverage.class(), 7);
        assert_eq!(ResolutionRefusal::AmbiguousDenominator.class(), 8);
        assert_eq!(ResolutionRefusal::PayoutIndexOutOfRange.class(), 9);
        assert_eq!(ResolutionRefusal::MarketNotActive.class(), 10);
        assert_eq!(ResolutionRefusal::ArithmeticOverflow.class(), 11);
        assert_eq!(ResolutionRefusal::BasisMalformed.class(), 12);
        assert_eq!(ResolutionRefusal::WeightDerivationOverflow.class(), 13);
        assert_eq!(ResolutionRefusal::ValueOutOfRange.class(), 14);
        assert_eq!(ResolutionRefusal::NonPointEvidence.class(), 15);
        assert_eq!(ResolutionRefusal::DerivedVectorUnrepresentable.class(), 16);
        assert_eq!(ResolutionRefusal::WrongResolutionMode.class(), 17);

        // Merely flipping a categorical artifact's degree cannot manufacture
        // a smooth basis: its knot/outcome count and uniform declaration are
        // still malformed. Valid d2/d3 terms are tested at the vector seam.
        let mut smooth = derived;
        smooth.basis_degree = 2;
        assert_eq!(smooth.validate(), Err(ResolutionRefusal::BasisMalformed));
        smooth.basis_degree = 3;
        assert_eq!(smooth.validate(), Err(ResolutionRefusal::BasisMalformed));
        smooth.basis_degree = 4;
        assert_eq!(smooth.validate(), Err(ResolutionRefusal::BasisMalformed));
    }

    #[test]
    fn redemption_refuses_forged_resolved_state_and_unbound_records() {
        let f = fixture();
        let split = split_state(&f, 12);
        let (window, len) = encode_window(&f.window_spec(), &winning_records());
        let resolve_evidence = ResolutionEvidence {
            bytes: EvidenceBytes {
                terms: &f.terms,
                resolution: &f.resolution,
                window: &window[..len],
            },
            metadata: f.evidence_metadata,
            bindings: f.evidence_bindings,
            feed_cursor: FEED_CURSOR,
            resolved_slot: RESOLVED_SLOT,
        };
        let resolved = apply_with_evidence(
            &resolve_request(1, 1),
            state_bytes(&split),
            &resolve_evidence,
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        let record_bytes = resolved.resolution.unwrap();

        let mut readonly = f.evidence_metadata;
        readonly.resolution.writable = false;

        // Forged resolved market and kernel bytes without the terms/resolution
        // evidence chain: the record is still unresolved, so redemption fails.
        let mut forged = split.clone();
        let mut market_account = MarketAccount::decode(&forged.market).unwrap();
        market_account.lifecycle = 1;
        market_account.encode(&mut forged.market).unwrap();
        let mut kernel = KernelAccount::decode(&forged.kernel).unwrap();
        kernel.phase = 1;
        kernel.resolved_payout = 1;
        kernel.encode(&mut forged.kernel).unwrap();
        assert_eq!(
            apply_with_evidence(
                &redeem_request(1, 1, 12),
                state_bytes(&forged),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &f.resolution,
                        window: &[],
                    },
                    metadata: readonly,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionNotRecorded)
        );

        // A genuine resolution record presented against unresolved kernel
        // state does not resolve the market by being shown to it.
        assert_eq!(
            apply_with_evidence(
                &redeem_request(1, 1, 12),
                state_bytes(&split),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &record_bytes,
                        window: &[],
                    },
                    metadata: readonly,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::MismatchedState)
        );

        // Resolved kernel state whose payout index disagrees with the record.
        let mut disagreeing = resolved.clone();
        let mut kernel = KernelAccount::decode(&disagreeing.kernel).unwrap();
        kernel.resolved_payout = 0;
        kernel.encode(&mut disagreeing.kernel).unwrap();
        assert_eq!(
            apply_with_evidence(
                &redeem_request(2, 1, 12),
                state_bytes(&disagreeing),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &record_bytes,
                        window: &[],
                    },
                    metadata: readonly,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::MismatchedState)
        );

        // Redemption never re-derives a payout, so a window blob is refused.
        assert_eq!(
            apply_with_evidence(
                &redeem_request(2, 1, 12),
                state_bytes(&resolved),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &record_bytes,
                        window: &window[..len],
                    },
                    metadata: readonly,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::UnexpectedEvidence)
        );

        // A resolution record for another window identity refuses.
        let mut other_window = f.evidence_bindings;
        other_window.window_id = h(0x66);
        assert_eq!(
            apply_with_evidence(
                &redeem_request(2, 1, 12),
                state_bytes(&resolved),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &record_bytes,
                        window: &[],
                    },
                    metadata: readonly,
                    bindings: other_window,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::ResolutionBindingMismatch)
        );

        // The market cannot resolve twice.
        assert_eq!(
            apply_with_evidence(
                &resolve_request(2, 1),
                state_bytes(&resolved),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &record_bytes,
                        window: &window[..len],
                    },
                    metadata: f.evidence_metadata,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Resolution(ResolutionRefusal::MarketNotActive))
        );

        // Redemption is still the owner's action.
        let mut stranger = f.metadata;
        stranger.actor = ActorMetadata {
            key: h(60),
            signer: true,
        };
        assert_eq!(
            apply_with_evidence(
                &redeem_request(2, 1, 12),
                state_bytes(&resolved),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &record_bytes,
                        window: &[],
                    },
                    metadata: readonly,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &stranger,
                &f.bindings,
            ),
            Err(Error::UnauthorizedActor)
        );
    }

    #[test]
    fn aliases_versions_owners_bumps_and_replays_fail_closed() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let request = layout_request(
            0,
            Intent::Split {
                market,
                owner,
                quantity: 1,
            },
        );
        let request = &request[..layout_request_len(&request)];

        let mut alias = f.metadata;
        alias.hoard.key = alias.market.key;
        assert_eq!(
            apply(request, state_bytes(&f.state), &alias, &f.bindings),
            Err(Error::AccountAlias)
        );

        let mut versioned = f.state.market;
        versioned[1] = 2;
        let state = StateBytes {
            market: &versioned,
            ..state_bytes(&f.state)
        };
        assert_eq!(
            apply(request, state, &f.metadata, &f.bindings),
            Err(Error::Layout(CodecError::WrongVersion))
        );

        let mut wrong_owner = f.metadata;
        wrong_owner.kernel.owner_program = h(99);
        assert_eq!(
            apply(request, state_bytes(&f.state), &wrong_owner, &f.bindings),
            Err(Error::WrongProgramOwner)
        );

        let mut wrong_bump = f.bindings;
        wrong_bump.position_bump ^= 1;
        assert_eq!(
            apply(request, state_bytes(&f.state), &f.metadata, &wrong_bump),
            Err(Error::WrongBump)
        );

        let mut wrong_supply_bump = f.bindings;
        wrong_supply_bump.supply_bump ^= 1;
        assert_eq!(
            apply(
                request,
                state_bytes(&f.state),
                &f.metadata,
                &wrong_supply_bump
            ),
            Err(Error::WrongBump)
        );

        let first = apply(request, state_bytes(&f.state), &f.metadata, &f.bindings).unwrap();
        assert_eq!(
            apply(request, state_bytes(&first), &f.metadata, &f.bindings),
            Err(Error::Replay)
        );
    }

    #[test]
    fn replay_and_arithmetic_overflow_refuse_without_output() {
        let mut f = fixture();
        let mut replay = ReplayAccount::decode(&f.state.replay).unwrap();
        replay.sequence = u64::MAX;
        replay.encode(&mut f.state.replay).unwrap();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        let request = layout_request(
            u64::MAX,
            Intent::Split {
                market,
                owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &request[..layout_request_len(&request)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::Replay)
        );

        let overflow = fixture();
        let mut hoard = HoardAccount::decode(&overflow.state.hoard).unwrap();
        hoard.collateral_atoms = u64::MAX;
        let mut hoard_bytes = overflow.state.hoard;
        hoard.encode(&mut hoard_bytes).unwrap();
        let mut position = PositionAccount::decode(&overflow.state.position).unwrap();
        position.internal[0] = u64::MAX;
        position.internal[1] = u64::MAX;
        let mut position_bytes = overflow.state.position;
        position.encode(&mut position_bytes).unwrap();
        let mut kernel = KernelAccount::decode(&overflow.state.kernel).unwrap();
        kernel.total_supply[0] = u64::MAX;
        kernel.total_supply[1] = u64::MAX;
        let mut kernel_bytes = overflow.state.kernel;
        kernel.encode(&mut kernel_bytes).unwrap();
        let mut supply = SupplyLedgerAccount::decode(&overflow.state.supply).unwrap();
        supply.internal_supply[0] = u64::MAX;
        supply.internal_supply[1] = u64::MAX;
        let mut supply_bytes = overflow.state.supply;
        supply.encode(&mut supply_bytes).unwrap();
        let overflow_state = StateBytes {
            hoard: &hoard_bytes,
            position: &position_bytes,
            kernel: &kernel_bytes,
            supply: &supply_bytes,
            ..state_bytes(&overflow.state)
        };
        let overflow_request = layout_request(
            0,
            Intent::Split {
                market: MarketAccount::decode(overflow_state.market).unwrap().market,
                owner: position.owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &overflow_request[..layout_request_len(&overflow_request)],
                overflow_state,
                &overflow.metadata,
                &overflow.bindings,
            ),
            Err(Error::Arithmetic)
        );
    }

    #[test]
    fn unsupported_layout_intents_and_unsigned_owner_refuse() {
        let f = fixture();
        let market = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;
        /* `Merge` is no longer on this list: it is implemented, and the four
         * seam intents (`Split`, `Merge`, `Materialize`, `Dematerialize`) plus
         * `CreateMarket` are now the whole of the layout plane this adapter
         * models.  What remains outside the reference subset is the feed and
         * order families, and each of them must still fall through to
         * `UnsupportedIntent` rather than into a plane that does not model it. */
        for unsupported in [
            Intent::FeedAdvance {
                feed: FeedId::from_bytes([9; 32]),
                cursor: 1,
                evidence: h(0x1e),
            },
            Intent::CancelOrder {
                market,
                epoch: h(0x2e),
                owner,
                /* v4: an order id is a positional rank, not a caller-chosen
                 * identity, so the fixture names one the codec admits; the
                 * refusal under test is the plane's, not the id's. */
                order_id: clutch_solana_layout::canonical_order_id(1),
                generation: 2,
            },
            Intent::SettlePage {
                market,
                epoch: h(0x2e),
                page_index: 0,
            },
            /* The Tier 2 staged-creation pair (tags 47, 48) lands in the
             * layout wire ahead of any reference model of the clearing
             * plane, exactly as the genesis initializers did: the adapter
             * refuses both, so the SVM oracle for this family is the layout
             * codec byte-for-byte, never a comparison against this refusal. */
            Intent::InitClearWork {
                market,
                epoch: h(0x2e),
                candidate: h(0x3e),
            },
            Intent::GrowClearWork {
                market,
                epoch: h(0x2e),
                candidate: h(0x3e),
            },
            /* The general epoch lifecycle (tags 49, 50) lands ahead of any
             * reference model of the general clearing plane, exactly like the
             * staged-creation pair above: the adapter refuses both, so the
             * SVM oracle for this family is the layout codec byte-for-byte. */
            Intent::InitEpoch {
                market,
                epoch_index: 7,
                policy: h(0x4e),
                freeze_deadline_slot: 900,
            },
            Intent::FreezeEpoch {
                market,
                epoch: h(0x2e),
            },
            Intent::AdvanceClearWork {
                market,
                epoch: h(0x2e),
                candidate: h(0x3e),
                max_orders: 16,
            },
            Intent::AdvanceClearSlices {
                market,
                epoch: h(0x2e),
                candidate: h(0x3e),
                max_slices: 16,
            },
            Intent::CompleteClearWork {
                market,
                epoch: h(0x2e),
                candidate: h(0x3e),
            },
        ] {
            let request = layout_request(0, unsupported);
            assert_eq!(
                apply(
                    &request[..layout_request_len(&request)],
                    state_bytes(&f.state),
                    &f.metadata,
                    &f.bindings,
                ),
                Err(Error::UnsupportedIntent),
                "{unsupported:?} is outside the reference subset"
            );
        }

        let mut unsigned = f.metadata;
        unsigned.actor.signer = false;
        let split = layout_request(
            0,
            Intent::Split {
                market,
                owner,
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &split[..layout_request_len(&split)],
                state_bytes(&f.state),
                &unsigned,
                &f.bindings,
            ),
            Err(Error::MissingSignature)
        );

        let wrong_owner = layout_request(
            0,
            Intent::Split {
                market,
                owner: h(98),
                quantity: 1,
            },
        );
        assert_eq!(
            apply(
                &wrong_owner[..layout_request_len(&wrong_owner)],
                state_bytes(&f.state),
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::MismatchedState)
        );
    }

    #[test]
    fn evidence_gated_resolution_and_redemption_have_exact_byte_vectors() {
        let f = fixture();
        let market_id = MarketAccount::decode(&f.state.market).unwrap().market;
        let owner = PositionAccount::decode(&f.state.position).unwrap().owner;

        // create -> the fixture is a validated initial market.
        let mut init = f.state.clone();
        clear_init_cash(&mut init);
        assert_eq!(
            validate_market_init(
                &f.realm,
                &f.profile,
                &f.policy,
                &f.terms,
                state_bytes(&init),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Ok(())
        );

        // split 20 complete sets.
        let split = split_state(&f, 20);
        assert_eq!(
            HoardAccount::decode(&split.hoard).unwrap().collateral_atoms,
            20
        );

        // The exact canonical window preimage the terms name; a real adapter
        // hashes WINDOW_DOMAIN_TAG followed by exactly these bytes.
        let preimage = expected_window_preimage(
            &MarketAccount::decode(&split.market).unwrap(),
            &f.terms_account,
        )
        .unwrap();
        assert_eq!(preimage.len(), WINDOW_DOMAIN_BYTES);
        assert_eq!(&preimage[..8], b"DCWINR1\0");
        assert_eq!(
            u16::from_le_bytes([preimage[10], preimage[11]]),
            COVERAGE_POLICY_COMPLETE_REQUIRED
        );
        assert_eq!(&preimage[20..52], &f.terms_account.feed.bytes());
        assert_eq!(&preimage[52..84], &f.terms_account.feed.bytes());
        assert_eq!(
            u64::from_le_bytes(preimage[106..114].try_into().unwrap()),
            START_BUCKET
        );
        assert_eq!(
            u64::from_le_bytes(preimage[114..122].try_into().unwrap()),
            END_BUCKET
        );
        assert_eq!(
            u64::from_le_bytes(preimage[122..130].try_into().unwrap()),
            START_BUCKET + MATURITY_HORIZON
        );
        assert_eq!(
            u64::from_le_bytes(preimage[130..138].try_into().unwrap()),
            V1_EXACT_GENERATION
        );

        // observe/seal -> resolve.
        let (window, len) = encode_window(&f.window_spec(), &winning_records());
        let resolved = apply_with_evidence(
            &resolve_request(1, 1),
            state_bytes(&split),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &f.resolution,
                    window: &window[..len],
                },
                metadata: f.evidence_metadata,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &f.metadata,
            &f.bindings,
        )
        .unwrap();

        let mut expected_market = split.market;
        expected_market[131] = 1;
        let mut expected_kernel = split.kernel;
        expected_kernel[34] = 1;
        expected_kernel[36] = 1;
        let mut expected_replay = split.replay;
        expected_replay[74..82].copy_from_slice(&2_u64.to_le_bytes());
        let mut expected_resolution = f.resolution;
        expected_resolution[98..130].copy_from_slice(&h(77).bytes());
        expected_resolution[130..138].copy_from_slice(&FEED_CURSOR.to_le_bytes());
        expected_resolution[138..146].copy_from_slice(&END_BUCKET.to_le_bytes());
        expected_resolution[146..154].copy_from_slice(&V1_EXACT_GENERATION.to_le_bytes());
        expected_resolution[154..162].copy_from_slice(&RESOLVED_SLOT.to_le_bytes());
        expected_resolution[162] = 1;

        assert_eq!(resolved.market, expected_market);
        assert_eq!(resolved.hoard, split.hoard);
        assert_eq!(resolved.position, split.position);
        assert_eq!(resolved.kernel, expected_kernel);
        assert_eq!(resolved.external, split.external);
        assert_eq!(resolved.replay, expected_replay);
        assert_eq!(resolved.supply, split.supply);
        assert_eq!(resolved.resolution, Some(expected_resolution));
        assert_eq!(resolved.redemption_payout, 0);

        // redeem_internal the winning outcome in full.
        let mut readonly = f.evidence_metadata;
        readonly.resolution.writable = false;
        let redeemed = apply_with_evidence(
            &redeem_request(2, 1, 20),
            state_bytes(&resolved),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &expected_resolution,
                    window: &[],
                },
                metadata: readonly,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &f.metadata,
            &f.bindings,
        )
        .unwrap();

        let mut expected_hoard = resolved.hoard;
        expected_hoard[98..106].copy_from_slice(&0_u64.to_le_bytes());
        let mut expected_position = resolved.position;
        expected_position[82..90].copy_from_slice(&0_u64.to_le_bytes());
        expected_position[202..210].copy_from_slice(&100_u64.to_le_bytes());
        let mut expected_kernel = resolved.kernel;
        expected_kernel[47..55].copy_from_slice(&0_u64.to_le_bytes());
        let mut expected_replay = resolved.replay;
        expected_replay[74..82].copy_from_slice(&3_u64.to_le_bytes());
        let mut expected_supply = resolved.supply;
        expected_supply[83..91].copy_from_slice(&0_u64.to_le_bytes());

        assert_eq!(redeemed.market, resolved.market);
        assert_eq!(redeemed.hoard, expected_hoard);
        assert_eq!(redeemed.position, expected_position);
        assert_eq!(redeemed.kernel, expected_kernel);
        assert_eq!(redeemed.external, resolved.external);
        assert_eq!(redeemed.replay, expected_replay);
        assert_eq!(redeemed.supply, expected_supply);
        assert_eq!(redeemed.resolution, Some(expected_resolution));
        assert_eq!(redeemed.redemption_payout, 20);

        // The losing outcome pays nothing and cannot mint collateral.
        assert_eq!(
            PositionAccount::decode(&redeemed.position)
                .unwrap()
                .internal[0],
            20
        );
        assert_eq!(
            KernelAccount::decode(&redeemed.kernel)
                .unwrap()
                .total_supply[0],
            20
        );
        assert_eq!(
            SupplyLedgerAccount::decode(&redeemed.supply)
                .unwrap()
                .aggregate_supply(0),
            Ok(20)
        );
        // Redeeming the losing outcome burns the claim for exactly zero: the
        // Hoard is already empty and no path here can mint collateral back.
        let mut readonly = f.evidence_metadata;
        readonly.resolution.writable = false;
        let burned = apply_with_evidence(
            &redeem_request(3, 0, 1),
            state_bytes(&redeemed),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &expected_resolution,
                    window: &[],
                },
                metadata: readonly,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &f.metadata,
            &f.bindings,
        )
        .unwrap();
        assert_eq!(burned.redemption_payout, 0);
        assert_eq!(
            HoardAccount::decode(&burned.hoard)
                .unwrap()
                .collateral_atoms,
            0
        );
        assert_eq!(
            PositionAccount::decode(&burned.position)
                .unwrap()
                .cash_atoms,
            100
        );
        assert_eq!(
            SupplyLedgerAccount::decode(&burned.supply)
                .unwrap()
                .aggregate_supply(0),
            Ok(19)
        );

        assert_eq!(
            market_id,
            MarketAccount::decode(&redeemed.market).unwrap().market
        );
        assert_eq!(
            owner,
            PositionAccount::decode(&redeemed.position).unwrap().owner
        );
    }

    #[test]
    fn window_evidence_codec_refuses_malformed_blobs() {
        let f = fixture();
        let split = split_state(&f, 12);
        let spec = f.window_spec();
        let (window, len) = encode_window(&spec, &winning_records());

        let refuse = |bytes: &[u8]| {
            apply_with_evidence(
                &resolve_request(1, 1),
                state_bytes(&split),
                &ResolutionEvidence {
                    bytes: EvidenceBytes {
                        terms: &f.terms,
                        resolution: &f.resolution,
                        window: bytes,
                    },
                    metadata: f.evidence_metadata,
                    bindings: f.evidence_bindings,
                    feed_cursor: FEED_CURSOR,
                    resolved_slot: RESOLVED_SLOT,
                },
                &f.metadata,
                &f.bindings,
            )
        };

        assert_eq!(refuse(&[]), Err(Error::WrongLength));
        assert_eq!(refuse(&window[..len - 1]), Err(Error::WrongLength));
        let mut wrong_tag = window;
        wrong_tag[0] ^= 1;
        assert_eq!(refuse(&wrong_tag[..len]), Err(Error::WrongTag));
        let mut wrong_version = window;
        wrong_version[1] = 2;
        assert_eq!(refuse(&wrong_version[..len]), Err(Error::WrongVersion));
        let mut wrong_kind = window;
        wrong_kind[WINDOW_EVIDENCE_HEADER_BYTES] = 2;
        assert_eq!(refuse(&wrong_kind[..len]), Err(Error::NonCanonical));
        // A record labelled as an explicit gap may not also carry a value: the
        // third record holds [1, 1], so relabelling it is non-canonical.
        let mut valued_gap = window;
        valued_gap[WINDOW_EVIDENCE_HEADER_BYTES + (2 * OBSERVATION_RECORD_BYTES)] =
            OBSERVATION_MISSING;
        assert_eq!(refuse(&valued_gap[..len]), Err(Error::NonCanonical));

        // A zero feed identity names no adapter and is refused before folding.
        let mut zero_feed = spec;
        zero_feed.feed_spec_id = [0; IDENTITY_BYTES];
        let (bytes, zero_len) = encode_window(&zero_feed, &winning_records());
        assert_eq!(
            refuse(&bytes[..zero_len]),
            Err(Error::Window(WindowError::ZeroIdentity))
        );

        // An unregistered coverage policy is refused by the crate's registry.
        let mut unknown_policy = spec;
        unknown_policy.coverage_policy_id = 9;
        let (bytes, policy_len) = encode_window(&unknown_policy, &winning_records());
        assert_eq!(
            refuse(&bytes[..policy_len]),
            Err(Error::Window(WindowError::UnknownCoveragePolicy))
        );

        // A maturity bound before the window end is not a domain at all.
        let mut early_maturity = spec;
        early_maturity.maturity_bucket_exclusive = END_BUCKET - 1;
        let (bytes, maturity_len) = encode_window(&early_maturity, &winning_records());
        assert_eq!(
            refuse(&bytes[..maturity_len]),
            Err(Error::Window(WindowError::InvalidMaturity))
        );
    }

    /* --------------------------------------------------------------------
     * TermsAccount v3: boundary tables, derived bases, and the cap flow.
     * ------------------------------------------------------------------ */

    /// Re-freeze a fixture around revised terms: recompute the digest, then
    /// point the market, the kernel payout set, and the unresolved record at
    /// it, exactly as a founding write would have.
    fn refreeze_terms(f: &mut Fixture, mut terms: TermsAccount) {
        terms.terms = terms.recomputed_terms_digest().unwrap();
        let mut market = MarketAccount::decode(&f.state.market).unwrap();
        market.terms = terms.terms;
        market.collateral_cap = terms.collateral_cap;
        market.encode(&mut f.state.market).unwrap();
        let mut kernel = KernelAccount::decode(&f.state.kernel).unwrap();
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut index = 0usize;
        while index < usize::from(terms.payout_count) {
            vectors[index] = PayoutVector::new(
                terms.payouts[index].denominator,
                terms.payouts[index].weights,
            );
            index += 1;
        }
        kernel.payouts = PayoutSet::new(terms.payout_count, terms.outcome_count, vectors);
        kernel.encode(&mut f.state.kernel).unwrap();
        let mut record = ResolutionAccount::decode(&f.resolution).unwrap();
        record.terms = terms.terms;
        record.encode(&mut f.resolution).unwrap();
        terms.encode(&mut f.terms).unwrap();
        f.terms_account = terms;
    }

    /// One resolve attempt over a fixture state, with the last observed
    /// bucket carrying the interval `[low, high]`.
    fn resolve_terminal(
        f: &Fixture,
        state: &TransitionOutput,
        sequence: u64,
        payout: u8,
        low: u128,
        high: u128,
    ) -> Result<TransitionOutput> {
        let records = [
            (OBSERVATION_ACCEPTED, 100, 0, 0),
            (OBSERVATION_ACCEPTED, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 102, low, high),
        ];
        let (window, len) = encode_window(&f.window_spec(), &records);
        apply_with_evidence(
            &resolve_request(sequence, payout),
            state_bytes(state),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: &f.terms,
                    resolution: &f.resolution,
                    window: &window[..len],
                },
                metadata: f.evidence_metadata,
                bindings: f.evidence_bindings,
                feed_cursor: FEED_CURSOR,
                resolved_slot: RESOLVED_SLOT,
            },
            &f.metadata,
            &f.bindings,
        )
    }

    /// The pure derived-vector seam over one terminal interval.
    fn derived_vector_at(
        f: &Fixture,
        low: u128,
        high: u128,
    ) -> core::result::Result<PayoutVectorBytes, ResolutionRefusal> {
        let market = MarketAccount::decode(&f.state.market).unwrap();
        let derived = ResolutionTerms::from_market_terms(&market, &f.terms_account)?;
        let records = [
            (OBSERVATION_ACCEPTED, 100, 0, 0),
            (OBSERVATION_ACCEPTED, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 102, low, high),
        ];
        let (window, len) = encode_window(&f.window_spec(), &records);
        let window = fold_window_evidence(&window[..len], FEED_CURSOR).unwrap();
        derive_payout_vector(&derived, &window)
    }

    /// The same evidence as [`derived_vector_at`], handed over unreduced so a
    /// test can drive the joined derive-and-resolve seam.
    fn derived_terms_and_window(
        f: &Fixture,
        low: u128,
        high: u128,
    ) -> (ResolutionTerms, WindowResult) {
        let market = MarketAccount::decode(&f.state.market).unwrap();
        let derived = ResolutionTerms::from_market_terms(&market, &f.terms_account).unwrap();
        let records = [
            (OBSERVATION_ACCEPTED, 100, 0, 0),
            (OBSERVATION_ACCEPTED, 101, 0, 0),
            (OBSERVATION_ACCEPTED, 102, low, high),
        ];
        let (window, len) = encode_window(&f.window_spec(), &records);
        let window = fold_window_evidence(&window[..len], FEED_CURSOR).unwrap();
        (derived, window)
    }

    /// The frozen terms presets as the kernel payout set they anchor.
    fn kernel_payout_set(terms: &TermsAccount) -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut index = 0usize;
        while index < usize::from(terms.payout_count) {
            vectors[index] = PayoutVector::new(
                terms.payouts[index].denominator,
                terms.payouts[index].weights,
            );
            index += 1;
        }
        PayoutSet::new(terms.payout_count, terms.outcome_count, vectors)
    }

    /// The degree-1 hat-basis terms whose reachable weight lattice is exactly
    /// the preset set: two outcomes, `D = 7`, anchors at 0 and 8, so the
    /// eight reachable vectors `(7 − r, r)` are the eight frozen presets.
    fn hat_terms_with_enumerated_lattice(f: &Fixture) -> TermsAccount {
        let mut terms = f.terms_account;
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut r = 0usize;
        while r < MAX_PAYOUTS {
            let mut weights = [0u64; MAX_OUTCOMES];
            weights[0] = 7 - r as u64;
            weights[1] = r as u64;
            payouts[r] = PayoutVectorBytes {
                denominator: 7,
                weights,
            };
            r += 1;
        }
        terms.payout_count = MAX_PAYOUTS as u8;
        terms.payouts = payouts;
        terms.basis_degree = 1;
        terms.knot_count = 2;
        terms.uniform_log2_spacing = 3;
        terms.knots = [0; MAX_KNOTS];
        terms.knots[1] = 8;
        terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        terms.failure_payout_index = 0;
        terms
    }

    /// A degree-1 basis in the exact `B1-EXACT` variant `D = g = 2^3`, whose
    /// lattice (nine vectors) cannot fit the preset set.
    fn hat_terms_exact_d8(f: &Fixture) -> TermsAccount {
        let mut terms = hat_terms_with_enumerated_lattice(f);
        let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut left = [0u64; MAX_OUTCOMES];
        left[0] = 8;
        let mut right = [0u64; MAX_OUTCOMES];
        right[1] = 8;
        payouts[0] = PayoutVectorBytes {
            denominator: 8,
            weights: left,
        };
        payouts[1] = PayoutVectorBytes {
            denominator: 8,
            weights: right,
        };
        terms.payout_count = 2;
        terms.payouts = payouts;
        terms
    }

    #[test]
    fn threshold_boundary_market_resolves_end_to_end() {
        /* Obligation 18 closes: the plan §2.6 worked example — a binary
         * market with the frozen boundary table [50] — resolves through the
         * full evidence gate, which the v2 terms could not express at all. */
        let mut f = fixture();
        let mut terms = f.terms_account;
        terms.knots[0] = 50;
        refreeze_terms(&mut f, terms);
        let split = split_state(&f, 12);

        // terminal = [47, 49] -> cell 0 -> payout 0.
        let low_side = resolve_terminal(&f, &split, 1, 0, 47, 49).unwrap();
        assert_eq!(
            KernelAccount::decode(&low_side.kernel)
                .unwrap()
                .resolved_payout,
            0
        );
        // terminal = [50, 50] -> the boundary itself lands in the closed
        // upper cell -> payout 1.
        let at_boundary = resolve_terminal(&f, &split, 1, 1, 50, 50).unwrap();
        assert_eq!(
            KernelAccount::decode(&at_boundary.kernel)
                .unwrap()
                .resolved_payout,
            1
        );
        // terminal = [49, 51] straddles -> AMBIG-REFUSE-01.
        assert_eq!(
            resolve_terminal(&f, &split, 1, 0, 49, 51),
            Err(Error::Resolution(ResolutionRefusal::AmbiguousInterval))
        );
        // And the request must ask for the derived cell's payout.
        assert_eq!(
            resolve_terminal(&f, &split, 1, 1, 47, 49),
            Err(Error::PayoutIndexMismatch)
        );
    }

    #[test]
    fn degree_one_account_resolution_refuses_preset_lowering() {
        /* At x̂ = 3 the native vector is (4, 3). Even though that vector is
         * also present at preset index 3, this compatibility path has no v3
         * Resolution vector and must not lower shaped settlement into an
         * index lookup. The immutable stored mode makes that an earlier
         * kernel refusal. */
        let mut f = fixture();
        let terms = hat_terms_with_enumerated_lattice(&f);
        refreeze_terms(&mut f, terms);
        let split = split_state(&f, 14);

        assert_eq!(
            resolve_terminal(&f, &split, 1, 3, 3, 3),
            Err(Error::Kernel(KernelError::WrongResolutionMode))
        );
        assert_eq!(derived_vector_at(&f, 3, 3).unwrap().weights[..2], [4, 3]);
    }

    #[test]
    fn degree_one_account_path_names_the_native_storage_residue() {
        /* The same derivation under D = 64 lands on (40, 24), which no
         * preset carries. KernelAccount persists the immutable mode, while
         * the v3 SBF Resolution record remains the sole vector owner. This
         * compatibility path has no such record and refuses explicitly. */
        let mut f = fixture();
        let mut terms = hat_terms_exact_d8(&f);
        let mut left = [0u64; MAX_OUTCOMES];
        left[0] = 64;
        let mut right = [0u64; MAX_OUTCOMES];
        right[1] = 64;
        terms.payouts[0] = PayoutVectorBytes {
            denominator: 64,
            weights: left,
        };
        terms.payouts[1] = PayoutVectorBytes {
            denominator: 64,
            weights: right,
        };
        refreeze_terms(&mut f, terms);
        let split = split_state(&f, 14);
        assert_eq!(
            resolve_terminal(&f, &split, 1, 0, 3, 3),
            Err(Error::Kernel(KernelError::WrongResolutionMode))
        );
        /* The pure seam still derives the validated member-shaped vector.
         * Account-backed native persistence belongs to the SBF adapter, not
         * this resolve-by-index compatibility helper. */
        assert_eq!(derived_vector_at(&f, 3, 3).unwrap().weights[..2], [40, 24]);
        /* A knot vector that happens to equal a preset refuses too: the mode
         * boundary is semantic, not a membership optimization. */
        assert_eq!(
            resolve_terminal(&f, &split, 1, 1, 8, 8),
            Err(Error::Kernel(KernelError::WrongResolutionMode))
        );
    }

    #[test]
    fn degree_one_vector_outside_the_presets_resolves_through_the_kernel_seam() {
        /* The same D = 64 terms whose derived vector (40, 24) no preset
         * carries resolves directly and pays the derived fractions exactly.
         * No step of this path searches the preset set. */
        let mut f = fixture();
        let mut terms = hat_terms_exact_d8(&f);
        let mut left = [0u64; MAX_OUTCOMES];
        left[0] = 64;
        let mut right = [0u64; MAX_OUTCOMES];
        right[1] = 64;
        terms.payouts[0] = PayoutVectorBytes {
            denominator: 64,
            weights: left,
        };
        terms.payouts[1] = PayoutVectorBytes {
            denominator: 64,
            weights: right,
        };
        refreeze_terms(&mut f, terms);

        let payouts = kernel_payout_set(&f.terms_account);
        let mut market = MarketState::new(2, BasisMode::DerivedBasis, payouts, 0).unwrap();
        let mut position = Position::EMPTY;
        market.split(&mut position, 64).unwrap();

        let (derived, window) = derived_terms_and_window(&f, 3, 3);
        let installed = resolve_derived_market(&mut market, &derived, &window).unwrap();
        assert_eq!(installed.denominator, 64);
        assert_eq!(installed.weights[..2], [40, 24]);
        assert_eq!(market.resolved_vector, installed);
        // The vector is genuinely outside the frozen set: this is the case
        // preset membership could not express, not a member in disguise.
        let mut index = 0usize;
        while index < usize::from(f.terms_account.payout_count) {
            assert_ne!(payouts.vectors[index], installed);
            index += 1;
        }

        // 64 * 40 / 64 = 40 and 64 * 24 / 64 = 24, both exact.
        assert_eq!(market.redeem_internal(&mut position, 0, 64), Ok(40));
        assert_eq!(market.redeem_internal(&mut position, 1, 64), Ok(24));
        assert_eq!(market.collateral, 0);
        market.check_invariants().unwrap();

        // Remainder refusal and the complete-set exit survive the new seam:
        // 3 * 40 / 64 is not an atom count, and a balanced holder still exits
        // exactly at the same vector.
        let mut second = MarketState::new(2, BasisMode::DerivedBasis, payouts, 0).unwrap();
        let mut holder = Position::EMPTY;
        second.split(&mut holder, 3).unwrap();
        resolve_derived_market(&mut second, &derived, &window).unwrap();
        assert_eq!(
            second.redeem_internal(&mut holder, 0, 3),
            Err(KernelError::RemainderRequired)
        );
        assert_eq!(second.redeem_complete_set(&mut holder, 3), Ok(3));
        assert_eq!(second.collateral, 0);
        assert_eq!(holder, Position::EMPTY);
    }

    #[test]
    fn degree_one_lattice_resolves_past_the_eight_preset_cap() {
        /* The defect the residue named, measured: with D = g = 8 the reachable
         * lattice has nine members and `MAX_PAYOUTS` is eight, so preset
         * membership could not represent the lattice at any enumeration. Every
         * one of the nine resolves through the kernel seam and pays exactly,
         * and the complete set is worth exactly one collateral unit at each —
         * Theorem (ii) of design §3.2 over the whole reachable set. */
        let mut f = fixture();
        let terms = hat_terms_exact_d8(&f);
        refreeze_terms(&mut f, terms);
        let payouts = kernel_payout_set(&f.terms_account);

        let mut distinct = 0u32;
        let mut value = 0u128;
        while value <= 8 {
            let (derived, window) = derived_terms_and_window(&f, value, value);
            let mut market = MarketState::new(2, BasisMode::DerivedBasis, payouts, 0).unwrap();
            let mut position = Position::EMPTY;
            market.split(&mut position, 8).unwrap();
            let installed = resolve_derived_market(&mut market, &derived, &window).unwrap();
            let right = value as u64;
            assert_eq!(installed.denominator, 8);
            assert_eq!(installed.weights[..2], [8 - right, right], "at {value}");

            // 8 * w_i / 8 = w_i: every leg is exact at this denominator.
            assert_eq!(market.redeem_internal(&mut position, 0, 8), Ok(8 - right));
            assert_eq!(market.redeem_internal(&mut position, 1, 8), Ok(right));
            assert_eq!(market.collateral, 0);
            market.check_invariants().unwrap();

            // The same vector, exited as a complete set instead.
            let mut whole = MarketState::new(2, BasisMode::DerivedBasis, payouts, 0).unwrap();
            let mut balanced = Position::EMPTY;
            whole.split(&mut balanced, 5).unwrap();
            resolve_derived_market(&mut whole, &derived, &window).unwrap();
            assert_eq!(whole.redeem_complete_set(&mut balanced, 5), Ok(5));
            assert_eq!(whole.collateral, 0);

            distinct += 1;
            value += 1;
        }
        assert_eq!(distinct, 9);
        assert!(usize::from(distinct as u8) > MAX_PAYOUTS);
    }

    #[test]
    fn the_derived_resolution_seam_refuses_both_wrong_modes() {
        /* One resolution seam per mode, never both, checked at each end
         * independently: the derivation refuses degree-0 terms (R-17) and the
         * kernel refuses a `FinitePreset` market, so a caller cannot cross the
         * seams by supplying a matched pair of the wrong kind. */
        let mut categorical = fixture();
        let mut boundary = categorical.terms_account;
        boundary.knots[0] = 50;
        refreeze_terms(&mut categorical, boundary);
        let (deg0_terms, deg0_window) = derived_terms_and_window(&categorical, 47, 49);
        let mut derived_market = MarketState::new(
            2,
            BasisMode::DerivedBasis,
            kernel_payout_set(&categorical.terms_account),
            0,
        )
        .unwrap();
        let before = derived_market;
        assert_eq!(
            resolve_derived_market(&mut derived_market, &deg0_terms, &deg0_window),
            Err(Error::Resolution(ResolutionRefusal::WrongResolutionMode))
        );
        assert_eq!(derived_market, before);

        let mut hat = fixture();
        let terms = hat_terms_exact_d8(&hat);
        refreeze_terms(&mut hat, terms);
        let (deg1_terms, deg1_window) = derived_terms_and_window(&hat, 3, 3);
        let mut preset_market = MarketState::new(
            2,
            BasisMode::FinitePreset,
            kernel_payout_set(&hat.terms_account),
            0,
        )
        .unwrap();
        let before = preset_market;
        assert_eq!(
            resolve_derived_market(&mut preset_market, &deg1_terms, &deg1_window),
            Err(Error::Kernel(KernelError::WrongResolutionMode))
        );
        assert_eq!(preset_market, before);
        // And the mode-0 market still resolves through its own seam.
        preset_market.resolve(1).unwrap();
    }

    #[test]
    fn degree_one_pow2_weights_are_exact_shifts_and_sum_to_d() {
        /* B1-EXACT (design §2.4): with D = g = 2^3 the pane-local coordinate
         * IS the weight — w_right == u for every u in the pane, the floor is
         * the identity, and the partition of unity is exact at every x̂.
         * Checked exhaustively over the whole pane, not sampled. */
        let mut f = fixture();
        let terms = hat_terms_exact_d8(&f);
        refreeze_terms(&mut f, terms);
        let mut value = 0u128;
        while value <= 12 {
            let vector = derived_vector_at(&f, value, value).unwrap();
            let expected_right = if value >= 8 { 8 } else { value as u64 };
            assert_eq!(vector.denominator, 8);
            assert_eq!(vector.weights[1], expected_right, "at {value}");
            assert_eq!(vector.weights[0], 8 - expected_right, "at {value}");
            assert_eq!(
                vector.weights.iter().sum::<u64>(),
                8,
                "partition of unity at {value}"
            );
            value += 1;
        }

        /* Pane-boundary continuity on a three-anchor grid: the u -> g limit
         * of pane 0 meets pane 1's u = 0 exactly at the shared knot, and the
         * shift path and the scan path derive identical vectors. */
        let mut three = hat_terms_exact_d8(&f);
        three.outcome_count = 3;
        three.knot_count = 3;
        three.knots[2] = 16;
        let mut vectors = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
        let mut index = 0usize;
        while index < 3 {
            let mut weights = [0u64; MAX_OUTCOMES];
            weights[index] = 8;
            vectors[index] = PayoutVectorBytes {
                denominator: 8,
                weights,
            };
            index += 1;
        }
        three.payout_count = 3;
        three.payouts = vectors;
        /* The fixture market and window stay two-outcome; this arm exercises
         * only the pure derivation, so a bare market head carrying the digest
         * is enough. */
        three.terms = three.recomputed_terms_digest().unwrap();
        let mut market = MarketAccount::decode(&f.state.market).unwrap();
        market.terms = three.terms;
        let derived = ResolutionTerms::from_market_terms(&market, &three).unwrap();
        let mut scanned = derived;
        scanned.uniform_log2_spacing = UNIFORM_SPACING_NONE;
        let mut value = 0u128;
        while value <= 16 {
            let records = [
                (OBSERVATION_ACCEPTED, 100, 0, 0),
                (OBSERVATION_ACCEPTED, 101, 0, 0),
                (OBSERVATION_ACCEPTED, 102, value, value),
            ];
            let (window, len) = encode_window(&f.window_spec(), &records);
            let window = fold_window_evidence(&window[..len], FEED_CURSOR).unwrap();
            let shifted = derive_payout_vector(&derived, &window).unwrap();
            let walked = derive_payout_vector(&scanned, &window).unwrap();
            assert_eq!(shifted, walked, "shift path == scan path at {value}");
            assert_eq!(shifted.weights.iter().sum::<u64>(), 8);
            if value == 8 {
                /* The shared knot: full weight on the middle claim, exactly
                 * the limit of pane 0 (u -> 8) and the start of pane 1. */
                assert_eq!(shifted.weights[..3], [0, 8, 0]);
            }
            if value == 7 {
                assert_eq!(shifted.weights[..3], [1, 7, 0]);
            }
            if value == 9 {
                assert_eq!(shifted.weights[..3], [0, 7, 1]);
            }
            value += 1;
        }
    }

    #[test]
    fn degree_one_ambiguity_and_edge_policies_refuse_or_clamp() {
        let mut f = fixture();
        let terms = hat_terms_exact_d8(&f);
        refreeze_terms(&mut f, terms);

        /* The generalized AMBIG-REFUSE-01: endpoint weight vectors differ,
         * so the interval refuses — and by φ-monotonicity, agreement would
         * have implied constancy on the whole interval. */
        assert_eq!(
            derived_vector_at(&f, 1, 2),
            Err(ResolutionRefusal::AmbiguousInterval)
        );
        /* EDGE-CLAMP-01: out-of-span values clamp to the extreme anchors, so
         * an interval entirely above the span is *not* ambiguous. */
        assert_eq!(derived_vector_at(&f, 9, 20).unwrap().weights[1], 8);
        assert_eq!(derived_vector_at(&f, 0, 0).unwrap().weights[0], 8);

        /* EDGE-REFUSE-02: the same interval refuses into the failure policy
         * class instead of clamping. */
        let mut refusing = f.terms_account;
        refusing.edge_policy_id = EDGE_REFUSE_02;
        refreeze_terms(&mut f, refusing);
        assert_eq!(
            derived_vector_at(&f, 9, 20),
            Err(ResolutionRefusal::ValueOutOfRange)
        );
        assert_eq!(derived_vector_at(&f, 8, 8).unwrap().weights[1], 8);
    }

    #[test]
    fn degree_gates_twap_and_the_vector_seam_refuses_the_wrong_mode() {
        let f = fixture();
        /* TWAP stays admissible for a degree-0 boundary table... */
        let mut categorical = f.terms_account;
        categorical.statistic_id = STAT_TWAP_04;
        categorical.terms = categorical.recomputed_terms_digest().unwrap();
        let mut market = MarketAccount::decode(&f.state.market).unwrap();
        market.terms = categorical.terms;
        assert!(ResolutionTerms::from_market_terms(&market, &categorical).is_ok());

        /* ...and is deferred for degree >= 1 (design §2.6): the weight
         * derivation's intermediate product has no proven u128 bound. */
        let mut derived = hat_terms_exact_d8(&f);
        derived.statistic_id = STAT_TWAP_04;
        derived.terms = derived.recomputed_terms_digest().unwrap();
        let mut market = MarketAccount::decode(&f.state.market).unwrap();
        market.terms = derived.terms;
        assert_eq!(
            ResolutionTerms::from_market_terms(&market, &derived),
            Err(ResolutionRefusal::StatisticUnsupported)
        );

        /* One resolution seam per mode: a categorical market has no derived
         * vector. */
        assert_eq!(
            derived_vector_at(&f, 0, 0),
            Err(ResolutionRefusal::WrongResolutionMode)
        );
    }

    #[test]
    fn market_init_cap_flow_binds_the_terms_cap_and_the_policy_ceiling() {
        /* The founded market's cap is the terms' own: a market whose stored
         * cap disagrees with its digest-bound terms refuses. */
        let mut f = fixture();
        clear_init_cash(&mut f.state);
        let mut market = MarketAccount::decode(&f.state.market).unwrap();
        assert_eq!(market.collateral_cap, 1_000);
        assert_eq!(f.terms_account.collateral_cap, 1_000);
        market.collateral_cap = 999;
        market.encode(&mut f.state.market).unwrap();
        assert_eq!(
            validate_market_init(
                &f.realm,
                &f.profile,
                &f.policy,
                &f.terms,
                state_bytes(&f.state),
                &f.create,
                &f.metadata,
                &f.bindings,
            ),
            Err(Error::TermsBindingMismatch)
        );

        /* A cap the Realm's admitted mint could never back refuses against
         * the recomputed policy's ceiling, even though the terms commit to
         * it. */
        let mut over = fixture();
        clear_init_cash(&mut over.state);
        let mut terms = over.terms_account;
        terms.collateral_cap = 2_000_000_000;
        refreeze_terms(&mut over, terms);
        /* refreeze_terms updated the market's digest but the create intent
         * still names the old one; rebuild it so the ceiling is the check
         * that speaks. */
        let create_intent = Intent::CreateMarket {
            realm: MarketAccount::decode(&over.state.market).unwrap().realm,
            profile: MarketAccount::decode(&over.state.market).unwrap().profile,
            market_nonce: 9,
            outcome_count: 2,
            terms: over.terms_account.terms,
            feed: over.terms_account.feed,
        };
        let mut create = [0; 139];
        assert_eq!(create_intent.encode(&mut create), Ok(139));
        assert_eq!(
            validate_market_init(
                &over.realm,
                &over.profile,
                &over.policy,
                &over.terms,
                state_bytes(&over.state),
                &create,
                &over.metadata,
                &over.bindings,
            ),
            Err(Error::CollateralCap)
        );
    }

    #[test]
    fn collateral_policy_binding_refuses_foreign_flipped_and_hostile_bytes() {
        let f = fixture();
        let mut init = f.state.clone();
        clear_init_cash(&mut init);
        let check = |profile: &[u8], policy: &[u8]| {
            validate_market_init(
                &f.realm,
                profile,
                policy,
                &f.terms,
                state_bytes(&init),
                &f.create,
                &f.metadata,
                &f.bindings,
            )
        };

        /* A *foreign*, perfectly well-formed policy: it decodes, and only
         * the recompute-and-compare refuses it. */
        let mut foreign = fixture_policy();
        foreign.collateral =
            collateral::CurrencyRef::spl(collateral::TOKEN_2022_PROGRAM, [0xaa; 32], 9);
        foreign.fee = foreign.collateral;
        let foreign_bytes = foreign.canonical_bytes().unwrap();
        assert_eq!(
            check(&f.profile, &foreign_bytes),
            Err(Error::Layout(CodecError::MismatchedBinding))
        );

        /* A bit-flipped stored digest: the Profile still decodes (frozen
         * flag, nonzero digest), and the binding is what refuses. */
        let mut flipped = ProfileAccount::decode(&f.profile).unwrap();
        let mut digest = flipped.collateral_policy_digest.bytes();
        digest[0] ^= 1;
        flipped.collateral_policy_digest = Hash32::from_bytes(digest);
        let mut flipped_bytes = [0; account_len::PROFILE];
        flipped.encode(&mut flipped_bytes).unwrap();
        assert_eq!(
            check(&flipped_bytes, &f.policy),
            Err(Error::Layout(CodecError::MismatchedBinding))
        );

        /* Hostile policy bytes surface the decoder's own refusal, never a
         * generic mismatch. */
        assert_eq!(
            check(&f.profile, &f.policy[..100]),
            Err(Error::Layout(CodecError::Truncated))
        );
        let mut wrong_magic = f.policy;
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            check(&f.profile, &wrong_magic),
            Err(Error::Layout(CodecError::WrongTag))
        );
    }

    include!("reference_adversarial_campaign.rs");
}
