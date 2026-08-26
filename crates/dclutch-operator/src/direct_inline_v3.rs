//! Chain-derived Direct V3 inline execution construction.
//!
//! This host-only adapter joins the canonical action-selected Direct artifact
//! bundle, expands the authenticated AccountProfile account space, and emits
//! the adjacent native-Ed25519 plus Trading instruction pair. It never performs
//! RPC, signs maker material, signs a transaction, or submits one.

use crate::{
    Finality, Observation, ObservedAccount,
    product_graph_observation_v3::{
        AuthenticatedProductGraphObservationV3, FinalizedProductGraphAccountsV3,
        authenticate_product_graph_observation_v3,
    },
};
use dclutch_account_profile_contract::v2::AccountPrestateV2;
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_direct_codec::{
    artifacts_v3::{
        DirectArtifactBytesV3, DirectArtifactSelectionV3, authenticate_direct_artifacts_v3,
    },
    execution_v3::{
        DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3, DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        DirectExecutionActionV3, DirectExecutionRequestV3, encode_header_v3,
    },
    intent_v2::{COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2, CompactIntentV2},
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
use solana_sdk_ids::{ed25519_program, sysvar};

use crate::versioned::{VersionedMessagePlanV0, compile_v0_message};

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
    /// Action-selected CapabilityProgramV3 content digest.
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
    /// Action-selected CapabilityProgramV3 content digest.
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
#[allow(clippy::too_many_arguments)]
pub fn build_direct_inline_hot_v3(
    state: &DirectInlineHotStateV3,
    artifact_selection: DirectArtifactSelectionV3,
    artifact_bytes: DirectArtifactBytesV3<'_>,
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
    let bundle = authenticate_direct_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &request,
        product.outcome_count,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
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
        selected_program: hash(artifact_bytes.descriptor).to_bytes(),
        outcome_count: product.outcome_count,
        product_record: product.product_record,
        trading_artifact_release: checked.artifact_release,
        checked_manifest_digest: checked.checked_manifest_digest,
        required_instruction_signers,
        preview,
    })
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
    bundle: dclutch_direct_codec::artifacts_v3::DirectArtifactBundleV3<'_>,
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
        let lookup = lookup(&oversized, payer);
        assert_eq!(
            compile_direct_inline_hot_v0(
                &oversized,
                payer,
                Hash::new_from_array([16; 32]),
                &lookup,
            ),
            Err(DirectInlineTransactionErrorV3::Routing(
                crate::versioned::Error::PacketTooLarge
            ))
        );
    }
}
