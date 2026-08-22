//! The host differential for the page→order projection (backlog 6.2).
//!
//! Every case here builds REAL v4 pages through the streaming writers —
//! `stream::{init_page, append_slot, write_tombstone, frozen_set_commitment,
//! seal_page}` — walks them through `projection::project_slot` into the
//! streaming verifier's feed (`ClearWorkV1::begin/push_order/end_pass`), and
//! asserts **verdict identity** with `relation_v1::verify` on an equivalent
//! hand-assembled `BookV1`: the same `SummaryV1` on acceptance, the same
//! `ErrorV1` on refusal.
//!
//! The index vocabularies pinned in `clearing.rs` are exercised here in their
//! general reading: candidate fills are indexed by **zero-based live rank**
//! (`fills[live_rank]` travels with the order the projection numbered
//! `live_rank + 1`), and a `LegRef::Order` index is bounded by the live count,
//! not the populated-slot count.

use clutch_batch::relation_v1::{
    canonical_candidate, verify, AllocationPolicyV1, AonPolicyV1, BookV1, CandidateV1, ErrorV1,
    FeeBaseV1, FrozenPolicyV1, OrderV1, PairingWitnessPolicyV1, PortfolioLotPolicyV1,
    PortfolioOrderV1, RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1, ScorePolicyV1,
    SelfCrossPolicyV1, SingleEggOrderV1, SummaryV1, TransferPhaseV1, PRICE_SCALE,
    RELATION_VERSION_V1,
};
use clutch_batch::relation_v1_stream::{ClearWorkV1, FeedStatusV1, StreamCandidateV1};
use clutch_batch::{DustPolicy, PartialPolicy, Side};
use clutch_solana_layout::clearing::{LegRef, PairingSlice};
use clutch_solana_layout::projection::{project_slot, OwnerInterner};
use clutch_solana_layout::{
    account_len, canonical_epoch_id, canonical_order_id, stream, CodecError, EpochAccount, Hash32,
    OrderRecord, OrderSlot, PortfolioRecord, EPOCH_PHASE_FROZEN, MAX_ORDERS_PER_PAGE,
    MAX_OUTCOMES, MAX_PORTFOLIO_ORDERS, RELATION_VERSION,
};

const SCALE: u64 = PRICE_SCALE;
/// The relation admits an order while `expiry_epoch >= domain.epoch`; every
/// live record in the accepting cases sits exactly on that boundary.
const EPOCH_INDEX: u64 = 7;

fn owner(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

fn market() -> Hash32 {
    Hash32::from_bytes([1; 32])
}

fn epoch_id() -> Hash32 {
    canonical_epoch_id(market(), EPOCH_INDEX)
}

/* ------------------------------------------------------------------------ */
/* Page-set construction, entirely through the streaming writers             */
/* ------------------------------------------------------------------------ */

fn single_slot(owner_byte: u8, outcome: u8, side: u8, quantity: u64, limit: u64) -> OrderSlot {
    single_slot_expiring(owner_byte, outcome, side, quantity, limit, EPOCH_INDEX)
}

fn single_slot_expiring(
    owner_byte: u8,
    outcome: u8,
    side: u8,
    quantity: u64,
    limit: u64,
    expiry_epoch: u64,
) -> OrderSlot {
    OrderSlot::Single(OrderRecord {
        owner: owner(owner_byte),
        order_id: Hash32::ZERO, // stamped with the slot's own rank at append
        outcome,
        side,
        quantity,
        limit,
        minimum_fill: 1,
        flags: 0,
        generation: 1,
        expiry_epoch,
    })
}

fn portfolio_slot(
    owner_byte: u8,
    side: u8,
    coefficients: &[u64],
    lots: u64,
    limit_per_lot: u64,
    flags: u8,
) -> OrderSlot {
    let mut vector = [0u64; MAX_OUTCOMES];
    vector[..coefficients.len()].copy_from_slice(coefficients);
    OrderSlot::Portfolio(PortfolioRecord {
        owner: owner(owner_byte),
        order_id: Hash32::ZERO, // stamped with the slot's own rank at append
        side,
        active_len: coefficients.len() as u8,
        flags,
        coefficients: vector,
        lots,
        limit_collateral_per_lot: limit_per_lot,
        minimum_fill_lots: if flags & 1 != 0 { lots } else { 1 },
        generation: 1,
        expiry_epoch: EPOCH_INDEX,
    })
}

/// Stamp the rank the page's own state will demand; ids are positional.
fn place(slot: &OrderSlot, rank: u64) -> OrderSlot {
    match *slot {
        OrderSlot::Single(mut record) => {
            record.order_id = canonical_order_id(rank);
            OrderSlot::Single(record)
        }
        OrderSlot::Portfolio(mut record) => {
            record.order_id = canonical_order_id(rank);
            OrderSlot::Portfolio(record)
        }
        other => other,
    }
}

/// Build a frozen v4 page set through the streaming writers alone: init every
/// page, append every slot at the id the page fixes, retire the named ranks in
/// place, compute the freeze commitment over the open set, and seal each page.
fn build_pages(slots: &[OrderSlot], retire: &[u64]) -> Vec<Vec<u8>> {
    assert!(!slots.is_empty());
    let page_count = slots.len().div_ceil(MAX_ORDERS_PER_PAGE) as u16;
    let mut pages: Vec<Vec<u8>> = Vec::new();
    for index in 0..page_count {
        let mut page = vec![0u8; account_len::ORDER_PAGE];
        stream::init_page(&mut page, market(), epoch_id(), index, page_count, 5).unwrap();
        pages.push(page);
    }
    for (index, slot) in slots.iter().enumerate() {
        let rank = index as u64 + 1;
        stream::append_slot(&mut pages[index / MAX_ORDERS_PER_PAGE], place(slot, rank)).unwrap();
    }
    for &rank in retire {
        let index = rank as usize - 1;
        stream::write_tombstone(
            &mut pages[index / MAX_ORDERS_PER_PAGE],
            canonical_order_id(rank),
            slots[index].owner(),
            2,
        )
        .unwrap();
    }
    let refs: Vec<&[u8]> = pages.iter().map(|page| page.as_slice()).collect();
    let (order_set, total) = stream::frozen_set_commitment(&refs).unwrap();
    assert_eq!(total as usize, slots.len());
    for page in &mut pages {
        stream::seal_page(page, order_set, total).unwrap();
    }
    pages
}

/// A frozen `EpochAccount` agreeing with a sealed set, for the bind gate.
fn frozen_epoch(pages: &[Vec<u8>], owner_count: u16, outcome_count: u8) -> EpochAccount {
    let head = stream::verify_page(&pages[0]).unwrap();
    let tail = stream::verify_page(&pages[pages.len() - 1]).unwrap();
    EpochAccount {
        epoch: epoch_id(),
        market: market(),
        book: Hash32::from_bytes([3; 32]),
        terms: Hash32::from_bytes([4; 32]),
        price_grid: Hash32::from_bytes([5; 32]),
        policy: Hash32::from_bytes([6; 32]),
        order_set: head.order_set,
        first_order_id: head.first_order_id,
        last_order_id: tail.last_order_id,
        epoch_index: EPOCH_INDEX,
        relation_version: RELATION_VERSION,
        price_scale: SCALE,
        remainder_seed: 0x00C0_FFEE,
        owner_count,
        page_count: head.page_count,
        order_count: head.set_order_count,
        outcome_count,
        basis_degree: 1,
        phase: EPOCH_PHASE_FROZEN,
        stored_bump: 5,
        flags: 0,
    }
}

/* ------------------------------------------------------------------------ */
/* The projection walk and the two verifiers                                 */
/* ------------------------------------------------------------------------ */

/// One canonical page-set walk through the projection: every populated slot of
/// every page in set order, live records numbered 1, 2, ... as retirements are
/// skipped.  Padding at or beyond each page's `order_count` is never offered.
fn project_set(pages: &[Vec<u8>], owners: &mut OwnerInterner) -> Vec<OrderV1> {
    let mut orders = Vec::new();
    for page in pages {
        let header = stream::verify_page(page).unwrap();
        let mut cursor = stream::OrderSlotCursor::new(page).unwrap();
        for _ in 0..header.order_count {
            let slot = cursor.next_slot().unwrap().unwrap();
            let live_rank = orders.len() as u64 + 1;
            if let Some(order) = project_slot(&slot, live_rank, owners).unwrap() {
                orders.push(order);
            }
        }
    }
    orders
}

fn book_of(orders: &[OrderV1]) -> BookV1 {
    let mut book = BookV1::empty();
    book.orders[..orders.len()].copy_from_slice(orders);
    book.len = orders.len() as u8;
    book
}

fn base_policy() -> FrozenPolicyV1 {
    FrozenPolicyV1 {
        allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
        self_cross: SelfCrossPolicyV1::AllowGateAtPairing,
        aon: AonPolicyV1::RefuseAdmission,
        rounding: RoundingBoundaryV1::TerminalOwnerFloor,
        residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
        transfer_phase: TransferPhaseV1::ActiveOrResolved,
        portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
        pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
        dust: DustPolicy::AssignCanonical,
        score: ScorePolicyV1::LexicographicDispersionV1,
        fee_base: FeeBaseV1::None,
    }
}

fn domain(outcomes: u8, owners: u16) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 11,
        book_id: 22,
        epoch: EPOCH_INDEX,
        policy_id: 33,
        order_set_id: 44,
        outcome_count: outcomes,
        owner_count: owners,
        price_scale: SCALE,
        remainder_seed: 0x00C0_FFEE,
        policy: base_policy(),
    }
}

fn prices(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut vector = [0u64; MAX_OUTCOMES];
    vector[..values.len()].copy_from_slice(values);
    vector
}

/// Feed the streaming verifier from the pages themselves: every pass re-walks
/// the raw page bytes through the projection, and each order travels with
/// `fills[live_rank]` — the zero-based live-rank fill vocabulary the candidate
/// feed account pins.
fn drive_projected(
    domain: &RelationDomainV1,
    pages: &[Vec<u8>],
    candidate: &CandidateV1,
) -> Result<SummaryV1, ErrorV1> {
    let header = StreamCandidateV1 {
        order_len: candidate.order_len,
        prices: candidate.prices,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        honored_aon_mask: candidate.honored_aon_mask,
        claimed_score: candidate.claimed_score,
        canonical_candidate_digest: candidate.canonical_candidate_digest,
        declared_slices: None,
    };
    let mut work = Box::new(ClearWorkV1::NEW);
    // One interner for the whole feed: interning is idempotent, so every pass
    // reproduces the same tags from the same walk.
    let mut owners = OwnerInterner::new();
    work.begin(domain, &header, true).unwrap();
    while work.status() != FeedStatusV1::Complete {
        match work.status() {
            FeedStatusV1::NeedOrders { .. } => {
                let mut live = 0usize;
                'walk: for page in pages {
                    let page_header = stream::verify_page(page).unwrap();
                    let mut cursor = stream::OrderSlotCursor::new(page).unwrap();
                    for _ in 0..page_header.order_count {
                        if work.status() == FeedStatusV1::Complete {
                            break 'walk;
                        }
                        let slot = cursor.next_slot().unwrap().unwrap();
                        let live_rank = live as u64 + 1;
                        if let Some(order) = project_slot(&slot, live_rank, &mut owners).unwrap() {
                            work.push_order(&order, candidate.fills[live]).unwrap();
                            live += 1;
                        }
                    }
                }
                if work.status() != FeedStatusV1::Complete {
                    work.end_pass().unwrap();
                }
            }
            FeedStatusV1::NeedSlices => panic!("no case here declares an explicit witness"),
            FeedStatusV1::Complete => {}
        }
    }
    work.verdict().expect("complete feed has a verdict").copied()
}

/// The differential gate: the projection reproduces the hand-assembled book
/// exactly, and the streaming verdict fed from the real pages is identical to
/// the batch verdict on that book.
fn assert_projection_matches(
    domain: &RelationDomainV1,
    pages: &[Vec<u8>],
    hand_book: &BookV1,
    candidate: &CandidateV1,
) -> Result<SummaryV1, ErrorV1> {
    let mut owners = OwnerInterner::new();
    let projected = project_set(pages, &mut owners);
    assert_eq!(
        projected.as_slice(),
        &hand_book.orders[..hand_book.len as usize],
        "projected book diverged from the hand-assembled book"
    );
    let batch = verify(domain, hand_book, candidate, None);
    let streamed = drive_projected(domain, pages, candidate);
    assert_eq!(batch, streamed, "stream verdict diverged from batch");
    batch
}

/* ------------------------------------------------------------------------ */
/* Hand-assembled relation orders                                            */
/* ------------------------------------------------------------------------ */

#[allow(clippy::too_many_arguments)]
fn hand_single(
    id: u64,
    owner_tag: u16,
    outcome: u8,
    side: Side,
    quantity: u64,
    limit: u64,
    expiry_epoch: u64,
) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner: owner_tag,
        outcome,
        side,
        quantity,
        limit_price: limit,
        minimum_fill: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch,
    })
}

fn hand_portfolio(
    id: u64,
    owner_tag: u16,
    side: Side,
    coefficients: &[u64],
    lots: u64,
    limit_per_lot: u64,
) -> OrderV1 {
    let mut vector = [0u64; MAX_OUTCOMES];
    vector[..coefficients.len()].copy_from_slice(coefficients);
    OrderV1::Portfolio(PortfolioOrderV1 {
        canonical_order_id: id,
        owner: owner_tag,
        side,
        coefficients: vector,
        active_len: coefficients.len() as u8,
        lots,
        limit_collateral_per_lot: limit_per_lot,
        minimum_fill_lots: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: EPOCH_INDEX,
    })
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */
/* ------------------------------------------------------------------------ */

/// One page, two crossing singles: the smallest accepting differential, plus
/// refusal identity on three candidate mutations.
#[test]
fn single_page_crossing_book_matches_batch_verdict() {
    let slots = [
        single_slot(20, 0, 0, 4, SCALE),
        single_slot(21, 0, 1, 4, 0),
    ];
    let pages = build_pages(&slots, &[]);
    let book = book_of(&[
        hand_single(1, 0, 0, Side::Buy, 4, SCALE, EPOCH_INDEX),
        hand_single(2, 1, 0, Side::Sell, 4, 0, EPOCH_INDEX),
    ]);
    let domain = domain(2, 2);
    let price_vector = prices(&[6_000, 4_000]);
    let candidate = canonical_candidate(&domain, &book, &price_vector, 0, 0).unwrap();
    let verdict = assert_projection_matches(&domain, &pages, &book, &candidate);
    let summary = verdict.expect("crossing book clears");
    assert_eq!(summary.buy_flow[0], 4);

    // Refusal identity: a bumped fill, phantom churn, and a stale digest all
    // refuse identically through both verifiers.
    let mut bumped = candidate;
    bumped.fills[0] += 1;
    let mut churned = candidate;
    churned.virtual_split += 1;
    let mut stale = candidate;
    stale.canonical_candidate_digest ^= 1;
    for mutated in [bumped, churned, stale] {
        let batch = verify(&domain, &book, &mutated, None);
        assert!(batch.is_err());
        assert_eq!(batch, drive_projected(&domain, &pages, &mutated));
    }
}

/// The headline: a tombstone-bearing set whose live orders equal a
/// tombstone-free set projects to the identical book and produces the
/// **identical verdict** — same `SummaryV1`, byte for byte.
///
/// The retirements are chosen adversarially: rank 2's owner (26) has no live
/// record at all, so a stored-slot interning would mint it tag 1 and shift
/// every later owner; the live-walk interning never sees it.  Every live
/// record after rank 1 sits at a live rank different from its stored rank.
#[test]
fn tombstoned_set_matches_its_tombstone_free_equivalent() {
    // Stored ranks:  1     2*    3     4     5*    6     7*    8     9
    // Live ranks:    1     -     2     3     -     4     -     5     6
    let slots = [
        single_slot(20, 0, 0, 4, SCALE),
        single_slot(26, 0, 1, 4, 0),
        single_slot(21, 0, 1, 4, 0),
        single_slot(22, 1, 0, 3, SCALE),
        single_slot(20, 1, 1, 9, 0),
        single_slot(21, 1, 1, 3, 0),
        single_slot(22, 0, 0, 7, SCALE),
        portfolio_slot(23, 0, &[1, 1], 2, SCALE, 0),
        portfolio_slot(24, 1, &[1, 1], 2, 0, 0),
    ];
    let retired = [2u64, 5, 7];
    let live = [slots[0], slots[2], slots[3], slots[5], slots[7], slots[8]];
    let tombstoned = build_pages(&slots, &retired);
    let tombstone_free = build_pages(&live, &[]);

    // Same live orders, same hand book: live ranks 1..=6, owner tags by first
    // appearance among live records (20 -> 0, 21 -> 1, 22 -> 2, 23 -> 3,
    // 24 -> 4; the retired 26 takes no tag).  Both outcomes' buy and sell
    // flows balance exactly, so the whole book clears at whole lots.
    let book = book_of(&[
        hand_single(1, 0, 0, Side::Buy, 4, SCALE, EPOCH_INDEX),
        hand_single(2, 1, 0, Side::Sell, 4, 0, EPOCH_INDEX),
        hand_single(3, 2, 1, Side::Buy, 3, SCALE, EPOCH_INDEX),
        hand_single(4, 1, 1, Side::Sell, 3, 0, EPOCH_INDEX),
        hand_portfolio(5, 3, Side::Buy, &[1, 1], 2, SCALE),
        hand_portfolio(6, 4, Side::Sell, &[1, 1], 2, 0),
    ]);
    let domain = domain(2, 5);
    let price_vector = prices(&[6_000, 4_000]);
    let candidate = canonical_candidate(&domain, &book, &price_vector, 0, 0).unwrap();

    let with_tombstones = assert_projection_matches(&domain, &tombstoned, &book, &candidate);
    let without = assert_projection_matches(&domain, &tombstone_free, &book, &candidate);
    assert!(with_tombstones.is_ok(), "the crossing live book clears");
    assert_eq!(
        with_tombstones, without,
        "a retirement must be invisible to the verdict"
    );

    // The header arithmetic agrees with the walk: live_count() sums to the
    // projected length.
    let live_total: usize = tombstoned
        .iter()
        .map(|page| stream::verify_page(page).unwrap().live_count() as usize)
        .sum();
    assert_eq!(live_total, book.len as usize);
}

/// Four pages, sixteen slots each — the full 64-slot book, carrying the
/// relation's whole portfolio budget (`MAX_PORTFOLIO_ORDERS` = 8) — with
/// retirements chosen to straddle a page boundary, so live ranks diverge from
/// stored ranks across pages.
#[test]
fn four_page_full_width_set_with_cross_page_tombstones_matches() {
    let mut slots = Vec::new();
    for index in 0..MAX_PORTFOLIO_ORDERS as u64 {
        let side = (index % 2) as u8;
        let limit = if side == 0 { SCALE } else { 0 };
        slots.push(portfolio_slot(index as u8 + 1, side, &[2, 1], 3, limit, 0));
    }
    for index in 0..56u64 {
        let owner_byte = (index % 8) as u8 + 1;
        let outcome = (index % 2) as u8;
        let side = if index % 4 < 2 { 0 } else { 1 };
        let limit = if side == 0 { SCALE } else { 0 };
        slots.push(single_slot(owner_byte, outcome, side, 4, limit));
    }
    assert_eq!(slots.len(), 64);
    // Retire two buy/sell pairs on outcome 1 — equal quantities, so the live
    // book still balances exactly — straddling the page 0/1 boundary (ranks
    // 16 and 18) and deep in page 2 (ranks 40 and 42).
    let retired = [16u64, 18, 40, 42];
    for &rank in &retired {
        assert!(matches!(slots[rank as usize - 1], OrderSlot::Single(o) if o.outcome == 1));
    }
    let pages = build_pages(&slots, &retired);
    assert_eq!(pages.len(), 4);

    // The equivalent hand-assembled book: the same walk, written against the
    // slot specs rather than the pages, skipping the retired ranks.
    let mut hand = Vec::new();
    for (index, slot) in slots.iter().enumerate() {
        let rank = index as u64 + 1;
        if retired.contains(&rank) {
            continue;
        }
        let id = hand.len() as u64 + 1;
        // First-appearance tags: owners 1..=8 all appear among the first eight
        // live records, in owner order, so the tag is the owner byte less one.
        let tag = (slot.owner().0[0] - 1) as u16;
        hand.push(match *slot {
            OrderSlot::Single(o) => hand_single(
                id,
                tag,
                o.outcome,
                if o.side == 0 { Side::Buy } else { Side::Sell },
                o.quantity,
                o.limit,
                o.expiry_epoch,
            ),
            OrderSlot::Portfolio(p) => hand_portfolio(
                id,
                tag,
                if p.side == 0 { Side::Buy } else { Side::Sell },
                &p.coefficients[..p.active_len as usize],
                p.lots,
                p.limit_collateral_per_lot,
            ),
            _ => unreachable!("every spec slot is a live record"),
        });
    }
    assert_eq!(hand.len(), 60);
    let book = book_of(&hand);
    let domain = domain(2, 8);
    let price_vector = prices(&[6_000, 4_000]);
    let candidate = canonical_candidate(&domain, &book, &price_vector, 0, 0).unwrap();
    let verdict = assert_projection_matches(&domain, &pages, &book, &candidate);
    assert!(verdict.is_ok(), "the full-width crossing book clears");
}

/// An all-or-none record projects to `PartialPolicy::AllOrNone` and both
/// verifiers refuse it identically under the frozen `RefuseAdmission` policy.
#[test]
fn aon_flag_projects_and_refuses_identically() {
    let slots = [
        single_slot(20, 0, 0, 4, SCALE),
        portfolio_slot(21, 1, &[1, 1], 2, 0, 1),
    ];
    let pages = build_pages(&slots, &[]);
    let mut aon = hand_portfolio(2, 1, Side::Sell, &[1, 1], 2, 0);
    if let OrderV1::Portfolio(ref mut record) = aon {
        record.partial_policy = PartialPolicy::AllOrNone;
        record.minimum_fill_lots = 2;
    }
    let book = book_of(&[hand_single(1, 0, 0, Side::Buy, 4, SCALE, EPOCH_INDEX), aon]);
    let domain = domain(2, 2);
    let candidate = CandidateV1::empty(book.len, prices(&[6_000, 4_000]));
    let verdict = assert_projection_matches(&domain, &pages, &book, &candidate);
    assert_eq!(verdict, Err(ErrorV1::AonNotAdmitted));
}

/// Per-order expiry: a live record already past the epoch index refuses at
/// bind (`stream::epoch_binds_page_set`), a *retired* expired record does not,
/// and the relation's own admission refusal is verdict-identical through the
/// projection.
#[test]
fn expired_live_record_refuses_at_bind_and_identically_in_both_verifiers() {
    let expired = [
        single_slot(20, 0, 0, 4, SCALE),
        single_slot_expiring(21, 0, 1, 4, 0, EPOCH_INDEX - 1),
    ];
    let pages = build_pages(&expired, &[]);
    let refs: Vec<&[u8]> = pages.iter().map(|page| page.as_slice()).collect();
    let epoch = frozen_epoch(&pages, 2, 2);
    assert_eq!(
        stream::epoch_binds_page_set(&epoch, &refs),
        Err(CodecError::MismatchedBinding),
        "a live record past its expiry must refuse at bind"
    );

    // Retiring the expired record clears the bind refusal: a tombstone is
    // never fed, so no horizon applies to it.
    let retired_pages = build_pages(&expired, &[2]);
    let retired_refs: Vec<&[u8]> = retired_pages.iter().map(|page| page.as_slice()).collect();
    let epoch = frozen_epoch(&retired_pages, 1, 2);
    assert_eq!(stream::epoch_binds_page_set(&epoch, &retired_refs), Ok(()));

    // The relation refuses the same record at admission, identically through
    // the projection (the bind gate above is what keeps such a set from ever
    // reaching this point on-chain).
    let book = book_of(&[
        hand_single(1, 0, 0, Side::Buy, 4, SCALE, EPOCH_INDEX),
        hand_single(2, 1, 0, Side::Sell, 4, 0, EPOCH_INDEX - 1),
    ]);
    let domain = domain(2, 2);
    let candidate = CandidateV1::empty(book.len, prices(&[6_000, 4_000]));
    let verdict = assert_projection_matches(&domain, &pages, &book, &candidate);
    assert_eq!(verdict, Err(ErrorV1::ExpiredOrder));
}

/// The owner-interner bijection over a real walk: tags are exactly
/// `0..count`, the same owner always answers the same tag, distinct owners
/// answer distinct tags, and the interned table lists first live appearances
/// in walk order — the retired rank-2 record mints no tag for owner 22.
#[test]
fn owner_interner_bijection_holds_over_the_walk() {
    let slots = [
        single_slot(20, 0, 0, 4, SCALE),
        single_slot(22, 0, 1, 5, 0),
        single_slot(21, 0, 1, 3, 0),
        single_slot(22, 0, 0, 2, SCALE),
        single_slot(20, 0, 1, 1, 0),
        portfolio_slot(23, 0, &[1, 1], 2, 2, 0),
    ];
    let pages = build_pages(&slots, &[2]);
    let mut owners = OwnerInterner::new();
    let projected = project_set(&pages, &mut owners);
    assert_eq!(projected.len(), 5);

    // First live appearances, in walk order; the tombstoned slot 2 minted no
    // tag, so owner 22's tag comes from its rank-4 record.
    assert_eq!(
        owners.owners(),
        &[owner(20), owner(21), owner(22), owner(23)]
    );
    assert_eq!(owners.count(), 4);

    // Tags on the projected orders are the interner's images of the slot
    // owners: 0..count with no gap, same owner same tag, distinct distinct.
    let tags: Vec<u16> = projected.iter().map(|order| order.owner()).collect();
    assert_eq!(tags, vec![0, 1, 2, 0, 3]);
    let mut seen = [false; 4];
    for &tag in &tags {
        seen[tag as usize] = true;
    }
    assert!(seen.iter().all(|&hit| hit), "tags cover exactly 0..count");
}

/// The index-vocabulary pin: order indices in the clearing plane are
/// zero-based **live ranks**, bounded by the live count — never global slot
/// indices, which only coincide on a set with no tombstone.
#[test]
fn order_indices_are_live_ranks_not_slot_indices() {
    // Eight populated slots, three retired: five live orders.  In the stored
    // vocabulary index 5 names a populated slot; in the live vocabulary the
    // book ends at 4.
    let live_len = 5u8;
    let ok = PairingSlice {
        buy_ref: LegRef::Order(live_len - 1),
        sell_ref: LegRef::Order(1),
        outcome: 0,
        quantity: 1,
    };
    assert_eq!(ok.validate(live_len, 2), Ok(()));
    let stored_vocabulary = PairingSlice {
        buy_ref: LegRef::Order(live_len),
        sell_ref: LegRef::Order(1),
        outcome: 0,
        quantity: 1,
    };
    assert_eq!(
        stored_vocabulary.validate(live_len, 2),
        Err(CodecError::InvalidCount),
        "a live-rank vocabulary refuses an index only the slot vocabulary admits"
    );
}
