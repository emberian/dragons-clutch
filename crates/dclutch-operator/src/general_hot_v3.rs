//! Chain-derived General V3 Hot execution and packet construction.
//!
//! The operator never owns an action-specific account list. It authenticates
//! the selected General artifacts, expands the exact selected AccountProfile,
//! derives Product width from the finalized Product graph, and then compiles a
//! single unsigned v0 message through one exact canonical lookup table. It
//! performs no RPC, signing, submission, or account mutation.

use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HotExecutionEnvelopeV3,
};
use dclutch_execution_strategy_contract::v2::{BankTransportV2, classify_bank_transport_v2};
use dclutch_general_adapter_contract::artifacts_v3::{
    GeneralArtifactBytesV3, GeneralArtifactSelectionV3, authenticate_general_artifacts_v3,
};
use dclutch_general_codec::{
    Action,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_admission::{
    AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
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
    /// Action-selected CapabilityProgramV3 content digest.
    pub selected_program: [u8; 32],
    /// Selected CapabilityProgramSetV1 content digest.
    pub selected_program_set: [u8; 32],
    /// Selected GeneralConfigV3 content digest.
    pub selected_config: [u8; 32],
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Digest of the exact canonical family request.
    pub family_request_digest: [u8; 32],
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
/// `request` is re-encoded canonically. The action-specific account width and
/// privileges come only from the authenticated AccountProfile selected by that
/// request; this operator carries no parallel per-action account table.
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
    let request_bytes = request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if request_bytes.len() != CONTROLLER_REQUEST_BYTES_V2 {
        return Err(GeneralHotOperatorErrorV3::Arithmetic);
    }
    let bundle = authenticate_general_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &request_bytes,
        product.outcome_count,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if bundle.request != request {
        return Err(GeneralHotOperatorErrorV3::Artifact);
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
        action: request.action,
        outcome_count: product.outcome_count,
        observation,
        required_instruction_signers,
        checked_manifest_digest: checked.checked_manifest_digest,
        trading_artifact_release: checked.trading_artifact_release,
        general_artifact_release: checked.general_artifact_release,
        selected_program: hash(artifact_bytes.descriptor).to_bytes(),
        selected_program_set: artifact_selection.program_set,
        selected_config: artifact_selection.config,
        product_record: product.product_record,
        family_request_digest: hash(&request_bytes).to_bytes(),
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

#[derive(Clone, Copy)]
struct AuthenticatedProductWidthV3 {
    outcome_count: u32,
    product_record: [u8; 32],
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
) -> Result<AuthenticatedProductWidthV3, GeneralHotOperatorErrorV3> {
    let registry = state
        .fixed_accounts
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::Product)?
        .account
        .key;
    let product = finalized_coordinate(
        state,
        HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3 + 1,
        registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let domain = finalized_coordinate(
        state,
        HOT_PRODUCT_RAW_ACCOUNT_V3 + 2,
        HOT_PRODUCT_RAW_ACCOUNT_V3 + 3,
        registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio = finalized_coordinate(
        state,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3 + 1,
        registry,
        PORTFOLIO_SCHEMA_ID_V2,
    )?;
    let product_bytes = &state
        .fixed_accounts
        .get(HOT_PRODUCT_RAW_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::Product)?
        .account
        .data;
    let domain_bytes = &state
        .fixed_accounts
        .get(HOT_PRODUCT_RAW_ACCOUNT_V3 + 2)
        .ok_or(GeneralHotOperatorErrorV3::Product)?
        .account
        .data;
    let portfolio_bytes = &state
        .fixed_accounts
        .get(HOT_PORTFOLIO_RAW_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::Product)?
        .account
        .data;
    let admitted = admit_authenticated_records_v2(
        AdmissionReceiptV2 {
            product,
            result_domain: domain,
            portfolio,
        },
        product_bytes,
        domain_bytes,
        portfolio_bytes,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Product)?;
    Ok(AuthenticatedProductWidthV3 {
        outcome_count: admitted.join.outcome_count,
        product_record: admitted.product_record_digest.to_bytes(),
    })
}

fn finalized_coordinate(
    state: &GeneralHotStateV3,
    raw_index: usize,
    staging_index: usize,
    registry: Pubkey,
    schema: [u8; 32],
) -> Result<FinalizedRecordCoordinateV2, GeneralHotOperatorErrorV3> {
    let raw = state
        .fixed_accounts
        .get(raw_index)
        .ok_or(GeneralHotOperatorErrorV3::Product)?;
    let staging = state
        .fixed_accounts
        .get(staging_index)
        .ok_or(GeneralHotOperatorErrorV3::Product)?;
    let digest = ContentId::new(hash(&raw.account.data).to_bytes())
        .map_err(|_| GeneralHotOperatorErrorV3::Product)?;
    let (expected_raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest.to_bytes()],
        &registry,
    );
    let (expected_staging, _) = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest.to_bytes()],
        &registry,
    );
    if raw.account.key != expected_raw
        || raw.account.owner != registry
        || raw.account.executable
        || raw.account.data.is_empty()
        || raw.is_signer
        || raw.is_writable
        || staging.account.key != expected_staging
        || staging.account.owner != system_program::ID
        || staging.account.executable
        || !staging.account.data.is_empty()
        || staging.is_signer
        || staging.is_writable
    {
        return Err(GeneralHotOperatorErrorV3::Product);
    }
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ContentId::new(schema).map_err(|_| GeneralHotOperatorErrorV3::Product)?,
        content_digest: digest,
        raw_account: ContentId::new(expected_raw.to_bytes())
            .map_err(|_| GeneralHotOperatorErrorV3::Product)?,
        staging_account: ContentId::new(expected_staging.to_bytes())
            .map_err(|_| GeneralHotOperatorErrorV3::Product)?,
    })
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
            selected_program: [11; 32],
            selected_program_set: [12; 32],
            selected_config: [13; 32],
            product_record: [14; 32],
            family_request_digest: [15; 32],
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
}
