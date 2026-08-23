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
use sha2::{Digest, Sha256};

/// Local semantic magic for the non-self-referential Position V3 binding.
pub const FACILITY_POSITION_BINDING_MAGIC_V2: [u8; 8] = *b"DCFPBND2";
/// Exact local semantic version.
pub const FACILITY_POSITION_BINDING_VERSION_V2: u16 = 2;
/// Exact canonical binding bytes: header, six identities, role, and generation.
pub const FACILITY_POSITION_BINDING_BYTES_V2: usize = HEADER_BYTES + (6 * 32) + 8 + 8;

/// Immutable non-cyclic purpose binding consumed by canonical Position V3.
///
/// The body deliberately omits the Position semantic ID: Position V3 commits
/// this binding ID. It also omits the physical Position and Replay addresses:
/// their PDAs are derived from this ID, so committing either address would
/// create an address/hash fixed point. `DealerStateV2` separately owns both
/// physical addresses and the current mutable Position semantic ID.
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
            self.dealer_state_account_id,
        ] {
            identity.validate_live()?;
        }
        if self.initial_position_generation != 1 {
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
        self.purpose_binding_id()
    }

    /// Return the canonical purpose-binding ID after local validation.
    pub fn binding_id(&self) -> Result<Id> {
        self.validate()?;
        self.purpose_binding_id()
    }

    /// Derive the non-cyclic purpose binding used by PositionV3 and ReplayV3.
    ///
    /// Physical Position/Replay addresses are deliberately absent: both PDAs
    /// are derived *from* this identity, while `DealerStateV2` remains their
    /// sole persisted owner. Including either address here would require an
    /// impossible address/hash fixed point at facility initialization.
    fn purpose_binding_id(&self) -> Result<Id> {
        let mut hasher = Sha256::new();
        hasher.update(FACILITY_POSITION_BINDING_CONTENT_DOMAIN_V2);
        for identity in [
            self.facility_id,
            self.policy_id,
            self.market_instance_v2_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.dealer_state_account_id,
        ] {
            hasher.update(identity.bytes());
        }
        hasher.update([u8::from(clutch_retirement::PositionPurposeV3::DealerFacility)]);
        hasher.update([0u8; 7]);
        hasher.update(self.initial_position_generation.to_le_bytes());
        let digest: [u8; 32] = hasher.finalize().into();
        let value = Id::from_bytes(digest);
        value.validate_live()?;
        Ok(value)
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
            self.dealer_state_account_id,
        ] {
            writer.id(identity);
        }
        writer.u8(u8::from(clutch_retirement::PositionPurposeV3::DealerFacility));
        writer.reserved(7);
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
            dealer_state_account_id: reader.id(),
            initial_position_generation: {
                if reader.u8() != u8::from(clutch_retirement::PositionPurposeV3::DealerFacility) {
                    return Err(Error::MismatchedBinding);
                }
                reader.reserved(7)?;
                reader.u64()
            },
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
        if Id::from_bytes(position.market_instance_id().bytes())
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

const _: () = assert!(FACILITY_POSITION_BINDING_BYTES_V2 == 220);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn binding() -> FacilityPositionBindingV2 {
        FacilityPositionBindingV2 {
            facility_id: id(1),
            policy_id: id(2),
            market_instance_v2_id: id(3),
            collateral_policy_id: id(4),
            collateral_release_id: id(5),
            dealer_state_account_id: id(6),
            initial_position_generation: 1,
        }
    }

    #[test]
    fn purpose_binding_codec_and_noncyclic_digest_are_frozen() {
        let value = binding();
        let mut bytes = [0u8; FACILITY_POSITION_BINDING_BYTES_V2];
        value.encode_into(&mut bytes).unwrap();
        assert_eq!(FacilityPositionBindingV2::decode(&bytes), Ok(value));
        assert_eq!(bytes[204], u8::from(clutch_retirement::PositionPurposeV3::DealerFacility));
        assert_eq!(&bytes[205..212], &[0; 7]);

        let baseline = value.binding_id().unwrap();
        for replacement in [id(11), id(12), id(13), id(14), id(15), id(16)] {
            let mut changed = value;
            match replacement.bytes()[0] {
                11 => changed.facility_id = replacement,
                12 => changed.policy_id = replacement,
                13 => changed.market_instance_v2_id = replacement,
                14 => changed.collateral_policy_id = replacement,
                15 => changed.collateral_release_id = replacement,
                _ => changed.dealer_state_account_id = replacement,
            }
            assert_ne!(changed.binding_id().unwrap(), baseline);
        }

        let mut hostile = bytes;
        hostile[205] = 1;
        assert!(FacilityPositionBindingV2::decode(&hostile).is_err());
    }
}
const _: () = assert!(FACILITY_POSITION_BINDING_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
