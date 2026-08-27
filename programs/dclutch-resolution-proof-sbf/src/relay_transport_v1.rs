//! Executable `RelayedMainnetStateV1` observation-record transport.
//!
//! Four permissionless routes create, fill, seal and close one cross-cluster
//! observation record. Everything they enforce lives in
//! `dclutch-relay-contract`; this module is the account boundary: it
//! authenticates the Core Market, the Registry-owned immutable records, the
//! record PDA, the preceding native Ed25519 instruction, and the two clocks,
//! then hands exact bytes to the contract.
//!
//! # Why this lives in the Resolution role
//!
//! The relayed observation record is provider evidence: it is to
//! `RelayedMainnetStateV1` what a Receiver-owned `PriceUpdateV2` is to Pyth,
//! and the difference is only that no external program on this cluster will
//! hold it. So dClutch has to, and Resolution is the role that already does.
//! [`crate::provider_transport_v3`] beside this module owns exactly the same
//! class of object for the Pyth family: a Resolution-owned, permissionlessly
//! created, permissionlessly reclaimed lifecycle account that holds one
//! Market's provider evidence until a resolution consumes it. Putting the
//! relayed record anywhere else would give provider-transport custody two
//! owners.
//!
//! A separate Program was the alternative and Decision 0003 refuses it: the
//! release set describes exactly five replaceable roles, and a genuinely
//! state-owning sixth needs a new measured release-set profile and its own
//! authority decision. This record needs neither — it is read by Resolution,
//! held by Resolution, and reclaimed by Resolution.
//!
//! # What creation does and does not prove
//!
//! Creation is permissionless and self-funded. It authenticates the Market as
//! Core-owned state at its own derived address, and requires the Market's
//! selected release set to name **this** Program as its Resolution role, so a
//! record cannot appear under a Market that runs a different Resolution
//! release. It deliberately does not hash this Program's ELF: that is the
//! whole-artifact authentication the Registry activation already performed once
//! at activation time, and repeating it per record would put a megabyte of
//! SHA-256 on a route anyone may call.
//!
//! The record is not a Market child and mutates no Core state. A caller who
//! builds a record against a substituted Market spends their own rent on an
//! account at an address no resolution will ever read, because the record's PDA
//! is derived from the Market it names.
//!
//! **The observed cluster is pinned by this adapter release**, not by a record
//! field a founder could set. `RelayedMainnetStateV1` v1 observes Solana
//! mainnet-beta and nothing else; observing a different cluster is a different
//! `adapter_release_id`, which is the existing immutability discipline rather
//! than a new one. `account_set_id` binds the same genesis hash a second time,
//! so a substituted cluster fails twice and fails *specifically* — which
//! matters, because a venue `Program` account can be byte-identical on two
//! clusters and nothing else can tell them apart.
//!
//! Digests are computed here with the runtime's SHA-256 and compared by the
//! contract; the contract itself hashes nothing, so the daemon's software
//! implementation and this syscall agree on one canonical preimage.

use alloc::{boxed::Box, vec::Vec};

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingCustodyObservationV1,
    FundingStateV1,
};
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, Readiness as CoreReadiness,
};
use dclutch_product_runtime_v2::{ContentId as ProductContentId, ResultDomainV2};
use dclutch_product_runtime_v2_svm_reader::{
    AuthenticatedProductRuntimeV2, FinalizedRecordFrameV2, ProductRuntimeFrameV2,
    authenticate_product_runtime_v2,
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1,
};
use dclutch_relay_contract::{
    MAX_RELAYED_ACCOUNTS_V1, RELAYED_ADAPTER_CONFIG_BYTES,
    RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1, RELAYED_FAMILY_RELEASE_ID_V1,
    RELAYED_RECORD_PDA_DOMAIN_V1, RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1, RELAYER_KEY_SET_BYTES,
    RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1, SOLANA_MAINNET_GENESIS_HASH_V1,
    frame::{RelayAccountPrivilegeV1, RelayFrameKindV1, validate_relay_frame_v1},
    instruction::{
        APPEND_OBSERVATION_PREFIX_BYTES, AppendObservationInstructionV1,
        CommitDeadlineFailureInstructionV1, ConsumeRecordInstructionV1, CreateRecordInstructionV1,
        RELAY_INSTRUCTION_MAGIC, RelayInstructionV1, RetireRecordInstructionV1,
        SEAL_RECORD_PREFIX_BYTES, SealRecordInstructionV1,
    },
    record::{
        RelayedObservationRecordViewV1, RelayedRecordBindingV1,
        append_relayed_observation_in_place_v1, consume_relayed_observation_in_place_v1,
        create_relayed_observation_record_into_v1, relayed_observation_record_bytes_v1,
        retire_relayed_observation_in_place_v1, seal_relayed_observation_in_place_v1,
    },
    release::{
        AccountSetEntryV1, RelayedAdapterConfigV1, RelayerKeySetV1, account_set_id_preimage_len_v1,
        decode_account_set_entry_v1, encode_account_set_id_preimage_v1,
        encode_set_digest_seed_preimage_v1,
    },
    signature::{
        ED25519_PROGRAM_ID_3_0, Ed25519InstructionViewV1, inspect_preceding_relay_signature_v1,
    },
    wire::{AttestationMessageV1, ObservationSetSealV1},
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1, ProviderReleaseV1,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SOURCE_MATERIAL_V2_BYTES,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_SPEC_BYTES, SOURCE_SPEC_SCHEMA_ID_V1,
    SourceAccessProfile, SourceMaterialV2, SourceResolutionStateV2, SourceSpecV1,
    WINDOW_SPEC_BYTES, WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, create_account, transfer};

use crate::{
    RecordKind, ResolutionError, authenticate_clock, authenticate_finalized_record,
    authenticate_rent,
    funded::{
        AuthenticatedFailureFundingV1, AuthenticatedWalkSourceV1, DeadlineFailureRequestV1,
        FundedWalkErrorV1, plan_deadline_failure_v1,
    },
    provider_instruction_v3::authenticate_record,
    relay_v1::{
        AuthenticatedRelaySourceRecordsV1, RelayJoinErrorV1, RelayResolutionRequestV1,
        plan_relayed_resolution_v1,
    },
};

/// Return whether bytes select one relay transport route.
pub(crate) fn is_relay_transport_v1(bytes: &[u8]) -> bool {
    bytes.get(..RELAY_INSTRUCTION_MAGIC.len()) == Some(&RELAY_INSTRUCTION_MAGIC)
}

/// Dispatch one exact relay instruction after top-level magic routing.
///
/// **Every arm below is `#[inline(never)]`, and that is load-bearing rather than
/// stylistic.** An SBF stack frame is four kilobytes and `cargo build-sbf` exits
/// *zero* when the backend reports that a call overwrites its own frame, so a
/// dispatcher that inlines several large handlers ships a potentially-undefined
/// artifact with a green build. This one did: the journey tier measured
/// sixty-five such diagnostics against this symbol, every one in the whole
/// seven-artifact build. Keeping each handler out of line makes the dispatcher's
/// own frame the size of one match rather than the union of five bodies, which
/// is the structural version of the fix instead of one that depends on whatever
/// the inliner decided this week.
///
/// The check is on the build OUTPUT, not the exit code:
/// `cargo build-sbf --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml
/// 2>&1 | grep -c 'overwrites values in the frame'` must print `0`.
#[inline(never)]
pub(crate) fn process_relay_transport_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    match RelayInstructionV1::decode(instruction_data).map_err(|_| ResolutionError::Instruction)? {
        RelayInstructionV1::CreateRecord(request) => {
            process_create_record(program_id, accounts, request)
        }
        RelayInstructionV1::AppendObservation(request, message) => {
            process_append(program_id, accounts, instruction_data, request, message)
        }
        RelayInstructionV1::SealRecord(request, message) => {
            process_seal(program_id, accounts, instruction_data, request, message)
        }
        RelayInstructionV1::RetireRecord(request) => process_retire(program_id, accounts, request),
        RelayInstructionV1::ConsumeRecord(request, entries) => {
            process_consume(program_id, accounts, request, entries)
        }
        RelayInstructionV1::CommitDeadlineFailure(request) => {
            process_commit_deadline_failure(program_id, accounts, request)
        }
    }
}

/// The immutable release facts creation re-derives rather than trusts.
struct ReleaseFacts {
    provider_release_id: [u8; 32],
    relayer_key_set_id: [u8; 32],
    account_set_id: [u8; 32],
    key_set: RelayerKeySetV1,
}

/// What the authenticated Core Market says, and nothing the caller said.
struct MarketFacts {
    registry_program: Pubkey,
    rent_beneficiary: [u8; 32],
    product_record: [u8; 32],
    capability_manifest: [u8; 32],
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(ResolutionError::AccountFrame.into())
}

fn validate_frame(kind: RelayFrameKindV1, accounts: &[AccountInfo<'_>]) -> ProgramResult {
    let mut observed = Vec::new();
    observed
        .try_reserve_exact(accounts.len())
        .map_err(|_| ResolutionError::Arithmetic)?;
    for info in accounts {
        observed.push(RelayAccountPrivilegeV1 {
            key: info.key.to_bytes(),
            is_signer: info.is_signer,
            is_writable: info.is_writable,
        });
    }
    validate_relay_frame_v1(kind, &observed).map_err(|_| ResolutionError::AccountFrame)?;
    Ok(())
}

fn require_system(account: &AccountInfo<'_>) -> ProgramResult {
    if account.key != &system_program::ID || !account.executable {
        return Err(ResolutionError::AccountFrame.into());
    }
    Ok(())
}

/// Authenticate the Core Market and this Program's Resolution role under it.
///
/// The Core Program is a named account rather than a constant, and the Market's
/// own owner and derived address are what pin it: a state account owned by
/// `core` and equal to `MarketCoreStateSeedsV2` under `core` cannot be a Core
/// Market of some other Core. The activation cache then closes the loop in the
/// other direction — the release set this Market selected must name this
/// executing Program as its Resolution role.
fn authenticate_market(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    core: &AccountInfo<'_>,
    activation: &AccountInfo<'_>,
    generation: u64,
    source_material_id: [u8; 32],
) -> Result<MarketFacts, ProgramError> {
    if !core.executable || activation.executable {
        return Err(ResolutionError::AccountFrame.into());
    }
    let market_data = market
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let state = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    if market.owner != core.key
        || market.executable
        || state.phase != CorePhase::Open
        || state.readiness != CoreReadiness::Consumed
        || state.identity.generation != generation
        || state.identity.resolution_policy.to_bytes() != source_material_id
        || Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
            core.key,
        )
        .0 != *market.key
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    let registry_program = Pubkey::new_from_array(state.identity.registry_program.to_bytes());
    let release_set = state.identity.selected_release_set.to_bytes();
    let rent_beneficiary = state.rent_beneficiary.to_bytes();
    let product_record = state.identity.product_record.to_bytes();
    // Which manifest quotes this market's funding is a Market fact, never an
    // argument: the deadline walk debits an escrow, and a caller who could name
    // the manifest could name one that quotes a bounty this market never paid.
    let capability_manifest = state.identity.capability_manifest.to_bytes();
    drop(market_data);

    let activation_data = activation
        .try_borrow_data()
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activation.owner != &registry_program
        || activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || activation.key
            != &Pubkey::find_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
                &registry_program,
            )
            .0
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    let selected = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ResolutionRelease)?
        .to_bytes()
        != release_set
        || selected.release().program().to_bytes() != program_id.to_bytes()
    {
        return Err(ResolutionError::ResolutionRelease.into());
    }
    drop(activation_data);
    Ok(MarketFacts {
        registry_program,
        rent_beneficiary,
        product_record,
        capability_manifest,
    })
}

/// Re-derive every immutable release fact from the authenticated Source graph.
///
/// Nothing here is taken from the instruction beyond the material identity the
/// Market itself persists: the caller names accounts, and each one has to hash
/// to the identity the previous link already committed to.
#[allow(clippy::too_many_arguments)]
fn release_facts(
    registry: &Pubkey,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
    material_id: [u8; 32],
    source_spec_id: [u8; 32],
) -> Result<ReleaseFacts, ProgramError> {
    let material_data = account(accounts, 5)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 5)?,
        account(accounts, 6)?,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        material_id,
        &material_data,
        SOURCE_MATERIAL_V2_BYTES,
    )?;
    let material =
        SourceMaterialV2::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if material.primary_source_spec().to_bytes() != source_spec_id {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let window_spec_id = material.window_spec().to_bytes();
    drop(material_data);

    let spec_data = account(accounts, 7)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 7)?,
        account(accounts, 8)?,
        rent,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id,
        &spec_data,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&spec_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if source.access_profile() != SourceAccessProfile::RelayedObservationRecord {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let provider_release_id = source.provider_release_id().to_bytes();
    drop(spec_data);

    let provider_data = account(accounts, 9)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 9)?,
        account(accounts, 10)?,
        rent,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_id,
        &provider_data,
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider =
        ProviderReleaseV1::decode(&provider_data).map_err(|_| ResolutionError::ProviderRelease)?;
    if provider.provider_family_id().to_bytes() != RELAYED_FAMILY_RELEASE_ID_V1
        || provider.transport_profile_id().to_bytes() != RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    let relayer_key_set_id = provider.provider_deployment_release_id().to_bytes();
    let decoding_rules_id = provider.decoding_rules_id().to_bytes();
    drop(provider_data);

    let window_data = account(accounts, 11)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 11)?,
        account(accounts, 12)?,
        rent,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id,
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    window
        .validate_source(
            dclutch_source_contract::ContentId::new(source_spec_id)
                .map_err(|_| ResolutionError::SourceMaterial)?,
        )
        .map_err(|_| ResolutionError::SourceMaterial)?;
    let window_max_age_seconds = window.max_age_seconds();
    drop(window_data);

    let key_set_data = account(accounts, 13)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 13)?,
        account(accounts, 14)?,
        rent,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        relayer_key_set_id,
        &key_set_data,
        RELAYER_KEY_SET_BYTES,
    )?;
    let key_set =
        RelayerKeySetV1::decode(&key_set_data).map_err(|_| ResolutionError::ProviderRelease)?;
    drop(key_set_data);

    let config_data = account(accounts, 15)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 15)?,
        account(accounts, 16)?,
        rent,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        decoding_rules_id,
        &config_data,
        RELAYED_ADAPTER_CONFIG_BYTES,
    )?;
    let config = RelayedAdapterConfigV1::decode(&config_data)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    // Section 4.7's founding-time admission predicate, enforced where a record
    // first comes into existence: the window's own liveness grace must cover the
    // declared two-clock skew allowance, so skew alone can never be the thing
    // that walks a market to its funded failure outcome.
    config
        .require_window_admits_skew(window_max_age_seconds)
        .map_err(|_| ResolutionError::Transition)?;
    let account_set_id = config.account_set_id();
    drop(config_data);

    Ok(ReleaseFacts {
        provider_release_id,
        relayer_key_set_id,
        account_set_id,
        key_set,
    })
}

fn record_binding(
    market: &AccountInfo<'_>,
    generation: u64,
    material_id: [u8; 32],
    account_set_id: [u8; 32],
    provider_release_id: [u8; 32],
    relayer_key_set_id: [u8; 32],
    observed_slot: u64,
) -> RelayedRecordBindingV1 {
    RelayedRecordBindingV1 {
        market: market.key.to_bytes(),
        generation,
        source_material_id: material_id,
        account_set_id,
        provider_release_id,
        relayer_key_set_id,
        observed_cluster_id: SOLANA_MAINNET_GENESIS_HASH_V1,
        observed_slot,
    }
}

fn record_pda_seeds<'a>(
    market: &'a Pubkey,
    generation: &'a [u8; 8],
    account_set_id: &'a [u8; 32],
    observed_slot: &'a [u8; 8],
    bump: &'a [u8; 1],
) -> [&'a [u8]; 6] {
    [
        RELAYED_RECORD_PDA_DOMAIN_V1,
        market.as_ref(),
        generation.as_slice(),
        account_set_id.as_slice(),
        observed_slot.as_slice(),
        bump.as_slice(),
    ]
}

fn authenticate_record_account(
    program_id: &Pubkey,
    record: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
) -> ProgramResult {
    if record.owner != program_id || record.executable {
        return Err(ResolutionError::OutputState.into());
    }
    let data = record
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let view =
        RelayedObservationRecordViewV1::decode(&data).map_err(|_| ResolutionError::OutputState)?;
    let seeds = view.pda_seeds().map_err(|_| ResolutionError::OutputState)?;
    if seeds.market() != market.key.to_bytes() {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

/// Read the Registry program back out of the Market the record already names.
///
/// Fill and seal do not re-walk the Source graph — creation walked it once and
/// persisted its conclusions — but they still have to authenticate the raw key
/// set they are handed, and the program that owns raw records is a fact of the
/// Market rather than of the caller.
fn registry_of(market: &AccountInfo<'_>, record: &AccountInfo<'_>) -> Result<Pubkey, ProgramError> {
    {
        let data = record
            .try_borrow_data()
            .map_err(|_| ResolutionError::OutputState)?;
        let view = RelayedObservationRecordViewV1::decode(&data)
            .map_err(|_| ResolutionError::OutputState)?;
        if view.market().map_err(|_| ResolutionError::OutputState)? != market.key.to_bytes() {
            return Err(ResolutionError::MarketAuthority.into());
        }
    }
    let data = market
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let state = CoreState::decode(&data).map_err(|_| ResolutionError::MarketAuthority)?;
    Ok(Pubkey::new_from_array(
        state.identity.registry_program.to_bytes(),
    ))
}

#[inline(never)]
fn process_create_record(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CreateRecordInstructionV1,
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::CreateRecord, accounts)?;
    let worker = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let core = account(accounts, 2)?;
    let activation = account(accounts, 3)?;
    let record_account = account(accounts, 4)?;
    let beneficiary = account(accounts, 17)?;
    let rent_sysvar = account(accounts, 18)?;
    let clock_account = account(accounts, 19)?;
    let system = account(accounts, 20)?;
    require_system(system)?;
    let rent = authenticate_rent(rent_sysvar)?;
    let clock = authenticate_clock(clock_account)?;

    let market = authenticate_market(
        program_id,
        market_account,
        core,
        activation,
        request.generation(),
        request.source_material_id(),
    )?;
    // One Market, one rent beneficiary. Core already persists it, so the
    // request may only echo it; a record cannot name a different destination
    // for the lamports it is about to hold.
    if beneficiary.key.to_bytes() != request.rent_beneficiary()
        || beneficiary.key.to_bytes() != market.rent_beneficiary
    {
        return Err(ResolutionError::MarketAuthority.into());
    }

    let facts = release_facts(
        &market.registry_program,
        accounts,
        &rent,
        request.source_material_id(),
        request.source_spec_id(),
    )?;
    if facts.key_set.seal_threshold() != request.seal_threshold() {
        // The threshold is a release parameter, never an instruction one.
        return Err(ResolutionError::Transition.into());
    }

    let generation = request.generation().to_le_bytes();
    let observed_slot = request.observed_slot().to_le_bytes();
    let bump = [request.pda_bump()];
    let signer = record_pda_seeds(
        market_account.key,
        &generation,
        &facts.account_set_id,
        &observed_slot,
        &bump,
    );
    let expected = Pubkey::create_program_address(&signer, program_id)
        .map_err(|_| ResolutionError::OutputState)?;
    if record_account.key != &expected {
        // This is the equivocation bound: the address is a function of the
        // observed slot, so a second contradictory observation of the same set
        // at the same slot has nowhere to live.
        return Err(ResolutionError::OutputState.into());
    }

    let width = relayed_observation_record_bytes_v1(request.set_count())
        .map_err(|_| ResolutionError::Instruction)?;
    create_prefunded_pda(
        worker,
        record_account,
        system,
        rent.minimum_balance(width),
        width,
        program_id,
        &signer,
    )?;

    let mut seed_preimage = [0u8; dclutch_relay_contract::release::SET_DIGEST_SEED_PREIMAGE_BYTES];
    encode_set_digest_seed_preimage_v1(
        &mut seed_preimage,
        facts.account_set_id,
        request.observed_slot(),
    )
    .map_err(|_| ResolutionError::Transition)?;
    let seed_digest = hash(&seed_preimage).to_bytes();

    let mut data = record_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    create_relayed_observation_record_into_v1(
        &mut data,
        record_binding(
            market_account,
            request.generation(),
            request.source_material_id(),
            facts.account_set_id,
            facts.provider_release_id,
            facts.relayer_key_set_id,
            request.observed_slot(),
        ),
        request.rent_beneficiary(),
        request.set_count(),
        request.seal_threshold(),
        seed_digest,
        clock.unix_timestamp,
    )
    .map_err(|_| ResolutionError::Transition)?;
    Ok(())
}

#[inline(never)]
fn process_append(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    request: AppendObservationInstructionV1,
    message: &[u8],
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::AppendObservation, accounts)?;
    let market_account = account(accounts, 1)?;
    let record_account = account(accounts, 2)?;
    let key_set_raw = account(accounts, 3)?;
    let key_set_staging = account(accounts, 4)?;
    let rent_sysvar = account(accounts, 5)?;
    let instructions = account(accounts, 6)?;
    let clock_account = account(accounts, 7)?;
    let rent = authenticate_rent(rent_sysvar)?;
    authenticate_clock(clock_account)?;
    authenticate_record_account(program_id, record_account, market_account)?;

    let attestation =
        AttestationMessageV1::decode(message).map_err(|_| ResolutionError::ProviderObservation)?;
    let signer = authenticate_adjacent_signature(
        program_id,
        accounts,
        instruction_data,
        instructions,
        APPEND_OBSERVATION_PREFIX_BYTES,
        message.len(),
    )?;

    let persisted = persisted_binding(
        market_account,
        record_account,
        key_set_raw,
        key_set_staging,
        &rent,
        request.generation(),
        request.observed_slot(),
    )?;
    // Filling is 1-of-n: any single member may complete a record, and the
    // quorum only certifies it afterwards.  A member who fills a record with
    // false bytes cannot get it sealed, so a bad fill is a wasted rent deposit
    // and a permanent signed lie, never a denial of service.
    persisted
        .key_set
        .require_member(&signer)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if attestation.relay_family_id() != RELAYED_FAMILY_RELEASE_ID_V1 {
        return Err(ResolutionError::ProviderRelease.into());
    }
    // The attestation's `decoding_rules_id` is not compared here. Filling only
    // moves bytes the signer committed to; the decoding rules are what turn
    // those bytes into an observation, so their identity is checked where they
    // are actually applied, at resolution. A relayer that echoes the wrong
    // rules identity has signed a statement that no resolution will accept.

    let body_width = attestation.body().encoded_len();
    let body = message
        .get(message.len().saturating_sub(body_width)..)
        .ok_or(ResolutionError::ProviderObservation)?;

    let mut data = record_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let running = {
        let view = RelayedObservationRecordViewV1::decode(&data)
            .map_err(|_| ResolutionError::OutputState)?;
        view.set_digest()
            .map_err(|_| ResolutionError::OutputState)?
    };
    let folded = hashv(&[running.as_slice(), body]).to_bytes();
    append_relayed_observation_in_place_v1(&mut data, persisted.binding, attestation, folded)
        .map_err(|_| ResolutionError::Transition)?;
    Ok(())
}

#[inline(never)]
fn process_seal(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    request: SealRecordInstructionV1,
    message: &[u8],
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::SealRecord, accounts)?;
    let market_account = account(accounts, 1)?;
    let record_account = account(accounts, 2)?;
    let key_set_raw = account(accounts, 3)?;
    let key_set_staging = account(accounts, 4)?;
    let rent_sysvar = account(accounts, 5)?;
    let instructions = account(accounts, 6)?;
    let clock_account = account(accounts, 7)?;
    let rent = authenticate_rent(rent_sysvar)?;
    let clock = authenticate_clock(clock_account)?;
    authenticate_record_account(program_id, record_account, market_account)?;

    let seal =
        ObservationSetSealV1::decode(message).map_err(|_| ResolutionError::ProviderObservation)?;
    let signer = authenticate_adjacent_signature(
        program_id,
        accounts,
        instruction_data,
        instructions,
        SEAL_RECORD_PREFIX_BYTES,
        message.len(),
    )?;

    let persisted = persisted_binding(
        market_account,
        record_account,
        key_set_raw,
        key_set_staging,
        &rent,
        request.generation(),
        request.observed_slot(),
    )?;
    // Sealing is m-of-n and the member's position in the release key set is
    // what the bitmap records, so one member cannot reach a quorum alone.
    let member = persisted
        .key_set
        .require_member(&signer)
        .map_err(|_| ResolutionError::ProviderObservation)?;

    let mut data = record_account
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    seal_relayed_observation_in_place_v1(
        &mut data,
        persisted.binding,
        seal,
        member,
        clock.unix_timestamp,
    )
    .map_err(|_| ResolutionError::Transition)?;
    Ok(())
}

#[inline(never)]
fn process_retire(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RetireRecordInstructionV1,
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::RetireRecord, accounts)?;
    let market_account = account(accounts, 1)?;
    let record_account = account(accounts, 2)?;
    let beneficiary = account(accounts, 3)?;
    authenticate_record_account(program_id, record_account, market_account)?;

    let (persisted_beneficiary, created) = {
        let data = record_account
            .try_borrow_data()
            .map_err(|_| ResolutionError::OutputState)?;
        let view = RelayedObservationRecordViewV1::decode(&data)
            .map_err(|_| ResolutionError::OutputState)?;
        (
            view.rent_credit_beneficiary()
                .map_err(|_| ResolutionError::OutputState)?,
            view.created_unix_seconds()
                .map_err(|_| ResolutionError::OutputState)?,
        )
    };
    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::MarketAuthority)?;
    let state = CoreState::decode(&market_data).map_err(|_| ResolutionError::MarketAuthority)?;
    if state.identity.generation != request.generation()
        || state.rent_beneficiary.to_bytes() != persisted_beneficiary
        || beneficiary.key.to_bytes() != persisted_beneficiary
    {
        return Err(ResolutionError::MarketAuthority.into());
    }
    drop(market_data);

    {
        let mut data = record_account
            .try_borrow_mut_data()
            .map_err(|_| ResolutionError::OutputState)?;
        retire_relayed_observation_in_place_v1(&mut data, request.generation(), created)
            .map_err(|_| ResolutionError::Transition)?;
    }
    close_to_beneficiary(record_account, beneficiary)
}

/// Consume one sealed record into the Source's terminal result.
///
/// This is the route the family existed for. Everything before it moved bytes
/// nobody had read: creation proved a record could exist under this Market, fill
/// and seal proved a release-pinned quorum stood behind the bytes, and retire
/// gave the rent back. None of that resolves anything. This does, and it is the
/// only route in the family that touches Source state.
///
/// What it authenticates, in order, each refusing on its own field:
///
/// 1. the frame — twenty-eight positions, three writable, no aliases;
/// 2. the Market, its Core ownership, its derived address, and this Program as
///    its Resolution role;
/// 3. the Source graph, link by link, from the material the Market itself names;
/// 4. the venue's pinned `ArtifactReleaseV1`, named by the Source spec;
/// 5. the Product Runtime V2 graph, against the Market's own Product record;
/// 6. the record account's program custody and its slot-seeded address;
/// 7. the Source state account's own derived address;
/// 8. the pinned account set, by re-derived digest over caller-supplied entries.
///
/// Only then does [`crate::relay_v1::plan_relayed_resolution_v1`] read a byte of
/// what the relayer signed.
#[inline(never)]
fn process_consume(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ConsumeRecordInstructionV1,
    entry_bytes: &[u8],
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::ConsumeRecord, accounts)?;
    let worker = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let core = account(accounts, 2)?;
    let activation = account(accounts, 3)?;
    let record_account = account(accounts, 4)?;
    let source_state_account = account(accounts, 5)?;
    let certificate_account = account(accounts, 6)?;
    let clock_account = account(accounts, 25)?;
    let rent_sysvar = account(accounts, 26)?;
    let system = account(accounts, 27)?;
    require_system(system)?;
    let _ = worker;
    let rent = authenticate_rent(rent_sysvar)?;
    let clock = authenticate_clock(clock_account)?;

    let market = authenticate_market(
        program_id,
        market_account,
        core,
        activation,
        request.generation(),
        request.source_material_id(),
    )?;
    let records = boxed_consume_source_records(
        &market,
        accounts,
        &rent,
        request.source_material_id(),
        request.source_spec_id(),
    )?;
    let product_runtime = boxed_product_runtime(
        &market.registry_program,
        &rent,
        ProductContentId::new(market.product_record).map_err(|_| ResolutionError::ProductDomain)?,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: account(accounts, 19)?,
                staging: account(accounts, 20)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: account(accounts, 21)?,
                staging: account(accounts, 22)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: account(accounts, 23)?,
                staging: account(accounts, 24)?,
            },
        },
    )?;

    authenticate_consumable_record(
        program_id,
        record_account,
        market_account,
        records.config.account_set_id(),
        request.generation(),
        request.observed_slot(),
    )?;
    authenticate_source_state_account(program_id, source_state_account, market_account)?;

    // Decoded once and kept on the heap. The set is up to eight sixty-six-byte
    // entries and the SBF stack frame is four kilobytes; decoding it twice, once
    // for the digest and once for the positions, is what made the dispatcher's
    // frame overflow the first time this route was compiled.
    let entries = boxed_entries(entry_bytes, request.entry_count())?;
    let entries = entries
        .get(..usize::from(request.entry_count()))
        .ok_or(ResolutionError::Instruction)?;
    let recomputed_account_set_id = recompute_account_set_id(entries)?;

    let domain_data = account(accounts, 21)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let result_domain =
        ResultDomainV2::decode(&domain_data).map_err(|_| ResolutionError::ProductDomain)?;
    let record_data = record_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let record = RelayedObservationRecordViewV1::decode(&record_data)
        .map_err(|_| ResolutionError::OutputState)?;
    let source_data = source_state_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let source_state = Box::new(
        SourceResolutionStateV2::decode(&source_data).map_err(|_| ResolutionError::OutputState)?,
    );

    let plan = plan_relayed_resolution_v1(
        &RelayResolutionRequestV1 {
            market: market_account.key.to_bytes(),
            generation: request.generation(),
            terminal_sequence: request.terminal_sequence(),
            certificate_account: certificate_account.key.to_bytes(),
            record_account: record_account.key.to_bytes(),
            pinned_cluster_id: SOLANA_MAINNET_GENESIS_HASH_V1,
            current_unix_seconds: clock.unix_timestamp,
        },
        &source_state,
        &records,
        &product_runtime,
        result_domain,
        record,
        entries,
        recomputed_account_set_id,
    )
    .map_err(map_relay_join_error)?;

    let next_source = Box::new(plan.next_source.to_bytes());
    let certificate = Box::new(
        plan.certificate
            .to_bytes()
            .map_err(|_| ResolutionError::Transition)?,
    );
    drop(source_data);
    drop(record_data);
    drop(domain_data);
    commit_consumption(
        program_id,
        request.terminal_sequence(),
        source_state_account,
        certificate_account,
        record_account,
        system,
        &rent,
        &next_source,
        &certificate,
    )
}

/// Walk a silent market to its Product's pre-disclosed failure outcome.
///
/// This is the route section 4.8 of `MAINNET_STATE_RELAY.md` promised and the
/// one the family could not have without it: **a silent relayer cannot make a
/// market unresolvable, only drive it to a pre-disclosed outcome, along a
/// bounded, prepaid, permissionless path that pays whoever walks it.**
///
/// Every noun in that sentence is an account or a check here, and none of them
/// is the relayer's. The narrowest instruction in the family carries only a
/// generation and a terminal sequence; even the Source material identity is read
/// out of the Resolution-owned Source state rather than accepted from the
/// caller, and then compared against what the Market itself says its resolution
/// policy is. A caller supplies which market, when, and nothing else.
///
/// What it authenticates, in order, each refusing on its own field:
///
/// 1. the frame — twenty-two positions, four writable, no aliases;
/// 2. the Source state's program custody and its own derived address;
/// 3. the Market, its Core ownership, its derived address, this Program as its
///    Resolution role, and that its resolution policy is the material the state
///    is bound to;
/// 4. the `SourceMaterialV2` record, and the `WindowSpecV1` it names;
/// 5. the Product Runtime V2 graph, against the Market's own Product record;
/// 6. the `CapabilityManifestV1` the Market names, and the explicit-failure
///    compartment's derived address under it.
///
/// Only then does [`crate::funded::plan_deadline_failure_v1`] decide anything,
/// and it debits before it transitions so that a walk that cannot be paid for
/// cannot move the market either.
#[inline(never)]
fn process_commit_deadline_failure(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CommitDeadlineFailureInstructionV1,
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::CommitDeadlineFailure, accounts)?;
    let worker = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let core = account(accounts, 2)?;
    let activation = account(accounts, 3)?;
    let source_state_account = account(accounts, 4)?;
    let certificate_account = account(accounts, 5)?;
    let funding_account = account(accounts, 18)?;
    let clock_account = account(accounts, 19)?;
    let rent_sysvar = account(accounts, 20)?;
    let system = account(accounts, 21)?;
    require_system(system)?;
    let rent = authenticate_rent(rent_sysvar)?;
    let clock = authenticate_clock(clock_account)?;

    // The material identity comes from the Source state, which this Program
    // owns and derives, and is then checked against the Market's own resolution
    // policy inside `authenticate_market`. Neither side is the caller.
    authenticate_source_state_account(program_id, source_state_account, market_account)?;
    let source_state = Box::new({
        let data = source_state_account
            .try_borrow_data()
            .map_err(|_| ResolutionError::OutputState)?;
        SourceResolutionStateV2::decode(&data).map_err(|_| ResolutionError::OutputState)?
    });
    let material_id = source_state.material_id();

    let market = authenticate_market(
        program_id,
        market_account,
        core,
        activation,
        request.generation(),
        material_id.to_bytes(),
    )?;
    let walk_source = Box::new(deadline_walk_source(
        &market.registry_program,
        accounts,
        &rent,
        material_id,
    )?);
    let product_runtime = boxed_product_runtime(
        &market.registry_program,
        &rent,
        ProductContentId::new(market.product_record).map_err(|_| ResolutionError::ProductDomain)?,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: account(accounts, 10)?,
                staging: account(accounts, 11)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: account(accounts, 12)?,
                staging: account(accounts, 13)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: account(accounts, 14)?,
                staging: account(accounts, 15)?,
            },
        },
    )?;

    let manifest_data = account(accounts, 16)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    authenticate_finalized_record(
        market.registry_program,
        account(accounts, 16)?,
        account(accounts, 17)?,
        &rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market.capability_manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id = CapabilityContentId::new(market.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    let escrow = Box::new(authenticate_failure_funding(
        program_id,
        funding_account,
        market_account,
        manifest_id,
        manifest,
        request.generation(),
        &rent,
    )?);

    let domain_data = account(accounts, 12)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let result_domain =
        ResultDomainV2::decode(&domain_data).map_err(|_| ResolutionError::ProductDomain)?;

    let outputs = plan_and_encode_deadline_failure(
        &DeadlineFailureRequestV1 {
            market: market_account.key.to_bytes(),
            generation: request.generation(),
            terminal_sequence: request.terminal_sequence(),
            certificate_account: certificate_account.key.to_bytes(),
            current_unix_seconds: clock.unix_timestamp,
        },
        &source_state,
        &walk_source,
        &product_runtime,
        result_domain,
        &escrow,
    )?;

    let worker_lamports_after = worker
        .lamports()
        .checked_add(outputs.work_paid)
        .ok_or(ResolutionError::Arithmetic)?;
    drop(domain_data);
    drop(manifest_data);
    commit_deadline_failure(
        program_id,
        request.terminal_sequence(),
        DeadlineFailureOutputs {
            source_state: source_state_account,
            certificate: certificate_account,
            funding: funding_account,
            worker,
            system,
        },
        &rent,
        &outputs,
        worker_lamports_after,
    )
}

/// The three account bodies and the lamport figure a completed walk writes.
///
/// This exists to keep [`DeadlineFailurePlanV1`] out of the dispatch arm's own
/// stack frame. The plan carries a `SourceResolutionStateV2`, a
/// `ResolutionCertificateV2` and a `FundingStateV1` by value — about eight
/// hundred bytes of decoded state — and encoding it produces another eight
/// hundred of wire bytes beside them. The SBF frame is four kilobytes and the
/// arm already holds the authenticated Market, Source graph and escrow, so the
/// two together overflowed it: `cargo build-sbf` reported nine
/// stack-frame-overwrite diagnostics against `process_commit_deadline_failure`
/// and exited zero anyway. Boxing the result is not enough on its own, because
/// the plan is still returned *through* the caller's frame; the planning and
/// the encoding have to happen somewhere else entirely.
struct EncodedDeadlineFailureV1 {
    /// `SourceResolutionStateV2` after `Primary → Exhausted → FailureCommitted`.
    source: [u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    /// The terminal `ResolutionFailure` certificate, already schema-validated.
    certificate: [u8; RESOLUTION_CERTIFICATE_BYTES_V2],
    /// `FundingStateV1` after the bounty debit.
    funding: [u8; FUNDING_STATE_BYTES],
    /// Exact lamports the walker is owed.
    work_paid: u64,
    /// Exact funding-account lamports after the debit.
    funding_lamports_after: u64,
}

/// Plan the walk and encode every byte it writes, on a frame of its own.
///
/// The encoding happens here rather than at the commit, and that ordering is
/// the point: `ResolutionCertificateV2::to_bytes` runs `validate_shape`, which
/// is where the Lean-owned schema refuses a `ResolutionFailure` carrying a zero
/// `funding_allocation` or `work_paid`. A certificate the schema would refuse
/// therefore never reaches an account, and no lamport has moved when it is
/// refused.
#[inline(never)]
fn plan_and_encode_deadline_failure(
    request: &DeadlineFailureRequestV1,
    source_state: &SourceResolutionStateV2,
    walk_source: &AuthenticatedWalkSourceV1,
    product_runtime: &AuthenticatedProductRuntimeV2,
    result_domain: ResultDomainV2<'_>,
    escrow: &AuthenticatedFailureFundingV1<'_>,
) -> Result<Box<EncodedDeadlineFailureV1>, ProgramError> {
    let plan = plan_deadline_failure_v1(
        request,
        source_state,
        walk_source,
        product_runtime,
        result_domain,
        escrow,
    )
    .map_err(map_funded_walk_error)?;
    let mut encoded = Box::new(EncodedDeadlineFailureV1 {
        source: [0; SOURCE_RESOLUTION_STATE_BYTES_V2],
        certificate: [0; RESOLUTION_CERTIFICATE_BYTES_V2],
        funding: [0; FUNDING_STATE_BYTES],
        work_paid: plan.work_paid,
        funding_lamports_after: plan.funding_lamports_after,
    });
    encoded.source = plan.next_source.to_bytes();
    encoded.certificate = plan
        .certificate
        .to_bytes()
        .map_err(|_| ResolutionError::Transition)?;
    encoded.funding = plan.next_funding.to_bytes();
    Ok(encoded)
}

const fn map_funded_walk_error(error: FundedWalkErrorV1) -> ResolutionError {
    match error {
        FundedWalkErrorV1::Request => ResolutionError::Instruction,
        FundedWalkErrorV1::Source => ResolutionError::SourceMaterial,
        FundedWalkErrorV1::Product => ResolutionError::ProductDomain,
        FundedWalkErrorV1::Transition => ResolutionError::Transition,
        FundedWalkErrorV1::Funding => ResolutionError::Funding,
    }
}

/// Authenticate exactly the two Source records the walk reads.
///
/// A consumption walks material → spec → provider release → configuration →
/// window → venue, because it has to interpret bytes a provider signed. This
/// walk interprets nothing: it needs the material (for the Product record and
/// the absence of a recovery policy) and the window (for the deadline), and
/// naming any of the others would make the route depend on the party that has
/// stopped answering.
#[inline(never)]
fn deadline_walk_source(
    registry: &Pubkey,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
    material_id: dclutch_source_contract::ContentId,
) -> Result<AuthenticatedWalkSourceV1, ProgramError> {
    let material_data = account(accounts, 6)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 6)?,
        account(accounts, 7)?,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        material_id.to_bytes(),
        &material_data,
        SOURCE_MATERIAL_V2_BYTES,
    )?;
    let material =
        SourceMaterialV2::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let window_spec_id = material.window_spec();
    drop(material_data);

    let window_data = account(accounts, 8)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 8)?,
        account(accounts, 9)?,
        rent,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id.to_bytes(),
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(window_data);

    Ok(AuthenticatedWalkSourceV1 {
        material_id,
        material,
        window_spec_id,
        window,
    })
}

/// Authenticate the escrowed explicit-failure compartment at its own address.
///
/// The address is the check that matters. `CapabilityFundingDerivationV1` folds
/// the Market, the generation, the manifest identity and the funding state's own
/// entry index into the seeds, so a caller cannot present the *recovery*
/// compartment — which is a real, active, correctly-owned `FundingState` of the
/// same market — and have the walk debit it: that account lives at a different
/// address than the one its own bytes derive.
#[inline(never)]
fn authenticate_failure_funding<'a>(
    program_id: &Pubkey,
    account_info: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'a>,
    generation: u64,
    rent: &Rent,
) -> Result<AuthenticatedFailureFundingV1<'a>, ProgramError> {
    if account_info.owner != program_id
        || account_info.executable
        || account_info.data_len() != FUNDING_STATE_BYTES
    {
        return Err(ResolutionError::Funding.into());
    }
    let data = account_info
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let funding = FundingStateV1::decode(&data).map_err(|_| ResolutionError::Funding)?;
    drop(data);
    let custody = FundingCustodyObservationV1::native_only(
        account_info.lamports(),
        rent.minimum_balance(FUNDING_STATE_BYTES),
    )
    .map_err(|_| ResolutionError::Funding)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market.key.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), program_id).0 != *account_info.key
    {
        return Err(ResolutionError::Funding.into());
    }
    Ok(AuthenticatedFailureFundingV1 {
        manifest_id,
        manifest,
        entry_index: funding.entry_index(),
        funding,
        custody,
    })
}

/// The four accounts a completed walk writes, named rather than indexed.
struct DeadlineFailureOutputs<'a, 'info> {
    source_state: &'a AccountInfo<'info>,
    certificate: &'a AccountInfo<'info>,
    funding: &'a AccountInfo<'info>,
    worker: &'a AccountInfo<'info>,
    system: &'a AccountInfo<'info>,
}

/// Commit the terminal failure, the debited escrow and the walker's payment.
///
/// All four move or none do, which is what makes the bounty a payment for work
/// rather than a claim about it: the certificate that says `work_paid` is the
/// same transaction that moved the lamports.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn commit_deadline_failure(
    program_id: &Pubkey,
    terminal_sequence: u64,
    outputs: DeadlineFailureOutputs<'_, '_>,
    rent: &Rent,
    encoded: &EncodedDeadlineFailureV1,
    worker_lamports_after: u64,
) -> ProgramResult {
    initialize_certificate_at_kind(
        program_id,
        RESOLUTION_FAILURE_CERTIFICATE_KIND_SEED,
        terminal_sequence,
        outputs.source_state,
        outputs.certificate,
        outputs.system,
        rent,
    )?;
    let mut state_output = outputs
        .source_state
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut certificate_output = outputs
        .certificate
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut funding_output = outputs
        .funding
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if state_output.len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || certificate_output.len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || funding_output.len() != FUNDING_STATE_BYTES
        || certificate_output.iter().any(|byte| *byte != 0)
    {
        return Err(ResolutionError::OutputState.into());
    }
    state_output.copy_from_slice(&encoded.source);
    certificate_output.copy_from_slice(&encoded.certificate);
    funding_output.copy_from_slice(&encoded.funding);

    let mut funding_lamports = outputs
        .funding
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut worker_lamports = outputs
        .worker
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    **funding_lamports = encoded.funding_lamports_after;
    **worker_lamports = worker_lamports_after;
    Ok(())
}

/// The Lean-owned Runtime V2 wire tag for `ResolutionFailure`.
///
/// The kind is a PDA *seed*, so the failure a deadline walk writes and the
/// success an observation writes live at different addresses for one Source
/// state at one sequence, and neither can overwrite the other.
const RESOLUTION_FAILURE_CERTIFICATE_KIND_SEED: u8 = 4;

const fn map_relay_join_error(error: RelayJoinErrorV1) -> ResolutionError {
    match error {
        RelayJoinErrorV1::Request => ResolutionError::Instruction,
        RelayJoinErrorV1::Source => ResolutionError::SourceMaterial,
        RelayJoinErrorV1::Product => ResolutionError::ProductDomain,
        RelayJoinErrorV1::Record => ResolutionError::RelayedRecord,
        RelayJoinErrorV1::Observation => ResolutionError::ProviderObservation,
        RelayJoinErrorV1::Window => ResolutionError::RelayedWindow,
        RelayJoinErrorV1::Transition => ResolutionError::Transition,
    }
}

/// Walk the Source graph a consumption needs, one authenticated record per link.
///
/// Creation walked the same chain and persisted its conclusions into the record;
/// this walks it again rather than trusting them, because a consumption maps a
/// result through a *Product*, and the records that decide how are not fields of
/// the record being consumed.
#[inline(never)]
fn boxed_product_runtime(
    registry: &Pubkey,
    rent: &Rent,
    product_record: ProductContentId,
    frame: ProductRuntimeFrameV2<'_, '_>,
) -> Result<Box<AuthenticatedProductRuntimeV2>, ProgramError> {
    Ok(Box::new(
        authenticate_product_runtime_v2(registry, rent, product_record, frame)
            .map_err(|_| ResolutionError::ProductDomain)?,
    ))
}

#[inline(never)]
fn boxed_consume_source_records(
    market: &MarketFacts,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
    material_id: [u8; 32],
    source_spec_id: [u8; 32],
) -> Result<Box<AuthenticatedRelaySourceRecordsV1>, ProgramError> {
    Ok(Box::new(consume_source_records(
        market,
        accounts,
        rent,
        material_id,
        source_spec_id,
    )?))
}

#[inline(never)]
fn consume_source_records(
    market: &MarketFacts,
    accounts: &[AccountInfo<'_>],
    rent: &Rent,
    material_id: [u8; 32],
    source_spec_id: [u8; 32],
) -> Result<AuthenticatedRelaySourceRecordsV1, ProgramError> {
    let registry = &market.registry_program;
    let material_data = account(accounts, 7)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 7)?,
        account(accounts, 8)?,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        material_id,
        &material_data,
        SOURCE_MATERIAL_V2_BYTES,
    )?;
    let material =
        SourceMaterialV2::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let window_spec_id = material.window_spec().to_bytes();
    drop(material_data);

    let spec_data = account(accounts, 9)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 9)?,
        account(accounts, 10)?,
        rent,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id,
        &spec_data,
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&spec_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if source.access_profile() != SourceAccessProfile::RelayedObservationRecord {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let provider_release_id = source.provider_release_id().to_bytes();
    let venue_release_id = source.adapter_config_id().to_bytes();
    drop(spec_data);

    let provider_data = account(accounts, 11)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 11)?,
        account(accounts, 12)?,
        rent,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_id,
        &provider_data,
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider =
        ProviderReleaseV1::decode(&provider_data).map_err(|_| ResolutionError::ProviderRelease)?;
    if provider.provider_family_id().to_bytes() != RELAYED_FAMILY_RELEASE_ID_V1
        || provider.transport_profile_id().to_bytes() != RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    let decoding_rules_id = provider.decoding_rules_id().to_bytes();
    drop(provider_data);

    let window_data = account(accounts, 13)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 13)?,
        account(accounts, 14)?,
        rent,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id,
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(window_data);

    let config_data = account(accounts, 15)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 15)?,
        account(accounts, 16)?,
        rent,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        decoding_rules_id,
        &config_data,
        RELAYED_ADAPTER_CONFIG_BYTES,
    )?;
    let config = RelayedAdapterConfigV1::decode(&config_data)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    config
        .require_window_admits_skew(window.max_age_seconds())
        .map_err(|_| ResolutionError::Transition)?;
    drop(config_data);

    let venue_data = account(accounts, 17)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 17)?,
        account(accounts, 18)?,
        rent,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        venue_release_id,
        &venue_data,
        ARTIFACT_RELEASE_BYTES_V1,
    )?;
    let venue_release =
        ArtifactReleaseV1::decode(&venue_data).map_err(|_| ResolutionError::ProviderRelease)?;
    drop(venue_data);

    let id = |value: [u8; 32]| {
        dclutch_source_contract::ContentId::new(value).map_err(|_| ResolutionError::SourceMaterial)
    };
    Ok(AuthenticatedRelaySourceRecordsV1 {
        material_id: id(material_id)?,
        material,
        source_spec_id: id(source_spec_id)?,
        source,
        provider_release_id: id(provider_release_id)?,
        provider_release: provider,
        decoding_rules_id: id(decoding_rules_id)?,
        config,
        window_spec_id: id(window_spec_id)?,
        window,
        venue_release_id: id(venue_release_id)?,
        venue_release,
    })
}

/// Authenticate the record's custody and its slot-seeded address.
///
/// The address is the equivocation bound and it is re-derived here from facts
/// the *configuration* supplies, not from the record's own header: a record that
/// merely claims an account set has to live at the address that set implies.
#[inline(never)]
fn authenticate_consumable_record(
    program_id: &Pubkey,
    record: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    account_set_id: [u8; 32],
    generation: u64,
    observed_slot: u64,
) -> ProgramResult {
    if record.owner != program_id || record.executable {
        return Err(ResolutionError::OutputState.into());
    }
    let generation_le = generation.to_le_bytes();
    let observed_slot_le = observed_slot.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RELAYED_RECORD_PDA_DOMAIN_V1,
            market.key.as_ref(),
            generation_le.as_slice(),
            account_set_id.as_slice(),
            observed_slot_le.as_slice(),
        ],
        program_id,
    )
    .0;
    if record.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

/// Authenticate the Source state account this consumption makes terminal.
#[inline(never)]
fn authenticate_source_state_account(
    program_id: &Pubkey,
    state: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
) -> ProgramResult {
    if state.owner != program_id
        || state.executable
        || state.data_len() != SOURCE_RESOLUTION_STATE_BYTES_V2
    {
        return Err(ResolutionError::OutputState.into());
    }
    let data = state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let decoded =
        SourceResolutionStateV2::decode(&data).map_err(|_| ResolutionError::OutputState)?;
    let seeds = decoded.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        program_id,
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if state.key != &expected || seeds.market() != market.key.to_bytes() {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

/// Re-derive the pinned account-set identity from caller-supplied entries.
///
/// This is where untrusted entries become authoritative, and the mechanism is
/// the only one available: re-encode the one canonical preimage the contract
/// writes, and hash it with the runtime's own SHA-256. The contract hashes
/// nothing, so the daemon's software implementation and this syscall agree by
/// construction rather than by two transcriptions of a rule.
#[inline(never)]
fn recompute_account_set_id(entries: &[AccountSetEntryV1]) -> Result<[u8; 32], ProgramError> {
    let width =
        account_set_id_preimage_len_v1(entries.len()).map_err(|_| ResolutionError::Instruction)?;
    let mut preimage = Vec::new();
    preimage
        .try_reserve_exact(width)
        .map_err(|_| ResolutionError::Arithmetic)?;
    preimage.resize(width, 0);
    encode_account_set_id_preimage_v1(
        &mut preimage,
        SOLANA_MAINNET_GENESIS_HASH_V1,
        RELAYED_FAMILY_RELEASE_ID_V1,
        entries,
    )
    .map_err(|_| ResolutionError::Instruction)?;
    Ok(hash(&preimage).to_bytes())
}

/// Decode the wire entries once, onto the heap.
///
/// Eight sixty-six-byte entries against a four-kilobyte SBF stack frame is not
/// a comfortable margin, and decoding them twice — once for the digest, once for
/// the positions — is what overflowed the dispatcher's frame the first time this
/// route was compiled.
#[inline(never)]
fn boxed_entries(
    entry_bytes: &[u8],
    count: u16,
) -> Result<Box<[AccountSetEntryV1; MAX_RELAYED_ACCOUNTS_V1]>, ProgramError> {
    if usize::from(count) > MAX_RELAYED_ACCOUNTS_V1 {
        return Err(ResolutionError::Instruction.into());
    }
    let mut output = Box::new(
        [AccountSetEntryV1 {
            key: [0; 32],
            expected_owner: [0; 32],
            inline_len: 0,
        }; MAX_RELAYED_ACCOUNTS_V1],
    );
    for index in 0..usize::from(count) {
        let entry = decode_account_set_entry_v1(entry_bytes, index)
            .map_err(|_| ResolutionError::Instruction)?;
        *output.get_mut(index).ok_or(ResolutionError::Instruction)? = entry;
    }
    Ok(output)
}

/// Write the three outputs, or none of them.
///
/// The record's phase advances last. Until it does, the record is still sealed
/// and this whole transaction is still revertible; once it is `Consumed`, the
/// same signed observation cannot resolve a second market state, which is the
/// replay bound the whole family rests on.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn commit_consumption<'info>(
    program_id: &Pubkey,
    terminal_sequence: u64,
    source_state: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    record: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
    next_source: &[u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    next_certificate: &[u8; RESOLUTION_CERTIFICATE_BYTES_V2],
) -> ProgramResult {
    initialize_certificate_at_kind(
        program_id,
        RESOLUTION_SUCCESS_CERTIFICATE_KIND_SEED,
        terminal_sequence,
        source_state,
        certificate,
        system,
        rent,
    )?;
    {
        let mut state_output = source_state
            .try_borrow_mut_data()
            .map_err(|_| ResolutionError::OutputState)?;
        let mut certificate_output = certificate
            .try_borrow_mut_data()
            .map_err(|_| ResolutionError::OutputState)?;
        if state_output.len() != SOURCE_RESOLUTION_STATE_BYTES_V2
            || certificate_output.len() != RESOLUTION_CERTIFICATE_BYTES_V2
            || certificate_output.iter().any(|byte| *byte != 0)
        {
            return Err(ResolutionError::OutputState.into());
        }
        state_output.copy_from_slice(next_source);
        certificate_output.copy_from_slice(next_certificate);
    }
    let mut record_data = record
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    consume_relayed_observation_in_place_v1(&mut record_data)
        .map_err(|_| ResolutionError::Transition)?;
    Ok(())
}

/// The Lean-owned Runtime V2 wire tag for `ResolutionSuccess`.
///
/// The kind is a PDA *seed*, so a success and a failure for one Source state at
/// one sequence live at different addresses and neither can overwrite the other.
/// `ResolutionFailure` has the tag `4` and no route emits it yet: see the
/// `CommitDeadlineFailure` arm above for why that is a refusal rather than an
/// omission.
const RESOLUTION_SUCCESS_CERTIFICATE_KIND_SEED: u8 = 1;

/// Allocate and assign a terminal certificate at its canonical address.
///
/// The domain and seed shape are the Resolution role's existing certificate
/// address space, deliberately: two provider families resolving one Market must
/// not have two certificate namespaces, or "the certificate for this Source
/// state" stops being a well-defined phrase. The kind is a *seed*, so a success
/// and a failure for one Source state at one sequence live at different
/// addresses and neither can quietly overwrite the other.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn initialize_certificate_at_kind<'info>(
    program_id: &Pubkey,
    kind: u8,
    terminal_sequence: u64,
    source_state: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    let kind_seed = [kind];
    let sequence_seed = terminal_sequence.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.key.as_ref(),
            &kind_seed,
            &sequence_seed,
        ],
        program_id,
    );
    if certificate.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    let minimum = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
    if certificate.owner == program_id {
        if certificate.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2
            || certificate.lamports() < minimum
            || certificate.executable
        {
            return Err(ResolutionError::OutputState.into());
        }
        return Ok(());
    }
    if certificate.owner != &system_program::ID
        || certificate.data_len() != 0
        || certificate.lamports() < minimum
        || certificate.executable
    {
        return Err(ResolutionError::OutputState.into());
    }
    let bump_seed = [bump];
    let signer = [
        RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
        source_state.key.as_ref(),
        kind_seed.as_slice(),
        sequence_seed.as_slice(),
        bump_seed.as_slice(),
    ];
    let space =
        u64::try_from(RESOLUTION_CERTIFICATE_BYTES_V2).map_err(|_| ResolutionError::Arithmetic)?;
    invoke_signed(
        &allocate(certificate.key, space),
        &[certificate.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    invoke_signed(
        &assign(certificate.key, program_id),
        &[certificate.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    Ok(())
}

/// Every binding a fill or a seal needs, read back out of the record.
///
/// This is deliberately NOT re-derived from the Source material. The material
/// chain was walked once, at creation, and its conclusions were persisted; a
/// later route re-deriving them would let a caller present a *different*
/// material account and quietly move a live record's authority. What a later
/// route must still prove is that the raw key-set account it presents hashes to
/// the identity the record already committed to.
struct PersistedBindingV1 {
    binding: RelayedRecordBindingV1,
    key_set: RelayerKeySetV1,
}

fn persisted_binding(
    market: &AccountInfo<'_>,
    record: &AccountInfo<'_>,
    key_set_raw: &AccountInfo<'_>,
    key_set_staging: &AccountInfo<'_>,
    rent: &Rent,
    generation: u64,
    observed_slot: u64,
) -> Result<PersistedBindingV1, ProgramError> {
    let registry = registry_of(market, record)?;
    let binding = {
        let data = record
            .try_borrow_data()
            .map_err(|_| ResolutionError::OutputState)?;
        let view = RelayedObservationRecordViewV1::decode(&data)
            .map_err(|_| ResolutionError::OutputState)?;
        let field = |value: Result<[u8; 32], dclutch_relay_contract::Error>| {
            value.map_err(|_| ResolutionError::OutputState)
        };
        RelayedRecordBindingV1 {
            market: market.key.to_bytes(),
            generation,
            source_material_id: field(view.source_material_id())?,
            account_set_id: field(view.account_set_id())?,
            provider_release_id: field(view.provider_release_id())?,
            relayer_key_set_id: field(view.relayer_key_set_id())?,
            observed_cluster_id: SOLANA_MAINNET_GENESIS_HASH_V1,
            observed_slot,
        }
    };
    let key_set_data = key_set_raw
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        &registry,
        key_set_raw,
        key_set_staging,
        rent,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        binding.relayer_key_set_id,
        &key_set_data,
        RELAYER_KEY_SET_BYTES,
    )?;
    let key_set =
        RelayerKeySetV1::decode(&key_set_data).map_err(|_| ResolutionError::ProviderRelease)?;
    Ok(PersistedBindingV1 { binding, key_set })
}

/// Authenticate the immediately preceding native Ed25519 instruction.
///
/// Adjacency selects which instruction to parse and nothing else: the signer is
/// then required to be a release-pinned key-set member by the caller, and the
/// message slice is required to be exactly the span of *this* instruction's own
/// data that carries the signed bytes.
fn authenticate_adjacent_signature(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    instructions: &AccountInfo<'_>,
    message_offset: usize,
    message_len: usize,
) -> Result<[u8; 32], ProgramError> {
    if instructions.key != &solana_instructions_sysvar::ID
        || instructions.owner != &sysvar::ID
        || instructions.is_writable
        || instructions.is_signer
    {
        return Err(ResolutionError::Sysvar.into());
    }
    let current = load_current_index_checked(instructions)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let loaded = load_instruction_at_checked(usize::from(current), instructions)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if loaded.program_id != *program_id
        || loaded.data.as_slice() != instruction_data
        || loaded.accounts.len() != accounts.len()
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    for (meta, actual) in loaded.accounts.iter().zip(accounts) {
        if meta.pubkey != *actual.key
            || meta.is_signer != actual.is_signer
            || meta.is_writable != actual.is_writable
        {
            return Err(ResolutionError::ProviderObservation.into());
        }
    }
    let preceding_index = current
        .checked_sub(1)
        .ok_or(ResolutionError::ProviderObservation)?;
    let preceding = load_instruction_at_checked(usize::from(preceding_index), instructions)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if !preceding.accounts.is_empty() || preceding.program_id.to_bytes() != ED25519_PROGRAM_ID_3_0 {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let view = Ed25519InstructionViewV1 {
        program_id: preceding.program_id.to_bytes(),
        ed25519_data: preceding.data.as_slice(),
        preceding_index,
        current_index: current,
        current_data: instruction_data,
    };
    let offset = u16::try_from(message_offset).map_err(|_| ResolutionError::Arithmetic)?;
    let length = u16::try_from(message_len).map_err(|_| ResolutionError::Arithmetic)?;
    let authorization = inspect_preceding_relay_signature_v1(view, offset, length)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    Ok(authorization.signer())
}

fn create_prefunded_pda<'info>(
    payer: &AccountInfo<'info>,
    created: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    minimum_balance: u64,
    space: usize,
    owner: &Pubkey,
    signer: &[&[u8]],
) -> ProgramResult {
    if payer.owner != &system_program::ID
        || created.owner != &system_program::ID
        || created.executable
        || !created
            .try_data_is_empty()
            .map_err(|_| ResolutionError::OutputState)?
    {
        return Err(ResolutionError::OutputState.into());
    }
    let before = created.lamports();
    let top_up = minimum_balance.saturating_sub(before);
    let space_u64 = u64::try_from(space).map_err(|_| ResolutionError::Arithmetic)?;
    if before == 0 {
        invoke_signed(
            &create_account(payer.key, created.key, minimum_balance, space_u64, owner),
            &[payer.clone(), created.clone(), system.clone()],
            &[signer],
        )
        .map_err(|_| ResolutionError::OutputState)?;
    } else {
        if top_up != 0 {
            invoke(
                &transfer(payer.key, created.key, top_up),
                &[payer.clone(), created.clone(), system.clone()],
            )
            .map_err(|_| ResolutionError::OutputState)?;
        }
        invoke_signed(
            &allocate(created.key, space_u64),
            &[created.clone(), system.clone()],
            &[signer],
        )
        .map_err(|_| ResolutionError::OutputState)?;
        invoke_signed(
            &assign(created.key, owner),
            &[created.clone(), system.clone()],
            &[signer],
        )
        .map_err(|_| ResolutionError::OutputState)?;
    }
    if created.owner != owner
        || created.data_len() != space
        || created.lamports()
            != before
                .checked_add(top_up)
                .ok_or(ResolutionError::Arithmetic)?
        || created.lamports() < minimum_balance
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

fn close_to_beneficiary(source: &AccountInfo<'_>, beneficiary: &AccountInfo<'_>) -> ProgramResult {
    let source_balance = source.lamports();
    let after = beneficiary
        .lamports()
        .checked_add(source_balance)
        .ok_or(ResolutionError::Arithmetic)?;
    {
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| ResolutionError::OutputState)?;
        let mut beneficiary_lamports = beneficiary
            .try_borrow_mut_lamports()
            .map_err(|_| ResolutionError::OutputState)?;
        **source_lamports = 0;
        **beneficiary_lamports = after;
    }
    source.resize(0).map_err(|_| ResolutionError::OutputState)?;
    source.assign(&system_program::ID);
    if source.lamports() != 0
        || source.owner != &system_program::ID
        || !source
            .try_data_is_empty()
            .map_err(|_| ResolutionError::OutputState)?
        || beneficiary.lamports() != after
    {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}
