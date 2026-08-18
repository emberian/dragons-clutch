//! `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SettlePage`.
//!
//! This module owns the batch-auction plane's account lists.  Exactly one of
//! its three intents is implemented: [`Intent::PlaceOrder`] appends one
//! single-Egg record to an open order page.  The other two refuse, each for a
//! *measured or structural* reason written down below rather than for "not
//! written yet", and neither reads an account.
//!
//! | intent | this wave |
//! | --- | --- |
//! | `PlaceOrder` | **implemented** for the single-Egg family; the wire cannot express any other |
//! | `CancelOrder` | **refused**: the frozen page format has no cancellation representation (§ *Cancellation*) |
//! | `SettlePage` | **refused**: `clutch_batch::relation_v1::verify` is measured at a 39,104-byte SBF frame (§ *Settlement*) |
//!
//! Nothing here computes a clearing price, selects a candidate, or moves
//! collateral.  A placement writes one order record into one page and updates
//! that page's commitment; it is bookkeeping under a frozen codec, not an
//! economic transition, and the collateral gap that implies is named in
//! § *Named gaps*.
//!
//! ## The account list of `PlaceOrder`
//!
//! Four accounts, exact count, no remaining-account tail:
//!
//! | index | account | role |
//! | --- | --- | --- |
//! | 0 | actor | signer; its key **is** the order's 32-byte `owner` |
//! | 1 | epoch | read-only; owns the phase, the outcome width, and the grid identity |
//! | 2 | price grid | read-only; owns the admitted limit prices |
//! | 3 | order page | writable; the page the record is appended to |
//!
//! The market, realm, and profile accounts are deliberately **absent**, for the
//! same reason [`super::observe_resolve`] omits realm and profile: the epoch
//! already names this market, this grid, and this outcome width, and
//! [`clutch_solana_layout::EpochAccount::validate`] recomputes the epoch
//! identity from `(market, epoch_index)`, so three more accounts and three more
//! decodes would add transaction weight and no fact.  The uniformity cost is an
//! ABI decision for whoever freezes the account schema, not this lane's.
//!
//! Every address is recomputed from [`crate::seeds`] out of the accounts' own
//! decoded bytes; no caller-supplied expected key is accepted anywhere.
//!
//! ## The order of the checks
//!
//! There is no offline oracle for this family — `clutch_solana_reference::apply`
//! refuses `PlaceOrder` with `Error::UnsupportedIntent` — so the order below is
//! this lane's, and the host tests at the bottom of this file pin it.  The
//! oracles that do exist are the frozen codec's own adversarial fixtures, and
//! every page refusal here is asserted to be *exactly* the verdict
//! [`clutch_solana_layout::stream::verify_page`] gives on the same bytes.
//!
//! 1. account count, actor signature, role aliasing, program ownership, the
//!    executable bit, declared writability, exact data lengths;
//! 2. every address, against [`crate::seeds`].  The page's addressing fields
//!    are read with [`clutch_solana_layout::stream::OrderPageHeader::decode`],
//!    which reads the fixed 235-byte header and folds no digest, because an
//!    address comparison needs `epoch`, `page_index`, and `stored_bump` and
//!    nothing else; the page's *verdict* is step 3;
//! 3. the page, in full, through the streaming decoder **on the frozen grid**
//!    ([`clutch_solana_layout::stream::verify_page_on_grid`]) — so a page whose
//!    stored digest, order-id chain, slot padding, stored range, or existing
//!    limits are not canonical is refused before anything is decided about the
//!    new record;
//! 4. the epoch is `EPOCH_PHASE_OPEN` and the page is unfrozen;
//! 5. the page belongs to this epoch and this market, and the grid is the one
//!    the epoch names, at the scale the epoch names;
//! 6. the intent's own `market` and `epoch` name the same epoch;
//! 7. replay: the request sequence equals the page's current `order_count`;
//! 8. the actor is the record's owner;
//! 9. the record is valid ([`clutch_solana_layout::OrderRecord::validate`]) and
//!    its outcome is inside the *epoch's* width, which no page can bound;
//! 10. the record's limit is an exact member of the frozen tick vector;
//! 11. the page has a free slot, and the record's order id is strictly above
//!     the page's last — or, on an empty page, above the predecessor page's
//!     last;
//! 12. write, then re-verify (below).
//!
//! ## Writing bytes this crate does not own — and the debt that is
//!
//! `clutch-solana-layout` publishes a streaming page **reader** and no
//! streaming page **writer**.  `OrderPageAccount::encode` is a method on a
//! whole decoded page — `size_of::<OrderPageAccount>()` is 4,080 bytes, over
//! the frame maximum before the output buffer is counted — and `encode_slot`,
//! `Writer`, and `put_header` are private, so there is no public per-slot or
//! per-header encoder at all.  An on-chain placement therefore cannot ask the
//! owning crate to write its bytes.  This is the write-side twin of the
//! page-decode blocker the streaming lane closed on the read side, and it was
//! not visible until an instruction tried to write a page.
//!
//! This module therefore writes four regions of the page itself — one slot,
//! `order_count`, the stored range, and `page_digest` — and pays for that with
//! a per-execution proof rather than with confidence:
//!
//! * the byte offsets are a chain of `const`s whose end is `const`-asserted
//!   equal to [`clutch_solana_layout::stream::ORDER_PAGE_HEADER_BYTES`],
//!   [`clutch_solana_layout::ORDER_RECORD_BYTES`], and
//!   [`clutch_solana_layout::ORDER_SLOT_BYTES`], so a layout that moves a field
//!   without changing a width is the only drift a compile can miss;
//! * the digest written is [`clutch_solana_layout::stream::streamed_page_digest`]
//!   over the already-mutated bytes, so the digest is the layout crate's own
//!   fold and never this module's arithmetic;
//! * after the write, the page is verified **again** with
//!   [`clutch_solana_layout::stream::verify_page`], its returned header is
//!   compared field for field against the intended post-state, and the written
//!   slot is decoded back through
//!   [`clutch_solana_layout::stream::OrderSlotCursor`] and compared to the
//!   intended record.  Any disagreement refuses, and a refusing instruction
//!   discards every account write it made;
//! * `place_order_writes_exactly_what_the_layout_encoder_would` compares the
//!   post-state, byte for byte, against a page built and encoded by
//!   `OrderPageAccount::encode`, which is the golden reference.
//!
//! That is fail-closed, and it is still debt.  **The fix belongs in
//! `clutch-solana-layout`**: a `stream::write_single_slot(&mut [u8], index,
//! &OrderRecord)` and a `stream::seal_page(&mut [u8], order_count, first, last)`
//! beside the streaming readers, at which point this module's offset table and
//! its post-write re-verification both delete.  Until then the offsets live in
//! two crates, which is exactly the shape of mistake the layout crate exists to
//! prevent.
//!
//! ## Cancellation
//!
//! **The frozen page format has no cancellation representation.**  This is a
//! reading of the codec, not an implementation gap, and it is why
//! `CancelOrder` refuses rather than guessing:
//!
//! * there is no tombstone slot kind — [`clutch_solana_layout::OrderSlot`] is
//!   `Empty | Single | Portfolio`, and `Empty` below `order_count` is refused as
//!   a *missing* order (`CodecError::ZeroIdentity`), not as a cancelled one;
//! * there is no status or generation field on a record, and no reserved flag
//!   bit can carry one: `OrderRecord::validate` refuses `flags & !1 != 0`, so
//!   bit 0 (all-or-none) is the only defined bit and every other is refused;
//! * `clutch_batch::relation_v1` has no cancelled-order concept either — its
//!   `NormalizedBookV1::cancelled` is `N-b` self-cross netting, and
//!   `expiry_epoch` is not persisted by any record.
//!
//! The one representation the bytes *do* admit is delete-and-compact: drop the
//! slot, shift the tail down, decrement `order_count`, restate the stored
//! range, re-fold the digest.  That is a canonical page afterwards, and it is
//! still not implemented here, for three reasons that are not this module's to
//! decide:
//!
//! 1. it is sound only when `page_count == 1`.  Removing the last record of a
//!    non-final page changes that page's `last_order_id`, which is stored again
//!    in the *next* page's `prev_page_last_order_id`; a single-page instruction
//!    cannot repair the chain, and
//!    [`clutch_solana_layout::stream::verify_page_set`] refuses the set
//!    afterwards.
//! 2. it makes a multi-page book unfreezable.  `validate_commitments` requires
//!    every non-final page of a frozen set to hold exactly
//!    [`clutch_solana_layout::MAX_ORDERS_PER_PAGE`] records, so a hole left in
//!    page 0 can only be closed by a cross-page compaction that no instruction
//!    in this program performs.
//! 3. choosing removal over a tombstone *is* the cancellation semantics —
//!    whether a cancelled order leaves a trace, whether its id is retired, and
//!    what a candidate that referenced it sees.  Freezing that from inside an
//!    instruction is precisely the drift the layering rule forbids.
//!
//! So the disposition is: **cancellation needs a layout decision**, and the
//! smallest one that unblocks it is a fourth `OrderSlot` kind (a tombstone
//! carrying the retired order id and its owner) plus a page rule that a
//! tombstone occupies a slot below `order_count` and contributes to the
//! order-id chain but not to the relation's book.
//!
//! ## Settlement, and the frame budget of the relation — measured
//!
//! `SettlePage` refuses because the relation does not fit an SBF frame, and
//! that is measured on the pinned `cargo-build-sbf` (platform-tools frame
//! diagnostics, the same method the streaming lane used), against a 4,096-byte
//! per-frame maximum:
//!
//! | function | estimated frame |
//! | --- | --- |
//! | `clutch_batch::relation_v1::canonical_candidate` | 45,824 |
//! | `clutch_batch::relation_v1::verify_inner` (what `verify` is) | **39,104** |
//! | `clutch_batch::relation_v1::canonical_pairing` | 38,016 |
//! | `clutch_batch::relation_v1::propose_best_valid` | 25,472 |
//! | `clutch_batch::relation_v1::participation_from_fills` | 24,704 |
//! | `clutch_batch::relation_v1::verify_pairing_witness` | 23,872 |
//! | `clutch_batch::relation_v1::normalize` | 12,160 |
//! | `clutch_batch::relation_v1::check_explicit_slices` | 8,832 |
//! | `clutch_batch::relation_v1::settle_cash` | 6,080 |
//!
//! A caller fares no better: a probe whose whole body is `BookV1::empty()`,
//! `CandidateV1::empty()`, `verify(..)` is reported at 23,168 bytes, and the
//! `entrypoint` that calls it at 22,976.
//!
//! The type widths behind those numbers are the cause, and none of them is
//! target-dependent — every field is a fixed-width integer or a fieldless enum:
//!
//! | type | bytes |
//! | --- | --- |
//! | `BookV1` (`[OrderV1; 64]`) | 11,272 |
//! | `NormalizedBookV1` | 11,912 |
//! | `ParticipationV1` | 16,384 |
//! | `PairingWitnessV1` (`[PairingSliceV1; 416]`) | 6,664 |
//! | `SummaryV1` | 1,184 |
//! | `CandidateV1` | 752 |
//! | `RelationDomainV1` | 88 |
//!
//! `OrderV1` is 176 bytes because `PortfolioOrderV1` carries a
//! `[u64; MAX_OUTCOMES]` coefficient vector, and a 64-order book carries 64 of
//! them whether or not any order is a portfolio.  `verify_inner` holds a
//! `BookV1`, the `NormalizedBookV1` it derives, and a `ParticipationV1` at
//! once: 11,272 + 11,912 + 16,384 is already 39,568, which is the measured
//! 39,104 to within the compiler's slot reuse.
//!
//! **Finding for the design queue.** On-chain candidate verification needs a
//! *streaming or resumable* relation API in `clutch-batch`, in the same shape
//! and for the same reason as the page-decode finding the streaming lane
//! closed: an interface whose working set is one order (176 bytes), not one
//! book (11 KB).  Nothing in this program can route around it, and no
//! arrangement of `#[inline(never)]` frames helps, because the large values are
//! single locals rather than a composition of small ones.  A second,
//! independent blocker sits behind it and is *not* a frame question: the
//! projection from the persisted page format to `BookV1` is undefined on-chain.
//! `docs/implementation/SOLANA_LAYOUT.md` states `canonical_order_id` as "the
//! record's rank in the verified page set, plus one" — which needs the whole
//! frozen set at once — while the `u16` owner tag is "the adapter's owner-tag
//! image" with nothing proving it a bijection into `EpochAccount.owner_count`,
//! and `expiry_epoch` "is not persisted by any record".  Two of the three
//! coordinates of every relation order therefore have no on-chain source.
//!
//! ## What the frozen wire cannot express
//!
//! [`Intent::PlaceOrder`] carries an [`clutch_solana_layout::OrderRecord`], not
//! an [`clutch_solana_layout::OrderSlot`], and its encoded length is
//! `2 + 32 + 32 + ORDER_RECORD_BYTES`.  **A portfolio placement is not
//! expressible on the frozen intent wire at all** — a page can hold
//! `OrderSlot::Portfolio` records and no intent can put one there.  This is not
//! an unimplemented variant that could be added here; it is a wire gap, and it
//! is pinned by `the_place_order_wire_cannot_carry_a_portfolio_record`.  The
//! fix is an `Intent` revision (a portfolio placement tag carrying a
//! `PortfolioRecord`), which is `clutch-solana-layout`'s to make.
//!
//! ## Named gaps
//!
//! Each of these is a fact about what an accepted placement does *not* do.
//!
//! * **No collateral is reserved.**  The relation's
//!   `opening_reserved_cash_price_units` and `opening_reserved_egg` are
//!   admission-time reservations, and this instruction binds no position, no
//!   hoard, and no supply ledger.  An order placed by this program is unfunded.
//!   Nothing downstream is misled today — no instruction freezes an epoch and
//!   `SettlePage` refuses — but a reservation seam must land before either does.
//! * **Nothing moves an epoch out of `EPOCH_PHASE_OPEN`.**  No intent in the
//!   frozen wire freezes a page set, so a book placed by this program can never
//!   be closed, and `page_count`, `set_order_count`, and `order_set` stay zero
//!   forever.
//! * **Order ids are caller-chosen.**  `clutch-solana-layout` publishes no
//!   `canonical_order_id` derivation; the page's only rule is nonzero and
//!   strictly increasing.  A single caller can therefore place one order with
//!   an id of `0xff..ff` and no further order can ever be placed on that page
//!   or on any later page of the set.  That is a griefing vector, it is
//!   unpriced, and closing it is a layout decision (a derivation the page
//!   enforces), not an adapter check.
//! * **`generation` is written and never read.**  The layout documents it as
//!   "replay protection for the placing instruction"; the replay counter this
//!   instruction actually uses is the page's own `order_count`, which is state
//!   rather than a caller assertion.  Nothing binds `generation` to a position
//!   generation, because no position account is in the list.
//! * **The page set's shape is unbound while open.**  `EpochAccount::validate`
//!   forces `page_count == 0` on an open epoch, so nothing cross-checks one
//!   page's `page_count` against another's, or against the epoch's, until the
//!   set is frozen.  Page creation and page-set growth have no instruction at
//!   all.
//! * **Distinct-owner admission is unchecked.**  `EpochAccount.owner_count`
//!   bounds the relation's owner tags, and the identity-to-tag interning is
//!   unspecified (above), so this instruction cannot tell whether one more
//!   distinct owner is admissible.
//!
//! ## Frames and compute
//!
//! No function in this module holds a page, a book, or a candidate.  The
//! largest values in flight are a decoded `PriceGridAccount` (a
//! `[u64; MAX_GRID_TICKS]` tick vector) and two
//! [`clutch_solana_layout::stream::OrderPageHeader`] values at 234 bytes each;
//! the grid is loaded into a caller slot through an `#[inline(never)]`
//! out-parameter rather than returned, for the reason
//! [`super::observe_resolve`] records.  `cargo-build-sbf` reports no frame
//! diagnostic for any `clutch_sbf` function, which is the only check there is.
//!
//! Compute is **not** measured, and the structure says it is not small.  One
//! accepted placement performs three folds of the page preimage through the
//! layout crate's *software* SHA-256 — the pre-state verify, the post-write
//! digest, and the post-write re-verify — over a 3,743-byte preimage each,
//! which is 59 compression blocks per fold and 177 per placement, plus three
//! `sol_try_find_program_address` calls, two `EpochAccount` decodes (each
//! recomputing the canonical epoch identity), two `PriceGridAccount` decodes
//! (each recomputing the grid digest over its 585-byte body) plus a third
//! grid-digest recompute inside `verify_page_on_grid`, and seven walks of the
//! slot array — the three digest folds above, the two record-semantics sweeps
//! inside the two `verify_page` calls, the grid sweep, and the partial cursor
//! walk that reads the written slot back.  The repeated decodes are the same shape
//! [`super::observe_resolve`] records and have the same cause: each check must
//! run in its own small frame and at its own point in the order, and the
//! layout crate publishes no facts-only or `decode_unchecked` entry point.
//! The one measured comparison point in this repository is `Split` at 72,869
//! compute units with eight address derivations and no page fold, so a
//! placement is expected to need an explicit compute-budget request rather than
//! to fit the 200,000-unit default.  **That expectation is an obligation to
//! measure, not a measurement**; measuring it means a `PlaceOrder` fixture in
//! the differential harness, which is that lane's file.
//!
//! ## Refusal codes
//!
//! `0x0060-0x006f` is reserved for this module and **this wave allocates none
//! of it**, following [`super::observe_resolve`]'s precedent: every refusal
//! raised here already has an owner.  A page that is not a page refuses with
//! the frozen codec's own [`clutch_solana_layout::CodecError`]; a full page
//! refuses `CodecError::InvalidCount`, which is literally what
//! `OrderPageHeader::validate_head` would say about the resulting count; the
//! account plane refuses with an existing [`crate::error::ClutchError`].  The
//! proposed allocation, for whoever unfreezes `error.rs`:
//!
//! | class | proposed code |
//! | --- | --- |
//! | order cancellation has no representation in the frozen page format | `0x0060` |
//! | candidate verification does not fit an SBF frame | `0x0061` |
//! | `0x0062-0x006f` | unallocated |
//!
//! Until `error.rs` is unfrozen, `CancelOrder` and `SettlePage` both refuse
//! [`crate::error::ClutchError::NotYetImplemented`] and are distinguishable
//! only by this file — a lossy numeric projection of exactly the kind
//! `reference_code` already has, and the reason each refusal's real cause is
//! written out above rather than left to a code.  The portfolio wire gap needs
//! no code at all: it is unreachable, not refusable.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_solana_layout::{
    account_len, stream, CodecError, Hash32, Intent, OrderRecord, OrderSlot, PriceGridAccount,
    EPOCH_PHASE_OPEN, MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, ORDER_KIND_SINGLE, ORDER_RECORD_BYTES,
    ORDER_SLOT_BYTES,
};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Borrow one account's data mutably, or refuse.
///
/// A macro rather than a function for the reason [`super::observe_resolve`]
/// records: `AccountInfo` is invariant in its lifetime.
macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

/* ------------------------------------------------------------------------ */
/* Account list                                                              */
/* ------------------------------------------------------------------------ */

/// Accounts in a `PlaceOrder` instruction, exactly.
pub const PLACE_ORDER_ACCOUNT_COUNT: usize = 4;

/// Authenticated actor; its key is the order's owner identity.
pub const IX_ACTOR: usize = 0;
/// Epoch/book-domain account.
pub const IX_EPOCH: usize = 1;
/// Frozen price-grid account.
pub const IX_GRID: usize = 2;
/// The order page the record is appended to.
pub const IX_PAGE: usize = 3;

/// Program-owned roles of `PlaceOrder`, in account-index order.
const PLACE_ORDER_STATE_ROLES: [StateRole; 3] = [
    StateRole::read_only(IX_EPOCH, account_len::EPOCH),
    StateRole::read_only(IX_GRID, account_len::PRICE_GRID),
    StateRole::writable(IX_PAGE, account_len::ORDER_PAGE),
];

/* ------------------------------------------------------------------------ */
/* Page byte offsets — see the module docs on writing bytes this crate does   */
/* not own.  Every offset is a chain from the previous field's width, and the */
/* chain's end is const-asserted against the layout crate's own constant.     */
/* ------------------------------------------------------------------------ */

/// Bytes of the account discriminator and schema version, which are never
/// rewritten: a placement can only ever touch a page that already decoded.
const OFF_TAG_AND_VERSION: usize = 2;
/// `market`.
const OFF_MARKET: usize = OFF_TAG_AND_VERSION;
/// `epoch`.
const OFF_EPOCH: usize = OFF_MARKET + 32;
/// `order_set`.
const OFF_ORDER_SET: usize = OFF_EPOCH + 32;
/// `page_digest`.
const OFF_PAGE_DIGEST: usize = OFF_ORDER_SET + 32;
/// `first_order_id`.
const OFF_FIRST_ORDER_ID: usize = OFF_PAGE_DIGEST + 32;
/// `last_order_id`.
const OFF_LAST_ORDER_ID: usize = OFF_FIRST_ORDER_ID + 32;
/// `prev_page_last_order_id`.
const OFF_PREV_PAGE_LAST_ORDER_ID: usize = OFF_LAST_ORDER_ID + 32;
/// `page_index`.
const OFF_PAGE_INDEX: usize = OFF_PREV_PAGE_LAST_ORDER_ID + 32;
/// `page_count`.
const OFF_PAGE_COUNT: usize = OFF_PAGE_INDEX + 2;
/// `set_order_count`.
const OFF_SET_ORDER_COUNT: usize = OFF_PAGE_COUNT + 2;
/// `order_count`.
const OFF_ORDER_COUNT: usize = OFF_SET_ORDER_COUNT + 2;
/// `frozen`.
const OFF_FROZEN: usize = OFF_ORDER_COUNT + 1;
/// `stored_bump`.
const OFF_STORED_BUMP: usize = OFF_FROZEN + 1;

/* The header chain closes exactly where the layout crate says the slot array
 * begins.  A field width that changed without a field moving is the only
 * drift this cannot see, and the byte-for-byte differential against
 * `OrderPageAccount::encode` is what covers that. */
const _: () = assert!(OFF_STORED_BUMP + 1 == stream::ORDER_PAGE_HEADER_BYTES);

/// Slot-relative offset of a single-Egg record's `owner`, after the kind byte.
const SLOT_OFF_OWNER: usize = 1;
/// Slot-relative offset of `order_id`.
const SLOT_OFF_ORDER_ID: usize = SLOT_OFF_OWNER + 32;
/// Slot-relative offset of `outcome`.
const SLOT_OFF_OUTCOME: usize = SLOT_OFF_ORDER_ID + 32;
/// Slot-relative offset of `side`.
const SLOT_OFF_SIDE: usize = SLOT_OFF_OUTCOME + 1;
/// Slot-relative offset of `quantity`.
const SLOT_OFF_QUANTITY: usize = SLOT_OFF_SIDE + 1;
/// Slot-relative offset of `limit`.
const SLOT_OFF_LIMIT: usize = SLOT_OFF_QUANTITY + 8;
/// Slot-relative offset of `minimum_fill`.
const SLOT_OFF_MINIMUM_FILL: usize = SLOT_OFF_LIMIT + 8;
/// Slot-relative offset of `flags`.
const SLOT_OFF_FLAGS: usize = SLOT_OFF_MINIMUM_FILL + 8;
/// Slot-relative offset of `generation`.
const SLOT_OFF_GENERATION: usize = SLOT_OFF_FLAGS + 1;

/* The record chain closes exactly at the record width the layout crate
 * publishes, and a record still fits inside one common-width slot. */
const _: () = assert!(SLOT_OFF_GENERATION + 8 == 1 + ORDER_RECORD_BYTES);
const _: () = assert!(ORDER_RECORD_BYTES < ORDER_SLOT_BYTES);

/* ------------------------------------------------------------------------ */
/* Frame-bounded readers                                                     */
/* ------------------------------------------------------------------------ */

/// An all-zero price grid, used only to give [`load_grid`] a caller slot.
const ZERO_GRID: PriceGridAccount = PriceGridAccount {
    grid: Hash32::ZERO,
    realm: Hash32::ZERO,
    price_scale: 0,
    tick_count: 0,
    ticks: [0; MAX_GRID_TICKS],
    stored_bump: 0,
    flags: 0,
};

/// Decode the frozen grid **into** a caller slot.
///
/// Returning a `Result<PriceGridAccount, _>` would cost the caller two tick
/// vectors instead of one; this is the out-parameter shape
/// [`super::observe_resolve`] measured and adopted.
#[inline(never)]
fn load_grid(bytes: &[u8], out: &mut PriceGridAccount) -> Outcome<()> {
    *out = PriceGridAccount::decode(bytes)?;
    Ok(())
}

/// Verify one order page on the frozen grid, streaming, and return its header.
///
/// This is the whole pre-state page check: framing, slot structure, record
/// semantics, the order-id chain, the padding rule, the stored range, the
/// stored digest, and — because the grid is here — that every limit already on
/// the page is an exact tick.  It never materializes a slot array.
#[inline(never)]
fn verify_page_on_grid(page: &[u8], grid: &PriceGridAccount) -> Outcome<stream::OrderPageHeader> {
    Ok(stream::verify_page_on_grid(page, grid)?)
}

/// Re-verify a page this module just wrote, streaming, and return its header.
#[inline(never)]
fn verify_page(page: &[u8]) -> Outcome<stream::OrderPageHeader> {
    Ok(stream::verify_page(page)?)
}

/// Decode the slot at `index` of an already-verified page.
#[inline(never)]
fn read_slot(page: &[u8], index: usize) -> Outcome<OrderSlot> {
    let mut cursor = stream::OrderSlotCursor::new(page)?;
    let mut at = 0usize;
    loop {
        let slot = match cursor.next_slot() {
            Some(step) => step?,
            /* Unreachable: `index` is below `MAX_ORDERS_PER_PAGE` and the page
             * has already been verified.  Stated, not assumed. */
            None => return Err(CodecError::Truncated.into()),
        };
        if at == index {
            return Ok(slot);
        }
        at += 1;
    }
}

/* ------------------------------------------------------------------------ */
/* Frame-bounded writers                                                     */
/* ------------------------------------------------------------------------ */

/// Overwrite one slot of a page with a single-Egg record.
///
/// The whole [`ORDER_SLOT_BYTES`] slot is zeroed first, so the canonical zero
/// padding beyond the record body is written rather than inherited.
fn write_single_slot(page: &mut [u8], index: usize, order: &OrderRecord) -> Outcome<()> {
    require(
        page.len() == account_len::ORDER_PAGE && index < MAX_ORDERS_PER_PAGE,
        ClutchError::WrongDataLength,
    )?;
    let start = stream::ORDER_PAGE_HEADER_BYTES + (index * ORDER_SLOT_BYTES);
    let slot = &mut page[start..start + ORDER_SLOT_BYTES];
    slot.fill(0);
    slot[0] = ORDER_KIND_SINGLE;
    slot[SLOT_OFF_OWNER..SLOT_OFF_OWNER + 32].copy_from_slice(&order.owner.0);
    slot[SLOT_OFF_ORDER_ID..SLOT_OFF_ORDER_ID + 32].copy_from_slice(&order.order_id.0);
    slot[SLOT_OFF_OUTCOME] = order.outcome;
    slot[SLOT_OFF_SIDE] = order.side;
    slot[SLOT_OFF_QUANTITY..SLOT_OFF_QUANTITY + 8].copy_from_slice(&order.quantity.to_le_bytes());
    slot[SLOT_OFF_LIMIT..SLOT_OFF_LIMIT + 8].copy_from_slice(&order.limit.to_le_bytes());
    slot[SLOT_OFF_MINIMUM_FILL..SLOT_OFF_MINIMUM_FILL + 8]
        .copy_from_slice(&order.minimum_fill.to_le_bytes());
    slot[SLOT_OFF_FLAGS] = order.flags;
    slot[SLOT_OFF_GENERATION..SLOT_OFF_GENERATION + 8]
        .copy_from_slice(&order.generation.to_le_bytes());
    Ok(())
}

/// Write the three header fields a placement moves.
///
/// `page_digest` is deliberately **not** written here: it is a fold over these
/// very bytes, so it can only be written after them.
fn write_stored_range(
    page: &mut [u8],
    order_count: u8,
    first: Hash32,
    last: Hash32,
) -> Outcome<()> {
    require(
        page.len() == account_len::ORDER_PAGE,
        ClutchError::WrongDataLength,
    )?;
    page[OFF_ORDER_COUNT] = order_count;
    page[OFF_FIRST_ORDER_ID..OFF_FIRST_ORDER_ID + 32].copy_from_slice(&first.0);
    page[OFF_LAST_ORDER_ID..OFF_LAST_ORDER_ID + 32].copy_from_slice(&last.0);
    Ok(())
}

/// Fold the mutated page through the layout crate's own digest and store it.
///
/// The value written is [`stream::streamed_page_digest`] over the post-write
/// bytes, so the digest is never this module's arithmetic.
fn seal_page_digest(page: &mut [u8]) -> Outcome<()> {
    require(
        page.len() == account_len::ORDER_PAGE,
        ClutchError::WrongDataLength,
    )?;
    let digest = stream::streamed_page_digest(page)?;
    page[OFF_PAGE_DIGEST..OFF_PAGE_DIGEST + 32].copy_from_slice(&digest.0);
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Transition                                                                */
/* ------------------------------------------------------------------------ */

/// Everything a placement needs that is not the page it writes.
#[derive(Clone, Copy, Debug)]
struct Placement<'a> {
    /// Epoch account bytes.
    epoch: &'a [u8],
    /// Frozen price-grid account bytes.
    grid: &'a [u8],
    /// The authenticated actor's key, as a 32-byte identity.
    actor: Hash32,
    /// The request envelope's replay sequence.
    sequence: u64,
    /// The intent's declared market.
    intent_market: Hash32,
    /// The intent's declared epoch.
    intent_epoch: Hash32,
    /// The record to append.
    order: OrderRecord,
}

/// Append one single-Egg record to an open page, or refuse.
///
/// Every check runs before any byte is written; the only checks after the
/// write are the post-write proofs described in the module docs, and an
/// instruction that refuses discards its writes.
fn apply_place_order(page: &mut [u8], placement: &Placement<'_>) -> Outcome<()> {
    let epoch = accounts::read_epoch(placement.epoch)?;
    let mut grid = ZERO_GRID;
    load_grid(placement.grid, &mut grid)?;

    // 3. The page, in full, on the frozen grid.
    let header = verify_page_on_grid(page, &grid)?;

    // 4. Placements are admitted only while the book is open.
    require(epoch.phase == EPOCH_PHASE_OPEN, ClutchError::NotActive)?;
    require(header.frozen == 0, ClutchError::NotActive)?;

    // 5. The page is this epoch's, and the grid is the one the epoch names.
    require(
        header.market == epoch.market && header.epoch == epoch.epoch,
        ClutchError::MismatchedState,
    )?;
    require(
        grid.grid == epoch.price_grid && grid.price_scale == epoch.price_scale,
        ClutchError::MismatchedState,
    )?;

    // 6. The intent names the same epoch the accounts do.
    require(
        placement.intent_market == epoch.market && placement.intent_epoch == epoch.epoch,
        ClutchError::MismatchedState,
    )?;

    /* 7. Replay.  The page's own populated-record count is the counter, so a
     * replayed transaction is refused by state rather than by a caller's
     * assertion, exactly as `FeedAdvance` uses the feed's page counter. */
    require(
        placement.sequence == header.order_count as u64,
        ClutchError::Replay,
    )?;

    // 8. The actor is the owner the record claims.
    require(
        placement.actor == placement.order.owner,
        ClutchError::UnauthorizedActor,
    )?;

    // 9. The record, and the width only the epoch knows.
    placement.order.validate()?;
    /* The epoch owns this market's actual outcome width, which no page can
     * bound below `MAX_OUTCOMES`; this is the same refusal
     * `stream::epoch_binds_page_set` gives for a record the epoch's width does
     * not admit, applied one record early. */
    if placement.order.outcome >= epoch.outcome_count {
        return Err(CodecError::MismatchedBinding.into());
    }

    // 10. The limit is an exact member of the frozen tick vector.
    grid.tick_of(placement.order.limit)?;

    /* 11. There is a free slot, and the id extends the chain.  A page whose
     * `order_count` is already `MAX_ORDERS_PER_PAGE` would become a page whose
     * count is out of range, which is what `validate_head` calls
     * `InvalidCount`; the refusal is the codec's, not a second vocabulary. */
    let index = header.order_count as usize;
    if index >= MAX_ORDERS_PER_PAGE {
        return Err(CodecError::InvalidCount.into());
    }
    /* An empty page extends the *previous page's* last id; a populated one
     * extends its own.  Both are the rule `validate_link` and the slot cursor
     * already enforce over the stored bytes, applied one record early. */
    let predecessor = if index == 0 {
        header.prev_page_last_order_id
    } else {
        header.last_order_id
    };
    if placement.order.order_id.0 <= predecessor.0 {
        return Err(CodecError::NonCanonicalIdentity.into());
    }

    // 12. Write.
    let first = if index == 0 {
        placement.order.order_id
    } else {
        header.first_order_id
    };
    let last = placement.order.order_id;
    let count = index as u8 + 1;
    write_single_slot(page, index, &placement.order)?;
    write_stored_range(page, count, first, last)?;
    seal_page_digest(page)?;

    /* The post-write proof.  This module wrote bytes `clutch-solana-layout`
     * owns, so the frozen codec is asked to accept them and the intended
     * post-state is compared field for field, on every execution. */
    let post = verify_page(page)?;
    /* `verify_page` returning at all is already the proof that the stored
     * digest equals the layout crate's own fold over the post-write bytes, so
     * the digest is carried across here rather than folded a fourth time;
     * every other field is compared against the intended post-state. */
    let expected = stream::OrderPageHeader {
        page_digest: post.page_digest,
        first_order_id: first,
        last_order_id: last,
        order_count: count,
        ..header
    };
    require(post == expected, ClutchError::MismatchedState)?;
    require(
        read_slot(page, index)? == OrderSlot::Single(placement.order),
        ClutchError::MismatchedState,
    )?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Account plane                                                             */
/* ------------------------------------------------------------------------ */

/// Validate hostile accounts and apply exactly one batch-plane transition.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::PlaceOrder {
            market,
            epoch,
            order,
        }) => place_order(program_id, accounts, request.sequence, market, epoch, order),
        /* Refused for a reason, not for a schedule: see the module docs.
         * Neither reads an account, because reading one would suggest that the
         * account list it read is the right one, and neither list can be
         * chosen before the representation and the frame budget are. */
        Action::Layout(Intent::CancelOrder { .. }) | Action::Layout(Intent::SettlePage { .. }) => {
            Err(ClutchError::NotYetImplemented.into())
        }
        /* Every other action belongs to another family module; the router never
         * sends one here, and this arm exists so that adding one to the router
         * is a compile error rather than a silent success. */
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// The `PlaceOrder` account plane.
fn place_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    order: OrderRecord,
) -> Outcome<()> {
    require_count(accounts, PLACE_ORDER_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &PLACE_ORDER_STATE_ROLES)?;

    /* Addresses, recomputed from the frozen seed schema out of each account's
     * own decoded bytes.  The epoch's identity is itself derived from
     * `(market, epoch_index)` by its codec, and the grid's identity is a digest
     * over a body that includes its realm, so neither can lie about the seeds
     * it is addressed by without failing to decode. */
    let epoch = accounts::read_epoch(&accounts[IX_EPOCH].data.borrow())?;
    let grid = accounts::read_price_grid(&accounts[IX_GRID].data.borrow())?;
    let page_header = {
        let data = accounts[IX_PAGE].data.borrow();
        stream::OrderPageHeader::decode(&data)?
    };
    expect_pda(
        accounts[IX_EPOCH].key,
        seeds::epoch_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;
    expect_pda(
        accounts[IX_GRID].key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;
    expect_pda(
        accounts[IX_PAGE].key,
        seeds::page_pda(
            program_id,
            &page_header.epoch.bytes(),
            page_header.page_index,
        ),
        Some(page_header.stored_bump),
    )?;

    let epoch_data = accounts[IX_EPOCH].data.borrow();
    let grid_data = accounts[IX_GRID].data.borrow();
    let placement = Placement {
        epoch: &epoch_data,
        grid: &grid_data,
        actor: Hash32::from_bytes(accounts[IX_ACTOR].key.to_bytes()),
        sequence,
        intent_market,
        intent_epoch,
        order,
    };
    let mut page_data = borrow_mut!(accounts[IX_PAGE])?;
    apply_place_order(&mut page_data, &placement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        canonical_epoch_id, EpochAccount, OrderPageAccount, EPOCH_PHASE_CLEARED,
        EPOCH_PHASE_FROZEN, RELATION_VERSION,
    };

    /// Length of the layout crate's private page-digest domain tag,
    /// `b"dragons-clutch/order-page/v2"`.  It is not exported, so this is the
    /// one number below that a rename would not turn red.
    const ORDER_PAGE_DOMAIN_BYTES: usize = 28;

    /* These tests drive the transition directly, on byte slices, exactly as
     * `observe_resolve`'s do and for the same reason: off-chain program-address
     * derivation is not compiled into this crate (see `crate::seeds`), so no
     * host test can reach `process`'s account plane.  What is covered here is
     * every check from the page verdict onward, plus the byte-for-byte
     * agreement of the write-back with the layout crate's own encoder.  The
     * account plane is covered only by the SVM differential, which does not
     * exercise this family yet. */

    fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    /// The frozen grid every fixture clears on: three exact ticks.
    fn grid_account() -> PriceGridAccount {
        let mut ticks = [0; MAX_GRID_TICKS];
        ticks[0] = 2_500;
        ticks[1] = 5_000;
        ticks[2] = 7_500;
        let mut grid = PriceGridAccount {
            grid: Hash32::ZERO,
            realm: h(0x11),
            price_scale: 10_000,
            tick_count: 3,
            ticks,
            stored_bump: 7,
            flags: 0,
        };
        grid.grid = grid.recomputed_grid_id().expect("grid identity");
        grid
    }

    /// An open epoch on that grid, two outcomes wide.
    fn epoch_account(grid: &PriceGridAccount) -> EpochAccount {
        let market = h(1);
        EpochAccount {
            epoch: canonical_epoch_id(market, 4),
            market,
            book: h(2),
            terms: h(3),
            price_grid: grid.grid,
            policy: h(4),
            order_set: Hash32::ZERO,
            first_order_id: Hash32::ZERO,
            last_order_id: Hash32::ZERO,
            epoch_index: 4,
            relation_version: RELATION_VERSION,
            price_scale: grid.price_scale,
            remainder_seed: 0,
            owner_count: 4,
            page_count: 0,
            order_count: 0,
            outcome_count: 2,
            phase: EPOCH_PHASE_OPEN,
            stored_bump: 6,
            flags: 0,
        }
    }

    /// One page of that epoch, with `records` already placed.
    fn page_account(
        epoch: &EpochAccount,
        page_index: u16,
        page_count: u16,
        prev: Hash32,
        records: &[OrderRecord],
    ) -> OrderPageAccount {
        let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
        let mut i = 0;
        while i < records.len() {
            orders[i] = OrderSlot::Single(records[i]);
            i += 1;
        }
        let mut page = OrderPageAccount {
            market: epoch.market,
            epoch: epoch.epoch,
            order_set: Hash32::ZERO,
            page_digest: Hash32::ZERO,
            first_order_id: records.first().map_or(Hash32::ZERO, |o| o.order_id),
            last_order_id: records.last().map_or(Hash32::ZERO, |o| o.order_id),
            prev_page_last_order_id: prev,
            page_index,
            page_count,
            set_order_count: 0,
            order_count: records.len() as u8,
            frozen: 0,
            stored_bump: 5,
            orders,
        };
        page.page_digest = page.recomputed_page_digest().expect("page digest");
        page
    }

    fn encode_page(page: &OrderPageAccount) -> [u8; account_len::ORDER_PAGE] {
        let mut bytes = [0; account_len::ORDER_PAGE];
        page.encode(&mut bytes).expect("page encodes");
        bytes
    }

    fn order(owner: u8, id: u8, limit: u64) -> OrderRecord {
        OrderRecord {
            owner: h(owner),
            order_id: h(id),
            outcome: 0,
            side: 0,
            quantity: 10,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
        }
    }

    /// Every account byte string a placement reads but never writes.
    struct Domain {
        epoch: EpochAccount,
        epoch_bytes: [u8; account_len::EPOCH],
        grid_bytes: [u8; account_len::PRICE_GRID],
    }

    fn domain() -> Domain {
        domain_with(epoch_account(&grid_account()), grid_account())
    }

    fn domain_with(epoch: EpochAccount, grid: PriceGridAccount) -> Domain {
        let mut epoch_bytes = [0; account_len::EPOCH];
        epoch.encode(&mut epoch_bytes).expect("epoch encodes");
        let mut grid_bytes = [0; account_len::PRICE_GRID];
        grid.encode(&mut grid_bytes).expect("grid encodes");
        Domain {
            epoch,
            epoch_bytes,
            grid_bytes,
        }
    }

    impl Domain {
        /// A well-formed placement of `order` at `sequence`, signed by its owner.
        fn placement(&self, sequence: u64, order: OrderRecord) -> Placement<'_> {
            Placement {
                epoch: &self.epoch_bytes,
                grid: &self.grid_bytes,
                actor: order.owner,
                sequence,
                intent_market: self.epoch.market,
                intent_epoch: self.epoch.epoch,
                order,
            }
        }

        fn place(&self, page: &mut [u8], sequence: u64, order: OrderRecord) -> Outcome<()> {
            apply_place_order(page, &self.placement(sequence, order))
        }
    }

    fn codec(error: CodecError) -> Refusal {
        Refusal::Codec(error)
    }

    fn adapter(error: ClutchError) -> Refusal {
        Refusal::Adapter(error)
    }

    #[test]
    fn place_order_writes_exactly_what_the_layout_encoder_would() {
        let d = domain();
        let first = order(0x20, 3, 5_000);
        let second = order(0x21, 9, 2_500);

        let mut page = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[]));
        assert_eq!(d.place(&mut page, 0, first), Ok(()));
        assert_eq!(
            page,
            encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[first])),
            "the post-state must be byte-identical to the layout crate's own encoding"
        );

        assert_eq!(d.place(&mut page, 1, second), Ok(()));
        assert_eq!(
            page,
            encode_page(&page_account(
                &d.epoch,
                0,
                1,
                Hash32::ZERO,
                &[first, second]
            ))
        );

        // And the buffered decoder — the golden reference — reads it back.
        let decoded = OrderPageAccount::decode(&page).expect("post-state decodes");
        assert_eq!(decoded.order_count, 2);
        assert_eq!(decoded.first_order_id, first.order_id);
        assert_eq!(decoded.last_order_id, second.order_id);
        assert_eq!(decoded.orders[0], OrderSlot::Single(first));
        assert_eq!(decoded.orders[1], OrderSlot::Single(second));
        assert_eq!(decoded.orders[2], OrderSlot::Empty);
    }

    #[test]
    fn place_order_fills_a_page_and_then_refuses_a_seventeenth_record() {
        let d = domain();
        let mut page = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[]));
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            let id = (i + 1) as u8;
            assert_eq!(d.place(&mut page, i as u64, order(0x20, id, 5_000)), Ok(()));
            i += 1;
        }
        let full = page;
        // A seventeenth record would be a page whose count is out of range,
        // which is exactly what the page header calls `InvalidCount`.
        assert_eq!(
            d.place(
                &mut page,
                MAX_ORDERS_PER_PAGE as u64,
                order(0x20, 0xf0, 5_000)
            ),
            Err(codec(CodecError::InvalidCount))
        );
        assert_eq!(page, full, "a refusal writes nothing");
    }

    #[test]
    fn place_order_refuses_every_page_the_streaming_decoder_refuses() {
        let d = domain();
        let existing = order(0x20, 3, 5_000);
        let clean = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[existing]));
        let next = order(0x20, 9, 2_500);

        // Each case is a byte-level fixture the frozen codec's own adversarial
        // tests carry, and the refusal must be the codec's verdict verbatim.
        let mut cases: [(&str, [u8; account_len::ORDER_PAGE]); 11] = [("clean", clean); 11];
        cases[0].0 = "wrong tag";
        cases[0].1[0] = clean[0].wrapping_add(1);
        cases[1].0 = "wrong version";
        cases[1].1[1] = clean[1].wrapping_add(1);
        cases[2].0 = "unknown slot kind";
        cases[2].1[stream::ORDER_PAGE_HEADER_BYTES] = 3;
        cases[3].0 = "unknown kind in a padding slot";
        cases[3].1[stream::ORDER_PAGE_HEADER_BYTES + ORDER_SLOT_BYTES] = u8::MAX;
        cases[4].0 = "nonzero single-egg tail";
        cases[4].1[stream::ORDER_PAGE_HEADER_BYTES + 1 + ORDER_RECORD_BYTES] = 1;
        cases[5].0 = "nonzero slot end";
        cases[5].1[stream::ORDER_PAGE_HEADER_BYTES + ORDER_SLOT_BYTES - 1] = 1;
        cases[6].0 = "dirty padding slot";
        cases[6].1[stream::ORDER_PAGE_HEADER_BYTES + ORDER_SLOT_BYTES + 5] = 1;
        cases[7].0 = "all-zero record in a padding slot";
        cases[7].1[stream::ORDER_PAGE_HEADER_BYTES + ORDER_SLOT_BYTES] = ORDER_KIND_SINGLE;
        cases[8].0 = "nonzero final byte";
        cases[8].1[account_len::ORDER_PAGE - 1] = 1;
        cases[9].0 = "stale page digest";
        cases[9].1[OFF_PAGE_DIGEST] ^= 1;
        cases[10].0 = "stale stored range";
        cases[10].1[OFF_LAST_ORDER_ID] ^= 1;

        for (name, bytes) in cases.iter() {
            let expected = stream::verify_page(bytes).expect_err(name);
            let mut page = *bytes;
            assert_eq!(
                d.place(&mut page, 1, next),
                Err(codec(expected)),
                "{name}: the placement must report the codec's own verdict"
            );
            assert_eq!(&page, bytes, "{name}: a refusal writes nothing");
        }

        // Framing faults that change the buffer's length are the same story.
        let mut short = clean[..account_len::ORDER_PAGE - 1].to_vec();
        assert_eq!(
            d.place(&mut short, 1, next),
            Err(codec(CodecError::Truncated))
        );
        let mut long = clean.to_vec();
        long.push(0);
        assert_eq!(
            d.place(&mut long, 1, next),
            Err(codec(CodecError::TrailingBytes))
        );
        let mut zeros = [0u8; account_len::ORDER_PAGE];
        assert_eq!(
            d.place(&mut zeros, 0, next),
            Err(codec(
                stream::verify_page(&[0; account_len::ORDER_PAGE]).expect_err("all zero")
            ))
        );
    }

    #[test]
    fn place_order_refuses_an_off_grid_limit_and_a_grid_the_epoch_does_not_name() {
        let d = domain();
        let mut page = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[]));

        // A limit between two ticks has no tick, so it has no relation price.
        assert_eq!(
            d.place(&mut page, 0, order(0x20, 3, 5_001)),
            Err(codec(CodecError::InvalidTick))
        );
        // Neither has one above the scale.
        assert_eq!(
            d.place(&mut page, 0, order(0x20, 3, 20_000)),
            Err(codec(CodecError::InvalidTick))
        );

        // A different, internally valid grid is still not this epoch's grid.
        let mut other = grid_account();
        other.ticks[2] = 8_000;
        other.grid = other.recomputed_grid_id().expect("grid identity");
        let mut other_bytes = [0; account_len::PRICE_GRID];
        other.encode(&mut other_bytes).expect("grid encodes");
        let mut placement = d.placement(0, order(0x20, 3, 5_000));
        placement.grid = &other_bytes;
        assert_eq!(
            apply_place_order(&mut page, &placement),
            Err(adapter(ClutchError::MismatchedState))
        );

        // The page's existing records are held to the grid too: a page carrying
        // an off-grid limit cannot be extended at all.
        let mut stale = grid_account();
        stale.ticks[1] = 5_500;
        stale.grid = stale.recomputed_grid_id().expect("grid identity");
        let stale_epoch = EpochAccount {
            price_grid: stale.grid,
            ..d.epoch
        };
        let stale_domain = domain_with(stale_epoch, stale);
        let mut occupied = encode_page(&page_account(
            &stale_domain.epoch,
            0,
            1,
            Hash32::ZERO,
            &[order(0x20, 3, 5_000)],
        ));
        assert_eq!(
            stale_domain.place(&mut occupied, 1, order(0x20, 9, 2_500)),
            Err(codec(CodecError::InvalidTick))
        );
    }

    #[test]
    fn place_order_refuses_a_closed_epoch_and_a_frozen_page() {
        let grid = grid_account();
        for phase in [EPOCH_PHASE_FROZEN, EPOCH_PHASE_CLEARED] {
            let base = epoch_account(&grid);
            /* A non-open epoch carries the frozen-set commitments, so the
             * fixture must be a whole frozen epoch rather than a phase byte. */
            let closed = EpochAccount {
                phase,
                order_set: h(0x40),
                first_order_id: h(3),
                last_order_id: h(9),
                page_count: 1,
                order_count: 2,
                ..base
            };
            let d = domain_with(closed, grid);
            let mut page = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[]));
            assert_eq!(
                d.place(&mut page, 0, order(0x20, 3, 5_000)),
                Err(adapter(ClutchError::NotActive))
            );
        }

        // The page's own freeze flag is checked as well as the epoch's phase:
        // an open epoch and a frozen page still refuse.
        let d = domain();
        let mut frozen = page_account(&d.epoch, 0, 1, Hash32::ZERO, &[order(0x20, 3, 5_000)]);
        frozen.frozen = 1;
        frozen.set_order_count = 1;
        frozen.order_set = h(0x40);
        frozen.page_digest = frozen.recomputed_page_digest().expect("page digest");
        let mut page = encode_page(&frozen);
        assert_eq!(
            d.place(&mut page, 1, order(0x20, 9, 2_500)),
            Err(adapter(ClutchError::NotActive))
        );
    }

    #[test]
    fn place_order_refuses_an_order_id_that_does_not_extend_the_chain() {
        let d = domain();
        let existing = order(0x20, 9, 5_000);
        let occupied = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[existing]));

        for id in [9u8, 3, 0] {
            let mut page = occupied;
            let expected = if id == 0 {
                // A zero identity is refused by the record codec first.
                codec(CodecError::ZeroIdentity)
            } else {
                codec(CodecError::NonCanonicalIdentity)
            };
            assert_eq!(d.place(&mut page, 1, order(0x20, id, 2_500)), Err(expected));
            assert_eq!(page, occupied);
        }

        // On an empty page the chain is the *previous* page's last id, which is
        // the rule `validate_link` states over stored bytes.
        let mut tail = encode_page(&page_account(&d.epoch, 1, 2, h(9), &[]));
        assert_eq!(
            d.place(&mut tail, 0, order(0x20, 5, 5_000)),
            Err(codec(CodecError::NonCanonicalIdentity))
        );
        assert_eq!(d.place(&mut tail, 0, order(0x20, 20, 5_000)), Ok(()));
        assert_eq!(
            OrderPageAccount::decode(&tail)
                .expect("post-state decodes")
                .first_order_id,
            h(20)
        );
    }

    #[test]
    fn place_order_refuses_an_unauthenticated_owner_and_a_replayed_sequence() {
        let d = domain();
        let clean = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[]));

        // The signer is the owner, or there is no placement.
        let mut page = clean;
        let mut placement = d.placement(0, order(0x20, 3, 5_000));
        placement.actor = h(0x21);
        assert_eq!(
            apply_place_order(&mut page, &placement),
            Err(adapter(ClutchError::UnauthorizedActor))
        );
        assert_eq!(page, clean);

        // The page's own record count is the replay counter, so a sequence that
        // is not the next free slot is refused in both directions.
        for sequence in [1u64, 7, u64::MAX] {
            let mut page = clean;
            assert_eq!(
                d.place(&mut page, sequence, order(0x20, 3, 5_000)),
                Err(adapter(ClutchError::Replay))
            );
        }
        let mut page = clean;
        assert_eq!(d.place(&mut page, 0, order(0x20, 3, 5_000)), Ok(()));
        // Replaying the accepted request now names a stale slot.
        assert_eq!(
            d.place(&mut page, 0, order(0x20, 5, 5_000)),
            Err(adapter(ClutchError::Replay))
        );
    }

    #[test]
    fn place_order_refuses_a_page_or_an_intent_that_names_another_epoch() {
        let d = domain();
        let clean = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[]));

        // A page of a different epoch, internally valid, is still not this one.
        let other_epoch = EpochAccount {
            epoch: canonical_epoch_id(d.epoch.market, 5),
            epoch_index: 5,
            ..d.epoch
        };
        let mut foreign = encode_page(&page_account(&other_epoch, 0, 1, Hash32::ZERO, &[]));
        assert_eq!(
            d.place(&mut foreign, 0, order(0x20, 3, 5_000)),
            Err(adapter(ClutchError::MismatchedState))
        );

        // And an intent that names another market or another epoch.
        for (market, epoch) in [
            (h(0x7e), d.epoch.epoch),
            (d.epoch.market, canonical_epoch_id(d.epoch.market, 5)),
        ] {
            let mut page = clean;
            let mut placement = d.placement(0, order(0x20, 3, 5_000));
            placement.intent_market = market;
            placement.intent_epoch = epoch;
            assert_eq!(
                apply_place_order(&mut page, &placement),
                Err(adapter(ClutchError::MismatchedState))
            );
            assert_eq!(page, clean);
        }
    }

    #[test]
    fn place_order_mirrors_the_record_codec_and_the_epochs_outcome_width() {
        let d = domain();
        let clean = encode_page(&page_account(&d.epoch, 0, 1, Hash32::ZERO, &[]));
        let base = order(0x20, 3, 5_000);

        let cases: [(&str, OrderRecord, CodecError); 7] = [
            (
                "zero owner",
                OrderRecord {
                    owner: Hash32::ZERO,
                    ..base
                },
                CodecError::ZeroIdentity,
            ),
            (
                "zero order id",
                OrderRecord {
                    order_id: Hash32::ZERO,
                    ..base
                },
                CodecError::ZeroIdentity,
            ),
            (
                "unknown side",
                OrderRecord { side: 2, ..base },
                CodecError::InvalidEnum,
            ),
            (
                "zero quantity",
                OrderRecord {
                    quantity: 0,
                    minimum_fill: 0,
                    ..base
                },
                CodecError::InvalidEnum,
            ),
            (
                "minimum fill above quantity",
                OrderRecord {
                    minimum_fill: 11,
                    ..base
                },
                CodecError::InvalidEnum,
            ),
            (
                "reserved flag bit",
                OrderRecord { flags: 2, ..base },
                CodecError::InvalidEnum,
            ),
            (
                "all-or-none with a partial minimum",
                OrderRecord {
                    flags: 1,
                    minimum_fill: 4,
                    ..base
                },
                CodecError::InvalidEnum,
            ),
        ];
        for (name, record, expected) in cases {
            // The refusal is the record codec's own, not a second vocabulary.
            assert_eq!(record.validate(), Err(expected), "{name}");
            let mut page = clean;
            let mut placement = d.placement(0, record);
            placement.actor = record.owner;
            assert_eq!(
                apply_place_order(&mut page, &placement),
                Err(codec(expected)),
                "{name}"
            );
            assert_eq!(page, clean, "{name}: a refusal writes nothing");
        }

        /* An outcome inside `MAX_OUTCOMES` but outside this market's width is a
         * record no page can refuse and the epoch must: it is the same bound
         * `stream::epoch_binds_page_set` applies to a frozen set. */
        let wide = OrderRecord { outcome: 2, ..base };
        assert_eq!(wide.validate(), Ok(()));
        let mut page = clean;
        assert_eq!(
            d.place(&mut page, 0, wide),
            Err(codec(CodecError::MismatchedBinding))
        );
        assert_eq!(page, clean);
    }

    #[test]
    fn the_page_offsets_this_module_writes_match_the_frozen_codec() {
        let d = domain();
        let record = OrderRecord {
            owner: h(0x20),
            order_id: h(0x33),
            outcome: 1,
            side: 1,
            quantity: 0x0102_0304_0506_0708,
            limit: 5_000,
            minimum_fill: 0x0102_0304_0506_0708,
            flags: 1,
            generation: 0x1122_3344_5566_7788,
        };
        let page = page_account(&d.epoch, 1, 2, h(0x30), &[record]);
        let bytes = encode_page(&page);

        // Header offsets, read back out of the layout crate's own encoding.
        let hash_at = |off: usize| Hash32::from_bytes(bytes[off..off + 32].try_into().unwrap());
        let u16_at = |off: usize| u16::from_le_bytes(bytes[off..off + 2].try_into().unwrap());
        assert_eq!(hash_at(OFF_MARKET), page.market);
        assert_eq!(hash_at(OFF_EPOCH), page.epoch);
        assert_eq!(hash_at(OFF_ORDER_SET), page.order_set);
        assert_eq!(hash_at(OFF_PAGE_DIGEST), page.page_digest);
        assert_eq!(hash_at(OFF_FIRST_ORDER_ID), page.first_order_id);
        assert_eq!(hash_at(OFF_LAST_ORDER_ID), page.last_order_id);
        assert_eq!(
            hash_at(OFF_PREV_PAGE_LAST_ORDER_ID),
            page.prev_page_last_order_id
        );
        assert_eq!(u16_at(OFF_PAGE_INDEX), page.page_index);
        assert_eq!(u16_at(OFF_PAGE_COUNT), page.page_count);
        assert_eq!(u16_at(OFF_SET_ORDER_COUNT), page.set_order_count);
        assert_eq!(bytes[OFF_ORDER_COUNT], page.order_count);
        assert_eq!(bytes[OFF_FROZEN], page.frozen);
        assert_eq!(bytes[OFF_STORED_BUMP], page.stored_bump);

        // Slot offsets, likewise.
        let slot = &bytes
            [stream::ORDER_PAGE_HEADER_BYTES..stream::ORDER_PAGE_HEADER_BYTES + ORDER_SLOT_BYTES];
        let u64_at = |off: usize| u64::from_le_bytes(slot[off..off + 8].try_into().unwrap());
        assert_eq!(slot[0], ORDER_KIND_SINGLE);
        assert_eq!(&slot[SLOT_OFF_OWNER..SLOT_OFF_OWNER + 32], &record.owner.0);
        assert_eq!(
            &slot[SLOT_OFF_ORDER_ID..SLOT_OFF_ORDER_ID + 32],
            &record.order_id.0
        );
        assert_eq!(slot[SLOT_OFF_OUTCOME], record.outcome);
        assert_eq!(slot[SLOT_OFF_SIDE], record.side);
        assert_eq!(u64_at(SLOT_OFF_QUANTITY), record.quantity);
        assert_eq!(u64_at(SLOT_OFF_LIMIT), record.limit);
        assert_eq!(u64_at(SLOT_OFF_MINIMUM_FILL), record.minimum_fill);
        assert_eq!(slot[SLOT_OFF_FLAGS], record.flags);
        assert_eq!(u64_at(SLOT_OFF_GENERATION), record.generation);
        // Everything past the record body is canonical zero padding.
        assert!(slot[1 + ORDER_RECORD_BYTES..].iter().all(|b| *b == 0));

        // And this module's own writer reproduces that slot exactly.
        let mut written = encode_page(&page_account(&d.epoch, 1, 2, h(0x30), &[]));
        write_single_slot(&mut written, 0, &record).expect("slot writes");
        assert_eq!(
            &written[stream::ORDER_PAGE_HEADER_BYTES
                ..stream::ORDER_PAGE_HEADER_BYTES + ORDER_SLOT_BYTES],
            slot
        );
    }

    #[test]
    fn the_place_order_wire_cannot_carry_a_portfolio_record() {
        /* `Intent::PlaceOrder` carries an `OrderRecord`, and its encoded length
         * says so: a portfolio record's body alone is wider than the whole
         * intent.  There is no portfolio placement to implement here — the
         * frozen wire has no way to express one, which is a layout finding and
         * not an unimplemented branch. */
        let intent = Intent::PlaceOrder {
            market: h(1),
            epoch: canonical_epoch_id(h(1), 4),
            order: order(0x20, 3, 5_000),
        };
        assert_eq!(intent.encoded_len(), 2 + 32 + 32 + ORDER_RECORD_BYTES);
        /* A slot is wider than a single-Egg record precisely because the
         * portfolio family exists in the page and not on the wire. */
        const _: () = assert!(ORDER_RECORD_BYTES < ORDER_SLOT_BYTES);

        let mut bytes = [0; 2 + 32 + 32 + ORDER_RECORD_BYTES];
        assert_eq!(intent.encode(&mut bytes), Ok(bytes.len()));
        assert_eq!(Intent::decode(&bytes), Ok(intent));
    }

    #[test]
    fn cancel_and_settle_refuse_before_any_account_is_touched() {
        let program_id = Pubkey::new_from_array([9; 32]);
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        for action in [
            Action::Layout(Intent::CancelOrder {
                market,
                epoch,
                owner: h(0x20),
                order_id: h(3),
            }),
            Action::Layout(Intent::SettlePage {
                market,
                epoch,
                page_index: 0,
            }),
        ] {
            let request = Request {
                sequence: 0,
                action,
            };
            // An empty account list is enough: neither reads an account, so
            // neither can reach the account plane's own refusals.
            assert_eq!(
                process(&program_id, &[], &request),
                Err(adapter(ClutchError::NotYetImplemented))
            );
        }

        // `PlaceOrder` does reach it, and says so.
        let request = Request {
            sequence: 0,
            action: Action::Layout(Intent::PlaceOrder {
                market,
                epoch,
                order: order(0x20, 3, 5_000),
            }),
        };
        assert_eq!(
            process(&program_id, &[], &request),
            Err(adapter(ClutchError::AccountCount))
        );
    }

    #[test]
    fn the_documented_page_fold_follows_from_the_frozen_widths() {
        /* The module docs state a compute *structure* rather than a
         * measurement, and this test is what keeps that arithmetic honest if
         * the page ever grows. */
        let preimage =
            ORDER_PAGE_DOMAIN_BYTES + 32 + 32 + 2 + 1 + (MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES);
        assert_eq!(preimage, 3_743);
        // SHA-256 pads with one `0x80` byte and an eight-byte length.
        let blocks = (preimage + 1 + 8).div_ceil(64);
        assert_eq!(blocks, 59, "compression blocks per page fold");
        assert_eq!(3 * blocks, 177, "compression blocks per accepted placement");
        assert_eq!(account_len::ORDER_PAGE, 3_883);
        assert_eq!(stream::ORDER_PAGE_HEADER_BYTES, 235);
    }
}
