//! Compact, streaming retirement authority for one selected fee record.
//!
//! The complete owner fee book is authenticated once when this accumulator is
//! created.  Owner finalizations can then be consumed in canonical owner order
//! without retaining all 64 temporary accounts in one transaction.  The two
//! rolling commitments prevent a sequential close from degrading the terminal
//! contract to a count-and-sum check.

use crate::intent::RecipientAllocationIntentV1;
use crate::codec::CertifiedRecipientAllocationAccessV3;
use crate::projection::SelectedOwnerFeeBookHashV1;
use crate::selected::SelectedCompositeFeeV2;
use crate::terminal::{
    AuthenticatedOwnerFeeFinalizationV1, CandidateFeeAccountClosuresV1,
    CandidateFeeAccountRoleV1, ExternalFeeAccountClosureV1,
    FeeTerminalOutcomeV1, OwnerFeeFinalizationOutcomeV2,
};
use crate::{add, independent, live, Error, Id, Result, MAX_FEE_ROWS_V1};

/// Commitment domain for the expected canonical owner-fee row sequence.
pub const FEE_OWNER_ROW_FOLD_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fee-owner-row-fold/v1\0";
/// Commitment domain for the observed terminal owner-account closures.
pub const FEE_OWNER_CLOSURE_FOLD_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fee-owner-closure-fold/v1\0";
/// Commitment domain for the complete global-plus-owner closure set.
pub const FEE_CLOSURE_SET_DATA_ID_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fee-closure-set-data-id/v1\0";
/// Commitment domain for the accumulator's terminal authority receipt.
pub const FEE_RETIREMENT_AUTHORITY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fee-retirement-authority/v1\0";
/// Initial transcript for the current streaming V2 owner-fee book.
pub const FEE_OWNER_BOOK_STREAM_START_DOMAIN_V2: &[u8] =
    b"dragons-clutch/fee-owner-book-start/v2\0";
/// Ordered row fold for the current streaming V2 owner-fee book.
pub const FEE_OWNER_BOOK_STREAM_ROW_DOMAIN_V2: &[u8] =
    b"dragons-clutch/fee-owner-book-row/v2\0";
/// Final data identity for the current streaming V2 owner-fee book.
pub const FEE_OWNER_BOOK_STREAM_DATA_ID_DOMAIN_V2: &[u8] =
    b"dragons-clutch/fee-owner-book-data/v2\0";
/// Commitment domain for ordered ordinary-Position fee credits.
pub const FEE_VALUE_DISPOSITION_FOLD_DOMAIN_V1: &[u8] =
    b"dragons-clutch/fee-value-disposition/v1\0";
/// Exact semantic body width of the streaming accumulator.
pub const FEE_RETIREMENT_ACCUMULATOR_BODY_V1_BYTES: usize = 656;
/// Canonical semantic discriminator of the streaming accumulator.
pub const FEE_RETIREMENT_ACCUMULATOR_MAGIC_V1: [u8; 8] = *b"DCFEEACC";
/// Canonical semantic body version.
pub const FEE_RETIREMENT_ACCUMULATOR_VERSION_V1: u16 = 1;

/// Minimal exact SHA-256 seam used by the streaming fee-retirement owner.
pub trait FeeRetirementHashV1: SelectedOwnerFeeBookHashV1 {
    /// SHA-256 over the exact ordered byte slices.
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32];
}

/// Compact builder for the exact Position-PDA-sorted owner fee rows produced
/// by one authenticated traversal. It never materializes the 64-row book.
#[derive(Debug, Eq, PartialEq)]
pub struct StreamingOwnerFeeBookV2 {
    fee_record: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    owner_order_set_digest: Id,
    expected_owner_count: u8,
    processed_owner_count: u8,
    expected_fee_atoms: u128,
    processed_fee_atoms: u128,
    prior_owner: Id,
    initial_row_fold: Id,
    row_fold: Id,
}

/// One-shot completion consumed by accumulator creation. Private fields make
/// a caller-supplied count, sum, or digest insufficient.
#[derive(Debug, Eq, PartialEq)]
pub struct CompletedOwnerFeeBookV2 {
    fee_record: Id,
    settlement_candidate: Id,
    revenue_policy: Id,
    owner_order_set_digest: Id,
    owner_count: u8,
    fee_atoms: u128,
    initial_row_fold: Id,
    expected_row_fold: Id,
    data_id: Id,
}

impl StreamingOwnerFeeBookV2 {
    pub fn begin<H: FeeRetirementHashV1>(
        selected: &SelectedCompositeFeeV2,
        owner_order_set_digest: Id,
        owner_count: u8,
        fee_atoms: u128,
        hash: &H,
    ) -> Result<Self> {
        live(owner_order_set_digest)?;
        if owner_count == 0
            || usize::from(owner_count) > MAX_FEE_ROWS_V1
            || fee_atoms == 0
            || fee_atoms > u128::from(u64::MAX)
        {
            return Err(Error::InvalidWidth);
        }
        let initial_row_fold = owner_book_stream_start_v2(
            selected,
            owner_order_set_digest,
            owner_count,
            fee_atoms,
            hash,
        )?;
        Ok(Self {
            fee_record: selected.fee_record(),
            settlement_candidate: selected.selected_candidate(),
            revenue_policy: selected.revenue_policy(),
            owner_order_set_digest,
            expected_owner_count: owner_count,
            processed_owner_count: 0,
            expected_fee_atoms: fee_atoms,
            processed_fee_atoms: 0,
            prior_owner: Id([0; 32]),
            initial_row_fold,
            row_fold: initial_row_fold,
        })
    }

    pub fn fold<H: FeeRetirementHashV1>(
        mut self,
        owner: Id,
        fee_atoms: u64,
        hash: &H,
    ) -> Result<Self> {
        live(owner)?;
        if self.processed_owner_count >= self.expected_owner_count
            || (!self.prior_owner.is_zero() && owner <= self.prior_owner)
        {
            return Err(Error::NonCanonicalOrder);
        }
        self.row_fold = fold_owner_book_row_v2(
            self.row_fold,
            self.processed_owner_count,
            owner,
            fee_atoms,
            hash,
        )?;
        self.processed_fee_atoms = self
            .processed_fee_atoms
            .checked_add(u128::from(fee_atoms))
            .ok_or(Error::ArithmeticOverflow)?;
        self.prior_owner = owner;
        self.processed_owner_count = self
            .processed_owner_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(self)
    }

    pub fn complete<H: FeeRetirementHashV1>(
        self,
        hash: &H,
    ) -> Result<CompletedOwnerFeeBookV2> {
        if self.processed_owner_count != self.expected_owner_count
            || self.processed_fee_atoms != self.expected_fee_atoms
        {
            return Err(Error::MissingParticipant);
        }
        let data_id = Id(hash.sha256(&[
            FEE_OWNER_BOOK_STREAM_DATA_ID_DOMAIN_V2,
            &self.fee_record.0,
            &self.settlement_candidate.0,
            &self.revenue_policy.0,
            &self.owner_order_set_digest.0,
            &[self.expected_owner_count],
            &self.expected_fee_atoms.to_le_bytes(),
            &self.row_fold.0,
        ]));
        live(data_id)?;
        Ok(CompletedOwnerFeeBookV2 {
            fee_record: self.fee_record,
            settlement_candidate: self.settlement_candidate,
            revenue_policy: self.revenue_policy,
            owner_order_set_digest: self.owner_order_set_digest,
            owner_count: self.expected_owner_count,
            fee_atoms: self.expected_fee_atoms,
            initial_row_fold: self.initial_row_fold,
            expected_row_fold: self.row_fold,
            data_id,
        })
    }
}

impl CompletedOwnerFeeBookV2 {
    pub const fn data_id(&self) -> Id { self.data_id }
    pub const fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    pub const fn owner_count(&self) -> u8 { self.owner_count }
    pub const fn fee_atoms(&self) -> u128 { self.fee_atoms }
}

/// Compact semantic state persisted while owner finalizations are retired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeRetirementAccumulatorV1 {
    runtime_program: Id,
    runtime_release: Id,
    settlement_root: Id,
    selected_feed_data_id: Id,
    fee_record: Id,
    settlement_candidate: Id,
    recipient_allocation: Id,
    recipient_allocation_data_id: Id,
    treasury_ledger: Id,
    settlement_cash_pot: Id,
    treasury_position: Id,
    owner_fee_book_data_id: Id,
    owner_order_set_digest: Id,
    expected_owner_row_fold: Id,
    observed_owner_row_fold: Id,
    owner_closure_fold: Id,
    value_disposition_fold: Id,
    prior_owner: Id,
    expected_owner_count: u8,
    processed_owner_count: u8,
    expected_maker_count: u8,
    processed_maker_count: u8,
    treasury_distributed: bool,
    expected_fee_atoms: u128,
    processed_fee_atoms: u128,
    distributed_maker_atoms: u64,
    distributed_treasury_atoms: u64,
    owner_refund_lamports: u64,
    owner_neutral_credit_lamports: u64,
}

/// Complete, non-forgeable authority consumed by candidate-wide terminal
/// construction after all owner finalizations have been closed.
#[derive(Debug, Eq, PartialEq)]
pub struct CompletedFeeRetirementV1 {
    accumulator: FeeRetirementAccumulatorV1,
    closure_set_data_id: Id,
    terminal_authority_receipt: Id,
    payer_refund_lamports: u64,
    neutral_credit_lamports: u64,
    accumulator_account: Id,
    accumulator_close_receipt: Id,
    accumulator_rent_payer: Id,
    accumulator_neutral_sink: Id,
    accumulator_refund_lamports: u64,
    accumulator_neutral_credit_lamports: u64,
}

/// Adapter-authenticated value transition for one ordinary Position credit.
/// The Position and Replay semantic owners authenticate these identities; this
/// owner binds their exact ordered transition to the fee allocation and cash
/// pot without duplicating either account body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeePositionCreditTransitionV1 {
    pub position_account: Id,
    pub replay_account: Id,
    pub position_prestate: Id,
    pub position_poststate: Id,
    pub replay_prestate: Id,
    pub replay_poststate: Id,
    pub cash_pot_account: Id,
    pub cash_pot_prestate: Id,
    pub cash_pot_poststate: Id,
    pub credited_atoms: u64,
}

/// One-shot authority proving the exact treasury credit was the terminal
/// recipient in the ordered value-disposition fold.
#[derive(Debug, Eq, PartialEq)]
pub struct TreasuryDistributionAuthorizationV1 {
    pub(crate) fee_record: Id,
    pub(crate) treasury_owner: Id,
    pub(crate) treasury_position: Id,
    pub(crate) settlement_candidate: Id,
    pub(crate) revenue_policy: Id,
    pub(crate) credited_atoms: u64,
    pub(crate) value_disposition_receipt: Id,
}

impl FeePositionCreditTransitionV1 {
    fn validate(self) -> Result<()> {
        independent(&[
            self.position_account,
            self.replay_account,
            self.cash_pot_account,
        ])?;
        for identity in [
            self.position_prestate,
            self.position_poststate,
            self.replay_prestate,
            self.replay_poststate,
            self.cash_pot_prestate,
            self.cash_pot_poststate,
        ] {
            live(identity)?;
        }
        if self.credited_atoms == 0 {
            if self.position_prestate != self.position_poststate
                || self.replay_prestate != self.replay_poststate
                || self.cash_pot_prestate != self.cash_pot_poststate
            {
                return Err(Error::InvalidTerminalDisposition);
            }
        } else if self.position_prestate == self.position_poststate
            || self.replay_prestate == self.replay_poststate
            || self.cash_pot_prestate == self.cash_pot_poststate
        {
            return Err(Error::InvalidTerminalDisposition);
        }
        Ok(())
    }
}

impl FeeRetirementAccumulatorV1 {
    /// Create the accumulator from the one-shot streaming book completion and
    /// the certified recipient body written from the same traversal.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_streaming<C: CertifiedRecipientAllocationAccessV3 + ?Sized>(
        runtime_program: Id,
        runtime_release: Id,
        settlement_root: Id,
        selected_feed_data_id: Id,
        recipient_allocation: Id,
        recipient_allocation_data_id: Id,
        treasury_ledger: Id,
        settlement_cash_pot: Id,
        selected: &SelectedCompositeFeeV2,
        book: CompletedOwnerFeeBookV2,
        certified: &C,
        hash: &impl FeeRetirementHashV1,
    ) -> Result<Self> {
        independent(&[
            runtime_program,
            runtime_release,
            settlement_root,
            selected_feed_data_id,
            selected.fee_record(),
            selected.selected_candidate(),
            recipient_allocation,
            recipient_allocation_data_id,
            treasury_ledger,
            settlement_cash_pot,
            selected.treasury_position(),
            book.data_id,
            book.owner_order_set_digest,
        ])?;
        if book.fee_record != selected.fee_record()
            || book.settlement_candidate != selected.selected_candidate()
            || book.revenue_policy != selected.revenue_policy()
            || certified.owner_order_set_digest() != book.owner_order_set_digest
            || certified.nonzero_weight_row_count() != book.owner_count
            || certified.row_count() != book.owner_count
            || certified.fee_record() != selected.fee_record()
            || u128::from(certified.collected_fee_atoms()) != book.fee_atoms
        {
            return Err(Error::MismatchedBinding);
        }
        let owner_closure_fold = closure_fold_start(selected.fee_record(), hash)?;
        let value_disposition_fold = value_fold_start(
            selected.fee_record(),
            recipient_allocation_data_id,
            settlement_cash_pot,
            hash,
        )?;
        let value = Self {
            runtime_program,
            runtime_release,
            settlement_root,
            selected_feed_data_id,
            fee_record: selected.fee_record(),
            settlement_candidate: selected.selected_candidate(),
            recipient_allocation,
            recipient_allocation_data_id,
            treasury_ledger,
            settlement_cash_pot,
            treasury_position: selected.treasury_position(),
            owner_fee_book_data_id: book.data_id,
            owner_order_set_digest: book.owner_order_set_digest,
            expected_owner_row_fold: book.expected_row_fold,
            observed_owner_row_fold: book.initial_row_fold,
            owner_closure_fold,
            value_disposition_fold,
            prior_owner: Id([0; 32]),
            expected_owner_count: book.owner_count,
            processed_owner_count: 0,
            expected_maker_count: certified.row_count(),
            processed_maker_count: 0,
            treasury_distributed: false,
            expected_fee_atoms: book.fee_atoms,
            processed_fee_atoms: 0,
            distributed_maker_atoms: 0,
            distributed_treasury_atoms: 0,
            owner_refund_lamports: 0,
            owner_neutral_credit_lamports: 0,
        };
        value.validate_open()?;
        Ok(value)
    }

    /// Restore exact persisted words through the same invariant set used by
    /// every transition. This constructor is intentionally exhaustive so an
    /// outer codec never becomes a second semantic owner.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        runtime_program: Id,
        runtime_release: Id,
        settlement_root: Id,
        selected_feed_data_id: Id,
        fee_record: Id,
        settlement_candidate: Id,
        recipient_allocation: Id,
        recipient_allocation_data_id: Id,
        treasury_ledger: Id,
        settlement_cash_pot: Id,
        treasury_position: Id,
        owner_fee_book_data_id: Id,
        owner_order_set_digest: Id,
        expected_owner_row_fold: Id,
        observed_owner_row_fold: Id,
        owner_closure_fold: Id,
        value_disposition_fold: Id,
        prior_owner: Id,
        expected_owner_count: u8,
        processed_owner_count: u8,
        expected_maker_count: u8,
        processed_maker_count: u8,
        treasury_distributed: bool,
        expected_fee_atoms: u128,
        processed_fee_atoms: u128,
        distributed_maker_atoms: u64,
        distributed_treasury_atoms: u64,
        owner_refund_lamports: u64,
        owner_neutral_credit_lamports: u64,
    ) -> Result<Self> {
        let value = Self {
            runtime_program,
            runtime_release,
            settlement_root,
            selected_feed_data_id,
            fee_record,
            settlement_candidate,
            recipient_allocation,
            recipient_allocation_data_id,
            treasury_ledger,
            settlement_cash_pot,
            treasury_position,
            owner_fee_book_data_id,
            owner_order_set_digest,
            expected_owner_row_fold,
            observed_owner_row_fold,
            owner_closure_fold,
            value_disposition_fold,
            prior_owner,
            expected_owner_count,
            processed_owner_count,
            expected_maker_count,
            processed_maker_count,
            treasury_distributed,
            expected_fee_atoms,
            processed_fee_atoms,
            distributed_maker_atoms,
            distributed_treasury_atoms,
            owner_refund_lamports,
            owner_neutral_credit_lamports,
        };
        value.validate_open()?;
        Ok(value)
    }

    /// Fold and consume exactly the next lexicographic owner finalization.
    pub fn fold_owner<H: FeeRetirementHashV1>(
        mut self,
        owner: &AuthenticatedOwnerFeeFinalizationV1,
        closure: &ExternalFeeAccountClosureV1,
        hash: &H,
    ) -> Result<Self> {
        self.validate_open()?;
        if self.processed_owner_count >= self.expected_owner_count
            || owner.receipt.runtime_release() != self.runtime_release
            || owner.receipt.fee_record() != self.fee_record
            || owner.receipt.settlement_candidate() != self.settlement_candidate
            || owner.receipt.outcome() != OwnerFeeFinalizationOutcomeV2::Settled
            || closure.role() != CandidateFeeAccountRoleV1::OwnerFinalization
            || closure.outcome() != FeeTerminalOutcomeV1::Settled
            || closure.runtime_program() != self.runtime_program
            || closure.runtime_release() != self.runtime_release
            || closure.fee_record() != self.fee_record
            || closure.account() != owner.carry_account
            || closure.semantic_owner() != owner.receipt.owner()
            || (!self.prior_owner.is_zero() && owner.receipt.owner() <= self.prior_owner)
        {
            return Err(Error::MismatchedBinding);
        }
        let ordinal = self.processed_owner_count;
        self.observed_owner_row_fold = fold_owner_book_row_v2(
            self.observed_owner_row_fold,
            ordinal,
            owner.receipt.owner(),
            owner.receipt.authorized_fee_atoms(),
            hash,
        )?;
        self.owner_closure_fold = fold_closure(
            self.owner_closure_fold,
            ordinal,
            closure,
            hash,
        )?;
        self.processed_fee_atoms = self
            .processed_fee_atoms
            .checked_add(u128::from(owner.receipt.authorized_fee_atoms()))
            .ok_or(Error::ArithmeticOverflow)?;
        self.owner_refund_lamports = add(
            self.owner_refund_lamports,
            closure.rent_refund_lamports(),
        )?;
        self.owner_neutral_credit_lamports = add(
            self.owner_neutral_credit_lamports,
            closure.neutral_credit_lamports(),
        )?;
        self.prior_owner = owner.receipt.owner();
        self.processed_owner_count = self
            .processed_owner_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        self.validate_open()?;
        Ok(self)
    }

    /// Fold one exact maker credit in certified recipient order. The adapter
    /// must first authenticate the Position, Replay, and cash-pot successors.
    pub fn fold_maker_distribution<H: FeeRetirementHashV1, C: CertifiedRecipientAllocationAccessV3 + ?Sized>(
        mut self,
        certified: &C,
        transition: FeePositionCreditTransitionV1,
        hash: &H,
    ) -> Result<Self> {
        self.validate_open()?;
        transition.validate()?;
        let ordinal = usize::from(self.processed_maker_count);
        if self.processed_owner_count != self.expected_owner_count
            || certified.owner_order_set_digest() != self.owner_order_set_digest
            || certified.nonzero_weight_row_count() != self.expected_owner_count
            || certified.fee_record() != self.fee_record
            || certified.row_count() != self.expected_maker_count
            || ordinal >= usize::from(self.expected_maker_count)
            || transition.position_account
                != certified
                    .row(self.processed_maker_count)?
                    .ok_or(Error::MissingParticipant)?
                    .position()
            || transition.cash_pot_account != self.settlement_cash_pot
            || transition.credited_atoms
                != certified
                    .row(self.processed_maker_count)?
                    .ok_or(Error::MissingParticipant)?
                    .rebate_atoms()
        {
            return Err(Error::MismatchedBinding);
        }
        self.value_disposition_fold = fold_value_transition(
            self.value_disposition_fold,
            self.processed_maker_count,
            1,
            transition,
            hash,
        )?;
        self.distributed_maker_atoms = add(
            self.distributed_maker_atoms,
            transition.credited_atoms,
        )?;
        self.processed_maker_count = self
            .processed_maker_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        self.validate_open()?;
        Ok(self)
    }

    /// Fold the sole treasury Position credit after every maker row. Executor
    /// allocation is explicitly absent in the current selected-record schema.
    pub fn fold_treasury_distribution<H: FeeRetirementHashV1, C: CertifiedRecipientAllocationAccessV3 + ?Sized>(
        mut self,
        certified: &C,
        transition: FeePositionCreditTransitionV1,
        hash: &H,
        selected: &SelectedCompositeFeeV2,
        recipient_intent: &RecipientAllocationIntentV1,
    ) -> Result<(Self, TreasuryDistributionAuthorizationV1)> {
        self.validate_open()?;
        transition.validate()?;
        if self.processed_owner_count != self.expected_owner_count
            || self.processed_maker_count != self.expected_maker_count
            || self.treasury_distributed
            || certified.owner_order_set_digest() != self.owner_order_set_digest
            || certified.fee_record() != self.fee_record
            || certified.executor_atoms() != 0
            || transition.position_account != self.treasury_position
            || transition.cash_pot_account != self.settlement_cash_pot
            || transition.credited_atoms != certified.treasury_atoms()
            || selected.fee_record() != self.fee_record
            || selected.selected_candidate() != self.settlement_candidate
            || selected.treasury_position() != self.treasury_position
            || recipient_intent.fee_record().identity() != self.fee_record
            || recipient_intent.recipient_allocation().identity()
                != self.recipient_allocation
            || recipient_intent.treasury_ledger().identity() != self.treasury_ledger
            || recipient_intent.settlement_candidate() != self.settlement_candidate
            || recipient_intent.revenue_policy() != selected.revenue_policy()
            || recipient_intent.treasury_position() != self.treasury_position
        {
            return Err(Error::MismatchedBinding);
        }
        self.value_disposition_fold = fold_value_transition(
            self.value_disposition_fold,
            self.processed_maker_count,
            2,
            transition,
            hash,
        )?;
        self.distributed_treasury_atoms = transition.credited_atoms;
        self.treasury_distributed = true;
        self.validate_open()?;
        let authority = TreasuryDistributionAuthorizationV1 {
            fee_record: self.fee_record,
            treasury_owner: selected.treasury_owner(),
            treasury_position: self.treasury_position,
            settlement_candidate: self.settlement_candidate,
            revenue_policy: selected.revenue_policy(),
            credited_atoms: transition.credited_atoms,
            value_disposition_receipt: self.value_disposition_fold,
        };
        Ok((self, authority))
    }

    /// Seal the exact global closure set after every owner row has matched the
    /// complete-book commitment.
    pub fn complete<H: FeeRetirementHashV1, C: CertifiedRecipientAllocationAccessV3 + ?Sized>(
        self,
        accumulator_closure: ExternalFeeAccountClosureV1,
        global: CandidateFeeAccountClosuresV1,
        certified: &C,
        hash: &H,
    ) -> Result<CompletedFeeRetirementV1> {
        self.validate_open()?;
        if self.processed_owner_count != self.expected_owner_count
            || self.processed_fee_atoms != self.expected_fee_atoms
            || self.observed_owner_row_fold != self.expected_owner_row_fold
            || self.processed_maker_count != self.expected_maker_count
            || !self.treasury_distributed
            || certified.owner_order_set_digest() != self.owner_order_set_digest
            || certified.nonzero_weight_row_count() != self.expected_owner_count
            || certified.row_count() != self.expected_maker_count
            || self.distributed_maker_atoms != certified.maker_rebate_total()
            || self.distributed_treasury_atoms != certified.treasury_atoms()
            || certified.executor_atoms() != 0
        {
            return Err(Error::MissingParticipant);
        }
        validate_global_closures(&self, &global)?;
        validate_accumulator_closure(&self, &accumulator_closure)?;
        let accumulator_account = accumulator_closure.account();
        let global_fold = fold_global_closures(self.owner_closure_fold, &global, hash)?;
        let global_fold = fold_closure(
            global_fold,
            u8::try_from(MAX_FEE_ROWS_V1 + 3).map_err(|_| Error::InvalidWidth)?,
            &accumulator_closure,
            hash,
        )?;
        let closure_set_data_id = Id(hash.sha256(&[
            FEE_CLOSURE_SET_DATA_ID_DOMAIN_V1,
            &self.fee_record.0,
            &self.owner_fee_book_data_id.0,
            &global_fold.0,
            &[self.expected_owner_count],
        ]));
        let terminal_authority_receipt = Id(hash.sha256(&[
            FEE_RETIREMENT_AUTHORITY_DOMAIN_V1,
            &accumulator_account.0,
            &self.settlement_root.0,
            &self.selected_feed_data_id.0,
            &closure_set_data_id.0,
            &self.value_disposition_fold.0,
        ]));
        live(closure_set_data_id)?;
        live(terminal_authority_receipt)?;
        let global_refund = add(
            add(
                global.selected_record.rent_refund_lamports(),
                global.recipient_allocation.rent_refund_lamports(),
            )?,
            global.treasury_ledger.rent_refund_lamports(),
        )?;
        let global_neutral = add(
            add(
                global.selected_record.neutral_credit_lamports(),
                global.recipient_allocation.neutral_credit_lamports(),
            )?,
            global.treasury_ledger.neutral_credit_lamports(),
        )?;
        let total_refund = add(
            add(self.owner_refund_lamports, global_refund)?,
            accumulator_closure.rent_refund_lamports(),
        )?;
        let total_neutral = add(
            add(self.owner_neutral_credit_lamports, global_neutral)?,
            accumulator_closure.neutral_credit_lamports(),
        )?;
        Ok(CompletedFeeRetirementV1 {
            accumulator: self,
            closure_set_data_id,
            terminal_authority_receipt,
            payer_refund_lamports: total_refund,
            neutral_credit_lamports: total_neutral,
            accumulator_account,
            accumulator_close_receipt: accumulator_closure.close_receipt(),
            accumulator_rent_payer: accumulator_closure.rent_payer(),
            accumulator_neutral_sink: accumulator_closure.neutral_sink(),
            accumulator_refund_lamports: accumulator_closure.rent_refund_lamports(),
            accumulator_neutral_credit_lamports: accumulator_closure
                .neutral_credit_lamports(),
        })
    }

    fn validate_open(&self) -> Result<()> {
        for identity in [
            self.runtime_program,
            self.runtime_release,
            self.settlement_root,
            self.selected_feed_data_id,
            self.fee_record,
            self.settlement_candidate,
            self.recipient_allocation,
            self.recipient_allocation_data_id,
            self.treasury_ledger,
            self.settlement_cash_pot,
            self.treasury_position,
            self.owner_fee_book_data_id,
            self.owner_order_set_digest,
            self.expected_owner_row_fold,
            self.observed_owner_row_fold,
            self.owner_closure_fold,
            self.value_disposition_fold,
        ] {
            live(identity)?;
        }
        if self.expected_owner_count == 0
            || usize::from(self.expected_owner_count) > MAX_FEE_ROWS_V1
            || self.processed_owner_count > self.expected_owner_count
            || self.processed_maker_count > self.expected_maker_count
            || self.processed_fee_atoms > self.expected_fee_atoms
            || (self.processed_owner_count == 0) != self.prior_owner.is_zero()
            || (!self.treasury_distributed && self.distributed_treasury_atoms != 0)
        {
            return Err(Error::InvalidAccountData);
        }
        Ok(())
    }

    pub const fn runtime_program(&self) -> Id { self.runtime_program }
    pub const fn runtime_release(&self) -> Id { self.runtime_release }
    pub const fn settlement_root(&self) -> Id { self.settlement_root }
    pub const fn selected_feed_data_id(&self) -> Id { self.selected_feed_data_id }
    pub const fn fee_record(&self) -> Id { self.fee_record }
    pub const fn settlement_candidate(&self) -> Id { self.settlement_candidate }
    pub const fn recipient_allocation(&self) -> Id { self.recipient_allocation }
    pub const fn recipient_allocation_data_id(&self) -> Id { self.recipient_allocation_data_id }
    pub const fn treasury_ledger(&self) -> Id { self.treasury_ledger }
    pub const fn settlement_cash_pot(&self) -> Id { self.settlement_cash_pot }
    pub const fn treasury_position(&self) -> Id { self.treasury_position }
    pub const fn owner_fee_book_data_id(&self) -> Id { self.owner_fee_book_data_id }
    pub const fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    pub const fn expected_owner_row_fold(&self) -> Id { self.expected_owner_row_fold }
    pub const fn observed_owner_row_fold(&self) -> Id { self.observed_owner_row_fold }
    pub const fn owner_closure_fold(&self) -> Id { self.owner_closure_fold }
    pub const fn value_disposition_receipt(&self) -> Id { self.value_disposition_fold }
    pub const fn prior_owner(&self) -> Id { self.prior_owner }
    pub const fn expected_owner_count(&self) -> u8 { self.expected_owner_count }
    pub const fn processed_owner_count(&self) -> u8 { self.processed_owner_count }
    pub const fn expected_maker_count(&self) -> u8 { self.expected_maker_count }
    pub const fn processed_maker_count(&self) -> u8 { self.processed_maker_count }
    pub const fn treasury_distributed(&self) -> bool { self.treasury_distributed }
    pub const fn expected_fee_atoms(&self) -> u128 { self.expected_fee_atoms }
    pub const fn processed_fee_atoms(&self) -> u128 { self.processed_fee_atoms }
    pub const fn distributed_maker_atoms(&self) -> u64 { self.distributed_maker_atoms }
    pub const fn distributed_treasury_atoms(&self) -> u64 { self.distributed_treasury_atoms }
    pub const fn owner_refund_lamports(&self) -> u64 { self.owner_refund_lamports }
    pub const fn owner_neutral_credit_lamports(&self) -> u64 {
        self.owner_neutral_credit_lamports
    }

    /// Encode the exact fixed-width semantic body.
    pub fn encode(&self) -> Result<[u8; FEE_RETIREMENT_ACCUMULATOR_BODY_V1_BYTES]> {
        self.validate_open()?;
        let mut output = [0u8; FEE_RETIREMENT_ACCUMULATOR_BODY_V1_BYTES];
        let mut at = 0usize;
        put(&mut output, &mut at, &FEE_RETIREMENT_ACCUMULATOR_MAGIC_V1)?;
        put(
            &mut output,
            &mut at,
            &FEE_RETIREMENT_ACCUMULATOR_VERSION_V1.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut at,
            &[
                self.expected_owner_count,
                self.processed_owner_count,
                self.expected_maker_count,
                self.processed_maker_count,
                u8::from(self.treasury_distributed),
            ],
        )?;
        put(&mut output, &mut at, &[0])?;
        for identity in [
            self.runtime_program,
            self.runtime_release,
            self.settlement_root,
            self.selected_feed_data_id,
            self.fee_record,
            self.settlement_candidate,
            self.recipient_allocation,
            self.recipient_allocation_data_id,
            self.treasury_ledger,
            self.settlement_cash_pot,
            self.treasury_position,
            self.owner_fee_book_data_id,
            self.owner_order_set_digest,
            self.expected_owner_row_fold,
            self.observed_owner_row_fold,
            self.owner_closure_fold,
            self.value_disposition_fold,
            self.prior_owner,
        ] {
            put(&mut output, &mut at, &identity.0)?;
        }
        put(&mut output, &mut at, &self.expected_fee_atoms.to_le_bytes())?;
        put(&mut output, &mut at, &self.processed_fee_atoms.to_le_bytes())?;
        put(&mut output, &mut at, &self.distributed_maker_atoms.to_le_bytes())?;
        put(&mut output, &mut at, &self.distributed_treasury_atoms.to_le_bytes())?;
        put(&mut output, &mut at, &self.owner_refund_lamports.to_le_bytes())?;
        put(
            &mut output,
            &mut at,
            &self.owner_neutral_credit_lamports.to_le_bytes(),
        )?;
        if at != output.len() {
            return Err(Error::InvalidWidth);
        }
        Ok(output)
    }

    /// Decode hostile bytes only through the semantic invariant constructor.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != FEE_RETIREMENT_ACCUMULATOR_BODY_V1_BYTES
            || input[..8] != FEE_RETIREMENT_ACCUMULATOR_MAGIC_V1
            || u16::from_le_bytes([input[8], input[9]])
                != FEE_RETIREMENT_ACCUMULATOR_VERSION_V1
            || input[14] > 1
            || input[15] != 0
        {
            return Err(Error::InvalidAccountData);
        }
        let expected_owner_count = input[10];
        let processed_owner_count = input[11];
        let expected_maker_count = input[12];
        let processed_maker_count = input[13];
        let treasury_distributed = input[14] == 1;
        let mut at = 16usize;
        let runtime_program = take_id(input, &mut at)?;
        let runtime_release = take_id(input, &mut at)?;
        let settlement_root = take_id(input, &mut at)?;
        let selected_feed_data_id = take_id(input, &mut at)?;
        let fee_record = take_id(input, &mut at)?;
        let settlement_candidate = take_id(input, &mut at)?;
        let recipient_allocation = take_id(input, &mut at)?;
        let recipient_allocation_data_id = take_id(input, &mut at)?;
        let treasury_ledger = take_id(input, &mut at)?;
        let settlement_cash_pot = take_id(input, &mut at)?;
        let treasury_position = take_id(input, &mut at)?;
        let owner_fee_book_data_id = take_id(input, &mut at)?;
        let owner_order_set_digest = take_id(input, &mut at)?;
        let expected_owner_row_fold = take_id(input, &mut at)?;
        let observed_owner_row_fold = take_id(input, &mut at)?;
        let owner_closure_fold = take_id(input, &mut at)?;
        let value_disposition_fold = take_id(input, &mut at)?;
        let prior_owner = take_id(input, &mut at)?;
        let expected_fee_atoms = take_u128(input, &mut at)?;
        let processed_fee_atoms = take_u128(input, &mut at)?;
        let distributed_maker_atoms = take_u64(input, &mut at)?;
        let distributed_treasury_atoms = take_u64(input, &mut at)?;
        let owner_refund_lamports = take_u64(input, &mut at)?;
        let owner_neutral_credit_lamports = take_u64(input, &mut at)?;
        if at != input.len() {
            return Err(Error::InvalidWidth);
        }
        Self::restore(
            runtime_program,
            runtime_release,
            settlement_root,
            selected_feed_data_id,
            fee_record,
            settlement_candidate,
            recipient_allocation,
            recipient_allocation_data_id,
            treasury_ledger,
            settlement_cash_pot,
            treasury_position,
            owner_fee_book_data_id,
            owner_order_set_digest,
            expected_owner_row_fold,
            observed_owner_row_fold,
            owner_closure_fold,
            value_disposition_fold,
            prior_owner,
            expected_owner_count,
            processed_owner_count,
            expected_maker_count,
            processed_maker_count,
            treasury_distributed,
            expected_fee_atoms,
            processed_fee_atoms,
            distributed_maker_atoms,
            distributed_treasury_atoms,
            owner_refund_lamports,
            owner_neutral_credit_lamports,
        )
    }
}

impl CompletedFeeRetirementV1 {
    pub const fn accumulator(&self) -> &FeeRetirementAccumulatorV1 {
        &self.accumulator
    }

    pub const fn closure_set_data_id(&self) -> Id {
        self.closure_set_data_id
    }

    pub const fn terminal_authority_receipt(&self) -> Id {
        self.terminal_authority_receipt
    }

    pub const fn payer_refund_lamports(&self) -> u64 {
        self.payer_refund_lamports
    }

    pub const fn neutral_credit_lamports(&self) -> u64 {
        self.neutral_credit_lamports
    }

    pub const fn accumulator_account(&self) -> Id {
        self.accumulator_account
    }

    pub const fn accumulator_close_receipt(&self) -> Id {
        self.accumulator_close_receipt
    }

    pub const fn accumulator_rent_payer(&self) -> Id {
        self.accumulator_rent_payer
    }

    pub const fn accumulator_neutral_sink(&self) -> Id {
        self.accumulator_neutral_sink
    }

    pub const fn accumulator_refund_lamports(&self) -> u64 {
        self.accumulator_refund_lamports
    }

    pub const fn accumulator_neutral_credit_lamports(&self) -> u64 {
        self.accumulator_neutral_credit_lamports
    }
}

fn owner_book_stream_start_v2<H: FeeRetirementHashV1>(
    selected: &SelectedCompositeFeeV2,
    owner_order_set_digest: Id,
    owner_count: u8,
    fee_atoms: u128,
    hash: &H,
) -> Result<Id> {
    let value = Id(hash.sha256(&[
        FEE_OWNER_BOOK_STREAM_START_DOMAIN_V2,
        &selected.fee_record().0,
        &selected.selected_candidate().0,
        &selected.revenue_policy().0,
        &owner_order_set_digest.0,
        &[owner_count],
        &fee_atoms.to_le_bytes(),
    ]));
    live(value)?;
    Ok(value)
}

fn closure_fold_start<H: FeeRetirementHashV1>(fee_record: Id, hash: &H) -> Result<Id> {
    let value = Id(hash.sha256(&[FEE_OWNER_CLOSURE_FOLD_DOMAIN_V1, &fee_record.0]));
    live(value)?;
    Ok(value)
}

fn fold_owner_book_row_v2<H: FeeRetirementHashV1>(
    prior: Id,
    ordinal: u8,
    owner: Id,
    fee_atoms: u64,
    hash: &H,
) -> Result<Id> {
    live(prior)?;
    live(owner)?;
    let value = Id(hash.sha256(&[
        FEE_OWNER_BOOK_STREAM_ROW_DOMAIN_V2,
        &prior.0,
        &[ordinal],
        &owner.0,
        &fee_atoms.to_le_bytes(),
    ]));
    live(value)?;
    Ok(value)
}

fn value_fold_start<H: FeeRetirementHashV1>(
    fee_record: Id,
    recipient_allocation_data_id: Id,
    cash_pot: Id,
    hash: &H,
) -> Result<Id> {
    let value = Id(hash.sha256(&[
        FEE_VALUE_DISPOSITION_FOLD_DOMAIN_V1,
        &fee_record.0,
        &recipient_allocation_data_id.0,
        &cash_pot.0,
    ]));
    live(value)?;
    Ok(value)
}

fn fold_value_transition<H: FeeRetirementHashV1>(
    prior: Id,
    ordinal: u8,
    recipient_kind: u8,
    transition: FeePositionCreditTransitionV1,
    hash: &H,
) -> Result<Id> {
    let value = Id(hash.sha256(&[
        FEE_VALUE_DISPOSITION_FOLD_DOMAIN_V1,
        &prior.0,
        &[recipient_kind, ordinal],
        &transition.position_account.0,
        &transition.replay_account.0,
        &transition.position_prestate.0,
        &transition.position_poststate.0,
        &transition.replay_prestate.0,
        &transition.replay_poststate.0,
        &transition.cash_pot_account.0,
        &transition.cash_pot_prestate.0,
        &transition.cash_pot_poststate.0,
        &transition.credited_atoms.to_le_bytes(),
    ]));
    live(value)?;
    Ok(value)
}

fn fold_closure<H: FeeRetirementHashV1>(
    prior: Id,
    ordinal: u8,
    closure: &ExternalFeeAccountClosureV1,
    hash: &H,
) -> Result<Id> {
    let value = Id(hash.sha256(&[
        FEE_OWNER_CLOSURE_FOLD_DOMAIN_V1,
        &prior.0,
        &[ordinal, closure.role() as u8, closure.outcome() as u8],
        &closure.account().0,
        &closure.semantic_owner().0,
        &closure.close_receipt().0,
        &closure.rent_payer().0,
        &closure.neutral_sink().0,
        &closure.balance_before_lamports().to_le_bytes(),
        &closure.rent_refund_lamports().to_le_bytes(),
        &closure.neutral_credit_lamports().to_le_bytes(),
    ]));
    live(value)?;
    Ok(value)
}

fn fold_global_closures<H: FeeRetirementHashV1>(
    mut prior: Id,
    global: &CandidateFeeAccountClosuresV1,
    hash: &H,
) -> Result<Id> {
    for (ordinal, closure) in [
        &global.selected_record,
        &global.recipient_allocation,
        &global.treasury_ledger,
    ]
    .iter()
    .enumerate()
    {
        prior = fold_closure(
            prior,
            u8::try_from(ordinal + MAX_FEE_ROWS_V1).map_err(|_| Error::InvalidWidth)?,
            closure,
            hash,
        )?;
    }
    Ok(prior)
}

fn validate_global_closures(
    accumulator: &FeeRetirementAccumulatorV1,
    global: &CandidateFeeAccountClosuresV1,
) -> Result<()> {
    let expected = [
        (
            &global.selected_record,
            CandidateFeeAccountRoleV1::SelectedFeeRecord,
            accumulator.fee_record,
        ),
        (
            &global.recipient_allocation,
            CandidateFeeAccountRoleV1::RecipientAllocation,
            accumulator.recipient_allocation,
        ),
        (
            &global.treasury_ledger,
            CandidateFeeAccountRoleV1::TreasuryLedger,
            accumulator.treasury_ledger,
        ),
    ];
    for (closure, role, account) in expected {
        if closure.role() != role
            || closure.outcome() != FeeTerminalOutcomeV1::Settled
            || closure.runtime_program() != accumulator.runtime_program
            || closure.runtime_release() != accumulator.runtime_release
            || closure.fee_record() != accumulator.fee_record
            || closure.account() != account
            || !closure.semantic_owner().is_zero()
        {
            return Err(Error::MissingClosure);
        }
    }
    if global.selected_record.account() == global.recipient_allocation.account()
        || global.selected_record.account() == global.treasury_ledger.account()
        || global.recipient_allocation.account() == global.treasury_ledger.account()
        || global.selected_record.close_receipt() == global.recipient_allocation.close_receipt()
        || global.selected_record.close_receipt() == global.treasury_ledger.close_receipt()
        || global.recipient_allocation.close_receipt() == global.treasury_ledger.close_receipt()
    {
        return Err(Error::DuplicateIdentity);
    }
    Ok(())
}

fn validate_accumulator_closure(
    accumulator: &FeeRetirementAccumulatorV1,
    closure: &ExternalFeeAccountClosureV1,
) -> Result<()> {
    if closure.role() != CandidateFeeAccountRoleV1::RetirementAccumulator
        || closure.outcome() != FeeTerminalOutcomeV1::Settled
        || closure.runtime_program() != accumulator.runtime_program
        || closure.runtime_release() != accumulator.runtime_release
        || closure.fee_record() != accumulator.fee_record
        || !closure.semantic_owner().is_zero()
    {
        return Err(Error::MissingClosure);
    }
    Ok(())
}

fn put(output: &mut [u8], at: &mut usize, value: &[u8]) -> Result<()> {
    let end = at.checked_add(value.len()).ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(*at..end)
        .ok_or(Error::InvalidWidth)?
        .copy_from_slice(value);
    *at = end;
    Ok(())
}

fn take_id(input: &[u8], at: &mut usize) -> Result<Id> {
    let mut output = [0u8; 32];
    take(input, at, &mut output)?;
    Ok(Id(output))
}

fn take_u64(input: &[u8], at: &mut usize) -> Result<u64> {
    let mut output = [0u8; 8];
    take(input, at, &mut output)?;
    Ok(u64::from_le_bytes(output))
}

fn take_u128(input: &[u8], at: &mut usize) -> Result<u128> {
    let mut output = [0u8; 16];
    take(input, at, &mut output)?;
    Ok(u128::from_le_bytes(output))
}

fn take<const N: usize>(input: &[u8], at: &mut usize, output: &mut [u8; N]) -> Result<()> {
    let end = at.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    output.copy_from_slice(input.get(*at..end).ok_or(Error::InvalidWidth)?);
    *at = end;
    Ok(())
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
    use clutch_batch_policy_identity::revenue_policy_v2::RevenuePolicyV2;

    #[derive(Clone, Copy)]
    struct ToyHash;
    impl SelectedOwnerFeeBookHashV1 for ToyHash {
        fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
            FeeRetirementHashV1::sha256(self, &[domain, body])
        }
    }
    impl FeeRetirementHashV1 for ToyHash {
        fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
            let mut out = [0u8; 32];
            for part in parts {
                for (index, byte) in part.iter().enumerate() {
                    out[index % 32] = out[index % 32].wrapping_mul(31).wrapping_add(*byte);
                }
            }
            if out == [0; 32] { out[0] = 1; }
            out
        }
    }

    fn selected_v2() -> SelectedCompositeFeeV2 {
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
        SelectedCompositeFeeV2::select(
            Id([1; 32]),
            Id([2; 32]),
            Id([3; 32]),
            Id([4; 32]),
            Id([5; 32]),
            Id([6; 32]),
            10_000,
            2,
            &batch,
            &RevenuePolicyV2::successor_development([9; 32]),
        )
        .unwrap()
    }

    #[test]
    fn row_fold_is_order_and_amount_sensitive() {
        let start = Id([1; 32]);
        let first = fold_owner_book_row_v2(start, 0, Id([2; 32]), 7, &ToyHash).unwrap();
        assert_ne!(
            first,
            fold_owner_book_row_v2(start, 0, Id([2; 32]), 8, &ToyHash).unwrap()
        );
        assert_ne!(
            first,
            fold_owner_book_row_v2(start, 1, Id([2; 32]), 7, &ToyHash).unwrap()
        );
        assert_ne!(
            first,
            fold_owner_book_row_v2(start, 0, Id([3; 32]), 7, &ToyHash).unwrap()
        );
    }

    #[test]
    fn streaming_book_requires_exact_order_count_and_sum() {
        let selected = selected_v2();
        let complete = StreamingOwnerFeeBookV2::begin(
            &selected,
            Id([20; 32]),
            2,
            12,
            &ToyHash,
        )
        .unwrap()
        .fold(Id([21; 32]), 5, &ToyHash)
        .unwrap()
        .fold(Id([22; 32]), 7, &ToyHash)
        .unwrap()
        .complete(&ToyHash)
        .unwrap();
        assert_eq!(complete.owner_count(), 2);
        assert_eq!(complete.fee_atoms(), 12);

        let wrong_order = StreamingOwnerFeeBookV2::begin(
            &selected,
            Id([20; 32]),
            2,
            12,
            &ToyHash,
        )
        .unwrap()
        .fold(Id([22; 32]), 5, &ToyHash)
        .unwrap()
        .fold(Id([21; 32]), 7, &ToyHash);
        assert_eq!(wrong_order, Err(Error::NonCanonicalOrder));

        let wrong_sum = StreamingOwnerFeeBookV2::begin(
            &selected,
            Id([20; 32]),
            1,
            12,
            &ToyHash,
        )
        .unwrap()
        .fold(Id([21; 32]), 11, &ToyHash)
        .unwrap()
        .complete(&ToyHash);
        assert_eq!(wrong_sum, Err(Error::MissingParticipant));
    }

    #[test]
    fn accumulator_decoder_refuses_noncanonical_header_padding() {
        let mut bytes = [0u8; FEE_RETIREMENT_ACCUMULATOR_BODY_V1_BYTES];
        bytes[..8].copy_from_slice(&FEE_RETIREMENT_ACCUMULATOR_MAGIC_V1);
        bytes[8..10].copy_from_slice(&FEE_RETIREMENT_ACCUMULATOR_VERSION_V1.to_le_bytes());
        bytes[10] = 1;
        bytes[12] = 1;
        assert_eq!(
            FeeRetirementAccumulatorV1::decode(&bytes),
            Err(Error::InvalidAccountData)
        );
    }
}
