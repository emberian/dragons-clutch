//! Construction-only transactions for the current protocol workstreams.
//!
//! Semantic crates own payload bytes and account ordering. This module owns
//! only the outer Solana instruction/transaction boundary, explicit release
//! binding, signer declaration, exact-integer balance equations, and grouping
//! of independently owned actions into an atomic unsigned transaction. It has
//! no keypair, blockhash, RPC, signing, or submission dependency.

use clutch_solana_layout::registry::{
    AllocationStatus, ExtensionAction, ExtensionEnvelope, ExtensionFamily, RegistryError,
    EXTENSION_ENVELOPE_BYTES, MAX_EXTENSION_PAYLOAD_BYTES,
};
use clutch_solana_layout::source_series::{validate_account_metas_v2, ObservedSourceAccountMetaV2};
use clutch_solana_layout::Intent;
use clutch_solana_reference::{Action, ExtensionRequest, Request};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_transaction::Transaction;
use std::collections::BTreeSet;

pub type Result<T> = std::result::Result<T, ConstructionError>;

/// Construction artifact schema. This is not a release or execution receipt.
pub const CONSTRUCTION_PLAN_SCHEMA: &str =
    "dragons-clutch/operator/unsigned-protocol-transaction/v3";
/// Runtime liveness intent magic owned by the liveness adapter codec.
pub const LIVENESS_RUNTIME_V1_INTENT_MAGIC: [u8; 8] = *b"DCLINT01";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionError {
    ZeroIdentity,
    ZeroReleaseDigest,
    EmptyActionName,
    EmptyInstructionData,
    DuplicateAccount,
    InvalidAccountContract,
    DuplicateSigner,
    MissingSignerMeta,
    ForeignProgram,
    WrongFlow,
    WrongWirePrefix,
    WrongWireLength,
    PayloadTooLong,
    UnallocatedRegistryCoordinate,
    MissingExactEquation,
    UnbalancedExactEquation,
    EmptyBundle,
    PacketTooLarge,
    Serialization,
}

impl core::fmt::Display for ConstructionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ZeroIdentity => "construction plan contains a zero identity",
            Self::ZeroReleaseDigest => "construction plan is not bound to a release digest",
            Self::EmptyActionName => "construction action name is empty",
            Self::EmptyInstructionData => "construction instruction data is empty",
            Self::DuplicateAccount => "construction instruction aliases an account role",
            Self::InvalidAccountContract => {
                "construction instruction violates its exact account-role contract"
            }
            Self::DuplicateSigner => "construction signer list contains a duplicate",
            Self::MissingSignerMeta => "required signer is neither the payer nor a signer meta",
            Self::ForeignProgram => "instruction program differs from the bound release",
            Self::WrongFlow => "instruction belongs to the wrong protocol flow",
            Self::WrongWirePrefix => "instruction bytes do not match their owned wire contract",
            Self::WrongWireLength => "instruction bytes do not have the exact owned width",
            Self::PayloadTooLong => "successor payload exceeds the central intent ceiling",
            Self::UnallocatedRegistryCoordinate => {
                "successor coordinate is not allocated by the central registry"
            }
            Self::MissingExactEquation => "instruction omits exact-integer accounting",
            Self::UnbalancedExactEquation => "exact-integer accounting equation is unbalanced",
            Self::EmptyBundle => "atomic transaction bundle is empty",
            Self::PacketTooLarge => {
                "serialized unsigned transaction exceeds its explicit packet limit"
            }
            Self::Serialization => "unsigned transaction serialization failed",
        })
    }
}

impl std::error::Error for ConstructionError {}

/// Current implementation lanes. Classification does not imply that the SBF
/// dispatcher has enabled a corresponding capability.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProtocolFlow {
    CollateralCustodyV3,
    MarketEpochCreation,
    SourcePlaneV3,
    GeneralV2Candidate,
    GeneralV2Settlement,
    GeneralV2Fees,
    /// Current Direct `80/1` successor family; every action remains disabled.
    DirectMarketV1,
    DirectEggSettlement,
    Liveness,
    ProductSeries,
    StructuredClaim,
    KeeperSettlement,
    RecoveryRetirement,
}

/// Runtime status carried into every construction artifact.
///
/// Admission is assigned by closed constructors, never accepted as a caller
/// argument. It still is not an execution receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAdmission {
    /// Dispatcher capability is known to be disabled. Bytes are useful only
    /// for integration work and must not be represented as executable.
    ReservedDisabled,
    /// The exact SourceSeries coordinate is enabled by the release-bound
    /// Clutch dispatcher and encoded in its strict replay request. This says
    /// nothing about signing, submission, or successful onchain execution.
    ReleaseBoundEnabled,
}

/// Central-registry provenance for a main-program successor envelope.
///
/// `central_action` distinguishes a centrally allocated local action from a
/// family whose semantic owner still owns an unallocated local-action codec.
/// Family allocation and runtime admission remain deliberately separate facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuccessorRegistryBinding {
    pub family: ExtensionFamily,
    pub local_action: u8,
    pub family_status: AllocationStatus,
    pub central_action: Option<ExtensionAction>,
}

/// The semantic package and reviewed digest that owned instruction bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOwner {
    pub package: String,
    pub schema: String,
    pub release_sha256: [u8; 32],
}

impl SemanticOwner {
    pub fn validate(&self) -> Result<()> {
        if self.package.trim().is_empty() || self.schema.trim().is_empty() {
            return Err(ConstructionError::EmptyActionName);
        }
        if self.release_sha256 == [0; 32] {
            return Err(ConstructionError::ZeroReleaseDigest);
        }
        Ok(())
    }
}

/// Exact integer units kept separate across cash, Eggs, fees, rent, liveness,
/// Series funding, and structured-claim backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerUnit {
    Lamports,
    CollateralAtoms { mint: Address },
    PriceUnits { scale: u64 },
    EggAtoms { market: [u8; 32], outcome: u8 },
    FeeAtoms { mint: Address },
    WrapperAtoms { mint: Address },
}

impl IntegerUnit {
    fn validate(self) -> Result<()> {
        match self {
            Self::Lamports => Ok(()),
            Self::CollateralAtoms { mint }
            | Self::FeeAtoms { mint }
            | Self::WrapperAtoms { mint } => {
                if mint == Address::default() {
                    Err(ConstructionError::ZeroIdentity)
                } else {
                    Ok(())
                }
            }
            Self::PriceUnits { scale } => {
                if scale == 0 {
                    Err(ConstructionError::ZeroIdentity)
                } else {
                    Ok(())
                }
            }
            Self::EggAtoms { market, .. } => {
                if market == [0; 32] {
                    Err(ConstructionError::ZeroIdentity)
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// One named `left == right` equation over a single exact integer unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEquation {
    pub name: String,
    pub unit: IntegerUnit,
    pub left: u128,
    pub right: u128,
}

impl ExactEquation {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(ConstructionError::EmptyActionName);
        }
        self.unit.validate()?;
        if self.left != self.right {
            return Err(ConstructionError::UnbalancedExactEquation);
        }
        Ok(())
    }
}

/// How the semantic owner encoded the instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedWireContract {
    /// Exact liveness runtime intent, including `DCLINT01`.
    LivenessRuntimeV1 { exact_bytes: usize },
    /// Main-program successor envelope. The central registry owns family tag
    /// and version; the named semantic package owns action and payload bytes.
    MainSuccessor { binding: SuccessorRegistryBinding },
    /// Exact outer Clutch replay request carrying a centrally allocated
    /// SourceSeries successor envelope. The sequence is the Source work-call
    /// ordinal and therefore cannot be silently supplied by a launcher.
    MainSuccessorRequest {
        binding: SuccessorRegistryBinding,
        sequence: u64,
    },
    /// Exact replay request for the allocated-but-disabled Direct `80/1`
    /// family. This remains distinct from an enabled Source request so a
    /// construction artifact cannot silently promote runtime admission.
    DisabledDirectMarketRequestV1 {
        binding: SuccessorRegistryBinding,
        sequence: u64,
    },
    /// Enabled legacy outer request whose instruction account ABI is the
    /// full-width V3 collateral contract. This does not authorize a lowered
    /// Market/Hoard/Kernel interpretation of the request body.
    CollateralReplayRequestV3 {
        action: clutch_solana_layout::collateral_v3_accounts::CollateralActionV3,
        outcome_count: u8,
        selected_outcome: Option<u8>,
    },
}

/// One semantic-owner-produced instruction ready for outer construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedInstructionDraft {
    pub flow: ProtocolFlow,
    pub action_name: String,
    pub semantic_owner: SemanticOwner,
    pub program_id: Address,
    pub accounts: Vec<AccountMeta>,
    pub required_signers: Vec<Address>,
    pub equations: Vec<ExactEquation>,
    pub registry_binding: Option<SuccessorRegistryBinding>,
    pub runtime_admission: RuntimeAdmission,
    wire: OwnedWireContract,
    data: Vec<u8>,
}

impl OwnedInstructionDraft {
    /// Admit an enabled full-width collateral instruction after decoding the
    /// exact request variant and validating every account role. The first
    /// role is the authenticated owner/claimant signer and is retained as a
    /// required signer automatically.
    #[allow(clippy::too_many_arguments)]
    pub fn enabled_full_width_collateral_v3(
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        action: clutch_solana_layout::collateral_v3_accounts::CollateralActionV3,
        outcome_count: u8,
        data: Vec<u8>,
    ) -> Result<Self> {
        use clutch_solana_layout::collateral_v3_accounts::{
            validate_collateral_account_metas_v3, ObservedCollateralAccountMetaV3,
        };

        let request = Request::decode(&data).map_err(|_| ConstructionError::WrongWirePrefix)?;
        let selected_outcome = collateral_request_outcome_v3(action, &request)?;
        let observed = accounts
            .iter()
            .map(|account| ObservedCollateralAccountMetaV3 {
                key: account.pubkey.to_bytes(),
                writable: account.is_writable,
                signer: account.is_signer,
            })
            .collect::<Vec<_>>();
        validate_collateral_account_metas_v3(action, outcome_count, selected_outcome, &observed)
            .map_err(|_| ConstructionError::InvalidAccountContract)?;
        let actor = accounts
            .first()
            .ok_or(ConstructionError::InvalidAccountContract)?
            .pubkey;
        let value = Self {
            flow: ProtocolFlow::CollateralCustodyV3,
            action_name: action_name.into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers: vec![actor],
            equations,
            registry_binding: None,
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::CollateralReplayRequestV3 {
                action,
                outcome_count,
                selected_outcome,
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Wrap an exact liveness runtime intent without creating a parallel codec.
    pub fn liveness(
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        required_signers: Vec<Address>,
        equations: Vec<ExactEquation>,
        exact_bytes: usize,
        data: Vec<u8>,
    ) -> Result<Self> {
        Self::owned_bytes(
            ProtocolFlow::Liveness,
            action_name,
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            OwnedWireContract::LivenessRuntimeV1 { exact_bytes },
            data,
            None,
        )
    }

    /// Assemble a production-inert, centrally allocated successor envelope
    /// around bytes from their semantic owner. The typed registry action owns
    /// the coordinate; its ledger status does not imply dispatcher or checked
    /// release admission.
    pub fn allocated_successor(
        flow: ProtocolFlow,
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        required_signers: Vec<Address>,
        equations: Vec<ExactEquation>,
        action: ExtensionAction,
        payload: &[u8],
    ) -> Result<Self> {
        let family = action.family();
        let binding = registry_binding(family, action.local_tag(), Some(action))?;
        let envelope = ExtensionEnvelope {
            family,
            action,
            payload,
        };
        let mut data = vec![0; EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = envelope.encode(&mut data).map_err(map_registry_error)?;
        data.truncate(exact);
        Self::owned_bytes(
            flow,
            action_name,
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            OwnedWireContract::MainSuccessor { binding },
            data,
            Some(binding),
        )
    }

    /// Assemble the exact outer replay request for one allocated Direct
    /// `80/1` action while retaining disabled runtime admission.
    ///
    /// The Direct client contract owns the action/payload join and the
    /// reference adapter owns the outer request. Account metas remain an
    /// untrusted construction projection; the disabled SBF family will later
    /// authenticate its complete action-specific account list on chain.
    #[allow(clippy::too_many_arguments)]
    pub fn allocated_direct_market_request_v1(
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        required_signers: Vec<Address>,
        equations: Vec<ExactEquation>,
        sequence: u64,
        payload: &clutch_client_contract::direct_market::DirectMarketClientPayloadV1,
    ) -> Result<Self> {
        use clutch_client_contract::direct_market::DirectMarketClientRequestV1;

        let action = ExtensionAction::DirectMarket(payload.action());
        if (matches!(
            payload.action(),
            clutch_solana_layout::registry::DirectMarketAction::InitializeMarket
        ) && sequence != 0)
            || (!matches!(
                payload.action(),
                clutch_solana_layout::registry::DirectMarketAction::InitializeMarket
            ) && sequence == 0)
        {
            return Err(ConstructionError::WrongWirePrefix);
        }
        let binding = registry_binding(action.family(), action.local_tag(), Some(action))?;
        let request = DirectMarketClientRequestV1::encode(sequence, payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        Self::owned_bytes(
            ProtocolFlow::DirectMarketV1,
            action_name,
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            OwnedWireContract::DisabledDirectMarketRequestV1 { binding, sequence },
            request.bytes().to_vec(),
            Some(binding),
        )
    }

    /// Assemble an enabled SourceSeries request through the exact outer
    /// Clutch replay codec. Only coordinates whose dispatcher implementation
    /// is complete may be admitted here.
    pub fn enabled_source_successor(
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        required_signers: Vec<Address>,
        equations: Vec<ExactEquation>,
        action: clutch_solana_layout::registry::SourceSeriesAction,
        call_ordinal: u32,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_solana_layout::registry::SourceSeriesAction;

        if call_ordinal == 0
            || !matches!(
                action,
                SourceSeriesAction::InitializeHead
                    | SourceSeriesAction::OpenRawPage
                    | SourceSeriesAction::IngestBoundaryBatch
            )
        {
            return Err(ConstructionError::UnallocatedRegistryCoordinate);
        }
        let central_action = ExtensionAction::SourceV3(action);
        let family = central_action.family();
        let binding = registry_binding(family, central_action.local_tag(), Some(central_action))?;
        let envelope = ExtensionEnvelope {
            family,
            action: central_action,
            payload,
        };
        let request = ExtensionRequest {
            sequence: u64::from(call_ordinal),
            envelope,
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request
            .encode(&mut data)
            .map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::SourcePlaneV3,
            action_name: action_name.into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::MainSuccessorRequest {
                binding,
                sequence: u64::from(call_ordinal),
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble current Source action 12 only when the exact checked release
    /// admits `(77, 2, 12)`. The caller is crate-private because the ordered
    /// accounts and comparison-only payload must first be reconstructed from
    /// hostile chain state by `source_action12_material`.
    #[cfg(feature = "operator")]
    pub(crate) fn checked_release_source_close_generation(
        release: &crate::rpc_index::IndexedProgramRelease,
        semantic_owner: SemanticOwner,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        payload: &[u8],
    ) -> Result<Self> {
        use crate::rpc_index::{CanonicalFamily, CanonicalIntentCoordinate};
        use clutch_solana_layout::registry::SourceSeriesAction;

        const ACTION: SourceSeriesAction = SourceSeriesAction::CloseGeneration;
        let central_action = ExtensionAction::SourceV3(ACTION);
        let family = central_action.family();
        let coordinate = CanonicalIntentCoordinate {
            family_tag: family.tag(),
            family_version: family.version(),
            local_action: central_action.local_tag(),
        };
        release
            .validate()
            .map_err(|_| ConstructionError::UnallocatedRegistryCoordinate)?;
        if release.program_id == Address::default()
            || semantic_owner.release_sha256 != release.release_manifest_sha256
            || !release.families.contains(&CanonicalFamily::Source)
            || release.enabled_intents.binary_search(&coordinate).is_err()
        {
            return Err(ConstructionError::UnallocatedRegistryCoordinate);
        }
        let binding = registry_binding(family, central_action.local_tag(), Some(central_action))?;
        let envelope = ExtensionEnvelope {
            family,
            action: central_action,
            payload,
        };
        let request = ExtensionRequest {
            sequence: 0,
            envelope,
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request
            .encode(&mut data)
            .map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::SourcePlaneV3,
            action_name: "close-source-generation-v2".into(),
            semantic_owner,
            program_id: release.program_id,
            accounts,
            required_signers: Vec::new(),
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::MainSuccessorRequest {
                binding,
                sequence: 0,
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble a production-inert structured-claim successor whose local
    /// action codec remains owned outside the central action registry. General
    /// V2 and Source/Series callers must use
    /// [`Self::allocated_successor`].
    pub fn semantic_reserved_successor(
        flow: ProtocolFlow,
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        required_signers: Vec<Address>,
        equations: Vec<ExactEquation>,
        family: ExtensionFamily,
        local_action: u8,
        payload: &[u8],
    ) -> Result<Self> {
        if family != ExtensionFamily::StructuredClaim {
            return Err(ConstructionError::UnallocatedRegistryCoordinate);
        }
        if local_action == 0 || payload.len() > MAX_EXTENSION_PAYLOAD_BYTES {
            return Err(if local_action == 0 {
                ConstructionError::UnallocatedRegistryCoordinate
            } else {
                ConstructionError::PayloadTooLong
            });
        }
        let binding = registry_binding(family, local_action, None)?;
        let mut data = Vec::with_capacity(EXTENSION_ENVELOPE_BYTES + payload.len());
        data.push(family.tag());
        data.push(family.version());
        data.push(local_action);
        data.extend_from_slice(payload);
        Self::owned_bytes(
            flow,
            action_name,
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            OwnedWireContract::MainSuccessor { binding },
            data,
            Some(binding),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn owned_bytes(
        flow: ProtocolFlow,
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        required_signers: Vec<Address>,
        equations: Vec<ExactEquation>,
        wire: OwnedWireContract,
        data: Vec<u8>,
        registry_binding: Option<SuccessorRegistryBinding>,
    ) -> Result<Self> {
        let value = Self {
            flow,
            action_name: action_name.into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            registry_binding,
            runtime_admission: RuntimeAdmission::ReservedDisabled,
            wire,
            data,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if self.program_id == Address::default() {
            return Err(ConstructionError::ZeroIdentity);
        }
        if self.action_name.trim().is_empty() {
            return Err(ConstructionError::EmptyActionName);
        }
        self.semantic_owner.validate()?;
        if self.data.is_empty() {
            return Err(ConstructionError::EmptyInstructionData);
        }
        if self.equations.is_empty() {
            return Err(ConstructionError::MissingExactEquation);
        }
        for equation in &self.equations {
            equation.validate()?;
        }
        let source_request = matches!(
            self.wire,
            OwnedWireContract::MainSuccessorRequest { binding, .. }
                if binding.family == ExtensionFamily::SourceSeries
        );
        let collateral_request = matches!(
            self.wire,
            OwnedWireContract::CollateralReplayRequestV3 { .. }
        );
        let direct_request = matches!(
            self.wire,
            OwnedWireContract::DisabledDirectMarketRequestV1 { .. }
        );
        if !source_request && !collateral_request && !direct_request {
            let mut accounts = BTreeSet::new();
            for account in &self.accounts {
                if !accounts.insert(account.pubkey) {
                    return Err(ConstructionError::DuplicateAccount);
                }
            }
        }
        let mut signers = BTreeSet::new();
        for signer in &self.required_signers {
            if *signer == Address::default() {
                return Err(ConstructionError::ZeroIdentity);
            }
            if !signers.insert(*signer) {
                return Err(ConstructionError::DuplicateSigner);
            }
        }
        match self.wire {
            OwnedWireContract::LivenessRuntimeV1 { exact_bytes } => {
                if self.registry_binding.is_some() {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                if self.flow != ProtocolFlow::Liveness {
                    return Err(ConstructionError::WrongFlow);
                }
                if self.data.len() != exact_bytes {
                    return Err(ConstructionError::WrongWireLength);
                }
                if !self.data.starts_with(&LIVENESS_RUNTIME_V1_INTENT_MAGIC) {
                    return Err(ConstructionError::WrongWirePrefix);
                }
            }
            OwnedWireContract::MainSuccessor { binding } => {
                let family = binding.family;
                let local_action = binding.local_action;
                if local_action == 0
                    || self.data.len() < 3
                    || self.data[0] != family.tag()
                    || self.data[1] != family.version()
                    || self.data[2] != local_action
                {
                    return Err(ConstructionError::WrongWirePrefix);
                }
                if self.registry_binding != Some(binding)
                    || family.allocation_status() != Some(binding.family_status)
                {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                if let Some(action) = binding.central_action {
                    if action.family() != family || action.local_tag() != local_action {
                        return Err(ConstructionError::UnallocatedRegistryCoordinate);
                    }
                } else if matches!(
                    family,
                    ExtensionFamily::GeneralV2 | ExtensionFamily::SourceSeries
                ) {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                let family_matches = match self.flow {
                    ProtocolFlow::GeneralV2Candidate
                    | ProtocolFlow::GeneralV2Settlement
                    | ProtocolFlow::GeneralV2Fees
                    | ProtocolFlow::DirectEggSettlement
                    | ProtocolFlow::MarketEpochCreation
                    | ProtocolFlow::KeeperSettlement
                    | ProtocolFlow::RecoveryRetirement => family == ExtensionFamily::GeneralV2,
                    ProtocolFlow::ProductSeries | ProtocolFlow::SourcePlaneV3 => {
                        family == ExtensionFamily::SourceSeries
                    }
                    ProtocolFlow::StructuredClaim => family == ExtensionFamily::StructuredClaim,
                    ProtocolFlow::CollateralCustodyV3
                    | ProtocolFlow::DirectMarketV1
                    | ProtocolFlow::Liveness => false,
                };
                if !family_matches {
                    return Err(ConstructionError::WrongFlow);
                }
            }
            OwnedWireContract::MainSuccessorRequest { binding, sequence } => {
                let request = ExtensionRequest::decode(&self.data)
                    .map_err(|_| ConstructionError::WrongWirePrefix)?;
                if request.sequence != sequence
                    || request.envelope.family != binding.family
                    || request.envelope.action.local_tag() != binding.local_action
                    || self.registry_binding != Some(binding)
                    || binding.family.allocation_status() != Some(binding.family_status)
                    || binding.family != ExtensionFamily::SourceSeries
                    || self.flow != ProtocolFlow::SourcePlaneV3
                    || !matches!(
                        binding.central_action,
                        Some(ExtensionAction::SourceV3(action))
                            if request.envelope.action == ExtensionAction::SourceV3(action)
                    )
                    || self.runtime_admission != RuntimeAdmission::ReleaseBoundEnabled
                {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                let ExtensionAction::SourceV3(action) = request.envelope.action else {
                    return Err(ConstructionError::WrongFlow);
                };
                let sequence_is_valid = if matches!(
                    action,
                    clutch_solana_layout::registry::SourceSeriesAction::CloseGeneration
                ) {
                    sequence == 0
                } else {
                    sequence != 0
                };
                if !matches!(
                    action,
                    clutch_solana_layout::registry::SourceSeriesAction::InitializeHead
                        | clutch_solana_layout::registry::SourceSeriesAction::OpenRawPage
                        | clutch_solana_layout::registry::SourceSeriesAction::IngestBoundaryBatch
                        | clutch_solana_layout::registry::SourceSeriesAction::CloseGeneration
                ) || !sequence_is_valid
                {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                let observed = self
                    .accounts
                    .iter()
                    .map(|account| ObservedSourceAccountMetaV2 {
                        key: account.pubkey.to_bytes(),
                        writable: account.is_writable,
                        signer: account.is_signer,
                    })
                    .collect::<Vec<_>>();
                validate_account_metas_v2(action, &observed)
                    .map_err(|_| ConstructionError::InvalidAccountContract)?;
            }
            OwnedWireContract::DisabledDirectMarketRequestV1 { binding, sequence } => {
                let request = ExtensionRequest::decode(&self.data)
                    .map_err(|_| ConstructionError::WrongWirePrefix)?;
                if request.sequence != sequence
                    || request.envelope.family != ExtensionFamily::DirectMarket
                    || request.envelope.family != binding.family
                    || request.envelope.action.local_tag() != binding.local_action
                    || self.registry_binding != Some(binding)
                    || binding.family.allocation_status() != Some(binding.family_status)
                    || self.flow != ProtocolFlow::DirectMarketV1
                    || self.runtime_admission != RuntimeAdmission::ReservedDisabled
                    || !matches!(
                        binding.central_action,
                        Some(ExtensionAction::DirectMarket(action))
                            if request.envelope.action == ExtensionAction::DirectMarket(action)
                    )
                {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                let ExtensionAction::DirectMarket(action) = request.envelope.action else {
                    return Err(ConstructionError::WrongFlow);
                };
                if (matches!(
                    action,
                    clutch_solana_layout::registry::DirectMarketAction::InitializeMarket
                ) && sequence != 0)
                    || (!matches!(
                        action,
                        clutch_solana_layout::registry::DirectMarketAction::InitializeMarket
                    ) && sequence == 0)
                {
                    return Err(ConstructionError::WrongWirePrefix);
                }
            }
            OwnedWireContract::CollateralReplayRequestV3 {
                action,
                outcome_count,
                selected_outcome,
            } => {
                use clutch_solana_layout::collateral_v3_accounts::{
                    validate_collateral_account_metas_v3, ObservedCollateralAccountMetaV3,
                };

                let request =
                    Request::decode(&self.data).map_err(|_| ConstructionError::WrongWirePrefix)?;
                if self.registry_binding.is_some()
                    || self.flow != ProtocolFlow::CollateralCustodyV3
                    || self.runtime_admission != RuntimeAdmission::ReleaseBoundEnabled
                    || collateral_request_outcome_v3(action, &request)? != selected_outcome
                {
                    return Err(ConstructionError::WrongFlow);
                }
                let observed = self
                    .accounts
                    .iter()
                    .map(|account| ObservedCollateralAccountMetaV3 {
                        key: account.pubkey.to_bytes(),
                        writable: account.is_writable,
                        signer: account.is_signer,
                    })
                    .collect::<Vec<_>>();
                validate_collateral_account_metas_v3(
                    action,
                    outcome_count,
                    selected_outcome,
                    &observed,
                )
                .map_err(|_| ConstructionError::InvalidAccountContract)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn wire_contract(&self) -> OwnedWireContract {
        self.wire
    }

    fn instruction(&self) -> Instruction {
        Instruction {
            program_id: self.program_id,
            accounts: self.accounts.clone(),
            data: self.data.clone(),
        }
    }
}

/// Explicit transport bounds. They are deployment/session configuration, not
/// protocol invariants, so alternate local infrastructure can raise them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransactionTransport {
    pub packet_limit_bytes: usize,
}

impl Default for TransactionTransport {
    fn default() -> Self {
        Self {
            packet_limit_bytes: 1_232,
        }
    }
}

/// Fully assembled but unsigned and blockhash-free transaction artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsignedProtocolTransaction {
    pub schema: &'static str,
    pub flows: Vec<ProtocolFlow>,
    pub actions: Vec<String>,
    pub semantic_owners: Vec<SemanticOwner>,
    pub registry_bindings: Vec<Option<SuccessorRegistryBinding>>,
    pub runtime_admissions: Vec<RuntimeAdmission>,
    pub required_signers: Vec<Address>,
    pub exact_equations: Vec<ExactEquation>,
    pub serialized_transaction: Vec<u8>,
    pub has_recent_blockhash: bool,
    pub signed: bool,
    pub submitted: bool,
}

/// Complete construction input for the eight currently identified operator
/// flows. Each vector is nonempty by contract. Candidate/source/Series/wrapper
/// actions remain separate transactions so their cursors can advance between
/// observations. Settlement, fees, direct Eggs, and its liveness movement are
/// intentionally joined into one atomic transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentProtocolWorkflowDraft {
    pub source_plane_v3: Vec<OwnedInstructionDraft>,
    pub general_candidate: Vec<OwnedInstructionDraft>,
    pub general_settlement: Vec<OwnedInstructionDraft>,
    pub general_fees: Vec<OwnedInstructionDraft>,
    pub direct_eggs: Vec<OwnedInstructionDraft>,
    pub settlement_liveness: Vec<OwnedInstructionDraft>,
    pub product_series: Vec<OwnedInstructionDraft>,
    pub structured_claim: Vec<OwnedInstructionDraft>,
}

/// Unsigned, ordered output of [`CurrentProtocolWorkflowDraft`]. No field is
/// an execution receipt and no step is advanced from another step's expected
/// poststate without an external account reload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentProtocolWorkflowConstruction {
    pub source_plane_v3: Vec<UnsignedProtocolTransaction>,
    pub general_candidate: Vec<UnsignedProtocolTransaction>,
    pub atomic_general_settlement: UnsignedProtocolTransaction,
    pub product_series: Vec<UnsignedProtocolTransaction>,
    pub structured_claim: Vec<UnsignedProtocolTransaction>,
}

/// Release-bound outer builder. It never accepts or returns a keypair.
pub struct ProtocolTransactionBuilder {
    payer: Address,
    clutch_program: Address,
    clutch_release_sha256: [u8; 32],
    transport: TransactionTransport,
}

impl ProtocolTransactionBuilder {
    pub fn new(
        payer: Address,
        clutch_program: Address,
        clutch_release_sha256: [u8; 32],
        transport: TransactionTransport,
    ) -> Result<Self> {
        if payer == Address::default() || clutch_program == Address::default() {
            return Err(ConstructionError::ZeroIdentity);
        }
        if clutch_release_sha256 == [0; 32] {
            return Err(ConstructionError::ZeroReleaseDigest);
        }
        if transport.packet_limit_bytes == 0 {
            return Err(ConstructionError::PacketTooLarge);
        }
        Ok(Self {
            payer,
            clutch_program,
            clutch_release_sha256,
            transport,
        })
    }

    /// Build one or more current flows atomically. SourcePlane V3 and liveness
    /// may target their separately released adapter programs; all successor
    /// envelopes must target the release-bound Clutch program.
    pub fn build_atomic(
        &self,
        drafts: &[OwnedInstructionDraft],
    ) -> Result<UnsignedProtocolTransaction> {
        if drafts.is_empty() {
            return Err(ConstructionError::EmptyBundle);
        }
        let mut instructions = Vec::with_capacity(drafts.len());
        let mut flows = Vec::new();
        let mut actions = Vec::with_capacity(drafts.len());
        let mut semantic_owners = Vec::with_capacity(drafts.len());
        let mut registry_bindings = Vec::with_capacity(drafts.len());
        let mut runtime_admissions = Vec::with_capacity(drafts.len());
        let mut required_signers = BTreeSet::from([self.payer]);
        let mut exact_equations = Vec::new();

        for draft in drafts {
            draft.validate()?;
            if matches!(
                draft.wire,
                OwnedWireContract::MainSuccessor { .. }
                    | OwnedWireContract::MainSuccessorRequest { .. }
                    | OwnedWireContract::DisabledDirectMarketRequestV1 { .. }
                    | OwnedWireContract::CollateralReplayRequestV3 { .. }
            ) && draft.program_id != self.clutch_program
            {
                return Err(ConstructionError::ForeignProgram);
            }
            if matches!(
                draft.wire,
                OwnedWireContract::CollateralReplayRequestV3 {
                    action:
                        clutch_solana_layout::collateral_v3_accounts::CollateralActionV3::RedeemExternal,
                    ..
                }
            ) && draft.accounts.first().map(|account| account.pubkey) == Some(self.payer)
            {
                // External redemption alone requires the claimant AccountInfo
                // itself to remain read-only. A transaction payer is globally
                // writable after privilege union even if this instruction's
                // meta says read-only, so that alias cannot be submitted.
                return Err(ConstructionError::InvalidAccountContract);
            }
            for signer in &draft.required_signers {
                let represented = *signer == self.payer
                    || draft
                        .accounts
                        .iter()
                        .any(|meta| meta.pubkey == *signer && meta.is_signer);
                if !represented {
                    return Err(ConstructionError::MissingSignerMeta);
                }
                required_signers.insert(*signer);
            }
            if !flows.contains(&draft.flow) {
                flows.push(draft.flow);
            }
            actions.push(draft.action_name.clone());
            semantic_owners.push(draft.semantic_owner.clone());
            registry_bindings.push(draft.registry_binding);
            runtime_admissions.push(draft.runtime_admission);
            exact_equations.extend(draft.equations.iter().cloned());
            instructions.push(draft.instruction());
        }

        let transaction = Transaction::new_with_payer(&instructions, Some(&self.payer));
        let serialized_transaction =
            bincode::serialize(&transaction).map_err(|_| ConstructionError::Serialization)?;
        if serialized_transaction.len() > self.transport.packet_limit_bytes {
            return Err(ConstructionError::PacketTooLarge);
        }
        Ok(UnsignedProtocolTransaction {
            schema: CONSTRUCTION_PLAN_SCHEMA,
            flows,
            actions,
            semantic_owners,
            registry_bindings,
            runtime_admissions,
            required_signers: required_signers.into_iter().collect(),
            exact_equations,
            serialized_transaction,
            has_recent_blockhash: false,
            signed: false,
            submitted: false,
        })
    }

    /// Construct the current end-to-end work inventory without signing or
    /// silently substituting a missing flow. The semantic owners remain
    /// responsible for deriving each successive payload from freshly
    /// authenticated state; this method only preserves the declared order.
    pub fn build_current_workflow(
        &self,
        workflow: &CurrentProtocolWorkflowDraft,
    ) -> Result<CurrentProtocolWorkflowConstruction> {
        let source_plane_v3 =
            self.build_sequence(ProtocolFlow::SourcePlaneV3, &workflow.source_plane_v3)?;
        let general_candidate = self.build_sequence(
            ProtocolFlow::GeneralV2Candidate,
            &workflow.general_candidate,
        )?;
        require_nonempty_flow(
            ProtocolFlow::GeneralV2Settlement,
            &workflow.general_settlement,
        )?;
        require_nonempty_flow(ProtocolFlow::GeneralV2Fees, &workflow.general_fees)?;
        require_nonempty_flow(ProtocolFlow::DirectEggSettlement, &workflow.direct_eggs)?;
        require_nonempty_flow(ProtocolFlow::Liveness, &workflow.settlement_liveness)?;
        let mut settlement = Vec::with_capacity(
            workflow.general_settlement.len()
                + workflow.general_fees.len()
                + workflow.direct_eggs.len()
                + workflow.settlement_liveness.len(),
        );
        settlement.extend(workflow.general_settlement.iter().cloned());
        settlement.extend(workflow.general_fees.iter().cloned());
        settlement.extend(workflow.direct_eggs.iter().cloned());
        settlement.extend(workflow.settlement_liveness.iter().cloned());
        let atomic_general_settlement = self.build_atomic(&settlement)?;
        let product_series =
            self.build_sequence(ProtocolFlow::ProductSeries, &workflow.product_series)?;
        let structured_claim =
            self.build_sequence(ProtocolFlow::StructuredClaim, &workflow.structured_claim)?;
        Ok(CurrentProtocolWorkflowConstruction {
            source_plane_v3,
            general_candidate,
            atomic_general_settlement,
            product_series,
            structured_claim,
        })
    }

    fn build_sequence(
        &self,
        expected: ProtocolFlow,
        drafts: &[OwnedInstructionDraft],
    ) -> Result<Vec<UnsignedProtocolTransaction>> {
        require_nonempty_flow(expected, drafts)?;
        let mut plans = Vec::with_capacity(drafts.len());
        for draft in drafts {
            plans.push(self.build_atomic(core::slice::from_ref(draft))?);
        }
        Ok(plans)
    }

    /// Release digest carried by construction metadata. A transaction cannot
    /// authenticate this digest by itself; a launcher must join it to the ELF
    /// it explicitly loads into the local validator.
    #[must_use]
    pub const fn clutch_release_sha256(&self) -> [u8; 32] {
        self.clutch_release_sha256
    }

    /// Main program identity bound to every successor envelope.
    #[must_use]
    pub const fn clutch_program(&self) -> Address {
        self.clutch_program
    }

    /// Public fee-payer identity carried by every blockhash-free draft. This
    /// exposes no key or signing capability.
    #[must_use]
    pub const fn payer(&self) -> Address {
        self.payer
    }
}

fn collateral_request_outcome_v3(
    expected: clutch_solana_layout::collateral_v3_accounts::CollateralActionV3,
    request: &Request,
) -> Result<Option<u8>> {
    use clutch_solana_layout::collateral_v3_accounts::CollateralActionV3 as Expected;

    let actual = match request.action {
        Action::Layout(Intent::Endow { .. }) => (Expected::Endow, None),
        Action::Layout(Intent::WithdrawCash { .. }) => (Expected::WithdrawCash, None),
        Action::Layout(Intent::Split { .. }) => (Expected::Split, None),
        Action::Layout(Intent::Merge { .. }) => (Expected::Merge, None),
        Action::Layout(Intent::Materialize { outcome, .. }) => {
            (Expected::Materialize, Some(outcome))
        }
        Action::Layout(Intent::Dematerialize { outcome, .. }) => {
            (Expected::Dematerialize, Some(outcome))
        }
        Action::Layout(Intent::RedeemExternal { outcome, .. }) if request.sequence == 0 => {
            (Expected::RedeemExternal, Some(outcome))
        }
        _ => return Err(ConstructionError::WrongWirePrefix),
    };
    if actual.0 != expected {
        return Err(ConstructionError::WrongWirePrefix);
    }
    Ok(actual.1)
}

fn require_nonempty_flow(expected: ProtocolFlow, drafts: &[OwnedInstructionDraft]) -> Result<()> {
    if drafts.is_empty() {
        return Err(ConstructionError::EmptyBundle);
    }
    if drafts.iter().any(|draft| draft.flow != expected) {
        return Err(ConstructionError::WrongFlow);
    }
    Ok(())
}

fn registry_binding(
    family: ExtensionFamily,
    local_action: u8,
    central_action: Option<ExtensionAction>,
) -> Result<SuccessorRegistryBinding> {
    let family_status = family
        .allocation_status()
        .ok_or(ConstructionError::UnallocatedRegistryCoordinate)?;
    Ok(SuccessorRegistryBinding {
        family,
        local_action,
        family_status,
        central_action,
    })
}

const fn map_registry_error(error: RegistryError) -> ConstructionError {
    match error {
        RegistryError::TooLong => ConstructionError::PayloadTooLong,
        RegistryError::Truncated
        | RegistryError::UnknownFamilyVersion
        | RegistryError::UnknownLocalAction
        | RegistryError::OutputTooSmall => ConstructionError::UnallocatedRegistryCoordinate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::collateral_v3_accounts::{
        account_contract_v3, CollateralAccountRoleV3, CollateralActionV3,
    };
    use clutch_solana_layout::registry::{GeneralV2Action, SourceSeriesAction};
    use clutch_solana_layout::source_series::account_contract_v2;
    use clutch_solana_layout::{Hash32, Intent};

    fn owner() -> SemanticOwner {
        SemanticOwner {
            package: "clutch-structured-claim-runtime-contract".into(),
            schema: "structured-claim/v1".into(),
            release_sha256: [7; 32],
        }
    }

    fn source_accounts(
        action: SourceSeriesAction,
        coalesce_payer_keeper: bool,
    ) -> Vec<AccountMeta> {
        let contract = account_contract_v2(action);
        let keeper_index = match action {
            SourceSeriesAction::InitializeHead => 14,
            SourceSeriesAction::OpenRawPage => 15,
            SourceSeriesAction::IngestBoundaryBatch => 20,
            _ => unreachable!(),
        };
        let payer_index = keeper_index + 1;
        (0..contract.len())
            .map(|index| {
                let required = contract.meta(index).unwrap();
                let identity_index = if coalesce_payer_keeper && index == payer_index {
                    keeper_index
                } else {
                    index
                };
                AccountMeta {
                    pubkey: Address::new_from_array(
                        [u8::try_from(identity_index + 10).unwrap(); 32],
                    ),
                    is_signer: required.signer,
                    is_writable: required.writable,
                }
            })
            .collect()
    }

    fn collateral_accounts(
        action: CollateralActionV3,
        selected: Option<u8>,
        coalesce_token_programs: bool,
    ) -> Vec<AccountMeta> {
        let contract = account_contract_v3(action, 2, selected).unwrap();
        let collateral_program = (0..contract.len())
            .find(|index| {
                contract.meta(*index).unwrap().role
                    == CollateralAccountRoleV3::CollateralTokenProgram
            })
            .unwrap();
        (0..contract.len())
            .map(|index| {
                let required = contract.meta(index).unwrap();
                let identity_index = if coalesce_token_programs
                    && required.role == CollateralAccountRoleV3::OutcomeTokenProgram
                {
                    collateral_program
                } else {
                    index
                };
                AccountMeta {
                    pubkey: Address::new_from_array(
                        [u8::try_from(identity_index + 40).unwrap(); 32],
                    ),
                    is_signer: required.signer,
                    is_writable: required.writable,
                }
            })
            .collect()
    }

    fn collateral_request(sequence: u64, intent: Intent) -> Vec<u8> {
        let mut inner = vec![0; intent.encoded_len()];
        let written = intent.encode(&mut inner).unwrap();
        inner.truncate(written);
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&sequence.to_le_bytes());
        data.push(0);
        let inner_len = u16::try_from(inner.len()).expect("fixed Request intent fits u16");
        data.extend_from_slice(&inner_len.to_le_bytes());
        data.extend_from_slice(&inner);
        data
    }

    fn collateral_equation() -> Vec<ExactEquation> {
        vec![ExactEquation {
            name: "collateral atoms conserved".into(),
            unit: IntegerUnit::CollateralAtoms {
                mint: Address::new_from_array([90; 32]),
            },
            left: 7,
            right: 7,
        }]
    }

    #[test]
    fn enabled_collateral_request_uses_full_width_roles_and_closed_program_alias() {
        let action = CollateralActionV3::Materialize;
        let mut accounts = collateral_accounts(action, Some(1), true);
        accounts[0].is_writable = true;
        let draft = OwnedInstructionDraft::enabled_full_width_collateral_v3(
            "materialize-v3",
            owner(),
            Address::new_from_array([2; 32]),
            accounts,
            collateral_equation(),
            action,
            2,
            collateral_request(
                4,
                Intent::Materialize {
                    market: Hash32::from_bytes([91; 32]),
                    owner: Hash32::from_bytes([40; 32]),
                    destination: Hash32::from_bytes([92; 32]),
                    outcome: 1,
                    quantity: 7,
                },
            ),
        )
        .unwrap();
        assert_eq!(draft.flow, ProtocolFlow::CollateralCustodyV3);
        assert_eq!(
            draft.runtime_admission,
            RuntimeAdmission::ReleaseBoundEnabled
        );
        assert_eq!(
            draft.required_signers,
            vec![Address::new_from_array([40; 32])]
        );
    }

    #[test]
    fn enabled_collateral_request_refuses_action_or_role_substitution() {
        let action = CollateralActionV3::Materialize;
        let data = collateral_request(
            4,
            Intent::Dematerialize {
                market: Hash32::from_bytes([91; 32]),
                owner: Hash32::from_bytes([40; 32]),
                source: Hash32::from_bytes([92; 32]),
                outcome: 1,
                quantity: 7,
            },
        );
        assert_eq!(
            OwnedInstructionDraft::enabled_full_width_collateral_v3(
                "materialize-v3",
                owner(),
                Address::new_from_array([2; 32]),
                collateral_accounts(action, Some(1), false),
                collateral_equation(),
                action,
                2,
                data,
            ),
            Err(ConstructionError::WrongWirePrefix)
        );

        let mut aliased = collateral_accounts(action, Some(1), false);
        aliased[1].pubkey = aliased[0].pubkey;
        assert_eq!(
            OwnedInstructionDraft::enabled_full_width_collateral_v3(
                "materialize-v3",
                owner(),
                Address::new_from_array([2; 32]),
                aliased,
                collateral_equation(),
                action,
                2,
                collateral_request(
                    4,
                    Intent::Materialize {
                        market: Hash32::from_bytes([91; 32]),
                        owner: Hash32::from_bytes([40; 32]),
                        destination: Hash32::from_bytes([92; 32]),
                        outcome: 1,
                        quantity: 7,
                    },
                ),
            ),
            Err(ConstructionError::InvalidAccountContract)
        );
    }

    #[test]
    fn enabled_source_request_carries_exact_call_ordinal_and_allowed_alias() {
        let action = SourceSeriesAction::InitializeHead;
        let draft = OwnedInstructionDraft::enabled_source_successor(
            "initialize-source-head",
            owner(),
            Address::new_from_array([2; 32]),
            source_accounts(action, true),
            vec![Address::new_from_array([24; 32])],
            vec![ExactEquation {
                name: "source call quote".into(),
                unit: IntegerUnit::Lamports,
                left: 9,
                right: 9,
            }],
            action,
            7,
            &[8; 160],
        )
        .unwrap();
        let decoded = ExtensionRequest::decode(draft.data()).unwrap();
        assert_eq!(decoded.sequence, 7);
        assert_eq!(decoded.envelope.action, ExtensionAction::SourceV3(action));
        assert_eq!(
            draft.runtime_admission,
            RuntimeAdmission::ReleaseBoundEnabled
        );
    }

    #[test]
    fn enabled_ingest_request_uses_the_frozen_twenty_four_account_contract() {
        let action = SourceSeriesAction::IngestBoundaryBatch;
        let accounts = source_accounts(action, true);
        assert_eq!(accounts.len(), 24);
        let draft = OwnedInstructionDraft::enabled_source_successor(
            "ingest-source-boundary",
            owner(),
            Address::new_from_array([2; 32]),
            accounts,
            vec![Address::new_from_array([30; 32])],
            vec![ExactEquation {
                name: "append-boundary call quote".into(),
                unit: IntegerUnit::Lamports,
                left: 11,
                right: 11,
            }],
            action,
            9,
            &[8; 160],
        )
        .unwrap();
        let decoded = ExtensionRequest::decode(draft.data()).unwrap();
        assert_eq!(decoded.envelope.action, ExtensionAction::SourceV3(action));
        assert_eq!(decoded.sequence, 9);
        assert_eq!(
            draft.runtime_admission,
            RuntimeAdmission::ReleaseBoundEnabled
        );
    }

    #[test]
    fn enabled_source_request_refuses_unlisted_role_alias() {
        let action = SourceSeriesAction::OpenRawPage;
        let mut accounts = source_accounts(action, false);
        accounts[1].pubkey = accounts[0].pubkey;
        assert_eq!(
            OwnedInstructionDraft::enabled_source_successor(
                "open-raw-page",
                owner(),
                Address::new_from_array([2; 32]),
                accounts,
                vec![
                    Address::new_from_array([25; 32]),
                    Address::new_from_array([26; 32])
                ],
                vec![ExactEquation {
                    name: "source call quote".into(),
                    unit: IntegerUnit::Lamports,
                    left: 9,
                    right: 9,
                }],
                action,
                8,
                &[8; 160],
            ),
            Err(ConstructionError::InvalidAccountContract)
        );
    }

    #[test]
    fn structured_claim_envelope_is_built_without_signing_or_submission() {
        let payer = Address::new_from_array([1; 32]);
        let program = Address::new_from_array([2; 32]);
        let draft = OwnedInstructionDraft::semantic_reserved_successor(
            ProtocolFlow::StructuredClaim,
            "wrap-full",
            owner(),
            program,
            vec![AccountMeta::new_readonly(payer, true)],
            vec![payer],
            vec![ExactEquation {
                name: "full-vector backing".into(),
                unit: IntegerUnit::WrapperAtoms {
                    mint: Address::new_from_array([3; 32]),
                },
                left: 9,
                right: 9,
            }],
            ExtensionFamily::StructuredClaim,
            3,
            &[9; 72],
        )
        .unwrap();
        assert_eq!(&draft.data()[..3], &[75, 1, 3]);
        let builder = ProtocolTransactionBuilder::new(
            payer,
            program,
            [4; 32],
            TransactionTransport::default(),
        )
        .unwrap();
        let plan = builder.build_atomic(&[draft]).unwrap();
        assert!(!plan.has_recent_blockhash);
        assert!(!plan.signed);
        assert!(!plan.submitted);
    }

    #[test]
    fn direct_successor_request_stays_exact_and_runtime_disabled() {
        use clutch_client_contract::direct_market::DirectMarketClientPayloadV1;
        use clutch_solana_layout::registry::DirectMarketAction;

        let program = Address::new_from_array([2; 32]);
        let accounts = vec![
            AccountMeta::new(Address::new_from_array([3; 32]), false),
            AccountMeta::new(Address::new_from_array([4; 32]), false),
            AccountMeta::new(Address::new_from_array([5; 32]), false),
            AccountMeta::new_readonly(Address::new_from_array([6; 32]), false),
        ];
        let payload = DirectMarketClientPayloadV1::empty(
            DirectMarketAction::BeginVerification,
        )
        .unwrap();
        let draft = OwnedInstructionDraft::allocated_direct_market_request_v1(
            "direct-begin-verification",
            owner(),
            program,
            accounts.clone(),
            vec![],
            vec![ExactEquation {
                name: "no collateral movement".into(),
                unit: IntegerUnit::Lamports,
                left: 0,
                right: 0,
            }],
            9,
            &payload,
        )
        .unwrap();
        assert_eq!(draft.flow, ProtocolFlow::DirectMarketV1);
        assert_eq!(draft.runtime_admission, RuntimeAdmission::ReservedDisabled);
        let request = ExtensionRequest::decode(draft.data()).unwrap();
        assert_eq!(request.sequence, 9);
        assert_eq!(
            request.envelope.action,
            ExtensionAction::DirectMarket(DirectMarketAction::BeginVerification)
        );
        assert_eq!(
            OwnedInstructionDraft::allocated_direct_market_request_v1(
                "direct-begin-verification",
                owner(),
                program,
                accounts,
                vec![],
                vec![ExactEquation {
                    name: "no collateral movement".into(),
                    unit: IntegerUnit::Lamports,
                    left: 0,
                    right: 0,
                }],
                0,
                &payload,
            ),
            Err(ConstructionError::WrongWirePrefix)
        );
    }

    #[test]
    fn settlement_can_atomically_join_fees_direct_eggs_and_liveness() {
        let payer = Address::new_from_array([1; 32]);
        let program = Address::new_from_array([2; 32]);
        let runtime = Address::new_from_array([3; 32]);
        let equation = || ExactEquation {
            name: "exact conservation".into(),
            unit: IntegerUnit::Lamports,
            left: 5,
            right: 5,
        };
        let allocated_successor = |flow, action| {
            OwnedInstructionDraft::allocated_successor(
                flow,
                "general-action",
                owner(),
                program,
                vec![AccountMeta::new_readonly(payer, true)],
                vec![payer],
                vec![equation()],
                action,
                &[8; 32],
            )
            .unwrap()
        };
        let mut liveness_data = vec![0; 272];
        liveness_data[..8].copy_from_slice(&LIVENESS_RUNTIME_V1_INTENT_MAGIC);
        let liveness = OwnedInstructionDraft::liveness(
            "settlement-work",
            owner(),
            runtime,
            vec![AccountMeta::new_readonly(payer, true)],
            vec![payer],
            vec![equation()],
            liveness_data.len(),
            liveness_data,
        )
        .unwrap();
        let drafts = [
            allocated_successor(
                ProtocolFlow::GeneralV2Settlement,
                ExtensionAction::GeneralV2(GeneralV2Action::FinalizeOwnerSettlement),
            ),
            allocated_successor(
                ProtocolFlow::GeneralV2Fees,
                ExtensionAction::GeneralV2(GeneralV2Action::AccountReceiptEnd),
            ),
            allocated_successor(
                ProtocolFlow::DirectEggSettlement,
                ExtensionAction::GeneralV2(GeneralV2Action::ConsumeDirectReceiptEggs),
            ),
            liveness,
        ];
        let builder = ProtocolTransactionBuilder::new(
            payer,
            program,
            [4; 32],
            TransactionTransport {
                packet_limit_bytes: 8_192,
            },
        )
        .unwrap();
        let plan = builder.build_atomic(&drafts).unwrap();
        assert_eq!(plan.flows.len(), 4);
        assert_eq!(plan.exact_equations.len(), 4);
    }
}
