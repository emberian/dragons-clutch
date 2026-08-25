#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::indexing_slicing, clippy::needless_range_loop)]

//! Stack-bounded physical refinement of Lean-owned General clearing.
//!
//! Candidate verification consumes one borrowed page at a time and maintains
//! one current order accumulator. Physical rows must be globally ordered by
//! order identity, allowing exact candidate-wide quote rounding without an
//! allocation or a width-specialized verifier. Settlement consumes the same
//! immutable pages through collect, one complete-set operation, distribute,
//! and close. External Claims and Custody mutation is represented by exact
//! fixed-layout CPI plans and remains an adapter/runtime boundary.

#[cfg(test)]
extern crate std;

use dclutch_general_codec::{
    CandidateV1, ExecutionV1, MAX_EXECUTIONS_PER_PAGE, MAX_OUTCOMES, MAX_PAGES_PER_CANDIDATE,
    PageViewV1, Phase, SelectionCriterion, SelectionCursorV1, SelectionPolicyV1,
    SettlementCursorV1,
};

/// Exact verified-candidate certificate width.
pub const VERIFIED_CANDIDATE_BYTES_V1: usize = 416;
/// Exact persisted candidate-verification cursor width.
pub const VERIFICATION_CURSOR_BYTES_V1: usize = 960;
/// Release-pinned Trading authority PDA domain.
pub const GENERAL_AUTHORITY_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-authority:v1";
/// Market/batch selection cursor PDA domain.
pub const GENERAL_SELECTION_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-selection:v1";
/// Market/candidate verification cursor PDA domain.
pub const GENERAL_VERIFICATION_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-verification:v1";
/// Market/candidate verified-certificate PDA domain.
pub const GENERAL_CERTIFICATE_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-certificate:v1";
/// Market/candidate settlement cursor PDA domain.
pub const GENERAL_SETTLEMENT_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-settlement:v1";
/// Immutable candidate header PDA domain.
pub const GENERAL_CANDIDATE_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-candidate:v1";
/// Immutable selection policy PDA domain.
pub const GENERAL_POLICY_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-policy:v1";
/// Immutable candidate page PDA domain.
pub const GENERAL_PAGE_PDA_DOMAIN_V1: &[u8] = b"dclutch:general-page:v1";

const CERTIFICATE_MAGIC: [u8; 8] = *b"DCGVCER1";
const VERIFICATION_CURSOR_MAGIC: [u8; 8] = *b"DCGVERF1";
const VERSION: u16 = 1;

/// Stable refusal from bounded verification or settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A generated candidate, page, policy, or cursor wire refused.
    Codec,
    /// A page coordinate or candidate binding did not match the header.
    CandidateBinding,
    /// Rows were not globally grouped in strictly increasing identity order.
    NonCanonicalOrder,
    /// One order identity was paired with two different immutable preimages.
    OrderSubstitution,
    /// Checked physical scalar arithmetic overflowed.
    ArithmeticOverflow,
    /// Candidate-wide lots exceeded the signed order maximum.
    ExcessLots,
    /// Candidate-wide quote portions did not equal the one rounded value.
    QuoteMismatch,
    /// Candidate-wide debit exceeded the signed exact limit.
    QuoteLimit,
    /// Claim inputs/outputs were not a uniform complete-set difference.
    ClaimImbalance,
    /// Quote input could not fund materialization and certified outputs.
    QuoteImbalance,
    /// Candidate verification had not consumed every declared page.
    VerificationIncomplete,
    /// Fixed certificate bytes were hostile or noncanonical.
    Certificate,
    /// Selection was closed, empty, stale, or bound to another policy/batch.
    Selection,
    /// Optimistic concurrency coordinate differed.
    RevisionMismatch,
    /// Settlement phase or page cursor refused the action.
    SettlementPhase,
    /// Settlement inventory could not fund the exact outgoing plan.
    Inventory,
    /// Claims or Custody child execution refused the exact plan.
    ChildRefusal,
}

/// Result alias for General adapter operations.
pub type Result<T> = core::result::Result<T, Error>;

/// The sole complete-set liability movement certified for a candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompleteSetMoveV1 {
    /// Claim inputs and outputs are exactly equal.
    None = 0,
    /// Mint one uniform quantity of every outcome.
    Mint = 1,
    /// Merge one uniform quantity of every outcome.
    Merge = 2,
}

/// Candidate-wide, program-derived verification certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCandidateV1 {
    /// Candidate content identity.
    pub candidate_id: [u8; 32],
    /// Product content identity.
    pub product_id: [u8; 32],
    /// Batch content identity.
    pub batch_id: [u8; 32],
    /// Active outcome width.
    pub outcome_count: u8,
    /// Authenticated page count.
    pub page_count: u32,
    /// Candidate-wide filled lots objective.
    pub filled_lots: u64,
    /// Exact quote surplus after the complete-set move and outputs.
    pub quote_surplus: u64,
    /// Exact aggregate quote input.
    pub quote_inputs: u64,
    /// Exact aggregate quote output.
    pub quote_outputs: u64,
    /// Sole complete-set movement direction.
    pub complete_set_move: CompleteSetMoveV1,
    /// Uniform complete-set quantity; zero only for no movement.
    pub complete_set_quantity: u64,
    /// Exact aggregate incoming claims.
    pub claim_inputs: [u64; MAX_OUTCOMES],
    /// Exact aggregate outgoing claims.
    pub claim_outputs: [u64; MAX_OUTCOMES],
}

impl VerifiedCandidateV1 {
    /// Decode one exact program-derived certificate.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != VERIFIED_CANDIDATE_BYTES_V1
            || input.get(..8) != Some(CERTIFICATE_MAGIC.as_slice())
            || read_u16(input, 8)? != VERSION
            || !zero(input, 12, 4)?
        {
            return Err(Error::Certificate);
        }
        let complete_set_move = match read_byte(input, 10)? {
            0 => CompleteSetMoveV1::None,
            1 => CompleteSetMoveV1::Mint,
            2 => CompleteSetMoveV1::Merge,
            _ => return Err(Error::Certificate),
        };
        let value = Self {
            candidate_id: read_array(input, 16)?,
            product_id: read_array(input, 48)?,
            batch_id: read_array(input, 80)?,
            outcome_count: read_byte(input, 11)?,
            page_count: read_u32(input, 112)?,
            filled_lots: read_u64(input, 120)?,
            quote_surplus: read_u64(input, 128)?,
            quote_inputs: read_u64(input, 136)?,
            quote_outputs: read_u64(input, 144)?,
            complete_set_quantity: read_u64(input, 152)?,
            claim_inputs: read_u64_array(input, 160)?,
            claim_outputs: read_u64_array(input, 288)?,
            complete_set_move,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact program-derived certificate.
    pub fn to_bytes(self) -> Result<[u8; VERIFIED_CANDIDATE_BYTES_V1]> {
        self.validate()?;
        let mut output = [0_u8; VERIFIED_CANDIDATE_BYTES_V1];
        put(&mut output, 0, &CERTIFICATE_MAGIC)?;
        put(&mut output, 8, &VERSION.to_le_bytes())?;
        put_byte(&mut output, 10, self.complete_set_move as u8)?;
        put_byte(&mut output, 11, self.outcome_count)?;
        put(&mut output, 16, &self.candidate_id)?;
        put(&mut output, 48, &self.product_id)?;
        put(&mut output, 80, &self.batch_id)?;
        put(&mut output, 112, &self.page_count.to_le_bytes())?;
        put(&mut output, 120, &self.filled_lots.to_le_bytes())?;
        put(&mut output, 128, &self.quote_surplus.to_le_bytes())?;
        put(&mut output, 136, &self.quote_inputs.to_le_bytes())?;
        put(&mut output, 144, &self.quote_outputs.to_le_bytes())?;
        put(&mut output, 152, &self.complete_set_quantity.to_le_bytes())?;
        put_u64_array(&mut output, 160, &self.claim_inputs)?;
        put_u64_array(&mut output, 288, &self.claim_outputs)?;
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        let count = usize::from(self.outcome_count);
        if count == 0
            || count > MAX_OUTCOMES
            || self.page_count == 0
            || is_zero(&self.candidate_id)
            || is_zero(&self.product_id)
            || is_zero(&self.batch_id)
            || self.filled_lots == 0
            || self.claim_inputs[count..].iter().any(|value| *value != 0)
            || self.claim_outputs[count..].iter().any(|value| *value != 0)
        {
            return Err(Error::Certificate);
        }
        if (self.complete_set_move == CompleteSetMoveV1::None) != (self.complete_set_quantity == 0)
        {
            return Err(Error::Certificate);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OrderAccumulator {
    terms: ExecutionV1,
    lots: u64,
    quote_debit: u64,
    quote_credit: u64,
}

/// Stack-bounded verifier consuming globally ordered execution rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateVerifierV1 {
    candidate: CandidateV1,
    next_page: u32,
    revision: u64,
    current_order: Option<OrderAccumulator>,
    filled_lots: u64,
    quote_inputs: u64,
    quote_outputs: u64,
    claim_inputs: [u64; MAX_OUTCOMES],
    claim_outputs: [u64; MAX_OUTCOMES],
}

impl CandidateVerifierV1 {
    /// Begin verification from one hostile-decoded candidate header.
    #[must_use]
    pub const fn begin(candidate: CandidateV1) -> Self {
        Self {
            candidate,
            next_page: 0,
            revision: 0,
            current_order: None,
            filled_lots: 0,
            quote_inputs: 0,
            quote_outputs: 0,
            claim_inputs: [0; MAX_OUTCOMES],
            claim_outputs: [0; MAX_OUTCOMES],
        }
    }

    /// Consume the exact next page. Every refusal leaves this verifier unchanged.
    #[inline(never)]
    pub fn ingest_page(&mut self, bytes: &[u8]) -> Result<()> {
        let mut staged = *self;
        staged.ingest_page_inner(bytes)?;
        staged.revision = staged
            .revision
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        *self = staged;
        Ok(())
    }

    /// Consume one page only at the exact optimistic-concurrency revision.
    #[inline(never)]
    pub fn ingest_page_at(&mut self, bytes: &[u8], expected_revision: u64) -> Result<()> {
        if self.revision != expected_revision {
            return Err(Error::RevisionMismatch);
        }
        self.ingest_page(bytes)
    }

    /// Return the immutable candidate header bound to this cursor.
    #[must_use]
    pub const fn candidate(&self) -> CandidateV1 {
        self.candidate
    }

    /// Return the next page coordinate.
    #[must_use]
    pub const fn next_page(&self) -> u32 {
        self.next_page
    }

    /// Return the optimistic-concurrency revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Return whether every declared page has been consumed.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.next_page == self.candidate.page_count
    }

    /// Encode one exact persisted verification cursor.
    #[inline(never)]
    pub fn to_bytes(self) -> Result<[u8; VERIFICATION_CURSOR_BYTES_V1]> {
        self.validate_cursor()?;
        let mut output = [0_u8; VERIFICATION_CURSOR_BYTES_V1];
        infallible_put(&mut output, 0, &VERIFICATION_CURSOR_MAGIC);
        infallible_put(&mut output, 8, &VERSION.to_le_bytes());
        output[10] = u8::from(self.current_order.is_some());
        infallible_put(
            &mut output,
            16,
            &self.candidate.to_bytes().map_err(|_| Error::Codec)?,
        );
        infallible_put(&mut output, 272, &self.next_page.to_le_bytes());
        if let Some(current) = self.current_order {
            infallible_put(
                &mut output,
                280,
                &current
                    .terms
                    .to_bytes_for_outcomes(self.candidate.outcome_count)
                    .map_err(|_| Error::Codec)?,
            );
            infallible_put(&mut output, 648, &current.lots.to_le_bytes());
            infallible_put(&mut output, 656, &current.quote_debit.to_le_bytes());
            infallible_put(&mut output, 664, &current.quote_credit.to_le_bytes());
        }
        infallible_put(&mut output, 672, &self.filled_lots.to_le_bytes());
        infallible_put(&mut output, 680, &self.quote_inputs.to_le_bytes());
        infallible_put(&mut output, 688, &self.quote_outputs.to_le_bytes());
        infallible_put_u64_array(&mut output, 696, &self.claim_inputs);
        infallible_put_u64_array(&mut output, 824, &self.claim_outputs);
        infallible_put(&mut output, 952, &self.revision.to_le_bytes());
        Ok(output)
    }

    /// Hostile-decode one exact persisted verification cursor.
    #[inline(never)]
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != VERIFICATION_CURSOR_BYTES_V1
            || input.get(..8) != Some(VERIFICATION_CURSOR_MAGIC.as_slice())
            || read_u16(input, 8)? != VERSION
            || !zero(input, 11, 5)?
            || !zero(input, 276, 4)?
        {
            return Err(Error::Certificate);
        }
        let candidate =
            CandidateV1::decode(read_slice(input, 16, 256)?).map_err(|_| Error::Codec)?;
        let has_order = match read_byte(input, 10)? {
            0 => false,
            1 => true,
            _ => return Err(Error::Certificate),
        };
        let current_order = if has_order {
            Some(OrderAccumulator {
                terms: ExecutionV1::decode_for_outcomes(
                    read_slice(input, 280, 368)?,
                    candidate.outcome_count,
                )
                .map_err(|_| Error::Codec)?,
                lots: read_u64(input, 648)?,
                quote_debit: read_u64(input, 656)?,
                quote_credit: read_u64(input, 664)?,
            })
        } else {
            if !zero(input, 280, 392)? {
                return Err(Error::Certificate);
            }
            None
        };
        let value = Self {
            candidate,
            next_page: read_u32(input, 272)?,
            revision: read_u64(input, 952)?,
            current_order,
            filled_lots: read_u64(input, 672)?,
            quote_inputs: read_u64(input, 680)?,
            quote_outputs: read_u64(input, 688)?,
            claim_inputs: read_u64_array(input, 696)?,
            claim_outputs: read_u64_array(input, 824)?,
        };
        value.validate_cursor()?;
        Ok(value)
    }

    /// Finalize the candidate-wide per-order rounding and balance checks.
    #[inline(never)]
    pub fn finish(mut self) -> Result<VerifiedCandidateV1> {
        if self.next_page != self.candidate.page_count {
            return Err(Error::VerificationIncomplete);
        }
        self.finalize_current_order()?;
        let (complete_set_move, complete_set_quantity, available_quote) =
            complete_set_balance(&self)?;
        if self.quote_outputs > available_quote {
            return Err(Error::QuoteImbalance);
        }
        let quote_surplus = available_quote
            .checked_sub(self.quote_outputs)
            .ok_or(Error::ArithmeticOverflow)?;
        let certificate = VerifiedCandidateV1 {
            candidate_id: self.candidate.candidate_id,
            product_id: self.candidate.product_id,
            batch_id: self.candidate.batch_id,
            outcome_count: self.candidate.outcome_count,
            page_count: self.candidate.page_count,
            filled_lots: self.filled_lots,
            quote_surplus,
            quote_inputs: self.quote_inputs,
            quote_outputs: self.quote_outputs,
            complete_set_move,
            complete_set_quantity,
            claim_inputs: self.claim_inputs,
            claim_outputs: self.claim_outputs,
        };
        certificate.validate()?;
        Ok(certificate)
    }

    fn ingest_page_inner(&mut self, bytes: &[u8]) -> Result<()> {
        let page = PageViewV1::decode(bytes).map_err(|_| Error::Codec)?;
        if page.candidate_id() != self.candidate.candidate_id
            || page.outcome_count() != self.candidate.outcome_count
            || page.page_count() != self.candidate.page_count
            || page.page_index() != self.next_page
        {
            return Err(Error::CandidateBinding);
        }
        for index in 0..usize::from(page.execution_count()) {
            let execution = page.execution(index).map_err(|_| Error::Codec)?;
            self.ingest_execution(execution)?;
        }
        self.next_page = self
            .next_page
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    fn validate_cursor(&self) -> Result<()> {
        let count = usize::from(self.candidate.outcome_count);
        self.candidate.to_bytes().map_err(|_| Error::Codec)?;
        if self.next_page > self.candidate.page_count
            || self.revision != u64::from(self.next_page)
            || self.claim_inputs[count..].iter().any(|value| *value != 0)
            || self.claim_outputs[count..].iter().any(|value| *value != 0)
            || (self.next_page == 0)
                != (self.current_order.is_none()
                    && self.filled_lots == 0
                    && self.quote_inputs == 0
                    && self.quote_outputs == 0)
        {
            return Err(Error::Certificate);
        }
        if let Some(current) = self.current_order
            && (current.lots == 0 || current.lots > current.terms.max_lots)
        {
            return Err(Error::Certificate);
        }
        Ok(())
    }

    fn ingest_execution(&mut self, execution: ExecutionV1) -> Result<()> {
        match self.current_order {
            None => self.current_order = Some(new_accumulator(execution)),
            Some(current) if current.terms.order_id == execution.order_id => {
                if !same_order_terms(&current.terms, &execution) {
                    return Err(Error::OrderSubstitution);
                }
            }
            Some(current) => {
                if current.terms.order_id >= execution.order_id {
                    return Err(Error::NonCanonicalOrder);
                }
                self.finalize_current_order()?;
                self.current_order = Some(new_accumulator(execution));
            }
        }
        let accumulator = self.current_order.as_mut().ok_or(Error::CandidateBinding)?;
        accumulator.lots = add(accumulator.lots, execution.lots)?;
        accumulator.quote_debit = add(accumulator.quote_debit, execution.quote_debit)?;
        accumulator.quote_credit = add(accumulator.quote_credit, execution.quote_credit)?;
        self.filled_lots = add(self.filled_lots, execution.lots)?;
        self.quote_inputs = add(self.quote_inputs, execution.quote_debit)?;
        self.quote_outputs = add(self.quote_outputs, execution.quote_credit)?;
        let count = usize::from(self.candidate.outcome_count);
        for outcome in 0..count {
            self.claim_inputs[outcome] = add(
                self.claim_inputs[outcome],
                multiply(execution.deliver_per_lot[outcome], execution.lots)?,
            )?;
            self.claim_outputs[outcome] = add(
                self.claim_outputs[outcome],
                multiply(execution.receive_per_lot[outcome], execution.lots)?,
            )?;
        }
        Ok(())
    }

    fn finalize_current_order(&mut self) -> Result<()> {
        let Some(order) = self.current_order.take() else {
            return Ok(());
        };
        if order.lots > order.terms.max_lots {
            return Err(Error::ExcessLots);
        }
        let received = weighted_value(
            &self.candidate.prices,
            &order.terms.receive_per_lot,
            order.lots,
            self.candidate.outcome_count,
        )?;
        let delivered = weighted_value(
            &self.candidate.prices,
            &order.terms.deliver_per_lot,
            order.lots,
            self.candidate.outcome_count,
        )?;
        let (debit, credit) = if delivered <= received {
            let difference = received - delivered;
            let numerator = add(difference, self.candidate.price_scale - 1)?;
            (numerator / self.candidate.price_scale, 0)
        } else {
            (0, (delivered - received) / self.candidate.price_scale)
        };
        if (debit, credit) != (order.quote_debit, order.quote_credit) {
            return Err(Error::QuoteMismatch);
        }
        let limit = multiply(order.terms.max_quote_debit_per_lot, order.lots)?;
        if debit > limit {
            return Err(Error::QuoteLimit);
        }
        Ok(())
    }
}

fn new_accumulator(execution: ExecutionV1) -> OrderAccumulator {
    OrderAccumulator {
        terms: execution,
        lots: 0,
        quote_debit: 0,
        quote_credit: 0,
    }
}

fn same_order_terms(left: &ExecutionV1, right: &ExecutionV1) -> bool {
    left.order_id == right.order_id
        && left.owner_id == right.owner_id
        && left.nonce == right.nonce
        && left.max_lots == right.max_lots
        && left.max_quote_debit_per_lot == right.max_quote_debit_per_lot
        && left.receive_per_lot == right.receive_per_lot
        && left.deliver_per_lot == right.deliver_per_lot
}

fn weighted_value(
    prices: &[u64; MAX_OUTCOMES],
    quantities: &[u64; MAX_OUTCOMES],
    lots: u64,
    outcome_count: u8,
) -> Result<u64> {
    let mut per_lot = 0_u64;
    for outcome in 0..usize::from(outcome_count) {
        per_lot = add(per_lot, multiply(prices[outcome], quantities[outcome])?)?;
    }
    multiply(per_lot, lots)
}

fn complete_set_balance(verifier: &CandidateVerifierV1) -> Result<(CompleteSetMoveV1, u64, u64)> {
    let count = usize::from(verifier.candidate.outcome_count);
    let first_input = verifier.claim_inputs[0];
    let first_output = verifier.claim_outputs[0];
    if first_input == first_output {
        for outcome in 0..count {
            if verifier.claim_inputs[outcome] != verifier.claim_outputs[outcome] {
                return Err(Error::ClaimImbalance);
            }
        }
        return Ok((CompleteSetMoveV1::None, 0, verifier.quote_inputs));
    }
    if first_input < first_output {
        let quantity = first_output - first_input;
        for outcome in 0..count {
            if add(verifier.claim_inputs[outcome], quantity)? != verifier.claim_outputs[outcome] {
                return Err(Error::ClaimImbalance);
            }
        }
        let available = verifier
            .quote_inputs
            .checked_sub(quantity)
            .ok_or(Error::QuoteImbalance)?;
        Ok((CompleteSetMoveV1::Mint, quantity, available))
    } else {
        let quantity = first_input - first_output;
        for outcome in 0..count {
            if add(verifier.claim_outputs[outcome], quantity)? != verifier.claim_inputs[outcome] {
                return Err(Error::ClaimImbalance);
            }
        }
        Ok((
            CompleteSetMoveV1::Merge,
            quantity,
            add(verifier.quote_inputs, quantity)?,
        ))
    }
}

/// Return whether `left` is preferred under immutable interpreted policy data.
#[must_use]
pub fn candidate_better(
    policy: &SelectionPolicyV1,
    left: &VerifiedCandidateV1,
    right: &VerifiedCandidateV1,
) -> bool {
    for criterion in policy
        .criteria
        .iter()
        .take(usize::from(policy.criterion_count))
    {
        match criterion {
            SelectionCriterion::MaximizeFilledLots if left.filled_lots != right.filled_lots => {
                return left.filled_lots > right.filled_lots;
            }
            SelectionCriterion::MinimizeQuoteSurplus
                if left.quote_surplus != right.quote_surplus =>
            {
                return left.quote_surplus < right.quote_surplus;
            }
            SelectionCriterion::MinimizeCandidateId if left.candidate_id != right.candidate_id => {
                return le_numeric_id(&left.candidate_id, &right.candidate_id);
            }
            _ => {}
        }
    }
    false
}

/// Admit one verified candidate and atomically update selection/certificate bytes.
pub fn consider_verified(
    selection_output: &mut [u8],
    certificate_output: &mut [u8],
    candidate: &CandidateV1,
    policy: &SelectionPolicyV1,
    verified: VerifiedCandidateV1,
    incumbent: Option<&VerifiedCandidateV1>,
    expected_revision: u64,
) -> Result<()> {
    if verified.candidate_id != candidate.candidate_id
        || verified.product_id != candidate.product_id
        || verified.batch_id != candidate.batch_id
        || verified.outcome_count != candidate.outcome_count
        || verified.page_count != candidate.page_count
    {
        return Err(Error::CandidateBinding);
    }
    let mut selection = if selection_output.iter().all(|byte| *byte == 0) {
        if expected_revision != 0 {
            return Err(Error::RevisionMismatch);
        }
        SelectionCursorV1 {
            closed: false,
            batch_id: candidate.batch_id,
            policy_id: policy.policy_id,
            best_candidate_id: None,
            revision: 0,
        }
    } else {
        SelectionCursorV1::decode(selection_output).map_err(|_| Error::Selection)?
    };
    if selection.closed
        || selection.batch_id != candidate.batch_id
        || selection.policy_id != policy.policy_id
    {
        return Err(Error::Selection);
    }
    if selection.revision != expected_revision {
        return Err(Error::RevisionMismatch);
    }
    let replace = match selection.best_candidate_id {
        None => incumbent.is_none(),
        Some(best) => match incumbent {
            Some(certificate) if certificate.candidate_id == best => {
                candidate_better(policy, &verified, certificate)
            }
            _ => return Err(Error::Selection),
        },
    };
    let next_revision = selection
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let certificate_bytes = verified.to_bytes()?;
    if certificate_output.len() != VERIFIED_CANDIDATE_BYTES_V1
        || certificate_output.iter().any(|byte| *byte != 0)
    {
        return Err(Error::Certificate);
    }
    if replace {
        selection.best_candidate_id = Some(verified.candidate_id);
    }
    selection.revision = next_revision;
    let selection_bytes = selection.to_bytes().map_err(|_| Error::Selection)?;
    certificate_output.copy_from_slice(&certificate_bytes);
    selection_output.copy_from_slice(&selection_bytes);
    Ok(())
}

/// Freeze a nonempty selection at one exact optimistic revision.
pub fn freeze_selection(selection_output: &mut [u8], expected_revision: u64) -> Result<()> {
    let mut selection =
        SelectionCursorV1::decode(selection_output).map_err(|_| Error::Selection)?;
    if selection.closed || selection.best_candidate_id.is_none() {
        return Err(Error::Selection);
    }
    if selection.revision != expected_revision {
        return Err(Error::RevisionMismatch);
    }
    selection.closed = true;
    selection.revision = selection
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let bytes = selection.to_bytes().map_err(|_| Error::Selection)?;
    selection_output.copy_from_slice(&bytes);
    Ok(())
}

/// Initialize an empty settlement cursor for the frozen best candidate.
pub fn initialize_settlement(
    output: &mut [u8],
    selection_bytes: &[u8],
    verified: &VerifiedCandidateV1,
    expected_revision: u64,
) -> Result<()> {
    if output.iter().any(|byte| *byte != 0) {
        return Err(Error::SettlementPhase);
    }
    let selection = SelectionCursorV1::decode(selection_bytes).map_err(|_| Error::Selection)?;
    if !selection.closed
        || selection.best_candidate_id != Some(verified.candidate_id)
        || expected_revision != 0
    {
        return Err(Error::Selection);
    }
    let cursor = SettlementCursorV1 {
        phase: Phase::Collecting,
        outcome_count: verified.outcome_count,
        candidate_id: verified.candidate_id,
        page_count: verified.page_count,
        next_page: 0,
        next_execution: 0,
        revision: 0,
        claim_inventory: [0; MAX_OUTCOMES],
        quote_inventory: 0,
        quote_surplus_paid: 0,
    };
    output.copy_from_slice(&cursor.to_bytes().map_err(|_| Error::SettlementPhase)?);
    Ok(())
}

/// Authenticated Market/release coordinates required by every canonical child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionContextV1 {
    /// Canonical Market account identity.
    pub market_id: [u8; 32],
    /// Registry-authenticated execution-release-set identity.
    pub release_set_id: [u8; 32],
}

impl ExecutionContextV1 {
    fn validate(self) -> Result<()> {
        if is_zero(&self.market_id) || is_zero(&self.release_set_id) {
            Err(Error::CandidateBinding)
        } else {
            Ok(())
        }
    }
}

/// Replay coordinates required for one candidate execution row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowReplayContextV1 {
    /// Authenticated Market and release-set coordinates.
    pub execution: ExecutionContextV1,
    /// Verified candidate identity.
    pub candidate_id: [u8; 32],
    /// Signed order owner identity.
    pub owner_id: [u8; 32],
    /// Signed order identity.
    pub order_id: [u8; 32],
    /// Settlement revision consumed by this child operation.
    pub revision: u64,
    /// Signed order nonce.
    pub order_nonce: u64,
    /// Candidate page coordinate.
    pub page_index: u32,
    /// Execution-row coordinate.
    pub execution_index: u8,
}

impl RowReplayContextV1 {
    fn validate(self) -> Result<()> {
        self.execution.validate()?;
        if is_zero(&self.candidate_id)
            || is_zero(&self.owner_id)
            || is_zero(&self.order_id)
            || self.page_index >= MAX_PAGES_PER_CANDIDATE
            || usize::from(self.execution_index) >= MAX_EXECUTIONS_PER_PAGE
        {
            return Err(Error::CandidateBinding);
        }
        Ok(())
    }
}

/// Replay coordinates required for a candidate-wide settlement operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateReplayContextV1 {
    /// Authenticated Market and release-set coordinates.
    pub execution: ExecutionContextV1,
    /// Verified candidate identity.
    pub candidate_id: [u8; 32],
    /// Settlement revision consumed by this child operation.
    pub revision: u64,
}

/// Opaque refusal from a canonical Claims or Custody child implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildExecutionError {
    /// The canonical child refused before the General cursor could commit.
    Refused,
}

impl AggregateReplayContextV1 {
    fn validate(self) -> Result<()> {
        self.execution.validate()?;
        if is_zero(&self.candidate_id) {
            Err(Error::CandidateBinding)
        } else {
            Ok(())
        }
    }
}

/// Canonical Claims/Custody integration requirements without defining a child wire.
///
/// Separate Claims and Custody role crates own serialization, account mutation,
/// replay state, Registry reauthentication, and return-data receipts. A Solana
/// adapter implements this trait by invoking those canonical children.
pub trait SettlementChildrenV1 {
    /// Move one row owner's delivered claims into settlement inventory.
    fn collect_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError>;
    /// Collect one row's quote debit into settlement custody.
    fn collect_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError>;
    /// Mint one complete set and fund its Hoard principal.
    fn mint_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError>;
    /// Merge one complete set and receive its released Hoard principal.
    fn merge_complete_set(
        &mut self,
        context: AggregateReplayContextV1,
        outcome_count: u8,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError>;
    /// Move settlement claims to one row owner.
    fn distribute_claims(
        &mut self,
        context: RowReplayContextV1,
        outcome_count: u8,
        quantities: &[u64; MAX_OUTCOMES],
    ) -> core::result::Result<(), ChildExecutionError>;
    /// Pay one row's quote credit from settlement custody.
    fn distribute_collateral(
        &mut self,
        context: RowReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError>;
    /// Route the exact terminal quote surplus.
    fn pay_surplus(
        &mut self,
        context: AggregateReplayContextV1,
        quantity: u64,
    ) -> core::result::Result<(), ChildExecutionError>;
}

/// Collect one exact next execution row, staging cursor bytes until every child accepts.
pub fn collect_execution<C: SettlementChildrenV1>(
    output: &mut [u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    page_bytes: &[u8],
    expected_revision: u64,
    children: &mut C,
) -> Result<()> {
    context.validate()?;
    let mut cursor = settlement_pre(output, verified, expected_revision, Phase::Collecting)?;
    let page = settlement_page(verified, page_bytes, cursor.next_page)?;
    let count = usize::from(verified.outcome_count);
    let execution_index = cursor.next_execution;
    let execution = page
        .execution(usize::from(execution_index))
        .map_err(|_| Error::Codec)?;
    let quantities = scaled(&execution.deliver_per_lot, execution.lots, count)?;
    let replay = RowReplayContextV1 {
        execution: context,
        candidate_id: verified.candidate_id,
        owner_id: execution.owner_id,
        order_id: execution.order_id,
        revision: expected_revision,
        order_nonce: execution.nonce,
        page_index: cursor.next_page,
        execution_index,
    };
    replay.validate()?;
    if any_active(&quantities, count) {
        children
            .collect_claims(replay, verified.outcome_count, &quantities)
            .map_err(|_| Error::ChildRefusal)?;
    }
    if execution.quote_debit != 0 {
        children
            .collect_collateral(replay, execution.quote_debit)
            .map_err(|_| Error::ChildRefusal)?;
    }
    for outcome in 0..count {
        cursor.claim_inventory[outcome] =
            add(cursor.claim_inventory[outcome], quantities[outcome])?;
    }
    cursor.quote_inventory = add(cursor.quote_inventory, execution.quote_debit)?;
    advance_execution(
        &mut cursor,
        page.execution_count(),
        Phase::Collecting,
        Phase::Materializing,
    )?;
    commit_settlement(output, cursor)
}

/// Execute the certificate's one complete-set movement.
pub fn materialize<C: SettlementChildrenV1>(
    output: &mut [u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    expected_revision: u64,
    children: &mut C,
) -> Result<()> {
    context.validate()?;
    let mut cursor = settlement_pre(output, verified, expected_revision, Phase::Materializing)?;
    let count = usize::from(verified.outcome_count);
    let quantity = verified.complete_set_quantity;
    let replay = AggregateReplayContextV1 {
        execution: context,
        candidate_id: verified.candidate_id,
        revision: expected_revision,
    };
    replay.validate()?;
    match verified.complete_set_move {
        CompleteSetMoveV1::None => {}
        CompleteSetMoveV1::Mint => {
            if cursor.quote_inventory < quantity {
                return Err(Error::Inventory);
            }
            children
                .mint_complete_set(replay, verified.outcome_count, quantity)
                .map_err(|_| Error::ChildRefusal)?;
            for outcome in 0..count {
                cursor.claim_inventory[outcome] = add(cursor.claim_inventory[outcome], quantity)?;
            }
            cursor.quote_inventory -= quantity;
        }
        CompleteSetMoveV1::Merge => {
            if cursor.claim_inventory[..count]
                .iter()
                .any(|available| *available < quantity)
            {
                return Err(Error::Inventory);
            }
            children
                .merge_complete_set(replay, verified.outcome_count, quantity)
                .map_err(|_| Error::ChildRefusal)?;
            for outcome in 0..count {
                cursor.claim_inventory[outcome] -= quantity;
            }
            cursor.quote_inventory = add(cursor.quote_inventory, quantity)?;
        }
    }
    cursor.phase = Phase::Distributing;
    cursor.next_page = 0;
    cursor.next_execution = 0;
    commit_settlement(output, cursor)
}

/// Distribute one exact next execution row, staging cursor bytes until every child accepts.
pub fn distribute_execution<C: SettlementChildrenV1>(
    output: &mut [u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    page_bytes: &[u8],
    expected_revision: u64,
    children: &mut C,
) -> Result<()> {
    context.validate()?;
    let mut cursor = settlement_pre(output, verified, expected_revision, Phase::Distributing)?;
    let page = settlement_page(verified, page_bytes, cursor.next_page)?;
    let count = usize::from(verified.outcome_count);
    let execution_index = cursor.next_execution;
    let execution = page
        .execution(usize::from(execution_index))
        .map_err(|_| Error::Codec)?;
    let quantities = scaled(&execution.receive_per_lot, execution.lots, count)?;
    if cursor.quote_inventory < execution.quote_credit {
        return Err(Error::Inventory);
    }
    for outcome in 0..count {
        if cursor.claim_inventory[outcome] < quantities[outcome] {
            return Err(Error::Inventory);
        }
    }
    let replay = RowReplayContextV1 {
        execution: context,
        candidate_id: verified.candidate_id,
        owner_id: execution.owner_id,
        order_id: execution.order_id,
        revision: expected_revision,
        order_nonce: execution.nonce,
        page_index: cursor.next_page,
        execution_index,
    };
    replay.validate()?;
    if any_active(&quantities, count) {
        children
            .distribute_claims(replay, verified.outcome_count, &quantities)
            .map_err(|_| Error::ChildRefusal)?;
    }
    if execution.quote_credit != 0 {
        children
            .distribute_collateral(replay, execution.quote_credit)
            .map_err(|_| Error::ChildRefusal)?;
    }
    for outcome in 0..count {
        cursor.claim_inventory[outcome] -= quantities[outcome];
    }
    cursor.quote_inventory -= execution.quote_credit;
    advance_execution(
        &mut cursor,
        page.execution_count(),
        Phase::Distributing,
        Phase::ReadyToClose,
    )?;
    commit_settlement(output, cursor)
}

/// Route the exact quote remainder and enter terminal state.
pub fn close<C: SettlementChildrenV1>(
    output: &mut [u8],
    context: ExecutionContextV1,
    verified: &VerifiedCandidateV1,
    expected_revision: u64,
    children: &mut C,
) -> Result<()> {
    context.validate()?;
    let mut cursor = settlement_pre(output, verified, expected_revision, Phase::ReadyToClose)?;
    if cursor.claim_inventory.iter().any(|value| *value != 0) {
        return Err(Error::Inventory);
    }
    let surplus = cursor.quote_inventory;
    if surplus != 0 {
        let replay = AggregateReplayContextV1 {
            execution: context,
            candidate_id: verified.candidate_id,
            revision: expected_revision,
        };
        replay.validate()?;
        children
            .pay_surplus(replay, surplus)
            .map_err(|_| Error::ChildRefusal)?;
    }
    cursor.quote_inventory = 0;
    cursor.quote_surplus_paid = add(cursor.quote_surplus_paid, surplus)?;
    cursor.phase = Phase::Terminal;
    cursor.next_page = 0;
    cursor.next_execution = 0;
    commit_settlement(output, cursor)
}

fn settlement_pre(
    output: &[u8],
    verified: &VerifiedCandidateV1,
    expected_revision: u64,
    phase: Phase,
) -> Result<SettlementCursorV1> {
    let cursor = SettlementCursorV1::decode(output).map_err(|_| Error::SettlementPhase)?;
    if cursor.candidate_id != verified.candidate_id
        || cursor.outcome_count != verified.outcome_count
        || cursor.page_count != verified.page_count
        || cursor.phase != phase
    {
        return Err(Error::SettlementPhase);
    }
    if cursor.revision != expected_revision {
        return Err(Error::RevisionMismatch);
    }
    Ok(cursor)
}

fn settlement_page<'a>(
    verified: &VerifiedCandidateV1,
    bytes: &'a [u8],
    expected_page: u32,
) -> Result<PageViewV1<'a>> {
    let page = PageViewV1::decode(bytes).map_err(|_| Error::Codec)?;
    if page.candidate_id() != verified.candidate_id
        || page.outcome_count() != verified.outcome_count
        || page.page_count() != verified.page_count
        || page.page_index() != expected_page
    {
        return Err(Error::CandidateBinding);
    }
    Ok(page)
}

fn advance_execution(
    cursor: &mut SettlementCursorV1,
    execution_count: u8,
    current: Phase,
    final_phase: Phase,
) -> Result<()> {
    if cursor.phase != current {
        return Err(Error::SettlementPhase);
    }
    let next_execution = cursor
        .next_execution
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if next_execution < execution_count {
        cursor.next_execution = next_execution;
        return Ok(());
    }
    cursor.next_execution = 0;
    let next = cursor
        .next_page
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if next == cursor.page_count {
        cursor.phase = final_phase;
        cursor.next_page = 0;
    } else {
        cursor.next_page = next;
    }
    Ok(())
}

fn commit_settlement(output: &mut [u8], mut cursor: SettlementCursorV1) -> Result<()> {
    cursor.revision = cursor
        .revision
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let bytes = cursor.to_bytes().map_err(|_| Error::SettlementPhase)?;
    output.copy_from_slice(&bytes);
    Ok(())
}

fn scaled(values: &[u64; MAX_OUTCOMES], lots: u64, count: usize) -> Result<[u64; MAX_OUTCOMES]> {
    let mut output = [0_u64; MAX_OUTCOMES];
    for index in 0..count {
        output[index] = multiply(values[index], lots)?;
    }
    Ok(output)
}

fn any_active(values: &[u64; MAX_OUTCOMES], count: usize) -> bool {
    values[..count].iter().any(|value| *value != 0)
}

fn le_numeric_id(left: &[u8; 32], right: &[u8; 32]) -> bool {
    for index in (0..32).rev() {
        if left[index] != right[index] {
            return left[index] < right[index];
        }
    }
    false
}

fn add(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right).ok_or(Error::ArithmeticOverflow)
}

fn multiply(left: u64, right: u64) -> Result<u64> {
    left.checked_mul(right).ok_or(Error::ArithmeticOverflow)
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn zero(input: &[u8], offset: usize, width: usize) -> Result<bool> {
    let end = offset.checked_add(width).ok_or(Error::Certificate)?;
    Ok(input
        .get(offset..end)
        .ok_or(Error::Certificate)?
        .iter()
        .all(|byte| *byte == 0))
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::Certificate)
}

fn read_slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8]> {
    let end = offset.checked_add(width).ok_or(Error::Certificate)?;
    input.get(offset..end).ok_or(Error::Certificate)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::Certificate)?;
    input
        .get(offset..end)
        .ok_or(Error::Certificate)?
        .try_into()
        .map_err(|_| Error::Certificate)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_u64_array(input: &[u8], offset: usize) -> Result<[u64; MAX_OUTCOMES]> {
    let mut output = [0_u64; MAX_OUTCOMES];
    for (index, value) in output.iter_mut().enumerate() {
        *value = read_u64(input, offset + index * 8)?;
    }
    Ok(output)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset.checked_add(value.len()).ok_or(Error::Certificate)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Certificate)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::Certificate)? = value;
    Ok(())
}

fn put_u64_array(output: &mut [u8], offset: usize, values: &[u64; MAX_OUTCOMES]) -> Result<()> {
    for (index, value) in values.iter().enumerate() {
        put(output, offset + index * 8, &value.to_le_bytes())?;
    }
    Ok(())
}

fn infallible_put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset + value.len()) {
        target.copy_from_slice(value);
    }
}

fn infallible_put_u64_array(output: &mut [u8], offset: usize, values: &[u64; MAX_OUTCOMES]) {
    for (index, value) in values.iter().enumerate() {
        infallible_put(output, offset + index * 8, &value.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_general_codec::{
        ExecutionV1, MAX_EXECUTIONS_PER_PAGE, PageV1, SELECTION_CURSOR_BYTES,
        SETTLEMENT_CURSOR_BYTES,
    };

    fn id(low: u8) -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[0] = low;
        value
    }

    fn vector(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
        let mut values = [0_u64; MAX_OUTCOMES];
        values[0] = first;
        values[1] = second;
        values
    }

    fn execution(
        order: u8,
        owner: u8,
        receive: [u64; MAX_OUTCOMES],
        lots: u64,
        max_lots: u64,
        debit: u64,
    ) -> ExecutionV1 {
        ExecutionV1 {
            order_id: id(order),
            owner_id: id(owner),
            nonce: 1,
            max_lots,
            max_quote_debit_per_lot: 1,
            lots,
            quote_debit: debit,
            quote_credit: 0,
            receive_per_lot: receive,
            deliver_per_lot: [0; MAX_OUTCOMES],
        }
    }

    fn candidate(candidate_id: u8, page_count: u32) -> CandidateV1 {
        CandidateV1 {
            outcome_count: 2,
            candidate_id: id(candidate_id),
            product_id: id(31),
            batch_id: id(41),
            page_count,
            price_scale: 2,
            prices: vector(1, 1),
        }
    }

    fn page(
        candidate_id: u8,
        page_index: u32,
        page_count: u32,
        rows: &[ExecutionV1],
    ) -> [u8; dclutch_general_codec::PAGE_BYTES] {
        let mut executions = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
        executions[..rows.len()].copy_from_slice(rows);
        PageV1 {
            outcome_count: 2,
            candidate_id: id(candidate_id),
            page_index,
            page_count,
            execution_count: u8::try_from(rows.len()).expect("bounded rows"),
            executions,
        }
        .to_bytes()
        .expect("canonical page")
    }

    fn mint_fixture() -> (
        CandidateV1,
        [u8; dclutch_general_codec::PAGE_BYTES],
        VerifiedCandidateV1,
    ) {
        let candidate = candidate(21, 1);
        let page = page(
            21,
            0,
            1,
            &[
                execution(1, 11, vector(1, 0), 1, 1, 1),
                execution(2, 12, vector(0, 1), 1, 1, 1),
            ],
        );
        let mut verifier = CandidateVerifierV1::begin(candidate);
        verifier.ingest_page(&page).expect("page verifies");
        let verified = verifier.finish().expect("candidate verifies");
        (candidate, page, verified)
    }

    fn policy() -> SelectionPolicyV1 {
        let mut criteria =
            [SelectionCriterion::MaximizeFilledLots; dclutch_general_codec::MAX_SELECTION_CRITERIA];
        criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
        criteria[2] = SelectionCriterion::MinimizeCandidateId;
        SelectionPolicyV1 {
            policy_id: id(51),
            criterion_count: 3,
            criteria,
        }
    }

    fn context() -> ExecutionContextV1 {
        ExecutionContextV1 {
            market_id: id(61),
            release_set_id: id(71),
        }
    }

    #[derive(Default)]
    struct MockChildren {
        calls: u8,
        fail_at: u8,
        claims_collect: u8,
        collateral_collect: u8,
        mint: u8,
        merge: u8,
        claims_distribute: u8,
        collateral_distribute: u8,
        surplus: u8,
        last_row: Option<RowReplayContextV1>,
        last_aggregate: Option<AggregateReplayContextV1>,
    }

    impl MockChildren {
        fn step(&mut self) -> core::result::Result<(), ChildExecutionError> {
            self.calls += 1;
            if self.fail_at != 0 && self.calls == self.fail_at {
                Err(ChildExecutionError::Refused)
            } else {
                Ok(())
            }
        }
    }

    impl SettlementChildrenV1 for MockChildren {
        fn collect_claims(
            &mut self,
            context: RowReplayContextV1,
            _: u8,
            _: &[u64; MAX_OUTCOMES],
        ) -> core::result::Result<(), ChildExecutionError> {
            self.claims_collect += 1;
            self.last_row = Some(context);
            self.step()
        }

        fn collect_collateral(
            &mut self,
            context: RowReplayContextV1,
            _: u64,
        ) -> core::result::Result<(), ChildExecutionError> {
            self.collateral_collect += 1;
            self.last_row = Some(context);
            self.step()
        }

        fn mint_complete_set(
            &mut self,
            context: AggregateReplayContextV1,
            _: u8,
            _: u64,
        ) -> core::result::Result<(), ChildExecutionError> {
            self.mint += 1;
            self.last_aggregate = Some(context);
            self.step()
        }

        fn merge_complete_set(
            &mut self,
            context: AggregateReplayContextV1,
            _: u8,
            _: u64,
        ) -> core::result::Result<(), ChildExecutionError> {
            self.merge += 1;
            self.last_aggregate = Some(context);
            self.step()
        }

        fn distribute_claims(
            &mut self,
            context: RowReplayContextV1,
            _: u8,
            _: &[u64; MAX_OUTCOMES],
        ) -> core::result::Result<(), ChildExecutionError> {
            self.claims_distribute += 1;
            self.last_row = Some(context);
            self.step()
        }

        fn distribute_collateral(
            &mut self,
            context: RowReplayContextV1,
            _: u64,
        ) -> core::result::Result<(), ChildExecutionError> {
            self.collateral_distribute += 1;
            self.last_row = Some(context);
            self.step()
        }

        fn pay_surplus(
            &mut self,
            context: AggregateReplayContextV1,
            _: u64,
        ) -> core::result::Result<(), ChildExecutionError> {
            self.surplus += 1;
            self.last_aggregate = Some(context);
            self.step()
        }
    }

    #[test]
    fn candidate_wide_rounding_crosses_page_boundaries_once() {
        let candidate_two = candidate(22, 2);
        let first = page(22, 0, 2, &[execution(1, 11, vector(1, 0), 1, 2, 1)]);
        let second = page(
            22,
            1,
            2,
            &[
                execution(1, 11, vector(1, 0), 1, 2, 0),
                execution(2, 12, vector(0, 1), 1, 2, 1),
                execution(2, 12, vector(0, 1), 1, 2, 0),
            ],
        );
        let mut verifier = CandidateVerifierV1::begin(candidate_two);
        verifier.ingest_page(&first).expect("first page");
        verifier.ingest_page(&second).expect("second page");
        let verified = verifier.finish().expect("aggregate rounding");
        assert_eq!(verified.filled_lots, 4);
        assert_eq!(verified.quote_inputs, 2);
        assert_eq!(verified.complete_set_move, CompleteSetMoveV1::Mint);
        assert_eq!(verified.complete_set_quantity, 2);
        assert_eq!(verified.quote_surplus, 0);
    }

    #[test]
    fn persisted_verifier_roundtrips_and_stale_or_hostile_bytes_refuse_atomically() {
        let candidate = candidate(24, 2);
        let first = page(24, 0, 2, &[execution(1, 11, vector(1, 0), 1, 2, 1)]);
        let second = page(24, 1, 2, &[execution(2, 12, vector(0, 1), 1, 2, 1)]);
        let mut verifier = CandidateVerifierV1::begin(candidate);
        verifier.ingest_page_at(&first, 0).expect("first page");
        assert_eq!(verifier.revision(), 1);
        assert_eq!(verifier.next_page(), 1);
        let bytes = verifier.to_bytes().expect("encode cursor");
        assert_eq!(CandidateVerifierV1::decode(&bytes), Ok(verifier));

        let snapshot = verifier;
        assert_eq!(
            verifier.ingest_page_at(&second, 0),
            Err(Error::RevisionMismatch)
        );
        assert_eq!(verifier, snapshot);

        let mut hostile = bytes;
        hostile[11] = 1;
        assert_eq!(
            CandidateVerifierV1::decode(&hostile),
            Err(Error::Certificate)
        );
    }

    #[test]
    fn out_of_order_substitution_and_fragment_rounding_refuse_atomically() {
        let candidate_two = candidate(22, 2);
        let first = page(22, 0, 2, &[execution(2, 12, vector(0, 1), 1, 2, 1)]);
        let hostile = page(22, 1, 2, &[execution(1, 11, vector(1, 0), 1, 2, 1)]);
        let mut verifier = CandidateVerifierV1::begin(candidate_two);
        verifier.ingest_page(&first).expect("first page");
        let snapshot = verifier;
        assert_eq!(
            verifier.ingest_page(&hostile),
            Err(Error::NonCanonicalOrder)
        );
        assert_eq!(verifier, snapshot);

        let candidate_one = candidate(23, 1);
        let per_fragment_rounded = page(
            23,
            0,
            1,
            &[
                execution(1, 11, vector(1, 0), 1, 2, 1),
                execution(1, 11, vector(1, 0), 1, 2, 1),
                execution(2, 12, vector(0, 1), 2, 2, 1),
            ],
        );
        let mut verifier = CandidateVerifierV1::begin(candidate_one);
        let snapshot = verifier;
        assert_eq!(
            verifier.ingest_page(&per_fragment_rounded),
            Err(Error::QuoteMismatch)
        );
        assert_eq!(verifier, snapshot);

        let mut substituted = execution(1, 11, vector(1, 0), 1, 2, 1);
        let first = substituted;
        substituted.owner_id = id(99);
        let hostile = page(23, 0, 1, &[first, substituted]);
        let mut verifier = CandidateVerifierV1::begin(candidate_one);
        assert_eq!(
            verifier.ingest_page(&hostile),
            Err(Error::OrderSubstitution)
        );
    }

    #[test]
    fn selection_and_certificate_refusals_preserve_bytes() {
        let (candidate, _, verified) = mint_fixture();
        let mut selection = [0_u8; SELECTION_CURSOR_BYTES];
        let mut certificate = [0_u8; VERIFIED_CANDIDATE_BYTES_V1];
        consider_verified(
            &mut selection,
            &mut certificate,
            &candidate,
            &policy(),
            verified,
            None,
            0,
        )
        .expect("first valid submission");
        assert_eq!(VerifiedCandidateV1::decode(&certificate), Ok(verified));
        let selection_snapshot = selection;
        let mut second_certificate = [0_u8; VERIFIED_CANDIDATE_BYTES_V1];
        assert_eq!(
            consider_verified(
                &mut selection,
                &mut second_certificate,
                &candidate,
                &policy(),
                verified,
                Some(&verified),
                0,
            ),
            Err(Error::RevisionMismatch)
        );
        assert_eq!(selection, selection_snapshot);
        assert!(second_certificate.iter().all(|byte| *byte == 0));

        let mut hostile = certificate;
        hostile[0] ^= 1;
        assert_eq!(
            VerifiedCandidateV1::decode(&hostile),
            Err(Error::Certificate)
        );
        freeze_selection(&mut selection, 1).expect("freeze");
        assert!(
            SelectionCursorV1::decode(&selection)
                .expect("cursor")
                .closed
        );
    }

    #[test]
    fn full_streamed_mint_settlement_calls_disjoint_child_requirements() {
        let (candidate, page, verified) = mint_fixture();
        let mut selection = [0_u8; SELECTION_CURSOR_BYTES];
        let mut certificate = [0_u8; VERIFIED_CANDIDATE_BYTES_V1];
        consider_verified(
            &mut selection,
            &mut certificate,
            &candidate,
            &policy(),
            verified,
            None,
            0,
        )
        .expect("consider");
        freeze_selection(&mut selection, 1).expect("freeze");
        let mut settlement = [0_u8; SETTLEMENT_CURSOR_BYTES];
        initialize_settlement(&mut settlement, &selection, &verified, 0).expect("initialize");
        let mut children = MockChildren::default();
        collect_execution(
            &mut settlement,
            context(),
            &verified,
            &page,
            0,
            &mut children,
        )
        .expect("collect row zero");
        collect_execution(
            &mut settlement,
            context(),
            &verified,
            &page,
            1,
            &mut children,
        )
        .expect("collect row one");
        assert_eq!(children.claims_collect, 0);
        assert_eq!(children.collateral_collect, 2);

        materialize(&mut settlement, context(), &verified, 2, &mut children).expect("materialize");
        assert_eq!(children.mint, 1);
        assert_eq!(children.merge, 0);

        distribute_execution(
            &mut settlement,
            context(),
            &verified,
            &page,
            3,
            &mut children,
        )
        .expect("distribute row zero");
        distribute_execution(
            &mut settlement,
            context(),
            &verified,
            &page,
            4,
            &mut children,
        )
        .expect("distribute row one");
        assert_eq!(children.claims_distribute, 2);
        assert_eq!(children.collateral_distribute, 0);

        close(&mut settlement, context(), &verified, 5, &mut children).expect("close");
        assert_eq!(children.surplus, 1);
        let terminal = SettlementCursorV1::decode(&settlement).expect("terminal cursor");
        assert_eq!(terminal.phase, Phase::Terminal);
        assert_eq!(terminal.quote_inventory, 0);
        assert_eq!(terminal.quote_surplus_paid, 1);
        assert_eq!(terminal.revision, 6);
    }

    #[test]
    fn child_refusal_and_early_distribution_preserve_cursor_bytes() {
        let (candidate, page, verified) = mint_fixture();
        let mut selection = [0_u8; SELECTION_CURSOR_BYTES];
        let mut certificate = [0_u8; VERIFIED_CANDIDATE_BYTES_V1];
        consider_verified(
            &mut selection,
            &mut certificate,
            &candidate,
            &policy(),
            verified,
            None,
            0,
        )
        .expect("consider");
        freeze_selection(&mut selection, 1).expect("freeze");
        let mut settlement = [0_u8; SETTLEMENT_CURSOR_BYTES];
        initialize_settlement(&mut settlement, &selection, &verified, 0).expect("initialize");
        let snapshot = settlement;
        let mut accepting = MockChildren::default();
        assert_eq!(
            distribute_execution(
                &mut settlement,
                context(),
                &verified,
                &page,
                0,
                &mut accepting
            ),
            Err(Error::SettlementPhase)
        );
        assert_eq!(settlement, snapshot);
        let mut refusing = MockChildren {
            fail_at: 1,
            ..MockChildren::default()
        };
        assert_eq!(
            collect_execution(
                &mut settlement,
                context(),
                &verified,
                &page,
                0,
                &mut refusing
            ),
            Err(Error::ChildRefusal)
        );
        assert_eq!(settlement, snapshot);
        assert_eq!(refusing.calls, 1);
    }

    #[test]
    fn child_trait_receives_exact_general_owned_replay_coordinates() {
        let (_, page, verified) = mint_fixture();
        let mut settlement = SettlementCursorV1 {
            phase: Phase::Collecting,
            outcome_count: 2,
            candidate_id: verified.candidate_id,
            page_count: 1,
            next_page: 0,
            next_execution: 0,
            revision: 0,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 0,
            quote_surplus_paid: 0,
        }
        .to_bytes()
        .expect("cursor");
        let mut children = MockChildren {
            fail_at: 1,
            ..MockChildren::default()
        };
        collect_execution(
            &mut settlement,
            context(),
            &verified,
            &page,
            0,
            &mut children,
        )
        .expect_err("child refuses");
        let row = children.last_row.expect("row context");
        assert_eq!(row.execution, context());
        assert_eq!(row.candidate_id, verified.candidate_id);
        assert_eq!(row.owner_id, id(11));
        assert_eq!(row.order_id, id(1));
        assert_eq!(row.order_nonce, 1);
        assert_eq!(row.revision, 0);
        assert_eq!(row.page_index, 0);
        assert_eq!(row.execution_index, 0);
    }
}
