//! Construction-only transactions for the current protocol workstreams.
//!
//! Semantic crates own payload bytes and account ordering. This module owns
//! only the outer Solana instruction/transaction boundary, explicit release
//! binding, signer declaration, exact-integer balance equations, and grouping
//! of independently owned actions into an atomic unsigned transaction. It has
//! no keypair, blockhash, RPC, signing, or submission dependency.

use clutch_solana_layout::registry::{
    AllocationStatus, ExtensionAction, ExtensionEnvelope, ExtensionFamily, RegistryError,
    EXTENSION_ENVELOPE_BYTES,
};
use clutch_solana_layout::source_series::{validate_account_metas_v2, ObservedSourceAccountMetaV2};
use clutch_solana_layout::Intent;
use clutch_solana_reference::{Action, ExtensionRequest, Request};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_message::{v0, AddressLookupTableAccount, VersionedMessage};
use solana_transaction::{versioned::VersionedTransaction, Transaction};
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
    MissingLookupTable,
    InvalidLookupTable,
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
            Self::MissingLookupTable => {
                "current wide transactions require an authenticated address lookup table"
            }
            Self::InvalidLookupTable => {
                "address lookup table does not cover the exact wide transaction"
            }
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
    /// Current Direct `80/1` successor family.
    DirectMarketV1,
    DirectEggSettlement,
    Liveness,
    ProductSeries,
    FractionalRedemption,
    StructuredClaim,
    /// Payload-scoped current Dealer facility retirement targets 8 and 9.
    DealerFacilityTerminal,
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
    /// Only one checked payload discriminator beneath a disabled coarse tuple
    /// is admitted by the release manifest.
    PayloadVariantReleaseBoundEnabled,
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
    /// Exact enabled outer replay request for one current Fractional
    /// redemption action. The semantic owner fixes both payload width and the
    /// complete account privilege bitmap.
    FractionalRedemptionRequestV1 {
        binding: SuccessorRegistryBinding,
        action: clutch_fractional_redemption_runtime::FractionalRedemptionActionV1,
        sequence: u64,
    },
    /// Exact replay request for the allocated-but-disabled Direct `80/1`
    /// family. This remains distinct from an enabled Source request so a
    /// construction artifact cannot silently promote runtime admission.
    DisabledDirectMarketRequestV1 {
        binding: SuccessorRegistryBinding,
        sequence: u64,
    },
    /// Exact outer request for a Direct `80/1` coordinate already admitted by
    /// the checked release used by the caller. Only the release-bound action
    /// material constructor can select this admission state.
    EnabledDirectMarketRequestV1 {
        binding: SuccessorRegistryBinding,
        sequence: u64,
    },
    /// Sole chain-derived current Direct action-8 request. The hostile state
    /// chooses the branch through the exact account geometry; payload is empty.
    EnabledDirectFinalizeSelectionRequestV2 {
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
    /// Exact enabled outer Structured wrapper envelope and account ABI.
    StructuredWrapperV1 {
        action: clutch_structured_claim_runtime_contract::StructuredClaimActionV1,
        product_link_writable: bool,
    },
    /// Exact central Dealer request for one closed terminal discriminator.
    DealerTerminalRetireV1 {
        variant: crate::rpc_index::CanonicalIntentVariantV1,
        sequence: u64,
    },
}

/// One exact role in the payload-scoped Dealer terminal frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DealerTerminalAccountSpecV1 {
    pub label: &'static str,
    pub signer: bool,
    pub writable: bool,
}

const fn dealer_terminal_spec(
    label: &'static str,
    signer: bool,
    writable: bool,
) -> DealerTerminalAccountSpecV1 {
    DealerTerminalAccountSpecV1 {
        label,
        signer,
        writable,
    }
}

const DEALER_TERMINAL_COMMON_V1: [DealerTerminalAccountSpecV1; 43] = [
    dealer_terminal_spec("actor", true, true),
    dealer_terminal_spec("policy", false, false),
    dealer_terminal_spec("state-v3", false, true),
    dealer_terminal_spec("facility-position-v3", false, true),
    dealer_terminal_spec("facility-replay-v3", false, true),
    dealer_terminal_spec("funded-dependencies-v2", false, false),
    dealer_terminal_spec("dealer-liveness-schedule-v1", false, false),
    dealer_terminal_spec("liveness-policy", false, false),
    dealer_terminal_spec("liveness-source", false, false),
    dealer_terminal_spec("liveness-candidate", false, false),
    dealer_terminal_spec("liveness-clearing", false, false),
    dealer_terminal_spec("liveness-settlement", false, false),
    dealer_terminal_spec("liveness-resolution", false, false),
    dealer_terminal_spec("liveness-retirement", false, true),
    dealer_terminal_spec("liveness-recovery", false, false),
    dealer_terminal_spec("liveness-receipt", false, true),
    dealer_terminal_spec("liveness-payer", false, true),
    dealer_terminal_spec("position-rent-payer", false, true),
    dealer_terminal_spec("replay-rent-payer", false, true),
    dealer_terminal_spec("obligation-rent-payer", false, true),
    dealer_terminal_spec("neutral-lamport-sink", false, true),
    dealer_terminal_spec("clock-sysvar", false, false),
    dealer_terminal_spec("rent-sysvar", false, false),
    dealer_terminal_spec("system-program", false, false),
    dealer_terminal_spec("dealer-series-obligation-v2", false, true),
    dealer_terminal_spec("product-market-root-v2", false, false),
    dealer_terminal_spec("series-registry-v3", false, false),
    dealer_terminal_spec("current-program", false, false),
    dealer_terminal_spec("current-programdata", false, false),
    dealer_terminal_spec("registry-release-v2", false, false),
    dealer_terminal_spec("capability-profile-v4", false, false),
    dealer_terminal_spec("series-market-link-v2", false, true),
    dealer_terminal_spec("compiler-bundle-v6", false, false),
    dealer_terminal_spec("attachment-v5", false, false),
    dealer_terminal_spec("realm", false, false),
    dealer_terminal_spec("collateral-profile-v2", false, false),
    dealer_terminal_spec("collateral-policy-v2", false, false),
    dealer_terminal_spec("collateral-token-program", false, false),
    dealer_terminal_spec("collateral-token-programdata", false, false),
    dealer_terminal_spec("market-binding-v2", false, false),
    dealer_terminal_spec("market-runtime-v3", false, false),
    dealer_terminal_spec("market-instance-v2", false, false),
    dealer_terminal_spec("hoard-v2", false, false),
];

const DEALER_TERMINAL_ACTIVE_TAIL_V1: [DealerTerminalAccountSpecV1; 5] = [
    dealer_terminal_spec("claim-ledger-v3", false, true),
    dealer_terminal_spec("resolution-v5", false, false),
    dealer_terminal_spec("fractional-policy-v3", false, false),
    dealer_terminal_spec("fractional-ledger-v1", false, true),
    dealer_terminal_spec("facility-credit-v2", false, true),
];

const DEALER_TERMINAL_UNUSED_TAIL_V1: [DealerTerminalAccountSpecV1; 2] = [
    dealer_terminal_spec("claim-ledger-v3", false, false),
    dealer_terminal_spec("dealer-future-credit-funding-v1", false, true),
];

pub(crate) fn dealer_terminal_account_spec_v1(
    variant: crate::rpc_index::CanonicalIntentVariantV1,
    index: usize,
) -> Option<DealerTerminalAccountSpecV1> {
    if index < DEALER_TERMINAL_COMMON_V1.len() {
        return Some(DEALER_TERMINAL_COMMON_V1[index]);
    }
    let tail = index.checked_sub(DEALER_TERMINAL_COMMON_V1.len())?;
    match variant {
        crate::rpc_index::CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit => {
            DEALER_TERMINAL_ACTIVE_TAIL_V1.get(tail).copied()
        }
        crate::rpc_index::CanonicalIntentVariantV1::DealerRetireUnusedFutureCredit => {
            DEALER_TERMINAL_UNUSED_TAIL_V1.get(tail).copied()
        }
    }
}

pub(crate) const fn dealer_terminal_account_count_v1(
    variant: crate::rpc_index::CanonicalIntentVariantV1,
) -> usize {
    match variant {
        crate::rpc_index::CanonicalIntentVariantV1::DealerRetireActiveFacilityCredit => 48,
        crate::rpc_index::CanonicalIntentVariantV1::DealerRetireUnusedFutureCredit => 45,
    }
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

    /// Assemble one release-admitted Direct `80/1` request from its exact
    /// typed account projection. This constructor does not choose accounts or
    /// promote a disabled coordinate; its caller must first prove membership
    /// in the checked release manifest.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn enabled_direct_market_request_v1(
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

        if payload.action()
            == clutch_solana_layout::registry::DirectMarketAction::FinalizeSelection
        {
            return Err(ConstructionError::UnallocatedRegistryCoordinate);
        }
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
        let value = Self {
            flow: ProtocolFlow::DirectMarketV1,
            action_name: action_name.into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::EnabledDirectMarketRequestV1 { binding, sequence },
            data: request.bytes().to_vec(),
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble the exact current Direct action-8 request after hostile chain
    /// reconstruction of either physical handler frame. The caller cannot
    /// supply payload bytes or select another Direct coordinate.
    pub(crate) fn enabled_direct_finalize_selection_request_v2(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        keeper: Address,
        equations: Vec<ExactEquation>,
        sequence: u64,
    ) -> Result<Self> {
        use clutch_client_contract::direct_market::{
            DirectMarketClientPayloadV1, DirectMarketClientRequestV1,
        };
        use clutch_solana_layout::registry::DirectMarketAction;

        if sequence == 0
            || !matches!(accounts.len(), 9..=11 | 22)
            || keeper == Address::default()
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let payload = DirectMarketClientPayloadV1::empty(DirectMarketAction::FinalizeSelection)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        let action = ExtensionAction::DirectMarket(DirectMarketAction::FinalizeSelection);
        let binding = registry_binding(action.family(), action.local_tag(), Some(action))?;
        let request = DirectMarketClientRequestV1::encode(sequence, &payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        let value = Self {
            flow: ProtocolFlow::DirectMarketV1,
            action_name: "finalize-direct-selection-current-v2".to_string(),
            semantic_owner,
            program_id,
            accounts,
            required_signers: vec![keeper],
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::EnabledDirectFinalizeSelectionRequestV2 {
                binding,
                sequence,
            },
            data: request.bytes().to_vec(),
        };
        value.validate()?;
        Ok(value)
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

    /// Assemble the current exact internal Fractional redemption from bytes
    /// produced by the chain-state constructor. No account vector, signer, or
    /// payload shape can be weakened after this boundary.
    pub(crate) fn enabled_fractional_redeem_internal_exact_v1(
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        sequence: u64,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_fractional_redemption_runtime::{
            fractional_account_contract_v1, FractionalRedeemIntentV1,
            FractionalRedemptionActionV1,
        };

        let action = FractionalRedemptionActionV1::RedeemInternalExact;
        let contract = fractional_account_contract_v1(action);
        let intent = FractionalRedeemIntentV1::decode(payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        if sequence == 0
            || intent.expected_ledger_sequence != sequence
            || intent.expected_credit_sequence != 0
            || intent.expected_position_replay_sequence == 0
            || intent.claim_source != intent.payout_target
            || intent.credit_mode != 0
            || accounts.len() != usize::from(contract.account_count)
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let mut required_signers = Vec::new();
        let mut identities = BTreeSet::new();
        for (index, account) in accounts.iter().enumerate() {
            let mask = 1_u32
                .checked_shl(u32::try_from(index).map_err(|_| ConstructionError::InvalidAccountContract)?)
                .ok_or(ConstructionError::InvalidAccountContract)?;
            let expected_signer = contract.signer_mask & mask != 0;
            let expected_writable = contract.writable_mask & mask != 0;
            if account.is_signer != expected_signer
                || account.is_writable != expected_writable
                || !identities.insert(account.pubkey)
            {
                return Err(ConstructionError::InvalidAccountContract);
            }
            if expected_signer {
                required_signers.push(account.pubkey);
            }
        }
        if accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
            || accounts[11].pubkey.to_bytes() != intent.credit_or_policy.bytes()
            || accounts[13].pubkey.to_bytes() != intent.claim_source.bytes()
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let central_action = ExtensionAction::FractionalRedemption(action);
        let binding = registry_binding(
            ExtensionFamily::FractionalRedemption,
            action.tag(),
            Some(central_action),
        )?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family: ExtensionFamily::FractionalRedemption,
                action: central_action,
                payload,
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request
            .encode(&mut data)
            .map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::FractionalRedemption,
            action_name: action_name.into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::FractionalRedemptionRequestV1 {
                binding,
                action,
                sequence,
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble the permissionless chain-derived claims-exhausted seal. The
    /// caller cannot add a signer or alter the two writable ledger roles.
    pub(crate) fn enabled_fractional_seal_claims_exhausted_v1(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        sequence: u64,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_fractional_redemption_runtime::{
            fractional_account_contract_v1, FractionalRedemptionActionV1,
            FractionalTerminalIntentV1,
        };

        let action = FractionalRedemptionActionV1::SealClaimsExhausted;
        let contract = fractional_account_contract_v1(action);
        let intent = FractionalTerminalIntentV1::decode(payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        if sequence == 0
            || intent.expected_ledger_sequence != sequence
            || accounts.len() != usize::from(contract.account_count)
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let mut identities = BTreeSet::new();
        for (index, account) in accounts.iter().enumerate() {
            let mask = 1_u32 << index;
            if account.is_signer != (contract.signer_mask & mask != 0)
                || account.is_writable != (contract.writable_mask & mask != 0)
                || !identities.insert(account.pubkey)
            {
                return Err(ConstructionError::InvalidAccountContract);
            }
        }
        let central_action = ExtensionAction::FractionalRedemption(action);
        let binding = registry_binding(
            ExtensionFamily::FractionalRedemption,
            action.tag(),
            Some(central_action),
        )?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family: ExtensionFamily::FractionalRedemption,
                action: central_action,
                payload,
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request
            .encode(&mut data)
            .map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::FractionalRedemption,
            action_name: "seal-fractional-claims-exhausted".into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers: Vec::new(),
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::FractionalRedemptionRequestV1 {
                binding,
                action,
                sequence,
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble a zero-credit close from the decoded live credit and its
    /// persisted rent owner. The only permitted alias is claimant == payer,
    /// matching the SBF privilege-union contract.
    pub(crate) fn enabled_fractional_close_zero_credit_v1(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        sequence: u64,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_fractional_redemption_runtime::{
            FractionalCloseCreditIntentV1, FractionalRedemptionActionV1,
        };
        let action = FractionalRedemptionActionV1::CloseZeroCredit;
        let intent = FractionalCloseCreditIntentV1::decode(payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        if sequence == 0
            || intent.expected_ledger_sequence != sequence
            || accounts.len() != 18
            || accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
            || accounts[13].pubkey.to_bytes() != intent.credit_account.bytes()
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let payer_alias = accounts[0].pubkey == accounts[14].pubkey;
        for (index, account) in accounts.iter().enumerate() {
            let expected_writable = matches!(index, 9 | 12 | 13 | 14 | 16)
                || (index == 0 && payer_alias);
            let expected_signer = index == 0 || (index == 14 && payer_alias);
            if account.is_writable != expected_writable || account.is_signer != expected_signer {
                return Err(ConstructionError::InvalidAccountContract);
            }
            for other in index + 1..accounts.len() {
                if account.pubkey == accounts[other].pubkey && !(index == 0 && other == 14) {
                    return Err(ConstructionError::DuplicateAccount);
                }
            }
        }
        let central_action = ExtensionAction::FractionalRedemption(action);
        let binding = registry_binding(
            ExtensionFamily::FractionalRedemption,
            action.tag(),
            Some(central_action),
        )?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family: ExtensionFamily::FractionalRedemption,
                action: central_action,
                payload,
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request
            .encode(&mut data)
            .map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::FractionalRedemption,
            action_name: "close-fractional-zero-credit".into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers: vec![Address::new_from_array(intent.claimant.bytes())],
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::FractionalRedemptionRequestV1 {
                binding,
                action,
                sequence,
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble an internal-to-credit redemption from a hostile-decoded
    /// Position/Replay and live, fresh, or tombstoned owner-credit prestate.
    pub(crate) fn enabled_fractional_redeem_internal_credit_v1(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        sequence: u64,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_fractional_redemption_runtime::{
            FractionalRedeemIntentV1, FractionalRedemptionActionV1,
        };
        let action = FractionalRedemptionActionV1::RedeemInternalCredit;
        let intent = FractionalRedeemIntentV1::decode(payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        let creation = matches!(intent.credit_mode, 2 | 3);
        let expected_count = 19 + if creation { 2 } else { 0 };
        if sequence == 0
            || intent.expected_ledger_sequence != sequence
            || intent.expected_credit_sequence == 0
            || intent.expected_position_replay_sequence == 0
            || !(1..=3).contains(&intent.credit_mode)
            || intent.claim_source != intent.payout_target
            || accounts.len() != expected_count
            || accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
            || accounts[13].pubkey.to_bytes() != intent.claim_source.bytes()
            || accounts[15].pubkey.to_bytes() != intent.credit_or_policy.bytes()
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let payer_alias = creation && accounts[0].pubkey == accounts[19].pubkey;
        let mut required_signers = BTreeSet::new();
        for (index, account) in accounts.iter().enumerate() {
            let expected_writable = matches!(index, 8 | 9 | 12 | 13 | 14 | 15)
                || (creation && index == 19)
                || (index == 0 && payer_alias);
            let expected_signer = index == 0 || (creation && index == 19);
            if account.is_writable != expected_writable || account.is_signer != expected_signer {
                return Err(ConstructionError::InvalidAccountContract);
            }
            if expected_signer {
                required_signers.insert(account.pubkey);
            }
            for other in index + 1..accounts.len() {
                if account.pubkey == accounts[other].pubkey
                    && !(creation && index == 0 && other == 19)
                {
                    return Err(ConstructionError::DuplicateAccount);
                }
            }
        }
        let central_action = ExtensionAction::FractionalRedemption(action);
        let binding = registry_binding(
            ExtensionFamily::FractionalRedemption,
            action.tag(),
            Some(central_action),
        )?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family: ExtensionFamily::FractionalRedemption,
                action: central_action,
                payload,
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request
            .encode(&mut data)
            .map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::FractionalRedemption,
            action_name: "redeem-fractional-internal-credit".into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers: required_signers.into_iter().collect(),
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::FractionalRedemptionRequestV1 {
                binding,
                action,
                sequence,
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble an exact bearer burn/payout with the active mint suffix and
    /// correlated loader aliases fixed by authenticated chain state.
    pub(crate) fn enabled_fractional_redeem_bearer_exact_v1(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        sequence: u64,
        outcome_count: u8,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_fractional_redemption_runtime::{
            FractionalRedeemIntentV1, FractionalRedemptionActionV1,
        };
        let action = FractionalRedemptionActionV1::RedeemBearerExact;
        let intent = FractionalRedeemIntentV1::decode(payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        let expected_count = 21_usize
            .checked_add(usize::from(outcome_count))
            .ok_or(ConstructionError::InvalidAccountContract)?;
        if outcome_count < 2
            || intent.outcome >= outcome_count
            || sequence == 0
            || intent.expected_ledger_sequence != sequence
            || intent.expected_credit_sequence != 0
            || intent.expected_position_replay_sequence != 0
            || intent.credit_mode != 0
            || accounts.len() != expected_count
            || accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
            || accounts[11].pubkey.to_bytes() != intent.credit_or_policy.bytes()
            || accounts[14].pubkey.to_bytes() != intent.payout_target.bytes()
            || accounts[19].pubkey.to_bytes() != intent.claim_source.bytes()
            || (accounts[4].pubkey == accounts[17].pubkey)
                != (accounts[20].pubkey == accounts[18].pubkey)
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        for (index, account) in accounts.iter().enumerate() {
            let expected_writable = matches!(index, 8 | 9 | 12 | 14 | 16 | 19)
                || index == 21 + usize::from(intent.outcome);
            if account.is_writable != expected_writable || account.is_signer != (index == 0) {
                return Err(ConstructionError::InvalidAccountContract);
            }
            for other in index + 1..accounts.len() {
                let loader_alias = matches!((index, other), (4, 17) | (18, 20));
                if account.pubkey == accounts[other].pubkey && !loader_alias {
                    return Err(ConstructionError::DuplicateAccount);
                }
            }
        }
        let central_action = ExtensionAction::FractionalRedemption(action);
        let binding = registry_binding(ExtensionFamily::FractionalRedemption, action.tag(), Some(central_action))?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family: ExtensionFamily::FractionalRedemption,
                action: central_action,
                payload,
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request.encode(&mut data).map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::FractionalRedemption,
            action_name: "redeem-fractional-bearer-exact".into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers: vec![Address::new_from_array(intent.claimant.bytes())],
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::FractionalRedemptionRequestV1 { binding, action, sequence },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble a bearer redemption into live/fresh/reopened numerator credit.
    pub(crate) fn enabled_fractional_redeem_bearer_credit_v1(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        sequence: u64,
        outcome_count: u8,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_fractional_redemption_runtime::{
            FractionalRedeemIntentV1, FractionalRedemptionActionV1,
        };
        let action = FractionalRedemptionActionV1::RedeemBearerCredit;
        let intent = FractionalRedeemIntentV1::decode(payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        let creation = matches!(intent.credit_mode, 2 | 3);
        let credit_index = 21 + usize::from(outcome_count);
        let payer_index = credit_index + 4;
        let expected_count = credit_index + 4 + if creation { 2 } else { 0 };
        if outcome_count < 2
            || intent.outcome >= outcome_count
            || sequence == 0
            || intent.expected_ledger_sequence != sequence
            || intent.expected_credit_sequence == 0
            || intent.expected_position_replay_sequence != 0
            || !(1..=3).contains(&intent.credit_mode)
            || accounts.len() != expected_count
            || accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
            || accounts[14].pubkey.to_bytes() != intent.payout_target.bytes()
            || accounts[19].pubkey.to_bytes() != intent.claim_source.bytes()
            || accounts[credit_index].pubkey.to_bytes() != intent.credit_or_policy.bytes()
            || (accounts[4].pubkey == accounts[17].pubkey)
                != (accounts[20].pubkey == accounts[18].pubkey)
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let payer_alias = creation && accounts[0].pubkey == accounts[payer_index].pubkey;
        let mut required_signers = BTreeSet::new();
        for (index, account) in accounts.iter().enumerate() {
            let expected_writable = matches!(index, 8 | 9 | 12 | 14 | 16 | 19)
                || index == 21 + usize::from(intent.outcome)
                || index == credit_index
                || (creation && index == payer_index)
                || (index == 0 && payer_alias);
            let expected_signer = index == 0 || (creation && index == payer_index);
            if account.is_writable != expected_writable || account.is_signer != expected_signer {
                return Err(ConstructionError::InvalidAccountContract);
            }
            if expected_signer {
                required_signers.insert(account.pubkey);
            }
            for other in index + 1..accounts.len() {
                let loader_alias = matches!((index, other), (4, 17) | (18, 20));
                let payer_role_alias = creation && index == 0 && other == payer_index;
                if account.pubkey == accounts[other].pubkey && !loader_alias && !payer_role_alias {
                    return Err(ConstructionError::DuplicateAccount);
                }
            }
        }
        let central_action = ExtensionAction::FractionalRedemption(action);
        let binding = registry_binding(ExtensionFamily::FractionalRedemption, action.tag(), Some(central_action))?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family: ExtensionFamily::FractionalRedemption,
                action: central_action,
                payload,
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request.encode(&mut data).map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::FractionalRedemption,
            action_name: "redeem-fractional-bearer-credit".into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers: required_signers.into_iter().collect(),
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::FractionalRedemptionRequestV1 { binding, action, sequence },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Assemble a two-holder same-domain credit transfer or full-source merge.
    pub(crate) fn enabled_fractional_credit_move_v1(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        action: clutch_fractional_redemption_runtime::FractionalRedemptionActionV1,
        sequence: u64,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_fractional_redemption_runtime::{
            FractionalRedemptionActionV1, FractionalTransferIntentV1,
        };
        if !matches!(
            action,
            FractionalRedemptionActionV1::TransferCredit
                | FractionalRedemptionActionV1::MergeCredit
        ) {
            return Err(ConstructionError::WrongFlow);
        }
        let intent = FractionalTransferIntentV1::decode(payload)
            .map_err(|_| ConstructionError::WrongWirePrefix)?;
        let creation = matches!(intent.destination_mode, 2 | 3);
        let (root, neutral, rent) = match intent.payout_kind {
            1 => (18, 19, 20),
            2 => (21, 22, 23),
            _ => return Err(ConstructionError::InvalidAccountContract),
        };
        let funding = rent + 1;
        let expected_count = funding + if creation { 2 } else { 0 };
        if sequence == 0
            || intent.expected_ledger_sequence != sequence
            || (action == FractionalRedemptionActionV1::TransferCredit && intent.numerator == 0)
            || (action == FractionalRedemptionActionV1::MergeCredit && intent.numerator != 0)
            || (intent.payout_kind == 1 && intent.expected_payout_replay_sequence == 0)
            || (intent.payout_kind == 2 && intent.expected_payout_replay_sequence != 0)
            || accounts.len() != expected_count
            || accounts[0].pubkey.to_bytes() != intent.source_claimant.bytes()
            || accounts[1].pubkey.to_bytes() != intent.destination_claimant.bytes()
            || accounts[14].pubkey.to_bytes() != intent.source_credit.bytes()
            || accounts[15].pubkey.to_bytes() != intent.destination_credit.bytes()
            || accounts[16 + usize::from(intent.payout_kind == 2)].pubkey.to_bytes()
                != intent.payout_target.bytes()
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let payer_alias_source = creation && accounts[0].pubkey == accounts[funding].pubkey;
        let payer_alias_destination = creation && accounts[1].pubkey == accounts[funding].pubkey;
        let mut required_signers = BTreeSet::new();
        for (index, account) in accounts.iter().enumerate() {
            let payout_writable = if intent.payout_kind == 1 {
                matches!(index, 16 | 17)
            } else {
                matches!(index, 17 | 19)
            };
            let expected_writable = matches!(index, 9 | 10 | 13 | 14 | 15)
                || payout_writable
                || (creation && index == funding)
                || (index == 0 && payer_alias_source)
                || (index == 1 && payer_alias_destination);
            let expected_signer = matches!(index, 0 | 1) || (creation && index == funding);
            if account.is_writable != expected_writable || account.is_signer != expected_signer {
                return Err(ConstructionError::InvalidAccountContract);
            }
            if expected_signer {
                required_signers.insert(account.pubkey);
            }
            for other in index + 1..accounts.len() {
                let payer_alias = creation && other == funding && matches!(index, 0 | 1);
                if account.pubkey == accounts[other].pubkey && !payer_alias {
                    return Err(ConstructionError::DuplicateAccount);
                }
            }
        }
        let central_action = ExtensionAction::FractionalRedemption(action);
        let binding = registry_binding(ExtensionFamily::FractionalRedemption, action.tag(), Some(central_action))?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family: ExtensionFamily::FractionalRedemption,
                action: central_action,
                payload,
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request.encode(&mut data).map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::FractionalRedemption,
            action_name: if action == FractionalRedemptionActionV1::TransferCredit {
                "transfer-fractional-credit".into()
            } else {
                "merge-fractional-credit".into()
            },
            semantic_owner,
            program_id,
            accounts,
            required_signers: required_signers.into_iter().collect(),
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::FractionalRedemptionRequestV1 { binding, action, sequence },
            data,
        };
        value.validate()?;
        let _ = (root, neutral);
        Ok(value)
    }

    /// Assemble one current Structured wrapper call through the semantic
    /// owner's exact action/count/privilege contract. The wrapper program, not
    /// the central base, is the instruction target. Payload bytes must already
    /// have been derived by the chain-state constructor.
    pub(crate) fn enabled_structured_claim_v1(
        action_name: impl Into<String>,
        semantic_owner: SemanticOwner,
        wrapper_program: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        action: clutch_structured_claim_runtime_contract::StructuredClaimActionV1,
        product_link_writable: bool,
        payload: &[u8],
    ) -> Result<Self> {
        use clutch_structured_claim_adapter::{
            current_structured_account_meta_v1, current_structured_action_contract_v1,
            current_structured_alias_allowed_v1,
        };
        use clutch_structured_claim_runtime_contract::decode_structured_claim_payload_v1;

        let contract = current_structured_action_contract_v1(action)
            .ok_or(ConstructionError::InvalidAccountContract)?;
        if accounts.len() != usize::from(contract.account_count)
            || decode_structured_claim_payload_v1(action.tag(), payload).is_err()
        {
            return Err(ConstructionError::InvalidAccountContract);
        }
        let mut required_signers = Vec::new();
        let mut index = 0_usize;
        while index < accounts.len() {
            let expected = current_structured_account_meta_v1(
                action,
                index,
                product_link_writable,
            )
            .ok_or(ConstructionError::InvalidAccountContract)?;
            if accounts[index].is_signer != expected.signer
                || accounts[index].is_writable != expected.writable
            {
                return Err(ConstructionError::InvalidAccountContract);
            }
            if expected.signer {
                required_signers.push(accounts[index].pubkey);
            }
            let mut right = index + 1;
            while right < accounts.len() {
                if accounts[index].pubkey == accounts[right].pubkey
                    && !current_structured_alias_allowed_v1(action, index, right)
                {
                    return Err(ConstructionError::DuplicateAccount);
                }
                right += 1;
            }
            index += 1;
        }
        let wrapper_index = match action {
            clutch_structured_claim_runtime_contract::StructuredClaimActionV1::CreateDescriptor => 15,
            clutch_structured_claim_runtime_contract::StructuredClaimActionV1::WrapFull
            | clutch_structured_claim_runtime_contract::StructuredClaimActionV1::UnwrapFull
            | clutch_structured_claim_runtime_contract::StructuredClaimActionV1::RedeemTerminal => 14,
            clutch_structured_claim_runtime_contract::StructuredClaimActionV1::CompactDonation
            | clutch_structured_claim_runtime_contract::StructuredClaimActionV1::RetireDescriptor => 11,
        };
        if accounts[wrapper_index].pubkey != wrapper_program {
            return Err(ConstructionError::ForeignProgram);
        }
        let family = ExtensionFamily::StructuredClaim;
        let binding = registry_binding(family, action.tag(), None)?;
        let mut data = Vec::with_capacity(EXTENSION_ENVELOPE_BYTES + payload.len());
        data.push(family.tag());
        data.push(family.version());
        data.push(action.tag());
        data.extend_from_slice(payload);
        let value = Self {
            flow: ProtocolFlow::StructuredClaim,
            action_name: action_name.into(),
            semantic_owner,
            program_id: wrapper_program,
            accounts,
            required_signers,
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::ReleaseBoundEnabled,
            wire: OwnedWireContract::StructuredWrapperV1 {
                action,
                product_link_writable,
            },
            data,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct the exact central request for Dealer action 25 target 8 or
    /// 9. The payload was derived from finalized account bytes by the action
    /// material owner; this boundary rechecks its canonical fixed layout and
    /// exact role privileges before admitting the discriminator.
    pub(crate) fn enabled_dealer_terminal_retire_v1(
        semantic_owner: SemanticOwner,
        program_id: Address,
        accounts: Vec<AccountMeta>,
        equations: Vec<ExactEquation>,
        variant: crate::rpc_index::CanonicalIntentVariantV1,
        sequence: u64,
        payload: &[u8; 40],
    ) -> Result<Self> {
        let expected_count = dealer_terminal_account_count_v1(variant);
        if u64::from_le_bytes(
            payload[0..8]
                .try_into()
                .map_err(|_| ConstructionError::WrongWireLength)?,
        ) == 0
            || sequence == 0
            || accounts.len() != expected_count
            || payload[8..16] != sequence.to_le_bytes()
            || payload[16] != variant.payload_discriminator()
            || payload[17..24].iter().any(|byte| *byte != 0)
            || payload[28..32].iter().any(|byte| *byte != 0)
            || u32::from_le_bytes(
                payload[24..28]
                    .try_into()
                    .map_err(|_| ConstructionError::WrongWireLength)?,
            ) == 0
            || u64::from_le_bytes(
                payload[32..40]
                    .try_into()
                    .map_err(|_| ConstructionError::WrongWireLength)?,
            ) == 0
        {
            return Err(ConstructionError::WrongWireLength);
        }
        let mut required_signers = Vec::new();
        let mut index = 0_usize;
        while index < accounts.len() {
            let spec = dealer_terminal_account_spec_v1(variant, index)
                .ok_or(ConstructionError::InvalidAccountContract)?;
            if accounts[index].is_signer != spec.signer
                || accounts[index].is_writable != spec.writable
            {
                return Err(ConstructionError::InvalidAccountContract);
            }
            if spec.signer {
                required_signers.push(accounts[index].pubkey);
            }
            let mut peer = index + 1;
            while peer < accounts.len() {
                if accounts[index].pubkey == accounts[peer].pubkey {
                    let peer_spec = dealer_terminal_account_spec_v1(variant, peer)
                        .ok_or(ConstructionError::InvalidAccountContract)?;
                    let allowed = matches!(
                        (spec.label, peer_spec.label),
                        ("position-rent-payer", "replay-rent-payer")
                            | ("position-rent-payer", "obligation-rent-payer")
                            | ("replay-rent-payer", "obligation-rent-payer")
                            | ("actor", "position-rent-payer")
                            | ("actor", "replay-rent-payer")
                            | ("actor", "obligation-rent-payer")
                            | ("actor", "liveness-payer")
                            | ("liveness-payer", "position-rent-payer")
                            | ("liveness-payer", "replay-rent-payer")
                            | ("liveness-payer", "obligation-rent-payer")
                    );
                    if !allowed {
                        return Err(ConstructionError::DuplicateAccount);
                    }
                }
                peer += 1;
            }
            index += 1;
        }
        let family = ExtensionFamily::Dealer;
        let action = clutch_solana_layout::registry::DealerFacilityAction::Retire;
        let binding = registry_binding(
            family,
            action.tag(),
            Some(ExtensionAction::DealerFacility(action)),
        )?;
        let request = ExtensionRequest {
            sequence,
            envelope: ExtensionEnvelope {
                family,
                action: ExtensionAction::DealerFacility(action),
                payload: payload.to_vec(),
            },
        };
        let mut data = vec![0; 13 + EXTENSION_ENVELOPE_BYTES + payload.len()];
        let exact = request
            .encode(&mut data)
            .map_err(|_| ConstructionError::WrongWireLength)?;
        data.truncate(exact);
        let value = Self {
            flow: ProtocolFlow::DealerFacilityTerminal,
            action_name: variant.name().into(),
            semantic_owner,
            program_id,
            accounts,
            required_signers,
            equations,
            registry_binding: Some(binding),
            runtime_admission: RuntimeAdmission::PayloadVariantReleaseBoundEnabled,
            wire: OwnedWireContract::DealerTerminalRetireV1 { variant, sequence },
            data,
        };
        value.validate()?;
        Ok(value)
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
                | OwnedWireContract::EnabledDirectMarketRequestV1 { .. }
                | OwnedWireContract::EnabledDirectFinalizeSelectionRequestV2 { .. }
        );
        let fractional_request = matches!(
            self.wire,
            OwnedWireContract::FractionalRedemptionRequestV1 { .. }
        );
        let structured_wrapper = matches!(self.wire, OwnedWireContract::StructuredWrapperV1 { .. });
        let dealer_terminal = matches!(self.wire, OwnedWireContract::DealerTerminalRetireV1 { .. });
        if !source_request
            && !collateral_request
            && !direct_request
            && !fractional_request
            && !structured_wrapper
            && !dealer_terminal
        {
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
                    ProtocolFlow::FractionalRedemption => {
                        family == ExtensionFamily::FractionalRedemption
                    }
                    ProtocolFlow::StructuredClaim => family == ExtensionFamily::StructuredClaim,
                    ProtocolFlow::DealerFacilityTerminal => family == ExtensionFamily::Dealer,
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
                if sequence == 0
                    || request.sequence != sequence
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
                if !matches!(
                    action,
                    clutch_solana_layout::registry::SourceSeriesAction::InitializeHead
                        | clutch_solana_layout::registry::SourceSeriesAction::OpenRawPage
                        | clutch_solana_layout::registry::SourceSeriesAction::IngestBoundaryBatch
                ) {
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
            OwnedWireContract::FractionalRedemptionRequestV1 {
                binding,
                action,
                sequence,
            } => {
                use clutch_fractional_redemption_runtime::{
                    fractional_account_contract_v1, FractionalRedeemIntentV1,
                    FractionalRedemptionActionV1,
                };

                let request = ExtensionRequest::decode(&self.data)
                    .map_err(|_| ConstructionError::WrongWirePrefix)?;
                if !matches!(
                    action,
                    FractionalRedemptionActionV1::RedeemInternalExact
                        | FractionalRedemptionActionV1::RedeemBearerExact
                        | FractionalRedemptionActionV1::RedeemBearerCredit
                        | FractionalRedemptionActionV1::RedeemInternalCredit
                        | FractionalRedemptionActionV1::TransferCredit
                        | FractionalRedemptionActionV1::MergeCredit
                        | FractionalRedemptionActionV1::CloseZeroCredit
                        | FractionalRedemptionActionV1::SealClaimsExhausted
                )
                    || sequence == 0
                    || request.sequence != sequence
                    || request.envelope.family != ExtensionFamily::FractionalRedemption
                    || request.envelope.action != ExtensionAction::FractionalRedemption(action)
                    || self.registry_binding != Some(binding)
                    || binding.family != ExtensionFamily::FractionalRedemption
                    || binding.local_action != action.tag()
                    || binding.family.allocation_status() != Some(binding.family_status)
                    || binding.central_action != Some(ExtensionAction::FractionalRedemption(action))
                    || self.flow != ProtocolFlow::FractionalRedemption
                    || self.runtime_admission != RuntimeAdmission::ReleaseBoundEnabled
                {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                let contract = fractional_account_contract_v1(action);
                if !matches!(
                    action,
                    FractionalRedemptionActionV1::RedeemInternalCredit
                        | FractionalRedemptionActionV1::RedeemBearerExact
                        | FractionalRedemptionActionV1::RedeemBearerCredit
                        | FractionalRedemptionActionV1::TransferCredit
                        | FractionalRedemptionActionV1::MergeCredit
                )
                    && self.accounts.len() != usize::from(contract.account_count)
                {
                    return Err(ConstructionError::InvalidAccountContract);
                }
                match action {
                    FractionalRedemptionActionV1::RedeemInternalExact => {
                        let intent = FractionalRedeemIntentV1::decode(request.envelope.payload)
                            .map_err(|_| ConstructionError::WrongWirePrefix)?;
                        if intent.expected_ledger_sequence != sequence
                            || intent.expected_credit_sequence != 0
                            || intent.expected_position_replay_sequence == 0
                            || intent.claim_source != intent.payout_target
                            || intent.credit_mode != 0
                            || self.accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
                            || self.accounts[11].pubkey.to_bytes()
                                != intent.credit_or_policy.bytes()
                            || self.accounts[13].pubkey.to_bytes() != intent.claim_source.bytes()
                        {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                    }
                    FractionalRedemptionActionV1::SealClaimsExhausted => {
                        let intent = clutch_fractional_redemption_runtime::FractionalTerminalIntentV1::decode(
                            request.envelope.payload,
                        )
                        .map_err(|_| ConstructionError::WrongWirePrefix)?;
                        if intent.expected_ledger_sequence != sequence {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                    }
                    FractionalRedemptionActionV1::CloseZeroCredit => {
                        let intent = clutch_fractional_redemption_runtime::FractionalCloseCreditIntentV1::decode(
                            request.envelope.payload,
                        )
                        .map_err(|_| ConstructionError::WrongWirePrefix)?;
                        if intent.expected_ledger_sequence != sequence
                            || self.accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
                            || self.accounts[13].pubkey.to_bytes() != intent.credit_account.bytes()
                        {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                        let payer_alias = self.accounts[0].pubkey == self.accounts[14].pubkey;
                        for (index, account) in self.accounts.iter().enumerate() {
                            let expected_writable = matches!(index, 9 | 12 | 13 | 14 | 16)
                                || (index == 0 && payer_alias);
                            let expected_signer = index == 0 || (index == 14 && payer_alias);
                            if account.is_writable != expected_writable
                                || account.is_signer != expected_signer
                            {
                                return Err(ConstructionError::InvalidAccountContract);
                            }
                        }
                    }
                    FractionalRedemptionActionV1::RedeemInternalCredit => {
                        let intent = FractionalRedeemIntentV1::decode(request.envelope.payload)
                            .map_err(|_| ConstructionError::WrongWirePrefix)?;
                        let creation = matches!(intent.credit_mode, 2 | 3);
                        let expected_count = 19 + if creation { 2 } else { 0 };
                        if intent.expected_ledger_sequence != sequence
                            || intent.expected_credit_sequence == 0
                            || intent.expected_position_replay_sequence == 0
                            || !(1..=3).contains(&intent.credit_mode)
                            || intent.claim_source != intent.payout_target
                            || self.accounts.len() != expected_count
                            || self.accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
                            || self.accounts[13].pubkey.to_bytes() != intent.claim_source.bytes()
                            || self.accounts[15].pubkey.to_bytes()
                                != intent.credit_or_policy.bytes()
                        {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                        let payer_alias = creation
                            && self.accounts[0].pubkey == self.accounts[19].pubkey;
                        for (index, account) in self.accounts.iter().enumerate() {
                            let expected_writable = matches!(index, 8 | 9 | 12 | 13 | 14 | 15)
                                || (creation && index == 19)
                                || (index == 0 && payer_alias);
                            let expected_signer = index == 0 || (creation && index == 19);
                            if account.is_writable != expected_writable
                                || account.is_signer != expected_signer
                            {
                                return Err(ConstructionError::InvalidAccountContract);
                            }
                        }
                    }
                    FractionalRedemptionActionV1::RedeemBearerExact => {
                        let intent = FractionalRedeemIntentV1::decode(request.envelope.payload)
                            .map_err(|_| ConstructionError::WrongWirePrefix)?;
                        let outcome_count = self.accounts.len().checked_sub(21)
                            .and_then(|count| u8::try_from(count).ok())
                            .ok_or(ConstructionError::InvalidAccountContract)?;
                        if outcome_count < 2
                            || intent.outcome >= outcome_count
                            || intent.expected_ledger_sequence != sequence
                            || intent.expected_credit_sequence != 0
                            || intent.expected_position_replay_sequence != 0
                            || intent.credit_mode != 0
                            || self.accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
                            || self.accounts[11].pubkey.to_bytes()
                                != intent.credit_or_policy.bytes()
                            || self.accounts[14].pubkey.to_bytes() != intent.payout_target.bytes()
                            || self.accounts[19].pubkey.to_bytes() != intent.claim_source.bytes()
                            || (self.accounts[4].pubkey == self.accounts[17].pubkey)
                                != (self.accounts[20].pubkey == self.accounts[18].pubkey)
                        {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                        for (index, account) in self.accounts.iter().enumerate() {
                            let expected_writable = matches!(index, 8 | 9 | 12 | 14 | 16 | 19)
                                || index == 21 + usize::from(intent.outcome);
                            if account.is_writable != expected_writable
                                || account.is_signer != (index == 0)
                            {
                                return Err(ConstructionError::InvalidAccountContract);
                            }
                        }
                    }
                    FractionalRedemptionActionV1::RedeemBearerCredit => {
                        let intent = FractionalRedeemIntentV1::decode(request.envelope.payload)
                            .map_err(|_| ConstructionError::WrongWirePrefix)?;
                        let creation = matches!(intent.credit_mode, 2 | 3);
                        let suffix = if creation { 6 } else { 4 };
                        let outcome_count = self.accounts.len().checked_sub(21 + suffix)
                            .and_then(|count| u8::try_from(count).ok())
                            .ok_or(ConstructionError::InvalidAccountContract)?;
                        let credit_index = 21 + usize::from(outcome_count);
                        let payer_index = credit_index + 4;
                        if outcome_count < 2
                            || intent.outcome >= outcome_count
                            || intent.expected_ledger_sequence != sequence
                            || intent.expected_credit_sequence == 0
                            || intent.expected_position_replay_sequence != 0
                            || !(1..=3).contains(&intent.credit_mode)
                            || self.accounts[0].pubkey.to_bytes() != intent.claimant.bytes()
                            || self.accounts[14].pubkey.to_bytes() != intent.payout_target.bytes()
                            || self.accounts[19].pubkey.to_bytes() != intent.claim_source.bytes()
                            || self.accounts[credit_index].pubkey.to_bytes()
                                != intent.credit_or_policy.bytes()
                            || (self.accounts[4].pubkey == self.accounts[17].pubkey)
                                != (self.accounts[20].pubkey == self.accounts[18].pubkey)
                        {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                        let payer_alias = creation
                            && self.accounts[0].pubkey == self.accounts[payer_index].pubkey;
                        for (index, account) in self.accounts.iter().enumerate() {
                            let expected_writable = matches!(index, 8 | 9 | 12 | 14 | 16 | 19)
                                || index == 21 + usize::from(intent.outcome)
                                || index == credit_index
                                || (creation && index == payer_index)
                                || (index == 0 && payer_alias);
                            let expected_signer = index == 0 || (creation && index == payer_index);
                            if account.is_writable != expected_writable
                                || account.is_signer != expected_signer
                            {
                                return Err(ConstructionError::InvalidAccountContract);
                            }
                        }
                    }
                    FractionalRedemptionActionV1::TransferCredit
                    | FractionalRedemptionActionV1::MergeCredit => {
                        let intent = clutch_fractional_redemption_runtime::FractionalTransferIntentV1::decode(
                            request.envelope.payload,
                        )
                        .map_err(|_| ConstructionError::WrongWirePrefix)?;
                        let creation = matches!(intent.destination_mode, 2 | 3);
                        let rent = if intent.payout_kind == 1 { 20 } else { 23 };
                        let funding = rent + 1;
                        let expected_count = funding + if creation { 2 } else { 0 };
                        if intent.expected_ledger_sequence != sequence
                            || (action == FractionalRedemptionActionV1::TransferCredit
                                && intent.numerator == 0)
                            || (action == FractionalRedemptionActionV1::MergeCredit
                                && intent.numerator != 0)
                            || (intent.payout_kind == 1
                                && intent.expected_payout_replay_sequence == 0)
                            || (intent.payout_kind == 2
                                && intent.expected_payout_replay_sequence != 0)
                            || self.accounts.len() != expected_count
                            || self.accounts[0].pubkey.to_bytes()
                                != intent.source_claimant.bytes()
                            || self.accounts[1].pubkey.to_bytes()
                                != intent.destination_claimant.bytes()
                            || self.accounts[14].pubkey.to_bytes() != intent.source_credit.bytes()
                            || self.accounts[15].pubkey.to_bytes()
                                != intent.destination_credit.bytes()
                            || self.accounts[16 + usize::from(intent.payout_kind == 2)]
                                .pubkey
                                .to_bytes()
                                != intent.payout_target.bytes()
                        {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                        let payer_alias_source = creation
                            && self.accounts[0].pubkey == self.accounts[funding].pubkey;
                        let payer_alias_destination = creation
                            && self.accounts[1].pubkey == self.accounts[funding].pubkey;
                        for (index, account) in self.accounts.iter().enumerate() {
                            let payout_writable = if intent.payout_kind == 1 {
                                matches!(index, 16 | 17)
                            } else {
                                matches!(index, 17 | 19)
                            };
                            let expected_writable = matches!(index, 9 | 10 | 13 | 14 | 15)
                                || payout_writable
                                || (creation && index == funding)
                                || (index == 0 && payer_alias_source)
                                || (index == 1 && payer_alias_destination);
                            let expected_signer = matches!(index, 0 | 1)
                                || (creation && index == funding);
                            if account.is_writable != expected_writable
                                || account.is_signer != expected_signer
                            {
                                return Err(ConstructionError::InvalidAccountContract);
                            }
                        }
                    }
                    _ => return Err(ConstructionError::UnallocatedRegistryCoordinate),
                }
                for (index, account) in self.accounts.iter().enumerate() {
                    let mask = 1_u32
                        .checked_shl(u32::try_from(index).map_err(|_| ConstructionError::InvalidAccountContract)?)
                        .ok_or(ConstructionError::InvalidAccountContract)?;
                    if !matches!(
                        action,
                        FractionalRedemptionActionV1::CloseZeroCredit
                            | FractionalRedemptionActionV1::RedeemInternalCredit
                            | FractionalRedemptionActionV1::RedeemBearerExact
                            | FractionalRedemptionActionV1::RedeemBearerCredit
                            | FractionalRedemptionActionV1::TransferCredit
                            | FractionalRedemptionActionV1::MergeCredit
                    ) {
                        if account.is_signer != (contract.signer_mask & mask != 0)
                            || account.is_writable != (contract.writable_mask & mask != 0)
                        {
                            return Err(ConstructionError::InvalidAccountContract);
                        }
                    }
                    for other in index + 1..self.accounts.len() {
                        let allowed_close_payer_alias = action
                            == FractionalRedemptionActionV1::CloseZeroCredit
                            && index == 0
                            && other == 14;
                        let allowed_credit_payer_alias = action
                            == FractionalRedemptionActionV1::RedeemInternalCredit
                            && index == 0
                            && other == 19;
                        let allowed_bearer_loader_alias = matches!(
                            action,
                            FractionalRedemptionActionV1::RedeemBearerExact
                                | FractionalRedemptionActionV1::RedeemBearerCredit
                        ) && matches!((index, other), (4, 17) | (18, 20));
                        let allowed_bearer_credit_payer_alias = if action
                            == FractionalRedemptionActionV1::RedeemBearerCredit
                        {
                            let intent = FractionalRedeemIntentV1::decode(request.envelope.payload)
                                .map_err(|_| ConstructionError::WrongWirePrefix)?;
                            let creation = matches!(intent.credit_mode, 2 | 3);
                            let suffix = if creation { 6 } else { 4 };
                            let count = self.accounts.len().saturating_sub(21 + suffix);
                            creation && index == 0 && other == 25 + count
                        } else {
                            false
                        };
                        let allowed_move_payer_alias = if matches!(
                            action,
                            FractionalRedemptionActionV1::TransferCredit
                                | FractionalRedemptionActionV1::MergeCredit
                        ) {
                            let intent = clutch_fractional_redemption_runtime::FractionalTransferIntentV1::decode(
                                request.envelope.payload,
                            )
                            .map_err(|_| ConstructionError::WrongWirePrefix)?;
                            let creation = matches!(intent.destination_mode, 2 | 3);
                            let funding = if intent.payout_kind == 1 { 21 } else { 24 };
                            creation && other == funding && matches!(index, 0 | 1)
                        } else {
                            false
                        };
                        if account.pubkey == self.accounts[other].pubkey
                            && !allowed_close_payer_alias
                            && !allowed_credit_payer_alias
                            && !allowed_bearer_loader_alias
                            && !allowed_bearer_credit_payer_alias
                            && !allowed_move_payer_alias
                        {
                            return Err(ConstructionError::DuplicateAccount);
                        }
                    }
                }
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
            OwnedWireContract::EnabledDirectMarketRequestV1 { binding, sequence } => {
                let request = ExtensionRequest::decode(&self.data)
                    .map_err(|_| ConstructionError::WrongWirePrefix)?;
                if request.sequence != sequence
                    || request.envelope.family != ExtensionFamily::DirectMarket
                    || request.envelope.family != binding.family
                    || request.envelope.action.local_tag() != binding.local_action
                    || self.registry_binding != Some(binding)
                    || binding.family.allocation_status() != Some(binding.family_status)
                    || self.flow != ProtocolFlow::DirectMarketV1
                    || self.runtime_admission != RuntimeAdmission::ReleaseBoundEnabled
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
            OwnedWireContract::EnabledDirectFinalizeSelectionRequestV2 {
                binding,
                sequence,
            } => {
                let request = ExtensionRequest::decode(&self.data)
                    .map_err(|_| ConstructionError::WrongWirePrefix)?;
                if request.sequence != sequence
                    || request.envelope.family != ExtensionFamily::DirectMarket
                    || request.envelope.family != binding.family
                    || request.envelope.action.local_tag() != binding.local_action
                    || self.registry_binding != Some(binding)
                    || binding.family.allocation_status() != Some(binding.family_status)
                    || self.flow != ProtocolFlow::DirectMarketV1
                    || self.runtime_admission != RuntimeAdmission::ReleaseBoundEnabled
                    || !matches!(
                        request.envelope.action,
                        ExtensionAction::DirectMarket(
                            clutch_solana_layout::registry::DirectMarketAction::FinalizeSelection
                        )
                    )
                    || sequence == 0
                    || !matches!(self.accounts.len(), 9..=11 | 22)
                {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
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
            OwnedWireContract::StructuredWrapperV1 {
                action,
                product_link_writable,
            } => {
                use clutch_structured_claim_adapter::{
                    current_structured_account_meta_v1, current_structured_action_contract_v1,
                    current_structured_alias_allowed_v1,
                };
                let binding = self
                    .registry_binding
                    .ok_or(ConstructionError::UnallocatedRegistryCoordinate)?;
                if self.flow != ProtocolFlow::StructuredClaim
                    || self.runtime_admission != RuntimeAdmission::ReleaseBoundEnabled
                    || binding.family != ExtensionFamily::StructuredClaim
                    || binding.local_action != action.tag()
                    || binding.family_status
                        != ExtensionFamily::StructuredClaim
                            .allocation_status()
                            .ok_or(ConstructionError::UnallocatedRegistryCoordinate)?
                    || self.data.len() < EXTENSION_ENVELOPE_BYTES
                    || self.data[0] != ExtensionFamily::StructuredClaim.tag()
                    || self.data[1] != ExtensionFamily::StructuredClaim.version()
                    || self.data[2] != action.tag()
                    || clutch_structured_claim_runtime_contract::decode_structured_claim_payload_v1(
                        action.tag(),
                        &self.data[EXTENSION_ENVELOPE_BYTES..],
                    )
                    .is_err()
                {
                    return Err(ConstructionError::WrongWirePrefix);
                }
                let contract = current_structured_action_contract_v1(action)
                    .ok_or(ConstructionError::InvalidAccountContract)?;
                if self.accounts.len() != usize::from(contract.account_count) {
                    return Err(ConstructionError::InvalidAccountContract);
                }
                let mut left = 0_usize;
                while left < self.accounts.len() {
                    let expected = current_structured_account_meta_v1(
                        action,
                        left,
                        product_link_writable,
                    )
                    .ok_or(ConstructionError::InvalidAccountContract)?;
                    if self.accounts[left].is_signer != expected.signer
                        || self.accounts[left].is_writable != expected.writable
                    {
                        return Err(ConstructionError::InvalidAccountContract);
                    }
                    let mut right = left + 1;
                    while right < self.accounts.len() {
                        if self.accounts[left].pubkey == self.accounts[right].pubkey
                            && !current_structured_alias_allowed_v1(action, left, right)
                        {
                            return Err(ConstructionError::DuplicateAccount);
                        }
                        right += 1;
                    }
                    left += 1;
                }
            }
            OwnedWireContract::DealerTerminalRetireV1 { variant, sequence } => {
                let request = ExtensionRequest::decode(&self.data)
                    .map_err(|_| ConstructionError::WrongWirePrefix)?;
                let binding = self
                    .registry_binding
                    .ok_or(ConstructionError::UnallocatedRegistryCoordinate)?;
                let payload: &[u8; 40] = request
                    .envelope
                    .payload
                    .as_slice()
                    .try_into()
                    .map_err(|_| ConstructionError::WrongWireLength)?;
                if u64::from_le_bytes(
                    payload[0..8]
                        .try_into()
                        .map_err(|_| ConstructionError::WrongWireLength)?,
                ) == 0
                    || sequence == 0
                    || request.sequence != sequence
                    || request.envelope.family != ExtensionFamily::Dealer
                    || !matches!(
                        request.envelope.action,
                        ExtensionAction::DealerFacility(
                            clutch_solana_layout::registry::DealerFacilityAction::Retire
                        )
                    )
                    || binding.family != ExtensionFamily::Dealer
                    || binding.local_action
                        != clutch_solana_layout::registry::DealerFacilityAction::Retire.tag()
                    || self.flow != ProtocolFlow::DealerFacilityTerminal
                    || self.runtime_admission
                        != RuntimeAdmission::PayloadVariantReleaseBoundEnabled
                    || payload[8..16] != sequence.to_le_bytes()
                    || payload[16] != variant.payload_discriminator()
                    || payload[17..24].iter().any(|byte| *byte != 0)
                    || payload[28..32].iter().any(|byte| *byte != 0)
                    || u32::from_le_bytes(
                        payload[24..28]
                            .try_into()
                            .map_err(|_| ConstructionError::WrongWireLength)?,
                    ) == 0
                    || u64::from_le_bytes(
                        payload[32..40]
                            .try_into()
                            .map_err(|_| ConstructionError::WrongWireLength)?,
                    ) == 0
                    || self.accounts.len() != dealer_terminal_account_count_v1(variant)
                {
                    return Err(ConstructionError::UnallocatedRegistryCoordinate);
                }
                for (index, account) in self.accounts.iter().enumerate() {
                    let spec = dealer_terminal_account_spec_v1(variant, index)
                        .ok_or(ConstructionError::InvalidAccountContract)?;
                    if account.is_signer != spec.signer || account.is_writable != spec.writable {
                        return Err(ConstructionError::InvalidAccountContract);
                    }
                }
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

/// Solana message geometry used by one blockhash-free construction artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionMessageVersionV1 {
    /// Traditional transaction message with every key stored inline.
    Legacy,
    /// Version-zero message backed by one or more on-chain address tables.
    V0,
}

/// Exact finalized lookup-table observation used to compile a v0 message.
/// This is transport provenance, never a protocol account role or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressLookupTableUseV1 {
    /// Lookup-table account identity encoded in the v0 message.
    pub account: Address,
    /// Finalized slot at which the exact table body was observed.
    pub observed_slot: u64,
    /// Digest of the complete hostile-decoded table account observation.
    pub state_sha256: [u8; 32],
    /// Number of writable role addresses loaded from this table.
    pub writable_addresses: u16,
    /// Number of read-only role addresses loaded from this table.
    pub readonly_addresses: u16,
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
    pub message_version: TransactionMessageVersionV1,
    pub address_lookup_tables: Vec<AddressLookupTableUseV1>,
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
        if drafts.iter().any(|draft| {
            matches!(
                draft.wire,
                OwnedWireContract::StructuredWrapperV1 { .. }
                    | OwnedWireContract::DealerTerminalRetireV1 { .. }
            )
        }) {
            return Err(ConstructionError::MissingLookupTable);
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
                    | OwnedWireContract::FractionalRedemptionRequestV1 { .. }
                    | OwnedWireContract::DisabledDirectMarketRequestV1 { .. }
                    | OwnedWireContract::EnabledDirectMarketRequestV1 { .. }
                    | OwnedWireContract::EnabledDirectFinalizeSelectionRequestV2 { .. }
                    | OwnedWireContract::CollateralReplayRequestV3 { .. }
                    | OwnedWireContract::DealerTerminalRetireV1 { .. }
            ) && draft.program_id != self.clutch_program
            {
                return Err(ConstructionError::ForeignProgram);
            }
            if matches!(
                draft.wire,
                OwnedWireContract::FractionalRedemptionRequestV1 {
                    action: clutch_fractional_redemption_runtime::FractionalRedemptionActionV1::RedeemInternalExact,
                    ..
                }
            ) && draft.accounts.first().map(|account| account.pubkey) == Some(self.payer)
            {
                // The exact internal claimant role is read-only. Solana makes
                // the transaction payer globally writable after privilege
                // union, so that alias is not an executable transaction.
                return Err(ConstructionError::InvalidAccountContract);
            }
            if matches!(
                draft.wire,
                OwnedWireContract::FractionalRedemptionRequestV1 { .. }
            ) && draft
                .accounts
                .iter()
                .any(|account| account.pubkey == self.payer && !account.is_writable)
            {
                return Err(ConstructionError::InvalidAccountContract);
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
            message_version: TransactionMessageVersionV1::Legacy,
            address_lookup_tables: Vec::new(),
            serialized_transaction,
            has_recent_blockhash: false,
            signed: false,
            submitted: false,
        })
    }

    /// Compile exactly one current wide Structured or Dealer instruction as a
    /// v0 message. This crate-private seam is reachable only after the
    /// operator material constructor has hostile-decoded the finalized
    /// lookup-table account and proved complete role coverage.
    pub(crate) fn build_exact_v0(
        &self,
        draft: OwnedInstructionDraft,
        lookup_table: AddressLookupTableAccount,
        lookup_observed_slot: u64,
        lookup_state_sha256: [u8; 32],
    ) -> Result<UnsignedProtocolTransaction> {
        let supported = matches!(
            draft.wire,
            OwnedWireContract::StructuredWrapperV1 { .. }
                | OwnedWireContract::DealerTerminalRetireV1 { .. }
        );
        if !supported
            || !matches!(
                draft.flow,
                ProtocolFlow::StructuredClaim | ProtocolFlow::DealerFacilityTerminal
            )
            || lookup_table.key == Address::default()
            || lookup_table.addresses.is_empty()
            || lookup_table.addresses.len() > 256
            || lookup_observed_slot == 0
            || lookup_state_sha256 == [0; 32]
        {
            return Err(ConstructionError::InvalidLookupTable);
        }
        draft.validate()?;
        let mut table_addresses = BTreeSet::new();
        for address in &lookup_table.addresses {
            if *address == Address::default() || !table_addresses.insert(*address) {
                return Err(ConstructionError::InvalidLookupTable);
            }
        }
        for account in &draft.accounts {
            if account.pubkey == lookup_table.key || self.payer == lookup_table.key {
                return Err(ConstructionError::InvalidLookupTable);
            }
            if account.pubkey == self.payer && (!account.is_signer || !account.is_writable) {
                return Err(ConstructionError::InvalidAccountContract);
            }
            if !account.is_signer
                && account.pubkey != draft.program_id
                && !table_addresses.contains(&account.pubkey)
            {
                return Err(ConstructionError::InvalidLookupTable);
            }
        }
        let mut required_signers = BTreeSet::from([self.payer]);
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
        let instruction = draft.instruction();
        let message = v0::Message::try_compile(
            &self.payer,
            &[instruction],
            core::slice::from_ref(&lookup_table),
            Default::default(),
        )
        .map_err(|_| ConstructionError::InvalidLookupTable)?;
        let lookup = match message.address_table_lookups.as_slice() {
            [lookup] if lookup.account_key == lookup_table.key => lookup,
            _ => return Err(ConstructionError::InvalidLookupTable),
        };
        if lookup.writable_indexes.is_empty() && lookup.readonly_indexes.is_empty() {
            return Err(ConstructionError::InvalidLookupTable);
        }
        let lookup_use = AddressLookupTableUseV1 {
            account: lookup_table.key,
            observed_slot: lookup_observed_slot,
            state_sha256: lookup_state_sha256,
            writable_addresses: u16::try_from(lookup.writable_indexes.len())
                .map_err(|_| ConstructionError::InvalidLookupTable)?,
            readonly_addresses: u16::try_from(lookup.readonly_indexes.len())
                .map_err(|_| ConstructionError::InvalidLookupTable)?,
        };
        let signature_count = usize::from(message.header.num_required_signatures);
        if signature_count != required_signers.len() {
            return Err(ConstructionError::MissingSignerMeta);
        }
        let transaction = VersionedTransaction {
            signatures: vec![Default::default(); signature_count],
            message: VersionedMessage::V0(message),
        };
        let serialized_transaction =
            bincode::serialize(&transaction).map_err(|_| ConstructionError::Serialization)?;
        if serialized_transaction.len() > self.transport.packet_limit_bytes {
            return Err(ConstructionError::PacketTooLarge);
        }
        Ok(UnsignedProtocolTransaction {
            schema: CONSTRUCTION_PLAN_SCHEMA,
            flows: vec![draft.flow],
            actions: vec![draft.action_name],
            semantic_owners: vec![draft.semantic_owner],
            registry_bindings: vec![draft.registry_binding],
            runtime_admissions: vec![draft.runtime_admission],
            required_signers: required_signers.into_iter().collect(),
            exact_equations: draft.equations,
            message_version: TransactionMessageVersionV1::V0,
            address_lookup_tables: vec![lookup_use],
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
    fn structured_claim_wrapper_uses_exact_current_account_contract() {
        use clutch_structured_claim_adapter::{
            current_structured_account_meta_v1, STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1,
        };
        use clutch_structured_claim_runtime_contract::{
            StructuredClaimActionV1, WrapperQuantityPayloadV1,
        };

        let payer = Address::new_from_array([1; 32]);
        let program = Address::new_from_array([2; 32]);
        let action = StructuredClaimActionV1::WrapFull;
        let accounts = (0..STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1)
            .map(|index| {
                let expected = current_structured_account_meta_v1(action, index, false).unwrap();
                let address = if index == 14 {
                    program
                } else {
                    Address::new_from_array([u8::try_from(index + 3).unwrap(); 32])
                };
                if expected.writable {
                    AccountMeta::new(address, expected.signer)
                } else {
                    AccountMeta::new_readonly(address, expected.signer)
                }
            })
            .collect();
        let payload = WrapperQuantityPayloadV1 {
            wrapper_product_id: [7; 32],
            quantity: 9,
            user_generation: 2,
            user_replay_sequence: 3,
            vault_generation: 4,
            vault_replay_sequence: 5,
        }
        .encode()
        .unwrap();
        let draft = OwnedInstructionDraft::enabled_structured_claim_v1(
            "wrap-full",
            owner(),
            program,
            accounts,
            vec![ExactEquation {
                name: "full-vector backing".into(),
                unit: IntegerUnit::WrapperAtoms {
                    mint: Address::new_from_array([3; 32]),
                },
                left: 9,
                right: 9,
            }],
            action,
            false,
            &payload,
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
        assert_eq!(
            builder.build_atomic(core::slice::from_ref(&draft)),
            Err(ConstructionError::MissingLookupTable)
        );
        let lookup_addresses = draft
            .accounts
            .iter()
            .filter(|account| !account.is_signer && account.pubkey != program)
            .map(|account| account.pubkey)
            .collect();
        let plan = builder
            .build_exact_v0(
                draft,
                AddressLookupTableAccount {
                    key: Address::new_from_array([99; 32]),
                    addresses: lookup_addresses,
                },
                17,
                [6; 32],
            )
            .unwrap();
        assert_eq!(plan.message_version, TransactionMessageVersionV1::V0);
        assert_eq!(plan.address_lookup_tables.len(), 1);
        assert!(!plan.has_recent_blockhash);
        assert!(!plan.signed);
        assert!(!plan.submitted);
    }

    fn dealer_terminal_accounts(
        variant: crate::rpc_index::CanonicalIntentVariantV1,
    ) -> Vec<AccountMeta> {
        (0..dealer_terminal_account_count_v1(variant))
            .map(|index| {
                let spec = dealer_terminal_account_spec_v1(variant, index).unwrap();
                let byte = u8::try_from(index + 1).unwrap();
                let address = Address::new_from_array([byte; 32]);
                if spec.writable {
                    AccountMeta::new(address, spec.signer)
                } else {
                    AccountMeta::new_readonly(address, spec.signer)
                }
            })
            .collect()
    }

    fn dealer_terminal_payload(
        variant: crate::rpc_index::CanonicalIntentVariantV1,
    ) -> [u8; 40] {
        let mut payload = [0u8; 40];
        payload[0..8].copy_from_slice(&3u64.to_le_bytes());
        payload[8..16].copy_from_slice(&7u64.to_le_bytes());
        payload[16] = variant.payload_discriminator();
        payload[24..28].copy_from_slice(&1u32.to_le_bytes());
        payload[32..40].copy_from_slice(&9u64.to_le_bytes());
        payload
    }

    #[test]
    fn dealer_terminal_variants_have_disjoint_exact_tails_and_payloads() {
        use crate::rpc_index::CanonicalIntentVariantV1 as Variant;

        for variant in [
            Variant::DealerRetireActiveFacilityCredit,
            Variant::DealerRetireUnusedFutureCredit,
        ] {
            let draft = OwnedInstructionDraft::enabled_dealer_terminal_retire_v1(
                owner(),
                Address::new_from_array([90; 32]),
                dealer_terminal_accounts(variant),
                vec![ExactEquation {
                    name: "exact terminal keeper payment".into(),
                    unit: IntegerUnit::Lamports,
                    left: 9,
                    right: 9,
                }],
                variant,
                7,
                &dealer_terminal_payload(variant),
            )
            .unwrap();
            assert_eq!(draft.flow, ProtocolFlow::DealerFacilityTerminal);
            assert_eq!(
                draft.runtime_admission,
                RuntimeAdmission::PayloadVariantReleaseBoundEnabled
            );
        }

        let active = Variant::DealerRetireActiveFacilityCredit;
        let mut wrong_target = dealer_terminal_payload(active);
        wrong_target[16] = Variant::DealerRetireUnusedFutureCredit.payload_discriminator();
        assert_eq!(
            OwnedInstructionDraft::enabled_dealer_terminal_retire_v1(
                owner(),
                Address::new_from_array([90; 32]),
                dealer_terminal_accounts(active),
                vec![ExactEquation {
                    name: "exact terminal keeper payment".into(),
                    unit: IntegerUnit::Lamports,
                    left: 9,
                    right: 9,
                }],
                active,
                7,
                &wrong_target,
            ),
            Err(ConstructionError::WrongWireLength)
        );

        let mut reserved = dealer_terminal_payload(active);
        reserved[17] = 1;
        assert_eq!(
            OwnedInstructionDraft::enabled_dealer_terminal_retire_v1(
                owner(),
                Address::new_from_array([90; 32]),
                dealer_terminal_accounts(active),
                vec![ExactEquation {
                    name: "exact terminal keeper payment".into(),
                    unit: IntegerUnit::Lamports,
                    left: 9,
                    right: 9,
                }],
                active,
                7,
                &reserved,
            ),
            Err(ConstructionError::WrongWireLength)
        );

        let mut wrong_tail = dealer_terminal_accounts(active);
        wrong_tail.pop();
        assert_eq!(
            OwnedInstructionDraft::enabled_dealer_terminal_retire_v1(
                owner(),
                Address::new_from_array([90; 32]),
                wrong_tail,
                vec![ExactEquation {
                    name: "exact terminal keeper payment".into(),
                    unit: IntegerUnit::Lamports,
                    left: 9,
                    right: 9,
                }],
                active,
                7,
                &dealer_terminal_payload(active),
            ),
            Err(ConstructionError::WrongWireLength)
        );

        let mut zero_generation = dealer_terminal_payload(active);
        zero_generation[0..8].fill(0);
        assert_eq!(
            OwnedInstructionDraft::enabled_dealer_terminal_retire_v1(
                owner(),
                Address::new_from_array([90; 32]),
                dealer_terminal_accounts(active),
                vec![ExactEquation {
                    name: "exact terminal keeper payment".into(),
                    unit: IntegerUnit::Lamports,
                    left: 9,
                    right: 9,
                }],
                active,
                7,
                &zero_generation,
            ),
            Err(ConstructionError::WrongWireLength)
        );

        let mut sink_alias = dealer_terminal_accounts(active);
        sink_alias[20].pubkey = sink_alias[19].pubkey;
        assert_eq!(
            OwnedInstructionDraft::enabled_dealer_terminal_retire_v1(
                owner(),
                Address::new_from_array([90; 32]),
                sink_alias,
                vec![ExactEquation {
                    name: "exact terminal keeper payment".into(),
                    unit: IntegerUnit::Lamports,
                    left: 9,
                    right: 9,
                }],
                active,
                7,
                &dealer_terminal_payload(active),
            ),
            Err(ConstructionError::DuplicateAccount)
        );
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
