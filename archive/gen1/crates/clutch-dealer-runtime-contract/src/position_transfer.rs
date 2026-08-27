// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact Dealer transfers over the canonical global Position V3 body.
//!
//! Sponsor and LP funding, settlement collection/delivery, Begin, Finalize,
//! Abort, and sponsor refunds are internal liability-ledger movements after a
//! user has already entered the Realm Hoard. They never perform token CPI,
//! change the collateral mint supply, or mutate Hoard custody. The separate
//! Realm-selected collateral adapter remains the only holder↔Hoard boundary.

use clutch_retirement::{
    DealerPositionProjectionV3, GeneralPositionProjectionV3, PositionAccountV3,
    PositionLifecycleV3, PositionV3Fields, PositionV3Sha256Backend,
};
use sha2::{Digest, Sha256};

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::replay::{action_byte, decode_action};
use crate::{
    validate_padding_u64, CoveredDealerSelectionV1, DealerRuntimeActionV1, Error, FixedCodec, Id,
    Result, MAX_OUTCOMES,
};

/// Local magic for an exact single-transfer bundle.
pub const DEALER_ASSET_TRANSFER_BUNDLE_MAGIC_V1: [u8; 8] = *b"DCDATB01";
/// Exact local transfer-bundle version.
pub const DEALER_ASSET_TRANSFER_BUNDLE_VERSION_V1: u16 = 1;
/// Exact bytes in a single-transfer bundle.
pub const DEALER_ASSET_TRANSFER_BUNDLE_BYTES_V1: usize =
    HEADER_BYTES + (10 * 32) + (3 * 8) + (MAX_OUTCOMES * 8) + 8;
/// Content domain for an exact transfer bundle.
pub const DEALER_ASSET_TRANSFER_BUNDLE_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-asset-transfer-bundle/v1\0";

/// Local magic for an action's canonical empty transfer bundle.
pub const DEALER_EMPTY_ASSET_TRANSFER_MAGIC_V1: [u8; 8] = *b"DCDAEMPTY";
/// Exact local empty-bundle version.
pub const DEALER_EMPTY_ASSET_TRANSFER_VERSION_V1: u16 = 1;
/// Exact bytes in an action-bound empty transfer bundle.
pub const DEALER_EMPTY_ASSET_TRANSFER_BYTES_V1: usize = HEADER_BYTES + 8;
/// Content domain for a canonical empty transfer bundle.
pub const DEALER_EMPTY_ASSET_TRANSFER_CONTENT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/dealer-empty-asset-transfer/v1\0";

const _: () = assert!(DEALER_ASSET_TRANSFER_BUNDLE_BYTES_V1 == 492);
const _: () = assert!(DEALER_EMPTY_ASSET_TRANSFER_BYTES_V1 == 20);
const _: () = assert!(DEALER_ASSET_TRANSFER_BUNDLE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);

/// Semantic owner of one endpoint in an internal Dealer transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerAssetEndpointKindV1 {
    /// Ordinary user, LP, sponsor, maker, or treasury Position V3.
    GeneralPosition = 1,
    /// Dealer facility Position V3.
    FacilityPosition = 2,
    /// Transient SettlementPotV2 custody derived from progress facts.
    SettlementPot = 3,
}

impl DealerAssetEndpointKindV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::GeneralPosition),
            2 => Ok(Self::FacilityPosition),
            3 => Ok(Self::SettlementPot),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Exact Realm-selected collateral identity shared by both endpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPositionMarketJoinV1 {
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Immutable Realm identity.
    pub realm_id: Id,
    /// Immutable Realm collateral-policy content identity.
    pub collateral_policy_id: Id,
    /// Exact compiled collateral-adapter release identity.
    pub collateral_release_id: Id,
    /// Exact authenticated Market outcome width.
    pub outcome_count: u8,
}

impl DealerPositionMarketJoinV1 {
    fn validate(self) -> Result<()> {
        for identity in [
            self.market_instance_v2_id,
            self.realm_id,
            self.collateral_policy_id,
            self.collateral_release_id,
        ] {
            identity.validate_live()?;
        }
        if self.outcome_count < 2 || usize::from(self.outcome_count) > MAX_OUTCOMES {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Exact single movement of total cash and native Eggs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAssetTransferAmountsV1 {
    /// Total cash atoms moved from source to destination.
    pub cash_atoms: u64,
    /// Source reserved-cash subset consumed by the movement.
    pub source_reserved_cash_atoms: u64,
    /// Destination reserved-cash subset created by the movement.
    pub destination_reserved_cash_atoms: u64,
    /// Exact native-Egg vector moved from source to destination.
    pub native_eggs: [u64; MAX_OUTCOMES],
}

impl DealerAssetTransferAmountsV1 {
    fn is_zero(self) -> bool {
        self.cash_atoms == 0
            && self.source_reserved_cash_atoms == 0
            && self.destination_reserved_cash_atoms == 0
            && self.native_eggs == [0; MAX_OUTCOMES]
    }

    fn validate(self, outcome_count: u8) -> Result<()> {
        validate_padding_u64(outcome_count, &self.native_eggs)?;
        if self.source_reserved_cash_atoms > self.cash_atoms
            || self.destination_reserved_cash_atoms > self.cash_atoms
            || (self.cash_atoms == 0 && self.native_eggs == [0; MAX_OUTCOMES])
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Canonical content-addressed bundle bound by one Dealer Replay intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAssetTransferBundleV1 {
    /// Exact Dealer action whose ownership direction this transfer implements.
    pub action: DealerRuntimeActionV1,
    /// Source semantic-owner kind.
    pub source_kind: DealerAssetEndpointKindV1,
    /// Destination semantic-owner kind.
    pub destination_kind: DealerAssetEndpointKindV1,
    /// Exact common Market/Realm collateral join.
    pub market: DealerPositionMarketJoinV1,
    /// Exact source runtime account.
    pub source_account_id: Id,
    /// Exact destination runtime account.
    pub destination_account_id: Id,
    /// Source semantic identity before mutation.
    pub source_pre_semantic_id: Id,
    /// Source semantic identity after mutation.
    pub source_post_semantic_id: Id,
    /// Destination semantic identity before mutation.
    pub destination_pre_semantic_id: Id,
    /// Destination semantic identity after mutation.
    pub destination_post_semantic_id: Id,
    /// Exact moved assets and reservation-state deltas.
    pub amounts: DealerAssetTransferAmountsV1,
}

impl DealerAssetTransferBundleV1 {
    /// Validate identities, direction, outcome padding, and reservation rules.
    pub fn validate(&self) -> Result<()> {
        self.market.validate()?;
        for identity in [
            self.source_account_id,
            self.destination_account_id,
            self.source_pre_semantic_id,
            self.source_post_semantic_id,
            self.destination_pre_semantic_id,
            self.destination_post_semantic_id,
        ] {
            identity.validate_live()?;
        }
        let zero_claim = self.action == DealerRuntimeActionV1::Claim && self.amounts.is_zero();
        let zero_begin =
            self.action == DealerRuntimeActionV1::SelectLeaseAndBegin && self.amounts.is_zero();
        if self.source_account_id == self.destination_account_id {
            return Err(Error::MismatchedBinding);
        }
        if zero_claim || zero_begin {
            validate_padding_u64(self.market.outcome_count, &self.amounts.native_eggs)?;
            if self.source_pre_semantic_id != self.source_post_semantic_id
                || (zero_claim
                    && self.destination_pre_semantic_id != self.destination_post_semantic_id)
                || (zero_begin
                    && self.destination_pre_semantic_id == self.destination_post_semantic_id)
            {
                return Err(Error::ConservationFailure);
            }
        } else {
            if self.source_pre_semantic_id == self.source_post_semantic_id
                || self.destination_pre_semantic_id == self.destination_post_semantic_id
            {
                return Err(Error::MismatchedBinding);
            }
            self.amounts.validate(self.market.outcome_count)?;
        }
        require_transfer_direction(self.action, self.source_kind, self.destination_kind)?;
        match (self.source_kind, self.destination_kind) {
            (
                DealerAssetEndpointKindV1::GeneralPosition,
                DealerAssetEndpointKindV1::FacilityPosition,
            )
            | (
                DealerAssetEndpointKindV1::FacilityPosition,
                DealerAssetEndpointKindV1::GeneralPosition,
            ) => {
                if self.amounts.source_reserved_cash_atoms != 0
                    || self.amounts.destination_reserved_cash_atoms != 0
                {
                    return Err(Error::ConservationFailure);
                }
            }
            (DealerAssetEndpointKindV1::SettlementPot, _) => {
                if self.amounts.source_reserved_cash_atoms != 0
                    || self.amounts.destination_reserved_cash_atoms != 0
                {
                    return Err(Error::ConservationFailure);
                }
            }
            (_, DealerAssetEndpointKindV1::SettlementPot) => {
                if self.amounts.destination_reserved_cash_atoms != 0 {
                    return Err(Error::ConservationFailure);
                }
            }
            _ => return Err(Error::MismatchedBinding),
        }
        Ok(())
    }

    /// Canonical transfer-bundle identity stored in the Replay intent.
    pub fn bundle_id(&self) -> Result<Id> {
        let id = self.content_id(DEALER_ASSET_TRANSFER_BUNDLE_CONTENT_DOMAIN_V1)?;
        id.validate_live()?;
        Ok(id)
    }
}

impl FixedCodec for DealerAssetTransferBundleV1 {
    const ENCODED_LEN: usize = DEALER_ASSET_TRANSFER_BUNDLE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_ASSET_TRANSFER_BUNDLE_MAGIC_V1,
            DEALER_ASSET_TRANSFER_BUNDLE_VERSION_V1,
        );
        for identity in [
            self.market.market_instance_v2_id,
            self.market.realm_id,
            self.market.collateral_policy_id,
            self.market.collateral_release_id,
            self.source_account_id,
            self.destination_account_id,
            self.source_pre_semantic_id,
            self.source_post_semantic_id,
            self.destination_pre_semantic_id,
            self.destination_post_semantic_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.amounts.cash_atoms);
        writer.u64(self.amounts.source_reserved_cash_atoms);
        writer.u64(self.amounts.destination_reserved_cash_atoms);
        for amount in self.amounts.native_eggs {
            writer.u64(amount);
        }
        writer.u8(action_byte(self.action));
        writer.u8(endpoint_kind_byte(self.source_kind));
        writer.u8(endpoint_kind_byte(self.destination_kind));
        writer.u8(self.market.outcome_count);
        writer.reserved(4);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_ASSET_TRANSFER_BUNDLE_MAGIC_V1,
            DEALER_ASSET_TRANSFER_BUNDLE_VERSION_V1,
        )?;
        let market_instance_v2_id = reader.id();
        let realm_id = reader.id();
        let collateral_policy_id = reader.id();
        let collateral_release_id = reader.id();
        let source_account_id = reader.id();
        let destination_account_id = reader.id();
        let source_pre_semantic_id = reader.id();
        let source_post_semantic_id = reader.id();
        let destination_pre_semantic_id = reader.id();
        let destination_post_semantic_id = reader.id();
        let cash_atoms = reader.u64();
        let source_reserved_cash_atoms = reader.u64();
        let destination_reserved_cash_atoms = reader.u64();
        let mut native_eggs = [0u64; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            native_eggs[index] = reader.u64();
            index += 1;
        }
        let action = decode_action(reader.u8())?;
        let source_kind = DealerAssetEndpointKindV1::decode(reader.u8())?;
        let destination_kind = DealerAssetEndpointKindV1::decode(reader.u8())?;
        let outcome_count = reader.u8();
        reader.reserved(4)?;
        reader.finish()?;
        let value = Self {
            action,
            source_kind,
            destination_kind,
            market: DealerPositionMarketJoinV1 {
                market_instance_v2_id,
                realm_id,
                collateral_policy_id,
                collateral_release_id,
                outcome_count,
            },
            source_account_id,
            destination_account_id,
            source_pre_semantic_id,
            source_post_semantic_id,
            destination_pre_semantic_id,
            destination_post_semantic_id,
            amounts: DealerAssetTransferAmountsV1 {
                cash_atoms,
                source_reserved_cash_atoms,
                destination_reserved_cash_atoms,
                native_eggs,
            },
        };
        value.validate()?;
        Ok(value)
    }
}

/// Canonical no-asset bundle for one replayed lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEmptyAssetTransferBundleV1 {
    /// Action that proves no Position/Pot asset movement occurred.
    pub action: DealerRuntimeActionV1,
}

impl DealerEmptyAssetTransferBundleV1 {
    /// Canonical nonzero identity used by Replay rather than a zero sentinel.
    pub fn bundle_id(&self) -> Result<Id> {
        if action_has_asset_transfer(self.action) {
            return Err(Error::MismatchedBinding);
        }
        let id = self.content_id(DEALER_EMPTY_ASSET_TRANSFER_CONTENT_DOMAIN_V1)?;
        id.validate_live()?;
        Ok(id)
    }
}

impl FixedCodec for DealerEmptyAssetTransferBundleV1 {
    const ENCODED_LEN: usize = DEALER_EMPTY_ASSET_TRANSFER_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        if action_has_asset_transfer(self.action) {
            return Err(Error::MismatchedBinding);
        }
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_EMPTY_ASSET_TRANSFER_MAGIC_V1,
            DEALER_EMPTY_ASSET_TRANSFER_VERSION_V1,
        );
        writer.u8(action_byte(self.action));
        writer.reserved(7);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_EMPTY_ASSET_TRANSFER_MAGIC_V1,
            DEALER_EMPTY_ASSET_TRANSFER_VERSION_V1,
        )?;
        let value = Self {
            action: decode_action(reader.u8())?,
        };
        reader.reserved(7)?;
        reader.finish()?;
        if action_has_asset_transfer(value.action) {
            return Err(Error::MismatchedBinding);
        }
        Ok(value)
    }
}

/// Typed canonical Position projection admitted to Dealer transfers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerTransferPositionV3 {
    /// Ordinary user, LP, sponsor, maker, or treasury Position.
    General {
        /// Exact runtime Position account.
        account_id: Id,
        /// Purpose- and Market-joined canonical body.
        position: GeneralPositionProjectionV3,
    },
    /// Dealer facility Position.
    Facility {
        /// Exact runtime Position account.
        account_id: Id,
        /// Purpose- and State-binding-joined canonical body.
        position: DealerPositionProjectionV3,
    },
}

impl DealerTransferPositionV3 {
    fn validate(self, market: DealerPositionMarketJoinV1) -> Result<()> {
        market.validate()?;
        self.account_id().validate_live()?;
        let position = self.position();
        position.validate().map_err(|_| Error::MismatchedBinding)?;
        if position.lifecycle() != PositionLifecycleV3::Open
            || position.outcome_count() != market.outcome_count
            || !identity_matches(position.market_instance_id(), market.market_instance_v2_id)
            || !identity_matches(position.realm_id(), market.realm_id)
            || !identity_matches(position.collateral_policy_id(), market.collateral_policy_id)
            || !identity_matches(
                position.collateral_release_id(),
                market.collateral_release_id,
            )
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Exact Position runtime account.
    pub const fn account_id(self) -> Id {
        match self {
            Self::General { account_id, .. } | Self::Facility { account_id, .. } => account_id,
        }
    }

    /// Canonical global Position V3 body.
    pub const fn position(self) -> PositionAccountV3 {
        match self {
            Self::General { position, .. } => position.position(),
            Self::Facility { position, .. } => position.position(),
        }
    }

    /// Endpoint kind implied by the typed purpose projection.
    pub const fn kind(self) -> DealerAssetEndpointKindV1 {
        match self {
            Self::General { .. } => DealerAssetEndpointKindV1::GeneralPosition,
            Self::Facility { .. } => DealerAssetEndpointKindV1::FacilityPosition,
        }
    }

    fn semantic_id(self) -> Result<Id> {
        position_semantic_id(self.position())
    }
}

/// Exact Pot custody pre/post projection derived from SettlementPotV2.
///
/// This carrier is deliberately not persisted. The Pot codec remains the sole
/// semantic owner of progress facts from which both custody snapshots derive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPotCustodyTransitionV1 {
    /// Exact SettlementPotV2 runtime account.
    pub pot_account_id: Id,
    /// Pot semantic identity before its progress mutation.
    pub pot_pre_semantic_id: Id,
    /// Pot semantic identity after its progress mutation.
    pub pot_post_semantic_id: Id,
    /// Exact Pot cash custody before mutation.
    pub cash_pre_atoms: u64,
    /// Exact Pot cash custody after mutation.
    pub cash_post_atoms: u64,
    /// Exact Pot native-Egg custody before mutation.
    pub eggs_pre: [u64; MAX_OUTCOMES],
    /// Exact Pot native-Egg custody after mutation.
    pub eggs_post: [u64; MAX_OUTCOMES],
    /// Exact Pot outcome width.
    pub outcome_count: u8,
}

impl DealerPotCustodyTransitionV1 {
    fn validate(self, market: DealerPositionMarketJoinV1) -> Result<()> {
        for identity in [
            self.pot_account_id,
            self.pot_pre_semantic_id,
            self.pot_post_semantic_id,
        ] {
            identity.validate_live()?;
        }
        if self.pot_pre_semantic_id == self.pot_post_semantic_id
            || self.outcome_count != market.outcome_count
        {
            return Err(Error::MismatchedBinding);
        }
        validate_padding_u64(self.outcome_count, &self.eggs_pre)?;
        validate_padding_u64(self.outcome_count, &self.eggs_post)
    }
}

/// Exact result prepared before atomic Position↔Position mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerPositionPairTransferV1 {
    bundle: DealerAssetTransferBundleV1,
    source_post: PositionAccountV3,
    destination_post: PositionAccountV3,
}

impl PreparedDealerPositionPairTransferV1 {
    /// Canonical bundle whose identity enters Replay.
    pub const fn bundle(self) -> DealerAssetTransferBundleV1 {
        self.bundle
    }

    /// Exact source Position V3 post-state.
    pub const fn source_post(self) -> PositionAccountV3 {
        self.source_post
    }

    /// Exact destination Position V3 post-state.
    pub const fn destination_post(self) -> PositionAccountV3 {
        self.destination_post
    }
}

/// Prepare sponsor initialization, LP contribution, pre-activation withdrawal,
/// or sponsor refund.
pub(crate) fn prepare_dealer_position_pair_transfer_v1(
    action: DealerRuntimeActionV1,
    market: DealerPositionMarketJoinV1,
    source: DealerTransferPositionV3,
    destination: DealerTransferPositionV3,
    amounts: DealerAssetTransferAmountsV1,
) -> Result<PreparedDealerPositionPairTransferV1> {
    market.validate()?;
    source.validate(market)?;
    destination.validate(market)?;
    let zero_claim = action == DealerRuntimeActionV1::Claim && amounts.is_zero();
    if zero_claim {
        validate_padding_u64(market.outcome_count, &amounts.native_eggs)?;
    } else {
        amounts.validate(market.outcome_count)?;
    }
    require_transfer_direction(action, source.kind(), destination.kind())?;
    if source.account_id() == destination.account_id()
        || amounts.source_reserved_cash_atoms != 0
        || amounts.destination_reserved_cash_atoms != 0
    {
        return Err(Error::MismatchedBinding);
    }
    let source_pre_id = source.semantic_id()?;
    let destination_pre_id = destination.semantic_id()?;
    let source_post = if zero_claim {
        source.position()
    } else {
        apply_position_debit(source.position(), amounts)?
    };
    let destination_post = if zero_claim {
        destination.position()
    } else {
        apply_position_credit(destination.position(), amounts)?
    };
    let source_post_id = position_semantic_id(source_post)?;
    let destination_post_id = position_semantic_id(destination_post)?;
    let bundle = DealerAssetTransferBundleV1 {
        action,
        source_kind: source.kind(),
        destination_kind: destination.kind(),
        market,
        source_account_id: source.account_id(),
        destination_account_id: destination.account_id(),
        source_pre_semantic_id: source_pre_id,
        source_post_semantic_id: source_post_id,
        destination_pre_semantic_id: destination_pre_id,
        destination_post_semantic_id: destination_post_id,
        amounts,
    };
    bundle.validate()?;
    Ok(PreparedDealerPositionPairTransferV1 {
        bundle,
        source_post,
        destination_post,
    })
}

/// Prepare exact sponsor cash funding from the authenticated sponsor Position.
///
/// `sponsor_capital_atoms` must be projected from the authoritative Dealer
/// State initialization transition. Sponsor funding never transfers Eggs or
/// reserved cash and never crosses the Realm Hoard token boundary.
pub fn prepare_dealer_sponsor_funding_transfer_v1(
    market: DealerPositionMarketJoinV1,
    sponsor_owner: Id,
    sponsor_capital_atoms: u64,
    sponsor_position: DealerTransferPositionV3,
    facility_position: DealerTransferPositionV3,
) -> Result<PreparedDealerPositionPairTransferV1> {
    require_general_owner(sponsor_position, sponsor_owner)?;
    let amounts = sponsor_capital_amounts(sponsor_capital_atoms)?;
    prepare_dealer_position_pair_transfer_v1(
        DealerRuntimeActionV1::Initialize,
        market,
        sponsor_position,
        facility_position,
        amounts,
    )
}

/// Prepare one exact LP contribution or pre-activation withdrawal.
///
/// Every moved amount is derived from the immutable Dealer capital unit and
/// the exact share delta. The caller cannot supply an independent cash/Egg
/// vector or silently round a capital unit.
#[allow(clippy::too_many_arguments)]
pub fn prepare_dealer_lp_share_transfer_v1(
    action: DealerRuntimeActionV1,
    policy: &crate::DealerPolicyV1,
    market: DealerPositionMarketJoinV1,
    lp_owner: Id,
    share_delta: u64,
    lp_position: DealerTransferPositionV3,
    facility_position: DealerTransferPositionV3,
) -> Result<PreparedDealerPositionPairTransferV1> {
    policy.validate()?;
    require_general_owner(lp_position, lp_owner)?;
    if market.market_instance_v2_id != policy.market_instance_v2_id
        || market.outcome_count != policy.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let amounts = lp_share_amounts(policy, share_delta)?;
    match action {
        DealerRuntimeActionV1::Contribute => prepare_dealer_position_pair_transfer_v1(
            action,
            market,
            lp_position,
            facility_position,
            amounts,
        ),
        DealerRuntimeActionV1::WithdrawFunding => prepare_dealer_position_pair_transfer_v1(
            action,
            market,
            facility_position,
            lp_position,
            amounts,
        ),
        _ => Err(Error::MismatchedBinding),
    }
}

/// Prepare the exact pre-activation sponsor-principal refund.
///
/// The State transition supplies both the immutable refund owner and the
/// original sponsor principal; donation, fee, rent, and liveness balances are
/// not admitted to this internal cash movement.
pub fn prepare_dealer_sponsor_refund_transfer_v1(
    market: DealerPositionMarketJoinV1,
    sponsor_refund_owner: Id,
    sponsor_capital_atoms: u64,
    facility_position: DealerTransferPositionV3,
    refund_position: DealerTransferPositionV3,
) -> Result<PreparedDealerPositionPairTransferV1> {
    require_general_owner(refund_position, sponsor_refund_owner)?;
    let amounts = sponsor_capital_amounts(sponsor_capital_atoms)?;
    prepare_dealer_position_pair_transfer_v1(
        DealerRuntimeActionV1::RefundCancelledSponsor,
        market,
        facility_position,
        refund_position,
        amounts,
    )
}

/// Exact result prepared before atomic Position↔Pot mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerPositionPotTransferV1 {
    bundle: DealerAssetTransferBundleV1,
    position_post: PositionAccountV3,
}

/// Deterministic facility Position identities surrounding one complete
/// CoveredDealer Lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerCoveredPositionProjectionV1 {
    leased: PositionAccountV3,
    terminal: PositionAccountV3,
}

impl DealerCoveredPositionProjectionV1 {
    /// Position postimage after Begin deposits dealer cash/Eggs into the Pot.
    pub const fn leased(&self) -> PositionAccountV3 {
        self.leased
    }

    /// Expected postimage after complete Finalize returns dealer proceeds and
    /// rotates the facility generation exactly once.
    pub const fn terminal(&self) -> PositionAccountV3 {
        self.terminal
    }
}

/// Derive both Position postimages solely from an authenticated persisted
/// CoveredDealer selection. No caller-shaped Pot semantic ID or amount enters
/// this constructor; the later atomic Begin transfer rederives and compares
/// the same leased postimage against the actual Pot.
pub fn project_covered_dealer_position_v1(
    selection: &CoveredDealerSelectionV1,
    market: DealerPositionMarketJoinV1,
    facility_position: DealerTransferPositionV3,
) -> Result<DealerCoveredPositionProjectionV1> {
    selection.validate()?;
    market.validate()?;
    facility_position.validate(market)?;
    if facility_position.kind() != DealerAssetEndpointKindV1::FacilityPosition
        || market.market_instance_v2_id != selection.market_instance_v2_id
        || market.outcome_count != selection.outcome_count
    {
        return Err(Error::MismatchedBinding);
    }
    let begin = DealerAssetTransferAmountsV1 {
        cash_atoms: selection.receipt.dealer_net_cash_out_atoms,
        source_reserved_cash_atoms: 0,
        destination_reserved_cash_atoms: 0,
        native_eggs: selection.trade.sell_to_users,
    };
    let leased = if begin.is_zero() {
        facility_position.position()
    } else {
        begin.validate(selection.outcome_count)?;
        apply_position_debit(facility_position.position(), begin)?
    };
    let finalize = DealerAssetTransferAmountsV1 {
        cash_atoms: selection.receipt.dealer_net_cash_in_atoms,
        source_reserved_cash_atoms: 0,
        destination_reserved_cash_atoms: 0,
        native_eggs: selection.trade.buy_from_users,
    };
    let credited = if finalize.is_zero() {
        leased
    } else {
        finalize.validate(selection.outcome_count)?;
        apply_position_credit(leased, finalize)?
    };
    let terminal = advance_position_generation(credited)?;
    Ok(DealerCoveredPositionProjectionV1 { leased, terminal })
}

impl PreparedDealerPositionPotTransferV1 {
    /// Canonical bundle whose identity enters Replay.
    pub const fn bundle(self) -> DealerAssetTransferBundleV1 {
        self.bundle
    }

    /// Exact Position V3 post-state; Pot post-state remains owned by PotV2.
    pub const fn position_post(self) -> PositionAccountV3 {
        self.position_post
    }
}

/// Prepare Begin/Collect Position→Pot or Deliver/Finalize/Abort Pot→Position.
pub fn prepare_dealer_position_pot_transfer_v1(
    action: DealerRuntimeActionV1,
    market: DealerPositionMarketJoinV1,
    position: DealerTransferPositionV3,
    pot: DealerPotCustodyTransitionV1,
    amounts: DealerAssetTransferAmountsV1,
) -> Result<PreparedDealerPositionPotTransferV1> {
    market.validate()?;
    position.validate(market)?;
    pot.validate(market)?;
    let zero_begin = action == DealerRuntimeActionV1::SelectLeaseAndBegin && amounts.is_zero();
    if zero_begin {
        validate_padding_u64(market.outcome_count, &amounts.native_eggs)?;
    } else {
        amounts.validate(market.outcome_count)?;
    }
    if position.account_id() == pot.pot_account_id {
        return Err(Error::MismatchedBinding);
    }
    let position_pre_id = position.semantic_id()?;
    let position_to_pot = matches!(
        action,
        DealerRuntimeActionV1::SelectLeaseAndBegin | DealerRuntimeActionV1::Collect
    );
    let (source_kind, destination_kind, source_account_id, destination_account_id) =
        if position_to_pot {
            (
                position.kind(),
                DealerAssetEndpointKindV1::SettlementPot,
                position.account_id(),
                pot.pot_account_id,
            )
        } else {
            (
                DealerAssetEndpointKindV1::SettlementPot,
                position.kind(),
                pot.pot_account_id,
                position.account_id(),
            )
        };
    require_transfer_direction(action, source_kind, destination_kind)?;
    let position_post = if position_to_pot {
        require_pot_credit_delta(pot, amounts)?;
        if zero_begin {
            position.position()
        } else {
            apply_position_debit(position.position(), amounts)?
        }
    } else {
        if amounts.source_reserved_cash_atoms != 0 || amounts.destination_reserved_cash_atoms != 0 {
            return Err(Error::ConservationFailure);
        }
        require_pot_debit_delta(pot, amounts)?;
        let credited = apply_position_credit(position.position(), amounts)?;
        if matches!(
            action,
            DealerRuntimeActionV1::FinalizeSettlement
                | DealerRuntimeActionV1::AbortBeforeCollection
        ) {
            advance_position_generation(credited)?
        } else {
            credited
        }
    };
    let position_post_id = position_semantic_id(position_post)?;
    let (source_pre, source_post, destination_pre, destination_post) = if position_to_pot {
        (
            position_pre_id,
            position_post_id,
            pot.pot_pre_semantic_id,
            pot.pot_post_semantic_id,
        )
    } else {
        (
            pot.pot_pre_semantic_id,
            pot.pot_post_semantic_id,
            position_pre_id,
            position_post_id,
        )
    };
    let bundle = DealerAssetTransferBundleV1 {
        action,
        source_kind,
        destination_kind,
        market,
        source_account_id,
        destination_account_id,
        source_pre_semantic_id: source_pre,
        source_post_semantic_id: source_post,
        destination_pre_semantic_id: destination_pre,
        destination_post_semantic_id: destination_post,
        amounts,
    };
    bundle.validate()?;
    Ok(PreparedDealerPositionPotTransferV1 {
        bundle,
        position_post,
    })
}

/// Reloaded semantic identities after an internal asset movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerAssetTransferPostObservationV1 {
    /// Exact runtime source account.
    pub source_account_id: Id,
    /// Exact runtime destination account.
    pub destination_account_id: Id,
    /// Reloaded source semantic identity.
    pub source_post_semantic_id: Id,
    /// Reloaded destination semantic identity.
    pub destination_post_semantic_id: Id,
}

/// Accept an exact internal transfer only after both semantic owners reload.
pub fn accept_dealer_asset_transfer_v1(
    bundle: DealerAssetTransferBundleV1,
    observed: DealerAssetTransferPostObservationV1,
) -> Result<Id> {
    bundle.validate()?;
    if observed.source_account_id != bundle.source_account_id
        || observed.destination_account_id != bundle.destination_account_id
        || observed.source_post_semantic_id != bundle.source_post_semantic_id
        || observed.destination_post_semantic_id != bundle.destination_post_semantic_id
    {
        return Err(Error::MismatchedBinding);
    }
    bundle.bundle_id()
}

fn apply_position_debit(
    position: PositionAccountV3,
    amounts: DealerAssetTransferAmountsV1,
) -> Result<PositionAccountV3> {
    let mut fields = position.fields();
    let unreserved_cash = fields
        .cash_atoms
        .checked_sub(fields.reserved_cash_atoms)
        .ok_or(Error::ConservationFailure)?;
    let unreserved_debit = amounts
        .cash_atoms
        .checked_sub(amounts.source_reserved_cash_atoms)
        .ok_or(Error::ConservationFailure)?;
    if amounts.source_reserved_cash_atoms > fields.reserved_cash_atoms
        || unreserved_debit > unreserved_cash
    {
        return Err(Error::ConservationFailure);
    }
    fields.cash_atoms = fields
        .cash_atoms
        .checked_sub(amounts.cash_atoms)
        .ok_or(Error::ConservationFailure)?;
    fields.reserved_cash_atoms = fields
        .reserved_cash_atoms
        .checked_sub(amounts.source_reserved_cash_atoms)
        .ok_or(Error::ConservationFailure)?;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        fields.native_eggs[index] = fields.native_eggs[index]
            .checked_sub(amounts.native_eggs[index])
            .ok_or(Error::ConservationFailure)?;
        index += 1;
    }
    PositionAccountV3::new(fields).map_err(|_| Error::ConservationFailure)
}

fn sponsor_capital_amounts(capital_atoms: u64) -> Result<DealerAssetTransferAmountsV1> {
    if capital_atoms == 0 {
        return Err(Error::InvalidParameter);
    }
    Ok(DealerAssetTransferAmountsV1 {
        cash_atoms: capital_atoms,
        source_reserved_cash_atoms: 0,
        destination_reserved_cash_atoms: 0,
        native_eggs: [0; MAX_OUTCOMES],
    })
}

fn lp_share_amounts(
    policy: &crate::DealerPolicyV1,
    share_delta: u64,
) -> Result<DealerAssetTransferAmountsV1> {
    if share_delta == 0 {
        return Err(Error::InvalidParameter);
    }
    let cash_atoms = policy
        .capital_unit_cash_atoms
        .checked_mul(share_delta)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut native_eggs = [0u64; MAX_OUTCOMES];
    let mut index = 0usize;
    while index < usize::from(policy.outcome_count) {
        native_eggs[index] = policy.capital_unit_eggs[index]
            .checked_mul(share_delta)
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    let amounts = DealerAssetTransferAmountsV1 {
        cash_atoms,
        source_reserved_cash_atoms: 0,
        destination_reserved_cash_atoms: 0,
        native_eggs,
    };
    amounts.validate(policy.outcome_count)?;
    Ok(amounts)
}

fn require_general_owner(position: DealerTransferPositionV3, expected_owner: Id) -> Result<()> {
    expected_owner.validate_live()?;
    match position {
        DealerTransferPositionV3::General { position, .. }
            if identity_matches(position.position().owner(), expected_owner) =>
        {
            Ok(())
        }
        _ => Err(Error::MismatchedBinding),
    }
}

fn apply_position_credit(
    position: PositionAccountV3,
    amounts: DealerAssetTransferAmountsV1,
) -> Result<PositionAccountV3> {
    let mut fields: PositionV3Fields = position.fields();
    fields.cash_atoms = fields
        .cash_atoms
        .checked_add(amounts.cash_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    fields.reserved_cash_atoms = fields
        .reserved_cash_atoms
        .checked_add(amounts.destination_reserved_cash_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        fields.native_eggs[index] = fields.native_eggs[index]
            .checked_add(amounts.native_eggs[index])
            .ok_or(Error::ArithmeticOverflow)?;
        index += 1;
    }
    PositionAccountV3::new(fields).map_err(|_| Error::ConservationFailure)
}

fn require_pot_credit_delta(
    pot: DealerPotCustodyTransitionV1,
    amounts: DealerAssetTransferAmountsV1,
) -> Result<()> {
    if pot.cash_pre_atoms.checked_add(amounts.cash_atoms) != Some(pot.cash_post_atoms) {
        return Err(Error::ConservationFailure);
    }
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        if pot.eggs_pre[index].checked_add(amounts.native_eggs[index]) != Some(pot.eggs_post[index])
        {
            return Err(Error::ConservationFailure);
        }
        index += 1;
    }
    Ok(())
}

fn require_pot_debit_delta(
    pot: DealerPotCustodyTransitionV1,
    amounts: DealerAssetTransferAmountsV1,
) -> Result<()> {
    if pot.cash_pre_atoms.checked_sub(amounts.cash_atoms) != Some(pot.cash_post_atoms) {
        return Err(Error::ConservationFailure);
    }
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        if pot.eggs_pre[index].checked_sub(amounts.native_eggs[index]) != Some(pot.eggs_post[index])
        {
            return Err(Error::ConservationFailure);
        }
        index += 1;
    }
    Ok(())
}

fn require_transfer_direction(
    action: DealerRuntimeActionV1,
    source: DealerAssetEndpointKindV1,
    destination: DealerAssetEndpointKindV1,
) -> Result<()> {
    let valid = match action {
        DealerRuntimeActionV1::Initialize | DealerRuntimeActionV1::Contribute => {
            source == DealerAssetEndpointKindV1::GeneralPosition
                && destination == DealerAssetEndpointKindV1::FacilityPosition
        }
        DealerRuntimeActionV1::WithdrawFunding | DealerRuntimeActionV1::RefundCancelledSponsor => {
            source == DealerAssetEndpointKindV1::FacilityPosition
                && destination == DealerAssetEndpointKindV1::GeneralPosition
        }
        DealerRuntimeActionV1::Claim => {
            source == DealerAssetEndpointKindV1::FacilityPosition
                && destination == DealerAssetEndpointKindV1::GeneralPosition
        }
        DealerRuntimeActionV1::SelectLeaseAndBegin => {
            source == DealerAssetEndpointKindV1::FacilityPosition
                && destination == DealerAssetEndpointKindV1::SettlementPot
        }
        DealerRuntimeActionV1::Collect => {
            source == DealerAssetEndpointKindV1::GeneralPosition
                && destination == DealerAssetEndpointKindV1::SettlementPot
        }
        DealerRuntimeActionV1::Deliver => {
            source == DealerAssetEndpointKindV1::SettlementPot
                && destination == DealerAssetEndpointKindV1::GeneralPosition
        }
        DealerRuntimeActionV1::FinalizeSettlement
        | DealerRuntimeActionV1::AbortBeforeCollection => {
            source == DealerAssetEndpointKindV1::SettlementPot
                && destination == DealerAssetEndpointKindV1::FacilityPosition
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::MismatchedBinding)
    }
}

const fn action_has_asset_transfer(action: DealerRuntimeActionV1) -> bool {
    matches!(
        action,
        DealerRuntimeActionV1::Initialize
            | DealerRuntimeActionV1::Contribute
            | DealerRuntimeActionV1::WithdrawFunding
            | DealerRuntimeActionV1::RefundCancelledSponsor
            | DealerRuntimeActionV1::SelectLeaseAndBegin
            | DealerRuntimeActionV1::Collect
            | DealerRuntimeActionV1::Deliver
            | DealerRuntimeActionV1::FinalizeSettlement
            | DealerRuntimeActionV1::AbortBeforeCollection
            | DealerRuntimeActionV1::Claim
    )
}

const fn endpoint_kind_byte(kind: DealerAssetEndpointKindV1) -> u8 {
    match kind {
        DealerAssetEndpointKindV1::GeneralPosition => 1,
        DealerAssetEndpointKindV1::FacilityPosition => 2,
        DealerAssetEndpointKindV1::SettlementPot => 3,
    }
}

fn identity_matches(identity: clutch_retirement::Identity32V1, expected: Id) -> bool {
    identity.bytes() == expected.bytes()
}

fn position_semantic_id(position: PositionAccountV3) -> Result<Id> {
    let identity = position
        .semantic_id(&DealerPositionSha256V1)
        .map_err(|_| Error::MismatchedBinding)?;
    Ok(Id::from_bytes(identity.bytes()))
}

fn advance_position_generation(position: PositionAccountV3) -> Result<PositionAccountV3> {
    let mut fields = position.fields();
    fields.generation = fields
        .generation
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    PositionAccountV3::new(fields).map_err(|_| Error::ConservationFailure)
}

#[derive(Clone, Copy, Debug)]
struct DealerPositionSha256V1;

impl PositionV3Sha256Backend for DealerPositionSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(body);
        hasher.finalize().into()
    }
}
