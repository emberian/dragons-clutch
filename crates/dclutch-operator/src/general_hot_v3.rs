//! Chain-derived General V3 Hot execution and packet construction.
//!
//! The operator never owns an action-specific account list. It authenticates
//! the selected General artifacts, expands the exact selected AccountProfile,
//! derives Product width from the finalized Product graph, and then compiles a
//! single unsigned v0 message through one exact canonical lookup table. It
//! performs no RPC, signing, submission, or account mutation.

use dclutch_account_profile_contract::lifecycle_v3::{
    LifecycleOperationV3, LifecycleRegistersV3, LifecycleSeedInputValueV3, SelectedLifecycleV3,
};
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_execution_strategy_contract::v2::{BankTransportV2, classify_bank_transport_v2};
use dclutch_general_adapter_contract::artifacts_v3::{
    GeneralArtifactBytesV3, GeneralArtifactSelectionV3, authenticate_general_artifacts_v3,
};
use dclutch_general_adapter_contract::{
    hot_candidate_v3::{identity as general_identity, scalar as general_scalar},
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    state_artifacts_v3::{
        GENERAL_CLOSE_PAYER_ACCOUNT_V3, GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
        encode_general_state_lifecycle_v3_atomic, general_child_account_start_v3,
        general_state_lifecycle_bytes_v3,
    },
};
use dclutch_general_codec::{
    Action,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Finality, Observation, ObservedAccount,
    product_graph_observation_v3::{
        AuthenticatedProductGraphObservationV3, FinalizedProductGraphAccountsV3,
        authenticate_product_graph_observation_v3,
    },
    versioned::{VersionedMessagePlanV0, compile_v0_message},
};

const HOT_RUNTIME_LOGICAL_PREFIX_V3: usize = 5;
const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = 8;
const ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3: usize = 6;

/// One exact finalized account plus its requested transaction privileges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralObservedAccountMetaV3 {
    /// Exact finalized account observation.
    pub account: ObservedAccount,
    /// Whether the top-level transaction requests signer privilege.
    pub is_signer: bool,
    /// Whether the top-level transaction requests writable privilege.
    pub is_writable: bool,
}

impl GeneralObservedAccountMetaV3 {
    fn meta(&self) -> AccountMeta {
        AccountMeta {
            pubkey: self.account.key,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}

/// Checked release evidence required before an operator recognizes General V3.
///
/// A chain reader cannot create this authority from self-consistent chain state
/// alone. A release checker constructs it after the selected immutable Trading
/// and admitted-accelerator ArtifactReleases match a user-supplied manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedGeneralHotReleaseV3 {
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Exact immutable admitted General accelerator ArtifactRelease identity.
    pub general_artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized physical state for one General Hot construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralHotStateV3 {
    /// Exact common physical Hot38 frame in canonical ABI order.
    pub fixed_accounts: Vec<GeneralObservedAccountMetaV3>,
    /// Exact admitted-AOT transport accounts between Hot38 and runtime state.
    pub strategy_accounts: Vec<GeneralObservedAccountMetaV3>,
    /// AccountProfile coordinates after the injected logical prefix
    /// `[root, config, Product, portfolio, linked-basis]`.
    pub runtime_suffix_accounts: Vec<GeneralObservedAccountMetaV3>,
    /// Immutable execution release set selected by Market.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Lowest finalized slot accepted for this construction attempt.
    pub minimum_finalized_slot: u64,
    /// Checked current Hot release, absent for unknown deployments.
    pub checked_release: Option<CheckedGeneralHotReleaseV3>,
}

/// Canonical action-state addresses derived from the authenticated lifecycle policy.
///
/// These values are an operator projection, never authority. Trading derives the
/// same addresses and bumps again before it creates, authenticates, or closes
/// either account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLifecycleProjectionV3 {
    /// Canonical selection or settlement state selected by the action.
    pub primary_state: Pubkey,
    /// Canonical primary-state PDA bump written into the request witness.
    pub primary_state_bump: u8,
    /// Close-only canonical terminal state.
    pub terminal_state: Option<Pubkey>,
    /// Close-only canonical terminal-state PDA bump.
    pub terminal_state_bump: Option<u8>,
    /// Close-only terminal coordinate, equal to the consumed revision plus one.
    pub terminal_coordinate: Option<u64>,
    /// First family child account after the exact lifecycle frame.
    pub child_account_start: u16,
}

/// Exact content identities of the complete authenticated General artifact graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralHotArtifactDigestsV3 {
    /// Action-selector CapabilityProgramSetV1.
    pub program_set: [u8; 32],
    /// Action-selected CapabilityProgramV3 descriptor.
    pub descriptor: [u8; 32],
    /// Immutable GeneralConfigV3.
    pub config: [u8; 32],
    /// Runtime-width AccountProfileV2.
    pub account_profile: [u8; 32],
    /// Protected StateLifecyclePolicyV3.
    pub lifecycle_policy: [u8; 32],
    /// Exact action RequestProfileV1.
    pub request_profile: [u8; 32],
    /// Selected ExecutionStrategyProgramV2.
    pub strategy: [u8; 32],
    /// Translation-equivalence certificate.
    pub certificate: [u8; 32],
    /// Registry admission for that certificate.
    pub admission: [u8; 32],
    /// Exact admitted TransitionProgramV3.
    pub transition: [u8; 32],
    /// Exact action EffectProgramV3.
    pub effect: [u8; 32],
}

/// Complete unsigned General instruction with exact checked provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralHotInstructionV3 {
    /// Exact unsigned Trading instruction.
    pub instruction: Instruction,
    /// Exact General action encoded in the family request.
    pub action: Action,
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Exact finalized observation shared by every input.
    pub observation: Observation,
    /// Wallet keys which the instruction itself requires to sign.
    pub required_instruction_signers: Vec<Pubkey>,
    /// Checked multiprogram manifest digest.
    pub checked_manifest_digest: [u8; 32],
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Exact admitted General accelerator ArtifactRelease identity.
    pub general_artifact_release: [u8; 32],
    /// Complete exact content identities selected by the artifact graph.
    pub artifacts: GeneralHotArtifactDigestsV3,
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Digest of the exact canonical family request.
    pub family_request_digest: [u8; 32],
    /// Canonical action-state projection derived from the lifecycle artifact.
    pub lifecycle: GeneralLifecycleProjectionV3,
}

/// Packet-safe unsigned General transaction plus its exact signer report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralHotTransactionPlanV3 {
    /// Packet-safe v0 message compiled through the exact canonical LUT.
    pub message: VersionedMessagePlanV0,
    /// Exact eventual wallet signer order, beginning with the fee payer.
    pub required_signers: Vec<Pubkey>,
    /// Exact General action carried by the instruction.
    pub action: Action,
    /// Checked multiprogram manifest digest.
    pub checked_manifest_digest: [u8; 32],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Exact immutable admitted General accelerator ArtifactRelease identity.
    pub general_artifact_release: [u8; 32],
    /// Complete exact content identities selected by the artifact graph.
    pub artifacts: GeneralHotArtifactDigestsV3,
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Canonical action-state projection carried by the request.
    pub lifecycle: GeneralLifecycleProjectionV3,
}

/// Stable refusal from General artifact, account, release, or packet checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralHotOperatorErrorV3 {
    /// The selected deployment was not recognized by checked release evidence.
    UnrecognizedRelease,
    /// A required content identity was zero or differed from selected bytes.
    ContentIdentity,
    /// Accounts did not share one sufficiently recent finalized snapshot.
    Snapshot,
    /// The common Hot38 physical frame differed.
    FixedFrame,
    /// Product/domain/portfolio finalization or composition differed.
    Product,
    /// The selected General artifact bundle refused.
    Artifact,
    /// Admitted-AOT transport geometry differed from the selected bank.
    StrategyGeometry,
    /// AccountProfile expansion, privilege, alias, or width differed.
    RuntimeGeometry,
    /// Lifecycle bytes, plan geometry, state shape, PDA, or bump differed.
    Lifecycle,
    /// Required signer reporting differed from the compiled message.
    Signer,
    /// The lookup table was not the exact canonical address set.
    LookupTable,
    /// Lookup-table or packet compilation refused.
    Routing(crate::versioned::Error),
    /// Checked arithmetic or encoding overflowed.
    Arithmetic,
}

/// Build one complete chain-derived General Hot instruction.
///
/// `request` is re-encoded canonically. Its two bump fields are untrusted
/// placeholders: this constructor replaces them with bumps derived from the
/// authenticated lifecycle policy and the exact observed state addresses. The
/// action-specific account width and privileges come only from the selected
/// AccountProfile; this operator carries no parallel per-action account table.
pub fn build_general_hot_instruction_v3(
    state: &GeneralHotStateV3,
    artifact_selection: GeneralArtifactSelectionV3,
    artifact_bytes: GeneralArtifactBytesV3<'_>,
    request: ControllerRequestV2,
) -> Result<GeneralHotInstructionV3, GeneralHotOperatorErrorV3> {
    let checked = state
        .checked_release
        .ok_or(GeneralHotOperatorErrorV3::UnrecognizedRelease)?;
    validate_release(checked, artifact_selection)?;
    let observation = validate_fixed_frame(state, checked)?;
    let product = authenticate_product_graph(state)?;
    let mut canonical_request = ControllerRequestV2 {
        state_bump: 0,
        terminal_record_bump: 0,
        ..request
    };
    let provisional_request_bytes = canonical_request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if provisional_request_bytes.len() != CONTROLLER_REQUEST_BYTES_V2 {
        return Err(GeneralHotOperatorErrorV3::Arithmetic);
    }
    let provisional_bundle = authenticate_general_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &provisional_request_bytes,
        product.outcome_count,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if provisional_bundle.request != canonical_request {
        return Err(GeneralHotOperatorErrorV3::Artifact);
    }
    let provisional_lifecycle = project_general_lifecycle_v3(
        state,
        provisional_bundle,
        canonical_request,
        checked.trading_program,
    )?;
    canonical_request.state_bump = provisional_lifecycle.primary_state_bump;
    canonical_request.terminal_record_bump = provisional_lifecycle
        .terminal_state_bump
        .unwrap_or_default();
    let request_bytes = canonical_request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    let bundle = authenticate_general_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &request_bytes,
        product.outcome_count,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if bundle.request != canonical_request {
        return Err(GeneralHotOperatorErrorV3::Artifact);
    }
    let lifecycle =
        project_general_lifecycle_v3(state, bundle, canonical_request, checked.trading_program)?;
    if lifecycle != provisional_lifecycle {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    validate_strategy_geometry(state, bundle)?;
    validate_runtime_geometry(state, bundle)?;

    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?
        .account
        .key;
    let root = state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request_bytes.len()).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        state.release_set,
        market.to_bytes(),
        state.generation,
        hash(&root.account.data).to_bytes(),
    )
    .map_err(|_| GeneralHotOperatorErrorV3::FixedFrame)?;
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(request_bytes.len())
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(&request_bytes);

    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|value| value.checked_add(state.runtime_suffix_accounts.len()))
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?,
    );
    accounts.extend(
        state
            .fixed_accounts
            .iter()
            .map(GeneralObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .strategy_accounts
            .iter()
            .map(GeneralObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .runtime_suffix_accounts
            .iter()
            .map(GeneralObservedAccountMetaV3::meta),
    );
    let required_instruction_signers = signer_keys(&accounts)?;
    let instruction = Instruction {
        program_id: checked.trading_program,
        accounts,
        data,
    };
    Ok(GeneralHotInstructionV3 {
        instruction,
        action: canonical_request.action,
        outcome_count: product.outcome_count,
        observation,
        required_instruction_signers,
        checked_manifest_digest: checked.checked_manifest_digest,
        trading_artifact_release: checked.trading_artifact_release,
        general_artifact_release: checked.general_artifact_release,
        artifacts: artifact_digests(artifact_bytes),
        product_record: product.product_record,
        family_request_digest: hash(&request_bytes).to_bytes(),
        lifecycle,
    })
}

/// Compile one General instruction into an unsigned packet-safe v0 message.
///
/// Exactly one finalized active LUT is accepted, and its address sequence must
/// equal [`canonical_general_lookup_addresses_v3`]. Extra addresses, alternate
/// ordering, and stale tables refuse even when Solana message compilation could
/// otherwise use them.
pub fn compile_general_hot_v0(
    report: &GeneralHotInstructionV3,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<GeneralHotTransactionPlanV3, GeneralHotOperatorErrorV3> {
    if payer == Pubkey::default()
        || report.observation.finality != Finality::Finalized
        || report.observation.slot == 0
        || lookup_table.observation != report.observation
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
    {
        return Err(GeneralHotOperatorErrorV3::Snapshot);
    }
    let expected = canonical_general_lookup_addresses_v3(&report.instruction, payer)?;
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| GeneralHotOperatorErrorV3::LookupTable)?;
    if table.addresses.as_ref() != expected.as_slice() {
        return Err(GeneralHotOperatorErrorV3::LookupTable);
    }
    let message = compile_v0_message(
        payer,
        core::slice::from_ref(&report.instruction),
        recent_blockhash,
        report.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(GeneralHotOperatorErrorV3::Routing)?;
    let mut required_signers = vec![payer];
    for signer in &report.required_instruction_signers {
        if !required_signers.contains(signer) {
            required_signers.push(*signer);
        }
    }
    if usize::from(message.required_signatures) != required_signers.len() {
        return Err(GeneralHotOperatorErrorV3::Signer);
    }
    Ok(GeneralHotTransactionPlanV3 {
        message,
        required_signers,
        action: report.action,
        checked_manifest_digest: report.checked_manifest_digest,
        outcome_count: report.outcome_count,
        trading_artifact_release: report.trading_artifact_release,
        general_artifact_release: report.general_artifact_release,
        artifacts: report.artifacts,
        product_record: report.product_record,
        lifecycle: report.lifecycle,
    })
}

/// Return the exact sorted, duplicate-free LUT address sequence for one General instruction.
pub fn canonical_general_lookup_addresses_v3(
    instruction: &Instruction,
    payer: Pubkey,
) -> Result<Vec<Pubkey>, GeneralHotOperatorErrorV3> {
    let mut signer_keys = vec![payer];
    for account in &instruction.accounts {
        if account.is_signer && !signer_keys.contains(&account.pubkey) {
            signer_keys.push(account.pubkey);
        }
    }
    let mut addresses = instruction
        .accounts
        .iter()
        .filter(|account| {
            !signer_keys.contains(&account.pubkey) && account.pubkey != instruction.program_id
        })
        .map(|account| account.pubkey)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > 256 {
        return Err(GeneralHotOperatorErrorV3::LookupTable);
    }
    Ok(addresses)
}

fn validate_release(
    checked: CheckedGeneralHotReleaseV3,
    selection: GeneralArtifactSelectionV3,
) -> Result<(), GeneralHotOperatorErrorV3> {
    if checked.trading_program == Pubkey::default()
        || checked.trading_artifact_release == [0; 32]
        || checked.general_artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || selection.program_set == [0; 32]
        || selection.config == [0; 32]
        || selection.artifact_release != checked.general_artifact_release
    {
        return Err(GeneralHotOperatorErrorV3::UnrecognizedRelease);
    }
    Ok(())
}

fn artifact_digests(bytes: GeneralArtifactBytesV3<'_>) -> GeneralHotArtifactDigestsV3 {
    GeneralHotArtifactDigestsV3 {
        program_set: hash(bytes.program_set).to_bytes(),
        descriptor: hash(bytes.descriptor).to_bytes(),
        config: hash(bytes.config).to_bytes(),
        account_profile: hash(bytes.account_profile).to_bytes(),
        lifecycle_policy: hash(bytes.lifecycle_policy).to_bytes(),
        request_profile: hash(bytes.request_profile).to_bytes(),
        strategy: hash(bytes.strategy).to_bytes(),
        certificate: hash(bytes.certificate).to_bytes(),
        admission: hash(bytes.admission).to_bytes(),
        transition: hash(bytes.transition).to_bytes(),
        effect: hash(bytes.effect).to_bytes(),
    }
}

fn validate_fixed_frame(
    state: &GeneralHotStateV3,
    checked: CheckedGeneralHotReleaseV3,
) -> Result<Observation, GeneralHotOperatorErrorV3> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.release_set == [0; 32]
        || state.minimum_finalized_slot == 0
    {
        return Err(GeneralHotOperatorErrorV3::FixedFrame);
    }
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let trading = state
        .fixed_accounts
        .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let registry = state
        .fixed_accounts
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let rent = state
        .fixed_accounts
        .get(HOT_RENT_SYSVAR_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let instructions = state
        .fixed_accounts
        .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    if market.account.key == Pubkey::default()
        || trading.account.key != checked.trading_program
        || !trading.account.executable
        || !registry.account.executable
        || rent.account.key != sysvar::rent::ID
        || instructions.account.key != sysvar::instructions::ID
    {
        return Err(GeneralHotOperatorErrorV3::FixedFrame);
    }
    let observation = market.account.observation;
    if observation.finality != Finality::Finalized
        || observation.slot < state.minimum_finalized_slot
    {
        return Err(GeneralHotOperatorErrorV3::Snapshot);
    }
    for (index, value) in state.fixed_accounts.iter().enumerate() {
        if value.account.observation != observation
            || value.is_signer
            || value.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(GeneralHotOperatorErrorV3::FixedFrame);
        }
    }
    for value in state
        .strategy_accounts
        .iter()
        .chain(&state.runtime_suffix_accounts)
    {
        if value.account.observation != observation
            || value.account.observation.finality != Finality::Finalized
        {
            return Err(GeneralHotOperatorErrorV3::Snapshot);
        }
    }
    Ok(observation)
}

fn authenticate_product_graph(
    state: &GeneralHotStateV3,
) -> Result<AuthenticatedProductGraphObservationV3, GeneralHotOperatorErrorV3> {
    let registry = state
        .fixed_accounts
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::Product)?
        .account
        .key;
    let account = |index: usize| {
        state
            .fixed_accounts
            .get(index)
            .map(|value| &value.account)
            .ok_or(GeneralHotOperatorErrorV3::Product)
    };
    authenticate_product_graph_observation_v3(FinalizedProductGraphAccountsV3 {
        registry_program: registry,
        product_raw: account(HOT_PRODUCT_RAW_ACCOUNT_V3)?,
        product_staging: account(HOT_PRODUCT_RAW_ACCOUNT_V3 + 1)?,
        domain_raw: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3)?,
        domain_staging: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3 + 1)?,
        portfolio_raw: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3)?,
        portfolio_staging: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3 + 1)?,
    })
    .map_err(|_| GeneralHotOperatorErrorV3::Product)
}

fn validate_strategy_geometry(
    state: &GeneralHotStateV3,
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<(), GeneralHotOperatorErrorV3> {
    let scalar_count = bundle
        .effect
        .scalar_count(bundle.tail_count)
        .map_err(|_| GeneralHotOperatorErrorV3::StrategyGeometry)?;
    let identity_count = bundle
        .effect
        .identity_count(bundle.tail_count)
        .map_err(|_| GeneralHotOperatorErrorV3::StrategyGeometry)?;
    let caller_count = match classify_bank_transport_v2(
        u32::try_from(scalar_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        u32::try_from(identity_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::StrategyGeometry)?
    {
        BankTransportV2::InlineReturnData { bank_bytes } if bank_bytes != 0 => 1_usize,
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } => {
            usize::try_from(page_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?
        }
        BankTransportV2::InlineReturnData { .. } => {
            return Err(GeneralHotOperatorErrorV3::StrategyGeometry);
        }
    };
    let expected = ADMITTED_AOT_FIXED_EXTRAS_V3
        .checked_add(caller_count)
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    if state.strategy_accounts.len() != expected
        || state
            .strategy_accounts
            .iter()
            .any(|account| account.is_signer || account.is_writable)
        || state
            .strategy_accounts
            .get(ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3)
            .is_none_or(|account| !account.account.executable)
    {
        return Err(GeneralHotOperatorErrorV3::StrategyGeometry);
    }
    Ok(())
}

fn validate_runtime_geometry(
    state: &GeneralHotStateV3,
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<(), GeneralHotOperatorErrorV3> {
    let profile = bundle.account_profile;
    let fixed = usize::from(profile.fixed_account_count());
    let stride = usize::from(profile.item_account_stride());
    let tail =
        usize::try_from(bundle.tail_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?;
    let total = stride
        .checked_mul(tail)
        .and_then(|value| fixed.checked_add(value))
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    if fixed < HOT_RUNTIME_LOGICAL_PREFIX_V3
        || state.runtime_suffix_accounts.len()
            != total
                .checked_sub(HOT_RUNTIME_LOGICAL_PREFIX_V3)
                .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)?
    {
        return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
    }
    for coordinate in 0..total {
        let account = logical_runtime_account(state, coordinate)?;
        let (item, local) = if coordinate < fixed {
            (false, coordinate)
        } else {
            if stride == 0 {
                return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
            }
            (true, (coordinate - fixed) % stride)
        };
        let rule = profile
            .rule(
                item,
                u16::try_from(local).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
            )
            .map_err(|_| GeneralHotOperatorErrorV3::RuntimeGeometry)?;
        let privileges = rule.privileges();
        if account.is_signer != (privileges & 1 != 0)
            || account.is_writable != (privileges & 2 != 0)
            || account.account.executable != (privileges & 4 != 0)
        {
            return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
        }
        let representative = profile
            .representative(bundle.tail_count, coordinate)
            .map_err(|_| GeneralHotOperatorErrorV3::RuntimeGeometry)?;
        if logical_runtime_account(state, representative)?.account.key != account.account.key {
            return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
        }
    }
    Ok(())
}

fn logical_runtime_account(
    state: &GeneralHotStateV3,
    coordinate: usize,
) -> Result<&GeneralObservedAccountMetaV3, GeneralHotOperatorErrorV3> {
    let fixed_index = match coordinate {
        0 => Some(HOT_ROOT_ACCOUNT_V3),
        1 => Some(HOT_CONFIG_RAW_ACCOUNT_V3),
        2 => Some(HOT_PRODUCT_RAW_ACCOUNT_V3),
        3 => Some(HOT_PORTFOLIO_RAW_ACCOUNT_V3),
        4 => Some(HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
        _ => None,
    };
    if let Some(index) = fixed_index {
        state
            .fixed_accounts
            .get(index)
            .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)
    } else {
        state
            .runtime_suffix_accounts
            .get(
                coordinate
                    .checked_sub(HOT_RUNTIME_LOGICAL_PREFIX_V3)
                    .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)?,
            )
            .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)
    }
}

fn project_general_lifecycle_v3(
    state: &GeneralHotStateV3,
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
    request: ControllerRequestV2,
    trading_program: Pubkey,
) -> Result<GeneralLifecycleProjectionV3, GeneralHotOperatorErrorV3> {
    let policy_bytes = general_state_lifecycle_bytes_v3(request.action)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let mut scratch = vec![0_u8; policy_bytes];
    let mut canonical = vec![0_u8; policy_bytes];
    encode_general_state_lifecycle_v3_atomic(request.action, &mut scratch, &mut canonical)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    if bundle.lifecycle_policy.bytes() != canonical.as_slice() {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }

    let plan_count = bundle
        .lifecycle_policy
        .action_plan_count(request.action as u32)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let expected_plan_count = if request.action == Action::Close {
        2
    } else {
        1
    };
    if plan_count != expected_plan_count {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let scalar_width = affine_register_width(
        bundle.account_profile.common_scalar_count(),
        bundle.account_profile.item_scalar_stride(),
        bundle.tail_count,
    )?;
    let identity_width = affine_register_width(
        bundle.account_profile.common_identity_count(),
        bundle.account_profile.item_identity_stride(),
        bundle.tail_count,
    )?;
    let mut scalars = vec![0_u64; scalar_width];
    let mut identities = vec![[0_u8; 32]; identity_width];
    let root = logical_runtime_account(state, 0)?.account.key;
    set_identity(
        &mut identities,
        general_identity::GENERAL_ROOT,
        root.to_bytes(),
    )?;
    set_identity(
        &mut identities,
        general_identity::CANDIDATE,
        request.candidate_id.unwrap_or([0; 32]),
    )?;
    let terminal_coordinate =
        canonical_terminal_coordinate_v3(request.action, request.expected_revision)?;
    if let Some(value) = terminal_coordinate {
        set_scalar(
            &mut scalars,
            general_scalar::CURSOR_TERMINAL_COORDINATE,
            value,
        )?;
    }
    let registers = LifecycleRegistersV3 {
        scalars: &scalars,
        identities: &identities,
    };
    let primary_plan = bundle
        .lifecycle_policy
        .action_plan(request.action as u32, 0)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let primary_expected_operation = if request.action == Action::Close {
        LifecycleOperationV3::Close
    } else {
        LifecycleOperationV3::AuthenticateOrCreate
    };
    let primary = derive_lifecycle_state_v3(
        state,
        bundle.account_profile,
        bundle.tail_count,
        registers,
        primary_plan,
        primary_expected_operation,
        usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        if request.action == Action::Close {
            None
        } else {
            Some(usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3))
        },
        Some(usize::from(if request.action == Action::Close {
            GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3
        } else {
            GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3
        })),
        trading_program,
        if matches!(request.action, Action::Consider | Action::Freeze) {
            GeneralLocalStateKindV3::Selection
        } else {
            GeneralLocalStateKindV3::Settlement
        },
    )?;

    let terminal = if request.action == Action::Close {
        let plan = bundle
            .lifecycle_policy
            .action_plan(request.action as u32, 1)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
        let terminal = derive_lifecycle_state_v3(
            state,
            bundle.account_profile,
            bundle.tail_count,
            registers,
            plan,
            LifecycleOperationV3::AuthenticateOrCreate,
            usize::from(GENERAL_TERMINAL_STATE_ACCOUNT_V3),
            Some(usize::from(GENERAL_CLOSE_PAYER_ACCOUNT_V3)),
            Some(usize::from(GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3)),
            trading_program,
            GeneralLocalStateKindV3::Settlement,
        )?;
        if terminal.key == primary.key {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
        Some(terminal)
    } else {
        None
    };
    let child_account_start = general_child_account_start_v3(request.action);
    let expected_child_start = if request.action == Action::Close {
        9
    } else {
        8
    };
    if child_account_start != expected_child_start {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let projection = GeneralLifecycleProjectionV3 {
        primary_state: primary.key,
        primary_state_bump: primary.bump,
        terminal_state: terminal.map(|value| value.key),
        terminal_state_bump: terminal.map(|value| value.bump),
        terminal_coordinate,
        child_account_start,
    };
    if (request.state_bump != 0 && request.state_bump != projection.primary_state_bump)
        || (request.terminal_record_bump != 0
            && Some(request.terminal_record_bump) != projection.terminal_state_bump)
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    Ok(projection)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedLifecycleStateV3 {
    key: Pubkey,
    bump: u8,
}

#[allow(clippy::too_many_arguments)]
fn derive_lifecycle_state_v3(
    state: &GeneralHotStateV3,
    profile: dclutch_account_profile_contract::v2::AccountProfileV2<'_>,
    tail_count: u32,
    registers: LifecycleRegistersV3<'_>,
    selected: SelectedLifecycleV3<'_>,
    expected_operation: LifecycleOperationV3,
    expected_state: usize,
    expected_payer: Option<usize>,
    expected_rent_credit: Option<usize>,
    trading_program: Pubkey,
    state_kind: GeneralLocalStateKindV3,
) -> Result<DerivedLifecycleStateV3, GeneralHotOperatorErrorV3> {
    if selected.operation() != expected_operation
        || selected.invocation_count(tail_count).ok() != Some(1)
        || selected.invocation_item(tail_count, 0).ok() != Some(None)
        || !selected
            .uses_canonical_bump()
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let indices = selected
        .project_account_indices(profile, tail_count, None)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    if indices.state() != expected_state
        || indices.payer() != expected_payer
        || indices.rent_credit() != expected_rent_credit
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let seed_count = selected
        .seed_count()
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let mut seed_values = Vec::with_capacity(
        usize::from(seed_count)
            .checked_sub(1)
            .ok_or(GeneralHotOperatorErrorV3::Lifecycle)?,
    );
    let mut saw_bump = false;
    for ordinal in 0..seed_count {
        match selected
            .materialize_seed_input(profile, tail_count, None, registers, ordinal)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
        {
            LifecycleSeedInputValueV3::Bytes(value) if !saw_bump && !value.is_empty() => {
                seed_values.push(value.as_slice().to_vec());
            }
            LifecycleSeedInputValueV3::CanonicalBump
                if !saw_bump && ordinal.checked_add(1) == Some(seed_count) =>
            {
                saw_bump = true;
            }
            LifecycleSeedInputValueV3::Bytes(_) | LifecycleSeedInputValueV3::CanonicalBump => {
                return Err(GeneralHotOperatorErrorV3::Lifecycle);
            }
        }
    }
    if !saw_bump || seed_values.is_empty() {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let seed_refs = seed_values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let (key, bump) = Pubkey::find_program_address(&seed_refs, &trading_program);
    let state_account = logical_runtime_account(state, indices.state())?;
    if state_account.account.key != key
        || state_account.account.key == Pubkey::default()
        || state_account.is_signer
        || !state_account.is_writable
        || state_account.account.executable
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let data_bytes = usize::try_from(
        selected
            .target_data_bytes(tail_count)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?;
    let live = state_account.account.owner == trading_program
        && state_account.account.data.len() == data_bytes;
    let vacant =
        state_account.account.owner == system_program::ID && state_account.account.data.is_empty();
    let accepted = match expected_operation {
        LifecycleOperationV3::Authenticate | LifecycleOperationV3::Close => live,
        LifecycleOperationV3::Create => vacant,
        LifecycleOperationV3::AuthenticateOrCreate => live || vacant,
    };
    if !accepted {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    if live {
        let decoded = GeneralLocalStateV3::decode(&state_account.account.data)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
        if decoded.header().bump != bump || decoded.header().kind != state_kind {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
    }
    validate_lifecycle_funding_accounts(
        state,
        indices.state(),
        indices.payer(),
        indices.rent_credit(),
    )?;
    Ok(DerivedLifecycleStateV3 { key, bump })
}

fn canonical_terminal_coordinate_v3(
    action: Action,
    expected_revision: u64,
) -> Result<Option<u64>, GeneralHotOperatorErrorV3> {
    if action == Action::Close {
        expected_revision
            .checked_add(1)
            .map(Some)
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)
    } else {
        Ok(None)
    }
}

fn validate_lifecycle_funding_accounts(
    state: &GeneralHotStateV3,
    state_index: usize,
    payer_index: Option<usize>,
    rent_credit_index: Option<usize>,
) -> Result<(), GeneralHotOperatorErrorV3> {
    let state_key = logical_runtime_account(state, state_index)?.account.key;
    let payer_key = payer_index
        .map(|index| {
            let payer = logical_runtime_account(state, index)?;
            if payer.account.key == Pubkey::default()
                || !payer.is_signer
                || !payer.is_writable
                || payer.account.executable
            {
                return Err(GeneralHotOperatorErrorV3::Lifecycle);
            }
            Ok(payer.account.key)
        })
        .transpose()?;
    let rent_credit_key = rent_credit_index
        .map(|index| {
            let credit = logical_runtime_account(state, index)?;
            if credit.account.key == Pubkey::default()
                || credit.is_signer
                || !credit.is_writable
                || credit.account.executable
            {
                return Err(GeneralHotOperatorErrorV3::Lifecycle);
            }
            Ok(credit.account.key)
        })
        .transpose()?;
    if payer_key == Some(state_key)
        || rent_credit_key == Some(state_key)
        || payer_key.is_some() && payer_key == rent_credit_key
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    Ok(())
}

fn affine_register_width(
    common: u16,
    stride: u16,
    tail_count: u32,
) -> Result<usize, GeneralHotOperatorErrorV3> {
    usize::from(stride)
        .checked_mul(
            usize::try_from(tail_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        )
        .and_then(|tail| usize::from(common).checked_add(tail))
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)
}

fn set_scalar(
    scalars: &mut [u64],
    coordinate: u32,
    value: u64,
) -> Result<(), GeneralHotOperatorErrorV3> {
    *scalars
        .get_mut(usize::try_from(coordinate).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?)
        .ok_or(GeneralHotOperatorErrorV3::Lifecycle)? = value;
    Ok(())
}

fn set_identity(
    identities: &mut [[u8; 32]],
    coordinate: u32,
    value: [u8; 32],
) -> Result<(), GeneralHotOperatorErrorV3> {
    *identities
        .get_mut(usize::try_from(coordinate).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?)
        .ok_or(GeneralHotOperatorErrorV3::Lifecycle)? = value;
    Ok(())
}

fn signer_keys(accounts: &[AccountMeta]) -> Result<Vec<Pubkey>, GeneralHotOperatorErrorV3> {
    let mut signers = Vec::new();
    for account in accounts.iter().filter(|account| account.is_signer) {
        if account.pubkey == Pubkey::default() {
            return Err(GeneralHotOperatorErrorV3::Signer);
        }
        if !signers.contains(&account.pubkey) {
            signers.push(account.pubkey);
        }
    }
    Ok(signers)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use solana_address_lookup_table_interface::state::LookupTableMeta;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn observation() -> Observation {
        Observation {
            slot: 500,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn report(data_bytes: usize) -> GeneralHotInstructionV3 {
        let actor = key(1);
        let mut accounts = vec![AccountMeta::new_readonly(actor, true)];
        accounts.extend((2_u8..92).map(|value| AccountMeta::new(key(value), false)));
        GeneralHotInstructionV3 {
            instruction: Instruction {
                program_id: key(200),
                accounts,
                data: vec![7; data_bytes],
            },
            action: Action::Collect,
            outcome_count: 258,
            observation: observation(),
            required_instruction_signers: vec![actor],
            checked_manifest_digest: [8; 32],
            trading_artifact_release: [9; 32],
            general_artifact_release: [10; 32],
            artifacts: GeneralHotArtifactDigestsV3 {
                program_set: [11; 32],
                descriptor: [12; 32],
                config: [13; 32],
                account_profile: [14; 32],
                lifecycle_policy: [15; 32],
                request_profile: [16; 32],
                strategy: [17; 32],
                certificate: [18; 32],
                admission: [19; 32],
                transition: [20; 32],
                effect: [21; 32],
            },
            product_record: [22; 32],
            family_request_digest: [23; 32],
            lifecycle: GeneralLifecycleProjectionV3 {
                primary_state: key(203),
                primary_state_bump: 7,
                terminal_state: None,
                terminal_state_bump: None,
                terminal_coordinate: None,
                child_account_start: 8,
            },
        }
    }

    fn lookup(report: &GeneralHotInstructionV3, payer: Pubkey) -> ObservedAccount {
        let addresses = canonical_general_lookup_addresses_v3(&report.instruction, payer)
            .expect("canonical addresses");
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(key(201)),
                last_extended_slot: observation().slot - 1,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: observation(),
            key: key(202),
            owner: lookup_table_program::id(),
            lamports: 1_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("lookup bytes"),
        }
    }

    fn lifecycle_funding_state() -> GeneralHotStateV3 {
        let observed = |value: u8, owner: Pubkey| ObservedAccount {
            observation: observation(),
            key: key(value),
            owner,
            lamports: 1_000_000,
            executable: false,
            data: Vec::new(),
        };
        GeneralHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: vec![
                GeneralObservedAccountMetaV3 {
                    account: observed(30, system_program::ID),
                    is_signer: false,
                    is_writable: true,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(31, system_program::ID),
                    is_signer: true,
                    is_writable: true,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(32, key(33)),
                    is_signer: false,
                    is_writable: true,
                },
            ],
            release_set: [1; 32],
            generation: 1,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        }
    }

    #[test]
    fn canonical_lut_compiles_packet_and_reports_payer_then_actor() {
        let report = report(192);
        let payer = key(250);
        let lookup = lookup(&report, payer);
        let plan = compile_general_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &lookup)
            .expect("packet-safe General action");
        assert_eq!(plan.required_signers, vec![payer, key(1)]);
        assert_eq!(plan.message.required_signatures, 2);
        assert!(plan.message.loaded_addresses >= 90);
        assert!(plan.message.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
        assert_eq!(plan.lifecycle, report.lifecycle);
    }

    #[test]
    fn stale_or_noncanonical_lookup_and_oversized_packet_refuse() {
        let payer = key(250);
        let canonical = report(192);
        let mut stale = lookup(&canonical, payer);
        stale.observation.slot += 1;
        assert_eq!(
            compile_general_hot_v0(&canonical, payer, Hash::new_from_array([16; 32]), &stale,),
            Err(GeneralHotOperatorErrorV3::Snapshot)
        );

        let mut extra = lookup(&canonical, payer);
        let decoded = AddressLookupTable::deserialize(&extra.data).expect("table");
        let mut addresses = decoded.addresses.into_owned();
        addresses.push(key(249));
        addresses.sort_unstable_by_key(Pubkey::to_bytes);
        let table = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(addresses),
        };
        extra.data = table.serialize_for_tests().expect("extra table");
        assert_eq!(
            compile_general_hot_v0(&canonical, payer, Hash::new_from_array([16; 32]), &extra,),
            Err(GeneralHotOperatorErrorV3::LookupTable)
        );

        let oversized = report(2_000);
        let lookup = lookup(&oversized, payer);
        assert_eq!(
            compile_general_hot_v0(&oversized, payer, Hash::new_from_array([16; 32]), &lookup,),
            Err(GeneralHotOperatorErrorV3::Routing(
                crate::versioned::Error::PacketTooLarge
            ))
        );
    }

    #[test]
    fn lifecycle_funding_requires_exact_signer_privileges_and_no_alias() {
        let state = lifecycle_funding_state();
        assert_eq!(
            validate_lifecycle_funding_accounts(&state, 5, Some(6), Some(7)),
            Ok(())
        );

        let mut unsigned = state.clone();
        unsigned
            .runtime_suffix_accounts
            .get_mut(1)
            .expect("payer")
            .is_signer = false;
        assert_eq!(
            validate_lifecycle_funding_accounts(&unsigned, 5, Some(6), Some(7)),
            Err(GeneralHotOperatorErrorV3::Lifecycle)
        );

        let mut signer_credit = state.clone();
        signer_credit
            .runtime_suffix_accounts
            .get_mut(2)
            .expect("RentCredit")
            .is_signer = true;
        assert_eq!(
            validate_lifecycle_funding_accounts(&signer_credit, 5, Some(6), Some(7)),
            Err(GeneralHotOperatorErrorV3::Lifecycle)
        );

        let mut alias = state;
        let state_key = alias
            .runtime_suffix_accounts
            .first()
            .expect("state")
            .account
            .key;
        alias
            .runtime_suffix_accounts
            .get_mut(2)
            .expect("RentCredit")
            .account
            .key = state_key;
        assert_eq!(
            validate_lifecycle_funding_accounts(&alias, 5, Some(6), Some(7)),
            Err(GeneralHotOperatorErrorV3::Lifecycle)
        );
    }

    #[test]
    fn terminal_coordinate_is_revision_successor_and_overflow_refuses() {
        assert_eq!(
            canonical_terminal_coordinate_v3(Action::Close, 41),
            Ok(Some(42))
        );
        assert_eq!(
            canonical_terminal_coordinate_v3(Action::Freeze, u64::MAX),
            Ok(None)
        );
        assert_eq!(
            canonical_terminal_coordinate_v3(Action::Close, u64::MAX),
            Err(GeneralHotOperatorErrorV3::Arithmetic)
        );
    }
}
