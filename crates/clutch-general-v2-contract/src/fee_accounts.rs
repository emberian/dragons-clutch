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
    decode_persisted_payer_allocation_v1, decode_recipient_allocation_v1,
    decode_treasury_ledger_v1, encode_fee_record_v1, encode_owner_fee_carry_v1,
    encode_payer_allocation_v1, encode_recipient_allocation_v1, encode_treasury_ledger_v1,
    FEE_RECORD_ACCOUNT_V1_BYTES, OWNER_FEE_CARRY_ACCOUNT_V1_BYTES,
    PAYER_ALLOCATION_ACCOUNT_V1_BYTES, RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES,
    TREASURY_LEDGER_ACCOUNT_V1_BYTES,
};
use clutch_fee_runtime_contract::selected::{
    OwnerFeeAssessmentV1, OwnerFeeCarryV1, SelectedCompositeFeeV1,
};
pub use clutch_fee_runtime_contract::terminal::{
    AuthenticatedOwnerFeeFinalizationV1, FeeClosureManifestReceiptV1, FeeRecordTerminalReceiptV1,
    FeeTerminalOutcomeV1, FeeTerminalReceiptBundleV1, GeneralFeeTerminalProjectionV1,
    GeneralOwnerFeeFinalizationProjectionV2, OwnerFeeFinalizationOutcomeV2,
    OwnerFeeFinalizationReceiptV1, FEE_CLOSURE_MANIFEST_V1_BYTES,
    FEE_TERMINAL_RECEIPT_V1_BYTES, OWNER_FEE_FINALIZATION_BODY_V2_BYTES,
};
use clutch_fee_runtime_contract::treasury::TreasuryLedgerV1;
pub use clutch_fee_runtime_contract::Id as FeeRuntimeId;
use clutch_fee_runtime_contract::MAX_FEE_ROWS_V1;

use crate::{
    CodecError, DeletableRentOwnerV1, Reader, Writer, OWNER_FEE_CARRY_ACCOUNT_BYTES,
    OWNER_FEE_CARRY_ACCOUNT_BYTES_V3, OWNER_FEE_CARRY_ACCOUNT_TAG,
    OWNER_FEE_CARRY_ACCOUNT_VERSION, OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
    OWNER_FEE_FINALIZATION_ACCOUNT_BYTES, OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4,
    OWNER_FEE_FINALIZATION_ACCOUNT_VERSION, OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4,
    PAYER_ALLOCATION_ACCOUNT_BYTES, PAYER_ALLOCATION_ACCOUNT_BYTES_V2,
    PAYER_ALLOCATION_ACCOUNT_TAG, PAYER_ALLOCATION_ACCOUNT_VERSION,
    PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
    RECIPIENT_ALLOCATION_ACCOUNT_BYTES, RECIPIENT_ALLOCATION_ACCOUNT_TAG,
    RECIPIENT_ALLOCATION_ACCOUNT_VERSION, SELECTED_FEE_RECORD_ACCOUNT_BYTES,
    SELECTED_FEE_RECORD_ACCOUNT_TAG, SELECTED_FEE_RECORD_ACCOUNT_VERSION,
    TREASURY_LEDGER_ACCOUNT_BYTES, TREASURY_LEDGER_ACCOUNT_TAG, TREASURY_LEDGER_ACCOUNT_VERSION,
};

const OUTER_FEE_ACCOUNT_BYTES: usize = 2 + 2;
const DELETABLE_RENT_OWNER_BYTES: usize = 32 + 8 + 8;

/// Convert exact persisted bytes into the fee semantic owner's identity type.
pub const fn fee_runtime_id_from_bytes(bytes: [u8; 32]) -> FeeRuntimeId {
    clutch_batch_policy_identity::Identity32V1(bytes)
}

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

fn encode_rent_owned_outer<const BODY: usize>(
    tag: u8,
    version: u8,
    body: &[u8; BODY],
    rent: DeletableRentOwnerV1,
    stored_bump: u8,
    output: &mut [u8],
) -> Result<(), CodecError> {
    rent.validate()?;
    let mut writer = Writer::exact(
        output,
        BODY + DELETABLE_RENT_OWNER_BYTES + OUTER_FEE_ACCOUNT_BYTES,
    )?;
    writer.u8(tag)?;
    writer.u8(version)?;
    writer.bytes(body)?;
    writer.bytes(&rent.payer.bytes())?;
    writer.u64(rent.refundable_principal)?;
    writer.u64(rent.donation_floor)?;
    writer.u8(stored_bump)?;
    writer.u8(0)?;
    writer.finish()
}

fn decode_rent_owned_outer<const BODY: usize>(
    tag: u8,
    version: u8,
    input: &[u8],
) -> Result<([u8; BODY], DeletableRentOwnerV1, u8), CodecError> {
    let mut reader = Reader::exact(
        input,
        BODY + DELETABLE_RENT_OWNER_BYTES + OUTER_FEE_ACCOUNT_BYTES,
    )?;
    if reader.u8()? != tag {
        return Err(CodecError::WrongTag);
    }
    if reader.u8()? != version {
        return Err(CodecError::WrongVersion);
    }
    let body = reader.array()?;
    let rent = DeletableRentOwnerV1 {
        payer: crate::Id32::new(reader.array()?)?,
        refundable_principal: reader.u64()?,
        donation_floor: reader.u64()?,
    };
    rent.validate()?;
    let stored_bump = reader.u8()?;
    if reader.u8()? != 0 {
        return Err(CodecError::InvalidState);
    }
    reader.finish()?;
    Ok((body, rent, stored_bump))
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

/// Sole future rent-owned live carry at the existing owner fee-carry PDA.
///
/// The fee semantic body owns exact rational carry and paid atoms. The
/// embedded rent compartment owns native-lamport principal independently;
/// neither compartment is usable as collateral, Hoard principal, fee value,
/// future revenue, or liveness capital.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeCarryV3AccountV1 {
    /// Constructor-authenticated carry semantics.
    pub semantic: OwnerFeeCarryV1,
    /// Exact payer-owned refundable rent principal and donation floor.
    pub rent: DeletableRentOwnerV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl OwnerFeeCarryV3AccountV1 {
    /// Encode the exact canonical rent-owned live carry.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(encode_owner_fee_carry_v1(&self.semantic))?;
        encode_rent_owned_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
            &body,
            self.rent,
            self.stored_bump,
            output,
        )
    }

    /// Decode hostile bytes only against the authenticated selected record.
    pub fn decode(input: &[u8], selected: &SelectedCompositeFeeV1) -> Result<Self, CodecError> {
        let (body, rent, stored_bump) = decode_rent_owned_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_owner_fee_carry_v1(&body, selected))?,
            rent,
            stored_bump,
        })
    }
}

/// In-place terminal successor at the existing owner fee-carry PDA.
///
/// The exact 496-byte semantic body remains owned and validated by
/// `clutch-fee-runtime-contract`. This outer adds only the unchanged 0x83 tag,
/// version 2 coordinate, stored PDA bump, and reserved-zero flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeFinalizationV2AccountV1 {
    /// Canonical fee-runtime terminal receipt.
    pub semantic: OwnerFeeFinalizationReceiptV1,
    /// Stored PDA bump retained across the in-place version transition.
    pub stored_bump: u8,
}

impl OwnerFeeFinalizationV2AccountV1 {
    /// Encode the exact canonical terminal successor outer account.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(self.semantic.encode())?;
        encode_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_FINALIZATION_ACCOUNT_VERSION,
            &body,
            self.stored_bump,
            output,
        )
    }

    /// Decode hostile outer bytes through the semantic owner's total decoder.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let (body, stored_bump) = decode_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_FINALIZATION_ACCOUNT_VERSION,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(OwnerFeeFinalizationReceiptV1::decode(&body))?,
            stored_bump,
        })
    }

    /// Project the exact terminal receipt after the outer adapter has
    /// authenticated this unchanged carry PDA.
    pub fn terminal_projection(
        self,
        carry_account: clutch_fee_runtime_contract::Id,
    ) -> Result<GeneralOwnerFeeFinalizationProjectionV2, CodecError> {
        map_fee_error(
            AuthenticatedOwnerFeeFinalizationV1 {
                carry_account,
                receipt: self.semantic,
            }
            .project_general(),
        )
    }
}

/// Sole future rent-owned immutable finalization successor at the carry PDA.
///
/// This is a same-address V3-to-V4 transition. Historical 0x83/v1 and v2
/// bytes are never reinterpreted as current state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerFeeFinalizationV4AccountV1 {
    /// Canonical fee-runtime terminal receipt.
    pub semantic: OwnerFeeFinalizationReceiptV1,
    /// Updated rent compartment after the exact in-place realloc top-up.
    pub rent: DeletableRentOwnerV1,
    /// Stored PDA bump retained across the in-place version transition.
    pub stored_bump: u8,
}

impl OwnerFeeFinalizationV4AccountV1 {
    /// Encode the exact canonical rent-owned terminal successor.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(self.semantic.encode())?;
        encode_rent_owned_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4,
            &body,
            self.rent,
            self.stored_bump,
            output,
        )
    }

    /// Decode hostile outer bytes through the semantic owner's total decoder.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let (body, rent, stored_bump) = decode_rent_owned_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_FINALIZATION_ACCOUNT_VERSION_V4,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(OwnerFeeFinalizationReceiptV1::decode(&body))?,
            rent,
            stored_bump,
        })
    }

    /// Project the exact current terminal receipt after PDA authentication.
    pub fn terminal_projection(
        self,
        carry_account: clutch_fee_runtime_contract::Id,
    ) -> Result<GeneralOwnerFeeFinalizationProjectionV2, CodecError> {
        map_fee_error(
            AuthenticatedOwnerFeeFinalizationV1 {
                carry_account,
                receipt: self.semantic,
            }
            .project_general(),
        )
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

    /// Decode a canonical persisted snapshot without replaying envelopes.
    ///
    /// This establishes only exact outer/inner structure. The live adapter
    /// must additionally authenticate program ownership and the payer PDA;
    /// the fee projection then joins this semantic body to its terminal carry.
    pub fn decode_persisted(input: &[u8]) -> Result<Self, CodecError> {
        let (body, stored_bump) = decode_outer(
            PAYER_ALLOCATION_ACCOUNT_TAG,
            PAYER_ALLOCATION_ACCOUNT_VERSION,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_persisted_payer_allocation_v1(&body))?,
            stored_bump,
        })
    }
}

/// Sole future rent-owned temporary payer-allocation snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayerAllocationV2AccountV1 {
    /// Constructor-authenticated allocation semantics.
    pub semantic: PayerAllocationV1,
    /// Exact payer-owned refundable rent principal and donation floor.
    pub rent: DeletableRentOwnerV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
}

impl PayerAllocationV2AccountV1 {
    /// Encode from the fully authenticated signed-envelope allocation.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        let body = map_fee_error(encode_payer_allocation_v1(&self.semantic))?;
        encode_rent_owned_outer(
            PAYER_ALLOCATION_ACCOUNT_TAG,
            PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
            &body,
            self.rent,
            self.stored_bump,
            output,
        )
    }

    /// Decode only while rederiving all authenticated signed envelopes.
    pub fn decode(
        input: &[u8],
        assessment: &OwnerFeeAssessmentV1,
        envelopes: &[FeeEnvelopeV1; MAX_FEE_ROWS_V1],
    ) -> Result<Self, CodecError> {
        let (body, rent, stored_bump) = decode_rent_owned_outer(
            PAYER_ALLOCATION_ACCOUNT_TAG,
            PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_payer_allocation_v1(&body, assessment, envelopes))?,
            rent,
            stored_bump,
        })
    }

    /// Structurally decode the immutable persisted allocation snapshot.
    ///
    /// This proves allocation structure and persisted rent ownership only.
    /// Program ownership, PDA identity, and original signed-envelope authority
    /// remain adapter obligations.
    pub fn decode_persisted(input: &[u8]) -> Result<Self, CodecError> {
        let (body, rent, stored_bump) = decode_rent_owned_outer(
            PAYER_ALLOCATION_ACCOUNT_TAG,
            PAYER_ALLOCATION_ACCOUNT_VERSION_V2,
            input,
        )?;
        Ok(Self {
            semantic: map_fee_error(decode_persisted_payer_allocation_v1(&body))?,
            rent,
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
    OWNER_FEE_CARRY_ACCOUNT_BYTES_V3
        == OWNER_FEE_CARRY_ACCOUNT_V1_BYTES
            + DELETABLE_RENT_OWNER_BYTES
            + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    OWNER_FEE_FINALIZATION_ACCOUNT_BYTES
        == OWNER_FEE_FINALIZATION_BODY_V2_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4
        == OWNER_FEE_FINALIZATION_BODY_V2_BYTES
            + DELETABLE_RENT_OWNER_BYTES
            + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    PAYER_ALLOCATION_ACCOUNT_BYTES == PAYER_ALLOCATION_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    PAYER_ALLOCATION_ACCOUNT_BYTES_V2
        == PAYER_ALLOCATION_ACCOUNT_V1_BYTES
            + DELETABLE_RENT_OWNER_BYTES
            + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    RECIPIENT_ALLOCATION_ACCOUNT_BYTES
        == RECIPIENT_ALLOCATION_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);
const _: () = assert!(
    TREASURY_LEDGER_ACCOUNT_BYTES == TREASURY_LEDGER_ACCOUNT_V1_BYTES + OUTER_FEE_ACCOUNT_BYTES
);

#[cfg(test)]
mod tests {
    use super::*;

    fn rent() -> DeletableRentOwnerV1 {
        DeletableRentOwnerV1 {
            payer: crate::Id32::from_bytes([9; 32]),
            refundable_principal: 1_000,
            donation_floor: 17,
        }
    }

    #[test]
    fn rent_owned_outer_round_trip_preserves_principal_and_donation() {
        let body = [7u8; OWNER_FEE_CARRY_ACCOUNT_V1_BYTES];
        let mut bytes = [0u8; OWNER_FEE_CARRY_ACCOUNT_BYTES_V3];
        encode_rent_owned_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
            &body,
            rent(),
            44,
            &mut bytes,
        )
        .unwrap();
        let decoded = decode_rent_owned_outer::<OWNER_FEE_CARRY_ACCOUNT_V1_BYTES>(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
            &bytes,
        )
        .unwrap();
        assert_eq!(decoded, (body, rent(), 44));
    }

    #[test]
    fn rent_owned_outer_refuses_zero_payer_and_noncanonical_flags() {
        let body = [7u8; OWNER_FEE_CARRY_ACCOUNT_V1_BYTES];
        let mut bytes = [0u8; OWNER_FEE_CARRY_ACCOUNT_BYTES_V3];
        assert_eq!(
            encode_rent_owned_outer(
                OWNER_FEE_CARRY_ACCOUNT_TAG,
                OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
                &body,
                DeletableRentOwnerV1 {
                    payer: crate::Id32::from_bytes([0; 32]),
                    ..rent()
                },
                44,
                &mut bytes,
            ),
            Err(CodecError::ZeroIdentity)
        );
        encode_rent_owned_outer(
            OWNER_FEE_CARRY_ACCOUNT_TAG,
            OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
            &body,
            rent(),
            44,
            &mut bytes,
        )
        .unwrap();
        bytes[OWNER_FEE_CARRY_ACCOUNT_BYTES_V3 - 1] = 1;
        assert_eq!(
            decode_rent_owned_outer::<OWNER_FEE_CARRY_ACCOUNT_V1_BYTES>(
                OWNER_FEE_CARRY_ACCOUNT_TAG,
                OWNER_FEE_CARRY_ACCOUNT_VERSION_V3,
                &bytes,
            ),
            Err(CodecError::InvalidState)
        );
    }
}
