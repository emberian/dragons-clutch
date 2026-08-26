//! Chain-derived Direct V3 inline execution construction.
//!
//! This host-only adapter joins the canonical action-selected Direct artifact
//! bundle, expands the authenticated AccountProfile account space, and emits
//! the adjacent native-Ed25519 plus Trading instruction pair. It never performs
//! RPC, signs maker material, signs a transaction, or submits one.

use crate::{
    foundation::{authenticate_finalized_record, decode_rent, FinalizedRecordProof},
    product_graph_observation_v3::{
        authenticate_product_graph_observation_v3, AuthenticatedProductGraphObservationV3,
        FinalizedProductGraphAccountsV3,
    },
    Finality, Observation, ObservedAccount,
};
use dclutch_account_profile_contract::v2::AccountPrestateV2;
use dclutch_capability_contract::{CapabilityManifestV1, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1};
use dclutch_capability_program_contract::{
    hot_v3::{
        HotExecutionEnvelopeV3, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_STAGING_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_MANIFEST_RAW_ACCOUNT_V3, HOT_MANIFEST_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
        HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_STRATEGY_STAGING_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HOT_TRANSITION_RAW_ACCOUNT_V3, HOT_TRANSITION_STAGING_ACCOUNT_V3,
    },
    set_v2::{CapabilityProgramSetV2, CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2},
    v4::{
        ArtifactReferenceV4, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
    CapabilityRootHeaderV1, CAPABILITY_ROOT_HEADER_BYTES_V1,
};
use dclutch_direct_codec::{
    artifacts_v4::{
        authenticate_direct_artifacts_v4, DirectArtifactBundleV4, DirectArtifactBytesV4,
        DirectArtifactSelectionV4,
    },
    execution_v3::{
        encode_header_v3, DirectExecutionActionV3, DirectExecutionRequestV3,
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
    },
    intent_v2::{CompactIntentV2, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2},
};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{ed25519_program, sysvar};

use crate::versioned::{compile_v0_message, VersionedMessagePlanV0};

/// Exact Direct V3 InlineOrdinary family-request width.
pub const DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3: usize =
    DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 16;

const ED25519_DESCRIPTOR_BYTES: usize = 14;
const ED25519_SIGNATURES: usize = 2;
const ED25519_HEADER_BYTES: usize = 2 + ED25519_SIGNATURES * ED25519_DESCRIPTOR_BYTES;
const ED25519_PARTICIPANT_BYTES: usize = 32 + 64;
const CURRENT_HOT_INSTRUCTION_INDEX: u16 = 1;

/// One exact detached maker signature and its canonical signed intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDirectIntentV3 {
    /// Native Ed25519 maker public key.
    pub maker: Pubkey,
    /// Detached Ed25519 signature over `intent.signed_preimage()`.
    pub signature: [u8; 64],
    /// Exact runtime-width Direct V2 semantic intent.
    pub intent: CompactIntentV2,
}

/// One same-finalized account plus the privileges requested by the transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccountMetaV3 {
    /// Exact finalized account observation.
    pub account: ObservedAccount,
    /// Whether the transaction requests signer privilege.
    pub is_signer: bool,
    /// Whether the transaction requests writable privilege.
    pub is_writable: bool,
}

impl ObservedAccountMetaV3 {
    fn meta(&self) -> AccountMeta {
        AccountMeta {
            pubkey: self.account.key,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}

/// Checked-release evidence that the selected Trading artifact implements the
/// common V3 hot outer.
///
/// This value is not a hard-coded client constant. A chain/release checker must
/// construct it only after the selected immutable ArtifactRelease and current
/// Loader observations match a user-supplied checked manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedHotOuterReleaseV3 {
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact immutable Trading ArtifactRelease identity.
    pub artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized authority and exact physical account projection for one hot
/// Direct instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotStateV3 {
    /// Exact 38-account family-neutral prefix in canonical ABI order.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Exact disposition-selected ExecutionStrategy account suffix.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// Expanded AccountProfile physical address space, including the capability
    /// root/config/Product/portfolio/linked-basis logical prefix. Those five
    /// injected coordinates are not appended a second time.
    pub runtime_accounts: Vec<ObservedAccountMetaV3>,
    /// Immutable execution release-set content identity selected by Market.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Trusted Clock slot used for an exact economic preview.
    pub clock_slot: u64,
    /// Lowest finalized slot accepted for this construction attempt.
    pub minimum_finalized_slot: u64,
    /// Checked current hot outer, absent while the common entrypoint is not an
    /// accepted immutable release.
    pub hot_outer: Option<CheckedHotOuterReleaseV3>,
}

/// Exact economic preview derived from immutable Direct config and the request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineEconomicPreviewV3 {
    /// Claims transferred from seller to buyer.
    pub claim_transfer: u64,
    /// Exact gross collateral at the immutable price scale.
    pub gross_collateral: u64,
    /// Gross less the seller-side floor fee.
    pub seller_net_collateral_credit: u64,
    /// Gross plus the buyer-side floor fee.
    pub buyer_collateral_debit: u64,
    /// Sum of seller-withheld and buyer-added floor fees.
    pub total_fee_transfer: u64,
}

/// Complete unsigned adjacent-evidence execution material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotReportV3 {
    /// Native Ed25519 verification followed immediately by Trading.
    pub instructions: [Instruction; 2],
    /// Complete exact HotExecutionEnvelopeV3 plus Direct request bytes.
    pub hot_instruction_data: Vec<u8>,
    /// Same finalized observation selecting every physical account.
    pub observation: Observation,
    /// Schema of the action-selected CapabilityProgramV4 descriptor.
    pub selected_program_schema: [u8; 32],
    /// Action-selected CapabilityProgramV4 content digest.
    pub selected_program: [u8; 32],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
    /// Wallet keys which the Trading instruction requires to sign.
    pub required_instruction_signers: Vec<Pubkey>,
    /// Exact economic preview; onchain execution remains authoritative.
    pub preview: DirectInlineEconomicPreviewV3,
}

/// Exact unsigned Direct v0 transaction and its signer/provenance report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotTransactionPlanV3 {
    /// Packet-safe v0 message compiled through the sole canonical LUT.
    pub message: VersionedMessagePlanV0,
    /// Exact eventual wallet signer order, beginning with the fee payer.
    pub required_signers: Vec<Pubkey>,
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Schema of the action-selected CapabilityProgramV4 descriptor.
    pub selected_program_schema: [u8; 32],
    /// Action-selected CapabilityProgramV4 content digest.
    pub selected_program: [u8; 32],
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Stable refusal from canonical Direct transaction routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineTransactionErrorV3 {
    /// Payer, report, or LUT did not share one finalized observation.
    Snapshot,
    /// LUT bytes were not the one exact canonical address sequence.
    LookupTable,
    /// Instruction signer reporting differed from the compiled message.
    Signer,
    /// Lookup-table activation, message compilation, or packet sizing refused.
    Routing(crate::versioned::Error),
}

/// Stable refusal from stale authority, malformed signatures, artifact joins,
/// account-profile expansion, or transaction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The current selected immutable Trading release is not checked as a V3 hot outer.
    HotOuterUnavailable,
    /// A required identity, maker, or signature was zero.
    ZeroIdentity,
    /// Account observations were not finalized at one exact snapshot.
    ObservationMismatch,
    /// The family-neutral fixed frame or selected program identity differed.
    FixedFrameMismatch,
    /// Action-selected finalized artifacts did not form one Direct bundle.
    ArtifactMismatch,
    /// Runtime AccountProfile width or privileges differed.
    RuntimeProfileMismatch,
    /// The finalized Product/domain/portfolio graph refused.
    ProductGraphMismatch,
    /// Interpreted execution carried a nonempty accelerator transport suffix.
    StrategyGeometry,
    /// Intent, slot, price, fee, or quantity facts were incompatible.
    EconomicMismatch,
    /// Checked arithmetic or instruction encoding failed.
    Arithmetic,
}

/// Encode the sole canonical Direct V3 InlineOrdinary family request.
pub fn compile_direct_inline_request_v3(
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
) -> Result<[u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3], Error> {
    if seller.maker == Pubkey::default()
        || buyer.maker == Pubkey::default()
        || seller.maker == buyer.maker
        || seller.signature.iter().all(|byte| *byte == 0)
        || buyer.signature.iter().all(|byte| *byte == 0)
        || fill == 0
        || execution_price == 0
    {
        return Err(Error::ZeroIdentity);
    }
    let mut output = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3];
    let body = encode_header_v3(DirectExecutionActionV3::InlineOrdinary, &mut output)
        .map_err(|_| Error::Arithmetic)?;
    let seller_message = seller
        .intent
        .signed_preimage()
        .map_err(|_| Error::EconomicMismatch)?;
    let buyer_message = buyer
        .intent
        .signed_preimage()
        .map_err(|_| Error::EconomicMismatch)?;
    put(body, 0, seller.maker.as_ref())?;
    put(body, 32, &seller_message)?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        buyer.maker.as_ref(),
    )?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32,
        &buyer_message,
    )?;
    put(
        body,
        2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        &fill.to_le_bytes(),
    )?;
    put(
        body,
        2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 8,
        &execution_price.to_le_bytes(),
    )?;
    DirectExecutionRequestV3::decode(&output, u32::MAX).map_err(|_| Error::EconomicMismatch)?;
    Ok(output)
}

/// Build one complete chain-derived Direct inline batch without signing or submitting.
///
/// Artifact selection and bytes are derived exclusively from the finalized
/// accounts in `state`; callers cannot supply a parallel descriptor/config
/// selection or detached artifact bodies.
#[allow(clippy::too_many_arguments)]
pub fn build_direct_inline_hot_v4(
    state: &DirectInlineHotStateV3,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
) -> Result<DirectInlineHotReportV3, Error> {
    let checked = state.hot_outer.ok_or(Error::HotOuterUnavailable)?;
    if checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || state.release_set == [0; 32]
    {
        return Err(Error::ZeroIdentity);
    }
    let observation = validate_frame(state, checked)?;
    let product = authenticate_product_graph(state)?;
    let request = compile_direct_inline_request_v3(seller, buyer, fill, execution_price)?;
    let bundle = authenticate_chain_artifacts_v4(state, &request, product.outcome_count)?;
    if bundle.action != DirectExecutionActionV3::InlineOrdinary
        || !bundle.request_profile.requires_native_signature()
    {
        return Err(Error::ArtifactMismatch);
    }
    if !state.strategy_accounts.is_empty() {
        return Err(Error::StrategyGeometry);
    }
    validate_runtime_profile(state, bundle, product.outcome_count)?;
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?
        .account
        .key;
    let root = &state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?
        .account;
    let preview = preview_economics(
        market,
        state,
        bundle.config,
        seller,
        buyer,
        fill,
        execution_price,
        product.outcome_count,
    )?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).map_err(|_| Error::Arithmetic)?,
        state.release_set,
        market.to_bytes(),
        state.generation,
        hash(&root.data).to_bytes(),
    )
    .map_err(|_| Error::FixedFrameMismatch)?;
    let mut hot_instruction_data = Vec::with_capacity(HOT_FAMILY_REQUEST_OFFSET_V3 + request.len());
    hot_instruction_data.extend_from_slice(&envelope.to_bytes());
    hot_instruction_data.extend_from_slice(&request);

    let mut accounts = Vec::new();
    accounts.extend(state.fixed_accounts.iter().map(ObservedAccountMetaV3::meta));
    accounts.extend(
        state
            .strategy_accounts
            .iter()
            .map(ObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .runtime_accounts
            .iter()
            .skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
            .map(ObservedAccountMetaV3::meta),
    );
    let required_instruction_signers = signer_keys(&accounts)?;
    let trading = Instruction {
        program_id: checked.trading_program,
        accounts,
        data: hot_instruction_data.clone(),
    };
    let native = native_ed25519_instruction([seller, buyer])?;
    Ok(DirectInlineHotReportV3 {
        instructions: [native, trading],
        hot_instruction_data,
        observation,
        selected_program_schema: CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        selected_program: hash(
            &fixed_account(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?
                .account
                .data,
        )
        .to_bytes(),
        outcome_count: product.outcome_count,
        product_record: product.product_record,
        trading_artifact_release: checked.artifact_release,
        checked_manifest_digest: checked.checked_manifest_digest,
        required_instruction_signers,
        preview,
    })
}

fn authenticate_chain_artifacts_v4<'a>(
    state: &'a DirectInlineHotStateV3,
    request: &'a [u8],
    outcome_count: u32,
) -> Result<DirectArtifactBundleV4<'a>, Error> {
    let registry = fixed_account(state, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?
        .account
        .key;
    let rent = decode_rent(&fixed_account(state, HOT_RENT_SYSVAR_ACCOUNT_V3)?.account)
        .map_err(|_| Error::ArtifactMismatch)?;
    let root = &fixed_account(state, HOT_ROOT_ACCOUNT_V3)?.account;
    let header = CapabilityRootHeaderV1::decode(
        root.data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(Error::ArtifactMismatch)?,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    let trading = fixed_account(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?
        .account
        .key;
    let market = fixed_account(state, HOT_MARKET_ACCOUNT_V3)?.account.key;
    let seeds = header.seeds();
    if root.owner != trading
        || root.executable
        || header.release_set().to_bytes() != state.release_set
        || header.market() != market.to_bytes()
        || header.generation() != state.generation
        || header.selection().executor_role() != ExecutionRoleV1::Trading
        || Pubkey::find_program_address(&seeds.as_slices(), &trading).0 != root.key
    {
        return Err(Error::ArtifactMismatch);
    }
    let selection = header.selection();
    let manifest_data = finalized_record(
        state,
        registry,
        &rent,
        HOT_MANIFEST_RAW_ACCOUNT_V3,
        HOT_MANIFEST_STAGING_ACCOUNT_V3,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        selection.manifest().to_bytes(),
    )?;
    let manifest =
        CapabilityManifestV1::decode(manifest_data).map_err(|_| Error::ArtifactMismatch)?;
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(|_| Error::ArtifactMismatch)?;

    let program_set_data = finalized_record(
        state,
        registry,
        &rent,
        HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        selection.capability_release().to_bytes(),
    )?;
    let program_set = CapabilityProgramSetV2::decode_selected(
        selection.capability_release().to_bytes(),
        hash(program_set_data).to_bytes(),
        program_set_data,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    let descriptor_reference = program_set
        .select_descriptor(request)
        .map_err(|_| Error::ArtifactMismatch)?;
    if descriptor_reference.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4 {
        return Err(Error::ArtifactMismatch);
    }
    let descriptor_data = finalized_record(
        state,
        registry,
        &rent,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        descriptor_reference.program().to_bytes(),
    )?;
    let descriptor =
        CapabilityProgramV4::decode(descriptor_data).map_err(|_| Error::ArtifactMismatch)?;
    descriptor
        .validate_selection(selection, entry)
        .map_err(|_| Error::ArtifactMismatch)?;
    let expected_root_bytes = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(usize::try_from(descriptor.root_state_bytes()).map_err(|_| Error::Arithmetic)?)
        .ok_or(Error::Arithmetic)?;
    if root.data.len() != expected_root_bytes {
        return Err(Error::ArtifactMismatch);
    }

    let config = finalized_record(
        state,
        registry,
        &rent,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CONFIG_STAGING_ACCOUNT_V3,
        descriptor.config_schema().to_bytes(),
        selection.config().to_bytes(),
    )?;
    let account_profile = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
        descriptor.account_profile(),
    )?;
    let request_profile = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
        descriptor.request_profile(),
    )?;
    let transition = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_TRANSITION_STAGING_ACCOUNT_V3,
        descriptor.transition(),
    )?;
    let effect = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_EFFECT_STAGING_ACCOUNT_V3,
        descriptor.effect(),
    )?;
    let lifecycle_policy = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
        descriptor.lifecycle(),
    )?;
    let strategy = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_STRATEGY_STAGING_ACCOUNT_V3,
        descriptor.strategy(),
    )?;
    authenticate_direct_artifacts_v4(
        DirectArtifactSelectionV4 {
            program_set: selection.capability_release().to_bytes(),
            config: selection.config().to_bytes(),
        },
        DirectArtifactBytesV4 {
            program_set: program_set_data,
            descriptor: descriptor_data,
            config,
            account_profile,
            lifecycle_policy,
            request_profile,
            strategy,
            transition,
            effect,
        },
        request,
        outcome_count,
    )
    .map_err(|_| Error::ArtifactMismatch)
}

fn finalized_artifact<'a>(
    state: &'a DirectInlineHotStateV3,
    registry: Pubkey,
    rent: &solana_program::rent::Rent,
    raw_coordinate: usize,
    staging_coordinate: usize,
    reference: ArtifactReferenceV4,
) -> Result<&'a [u8], Error> {
    finalized_record(
        state,
        registry,
        rent,
        raw_coordinate,
        staging_coordinate,
        reference.schema().to_bytes(),
        reference.program().to_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
fn finalized_record<'a>(
    state: &'a DirectInlineHotStateV3,
    registry: Pubkey,
    rent: &solana_program::rent::Rent,
    raw_coordinate: usize,
    staging_coordinate: usize,
    schema: [u8; 32],
    expected_content: [u8; 32],
) -> Result<&'a [u8], Error> {
    let raw = &fixed_account(state, raw_coordinate)?.account;
    let staging = &fixed_account(state, staging_coordinate)?.account;
    authenticate_finalized_record(
        registry,
        rent,
        raw,
        &FinalizedRecordProof {
            schema_release_id: schema,
            staging_cursor: staging.clone(),
        },
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    if hash(&raw.data).to_bytes() != expected_content {
        return Err(Error::ArtifactMismatch);
    }
    Ok(&raw.data)
}

fn fixed_account(
    state: &DirectInlineHotStateV3,
    coordinate: usize,
) -> Result<&ObservedAccountMetaV3, Error> {
    state
        .fixed_accounts
        .get(coordinate)
        .ok_or(Error::FixedFrameMismatch)
}

/// Compile the exact adjacent pair through one canonical finalized LUT.
pub fn compile_direct_inline_hot_v0(
    report: &DirectInlineHotReportV3,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<DirectInlineHotTransactionPlanV3, DirectInlineTransactionErrorV3> {
    if payer == Pubkey::default()
        || report.observation.finality != Finality::Finalized
        || report.observation.slot == 0
        || report.selected_program_schema != CAPABILITY_PROGRAM_SCHEMA_ID_V4
        || report.trading_artifact_release == [0; 32]
        || report.checked_manifest_digest == [0; 32]
        || lookup_table.observation != report.observation
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
    {
        return Err(DirectInlineTransactionErrorV3::Snapshot);
    }
    let expected = canonical_direct_inline_lookup_addresses_v3(report, payer)?;
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| DirectInlineTransactionErrorV3::LookupTable)?;
    if table.addresses.as_ref() != expected.as_slice() {
        return Err(DirectInlineTransactionErrorV3::LookupTable);
    }
    let message = compile_v0_message(
        payer,
        &report.instructions,
        recent_blockhash,
        report.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(DirectInlineTransactionErrorV3::Routing)?;
    let mut required_signers = vec![payer];
    for signer in &report.required_instruction_signers {
        if !required_signers.contains(signer) {
            required_signers.push(*signer);
        }
    }
    if usize::from(message.required_signatures) != required_signers.len() {
        return Err(DirectInlineTransactionErrorV3::Signer);
    }
    Ok(DirectInlineHotTransactionPlanV3 {
        message,
        required_signers,
        outcome_count: report.outcome_count,
        selected_program_schema: report.selected_program_schema,
        selected_program: report.selected_program,
        trading_artifact_release: report.trading_artifact_release,
        checked_manifest_digest: report.checked_manifest_digest,
    })
}

/// Return the sole sorted, duplicate-free LUT address sequence for Direct.
pub fn canonical_direct_inline_lookup_addresses_v3(
    report: &DirectInlineHotReportV3,
    payer: Pubkey,
) -> Result<Vec<Pubkey>, DirectInlineTransactionErrorV3> {
    if payer == Pubkey::default() {
        return Err(DirectInlineTransactionErrorV3::Snapshot);
    }
    let mut signers = vec![payer];
    for signer in &report.required_instruction_signers {
        if *signer == Pubkey::default() {
            return Err(DirectInlineTransactionErrorV3::Signer);
        }
        if !signers.contains(signer) {
            signers.push(*signer);
        }
    }
    let program_ids = report
        .instructions
        .iter()
        .map(|instruction| instruction.program_id)
        .collect::<Vec<_>>();
    let mut addresses = report
        .instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
        .filter(|account| {
            !signers.contains(&account.pubkey) && !program_ids.contains(&account.pubkey)
        })
        .map(|account| account.pubkey)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > 256 {
        return Err(DirectInlineTransactionErrorV3::LookupTable);
    }
    Ok(addresses)
}

fn validate_frame(
    state: &DirectInlineHotStateV3,
    checked: CheckedHotOuterReleaseV3,
) -> Result<Observation, Error> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.minimum_finalized_slot == 0
        || state.runtime_accounts.len() < HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3
    {
        return Err(Error::FixedFrameMismatch);
    }
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let trading = state
        .fixed_accounts
        .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let rent = state
        .fixed_accounts
        .get(HOT_RENT_SYSVAR_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let instructions = state
        .fixed_accounts
        .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let registry = state
        .fixed_accounts
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    if trading.account.key != checked.trading_program
        || !trading.account.executable
        || !registry.account.executable
        || rent.account.key != sysvar::rent::ID
        || instructions.account.key != sysvar::instructions::ID
    {
        return Err(Error::FixedFrameMismatch);
    }
    let observation = market.account.observation;
    if observation.finality != Finality::Finalized
        || observation.slot < state.minimum_finalized_slot
    {
        return Err(Error::ObservationMismatch);
    }
    for (index, value) in state.fixed_accounts.iter().enumerate() {
        if value.account.observation != observation
            || value.is_signer
            || value.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(Error::FixedFrameMismatch);
        }
    }
    for value in state
        .strategy_accounts
        .iter()
        .chain(&state.runtime_accounts)
    {
        if value.account.observation.finality != Finality::Finalized
            || value.account.observation != observation
        {
            return Err(Error::ObservationMismatch);
        }
    }
    for (runtime, physical) in [
        (0, HOT_ROOT_ACCOUNT_V3),
        (1, HOT_CONFIG_RAW_ACCOUNT_V3),
        (2, HOT_PRODUCT_RAW_ACCOUNT_V3),
        (3, HOT_PORTFOLIO_RAW_ACCOUNT_V3),
        (4, HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
    ] {
        if state.runtime_accounts.get(runtime) != state.fixed_accounts.get(physical) {
            return Err(Error::RuntimeProfileMismatch);
        }
    }
    Ok(observation)
}

fn authenticate_product_graph(
    state: &DirectInlineHotStateV3,
) -> Result<AuthenticatedProductGraphObservationV3, Error> {
    let account = |index: usize| {
        state
            .fixed_accounts
            .get(index)
            .map(|value| &value.account)
            .ok_or(Error::ProductGraphMismatch)
    };
    authenticate_product_graph_observation_v3(FinalizedProductGraphAccountsV3 {
        registry_program: account(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?.key,
        product_raw: account(HOT_PRODUCT_RAW_ACCOUNT_V3)?,
        product_staging: account(HOT_PRODUCT_RAW_ACCOUNT_V3 + 1)?,
        domain_raw: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3)?,
        domain_staging: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3 + 1)?,
        portfolio_raw: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3)?,
        portfolio_staging: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3 + 1)?,
    })
    .map_err(|_| Error::ProductGraphMismatch)
}

fn validate_runtime_profile(
    state: &DirectInlineHotStateV3,
    bundle: dclutch_direct_codec::artifacts_v4::DirectArtifactBundleV4<'_>,
    outcome_count: u32,
) -> Result<(), Error> {
    let profile = bundle.account_profile;
    let fixed = usize::from(profile.fixed_account_count());
    let stride = usize::from(profile.item_account_stride());
    let tail = usize::try_from(outcome_count).map_err(|_| Error::Arithmetic)?;
    let expected = stride
        .checked_mul(tail)
        .and_then(|value| fixed.checked_add(value))
        .ok_or(Error::Arithmetic)?;
    if fixed < HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 || state.runtime_accounts.len() != expected {
        return Err(Error::RuntimeProfileMismatch);
    }
    for (coordinate, account) in state.runtime_accounts.iter().enumerate() {
        let (item, index) = if coordinate < fixed {
            (false, coordinate)
        } else {
            if stride == 0 {
                return Err(Error::RuntimeProfileMismatch);
            }
            (true, (coordinate - fixed) % stride)
        };
        let rule = profile
            .rule(item, u16::try_from(index).map_err(|_| Error::Arithmetic)?)
            .map_err(|_| Error::RuntimeProfileMismatch)?;
        let privileges = rule.privileges();
        let expected_data = usize::try_from(rule.data_length()).map_err(|_| Error::Arithmetic)?;
        if account.is_signer != (privileges & 1 != 0)
            || account.is_writable != (privileges & 2 != 0)
            || account.account.executable != (privileges & 4 != 0)
            || (account.account.data.len() != expected_data
                && !(rule.prestate() == AccountPrestateV2::LifecycleBound
                    && account.account.data.is_empty()))
        {
            return Err(Error::RuntimeProfileMismatch);
        }
        let representative = profile
            .representative(outcome_count, coordinate)
            .map_err(|_| Error::RuntimeProfileMismatch)?;
        if state
            .runtime_accounts
            .get(representative)
            .is_none_or(|canonical| canonical.account.key != account.account.key)
        {
            return Err(Error::RuntimeProfileMismatch);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn preview_economics(
    market: Pubkey,
    state: &DirectInlineHotStateV3,
    config: dclutch_direct_codec::successor::DirectExecutionConfigV1,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
    outcome_count: u32,
) -> Result<DirectInlineEconomicPreviewV3, Error> {
    for (participant, side) in [(seller, 0_u8), (buyer, 1_u8)] {
        let intent = participant.intent;
        if intent.side != side
            || intent.lifecycle > 1
            || intent.market != market.to_bytes()
            || intent.generation != state.generation
            || intent.outcome >= outcome_count
            || intent.maximum_fill < fill
            || intent.fee_basis_points != config.fee_basis_points()
            || state.clock_slot < intent.valid_from
            || state.clock_slot > intent.valid_through
        {
            return Err(Error::EconomicMismatch);
        }
        if intent.lifecycle == 0 && intent.maximum_fill != fill {
            return Err(Error::EconomicMismatch);
        }
    }
    if seller.intent.outcome != buyer.intent.outcome
        || execution_price < seller.intent.limit_price
        || execution_price > buyer.intent.limit_price
        || execution_price > config.price_scale()
    {
        return Err(Error::EconomicMismatch);
    }
    let scaled = u128::from(fill)
        .checked_mul(u128::from(execution_price))
        .ok_or(Error::Arithmetic)?;
    let scale = u128::from(config.price_scale());
    if scaled % scale != 0 {
        return Err(Error::EconomicMismatch);
    }
    let gross = u64::try_from(scaled / scale).map_err(|_| Error::Arithmetic)?;
    let fee = u64::try_from(
        u128::from(gross)
            .checked_mul(u128::from(config.fee_basis_points()))
            .ok_or(Error::Arithmetic)?
            / 10_000,
    )
    .map_err(|_| Error::Arithmetic)?;
    Ok(DirectInlineEconomicPreviewV3 {
        claim_transfer: fill,
        gross_collateral: gross,
        seller_net_collateral_credit: gross.checked_sub(fee).ok_or(Error::Arithmetic)?,
        buyer_collateral_debit: gross.checked_add(fee).ok_or(Error::Arithmetic)?,
        total_fee_transfer: fee.checked_mul(2).ok_or(Error::Arithmetic)?,
    })
}

fn native_ed25519_instruction(
    participants: [SignedDirectIntentV3; ED25519_SIGNATURES],
) -> Result<Instruction, Error> {
    let payload_bytes = ED25519_SIGNATURES
        .checked_mul(ED25519_PARTICIPANT_BYTES)
        .and_then(|value| ED25519_HEADER_BYTES.checked_add(value))
        .ok_or(Error::Arithmetic)?;
    let mut data = vec![0_u8; payload_bytes];
    *data.first_mut().ok_or(Error::Arithmetic)? =
        u8::try_from(ED25519_SIGNATURES).map_err(|_| Error::Arithmetic)?;
    for (index, participant) in participants.iter().enumerate() {
        let descriptor = 2 + index * ED25519_DESCRIPTOR_BYTES;
        let public_key = ED25519_HEADER_BYTES + index * ED25519_PARTICIPANT_BYTES;
        let signature = public_key + 32;
        let family_offset = if index == 0 { 64 } else { 268 };
        let message = HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family_offset)
            .ok_or(Error::Arithmetic)?;
        for (offset, value) in [
            (descriptor, signature),
            (descriptor + 2, usize::from(u16::MAX)),
            (descriptor + 4, public_key),
            (descriptor + 6, usize::from(u16::MAX)),
            (descriptor + 8, message),
            (descriptor + 10, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2),
            (descriptor + 12, usize::from(CURRENT_HOT_INSTRUCTION_INDEX)),
        ] {
            put(
                &mut data,
                offset,
                &u16::try_from(value)
                    .map_err(|_| Error::Arithmetic)?
                    .to_le_bytes(),
            )?;
        }
        put(&mut data, public_key, participant.maker.as_ref())?;
        put(&mut data, signature, &participant.signature)?;
    }
    Ok(Instruction {
        program_id: ed25519_program::ID,
        accounts: Vec::new(),
        data,
    })
}

fn signer_keys(accounts: &[AccountMeta]) -> Result<Vec<Pubkey>, Error> {
    let mut signers = Vec::new();
    for account in accounts.iter().filter(|account| account.is_signer) {
        if account.pubkey == Pubkey::default() {
            return Err(Error::ZeroIdentity);
        }
        if !signers.contains(&account.pubkey) {
            signers.push(account.pubkey);
        }
    }
    Ok(signers)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Arithmetic)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use dclutch_capability_contract::{
        ActivationPolicy, CapabilityEntryV1, CompartmentFundingV1, FundingAmountsV1,
        FundingQuoteV1, CAPABILITY_ENTRY_BYTES, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_capability_program_contract::set_v2::{
        encode_program_set_v2, encoded_program_set_bytes_v2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, SelectorWidthV2,
    };
    use dclutch_custody_contract::CustodyReplayLayoutV1;
    use dclutch_direct_codec::{
        ordinary_account_artifacts_v3::DirectInlineOrdinaryAccountProfileInputV3,
        ordinary_bundle_v4::{
            build_direct_inline_ordinary_hot_bundle_v4, DirectInlineOrdinaryHotBundleInputV4,
        },
        ordinary_effect_artifacts_v3::DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
        successor::{
            DirectExecutionConfigV1, DirectRootStateV1, DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
            DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_SCHEMA_ID_V1,
        },
    };
    use dclutch_product_runtime_v2::{
        DOMAIN_CUT_BYTES, DOMAIN_HEADER_BYTES, PORTFOLIO_COEFFICIENT_BYTES, PORTFOLIO_HEADER_BYTES,
    };
    use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
    use dclutch_realm_contract::REALM_BYTES;
    use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
    use solana_address_lookup_table_interface::state::LookupTableMeta;
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};
    use solana_sdk_ids::system_program;

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

    fn intent(side: u8, maker_byte: u8) -> SignedDirectIntentV3 {
        SignedDirectIntentV3 {
            maker: Pubkey::new_from_array([maker_byte; 32]),
            signature: [maker_byte; 64],
            intent: CompactIntentV2 {
                side,
                lifecycle: 1,
                outcome: 70_000,
                market: [7; 32],
                generation: 9,
                nonce: 3,
                valid_from: 100,
                valid_through: 200,
                maximum_fill: 1_000,
                limit_price: if side == 0 { 400_000 } else { 600_000 },
                fee_basis_points: 25,
                collateral_account: [maker_byte + 10; 32],
            },
        }
    }

    fn transaction_report(data_bytes: usize) -> DirectInlineHotReportV3 {
        let actor = key(1);
        let mut accounts = vec![AccountMeta::new_readonly(actor, true)];
        accounts.extend((2_u8..92).map(|value| AccountMeta::new(key(value), false)));
        DirectInlineHotReportV3 {
            instructions: [
                Instruction {
                    program_id: ed25519_program::ID,
                    accounts: Vec::new(),
                    data: vec![3; 32],
                },
                Instruction {
                    program_id: key(200),
                    accounts,
                    data: vec![7; data_bytes],
                },
            ],
            hot_instruction_data: vec![7; data_bytes],
            observation: observation(),
            selected_program_schema: CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            selected_program: [8; 32],
            outcome_count: 258,
            product_record: [9; 32],
            trading_artifact_release: [10; 32],
            checked_manifest_digest: [11; 32],
            required_instruction_signers: vec![actor],
            preview: DirectInlineEconomicPreviewV3 {
                claim_transfer: 10,
                gross_collateral: 5,
                seller_net_collateral_credit: 4,
                buyer_collateral_debit: 6,
                total_fee_transfer: 2,
            },
        }
    }

    fn lookup(report: &DirectInlineHotReportV3, payer: Pubkey) -> ObservedAccount {
        let addresses = canonical_direct_inline_lookup_addresses_v3(report, payer)
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

    fn hot38_state() -> (DirectInlineHotStateV3, CheckedHotOuterReleaseV3) {
        let checked = CheckedHotOuterReleaseV3 {
            trading_program: key(200),
            artifact_release: [20; 32],
            checked_manifest_digest: [21; 32],
        };
        let mut fixed_accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| ObservedAccountMetaV3 {
                account: ObservedAccount {
                    observation: observation(),
                    key: key(u8::try_from(index + 100).expect("test key")),
                    owner: key(220),
                    lamports: 1,
                    executable: false,
                    data: vec![0],
                },
                is_signer: false,
                is_writable: index == HOT_ROOT_ACCOUNT_V3,
            })
            .collect::<Vec<_>>();
        let trading = fixed_accounts
            .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .expect("Trading coordinate");
        trading.account.key = checked.trading_program;
        trading.account.executable = true;
        fixed_accounts
            .get_mut(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
            .expect("Registry coordinate")
            .account
            .executable = true;
        fixed_accounts
            .get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3)
            .expect("Rent coordinate")
            .account
            .key = sysvar::rent::ID;
        fixed_accounts
            .get_mut(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .expect("Instructions coordinate")
            .account
            .key = sysvar::instructions::ID;
        let runtime_accounts = [
            HOT_ROOT_ACCOUNT_V3,
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        ]
        .map(|index| {
            fixed_accounts
                .get(index)
                .expect("injected coordinate")
                .clone()
        })
        .into_iter()
        .collect();
        (
            DirectInlineHotStateV3 {
                fixed_accounts,
                strategy_accounts: Vec::new(),
                runtime_accounts,
                release_set: [22; 32],
                generation: 1,
                clock_slot: observation().slot,
                minimum_finalized_slot: observation().slot,
                hot_outer: Some(checked),
            },
            checked,
        )
    }

    fn ordinary_logical_lengths() -> Vec<u32> {
        let mut output = vec![0_u32; usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)];
        *output.get_mut(0).expect("root") = u32::try_from(
            CAPABILITY_ROOT_HEADER_BYTES_V1
                + dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1,
        )
        .expect("root width");
        *output.get_mut(1).expect("config") =
            u32::try_from(dclutch_direct_codec::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1)
                .expect("config width");
        *output.get_mut(2).expect("Product") =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product width");
        *output.get_mut(3).expect("portfolio") =
            u32::try_from(PORTFOLIO_HEADER_BYTES + 3 * PORTFOLIO_COEFFICIENT_BYTES)
                .expect("portfolio width");
        *output.get_mut(4).expect("basis") = 24;
        for coordinate in [5_usize, 8] {
            *output.get_mut(coordinate).expect("maker") =
                u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker width");
        }
        *output.get_mut(7).expect("seller RentCredit") = 64;
        *output.get_mut(10).expect("buyer RentCredit") = 64;
        *output.get_mut(13).expect("Claims aggregate") = 256 + 3 * 8;
        *output.get_mut(14).expect("basis alias") = *output.get(4).expect("basis");
        *output.get_mut(16).expect("Product alias") =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product width");
        *output.get_mut(18).expect("domain") =
            u32::try_from(DOMAIN_HEADER_BYTES - DOMAIN_CUT_BYTES + 3 * DOMAIN_CUT_BYTES)
                .expect("domain width");
        *output.get_mut(20).expect("portfolio alias") = *output.get(3).expect("portfolio");
        *output.get_mut(22).expect("Registry") = 17;
        *output.get_mut(23).expect("Core") = 352;
        *output.get_mut(24).expect("activation") = 128;
        *output.get_mut(25).expect("Registry cache") = 36;
        *output.get_mut(26).expect("Claims program") = 36;
        *output.get_mut(27).expect("Claims ProgramData") = 1_024;
        *output.get_mut(28).expect("source admission") = 36;
        *output.get_mut(29).expect("source staging") = 1_024;
        *output.get_mut(30).expect("destination admission") = 36;
        *output.get_mut(31).expect("destination staging") = 1_024;
        let position = 128 + 3 * 8;
        *output.get_mut(32).expect("source Position") = position;
        *output.get_mut(33).expect("destination Position") = position;
        *output.get_mut(35).expect("Core alias") = *output.get(23).expect("Core");
        *output.get_mut(36).expect("activation alias") = *output.get(24).expect("activation");
        *output.get_mut(37).expect("Registry alias") = *output.get(25).expect("Registry");
        *output.get_mut(38).expect("Claims alias") = *output.get(26).expect("Claims");
        *output.get_mut(39).expect("ProgramData alias") = *output.get(27).expect("ProgramData");
        *output.get_mut(40).expect("Realm") = u32::try_from(REALM_BYTES).expect("Realm width");
        *output.get_mut(42).expect("Custody replay") =
            u32::try_from(CustodyReplayLayoutV1::BYTES).expect("replay width");
        *output.get_mut(43).expect("mint") = 82;
        *output.get_mut(44).expect("buyer token") = 165;
        *output.get_mut(45).expect("seller token") = 165;
        *output.get_mut(47).expect("token program") = 36;
        *output.get_mut(73).expect("fee token") = 165;
        for (account, representative) in [
            (49, 23),
            (50, 24),
            (51, 25),
            (52, 26),
            (53, 27),
            (54, 40),
            (55, 41),
            (56, 42),
            (57, 43),
            (58, 44),
            (59, 45),
            (60, 46),
            (61, 47),
            (63, 23),
            (64, 24),
            (65, 25),
            (66, 26),
            (67, 27),
            (68, 40),
            (69, 41),
            (70, 42),
            (71, 43),
            (72, 44),
            (74, 46),
            (75, 47),
            (77, 23),
            (78, 24),
            (79, 25),
            (80, 26),
            (81, 27),
            (82, 40),
            (83, 41),
            (84, 42),
            (85, 43),
            (86, 44),
            (87, 73),
            (88, 46),
            (89, 47),
        ] {
            let value = *output.get(representative).expect("representative");
            *output.get_mut(account).expect("route alias") = value;
        }
        output
    }

    fn put_rent(state: &mut DirectInlineHotStateV3, rent: &Rent) {
        let rent_account = &mut state
            .fixed_accounts
            .get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3)
            .expect("Rent account")
            .account;
        rent_account.owner = sysvar::ID;
        rent_account.data = vec![0_u8; Rent::size_of()];
        let mut lamports = rent_account.lamports;
        let mut info = AccountInfo::new(
            &rent_account.key,
            false,
            false,
            &mut lamports,
            &mut rent_account.data,
            &rent_account.owner,
            false,
        );
        rent.to_account_info(&mut info).expect("serialize Rent");
    }

    fn put_finalized_record(
        state: &mut DirectInlineHotStateV3,
        rent: &Rent,
        raw_coordinate: usize,
        staging_coordinate: usize,
        schema: [u8; 32],
        data: Vec<u8>,
    ) {
        let registry = state
            .fixed_accounts
            .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
            .expect("Registry")
            .account
            .key;
        let digest = hash(&data).to_bytes();
        let raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &registry,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &registry,
        )
        .0;
        let raw_account = &mut state
            .fixed_accounts
            .get_mut(raw_coordinate)
            .expect("raw record")
            .account;
        raw_account.key = raw;
        raw_account.owner = registry;
        raw_account.lamports = rent.minimum_balance(data.len());
        raw_account.executable = false;
        raw_account.data = data;
        let staging_account = &mut state
            .fixed_accounts
            .get_mut(staging_coordinate)
            .expect("staging cursor")
            .account;
        staging_account.key = staging;
        staging_account.owner = system_program::ID;
        staging_account.lamports = 0;
        staging_account.executable = false;
        staging_account.data.clear();
    }

    fn core_id(bytes: [u8; 32]) -> dclutch_core_contract::ContentId {
        dclutch_core_contract::ContentId::new(bytes).expect("nonzero core content ID")
    }

    fn capability_id(bytes: [u8; 32]) -> dclutch_capability_contract::ContentId {
        dclutch_capability_contract::ContentId::new(bytes).expect("nonzero capability content ID")
    }

    fn chain_artifact_fixture() -> (DirectInlineHotStateV3, [u8; 456]) {
        let (mut state, checked) = hot38_state();
        let rent = Rent::default();
        put_rent(&mut state, &rent);
        let lengths = ordinary_logical_lengths();
        let capacity_profile = [0x44; 32];
        let bundle =
            build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
                account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &lengths,
                },
                capacity_profile,
            })
            .expect("ordinary artifact bundle");
        let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).expect("descriptor");
        let config = DirectExecutionConfigV1::new(1_000_000, 25, [0x45; 32])
            .expect("config")
            .encode();
        let config_digest = hash(&config).to_bytes();
        let descriptor_digest = hash(&bundle.descriptor).to_bytes();
        let set_entry = CapabilityProgramSetEntryV2::new(
            DirectExecutionActionV3::InlineOrdinary as u32,
            CapabilityDescriptorReferenceV2::new(
                core_id(CAPABILITY_PROGRAM_SCHEMA_ID_V4),
                core_id(descriptor_digest),
            ),
        );
        let mut program_set =
            vec![0_u8; encoded_program_set_bytes_v2(1).expect("ProgramSet width")];
        encode_program_set_v2(12, SelectorWidthV2::U32, &[set_entry], &mut program_set)
            .expect("ProgramSet");
        let program_set_digest = hash(&program_set).to_bytes();
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(1).expect("Rent funding"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("Funding amounts");
        let entry = CapabilityEntryV1::new(
            capability_id(dclutch_direct_codec::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3),
            capability_id(program_set_digest),
            capability_id(config_digest),
            capability_id(capacity_profile),
            capability_id(DIRECT_ROOT_SCHEMA_ID_V1),
            capability_id(hash(&bundle.lifecycle_policy).to_bytes()),
            ActivationPolicy::PrepaidLazy,
            1_000,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("Funding quote"),
        )
        .expect("manifest entry");
        let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest");
        let manifest_digest = hash(&manifest).to_bytes();
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            capability_id(manifest_digest),
            capability_id(dclutch_direct_codec::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3),
            capability_id(program_set_digest),
            capability_id(config_digest),
        )
        .expect("selection");
        let market = state
            .fixed_accounts
            .get(HOT_MARKET_ACCOUNT_V3)
            .expect("Market")
            .account
            .key;
        let release_set = [0x46; 32];
        state.release_set = release_set;
        state.generation = 9;
        let header = CapabilityRootHeaderV1::new(
            core_id(release_set),
            market.to_bytes(),
            state.generation,
            selection,
        )
        .expect("root header");
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(&DirectRootStateV1::new().encode());
        let root_key =
            Pubkey::find_program_address(&header.seeds().as_slices(), &checked.trading_program).0;
        let root = &mut state
            .fixed_accounts
            .get_mut(HOT_ROOT_ACCOUNT_V3)
            .expect("root")
            .account;
        root.key = root_key;
        root.owner = checked.trading_program;
        root.lamports = rent.minimum_balance(root_data.len());
        root.data = root_data;
        state
            .runtime_accounts
            .get_mut(0)
            .expect("runtime root")
            .account = root.clone();

        for (raw, staging, schema, data) in [
            (
                HOT_MANIFEST_RAW_ACCOUNT_V3,
                HOT_MANIFEST_STAGING_ACCOUNT_V3,
                CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                manifest,
            ),
            (
                HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
                HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
                CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
                program_set,
            ),
            (
                HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
                HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
                CAPABILITY_PROGRAM_SCHEMA_ID_V4,
                bundle.descriptor.to_vec(),
            ),
            (
                HOT_CONFIG_RAW_ACCOUNT_V3,
                HOT_CONFIG_STAGING_ACCOUNT_V3,
                DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
                config.to_vec(),
            ),
            (
                HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
                HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
                descriptor.account_profile().schema().to_bytes(),
                bundle.account_profile.to_vec(),
            ),
            (
                HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
                HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
                descriptor.request_profile().schema().to_bytes(),
                bundle.request_profile.to_vec(),
            ),
            (
                HOT_TRANSITION_RAW_ACCOUNT_V3,
                HOT_TRANSITION_STAGING_ACCOUNT_V3,
                descriptor.transition().schema().to_bytes(),
                bundle.transition.to_vec(),
            ),
            (
                HOT_EFFECT_RAW_ACCOUNT_V3,
                HOT_EFFECT_STAGING_ACCOUNT_V3,
                descriptor.effect().schema().to_bytes(),
                bundle.effect.to_vec(),
            ),
            (
                HOT_LIFECYCLE_RAW_ACCOUNT_V3,
                HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
                descriptor.lifecycle().schema().to_bytes(),
                bundle.lifecycle_policy.to_vec(),
            ),
            (
                HOT_STRATEGY_RAW_ACCOUNT_V3,
                HOT_STRATEGY_STAGING_ACCOUNT_V3,
                descriptor.strategy().schema().to_bytes(),
                bundle.strategy.to_vec(),
            ),
        ] {
            put_finalized_record(&mut state, &rent, raw, staging, schema, data);
        }
        let request = compile_direct_inline_request_v3(intent(0, 1), intent(1, 2), 1_000, 500_000)
            .expect("Direct request");
        (state, request)
    }

    #[test]
    fn inline_request_has_exact_signed_offsets_and_u32_outcome() {
        let seller = intent(0, 1);
        let buyer = intent(1, 2);
        let request = compile_direct_inline_request_v3(seller, buyer, 1_000, 500_000)
            .expect("inline request");
        assert_eq!(request.len(), 456);
        let seller_message = seller.intent.signed_preimage().expect("seller message");
        let buyer_message = buyer.intent.signed_preimage().expect("buyer message");
        assert_eq!(request.get(64..236), Some(seller_message.as_slice()));
        assert_eq!(request.get(268..440), Some(buyer_message.as_slice()));
        assert!(matches!(
            DirectExecutionRequestV3::decode(&request, 70_001),
            Ok(DirectExecutionRequestV3::InlineOrdinary(_))
        ));
        assert_eq!(
            request.get(440..448),
            Some(1_000_u64.to_le_bytes().as_slice())
        );
        assert_eq!(
            request.get(448..456),
            Some(500_000_u64.to_le_bytes().as_slice())
        );
    }

    #[test]
    fn chain_frame_is_the_only_artifact_authority() {
        let (state, request) = chain_artifact_fixture();
        let bundle = authenticate_chain_artifacts_v4(&state, &request, 70_001)
            .expect("chain-selected Direct artifacts");
        assert_eq!(bundle.action, DirectExecutionActionV3::InlineOrdinary);

        let mut wrong_root_owner = state.clone();
        wrong_root_owner
            .fixed_accounts
            .get_mut(HOT_ROOT_ACCOUNT_V3)
            .expect("root")
            .account
            .owner = key(199);
        assert_eq!(
            authenticate_chain_artifacts_v4(&wrong_root_owner, &request, 70_001),
            Err(Error::ArtifactMismatch)
        );

        let mut live_staging = state.clone();
        live_staging
            .fixed_accounts
            .get_mut(HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)
            .expect("descriptor staging")
            .account
            .data = vec![1];
        assert_eq!(
            authenticate_chain_artifacts_v4(&live_staging, &request, 70_001),
            Err(Error::ArtifactMismatch)
        );

        let mut substituted_descriptor = state;
        *substituted_descriptor
            .fixed_accounts
            .get_mut(HOT_DESCRIPTOR_RAW_ACCOUNT_V3)
            .expect("descriptor")
            .account
            .data
            .get_mut(16)
            .expect("descriptor kind") ^= 1;
        assert_eq!(
            authenticate_chain_artifacts_v4(&substituted_descriptor, &request, 70_001),
            Err(Error::ArtifactMismatch)
        );
    }

    #[test]
    fn adjacent_ed25519_reads_messages_from_hot_instruction_one() {
        let seller = intent(0, 1);
        let buyer = intent(1, 2);
        let instruction = native_ed25519_instruction([seller, buyer]).expect("native evidence");
        assert_eq!(instruction.program_id, ed25519_program::ID);
        assert_eq!(instruction.data.first().copied(), Some(2));
        for (descriptor, expected_message) in [(2_usize, 192_u16), (16, 396)] {
            assert_eq!(
                read_test_u16(&instruction.data, descriptor + 8),
                expected_message
            );
            assert_eq!(read_test_u16(&instruction.data, descriptor + 10), 172);
            assert_eq!(read_test_u16(&instruction.data, descriptor + 12), 1);
        }
    }

    fn read_test_u16(bytes: &[u8], offset: usize) -> u16 {
        let end = offset.checked_add(2).expect("test offset");
        let encoded = bytes.get(offset..end).expect("test u16 bytes");
        u16::from_le_bytes(<[u8; 2]>::try_from(encoded).expect("test u16 width"))
    }

    #[test]
    fn zero_signature_and_maker_alias_refuse_before_artifact_use() {
        let seller = intent(0, 1);
        let mut buyer = intent(1, 2);
        buyer.signature = [0; 64];
        assert_eq!(
            compile_direct_inline_request_v3(seller, buyer, 1, 1),
            Err(Error::ZeroIdentity)
        );
        let mut buyer = intent(1, 2);
        buyer.maker = seller.maker;
        assert_eq!(
            compile_direct_inline_request_v3(seller, buyer, 1, 1),
            Err(Error::ZeroIdentity)
        );
    }

    #[test]
    fn hot38_requires_all_five_injected_runtime_coordinates() {
        let (state, checked) = hot38_state();
        assert_eq!(validate_frame(&state, checked), Ok(observation()));

        let mut substituted = state.clone();
        let root = substituted
            .runtime_accounts
            .first()
            .expect("runtime root")
            .clone();
        *substituted
            .runtime_accounts
            .get_mut(1)
            .expect("runtime config") = root;
        assert_eq!(
            validate_frame(&substituted, checked),
            Err(Error::RuntimeProfileMismatch)
        );

        let mut stale_prefix = state;
        stale_prefix.fixed_accounts.truncate(30);
        assert_eq!(
            validate_frame(&stale_prefix, checked),
            Err(Error::FixedFrameMismatch)
        );
    }

    #[test]
    fn canonical_lut_compiles_packet_and_reports_payer_then_actor() {
        let report = transaction_report(192);
        let payer = key(250);
        let lookup = lookup(&report, payer);
        let plan =
            compile_direct_inline_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &lookup)
                .expect("packet-safe Direct action");
        assert_eq!(plan.required_signers, vec![payer, key(1)]);
        assert_eq!(plan.message.required_signatures, 2);
        assert!(plan.message.loaded_addresses >= 90);
        assert!(plan.message.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
        assert_eq!(plan.outcome_count, 258);
        assert_eq!(
            plan.selected_program_schema,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4
        );
        assert_eq!(plan.selected_program, [8; 32]);
    }

    #[test]
    fn stale_extra_lookup_and_oversized_packet_refuse() {
        let payer = key(250);
        let report = transaction_report(192);
        let mut stale = lookup(&report, payer);
        stale.observation.slot += 1;
        assert_eq!(
            compile_direct_inline_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &stale,),
            Err(DirectInlineTransactionErrorV3::Snapshot)
        );

        let mut extra = lookup(&report, payer);
        let decoded = AddressLookupTable::deserialize(&extra.data).expect("table");
        let mut addresses = decoded.addresses.into_owned();
        addresses.push(key(249));
        addresses.sort_unstable_by_key(Pubkey::to_bytes);
        extra.data = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(addresses),
        }
        .serialize_for_tests()
        .expect("extra table");
        assert_eq!(
            compile_direct_inline_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &extra,),
            Err(DirectInlineTransactionErrorV3::LookupTable)
        );

        let oversized = transaction_report(2_000);
        let oversized_lookup = lookup(&oversized, payer);
        assert_eq!(
            compile_direct_inline_hot_v0(
                &oversized,
                payer,
                Hash::new_from_array([16; 32]),
                &oversized_lookup,
            ),
            Err(DirectInlineTransactionErrorV3::Routing(
                crate::versioned::Error::PacketTooLarge
            ))
        );

        let mut wrong_schema = report;
        wrong_schema.selected_program_schema[0] ^= 1;
        let lookup = lookup(&wrong_schema, payer);
        assert_eq!(
            compile_direct_inline_hot_v0(
                &wrong_schema,
                payer,
                Hash::new_from_array([16; 32]),
                &lookup,
            ),
            Err(DirectInlineTransactionErrorV3::Snapshot)
        );
    }
}
