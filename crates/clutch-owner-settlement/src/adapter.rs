//! Account-neutral adapter contracts for canonical owner settlement.
//!
//! This crate does not derive Solana PDAs or borrow account memory. The SBF
//! adapter must derive every [`AdapterDerivedPdaV1`] from the documented seed
//! preimage, authenticate the outer General tag/version, stage every returned
//! poststate, and commit a complete plan atomically.

use crate::{
    Amount, AuthenticatedOwnerFragmentV1, AuthenticatedPositionV3, Error,
    OwnerSettlementAccumulatorV1, OwnerSettlementDispositionV1,
    OwnerSettlementExpectationV1, PositionSettlementPoststateV3, Result, SettlementSideV1,
    OWNER_SETTLEMENT_BODY_V1_BYTES,
};

/// Ordered PDA domain for one General V2 owner-settlement row.
pub const OWNER_SETTLEMENT_PDA_DOMAIN_V1: &[u8] = b"owner-settlement:v1";
/// Exact semantic body width of the candidate-wide settlement cash pot.
pub const SETTLEMENT_CASH_POT_BODY_V1_BYTES: usize = 256;

const BUY_END_MASK: u8 = 1;
const SELL_END_MASK: u8 = 2;

/// Direction of the selected candidate's sole canonical virtual cash leg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum VirtualCashDirectionV1 {
    /// The selected candidate has no virtual complete-set conversion.
    None = 0,
    /// Buyer consideration leaves terminal cash that must fund a split.
    Split = 1,
    /// A completed merge contributes opening cash before seller realization.
    Merge = 2,
}

impl VirtualCashDirectionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Split),
            2 => Ok(Self::Merge),
            _ => Err(Error::InvalidExpectation),
        }
    }
}

/// A canonical PDA derivation performed by the trusted SBF adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AdapterDerivedPdaV1 {
    /// Program owning this seed domain.
    pub program_id: [u8; 32],
    /// Derived address.
    pub address: [u8; 32],
    /// Parent Epoch PDA seed.
    pub epoch: [u8; 32],
    /// Final selected candidate identity seed.
    pub candidate: [u8; 32],
    /// Semantic Position owner seed.
    pub owner: [u8; 32],
    /// Canonical bump returned by derivation.
    pub bump: u8,
}

impl AdapterDerivedPdaV1 {
    fn validate(self) -> Result<()> {
        if self.program_id == [0; 32]
            || self.address == [0; 32]
            || self.epoch == [0; 32]
            || self.candidate == [0; 32]
            || self.owner == [0; 32]
        {
            return Err(Error::InvalidAccount);
        }
        Ok(())
    }
}

/// Outer account facts authenticated by the General V2 SBF adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementAccountViewV1<'a> {
    /// Presented account address.
    pub address: [u8; 32],
    /// Presented account owner.
    pub program_owner: [u8; 32],
    /// Whether the account meta is writable.
    pub writable: bool,
    /// Bump stored in the centrally owned General header.
    pub stored_bump: u8,
    /// Current lamport balance.
    pub lamports: u64,
    /// Exact rent-exempt minimum for the current data length.
    pub rent_minimum: u64,
    /// Exact semantic body after outer tag/version authentication.
    pub body: &'a [u8],
}

/// Owned, authenticated owner-settlement projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedOwnerSettlementAccountV1 {
    /// Canonical row PDA.
    pub address: [u8; 32],
    /// Dragon's Clutch program id.
    pub program_id: [u8; 32],
    /// Current lamports retained for later rent disposition.
    pub lamports: u64,
    /// Current exact rent minimum.
    pub rent_minimum: u64,
    /// Decoded canonical accumulator.
    pub accumulator: OwnerSettlementAccumulatorV1,
}

/// Authenticate an existing row against its PDA, owner, rent, and body.
pub fn authenticate_owner_settlement_account_v1(
    view: OwnerSettlementAccountViewV1<'_>,
    derived: AdapterDerivedPdaV1,
) -> Result<AuthenticatedOwnerSettlementAccountV1> {
    derived.validate()?;
    if !view.writable
        || view.address != derived.address
        || view.program_owner != derived.program_id
        || view.stored_bump != derived.bump
        || view.lamports < view.rent_minimum
        || view.body.len() != OWNER_SETTLEMENT_BODY_V1_BYTES
    {
        return Err(Error::InvalidAccount);
    }
    let accumulator = OwnerSettlementAccumulatorV1::decode_body(view.body)?;
    if accumulator.expectation.epoch != derived.epoch
        || accumulator.expectation.candidate != derived.candidate
        || accumulator.expectation.owner != derived.owner
    {
        return Err(Error::InvalidAccount);
    }
    Ok(AuthenticatedOwnerSettlementAccountV1 {
        address: view.address,
        program_id: derived.program_id,
        lamports: view.lamports,
        rent_minimum: view.rent_minimum,
        accumulator,
    })
}

/// Selected-candidate authority for one lexicographically sorted builder row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SelectedOwnerRowAuthorityV1 {
    /// SelectedCandidate account PDA.
    pub selected_candidate_account: [u8; 32],
    /// Canonical expectation emitted by the complete owner builder.
    pub expectation: OwnerSettlementExpectationV1,
    /// Zero-based row ordinal.
    pub row_ordinal: u16,
    /// Exact owner-row count.
    pub owner_count: u16,
    /// Present payer selected by the settlement funding authority.
    pub rent_payer: [u8; 32],
    /// Sole eventual refund recipient selected by that authority.
    pub rent_refund_recipient: [u8; 32],
    /// Persisted Budget/rent-ledger account that owns the funding split.
    pub rent_ledger: [u8; 32],
    /// Canonical sink for unsolicited prefunding and later donations.
    pub donation_sink: [u8; 32],
}

impl SelectedOwnerRowAuthorityV1 {
    fn validate(self) -> Result<()> {
        self.expectation.validate()?;
        if self.selected_candidate_account == [0; 32]
            || self.owner_count == 0
            || self.row_ordinal >= self.owner_count
            || self.rent_payer == [0; 32]
            || self.rent_refund_recipient == [0; 32]
            || self.rent_ledger == [0; 32]
            || self.donation_sink == [0; 32]
            || self.rent_ledger == self.donation_sink
            || self.rent_ledger == self.selected_candidate_account
            || self.rent_ledger == self.rent_payer
            || self.rent_ledger == self.rent_refund_recipient
            || self.donation_sink == self.selected_candidate_account
            || self.donation_sink == self.rent_payer
            || self.donation_sink == self.rent_refund_recipient
            || self.selected_candidate_account == self.expectation.market
            || self.selected_candidate_account == self.expectation.epoch
            || self.selected_candidate_account == self.expectation.candidate
            || self.selected_candidate_account == self.expectation.owner
            || self.selected_candidate_account == self.expectation.owner_order_set_digest
        {
            return Err(Error::AuthorityUnavailable);
        }
        Ok(())
    }
}

/// Pre-fund-safe rent facts for owner-row creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementCreateFundingV1 {
    /// Present payer of only the missing rent shortfall.
    pub payer: [u8; 32],
    /// Semantic owner of the eventual rent refund.
    pub refund_recipient: [u8; 32],
    /// Payer's present lamport balance.
    pub payer_lamports: u64,
    /// Lamports already parked at the derived address.
    pub target_lamports_before: u64,
    /// Current owner of the zero-data target account.
    pub target_owner_before: [u8; 32],
    /// Canonical System Program id authenticated by the adapter.
    pub system_program_id: [u8; 32],
    /// Current target data length; fresh creation requires zero.
    pub target_data_len_before: u32,
    /// Whether the derived target was presented writable.
    pub target_writable: bool,
    /// Executable targets can never enter account creation.
    pub target_executable: bool,
    /// Rent-exempt minimum for the final General account length.
    pub rent_minimum: u64,
}

/// Atomic creation plan. Preexisting lamports grant no authority or refund.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementCreatePlanV1 {
    /// Account to allocate and assign.
    pub address: [u8; 32],
    /// Program owner after assignment.
    pub program_id: [u8; 32],
    /// Canonical stored bump.
    pub bump: u8,
    /// Present payer debit.
    pub payer_debit_lamports: u64,
    /// Final target lamports.
    pub target_lamports_after: u64,
    /// Sole eventual refund recipient.
    pub refund_recipient: [u8; 32],
    /// Persisted ledger that must record this exact ownership split.
    pub rent_ledger: [u8; 32],
    /// Authorized payer-funded rent principal, and the maximum refund.
    pub payer_rent_principal_lamports: u64,
    /// Unsolicited prefunding locked to the canonical donation sink.
    pub prefunded_donation_lamports: u64,
    /// Canonical recipient of unsolicited prefunding and later donations.
    pub donation_sink: [u8; 32],
    /// Canonical initial semantic body.
    pub body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES],
}

/// Prepare canonical row creation without rewarding an address pre-funder.
pub fn prepare_create_owner_settlement_account_v1(
    authority: SelectedOwnerRowAuthorityV1,
    derived: AdapterDerivedPdaV1,
    funding: OwnerSettlementCreateFundingV1,
) -> Result<OwnerSettlementCreatePlanV1> {
    authority.validate()?;
    derived.validate()?;
    if derived.epoch != authority.expectation.epoch
        || derived.candidate != authority.expectation.candidate
        || derived.owner != authority.expectation.owner
        || funding.payer == [0; 32]
        || funding.refund_recipient == [0; 32]
        || funding.payer != authority.rent_payer
        || funding.refund_recipient != authority.rent_refund_recipient
        || funding.payer == derived.address
        || funding.refund_recipient == derived.address
        || authority.rent_ledger == derived.address
        || authority.donation_sink == derived.address
        || funding.system_program_id == [0; 32]
        || funding.system_program_id == derived.program_id
        || funding.target_owner_before != funding.system_program_id
        || funding.target_data_len_before != 0
        || !funding.target_writable
        || funding.target_executable
        || funding.rent_minimum == 0
    {
        return Err(Error::InvalidAccount);
    }
    let payer_debit_lamports = funding
        .rent_minimum
        .saturating_sub(funding.target_lamports_before);
    if funding.payer_lamports < payer_debit_lamports {
        return Err(Error::InsufficientCash);
    }
    let accumulator = OwnerSettlementAccumulatorV1::new(authority.expectation)?;
    Ok(OwnerSettlementCreatePlanV1 {
        address: derived.address,
        program_id: derived.program_id,
        bump: derived.bump,
        payer_debit_lamports,
        target_lamports_after: funding
            .target_lamports_before
            .checked_add(payer_debit_lamports)
            .ok_or(Error::ArithmeticOverflow)?,
        refund_recipient: funding.refund_recipient,
        rent_ledger: authority.rent_ledger,
        payer_rent_principal_lamports: payer_debit_lamports,
        prefunded_donation_lamports: funding.target_lamports_before,
        donation_sink: authority.donation_sink,
        body: accumulator.encode_body()?,
    })
}

/// One authenticated receipt end and its independent once-only latch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedSettlementReceiptEndV1 {
    /// Canonical receipt account PDA.
    pub receipt: [u8; 32],
    /// Independent action-25 accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Epoch PDA.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Exact owner/order-set digest.
    pub owner_order_set_digest: [u8; 32],
    /// Owner of this receipt end.
    pub owner: [u8; 32],
    /// Canonical selected order index.
    pub order_index: u8,
    /// Buy or sell end.
    pub side: SettlementSideV1,
    /// Exact value carried by this end.
    pub consideration_price_units: u128,
    /// True only for the unique end completing this order.
    pub completes_order: bool,
    /// Canonical zero-based slice index.
    pub slice_index: u16,
    /// Must equal `slice_index + 1`.
    pub sequence: u64,
    /// Already accounted real-end bitmap: bit zero buy, bit one sell.
    pub accounted_end_mask: u8,
    /// Real ends present on the receipt; virtual ends have no bit.
    pub expected_end_mask: u8,
}

impl AuthenticatedSettlementReceiptEndV1 {
    fn side_mask(self) -> u8 {
        match self.side {
            SettlementSideV1::Buy => BUY_END_MASK,
            SettlementSideV1::Sell => SELL_END_MASK,
        }
    }

    fn validate(self) -> Result<()> {
        if self.receipt == [0; 32]
            || self.receipt_accounting_id == [0; 32]
            || self.market == [0; 32]
            || self.epoch == [0; 32]
            || self.candidate == [0; 32]
            || self.owner_order_set_digest == [0; 32]
            || self.owner == [0; 32]
            || self.consideration_price_units == 0
            || self.expected_end_mask == 0
            || self.expected_end_mask & !(BUY_END_MASK | SELL_END_MASK) != 0
            || self.accounted_end_mask & !self.expected_end_mask != 0
            || self.sequence != u64::from(self.slice_index) + 1
        {
            return Err(Error::InvalidOrder);
        }
        let side = self.side_mask();
        if self.expected_end_mask & side == 0 || self.accounted_end_mask & side != 0 {
            return Err(Error::DuplicateCompletion);
        }
        Ok(())
    }
}

/// Atomic row-plus-receipt poststate for one exact receipt end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerSettlementReceiptAccountingPlanV1 {
    /// Owner row to write.
    pub owner_settlement_account: [u8; 32],
    /// Canonical next row body.
    pub owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES],
    /// Receipt to write.
    pub receipt: [u8; 32],
    /// Exact action-25 accounting replay identity.
    pub receipt_accounting_id: [u8; 32],
    /// Next independent end-accounting bitmap.
    pub receipt_accounted_end_mask: u8,
}

/// Account for one receipt end without moving cash or Eggs.
///
/// A live action 25 writes this row, the Reservation accounting cursor, and
/// the receipt accounting latch atomically. Later delivery uses a disjoint ID
/// and latch.
pub fn prepare_account_receipt_end_v1(
    account: AuthenticatedOwnerSettlementAccountV1,
    receipt: AuthenticatedSettlementReceiptEndV1,
) -> Result<OwnerSettlementReceiptAccountingPlanV1> {
    receipt.validate()?;
    let expected = account.accumulator.expectation;
    if receipt.market != expected.market
        || receipt.epoch != expected.epoch
        || receipt.candidate != expected.candidate
        || receipt.owner_order_set_digest != expected.owner_order_set_digest
        || receipt.owner != expected.owner
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut next = account.accumulator;
    next.consume(AuthenticatedOwnerFragmentV1 {
        order_index: receipt.order_index,
        side: receipt.side,
        consideration_price_units: receipt.consideration_price_units,
        completes_order: receipt.completes_order,
    })?;
    Ok(OwnerSettlementReceiptAccountingPlanV1 {
        owner_settlement_account: account.address,
        owner_settlement_body: next.encode_body()?,
        receipt: receipt.receipt,
        receipt_accounting_id: receipt.receipt_accounting_id,
        receipt_accounted_end_mask: receipt.accounted_end_mask | receipt.side_mask(),
    })
}

/// Exact candidate-wide owner-settlement cash expectation.
///
/// This pot is a liability ledger inside pooled collateral custody, not a
/// token account and not protocol revenue. Buyer consideration is staged
/// before seller credit; fees, rounding slack, and virtual-claim cash remain
/// separate until their actual semantic owners authorize disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SettlementCashPotExpectationV1 {
    /// Market identity.
    pub market: [u8; 32],
    /// Epoch PDA.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Complete owner/order-set digest.
    pub owner_order_set_digest: [u8; 32],
    /// Selected fee-record identity; zero exactly for a zero-fee candidate.
    pub fee_record: [u8; 32],
    /// Collateral price scale.
    pub price_scale: Amount,
    /// Exact owner-row count.
    pub owner_count: u16,
    /// Sum of owner payer conversions, excluding fees.
    pub consideration_debit_atoms: Amount,
    /// Sum of owner payee conversions.
    pub seller_credit_atoms: Amount,
    /// Exact selected fee atoms.
    pub selected_fee_atoms: Amount,
    /// Relation-certified terminal rounding slack.
    pub rounding_pot_price_units: u128,
    /// Direction of the sole canonical virtual complete-set conversion.
    pub virtual_cash_direction: VirtualCashDirectionV1,
    /// Exact virtual cash atoms: terminal split funding or opening merge proceeds.
    pub virtual_cash_atoms: Amount,
}

impl SettlementCashPotExpectationV1 {
    /// Validate the whole-book terminal equation before any owner may move.
    pub fn validate(self) -> Result<()> {
        let identities = [
            self.market,
            self.epoch,
            self.candidate,
            self.owner_order_set_digest,
            self.fee_record,
        ];
        let active_len = if self.fee_record == [0; 32] { 4 } else { 5 };
        let mut left = 0_usize;
        while left < active_len {
            if identities[left] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            let mut right = left + 1;
            while right < active_len {
                if identities[left] == identities[right] {
                    return Err(Error::InvalidIdentity);
                }
                right += 1;
            }
            left += 1;
        }
        if self.price_scale == 0
            || self.owner_count == 0
            || (self.selected_fee_atoms == 0) != (self.fee_record == [0; 32])
            || self.rounding_pot_price_units % u128::from(self.price_scale) != 0
            || (self.virtual_cash_atoms == 0)
                != (self.virtual_cash_direction == VirtualCashDirectionV1::None)
        {
            return Err(Error::InvalidExpectation);
        }
        let opening_merge = self.opening_merge_cash_atoms();
        let terminal_split = self.terminal_split_cash_atoms();
        let available = self
            .consideration_debit_atoms
            .checked_add(opening_merge)
            .ok_or(Error::ArithmeticOverflow)?
            .checked_sub(self.seller_credit_atoms)
            .ok_or(Error::InvariantViolation)?;
        let rounding_atoms =
            Amount::try_from(self.rounding_pot_price_units / u128::from(self.price_scale))
                .map_err(|_| Error::ArithmeticOverflow)?;
        if available
            != rounding_atoms
                .checked_add(terminal_split)
                .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    /// Merge proceeds that must exist before any owner cash realization.
    pub const fn opening_merge_cash_atoms(self) -> Amount {
        match self.virtual_cash_direction {
            VirtualCashDirectionV1::Merge => self.virtual_cash_atoms,
            VirtualCashDirectionV1::None | VirtualCashDirectionV1::Split => 0,
        }
    }

    /// Split principal remaining after every owner cash realization.
    pub const fn terminal_split_cash_atoms(self) -> Amount {
        match self.virtual_cash_direction {
            VirtualCashDirectionV1::Split => self.virtual_cash_atoms,
            VirtualCashDirectionV1::None | VirtualCashDirectionV1::Merge => 0,
        }
    }
}

/// Mutable candidate-wide buyer-first settlement pot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct SettlementCashPotV1 {
    /// Immutable candidate expectation.
    pub expectation: SettlementCashPotExpectationV1,
    /// Buyer consideration not yet credited to sellers.
    pub available_consideration_atoms: Amount,
    /// Fees collected under the selected candidate-wide fee record.
    pub collected_fee_atoms: Amount,
    /// Relation-certified rounding slack realized by finalized owners.
    pub realized_rounding_price_units: u128,
    /// Number of atomically finalized owner rows.
    pub finalized_owner_count: u16,
    /// Zero while allocating, one after owner finalization, two after every
    /// split-cash atom has entered an atomic inventory-and-delivery transition.
    pub state: u8,
}

impl SettlementCashPotV1 {
    /// Create an empty pot from a complete verifier-owned expectation.
    pub fn new(expectation: SettlementCashPotExpectationV1) -> Result<Self> {
        expectation.validate()?;
        Ok(Self {
            expectation,
            available_consideration_atoms: expectation.opening_merge_cash_atoms(),
            collected_fee_atoms: 0,
            realized_rounding_price_units: 0,
            finalized_owner_count: 0,
            state: 0,
        })
    }

    /// Validate progress bounds and the allocation-complete state.
    pub fn validate(self) -> Result<()> {
        self.expectation.validate()?;
        if self.state > 2
            || self.finalized_owner_count > self.expectation.owner_count
            || self.available_consideration_atoms
                > self
                    .expectation
                    .consideration_debit_atoms
                    .checked_add(self.expectation.opening_merge_cash_atoms())
                    .ok_or(Error::ArithmeticOverflow)?
            || self.collected_fee_atoms > self.expectation.selected_fee_atoms
            || self.realized_rounding_price_units > self.expectation.rounding_pot_price_units
            || (self.state == 0 && self.finalized_owner_count == self.expectation.owner_count)
            || (self.finalized_owner_count == 0
                && (self.available_consideration_atoms
                    != self.expectation.opening_merge_cash_atoms()
                    || self.collected_fee_atoms != 0
                    || self.realized_rounding_price_units != 0))
        {
            return Err(Error::InvariantViolation);
        }
        if self.state != 0 {
            let rounding_atoms = Amount::try_from(
                self.expectation.rounding_pot_price_units
                    / u128::from(self.expectation.price_scale),
            )
            .map_err(|_| Error::ArithmeticOverflow)?;
            if self.finalized_owner_count != self.expectation.owner_count
                || self.collected_fee_atoms != self.expectation.selected_fee_atoms
                || self.realized_rounding_price_units != self.expectation.rounding_pot_price_units
            {
                return Err(Error::InvariantViolation);
            }
            match (self.expectation.virtual_cash_direction, self.state) {
                (VirtualCashDirectionV1::Split, 1)
                    if self.available_consideration_atoms >= rounding_atoms
                        && self.available_consideration_atoms
                            <= rounding_atoms
                                .checked_add(self.expectation.virtual_cash_atoms)
                                .ok_or(Error::ArithmeticOverflow)? => {}
                (VirtualCashDirectionV1::Split, 2)
                    if self.available_consideration_atoms == rounding_atoms => {}
                (VirtualCashDirectionV1::None | VirtualCashDirectionV1::Merge, 1)
                    if self.available_consideration_atoms == rounding_atoms => {}
                _ => return Err(Error::InvariantViolation),
            }
        }
        Ok(())
    }

    /// Encode the exact canonical semantic body.
    pub fn encode_body(self) -> Result<[u8; SETTLEMENT_CASH_POT_BODY_V1_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; SETTLEMENT_CASH_POT_BODY_V1_BYTES];
        let mut cursor = 0_usize;
        for key in [
            self.expectation.market,
            self.expectation.epoch,
            self.expectation.candidate,
            self.expectation.owner_order_set_digest,
            self.expectation.fee_record,
        ] {
            put(&mut output, &mut cursor, &key)?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.expectation.price_scale.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.owner_count.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.consideration_debit_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.seller_credit_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.selected_fee_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.rounding_pot_price_units.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.expectation.virtual_cash_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &[self.expectation.virtual_cash_direction as u8],
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.available_consideration_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.collected_fee_atoms.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.realized_rounding_price_units.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.finalized_owner_count.to_le_bytes(),
        )?;
        put(&mut output, &mut cursor, &[self.state])?;
        put(&mut output, &mut cursor, &[0; 2])?;
        if cursor != SETTLEMENT_CASH_POT_BODY_V1_BYTES {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }

    /// Decode one exact hostile-byte-facing body.
    pub fn decode_body(input: &[u8]) -> Result<Self> {
        if input.len() != SETTLEMENT_CASH_POT_BODY_V1_BYTES {
            return Err(Error::InvalidAccount);
        }
        let mut cursor = 0_usize;
        let expectation = SettlementCashPotExpectationV1 {
            market: read_key(input, &mut cursor)?,
            epoch: read_key(input, &mut cursor)?,
            candidate: read_key(input, &mut cursor)?,
            owner_order_set_digest: read_key(input, &mut cursor)?,
            fee_record: read_key(input, &mut cursor)?,
            price_scale: read_u64(input, &mut cursor)?,
            owner_count: read_u16(input, &mut cursor)?,
            consideration_debit_atoms: read_u64(input, &mut cursor)?,
            seller_credit_atoms: read_u64(input, &mut cursor)?,
            selected_fee_atoms: read_u64(input, &mut cursor)?,
            rounding_pot_price_units: read_u128(input, &mut cursor)?,
            virtual_cash_atoms: read_u64(input, &mut cursor)?,
            virtual_cash_direction: VirtualCashDirectionV1::decode(read_u8(input, &mut cursor)?)?,
        };
        let value = Self {
            expectation,
            available_consideration_atoms: read_u64(input, &mut cursor)?,
            collected_fee_atoms: read_u64(input, &mut cursor)?,
            realized_rounding_price_units: read_u128(input, &mut cursor)?,
            finalized_owner_count: read_u16(input, &mut cursor)?,
            state: read_u8(input, &mut cursor)?,
        };
        if take(input, &mut cursor, 2)? != &[0; 2] || cursor != input.len() {
            return Err(Error::InvalidAccount);
        }
        value.validate()?;
        Ok(value)
    }
}

/// Owner-scoped fee debit authenticated under the selected fee record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedOwnerFeeDebitV1 {
    /// Selected fee record, zero exactly for a zero-fee candidate.
    pub fee_record: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Epoch PDA.
    pub epoch: [u8; 32],
    /// Final candidate identity.
    pub candidate: [u8; 32],
    /// Owner whose signed envelopes fund the assessment.
    pub owner: [u8; 32],
    /// Exact whole-atom owner fee.
    pub fee_atoms: Amount,
    /// True only after canonical payer-envelope allocation is authorized.
    pub payer_allocation_authorized: bool,
}

/// Adapter-authenticated identity of the canonical finalized owner-row bytes.
///
/// The digest is request-scoped and intentionally not copied into every owner
/// row. Persistent replay safety belongs to the row's one-way state and to the
/// in-place fee/finalization receipt owned by the outer runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AuthenticatedOwnerFinalizationIdV1 {
    /// Action-38 identity supplied by the immutable selector.
    pub owner_finalization_id: [u8; 32],
    /// `SHA-256(OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V1 || finalized_body)`.
    pub finalized_row_data_id: [u8; 32],
    /// OwnerSettlement account whose poststate was digested.
    pub owner_settlement_account: [u8; 32],
    /// Position atomically realized with that row.
    pub position: [u8; 32],
    /// Market identity.
    pub market: [u8; 32],
    /// Counted Epoch identity.
    pub epoch: [u8; 32],
    /// Final selected candidate identity.
    pub candidate: [u8; 32],
    /// Semantic owner.
    pub owner: [u8; 32],
    /// True only after the outer adapter derived the canonical row data ID.
    pub derivation_authenticated: bool,
}

/// Atomic row, Position, and candidate-pot cash realization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OwnerCashRealizationPlanV1 {
    /// Owner row to write.
    pub owner_settlement_account: [u8; 32],
    /// Finalized owner-row body.
    pub owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES],
    /// Exact canonical Position V3 poststate to write.
    pub position: PositionSettlementPoststateV3,
    /// Prospective candidate-wide cash pot.
    pub settlement_cash_pot: SettlementCashPotV1,
    /// Exact disposition used to form every poststate.
    pub disposition: OwnerSettlementDispositionV1,
}

/// Realization plan bound to the adapter-derived action-38 row-data identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct BoundOwnerCashRealizationPlanV1 {
    /// Request-scoped identity equal to the canonical finalized-row data ID.
    pub owner_finalization_id: [u8; 32],
    /// Complete row, Position, and cash-pot poststate.
    pub realization: OwnerCashRealizationPlanV1,
}

/// Realize one fully accumulated row into a buyer-first candidate pot.
///
/// A sell-heavy row may temporarily refuse until earlier buyer rows have put
/// enough consideration into the pot. The row, Position, and pot are one
/// atomic write set. Completion does not authorize FinalPot or row retirement;
/// that whole-book terminal capability remains intentionally absent.
pub fn prepare_realize_owner_cash_v1(
    account: AuthenticatedOwnerSettlementAccountV1,
    position: AuthenticatedPositionV3,
    fee: AuthenticatedOwnerFeeDebitV1,
    pot: SettlementCashPotV1,
) -> Result<OwnerCashRealizationPlanV1> {
    pot.validate()?;
    position.validate()?;
    let expected = account.accumulator.expectation;
    let position_fields = position.semantic.fields();
    if pot.state != 0
        || position.general_market_runtime != expected.market
        || position_fields.owner.bytes() != expected.owner
        || pot.expectation.market != expected.market
        || pot.expectation.epoch != expected.epoch
        || pot.expectation.candidate != expected.candidate
        || pot.expectation.owner_order_set_digest != expected.owner_order_set_digest
        || fee.market != expected.market
        || fee.epoch != expected.epoch
        || fee.candidate != expected.candidate
        || fee.owner != expected.owner
        || fee.fee_record != pot.expectation.fee_record
        || fee.fee_atoms != expected.selected_fee_atoms
        || !fee.payer_allocation_authorized
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut next_row = account.accumulator;
    let disposition = next_row.finalize(
        position_fields.cash_atoms,
        position_fields.reserved_cash_atoms,
    )?;
    let consideration_debit = disposition
        .debit_atoms
        .checked_sub(disposition.selected_fee_atoms)
        .ok_or(Error::InvariantViolation)?;
    let available = pot
        .available_consideration_atoms
        .checked_add(consideration_debit)
        .ok_or(Error::ArithmeticOverflow)?
        .checked_sub(disposition.credit_atoms)
        .ok_or(Error::SettlementLiquidityUnavailable)?;
    let mut next_pot = pot;
    next_pot.available_consideration_atoms = available;
    next_pot.collected_fee_atoms = next_pot
        .collected_fee_atoms
        .checked_add(disposition.selected_fee_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    next_pot.realized_rounding_price_units = next_pot
        .realized_rounding_price_units
        .checked_add(disposition.residue_price_units)
        .ok_or(Error::ArithmeticOverflow)?;
    next_pot.finalized_owner_count = next_pot
        .finalized_owner_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    if next_pot.finalized_owner_count == next_pot.expectation.owner_count {
        next_pot.state = 1;
    }
    next_pot.validate()?;
    let next_position = position.settlement_poststate(
        disposition.position_cash_atoms,
        disposition.position_reserved_cash_atoms,
        position_fields.native_eggs,
    )?;
    Ok(OwnerCashRealizationPlanV1 {
        owner_settlement_account: account.address,
        owner_settlement_body: next_row.encode_body()?,
        position: next_position,
        settlement_cash_pot: next_pot,
        disposition,
    })
}

/// Bind a staged realization to the canonical finalized-row data identity.
///
/// The adapter computes the data ID from `owner_settlement_body`, authenticates
/// that derivation, and then calls this function. This ordering avoids a hash
/// circularity while keeping the unbound plan non-authorizing.
pub fn bind_owner_cash_realization_id_v1(
    account: AuthenticatedOwnerSettlementAccountV1,
    position: AuthenticatedPositionV3,
    fee: AuthenticatedOwnerFeeDebitV1,
    pot: SettlementCashPotV1,
    realization: OwnerCashRealizationPlanV1,
    finalization: AuthenticatedOwnerFinalizationIdV1,
) -> Result<BoundOwnerCashRealizationPlanV1> {
    let canonical = prepare_realize_owner_cash_v1(account, position, fee, pot)?;
    if realization != canonical {
        return Err(Error::InvariantViolation);
    }
    realization.position.validate_successor_of(
        position,
        realization.disposition.position_cash_atoms,
        realization.disposition.position_reserved_cash_atoms,
        position.semantic.fields().native_eggs,
    )?;
    realization.settlement_cash_pot.validate()?;
    let row = OwnerSettlementAccumulatorV1::decode_body(&realization.owner_settlement_body)?;
    let expected = row.expectation;
    if row.state != 1
        || !finalization.derivation_authenticated
        || finalization.owner_finalization_id == [0; 32]
        || finalization.owner_finalization_id != finalization.finalized_row_data_id
        || finalization.owner_settlement_account != realization.owner_settlement_account
        || finalization.position != realization.position.account
        || finalization.market != expected.market
        || finalization.epoch != expected.epoch
        || finalization.candidate != expected.candidate
        || finalization.owner != expected.owner
        || realization.settlement_cash_pot.expectation.market != expected.market
        || realization.settlement_cash_pot.expectation.epoch != expected.epoch
        || realization.settlement_cash_pot.expectation.candidate != expected.candidate
    {
        return Err(Error::AuthorityUnavailable);
    }
    Ok(BoundOwnerCashRealizationPlanV1 {
        owner_finalization_id: finalization.owner_finalization_id,
        realization,
    })
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    output
        .get_mut(*cursor..end)
        .ok_or(Error::InvalidAccount)?
        .copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

fn take<'a>(input: &'a [u8], cursor: &mut usize, width: usize) -> Result<&'a [u8]> {
    let end = cursor.checked_add(width).ok_or(Error::ArithmeticOverflow)?;
    let value = input.get(*cursor..end).ok_or(Error::InvalidAccount)?;
    *cursor = end;
    Ok(value)
}

fn read_key(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let mut value = [0_u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u8(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(take(input, cursor, 1)?[0])
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut value = [0_u8; 2];
    value.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(value))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = [0_u8; 8];
    value.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(value))
}

fn read_u128(input: &[u8], cursor: &mut usize) -> Result<u128> {
    let mut value = [0_u8; 16];
    value.copy_from_slice(take(input, cursor, 16)?);
    Ok(u128::from_le_bytes(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_retirement::{
        Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
        PositionV3Fields, RentSplitV2,
    };

    fn key(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn row(owner: u8, side: SettlementSideV1) -> AuthenticatedOwnerSettlementAccountV1 {
        let (buy_mask, sell_mask, buy_units, sell_units, reserved) = match side {
            SettlementSideV1::Buy => (1, 0, 15, 0, 2),
            SettlementSideV1::Sell => (0, 2, 0, 15, 0),
        };
        let expectation = OwnerSettlementExpectationV1 {
            market: key(1),
            epoch: key(2),
            candidate: key(3),
            owner: key(owner),
            owner_order_set_digest: key(9),
            price_scale: 10,
            expected_buy_order_mask: buy_mask,
            expected_sell_order_mask: sell_mask,
            expected_slice_count: 1,
            expected_buy_price_units: buy_units,
            expected_sell_price_units: sell_units,
            expected_buy_price_units_present: buy_mask != 0,
            expected_sell_price_units_present: sell_mask != 0,
            selected_fee_atoms: 0,
            reserved_cash_atoms: reserved,
        };
        let mut accumulator = OwnerSettlementAccumulatorV1::new(expectation).unwrap();
        accumulator
            .consume(AuthenticatedOwnerFragmentV1 {
                order_index: if side == SettlementSideV1::Buy { 0 } else { 1 },
                side,
                consideration_price_units: 15,
                completes_order: true,
            })
            .unwrap();
        AuthenticatedOwnerSettlementAccountV1 {
            address: key(owner + 16),
            program_id: key(8),
            lamports: 100,
            rent_minimum: 100,
            accumulator,
        }
    }

    fn pot() -> SettlementCashPotV1 {
        SettlementCashPotV1::new(SettlementCashPotExpectationV1 {
            market: key(1),
            epoch: key(2),
            candidate: key(3),
            owner_order_set_digest: key(9),
            fee_record: [0; 32],
            price_scale: 10,
            owner_count: 2,
            consideration_debit_atoms: 2,
            seller_credit_atoms: 1,
            selected_fee_atoms: 0,
            rounding_pot_price_units: 10,
            virtual_cash_direction: VirtualCashDirectionV1::None,
            virtual_cash_atoms: 0,
        })
        .unwrap()
    }

    fn identity(byte: u8) -> Identity32V1 {
        Identity32V1::new(key(byte)).unwrap()
    }

    fn position(owner: u8, cash: u64, reserved: u64) -> AuthenticatedPositionV3 {
        let account = key(owner + 32);
        let replay_account = identity(owner + 40);
        let purpose_binding_id = identity(64);
        let position_semantic_id = key(owner + 80);
        let semantic = PositionAccountV3::new(PositionV3Fields {
            purpose: PositionPurposeV3::General,
            lifecycle: PositionLifecycleV3::Open,
            outcome_count: 2,
            stored_bump: 254,
            generation: 1,
            market_instance_id: identity(60),
            realm_id: identity(61),
            collateral_policy_id: identity(62),
            collateral_release_id: identity(63),
            owner: identity(owner),
            controller: identity(owner + 8),
            replay_account,
            purpose_binding_id,
            cash_atoms: cash,
            reserved_cash_atoms: reserved,
            native_eggs: [0; crate::MAX_OUTCOMES],
            outstanding_reservations: 1,
            rent: RentSplitV2 {
                payer: identity(owner + 48),
                refundable_live_principal: 1,
                permanent_tombstone_principal: 1,
                donation_floor: 0,
            },
        })
        .unwrap();
        AuthenticatedPositionV3 {
            account,
            general_market_runtime: key(1),
            semantic,
            semantic_id: position_semantic_id,
            account_authenticated: true,
            semantic_id_authenticated: true,
            market_binding_authenticated: true,
            writable: true,
        }
    }

    fn zero_fee(owner: u8) -> AuthenticatedOwnerFeeDebitV1 {
        AuthenticatedOwnerFeeDebitV1 {
            fee_record: [0; 32],
            market: key(1),
            epoch: key(2),
            candidate: key(3),
            owner: key(owner),
            fee_atoms: 0,
            payer_allocation_authorized: true,
        }
    }

    fn finalization(owner: u8, identity: u8) -> AuthenticatedOwnerFinalizationIdV1 {
        AuthenticatedOwnerFinalizationIdV1 {
            owner_finalization_id: key(identity),
            finalized_row_data_id: key(identity),
            owner_settlement_account: key(owner + 16),
            position: key(owner + 32),
            market: key(1),
            epoch: key(2),
            candidate: key(3),
            owner: key(owner),
            derivation_authenticated: true,
        }
    }

    #[test]
    fn buyer_first_funds_seller_and_closes_exact_rounding() {
        let first = prepare_realize_owner_cash_v1(
            row(4, SettlementSideV1::Buy),
            position(4, 2, 2),
            zero_fee(4),
            pot(),
        )
        .unwrap();
        let bound = bind_owner_cash_realization_id_v1(
            row(4, SettlementSideV1::Buy),
            position(4, 2, 2),
            zero_fee(4),
            pot(),
            first,
            finalization(4, 40),
        )
        .unwrap();
        assert_eq!(bound.owner_finalization_id, key(40));
        assert_eq!(first.settlement_cash_pot.available_consideration_atoms, 2);
        assert_eq!(first.settlement_cash_pot.realized_rounding_price_units, 5);

        let second = prepare_realize_owner_cash_v1(
            row(5, SettlementSideV1::Sell),
            position(5, 0, 0),
            zero_fee(5),
            first.settlement_cash_pot,
        )
        .unwrap();
        assert_eq!(second.settlement_cash_pot.available_consideration_atoms, 1);
        assert_eq!(second.settlement_cash_pot.realized_rounding_price_units, 10);
        assert_eq!(second.settlement_cash_pot.state, 1);
    }

    #[test]
    fn seller_first_and_hostile_prefilled_pot_refuse() {
        assert_eq!(
            prepare_realize_owner_cash_v1(
                row(5, SettlementSideV1::Sell),
                position(5, 0, 0),
                zero_fee(5),
                pot(),
            ),
            Err(Error::SettlementLiquidityUnavailable)
        );
        let mut forged = pot();
        forged.available_consideration_atoms = 1;
        assert_eq!(forged.validate(), Err(Error::InvariantViolation));
    }

    #[test]
    fn prefunding_reduces_only_the_real_payer_debit() {
        let expectation = row(4, SettlementSideV1::Buy).accumulator.expectation;
        let plan = prepare_create_owner_settlement_account_v1(
            SelectedOwnerRowAuthorityV1 {
                selected_candidate_account: key(7),
                expectation,
                row_ordinal: 0,
                owner_count: 2,
                rent_payer: key(10),
                rent_refund_recipient: key(11),
                rent_ledger: key(13),
                donation_sink: key(14),
            },
            AdapterDerivedPdaV1 {
                program_id: key(8),
                address: key(20),
                epoch: key(2),
                candidate: key(3),
                owner: key(4),
                bump: 254,
            },
            OwnerSettlementCreateFundingV1 {
                payer: key(10),
                refund_recipient: key(11),
                payer_lamports: 50,
                target_lamports_before: 100,
                target_owner_before: key(12),
                system_program_id: key(12),
                target_data_len_before: 0,
                target_writable: true,
                target_executable: false,
                rent_minimum: 150,
            },
        )
        .unwrap();
        assert_eq!(plan.payer_debit_lamports, 50);
        assert_eq!(plan.target_lamports_after, 150);
        assert_eq!(plan.refund_recipient, key(11));
        assert_eq!(plan.payer_rent_principal_lamports, 50);
        assert_eq!(plan.prefunded_donation_lamports, 100);
        assert_eq!(plan.donation_sink, key(14));
    }
}
