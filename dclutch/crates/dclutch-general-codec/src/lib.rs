#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-layout data and cursor codec for successor General clearing.
//!
//! This crate validates canonical physical representation only. In particular,
//! quote debit and credit portions are aggregated and validated per order by
//! the Lean-owned candidate verifier; an execution row is not a rounding
//! boundary. Solana accounts, CPI, signatures, and release admission are
//! intentionally outside this crate.

#[rustfmt::skip]
mod generated_general_controller;

/// Maximum outcomes admitted by this physical profile.
pub const MAX_OUTCOMES: usize = generated_general_controller::MAX_OUTCOMES;
/// Maximum execution rows in one streamed page.
pub const MAX_EXECUTIONS_PER_PAGE: usize = generated_general_controller::MAX_EXECUTIONS_PER_PAGE;
/// Maximum authenticated pages in one candidate under this physical profile.
pub const MAX_PAGES_PER_CANDIDATE: u32 = generated_general_controller::MAX_PAGES_PER_CANDIDATE;
/// Maximum interpreted criteria in one immutable selection policy.
pub const MAX_SELECTION_CRITERIA: usize = generated_general_controller::MAX_SELECTION_CRITERIA;
/// Exact candidate header width.
pub const CANDIDATE_BYTES: usize = generated_general_controller::CANDIDATE_BYTES;
/// Exact execution row width.
pub const EXECUTION_BYTES: usize = generated_general_controller::EXECUTION_BYTES;
/// Exact page width.
pub const PAGE_BYTES: usize = generated_general_controller::PAGE_BYTES;
/// Exact immutable selection-policy width.
pub const SELECTION_POLICY_BYTES: usize = generated_general_controller::POLICY_BYTES;
/// Exact selection cursor width.
pub const SELECTION_CURSOR_BYTES: usize = generated_general_controller::SELECTION_BYTES;
/// Exact settlement cursor width.
pub const SETTLEMENT_CURSOR_BYTES: usize = generated_general_controller::SETTLEMENT_BYTES;
/// Exact controller request width.
pub const CONTROLLER_REQUEST_BYTES: usize = generated_general_controller::REQUEST_BYTES;

/// Stable refusal from canonical fixed-layout validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its single exact generated width.
    InvalidLength,
    /// Magic bytes did not identify the expected object.
    InvalidMagic,
    /// Version was not the generated V1 version.
    UnsupportedVersion,
    /// A tag was outside its closed enum.
    UnknownTag,
    /// A boolean was neither zero nor one.
    NonCanonicalBoolean,
    /// Reserved or inactive storage was not all zero.
    NonCanonicalPadding,
    /// A required identity or scalar was zero.
    ZeroCoordinate,
    /// A count or cursor exceeded this fixed profile.
    InvalidCursor,
    /// Exact integer validation overflowed.
    ArithmeticOverflow,
    /// Prices did not sum exactly to their one scale.
    InvalidSimplex,
}

/// Result alias for General controller codecs.
pub type Result<T> = core::result::Result<T, Error>;

/// Data-defined selection or settlement action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Action {
    /// Submit an authenticated candidate for deterministic comparison.
    Consider = generated_general_controller::ACTION_CONSIDER,
    /// Close selection around the current best valid submitted candidate.
    Freeze = generated_general_controller::ACTION_FREEZE,
    /// Initialize the streamed settlement cursor.
    InitializeSettlement = generated_general_controller::ACTION_INITIALIZE_SETTLEMENT,
    /// Collect one exact candidate page.
    Collect = generated_general_controller::ACTION_COLLECT,
    /// Perform the sole complete-set mint, merge, or no-op.
    Materialize = generated_general_controller::ACTION_MATERIALIZE,
    /// Distribute one exact candidate page.
    Distribute = generated_general_controller::ACTION_DISTRIBUTE,
    /// Route the exact quote remainder and enter terminal state.
    Close = generated_general_controller::ACTION_CLOSE,
}

impl Action {
    fn decode(tag: u8) -> Result<Self> {
        match tag {
            generated_general_controller::ACTION_CONSIDER => Ok(Self::Consider),
            generated_general_controller::ACTION_FREEZE => Ok(Self::Freeze),
            generated_general_controller::ACTION_INITIALIZE_SETTLEMENT => {
                Ok(Self::InitializeSettlement)
            }
            generated_general_controller::ACTION_COLLECT => Ok(Self::Collect),
            generated_general_controller::ACTION_MATERIALIZE => Ok(Self::Materialize),
            generated_general_controller::ACTION_DISTRIBUTE => Ok(Self::Distribute),
            generated_general_controller::ACTION_CLOSE => Ok(Self::Close),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Streamed settlement phase; `next_page` is stored separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Phase {
    /// Collecting authenticated input pages.
    Collecting = generated_general_controller::PHASE_COLLECTING,
    /// Ready for the unique complete-set operation.
    Materializing = generated_general_controller::PHASE_MATERIALIZING,
    /// Distributing authenticated output pages.
    Distributing = generated_general_controller::PHASE_DISTRIBUTING,
    /// All pages distributed and ready to close.
    ReadyToClose = generated_general_controller::PHASE_READY_TO_CLOSE,
    /// Terminal with no owned inventory.
    Terminal = generated_general_controller::PHASE_TERMINAL,
}

impl Phase {
    fn decode(tag: u8) -> Result<Self> {
        match tag {
            generated_general_controller::PHASE_COLLECTING => Ok(Self::Collecting),
            generated_general_controller::PHASE_MATERIALIZING => Ok(Self::Materializing),
            generated_general_controller::PHASE_DISTRIBUTING => Ok(Self::Distributing),
            generated_general_controller::PHASE_READY_TO_CLOSE => Ok(Self::ReadyToClose),
            generated_general_controller::PHASE_TERMINAL => Ok(Self::Terminal),
            _ => Err(Error::UnknownTag),
        }
    }
}

/// One interpreted lexicographic criterion from immutable policy data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SelectionCriterion {
    /// Prefer greater candidate-wide filled lots.
    MaximizeFilledLots = generated_general_controller::CRITERION_MAXIMIZE_FILLED_LOTS,
    /// Prefer smaller candidate-wide quote surplus.
    MinimizeQuoteSurplus = generated_general_controller::CRITERION_MINIMIZE_QUOTE_SURPLUS,
    /// Deterministic final identity tie-break.
    MinimizeCandidateId = generated_general_controller::CRITERION_MINIMIZE_CANDIDATE_ID,
}

impl SelectionCriterion {
    fn decode(tag: u8) -> Result<Self> {
        match tag {
            generated_general_controller::CRITERION_MAXIMIZE_FILLED_LOTS => {
                Ok(Self::MaximizeFilledLots)
            }
            generated_general_controller::CRITERION_MINIMIZE_QUOTE_SURPLUS => {
                Ok(Self::MinimizeQuoteSurplus)
            }
            generated_general_controller::CRITERION_MINIMIZE_CANDIDATE_ID => {
                Ok(Self::MinimizeCandidateId)
            }
            _ => Err(Error::UnknownTag),
        }
    }
}

/// Fixed physical candidate header and exact simplex coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateV1 {
    /// Active prefix length in `prices`.
    pub outcome_count: u8,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Product content identity.
    pub product_id: [u8; 32],
    /// Batch content identity.
    pub batch_id: [u8; 32],
    /// Number of separately authenticated pages.
    pub page_count: u32,
    /// Sole quote denominator.
    pub price_scale: u64,
    /// Fixed-capacity prices with a canonical zero inactive tail.
    pub prices: [u64; MAX_OUTCOMES],
}

impl CandidateV1 {
    /// Decode and validate one exact candidate header.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            CANDIDATE_BYTES,
            &generated_general_controller::CANDIDATE_MAGIC,
            generated_general_controller::CANDIDATE_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            generated_general_controller::CANDIDATE_RESERVED_A_OFFSET,
            5,
        )?;
        require_zero(
            input,
            generated_general_controller::CANDIDATE_RESERVED_B_OFFSET,
            4,
        )?;
        let value = Self {
            outcome_count: byte_at(
                input,
                generated_general_controller::CANDIDATE_OUTCOME_COUNT_OFFSET,
            )?,
            candidate_id: array_at(
                input,
                generated_general_controller::CANDIDATE_CANDIDATE_ID_OFFSET,
            )?,
            product_id: array_at(
                input,
                generated_general_controller::CANDIDATE_PRODUCT_ID_OFFSET,
            )?,
            batch_id: array_at(
                input,
                generated_general_controller::CANDIDATE_BATCH_ID_OFFSET,
            )?,
            page_count: u32_at(
                input,
                generated_general_controller::CANDIDATE_PAGE_COUNT_OFFSET,
            )?,
            price_scale: u64_at(
                input,
                generated_general_controller::CANDIDATE_PRICE_SCALE_OFFSET,
            )?,
            prices: u64_array_at(input, generated_general_controller::CANDIDATE_PRICES_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical candidate header.
    pub fn to_bytes(self) -> Result<[u8; CANDIDATE_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; CANDIDATE_BYTES];
        put_header(
            &mut output,
            &generated_general_controller::CANDIDATE_MAGIC,
            generated_general_controller::CANDIDATE_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::CANDIDATE_OUTCOME_COUNT_OFFSET,
            self.outcome_count,
        )?;
        put(
            &mut output,
            generated_general_controller::CANDIDATE_CANDIDATE_ID_OFFSET,
            &self.candidate_id,
        )?;
        put(
            &mut output,
            generated_general_controller::CANDIDATE_PRODUCT_ID_OFFSET,
            &self.product_id,
        )?;
        put(
            &mut output,
            generated_general_controller::CANDIDATE_BATCH_ID_OFFSET,
            &self.batch_id,
        )?;
        put(
            &mut output,
            generated_general_controller::CANDIDATE_PAGE_COUNT_OFFSET,
            &self.page_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_general_controller::CANDIDATE_PRICE_SCALE_OFFSET,
            &self.price_scale.to_le_bytes(),
        )?;
        put_u64_array(
            &mut output,
            generated_general_controller::CANDIDATE_PRICES_OFFSET,
            &self.prices,
        )?;
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        let count = active_count(self.outcome_count)?;
        if is_zero(&self.candidate_id)
            || is_zero(&self.product_id)
            || is_zero(&self.batch_id)
            || self.page_count == 0
            || self.price_scale == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        if self.page_count > MAX_PAGES_PER_CANDIDATE {
            return Err(Error::InvalidCursor);
        }
        require_inactive_zero(&self.prices, count)?;
        let sum = self
            .prices
            .get(..count)
            .ok_or(Error::InvalidCursor)?
            .iter()
            .try_fold(0_u64, |total, value| total.checked_add(*value))
            .ok_or(Error::ArithmeticOverflow)?;
        if sum != self.price_scale {
            return Err(Error::InvalidSimplex);
        }
        Ok(())
    }
}

/// One fixed execution row. Quote fields are portions, not local rounding claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionV1 {
    /// Immutable order content identity.
    pub order_id: [u8; 32],
    /// Immutable owner identity.
    pub owner_id: [u8; 32],
    /// Signed-order nonce.
    pub nonce: u64,
    /// Candidate-wide maximum lots for this order identity.
    pub max_lots: u64,
    /// Candidate-wide exact debit limit per lot.
    pub max_quote_debit_per_lot: u64,
    /// Positive lots in this page fragment.
    pub lots: u64,
    /// Portion of candidate-wide quote debit assigned to this fragment.
    pub quote_debit: u64,
    /// Portion of candidate-wide quote credit assigned to this fragment.
    pub quote_credit: u64,
    /// Fixed-capacity received-claim vector.
    pub receive_per_lot: [u64; MAX_OUTCOMES],
    /// Fixed-capacity delivered-claim vector.
    pub deliver_per_lot: [u64; MAX_OUTCOMES],
}

impl ExecutionV1 {
    /// Canonical all-zero inactive row.
    pub const EMPTY: Self = Self {
        order_id: [0; 32],
        owner_id: [0; 32],
        nonce: 0,
        max_lots: 0,
        max_quote_debit_per_lot: 0,
        lots: 0,
        quote_debit: 0,
        quote_credit: 0,
        receive_per_lot: [0; MAX_OUTCOMES],
        deliver_per_lot: [0; MAX_OUTCOMES],
    };

    /// Decode one active row for a checked runtime outcome width.
    ///
    /// Quote fields remain candidate fragments. The General verifier must
    /// aggregate them by order identity before applying quote rounding.
    pub fn decode_for_outcomes(input: &[u8], outcome_count: u8) -> Result<Self> {
        Self::decode_active(input, outcome_count)
    }

    /// Encode one active row outside its parent page using the supplied width.
    pub fn to_bytes_for_outcomes(self, outcome_count: u8) -> Result<[u8; EXECUTION_BYTES]> {
        let mut output = [0_u8; EXECUTION_BYTES];
        self.encode_active(&mut output, outcome_count)?;
        Ok(output)
    }

    fn decode_active(input: &[u8], outcome_count: u8) -> Result<Self> {
        exact_width(input, EXECUTION_BYTES)?;
        let value = Self {
            order_id: array_at(
                input,
                generated_general_controller::EXECUTION_ORDER_ID_OFFSET,
            )?,
            owner_id: array_at(
                input,
                generated_general_controller::EXECUTION_OWNER_ID_OFFSET,
            )?,
            nonce: u64_at(input, generated_general_controller::EXECUTION_NONCE_OFFSET)?,
            max_lots: u64_at(
                input,
                generated_general_controller::EXECUTION_MAX_LOTS_OFFSET,
            )?,
            max_quote_debit_per_lot: u64_at(
                input,
                generated_general_controller::EXECUTION_MAX_QUOTE_DEBIT_PER_LOT_OFFSET,
            )?,
            lots: u64_at(input, generated_general_controller::EXECUTION_LOTS_OFFSET)?,
            quote_debit: u64_at(
                input,
                generated_general_controller::EXECUTION_QUOTE_DEBIT_OFFSET,
            )?,
            quote_credit: u64_at(
                input,
                generated_general_controller::EXECUTION_QUOTE_CREDIT_OFFSET,
            )?,
            receive_per_lot: u64_array_at(
                input,
                generated_general_controller::EXECUTION_RECEIVE_PER_LOT_OFFSET,
            )?,
            deliver_per_lot: u64_array_at(
                input,
                generated_general_controller::EXECUTION_DELIVER_PER_LOT_OFFSET,
            )?,
        };
        value.validate(outcome_count)?;
        Ok(value)
    }

    fn encode_active(self, output: &mut [u8], outcome_count: u8) -> Result<()> {
        self.validate(outcome_count)?;
        exact_width(output, EXECUTION_BYTES)?;
        put(
            output,
            generated_general_controller::EXECUTION_ORDER_ID_OFFSET,
            &self.order_id,
        )?;
        put(
            output,
            generated_general_controller::EXECUTION_OWNER_ID_OFFSET,
            &self.owner_id,
        )?;
        put(
            output,
            generated_general_controller::EXECUTION_NONCE_OFFSET,
            &self.nonce.to_le_bytes(),
        )?;
        put(
            output,
            generated_general_controller::EXECUTION_MAX_LOTS_OFFSET,
            &self.max_lots.to_le_bytes(),
        )?;
        put(
            output,
            generated_general_controller::EXECUTION_MAX_QUOTE_DEBIT_PER_LOT_OFFSET,
            &self.max_quote_debit_per_lot.to_le_bytes(),
        )?;
        put(
            output,
            generated_general_controller::EXECUTION_LOTS_OFFSET,
            &self.lots.to_le_bytes(),
        )?;
        put(
            output,
            generated_general_controller::EXECUTION_QUOTE_DEBIT_OFFSET,
            &self.quote_debit.to_le_bytes(),
        )?;
        put(
            output,
            generated_general_controller::EXECUTION_QUOTE_CREDIT_OFFSET,
            &self.quote_credit.to_le_bytes(),
        )?;
        put_u64_array(
            output,
            generated_general_controller::EXECUTION_RECEIVE_PER_LOT_OFFSET,
            &self.receive_per_lot,
        )?;
        put_u64_array(
            output,
            generated_general_controller::EXECUTION_DELIVER_PER_LOT_OFFSET,
            &self.deliver_per_lot,
        )?;
        Ok(())
    }

    fn validate(&self, outcome_count: u8) -> Result<()> {
        let count = active_count(outcome_count)?;
        if is_zero(&self.order_id)
            || is_zero(&self.owner_id)
            || self.max_lots == 0
            || self.lots == 0
        {
            return Err(Error::ZeroCoordinate);
        }
        if self.lots > self.max_lots {
            return Err(Error::InvalidCursor);
        }
        require_inactive_zero(&self.receive_per_lot, count)?;
        require_inactive_zero(&self.deliver_per_lot, count)
    }
}

/// Stack-bounded borrowed view of one fixed-capacity candidate page.
///
/// This view validates the complete wire, including inactive row storage, but
/// decodes active rows one at a time. Its stack footprint therefore does not
/// grow with [`MAX_EXECUTIONS_PER_PAGE`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageViewV1<'a> {
    input: &'a [u8],
    outcome_count: u8,
    candidate_id: [u8; 32],
    page_index: u32,
    page_count: u32,
    execution_count: u8,
}

impl<'a> PageViewV1<'a> {
    /// Hostile-decode one exact page without constructing its 32-row array.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        header(
            input,
            PAGE_BYTES,
            &generated_general_controller::PAGE_MAGIC,
            generated_general_controller::PAGE_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            generated_general_controller::PAGE_RESERVED_A_OFFSET,
            4,
        )?;
        require_zero(
            input,
            generated_general_controller::PAGE_RESERVED_B_OFFSET,
            8,
        )?;
        let value = Self {
            input,
            outcome_count: byte_at(
                input,
                generated_general_controller::PAGE_OUTCOME_COUNT_OFFSET,
            )?,
            candidate_id: array_at(
                input,
                generated_general_controller::PAGE_CANDIDATE_ID_OFFSET,
            )?,
            page_index: u32_at(input, generated_general_controller::PAGE_PAGE_INDEX_OFFSET)?,
            page_count: u32_at(input, generated_general_controller::PAGE_PAGE_COUNT_OFFSET)?,
            execution_count: byte_at(
                input,
                generated_general_controller::PAGE_EXECUTION_COUNT_OFFSET,
            )?,
        };
        value.validate_header()?;
        let count = usize::from(value.execution_count);
        for index in 0..MAX_EXECUTIONS_PER_PAGE {
            let row = execution_slice(input, index)?;
            if index < count {
                ExecutionV1::decode_active(row, value.outcome_count)?;
            } else if !row.iter().all(|byte| *byte == 0) {
                return Err(Error::NonCanonicalPadding);
            }
        }
        Ok(value)
    }

    /// Active outcome prefix length.
    pub const fn outcome_count(self) -> u8 {
        self.outcome_count
    }

    /// Candidate identity shared with the candidate header.
    pub const fn candidate_id(self) -> [u8; 32] {
        self.candidate_id
    }

    /// Zero-based page coordinate.
    pub const fn page_index(self) -> u32 {
        self.page_index
    }

    /// Total candidate page count.
    pub const fn page_count(self) -> u32 {
        self.page_count
    }

    /// Active execution row count.
    pub const fn execution_count(self) -> u8 {
        self.execution_count
    }

    /// Decode one active row by runtime coordinate.
    pub fn execution(self, index: usize) -> Result<ExecutionV1> {
        if index >= usize::from(self.execution_count) {
            return Err(Error::InvalidCursor);
        }
        ExecutionV1::decode_active(execution_slice(self.input, index)?, self.outcome_count)
    }

    fn validate_header(self) -> Result<()> {
        active_count(self.outcome_count)?;
        let count = usize::from(self.execution_count);
        if count == 0
            || count > MAX_EXECUTIONS_PER_PAGE
            || self.page_count == 0
            || self.page_count > MAX_PAGES_PER_CANDIDATE
            || self.page_index >= self.page_count
        {
            return Err(Error::InvalidCursor);
        }
        if is_zero(&self.candidate_id) {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Fixed-capacity authenticated page.
///
/// Host tooling uses this owned assembly type. SBF consumers use
/// [`PageViewV1`] so the 11,840-byte page never enters a VM stack frame.
#[cfg(not(target_os = "solana"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageV1 {
    /// Outcome prefix shared with the candidate.
    pub outcome_count: u8,
    /// Candidate identity shared with the candidate header.
    pub candidate_id: [u8; 32],
    /// Zero-based streamed page coordinate.
    pub page_index: u32,
    /// Total candidate page count.
    pub page_count: u32,
    /// Number of active rows in `executions`.
    pub execution_count: u8,
    /// Fixed rows with an exact all-zero inactive tail.
    pub executions: [ExecutionV1; MAX_EXECUTIONS_PER_PAGE],
}

#[cfg(not(target_os = "solana"))]
impl PageV1 {
    /// Decode one exact page and reject all noncanonical inactive storage.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            PAGE_BYTES,
            &generated_general_controller::PAGE_MAGIC,
            generated_general_controller::PAGE_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            generated_general_controller::PAGE_RESERVED_A_OFFSET,
            4,
        )?;
        require_zero(
            input,
            generated_general_controller::PAGE_RESERVED_B_OFFSET,
            8,
        )?;
        let outcome_count = byte_at(
            input,
            generated_general_controller::PAGE_OUTCOME_COUNT_OFFSET,
        )?;
        let execution_count = byte_at(
            input,
            generated_general_controller::PAGE_EXECUTION_COUNT_OFFSET,
        )?;
        let count = usize::from(execution_count);
        if count == 0 || count > MAX_EXECUTIONS_PER_PAGE {
            return Err(Error::InvalidCursor);
        }
        active_count(outcome_count)?;
        let mut executions = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        for index in 0..MAX_EXECUTIONS_PER_PAGE {
            let row = execution_slice(input, index)?;
            if index < count {
                *executions.get_mut(index).ok_or(Error::InvalidCursor)? =
                    ExecutionV1::decode_active(row, outcome_count)?;
            } else if !row.iter().all(|byte| *byte == 0) {
                return Err(Error::NonCanonicalPadding);
            }
        }
        let value = Self {
            outcome_count,
            candidate_id: array_at(
                input,
                generated_general_controller::PAGE_CANDIDATE_ID_OFFSET,
            )?,
            page_index: u32_at(input, generated_general_controller::PAGE_PAGE_INDEX_OFFSET)?,
            page_count: u32_at(input, generated_general_controller::PAGE_PAGE_COUNT_OFFSET)?,
            execution_count,
            executions,
        };
        value.validate_header()?;
        Ok(value)
    }

    /// Encode one exact page with a canonical all-zero inactive tail.
    pub fn to_bytes(self) -> Result<[u8; PAGE_BYTES]> {
        self.validate_header()?;
        let count = usize::from(self.execution_count);
        let mut output = [0_u8; PAGE_BYTES];
        put_header(
            &mut output,
            &generated_general_controller::PAGE_MAGIC,
            generated_general_controller::PAGE_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::PAGE_OUTCOME_COUNT_OFFSET,
            self.outcome_count,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::PAGE_EXECUTION_COUNT_OFFSET,
            self.execution_count,
        )?;
        put(
            &mut output,
            generated_general_controller::PAGE_CANDIDATE_ID_OFFSET,
            &self.candidate_id,
        )?;
        put(
            &mut output,
            generated_general_controller::PAGE_PAGE_INDEX_OFFSET,
            &self.page_index.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_general_controller::PAGE_PAGE_COUNT_OFFSET,
            &self.page_count.to_le_bytes(),
        )?;
        for index in 0..count {
            let start = execution_offset(index)?;
            let end = start
                .checked_add(EXECUTION_BYTES)
                .ok_or(Error::InvalidLength)?;
            self.executions
                .get(index)
                .ok_or(Error::InvalidCursor)?
                .encode_active(
                    output.get_mut(start..end).ok_or(Error::InvalidLength)?,
                    self.outcome_count,
                )?;
        }
        if self
            .executions
            .get(count..)
            .ok_or(Error::InvalidCursor)?
            .iter()
            .any(|execution| *execution != ExecutionV1::EMPTY)
        {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(output)
    }

    fn validate_header(&self) -> Result<()> {
        active_count(self.outcome_count)?;
        let count = usize::from(self.execution_count);
        if count == 0
            || count > MAX_EXECUTIONS_PER_PAGE
            || self.page_count == 0
            || self.page_count > MAX_PAGES_PER_CANDIDATE
            || self.page_index >= self.page_count
        {
            return Err(Error::InvalidCursor);
        }
        if is_zero(&self.candidate_id) {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Immutable, data-interpreted deterministic selection policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionPolicyV1 {
    /// Content identity selected by the capability.
    pub policy_id: [u8; 32],
    /// Active criterion prefix length.
    pub criterion_count: u8,
    /// Fixed-capacity criteria; inactive slots are canonical tag zero.
    pub criteria: [SelectionCriterion; MAX_SELECTION_CRITERIA],
}

impl SelectionPolicyV1 {
    /// Decode one exact immutable policy record.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            SELECTION_POLICY_BYTES,
            &generated_general_controller::POLICY_MAGIC,
            generated_general_controller::POLICY_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            generated_general_controller::POLICY_RESERVED_OFFSET,
            5,
        )?;
        let criterion_count = byte_at(
            input,
            generated_general_controller::POLICY_CRITERION_COUNT_OFFSET,
        )?;
        let count = usize::from(criterion_count);
        if count == 0 || count > MAX_SELECTION_CRITERIA {
            return Err(Error::InvalidCursor);
        }
        let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
        for (index, criterion) in criteria.iter_mut().enumerate() {
            let tag = byte_at(
                input,
                generated_general_controller::POLICY_CRITERIA_OFFSET
                    .checked_add(index)
                    .ok_or(Error::InvalidLength)?,
            )?;
            if index < count {
                *criterion = SelectionCriterion::decode(tag)?;
            } else if tag != 0 {
                return Err(Error::NonCanonicalPadding);
            }
        }
        let value = Self {
            policy_id: array_at(input, generated_general_controller::POLICY_POLICY_ID_OFFSET)?,
            criterion_count,
            criteria,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact immutable policy record.
    pub fn to_bytes(self) -> Result<[u8; SELECTION_POLICY_BYTES]> {
        self.validate()?;
        let count = usize::from(self.criterion_count);
        let mut output = [0_u8; SELECTION_POLICY_BYTES];
        put_header(
            &mut output,
            &generated_general_controller::POLICY_MAGIC,
            generated_general_controller::POLICY_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::POLICY_CRITERION_COUNT_OFFSET,
            self.criterion_count,
        )?;
        put(
            &mut output,
            generated_general_controller::POLICY_POLICY_ID_OFFSET,
            &self.policy_id,
        )?;
        for (index, criterion) in self
            .criteria
            .get(..count)
            .ok_or(Error::InvalidCursor)?
            .iter()
            .enumerate()
        {
            put_byte(
                &mut output,
                generated_general_controller::POLICY_CRITERIA_OFFSET
                    .checked_add(index)
                    .ok_or(Error::InvalidLength)?,
                *criterion as u8,
            )?;
        }
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        let count = usize::from(self.criterion_count);
        if is_zero(&self.policy_id) {
            return Err(Error::ZeroCoordinate);
        }
        if count == 0 || count > MAX_SELECTION_CRITERIA {
            return Err(Error::InvalidCursor);
        }
        if self.criteria.get(count - 1) != Some(&SelectionCriterion::MinimizeCandidateId) {
            return Err(Error::InvalidCursor);
        }
        if self
            .criteria
            .get(count..)
            .ok_or(Error::InvalidCursor)?
            .iter()
            .any(|criterion| *criterion != SelectionCriterion::MaximizeFilledLots)
        {
            return Err(Error::NonCanonicalPadding);
        }
        Ok(())
    }
}

/// Optimistic selection cursor; candidate objective data is not duplicated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionCursorV1 {
    /// Whether the best-valid-submitted selection is frozen.
    pub closed: bool,
    /// Batch content identity.
    pub batch_id: [u8; 32],
    /// Immutable interpreted selection-policy content identity.
    pub policy_id: [u8; 32],
    /// Best candidate identity, if one has been admitted.
    pub best_candidate_id: Option<[u8; 32]>,
    /// Optimistic concurrency revision.
    pub revision: u64,
}

impl SelectionCursorV1 {
    /// Decode one exact canonical selection cursor.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            SELECTION_CURSOR_BYTES,
            &generated_general_controller::SELECTION_MAGIC,
            generated_general_controller::SELECTION_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            generated_general_controller::SELECTION_RESERVED_A_OFFSET,
            4,
        )?;
        require_zero(
            input,
            generated_general_controller::SELECTION_RESERVED_B_OFFSET,
            8,
        )?;
        let closed = bool_at(input, generated_general_controller::SELECTION_CLOSED_OFFSET)?;
        let has_best = bool_at(
            input,
            generated_general_controller::SELECTION_HAS_BEST_OFFSET,
        )?;
        let best = array_at(
            input,
            generated_general_controller::SELECTION_BEST_CANDIDATE_ID_OFFSET,
        )?;
        let value = Self {
            closed,
            batch_id: array_at(
                input,
                generated_general_controller::SELECTION_BATCH_ID_OFFSET,
            )?,
            policy_id: array_at(
                input,
                generated_general_controller::SELECTION_POLICY_ID_OFFSET,
            )?,
            best_candidate_id: if has_best {
                Some(best)
            } else if is_zero(&best) {
                None
            } else {
                return Err(Error::NonCanonicalPadding);
            },
            revision: u64_at(
                input,
                generated_general_controller::SELECTION_REVISION_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical selection cursor.
    pub fn to_bytes(self) -> Result<[u8; SELECTION_CURSOR_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; SELECTION_CURSOR_BYTES];
        put_header(
            &mut output,
            &generated_general_controller::SELECTION_MAGIC,
            generated_general_controller::SELECTION_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::SELECTION_CLOSED_OFFSET,
            u8::from(self.closed),
        )?;
        put_byte(
            &mut output,
            generated_general_controller::SELECTION_HAS_BEST_OFFSET,
            u8::from(self.best_candidate_id.is_some()),
        )?;
        put(
            &mut output,
            generated_general_controller::SELECTION_BATCH_ID_OFFSET,
            &self.batch_id,
        )?;
        put(
            &mut output,
            generated_general_controller::SELECTION_POLICY_ID_OFFSET,
            &self.policy_id,
        )?;
        if let Some(candidate_id) = self.best_candidate_id {
            put(
                &mut output,
                generated_general_controller::SELECTION_BEST_CANDIDATE_ID_OFFSET,
                &candidate_id,
            )?;
        }
        put(
            &mut output,
            generated_general_controller::SELECTION_REVISION_OFFSET,
            &self.revision.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        if is_zero(&self.batch_id)
            || is_zero(&self.policy_id)
            || self.best_candidate_id.as_ref().is_some_and(is_zero)
        {
            return Err(Error::ZeroCoordinate);
        }
        Ok(())
    }
}

/// Streamed settlement cursor with fixed outcome inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCursorV1 {
    /// Current semantic phase.
    pub phase: Phase,
    /// Active inventory prefix length.
    pub outcome_count: u8,
    /// Settled candidate identity.
    pub candidate_id: [u8; 32],
    /// Total authenticated candidate pages.
    pub page_count: u32,
    /// Next page for collecting/distributing, otherwise canonical zero.
    pub next_page: u32,
    /// Next row within the current page, otherwise canonical zero.
    pub next_execution: u8,
    /// Optimistic concurrency revision.
    pub revision: u64,
    /// Exact fixed-capacity claim inventory.
    pub claim_inventory: [u64; MAX_OUTCOMES],
    /// Exact owned quote inventory.
    pub quote_inventory: u64,
    /// Cumulative quote routed to the declared surplus beneficiary.
    pub quote_surplus_paid: u64,
}

impl SettlementCursorV1 {
    /// Decode one exact canonical settlement cursor.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            SETTLEMENT_CURSOR_BYTES,
            &generated_general_controller::SETTLEMENT_MAGIC,
            generated_general_controller::SETTLEMENT_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            generated_general_controller::SETTLEMENT_RESERVED_OFFSET,
            3,
        )?;
        let value = Self {
            phase: Phase::decode(byte_at(
                input,
                generated_general_controller::SETTLEMENT_PHASE_OFFSET,
            )?)?,
            outcome_count: byte_at(
                input,
                generated_general_controller::SETTLEMENT_OUTCOME_COUNT_OFFSET,
            )?,
            candidate_id: array_at(
                input,
                generated_general_controller::SETTLEMENT_CANDIDATE_ID_OFFSET,
            )?,
            page_count: u32_at(
                input,
                generated_general_controller::SETTLEMENT_PAGE_COUNT_OFFSET,
            )?,
            next_page: u32_at(
                input,
                generated_general_controller::SETTLEMENT_NEXT_PAGE_OFFSET,
            )?,
            next_execution: byte_at(
                input,
                generated_general_controller::SETTLEMENT_NEXT_EXECUTION_OFFSET,
            )?,
            revision: u64_at(
                input,
                generated_general_controller::SETTLEMENT_REVISION_OFFSET,
            )?,
            claim_inventory: u64_array_at(
                input,
                generated_general_controller::SETTLEMENT_CLAIM_INVENTORY_OFFSET,
            )?,
            quote_inventory: u64_at(
                input,
                generated_general_controller::SETTLEMENT_QUOTE_INVENTORY_OFFSET,
            )?,
            quote_surplus_paid: u64_at(
                input,
                generated_general_controller::SETTLEMENT_QUOTE_SURPLUS_PAID_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical settlement cursor.
    pub fn to_bytes(self) -> Result<[u8; SETTLEMENT_CURSOR_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; SETTLEMENT_CURSOR_BYTES];
        put_header(
            &mut output,
            &generated_general_controller::SETTLEMENT_MAGIC,
            generated_general_controller::SETTLEMENT_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::SETTLEMENT_PHASE_OFFSET,
            self.phase as u8,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::SETTLEMENT_OUTCOME_COUNT_OFFSET,
            self.outcome_count,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::SETTLEMENT_NEXT_EXECUTION_OFFSET,
            self.next_execution,
        )?;
        put(
            &mut output,
            generated_general_controller::SETTLEMENT_CANDIDATE_ID_OFFSET,
            &self.candidate_id,
        )?;
        put(
            &mut output,
            generated_general_controller::SETTLEMENT_PAGE_COUNT_OFFSET,
            &self.page_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_general_controller::SETTLEMENT_NEXT_PAGE_OFFSET,
            &self.next_page.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_general_controller::SETTLEMENT_REVISION_OFFSET,
            &self.revision.to_le_bytes(),
        )?;
        put_u64_array(
            &mut output,
            generated_general_controller::SETTLEMENT_CLAIM_INVENTORY_OFFSET,
            &self.claim_inventory,
        )?;
        put(
            &mut output,
            generated_general_controller::SETTLEMENT_QUOTE_INVENTORY_OFFSET,
            &self.quote_inventory.to_le_bytes(),
        )?;
        put(
            &mut output,
            generated_general_controller::SETTLEMENT_QUOTE_SURPLUS_PAID_OFFSET,
            &self.quote_surplus_paid.to_le_bytes(),
        )?;
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        let count = active_count(self.outcome_count)?;
        if is_zero(&self.candidate_id) || self.page_count == 0 {
            return Err(Error::ZeroCoordinate);
        }
        if self.page_count > MAX_PAGES_PER_CANDIDATE {
            return Err(Error::InvalidCursor);
        }
        require_inactive_zero(&self.claim_inventory, count)?;
        match self.phase {
            Phase::Collecting | Phase::Distributing
                if self.next_page >= self.page_count
                    || usize::from(self.next_execution) >= MAX_EXECUTIONS_PER_PAGE =>
            {
                Err(Error::InvalidCursor)
            }
            Phase::Collecting | Phase::Distributing => Ok(()),
            Phase::Terminal
                if self.next_page != 0
                    || self.next_execution != 0
                    || self.quote_inventory != 0
                    || self.claim_inventory.iter().any(|value| *value != 0) =>
            {
                Err(Error::NonCanonicalPadding)
            }
            Phase::Materializing | Phase::ReadyToClose | Phase::Terminal
                if self.next_page != 0 || self.next_execution != 0 =>
            {
                Err(Error::InvalidCursor)
            }
            Phase::Materializing | Phase::ReadyToClose | Phase::Terminal => Ok(()),
        }
    }
}

/// Thin action request carrying only authenticated-account coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerRequestV1 {
    /// Requested data-defined action.
    pub action: Action,
    /// Exact cursor revision the caller observed.
    pub expected_revision: u64,
    /// Candidate identity, absent only for selection freeze.
    pub candidate_id: Option<[u8; 32]>,
    /// Page coordinate for collect/distribute, canonical zero otherwise.
    pub page_index: u32,
    /// Execution-row coordinate for collect/distribute, canonical zero otherwise.
    pub execution_index: u8,
}

impl ControllerRequestV1 {
    /// Decode one exact canonical request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        header(
            input,
            CONTROLLER_REQUEST_BYTES,
            &generated_general_controller::REQUEST_MAGIC,
            generated_general_controller::REQUEST_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            generated_general_controller::REQUEST_RESERVED_A_OFFSET,
            5,
        )?;
        require_zero(
            input,
            generated_general_controller::REQUEST_RESERVED_B_OFFSET,
            3,
        )?;
        let action = Action::decode(byte_at(
            input,
            generated_general_controller::REQUEST_ACTION_OFFSET,
        )?)?;
        let raw_id = array_at(
            input,
            generated_general_controller::REQUEST_CANDIDATE_ID_OFFSET,
        )?;
        let value = Self {
            action,
            expected_revision: u64_at(
                input,
                generated_general_controller::REQUEST_EXPECTED_REVISION_OFFSET,
            )?,
            candidate_id: if is_zero(&raw_id) { None } else { Some(raw_id) },
            page_index: u32_at(
                input,
                generated_general_controller::REQUEST_PAGE_INDEX_OFFSET,
            )?,
            execution_index: byte_at(
                input,
                generated_general_controller::REQUEST_EXECUTION_INDEX_OFFSET,
            )?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(self) -> Result<[u8; CONTROLLER_REQUEST_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; CONTROLLER_REQUEST_BYTES];
        put_header(
            &mut output,
            &generated_general_controller::REQUEST_MAGIC,
            generated_general_controller::REQUEST_VERSION_OFFSET,
        )?;
        put_byte(
            &mut output,
            generated_general_controller::REQUEST_ACTION_OFFSET,
            self.action as u8,
        )?;
        put(
            &mut output,
            generated_general_controller::REQUEST_EXPECTED_REVISION_OFFSET,
            &self.expected_revision.to_le_bytes(),
        )?;
        if let Some(candidate_id) = self.candidate_id {
            put(
                &mut output,
                generated_general_controller::REQUEST_CANDIDATE_ID_OFFSET,
                &candidate_id,
            )?;
        }
        put(
            &mut output,
            generated_general_controller::REQUEST_PAGE_INDEX_OFFSET,
            &self.page_index.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            generated_general_controller::REQUEST_EXECUTION_INDEX_OFFSET,
            self.execution_index,
        )?;
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        if self.candidate_id.as_ref().is_some_and(is_zero) {
            return Err(Error::ZeroCoordinate);
        }
        match self.action {
            Action::Freeze
                if self.candidate_id.is_none()
                    && self.page_index == 0
                    && self.execution_index == 0 =>
            {
                Ok(())
            }
            Action::Consider
                if self.candidate_id.is_some()
                    && self.page_index < MAX_PAGES_PER_CANDIDATE
                    && self.execution_index == 0 =>
            {
                Ok(())
            }
            Action::Collect | Action::Distribute
                if self.candidate_id.is_some()
                    && self.page_index < MAX_PAGES_PER_CANDIDATE
                    && usize::from(self.execution_index) < MAX_EXECUTIONS_PER_PAGE =>
            {
                Ok(())
            }
            Action::InitializeSettlement | Action::Materialize | Action::Close
                if self.candidate_id.is_some()
                    && self.page_index == 0
                    && self.execution_index == 0 =>
            {
                Ok(())
            }
            _ => Err(Error::InvalidCursor),
        }
    }
}

fn header(input: &[u8], width: usize, magic: &[u8; 8], version_offset: usize) -> Result<()> {
    exact_width(input, width)?;
    exact(input, 0, magic, Error::InvalidMagic)?;
    if u16_at(input, version_offset)? != generated_general_controller::ABI_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    Ok(())
}

fn put_header(output: &mut [u8], magic: &[u8; 8], version_offset: usize) -> Result<()> {
    put(output, 0, magic)?;
    put(
        output,
        version_offset,
        &generated_general_controller::ABI_VERSION.to_le_bytes(),
    )
}

fn active_count(count: u8) -> Result<usize> {
    let count = usize::from(count);
    if count == 0 || count > MAX_OUTCOMES {
        Err(Error::InvalidCursor)
    } else {
        Ok(count)
    }
}

fn require_inactive_zero(values: &[u64; MAX_OUTCOMES], count: usize) -> Result<()> {
    if values
        .get(count..)
        .ok_or(Error::InvalidCursor)?
        .iter()
        .all(|value| *value == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalPadding)
    }
}

fn execution_offset(index: usize) -> Result<usize> {
    generated_general_controller::PAGE_EXECUTIONS_OFFSET
        .checked_add(
            index
                .checked_mul(EXECUTION_BYTES)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)
}

fn execution_slice(input: &[u8], index: usize) -> Result<&[u8]> {
    let start = execution_offset(index)?;
    let end = start
        .checked_add(EXECUTION_BYTES)
        .ok_or(Error::InvalidLength)?;
    input.get(start..end).ok_or(Error::InvalidLength)
}

fn exact_width(input: &[u8], expected: usize) -> Result<()> {
    if input.len() == expected {
        Ok(())
    } else {
        Err(Error::InvalidLength)
    }
}

fn exact(input: &[u8], offset: usize, expected: &[u8], error: Error) -> Result<()> {
    let end = offset
        .checked_add(expected.len())
        .ok_or(Error::InvalidLength)?;
    if input.get(offset..end) == Some(expected) {
        Ok(())
    } else {
        Err(error)
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalPadding)
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

fn put_u64_array(output: &mut [u8], offset: usize, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        let coordinate = offset
            .checked_add(index.checked_mul(8).ok_or(Error::InvalidLength)?)
            .ok_or(Error::InvalidLength)?;
        put(output, coordinate, &value.to_le_bytes())?;
    }
    Ok(())
}

fn byte_at(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn bool_at(input: &[u8], offset: usize) -> Result<bool> {
    match byte_at(input, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::NonCanonicalBoolean),
    }
}

fn array_at<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array_at(input, offset)?))
}

fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array_at(input, offset)?))
}

fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array_at(input, offset)?))
}

fn u64_array_at(input: &[u8], offset: usize) -> Result<[u64; MAX_OUTCOMES]> {
    let mut values = [0_u64; MAX_OUTCOMES];
    for (index, value) in values.iter_mut().enumerate() {
        let coordinate = offset
            .checked_add(index.checked_mul(8).ok_or(Error::InvalidLength)?)
            .ok_or(Error::InvalidLength)?;
        *value = u64_at(input, coordinate)?;
    }
    Ok(values)
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }

    fn execution() -> ExecutionV1 {
        ExecutionV1 {
            order_id: id(0x44),
            owner_id: id(0x55),
            nonce: 7,
            max_lots: 9,
            max_quote_debit_per_lot: 40,
            lots: 2,
            quote_debit: 80,
            quote_credit: 0,
            receive_per_lot: {
                let mut v = [0; MAX_OUTCOMES];
                v[0] = 2;
                v
            },
            deliver_per_lot: [0; MAX_OUTCOMES],
        }
    }

    fn candidate() -> CandidateV1 {
        CandidateV1 {
            outcome_count: 2,
            candidate_id: id(0x11),
            product_id: id(0x22),
            batch_id: id(0x33),
            page_count: 1,
            price_scale: 100,
            prices: {
                let mut v = [0; MAX_OUTCOMES];
                v[0] = 40;
                v[1] = 60;
                v
            },
        }
    }

    fn page() -> PageV1 {
        let mut executions = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        executions[0] = execution();
        PageV1 {
            outcome_count: 2,
            candidate_id: id(0x11),
            page_index: 0,
            page_count: 1,
            execution_count: 1,
            executions,
        }
    }

    #[test]
    fn all_lean_vectors_round_trip_exactly() -> Result<()> {
        let candidate_bytes = candidate().to_bytes()?;
        assert_eq!(
            candidate_bytes,
            generated_general_controller::CANDIDATE_EXAMPLE
        );
        assert_eq!(CandidateV1::decode(&candidate_bytes), Ok(candidate()));

        let page_bytes = page().to_bytes()?;
        assert_eq!(page_bytes, generated_general_controller::PAGE_EXAMPLE);
        assert_eq!(
            &page_bytes[generated_general_controller::PAGE_EXECUTIONS_OFFSET
                ..generated_general_controller::PAGE_EXECUTIONS_OFFSET + EXECUTION_BYTES],
            &generated_general_controller::EXECUTION_EXAMPLE
        );
        assert_eq!(PageV1::decode(&page_bytes), Ok(page()));

        let policy = SelectionPolicyV1 {
            policy_id: id(0x66),
            criterion_count: 3,
            criteria: {
                let mut values = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
                values[1] = SelectionCriterion::MinimizeQuoteSurplus;
                values[2] = SelectionCriterion::MinimizeCandidateId;
                values
            },
        };
        let policy_bytes = policy.to_bytes()?;
        assert_eq!(policy_bytes, generated_general_controller::POLICY_EXAMPLE);
        assert_eq!(SelectionPolicyV1::decode(&policy_bytes), Ok(policy));

        let selection = SelectionCursorV1 {
            closed: true,
            batch_id: id(0x33),
            policy_id: id(0x66),
            best_candidate_id: Some(id(0x11)),
            revision: 2,
        };
        let selection_bytes = selection.to_bytes()?;
        assert_eq!(
            selection_bytes,
            generated_general_controller::SELECTION_EXAMPLE
        );
        assert_eq!(SelectionCursorV1::decode(&selection_bytes), Ok(selection));

        let settlement = SettlementCursorV1 {
            phase: Phase::Collecting,
            outcome_count: 2,
            candidate_id: id(0x11),
            page_count: 1,
            next_page: 0,
            next_execution: 0,
            revision: 3,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 0,
            quote_surplus_paid: 0,
        };
        let settlement_bytes = settlement.to_bytes()?;
        assert_eq!(
            settlement_bytes,
            generated_general_controller::SETTLEMENT_EXAMPLE
        );
        assert_eq!(
            SettlementCursorV1::decode(&settlement_bytes),
            Ok(settlement)
        );

        let request = ControllerRequestV1 {
            action: Action::Collect,
            expected_revision: 3,
            candidate_id: Some(id(0x11)),
            page_index: 0,
            execution_index: 0,
        };
        let request_bytes = request.to_bytes()?;
        assert_eq!(request_bytes, generated_general_controller::REQUEST_EXAMPLE);
        assert_eq!(ControllerRequestV1::decode(&request_bytes), Ok(request));
        Ok(())
    }

    #[test]
    fn every_object_refuses_truncation_and_extension() -> Result<()> {
        let candidate = candidate().to_bytes()?;
        for length in 0..CANDIDATE_BYTES {
            assert_eq!(
                CandidateV1::decode(candidate.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }
        let mut candidate_long = [0_u8; CANDIDATE_BYTES + 1];
        candidate_long[..CANDIDATE_BYTES].copy_from_slice(&candidate);
        assert_eq!(
            CandidateV1::decode(&candidate_long),
            Err(Error::InvalidLength)
        );

        let policy = SelectionPolicyV1 {
            policy_id: id(1),
            criterion_count: 1,
            criteria: {
                let mut values = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
                values[0] = SelectionCriterion::MinimizeCandidateId;
                values
            },
        }
        .to_bytes()?;
        for length in 0..SELECTION_POLICY_BYTES {
            assert_eq!(
                SelectionPolicyV1::decode(policy.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }

        let page = page().to_bytes()?;
        for length in 0..PAGE_BYTES {
            assert_eq!(
                PageV1::decode(page.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }
        let request = ControllerRequestV1 {
            action: Action::Collect,
            expected_revision: 3,
            candidate_id: Some(id(0x11)),
            page_index: 0,
            execution_index: 0,
        }
        .to_bytes()?;
        for length in 0..CONTROLLER_REQUEST_BYTES {
            assert_eq!(
                ControllerRequestV1::decode(request.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }

        let selection = SelectionCursorV1 {
            closed: false,
            batch_id: id(1),
            policy_id: id(2),
            best_candidate_id: None,
            revision: 0,
        }
        .to_bytes()?;
        for length in 0..SELECTION_CURSOR_BYTES {
            assert_eq!(
                SelectionCursorV1::decode(selection.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }

        let settlement = SettlementCursorV1 {
            phase: Phase::Collecting,
            outcome_count: 2,
            candidate_id: id(1),
            page_count: 1,
            next_page: 0,
            next_execution: 0,
            revision: 0,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 0,
            quote_surplus_paid: 0,
        }
        .to_bytes()?;
        for length in 0..SETTLEMENT_CURSOR_BYTES {
            assert_eq!(
                SettlementCursorV1::decode(settlement.get(..length).ok_or(Error::InvalidLength)?),
                Err(Error::InvalidLength)
            );
        }
        Ok(())
    }

    #[test]
    fn hostile_headers_counts_padding_and_overflow_refuse() -> Result<()> {
        let mut candidate = candidate().to_bytes()?;
        candidate[0] ^= 1;
        assert_eq!(CandidateV1::decode(&candidate), Err(Error::InvalidMagic));
        candidate = super::tests::candidate().to_bytes()?;
        candidate[generated_general_controller::CANDIDATE_RESERVED_A_OFFSET] = 1;
        assert_eq!(
            CandidateV1::decode(&candidate),
            Err(Error::NonCanonicalPadding)
        );
        candidate = super::tests::candidate().to_bytes()?;
        candidate[generated_general_controller::CANDIDATE_OUTCOME_COUNT_OFFSET] = 1;
        assert_eq!(
            CandidateV1::decode(&candidate),
            Err(Error::NonCanonicalPadding)
        );

        let mut overflow = super::tests::candidate();
        overflow.price_scale = u64::MAX;
        overflow.prices[0] = u64::MAX;
        overflow.prices[1] = 1;
        assert_eq!(overflow.to_bytes(), Err(Error::ArithmeticOverflow));

        let mut page = super::tests::page().to_bytes()?;
        page[generated_general_controller::PAGE_EXECUTION_COUNT_OFFSET] = 0;
        assert_eq!(PageV1::decode(&page), Err(Error::InvalidCursor));
        page = super::tests::page().to_bytes()?;
        page[generated_general_controller::PAGE_EXECUTIONS_OFFSET + EXECUTION_BYTES] = 1;
        assert_eq!(PageV1::decode(&page), Err(Error::NonCanonicalPadding));

        let mut policy = SelectionPolicyV1 {
            policy_id: id(1),
            criterion_count: 2,
            criteria: {
                let mut values = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
                values[1] = SelectionCriterion::MinimizeCandidateId;
                values
            },
        }
        .to_bytes()?;
        policy[generated_general_controller::POLICY_CRITERIA_OFFSET + 2] = 1;
        assert_eq!(
            SelectionPolicyV1::decode(&policy),
            Err(Error::NonCanonicalPadding)
        );
        policy[generated_general_controller::POLICY_CRITERIA_OFFSET + 2] = 0;
        policy[generated_general_controller::POLICY_CRITERIA_OFFSET + 1] = 1;
        assert_eq!(
            SelectionPolicyV1::decode(&policy),
            Err(Error::InvalidCursor)
        );
        Ok(())
    }

    #[test]
    fn hostile_tags_booleans_and_phase_cursors_refuse() -> Result<()> {
        let selection = SelectionCursorV1 {
            closed: false,
            batch_id: id(1),
            policy_id: id(2),
            best_candidate_id: None,
            revision: 0,
        };
        let mut bytes = selection.to_bytes()?;
        bytes[generated_general_controller::SELECTION_CLOSED_OFFSET] = 2;
        assert_eq!(
            SelectionCursorV1::decode(&bytes),
            Err(Error::NonCanonicalBoolean)
        );
        bytes = selection.to_bytes()?;
        bytes[generated_general_controller::SELECTION_BEST_CANDIDATE_ID_OFFSET] = 1;
        assert_eq!(
            SelectionCursorV1::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );

        let request = ControllerRequestV1 {
            action: Action::Freeze,
            expected_revision: 4,
            candidate_id: None,
            page_index: 0,
            execution_index: 0,
        };
        let mut request_bytes = request.to_bytes()?;
        request_bytes[generated_general_controller::REQUEST_ACTION_OFFSET] = 255;
        assert_eq!(
            ControllerRequestV1::decode(&request_bytes),
            Err(Error::UnknownTag)
        );

        let terminal = SettlementCursorV1 {
            phase: Phase::Terminal,
            outcome_count: 2,
            candidate_id: id(1),
            page_count: 1,
            next_page: 0,
            next_execution: 0,
            revision: 8,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 0,
            quote_surplus_paid: 3,
        };
        let mut terminal_bytes = terminal.to_bytes()?;
        terminal_bytes[generated_general_controller::SETTLEMENT_QUOTE_INVENTORY_OFFSET] = 1;
        assert_eq!(
            SettlementCursorV1::decode(&terminal_bytes),
            Err(Error::NonCanonicalPadding)
        );
        Ok(())
    }

    #[test]
    fn quote_portions_are_not_a_fragment_local_rounding_boundary() -> Result<()> {
        let first = ExecutionV1 {
            quote_debit: 1,
            ..execution()
        };
        let second = ExecutionV1 {
            quote_debit: 0,
            ..execution()
        };
        let mut executions = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        executions[0] = first;
        executions[1] = second;
        let fragmented = PageV1 {
            outcome_count: 2,
            candidate_id: id(9),
            page_index: 0,
            page_count: 1,
            execution_count: 2,
            executions,
        };
        assert!(fragmented.to_bytes().is_ok());
        Ok(())
    }
}
