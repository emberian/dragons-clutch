// SPDX-License-Identifier: AGPL-3.0-or-later

//! Disabled General V2 envelope for the owner-settlement semantic body.
//!
//! Versions one through four relied on a separately authenticated rent-ledger
//! identity returned by the upstream creation plan, but no persisted ledger
//! codec owned its eventual refund facts. They remain historical schemas only.
//! Version five embeds the sole exact deletable-rent owner beside the unchanged
//! V4 semantic state machine so terminal deletion has durable authority.

use crate::{
    CodecError, DeletableRentOwnerV1, Id32, Reader, Sha256BackendV1, Writer,
    OWNER_SETTLEMENT_ACCOUNT_BYTES, OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    OWNER_SETTLEMENT_ACCOUNT_TAG, OWNER_SETTLEMENT_ACCOUNT_VERSION,
    OWNER_SETTLEMENT_ACCOUNT_VERSION_V1, OWNER_SETTLEMENT_ACCOUNT_VERSION_V2,
    OWNER_SETTLEMENT_ACCOUNT_VERSION_V3, OWNER_SETTLEMENT_ACCOUNT_VERSION_V4,
    OWNER_SETTLEMENT_ACCOUNT_VERSION_V5,
};

pub use clutch_owner_settlement::{
    build_owner_settlement_book_v1, AuthenticatedOwnerFragmentV1, CandidateSettlementTotalsV1,
    Error as OwnerSettlementError, OwnerSettlementAccumulatorV1, OwnerSettlementBookV1,
    OwnerSettlementDispositionV1, OwnerSettlementExpectationV1, SelectedOwnerFeeV1,
    SettlementCashPotExpectationV1, SettlementCashPotV1, SettlementSideV1,
    VerifiedSettlementOrderV1, OWNER_SETTLEMENT_BODY_V1_BYTES, SETTLEMENT_CASH_POT_BODY_V1_BYTES,
};
pub use clutch_owner_settlement::{
    build_owner_settlement_expectation_basis_book_v4, OwnerSettlementAccumulatorV4,
    OwnerSettlementExpectationBasisBookV4, OwnerSettlementExpectationBasisV4,
    OwnerSettlementExpectationV4, OwnerSettlementStateV4, OwnerSettlementTerminalProjectionV4,
    VerifiedSettlementOrderV4, OWNER_SETTLEMENT_BODY_V4_BYTES, OWNER_SETTLEMENT_OUTER_TAG_V4,
    OWNER_SETTLEMENT_OUTER_VERSION_V4,
};

/// Fresh exact account-data identity domain for the rent-owned V5 envelope.
pub const OWNER_SETTLEMENT_ACCOUNT_DATA_ID_DOMAIN_V5: &[u8] =
    b"dragons-clutch/general-owner-settlement/account-data/v5\0";
pub use clutch_owner_settlement::{
    build_owner_settlement_book_v2, build_owner_settlement_expectation_basis_book_v2,
    derive_owner_finalized_row_data_id_v2, derive_settlement_receipt_data_id_v2,
    AuthenticatedOwnerFragmentV2, AuthenticatedSettlementReceiptEndV2,
    AuthenticatedSettlementReceiptV2, CandidateSettlementTotalsV2, OwnerFinalizedRowDataHashV2,
    OwnerSettlementAccumulatorV2, OwnerSettlementBookV2, OwnerSettlementExpectationBasisBookV2,
    OwnerSettlementExpectationBasisV2, OwnerSettlementExpectationV2,
    OwnerSettlementTerminalProjectionV2, PresentConsiderationV2, PresentPriceV2,
    SettlementReceiptDataHashV2, SettlementReceiptRouteV2, VerifiedSettlementOrderV2,
    OWNER_SETTLEMENT_BODY_V2_BYTES, SETTLEMENT_RECEIPT_DATA_ID_DOMAIN_V2,
    SETTLEMENT_RECEIPT_DATA_TRANSCRIPT_V2_BYTES,
};
pub use clutch_owner_settlement::{
    build_owner_settlement_expectation_basis_book_v3, OwnerSettlementAccumulatorV3,
    OwnerSettlementExpectationBasisBookV3, OwnerSettlementExpectationBasisV3,
    OwnerSettlementExpectationV3, OwnerSettlementStateV3, VerifiedSettlementOrderV3,
    OWNER_SETTLEMENT_BODY_V3_BYTES, OWNER_SETTLEMENT_OUTER_TAG_V3,
    OWNER_SETTLEMENT_OUTER_VERSION_V3,
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

/// Withdrawn presence-explicit envelope selected only by outer version two.
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

    /// Encode the canonical version-two 292-byte outer account.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let semantic = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_TAG)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_VERSION_V2)?;
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
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_VERSION_V2 {
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

/// Canonical General envelope around the Reservation-handoff V3 semantic body.
///
/// This is the only future owner-settlement route. The explicit V1 and V2
/// envelope types remain decodeable only for migration/audit tooling and are
/// never aliases for this schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV3AccountV1 {
    /// Exact canonical 288-byte Reservation-handoff semantic owner.
    pub semantic: OwnerSettlementAccumulatorV3,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl OwnerSettlementV3AccountV1 {
    /// Validate the authoritative V3 body and reserved outer byte.
    pub fn validate(self) -> Result<(), CodecError> {
        self.semantic
            .validate()
            .map_err(|_| CodecError::InvalidState)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode the exact canonical tag-`0x81`, version-3, 292-byte account.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let semantic = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_TAG)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_VERSION_V3)?;
        writer.bytes(&semantic)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Strictly decode only the exact V3 envelope and authoritative V3 body.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        let tag = reader.u8()?;
        if tag != OWNER_SETTLEMENT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        let version = reader.u8()?;
        if version != OWNER_SETTLEMENT_ACCOUNT_VERSION_V3 {
            return Err(CodecError::WrongVersion);
        }
        let semantic_body: [u8; OWNER_SETTLEMENT_BODY_V3_BYTES] = reader.array()?;
        let value = Self {
            semantic: OwnerSettlementAccumulatorV3::decode_body(tag, version, &semantic_body)
                .map_err(|_| CodecError::InvalidState)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_TAG == OWNER_SETTLEMENT_OUTER_TAG_V3);
const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_VERSION_V3 == OWNER_SETTLEMENT_OUTER_VERSION_V3);
const _: () = assert!(OWNER_SETTLEMENT_BODY_V3_BYTES == 288);
const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_BYTES == 2 + OWNER_SETTLEMENT_BODY_V3_BYTES + 2);

/// Historical General envelope around the delivery-complete V4 semantic body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV4AccountV1 {
    /// Exact canonical 288-byte V4 owner-settlement semantic owner.
    pub semantic: OwnerSettlementAccumulatorV4,
    /// Stored fresh V4 PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl OwnerSettlementV4AccountV1 {
    /// Validate the authoritative V4 body and reserved outer byte.
    pub fn validate(self) -> Result<(), CodecError> {
        self.semantic
            .validate()
            .map_err(|_| CodecError::InvalidState)?;
        if self.flags != 0 {
            return Err(CodecError::InvalidState);
        }
        Ok(())
    }

    /// Encode the exact tag-`0x81`, version-4, 292-byte account.
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

    /// Strictly decode only the exact V4 envelope and authoritative V4 body.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, OWNER_SETTLEMENT_ACCOUNT_BYTES)?;
        let tag = reader.u8()?;
        if tag != OWNER_SETTLEMENT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        let version = reader.u8()?;
        if version != OWNER_SETTLEMENT_ACCOUNT_VERSION_V4 {
            return Err(CodecError::WrongVersion);
        }
        let semantic_body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES] = reader.array()?;
        let value = Self {
            semantic: OwnerSettlementAccumulatorV4::decode_body(tag, version, &semantic_body)
                .map_err(|_| CodecError::InvalidState)?,
            stored_bump: reader.u8()?,
            flags: reader.u8()?,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_TAG == OWNER_SETTLEMENT_OUTER_TAG_V4);
const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_VERSION == OWNER_SETTLEMENT_OUTER_VERSION_V4);
const _: () = assert!(OWNER_SETTLEMENT_BODY_V4_BYTES == 288);
const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_BYTES == 2 + OWNER_SETTLEMENT_BODY_V4_BYTES + 2);

/// Sole future rent-owned General envelope around the V4 semantic state machine.
///
/// The semantic body stays byte-for-byte V4 so no second owner of settlement
/// arithmetic is introduced. The outer version, PDA domain, account-data
/// identity, and embedded rent compartment are all fresh and non-aliasing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV5AccountV1 {
    /// Exact canonical 288-byte delivery-complete semantic owner.
    pub semantic: OwnerSettlementAccumulatorV4,
    /// Sole payer-owned refundable principal and hostile-prefund floor.
    pub rent: DeletableRentOwnerV1,
    /// Stored fresh V5 PDA bump.
    pub stored_bump: u8,
    /// Reserved zero flags.
    pub flags: u8,
}

impl OwnerSettlementV5AccountV1 {
    /// Validate semantic, rent, and reserved outer compartments.
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

    /// Encode the exact tag-`0x81`, version-5, 340-byte account.
    pub fn encode(self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        let semantic = self
            .semantic
            .encode_body()
            .map_err(|_| CodecError::InvalidState)?;
        let mut writer = Writer::exact(output, OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_TAG)?;
        writer.u8(OWNER_SETTLEMENT_ACCOUNT_VERSION_V5)?;
        writer.bytes(&semantic)?;
        writer.bytes(&self.rent.payer.bytes())?;
        writer.u64(self.rent.refundable_principal)?;
        writer.u64(self.rent.donation_floor)?;
        writer.u8(self.stored_bump)?;
        writer.u8(self.flags)?;
        writer.finish()
    }

    /// Return exact canonical account bytes.
    pub fn encode_exact(self) -> Result<[u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5], CodecError> {
        let mut output = [0u8; OWNER_SETTLEMENT_ACCOUNT_BYTES_V5];
        self.encode(&mut output)?;
        Ok(output)
    }

    /// Strictly decode only the exact V5 envelope and V4 semantic body.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, OWNER_SETTLEMENT_ACCOUNT_BYTES_V5)?;
        let tag = reader.u8()?;
        if tag != OWNER_SETTLEMENT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if reader.u8()? != OWNER_SETTLEMENT_ACCOUNT_VERSION_V5 {
            return Err(CodecError::WrongVersion);
        }
        let semantic_body: [u8; OWNER_SETTLEMENT_BODY_V4_BYTES] = reader.array()?;
        let value = Self {
            semantic: OwnerSettlementAccumulatorV4::decode_body(
                tag,
                OWNER_SETTLEMENT_ACCOUNT_VERSION_V4,
                &semantic_body,
            )
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

    /// Derive the exact nonzero account-data identity over all 340 bytes.
    pub fn data_id<B: Sha256BackendV1>(&self, backend: &B) -> Result<Id32, CodecError> {
        let bytes = self.encode_exact()?;
        Id32::new(backend.sha256(&[OWNER_SETTLEMENT_ACCOUNT_DATA_ID_DOMAIN_V5, &bytes]))
    }

    /// Project a finalized semantic row together with its sole close authority.
    pub fn terminal_projection(&self) -> Result<OwnerSettlementV5TerminalProjectionV1, CodecError> {
        self.validate()?;
        let semantic = self
            .semantic
            .terminal_projection()
            .map_err(|_| CodecError::InvalidState)?;
        Ok(OwnerSettlementV5TerminalProjectionV1 {
            semantic,
            rent: self.rent,
        })
    }
}

/// Exact finalized V5 row facts needed by a higher authenticated close join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerSettlementV5TerminalProjectionV1 {
    semantic: OwnerSettlementTerminalProjectionV4,
    rent: DeletableRentOwnerV1,
}

impl OwnerSettlementV5TerminalProjectionV1 {
    /// Immutable finalized V4 semantic projection.
    pub const fn semantic(&self) -> OwnerSettlementTerminalProjectionV4 {
        self.semantic
    }

    /// Sole exact rent principal/donation close compartment.
    pub const fn rent(&self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

const _: () = assert!(OWNER_SETTLEMENT_ACCOUNT_BYTES_V5 == 2 + 288 + 48 + 2);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_ORDERS;

    fn semantic_v3() -> OwnerSettlementAccumulatorV3 {
        let mut orders = [VerifiedSettlementOrderV3 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV3 {
            owner: [6; 32],
            order_index: 4,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::new(0),
            slice_count: 1,
        };
        let expectation = build_owner_settlement_expectation_basis_book_v3(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
        )
        .unwrap()
        .row(0)
        .unwrap()
        .with_selected_fee(SelectedOwnerFeeV1 {
            owner: [6; 32],
            fee_atoms: 0,
        })
        .unwrap();
        OwnerSettlementAccumulatorV3::new(expectation).unwrap()
    }

    fn encoded_v3() -> [u8; OWNER_SETTLEMENT_ACCOUNT_BYTES] {
        let mut bytes = [0; OWNER_SETTLEMENT_ACCOUNT_BYTES];
        OwnerSettlementV3AccountV1 {
            semantic: semantic_v3(),
            stored_bump: 7,
            flags: 0,
        }
        .encode(&mut bytes)
        .unwrap();
        bytes
    }

    fn semantic_v4() -> OwnerSettlementAccumulatorV4 {
        let mut orders = [VerifiedSettlementOrderV4 {
            owner: [0; 32],
            order_index: 0,
            side: SettlementSideV1::Buy,
            consideration_price_units: PresentConsiderationV2::ABSENT,
            slice_count: 0,
            merge_delivery_count: 0,
        }; MAX_ORDERS];
        orders[0] = VerifiedSettlementOrderV4 {
            owner: [6; 32],
            order_index: 4,
            side: SettlementSideV1::Sell,
            consideration_price_units: PresentConsiderationV2::new(0),
            slice_count: 1,
            merge_delivery_count: 0,
        };
        let expectation = build_owner_settlement_expectation_basis_book_v4(
            [1; 32], [2; 32], [3; 32], [4; 32], 100, &orders, 1,
        )
        .unwrap()
        .row(0)
        .unwrap()
        .with_selected_fee(SelectedOwnerFeeV1 {
            owner: [6; 32],
            fee_atoms: 0,
        })
        .unwrap();
        OwnerSettlementAccumulatorV4::new(expectation).unwrap()
    }

    fn value_v5() -> OwnerSettlementV5AccountV1 {
        OwnerSettlementV5AccountV1 {
            semantic: semantic_v4(),
            rent: DeletableRentOwnerV1 {
                payer: Id32::new([9; 32]).unwrap(),
                refundable_principal: 3_000,
                donation_floor: 19,
            },
            stored_bump: 8,
            flags: 0,
        }
    }

    #[test]
    fn canonical_v3_outer_round_trips_at_the_frozen_width() {
        let bytes = encoded_v3();
        assert_eq!(bytes.len(), 292);
        assert_eq!(bytes[0], 0x81);
        assert_eq!(bytes[1], 3);
        assert_eq!(
            OwnerSettlementV3AccountV1::decode(&bytes),
            Ok(OwnerSettlementV3AccountV1 {
                semantic: semantic_v3(),
                stored_bump: 7,
                flags: 0,
            })
        );
    }

    #[test]
    fn v3_outer_refuses_withdrawn_versions_tags_and_lengths() {
        let bytes = encoded_v3();
        assert_eq!(
            OwnerSettlementV1AccountV1::decode(&bytes),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(
            OwnerSettlementV2AccountV1::decode(&bytes),
            Err(CodecError::WrongVersion)
        );
        assert_eq!(
            OwnerSettlementV3AccountV1::decode(&bytes[..OWNER_SETTLEMENT_ACCOUNT_BYTES - 1]),
            Err(CodecError::WrongLength)
        );
        let mut long = [0; OWNER_SETTLEMENT_ACCOUNT_BYTES + 1];
        long[..OWNER_SETTLEMENT_ACCOUNT_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            OwnerSettlementV3AccountV1::decode(&long),
            Err(CodecError::WrongLength)
        );

        let mut wrong_tag = bytes;
        wrong_tag[0] = 0x80;
        assert_eq!(
            OwnerSettlementV3AccountV1::decode(&wrong_tag),
            Err(CodecError::WrongTag)
        );
        for withdrawn in [
            OWNER_SETTLEMENT_ACCOUNT_VERSION_V1,
            OWNER_SETTLEMENT_ACCOUNT_VERSION_V2,
        ] {
            let mut bytes = encoded_v3();
            bytes[1] = withdrawn;
            assert_eq!(
                OwnerSettlementV3AccountV1::decode(&bytes),
                Err(CodecError::WrongVersion)
            );
        }
    }

    #[test]
    fn v3_outer_refuses_noncanonical_inner_and_outer_reserved_bytes() {
        let mut inner_padding = encoded_v3();
        inner_padding[2 + OWNER_SETTLEMENT_BODY_V3_BYTES - 1] = 1;
        assert_eq!(
            OwnerSettlementV3AccountV1::decode(&inner_padding),
            Err(CodecError::InvalidState)
        );

        let mut outer_flags = encoded_v3();
        outer_flags[OWNER_SETTLEMENT_ACCOUNT_BYTES - 1] = 1;
        assert_eq!(
            OwnerSettlementV3AccountV1::decode(&outer_flags),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn v5_round_trip_preserves_rent_and_refuses_v4_aliases() {
        let value = value_v5();
        let bytes = value.encode_exact().unwrap();
        assert_eq!(bytes.len(), 340);
        assert_eq!(&bytes[..2], &[0x81, 5]);
        assert_eq!(OwnerSettlementV5AccountV1::decode(&bytes), Ok(value));
        assert_eq!(value.rent.payer, Id32::new([9; 32]).unwrap());
        assert_eq!(
            OwnerSettlementV4AccountV1::decode(&bytes),
            Err(CodecError::WrongLength)
        );

        let mut wrong_version = bytes;
        wrong_version[1] = OWNER_SETTLEMENT_ACCOUNT_VERSION_V4;
        assert_eq!(
            OwnerSettlementV5AccountV1::decode(&wrong_version),
            Err(CodecError::WrongVersion)
        );
    }

    #[test]
    fn v5_refuses_zero_rent_principal_and_hostile_lengths() {
        let value = value_v5();
        let bytes = value.encode_exact().unwrap();
        let principal_offset = 2 + OWNER_SETTLEMENT_BODY_V4_BYTES + 32;
        let mut zero_principal = bytes;
        zero_principal[principal_offset..principal_offset + 8].fill(0);
        assert_eq!(
            OwnerSettlementV5AccountV1::decode(&zero_principal),
            Err(CodecError::InvalidState)
        );
        assert_eq!(
            OwnerSettlementV5AccountV1::decode(&bytes[..bytes.len() - 1]),
            Err(CodecError::WrongLength)
        );
    }
}
