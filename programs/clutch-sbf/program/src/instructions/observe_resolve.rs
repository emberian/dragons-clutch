//! `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal`.
//!
//! This module owns the observation and resolution plane: the feed head that
//! turns a folded observation page into an advanced, replay-guarded cursor
//! (digest-chained and signer-authorized — nothing here authenticates the
//! observation *sources*), and the
//! evidence gate that turns a sealed window into either one categorical payout
//! index (degree zero, version-two Resolution bytes) or one native B-spline
//! vector (degrees one through three, version-three Resolution bytes). Smooth
//! terms are never searched through the preset set. The native vector has one
//! persisted owner—the immutable Resolution record—and is reconstructed only
//! into an ephemeral kernel value for redemption. It
//! contains no economic logic — the payout algebra is [`clutch_kernel`], the
//! window algebra is [`clutch_accumulator`], the terms-to-payout derivation is
//! [`clutch_solana_reference::derive_payout`], and byte ownership is
//! [`clutch_solana_layout`].  What lives here is the account list, the order of
//! the checks, the hostile decoding of the two caller-supplied blobs, and the
//! write-back.
//!
//! ## Resolution, redemption, and withdrawal are distinct
//!
//! `Resolve` moves no value but carries the complete canonical outcome-mint
//! vector so direct bearer burns are synchronized before the payout freezes.
//! It has no Position or owner Replay account: immutable Terms and the sealed
//! window generation define its replay domain, while the canonical Resolution
//! account records the one market-global fact.
//!
//! `RedeemInternal` converts locked backing into owner Position cash; it is not
//! a physical payout. It takes [`REDEEM_ACCOUNT_PREFIX`] plus the mint vector
//! and collateral admission roles so the program can prove that *zero*
//! Token-2022 atoms moved and that pooled custody still covers the remaining
//! locked backing. The admission decision over those accounts is
//! [`crate::instructions::split::validate_collateral_leg`], called with this
//! own positions rather than copied — one decision procedure, two account
//! lists. [`super::cash_exit`] is the separate owner-authorized Hoard-to-wallet
//! `TransferChecked` boundary.
//!
//! ## The oracle
//!
//! `clutch_solana_reference::apply_market_resolution_with_evidence` is the
//! degree-zero market-global Resolve oracle. Native resolution composes the
//! reference's pure `derive_payout_vector` seam with the kernel's
//! `resolve_with_vector` seam and distinct v3 point/v4 occupation codecs; the real-SBF campaign is
//! the adapter evidence because the old composed reference account path is
//! deliberately version-two/index-shaped.
//! `clutch_solana_reference::apply_with_evidence` remains the owner-scoped
//! redemption oracle. Every check is rebuilt here from `#[inline(never)]`
//! frames because calling the composed offline adapter exceeds SBF's 4 KiB
//! frame. The generated harness runs both references on identical bytes; its
//! committed plan then exercises the production account plane.
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
//! | owner replay sequence (`RedeemInternal` only) | step 5 | yes |
//! | `kernel_market` invariants | step 6 | yes for v2; v3/v4 additionally read their sole vector owner |
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
//! Resolve has the same evidence ordering but a deliberately narrower state
//! plane: no Position and no owner Replay. Its request sequence is checked
//! against both `Terms.repair_generation` and the sealed window generation;
//! an exact already-recorded fact is idempotent and any conflicting repeat
//! refuses.
//!
//! ## What this program cannot derive, and what that costs
//!
//! Two values the frozen layouts require are called digests, but this
//! instruction does not possess canonical digest preimages for them:
//! [`clutch_accumulator`] deliberately publishes the canonical `WindowDomain`
//! fields and no authenticated archive commitment, while the feed summary
//! commitment has no checked source/archive chain here.
//!
//! - **The window identity.**  Both resolution codecs
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
//!   binding, payout derivation — but the
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
//! - The historical inline differential is retained under `cfg(any())` as
//!   migration archaeology because it still carries the deleted external
//!   shadow DTO. Current process coverage lives in the generated SBF harness
//!   and committed local-bank lane.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::claim_truth;
use crate::error::{ClutchError, Refusal};
use crate::native_window::{self, NativeWindowPreflightV1};
use crate::seeds;
use crate::source_archive::{
    self, ArchiveAccountViewV1, SealedArchiveReceiptV1, SourceSpecAccountViewV1,
    VerifiedSealedArchiveViewV1, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES, SOURCE_SPEC_ACCOUNT_V1_BYTES,
};
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
    account_len,
    native_resolution::{
        NativeResolutionAccount, NATIVE_RESOLUTION_LEN, RESOLUTION_MODE_DERIVED_POINT,
    },
    occupation_resolution::{
        is_occupation_statistic, OccupationResolutionAccount, OCCUPATION_RESOLUTION_LEN,
        RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
    },
    CodecError, FeedAccount, Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes,
    PositionAccount, ResolutionAccount, SupplyLedgerAccount, TermsAccount, MAX_OUTCOMES,
    PAYOUT_INDEX_UNRESOLVED,
};
use clutch_solana_reference::{
    derive_payout, derive_payout_vector, Action, Error as ReferenceError, KernelAccount,
    ReplayAccount, Request, ResolutionRefusal, ResolutionTerms, WindowError, KERNEL_ACCOUNT_LEN,
    MAX_OBSERVATIONS, MAX_WINDOW_EVIDENCE_LEN, OBSERVATION_RECORD_BYTES, REPLAY_ACCOUNT_LEN,
    STAT_SAMPLED_MAX_03, STAT_SAMPLED_MIN_02, STAT_TERMINAL_01, WINDOW_EVIDENCE_HEADER_BYTES,
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

/// Fixed market-global prefix of `Resolve`, before its canonical mint vector.
///
/// Resolution deliberately has no Position or owner Replay account. Its only
/// replay fact is the market's canonical [`ResolutionAccount`], and an exact
/// repeat of that fact is idempotent. The eleven roles are actor, Market,
/// Hoard, kernel aggregate, SupplyLedger, immutable Terms, Resolution, Feed,
/// immutable SourceSpec, sealed SourceArchive, and the hostile evidence
/// projection.  The projection remains only because the existing derivation
/// folds that wire shape; every domain and value record must equal the archive.
pub const RESOLVE_ACCOUNT_PREFIX: usize = 11;
/// Fixed market-global occupation Resolve prefix before its canonical mints.
///
/// Unlike point resolution, occupation consumes the sealed archive directly
/// and has no redundant caller-supplied projection account.
pub const OCCUPATION_RESOLVE_ACCOUNT_PREFIX: usize = 10;
/// Fixed owner-scoped evidence prefix of `RedeemInternal`.
///
/// Redemption remains a position transition and therefore retains the owner
/// Position and generation-scoped Replay account.
pub const EVIDENCE_ACCOUNT_PREFIX: usize = 11;
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
/// Reference-only replay-sequence account.
pub const IX_REPLAY: usize = 5;
/// Market-wide supply-ledger account.
pub const IX_SUPPLY: usize = 6;
/// Immutable terms account.
pub const IX_TERMS: usize = 7;
/// Resolution-record account.
pub const IX_RESOLUTION: usize = 8;
/// Feed-head account (read-only).
pub const IX_FEED: usize = 9;
/// Caller-supplied evidence buffer (read-only, hostile).
pub const IX_BUFFER: usize = 10;

/// Authenticated fee-payer on the market-global Resolve plane.
pub const IX_RESOLVE_ACTOR: usize = 0;
/// Market account on the market-global Resolve plane.
pub const IX_RESOLVE_MARKET: usize = 1;
/// Hoard account on the market-global Resolve plane.
pub const IX_RESOLVE_HOARD: usize = 2;
/// Kernel aggregate on the market-global Resolve plane.
pub const IX_RESOLVE_KERNEL: usize = 3;
/// Market-wide SupplyLedger on the market-global Resolve plane.
pub const IX_RESOLVE_SUPPLY: usize = 4;
/// Immutable Terms on the market-global Resolve plane.
pub const IX_RESOLVE_TERMS: usize = 5;
/// Canonical market Resolution account.
pub const IX_RESOLVE_RESOLUTION: usize = 6;
/// Feed head on the market-global Resolve plane.
pub const IX_RESOLVE_FEED: usize = 7;
/// Immutable authenticated source-spec account.
pub const IX_RESOLVE_SOURCE_SPEC: usize = 8;
/// Canonical sealed source-archive account.
pub const IX_RESOLVE_SOURCE_ARCHIVE: usize = 9;
/// Hostile, redundant evidence projection on the market-global Resolve plane.
pub const IX_RESOLVE_BUFFER: usize = 10;

/* --------------------------------------------------------------------- */
/* `RedeemInternal`'s collateral leg, mandatory                            */
/* --------------------------------------------------------------------- */

/// Fixed prefix length of `RedeemInternal`, before its canonical mint vector.
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
pub const REDEEM_ACCOUNT_PREFIX: usize = 18;

/// Profile identity account (read-only).
pub const IX_PROFILE: usize = 11;
/// The pinned Token-2022 program (read-only, executable).
pub const IX_TOKEN_PROGRAM: usize = 12;
/// The Realm's 266-byte collateral policy (read-only).
pub const IX_POLICY: usize = 13;
/// The collateral mint the Realm's policy names (read-only).
pub const IX_COLLATERAL_MINT: usize = 14;
/// The redeemer's own Token-2022 collateral account (writable).
pub const IX_ACTOR_TOKEN: usize = 15;
/// The Hoard's signing authority; holds no data and is never written.
pub const IX_HOARD_AUTHORITY: usize = 16;
/// The Hoard's Token-2022 collateral account (writable).
pub const IX_HOARD_TOKEN: usize = 17;
/// First canonical outcome mint on a Resolve.
pub const IX_RESOLVE_OUTCOME_MINTS: usize = RESOLVE_ACCOUNT_PREFIX;
/// First canonical outcome mint on an occupation Resolve.
pub const IX_OCCUPATION_RESOLVE_OUTCOME_MINTS: usize = OCCUPATION_RESOLVE_ACCOUNT_PREFIX;
/// First canonical outcome mint on a RedeemInternal.
pub const IX_REDEEM_OUTCOME_MINTS: usize = REDEEM_ACCOUNT_PREFIX;

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
const EVIDENCE_STATE_ROLES: [StateRole; 7] = [
    StateRole::writable(IX_MARKET, account_len::MARKET),
    StateRole::writable(IX_HOARD, account_len::HOARD),
    StateRole::writable(IX_POSITION, account_len::POSITION),
    StateRole::writable(IX_KERNEL, KERNEL_ACCOUNT_LEN),
    StateRole::writable(IX_REPLAY, REPLAY_ACCOUNT_LEN),
    StateRole::writable(IX_SUPPLY, account_len::SUPPLY_LEDGER),
    StateRole::read_only(IX_FEED, account_len::FEED),
];

/// Program-owned market-global roles of `Resolve`.
const RESOLVE_STATE_ROLES: [StateRole; 7] = [
    StateRole::writable(IX_RESOLVE_MARKET, account_len::MARKET),
    StateRole::read_only(IX_RESOLVE_HOARD, account_len::HOARD),
    StateRole::writable(IX_RESOLVE_KERNEL, KERNEL_ACCOUNT_LEN),
    StateRole::writable(IX_RESOLVE_SUPPLY, account_len::SUPPLY_LEDGER),
    StateRole::read_only(IX_RESOLVE_FEED, account_len::FEED),
    StateRole::read_only(IX_RESOLVE_SOURCE_SPEC, SOURCE_SPEC_ACCOUNT_V1_BYTES),
    StateRole::read_only(IX_RESOLVE_SOURCE_ARCHIVE, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES),
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

/// Require the legacy window blob to be an exact value projection of one
/// canonical sealed source archive.
///
/// The blob remains a transport compatibility shape only.  It cannot select a
/// domain, feed cursor, bucket, missing marker, or endpoint: each is compared
/// to immutable terms and to a record read back through the sealed archive
/// receipt.  Source sequence and publish lineage do not appear in the old
/// shape; they remain committed by the archive and are never supplied by this
/// projection.
#[inline(never)]
fn require_archive_projection(
    receipt: SealedArchiveReceiptV1,
    archive: ArchiveAccountViewV1<'_>,
    bytes: &[u8],
    expected_domain: WindowDomain,
) -> Gate<()> {
    if bytes.len() < WINDOW_EVIDENCE_HEADER_BYTES || bytes.len() > MAX_WINDOW_EVIDENCE_LEN {
        return Err(ReferenceError::WrongLength);
    }
    let count = usize::from(u16::from_le_bytes([
        bytes[WINDOW_EVIDENCE_HEADER_BYTES - 2],
        bytes[WINDOW_EVIDENCE_HEADER_BYTES - 1],
    ]));
    if count > MAX_OBSERVATIONS
        || count != usize::try_from(expected_domain.range_len()).unwrap_or(usize::MAX)
    {
        return Err(ReferenceError::WrongLength);
    }
    let exact = WINDOW_EVIDENCE_HEADER_BYTES
        .checked_add(
            count
                .checked_mul(OBSERVATION_RECORD_BYTES)
                .ok_or(ReferenceError::WrongLength)?,
        )
        .ok_or(ReferenceError::WrongLength)?;
    if bytes.len() != exact || bytes[0] != WINDOW_EVIDENCE_TAG {
        return Err(ReferenceError::WrongLength);
    }
    if bytes[1] != REFERENCE_VERSION {
        return Err(ReferenceError::WrongVersion);
    }

    let mut reader = Cursor::new(bytes, 2);
    let feed = FeedIdentity::new(
        reader.raw::<IDENTITY_BYTES>()?,
        reader.raw::<IDENTITY_BYTES>()?,
        reader.u32()?,
        reader.u32()?,
    )?;
    let grid = Grid::new(reader.u32()?, reader.u16()?, reader.u64()?)
        .map_err(|error| ReferenceError::Window(WindowError::Summary(error)))?;
    let start = reader.u64()?;
    let end = reader.u64()?;
    let maturity = reader.u64()?;
    let generation = reader.u64()?;
    let coverage = CoveragePolicy::from_registry(reader.u16()?, reader.u64()?)?;
    if usize::from(reader.u16()?) != count {
        return Err(ReferenceError::WrongLength);
    }
    let declared = WindowDomain::new(feed, grid, start, end, maturity, generation, coverage)?;
    declared.check_against(&expected_domain)?;
    if source_archive::canonical_window_id(declared) != receipt.window()
        || receipt.start_bucket() != start
        || receipt.end_bucket_exclusive() != end
    {
        return Err(ReferenceError::ResolutionBindingMismatch);
    }

    let mut index = 0_usize;
    while index < count {
        let kind = reader.u8()?;
        let bucket = reader.u64()?;
        let low = reader.u128()?;
        let high = reader.u128()?;
        if kind != OBSERVATION_ACCEPTED {
            return Err(ReferenceError::ResolutionEvidenceUnavailable);
        }
        let archived = source_archive::archived_observation(receipt, archive, index)
            .map_err(|_| ReferenceError::ResolutionEvidenceUnavailable)?;
        if bucket != archived.bucket || low != archived.low || high != archived.high {
            return Err(ReferenceError::ResolutionEvidenceUnavailable);
        }
        index += 1;
    }
    Ok(())
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
    basis_degree: u8,
    statistic: u16,
    repair_generation: u64,
}

/// The resolution-record facts the gate reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecordHead {
    window: Hash32,
    payout_index: u8,
    resolved: bool,
    native_vector: PayoutVectorBytes,
}

/// The sealed-window facts a resolve records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SealedFacts {
    payout_index: u8,
    sealed_cursor: u64,
    end_bucket_exclusive: u64,
    repair_generation: u64,
}

/// The point and exact native vector derived from one sealed smooth window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeSealedFacts {
    resolved_value: u128,
    vector: PayoutVectorBytes,
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
    basis_mode: BasisMode::FinitePreset,
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

#[inline(never)]
fn load_native_resolution(bytes: &[u8], out: &mut NativeResolutionAccount) -> Gate<()> {
    NativeResolutionAccount::decode_into(bytes, out)?;
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

/// The reusable native projection consumed by every post-resolution payout path.
///
/// Both internal and bearer redemption call this helper (and
/// [`reconstruct_native_market`]) rather than decode another interpretation of
/// v3 or v4. That keeps Resolution the sole persisted vector owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BoundNativeResolution {
    /// Declared sealed-window identity recorded at resolution.
    pub(crate) window: Hash32,
    /// Exact native payout vector owned by the Terms-selected v3 or v4 record.
    pub(crate) vector: PayoutVectorBytes,
}

/// Decode and bind a resolved native record to immutable terms and market.
#[inline(never)]
pub(crate) fn bound_native_resolution(
    resolution_bytes: &[u8],
    terms_bytes: &[u8],
    resolution_bump: u8,
    market: Hash32,
) -> core::result::Result<BoundNativeResolution, ReferenceError> {
    let mut terms = ZERO_TERMS;
    load_terms(terms_bytes, &mut terms)?;
    if !(1..=3).contains(&terms.basis_degree) {
        return Err(ReferenceError::Resolution(
            ResolutionRefusal::WrongResolutionMode,
        ));
    }
    if is_occupation_statistic(terms.statistic_id) {
        let record = OccupationResolutionAccount::decode(resolution_bytes)?;
        if record.stored_bump != resolution_bump {
            return Err(ReferenceError::WrongBump);
        }
        if record.market != market {
            return Err(ReferenceError::ResolutionBindingMismatch);
        }
        record
            .binds_terms_fields(&terms)
            .map_err(|_| ReferenceError::ResolutionBindingMismatch)?;
        if record.mode != RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION {
            return Err(ReferenceError::ResolutionNotRecorded);
        }
        return Ok(BoundNativeResolution {
            window: record.window,
            vector: record.vector,
        });
    }
    let mut record = NativeResolutionAccount::ZEROED;
    load_native_resolution(resolution_bytes, &mut record)?;
    if record.stored_bump != resolution_bump {
        return Err(ReferenceError::WrongBump);
    }
    if record.market != market {
        return Err(ReferenceError::ResolutionBindingMismatch);
    }
    record
        .binds_terms_fields(&terms)
        .map_err(|_| ReferenceError::ResolutionBindingMismatch)?;
    if record.mode != RESOLUTION_MODE_DERIVED_POINT {
        return Err(ReferenceError::ResolutionNotRecorded);
    }
    Ok(BoundNativeResolution {
        window: record.window,
        vector: record.vector,
    })
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
        basis_degree: terms.basis_degree,
        statistic: terms.statistic_id,
        repair_generation: terms.repair_generation,
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
    let expected_mode = if terms.basis_degree == 0 {
        BasisMode::FinitePreset
    } else {
        BasisMode::DerivedBasis
    };
    if kernel.basis_mode != expected_mode {
        return Err(ReferenceError::Kernel(
            clutch_kernel::Error::WrongResolutionMode,
        ));
    }
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
    if resolution_bytes.len() == account_len::RESOLUTION {
        let mut terms = ZERO_TERMS;
        load_terms(terms_bytes, &mut terms)?;
        if terms.basis_degree != 0 {
            return Err(ReferenceError::Resolution(
                ResolutionRefusal::WrongResolutionMode,
            ));
        }
        let mut record = ZERO_RESOLUTION;
        load_resolution(resolution_bytes, &mut record)?;
        if record.stored_bump != resolution_bump {
            return Err(ReferenceError::WrongBump);
        }
        if record.market != market {
            return Err(ReferenceError::ResolutionBindingMismatch);
        }
        require_record_binds_terms(&record, &terms)?;
        return Ok(RecordHead {
            window: record.window,
            payout_index: record.payout_index,
            resolved: record.is_resolved(),
            native_vector: PayoutVectorBytes::ZERO,
        });
    }
    if resolution_bytes.len() != NATIVE_RESOLUTION_LEN
        && resolution_bytes.len() != OCCUPATION_RESOLUTION_LEN
    {
        return Err(ReferenceError::WrongLength);
    }
    let record = bound_native_resolution(resolution_bytes, terms_bytes, resolution_bump, market)?;
    Ok(RecordHead {
        window: record.window,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        resolved: true,
        native_vector: record.vector,
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

/// Reconstruct only the immutable expected window, keeping the larger terms
/// derivation in its own SBF frame.
#[inline(never)]
fn expected_window_domain(market_bytes: &[u8], terms_bytes: &[u8]) -> Gate<WindowDomain> {
    Ok(derived_terms(market_bytes, terms_bytes)?.window)
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
    /* This persisted account path carries only a finite-preset index. Native
     * d1-d3 resolution belongs to `derive_payout_vector` plus the kernel's
     * `resolve_with_vector`; a preset membership search here would silently
     * lower shaped settlement back into portfolio sugar. */
    if derived.basis_degree != 0 {
        return Err(ReferenceError::Resolution(
            ResolutionRefusal::WrongResolutionMode,
        ));
    }
    let mut terms = ZERO_TERMS;
    load_terms(terms_bytes, &mut terms)?;
    let payouts = terms.payouts;
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

/// Derive one smooth-basis payout from an exact integer statistic point.
///
/// The persisted v3 record stores the raw point before edge handling.  All
/// smooth degrees therefore use the stricter point-only admission rule here,
/// including degree one: an interval whose endpoint vectors happen to
/// quantize equally is still not a point and cannot be serialized as one.
/// No midpoint or endpoint choice exists on this path.
#[inline(never)]
fn derive_native_from_evidence(
    market_bytes: &[u8],
    terms_bytes: &[u8],
    window_bytes: &[u8],
    feed_cursor: u64,
) -> Gate<NativeSealedFacts> {
    let derived = derived_terms(market_bytes, terms_bytes)?;
    if !(1..=3).contains(&derived.basis_degree) {
        return Err(ReferenceError::Resolution(
            ResolutionRefusal::WrongResolutionMode,
        ));
    }
    let window = fold_window_evidence(window_bytes, feed_cursor)?;
    /* The reference seam owns source identity, grid, exact window-domain,
     * edge policy, spline evaluation and largest-remainder quantization. */
    let vector = derive_payout_vector(&derived, &window)?;
    let (low, high) = match derived.statistic {
        STAT_TERMINAL_01 => {
            let interval = window
                .terminal()
                .map_err(|_| ReferenceError::Resolution(ResolutionRefusal::NoAcceptedCoverage))?;
            (interval.low(), interval.high())
        }
        STAT_SAMPLED_MIN_02 => {
            let interval = window.sampled_min().ok_or(ReferenceError::Resolution(
                ResolutionRefusal::NoAcceptedCoverage,
            ))?;
            (interval.low(), interval.high())
        }
        STAT_SAMPLED_MAX_03 => {
            let interval = window.sampled_max().ok_or(ReferenceError::Resolution(
                ResolutionRefusal::NoAcceptedCoverage,
            ))?;
            (interval.low(), interval.high())
        }
        _ => {
            return Err(ReferenceError::Resolution(
                ResolutionRefusal::StatisticUnsupported,
            ))
        }
    };
    if low != high {
        return Err(ReferenceError::Resolution(
            ResolutionRefusal::NonPointEvidence,
        ));
    }
    Ok(NativeSealedFacts {
        resolved_value: low,
        vector,
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

/// Rebuild the derived-basis invariant from the v3/v4 record without retaining
/// a second persisted vector.
#[inline(never)]
fn native_kernel_invariants(
    kernel_bytes: &[u8],
    outcome_count: u8,
    collateral: u64,
    resolution_bytes: &[u8],
) -> Gate<()> {
    let vector = if resolution_bytes.len() == NATIVE_RESOLUTION_LEN {
        let mut record = NativeResolutionAccount::ZEROED;
        load_native_resolution(resolution_bytes, &mut record)?;
        if record.mode == RESOLUTION_MODE_DERIVED_POINT {
            PayoutVector::new(record.vector.denominator, record.vector.weights)
        } else {
            PayoutVector::ZERO
        }
    } else if resolution_bytes.len() == OCCUPATION_RESOLUTION_LEN {
        let record = OccupationResolutionAccount::decode(resolution_bytes)?;
        if record.mode == RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION {
            PayoutVector::new(record.vector.denominator, record.vector.weights)
        } else {
            PayoutVector::ZERO
        }
    } else {
        return Err(ReferenceError::WrongLength);
    };
    let market = reconstruct_native_market(kernel_bytes, outcome_count, collateral, vector)?;
    market.check_invariants()?;
    Ok(())
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
    if kernel.basis_mode != BasisMode::FinitePreset {
        return Err(ReferenceError::Kernel(
            clutch_kernel::Error::WrongResolutionMode,
        ));
    }
    if usize::from(outcome_count) > KERNEL_MAX_OUTCOMES || kernel.payouts.outcomes != outcome_count
    {
        return Err(ReferenceError::MismatchedState);
    }
    let phase = match kernel.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(ReferenceError::NonCanonical),
    };
    let market = MarketState {
        outcomes: outcome_count,
        phase,
        resolved_payout: kernel.resolved_payout,
        basis_mode: kernel.basis_mode,
        resolved_vector: PayoutVector::ZERO,
        collateral,
        total_supply: kernel.total_supply,
        payouts: kernel.payouts,
    };
    market.check_invariants()?;
    Ok(market)
}

/// Rebuild a derived-basis market from the aggregate and the sole persisted
/// native vector.  The vector is ephemeral: it is installed only in this
/// stack value and is never copied into `KernelAccount`.
///
/// This constructor deliberately performs no nested `MarketState` call while
/// its large decoded `KernelAccount` is live.  Such a call caused the final
/// SBF linker to report a callee overwriting this frame.  Invariant-only users
/// call `check_invariants` after this decode frame has returned; transition
/// users immediately call a kernel transition, whose first operation checks
/// the same invariants before any mutation.
#[inline(never)]
pub(crate) fn reconstruct_native_market(
    kernel_bytes: &[u8],
    outcome_count: u8,
    collateral: u64,
    resolved_vector: PayoutVector,
) -> core::result::Result<MarketState, ReferenceError> {
    let kernel = KernelAccount::decode(kernel_bytes)?;
    if kernel.basis_mode != BasisMode::DerivedBasis {
        return Err(ReferenceError::Kernel(
            clutch_kernel::Error::WrongResolutionMode,
        ));
    }
    if usize::from(outcome_count) > KERNEL_MAX_OUTCOMES
        || kernel.payouts.outcomes != outcome_count
        || (kernel.phase == 1 && kernel.resolved_payout != 0)
    {
        return Err(ReferenceError::MismatchedState);
    }
    let phase = match kernel.phase {
        0 => Phase::Active,
        1 => Phase::Resolved,
        _ => return Err(ReferenceError::NonCanonical),
    };
    let market = MarketState {
        outcomes: outcome_count,
        phase,
        resolved_payout: 0,
        basis_mode: kernel.basis_mode,
        resolved_vector: if phase == Phase::Resolved {
            resolved_vector
        } else {
            PayoutVector::ZERO
        },
        collateral,
        total_supply: kernel.total_supply,
        payouts: kernel.payouts,
    };
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

/// Run the native derived-vector resolution seam in its own frame.
#[inline(never)]
fn kernel_resolve_native(
    kernel_bytes: &[u8],
    outcome_count: u8,
    collateral: u64,
    vector: PayoutVectorBytes,
) -> Gate<KernelStep> {
    let installed = PayoutVector::new(vector.denominator, vector.weights);
    let mut market =
        reconstruct_native_market(kernel_bytes, outcome_count, collateral, PayoutVector::ZERO)?;
    market.resolve_with_vector(installed)?;
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

/// Redeem against the immutable v3/v4 record's vector without persisting a copy.
#[inline(never)]
fn kernel_redeem_native(
    kernel_bytes: &[u8],
    outcome_count: u8,
    collateral: u64,
    vector: PayoutVectorBytes,
    position: &mut Position,
    outcome: u8,
    quantity: u64,
) -> Gate<(KernelStep, u64)> {
    let installed = PayoutVector::new(vector.denominator, vector.weights);
    let mut market = reconstruct_native_market(kernel_bytes, outcome_count, collateral, installed)?;
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

#[cfg(test)]
mod basis_mode_tests {
    use super::*;
    use clutch_kernel::{Error as KernelError, PayoutSet, MAX_PAYOUTS};

    fn payout() -> PayoutVector {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[0] = 1;
        PayoutVector::new(1, weights)
    }

    fn encoded(mode: BasisMode, phase: u8) -> Vec<u8> {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = payout();
        let mut total_supply = [0_u64; MAX_OUTCOMES];
        total_supply[0] = 4;
        let account = KernelAccount {
            market: Hash32::from_bytes([3; 32]),
            phase,
            basis_mode: mode,
            resolved_payout: 0,
            payouts: PayoutSet::new(1, 2, vectors),
            total_supply,
        };
        let mut bytes = vec![0_u8; KERNEL_ACCOUNT_LEN];
        account.encode(&mut bytes).unwrap();
        bytes
    }

    fn wrong_mode() -> ReferenceError {
        ReferenceError::Kernel(KernelError::WrongResolutionMode)
    }

    #[test]
    fn categorical_and_native_resolution_refuse_opposite_stored_modes() {
        assert_eq!(
            kernel_resolve(&encoded(BasisMode::DerivedBasis, 0), 2, 4, 0),
            Err(wrong_mode())
        );
        assert_eq!(
            kernel_resolve_native(
                &encoded(BasisMode::FinitePreset, 0),
                2,
                4,
                PayoutVectorBytes {
                    denominator: 1,
                    weights: payout().weights,
                },
            ),
            Err(wrong_mode())
        );
    }

    #[test]
    fn categorical_and_native_internal_redemption_refuse_before_position_mutation() {
        let mut categorical_position = Position::EMPTY;
        categorical_position.internal[0] = 1;
        let categorical_before = categorical_position;
        assert_eq!(
            kernel_redeem(
                &encoded(BasisMode::DerivedBasis, 1),
                2,
                4,
                &mut categorical_position,
                0,
                1,
            ),
            Err(wrong_mode())
        );
        assert_eq!(categorical_position, categorical_before);

        let mut native_position = categorical_before;
        assert_eq!(
            kernel_redeem_native(
                &encoded(BasisMode::FinitePreset, 1),
                2,
                4,
                PayoutVectorBytes {
                    denominator: 1,
                    weights: payout().weights,
                },
                &mut native_position,
                0,
                1,
            ),
            Err(wrong_mode())
        );
        assert_eq!(native_position, categorical_before);
    }
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

/// The six mutable state accounts of one evidence-gated transition.
#[derive(Debug)]
struct StateSlices<'a> {
    market: &'a mut [u8],
    hoard: &'a mut [u8],
    position: &'a mut [u8],
    kernel: &'a mut [u8],
    replay: &'a mut [u8],
    supply: &'a mut [u8],
}

/// The four mutable/read-only market-global state slices of `Resolve`.
///
/// There is intentionally no Position or owner Replay slice here. If either
/// becomes necessary to express resolution, the market-global replay domain
/// has regressed.
#[derive(Debug)]
struct ResolveStateSlices<'a> {
    market: &'a mut [u8],
    hoard: &'a [u8],
    kernel: &'a mut [u8],
    supply: &'a mut [u8],
}

/// The canonical bumps [`process`] derived, compared at the reference's points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Bumps {
    market: u8,
    hoard: u8,
    position: u8,
    replay: u8,
    supply: u8,
    terms: u8,
    resolution: u8,
}

/// Canonical market-global bumps used by `Resolve`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolveBumps {
    market: u8,
    hoard: u8,
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

/// Owner-scoped internal-redemption request, already routed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RedeemAction {
    outcome: u8,
    quantity: u64,
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
    action: RedeemAction,
) -> Gate<()> {
    /* 1. Decode, in the reference's order. */
    let market = market_head(state.market)?;
    let mut hoard = HoardAccount::decode(state.hoard)?;
    let mut position = PositionAccount::decode(state.position)?;
    let kernel = kernel_head(state.kernel)?;
    let mut replay = ReplayAccount::decode(state.replay)?;
    let mut supply = SupplyLedgerAccount::decode(state.supply)?;

    /* 2. `validate_links`: stored bumps, then cross-account identity. */
    if market.stored_bump != bumps.market
        || market.hoard_bump != bumps.hoard
        || hoard.stored_bump != bumps.hoard
        || position.stored_bump != bumps.position
        || replay.stored_bump != bumps.replay
        || supply.stored_bump != bumps.supply
    {
        return Err(ReferenceError::WrongBump);
    }
    if market.market != hoard.market
        || market.realm != hoard.realm
        || market.market != position.market
        || market.market != kernel.market
        || market.market != replay.market
        || market.market != supply.market
        || market.realm != supply.realm
        || market.outcome_count != supply.outcome_count
        || position.owner != replay.owner
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
        if position.internal[index] != 0 || kernel.total_supply[index] != 0 {
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
    if plane.resolution.len() == NATIVE_RESOLUTION_LEN
        || plane.resolution.len() == OCCUPATION_RESOLUTION_LEN
    {
        native_kernel_invariants(
            state.kernel,
            market.outcome_count,
            hoard.collateral_atoms,
            plane.resolution,
        )?;
    } else {
        kernel_invariants(state.kernel, market.outcome_count, hoard.collateral_atoms)?;
    }
    let mut pure_position = Position {
        internal: position.internal,
        external: [0; MAX_OUTCOMES],
    };

    /* 7. `validate_evidence_metadata`, the half that is a byte-level fact. */
    if plane.terms_writable {
        return Err(ReferenceError::ImmutableAccountWritable);
    }
    if plane.resolution_writable {
        return Err(ReferenceError::ImmutableAccountWritable);
    }
    if plane.window_id == Hash32::ZERO {
        return Err(ReferenceError::WindowIdentityUnavailable);
    }

    let step = {
        let RedeemAction { outcome, quantity } = action;
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
        if terms.basis_degree == 0 && record.payout_index >= terms.payout_count {
            return Err(ReferenceError::Resolution(
                ResolutionRefusal::PayoutIndexOutOfRange,
            ));
        }
        if market.lifecycle != 1
            || kernel.phase != 1
            || (terms.basis_degree == 0 && kernel.resolved_payout != record.payout_index)
            || (terms.basis_degree != 0 && kernel.resolved_payout != 0)
        {
            return Err(ReferenceError::MismatchedState);
        }

        /* 10. Only now does the kernel move. */
        let (step, paid) = if terms.basis_degree == 0 {
            kernel_redeem(
                state.kernel,
                market.outcome_count,
                hoard.collateral_atoms,
                &mut pure_position,
                outcome,
                quantity,
            )?
        } else {
            kernel_redeem_native(
                state.kernel,
                market.outcome_count,
                hoard.collateral_atoms,
                record.native_vector,
                &mut pure_position,
                outcome,
                quantity,
            )?
        };
        position.cash_atoms = position
            .cash_atoms
            .checked_add(paid)
            .ok_or(ReferenceError::Arithmetic)?;
        step
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
        outcome += 1;
    }
    position.internal = pure_position.internal;
    replay.sequence = next_sequence;

    /* 12. C1 and C2 again, over the post-state. */
    check_closure(
        market.outcome_count,
        &supply,
        &step.total_supply,
        &position.internal,
    )?;

    /* 13. Everything below this line writes. */
    write_kernel(state.kernel, &step)?;
    hoard.encode(state.hoard)?;
    position.encode(state.position)?;
    replay.encode(state.replay)?;
    supply.encode(state.supply)?;
    Ok(())
}

/// Result of one market-global resolution attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolveOutput {
    resolution: ResolutionWrite,
    repeated: bool,
}

/// Explicitly versioned resolution bytes. Account length and immutable Terms
/// select one arm; v2, v3, and v4 bytes are never interpreted as one another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionWrite {
    Legacy([u8; account_len::RESOLUTION]),
    Native([u8; NATIVE_RESOLUTION_LEN]),
    Occupation([u8; OCCUPATION_RESOLUTION_LEN]),
}

impl ResolutionWrite {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Legacy(bytes) => bytes,
            Self::Native(bytes) => bytes,
            Self::Occupation(bytes) => bytes,
        }
    }
}

/// Apply the market-global resolution transition without an owner state plane.
///
/// `Request::sequence` has a resolution-specific meaning on this action: it is
/// the exact repair generation selected by immutable Terms and by the sealed
/// evidence. It is not an incrementing owner nonce. The persisted
/// [`ResolutionAccount`] is the sole replay fact. An exact repeat validates
/// the same derivation and returns the record byte-for-byte without writing any
/// market state; a conflicting repeat refuses.
#[inline(never)]
fn apply_market_resolution(
    state: &mut ResolveStateSlices<'_>,
    plane: &EvidencePlane<'_>,
    bumps: &ResolveBumps,
    occupation: Option<&NativeWindowPreflightV1>,
    signer: bool,
    sequence: u64,
    requested_payout: u8,
) -> Gate<ResolveOutput> {
    if plane.resolution.len() == OCCUPATION_RESOLUTION_LEN {
        let candidate = occupation.ok_or(ReferenceError::ResolutionEvidenceUnavailable)?;
        return apply_occupation_market_resolution(
            state,
            plane,
            bumps,
            *candidate,
            signer,
            sequence,
            requested_payout,
        );
    }
    if plane.resolution.len() == NATIVE_RESOLUTION_LEN {
        return apply_native_market_resolution(
            state,
            plane,
            bumps,
            signer,
            sequence,
            requested_payout,
        );
    }
    apply_legacy_market_resolution(state, plane, bumps, signer, sequence, requested_payout)
}

/// The unchanged version-two finite-preset transition.
#[inline(never)]
fn apply_legacy_market_resolution(
    state: &mut ResolveStateSlices<'_>,
    plane: &EvidencePlane<'_>,
    bumps: &ResolveBumps,
    signer: bool,
    sequence: u64,
    requested_payout: u8,
) -> Gate<ResolveOutput> {
    let market = market_head(state.market)?;
    let hoard = HoardAccount::decode(state.hoard)?;
    let kernel = kernel_head(state.kernel)?;
    let supply = SupplyLedgerAccount::decode(state.supply)?;

    if market.stored_bump != bumps.market
        || market.hoard_bump != bumps.hoard
        || hoard.stored_bump != bumps.hoard
        || supply.stored_bump != bumps.supply
    {
        return Err(ReferenceError::WrongBump);
    }
    if market.market != hoard.market
        || market.realm != hoard.realm
        || market.market != kernel.market
        || market.market != supply.market
        || market.realm != supply.realm
        || market.outcome_count != supply.outcome_count
        || kernel.payout_outcomes != market.outcome_count
        || (market.lifecycle == 0 && kernel.phase != 0)
        || (market.lifecycle == 1 && kernel.phase != 1)
        || market.lifecycle > 1
    {
        return Err(ReferenceError::MismatchedState);
    }
    let count = usize::from(market.outcome_count);
    let mut index = count;
    while index < MAX_OUTCOMES {
        if kernel.total_supply[index] != 0 {
            return Err(ReferenceError::NonCanonical);
        }
        index += 1;
    }
    check_market_closure(market.outcome_count, &supply, &kernel.total_supply)?;
    kernel_invariants(state.kernel, market.outcome_count, hoard.collateral_atoms)?;

    if plane.terms_writable {
        return Err(ReferenceError::ImmutableAccountWritable);
    }
    if !plane.resolution_writable {
        return Err(ReferenceError::NotWritable);
    }
    if plane.window_id == Hash32::ZERO {
        return Err(ReferenceError::WindowIdentityUnavailable);
    }
    if !signer {
        return Err(ReferenceError::MissingSignature);
    }

    let terms = terms_binds_market(state.market, plane.terms, bumps.terms)?;
    payout_set_binds_terms(state.kernel, plane.terms)?;
    let mut record = ZERO_RESOLUTION;
    load_resolution(plane.resolution, &mut record)?;
    if record.stored_bump != bumps.resolution {
        return Err(ReferenceError::WrongBump);
    }
    if record.market != market.market {
        return Err(ReferenceError::ResolutionBindingMismatch);
    }
    let mut full_terms = ZERO_TERMS;
    load_terms(plane.terms, &mut full_terms)?;
    require_record_binds_terms(&record, &full_terms)?;

    let sealed = derive_from_evidence(
        state.market,
        plane.terms,
        plane.window,
        plane.feed_cursor,
        requested_payout,
    )?;
    if sequence != terms.repair_generation || sequence != sealed.repair_generation {
        return Err(ReferenceError::Replay);
    }

    let already_resolved = record.is_resolved();
    let expected = ResolutionAccount {
        market: market.market,
        terms: terms.terms,
        feed: terms.feed,
        window: plane.window_id,
        feed_cursor: if already_resolved {
            record.feed_cursor
        } else {
            sealed.sealed_cursor
        },
        sealed_end_bucket_exclusive: sealed.end_bucket_exclusive,
        repair_generation: sealed.repair_generation,
        resolved_slot: if already_resolved {
            record.resolved_slot
        } else {
            plane.resolved_slot
        },
        payout_index: sealed.payout_index,
        stored_bump: bumps.resolution,
        flags: 0,
    };
    let mut resolution_bytes = [0; account_len::RESOLUTION];
    expected.encode(&mut resolution_bytes)?;

    match (market.lifecycle, kernel.phase, already_resolved) {
        (0, 0, false) => {
            let step = kernel_resolve(
                state.kernel,
                market.outcome_count,
                hoard.collateral_atoms,
                requested_payout,
            )?;
            check_market_closure(market.outcome_count, &supply, &step.total_supply)?;
            write_market_lifecycle(state.market, 1)?;
            write_kernel(state.kernel, &step)?;
            Ok(ResolveOutput {
                resolution: ResolutionWrite::Legacy(resolution_bytes),
                repeated: false,
            })
        }
        (1, 1, true) => {
            if kernel.resolved_payout != requested_payout || record != expected {
                return Err(ReferenceError::ResolutionBindingMismatch);
            }
            Ok(ResolveOutput {
                resolution: ResolutionWrite::Legacy(resolution_bytes),
                repeated: true,
            })
        }
        (0, 0, true) => Err(ReferenceError::ResolutionAlreadyRecorded),
        _ => Err(ReferenceError::MismatchedState),
    }
}

/// Version-three point resolution for degree-one through degree-three terms.
///
/// The request's historical `payout_index` byte is a mode discriminator on
/// this path and must be the unresolved sentinel.  The selected payout is the
/// vector derived from evidence, never a caller-named preset.
#[inline(never)]
fn apply_native_market_resolution(
    state: &mut ResolveStateSlices<'_>,
    plane: &EvidencePlane<'_>,
    bumps: &ResolveBumps,
    signer: bool,
    sequence: u64,
    requested_payout: u8,
) -> Gate<ResolveOutput> {
    let market = market_head(state.market)?;
    let hoard = HoardAccount::decode(state.hoard)?;
    let kernel = kernel_head(state.kernel)?;
    let supply = SupplyLedgerAccount::decode(state.supply)?;

    if market.stored_bump != bumps.market
        || market.hoard_bump != bumps.hoard
        || hoard.stored_bump != bumps.hoard
        || supply.stored_bump != bumps.supply
    {
        return Err(ReferenceError::WrongBump);
    }
    if market.market != hoard.market
        || market.realm != hoard.realm
        || market.market != kernel.market
        || market.market != supply.market
        || market.realm != supply.realm
        || market.outcome_count != supply.outcome_count
        || kernel.payout_outcomes != market.outcome_count
        || (market.lifecycle == 0 && kernel.phase != 0)
        || (market.lifecycle == 1 && kernel.phase != 1)
        || market.lifecycle > 1
    {
        return Err(ReferenceError::MismatchedState);
    }
    let count = usize::from(market.outcome_count);
    let mut index = count;
    while index < MAX_OUTCOMES {
        if kernel.total_supply[index] != 0 {
            return Err(ReferenceError::NonCanonical);
        }
        index += 1;
    }
    check_market_closure(market.outcome_count, &supply, &kernel.total_supply)?;

    if plane.terms_writable {
        return Err(ReferenceError::ImmutableAccountWritable);
    }
    if !plane.resolution_writable {
        return Err(ReferenceError::NotWritable);
    }
    if plane.window_id == Hash32::ZERO {
        return Err(ReferenceError::WindowIdentityUnavailable);
    }
    if !signer {
        return Err(ReferenceError::MissingSignature);
    }
    if requested_payout != PAYOUT_INDEX_UNRESOLVED {
        return Err(ReferenceError::PayoutIndexMismatch);
    }

    let terms = terms_binds_market(state.market, plane.terms, bumps.terms)?;
    if !(1..=3).contains(&terms.basis_degree) {
        return Err(ReferenceError::Resolution(
            ResolutionRefusal::WrongResolutionMode,
        ));
    }
    payout_set_binds_terms(state.kernel, plane.terms)?;
    let mut record = NativeResolutionAccount::ZEROED;
    load_native_resolution(plane.resolution, &mut record)?;
    if record.stored_bump != bumps.resolution {
        return Err(ReferenceError::WrongBump);
    }
    if record.market != market.market {
        return Err(ReferenceError::ResolutionBindingMismatch);
    }
    {
        let mut full_terms = ZERO_TERMS;
        load_terms(plane.terms, &mut full_terms)?;
        record
            .binds_terms_fields(&full_terms)
            .map_err(|_| ReferenceError::ResolutionBindingMismatch)?;
    }
    native_kernel_invariants(
        state.kernel,
        market.outcome_count,
        hoard.collateral_atoms,
        plane.resolution,
    )?;

    let sealed =
        derive_native_from_evidence(state.market, plane.terms, plane.window, plane.feed_cursor)?;
    if sequence != terms.repair_generation || sequence != sealed.repair_generation {
        return Err(ReferenceError::Replay);
    }

    let already_resolved = record.is_resolved();
    let expected = NativeResolutionAccount {
        market: market.market,
        terms: terms.terms,
        feed: terms.feed,
        window: plane.window_id,
        feed_cursor: if already_resolved {
            record.feed_cursor
        } else {
            sealed.sealed_cursor
        },
        sealed_end_bucket_exclusive: sealed.end_bucket_exclusive,
        repair_generation: sealed.repair_generation,
        resolved_slot: if already_resolved {
            record.resolved_slot
        } else {
            plane.resolved_slot
        },
        mode: RESOLUTION_MODE_DERIVED_POINT,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: market.outcome_count,
        resolved_value: sealed.resolved_value,
        vector: sealed.vector,
        stored_bump: bumps.resolution,
        flags: 0,
    };
    {
        let mut full_terms = ZERO_TERMS;
        load_terms(plane.terms, &mut full_terms)?;
        expected
            .binds_terms_fields(&full_terms)
            .map_err(|_| ReferenceError::ResolutionBindingMismatch)?;
    }
    let mut resolution_bytes = [0_u8; NATIVE_RESOLUTION_LEN];
    expected.encode(&mut resolution_bytes)?;

    match (market.lifecycle, kernel.phase, already_resolved) {
        (0, 0, false) => {
            let step = kernel_resolve_native(
                state.kernel,
                market.outcome_count,
                hoard.collateral_atoms,
                sealed.vector,
            )?;
            check_market_closure(market.outcome_count, &supply, &step.total_supply)?;
            write_market_lifecycle(state.market, 1)?;
            write_kernel(state.kernel, &step)?;
            Ok(ResolveOutput {
                resolution: ResolutionWrite::Native(resolution_bytes),
                repeated: false,
            })
        }
        (1, 1, true) => {
            if kernel.resolved_payout != 0 || record != expected {
                return Err(ReferenceError::ResolutionBindingMismatch);
            }
            Ok(ResolveOutput {
                resolution: ResolutionWrite::Native(resolution_bytes),
                repeated: true,
            })
        }
        (0, 0, true) => Err(ReferenceError::ResolutionAlreadyRecorded),
        _ => Err(ReferenceError::MismatchedState),
    }
}

/// Version-four quantized-occupation resolution for degree-one through
/// degree-three Terms.
///
/// The candidate is derived only from the once-verified canonical archive in
/// the account plane.  No caller projection, midpoint, preset lookup, or point
/// statistic enters this transition.
#[inline(never)]
fn apply_occupation_market_resolution(
    state: &mut ResolveStateSlices<'_>,
    plane: &EvidencePlane<'_>,
    bumps: &ResolveBumps,
    candidate: NativeWindowPreflightV1,
    signer: bool,
    sequence: u64,
    requested_payout: u8,
) -> Gate<ResolveOutput> {
    let market = market_head(state.market)?;
    let hoard = HoardAccount::decode(state.hoard)?;
    let kernel = kernel_head(state.kernel)?;
    let supply = SupplyLedgerAccount::decode(state.supply)?;

    if market.stored_bump != bumps.market
        || market.hoard_bump != bumps.hoard
        || hoard.stored_bump != bumps.hoard
        || supply.stored_bump != bumps.supply
    {
        return Err(ReferenceError::WrongBump);
    }
    if market.market != hoard.market
        || market.realm != hoard.realm
        || market.market != kernel.market
        || market.market != supply.market
        || market.realm != supply.realm
        || market.outcome_count != supply.outcome_count
        || kernel.payout_outcomes != market.outcome_count
        || (market.lifecycle == 0 && kernel.phase != 0)
        || (market.lifecycle == 1 && kernel.phase != 1)
        || market.lifecycle > 1
    {
        return Err(ReferenceError::MismatchedState);
    }
    let count = usize::from(market.outcome_count);
    let mut index = count;
    while index < MAX_OUTCOMES {
        if kernel.total_supply[index] != 0 {
            return Err(ReferenceError::NonCanonical);
        }
        index += 1;
    }
    check_market_closure(market.outcome_count, &supply, &kernel.total_supply)?;

    if plane.terms_writable {
        return Err(ReferenceError::ImmutableAccountWritable);
    }
    if !plane.resolution_writable {
        return Err(ReferenceError::NotWritable);
    }
    if plane.window_id == Hash32::ZERO {
        return Err(ReferenceError::WindowIdentityUnavailable);
    }
    if !signer {
        return Err(ReferenceError::MissingSignature);
    }
    if requested_payout != PAYOUT_INDEX_UNRESOLVED {
        return Err(ReferenceError::PayoutIndexMismatch);
    }

    let terms = terms_binds_market(state.market, plane.terms, bumps.terms)?;
    if !(1..=3).contains(&terms.basis_degree) || !is_occupation_statistic(terms.statistic) {
        return Err(ReferenceError::Resolution(
            ResolutionRefusal::WrongResolutionMode,
        ));
    }
    payout_set_binds_terms(state.kernel, plane.terms)?;
    let record = OccupationResolutionAccount::decode(plane.resolution)?;
    if record.stored_bump != bumps.resolution {
        return Err(ReferenceError::WrongBump);
    }
    if record.market != market.market {
        return Err(ReferenceError::ResolutionBindingMismatch);
    }
    {
        let mut full_terms = ZERO_TERMS;
        load_terms(plane.terms, &mut full_terms)?;
        record
            .binds_terms_fields(&full_terms)
            .map_err(|_| ReferenceError::ResolutionBindingMismatch)?;
    }
    native_kernel_invariants(
        state.kernel,
        market.outcome_count,
        hoard.collateral_atoms,
        plane.resolution,
    )?;

    if sequence != terms.repair_generation
        || sequence != candidate.repair_generation()
        || candidate.terms() != terms.terms
        || candidate.feed() != terms.feed
        || candidate.window() != plane.window_id
        || candidate.statistic() != terms.statistic
    {
        return Err(ReferenceError::ResolutionBindingMismatch);
    }

    let already_resolved = record.is_resolved();
    let expected = OccupationResolutionAccount {
        market: market.market,
        terms: terms.terms,
        feed: terms.feed,
        window: candidate.window(),
        feed_cursor: candidate.sealed_feed_cursor(),
        sealed_end_bucket_exclusive: candidate.end_bucket_exclusive(),
        repair_generation: candidate.repair_generation(),
        resolved_slot: if already_resolved {
            record.resolved_slot
        } else {
            plane.resolved_slot
        },
        mode: RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: market.outcome_count,
        resolved_value: 0,
        vector: candidate.vector(),
        archive_commitment: candidate.archive_commitment(),
        statistic: candidate.statistic(),
        finalization: candidate.finalization().wire_id(),
        basis_evaluator_version: candidate.basis_evaluator_version(),
        occupation_summary_version: candidate.occupation_summary_version(),
        sample_count: candidate.sample_count(),
        coverage_count: candidate.coverage_count(),
        gap_count: candidate.gap_count(),
        stored_bump: bumps.resolution,
        flags: 0,
        reserved: 0,
    };
    {
        let mut full_terms = ZERO_TERMS;
        load_terms(plane.terms, &mut full_terms)?;
        expected
            .binds_terms_fields(&full_terms)
            .map_err(|_| ReferenceError::ResolutionBindingMismatch)?;
    }
    let mut resolution_bytes = [0_u8; OCCUPATION_RESOLUTION_LEN];
    expected.encode(&mut resolution_bytes)?;

    match (market.lifecycle, kernel.phase, already_resolved) {
        (0, 0, false) => {
            let step = kernel_resolve_native(
                state.kernel,
                market.outcome_count,
                hoard.collateral_atoms,
                candidate.vector(),
            )?;
            check_market_closure(market.outcome_count, &supply, &step.total_supply)?;
            write_market_lifecycle(state.market, 1)?;
            write_kernel(state.kernel, &step)?;
            Ok(ResolveOutput {
                resolution: ResolutionWrite::Occupation(resolution_bytes),
                repeated: false,
            })
        }
        (1, 1, true) => {
            if kernel.resolved_payout != 0 || record != expected {
                return Err(ReferenceError::ResolutionBindingMismatch);
            }
            Ok(ResolveOutput {
                resolution: ResolutionWrite::Occupation(resolution_bytes),
                repeated: true,
            })
        }
        (0, 0, true) => Err(ReferenceError::ResolutionAlreadyRecorded),
        _ => Err(ReferenceError::MismatchedState),
    }
}

/// Market-wide aggregate closure, with no owner-local lower-bound check.
fn check_market_closure(
    outcome_count: u8,
    supply: &SupplyLedgerAccount,
    total_supply: &[u64; MAX_OUTCOMES],
) -> Gate<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        let aggregate = supply
            .aggregate_supply(outcome as u8)
            .map_err(|_| ReferenceError::Arithmetic)?;
        if aggregate != total_supply[outcome] {
            return Err(ReferenceError::AggregateClosureMismatch);
        }
        outcome += 1;
    }
    Ok(())
}

/// CLO-DELTA-V1 C1 and C2 against one presented triple.
fn check_closure(
    outcome_count: u8,
    supply: &SupplyLedgerAccount,
    total_supply: &[u64; MAX_OUTCOMES],
    internal: &[u64; MAX_OUTCOMES],
) -> Gate<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        let aggregate = supply
            .aggregate_supply(outcome as u8)
            .map_err(|_| ReferenceError::Arithmetic)?;
        if aggregate != total_supply[outcome] {
            return Err(ReferenceError::AggregateClosureMismatch);
        }
        if internal[outcome] > supply.internal_supply[outcome] {
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
#[inline(never)]
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
        Action::Resolve { payout_index } => {
            resolve_global(program_id, accounts, request.sequence, payout_index)
        }
        Action::RedeemInternal { outcome, quantity } => evidence_gated(
            program_id,
            accounts,
            request.sequence,
            RedeemAction { outcome, quantity },
        ),
        /* Every other layout intent belongs to another family module; the
         * router never sends one here, and this arm exists so that adding one
         * to the router is a compile error rather than a silent success. */
        Action::Layout(_) => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

#[inline(never)]
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

/// Authenticate and apply the market-global Resolve plane.
///
/// No account address in this function is derived from an owner, and no
/// Position or owner Replay account is accepted in the exact account count.
#[inline(never)]
fn resolve_global(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    payout_index: u8,
) -> Outcome<()> {
    require(
        accounts.len() >= OCCUPATION_RESOLVE_ACCOUNT_PREFIX,
        ClutchError::AccountCount,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &RESOLVE_STATE_ROLES)?;
    validate_open_roles(
        program_id,
        accounts,
        &[OpenRole {
            index: IX_RESOLVE_TERMS,
            len: account_len::TERMS,
        }],
    )?;
    let market = accounts::read_market(&accounts[IX_RESOLVE_MARKET].data.borrow())?;
    let terms = accounts::read_terms(&accounts[IX_RESOLVE_TERMS].data.borrow())?;
    let occupation = is_occupation_statistic(terms.statistic_id);
    require(
        !occupation || (1..=3).contains(&terms.basis_degree),
        ClutchError::NonCanonical,
    )?;
    let resolve_prefix = if occupation {
        OCCUPATION_RESOLVE_ACCOUNT_PREFIX
    } else {
        RESOLVE_ACCOUNT_PREFIX
    };
    require(
        accounts.len() == resolve_prefix + usize::from(market.outcome_count),
        ClutchError::AccountCount,
    )?;
    if !occupation {
        validate_buffer_role(
            program_id,
            &accounts[IX_RESOLVE_BUFFER],
            MAX_EVIDENCE_BUFFER_LEN,
            EVIDENCE_BUFFER_HEADER_BYTES,
        )?;
    }
    let resolution_len = if terms.basis_degree == 0 {
        account_len::RESOLUTION
    } else if occupation {
        OCCUPATION_RESOLUTION_LEN
    } else {
        NATIVE_RESOLUTION_LEN
    };
    validate_open_roles(
        program_id,
        accounts,
        &[OpenRole {
            index: IX_RESOLVE_RESOLUTION,
            len: resolution_len,
        }],
    )?;
    let feed = accounts::read_feed(&accounts[IX_RESOLVE_FEED].data.borrow())?;
    let market_bytes = market.market.bytes();
    let realm_bytes = market.realm.bytes();

    let market_pda = seeds::market_pda(program_id, &realm_bytes, &market_bytes);
    expect_pda(accounts[IX_RESOLVE_MARKET].key, market_pda, None)?;
    let hoard_pda = seeds::hoard_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_RESOLVE_HOARD].key, hoard_pda, None)?;
    expect_pda(
        accounts[IX_RESOLVE_KERNEL].key,
        seeds::kernel_pda(program_id, &market_bytes),
        None,
    )?;
    let supply_pda = seeds::supply_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_RESOLVE_SUPPLY].key, supply_pda, None)?;
    let terms_pda = seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes());
    expect_pda(accounts[IX_RESOLVE_TERMS].key, terms_pda, None)?;
    let resolution_pda = seeds::resolution_pda(program_id, &market_bytes);
    expect_pda(accounts[IX_RESOLVE_RESOLUTION].key, resolution_pda, None)?;
    expect_pda(
        accounts[IX_RESOLVE_FEED].key,
        seeds::feed_pda(program_id, &feed.feed.bytes()),
        Some(feed.stored_bump),
    )?;
    require(feed.feed == market.feed, ClutchError::MismatchedState)?;

    /* The expected source/window addresses come only from digest-bound market
     * terms.  The hostile compatibility projection is not read until both
     * canonical accounts have been authenticated. */
    let expected_window = expected_window_domain(
        &accounts[IX_RESOLVE_MARKET].data.borrow(),
        &accounts[IX_RESOLVE_TERMS].data.borrow(),
    )?;
    let expected_window_id = source_archive::canonical_window_id(expected_window);
    let source_spec_pda = seeds::source_spec_pda(program_id, &market.feed.bytes());
    let source_archive_pda = seeds::source_archive_pda(
        program_id,
        &market.feed.bytes(),
        &expected_window_id.bytes(),
    );
    expect_pda(accounts[IX_RESOLVE_SOURCE_SPEC].key, source_spec_pda, None)?;
    expect_pda(
        accounts[IX_RESOLVE_SOURCE_ARCHIVE].key,
        source_archive_pda,
        None,
    )?;
    let source_spec_data = accounts[IX_RESOLVE_SOURCE_SPEC].data.borrow();
    let verified_spec = source_archive::verify_source_spec_account(
        program_id.to_bytes(),
        source_spec_pda.0.to_bytes(),
        SourceSpecAccountViewV1::new(
            accounts[IX_RESOLVE_SOURCE_SPEC].key.to_bytes(),
            accounts[IX_RESOLVE_SOURCE_SPEC].owner.to_bytes(),
            accounts[IX_RESOLVE_SOURCE_SPEC].executable,
            &source_spec_data,
        ),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::ResolutionEvidenceUnavailable))?;
    require(
        verified_spec.feed() == market.feed && verified_spec.stored_bump() == source_spec_pda.1,
        ClutchError::MismatchedState,
    )?;
    let source_archive_data = accounts[IX_RESOLVE_SOURCE_ARCHIVE].data.borrow();
    let source_archive_view = ArchiveAccountViewV1::new(
        accounts[IX_RESOLVE_SOURCE_ARCHIVE].key.to_bytes(),
        accounts[IX_RESOLVE_SOURCE_ARCHIVE].owner.to_bytes(),
        accounts[IX_RESOLVE_SOURCE_ARCHIVE].executable,
        &source_archive_data,
    );
    let verified_archive: VerifiedSealedArchiveViewV1<'_> =
        source_archive::verify_recorded_sealed_archive_view(
            program_id.to_bytes(),
            source_archive_pda.0.to_bytes(),
            source_archive_view,
            verified_spec,
            expected_window,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::ResolutionEvidenceUnavailable))?;
    let archive_receipt = verified_archive.receipt();
    require(
        archive_receipt.feed() == market.feed
            && archive_receipt.window() == expected_window_id
            && archive_receipt.stored_bump() == source_archive_pda.1
            && feed.cursor >= archive_receipt.sealed_feed_cursor(),
        ClutchError::MismatchedState,
    )?;

    let observed = claim_truth::observe_outcome_mints(
        program_id,
        accounts,
        resolve_prefix,
        *accounts[IX_RESOLVE_MARKET].key,
        market.market,
        market.outcome_count,
        None,
    )?;
    let terms_data = accounts[IX_RESOLVE_TERMS].data.borrow();
    let occupation_candidate = if occupation {
        Some(derive_occupation_candidate(&terms_data, verified_archive)?)
    } else {
        None
    };
    let buffer = if occupation {
        None
    } else {
        Some(accounts[IX_RESOLVE_BUFFER].data.borrow())
    };
    let evidence = if let Some(bytes) = &buffer {
        let evidence = read_evidence_buffer(bytes)?;
        if evidence.window_id != archive_receipt.window() {
            return Err(ReferenceError::ResolutionBindingMismatch.into());
        }
        require_archive_projection(
            archive_receipt,
            source_archive_view,
            evidence.window,
            expected_window,
        )?;
        Some(evidence)
    } else {
        None
    };
    let resolution_data = accounts[IX_RESOLVE_RESOLUTION].data.borrow();
    let plane = EvidencePlane {
        terms: &terms_data,
        resolution: &resolution_data,
        window: evidence.map_or(&[], |value| value.window),
        window_id: archive_receipt.window(),
        feed_cursor: archive_receipt.sealed_feed_cursor(),
        resolved_slot: 0,
        terms_writable: accounts[IX_RESOLVE_TERMS].is_writable,
        resolution_writable: accounts[IX_RESOLVE_RESOLUTION].is_writable,
    };
    let output = {
        let mut market_data = borrow_mut!(accounts[IX_RESOLVE_MARKET])?;
        let hoard_data = accounts[IX_RESOLVE_HOARD]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut kernel_data = borrow_mut!(accounts[IX_RESOLVE_KERNEL])?;
        let mut supply_data = borrow_mut!(accounts[IX_RESOLVE_SUPPLY])?;
        let mut state = ResolveStateSlices {
            market: &mut market_data,
            hoard: &hoard_data,
            kernel: &mut kernel_data,
            supply: &mut supply_data,
        };
        apply_market_resolution(
            &mut state,
            &plane,
            &ResolveBumps {
                market: market_pda.1,
                hoard: hoard_pda.1,
                supply: supply_pda.1,
                terms: terms_pda.1,
                resolution: resolution_pda.1,
            },
            occupation_candidate.as_ref(),
            accounts[IX_RESOLVE_ACTOR].is_signer,
            sequence,
            payout_index,
        )?
    };
    drop(resolution_data);
    drop(terms_data);
    drop(buffer);
    drop(source_archive_data);
    drop(source_spec_data);

    if !output.repeated {
        /* Direct holder burns are synchronized only after the semantic gate.
         * A later synchronization refusal is intentionally a late failure;
         * SVM atomicity must roll the earlier lifecycle writes back. */
        {
            let mut supply_data = borrow_mut!(accounts[IX_RESOLVE_SUPPLY])?;
            let mut kernel_data = borrow_mut!(accounts[IX_RESOLVE_KERNEL])?;
            claim_truth::synchronize_external_truth(
                &mut supply_data,
                &mut kernel_data,
                market.market,
                market.realm,
                market.outcome_count,
                &observed,
            )?;
        }
        let mut record = borrow_mut!(accounts[IX_RESOLVE_RESOLUTION])?;
        record.copy_from_slice(output.resolution.bytes());
    } else {
        /* Exact repeats are observationally idempotent, including the cached
         * external-supply plane. A holder burn after resolution is handled by
         * an actual claim transition, not smuggled into a Resolve replay. */
        let supply = SupplyLedgerAccount::decode(&accounts[IX_RESOLVE_SUPPLY].data.borrow())?;
        let mut outcome = 0_usize;
        while outcome < usize::from(market.outcome_count) {
            require(
                supply.external_supply[outcome] == observed.values[outcome],
                ClutchError::ShadowSupplyMismatch,
            )?;
            outcome += 1;
        }
    }

    let observed_after = claim_truth::observe_outcome_mints(
        program_id,
        accounts,
        resolve_prefix,
        *accounts[IX_RESOLVE_MARKET].key,
        market.market,
        market.outcome_count,
        None,
    )?;
    claim_truth::require_exact_mint_vector_delta(&observed, &observed_after, None)?;
    Ok(())
}

/// Keep the large hostile Terms decode and bounded occupation summary out of
/// the account-plane frame.  The no-inline boundary is part of the measured
/// SBF frame discipline, not a relaxation of any binding check.
#[inline(never)]
fn derive_occupation_candidate(
    terms_data: &[u8],
    verified_archive: VerifiedSealedArchiveViewV1<'_>,
) -> Gate<NativeWindowPreflightV1> {
    let mut terms = ZERO_TERMS;
    load_terms(terms_data, &mut terms)?;
    native_window::preflight_verified_archive(&terms, verified_archive)
        .map_err(|_| ReferenceError::ResolutionEvidenceUnavailable)
}

#[inline(never)]
fn evidence_gated(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    action: RedeemAction,
) -> Outcome<()> {
    let redeems = true;
    require(
        accounts.len()
            >= if redeems {
                REDEEM_ACCOUNT_PREFIX
            } else {
                EVIDENCE_ACCOUNT_PREFIX
            },
        ClutchError::AccountCount,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &EVIDENCE_STATE_ROLES)?;
    validate_open_roles(
        program_id,
        accounts,
        &[OpenRole {
            index: IX_TERMS,
            len: account_len::TERMS,
        }],
    )?;
    let terms = accounts::read_terms(&accounts[IX_TERMS].data.borrow())?;
    let resolution_len = if terms.basis_degree == 0 {
        account_len::RESOLUTION
    } else if is_occupation_statistic(terms.statistic_id) {
        OCCUPATION_RESOLUTION_LEN
    } else {
        NATIVE_RESOLUTION_LEN
    };
    validate_open_roles(
        program_id,
        accounts,
        &[OpenRole {
            index: IX_RESOLUTION,
            len: resolution_len,
        }],
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
    let mint_prefix = if redeems {
        IX_REDEEM_OUTCOME_MINTS
    } else {
        IX_RESOLVE_OUTCOME_MINTS
    };
    require(
        accounts.len() == mint_prefix + usize::from(market.outcome_count),
        ClutchError::AccountCount,
    )?;
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

    let observed = claim_truth::observe_outcome_mints(
        program_id,
        accounts,
        mint_prefix,
        *accounts[IX_MARKET].key,
        market.market,
        market.outcome_count,
        None,
    )?;
    {
        let mut supply_data = borrow_mut!(accounts[IX_SUPPLY])?;
        let mut kernel_data = borrow_mut!(accounts[IX_KERNEL])?;
        claim_truth::synchronize_external_truth(
            &mut supply_data,
            &mut kernel_data,
            market.market,
            market.realm,
            market.outcome_count,
            &observed,
        )?;
    }

    let actor = Actor {
        key: Hash32::from_bytes(accounts[IX_ACTOR].key.to_bytes()),
        signer: accounts[IX_ACTOR].is_signer,
    };
    let bumps = Bumps {
        market: market_pda.1,
        hoard: hoard_pda.1,
        position: position_pda.1,
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

    {
        let mut market_data = borrow_mut!(accounts[IX_MARKET])?;
        let mut hoard_data = borrow_mut!(accounts[IX_HOARD])?;
        let mut position_data = borrow_mut!(accounts[IX_POSITION])?;
        let mut kernel_data = borrow_mut!(accounts[IX_KERNEL])?;
        let mut replay_data = borrow_mut!(accounts[IX_REPLAY])?;
        let mut supply_data = borrow_mut!(accounts[IX_SUPPLY])?;
        let mut state = StateSlices {
            market: &mut market_data,
            hoard: &mut hoard_data,
            position: &mut position_data,
            kernel: &mut kernel_data,
            replay: &mut replay_data,
            supply: &mut supply_data,
        };
        apply_evidence_transition(&mut state, &plane, &bumps, actor, sequence, action)?;
    }

    /* Every borrow the evidence plane held is released before the CPI: a live
     * `RefCell` borrow across `invoke` is a runtime failure, not a lint. */
    drop(terms_data);
    drop(buffer);
    drop(resolution_data);

    /* Redemption changes *which ledger term* owns collateral already retained
     * by the pooled Hoard: locked complete-set backing falls and this
     * position's cash rises by the same payout.  It is not a withdrawal and
     * therefore must move exactly zero Token-2022 atoms. */
    if let Some(leg) = leg {
        let post_actor = token::token_amount(&accounts[IX_ACTOR_TOKEN])?;
        let post_hoard = token::token_amount(&accounts[IX_HOARD_TOKEN])?;
        token::require_exact_credit(leg.actor_amount, post_actor, 0)?;
        token::require_exact_credit(leg.hoard_amount, post_hoard, 0)?;
        let collateral_atoms =
            HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?.collateral_atoms;
        token::require_hoard_covers_collateral(collateral_atoms, post_hoard)?;
    }
    let observed_after = claim_truth::observe_outcome_mints(
        program_id,
        accounts,
        mint_prefix,
        *accounts[IX_MARKET].key,
        market.market,
        market.outcome_count,
        None,
    )?;
    claim_truth::require_exact_mint_vector_delta(&observed, &observed_after, None)?;
    {
        let mut supply_data = borrow_mut!(accounts[IX_SUPPLY])?;
        let kernel_data = accounts[IX_KERNEL]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        claim_truth::commit_observed_supplies(
            &mut supply_data,
            &kernel_data,
            market.market,
            market.realm,
            market.outcome_count,
            &observed_after,
        )?;
    }
    Ok(())
}

// The historical differential below carries the deleted owner-local External
// account.  Keep it as migration archaeology until an external-truth-aware
// reference state replaces that DTO; do not compile it as bearer-plane proof.
#[cfg(any())]
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
