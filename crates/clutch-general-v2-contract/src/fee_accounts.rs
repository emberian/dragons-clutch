// SPDX-License-Identifier: AGPL-3.0-or-later

//! Capability-disabled General V2 envelopes for the account-neutral fee owners.
//!
//! The inner fee crate remains the sole semantic owner of fee selection,
//! carry, payer allocation, recipient allocation, and treasury accounting.
//! This module adds only exact Solana-facing tag/version/bump/flags bytes and
//! always re-enters the inner codec's constructor-backed decoder. Funding and
//! rent disposition remain owned by a separately authenticated runtime/rent
//! ledger rather than duplicated in these semantic accounts.

use clutch_batch::relation_v1::FrozenPolicyV1;
use clutch_batch_policy_identity::revenue_policy_v1::RevenuePolicyV1;
use clutch_fee_runtime_contract::allocation::{
    FeeEnvelopeV1, PayerAllocationV1, RecipientAllocationV1, StandingMakerRowV1,
};
use clutch_fee_runtime_contract::codec::{
    decode_fee_record_v1, decode_owner_fee_carry_v1, decode_payer_allocation_v1,
    decode_recipient_allocation_v1, decode_treasury_ledger_v1, encode_fee_record_v1,
    encode_owner_fee_carry_v1, encode_payer_allocation_v1, encode_recipient_allocation_v1,
    encode_treasury_ledger_v1, FEE_RECORD_ACCOUNT_V1_BYTES, OWNER_FEE_CARRY_ACCOUNT_V1_BYTES,
    PAYER_ALLOCATION_ACCOUNT_V1_BYTES, RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES,
    TREASURY_LEDGER_ACCOUNT_V1_BYTES,
};
use clutch_fee_runtime_contract::selected::{
    OwnerFeeAssessmentV1, OwnerFeeCarryV1, SelectedCompositeFeeV1,
};
use clutch_fee_runtime_contract::treasury::TreasuryLedgerV1;
use clutch_fee_runtime_contract::MAX_FEE_ROWS_V1;

use crate::{
    CodecError, Reader, Writer, OWNER_FEE_CARRY_ACCOUNT_BYTES, OWNER_FEE_CARRY_ACCOUNT_TAG,
    OWNER_FEE_CARRY_ACCOUNT_VERSION, PAYER_ALLOCATION_ACCOUNT_BYTES, PAYER_ALLOCATION_ACCOUNT_TAG,
    PAYER_ALLOCATION_ACCOUNT_VERSION, RECIPIENT_ALLOCATION_ACCOUNT_BYTES,
    RECIPIENT_ALLOCATION_ACCOUNT_TAG, RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
    SELECTED_FEE_RECORD_ACCOUNT_BYTES, SELECTED_FEE_RECORD_ACCOUNT_TAG,
    SELECTED_FEE_RECORD_ACCOUNT_VERSION, TREASURY_LEDGER_ACCOUNT_BYTES,
    TREASURY_LEDGER_ACCOUNT_TAG, TREASURY_LEDGER_ACCOUNT_VERSION,
};

const OUTER_FEE_ACCOUNT_BYTES: usize = 2 + 2;

fn map_fee_error<T>(result: clutch_fee_runtime_contract::Result<T>) -> Result<T, CodecError> {
    result.map_err(|_| CodecError::InvalidState)
}

fn encode_outer<const BODY: usize>(
    tag: u8,
    version: u8,
    body: &[u8; BODY],
    stored_bump: u8,
    output: &mut [u8],
) -> Result<(), CodecError> {
    let mut writer = Writer::exact(output, BODY + OUTER_FEE_ACCOUNT_BYTES)?;
    writer.u8(tag)?;
    writer.u8(version)?;
    writer.bytes(body)?;
    writer.u8(stored_bump)?;
    writer.u8(0)?;
    writer.finish()
}

fn decode_outer<const BODY: usize>(
    tag: u8,
    version: u8,
    input: &[u8],
) -> Result<([u8; BODY], u8), CodecError> {
    let mut reader = Reader::exact(input, BODY + OUTER_FEE_ACCOUNT_BYTES)?;
    if reader.u8()? != tag {
        return Err(CodecError::WrongTag);
    }
    if reader.u8()? != version {
        return Err(CodecError::WrongVersion);
    }
    let body = reader.array()?;
    let stored_bump = reader.u8()?;
    if reader.u8()? != 0 {
        return Err(CodecError::InvalidState);
    }
    reader.finish()?;
    Ok((body, stored_bump))
}

/// Immutable selected composite-fee record outer envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedFeeRecordV1AccountV1 {
    /// Constructor-authenticated selected fee semantics.
    pub semantic: SelectedCompositeFeeV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl SelectedFeeRecordV1AccountV1 {
    /// Encode the exact canonical outer account.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(encode_fee_record_v1(&self.semantic))?;
        encode_outer(
            SELECTED_FEE_RECORD_ACCOUNT_TAG,
            SELECTED_FEE_RECORD_ACCOUNT_VERSION,
            &body,
            self.stored_bump,
            output,
        )
    }

    /// Decode only with the authenticated batch and revenue policy preimages.
    pub fn decode(
        input: &[u8],
        batch: &FrozenPolicyV1,
        revenue: &RevenuePolicyV1,
    ) -> Result<Self, CodecError> {
        let (body, stored_bump) = decode_outer(
            SELECTED_FEE_RECORD_ACCOUNT_TAG,
            SELECTED_FEE_RECORD_ACCOUNT_VERSION,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_fee_record_v1(&body, batch, revenue))?,
            stored_bump,
        })
    }
}

/// One owner-scoped fee carry outer envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeCarryV1AccountV1 {
    /// Constructor-authenticated carry semantics.
    pub semantic: OwnerFeeCarryV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl OwnerFeeCarryV1AccountV1 {
    /// Encode the exact canonical outer account.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(encode_owner_fee_carry_v1(&self.semantic))?;
        encode_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_CARRY_ACCOUNT_VERSION,
            &body,
            self.stored_bump,
            output,
        )
    }

    /// Decode only against the authenticated selected fee record.
    pub fn decode(input: &[u8], selected: &SelectedCompositeFeeV1) -> Result<Self, CodecError> {
        let (body, stored_bump) = decode_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_CARRY_ACCOUNT_VERSION,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_owner_fee_carry_v1(&body, selected))?,
            stored_bump,
        })
    }
}

/// Temporary owner payer-allocation snapshot outer envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayerAllocationV1AccountV1 {
    /// Constructor-authenticated allocation semantics.
    pub semantic: PayerAllocationV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl PayerAllocationV1AccountV1 {
    /// Encode the exact canonical outer account.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(encode_payer_allocation_v1(&self.semantic))?;
        encode_outer(
            PAYER_ALLOCATION_ACCOUNT_TAG,
            PAYER_ALLOCATION_ACCOUNT_VERSION,
            &body,
            self.stored_bump,
            output,
        )
    }

    /// Decode only from the authenticated assessment and signed envelopes.
    pub fn decode(
        input: &[u8],
        assessment: &OwnerFeeAssessmentV1,
        envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    ) -> Result<Self, CodecError> {
        let (body, stored_bump) = decode_outer(
            PAYER_ALLOCATION_ACCOUNT_TAG,
            PAYER_ALLOCATION_ACCOUNT_VERSION,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_payer_allocation_v1(&body, assessment, envelopes))?,
            stored_bump,
        })
    }
}

/// Temporary candidate-wide recipient snapshot outer envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecipientAllocationV1AccountV1 {
    /// Constructor-authenticated allocation semantics.
    pub semantic: RecipientAllocationV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl RecipientAllocationV1AccountV1 {
    /// Encode the exact canonical outer account.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(encode_recipient_allocation_v1(&self.semantic))?;
        encode_outer(
            RECIPIENT_ALLOCATION_ACCOUNT_TAG,
            RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
            &body,
            self.stored_bump,
            output,
        )
    }

    /// Decode only from selected policy and authenticated standing-maker rows.
    pub fn decode(
        input: &[u8],
        selected: &SelectedCompositeFeeV1,
        revenue: &RevenuePolicyV1,
        makers: &[StandingMakerRowV1; MAX_FEE_ROWS_V1],
    ) -> Result<Self, CodecError> {
        let (body, stored_bump) = decode_outer(
            RECIPIENT_ALLOCATION_ACCOUNT_TAG,
            RECIPIENT_ALLOCATION_ACCOUNT_VERSION,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_recipient_allocation_v1(
                &body, selected, revenue, makers,
            ))?,
            stored_bump,
        })
    }
}

/// Treasury ordinary-Position ledger outer envelope.
#[derive(Debug, Eq, PartialEq)]
pub struct TreasuryLedgerV1AccountV1 {
    /// Constructor-authenticated treasury semantics.
    pub semantic: TreasuryLedgerV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl TreasuryLedgerV1AccountV1 {
    /// Encode the exact canonical outer account.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(encode_treasury_ledger_v1(&self.semantic))?;
        encode_outer(
            TREASURY_LEDGER_ACCOUNT_TAG,
            TREASURY_LEDGER_ACCOUNT_VERSION,
            &body,
            self.stored_bump,
            output,
        )
    }

    /// Decode only against the selected record that fixes its treasury facts.
    pub fn decode(input: &[u8], selected: &SelectedCompositeFeeV1) -> Result<Self, CodecError> {
        let (body, stored_bump) = decode_outer(
            TREASURY_LEDGER_ACCOUNT_TAG,
            TREASURY_LEDGER_ACCOUNT_VERSION,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_treasury_ledger_v1(&body, selected))?,
            stored_bump,
        })
    }
}

const _: () = assert!(
    SELECTED_FEE_RECORD_ACCOUNT_BYTES == FEE_RECORD_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    OWNER_FEE_CARRY_ACCOUNT_BYTES == OWNER_FEE_CARRY_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    PAYER_ALLOCATION_ACCOUNT_BYTES == PAYER_ALLOCATION_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    RECIPIENT_ALLOCATION_ACCOUNT_BYTES
        == RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    TREASURY_LEDGER_ACCOUNT_BYTES == TREASURY_LEDGER_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
