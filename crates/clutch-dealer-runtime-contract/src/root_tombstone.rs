// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerFacilityGenesisV1, DealerPhaseV1, DealerPhaseV2, DealerPolicyV1, DealerStateV1, DealerStateV2,
    Error, FacilityPositionBindingV1, FixedCodec, Id, Result,
};

/// Local semantic-body magic for the permanent Dealer root tombstone.
pub const DEALER_ROOT_TOMBSTONE_MAGIC_V1: [u8; 8] = *b"DCRTMBV1";
/// Exact local semantic-body version.
pub const DEALER_ROOT_TOMBSTONE_VERSION_V1: u16 = 1;
/// Exact bytes retained after the live Dealer root is shrunk.
pub const DEALER_ROOT_TOMBSTONE_BYTES_V1: usize = HEADER_BYTES + (7 * 32) + (5 * 8);
/// Local semantic-body magic for the funded-provenance root successor.
pub const DEALER_ROOT_TOMBSTONE_MAGIC_V2: [u8; 8] = *b"DCRTMBV2";
/// Exact local semantic-body version for the root successor.
pub const DEALER_ROOT_TOMBSTONE_VERSION_V2: u16 = 2;
/// Exact bytes retained after shrinking a V2 State root.
pub const DEALER_ROOT_TOMBSTONE_BYTES_V2: usize = HEADER_BYTES + (9 * 32) + (5 * 8);

/// Permanent evidence retained after every counted Dealer child is closed.
///
/// The tombstone owns no assets and no refundable liability. It preserves the
/// exact terminal State content identity and original rent split so an adapter
/// can prove that only the live-state principal was refunded, the independently
/// prepaid tombstone principal remained, and all surplus went to the one policy
/// sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerRootTombstoneV1 {
    /// Exact retained Dealer policy identity.
    pub policy_id: Id,
    /// Canonical facility identity.
    pub facility_id: Id,
    /// Exact immutable Facility Position authority-binding identity.
    pub facility_position_binding_id: Id,
    /// Content identity of the exhaustive terminal `DealerStateV1` body.
    pub terminal_state_id: Id,
    /// Exact root account that was shrunk in place.
    pub dealer_state_account_id: Id,
    /// Sole recipient of the refunded live-state rent principal.
    pub rent_payer: Id,
    /// One policy sink that received prefunds and surplus.
    pub neutral_sink: Id,
    /// Final economic generation.
    pub terminal_generation: u64,
    /// Final monotone child/account sequence.
    pub terminal_child_sequence: u64,
    /// Exact live-state principal returned to `rent_payer`.
    pub refunded_live_principal: u64,
    /// Independently prepaid principal retained by this tombstone.
    pub permanent_tombstone_principal: u64,
    /// Creation-time hostile prefund routed to `neutral_sink`.
    pub creation_donation_floor: u64,
}

impl DealerRootTombstoneV1 {
    /// Validate all retained identities and the disjoint rent split.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.terminal_state_id,
            self.dealer_state_account_id,
            self.rent_payer,
            self.neutral_sink,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index].validate_live()?;
            index += 1;
        }
        if self.rent_payer == self.neutral_sink
            || self.refunded_live_principal == 0
            || self.permanent_tombstone_principal == 0
        {
            return Err(Error::InvalidParameter);
        }
        self.refunded_live_principal
            .checked_add(self.permanent_tombstone_principal)
            .and_then(|value| value.checked_add(self.creation_donation_floor))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Require an exhaustive terminal root and every immutable identity/rent join.
    pub fn validate_retirement(
        &self,
        genesis: &DealerFacilityGenesisV1,
        binding: &FacilityPositionBindingV1,
        policy: &DealerPolicyV1,
        terminal_state: &DealerStateV1,
    ) -> Result<()> {
        self.validate()?;
        let facility_id = genesis.facility_id_for_policy(policy)?;
        let binding_id = binding.binding_id_for(genesis, policy)?;
        terminal_state.validate_against_policy(policy)?;
        if terminal_state.phase != DealerPhaseV1::Closed
            || self.policy_id != genesis.policy_id
            || self.facility_id != facility_id.untyped()
            || self.facility_position_binding_id != binding_id.untyped()
            || self.facility_position_binding_id
                != terminal_state.facility_position_binding_id
            || self.terminal_state_id != terminal_state.state_content_id()?
            || self.dealer_state_account_id != binding.dealer_state_account_id
            || self.facility_id != terminal_state.facility_id
            || terminal_state.facility_position_id != binding.facility_position_semantic_id
            || terminal_state.facility_position_account_id != binding.facility_position_account_id
            || terminal_state.facility_replay_account_id != binding.facility_replay_account_id
            || self.terminal_generation != terminal_state.generation
            || self.terminal_child_sequence != terminal_state.child_sequence
            || self.rent_payer != terminal_state.rent.payer
            || self.neutral_sink != terminal_state.rent.neutral_sink
            || self.refunded_live_principal != terminal_state.rent.refundable_live_principal
            || self.permanent_tombstone_principal
                != terminal_state.rent.permanent_tombstone_principal
            || self.creation_donation_floor != terminal_state.rent.donation_floor
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical permanent evidence identity.
    pub fn tombstone_id(&self) -> Result<Id> {
        self.content_id(crate::DEALER_ROOT_TOMBSTONE_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for DealerRootTombstoneV1 {
    const ENCODED_LEN: usize = DEALER_ROOT_TOMBSTONE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_ROOT_TOMBSTONE_MAGIC_V1,
            DEALER_ROOT_TOMBSTONE_VERSION_V1,
        );
        writer.id(self.policy_id);
        writer.id(self.facility_id);
        writer.id(self.facility_position_binding_id);
        writer.id(self.terminal_state_id);
        writer.id(self.dealer_state_account_id);
        writer.id(self.rent_payer);
        writer.id(self.neutral_sink);
        writer.u64(self.terminal_generation);
        writer.u64(self.terminal_child_sequence);
        writer.u64(self.refunded_live_principal);
        writer.u64(self.permanent_tombstone_principal);
        writer.u64(self.creation_donation_floor);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_ROOT_TOMBSTONE_MAGIC_V1,
            DEALER_ROOT_TOMBSTONE_VERSION_V1,
        )?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            facility_position_binding_id: reader.id(),
            terminal_state_id: reader.id(),
            dealer_state_account_id: reader.id(),
            rent_payer: reader.id(),
            neutral_sink: reader.id(),
            terminal_generation: reader.u64(),
            terminal_child_sequence: reader.u64(),
            refunded_live_principal: reader.u64(),
            permanent_tombstone_principal: reader.u64(),
            creation_donation_floor: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Permanent V2 evidence retaining funded-dependency and terminal Position provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerRootTombstoneV2 {
    /// Dealer policy identity.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Immutable Facility Position authority-binding identity.
    pub facility_position_binding_id: Id,
    /// Immutable semantic identity of the deleted funded-dependency child.
    pub funded_dependencies_id: Id,
    /// Last Facility Position semantic identity before that account closed.
    pub terminal_facility_position_id: Id,
    /// Content identity of the exhaustive closed `DealerStateV2` body.
    pub terminal_state_id: Id,
    /// Root account shrunk in place.
    pub dealer_state_account_id: Id,
    /// Sole recipient of the live-state refundable rent principal.
    pub rent_payer: Id,
    /// Sole sink for prefunds and surplus.
    pub neutral_sink: Id,
    /// Final economic generation.
    pub terminal_generation: u64,
    /// Final child/account sequence.
    pub terminal_child_sequence: u64,
    /// Exact returned live-state principal.
    pub refunded_live_principal: u64,
    /// Independently prepaid principal retained by this tombstone.
    pub permanent_tombstone_principal: u64,
    /// Creation-time hostile prefund routed to the neutral sink.
    pub creation_donation_floor: u64,
}

impl DealerRootTombstoneV2 {
    /// Validate retained identities and the disjoint rent split.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.funded_dependencies_id,
            self.terminal_facility_position_id,
            self.terminal_state_id,
            self.dealer_state_account_id,
            self.rent_payer,
            self.neutral_sink,
        ] {
            identity.validate_live()?;
        }
        if self.rent_payer == self.neutral_sink
            || self.refunded_live_principal == 0
            || self.permanent_tombstone_principal == 0
        {
            return Err(Error::InvalidParameter);
        }
        self.refunded_live_principal
            .checked_add(self.permanent_tombstone_principal)
            .and_then(|value| value.checked_add(self.creation_donation_floor))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Require the closed V2 root and exact immutable facility/rent joins.
    pub fn validate_retirement(
        &self,
        genesis: &DealerFacilityGenesisV1,
        binding: &FacilityPositionBindingV1,
        policy: &DealerPolicyV1,
        terminal_state: &DealerStateV2,
    ) -> Result<()> {
        self.validate()?;
        let facility_id = genesis.facility_id_for_policy(policy)?;
        let binding_id = binding.binding_id_for(genesis, policy)?;
        terminal_state.validate_against_policy(policy)?;
        if terminal_state.phase != DealerPhaseV2::Closed
            || terminal_state.children != crate::DealerChildCountsV2::default()
            || !terminal_state.funded_dependencies_account_id.is_zero()
            || self.policy_id != genesis.policy_id
            || self.facility_id != facility_id.untyped()
            || self.facility_position_binding_id != binding_id.untyped()
            || self.funded_dependencies_id != terminal_state.funded_dependencies_id
            || self.terminal_facility_position_id != terminal_state.facility_position_id
            || self.terminal_state_id != terminal_state.state_content_id()?
            || self.dealer_state_account_id != binding.dealer_state_account_id
            || self.facility_id != terminal_state.facility_id
            || terminal_state.facility_position_account_id
                != binding.facility_position_account_id
            || terminal_state.facility_replay_account_id != binding.facility_replay_account_id
            || self.terminal_generation != terminal_state.generation
            || self.terminal_child_sequence != terminal_state.child_sequence
            || self.rent_payer != terminal_state.rent.payer
            || self.neutral_sink != terminal_state.rent.neutral_sink
            || self.refunded_live_principal
                != terminal_state.rent.refundable_live_principal
            || self.permanent_tombstone_principal
                != terminal_state.rent.permanent_tombstone_principal
            || self.creation_donation_floor != terminal_state.rent.donation_floor
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Canonical V2 permanent evidence identity.
    pub fn tombstone_id(&self) -> Result<Id> {
        self.content_id(crate::DEALER_ROOT_TOMBSTONE_CONTENT_DOMAIN_V2)
    }
}

impl FixedCodec for DealerRootTombstoneV2 {
    const ENCODED_LEN: usize = DEALER_ROOT_TOMBSTONE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_ROOT_TOMBSTONE_MAGIC_V2,
            DEALER_ROOT_TOMBSTONE_VERSION_V2,
        );
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.funded_dependencies_id,
            self.terminal_facility_position_id,
            self.terminal_state_id,
            self.dealer_state_account_id,
            self.rent_payer,
            self.neutral_sink,
        ] {
            writer.id(identity);
        }
        writer.u64(self.terminal_generation);
        writer.u64(self.terminal_child_sequence);
        writer.u64(self.refunded_live_principal);
        writer.u64(self.permanent_tombstone_principal);
        writer.u64(self.creation_donation_floor);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_ROOT_TOMBSTONE_MAGIC_V2,
            DEALER_ROOT_TOMBSTONE_VERSION_V2,
        )?;
        let value = Self {
            policy_id: reader.id(),
            facility_id: reader.id(),
            facility_position_binding_id: reader.id(),
            funded_dependencies_id: reader.id(),
            terminal_facility_position_id: reader.id(),
            terminal_state_id: reader.id(),
            dealer_state_account_id: reader.id(),
            rent_payer: reader.id(),
            neutral_sink: reader.id(),
            terminal_generation: reader.u64(),
            terminal_child_sequence: reader.u64(),
            refunded_live_principal: reader.u64(),
            permanent_tombstone_principal: reader.u64(),
            creation_donation_floor: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(DEALER_ROOT_TOMBSTONE_BYTES_V1 == 276);
const _: () = assert!(DEALER_ROOT_TOMBSTONE_BYTES_V2 == 340);
const _: () = assert!(DEALER_ROOT_TOMBSTONE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_ROOT_TOMBSTONE_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
