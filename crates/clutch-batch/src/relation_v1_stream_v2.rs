//! Active-width storage primitives for resumable `BatchRelationV1` clearing.
//!
//! V1 has one canonical 47,846-byte body, irrespective of the frozen book's
//! active dimensions.  V2 preserves V1's field encodings and semantic region
//! order, but stores only the rows selected by the independently authenticated
//! `(outcomes, orders, owners)` dimensions.  It is a distinct codec version:
//! callers must bind the outer account version and derive the dimensions from
//! frozen state before opening a body.  Account length is not an authority.
//!
//! The intended runtime representation is [`ClearWorkViewV2`]/
//! [`ClearWorkViewMutV2`], a small borrowed view over account bytes.  It never
//! expands a fixed checkpoint and all active matrix access is bounds checked.
//! The V1 bridge is only for migration, differential tests, and rollout: it is
//! `no_alloc`, but explicitly requires a caller-owned V1-sized scratch buffer.
//!
//! This module does not replace the three independent continuation gates:
//! the relation fold, the layout-owned order-set/page continuation, and the
//! layout header's consumed-fold seal.  In particular, [`require_sealed_fold`]
//! compares a persisted fold; it does not turn `DigestFoldV1` into a
//! cryptographic commitment or authenticate unrelated regions.

use crate::relation_v1::{MAX_OUTCOMES, MAX_OWNER_SLOTS, MAX_PORTFOLIO_ORDERS, MAX_SLICES};
use crate::relation_v1_stream::{
    decode_policy_v1, ClearWorkV1, CodecFaultV1, POLICY_ENCODED_BYTES,
};
use crate::MAX_ORDERS;

/// Outer account/codec version for this active-width body.
pub const CLEAR_WORK_CODEC_VERSION_V2: u8 = 2;

/// The fixed V1 body length, exposed to make bridge scratch ownership explicit.
pub const CLEAR_WORK_V1_BODY_BYTES: usize = ClearWorkV1::ENCODED_BYTES;

const PHASE_IDLE: u8 = 0;
const PHASE_ORDERS: u8 = 1;
const PHASE_SLICES: u8 = 2;
const PHASE_COMPLETE: u8 = 3;
const PHASE_POISONED: u8 = 4;

const CLASS_INELIGIBLE: u8 = 2;
const FLAG_ALL: u8 = 0x1f;
const POOL_NONE: u8 = u8::MAX;

// Fixed V1 wire offsets.  These are bridge facts, not the V2 runtime layout.
const V1_CONTROL_AT: usize = 0;
const V1_CONTROL_BYTES: usize = 82;
const V1_DOMAIN_BYTES: usize = 78;
const V1_CAND_AT: usize = 160;
const V1_CAND_PRICES_AT: usize = V1_CAND_AT + 1;
const V1_CAND_TAIL_AT: usize = V1_CAND_PRICES_AT + MAX_OUTCOMES * 8;
const V1_CAND_TAIL_BYTES: usize = 101;
const V1_OWNERS_AT: usize = V1_CAND_TAIL_AT + V1_CAND_TAIL_BYTES;
const V1_OWNER_SLOTS_AT: usize = V1_OWNERS_AT + MAX_OWNER_SLOTS * 2;
const V1_OWNER_SLOT_AT: usize = V1_OWNER_SLOTS_AT + 2;
const V1_SIDE_BUY_BITS_AT: usize = V1_OWNER_SLOT_AT + MAX_ORDERS * 2;
const V1_TOUCH_AT: usize = V1_SIDE_BUY_BITS_AT + 8;
const V1_CLASSES_AT: usize = V1_TOUCH_AT + MAX_ORDERS * 2;
const V1_FLAGS_AT: usize = V1_CLASSES_AT + MAX_ORDERS;
const V1_CANCELLED_AT: usize = V1_FLAGS_AT + MAX_ORDERS;
const V1_KEYS_AT: usize = V1_CANCELLED_AT + MAX_ORDERS * 8;
const V1_SCRATCH_BUY_AT: usize = V1_KEYS_AT + MAX_ORDERS * 59;
const V1_SCRATCH_SELL_AT: usize = V1_SCRATCH_BUY_AT + MAX_ORDERS * MAX_OUTCOMES * 8;
const V1_CELL_PORTFOLIO_AT: usize = V1_SCRATCH_SELL_AT + MAX_ORDERS * MAX_OUTCOMES * 8;
const V1_FLOW_BUY_AT: usize = V1_CELL_PORTFOLIO_AT + MAX_OWNER_SLOTS * 2;
const V1_FLOW_SELL_AT: usize = V1_FLOW_BUY_AT + MAX_OUTCOMES * 16;
const V1_PART_BUY_AT: usize = V1_FLOW_SELL_AT + MAX_OUTCOMES * 16;
const V1_PART_SELL_AT: usize = V1_PART_BUY_AT + MAX_OWNER_SLOTS * MAX_OUTCOMES * 8;
const V1_AGG_AT: usize = V1_PART_SELL_AT + MAX_OWNER_SLOTS * MAX_OUTCOMES * 8;
const V1_POOLS_AT: usize = V1_AGG_AT + MAX_OUTCOMES * 128;
const V1_RESERVED_UNITS_AT: usize = V1_POOLS_AT + 2 * MAX_OUTCOMES * 36;
const V1_LEDGER_EGG_AT: usize = V1_RESERVED_UNITS_AT + 4 * MAX_OWNER_SLOTS * 16;
const V1_CASH_SCALARS_AT: usize = V1_LEDGER_EGG_AT + 3 * MAX_OUTCOMES * 8;
const V1_SPLIT_USED_AT: usize = V1_CASH_SCALARS_AT + 8 * 16;
const V1_SUMMARY_AT: usize = V1_SPLIT_USED_AT + 2 * MAX_OUTCOMES * 8;
const V1_SUMMARY_VALID_AT: usize = V1_SUMMARY_AT + 1_173;

const _: () = assert!(V1_SUMMARY_VALID_AT + 1 == CLEAR_WORK_V1_BODY_BYTES);

/// Frozen active dimensions, derived outside this codec from authenticated
/// epoch/window/feed state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearWorkWidthsV2 {
    pub outcomes: u8,
    pub orders: u8,
    pub owners: u8,
}

impl ClearWorkWidthsV2 {
    pub const fn new(outcomes: u8, orders: u8, owners: u8) -> Self {
        Self {
            outcomes,
            orders,
            owners,
        }
    }

    /// Refuse dimensions before using them in any offset calculation.
    pub fn validate(self) -> Result<Self, ClearWorkFaultV2> {
        if self.outcomes == 0
            || self.outcomes as usize > MAX_OUTCOMES
            || self.orders as usize > MAX_ORDERS
            || self.owners as usize > MAX_OWNER_SLOTS
            || self.owners > self.orders
            || (self.orders == 0) != (self.owners == 0)
        {
            return Err(ClearWorkFaultV2::InvalidWidths);
        }
        Ok(self)
    }
}

/// The eight semantic storage regions.  Their order is canonical and frozen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClearWorkRegionV2 {
    Control = 0,
    Orders = 1,
    Scratch = 2,
    Flows = 3,
    Pools = 4,
    Ledger = 5,
    Slices = 6,
    Summary = 7,
}

/// One exact half-open region span within a V2 body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionSpanV2 {
    pub offset: usize,
    pub len: usize,
}

impl RegionSpanV2 {
    pub const fn end(self) -> usize {
        self.offset + self.len
    }
}

#[derive(Clone, Copy)]
struct LayoutV2 {
    spans: [RegionSpanV2; 8],
}

impl LayoutV2 {
    const fn new(widths: ClearWorkWidthsV2) -> Self {
        let n = widths.orders as usize;
        let u = widths.owners as usize;
        let o = widths.outcomes as usize;
        let lengths = [
            264 + 8 * o + 2 * u,
            8 + 73 * n,
            16 * n * o + 2 * u,
            32 * o + 16 * u * o,
            200 * o,
            128 + 64 * u + 24 * o,
            16 * o,
            278 + 56 * o,
        ];
        let mut spans = [RegionSpanV2 { offset: 0, len: 0 }; 8];
        let mut offset = 0usize;
        let mut i = 0usize;
        while i < spans.len() {
            spans[i] = RegionSpanV2 {
                offset,
                len: lengths[i],
            };
            offset += lengths[i];
            i += 1;
        }
        Self { spans }
    }

    const fn span(self, region: ClearWorkRegionV2) -> RegionSpanV2 {
        self.spans[region as usize]
    }

    const fn len(self) -> usize {
        self.spans[7].end()
    }
}

/// Exact V2 body length.  Validate `widths` before allocating or slicing.
pub const fn clear_work_v2_body_len(widths: ClearWorkWidthsV2) -> usize {
    LayoutV2::new(widths).len()
}

/// Exact region geometry for already validated dimensions.
pub const fn clear_work_v2_region_span(
    widths: ClearWorkWidthsV2,
    region: ClearWorkRegionV2,
) -> RegionSpanV2 {
    LayoutV2::new(widths).span(region)
}

/// Typed refusal for V2 bytes and active-region access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClearWorkFaultV2 {
    WrongLength,
    InvalidWidths,
    WidthBindingMismatch,
    NonCanonicalPadding,
    InvalidPhase,
    InvalidBool,
    InvalidClass,
    InvalidFlags,
    InvalidErrorCode,
    InvalidCount,
    InvalidSlot,
    InvalidSliceDeclaration,
    InvalidIndex,
    ResumeSealMismatch,
    V1Codec(CodecFaultV1),
}

/// Feed phase decoded without exposing raw phase bytes to runtime consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClearWorkPhaseV2 {
    Idle,
    Orders,
    Slices,
    Complete,
    Poisoned,
}

impl ClearWorkPhaseV2 {
    fn decode(value: u8) -> Result<Self, ClearWorkFaultV2> {
        match value {
            PHASE_IDLE => Ok(Self::Idle),
            PHASE_ORDERS => Ok(Self::Orders),
            PHASE_SLICES => Ok(Self::Slices),
            PHASE_COMPLETE => Ok(Self::Complete),
            PHASE_POISONED => Ok(Self::Poisoned),
            _ => Err(ClearWorkFaultV2::InvalidPhase),
        }
    }

    fn active(self) -> bool {
        matches!(self, Self::Orders | Self::Slices)
    }
}

/// Small lifecycle projection used by adapters and native execution code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearWorkProgressV2 {
    pub phase: ClearWorkPhaseV2,
    pub pass: u8,
    pub cursor: u16,
    pub slice_cursor: u16,
    pub order_count: u16,
    pub owner_slots: u16,
}

/// One relation fold in its canonical V1 high-word/low-word order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldWordsV2 {
    pub high: u64,
    pub low: u64,
}

/// The three relation-owned folds persisted in the control region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResumeFoldsV2 {
    pub current: FoldWordsV2,
    pub sealed: FoldWordsV2,
    pub candidate: FoldWordsV2,
}

/// Active-width matrix families used by the streaming relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatrixU64V2 {
    ScratchBuy,
    ScratchSell,
    ParticipationBuy,
    ParticipationSell,
}

/// Per-owner `u128` settlement accumulators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerUnitsV2 {
    Reserved,
    Debit,
    Credit,
    FeeBps,
}

/// Per-outcome `u128` flow accumulators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeFlowV2 {
    Buy,
    Sell,
}

/// One of the eight per-outcome canonical-allocation aggregates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OutcomeAggregateV2 {
    Demand = 0,
    Supply = 1,
    ForcedBuy = 2,
    ForcedSell = 3,
    ForcedAonBuy = 4,
    ForcedAonSell = 5,
    StrictBuy = 6,
    StrictSell = 7,
}

/// Initialize the exact-width canonical idle image directly, without V1
/// expansion or a static 48 KiB checkpoint.
pub fn initialize_clear_work_v2_idle(
    out: &mut [u8],
    widths: ClearWorkWidthsV2,
) -> Result<(), ClearWorkFaultV2> {
    widths.validate()?;
    let layout = LayoutV2::new(widths);
    if out.len() != layout.len() {
        return Err(ClearWorkFaultV2::WrongLength);
    }
    out.fill(0);

    // V1::NEW.check_claims = true and DOMAIN_ZERO.policy.dust = Reject.
    out[5] = 1;
    out[153] = 1;
    // DigestFoldV1::NEW, repeated for current/sealed/candidate.  These words
    // are part of the frozen V1 field encoding, not an all-zero default.
    let mut fold_at = 25usize;
    while fold_at < 73 {
        write_u64(out, fold_at, 0x243f_6a88_85a3_08d3);
        write_u64(out, fold_at + 8, 0x1319_8a2e_0370_7344);
        fold_at += 16;
    }

    let orders = layout.span(ClearWorkRegionV2::Orders);
    let n = widths.orders as usize;
    let classes = orders.offset + 8 + 4 * n;
    out[classes..classes + n].fill(CLASS_INELIGIBLE);
    let keys = orders.offset + 8 + 14 * n;
    let mut i = 0usize;
    while i < n {
        out[keys + i * 59 + 56] = POOL_NONE;
        i += 1;
    }
    Ok(())
}

/// Validate an exact-width body without allocating or expanding to V1.
///
/// This validates every structural byte before a typed view can index it.  It
/// deliberately does not claim to re-prove relation arithmetic: folds,
/// candidate claims, and aggregates retain their separate semantic owners.
pub fn validate_clear_work_v2(
    input: &[u8],
    widths: ClearWorkWidthsV2,
) -> Result<ClearWorkProgressV2, ClearWorkFaultV2> {
    widths.validate()?;
    let layout = LayoutV2::new(widths);
    if input.len() != layout.len() {
        return Err(ClearWorkFaultV2::WrongLength);
    }
    let n = widths.orders as usize;
    let u = widths.owners as usize;
    let o = widths.outcomes as usize;

    let phase = ClearWorkPhaseV2::decode(input[0])?;
    require_bool(input[4])?;
    require_bool(input[5])?;
    let cursor = read_u16(input, 6);
    let slice_cursor = read_u16(input, 8);
    let order_count = read_u16(input, 10);
    require_bool(input[12])?;
    if cursor as usize > n || slice_cursor as usize > MAX_SLICES || order_count as usize > n {
        return Err(ClearWorkFaultV2::InvalidCount);
    }
    validate_error(&input[21..25])?;
    if input[81] as usize > MAX_PORTFOLIO_ORDERS {
        return Err(ClearWorkFaultV2::InvalidCount);
    }

    let domain_outcomes = input[126];
    let domain_owners = read_u16(input, 127);
    if phase.active() {
        if domain_outcomes != widths.outcomes
            || domain_owners != widths.owners as u16
            || input[160] != widths.orders
        {
            return Err(ClearWorkFaultV2::WidthBindingMismatch);
        }
        if read_u64(input, 129) == 0 {
            return Err(ClearWorkFaultV2::InvalidCount);
        }
    }
    decode_policy_v1(&input[145..145 + POLICY_ENCODED_BYTES]).map_err(ClearWorkFaultV2::V1Codec)?;

    let control = layout.span(ClearWorkRegionV2::Control);
    let candidate_tail = 161 + 8 * o;
    let declared_flag = input[candidate_tail + 98];
    let declared = read_u16(input, candidate_tail + 99);
    match declared_flag {
        0 if declared == 0 => {}
        1 => {}
        _ => return Err(ClearWorkFaultV2::InvalidSliceDeclaration),
    }
    let owners_at = candidate_tail + V1_CAND_TAIL_BYTES;
    let owner_slots_at = owners_at + 2 * u;
    debug_assert_eq!(owner_slots_at + 2, control.end());
    let owner_slots = read_u16(input, owner_slots_at);
    if owner_slots as usize > u {
        return Err(ClearWorkFaultV2::InvalidCount);
    }
    if phase.active() {
        let consumed = if phase == ClearWorkPhaseV2::Orders && input[1] <= 1 {
            cursor
        } else {
            order_count
        };
        if owner_slots > consumed {
            return Err(ClearWorkFaultV2::InvalidCount);
        }
    }
    let mut i = owner_slots as usize;
    while i < u {
        if read_u16(input, owners_at + 2 * i) != 0 {
            return Err(ClearWorkFaultV2::NonCanonicalPadding);
        }
        i += 1;
    }

    let orders = layout.span(ClearWorkRegionV2::Orders);
    let owner_slot_at = orders.offset;
    let side_bits_at = owner_slot_at + 2 * n;
    let touch_at = side_bits_at + 8;
    let classes_at = touch_at + 2 * n;
    let flags_at = classes_at + n;
    let cancelled_at = flags_at + n;
    let keys_at = cancelled_at + 8 * n;
    debug_assert_eq!(keys_at + 59 * n, orders.end());
    let side_bits = read_u64(input, side_bits_at);
    if n < 64 && side_bits >> n != 0 {
        return Err(ClearWorkFaultV2::NonCanonicalPadding);
    }
    let outcome_mask = if o == 16 { u16::MAX } else { (1u16 << o) - 1 };
    i = 0;
    while i < n {
        if u == 0 || read_u16(input, owner_slot_at + 2 * i) as usize >= u {
            return Err(ClearWorkFaultV2::InvalidSlot);
        }
        if read_u16(input, touch_at + 2 * i) & !outcome_mask != 0 {
            return Err(ClearWorkFaultV2::NonCanonicalPadding);
        }
        if input[classes_at + i] > CLASS_INELIGIBLE {
            return Err(ClearWorkFaultV2::InvalidClass);
        }
        if input[flags_at + i] & !FLAG_ALL != 0 {
            return Err(ClearWorkFaultV2::InvalidFlags);
        }
        let row = keys_at + 59 * i;
        let pool = input[row + 56];
        if pool != POOL_NONE && pool as usize >= 2 * o {
            return Err(ClearWorkFaultV2::InvalidSlot);
        }
        require_bool(input[row + 57])?;
        require_bool(input[row + 58])?;
        i += 1;
    }

    let scratch = layout.span(ClearWorkRegionV2::Scratch);
    let cell_portfolio = scratch.offset + 16 * n * o;
    i = 0;
    while i < u {
        if read_u16(input, cell_portfolio + 2 * i) & !outcome_mask != 0 {
            return Err(ClearWorkFaultV2::NonCanonicalPadding);
        }
        i += 1;
    }

    let pools = layout.span(ClearWorkRegionV2::Pools);
    let pool_rows = pools.offset + 128 * o;
    i = 0;
    while i < 2 * o {
        let row = pool_rows + 36 * i;
        let total = read_u128(input, row);
        let count = read_u16(input, row + 16);
        let target = read_u64(input, row + 18);
        let ready = require_bool(input[row + 34])?;
        require_bool(input[row + 35])?;
        if count as usize > n || (ready && target != 0 && total == 0) {
            return Err(ClearWorkFaultV2::InvalidCount);
        }
        i += 1;
    }

    let summary = layout.span(ClearWorkRegionV2::Summary);
    let summary_valid = require_bool(input[summary.end() - 1])?;
    if summary_valid && input[summary.offset] != widths.outcomes {
        return Err(ClearWorkFaultV2::WidthBindingMismatch);
    }

    Ok(ClearWorkProgressV2 {
        phase,
        pass: input[1],
        cursor,
        slice_cursor,
        order_count,
        owner_slots,
    })
}

/// Immutable direct view of a validated V2 body.
pub struct ClearWorkViewV2<'a> {
    bytes: &'a [u8],
    widths: ClearWorkWidthsV2,
    progress: ClearWorkProgressV2,
}

impl<'a> ClearWorkViewV2<'a> {
    pub fn open(bytes: &'a [u8], widths: ClearWorkWidthsV2) -> Result<Self, ClearWorkFaultV2> {
        let progress = validate_clear_work_v2(bytes, widths)?;
        Ok(Self {
            bytes,
            widths,
            progress,
        })
    }

    pub const fn widths(&self) -> ClearWorkWidthsV2 {
        self.widths
    }

    pub const fn progress(&self) -> ClearWorkProgressV2 {
        self.progress
    }

    pub fn region(&self, region: ClearWorkRegionV2) -> &'a [u8] {
        let span = LayoutV2::new(self.widths).span(region);
        &self.bytes[span.offset..span.end()]
    }

    pub fn folds(&self) -> ResumeFoldsV2 {
        ResumeFoldsV2 {
            current: read_fold(self.bytes, 25),
            sealed: read_fold(self.bytes, 41),
            candidate: read_fold(self.bytes, 57),
        }
    }

    pub fn candidate_digest(&self) -> u128 {
        let at = 161 + 8 * self.widths.outcomes as usize + 82;
        read_u128(self.bytes, at)
    }

    pub fn matrix_u64(
        &self,
        matrix: MatrixU64V2,
        row: usize,
        outcome: usize,
    ) -> Result<u64, ClearWorkFaultV2> {
        let at = matrix_offset(
            LayoutV2::new(self.widths),
            self.widths,
            matrix,
            row,
            outcome,
        )?;
        Ok(read_u64(self.bytes, at))
    }

    pub fn owner_units(
        &self,
        column: OwnerUnitsV2,
        owner: usize,
    ) -> Result<u128, ClearWorkFaultV2> {
        let at = owner_units_offset(LayoutV2::new(self.widths), self.widths, column, owner)?;
        Ok(read_u128(self.bytes, at))
    }

    pub fn outcome_flow(
        &self,
        side: OutcomeFlowV2,
        outcome: usize,
    ) -> Result<u128, ClearWorkFaultV2> {
        let at = outcome_flow_offset(LayoutV2::new(self.widths), self.widths, side, outcome)?;
        Ok(read_u128(self.bytes, at))
    }

    pub fn outcome_aggregate(
        &self,
        column: OutcomeAggregateV2,
        outcome: usize,
    ) -> Result<u128, ClearWorkFaultV2> {
        let at =
            outcome_aggregate_offset(LayoutV2::new(self.widths), self.widths, column, outcome)?;
        Ok(read_u128(self.bytes, at))
    }
}

/// Mutable direct view.  Only numeric cells whose full bit range is canonical
/// are writable here; selector, boolean, index, phase, and fold mutation stay
/// with the eventual native feed engine so this primitive layer cannot create
/// structurally invalid bytes through a typed setter.
pub struct ClearWorkViewMutV2<'a> {
    bytes: &'a mut [u8],
    widths: ClearWorkWidthsV2,
    progress: ClearWorkProgressV2,
}

impl<'a> ClearWorkViewMutV2<'a> {
    pub fn open(bytes: &'a mut [u8], widths: ClearWorkWidthsV2) -> Result<Self, ClearWorkFaultV2> {
        let progress = validate_clear_work_v2(bytes, widths)?;
        Ok(Self {
            bytes,
            widths,
            progress,
        })
    }

    pub const fn progress(&self) -> ClearWorkProgressV2 {
        self.progress
    }

    pub fn matrix_u64(
        &self,
        matrix: MatrixU64V2,
        row: usize,
        outcome: usize,
    ) -> Result<u64, ClearWorkFaultV2> {
        let at = matrix_offset(
            LayoutV2::new(self.widths),
            self.widths,
            matrix,
            row,
            outcome,
        )?;
        Ok(read_u64(self.bytes, at))
    }

    pub fn set_matrix_u64(
        &mut self,
        matrix: MatrixU64V2,
        row: usize,
        outcome: usize,
        value: u64,
    ) -> Result<(), ClearWorkFaultV2> {
        let at = matrix_offset(
            LayoutV2::new(self.widths),
            self.widths,
            matrix,
            row,
            outcome,
        )?;
        write_u64(self.bytes, at, value);
        Ok(())
    }

    pub fn owner_units(
        &self,
        column: OwnerUnitsV2,
        owner: usize,
    ) -> Result<u128, ClearWorkFaultV2> {
        let at = owner_units_offset(LayoutV2::new(self.widths), self.widths, column, owner)?;
        Ok(read_u128(self.bytes, at))
    }

    pub fn set_owner_units(
        &mut self,
        column: OwnerUnitsV2,
        owner: usize,
        value: u128,
    ) -> Result<(), ClearWorkFaultV2> {
        let at = owner_units_offset(LayoutV2::new(self.widths), self.widths, column, owner)?;
        write_u128(self.bytes, at, value);
        Ok(())
    }

    pub fn set_outcome_flow(
        &mut self,
        side: OutcomeFlowV2,
        outcome: usize,
        value: u128,
    ) -> Result<(), ClearWorkFaultV2> {
        let at = outcome_flow_offset(LayoutV2::new(self.widths), self.widths, side, outcome)?;
        write_u128(self.bytes, at, value);
        Ok(())
    }

    pub fn set_outcome_aggregate(
        &mut self,
        column: OutcomeAggregateV2,
        outcome: usize,
        value: u128,
    ) -> Result<(), ClearWorkFaultV2> {
        let at =
            outcome_aggregate_offset(LayoutV2::new(self.widths), self.widths, column, outcome)?;
        write_u128(self.bytes, at, value);
        Ok(())
    }
}

/// Preserve the layout-owned consumed-fold gate across a native V2 resume.
pub fn require_sealed_fold(
    view: &ClearWorkViewV2<'_>,
    expected: FoldWordsV2,
) -> Result<(), ClearWorkFaultV2> {
    if view.folds().sealed != expected {
        return Err(ClearWorkFaultV2::ResumeSealMismatch);
    }
    Ok(())
}

/// Project a canonical V1 wire image into exact-width V2 bytes.
///
/// `scratch` is overwritten with the reconstructed V1 image and exists solely
/// to prove that every omitted byte was canonical padding.  Runtime V2 code
/// should open the compact body directly and should not call this bridge.
pub fn project_clear_work_v1_wire_into_v2(
    v1: &[u8],
    widths: ClearWorkWidthsV2,
    out: &mut [u8],
    scratch: &mut [u8],
) -> Result<(), ClearWorkFaultV2> {
    widths.validate()?;
    if v1.len() != CLEAR_WORK_V1_BODY_BYTES
        || scratch.len() != CLEAR_WORK_V1_BODY_BYTES
        || out.len() != clear_work_v2_body_len(widths)
    {
        return Err(ClearWorkFaultV2::WrongLength);
    }
    project_payload(v1, widths, out);
    expand_payload_into_v1(out, widths, scratch)?;
    if scratch != v1 {
        return Err(ClearWorkFaultV2::NonCanonicalPadding);
    }
    validate_clear_work_v2(out, widths)?;
    Ok(())
}

/// Reconstruct a canonical V1 wire image from V2 using caller-owned output.
/// Hostile V2 bytes are structurally validated before any V1 image is exposed.
pub fn expand_clear_work_v2_into_v1_wire(
    input: &[u8],
    widths: ClearWorkWidthsV2,
    out: &mut [u8],
) -> Result<(), ClearWorkFaultV2> {
    validate_clear_work_v2(input, widths)?;
    if out.len() != CLEAR_WORK_V1_BODY_BYTES {
        return Err(ClearWorkFaultV2::WrongLength);
    }
    expand_payload_into_v1(input, widths, out)
}

fn matrix_offset(
    layout: LayoutV2,
    widths: ClearWorkWidthsV2,
    matrix: MatrixU64V2,
    row: usize,
    outcome: usize,
) -> Result<usize, ClearWorkFaultV2> {
    let n = widths.orders as usize;
    let u = widths.owners as usize;
    let o = widths.outcomes as usize;
    if outcome >= o {
        return Err(ClearWorkFaultV2::InvalidIndex);
    }
    let (base, rows) = match matrix {
        MatrixU64V2::ScratchBuy => (layout.span(ClearWorkRegionV2::Scratch).offset, n),
        MatrixU64V2::ScratchSell => (
            layout.span(ClearWorkRegionV2::Scratch).offset + 8 * n * o,
            n,
        ),
        MatrixU64V2::ParticipationBuy => (layout.span(ClearWorkRegionV2::Flows).offset + 32 * o, u),
        MatrixU64V2::ParticipationSell => (
            layout.span(ClearWorkRegionV2::Flows).offset + 32 * o + 8 * u * o,
            u,
        ),
    };
    if row >= rows {
        return Err(ClearWorkFaultV2::InvalidIndex);
    }
    Ok(base + 8 * (row * o + outcome))
}

fn owner_units_offset(
    layout: LayoutV2,
    widths: ClearWorkWidthsV2,
    column: OwnerUnitsV2,
    owner: usize,
) -> Result<usize, ClearWorkFaultV2> {
    let u = widths.owners as usize;
    if owner >= u {
        return Err(ClearWorkFaultV2::InvalidIndex);
    }
    let column = match column {
        OwnerUnitsV2::Reserved => 0,
        OwnerUnitsV2::Debit => 1,
        OwnerUnitsV2::Credit => 2,
        OwnerUnitsV2::FeeBps => 3,
    };
    Ok(layout.span(ClearWorkRegionV2::Ledger).offset + 16 * (column * u + owner))
}

fn outcome_flow_offset(
    layout: LayoutV2,
    widths: ClearWorkWidthsV2,
    side: OutcomeFlowV2,
    outcome: usize,
) -> Result<usize, ClearWorkFaultV2> {
    let o = widths.outcomes as usize;
    if outcome >= o {
        return Err(ClearWorkFaultV2::InvalidIndex);
    }
    let side = match side {
        OutcomeFlowV2::Buy => 0,
        OutcomeFlowV2::Sell => 1,
    };
    Ok(layout.span(ClearWorkRegionV2::Flows).offset + 16 * (side * o + outcome))
}

fn outcome_aggregate_offset(
    layout: LayoutV2,
    widths: ClearWorkWidthsV2,
    column: OutcomeAggregateV2,
    outcome: usize,
) -> Result<usize, ClearWorkFaultV2> {
    if outcome >= widths.outcomes as usize {
        return Err(ClearWorkFaultV2::InvalidIndex);
    }
    Ok(layout.span(ClearWorkRegionV2::Pools).offset + 128 * outcome + 16 * column as usize)
}

fn require_bool(value: u8) -> Result<bool, ClearWorkFaultV2> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ClearWorkFaultV2::InvalidBool),
    }
}

fn validate_error(bytes: &[u8]) -> Result<(), ClearWorkFaultV2> {
    let code = bytes[0];
    let outcome = bytes[1];
    let owner = u16::from_le_bytes([bytes[2], bytes[3]]);
    let payload_ok = match code {
        30 => true,
        47 => owner == 0,
        0..=46 => outcome == 0 && owner == 0,
        _ => false,
    };
    if !payload_ok {
        return Err(ClearWorkFaultV2::InvalidErrorCode);
    }
    Ok(())
}

fn read_u16(input: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([input[at], input[at + 1]])
}

fn read_u64(input: &[u8], at: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&input[at..at + 8]);
    u64::from_le_bytes(bytes)
}

fn read_u128(input: &[u8], at: usize) -> u128 {
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&input[at..at + 16]);
    u128::from_le_bytes(bytes)
}

fn read_fold(input: &[u8], at: usize) -> FoldWordsV2 {
    FoldWordsV2 {
        high: read_u64(input, at),
        low: read_u64(input, at + 8),
    }
}

fn write_u64(output: &mut [u8], at: usize, value: u64) {
    output[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_u128(output: &mut [u8], at: usize, value: u128) {
    output[at..at + 16].copy_from_slice(&value.to_le_bytes());
}

fn append(out: &mut [u8], cursor: &mut usize, source: &[u8], at: usize, len: usize) {
    out[*cursor..*cursor + len].copy_from_slice(&source[at..at + len]);
    *cursor += len;
}

fn append_matrix(
    out: &mut [u8],
    cursor: &mut usize,
    source: &[u8],
    at: usize,
    rows: usize,
    cols: usize,
    cell: usize,
) {
    let stride = MAX_OUTCOMES * cell;
    let mut row = 0usize;
    while row < rows {
        append(out, cursor, source, at + row * stride, cols * cell);
        row += 1;
    }
}

fn project_payload(v1: &[u8], widths: ClearWorkWidthsV2, out: &mut [u8]) {
    let n = widths.orders as usize;
    let u = widths.owners as usize;
    let o = widths.outcomes as usize;
    let mut cursor = 0usize;

    append(
        out,
        &mut cursor,
        v1,
        V1_CONTROL_AT,
        V1_CONTROL_BYTES + V1_DOMAIN_BYTES,
    );
    append(out, &mut cursor, v1, V1_CAND_AT, 1);
    append(out, &mut cursor, v1, V1_CAND_PRICES_AT, o * 8);
    append(out, &mut cursor, v1, V1_CAND_TAIL_AT, V1_CAND_TAIL_BYTES);
    append(out, &mut cursor, v1, V1_OWNERS_AT, u * 2);
    append(out, &mut cursor, v1, V1_OWNER_SLOTS_AT, 2);

    append(out, &mut cursor, v1, V1_OWNER_SLOT_AT, n * 2);
    append(out, &mut cursor, v1, V1_SIDE_BUY_BITS_AT, 8);
    append(out, &mut cursor, v1, V1_TOUCH_AT, n * 2);
    append(out, &mut cursor, v1, V1_CLASSES_AT, n);
    append(out, &mut cursor, v1, V1_FLAGS_AT, n);
    append(out, &mut cursor, v1, V1_CANCELLED_AT, n * 8);
    append(out, &mut cursor, v1, V1_KEYS_AT, n * 59);

    append_matrix(out, &mut cursor, v1, V1_SCRATCH_BUY_AT, n, o, 8);
    append_matrix(out, &mut cursor, v1, V1_SCRATCH_SELL_AT, n, o, 8);
    append(out, &mut cursor, v1, V1_CELL_PORTFOLIO_AT, u * 2);

    append(out, &mut cursor, v1, V1_FLOW_BUY_AT, o * 16);
    append(out, &mut cursor, v1, V1_FLOW_SELL_AT, o * 16);
    append_matrix(out, &mut cursor, v1, V1_PART_BUY_AT, u, o, 8);
    append_matrix(out, &mut cursor, v1, V1_PART_SELL_AT, u, o, 8);

    append(out, &mut cursor, v1, V1_AGG_AT, o * 128);
    append(out, &mut cursor, v1, V1_POOLS_AT, 2 * o * 36);
    let mut array = 0usize;
    while array < 4 {
        append(
            out,
            &mut cursor,
            v1,
            V1_RESERVED_UNITS_AT + array * MAX_OWNER_SLOTS * 16,
            u * 16,
        );
        array += 1;
    }
    array = 0;
    while array < 3 {
        append(
            out,
            &mut cursor,
            v1,
            V1_LEDGER_EGG_AT + array * MAX_OUTCOMES * 8,
            o * 8,
        );
        array += 1;
    }
    append(out, &mut cursor, v1, V1_CASH_SCALARS_AT, 8 * 16);
    array = 0;
    while array < 2 {
        append(
            out,
            &mut cursor,
            v1,
            V1_SPLIT_USED_AT + array * MAX_OUTCOMES * 8,
            o * 8,
        );
        array += 1;
    }

    append(out, &mut cursor, v1, V1_SUMMARY_AT, 1);
    let summary_flows = V1_SUMMARY_AT + 1;
    array = 0;
    while array < 4 {
        append(
            out,
            &mut cursor,
            v1,
            summary_flows + array * MAX_OUTCOMES * 8,
            o * 8,
        );
        array += 1;
    }
    let summary_virtual = summary_flows + 4 * MAX_OUTCOMES * 8;
    append(out, &mut cursor, v1, summary_virtual, 16);
    let summary_eggs = summary_virtual + 16;
    array = 0;
    while array < 3 {
        append(
            out,
            &mut cursor,
            v1,
            summary_eggs + array * MAX_OUTCOMES * 8,
            o * 8,
        );
        array += 1;
    }
    let summary_tail = summary_eggs + 3 * MAX_OUTCOMES * 8;
    append(out, &mut cursor, v1, summary_tail, 260);
    append(out, &mut cursor, v1, V1_SUMMARY_VALID_AT, 1);
    debug_assert_eq!(cursor, out.len());
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], ClearWorkFaultV2> {
    let end = cursor
        .checked_add(len)
        .ok_or(ClearWorkFaultV2::WrongLength)?;
    let value = input
        .get(*cursor..end)
        .ok_or(ClearWorkFaultV2::WrongLength)?;
    *cursor = end;
    Ok(value)
}

fn place(target: &mut [u8], at: usize, source: &[u8]) {
    target[at..at + source.len()].copy_from_slice(source);
}

fn place_matrix(
    target: &mut [u8],
    at: usize,
    input: &[u8],
    cursor: &mut usize,
    rows: usize,
    cols: usize,
    cell: usize,
) -> Result<(), ClearWorkFaultV2> {
    let stride = MAX_OUTCOMES * cell;
    let mut row = 0usize;
    while row < rows {
        place(target, at + row * stride, take(input, cursor, cols * cell)?);
        row += 1;
    }
    Ok(())
}

fn expand_payload_into_v1(
    input: &[u8],
    widths: ClearWorkWidthsV2,
    target: &mut [u8],
) -> Result<(), ClearWorkFaultV2> {
    if input.len() != clear_work_v2_body_len(widths) || target.len() != CLEAR_WORK_V1_BODY_BYTES {
        return Err(ClearWorkFaultV2::WrongLength);
    }
    ClearWorkV1::encode_idle_into(target).map_err(ClearWorkFaultV2::V1Codec)?;
    let n = widths.orders as usize;
    let u = widths.owners as usize;
    let o = widths.outcomes as usize;
    let mut cursor = 0usize;

    place(
        target,
        V1_CONTROL_AT,
        take(input, &mut cursor, V1_CONTROL_BYTES + V1_DOMAIN_BYTES)?,
    );
    place(target, V1_CAND_AT, take(input, &mut cursor, 1)?);
    place(target, V1_CAND_PRICES_AT, take(input, &mut cursor, o * 8)?);
    place(
        target,
        V1_CAND_TAIL_AT,
        take(input, &mut cursor, V1_CAND_TAIL_BYTES)?,
    );
    place(target, V1_OWNERS_AT, take(input, &mut cursor, u * 2)?);
    place(target, V1_OWNER_SLOTS_AT, take(input, &mut cursor, 2)?);

    place(target, V1_OWNER_SLOT_AT, take(input, &mut cursor, n * 2)?);
    place(target, V1_SIDE_BUY_BITS_AT, take(input, &mut cursor, 8)?);
    place(target, V1_TOUCH_AT, take(input, &mut cursor, n * 2)?);
    place(target, V1_CLASSES_AT, take(input, &mut cursor, n)?);
    place(target, V1_FLAGS_AT, take(input, &mut cursor, n)?);
    place(target, V1_CANCELLED_AT, take(input, &mut cursor, n * 8)?);
    place(target, V1_KEYS_AT, take(input, &mut cursor, n * 59)?);

    place_matrix(target, V1_SCRATCH_BUY_AT, input, &mut cursor, n, o, 8)?;
    place_matrix(target, V1_SCRATCH_SELL_AT, input, &mut cursor, n, o, 8)?;
    place(
        target,
        V1_CELL_PORTFOLIO_AT,
        take(input, &mut cursor, u * 2)?,
    );
    place(target, V1_FLOW_BUY_AT, take(input, &mut cursor, o * 16)?);
    place(target, V1_FLOW_SELL_AT, take(input, &mut cursor, o * 16)?);
    place_matrix(target, V1_PART_BUY_AT, input, &mut cursor, u, o, 8)?;
    place_matrix(target, V1_PART_SELL_AT, input, &mut cursor, u, o, 8)?;
    place(target, V1_AGG_AT, take(input, &mut cursor, o * 128)?);
    place(target, V1_POOLS_AT, take(input, &mut cursor, 2 * o * 36)?);

    let mut array = 0usize;
    while array < 4 {
        place(
            target,
            V1_RESERVED_UNITS_AT + array * MAX_OWNER_SLOTS * 16,
            take(input, &mut cursor, u * 16)?,
        );
        array += 1;
    }
    array = 0;
    while array < 3 {
        place(
            target,
            V1_LEDGER_EGG_AT + array * MAX_OUTCOMES * 8,
            take(input, &mut cursor, o * 8)?,
        );
        array += 1;
    }
    place(
        target,
        V1_CASH_SCALARS_AT,
        take(input, &mut cursor, 8 * 16)?,
    );
    array = 0;
    while array < 2 {
        place(
            target,
            V1_SPLIT_USED_AT + array * MAX_OUTCOMES * 8,
            take(input, &mut cursor, o * 8)?,
        );
        array += 1;
    }

    place(target, V1_SUMMARY_AT, take(input, &mut cursor, 1)?);
    let summary_flows = V1_SUMMARY_AT + 1;
    array = 0;
    while array < 4 {
        place(
            target,
            summary_flows + array * MAX_OUTCOMES * 8,
            take(input, &mut cursor, o * 8)?,
        );
        array += 1;
    }
    let summary_virtual = summary_flows + 4 * MAX_OUTCOMES * 8;
    place(target, summary_virtual, take(input, &mut cursor, 16)?);
    let summary_eggs = summary_virtual + 16;
    array = 0;
    while array < 3 {
        place(
            target,
            summary_eggs + array * MAX_OUTCOMES * 8,
            take(input, &mut cursor, o * 8)?,
        );
        array += 1;
    }
    let summary_tail = summary_eggs + 3 * MAX_OUTCOMES * 8;
    place(target, summary_tail, take(input, &mut cursor, 260)?);
    place(target, V1_SUMMARY_VALID_AT, take(input, &mut cursor, 1)?);
    if cursor != input.len() {
        return Err(ClearWorkFaultV2::WrongLength);
    }
    Ok(())
}
