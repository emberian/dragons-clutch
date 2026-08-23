// SPDX-License-Identifier: AGPL-3.0-or-later

//! Disabled General V2 envelope for the owner-settlement semantic body.

use crate::{
    CodecError, DeletableRentOwnerV1, Id32, Reader, Writer, OWNER_SETTLEMENT_ACCOUNT_BYTES,
    OWNER_SETTLEMENT_ACCOUNT_TAG, OWNER_SETTLEMENT_ACCOUNT_VERSION,
};

pub use clutch_owner_settlement::{
    build_owner_settlement_book_v1, AuthenticatedOwnerFragmentV1, CandidateSettlementTotalsV1,
    Error as OwnerSettlementError, OwnerSettlementAccumulatorV1, OwnerSettlementBookV1,
    OwnerSettlementDispositionV1, OwnerSettlementExpectationV1, SelectedOwnerFeeV1,
    SettlementSideV1, VerifiedSettlementOrderV1, OWNER_SETTLEMENT_BODY_V1_BYTES,
};

/// Disabled outer account envelope around the exact upstream semantic body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV1AccountV1 {
    /// Exact 288-byte owner-settlement semantic owner.
    pub semantic: OwnerSettlementAccumulatorV1,
    /// Disjoint refundable rent principal and hostile-prefund floor.
    pub rent: DeletableRentOwnerV1,
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
        self.rent.validate()?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode the exact canonical 340-byte outer account.
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
        writer.bytes(&self.rent.payer.bytes())?;
        writer.u64(self.rent.refundable_principal)?;
        writer.u64(self.rent.donation_floor)?;
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
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let semantic_body: [u8; OWNER_SETTLEMENT_BODY_V1_BYTES] = reader.array()?;
        let value = Self {
            semantic: OwnerSettlementAccumulatorV1::decode_body(&semantic_body)
                .map_err(|_| CodecError::InvalidState)?,
            rent: DeletableRentOwnerV1 {
                payer: Id32::new(reader.array()?)?,
                refundable_principal: reader.u64()?,
                donation_floor: reader.u64()?,
            },
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(OWNER_SETTLEMENT_BODY_V1_BYTES == 288);
const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_BYTES == 2 + 288 + 48 + 2);
