// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    DealerChildCountsV1, DealerChildCountsV2, DealerFundedDependenciesV2,
    DealerLivenessScheduleV1, DealerPhaseV1, DealerPhaseV2, DealerPolicyV1,
    DealerPositionObservationV3, DealerRuntimeLivenessBindingV1, DealerStateV1, DealerStateV2,
    Error, FacilityPositionBindingV2, FixedCodec, Id, Result, SponsorCapitalDispositionV1,
    DealerActionLivenessAuthorizationV1, DealerAssetEndpointKindV1,
    DealerFacilityReplayV1, DealerReplayAccountBindingV1, DealerRuntimeActionV1,
    DealerTransitionIntentV1, DealerTransitionLivenessModeV1,
    PreparedDealerPositionPairTransferV1, PreparedDealerReplayTransitionV1,
};

/// Local semantic-body magic for one immutable facility genesis.
pub const DEALER_FACILITY_GENESIS_MAGIC_V1: [u8; 8] = *b"DCDFGNV1";
/// Exact local semantic-body version.
pub const DEALER_FACILITY_GENESIS_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical `DealerFacilityGenesisV1` body.
pub const DEALER_FACILITY_GENESIS_BYTES_V1: usize = HEADER_BYTES + (3 * 32) + 8;

/// Local semantic-body magic for the external Facility Position authority join.
pub const FACILITY_POSITION_BINDING_MAGIC_V1: [u8; 8] = *b"DCFPBND1";
/// Exact local semantic-body version.
pub const FACILITY_POSITION_BINDING_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical `FacilityPositionBindingV1` body.
pub const FACILITY_POSITION_BINDING_BYTES_V1: usize = HEADER_BYTES + (7 * 32) + 8;

/// Local semantic-body magic for the Facility Position itself.
pub const DEALER_FACILITY_POSITION_MAGIC_V1: [u8; 8] = *b"DCFPOSV1";
/// Exact local semantic-body version.
pub const DEALER_FACILITY_POSITION_VERSION_V1: u16 = 1;
/// Exact bytes in one canonical `DealerFacilityPositionV1` body.
pub const DEALER_FACILITY_POSITION_BYTES_V1: usize =
    HEADER_BYTES + (7 * 32) + 8 + (2 * 8) + (crate::MAX_OUTCOMES * 8);

/// Exact content-derived facility identity.
///
/// This is the canonical input to the DealerState PDA recipe. It is never an
/// eight-byte market lowering, a caller-selected nonce by itself, or an
/// account address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DealerFacilityIdV1(Id);

impl DealerFacilityIdV1 {
    /// Recover a typed identity from authenticated persisted bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Id::from_bytes(bytes))
    }

    /// Return the exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Project into the repository's existing untyped 32-byte join surface.
    pub const fn untyped(self) -> Id {
        self.0
    }
}

/// Exact content-derived identity of one Position authority binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct FacilityPositionBindingIdV1(Id);

impl FacilityPositionBindingIdV1 {
    /// Recover a typed identity from authenticated persisted bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Id::from_bytes(bytes))
    }

    /// Return the exact identity bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Project into the repository's existing untyped 32-byte join surface.
    pub const fn untyped(self) -> Id {
        self.0
    }
}

/// Immutable preimage owning the canonical facility identity.
///
/// The policy transitively fixes Realm, full MarketInstanceV2, collateral,
/// curve, fee, liveness, and retirement semantics. Sponsor and refund
/// recipient remain explicit immutable facts. The nonce only distinguishes
/// otherwise identical facilities and has no authority by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFacilityGenesisV1 {
    /// Exact authenticated `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Facility sponsor whose capital is governed by the policy.
    pub sponsor: Id,
    /// Exact pre-activation sponsor-capital refund recipient.
    pub sponsor_refund_recipient: Id,
    /// Caller-selected disambiguator committed inside the content identity.
    pub facility_nonce: u64,
}

impl DealerFacilityGenesisV1 {
    /// Validate all immutable live identities.
    pub fn validate(&self) -> Result<()> {
        self.policy_id.validate_live()?;
        self.sponsor.validate_live()?;
        self.sponsor_refund_recipient.validate_live()
    }

    /// Require that the authenticated policy is the exact policy committed by
    /// this genesis, then return the canonical typed facility identity.
    pub fn facility_id_for_policy(&self, policy: &DealerPolicyV1) -> Result<DealerFacilityIdV1> {
        self.validate()?;
        policy.validate()?;
        if self.policy_id != policy.policy_id()? {
            return Err(Error::MismatchedBinding);
        }
        self.facility_id()
    }

    /// Compute the canonical facility identity from this exact body.
    pub fn facility_id(&self) -> Result<DealerFacilityIdV1> {
        Ok(DealerFacilityIdV1(self.content_id(
            crate::DEALER_FACILITY_GENESIS_CONTENT_DOMAIN_V1,
        )?))
    }
}

impl FixedCodec for DealerFacilityGenesisV1 {
    const ENCODED_LEN: usize = DEALER_FACILITY_GENESIS_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_FACILITY_GENESIS_MAGIC_V1,
            DEALER_FACILITY_GENESIS_VERSION_V1,
        );
        writer.id(self.policy_id);
        writer.id(self.sponsor);
        writer.id(self.sponsor_refund_recipient);
        writer.u64(self.facility_nonce);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_FACILITY_GENESIS_MAGIC_V1,
            DEALER_FACILITY_GENESIS_VERSION_V1,
        )?;
        let value = Self {
            policy_id: reader.id(),
            sponsor: reader.id(),
            sponsor_refund_recipient: reader.id(),
            facility_nonce: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Immutable authenticated join between a facility and its external Position.
///
/// `dealer_state_account_id` is the only admitted authority over the external
/// Facility Position. The adapter must additionally prove that this address is
/// the canonical DealerState PDA under the exact deployed program and that the
/// authenticated external Position body has `facility_position_semantic_id`,
/// account key, Replay key, codec identity, authority, and generation zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FacilityPositionBindingV1 {
    /// Canonical `DealerFacilityGenesisV1` content identity.
    pub facility_id: Id,
    /// Exact authenticated `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Full successor MarketInstanceV2 identity, copied for an exact adapter join.
    pub market_instance_v2_id: Id,
    /// Exact initial external Position semantic content identity.
    pub facility_position_semantic_id: Id,
    /// Exact external Facility Position account key.
    pub facility_position_account_id: Id,
    /// Exact external Replay companion account key.
    pub facility_replay_account_id: Id,
    /// Exact DealerState account key and sole Position authority.
    pub dealer_state_account_id: Id,
    /// Exact initial external Position generation; V1 admits only zero.
    pub initial_position_generation: u64,
}

impl FacilityPositionBindingV1 {
    /// Validate live identities, disjoint account roles, and initial generation.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.facility_id,
            self.policy_id,
            self.market_instance_v2_id,
            self.facility_position_semantic_id,
            self.facility_position_account_id,
            self.facility_replay_account_id,
            self.dealer_state_account_id,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index].validate_live()?;
            index += 1;
        }
        if self.initial_position_generation != 0
            || self.facility_position_account_id == self.facility_replay_account_id
            || self.facility_position_account_id == self.dealer_state_account_id
            || self.facility_replay_account_id == self.dealer_state_account_id
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Require exact Genesis/Policy joins and recompute this binding identity.
    pub fn binding_id_for(
        &self,
        genesis: &DealerFacilityGenesisV1,
        policy: &DealerPolicyV1,
    ) -> Result<FacilityPositionBindingIdV1> {
        self.validate()?;
        let facility_id = genesis.facility_id_for_policy(policy)?;
        if self.facility_id != facility_id.untyped()
            || self.policy_id != genesis.policy_id
            || self.market_instance_v2_id != policy.market_instance_v2_id
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(FacilityPositionBindingIdV1(self.content_id(
            crate::FACILITY_POSITION_BINDING_CONTENT_DOMAIN_V1,
        )?))
    }

    /// Compute this binding's typed identity after local validation only.
    pub fn binding_id(&self) -> Result<FacilityPositionBindingIdV1> {
        Ok(FacilityPositionBindingIdV1(self.content_id(
            crate::FACILITY_POSITION_BINDING_CONTENT_DOMAIN_V1,
        )?))
    }
}

impl FixedCodec for FacilityPositionBindingV1 {
    const ENCODED_LEN: usize = FACILITY_POSITION_BINDING_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &FACILITY_POSITION_BINDING_MAGIC_V1,
            FACILITY_POSITION_BINDING_VERSION_V1,
        );
        writer.id(self.facility_id);
        writer.id(self.policy_id);
        writer.id(self.market_instance_v2_id);
        writer.id(self.facility_position_semantic_id);
        writer.id(self.facility_position_account_id);
        writer.id(self.facility_replay_account_id);
        writer.id(self.dealer_state_account_id);
        writer.u64(self.initial_position_generation);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &FACILITY_POSITION_BINDING_MAGIC_V1,
            FACILITY_POSITION_BINDING_VERSION_V1,
        )?;
        let value = Self {
            facility_id: reader.id(),
            policy_id: reader.id(),
            market_instance_v2_id: reader.id(),
            facility_position_semantic_id: reader.id(),
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

/// Facility Position lifecycle owned by the exact Position semantic body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerFacilityPositionPhaseV1 {
    /// Assets are held by the Position and no settlement Pot is active.
    Idle = 0,
    /// One authenticated generation Lease has refined custody into its Pot.
    Leased = 1,
    /// Resolution/retirement has made the Position terminal.
    Terminal = 2,
}

impl DealerFacilityPositionPhaseV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Idle),
            1 => Ok(Self::Leased),
            2 => Ok(Self::Terminal),
            _ => Err(Error::InvalidPhase),
        }
    }
}

/// Exact semantic owner of the long-lived Dealer cash and Egg balances.
///
/// This body is deliberately distinct from the legacy user Position codec and
/// commits the full MarketInstanceV2 identity and exact DealerState authority.
/// An adapter still owns token/Hoard custody authentication and must refuse if
/// those observed assets do not equal this accounting body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerFacilityPositionV1 {
    /// Exact authenticated Dealer policy identity.
    pub policy_id: Id,
    /// Canonical facility identity.
    pub facility_id: Id,
    /// Full successor MarketInstanceV2 identity.
    pub market_instance_v2_id: Id,
    /// Exact collateral mint identity.
    pub collateral_mint: Id,
    /// Exact admitted token-program identity.
    pub token_program: Id,
    /// Exact DealerState account key and sole Position authority.
    pub authority_state_account_id: Id,
    /// Exact Replay companion account key.
    pub replay_account_id: Id,
    /// Current custody/lifecycle phase.
    pub phase: DealerFacilityPositionPhaseV1,
    /// Monotone economic generation, equal to DealerState generation while idle.
    pub generation: u64,
    /// Exact collateral cash atoms owned by the Position while idle.
    pub cash_atoms: u64,
    /// Exact existing backed Eggs owned by the Position while idle.
    pub eggs: [u64; crate::MAX_OUTCOMES],
}

impl DealerFacilityPositionV1 {
    /// Validate local identities, phase, and bounded raw asset facts.
    pub fn validate(&self) -> Result<()> {
        let identities = [
            self.policy_id,
            self.facility_id,
            self.market_instance_v2_id,
            self.collateral_mint,
            self.token_program,
            self.authority_state_account_id,
            self.replay_account_id,
        ];
        let mut index = 0usize;
        while index < identities.len() {
            identities[index].validate_live()?;
            index += 1;
        }
        if self.authority_state_account_id == self.replay_account_id
            || self.cash_atoms > crate::MAX_ATOMS
        {
            return Err(Error::InvalidParameter);
        }
        index = 0;
        while index < self.eggs.len() {
            if self.eggs[index] > crate::MAX_ATOMS {
                return Err(Error::InvalidParameter);
            }
            index += 1;
        }
        Ok(())
    }

    /// Compute the exact semantic content identity.
    pub fn position_id(&self) -> Result<Id> {
        self.content_id(crate::DEALER_FACILITY_POSITION_CONTENT_DOMAIN_V1)
    }

    /// Join this exact Position body to its immutable binding and Policy.
    pub fn validate_against(
        &self,
        binding: &FacilityPositionBindingV1,
        policy: &DealerPolicyV1,
    ) -> Result<()> {
        self.validate_live_against(binding, policy)?;
        if self.position_id()? != binding.facility_position_semantic_id {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Join a current mutable Position body to its immutable authority binding.
    ///
    /// Unlike `validate_against`, this does not require the current content ID
    /// to equal the binding's exact generation-zero content ID. Callers must
    /// instead join `position_id()` to the current DealerState owner field.
    pub fn validate_live_against(
        &self,
        binding: &FacilityPositionBindingV1,
        policy: &DealerPolicyV1,
    ) -> Result<()> {
        self.validate()?;
        binding.validate()?;
        policy.validate()?;
        if self.policy_id != binding.policy_id
            || self.facility_id != binding.facility_id
            || self.market_instance_v2_id != binding.market_instance_v2_id
            || self.market_instance_v2_id != policy.market_instance_v2_id
            || self.collateral_mint != policy.collateral_mint
            || self.token_program != policy.token_program
            || self.authority_state_account_id != binding.dealer_state_account_id
            || self.replay_account_id != binding.facility_replay_account_id
        {
            return Err(Error::MismatchedBinding);
        }
        crate::validate_padding_u64(policy.outcome_count, &self.eggs)
    }
}

impl FixedCodec for DealerFacilityPositionV1 {
    const ENCODED_LEN: usize = DEALER_FACILITY_POSITION_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(
            &DEALER_FACILITY_POSITION_MAGIC_V1,
            DEALER_FACILITY_POSITION_VERSION_V1,
        );
        writer.id(self.policy_id);
        writer.id(self.facility_id);
        writer.id(self.market_instance_v2_id);
        writer.id(self.collateral_mint);
        writer.id(self.token_program);
        writer.id(self.authority_state_account_id);
        writer.id(self.replay_account_id);
        writer.u8(self.phase as u8);
        writer.reserved(7);
        writer.u64(self.generation);
        writer.u64(self.cash_atoms);
        let mut index = 0usize;
        while index < self.eggs.len() {
            writer.u64(self.eggs[index]);
            index += 1;
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(
            &DEALER_FACILITY_POSITION_MAGIC_V1,
            DEALER_FACILITY_POSITION_VERSION_V1,
        )?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let market_instance_v2_id = reader.id();
        let collateral_mint = reader.id();
        let token_program = reader.id();
        let authority_state_account_id = reader.id();
        let replay_account_id = reader.id();
        let phase = DealerFacilityPositionPhaseV1::decode(reader.u8())?;
        reader.reserved(7)?;
        let generation = reader.u64();
        let cash_atoms = reader.u64();
        let mut eggs = [0; crate::MAX_OUTCOMES];
        let mut index = 0usize;
        while index < eggs.len() {
            eggs[index] = reader.u64();
            index += 1;
        }
        let value = Self {
            policy_id,
            facility_id,
            market_instance_v2_id,
            collateral_mint,
            token_program,
            authority_state_account_id,
            replay_account_id,
            phase,
            generation,
            cash_atoms,
            eggs,
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Validate the exact initial State/Position authority join.
///
/// This pure check deliberately does not claim account ownership, PDA
/// derivation, signature, token custody, Replay contents, or budget funding.
/// The adapter must establish those facts and then pass their authenticated
/// identities here. No SBF action is enabled by this function.
pub fn validate_facility_initialization_v1(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    state_account_id: Id,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV1,
) -> Result<FacilityPositionBindingIdV1> {
    state_account_id.validate_live()?;
    let binding_id = binding.binding_id_for(genesis, policy)?;
    position.validate_against(binding, policy)?;
    state.validate_against_policy(policy)?;
    let expected_children = DealerChildCountsV1 {
        facility_positions: 1,
        facility_replays: 1,
        lp_pages: 0,
        live_lp_positions: 0,
        unclaimed_lp_positions: 0,
        epoch_bindings: 0,
        leases: 0,
        settlement_pots: 0,
        fee_budgets: 0,
        liveness_budgets: 0,
        resolution_claim_work: 0,
    };
    if state.policy_id != genesis.policy_id
        || state.facility_id != binding.facility_id
        || state.facility_position_id != binding.facility_position_semantic_id
        || state.facility_position_account_id != binding.facility_position_account_id
        || state.facility_replay_account_id != binding.facility_replay_account_id
        || state.sponsor != genesis.sponsor
        || state.sponsor_refund_recipient != genesis.sponsor_refund_recipient
        || state_account_id != binding.dealer_state_account_id
        || state.phase != DealerPhaseV1::Funding
        || state.sponsor_capital_disposition != SponsorCapitalDispositionV1::Refundable
        || state.generation != binding.initial_position_generation
        || state.child_sequence != 0
        || state.total_shares != 0
        || state.queued_shares != 0
        || state.terminal_claimed_shares != 0
        || state.net_sold != [0; crate::MAX_OUTCOMES]
        || state.children != expected_children
        || position.phase != DealerFacilityPositionPhaseV1::Idle
        || position.generation != state.generation
        || position.cash_atoms != state.sponsor_capital_atoms
        || position.eggs != [0; crate::MAX_OUTCOMES]
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(binding_id)
}

/// Validate authoritative V2 initialization with its one counted funded dependency.
///
/// Legacy fee/liveness budget children are not admitted. External runtime
/// accounts and fee records remain owned by their respective runtimes; this
/// root counts only the one rent-owned dependency artifact joining them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_facility_initialization_v2(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV1,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    state_account_id: Id,
    dependency_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    position: &DealerFacilityPositionV1,
    state: &DealerStateV2,
) -> Result<FacilityPositionBindingIdV1> {
    state_account_id.validate_live()?;
    dependency_account_id.validate_live()?;
    let binding_id = binding.binding_id_for(genesis, policy)?;
    dependency.validate_bindings(genesis, binding, policy, schedule, runtime)?;
    position.validate_against(binding, policy)?;
    state.validate_against_policy(policy)?;
    let expected_children = DealerChildCountsV2 {
        facility_positions: 1,
        facility_replays: 1,
        funded_dependencies: 1,
        ..DealerChildCountsV2::default()
    };
    if state.policy_id != genesis.policy_id
        || state.facility_id != binding.facility_id
        || state.facility_position_binding_id != binding_id.untyped()
        || state.facility_position_id != binding.facility_position_semantic_id
        || state.facility_position_account_id != binding.facility_position_account_id
        || state.facility_replay_account_id != binding.facility_replay_account_id
        || state.sponsor != genesis.sponsor
        || state.sponsor_refund_recipient != genesis.sponsor_refund_recipient
        || state.funded_dependencies_id != dependency.dependency_id()?
        || state.funded_dependencies_account_id != dependency_account_id
        || state_account_id != binding.dealer_state_account_id
        || dependency.bindings.asset_vault_authority_account_id != state_account_id
        || state.phase != DealerPhaseV2::Funding
        || state.sponsor_capital_disposition != SponsorCapitalDispositionV1::Refundable
        || state.generation != binding.initial_position_generation
        || state.child_sequence != 0
        || state.total_shares != 0
        || state.queued_shares != 0
        || state.terminal_claimed_shares != 0
        || state.net_sold != [0; crate::MAX_OUTCOMES]
        || state.children != expected_children
        || position.phase != DealerFacilityPositionPhaseV1::Idle
        || position.generation != state.generation
        || position.cash_atoms != state.sponsor_capital_atoms
        || position.eggs != [0; crate::MAX_OUTCOMES]
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(binding_id)
}

/// Validate authoritative V2 initialization against canonical Position V3.
///
/// The legacy Dealer position body is not admitted here. Asset balances,
/// lifecycle, generation, purpose, controller, and Replay are projected from
/// the shared Position V3 semantic owner.
#[allow(clippy::too_many_arguments)]
pub fn validate_facility_initialization_v3(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV2,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    state_account_id: Id,
    dependency_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    position: &DealerPositionObservationV3,
    state: &DealerStateV2,
) -> Result<Id> {
    use clutch_retirement::PositionLifecycleV3;

    state_account_id.validate_live()?;
    dependency_account_id.validate_live()?;
    let binding_id = binding.binding_id_for(genesis, policy)?;
    dependency.validate_bindings_v3(genesis, binding, policy, schedule, runtime)?;
    position.validate_against(binding, binding_id, policy)?;
    state.validate_against_policy(policy)?;
    let canonical = position.projection.position();
    let expected_children = DealerChildCountsV2 {
        facility_positions: 1,
        facility_replays: 1,
        funded_dependencies: 1,
        ..DealerChildCountsV2::default()
    };
    if state.policy_id != policy.policy_id()?
        || state.facility_id != binding.facility_id
        || state.facility_position_binding_id != binding_id
        || state.facility_position_id != position.semantic_id
        || state.facility_position_account_id != position.account_id
        || state.facility_position_account_id != binding.facility_position_account_id
        || state.facility_replay_account_id != binding.facility_replay_account_id
        || Id::from_bytes(canonical.replay_account().bytes())
            != state.facility_replay_account_id
        || state.sponsor != genesis.sponsor
        || state.sponsor_refund_recipient != genesis.sponsor_refund_recipient
        || state.funded_dependencies_id != dependency.dependency_id()?
        || state.funded_dependencies_account_id != dependency_account_id
        || state_account_id != binding.dealer_state_account_id
        || dependency.bindings.asset_vault_authority_account_id != state_account_id
        || dependency.facility_position_binding_id != binding_id
        || state.phase != DealerPhaseV2::Funding
        || state.sponsor_capital_disposition != SponsorCapitalDispositionV1::Refundable
        || state.generation != binding.initial_position_generation
        || canonical.generation() != state.generation
        || canonical.lifecycle() != PositionLifecycleV3::Open
        || canonical.cash_atoms() != state.sponsor_capital_atoms
        || canonical.reserved_cash_atoms() != 0
        || canonical.native_eggs() != [0; crate::MAX_OUTCOMES]
        || canonical.outstanding_reservations() != 0
        || state.child_sequence != 0
        || state.total_shares != 0
        || state.queued_shares != 0
        || state.terminal_claimed_shares != 0
        || state.net_sold != [0; crate::MAX_OUTCOMES]
        || state.children != expected_children
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(binding_id)
}

/// Exact atomic initialization bundle over StateV2, PositionV3, ReplayV3,
/// funded dependencies, sponsor transfer, and liveness receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedDealerInitializationV3 {
    /// Canonical initialized State body supplied to the exact validator.
    pub state: DealerStateV2,
    /// Sponsor-to-facility PositionV3 transfer.
    pub transfer: PreparedDealerPositionPairTransferV1,
    /// First accepted Replay intent, advancing the founding ordinal to one.
    pub replay: PreparedDealerReplayTransitionV1,
}

/// Prepare the first real facility transition without inventing a State preimage.
///
/// Initialize alone commits a zero `state_pre_content_id`; every later action
/// requires a live exact State preimage. The post-State, both Position semantic
/// IDs, sponsor transfer, funded receipt, and founding Replay are still bound
/// atomically by the first intent.
#[allow(clippy::too_many_arguments)]
pub fn prepare_facility_initialization_v3(
    genesis: &DealerFacilityGenesisV1,
    binding: &FacilityPositionBindingV2,
    policy: &DealerPolicyV1,
    schedule: &DealerLivenessScheduleV1,
    runtime: &DealerRuntimeLivenessBindingV1,
    state_account_id: Id,
    dependency_account_id: Id,
    dependency: &DealerFundedDependenciesV2,
    authorization: &DealerActionLivenessAuthorizationV1,
    position: &DealerPositionObservationV3,
    state: &DealerStateV2,
    transfer: PreparedDealerPositionPairTransferV1,
    replay: &DealerFacilityReplayV1,
    replay_binding: DealerReplayAccountBindingV1,
) -> Result<PreparedDealerInitializationV3> {
    validate_facility_initialization_v3(
        genesis,
        binding,
        policy,
        schedule,
        runtime,
        state_account_id,
        dependency_account_id,
        dependency,
        position,
        state,
    )?;
    authorization.validate_against(schedule, runtime)?;
    replay.validate()?;
    let bundle = transfer.bundle();
    bundle.validate()?;
    if authorization.action != DealerRuntimeActionV1::Initialize
        || authorization.owner != state_account_id
        || authorization.lifecycle_id != state.facility_id
        || authorization.facility_generation != state.generation
        || bundle.action != DealerRuntimeActionV1::Initialize
        || bundle.source_kind != DealerAssetEndpointKindV1::GeneralPosition
        || bundle.destination_kind != DealerAssetEndpointKindV1::FacilityPosition
        || bundle.destination_account_id != state.facility_position_account_id
        || bundle.destination_post_semantic_id != state.facility_position_id
        || bundle.amounts.cash_atoms != state.sponsor_capital_atoms
        || bundle.amounts.native_eggs != [0; crate::MAX_OUTCOMES]
        || replay.facility_position_account_id() != state.facility_position_account_id
        || replay.replay_account_id() != state.facility_replay_account_id
        || replay.facility_position_binding_id() != state.facility_position_binding_id
        || replay.position_generation() != state.generation
        || replay.next_transition_ordinal()
            != crate::DEALER_FACILITY_REPLAY_FOUNDING_ORDINAL_V1
        || !replay.last_transition_intent_id().is_zero()
    {
        return Err(Error::MismatchedBinding);
    }
    let prepared = replay.prepare_transition(
        replay_binding,
        DealerTransitionIntentV1 {
            replay_account_id: replay.replay_account_id(),
            replay_pre_id: replay.replay_id()?,
            state_pre_content_id: Id::ZERO,
            state_post_content_id: state.state_content_id()?,
            position_pre_semantic_id: bundle.destination_pre_semantic_id,
            position_post_semantic_id: bundle.destination_post_semantic_id,
            liveness_receipt_semantic_id: authorization.receipt_semantic_id,
            fee_receipt_semantic_id: Id::ZERO,
            asset_transfer_bundle_id: bundle.bundle_id()?,
            position_generation_before: state.generation,
            position_generation_after: state.generation,
            expected_ordinal: crate::DEALER_FACILITY_REPLAY_FOUNDING_ORDINAL_V1,
            action: DealerRuntimeActionV1::Initialize,
            liveness_mode: DealerTransitionLivenessModeV1::ExternalReceipt,
        },
    )?;
    Ok(PreparedDealerInitializationV3 {
        state: *state,
        transfer,
        replay: prepared,
    })
}

const _: () = assert!(DEALER_FACILITY_GENESIS_BYTES_V1 == 116);
const _: () = assert!(FACILITY_POSITION_BINDING_BYTES_V1 == 244);
const _: () = assert!(DEALER_FACILITY_POSITION_BYTES_V1 == 388);
const _: () = assert!(DEALER_FACILITY_GENESIS_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(FACILITY_POSITION_BINDING_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
const _: () = assert!(DEALER_FACILITY_POSITION_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Id {
        Id::from_bytes([byte; 32])
    }

    fn policy() -> DealerPolicyV1 {
        let mut unit_eggs = [0; crate::MAX_OUTCOMES];
        unit_eggs[0] = 10;
        unit_eggs[1] = 10;
        let mut weights = [0; crate::MAX_OUTCOMES];
        weights[0] = 1;
        weights[1] = 1;
        let mut buy = [0; crate::MAX_OUTCOMES];
        buy[0] = 100;
        buy[1] = 100;
        let mut sell = [0; crate::MAX_OUTCOMES];
        sell[0] = 100;
        sell[1] = 100;
        DealerPolicyV1 {
            realm_id: id(1),
            profile_id: id(2),
            market_instance_v2_id: id(3),
            claim_basis_id: id(4),
            collateral_mint: id(5),
            token_program: id(6),
            hoard_custody_semantics_id: id(7),
            relation_v2_id: id(8),
            price_measure_policy_id: id(9),
            curve_policy_id: id(10),
            curve_price_certificate_policy_id: id(11),
            fee_policy_id: id(12),
            liveness_policy_id: id(13),
            retirement_policy_id: id(14),
            neutral_sink: id(72),
            quote_authority: id(15),
            outcome_count: 2,
            payout_denominator: 10,
            capital_unit_cash_atoms: 10,
            capital_unit_eggs: unit_eggs,
            initial_price_denominator: 2,
            initial_price_weights: weights,
            depth_atoms: 1_000,
            max_net_buy: buy,
            max_net_sell: sell,
            minimum_lp_shares: 10,
            maximum_lp_shares: 100,
            funding_deadline_slot: 100,
            trading_open_slot: 100,
            trading_close_slot: 1_000,
            maturity_slot: 2_000,
            shutdown_queue_numerator: 1,
            shutdown_queue_denominator: 2,
            maximum_lp_pages: 4,
        }
    }

    fn genesis(policy: &DealerPolicyV1) -> DealerFacilityGenesisV1 {
        DealerFacilityGenesisV1 {
            policy_id: policy.policy_id().unwrap(),
            sponsor: id(20),
            sponsor_refund_recipient: id(21),
            facility_nonce: 7,
        }
    }

    fn binding(
        genesis: &DealerFacilityGenesisV1,
        policy: &DealerPolicyV1,
    ) -> FacilityPositionBindingV1 {
        FacilityPositionBindingV1 {
            facility_id: genesis.facility_id().unwrap().untyped(),
            policy_id: genesis.policy_id,
            market_instance_v2_id: policy.market_instance_v2_id,
            facility_position_semantic_id: id(31),
            facility_position_account_id: id(32),
            facility_replay_account_id: id(33),
            dealer_state_account_id: id(34),
            initial_position_generation: 0,
        }
    }

    #[test]
    fn facility_and_position_binding_are_canonical_and_hostile() {
        let policy = policy();
        let genesis = genesis(&policy);
        let facility_id = genesis.facility_id_for_policy(&policy).unwrap();
        let mut genesis_bytes = [0; DEALER_FACILITY_GENESIS_BYTES_V1];
        genesis.encode_into(&mut genesis_bytes).unwrap();
        assert_eq!(DealerFacilityGenesisV1::decode(&genesis_bytes), Ok(genesis));
        assert_eq!(
            DealerFacilityGenesisV1::decode(&genesis_bytes)
                .unwrap()
                .facility_id()
                .unwrap(),
            facility_id
        );

        let binding = binding(&genesis, &policy);
        let binding_id = binding.binding_id_for(&genesis, &policy).unwrap();
        let mut binding_bytes = [0; FACILITY_POSITION_BINDING_BYTES_V1];
        binding.encode_into(&mut binding_bytes).unwrap();
        assert_eq!(
            FacilityPositionBindingV1::decode(&binding_bytes),
            Ok(binding)
        );
        assert_eq!(binding.binding_id().unwrap(), binding_id);

        let mut swapped = binding;
        swapped.facility_position_account_id = swapped.facility_replay_account_id;
        assert_eq!(swapped.validate(), Err(Error::InvalidParameter));
        let mut wrong_market = binding;
        wrong_market.market_instance_v2_id = id(99);
        assert_eq!(
            wrong_market.binding_id_for(&genesis, &policy),
            Err(Error::MismatchedBinding)
        );
        let mut wrong_facility = binding;
        wrong_facility.facility_id = id(98);
        assert_eq!(
            wrong_facility.binding_id_for(&genesis, &policy),
            Err(Error::MismatchedBinding)
        );
        let mut wrong_generation = binding;
        wrong_generation.initial_position_generation = 1;
        assert_eq!(wrong_generation.validate(), Err(Error::InvalidParameter));

        for offset in [0, 8, 10] {
            let mut hostile = binding_bytes;
            hostile[offset] ^= 0xff;
            assert!(FacilityPositionBindingV1::decode(&hostile).is_err());
        }
        assert!(
            FacilityPositionBindingV1::decode(&binding_bytes[..binding_bytes.len() - 1]).is_err()
        );
    }
}
