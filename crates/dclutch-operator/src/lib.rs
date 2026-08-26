//! Host-only construction of unsigned dClutch categorical-resolution instructions.
//!
//! This untrusted projection builder accepts one finalized snapshot of
//! canonical accounts, re-decodes their immutable bindings, and constructs an
//! unsigned instruction. It never performs RPC, signing, or submission.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingCustodyObservationV1,
};
use dclutch_collateral_contract::{COMPACT_TERMINAL_MARKET_BYTES, CompactTerminalMarketV1};
use dclutch_core_contract::Phase;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_product_contract::result_domain::FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1;
use dclutch_pyth_contract::{
    funding::{
        FundingStateV1, required_resolution_minimum_balance, validate_required_resolution_funding,
    },
    instruction::{
        RESOLVE_FAILURE_BYTES, RESOLVE_HEADER_BYTES, ResolveCategoricalFailureV1,
        ResolveCategoricalPythV1,
    },
};
use dclutch_pyth_svm::{PRODUCTION_RELEASES, PostUpdateParamsView, PythReleaseV1};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
};
use dclutch_source_contract::{
    PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    SourceAccessProfile, SourceMaterialViewV1,
};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

pub use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};

/// Canonical schema-bound CapabilityProgramSetV2 artifact construction.
pub mod capability_program_set_v2;
/// Chain-derived compiled Direct transaction construction.
pub mod compiled_direct;
/// Chain-derived unsigned Dealer junior-equity Hot execution construction.
pub mod dealer_equity_hot_v3;
/// Exact delegated-allowance Custody successor CPI construction.
pub mod delegated_custody;
/// Chain-derived action-selected Direct V3 inline execution construction.
pub mod direct_inline_v3;
/// Exact unsigned signing material for the Direct V2 successor.
pub mod direct_successor;
/// Chain-derived unsigned Realm and Market foundation workflows.
pub mod foundation;
/// Chain-derived General V3 Hot execution and packet construction.
pub mod general_hot_v3;
/// Chain-derived unsigned General physical-controller workflows.
pub mod general_physical;
/// Chain-derived inspection of immutable Core/Registry/Rent infrastructure.
pub mod infrastructure;
/// Lifecycle-scoped RentCredit creation, sweeping, and close evidence.
pub mod lifecycle_rent_v2 {
    pub use dclutch_product_runtime_v2_operator::lifecycle_rent_v2::*;
}
mod product_graph_observation_v3 {
    pub(crate) use dclutch_resolution_core_v3_operator::product_graph_observation_v3::{
        AuthenticatedProductGraphObservationV3, FinalizedProductGraphAccountsV3,
        authenticate_product_graph_observation_v3,
    };
}
/// Chain-derived real-provider submission and permissionless reclaim.
pub mod provider_transport_v3;
/// Packet-safe unsigned Rational terminal Bearer redemption construction.
pub mod rational_terminal_v3;
/// Chain-derived registered Direct execution and terminal workflows.
pub mod registered_direct;
/// Chain-derived unsigned Registry activation and reauthentication workflows.
pub mod registry;
/// Checked-release admission into unsigned Registry activation workflows.
pub mod release_activation;
/// Chain-derived Core effects for the complete funded Resolution lifecycle.
pub mod resolution_core_v3 {
    pub use dclutch_resolution_core_v3_operator::*;
}
/// Chain-derived Series V3 Hot lifecycle and packet construction.
pub mod series_hot_v3;
/// Chain-derived address-table lifecycle and versioned-message construction.
pub mod versioned;
/// Chain-derived unsigned Series and Dealer workflows.
pub mod verticals;

pub(crate) const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";
const RECEIVER_CONFIG_SEED: &[u8] = b"config";

/// The exact number of accounts in a price-resolution frame.
pub const PRICE_FRAME_ACCOUNTS: usize =
    dclutch_pyth_contract::frame::PRICE_RESOLUTION_FRAME_V1.len();
/// The exact number of accounts in a permissionless failure-resolution frame.
pub const FAILURE_FRAME_ACCOUNTS: usize =
    dclutch_pyth_contract::frame::FAILURE_RESOLUTION_FRAME_V1.len();

/// Same-finalized account observations required by resolution.
///
/// Material and manifest are mandatory immutable inputs. Callers cannot supply
/// alternate policy, feed, capability, provider, or funding DTO authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionState {
    /// Provider-neutral categorical Market.
    pub market: ObservedAccount,
    /// Raw canonical typed capability-funding state.
    pub fund: ObservedAccount,
    /// Canonical provider-neutral SourceMaterial authority.
    pub resolution_material: ObservedAccount,
    /// Finalization proof for the immutable material record.
    pub resolution_material_finalization: foundation::FinalizedRecordProof,
    /// Immutable manifest selecting the funding entry.
    pub capability_manifest: ObservedAccount,
    /// Finalization proof for the immutable manifest record.
    pub capability_manifest_finalization: foundation::FinalizedRecordProof,
    /// Permanent RentCredit bound to the Market's immutable beneficiary.
    pub rent_credit: ObservedAccount,
    /// Canonical Rent sysvar used to authenticate finalized raw records.
    pub rent_sysvar: ObservedAccount,
    /// Same-snapshot rent-exempt minimum for the canonical funding account.
    pub fund_rent_minimum: u64,
}

/// Caller-selected transaction plumbing for a price path, never semantic authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricePlumbing {
    /// Writable signing resolver.
    pub resolver: Pubkey,
    /// Writable signing temporary update account.
    pub update: Pubkey,
    /// Provider message account.
    pub encoded_vaa: Pubkey,
    /// Exact Pyth receiver post-update body.
    pub post_update_body: Vec<u8>,
}

/// Caller-selected permissionless bounty destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailurePlumbing {
    /// Writable payout recipient.
    pub bounty_recipient: Pubkey,
}

/// Constructed unsigned instruction and the observations that selected it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionReport {
    /// Exact instruction material, never signed or sent.
    pub instruction: Instruction,
    /// Shared finalized observation.
    pub observation: Observation,
    /// Exact pre-submission funding classification.
    pub funding: FundingReport,
}

/// Exact non-Hoard resolution-fund movements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingReport {
    /// Fund account rent refunded at closure.
    pub fund_rent_refund: u64,
    /// Immutable manifest-selected provider reimbursement.
    pub provider_fee_reimbursement: u64,
    /// Immutable manifest-selected bounty.
    pub bounty: u64,
    /// Actual Fund balance above its manifest-bound required minimum.
    ///
    /// This is unclassified native surplus. Resolution credits it only to the
    /// bound permanent RentCredit, never to a caller-selected wallet.
    pub unclassified_credit_excess: u64,
}

/// Refusal from hostile state, immutable bindings, or frame construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// Market bytes were not one implemented categorical Market width.
    InvalidMarket,
    /// Funding bytes were not the raw canonical funding state.
    InvalidFund,
    /// Immutable resolution material was malformed.
    InvalidMaterial,
    /// Immutable capability manifest was malformed.
    InvalidManifest,
    /// A program-owned role had an invalid owner or executable bit.
    InvalidOwner,
    /// A permanent credit was malformed, not program-owned, or executable.
    InvalidRentCredit,
    /// An observation was not finalized.
    ObservationNotFinalized,
    /// Inputs did not share exactly one observation.
    ObservationMismatch,
    /// Market address was not its identity PDA.
    MarketPdaMismatch,
    /// Funding address was not the Market-derived PDA.
    FundPdaMismatch,
    /// RentCredit address or immutable beneficiary binding differed.
    RentCreditPdaMismatch,
    /// An immutable content binding differed.
    ContentIdentityMismatch,
    /// Manifest did not uniquely select the policy configuration.
    FundingSelectionMismatch,
    /// Funding state or balance was insufficient or incompatible.
    FundUnderfunded,
    /// Market was not open.
    MarketNotOpen,
    /// No catalog release selected the committed material release.
    ReleaseUnavailable,
    /// SourceMaterial selected a path this direct Pyth frame cannot execute.
    SourceUnavailable,
    /// Snapshot time cannot admit a provider publication in the committed window.
    PriceWindowClosed,
    /// Failure was attempted before the committed source window elapsed.
    FailureTooEarly,
    /// Release receiver configuration was not its canonical PDA.
    ConfigPdaMismatch,
    /// Post-update body was not exact Pyth receiver material.
    InvalidPostUpdateBody,
    /// Encoding an already validated instruction failed.
    InstructionEncoding,
}

/// Build the canonical 15-account Pyth price-resolution frame.
pub fn build_price_resolution(
    program_id: Pubkey,
    state: &ResolutionState,
    plumbing: &PricePlumbing,
) -> Result<ResolutionReport, Error> {
    let observation = state_observation(state)?;
    let facts = decode_state(program_id, state)?;
    if observation.unix_timestamp < facts.price_window_start
        || observation.unix_timestamp > facts.price_window_end
    {
        return Err(Error::PriceWindowClosed);
    }
    let release = select_release(
        facts.provider_deployment_release_id,
        facts.decoding_rules_id,
        facts.transport_profile_id,
        observation.unix_timestamp,
    )?;
    let post = PostUpdateParamsView::parse(&plumbing.post_update_body)
        .map_err(|_| Error::InvalidPostUpdateBody)?;
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let (expected_config, _) = Pubkey::find_program_address(&[RECEIVER_CONFIG_SEED], &receiver);
    if Pubkey::new_from_array(release.receiver_config()) != expected_config {
        return Err(Error::ConfigPdaMismatch);
    }
    let (treasury, _) =
        Pubkey::find_program_address(&[RECEIVER_TREASURY_SEED, &[post.treasury_id()]], &receiver);
    let data = encode_price(
        facts.generation,
        facts.child_count,
        &plumbing.post_update_body,
    )?;
    let accounts = vec![
        AccountMeta::new(plumbing.resolver, true),
        AccountMeta::new(plumbing.update, true),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new(state.fund.key, false),
        AccountMeta::new_readonly(state.resolution_material.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new(state.rent_credit.key, false),
        AccountMeta::new_readonly(receiver, false),
        AccountMeta::new_readonly(
            Pubkey::new_from_array(release.receiver_programdata()),
            false,
        ),
        AccountMeta::new_readonly(Pubkey::new_from_array(release.receiver_config()), false),
        AccountMeta::new_readonly(plumbing.encoded_vaa, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(release.router_program()), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(release.router_programdata()), false),
        AccountMeta::new(treasury, false),
        AccountMeta::new_readonly(
            state.resolution_material_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(
            state.capability_manifest_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
    ];
    Ok(ResolutionReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation,
        funding: facts.funding,
    })
}

/// Build the canonical six-account permissionless failure-resolution frame.
pub fn build_failure_resolution(
    program_id: Pubkey,
    state: &ResolutionState,
    plumbing: FailurePlumbing,
) -> Result<ResolutionReport, Error> {
    let observation = state_observation(state)?;
    let facts = decode_state(program_id, state)?;
    if observation.unix_timestamp <= facts.failure_window_end {
        return Err(Error::FailureTooEarly);
    }
    let mut data = vec![0; RESOLVE_FAILURE_BYTES];
    ResolveCategoricalFailureV1::new(facts.generation, facts.child_count)
        .encode(&mut data)
        .map_err(|_| Error::InstructionEncoding)?;
    let accounts = vec![
        AccountMeta::new(plumbing.bounty_recipient, false),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new(state.fund.key, false),
        AccountMeta::new_readonly(state.resolution_material.key, false),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new(state.rent_credit.key, false),
        AccountMeta::new_readonly(
            state.resolution_material_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(
            state.capability_manifest_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
    ];
    Ok(ResolutionReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation,
        funding: facts.funding,
    })
}

/// Build the permissionless three-account terminal Market compaction action.
pub fn build_compact_terminal_market(
    program_id: Pubkey,
    market: &ObservedAccount,
    rent_credit: &ObservedAccount,
    rent_sysvar: &ObservedAccount,
) -> Result<Instruction, Error> {
    let _ = same_observation(&[market, rent_credit, rent_sysvar])?;
    if market.owner != program_id || market.executable {
        return Err(Error::InvalidOwner);
    }
    let facts = terminal_market_facts(program_id, market.key, &market.data)?;
    authenticate_rent_credit(
        program_id,
        rent_credit,
        Pubkey::new_from_array(facts.rent_refund),
    )?;
    foundation::decode_rent(rent_sysvar).map_err(|_| Error::InvalidOwner)?;
    let mut data = vec![0; COMPACT_TERMINAL_MARKET_BYTES];
    CompactTerminalMarketV1::new(facts.generation)
        .encode(&mut data)
        .map_err(|_| Error::InstructionEncoding)?;
    Ok(Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(market.key, false),
            AccountMeta::new(rent_credit.key, false),
            AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
        ],
        data,
    })
}

#[derive(Clone, Copy)]
struct Facts {
    generation: u64,
    child_count: u64,
    outcome_count: u8,
    product_instance_id: [u8; 32],
    resolution_material_id: [u8; 32],
    manifest_id: [u8; 32],
    rent_refund: [u8; 32],
    provider_deployment_release_id: [u8; 32],
    decoding_rules_id: [u8; 32],
    transport_profile_id: [u8; 32],
    price_window_start: i64,
    price_window_end: i64,
    failure_window_end: i64,
    funding: FundingReport,
}

#[derive(Clone, Copy)]
struct SourceFacts {
    provider_adapter_release_id: [u8; 32],
    provider_deployment_release_id: [u8; 32],
    decoding_rules_id: [u8; 32],
    transport_profile_id: [u8; 32],
    price_window_start: i64,
    price_window_end: i64,
    failure_window_end: i64,
}

fn encode_price(generation: u64, child_count: u64, body: &[u8]) -> Result<Vec<u8>, Error> {
    let wire = ResolveCategoricalPythV1::new(generation, child_count, body)
        .map_err(|_| Error::InstructionEncoding)?;
    let mut data = vec![0; RESOLVE_HEADER_BYTES + body.len()];
    wire.encode(&mut data)
        .map_err(|_| Error::InstructionEncoding)?;
    Ok(data)
}

fn state_observation(state: &ResolutionState) -> Result<Observation, Error> {
    let observation = same_observation(&[
        &state.market,
        &state.fund,
        &state.resolution_material,
        &state.capability_manifest,
        &state.rent_credit,
        &state.rent_sysvar,
    ])?;
    if state
        .resolution_material_finalization
        .staging_cursor
        .observation
        != observation
        || state
            .capability_manifest_finalization
            .staging_cursor
            .observation
            != observation
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn decode_state(program_id: Pubkey, state: &ResolutionState) -> Result<Facts, Error> {
    state_observation(state)?;
    require_distinct_keys(&[
        state.market.key,
        state.fund.key,
        state.resolution_material.key,
        state.resolution_material_finalization.staging_cursor.key,
        state.capability_manifest.key,
        state.capability_manifest_finalization.staging_cursor.key,
        state.rent_credit.key,
        state.rent_sysvar.key,
    ])?;
    if state.market.owner != program_id
        || state.fund.owner != program_id
        || state.resolution_material.owner != program_id
        || state.capability_manifest.owner != program_id
        || state.market.executable
        || state.fund.executable
        || state.resolution_material.executable
        || state.capability_manifest.executable
    {
        return Err(Error::InvalidOwner);
    }
    let rent = foundation::decode_rent(&state.rent_sysvar).map_err(|_| Error::InvalidOwner)?;
    if state.resolution_material_finalization.schema_release_id
        != SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1
    {
        return Err(Error::InvalidMaterial);
    }
    if state.capability_manifest_finalization.schema_release_id
        != hash(b"dclutch/schema/capability-manifest-profile-1-v1").to_bytes()
    {
        return Err(Error::InvalidManifest);
    }
    foundation::authenticate_finalized_record(
        program_id,
        &rent,
        &state.resolution_material,
        &state.resolution_material_finalization,
    )
    .map_err(|_| Error::InvalidMaterial)?;
    foundation::authenticate_finalized_record(
        program_id,
        &rent,
        &state.capability_manifest,
        &state.capability_manifest_finalization,
    )
    .map_err(|_| Error::InvalidManifest)?;
    let mut facts = market_facts(program_id, state.market.key, &state.market.data)?;
    let source = source_material_facts(&state.resolution_material.data, facts)?;
    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| Error::InvalidManifest)?;
    if manifest.as_bytes() != state.capability_manifest.data.as_slice()
        || hash(manifest.as_bytes()).to_bytes() != facts.manifest_id
    {
        return Err(Error::ContentIdentityMismatch);
    }
    let funding = FundingStateV1::decode(&state.fund.data).map_err(|_| Error::InvalidFund)?;
    if funding.to_bytes().as_slice() != state.fund.data.as_slice() {
        return Err(Error::InvalidFund);
    }
    let manifest_id =
        CapabilityContentId::new(facts.manifest_id).map_err(|_| Error::ContentIdentityMismatch)?;
    let derivation = CapabilityFundingDerivationV1::new(
        state.market.key.to_bytes(),
        facts.generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| Error::FundingSelectionMismatch)?;
    let (expected_fund, _) =
        Pubkey::find_program_address(&derivation.seed_components(), &program_id);
    if state.fund.key != expected_fund {
        return Err(Error::FundPdaMismatch);
    }
    let material_id = CapabilityContentId::new(facts.resolution_material_id)
        .map_err(|_| Error::ContentIdentityMismatch)?;
    let selected = manifest
        .required_founding_entry_for_config(material_id)
        .map_err(|_| Error::FundingSelectionMismatch)?;
    if selected.entry().release_id().to_bytes() != source.provider_adapter_release_id {
        return Err(Error::ContentIdentityMismatch);
    }
    let custody =
        FundingCustodyObservationV1::native_only(state.fund.lamports, state.fund_rent_minimum)
            .map_err(|_| Error::FundUnderfunded)?;
    validate_required_resolution_funding(
        funding,
        manifest_id,
        manifest,
        selected,
        state.fund_rent_minimum,
        custody,
    )
    .map_err(|_| Error::FundUnderfunded)?;
    let minimum =
        required_resolution_minimum_balance(funding).map_err(|_| Error::FundUnderfunded)?;
    let sponsor_refund_excess = state
        .fund
        .lamports
        .checked_sub(minimum)
        .ok_or(Error::FundUnderfunded)?;
    authenticate_rent_credit(
        program_id,
        &state.rent_credit,
        Pubkey::new_from_array(facts.rent_refund),
    )?;
    facts.provider_deployment_release_id = source.provider_deployment_release_id;
    facts.decoding_rules_id = source.decoding_rules_id;
    facts.transport_profile_id = source.transport_profile_id;
    facts.price_window_start = source.price_window_start;
    facts.price_window_end = source.price_window_end;
    facts.failure_window_end = source.failure_window_end;
    facts.funding = FundingReport {
        fund_rent_refund: state.fund_rent_minimum,
        provider_fee_reimbursement: funding.remaining().provider().amount(),
        bounty: funding.remaining().bounty().amount(),
        unclassified_credit_excess: sponsor_refund_excess,
    };
    Ok(facts)
}

fn source_material_facts(bytes: &[u8], market: Facts) -> Result<SourceFacts, Error> {
    let material = SourceMaterialViewV1::decode(bytes).map_err(|_| Error::InvalidMaterial)?;
    let policy = material.policy().map_err(|_| Error::InvalidMaterial)?;
    let (capacity_id, capacity) = material
        .capacity_profile()
        .map_err(|_| Error::InvalidMaterial)?;
    let (source_id, source) = material
        .primary_source()
        .map_err(|_| Error::InvalidMaterial)?;
    let (window_id, window) = material.window_spec().map_err(|_| Error::InvalidMaterial)?;
    let statistic = material.statistic().map_err(|_| Error::InvalidMaterial)?;
    let (provider_release_id, provider) = material
        .primary_provider_release()
        .map_err(|_| Error::InvalidMaterial)?;
    let adapter = material
        .primary_adapter_config()
        .map_err(|_| Error::InvalidMaterial)?;
    let product_instance_id = material
        .product_instance_id()
        .map_err(|_| Error::InvalidMaterial)?;
    let result_domain = material
        .result_domain()
        .map_err(|_| Error::InvalidMaterial)?;
    let result_domain_bytes = result_domain.to_bytes();
    let result_domain_id = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        result_domain_bytes.as_slice(),
    ])
    .to_bytes();
    if material.as_bytes() != bytes
        || hash(material.as_bytes()).to_bytes() != market.resolution_material_id
        || product_instance_id.to_bytes() != market.product_instance_id
        || policy.product_instance_id().to_bytes() != market.product_instance_id
        || policy.result_domain_id().to_bytes() != result_domain_id
        || result_domain.outcome_count() != market.outcome_count
        || hash(&capacity.to_bytes()).to_bytes() != capacity_id.to_bytes()
        || hash(&source.to_bytes()).to_bytes() != source_id.to_bytes()
        || hash(&window.to_bytes()).to_bytes() != window_id.to_bytes()
        || hash(&statistic.to_bytes()).to_bytes() != policy.statistic_spec_id().to_bytes()
        || hash(&provider.to_bytes()).to_bytes() != provider_release_id.to_bytes()
        || hash(&adapter.to_bytes()).to_bytes() != source.adapter_config_id().to_bytes()
    {
        return Err(Error::ContentIdentityMismatch);
    }
    if source.access_profile() != SourceAccessProfile::PythTerminalOneTransaction
        || provider.adapter_release_id().to_bytes() != PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
        || material
            .recovery_policy()
            .map_err(|_| Error::InvalidMaterial)?
            .is_some()
    {
        return Err(Error::SourceUnavailable);
    }
    let price_window_start = window
        .start_unix_seconds()
        .checked_sub(i64::from(window.max_future_skew_seconds()))
        .ok_or(Error::InvalidMaterial)?;
    let price_window_end = window
        .end_unix_seconds()
        .checked_add(i64::from(window.max_age_seconds()))
        .ok_or(Error::InvalidMaterial)?;
    Ok(SourceFacts {
        provider_adapter_release_id: provider.adapter_release_id().to_bytes(),
        provider_deployment_release_id: provider.provider_deployment_release_id().to_bytes(),
        decoding_rules_id: provider.decoding_rules_id().to_bytes(),
        transport_profile_id: provider.transport_profile_id().to_bytes(),
        price_window_start,
        price_window_end,
        failure_window_end: window.end_unix_seconds(),
    })
}

fn require_distinct_keys(keys: &[Pubkey]) -> Result<(), Error> {
    for (index, key) in keys.iter().enumerate() {
        if keys.iter().skip(index + 1).any(|other| other == key) {
            return Err(Error::InvalidOwner);
        }
    }
    Ok(())
}

fn same_observation(accounts: &[&ObservedAccount]) -> Result<Observation, Error> {
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(Error::ObservationNotFinalized)?;
    if observation.finality != Finality::Finalized {
        return Err(Error::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn market_facts(program_id: Pubkey, market_key: Pubkey, bytes: &[u8]) -> Result<Facts, Error> {
    match decode_market_outcome_count(bytes).map_err(|_| Error::InvalidMarket)? {
        2 => typed_market_facts::<2>(program_id, market_key, bytes),
        3 => typed_market_facts::<3>(program_id, market_key, bytes),
        4 => typed_market_facts::<4>(program_id, market_key, bytes),
        5 => typed_market_facts::<5>(program_id, market_key, bytes),
        6 => typed_market_facts::<6>(program_id, market_key, bytes),
        7 => typed_market_facts::<7>(program_id, market_key, bytes),
        8 => typed_market_facts::<8>(program_id, market_key, bytes),
        9 => typed_market_facts::<9>(program_id, market_key, bytes),
        10 => typed_market_facts::<10>(program_id, market_key, bytes),
        11 => typed_market_facts::<11>(program_id, market_key, bytes),
        12 => typed_market_facts::<12>(program_id, market_key, bytes),
        13 => typed_market_facts::<13>(program_id, market_key, bytes),
        14 => typed_market_facts::<14>(program_id, market_key, bytes),
        15 => typed_market_facts::<15>(program_id, market_key, bytes),
        16 => typed_market_facts::<16>(program_id, market_key, bytes),
        _ => Err(Error::InvalidMarket),
    }
}

fn typed_market_facts<const N: usize>(
    program_id: Pubkey,
    market_key: Pubkey,
    bytes: &[u8],
) -> Result<Facts, Error> {
    let market = CategoricalMarketV1::<N>::decode(bytes).map_err(|_| Error::InvalidMarket)?;
    let root = market.root();
    if root.phase() != Phase::Open {
        return Err(Error::MarketNotOpen);
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &program_id);
    if market_key != expected_market {
        return Err(Error::MarketPdaMismatch);
    }
    Ok(Facts {
        generation: root.identity().generation(),
        child_count: root.outstanding_children(),
        outcome_count: u8::try_from(N).map_err(|_| Error::InvalidMarket)?,
        product_instance_id: root.identity().product_instance_id().to_bytes(),
        resolution_material_id: root.identity().resolution_policy_id().to_bytes(),
        manifest_id: root.identity().capability_manifest_id().to_bytes(),
        rent_refund: root.rent_refund(),
        provider_deployment_release_id: [0; 32],
        decoding_rules_id: [0; 32],
        transport_profile_id: [0; 32],
        price_window_start: 0,
        price_window_end: 0,
        failure_window_end: 0,
        funding: FundingReport {
            fund_rent_refund: 0,
            provider_fee_reimbursement: 0,
            bounty: 0,
            unclassified_credit_excess: 0,
        },
    })
}

fn authenticate_rent_credit(
    program_id: Pubkey,
    account: &ObservedAccount,
    beneficiary: Pubkey,
) -> Result<RentCreditV1, Error> {
    let authority =
        RefundAuthority::new(beneficiary.to_bytes()).map_err(|_| Error::RentCreditPdaMismatch)?;
    let authority_bytes = authority.to_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        &program_id,
    );
    if account.key != expected {
        return Err(Error::RentCreditPdaMismatch);
    }
    if account.owner != program_id
        || account.executable
        || account.data.len() != RENT_CREDIT_BYTES_V1
    {
        return Err(Error::InvalidRentCredit);
    }
    let credit = RentCreditV1::decode(&account.data).map_err(|_| Error::InvalidRentCredit)?;
    credit
        .validate_binding(authority, bump)
        .map_err(|_| Error::RentCreditPdaMismatch)?;
    if credit.to_bytes().as_slice() != account.data.as_slice() {
        return Err(Error::InvalidRentCredit);
    }
    Ok(credit)
}

fn terminal_market_facts(
    program_id: Pubkey,
    market_key: Pubkey,
    bytes: &[u8],
) -> Result<Facts, Error> {
    match decode_market_outcome_count(bytes).map_err(|_| Error::InvalidMarket)? {
        2 => typed_terminal_market_facts::<2>(program_id, market_key, bytes),
        3 => typed_terminal_market_facts::<3>(program_id, market_key, bytes),
        4 => typed_terminal_market_facts::<4>(program_id, market_key, bytes),
        5 => typed_terminal_market_facts::<5>(program_id, market_key, bytes),
        6 => typed_terminal_market_facts::<6>(program_id, market_key, bytes),
        7 => typed_terminal_market_facts::<7>(program_id, market_key, bytes),
        8 => typed_terminal_market_facts::<8>(program_id, market_key, bytes),
        9 => typed_terminal_market_facts::<9>(program_id, market_key, bytes),
        10 => typed_terminal_market_facts::<10>(program_id, market_key, bytes),
        11 => typed_terminal_market_facts::<11>(program_id, market_key, bytes),
        12 => typed_terminal_market_facts::<12>(program_id, market_key, bytes),
        13 => typed_terminal_market_facts::<13>(program_id, market_key, bytes),
        14 => typed_terminal_market_facts::<14>(program_id, market_key, bytes),
        15 => typed_terminal_market_facts::<15>(program_id, market_key, bytes),
        16 => typed_terminal_market_facts::<16>(program_id, market_key, bytes),
        _ => Err(Error::InvalidMarket),
    }
}

fn typed_terminal_market_facts<const N: usize>(
    program_id: Pubkey,
    market_key: Pubkey,
    bytes: &[u8],
) -> Result<Facts, Error> {
    let market = CategoricalMarketV1::<N>::decode(bytes).map_err(|_| Error::InvalidMarket)?;
    let root = market.root();
    if root.phase() != Phase::Retired
        || root.outstanding_children() != 0
        || market.hoard_atoms() != 0
        || market.supply().iter().any(|amount| *amount != 0)
    {
        return Err(Error::MarketNotOpen);
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &program_id);
    if market_key != expected_market {
        return Err(Error::MarketPdaMismatch);
    }
    Ok(Facts {
        generation: root.identity().generation(),
        child_count: 0,
        outcome_count: u8::try_from(N).map_err(|_| Error::InvalidMarket)?,
        product_instance_id: [0; 32],
        resolution_material_id: [0; 32],
        manifest_id: [0; 32],
        rent_refund: root.rent_refund(),
        provider_deployment_release_id: [0; 32],
        decoding_rules_id: [0; 32],
        transport_profile_id: [0; 32],
        price_window_start: 0,
        price_window_end: 0,
        failure_window_end: 0,
        funding: FundingReport {
            fund_rent_refund: 0,
            provider_fee_reimbursement: 0,
            bounty: 0,
            unclassified_credit_excess: 0,
        },
    })
}

fn select_release(
    release_id: [u8; 32],
    decoding_rules_id: [u8; 32],
    transport_profile_id: [u8; 32],
    observed_time: i64,
) -> Result<PythReleaseV1, Error> {
    for release in &PRODUCTION_RELEASES {
        if hash(&release.to_bytes()).to_bytes() == release_id
            && observed_time >= release.activation_time()
            && release.price_update_codec_id() == decoding_rules_id
            && release.adapter_id() == transport_profile_id
        {
            return Ok(*release);
        }
    }
    #[cfg(feature = "non-production-real-pyth-lab")]
    {
        let release = dclutch_pyth_svm::synthetic_local_release_v1()
            .map_err(|_| Error::ReleaseUnavailable)?;
        let release = *release.release();
        if hash(&release.to_bytes()).to_bytes() == release_id
            && observed_time >= release.activation_time()
            && release.price_update_codec_id() == decoding_rules_id
            && release.adapter_id() == transport_profile_id
        {
            return Ok(release);
        }
    }
    Err(Error::ReleaseUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot};
    use dclutch_market_contract::market::CategoricalSettlementSummaryV1;
    use dclutch_product_contract::{
        ContentId as ProductContentId,
        capacity::CapacityProfileId,
        product::{InstanceV1, InstanceV1Input},
        result_domain::FiniteResultDomainV1,
    };
    use dclutch_pyth_contract::{
        funding::{FUNDING_BYTES, construct_required_resolution_funding},
        instruction::ResolveCategoricalFailureV1,
    };
    use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
    use dclutch_source_contract::{
        CapacityEnvelope as SourceCapacityEnvelope, ProviderReleaseV1, PythAdapterConfigV1,
        ResolutionPolicyV1, RoundingBoundary, SOURCE_MATERIAL_BYTES, SourceCapacityProfileV1,
        SourceMaterialInputV1, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind,
        WindowSpecV1, encode_source_material_into_v1,
    };
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};

    fn observation() -> Observation {
        Observation {
            slot: 7,
            unix_timestamp: 12,
            finality: Finality::Finalized,
        }
    }

    fn account(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    fn native_resolution_quote(rent: u64, provider: u64, bounty: u64) -> FundingQuoteV1 {
        let native = |amount| {
            CompartmentFundingV1::native_lamports(amount).expect("native resolution amount")
        };
        let not_applicable = CompartmentFundingV1::not_applicable();
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                native(rent),
                not_applicable,
                not_applicable,
                if provider == 0 {
                    not_applicable
                } else {
                    native(provider)
                },
                if bounty == 0 {
                    not_applicable
                } else {
                    native(bounty)
                },
                not_applicable,
                not_applicable,
            )
            .expect("typed native resolution amounts"),
            None,
        )
        .expect("typed native resolution quote")
    }

    fn finalized_record(
        program: Pubkey,
        schema: [u8; 32],
        data: Vec<u8>,
    ) -> (ObservedAccount, foundation::FinalizedRecordProof) {
        let digest = hash(&data).to_bytes();
        let (raw, _) = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &program,
        );
        let (cursor, _) = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &program,
        );
        (
            account(raw, program, u64::MAX, data),
            foundation::FinalizedRecordProof {
                schema_release_id: schema,
                staging_cursor: account(cursor, system_program::ID, 0, Vec::new()),
            },
        )
    }

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero ID")
    }

    fn product_id(bytes: [u8; 32]) -> ProductContentId {
        ProductContentId::new(bytes).expect("nonzero Product ID")
    }

    fn source_id(bytes: [u8; 32]) -> dclutch_source_contract::ContentId {
        dclutch_source_contract::ContentId::new(bytes).expect("nonzero Source ID")
    }

    fn product_material() -> (InstanceV1, [u8; 32], FiniteResultDomainV1) {
        let domain = FiniteResultDomainV1::new(product_id([1; 32]), product_id([2; 32]), 1, &[])
            .expect("binary result domain");
        let domain_bytes = domain.to_bytes();
        let domain_id = product_id(
            hashv(&[
                FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
                &[0],
                domain_bytes.as_slice(),
            ])
            .to_bytes(),
        );
        let instance = InstanceV1::new(InstanceV1Input {
            terms_id: product_id([3; 32]),
            occurrence_id: product_id([4; 32]),
            claim_basis_id: product_id([5; 32]),
            result_domain_id: domain_id,
            capacity_profile_id: CapacityProfileId::new(product_id([6; 32])),
            partition_cell_count: 2,
        })
        .expect("Product instance");
        (instance, hash(&instance.to_bytes()).to_bytes(), domain)
    }

    fn source_material(
        instance: InstanceV1,
        instance_id: [u8; 32],
        domain: FiniteResultDomainV1,
    ) -> Vec<u8> {
        source_material_with_access(
            instance,
            instance_id,
            domain,
            SourceAccessProfile::PythTerminalOneTransaction,
        )
    }

    fn source_material_with_access(
        instance: InstanceV1,
        instance_id: [u8; 32],
        domain: FiniteResultDomainV1,
        access: SourceAccessProfile,
    ) -> Vec<u8> {
        let capacity = SourceCapacityProfileV1::new(
            SourceCapacityEnvelope::Measured,
            1,
            0,
            source_id([37; 32]),
            source_id([38; 32]),
            512,
            1,
        )
        .expect("source capacity");
        let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
        let provider = ProviderReleaseV1::new(
            source_id([31; 32]),
            source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
            source_id([9; 32]),
            source_id([33; 32]),
            source_id([34; 32]),
        );
        let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
        let adapter = PythAdapterConfigV1::new([35; 32], -8, 10_000).expect("Pyth config");
        let adapter_id = source_id(hash(&adapter.to_bytes()).to_bytes());
        let source = SourceSpecV1::new(
            source_id(domain.coordinate_domain_id().to_bytes()),
            source_id(domain.result_unit_id().to_bytes()),
            provider_id,
            access,
            adapter_id,
            capacity_id,
        );
        let primary_source_id = source_id(hash(&source.to_bytes()).to_bytes());
        let window = WindowSpecV1::new(
            primary_source_id,
            WindowKind::Terminal,
            10,
            10,
            10,
            2,
            source_id([36; 32]),
        )
        .expect("terminal window");
        let window_id = source_id(hash(&window.to_bytes()).to_bytes());
        let statistic = StatisticSpecV1::new(
            source_id(domain.result_unit_id().to_bytes()),
            source_id(domain.result_unit_id().to_bytes()),
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            capacity_id,
            source_id([39; 32]),
            capacity,
        )
        .expect("terminal statistic");
        let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes());
        let domain_bytes = domain.to_bytes();
        let domain_id = source_id(
            hashv(&[
                FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
                &[0],
                domain_bytes.as_slice(),
            ])
            .to_bytes(),
        );
        let policy = ResolutionPolicyV1::new(
            capacity_id,
            source_id(instance_id),
            primary_source_id,
            window_id,
            statistic_id,
            domain_id,
            None,
        );
        let mut material = vec![0; SOURCE_MATERIAL_BYTES];
        encode_source_material_into_v1(
            &mut material,
            SourceMaterialInputV1 {
                policy: &policy,
                capacity_profile_id: capacity_id,
                capacity_profile: &capacity,
                primary_source_id,
                primary_source: &source,
                primary_provider_release_id: provider_id,
                primary_provider_release: &provider,
                primary_adapter_config: &adapter,
                window_id,
                window: &window,
                statistic_id,
                statistic: &statistic,
                product_instance_id: source_id(instance_id),
                product_instance: &instance,
                result_domain: &domain,
                recovery: None,
            },
        )
        .expect("canonical Source material");
        material
    }

    fn rent_account() -> ObservedAccount {
        let rent = Rent::default();
        let mut data = vec![0; Rent::size_of()];
        let mut lamports = 1;
        let mut info = AccountInfo::new(
            &solana_sdk_ids::sysvar::rent::ID,
            false,
            false,
            &mut lamports,
            &mut data,
            &solana_sdk_ids::sysvar::ID,
            false,
        );
        rent.to_account_info(&mut info).expect("rent");
        drop(info);
        account(
            solana_sdk_ids::sysvar::rent::ID,
            solana_sdk_ids::sysvar::ID,
            1,
            data,
        )
    }

    fn system_program_account() -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key: system_program::ID,
            owner: solana_sdk_ids::native_loader::ID,
            lamports: 1,
            executable: true,
            data: Vec::new(),
        }
    }

    fn fixture() -> (Pubkey, ResolutionState) {
        let program = Pubkey::new_from_array([40; 32]);
        let sponsor = Pubkey::new_from_array([41; 32]);
        let (instance, instance_id, domain) = product_material();
        let material = source_material(instance, instance_id, domain);
        let material_id = hash(&material).to_bytes();
        let quote = native_resolution_quote(100, 7, 11);
        let entry = CapabilityEntryV1::new(
            CapabilityContentId::new([11; 32]).expect("kind"),
            CapabilityContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1).expect("release"),
            CapabilityContentId::new(material_id).expect("SourceMaterial ID"),
            CapabilityContentId::new([12; 32]).expect("capacity"),
            CapabilityContentId::new([13; 32]).expect("schema"),
            CapabilityContentId::new([14; 32]).expect("derivation"),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("entry");
        let mut manifest_data = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest =
            CapabilityManifestV1::encode_into(&[entry], &mut manifest_data).expect("manifest");
        let manifest_id = hash(manifest.as_bytes()).to_bytes();
        let identity = MarketIdentity::new(
            id(21),
            ContentId::new(instance_id).expect("Product instance ID"),
            id(23),
            ContentId::new(material_id).expect("SourceMaterial"),
            ContentId::new(manifest_id).expect("manifest"),
            0,
        );
        let mut root = MarketRoot::founding(identity, sponsor.to_bytes()).expect("root");
        root.transition_phase(0, Phase::Open).expect("open");
        root.register_child(0, 0).expect("child");
        let market =
            CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
                .expect("market");
        let mut market_data = vec![0; CategoricalMarketV1::<2>::encoded_len().expect("length")];
        market.encode(&mut market_data).expect("encode");
        let (market_key, _) = Pubkey::find_program_address(
            &[MARKET_SEED, &hash(&identity.to_bytes()).to_bytes()],
            &program,
        );
        let selected = manifest
            .required_founding_entry_for_config(
                CapabilityContentId::new(material_id).expect("SourceMaterial"),
            )
            .expect("selected");
        let funding = construct_required_resolution_funding(
            CapabilityContentId::new(manifest_id).expect("manifest"),
            manifest,
            selected,
            100,
            7,
        )
        .expect("funding");
        assert_eq!(
            FUNDING_BYTES,
            dclutch_capability_contract::FUNDING_STATE_BYTES
        );
        let fund_data = funding.to_bytes().to_vec();
        let derivation = CapabilityFundingDerivationV1::new(
            market_key.to_bytes(),
            identity.generation(),
            CapabilityContentId::new(manifest_id).expect("manifest"),
            manifest,
            funding,
        )
        .expect("fund derivation");
        let (fund_key, _) = Pubkey::find_program_address(&derivation.seed_components(), &program);
        let authority = RefundAuthority::new(sponsor.to_bytes()).expect("authority");
        let (rent_credit_key, rent_credit_bump) =
            Pubkey::find_program_address(&[RENT_CREDIT_PDA_DOMAIN_V1, sponsor.as_ref()], &program);
        let (resolution_material, resolution_material_finalization) =
            finalized_record(program, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, material);
        let (capability_manifest, capability_manifest_finalization) = finalized_record(
            program,
            hash(b"dclutch/schema/capability-manifest-profile-1-v1").to_bytes(),
            manifest_data,
        );
        (
            program,
            ResolutionState {
                market: account(market_key, program, 1, market_data),
                fund: account(fund_key, program, 118, fund_data),
                resolution_material,
                resolution_material_finalization,
                capability_manifest,
                capability_manifest_finalization,
                rent_credit: account(
                    rent_credit_key,
                    program,
                    1,
                    RentCreditV1::new(authority, rent_credit_bump)
                        .to_bytes()
                        .to_vec(),
                ),
                rent_sysvar: rent_account(),
                fund_rent_minimum: 100,
            },
        )
    }

    #[test]
    fn failure_frame_is_current_six_role_frame_and_raw_funding_is_classified() {
        let (program, state) = fixture();
        assert_eq!(
            PRICE_FRAME_ACCOUNTS,
            dclutch_pyth_contract::frame::PRICE_RESOLUTION_FRAME_V1.len()
        );
        assert_eq!(
            FAILURE_FRAME_ACCOUNTS,
            dclutch_pyth_contract::frame::FAILURE_RESOLUTION_FRAME_V1.len()
        );
        let report = build_failure_resolution(
            program,
            &state,
            FailurePlumbing {
                bounty_recipient: Pubkey::new_from_array([44; 32]),
            },
        )
        .expect("failure frame");
        assert_eq!(report.instruction.accounts.len(), FAILURE_FRAME_ACCOUNTS);
        assert_eq!(
            report.funding,
            FundingReport {
                fund_rent_refund: 100,
                provider_fee_reimbursement: 7,
                bounty: 11,
                unclassified_credit_excess: 0
            }
        );
        assert_eq!(
            ResolveCategoricalFailureV1::decode(&report.instruction.data)
                .expect("wire")
                .child_count(),
            1
        );
        assert_eq!(
            report.instruction.accounts.get(3).map(|meta| meta.pubkey),
            Some(state.resolution_material.key)
        );
        assert_eq!(
            report.instruction.accounts.get(4).map(|meta| meta.pubkey),
            Some(state.capability_manifest.key)
        );
    }

    #[test]
    fn source_create_is_chain_derived_and_refuses_material_or_pda_substitution() {
        let (program, resolution) = fixture();
        let facts = market_facts(program, resolution.market.key, &resolution.market.data)
            .expect("Market facts");
        let generation = facts.generation.to_le_bytes();
        let (destination, _) = Pubkey::find_program_address(
            &[
                dclutch_source_contract::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
                resolution.market.key.as_ref(),
                generation.as_slice(),
            ],
            &program,
        );
        let mut rent_credit = resolution.rent_credit.clone();
        rent_credit.lamports = u64::MAX;
        let state = verticals::SourceCreateResolutionState {
            payer: account(
                Pubkey::new_from_array([88; 32]),
                system_program::ID,
                u64::MAX,
                Vec::new(),
            ),
            resolution_state_destination: account(destination, system_program::ID, 7, Vec::new()),
            market: resolution.market.clone(),
            resolution_material: resolution.resolution_material.clone(),
            resolution_material_finalization: resolution.resolution_material_finalization.clone(),
            rent_credit,
            system_program: system_program_account(),
            rent_sysvar: resolution.rent_sysvar.clone(),
        };
        let report = verticals::build_source_create_resolution_v1(program, &state)
            .expect("chain-derived Source Create");
        assert_eq!(report.instruction.accounts.len(), 8);
        assert_eq!(report.resolution_state, destination);
        assert_eq!(report.expected_market_child_count, facts.child_count);
        let decoded =
            dclutch_source_contract::SourceInstructionV1::decode(&report.instruction.data)
                .expect("Source Create wire");
        if let dclutch_source_contract::SourceInstructionV1::CreateResolution(wire) = decoded {
            assert_eq!(wire.market(), state.market.key.to_bytes());
            assert_eq!(wire.generation(), facts.generation);
            assert_eq!(wire.expected_market_child_count(), facts.child_count);
            assert_eq!(wire.material_id().to_bytes(), facts.resolution_material_id);
        }
        assert!(matches!(
            decoded,
            dclutch_source_contract::SourceInstructionV1::CreateResolution(_)
        ));

        let mut wrong_material = state.clone();
        *wrong_material
            .resolution_material
            .data
            .get_mut(16)
            .expect("SourceMaterial body") ^= 1;
        assert!(matches!(
            verticals::build_source_create_resolution_v1(program, &wrong_material),
            Err(verticals::VerticalError::FinalizationMismatch)
                | Err(verticals::VerticalError::ContentMismatch)
        ));
        let mut wrong_destination = state;
        wrong_destination.resolution_state_destination.key = Pubkey::new_from_array([89; 32]);
        assert_eq!(
            verticals::build_source_create_resolution_v1(program, &wrong_destination),
            Err(verticals::VerticalError::PdaMismatch)
        );
    }

    #[test]
    fn source_material_is_the_root_and_product_domain_identity_is_domain_separated() {
        let (program, state) = fixture();
        let mut facts =
            market_facts(program, state.market.key, &state.market.data).expect("Market facts");
        let decoded = source_material_facts(&state.resolution_material.data, facts)
            .expect("canonical SourceMaterial");
        assert_eq!(
            decoded.provider_adapter_release_id,
            PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1
        );

        let mut substituted_domain_identity = state.resolution_material.data.clone();
        substituted_domain_identity
            .get_mut(192..224)
            .expect("policy result-domain ID")
            .fill(88);
        assert!(SourceMaterialViewV1::decode(&substituted_domain_identity).is_ok());
        facts.resolution_material_id = hash(&substituted_domain_identity).to_bytes();
        assert_eq!(
            source_material_facts(&substituted_domain_identity, facts).err(),
            Some(Error::ContentIdentityMismatch)
        );

        let mut substituted_product = facts;
        substituted_product.product_instance_id = [89; 32];
        substituted_product.resolution_material_id =
            hash(&state.resolution_material.data).to_bytes();
        assert_eq!(
            source_material_facts(&state.resolution_material.data, substituted_product).err(),
            Some(Error::ContentIdentityMismatch)
        );
    }

    #[test]
    fn unsupported_source_paths_and_wrong_record_schema_are_explicit() {
        let (program, state) = fixture();
        let mut wrong_schema = state.clone();
        wrong_schema
            .resolution_material_finalization
            .schema_release_id = [90; 32];
        assert_eq!(
            build_failure_resolution(
                program,
                &wrong_schema,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::InvalidMaterial)
        );

        let (instance, instance_id, domain) = product_material();
        let shared = source_material_with_access(
            instance,
            instance_id,
            domain,
            SourceAccessProfile::SharedObservationChild,
        );
        let mut facts =
            market_facts(program, state.market.key, &state.market.data).expect("Market facts");
        facts.resolution_material_id = hash(&shared).to_bytes();
        assert_eq!(
            source_material_facts(&shared, facts).err(),
            Some(Error::SourceUnavailable)
        );
        assert_eq!(
            build_price_resolution(
                program,
                &state,
                &PricePlumbing {
                    resolver: Pubkey::new_unique(),
                    update: Pubkey::new_unique(),
                    encoded_vaa: Pubkey::new_unique(),
                    post_update_body: Vec::new(),
                }
            ),
            Err(Error::ReleaseUnavailable)
        );
    }

    #[test]
    fn hostile_snapshot_and_vacancy_of_funding_principal_refuse() {
        let (program, state) = fixture();
        let mut mismatched = state.clone();
        mismatched.capability_manifest.observation.slot += 1;
        assert_eq!(
            build_failure_resolution(
                program,
                &mismatched,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::ObservationMismatch)
        );
        let mut underfunded = state;
        underfunded.fund.lamports = 117;
        assert_eq!(
            build_failure_resolution(
                program,
                &underfunded,
                FailurePlumbing {
                    bounty_recipient: Pubkey::new_unique()
                }
            ),
            Err(Error::FundUnderfunded)
        );
    }
}
