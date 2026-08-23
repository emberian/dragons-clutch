// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reserved-disabled outer codecs for owner settlement and FinalPot.

use crate::{
    CodecError, DeletableRentOwnerV1, Id32, Reader, Writer, FINAL_POT_ACCOUNT_TAG,
    FINAL_POT_ACCOUNT_VERSION, OWNER_SETTLEMENT_ACCOUNT_TAG, OWNER_SETTLEMENT_ACCOUNT_VERSION,
    SETTLEMENT_CASH_POT_ACCOUNT_TAG, SETTLEMENT_CASH_POT_ACCOUNT_VERSION,
};

pub use clutch_owner_settlement::{
    FinalPotAuthorityBindingsV1, FinalPotDischargeKindV1, FinalPotDischargeReceiptV1,
    FinalPotRetirementDispositionV1, FinalPotVirtualClaimOpeningV1, GeneralV2FinalPotV1,
    OwnerSettlementAccumulatorV1, OwnerSettlementRowRetirementPlanV1, SettlementCashPotV1,
    FINAL_POT_DISCHARGE_RECEIPT_BODY_V1_BYTES, GENERAL_V2_FINAL_POT_BODY_V1_BYTES,
    OWNER_SETTLEMENT_BODY_V1_BYTES, SETTLEMENT_CASH_POT_BODY_V1_BYTES,
};

/// Exact outer owner-settlement account bytes.
pub const OWNER_SETTLEMENT_ACCOUNT_BYTES: usize = 340;
/// Exact outer allocation-complete cash-pot account bytes.
pub const SETTLEMENT_CASH_POT_ACCOUNT_BYTES: usize = 308;
/// Exact outer terminal FinalPot account bytes.
pub const FINAL_POT_ACCOUNT_BYTES: usize = 810;

fn write_rent(writer: &mut Writer<'_>, rent: DeletableRentOwnerV1) -> Result<(), CodecError> {
    rent.validate()?;
    writer.bytes(&rent.payer.bytes())?;
    writer.u64(rent.refundable_principal)?;
    writer.u64(rent.donation_floor)
}

fn read_rent(reader: &mut Reader<'_>) -> Result<DeletableRentOwnerV1, CodecError> {
    let value = DeletableRentOwnerV1 {
        payer: Id32::new(reader.array()?)?,
        refundable_principal: reader.u64()?,
        donation_floor: reader.u64()?,
    };
    value.validate()?;
    Ok(value)
}

/// Disabled exact envelope around one owner-settlement semantic row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV1AccountV1 {
    /// Exact upstream semantic owner.
    pub semantic: OwnerSettlementAccumulatorV1,
    /// Disjoint refundable rent principal and hostile-prefund floor.
    pub rent: DeletableRentOwnerV1,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl OwnerSettlementV1AccountV1 {
    /// Validate semantic state, rent ownership, and reserved flags.
    pub fn validate(self) -> Result<(), CodecError> {
        self.semantic
            .validate()
            .map_err(|_| CodecError::InvalidState)?;
        self.rent.validate()?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode the exact reserved-disabled account image.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let body = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_TAG)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_VERSION)?;
        writer.bytes(&body)?;
        write_rent(&mut writer, self.rent)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode and totally validate one exact hostile outer frame.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES] = reader.array()?;
        let value = Self {
            semantic: OwnerSettlementAccumulatorV1::decode_body(&body)
                .map_err(|_| CodecError::InvalidState)?,
            rent: read_rent(&mut reader)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Disabled exact envelope around the allocation-complete buyer-first pot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCashPotV1AccountV1 {
    /// Candidate-wide allocation state.
    pub semantic: SettlementCashPotV1,
    /// Disjoint refundable rent principal and hostile-prefund floor.
    pub rent: DeletableRentOwnerV1,
    /// Stored canonical FinalPot PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl SettlementCashPotV1AccountV1 {
    /// Validate semantic state, rent ownership, and reserved flags.
    pub fn validate(self) -> Result<(), CodecError> {
        self.semantic
            .validate()
            .map_err(|_| CodecError::InvalidState)?;
        self.rent.validate()?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode the exact reserved-disabled account image.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let body = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?;
        writer.u8(SETTLEMENT_CASH_POT_ACCOUNT_TAG)?;
        writer.u8(SETTLEMENT_CASH_POT_ACCOUNT_VERSION)?;
        writer.bytes(&body)?;
        write_rent(&mut writer, self.rent)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode and totally validate one exact hostile outer frame.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?;
        if reader.u8()? != SETTLEMENT_CASH_POT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != SETTLEMENT_CASH_POT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let body: [u8; SETTLEMENT_CASH_POT_BODY_V1_BYTES] = reader.array()?;
        let value = Self {
            semantic: SettlementCashPotV1::decode_body(&body)
                .map_err(|_| CodecError::InvalidState)?,
            rent: read_rent(&mut reader)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Disabled exact envelope around the explicit-liability FinalPot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralV2FinalPotV1AccountV1 {
    /// Exact upstream FinalPot semantic owner.
    pub semantic: GeneralV2FinalPotV1,
    /// Disjoint refundable rent principal and hostile-prefund floor.
    pub rent: DeletableRentOwnerV1,
    /// Stored canonical PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl GeneralV2FinalPotV1AccountV1 {
    /// Validate semantic state, rent ownership, and reserved flags.
    pub fn validate(self) -> Result<(), CodecError> {
        self.semantic
            .validate()
            .map_err(|_| CodecError::InvalidState)?;
        self.rent.validate()?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode the exact reserved-disabled account image.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let body = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, FINAL_POT_ACCOUNT_BYTES)?;
        writer.u8(FINAL_POT_ACCOUNT_TAG)?;
        writer.u8(FINAL_POT_ACCOUNT_VERSION)?;
        writer.bytes(&body)?;
        write_rent(&mut writer, self.rent)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode and totally validate one exact hostile outer frame.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, FINAL_POT_ACCOUNT_BYTES)?;
        if reader.u8()? != FINAL_POT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != FINAL_POT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let body: [u8; GENERAL_V2_FINAL_POT_BODY_V1_BYTES] = reader.array()?;
        let value = Self {
            semantic: GeneralV2FinalPotV1::decode_body(&body)
                .map_err(|_| CodecError::InvalidState)?,
            rent: read_rent(&mut reader)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Present-funding inputs for the cash-pot to FinalPot reallocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotPromotionFundingV1 {
    /// Present signer/writable payer authenticated by the adapter.
    pub payer: Id32,
    /// Present payer balance.
    pub payer_lamports: u64,
    /// Actual pot-account balance before reallocation.
    pub account_lamports_before: u64,
    /// Exact rent minimum for [`FINAL_POT_ACCOUNT_BYTES`].
    pub final_rent_principal: u64,
}

/// Atomic payer debit, realloc image, and absolute balance result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotPromotionPlanV1 {
    /// Payer supplying the complete size-increase rent delta.
    pub payer: Id32,
    /// Exact debit unaffected by hostile prefunds or later donations.
    pub payer_debit_lamports: u64,
    /// Exact account balance after the payer transfer.
    pub account_lamports_after: u64,
    /// Exact larger account image to write after realloc.
    pub account_bytes: [u8; FINAL_POT_ACCOUNT_BYTES],
}

/// Promote an allocation-complete cash pot without letting donation lamports
/// discount the recorded payer-owned rent principal.
pub fn prepare_promote_settlement_cash_pot_v1(
    current: SettlementCashPotV1AccountV1,
    final_pot: GeneralV2FinalPotV1,
    funding: FinalPotPromotionFundingV1,
) -> Result<FinalPotPromotionPlanV1, CodecError> {
    current.validate()?;
    final_pot.validate().map_err(|_| CodecError::InvalidState)?;
    if current.semantic.state != 1
        || final_pot.settled != current.semantic
        || funding.payer != current.rent.payer
        || funding.final_rent_principal < current.rent.refundable_principal
        || funding.account_lamports_before
            < current
                .rent
                .refundable_principal
                .checked_add(current.rent.donation_floor)
                .ok_or(CodecError::ArithmeticOverflow)?
    {
        return Err(CodecError::MismatchedBinding);
    }
    let payer_debit_lamports = funding
        .final_rent_principal
        .checked_sub(current.rent.refundable_principal)
        .ok_or(CodecError::ArithmeticOverflow)?;
    if funding.payer_lamports < payer_debit_lamports {
        return Err(CodecError::InvalidState);
    }
    let account_lamports_after = funding
        .account_lamports_before
        .checked_add(payer_debit_lamports)
        .ok_or(CodecError::ArithmeticOverflow)?;
    let account = GeneralV2FinalPotV1AccountV1 {
        semantic: final_pot,
        rent: DeletableRentOwnerV1 {
            payer: current.rent.payer,
            refundable_principal: funding.final_rent_principal,
            donation_floor: current.rent.donation_floor,
        },
        stored_bump: current.stored_bump,
        flags: 0,
    };
    let mut account_bytes = [0u8; FINAL_POT_ACCOUNT_BYTES];
    account.encode(&mut account_bytes)?;
    Ok(FinalPotPromotionPlanV1 {
        payer: funding.payer,
        payer_debit_lamports,
        account_lamports_after,
        account_bytes,
    })
}

const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_BYTES == 2 + 288 + 48 + 2);
const _: () = assert!(SETTLEMENT_CASH_POT_ACCOUNT_BYTES == 2 + 256 + 48 + 2);
const _: () = assert!(FINAL_POT_ACCOUNT_BYTES == 2 + 758 + 48 + 2);
