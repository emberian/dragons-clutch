//! Authenticated projection into the General V2 owner-settlement fee rows.

use clutch_batch_policy_identity::revenue_policy_v1::RevenuePolicyV1;
use clutch_owner_settlement::{
    owner_debit_atoms, CandidateSettlementTotalsV1, CandidateSettlementTotalsV2,
    OwnerSettlementExpectationV2, SelectedOwnerFeeV1, MAX_ORDERS,
};

use crate::allocation::{
    allocate_payer_debit, allocate_recipients, FeeEnvelopeFundingV1, FeeEnvelopeV1,
    PayerAllocationV1, RecipientAllocationV1, StandingMakerRowV1,
};
use crate::intent::{OwnerFeeTransitionIntentV1, RecipientAllocationIntentV1};
use crate::selected::{
    AssessmentBoundaryV1, OwnerFeeAssessmentV1, OwnerFeeCarryV1, SelectedCompositeFeeV1,
};
use crate::{live, Error, Id, Result, MAX_FEE_ROWS_V1};

const _: () = assert!(MAX_FEE_ROWS_V1 == MAX_ORDERS);

/// Authenticated owner-order projection needed to prove fee funding. These
/// values must come from the same complete selected-order set consumed by
/// `build_owner_settlement_book_v1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedOwnerFeeFundingV1 {
    pub owner: Id,
    pub price_scale: u64,
    pub buy_price_units: u128,
    pub reserved_buy_cash_atoms: u64,
    pub has_buy: bool,
    pub has_sell: bool,
}

/// A private-construction bridge result carrying the exact public row expected
/// by `clutch-owner-settlement` plus every account identity that authenticated
/// it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSelectedOwnerFeeV1 {
    fee_record: Id,
    carry_account: Id,
    payer_allocation_account: Id,
    owner_settlement_account: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    row: SelectedOwnerFeeV1,
}

/// Presence-explicit successor to [`AuthenticatedSelectedOwnerFeeV1`].
///
/// V2 is minted only from the exact V2 owner-settlement expectation, so a
/// present zero-price buy cannot be confused with an absent buy side. The
/// public fee row remains the same exact `(owner, fee_atoms)` contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSelectedOwnerFeeV2 {
    fee_record: Id,
    carry_account: Id,
    payer_allocation_account: Id,
    owner_settlement_account: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    row: SelectedOwnerFeeV1,
}

impl AuthenticatedSelectedOwnerFeeV2 {
    pub const EMPTY: Self = Self {
        fee_record: Id([0; 32]),
        carry_account: Id([0; 32]),
        payer_allocation_account: Id([0; 32]),
        owner_settlement_account: Id([0; 32]),
        settlement_candidate: Id([0; 32]),
        revenue_policy: Id([0; 32]),
        row: SelectedOwnerFeeV1::EMPTY,
    };

    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn carry_account(&self) -> Id {
        self.carry_account
    }

    pub const fn payer_allocation_account(&self) -> Id {
        self.payer_allocation_account
    }

    pub const fn owner_settlement_account(&self) -> Id {
        self.owner_settlement_account
    }

    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn row(&self) -> SelectedOwnerFeeV1 {
        self.row
    }
}

impl AuthenticatedSelectedOwnerFeeV1 {
    pub const EMPTY: Self = Self {
        fee_record: Id([0; 32]),
        carry_account: Id([0; 32]),
        payer_allocation_account: Id([0; 32]),
        owner_settlement_account: Id([0; 32]),
        settlement_candidate: Id([0; 32]),
        revenue_policy: Id([0; 32]),
        row: SelectedOwnerFeeV1::EMPTY,
    };

    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn carry_account(&self) -> Id {
        self.carry_account
    }

    pub const fn payer_allocation_account(&self) -> Id {
        self.payer_allocation_account
    }

    pub const fn owner_settlement_account(&self) -> Id {
        self.owner_settlement_account
    }

    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn row(&self) -> SelectedOwnerFeeV1 {
        self.row
    }
}

/// Canonical complete fee projection consumed by the owner-settlement builder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedOwnerFeeBookV1 {
    fee_record: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    rows: [SelectedOwnerFeeV1; MAX_ORDERS],
    owner_count: u8,
    selected_fee_atoms: u128,
}

impl SelectedOwnerFeeBookV1 {
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    pub const fn rows(&self) -> &[SelectedOwnerFeeV1; MAX_ORDERS] {
        &self.rows
    }

    pub const fn owner_count(&self) -> u8 {
        self.owner_count
    }

    pub const fn selected_fee_atoms(&self) -> u128 {
        self.selected_fee_atoms
    }
}

/// Project one terminal owner carry into the owner-settlement builder's exact
/// `SelectedOwnerFeeV1` row.
///
/// `envelopes` is the pre-transition reservation state. Recomputing `payer`
/// proves the terminal debit, while the post-transition sum of every intent's
/// cumulative fee debit proves the closed carry's cumulative fee total.
#[allow(clippy::too_many_arguments)]
pub fn project_terminal_owner_fee_v1(
    selected: &SelectedCompositeFeeV1,
    transition: &OwnerFeeTransitionIntentV1,
    carry: &OwnerFeeCarryV1,
    assessment: &OwnerFeeAssessmentV1,
    payer: &PayerAllocationV1,
    funding: VerifiedOwnerFeeFundingV1,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    envelope_len: u8,
) -> Result<AuthenticatedSelectedOwnerFeeV1> {
    live(funding.owner)?;
    if transition.fee_record().identity() != selected.fee_record()
        || transition.settlement_candidate() != selected.selected_candidate()
        || transition.revenue_policy() != selected.revenue_policy()
        || transition.owner() != funding.owner
        || carry.fee_record() != selected.fee_record()
        || carry.owner() != funding.owner
        || carry.denominator() != selected.carry_denominator()
        || assessment.fee_record() != selected.fee_record()
        || assessment.owner() != funding.owner
        || assessment.denominator() != selected.carry_denominator()
        || payer.fee_record() != selected.fee_record()
        || payer.owner() != funding.owner
        || payer.carry_denominator() != selected.carry_denominator()
    {
        return Err(Error::MismatchedBinding);
    }
    if !carry.is_closed()
        || carry.remainder() != 0
        || assessment.boundary() != AssessmentBoundaryV1::TerminalCeil
        || assessment.next_carry() != 0
        || payer.boundary() != AssessmentBoundaryV1::TerminalCeil
        || payer.next_carry() != 0
    {
        return Err(Error::TerminalStateRequired);
    }
    if funding.price_scale != selected.price_scale()
        || (!funding.has_buy && !funding.has_sell)
        || funding.has_buy != (funding.buy_price_units != 0)
        || (!funding.has_buy && funding.reserved_buy_cash_atoms != 0)
    {
        return Err(Error::InvalidAccountData);
    }

    let recomputed = allocate_payer_debit(assessment, envelopes, envelope_len)?;
    if recomputed != *payer {
        return Err(Error::MismatchedBinding);
    }

    let mut post_debited_atoms = 0u128;
    let mut has_buy_envelope = false;
    let mut index = 0usize;
    while index < usize::from(envelope_len) {
        let envelope = envelopes[index];
        let transition_debit = payer.debit_atoms()[index];
        if envelope.funding == FeeEnvelopeFundingV1::BuyCashReservation {
            has_buy_envelope = true;
        } else if transition_debit != 0 {
            return Err(Error::SellerFeeForbidden);
        }
        post_debited_atoms = post_debited_atoms
            .checked_add(u128::from(envelope.debited_atoms))
            .and_then(|value| value.checked_add(u128::from(transition_debit)))
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if post_debited_atoms != u128::from(carry.paid_atoms()) {
        return Err(Error::ConservationFailure);
    }
    if carry.paid_atoms() != 0 && (!funding.has_buy || !has_buy_envelope) {
        return Err(Error::SellerFeeForbidden);
    }
    let required = owner_debit_atoms(
        funding.buy_price_units,
        funding.price_scale,
        carry.paid_atoms(),
    )
    .map_err(|_| Error::ArithmeticOverflow)?;
    if required > funding.reserved_buy_cash_atoms {
        return Err(Error::InsufficientBuyReservation);
    }

    Ok(AuthenticatedSelectedOwnerFeeV1 {
        fee_record: selected.fee_record(),
        carry_account: transition.carry().identity(),
        payer_allocation_account: transition.payer_allocation().identity(),
        owner_settlement_account: transition.owner_settlement().identity(),
        settlement_candidate: selected.selected_candidate(),
        revenue_policy: selected.revenue_policy(),
        row: SelectedOwnerFeeV1 {
            owner: funding.owner.0,
            fee_atoms: carry.paid_atoms(),
        },
    })
}

/// Project one terminal owner carry through the presence-explicit V2 owner row.
///
/// The authenticated V2 expectation is the sole owner of side presence,
/// consideration, and reserved buy cash. In particular, zero consideration is
/// valid when the buy side is present. The payer snapshot is reconstructed
/// from every signed pre-transition envelope before its complete bytes may be
/// used as Replay evidence and atomically deleted.
#[allow(clippy::too_many_arguments)]
pub fn project_terminal_owner_fee_v2(
    selected: &SelectedCompositeFeeV1,
    transition: &OwnerFeeTransitionIntentV1,
    carry: &OwnerFeeCarryV1,
    assessment: &OwnerFeeAssessmentV1,
    payer: &PayerAllocationV1,
    expectation: OwnerSettlementExpectationV2,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    envelope_len: u8,
) -> Result<AuthenticatedSelectedOwnerFeeV2> {
    expectation
        .validate()
        .map_err(|_| Error::InvalidAccountData)?;
    let owner = Id(expectation.owner);
    live(owner)?;
    if transition.fee_record().identity() != selected.fee_record()
        || transition.settlement_candidate() != selected.selected_candidate()
        || transition.revenue_policy() != selected.revenue_policy()
        || transition.owner() != owner
        || carry.fee_record() != selected.fee_record()
        || carry.owner() != owner
        || carry.denominator() != selected.carry_denominator()
        || assessment.fee_record() != selected.fee_record()
        || assessment.owner() != owner
        || assessment.denominator() != selected.carry_denominator()
        || payer.fee_record() != selected.fee_record()
        || payer.owner() != owner
        || payer.carry_denominator() != selected.carry_denominator()
        || expectation.candidate != selected.selected_candidate().0
        || expectation.price_scale != selected.price_scale()
        || expectation.selected_fee_atoms != carry.paid_atoms()
    {
        return Err(Error::MismatchedBinding);
    }
    if !carry.is_closed()
        || carry.remainder() != 0
        || assessment.boundary() != AssessmentBoundaryV1::TerminalCeil
        || assessment.next_carry() != 0
        || payer.boundary() != AssessmentBoundaryV1::TerminalCeil
        || payer.next_carry() != 0
    {
        return Err(Error::TerminalStateRequired);
    }

    let recomputed = allocate_payer_debit(assessment, envelopes, envelope_len)?;
    if recomputed != *payer {
        return Err(Error::MismatchedBinding);
    }

    let has_buy = expectation.expected_buy_order_mask != 0;
    let mut post_debited_atoms = 0u128;
    let mut has_buy_envelope = false;
    let mut index = 0usize;
    while index < usize::from(envelope_len) {
        let envelope = envelopes[index];
        let transition_debit = payer.debit_atoms()[index];
        if envelope.funding == FeeEnvelopeFundingV1::BuyCashReservation {
            has_buy_envelope = true;
        } else if transition_debit != 0 {
            return Err(Error::SellerFeeForbidden);
        }
        post_debited_atoms = post_debited_atoms
            .checked_add(u128::from(envelope.debited_atoms))
            .and_then(|value| value.checked_add(u128::from(transition_debit)))
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if post_debited_atoms != u128::from(carry.paid_atoms()) {
        return Err(Error::ConservationFailure);
    }
    if carry.paid_atoms() != 0 && (!has_buy || !has_buy_envelope) {
        return Err(Error::SellerFeeForbidden);
    }
    let required = owner_debit_atoms(
        expectation.expected_buy_price_units.value,
        expectation.price_scale,
        carry.paid_atoms(),
    )
    .map_err(|_| Error::ArithmeticOverflow)?;
    if required > expectation.reserved_cash_atoms {
        return Err(Error::InsufficientBuyReservation);
    }

    Ok(AuthenticatedSelectedOwnerFeeV2 {
        fee_record: selected.fee_record(),
        carry_account: transition.carry().identity(),
        payer_allocation_account: transition.payer_allocation().identity(),
        owner_settlement_account: transition.owner_settlement().identity(),
        settlement_candidate: selected.selected_candidate(),
        revenue_policy: selected.revenue_policy(),
        row: SelectedOwnerFeeV1 {
            owner: expectation.owner,
            fee_atoms: carry.paid_atoms(),
        },
    })
}

/// Require one authenticated projection for every canonical participating
/// owner, including explicit zero rows, and bind the exact candidate total.
pub fn assemble_selected_owner_fee_book_v1(
    selected: &SelectedCompositeFeeV1,
    participating_owners: &[Id; MAX_ORDERS],
    projections: &[AuthenticatedSelectedOwnerFeeV1; MAX_ORDERS],
    projection_len: u8,
    expected: CandidateSettlementTotalsV1,
) -> Result<SelectedOwnerFeeBookV1> {
    let owner_count = u8::try_from(expected.owner_count).map_err(|_| Error::InvalidWidth)?;
    if owner_count == 0 || usize::from(owner_count) > MAX_ORDERS || projection_len != owner_count {
        return Err(Error::MissingParticipant);
    }
    let mut rows = [SelectedOwnerFeeV1::EMPTY; MAX_ORDERS];
    let mut total = 0u128;
    let mut index = 0usize;
    while index < usize::from(owner_count) {
        let owner = participating_owners[index];
        live(owner)?;
        if index != 0 && owner <= participating_owners[index - 1] {
            return Err(Error::NonCanonicalOrder);
        }
        let projection = projections[index];
        if projection.fee_record != selected.fee_record()
            || projection.settlement_candidate != selected.selected_candidate()
            || projection.revenue_policy != selected.revenue_policy()
            || projection.row.owner != owner.0
        {
            return Err(Error::MissingParticipant);
        }
        rows[index] = projection.row;
        total = total
            .checked_add(u128::from(projection.row.fee_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    while index < MAX_ORDERS {
        if participating_owners[index] != Id([0; 32])
            || projections[index] != AuthenticatedSelectedOwnerFeeV1::EMPTY
        {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if total != expected.selected_fee_atoms {
        return Err(Error::SelectedFeeTotalMismatch);
    }
    Ok(SelectedOwnerFeeBookV1 {
        fee_record: selected.fee_record(),
        settlement_candidate: selected.selected_candidate(),
        revenue_policy: selected.revenue_policy(),
        rows,
        owner_count,
        selected_fee_atoms: total,
    })
}

/// Assemble the exact fee book used by the presence-explicit V2 settlement.
/// Explicit seller-only and zero-fee participants remain mandatory rows.
pub fn assemble_selected_owner_fee_book_v2(
    selected: &SelectedCompositeFeeV1,
    participating_owners: &[Id; MAX_ORDERS],
    projections: &[AuthenticatedSelectedOwnerFeeV2; MAX_ORDERS],
    projection_len: u8,
    expected: CandidateSettlementTotalsV2,
) -> Result<SelectedOwnerFeeBookV1> {
    let owner_count = u8::try_from(expected.owner_count).map_err(|_| Error::InvalidWidth)?;
    if owner_count == 0 || usize::from(owner_count) > MAX_ORDERS || projection_len != owner_count {
        return Err(Error::MissingParticipant);
    }
    let mut rows = [SelectedOwnerFeeV1::EMPTY; MAX_ORDERS];
    let mut total = 0u128;
    let mut index = 0usize;
    while index < usize::from(owner_count) {
        let owner = participating_owners[index];
        live(owner)?;
        if index != 0 && owner <= participating_owners[index - 1] {
            return Err(Error::NonCanonicalOrder);
        }
        let projection = projections[index];
        if projection.fee_record != selected.fee_record()
            || projection.settlement_candidate != selected.selected_candidate()
            || projection.revenue_policy != selected.revenue_policy()
            || projection.row.owner != owner.0
        {
            return Err(Error::MissingParticipant);
        }
        rows[index] = projection.row;
        total = total
            .checked_add(u128::from(projection.row.fee_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    while index < MAX_ORDERS {
        if participating_owners[index] != Id([0; 32])
            || projections[index] != AuthenticatedSelectedOwnerFeeV2::EMPTY
        {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if total != expected.selected_fee_atoms {
        return Err(Error::SelectedFeeTotalMismatch);
    }
    Ok(SelectedOwnerFeeBookV1 {
        fee_record: selected.fee_record(),
        settlement_candidate: selected.selected_candidate(),
        revenue_policy: selected.revenue_policy(),
        rows,
        owner_count,
        selected_fee_atoms: total,
    })
}

/// Allocate recipients from the exact candidate selected-fee total, refusing
/// if the candidate-wide aggregate cannot fit the collateral account's `u64`
/// amount domain.
pub fn allocate_selected_owner_fee_recipients_v1(
    selected: &SelectedCompositeFeeV1,
    intent: &RecipientAllocationIntentV1,
    policy: &RevenuePolicyV1,
    book: &SelectedOwnerFeeBookV1,
    makers: &[StandingMakerRowV1; MAX_FEE_ROWS_V1],
    maker_len: u8,
) -> Result<RecipientAllocationV1> {
    if book.fee_record != selected.fee_record()
        || book.settlement_candidate != selected.selected_candidate()
        || book.revenue_policy != selected.revenue_policy()
        || intent.fee_record().identity() != selected.fee_record()
        || intent.settlement_candidate() != selected.selected_candidate()
        || intent.revenue_policy() != selected.revenue_policy()
        || intent.treasury_position() != selected.treasury_position()
    {
        return Err(Error::MismatchedBinding);
    }
    let total = u64::try_from(book.selected_fee_atoms).map_err(|_| Error::AmountOutOfRange)?;
    allocate_recipients(selected, policy, makers, maker_len, total)
}
