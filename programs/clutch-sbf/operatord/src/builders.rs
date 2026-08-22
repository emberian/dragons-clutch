//! Every Friday-session transaction, built through the one true builder.
//!
//! Each function below decides only *which accounts in which roles* an intent
//! needs; the bytes are assembled by `clutch_sbf_harness::general_transaction`,
//! which is the same function the sealed lane's plan emitter calls and the
//! same one `operatord replay` byte-diffs.  The browser posts an intent, the
//! daemon picks the role vector, and the harness serializes — there is no
//! third serializer anywhere in this path.
//!
//! The role orders are read off the program's own account contracts, the same
//! way `svm-tests/tests/disagreement_exhibit.rs` reads them, and the fixed
//! prefixes are asserted against the program's exported account counts so a
//! contract that moves fails here loudly rather than on chain quietly.
//!
//! `heap` marks the transactions that need the 256 KiB frame: every one that
//! touches the boxed ~48.7 KiB `ClearWorkV1` body — its creation, its growth,
//! both advance passes, the slice pass, its completion, and the entitlement
//! freeze that reads it. Without the frame the program does not refuse, it
//! *fails to complete*, which is a much less legible way to be wrong; the
//! same four steps carry it in the sealed lane's plan.

use crate::friday::{Friday, OUTCOMES};
use clutch_sbf::instructions::orders_batch::{
    self,
    clear_walk::{
        ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT, ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT,
        COMPLETE_CLEAR_WORK_ACCOUNT_COUNT,
    },
    entitlement::{
        ENTITLE_SLICE_FIXED_ACCOUNT_COUNT, FREEZE_ENTITLEMENT_ACCOUNT_COUNT,
        SETTLE_PAIR_FIXED_ACCOUNT_COUNT,
    },
    selection::{SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT, SUBMIT_CANDIDATE_ACCOUNT_COUNT},
};
use clutch_sbf_harness::{general_transaction, layout_request, GeneralTx};
use clutch_solana_layout::{CandidateFeedChunk, Hash32, Intent, OrderSlot, MAX_OUTCOMES};

/// The Clock sysvar, as the program's own artifact module names it.
fn clock() -> [u8; 32] {
    clutch_sbf::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes()
}

/// Found the market and its seven state PDAs through signed System CPIs.
pub fn create_market(f: &Friday, creator: [u8; 32]) -> Vec<u8> {
    let shared = &f.shared;
    let mut writable = vec![
        f.market.bytes,
        f.hoard.bytes,
        position_of(f, creator),
        f.kernel.bytes,
        replay_of(f, creator),
        f.supply.bytes,
        f.resolution.bytes,
        f.hoard_token.bytes,
    ];
    writable.extend(f.outcome_mints.iter().map(|mint| mint.bytes));
    let readonly = [
        shared.realm.bytes,
        shared.profile.bytes,
        f.terms.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        shared.system_program,
        shared.rent_sysvar,
        f.hoard_authority.bytes,
    ];
    let mut keys = vec![
        creator,
        shared.realm.bytes,
        shared.profile.bytes,
        f.terms.bytes,
        f.market.bytes,
        f.hoard.bytes,
        position_of(f, creator),
        f.kernel.bytes,
        replay_of(f, creator),
        f.supply.bytes,
        f.resolution.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        shared.system_program,
        shared.rent_sysvar,
        f.hoard_authority.bytes,
        f.hoard_token.bytes,
    ];
    keys.extend(f.outcome_mints.iter().map(|mint| mint.bytes));
    assert_eq!(
        keys.len(),
        clutch_sbf::instructions::market_init::account_count(OUTCOMES),
        "the founding plane must be exactly the plane the program requires"
    );
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &signer_slot(shared, creator),
            readonly_signers: &[],
            writable: &writable,
            readonly: &readonly,
            keys: &keys,
            data: layout_request(
                0,
                Intent::CreateMarket {
                    realm: shared.realm_hash,
                    profile: shared.profile_hash,
                    market_nonce: crate::friday::NONCE_FRIDAY,
                    outcome_count: OUTCOMES,
                    terms: f.terms_value.terms,
                    feed: shared.feed,
                },
            ),
            heap: false,
        },
    )
}

/// A backed deposit: collateral leaves the owner's ordinary token account and
/// enters pooled custody, and the owner's Position and Replay are created if
/// this is its first.
pub fn endow(f: &Friday, owner: [u8; 32], sequence: u64, amount: u64) -> Vec<u8> {
    let shared = &f.shared;
    let (position, replay, token) = owner_plane(f, owner);
    let writable = [position, replay, token, f.hoard_token.bytes];
    let readonly = [
        f.market.bytes,
        f.hoard.bytes,
        shared.profile.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        shared.system_program,
        shared.rent_sysvar,
        f.terms.bytes,
        shared.source_spec.bytes,
    ];
    let keys = [
        owner,
        f.market.bytes,
        f.hoard.bytes,
        position,
        replay,
        shared.profile.bytes,
        shared.policy_account.bytes,
        shared.token_program,
        shared.collateral_mint.bytes,
        token,
        f.hoard_token.bytes,
        shared.system_program,
        shared.rent_sysvar,
        f.terms.bytes,
        shared.source_spec.bytes,
    ];
    assert_eq!(
        keys.len(),
        clutch_sbf::instructions::genesis::ENDOW_ACCOUNT_COUNT,
        "the endowment plane must be exactly the plane the program requires"
    );
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &signer_slot(shared, owner),
            readonly_signers: &[],
            writable: &writable,
            readonly: &readonly,
            keys: &keys,
            data: layout_request(
                sequence,
                Intent::Endow {
                    market: f.market_id,
                    owner: Hash32::from_bytes(owner),
                    amount,
                },
            ),
            heap: false,
        },
    )
}

/// Lock a complete set: cash becomes one Egg on every active outcome.
pub fn split(f: &Friday, owner: [u8; 32], sequence: u64, quantity: u64) -> Vec<u8> {
    let shared = &f.shared;
    let (position, replay, token) = owner_plane(f, owner);
    let writable = [
        f.market.bytes,
        f.hoard.bytes,
        position,
        f.kernel.bytes,
        replay,
        f.supply.bytes,
        token,
        f.hoard_token.bytes,
    ];
    let mut readonly = vec![
        shared.realm.bytes,
        shared.profile.bytes,
        shared.token_program,
        shared.policy_account.bytes,
        shared.collateral_mint.bytes,
        f.hoard_authority.bytes,
    ];
    readonly.extend(f.outcome_mints.iter().map(|mint| mint.bytes));
    let mut keys = vec![
        owner,
        shared.realm.bytes,
        shared.profile.bytes,
        f.market.bytes,
        f.hoard.bytes,
        position,
        f.kernel.bytes,
        replay,
        f.supply.bytes,
        shared.token_program,
        shared.policy_account.bytes,
        shared.collateral_mint.bytes,
        token,
        f.hoard_authority.bytes,
        f.hoard_token.bytes,
    ];
    keys.extend(f.outcome_mints.iter().map(|mint| mint.bytes));
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &readonly_signer_slot(shared, owner),
            writable: &writable,
            readonly: &readonly,
            keys: &keys,
            data: layout_request(
                sequence,
                Intent::Split {
                    market: f.market_id,
                    owner: Hash32::from_bytes(owner),
                    quantity,
                },
            ),
            heap: false,
        },
    )
}

/// Open the trading epoch and its candidate window.
pub fn init_epoch(f: &Friday, freeze_deadline_slot: u64) -> Vec<u8> {
    let shared = &f.shared;
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[f.epoch.bytes, f.window.bytes],
            readonly: &[
                f.market.bytes,
                f.terms.bytes,
                f.grid.bytes,
                f.batch_policy.bytes,
                shared.system_program,
                shared.rent_sysvar,
                clock(),
            ],
            keys: &[
                shared.payer.bytes,
                f.market.bytes,
                f.terms.bytes,
                f.grid.bytes,
                f.batch_policy.bytes,
                f.epoch.bytes,
                f.window.bytes,
                shared.system_program,
                shared.rent_sysvar,
                clock(),
            ],
            data: layout_request(
                0,
                Intent::InitEpoch {
                    market: f.market_id,
                    epoch_index: crate::friday::EPOCH_INDEX,
                    policy: f.policy_digest,
                    freeze_deadline_slot,
                },
            ),
            heap: false,
        },
    )
}

/// Create the single order page this session's book lives on.
pub fn init_page(f: &Friday) -> Vec<u8> {
    let shared = &f.shared;
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[f.page.bytes],
            readonly: &[
                f.market.bytes,
                f.epoch.bytes,
                shared.system_program,
                shared.rent_sysvar,
            ],
            keys: &[
                shared.payer.bytes,
                f.page.bytes,
                f.market.bytes,
                f.epoch.bytes,
                shared.system_program,
                shared.rent_sysvar,
            ],
            data: layout_request(
                0,
                Intent::InitOrderPage {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    page_index: 0,
                    page_count: 1,
                },
            ),
            heap: false,
        },
    )
}

/// Rest one order and fund its reservation.
pub fn place_order(f: &Friday, owner: [u8; 32], sequence: u64, slot: OrderSlot) -> Vec<u8> {
    let shared = &f.shared;
    let (position, _, _) = owner_plane(f, owner);
    let reservation = f.reservation(slot.owner(), slot.order_id()).bytes;
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &signer_slot(shared, owner),
            readonly_signers: &[],
            writable: &[f.page.bytes, position, reservation],
            readonly: &[
                f.epoch.bytes,
                f.grid.bytes,
                shared.system_program,
                shared.rent_sysvar,
            ],
            keys: &[
                owner,
                f.epoch.bytes,
                f.grid.bytes,
                f.page.bytes,
                position,
                reservation,
                shared.system_program,
                shared.rent_sysvar,
            ],
            data: layout_request(
                sequence,
                Intent::PlaceOrder {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    max_fee_atoms: 0,
                    slot,
                },
            ),
            heap: false,
        },
    )
}

/// Retire one order: a tombstone in the page and the envelope released.
pub fn cancel_order(f: &Friday, owner: [u8; 32], order_id: Hash32, generation: u64) -> Vec<u8> {
    let shared = &f.shared;
    let (position, _, _) = owner_plane(f, owner);
    let reservation = f.reservation(Hash32::from_bytes(owner), order_id).bytes;
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &readonly_signer_slot(shared, owner),
            writable: &[f.page.bytes, position, reservation],
            readonly: &[f.epoch.bytes],
            keys: &[owner, f.epoch.bytes, f.page.bytes, position, reservation],
            data: layout_request(
                generation,
                Intent::CancelOrder {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    owner: Hash32::from_bytes(owner),
                    order_id,
                    generation,
                },
            ),
            heap: false,
        },
    )
}

/// Seal the book at the deadline.
pub fn freeze_epoch(f: &Friday) -> Vec<u8> {
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[f.epoch.bytes, f.window.bytes, f.page.bytes],
            readonly: &[clock()],
            keys: &[f.epoch.bytes, f.window.bytes, clock(), f.page.bytes],
            data: layout_request(
                0,
                Intent::FreezeEpoch {
                    market: f.market_id,
                    epoch: f.epoch_id,
                },
            ),
            heap: false,
        },
    )
}

/// One candidate's coordinates, and the accounts its feed lives at.
pub struct Submission {
    pub id: Hash32,
    pub prices: [u64; MAX_OUTCOMES],
    pub virtual_split: u64,
    pub virtual_merge: u64,
    pub record: [u8; 32],
    pub feed: [u8; 32],
}

pub fn submit_candidate(f: &Friday, s: &Submission, declared_slices: Option<u16>) -> Vec<u8> {
    let shared = &f.shared;
    let keys = [
        shared.payer.bytes,
        f.epoch.bytes,
        f.window.bytes,
        s.record,
        s.feed,
        shared.system_program,
        shared.rent_sysvar,
        clock(),
    ];
    assert_eq!(keys.len(), SUBMIT_CANDIDATE_ACCOUNT_COUNT);
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[s.record, s.feed],
            readonly: &[
                f.epoch.bytes,
                f.window.bytes,
                shared.system_program,
                shared.rent_sysvar,
                clock(),
            ],
            keys: &keys,
            data: layout_request(
                0,
                Intent::SubmitCandidate {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    prices: s.prices,
                    virtual_split: s.virtual_split,
                    virtual_merge: s.virtual_merge,
                    honored_aon_mask: 0,
                    declared_slices,
                    weighted_direct_volume: 0,
                    limit_surplus_price_units: 0,
                    distinct_owners: 0,
                },
            ),
            heap: false,
        },
    )
}

pub fn write_feed(
    f: &Friday,
    s: &Submission,
    sequence: u64,
    chunk: &CandidateFeedChunk,
) -> Vec<u8> {
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[s.feed],
            readonly: &[],
            keys: &[s.feed],
            data: layout_request(
                sequence,
                Intent::WriteCandidateFeed {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate: s.id,
                    chunk: *chunk,
                },
            ),
            heap: false,
        },
    )
}

pub fn seal_candidate(f: &Friday, s: &Submission) -> Vec<u8> {
    let keys = [f.epoch.bytes, f.window.bytes, s.feed, clock()];
    assert_eq!(keys.len(), SEAL_CANDIDATE_FIXED_ACCOUNT_COUNT);
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[f.window.bytes, s.feed],
            readonly: &[f.epoch.bytes, clock()],
            keys: &keys,
            data: layout_request(
                0,
                Intent::SealCandidate {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate: s.id,
                },
            ),
            heap: false,
        },
    )
}

pub fn init_clear_work(f: &Friday, candidate: Hash32) -> Vec<u8> {
    let shared = &f.shared;
    let work = f.clear_work(candidate).bytes;
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[work],
            readonly: &[shared.system_program, shared.rent_sysvar],
            keys: &[
                shared.payer.bytes,
                work,
                shared.system_program,
                shared.rent_sysvar,
            ],
            data: layout_request(
                0,
                Intent::InitClearWork {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate,
                },
            ),
            heap: true,
        },
    )
}

pub fn grow_clear_work(f: &Friday, candidate: Hash32, sequence: u64) -> Vec<u8> {
    let work = f.clear_work(candidate).bytes;
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[work],
            readonly: &[],
            keys: &[work],
            data: layout_request(
                sequence,
                Intent::GrowClearWork {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate,
                },
            ),
            heap: true,
        },
    )
}

pub fn advance_clear_work(
    f: &Friday,
    candidate: Hash32,
    max_orders: u16,
    reservations: &[[u8; 32]],
) -> Vec<u8> {
    let work = f.clear_work(candidate).bytes;
    let feed = f.candidate_feed(candidate).bytes;
    let mut readonly = vec![f.epoch.bytes, feed, f.page.bytes];
    readonly.extend_from_slice(reservations);
    let mut keys = vec![f.epoch.bytes, feed, work, f.page.bytes];
    assert_eq!(keys.len(), ADVANCE_CLEAR_WORK_FIXED_ACCOUNT_COUNT);
    keys.extend_from_slice(reservations);
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[work],
            readonly: &readonly,
            keys: &keys,
            data: layout_request(
                0,
                Intent::AdvanceClearWork {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate,
                    max_orders,
                },
            ),
            heap: true,
        },
    )
}

pub fn advance_clear_slices(f: &Friday, candidate: Hash32, max_slices: u16) -> Vec<u8> {
    let work = f.clear_work(candidate).bytes;
    let feed = f.candidate_feed(candidate).bytes;
    let keys = [f.epoch.bytes, feed, work];
    assert_eq!(keys.len(), ADVANCE_CLEAR_SLICES_ACCOUNT_COUNT);
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[work],
            readonly: &[f.epoch.bytes, feed],
            keys: &keys,
            data: layout_request(
                0,
                Intent::AdvanceClearSlices {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate,
                    max_slices,
                },
            ),
            heap: true,
        },
    )
}

pub fn complete_clear_work(f: &Friday, candidate: Hash32) -> Vec<u8> {
    let work = f.clear_work(candidate).bytes;
    let feed = f.candidate_feed(candidate).bytes;
    let record = f.candidate_record(candidate).bytes;
    let keys = [f.epoch.bytes, feed, work, record];
    assert_eq!(keys.len(), COMPLETE_CLEAR_WORK_ACCOUNT_COUNT);
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[work, record],
            readonly: &[f.epoch.bytes, feed],
            keys: &keys,
            data: layout_request(
                0,
                Intent::CompleteClearWork {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate,
                },
            ),
            heap: true,
        },
    )
}

pub fn finalize_selection(f: &Friday, retained: &[Hash32]) -> Vec<u8> {
    let mut writable = vec![f.epoch.bytes, f.window.bytes];
    let mut readonly = vec![clock()];
    let mut keys = vec![f.epoch.bytes, f.window.bytes, clock()];
    for candidate in retained {
        let record = f.candidate_record(*candidate).bytes;
        let feed = f.candidate_feed(*candidate).bytes;
        writable.push(record);
        readonly.push(feed);
        keys.push(record);
        keys.push(feed);
    }
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &writable,
            readonly: &readonly,
            keys: &keys,
            data: layout_request(
                0,
                Intent::FinalizeSelection {
                    market: f.market_id,
                    epoch: f.epoch_id,
                },
            ),
            heap: false,
        },
    )
}

pub fn freeze_entitlement(f: &Friday, candidate: Hash32) -> Vec<u8> {
    let shared = &f.shared;
    let record = f.candidate_record(candidate).bytes;
    let work = f.clear_work(candidate).bytes;
    let keys = [
        shared.payer.bytes,
        f.epoch.bytes,
        record,
        work,
        f.pot.bytes,
        shared.system_program,
        shared.rent_sysvar,
    ];
    assert_eq!(keys.len(), FREEZE_ENTITLEMENT_ACCOUNT_COUNT);
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &[f.pot.bytes],
            readonly: &[
                f.epoch.bytes,
                record,
                work,
                shared.system_program,
                shared.rent_sysvar,
            ],
            keys: &keys,
            data: layout_request(
                0,
                Intent::FreezeEntitlement {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate,
                },
            ),
            heap: true,
        },
    )
}

/// One entitlement, single-Egg or portfolio-pair shaped.
pub struct Entitle<'a> {
    pub candidate: Hash32,
    pub slice_index: u16,
    pub buy_reservation: [u8; 32],
    pub sell_reservation: [u8; 32],
    /// Present for a portfolio pair: the terms account and every receipt the
    /// pair's legs write.  Empty for a single-Egg slice.
    pub pair_receipts: Option<&'a [u16]>,
}

pub fn entitle_slice(f: &Friday, e: &Entitle<'_>) -> Vec<u8> {
    let shared = &f.shared;
    let record = f.candidate_record(e.candidate).bytes;
    let feed = f.candidate_feed(e.candidate).bytes;
    let mut keys = vec![
        shared.payer.bytes,
        f.epoch.bytes,
        record,
        feed,
        f.pot.bytes,
        shared.system_program,
        shared.rent_sysvar,
    ];
    assert_eq!(keys.len(), ENTITLE_SLICE_FIXED_ACCOUNT_COUNT);
    let mut readonly = vec![
        f.epoch.bytes,
        record,
        feed,
        f.pot.bytes,
        shared.system_program,
        shared.rent_sysvar,
        f.page.bytes,
    ];
    let mut writable = vec![e.buy_reservation, e.sell_reservation];
    keys.push(f.page.bytes);
    if e.pair_receipts.is_some() {
        keys.push(f.terms.bytes);
        readonly.push(f.terms.bytes);
    }
    keys.push(e.buy_reservation);
    keys.push(e.sell_reservation);
    let slices: Vec<u16> = e
        .pair_receipts
        .map_or_else(|| vec![e.slice_index], <[u16]>::to_vec);
    for slice in &slices {
        let receipt = f.receipt(e.candidate, *slice).bytes;
        keys.push(receipt);
        writable.push(receipt);
    }
    general_transaction(
        shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &writable,
            readonly: &readonly,
            keys: &keys,
            data: layout_request(
                0,
                Intent::EntitleSlice {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    candidate: e.candidate,
                    slice_index: e.slice_index,
                },
            ),
            heap: false,
        },
    )
}

/// One settlement, single-Egg or portfolio-pair shaped.
pub struct Settle<'a> {
    pub candidate: Hash32,
    pub sequence: u64,
    pub buyer_position: [u8; 32],
    pub seller_position: [u8; 32],
    pub buy_reservation: [u8; 32],
    pub sell_reservation: [u8; 32],
    pub slice_index: u16,
    pub pair_receipts: Option<&'a [u16]>,
}

pub fn settle_page(f: &Friday, s: &Settle<'_>) -> Vec<u8> {
    let record = f.candidate_record(s.candidate).bytes;
    let mut keys = vec![f.epoch.bytes, record];
    let mut readonly = vec![f.epoch.bytes, record];
    let mut writable = vec![
        s.buyer_position,
        s.seller_position,
        s.buy_reservation,
        s.sell_reservation,
    ];
    if s.pair_receipts.is_some() {
        keys.push(f.terms.bytes);
        readonly.push(f.terms.bytes);
    }
    keys.push(s.buyer_position);
    keys.push(s.seller_position);
    keys.push(s.buy_reservation);
    keys.push(s.sell_reservation);
    if s.pair_receipts.is_some() {
        assert_eq!(keys.len(), SETTLE_PAIR_FIXED_ACCOUNT_COUNT);
        keys.push(f.page.bytes);
        readonly.push(f.page.bytes);
    }
    let slices: Vec<u16> = s
        .pair_receipts
        .map_or_else(|| vec![s.slice_index], <[u16]>::to_vec);
    for slice in &slices {
        let receipt = f.receipt(s.candidate, *slice).bytes;
        keys.push(receipt);
        writable.push(receipt);
    }
    if s.pair_receipts.is_none() {
        assert_eq!(keys.len(), orders_batch::SETTLE_PAGE_ACCOUNT_COUNT);
    }
    general_transaction(
        &f.shared,
        GeneralTx {
            writable_signers: &[],
            readonly_signers: &[],
            writable: &writable,
            readonly: &readonly,
            keys: &keys,
            data: layout_request(
                s.sequence,
                Intent::SettlePage {
                    market: f.market_id,
                    epoch: f.epoch_id,
                    page_index: 0,
                },
            ),
            heap: false,
        },
    )
}

/* ------------------------------------------------------------------ */
/* Role plumbing                                                       */
/* ------------------------------------------------------------------ */

fn owner_plane(f: &Friday, owner: [u8; 32]) -> ([u8; 32], [u8; 32], [u8; 32]) {
    let actor = f
        .actors
        .iter()
        .find(|actor| actor.key == owner)
        .expect("every Friday signer is a session actor");
    (actor.position.bytes, actor.replay.bytes, actor.token.bytes)
}

fn position_of(f: &Friday, owner: [u8; 32]) -> [u8; 32] {
    owner_plane(f, owner).0
}

fn replay_of(f: &Friday, owner: [u8; 32]) -> [u8; 32] {
    owner_plane(f, owner).1
}

/// The fee payer is already the first writable signer, so an owner that *is*
/// the payer must not be listed twice.
fn signer_slot(shared: &clutch_sbf_harness::Shared, key: [u8; 32]) -> Vec<[u8; 32]> {
    if key == shared.payer.bytes {
        Vec::new()
    } else {
        vec![key]
    }
}

fn readonly_signer_slot(shared: &clutch_sbf_harness::Shared, key: [u8; 32]) -> Vec<[u8; 32]> {
    if key == shared.payer.bytes {
        Vec::new()
    } else {
        vec![key]
    }
}
