//! Isolated model of an active-width successor to `ClearWorkV1`.
//!
//! This crate does not participate in the deployable SBF graph.  It projects
//! the existing canonical V1 wire image into an exact-width payload and can
//! reconstruct the original V1 bytes byte-for-byte.  Reconstruction is then
//! decoded by the owning V1 codec; V2 therefore inherits the current typed
//! hostile-byte refusals instead of inventing a weaker validator.

use clutch_batch::relation_v1::{MAX_OUTCOMES, MAX_OWNER_SLOTS};
use clutch_batch::relation_v1_stream::{ClearWorkV1, CodecFaultV1};
use clutch_batch::MAX_ORDERS;

pub const V1_BODY_BYTES: usize = ClearWorkV1::ENCODED_BYTES;
pub const V1_ACCOUNT_BYTES: usize = 50_054;
pub const V1_LAYOUT_HEADER_BYTES: usize = 158;
pub const V1_INTERNER_BYTES: usize = 2 + 32 * MAX_OWNER_SLOTS;
pub const ACCOUNT_STORAGE_OVERHEAD_BYTES: usize = 128;
pub const RENT_LAMPORTS_PER_BYTE: u64 = 6_960;

const MAGIC: [u8; 8] = *b"DC-CWV2\0";
const MODEL_PREFIX_BYTES: usize = 16;

const CONTROL_AT: usize = 0;
const CONTROL_BYTES: usize = 82;
const DOMAIN_BYTES: usize = 78;
const CAND_AT: usize = 160;
const CAND_PRICES_AT: usize = CAND_AT + 1;
const CAND_PRICES_BYTES: usize = MAX_OUTCOMES * 8;
const CAND_TAIL_AT: usize = CAND_PRICES_AT + CAND_PRICES_BYTES;
const CAND_TAIL_BYTES: usize = 101;
const OWNERS_AT: usize = CAND_TAIL_AT + CAND_TAIL_BYTES;
const OWNER_SLOTS_AT: usize = OWNERS_AT + MAX_OWNER_SLOTS * 2;
const OWNER_SLOT_AT: usize = OWNER_SLOTS_AT + 2;
const SIDE_BUY_BITS_AT: usize = OWNER_SLOT_AT + MAX_ORDERS * 2;
const TOUCH_AT: usize = SIDE_BUY_BITS_AT + 8;
const CLASSES_AT: usize = TOUCH_AT + MAX_ORDERS * 2;
const FLAGS_AT: usize = CLASSES_AT + MAX_ORDERS;
const CANCELLED_AT: usize = FLAGS_AT + MAX_ORDERS;
const KEYS_AT: usize = CANCELLED_AT + MAX_ORDERS * 8;
const SCRATCH_BUY_AT: usize = KEYS_AT + MAX_ORDERS * 59;
const SCRATCH_SELL_AT: usize = SCRATCH_BUY_AT + MAX_ORDERS * MAX_OUTCOMES * 8;
const CELL_PORTFOLIO_AT: usize = SCRATCH_SELL_AT + MAX_ORDERS * MAX_OUTCOMES * 8;
const FLOW_BUY_AT: usize = CELL_PORTFOLIO_AT + MAX_OWNER_SLOTS * 2;
const FLOW_SELL_AT: usize = FLOW_BUY_AT + MAX_OUTCOMES * 16;
const PART_BUY_AT: usize = FLOW_SELL_AT + MAX_OUTCOMES * 16;
const PART_SELL_AT: usize = PART_BUY_AT + MAX_OWNER_SLOTS * MAX_OUTCOMES * 8;
const AGG_AT: usize = PART_SELL_AT + MAX_OWNER_SLOTS * MAX_OUTCOMES * 8;
const POOLS_AT: usize = AGG_AT + MAX_OUTCOMES * 128;
const RESERVED_UNITS_AT: usize = POOLS_AT + 2 * MAX_OUTCOMES * 36;
const LEDGER_EGG_AT: usize = RESERVED_UNITS_AT + 4 * MAX_OWNER_SLOTS * 16;
const CASH_SCALARS_AT: usize = LEDGER_EGG_AT + 3 * MAX_OUTCOMES * 8;
const SPLIT_USED_AT: usize = CASH_SCALARS_AT + 8 * 16;
const SUMMARY_AT: usize = SPLIT_USED_AT + 2 * MAX_OUTCOMES * 8;
const SUMMARY_VALID_AT: usize = SUMMARY_AT + 1_173;

const _: () = assert!(SUMMARY_VALID_AT + 1 == V1_BODY_BYTES);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Widths {
    pub outcomes: u8,
    pub orders: u8,
    pub owners: u8,
}

impl Widths {
    pub const fn new(outcomes: u8, orders: u8, owners: u8) -> Self {
        Self {
            outcomes,
            orders,
            owners,
        }
    }

    pub fn validate(self) -> Result<Self, Fault> {
        if self.outcomes == 0
            || self.outcomes as usize > MAX_OUTCOMES
            || self.orders as usize > MAX_ORDERS
            || self.owners as usize > MAX_OWNER_SLOTS
            || self.owners > self.orders
        {
            return Err(Fault::InvalidWidths);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    WrongLength,
    InvalidMagic,
    InvalidWidths,
    WidthBindingMismatch,
    NonCanonicalOmittedPadding,
    V1Codec(CodecFaultV1),
}

/// Exact active-width body length, excluding the layout-owned account header
/// and owner interner.
pub const fn compact_body_len(widths: Widths) -> usize {
    let n = widths.orders as usize;
    let u = widths.owners as usize;
    let o = widths.outcomes as usize;
    678 + 73 * n + 68 * u + 336 * o + 16 * n * o + 16 * u * o
}

/// Projected live account length.  V2 reuses the existing 158-byte identity,
/// cursor, status, and fold header and makes the interner exact-width.
pub const fn account_len(widths: Widths) -> usize {
    V1_LAYOUT_HEADER_BYTES + 2 + 32 * widths.owners as usize + compact_body_len(widths)
}

pub const fn minimum_balance(widths: Widths) -> u64 {
    (account_len(widths) as u64 + ACCOUNT_STORAGE_OVERHEAD_BYTES as u64) * RENT_LAMPORTS_PER_BYTE
}

/// Research-only self-describing prefix.  A live V2 account does not need
/// these 16 bytes: its version is in the outer header and the three widths are
/// authenticated against the frozen Epoch/CandidateFeed before body decode.
pub const fn model_image_len(widths: Widths) -> usize {
    MODEL_PREFIX_BYTES + compact_body_len(widths)
}

pub fn encode_from_v1(v1: &[u8], widths: Widths) -> Result<Vec<u8>, Fault> {
    widths.validate()?;
    if v1.len() != V1_BODY_BYTES {
        return Err(Fault::WrongLength);
    }
    let mut payload = Vec::with_capacity(compact_body_len(widths));
    project_payload(v1, widths, &mut payload);
    if payload.len() != compact_body_len(widths) {
        return Err(Fault::WrongLength);
    }

    // Omitted bytes are not "don't care".  They must be the canonical V1
    // padding image or the projection would merge distinguishable states.
    let reconstructed = expand_payload(&payload, widths)?;
    if reconstructed != v1 {
        return Err(Fault::NonCanonicalOmittedPadding);
    }

    let mut out = Vec::with_capacity(model_image_len(widths));
    out.extend_from_slice(&MAGIC);
    out.push(widths.outcomes);
    out.push(widths.orders);
    out.push(widths.owners);
    out.push(0);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn decode_to_v1(input: &[u8], expected: Widths) -> Result<Vec<u8>, Fault> {
    expected.validate()?;
    if input.len() < MODEL_PREFIX_BYTES || input[..8] != MAGIC {
        return Err(if input.len() < MODEL_PREFIX_BYTES {
            Fault::WrongLength
        } else {
            Fault::InvalidMagic
        });
    }
    let encoded = Widths::new(input[8], input[9], input[10]).validate()?;
    if input[11] != 0 {
        return Err(Fault::InvalidWidths);
    }
    if encoded != expected {
        return Err(Fault::WidthBindingMismatch);
    }
    let declared = u32::from_le_bytes([input[12], input[13], input[14], input[15]]) as usize;
    if declared != compact_body_len(encoded) || input.len() != MODEL_PREFIX_BYTES + declared {
        return Err(Fault::WrongLength);
    }
    expand_payload(&input[MODEL_PREFIX_BYTES..], encoded)
}

pub fn decode_into_v1(
    input: &[u8],
    expected: Widths,
    target: &mut ClearWorkV1,
) -> Result<(), Fault> {
    let expanded = decode_to_v1(input, expected)?;
    target.decode_into(&expanded).map_err(Fault::V1Codec)
}

fn append(out: &mut Vec<u8>, source: &[u8], at: usize, len: usize) {
    out.extend_from_slice(&source[at..at + len]);
}

fn append_matrix(
    out: &mut Vec<u8>,
    source: &[u8],
    at: usize,
    rows: usize,
    cols: usize,
    cell: usize,
) {
    let stride = MAX_OUTCOMES * cell;
    for row in 0..rows {
        append(out, source, at + row * stride, cols * cell);
    }
}

fn project_payload(v1: &[u8], widths: Widths, out: &mut Vec<u8>) {
    let n = widths.orders as usize;
    let u = widths.owners as usize;
    let o = widths.outcomes as usize;

    // Region 0: control, frozen coordinates, candidate, relation owner tags.
    append(out, v1, CONTROL_AT, CONTROL_BYTES + DOMAIN_BYTES);
    append(out, v1, CAND_AT, 1);
    append(out, v1, CAND_PRICES_AT, o * 8);
    append(out, v1, CAND_TAIL_AT, CAND_TAIL_BYTES);
    append(out, v1, OWNERS_AT, u * 2);
    append(out, v1, OWNER_SLOTS_AT, 2);

    // Region 1: per-order state in the existing structure-of-arrays order.
    append(out, v1, OWNER_SLOT_AT, n * 2);
    append(out, v1, SIDE_BUY_BITS_AT, 8);
    append(out, v1, TOUCH_AT, n * 2);
    append(out, v1, CLASSES_AT, n);
    append(out, v1, FLAGS_AT, n);
    append(out, v1, CANCELLED_AT, n * 8);
    append(out, v1, KEYS_AT, n * 59);

    // Region 2: self-cross scratch.
    append_matrix(out, v1, SCRATCH_BUY_AT, n, o, 8);
    append_matrix(out, v1, SCRATCH_SELL_AT, n, o, 8);
    append(out, v1, CELL_PORTFOLIO_AT, u * 2);

    // Region 3: outcome flows and owner participation.
    append(out, v1, FLOW_BUY_AT, o * 16);
    append(out, v1, FLOW_SELL_AT, o * 16);
    append_matrix(out, v1, PART_BUY_AT, u, o, 8);
    append_matrix(out, v1, PART_SELL_AT, u, o, 8);

    // Region 4: V3 aggregate and pool state.
    append(out, v1, AGG_AT, o * 128);
    append(out, v1, POOLS_AT, 2 * o * 36);

    // Region 5: owner/outcome settlement ledgers.
    for array in 0..4 {
        append(
            out,
            v1,
            RESERVED_UNITS_AT + array * MAX_OWNER_SLOTS * 16,
            u * 16,
        );
    }
    for array in 0..3 {
        append(out, v1, LEDGER_EGG_AT + array * MAX_OUTCOMES * 8, o * 8);
    }
    append(out, v1, CASH_SCALARS_AT, 8 * 16);

    // Region 6: explicit-slice aggregate state.
    for array in 0..2 {
        append(out, v1, SPLIT_USED_AT + array * MAX_OUTCOMES * 8, o * 8);
    }

    // Region 7: output summary, with active outcome vectors only.
    append(out, v1, SUMMARY_AT, 1);
    let summary_flows = SUMMARY_AT + 1;
    for array in 0..4 {
        append(out, v1, summary_flows + array * MAX_OUTCOMES * 8, o * 8);
    }
    let summary_virtual = summary_flows + 4 * MAX_OUTCOMES * 8;
    append(out, v1, summary_virtual, 16);
    let summary_eggs = summary_virtual + 16;
    for array in 0..3 {
        append(out, v1, summary_eggs + array * MAX_OUTCOMES * 8, o * 8);
    }
    let summary_tail = summary_eggs + 3 * MAX_OUTCOMES * 8;
    append(out, v1, summary_tail, 260);
    append(out, v1, SUMMARY_VALID_AT, 1);
}

fn take<'a>(payload: &'a [u8], at: &mut usize, len: usize) -> Result<&'a [u8], Fault> {
    let end = at.checked_add(len).ok_or(Fault::WrongLength)?;
    let value = payload.get(*at..end).ok_or(Fault::WrongLength)?;
    *at = end;
    Ok(value)
}

fn place(target: &mut [u8], at: usize, bytes: &[u8]) {
    target[at..at + bytes.len()].copy_from_slice(bytes);
}

fn place_matrix(
    target: &mut [u8],
    at: usize,
    payload: &[u8],
    cursor: &mut usize,
    rows: usize,
    cols: usize,
    cell: usize,
) -> Result<(), Fault> {
    let stride = MAX_OUTCOMES * cell;
    for row in 0..rows {
        place(
            target,
            at + row * stride,
            take(payload, cursor, cols * cell)?,
        );
    }
    Ok(())
}

fn expand_payload(payload: &[u8], widths: Widths) -> Result<Vec<u8>, Fault> {
    if payload.len() != compact_body_len(widths) {
        return Err(Fault::WrongLength);
    }
    let n = widths.orders as usize;
    let u = widths.owners as usize;
    let o = widths.outcomes as usize;
    let mut target = vec![0u8; V1_BODY_BYTES];
    ClearWorkV1::encode_idle_into(&mut target).map_err(Fault::V1Codec)?;
    let mut cursor = 0usize;

    place(
        &mut target,
        CONTROL_AT,
        take(payload, &mut cursor, CONTROL_BYTES + DOMAIN_BYTES)?,
    );
    place(&mut target, CAND_AT, take(payload, &mut cursor, 1)?);
    place(
        &mut target,
        CAND_PRICES_AT,
        take(payload, &mut cursor, o * 8)?,
    );
    place(
        &mut target,
        CAND_TAIL_AT,
        take(payload, &mut cursor, CAND_TAIL_BYTES)?,
    );
    place(&mut target, OWNERS_AT, take(payload, &mut cursor, u * 2)?);
    place(&mut target, OWNER_SLOTS_AT, take(payload, &mut cursor, 2)?);

    place(
        &mut target,
        OWNER_SLOT_AT,
        take(payload, &mut cursor, n * 2)?,
    );
    place(
        &mut target,
        SIDE_BUY_BITS_AT,
        take(payload, &mut cursor, 8)?,
    );
    place(&mut target, TOUCH_AT, take(payload, &mut cursor, n * 2)?);
    place(&mut target, CLASSES_AT, take(payload, &mut cursor, n)?);
    place(&mut target, FLAGS_AT, take(payload, &mut cursor, n)?);
    place(
        &mut target,
        CANCELLED_AT,
        take(payload, &mut cursor, n * 8)?,
    );
    place(&mut target, KEYS_AT, take(payload, &mut cursor, n * 59)?);

    place_matrix(&mut target, SCRATCH_BUY_AT, payload, &mut cursor, n, o, 8)?;
    place_matrix(&mut target, SCRATCH_SELL_AT, payload, &mut cursor, n, o, 8)?;
    place(
        &mut target,
        CELL_PORTFOLIO_AT,
        take(payload, &mut cursor, u * 2)?,
    );

    place(
        &mut target,
        FLOW_BUY_AT,
        take(payload, &mut cursor, o * 16)?,
    );
    place(
        &mut target,
        FLOW_SELL_AT,
        take(payload, &mut cursor, o * 16)?,
    );
    place_matrix(&mut target, PART_BUY_AT, payload, &mut cursor, u, o, 8)?;
    place_matrix(&mut target, PART_SELL_AT, payload, &mut cursor, u, o, 8)?;

    place(&mut target, AGG_AT, take(payload, &mut cursor, o * 128)?);
    place(
        &mut target,
        POOLS_AT,
        take(payload, &mut cursor, 2 * o * 36)?,
    );

    for array in 0..4 {
        place(
            &mut target,
            RESERVED_UNITS_AT + array * MAX_OWNER_SLOTS * 16,
            take(payload, &mut cursor, u * 16)?,
        );
    }
    for array in 0..3 {
        place(
            &mut target,
            LEDGER_EGG_AT + array * MAX_OUTCOMES * 8,
            take(payload, &mut cursor, o * 8)?,
        );
    }
    place(
        &mut target,
        CASH_SCALARS_AT,
        take(payload, &mut cursor, 8 * 16)?,
    );
    for array in 0..2 {
        place(
            &mut target,
            SPLIT_USED_AT + array * MAX_OUTCOMES * 8,
            take(payload, &mut cursor, o * 8)?,
        );
    }

    place(&mut target, SUMMARY_AT, take(payload, &mut cursor, 1)?);
    let summary_flows = SUMMARY_AT + 1;
    for array in 0..4 {
        place(
            &mut target,
            summary_flows + array * MAX_OUTCOMES * 8,
            take(payload, &mut cursor, o * 8)?,
        );
    }
    let summary_virtual = summary_flows + 4 * MAX_OUTCOMES * 8;
    place(
        &mut target,
        summary_virtual,
        take(payload, &mut cursor, 16)?,
    );
    let summary_eggs = summary_virtual + 16;
    for array in 0..3 {
        place(
            &mut target,
            summary_eggs + array * MAX_OUTCOMES * 8,
            take(payload, &mut cursor, o * 8)?,
        );
    }
    let summary_tail = summary_eggs + 3 * MAX_OUTCOMES * 8;
    place(&mut target, summary_tail, take(payload, &mut cursor, 260)?);
    place(
        &mut target,
        SUMMARY_VALID_AT,
        take(payload, &mut cursor, 1)?,
    );
    if cursor != payload.len() {
        return Err(Fault::WrongLength);
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v1::{
        canonical_candidate, canonical_pairing, AllocationPolicyV1, AonPolicyV1, BookV1,
        CandidateV1, FeeBaseV1, FrozenPolicyV1, OrderV1, PairingWitnessPolicyV1,
        PortfolioLotPolicyV1, RelationDomainV1, ResidualSettlementV1, RoundingBoundaryV1,
        ScorePolicyV1, SelfCrossPolicyV1, SingleEggOrderV1, TransferPhaseV1, PRICE_SCALE,
        RELATION_VERSION_V1,
    };
    use clutch_batch::relation_v1_stream::{FeedErrorV1, StreamCandidateV1};
    use clutch_batch::{DustPolicy, PartialPolicy, Side};

    fn policy(self_cross: SelfCrossPolicyV1) -> FrozenPolicyV1 {
        FrozenPolicyV1 {
            allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
            self_cross,
            aon: AonPolicyV1::RefuseAdmission,
            rounding: RoundingBoundaryV1::TerminalOwnerFloor,
            residual_settlement: ResidualSettlementV1::FullPairOnly,
            transfer_phase: TransferPhaseV1::ActiveOnly,
            portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
            pairing_witness: PairingWitnessPolicyV1::RecomputedConstructor,
            dust: DustPolicy::AssignCanonical,
            score: ScorePolicyV1::LexicographicDispersionV1,
            fee_base: FeeBaseV1::None,
        }
    }

    fn domain(self_cross: SelfCrossPolicyV1) -> RelationDomainV1 {
        RelationDomainV1 {
            relation_version: RELATION_VERSION_V1,
            market_id: 11,
            book_id: 22,
            epoch: 7,
            policy_id: 33,
            order_set_id: 44,
            outcome_count: 2,
            owner_count: 3,
            price_scale: PRICE_SCALE,
            remainder_seed: 0xC0FFEE,
            policy: policy(self_cross),
        }
    }

    fn single(id: u64, owner: u16, outcome: u8, side: Side) -> OrderV1 {
        OrderV1::SingleEgg(SingleEggOrderV1 {
            canonical_order_id: id,
            owner,
            outcome,
            side,
            quantity: 2,
            limit_price: if side == Side::Buy { PRICE_SCALE } else { 0 },
            minimum_fill: 1,
            partial_policy: PartialPolicy::Allow,
            expiry_epoch: u64::MAX,
        })
    }

    fn fixture(self_cross: SelfCrossPolicyV1) -> (RelationDomainV1, BookV1, [u64; MAX_OUTCOMES]) {
        let domain = domain(self_cross);
        let mut book = BookV1::empty();
        book.len = 4;
        book.orders[0] = single(1, 0, 0, Side::Buy);
        book.orders[1] = single(2, 1, 0, Side::Sell);
        book.orders[2] = single(3, 2, 1, Side::Buy);
        book.orders[3] = single(4, 1, 1, Side::Sell);
        let mut prices = [0u64; MAX_OUTCOMES];
        prices[0] = PRICE_SCALE / 2;
        prices[1] = PRICE_SCALE / 2;
        (domain, book, prices)
    }

    fn header(candidate: &clutch_batch::relation_v1::CandidateV1) -> StreamCandidateV1 {
        StreamCandidateV1 {
            order_len: candidate.order_len,
            prices: candidate.prices,
            virtual_split: candidate.virtual_split,
            virtual_merge: candidate.virtual_merge,
            honored_aon_mask: candidate.honored_aon_mask,
            claimed_score: candidate.claimed_score,
            canonical_candidate_digest: candidate.canonical_candidate_digest,
            declared_slices: None,
        }
    }

    fn encoded(work: &ClearWorkV1) -> Vec<u8> {
        let mut out = vec![0u8; V1_BODY_BYTES];
        work.encode_into(&mut out).unwrap();
        out
    }

    fn reachable_snapshots(self_cross: SelfCrossPolicyV1) -> Vec<Vec<u8>> {
        let (domain, book, prices) = fixture(self_cross);
        let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
        let mut work = ClearWorkV1::new();
        let mut snapshots = vec![encoded(&work)];
        work.begin(&domain, &header(&candidate), true).unwrap();
        snapshots.push(encoded(&work));
        for pass in 0..if self_cross == SelfCrossPolicyV1::NetAtAdmission {
            3
        } else {
            2
        } {
            for index in 0..book.len as usize {
                work.push_order(&book.orders[index], candidate.fills[index])
                    .unwrap();
                snapshots.push(encoded(&work));
            }
            work.end_pass().unwrap();
            snapshots.push(encoded(&work));
            if pass > 3 {
                unreachable!();
            }
        }
        snapshots
    }

    fn edge_snapshots() -> Vec<(Widths, Vec<u8>)> {
        let (domain, book, prices) = fixture(SelfCrossPolicyV1::AllowGateAtPairing);
        let candidate = canonical_candidate(&domain, &book, &prices, 0, 0).unwrap();
        let widths = Widths::new(2, 4, 3);
        let mut states = Vec::new();

        // The empty frozen set has zero active order/owner rows. The relation
        // refuses owner_count=0 at begin, but the checkpoint must persist that
        // exact verdict and remain closable.
        let mut empty_domain = domain;
        empty_domain.owner_count = 0;
        let empty_candidate = CandidateV1::empty(0, prices);
        let mut empty_work = ClearWorkV1::new();
        empty_work
            .begin(&empty_domain, &header(&empty_candidate), true)
            .unwrap();
        states.push((Widths::new(2, 0, 0), encoded(&empty_work)));

        // A relation refusal reached at begin still carries its refused frozen
        // coordinates and must survive a save/resume.
        let mut invalid_domain = domain;
        invalid_domain.relation_version = 99;
        let mut work = ClearWorkV1::new();
        work.begin(&invalid_domain, &header(&candidate), true)
            .unwrap();
        states.push((widths, encoded(&work)));

        // A mismatched second pass poisons the feed and yields no verdict.
        let mut poisoned = ClearWorkV1::new();
        poisoned.begin(&domain, &header(&candidate), true).unwrap();
        for index in 0..book.len as usize {
            poisoned
                .push_order(&book.orders[index], candidate.fills[index])
                .unwrap();
        }
        poisoned.end_pass().unwrap();
        for index in 0..book.len as usize {
            let fill = if index == 0 {
                candidate.fills[index].wrapping_add(1)
            } else {
                candidate.fills[index]
            };
            poisoned.push_order(&book.orders[index], fill).unwrap();
        }
        assert_eq!(poisoned.end_pass(), Err(FeedErrorV1::ResumeFoldMismatch));
        states.push((widths, encoded(&poisoned)));

        // Claims-disabled mode is the live SBF mode; check_claims is persisted.
        let mut unchecked = ClearWorkV1::new();
        unchecked
            .begin(&domain, &header(&candidate), false)
            .unwrap();
        states.push((widths, encoded(&unchecked)));

        // Explicit slices exercise the region that order-only walks leave at
        // canonical zero, including every slice cursor boundary.
        let mut explicit_domain = domain;
        explicit_domain.owner_count = 2;
        explicit_domain.policy.pairing_witness = PairingWitnessPolicyV1::ExplicitSlices;
        explicit_domain.policy.residual_settlement = ResidualSettlementV1::UniqueSliceReceipts;
        let mut cross = BookV1::empty();
        cross.len = 2;
        cross.orders[0] = single(1, 0, 0, Side::Buy);
        cross.orders[1] = single(2, 1, 0, Side::Sell);
        let sliced = canonical_candidate(&explicit_domain, &cross, &prices, 0, 0).unwrap();
        let witness = canonical_pairing(&explicit_domain, &cross, &sliced).unwrap();
        let sliced_header = StreamCandidateV1 {
            declared_slices: Some(witness.len),
            ..header(&sliced)
        };
        let slice_widths = Widths::new(2, 2, 2);
        let mut sliced_work = ClearWorkV1::new();
        sliced_work
            .begin(&explicit_domain, &sliced_header, true)
            .unwrap();
        for index in 0..cross.len as usize {
            sliced_work
                .push_order(&cross.orders[index], sliced.fills[index])
                .unwrap();
        }
        sliced_work.end_pass().unwrap();
        states.push((slice_widths, encoded(&sliced_work)));
        for index in 0..witness.len as usize {
            sliced_work.push_slice(&witness.slices[index]).unwrap();
            states.push((slice_widths, encoded(&sliced_work)));
        }
        sliced_work.end_pass().unwrap();
        states.push((slice_widths, encoded(&sliced_work)));
        for index in 0..cross.len as usize {
            sliced_work
                .push_order(&cross.orders[index], sliced.fills[index])
                .unwrap();
        }
        sliced_work.end_pass().unwrap();
        states.push((slice_widths, encoded(&sliced_work)));
        states
    }

    #[test]
    fn formula_reconstructs_the_pinned_v1_body_and_account() {
        let max = Widths::new(16, 64, 64);
        assert_eq!(compact_body_len(max), 47_846);
        assert_eq!(account_len(max), V1_ACCOUNT_BYTES);
        assert_eq!(minimum_balance(max), 349_266_720);
    }

    #[test]
    fn every_reachable_boundary_round_trips_byte_exactly() {
        let widths = Widths::new(2, 4, 3);
        let mut count = 0usize;
        for self_cross in [
            SelfCrossPolicyV1::AllowGateAtPairing,
            SelfCrossPolicyV1::NetAtAdmission,
        ] {
            for v1 in reachable_snapshots(self_cross) {
                let compact = encode_from_v1(&v1, widths).unwrap_or_else(|fault| {
                    let mut payload = Vec::new();
                    project_payload(&v1, widths, &mut payload);
                    let rebuilt = expand_payload(&payload, widths).unwrap();
                    let diffs: Vec<_> = v1
                        .iter()
                        .zip(rebuilt.iter())
                        .enumerate()
                        .filter(|(_, (a, b))| a != b)
                        .take(16)
                        .map(|(at, (a, b))| (at, *a, *b))
                        .collect();
                    panic!("projection {fault:?}, first diffs {diffs:?}")
                });
                assert_eq!(decode_to_v1(&compact, widths).unwrap(), v1);
                let mut decoded = ClearWorkV1::new();
                decode_into_v1(&compact, widths, &mut decoded).unwrap();
                assert_eq!(encoded(&decoded), v1);
                count += 1;
            }
        }
        for (widths, v1) in edge_snapshots() {
            let compact = encode_from_v1(&v1, widths).unwrap();
            assert_eq!(decode_to_v1(&compact, widths).unwrap(), v1);
            let mut decoded = ClearWorkV1::new();
            decode_into_v1(&compact, widths, &mut decoded).unwrap();
            assert_eq!(encoded(&decoded), v1);
            count += 1;
        }
        assert_eq!(
            count, 37,
            "every push/pass boundary plus refusal, poison, unchecked, and slice states"
        );
    }

    #[test]
    fn every_omitted_v1_byte_is_required_to_be_canonical_padding() {
        let widths = Widths::new(2, 4, 3);
        let idle = encoded(&ClearWorkV1::new());
        let compact = encode_from_v1(&idle, widths).unwrap();
        let retained = decode_to_v1(&compact, widths).unwrap();
        let mut omitted = 0usize;
        for at in 0..V1_BODY_BYTES {
            let mut changed = idle.clone();
            changed[at] ^= 0xA5;
            if retained[at] == idle[at] && encode_from_v1(&changed, widths).is_err() {
                omitted += 1;
            }
        }
        assert_eq!(omitted, V1_BODY_BYTES - compact_body_len(widths));
    }

    #[test]
    fn hostile_compact_bytes_are_total_and_reencoding_closed() {
        let widths = Widths::new(2, 4, 3);
        let snapshots = reachable_snapshots(SelfCrossPolicyV1::AllowGateAtPairing);
        let base = encode_from_v1(snapshots.last().unwrap(), widths).unwrap();
        for at in 0..base.len() {
            let mut changed = base.clone();
            changed[at] ^= 0xFF;
            let mut decoded = ClearWorkV1::new();
            if decode_into_v1(&changed, widths, &mut decoded).is_ok() {
                let v1 = encoded(&decoded);
                assert_eq!(encode_from_v1(&v1, widths).unwrap(), changed);
            }
        }
        for len in 0..MODEL_PREFIX_BYTES {
            assert!(decode_to_v1(&base[..len], widths).is_err());
        }
        assert!(decode_to_v1(&base[..base.len() - 1], widths).is_err());
        let mut long = base;
        long.push(0);
        assert!(decode_to_v1(&long, widths).is_err());
    }

    #[test]
    fn dimensions_are_candidate_bound_not_caller_selected() {
        let widths = Widths::new(2, 4, 3);
        let compact = encode_from_v1(&encoded(&ClearWorkV1::new()), widths).unwrap();
        for wrong in [
            Widths::new(3, 4, 3),
            Widths::new(2, 5, 3),
            Widths::new(2, 4, 4),
        ] {
            assert_eq!(
                decode_to_v1(&compact, wrong),
                Err(Fault::WidthBindingMismatch)
            );
        }
    }

    #[test]
    fn best_common_and_worst_widths_are_pinned() {
        let rows = [
            (Widths::new(2, 0, 0), 1_510, 11_400_480),
            (Widths::new(2, 1, 1), 1_747, 13_050_000),
            (Widths::new(2, 4, 3), 2_326, 17_079_840),
            (Widths::new(4, 16, 8), 5_686, 40_465_440),
            (Widths::new(8, 32, 16), 13_606, 95_588_640),
            (Widths::new(16, 64, 64), 50_054, 349_266_720),
        ];
        for (widths, bytes, rent) in rows {
            assert_eq!(account_len(widths), bytes);
            assert_eq!(minimum_balance(widths), rent);
        }
    }
}
