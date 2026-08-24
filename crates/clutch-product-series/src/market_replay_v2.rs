//! Current persistent Market generation and lifecycle replay owner.
//!
//! Unlike the historical closure-only replay receipt, this owner exists before
//! `MarketLifecycleRootV2`.  Its market-only coordinate is the sole durable
//! source of the nonzero Market generation.  The same body is advanced through
//! foundation settlement, root activation, and whole-Market terminal replay;
//! callers cannot supply a generation or replace it with a Series ordinal.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ContentId, Error, FixedCodec, MarketFoundationAccountGraphV3Id,
    MarketFoundationScheduleV3Id, MarketInstanceTerminalProjectionV2,
    MarketInstanceV2Id, MarketLifecycleBindingV2, MarketLifecycleGenerationBindingV2Id,
    MarketLifecyclePhaseV2, MarketLifecycleReplayV2Id, MarketLifecycleRootV2,
    RegistryCapabilityProfileV4Id, RegistryProgramReleaseV2Id, Result,
};

const MARKET_LIFECYCLE_REPLAY_MAGIC_V2: [u8; 8] = *b"DCMLRPV2";
const MARKET_LIFECYCLE_REPLAY_SCHEMA_V2: u16 = 2;

/// Exact semantic width of the current persistent ProductReplayAnchor.
pub const MARKET_LIFECYCLE_REPLAY_BYTES_V2: usize = 544;
/// Immutable generation-binding identity domain.
pub const MARKET_LIFECYCLE_GENERATION_BINDING_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-lifecycle-generation-binding/v2";
/// Deterministic nonzero initial-generation derivation domain.
pub const MARKET_LIFECYCLE_INITIAL_GENERATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-lifecycle-initial-generation/v2";
/// Complete current replay-state identity domain.
pub const MARKET_LIFECYCLE_REPLAY_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-lifecycle-replay/v2";

const _: () = assert!(MARKET_LIFECYCLE_REPLAY_BYTES_V2 == 16 + 8 * 32 + 8 + 8 * 32 + 8);

/// Exhaustive lifecycle of the permanent current ProductReplayAnchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketLifecycleReplayPhaseV2 {
    /// Physically initialized before RootV2; slot-13 principal is not settled yet.
    Founding,
    /// Slot-13 principal was consumed exactly once by the foundation cursor.
    FoundationSettled,
    /// The exact RootV2 binding and activation receipt were recorded.
    Active,
    /// The exact terminal RootV2 projection was durably recorded.
    Terminal,
}

impl MarketLifecycleReplayPhaseV2 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::Founding => 1,
            Self::FoundationSettled => 2,
            Self::Active => 3,
            Self::Terminal => 4,
        }
    }

    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Founding),
            2 => Ok(Self::FoundationSettled),
            3 => Ok(Self::Active),
            4 => Ok(Self::Terminal),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Immutable facts selecting one exact initial Market generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLifecycleGenerationBindingV2 {
    /// Canonical market-only ProductReplayAnchor account.
    pub replay_account_id: ContentId,
    /// Full-width economic Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Hostile-authenticated immutable family capability policy.
    pub market_family_capability_policy_id: ContentId,
    /// Loader-authenticated current central Registry release.
    pub registry_release_id: RegistryProgramReleaseV2Id,
    /// Exact current Registry capability profile.
    pub capability_profile_id: RegistryCapabilityProfileV4Id,
    /// Exact 47-slot foundation schedule.
    pub foundation_schedule_id: MarketFoundationScheduleV3Id,
    /// Exact canonical 47-slot physical graph.
    pub foundation_account_graph_id: MarketFoundationAccountGraphV3Id,
    /// Canonical future RootV2 coordinate.
    pub lifecycle_root_account_id: ContentId,
    /// Deterministically derived nonzero generation.
    pub generation: u64,
}

impl MarketLifecycleGenerationBindingV2 {
    /// Validate that no caller-selected generation can enter the binding.
    pub fn validate(self) -> Result<()> {
        self.market_instance_id.validate()?;
        self.registry_release_id.validate()?;
        self.capability_profile_id.validate()?;
        self.foundation_schedule_id.validate()?;
        self.foundation_account_graph_id.validate()?;
        for id in [
            self.replay_account_id,
            self.market_family_capability_policy_id,
            self.lifecycle_root_account_id,
        ] {
            id.validate()?;
        }
        if self.generation
            != derive_initial_market_generation_v2(
                self.market_instance_id,
                self.market_family_capability_policy_id,
                self.registry_release_id,
                self.capability_profile_id,
            )?
        {
            return Err(Error::MismatchedArtifact);
        }
        let ids = self.ids();
        let mut left = 0usize;
        while left < ids.len() {
            let mut right = left + 1;
            while right < ids.len() {
                if ids[left] == ids[right] {
                    return Err(Error::MismatchedArtifact);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }

    /// Domain-separated immutable binding identity.
    pub fn id(self) -> Result<MarketLifecycleGenerationBindingV2Id> {
        self.validate()?;
        let mut body = [0u8; 264];
        let mut writer = Writer::new(&mut body, 264)?;
        for id in self.ids() {
            writer.id(id);
        }
        writer.u64(self.generation);
        writer.finish()?;
        Ok(MarketLifecycleGenerationBindingV2Id::from_bytes(
            content_id(MARKET_LIFECYCLE_GENERATION_BINDING_DOMAIN_V2, &body).bytes(),
        ))
    }

    fn ids(self) -> [ContentId; 8] {
        [
            self.replay_account_id,
            self.market_instance_id.content_id(),
            self.market_family_capability_policy_id,
            self.registry_release_id.content_id(),
            self.capability_profile_id.content_id(),
            self.foundation_schedule_id.content_id(),
            self.foundation_account_graph_id.content_id(),
            self.lifecycle_root_account_id,
        ]
    }
}

/// Derive the only permitted first generation from immutable current owners.
///
/// Setting the high bit makes zero unrepresentable without a special-case
/// literal or caller fallback.  The complete digest remains committed by the
/// generation binding ID.
pub fn derive_initial_market_generation_v2(
    market_instance_id: MarketInstanceV2Id,
    market_family_capability_policy_id: ContentId,
    registry_release_id: RegistryProgramReleaseV2Id,
    capability_profile_id: RegistryCapabilityProfileV4Id,
) -> Result<u64> {
    market_instance_id.validate()?;
    market_family_capability_policy_id.validate()?;
    registry_release_id.validate()?;
    capability_profile_id.validate()?;
    let mut body = [0u8; 128];
    body[..32].copy_from_slice(&market_instance_id.bytes());
    body[32..64].copy_from_slice(&market_family_capability_policy_id.bytes());
    body[64..96].copy_from_slice(&registry_release_id.bytes());
    body[96..128].copy_from_slice(&capability_profile_id.bytes());
    let digest = content_id(MARKET_LIFECYCLE_INITIAL_GENERATION_DOMAIN_V2, &body).bytes();
    let mut generation_bytes = [0u8; 8];
    generation_bytes.copy_from_slice(&digest[..8]);
    generation_bytes[7] |= 0x80;
    Ok(u64::from_le_bytes(generation_bytes))
}

/// Default-refusing pure initialization authority.
pub trait AuthenticatedMarketLifecycleGenerationAuthorityV2 {
    /// Authenticate the exact physical bootstrap receipt and immutable binding.
    fn authenticate_market_lifecycle_generation_v2(
        &self,
        _binding: MarketLifecycleGenerationBindingV2,
        _bootstrap_authority_id: ContentId,
        _bootstrap_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Default-refusing physical slot-13 settlement authority.
pub trait AuthenticatedMarketLifecycleReplayFoundationAuthorityV2 {
    /// Authenticate the once-only principal settlement against the live body.
    fn authenticate_market_lifecycle_replay_foundation_v2(
        &self,
        _state: &MarketLifecycleReplayV2,
        _foundation_settlement_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Default-refusing RootV2 activation authority.
pub trait AuthenticatedMarketLifecycleReplayActivationAuthorityV2 {
    /// Authenticate the exact hostile RootV2 postwrite.
    fn authenticate_market_lifecycle_replay_activation_v2(
        &self,
        _state: &MarketLifecycleReplayV2,
        _root: &MarketLifecycleRootV2,
        _root_activation_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Default-refusing whole-Market terminal authority.
pub trait AuthenticatedMarketLifecycleReplayTerminalAuthorityV2 {
    /// Authenticate the terminal RootV2 postwrite before replay persistence.
    fn authenticate_market_lifecycle_replay_terminal_v2(
        &self,
        _state: &MarketLifecycleReplayV2,
        _root: &MarketLifecycleRootV2,
        _terminal: MarketInstanceTerminalProjectionV2,
        _terminal_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Persistent generation/replay semantic owner stored at ProductReplayAnchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLifecycleReplayV2 {
    binding: MarketLifecycleGenerationBindingV2,
    phase: MarketLifecycleReplayPhaseV2,
    transition_sequence: u64,
    bootstrap_authority_id: ContentId,
    bootstrap_receipt_id: ContentId,
    foundation_settlement_receipt_id: ContentId,
    root_binding_id: ContentId,
    root_activation_receipt_id: ContentId,
    terminal_root_semantic_id: ContentId,
    terminal_projection_id: ContentId,
    last_transition_receipt_id: ContentId,
}

impl MarketLifecycleReplayV2 {
    /// Initialize the sole durable generation before RootV2 exists.
    pub fn initialize<A: AuthenticatedMarketLifecycleGenerationAuthorityV2 + ?Sized>(
        authority: &A,
        binding: MarketLifecycleGenerationBindingV2,
        bootstrap_authority_id: ContentId,
        bootstrap_receipt_id: ContentId,
    ) -> Result<Self> {
        binding.validate()?;
        bootstrap_authority_id.validate()?;
        bootstrap_receipt_id.validate()?;
        if bootstrap_authority_id == bootstrap_receipt_id {
            return Err(Error::MismatchedArtifact);
        }
        authority.authenticate_market_lifecycle_generation_v2(
            binding,
            bootstrap_authority_id,
            bootstrap_receipt_id,
        )?;
        let value = Self {
            binding,
            phase: MarketLifecycleReplayPhaseV2::Founding,
            transition_sequence: 0,
            bootstrap_authority_id,
            bootstrap_receipt_id,
            foundation_settlement_receipt_id: ContentId::ZERO,
            root_binding_id: ContentId::ZERO,
            root_activation_receipt_id: ContentId::ZERO,
            terminal_root_semantic_id: ContentId::ZERO,
            terminal_projection_id: ContentId::ZERO,
            last_transition_receipt_id: bootstrap_receipt_id,
        };
        value.validate()?;
        Ok(value)
    }

    /// Persist the exact slot-13 principal settlement once.
    pub fn settle_foundation<
        A: AuthenticatedMarketLifecycleReplayFoundationAuthorityV2 + ?Sized,
    >(
        self,
        authority: &A,
        foundation_settlement_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        foundation_settlement_receipt_id.validate()?;
        if self.phase != MarketLifecycleReplayPhaseV2::Founding
            || foundation_settlement_receipt_id == self.bootstrap_receipt_id
        {
            return Err(Error::WorkStateMismatch);
        }
        authority.authenticate_market_lifecycle_replay_foundation_v2(
            &self,
            foundation_settlement_receipt_id,
        )?;
        let value = Self {
            phase: MarketLifecycleReplayPhaseV2::FoundationSettled,
            transition_sequence: 1,
            foundation_settlement_receipt_id,
            last_transition_receipt_id: foundation_settlement_receipt_id,
            ..self
        };
        value.validate()?;
        Ok(value)
    }

    /// Bind the exact active RootV2 after the foundation cursor completes.
    pub fn activate<A: AuthenticatedMarketLifecycleReplayActivationAuthorityV2 + ?Sized>(
        self,
        authority: &A,
        root: &MarketLifecycleRootV2,
        root_activation_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        root_activation_receipt_id.validate()?;
        let root_binding = root.binding_ref();
        let root_binding_id = root_binding.id()?;
        if self.phase != MarketLifecycleReplayPhaseV2::FoundationSettled
            || root.phase() != MarketLifecyclePhaseV2::Active
            || root_binding.market_instance_id != self.binding.market_instance_id
            || root_binding.generation != self.binding.generation
            || root_binding.registry_release_id
                != self.binding.registry_release_id.content_id()
            || root_binding.capability_profile_id
                != self.binding.capability_profile_id.content_id()
            || root_binding.foundation_schedule_id != self.binding.foundation_schedule_id
            || root_binding.foundation_account_graph_id
                != self.binding.foundation_account_graph_id
        {
            return Err(Error::MismatchedArtifact);
        }
        authority.authenticate_market_lifecycle_replay_activation_v2(
            &self,
            root,
            root_activation_receipt_id,
        )?;
        let value = Self {
            phase: MarketLifecycleReplayPhaseV2::Active,
            transition_sequence: 2,
            root_binding_id,
            root_activation_receipt_id,
            last_transition_receipt_id: root_activation_receipt_id,
            ..self
        };
        value.validate()?;
        Ok(value)
    }

    /// Persist the exact current whole-Market terminal postimage.
    pub fn terminalize<A: AuthenticatedMarketLifecycleReplayTerminalAuthorityV2 + ?Sized>(
        self,
        authority: &A,
        root: &MarketLifecycleRootV2,
        terminal: MarketInstanceTerminalProjectionV2,
        terminal_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        terminal_receipt_id.validate()?;
        let root_binding = root.binding_ref();
        if self.phase != MarketLifecycleReplayPhaseV2::Active
            || root.phase() != MarketLifecyclePhaseV2::Terminal
            || root_binding.id()? != self.root_binding_id
            || terminal.id() != terminal_receipt_id
            || terminal.root_semantic_id() != root.semantic_id()?
            || terminal.market_instance_id() != self.binding.market_instance_id
            || terminal.generation() != self.binding.generation
            || terminal.final_transition_sequence() != root.transition_sequence()
        {
            return Err(Error::MismatchedArtifact);
        }
        authority.authenticate_market_lifecycle_replay_terminal_v2(
            &self,
            root,
            terminal,
            terminal_receipt_id,
        )?;
        let value = Self {
            phase: MarketLifecycleReplayPhaseV2::Terminal,
            transition_sequence: 3,
            terminal_root_semantic_id: terminal.root_semantic_id(),
            terminal_projection_id: terminal.id(),
            last_transition_receipt_id: terminal_receipt_id,
            ..self
        };
        value.validate()?;
        Ok(value)
    }

    /// Complete immutable generation binding.
    pub const fn binding(&self) -> MarketLifecycleGenerationBindingV2 {
        self.binding
    }
    /// Exact nonzero generation; no other current owner may synthesize it.
    pub const fn generation(&self) -> u64 {
        self.binding.generation
    }
    /// Current replay phase.
    pub const fn phase(&self) -> MarketLifecycleReplayPhaseV2 {
        self.phase
    }
    /// Monotone replay sequence.
    pub const fn transition_sequence(&self) -> u64 {
        self.transition_sequence
    }
    /// Exact slot-13 settlement receipt, zero before the cursor consumes it.
    pub const fn foundation_settlement_receipt_id(&self) -> ContentId {
        self.foundation_settlement_receipt_id
    }
    /// Exact active RootV2 binding, zero before activation.
    pub const fn root_binding_id(&self) -> ContentId {
        self.root_binding_id
    }
    /// Exact terminal projection, zero before terminal replay.
    pub const fn terminal_projection_id(&self) -> ContentId {
        self.terminal_projection_id
    }

    /// Domain-separated identity of the complete hostile-encoded state.
    pub fn id(&self) -> Result<MarketLifecycleReplayV2Id> {
        let mut body = [0u8; MARKET_LIFECYCLE_REPLAY_BYTES_V2];
        self.encode_into(&mut body)?;
        Ok(MarketLifecycleReplayV2Id::from_bytes(
            content_id(MARKET_LIFECYCLE_REPLAY_DOMAIN_V2, &body).bytes(),
        ))
    }

    fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        self.bootstrap_authority_id.validate()?;
        self.bootstrap_receipt_id.validate()?;
        self.last_transition_receipt_id.validate()?;
        if self.bootstrap_authority_id == self.bootstrap_receipt_id {
            return Err(Error::MismatchedArtifact);
        }
        let settlement_live = !self.foundation_settlement_receipt_id.is_zero();
        let root_live = !self.root_binding_id.is_zero() && !self.root_activation_receipt_id.is_zero();
        let terminal_live = !self.terminal_root_semantic_id.is_zero()
            && !self.terminal_projection_id.is_zero();
        let valid_phase = match self.phase {
            MarketLifecycleReplayPhaseV2::Founding => {
                self.transition_sequence == 0
                    && !settlement_live
                    && !root_live
                    && !terminal_live
                    && self.last_transition_receipt_id == self.bootstrap_receipt_id
            }
            MarketLifecycleReplayPhaseV2::FoundationSettled => {
                self.transition_sequence == 1
                    && settlement_live
                    && !root_live
                    && !terminal_live
                    && self.last_transition_receipt_id
                        == self.foundation_settlement_receipt_id
            }
            MarketLifecycleReplayPhaseV2::Active => {
                self.transition_sequence == 2
                    && settlement_live
                    && root_live
                    && !terminal_live
                    && self.last_transition_receipt_id == self.root_activation_receipt_id
            }
            MarketLifecycleReplayPhaseV2::Terminal => {
                self.transition_sequence == 3
                    && settlement_live
                    && root_live
                    && terminal_live
                    && self.last_transition_receipt_id == self.terminal_projection_id
            }
        };
        if !valid_phase {
            return Err(Error::WorkStateMismatch);
        }
        Ok(())
    }
}

impl FixedCodec for MarketLifecycleReplayV2 {
    const ENCODED_LEN: usize = MARKET_LIFECYCLE_REPLAY_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_LIFECYCLE_REPLAY_MAGIC_V2);
        writer.u16(MARKET_LIFECYCLE_REPLAY_SCHEMA_V2);
        writer.u8(self.phase.wire_byte());
        writer.reserved(5);
        for id in self.binding.ids() {
            writer.id(id);
        }
        writer.u64(self.binding.generation);
        for id in [
            self.bootstrap_authority_id,
            self.bootstrap_receipt_id,
            self.foundation_settlement_receipt_id,
            self.root_binding_id,
            self.root_activation_receipt_id,
            self.terminal_root_semantic_id,
            self.terminal_projection_id,
            self.last_transition_receipt_id,
        ] {
            writer.id(id);
        }
        writer.u64(self.transition_sequence);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_LIFECYCLE_REPLAY_MAGIC_V2)?;
        if reader.u16() != MARKET_LIFECYCLE_REPLAY_SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        let phase = MarketLifecycleReplayPhaseV2::decode(reader.u8())?;
        reader.reserved(5)?;
        let binding_ids = [
            reader.id(), reader.id(), reader.id(), reader.id(),
            reader.id(), reader.id(), reader.id(), reader.id(),
        ];
        let binding = MarketLifecycleGenerationBindingV2 {
            replay_account_id: binding_ids[0],
            market_instance_id: MarketInstanceV2Id::from_bytes(binding_ids[1].bytes()),
            market_family_capability_policy_id: binding_ids[2],
            registry_release_id: RegistryProgramReleaseV2Id::from_bytes(binding_ids[3].bytes()),
            capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes(
                binding_ids[4].bytes(),
            ),
            foundation_schedule_id: MarketFoundationScheduleV3Id::from_bytes(
                binding_ids[5].bytes(),
            ),
            foundation_account_graph_id: MarketFoundationAccountGraphV3Id::from_bytes(
                binding_ids[6].bytes(),
            ),
            lifecycle_root_account_id: binding_ids[7],
            generation: reader.u64(),
        };
        let value = Self {
            binding,
            phase,
            bootstrap_authority_id: reader.id(),
            bootstrap_receipt_id: reader.id(),
            foundation_settlement_receipt_id: reader.id(),
            root_binding_id: reader.id(),
            root_activation_receipt_id: reader.id(),
            terminal_root_semantic_id: reader.id(),
            terminal_projection_id: reader.id(),
            last_transition_receipt_id: reader.id(),
            transition_sequence: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}
