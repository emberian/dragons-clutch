//! Exact selected-execution fee weights and final-atom Hamilton allocation.
//!
//! This successor deliberately does not accept consideration, posted size, or
//! a pre-rounded fee amount as a weight.  Its sole row measure is the exact
//! `u128` `CompositeDispersionFloor` base numerator already owned by
//! `clutch-batch::relation_v1::composite_fee_quote`, evaluated over one
//! owner's complete selected executed payoff under the same rates and common
//! denominator used for fee charging.

use sha2::{Digest, Sha256};

use crate::{live, Error, Id, Result, MAX_FEE_ROWS_V1};

/// Fresh semantic version for selected-execution fee weighting.
pub const COMPOSITE_FEE_WEIGHT_POLICY_VERSION_V2: u16 = 2;
/// Exact canonical policy byte width.
pub const COMPOSITE_FEE_WEIGHT_POLICY_BYTES_V2: usize = 16;
/// Domain separating the immutable V2 policy identity.
pub const COMPOSITE_FEE_WEIGHT_POLICY_DOMAIN_V2: &[u8] =
    b"dragons-clutch/composite-fee-weight-policy/v2\0";
/// Domain separating one complete selected-execution weight stream.
pub const COMPOSITE_FEE_WEIGHT_TRANSCRIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/composite-fee-weight-transcript/v2\0";

const COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2: [u8; 8] = *b"DCFWEV2\0";

/// Sole exact measure accepted by the V2 policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeFeeWeightMeasureV2 {
    /// Owner-wide `FeeQuoteV1::base_numerator` at zero carry.
    OwnerNettedCompositeBaseNumerator,
}

/// Eligibility predicate for a V2 weight row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeFeeWeightEligibilityV2 {
    /// Ordinary Position V3 owners with nonzero selected executed weight.
    SelectedExecutedOrdinaryPosition,
}

/// Self-cross treatment frozen by this weight policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeFeeWeightSelfCrossV2 {
    /// Refuse same-owner buy/sell overlap in any payoff coordinate.
    RefuseOwnerOutcomeOverlap,
}

/// The only rounding boundary in recipient weighting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeFeeWeightRoundingV2 {
    /// Preserve exact weights until Hamilton assigns final collateral atoms.
    HamiltonFinalAtom,
}

/// Immutable V2 weight policy.
///
/// The fields are intentionally not configurable. A different measure,
/// eligibility rule, zero treatment, row order, self-cross rule, or rounding
/// boundary requires another semantic version and identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeFeeWeightPolicyV2 {
    _private: (),
}

/// Sole current V2 member.
pub const COMPOSITE_FEE_WEIGHT_POLICY_V2: CompositeFeeWeightPolicyV2 =
    CompositeFeeWeightPolicyV2 { _private: () };

impl CompositeFeeWeightPolicyV2 {
    /// Fresh immutable policy version.
    pub const fn version(self) -> u16 { COMPOSITE_FEE_WEIGHT_POLICY_VERSION_V2 }
    /// Sole exact row measure.
    pub const fn measure(self) -> CompositeFeeWeightMeasureV2 {
        CompositeFeeWeightMeasureV2::OwnerNettedCompositeBaseNumerator
    }
    /// Sole eligible account class and execution predicate.
    pub const fn eligibility(self) -> CompositeFeeWeightEligibilityV2 {
        CompositeFeeWeightEligibilityV2::SelectedExecutedOrdinaryPosition
    }
    /// Same-owner cross behavior required of the selected book.
    pub const fn self_cross(self) -> CompositeFeeWeightSelfCrossV2 {
        CompositeFeeWeightSelfCrossV2::RefuseOwnerOutcomeOverlap
    }
    /// Sole permitted recipient-allocation rounding boundary.
    pub const fn rounding(self) -> CompositeFeeWeightRoundingV2 {
        CompositeFeeWeightRoundingV2::HamiltonFinalAtom
    }

    /// Unique canonical policy bytes. Zero weights are omitted and active
    /// rows are ordered by Position identity; both rules are explicit bytes.
    pub const fn encode(self) -> [u8; COMPOSITE_FEE_WEIGHT_POLICY_BYTES_V2] {
        let version = COMPOSITE_FEE_WEIGHT_POLICY_VERSION_V2.to_le_bytes();
        [
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[0],
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[1],
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[2],
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[3],
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[4],
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[5],
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[6],
            COMPOSITE_FEE_WEIGHT_POLICY_MAGIC_V2[7],
            version[0],
            version[1],
            1, // owner-netted CompositeDispersionFloor base numerator
            1, // selected executed ordinary Position
            1, // omit zero
            1, // Position identity ascending
            1, // refuse same-owner opposite-side overlap per payoff outcome
            1, // Hamilton at the final collateral-atom boundary
        ]
    }

    /// Domain-separated immutable policy identity.
    pub fn id(self) -> Result<Id> {
        let mut hash = Sha256::new();
        hash.update(COMPOSITE_FEE_WEIGHT_POLICY_DOMAIN_V2);
        hash.update(self.encode());
        let id = Id(hash.finalize().into());
        live(id)?;
        Ok(id)
    }
}

/// One exact nonzero Position weight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeFeeWeightRowV2 {
    position: Id,
    exact_numerator: u128,
}

/// Compact canonical transcript of one complete exact weight stream.
///
/// The transcript contains no row array. An authenticated adapter may retain
/// only this summary and reproduce Position-sorted rows from its immutable
/// account borrows, avoiding a maximum-width by-value SBF frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeFeeWeightTranscriptV2 {
    policy_id: Id,
    fee_record: Id,
    common_denominator: u128,
    len: u8,
    total_weight: u128,
    transcript_id: Id,
}

impl CompositeFeeWeightTranscriptV2 {
    /// Immutable V2 weighting-policy identity.
    pub const fn policy_id(self) -> Id { self.policy_id }
    /// Existing selected composite-fee semantic owner.
    pub const fn fee_record(self) -> Id { self.fee_record }
    /// Exact common denominator used by fee charging.
    pub const fn common_denominator(self) -> u128 { self.common_denominator }
    /// Number of nonzero, Position-sorted rows.
    pub const fn len(self) -> u8 { self.len }
    /// Exact sum of all unrounded `u128` row numerators.
    pub const fn total_weight(self) -> u128 { self.total_weight }
    /// Domain-separated identity of the complete canonical stream.
    pub const fn transcript_id(self) -> Id { self.transcript_id }
}

/// Commit one complete Position-sorted stream without materializing its rows.
///
/// `next` is structural input: it must return the least remaining row after
/// the supplied Position. Live authentication belongs to the traversal-backed
/// capability that invokes this constructor and retains the account borrows.
pub fn composite_fee_weight_transcript_v2<F>(
    fee_record: Id,
    common_denominator: u128,
    mut next: F,
) -> Result<CompositeFeeWeightTranscriptV2>
where
    F: FnMut(Option<Id>) -> Result<Option<CompositeFeeWeightRowV2>>,
{
    live(fee_record)?;
    if common_denominator == 0 {
        return Err(Error::InvalidWidth);
    }
    let policy_id = COMPOSITE_FEE_WEIGHT_POLICY_V2.id()?;
    let mut hash = Sha256::new();
    hash.update(COMPOSITE_FEE_WEIGHT_TRANSCRIPT_DOMAIN_V2);
    hash.update(policy_id.0);
    hash.update(fee_record.0);
    hash.update(common_denominator.to_le_bytes());
    let mut prior = None;
    let mut len = 0u8;
    let mut total_weight = 0u128;
    loop {
        let Some(row) = next(prior)? else { break };
        live(row.position)?;
        if row.exact_numerator == 0 {
            return Err(Error::EmptyAllocation);
        }
        if prior.is_some_and(|position| row.position <= position) {
            return Err(if prior == Some(row.position) {
                Error::DuplicateIdentity
            } else {
                Error::NonCanonicalOrder
            });
        }
        len = len.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        if usize::from(len) > MAX_FEE_ROWS_V1 {
            return Err(Error::InvalidWidth);
        }
        total_weight = total_weight
            .checked_add(row.exact_numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        hash.update(row.position.0);
        hash.update(row.exact_numerator.to_le_bytes());
        prior = Some(row.position);
    }
    hash.update([len]);
    hash.update(total_weight.to_le_bytes());
    let transcript_id = Id(hash.finalize().into());
    live(transcript_id)?;
    Ok(CompositeFeeWeightTranscriptV2 {
        policy_id,
        fee_record,
        common_denominator,
        len,
        total_weight,
        transcript_id,
    })
}

impl CompositeFeeWeightRowV2 {
    /// Canonical zero padding row for structural fixed-capacity books only.
    pub const EMPTY: Self = Self { position: Id([0; 32]), exact_numerator: 0 };

    /// Untrusted structural row. Authentication belongs to the traversal
    /// constructor; this constructor only enforces local nonzero shape.
    pub fn structural(position: Id, exact_numerator: u128) -> Result<Self> {
        live(position)?;
        if exact_numerator == 0 {
            return Err(Error::EmptyAllocation);
        }
        Ok(Self { position, exact_numerator })
    }

    /// Ordinary Position account identity used for canonical ordering.
    pub const fn position(self) -> Id { self.position }
    /// Unrounded owner-wide CompositeDispersionFloor numerator.
    pub const fn exact_numerator(self) -> u128 { self.exact_numerator }
}

/// Canonical selected-execution structural weight book.
///
/// This fixed-capacity value is useful for pure fixtures and offline
/// allocation. It does not authenticate accounts and is deliberately not
/// embedded in the live General V5 SBF capability, which retains a compact
/// traversal-backed stream instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeFeeWeightBookV2 {
    policy_id: Id,
    fee_record: Id,
    common_denominator: u128,
    len: u8,
    rows: [CompositeFeeWeightRowV2; MAX_FEE_ROWS_V1],
    total_weight: u128,
    transcript_id: Id,
}

impl CompositeFeeWeightBookV2 {
    /// Immutable V2 weighting-policy identity.
    pub const fn policy_id(&self) -> Id { self.policy_id }
    /// Existing selected composite-fee semantic owner.
    pub const fn fee_record(&self) -> Id { self.fee_record }
    /// Exact common denominator used by fee charging.
    pub const fn common_denominator(&self) -> u128 { self.common_denominator }
    /// Number of nonzero active rows.
    pub const fn len(&self) -> u8 { self.len }
    /// Exact sum of all active `u128` row numerators.
    pub const fn total_weight(&self) -> u128 { self.total_weight }
    /// Domain-separated identity of this complete book.
    pub const fn transcript_id(&self) -> Id { self.transcript_id }
    /// Checked active row, or `None` for padding/out-of-range indices.
    pub fn row(&self, index: u8) -> Option<CompositeFeeWeightRowV2> {
        if index < self.len { Some(self.rows[usize::from(index)]) } else { None }
    }
}

/// Validate one canonical Position-sorted, zero-omitting structural book.
pub fn canonical_composite_fee_weight_book_v2(
    fee_record: Id,
    common_denominator: u128,
    rows: [CompositeFeeWeightRowV2; MAX_FEE_ROWS_V1],
    len: u8,
) -> Result<CompositeFeeWeightBookV2> {
    live(fee_record)?;
    if common_denominator == 0 || usize::from(len) > MAX_FEE_ROWS_V1 {
        return Err(Error::InvalidWidth);
    }
    let policy_id = COMPOSITE_FEE_WEIGHT_POLICY_V2.id()?;
    let mut total_weight = 0u128;
    let mut prior = None;
    let mut index = 0usize;
    while index < usize::from(len) {
        let row = rows[index];
        live(row.position)?;
        if row.exact_numerator == 0 {
            return Err(Error::EmptyAllocation);
        }
        if let Some(previous) = prior {
            if row.position <= previous {
                return Err(if row.position == previous {
                    Error::DuplicateIdentity
                } else {
                    Error::NonCanonicalOrder
                });
            }
        }
        total_weight = total_weight
            .checked_add(row.exact_numerator)
            .ok_or(Error::ArithmeticOverflow)?;
        prior = Some(row.position);
        index += 1;
    }
    while index < MAX_FEE_ROWS_V1 {
        if rows[index] != CompositeFeeWeightRowV2::EMPTY {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    if (len == 0) != (total_weight == 0) {
        return Err(Error::EmptyAllocation);
    }

    let mut stream_index = 0usize;
    let transcript = composite_fee_weight_transcript_v2(
        fee_record,
        common_denominator,
        |_| {
            if stream_index < usize::from(len) {
                let row = rows[stream_index];
                stream_index += 1;
                Ok(Some(row))
            } else {
                Ok(None)
            }
        },
    )?;
    if transcript.policy_id != policy_id
        || transcript.len != len
        || transcript.total_weight != total_weight
    {
        return Err(Error::ConservationFailure);
    }
    Ok(CompositeFeeWeightBookV2 {
        policy_id,
        fee_record,
        common_denominator,
        len,
        rows,
        total_weight,
        transcript_id: transcript.transcript_id,
    })
}

/// One exact final-atom Hamilton allocation over a certified weight book.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeFeeHamiltonAllocationV2 {
    weight_transcript_id: Id,
    len: u8,
    positions: [Id; MAX_FEE_ROWS_V1],
    atoms: [u64; MAX_FEE_ROWS_V1],
    total_atoms: u64,
}

/// Exact floor share and remainder for one row before Hamilton's final atom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositeFeeHamiltonShareV2 {
    floor_atoms: u64,
    remainder: u128,
}

impl CompositeFeeHamiltonShareV2 {
    /// Exact floor before the final-atom ranking boundary.
    pub const fn floor_atoms(self) -> u64 { self.floor_atoms }
    /// Exact remainder ranked descending, with Position identity as tie-break.
    pub const fn remainder(self) -> u128 { self.remainder }
}

/// Derive one exact Hamilton floor/remainder pair without narrowing a weight.
///
/// The authenticated streaming adapter uses this primitive in repeated scans;
/// no maximum-width allocation array is required on its SBF frame.
pub fn composite_fee_hamilton_share_v2(
    total_atoms: u64,
    exact_weight: u128,
    total_weight: u128,
) -> Result<CompositeFeeHamiltonShareV2> {
    if exact_weight == 0 || total_weight == 0 || exact_weight > total_weight {
        return Err(Error::InvalidWidth);
    }
    let (floor_atoms, remainder) =
        mul_u64_u128_div_rem(total_atoms, exact_weight, total_weight)?;
    Ok(CompositeFeeHamiltonShareV2 { floor_atoms, remainder })
}

impl CompositeFeeHamiltonAllocationV2 {
    /// Exact source weight transcript.
    pub const fn weight_transcript_id(&self) -> Id { self.weight_transcript_id }
    /// Number of active allocated Position rows.
    pub const fn len(&self) -> u8 { self.len }
    /// Position identities followed by zero padding.
    pub const fn positions(&self) -> &[Id; MAX_FEE_ROWS_V1] { &self.positions }
    /// Final allocated atoms followed by zero padding.
    pub const fn atoms(&self) -> &[u64; MAX_FEE_ROWS_V1] { &self.atoms }
    /// Conserved input atom pool.
    pub const fn total_atoms(&self) -> u64 { self.total_atoms }
}

/// Allocate a final collateral-atom pool without ever narrowing or
/// pre-normalizing an exact `u128` row weight.
pub fn allocate_composite_fee_weight_atoms_v2(
    book: &CompositeFeeWeightBookV2,
    total_atoms: u64,
) -> Result<CompositeFeeHamiltonAllocationV2> {
    if total_atoms != 0 && book.len == 0 {
        return Err(Error::EmptyAllocation);
    }
    let mut positions = [Id([0; 32]); MAX_FEE_ROWS_V1];
    let mut atoms = [0u64; MAX_FEE_ROWS_V1];
    let mut remainders = [0u128; MAX_FEE_ROWS_V1];
    let mut assigned = 0u64;
    let mut index = 0usize;
    while index < usize::from(book.len) {
        let row = book.rows[index];
        positions[index] = row.position;
        let share = composite_fee_hamilton_share_v2(
            total_atoms,
            row.exact_numerator,
            book.total_weight,
        )?;
        atoms[index] = share.floor_atoms;
        remainders[index] = share.remainder;
        assigned = assigned
            .checked_add(share.floor_atoms)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    let mut dust = total_atoms
        .checked_sub(assigned)
        .ok_or(Error::ConservationFailure)?;
    while dust != 0 {
        let mut best = None;
        index = 0;
        while index < usize::from(book.len) {
            if remainders[index] != 0
                && best.map_or(true, |current: usize| {
                    remainders[index] > remainders[current]
                        || (remainders[index] == remainders[current]
                            && positions[index] < positions[current])
                })
            {
                best = Some(index);
            }
            index += 1;
        }
        let best = best.ok_or(Error::ConservationFailure)?;
        atoms[best] = atoms[best].checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        remainders[best] = 0;
        dust -= 1;
    }
    let mut observed = 0u64;
    index = 0;
    while index < usize::from(book.len) {
        observed = observed.checked_add(atoms[index]).ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if observed != total_atoms {
        return Err(Error::ConservationFailure);
    }
    Ok(CompositeFeeHamiltonAllocationV2 {
        weight_transcript_id: book.transcript_id,
        len: book.len,
        positions,
        atoms,
        total_atoms,
    })
}

/// Exact `(left * weight) / denominator` for `u64 * u128` without a wider
/// primitive or overflow. The quotient is at most `left` because
/// `weight <= denominator` for every row in a total-weight book.
fn mul_u64_u128_div_rem(
    left: u64,
    weight: u128,
    denominator: u128,
) -> Result<(u64, u128)> {
    if denominator == 0 || weight > denominator {
        return Err(Error::InvalidWidth);
    }
    let mut quotient = 0u64;
    let mut remainder = 0u128;
    let mut bit = 64u32;
    while bit != 0 {
        bit -= 1;
        quotient = quotient.checked_mul(2).ok_or(Error::ArithmeticOverflow)?;
        let mut digit = 0u64;
        if remainder >= denominator - remainder {
            remainder -= denominator - remainder;
            digit = 1;
        } else {
            remainder += remainder;
        }
        if ((left >> bit) & 1) != 0 {
            if remainder >= denominator - weight {
                remainder -= denominator - weight;
                digit = digit.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            } else {
                remainder += weight;
            }
        }
        quotient = quotient.checked_add(digit).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok((quotient, remainder))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id { Id([byte; 32]) }

    #[test]
    fn policy_bytes_name_one_measure_order_and_rounding_boundary() {
        let bytes = COMPOSITE_FEE_WEIGHT_POLICY_V2.encode();
        assert_eq!(&bytes[..8], b"DCFWEV2\0");
        assert_eq!(&bytes[8..10], &2u16.to_le_bytes());
        assert_eq!(&bytes[10..], &[1, 1, 1, 1, 1, 1]);
        assert!(!COMPOSITE_FEE_WEIGHT_POLICY_V2.id().unwrap().is_zero());
    }

    #[test]
    fn u128_weights_reach_hamilton_without_intermediate_narrowing() {
        let mut rows = [CompositeFeeWeightRowV2::EMPTY; MAX_FEE_ROWS_V1];
        rows[0] = CompositeFeeWeightRowV2::structural(id(1), u128::MAX - 1).unwrap();
        rows[1] = CompositeFeeWeightRowV2::structural(id(2), 1).unwrap();
        let book = canonical_composite_fee_weight_book_v2(id(9), 77, rows, 2).unwrap();
        assert_eq!(book.total_weight(), u128::MAX);
        let allocation = allocate_composite_fee_weight_atoms_v2(&book, u64::MAX).unwrap();
        assert_eq!(allocation.atoms()[0], u64::MAX);
        assert_eq!(allocation.atoms()[1], 0);
        assert_eq!(allocation.total_atoms(), u64::MAX);
    }

    #[test]
    fn equal_remainders_break_by_ascending_position() {
        let mut rows = [CompositeFeeWeightRowV2::EMPTY; MAX_FEE_ROWS_V1];
        rows[0] = CompositeFeeWeightRowV2::structural(id(1), 1).unwrap();
        rows[1] = CompositeFeeWeightRowV2::structural(id(2), 1).unwrap();
        let book = canonical_composite_fee_weight_book_v2(id(9), 5, rows, 2).unwrap();
        let allocation = allocate_composite_fee_weight_atoms_v2(&book, 1).unwrap();
        assert_eq!(&allocation.atoms()[..2], &[1, 0]);
    }

    #[test]
    fn streamed_transcript_matches_the_structural_book_without_a_row_copy() {
        let mut rows = [CompositeFeeWeightRowV2::EMPTY; MAX_FEE_ROWS_V1];
        rows[0] = CompositeFeeWeightRowV2::structural(id(1), u128::MAX - 7).unwrap();
        rows[1] = CompositeFeeWeightRowV2::structural(id(9), 7).unwrap();
        let book = canonical_composite_fee_weight_book_v2(id(8), 11, rows, 2).unwrap();
        let transcript = composite_fee_weight_transcript_v2(id(8), 11, |prior| {
            Ok(match prior {
                None => Some(rows[0]),
                Some(position) if position == rows[0].position() => Some(rows[1]),
                Some(position) if position == rows[1].position() => None,
                Some(_) => return Err(Error::MismatchedBinding),
            })
        }).unwrap();
        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript.total_weight(), u128::MAX);
        assert_eq!(transcript.transcript_id(), book.transcript_id());
    }

    #[test]
    fn streamed_transcript_refuses_a_nonadvancing_position() {
        let row = CompositeFeeWeightRowV2::structural(id(1), 5).unwrap();
        assert_eq!(
            composite_fee_weight_transcript_v2(id(8), 11, |prior| {
                Ok(if prior.is_none() { Some(row) } else { Some(row) })
            }),
            Err(Error::DuplicateIdentity)
        );
    }

    #[test]
    fn zero_unsorted_duplicate_and_nonzero_tail_rows_refuse() {
        assert_eq!(
            CompositeFeeWeightRowV2::structural(id(1), 0),
            Err(Error::EmptyAllocation)
        );
        let mut unsorted = [CompositeFeeWeightRowV2::EMPTY; MAX_FEE_ROWS_V1];
        unsorted[0] = CompositeFeeWeightRowV2::structural(id(2), 1).unwrap();
        unsorted[1] = CompositeFeeWeightRowV2::structural(id(1), 1).unwrap();
        assert_eq!(
            canonical_composite_fee_weight_book_v2(id(9), 5, unsorted, 2),
            Err(Error::NonCanonicalOrder)
        );
        let mut tail = [CompositeFeeWeightRowV2::EMPTY; MAX_FEE_ROWS_V1];
        tail[1] = CompositeFeeWeightRowV2::structural(id(2), 1).unwrap();
        assert_eq!(
            canonical_composite_fee_weight_book_v2(id(9), 5, tail, 0),
            Err(Error::NonCanonicalPadding)
        );
    }
}
