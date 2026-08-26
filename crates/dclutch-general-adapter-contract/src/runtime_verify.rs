//! Runtime-width candidate verification for the successor General vertical.
//!
//! The compact page row is only a transport projection. Generic Trading must
//! authenticate the immutable order identified by the row before constructing
//! [`AuthenticatedOrderTermsV2`]. This evaluator then derives quote debit and
//! credit itself, enforces the signed debit limit, aggregates claim movement,
//! and emits one complete runtime-width verified-candidate record. It owns no
//! accounts, performs no CPI, and copies caller-owned candidate outputs only
//! after the complete row transition accepts.

use dclutch_general_codec::{SelectionCriterion, SelectionPolicyV1};

use crate::runtime_width::{
    CANDIDATE_HEADER_BYTES_V2, CandidateV2, PageV2, RuntimeWidthErrorV2, VerifiedCandidateHeaderV2,
    VerifiedCandidateV2, verified_candidate_len,
};

/// Exact fixed bytes before five runtime-width `u64` tails in the verifier.
pub const RUNTIME_VERIFIER_HEADER_BYTES_V2: usize = 288;

const VERIFIER_MAGIC: [u8; 8] = *b"DCGVFY02";
const VERSION: u16 = 2;
const PRICES_TAIL: usize = 0;
const CURRENT_RECEIVE_TAIL: usize = 1;
const CURRENT_DELIVER_TAIL: usize = 2;
const CLAIM_INPUTS_TAIL: usize = 3;
const CLAIM_OUTPUTS_TAIL: usize = 4;

/// Stable refusal from runtime-width candidate verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeVerifyErrorV2 {
    /// A Candidate, Page, Execution, or verified-candidate record refused.
    Codec,
    /// A caller-owned cursor or certificate bank had another exact width.
    InvalidLength,
    /// A checked scalar, tail, or byte calculation overflowed.
    ArithmeticOverflow,
    /// Candidate, Page, row, or optimistic revision coordinates differed.
    CoordinateMismatch,
    /// The authenticated immutable order did not match its compact row.
    AuthenticatedOrderMismatch,
    /// Candidate rows were not globally grouped by increasing order identity.
    NonCanonicalOrder,
    /// Two fragments of one order carried different immutable terms.
    OrderSubstitution,
    /// The immutable maximum number of orders was exceeded.
    TooManyOrders,
    /// Candidate-wide lots exceeded the signed order maximum.
    ExcessLots,
    /// Derived quote debit exceeded the authenticated signed-order limit.
    QuoteLimit,
    /// Aggregate claim inputs and outputs had no uniform complete-set delta.
    ClaimImbalance,
    /// Derived quote inventory could not fund the complete-set move and credits.
    QuoteImbalance,
    /// A persisted verifier cursor was hostile or noncanonical.
    InvalidCursor,
    /// Candidate comparison used different Product, Batch, width, or scale.
    ComparisonDomain,
}

/// Result alias for runtime-width candidate verification.
pub type RuntimeVerifyResultV2<T> = core::result::Result<T, RuntimeVerifyErrorV2>;

/// Already-authenticated immutable order terms omitted from the compact row.
///
/// Generic Trading constructs this value only after its selected Account and
/// Request Profiles authenticate the finalized order record and bind
/// `order_id` to its exact contents. This value is not itself an authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedOrderTermsV2 {
    /// Immutable order content identity.
    pub order_id: [u8; 32],
    /// Immutable order owner identity.
    pub owner_id: [u8; 32],
    /// Signed-order nonce.
    pub nonce: u64,
    /// Candidate-wide maximum fill.
    pub max_lots: u64,
    /// Candidate-wide maximum derived quote debit per filled lot.
    pub max_quote_debit_per_lot: u64,
}

/// Fixed fields decoded from one persisted runtime-width verifier cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeVerifierHeaderV2 {
    /// Runtime outcome width.
    pub outcome_count: u32,
    /// Total immutable candidate pages.
    pub page_count: u32,
    /// Zero-based next page index, equal to `page_count` only when complete.
    pub next_page_index: u32,
    /// Zero-based next row index inside the next page.
    pub next_row_index: u32,
    /// Number of distinct globally grouped orders consumed.
    pub order_count: u32,
    /// Optimistic revision, advanced exactly once per row.
    pub revision: u64,
    /// Immutable candidate coordinate in its Batch.
    pub candidate_coordinate: u32,
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Product content identity.
    pub product_id: [u8; 32],
    /// Batch content identity.
    pub batch_id: [u8; 32],
    /// Exact price denominator.
    pub price_scale: u64,
    /// Candidate-wide filled lots.
    pub filled_lots: u64,
    /// Candidate-wide derived quote debit.
    pub quote_debit: u64,
    /// Candidate-wide derived quote credit.
    pub quote_credit: u64,
    /// Whether an unfinished globally grouped order is present.
    pub has_current_order: bool,
}

/// Borrowed hostile-decoded runtime-width verifier cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCandidateVerifierV2<'a> {
    bytes: &'a [u8],
    header: RuntimeVerifierHeaderV2,
}

impl<'a> RuntimeCandidateVerifierV2<'a> {
    /// Hostile-decode one exact `288 + 40N` verifier cursor.
    pub fn decode(bytes: &'a [u8]) -> RuntimeVerifyResultV2<Self> {
        if bytes.len() < RUNTIME_VERIFIER_HEADER_BYTES_V2
            || bytes.get(..8) != Some(VERIFIER_MAGIC.as_slice())
            || read_u16(bytes, 8)? != VERSION
            || !zero_range(bytes, 11, 1)?
            || !zero_range(bytes, 44, 4)?
            || !zero_range(bytes, 272, 16)?
        {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
        let has_current_order = match read_byte(bytes, 10)? {
            0 => false,
            1 => true,
            _ => return Err(RuntimeVerifyErrorV2::InvalidCursor),
        };
        let header = RuntimeVerifierHeaderV2 {
            outcome_count: read_u32(bytes, 12)?,
            page_count: read_u32(bytes, 16)?,
            next_page_index: read_u32(bytes, 20)?,
            next_row_index: read_u32(bytes, 24)?,
            order_count: read_u32(bytes, 28)?,
            revision: read_u64(bytes, 32)?,
            candidate_coordinate: read_u32(bytes, 40)?,
            candidate_id: read_array32(bytes, 48)?,
            product_id: read_array32(bytes, 80)?,
            batch_id: read_array32(bytes, 112)?,
            price_scale: read_u64(bytes, 144)?,
            filled_lots: read_u64(bytes, 152)?,
            quote_debit: read_u64(bytes, 160)?,
            quote_credit: read_u64(bytes, 168)?,
            has_current_order,
        };
        if bytes.len() != runtime_verifier_len_v2(header.outcome_count)? {
            return Err(RuntimeVerifyErrorV2::InvalidLength);
        }
        validate_cursor(bytes, header)?;
        Ok(Self { bytes, header })
    }

    /// Return the fixed verifier coordinates and aggregates.
    pub const fn header(self) -> RuntimeVerifierHeaderV2 {
        self.header
    }

    /// Return whether all declared pages have been consumed.
    pub const fn is_complete(self) -> bool {
        self.header.next_page_index == self.header.page_count
    }

    /// Return one checked exact simplex price.
    pub fn price(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(self.bytes, self.header.outcome_count, PRICES_TAIL, index)
    }

    /// Return one checked aggregate claim input.
    pub fn claim_input(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            CLAIM_INPUTS_TAIL,
            index,
        )
    }

    /// Return one checked aggregate claim output.
    pub fn claim_output(self, index: u32) -> RuntimeVerifyResultV2<u64> {
        read_tail_u64(
            self.bytes,
            self.header.outcome_count,
            CLAIM_OUTPUTS_TAIL,
            index,
        )
    }

    /// Return the exact hostile-decoded cursor bytes.
    pub const fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Readonly inputs for one exact runtime-width candidate row.
pub struct RuntimeConsiderRowViewV2<'a> {
    /// Immutable runtime-width Candidate record.
    pub candidate: &'a [u8],
    /// Immutable runtime-width Page containing the next row.
    pub page: &'a [u8],
    /// All-zero initial or canonical persisted verifier cursor.
    pub cursor_before: &'a [u8],
    /// All-zero candidate certificate destination.
    pub verified_before: &'a [u8],
    /// Authenticated order terms bound by the compact row's `order_id`.
    pub authenticated_order: AuthenticatedOrderTermsV2,
    /// Zero-based optimistic page index.
    pub expected_page_index: u32,
    /// Zero-based optimistic row index.
    pub expected_row_index: u32,
    /// Exact immutable Page revision selected by authenticated state.
    pub expected_page_revision: u64,
    /// Exact optimistic verifier revision.
    pub expected_revision: u64,
    /// Immutable positive order-count envelope.
    pub max_orders: u32,
}

/// Scratch and candidate banks for one failure-atomic runtime row.
pub struct RuntimeConsiderRowBuffersV2<'a> {
    /// Non-authoritative verifier scratch; may change on refusal.
    pub cursor_scratch: &'a mut [u8],
    /// Complete verifier candidate; unchanged on refusal.
    pub cursor_output: &'a mut [u8],
    /// Non-authoritative verified-candidate scratch; may change on refusal.
    pub verified_scratch: &'a mut [u8],
    /// Complete verified-candidate candidate; unchanged on refusal.
    pub verified_output: &'a mut [u8],
}

/// Accepted summary for one runtime-width candidate row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConsiderRowSummaryV2 {
    /// Whether the row completed every declared candidate page.
    pub complete: bool,
    /// Exact distinct globally grouped order count.
    pub order_count: u32,
    /// Exact successor verifier revision.
    pub revision: u64,
}

/// Complete-set direction derived from a verified candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCompleteSetMoveV2 {
    /// Claim inputs and outputs are equal.
    None,
    /// Outputs exceed inputs uniformly and require one complete-set mint.
    Mint,
    /// Inputs exceed outputs uniformly and require one complete-set merge.
    Merge,
}

/// Exact materialization and terminal quote consequence of a certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCandidateBalanceV2 {
    /// Sole complete-set direction.
    pub complete_set_move: RuntimeCompleteSetMoveV2,
    /// Uniform quantity minted or merged.
    pub complete_set_quantity: u64,
    /// Exact quote remainder after materialization and credits.
    pub quote_surplus: u64,
}

/// Return the exact `288 + 40N` runtime verifier cursor width.
pub fn runtime_verifier_len_v2(outcome_count: u32) -> RuntimeVerifyResultV2<usize> {
    if outcome_count == 0 {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let count =
        usize::try_from(outcome_count).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    RUNTIME_VERIFIER_HEADER_BYTES_V2
        .checked_add(
            count
                .checked_mul(40)
                .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

/// Evaluate one exact row and copy complete candidate banks only on success.
///
/// A zero cursor starts at page and row index zero. Nonzero cursors must match
/// every optimistic coordinate. The final row also derives the exact complete
/// verified-candidate record; earlier rows leave the all-zero certificate
/// destination unchanged.
#[inline(never)]
pub fn evaluate_runtime_consider_row_v2(
    view: RuntimeConsiderRowViewV2<'_>,
    buffers: RuntimeConsiderRowBuffersV2<'_>,
) -> RuntimeVerifyResultV2<RuntimeConsiderRowSummaryV2> {
    let candidate = CandidateV2::decode(view.candidate).map_err(map_codec)?;
    let page = PageV2::decode(view.page).map_err(map_codec)?;
    let candidate_header = candidate.header();
    let cursor_len = runtime_verifier_len_v2(candidate_header.outcome_count)?;
    let verified_len = verified_candidate_len(candidate_header.outcome_count).map_err(map_codec)?;
    if view.cursor_before.len() != cursor_len
        || buffers.cursor_scratch.len() != cursor_len
        || buffers.cursor_output.len() != cursor_len
        || view.verified_before.len() != verified_len
        || buffers.verified_scratch.len() != verified_len
        || buffers.verified_output.len() != verified_len
        || view.max_orders == 0
        || view.verified_before.iter().any(|byte| *byte != 0)
    {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let page_header = page.header();
    let expected_page_coordinate = view
        .expected_page_index
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    if page.row_count() == 0
        || page_header.outcome_count != candidate_header.outcome_count
        || page_header.page_count != candidate_header.page_count
        || page_header.page_coordinate != expected_page_coordinate
        || page_header.revision != view.expected_page_revision
        || page_header.candidate_id != candidate_header.candidate_id
        || view.expected_row_index >= page.row_count()
    {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    let execution = page.execution(view.expected_row_index).map_err(map_codec)?;
    require_authenticated_order(execution.header(), view.authenticated_order)?;

    if view.cursor_before.iter().all(|byte| *byte == 0) {
        if view.expected_page_index != 0
            || view.expected_row_index != 0
            || view.expected_revision != 0
        {
            return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
        }
        initialize_cursor(candidate, buffers.cursor_scratch)?;
    } else {
        let before = RuntimeCandidateVerifierV2::decode(view.cursor_before)?;
        require_candidate(before, candidate)?;
        let before_header = before.header();
        if before_header.next_page_index != view.expected_page_index
            || before_header.next_row_index != view.expected_row_index
            || before_header.revision != view.expected_revision
            || before_header.order_count > view.max_orders
            || before.is_complete()
        {
            return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
        }
        buffers.cursor_scratch.copy_from_slice(view.cursor_before);
    }
    buffers.verified_scratch.fill(0);
    ingest_execution(
        buffers.cursor_scratch,
        execution,
        view.authenticated_order,
        view.max_orders,
    )?;

    let next_row = view
        .expected_row_index
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    if next_row == page.row_count() {
        put_u32(
            buffers.cursor_scratch,
            20,
            view.expected_page_index
                .checked_add(1)
                .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )?;
        put_u32(buffers.cursor_scratch, 24, 0)?;
    } else {
        put_u32(buffers.cursor_scratch, 24, next_row)?;
    }
    let successor_revision = view
        .expected_revision
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    put_u64(buffers.cursor_scratch, 32, successor_revision)?;

    let mut complete = false;
    let reached_terminal_page =
        read_u32(buffers.cursor_scratch, 20)? == candidate_header.page_count;
    if reached_terminal_page {
        finalize_current_order(buffers.cursor_scratch)?;
        let completed = RuntimeCandidateVerifierV2::decode(buffers.cursor_scratch)?;
        let balance = balance_from_cursor(completed)?;
        let header = completed.header();
        let inputs = tail_bytes(
            buffers.cursor_scratch,
            header.outcome_count,
            CLAIM_INPUTS_TAIL,
        )?;
        let outputs = tail_bytes(
            buffers.cursor_scratch,
            header.outcome_count,
            CLAIM_OUTPUTS_TAIL,
        )?;
        VerifiedCandidateV2::encode_le_tails_into(
            VerifiedCandidateHeaderV2 {
                outcome_count: header.outcome_count,
                page_count: header.page_count,
                candidate_coordinate: header.candidate_coordinate,
                revision: header.revision,
                candidate_id: header.candidate_id,
                product_id: header.product_id,
                batch_id: header.batch_id,
                filled_lots: header.filled_lots,
                quote_debit: header.quote_debit,
                quote_credit: header.quote_credit,
                price_scale: header.price_scale,
            },
            inputs,
            outputs,
            buffers.verified_scratch,
        )
        .map_err(map_codec)?;
        if runtime_verified_balance_v2(buffers.verified_scratch)? != balance {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
        complete = true;
    } else {
        RuntimeCandidateVerifierV2::decode(buffers.cursor_scratch)?;
    }

    let accepted = RuntimeCandidateVerifierV2::decode(buffers.cursor_scratch)?;
    buffers
        .cursor_output
        .copy_from_slice(buffers.cursor_scratch);
    if complete {
        buffers
            .verified_output
            .copy_from_slice(buffers.verified_scratch);
    }
    Ok(RuntimeConsiderRowSummaryV2 {
        complete,
        order_count: accepted.header().order_count,
        revision: accepted.header().revision,
    })
}

/// Derive the unique complete-set movement and exact quote surplus.
pub fn runtime_verified_balance_v2(
    verified_bytes: &[u8],
) -> RuntimeVerifyResultV2<RuntimeCandidateBalanceV2> {
    let verified = VerifiedCandidateV2::decode(verified_bytes).map_err(map_codec)?;
    let header = verified.header();
    derive_balance(
        header.outcome_count,
        |index| verified.claim_input(index).map_err(map_codec),
        |index| verified.claim_output(index).map_err(map_codec),
        header.quote_debit,
        header.quote_credit,
    )
}

/// Compare two valid submitted candidates under immutable interpreted policy.
///
/// This does not claim global optimality. It implements only the exact
/// lexicographic comparison used to maintain the best valid submitted
/// candidate among candidates the protocol has actually verified.
pub fn runtime_candidate_better_v2(
    policy: &SelectionPolicyV1,
    left_bytes: &[u8],
    right_bytes: &[u8],
) -> RuntimeVerifyResultV2<bool> {
    let left = VerifiedCandidateV2::decode(left_bytes).map_err(map_codec)?;
    let right = VerifiedCandidateV2::decode(right_bytes).map_err(map_codec)?;
    let left_header = left.header();
    let right_header = right.header();
    if left_header.product_id != right_header.product_id
        || left_header.batch_id != right_header.batch_id
        || left_header.outcome_count != right_header.outcome_count
        || left_header.price_scale != right_header.price_scale
    {
        return Err(RuntimeVerifyErrorV2::ComparisonDomain);
    }
    let left_balance = runtime_verified_balance_v2(left_bytes)?;
    let right_balance = runtime_verified_balance_v2(right_bytes)?;
    for criterion in policy
        .criteria
        .iter()
        .take(usize::from(policy.criterion_count))
    {
        match criterion {
            SelectionCriterion::MaximizeFilledLots
                if left_header.filled_lots != right_header.filled_lots =>
            {
                return Ok(left_header.filled_lots > right_header.filled_lots);
            }
            SelectionCriterion::MinimizeQuoteSurplus
                if left_balance.quote_surplus != right_balance.quote_surplus =>
            {
                return Ok(left_balance.quote_surplus < right_balance.quote_surplus);
            }
            SelectionCriterion::MinimizeCandidateId
                if left_header.candidate_id != right_header.candidate_id =>
            {
                return Ok(le_numeric_id(
                    &left_header.candidate_id,
                    &right_header.candidate_id,
                ));
            }
            _ => {}
        }
    }
    Ok(false)
}

fn initialize_cursor(candidate: CandidateV2<'_>, output: &mut [u8]) -> RuntimeVerifyResultV2<()> {
    let header = candidate.header();
    if output.len() != runtime_verifier_len_v2(header.outcome_count)? {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    output.fill(0);
    put(output, 0, &VERIFIER_MAGIC)?;
    put_u16(output, 8, VERSION)?;
    put_u32(output, 12, header.outcome_count)?;
    put_u32(output, 16, header.page_count)?;
    put_u32(output, 40, header.candidate_coordinate)?;
    put(output, 48, &header.candidate_id)?;
    put(output, 80, &header.product_id)?;
    put(output, 112, &header.batch_id)?;
    put_u64(output, 144, header.price_scale)?;
    let prices = candidate
        .as_bytes()
        .get(CANDIDATE_HEADER_BYTES_V2..)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    put(
        output,
        tail_offset(header.outcome_count, PRICES_TAIL)?,
        prices,
    )
}

fn require_candidate(
    cursor: RuntimeCandidateVerifierV2<'_>,
    candidate: CandidateV2<'_>,
) -> RuntimeVerifyResultV2<()> {
    let observed = cursor.header();
    let expected = candidate.header();
    if observed.outcome_count != expected.outcome_count
        || observed.page_count != expected.page_count
        || observed.candidate_coordinate != expected.candidate_coordinate
        || observed.price_scale != expected.price_scale
        || observed.candidate_id != expected.candidate_id
        || observed.product_id != expected.product_id
        || observed.batch_id != expected.batch_id
    {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    let candidate_prices = candidate
        .as_bytes()
        .get(CANDIDATE_HEADER_BYTES_V2..)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    if tail_bytes(cursor.as_bytes(), observed.outcome_count, PRICES_TAIL)? != candidate_prices {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    Ok(())
}

fn require_authenticated_order(
    execution: crate::runtime_width::ExecutionHeaderV2,
    order: AuthenticatedOrderTermsV2,
) -> RuntimeVerifyResultV2<()> {
    if zero_identity(&order.order_id)
        || zero_identity(&order.owner_id)
        || order.max_lots == 0
        || execution.order_id != order.order_id
        || execution.owner_id != order.owner_id
        || execution.nonce != order.nonce
        || execution.max_lots != order.max_lots
    {
        Err(RuntimeVerifyErrorV2::AuthenticatedOrderMismatch)
    } else {
        Ok(())
    }
}

fn ingest_execution(
    cursor: &mut [u8],
    execution: crate::runtime_width::ExecutionV2<'_>,
    order: AuthenticatedOrderTermsV2,
    max_orders: u32,
) -> RuntimeVerifyResultV2<()> {
    let before = RuntimeCandidateVerifierV2::decode(cursor)?;
    let header = before.header();
    let execution_header = execution.header();
    if execution_header.outcome_count != header.outcome_count {
        return Err(RuntimeVerifyErrorV2::CoordinateMismatch);
    }
    if header.has_current_order {
        let current_id = read_array32(cursor, 176)?;
        if current_id == execution_header.order_id {
            require_same_order(cursor, execution, order)?;
        } else {
            if !le_numeric_id(&current_id, &execution_header.order_id) {
                return Err(RuntimeVerifyErrorV2::NonCanonicalOrder);
            }
            finalize_current_order(cursor)?;
            start_current_order(cursor, execution, order, max_orders)?;
        }
    } else {
        start_current_order(cursor, execution, order, max_orders)?;
    }

    let lots = execution_header.lots;
    let current_lots = add(read_u64(cursor, 264)?, lots)?;
    if current_lots > order.max_lots {
        return Err(RuntimeVerifyErrorV2::ExcessLots);
    }
    put_u64(cursor, 264, current_lots)?;
    put_u64(cursor, 152, add(read_u64(cursor, 152)?, lots)?)?;
    for outcome in 0..header.outcome_count {
        let receive = execution.receive_per_lot(outcome).map_err(map_codec)?;
        let deliver = execution.deliver_per_lot(outcome).map_err(map_codec)?;
        add_tail_u64(
            cursor,
            header.outcome_count,
            CLAIM_INPUTS_TAIL,
            outcome,
            multiply(deliver, lots)?,
        )?;
        add_tail_u64(
            cursor,
            header.outcome_count,
            CLAIM_OUTPUTS_TAIL,
            outcome,
            multiply(receive, lots)?,
        )?;
    }
    Ok(())
}

fn start_current_order(
    cursor: &mut [u8],
    execution: crate::runtime_width::ExecutionV2<'_>,
    order: AuthenticatedOrderTermsV2,
    max_orders: u32,
) -> RuntimeVerifyResultV2<()> {
    let header = RuntimeCandidateVerifierV2::decode(cursor)?.header();
    let next_count = header
        .order_count
        .checked_add(1)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    if next_count > max_orders {
        return Err(RuntimeVerifyErrorV2::TooManyOrders);
    }
    put_byte(cursor, 10, 1)?;
    put_u32(cursor, 28, next_count)?;
    put(cursor, 176, &order.order_id)?;
    put(cursor, 208, &order.owner_id)?;
    put_u64(cursor, 240, order.nonce)?;
    put_u64(cursor, 248, order.max_lots)?;
    put_u64(cursor, 256, order.max_quote_debit_per_lot)?;
    put_u64(cursor, 264, 0)?;
    let count = header.outcome_count;
    for outcome in 0..count {
        write_tail_u64(
            cursor,
            count,
            CURRENT_RECEIVE_TAIL,
            outcome,
            execution.receive_per_lot(outcome).map_err(map_codec)?,
        )?;
        write_tail_u64(
            cursor,
            count,
            CURRENT_DELIVER_TAIL,
            outcome,
            execution.deliver_per_lot(outcome).map_err(map_codec)?,
        )?;
    }
    Ok(())
}

fn require_same_order(
    cursor: &[u8],
    execution: crate::runtime_width::ExecutionV2<'_>,
    order: AuthenticatedOrderTermsV2,
) -> RuntimeVerifyResultV2<()> {
    let header = RuntimeCandidateVerifierV2::decode(cursor)?.header();
    if read_array32(cursor, 176)? != order.order_id
        || read_array32(cursor, 208)? != order.owner_id
        || read_u64(cursor, 240)? != order.nonce
        || read_u64(cursor, 248)? != order.max_lots
        || read_u64(cursor, 256)? != order.max_quote_debit_per_lot
    {
        return Err(RuntimeVerifyErrorV2::OrderSubstitution);
    }
    for outcome in 0..header.outcome_count {
        if read_tail_u64(cursor, header.outcome_count, CURRENT_RECEIVE_TAIL, outcome)?
            != execution.receive_per_lot(outcome).map_err(map_codec)?
            || read_tail_u64(cursor, header.outcome_count, CURRENT_DELIVER_TAIL, outcome)?
                != execution.deliver_per_lot(outcome).map_err(map_codec)?
        {
            return Err(RuntimeVerifyErrorV2::OrderSubstitution);
        }
    }
    Ok(())
}

fn finalize_current_order(cursor: &mut [u8]) -> RuntimeVerifyResultV2<()> {
    let decoded = RuntimeCandidateVerifierV2::decode(cursor)?;
    let header = decoded.header();
    if !header.has_current_order {
        return Ok(());
    }
    let lots = read_u64(cursor, 264)?;
    let max_lots = read_u64(cursor, 248)?;
    if lots == 0 || lots > max_lots {
        return Err(RuntimeVerifyErrorV2::ExcessLots);
    }
    let mut received_per_lot = 0_u64;
    let mut delivered_per_lot = 0_u64;
    for outcome in 0..header.outcome_count {
        let price = decoded.price(outcome)?;
        received_per_lot = add(
            received_per_lot,
            multiply(
                price,
                read_tail_u64(cursor, header.outcome_count, CURRENT_RECEIVE_TAIL, outcome)?,
            )?,
        )?;
        delivered_per_lot = add(
            delivered_per_lot,
            multiply(
                price,
                read_tail_u64(cursor, header.outcome_count, CURRENT_DELIVER_TAIL, outcome)?,
            )?,
        )?;
    }
    let received = multiply(received_per_lot, lots)?;
    let delivered = multiply(delivered_per_lot, lots)?;
    let (debit, credit) = if delivered <= received {
        let difference = received - delivered;
        let rounded = add(difference, header.price_scale - 1)? / header.price_scale;
        (rounded, 0)
    } else {
        (0, (delivered - received) / header.price_scale)
    };
    let debit_limit = multiply(read_u64(cursor, 256)?, lots)?;
    if debit > debit_limit {
        return Err(RuntimeVerifyErrorV2::QuoteLimit);
    }
    put_u64(cursor, 160, add(header.quote_debit, debit)?)?;
    put_u64(cursor, 168, add(header.quote_credit, credit)?)?;
    put_byte(cursor, 10, 0)?;
    zero_mut(cursor, 176, 96)?;
    zero_tail(cursor, header.outcome_count, CURRENT_RECEIVE_TAIL)?;
    zero_tail(cursor, header.outcome_count, CURRENT_DELIVER_TAIL)
}

fn balance_from_cursor(
    cursor: RuntimeCandidateVerifierV2<'_>,
) -> RuntimeVerifyResultV2<RuntimeCandidateBalanceV2> {
    let header = cursor.header();
    derive_balance(
        header.outcome_count,
        |index| cursor.claim_input(index),
        |index| cursor.claim_output(index),
        header.quote_debit,
        header.quote_credit,
    )
}

fn derive_balance(
    count: u32,
    mut input: impl FnMut(u32) -> RuntimeVerifyResultV2<u64>,
    mut output: impl FnMut(u32) -> RuntimeVerifyResultV2<u64>,
    quote_debit: u64,
    quote_credit: u64,
) -> RuntimeVerifyResultV2<RuntimeCandidateBalanceV2> {
    if count == 0 {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    let first_input = input(0)?;
    let first_output = output(0)?;
    let (complete_set_move, quantity) = if first_input == first_output {
        (RuntimeCompleteSetMoveV2::None, 0)
    } else if first_input < first_output {
        (RuntimeCompleteSetMoveV2::Mint, first_output - first_input)
    } else {
        (RuntimeCompleteSetMoveV2::Merge, first_input - first_output)
    };
    for outcome in 0..count {
        let observed_input = input(outcome)?;
        let observed_output = output(outcome)?;
        let valid = match complete_set_move {
            RuntimeCompleteSetMoveV2::None => observed_input == observed_output,
            RuntimeCompleteSetMoveV2::Mint => {
                observed_input.checked_add(quantity) == Some(observed_output)
            }
            RuntimeCompleteSetMoveV2::Merge => {
                observed_output.checked_add(quantity) == Some(observed_input)
            }
        };
        if !valid {
            return Err(RuntimeVerifyErrorV2::ClaimImbalance);
        }
    }
    let available = match complete_set_move {
        RuntimeCompleteSetMoveV2::None => quote_debit,
        RuntimeCompleteSetMoveV2::Mint => quote_debit
            .checked_sub(quantity)
            .ok_or(RuntimeVerifyErrorV2::QuoteImbalance)?,
        RuntimeCompleteSetMoveV2::Merge => add(quote_debit, quantity)?,
    };
    let quote_surplus = available
        .checked_sub(quote_credit)
        .ok_or(RuntimeVerifyErrorV2::QuoteImbalance)?;
    Ok(RuntimeCandidateBalanceV2 {
        complete_set_move,
        complete_set_quantity: quantity,
        quote_surplus,
    })
}

fn validate_cursor(bytes: &[u8], header: RuntimeVerifierHeaderV2) -> RuntimeVerifyResultV2<()> {
    let initial = header.revision == 0
        && header.next_page_index == 0
        && header.next_row_index == 0
        && header.order_count == 0
        && header.filled_lots == 0
        && header.quote_debit == 0
        && header.quote_credit == 0
        && !header.has_current_order
        && tail_is_zero(bytes, header.outcome_count, CLAIM_INPUTS_TAIL)?
        && tail_is_zero(bytes, header.outcome_count, CLAIM_OUTPUTS_TAIL)?;
    if header.outcome_count == 0
        || header.page_count == 0
        || header.candidate_coordinate == 0
        || header.price_scale == 0
        || zero_identity(&header.candidate_id)
        || zero_identity(&header.product_id)
        || zero_identity(&header.batch_id)
        || header.next_page_index > header.page_count
        || (header.next_page_index == header.page_count && header.next_row_index != 0)
        || (header.revision == 0) != initial
        || (!initial && header.order_count == 0)
    {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    let mut price_total = 0_u64;
    for outcome in 0..header.outcome_count {
        price_total = add(
            price_total,
            read_tail_u64(bytes, header.outcome_count, PRICES_TAIL, outcome)?,
        )?;
    }
    if price_total != header.price_scale {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    if header.has_current_order {
        let current_lots = read_u64(bytes, 264)?;
        let max_lots = read_u64(bytes, 248)?;
        if zero_identity(&read_array32(bytes, 176)?)
            || zero_identity(&read_array32(bytes, 208)?)
            || max_lots == 0
            || current_lots == 0
            || current_lots > max_lots
        {
            return Err(RuntimeVerifyErrorV2::InvalidCursor);
        }
    } else if !zero_range(bytes, 176, 96)?
        || !tail_is_zero(bytes, header.outcome_count, CURRENT_RECEIVE_TAIL)?
        || !tail_is_zero(bytes, header.outcome_count, CURRENT_DELIVER_TAIL)?
    {
        return Err(RuntimeVerifyErrorV2::InvalidCursor);
    }
    Ok(())
}

fn tail_offset(count: u32, tail: usize) -> RuntimeVerifyResultV2<usize> {
    let count = usize::try_from(count).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    RUNTIME_VERIFIER_HEADER_BYTES_V2
        .checked_add(
            count
                .checked_mul(8)
                .and_then(|width| width.checked_mul(tail))
                .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

fn tail_bytes(bytes: &[u8], count: u32, tail: usize) -> RuntimeVerifyResultV2<&[u8]> {
    let start = tail_offset(count, tail)?;
    let width = usize::try_from(count)
        .map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    bytes
        .get(
            start
                ..start
                    .checked_add(width)
                    .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)
}

fn read_tail_u64(bytes: &[u8], count: u32, tail: usize, index: u32) -> RuntimeVerifyResultV2<u64> {
    if index >= count {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let index = usize::try_from(index).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    read_u64(
        bytes,
        tail_offset(count, tail)?
            .checked_add(
                index
                    .checked_mul(8)
                    .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
    )
}

fn write_tail_u64(
    bytes: &mut [u8],
    count: u32,
    tail: usize,
    index: u32,
    value: u64,
) -> RuntimeVerifyResultV2<()> {
    if index >= count {
        return Err(RuntimeVerifyErrorV2::InvalidLength);
    }
    let index = usize::try_from(index).map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    put_u64(
        bytes,
        tail_offset(count, tail)?
            .checked_add(
                index
                    .checked_mul(8)
                    .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?,
        value,
    )
}

fn add_tail_u64(
    bytes: &mut [u8],
    count: u32,
    tail: usize,
    index: u32,
    value: u64,
) -> RuntimeVerifyResultV2<()> {
    let successor = add(read_tail_u64(bytes, count, tail, index)?, value)?;
    write_tail_u64(bytes, count, tail, index, successor)
}

fn zero_tail(bytes: &mut [u8], count: u32, tail: usize) -> RuntimeVerifyResultV2<()> {
    let start = tail_offset(count, tail)?;
    let width = usize::try_from(count)
        .map_err(|_| RuntimeVerifyErrorV2::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    zero_mut(bytes, start, width)
}

fn tail_is_zero(bytes: &[u8], count: u32, tail: usize) -> RuntimeVerifyResultV2<bool> {
    Ok(tail_bytes(bytes, count, tail)?
        .iter()
        .all(|byte| *byte == 0))
}

fn map_codec(_: RuntimeWidthErrorV2) -> RuntimeVerifyErrorV2 {
    RuntimeVerifyErrorV2::Codec
}

fn zero_identity(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn le_numeric_id(left: &[u8; 32], right: &[u8; 32]) -> bool {
    for index in (0..32).rev() {
        if left[index] != right[index] {
            return left[index] < right[index];
        }
    }
    false
}

fn add(left: u64, right: u64) -> RuntimeVerifyResultV2<u64> {
    left.checked_add(right)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

fn multiply(left: u64, right: u64) -> RuntimeVerifyResultV2<u64> {
    left.checked_mul(right)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)
}

fn read_byte(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    let array = <[u8; 2]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)?;
    Ok(u16::from_le_bytes(array))
}

fn read_u32(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    let array = <[u8; 4]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)?;
    Ok(u32::from_le_bytes(array))
}

fn read_u64(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    let array = <[u8; 8]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)?;
    Ok(u64::from_le_bytes(array))
}

fn read_array32(bytes: &[u8], offset: usize) -> RuntimeVerifyResultV2<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?;
    <[u8; 32]>::try_from(value).map_err(|_| RuntimeVerifyErrorV2::InvalidLength)
}

fn zero_range(bytes: &[u8], offset: usize, length: usize) -> RuntimeVerifyResultV2<bool> {
    let end = offset
        .checked_add(length)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    Ok(bytes
        .get(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0))
}

fn zero_mut(bytes: &mut [u8], offset: usize, length: usize) -> RuntimeVerifyResultV2<()> {
    let end = offset
        .checked_add(length)
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    bytes
        .get_mut(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?
        .fill(0);
    Ok(())
}

fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> RuntimeVerifyResultV2<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(RuntimeVerifyErrorV2::ArithmeticOverflow)?;
    bytes
        .get_mut(offset..end)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(bytes: &mut [u8], offset: usize, value: u8) -> RuntimeVerifyResultV2<()> {
    *bytes
        .get_mut(offset)
        .ok_or(RuntimeVerifyErrorV2::InvalidLength)? = value;
    Ok(())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) -> RuntimeVerifyResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) -> RuntimeVerifyResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) -> RuntimeVerifyResultV2<()> {
    put(bytes, offset, &value.to_le_bytes())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::runtime_width::{
        CandidateHeaderV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, candidate_len,
        execution_len, page_len,
    };
    use dclutch_general_codec::{MAX_SELECTION_CRITERIA, SelectionCriterion};
    use std::vec;

    const CANDIDATE: [u8; 32] = [1; 32];
    const PRODUCT: [u8; 32] = [2; 32];
    const BATCH: [u8; 32] = [3; 32];
    const OWNER: [u8; 32] = [4; 32];

    fn order(low: u8) -> [u8; 32] {
        let mut id = [0_u8; 32];
        id[0] = low;
        id
    }

    fn candidate(width: u32, pages: u32, coordinate: u32) -> std::vec::Vec<u8> {
        let count = usize::try_from(width).expect("test width");
        let mut output = vec![0; candidate_len(width).expect("candidate width")];
        CandidateV2::encode_into(
            CandidateHeaderV2 {
                outcome_count: width,
                page_count: pages,
                candidate_coordinate: coordinate,
                price_scale: u64::from(width),
                candidate_id: CANDIDATE,
                product_id: PRODUCT,
                batch_id: BATCH,
            },
            &vec![1; count],
            &mut output,
        )
        .expect("candidate");
        output
    }

    struct RowFixture {
        bytes: std::vec::Vec<u8>,
        order: AuthenticatedOrderTermsV2,
    }

    fn row(
        width: u32,
        page_coordinate: u32,
        row_coordinate: u32,
        order_low: u8,
        lots: u64,
        vectors: (&[u64], &[u64]),
        debit_limit: u64,
    ) -> RowFixture {
        let (receive, deliver) = vectors;
        let order_id = order(order_low);
        let terms = AuthenticatedOrderTermsV2 {
            order_id,
            owner_id: OWNER,
            nonce: u64::from(order_low),
            max_lots: 10,
            max_quote_debit_per_lot: debit_limit,
        };
        let mut bytes = vec![0; execution_len(width).expect("execution width")];
        ExecutionV2::encode_into(
            ExecutionHeaderV2 {
                outcome_count: width,
                page_coordinate,
                execution_coordinate: row_coordinate,
                nonce: terms.nonce,
                order_id,
                owner_id: OWNER,
                max_lots: terms.max_lots,
                lots,
            },
            receive,
            deliver,
            &mut bytes,
        )
        .expect("row");
        RowFixture {
            bytes,
            order: terms,
        }
    }

    fn page(
        width: u32,
        coordinate: u32,
        pages: u32,
        revision: u64,
        rows: &[&[u8]],
    ) -> std::vec::Vec<u8> {
        let mut output =
            vec![0; page_len(width, u32::try_from(rows.len()).expect("rows")).expect("page width")];
        PageV2::encode_into(
            PageHeaderV2 {
                outcome_count: width,
                page_coordinate: coordinate,
                page_count: pages,
                revision,
                candidate_id: CANDIDATE,
            },
            rows,
            &mut output,
        )
        .expect("page");
        output
    }

    fn apply_row(
        candidate: &[u8],
        page: &[u8],
        before: &[u8],
        verified_before: &[u8],
        order: AuthenticatedOrderTermsV2,
        coordinate: (u32, u32, u64),
    ) -> (
        RuntimeConsiderRowSummaryV2,
        std::vec::Vec<u8>,
        std::vec::Vec<u8>,
    ) {
        let (page_index, row_index, revision) = coordinate;
        let width = CandidateV2::decode(candidate)
            .expect("candidate")
            .header()
            .outcome_count;
        let mut cursor_scratch = vec![0; runtime_verifier_len_v2(width).expect("cursor")];
        let mut cursor_output = vec![0; cursor_scratch.len()];
        let mut verified_scratch = vec![0; verified_candidate_len(width).expect("verified")];
        let mut verified_output = verified_before.to_vec();
        let summary = evaluate_runtime_consider_row_v2(
            RuntimeConsiderRowViewV2 {
                candidate,
                page,
                cursor_before: before,
                verified_before,
                authenticated_order: order,
                expected_page_index: page_index,
                expected_row_index: row_index,
                expected_page_revision: 11 + u64::from(page_index),
                expected_revision: revision,
                max_orders: 10,
            },
            RuntimeConsiderRowBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
            },
        )
        .expect("row accepts");
        (summary, cursor_output, verified_output)
    }

    #[test]
    fn runtime_width_sixteen_streams_across_pages_without_page_balance() {
        let width = 16;
        let candidate = candidate(width, 2, 1);
        let receive_a = vec![1; 16];
        let deliver_zero = vec![0; 16];
        let first = row(width, 1, 1, 1, 2, (&receive_a, &deliver_zero), 2);
        let second = row(width, 2, 1, 1, 3, (&receive_a, &deliver_zero), 2);
        let first_page = page(width, 1, 2, 11, &[&first.bytes]);
        let second_page = page(width, 2, 2, 12, &[&second.bytes]);
        let cursor_len = runtime_verifier_len_v2(width).expect("cursor");
        let verified_len = verified_candidate_len(width).expect("verified");
        let zero_cursor = vec![0; cursor_len];
        let zero_verified = vec![0; verified_len];

        let (summary, middle, unchanged_verified) = apply_row(
            &candidate,
            &first_page,
            &zero_cursor,
            &zero_verified,
            first.order,
            (0, 0, 0),
        );
        assert_eq!(
            summary,
            RuntimeConsiderRowSummaryV2 {
                complete: false,
                order_count: 1,
                revision: 1,
            }
        );
        assert_eq!(unchanged_verified, zero_verified);

        let (summary, terminal, verified) = apply_row(
            &candidate,
            &second_page,
            &middle,
            &zero_verified,
            second.order,
            (1, 0, 1),
        );
        assert!(summary.complete);
        assert_eq!(summary.revision, 2);
        let cursor = RuntimeCandidateVerifierV2::decode(&terminal).expect("terminal cursor");
        assert!(cursor.is_complete());
        assert!(!cursor.header().has_current_order);
        let certificate = VerifiedCandidateV2::decode(&verified).expect("certificate");
        assert_eq!(certificate.header().filled_lots, 5);
        assert_eq!(certificate.header().quote_debit, 5);
        assert_eq!(certificate.claim_output(0).expect("output"), 5);
        assert_eq!(certificate.claim_output(15).expect("output"), 5);
        assert_eq!(
            runtime_verified_balance_v2(&verified).expect("balance"),
            RuntimeCandidateBalanceV2 {
                complete_set_move: RuntimeCompleteSetMoveV2::Mint,
                complete_set_quantity: 5,
                quote_surplus: 0,
            }
        );
    }

    #[test]
    fn runtime_width_two_fifty_eight_uses_scratch_without_semantic_cap() {
        let width = 258;
        let candidate = candidate(width, 1, 9);
        let receive = vec![1; 258];
        let deliver = vec![1; 258];
        let row = row(width, 1, 1, 1, 1, (&receive, &deliver), 0);
        let page = page(width, 1, 1, 11, &[&row.bytes]);
        let zero_cursor = vec![0; runtime_verifier_len_v2(width).expect("cursor")];
        let zero_verified = vec![0; verified_candidate_len(width).expect("verified")];
        let (summary, cursor, verified) = apply_row(
            &candidate,
            &page,
            &zero_cursor,
            &zero_verified,
            row.order,
            (0, 0, 0),
        );
        assert!(summary.complete);
        assert_eq!(cursor.len(), RUNTIME_VERIFIER_HEADER_BYTES_V2 + 40 * 258);
        let certificate = VerifiedCandidateV2::decode(&verified).expect("verified");
        assert_eq!(certificate.claim_input(257).expect("tail"), 1);
        assert_eq!(certificate.claim_output(257).expect("tail"), 1);
    }

    #[test]
    fn hostile_order_substitution_limit_and_skip_preserve_candidates() {
        let width = 2;
        let candidate = candidate(width, 1, 1);
        let first = row(width, 1, 1, 1, 1, (&[1, 0], &[0, 0]), 0);
        let page = page(width, 1, 1, 11, &[&first.bytes]);
        let cursor_len = runtime_verifier_len_v2(width).expect("cursor");
        let verified_len = verified_candidate_len(width).expect("verified");
        let zero_cursor = vec![0; cursor_len];
        let zero_verified = vec![0; verified_len];
        let mut cursor_scratch = vec![0; cursor_len];
        let mut cursor_output = vec![0x55; cursor_len];
        let mut verified_scratch = vec![0; verified_len];
        let mut verified_output = vec![0xaa; verified_len];
        let result = evaluate_runtime_consider_row_v2(
            RuntimeConsiderRowViewV2 {
                candidate: &candidate,
                page: &page,
                cursor_before: &zero_cursor,
                verified_before: &zero_verified,
                authenticated_order: first.order,
                expected_page_index: 0,
                expected_row_index: 0,
                expected_page_revision: 11,
                expected_revision: 0,
                max_orders: 10,
            },
            RuntimeConsiderRowBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
            },
        );
        assert_eq!(result, Err(RuntimeVerifyErrorV2::QuoteLimit));
        assert_eq!(cursor_output, vec![0x55; cursor_len]);
        assert_eq!(verified_output, vec![0xaa; verified_len]);

        let mut substituted = first.order;
        substituted.owner_id = [9; 32];
        let result = evaluate_runtime_consider_row_v2(
            RuntimeConsiderRowViewV2 {
                candidate: &candidate,
                page: &page,
                cursor_before: &zero_cursor,
                verified_before: &zero_verified,
                authenticated_order: substituted,
                expected_page_index: 0,
                expected_row_index: 0,
                expected_page_revision: 11,
                expected_revision: 0,
                max_orders: 10,
            },
            RuntimeConsiderRowBuffersV2 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
            },
        );
        assert_eq!(
            result,
            Err(RuntimeVerifyErrorV2::AuthenticatedOrderMismatch)
        );
        assert_eq!(cursor_output, vec![0x55; cursor_len]);
        assert_eq!(verified_output, vec![0xaa; verified_len]);
    }

    #[test]
    fn best_valid_submitted_candidate_uses_exact_policy() {
        let width = 2;
        let left_candidate = candidate(width, 1, 1);
        let mut right_candidate = candidate(width, 1, 2);
        right_candidate[32] = 7;
        let receive = [1, 1];
        let deliver = [1, 1];
        let row = row(width, 1, 1, 1, 1, (&receive, &deliver), 0);
        let left_page = page(width, 1, 1, 11, &[&row.bytes]);
        let mut right_page = left_page.clone();
        right_page[32] = 7;
        let zero_cursor = vec![0; runtime_verifier_len_v2(width).expect("cursor")];
        let zero_verified = vec![0; verified_candidate_len(width).expect("verified")];
        let (_, _, left) = apply_row(
            &left_candidate,
            &left_page,
            &zero_cursor,
            &zero_verified,
            row.order,
            (0, 0, 0),
        );
        let (_, _, right) = apply_row(
            &right_candidate,
            &right_page,
            &zero_cursor,
            &zero_verified,
            row.order,
            (0, 0, 0),
        );
        let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
        criteria[0] = SelectionCriterion::MinimizeCandidateId;
        let policy = SelectionPolicyV1 {
            policy_id: [8; 32],
            criterion_count: 1,
            criteria,
        };
        assert_eq!(
            runtime_candidate_better_v2(&policy, &left, &right),
            Ok(true)
        );
    }
}
