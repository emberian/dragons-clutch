//! Authenticated projection into the General V2 owner-settlement fee rows.

use clutch_batch_policy_identity::revenue_policy_v1::RevenuePolicyV1;
use clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2;
use clutch_owner_settlement::{
    owner_debit_atoms, CandidateSettlementTotalsV1, CandidateSettlementTotalsV2,
    OwnerSettlementExpectationBasisV2, OwnerSettlementExpectationBasisV3,
    OwnerSettlementExpectationBasisV4, OwnerSettlementExpectationV2,
    OwnerSettlementExpectationV4, SelectedOwnerFeeV1, MAX_ORDERS,
};

use crate::allocation::{
    allocate_payer_debit, allocate_recipients, allocate_recipients_from_weight_stream_v2,
    FeeEnvelopeFundingV1, FeeEnvelopeV1, PayerAllocationV1, RecipientAllocationV1,
    StandingMakerRowV1,
};
use crate::intent::{OwnerFeeTransitionIntentV1, RecipientAllocationIntentV1};
use crate::selected::{
    AssessmentBoundaryV1, OwnerFeeAssessmentV1, OwnerFeeCarryV1,
    SelectedCompositeFeeAccess, SelectedCompositeFeeV1, SelectedCompositeFeeV2,
};
use crate::weight_v2::{
    CompositeFeeWeightRowV2, CompositeFeeWeightTranscriptV2, COMPOSITE_FEE_WEIGHT_POLICY_V2,
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
/// V2 is minted from the exact owner-settlement expectation basis before row
/// creation, or reauthenticated from the sealed V2 expectation at terminal
/// realization. The public fee row remains the same exact
/// `(owner, fee_atoms)` contract.
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

/// V3 pre-row fee projection backed by an immutable payer snapshot.
///
/// This carries the exact payer outer complete-data ID as allocation evidence,
/// but deliberately carries no Reservation balance or cash-coverage claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSelectedOwnerFeeV3 {
    fee_record: Id,
    carry_account: Id,
    payer_allocation_account: Id,
    payer_allocation_data_id: Id,
    owner_settlement_account: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    row: SelectedOwnerFeeV1,
}

impl AuthenticatedSelectedOwnerFeeV3 {
    pub const EMPTY: Self = Self {
        fee_record: Id([0; 32]),
        carry_account: Id([0; 32]),
        payer_allocation_account: Id([0; 32]),
        payer_allocation_data_id: Id([0; 32]),
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

    pub const fn payer_allocation_data_id(&self) -> Id {
        self.payer_allocation_data_id
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

/// V4 pre-row fee projection bound to the delivery-complete owner expectation.
///
/// The embedded expectation is sealed directly from the exact V4 basis. Its
/// merge-delivery count remains verifier-derived identity, but never enters
/// fee arithmetic. Like V3, this projection authenticates allocation only: it
/// proves neither present cash nor liveness capitalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSelectedOwnerFeeV4 {
    fee_record: Id,
    carry_account: Id,
    payer_allocation_account: Id,
    payer_allocation_data_id: Id,
    owner_settlement_account: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    row: SelectedOwnerFeeV1,
    expectation: OwnerSettlementExpectationV4,
}

impl AuthenticatedSelectedOwnerFeeV4 {
    /// Selected composite-fee record authenticated by the payer snapshot.
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    /// Terminal owner carry account authenticated by the payer snapshot.
    pub const fn carry_account(&self) -> Id {
        self.carry_account
    }

    /// Persisted payer-allocation account.
    pub const fn payer_allocation_account(&self) -> Id {
        self.payer_allocation_account
    }

    /// Complete-data identity of the exact persisted payer-allocation outer.
    pub const fn payer_allocation_data_id(&self) -> Id {
        self.payer_allocation_data_id
    }

    /// Fresh canonical V4 owner-row PDA supplied by the General composer.
    pub const fn owner_settlement_account(&self) -> Id {
        self.owner_settlement_account
    }

    /// Final selected settlement candidate.
    pub const fn settlement_candidate(&self) -> Id {
        self.settlement_candidate
    }

    /// Immutable revenue-policy identity.
    pub const fn revenue_policy(&self) -> Id {
        self.revenue_policy
    }

    /// Exact public owner-fee row.
    pub const fn row(&self) -> SelectedOwnerFeeV1 {
        self.row
    }

    /// Complete V4 expectation sealed from the verifier-derived basis.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV4 {
        self.expectation
    }
}

/// Reauthenticated terminal payer snapshot for one semantic owner.
///
/// Creation rederives every signed envelope. Later consumers may restore this
/// projection from the immutable program-owned payer PDA, its exact complete
/// outer data ID, and the matching terminal carry. The snapshot authenticates
/// fee allocation only: it makes no claim that collateral cash currently
/// exists or is available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPayerAllocationSnapshotV1 {
    fee_record: Id,
    carry_account: Id,
    payer_allocation_account: Id,
    payer_allocation_data_id: Id,
    payer_envelope_count: u8,
    owner_settlement_account: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    row: SelectedOwnerFeeV1,
}

impl AuthenticatedPayerAllocationSnapshotV1 {
    pub const fn fee_record(&self) -> Id {
        self.fee_record
    }

    pub const fn carry_account(&self) -> Id {
        self.carry_account
    }

    pub const fn payer_allocation_account(&self) -> Id {
        self.payer_allocation_account
    }

    pub const fn payer_allocation_data_id(&self) -> Id {
        self.payer_allocation_data_id
    }

    pub const fn payer_envelope_count(&self) -> u8 {
        self.payer_envelope_count
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

/// Exact canonical selected-owner fee-book transcript width.
pub const SELECTED_OWNER_FEE_BOOK_V1_BYTES: usize =
    8 + 2 + 2 + (3 * 32) + 1 + 7 + 16 + (MAX_ORDERS * (32 + 8));
/// Canonical selected-owner fee-book transcript discriminator.
pub const SELECTED_OWNER_FEE_BOOK_MAGIC_V1: [u8; 8] = *b"DCFEEBOK";
/// Encode the complete canonical owner-sorted fee book.
pub fn encode_selected_owner_fee_book_v1(
    book: &SelectedOwnerFeeBookV1,
) -> Result<[u8; SELECTED_OWNER_FEE_BOOK_V1_BYTES]> {
    let mut output = [0u8; SELECTED_OWNER_FEE_BOOK_V1_BYTES];
    let mut at = 0usize;
    fn put(output: &mut [u8], at: &mut usize, bytes: &[u8]) -> Result<()> {
        let end = at.checked_add(bytes.len()).ok_or(Error::ArithmeticOverflow)?;
        let target = output.get_mut(*at..end).ok_or(Error::InvalidWidth)?;
        target.copy_from_slice(bytes);
        *at = end;
        Ok(())
    }
    put(&mut output, &mut at, &SELECTED_OWNER_FEE_BOOK_MAGIC_V1)?;
    put(&mut output, &mut at, &1u16.to_le_bytes())?;
    put(&mut output, &mut at, &0u16.to_le_bytes())?;
    put(&mut output, &mut at, &book.fee_record.0)?;
    put(&mut output, &mut at, &book.settlement_candidate.0)?;
    put(&mut output, &mut at, &book.revenue_policy.0)?;
    put(&mut output, &mut at, &[book.owner_count])?;
    put(&mut output, &mut at, &[0; 7])?;
    put(
        &mut output,
        &mut at,
        &book.selected_fee_atoms.to_le_bytes(),
    )?;
    let mut index = 0usize;
    while index < MAX_ORDERS {
        put(&mut output, &mut at, &book.rows[index].owner)?;
        put(
            &mut output,
            &mut at,
            &book.rows[index].fee_atoms.to_le_bytes(),
        )?;
        index += 1;
    }
    if at != output.len() {
        return Err(Error::InvalidWidth);
    }
    Ok(output)
}

/// Historical complete-fee-book recipient certificate, retained for decoding
/// immutable 0x85/v2 bytes only. No current constructor or content-ID owner is
/// exposed for this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CertifiedRecipientAllocationV2 {
    allocation: RecipientAllocationV1,
    owner_fee_book_data_id: Id,
    owner_order_set_digest: Id,
    owner_count: u16,
}

impl CertifiedRecipientAllocationV2 {
    /// Exact recipient allocation and collected total.
    pub const fn allocation(&self) -> RecipientAllocationV1 {
        self.allocation
    }

    /// Content identity of the canonical complete selected-owner fee book.
    pub const fn owner_fee_book_data_id(&self) -> Id {
        self.owner_fee_book_data_id
    }

    /// Exhaustive traversal's immutable owner-order-set digest.
    pub const fn owner_order_set_digest(&self) -> Id {
        self.owner_order_set_digest
    }

    /// Exact number of canonical participating owner rows.
    pub const fn owner_count(&self) -> u16 {
        self.owner_count
    }

    pub(crate) fn restore_persisted(
        allocation: RecipientAllocationV1,
        owner_fee_book_data_id: Id,
        owner_order_set_digest: Id,
        owner_count: u16,
    ) -> Result<Self> {
        live(owner_fee_book_data_id)?;
        live(owner_order_set_digest)?;
        if owner_count == 0
            || usize::from(owner_count) > MAX_ORDERS
            || allocation.collected_fee_atoms() == 0
        {
            return Err(Error::InvalidAccountData);
        }
        Ok(Self {
            allocation,
            owner_fee_book_data_id,
            owner_order_set_digest,
            owner_count,
        })
    }
}

/// Candidate-wide recipient allocation certified by the exact V2 weight
/// stream rather than the historical selected-owner fee-book snapshot.
///
/// The allocation already owns its selected fee-record identity. The added
/// fields persist only the V2 stream's policy/transcript provenance, the
/// traversal-owned order-set digest, and the two distinct cardinalities:
/// every traversed executed owner and the zero-omitting nonzero weight rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CertifiedRecipientAllocationV3 {
    allocation: RecipientAllocationV1,
    weight_policy_id: Id,
    weight_transcript_id: Id,
    owner_order_set_digest: Id,
    traversed_owner_count: u16,
    nonzero_weight_row_count: u8,
}

impl CertifiedRecipientAllocationV3 {
    /// Exact allocation and selected fee total.
    pub const fn allocation(&self) -> RecipientAllocationV1 { self.allocation }
    /// Borrow the exact allocation without copying its maximum-width arrays.
    pub const fn allocation_ref(&self) -> &RecipientAllocationV1 { &self.allocation }
    /// Immutable V2 weight-policy identity.
    pub const fn weight_policy_id(&self) -> Id { self.weight_policy_id }
    /// Commitment to the complete Position-sorted exact-weight stream.
    pub const fn weight_transcript_id(&self) -> Id { self.weight_transcript_id }
    /// Exhaustive traversal's immutable owner/order-set digest.
    pub const fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    /// Distinct owners with nonzero selected execution before zero omission.
    pub const fn traversed_owner_count(&self) -> u16 { self.traversed_owner_count }
    /// Exact number of persisted Position allocation rows.
    pub const fn nonzero_weight_row_count(&self) -> u8 {
        self.nonzero_weight_row_count
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore_persisted(
        allocation: RecipientAllocationV1,
        weight_policy_id: Id,
        weight_transcript_id: Id,
        owner_order_set_digest: Id,
        traversed_owner_count: u16,
        nonzero_weight_row_count: u8,
    ) -> Result<Self> {
        live(weight_policy_id)?;
        live(weight_transcript_id)?;
        live(owner_order_set_digest)?;
        if weight_policy_id != COMPOSITE_FEE_WEIGHT_POLICY_V2.id()?
            || traversed_owner_count == 0
            || usize::from(traversed_owner_count) > MAX_ORDERS
            || usize::from(nonzero_weight_row_count) > MAX_FEE_ROWS_V1
            || u16::from(nonzero_weight_row_count) > traversed_owner_count
            || allocation.maker_len() != nonzero_weight_row_count
            || (nonzero_weight_row_count == 0) != (allocation.collected_fee_atoms() == 0)
        {
            return Err(Error::InvalidAccountData);
        }
        Ok(Self {
            allocation,
            weight_policy_id,
            weight_transcript_id,
            owner_order_set_digest,
            traversed_owner_count,
            nonzero_weight_row_count,
        })
    }
}

/// Create the V3 allocation only by replaying one exact V2 weight stream.
///
/// The callback supplies structural rows to the pure contract. The live SBF
/// writer exposes this constructor only through its private
/// `AuthenticatedPortfolioFeeWeightStreamV2` capability; it accepts neither a
/// caller allocation nor a fee-book certificate/data ID.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn certify_recipient_allocation_v3<F>(
    selected: &SelectedCompositeFeeV2,
    revenue: &RevenuePolicyV2,
    transcript: CompositeFeeWeightTranscriptV2,
    owner_order_set_digest: Id,
    traversed_owner_count: u16,
    row_at: F,
) -> Result<CertifiedRecipientAllocationV3>
where
    F: FnMut(u8) -> Result<Option<CompositeFeeWeightRowV2>>,
{
    live(owner_order_set_digest)?;
    let allocation = allocate_recipients_from_weight_stream_v2(
        selected,
        revenue,
        transcript,
        row_at,
    )?;
    CertifiedRecipientAllocationV3::restore_persisted(
        allocation,
        transcript.policy_id(),
        transcript.transcript_id(),
        owner_order_set_digest,
        traversed_owner_count,
        transcript.len(),
    )
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

    /// Canonical complete-book content identity under the supplied exact hash
    /// backend. This keeps downstream streaming retirement bound to the same
    /// semantic owner instead of reimplementing the transcript.
    pub fn owner_fee_book_data_id<H: SelectedOwnerFeeBookHashV1>(
        &self,
        hash: &H,
    ) -> Result<Id> {
        selected_owner_fee_book_data_id_v1(self, hash)
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

fn bind_persisted_payer_snapshot_v1(
    selected: &SelectedCompositeFeeV1,
    transition: &OwnerFeeTransitionIntentV1,
    carry: &OwnerFeeCarryV1,
    payer: &PayerAllocationV1,
    payer_allocation_data_id: Id,
) -> Result<AuthenticatedPayerAllocationSnapshotV1> {
    live(payer_allocation_data_id)?;
    let owner = transition.owner();
    if transition.fee_record().identity() != selected.fee_record()
        || transition.settlement_candidate() != selected.selected_candidate()
        || transition.revenue_policy() != selected.revenue_policy()
        || carry.fee_record() != selected.fee_record()
        || carry.owner() != owner
        || carry.denominator() != selected.carry_denominator()
        || payer.fee_record() != selected.fee_record()
        || payer.owner() != owner
        || payer.carry_denominator() != selected.carry_denominator()
        || u128::from(payer.total_debit_atoms()) > u128::from(carry.paid_atoms())
    {
        return Err(Error::MismatchedBinding);
    }
    if !carry.is_closed()
        || carry.remainder() != 0
        || payer.boundary() != AssessmentBoundaryV1::TerminalCeil
        || payer.next_carry() != 0
    {
        return Err(Error::TerminalStateRequired);
    }
    Ok(AuthenticatedPayerAllocationSnapshotV1 {
        fee_record: selected.fee_record(),
        carry_account: transition.carry().identity(),
        payer_allocation_account: transition.payer_allocation().identity(),
        payer_allocation_data_id,
        payer_envelope_count: payer.len(),
        owner_settlement_account: transition.owner_settlement().identity(),
        settlement_candidate: selected.selected_candidate(),
        revenue_policy: selected.revenue_policy(),
        row: SelectedOwnerFeeV1 {
            owner: owner.0,
            fee_atoms: carry.paid_atoms(),
        },
    })
}

/// Authenticate snapshot creation by rederiving all signed fee envelopes.
///
/// `payer_allocation_data_id` is the adapter-derived digest of the exact
/// canonical outer account bytes that will be persisted. This path proves fee
/// authorization and allocation but deliberately does not attest present cash.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_created_payer_allocation_snapshot_v1(
    selected: &SelectedCompositeFeeV1,
    transition: &OwnerFeeTransitionIntentV1,
    carry: &OwnerFeeCarryV1,
    assessment: &OwnerFeeAssessmentV1,
    payer: &PayerAllocationV1,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    envelope_len: u8,
    payer_allocation_data_id: Id,
) -> Result<AuthenticatedPayerAllocationSnapshotV1> {
    let owner = transition.owner();
    if assessment.fee_record() != selected.fee_record()
        || assessment.owner() != owner
        || assessment.denominator() != selected.carry_denominator()
        || assessment.boundary() != AssessmentBoundaryV1::TerminalCeil
        || assessment.next_carry() != 0
    {
        return Err(Error::MismatchedBinding);
    }
    let recomputed = allocate_payer_debit(assessment, envelopes, envelope_len)?;
    if recomputed != *payer {
        return Err(Error::MismatchedBinding);
    }
    let mut cumulative_debit_atoms = 0u128;
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
        cumulative_debit_atoms = cumulative_debit_atoms
            .checked_add(u128::from(envelope.debited_atoms))
            .and_then(|value| value.checked_add(u128::from(transition_debit)))
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    if cumulative_debit_atoms != u128::from(carry.paid_atoms()) {
        return Err(Error::ConservationFailure);
    }
    if carry.paid_atoms() != 0 && !has_buy_envelope {
        return Err(Error::SellerFeeForbidden);
    }
    bind_persisted_payer_snapshot_v1(
        selected,
        transition,
        carry,
        payer,
        payer_allocation_data_id,
    )
}

/// Reauthenticate an immutable program-owned payer snapshot without envelopes.
///
/// The caller must first authenticate the owning program, canonical payer PDA,
/// exact outer bytes, and complete-data ID. This function then joins those
/// persisted semantics to the selected fee record and terminal carry. It does
/// not prove cash existence or Reservation coverage.
pub fn reauthenticate_persisted_payer_allocation_snapshot_v1(
    selected: &SelectedCompositeFeeV1,
    transition: &OwnerFeeTransitionIntentV1,
    carry: &OwnerFeeCarryV1,
    payer: &PayerAllocationV1,
    payer_allocation_data_id: Id,
) -> Result<AuthenticatedPayerAllocationSnapshotV1> {
    bind_persisted_payer_snapshot_v1(
        selected,
        transition,
        carry,
        payer,
        payer_allocation_data_id,
    )
}

/// Bind a persisted payer snapshot to one fresh cash-agnostic V3 owner row.
///
/// Action 24 supplies the canonical fresh `0x81/3` row PDA. The snapshot owns
/// fee authorization and allocation; the V3 basis owns selected-order shape.
/// Cash existence and fee-inclusive coverage are intentionally deferred to
/// action 38's exact accumulated buy-cash handoff.
pub fn project_pre_row_owner_fee_v3(
    selected: &SelectedCompositeFeeV1,
    owner_settlement_account: Id,
    basis: OwnerSettlementExpectationBasisV3,
    snapshot: AuthenticatedPayerAllocationSnapshotV1,
) -> Result<AuthenticatedSelectedOwnerFeeV3> {
    live(owner_settlement_account)?;
    let expected_envelope_count = basis
        .expected_buy_order_mask()
        .count_ones()
        .checked_add(basis.expected_sell_order_mask().count_ones())
        .ok_or(Error::ArithmeticOverflow)?;
    let row = snapshot.row();
    if snapshot.fee_record() != selected.fee_record()
        || snapshot.settlement_candidate() != selected.selected_candidate()
        || snapshot.revenue_policy() != selected.revenue_policy()
        || snapshot.owner_settlement_account() != owner_settlement_account
        || snapshot.payer_envelope_count() != u8::try_from(expected_envelope_count)
            .map_err(|_| Error::InvalidWidth)?
        || basis.market() != selected.market().0
        || basis.epoch() != selected.epoch().0
        || basis.candidate() != selected.selected_candidate().0
        || basis.price_scale() != selected.price_scale()
        || basis.owner() != row.owner
    {
        return Err(Error::MismatchedBinding);
    }
    if row.fee_atoms != 0 && basis.expected_buy_order_mask() == 0 {
        return Err(Error::SellerFeeForbidden);
    }
    basis
        .with_selected_fee(row)
        .map_err(|_| Error::InvalidAccountData)?;
    Ok(AuthenticatedSelectedOwnerFeeV3 {
        fee_record: snapshot.fee_record(),
        carry_account: snapshot.carry_account(),
        payer_allocation_account: snapshot.payer_allocation_account(),
        payer_allocation_data_id: snapshot.payer_allocation_data_id(),
        owner_settlement_account,
        settlement_candidate: snapshot.settlement_candidate(),
        revenue_policy: snapshot.revenue_policy(),
        row,
    })
}

/// Bind one persisted payer snapshot to a fresh delivery-complete V4 row.
///
/// The exact V4 basis owns order masks, present considerations, receipt-end
/// count, and virtual-merge delivery count. Fee allocation consumes only the
/// owner/order cardinality and selected-fee identities; it seals and returns
/// the complete expectation without lowering through V3. The caller must
/// separately authenticate the SettlementRoot and derive the supplied row PDA.
pub fn project_pre_row_owner_fee_v4<S: SelectedCompositeFeeAccess + ?Sized>(
    selected: &S,
    owner_settlement_account: Id,
    basis: OwnerSettlementExpectationBasisV4,
    snapshot: AuthenticatedPayerAllocationSnapshotV1,
) -> Result<AuthenticatedSelectedOwnerFeeV4> {
    live(owner_settlement_account)?;
    let expected_envelope_count = basis
        .expected_buy_order_mask()
        .count_ones()
        .checked_add(basis.expected_sell_order_mask().count_ones())
        .ok_or(Error::ArithmeticOverflow)?;
    let row = snapshot.row();
    if snapshot.fee_record() != selected.fee_record()
        || snapshot.settlement_candidate() != selected.selected_candidate()
        || snapshot.revenue_policy() != selected.revenue_policy()
        || snapshot.owner_settlement_account() != owner_settlement_account
        || snapshot.payer_envelope_count() != u8::try_from(expected_envelope_count)
            .map_err(|_| Error::InvalidWidth)?
        || basis.market() != selected.market().0
        || basis.epoch() != selected.epoch().0
        || basis.candidate() != selected.selected_candidate().0
        || basis.price_scale() != selected.price_scale()
        || basis.owner() != row.owner
    {
        return Err(Error::MismatchedBinding);
    }
    if row.fee_atoms != 0 && basis.expected_buy_order_mask() == 0 {
        return Err(Error::SellerFeeForbidden);
    }
    let expectation = basis
        .with_selected_fee(row)
        .map_err(|_| Error::InvalidAccountData)?;
    Ok(AuthenticatedSelectedOwnerFeeV4 {
        fee_record: snapshot.fee_record(),
        carry_account: snapshot.carry_account(),
        payer_allocation_account: snapshot.payer_allocation_account(),
        payer_allocation_data_id: snapshot.payer_allocation_data_id(),
        owner_settlement_account,
        settlement_candidate: snapshot.settlement_candidate(),
        revenue_policy: snapshot.revenue_policy(),
        row,
        expectation,
    })
}

/// Project one terminal owner carry before the V2 owner row exists.
///
/// General's complete action-24 materializer owns the pre-fee basis. The fee
/// runtime owns assessment/carry/payer validation and returns only the exact
/// `(owner, fee_atoms)` row. Owner settlement then seals the final expectation
/// with `basis.with_selected_fee(row)`, avoiding a circular dependency. Zero
/// consideration remains valid when the basis says the buy side is present.
#[allow(clippy::too_many_arguments)]
pub fn project_pre_row_owner_fee_v2(
    selected: &SelectedCompositeFeeV1,
    transition: &OwnerFeeTransitionIntentV1,
    carry: &OwnerFeeCarryV1,
    assessment: &OwnerFeeAssessmentV1,
    payer: &PayerAllocationV1,
    basis: OwnerSettlementExpectationBasisV2,
    envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    envelope_len: u8,
) -> Result<AuthenticatedSelectedOwnerFeeV2> {
    let owner = Id(basis.owner());
    live(owner)?;
    let expected_envelope_len = basis
        .expected_buy_order_mask()
        .count_ones()
        .checked_add(basis.expected_sell_order_mask().count_ones())
        .ok_or(Error::ArithmeticOverflow)?;
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
        || basis.market() != selected.market().0
        || basis.epoch() != selected.epoch().0
        || basis.candidate() != selected.selected_candidate().0
        || basis.price_scale() != selected.price_scale()
        || u32::from(envelope_len) != expected_envelope_len
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

    let has_buy = basis.expected_buy_order_mask() != 0;
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
        basis.expected_buy_price_units().value,
        basis.price_scale(),
        carry.paid_atoms(),
    )
    .map_err(|_| Error::ArithmeticOverflow)?;
    if required > basis.reserved_cash_atoms() {
        return Err(Error::InsufficientBuyReservation);
    }

    let row = SelectedOwnerFeeV1 {
        owner: basis.owner(),
        fee_atoms: carry.paid_atoms(),
    };
    basis
        .with_selected_fee(row)
        .map_err(|_| Error::InvalidAccountData)?;

    Ok(AuthenticatedSelectedOwnerFeeV2 {
        fee_record: selected.fee_record(),
        carry_account: transition.carry().identity(),
        payer_allocation_account: transition.payer_allocation().identity(),
        owner_settlement_account: transition.owner_settlement().identity(),
        settlement_candidate: selected.selected_candidate(),
        revenue_policy: selected.revenue_policy(),
        row,
    })
}

/// Reauthenticate a terminal fee projection against an already sealed V2 row.
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
    let basis = OwnerSettlementExpectationBasisV2::from_expectation(expectation)
        .map_err(|_| Error::InvalidAccountData)?;
    let projection = project_pre_row_owner_fee_v2(
        selected,
        transition,
        carry,
        assessment,
        payer,
        basis,
        envelopes,
        envelope_len,
    )?;
    if expectation.selected_fee_atoms != projection.row().fee_atoms {
        return Err(Error::MismatchedBinding);
    }
    Ok(projection)
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

/// Assemble the exact fee book used by presence-explicit V2 settlement.
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

/// Assemble the exact V3 fee book from owner-sorted payer snapshots.
///
/// Every participating owner, including seller-only and zero-fee owners, must
/// have an explicit row. Payer evidence stays on each authenticated projection
/// and is not collapsed into the candidate total.
pub fn assemble_selected_owner_fee_book_v3(
    selected: &SelectedCompositeFeeV1,
    participating_owners: &[Id; MAX_ORDERS],
    projections: &[AuthenticatedSelectedOwnerFeeV3; MAX_ORDERS],
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
            || projections[index] != AuthenticatedSelectedOwnerFeeV3::EMPTY
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

/// Assemble the exact V4 fee book from owner-sorted, presence-explicit rows.
///
/// Active entries must be `Some` and padding must be `None`; this avoids an
/// artificial empty V4 expectation. Each sealed expectation remains available
/// to the General composer for its independent SettlementRoot/PDA equality
/// join, while this aggregate owns only the selected-fee total.
pub fn assemble_selected_owner_fee_book_v4(
    selected: &SelectedCompositeFeeV1,
    participating_owners: &[Id; MAX_ORDERS],
    projections: &[Option<AuthenticatedSelectedOwnerFeeV4>; MAX_ORDERS],
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
        let projection = projections[index].ok_or(Error::MissingParticipant)?;
        let expectation = projection.expectation;
        if projection.fee_record != selected.fee_record()
            || projection.settlement_candidate != selected.selected_candidate()
            || projection.revenue_policy != selected.revenue_policy()
            || projection.row.owner != owner.0
            || expectation.candidate() != selected.selected_candidate().0
            || expectation.owner() != owner.0
            || expectation.selected_fee_atoms() != projection.row.fee_atoms
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
        if participating_owners[index] != Id([0; 32]) || projections[index].is_some() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v1::{
        AllocationPolicyV1, AonPolicyV1, FeeBaseV1, FrozenPolicyV1,
        PairingWitnessPolicyV1, PortfolioLotPolicyV1, ResidualSettlementV1,
        RoundingBoundaryV1, ScorePolicyV1, SelfCrossPolicyV1, TransferPhaseV1,
    };
    use clutch_batch::DustPolicy;
    use clutch_batch_policy_identity::revenue_policy_v1::{
        LamportSinkV1, RevenuePolicyV1, RevenueResidualV1, StandingMakerV1,
        REVENUE_POLICY_SCHEMA_V1,
    };
    use clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2;
    use clutch_owner_settlement::{
        build_owner_settlement_expectation_basis_book_v4, PresentConsiderationV2,
        SettlementSideV1, VerifiedSettlementOrderV4,
    };

    fn id(byte: u8) -> Id {
        Id([byte; 32])
    }

    fn selected() -> SelectedCompositeFeeV1 {
        let batch = FrozenPolicyV1 {
            allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
            self_cross: SelfCrossPolicyV1::RefuseOverlap,
            aon: AonPolicyV1::RefuseAdmission,
            rounding: RoundingBoundaryV1::TerminalOwnerFloor,
            residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
            transfer_phase: TransferPhaseV1::ActiveOrResolved,
            portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            dust: DustPolicy::AssignCanonical,
            score: ScorePolicyV1::LexicographicDispersionV1,
            fee_base: FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps: 25,
                floor_range_bps: 10,
            },
        };
        let revenue = revenue();
        SelectedCompositeFeeV1::select(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            10_000,
            2,
            &batch,
            &revenue,
        )
        .unwrap()
    }

    fn revenue() -> RevenuePolicyV1 {
        RevenuePolicyV1 {
            version: u32::from(REVENUE_POLICY_SCHEMA_V1),
            treasury: [9; 32],
            maker_rebate_num: 60,
            executor_num: 0,
            treasury_num: 40,
            split_den: 100,
            residual: RevenueResidualV1::Treasury,
            standing_maker: StandingMakerV1::AllRestingMakers,
            lamport_sink: LamportSinkV1::None,
        }
    }

    fn selected_v2() -> (SelectedCompositeFeeV2, RevenuePolicyV2) {
        let batch = FrozenPolicyV1 {
            allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
            self_cross: SelfCrossPolicyV1::RefuseOverlap,
            aon: AonPolicyV1::RefuseAdmission,
            rounding: RoundingBoundaryV1::TerminalOwnerFloor,
            residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
            transfer_phase: TransferPhaseV1::ActiveOrResolved,
            portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            dust: DustPolicy::AssignCanonical,
            score: ScorePolicyV1::LexicographicDispersionV1,
            fee_base: FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps: 40,
                floor_range_bps: 10,
            },
        };
        let revenue = RevenuePolicyV2::successor_development([9; 32]);
        let selected = SelectedCompositeFeeV2::select(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            10_000,
            2,
            &batch,
            &revenue,
        )
        .unwrap();
        (selected, revenue)
    }

    fn basis(
        selected: &SelectedCompositeFeeV1,
        owner: [u8; 32],
    ) -> OwnerSettlementExpectationBasisV4 {
        let buy = VerifiedSettlementOrderV4 {
            owner,
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2 {
                present: true,
                value: 70_000,
            },
            slice_count: 1,
            merge_delivery_count: 0,
        };
        let sell = VerifiedSettlementOrderV4 {
            owner,
            order_index: 1,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2 {
                present: true,
                value: 40_000,
            },
            slice_count: 2,
            merge_delivery_count: 1,
        };
        let mut orders = [buy; MAX_ORDERS];
        orders[1] = sell;
        build_owner_settlement_expectation_basis_book_v4(
            selected.market().0,
            selected.epoch().0,
            selected.selected_candidate().0,
            [15; 32],
            selected.price_scale(),
            &orders,
            2,
        )
        .unwrap()
        .row(0)
        .unwrap()
    }

    fn snapshot(
        selected: &SelectedCompositeFeeV1,
        owner_settlement_account: Id,
        owner: [u8; 32],
        fee_atoms: u64,
        payer_envelope_count: u8,
    ) -> AuthenticatedPayerAllocationSnapshotV1 {
        AuthenticatedPayerAllocationSnapshotV1 {
            fee_record: selected.fee_record(),
            carry_account: id(10),
            payer_allocation_account: id(11),
            payer_allocation_data_id: id(12),
            payer_envelope_count,
            owner_settlement_account,
            settlement_candidate: selected.selected_candidate(),
            revenue_policy: selected.revenue_policy(),
            row: SelectedOwnerFeeV1 { owner, fee_atoms },
        }
    }

    #[test]
    fn v4_projection_preserves_merge_identity_and_seals_fee() {
        let selected = selected();
        let owner = [14; 32];
        let row_account = id(13);
        let projection = project_pre_row_owner_fee_v4(
            &selected,
            row_account,
            basis(&selected, owner),
            snapshot(&selected, row_account, owner, 7, 2),
        )
        .unwrap();
        assert_eq!(projection.owner_settlement_account(), row_account);
        assert_eq!(projection.row().fee_atoms, 7);
        assert_eq!(projection.expectation().selected_fee_atoms(), 7);
        assert_eq!(projection.expectation().expected_merge_delivery_count(), 1);
        assert_eq!(projection.expectation().expected_slice_count(), 3);
    }

    #[test]
    fn wrong_row_or_envelope_cardinality_cannot_rebind_snapshot() {
        let selected = selected();
        let owner = [14; 32];
        let row_account = id(13);
        assert_eq!(
            project_pre_row_owner_fee_v4(
                &selected,
                id(16),
                basis(&selected, owner),
                snapshot(&selected, row_account, owner, 7, 2),
            ),
            Err(Error::MismatchedBinding)
        );
        assert_eq!(
            project_pre_row_owner_fee_v4(
                &selected,
                row_account,
                basis(&selected, owner),
                snapshot(&selected, row_account, owner, 7, 1),
            ),
            Err(Error::MismatchedBinding)
        );
    }

    #[test]
    fn seller_only_v4_row_is_explicit_but_cannot_pay_fee() {
        let selected = selected();
        let owner = [14; 32];
        let row_account = id(13);
        let sell = VerifiedSettlementOrderV4 {
            owner,
            order_index: 1,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2 {
                present: true,
                value: 0,
            },
            slice_count: 1,
            merge_delivery_count: 1,
        };
        let orders = [sell; MAX_ORDERS];
        let seller_basis = build_owner_settlement_expectation_basis_book_v4(
            selected.market().0,
            selected.epoch().0,
            selected.selected_candidate().0,
            [15; 32],
            selected.price_scale(),
            &orders,
            1,
        )
        .unwrap()
        .row(0)
        .unwrap();
        assert_eq!(
            project_pre_row_owner_fee_v4(
                &selected,
                row_account,
                seller_basis,
                snapshot(&selected, row_account, owner, 1, 1),
            ),
            Err(Error::SellerFeeForbidden)
        );
        let zero = project_pre_row_owner_fee_v4(
            &selected,
            row_account,
            seller_basis,
            snapshot(&selected, row_account, owner, 0, 1),
        )
        .unwrap();
        assert_eq!(zero.row().fee_atoms, 0);
        assert_eq!(zero.expectation().expected_merge_delivery_count(), 1);
    }

    #[test]
    fn v4_book_requires_explicit_rows_exact_total_and_none_padding() {
        let selected = selected();
        let owner = [14; 32];
        let row_account = id(13);
        let projection = project_pre_row_owner_fee_v4(
            &selected,
            row_account,
            basis(&selected, owner),
            snapshot(&selected, row_account, owner, 7, 2),
        )
        .unwrap();
        let mut owners = [Id([0; 32]); MAX_ORDERS];
        owners[0] = Id(owner);
        let mut projections = [None; MAX_ORDERS];
        projections[0] = Some(projection);
        let totals = CandidateSettlementTotalsV2 {
            owner_count: 1,
            buy_price_units: PresentConsiderationV2 {
                present: true,
                value: 70_000,
            },
            sell_price_units: PresentConsiderationV2 {
                present: true,
                value: 40_000,
            },
            selected_fee_atoms: 7,
            rounding_pot_price_units: 0,
            owner_slice_end_count: 3,
        };
        let book = assemble_selected_owner_fee_book_v4(
            &selected,
            &owners,
            &projections,
            1,
            totals,
        )
        .unwrap();
        assert_eq!(book.owner_count, 1);
        assert_eq!(book.selected_fee_atoms, 7);
        let mut wrong_total = totals;
        wrong_total.selected_fee_atoms = 8;
        assert_eq!(
            assemble_selected_owner_fee_book_v4(
                &selected,
                &owners,
                &projections,
                1,
                wrong_total,
            ),
            Err(Error::SelectedFeeTotalMismatch)
        );
        projections[1] = Some(projection);
        assert_eq!(
            assemble_selected_owner_fee_book_v4(
                &selected,
                &owners,
                &projections,
                1,
                totals,
            ),
            Err(Error::NonCanonicalPadding)
        );
    }

    #[test]
    fn v3_persists_only_stream_provenance_and_exact_hamilton_output() {
        let (selected, revenue) = selected_v2();
        let denominator = selected.carry_denominator();
        let rows = [
            CompositeFeeWeightRowV2::structural(id(20), denominator).unwrap(),
            CompositeFeeWeightRowV2::structural(id(21), denominator / 2).unwrap(),
        ];
        let mut transcript_at = 0usize;
        let transcript = crate::weight_v2::composite_fee_weight_transcript_v2(
            selected.fee_record(),
            denominator,
            |_| {
                if transcript_at < rows.len() {
                    let row = rows[transcript_at];
                    transcript_at += 1;
                    Ok(Some(row))
                } else {
                    Ok(None)
                }
            },
        )
        .unwrap();
        let certified = certify_recipient_allocation_v3(
            &selected,
            &revenue,
            transcript,
            id(30),
            3,
            |index| Ok(rows.get(usize::from(index)).copied()),
        )
        .unwrap();
        assert_eq!(certified.weight_policy_id(), transcript.policy_id());
        assert_eq!(certified.weight_transcript_id(), transcript.transcript_id());
        assert_eq!(certified.owner_order_set_digest(), id(30));
        assert_eq!(certified.traversed_owner_count(), 3);
        assert_eq!(certified.nonzero_weight_row_count(), 2);
        assert_eq!(certified.allocation().collected_fee_atoms(), 2);
        assert_eq!(certified.allocation().maker_len(), 2);

        let mut encoded = [0u8; crate::codec::CERTIFIED_RECIPIENT_ALLOCATION_V3_BYTES];
        crate::codec::encode_certified_recipient_allocation_v3_into(
            &certified,
            &mut encoded,
        )
        .unwrap();
        assert_eq!(encoded.len(), 2_744);
        let borrowed = crate::codec::decode_borrowed_certified_recipient_allocation_v3(
            &encoded,
        )
        .unwrap();
        let summary = borrowed.summary();
        assert_eq!(summary.fee_record(), selected.fee_record());
        assert_eq!(summary.row_count(), 2);
        assert_eq!(summary.collected_fee_atoms(), 2);
        assert_eq!(summary.weight_policy_id(), transcript.policy_id());
        assert_eq!(summary.weight_transcript_id(), transcript.transcript_id());
        assert_eq!(summary.owner_order_set_digest(), id(30));
        assert_eq!(summary.traversed_owner_count(), 3);
        assert_eq!(summary.nonzero_weight_row_count(), 2);
        assert_eq!(borrowed.allocation().row(0).unwrap().unwrap().position(), id(20));
        assert_eq!(borrowed.allocation().row(1).unwrap().unwrap().position(), id(21));
        assert_eq!(borrowed.allocation().row(2).unwrap(), None);
        let mut streamed = [0u8; crate::codec::CERTIFIED_RECIPIENT_ALLOCATION_V3_BYTES];
        crate::codec::encode_certified_recipient_allocation_v3_from_access_into(
            &borrowed,
            &mut streamed,
        )
        .unwrap();
        assert_eq!(streamed, encoded);

        let mut hostile_reserved = encoded;
        hostile_reserved[2_743] = 1;
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(
                &hostile_reserved,
            ),
            Err(Error::NonCanonicalPadding)
        );
        let mut hostile_order = encoded;
        let first_position: [u8; 32] = hostile_order[80..112].try_into().unwrap();
        hostile_order[120..152].copy_from_slice(&first_position);
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(&hostile_order),
            Err(Error::NonCanonicalOrder)
        );
        let mut hostile_tail = encoded;
        hostile_tail[160] = 1;
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(&hostile_tail),
            Err(Error::NonCanonicalPadding)
        );
        let mut hostile_policy = encoded;
        hostile_policy[2_640..2_672].fill(0);
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(&hostile_policy),
            Err(Error::ZeroIdentity)
        );
        let mut hostile_count = encoded;
        hostile_count[2_738] = 1;
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(&hostile_count),
            Err(Error::InvalidAccountData)
        );
        let mut hostile_total = encoded;
        hostile_total[72..80].copy_from_slice(&3u64.to_le_bytes());
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(&hostile_total),
            Err(Error::ConservationFailure)
        );
        assert_eq!(
            CertifiedRecipientAllocationV3::restore_persisted(
                certified.allocation(),
                certified.weight_policy_id(),
                certified.weight_transcript_id(),
                certified.owner_order_set_digest(),
                1,
                certified.nonzero_weight_row_count(),
            ),
            Err(Error::InvalidAccountData)
        );
    }

    #[test]
    fn v3_canonically_persists_present_fee_policy_with_zero_weight() {
        let (selected, revenue) = selected_v2();
        let transcript = crate::weight_v2::composite_fee_weight_transcript_from_indexed_rows_v2(
            selected.fee_record(),
            selected.carry_denominator(),
            0,
            |_| Ok(None),
        )
        .unwrap();
        assert_eq!(transcript.len(), 0);
        assert_eq!(transcript.total_weight(), 0);

        let certified = certify_recipient_allocation_v3(
            &selected,
            &revenue,
            transcript,
            id(30),
            2,
            |_| Ok(None),
        )
        .unwrap();
        assert_eq!(certified.nonzero_weight_row_count(), 0);
        assert_eq!(certified.allocation().maker_len(), 0);
        assert_eq!(certified.allocation().maker_rebate_total(), 0);
        assert_eq!(certified.allocation().executor_atoms(), 0);
        assert_eq!(certified.allocation().treasury_atoms(), 0);
        assert_eq!(certified.allocation().collected_fee_atoms(), 0);

        let mut encoded = [0u8; crate::codec::CERTIFIED_RECIPIENT_ALLOCATION_V3_BYTES];
        crate::codec::encode_certified_recipient_allocation_v3_into(
            &certified,
            &mut encoded,
        )
        .unwrap();
        let borrowed = crate::codec::decode_borrowed_certified_recipient_allocation_v3(
            &encoded,
        )
        .unwrap();
        assert_eq!(borrowed.summary().row_count(), 0);
        assert_eq!(borrowed.summary().traversed_owner_count(), 2);
        assert_eq!(borrowed.summary().collected_fee_atoms(), 0);
        assert_eq!(borrowed.allocation().row(0).unwrap(), None);

        let mut hostile_row_count = encoded;
        hostile_row_count[2_738] = 1;
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(
                &hostile_row_count,
            ),
            Err(Error::InvalidAccountData)
        );
        let mut hostile_total = encoded;
        hostile_total[72..80].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(
            crate::codec::decode_borrowed_certified_recipient_allocation_v3(&hostile_total),
            Err(Error::ConservationFailure)
        );
    }
}
