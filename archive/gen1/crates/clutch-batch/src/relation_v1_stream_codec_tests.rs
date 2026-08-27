//! The checkpoint-codec battery (Tier 2 join 5).
//!
//! Four obligations, stated in the Tier 2 plan and discharged here:
//!
//! 1. **Round-trip identity** at representative reachable states (the
//!    every-push-boundary corpus lives in the resumption gate of
//!    `relation_v1_stream_tests.rs`, upgraded to `save = encode / resume =
//!    decode`);
//! 2. **Hostile-byte totality**: every byte of a valid encoding flipped, every
//!    control field swept out of range — no panic, typed refusals, and the
//!    accepted set closed under re-encoding;
//! 3. **The three-layer tamper stack**, pinned mutation by mutation: (a) the
//!    fold seal (`ResumeFoldMismatch`), (c) the `(order_set, consumed_fold)`
//!    anchor the program compares at every resume.  Layer (b) — header
//!    mutations — is `clutch-solana-layout`'s and is pinned by its own
//!    `the_checkpoint_refuses_every_hostile_frame`;
//! 4. The codec's **frame** obligation is measured by the SBF `.stack_sizes`
//!    probe (design §9), not here.

use super::codec_offsets as off;
use super::*;
use crate::relation_v1::{
    canonical_candidate, canonical_pairing, AllocationPolicyV1, AonPolicyV1, BookV1, CandidateV1,
    ErrorV1, FeeBaseV1, FrozenPolicyV1, OrderV1, PairingWitnessPolicyV1, PairingWitnessV1,
    PortfolioLotPolicyV1, RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1,
    ScorePolicyV1, SelfCrossPolicyV1, SingleEggOrderV1, TransferPhaseV1, MAX_OUTCOMES, PRICE_SCALE,
    RELATION_VERSION_V1,
};
use crate::{DustPolicy, PartialPolicy, Side};

extern crate std;
use std::boxed::Box;
use std::vec;
use std::vec::Vec;

const SCALE: u64 = PRICE_SCALE;

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

fn domain_with(policy: FrozenPolicyV1, outcomes: u8, owners: u16) -> RelationDomainV1 {
    RelationDomainV1 {
        relation_version: RELATION_VERSION_V1,
        market_id: 11,
        book_id: 22,
        epoch: 7,
        policy_id: 33,
        order_set_id: 44,
        outcome_count: outcomes,
        owner_count: owners,
        price_scale: SCALE,
        remainder_seed: 0x00C0_FFEE,
        policy,
    }
}

fn single(id: u64, owner: u16, outcome: u8, side: Side, quantity: u64, limit: u64) -> OrderV1 {
    OrderV1::SingleEgg(SingleEggOrderV1 {
        canonical_order_id: id,
        owner,
        outcome,
        side,
        quantity,
        limit_price: limit,
        minimum_fill: 1,
        partial_policy: PartialPolicy::Allow,
        expiry_epoch: u64::MAX,
    })
}

fn book_of(orders: &[OrderV1]) -> BookV1 {
    let mut book = BookV1::empty();
    let mut i = 0usize;
    while i < orders.len() {
        book.orders[i] = orders[i];
        i += 1;
    }
    book.len = orders.len() as u8;
    book
}

fn prices(values: &[u64]) -> [u64; MAX_OUTCOMES] {
    let mut vector = [0u64; MAX_OUTCOMES];
    let mut i = 0usize;
    while i < values.len() {
        vector[i] = values[i];
        i += 1;
    }
    vector
}

fn four_book() -> BookV1 {
    book_of(&[
        single(1, 0, 0, Side::Buy, 2, SCALE),
        single(2, 1, 0, Side::Sell, 2, 0),
        single(3, 2, 1, Side::Buy, 1, SCALE),
        single(4, 1, 1, Side::Sell, 1, 0),
    ])
}

fn header_of(candidate: &CandidateV1, pairing: Option<&PairingWitnessV1>) -> StreamCandidateV1 {
    StreamCandidateV1 {
        order_len: candidate.order_len,
        prices: candidate.prices,
        virtual_split: candidate.virtual_split,
        virtual_merge: candidate.virtual_merge,
        honored_aon_mask: candidate.honored_aon_mask,
        claimed_score: candidate.claimed_score,
        canonical_candidate_digest: candidate.canonical_candidate_digest,
        declared_slices: pairing.map(|witness| witness.len),
    }
}

fn buffer() -> Vec<u8> {
    vec![0u8; ClearWorkV1::ENCODED_BYTES]
}

/// `begin` plus one whole order pass, uninterrupted.
fn seal_pass_one(
    work: &mut ClearWorkV1,
    domain: &RelationDomainV1,
    book: &BookV1,
    candidate: &CandidateV1,
) {
    let header = header_of(candidate, None);
    work.begin(domain, &header, true).unwrap();
    let mut j = 0usize;
    while j < book.len as usize {
        work.push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
        j += 1;
    }
    work.end_pass().unwrap();
    assert_eq!(work.status(), FeedStatusV1::NeedOrders { pass: 2 });
}

/// Feed the true `(order, fill)` sequence until the feed completes or refuses,
/// bounded so a tampered pass counter cannot spin forever.  `Ok(())` means a
/// verdict became available; `Err` is the feed-protocol refusal.
fn resume_to_completion(
    work: &mut ClearWorkV1,
    book: &BookV1,
    candidate: &CandidateV1,
) -> Result<(), FeedErrorV1> {
    let mut rounds = 0usize;
    loop {
        match work.status() {
            FeedStatusV1::Complete => return Ok(()),
            FeedStatusV1::NeedOrders { .. } => {
                let mut j = 0usize;
                while j < book.len as usize {
                    if work.status() == FeedStatusV1::Complete {
                        break;
                    }
                    work.push_order(&book.orders[j], candidate.fills[j])?;
                    j += 1;
                }
                if work.status() != FeedStatusV1::Complete {
                    work.end_pass()?;
                }
            }
            FeedStatusV1::NeedSlices => {
                work.end_pass()?;
            }
        }
        rounds += 1;
        if rounds > 8 {
            // A tampered pass ladder that never completes yields no verdict;
            // that is a refusal for the caller's purposes.
            return Err(FeedErrorV1::NotInProgress);
        }
    }
}

/* ------------------------------------------------------------------------ */
/* Pins and round trips                                                      */
/* ------------------------------------------------------------------------ */

#[test]
fn clear_work_encoded_bytes_are_pinned() {
    // The cross-crate half of this pin is `clutch-solana-layout`'s
    // `CLEAR_WORK_BODY_BYTES` and its `clearing_account_golden_lengths`.
    assert_eq!(ClearWorkV1::ENCODED_BYTES, 47_846);
    assert_eq!(POLICY_ENCODED_BYTES, 15);
    assert_eq!(off::END, ClearWorkV1::ENCODED_BYTES);
    // The encoding is strictly smaller than the in-memory struct: it packs
    // enums, bools, and the Option, so a size_of regression cannot hide here.
    assert!(ClearWorkV1::ENCODED_BYTES < core::mem::size_of::<ClearWorkV1>());
}

#[test]
fn named_offsets_locate_their_fields() {
    let domain = domain_with(base_policy(), 2, 3);
    let book = four_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let mut work = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work, &domain, &book, &candidate);
    let mut out = buffer();
    work.encode_into(&mut out).unwrap();

    assert_eq!(out[off::PHASE], work.phase);
    assert_eq!(out[off::PASS], work.pass);
    assert_eq!(out[off::SLICES_EXPECTED], work.slices_expected as u8);
    assert_eq!(out[off::CHECK_CLAIMS], work.check_claims as u8);
    assert_eq!(out[off::CURSOR..off::CURSOR + 2], work.cursor.to_le_bytes());
    assert_eq!(
        out[off::SLICE_CURSOR..off::SLICE_CURSOR + 2],
        work.slice_cursor.to_le_bytes()
    );
    assert_eq!(
        out[off::ORDER_COUNT..off::ORDER_COUNT + 2],
        work.order_count.to_le_bytes()
    );
    assert_eq!(out[off::LATCH_SET], work.latch_set as u8);
    assert_eq!(out[off::LATCH_ERROR], 0, "idle latch is code 0");
    let (high, low) = work.fold.words();
    assert_eq!(out[off::FOLD..off::FOLD + 8], high.to_le_bytes());
    assert_eq!(out[off::FOLD + 8..off::FOLD + 16], low.to_le_bytes());
    let (high, low) = work.sealed_fold.words();
    assert_eq!(
        out[off::SEALED_FOLD..off::SEALED_FOLD + 8],
        high.to_le_bytes()
    );
    assert_eq!(
        out[off::SEALED_FOLD + 8..off::SEALED_FOLD + 16],
        low.to_le_bytes()
    );
    let (high, _) = work.digest.words();
    assert_eq!(out[off::DIGEST..off::DIGEST + 8], high.to_le_bytes());
    assert_eq!(
        out[off::PREVIOUS_ID..off::PREVIOUS_ID + 8],
        work.previous_id.to_le_bytes()
    );
    assert_eq!(
        out[off::DOMAIN..off::DOMAIN + 4],
        work.domain.relation_version.to_le_bytes()
    );
    assert_eq!(out[off::DOMAIN_OUTCOME_COUNT], work.domain.outcome_count);
    assert_eq!(
        out[off::DOMAIN_PRICE_SCALE..off::DOMAIN_PRICE_SCALE + 8],
        work.domain.price_scale.to_le_bytes()
    );
    assert_eq!(out[off::DOMAIN_POLICY], 0, "allocation A is selector 0");
    assert_eq!(out[off::CAND], work.cand.order_len);
    assert_eq!(out[off::CAND_DECLARED], 0, "no declared witness");
    assert_eq!(
        out[off::OWNERS..off::OWNERS + 2],
        work.owners[0].to_le_bytes()
    );
    assert_eq!(
        out[off::OWNER_SLOTS..off::OWNER_SLOTS + 2],
        work.owner_slots.to_le_bytes()
    );
    assert_eq!(
        out[off::OWNER_SLOT..off::OWNER_SLOT + 2],
        work.owner_slot[0].to_le_bytes()
    );
    assert_eq!(
        out[off::SIDE_BUY_BITS..off::SIDE_BUY_BITS + 8],
        work.side_buy_bits.to_le_bytes()
    );
    assert_eq!(out[off::TOUCH..off::TOUCH + 2], work.touch[0].to_le_bytes());
    assert_eq!(out[off::CLASSES], work.classes[0]);
    assert_eq!(out[off::FLAGS], work.flags[0]);
    assert_eq!(
        out[off::CANCELLED..off::CANCELLED + 8],
        work.cancelled[0].to_le_bytes()
    );
    assert_eq!(
        out[off::KEYS..off::KEYS + 16],
        work.keys[0].remainder.to_le_bytes()
    );
    assert_eq!(out[off::KEYS_POOL], work.keys[0].pool);
    assert_eq!(out[off::KEYS_EXTRA], work.keys[0].extra as u8);
    assert_eq!(
        out[off::SCRATCH_BUY..off::SCRATCH_BUY + 8],
        work.scratch_buy[0][0].to_le_bytes()
    );
    assert_eq!(
        out[off::SCRATCH_SELL..off::SCRATCH_SELL + 8],
        work.scratch_sell[0][0].to_le_bytes()
    );
    assert_eq!(
        out[off::CELL_PORTFOLIO..off::CELL_PORTFOLIO + 2],
        work.cell_portfolio[0].to_le_bytes()
    );
    assert_eq!(
        out[off::FLOW_BUY..off::FLOW_BUY + 16],
        work.flow_buy[0].to_le_bytes()
    );
    assert_eq!(
        out[off::FLOW_SELL..off::FLOW_SELL + 16],
        work.flow_sell[0].to_le_bytes()
    );
    assert_eq!(
        out[off::PART_BUY..off::PART_BUY + 8],
        work.part_buy[0][0].to_le_bytes()
    );
    assert_eq!(
        out[off::PART_SELL..off::PART_SELL + 8],
        work.part_sell[0][0].to_le_bytes()
    );
    assert_eq!(
        out[off::AGG..off::AGG + 16],
        work.agg[0].demand.to_le_bytes()
    );
    assert_eq!(
        out[off::POOLS..off::POOLS + 16],
        work.pools[0].total.to_le_bytes()
    );
    assert_eq!(out[off::POOLS_READY], work.pools[0].ready as u8);
    assert_eq!(
        out[off::RESERVED_UNITS..off::RESERVED_UNITS + 16],
        work.reserved_units[0].to_le_bytes()
    );
    assert_eq!(
        out[off::LEDGER_EGG..off::LEDGER_EGG + 8],
        work.opening_reserved_egg[0].to_le_bytes()
    );
    assert_eq!(
        out[off::CASH_SCALARS..off::CASH_SCALARS + 16],
        work.opening_reserved_cash.to_le_bytes()
    );
    assert_eq!(
        out[off::SPLIT_USED..off::SPLIT_USED + 8],
        work.split_used[0].to_le_bytes()
    );
    assert_eq!(out[off::SUMMARY], work.summary.outcome_count);
    assert_eq!(out[off::SUMMARY_VALID], work.summary_valid as u8);
}

#[test]
fn encoding_round_trips_at_representative_states() {
    let plain = domain_with(base_policy(), 2, 3);
    let netting = domain_with(
        FrozenPolicyV1 {
            self_cross: SelfCrossPolicyV1::NetAtAdmission,
            ..base_policy()
        },
        2,
        2,
    );
    let explicit = domain_with(
        FrozenPolicyV1 {
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            ..base_policy()
        },
        2,
        2,
    );
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let book = four_book();
    let candidate = canonical_candidate(&plain, &book, &vector, 0, 0).unwrap();
    let cross = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE),
        single(2, 1, 0, Side::Sell, 4, 0),
    ]);
    let sliced = canonical_candidate(&explicit, &cross, &vector, 0, 0).unwrap();
    let witness = canonical_pairing(&explicit, &cross, &sliced).unwrap();
    let self_cross_book = book_of(&[
        single(1, 0, 0, Side::Buy, 3, SCALE),
        single(2, 0, 0, Side::Sell, 2, 0),
        single(3, 1, 0, Side::Sell, 1, 0),
    ]);
    let netted = canonical_candidate(&netting, &self_cross_book, &vector, 0, 0).unwrap();

    // Every state a resumable walk can be saved at, plus the terminal ones.
    let mut states: Vec<Box<ClearWorkV1>> = Vec::new();
    // Idle.
    states.push(Box::new(ClearWorkV1::new()));
    // Post-begin.
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&plain, &header_of(&candidate, None), true)
        .unwrap();
    states.push(work);
    // Mid-pass-1.
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&plain, &header_of(&candidate, None), true)
        .unwrap();
    work.push_order(&book.orders[0], candidate.fills[0])
        .unwrap();
    work.push_order(&book.orders[1], candidate.fills[1])
        .unwrap();
    states.push(work);
    // Sealed pass-1.
    let mut work = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work, &plain, &book, &candidate);
    states.push(work);
    // Mid-pass-2.
    let mut work = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work, &plain, &book, &candidate);
    work.push_order(&book.orders[0], candidate.fills[0])
        .unwrap();
    states.push(work);
    // Complete, accepted.
    let mut work = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work, &plain, &book, &candidate);
    resume_to_completion(&mut work, &book, &candidate).unwrap();
    assert!(matches!(work.verdict(), Some(Ok(_))));
    states.push(work);
    // Complete, refused (a forged fill).
    let mut forged = candidate;
    forged.fills[0] = forged.fills[0].wrapping_add(1);
    let mut work = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work, &plain, &book, &forged);
    resume_to_completion(&mut work, &book, &forged).unwrap();
    assert!(matches!(work.verdict(), Some(Err(_))));
    states.push(work);
    // Complete at begin (a refused domain, carried verbatim).
    let mut bad_domain = plain;
    bad_domain.relation_version = 9;
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&bad_domain, &header_of(&candidate, None), true)
        .unwrap();
    assert_eq!(work.status(), FeedStatusV1::Complete);
    states.push(work);
    // Poisoned by a tampered resumption.
    let mut work = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work, &plain, &book, &candidate);
    work.push_order(&book.orders[0], candidate.fills[0].wrapping_add(1))
        .unwrap();
    let mut j = 1usize;
    while j < book.len as usize {
        work.push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
        j += 1;
    }
    assert_eq!(work.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
    states.push(work);
    // N-b netting: sealed pass-1 (netting totals live in scratch) and pass-3.
    let mut work = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work, &netting, &self_cross_book, &netted);
    states.push(work);
    // Explicit slices: at the slice phase and mid-slices.
    let mut work = Box::new(ClearWorkV1::new());
    let header = header_of(&sliced, Some(&witness));
    work.begin(&explicit, &header, true).unwrap();
    let mut j = 0usize;
    while j < cross.len as usize {
        work.push_order(&cross.orders[j], sliced.fills[j]).unwrap();
        j += 1;
    }
    work.end_pass().unwrap();
    assert_eq!(work.status(), FeedStatusV1::NeedSlices);
    let mut mid = work.clone();
    states.push(work);
    if witness.len > 0 {
        mid.push_slice(&witness.slices[0]).unwrap();
    }
    states.push(mid);
    // Unchecked-claims mode.
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&plain, &header_of(&candidate, None), false)
        .unwrap();
    states.push(work);

    let mut out = buffer();
    let mut second = buffer();
    for (index, state) in states.iter().enumerate() {
        state.encode_into(&mut out).unwrap();
        let mut decoded = Box::new(ClearWorkV1::new());
        decoded.decode_into(&out).unwrap();
        assert_eq!(&*decoded, &**state, "state {index} did not round-trip");
        decoded.encode_into(&mut second).unwrap();
        assert_eq!(out, second, "state {index} re-encoded differently");
    }
}

#[test]
fn encode_idle_writes_the_idle_checkpoint() {
    let mut from_static = buffer();
    ClearWorkV1::encode_idle_into(&mut from_static).unwrap();
    let mut from_value = buffer();
    ClearWorkV1::new().encode_into(&mut from_value).unwrap();
    assert_eq!(from_static, from_value);
    let mut decoded = Box::new(ClearWorkV1::new());
    decoded.decode_into(&from_static).unwrap();
    assert_eq!(*decoded, ClearWorkV1::NEW);
    // The idle encoding is not all-zero: the fold IVs and the canonical
    // ineligible class are nonzero, so a zeroed account is not an idle body.
    assert!(from_static.iter().any(|byte| *byte != 0));
}

#[test]
fn wrong_length_is_refused_on_both_sides() {
    let work = Box::new(ClearWorkV1::new());
    let mut short = vec![0u8; ClearWorkV1::ENCODED_BYTES - 1];
    let mut long = vec![0u8; ClearWorkV1::ENCODED_BYTES + 1];
    assert_eq!(work.encode_into(&mut short), Err(CodecFaultV1::WrongLength));
    assert_eq!(work.encode_into(&mut long), Err(CodecFaultV1::WrongLength));
    assert_eq!(
        ClearWorkV1::encode_idle_into(&mut short),
        Err(CodecFaultV1::WrongLength)
    );
    let mut target = Box::new(ClearWorkV1::new());
    assert_eq!(target.decode_into(&short), Err(CodecFaultV1::WrongLength));
    assert_eq!(target.decode_into(&long), Err(CodecFaultV1::WrongLength));
    assert_eq!(
        encode_policy_v1(&base_policy(), &mut [0u8; POLICY_ENCODED_BYTES - 1]),
        Err(CodecFaultV1::WrongLength)
    );
    assert_eq!(
        decode_policy_v1(&[0u8; POLICY_ENCODED_BYTES + 1]),
        Err(CodecFaultV1::WrongLength)
    );
}

/* ------------------------------------------------------------------------ */
/* Hostile-byte totality                                                     */
/* ------------------------------------------------------------------------ */

#[test]
fn every_flipped_byte_decodes_totally_and_canonically() {
    // Two bases: an active sealed walk and a completed verdict.  Every single
    // byte of each is flipped; the decoder must either refuse with a typed
    // fault or accept — and whatever it accepts must re-encode to the very
    // bytes it read, so the accepted set is closed and canonical.
    let domain = domain_with(base_policy(), 2, 3);
    let book = four_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let mut sealed = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut sealed, &domain, &book, &candidate);
    let mut complete = sealed.clone();
    resume_to_completion(&mut complete, &book, &candidate).unwrap();

    let mut accepted = 0u32;
    let mut refused = 0u32;
    let mut out = buffer();
    let mut reencoded = buffer();
    let mut target = Box::new(ClearWorkV1::new());
    for base in [&sealed, &complete] {
        base.encode_into(&mut out).unwrap();
        for at in 0..ClearWorkV1::ENCODED_BYTES {
            // Every byte inverted; the control plane and the per-order tables
            // additionally get the low-bit flip, which lands *inside* the
            // registered ranges and must therefore be accepted verbatim.
            let low_bit_too = at < off::SCRATCH_BUY;
            for pattern in [Some(0xFFu8), if low_bit_too { Some(0x01) } else { None }]
                .into_iter()
                .flatten()
            {
                let original = out[at];
                out[at] ^= pattern;
                match target.decode_into(&out) {
                    Ok(()) => {
                        accepted += 1;
                        target.encode_into(&mut reencoded).unwrap();
                        assert_eq!(
                            out, reencoded,
                            "an accepted flip at byte {at} re-encoded differently"
                        );
                    }
                    Err(_) => {
                        refused += 1;
                        // A refused decode resets to idle, never half-writes.
                        assert_eq!(*target, ClearWorkV1::NEW);
                    }
                }
                out[at] = original;
            }
        }
    }
    assert!(
        accepted > 90_000,
        "flips must mostly land in plain data: {accepted}"
    );
    assert!(
        refused > 100,
        "flips must hit the control planes: {refused}"
    );
}

#[test]
fn hostile_byte_patterns_never_panic() {
    let mut target = Box::new(ClearWorkV1::new());
    // Constant fills.
    for pattern in [0x00u8, 0x01, 0x7F, 0xA5, 0xFF] {
        let bytes = vec![pattern; ClearWorkV1::ENCODED_BYTES];
        let _ = target.decode_into(&bytes);
    }
    // An all-ones body cannot be a checkpoint: the phase byte alone refuses.
    let ones = vec![0xFFu8; ClearWorkV1::ENCODED_BYTES];
    assert_eq!(target.decode_into(&ones), Err(CodecFaultV1::InvalidPhase));
    // A deterministic pseudo-random sweep.
    let mut bytes = buffer();
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..64 {
        for byte in bytes.iter_mut() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *byte = (state >> 56) as u8;
        }
        let _ = target.decode_into(&bytes);
    }
}

#[test]
fn control_field_sweeps_refuse_with_typed_faults() {
    let domain = domain_with(base_policy(), 2, 3);
    let book = four_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let mut sealed = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut sealed, &domain, &book, &candidate);
    let mut complete = sealed.clone();
    resume_to_completion(&mut complete, &book, &candidate).unwrap();
    let mut active_bytes = buffer();
    sealed.encode_into(&mut active_bytes).unwrap();
    let mut complete_bytes = buffer();
    complete.encode_into(&mut complete_bytes).unwrap();
    let mut target = Box::new(ClearWorkV1::new());

    fn sweep(
        target: &mut ClearWorkV1,
        bytes: &mut [u8],
        at: usize,
        ok: &dyn Fn(u8) -> bool,
        fault: CodecFaultV1,
        label: &str,
    ) {
        let original = bytes[at];
        for value in 0..=255u8 {
            bytes[at] = value;
            let outcome = target.decode_into(bytes);
            if ok(value) {
                assert_eq!(outcome, Ok(()), "{label}: {value} must decode");
            } else {
                assert_eq!(outcome, Err(fault), "{label}: {value} must refuse");
            }
        }
        bytes[at] = original;
    }

    // Phase, on both bases.
    sweep(
        &mut target,
        &mut complete_bytes,
        off::PHASE,
        &|v| v <= 4,
        CodecFaultV1::InvalidPhase,
        "phase (complete)",
    );
    // The five top-level booleans.
    for (at, label) in [
        (off::SLICES_EXPECTED, "slices_expected"),
        (off::CHECK_CLAIMS, "check_claims"),
        (off::LATCH_SET, "latch_set"),
        (off::SUMMARY_VALID, "summary_valid"),
        (off::POOLS_READY, "pools[0].ready"),
        (off::KEYS_EXTRA, "keys[0].extra"),
    ] {
        sweep(
            &mut target,
            &mut active_bytes,
            at,
            &|v| v <= 1,
            CodecFaultV1::InvalidBool,
            label,
        );
    }
    // The latch-error registry: 48 registered refusals, the last of them V1b's
    // `PriceOutsideMomentCone` at the append-only code 47.
    sweep(
        &mut target,
        &mut active_bytes,
        off::LATCH_ERROR,
        &|v| v < 48,
        CodecFaultV1::InvalidErrorCode,
        "latch_error code",
    );
    // A payload behind a payload-free refusal.
    {
        let original = active_bytes[off::LATCH_ERROR + 1];
        active_bytes[off::LATCH_ERROR + 1] = 3;
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidErrorCode)
        );
        // The same payload under either payload-bearing code decodes.
        let code = active_bytes[off::LATCH_ERROR];
        active_bytes[off::LATCH_ERROR] = 30;
        target.decode_into(&active_bytes).unwrap();
        assert_eq!(
            target.latch_error,
            ErrorV1::PairingInfeasible {
                outcome: 3,
                owner: 0
            }
        );
        active_bytes[off::LATCH_ERROR] = 47;
        target.decode_into(&active_bytes).unwrap();
        assert_eq!(
            target.latch_error,
            ErrorV1::PriceOutsideMomentCone { outcome: 3 }
        );
        // V1b's refusal owns the outcome lane only: an owner behind it is
        // still non-canonical.
        let owner_lane = active_bytes[off::LATCH_ERROR + 2];
        active_bytes[off::LATCH_ERROR + 2] = 1;
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidErrorCode)
        );
        active_bytes[off::LATCH_ERROR + 2] = owner_lane;
        active_bytes[off::LATCH_ERROR] = code;
        active_bytes[off::LATCH_ERROR + 1] = original;
    }
    // The ten policy selectors, at their registered radices.
    for (selector, radix, label) in [
        (0usize, 2u8, "allocation"),
        (1, 3, "self_cross"),
        (2, 3, "aon"),
        (3, 3, "rounding"),
        (4, 4, "residual_settlement"),
        (5, 2, "transfer_phase"),
        (6, 2, "portfolio_lots"),
        (7, 2, "pairing_witness"),
        (8, 2, "dust"),
        (9, 1, "score"),
    ] {
        sweep(
            &mut target,
            &mut active_bytes,
            off::DOMAIN_POLICY + selector,
            &|v| v < radix,
            CodecFaultV1::InvalidPolicy,
            label,
        );
    }
    // The fee discriminant; a fee payload behind the no-fee tag refuses.
    // Tag 2 (the composite shape) decodes only with the rate word zero.
    sweep(
        &mut target,
        &mut active_bytes,
        off::DOMAIN_POLICY + 10,
        &|v| v <= 2,
        CodecFaultV1::InvalidPolicy,
        "fee tag",
    );
    {
        let at = off::DOMAIN_POLICY + 11;
        active_bytes[at] = 1;
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidPolicy),
            "bps behind FeeBaseV1::None"
        );
        active_bytes[off::DOMAIN_POLICY + 10] = 1;
        target.decode_into(&active_bytes).unwrap();
        assert_eq!(
            target.domain.policy.fee_base,
            FeeBaseV1::FlatNotional { bps: 1 }
        );
        // The composite packs both rates into the one rate word: dispersion
        // low, floor high.
        active_bytes[off::DOMAIN_POLICY + 10] = 2;
        target.decode_into(&active_bytes).unwrap();
        assert_eq!(
            target.domain.policy.fee_base,
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps: 1,
                floor_range_bps: 0,
            },
            "the dispersion rate rides the low half"
        );
        active_bytes[at] = 0;
        active_bytes[at + 2] = 1;
        target.decode_into(&active_bytes).unwrap();
        assert_eq!(
            target.domain.policy.fee_base,
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps: 0,
                floor_range_bps: 1,
            },
            "the floor rate rides the high half"
        );
        // A rate past `FEE_BPS_DENOMINATOR` in either half is fail-closed
        // corruption, never a policy.
        active_bytes[at + 2] = 0;
        for half in [0usize, 2] {
            active_bytes[at + half] = 0x11;
            active_bytes[at + half + 1] = 0x27;
            assert_eq!(
                target.decode_into(&active_bytes),
                Err(CodecFaultV1::InvalidPolicy),
                "an over-denominator composite rate must refuse"
            );
            active_bytes[at + half] = 0;
            active_bytes[at + half + 1] = 0;
        }
        target.decode_into(&active_bytes).unwrap();
        assert_eq!(
            target.domain.policy.fee_base,
            FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps: 0,
                floor_range_bps: 0,
            }
        );
        active_bytes[off::DOMAIN_POLICY + 10] = 0;
    }
    // The declared-slices flag; a count behind a clear flag refuses.
    sweep(
        &mut target,
        &mut active_bytes,
        off::CAND_DECLARED,
        &|v| v <= 1,
        CodecFaultV1::InvalidSliceDeclaration,
        "declared_slices flag",
    );
    {
        active_bytes[off::CAND_DECLARED + 1] = 9;
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidSliceDeclaration)
        );
        active_bytes[off::CAND_DECLARED] = 1;
        target.decode_into(&active_bytes).unwrap();
        assert_eq!(target.cand.declared_slices, Some(9));
        active_bytes[off::CAND_DECLARED] = 0;
        active_bytes[off::CAND_DECLARED + 1] = 0;
    }
    // Classes and flags.
    sweep(
        &mut target,
        &mut active_bytes,
        off::CLASSES,
        &|v| v <= 2,
        CodecFaultV1::InvalidClass,
        "classes[0]",
    );
    sweep(
        &mut target,
        &mut active_bytes,
        off::FLAGS,
        &|v| v < 32,
        CodecFaultV1::InvalidFlags,
        "flags[0]",
    );
    // The pool index of a key row: a table index or the none sentinel.
    sweep(
        &mut target,
        &mut active_bytes,
        off::KEYS_POOL,
        &|v| v < 32 || v == 255,
        CodecFaultV1::InvalidSlot,
        "keys[0].pool",
    );
    // Owner slots: interned tags index 64-row tables.
    sweep(
        &mut target,
        &mut active_bytes,
        off::OWNER_SLOT,
        &|v| v < 64,
        CodecFaultV1::InvalidSlot,
        "owner_slot[0] low byte",
    );
    {
        active_bytes[off::OWNER_SLOT + 1] = 1;
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidSlot)
        );
        active_bytes[off::OWNER_SLOT + 1] = 0;
    }
    // Counts: cursor and order_count against MAX_ORDERS, the interning bound
    // against the consumed prefix, the active outcome count, the zero scale.
    sweep(
        &mut target,
        &mut active_bytes,
        off::CURSOR,
        &|v| v <= 64,
        CodecFaultV1::InvalidCount,
        "cursor (active, pass 2)",
    );
    // The sealed base interned 3 owners over 4 orders, so an order count
    // below the interned count is unreachable and refused.
    sweep(
        &mut target,
        &mut active_bytes,
        off::ORDER_COUNT,
        &|v| (3..=64).contains(&v),
        CodecFaultV1::InvalidCount,
        "order_count (active)",
    );
    sweep(
        &mut target,
        &mut active_bytes,
        off::OWNER_SLOTS,
        &|v| v <= 4,
        CodecFaultV1::InvalidCount,
        "owner_slots (active, order_count 4)",
    );
    sweep(
        &mut target,
        &mut active_bytes,
        off::DOMAIN_OUTCOME_COUNT,
        &|v| v <= 16,
        CodecFaultV1::InvalidCount,
        "outcome_count (active)",
    );
    // The same coordinates are representable on a completed checkpoint, where
    // nothing will ever index by them again: a begin-refused domain must
    // round-trip verbatim.
    sweep(
        &mut target,
        &mut complete_bytes,
        off::DOMAIN_OUTCOME_COUNT,
        &|_| true,
        CodecFaultV1::InvalidCount,
        "outcome_count (complete)",
    );
    {
        let at = off::DOMAIN_PRICE_SCALE;
        let mut original = [0u8; 8];
        original.copy_from_slice(&active_bytes[at..at + 8]);
        active_bytes[at..at + 8].fill(0);
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidCount),
            "a zero price scale on an active feed"
        );
        complete_bytes[at..at + 8].fill(0);
        target.decode_into(&complete_bytes).unwrap();
        active_bytes[at..at + 8].copy_from_slice(&original);
    }
    // Slice cursor at the MAX_SLICES boundary.
    {
        let at = off::SLICE_CURSOR;
        active_bytes[at..at + 2].copy_from_slice(&416u16.to_le_bytes());
        target.decode_into(&active_bytes).unwrap();
        active_bytes[at..at + 2].copy_from_slice(&417u16.to_le_bytes());
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidCount)
        );
        active_bytes[at..at + 2].copy_from_slice(&0u16.to_le_bytes());
    }
    // A ready pool with a target but no total would be the floor step's
    // division by zero; unreachable, so refused.
    {
        let pool_at = off::POOLS;
        let original = Vec::from(&active_bytes[pool_at..pool_at + 36]);
        active_bytes[pool_at..pool_at + 16].fill(0); // total = 0
        active_bytes[pool_at + 18..pool_at + 26].copy_from_slice(&5u64.to_le_bytes()); // target
        active_bytes[pool_at + 34] = 1; // ready
        assert_eq!(
            target.decode_into(&active_bytes),
            Err(CodecFaultV1::InvalidCount)
        );
        active_bytes[pool_at..pool_at + 36].copy_from_slice(&original);
    }
}

/* ------------------------------------------------------------------------ */
/* The three-layer tamper stack                                              */
/* ------------------------------------------------------------------------ */

/// The anchor the layout header carries: `bind_order_set` stamps
/// `(order_set, consumed_fold)` at pass-1 finalize, and the program compares
/// `body.consumed_fold() == header.consumed_fold` at every resume.  This is
/// the in-crate model of that comparison; the header's own refusals are
/// layer (b), tested in `clutch-solana-layout`.
struct Anchor {
    order_set: u64,
    consumed_fold: u128,
}

impl Anchor {
    fn stamp(domain: &RelationDomainV1, work: &ClearWorkV1) -> Self {
        Self {
            order_set: domain.order_set_id,
            consumed_fold: work.consumed_fold(),
        }
    }

    fn admits(&self, epoch_order_set: u64, body: &ClearWorkV1) -> bool {
        self.order_set == epoch_order_set && self.consumed_fold == body.consumed_fold()
    }
}

#[test]
fn fold_state_tamper_between_resume_steps_is_refused() {
    // Layer (a): body edits that touch the consumed-fold state refuse the
    // next pass with `ResumeFoldMismatch`/`TooManyPushes`, poison the
    // checkpoint, and never yield a verdict.
    let domain = domain_with(base_policy(), 2, 3);
    let book = four_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let mut sealed = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut sealed, &domain, &book, &candidate);
    let anchor = Anchor::stamp(&domain, &sealed);
    let mut saved = buffer();
    sealed.encode_into(&mut saved).unwrap();

    // A flipped sealed fold: the anchor comparison already refuses it, and
    // even a resumer that skipped the anchor is refused by the fold seal.
    let mut tampered = saved.clone();
    tampered[off::SEALED_FOLD] ^= 0x40;
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&tampered).unwrap();
    assert!(
        !anchor.admits(domain.order_set_id, &resumed),
        "layer (c) must catch a sealed-fold edit"
    );
    assert_eq!(
        resume_to_completion(&mut resumed, &book, &candidate),
        Err(FeedErrorV1::ResumeFoldMismatch)
    );
    assert_eq!(resumed.verdict(), None);
    assert_eq!(
        resumed.push_order(&book.orders[0], 0),
        Err(FeedErrorV1::NotInProgress),
        "the checkpoint must be poisoned"
    );

    // A flipped running fold, saved mid-pass-2.
    let mut mid = sealed.clone();
    mid.push_order(&book.orders[0], candidate.fills[0]).unwrap();
    let mut saved_mid = buffer();
    mid.encode_into(&mut saved_mid).unwrap();
    let mut tampered = saved_mid.clone();
    tampered[off::FOLD + 3] ^= 0x08;
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&tampered).unwrap();
    assert!(
        anchor.admits(domain.order_set_id, &resumed),
        "a running-fold edit is invisible to the anchor; the seal owns it"
    );
    let mut j = 1usize;
    while j < book.len as usize {
        resumed
            .push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
        j += 1;
    }
    assert_eq!(resumed.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
    assert_eq!(resumed.verdict(), None);

    // A grown order count: the pass-2 push count no longer matches.
    let mut tampered = saved.clone();
    tampered[off::ORDER_COUNT] = 5;
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&tampered).unwrap();
    assert_eq!(
        resume_to_completion(&mut resumed, &book, &candidate),
        Err(FeedErrorV1::ResumeFoldMismatch)
    );
    // A shrunk order count under the interned-owner floor refuses at decode.
    let mut tampered = saved.clone();
    tampered[off::ORDER_COUNT] = 2;
    let mut resumed = Box::new(ClearWorkV1::new());
    assert_eq!(
        resumed.decode_into(&tampered),
        Err(CodecFaultV1::InvalidCount)
    );

    // A pushed-forward cursor, saved mid-pass-2: the resumer's own pushes
    // overflow the pass.
    let mut tampered = saved_mid.clone();
    tampered[off::CURSOR] = 4;
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&tampered).unwrap();
    assert_eq!(
        resumed.push_order(&book.orders[1], candidate.fills[1]),
        Err(FeedErrorV1::TooManyPushes)
    );
    // A pulled-back cursor: the pass consumes too few and the seal refuses.
    let mut tampered = saved_mid.clone();
    tampered[off::CURSOR] = 0;
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&tampered).unwrap();
    let mut j = 1usize;
    while j < book.len as usize {
        resumed
            .push_order(&book.orders[j], candidate.fills[j])
            .unwrap();
        j += 1;
    }
    assert_eq!(resumed.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
}

#[test]
fn wholesale_substitution_is_refused_by_the_anchor() {
    // Layer (c): a body swapped for another *internally consistent*
    // checkpoint sails through the fold seal — its fold seals its own feed —
    // and is refused only by the `(order_set, consumed_fold)` anchor the
    // layout header stamped at pass-1 finalize.  This is why the program must
    // compare `body.consumed_fold() == header.consumed_fold` at every resume.
    let domain_a = domain_with(base_policy(), 2, 3);
    let mut domain_b = domain_with(base_policy(), 2, 3);
    domain_b.order_set_id = 99;
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let book_a = four_book();
    let book_b = book_of(&[
        single(1, 0, 0, Side::Buy, 4, SCALE),
        single(2, 1, 0, Side::Sell, 4, 0),
    ]);
    let candidate_a = canonical_candidate(&domain_a, &book_a, &vector, 0, 0).unwrap();
    let candidate_b = canonical_candidate(&domain_b, &book_b, &vector, 0, 0).unwrap();

    let mut work_a = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work_a, &domain_a, &book_a, &candidate_a);
    let anchor = Anchor::stamp(&domain_a, &work_a);

    let mut work_b = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut work_b, &domain_b, &book_b, &candidate_b);
    let mut substituted = buffer();
    work_b.encode_into(&mut substituted).unwrap();

    // The substituted body decodes and resumes cleanly against *its own*
    // feed: the fold seal cannot see the swap.
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&substituted).unwrap();
    assert!(
        !anchor.admits(domain_a.order_set_id, &resumed),
        "the anchor is the only layer that catches a wholesale substitution"
    );
    resume_to_completion(&mut resumed, &book_b, &candidate_b).unwrap();
    assert!(matches!(resumed.verdict(), Some(Ok(_))));

    // Cross-feeding the substituted body with the original sequence is the
    // fold seal's case again: the longer original pass overflows the
    // substituted count before the fold comparison could even run.
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&substituted).unwrap();
    assert!(matches!(
        resume_to_completion(&mut resumed, &book_a, &candidate_a),
        Err(FeedErrorV1::TooManyPushes | FeedErrorV1::ResumeFoldMismatch)
    ));

    // And the header half of the anchor: a resumed pass whose epoch shows a
    // different frozen order set is refused by the same comparison
    // (`require_continuation` is the layout's spelling of it).
    assert!(!anchor.admits(domain_b.order_set_id, &work_a));
    assert!(anchor.admits(domain_a.order_set_id, &work_a));
}

#[test]
fn every_body_region_mutation_lands_inside_the_documented_boundary() {
    // The tamper stack, region by region.  For each named region of the
    // encoding, one byte is edited between two on-chain-shaped resume steps;
    // the outcome must be the one this table documents.  `Caught` means a
    // decode refusal, the anchor comparison, or the fold seal fired; `Residual`
    // means the layers pass and the walk completes — the account-owner
    // threat-model residue the design § "checkpoint tamper refusal" names
    // (only the program itself can write the body; these layers exist for the
    // substitution seams, not for arbitrary-write adversaries).  A region
    // moving between buckets is a finding about the stack, so the table is
    // exact.
    #[derive(PartialEq, Debug, Clone, Copy)]
    enum Expect {
        Caught,
        Residual,
    }
    let regions: &[(&str, usize, Expect)] = &[
        ("phase", off::PHASE, Expect::Caught),
        ("pass", off::PASS, Expect::Caught),
        ("cursor", off::CURSOR, Expect::Caught),
        ("order_count", off::ORDER_COUNT, Expect::Caught),
        ("latch_set", off::LATCH_SET, Expect::Residual),
        ("fold", off::FOLD, Expect::Caught),
        ("sealed_fold", off::SEALED_FOLD, Expect::Caught),
        ("digest", off::DIGEST, Expect::Residual),
        ("previous_id", off::PREVIOUS_ID, Expect::Residual),
        ("domain", off::DOMAIN, Expect::Residual),
        ("candidate", off::CAND, Expect::Residual),
        ("owners", off::OWNERS, Expect::Residual),
        ("owner_slots", off::OWNER_SLOTS, Expect::Residual),
        ("owner_slot", off::OWNER_SLOT, Expect::Residual),
        ("side_buy_bits", off::SIDE_BUY_BITS, Expect::Residual),
        ("touch", off::TOUCH, Expect::Residual),
        ("classes", off::CLASSES, Expect::Residual),
        ("flags", off::FLAGS, Expect::Residual),
        ("cancelled", off::CANCELLED, Expect::Residual),
        ("keys", off::KEYS, Expect::Residual),
        ("scratch_buy", off::SCRATCH_BUY, Expect::Residual),
        ("scratch_sell", off::SCRATCH_SELL, Expect::Residual),
        ("cell_portfolio", off::CELL_PORTFOLIO, Expect::Residual),
        ("flow_buy", off::FLOW_BUY, Expect::Residual),
        ("flow_sell", off::FLOW_SELL, Expect::Residual),
        ("part_buy", off::PART_BUY, Expect::Residual),
        ("part_sell", off::PART_SELL, Expect::Residual),
        ("agg", off::AGG, Expect::Residual),
        ("pools", off::POOLS, Expect::Residual),
        ("reserved_units", off::RESERVED_UNITS, Expect::Residual),
        ("ledger_egg", off::LEDGER_EGG, Expect::Residual),
        ("cash_scalars", off::CASH_SCALARS, Expect::Residual),
        ("split_used", off::SPLIT_USED, Expect::Residual),
        ("summary", off::SUMMARY + 1, Expect::Residual),
        ("summary_valid", off::SUMMARY_VALID, Expect::Residual),
    ];

    let domain = domain_with(base_policy(), 2, 3);
    let book = four_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let mut sealed = Box::new(ClearWorkV1::new());
    seal_pass_one(&mut sealed, &domain, &book, &candidate);
    let anchor = Anchor::stamp(&domain, &sealed);
    let mut saved = buffer();
    sealed.encode_into(&mut saved).unwrap();

    for (name, at, expect) in regions {
        let mut tampered = saved.clone();
        tampered[*at] ^= 0x01;
        let mut resumed = Box::new(ClearWorkV1::new());
        // Caught means one of the layers fired — the codec's typed refusal,
        // the anchor comparison, the feed protocol — or the walk ended with
        // no verdict at all (a walk that never yields a verdict settles
        // nothing).  Residual means the layers passed and a verdict exists.
        let caught = match resumed.decode_into(&tampered) {
            Err(_) => true,
            Ok(()) => {
                if !anchor.admits(domain.order_set_id, &resumed) {
                    true
                } else {
                    resume_to_completion(&mut resumed, &book, &candidate).is_err()
                        || resumed.verdict().is_none()
                }
            }
        };
        let outcome = if caught {
            Expect::Caught
        } else {
            Expect::Residual
        };
        assert_eq!(outcome, *expect, "region {name}: the tamper boundary moved");
    }
}

/* ------------------------------------------------------------------------ */
/* The policy sub-codec                                                      */
/* ------------------------------------------------------------------------ */

#[test]
fn policy_codec_round_trips_every_registered_family() {
    // The full selector product; the byte-for-byte comparison against
    // `clutch-batch-policy-identity`'s artifact lives in that crate, which
    // can see both encoders.
    let allocations = [
        AllocationPolicyV1::PricePriorityMarginalProRata,
        AllocationPolicyV1::FullProRata,
    ];
    let self_crosses = [
        SelfCrossPolicyV1::RefuseOverlap,
        SelfCrossPolicyV1::NetAtAdmission,
        SelfCrossPolicyV1::AllowGateAtPairing,
    ];
    let aons = [
        AonPolicyV1::RefuseAdmission,
        AonPolicyV1::WitnessedHonoredMask,
        AonPolicyV1::FullSizeCounting,
    ];
    let roundings = [
        RoundingBoundaryV1::None,
        RoundingBoundaryV1::TerminalOwnerFloor,
        RoundingBoundaryV1::ReceiptFloor,
    ];
    let residuals = [
        ResidualSettlementV1::FullPairOnly,
        ResidualSettlementV1::CumulativePairCanonical,
        ResidualSettlementV1::CumulativePairFree,
        ResidualSettlementV1::UniqueSliceReceipts,
    ];
    let transfers = [
        TransferPhaseV1::ActiveOnly,
        TransferPhaseV1::ActiveOrResolved,
    ];
    let lots = [
        PortfolioLotPolicyV1::StrictWholeOrder,
        PortfolioLotPolicyV1::MarginalProRataLots,
    ];
    let witnesses = [
        PairingWitnessPolicyV1::RecomputedConstructor,
        PairingWitnessPolicyV1::ExplicitSlices,
    ];
    let dusts = [DustPolicy::AssignCanonical, DustPolicy::Reject];
    let fees = [
        FeeBaseV1::None,
        FeeBaseV1::FlatNotional { bps: 0 },
        FeeBaseV1::FlatNotional { bps: 30 },
        FeeBaseV1::FlatNotional { bps: 10_000 },
    ];
    let mut count = 0u32;
    let mut bytes = [0u8; POLICY_ENCODED_BYTES];
    for allocation in allocations {
        for self_cross in self_crosses {
            for aon in aons {
                for rounding in roundings {
                    for residual_settlement in residuals {
                        for transfer_phase in transfers {
                            for portfolio_lots in lots {
                                for pairing_witness in witnesses {
                                    for dust in dusts {
                                        for fee_base in fees {
                                            let policy = FrozenPolicyV1 {
                                                allocation,
                                                self_cross,
                                                aon,
                                                rounding,
                                                residual_settlement,
                                                transfer_phase,
                                                portfolio_lots,
                                                pairing_witness,
                                                dust,
                                                score: ScorePolicyV1::LexicographicDispersionV1,
                                                fee_base,
                                            };
                                            encode_policy_v1(&policy, &mut bytes).unwrap();
                                            assert_eq!(decode_policy_v1(&bytes), Ok(policy));
                                            count += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 2 * 3 * 3 * 3 * 4 * 2 * 2 * 2 * 2 * 4);
    // Registered-but-refused selections still round-trip: a checkpoint that
    // latched their refusal carries them.
    let over = FrozenPolicyV1 {
        fee_base: FeeBaseV1::FlatNotional { bps: 50_000 },
        ..base_policy()
    };
    encode_policy_v1(&over, &mut bytes).unwrap();
    assert_eq!(decode_policy_v1(&bytes), Ok(over));
}

/// The resumable driver's public cursors: idle/poisoned discrimination, the
/// per-pass order cursor, and the slice cursor, all surviving encode/decode.
#[test]
fn driver_cursors_report_the_exact_resume_position() {
    let idle = Box::new(ClearWorkV1::new());
    assert!(idle.is_idle());
    assert!(!idle.is_poisoned());
    assert_eq!(idle.orders_consumed(), 0);
    assert_eq!(idle.slices_consumed(), 0);

    let domain = domain_with(base_policy(), 2, 3);
    let book = four_book();
    let vector = prices(&[SCALE / 2, SCALE / 2]);
    let candidate = canonical_candidate(&domain, &book, &vector, 0, 0).unwrap();
    let mut work = Box::new(ClearWorkV1::new());
    work.begin(&domain, &header_of(&candidate, None), true)
        .unwrap();
    assert!(!work.is_idle());
    let mut fed = 0u16;
    while fed < book.len as u16 {
        assert_eq!(work.orders_consumed(), fed);
        // The cursor is exactly what a resumed driver reads back off bytes.
        let mut out = buffer();
        work.encode_into(&mut out).unwrap();
        let mut resumed = Box::new(ClearWorkV1::new());
        resumed.decode_into(&out).unwrap();
        assert_eq!(resumed.orders_consumed(), fed);
        assert!(!resumed.is_idle());
        work.push_order(&book.orders[fed as usize], candidate.fills[fed as usize])
            .unwrap();
        fed += 1;
    }
    work.end_pass().unwrap();
    // A fresh pass restarts the order cursor.
    assert_eq!(work.orders_consumed(), 0);

    // A mismatched resumption poisons, and the poison round-trips.
    work.push_order(&book.orders[0], candidate.fills[0] + 1)
        .unwrap();
    let mut i = 1;
    while i < book.len as usize {
        work.push_order(&book.orders[i], candidate.fills[i])
            .unwrap();
        i += 1;
    }
    assert_eq!(work.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
    assert!(work.is_poisoned());
    let mut out = buffer();
    work.encode_into(&mut out).unwrap();
    let mut resumed = Box::new(ClearWorkV1::new());
    resumed.decode_into(&out).unwrap();
    assert!(resumed.is_poisoned());
    assert!(!resumed.is_idle());
}
