//! `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal`.
//!
//! This module owns the observation and resolution plane: the feed head that
//! turns a folded observation page into an advanced, replay-guarded cursor
//! (digest-chained and signer-authorized — nothing here authenticates the
//! observation *sources*), and the
//! evidence gate that turns a sealed window into exactly one payout index.  It
//! contains no economic logic — the payout algebra is [`clutch_kernel`], the
//! window algebra is [`clutch_accumulator`], the terms-to-payout derivation is
//! [`clutch_solana_reference::derive_payout`], and byte ownership is
//! [`clutch_solana_layout`].  What lives here is the account list, the order of
//! the checks, the hostile decoding of the two caller-supplied blobs, and the
//! write-back.
//!
//! ## `RedeemInternal` now pays real collateral
//!
//! `Resolve` moves no value and its plane is unchanged at twelve accounts.
//! `RedeemInternal` pays collateral *out* of the Hoard, so it takes
//! [`REDEEM_ACCOUNT_COUNT`] — the twelve, plus the Profile, the token program,
//! the Realm's 266 collateral-policy bytes, the collateral mint, the
//! redeemer's own token account, the Hoard's signing authority, and the Hoard
//! token account.  The outflow is a `TransferChecked` signed by
//! [`crate::seeds::hoard_authority_pda`]: the probe established there is no
//! other shape, because a token account owned by a program address refuses a
//! wallet-signed transfer out.
//!
//! The admission decision over those accounts is
//! [`crate::instructions::split::validate_collateral_leg`], called with this
//! plane's own positions rather than copied — one decision procedure, two
//! account lists.  After the CPI the exact deltas and the mirror
//! `HoardAccount::collateral_atoms == hoard_token.amount` are required, which
//! is the same step-6 discipline `TOKEN2022_PLAN.md` §3.3 gives every other
//! token instruction.
//!
//! One deviation from that discipline is inherited and named: the transition
//! writes its seven state accounts *before* the CPI, because
//! `apply_evidence_transition` holds them as mutable borrows and a live
//! borrow across `invoke` is a runtime failure.  On chain the deviation is
//! invisible — SVM transaction semantics discard every byte on any later
//! refusal — and `programs/clutch-sbf/svm-tests` demonstrates that rather than
//! assuming it.
//!
//! ## The oracle
//!
//! `clutch_solana_reference::apply_with_evidence` is the whole gate offline.
//! Every check it performs is performed here, in the same order, with the same
//! refusal class — [`clutch_solana_reference::Error`] values, not a parallel
//! vocabulary — so that "same class" is `==` in a test rather than a
//! hand-maintained mapping.  It is not *called*: the SBF backend reports its
//! composition as overflowing the 4 KiB frame, so it is rebuilt here out of
//! `#[inline(never)]` frames that each hold at most two large decoded accounts.
//! The host tests at the bottom of this file run both implementations on
//! identical bytes and compare post-state and refusals.
//!
//! The reference has no `FeedAdvance`, so that instruction's oracle is the
//! accumulator's own algebra plus the frozen [`clutch_solana_layout::FeedAccount`]
//! codec, and its observation discipline mirrors the vertical model's: one
//! bucket at a time, exactly at the next expected bucket, refusing anything
//! else.
//!
//! ## Gate-order fidelity, and the three places the order is not identical
//!
//! | reference step | here | same order? |
//! | --- | --- | --- |
//! | `validate_metadata` (owner, key, writable, alias) | [`process`]: `require_count`, `require_distinct`, `validate_state_roles`, `expect_pda` | yes, first |
//! | `Request::decode` | [`crate::dispatch`] | yes, already recorded in `SBF_BRINGUP.md` |
//! | `DecodedState::decode` | `apply_evidence_transition` step 1 | yes |
//! | `validate_links` | step 2 | yes |
//! | `validate_padding` | step 3 | yes |
//! | `validate_aggregate_closure` | step 4 | yes |
//! | replay sequence | step 5 | yes |
//! | `kernel_market` invariants | step 6 | yes |
//! | `validate_evidence_metadata` writability + zero window id | step 7 | yes |
//! | `validate_evidence_metadata` owner/key/alias | [`process`], with the other addresses | **moved earlier** |
//! | signer / `authorize_owner` | step 8 | yes |
//! | `resolve_from_evidence` / `redeem_from_evidence` | step 9 | yes |
//! | kernel transition, ledger delta, post-state closure | steps 10-13 | yes |
//!
//! The three divergences, each fail-closed:
//!
//! 1. **Evidence-account addresses are checked with the other addresses.** An
//!    on-chain expected key is *derived* from the account's own decoded bytes,
//!    so no address comparison can happen mid-gate the way it does in the
//!    reference, where the expected keys arrive as trusted bindings.  The set
//!    of accepted requests is unchanged; a request that is both aliased and
//!    otherwise bad is named by the alias here and by the later check there.
//! 2. **Stored bumps are compared against a derived bump, not a supplied one.**
//!    [`process`] derives every canonical bump from [`crate::seeds`] and passes
//!    it in, and the comparison itself still happens at the reference's point
//!    (`validate_links`, `bind_terms`, `bind_resolution`), so the ordering is
//!    preserved while the binding is strictly stronger.
//! 3. **`feed_cursor` is read from the feed head, not supplied.** The reference
//!    takes the witnessed cursor as a caller parameter; here it is
//!    [`clutch_solana_layout::FeedAccount::cursor`], which only [`process`]'s
//!    `FeedAdvance` path can move, and only across buckets a folded page
//!    covered.  A caller can therefore no longer assert maturity.
//!
//! ## What this program cannot derive, and what that costs
//!
//! Two values the frozen layouts require are digests, and this crate owns no
//! hash primitive: [`clutch_accumulator`] deliberately publishes the canonical
//! `WindowDomain` preimage and no digest, and `clutch-solana-layout`'s SHA-256
//! is private to its own canonical identities.
//!
//! - **The window identity.**  [`clutch_solana_layout::ResolutionAccount`]
//!   refuses a zero `window` on a resolved record, so a resolve must write one.
//!   It arrives declared, in the evidence buffer, is refused when zero, and is
//!   *recorded, not believed* — exactly the posture
//!   `clutch_solana_reference::EvidenceBindings::window_id` documents.  No gate
//!   decision depends on it: which payout is selected comes from
//!   `WindowResult::check_domain` against the domain the market's own terms
//!   derive, field by field.  Its one live use is the redemption check that the
//!   record's digest equals the one declared at redeem time, which is a
//!   cross-transaction consistency check on a caller-chosen label and is not
//!   evidence about a window.
//! - **The feed summary digest.**  `Intent::FeedAdvance` carries a non-zero
//!   `evidence: Hash32` on the frozen wire and [`clutch_solana_layout::FeedAccount`]
//!   has a `summary` field for it.  It is recorded verbatim.  Nothing reads it,
//!   and nothing here or anywhere else in this repository proves it is the
//!   digest of the page that was folded.
//!
//! Both are obligations on a build that has selected a hash primitive; neither
//! is load-bearing for any refusal.
//!
//! ## Refusal codes
//!
//! `0x0050-0x005f` is this module's block, and `error.rs` now carries the
//! allocation this section used to only propose — the terms-revision wave
//! unfroze it.  The principle held: no refusal gained a *second* identifier.
//! What the block carries is the numeric **projection** of the gate's own
//! classes, which used to collapse onto the `0x3fff` catch-all:
//!
//! | class | code |
//! | --- | --- |
//! | `Error::Window(_)` | `0x0050` |
//! | `Error::Resolution(_)` | `0x0051` |
//! | `Error::TermsBindingMismatch` | `0x0052` |
//! | `Error::PayoutSetMismatch` | `0x0053` |
//! | `Error::ResolutionBindingMismatch` | `0x0054` |
//! | `Error::ResolutionAlreadyRecorded` | `0x0055` |
//! | `Error::ResolutionNotRecorded` | `0x0056` |
//! | `Error::PayoutIndexMismatch` | `0x0057` |
//! | `Error::ImmutableAccountWritable` | `0x0058` |
//! | `Error::UnexpectedEvidence` | `0x0059` |
//! | `Error::WindowIdentityUnavailable` | `0x005a` |
//! | `0x005b-0x005f` | unallocated |
//!
//! The sub-reasons a `Window(_)` or `Resolution(_)` carries stay one number
//! per class: they remain exactly distinguishable in the host differential,
//! which compares typed values, and a per-sub-reason identifier would be a
//! parallel truth.  `the_numeric_projection_of_the_gate_is_allocated` pins
//! the table, so a renumbering cannot pass silently.
//!
//! One consequence is visible from outside: the actor's signature is checked at
//! the reference's point in the gate, not hoisted to the account plane the way
//! [`super::split`] hoists it, so an unsigned resolve or redemption reports
//! `Error::MissingSignature` (`0x3009`) rather than
//! [`crate::error::ClutchError::MissingSignature`] (`0x0002`).  Hoisting it
//! would be a cheaper refusal and a different order, and order is what the
//! differential is checking.
//!
//! ## Frames, measured
//!
//! `cargo build-sbf` reports a per-function frame estimate, and it is the only
//! thing in this repository that can tell a 4 KiB frame from a 6 KiB one.  The
//! first cut of the gate put two decoded accounts in one frame and overflowed
//! four functions at 4288, 4672, 4800, and 6016 bytes — undefined behaviour
//! on-chain, invisible to every host test.  The shapes below are what fixed it,
//! and the build is re-run rather than reasoned about:
//!
//! - a decoded `TermsAccount` is 1656 bytes (v3), a `KernelAccount` 1264, a
//!   `MarketState` 1240, a `MarketAccount` 728;
//! - a *returned* `Result<TermsAccount, _>` costs the caller two such slots, so
//!   the big accounts are **loaded into** a caller slot (`load_terms` and
//!   friends) rather than returned;
//! - a full `TermsAccount::validate` builds a 1.6 KiB body buffer to recompute
//!   its own digest, in its own `#[inline(never)]` frame since the v3
//!   revision; the gate's own loads use `decode_unchecked` and never build
//!   the buffer, and the binding checks compare fields without re-validating;
//! - no function holds more than two of those values at once.
//!
//! Deleting an `#[inline(never)]` or an out-parameter here is not a style
//! change.  The offline adapter's own `apply_inner` still measures 9792 bytes,
//! which is the finding this module exists to route around.
//!
//! ## Named gaps
//!
//! - `resolved_slot` is written as zero.  This program has no clock: reading
//!   one needs a sysvar dependency or a syscall, and neither is a lane's call.
//!   Zero here means "no slot recorded", which is why nothing may ever read the
//!   field back as a slot.
//! - `FeedAccount::next_boundary` is neither read nor written.  Nothing in this
//!   repository says what a "next logical boundary" is, and inventing a
//!   boundary policy inside an instruction is how an unbound field becomes a
//!   frozen mistake.
//! - `FeedAccount` carries no grid, so an observation page's grid is checked
//!   *within* a page by the summary algebra and bound to nothing *across*
//!   pages.  Two pages on different grids are each internally valid.  This is
//!   an obligation on a `FeedAccount` revision.
//! - The evidence buffer has no canonical address: `seeds.rs` is frozen and no
//!   seed exists for it.  Nothing is lost today — its bytes are hostile by
//!   construction and no decision depends on its identity — but it cannot be
//!   *created* on-chain until it has one.
//! - The terms artifact is still read in its own small frame at each gate
//!   step — address derivation, market binding, payout-set binding, record
//!   binding, payout derivation (plus the preset-vector load) — but the
//!   SHA-256 over its body is paid **once per transaction**, in the account
//!   plane's `accounts::read_terms`; every later read is
//!   [`clutch_solana_layout::TermsAccount::decode_unchecked`], which runs
//!   every structural check and skips only the digest recomputation, sound
//!   because the account is presented read-only and the same transaction
//!   already proved the digest over the same bytes.  The five-full-decode
//!   shape this note used to describe is what put `Resolve` past the
//!   1.4-million-unit transaction ceiling outright (SBF_BRINGUP.md,
//!   measured); no binding was skipped to fix it.
//! - The account list does **not** carry Realm and Profile, which
//!   [`super::split`] does carry at indices 1 and 2.  Both edges that lane
//!   checks against them are already implied here by
//!   [`clutch_solana_layout::TermsAccount::binds_market`], which compares the
//!   market's realm, profile, feed, and outcome count against a terms body the
//!   market's own digest commits to; two more accounts and two more decodes
//!   would add transaction weight and no fact.  The cost is that the family's
//!   account-list prefix is not uniform, which is an ABI decision for whoever
//!   freezes the schema, not a lane's.
//! - No host test can reach [`process`]: off-chain program-address derivation
//!   is not compiled into this crate (see [`crate::seeds`]), so the account
//!   plane of these three instructions is covered only by the SVM differential,
//!   which does not exercise them yet.  The host tests cover the transition and
//!   both blob codecs.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use crate::token;
use clutch_accumulator::{
    CoveragePolicy, FeedIdentity, Grid, Observation, Summary, WindowAccumulator, WindowDomain,
    WindowResult, IDENTITY_BYTES,
};
use clutch_kernel::PayoutSet;
use clutch_kernel::{
    BasisMode, MarketState, PayoutVector, Phase, Position, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES,
};
use clutch_solana_layout::{
    account_len, CodecError, FeedAccount, Hash32, HoardAccount, Intent, MarketAccount,
    PayoutVectorBytes, PositionAccount, ResolutionAccount, SupplyLedgerAccount, TermsAccount,
    MAX_OUTCOMES,
};
use clutch_solana_reference::{
    derive_payout, Action, Error as ReferenceError, ExternalAccount, KernelAccount, ReplayAccount,
    Request, ResolutionRefusal, ResolutionTerms, WindowError, EXTERNAL_ACCOUNT_LEN,
    KERNEL_ACCOUNT_LEN, MAX_OBSERVATIONS, MAX_WINDOW_EVIDENCE_LEN, OBSERVATION_RECORD_BYTES,
    REPLAY_ACCOUNT_LEN, WINDOW_EVIDENCE_HEADER_BYTES,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::instructions::split;

/// The gate's result type: the reference adapter's own refusal vocabulary.
type Gate<T> = core::result::Result<T, ReferenceError>;

/// Borrow one account's data mutably, or refuse.
///
/// A macro rather than a function because `AccountInfo` is invariant in its
/// lifetime, so a helper taking `&[AccountInfo]` would force the slice and the
/// account data to share one lifetime.
macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

/* ------------------------------------------------------------------------ */
/* Wire formats — PROPOSED                                                   */
/* ------------------------------------------------------------------------ */

/* These four constants mirror private constants of `clutch_solana_reference`.
 * They are re-declared rather than imported because that crate does not export
 * them, and the agreement is *pinned by test*: the host differential encodes a
 * window-evidence blob with the values below and feeds it to
 * `apply_with_evidence`, which folds it with the reference's own copies.  A
 * drift would turn that test red rather than silently produce two formats. */
const WINDOW_EVIDENCE_TAG: u8 = 0x45;
const REFERENCE_VERSION: u8 = 1;
const OBSERVATION_ACCEPTED: u8 = 1;
const OBSERVATION_MISSING: u8 = 0;

/// **PROPOSED** discriminator of the resolution evidence buffer.
pub const EVIDENCE_BUFFER_TAG: u8 = 0x47;
/// **PROPOSED** discriminator of a feed observation page.
pub const FEED_PAGE_TAG: u8 = 0x48;
/// **PROPOSED** schema version of both buffer objects.
pub const BUFFER_VERSION: u8 = 1;

/// **PROPOSED** fixed header bytes of the resolution evidence buffer.
///
/// ```text
///   0   u8    tag = EVIDENCE_BUFFER_TAG
///   1   u8    version = BUFFER_VERSION
///   2   [32]  declared window identity; refused when zero
///  34   u16   payload length, little-endian
///  36   ..    exactly `payload` bytes of the clutch_solana_reference
///             window-evidence blob (its own tag 0x45); zero length for a
///             redemption, which never re-derives a payout
///  36+n ..    every remaining byte of the account must be zero
/// ```
///
/// The payload is byte-identical to what the offline reference folds, which is
/// what lets one encoder feed both implementations in the differential.  The
/// wrapper exists for exactly two reasons: an account has a fixed length while
/// a blob does not, and the window identity has nowhere else to ride — the
/// frozen `Action::Resolve` envelope carries one `u8`.
pub const EVIDENCE_BUFFER_HEADER_BYTES: usize = 2 + 32 + 2;

/// **PROPOSED** largest resolution evidence buffer this program folds.
pub const MAX_EVIDENCE_BUFFER_LEN: usize = EVIDENCE_BUFFER_HEADER_BYTES + MAX_WINDOW_EVIDENCE_LEN;

/// **PROPOSED** fixed header bytes of a feed observation page.
///
/// ```text
///   0   u8    tag = FEED_PAGE_TAG
///   1   u8    version = BUFFER_VERSION
///   2   [32]  feed identity; must equal the feed head and the intent
///  34   u32   grid family id
///  38   u16   grid version
///  40   u64   grid bucket seconds
///  48   u64   first bucket, inclusive
///  56   u64   last bucket, exclusive
///  64   u16   record count; must equal the declared span
///  66   ..    `count` records of OBSERVATION_RECORD_BYTES, in the same
///             encoding the reference uses: kind u8, bucket u64, low u128,
///             high u128, all little-endian
///  66+n ..    every remaining byte of the account must be zero
/// ```
///
/// The record encoding is deliberately the reference's, so one encoder serves a
/// feed page and a window-evidence blob and the two can never drift into two
/// meanings of "an observation".
pub const FEED_PAGE_HEADER_BYTES: usize = 2 + 32 + 4 + 2 + 8 + 8 + 8 + 2;

/// **PROPOSED** largest observation count in one feed page.
///
/// A bound, not a measurement: the fold is streaming, so the page costs one
/// [`Summary`] of stack whatever this is, and nothing here has measured the
/// compute or transaction-size envelope (obligation 10).
pub const MAX_FEED_PAGE_RECORDS: usize = 64;

/// **PROPOSED** largest feed observation page this program folds.
pub const MAX_FEED_PAGE_LEN: usize =
    FEED_PAGE_HEADER_BYTES + (MAX_FEED_PAGE_RECORDS * OBSERVATION_RECORD_BYTES);

/* ------------------------------------------------------------------------ */
/* Account lists                                                             */
/* ------------------------------------------------------------------------ */

/// Exact number of accounts `Resolve` accepts.
///
/// `Resolve` is token-free — `TOKEN2022_PLAN.md` §3.2's table gives it no CPI —
/// so its plane is unchanged.  `RedeemInternal` pays collateral out and takes
/// [`REDEEM_ACCOUNT_COUNT`].
pub const EVIDENCE_ACCOUNT_COUNT: usize = 12;
/// Exact number of accounts `FeedAdvance` accepts.
pub const FEED_ADVANCE_ACCOUNT_COUNT: usize = 3;

/// Authenticated actor.
pub const IX_ACTOR: usize = 0;
/// Market account.
pub const IX_MARKET: usize = 1;
/// Hoard collateral account.
pub const IX_HOARD: usize = 2;
/// Owner position account.
pub const IX_POSITION: usize = 3;
/// Reference-only kernel-aggregate account.
pub const IX_KERNEL: usize = 4;
/// Reference-only external-shadow account.
pub const IX_EXTERNAL: usize = 5;
/// Reference-only replay-sequence account.
pub const IX_REPLAY: usize = 6;
/// Market-wide supply-ledger account.
pub const IX_SUPPLY: usize = 7;
/// Immutable terms account.
pub const IX_TERMS: usize = 8;
/// Resolution-record account.
pub const IX_RESOLUTION: usize = 9;
/// Feed-head account (read-only).
pub const IX_FEED: usize = 10;
/// Caller-supplied evidence buffer (read-only, hostile).
pub const IX_BUFFER: usize = 11;

/* --------------------------------------------------------------------- */
/* `RedeemInternal`'s collateral leg, mandatory                            */
/* --------------------------------------------------------------------- */

/// Exact number of accounts `RedeemInternal` accepts.
///
/// The twelve evidence accounts, plus the Profile the 266 policy bytes are
/// bound to, the token program, the policy, the collateral mint, the
/// redeemer's own collateral account, the Hoard's signing authority, and the
/// Hoard token account.
///
/// The Profile is in the list for the same reason it is in the seam plane's:
/// the collateral mint's identity lives only in the Realm's frozen policy, and
/// the only thing that binds those 266 bytes to *this* Realm is the Profile's
/// digest.  The evidence plane never needed a Profile before because nothing
/// in it named an asset.
pub const REDEEM_ACCOUNT_COUNT: usize = 19;

/// Profile identity account (read-only).
pub const IX_PROFILE: usize = 12;
/// The pinned Token-2022 program (read-only, executable).
pub const IX_TOKEN_PROGRAM: usize = 13;
/// The Realm's 266-byte collateral policy (read-only).
pub const IX_POLICY: usize = 14;
/// The collateral mint the Realm's policy names (read-only).
pub const IX_COLLATERAL_MINT: usize = 15;
/// The redeemer's own Token-2022 collateral account (writable).
pub const IX_ACTOR_TOKEN: usize = 16;
/// The Hoard's signing authority; holds no data and is never written.
pub const IX_HOARD_AUTHORITY: usize = 17;
/// The Hoard's Token-2022 collateral account (writable).
pub const IX_HOARD_TOKEN: usize = 18;

/// Where this plane puts the collateral accounts the seam plane also carries.
const REDEEM_COLLATERAL_ROLES: split::CollateralRoles = split::CollateralRoles {
    actor: IX_ACTOR,
    profile: IX_PROFILE,
    policy: IX_POLICY,
    mint: IX_COLLATERAL_MINT,
    actor_token: IX_ACTOR_TOKEN,
    authority: IX_HOARD_AUTHORITY,
    hoard_token: IX_HOARD_TOKEN,
};

/// Feed-head account in the `FeedAdvance` list.
pub const IX_ADVANCE_FEED: usize = 1;
/// Caller-supplied observation page in the `FeedAdvance` list.
pub const IX_ADVANCE_BUFFER: usize = 2;

/// Program-owned roles of `Resolve`/`RedeemInternal` whose mutability is fixed.
///
/// The terms and resolution accounts are deliberately absent: their required
/// mutability is a *gate* decision made at the reference's point in the order,
/// so their declared writability is carried into the transition as a fact
/// rather than refused here.
const EVIDENCE_STATE_ROLES: [StateRole; 8] = [
    StateRole::writable(IX_MARKET, account_len::MARKET),
    StateRole::writable(IX_HOARD, account_len::HOARD),
    StateRole::writable(IX_POSITION, account_len::POSITION),
    StateRole::writable(IX_KERNEL, KERNEL_ACCOUNT_LEN),
    StateRole::writable(IX_EXTERNAL, EXTERNAL_ACCOUNT_LEN),
    StateRole::writable(IX_REPLAY, REPLAY_ACCOUNT_LEN),
    StateRole::writable(IX_SUPPLY, account_len::SUPPLY_LEDGER),
    StateRole::read_only(IX_FEED, account_len::FEED),
];

/// Program-owned roles of `FeedAdvance`.
const ADVANCE_STATE_ROLES: [StateRole; 1] =
    [StateRole::writable(IX_ADVANCE_FEED, account_len::FEED)];

/// A program-owned role whose declared writability the caller does not fix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenRole {
    index: usize,
    len: usize,
}

/// Ownership, executable bit, and exact length, without a writability rule.
fn validate_open_roles(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    roles: &[OpenRole],
) -> Outcome<()> {
    for role in roles {
        let account = &accounts[role.index];
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.executable, ClutchError::ExecutableAccount)?;
    }
    for role in roles {
        require(
            accounts[role.index].data_len() == role.len,
            ClutchError::WrongDataLength,
        )?;
    }
    Ok(())
}

/// Authenticate the caller-supplied buffer account.
///
/// The buffer is the one account in this program that is *not* address-bound,
/// and that is a statement rather than an omission: its bytes are the claim,
/// not the state, so binding its identity would suggest the bytes are trusted.
/// It is still required to be program-owned, non-executable, and read-only —
/// a writable scratch account is an over-permission a runtime would honour.
fn validate_buffer_role(
    program_id: &Pubkey,
    account: &AccountInfo,
    max_len: usize,
    min_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        account.data_len() >= min_len && account.data_len() <= max_len,
        ClutchError::WrongDataLength,
    )
}

/* ------------------------------------------------------------------------ */
/* Hostile byte reading                                                      */
/* ------------------------------------------------------------------------ */

/// A bounds-checked forward cursor over one caller-supplied blob.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8], at: usize) -> Self {
        Self { bytes, at }
    }

    fn raw<const N: usize>(&mut self) -> Gate<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(ReferenceError::WrongLength)?;
        if end > self.bytes.len() {
            return Err(ReferenceError::WrongLength);
        }
        let mut out = [0; N];
        out.copy_from_slice(&self.bytes[self.at..end]);
        self.at = end;
        Ok(out)
    }

    fn u8(&mut self) -> Gate<u8> {
        Ok(self.raw::<1>()?[0])
    }

    fn u16(&mut self) -> Gate<u16> {
        Ok(u16::from_le_bytes(self.raw::<2>()?))
    }

    fn u32(&mut self) -> Gate<u32> {
        Ok(u32::from_le_bytes(self.raw::<4>()?))
    }

    fn u64(&mut self) -> Gate<u64> {
        Ok(u64::from_le_bytes(self.raw::<8>()?))
    }

    fn u128(&mut self) -> Gate<u128> {
        Ok(u128::from_le_bytes(self.raw::<16>()?))
    }

    fn hash(&mut self) -> Gate<Hash32> {
        Ok(Hash32::from_bytes(self.raw::<32>()?))
    }
}

/// Refuse a buffer whose declared payload does not run to canonical zeros.
///
/// An account is a fixed length and a blob is not, so the tail is where a
/// second, unread message would hide.  Requiring it zero is what makes the
/// declared length the whole content.
fn require_zero_tail(bytes: &[u8], from: usize) -> Gate<()> {
    let mut index = from;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return Err(ReferenceError::NonCanonical);
        }
        index += 1;
    }
    Ok(())
}

/// The declared window identity and window-evidence payload of one buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EvidenceBuffer<'a> {
    window_id: Hash32,
    window: &'a [u8],
}

/// Decode the resolution evidence buffer.
///
/// The window identity is refused when zero here rather than at its use, so
/// that a redemption — which folds nothing — still cannot present an absent
/// identity.
fn read_evidence_buffer(bytes: &[u8]) -> Gate<EvidenceBuffer<'_>> {
    if bytes.len() < EVIDENCE_BUFFER_HEADER_BYTES || bytes.len() > MAX_EVIDENCE_BUFFER_LEN {
        return Err(ReferenceError::WrongLength);
    }
    if bytes[0] != EVIDENCE_BUFFER_TAG {
        return Err(ReferenceError::WrongTag);
    }
    if bytes[1] != BUFFER_VERSION {
        return Err(ReferenceError::WrongVersion);
    }
    let mut cursor = Cursor::new(bytes, 2);
    let window_id = cursor.hash()?;
    let payload = usize::from(cursor.u16()?);
    if payload > MAX_WINDOW_EVIDENCE_LEN {
        return Err(ReferenceError::WrongLength);
    }
    let end = EVIDENCE_BUFFER_HEADER_BYTES
        .checked_add(payload)
        .ok_or(ReferenceError::WrongLength)?;
    if end > bytes.len() {
        return Err(ReferenceError::WrongLength);
    }
    require_zero_tail(bytes, end)?;
    Ok(EvidenceBuffer {
        window_id,
        window: &bytes[EVIDENCE_BUFFER_HEADER_BYTES..end],
    })
}

/// Fold a caller-supplied window-evidence blob into a sealed [`WindowResult`].
///
/// The port of `clutch_solana_reference`'s private `fold_window_evidence`, in
/// its own frame.  The blob declares its own domain and the fold drives the
/// accumulator's `Open -> Mature -> Sealed` machine over its records: maturity,
/// completeness, contiguity, and the seal are consequences of the records and
/// the witnessed feed cursor, never assertions on the wire.  Checking the
/// declared domain against the market's expected one is deliberately *later*,
/// in [`derive_payout`], so a wrong feed, window, maturity, generation, grid, or
/// coverage policy is reported as the field that differed.
#[inline(never)]
fn fold_window_evidence(bytes: &[u8], feed_cursor: u64) -> Gate<WindowResult> {
    if bytes.len() < WINDOW_EVIDENCE_HEADER_BYTES || bytes.len() > MAX_WINDOW_EVIDENCE_LEN {
        return Err(ReferenceError::WrongLength);
    }
    let count = usize::from(u16::from_le_bytes([
        bytes[WINDOW_EVIDENCE_HEADER_BYTES - 2],
        bytes[WINDOW_EVIDENCE_HEADER_BYTES - 1],
    ]));
    if count > MAX_OBSERVATIONS {
        return Err(ReferenceError::WrongLength);
    }
    let expected = WINDOW_EVIDENCE_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(OBSERVATION_RECORD_BYTES)
                .ok_or(ReferenceError::WrongLength)?,
        )
        .ok_or(ReferenceError::WrongLength)?;
    if bytes.len() != expected {
        return Err(ReferenceError::WrongLength);
    }
    if bytes[0] != WINDOW_EVIDENCE_TAG {
        return Err(ReferenceError::WrongTag);
    }
    if bytes[1] != REFERENCE_VERSION {
        return Err(ReferenceError::WrongVersion);
    }
    let mut cursor = Cursor::new(bytes, 2);
    let source_adapter_id = cursor.raw::<IDENTITY_BYTES>()?;
    let feed_spec_id = cursor.raw::<IDENTITY_BYTES>()?;
    let source_version = cursor.u32()?;
    let evaluator_version = cursor.u32()?;
    let grid_family_id = cursor.u32()?;
    let grid_version = cursor.u16()?;
    let bucket_seconds = cursor.u64()?;
    let start_bucket = cursor.u64()?;
    let end_bucket_exclusive = cursor.u64()?;
    let maturity_bucket_exclusive = cursor.u64()?;
    let generation = cursor.u64()?;
    let coverage_policy_id = cursor.u16()?;
    let coverage_policy_parameter = cursor.u64()?;
    let declared_count = cursor.u16()?;
    if usize::from(declared_count) != count {
        return Err(ReferenceError::WrongLength);
    }
    let feed = FeedIdentity::new(
        source_adapter_id,
        feed_spec_id,
        source_version,
        evaluator_version,
    )?;
    let grid = Grid::new(grid_family_id, grid_version, bucket_seconds)
        .map_err(|error| ReferenceError::Window(WindowError::Summary(error)))?;
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
    let mut index = 0_usize;
    while index < count {
        window.observe(read_observation(&mut cursor)?)?;
        index += 1;
    }
    window.witness_feed_cursor(feed_cursor)?;
    window.seal()?;
    Ok(window.result()?)
}

/// Read one observation record in the reference's encoding.
///
/// An explicit gap may not also carry a value: two encodings of one bucket is
/// exactly the kind of non-canonical byte a settlement input must not have.
fn read_observation(cursor: &mut Cursor<'_>) -> Gate<Observation> {
    let kind = cursor.u8()?;
    let bucket = cursor.u64()?;
    let low = cursor.u128()?;
    let high = cursor.u128()?;
    match kind {
        OBSERVATION_MISSING => {
            if low != 0 || high != 0 {
                return Err(ReferenceError::NonCanonical);
            }
            Ok(Observation::missing(bucket))
        }
        OBSERVATION_ACCEPTED => Ok(Observation::accepted(bucket, low, high)),
        _ => Err(ReferenceError::NonCanonical),
    }
}

/// One decoded feed observation page.
#[derive(Clone, Copy, Debug)]
struct FeedPage<'a> {
    feed: Hash32,
    grid: Grid,
    start_bucket: u64,
    end_bucket_exclusive: u64,
    records: &'a [u8],
    count: usize,
}

/// Decode a feed observation page.
fn read_feed_page(bytes: &[u8]) -> Gate<FeedPage<'_>> {
    if bytes.len() < FEED_PAGE_HEADER_BYTES || bytes.len() > MAX_FEED_PAGE_LEN {
        return Err(ReferenceError::WrongLength);
    }
    if bytes[0] != FEED_PAGE_TAG {
        return Err(ReferenceError::WrongTag);
    }
    if bytes[1] != BUFFER_VERSION {
        return Err(ReferenceError::WrongVersion);
    }
    let mut cursor = Cursor::new(bytes, 2);
    let feed = cursor.hash()?;
    let grid_family_id = cursor.u32()?;
    let grid_version = cursor.u16()?;
    let bucket_seconds = cursor.u64()?;
    let start_bucket = cursor.u64()?;
    let end_bucket_exclusive = cursor.u64()?;
    let count = usize::from(cursor.u16()?);
    if feed == Hash32::ZERO {
        return Err(ReferenceError::Layout(CodecError::ZeroIdentity));
    }
    if count == 0 || count > MAX_FEED_PAGE_RECORDS {
        return Err(ReferenceError::WrongLength);
    }
    let span = end_bucket_exclusive
        .checked_sub(start_bucket)
        .ok_or(ReferenceError::Window(WindowError::InvalidRange))?;
    if span == 0 {
        return Err(ReferenceError::Window(WindowError::InvalidRange));
    }
    /* Every bucket of the declared span carries exactly one record — accepted
     * or explicitly missing.  With the fold's contiguity that makes a gap and
     * an overlap unrepresentable rather than merely refused. */
    if span != count as u64 {
        return Err(ReferenceError::WrongLength);
    }
    let end = FEED_PAGE_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(OBSERVATION_RECORD_BYTES)
                .ok_or(ReferenceError::WrongLength)?,
        )
        .ok_or(ReferenceError::WrongLength)?;
    if end > bytes.len() {
        return Err(ReferenceError::WrongLength);
    }
    require_zero_tail(bytes, end)?;
    let grid = Grid::new(grid_family_id, grid_version, bucket_seconds)
        .map_err(|error| ReferenceError::Window(WindowError::Summary(error)))?;
    Ok(FeedPage {
        feed,
        grid,
        start_bucket,
        end_bucket_exclusive,
        records: &bytes[FEED_PAGE_HEADER_BYTES..end],
        count,
    })
}

/// Fold one observation page into a summary, one bucket at a time.
///
/// The accumulator owns the algebra: [`Summary::append`] refuses a bucket that
/// is not the next one, a duplicate, a reordering, a reversed or oversized
/// interval, and a grid change, each with its own named reason.  This function
/// owns only the hostile bytes and the check that the fold landed on exactly
/// the declared range.
#[inline(never)]
fn fold_feed_page(page: &FeedPage<'_>) -> Gate<Summary> {
    let mut cursor = Cursor::new(page.records, 0);
    let mut summary = Summary::empty(page.grid);
    let mut index = 0_usize;
    while index < page.count {
        let observation = read_observation(&mut cursor)?;
        summary = summary
            .append(observation)
            .map_err(|error| ReferenceError::Window(WindowError::Summary(error)))?;
        index += 1;
    }
    if summary.start_bucket() != Some(page.start_bucket)
        || summary.end_bucket_exclusive() != Some(page.end_bucket_exclusive)
    {
        return Err(ReferenceError::Window(WindowError::NonContiguous));
    }
    Ok(summary)
}

/* ------------------------------------------------------------------------ */
/* Small decoded heads                                                       */
/* ------------------------------------------------------------------------ */

/// The market facts the gate reads, without the 512-byte outcome table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MarketHead {
    market: Hash32,
    realm: Hash32,
    feed: Hash32,
    outcome_count: u8,
    lifecycle: u8,
    stored_bump: u8,
    hoard_bump: u8,
}

#[inline(never)]
fn market_head(bytes: &[u8]) -> Gate<MarketHead> {
    let value = MarketAccount::decode(bytes)?;
    Ok(MarketHead {
        market: value.market,
        realm: value.realm,
        feed: value.feed,
        outcome_count: value.outcome_count,
        lifecycle: value.lifecycle,
        stored_bump: value.stored_bump,
        hoard_bump: value.hoard_bump,
    })
}

/// The kernel-aggregate facts the gate reads, without the 1 KiB payout set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelHead {
    market: Hash32,
    phase: u8,
    resolved_payout: u8,
    payout_outcomes: u8,
    total_supply: [u64; MAX_OUTCOMES],
}

#[inline(never)]
fn kernel_head(bytes: &[u8]) -> Gate<KernelHead> {
    let value = KernelAccount::decode(bytes)?;
    Ok(KernelHead {
        market: value.market,
        phase: value.phase,
        resolved_payout: value.resolved_payout,
        payout_outcomes: value.payouts.outcomes,
        total_supply: value.total_supply,
    })
}

/// The immutable-terms facts the gate reads, without the payout vectors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TermsHead {
    terms: Hash32,
    feed: Hash32,
    payout_count: u8,
}

/// The resolution-record facts the gate reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecordHead {
    window: Hash32,
    payout_index: u8,
    resolved: bool,
}

/// The sealed-window facts a resolve records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SealedFacts {
    payout_index: u8,
    sealed_cursor: u64,
    end_bucket_exclusive: u64,
    repair_generation: u64,
}

/// The kernel effect of one transition, without the payout set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelStep {
    phase: u8,
    resolved_payout: u8,
    total_supply: [u64; MAX_OUTCOMES],
    collateral: u64,
}

/* ------------------------------------------------------------------------ */
/* Gate steps, each in its own frame                                         */
/* ------------------------------------------------------------------------ */

/* Frame discipline, measured rather than assumed.  A decoded `TermsAccount` is
 * about 1.6 KiB (v3), a `MarketAccount` about 0.75 KiB, a `KernelAccount`
 * about 1.25 KiB, and a full `TermsAccount::validate` builds a 1.6 KiB body
 * buffer to recompute its own digest (in its own `#[inline(never)]` frame
 * since the v3 revision; the gate's own loads skip the recomputation
 * entirely, see `load_terms`).  Any two of those in one frame, plus an inlined
 * decode temporary, overflows the SBF 4 KiB frame -- `cargo build-sbf`
 * reported exactly that for the first cut of the four functions below, with
 * estimates from 4288 to 6016 bytes.  The wrappers here exist so that each
 * decode temporary and each validation buffer lives in its own frame and only
 * the decoded values cross back.  They look like noise and are not: deleting an
 * `#[inline(never)]` here is undefined behaviour on-chain, and the SBF build is
 * what says so. */

/* The placeholders exist only so that a decoded account can be *loaded into*
 * a caller's slot instead of *returned into* one.  A returned
 * `Result<TermsAccount, _>` costs the caller two 1.3 KiB slots -- the return
 * temporary and the binding -- and two of those plus a validation buffer is
 * how the first cut of this file reached a 6016-byte frame.  Every byte here
 * is overwritten before it is read. */
const ZERO_MARKET: MarketAccount = MarketAccount {
    market: Hash32::ZERO,
    realm: Hash32::ZERO,
    profile: Hash32::ZERO,
    terms: Hash32::ZERO,
    outcome_count: 0,
    lifecycle: 0,
    stored_bump: 0,
    hoard_bump: 0,
    outcomes: [Hash32::ZERO; MAX_OUTCOMES],
    feed: Hash32::ZERO,
    collateral_cap: 0,
    created_slot: 0,
    reserved: Hash32::ZERO,
};

const ZERO_TERMS: TermsAccount = TermsAccount {
    terms: Hash32::ZERO,
    realm: Hash32::ZERO,
    profile: Hash32::ZERO,
    feed: Hash32::ZERO,
    price_grid: Hash32::ZERO,
    outcome_count: 0,
    payout_count: 0,
    payouts: [PayoutVectorBytes::ZERO; clutch_kernel::MAX_PAYOUTS],
    grid_family_id: 0,
    grid_version: 0,
    bucket_seconds: 0,
    expected_start_bucket: 0,
    expected_end_bucket_exclusive: 0,
    maturity_horizon_buckets: 0,
    coverage_policy_id: 0,
    repair_policy_id: 0,
    failure_policy_id: 0,
    statistic_id: 0,
    ambiguity_policy_id: 0,
    edge_policy_id: 0,
    basis_degree: 0,
    knot_count: 0,
    uniform_log2_spacing: 0,
    failure_payout_index: 0,
    coverage_policy_parameter: 0,
    repair_generation: 0,
    source_version: 0,
    evaluator_version: 0,
    source_adapter_id: Hash32::ZERO,
    payout_map: [0; MAX_OUTCOMES],
    knots: [0; clutch_solana_layout::MAX_KNOTS],
    collateral_cap: 0,
    stored_bump: 0,
    flags: 0,
};

const ZERO_KERNEL: KernelAccount = KernelAccount {
    market: Hash32::ZERO,
    phase: 0,
    resolved_payout: 0,
    payouts: PayoutSet::EMPTY,
    total_supply: [0; MAX_OUTCOMES],
};

const ZERO_RESOLUTION: ResolutionAccount = ResolutionAccount {
    market: Hash32::ZERO,
    terms: Hash32::ZERO,
    feed: Hash32::ZERO,
    window: Hash32::ZERO,
    feed_cursor: 0,
    sealed_end_bucket_exclusive: 0,
    repair_generation: 0,
    resolved_slot: 0,
    payout_index: 0,
    stored_bump: 0,
    flags: 0,
};

/* `decode_unchecked`: every call site below runs after [`evidence_gated`]'s
 * address plane already paid `accounts::read_terms` — a FULL decode of the
 * same bytes, self-certifying digest included — and the terms account is
 * presented read-only (step 7 refuses a writable presentation before any of
 * these loads), so the runtime forbids the bytes from moving between the two
 * reads.  Re-hashing 1.6 KiB per load is what put `Resolve` over the compute
 * ceiling (SBF_BRINGUP.md, measured); every structural check still runs on
 * every load, and the one digest recomputation still happens on every
 * transaction, in the account plane. */
#[inline(never)]
fn load_terms(bytes: &[u8], out: &mut TermsAccount) -> Gate<()> {
    TermsAccount::decode_unchecked_into(bytes, out)?;
    Ok(())
}

#[inline(never)]
fn load_market(bytes: &[u8], out: &mut MarketAccount) -> Gate<()> {
    *out = MarketAccount::decode(bytes)?;
    Ok(())
}

#[inline(never)]
fn load_kernel(bytes: &[u8], out: &mut KernelAccount) -> Gate<()> {
    *out = KernelAccount::decode(bytes)?;
    Ok(())
}

#[inline(never)]
fn load_resolution(bytes: &[u8], out: &mut ResolutionAccount) -> Gate<()> {
    *out = ResolutionAccount::decode(bytes)?;
    Ok(())
}

/* The `_fields` variants run exactly the binding comparisons of
 * `binds_market`/`binds_terms` without re-validating operands this gate has
 * already decoded (and, for the terms, digest-checked) in this transaction;
 * the refusal classes are identical. */
#[inline(never)]
fn require_terms_binds_market(terms: &TermsAccount, market: &MarketAccount) -> Gate<()> {
    terms
        .binds_market_fields(market)
        .map_err(|_| ReferenceError::TermsBindingMismatch)
}

#[inline(never)]
fn require_record_binds_terms(record: &ResolutionAccount, terms: &TermsAccount) -> Gate<()> {
    record
        .binds_terms_fields(terms)
        .map_err(|_| ReferenceError::ResolutionBindingMismatch)
}

#[inline(never)]
fn resolution_terms_of(market: &MarketAccount, terms: &TermsAccount) -> Gate<ResolutionTerms> {
    Ok(ResolutionTerms::from_market_terms(market, terms)?)
}

/// `bind_terms`, first half: the artifact is the one the market's digest binds.
#[inline(never)]
fn terms_binds_market(market_bytes: &[u8], terms_bytes: &[u8], terms_bump: u8) -> Gate<TermsHead> {
    let mut terms = ZERO_TERMS;
    load_terms(terms_bytes, &mut terms)?;
    if terms.stored_bump != terms_bump {
        return Err(ReferenceError::WrongBump);
    }
    let mut market = ZERO_MARKET;
    load_market(market_bytes, &mut market)?;
    require_terms_binds_market(&terms, &market)?;
    Ok(TermsHead {
        terms: terms.terms,
        feed: terms.feed,
        payout_count: terms.payout_count,
    })
}

/// `bind_terms`, second half: the reference kernel's payout set is the frozen one.
///
/// The kernel account is reference-only state, so its payout set is otherwise
/// unbound caller bytes.  `MarketAccount::terms` is the digest of a body that
/// contains the payout vectors, so requiring equality is what makes "the
/// payouts this market pays" a committed fact.
#[inline(never)]
fn payout_set_binds_terms(kernel_bytes: &[u8], terms_bytes: &[u8]) -> Gate<()> {
    let mut terms = ZERO_TERMS;
    load_terms(terms_bytes, &mut terms)?;
    let mut kernel = ZERO_KERNEL;
    load_kernel(kernel_bytes, &mut kernel)?;
    require_payout_set(&kernel, &terms)
}

#[inline(never)]
fn require_payout_set(kernel: &KernelAccount, terms: &TermsAccount) -> Gate<()> {
    if kernel.payouts.count != terms.payout_count || kernel.payouts.outcomes != terms.outcome_count
    {
        return Err(ReferenceError::PayoutSetMismatch);
    }
    let mut index = 0_usize;
    while index < clutch_kernel::MAX_PAYOUTS {
        let vector = kernel.payouts.vectors[index];
        let frozen = terms.payouts[index];
        if vector.denominator != frozen.denominator || vector.weights != frozen.weights {
            return Err(ReferenceError::PayoutSetMismatch);
        }
        index += 1;
    }
    Ok(())
}

/// `bind_resolution`: the record belongs to this market and these terms.
#[inline(never)]
fn resolution_binds(
    resolution_bytes: &[u8],
    terms_bytes: &[u8],
    resolution_bump: u8,
    market: Hash32,
) -> Gate<RecordHead> {
    let mut record = ZERO_RESOLUTION;
    load_resolution(resolution_bytes, &mut record)?;
    if record.stored_bump != resolution_bump {
        return Err(ReferenceError::WrongBump);
    }
    if record.market != market {
        return Err(ReferenceError::ResolutionBindingMismatch);
    }
    let mut terms = ZERO_TERMS;
    load_terms(terms_bytes, &mut terms)?;
    require_record_binds_terms(&record, &terms)?;
    Ok(RecordHead {
        window: record.window,
        payout_index: record.payout_index,
        resolved: record.is_resolved(),
    })
}

/// The canonical [`ResolutionTerms`] of one market, in its own frame.
#[inline(never)]
fn derived_terms(market_bytes: &[u8], terms_bytes: &[u8]) -> Gate<ResolutionTerms> {
    let mut market = ZERO_MARKET;
    load_market(market_bytes, &mut market)?;
    let mut terms = ZERO_TERMS;
    load_terms(terms_bytes, &mut terms)?;
    resolution_terms_of(&market, &terms)
}

/// The frozen terms' payout vectors, loaded into a caller slot.
///
/// `derive_payout` consumes the digest-bound preset set alongside the derived
/// terms (a degree >= 1 market's payout is the preset equal to the derived
/// weight vector); the vectors are read from the same bytes every other gate
/// step reads, in their own frame.
#[inline(never)]
fn load_terms_payouts(
    terms_bytes: &[u8],
    out: &mut [PayoutVectorBytes; clutch_kernel::MAX_PAYOUTS],
) -> Gate<()> {
    let mut terms = ZERO_TERMS;
    load_terms(terms_bytes, &mut terms)?;
    *out = terms.payouts;
    Ok(())
}

/// Derive the payout the evidence selects, and refuse any other request.
///
/// The three reference steps that must stay in this order and in one frame:
/// the canonical [`ResolutionTerms`] derivation from bytes the market's digest
/// commits to, the fold of the observation records into a sealed window, and
/// the domain-checked derivation itself.
#[inline(never)]
fn derive_from_evidence(
    market_bytes: &[u8],
    terms_bytes: &[u8],
    window_bytes: &[u8],
    feed_cursor: u64,
    requested_payout: u8,
) -> Gate<SealedFacts> {
    let derived = derived_terms(market_bytes, terms_bytes)?;
    let mut payouts = [PayoutVectorBytes::ZERO; clutch_kernel::MAX_PAYOUTS];
    load_terms_payouts(terms_bytes, &mut payouts)?;
    let window = fold_window_evidence(window_bytes, feed_cursor)?;
    let payout_index = derive_payout(&derived, &payouts, &window)?;
    if payout_index != requested_payout {
        return Err(ReferenceError::PayoutIndexMismatch);
    }
    Ok(SealedFacts {
        payout_index,
        sealed_cursor: window.sealed_cursor(),
        end_bucket_exclusive: window.domain().end_bucket_exclusive(),
        repair_generation: window.domain().generation(),
    })
}

/// `kernel_market`: rebuild the pure market and run its invariants.
#[inline(never)]
fn kernel_invariants(kernel_bytes: &[u8], outcome_count: u8, collateral: u64) -> Gate<()> {
    pure_market(kernel_bytes, outcome_count, collateral).map(|_| ())
}

/// Decode the aggregate into a pure `MarketState`, in **its own frame**.
///
/// `#[inline(never)]` is load-bearing rather than decorative: a decoded
/// `KernelAccount` and a `MarketState` are over a kilobyte each and this
/// function holds both, so inlining it into `kernel_resolve` and
/// `kernel_redeem` — which hold a third — overflows the 4 KiB SBF frame.
/// `cargo-build-sbf`'s frame diagnostic is what says so, and it said so the
/// day `MarketState` grew its `resolved_vector`.
#[inline(never)]
fn pure_market(kernel_bytes: &[u8], outcome_count: u8, collateral: u64) -> Gate<MarketState> {
    let kernel = KernelAccount::decode(kernel_bytes)?;
    if usize::from(outcome_count) > KERNEL_MAX_OUTCOMES || kernel.payouts.outcomes != outcome_count
    {
        return Err(ReferenceError::MismatchedState);
    }
    let phase = match kernel.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(ReferenceError::NonCanonical),
    };
    /* `basis_mode` and `resolved_vector` are the resolution seam
     * `clutch_kernel` grew for distributional claims.  This program rebuilds
     * every `MarketState` from a stored `KernelAccount`, and that frozen
     * layout carries no mode field -- so `FinitePreset` is not a choice made
     * here, it is the only mode a decoded aggregate can name, and the kernel
     * documents it as "byte-for-byte the semantics this kernel had before mode
     * 1 existed".  A market whose terms select mode 1 needs a mode carrier in
     * the layout before this line may say anything else; until then a
     * derived-basis market is unrepresentable on chain rather than silently
     * resolved as a preset one. */
    let market = MarketState {
        outcomes: outcome_count,
        phase,
        resolved_payout: kernel.resolved_payout,
        basis_mode: BasisMode::FinitePreset,
        resolved_vector: PayoutVector::ZERO,
        collateral,
        total_supply: kernel.total_supply,
        payouts: kernel.payouts,
    };
    market.check_invariants()?;
    Ok(market)
}

fn step_of(market: &MarketState) -> KernelStep {
    KernelStep {
        phase: match market.phase {
            Phase::Active => 0,
            Phase::Resolved => 1,
        },
        resolved_payout: market.resolved_payout,
        total_supply: market.total_supply,
        collateral: market.collateral,
    }
}

/// Run `MarketState::resolve` in its own frame.
#[inline(never)]
fn kernel_resolve(
    kernel_bytes: &[u8],
    outcome_count: u8,
    collateral: u64,
    payout_index: u8,
) -> Gate<KernelStep> {
    let mut market = pure_market(kernel_bytes, outcome_count, collateral)?;
    market.resolve(payout_index)?;
    Ok(step_of(&market))
}

/// Run `MarketState::redeem_internal` in its own frame.
#[inline(never)]
fn kernel_redeem(
    kernel_bytes: &[u8],
    outcome_count: u8,
    collateral: u64,
    position: &mut Position,
    outcome: u8,
    quantity: u64,
) -> Gate<(KernelStep, u64)> {
    let mut market = pure_market(kernel_bytes, outcome_count, collateral)?;
    let paid = market.redeem_internal(position, outcome, quantity)?;
    Ok((step_of(&market), paid))
}

/// Write the kernel aggregate back, in its own frame.
#[inline(never)]
fn write_kernel(kernel_bytes: &mut [u8], step: &KernelStep) -> Gate<()> {
    let mut kernel = KernelAccount::decode(kernel_bytes)?;
    kernel.phase = step.phase;
    kernel.resolved_payout = step.resolved_payout;
    kernel.total_supply = step.total_supply;
    kernel.encode(kernel_bytes)?;
    Ok(())
}

/// Write the market lifecycle back, in its own frame.
#[inline(never)]
fn write_market_lifecycle(market_bytes: &mut [u8], lifecycle: u8) -> Gate<()> {
    let mut market = MarketAccount::decode(market_bytes)?;
    market.lifecycle = lifecycle;
    market.encode(market_bytes)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* The evidence-gated transition                                             */
/* ------------------------------------------------------------------------ */

/// The seven mutable state accounts of one evidence-gated transition.
#[derive(Debug)]
struct StateSlices<'a> {
    market: &'a mut [u8],
    hoard: &'a mut [u8],
    position: &'a mut [u8],
    kernel: &'a mut [u8],
    external: &'a mut [u8],
    replay: &'a mut [u8],
    supply: &'a mut [u8],
}

/// The canonical bumps [`process`] derived, compared at the reference's points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Bumps {
    market: u8,
    hoard: u8,
    position: u8,
    external: u8,
    replay: u8,
    supply: u8,
    terms: u8,
    resolution: u8,
}

/// The evidence plane: two account byte slices plus the declared window facts.
#[derive(Clone, Copy, Debug)]
struct EvidencePlane<'a> {
    terms: &'a [u8],
    resolution: &'a [u8],
    window: &'a [u8],
    window_id: Hash32,
    feed_cursor: u64,
    resolved_slot: u64,
    terms_writable: bool,
    resolution_writable: bool,
}

/// Runtime facts about the actor, carried so the checks stay in gate order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Actor {
    key: Hash32,
    signer: bool,
}

/// The two evidence-gated actions, already routed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GateAction {
    Resolve { payout_index: u8 },
    Redeem { outcome: u8, quantity: u64 },
}

/// Post-state this transition produced for the evidence plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GateOutput {
    /// Resolution record bytes: written by a resolve, returned unchanged by a
    /// redemption so a caller can see that redemption never edits its own
    /// authority.
    resolution: [u8; account_len::RESOLUTION],
    /// Collateral atoms paid by a redemption; zero for a resolve.
    redemption_payout: u64,
}

/// Apply one evidence-gated transition to account bytes.
///
/// This is `clutch_solana_reference::apply_with_evidence` rebuilt over the
/// account plane.  Nothing is written until every check has passed, so a
/// refusal leaves all seven slices byte-identical; the only exception is an
/// encode that fails after an earlier encode succeeded, which no reachable
/// state produces and which SVM rollback would discard anyway.
///
/// There is deliberately no evidence-free entry point.  The reference has two
/// (`apply` and `apply_with_evidence`) because it must show that the absent
/// case refuses; here the absent case is not expressible — the plane is a
/// parameter, not an `Option`, and [`process`] cannot build one without the
/// terms, resolution, and buffer accounts, which the account count requires.
#[allow(clippy::too_many_arguments)]
fn apply_evidence_transition(
    state: &mut StateSlices<'_>,
    plane: &EvidencePlane<'_>,
    bumps: &Bumps,
    actor: Actor,
    sequence: u64,
    action: GateAction,
) -> Gate<GateOutput> {
    /* 1. Decode, in the reference's order. */
    let market = market_head(state.market)?;
    let mut hoard = HoardAccount::decode(state.hoard)?;
    let mut position = PositionAccount::decode(state.position)?;
    let kernel = kernel_head(state.kernel)?;
    let mut external = ExternalAccount::decode(state.external)?;
    let mut replay = ReplayAccount::decode(state.replay)?;
    let mut supply = SupplyLedgerAccount::decode(state.supply)?;

    /* 2. `validate_links`: stored bumps, then cross-account identity. */
    if market.stored_bump != bumps.market
        || market.hoard_bump != bumps.hoard
        || hoard.stored_bump != bumps.hoard
        || position.stored_bump != bumps.position
        || external.stored_bump != bumps.external
        || replay.stored_bump != bumps.replay
        || supply.stored_bump != bumps.supply
    {
        return Err(ReferenceError::WrongBump);
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
        return Err(ReferenceError::MismatchedState);
    }

    /* 3. `validate_padding`. */
    let count = usize::from(market.outcome_count);
    let mut index = count;
    while index < MAX_OUTCOMES {
        if position.internal[index] != 0
            || kernel.total_supply[index] != 0
            || external.balances[index] != 0
        {
            return Err(ReferenceError::NonCanonical);
        }
        index += 1;
    }

    /* 4. `validate_aggregate_closure` over the pre-state. */
    check_closure(
        market.outcome_count,
        &supply,
        &kernel.total_supply,
        &position.internal,
        &external.balances,
    )?;

    /* 5. Replay. */
    if sequence != replay.sequence {
        return Err(ReferenceError::Replay);
    }
    let next_sequence = replay
        .sequence
        .checked_add(1)
        .ok_or(ReferenceError::Replay)?;

    /* 6. `kernel_market`: the pure invariants, before any evidence is read. */
    kernel_invariants(state.kernel, market.outcome_count, hoard.collateral_atoms)?;
    let mut pure_position = Position {
        internal: position.internal,
        external: external.balances,
    };

    /* 7. `validate_evidence_metadata`, the half that is a byte-level fact. */
    if plane.terms_writable {
        return Err(ReferenceError::ImmutableAccountWritable);
    }
    match action {
        GateAction::Resolve { .. } => {
            if !plane.resolution_writable {
                return Err(ReferenceError::NotWritable);
            }
        }
        GateAction::Redeem { .. } => {
            if plane.resolution_writable {
                return Err(ReferenceError::ImmutableAccountWritable);
            }
        }
    }
    if plane.window_id == Hash32::ZERO {
        return Err(ReferenceError::WindowIdentityUnavailable);
    }

    let mut record_bytes = [0; account_len::RESOLUTION];
    let (step, paid) = match action {
        GateAction::Resolve { payout_index } => {
            /* 8. Resolution is non-discretionary: the typed evidence authorizes
             * it and no key does.  A signature is still required because a
             * transaction has a fee payer, but no signer is privileged. */
            if !actor.signer {
                return Err(ReferenceError::MissingSignature);
            }

            /* 9. `resolve_from_evidence`. */
            if market.lifecycle != 0 || kernel.phase != 0 {
                return Err(ReferenceError::Resolution(
                    ResolutionRefusal::MarketNotActive,
                ));
            }
            let terms = terms_binds_market(state.market, plane.terms, bumps.terms)?;
            payout_set_binds_terms(state.kernel, plane.terms)?;
            let record = resolution_binds(
                plane.resolution,
                plane.terms,
                bumps.resolution,
                market.market,
            )?;
            if record.resolved {
                return Err(ReferenceError::ResolutionAlreadyRecorded);
            }
            let sealed = derive_from_evidence(
                state.market,
                plane.terms,
                plane.window,
                plane.feed_cursor,
                payout_index,
            )?;

            /* 10. Only now does the kernel move. */
            let step = kernel_resolve(
                state.kernel,
                market.outcome_count,
                hoard.collateral_atoms,
                payout_index,
            )?;
            ResolutionAccount {
                market: market.market,
                terms: terms.terms,
                feed: terms.feed,
                window: plane.window_id,
                feed_cursor: sealed.sealed_cursor,
                sealed_end_bucket_exclusive: sealed.end_bucket_exclusive,
                repair_generation: sealed.repair_generation,
                resolved_slot: plane.resolved_slot,
                payout_index: sealed.payout_index,
                stored_bump: bumps.resolution,
                flags: 0,
            }
            .encode(&mut record_bytes)?;
            (step, 0)
        }
        GateAction::Redeem { outcome, quantity } => {
            /* 8. Redemption is still the owner's action. */
            if !actor.signer {
                return Err(ReferenceError::MissingSignature);
            }
            if actor.key != position.owner {
                return Err(ReferenceError::UnauthorizedActor);
            }

            /* 9. `redeem_from_evidence`.  Redemption's authority is the
             * recorded resolution, not a re-fold: re-deriving a payout here
             * would create a second place a payout can be decided. */
            if !plane.window.is_empty() {
                return Err(ReferenceError::UnexpectedEvidence);
            }
            let terms = terms_binds_market(state.market, plane.terms, bumps.terms)?;
            payout_set_binds_terms(state.kernel, plane.terms)?;
            let record = resolution_binds(
                plane.resolution,
                plane.terms,
                bumps.resolution,
                market.market,
            )?;
            if !record.resolved {
                return Err(ReferenceError::ResolutionNotRecorded);
            }
            if record.window != plane.window_id {
                return Err(ReferenceError::ResolutionBindingMismatch);
            }
            if record.payout_index >= terms.payout_count {
                return Err(ReferenceError::Resolution(
                    ResolutionRefusal::PayoutIndexOutOfRange,
                ));
            }
            if market.lifecycle != 1
                || kernel.phase != 1
                || kernel.resolved_payout != record.payout_index
            {
                return Err(ReferenceError::MismatchedState);
            }

            /* 10. Only now does the kernel move. */
            let (step, paid) = kernel_redeem(
                state.kernel,
                market.outcome_count,
                hoard.collateral_atoms,
                &mut pure_position,
                outcome,
                quantity,
            )?;
            position.cash_atoms = position
                .cash_atoms
                .checked_add(paid)
                .ok_or(ReferenceError::Arithmetic)?;
            record_bytes.copy_from_slice(plane.resolution);
            (step, paid)
        }
    };

    /* 11. CLO-DELTA-V1 C3: move the ledger by exactly the applied delta. */
    hoard.collateral_atoms = step.collateral;
    let mut outcome = 0_usize;
    while outcome < count {
        supply.internal_supply[outcome] = supply.internal_supply[outcome]
            .checked_sub(position.internal[outcome])
            .ok_or(ReferenceError::AggregateClosureMismatch)?
            .checked_add(pure_position.internal[outcome])
            .ok_or(ReferenceError::Arithmetic)?;
        supply.external_supply[outcome] = supply.external_supply[outcome]
            .checked_sub(external.balances[outcome])
            .ok_or(ReferenceError::AggregateClosureMismatch)?
            .checked_add(pure_position.external[outcome])
            .ok_or(ReferenceError::Arithmetic)?;
        outcome += 1;
    }
    position.internal = pure_position.internal;
    external.balances = pure_position.external;
    replay.sequence = next_sequence;

    /* 12. C1 and C2 again, over the post-state. */
    check_closure(
        market.outcome_count,
        &supply,
        &step.total_supply,
        &position.internal,
        &external.balances,
    )?;

    /* 13. Everything below this line writes. */
    if matches!(action, GateAction::Resolve { .. }) {
        write_market_lifecycle(state.market, 1)?;
    }
    write_kernel(state.kernel, &step)?;
    hoard.encode(state.hoard)?;
    position.encode(state.position)?;
    external.encode(state.external)?;
    replay.encode(state.replay)?;
    supply.encode(state.supply)?;
    Ok(GateOutput {
        resolution: record_bytes,
        redemption_payout: paid,
    })
}

/// CLO-DELTA-V1 C1 and C2 against one presented triple.
fn check_closure(
    outcome_count: u8,
    supply: &SupplyLedgerAccount,
    total_supply: &[u64; MAX_OUTCOMES],
    internal: &[u64; MAX_OUTCOMES],
    external: &[u64; MAX_OUTCOMES],
) -> Gate<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        let aggregate = supply
            .aggregate_supply(outcome as u8)
            .map_err(|_| ReferenceError::Arithmetic)?;
        if aggregate != total_supply[outcome] {
            return Err(ReferenceError::AggregateClosureMismatch);
        }
        if internal[outcome] > supply.internal_supply[outcome]
            || external[outcome] > supply.external_supply[outcome]
        {
            return Err(ReferenceError::AggregateClosureMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* The feed-head transition                                                  */
/* ------------------------------------------------------------------------ */

/// Advance one feed head across exactly the buckets a folded page covered.
///
/// The accepted cursor is the feed's own replay guard: a page that starts
/// before it has already been accepted, and a page that starts after it would
/// leave a hole no later page can fill, because a page must begin exactly at
/// the cursor.  The two are distinguished so that a replay and a gap are not
/// one opaque failure.
fn apply_feed_advance(
    feed_bytes: &mut [u8],
    page_bytes: &[u8],
    intent_feed: Hash32,
    intent_cursor: u64,
    intent_evidence: Hash32,
    sequence: u64,
) -> Gate<()> {
    let mut feed = FeedAccount::decode(feed_bytes)?;
    if intent_feed != feed.feed {
        return Err(ReferenceError::MismatchedState);
    }
    if intent_evidence == Hash32::ZERO {
        return Err(ReferenceError::Layout(CodecError::ZeroIdentity));
    }
    /* The page index is the feed's only counter, so it is what the frozen
     * envelope's replay sequence names. */
    if sequence != feed.archive_pages {
        return Err(ReferenceError::Replay);
    }
    let page = read_feed_page(page_bytes)?;
    if page.feed != feed.feed {
        return Err(ReferenceError::MismatchedState);
    }
    if page.start_bucket < feed.cursor {
        return Err(ReferenceError::Window(WindowError::NonMonotoneCursor));
    }
    if page.start_bucket > feed.cursor {
        return Err(ReferenceError::Window(WindowError::NonContiguous));
    }
    let summary = fold_feed_page(&page)?;
    if summary.end_bucket_exclusive() != Some(intent_cursor) {
        return Err(ReferenceError::MismatchedState);
    }
    let archive_pages = feed
        .archive_pages
        .checked_add(1)
        .ok_or(ReferenceError::Arithmetic)?;

    feed.cursor = intent_cursor;
    feed.archive_pages = archive_pages;
    /* Recorded, not believed: this program owns no hash primitive, so nothing
     * here proves `evidence` is the digest of the page that was folded, and
     * nothing reads it back. */
    feed.summary = intent_evidence;
    feed.encode(feed_bytes)?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Account plane                                                             */
/* ------------------------------------------------------------------------ */

/// Validate hostile accounts and apply exactly one observation or resolution
/// transition.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::FeedAdvance {
            feed,
            cursor,
            evidence,
        }) => feed_advance(
            program_id,
            accounts,
            request.sequence,
            feed,
            cursor,
            evidence,
        ),
        Action::Resolve { payout_index } => evidence_gated(
            program_id,
            accounts,
            request.sequence,
            GateAction::Resolve { payout_index },
        ),
        Action::RedeemInternal { outcome, quantity } => evidence_gated(
            program_id,
            accounts,
            request.sequence,
            GateAction::Redeem { outcome, quantity },
        ),
        /* Every other layout intent belongs to another family module; the
         * router never sends one here, and this arm exists so that adding one
         * to the router is a compile error rather than a silent success. */
        Action::Layout(_) => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

fn feed_advance(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_feed: Hash32,
    intent_cursor: u64,
    intent_evidence: Hash32,
) -> Outcome<()> {
    require_count(accounts, FEED_ADVANCE_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &ADVANCE_STATE_ROLES)?;
    validate_buffer_role(
        program_id,
        &accounts[IX_ADVANCE_BUFFER],
        MAX_FEED_PAGE_LEN,
        FEED_PAGE_HEADER_BYTES,
    )?;

    let feed = accounts::read_feed(&accounts[IX_ADVANCE_FEED].data.borrow())?;
    expect_pda(
        accounts[IX_ADVANCE_FEED].key,
        seeds::feed_pda(program_id, &feed.feed.bytes()),
        Some(feed.stored_bump),
    )?;

    let mut feed_data = borrow_mut!(accounts[IX_ADVANCE_FEED])?;
    let page = accounts[IX_ADVANCE_BUFFER].data.borrow();
    apply_feed_advance(
        &mut feed_data,
        &page,
        intent_feed,
        intent_cursor,
        intent_evidence,
        sequence,
    )?;
    Ok(())
}

fn evidence_gated(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    action: GateAction,
) -> Outcome<()> {
    let redeems = matches!(action, GateAction::Redeem { .. });
    require_count(
        accounts,
        if redeems {
            REDEEM_ACCOUNT_COUNT
        } else {
            EVIDENCE_ACCOUNT_COUNT
        },
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &EVIDENCE_STATE_ROLES)?;
    validate_open_roles(
        program_id,
        accounts,
        &[
            OpenRole {
                index: IX_TERMS,
                len: account_len::TERMS,
            },
            OpenRole {
                index: IX_RESOLUTION,
                len: account_len::RESOLUTION,
            },
        ],
    )?;
    validate_buffer_role(
        program_id,
        &accounts[IX_BUFFER],
        MAX_EVIDENCE_BUFFER_LEN,
        EVIDENCE_BUFFER_HEADER_BYTES,
    )?;

    /* Addresses.  Caller-supplied expected keys are never accepted: every
     * address is recomputed from the frozen seed schema.  The stored bumps are
     * carried into the transition instead of being compared here, so that the
     * comparison still happens at the reference's point in the gate order. */
    let market = accounts::read_market(&accounts[IX_MARKET].data.borrow())?;
    let terms = accounts::read_terms(&accounts[IX_TERMS].data.borrow())?;
    let feed = accounts::read_feed(&accounts[IX_FEED].data.borrow())?;
    let (owner, generation) = {
        let position = PositionAccount::decode(&accounts[IX_POSITION].data.borrow())?;
        (position.owner.bytes(), position.generation)
    };
    let market_bytes = market.market.bytes();
    let realm_bytes = market.realm.bytes();

    let market_pda = seeds::market_pda(program_id, &realm_bytes, &market_bytes);
    expect_pda(accounts[IX_MARKET].key, market_pda, None)?;
    let hoard_pda = seeds::hoard_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_HOARD].key, hoard_pda, None)?;
    let position_pda = seeds::position_pda(program_id, &market_bytes, &owner);
    expect_pda(accounts[IX_POSITION].key, position_pda, None)?;
    expect_pda(
        accounts[IX_KERNEL].key,
        seeds::kernel_pda(program_id, &market_bytes),
        None,
    )?;
    let external_pda = seeds::external_pda(program_id, &market_bytes, &owner, generation);
    expect_pda(accounts[IX_EXTERNAL].key, external_pda, None)?;
    let replay_pda = seeds::replay_pda(program_id, &market_bytes, &owner, generation);
    expect_pda(accounts[IX_REPLAY].key, replay_pda, None)?;
    let supply_pda = seeds::supply_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_SUPPLY].key, supply_pda, None)?;
    let terms_pda = seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes());
    expect_pda(accounts[IX_TERMS].key, terms_pda, None)?;
    let resolution_pda = seeds::resolution_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_RESOLUTION].key, resolution_pda, None)?;
    expect_pda(
        accounts[IX_FEED].key,
        seeds::feed_pda(program_id, &feed.feed.bytes()),
        Some(feed.stored_bump),
    )?;

    /* The feed head is this program's replacement for the reference's
     * caller-supplied witnessed cursor, so it must be *this market's* feed. */
    require(feed.feed == market.feed, ClutchError::MismatchedState)?;

    /* Steps 1-3 of `TOKEN2022_PLAN.md` §3.3 for the redemption's collateral
     * leg, before anything is written.  A resolve has no leg: it moves no
     * value and takes the twelve-account plane unchanged.
     *
     * The snapshot carries a zero quantity because a redemption does not know
     * what it pays until the kernel has run; the paid amount is supplied to
     * the exact-delta check below instead. */
    let leg = if redeems {
        split::validate_token_program(&accounts[IX_TOKEN_PROGRAM])?;
        require(
            !accounts[IX_PROFILE].is_writable,
            ClutchError::UnexpectedWritable,
        )?;
        validate_open_roles(
            program_id,
            accounts,
            &[OpenRole {
                index: IX_PROFILE,
                len: account_len::PROFILE,
            }],
        )?;
        let profile = accounts::read_profile(&accounts[IX_PROFILE].data.borrow())?;
        expect_pda(
            accounts[IX_PROFILE].key,
            seeds::profile_pda(program_id, &realm_bytes, &profile.profile.bytes()),
            None,
        )?;
        require(
            profile.realm == market.realm && profile.profile == market.profile,
            ClutchError::MismatchedState,
        )?;
        let collateral_atoms =
            HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?.collateral_atoms;
        Some(split::validate_collateral_leg(
            accounts,
            &REDEEM_COLLATERAL_ROLES,
            &market_bytes,
            seeds::hoard_authority_pda(program_id, &market_bytes),
            seeds::hoard_token_pda(program_id, &market_bytes),
            collateral_atoms,
            0,
        )?)
    } else {
        None
    };

    let actor = Actor {
        key: Hash32::from_bytes(accounts[IX_ACTOR].key.to_bytes()),
        signer: accounts[IX_ACTOR].is_signer,
    };
    let bumps = Bumps {
        market: market_pda.1,
        hoard: hoard_pda.1,
        position: position_pda.1,
        external: external_pda.1,
        replay: replay_pda.1,
        supply: supply_pda.1,
        terms: terms_pda.1,
        resolution: resolution_pda.1,
    };

    let buffer = accounts[IX_BUFFER].data.borrow();
    let evidence = read_evidence_buffer(&buffer)?;
    let terms_data = accounts[IX_TERMS].data.borrow();
    let resolution_data = accounts[IX_RESOLUTION].data.borrow();
    let plane = EvidencePlane {
        terms: &terms_data,
        resolution: &resolution_data,
        window: evidence.window,
        window_id: evidence.window_id,
        feed_cursor: feed.cursor,
        /* Named gap: this program has no clock.  Zero means "no slot
         * recorded", which is why nothing may read this field as a slot. */
        resolved_slot: 0,
        terms_writable: accounts[IX_TERMS].is_writable,
        resolution_writable: accounts[IX_RESOLUTION].is_writable,
    };

    let output = {
        let mut market_data = borrow_mut!(accounts[IX_MARKET])?;
        let mut hoard_data = borrow_mut!(accounts[IX_HOARD])?;
        let mut position_data = borrow_mut!(accounts[IX_POSITION])?;
        let mut kernel_data = borrow_mut!(accounts[IX_KERNEL])?;
        let mut external_data = borrow_mut!(accounts[IX_EXTERNAL])?;
        let mut replay_data = borrow_mut!(accounts[IX_REPLAY])?;
        let mut supply_data = borrow_mut!(accounts[IX_SUPPLY])?;
        let mut state = StateSlices {
            market: &mut market_data,
            hoard: &mut hoard_data,
            position: &mut position_data,
            kernel: &mut kernel_data,
            external: &mut external_data,
            replay: &mut replay_data,
            supply: &mut supply_data,
        };
        apply_evidence_transition(&mut state, &plane, &bumps, actor, sequence, action)?
    };

    /* Every borrow the evidence plane held is released before the CPI: a live
     * `RefCell` borrow across `invoke` is a runtime failure, not a lint. */
    drop(terms_data);
    drop(buffer);
    drop(resolution_data);

    /* A redemption returns the record unchanged and never writes it; only a
     * resolve does, and only after `derive_payout` agreed with the request. */
    if matches!(action, GateAction::Resolve { .. }) {
        let mut record = borrow_mut!(accounts[IX_RESOLUTION])?;
        record.copy_from_slice(&output.resolution);
    }

    /* Steps 5-6 of §3.3, and the mirror.  The transfer runs even when the
     * kernel paid zero — a losing claim redeems for nothing — because a branch
     * that skipped it would also skip the mirror, and the one transition that
     * must never quietly leave the two collateral truths disagreeing is the
     * one that pays out. */
    if let Some(leg) = leg {
        let paid = output.redemption_payout;
        let signer: [&[u8]; 3] = [
            seeds::SEED_HOARD_AUTHORITY,
            &leg.market,
            &leg.authority_bump,
        ];
        token::transfer_checked_signed(
            &accounts[IX_TOKEN_PROGRAM],
            &accounts[IX_HOARD_TOKEN],
            &accounts[IX_COLLATERAL_MINT],
            &accounts[IX_ACTOR_TOKEN],
            &accounts[IX_HOARD_AUTHORITY],
            paid,
            leg.decimals,
            &signer,
        )?;
        let post_actor = token::token_amount(&accounts[IX_ACTOR_TOKEN])?;
        let post_hoard = token::token_amount(&accounts[IX_HOARD_TOKEN])?;
        token::require_exact_credit(leg.actor_amount, post_actor, paid)?;
        token::require_exact_debit(leg.hoard_amount, post_hoard, paid)?;
        let collateral_atoms =
            HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?.collateral_atoms;
        token::require_hoard_mirror(collateral_atoms, post_hoard)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_accumulator::{COVERAGE_POLICY_BOUNDED_GAPS, COVERAGE_POLICY_COMPLETE_REQUIRED};
    use clutch_kernel::{PayoutSet, PayoutVector, MAX_PAYOUTS};
    use clutch_solana_layout::{
        canonical_market_id, canonical_outcome_id, canonical_profile_hash, canonical_realm_id,
        FeedId, PayoutVectorBytes, PAYOUT_INDEX_UNRESOLVED, PROFILE_PARENT_BYTES,
    };
    use clutch_solana_reference::{
        apply, apply_with_evidence, AccountMetadata, ActorMetadata, EvidenceBindings,
        EvidenceBytes, EvidenceMetadata, ExpectedBindings, ResolutionEvidence, StateBytes,
        TransitionMetadata, TransitionOutput, MAX_REQUEST_LEN, V1_EVALUATOR_VERSION,
        V1_EXACT_GENERATION, V1_SOURCE_VERSION,
    };

    /* The oracle for `Resolve` and `RedeemInternal` is
     * `clutch_solana_reference::apply_with_evidence`, run on the *same* bytes
     * through `differential` below.  A case that only asserts what this module
     * returns is a case with no oracle, and the ones that have to be that way
     * -- the whole of `FeedAdvance`, and both buffer codecs -- say so in place.
     *
     * What no test here can reach is `process`: off-chain program-address
     * derivation is not compiled into this crate (see `crate::seeds`), so the
     * account plane of these instructions is covered only by an SVM
     * differential that does not exercise them yet. */

    const RESOLVED_SLOT: u64 = 0;
    const FEED_CURSOR: u64 = 104;
    const START_BUCKET: u64 = 100;
    const END_BUCKET: u64 = 103;
    const MATURITY_HORIZON: u64 = 4;
    const GRID_FAMILY: u32 = 7;
    const GRID_VERSION: u16 = 1;
    const BUCKET_SECONDS: u64 = 60;
    const REQUEST_TAG: u8 = 0xd1;
    const ACTION_LAYOUT: u8 = 0;
    const ACTION_RESOLVE: u8 = 1;
    const ACTION_REDEEM_INTERNAL: u8 = 2;

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    fn parent_profile_bytes(child_digest: Hash32) -> [u8; PROFILE_PARENT_BYTES] {
        let mut parent = [0; PROFILE_PARENT_BYTES];
        parent[..8].copy_from_slice(b"DCPROF1\0");
        parent[8..10].copy_from_slice(&1_u16.to_le_bytes());
        parent[12..14].copy_from_slice(&1_u16.to_le_bytes());
        parent[14..16].copy_from_slice(&1_u16.to_le_bytes());
        parent[16..48].copy_from_slice(&child_digest.bytes());
        parent
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
        let mut knots = [0u128; clutch_solana_layout::MAX_KNOTS];
        knots[0] = 1;
        let mut payout_map = [clutch_solana_layout::PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
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
            repair_policy_id: 1,
            failure_policy_id: 1,
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

    /// The declared window domain of one blob, field by field, so a case can
    /// name exactly which one it corrupted.
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

    fn put(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(bytes);
    }

    fn encode_records(out: &mut Vec<u8>, records: &[(u8, u64, u128, u128)]) {
        for (kind, bucket, low, high) in records {
            put(out, &[*kind]);
            put(out, &bucket.to_le_bytes());
            put(out, &low.to_le_bytes());
            put(out, &high.to_le_bytes());
        }
    }

    /// Encode one window-evidence blob in the reference's own format.
    ///
    /// This is the pin on the four private reference constants this module
    /// re-declares: the same bytes are folded by `apply_with_evidence` in every
    /// differential case, so a drift in the tag, the version, or either
    /// observation kind turns those cases red.
    fn encode_window(spec: &WindowSpec, records: &[(u8, u64, u128, u128)]) -> Vec<u8> {
        let mut out = Vec::new();
        put(&mut out, &[WINDOW_EVIDENCE_TAG, REFERENCE_VERSION]);
        put(&mut out, &spec.source_adapter_id);
        put(&mut out, &spec.feed_spec_id);
        put(&mut out, &spec.source_version.to_le_bytes());
        put(&mut out, &spec.evaluator_version.to_le_bytes());
        put(&mut out, &spec.grid_family_id.to_le_bytes());
        put(&mut out, &spec.grid_version.to_le_bytes());
        put(&mut out, &spec.bucket_seconds.to_le_bytes());
        put(&mut out, &spec.start_bucket.to_le_bytes());
        put(&mut out, &spec.end_bucket_exclusive.to_le_bytes());
        put(&mut out, &spec.maturity_bucket_exclusive.to_le_bytes());
        put(&mut out, &spec.generation.to_le_bytes());
        put(&mut out, &spec.coverage_policy_id.to_le_bytes());
        put(&mut out, &spec.coverage_policy_parameter.to_le_bytes());
        put(&mut out, &(records.len() as u16).to_le_bytes());
        assert_eq!(out.len(), WINDOW_EVIDENCE_HEADER_BYTES);
        encode_records(&mut out, records);
        out
    }

    /// Wrap one window-evidence payload in this module's evidence buffer.
    fn encode_buffer(window_id: Hash32, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        put(&mut out, &[EVIDENCE_BUFFER_TAG, BUFFER_VERSION]);
        put(&mut out, &window_id.bytes());
        put(&mut out, &(payload.len() as u16).to_le_bytes());
        assert_eq!(out.len(), EVIDENCE_BUFFER_HEADER_BYTES);
        put(&mut out, payload);
        out
    }

    fn encode_feed_page(
        feed: Hash32,
        grid: (u32, u16, u64),
        start: u64,
        end: u64,
        records: &[(u8, u64, u128, u128)],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        put(&mut out, &[FEED_PAGE_TAG, BUFFER_VERSION]);
        put(&mut out, &feed.bytes());
        put(&mut out, &grid.0.to_le_bytes());
        put(&mut out, &grid.1.to_le_bytes());
        put(&mut out, &grid.2.to_le_bytes());
        put(&mut out, &start.to_le_bytes());
        put(&mut out, &end.to_le_bytes());
        put(&mut out, &(records.len() as u16).to_le_bytes());
        assert_eq!(out.len(), FEED_PAGE_HEADER_BYTES);
        encode_records(&mut out, records);
        out
    }

    /// Buckets 100 and 101 sit in cell 0; bucket 102 terminates in cell 1.
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
        terms: [u8; account_len::TERMS],
        terms_account: TermsAccount,
        resolution: [u8; account_len::RESOLUTION],
    }

    impl Fixture {
        fn window_spec(&self) -> WindowSpec {
            WindowSpec::expected(self.terms_account.feed)
        }
    }

    fn fixture() -> Fixture {
        let profile_hash =
            canonical_profile_hash(&parent_profile_bytes(h(0xc0))).expect("exact parent preimage");
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
        Fixture {
            state,
            metadata,
            bindings,
            evidence_metadata,
            evidence_bindings,
            terms: terms_bytes,
            terms_account,
            resolution: resolution_bytes,
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

    fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
        let mut intent_bytes = [0; clutch_solana_layout::MAX_INTENT_BYTES];
        let len = intent.encode(&mut intent_bytes).unwrap();
        let mut out = Vec::new();
        put(&mut out, &[REQUEST_TAG, REFERENCE_VERSION]);
        put(&mut out, &sequence.to_le_bytes());
        put(&mut out, &[ACTION_LAYOUT]);
        put(&mut out, &(len as u16).to_le_bytes());
        put(&mut out, &intent_bytes[..len]);
        assert!(out.len() <= MAX_REQUEST_LEN);
        out
    }

    fn request_bytes(sequence: u64, action: GateAction) -> Vec<u8> {
        let mut out = Vec::new();
        put(&mut out, &[REQUEST_TAG, REFERENCE_VERSION]);
        put(&mut out, &sequence.to_le_bytes());
        match action {
            GateAction::Resolve { payout_index } => {
                put(&mut out, &[ACTION_RESOLVE, payout_index]);
            }
            GateAction::Redeem { outcome, quantity } => {
                put(&mut out, &[ACTION_REDEEM_INTERNAL, outcome]);
                put(&mut out, &quantity.to_le_bytes());
            }
        }
        out
    }

    /// Split `quantity` complete sets out of the fixture's opening position.
    ///
    /// Built by the reference adapter on purpose: the pre-state of a resolve
    /// must be a state the oracle itself produced, or the differential is
    /// comparing two implementations against a state neither one made.
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
        apply(&request, state_bytes(&f.state), &f.metadata, &f.bindings)
            .expect("the fixture splits")
    }

    /// One differential case: identical bytes into both implementations.
    #[derive(Clone, Copy, Debug)]
    struct Case<'a> {
        metadata: TransitionMetadata,
        evidence_metadata: EvidenceMetadata,
        evidence_bindings: EvidenceBindings,
        terms: &'a [u8],
        resolution: &'a [u8],
        window: &'a [u8],
        feed_cursor: u64,
        sequence: u64,
        action: GateAction,
    }

    impl<'a> Case<'a> {
        fn resolve(f: &'a Fixture, window: &'a [u8]) -> Self {
            Self {
                metadata: f.metadata,
                evidence_metadata: f.evidence_metadata,
                evidence_bindings: f.evidence_bindings,
                terms: &f.terms,
                resolution: &f.resolution,
                window,
                feed_cursor: FEED_CURSOR,
                sequence: 1,
                action: GateAction::Resolve { payout_index: 1 },
            }
        }

        /// A redemption presents a read-only record and folds nothing.
        fn redeem(f: &'a Fixture, record: &'a [u8], sequence: u64, quantity: u64) -> Self {
            let mut evidence_metadata = f.evidence_metadata;
            evidence_metadata.resolution.writable = false;
            Self {
                metadata: f.metadata,
                evidence_metadata,
                evidence_bindings: f.evidence_bindings,
                terms: &f.terms,
                resolution: record,
                window: &[],
                feed_cursor: FEED_CURSOR,
                sequence,
                action: GateAction::Redeem {
                    outcome: 1,
                    quantity,
                },
            }
        }
    }

    /// Run this module's transition on a copy of `pre`.
    ///
    /// Also asserts the no-partial-write property: a refusal must leave all
    /// seven state slices byte-identical to the input.
    fn run_here(
        f: &Fixture,
        pre: &TransitionOutput,
        case: &Case<'_>,
    ) -> Result<TransitionOutput, ReferenceError> {
        let mut post = pre.clone();
        let plane = EvidencePlane {
            terms: case.terms,
            resolution: case.resolution,
            window: case.window,
            window_id: case.evidence_bindings.window_id,
            feed_cursor: case.feed_cursor,
            resolved_slot: RESOLVED_SLOT,
            terms_writable: case.evidence_metadata.terms.writable,
            resolution_writable: case.evidence_metadata.resolution.writable,
        };
        let bumps = Bumps {
            market: f.bindings.market_bump,
            hoard: f.bindings.hoard_bump,
            position: f.bindings.position_bump,
            external: f.bindings.external_bump,
            replay: f.bindings.replay_bump,
            supply: f.bindings.supply_bump,
            terms: case.evidence_bindings.terms_bump,
            resolution: case.evidence_bindings.resolution_bump,
        };
        let actor = Actor {
            key: case.metadata.actor.key,
            signer: case.metadata.actor.signer,
        };
        let outcome = {
            let mut state = StateSlices {
                market: &mut post.market[..],
                hoard: &mut post.hoard[..],
                position: &mut post.position[..],
                kernel: &mut post.kernel[..],
                external: &mut post.external[..],
                replay: &mut post.replay[..],
                supply: &mut post.supply[..],
            };
            apply_evidence_transition(
                &mut state,
                &plane,
                &bumps,
                actor,
                case.sequence,
                case.action,
            )
        };
        match outcome {
            Ok(output) => {
                post.resolution = Some(output.resolution);
                post.redemption_payout = output.redemption_payout;
                Ok(post)
            }
            Err(error) => {
                assert_eq!(post.market, pre.market, "refusal wrote the market");
                assert_eq!(post.hoard, pre.hoard, "refusal wrote the hoard");
                assert_eq!(post.position, pre.position, "refusal wrote the position");
                assert_eq!(post.kernel, pre.kernel, "refusal wrote the kernel");
                assert_eq!(post.external, pre.external, "refusal wrote the shadow");
                assert_eq!(post.replay, pre.replay, "refusal wrote the replay");
                assert_eq!(post.supply, pre.supply, "refusal wrote the ledger");
                Err(error)
            }
        }
    }

    /// Run both implementations on identical bytes and require they agree.
    fn differential(
        f: &Fixture,
        pre: &TransitionOutput,
        case: &Case<'_>,
    ) -> Result<TransitionOutput, ReferenceError> {
        let request = request_bytes(case.sequence, case.action);
        let there = apply_with_evidence(
            &request,
            state_bytes(pre),
            &ResolutionEvidence {
                bytes: EvidenceBytes {
                    terms: case.terms,
                    resolution: case.resolution,
                    window: case.window,
                },
                metadata: case.evidence_metadata,
                bindings: case.evidence_bindings,
                feed_cursor: case.feed_cursor,
                resolved_slot: RESOLVED_SLOT,
            },
            &case.metadata,
            &f.bindings,
        );
        let here = run_here(f, pre, case);
        assert_eq!(here, there, "the account plane and the oracle disagreed");
        there
    }

    /* -------------------------------------------------------------------- */
    /* Ported evidence-gate cases                                            */
    /* -------------------------------------------------------------------- */

    #[test]
    fn evidence_absent_is_a_missing_code_path_not_a_flag() {
        /* The oracle's evidence-free entry point refuses both actions
         * unconditionally.  This module has no such entry point at all:
         * `apply_evidence_transition` takes the plane by value rather than as
         * an `Option`, and `evidence_gated` cannot build one without the terms,
         * resolution, and buffer accounts, which `EVIDENCE_ACCOUNT_COUNT`
         * requires.  A request that omits them is refused
         * `ClutchError::AccountCount` before a byte is read -- a different
         * class from the oracle's `ResolutionEvidenceUnavailable`, named here
         * rather than papered over, and still a refusal. */
        let f = fixture();
        let split = split_state(&f, 12);
        for request in [
            request_bytes(1, GateAction::Resolve { payout_index: 0 }),
            request_bytes(1, GateAction::Resolve { payout_index: 1 }),
            request_bytes(
                1,
                GateAction::Redeem {
                    outcome: 1,
                    quantity: 12,
                },
            ),
        ] {
            assert_eq!(
                apply(&request, state_bytes(&split), &f.metadata, &f.bindings),
                Err(ReferenceError::ResolutionEvidenceUnavailable)
            );
        }
        assert_eq!(EVIDENCE_ACCOUNT_COUNT, IX_BUFFER + 1);
    }

    #[test]
    fn the_numeric_projection_of_the_gate_is_allocated() {
        /* This test used to pin the opposite fact: every class below
         * collapsed onto `error.rs`'s `0x3fff` catch-all, and the collapse
         * was asserted so that widening the table would turn it red.  It did,
         * and `error.rs` now carries the exact `0x0050-0x005f` allocation the
         * module docs proposed; this pin now holds the allocation itself, so
         * a renumbering cannot pass silently.  The sub-reasons inside
         * `Window(_)` and `Resolution(_)` stay one number each on purpose:
         * they remain exactly distinguishable in the host differential, which
         * compares typed values. */
        for (class, code) in [
            (ReferenceError::Window(WindowError::NotMature), 0x0050),
            (
                ReferenceError::Resolution(ResolutionRefusal::AmbiguousInterval),
                0x0051,
            ),
            (ReferenceError::TermsBindingMismatch, 0x0052),
            (ReferenceError::PayoutSetMismatch, 0x0053),
            (ReferenceError::ResolutionBindingMismatch, 0x0054),
            (ReferenceError::ResolutionAlreadyRecorded, 0x0055),
            (ReferenceError::ResolutionNotRecorded, 0x0056),
            (ReferenceError::PayoutIndexMismatch, 0x0057),
            (ReferenceError::ImmutableAccountWritable, 0x0058),
            (ReferenceError::UnexpectedEvidence, 0x0059),
            (ReferenceError::WindowIdentityUnavailable, 0x005a),
        ] {
            assert_eq!(Refusal::Reference(class).code(), code);
        }
        /* One number per sub-reason would be a parallel truth: every window
         * reason projects onto the class code. */
        assert_eq!(
            Refusal::Reference(ReferenceError::Window(WindowError::CoverageRefused)).code(),
            0x0050
        );
        assert_eq!(
            Refusal::Reference(ReferenceError::Resolution(
                ResolutionRefusal::DerivedVectorUnrepresentable
            ))
            .code(),
            0x0051
        );
        /* The classes this module shares with the account plane still project
         * onto their own vocabularies' numbers. */
        assert_eq!(
            Refusal::Reference(ReferenceError::MissingSignature).code(),
            0x3009
        );
        assert_eq!(
            Refusal::Adapter(ClutchError::MissingSignature).code(),
            0x0002
        );
        /* And the market-initialization appends: the freeze check is one
         * number whichever vocabulary raised it. */
        assert_eq!(
            Refusal::Reference(ReferenceError::CollateralPolicyNotFrozen).code(),
            0x0041
        );
        assert_eq!(
            Refusal::Adapter(ClutchError::CollateralPolicyNotFrozen).code(),
            0x0041
        );
    }

    #[test]
    fn resolution_rejects_prefix_before_exact_window_seal() {
        let f = fixture();
        let split = split_state(&f, 12);
        let spec = f.window_spec();
        let all = winning_records();

        for prefix in 0..all.len() {
            let window = encode_window(&spec, &all[..prefix]);
            assert_eq!(
                differential(&f, &split, &Case::resolve(&f, &window)),
                Err(ReferenceError::Window(WindowError::IncompleteDomain))
            );
        }

        let window = encode_window(&spec, &all);
        for cursor in [END_BUCKET, FEED_CURSOR - 1] {
            let mut case = Case::resolve(&f, &window);
            case.feed_cursor = cursor;
            assert_eq!(
                differential(&f, &split, &case),
                Err(ReferenceError::Window(WindowError::NotMature))
            );
        }

        let gapped = encode_window(
            &spec,
            &[
                (OBSERVATION_ACCEPTED, 100, 0, 0),
                (OBSERVATION_MISSING, 101, 0, 0),
                (OBSERVATION_ACCEPTED, 102, 1, 1),
            ],
        );
        assert_eq!(
            differential(&f, &split, &Case::resolve(&f, &gapped)),
            Err(ReferenceError::Window(WindowError::CoverageRefused))
        );

        let reordered = encode_window(
            &spec,
            &[
                (OBSERVATION_ACCEPTED, 101, 0, 0),
                (OBSERVATION_ACCEPTED, 100, 0, 0),
                (OBSERVATION_ACCEPTED, 102, 1, 1),
            ],
        );
        assert_eq!(
            differential(&f, &split, &Case::resolve(&f, &reordered)),
            Err(ReferenceError::Window(WindowError::NonContiguous))
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

        for (spec, reason) in [
            (wrong_source, WindowError::MismatchedFeed),
            (wrong_evaluator, WindowError::MismatchedFeed),
            (wrong_adapter, WindowError::MismatchedFeed),
            (wrong_spec, WindowError::MismatchedFeed),
            (wrong_generation, WindowError::MismatchedGeneration),
            (wrong_grid, WindowError::MismatchedGrid),
            (wrong_maturity, WindowError::MismatchedMaturity),
            (wrong_coverage, WindowError::MismatchedCoveragePolicy),
        ] {
            let window = encode_window(&spec, &records);
            let mut case = Case::resolve(&f, &window);
            case.feed_cursor = if spec.maturity_bucket_exclusive > FEED_CURSOR {
                spec.maturity_bucket_exclusive
            } else {
                FEED_CURSOR
            };
            assert_eq!(
                differential(&f, &split, &case),
                Err(ReferenceError::Resolution(
                    ResolutionRefusal::WindowDomainMismatch(reason)
                ))
            );
        }

        let mut shifted = base;
        shifted.start_bucket = START_BUCKET + 1;
        shifted.end_bucket_exclusive = END_BUCKET + 1;
        let window = encode_window(
            &shifted,
            &[
                (OBSERVATION_ACCEPTED, 101, 0, 0),
                (OBSERVATION_ACCEPTED, 102, 0, 0),
                (OBSERVATION_ACCEPTED, 103, 1, 1),
            ],
        );
        assert_eq!(
            differential(&f, &split, &Case::resolve(&f, &window)),
            Err(ReferenceError::Resolution(
                ResolutionRefusal::WindowDomainMismatch(WindowError::WrongWindow)
            ))
        );
    }

    #[test]
    fn payout_set_is_bound_to_the_immutable_terms_artifact() {
        let f = fixture();
        let split = split_state(&f, 12);
        let window = encode_window(&f.window_spec(), &winning_records());

        let mut forged = split.clone();
        let mut kernel = KernelAccount::decode(&forged.kernel).unwrap();
        let mut weights = [0; MAX_OUTCOMES];
        weights[0] = 1;
        kernel.payouts.vectors[1] = PayoutVector::new(1, weights);
        kernel.encode(&mut forged.kernel).unwrap();
        assert_eq!(
            differential(&f, &forged, &Case::resolve(&f, &window)),
            Err(ReferenceError::PayoutSetMismatch)
        );

        let mut swapped = f.terms_account;
        swapped.payouts[1].weights[0] = 1;
        swapped.payouts[1].weights[1] = 0;
        swapped.terms = swapped.recomputed_terms_digest().unwrap();
        let mut swapped_bytes = [0; account_len::TERMS];
        swapped.encode(&mut swapped_bytes).unwrap();
        let mut case = Case::resolve(&f, &window);
        case.terms = &swapped_bytes;
        assert_eq!(
            differential(&f, &split, &case),
            Err(ReferenceError::TermsBindingMismatch)
        );

        /* A terms artifact whose digest field is simply forged to the market's
         * value fails its own re-encode check inside the frozen codec, so it
         * cannot even be presented. */
        let mut lying = f.terms_account;
        lying.payouts[1].weights[0] = 1;
        lying.payouts[1].weights[1] = 0;
        let mut lying_bytes = [0; account_len::TERMS];
        assert_eq!(
            lying.encode(&mut lying_bytes),
            Err(CodecError::NonCanonicalIdentity)
        );

        let mut case = Case::resolve(&f, &window);
        case.action = GateAction::Resolve { payout_index: 0 };
        assert_eq!(
            differential(&f, &split, &case),
            Err(ReferenceError::PayoutIndexMismatch)
        );
    }

    #[test]
    fn ambiguous_interval_and_wrong_mutability_refuse() {
        let f = fixture();
        let split = split_state(&f, 12);
        let spec = f.window_spec();

        let straddling = encode_window(
            &spec,
            &[
                (OBSERVATION_ACCEPTED, 100, 0, 0),
                (OBSERVATION_ACCEPTED, 101, 0, 0),
                (OBSERVATION_ACCEPTED, 102, 0, 1),
            ],
        );
        assert_eq!(
            differential(&f, &split, &Case::resolve(&f, &straddling)),
            Err(ReferenceError::Resolution(
                ResolutionRefusal::AmbiguousInterval
            ))
        );

        let window = encode_window(&spec, &winning_records());

        let mut case = Case::resolve(&f, &window);
        case.evidence_metadata.terms.writable = true;
        assert_eq!(
            differential(&f, &split, &case),
            Err(ReferenceError::ImmutableAccountWritable)
        );

        let mut case = Case::resolve(&f, &window);
        case.evidence_metadata.resolution.writable = false;
        assert_eq!(
            differential(&f, &split, &case),
            Err(ReferenceError::NotWritable)
        );

        let mut case = Case::resolve(&f, &window);
        case.evidence_bindings.window_id = Hash32::ZERO;
        assert_eq!(
            differential(&f, &split, &case),
            Err(ReferenceError::WindowIdentityUnavailable)
        );

        /* Evidence authorizes the transition, but a transaction still has to
         * have been submitted by somebody. */
        let mut case = Case::resolve(&f, &window);
        case.metadata.actor.signer = false;
        assert_eq!(
            differential(&f, &split, &case),
            Err(ReferenceError::MissingSignature)
        );

        /* The oracle's aliased-evidence-account case has no counterpart here:
         * aliasing is `accounts::require_distinct` in `evidence_gated`, which
         * runs before any byte is decoded and which no host test can reach. */
    }

    #[test]
    fn redemption_refuses_forged_resolved_state_and_unbound_records() {
        let f = fixture();
        let split = split_state(&f, 12);
        let window = encode_window(&f.window_spec(), &winning_records());
        let resolved = differential(&f, &split, &Case::resolve(&f, &window)).unwrap();
        let record = resolved.resolution.unwrap();

        /* Forged resolved market and kernel bytes with no evidence chain: the
         * record is still unresolved, so redemption fails. */
        let mut forged = split.clone();
        let mut market = MarketAccount::decode(&forged.market).unwrap();
        market.lifecycle = 1;
        market.encode(&mut forged.market).unwrap();
        let mut kernel = KernelAccount::decode(&forged.kernel).unwrap();
        kernel.phase = 1;
        kernel.resolved_payout = 1;
        kernel.encode(&mut forged.kernel).unwrap();
        assert_eq!(
            differential(&f, &forged, &Case::redeem(&f, &f.resolution, 1, 12)),
            Err(ReferenceError::ResolutionNotRecorded)
        );

        /* A genuine record presented against unresolved kernel state does not
         * resolve the market by being shown to it. */
        assert_eq!(
            differential(&f, &split, &Case::redeem(&f, &record, 1, 12)),
            Err(ReferenceError::MismatchedState)
        );

        let mut disagreeing = resolved.clone();
        let mut kernel = KernelAccount::decode(&disagreeing.kernel).unwrap();
        kernel.resolved_payout = 0;
        kernel.encode(&mut disagreeing.kernel).unwrap();
        assert_eq!(
            differential(&f, &disagreeing, &Case::redeem(&f, &record, 2, 12)),
            Err(ReferenceError::MismatchedState)
        );

        /* Redemption never re-derives a payout, so a window blob is refused. */
        let mut case = Case::redeem(&f, &record, 2, 12);
        case.window = &window;
        assert_eq!(
            differential(&f, &resolved, &case),
            Err(ReferenceError::UnexpectedEvidence)
        );

        let mut case = Case::redeem(&f, &record, 2, 12);
        case.evidence_bindings.window_id = h(0x66);
        assert_eq!(
            differential(&f, &resolved, &case),
            Err(ReferenceError::ResolutionBindingMismatch)
        );

        /* The market cannot resolve twice. */
        let mut case = Case::resolve(&f, &window);
        case.resolution = &record;
        case.sequence = 2;
        assert_eq!(
            differential(&f, &resolved, &case),
            Err(ReferenceError::Resolution(
                ResolutionRefusal::MarketNotActive
            ))
        );

        /* Redemption is still the owner's action. */
        let mut case = Case::redeem(&f, &record, 2, 12);
        case.metadata.actor = ActorMetadata {
            key: h(60),
            signer: true,
        };
        assert_eq!(
            differential(&f, &resolved, &case),
            Err(ReferenceError::UnauthorizedActor)
        );
    }

    #[test]
    fn the_lifecycle_vector_agrees_byte_for_byte_through_resolve_and_redeem() {
        let f = fixture();
        let split = split_state(&f, 20);
        assert_eq!(
            HoardAccount::decode(&split.hoard).unwrap().collateral_atoms,
            20
        );

        let window = encode_window(&f.window_spec(), &winning_records());
        let resolved = differential(&f, &split, &Case::resolve(&f, &window)).unwrap();

        let mut expected_market = split.market;
        expected_market[131] = 1;
        let mut expected_kernel = split.kernel;
        expected_kernel[34] = 1;
        expected_kernel[35] = 1;
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

        let redeemed = differential(
            &f,
            &resolved,
            &Case::redeem(&f, &expected_resolution, 2, 20),
        )
        .unwrap();

        let mut expected_hoard = resolved.hoard;
        expected_hoard[98..106].copy_from_slice(&0_u64.to_le_bytes());
        let mut expected_position = resolved.position;
        expected_position[82..90].copy_from_slice(&0_u64.to_le_bytes());
        expected_position[202..210].copy_from_slice(&100_u64.to_le_bytes());
        let mut expected_kernel = resolved.kernel;
        expected_kernel[46..54].copy_from_slice(&0_u64.to_le_bytes());
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

        /* Redeeming the losing outcome burns the claim for exactly zero. */
        let mut case = Case::redeem(&f, &expected_resolution, 3, 1);
        case.action = GateAction::Redeem {
            outcome: 0,
            quantity: 1,
        };
        let burned = differential(&f, &redeemed, &case).unwrap();
        assert_eq!(burned.redemption_payout, 0);
        assert_eq!(
            HoardAccount::decode(&burned.hoard)
                .unwrap()
                .collateral_atoms,
            0
        );
        assert_eq!(
            SupplyLedgerAccount::decode(&burned.supply)
                .unwrap()
                .aggregate_supply(0),
            Ok(19)
        );
    }

    #[test]
    fn window_evidence_codec_refuses_malformed_blobs() {
        let f = fixture();
        let split = split_state(&f, 12);
        let spec = f.window_spec();
        let window = encode_window(&spec, &winning_records());

        let refuse = |bytes: &[u8]| differential(&f, &split, &Case::resolve(&f, bytes));

        assert_eq!(refuse(&[]), Err(ReferenceError::WrongLength));
        assert_eq!(
            refuse(&window[..window.len() - 1]),
            Err(ReferenceError::WrongLength)
        );
        let mut wrong_tag = window.clone();
        wrong_tag[0] ^= 1;
        assert_eq!(refuse(&wrong_tag), Err(ReferenceError::WrongTag));
        let mut wrong_version = window.clone();
        wrong_version[1] = 2;
        assert_eq!(refuse(&wrong_version), Err(ReferenceError::WrongVersion));
        let mut wrong_kind = window.clone();
        wrong_kind[WINDOW_EVIDENCE_HEADER_BYTES] = 2;
        assert_eq!(refuse(&wrong_kind), Err(ReferenceError::NonCanonical));
        let mut valued_gap = window.clone();
        valued_gap[WINDOW_EVIDENCE_HEADER_BYTES + (2 * OBSERVATION_RECORD_BYTES)] =
            OBSERVATION_MISSING;
        assert_eq!(refuse(&valued_gap), Err(ReferenceError::NonCanonical));

        let mut zero_feed = spec;
        zero_feed.feed_spec_id = [0; IDENTITY_BYTES];
        assert_eq!(
            refuse(&encode_window(&zero_feed, &winning_records())),
            Err(ReferenceError::Window(WindowError::ZeroIdentity))
        );

        let mut unknown_policy = spec;
        unknown_policy.coverage_policy_id = 9;
        assert_eq!(
            refuse(&encode_window(&unknown_policy, &winning_records())),
            Err(ReferenceError::Window(WindowError::UnknownCoveragePolicy))
        );

        let mut early_maturity = spec;
        early_maturity.maturity_bucket_exclusive = END_BUCKET - 1;
        assert_eq!(
            refuse(&encode_window(&early_maturity, &winning_records())),
            Err(ReferenceError::Window(WindowError::InvalidMaturity))
        );
    }

    /* -------------------------------------------------------------------- */
    /* The evidence buffer, which has no oracle                              */
    /* -------------------------------------------------------------------- */

    #[test]
    fn the_evidence_buffer_refuses_hostile_wrappers() {
        let f = fixture();
        let window = encode_window(&f.window_spec(), &winning_records());
        let good = encode_buffer(h(77), &window);
        let read = read_evidence_buffer(&good).expect("a well-formed buffer");
        assert_eq!(read.window_id, h(77));
        assert_eq!(read.window, &window[..]);

        assert_eq!(
            read_evidence_buffer(&good[..EVIDENCE_BUFFER_HEADER_BYTES - 1]),
            Err(ReferenceError::WrongLength)
        );
        let mut wrong_tag = good.clone();
        wrong_tag[0] ^= 1;
        assert_eq!(
            read_evidence_buffer(&wrong_tag).map(|_| ()),
            Err(ReferenceError::WrongTag)
        );
        let mut wrong_version = good.clone();
        wrong_version[1] = 9;
        assert_eq!(
            read_evidence_buffer(&wrong_version).map(|_| ()),
            Err(ReferenceError::WrongVersion)
        );
        let mut over_long = good.clone();
        over_long[34..36].copy_from_slice(&((window.len() + 1) as u16).to_le_bytes());
        assert_eq!(
            read_evidence_buffer(&over_long).map(|_| ()),
            Err(ReferenceError::WrongLength)
        );

        /* An account is a fixed length and a blob is not, so a second message
         * hiding in the tail is the thing to refuse. */
        let mut padded = good.clone();
        padded.push(0);
        assert_eq!(
            read_evidence_buffer(&padded).map(|b| b.window),
            Ok(&window[..])
        );
        let mut smuggled = padded;
        let last = smuggled.len() - 1;
        smuggled[last] = 1;
        assert_eq!(
            read_evidence_buffer(&smuggled).map(|_| ()),
            Err(ReferenceError::NonCanonical)
        );

        /* A redemption presents a zero-length payload, and the declared window
         * identity still has to be there. */
        let empty = encode_buffer(h(77), &[]);
        assert_eq!(
            read_evidence_buffer(&empty).map(|b| b.window.len()),
            Ok(0_usize)
        );
    }

    /* -------------------------------------------------------------------- */
    /* FeedAdvance, whose oracle is the accumulator and the frozen codec      */
    /* -------------------------------------------------------------------- */

    fn feed_account(cursor: u64, archive_pages: u64) -> [u8; account_len::FEED] {
        let feed = FeedAccount {
            feed: h(9),
            realm: h(21),
            cursor,
            next_boundary: 4_096,
            archive_pages,
            summary: h(1),
            stored_bump: 11,
            flags: 0,
        };
        let mut bytes = [0; account_len::FEED];
        feed.encode(&mut bytes).unwrap();
        bytes
    }

    fn page(start: u64, end: u64) -> Vec<u8> {
        let records: Vec<(u8, u64, u128, u128)> = (start..end)
            .map(|bucket| (OBSERVATION_ACCEPTED, bucket, 40, 41))
            .collect();
        encode_feed_page(
            h(9),
            (GRID_FAMILY, GRID_VERSION, BUCKET_SECONDS),
            start,
            end,
            &records,
        )
    }

    #[test]
    fn feed_advance_moves_the_cursor_exactly_across_a_folded_page() {
        let mut bytes = feed_account(100, 3);
        let advanced = apply_feed_advance(&mut bytes, &page(100, 103), h(9), 103, h(0x5e), 3);
        assert_eq!(advanced, Ok(()));
        let after = FeedAccount::decode(&bytes).unwrap();
        assert_eq!(after.cursor, 103);
        assert_eq!(after.archive_pages, 4);
        assert_eq!(after.summary, h(0x5e));
        /* The one field with no defined policy anywhere is left alone. */
        assert_eq!(after.next_boundary, 4_096);

        /* An explicit gap is still a represented bucket: the feed head records
         * coverage, and a window's own coverage policy is what refuses gaps at
         * resolution time. */
        let mut bytes = feed_account(103, 4);
        let gapped = encode_feed_page(
            h(9),
            (GRID_FAMILY, GRID_VERSION, BUCKET_SECONDS),
            103,
            105,
            &[
                (OBSERVATION_MISSING, 103, 0, 0),
                (OBSERVATION_ACCEPTED, 104, 7, 7),
            ],
        );
        assert_eq!(
            apply_feed_advance(&mut bytes, &gapped, h(9), 105, h(0x5f), 4),
            Ok(())
        );
        assert_eq!(FeedAccount::decode(&bytes).unwrap().cursor, 105);
    }

    #[test]
    fn feed_advance_refuses_replays_gaps_and_a_lying_intent() {
        let good = page(100, 103);

        /* A page that starts before the accepted cursor has already been
         * accepted; one that starts after it would leave a hole. */
        let mut bytes = feed_account(101, 3);
        assert_eq!(
            apply_feed_advance(&mut bytes, &good, h(9), 103, h(0x5e), 3),
            Err(ReferenceError::Window(WindowError::NonMonotoneCursor))
        );
        let mut bytes = feed_account(99, 3);
        assert_eq!(
            apply_feed_advance(&mut bytes, &good, h(9), 103, h(0x5e), 3),
            Err(ReferenceError::Window(WindowError::NonContiguous))
        );

        /* The page index is the feed's replay guard. */
        let mut bytes = feed_account(100, 3);
        assert_eq!(
            apply_feed_advance(&mut bytes, &good, h(9), 103, h(0x5e), 2),
            Err(ReferenceError::Replay)
        );

        /* The intent may not claim a cursor the page does not reach. */
        let mut bytes = feed_account(100, 3);
        assert_eq!(
            apply_feed_advance(&mut bytes, &good, h(9), 104, h(0x5e), 3),
            Err(ReferenceError::MismatchedState)
        );

        /* Nor a feed it is not. */
        let mut bytes = feed_account(100, 3);
        assert_eq!(
            apply_feed_advance(&mut bytes, &good, h(8), 103, h(0x5e), 3),
            Err(ReferenceError::MismatchedState)
        );
        let mut bytes = feed_account(100, 3);
        let other_feed = encode_feed_page(
            h(8),
            (GRID_FAMILY, GRID_VERSION, BUCKET_SECONDS),
            100,
            103,
            &[
                (OBSERVATION_ACCEPTED, 100, 1, 1),
                (OBSERVATION_ACCEPTED, 101, 1, 1),
                (OBSERVATION_ACCEPTED, 102, 1, 1),
            ],
        );
        assert_eq!(
            apply_feed_advance(&mut bytes, &other_feed, h(9), 103, h(0x5e), 3),
            Err(ReferenceError::MismatchedState)
        );

        /* `FeedAccount::summary` refuses a zero identity, and so does this. */
        let mut bytes = feed_account(100, 3);
        assert_eq!(
            apply_feed_advance(&mut bytes, &good, h(9), 103, Hash32::ZERO, 3),
            Err(ReferenceError::Layout(CodecError::ZeroIdentity))
        );

        /* Nothing above wrote a byte. */
        assert_eq!(bytes, feed_account(100, 3));
    }

    #[test]
    fn feed_pages_refuse_malformed_and_non_contiguous_records() {
        let grid = (GRID_FAMILY, GRID_VERSION, BUCKET_SECONDS);

        /* The bucket sequence is the accumulator's business, and it names the
         * reason: a duplicate and a reordering are both non-adjacent folds. */
        let duplicated = encode_feed_page(
            h(9),
            grid,
            100,
            103,
            &[
                (OBSERVATION_ACCEPTED, 100, 1, 1),
                (OBSERVATION_ACCEPTED, 100, 1, 1),
                (OBSERVATION_ACCEPTED, 102, 1, 1),
            ],
        );
        let mut bytes = feed_account(100, 0);
        assert_eq!(
            apply_feed_advance(&mut bytes, &duplicated, h(9), 103, h(0x5e), 0),
            Err(ReferenceError::Window(WindowError::Summary(
                clutch_accumulator::SummaryError::NonAdjacent
            )))
        );

        /* A reversed interval is refused by the summary constructor. */
        let reversed = encode_feed_page(h(9), grid, 100, 101, &[(OBSERVATION_ACCEPTED, 100, 9, 1)]);
        let mut bytes = feed_account(100, 0);
        assert_eq!(
            apply_feed_advance(&mut bytes, &reversed, h(9), 101, h(0x5e), 0),
            Err(ReferenceError::Window(WindowError::Summary(
                clutch_accumulator::SummaryError::InvalidObservation
            )))
        );

        /* Header shape: tag, version, zero feed, empty range, a record count
         * that is not the declared span, and a non-zero tail. */
        let good = page(100, 103);
        let mut wrong_tag = good.clone();
        wrong_tag[0] ^= 1;
        assert_eq!(
            read_feed_page(&wrong_tag).map(|_| ()),
            Err(ReferenceError::WrongTag)
        );
        let mut wrong_version = good.clone();
        wrong_version[1] = 7;
        assert_eq!(
            read_feed_page(&wrong_version).map(|_| ()),
            Err(ReferenceError::WrongVersion)
        );
        let zero_feed = encode_feed_page(
            Hash32::ZERO,
            grid,
            100,
            101,
            &[(OBSERVATION_ACCEPTED, 100, 1, 1)],
        );
        assert_eq!(
            read_feed_page(&zero_feed).map(|_| ()),
            Err(ReferenceError::Layout(CodecError::ZeroIdentity))
        );
        let empty_range =
            encode_feed_page(h(9), grid, 100, 100, &[(OBSERVATION_ACCEPTED, 100, 1, 1)]);
        assert_eq!(
            read_feed_page(&empty_range).map(|_| ()),
            Err(ReferenceError::Window(WindowError::InvalidRange))
        );
        let short_count =
            encode_feed_page(h(9), grid, 100, 103, &[(OBSERVATION_ACCEPTED, 100, 1, 1)]);
        assert_eq!(
            read_feed_page(&short_count).map(|_| ()),
            Err(ReferenceError::WrongLength)
        );
        let mut smuggled = good.clone();
        smuggled.push(1);
        assert_eq!(
            read_feed_page(&smuggled).map(|_| ()),
            Err(ReferenceError::NonCanonical)
        );
        let mut zero_padded = good;
        zero_padded.push(0);
        assert!(read_feed_page(&zero_padded).is_ok());
    }

    #[test]
    fn a_feed_page_over_the_bound_is_refused_rather_than_truncated() {
        let records: Vec<(u8, u64, u128, u128)> = (0..=MAX_FEED_PAGE_RECORDS as u64)
            .map(|bucket| (OBSERVATION_ACCEPTED, bucket, 1, 1))
            .collect();
        let oversized = encode_feed_page(
            h(9),
            (GRID_FAMILY, GRID_VERSION, BUCKET_SECONDS),
            0,
            records.len() as u64,
            &records,
        );
        assert_eq!(
            read_feed_page(&oversized).map(|_| ()),
            Err(ReferenceError::WrongLength)
        );
    }
}
