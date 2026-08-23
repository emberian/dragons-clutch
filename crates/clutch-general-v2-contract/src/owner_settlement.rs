// SPDX-License-Identifier: AGPL-3.0-or-later

//! Disabled General V2 envelope for the owner-settlement semantic body.
//!
//! Funding, refundable principal, and hostile prefunding are owned by the
//! separately authenticated rent ledger returned by the upstream creation
//! plan. This envelope deliberately does not persist a second rent truth.

use crate::{
    CodecError, Reader, Writer, OWNER_SETTLEMENT_ACCOUNT_BYTES, OWNER_SETTLEMENT_ACCOUNT_TAG,
    OWNER_SETTLEMENT_ACCOUNT_VERSION, OWNER_SETTLEMENT_ACCOUNT_VERSION_V1,
};

pub use clutch_owner_settlement::{
    build_owner_settlement_book_v1, AuthenticatedOwnerFragmentV1, CandidateSettlementTotalsV1,
    Error as OwnerSettlementError, OwnerSettlementAccumulatorV1, OwnerSettlementBookV1,
    OwnerSettlementDispositionV1, OwnerSettlementExpectationV1,
    OwnerSettlementTerminalProjectionV1, SelectedOwnerFeeV1, SettlementCashPotExpectationV1,
    SettlementCashPotV1, SettlementSideV1, VerifiedSettlementOrderV1,
    OWNER_FINALIZED_ROW_DATA_ID_DOMAIN_V1, OWNER_SETTLEMENT_BODY_V1_BYTES,
    SETTLEMENT_CASH_POT_BODY_V1_BYTES,
};
pub use clutch_owner_settlement::{
    build_owner_settlement_book_v2, derive_owner_finalized_row_data_id_v2,
    derive_settlement_receipt_data_id_v2,
    AuthenticatedOwnerFragmentV2, AuthenticatedSettlementReceiptEndV2,
    AuthenticatedSettlementReceiptV2, CandidateSettlementTotalsV2, OwnerSettlementAccumulatorV2,
    OwnerFinalizedRowDataHashV2, OwnerSettlementBookV2, OwnerSettlementExpectationV2,
    PresentConsiderationV2, PresentPriceV2,
    SettlementReceiptDataHashV2, SettlementReceiptRouteV2, VerifiedSettlementOrderV2,
    OwnerSettlementTerminalProjectionV2,
    OWNER_SETTLEMENT_BODY_V2_BYTES, SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V2,
    SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V2_BYTES,
};

/// Disabled outer account envelope around the exact upstream semantic body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV1AccountV1 {
    /// Exact 288-byte owner-settlement semantic owner.
    pub semantic: OwnerSettlementAccumulatorV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl OwnerSettlementV1AccountV1 {
    /// Validate the upstream semantic owner and outer deletion compartments.
    pub fn validate(self) -> Result<(), CodecError> {
        self.semantic
            .validate()
            .map_err(|_| CodecError::InvalidState)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Consume the semantic owner's finalized-row deletion projection.
    pub fn retirement_projection(self) -> Result<OwnerSettlementTerminalProjectionV1, CodecError> {
        self.validate()?;
        self.semantic
            .terminal_projection()
            .map_err(|_| CodecError::InvalidState)
    }

    /// Encode the exact canonical 292-byte outer account.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let semantic = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_TAG)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_VERSION_V1)?;
        writer.bytes(&semantic)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode one exact hostile outer account and the canonical semantic body.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_VERSION_V1 {
            return Err(CodecError::WrongVersion);
        }
        let semantic_body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES] = reader.array()?;
        let value = Self {
            semantic: OwnerSettlementAccumulatorV1::decode_body(&semantic_body)
                .map_err(|_| CodecError::InvalidState)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(OWNER_SETTLEMENT_BODY_V1_BYTES == 288);
const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_BYTES == 2 + 288 + 2);

/// Presence-explicit successor envelope selected only by outer version two.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV2AccountV1 {
    /// Exact 288-byte presence-explicit owner-settlement semantic owner.
    pub semantic: OwnerSettlementAccumulatorV2,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl OwnerSettlementV2AccountV1 {
    /// Validate the successor semantics and outer deletion compartments.
    pub fn validate(self) -> Result<(), CodecError> {
        self.semantic
            .validate()
            .map_err(|_| CodecError::InvalidState)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Consume the semantic owner's exact finalized-row projection.
    pub fn retirement_projection(
        self,
    ) -> Result<OwnerSettlementTerminalProjectionV2, CodecError> {
        self.validate()?;
        self.semantic
            .terminal_projection()
            .map_err(|_| CodecError::InvalidState)
    }

    /// Encode the canonical version-two 292-byte outer account.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let semantic = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_TAG)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_VERSION)?;
        writer.bytes(&semantic)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode one hostile version-two account without accepting V1 aliases.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let semantic_body: [u8; OWNER_SETTLEMENT_BODY_V2_BYTES] = reader.array()?;
        let value = Self {
            semantic: OwnerSettlementAccumulatorV2::decode_body(&semantic_body)
                .map_err(|_| CodecError::InvalidState)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(OWNER_SETTLEMENT_BODY_V2_BYTES == 288);

/// Capability-disabled outer account for the buyer-first candidate cash pot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettlementCashPotV1AccountV1 {
    /// Exact constructor-checked candidate-wide cash-pot semantics.
    pub semantic: SettlementCashPotV1,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl SettlementCashPotV1AccountV1 {
    /// Encode the exact canonical 260-byte outer account.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        let semantic = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, crate::SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?;
        writer.u8(crate::SETTLEMENT_CASH_POT_ACCOUNT_TAG)?;
        writer.u8(crate::SETTLEMENT_CASH_POT_ACCOUNT_VERSION)?;
        writer.bytes(&semantic)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Decode one hostile outer account through the authoritative inner codec.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, crate::SETTLEMENT_CASH_POT_ACCOUNT_BYTES)?;
        if reader.u8()? != crate::SETTLEMENT_CASH_POT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != crate::SETTLEMENT_CASH_POT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let body: [u8; SETTLEMENT_CASH_POT_BODY_V1_BYTES] = reader.array()?;
        let value = Self {
            semantic: SettlementCashPotV1::decode_body(&body)
                .map_err(|_| CodecError::InvalidState)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        if value.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(value)
    }
}

const _: () = assert!(SETTLEMENT_CASH_POT_BODY_V1_BYTES == 256);
const _: () = assert!(crate::SETTLEMENT_CASH_POT_ACCOUNT_BYTES == 2 + 256 + 2);
