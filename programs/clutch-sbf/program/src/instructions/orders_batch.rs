//! `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SubmitDirectPage`,
//! `Intent::SettlePage`, the staged checkpoint creation pair
//! `Intent::InitClearWork` / `Intent::GrowClearWork` ([`clear_work`]), the
//! general epoch lifecycle ([`general_epoch`]), the on-chain streaming walk
//! ([`clear_walk`]), the candidate submission and selection lifecycle
//! ([`selection`]), and the entitlement freeze ([`entitlement`]).
//!
//! This module owns the batch-auction plane's account lists.  `PlaceOrder` and
//! `CancelOrder` own the funded order lifecycle.  Since T2-8, `SettlePage`
//! consumes the entitled receipts the freeze (tags 58-59) creates from the
//! SELECTED candidate's verified allocation: single-Egg direct slices through
//! the generalized entitled seam, portfolio full pairs through the layout
//! crate's `{prepare,apply}_full_pair`, and one-ended virtual legs — a
//! `sigma` split or a `mu` merge — through the pooled complete-set primitive.
//! Partial fills, mixed pairs and inexact conversions all consume; the
//! `pub(super)` `settlement` submodule's standing ledger row list is empty,
//! and what still refuses is authority (a fee) rather than a missing join.
//!
//! | intent | this wave |
//! | --- | --- |
//! | `PlaceOrder` | **implemented**, both order families: the v3 wire carries an `OrderSlot` and signed fee cap; Position assets move into one reservation |
//! | `CancelOrder` | **implemented**: the v4 page retires an order in place and returns only its unused reservation (§ *Cancellation*) |
//! | `SubmitDirectPage` | **narrow constructor**: creates one deterministic `SUBMITTED` Candidate and exact feed from a funded two-order frozen page; it does not verify/select or create a receipt |
//! | `SettlePage` | **entitled consumption**: consumes one entitled direct slice (7 accounts, 8 with the pot), one entitled virtual leg in either churn direction (11 accounts, the pooled complete-set five among them), or one entitled portfolio full pair (variable list); every receipt consumes exactly once and every consumed reservation persists as its own archive |
//! | `FreezeEntitlement` / `EntitleSlice` | **entitlement freeze** ([`entitlement`]): pot from the verified summary, resumable per-slice receipts, reservations `ACTIVE → ENTITLED` |
//!
//! Nothing here computes a clearing price or selects a candidate. A placement
//! atomically writes one order record, encumbers exact free cash or moves exact
//! internal Eggs from its owner's Position, and creates the canonical per-order
//! reservation. A cancellation atomically tombstones that record and releases
//! only the same reservation's unused assets. Cash remains pooled in the Hoard;
//! the reservation owns its decomposition, and an order page alone owns no
//! funds.
//!
//! ## The write path belongs to `clutch-solana-layout`
//!
//! At the previous page revision this module owned thirteen page-header byte
//! offsets, nine record offsets, two `const` tripwires against the layout
//! crate's widths, four writer helpers, and a post-write re-verification that
//! decoded the whole page a second time and compared the resulting header
//! field by field against an intended post-state.  It owned them because the
//! layout crate published a streaming page *reader* and no streaming page
//! *writer*, so an on-chain placement could not ask the owning crate to write
//! its bytes; that was recorded here as debt, and the debt is paid.  v4
//! publishes [`clutch_solana_layout::stream::init_page`], `append_slot`,
//! `write_single_slot`, `write_tombstone`, `frozen_set_commitment`, and
//! `seal_page`, and both tripwires fired on the revision that landed them,
//! which is what they were for.
//!
//! **This module now owns no page offset at all.**  A transition is the layout
//! crate's own verifier, this module's checks, and the layout crate's own
//! writer:
//!
//! * the writer writes a header through the same `Writer` field sequence
//!   `OrderPageAccount::encode` uses and a slot through the same `encode_slot`,
//!   so the write side is not a second transcription of the layout that could
//!   drift from the first;
//! * it folds the page digest once, at the end, and that fold decodes every
//!   slot as it goes, so a page left non-canonical in any slot comes out with
//!   no digest rather than with a digest over junk;
//! * it returns the post-state as a header, so "what did the page become" is an
//!   answer rather than a second decode and a field-by-field comparison against
//!   a guess.
//!
//! The post-write re-verification is therefore gone, and with it one whole page
//! fold and two full slot-decode passes per placement (§ *Frames and compute*).
//! What is *not* gone is the byte-for-byte differential:
//! `place_order_writes_exactly_what_the_layout_encoder_would`,
//! `place_order_writes_a_portfolio_record_the_same_way`, and
//! `cancel_order_writes_exactly_what_the_layout_encoder_would` each compare the
//! post-state against a page built and encoded by `OrderPageAccount::encode`,
//! which is the golden reference.
//!
//! ## The account lists
//!
//! `PlaceOrder`: eight accounts, exact count, no remaining-account tail.
//!
//! | index | account | role |
//! | --- | --- | --- |
//! | 0 | actor | signer; its key **is** the order's 32-byte `owner` |
//! | 1 | epoch | read-only; owns the phase, the outcome width, the epoch index, and the grid identity |
//! | 2 | price grid | read-only; owns the admitted limit prices and the frozen scale |
//! | 3 | order page | writable; the page the record is appended to |
//! | 4 | Position | writable; owner-bound free cash and internal Eggs fund the order |
//! | 5 | reservation | writable, initially absent; canonical PDA created for this exact order |
//! | 6 | System program | read-only executable; creates the reservation PDA |
//! | 7 | Rent sysvar | read-only; fixes the reservation's exact rent floor |
//!
//! `CancelOrder`: five, and the grid is **absent**.  A retirement has no limit
//! and no lots, so nothing in the transition consults a tick vector or a price
//! scale, and a cancellation does not re-price the records already on the page
//! — [`clutch_solana_layout::stream::verify_page`] is precisely the reader that
//! says so, and it is the one this transition uses.
//!
//! | index | account | role |
//! | --- | --- | --- |
//! | 0 | actor | signer; its key must be the retired record's `owner` |
//! | 1 | epoch | read-only; owns the phase and this market's identity |
//! | 2 | order page | writable; the page whose slot the retired id names |
//! | 3 | Position | writable; receives only the reservation's unused assets |
//! | 4 | reservation | writable; exact active order reservation, released once |
//!
//! The market, realm, and profile accounts are deliberately **absent** from
//! both, for the same reason [`super::observe_resolve`] omits realm and
//! profile: the epoch already names this market, this grid, and this outcome
//! width, and [`clutch_solana_layout::EpochAccount::validate`] recomputes the
//! epoch identity from `(market, epoch_index)`, so three more accounts and
//! three more decodes would add transaction weight and no fact.  The uniformity
//! cost is an ABI decision for whoever freezes the account schema, not this
//! lane's.
//!
//! Every address is recomputed from [`crate::seeds`] out of the accounts' own
//! decoded bytes; no caller-supplied expected key is accepted anywhere. The
//! reservation PDA is derived from the domain-separated digest of `(market,
//! epoch, owner, position generation, order id)`, while its bytes additionally
//! bind page, order generation, Terms/basis, grid, policy, family, side, and
//! signed maximum fee.
//!
//! ## The order of the checks
//!
//! There is no offline oracle for this family — `clutch_solana_reference::apply`
//! refuses both intents with `Error::UnsupportedIntent` — so the order below is
//! this lane's, and the host tests at the bottom of this file pin it.  The
//! oracles that do exist are the frozen codec's own adversarial fixtures, and
//! every page refusal here is asserted to be *exactly* the verdict
//! [`clutch_solana_layout::stream::verify_page`] gives on the same bytes.
//!
//! Both transitions share the account plane and then diverge only where the two
//! writes differ.
//!
//! 1. account count, actor signature, role aliasing, program ownership, the
//!    executable bit, declared writability, exact data lengths, System program,
//!    Rent sysvar, and an actually creatable reservation target;
//! 2. every address, against [`crate::seeds`].  The page's addressing fields
//!    are read with [`clutch_solana_layout::stream::OrderPageHeader::decode`],
//!    which reads the fixed 236-byte header and folds no digest, because an
//!    address comparison needs `epoch`, `page_index`, and `stored_bump` and
//!    nothing else; the page's *verdict* is step 3.
//!
//! `PlaceOrder`, from there:
//!
//! 3. the page, in full, through the streaming decoder **on the frozen grid**
//!    ([`clutch_solana_layout::stream::verify_page_on_grid`]) — so a page whose
//!    stored digest, positional order ids, retirement count, slot padding,
//!    stored range, or existing limits are not canonical is refused before
//!    anything is decided about the new record;
//! 4. the epoch is `EPOCH_PHASE_OPEN` and the page is unfrozen;
//! 5. the page belongs to this epoch and this market, and the grid is the one
//!    the epoch names, at the scale the epoch names;
//! 6. the intent's own `market` and `epoch` name the same epoch;
//! 7. replay: the request sequence equals the page's current `order_count`;
//! 8. the actor is the record's owner;
//! 9. the record is valid, and it is inside the *epoch's* outcome width and
//!    not already behind the *epoch's* index — two bounds no page can apply,
//!    because a page bounds an outcome only by
//!    [`clutch_solana_layout::MAX_OUTCOMES`] and cannot recover an epoch index
//!    from the 32-byte epoch identity it stores.  Both are exactly the refusals
//!    [`clutch_solana_layout::stream::epoch_binds_page_set`] gives a frozen set,
//!    applied one record early;
//! 10. the grid half: a single-Egg `limit` is an exact member of the frozen
//!     tick vector, and a portfolio record's per-lot value and per-lot bound are
//!     representable on the frozen price scale.  That split is not this
//!     module's invention — it is the one `verify_page_on_grid` already applies
//!     to every record stored on the page;
//! 11. the order id is the one the page's own state fixes
//!     ([`clutch_solana_layout::stream::OrderPageHeader::next_order_id`]), which
//!     is also where a full page and a frozen page refuse;
//! 12. the signer owns the canonical Position, its generation binds the
//!     reservation identity, and the exact reservation fits its free cash or
//!     internal Egg vector; and
//! 13. create and encode the reservation PDA and write Position/page through
//!     their owner codecs in the same instruction.
//!
//! `CancelOrder`, from there:
//!
//! 3. the page, in full, through `stream::verify_page` — no grid;
//! 4. the epoch is `EPOCH_PHASE_OPEN` and the page is unfrozen;
//! 5. the page belongs to this epoch and this market;
//! 6. the intent's own `market` and `epoch` name the same epoch;
//! 7. replay: the retirement's generation **is** the request envelope's
//!    sequence, and the intent's declared generation must equal it;
//! 8. the actor is the owner the intent names;
//! 9. the id names a populated slot **of this page**, by arithmetic
//!    ([`clutch_solana_layout::stream::OrderPageHeader::slot_index_of`]);
//! 10. that live slot, Position, and active reservation agree on every frozen
//!     identity and exact initial envelope; and
//! 11. tombstone the slot, return only the reservation's remaining assets, and
//!     mark it released with zero remaining assets.
//!
//! The page prechecks at steps 11 and 9 respectively are *optional*: the writers refuse the same
//! mismatches themselves, and they refuse them before touching a byte, so a
//! refused transition leaves the account unchanged whether or not this module
//! pre-checks.  They are kept because they cost no fold and they preserve this
//! module's stated property — every check runs, in this module's own error
//! vocabulary, before any write is attempted.
//!
//! ## Cancellation
//!
//! At the previous page revision this refused, and the refusal was a reading of
//! the codec rather than an implementation gap: the frozen page format had no
//! way to say "this order is retired".  `OrderSlot` was `Empty | Single |
//! Portfolio`, an `Empty` slot below `order_count` was a *missing* order rather
//! than a cancelled one, and no record field or reserved flag bit could carry a
//! status.  The one representation the bytes did admit was delete-and-compact,
//! which is sound only on a single-page book, leaves a multi-page book
//! unfreezable, and — decisively — *is* the cancellation semantics, which an
//! instruction may not freeze on the layout's behalf.
//!
//! v4 answers it. [`clutch_solana_layout::ORDER_KIND_TOMBSTONE`] is a fourth
//! slot kind carrying a [`clutch_solana_layout::TombstoneRecord`], and the rule
//! is **retire in place**: a cancellation replaces the record in its slot and
//! keeps both the slot and the id, so `order_count`, `first_order_id`, and
//! `last_order_id` do not move and no later order is renumbered.  That matters
//! more at v4 than it would have before, because ids are now positional, so
//! renumbering would silently rewrite identities that receipts, candidates, and
//! clients already name.  Only `tombstone_count` and the page digest change.
//!
//! What this instruction adds to the writer is the three facts the page cannot
//! see: that the epoch is open, that the account list supplied *this* market's
//! and *this* epoch's page, and that the actor signing is the owner named.
//! Three further notes:
//!
//! * **Replay.**  A placement uses the page's `order_count` as its counter; a
//!   cancellation does not move that count, so it needs a different one.  It
//!   has two, and both are state rather than a caller's assertion: the slot
//!   kind — `write_tombstone` refuses a slot that is not a *live* record, so a
//!   replayed cancellation refuses with `CodecError::MismatchedBinding` — and
//!   the generation rule, `generation > retired_generation`, which
//!   `TombstoneRecord::validate` refuses with `CodecError::InvalidEnum`.  The
//!   value bound as the retirement's generation is the **request envelope's
//!   sequence**.  The intent carries a `generation` field as well and it is an
//!   assertion, exactly as a placement's order id is: a wire that names one
//!   generation while the envelope carries another refuses
//!   [`ClutchError::Replay`] rather than retiring at a generation the caller
//!   never stated.
//! * **Which page.**  The target order id *is* the page and the slot.  A
//!   canonical id decodes to a rank, `rank / MAX_ORDERS_PER_PAGE` is the page
//!   that owns it, and `slot_index_of` recovers the slot index by subtraction;
//!   nothing searches the slot array.  An account list that supplies the wrong
//!   page refuses `CodecError::MismatchedBinding`, which is *this page's*
//!   verdict about the id rather than a verdict about the id itself — it may be
//!   a perfectly good id on another page, and this page is not the one to say
//!   otherwise.
//! * **What a retirement releases.** It returns exactly one still-active
//!   reservation's remaining assets to the same-generation Position. It does
//!   not debit the Hoard or infer an amount from the page alone. The retirement
//!   sits *inside* the page-set commitment — its bytes are slot bytes, so they
//!   are in the page digest and therefore in the order-set fold — and both page
//!   and Epoch must still be open. A replay sees a tombstone and a released
//!   reservation and cannot release twice.
//!
//! ## Settlement: the entitled consumption seams, and what is left
//!
//! The original batch relation still does not fit an SBF frame: its measured
//! `verify_inner` frame is 39,104 bytes against the 4,096-byte maximum.  That
//! is no longer the current blocker.  `clutch_batch::relation_v1_stream`
//! reproduces the batch verdict with one order at a time; its largest measured
//! frame is 1,280 bytes and its resumable host gate has zero observed verdict
//! divergence.
//!
//! `settlement::verify_preflight` now executes every join which can be stated
//! without inventing protocol facts: it recomputes the complete frozen page
//! set, binds it to the epoch, binds a submitted candidate record to that
//! epoch, binds the solver feed field-for-field to the candidate and order set,
//! and binds the layout-owned ClearWork header and page cursor.  It writes
//! nothing and it does not interpret the opaque checkpoint body.
//!
//! The eight-row STOP this section used to carry — no reservation-set
//! closure, no live-order join, no frozen policy preimage, no full-width
//! relation domain, no stable checkpoint codec, no authenticated checkpoint
//! or feed creation, no on-chain candidate closure and selection, and pot and
//! receipt as codecs rather than entitlements — is **discharged**, row by row,
//! by the joins this module and its submodules now execute.  The record is not
//! this prose: it is `settlement::RETIRED_SETTLEMENT_BLOCKERS` and
//! `settlement::SETTLEMENT_BLOCKERS`, whose standing list is empty and whose
//! retirement notes carry each row's derivation.  A fail-closed test pins that
//! every row filed appears in exactly one of the two.
//!
//! What still refuses at this seam is authority rather than a missing join: a
//! nonzero `max_fee_atoms` on either end is `AuthorizationUnavailable`, the
//! reserved fee zone of a reservation is validated zero, and the per-owner
//! conversion coincidence the relation's rounding boundary needs is *checked*
//! (`distinct_owners == filled_order_count`) rather than assumed.
//!
//! The typed preflight remains executable in `settlement` and is not a
//! settlement verdict.  `SubmitDirectPage` removes the caller-provided feed
//! from the narrow path but leaves it explicitly `SUBMITTED`; neither it nor
//! the page is selection authority.
//!
//! **Post-resolution direction (PROPOSED, not integrated).** Verification,
//! selection, and the complete receipt/pot entitlement set must be frozen
//! before resolution.  A later resolution may not authorize generic account
//! mutation; it may only allow consumption of one already-frozen entitlement,
//! under the relation's `T-b` transfer phase.  Lazy filling therefore need not
//! delay resolution, while resolution can never create or re-price a receipt.
//!
//! ## What the wire carries
//!
//! At intent v1, `Intent::PlaceOrder` carried a bare
//! [`clutch_solana_layout::OrderRecord`], so **a portfolio placement was not
//! expressible at all**: a page could hold `OrderSlot::Portfolio` records and
//! no intent could put one there.  That was a wire gap rather than an
//! unimplemented branch, and intent v2 closes it. Intent v3 additionally binds
//! `max_fee_atoms`, making the owner-signed admission envelope exact.
//! `PlaceOrder` now carries an [`clutch_solana_layout::OrderSlot`], encoded as
//! the shared identities, fee cap, kind byte, and that kind's *exact* body —
//! 182 bytes single-Egg, 310 portfolio — and
//! `MAX_INTENT_BYTES` is the wider of the two exactly rather than a round
//! number with slack in it.
//!
//! The portfolio placement path is **implemented** here rather than named as a
//! gap, because its validation surface is the same shape as the single-Egg one
//! check for check, not merely similar:
//!
//! | check | single-Egg | portfolio |
//! | --- | --- | --- |
//! | record codec | `OrderRecord::validate` | `PortfolioRecord::validate` |
//! | the epoch's outcome width | `outcome < outcome_count` | `active_len <= outcome_count` |
//! | the epoch's horizon | `expiry_epoch >= epoch_index` | `expiry_epoch >= epoch_index` |
//! | the frozen grid | `limit` is an exact tick | per-lot value and bound are representable on the scale |
//! | the writer | `stream::append_slot` | `stream::append_slot` |
//!
//! Each right-hand cell is the refusal the layout crate already applies to a
//! *stored* portfolio record — the first two through `epoch_binds_page_set`,
//! the third through `PortfolioRecord::validate_on_scale` inside
//! `verify_page_on_grid` — so implementing the family adds no economic
//! judgement this module is not entitled to make.  `ORDER_KIND_EMPTY` and
//! `ORDER_KIND_TOMBSTONE` are recognized kinds that are not placements; the
//! intent codec refuses both with `CodecError::InvalidEnum` before this module
//! sees them, and this module refuses them again in its own `match`, because a
//! stated refusal on an unreachable arm is cheaper than an assumption.
//!
//! ## Named gaps
//!
//! Each of these is a fact about what an accepted placement or cancellation
//! does *not* do.
//!
//! * **Reservations are per-order, but no global frozen reservation-set
//!   commitment exists.** Placement and cancellation own exact assets, and the
//!   narrow settlement seam consumes two of them. No transition yet proves
//!   that every order in a selected candidate has exactly one reservation or
//!   constructs every receipt before resolution.
//! * **Nothing moves an epoch out of `EPOCH_PHASE_OPEN`, and nothing creates a
//!   page.**  No intent in the wire freezes a page set or initializes a page,
//!   so a book placed by this program can never be closed, `page_count`,
//!   `set_order_count`, and `order_set` stay zero forever, and both
//!   instructions here require a page some other process already wrote.
//!   `stream::init_page`, `frozen_set_commitment`, and `seal_page` exist and
//!   are called by nothing.
//! * **`generation` is now read — by cancellation, and only there.**  The
//!   layout documents it as replay protection for the placing instruction; a
//!   placement's actual replay counter is still the page's own `order_count`,
//!   which is state rather than a caller assertion, and the field it writes is
//!   read exactly once, when a retirement must strictly follow it.  Nothing
//!   binds either number to a position generation, because no position account
//!   is in either list.
//! * **Per-order expiry is a dead-on-arrival refusal and nothing more.**  A
//!   page set belongs to one epoch and no mechanism carries an order from one
//!   epoch's book into the next, so checking `expiry_epoch` against the epoch
//!   index refuses an order that could never clear and implies no good-till-
//!   cancelled behaviour.
//! * **The page set's shape is unbound while open.**
//!   `EpochAccount::validate` forces `page_count == 0` on an open epoch, so
//!   nothing cross-checks one page's `page_count` against another's, or against
//!   the epoch's, until the set is frozen.
//! * **Distinct-owner admission is unchecked.**  `EpochAccount.owner_count`
//!   bounds the relation's owner tags, and the identity-to-tag interning is
//!   unspecified (above), so neither instruction can tell whether one more
//!   distinct owner is admissible.
//!
//! One v3 gap is **closed** rather than carried: order ids were caller-chosen,
//! and a single caller could place one order with an id of `0xff..ff` after
//! which no further order could be placed on that page or on any later page of
//! the set.  At v4 an id is `canonical_order_id(rank)` and a caller has none to
//! choose, so the griefing vector is gone by construction rather than by a
//! check — which is also why step 11 above is an equality against
//! `next_order_id()` instead of an ordering against a predecessor.
//!
//! ## Frames and compute
//!
//! No function in this module holds a page, a book, or a candidate.  The
//! largest values in flight are a decoded `PriceGridAccount` (a
//! `[u64; MAX_GRID_TICKS]` tick vector), one
//! [`clutch_solana_layout::OrderSlot`] — 236 bytes, a width the portfolio
//! family's coefficient vector sets — and one
//! [`clutch_solana_layout::stream::OrderPageHeader`]; the grid is loaded into a
//! caller slot through an `#[inline(never)]` out-parameter rather than
//! returned, for the reason [`super::observe_resolve`] records.
//! `cargo-build-sbf` reports no frame diagnostic for any `clutch_sbf` function,
//! which is the only check there is.
//!
//! Compute is **not** measured.  What can be stated from the frozen widths is
//! the fold structure, and at v4 it is *smaller* than it was, in spite of a
//! wider page.  The page-digest preimage grew from 3,743 bytes to 3,872 — a
//! 28-byte domain, market, epoch, `page_index`, `order_count`, the new
//! `tombstone_count`, and sixteen 236-byte slots — so one fold is 61 SHA-256
//! compression blocks rather than 59.  Against that, the post-write
//! re-verification is gone: an accepted placement folds the page **twice**, the
//! pre-state verify and the writer's closing fold, where it folded three times
//! before.  So the documented figure moves from `3 x 59 = 177` blocks to
//! `2 x 61 = 122`, and full slot-decode passes drop from five to three — the
//! verify's structural fold, the verify's record-semantics sweep, and the
//! writer's closing fold.  A cancellation costs the same two folds and the same
//! three passes, plus one single-slot decode, and reads no grid at all.
//! `the_documented_page_fold_follows_from_the_frozen_widths` is what keeps that
//! arithmetic honest if the page grows again.
//!
//! The rest of the repeated work is unchanged in kind: three
//! `sol_try_find_program_address` calls for a placement and two for a
//! cancellation, two `EpochAccount` decodes (each recomputing the canonical
//! epoch identity), and — for a placement only — two `PriceGridAccount` decodes
//! plus a third grid-digest recompute inside `verify_page_on_grid`, and one
//! partial grid sweep over the populated slots.  The repeated decodes are the
//! same shape [`super::observe_resolve`] records and have the same cause: each
//! check must run in its own small frame and at its own point in the order, and
//! the layout crate publishes no facts-only or `decode_unchecked` entry point.
//!
//! The one measured comparison point in this repository is `Split` at 72,869
//! compute units with eight address derivations and no page fold, so either of
//! these is expected to need an explicit compute-budget request rather than to
//! fit the 200,000-unit default.  **That expectation is an obligation to
//! measure, not a measurement**, and the v3 figures it replaces were never
//! measured either; measuring it means `PlaceOrder` and `CancelOrder` fixtures
//! in the differential harness, which is that lane's file.
//!
//! ## Refusal codes
//!
//! `0x0060-0x006f` is reserved for this module and **this wave still allocates
//! none of it**, following [`super::observe_resolve`]'s precedent: every
//! refusal raised here already has an owner.  A page that is not a page refuses
//! with the frozen codec's own [`clutch_solana_layout::CodecError`]; a full
//! page refuses `CodecError::InvalidCount`, which is literally what
//! `OrderPageHeader::next_order_id` says about the count that would result; a
//! replayed cancellation refuses `CodecError::MismatchedBinding`, which is what
//! `write_tombstone` says about a slot that is not a live record; the account
//! plane refuses with an existing [`crate::error::ClutchError`].
//!
//! The v3 proposal reserved `0x0060` for "order cancellation has no
//! representation in the frozen page format".  That refusal no longer exists,
//! so the code is **withdrawn** rather than allocated, and the proposal for
//! whoever unfreezes `error.rs` is one line shorter than it was:
//!
//! | class | proposed code |
//! | --- | --- |
//! | candidate verification/checkpoint joins are not integrated | `0x0061` |
//! | `0x0060` and `0x0062-0x006f` | unallocated |
//!
//! The narrow seam uses existing typed codec/adapter refusals.  Broader
//! settlement is made unrepresentable by its checks rather than bridged with a
//! lossy projection or a `repr(Rust)` byte cast.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::direct_selection_v3::{
    create_pda_account_full_principal, direct_creation_funding, observe_direct_funding,
    DIRECT_NEUTRAL_SINK_V3, DIRECT_VERIFIER_RELEASE_ID_V3,
};
use crate::instructions::genesis::RentParameters;
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program,
};
use crate::instructions::split;
use crate::seeds;
use clutch_solana_layout::direct_selection_v3::{
    DirectBatchPolicyV3, DirectEpochV4Account, DirectReservationV2Account,
    DIRECT_BATCH_POLICY_V3_BYTES, DIRECT_EPOCH_V4_BYTES, DIRECT_RESERVATION_V2_BYTES,
};
use clutch_solana_layout::{
    account_len,
    clearing::{init_candidate_feed, verify_candidate_feed, write_fill, write_slice_at},
    reservation::{canonical_reservation_id, ReservationAccount, RESERVATION_ACCOUNT_BYTES},
    stream, CandidateRecord, CodecError, EpochAccount, Hash32, Intent, OrderSlot, PositionAccount,
    PriceGridAccount, SettlementReceiptAccount, EPOCH_PHASE_OPEN, MAX_GRID_TICKS,
};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

pub mod clear_walk;
pub mod clear_work;
pub mod entitlement;
pub mod general_epoch;
mod reservation;
pub mod selection;
pub(super) mod settlement;
pub mod terminal_closure;

/// Copy one static value onto the heap without materializing it on a frame.
///
/// The clearing plane's boxed-decode idiom for values near or past the 4-KiB
/// frame bound (`ClearWorkV1` ~48.7 KiB, `OwnerInterner` 2,050 B): the source
/// lives in static storage, the copy is a straight `memcpy` into a fresh
/// allocation, and no call frame ever holds the value.
///
/// SAFETY inside: `T: Copy` has no drop obligations and no interior
/// references, so a byte copy of a valid static value is a valid value, and
/// the pointer is freshly allocated for exactly `T`'s layout.
pub(in crate::instructions) fn boxed_copy_of<T: Copy>(source: &'static T) -> Outcome<Box<T>> {
    let layout = core::alloc::Layout::new::<T>();
    unsafe {
        let pointer = std::alloc::alloc(layout) as *mut T;
        if pointer.is_null() {
            return Err(Refusal::Adapter(ClutchError::AccountCreationFailed));
        }
        core::ptr::copy_nonoverlapping(source as *const T, pointer, 1);
        Ok(Box::from_raw(pointer))
    }
}

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
/* Account lists                                                             */
/* ------------------------------------------------------------------------ */

/// Accounts in a `PlaceOrder` instruction, exactly.
pub const PLACE_ORDER_ACCOUNT_COUNT: usize = 8;
/// Accounts in the still-unrouted Direct V3 placement branch, exactly.
pub const DIRECT_V4_PLACE_ORDER_ACCOUNT_COUNT: usize = 9;

/// Accounts in a `CancelOrder` instruction, exactly.
///
/// One fewer than a placement: a retirement has no limit and no lots, so the
/// frozen price grid is not in the list at all.
pub const CANCEL_ORDER_ACCOUNT_COUNT: usize = 5;

/// Accounts in the generalized entitled direct-slice `SettlePage` shape,
/// exactly; any other list dispatches to the portfolio full-pair shape
/// (`entitlement::settle_portfolio_pair`, `pub(super)`).
pub const SETTLE_PAGE_ACCOUNT_COUNT: usize = 7;

/// Accounts in the direct-slice `SettlePage` shape that also presents the
/// epoch's [`FinalPotAccount`] — required exactly when a slice completes an
/// end whose whole-order value does not convert to whole collateral atoms,
/// because the pot's verified residue expectation is what that completion
/// draws down.
pub const SETTLE_PAGE_POTTED_ACCOUNT_COUNT: usize = SETTLE_PAGE_ACCOUNT_COUNT + 1;

/// Accounts in the **virtual-leg** `SettlePage` shape, exactly.
///
/// One real end instead of two, plus the epoch pot it trades against, plus
/// the five accounts one pooled complete-set mint authenticates
/// (`split::PooledSetRoles`).  It is longer than either direct shape because
/// a virtual leg is the one settlement that changes the market's outstanding
/// supply, and every account that truth lives in has to be in the list.
///
/// The count alone does not select it: the portfolio full-pair shape is
/// variable-length and can also be eleven accounts long, so the pot's exact
/// data length at [`IX_VSETTLE_POT`] is the discriminator, checked before
/// anything is decoded.
pub const SETTLE_VIRTUAL_ACCOUNT_COUNT: usize = 11;

/// Cleared Epoch.  Virtual `SettlePage`.
pub const IX_VSETTLE_EPOCH: usize = 0;
/// Selected candidate record; its `virtual_split` sizes the mint.
pub const IX_VSETTLE_CANDIDATE: usize = 1;
/// The one real end's Position (writable).
pub const IX_VSETTLE_POSITION: usize = 2;
/// The one real end's stamped reservation (writable).
pub const IX_VSETTLE_RESERVATION: usize = 3;
/// The entitled virtual receipt of exactly one candidate slice (writable).
pub const IX_VSETTLE_RECEIPT: usize = 4;
/// The epoch's final pot: the virtual leg's counterparty (writable).
pub const IX_VSETTLE_POT: usize = 5;
/// The market account (read-only).
pub const IX_VSETTLE_MARKET: usize = 6;
/// The Hoard collateral account (writable).
pub const IX_VSETTLE_HOARD: usize = 7;
/// The reference-only kernel aggregate (writable).
pub const IX_VSETTLE_KERNEL: usize = 8;
/// The market-wide two-term supply ledger (writable).
pub const IX_VSETTLE_SUPPLY: usize = 9;
/// The Hoard's Token-2022 collateral account, mirrored and never moved.
pub const IX_VSETTLE_HOARD_TOKEN: usize = 10;

/// Accounts in the narrow `SubmitDirectPage` constructor, exactly.
pub const SUBMIT_DIRECT_PAGE_ACCOUNT_COUNT: usize = 11;

/// Authenticated actor; its key is the order's owner identity.  Both lists.
pub const IX_ACTOR: usize = 0;
/// Epoch/book-domain account.  Both lists.
pub const IX_EPOCH: usize = 1;
/// Frozen price-grid account.  `PlaceOrder` only.
pub const IX_GRID: usize = 2;
/// The order page the record is appended to.  `PlaceOrder`.
pub const IX_PAGE: usize = 3;
/// Owner Position whose free assets fund a placement. `PlaceOrder`.
pub const IX_POSITION: usize = 4;
/// Newly created per-order reservation account. `PlaceOrder`.
pub const IX_RESERVATION: usize = 5;
/// System program. `PlaceOrder`.
pub const IX_SYSTEM: usize = 6;
/// Rent sysvar. `PlaceOrder`.
pub const IX_RENT: usize = 7;
/// Exact 96-byte epoch-bound DirectBatchPolicy V3 artifact. Direct V4 only.
pub const IX_DIRECT_V4_POLICY: usize = 8;
/// The order page the retirement is written into.  `CancelOrder`.
pub const IX_CANCEL_PAGE: usize = 2;
/// Owner Position receiving a cancellation release. `CancelOrder`.
pub const IX_CANCEL_POSITION: usize = 3;
/// Existing per-order reservation released by cancellation. `CancelOrder`.
pub const IX_CANCEL_RESERVATION: usize = 4;
/// Cleared Epoch. `SettlePage`.
pub const IX_SETTLE_EPOCH: usize = 0;
/// Selected candidate record. `SettlePage`.
pub const IX_SETTLE_CANDIDATE: usize = 1;
/// Buyer Position. `SettlePage`.
pub const IX_SETTLE_BUY_POSITION: usize = 2;
/// Seller Position. `SettlePage`.
pub const IX_SETTLE_SELL_POSITION: usize = 3;
/// Buy order's exact ENTITLED reservation. `SettlePage`.
pub const IX_SETTLE_BUY_RESERVATION: usize = 4;
/// Sell order's exact ENTITLED reservation. `SettlePage`.
pub const IX_SETTLE_SELL_RESERVATION: usize = 5;
/// The entitled receipt of exactly one candidate slice. `SettlePage`.
pub const IX_SETTLE_RECEIPT: usize = 6;
/// The epoch's final pot, on the potted `SettlePage` shape only.
pub const IX_SETTLE_POT: usize = 7;
/// Permissionless rent payer and authenticated transaction signer.
pub const IX_SUBMIT_PAYER: usize = 0;
/// Frozen Epoch owning the submitted candidate.
pub const IX_SUBMIT_EPOCH: usize = 1;
/// Frozen price grid used by both candidate prices.
pub const IX_SUBMIT_GRID: usize = 2;
/// The complete one-page frozen order set.
pub const IX_SUBMIT_PAGE: usize = 3;
/// Reservation for live order index zero.
pub const IX_SUBMIT_RESERVATION_ZERO: usize = 4;
/// Reservation for live order index one.
pub const IX_SUBMIT_RESERVATION_ONE: usize = 5;
/// Canonical, not-yet-created Candidate PDA.
pub const IX_SUBMIT_CANDIDATE: usize = 6;
/// Canonical, not-yet-created CandidateFeed PDA.
pub const IX_SUBMIT_FEED: usize = 7;
/// System program.
pub const IX_SUBMIT_SYSTEM: usize = 8;
/// Rent sysvar.
pub const IX_SUBMIT_RENT: usize = 9;
/// Clock sysvar, owning `CandidateRecord.submitted_slot`.
pub const IX_SUBMIT_CLOCK: usize = 10;

/// Program-owned roles of `PlaceOrder`, in account-index order.
const PLACE_ORDER_STATE_ROLES: [StateRole; 3] = [
    StateRole::read_only(IX_GRID, account_len::PRICE_GRID),
    StateRole::writable(IX_PAGE, account_len::ORDER_PAGE),
    StateRole::writable(IX_POSITION, account_len::POSITION),
];

/// Program-owned roles unique to the Direct V4 placement branch.
const DIRECT_V4_POLICY_ROLE: [StateRole; 1] = [StateRole::read_only(
    IX_DIRECT_V4_POLICY,
    DIRECT_BATCH_POLICY_V3_BYTES,
)];

/// Program-owned roles of `CancelOrder`, in account-index order.
const CANCEL_ORDER_STATE_ROLES: [StateRole; 3] = [
    StateRole::writable(IX_CANCEL_PAGE, account_len::ORDER_PAGE),
    StateRole::writable(IX_CANCEL_POSITION, account_len::POSITION),
    StateRole::writable(IX_CANCEL_RESERVATION, RESERVATION_ACCOUNT_BYTES),
];

/// Program-owned roles of the entitled direct-slice settlement shape.
const SETTLE_PAGE_STATE_ROLES: [StateRole; SETTLE_PAGE_ACCOUNT_COUNT] = [
    StateRole::read_only(IX_SETTLE_EPOCH, account_len::EPOCH),
    StateRole::read_only(IX_SETTLE_CANDIDATE, account_len::CANDIDATE),
    StateRole::writable(IX_SETTLE_BUY_POSITION, account_len::POSITION),
    StateRole::writable(IX_SETTLE_SELL_POSITION, account_len::POSITION),
    StateRole::writable(IX_SETTLE_BUY_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_SETTLE_SELL_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_SETTLE_RECEIPT, account_len::SETTLEMENT_RECEIPT),
];

/// Program-owned roles of the potted direct-slice settlement shape.
const SETTLE_PAGE_POTTED_STATE_ROLES: [StateRole; SETTLE_PAGE_POTTED_ACCOUNT_COUNT] = [
    StateRole::read_only(IX_SETTLE_EPOCH, account_len::EPOCH),
    StateRole::read_only(IX_SETTLE_CANDIDATE, account_len::CANDIDATE),
    StateRole::writable(IX_SETTLE_BUY_POSITION, account_len::POSITION),
    StateRole::writable(IX_SETTLE_SELL_POSITION, account_len::POSITION),
    StateRole::writable(IX_SETTLE_BUY_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_SETTLE_SELL_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_SETTLE_RECEIPT, account_len::SETTLEMENT_RECEIPT),
    StateRole::writable(IX_SETTLE_POT, account_len::FINAL_POT),
];

/// Program-owned roles of the virtual-leg settlement shape's own half.
///
/// The five pooled-mint accounts are validated by
/// [`split::pooled_set_transition`] against the same role table the seam
/// plane uses, so they are deliberately absent here rather than checked
/// twice from two places that could drift.
const SETTLE_VIRTUAL_STATE_ROLES: [StateRole; 5] = [
    StateRole::read_only(IX_VSETTLE_EPOCH, account_len::EPOCH),
    StateRole::read_only(IX_VSETTLE_CANDIDATE, account_len::CANDIDATE),
    StateRole::writable(IX_VSETTLE_POSITION, account_len::POSITION),
    StateRole::writable(IX_VSETTLE_RESERVATION, RESERVATION_ACCOUNT_BYTES),
    StateRole::writable(IX_VSETTLE_RECEIPT, account_len::SETTLEMENT_RECEIPT),
];

/// Where the virtual shape presents the five pooled complete-set accounts.
const SETTLE_VIRTUAL_POOLED_ROLES: split::PooledSetRoles = split::PooledSetRoles {
    market: IX_VSETTLE_MARKET,
    hoard: IX_VSETTLE_HOARD,
    kernel: IX_VSETTLE_KERNEL,
    supply: IX_VSETTLE_SUPPLY,
    hoard_token: IX_VSETTLE_HOARD_TOKEN,
};

/// Program-owned frozen inputs to deterministic direct submission.
const SUBMIT_DIRECT_PAGE_STATE_ROLES: [StateRole; 5] = [
    StateRole::read_only(IX_SUBMIT_EPOCH, account_len::EPOCH),
    StateRole::read_only(IX_SUBMIT_GRID, account_len::PRICE_GRID),
    StateRole::read_only(IX_SUBMIT_PAGE, account_len::ORDER_PAGE),
    StateRole::read_only(IX_SUBMIT_RESERVATION_ZERO, RESERVATION_ACCOUNT_BYTES),
    StateRole::read_only(IX_SUBMIT_RESERVATION_ONE, RESERVATION_ACCOUNT_BYTES),
];

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
/// semantics, the positional order-id chain, the retirement count, the padding
/// rule, the stored range, the stored digest, and — because the grid is here —
/// that every limit already on the page is an exact tick and every stored
/// portfolio record is representable on the frozen scale.  It never
/// materializes a slot array.
#[inline(never)]
fn verify_page_on_grid(page: &[u8], grid: &PriceGridAccount) -> Outcome<stream::OrderPageHeader> {
    Ok(stream::verify_page_on_grid(page, grid)?)
}

/// Verify one order page without a grid, streaming, and return its header.
///
/// Everything above except the two grid-dependent record checks.  A
/// cancellation uses this: it neither reads nor writes a limit, a lot count, or
/// a price, so requiring the grid account would be requiring a fact the
/// transition does not use.
#[inline(never)]
fn verify_page(page: &[u8]) -> Outcome<stream::OrderPageHeader> {
    Ok(stream::verify_page(page)?)
}

/* ------------------------------------------------------------------------ */
/* Transitions                                                               */
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
    /// The slot to append: one order, of either family.
    slot: OrderSlot,
}

/// The `CancelOrder` intent's payload: what the wire says is being retired.
#[derive(Clone, Copy, Debug)]
struct Retirement {
    /// The intent's declared market.
    market: Hash32,
    /// The intent's declared epoch.
    epoch: Hash32,
    /// The owner the retirement claims, which must be the record's own.
    owner: Hash32,
    /// The canonical rank being retired; it names the page and the slot.
    order_id: Hash32,
    /// The retirement's declared replay generation, an assertion about the
    /// request envelope's sequence rather than a free choice.
    generation: u64,
}

/// Everything a cancellation needs that is not the page it writes.
#[derive(Clone, Copy, Debug)]
struct Cancellation<'a> {
    /// Epoch account bytes.
    epoch: &'a [u8],
    /// The authenticated actor's key, as a 32-byte identity.
    actor: Hash32,
    /// The request envelope's replay sequence, which **is** the retirement's
    /// generation.
    sequence: u64,
    /// The wire's own account of the retirement.
    intent: Retirement,
}

/// Append one order to an open page, or refuse.
///
/// Every check runs before any byte is written, and the writer holds that
/// property too, so a refusal leaves the account unchanged.  The whole write is
/// [`stream::append_slot`]: this module computes no offset and folds no digest.
fn validate_place_order(
    page: &[u8],
    placement: &Placement<'_>,
) -> Outcome<stream::OrderPageHeader> {
    let epoch = accounts::read_epoch(placement.epoch)?;
    let mut grid = ZERO_GRID;
    load_grid(placement.grid, &mut grid)?;

    // 3. The whole pre-state, on the frozen grid.
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

    /* 7. Replay.  The page's own populated-slot count is the counter, so a
     * replayed transaction is refused by state rather than by a caller's
     * assertion, exactly as `FeedAdvance` uses the feed's page counter.  A
     * retirement does not move this count, which is why a cancellation needs a
     * different counter entirely — see the module docs. */
    require(
        placement.sequence == header.order_count as u64,
        ClutchError::Replay,
    )?;

    // 8. The actor is the owner the record claims.
    require(
        placement.actor == placement.slot.owner(),
        ClutchError::UnauthorizedActor,
    )?;

    /* 9 and 10.  The record codec, then the two bounds only the epoch knows —
     * this market's actual outcome width, which no page can bound below
     * `MAX_OUTCOMES`, and this epoch's index, which no page can recover from
     * the 32-byte epoch identity it stores — then the grid half.  Both epoch
     * bounds are the refusals `stream::epoch_binds_page_set` gives a frozen
     * set, applied one record early; the grid half is the split
     * `stream::verify_page_on_grid` applies to every record already stored. */
    match &placement.slot {
        OrderSlot::Single(order) => {
            order.validate()?;
            if order.outcome >= epoch.outcome_count || order.expiry_epoch < epoch.epoch_index {
                return Err(CodecError::MismatchedBinding.into());
            }
            grid.tick_of(order.limit)?;
        }
        OrderSlot::Portfolio(order) => {
            order.validate()?;
            if order.active_len > epoch.outcome_count || order.expiry_epoch < epoch.epoch_index {
                return Err(CodecError::MismatchedBinding.into());
            }
            /* A portfolio bound is a per-lot collateral in complete-set units,
             * not a per-outcome price, so it has no tick to look up.  What the
             * grid contributes is the frozen scale, on which the per-lot value
             * and the per-lot bound must be representable at all. */
            order.validate_on_scale(grid.price_scale)?;
        }
        /* Unreachable through the wire: `Intent::decode` refuses both kinds as
         * placements.  Stated, not assumed. */
        OrderSlot::Empty | OrderSlot::Tombstone(_) => {
            return Err(CodecError::InvalidEnum.into());
        }
    }

    /* 11. The id is not a choice; the page's own state fixes it, and this is
     * also where a full page and a frozen page refuse.  `append_slot` refuses
     * the same mismatch with `NonCanonicalIdentity`; the pre-check is kept so
     * that every check runs in this module's vocabulary before any write. */
    require(
        placement.slot.order_id() == header.next_order_id()?,
        ClutchError::MismatchedState,
    )?;

    Ok(header)
}

/// Validate the fixed two-order Direct V4 admission profile.
///
/// This remains a branch of the existing `PlaceOrder` wire: the account
/// version selects it, and no Direct V3 lifecycle tag is routed by this seam.
#[allow(clippy::too_many_arguments)]
fn validate_direct_v4_place(
    page: &[u8],
    epoch: &DirectEpochV4Account,
    grid: &PriceGridAccount,
    actor: Hash32,
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    max_fee_atoms: u64,
    slot: OrderSlot,
) -> Outcome<stream::OrderPageHeader> {
    /* The caller decoded (and therefore fully validated) this exact epoch in
     * the same instruction; only the release binding, the sole placement
     * phase, and the digest-free funding shape need re-stating here. */
    require(
        epoch.verifier_release_id == DIRECT_VERIFIER_RELEASE_ID_V3
            && epoch.lifecycle_phase
                == clutch_solana_layout::direct_selection_v3::DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN
            && epoch.terminal
                == clutch_solana_layout::direct_selection_v3::DirectTerminalReceiptV3::EMPTY,
        ClutchError::NotActive,
    )?;
    epoch
        .page_funding
        .validate_for_sink(epoch.neutral_lamport_sink)?;
    epoch
        .epoch_funding
        .validate_for_sink(epoch.neutral_lamport_sink)?;
    grid.validate()?;
    let header = verify_page_on_grid(page, grid)?;
    let common = epoch.direct.common;
    require(
        epoch.neutral_lamport_sink == Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes())
            && common.outcome_count == 2
            && common.page_count == 1
            && header.market == common.market
            && header.epoch == common.epoch
            && header.page_index == 0
            && header.page_count == 1
            && header.set_order_count == 0
            && header.order_count < 2
            && header.tombstone_count == 0
            && header.frozen == 0
            && grid.grid == common.price_grid
            && grid.price_scale == common.price_scale
            && intent_market == common.market
            && intent_epoch == common.epoch
            && max_fee_atoms == 0,
        ClutchError::MismatchedState,
    )?;
    require(
        sequence == u64::from(header.order_count),
        ClutchError::Replay,
    )?;
    require(actor == slot.owner(), ClutchError::UnauthorizedActor)?;
    let order = match slot {
        OrderSlot::Single(order) => order,
        OrderSlot::Empty | OrderSlot::Portfolio(_) | OrderSlot::Tombstone(_) => {
            return Err(CodecError::InvalidEnum.into());
        }
    };
    order.validate()?;
    require(
        order.outcome < common.outcome_count
            && order.minimum_fill == 0
            && order.flags == 0
            && order.expiry_epoch >= common.epoch_index
            && order.order_id == header.next_order_id()?,
        ClutchError::MismatchedState,
    )?;
    grid.tick_of(order.limit)?;
    /* A reservation that encumbers nothing is refused at creation: with the
     * profile's forced zero fee, a zero-limit buy would reserve zero cash and
     * zero Eggs, and its release would be the Position no-op the release
     * kernel's unchanged-poststate rule refuses forever. */
    require(
        order.side == 1 || order.limit != 0,
        ClutchError::MismatchedState,
    )?;
    Ok(header)
}

#[cfg(test)]
fn apply_place_order(page: &mut [u8], placement: &Placement<'_>) -> Outcome<()> {
    validate_place_order(page, placement)?;
    // Slot bytes, header, and digest, in one call.
    stream::append_slot(page, placement.slot)?;
    Ok(())
}

/// Retire one order in place on an open page, or refuse.
///
/// The whole write is [`stream::write_tombstone`], which keeps the slot and the
/// id, moves only `tombstone_count` and the page digest, and refuses a slot
/// that is not a live record of this owner — which is what makes a replayed
/// cancellation refuse on state rather than on a caller's assertion.
fn validate_cancel_order(
    page: &[u8],
    cancellation: &Cancellation<'_>,
) -> Outcome<(stream::OrderPageHeader, OrderSlot)> {
    let epoch = accounts::read_epoch(cancellation.epoch)?;

    /* 3. The whole pre-state.  No grid: a retirement carries no limit and no
     * lots, and it does not re-price the records already on the page. */
    let header = verify_page(page)?;

    // 4. Cancellations are admitted only while the book is open.
    require(epoch.phase == EPOCH_PHASE_OPEN, ClutchError::NotActive)?;
    require(header.frozen == 0, ClutchError::NotActive)?;

    // 5. The page is this epoch's and this market's.
    require(
        header.market == epoch.market && header.epoch == epoch.epoch,
        ClutchError::MismatchedState,
    )?;

    // 6. The intent names the same epoch the accounts do.
    require(
        cancellation.intent.market == epoch.market && cancellation.intent.epoch == epoch.epoch,
        ClutchError::MismatchedState,
    )?;

    /* 7. Replay.  The retirement's generation is the request envelope's
     * sequence; the intent's own `generation` is an assertion about it, so a
     * wire naming one generation while the envelope carries another retires
     * nothing.  The refusals that make a *replay* fail are state: the slot kind
     * and the strict generation order, both inside `write_tombstone`. */
    require(
        cancellation.intent.generation == cancellation.sequence,
        ClutchError::Replay,
    )?;

    // 8. The actor is the owner the intent names.
    require(
        cancellation.actor == cancellation.intent.owner,
        ClutchError::UnauthorizedActor,
    )?;

    /* 9. The id names a populated slot of *this* page, by arithmetic rather
     * than by a search.  `write_tombstone` asks the same question again and
     * refuses it identically; the pre-check is kept for the same reason the
     * placement's is. */
    let wanted = header.slot_index_of(cancellation.intent.order_id)?;

    let mut cursor = stream::OrderSlotCursor::new(page)?;
    let mut live_slot = OrderSlot::Empty;
    let mut index = 0usize;
    while let Some(slot) = cursor.next_slot() {
        let slot = slot?;
        if index == wanted {
            live_slot = slot;
            break;
        }
        index += 1;
    }
    if !live_slot.is_live()
        || live_slot.order_id() != cancellation.intent.order_id
        || live_slot.owner() != cancellation.intent.owner
    {
        return Err(CodecError::MismatchedBinding.into());
    }

    Ok((header, live_slot))
}

fn apply_cancel_order(page: &mut [u8], cancellation: &Cancellation<'_>) -> Outcome<()> {
    validate_cancel_order(page, cancellation)?;
    // Retire in place: slot bytes, `tombstone_count`, and the digest.
    stream::write_tombstone(
        page,
        cancellation.intent.order_id,
        cancellation.intent.owner,
        cancellation.sequence,
    )?;
    Ok(())
}

/* ------------------------------------------------------------------------ */
/* Account plane                                                             */
/* ------------------------------------------------------------------------ */

/// Validate hostile accounts and apply exactly one batch-plane transition.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    /* Match through the borrowed envelope.  `Intent::PlaceOrder` carries a
     * full fixed-width portfolio slot; copying the whole Action into this
     * router made its SBF frame 4,736 bytes.  The family handlers own the one
     * copy they actually need, while this frame carries references only. */
    match &request.action {
        Action::Layout(Intent::PlaceOrder {
            market,
            epoch,
            max_fee_atoms,
            slot,
        }) => place_order(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            *max_fee_atoms,
            slot,
        ),
        Action::Layout(Intent::CancelOrder {
            market,
            epoch,
            owner,
            order_id,
            generation,
        }) => cancel_order(
            program_id,
            accounts,
            request.sequence,
            Retirement {
                market: *market,
                epoch: *epoch,
                owner: *owner,
                order_id: *order_id,
                generation: *generation,
            },
        ),
        Action::Layout(Intent::SettlePage {
            market,
            epoch,
            page_index,
        }) => settle_page(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            *page_index,
        ),
        Action::Layout(Intent::SubmitDirectPage {
            market,
            epoch,
            page_index,
        }) => submit_direct_page(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            *page_index,
        ),
        Action::Layout(Intent::InitClearWork {
            market,
            epoch,
            candidate,
        }) => clear_work::init_clear_work(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
        ),
        Action::Layout(Intent::GrowClearWork {
            market,
            epoch,
            candidate,
        }) => clear_work::grow_clear_work(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
        ),
        Action::Layout(Intent::InitEpoch {
            market,
            epoch_index,
            policy,
            freeze_deadline_slot,
        }) => general_epoch::init_epoch(
            program_id,
            accounts,
            request.sequence,
            market,
            *epoch_index,
            policy,
            *freeze_deadline_slot,
        ),
        Action::Layout(Intent::FreezeEpoch { market, epoch }) => {
            general_epoch::freeze_epoch(program_id, accounts, request.sequence, market, epoch)
        }
        Action::Layout(Intent::AdvanceClearWork {
            market,
            epoch,
            candidate,
            max_orders,
        }) => clear_walk::advance_clear_work(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
            *max_orders,
        ),
        Action::Layout(Intent::AdvanceClearSlices {
            market,
            epoch,
            candidate,
            max_slices,
        }) => clear_walk::advance_clear_slices(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
            *max_slices,
        ),
        Action::Layout(Intent::CompleteClearWork {
            market,
            epoch,
            candidate,
        }) => clear_walk::complete_clear_work(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
        ),
        Action::Layout(Intent::SubmitCandidate {
            market,
            epoch,
            prices,
            virtual_split,
            virtual_merge,
            honored_aon_mask,
            declared_slices,
            weighted_direct_volume,
            limit_surplus_price_units,
            distinct_owners,
        }) => selection::submit_candidate(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            prices,
            *virtual_split,
            *virtual_merge,
            *honored_aon_mask,
            *declared_slices,
            *weighted_direct_volume,
            *limit_surplus_price_units,
            *distinct_owners,
        ),
        Action::Layout(Intent::WriteCandidateFeed {
            market,
            epoch,
            candidate,
            chunk,
        }) => selection::write_candidate_feed(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
            chunk,
        ),
        Action::Layout(Intent::SealCandidate {
            market,
            epoch,
            candidate,
        }) => selection::seal_candidate(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
        ),
        Action::Layout(Intent::FinalizeSelection { market, epoch }) => {
            selection::finalize_selection(program_id, accounts, request.sequence, market, epoch)
        }
        Action::Layout(Intent::FreezeEntitlement {
            market,
            epoch,
            candidate,
        }) => entitlement::freeze_entitlement(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
        ),
        Action::Layout(Intent::EntitleSlice {
            market,
            epoch,
            candidate,
            slice_index,
        }) => entitlement::entitle_slice(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
            *slice_index,
        ),
        Action::Layout(Intent::ReleaseTerminalReservation { market, epoch }) => {
            terminal_closure::release_terminal_reservation(
                program_id,
                accounts,
                request.sequence,
                market,
                epoch,
            )
        }
        Action::Layout(Intent::CloseGeneralReceipt {
            market,
            epoch,
            candidate,
            slice_index,
        }) => terminal_closure::close_general_receipt(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
            *slice_index,
        ),
        Action::Layout(Intent::CloseGeneralReservation { market, epoch }) => {
            terminal_closure::close_general_reservation(
                program_id,
                accounts,
                request.sequence,
                market,
                epoch,
            )
        }
        Action::Layout(Intent::ClosePosition { market, owner }) => {
            terminal_closure::close_position(program_id, accounts, request.sequence, market, owner)
        }
        Action::Layout(Intent::CloseGeneralPage {
            market,
            epoch,
            page_index,
        }) => terminal_closure::close_general_page(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            *page_index,
        ),
        Action::Layout(Intent::CloseGeneralPot { market, epoch }) => {
            terminal_closure::close_general_pot(
                program_id,
                accounts,
                request.sequence,
                market,
                epoch,
            )
        }
        Action::Layout(Intent::CloseGeneralCandidate {
            market,
            epoch,
            candidate,
        }) => terminal_closure::close_general_candidate(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
        ),
        Action::Layout(Intent::CloseGeneralClearWork {
            market,
            epoch,
            candidate,
        }) => terminal_closure::close_general_clear_work(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            candidate,
        ),
        Action::Layout(Intent::CloseGeneralEpoch { market, epoch }) => {
            terminal_closure::close_general_epoch(
                program_id,
                accounts,
                request.sequence,
                market,
                epoch,
            )
        }
        /* Every other action belongs to another family module; the router never
         * sends one here, and this arm exists so that adding one to the router
         * is a compile error rather than a silent success. */
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Create one deterministic `SUBMITTED` Candidate and CandidateFeed.
///
/// This transition does not mutate the Epoch, does not verify or select the
/// proposal, and creates no SettlementReceipt. Its two new accounts are an
/// authenticated candidate submission, not clearing or settlement authority.
#[inline(never)]
fn submit_direct_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_page: u16,
) -> Outcome<()> {
    require_count(accounts, SUBMIT_DIRECT_PAGE_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_SUBMIT_PAYER])?;
    require(
        accounts[IX_SUBMIT_PAYER].is_writable,
        ClutchError::NotWritable,
    )?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &SUBMIT_DIRECT_PAGE_STATE_ROLES)?;
    require_creatable(&accounts[IX_SUBMIT_CANDIDATE])?;
    require_creatable(&accounts[IX_SUBMIT_FEED])?;
    require_system_program(&accounts[IX_SUBMIT_SYSTEM])?;
    let rent = read_rent(&accounts[IX_SUBMIT_RENT])?;
    let submitted_slot = read_clock_slot(&accounts[IX_SUBMIT_CLOCK])?;
    require(sequence == 0, ClutchError::Replay)?;
    validate_direct_submission_source_addresses(program_id, accounts)?;

    let mut plan = load_direct_submission_plan(
        accounts,
        intent_market,
        intent_epoch,
        intent_page,
        submitted_slot,
    )?;

    let epoch_bytes = plan.candidate.epoch.bytes();
    let candidate_bytes = plan.candidate.candidate.bytes();
    let candidate_derived = seeds::candidate_pda(program_id, &epoch_bytes, &candidate_bytes);
    let feed_derived = seeds::candidate_feed_pda(program_id, &epoch_bytes, &candidate_bytes);
    expect_pda(accounts[IX_SUBMIT_CANDIDATE].key, candidate_derived, None)?;
    expect_pda(accounts[IX_SUBMIT_FEED].key, feed_derived, None)?;
    plan.bind_bumps(candidate_derived.1, feed_derived.1);
    plan.candidate.validate()?;
    plan.feed.validate()?;

    let candidate_bump = [candidate_derived.1];
    let candidate_signer = [
        seeds::SEED_CANDIDATE,
        epoch_bytes.as_ref(),
        candidate_bytes.as_ref(),
        candidate_bump.as_ref(),
    ];
    create_pda_account(
        program_id,
        &accounts[IX_SUBMIT_PAYER],
        &accounts[IX_SUBMIT_CANDIDATE],
        &accounts[IX_SUBMIT_SYSTEM],
        &rent,
        account_len::CANDIDATE,
        &candidate_signer,
    )?;
    let feed_bump = [feed_derived.1];
    let feed_signer = [
        seeds::SEED_CANDIDATE_FEED,
        epoch_bytes.as_ref(),
        candidate_bytes.as_ref(),
        feed_bump.as_ref(),
    ];
    create_pda_account(
        program_id,
        &accounts[IX_SUBMIT_PAYER],
        &accounts[IX_SUBMIT_FEED],
        &accounts[IX_SUBMIT_SYSTEM],
        &rent,
        account_len::CANDIDATE_FEED,
        &feed_signer,
    )?;

    // Creation CPIs are the first mutations. Any later borrow/codec refusal
    // rolls both creations back at the transaction boundary.
    plan.candidate
        .encode(&mut borrow_mut!(accounts[IX_SUBMIT_CANDIDATE])?)?;
    {
        let mut feed = borrow_mut!(accounts[IX_SUBMIT_FEED])?;
        init_candidate_feed(&mut feed, &plan.feed)?;
        write_fill(&mut feed, 0, plan.fill_zero)?;
        write_fill(&mut feed, 1, plan.fill_one)?;
        write_slice_at(&mut feed, 0, &plan.slice)?;
        verify_candidate_feed(&feed)?;
    }
    Ok(())
}

/// Decode and release every large frozen input before account creation begins.
///
/// Keeping the Epoch, grid, two reservations, and output plan alive in the
/// outer CPI frame exceeds SBF's 4 KiB limit. This helper's return value is the
/// only state the creator needs after all semantic checks have passed.
#[inline(never)]
fn load_direct_submission_plan(
    accounts: &[AccountInfo],
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_page: u16,
    submitted_slot: u64,
) -> Outcome<settlement::DirectSubmissionPlan> {
    let epoch = EpochAccount::decode(&accounts[IX_SUBMIT_EPOCH].data.borrow())?;
    require(
        epoch.market == *intent_market && epoch.epoch == *intent_epoch && intent_page == 0,
        ClutchError::MismatchedState,
    )?;
    let grid = PriceGridAccount::decode(&accounts[IX_SUBMIT_GRID].data.borrow())?;
    let reservation_zero =
        ReservationAccount::decode(&accounts[IX_SUBMIT_RESERVATION_ZERO].data.borrow())?;
    let reservation_one =
        ReservationAccount::decode(&accounts[IX_SUBMIT_RESERVATION_ONE].data.borrow())?;
    let page = accounts[IX_SUBMIT_PAGE].data.borrow();
    settlement::prepare_direct_submission(&settlement::DirectSubmissionInput {
        epoch: &epoch,
        grid: &grid,
        page_bytes: &page,
        page_index: intent_page,
        reservation_zero: &reservation_zero,
        reservation_one: &reservation_one,
        submitted_slot,
    })
}

/// Authenticate every frozen input address without retaining decoded values.
#[inline(never)]
fn validate_direct_submission_source_addresses(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> Outcome<()> {
    {
        let value = EpochAccount::decode(&accounts[IX_SUBMIT_EPOCH].data.borrow())?;
        expect_pda(
            accounts[IX_SUBMIT_EPOCH].key,
            seeds::epoch_pda(program_id, &value.market.bytes(), value.epoch_index),
            Some(value.stored_bump),
        )?;
    }
    {
        let value = PriceGridAccount::decode(&accounts[IX_SUBMIT_GRID].data.borrow())?;
        expect_pda(
            accounts[IX_SUBMIT_GRID].key,
            seeds::grid_pda(program_id, &value.realm.bytes(), &value.grid.bytes()),
            Some(value.stored_bump),
        )?;
    }
    {
        let value = stream::OrderPageHeader::decode(&accounts[IX_SUBMIT_PAGE].data.borrow())?;
        expect_pda(
            accounts[IX_SUBMIT_PAGE].key,
            seeds::page_pda(program_id, &value.epoch.bytes(), value.page_index),
            Some(value.stored_bump),
        )?;
    }
    validate_reservation_address(program_id, &accounts[IX_SUBMIT_RESERVATION_ZERO])?;
    validate_reservation_address(program_id, &accounts[IX_SUBMIT_RESERVATION_ONE])
}

/// Consume one entitled receipt of the selected candidate.
///
/// The T2-8 supersession of the narrow coupled seam: receipts are created by
/// the entitlement freeze (tags 58-59) against the complete digest-verified
/// frozen page set, so consumption presents no page and no feed — the receipt
/// is the one-shot latch and both `ENTITLED` reservations carry the exact
/// frozen envelope and the cumulative per-order ledger.  The seven-account
/// shape is now the **universal per-slice consumer**: one slice of any
/// entitled pair, single or portfolio, completing either end independently
/// when its stamped total is reached.  A longer list is the atomic portfolio
/// full-pair shape and dispatches to [`entitlement::settle_portfolio_pair`].
/// Unentitled and general paths keep refusing honestly.
#[inline(never)]
fn settle_page(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_page: u16,
) -> Outcome<()> {
    let potted = accounts.len() == SETTLE_PAGE_POTTED_ACCOUNT_COUNT;
    if accounts.len() != SETTLE_PAGE_ACCOUNT_COUNT && !potted {
        /* The virtual-leg shape, selected by its exact length *and* by the
         * pot's exact data length at its fixed index.  The portfolio pair's
         * variable list can also be eleven accounts long but never carries a
         * `FinalPotAccount` at index five, so the two never overlap; a list
         * that satisfies neither refuses inside whichever it falls into. */
        if accounts.len() == SETTLE_VIRTUAL_ACCOUNT_COUNT
            && accounts[IX_VSETTLE_POT].data_len() == account_len::FINAL_POT
        {
            return settle_virtual_slice(
                program_id,
                accounts,
                sequence,
                intent_market,
                intent_epoch,
                intent_page,
            );
        }
        return entitlement::settle_portfolio_pair(
            program_id,
            accounts,
            sequence,
            intent_market,
            intent_epoch,
            intent_page,
        );
    }
    require_distinct(accounts)?;
    if potted {
        accounts::validate_state_roles(program_id, accounts, &SETTLE_PAGE_POTTED_STATE_ROLES)?;
    } else {
        accounts::validate_state_roles(program_id, accounts, &SETTLE_PAGE_STATE_ROLES)?;
    }
    validate_settle_addresses(program_id, accounts)?;

    let epoch = decode_epoch_boxed(&accounts[IX_SETTLE_EPOCH].data.borrow())?;
    let candidate = decode_candidate_boxed(&accounts[IX_SETTLE_CANDIDATE].data.borrow())?;
    let mut buyer_position =
        decode_position_boxed(&accounts[IX_SETTLE_BUY_POSITION].data.borrow())?;
    let mut seller_position =
        decode_position_boxed(&accounts[IX_SETTLE_SELL_POSITION].data.borrow())?;
    let mut buyer_reservation =
        decode_reservation_boxed(&accounts[IX_SETTLE_BUY_RESERVATION].data.borrow())?;
    let mut seller_reservation =
        decode_reservation_boxed(&accounts[IX_SETTLE_SELL_RESERVATION].data.borrow())?;
    let mut receipt = decode_receipt_boxed(&accounts[IX_SETTLE_RECEIPT].data.borrow())?;
    let mut pot = if potted {
        let value = decode_final_pot_boxed(&accounts[IX_SETTLE_POT].data.borrow())?;
        expect_pda(
            accounts[IX_SETTLE_POT].key,
            seeds::pot_pda(program_id, &value.epoch.bytes()),
            Some(value.stored_bump),
        )?;
        Some(value)
    } else {
        None
    };

    require(
        epoch.market == *intent_market
            && epoch.epoch == *intent_epoch
            && sequence == receipt.sequence,
        ClutchError::MismatchedState,
    )?;
    // The wire's page coordinate names the buy end's page, honestly consumed.
    require(
        buyer_reservation.page_index == intent_page,
        ClutchError::MismatchedState,
    )?;

    let plan = settlement::prepare_entitled_slice_consumption(
        &settlement::EntitledSliceConsumptionInput {
            epoch: &epoch,
            candidate: &candidate,
            buyer_position: &buyer_position,
            seller_position: &seller_position,
            buyer_reservation: &buyer_reservation,
            seller_reservation: &seller_reservation,
            receipt: &receipt,
            pot: pot.as_deref(),
        },
    )?;
    settlement::apply_entitled_slice_consumption(
        &mut buyer_position,
        &mut seller_position,
        &mut buyer_reservation,
        &mut seller_reservation,
        &mut receipt,
        pot.as_deref_mut(),
        plan,
    );
    // All validation remains over staged values.  No account byte has moved.
    buyer_position.validate()?;
    seller_position.validate()?;
    buyer_reservation.validate()?;
    seller_reservation.validate()?;
    receipt.validate()?;
    if let Some(pot) = pot.as_ref() {
        pot.validate()?;
    }

    buyer_position.encode(&mut borrow_mut!(accounts[IX_SETTLE_BUY_POSITION])?)?;
    seller_position.encode(&mut borrow_mut!(accounts[IX_SETTLE_SELL_POSITION])?)?;
    buyer_reservation.encode(&mut borrow_mut!(accounts[IX_SETTLE_BUY_RESERVATION])?)?;
    seller_reservation.encode(&mut borrow_mut!(accounts[IX_SETTLE_SELL_RESERVATION])?)?;
    receipt.encode(&mut borrow_mut!(accounts[IX_SETTLE_RECEIPT])?)?;
    if let Some(pot) = pot.as_ref() {
        pot.encode(&mut borrow_mut!(accounts[IX_SETTLE_POT])?)?;
    }
    Ok(())
}

/// Consume one entitled **virtual** slice, in whichever direction the selected
/// candidate's churn runs.
///
/// A *split* leg's real end is a buyer: it pays into the epoch pot, and the
/// pot mints and delivers.  A *merge* leg's real end is a seller: it delivers
/// into the pot, the delivery that completes `mu` sets burns them, and the pot
/// pays.  Both phases of both directions are read off the receipt's own
/// consumption flags, so which one this is comes from persisted state rather
/// than from the caller.
///
/// The mint and the burn both go through [`split::pooled_set_transition`] —
/// the same kernel step, ledger delta, internal bound, two-term closure and
/// Hoard mirror `Intent::Split` and `Intent::Merge` run — because there is no
/// second route to the market's outstanding supply and there must not be.
#[inline(never)]
fn settle_virtual_slice(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    intent_page: u16,
) -> Outcome<()> {
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &SETTLE_VIRTUAL_STATE_ROLES)?;

    let epoch = decode_epoch_boxed(&accounts[IX_VSETTLE_EPOCH].data.borrow())?;
    let candidate = decode_candidate_boxed(&accounts[IX_VSETTLE_CANDIDATE].data.borrow())?;
    let mut position = decode_position_boxed(&accounts[IX_VSETTLE_POSITION].data.borrow())?;
    let mut reservation =
        decode_reservation_boxed(&accounts[IX_VSETTLE_RESERVATION].data.borrow())?;
    let mut receipt = decode_receipt_boxed(&accounts[IX_VSETTLE_RECEIPT].data.borrow())?;
    let mut pot = decode_final_pot_boxed(&accounts[IX_VSETTLE_POT].data.borrow())?;

    expect_pda(
        accounts[IX_VSETTLE_EPOCH].key,
        seeds::epoch_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;
    expect_pda(
        accounts[IX_VSETTLE_CANDIDATE].key,
        seeds::candidate_pda(
            program_id,
            &candidate.epoch.bytes(),
            &candidate.candidate.bytes(),
        ),
        Some(candidate.stored_bump),
    )?;
    validate_position_address(program_id, &accounts[IX_VSETTLE_POSITION])?;
    expect_pda(
        accounts[IX_VSETTLE_RESERVATION].key,
        seeds::reservation_pda(program_id, &reservation.reservation.bytes()),
        Some(reservation.stored_bump),
    )?;
    expect_pda(
        accounts[IX_VSETTLE_RECEIPT].key,
        seeds::receipt_pda(
            program_id,
            &receipt.epoch.bytes(),
            &receipt.candidate.bytes(),
            receipt.slice_index,
        ),
        Some(receipt.stored_bump),
    )?;
    expect_pda(
        accounts[IX_VSETTLE_POT].key,
        seeds::pot_pda(program_id, &pot.epoch.bytes()),
        Some(pot.stored_bump),
    )?;

    require(
        epoch.market == *intent_market
            && epoch.epoch == *intent_epoch
            && sequence == receipt.sequence
            && reservation.page_index == intent_page,
        ClutchError::MismatchedState,
    )?;

    let plan =
        settlement::prepare_virtual_slice_consumption(&settlement::VirtualSliceConsumptionInput {
            epoch: &epoch,
            candidate: &candidate,
            position: &position,
            reservation: &reservation,
            receipt: &receipt,
            pot: &pot,
        })?;

    /* The mint, before the delivery it funds and before any other write.  It
     * takes the pot's own claim vector as the holder, so the claims it
     * creates are bounded by `require_internal_bound` against the very
     * ledger term it just moved -- there is no window in which the pot holds
     * a claim the supply ledger has not accounted. */
    if plan.mint_sets != 0 {
        split::pooled_set_transition(
            program_id,
            accounts,
            &SETTLE_VIRTUAL_POOLED_ROLES,
            epoch.market,
            &mut pot.pot_internal,
            split::PooledSetChange {
                quantity: plan.mint_sets,
                mint: true,
            },
        )?;
    } else if plan.burn_sets == 0 {
        /* No transition, but the same two closure obligations: a delivery
         * moves claims into or out of the pot's inventory, and the inventory
         * has to be one the supply ledger accounts for.  Both paying phases
         * run it too, so every virtual instruction authenticates the five
         * pooled accounts it presents. */
        split::require_pooled_holder_bound(
            program_id,
            accounts,
            &SETTLE_VIRTUAL_POOLED_ROLES,
            epoch.market,
            &pot.pot_internal,
        )?;
    }

    settlement::apply_virtual_slice_consumption(
        &mut position,
        &mut reservation,
        &mut receipt,
        &mut pot,
        plan,
    );

    /* The burn, *after* the delivery that assembles what it destroys -- the
     * mirror of the mint's placement, and the whole asymmetry between the two
     * churn directions.  A split mints before it hands claims out of the pot;
     * a merge burns once the claims have landed in it, so the primitive is
     * authorized against an inventory that already includes this slice's
     * atoms.  Same primitive, `mint: false`: same kernel step, ledger delta,
     * internal bound, two-term closure and Hoard mirror, with the collateral
     * cap correctly absent because a burn lowers the backing.  A refusal here
     * rolls back the apply above with the rest of the instruction. */
    if plan.burn_sets != 0 {
        split::pooled_set_transition(
            program_id,
            accounts,
            &SETTLE_VIRTUAL_POOLED_ROLES,
            epoch.market,
            &mut pot.pot_internal,
            split::PooledSetChange {
                quantity: plan.burn_sets,
                mint: false,
            },
        )?;
    }
    // All validation remains over staged values.  No account byte has moved.
    position.validate()?;
    reservation.validate()?;
    receipt.validate()?;
    pot.validate()?;

    position.encode(&mut borrow_mut!(accounts[IX_VSETTLE_POSITION])?)?;
    reservation.encode(&mut borrow_mut!(accounts[IX_VSETTLE_RESERVATION])?)?;
    receipt.encode(&mut borrow_mut!(accounts[IX_VSETTLE_RECEIPT])?)?;
    pot.encode(&mut borrow_mut!(accounts[IX_VSETTLE_POT])?)?;
    Ok(())
}

/// Authenticate every settlement address while holding at most one decoded
/// value at a time. This shape is load-bearing on SBF: keeping all the
/// decoded address facts in `settle_page` exceeded the 4 KiB frame.
#[inline(never)]
fn validate_settle_addresses(program_id: &Pubkey, accounts: &[AccountInfo]) -> Outcome<()> {
    {
        let value = EpochAccount::decode(&accounts[IX_SETTLE_EPOCH].data.borrow())?;
        expect_pda(
            accounts[IX_SETTLE_EPOCH].key,
            seeds::epoch_pda(program_id, &value.market.bytes(), value.epoch_index),
            Some(value.stored_bump),
        )?;
    }
    {
        let value = CandidateRecord::decode(&accounts[IX_SETTLE_CANDIDATE].data.borrow())?;
        expect_pda(
            accounts[IX_SETTLE_CANDIDATE].key,
            seeds::candidate_pda(program_id, &value.epoch.bytes(), &value.candidate.bytes()),
            Some(value.stored_bump),
        )?;
    }
    validate_position_address(program_id, &accounts[IX_SETTLE_BUY_POSITION])?;
    validate_position_address(program_id, &accounts[IX_SETTLE_SELL_POSITION])?;
    validate_reservation_address(program_id, &accounts[IX_SETTLE_BUY_RESERVATION])?;
    validate_reservation_address(program_id, &accounts[IX_SETTLE_SELL_RESERVATION])?;
    {
        let value = SettlementReceiptAccount::decode(&accounts[IX_SETTLE_RECEIPT].data.borrow())?;
        expect_pda(
            accounts[IX_SETTLE_RECEIPT].key,
            seeds::receipt_pda(
                program_id,
                &value.epoch.bytes(),
                &value.candidate.bytes(),
                value.slice_index,
            ),
            Some(value.stored_bump),
        )?;
    }
    Ok(())
}

#[inline(never)]
fn validate_position_address(program_id: &Pubkey, account: &AccountInfo) -> Outcome<()> {
    let value = PositionAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::position_pda(program_id, &value.market.bytes(), &value.owner.bytes()),
        Some(value.stored_bump),
    )
}

#[inline(never)]
fn validate_reservation_address(program_id: &Pubkey, account: &AccountInfo) -> Outcome<()> {
    let value = ReservationAccount::decode(&account.data.borrow())?;
    expect_pda(
        account.key,
        seeds::reservation_pda(program_id, &value.reservation.bytes()),
        Some(value.stored_bump),
    )
}

/* Boxed decodes, the `direct_selection_v3::common` discipline: each large
 * decoded value is hoisted into exactly one boxed decode inside an
 * `#[inline(never)]` helper, so sibling locals never co-reside in one
 * handler's bounded SBF frame — only a heap pointer crosses back.  The checks
 * and refusal classes are exactly the direct decodes'. */
#[inline(never)]
fn decode_epoch_boxed(bytes: &[u8]) -> Outcome<Box<EpochAccount>> {
    Ok(Box::new(EpochAccount::decode(bytes)?))
}

#[inline(never)]
fn decode_candidate_boxed(bytes: &[u8]) -> Outcome<Box<CandidateRecord>> {
    Ok(Box::new(CandidateRecord::decode(bytes)?))
}

#[inline(never)]
fn decode_position_boxed(bytes: &[u8]) -> Outcome<Box<PositionAccount>> {
    Ok(Box::new(PositionAccount::decode(bytes)?))
}

#[inline(never)]
fn decode_reservation_boxed(bytes: &[u8]) -> Outcome<Box<ReservationAccount>> {
    Ok(Box::new(ReservationAccount::decode(bytes)?))
}

#[inline(never)]
fn decode_receipt_boxed(bytes: &[u8]) -> Outcome<Box<SettlementReceiptAccount>> {
    Ok(Box::new(SettlementReceiptAccount::decode(bytes)?))
}

#[inline(never)]
fn decode_final_pot_boxed(bytes: &[u8]) -> Outcome<Box<clutch_solana_layout::FinalPotAccount>> {
    Ok(Box::new(clutch_solana_layout::FinalPotAccount::decode(
        bytes,
    )?))
}

#[inline(never)]
fn decode_terms_boxed(bytes: &[u8]) -> Outcome<Box<clutch_solana_layout::TermsAccount>> {
    Ok(Box::new(clutch_solana_layout::TermsAccount::decode(bytes)?))
}

/// Boxed [`load_grid`]: one tick vector in this helper's frame, one heap
/// pointer in the caller's.
#[inline(never)]
fn read_grid_boxed(bytes: &[u8]) -> Outcome<Box<PriceGridAccount>> {
    let mut grid = Box::new(ZERO_GRID);
    load_grid(bytes, &mut grid)?;
    Ok(grid)
}

/// Boxed [`reservation::prepare_placement`]: the staged Position/Reservation
/// pair lives on the heap rather than beside the handler's other locals.
#[inline(never)]
fn prepare_placement_boxed(
    position: &PositionAccount,
    input: &reservation::PlacementInput,
) -> Outcome<Box<(PositionAccount, ReservationAccount)>> {
    Ok(Box::new(reservation::prepare_placement(position, input)?))
}

/// The `PlaceOrder` account plane.
#[inline(never)]
fn place_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: &Hash32,
    intent_epoch: &Hash32,
    max_fee_atoms: u64,
    slot: &OrderSlot,
) -> Outcome<()> {
    if accounts.get(IX_EPOCH).map(|account| account.data_len()) == Some(DIRECT_EPOCH_V4_BYTES) {
        return place_direct_v4_order(
            program_id,
            accounts,
            sequence,
            *intent_market,
            *intent_epoch,
            max_fee_atoms,
            *slot,
        );
    }
    require_count(accounts, PLACE_ORDER_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &PLACE_ORDER_STATE_ROLES)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_EPOCH],
        false,
        &[
            account_len::EPOCH,
            clutch_solana_layout::direct_selection::DIRECT_EPOCH_BYTES,
        ],
    )?;
    require(accounts[IX_ACTOR].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[IX_SYSTEM])?;
    require_creatable(&accounts[IX_RESERVATION])?;
    let rent = read_rent(&accounts[IX_RENT])?;

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
    let position = decode_position_boxed(&accounts[IX_POSITION].data.borrow())?;
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
    expect_pda(
        accounts[IX_POSITION].key,
        seeds::position_pda(
            program_id,
            &position.market.bytes(),
            &position.owner.bytes(),
        ),
        Some(position.stored_bump),
    )?;

    let reservation_id = canonical_reservation_id(
        epoch.market,
        epoch.epoch,
        position.owner,
        position.generation,
        slot.order_id(),
    );
    let reservation_bytes = reservation_id.bytes();
    let (reservation_address, reservation_bump) =
        seeds::reservation_pda(program_id, &reservation_bytes);
    expect_pda(
        accounts[IX_RESERVATION].key,
        (reservation_address, reservation_bump),
        None,
    )?;

    let actor = Hash32::from_bytes(accounts[IX_ACTOR].key.to_bytes());
    let staged = {
        let epoch_data = accounts[IX_EPOCH].data.borrow();
        let grid_data = accounts[IX_GRID].data.borrow();
        let page_data = accounts[IX_PAGE].data.borrow();
        let placement = Placement {
            epoch: &epoch_data,
            grid: &grid_data,
            actor,
            sequence,
            intent_market: *intent_market,
            intent_epoch: *intent_epoch,
            slot: *slot,
        };
        validate_place_order(&page_data, &placement)?;
        prepare_placement_boxed(
            &position,
            &reservation::PlacementInput {
                actor,
                domain: reservation::ReservationDomain::from_epoch(&epoch, page_header.page_index),
                slot: *slot,
                max_fee_atoms,
                reservation_bump,
            },
        )?
    };
    let (next_position, reservation) = &*staged;

    create_pda_account(
        program_id,
        &accounts[IX_ACTOR],
        &accounts[IX_RESERVATION],
        &accounts[IX_SYSTEM],
        &rent,
        RESERVATION_ACCOUNT_BYTES,
        &[
            seeds::SEED_RESERVATION,
            &reservation_bytes,
            &[reservation_bump],
        ],
    )?;

    {
        /* The System CPI receives neither page nor Position, so it cannot
         * invalidate the verdict staged above. `append_slot` still performs
         * the layout owner's complete page validation before writing; rerunning
         * this module's epoch/grid admission after the CPI only repeated two
         * immutable decodes and one full page fold. */
        let mut page_data = borrow_mut!(accounts[IX_PAGE])?;
        stream::append_slot(&mut page_data, *slot)?;
    }
    {
        let mut position_data = borrow_mut!(accounts[IX_POSITION])?;
        next_position.encode(&mut position_data)?;
    }
    let mut reservation_data = borrow_mut!(accounts[IX_RESERVATION])?;
    reservation.encode(&mut reservation_data)?;
    Ok(())
}

struct DirectV4PlaceCommit {
    reservation: DirectReservationV2Account,
    reservation_id: Hash32,
    reservation_bump: u8,
    reservation_funding: clutch_solana_layout::direct_selection_v3::DirectFundingLedgerV3,
}

/// Place one of at most two direct orders under an authenticated V4 Epoch.
///
/// The legacy eight-account ABI above is byte- and behavior-stable. V4 is an
/// exact nine-account branch selected only by the otherwise-unrouted 672-byte
/// Epoch schema, with the exact 96-byte DirectBatchPolicy artifact appended.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn place_direct_v4_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    max_fee_atoms: u64,
    slot: OrderSlot,
) -> Outcome<()> {
    require_count(accounts, DIRECT_V4_PLACE_ORDER_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require(accounts[IX_ACTOR].is_writable, ClutchError::NotWritable)?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &PLACE_ORDER_STATE_ROLES)?;
    accounts::validate_state_roles(program_id, accounts, &DIRECT_V4_POLICY_ROLE)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_EPOCH],
        true,
        &[DIRECT_EPOCH_V4_BYTES],
    )?;
    require_system_program(&accounts[IX_SYSTEM])?;
    require_creatable(&accounts[IX_RESERVATION])?;
    let rent = read_rent(&accounts[IX_RENT])?;
    let commit = prepare_direct_v4_order(
        program_id,
        accounts,
        &rent,
        sequence,
        intent_market,
        intent_epoch,
        max_fee_atoms,
        slot,
    )?;
    let reservation_bytes = commit.reservation_id.bytes();
    create_pda_account_full_principal(
        program_id,
        &accounts[IX_ACTOR],
        &accounts[IX_RESERVATION],
        &accounts[IX_SYSTEM],
        &rent,
        DIRECT_RESERVATION_V2_BYTES,
        commit.reservation_funding,
        0,
        &[
            seeds::SEED_RESERVATION,
            &reservation_bytes,
            &[commit.reservation_bump],
        ],
    )?;
    commit.reservation.encode(
        Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
        &mut borrow_mut!(accounts[IX_RESERVATION])?,
    )?;
    Ok(())
}

struct DirectV4EconomicCommit {
    position: PositionAccount,
    reservation: DirectReservationV2Account,
    reservation_id: Hash32,
    reservation_bump: u8,
    reservation_funding: clutch_solana_layout::direct_selection_v3::DirectFundingLedgerV3,
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare_direct_v4_economics(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    rent: &RentParameters,
    epoch: &DirectEpochV4Account,
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    max_fee_atoms: u64,
    slot: OrderSlot,
) -> Outcome<DirectV4EconomicCommit> {
    let grid = read_grid_boxed(&accounts[IX_GRID].data.borrow())?;
    expect_pda(
        accounts[IX_GRID].key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;
    let page_header = stream::OrderPageHeader::decode(&accounts[IX_PAGE].data.borrow())?;
    expect_pda(
        accounts[IX_PAGE].key,
        seeds::page_pda(
            program_id,
            &page_header.epoch.bytes(),
            page_header.page_index,
        ),
        Some(page_header.stored_bump),
    )?;
    let position = decode_position_boxed(&accounts[IX_POSITION].data.borrow())?;
    expect_pda(
        accounts[IX_POSITION].key,
        seeds::position_pda(
            program_id,
            &position.market.bytes(),
            &position.owner.bytes(),
        ),
        Some(position.stored_bump),
    )?;

    let actor = Hash32::from_bytes(accounts[IX_ACTOR].key.to_bytes());
    let reservation_id = canonical_reservation_id(
        epoch.direct.common.market,
        epoch.direct.common.epoch,
        position.owner,
        position.generation,
        slot.order_id(),
    );
    let reservation_bytes = reservation_id.bytes();
    let (reservation_address, reservation_bump) =
        seeds::reservation_pda(program_id, &reservation_bytes);
    expect_pda(
        accounts[IX_RESERVATION].key,
        (reservation_address, reservation_bump),
        None,
    )?;
    let reservation_funding = direct_creation_funding(
        &accounts[IX_ACTOR],
        &accounts[IX_RESERVATION],
        rent,
        DIRECT_RESERVATION_V2_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let staged = {
        let page_data = accounts[IX_PAGE].data.borrow();
        validate_direct_v4_place(
            &page_data,
            epoch,
            &grid,
            actor,
            sequence,
            intent_market,
            intent_epoch,
            max_fee_atoms,
            slot,
        )?;
        let common = epoch.direct.common;
        prepare_placement_boxed(
            &position,
            &reservation::PlacementInput {
                actor,
                domain: reservation::ReservationDomain {
                    market: common.market,
                    epoch: common.epoch,
                    terms: common.terms,
                    price_grid: common.price_grid,
                    policy: common.policy,
                    epoch_index: common.epoch_index,
                    price_scale: common.price_scale,
                    outcome_count: common.outcome_count,
                    page_index: 0,
                    phase: common.phase,
                },
                slot,
                max_fee_atoms,
                reservation_bump,
            },
        )?
    };
    Ok(DirectV4EconomicCommit {
        position: staged.0,
        reservation: DirectReservationV2Account {
            reservation: staged.1,
            funding: reservation_funding,
        },
        reservation_id,
        reservation_bump,
        reservation_funding,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare_direct_v4_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    rent: &RentParameters,
    sequence: u64,
    intent_market: Hash32,
    intent_epoch: Hash32,
    max_fee_atoms: u64,
    slot: OrderSlot,
) -> Outcome<DirectV4PlaceCommit> {
    /* `decode` already ran the complete hostile-shape validation, including
     * recomputing the epoch-bound policy identity from the persisted release;
     * the placement gate below re-checks only the fields that select this
     * branch. `validate_direct_v4_place` is the single full placement gate. */
    let mut epoch = DirectEpochV4Account::decode(&accounts[IX_EPOCH].data.borrow())?;
    require(
        epoch.neutral_lamport_sink == Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
        ClutchError::MismatchedState,
    )?;
    let exact_policy = DirectBatchPolicyV3::direct(DIRECT_VERIFIER_RELEASE_ID_V3)?;
    let supplied_policy =
        DirectBatchPolicyV3::decode(&accounts[IX_DIRECT_V4_POLICY].data.borrow())?;
    require(
        supplied_policy == exact_policy,
        ClutchError::MismatchedState,
    )?;
    require(
        supplied_policy.digest_for_epoch(epoch.direct.common.epoch)? == epoch.direct_policy_v3_id,
        ClutchError::MismatchedState,
    )?;

    expect_pda(
        accounts[IX_EPOCH].key,
        seeds::epoch_pda(
            program_id,
            &epoch.direct.common.market.bytes(),
            epoch.direct.common.epoch_index,
        ),
        Some(epoch.direct.common.stored_bump),
    )?;
    expect_pda(
        accounts[IX_DIRECT_V4_POLICY].key,
        seeds::direct_batch_policy_v3_pda(
            program_id,
            &epoch.direct.common.epoch.bytes(),
            &epoch.direct_policy_v3_id.bytes(),
        ),
        None,
    )?;
    epoch.epoch_funding = observe_direct_funding(
        epoch.epoch_funding,
        accounts[IX_EPOCH].lamports(),
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    epoch.page_funding = observe_direct_funding(
        epoch.page_funding,
        accounts[IX_PAGE].lamports(),
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let economic = prepare_direct_v4_economics(
        program_id,
        accounts,
        rent,
        &epoch,
        sequence,
        intent_market,
        intent_epoch,
        max_fee_atoms,
        slot,
    )?;
    economic
        .reservation
        .validate(Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()))?;
    economic.position.validate()?;

    /* Every semantic check and every poststate encoding is complete before a
     * byte moves. Existing-state writes precede the System CPI deliberately:
     * a failed full-principal transfer is a real late-failure rollback test,
     * and no account-data borrow survives into that CPI. */
    stage_direct_v4_existing(accounts, &epoch, &economic.position, slot)?;
    Ok(DirectV4PlaceCommit {
        reservation: economic.reservation,
        reservation_id: economic.reservation_id,
        reservation_bump: economic.reservation_bump,
        reservation_funding: economic.reservation_funding,
    })
}

/// Commit only the already-authenticated mutable accounts of a Direct V4
/// placement. All three borrows are acquired before the first byte moves, and
/// each typed poststate was validated by the caller. This keeps the SBF frame
/// bounded without weakening host-level refusal atomicity.
#[inline(never)]
fn stage_direct_v4_existing(
    accounts: &[AccountInfo],
    epoch: &DirectEpochV4Account,
    position: &PositionAccount,
    slot: OrderSlot,
) -> Outcome<()> {
    let mut page_data = borrow_mut!(accounts[IX_PAGE])?;
    let mut position_data = borrow_mut!(accounts[IX_POSITION])?;
    let mut epoch_data = borrow_mut!(accounts[IX_EPOCH])?;
    stream::append_slot(&mut page_data, slot)?;
    position.encode(&mut position_data)?;
    epoch.encode(&mut epoch_data)?;
    Ok(())
}

/// The `CancelOrder` account plane.
///
/// Cancellation reads no grid account: its reservation has already frozen the
/// grid and the Epoch still authenticates that identity.
#[inline(never)]
fn cancel_order(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent: Retirement,
) -> Outcome<()> {
    require_count(accounts, CANCEL_ORDER_ACCOUNT_COUNT)?;
    require_signer(&accounts[IX_ACTOR])?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &CANCEL_ORDER_STATE_ROLES)?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[IX_EPOCH],
        false,
        &[
            account_len::EPOCH,
            clutch_solana_layout::direct_selection::DIRECT_EPOCH_BYTES,
        ],
    )?;

    let epoch = accounts::read_epoch(&accounts[IX_EPOCH].data.borrow())?;
    let page_header = {
        let data = accounts[IX_CANCEL_PAGE].data.borrow();
        stream::OrderPageHeader::decode(&data)?
    };
    let mut position = PositionAccount::decode(&accounts[IX_CANCEL_POSITION].data.borrow())?;
    let mut reservation_account =
        ReservationAccount::decode(&accounts[IX_CANCEL_RESERVATION].data.borrow())?;
    expect_pda(
        accounts[IX_EPOCH].key,
        seeds::epoch_pda(program_id, &epoch.market.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;
    expect_pda(
        accounts[IX_CANCEL_PAGE].key,
        seeds::page_pda(
            program_id,
            &page_header.epoch.bytes(),
            page_header.page_index,
        ),
        Some(page_header.stored_bump),
    )?;
    expect_pda(
        accounts[IX_CANCEL_POSITION].key,
        seeds::position_pda(
            program_id,
            &position.market.bytes(),
            &position.owner.bytes(),
        ),
        Some(position.stored_bump),
    )?;
    expect_pda(
        accounts[IX_CANCEL_RESERVATION].key,
        seeds::reservation_pda(program_id, &reservation_account.reservation.bytes()),
        Some(reservation_account.stored_bump),
    )?;

    let actor = Hash32::from_bytes(accounts[IX_ACTOR].key.to_bytes());
    {
        let epoch_data = accounts[IX_EPOCH].data.borrow();
        let page_data = accounts[IX_CANCEL_PAGE].data.borrow();
        let cancellation = Cancellation {
            epoch: &epoch_data,
            actor,
            sequence,
            intent,
        };
        let (_header, live_slot) = validate_cancel_order(&page_data, &cancellation)?;
        reservation::apply_release(
            &mut position,
            &mut reservation_account,
            &reservation::ReleaseInput {
                actor,
                domain: reservation::ReservationDomain::from_epoch(&epoch, page_header.page_index),
                live_slot,
                release_generation: sequence,
            },
        )?;
    }

    {
        let epoch_data = accounts[IX_EPOCH].data.borrow();
        let cancellation = Cancellation {
            epoch: &epoch_data,
            actor,
            sequence,
            intent,
        };
        let mut page_data = borrow_mut!(accounts[IX_CANCEL_PAGE])?;
        apply_cancel_order(&mut page_data, &cancellation)?;
    }
    {
        let mut position_data = borrow_mut!(accounts[IX_CANCEL_POSITION])?;
        position.encode(&mut position_data)?;
    }
    let mut reservation_data = borrow_mut!(accounts[IX_CANCEL_RESERVATION])?;
    reservation_account.encode(&mut reservation_data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch_policy_identity::batch_policy_digest;
    use clutch_batch_policy_identity::direct_window_v1::DIRECT_POLICY_V1;
    use clutch_solana_layout::direct_selection::DirectEpochV3Account;
    use clutch_solana_layout::direct_selection_v3::{
        DirectFundingLedgerV3, DirectTerminalReceiptV3, DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN,
    };
    use clutch_solana_layout::{
        canonical_epoch_id, canonical_order_id, EpochAccount, OrderPageAccount, OrderRecord,
        PortfolioRecord, TombstoneRecord, EPOCH_PHASE_CLEARED, EPOCH_PHASE_FROZEN,
        MAX_ORDERS_PER_PAGE, MAX_OUTCOMES, ORDER_KIND_SINGLE, ORDER_RECORD_BYTES, ORDER_SLOT_BYTES,
        PORTFOLIO_RECORD_BYTES, RELATION_VERSION,
    };

    /// Length of the layout crate's private page-digest domain tag,
    /// `b"dragons-clutch/order-page/v3"`.  It is not exported, so this is the
    /// one number below that a rename would not turn red.
    const ORDER_PAGE_DOMAIN_BYTES: usize = 28;

    /* These tests drive the transitions directly, on byte slices, exactly as
     * `observe_resolve`'s do and for the same reason: off-chain program-address
     * derivation is not compiled into this crate (see `crate::seeds`), so no
     * host test can reach the account planes.  What is covered here is every
     * check from the page verdict onward, plus the byte-for-byte agreement of
     * both write-backs with the layout crate's own encoder.  The account plane
     * is covered only by the SVM differential, which does not exercise this
     * family yet. */

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

    /// An open epoch on that grid, two outcomes wide, at index 4.
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
            basis_degree: 1,
            phase: EPOCH_PHASE_OPEN,
            stored_bump: 6,
            flags: 0,
        }
    }

    /// One page of that epoch, holding `slots` in its first slots.
    ///
    /// Three fields that were parameters before v4 are now derived, because the
    /// format derives them: the stored range and `prev_page_last_order_id` are
    /// functions of `page_index` and the slot count alone, and
    /// `tombstone_count` is a fold over the slots themselves.  A fixture that
    /// could disagree with them would be a fixture the codec refuses.
    fn page_account(
        epoch: &EpochAccount,
        page_index: u16,
        page_count: u16,
        slots: &[OrderSlot],
    ) -> OrderPageAccount {
        let base = page_index as u64 * MAX_ORDERS_PER_PAGE as u64;
        let mut orders = [OrderSlot::Empty; MAX_ORDERS_PER_PAGE];
        let mut tombstones = 0u8;
        let mut i = 0;
        while i < slots.len() {
            orders[i] = slots[i];
            if slots[i].is_tombstone() {
                tombstones += 1;
            }
            i += 1;
        }
        let rank = |offset: u64| canonical_order_id(base + offset);
        let mut page = OrderPageAccount {
            market: epoch.market,
            epoch: epoch.epoch,
            order_set: Hash32::ZERO,
            page_digest: Hash32::ZERO,
            first_order_id: if slots.is_empty() {
                Hash32::ZERO
            } else {
                rank(1)
            },
            last_order_id: if slots.is_empty() {
                Hash32::ZERO
            } else {
                rank(slots.len() as u64)
            },
            prev_page_last_order_id: if page_index == 0 {
                Hash32::ZERO
            } else {
                canonical_order_id(base)
            },
            page_index,
            page_count,
            set_order_count: 0,
            order_count: slots.len() as u8,
            tombstone_count: tombstones,
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

    /// One single-Egg record at the canonical rank `rank`.
    ///
    /// The rank is not decoration: at v4 an id is `canonical_order_id(rank)`
    /// and the only rank a page admits is the one its own state fixes, so a
    /// fixture that names any other rank is a fixture the placement refuses.
    fn order(owner: u8, rank: u64, limit: u64) -> OrderRecord {
        OrderRecord {
            owner: h(owner),
            order_id: canonical_order_id(rank),
            outcome: 0,
            side: 0,
            quantity: 10,
            limit,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 9,
        }
    }

    /// One portfolio record at the canonical rank `rank`: three Eggs of outcome
    /// 0 and one of outcome 1 per lot, five lots, on the epoch's two-outcome
    /// width.
    fn portfolio(owner: u8, rank: u64) -> PortfolioRecord {
        let mut coefficients = [0u64; MAX_OUTCOMES];
        coefficients[0] = 3;
        coefficients[1] = 1;
        PortfolioRecord {
            owner: h(owner),
            order_id: canonical_order_id(rank),
            side: 0,
            active_len: 2,
            flags: 0,
            coefficients,
            lots: 5,
            limit_collateral_per_lot: 9_000,
            minimum_fill_lots: 2,
            generation: 1,
            expiry_epoch: 9,
        }
    }

    /// The retirement of `order` at `generation`, as the writer would store it.
    fn tombstone(order: &OrderRecord, generation: u64) -> TombstoneRecord {
        TombstoneRecord {
            order_id: order.order_id,
            owner: order.owner,
            retired_generation: order.generation,
            generation,
        }
    }

    /// Where the layout crate's own encoder put this page's `page_digest`.
    ///
    /// Found by value rather than declared as a byte offset: this module owns
    /// no page offset any more, and a 32-byte SHA-256 image is its own
    /// unambiguous anchor inside a page whose other header fields are an
    /// identity, a zero, or a rank.
    fn page_digest_at(page: &[u8]) -> usize {
        let digest = stream::streamed_page_digest(page).expect("the page folds");
        page.windows(32)
            .position(|window| window == digest.0)
            .expect("the encoder stored the digest it folds")
    }

    /// Every account byte string a transition reads but never writes.
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

    fn direct_v4_epoch(grid: &PriceGridAccount) -> DirectEpochV4Account {
        let market = h(1);
        let epoch_id = canonical_epoch_id(market, 4);
        let relation_policy = Hash32::from_bytes(batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0);
        let mut direct = DirectEpochV3Account::open(
            market,
            h(3),
            grid.grid,
            relation_policy,
            4,
            grid.price_scale,
            1,
            100,
            110,
            6,
        )
        .unwrap();
        direct.common.page_count = 1;
        let sink = Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes());
        let policy = DirectBatchPolicyV3::direct(DIRECT_VERIFIER_RELEASE_ID_V3).unwrap();
        let ledger = |payer: u8| DirectFundingLedgerV3 {
            payer: h(payer),
            payer_principal_lamports: 1_000,
            prior_donation_lamports: 0,
        };
        let epoch = DirectEpochV4Account {
            direct,
            selection_deadline_slot: 120,
            settlement_deadline_slot: 140,
            lifecycle_phase: DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN,
            terminal: DirectTerminalReceiptV3::EMPTY,
            neutral_lamport_sink: sink,
            verifier_release_id: DIRECT_VERIFIER_RELEASE_ID_V3,
            direct_policy_v3_id: policy.digest_for_epoch(epoch_id).unwrap(),
            epoch_funding: ledger(0x70),
            page_funding: ledger(0x71),
            reserved: [0; 4],
        };
        epoch
            .validate_for_release(DIRECT_VERIFIER_RELEASE_ID_V3)
            .unwrap();
        epoch
    }

    impl Domain {
        /// A well-formed placement of `slot` at `sequence`, signed by its owner.
        fn placement(&self, sequence: u64, slot: OrderSlot) -> Placement<'_> {
            Placement {
                epoch: &self.epoch_bytes,
                grid: &self.grid_bytes,
                actor: slot.owner(),
                sequence,
                intent_market: self.epoch.market,
                intent_epoch: self.epoch.epoch,
                slot,
            }
        }

        fn place(&self, page: &mut [u8], sequence: u64, slot: OrderSlot) -> Outcome<()> {
            apply_place_order(page, &self.placement(sequence, slot))
        }

        fn place_single(&self, page: &mut [u8], sequence: u64, order: OrderRecord) -> Outcome<()> {
            self.place(page, sequence, OrderSlot::Single(order))
        }

        /// The retirement of rank `rank` by `owner`, at `generation`.
        fn retirement(&self, owner: Hash32, rank: u64, generation: u64) -> Retirement {
            Retirement {
                market: self.epoch.market,
                epoch: self.epoch.epoch,
                owner,
                order_id: canonical_order_id(rank),
                generation,
            }
        }

        /// A well-formed cancellation, signed by the owner it names, whose
        /// declared generation is the envelope's sequence.
        fn cancellation(&self, sequence: u64, intent: Retirement) -> Cancellation<'_> {
            Cancellation {
                epoch: &self.epoch_bytes,
                actor: intent.owner,
                sequence,
                intent,
            }
        }

        fn cancel(&self, page: &mut [u8], owner: Hash32, rank: u64, at: u64) -> Outcome<()> {
            let intent = self.retirement(owner, rank, at);
            apply_cancel_order(page, &self.cancellation(at, intent))
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
        let first = order(0x20, 1, 5_000);
        let second = order(0x21, 2, 2_500);

        let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[]));
        assert_eq!(d.place_single(&mut page, 0, first), Ok(()));
        assert_eq!(
            page,
            encode_page(&page_account(&d.epoch, 0, 1, &[OrderSlot::Single(first)])),
            "the post-state must be byte-identical to the layout crate's own encoding"
        );

        assert_eq!(d.place_single(&mut page, 1, second), Ok(()));
        assert_eq!(
            page,
            encode_page(&page_account(
                &d.epoch,
                0,
                1,
                &[OrderSlot::Single(first), OrderSlot::Single(second)]
            ))
        );

        // And the buffered decoder — the golden reference — reads it back.
        let decoded = OrderPageAccount::decode(&page).expect("post-state decodes");
        assert_eq!(decoded.order_count, 2);
        assert_eq!(decoded.tombstone_count, 0);
        assert_eq!(decoded.first_order_id, first.order_id);
        assert_eq!(decoded.last_order_id, second.order_id);
        assert_eq!(decoded.orders[0], OrderSlot::Single(first));
        assert_eq!(decoded.orders[1], OrderSlot::Single(second));
        assert_eq!(decoded.orders[2], OrderSlot::Empty);
    }

    #[test]
    fn direct_v4_place_accepts_only_the_fixed_two_order_profile() {
        let grid = grid_account();
        let epoch = direct_v4_epoch(&grid);
        let first = OrderSlot::Single(order(0x20, 1, 7_500));
        let page = encode_page(&page_account(&epoch.direct.common, 0, 1, &[]));
        assert_eq!(
            validate_direct_v4_place(
                &page,
                &epoch,
                &grid,
                first.owner(),
                0,
                epoch.direct.common.market,
                epoch.direct.common.epoch,
                0,
                first,
            ),
            Ok(stream::OrderPageHeader::decode(&page).unwrap())
        );

        let hostile_page = page;
        let mut bad_minimum = order(0x20, 1, 7_500);
        bad_minimum.minimum_fill = 1;
        for hostile in [
            OrderSlot::Single(bad_minimum),
            OrderSlot::Portfolio(portfolio(0x20, 1)),
        ] {
            assert!(validate_direct_v4_place(
                &hostile_page,
                &epoch,
                &grid,
                hostile.owner(),
                0,
                epoch.direct.common.market,
                epoch.direct.common.epoch,
                0,
                hostile,
            )
            .is_err());
            assert_eq!(hostile_page, page, "pure refusal cannot mutate the page");
        }
        assert!(validate_direct_v4_place(
            &page,
            &epoch,
            &grid,
            first.owner(),
            0,
            epoch.direct.common.market,
            epoch.direct.common.epoch,
            1,
            first,
        )
        .is_err());

        let second = OrderSlot::Single(OrderRecord {
            side: 1,
            order_id: canonical_order_id(2),
            owner: h(0x21),
            limit: 2_500,
            ..order(0x21, 2, 2_500)
        });
        let full = encode_page(&page_account(&epoch.direct.common, 0, 1, &[first, second]));
        let third = OrderSlot::Single(order(0x22, 3, 5_000));
        assert!(validate_direct_v4_place(
            &full,
            &epoch,
            &grid,
            third.owner(),
            2,
            epoch.direct.common.market,
            epoch.direct.common.epoch,
            0,
            third,
        )
        .is_err());
    }

    #[test]
    fn direct_v4_place_refuses_policy_schedule_page_and_replay_substitution() {
        let grid = grid_account();
        let epoch = direct_v4_epoch(&grid);
        let slot = OrderSlot::Single(order(0x20, 1, 5_000));
        let page = encode_page(&page_account(&epoch.direct.common, 0, 1, &[]));
        let assert_refuses =
            |epoch: &DirectEpochV4Account, page: &[u8], sequence: u64, intent_market: Hash32| {
                assert!(validate_direct_v4_place(
                    page,
                    epoch,
                    &grid,
                    slot.owner(),
                    sequence,
                    intent_market,
                    epoch.direct.common.epoch,
                    0,
                    slot,
                )
                .is_err());
            };

        assert_refuses(&epoch, &page, 1, epoch.direct.common.market);
        assert_refuses(&epoch, &page, 0, h(0xf0));

        let wrong_page = encode_page(&page_account(&epoch.direct.common, 1, 2, &[]));
        assert_refuses(&epoch, &wrong_page, 0, epoch.direct.common.market);

        let mut no_page_funding = epoch;
        no_page_funding.page_funding = DirectFundingLedgerV3::ZERO;
        assert_refuses(&no_page_funding, &page, 0, epoch.direct.common.market);

        let alternate_release = h(0x81);
        let mut wrong_release = epoch;
        wrong_release.verifier_release_id = alternate_release;
        wrong_release.direct_policy_v3_id = DirectBatchPolicyV3::direct(alternate_release)
            .unwrap()
            .digest_for_epoch(epoch.direct.common.epoch)
            .unwrap();
        assert_eq!(wrong_release.validate(), Ok(()));
        assert_refuses(&wrong_release, &page, 0, epoch.direct.common.market);
    }

    #[test]
    fn place_order_writes_a_portfolio_record_the_same_way() {
        /* The v1 wire could not carry a portfolio record at all.  It can now,
         * and the placement path is the same one: the same writer, the same
         * post-state, and a page that interleaves both families in one chain of
         * positional ranks. */
        let d = domain();
        let single = order(0x20, 1, 5_000);
        let basket = portfolio(0x21, 2);

        let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[]));
        assert_eq!(d.place_single(&mut page, 0, single), Ok(()));
        assert_eq!(
            d.place(&mut page, 1, OrderSlot::Portfolio(basket)),
            Ok(()),
            "a portfolio placement is expressible and admitted"
        );
        assert_eq!(
            page,
            encode_page(&page_account(
                &d.epoch,
                0,
                1,
                &[OrderSlot::Single(single), OrderSlot::Portfolio(basket)]
            )),
            "the post-state must be byte-identical to the layout crate's own encoding"
        );

        let decoded = OrderPageAccount::decode(&page).expect("post-state decodes");
        assert_eq!(decoded.orders[1], OrderSlot::Portfolio(basket));
        assert_eq!(decoded.last_order_id, basket.order_id);

        /* The refusals the portfolio family adds are the layout crate's own,
         * applied one record early: the epoch's outcome width bounds
         * `active_len`, and the frozen scale bounds the per-lot products. */
        let mut wide = encode_page(&page_account(&d.epoch, 0, 1, &[]));
        let too_wide = PortfolioRecord {
            active_len: 3,
            ..portfolio(0x21, 1)
        };
        assert_eq!(too_wide.validate(), Ok(()));
        assert_eq!(
            d.place(&mut wide, 0, OrderSlot::Portfolio(too_wide)),
            Err(codec(CodecError::MismatchedBinding)),
            "an active width above the epoch's outcome count is the epoch's refusal"
        );

        /* A per-lot demand and a lot count whose product the ledger holds,
         * but whose product *times the frozen scale* it does not: the record
         * is scale-free valid and could never be classified against any
         * candidate, which is the split `validate_on_scale` exists for. */
        let mut huge = [0u64; MAX_OUTCOMES];
        huge[0] = u64::MAX;
        let unrepresentable = PortfolioRecord {
            coefficients: huge,
            lots: 1 << 60,
            minimum_fill_lots: 0,
            ..portfolio(0x21, 1)
        };
        assert_eq!(unrepresentable.validate(), Ok(()));
        assert_eq!(
            d.place(&mut wide, 0, OrderSlot::Portfolio(unrepresentable)),
            Err(codec(CodecError::ArithmeticOverflow)),
            "a per-lot product the frozen scale cannot represent is the grid's refusal"
        );
        assert_eq!(
            wide,
            encode_page(&page_account(&d.epoch, 0, 1, &[])),
            "a refusal writes nothing"
        );
    }

    #[test]
    fn cancel_order_writes_exactly_what_the_layout_encoder_would() {
        let d = domain();
        let first = order(0x20, 1, 5_000);
        let second = order(0x21, 2, 2_500);
        let placed = encode_page(&page_account(
            &d.epoch,
            0,
            1,
            &[OrderSlot::Single(first), OrderSlot::Single(second)],
        ));

        let mut page = placed;
        assert_eq!(d.cancel(&mut page, first.owner, 1, 7), Ok(()));
        assert_eq!(
            page,
            encode_page(&page_account(
                &d.epoch,
                0,
                1,
                &[
                    OrderSlot::Tombstone(tombstone(&first, 7)),
                    OrderSlot::Single(second)
                ]
            )),
            "the retirement must be byte-identical to the layout crate's own encoding"
        );

        /* Retire in place: the slot and the id stay, so `order_count` and the
         * stored range do not move and the later order is not renumbered. */
        let decoded = OrderPageAccount::decode(&page).expect("post-state decodes");
        assert_eq!(decoded.order_count, 2);
        assert_eq!(decoded.tombstone_count, 1);
        assert_eq!(decoded.first_order_id, first.order_id);
        assert_eq!(decoded.last_order_id, second.order_id);
        assert_eq!(decoded.orders[1], OrderSlot::Single(second));

        // A page with a retirement on it still takes the next placement, at the
        // rank its own state fixes — retirements do not free a rank.
        let third = order(0x20, 3, 7_500);
        assert_eq!(d.place_single(&mut page, 2, third), Ok(()));
        assert_eq!(
            page,
            encode_page(&page_account(
                &d.epoch,
                0,
                1,
                &[
                    OrderSlot::Tombstone(tombstone(&first, 7)),
                    OrderSlot::Single(second),
                    OrderSlot::Single(third)
                ]
            ))
        );

        // A portfolio record retires exactly the same way.
        let basket = portfolio(0x22, 1);
        let mut baskets = encode_page(&page_account(
            &d.epoch,
            0,
            1,
            &[OrderSlot::Portfolio(basket)],
        ));
        assert_eq!(d.cancel(&mut baskets, basket.owner, 1, 4), Ok(()));
        assert_eq!(
            baskets,
            encode_page(&page_account(
                &d.epoch,
                0,
                1,
                &[OrderSlot::Tombstone(TombstoneRecord {
                    order_id: basket.order_id,
                    owner: basket.owner,
                    retired_generation: basket.generation,
                    generation: 4,
                })]
            ))
        );
    }

    #[test]
    fn cancel_order_refuses_a_replayed_retirement() {
        let d = domain();
        let placed = order(0x20, 1, 5_000);
        let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[OrderSlot::Single(placed)]));
        assert_eq!(d.cancel(&mut page, placed.owner, 1, 7), Ok(()));
        let retired = page;

        /* The replay counter is state, in two parts.  The slot kind is the
         * first: a retirement is not a live record, so it cannot be retired
         * again — at the same generation, at a higher one, or by anyone. */
        for at in [7u64, 8, u64::MAX] {
            let mut page = retired;
            assert_eq!(
                d.cancel(&mut page, placed.owner, 1, at),
                Err(codec(CodecError::MismatchedBinding)),
                "a retired slot holds no live record to retire"
            );
            assert_eq!(page, retired, "a refusal writes nothing");
        }

        // Nor does a placement reclaim the rank: the slot is populated, so the
        // page's own state fixes the next rank one higher.
        let mut page = retired;
        assert_eq!(
            d.place_single(&mut page, 1, order(0x20, 1, 5_000)),
            Err(adapter(ClutchError::MismatchedState))
        );
        assert_eq!(page, retired);
    }

    #[test]
    fn cancel_order_refuses_a_stale_generation_a_foreign_owner_and_another_page() {
        let d = domain();
        /* A record whose own generation is 5, so a retirement must carry a
         * strictly higher one. */
        let placed = OrderRecord {
            generation: 5,
            ..order(0x20, 1, 5_000)
        };
        let clean = encode_page(&page_account(&d.epoch, 0, 1, &[OrderSlot::Single(placed)]));

        // The retirement strictly follows the placement it retires.
        for at in [0u64, 3, 5] {
            let mut page = clean;
            assert_eq!(
                d.cancel(&mut page, placed.owner, 1, at),
                Err(codec(CodecError::InvalidEnum)),
                "a retirement at {at} does not follow generation 5"
            );
            assert_eq!(page, clean, "a refusal writes nothing");
        }
        let mut page = clean;
        assert_eq!(d.cancel(&mut page, placed.owner, 1, 6), Ok(()));

        // A signer who is not the record's owner retires nothing, whether the
        // intent names its own owner or the record's.
        let mut page = clean;
        let intent = d.retirement(h(0x21), 1, 9);
        assert_eq!(
            apply_cancel_order(&mut page, &d.cancellation(9, intent)),
            Err(codec(CodecError::MismatchedBinding)),
            "the slot's own owner is the only owner that may retire it"
        );
        let mut impostor = d.cancellation(9, d.retirement(placed.owner, 1, 9));
        impostor.actor = h(0x21);
        assert_eq!(
            apply_cancel_order(&mut page, &impostor),
            Err(adapter(ClutchError::UnauthorizedActor))
        );
        assert_eq!(page, clean);

        /* The id names the page and the slot by arithmetic.  A rank this page
         * does not own, and a rank it owns but has not populated, are both
         * refused as bindings of *this* page rather than as bad ids. */
        for rank in [2u64, 16, 17, 64] {
            let mut page = clean;
            assert_eq!(
                d.cancel(&mut page, placed.owner, rank, 9),
                Err(codec(CodecError::MismatchedBinding)),
                "rank {rank} is not a populated slot of page 0"
            );
        }
        // And page 1's own slots are not reachable from page 0's account.
        let tail_record = order(0x20, 17, 5_000);
        let mut tail = encode_page(&page_account(
            &d.epoch,
            1,
            2,
            &[OrderSlot::Single(tail_record)],
        ));
        assert_eq!(
            d.cancel(&mut tail, tail_record.owner, 1, 9),
            Err(codec(CodecError::MismatchedBinding)),
            "page one does not hold rank one"
        );
        assert_eq!(d.cancel(&mut tail, tail_record.owner, 17, 9), Ok(()));

        // A non-rank identity is refused before any page is consulted.
        let mut page = clean;
        let mut junk = d.cancellation(9, d.retirement(placed.owner, 1, 9));
        junk.intent.order_id = h(0x3e);
        assert_eq!(
            apply_cancel_order(&mut page, &junk),
            Err(codec(CodecError::NonCanonicalIdentity))
        );

        // The declared generation is an assertion about the envelope sequence.
        let mut disagreeing = d.cancellation(9, d.retirement(placed.owner, 1, 8));
        disagreeing.actor = placed.owner;
        assert_eq!(
            apply_cancel_order(&mut page, &disagreeing),
            Err(adapter(ClutchError::Replay))
        );
        assert_eq!(page, clean);
    }

    #[test]
    fn cancel_order_refuses_a_closed_epoch_a_frozen_page_and_a_foreign_epoch() {
        let grid = grid_account();
        let placed = order(0x20, 1, 5_000);

        for phase in [EPOCH_PHASE_FROZEN, EPOCH_PHASE_CLEARED] {
            let base = epoch_account(&grid);
            /* A non-open epoch carries the frozen-set commitments, so the
             * fixture must be a whole frozen epoch rather than a phase byte. */
            let closed = EpochAccount {
                phase,
                order_set: h(0x40),
                first_order_id: canonical_order_id(1),
                last_order_id: canonical_order_id(2),
                page_count: 1,
                order_count: 2,
                ..base
            };
            let d = domain_with(closed, grid);
            let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[OrderSlot::Single(placed)]));
            assert_eq!(
                d.cancel(&mut page, placed.owner, 1, 7),
                Err(adapter(ClutchError::NotActive))
            );
        }

        // The page's own freeze flag is checked as well as the epoch's phase.
        let d = domain();
        let mut frozen = page_account(&d.epoch, 0, 1, &[OrderSlot::Single(placed)]);
        frozen.frozen = 1;
        frozen.set_order_count = 1;
        frozen.order_set = h(0x40);
        frozen.page_digest = frozen.recomputed_page_digest().expect("page digest");
        let mut page = encode_page(&frozen);
        assert_eq!(
            d.cancel(&mut page, placed.owner, 1, 7),
            Err(adapter(ClutchError::NotActive))
        );

        // A page of another epoch, internally valid, is still not this one.
        let other_epoch = EpochAccount {
            epoch: canonical_epoch_id(d.epoch.market, 5),
            epoch_index: 5,
            ..d.epoch
        };
        let mut foreign = encode_page(&page_account(
            &other_epoch,
            0,
            1,
            &[OrderSlot::Single(placed)],
        ));
        assert_eq!(
            d.cancel(&mut foreign, placed.owner, 1, 7),
            Err(adapter(ClutchError::MismatchedState))
        );

        // And an intent that names another market or another epoch.
        let clean = encode_page(&page_account(&d.epoch, 0, 1, &[OrderSlot::Single(placed)]));
        for (market, epoch) in [
            (h(0x7e), d.epoch.epoch),
            (d.epoch.market, canonical_epoch_id(d.epoch.market, 5)),
        ] {
            let mut page = clean;
            let mut cancellation = d.cancellation(7, d.retirement(placed.owner, 1, 7));
            cancellation.intent.market = market;
            cancellation.intent.epoch = epoch;
            assert_eq!(
                apply_cancel_order(&mut page, &cancellation),
                Err(adapter(ClutchError::MismatchedState))
            );
            assert_eq!(page, clean, "a refusal writes nothing");
        }
    }

    #[test]
    fn place_order_fills_a_page_and_then_refuses_a_seventeenth_record() {
        let d = domain();
        let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[]));
        let mut i = 0;
        while i < MAX_ORDERS_PER_PAGE {
            let rank = i as u64 + 1;
            assert_eq!(
                d.place_single(&mut page, i as u64, order(0x20, rank, 5_000)),
                Ok(())
            );
            i += 1;
        }
        let full = page;
        /* A seventeenth record has no slot, which `next_order_id` reports as
         * the count that would result: `InvalidCount`, the page header's own
         * word for it. */
        assert_eq!(
            d.place_single(
                &mut page,
                MAX_ORDERS_PER_PAGE as u64,
                order(0x20, MAX_ORDERS_PER_PAGE as u64 + 1, 5_000)
            ),
            Err(codec(CodecError::InvalidCount))
        );
        assert_eq!(page, full, "a refusal writes nothing");
    }

    #[test]
    fn place_order_refuses_every_page_the_streaming_decoder_refuses() {
        let d = domain();
        let existing = order(0x20, 1, 5_000);
        let clean = encode_page(&page_account(
            &d.epoch,
            0,
            1,
            &[OrderSlot::Single(existing)],
        ));
        let next = order(0x20, 2, 2_500);

        // Each case is a byte-level fixture the frozen codec's own adversarial
        // tests carry, and the refusal must be the codec's verdict verbatim.
        let mut cases: [(&str, [u8; account_len::ORDER_PAGE]); 11] = [("clean", clean); 11];
        cases[0].0 = "wrong tag";
        cases[0].1[0] = clean[0].wrapping_add(1);
        cases[1].0 = "wrong version";
        cases[1].1[1] = clean[1].wrapping_add(1);
        cases[2].0 = "unknown slot kind";
        cases[2].1[stream::ORDER_PAGE_HEADER_BYTES] = 4;
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
        /* The two header corruptions locate their field by the value the
         * encoder stored rather than by an offset this module would then own;
         * `first_order_id` is the field the layout's header table puts
         * immediately after `page_digest`, and the fixture asserts that before
         * damaging it. */
        let at_digest = page_digest_at(&clean);
        let at_first_order_id = at_digest + 32;
        assert_eq!(
            &clean[at_first_order_id..at_first_order_id + 32],
            &existing.order_id.0,
            "first_order_id follows page_digest"
        );
        cases[9].0 = "stale page digest";
        cases[9].1[at_digest] ^= 1;
        cases[10].0 = "stale stored range";
        cases[10].1[at_first_order_id] ^= 1;

        for (name, bytes) in cases.iter() {
            let expected = stream::verify_page(bytes).expect_err(name);
            let mut page = *bytes;
            assert_eq!(
                d.place_single(&mut page, 1, next),
                Err(codec(expected)),
                "{name}: the placement must report the codec's own verdict"
            );
            assert_eq!(&page, bytes, "{name}: a refusal writes nothing");
            // A cancellation reads the same page through the same decoder.
            let mut page = *bytes;
            assert_eq!(
                d.cancel(&mut page, existing.owner, 1, 7),
                Err(codec(expected)),
                "{name}: the cancellation must report it too"
            );
            assert_eq!(&page, bytes, "{name}: a refusal writes nothing");
        }

        // Framing faults that change the buffer's length are the same story.
        let mut short = clean[..account_len::ORDER_PAGE - 1].to_vec();
        assert_eq!(
            d.place_single(&mut short, 1, next),
            Err(codec(CodecError::Truncated))
        );
        let mut long = clean.to_vec();
        long.push(0);
        assert_eq!(
            d.place_single(&mut long, 1, next),
            Err(codec(CodecError::TrailingBytes))
        );
        let mut zeros = [0u8; account_len::ORDER_PAGE];
        assert_eq!(
            d.place_single(&mut zeros, 0, next),
            Err(codec(
                stream::verify_page(&[0; account_len::ORDER_PAGE]).expect_err("all zero")
            ))
        );
    }

    #[test]
    fn place_order_refuses_an_off_grid_limit_and_a_grid_the_epoch_does_not_name() {
        let d = domain();
        let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[]));

        // A limit between two ticks has no tick, so it has no relation price.
        assert_eq!(
            d.place_single(&mut page, 0, order(0x20, 1, 5_001)),
            Err(codec(CodecError::InvalidTick))
        );
        // Neither has one above the scale.
        assert_eq!(
            d.place_single(&mut page, 0, order(0x20, 1, 20_000)),
            Err(codec(CodecError::InvalidTick))
        );

        // A different, internally valid grid is still not this epoch's grid.
        let mut other = grid_account();
        other.ticks[2] = 8_000;
        other.grid = other.recomputed_grid_id().expect("grid identity");
        let mut other_bytes = [0; account_len::PRICE_GRID];
        other.encode(&mut other_bytes).expect("grid encodes");
        let mut placement = d.placement(0, OrderSlot::Single(order(0x20, 1, 5_000)));
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
            &[OrderSlot::Single(order(0x20, 1, 5_000))],
        ));
        assert_eq!(
            stale_domain.place_single(&mut occupied, 1, order(0x20, 2, 2_500)),
            Err(codec(CodecError::InvalidTick))
        );
    }

    #[test]
    fn place_order_refuses_a_closed_epoch_and_a_frozen_page() {
        let grid = grid_account();
        for phase in [EPOCH_PHASE_FROZEN, EPOCH_PHASE_CLEARED] {
            let base = epoch_account(&grid);
            let closed = EpochAccount {
                phase,
                order_set: h(0x40),
                first_order_id: canonical_order_id(1),
                last_order_id: canonical_order_id(2),
                page_count: 1,
                order_count: 2,
                ..base
            };
            let d = domain_with(closed, grid);
            let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[]));
            assert_eq!(
                d.place_single(&mut page, 0, order(0x20, 1, 5_000)),
                Err(adapter(ClutchError::NotActive))
            );
        }

        // The page's own freeze flag is checked as well as the epoch's phase:
        // an open epoch and a frozen page still refuse.
        let d = domain();
        let mut frozen = page_account(&d.epoch, 0, 1, &[OrderSlot::Single(order(0x20, 1, 5_000))]);
        frozen.frozen = 1;
        frozen.set_order_count = 1;
        frozen.order_set = h(0x40);
        frozen.page_digest = frozen.recomputed_page_digest().expect("page digest");
        let mut page = encode_page(&frozen);
        assert_eq!(
            d.place_single(&mut page, 1, order(0x20, 2, 2_500)),
            Err(adapter(ClutchError::NotActive))
        );
    }

    #[test]
    fn place_order_refuses_a_rank_that_is_not_the_one_the_page_fixes() {
        /* At v3 this was an ordering rule — an id had to be strictly above its
         * predecessor — and a caller could burn a page by claiming a huge one.
         * At v4 the page's own state fixes exactly one admissible rank, so the
         * check is an equality and the griefing vector has nothing to grief. */
        let d = domain();
        let existing = order(0x20, 1, 5_000);
        let occupied = encode_page(&page_account(
            &d.epoch,
            0,
            1,
            &[OrderSlot::Single(existing)],
        ));

        // The page holds rank 1, so rank 2 is the only rank it will take.
        for rank in [1u64, 3, 16, 17, 64] {
            let mut page = occupied;
            assert_eq!(
                d.place_single(&mut page, 1, order(0x20, rank, 2_500)),
                Err(adapter(ClutchError::MismatchedState)),
                "rank {rank} is not the rank this page's state fixes"
            );
            assert_eq!(page, occupied, "a refusal writes nothing");
        }
        // Identities that are not ranks at all are the record codec's refusal.
        for (id, expected) in [
            (Hash32::ZERO, CodecError::ZeroIdentity),
            (h(0xff), CodecError::NonCanonicalIdentity),
            (canonical_order_id(65), CodecError::InvalidCount),
        ] {
            let mut page = occupied;
            let record = OrderRecord {
                order_id: id,
                ..order(0x20, 2, 2_500)
            };
            assert_eq!(
                d.place_single(&mut page, 1, record),
                Err(codec(expected)),
                "{id:?} is no rank"
            );
        }
        let mut page = occupied;
        assert_eq!(d.place_single(&mut page, 1, order(0x20, 2, 2_500)), Ok(()));

        /* A later page's ranks are its index's, not its predecessor's fill: an
         * empty page one takes rank 17 whatever page zero holds. */
        let mut tail = encode_page(&page_account(&d.epoch, 1, 2, &[]));
        assert_eq!(
            d.place_single(&mut tail, 0, order(0x20, 1, 5_000)),
            Err(adapter(ClutchError::MismatchedState))
        );
        assert_eq!(d.place_single(&mut tail, 0, order(0x20, 17, 5_000)), Ok(()));
        assert_eq!(
            OrderPageAccount::decode(&tail)
                .expect("post-state decodes")
                .first_order_id,
            canonical_order_id(17)
        );
    }

    #[test]
    fn place_order_refuses_an_unauthenticated_owner_and_a_replayed_sequence() {
        let d = domain();
        let clean = encode_page(&page_account(&d.epoch, 0, 1, &[]));

        // The signer is the owner, or there is no placement.
        let mut page = clean;
        let mut placement = d.placement(0, OrderSlot::Single(order(0x20, 1, 5_000)));
        placement.actor = h(0x21);
        assert_eq!(
            apply_place_order(&mut page, &placement),
            Err(adapter(ClutchError::UnauthorizedActor))
        );
        assert_eq!(page, clean);

        // The page's own populated-slot count is the replay counter, so a
        // sequence that is not the next free slot is refused in both directions.
        for sequence in [1u64, 7, u64::MAX] {
            let mut page = clean;
            assert_eq!(
                d.place_single(&mut page, sequence, order(0x20, 1, 5_000)),
                Err(adapter(ClutchError::Replay))
            );
        }
        let mut page = clean;
        assert_eq!(d.place_single(&mut page, 0, order(0x20, 1, 5_000)), Ok(()));
        // Replaying the accepted request now names a stale slot.
        assert_eq!(
            d.place_single(&mut page, 0, order(0x20, 2, 5_000)),
            Err(adapter(ClutchError::Replay))
        );
    }

    #[test]
    fn place_order_refuses_a_page_or_an_intent_that_names_another_epoch() {
        let d = domain();
        let clean = encode_page(&page_account(&d.epoch, 0, 1, &[]));

        // A page of a different epoch, internally valid, is still not this one.
        let other_epoch = EpochAccount {
            epoch: canonical_epoch_id(d.epoch.market, 5),
            epoch_index: 5,
            ..d.epoch
        };
        let mut foreign = encode_page(&page_account(&other_epoch, 0, 1, &[]));
        assert_eq!(
            d.place_single(&mut foreign, 0, order(0x20, 1, 5_000)),
            Err(adapter(ClutchError::MismatchedState))
        );

        // And an intent that names another market or another epoch.
        for (market, epoch) in [
            (h(0x7e), d.epoch.epoch),
            (d.epoch.market, canonical_epoch_id(d.epoch.market, 5)),
        ] {
            let mut page = clean;
            let mut placement = d.placement(0, OrderSlot::Single(order(0x20, 1, 5_000)));
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
    fn place_order_mirrors_the_record_codec_and_the_epochs_width_and_horizon() {
        let d = domain();
        let clean = encode_page(&page_account(&d.epoch, 0, 1, &[]));
        let base = order(0x20, 1, 5_000);

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
            let mut placement = d.placement(0, OrderSlot::Single(record));
            placement.actor = record.owner;
            assert_eq!(
                apply_place_order(&mut page, &placement),
                Err(codec(expected)),
                "{name}"
            );
            assert_eq!(page, clean, "{name}: a refusal writes nothing");
        }

        /* Two bounds a record can satisfy and the epoch still refuse: an
         * outcome inside `MAX_OUTCOMES` but outside this market's width, and an
         * expiry already behind this epoch's index.  Both are the refusals
         * `stream::epoch_binds_page_set` gives a frozen set, and neither is a
         * question a page can even ask — it bounds an outcome only by
         * `MAX_OUTCOMES`, and it cannot invert its 32-byte epoch identity into
         * an index. */
        for (name, record) in [
            (
                "outcome outside the market's width",
                OrderRecord { outcome: 2, ..base },
            ),
            (
                "expiry already behind the epoch",
                OrderRecord {
                    expiry_epoch: 3,
                    ..base
                },
            ),
        ] {
            assert_eq!(record.validate(), Ok(()), "{name}");
            let mut page = clean;
            assert_eq!(
                d.place_single(&mut page, 0, record),
                Err(codec(CodecError::MismatchedBinding)),
                "{name}"
            );
            assert_eq!(page, clean, "{name}: a refusal writes nothing");
        }
        // The epoch's own index is admitted: the horizon is inclusive.
        let mut page = clean;
        assert_eq!(
            d.place_single(
                &mut page,
                0,
                OrderRecord {
                    expiry_epoch: d.epoch.epoch_index,
                    ..base
                }
            ),
            Ok(())
        );
    }

    #[test]
    fn the_place_order_wire_carries_both_order_families() {
        /* `Intent::PlaceOrder` carries an `OrderSlot` and, at intent v3, an
         * exact fee ceiling.  The portfolio family is expressible: 182 bytes
         * single-Egg and 310 portfolio, the shared fields, fee ceiling, kind
         * byte, and that kind's exact body with none of a page slot's padding.
         * The v1 wire could carry only the narrower family. */
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);
        let single = Intent::PlaceOrder {
            market,
            epoch,
            max_fee_atoms: 0,
            slot: OrderSlot::Single(order(0x20, 1, 5_000)),
        };
        let basket = Intent::PlaceOrder {
            market,
            epoch,
            max_fee_atoms: 0,
            slot: OrderSlot::Portfolio(portfolio(0x20, 1)),
        };
        assert_eq!(
            single.encoded_len(),
            2 + 32 + 32 + 8 + 1 + ORDER_RECORD_BYTES
        );
        assert_eq!(
            basket.encoded_len(),
            2 + 32 + 32 + 8 + 1 + PORTFOLIO_RECORD_BYTES
        );

        for intent in [single, basket] {
            let mut bytes = [0; clutch_solana_layout::MAX_INTENT_BYTES];
            let len = intent.encode(&mut bytes).expect("intent encodes");
            assert_eq!(len, intent.encoded_len());
            assert_eq!(Intent::decode(&bytes[..len]), Ok(intent));
        }

        /* Padding and a retirement are recognized slot kinds that are not
         * placements.  The intent codec refuses both, and so does this module's
         * own transition, which is why the arm is stated rather than assumed. */
        let d = domain();
        let mut page = encode_page(&page_account(&d.epoch, 0, 1, &[]));
        for slot in [
            OrderSlot::Empty,
            OrderSlot::Tombstone(tombstone(&order(0x20, 1, 5_000), 7)),
        ] {
            let mut placement = d.placement(0, slot);
            placement.actor = slot.owner();
            assert_eq!(
                apply_place_order(&mut page, &placement),
                Err(codec(CodecError::InvalidEnum))
            );
            let mut bytes = [0; clutch_solana_layout::MAX_INTENT_BYTES];
            assert_eq!(
                Intent::PlaceOrder {
                    market,
                    epoch,
                    max_fee_atoms: 0,
                    slot
                }
                .encode(&mut bytes),
                Err(CodecError::InvalidEnum)
            );
        }
    }

    #[test]
    fn all_four_intents_reach_their_account_planes() {
        let program_id = Pubkey::new_from_array([9; 32]);
        let market = h(1);
        let epoch = canonical_epoch_id(market, 4);

        let request = Request {
            sequence: 0,
            action: Action::Layout(Intent::SettlePage {
                market,
                epoch,
                page_index: 0,
            }),
        };
        assert_eq!(
            process(&program_id, &[], &request),
            Err(adapter(ClutchError::AccountCount))
        );

        // The other three reach their account planes too.
        for action in [
            Action::Layout(Intent::PlaceOrder {
                market,
                epoch,
                max_fee_atoms: 0,
                slot: OrderSlot::Single(order(0x20, 1, 5_000)),
            }),
            Action::Layout(Intent::CancelOrder {
                market,
                epoch,
                owner: h(0x20),
                order_id: canonical_order_id(1),
                generation: 7,
            }),
            Action::Layout(Intent::SubmitDirectPage {
                market,
                epoch,
                page_index: 0,
            }),
        ] {
            let request = Request {
                sequence: 0,
                action,
            };
            assert_eq!(
                process(&program_id, &[], &request),
                Err(adapter(ClutchError::AccountCount))
            );
        }
    }

    #[test]
    fn the_documented_page_fold_follows_from_the_frozen_widths() {
        /* The module docs state a compute *structure* rather than a
         * measurement, and this test is what keeps that arithmetic honest if
         * the page ever grows again.  The v4 preimage gained `tombstone_count`
         * and eight bytes per slot; the transition lost a whole fold. */
        let preimage = ORDER_PAGE_DOMAIN_BYTES
            + 32
            + 32
            + 2
            + 1
            + 1
            + (MAX_ORDERS_PER_PAGE * ORDER_SLOT_BYTES);
        assert_eq!(preimage, 3_872);
        // SHA-256 pads with one `0x80` byte and an eight-byte length.
        let blocks = (preimage + 1 + 8).div_ceil(64);
        assert_eq!(blocks, 61, "compression blocks per page fold");
        assert_eq!(
            2 * blocks,
            122,
            "compression blocks per accepted placement or retirement"
        );
        assert_eq!(account_len::ORDER_PAGE, 4_012);
        assert_eq!(stream::ORDER_PAGE_HEADER_BYTES, 236);
        assert_eq!(ORDER_SLOT_BYTES, 236);
    }
}
