// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact Dealer projection of the shared canonical Position V3 owner.
//!
//! This module does not authenticate Solana metadata or compute the Position
//! semantic digest. A live adapter must authenticate the Position V3 account,
//! its program owner/PDA, and the supplied semantic identity before calling
//! these checks. The projection prevents Dealer from persisting a second asset
//! DTO beside the shared Position owner.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerFacilityGenesisV1, DealerPolicyV1, DealerStateV2, Error, FixedCodec, Id, Result,
    FACILITY_POSITION_BINDING_CONTENT_DOMAIN_V2,
};
use clutch_retirement::{DealerPositionProjectionV3, PositionLifecycleV3};

/// Local semantic magic for the non-self-referential Position V3 binding.
pub const FACILITY_POSITION_BINDING_MAGIC_V2: [u8; 8] = *b"DCFPBND2";
/// Exact local semantic version.
pub const FACILITY_POSITION_BINDING_VERSION_V2: u16 = 2;
/// Exact canonical binding bytes: header, eight identities, generation.
pub const FACILITY_POSITION_BINDING_BYTES_V2: usize = HEADER_BYTES + (8 * 32) + 8;

/// Immutable purpose binding consumed by canonical Position V3.
///
/// The body deliberately omits the Position semantic ID: Position V3 commits
/// this binding ID, so including its semantic ID here would create a cyclic
/// hash preimage. `DealerStateV2` separately owns the current mutable Position
/// semantic ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FacilityPositionBindingV2 {
    /// Canonical facility identity.
    pub facility_id: Id,
    /// Exact Dealer policy identity.
    pub policy_id: Id,
    /// Full MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Realm-selected collateral-policy identity.
    pub collateral_policy_id: Id,
    /// Exact admitted collateral-adapter release identity.
    pub collateral_release_id: Id,
    /// Shared Position V3 account identity.
    pub facility_position_account_id: Id,
    /// Founding current-generation Replay account identity.
    pub facility_replay_account_id: Id,
    /// Exact DealerState V2 account and Position controller.
    pub dealer_state_account_id: Id,
    /// Founding Position V3 generation; exactly one.
    pub initial_position_generation: u64,
}

impl FacilityPositionBindingV2 {
    /// Validate identities, distinct account roles, and Position V3 generation.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.facility_id,
            self.policy_id,
            self.market_instance_v2_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.facility_position_account_id,
            self.facility_replay_account_id,
            self.dealer_state_account_id,
        ] {
            identity.validate_live()?;
        }
        if self.initial_position_generation != 1
            || self.facility_position_account_id == self.facility_replay_account_id
            || self.facility_position_account_id == self.dealer_state_account_id
            || self.facility_replay_account_id == self.dealer_state_account_id
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Require exact Genesis/Policy provenance and return the purpose-binding ID.
    pub fn binding_id_for(
        &self,
        genesis: &DealerFacilityGenesisV1,
        policy: &DealerPolicyV1,
    ) -> Result<Id> {
        self.validate()?;
        genesis.validate()?;
        policy.validate()?;
        if self.facility_id != genesis.facility_id_for_policy(policy)?.untyped()
            || self.policy_id != policy.policy_id()?
            || self.market_instance_v2_id != policy.market_instance_v2_id
        {
            return Err(Error::MismatchedBinding);
        }
        self.content_id(FACILITY_POSITION_BINDING_CONTENT_DOMAIN_V2)
    }

    /// Return the canonical purpose-binding ID after local validation.
    pub fn binding_id(&self) -> Result<Id> {
        self.validate()?;
        self.content_id(FACILITY_POSITION_BINDING_CONTENT_DOMAIN_V2)
    }
}

impl FixedCodec for FacilityPositionBindingV2 {
    const ENCODED_LEN: usize = FACILITY_POSITION_BINDING_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &FACILITY_POSITION_BINDING_MAGIC_V2,
            FACILITY_POSITION_BINDING_VERSION_V2,
        );
        for identity in [
            self.facility_id,
            self.policy_id,
            self.market_instance_v2_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.facility_position_account_id,
            self.facility_replay_account_id,
            self.dealer_state_account_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.initial_position_generation);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &FACILITY_POSITION_BINDING_MAGIC_V2,
            FACILITY_POSITION_BINDING_VERSION_V2,
        )?;
        let value = Self {
            facility_id: reader.id(),
            policy_id: reader.id(),
            market_instance_v2_id: reader.id(),
            collateral_policy_id: reader.id(),
            collateral_release_id: reader.id(),
            facility_position_account_id: reader.id(),
            facility_replay_account_id: reader.id(),
            dealer_state_account_id: reader.id(),
            initial_position_generation: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Adapter-authenticated observation of one shared Position V3 account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerPositionObservationV3 {
    /// Physical Position V3 account identity.
    pub account_id: Id,
    /// SHA-256 semantic identity recomputed from the canonical body.
    pub semantic_id: Id,
    /// Purpose-checked canonical Dealer projection.
    pub projection: DealerPositionProjectionV3,
}

impl DealerPositionObservationV3 {
    /// Join immutable Position V3 fields to the exact Dealer binding and policy.
    pub fn validate_against(
        &self,
        binding: &FacilityPositionBindingV2,
        binding_id: Id,
        policy: &DealerPolicyV1,
    ) -> Result<()> {
        self.account_id.validate_live()?;
        self.semantic_id.validate_live()?;
        binding.validate()?;
        policy.validate()?;
        let position = self.projection.position();
        position.validate().map_err(|_| Error::MismatchedBinding)?;
        if self.account_id != binding.facility_position_account_id
            || Id::from_bytes(position.market_instance_id().bytes())
                != binding.market_instance_v2_id
            || Id::from_bytes(position.realm_id().bytes()) != policy.realm_id
            || Id::from_bytes(position.collateral_policy_id().bytes())
                != binding.collateral_policy_id
            || Id::from_bytes(position.collateral_release_id().bytes())
                != binding.collateral_release_id
            || Id::from_bytes(position.owner().bytes()) != binding.facility_id
            || Id::from_bytes(position.controller().bytes()) != binding.dealer_state_account_id
            || Id::from_bytes(position.purpose_binding_id().bytes()) != binding_id
            || position.outcome_count() != policy.outcome_count
            || position.outstanding_reservations() != 0
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Join a live State to the current Position semantic/account/Replay facts.
    pub fn validate_current(
        &self,
        state: &DealerStateV2,
        binding: &FacilityPositionBindingV2,
        policy: &DealerPolicyV1,
    ) -> Result<()> {
        let binding_id = binding.binding_id()?;
        self.validate_against(binding, binding_id, policy)?;
        let position = self.projection.position();
        if self.semantic_id != state.facility_position_id
            || self.account_id != state.facility_position_account_id
            || Id::from_bytes(position.replay_account().bytes())
                != state.facility_replay_account_id
            || position.generation() != state.generation
            || position.lifecycle() != PositionLifecycleV3::Open
            || state.facility_position_binding_id != binding_id
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }
}

const _: () = assert!(FACILITY_POSITION_BINDING_BYTES_V2 == 276);
const _: () = assert!(FACILITY_POSITION_BINDING_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
