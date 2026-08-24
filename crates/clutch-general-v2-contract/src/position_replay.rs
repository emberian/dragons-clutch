// SPDX-License-Identifier: AGPL-3.0-or-later

//! General-owned purpose Replay V3 extension and structural transition.
//!
//! This module is deliberately structural. It owns canonical bytes, hashes,
//! and the exhaustive General action/endpoint partition, but it does not
//! authenticate an SBF account owner, PDA, receipt, finalized row, or writable
//! account meta. A live action composer must rederive its concrete settlement
//! or structured-claim plan from authenticated prestates before committing the
//! Position and Replay postbodies together.

use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{
    DeletableRentOwnerV1, Identity32V1, PositionAccountV3, PositionLifecycleV3, PositionPurposeV3,
    PositionV3Fields, PositionV3Sha256Backend, RentSplitV2, ReplayV3Envelope,
    ReplayV3EnvelopeFields, ReplayV3EnvelopeHeader, ReplayV3ExtensionSchema, ReplayV3HashBackend,
    ReplayV3Lifecycle, MAX_OUTCOMES,
};

use crate::{CodecError, Id32, Reader, Writer};

/// Exact General purpose-extension schema coordinate (`GEN1`).
pub const GENERAL_REPLAY_EXTENSION_SCHEMA_V1: u32 = u32::from_le_bytes(*b"GEN1");
/// Exact General purpose-extension width.
pub const GENERAL_REPLAY_EXTENSION_V1_BYTES: usize = 136;
/// Exact Replay V3 body width when carrying `GEN1`.
pub const GENERAL_REPLAY_ACCOUNT_V1_BYTES: usize =
    clutch_retirement::PURPOSE_REPLAY_V3_PREFIX_BYTES + GENERAL_REPLAY_EXTENSION_V1_BYTES;
/// Domain for one exact General Position pre/post delta.
pub const GENERAL_REPLAY_DELTA_DOMAIN_V1: &[u8] = b"dragons-clutch/general-replay/delta/v1\0";
/// Domain for the permissionless terminal cut of the canonical Market
/// treasury Position.  The live adapter supplies the exact durable fee and
/// service-terminal authority; no caller transition identity is accepted.
pub const GENERAL_TREASURY_POSITION_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/general-treasury-position-terminal/v1\0";

/// Canonical founding generation for a fresh ordinary General Position.
pub const GENERAL_POSITION_FOUNDING_GENERATION_V1: u64 = 1;

const SETTLEMENT_FAMILY: u8 = 1;
const STRUCTURED_EXCHANGE_FAMILY: u8 = 2;
const COLLATERAL_CASH_FAMILY: u8 = 3;
const FRACTIONAL_REDEMPTION_FAMILY: u8 = 4;
const CLAIM_REPRESENTATION_FAMILY: u8 = 5;
const DEALER_FAMILY: u8 = 6;
const DIRECT_MARKET_FAMILY: u8 = 80;
const TRANSITION_VERSION_V1: u8 = 1;
const OWNER_ACCOUNTING_ROLE: u8 = 1;
const OWNER_CASH_ROLE: u8 = 2;
const DIRECT_BUYER_ROLE: u8 = 3;
const DIRECT_SELLER_ROLE: u8 = 4;
const VIRTUAL_SPLIT_BUYER_ROLE: u8 = 5;
const VIRTUAL_MERGE_SELLER_ROLE: u8 = 6;
const STRUCTURED_GENERAL_ROLE: u8 = 7;
const UNFILLED_RESERVATION_OWNER_ROLE: u8 = 8;
const MERGE_PAYMENT_OWNER_ROLE: u8 = 9;
const PORTFOLIO_PAIR_BUYER_ROLE: u8 = 10;
const PORTFOLIO_PAIR_SELLER_ROLE: u8 = 11;
const PORTFOLIO_ARCHIVE_BUYER_ROLE: u8 = 12;
const PORTFOLIO_ARCHIVE_SELLER_ROLE: u8 = 13;
const FEE_DISTRIBUTION_RECIPIENT_ROLE: u8 = 14;
const DEALER_BUYER_ROLE: u8 = 12;
const DEALER_SELLER_ROLE: u8 = 13;
const GENERAL_COLLATERAL_POSITION_ROLE: u8 = 1;
const DIRECT_MARKET_BUYER_ROLE: u8 = 1;
const DIRECT_MARKET_SELLER_ROLE: u8 = 2;
const DIRECT_MARKET_TREASURY_ROLE: u8 = 3;

/// Exhaustive General Replay transition partition for schema v1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralReplayTransitionKindV1 {
    /// Legacy Intent `Endow`, promoted to the canonical Holder→Hoard deposit.
    Endow,
    /// Legacy Intent `WithdrawCash`, promoted to canonical Hoard→Holder exit.
    WithdrawCash,
    /// Legacy Intent `Split`, locking cash as complete-set backing.
    Split,
    /// Legacy Intent `Merge`, unlocking complete-set backing into cash.
    Merge,
    /// Move Position-owned native Eggs into independently issued bearer claims.
    Materialize,
    /// Burn bearer claims back into Position-owned native Eggs.
    Dematerialize,
    /// Action 25 owner accounting; the Position body is unchanged.
    AccountReceiptEnd,
    /// Action 38 owner cash realization.
    FinalizeOwnerSettlement,
    /// Action 50 exact maker or treasury trading-fee Position credit.
    DistributeTradingFee,
    /// Action 34 terminal cut of the canonical Market treasury Position.
    CloseTreasuryPosition,
    /// Action 26 buyer Position endpoint.
    DirectBuyer,
    /// Action 26 seller Position endpoint.
    DirectSeller,
    /// Action 36 real buyer Position endpoint.
    VirtualSplitBuyer,
    /// Action 37 real seller Position endpoint.
    VirtualMergeSeller,
    /// Action 41 zero-fill Reservation release and co-close endpoint.
    ReleaseUnfilledReservation,
    /// Action 40 merge Reservation payment latch; Position is unchanged.
    FinalizeMergeReceiptPayment,
    /// Action 42 exact coefficient-portfolio buyer endpoint.
    PortfolioPairBuyer,
    /// Action 42 exact coefficient-portfolio seller endpoint.
    PortfolioPairSeller,
    /// Action 44 buyer Position child retirement endpoint.
    RetirePortfolioPairBuyerArchive,
    /// Action 44 seller Position child retirement endpoint.
    RetirePortfolioPairSellerArchive,
    /// Action 35 General Position endpoint of a structured exchange.
    StructuredGeneral,
    /// FractionalRedemption action 2 internal exact-lot endpoint.
    FractionalRedeemInternalExact,
    /// FractionalRedemption action 4 internal credited endpoint.
    FractionalRedeemInternalCredit,
    /// FractionalRedemption action 6 internal credit-payout endpoint.
    FractionalTransferCreditPayout,
    /// FractionalRedemption action 7 internal credit-payout endpoint.
    FractionalMergeCreditPayout,
    /// Dealer action 15 buyer collection releases reserved cash into custody.
    DealerCollectBuyer,
    /// Dealer action 15 seller collection moves Reservation-owned Eggs and
    /// may return an unfilled remainder to the Position.
    DealerCollectSeller,
    /// Dealer action 16 buyer delivery credits native Eggs and closes the
    /// Position's Reservation liability.
    DealerDeliverBuyer,
    /// Dealer action 16 seller delivery credits cash and closes the
    /// Position's Reservation liability.
    DealerDeliverSeller,
    /// Direct Market V1 action 2 buyer Reservation admission.
    DirectMarketAdmitBuyer,
    /// Direct Market V1 action 2 seller Reservation admission.
    DirectMarketAdmitSeller,
    /// Direct Market V1 action 3 buyer Reservation cancellation.
    DirectMarketCancelBuyer,
    /// Direct Market V1 action 3 seller Reservation cancellation.
    DirectMarketCancelSeller,
    /// Direct Market V1 action 9 buyer settlement endpoint.
    DirectMarketSettleBuyer,
    /// Direct Market V1 action 9 seller settlement endpoint.
    DirectMarketSettleSeller,
    /// Direct Market V1 action 9 authenticated revenue-treasury credit.
    DirectMarketSettleTreasury,
    /// Direct Market V1 action 10 buyer empty-market lapse endpoint.
    DirectMarketLapseEmptyBuyer,
    /// Direct Market V1 action 10 seller empty-market lapse endpoint.
    DirectMarketLapseEmptySeller,
    /// Direct Market V1 action 11 buyer unselected lapse endpoint.
    DirectMarketLapseUnselectedBuyer,
    /// Direct Market V1 action 11 seller unselected lapse endpoint.
    DirectMarketLapseUnselectedSeller,
    /// Direct Market V1 action 12 buyer selected-book lapse endpoint.
    DirectMarketLapseSelectedBuyer,
    /// Direct Market V1 action 12 seller selected-book lapse endpoint.
    DirectMarketLapseSelectedSeller,
}

impl GeneralReplayTransitionKindV1 {
    fn coordinates(self) -> (u8, u8, u8, u8) {
        match self {
            Self::Endow => (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                1,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::WithdrawCash => (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::Split => (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                3,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::Merge => (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                4,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::Materialize => (
                CLAIM_REPRESENTATION_FAMILY,
                TRANSITION_VERSION_V1,
                1,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::Dematerialize => (
                CLAIM_REPRESENTATION_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::AccountReceiptEnd => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                25,
                OWNER_ACCOUNTING_ROLE,
            ),
            Self::FinalizeOwnerSettlement => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                38,
                OWNER_CASH_ROLE,
            ),
            Self::DistributeTradingFee => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                50,
                FEE_DISTRIBUTION_RECIPIENT_ROLE,
            ),
            Self::CloseTreasuryPosition => (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                34,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::DirectBuyer => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                26,
                DIRECT_BUYER_ROLE,
            ),
            Self::DirectSeller => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                26,
                DIRECT_SELLER_ROLE,
            ),
            Self::VirtualSplitBuyer => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                36,
                VIRTUAL_SPLIT_BUYER_ROLE,
            ),
            Self::VirtualMergeSeller => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                37,
                VIRTUAL_MERGE_SELLER_ROLE,
            ),
            Self::ReleaseUnfilledReservation => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                41,
                UNFILLED_RESERVATION_OWNER_ROLE,
            ),
            Self::FinalizeMergeReceiptPayment => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                40,
                MERGE_PAYMENT_OWNER_ROLE,
            ),
            Self::PortfolioPairBuyer => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                42,
                PORTFOLIO_PAIR_BUYER_ROLE,
            ),
            Self::PortfolioPairSeller => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                42,
                PORTFOLIO_PAIR_SELLER_ROLE,
            ),
            Self::RetirePortfolioPairBuyerArchive => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                44,
                PORTFOLIO_ARCHIVE_BUYER_ROLE,
            ),
            Self::RetirePortfolioPairSellerArchive => (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                44,
                PORTFOLIO_ARCHIVE_SELLER_ROLE,
            ),
            Self::StructuredGeneral => (
                STRUCTURED_EXCHANGE_FAMILY,
                TRANSITION_VERSION_V1,
                35,
                STRUCTURED_GENERAL_ROLE,
            ),
            Self::FractionalRedeemInternalExact => (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::FractionalRedeemInternalCredit => (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                4,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::FractionalTransferCreditPayout => (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                6,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::FractionalMergeCreditPayout => (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                7,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ),
            Self::DealerCollectBuyer => (
                DEALER_FAMILY,
                TRANSITION_VERSION_V1,
                15,
                DEALER_BUYER_ROLE,
            ),
            Self::DealerCollectSeller => (
                DEALER_FAMILY,
                TRANSITION_VERSION_V1,
                15,
                DEALER_SELLER_ROLE,
            ),
            Self::DealerDeliverBuyer => (
                DEALER_FAMILY,
                TRANSITION_VERSION_V1,
                16,
                DEALER_BUYER_ROLE,
            ),
            Self::DealerDeliverSeller => (
                DEALER_FAMILY,
                TRANSITION_VERSION_V1,
                16,
                DEALER_SELLER_ROLE,
            ),
            Self::DirectMarketAdmitBuyer => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                DIRECT_MARKET_BUYER_ROLE,
            ),
            Self::DirectMarketAdmitSeller => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                DIRECT_MARKET_SELLER_ROLE,
            ),
            Self::DirectMarketCancelBuyer => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                3,
                DIRECT_MARKET_BUYER_ROLE,
            ),
            Self::DirectMarketCancelSeller => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                3,
                DIRECT_MARKET_SELLER_ROLE,
            ),
            Self::DirectMarketSettleBuyer => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                9,
                DIRECT_MARKET_BUYER_ROLE,
            ),
            Self::DirectMarketSettleSeller => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                9,
                DIRECT_MARKET_SELLER_ROLE,
            ),
            Self::DirectMarketSettleTreasury => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                9,
                DIRECT_MARKET_TREASURY_ROLE,
            ),
            Self::DirectMarketLapseEmptyBuyer => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                10,
                DIRECT_MARKET_BUYER_ROLE,
            ),
            Self::DirectMarketLapseEmptySeller => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                10,
                DIRECT_MARKET_SELLER_ROLE,
            ),
            Self::DirectMarketLapseUnselectedBuyer => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                11,
                DIRECT_MARKET_BUYER_ROLE,
            ),
            Self::DirectMarketLapseUnselectedSeller => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                11,
                DIRECT_MARKET_SELLER_ROLE,
            ),
            Self::DirectMarketLapseSelectedBuyer => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                12,
                DIRECT_MARKET_BUYER_ROLE,
            ),
            Self::DirectMarketLapseSelectedSeller => (
                DIRECT_MARKET_FAMILY,
                TRANSITION_VERSION_V1,
                12,
                DIRECT_MARKET_SELLER_ROLE,
            ),
        }
    }

    fn from_coordinates(family: u8, version: u8, action: u8, role: u8) -> Result<Self, CodecError> {
        match (family, version, action, role) {
            (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                1,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::Endow),
            (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::WithdrawCash),
            (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                3,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::Split),
            (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                4,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::Merge),
            (
                CLAIM_REPRESENTATION_FAMILY,
                TRANSITION_VERSION_V1,
                1,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::Materialize),
            (
                CLAIM_REPRESENTATION_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::Dematerialize),
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 25, OWNER_ACCOUNTING_ROLE) => {
                Ok(Self::AccountReceiptEnd)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 38, OWNER_CASH_ROLE) => {
                Ok(Self::FinalizeOwnerSettlement)
            }
            (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                50,
                FEE_DISTRIBUTION_RECIPIENT_ROLE,
            ) => Ok(Self::DistributeTradingFee),
            (
                COLLATERAL_CASH_FAMILY,
                TRANSITION_VERSION_V1,
                34,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::CloseTreasuryPosition),
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 26, DIRECT_BUYER_ROLE) => {
                Ok(Self::DirectBuyer)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 26, DIRECT_SELLER_ROLE) => {
                Ok(Self::DirectSeller)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 36, VIRTUAL_SPLIT_BUYER_ROLE) => {
                Ok(Self::VirtualSplitBuyer)
            }
            (SETTLEMENT_FAMILY, TRANSITION_VERSION_V1, 37, VIRTUAL_MERGE_SELLER_ROLE) => {
                Ok(Self::VirtualMergeSeller)
            }
            (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                41,
                UNFILLED_RESERVATION_OWNER_ROLE,
            ) => Ok(Self::ReleaseUnfilledReservation),
            (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                40,
                MERGE_PAYMENT_OWNER_ROLE,
            ) => Ok(Self::FinalizeMergeReceiptPayment),
            (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                42,
                PORTFOLIO_PAIR_BUYER_ROLE,
            ) => Ok(Self::PortfolioPairBuyer),
            (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                42,
                PORTFOLIO_PAIR_SELLER_ROLE,
            ) => Ok(Self::PortfolioPairSeller),
            (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                44,
                PORTFOLIO_ARCHIVE_BUYER_ROLE,
            ) => Ok(Self::RetirePortfolioPairBuyerArchive),
            (
                SETTLEMENT_FAMILY,
                TRANSITION_VERSION_V1,
                44,
                PORTFOLIO_ARCHIVE_SELLER_ROLE,
            ) => Ok(Self::RetirePortfolioPairSellerArchive),
            (STRUCTURED_EXCHANGE_FAMILY, TRANSITION_VERSION_V1, 35, STRUCTURED_GENERAL_ROLE) => {
                Ok(Self::StructuredGeneral)
            }
            (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                2,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::FractionalRedeemInternalExact),
            (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                4,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::FractionalRedeemInternalCredit),
            (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                6,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::FractionalTransferCreditPayout),
            (
                FRACTIONAL_REDEMPTION_FAMILY,
                TRANSITION_VERSION_V1,
                7,
                GENERAL_COLLATERAL_POSITION_ROLE,
            ) => Ok(Self::FractionalMergeCreditPayout),
            (DEALER_FAMILY, TRANSITION_VERSION_V1, 15, DEALER_BUYER_ROLE) => {
                Ok(Self::DealerCollectBuyer)
            }
            (DEALER_FAMILY, TRANSITION_VERSION_V1, 15, DEALER_SELLER_ROLE) => {
                Ok(Self::DealerCollectSeller)
            }
            (DEALER_FAMILY, TRANSITION_VERSION_V1, 16, DEALER_BUYER_ROLE) => {
                Ok(Self::DealerDeliverBuyer)
            }
            (DEALER_FAMILY, TRANSITION_VERSION_V1, 16, DEALER_SELLER_ROLE) => {
                Ok(Self::DealerDeliverSeller)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 2, DIRECT_MARKET_BUYER_ROLE) => {
                Ok(Self::DirectMarketAdmitBuyer)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 2, DIRECT_MARKET_SELLER_ROLE) => {
                Ok(Self::DirectMarketAdmitSeller)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 3, DIRECT_MARKET_BUYER_ROLE) => {
                Ok(Self::DirectMarketCancelBuyer)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 3, DIRECT_MARKET_SELLER_ROLE) => {
                Ok(Self::DirectMarketCancelSeller)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 9, DIRECT_MARKET_BUYER_ROLE) => {
                Ok(Self::DirectMarketSettleBuyer)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 9, DIRECT_MARKET_SELLER_ROLE) => {
                Ok(Self::DirectMarketSettleSeller)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 9, DIRECT_MARKET_TREASURY_ROLE) => {
                Ok(Self::DirectMarketSettleTreasury)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 10, DIRECT_MARKET_BUYER_ROLE) => {
                Ok(Self::DirectMarketLapseEmptyBuyer)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 10, DIRECT_MARKET_SELLER_ROLE) => {
                Ok(Self::DirectMarketLapseEmptySeller)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 11, DIRECT_MARKET_BUYER_ROLE) => {
                Ok(Self::DirectMarketLapseUnselectedBuyer)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 11, DIRECT_MARKET_SELLER_ROLE) => {
                Ok(Self::DirectMarketLapseUnselectedSeller)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 12, DIRECT_MARKET_BUYER_ROLE) => {
                Ok(Self::DirectMarketLapseSelectedBuyer)
            }
            (DIRECT_MARKET_FAMILY, TRANSITION_VERSION_V1, 12, DIRECT_MARKET_SELLER_ROLE) => {
                Ok(Self::DirectMarketLapseSelectedSeller)
            }
            _ => Err(CodecError::InvalidState),
        }
    }

    /// Exact owner-local action coordinate within this Replay family.
    pub fn action(self) -> u8 {
        self.coordinates().2
    }

    /// Exact endpoint role within that action.
    pub fn role(self) -> u8 {
        self.coordinates().3
    }
}

/// Founding versus advanced General extension state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralReplayExtensionStateV1 {
    Initial,
    Advanced,
}

/// Canonical fixed General extension under the common Replay V3 prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReplayExtensionV1 {
    general_market_runtime: Id32,
    current_position_semantic_id: Id32,
    last_transition_id: Id32,
    last_delta_id: Id32,
    last_kind: Option<GeneralReplayTransitionKindV1>,
    state: GeneralReplayExtensionStateV1,
}

impl GeneralReplayExtensionV1 {
    /// Construct the unique founding extension for an exact current Position.
    pub fn initial(
        general_market_runtime: Id32,
        current_position_semantic_id: Id32,
    ) -> Result<Self, CodecError> {
        let value = Self {
            general_market_runtime,
            current_position_semantic_id,
            last_transition_id: Id32::ZERO,
            last_delta_id: Id32::ZERO,
            last_kind: None,
            state: GeneralReplayExtensionStateV1::Initial,
        };
        value.validate()?;
        Ok(value)
    }

    fn advanced(
        self,
        kind: GeneralReplayTransitionKindV1,
        transition_id: Id32,
        delta_id: Id32,
        current_position_semantic_id: Id32,
    ) -> Result<Self, CodecError> {
        self.validate()?;
        let value = Self {
            general_market_runtime: self.general_market_runtime,
            current_position_semantic_id,
            last_transition_id: transition_id,
            last_delta_id: delta_id,
            last_kind: Some(kind),
            state: GeneralReplayExtensionStateV1::Advanced,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), CodecError> {
        if self.general_market_runtime.is_zero() || self.current_position_semantic_id.is_zero() {
            return Err(CodecError::ZeroIdentity);
        }
        match (self.state, self.last_kind) {
            (GeneralReplayExtensionStateV1::Initial, None)
                if self.last_transition_id.is_zero() && self.last_delta_id.is_zero() =>
            {
                Ok(())
            }
            (GeneralReplayExtensionStateV1::Advanced, Some(_))
                if !self.last_transition_id.is_zero()
                    && !self.last_delta_id.is_zero()
                    && self.last_transition_id != self.last_delta_id =>
            {
                Ok(())
            }
            _ => Err(CodecError::InvalidState),
        }
    }

    /// General runtime Market PDA.
    pub const fn general_market_runtime(self) -> Id32 {
        self.general_market_runtime
    }

    /// Position semantic ID current at this Replay state.
    pub const fn current_position_semantic_id(self) -> Id32 {
        self.current_position_semantic_id
    }

    /// Most recently consumed transition identity, absent at founding.
    pub const fn last_transition_id(self) -> Id32 {
        self.last_transition_id
    }

    /// Most recently consumed Position-delta identity, absent at founding.
    pub const fn last_delta_id(self) -> Id32 {
        self.last_delta_id
    }

    /// Most recently consumed exact action/role tuple, absent at founding.
    pub const fn last_kind(self) -> Option<GeneralReplayTransitionKindV1> {
        self.last_kind
    }

    /// Encode exactly 136 canonical bytes.
    pub fn encode(self) -> Result<[u8; GENERAL_REPLAY_EXTENSION_V1_BYTES], CodecError> {
        self.validate()?;
        let mut output = [0_u8; GENERAL_REPLAY_EXTENSION_V1_BYTES];
        let mut writer = Writer::exact(&mut output, GENERAL_REPLAY_EXTENSION_V1_BYTES)?;
        writer.bytes(&self.general_market_runtime.bytes())?;
        writer.bytes(&self.current_position_semantic_id.bytes())?;
        writer.bytes(&self.last_transition_id.bytes())?;
        writer.bytes(&self.last_delta_id.bytes())?;
        let (family, version, action, role) = match self.last_kind {
            Some(kind) => kind.coordinates(),
            None => (0, 0, 0, 0),
        };
        writer.u8(action)?;
        writer.u8(match self.state {
            GeneralReplayExtensionStateV1::Initial => 0,
            GeneralReplayExtensionStateV1::Advanced => 1,
        })?;
        writer.u8(family)?;
        writer.u8(version)?;
        writer.u8(role)?;
        writer.bytes(&[0; 3])?;
        writer.finish()?;
        Ok(output)
    }

    /// Decode exactly 136 hostile bytes and reject every unallocated tuple.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        let mut reader = Reader::exact(input, GENERAL_REPLAY_EXTENSION_V1_BYTES)?;
        let general_market_runtime = Id32::new(reader.array()?)?;
        let current_position_semantic_id = Id32::new(reader.array()?)?;
        let last_transition_id = Id32::from_bytes(reader.array()?);
        let last_delta_id = Id32::from_bytes(reader.array()?);
        let action = reader.u8()?;
        let state = reader.u8()?;
        let family = reader.u8()?;
        let version = reader.u8()?;
        let role = reader.u8()?;
        if reader.array::<3>()? != [0; 3] {
            return Err(CodecError::NonCanonicalPadding);
        }
        reader.finish()?;
        let (state, last_kind) = match state {
            0 if family == 0 && version == 0 && action == 0 && role == 0 => {
                (GeneralReplayExtensionStateV1::Initial, None)
            }
            1 => (
                GeneralReplayExtensionStateV1::Advanced,
                Some(GeneralReplayTransitionKindV1::from_coordinates(
                    family, version, action, role,
                )?),
            ),
            _ => return Err(CodecError::InvalidState),
        };
        let value = Self {
            general_market_runtime,
            current_position_semantic_id,
            last_transition_id,
            last_delta_id,
            last_kind,
            state,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Hash-checked structural General Position/Replay prestate.
///
/// Private fields prevent callers from assembling one without passing the
/// exact canonical Position and Replay bodies through the projection below.
/// This still does not authenticate an SBF program owner or PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPositionReplayPrestateV1 {
    position: AuthenticatedPositionV3,
    replay_account: Id32,
    replay_semantic_id: Id32,
    replay_header: ReplayV3EnvelopeHeader,
    extension: GeneralReplayExtensionV1,
}

/// Exact pure founding plan for one canonical General Position/Replay pair.
///
/// The runtime remains responsible for authenticating both absent PDAs,
/// immutable Market/Realm collateral facts, rent calculations, payer funding,
/// and account creation. Private fields ensure those callers cannot replace
/// either semantic body after this constructor has joined the complete facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPositionReplayFoundingPlanV1 {
    position: PositionAccountV3,
    position_semantic_id: Id32,
    position_body: [u8; clutch_retirement::POSITION_V3_BYTES],
    replay_semantic_id: Id32,
    replay_body: [u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES],
}

/// Exact pure founding plan for the Position half of a Product-funded
/// treasury pair.  This split exists because ScheduleV4 capitalizes slots 47
/// and 48 independently while preserving one canonical semantic constructor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPositionFoundingPlanV1 {
    position: PositionAccountV3,
    position_semantic_id: Id32,
    position_body: [u8; clutch_retirement::POSITION_V3_BYTES],
}

impl GeneralPositionFoundingPlanV1 {
    /// Canonical zero-liability Position postimage.
    pub const fn position(self) -> PositionAccountV3 { self.position }
    /// Semantic identity of the exact Position postimage.
    pub const fn position_semantic_id(self) -> Id32 { self.position_semantic_id }
    /// Canonical Position bytes.
    pub const fn position_body(&self) -> &[u8; clutch_retirement::POSITION_V3_BYTES] {
        &self.position_body
    }
}

/// Exact pure founding plan for the Replay half of a Product-funded treasury
/// pair.  The caller must supply the semantic identity of the already
/// authenticated slot-47 Position postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReplayFoundingPlanV1 {
    replay_semantic_id: Id32,
    replay_body: [u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES],
}

impl GeneralReplayFoundingPlanV1 {
    /// Semantic identity of the exact Replay postimage.
    pub const fn replay_semantic_id(self) -> Id32 { self.replay_semantic_id }
    /// Canonical Replay bytes.
    pub const fn replay_body(&self) -> &[u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES] {
        &self.replay_body
    }
}

impl GeneralPositionReplayFoundingPlanV1 {
    /// Canonical zero-balance Position body at founding generation one.
    pub const fn position(self) -> PositionAccountV3 {
        self.position
    }

    /// Internally derived semantic identity of the founding Position body.
    pub const fn position_semantic_id(self) -> Id32 {
        self.position_semantic_id
    }

    /// Exact canonical 480-byte founding Position body.
    pub const fn position_body(&self) -> &[u8; clutch_retirement::POSITION_V3_BYTES] {
        &self.position_body
    }

    /// Internally derived semantic identity of the founding Replay body.
    pub const fn replay_semantic_id(self) -> Id32 {
        self.replay_semantic_id
    }

    /// Canonical Replay V3 PDA bump retained by the hostile prestate.
    pub const fn replay_bump(self) -> u8 {
        self.replay_header.stored_bump()
    }

    /// Exact canonical 344-byte founding Replay body.
    pub const fn replay_body(&self) -> &[u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES] {
        &self.replay_body
    }
}

/// Construct the unique zero-liability General Position and sequence-zero
/// GEN1 Replay pair for complete authenticated external founding facts.
#[allow(clippy::too_many_arguments)]
pub fn found_general_position_replay_v1<B>(
    position_account: Identity32V1,
    replay_account: Identity32V1,
    market_instance_id: Identity32V1,
    realm_id: Identity32V1,
    collateral_policy_id: Identity32V1,
    collateral_release_id: Identity32V1,
    owner: Identity32V1,
    general_market_runtime: Identity32V1,
    outcome_count: u8,
    position_bump: u8,
    replay_bump: u8,
    position_rent: RentSplitV2,
    replay_rent: DeletableRentOwnerV1,
    backend: &B,
) -> Result<GeneralPositionReplayFoundingPlanV1, CodecError>
where
    B: PositionV3Sha256Backend + ReplayV3HashBackend,
{
    let position_plan = found_general_position_v1(
        position_account, replay_account, market_instance_id, realm_id,
        collateral_policy_id, collateral_release_id, owner, general_market_runtime,
        outcome_count, position_bump, position_rent, backend,
    )?;
    let replay_plan = found_general_replay_v1(
        position_account, replay_account, owner, general_market_runtime,
        replay_bump, replay_rent, position_plan.position_semantic_id, backend,
    )?;
    Ok(GeneralPositionReplayFoundingPlanV1 {
        position: position_plan.position,
        position_semantic_id: position_plan.position_semantic_id,
        position_body: position_plan.position_body,
        replay_semantic_id: replay_plan.replay_semantic_id,
        replay_body: replay_plan.replay_body,
    })
}

/// Construct the unique zero-liability PositionV3 postimage independently of
/// its later ScheduleV4 Replay slot.
#[allow(clippy::too_many_arguments)]
pub fn found_general_position_v1<B>(
    position_account: Identity32V1,
    replay_account: Identity32V1,
    market_instance_id: Identity32V1,
    realm_id: Identity32V1,
    collateral_policy_id: Identity32V1,
    collateral_release_id: Identity32V1,
    owner: Identity32V1,
    general_market_runtime: Identity32V1,
    outcome_count: u8,
    position_bump: u8,
    position_rent: RentSplitV2,
    backend: &B,
) -> Result<GeneralPositionFoundingPlanV1, CodecError>
where
    B: PositionV3Sha256Backend,
{
    if position_account == replay_account
        || owner == replay_account
        || general_market_runtime == replay_account
        || usize::from(outcome_count) == 0
        || usize::from(outcome_count) > MAX_OUTCOMES
    {
        return Err(CodecError::MismatchedBinding);
    }
    let position = PositionAccountV3::new(PositionV3Fields {
        purpose: PositionPurposeV3::General,
        lifecycle: PositionLifecycleV3::Open,
        outcome_count,
        stored_bump: position_bump,
        generation: GENERAL_POSITION_FOUNDING_GENERATION_V1,
        market_instance_id,
        realm_id,
        collateral_policy_id,
        collateral_release_id,
        owner,
        controller: owner,
        replay_account,
        purpose_binding_id: general_market_runtime,
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        native_eggs: [0; MAX_OUTCOMES],
        outstanding_reservations: 0,
        rent: position_rent,
    })
    .map_err(|_| CodecError::InvalidState)?;
    let position_semantic_id = Id32::new(
        position
            .semantic_id(backend)
            .map_err(|_| CodecError::InvalidState)?
            .bytes(),
    )?;
    let position_body = position.encode().map_err(|_| CodecError::InvalidState)?;
    Ok(GeneralPositionFoundingPlanV1 {
        position,
        position_semantic_id,
        position_body,
    })
}

/// Construct the unique GEN1 ReplayV3 postimage for an already-authenticated
/// founding Position semantic identity.
#[allow(clippy::too_many_arguments)]
pub fn found_general_replay_v1<B>(
    position_account: Identity32V1,
    replay_account: Identity32V1,
    owner: Identity32V1,
    general_market_runtime: Identity32V1,
    replay_bump: u8,
    replay_rent: DeletableRentOwnerV1,
    position_semantic_id: Id32,
    backend: &B,
) -> Result<GeneralReplayFoundingPlanV1, CodecError>
where
    B: ReplayV3HashBackend,
{
    if position_account == replay_account
        || owner == replay_account
        || general_market_runtime == replay_account
        || position_semantic_id.is_zero()
    {
        return Err(CodecError::MismatchedBinding);
    }
    let extension = GeneralReplayExtensionV1::initial(
        Id32::new(general_market_runtime.bytes())?,
        position_semantic_id,
    )?
    .encode()?;
    let header = ReplayV3EnvelopeHeader::new_live(
        ReplayV3EnvelopeFields {
            position_account,
            replay_account,
            purpose: PositionPurposeV3::General,
            purpose_binding_id: general_market_runtime,
            position_generation: GENERAL_POSITION_FOUNDING_GENERATION_V1,
            next_sequence: 0,
            stored_bump: replay_bump,
            rent: replay_rent,
        },
        ReplayV3ExtensionSchema::new(GENERAL_REPLAY_EXTENSION_SCHEMA_V1)
            .map_err(|_| CodecError::InvalidState)?,
        &extension,
        backend,
    )
    .map_err(|_| CodecError::InvalidState)?;
    let envelope = ReplayV3Envelope::from_header(header, &extension, backend)
        .map_err(|_| CodecError::InvalidState)?;
    let replay_semantic_id = Id32::new(
        envelope
            .semantic_id(backend)
            .map_err(|_| CodecError::InvalidState)?
            .bytes(),
    )?;
    let mut replay_body = [0_u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES];
    envelope
        .encode_into(&mut replay_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    Ok(GeneralReplayFoundingPlanV1 {
        replay_semantic_id,
        replay_body,
    })
}

impl GeneralPositionReplayPrestateV1 {
    /// Exact checked Position prestate.
    pub const fn position(self) -> AuthenticatedPositionV3 {
        self.position
    }

    /// Exact Replay V3 account key retained by its body.
    pub const fn replay_account(self) -> Id32 {
        self.replay_account
    }

    /// Internally derived Replay V3 semantic ID.
    pub const fn replay_semantic_id(self) -> Id32 {
        self.replay_semantic_id
    }

    /// Exact ordinal consumed by the next General transition.
    pub const fn next_sequence(self) -> u64 {
        self.replay_header.next_sequence()
    }

    /// Exact decoded General extension.
    pub const fn extension(self) -> GeneralReplayExtensionV1 {
        self.extension
    }
}

/// Recompute and compare the exact transition immediately preceding a checked
/// General Replay prestate.
///
/// This is structural, not an execution capability. The action-specific
/// composer must derive `prior_position_semantic_id`, `transition_id`, and
/// `transition_evidence_id` from its authenticated semantic owners. It is
/// useful for a sequence of same-Position transitions such as per-receipt
/// merge payments, where the current Replay retains the prior transition and
/// delta but no duplicate payment authority account should be invented.
pub fn verify_general_replay_last_transition_v1<B: ReplayV3HashBackend>(
    prestate: GeneralPositionReplayPrestateV1,
    prior_position_semantic_id: Id32,
    kind: GeneralReplayTransitionKindV1,
    transition_id: Id32,
    transition_evidence_id: Id32,
    backend: &B,
) -> Result<(), CodecError> {
    if prior_position_semantic_id.is_zero()
        || transition_id.is_zero()
        || transition_evidence_id.is_zero()
    {
        return Err(CodecError::ZeroIdentity);
    }
    let consumed_sequence = prestate
        .replay_header
        .next_sequence()
        .checked_sub(1)
        .ok_or(CodecError::InvalidState)?;
    let extension = prestate.extension;
    let current_position_semantic_id = Id32::new(prestate.position.semantic_id)?;
    let generation = prestate.position.semantic.fields().generation;
    let (family, version, action, role) = kind.coordinates();
    let expected_delta_id = Id32::new(backend.sha256_parts(&[
        GENERAL_REPLAY_DELTA_DOMAIN_V1,
        &[family],
        &[version],
        &[action],
        &[role],
        &consumed_sequence.to_le_bytes(),
        &transition_id.bytes(),
        &transition_evidence_id.bytes(),
        &prestate.position.account,
        &prior_position_semantic_id.bytes(),
        &current_position_semantic_id.bytes(),
        &generation.to_le_bytes(),
        &generation.to_le_bytes(),
    ]))?;
    if extension.last_kind() != Some(kind)
        || extension.last_transition_id() != transition_id
        || extension.last_delta_id() != expected_delta_id
        || extension.current_position_semantic_id() != current_position_semantic_id
    {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(())
}

/// Decode and structurally bind exact Position V3 and General Replay V3 bytes.
pub fn project_general_position_replay_prestate_v1<B>(
    replay_account: Id32,
    canonical_replay_bump: u8,
    expected_next_sequence: u64,
    replay_body: &[u8],
    position: AuthenticatedPositionV3,
    backend: &B,
) -> Result<GeneralPositionReplayPrestateV1, CodecError>
where
    B: PositionV3Sha256Backend + ReplayV3HashBackend,
{
    position
        .validate()
        .map_err(|_| CodecError::MismatchedBinding)?;
    let position_fields = position.semantic.fields();
    let derived_position_id = position
        .semantic
        .semantic_id(backend)
        .map_err(|_| CodecError::MismatchedBinding)?;
    if derived_position_id.bytes() != position.semantic_id {
        return Err(CodecError::MismatchedBinding);
    }
    let envelope = ReplayV3Envelope::decode(replay_body, backend)
        .map_err(|_| CodecError::MismatchedBinding)?;
    let header = envelope.header();
    let extension = GeneralReplayExtensionV1::decode(envelope.extension())?;
    let replay_semantic_id = Id32::new(
        envelope
            .semantic_id(backend)
            .map_err(|_| CodecError::MismatchedBinding)?
            .bytes(),
    )?;
    if replay_account.is_zero()
        || header.lifecycle() != ReplayV3Lifecycle::Live
        || header.purpose() != PositionPurposeV3::General
        || header.extension_schema().get() != GENERAL_REPLAY_EXTENSION_SCHEMA_V1
        || usize::try_from(header.extension_len()).map_err(|_| CodecError::ArithmeticOverflow)?
            != GENERAL_REPLAY_EXTENSION_V1_BYTES
        || header.replay_account().bytes() != replay_account.bytes()
        || header.stored_bump() != canonical_replay_bump
        || header.next_sequence() != expected_next_sequence
        || header.replay_account() != position_fields.replay_account
        || header.position_account().bytes() != position.account
        || header.purpose() != position_fields.purpose
        || header.purpose_binding_id() != position_fields.purpose_binding_id
        || header.position_generation() != position_fields.generation
        || extension.general_market_runtime.bytes() != position.general_market_runtime
        || extension.current_position_semantic_id.bytes() != position.semantic_id
    {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(GeneralPositionReplayPrestateV1 {
        position,
        replay_account,
        replay_semantic_id,
        replay_header: header,
        extension,
    })
}

/// Structural exact Replay successor; never a standalone execution capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReplayTransitionPlanV1 {
    replay_account: Id32,
    replay_prestate_semantic_id: Id32,
    replay_poststate_semantic_id: Id32,
    replay_poststate_body: [u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES],
    position_account: Id32,
    position_prestate_semantic_id: Id32,
    position_poststate_semantic_id: Id32,
    kind: GeneralReplayTransitionKindV1,
    transition_id: Id32,
    transition_evidence_id: Id32,
    delta_id: Id32,
    consumed_sequence: u64,
    next_sequence: u64,
}

impl GeneralReplayTransitionPlanV1 {
    /// Replay account to compare-and-write.
    pub const fn replay_account(&self) -> Id32 {
        self.replay_account
    }

    /// Exact Replay prestate semantic ID.
    pub const fn replay_prestate_semantic_id(&self) -> Id32 {
        self.replay_prestate_semantic_id
    }

    /// Exact internally derived Replay successor semantic ID.
    pub const fn replay_poststate_semantic_id(&self) -> Id32 {
        self.replay_poststate_semantic_id
    }

    /// Exact canonical 344-byte Replay successor body.
    pub const fn replay_poststate_body(&self) -> &[u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES] {
        &self.replay_poststate_body
    }

    /// Position account serialized by this Replay.
    pub const fn position_account(&self) -> Id32 {
        self.position_account
    }

    /// Position semantic ID before the action.
    pub const fn position_prestate_semantic_id(&self) -> Id32 {
        self.position_prestate_semantic_id
    }

    /// Internally derived Position semantic ID after the action.
    pub const fn position_poststate_semantic_id(&self) -> Id32 {
        self.position_poststate_semantic_id
    }

    /// Exact action and endpoint role.
    pub const fn kind(&self) -> GeneralReplayTransitionKindV1 {
        self.kind
    }

    /// Exact action-specific transition identity.
    pub const fn transition_id(&self) -> Id32 {
        self.transition_id
    }

    /// Exact semantic evidence digest that must authenticate the transition.
    pub const fn transition_evidence_id(&self) -> Id32 {
        self.transition_evidence_id
    }

    /// Domain-separated Position delta identity.
    pub const fn delta_id(&self) -> Id32 {
        self.delta_id
    }

    /// Ordinal consumed by this transition.
    pub const fn consumed_sequence(&self) -> u64 {
        self.consumed_sequence
    }

    /// Exact successor ordinal.
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

/// Project one exact structural General Replay successor.
///
/// `transition_id` and `transition_evidence_id` are committed but not
/// authenticated here. A live action-specific composer must obtain them from
/// its private typed action plan and then rederive this whole result.
pub fn project_general_replay_transition_v1<B>(
    prestate: GeneralPositionReplayPrestateV1,
    position_poststate: PositionSettlementPoststateV3,
    kind: GeneralReplayTransitionKindV1,
    transition_id: Id32,
    transition_evidence_id: Id32,
    backend: &B,
) -> Result<GeneralReplayTransitionPlanV1, CodecError>
where
    B: PositionV3Sha256Backend + ReplayV3HashBackend,
{
    if transition_id.is_zero() || transition_evidence_id.is_zero() {
        return Err(CodecError::ZeroIdentity);
    }
    let position_prestate = prestate.position;
    let pre_fields = position_prestate.semantic.fields();
    let post_fields = position_poststate.semantic.fields();
    if position_poststate.account != position_prestate.account
        || position_poststate.general_market_runtime != position_prestate.general_market_runtime
        || position_poststate.prestate_semantic_id != position_prestate.semantic_id
    {
        return Err(CodecError::MismatchedBinding);
    }
    let expected_outstanding_reservations = match kind {
        GeneralReplayTransitionKindV1::DirectMarketAdmitBuyer
        | GeneralReplayTransitionKindV1::DirectMarketAdmitSeller => pre_fields
            .outstanding_reservations
            .checked_add(1)
            .ok_or(CodecError::InvalidState)?,
        GeneralReplayTransitionKindV1::ReleaseUnfilledReservation
        | GeneralReplayTransitionKindV1::RetirePortfolioPairBuyerArchive
        | GeneralReplayTransitionKindV1::RetirePortfolioPairSellerArchive
        | GeneralReplayTransitionKindV1::DealerDeliverBuyer
        | GeneralReplayTransitionKindV1::DealerDeliverSeller
        | GeneralReplayTransitionKindV1::DirectMarketCancelBuyer
        | GeneralReplayTransitionKindV1::DirectMarketCancelSeller
        | GeneralReplayTransitionKindV1::DirectMarketSettleBuyer
        | GeneralReplayTransitionKindV1::DirectMarketSettleSeller
        | GeneralReplayTransitionKindV1::DirectMarketLapseEmptyBuyer
        | GeneralReplayTransitionKindV1::DirectMarketLapseEmptySeller
        | GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedBuyer
        | GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedSeller
        | GeneralReplayTransitionKindV1::DirectMarketLapseSelectedBuyer
        | GeneralReplayTransitionKindV1::DirectMarketLapseSelectedSeller => pre_fields
            .outstanding_reservations
            .checked_sub(1)
            .ok_or(CodecError::InvalidState)?,
        _ => pre_fields.outstanding_reservations,
    };
    let expected_poststate = PositionAccountV3::new(PositionV3Fields {
        cash_atoms: post_fields.cash_atoms,
        reserved_cash_atoms: post_fields.reserved_cash_atoms,
        native_eggs: post_fields.native_eggs,
        outstanding_reservations: expected_outstanding_reservations,
        ..pre_fields
    })
    .map_err(|_| CodecError::InvalidState)?;
    if position_poststate.semantic != expected_poststate {
        return Err(CodecError::MismatchedBinding);
    }
    let position_poststate_semantic_id = Id32::new(
        position_poststate
            .semantic
            .semantic_id(backend)
            .map_err(|_| CodecError::MismatchedBinding)?
            .bytes(),
    )?;
    let position_prestate_semantic_id = Id32::new(position_prestate.semantic_id)?;
    let unchanged_required = matches!(
        kind,
        GeneralReplayTransitionKindV1::AccountReceiptEnd
            | GeneralReplayTransitionKindV1::FinalizeMergeReceiptPayment
    );
    // Fractional credit transfer/merge can either realize a whole collateral
    // atom into Position cash or leave the Position body unchanged when the
    // destination residue remains below one atom. The Fractional composer owns
    // that exact arithmetic and rederives this structural plan.
    let changed_required = matches!(
        kind,
        GeneralReplayTransitionKindV1::Endow
            | GeneralReplayTransitionKindV1::WithdrawCash
            | GeneralReplayTransitionKindV1::Split
            | GeneralReplayTransitionKindV1::Merge
            | GeneralReplayTransitionKindV1::Materialize
            | GeneralReplayTransitionKindV1::Dematerialize
            | GeneralReplayTransitionKindV1::DirectBuyer
            | GeneralReplayTransitionKindV1::DistributeTradingFee
            | GeneralReplayTransitionKindV1::PortfolioPairBuyer
            | GeneralReplayTransitionKindV1::VirtualSplitBuyer
            | GeneralReplayTransitionKindV1::ReleaseUnfilledReservation
            | GeneralReplayTransitionKindV1::RetirePortfolioPairBuyerArchive
            | GeneralReplayTransitionKindV1::RetirePortfolioPairSellerArchive
            | GeneralReplayTransitionKindV1::StructuredGeneral
            | GeneralReplayTransitionKindV1::FractionalRedeemInternalExact
            | GeneralReplayTransitionKindV1::FractionalRedeemInternalCredit
            | GeneralReplayTransitionKindV1::DealerCollectBuyer
            | GeneralReplayTransitionKindV1::DealerDeliverBuyer
            | GeneralReplayTransitionKindV1::DealerDeliverSeller
            | GeneralReplayTransitionKindV1::DirectMarketAdmitBuyer
            | GeneralReplayTransitionKindV1::DirectMarketAdmitSeller
            | GeneralReplayTransitionKindV1::DirectMarketCancelBuyer
            | GeneralReplayTransitionKindV1::DirectMarketCancelSeller
            | GeneralReplayTransitionKindV1::DirectMarketSettleBuyer
            | GeneralReplayTransitionKindV1::DirectMarketSettleSeller
            | GeneralReplayTransitionKindV1::DirectMarketSettleTreasury
            | GeneralReplayTransitionKindV1::DirectMarketLapseEmptyBuyer
            | GeneralReplayTransitionKindV1::DirectMarketLapseEmptySeller
            | GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedBuyer
            | GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedSeller
            | GeneralReplayTransitionKindV1::DirectMarketLapseSelectedBuyer
            | GeneralReplayTransitionKindV1::DirectMarketLapseSelectedSeller
    );
    if (unchanged_required
        && (position_poststate.semantic != position_prestate.semantic
            || position_poststate_semantic_id != position_prestate_semantic_id))
        || (changed_required
            && (position_poststate.semantic == position_prestate.semantic
                || position_poststate_semantic_id == position_prestate_semantic_id))
    {
        return Err(CodecError::InvalidState);
    }
    let consumed_sequence = prestate.replay_header.next_sequence();
    let (family, version, action, role) = kind.coordinates();
    let delta_id = Id32::new(backend.sha256_parts(&[
        GENERAL_REPLAY_DELTA_DOMAIN_V1,
        &[family],
        &[version],
        &[action],
        &[role],
        &consumed_sequence.to_le_bytes(),
        &transition_id.bytes(),
        &transition_evidence_id.bytes(),
        &position_prestate.account,
        &position_prestate.semantic_id,
        &position_poststate_semantic_id.bytes(),
        &pre_fields.generation.to_le_bytes(),
        &post_fields.generation.to_le_bytes(),
    ]))?;
    let extension = prestate.extension.advanced(
        kind,
        transition_id,
        delta_id,
        position_poststate_semantic_id,
    )?;
    let extension_body = extension.encode()?;
    let replay_header = prestate
        .replay_header
        .advanced_live(post_fields.generation, &extension_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    let replay_envelope = ReplayV3Envelope::from_header(replay_header, &extension_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    let replay_poststate_semantic_id = Id32::new(
        replay_envelope
            .semantic_id(backend)
            .map_err(|_| CodecError::InvalidState)?
            .bytes(),
    )?;
    let mut replay_poststate_body = [0_u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES];
    replay_envelope
        .encode_into(&mut replay_poststate_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    Ok(GeneralReplayTransitionPlanV1 {
        replay_account: prestate.replay_account,
        replay_prestate_semantic_id: prestate.replay_semantic_id,
        replay_poststate_semantic_id,
        replay_poststate_body,
        position_account: Id32::new(position_prestate.account)?,
        position_prestate_semantic_id,
        position_poststate_semantic_id,
        kind,
        transition_id,
        transition_evidence_id,
        delta_id,
        consumed_sequence,
        next_sequence: replay_header.next_sequence(),
    })
}

/// Exact pure postimages for the canonical Market treasury Position cut.
///
/// This structural plan does not authenticate the fee terminal, service
/// ledger, Product Root, account owner, or PDA.  Its SBF caller must derive
/// `terminal_authority_id` from those hostile current semantic owners and
/// commit both postimages before physically retiring the pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralTreasuryPositionTerminalPlanV1 {
    position_prestate_semantic_id: Id32,
    position_terminal_semantic_id: Id32,
    replay_prestate_semantic_id: Id32,
    replay_terminal_semantic_id: Id32,
    terminal_authority_id: Id32,
    transition_id: Id32,
    delta_id: Id32,
    terminal_sequence: u64,
    position_terminal_body: [u8; clutch_retirement::POSITION_V3_BYTES],
    replay_terminal_body: [u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES],
}

impl GeneralTreasuryPositionTerminalPlanV1 {
    pub const fn position_prestate_semantic_id(&self) -> Id32 {
        self.position_prestate_semantic_id
    }
    pub const fn position_terminal_semantic_id(&self) -> Id32 {
        self.position_terminal_semantic_id
    }
    pub const fn replay_prestate_semantic_id(&self) -> Id32 {
        self.replay_prestate_semantic_id
    }
    pub const fn replay_terminal_semantic_id(&self) -> Id32 {
        self.replay_terminal_semantic_id
    }
    pub const fn terminal_authority_id(&self) -> Id32 { self.terminal_authority_id }
    pub const fn transition_id(&self) -> Id32 { self.transition_id }
    pub const fn delta_id(&self) -> Id32 { self.delta_id }
    pub const fn terminal_sequence(&self) -> u64 { self.terminal_sequence }
    pub const fn position_terminal_body(
        &self,
    ) -> &[u8; clutch_retirement::POSITION_V3_BYTES] {
        &self.position_terminal_body
    }
    pub const fn replay_terminal_body(&self) -> &[u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES] {
        &self.replay_terminal_body
    }
}

/// Seal an economically empty current General Position/Replay pair.
///
/// The Position moves `Open -> CloseRequested`; the GEN1 extension commits an
/// action-34 terminal delta and the common Replay envelope moves `Live ->
/// Terminal` at exactly its chain-derived next sequence.  All identities,
/// generation, rent ownership, and immutable Position fields are preserved.
pub fn prepare_general_treasury_position_terminal_v1<B>(
    prestate: GeneralPositionReplayPrestateV1,
    terminal_authority_id: Id32,
    backend: &B,
) -> Result<GeneralTreasuryPositionTerminalPlanV1, CodecError>
where
    B: PositionV3Sha256Backend + ReplayV3HashBackend,
{
    if terminal_authority_id.is_zero() {
        return Err(CodecError::ZeroIdentity);
    }
    let before = prestate.position;
    before
        .validate_writable()
        .map_err(|_| CodecError::MismatchedBinding)?;
    let fields = before.semantic.fields();
    if fields.cash_atoms != 0
        || fields.reserved_cash_atoms != 0
        || fields.native_eggs != [0; MAX_OUTCOMES]
        || fields.outstanding_reservations != 0
        || prestate.replay_header.lifecycle() != ReplayV3Lifecycle::Live
    {
        return Err(CodecError::InvalidState);
    }
    let terminal_position = PositionAccountV3::new(PositionV3Fields {
        lifecycle: PositionLifecycleV3::CloseRequested,
        ..fields
    })
    .map_err(|_| CodecError::InvalidState)?;
    let position_prestate_semantic_id = Id32::new(before.semantic_id)?;
    let position_terminal_semantic_id = Id32::new(
        terminal_position
            .semantic_id(backend)
            .map_err(|_| CodecError::InvalidState)?
            .bytes(),
    )?;
    if position_prestate_semantic_id == position_terminal_semantic_id {
        return Err(CodecError::InvalidState);
    }
    let consumed_sequence = prestate.replay_header.next_sequence();
    let kind = GeneralReplayTransitionKindV1::CloseTreasuryPosition;
    let (family, version, action, role) = kind.coordinates();
    let transition_id = Id32::new(backend.sha256_parts(&[
        GENERAL_TREASURY_POSITION_TERMINAL_DOMAIN_V1,
        &terminal_authority_id.bytes(),
        &before.account,
        &prestate.replay_account.bytes(),
        &position_prestate_semantic_id.bytes(),
        &position_terminal_semantic_id.bytes(),
        &prestate.replay_semantic_id.bytes(),
        &consumed_sequence.to_le_bytes(),
        &fields.generation.to_le_bytes(),
    ]))?;
    let delta_id = Id32::new(backend.sha256_parts(&[
        GENERAL_REPLAY_DELTA_DOMAIN_V1,
        &[family],
        &[version],
        &[action],
        &[role],
        &consumed_sequence.to_le_bytes(),
        &transition_id.bytes(),
        &terminal_authority_id.bytes(),
        &before.account,
        &position_prestate_semantic_id.bytes(),
        &position_terminal_semantic_id.bytes(),
        &fields.generation.to_le_bytes(),
        &fields.generation.to_le_bytes(),
    ]))?;
    let terminal_extension = prestate.extension.advanced(
        kind,
        transition_id,
        delta_id,
        position_terminal_semantic_id,
    )?;
    let terminal_extension_body = terminal_extension.encode()?;
    let terminal_header = prestate
        .replay_header
        .terminalized(fields.generation, &terminal_extension_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    let terminal_envelope = ReplayV3Envelope::from_header(
        terminal_header,
        &terminal_extension_body,
        backend,
    )
    .map_err(|_| CodecError::InvalidState)?;
    let replay_terminal_semantic_id = Id32::new(
        terminal_envelope
            .semantic_id(backend)
            .map_err(|_| CodecError::InvalidState)?
            .bytes(),
    )?;
    let mut position_terminal_body = [0u8; clutch_retirement::POSITION_V3_BYTES];
    position_terminal_body.copy_from_slice(
        &terminal_position
            .encode()
            .map_err(|_| CodecError::InvalidState)?,
    );
    let mut replay_terminal_body = [0u8; GENERAL_REPLAY_ACCOUNT_V1_BYTES];
    terminal_envelope
        .encode_into(&mut replay_terminal_body, backend)
        .map_err(|_| CodecError::InvalidState)?;
    Ok(GeneralTreasuryPositionTerminalPlanV1 {
        position_prestate_semantic_id,
        position_terminal_semantic_id,
        replay_prestate_semantic_id: prestate.replay_semantic_id,
        replay_terminal_semantic_id,
        terminal_authority_id,
        transition_id,
        delta_id,
        terminal_sequence: terminal_header.next_sequence(),
        position_terminal_body,
        replay_terminal_body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advanced_extension(action: u8, role: u8) -> [u8; GENERAL_REPLAY_EXTENSION_V1_BYTES] {
        let mut body = [0u8; GENERAL_REPLAY_EXTENSION_V1_BYTES];
        body[..32].copy_from_slice(&[1; 32]);
        body[32..64].copy_from_slice(&[2; 32]);
        body[64..96].copy_from_slice(&[3; 32]);
        body[96..128].copy_from_slice(&[4; 32]);
        body[128] = action;
        body[129] = 1;
        body[130] = SETTLEMENT_FAMILY;
        body[131] = TRANSITION_VERSION_V1;
        body[132] = role;
        body
    }

    fn direct_market_extension(
        action: u8,
        role: u8,
    ) -> [u8; GENERAL_REPLAY_EXTENSION_V1_BYTES] {
        let mut body = advanced_extension(action, role);
        body[130] = DIRECT_MARKET_FAMILY;
        body
    }

    #[test]
    fn action40_tuple_is_exact_and_wrong_role_is_refused() {
        let value = GeneralReplayExtensionV1::decode(&advanced_extension(
            40,
            MERGE_PAYMENT_OWNER_ROLE,
        ))
        .unwrap();
        assert_eq!(
            value.last_kind(),
            Some(GeneralReplayTransitionKindV1::FinalizeMergeReceiptPayment)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&advanced_extension(40, OWNER_CASH_ROLE)),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn action50_fee_distribution_tuple_is_exact_and_role_disjoint() {
        let value = GeneralReplayExtensionV1::decode(&advanced_extension(
            50,
            FEE_DISTRIBUTION_RECIPIENT_ROLE,
        ))
        .unwrap();
        assert_eq!(
            value.last_kind(),
            Some(GeneralReplayTransitionKindV1::DistributeTradingFee)
        );
        assert_eq!(value.last_kind().unwrap().action(), 50);
        assert_eq!(
            value.last_kind().unwrap().role(),
            FEE_DISTRIBUTION_RECIPIENT_ROLE
        );
        for wrong_role in [
            OWNER_ACCOUNTING_ROLE,
            OWNER_CASH_ROLE,
            PORTFOLIO_ARCHIVE_SELLER_ROLE,
        ] {
            assert_eq!(
                GeneralReplayExtensionV1::decode(&advanced_extension(50, wrong_role)),
                Err(CodecError::InvalidState)
            );
        }
        assert_eq!(
            GeneralReplayExtensionV1::decode(&advanced_extension(
                38,
                FEE_DISTRIBUTION_RECIPIENT_ROLE,
            )),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn action42_roles_are_fresh_exhaustive_and_not_interchangeable() {
        let buyer = GeneralReplayExtensionV1::decode(&advanced_extension(
            42,
            PORTFOLIO_PAIR_BUYER_ROLE,
        ))
        .unwrap();
        let seller = GeneralReplayExtensionV1::decode(&advanced_extension(
            42,
            PORTFOLIO_PAIR_SELLER_ROLE,
        ))
        .unwrap();
        assert_eq!(
            buyer.last_kind(),
            Some(GeneralReplayTransitionKindV1::PortfolioPairBuyer)
        );
        assert_eq!(
            seller.last_kind(),
            Some(GeneralReplayTransitionKindV1::PortfolioPairSeller)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&advanced_extension(42, DIRECT_BUYER_ROLE)),
            Err(CodecError::InvalidState)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&advanced_extension(
                26,
                PORTFOLIO_PAIR_BUYER_ROLE,
            )),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn action44_roles_are_fresh_exhaustive_and_not_action42_roles() {
        let buyer = GeneralReplayExtensionV1::decode(&advanced_extension(
            44,
            PORTFOLIO_ARCHIVE_BUYER_ROLE,
        ))
        .unwrap();
        let seller = GeneralReplayExtensionV1::decode(&advanced_extension(
            44,
            PORTFOLIO_ARCHIVE_SELLER_ROLE,
        ))
        .unwrap();
        assert_eq!(
            buyer.last_kind(),
            Some(GeneralReplayTransitionKindV1::RetirePortfolioPairBuyerArchive)
        );
        assert_eq!(
            seller.last_kind(),
            Some(GeneralReplayTransitionKindV1::RetirePortfolioPairSellerArchive)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&advanced_extension(
                44,
                PORTFOLIO_PAIR_BUYER_ROLE,
            )),
            Err(CodecError::InvalidState)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&advanced_extension(
                42,
                PORTFOLIO_ARCHIVE_SELLER_ROLE,
            )),
            Err(CodecError::InvalidState)
        );
    }

    #[test]
    fn dealer_collect_deliver_tuples_are_exact_and_role_disjoint() {
        let mut collect_buyer = advanced_extension(15, DEALER_BUYER_ROLE);
        collect_buyer[130] = DEALER_FAMILY;
        let mut collect_seller = advanced_extension(15, DEALER_SELLER_ROLE);
        collect_seller[130] = DEALER_FAMILY;
        let mut deliver_buyer = advanced_extension(16, DEALER_BUYER_ROLE);
        deliver_buyer[130] = DEALER_FAMILY;
        let mut deliver_seller = advanced_extension(16, DEALER_SELLER_ROLE);
        deliver_seller[130] = DEALER_FAMILY;
        assert_eq!(
            GeneralReplayExtensionV1::decode(&collect_buyer)
                .unwrap()
                .last_kind(),
            Some(GeneralReplayTransitionKindV1::DealerCollectBuyer)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&collect_seller)
                .unwrap()
                .last_kind(),
            Some(GeneralReplayTransitionKindV1::DealerCollectSeller)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&deliver_buyer)
                .unwrap()
                .last_kind(),
            Some(GeneralReplayTransitionKindV1::DealerDeliverBuyer)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&deliver_seller)
                .unwrap()
                .last_kind(),
            Some(GeneralReplayTransitionKindV1::DealerDeliverSeller)
        );
        collect_buyer[130] = SETTLEMENT_FAMILY;
        assert_eq!(
            GeneralReplayExtensionV1::decode(&collect_buyer),
            Err(CodecError::InvalidState)
        );
        deliver_seller[132] = DEALER_BUYER_ROLE;
        assert_eq!(
            GeneralReplayExtensionV1::decode(&deliver_seller)
                .unwrap()
                .last_kind(),
            Some(GeneralReplayTransitionKindV1::DealerDeliverBuyer)
        );
    }

    #[test]
    fn direct_market_roles_exhaustively_bind_fresh_family_actions() {
        let cases = [
            (2, DIRECT_MARKET_BUYER_ROLE, GeneralReplayTransitionKindV1::DirectMarketAdmitBuyer),
            (2, DIRECT_MARKET_SELLER_ROLE, GeneralReplayTransitionKindV1::DirectMarketAdmitSeller),
            (3, DIRECT_MARKET_BUYER_ROLE, GeneralReplayTransitionKindV1::DirectMarketCancelBuyer),
            (3, DIRECT_MARKET_SELLER_ROLE, GeneralReplayTransitionKindV1::DirectMarketCancelSeller),
            (9, DIRECT_MARKET_BUYER_ROLE, GeneralReplayTransitionKindV1::DirectMarketSettleBuyer),
            (9, DIRECT_MARKET_SELLER_ROLE, GeneralReplayTransitionKindV1::DirectMarketSettleSeller),
            (9, DIRECT_MARKET_TREASURY_ROLE, GeneralReplayTransitionKindV1::DirectMarketSettleTreasury),
            (10, DIRECT_MARKET_BUYER_ROLE, GeneralReplayTransitionKindV1::DirectMarketLapseEmptyBuyer),
            (10, DIRECT_MARKET_SELLER_ROLE, GeneralReplayTransitionKindV1::DirectMarketLapseEmptySeller),
            (11, DIRECT_MARKET_BUYER_ROLE, GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedBuyer),
            (11, DIRECT_MARKET_SELLER_ROLE, GeneralReplayTransitionKindV1::DirectMarketLapseUnselectedSeller),
            (12, DIRECT_MARKET_BUYER_ROLE, GeneralReplayTransitionKindV1::DirectMarketLapseSelectedBuyer),
            (12, DIRECT_MARKET_SELLER_ROLE, GeneralReplayTransitionKindV1::DirectMarketLapseSelectedSeller),
        ];
        for (action, role, expected) in cases {
            assert_eq!(
                GeneralReplayExtensionV1::decode(&direct_market_extension(action, role))
                    .unwrap()
                    .last_kind(),
                Some(expected)
            );
        }

        assert_eq!(
            GeneralReplayExtensionV1::decode(&direct_market_extension(2, 0)),
            Err(CodecError::InvalidState)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&direct_market_extension(
                4,
                DIRECT_MARKET_BUYER_ROLE,
            )),
            Err(CodecError::InvalidState)
        );
        assert_eq!(
            GeneralReplayExtensionV1::decode(&advanced_extension(
                2,
                DIRECT_MARKET_BUYER_ROLE,
            )),
            Err(CodecError::InvalidState)
        );
    }
}
