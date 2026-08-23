//! Canonical payer-envelope and recipient allocation.

use clutch_batch_policy_identity::revenue_policy_v1::{RevenuePolicyV1, StandingMakerV1};

use crate::selected::{AssessmentBoundaryV1, OwnerFeeAssessmentV1, SelectedCompositeFeeV1};
use crate::{add, live, Error, Id, Result, MAX_FEE_ROWS_V1};

/// One signed intent's remaining fee authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FeeEnvelopeFundingV1 {
    /// No cash reservation exists for this intent. V1 seller intents use this
    /// member and must authorize and pay exactly zero fee atoms.
    NoCashReservation = 0,
    /// Fee atoms are inside this buy intent's authenticated cash reservation.
    BuyCashReservation = 1,
}

/// One signed intent's remaining fee authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeEnvelopeV1 {
    pub owner: Id,
    pub intent: Id,
    pub funding: FeeEnvelopeFundingV1,
    pub max_fee_atoms: u64,
    pub debited_atoms: u64,
}

/// Fixed allocation of an owner-level fee debit across signed intents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayerAllocationV1 {
    fee_record: Id,
    owner: Id,
    len: u8,
    intents: [Id; MAX_FEE_ROWS_V1],
    debit_atoms: [u64; MAX_FEE_ROWS_V1],
    total_debit_atoms: u64,
    next_carry: u128,
    carry_denominator: u128,
    boundary: AssessmentBoundaryV1,
}

impl PayerAllocationV1 {
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn owner(&self) -> Id {
        self.owner
    }

    pub const fn len(&self) -> u8 {
        self.len
    }

    pub const fn intents(&self) -> &[Id; MAX_FEE_ROWS_V1] {
        &self.intents
    }

    pub const fn debit_atoms(&self) -> &[u64; MAX_FEE_ROWS_V1] {
        &self.debit_atoms
    }

    pub const fn total_debit_atoms(&self) -> u64 {
        self.total_debit_atoms
    }

    pub const fn next_carry(&self) -> u128 {
        self.next_carry
    }

    pub const fn carry_denominator(&self) -> u128 {
        self.carry_denominator
    }

    pub const fn boundary(&self) -> AssessmentBoundaryV1 {
        self.boundary
    }
}

/// Allocate one owner-level fee across that owner's signed envelopes.
///
/// Envelopes must be strictly ordered by immutable intent identity.  The
/// canonical prefix fill is deterministic, never exceeds an individual
/// signature's bound, and cannot be changed by account-meta reordering.
pub fn allocate_payer_debit(
    assessment: &OwnerFeeAssessmentV1,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    len: u8,
) -> Result<PayerAllocationV1> {
    let owner = assessment.owner();
    let fee_atoms = assessment.charged_atoms();
    live(assessment.fee_record())?;
    live(owner)?;
    if len == 0 || usize::from(len) > MAX_FEE_ROWS_V1 {
        return Err(Error::InvalidWidth);
    }
    let mut prior = None;
    let mut capacity = 0u128;
    let mut index = 0usize;
    while index < usize::from(len) {
        let envelope = envelopes[index];
        if envelope.owner != owner {
            return Err(Error::MismatchedBinding);
        }
        live(envelope.intent)?;
        if let Some(id) = prior {
            if envelope.intent <= id {
                return Err(if envelope.intent == id {
                    Error::DuplicateIdentity
                } else {
                    Error::NonCanonicalOrder
                });
            }
        }
        if envelope.debited_atoms > envelope.max_fee_atoms {
            return Err(Error::FeeEnvelopeExceeded);
        }
        if envelope.funding == FeeEnvelopeFundingV1::NoCashReservation
            && (envelope.max_fee_atoms != 0 || envelope.debited_atoms != 0)
        {
            return Err(Error::SellerFeeForbidden);
        }
        capacity = capacity
            .checked_add(u128::from(envelope.max_fee_atoms - envelope.debited_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        prior = Some(envelope.intent);
        index += 1;
    }
    if capacity < u128::from(fee_atoms) {
        return Err(Error::FeeEnvelopeExceeded);
    }

    let mut intents = [Id([0u8; 32]); MAX_FEE_ROWS_V1];
    let mut output = [0u64; MAX_FEE_ROWS_V1];
    let mut remaining = fee_atoms;
    index = 0;
    while index < usize::from(len) {
        let headroom = envelopes[index].max_fee_atoms - envelopes[index].debited_atoms;
        let debit = core::cmp::min(headroom, remaining);
        intents[index] = envelopes[index].intent;
        output[index] = debit;
        remaining -= debit;
        index += 1;
    }
    if remaining != 0 {
        return Err(Error::ConservationFailure);
    }
    Ok(PayerAllocationV1 {
        fee_record: assessment.fee_record(),
        owner,
        len,
        intents,
        debit_atoms: output,
        total_debit_atoms: fee_atoms,
        next_carry: assessment.next_carry(),
        carry_denominator: assessment.denominator(),
        boundary: assessment.boundary(),
    })
}

/// One verified standing maker and its exact allocation weight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandingMakerRowV1 {
    pub position: Id,
    pub verified_weight: u64,
}

/// Exact recipient allocation for one collected fee.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientAllocationV1 {
    fee_record: Id,
    maker_len: u8,
    maker_positions: [Id; MAX_FEE_ROWS_V1],
    maker_rebate_atoms: [u64; MAX_FEE_ROWS_V1],
    maker_rebate_total: u64,
    executor_atoms: u64,
    treasury_atoms: u64,
    collected_fee_atoms: u64,
}

impl RecipientAllocationV1 {
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn maker_len(&self) -> u8 {
        self.maker_len
    }

    pub const fn maker_positions(&self) -> &[Id; MAX_FEE_ROWS_V1] {
        &self.maker_positions
    }

    pub const fn maker_rebate_atoms(&self) -> &[u64; MAX_FEE_ROWS_V1] {
        &self.maker_rebate_atoms
    }

    pub const fn maker_rebate_total(&self) -> u64 {
        self.maker_rebate_total
    }

    pub const fn executor_atoms(&self) -> u64 {
        self.executor_atoms
    }

    pub const fn treasury_atoms(&self) -> u64 {
        self.treasury_atoms
    }

    pub const fn collected_fee_atoms(&self) -> u64 {
        self.collected_fee_atoms
    }
}

/// Split one fee and distribute the maker pool by Hamilton largest remainder
/// over verified standing weights, ties by ascending Position identity.
pub fn allocate_recipients(
    selected: &SelectedCompositeFeeV1,
    policy: &RevenuePolicyV1,
    makers: &[StandingMakerRowV1; MAX_FEE_ROWS_V1],
    maker_len: u8,
    collected_fee_atoms: u64,
) -> Result<RecipientAllocationV1> {
    let fee_record = selected.fee_record();
    live(fee_record)?;
    selected.binds_revenue_policy(policy)?;
    policy.validate().map_err(|_| Error::InvalidPolicy)?;
    if policy.standing_maker != StandingMakerV1::AllRestingMakers {
        return Err(Error::InvalidPolicy);
    }
    let split = policy
        .allocate_split(collected_fee_atoms)
        .map_err(|_| Error::InvalidPolicy)?;
    if split.maker_rebate_atoms != 0 && maker_len == 0 {
        return Err(Error::EmptyAllocation);
    }
    if usize::from(maker_len) > MAX_FEE_ROWS_V1 {
        return Err(Error::InvalidWidth);
    }

    let mut total_weight = 0u128;
    let mut prior = None;
    let mut index = 0usize;
    while index < usize::from(maker_len) {
        let row = makers[index];
        live(row.position)?;
        if row.verified_weight == 0 {
            return Err(Error::EmptyAllocation);
        }
        if let Some(id) = prior {
            if row.position <= id {
                return Err(if row.position == id {
                    Error::DuplicateIdentity
                } else {
                    Error::NonCanonicalOrder
                });
            }
        }
        total_weight = total_weight
            .checked_add(u128::from(row.verified_weight))
            .ok_or(Error::ArithmeticOverflow)?;
        prior = Some(row.position);
        index += 1;
    }

    let mut positions = [Id([0u8; 32]); MAX_FEE_ROWS_V1];
    let mut output = [0u64; MAX_FEE_ROWS_V1];
    let mut remainders = [0u128; MAX_FEE_ROWS_V1];
    let mut assigned = 0u64;
    if split.maker_rebate_atoms != 0 {
        index = 0;
        while index < usize::from(maker_len) {
            let numerator = u128::from(split.maker_rebate_atoms)
                .checked_mul(u128::from(makers[index].verified_weight))
                .ok_or(Error::ArithmeticOverflow)?;
            output[index] =
                u64::try_from(numerator / total_weight).map_err(|_| Error::AmountOutOfRange)?;
            remainders[index] = numerator % total_weight;
            assigned = add(assigned, output[index])?;
            index += 1;
        }
        let mut dust = split
            .maker_rebate_atoms
            .checked_sub(assigned)
            .ok_or(Error::ConservationFailure)?;
        while dust != 0 {
            let mut best = None;
            index = 0;
            while index < usize::from(maker_len) {
                if remainders[index] != 0
                    && best.map_or(true, |current: usize| {
                        remainders[index] > remainders[current]
                            || (remainders[index] == remainders[current]
                                && makers[index].position < makers[current].position)
                    })
                {
                    best = Some(index);
                }
                index += 1;
            }
            let best = best.ok_or(Error::ConservationFailure)?;
            output[best] = add(output[best], 1)?;
            remainders[best] = 0;
            dust -= 1;
        }
    }

    let mut maker_sum = 0u64;
    index = 0;
    while index < usize::from(maker_len) {
        positions[index] = makers[index].position;
        maker_sum = add(maker_sum, output[index])?;
        index += 1;
    }
    if maker_sum != split.maker_rebate_atoms
        || add(add(maker_sum, split.executor_atoms)?, split.treasury_atoms)? != collected_fee_atoms
    {
        return Err(Error::ConservationFailure);
    }
    Ok(RecipientAllocationV1 {
        fee_record,
        maker_len,
        maker_positions: positions,
        maker_rebate_atoms: output,
        maker_rebate_total: maker_sum,
        executor_atoms: split.executor_atoms,
        treasury_atoms: split.treasury_atoms,
        collected_fee_atoms,
    })
}
