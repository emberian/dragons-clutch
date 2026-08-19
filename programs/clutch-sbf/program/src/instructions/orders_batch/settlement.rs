//! Fail-closed settlement preflight and checkpoint model.
//!
//! This module deliberately stops before `relation_v1_stream`.  It closes the
//! joins which can be checked without inventing an on-chain policy preimage,
//! truncating a 32-byte identity to `u64`, or interpreting the opaque
//! `ClearWorkV1` bytes as a Rust value.  A successful [`verify_preflight`]
//! therefore means only that one submitted candidate feed, one checkpoint
//! header, and the complete frozen page set are mutually bound.  It is not a
//! candidate-verification or settlement verdict.

use clutch_solana_layout::{
    clearing::{
        verify_candidate_feed, verify_clear_work, CandidateFeedHeader, ClearWorkHeader,
        CLEAR_WORK_STATUS_COMPLETE,
    },
    stream, CandidateRecord, CodecError, EpochAccount, Hash32, CANDIDATE_STATUS_SUBMITTED,
    EPOCH_PHASE_FROZEN,
};

/// The facts discharged by the byte-level preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PreflightFacts {
    /// Market identity shared by every input.
    pub market: Hash32,
    /// Frozen epoch identity shared by every input.
    pub epoch: Hash32,
    /// Submitted candidate identity shared by record, feed, and checkpoint.
    pub candidate: Hash32,
    /// Recomputed frozen page-set identity.
    pub order_set: Hash32,
    /// Total populated slots, including retirements.
    pub slot_count: u16,
    /// Live relation orders.  Today this necessarily equals `slot_count`:
    /// [`CandidateRecord::binds_epoch`] still owns the unresolved tombstone
    /// cardinality join and refuses every other value.
    pub live_order_count: u16,
    /// Complete frozen page count.
    pub page_count: u16,
    /// Page named by the `SettlePage` wire and the checkpoint cursor.
    pub page_cursor: u16,
}

/// Borrowed inputs to one structural settlement preflight.
///
/// Grouped so an eventual account-plane adapter has one typed handoff rather
/// than eight positional arguments which could be silently transposed.
pub(super) struct PreflightInput<'a> {
    pub epoch_bytes: &'a [u8],
    pub candidate_bytes: &'a [u8],
    pub feed_bytes: &'a [u8],
    pub clear_work_bytes: &'a [u8],
    pub pages: &'a [&'a [u8]],
    pub intent_market: Hash32,
    pub intent_epoch: Hash32,
    pub intent_page: u16,
}

/// Verify every presently representable settlement binding.
///
/// The order matters.  Each input first passes its owning codec.  The complete
/// page set is then recomputed and bound to the epoch before any candidate or
/// checkpoint claim is trusted.  No account is mutated.
// The production instruction cannot call this until the missing account-init
// and stable-body joins land.  Keeping the executable preflight compiled (and
// tested) is intentional; it is not dormant settlement success.
#[allow(dead_code)]
pub(super) fn verify_preflight(input: &PreflightInput<'_>) -> Result<PreflightFacts, CodecError> {
    let epoch = EpochAccount::decode(input.epoch_bytes)?;
    if epoch.phase != EPOCH_PHASE_FROZEN
        || epoch.market != input.intent_market
        || epoch.epoch != input.intent_epoch
        || input.intent_page >= epoch.page_count
    {
        return Err(CodecError::MismatchedBinding);
    }

    // Inclusion is not inferred from a page carrying the same `order_set`.
    // Every page is present, verified, and folded into that identity here.
    stream::epoch_binds_page_set(&epoch, input.pages)?;

    let mut live_order_count = 0u16;
    let mut page_index = 0usize;
    while page_index < input.pages.len() {
        let header = stream::OrderPageHeader::decode(input.pages[page_index])?;
        live_order_count = live_order_count
            .checked_add(u16::from(header.live_count()))
            .ok_or(CodecError::ArithmeticOverflow)?;
        page_index += 1;
    }

    let candidate = CandidateRecord::decode(input.candidate_bytes)?;
    if candidate.status != CANDIDATE_STATUS_SUBMITTED {
        return Err(CodecError::MismatchedBinding);
    }
    /* This intentionally calls the semantic owner's current binding rather
     * than weakening it locally.  Since `epoch.order_count` includes
     * tombstones and a candidate feed skips them, a cancelled book stops here
     * until the layout schema owns an exact live cardinality. */
    candidate.binds_epoch(&epoch)?;
    if u16::from(candidate.order_len) != live_order_count {
        return Err(CodecError::MismatchedBinding);
    }

    let feed = verify_candidate_feed(input.feed_bytes)?;
    bind_feed(&feed, &candidate, &epoch)?;

    let clear = verify_clear_work(input.clear_work_bytes)?;
    bind_checkpoint(&clear, &candidate, &epoch, input.intent_page)?;

    Ok(PreflightFacts {
        market: epoch.market,
        epoch: epoch.epoch,
        candidate: candidate.candidate,
        order_set: epoch.order_set,
        slot_count: epoch.order_count,
        live_order_count,
        page_count: epoch.page_count,
        page_cursor: input.intent_page,
    })
}

/// Bind the solver-written feed to the candidate record and frozen set.
#[allow(dead_code)]
fn bind_feed(
    feed: &CandidateFeedHeader,
    candidate: &CandidateRecord,
    epoch: &EpochAccount,
) -> Result<(), CodecError> {
    if feed.candidate != candidate.candidate
        || feed.epoch != candidate.epoch
        || feed.market != candidate.market
        || feed.order_set != epoch.order_set
        || feed.prices != candidate.prices
        || feed.virtual_split != candidate.virtual_split
        || feed.virtual_merge != candidate.virtual_merge
        || feed.honored_aon_mask != candidate.honored_aon_mask
        || feed.weighted_direct_volume != candidate.weighted_direct_volume
        || feed.limit_surplus_price_units != candidate.limit_surplus_price_units
        || feed.churn != candidate.churn
        || feed.distinct_owners != candidate.distinct_owners
        || feed.order_len != candidate.order_len
        || feed.outcome_count != candidate.outcome_count
    {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(())
}

/// Bind the checkpoint's layout-owned header without interpreting its body.
#[allow(dead_code)]
fn bind_checkpoint(
    clear: &ClearWorkHeader,
    candidate: &CandidateRecord,
    epoch: &EpochAccount,
    intent_page: u16,
) -> Result<(), CodecError> {
    if clear.market != candidate.market
        || clear.epoch != candidate.epoch
        || clear.candidate != candidate.candidate
        || clear.page_cursor != intent_page
        || clear.status == CLEAR_WORK_STATUS_COMPLETE
    {
        return Err(CodecError::MismatchedBinding);
    }
    // An open checkpoint is canonically unbound by its codec.  Once bound, it
    // must remain on this exact frozen set.  The body remains opaque in both
    // cases and no relation step is attempted here.
    if clear.order_set != Hash32::ZERO && clear.order_set != epoch.order_set {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(())
}

/// Ranked prerequisites which keep the on-chain relation transition closed.
///
/// Order is dependency order, not severity.  A caller must not skip an earlier
/// item by fabricating the fact needed by a later one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SettlementBlocker {
    /// Orders are bookkeeping records; no position/hoard reservation backs them.
    FundedReservation,
    /// `EpochAccount` has no exact live-order cardinality after tombstones.
    TombstoneCardinality,
    /// The epoch persists a policy identity but no `FrozenPolicyV1` preimage.
    FrozenPolicyPreimage,
    /// Four relation-domain `u64` identities have no injective Hash32 mapping.
    LosslessDomainIdentity,
    /// `ClearWorkV1` is opaque `repr(Rust)` state, not a stable byte codec.
    PortableCheckpointBody,
    /// Neither clearing account has an authenticated init/realloc lifecycle.
    CheckpointInitialization,
    /// Candidate-set closure/selection is not an on-chain transition.
    CandidateSelection,
    /// Receipts/pot are not frozen as pre-resolution settlement entitlements.
    EntitlementFreeze,
}

/// The exact dependency order of the remaining settlement work.
pub(super) const SETTLEMENT_BLOCKERS: [SettlementBlocker; 8] = [
    SettlementBlocker::FundedReservation,
    SettlementBlocker::TombstoneCardinality,
    SettlementBlocker::FrozenPolicyPreimage,
    SettlementBlocker::LosslessDomainIdentity,
    SettlementBlocker::PortableCheckpointBody,
    SettlementBlocker::CheckpointInitialization,
    SettlementBlocker::CandidateSelection,
    SettlementBlocker::EntitlementFreeze,
];

/// A tiny executable state machine for the part which is safe today.
///
/// It can bind one byte-verified preflight, idempotently.  It cannot enter a
/// relation-verified or entitlement phase: [`advance_relation`] returns the
/// first ranked blocker without changing a byte.  This makes the current STOP
/// an executable property instead of a comment beside a success-shaped API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FailClosedCheckpoint {
    bound: Option<PreflightFacts>,
}

impl FailClosedCheckpoint {
    pub const NEW: Self = Self { bound: None };

    /// Bind once; an exact replay is idempotent and a conflicting replay fails.
    #[allow(dead_code)]
    pub fn bind(&mut self, facts: PreflightFacts) -> Result<(), CodecError> {
        match self.bound {
            None => {
                self.bound = Some(facts);
                Ok(())
            }
            Some(before) if before == facts => Ok(()),
            Some(_) => Err(CodecError::MismatchedBinding),
        }
    }

    /// Refuse the first relation step and leave the checkpoint unchanged.
    pub fn advance_relation(&mut self) -> Result<(), SettlementBlocker> {
        // An unbound invocation cannot even claim the first prerequisite was
        // reached; the same blocker is returned because the production wire
        // has no new error allocation for this research checkpoint.
        Err(SETTLEMENT_BLOCKERS[0])
    }
}

/// The production `SettlePage` terminus until the ranked prerequisites land.
pub(super) fn refuse_unintegrated() -> crate::accounts::Outcome<()> {
    let mut checkpoint = FailClosedCheckpoint::NEW;
    let before = checkpoint;
    let _blocker = checkpoint.advance_relation();
    debug_assert_eq!(checkpoint, before);
    Err(crate::error::ClutchError::NotYetImplemented.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        clearing::{bind_order_set, init_candidate_feed, init_clear_work, CandidateFeedHeader},
        stream::{append_slot, frozen_set_commitment, init_page, seal_page},
        CandidateRecord, OrderRecord, OrderSlot, CANDIDATE_STATUS_SELECTED, MAX_OUTCOMES,
        RELATION_VERSION,
    };

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn order(rank: u64, owner: u8) -> OrderRecord {
        OrderRecord {
            owner: h(owner),
            order_id: canonical_order_id(rank),
            outcome: (rank as u8 - 1) & 1,
            side: (rank as u8 - 1) & 1,
            quantity: 10,
            limit: 5_000,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: 9,
        }
    }

    struct Fixture {
        epoch: [u8; account_len::EPOCH],
        candidate: [u8; account_len::CANDIDATE],
        feed: [u8; account_len::CANDIDATE_FEED],
        work: [u8; account_len::CLEAR_WORK],
        page: [u8; account_len::ORDER_PAGE],
        market: Hash32,
        epoch_id: Hash32,
    }

    fn fixture() -> Fixture {
        let market = h(1);
        let epoch_id = canonical_epoch_id(market, 4);
        let mut page = [0; account_len::ORDER_PAGE];
        init_page(&mut page, market, epoch_id, 0, 1, 5).unwrap();
        append_slot(&mut page, OrderSlot::Single(order(1, 0x20))).unwrap();
        append_slot(&mut page, OrderSlot::Single(order(2, 0x21))).unwrap();
        let (order_set, slot_count) = frozen_set_commitment(&[&page]).unwrap();
        seal_page(&mut page, order_set, slot_count).unwrap();

        let epoch_account = EpochAccount {
            epoch: epoch_id,
            market,
            book: h(2),
            terms: h(3),
            price_grid: h(4),
            policy: h(5),
            order_set,
            first_order_id: canonical_order_id(1),
            last_order_id: canonical_order_id(2),
            epoch_index: 4,
            relation_version: RELATION_VERSION,
            price_scale: 10_000,
            remainder_seed: 7,
            owner_count: 2,
            page_count: 1,
            order_count: 2,
            outcome_count: 2,
            phase: EPOCH_PHASE_FROZEN,
            stored_bump: 6,
            flags: 0,
        };
        let mut epoch = [0; account_len::EPOCH];
        epoch_account.encode(&mut epoch).unwrap();

        let prices = {
            let mut values = [0; MAX_OUTCOMES];
            values[0] = 5_000;
            values[1] = 5_000;
            values
        };
        let mut candidate_account = CandidateRecord {
            candidate: Hash32::ZERO,
            epoch: epoch_id,
            market,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 20,
            limit_surplus_price_units: 0,
            churn: 0,
            submitted_slot: 99,
            distinct_owners: 2,
            order_len: 2,
            outcome_count: 2,
            status: CANDIDATE_STATUS_SUBMITTED,
            stored_bump: 7,
            flags: 0,
        };
        candidate_account.candidate = candidate_account.recomputed_candidate_digest().unwrap();
        let mut candidate = [0; account_len::CANDIDATE];
        candidate_account.encode(&mut candidate).unwrap();

        let feed_header = CandidateFeedHeader {
            candidate: candidate_account.candidate,
            epoch: epoch_id,
            market,
            order_set,
            prices,
            virtual_split: 0,
            virtual_merge: 0,
            honored_aon_mask: 0,
            weighted_direct_volume: 20,
            limit_surplus_price_units: 0,
            claimed_digest: 123,
            churn: 0,
            declared_slices: 0,
            distinct_owners: 2,
            order_len: 2,
            outcome_count: 2,
            stored_bump: 8,
            flags: 0,
        };
        let mut feed = [0; account_len::CANDIDATE_FEED];
        init_candidate_feed(&mut feed, &feed_header).unwrap();

        let mut work = [0; account_len::CLEAR_WORK];
        init_clear_work(&mut work, market, epoch_id, candidate_account.candidate, 9).unwrap();

        Fixture {
            epoch,
            candidate,
            feed,
            work,
            page,
            market,
            epoch_id,
        }
    }

    fn preflight(f: &Fixture) -> Result<PreflightFacts, CodecError> {
        verify_preflight(&PreflightInput {
            epoch_bytes: &f.epoch,
            candidate_bytes: &f.candidate,
            feed_bytes: &f.feed,
            clear_work_bytes: &f.work,
            pages: &[&f.page],
            intent_market: f.market,
            intent_epoch: f.epoch_id,
            intent_page: 0,
        })
    }

    #[test]
    fn complete_page_set_candidate_feed_and_checkpoint_bind() {
        let f = fixture();
        let facts = preflight(&f).unwrap();
        assert_eq!(facts.market, f.market);
        assert_eq!(facts.epoch, f.epoch_id);
        assert_eq!(facts.slot_count, 2);
        assert_eq!(facts.live_order_count, 2);
        assert_eq!(facts.page_count, 1);
        assert_eq!(facts.page_cursor, 0);
    }

    #[test]
    fn page_inclusion_candidate_and_checkpoint_tampering_refuse() {
        let f = fixture();

        let mut wrong_page = f.page;
        let last = wrong_page.len() - 1;
        wrong_page[last] ^= 1;
        assert_eq!(
            verify_preflight(&PreflightInput {
                epoch_bytes: &f.epoch,
                candidate_bytes: &f.candidate,
                feed_bytes: &f.feed,
                clear_work_bytes: &f.work,
                pages: &[&wrong_page],
                intent_market: f.market,
                intent_epoch: f.epoch_id,
                intent_page: 0,
            }),
            Err(CodecError::NonCanonicalPadding)
        );

        let mut wrong_feed = f.feed;
        // Reframe a valid feed against a different order set; the header is
        // internally valid but cannot bind this epoch.
        let mut header = CandidateFeedHeader::decode(&wrong_feed).unwrap();
        header.order_set = h(0xaa);
        init_candidate_feed(&mut wrong_feed, &header).unwrap();
        assert_eq!(
            verify_preflight(&PreflightInput {
                epoch_bytes: &f.epoch,
                candidate_bytes: &f.candidate,
                feed_bytes: &wrong_feed,
                clear_work_bytes: &f.work,
                pages: &[&f.page],
                intent_market: f.market,
                intent_epoch: f.epoch_id,
                intent_page: 0,
            }),
            Err(CodecError::MismatchedBinding)
        );

        let mut wrong_work = [0; account_len::CLEAR_WORK];
        let candidate = CandidateRecord::decode(&f.candidate).unwrap();
        init_clear_work(
            &mut wrong_work,
            f.market,
            f.epoch_id,
            candidate.candidate,
            9,
        )
        .unwrap();
        bind_order_set(&mut wrong_work, h(0xbb), 1).unwrap();
        assert_eq!(
            verify_preflight(&PreflightInput {
                epoch_bytes: &f.epoch,
                candidate_bytes: &f.candidate,
                feed_bytes: &f.feed,
                clear_work_bytes: &wrong_work,
                pages: &[&f.page],
                intent_market: f.market,
                intent_epoch: f.epoch_id,
                intent_page: 0,
            }),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn a_selected_candidate_cannot_reenter_verification() {
        let mut f = fixture();
        let mut candidate = CandidateRecord::decode(&f.candidate).unwrap();
        candidate.status = CANDIDATE_STATUS_SELECTED;
        candidate.encode(&mut f.candidate).unwrap();
        assert_eq!(preflight(&f), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn tombstone_cardinality_stays_a_fail_closed_semantic_owner_stop() {
        let mut f = fixture();
        // The frozen page is rebuilt with one retirement.  Epoch order_count is
        // still two populated slots; the relation feed has one live order.
        let mut page = [0; account_len::ORDER_PAGE];
        init_page(&mut page, f.market, f.epoch_id, 0, 1, 5).unwrap();
        let first = order(1, 0x20);
        append_slot(&mut page, OrderSlot::Single(first)).unwrap();
        append_slot(&mut page, OrderSlot::Single(order(2, 0x21))).unwrap();
        clutch_solana_layout::stream::write_tombstone(&mut page, first.order_id, first.owner, 2)
            .unwrap();
        let (order_set, slots) = frozen_set_commitment(&[&page]).unwrap();
        seal_page(&mut page, order_set, slots).unwrap();
        f.page = page;

        let mut epoch = EpochAccount::decode(&f.epoch).unwrap();
        epoch.order_set = order_set;
        epoch.encode(&mut f.epoch).unwrap();

        let mut candidate = CandidateRecord::decode(&f.candidate).unwrap();
        candidate.order_len = 1;
        candidate.candidate = candidate.recomputed_candidate_digest().unwrap();
        candidate.encode(&mut f.candidate).unwrap();
        // Rebind otherwise-valid feed/work to the live candidate; the epoch's
        // semantic owner still refuses `1 != 2`, before an adapter can invent a
        // cardinality convention.
        let mut feed = CandidateFeedHeader::decode(&f.feed).unwrap();
        feed.order_len = 1;
        feed.order_set = order_set;
        feed.candidate = feed.recomputed_candidate_digest().unwrap();
        init_candidate_feed(&mut f.feed, &feed).unwrap();
        init_clear_work(&mut f.work, f.market, f.epoch_id, feed.candidate, 9).unwrap();

        assert_eq!(preflight(&f), Err(CodecError::MismatchedBinding));
    }

    #[test]
    fn fail_closed_checkpoint_is_idempotent_and_refusal_atomic() {
        let facts = preflight(&fixture()).unwrap();
        let mut checkpoint = FailClosedCheckpoint::NEW;
        checkpoint.bind(facts).unwrap();
        let after_first = checkpoint;
        checkpoint.bind(facts).unwrap();
        assert_eq!(checkpoint, after_first, "exact replay is idempotent");

        let mut conflicting = facts;
        conflicting.page_cursor = 1;
        assert_eq!(
            checkpoint.bind(conflicting),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(checkpoint, after_first, "conflict writes nothing");

        assert_eq!(
            checkpoint.advance_relation(),
            Err(SettlementBlocker::FundedReservation)
        );
        assert_eq!(checkpoint, after_first, "blocked advance writes nothing");
    }

    #[test]
    fn ranked_blockers_keep_relation_and_entitlement_phases_unreachable() {
        assert_eq!(SETTLEMENT_BLOCKERS.len(), 8);
        assert_eq!(SETTLEMENT_BLOCKERS[0], SettlementBlocker::FundedReservation);
        assert_eq!(
            SETTLEMENT_BLOCKERS[1],
            SettlementBlocker::TombstoneCardinality
        );
        assert_eq!(SETTLEMENT_BLOCKERS[7], SettlementBlocker::EntitlementFreeze);
    }
}
