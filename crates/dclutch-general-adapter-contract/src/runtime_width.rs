//! Runtime-width, borrowed wire records for the successor General vertical.
//!
//! These records are deliberately transport-only.  They authenticate neither
//! accounts nor signers, perform no CPI, and impose no semantic maximum on the
//! number of outcomes or rows.  Generic Trading authenticates those facts and
//! owns the only effect projection.  This module merely makes the width carried
//! by each hostile record explicit and checks every derived byte offset before
//! borrowing it.
//!
//! In particular, the assigned `112 + 16N` Execution geometry deliberately
//! contains an order binding, filled lots, and claim vectors only.  It does
//! **not** carry V1's order debit cap or per-row quote fragments.  Therefore
//! this codec is not a semantic replacement for V1 order-price-limit or quote
//! rounding verification: the authenticated immutable order must carry that
//! cap, and Generic Trading's streamed verifier must derive and aggregate the
//! exact debit and credit quantities before it commits effects.

use core::convert::TryFrom;

/// Version accepted by every successor General record in this module.
pub const RUNTIME_WIDTH_VERSION_V2: u16 = 2;
/// Exact fixed bytes before the Candidate simplex tail.
pub const CANDIDATE_HEADER_BYTES_V2: usize = 128;
/// Exact fixed bytes before the Execution receive and deliver tails.
pub const EXECUTION_HEADER_BYTES_V2: usize = 112;
/// Exact fixed bytes before the Page execution rows.
pub const PAGE_HEADER_BYTES_V2: usize = 64;
/// Exact fixed bytes before the Settlement Cursor inventory tail.
pub const SETTLEMENT_CURSOR_HEADER_BYTES_V2: usize = 88;
/// Exact fixed bytes before the Verified Candidate input and output tails.
pub const VERIFIED_CANDIDATE_HEADER_BYTES_V2: usize = 160;

const CANDIDATE_MAGIC: [u8; 8] = *b"DCGCAN02";
const EXECUTION_MAGIC: [u8; 8] = *b"DCGEXE02";
const PAGE_MAGIC: [u8; 8] = *b"DCGPAG02";
const SETTLEMENT_CURSOR_MAGIC: [u8; 8] = *b"DCGSET02";
const VERIFIED_CANDIDATE_MAGIC: [u8; 8] = *b"DCGVER02";

const CANDIDATE_PHASE: u8 = 1;
const EXECUTION_PHASE: u8 = 2;
const PAGE_PHASE: u8 = 3;
const VERIFIED_PHASE: u8 = 9;

/// Stable refusal from a hostile successor-General runtime-width record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeWidthErrorV2 {
    /// The supplied byte slice did not have its exact count-derived width.
    InvalidLength,
    /// A checked count, stride, or byte offset did not fit the host address space.
    ArithmeticOverflow,
    /// The record belonged to another wire family.
    InvalidMagic,
    /// The record version did not select this exact grammar.
    InvalidVersion,
    /// A record phase tag was unknown or inappropriate for that record family.
    InvalidPhase,
    /// A reserved byte or word was nonzero.
    NonCanonicalPadding,
    /// A required content identity, coordinate, or scalar was zero.
    ZeroCoordinate,
    /// A declared outcome width, page coordinate, or cursor was inconsistent.
    InvalidCursor,
    /// A Candidate price tail was not an exact nonnegative simplex.
    InvalidSimplex,
    /// An embedded execution was not bound to its enclosing Page.
    Substitution,
    /// A row coordinate was duplicated or not strictly increasing.
    NonCanonicalRows,
    /// An indexed outcome or row was not in the declared runtime width.
    IndexOutOfBounds,
}

/// Result alias for successor-General runtime-width decoding.
pub type RuntimeWidthResultV2<T> = core::result::Result<T, RuntimeWidthErrorV2>;

/// Settlement progress owned by a [`SettlementCursorV2`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementPhaseV2 {
    /// Pages are being collected into the settlement compartment.
    Collecting,
    /// Collection is complete; the unique materialization remains.
    Materializing,
    /// Pages are being distributed from the settlement compartment.
    Distributing,
    /// Distribution is complete; only terminal close remains.
    ReadyToClose,
    /// The cursor is terminal and owns no further progression authority.
    Terminal,
}

impl SettlementPhaseV2 {
    fn decode(tag: u8) -> RuntimeWidthResultV2<Self> {
        match tag {
            4 => Ok(Self::Collecting),
            5 => Ok(Self::Materializing),
            6 => Ok(Self::Distributing),
            7 => Ok(Self::ReadyToClose),
            8 => Ok(Self::Terminal),
            _ => Err(RuntimeWidthErrorV2::InvalidPhase),
        }
    }

    fn tag(self) -> u8 {
        match self {
            Self::Collecting => 4,
            Self::Materializing => 5,
            Self::Distributing => 6,
            Self::ReadyToClose => 7,
            Self::Terminal => 8,
        }
    }
}

/// Caller-owned fixed Candidate fields before its runtime simplex tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateHeaderV2 {
    /// Declared runtime outcome width.
    pub outcome_count: u32,
    /// Number of separately authenticated candidate pages.
    pub page_count: u32,
    /// Nonzero immutable candidate coordinate within the batch.
    pub candidate_coordinate: u32,
    /// Sole exact denominator for the price simplex.
    pub price_scale: u64,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Product content identity.
    pub product_id: [u8; 32],
    /// Batch content identity.
    pub batch_id: [u8; 32],
}

/// Borrowed Candidate record with an exact `u64` simplex tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateV2<'a> {
    bytes: &'a [u8],
    header: CandidateHeaderV2,
}

impl<'a> CandidateV2<'a> {
    /// Hostile-decode one exact `128 + 8N` Candidate record.
    pub fn decode(bytes: &'a [u8]) -> RuntimeWidthResultV2<Self> {
        header(
            bytes,
            CANDIDATE_HEADER_BYTES_V2,
            &CANDIDATE_MAGIC,
            CANDIDATE_PHASE,
        )?;
        let header = CandidateHeaderV2 {
            outcome_count: u32_at(bytes, 12)?,
            page_count: u32_at(bytes, 16)?,
            candidate_coordinate: u32_at(bytes, 20)?,
            price_scale: u64_at(bytes, 24)?,
            candidate_id: array32_at(bytes, 32)?,
            product_id: array32_at(bytes, 64)?,
            batch_id: array32_at(bytes, 96)?,
        };
        exact_width(bytes, candidate_len(header.outcome_count)?)?;
        validate_candidate_header(header)?;
        let mut total = 0_u64;
        for index in 0..outcome_len(header.outcome_count)? {
            total = total
                .checked_add(u64_at(
                    bytes,
                    tail_offset(CANDIDATE_HEADER_BYTES_V2, index, 8)?,
                )?)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?;
        }
        if total != header.price_scale {
            return Err(RuntimeWidthErrorV2::InvalidSimplex);
        }
        Ok(Self { bytes, header })
    }

    /// Encode canonical Candidate bytes into a caller-owned exact-width buffer.
    pub fn encode_into(
        header_value: CandidateHeaderV2,
        prices: &[u64],
        output: &mut [u8],
    ) -> RuntimeWidthResultV2<()> {
        validate_candidate_header(header_value)?;
        let count = outcome_len(header_value.outcome_count)?;
        if prices.len() != count {
            return Err(RuntimeWidthErrorV2::InvalidLength);
        }
        exact_width(output, candidate_len(header_value.outcome_count)?)?;
        let total = prices.iter().try_fold(0_u64, |sum, value| {
            sum.checked_add(*value)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)
        })?;
        if total != header_value.price_scale {
            return Err(RuntimeWidthErrorV2::InvalidSimplex);
        }
        output.fill(0);
        write_header(output, &CANDIDATE_MAGIC, CANDIDATE_PHASE)?;
        put_u32(output, 12, header_value.outcome_count)?;
        put_u32(output, 16, header_value.page_count)?;
        put_u32(output, 20, header_value.candidate_coordinate)?;
        put_u64(output, 24, header_value.price_scale)?;
        put(output, 32, &header_value.candidate_id)?;
        put(output, 64, &header_value.product_id)?;
        put(output, 96, &header_value.batch_id)?;
        for (index, value) in prices.iter().enumerate() {
            put_u64(
                output,
                tail_offset(CANDIDATE_HEADER_BYTES_V2, index, 8)?,
                *value,
            )?;
        }
        Ok(())
    }

    /// Return fixed Candidate coordinates.
    pub const fn header(self) -> CandidateHeaderV2 {
        self.header
    }

    /// Return the canonical simplex price at one checked runtime index.
    pub fn price(self, index: u32) -> RuntimeWidthResultV2<u64> {
        index_at(self.header.outcome_count, index)?;
        u64_at(
            self.bytes,
            tail_offset(CANDIDATE_HEADER_BYTES_V2, usize_from_u32(index)?, 8)?,
        )
    }

    /// Return the exact hostile-decoded bytes after validation.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Caller-owned fixed Execution fields before two runtime-width claim tails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionHeaderV2 {
    /// Declared runtime outcome width.
    pub outcome_count: u32,
    /// Nonzero enclosing page coordinate.
    pub page_coordinate: u32,
    /// Nonzero execution coordinate, strictly ordered in a Page.
    pub execution_coordinate: u32,
    /// Immutable signed-order nonce.
    pub nonce: u64,
    /// Immutable order content identity.
    pub order_id: [u8; 32],
    /// Immutable owner identity.
    pub owner_id: [u8; 32],
    /// Candidate-wide maximum fill for this order.
    pub max_lots: u64,
    /// Positive fill represented by this row.
    pub lots: u64,
}

/// Borrowed compact Execution transport record with receive and deliver tails.
///
/// Its immutable `order_id` binds the order record that owns quote-limit
/// semantics.  This wire shape intentionally cannot itself prove a per-row
/// quote debit, credit, or candidate-wide rounding relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionV2<'a> {
    bytes: &'a [u8],
    header: ExecutionHeaderV2,
}

impl<'a> ExecutionV2<'a> {
    /// Hostile-decode one exact `112 + 16N` Execution record.
    pub fn decode(bytes: &'a [u8]) -> RuntimeWidthResultV2<Self> {
        header(
            bytes,
            EXECUTION_HEADER_BYTES_V2,
            &EXECUTION_MAGIC,
            EXECUTION_PHASE,
        )?;
        let header = ExecutionHeaderV2 {
            outcome_count: u32_at(bytes, 12)?,
            page_coordinate: u32_at(bytes, 16)?,
            execution_coordinate: u32_at(bytes, 20)?,
            nonce: u64_at(bytes, 24)?,
            order_id: array32_at(bytes, 32)?,
            owner_id: array32_at(bytes, 64)?,
            max_lots: u64_at(bytes, 96)?,
            lots: u64_at(bytes, 104)?,
        };
        exact_width(bytes, execution_len(header.outcome_count)?)?;
        validate_execution_header(header)?;
        Ok(Self { bytes, header })
    }

    /// Encode canonical Execution bytes into a caller-owned exact-width buffer.
    pub fn encode_into(
        header_value: ExecutionHeaderV2,
        receive_per_lot: &[u64],
        deliver_per_lot: &[u64],
        output: &mut [u8],
    ) -> RuntimeWidthResultV2<()> {
        validate_execution_header(header_value)?;
        let count = outcome_len(header_value.outcome_count)?;
        if receive_per_lot.len() != count || deliver_per_lot.len() != count {
            return Err(RuntimeWidthErrorV2::InvalidLength);
        }
        exact_width(output, execution_len(header_value.outcome_count)?)?;
        output.fill(0);
        write_header(output, &EXECUTION_MAGIC, EXECUTION_PHASE)?;
        put_u32(output, 12, header_value.outcome_count)?;
        put_u32(output, 16, header_value.page_coordinate)?;
        put_u32(output, 20, header_value.execution_coordinate)?;
        put_u64(output, 24, header_value.nonce)?;
        put(output, 32, &header_value.order_id)?;
        put(output, 64, &header_value.owner_id)?;
        put_u64(output, 96, header_value.max_lots)?;
        put_u64(output, 104, header_value.lots)?;
        let deliver_offset = execution_deliver_offset(header_value.outcome_count)?;
        for (index, value) in receive_per_lot.iter().enumerate() {
            put_u64(
                output,
                tail_offset(EXECUTION_HEADER_BYTES_V2, index, 8)?,
                *value,
            )?;
        }
        for (index, value) in deliver_per_lot.iter().enumerate() {
            put_u64(output, tail_offset(deliver_offset, index, 8)?, *value)?;
        }
        Ok(())
    }

    /// Return fixed Execution coordinates.
    pub const fn header(self) -> ExecutionHeaderV2 {
        self.header
    }

    /// Return a checked received-claim quantity per lot.
    pub fn receive_per_lot(self, index: u32) -> RuntimeWidthResultV2<u64> {
        index_at(self.header.outcome_count, index)?;
        u64_at(
            self.bytes,
            tail_offset(EXECUTION_HEADER_BYTES_V2, usize_from_u32(index)?, 8)?,
        )
    }

    /// Return a checked delivered-claim quantity per lot.
    pub fn deliver_per_lot(self, index: u32) -> RuntimeWidthResultV2<u64> {
        index_at(self.header.outcome_count, index)?;
        u64_at(
            self.bytes,
            tail_offset(
                execution_deliver_offset(self.header.outcome_count)?,
                usize_from_u32(index)?,
                8,
            )?,
        )
    }

    /// Return the exact hostile-decoded bytes after validation.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Caller-owned Page coordinates before compact Execution rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageHeaderV2 {
    /// Declared runtime outcome width shared by every embedded Execution.
    pub outcome_count: u32,
    /// Nonzero page coordinate.
    pub page_coordinate: u32,
    /// Number of pages declared by the parent Candidate.
    pub page_count: u32,
    /// Nonzero optimistic revision consumed by this Page.
    pub revision: u64,
    /// Candidate content identity to which this Page is bound.
    pub candidate_id: [u8; 32],
}

/// Borrowed Page with an exact number of compact runtime-width Execution rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageV2<'a> {
    bytes: &'a [u8],
    header: PageHeaderV2,
    row_count: u32,
}

impl<'a> PageV2<'a> {
    /// Hostile-decode one exact `64 + rows * (112 + 16N)` Page record.
    pub fn decode(bytes: &'a [u8]) -> RuntimeWidthResultV2<Self> {
        header(bytes, PAGE_HEADER_BYTES_V2, &PAGE_MAGIC, PAGE_PHASE)?;
        let header = PageHeaderV2 {
            outcome_count: u32_at(bytes, 12)?,
            page_coordinate: u32_at(bytes, 16)?,
            page_count: u32_at(bytes, 20)?,
            revision: u64_at(bytes, 24)?,
            candidate_id: array32_at(bytes, 32)?,
        };
        validate_page_header(header)?;
        let execution_width = execution_len(header.outcome_count)?;
        let tail = bytes
            .len()
            .checked_sub(PAGE_HEADER_BYTES_V2)
            .ok_or(RuntimeWidthErrorV2::InvalidLength)?;
        if tail % execution_width != 0 {
            return Err(RuntimeWidthErrorV2::InvalidLength);
        }
        let rows = tail / execution_width;
        let row_count = u32::try_from(rows).map_err(|_| RuntimeWidthErrorV2::ArithmeticOverflow)?;
        for index in 0..rows {
            let offset = row_offset(index, execution_width)?;
            let end = offset
                .checked_add(execution_width)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?;
            let row = bytes
                .get(offset..end)
                .ok_or(RuntimeWidthErrorV2::InvalidLength)?;
            let execution = ExecutionV2::decode(row)?;
            if execution.header.outcome_count != header.outcome_count
                || execution.header.page_coordinate != header.page_coordinate
            {
                return Err(RuntimeWidthErrorV2::Substitution);
            }
            let expected = u32::try_from(index)
                .map_err(|_| RuntimeWidthErrorV2::ArithmeticOverflow)?
                .checked_add(1)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?;
            if execution.header.execution_coordinate != expected {
                return Err(RuntimeWidthErrorV2::NonCanonicalRows);
            }
        }
        Ok(Self {
            bytes,
            header,
            row_count,
        })
    }

    /// Encode a compact Page from already canonical Execution rows.
    pub fn encode_into(
        header_value: PageHeaderV2,
        rows: &[&[u8]],
        output: &mut [u8],
    ) -> RuntimeWidthResultV2<()> {
        validate_page_header(header_value)?;
        let count =
            u32::try_from(rows.len()).map_err(|_| RuntimeWidthErrorV2::ArithmeticOverflow)?;
        exact_width(output, page_len(header_value.outcome_count, count)?)?;
        let width = execution_len(header_value.outcome_count)?;
        output.fill(0);
        write_header(output, &PAGE_MAGIC, PAGE_PHASE)?;
        put_u32(output, 12, header_value.outcome_count)?;
        put_u32(output, 16, header_value.page_coordinate)?;
        put_u32(output, 20, header_value.page_count)?;
        put_u64(output, 24, header_value.revision)?;
        put(output, 32, &header_value.candidate_id)?;
        for (index, row) in rows.iter().enumerate() {
            let execution = ExecutionV2::decode(row)?;
            if execution.header.outcome_count != header_value.outcome_count
                || execution.header.page_coordinate != header_value.page_coordinate
            {
                return Err(RuntimeWidthErrorV2::Substitution);
            }
            let expected = u32::try_from(index)
                .map_err(|_| RuntimeWidthErrorV2::ArithmeticOverflow)?
                .checked_add(1)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?;
            if execution.header.execution_coordinate != expected {
                return Err(RuntimeWidthErrorV2::NonCanonicalRows);
            }
            let offset = row_offset(index, width)?;
            put(output, offset, row)?;
        }
        Ok(())
    }

    /// Return fixed Page coordinates.
    pub const fn header(self) -> PageHeaderV2 {
        self.header
    }

    /// Return the exact number of compact rows.
    pub const fn row_count(self) -> u32 {
        self.row_count
    }

    /// Borrow and validate one indexed Execution row.
    pub fn execution(self, index: u32) -> RuntimeWidthResultV2<ExecutionV2<'a>> {
        index_at(self.row_count, index)?;
        let width = execution_len(self.header.outcome_count)?;
        let start = row_offset(usize_from_u32(index)?, width)?;
        let end = start
            .checked_add(width)
            .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?;
        ExecutionV2::decode(
            self.bytes
                .get(start..end)
                .ok_or(RuntimeWidthErrorV2::InvalidLength)?,
        )
    }

    /// Return the exact hostile-decoded bytes after validation.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Caller-owned Settlement Cursor fields before its runtime inventory tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCursorHeaderV2 {
    /// Declared runtime outcome width.
    pub outcome_count: u32,
    /// Total verifier-emitted per-order settlement rows.
    pub order_count: u32,
    /// Exact next order coordinate for the current phase.
    pub next_order: u32,
    /// Nonzero optimistic cursor revision.
    pub revision: u64,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Quote inventory held by the settlement compartment.
    pub quote_inventory: u64,
    /// Complete-set quantity already materialized.
    pub complete_set_quantity: u64,
    /// Nonzero only after terminal close, and unique to that terminal event.
    pub terminal_coordinate: u64,
    /// Progress phase.
    pub phase: SettlementPhaseV2,
}

/// Borrowed Settlement Cursor with one runtime-width inventory vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCursorV2<'a> {
    bytes: &'a [u8],
    header: SettlementCursorHeaderV2,
}

impl<'a> SettlementCursorV2<'a> {
    /// Hostile-decode one exact `88 + 8N` Settlement Cursor record.
    pub fn decode(bytes: &'a [u8]) -> RuntimeWidthResultV2<Self> {
        exact_width_at_least(bytes, SETTLEMENT_CURSOR_HEADER_BYTES_V2)?;
        require_magic(bytes, &SETTLEMENT_CURSOR_MAGIC)?;
        require_version(bytes)?;
        require_zero(bytes, 11, 1)?;
        let header = SettlementCursorHeaderV2 {
            outcome_count: u32_at(bytes, 12)?,
            order_count: u32_at(bytes, 16)?,
            next_order: u32_at(bytes, 20)?,
            revision: u64_at(bytes, 24)?,
            candidate_id: array32_at(bytes, 32)?,
            quote_inventory: u64_at(bytes, 64)?,
            complete_set_quantity: u64_at(bytes, 72)?,
            terminal_coordinate: u64_at(bytes, 80)?,
            phase: SettlementPhaseV2::decode(byte_at(bytes, 10)?)?,
        };
        exact_width(bytes, settlement_cursor_len(header.outcome_count)?)?;
        validate_settlement_cursor_header(header)?;
        Ok(Self { bytes, header })
    }

    /// Encode canonical Settlement Cursor bytes into a caller-owned buffer.
    pub fn encode_into(
        header_value: SettlementCursorHeaderV2,
        inventory: &[u64],
        output: &mut [u8],
    ) -> RuntimeWidthResultV2<()> {
        validate_settlement_cursor_header(header_value)?;
        if inventory.len() != outcome_len(header_value.outcome_count)? {
            return Err(RuntimeWidthErrorV2::InvalidLength);
        }
        exact_width(output, settlement_cursor_len(header_value.outcome_count)?)?;
        output.fill(0);
        put(output, 0, &SETTLEMENT_CURSOR_MAGIC)?;
        put_u16(output, 8, RUNTIME_WIDTH_VERSION_V2)?;
        put_byte(output, 10, header_value.phase.tag())?;
        put_u32(output, 12, header_value.outcome_count)?;
        put_u32(output, 16, header_value.order_count)?;
        put_u32(output, 20, header_value.next_order)?;
        put_u64(output, 24, header_value.revision)?;
        put(output, 32, &header_value.candidate_id)?;
        put_u64(output, 64, header_value.quote_inventory)?;
        put_u64(output, 72, header_value.complete_set_quantity)?;
        put_u64(output, 80, header_value.terminal_coordinate)?;
        for (index, value) in inventory.iter().enumerate() {
            put_u64(
                output,
                tail_offset(SETTLEMENT_CURSOR_HEADER_BYTES_V2, index, 8)?,
                *value,
            )?;
        }
        Ok(())
    }

    /// Encode a canonical cursor from an exact little-endian `u64[N]` inventory.
    ///
    /// This is the no-allocation settlement path: the caller derives the full
    /// successor inventory in non-authoritative scratch, then this method
    /// validates its exact width and commits the complete canonical record.
    pub fn encode_le_inventory_into(
        header_value: SettlementCursorHeaderV2,
        inventory_le: &[u8],
        output: &mut [u8],
    ) -> RuntimeWidthResultV2<()> {
        validate_settlement_cursor_header(header_value)?;
        let inventory_bytes = derived_len(0, header_value.outcome_count, 8)?;
        exact_width(inventory_le, inventory_bytes)?;
        exact_width(output, settlement_cursor_len(header_value.outcome_count)?)?;
        output.fill(0);
        put(output, 0, &SETTLEMENT_CURSOR_MAGIC)?;
        put_u16(output, 8, RUNTIME_WIDTH_VERSION_V2)?;
        put_byte(output, 10, header_value.phase.tag())?;
        put_u32(output, 12, header_value.outcome_count)?;
        put_u32(output, 16, header_value.order_count)?;
        put_u32(output, 20, header_value.next_order)?;
        put_u64(output, 24, header_value.revision)?;
        put(output, 32, &header_value.candidate_id)?;
        put_u64(output, 64, header_value.quote_inventory)?;
        put_u64(output, 72, header_value.complete_set_quantity)?;
        put_u64(output, 80, header_value.terminal_coordinate)?;
        put(output, SETTLEMENT_CURSOR_HEADER_BYTES_V2, inventory_le)
    }

    /// Return fixed Settlement Cursor coordinates.
    pub const fn header(self) -> SettlementCursorHeaderV2 {
        self.header
    }

    /// Return the inventory at one checked outcome index.
    pub fn inventory(self, index: u32) -> RuntimeWidthResultV2<u64> {
        index_at(self.header.outcome_count, index)?;
        u64_at(
            self.bytes,
            tail_offset(SETTLEMENT_CURSOR_HEADER_BYTES_V2, usize_from_u32(index)?, 8)?,
        )
    }

    /// Return the exact hostile-decoded bytes after validation.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Caller-owned Verified Candidate fields before two runtime aggregate tails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCandidateHeaderV2 {
    /// Declared runtime outcome width.
    pub outcome_count: u32,
    /// Authenticated Page count.
    pub page_count: u32,
    /// Nonzero Candidate coordinate.
    pub candidate_coordinate: u32,
    /// Nonzero verification revision.
    pub revision: u64,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Product content identity.
    pub product_id: [u8; 32],
    /// Batch content identity.
    pub batch_id: [u8; 32],
    /// Exact candidate-wide filled lots objective.
    pub filled_lots: u64,
    /// Exact aggregate quote debit.
    pub quote_debit: u64,
    /// Exact aggregate quote credit.
    pub quote_credit: u64,
    /// Nonzero price denominator inherited from the Candidate simplex.
    pub price_scale: u64,
}

/// Borrowed verified-candidate certificate with runtime input and output tails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCandidateV2<'a> {
    bytes: &'a [u8],
    header: VerifiedCandidateHeaderV2,
}

impl<'a> VerifiedCandidateV2<'a> {
    /// Hostile-decode one exact `160 + 16N` verified-candidate record.
    pub fn decode(bytes: &'a [u8]) -> RuntimeWidthResultV2<Self> {
        header(
            bytes,
            VERIFIED_CANDIDATE_HEADER_BYTES_V2,
            &VERIFIED_CANDIDATE_MAGIC,
            VERIFIED_PHASE,
        )?;
        let header = VerifiedCandidateHeaderV2 {
            outcome_count: u32_at(bytes, 12)?,
            page_count: u32_at(bytes, 16)?,
            candidate_coordinate: u32_at(bytes, 20)?,
            revision: u64_at(bytes, 24)?,
            candidate_id: array32_at(bytes, 32)?,
            product_id: array32_at(bytes, 64)?,
            batch_id: array32_at(bytes, 96)?,
            filled_lots: u64_at(bytes, 128)?,
            quote_debit: u64_at(bytes, 136)?,
            quote_credit: u64_at(bytes, 144)?,
            price_scale: u64_at(bytes, 152)?,
        };
        exact_width(bytes, verified_candidate_len(header.outcome_count)?)?;
        validate_verified_candidate_header(header)?;
        Ok(Self { bytes, header })
    }

    /// Encode canonical verified-candidate bytes into a caller-owned buffer.
    pub fn encode_into(
        header_value: VerifiedCandidateHeaderV2,
        claim_inputs: &[u64],
        claim_outputs: &[u64],
        output: &mut [u8],
    ) -> RuntimeWidthResultV2<()> {
        validate_verified_candidate_header(header_value)?;
        let count = outcome_len(header_value.outcome_count)?;
        if claim_inputs.len() != count || claim_outputs.len() != count {
            return Err(RuntimeWidthErrorV2::InvalidLength);
        }
        exact_width(output, verified_candidate_len(header_value.outcome_count)?)?;
        output.fill(0);
        write_header(output, &VERIFIED_CANDIDATE_MAGIC, VERIFIED_PHASE)?;
        put_u32(output, 12, header_value.outcome_count)?;
        put_u32(output, 16, header_value.page_count)?;
        put_u32(output, 20, header_value.candidate_coordinate)?;
        put_u64(output, 24, header_value.revision)?;
        put(output, 32, &header_value.candidate_id)?;
        put(output, 64, &header_value.product_id)?;
        put(output, 96, &header_value.batch_id)?;
        put_u64(output, 128, header_value.filled_lots)?;
        put_u64(output, 136, header_value.quote_debit)?;
        put_u64(output, 144, header_value.quote_credit)?;
        put_u64(output, 152, header_value.price_scale)?;
        let outputs = verified_outputs_offset(header_value.outcome_count)?;
        for (index, value) in claim_inputs.iter().enumerate() {
            put_u64(
                output,
                tail_offset(VERIFIED_CANDIDATE_HEADER_BYTES_V2, index, 8)?,
                *value,
            )?;
        }
        for (index, value) in claim_outputs.iter().enumerate() {
            put_u64(output, tail_offset(outputs, index, 8)?, *value)?;
        }
        Ok(())
    }

    /// Encode canonical little-endian aggregate tails without allocating `u64` arrays.
    ///
    /// Each tail must contain exactly `8N` bytes. Every eight-byte cell is an
    /// already-canonical little-endian `u64`; all byte patterns are therefore
    /// canonical. This is the streamed verifier path for runtime widths that
    /// cannot be represented by a fixed Rust array.
    pub fn encode_le_tails_into(
        header_value: VerifiedCandidateHeaderV2,
        claim_inputs_le: &[u8],
        claim_outputs_le: &[u8],
        output: &mut [u8],
    ) -> RuntimeWidthResultV2<()> {
        validate_verified_candidate_header(header_value)?;
        let tail_bytes = derived_len(0, header_value.outcome_count, 8)?;
        exact_width(claim_inputs_le, tail_bytes)?;
        exact_width(claim_outputs_le, tail_bytes)?;
        exact_width(output, verified_candidate_len(header_value.outcome_count)?)?;
        output.fill(0);
        write_header(output, &VERIFIED_CANDIDATE_MAGIC, VERIFIED_PHASE)?;
        put_u32(output, 12, header_value.outcome_count)?;
        put_u32(output, 16, header_value.page_count)?;
        put_u32(output, 20, header_value.candidate_coordinate)?;
        put_u64(output, 24, header_value.revision)?;
        put(output, 32, &header_value.candidate_id)?;
        put(output, 64, &header_value.product_id)?;
        put(output, 96, &header_value.batch_id)?;
        put_u64(output, 128, header_value.filled_lots)?;
        put_u64(output, 136, header_value.quote_debit)?;
        put_u64(output, 144, header_value.quote_credit)?;
        put_u64(output, 152, header_value.price_scale)?;
        put(output, VERIFIED_CANDIDATE_HEADER_BYTES_V2, claim_inputs_le)?;
        put(
            output,
            verified_outputs_offset(header_value.outcome_count)?,
            claim_outputs_le,
        )
    }

    /// Return fixed verified-candidate coordinates.
    pub const fn header(self) -> VerifiedCandidateHeaderV2 {
        self.header
    }

    /// Return an exact aggregate claim input at a checked outcome index.
    pub fn claim_input(self, index: u32) -> RuntimeWidthResultV2<u64> {
        index_at(self.header.outcome_count, index)?;
        u64_at(
            self.bytes,
            tail_offset(
                VERIFIED_CANDIDATE_HEADER_BYTES_V2,
                usize_from_u32(index)?,
                8,
            )?,
        )
    }

    /// Return an exact aggregate claim output at a checked outcome index.
    pub fn claim_output(self, index: u32) -> RuntimeWidthResultV2<u64> {
        index_at(self.header.outcome_count, index)?;
        u64_at(
            self.bytes,
            tail_offset(
                verified_outputs_offset(self.header.outcome_count)?,
                usize_from_u32(index)?,
                8,
            )?,
        )
    }

    /// Return the exact hostile-decoded bytes after validation.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Return the exact Candidate width for a hostile runtime outcome count.
pub fn candidate_len(outcome_count: u32) -> RuntimeWidthResultV2<usize> {
    derived_len(CANDIDATE_HEADER_BYTES_V2, outcome_count, 8)
}

/// Return the exact Execution width for a hostile runtime outcome count.
pub fn execution_len(outcome_count: u32) -> RuntimeWidthResultV2<usize> {
    derived_len(EXECUTION_HEADER_BYTES_V2, outcome_count, 16)
}

/// Return the exact Page width for hostile runtime outcome and row counts.
pub fn page_len(outcome_count: u32, rows: u32) -> RuntimeWidthResultV2<usize> {
    let row_bytes = execution_len(outcome_count)?;
    let rows = usize_from_u32(rows)?;
    PAGE_HEADER_BYTES_V2
        .checked_add(
            row_bytes
                .checked_mul(rows)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)
}

/// Return the exact Settlement Cursor width for a hostile runtime outcome count.
pub fn settlement_cursor_len(outcome_count: u32) -> RuntimeWidthResultV2<usize> {
    derived_len(SETTLEMENT_CURSOR_HEADER_BYTES_V2, outcome_count, 8)
}

/// Return the exact Verified Candidate width for a hostile runtime outcome count.
pub fn verified_candidate_len(outcome_count: u32) -> RuntimeWidthResultV2<usize> {
    derived_len(VERIFIED_CANDIDATE_HEADER_BYTES_V2, outcome_count, 16)
}

fn validate_candidate_header(value: CandidateHeaderV2) -> RuntimeWidthResultV2<()> {
    if value.outcome_count == 0
        || value.page_count == 0
        || value.candidate_coordinate == 0
        || value.price_scale == 0
        || zero(&value.candidate_id)
        || zero(&value.product_id)
        || zero(&value.batch_id)
    {
        return Err(RuntimeWidthErrorV2::ZeroCoordinate);
    }
    Ok(())
}

fn validate_execution_header(value: ExecutionHeaderV2) -> RuntimeWidthResultV2<()> {
    if value.outcome_count == 0
        || value.page_coordinate == 0
        || value.execution_coordinate == 0
        || value.max_lots == 0
        || value.lots == 0
        || zero(&value.order_id)
        || zero(&value.owner_id)
    {
        return Err(RuntimeWidthErrorV2::ZeroCoordinate);
    }
    if value.lots > value.max_lots {
        return Err(RuntimeWidthErrorV2::InvalidCursor);
    }
    Ok(())
}

fn validate_page_header(value: PageHeaderV2) -> RuntimeWidthResultV2<()> {
    if value.outcome_count == 0
        || value.page_coordinate == 0
        || value.page_count == 0
        || value.revision == 0
        || zero(&value.candidate_id)
    {
        return Err(RuntimeWidthErrorV2::ZeroCoordinate);
    }
    if value.page_coordinate > value.page_count {
        return Err(RuntimeWidthErrorV2::InvalidCursor);
    }
    Ok(())
}

fn validate_settlement_cursor_header(value: SettlementCursorHeaderV2) -> RuntimeWidthResultV2<()> {
    if value.outcome_count == 0
        || value.order_count == 0
        || value.revision == 0
        || zero(&value.candidate_id)
    {
        return Err(RuntimeWidthErrorV2::ZeroCoordinate);
    }
    let terminal = value.phase == SettlementPhaseV2::Terminal;
    let cursor_is_canonical = match value.phase {
        SettlementPhaseV2::Collecting | SettlementPhaseV2::Distributing => {
            value.next_order < value.order_count
        }
        SettlementPhaseV2::Materializing
        | SettlementPhaseV2::ReadyToClose
        | SettlementPhaseV2::Terminal => value.next_order == value.order_count,
    };
    if !cursor_is_canonical {
        return Err(RuntimeWidthErrorV2::InvalidCursor);
    }
    if terminal != (value.terminal_coordinate != 0) {
        return Err(RuntimeWidthErrorV2::InvalidCursor);
    }
    Ok(())
}

fn validate_verified_candidate_header(
    value: VerifiedCandidateHeaderV2,
) -> RuntimeWidthResultV2<()> {
    if value.outcome_count == 0
        || value.page_count == 0
        || value.candidate_coordinate == 0
        || value.revision == 0
        || value.price_scale == 0
        || zero(&value.candidate_id)
        || zero(&value.product_id)
        || zero(&value.batch_id)
    {
        return Err(RuntimeWidthErrorV2::ZeroCoordinate);
    }
    Ok(())
}

fn execution_deliver_offset(outcome_count: u32) -> RuntimeWidthResultV2<usize> {
    derived_len(EXECUTION_HEADER_BYTES_V2, outcome_count, 8)
}

fn verified_outputs_offset(outcome_count: u32) -> RuntimeWidthResultV2<usize> {
    derived_len(VERIFIED_CANDIDATE_HEADER_BYTES_V2, outcome_count, 8)
}

fn derived_len(header: usize, count: u32, item_width: usize) -> RuntimeWidthResultV2<usize> {
    let count = outcome_len(count)?;
    header
        .checked_add(
            count
                .checked_mul(item_width)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)
}

fn outcome_len(value: u32) -> RuntimeWidthResultV2<usize> {
    if value == 0 {
        return Err(RuntimeWidthErrorV2::ZeroCoordinate);
    }
    usize_from_u32(value)
}

fn usize_from_u32(value: u32) -> RuntimeWidthResultV2<usize> {
    usize::try_from(value).map_err(|_| RuntimeWidthErrorV2::ArithmeticOverflow)
}

fn index_at(width: u32, index: u32) -> RuntimeWidthResultV2<()> {
    if index >= width {
        Err(RuntimeWidthErrorV2::IndexOutOfBounds)
    } else {
        Ok(())
    }
}

fn row_offset(index: usize, width: usize) -> RuntimeWidthResultV2<usize> {
    PAGE_HEADER_BYTES_V2
        .checked_add(
            index
                .checked_mul(width)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)
}

fn tail_offset(header: usize, index: usize, width: usize) -> RuntimeWidthResultV2<usize> {
    header
        .checked_add(
            index
                .checked_mul(width)
                .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)
}

fn header(bytes: &[u8], width: usize, magic: &[u8; 8], phase: u8) -> RuntimeWidthResultV2<()> {
    exact_width_at_least(bytes, width)?;
    require_magic(bytes, magic)?;
    require_version(bytes)?;
    if byte_at(bytes, 10)? != phase {
        return Err(RuntimeWidthErrorV2::InvalidPhase);
    }
    require_zero(bytes, 11, 1)
}

fn write_header(output: &mut [u8], magic: &[u8; 8], phase: u8) -> RuntimeWidthResultV2<()> {
    put(output, 0, magic)?;
    put_u16(output, 8, RUNTIME_WIDTH_VERSION_V2)?;
    put_byte(output, 10, phase)
}

fn exact_width_at_least(bytes: &[u8], minimum: usize) -> RuntimeWidthResultV2<()> {
    if bytes.len() < minimum {
        Err(RuntimeWidthErrorV2::InvalidLength)
    } else {
        Ok(())
    }
}

fn exact_width(bytes: &[u8], expected: usize) -> RuntimeWidthResultV2<()> {
    if bytes.len() != expected {
        Err(RuntimeWidthErrorV2::InvalidLength)
    } else {
        Ok(())
    }
}

fn require_magic(bytes: &[u8], magic: &[u8; 8]) -> RuntimeWidthResultV2<()> {
    if bytes.get(..8) == Some(magic.as_slice()) {
        Ok(())
    } else {
        Err(RuntimeWidthErrorV2::InvalidMagic)
    }
}

fn require_version(bytes: &[u8]) -> RuntimeWidthResultV2<()> {
    if u16_at(bytes, 8)? == RUNTIME_WIDTH_VERSION_V2 {
        Ok(())
    } else {
        Err(RuntimeWidthErrorV2::InvalidVersion)
    }
}

fn require_zero(bytes: &[u8], offset: usize, length: usize) -> RuntimeWidthResultV2<()> {
    let end = offset
        .checked_add(length)
        .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?;
    if bytes
        .get(offset..end)
        .ok_or(RuntimeWidthErrorV2::InvalidLength)?
        .iter()
        .all(|value| *value == 0)
    {
        Ok(())
    } else {
        Err(RuntimeWidthErrorV2::NonCanonicalPadding)
    }
}

fn zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn byte_at(bytes: &[u8], offset: usize) -> RuntimeWidthResultV2<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(RuntimeWidthErrorV2::InvalidLength)
}

fn u16_at(bytes: &[u8], offset: usize) -> RuntimeWidthResultV2<u16> {
    let value = bytes
        .get(
            offset
                ..offset
                    .checked_add(2)
                    .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::InvalidLength)?;
    let array = <[u8; 2]>::try_from(value).map_err(|_| RuntimeWidthErrorV2::InvalidLength)?;
    Ok(u16::from_le_bytes(array))
}

fn u32_at(bytes: &[u8], offset: usize) -> RuntimeWidthResultV2<u32> {
    let value = bytes
        .get(
            offset
                ..offset
                    .checked_add(4)
                    .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::InvalidLength)?;
    let array = <[u8; 4]>::try_from(value).map_err(|_| RuntimeWidthErrorV2::InvalidLength)?;
    Ok(u32::from_le_bytes(array))
}

fn u64_at(bytes: &[u8], offset: usize) -> RuntimeWidthResultV2<u64> {
    let value = bytes
        .get(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::InvalidLength)?;
    let array = <[u8; 8]>::try_from(value).map_err(|_| RuntimeWidthErrorV2::InvalidLength)?;
    Ok(u64::from_le_bytes(array))
}

fn array32_at(bytes: &[u8], offset: usize) -> RuntimeWidthResultV2<[u8; 32]> {
    let value = bytes
        .get(
            offset
                ..offset
                    .checked_add(32)
                    .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeWidthErrorV2::InvalidLength)?;
    <[u8; 32]>::try_from(value).map_err(|_| RuntimeWidthErrorV2::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> RuntimeWidthResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(RuntimeWidthErrorV2::ArithmeticOverflow)?;
    output
        .get_mut(offset..end)
        .ok_or(RuntimeWidthErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> RuntimeWidthResultV2<()> {
    let destination = output
        .get_mut(offset)
        .ok_or(RuntimeWidthErrorV2::InvalidLength)?;
    *destination = value;
    Ok(())
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> RuntimeWidthResultV2<()> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) -> RuntimeWidthResultV2<()> {
    put(output, offset, &value.to_le_bytes())
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) -> RuntimeWidthResultV2<()> {
    put(output, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;

    const CANDIDATE: [u8; 32] = [1; 32];
    const PRODUCT: [u8; 32] = [2; 32];
    const BATCH: [u8; 32] = [3; 32];
    const ORDER: [u8; 32] = [4; 32];
    const OWNER: [u8; 32] = [5; 32];

    fn candidate_header(width: u32) -> CandidateHeaderV2 {
        CandidateHeaderV2 {
            outcome_count: width,
            page_count: 3,
            candidate_coordinate: 1,
            price_scale: u64::from(width),
            candidate_id: CANDIDATE,
            product_id: PRODUCT,
            batch_id: BATCH,
        }
    }

    fn execution_header(width: u32, coordinate: u32) -> ExecutionHeaderV2 {
        ExecutionHeaderV2 {
            outcome_count: width,
            page_coordinate: 1,
            execution_coordinate: coordinate,
            nonce: 7,
            order_id: ORDER,
            owner_id: OWNER,
            max_lots: 8,
            lots: 3,
        }
    }

    fn candidate(width: u32) -> std::vec::Vec<u8> {
        let count = usize::try_from(width).expect("test width fits");
        let mut output = vec![0; candidate_len(width).expect("test width")];
        CandidateV2::encode_into(candidate_header(width), &vec![1; count], &mut output)
            .expect("encode candidate");
        output
    }

    fn execution(width: u32, coordinate: u32) -> std::vec::Vec<u8> {
        let count = usize::try_from(width).expect("test width fits");
        let mut output = vec![0; execution_len(width).expect("test width")];
        ExecutionV2::encode_into(
            execution_header(width, coordinate),
            &vec![7; count],
            &vec![9; count],
            &mut output,
        )
        .expect("encode execution");
        output
    }

    #[test]
    fn exact_runtime_widths_one_sixteen_and_two_fifty_eight() {
        for width in [1_u32, 16, 258] {
            let candidate = candidate(width);
            let decoded = CandidateV2::decode(&candidate).expect("candidate decodes");
            assert_eq!(decoded.header().outcome_count, width);
            assert_eq!(decoded.price(width - 1).expect("last price"), 1);
            assert_eq!(
                decoded.price(width),
                Err(RuntimeWidthErrorV2::IndexOutOfBounds)
            );

            let execution = execution(width, 1);
            let decoded = ExecutionV2::decode(&execution).expect("execution decodes");
            assert_eq!(decoded.receive_per_lot(width - 1).expect("last receive"), 7);
            assert_eq!(decoded.deliver_per_lot(width - 1).expect("last deliver"), 9);

            let mut cursor = vec![0; settlement_cursor_len(width).expect("cursor width")];
            SettlementCursorV2::encode_into(
                SettlementCursorHeaderV2 {
                    outcome_count: width,
                    order_count: 3,
                    next_order: 0,
                    revision: 1,
                    candidate_id: CANDIDATE,
                    quote_inventory: 0,
                    complete_set_quantity: 0,
                    terminal_coordinate: 0,
                    phase: SettlementPhaseV2::Collecting,
                },
                &vec![11; usize::try_from(width).expect("test width")],
                &mut cursor,
            )
            .expect("cursor encode");
            assert_eq!(
                SettlementCursorV2::decode(&cursor)
                    .expect("cursor decode")
                    .inventory(width - 1)
                    .expect("inventory"),
                11
            );

            let mut verified = vec![0; verified_candidate_len(width).expect("verified width")];
            VerifiedCandidateV2::encode_into(
                VerifiedCandidateHeaderV2 {
                    outcome_count: width,
                    page_count: 3,
                    candidate_coordinate: 1,
                    revision: 1,
                    candidate_id: CANDIDATE,
                    product_id: PRODUCT,
                    batch_id: BATCH,
                    filled_lots: 1,
                    quote_debit: 2,
                    quote_credit: 3,
                    price_scale: 1,
                },
                &vec![13; usize::try_from(width).expect("test width")],
                &vec![17; usize::try_from(width).expect("test width")],
                &mut verified,
            )
            .expect("verified encode");
            let decoded = VerifiedCandidateV2::decode(&verified).expect("verified decode");
            assert_eq!(decoded.claim_input(width - 1).expect("input"), 13);
            assert_eq!(decoded.claim_output(width - 1).expect("output"), 17);
        }
    }

    #[test]
    fn compact_page_roundtrips_without_fixed_row_or_outcome_arrays() {
        let first = execution(258, 1);
        let second = execution(258, 2);
        let header = PageHeaderV2 {
            outcome_count: 258,
            page_coordinate: 1,
            page_count: 3,
            revision: 1,
            candidate_id: CANDIDATE,
        };
        let mut page = vec![0; page_len(258, 2).expect("page width")];
        PageV2::encode_into(header, &[&first, &second], &mut page).expect("page encode");
        let decoded = PageV2::decode(&page).expect("page decode");
        assert_eq!(decoded.row_count(), 2);
        assert_eq!(
            decoded
                .execution(1)
                .expect("second row")
                .header()
                .execution_coordinate,
            2
        );
    }

    #[test]
    fn hostile_records_refuse_truncation_overflow_substitution_and_padding() {
        let valid = candidate(16);
        assert_eq!(
            CandidateV2::decode(&valid[..valid.len() - 1]),
            Err(RuntimeWidthErrorV2::InvalidLength)
        );
        let mut padding = valid.clone();
        padding[11] = 1;
        assert_eq!(
            CandidateV2::decode(&padding),
            Err(RuntimeWidthErrorV2::NonCanonicalPadding)
        );
        let mut simplex = valid.clone();
        simplex[128] = 2;
        assert_eq!(
            CandidateV2::decode(&simplex),
            Err(RuntimeWidthErrorV2::InvalidSimplex)
        );

        let first = execution(16, 1);
        let mut other_page = execution(16, 2);
        other_page[16..20].copy_from_slice(&2_u32.to_le_bytes());
        let header = PageHeaderV2 {
            outcome_count: 16,
            page_coordinate: 1,
            page_count: 3,
            revision: 1,
            candidate_id: CANDIDATE,
        };
        let mut page = vec![0; page_len(16, 2).expect("page width")];
        assert_eq!(
            PageV2::encode_into(header, &[&first, &other_page], &mut page),
            Err(RuntimeWidthErrorV2::Substitution)
        );
        assert_eq!(
            page_len(u32::MAX, u32::MAX),
            Err(RuntimeWidthErrorV2::ArithmeticOverflow)
        );
    }

    #[test]
    fn noncanonical_rows_and_terminal_cursor_refuse() {
        let first = execution(1, 1);
        let second = execution(1, 1);
        let header = PageHeaderV2 {
            outcome_count: 1,
            page_coordinate: 1,
            page_count: 1,
            revision: 1,
            candidate_id: CANDIDATE,
        };
        let mut page = vec![0; page_len(1, 2).expect("page width")];
        assert_eq!(
            PageV2::encode_into(header, &[&first, &second], &mut page),
            Err(RuntimeWidthErrorV2::NonCanonicalRows)
        );

        let mut cursor = vec![0; settlement_cursor_len(1).expect("cursor width")];
        assert_eq!(
            SettlementCursorV2::encode_into(
                SettlementCursorHeaderV2 {
                    outcome_count: 1,
                    order_count: 1,
                    next_order: 1,
                    revision: 1,
                    candidate_id: CANDIDATE,
                    quote_inventory: 0,
                    complete_set_quantity: 0,
                    terminal_coordinate: 0,
                    phase: SettlementPhaseV2::Terminal
                },
                &[0],
                &mut cursor,
            ),
            Err(RuntimeWidthErrorV2::InvalidCursor)
        );
    }

    #[test]
    fn compact_execution_layout_does_not_claim_quote_fragment_semantics() {
        let width = 1;
        let row = execution(width, 1);
        assert_eq!(row.len(), EXECUTION_HEADER_BYTES_V2 + 16);
        let decoded = ExecutionV2::decode(&row).expect("compact row decodes");
        assert_eq!(decoded.header().lots, 3);
        assert_eq!(decoded.receive_per_lot(0).expect("receive"), 7);
        assert_eq!(decoded.deliver_per_lot(0).expect("deliver"), 9);
        // Quote caps and fragments have no offset in this fixed geometry: the
        // authenticated order and streamed verifier own those facts instead.
        assert_eq!(execution_len(width).expect("width"), 128);
    }
}
