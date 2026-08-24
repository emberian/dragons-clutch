//! Canonical payer-envelope and recipient allocation.

use clutch_batch::exact_integer::exact_mul_div_rem;
use clutch_batch_policy_identity::revenue_policy_v1::{RevenuePolicyV1, StandingMakerV1};
use clutch_batch_policy_identity::revenue_policy_v2::{
    MakerWeightAuthorityV2, RevenuePolicyV2,
};

use crate::selected::{
    AssessmentBoundaryV1, OwnerFeeAssessmentV1, SelectedCompositeFeeV1,
    SelectedCompositeFeeV2,
};
use crate::weight_v2::{
    composite_fee_hamilton_share_v2, composite_fee_weight_transcript_v2,
    CompositeFeeWeightRowV2, CompositeFeeWeightTranscriptV2, COMPOSITE_FEE_WEIGHT_POLICY_V2,
};
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore_persisted(
        fee_record: Id,
        owner: Id,
        len: u8,
        intents: [Id; MAX_FEE_ROWS_V1],
        debit_atoms: [u64; MAX_FEE_ROWS_V1],
        total_debit_atoms: u64,
        next_carry: u128,
        carry_denominator: u128,
        boundary: AssessmentBoundaryV1,
    ) -> Result<Self> {
        live(fee_record)?;
        live(owner)?;
        if len == 0
            || usize::from(len) > MAX_FEE_ROWS_V1
            || carry_denominator == 0
            || next_carry >= carry_denominator
            || (boundary == AssessmentBoundaryV1::TerminalCeil && next_carry != 0)
        {
            return Err(Error::InvalidAccountData);
        }
        let mut total = 0u64;
        let mut prior = None;
        let mut index = 0usize;
        while index < usize::from(len) {
            live(intents[index])?;
            if let Some(previous) = prior {
                if intents[index] <= previous {
                    return Err(if intents[index] == previous {
                        Error::DuplicateIdentity
                    } else {
                        Error::NonCanonicalOrder
                    });
                }
            }
            total = add(total, debit_atoms[index])?;
            prior = Some(intents[index]);
            index += 1;
        }
        while index < MAX_FEE_ROWS_V1 {
            if intents[index] != Id([0; 32]) || debit_atoms[index] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if total != total_debit_atoms {
            return Err(Error::ConservationFailure);
        }
        Ok(Self {
            fee_record,
            owner,
            len,
            intents,
            debit_atoms,
            total_debit_atoms,
            next_carry,
            carry_denominator,
            boundary,
        })
    }

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

/// Compact constructor-authenticated header for the current streaming
/// recipient encoder. Maker rows are supplied only by the private traversal
/// adapter and are checked again for canonical order and conservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CertifiedRecipientAllocationHeaderV2 {
    fee_record: Id,
    maker_len: u8,
    maker_rebate_total: u64,
    executor_atoms: u64,
    treasury_atoms: u64,
    collected_fee_atoms: u64,
    owner_fee_book_data_id: Id,
    owner_order_set_digest: Id,
    owner_count: u16,
}

impl CertifiedRecipientAllocationHeaderV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        selected: &SelectedCompositeFeeV2,
        policy: &RevenuePolicyV2,
        collected_fee_atoms: u64,
        maker_len: u8,
        owner_fee_book_data_id: Id,
        owner_order_set_digest: Id,
        owner_count: u16,
    ) -> Result<Self> {
        selected.binds_revenue_policy(policy)?;
        live(owner_fee_book_data_id)?;
        live(owner_order_set_digest)?;
        if collected_fee_atoms == 0
            || usize::from(maker_len) > MAX_FEE_ROWS_V1
            || owner_count == 0
            || usize::from(owner_count) > MAX_FEE_ROWS_V1
            || u16::from(maker_len) > owner_count
        {
            return Err(Error::InvalidWidth);
        }
        let split = policy
            .allocate_split(collected_fee_atoms)
            .map_err(|_| Error::InvalidPolicy)?;
        if split.executor_atoms != 0
            || (split.maker_rebate_atoms != 0 && maker_len == 0)
        {
            return Err(Error::EmptyAllocation);
        }
        Ok(Self {
            fee_record: selected.fee_record(),
            maker_len,
            maker_rebate_total: split.maker_rebate_atoms,
            executor_atoms: split.executor_atoms,
            treasury_atoms: split.treasury_atoms,
            collected_fee_atoms,
            owner_fee_book_data_id,
            owner_order_set_digest,
            owner_count,
        })
    }

    pub const fn fee_record(&self) -> Id { self.fee_record }
    pub const fn maker_len(&self) -> u8 { self.maker_len }
    pub const fn maker_rebate_total(&self) -> u64 { self.maker_rebate_total }
    pub const fn executor_atoms(&self) -> u64 { self.executor_atoms }
    pub const fn treasury_atoms(&self) -> u64 { self.treasury_atoms }
    pub const fn collected_fee_atoms(&self) -> u64 { self.collected_fee_atoms }
    pub const fn owner_fee_book_data_id(&self) -> Id { self.owner_fee_book_data_id }
    pub const fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    pub const fn owner_count(&self) -> u16 { self.owner_count }
}

/// One Hamilton base quotient/remainder over full u128 certified numerators.
/// This is the only allocation division boundary before residual atoms are
/// assigned by descending remainder and ascending Position identity.
pub fn certified_maker_floor_v2(
    maker_pool_atoms: u64,
    owner_numerator: u128,
    total_numerator: u128,
) -> Result<(u64, u128)> {
    if owner_numerator == 0 || total_numerator == 0 || owner_numerator > total_numerator {
        return Err(Error::EmptyAllocation);
    }
    let (quotient, remainder) = exact_mul_div_rem(
        u128::from(maker_pool_atoms),
        owner_numerator,
        total_numerator,
    )
    .map_err(|_| Error::ArithmeticOverflow)?;
    Ok((
        u64::try_from(quotient).map_err(|_| Error::AmountOutOfRange)?,
        remainder,
    ))
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

    /// Restore an immutable program-owned snapshot from exact persisted rows.
    ///
    /// This validates canonical ordering, padding, and conservation only. It
    /// does not re-prove maker weights or fee-book completeness; the adapter
    /// must authenticate the fresh certified outer account whose creation
    /// consumed those stronger capabilities.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore_persisted(
        fee_record: Id,
        maker_len: u8,
        maker_positions: [Id; MAX_FEE_ROWS_V1],
        maker_rebate_atoms: [u64; MAX_FEE_ROWS_V1],
        maker_rebate_total: u64,
        executor_atoms: u64,
        treasury_atoms: u64,
        collected_fee_atoms: u64,
    ) -> Result<Self> {
        live(fee_record)?;
        if usize::from(maker_len) > MAX_FEE_ROWS_V1 {
            return Err(Error::InvalidWidth);
        }
        let mut maker_sum = 0u64;
        let mut index = 0usize;
        while index < usize::from(maker_len) {
            live(maker_positions[index])?;
            if index != 0 && maker_positions[index] <= maker_positions[index - 1] {
                return Err(Error::NonCanonicalOrder);
            }
            maker_sum = add(maker_sum, maker_rebate_atoms[index])?;
            index += 1;
        }
        while index < MAX_FEE_ROWS_V1 {
            if !maker_positions[index].is_zero() || maker_rebate_atoms[index] != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if maker_sum != maker_rebate_total
            || add(add(maker_sum, executor_atoms)?, treasury_atoms)? != collected_fee_atoms
        {
            return Err(Error::ConservationFailure);
        }
        Ok(Self {
            fee_record,
            maker_len,
            maker_positions,
            maker_rebate_atoms,
            maker_rebate_total,
            executor_atoms,
            treasury_atoms,
            collected_fee_atoms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certified_floor_is_exact_at_u128_boundary() {
        assert_eq!(
            certified_maker_floor_v2(u64::MAX, u128::MAX, u128::MAX),
            Ok((u64::MAX, 0))
        );
        assert_eq!(
            certified_maker_floor_v2(u64::MAX, u128::MAX - 1, u128::MAX),
            Ok((u64::MAX - 1, u128::MAX - u128::from(u64::MAX)))
        );
    }

    #[test]
    fn certified_floor_refuses_uncertified_weight_domain() {
        assert_eq!(
            certified_maker_floor_v2(10, 0, 10),
            Err(Error::EmptyAllocation)
        );
        assert_eq!(
            certified_maker_floor_v2(10, 11, 10),
            Err(Error::EmptyAllocation)
        );
        assert_eq!(
            certified_maker_floor_v2(10, 1, 0),
            Err(Error::EmptyAllocation)
        );
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

/// Allocate recipients from the exact V2 selected-execution weight stream.
///
/// The callback is replayed rather than retained. Every replay must reproduce
/// the transcript's complete Position-sorted, zero-omitting row sequence.
/// Collected fee atoms are derived as the sum of each row's terminal ceiling
/// under the transcript denominator; neither the fee total nor any recipient
/// row is accepted from the caller. The revenue policy owns only the split,
/// while V2 owns row eligibility, measure, ordering, and the sole Hamilton
/// final-atom boundary.
#[inline(never)]
pub fn allocate_recipients_from_weight_stream_v2<F>(
    selected: &SelectedCompositeFeeV2,
    policy: &RevenuePolicyV2,
    transcript: CompositeFeeWeightTranscriptV2,
    mut row_at: F,
) -> Result<RecipientAllocationV1>
where
    F: FnMut(u8) -> Result<Option<CompositeFeeWeightRowV2>>,
{
    selected.binds_revenue_policy(policy)?;
    policy.validate().map_err(|_| Error::InvalidPolicy)?;
    if policy.maker_weight_authority
        != MakerWeightAuthorityV2::CertifiedOwnerNettedCompositeNumerator
        || transcript.policy_id() != COMPOSITE_FEE_WEIGHT_POLICY_V2.id()?
        || transcript.fee_record() != selected.fee_record()
        || transcript.common_denominator() != selected.carry_denominator()
        || usize::from(transcript.len()) > MAX_FEE_ROWS_V1
    {
        return Err(Error::MismatchedBinding);
    }

    let mut stream_index = 0u8;
    let reproduced = composite_fee_weight_transcript_v2(
        selected.fee_record(),
        selected.carry_denominator(),
        |_| {
            let row = row_at(stream_index)?;
            if row.is_some() {
                stream_index = stream_index
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            Ok(row)
        },
    )?;
    if reproduced != transcript {
        return Err(Error::MismatchedBinding);
    }

    let mut collected_fee_atoms = 0u64;
    let mut index = 0u8;
    while index < transcript.len() {
        let row = row_at(index)?.ok_or(Error::MismatchedBinding)?;
        let quotient = row.exact_numerator() / transcript.common_denominator();
        let remainder = row.exact_numerator() % transcript.common_denominator();
        let floor = u64::try_from(quotient).map_err(|_| Error::AmountOutOfRange)?;
        let terminal = if remainder == 0 {
            floor
        } else {
            floor.checked_add(1).ok_or(Error::ArithmeticOverflow)?
        };
        collected_fee_atoms = add(collected_fee_atoms, terminal)?;
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    let split = policy
        .allocate_split(collected_fee_atoms)
        .map_err(|_| Error::InvalidPolicy)?;
    if transcript.len() == 0 {
        if transcript.total_weight() != 0
            || collected_fee_atoms != 0
            || split.maker_rebate_atoms != 0
            || split.executor_atoms != 0
            || split.treasury_atoms != 0
        {
            return Err(Error::ConservationFailure);
        }
        return RecipientAllocationV1::restore_persisted(
            selected.fee_record(),
            0,
            [Id([0; 32]); MAX_FEE_ROWS_V1],
            [0; MAX_FEE_ROWS_V1],
            0,
            0,
            0,
            0,
        );
    }

    let mut positions = [Id([0u8; 32]); MAX_FEE_ROWS_V1];
    let mut output = [0u64; MAX_FEE_ROWS_V1];
    let mut maker_sum = 0u64;
    index = 0;
    while index < transcript.len() {
        let target = row_at(index)?.ok_or(Error::MismatchedBinding)?;
        positions[usize::from(index)] = target.position();
        let target_share = composite_fee_hamilton_share_v2(
            split.maker_rebate_atoms,
            target.exact_numerator(),
            transcript.total_weight(),
        )?;

        let mut assigned = 0u64;
        let mut higher_ranked = 0u64;
        let mut cursor = 0u8;
        while cursor < transcript.len() {
            let row = row_at(cursor)?.ok_or(Error::MismatchedBinding)?;
            let share = composite_fee_hamilton_share_v2(
                split.maker_rebate_atoms,
                row.exact_numerator(),
                transcript.total_weight(),
            )?;
            assigned = add(assigned, share.floor_atoms())?;
            if share.remainder() > target_share.remainder()
                || (share.remainder() == target_share.remainder()
                    && row.position() < target.position())
            {
                higher_ranked = higher_ranked
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
            cursor = cursor.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        let dust = split
            .maker_rebate_atoms
            .checked_sub(assigned)
            .ok_or(Error::ConservationFailure)?;
        if dust > u64::from(transcript.len()) {
            return Err(Error::ConservationFailure);
        }
        let extra = if target_share.remainder() != 0 && higher_ranked < dust {
            1u64
        } else {
            0u64
        };
        let atoms = target_share
            .floor_atoms()
            .checked_add(extra)
            .ok_or(Error::ArithmeticOverflow)?;
        output[usize::from(index)] = atoms;
        maker_sum = add(maker_sum, atoms)?;
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    if maker_sum != split.maker_rebate_atoms
        || add(add(maker_sum, split.executor_atoms)?, split.treasury_atoms)?
            != collected_fee_atoms
    {
        return Err(Error::ConservationFailure);
    }
    RecipientAllocationV1::restore_persisted(
        selected.fee_record(),
        transcript.len(),
        positions,
        output,
        maker_sum,
        split.executor_atoms,
        split.treasury_atoms,
        collected_fee_atoms,
    )
}
