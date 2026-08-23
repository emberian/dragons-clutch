// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerFacilityGenesisV1, DealerPhaseV1, DealerPhaseV2, DealerPolicyV1,
    DealerReplayClosePlanV1, DealerStateV1, DealerStateV2, DealerTerminalStateReceiptV2, Error,
    FacilityPositionBindingV1, FacilityPositionBindingV2, FixedCodec, Id, Result,
};
use clutch_retirement::{
    PositionPurposeV3, PositionTombstoneV3, PositionV3Sha256Backend,
};
use sha2::{Digest, Sha256};

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
pub const DEALER_ROOT_TOMBSTONE_BYTES_V2: usize = HEADER_BYTES + (13 * 32) + (5 * 8);

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
    /// Canonical permanent Position V3 tombstone identity.
    pub position_tombstone_id: Id,
    /// Semantic identity of the exact deleted terminal Replay V3 body.
    pub terminal_replay_semantic_id: Id,
    /// Last Retire transition intent committed by terminal Replay.
    pub terminal_replay_intent_id: Id,
    /// State-owned terminal receipt committed by terminal Replay.
    pub terminal_state_receipt_id: Id,
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
            self.position_tombstone_id,
            self.terminal_replay_semantic_id,
            self.terminal_replay_intent_id,
            self.terminal_state_receipt_id,
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
    #[allow(clippy::too_many_arguments)]
    pub fn validate_retirement(
        &self,
        genesis: &DealerFacilityGenesisV1,
        binding: &FacilityPositionBindingV2,
        policy: &DealerPolicyV1,
        terminalizing_state: &DealerStateV2,
        terminal_receipt: &DealerTerminalStateReceiptV2,
        closed_state: &DealerStateV2,
        position_tombstone: PositionTombstoneV3,
        replay_close: DealerReplayClosePlanV1,
    ) -> Result<()> {
        self.validate()?;
        let facility_id = genesis.facility_id_for_policy(policy)?.untyped();
        let binding_id = binding.binding_id_for(genesis, policy)?;
        terminalizing_state.validate_against_policy(policy)?;
        terminal_receipt.validate()?;
        closed_state.validate_against_policy(policy)?;
        position_tombstone
            .validate()
            .map_err(|_| Error::MismatchedBinding)?;
        let position_fields = position_tombstone.fields();
        let position_tombstone_id = Id::from_bytes(
            position_tombstone
                .semantic_id(&DealerRootSha256V2)
                .map_err(|_| Error::MismatchedBinding)?
                .bytes(),
        );
        if terminalizing_state.phase != DealerPhaseV2::Retiring
            || terminalizing_state.children.facility_positions != 1
            || terminalizing_state.children.facility_replays != 1
            || terminalizing_state.children.lp_pages != 0
            || terminalizing_state.children.live_lp_positions != 0
            || terminalizing_state.children.unclaimed_lp_positions != 0
            || terminalizing_state.children.funded_dependencies != 0
            || terminalizing_state.children.epoch_bindings != 0
            || terminalizing_state.children.leases != 0
            || terminalizing_state.children.settlement_pots != 0
            || terminalizing_state.children.terminal_allocations != 0
            || terminalizing_state.children.claim_work != 0
            || closed_state.phase != DealerPhaseV2::Closed
            || closed_state.children != crate::DealerChildCountsV2::default()
            || !closed_state.funded_dependencies_account_id.is_zero()
            || self.policy_id != policy.policy_id()?
            || self.facility_id != facility_id
            || self.facility_position_binding_id != binding_id
            || self.funded_dependencies_id != closed_state.funded_dependencies_id
            || self.terminal_facility_position_id != terminal_receipt.terminal_position_semantic_id
            || self.position_tombstone_id != position_tombstone_id
            || self.terminal_replay_semantic_id != replay_close.terminal_replay_semantic_id()
            || self.terminal_replay_intent_id != replay_close.last_transition_intent_id()
            || self.terminal_state_receipt_id != terminal_receipt.receipt_id()?
            || self.terminal_state_receipt_id != replay_close.terminal_state_receipt_id()
            || self.terminal_state_id != closed_state.state_content_id()?
            || self.dealer_state_account_id != binding.dealer_state_account_id
            || terminal_receipt.terminal_state_content_id
                != terminalizing_state.state_content_id()?
            || terminal_receipt.policy_id != self.policy_id
            || terminal_receipt.facility_id != self.facility_id
            || terminal_receipt.facility_position_binding_id != binding_id
            || terminal_receipt.dealer_state_account_id != self.dealer_state_account_id
            || terminal_receipt.replay_account_id != replay_close.replay_account_id()
            || self.facility_id != closed_state.facility_id
            || self.facility_id != terminalizing_state.facility_id
            || closed_state.facility_position_account_id != binding.facility_position_account_id
            || terminalizing_state.facility_position_account_id
                != binding.facility_position_account_id
            || closed_state.facility_replay_account_id != replay_close.replay_account_id()
            || terminalizing_state.facility_replay_account_id != replay_close.replay_account_id()
            || position_fields.purpose != PositionPurposeV3::DealerFacility
            || Id::from_bytes(position_fields.market_instance_id.bytes())
                != policy.market_instance_v2_id
            || Id::from_bytes(position_fields.realm_id.bytes()) != policy.realm_id
            || Id::from_bytes(position_fields.collateral_policy_id.bytes())
                != binding.collateral_policy_id
            || Id::from_bytes(position_fields.collateral_release_id.bytes())
                != binding.collateral_release_id
            || Id::from_bytes(position_fields.owner.bytes()) != self.facility_id
            || Id::from_bytes(position_fields.controller.bytes()) != self.dealer_state_account_id
            || Id::from_bytes(position_fields.replay_account.bytes())
                != replay_close.replay_account_id()
            || Id::from_bytes(position_fields.purpose_binding_id.bytes()) != binding_id
            || self.terminal_generation != closed_state.generation
            || self.terminal_generation != terminalizing_state.generation
            || self.terminal_generation != position_fields.generation
            || self.terminal_generation != terminal_receipt.terminal_generation
            || self.terminal_child_sequence != closed_state.child_sequence
            || self.terminal_child_sequence
                != terminalizing_state
                    .child_sequence
                    .checked_add(2)
                    .ok_or(Error::ArithmeticOverflow)?
            || terminal_receipt.terminal_child_sequence != terminalizing_state.child_sequence
            || self.rent_payer != closed_state.rent.payer
            || self.neutral_sink != closed_state.rent.neutral_sink
            || self.refunded_live_principal
                != closed_state.rent.refundable_live_principal
            || self.permanent_tombstone_principal
                != closed_state.rent.permanent_tombstone_principal
            || self.creation_donation_floor != closed_state.rent.donation_floor
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
            self.position_tombstone_id,
            self.terminal_replay_semantic_id,
            self.terminal_replay_intent_id,
            self.terminal_state_receipt_id,
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
            position_tombstone_id: reader.id(),
            terminal_replay_semantic_id: reader.id(),
            terminal_replay_intent_id: reader.id(),
            terminal_state_receipt_id: reader.id(),
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
const _: () = assert!(DEALER_ROOT_TOMBSTONE_BYTES_V2 == 468);
const _: () = assert!(DEALER_ROOT_TOMBSTONE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_ROOT_TOMBSTONE_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);

#[derive(Clone, Copy, Debug)]
struct DealerRootSha256V2;

impl PositionV3Sha256Backend for DealerRootSha256V2 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        hasher.update(body);
        hasher.finalize().into()
    }
}
