//! Compact, streaming retirement authority for one selected fee record.
//!
//! The complete owner fee book is authenticated once when this accumulator is
//! created.  Owner finalizations can then be consumed in canonical owner order
//! without retaining all 64 temporary accounts in one transaction.  The two
//! rolling commitments prevent a sequential close from degrading the terminal
//! contract to a count-and-sum check.

use crate::projection::{
    CertifiedRecipientAllocationV2, SelectedOwnerFeeBookHashV1, SelectedOwnerFeeBookV1,
};
use crate::selected::SelectedCompositeFeeV1;
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
/// Exact semantic body width of the streaming accumulator.
pub const FEE_RETIREMENT_ACCUMULATOR_BODY_V1_BYTES: usize = 544;
/// Canonical semantic discriminator of the streaming accumulator.
pub const FEE_RETIREMENT_ACCUMULATOR_MAGIC_V1: [u8; 8] = *b"DCFEEACC";
/// Canonical semantic body version.
pub const FEE_RETIREMENT_ACCUMULATOR_VERSION_V1: u16 = 1;

/// Minimal exact SHA-256 seam used by the streaming fee-retirement owner.
pub trait FeeRetirementHashV1: SelectedOwnerFeeBookHashV1 {
    /// SHA-256 over the exact ordered byte slices.
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32];
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
    owner_fee_book_data_id: Id,
    owner_order_set_digest: Id,
    expected_owner_row_fold: Id,
    observed_owner_row_fold: Id,
    owner_closure_fold: Id,
    prior_owner: Id,
    expected_owner_count: u8,
    processed_owner_count: u8,
    expected_fee_atoms: u128,
    processed_fee_atoms: u128,
    owner_refund_lamports: u64,
    owner_neutral_credit_lamports: u64,
}

/// Complete, non-forgeable authority consumed by candidate-wide terminal
/// construction after all owner finalizations have been closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletedFeeRetirementV1 {
    accumulator: FeeRetirementAccumulatorV1,
    closure_set_data_id: Id,
    terminal_authority_receipt: Id,
    payer_refund_lamports: u64,
    neutral_credit_lamports: u64,
}

impl FeeRetirementAccumulatorV1 {
    /// Create the streaming authority from the exact complete owner fee book
    /// and the certified recipient allocation produced from that same book.
    #[allow(clippy::too_many_arguments)]
    pub fn begin<H: FeeRetirementHashV1>(
        runtime_program: Id,
        runtime_release: Id,
        settlement_root: Id,
        selected_feed_data_id: Id,
        recipient_allocation: Id,
        recipient_allocation_data_id: Id,
        treasury_ledger: Id,
        selected: &SelectedCompositeFeeV1,
        book: &SelectedOwnerFeeBookV1,
        certified: &CertifiedRecipientAllocationV2,
        owner_order_set_digest: Id,
        hash: &H,
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
            book.owner_fee_book_data_id(hash)?,
            owner_order_set_digest,
        ])?;
        let allocation = certified.allocation();
        if book.fee_record() != selected.fee_record()
            || book.settlement_candidate() != selected.selected_candidate()
            || book.revenue_policy() != selected.revenue_policy()
            || certified.owner_fee_book_data_id() != book.owner_fee_book_data_id(hash)?
            || certified.owner_order_set_digest() != owner_order_set_digest
            || certified.owner_count() != u16::from(book.owner_count())
            || allocation.fee_record() != selected.fee_record()
            || u128::from(allocation.collected_fee_atoms()) != book.selected_fee_atoms()
        {
            return Err(Error::MismatchedBinding);
        }
        let expected_owner_row_fold = fold_book(book, hash)?;
        let observed_owner_row_fold = fold_start(book.owner_fee_book_data_id(hash)?, hash)?;
        let owner_closure_fold = closure_fold_start(selected.fee_record(), hash)?;
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
            owner_fee_book_data_id: book.owner_fee_book_data_id(hash)?,
            owner_order_set_digest,
            expected_owner_row_fold,
            observed_owner_row_fold,
            owner_closure_fold,
            prior_owner: Id([0; 32]),
            expected_owner_count: book.owner_count(),
            processed_owner_count: 0,
            expected_fee_atoms: book.selected_fee_atoms(),
            processed_fee_atoms: 0,
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
        owner_fee_book_data_id: Id,
        owner_order_set_digest: Id,
        expected_owner_row_fold: Id,
        observed_owner_row_fold: Id,
        owner_closure_fold: Id,
        prior_owner: Id,
        expected_owner_count: u8,
        processed_owner_count: u8,
        expected_fee_atoms: u128,
        processed_fee_atoms: u128,
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
            owner_fee_book_data_id,
            owner_order_set_digest,
            expected_owner_row_fold,
            observed_owner_row_fold,
            owner_closure_fold,
            prior_owner,
            expected_owner_count,
            processed_owner_count,
            expected_fee_atoms,
            processed_fee_atoms,
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
        self.observed_owner_row_fold = fold_row(
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

    /// Seal the exact global closure set after every owner row has matched the
    /// complete-book commitment.
    pub fn complete<H: FeeRetirementHashV1>(
        self,
        accumulator_account: Id,
        global: CandidateFeeAccountClosuresV1,
        hash: &H,
    ) -> Result<CompletedFeeRetirementV1> {
        self.validate_open()?;
        if self.processed_owner_count != self.expected_owner_count
            || self.processed_fee_atoms != self.expected_fee_atoms
            || self.observed_owner_row_fold != self.expected_owner_row_fold
        {
            return Err(Error::MissingParticipant);
        }
        validate_global_closures(&self, &global)?;
        live(accumulator_account)?;
        let global_fold = fold_global_closures(self.owner_closure_fold, &global, hash)?;
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
        Ok(CompletedFeeRetirementV1 {
            accumulator: self,
            closure_set_data_id,
            terminal_authority_receipt,
            payer_refund_lamports: add(self.owner_refund_lamports, global_refund)?,
            neutral_credit_lamports: add(
                self.owner_neutral_credit_lamports,
                global_neutral,
            )?,
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
            self.owner_fee_book_data_id,
            self.owner_order_set_digest,
            self.expected_owner_row_fold,
            self.observed_owner_row_fold,
            self.owner_closure_fold,
        ] {
            live(identity)?;
        }
        if self.expected_owner_count == 0
            || usize::from(self.expected_owner_count) > MAX_FEE_ROWS_V1
            || self.processed_owner_count > self.expected_owner_count
            || self.processed_fee_atoms > self.expected_fee_atoms
            || (self.processed_owner_count == 0) != self.prior_owner.is_zero()
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
    pub const fn owner_fee_book_data_id(&self) -> Id { self.owner_fee_book_data_id }
    pub const fn owner_order_set_digest(&self) -> Id { self.owner_order_set_digest }
    pub const fn expected_owner_row_fold(&self) -> Id { self.expected_owner_row_fold }
    pub const fn observed_owner_row_fold(&self) -> Id { self.observed_owner_row_fold }
    pub const fn owner_closure_fold(&self) -> Id { self.owner_closure_fold }
    pub const fn prior_owner(&self) -> Id { self.prior_owner }
    pub const fn expected_owner_count(&self) -> u8 { self.expected_owner_count }
    pub const fn processed_owner_count(&self) -> u8 { self.processed_owner_count }
    pub const fn expected_fee_atoms(&self) -> u128 { self.expected_fee_atoms }
    pub const fn processed_fee_atoms(&self) -> u128 { self.processed_fee_atoms }
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
            &[self.expected_owner_count, self.processed_owner_count],
        )?;
        put(&mut output, &mut at, &[0; 4])?;
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
            self.owner_fee_book_data_id,
            self.owner_order_set_digest,
            self.expected_owner_row_fold,
            self.observed_owner_row_fold,
            self.owner_closure_fold,
            self.prior_owner,
        ] {
            put(&mut output, &mut at, &identity.0)?;
        }
        put(&mut output, &mut at, &self.expected_fee_atoms.to_le_bytes())?;
        put(&mut output, &mut at, &self.processed_fee_atoms.to_le_bytes())?;
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
            || input[12..16] != [0; 4]
        {
            return Err(Error::InvalidAccountData);
        }
        let expected_owner_count = input[10];
        let processed_owner_count = input[11];
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
        let owner_fee_book_data_id = take_id(input, &mut at)?;
        let owner_order_set_digest = take_id(input, &mut at)?;
        let expected_owner_row_fold = take_id(input, &mut at)?;
        let observed_owner_row_fold = take_id(input, &mut at)?;
        let owner_closure_fold = take_id(input, &mut at)?;
        let prior_owner = take_id(input, &mut at)?;
        let expected_fee_atoms = take_u128(input, &mut at)?;
        let processed_fee_atoms = take_u128(input, &mut at)?;
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
            owner_fee_book_data_id,
            owner_order_set_digest,
            expected_owner_row_fold,
            observed_owner_row_fold,
            owner_closure_fold,
            prior_owner,
            expected_owner_count,
            processed_owner_count,
            expected_fee_atoms,
            processed_fee_atoms,
            owner_refund_lamports,
            owner_neutral_credit_lamports,
        )
    }
}

impl CompletedFeeRetirementV1 {
    pub const fn accumulator(&self) -> FeeRetirementAccumulatorV1 { self.accumulator }
    pub const fn closure_set_data_id(&self) -> Id { self.closure_set_data_id }
    pub const fn terminal_authority_receipt(&self) -> Id { self.terminal_authority_receipt }
    pub const fn payer_refund_lamports(&self) -> u64 { self.payer_refund_lamports }
    pub const fn neutral_credit_lamports(&self) -> u64 { self.neutral_credit_lamports }
}

fn fold_start<H: FeeRetirementHashV1>(book_data_id: Id, hash: &H) -> Result<Id> {
    let value = Id(hash.sha256(&[FEE_OWNER_ROW_FOLD_DOMAIN_V1, &book_data_id.0]));
    live(value)?;
    Ok(value)
}

fn closure_fold_start<H: FeeRetirementHashV1>(fee_record: Id, hash: &H) -> Result<Id> {
    let value = Id(hash.sha256(&[FEE_OWNER_CLOSURE_FOLD_DOMAIN_V1, &fee_record.0]));
    live(value)?;
    Ok(value)
}

fn fold_row<H: FeeRetirementHashV1>(
    prior: Id,
    ordinal: u8,
    owner: Id,
    fee_atoms: u64,
    hash: &H,
) -> Result<Id> {
    live(prior)?;
    live(owner)?;
    let value = Id(hash.sha256(&[
        FEE_OWNER_ROW_FOLD_DOMAIN_V1,
        &prior.0,
        &[ordinal],
        &owner.0,
        &fee_atoms.to_le_bytes(),
    ]));
    live(value)?;
    Ok(value)
}

fn fold_book<H: FeeRetirementHashV1>(book: &SelectedOwnerFeeBookV1, hash: &H) -> Result<Id> {
    let mut fold = fold_start(book.owner_fee_book_data_id(hash)?, hash)?;
    let mut ordinal = 0u8;
    while ordinal < book.owner_count() {
        let row = book.rows()[usize::from(ordinal)];
        fold = fold_row(fold, ordinal, Id(row.owner), row.fee_atoms, hash)?;
        ordinal = ordinal.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(fold)
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

    #[test]
    fn row_fold_is_order_and_amount_sensitive() {
        let start = fold_start(Id([1; 32]), &ToyHash).unwrap();
        let first = fold_row(start, 0, Id([2; 32]), 7, &ToyHash).unwrap();
        assert_ne!(first, fold_row(start, 0, Id([2; 32]), 8, &ToyHash).unwrap());
        assert_ne!(first, fold_row(start, 1, Id([2; 32]), 7, &ToyHash).unwrap());
        assert_ne!(first, fold_row(start, 0, Id([3; 32]), 7, &ToyHash).unwrap());
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
