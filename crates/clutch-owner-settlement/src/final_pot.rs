// SPDX-License-Identifier: AGPL-3.0-or-later

//! Semantic owner of General V2's terminal candidate-wide FinalPot.
//!
//! FinalPot is a collateral-liability ledger.  None of its rounding, virtual
//! claim, or selected-fee compartments is a donation, and no transition in
//! this module accepts a neutral-sink identity.  Solana-account rent and
//! unsolicited lamports remain a separate outer-envelope responsibility.

use crate::{
    Amount, AuthenticatedOwnerSettlementAccountV1, Error, OwnerSettlementAccumulatorV1, Result,
    SettlementCashPotV1, MAX_OUTCOMES, OWNER_SETTLEMENT_BODY_V1_BYTES,
    SETTLEMENT_CASH_POT_BODY_V1_BYTES,
};

/// Exact semantic bytes in [`GeneralV2FinalPotV1`].
pub const GENERAL_V2_FINAL_POT_BODY_V1_BYTES: usize = 758;

/// Separately authenticated semantic authorities fixed when the settled cash
/// pot becomes a FinalPot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotAuthorityBindingsV1 {
    /// Owner of relation-certified rounding slack, or zero iff that slack is zero.
    pub rounding_authority: [u8; 32],
    /// Owner of virtual internal claims and cash, or zero iff both are zero.
    pub virtual_claim_authority: [u8; 32],
    /// Owner of collected selected fees, or zero iff selected fees are zero.
    pub fee_authority: [u8; 32],
}

/// Exact virtual-claim opening authenticated from selected candidate semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotVirtualClaimOpeningV1 {
    /// Per-outcome internal claims placed under FinalPot ownership.
    pub internal_atoms: [Amount; MAX_OUTCOMES],
    /// Whole cash atoms; must equal the settled cash-pot expectation.
    pub cash_atoms: Amount,
}

fn validate_authority_bindings(
    settled: SettlementCashPotV1,
    initial_virtual_internal: [Amount; MAX_OUTCOMES],
    authorities: FinalPotAuthorityBindingsV1,
) -> Result<()> {
    let has_rounding = settled.expectation.rounding_pot_price_units != 0;
    let has_virtual = settled.expectation.terminal_claim_cash_atoms != 0
        || initial_virtual_internal.iter().any(|amount| *amount != 0);
    let has_fee = settled.expectation.selected_fee_atoms != 0;
    if (authorities.rounding_authority == [0; 32]) == has_rounding
        || (authorities.virtual_claim_authority == [0; 32]) == has_virtual
        || (authorities.fee_authority == [0; 32]) == has_fee
        || (has_fee && authorities.fee_authority == settled.expectation.fee_record)
    {
        return Err(Error::AuthorityUnavailable);
    }
    let identities = [
        authorities.rounding_authority,
        authorities.virtual_claim_authority,
        authorities.fee_authority,
    ];
    let mut left = 0usize;
    while left < identities.len() {
        if identities[left] != [0; 32] {
            for root in [
                settled.expectation.market,
                settled.expectation.epoch,
                settled.expectation.candidate,
                settled.expectation.owner_order_set_digest,
                settled.expectation.fee_record,
            ] {
                if root != [0; 32] && identities[left] == root {
                    return Err(Error::AuthorityUnavailable);
                }
            }
            let mut right = left + 1;
            while right < identities.len() {
                if identities[left] == identities[right] {
                    return Err(Error::AuthorityUnavailable);
                }
                right += 1;
            }
        }
        left += 1;
    }
    Ok(())
}

/// Persisted FinalPot with three explicit, non-fungible liability compartments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralV2FinalPotV1 {
    /// Allocation-complete buyer-first cash pot retained as immutable evidence.
    pub settled: SettlementCashPotV1,
    /// Nonzero fresh parent Epoch generation copied at promotion.
    pub epoch_generation: u64,
    /// Authorities fixed by the selected candidate/fee semantic owners.
    pub authorities: FinalPotAuthorityBindingsV1,
    /// Original virtual internal claims.
    pub initial_virtual_claim_internal_atoms: [Amount; MAX_OUTCOMES],
    /// Virtual internal claims not yet discharged.
    pub remaining_virtual_claim_internal_atoms: [Amount; MAX_OUTCOMES],
    /// Rounding price units not yet discharged by the rounding authority.
    pub remaining_rounding_price_units: u128,
    /// Whole rounding cash atoms not yet discharged.
    pub remaining_rounding_cash_atoms: Amount,
    /// Virtual-claim cash not yet discharged.
    pub remaining_virtual_claim_cash_atoms: Amount,
    /// Collected selected fees not yet discharged by the fee authority.
    pub remaining_fee_atoms: Amount,
    /// Exact number of finalized owner rows atomically retired.
    pub retired_owner_count: u16,
    /// Once-only rounding-disposition receipt identity; zero until discharged.
    pub rounding_disposition_id: [u8; 32],
    /// Once-only virtual-claim-disposition receipt identity; zero until discharged.
    pub virtual_claim_disposition_id: [u8; 32],
    /// Once-only fee-disposition receipt identity; zero until discharged.
    pub fee_disposition_id: [u8; 32],
    /// Zero while liabilities/rows remain; one only when physically retirable.
    pub state: u8,
}

impl GeneralV2FinalPotV1 {
    /// Promote one allocation-complete cash pot without merging any liability.
    pub fn from_settled_cash_pot(
        settled: SettlementCashPotV1,
        epoch_generation: u64,
        virtual_claims: FinalPotVirtualClaimOpeningV1,
        authorities: FinalPotAuthorityBindingsV1,
    ) -> Result<Self> {
        settled.validate()?;
        if settled.state != 1
            || epoch_generation == 0
            || virtual_claims.cash_atoms != settled.expectation.terminal_claim_cash_atoms
        {
            return Err(Error::Incomplete);
        }
        let rounding_cash_atoms = Amount::try_from(
            settled.expectation.rounding_pot_price_units
                / u128::from(settled.expectation.price_scale),
        )
        .map_err(|_| Error::ArithmeticOverflow)?;
        validate_authority_bindings(settled, virtual_claims.internal_atoms, authorities)?;
        let value = Self {
            settled,
            epoch_generation,
            authorities,
            initial_virtual_claim_internal_atoms: virtual_claims.internal_atoms,
            remaining_virtual_claim_internal_atoms: virtual_claims.internal_atoms,
            remaining_rounding_price_units: settled.expectation.rounding_pot_price_units,
            remaining_rounding_cash_atoms: rounding_cash_atoms,
            remaining_virtual_claim_cash_atoms: virtual_claims.cash_atoms,
            remaining_fee_atoms: settled.collected_fee_atoms,
            retired_owner_count: 0,
            rounding_disposition_id: [0; 32],
            virtual_claim_disposition_id: [0; 32],
            fee_disposition_id: [0; 32],
            state: 0,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate every immutable equation, remaining compartment, latch, and state.
    pub fn validate(self) -> Result<()> {
        self.settled.validate()?;
        validate_authority_bindings(
            self.settled,
            self.initial_virtual_claim_internal_atoms,
            self.authorities,
        )?;
        if self.settled.state != 1
            || self.epoch_generation == 0
            || self.retired_owner_count > self.settled.expectation.owner_count
            || self.remaining_rounding_price_units
                > self.settled.expectation.rounding_pot_price_units
            || self.remaining_rounding_price_units % u128::from(self.settled.expectation.price_scale)
                != 0
            || self.remaining_rounding_cash_atoms
                != Amount::try_from(
                    self.remaining_rounding_price_units
                        / u128::from(self.settled.expectation.price_scale),
                )
                .map_err(|_| Error::ArithmeticOverflow)?
            || self.remaining_virtual_claim_cash_atoms
                > self.settled.expectation.terminal_claim_cash_atoms
            || self.remaining_fee_atoms > self.settled.collected_fee_atoms
            || self.state > 1
        {
            return Err(Error::InvariantViolation);
        }
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            if self.remaining_virtual_claim_internal_atoms[index]
                > self.initial_virtual_claim_internal_atoms[index]
            {
                return Err(Error::InvariantViolation);
            }
            index += 1;
        }
        let rounding_open = self.remaining_rounding_price_units != 0;
        let virtual_open = self.remaining_virtual_claim_cash_atoms != 0
            || self
                .remaining_virtual_claim_internal_atoms
                .iter()
                .any(|amount| *amount != 0);
        let fee_open = self.remaining_fee_atoms != 0;
        let had_rounding = self.settled.expectation.rounding_pot_price_units != 0;
        let had_virtual = self.settled.expectation.terminal_claim_cash_atoms != 0
            || self
                .initial_virtual_claim_internal_atoms
                .iter()
                .any(|amount| *amount != 0);
        let had_fee = self.settled.expectation.selected_fee_atoms != 0;
        if (rounding_open && self.rounding_disposition_id != [0; 32])
            || (!rounding_open
                && (self.rounding_disposition_id != [0; 32]) != had_rounding)
            || (virtual_open && self.virtual_claim_disposition_id != [0; 32])
            || (!virtual_open
                && (self.virtual_claim_disposition_id != [0; 32]) != had_virtual)
            || (fee_open && self.fee_disposition_id != [0; 32])
            || (!fee_open && (self.fee_disposition_id != [0; 32]) != had_fee)
        {
            return Err(Error::InvariantViolation);
        }
        if (self.rounding_disposition_id == [0; 32]
            && self.remaining_rounding_price_units
                != self.settled.expectation.rounding_pot_price_units)
            || (self.virtual_claim_disposition_id == [0; 32]
                && (self.remaining_virtual_claim_cash_atoms
                    != self.settled.expectation.terminal_claim_cash_atoms
                    || self.remaining_virtual_claim_internal_atoms
                        != self.initial_virtual_claim_internal_atoms))
            || (self.fee_disposition_id == [0; 32]
                && self.remaining_fee_atoms != self.settled.collected_fee_atoms)
        {
            return Err(Error::InvariantViolation);
        }
        let ready = !rounding_open
            && !virtual_open
            && !fee_open
            && self.retired_owner_count == self.settled.expectation.owner_count;
        if (self.state == 1) != ready {
            return Err(Error::InvariantViolation);
        }
        Ok(())
    }

    fn refresh_state(&mut self) {
        let virtual_empty = self.remaining_virtual_claim_cash_atoms == 0
            && !self
                .remaining_virtual_claim_internal_atoms
                .iter()
                .any(|amount| *amount != 0);
        if self.remaining_rounding_price_units == 0
            && virtual_empty
            && self.remaining_fee_atoms == 0
            && self.retired_owner_count == self.settled.expectation.owner_count
        {
            self.state = 1;
        }
    }

    /// Produce an opaque terminal capability only after all three liabilities
    /// and every owner row are exhausted.
    pub fn retirement_disposition(self) -> Result<FinalPotRetirementDispositionV1> {
        self.validate()?;
        if self.state != 1 {
            return Err(Error::Incomplete);
        }
        Ok(FinalPotRetirementDispositionV1 {
            market: self.settled.expectation.market,
            epoch: self.settled.expectation.epoch,
            candidate: self.settled.expectation.candidate,
            epoch_generation: self.epoch_generation,
            owner_order_set_digest: self.settled.expectation.owner_order_set_digest,
            owner_count: self.settled.expectation.owner_count,
            collected_fee_atoms: self.settled.collected_fee_atoms,
            realized_rounding_price_units: self.settled.realized_rounding_price_units,
            rounding_disposition_id: self.rounding_disposition_id,
            virtual_claim_disposition_id: self.virtual_claim_disposition_id,
            fee_disposition_id: self.fee_disposition_id,
        })
    }

    /// Encode exactly [`GENERAL_V2_FINAL_POT_BODY_V1_BYTES`] canonical bytes.
    pub fn encode_body(self) -> Result<[u8; GENERAL_V2_FINAL_POT_BODY_V1_BYTES]> {
        self.validate()?;
        let settled = self.settled.encode_body()?;
        let mut output = [0u8; GENERAL_V2_FINAL_POT_BODY_V1_BYTES];
        let mut cursor = 0usize;
        put(&mut output, &mut cursor, &settled)?;
        put(&mut output, &mut cursor, &self.epoch_generation.to_le_bytes())?;
        for authority in [
            self.authorities.rounding_authority,
            self.authorities.virtual_claim_authority,
            self.authorities.fee_authority,
        ] {
            put(&mut output, &mut cursor, &authority)?;
        }
        for amount in self.initial_virtual_claim_internal_atoms {
            put(&mut output, &mut cursor, &amount.to_le_bytes())?;
        }
        for amount in self.remaining_virtual_claim_internal_atoms {
            put(&mut output, &mut cursor, &amount.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.remaining_rounding_price_units.to_le_bytes(),
        )?;
        for amount in [
            self.remaining_rounding_cash_atoms,
            self.remaining_virtual_claim_cash_atoms,
            self.remaining_fee_atoms,
        ] {
            put(&mut output, &mut cursor, &amount.to_le_bytes())?;
        }
        put(&mut output, &mut cursor, &self.retired_owner_count.to_le_bytes())?;
        for receipt in [
            self.rounding_disposition_id,
            self.virtual_claim_disposition_id,
            self.fee_disposition_id,
        ] {
            put(&mut output, &mut cursor, &receipt)?;
        }
        put(&mut output, &mut cursor, &[self.state])?;
        put(&mut output, &mut cursor, &[0; 3])?;
        if cursor != output.len() {
            return Err(Error::InvariantViolation);
        }
        Ok(output)
    }

    /// Decode and totally validate one exact hostile semantic body.
    pub fn decode_body(input: &[u8]) -> Result<Self> {
        if input.len() != GENERAL_V2_FINAL_POT_BODY_V1_BYTES {
            return Err(Error::InvalidAccount);
        }
        let mut cursor = 0usize;
        let settled = SettlementCashPotV1::decode_body(take(
            input,
            &mut cursor,
            SETTLEMENT_CASH_POT_BODY_V1_BYTES,
        )?)?;
        let epoch_generation = read_u64(input, &mut cursor)?;
        let authorities = FinalPotAuthorityBindingsV1 {
            rounding_authority: read_key(input, &mut cursor)?,
            virtual_claim_authority: read_key(input, &mut cursor)?,
            fee_authority: read_key(input, &mut cursor)?,
        };
        let mut initial = [0u64; MAX_OUTCOMES];
        let mut remaining = [0u64; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            initial[index] = read_u64(input, &mut cursor)?;
            index += 1;
        }
        index = 0;
        while index < MAX_OUTCOMES {
            remaining[index] = read_u64(input, &mut cursor)?;
            index += 1;
        }
        let value = Self {
            settled,
            epoch_generation,
            authorities,
            initial_virtual_claim_internal_atoms: initial,
            remaining_virtual_claim_internal_atoms: remaining,
            remaining_rounding_price_units: read_u128(input, &mut cursor)?,
            remaining_rounding_cash_atoms: read_u64(input, &mut cursor)?,
            remaining_virtual_claim_cash_atoms: read_u64(input, &mut cursor)?,
            remaining_fee_atoms: read_u64(input, &mut cursor)?,
            retired_owner_count: read_u16(input, &mut cursor)?,
            rounding_disposition_id: read_key(input, &mut cursor)?,
            virtual_claim_disposition_id: read_key(input, &mut cursor)?,
            fee_disposition_id: read_key(input, &mut cursor)?,
            state: read_u8(input, &mut cursor)?,
        };
        if take(input, &mut cursor, 3)? != &[0; 3] || cursor != input.len() {
            return Err(Error::InvalidAccount);
        }
        value.validate()?;
        Ok(value)
    }
}

/// Opaque rounding discharge authenticated by its still-missing semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRoundingDischargeV1 {
    authority: [u8; 32],
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    price_units: u128,
    cash_atoms: Amount,
    disposition_id: [u8; 32],
}

/// Opaque virtual-claim discharge authenticated by its still-missing owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedVirtualClaimDischargeV1 {
    authority: [u8; 32],
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    internal_atoms: [Amount; MAX_OUTCOMES],
    cash_atoms: Amount,
    disposition_id: [u8; 32],
}

/// Opaque selected-fee discharge authenticated by its still-missing owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSelectedFeeDischargeV1 {
    authority: [u8; 32],
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    fee_record: [u8; 32],
    fee_atoms: Amount,
    disposition_id: [u8; 32],
}

impl GeneralV2FinalPotV1 {
    /// Consume one exact owner-issued rounding discharge.  The authority type
    /// deliberately has no public constructor until its semantic owner lands.
    pub fn discharge_rounding(
        mut self,
        discharge: AuthenticatedRoundingDischargeV1,
    ) -> Result<Self> {
        self.validate()?;
        let expected = self.settled.expectation;
        if self.remaining_rounding_price_units == 0
            || discharge.authority != self.authorities.rounding_authority
            || discharge.market != expected.market
            || discharge.epoch != expected.epoch
            || discharge.candidate != expected.candidate
            || discharge.price_units != self.remaining_rounding_price_units
            || discharge.cash_atoms != self.remaining_rounding_cash_atoms
            || discharge.disposition_id == [0; 32]
        {
            return Err(Error::AuthorityUnavailable);
        }
        self.remaining_rounding_price_units = 0;
        self.remaining_rounding_cash_atoms = 0;
        self.rounding_disposition_id = discharge.disposition_id;
        self.refresh_state();
        self.validate()?;
        Ok(self)
    }

    /// Consume one exact owner-issued virtual-claim discharge.  The authority
    /// type deliberately has no public constructor until its owner lands.
    pub fn discharge_virtual_claims(
        mut self,
        discharge: AuthenticatedVirtualClaimDischargeV1,
    ) -> Result<Self> {
        self.validate()?;
        let expected = self.settled.expectation;
        if discharge.authority != self.authorities.virtual_claim_authority
            || discharge.market != expected.market
            || discharge.epoch != expected.epoch
            || discharge.candidate != expected.candidate
            || discharge.internal_atoms != self.remaining_virtual_claim_internal_atoms
            || discharge.cash_atoms != self.remaining_virtual_claim_cash_atoms
            || (discharge.cash_atoms == 0
                && !discharge.internal_atoms.iter().any(|amount| *amount != 0))
            || discharge.disposition_id == [0; 32]
        {
            return Err(Error::AuthorityUnavailable);
        }
        self.remaining_virtual_claim_internal_atoms = [0; MAX_OUTCOMES];
        self.remaining_virtual_claim_cash_atoms = 0;
        self.virtual_claim_disposition_id = discharge.disposition_id;
        self.refresh_state();
        self.validate()?;
        Ok(self)
    }

    /// Consume one exact owner-issued selected-fee discharge.  The authority
    /// type deliberately has no public constructor until its owner lands.
    pub fn discharge_selected_fee(
        mut self,
        discharge: AuthenticatedSelectedFeeDischargeV1,
    ) -> Result<Self> {
        self.validate()?;
        let expected = self.settled.expectation;
        if self.remaining_fee_atoms == 0
            || discharge.authority != self.authorities.fee_authority
            || discharge.market != expected.market
            || discharge.epoch != expected.epoch
            || discharge.candidate != expected.candidate
            || discharge.fee_record != expected.fee_record
            || discharge.fee_atoms != self.remaining_fee_atoms
            || discharge.disposition_id == [0; 32]
        {
            return Err(Error::AuthorityUnavailable);
        }
        self.remaining_fee_atoms = 0;
        self.fee_disposition_id = discharge.disposition_id;
        self.refresh_state();
        self.validate()?;
        Ok(self)
    }
}

/// Atomic owner-row close plus FinalPot decrement plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementRowRetirementPlanV1 {
    /// Canonical owner-settlement account being physically closed.
    pub owner_settlement_account: [u8; 32],
    /// Final row evidence staged before physical deletion.
    pub retired_owner_settlement_body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES],
    /// Exact prospective FinalPot body carrying the once-only decrement.
    pub final_pot_body: [u8; GENERAL_V2_FINAL_POT_BODY_V1_BYTES],
    /// FinalPot semantic poststate.
    pub final_pot: GeneralV2FinalPotV1,
}

/// Retire one finalized row exactly once and increment the FinalPot latch in
/// the same atomic write set.  A replayed state-2 row is refused by
/// `mark_retired`; an absent row cannot be authenticated by the adapter.
pub fn prepare_retire_owner_settlement_row_v1(
    account: AuthenticatedOwnerSettlementAccountV1,
    mut final_pot: GeneralV2FinalPotV1,
) -> Result<OwnerSettlementRowRetirementPlanV1> {
    final_pot.validate()?;
    let expected = account.accumulator.expectation;
    if expected.market != final_pot.settled.expectation.market
        || expected.epoch != final_pot.settled.expectation.epoch
        || expected.candidate != final_pot.settled.expectation.candidate
        || expected.owner_order_set_digest != final_pot.settled.expectation.owner_order_set_digest
        || final_pot.retired_owner_count >= final_pot.settled.expectation.owner_count
    {
        return Err(Error::AuthorityUnavailable);
    }
    let mut retired: OwnerSettlementAccumulatorV1 = account.accumulator;
    retired.mark_retired()?;
    final_pot.retired_owner_count = final_pot
        .retired_owner_count
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    final_pot.refresh_state();
    final_pot.validate()?;
    Ok(OwnerSettlementRowRetirementPlanV1 {
        owner_settlement_account: account.address,
        retired_owner_settlement_body: retired.encode_body()?,
        final_pot_body: final_pot.encode_body()?,
        final_pot,
    })
}

/// Opaque whole-book terminal facts.  No collateral destination or neutral
/// sink is present: every nonzero liability already has a receipt id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotRetirementDispositionV1 {
    market: [u8; 32],
    epoch: [u8; 32],
    candidate: [u8; 32],
    epoch_generation: u64,
    owner_order_set_digest: [u8; 32],
    owner_count: u16,
    collected_fee_atoms: Amount,
    realized_rounding_price_units: u128,
    rounding_disposition_id: [u8; 32],
    virtual_claim_disposition_id: [u8; 32],
    fee_disposition_id: [u8; 32],
}

impl FinalPotRetirementDispositionV1 {
    /// Market identity.
    pub const fn market(self) -> [u8; 32] { self.market }
    /// Fresh General V2 Epoch PDA.
    pub const fn epoch(self) -> [u8; 32] { self.epoch }
    /// Selected candidate identity.
    pub const fn candidate(self) -> [u8; 32] { self.candidate }
    /// Nonzero fresh parent Epoch generation.
    pub const fn epoch_generation(self) -> u64 { self.epoch_generation }
    /// Frozen owner/order-set digest.
    pub const fn owner_order_set_digest(self) -> [u8; 32] { self.owner_order_set_digest }
    /// Exact retired owner-row count.
    pub const fn owner_count(self) -> u16 { self.owner_count }
    /// Exact fees that were collected before separate disposition.
    pub const fn collected_fee_atoms(self) -> Amount { self.collected_fee_atoms }
    /// Exact rounding price units realized before separate disposition.
    pub const fn realized_rounding_price_units(self) -> u128 {
        self.realized_rounding_price_units
    }
    /// Rounding-disposition receipt, zero only for an originally zero compartment.
    pub const fn rounding_disposition_id(self) -> [u8; 32] { self.rounding_disposition_id }
    /// Virtual-disposition receipt, zero only for an originally zero compartment.
    pub const fn virtual_claim_disposition_id(self) -> [u8; 32] {
        self.virtual_claim_disposition_id
    }
    /// Fee-disposition receipt, zero only for an originally zero compartment.
    pub const fn fee_disposition_id(self) -> [u8; 32] { self.fee_disposition_id }
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor.checked_add(bytes.len()).ok_or(Error::ArithmeticOverflow)?;
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
    let mut value = [0u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u8(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(take(input, cursor, 1)?[0])
}

fn read_u16(input: &[u8], cursor: &mut usize) -> Result<u16> {
    let mut value = [0u8; 2];
    value.copy_from_slice(take(input, cursor, 2)?);
    Ok(u16::from_le_bytes(value))
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut value = [0u8; 8];
    value.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(value))
}

fn read_u128(input: &[u8], cursor: &mut usize) -> Result<u128> {
    let mut value = [0u8; 16];
    value.copy_from_slice(take(input, cursor, 16)?);
    Ok(u128::from_le_bytes(value))
}

const _: () = assert!(
    GENERAL_V2_FINAL_POT_BODY_V1_BYTES
        == SETTLEMENT_CASH_POT_BODY_V1_BYTES + 8 + 96 + 256 + 16 + 24 + 2 + 96 + 4
);
