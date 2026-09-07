//! Executable `RelayedMainnetStateV1` observation-record transport.
//!
//! Four permissionless routes create, fill, seal and close one cross-cluster
//! observation record. Everything they enforce lives in
//! `dclutch-source::relay`; this module is the account boundary: it
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

use dclutch_market::capability_manifest::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingLedgerDerivationV2,
    CapabilityManifestV1, ContentId as CapabilityContentId, FundingLedgerStatusV2, FundingLedgerV2,
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2};
use dclutch_product::svm_reader::{
    AuthenticatedProductRuntimeV2, FinalizedRecordFrameV2, ProductRuntimeFrameV2,
    authenticate_product_runtime_v2,
};
use dclutch_product::{ContentId as ProductContentId, ResultDomainV2};
use dclutch_registry::release_set::ExecutionRoleV1;
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_BYTES_V1,
    ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1,
};
use dclutch_source::relay::{
    Error as RelayContractError, MAX_RELAYED_ACCOUNTS_V1, RELAYED_ADAPTER_CONFIG_BYTES,
    RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1, RELAYED_FAMILY_RELEASE_ID_V1,
    RELAYED_RECORD_PDA_DOMAIN_V1, RELAYED_RECORD_TRANSPORT_PROFILE_ID_V1, RELAYER_KEY_SET_BYTES,
    RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1, SOLANA_MAINNET_GENESIS_HASH_V1,
    decode::{RelayedObservableV1, RelayedVenueKindV1},
    frame::{
        CONSUME_RECORD_FRAME_V1, CONSUME_RECORD_NATIVE_VENUE_FRAME_V1,
        ENSEMBLE_FOLD_FRAME_PREFIX_V1, RelayAccountPrivilegeV1, RelayAccountRoleV1,
        RelayFrameKindV1, consume_frame_kind_v1, consume_position_v1, ensemble_fold_tail_v1,
        validate_relay_frame_v1, validate_relay_frame_with_tail_v1,
    },
    instruction::{
        APPEND_OBSERVATION_PREFIX_BYTES, AdvanceRecoveryInstructionV1,
        AppendObservationInstructionV1, CommitDeadlineFailureInstructionV1,
        ConsumeRecordInstructionV1, CreateRecordInstructionV1, EnsembleFoldInstructionV1,
        RELAY_INSTRUCTION_MAGIC, ReclaimMemberSeatInstructionV1, RelayInstructionV1,
        RetireRecordInstructionV1, SEAL_RECORD_PREFIX_BYTES, SealRecordInstructionV1,
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
use dclutch_source::resolution::{
    EnsembleFoldReceiptSeatSeedsV1, EnsembleFragmentSeatSeedsV1, RESOLUTION_CERTIFICATE_BYTES_V2,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V7,
    ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source::{
    ENSEMBLE_FOLD_RECEIPT_PDA_DOMAIN_V1, ENSEMBLE_FOLD_RECEIPT_V1_BYTES,
    ENSEMBLE_FRAGMENT_PDA_DOMAIN_V1, PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1,
    ProviderReleaseV1, RECOVERY_POLICY_BYTES_V2, RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_MATERIAL_V3_BYTES,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_SPEC_BYTES, SOURCE_SPEC_SCHEMA_ID_V1,
    STATISTIC_SPEC_BYTES, STATISTIC_SPEC_SCHEMA_ID_V1, SourceAccessProfile, SourceMaterialV3,
    SourceResolutionStateV2, SourceSpecV1, StatisticSpecV1, WINDOW_SPEC_BYTES,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
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

use crate::market_admission_v1::RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1;
use crate::{
    RecordKind, ResolutionError, authenticate_clock, authenticate_finalized_record,
    authenticate_rent,
    ensemble_v1::{
        AuthenticatedEnsembleSourceV1, ENSEMBLE_MAX_MEMBERS, EnsembleFoldErrorV1,
        EnsembleFoldRequestV1, MemberSeatV1, plan_ensemble_fold_v1,
    },
    funded::{
        AuthenticatedFailureFundingV2, AuthenticatedRecoveryPolicyV1, AuthenticatedWalkSourceV1,
        DeadlineFailureRequestV1, FundedWalkErrorV1, MemberBountyReleaseV1,
        RESOLUTION_FUNDING_LEDGER_BYTES_V2, plan_deadline_failure_v1, process_funded_transition,
    },
    provider_instruction_v3::authenticate_record,
    relay_v1::{
        AuthenticatedRelaySourceRecordsV1, AuthenticatedVenueReleaseV1, RelayJoinErrorV1,
        RelayResolutionRequestV1, plan_relayed_resolution_v1,
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
        RelayInstructionV1::AdvanceRecovery(request) => {
            process_advance_recovery(program_id, accounts, request)
        }
        RelayInstructionV1::EnsembleFold(request) => {
            process_ensemble_fold(program_id, accounts, request)
        }
        RelayInstructionV1::ReclaimMemberSeat(request) => {
            process_reclaim_member_seat(program_id, accounts, request)
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
pub(crate) struct MarketFacts {
    pub(crate) registry_program: Pubkey,
    pub(crate) rent_beneficiary: [u8; 32],
    pub(crate) product_record: [u8; 32],
    pub(crate) capability_manifest: [u8; 32],
}

pub(crate) fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(ResolutionError::AccountFrame.into())
}

/// Which consumption frame this caller presented, read off its own width.
///
/// The two admissible consumption shapes differ by exactly the venue-release
/// pair, so the count is the discriminant -- the same way Core's Found parser
/// reads its three widths. What the count CANNOT say is whether the market's
/// own decoding-rules row agrees; `consume_source_records` decides that against
/// the configuration and refuses as `RelayedVenueKind`.
fn consume_venue_kind(accounts: &[AccountInfo<'_>]) -> Result<RelayedVenueKindV1, ProgramError> {
    if accounts.len() == CONSUME_RECORD_FRAME_V1.len() {
        Ok(RelayedVenueKindV1::LoaderV3)
    } else if accounts.len() == CONSUME_RECORD_NATIVE_VENUE_FRAME_V1.len() {
        Ok(RelayedVenueKindV1::Native)
    } else {
        Err(ResolutionError::AccountFrame.into())
    }
}

/// One canonical consumption position, in the frame this venue kind fills.
///
/// Every consumption index in this file is written once, in the thirty-slot
/// coordinate system `CONSUME_RECORD_FRAME_V1` declares, and moved here. A
/// position the native frame does not have is `AccountFrame` rather than a
/// silent neighbour.
fn consume_slot<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    venue: RelayedVenueKindV1,
    canonical: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    let position = consume_position_v1(venue, canonical).ok_or(ResolutionError::AccountFrame)?;
    account(accounts, position)
}

pub(crate) fn validate_frame(
    kind: RelayFrameKindV1,
    accounts: &[AccountInfo<'_>],
) -> ProgramResult {
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

pub(crate) fn require_system(account: &AccountInfo<'_>) -> ProgramResult {
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
pub(crate) fn authenticate_market(
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
        || !RESOLUTION_LIVE_MARKET_ADMISSIBLE_PRESTATES_V1.admits(state.phase, state.readiness)
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
        .map_err(|_| ResolutionError::ActivationCache)?;
    if activation.owner != &registry_program
        || activation_data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || activation.key
            != &Pubkey::find_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
                &registry_program,
            )
            .0
    {
        return Err(ResolutionError::ActivationCache.into());
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| ResolutionError::ActivationCache)?;
    let selected = activated
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionError::ResolutionRelease)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| ResolutionError::ActivationCache)?
        .to_bytes()
        != release_set
    {
        return Err(ResolutionError::ActivationCache.into());
    }
    if selected.release().program().to_bytes() != program_id.to_bytes() {
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
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        material_id,
        &material_data,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
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
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id,
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    window
        .validate_source(
            dclutch_source::ContentId::new(source_spec_id)
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

    let mut seed_preimage = [0u8; dclutch_source::relay::release::SET_DIGEST_SEED_PREIMAGE_BYTES];
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
    let instructions = account(accounts, 6)?;
    let clock_account = account(accounts, 7)?;
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
    let instructions = account(accounts, 6)?;
    let clock_account = account(accounts, 7)?;
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
    // Census Y3/Q9. The Market is already in this frame and already read; its
    // terminal receipt is the whole authority for whether this record is still
    // live evidence. `terminal_receipt.is_some()` is exactly
    // `phase in {Terminal, Retiring, Retired}` by `CoreState::valid_static`, so
    // this reads the fact rather than re-deriving it from the phase byte.
    let market_has_terminalized = state.terminal_receipt.is_some();
    drop(market_data);

    {
        let mut data = record_account
            .try_borrow_mut_data()
            .map_err(|_| ResolutionError::OutputState)?;
        retire_relayed_observation_in_place_v1(
            &mut data,
            request.generation(),
            created,
            market_has_terminalized,
        )
        .map_err(|error| match error {
            RelayContractError::RecordStillConsumable => ResolutionError::RecordStillConsumable,
            _ => ResolutionError::Transition,
        })?;
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
    let venue = consume_venue_kind(accounts)?;
    validate_frame(consume_frame_kind_v1(venue), accounts)?;
    let worker = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let core = account(accounts, 2)?;
    let activation = account(accounts, 3)?;
    let record_account = account(accounts, 4)?;
    let source_state_account = account(accounts, 5)?;
    let certificate_account = account(accounts, 6)?;
    let clock_account = consume_slot(accounts, venue, 27)?;
    let rent_sysvar = consume_slot(accounts, venue, 28)?;
    let system = consume_slot(accounts, venue, 29)?;
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
        venue,
        request.source_material_id(),
        request.source_spec_id(),
    )?;
    let product_runtime = boxed_product_runtime(
        &market.registry_program,
        ProductContentId::new(market.product_record).map_err(|_| ResolutionError::ProductDomain)?,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: consume_slot(accounts, venue, 21)?,
                staging: consume_slot(accounts, venue, 22)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: consume_slot(accounts, venue, 23)?,
                staging: consume_slot(accounts, venue, 24)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: consume_slot(accounts, venue, 25)?,
                staging: consume_slot(accounts, venue, 26)?,
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

    let domain_data = consume_slot(accounts, venue, 23)?
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
/// 4. the `SourceMaterialV3` record, and the `WindowSpecV1` it names;
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
    process_deadline_failure_coordinates(
        program_id,
        accounts,
        request.generation(),
        request.terminal_sequence(),
    )
}

/// Crank the funded ordered-recovery ladder by exactly one rung.
///
/// This is the route the completion contract's recovery row says does not
/// exist. A market founded with a `RecoveryPolicyV2` could not be terminalized
/// at all: `SourceResolutionStateV2::exhaust_after_primary_deadline` refuses a
/// recovery-bearing material by name -- skipping paid-for legs would take an
/// outcome away from the holders who paid for them -- and the failure commit
/// only fires from `Exhausted`, which nothing could reach. Core welded founding
/// shut rather than let another such market exist.
///
/// What it authenticates, in order, each refusing on its own field:
///
/// 1. the frame -- eighteen positions, three writable, no aliases;
/// 2. the Source state's program custody and its own derived address;
/// 3. the Market, its Core ownership, its derived address, this Program as its
///    Resolution role, and that its resolution policy is the material the state
///    is bound to;
/// 4. the `SourceMaterialV3` record, the `WindowSpecV1` it names, and the
///    `RecoveryPolicyV2` it selects;
/// 5. the `CapabilityManifestV1` the Market names, and the compartment the
///    crank's own rung configures.
///
/// There is no Product graph in that list, and its absence is the transition's
/// shape rather than an economy: a crank selects no outcome, so
/// `ResolutionCertificateV2::validate_terminal_product` refuses to be asked
/// about a `RecoveryAdvanced` or `Exhausted` receipt at all, and the only
/// Product fact the receipt carries is the digest the material already names.
///
/// As with the failure walk, the caller supplies which market and when, and
/// nothing else. Which rung the ladder stands on, which source that rung names,
/// and which compartment pays for leaving it are all read out of records the
/// market finalized before it opened.
#[inline(never)]
fn process_advance_recovery(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: AdvanceRecoveryInstructionV1,
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::AdvanceRecovery, accounts)?;
    let worker = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let core = account(accounts, 2)?;
    let activation = account(accounts, 3)?;
    let source_state_account = account(accounts, 4)?;
    let certificate_account = account(accounts, 5)?;
    let funding_account = account(accounts, 14)?;
    let clock_account = account(accounts, 15)?;
    let rent_sysvar = account(accounts, 16)?;
    let system = account(accounts, 17)?;
    require_system(system)?;
    let rent = authenticate_rent(rent_sysvar)?;
    let clock = authenticate_clock(clock_account)?;
    let generation = request.generation();

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
        generation,
        material_id.to_bytes(),
    )?;
    let walk_source = Box::new(deadline_walk_source(
        &market.registry_program,
        accounts,
        material_id,
    )?);
    let ladder = Box::new(authenticate_recovery_policy_record(
        &market.registry_program,
        accounts,
        walk_source.material,
    )?);

    // Which compartment this crank spends is a function of which rung it takes,
    // and the row has to be selected before the crank runs because one account
    // holds all three of a market's Resolution compartments and each is found
    // by its own pinned configuration. `next_crank_funding_config` is the one
    // author of that answer; `process_funded_transition` derives it again from
    // the crank it actually took, and `plan_funding_release` refuses if the two
    // ever disagreed.
    let selecting_config = source_state
        .next_crank_funding_config(walk_source.material, ladder.policy_id, ladder.policy)
        .map_err(|_| ResolutionError::Transition)?
        .to_bytes();

    let manifest_data = account(accounts, 12)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    authenticate_finalized_record(
        market.registry_program,
        account(accounts, 12)?,
        account(accounts, 13)?,
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
        generation,
        selecting_config,
    )?);

    let outputs = plan_and_encode_funded_transition(
        &DeadlineFailureRequestV1 {
            market: market_account.key.to_bytes(),
            generation,
            terminal_sequence: request.terminal_sequence(),
            certificate_account: certificate_account.key.to_bytes(),
            current_unix_seconds: clock.unix_timestamp,
        },
        &source_state,
        &walk_source,
        &ladder,
        &escrow,
        // No frame in this family carries a member seat, and no route yet
        // writes one: `select_rung` refuses a member capture on `Primary` by
        // name, so a crank cannot be looking at fragments it did not count.
        0,
    )?;

    let worker_lamports_after = worker
        .lamports()
        .checked_add(outputs.encoded.work_paid)
        .ok_or(ResolutionError::Arithmetic)?;
    drop(manifest_data);
    commit_deadline_failure(
        program_id,
        outputs.certificate_kind_seed,
        request.terminal_sequence(),
        DeadlineFailureOutputs {
            source_state: source_state_account,
            certificate: certificate_account,
            funding: funding_account,
            worker,
            system,
        },
        &rent,
        &outputs.encoded,
        worker_lamports_after,
    )
}

/// Fold an ensemble market's fragments into its one terminal.
///
/// This is the physical outer [`crate::ensemble_v1`] was written against: it
/// owns the accounts, the seat derivations and the writes, and the pure fold
/// owns every decision. What it authenticates, in order, each refusing on its
/// own field:
///
/// 1. the Source state's program custody and its own derived address;
/// 2. the Market, its Core ownership, its derived address, this Program as its
///    Resolution role, and that its resolution policy is the material the
///    state is bound to;
/// 3. the `SourceMaterialV3`, the primary `SourceSpecV1` whose provider
///    release is member zero's route, the `WindowSpecV1` that closes the fold's
///    window, the `StatisticSpecV1` that carries the source-to-result shift,
///    and the `RecoveryPolicyV2` whose leading slots are the members;
/// 4. the whole frame -- the fixed prefix and a tail of `2k`, `k` from the
///    authenticated material -- with no alias anywhere in it, so a seat cannot
///    be passed twice to answer twice;
/// 5. each member seat at its own derived address, as this Program's decoded
///    certificate or as a System-owned vacancy and nothing else;
/// 6. the Product Runtime V2 graph, the `CapabilityManifestV1` and the
///    three-row funding ledger.
///
/// The frame is validated after the material rather than before it, and that
/// ordering is forced rather than chosen: the frame's width is `25 + 2k` and
/// `k` is a byte of a finalized record, not a field of the request. Everything
/// read before the frame check is authenticated by custody and derivation --
/// the Source state by its PDA, the Market by Core's, each record by the
/// Registry's -- and no account is written until the whole frame has passed.
#[inline(never)]
fn process_ensemble_fold(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: EnsembleFoldInstructionV1,
) -> ProgramResult {
    let market_account = account(accounts, 1)?;
    let source_state_account = account(accounts, 4)?;
    let generation = request.generation();
    let terminal_sequence = request.terminal_sequence();

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
        account(accounts, 2)?,
        account(accounts, 3)?,
        generation,
        material_id.to_bytes(),
    )?;
    let source = boxed_ensemble_fold_source(&market.registry_program, accounts, material_id)?;
    let members = source.material.ensemble().members();
    let tail_len = usize::from(members)
        .checked_mul(2)
        .ok_or(ResolutionError::Arithmetic)?;
    validate_frame_with_tail(
        RelayFrameKindV1::EnsembleFold,
        accounts,
        tail_len,
        |index| ensemble_fold_tail_v1(members, index),
    )?;

    let worker = account(accounts, 0)?;
    let certificate_account = account(accounts, 5)?;
    let receipt_account = account(accounts, 6)?;
    let funding_account = account(accounts, 25)?;
    let clock = authenticate_clock(account(accounts, 26)?)?;
    let rent = authenticate_rent(account(accounts, 27)?)?;
    let system = account(accounts, 28)?;
    require_system(system)?;

    let seats = boxed_member_seats(
        program_id,
        accounts,
        source_state_account,
        members,
        terminal_sequence,
    )?;
    let product_runtime = boxed_product_runtime(
        &market.registry_program,
        ProductContentId::new(market.product_record).map_err(|_| ResolutionError::ProductDomain)?,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: account(accounts, 17)?,
                staging: account(accounts, 18)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: account(accounts, 19)?,
                staging: account(accounts, 20)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: account(accounts, 21)?,
                staging: account(accounts, 22)?,
            },
        },
    )?;

    let manifest_data = account(accounts, 23)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    authenticate_finalized_record(
        market.registry_program,
        account(accounts, 23)?,
        account(accounts, 24)?,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market.capability_manifest,
        &manifest_data,
        RecordKind::CapabilityManifest,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| ResolutionError::Funding)?;
    let manifest_id = CapabilityContentId::new(market.capability_manifest)
        .map_err(|_| ResolutionError::Funding)?;
    // The fold spends no compartment of its own, and the row named here is the
    // ledger's explicit-failure row for the same reason the failure walk names
    // it: to authenticate the ledger's derivation, its native custody and its
    // three-row shape. Which row each member's bounty leaves is decided inside
    // the plan, by that member's own attempt configuration and never by a
    // position.
    let escrow = Box::new(authenticate_failure_funding(
        program_id,
        funding_account,
        market_account,
        manifest_id,
        manifest,
        generation,
        material_id.to_bytes(),
    )?);

    let domain_data = account(accounts, 19)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let result_domain =
        ResultDomainV2::decode(&domain_data).map_err(|_| ResolutionError::ProductDomain)?;

    let encoded = plan_and_encode_ensemble_fold(
        &EnsembleFoldRequestV1 {
            market: market_account.key.to_bytes(),
            generation,
            terminal_sequence,
            certificate_account: certificate_account.key.to_bytes(),
            current_unix_seconds: clock.unix_timestamp,
        },
        &source_state,
        &source,
        &product_runtime,
        result_domain,
        &seats[..usize::from(members)],
        &escrow,
    )?;

    drop(domain_data);
    drop(manifest_data);
    let captors_from = ENSEMBLE_FOLD_FRAME_PREFIX_V1
        .len()
        .checked_add(usize::from(members))
        .ok_or(ResolutionError::Arithmetic)?;
    commit_ensemble_fold(
        program_id,
        terminal_sequence,
        EnsembleFoldOutputs {
            source_state: source_state_account,
            certificate: certificate_account,
            receipt: receipt_account,
            funding: funding_account,
            worker,
            system,
        },
        accounts
            .get(captors_from..)
            .ok_or(ResolutionError::AccountFrame)?,
        &rent,
        &encoded,
    )
}

/// Validate a fixed prefix and a tail whose width one authenticated record set.
fn validate_frame_with_tail(
    kind: RelayFrameKindV1,
    accounts: &[AccountInfo<'_>],
    tail_len: usize,
    tail_role: impl Fn(usize) -> Option<RelayAccountRoleV1>,
) -> ProgramResult {
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
    validate_relay_frame_with_tail_v1(kind, &observed, tail_len, tail_role)
        .map_err(|_| ResolutionError::AccountFrame)?;
    Ok(())
}

/// Authenticate the five Source records the fold reads.
///
/// The failure walk needs the material and the window; the crank needs the
/// policy as well. The fold needs two more and each for one value: the primary
/// `SourceSpecV1` for the provider release that is member zero's route, and
/// the `StatisticSpecV1` for the decimal shift the median reaches the
/// selector on. Neither is a caller's word; both hang off the material by
/// content identity.
#[inline(never)]
fn boxed_ensemble_fold_source(
    registry: &Pubkey,
    accounts: &[AccountInfo<'_>],
    material_id: dclutch_source::ContentId,
) -> Result<Box<AuthenticatedEnsembleSourceV1>, ProgramError> {
    let material_data = account(accounts, 7)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 7)?,
        account(accounts, 8)?,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        material_id.to_bytes(),
        &material_data,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(material_data);
    let source_spec_id = material.primary_source_spec();
    let window_spec_id = material.window_spec();
    let statistic_spec_id = material.statistic_spec();
    // The members live in the policy's leading slots, so a material that
    // declares an ensemble and selects no policy has nowhere to hold them.
    let policy_id = material
        .recovery_policy()
        .ok_or(ResolutionError::SourceMaterial)?;

    let spec_data = account(accounts, 9)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 9)?,
        account(accounts, 10)?,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id.to_bytes(),
        &spec_data,
        SOURCE_SPEC_BYTES,
    )?;
    let spec = SourceSpecV1::decode(&spec_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let primary_provider_release_id = spec.provider_release_id();
    drop(spec_data);

    let window_data = account(accounts, 11)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 11)?,
        account(accounts, 12)?,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id.to_bytes(),
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(window_data);

    let statistic_data = account(accounts, 13)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 13)?,
        account(accounts, 14)?,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        statistic_spec_id.to_bytes(),
        &statistic_data,
        STATISTIC_SPEC_BYTES,
    )?;
    let statistic =
        StatisticSpecV1::decode(&statistic_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let source_scale_exponent = statistic.source_scale_exponent();
    drop(statistic_data);

    let policy_data = account(accounts, 15)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 15)?,
        account(accounts, 16)?,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        policy_id.to_bytes(),
        &policy_data,
        RECOVERY_POLICY_BYTES_V2,
    )?;
    let policy =
        RecoveryPolicyV2::decode(&policy_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(policy_data);

    Ok(Box::new(AuthenticatedEnsembleSourceV1 {
        material_id,
        material,
        window_spec_id,
        window,
        policy_id,
        policy,
        primary_provider_release_id,
        source_scale_exponent,
    }))
}

/// Read every declared member's seat, at its own derived address.
///
/// A seat is this Program's decoded certificate or a System-owned vacancy, and
/// there is no third shape: an account at a member's seat address holding
/// anything else is a hostile rather than an absence, and the whole route
/// refuses on it. The address is derived from the Source state, the member
/// byte and the terminal sequence, so a fragment can neither stand at the
/// market's terminal seat nor stand in for another member.
#[inline(never)]
fn boxed_member_seats(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    source_state: &AccountInfo<'_>,
    members: u8,
    terminal_sequence: u64,
) -> Result<Box<[MemberSeatV1; ENSEMBLE_MAX_MEMBERS]>, ProgramError> {
    let mut seats = Box::new([MemberSeatV1::Vacant; ENSEMBLE_MAX_MEMBERS]);
    let mut member = 0_u8;
    while member < members {
        let index = ENSEMBLE_FOLD_FRAME_PREFIX_V1
            .len()
            .checked_add(usize::from(member))
            .ok_or(ResolutionError::Arithmetic)?;
        let seat = account(accounts, index)?;
        let seeds = EnsembleFragmentSeatSeedsV1::new(
            source_state.key.to_bytes(),
            member,
            terminal_sequence,
        );
        if seat.key != &Pubkey::find_program_address(&seeds.seeds(), program_id).0
            || seat.executable
        {
            return Err(ResolutionError::EnsembleMember.into());
        }
        if seat.owner == program_id {
            if seat.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2 {
                return Err(ResolutionError::EnsembleMember.into());
            }
            let data = seat
                .try_borrow_data()
                .map_err(|_| ResolutionError::EnsembleMember)?;
            let certificate = ResolutionCertificateV2::decode(&data)
                .map_err(|_| ResolutionError::EnsembleMember)?;
            drop(data);
            *seats
                .get_mut(usize::from(member))
                .ok_or(ResolutionError::EnsembleMember)? = MemberSeatV1::Written(certificate);
        } else if seat.owner != &system_program::ID || seat.data_len() != 0 {
            return Err(ResolutionError::EnsembleMember.into());
        }
        member = member.checked_add(1).ok_or(ResolutionError::Arithmetic)?;
    }
    Ok(seats)
}

/// Everything one fold writes, and who is owed what for it.
struct EncodedEnsembleFoldV1 {
    /// `SourceResolutionStateV2` after the primary transition on the median.
    source: [u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    /// The market's own terminal `ResolutionSuccess`, already schema-validated.
    certificate: [u8; RESOLUTION_CERTIFICATE_BYTES_V2],
    /// The fold's durable receipt, already schema-validated.
    receipt: [u8; ENSEMBLE_FOLD_RECEIPT_V1_BYTES],
    /// The complete `FundingLedgerV2` after every member's bounty release.
    funding: [u8; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
    /// Digest of the full ledger prestate the plan authenticated.
    funding_prestate_digest: [u8; 32],
    /// Exact funding-account lamports after every release.
    funding_lamports_after: u64,
    /// Which member is paid what, in member order.
    bounties: [Option<MemberBountyReleaseV1>; ENSEMBLE_MAX_MEMBERS],
    /// The captor each consumed member's fragment named, in member order.
    captors: [Option<[u8; 32]>; ENSEMBLE_MAX_MEMBERS],
}

/// Plan the fold and encode every byte it writes, on a frame of its own.
///
/// The same reason the failure walk and the crank do it here: the plan carries
/// a Source state, a certificate, a receipt and a complete three-row ledger
/// poststate by value, and encoding them produces another full wire image
/// beside them, which does not fit in the caller's four-kilobyte frame
/// alongside the authenticated Market, Source graph, seats and escrow.
///
/// The encoding happens here rather than at the commit, and that ordering is
/// the point: `to_bytes` runs the Lean-owned schema's own `validate_shape` for
/// the certificate and for the receipt, so a shape either schema would refuse
/// never reaches an account and no lamport has moved when it is refused.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn plan_and_encode_ensemble_fold(
    request: &EnsembleFoldRequestV1,
    source_state: &SourceResolutionStateV2,
    source: &AuthenticatedEnsembleSourceV1,
    product_runtime: &AuthenticatedProductRuntimeV2,
    result_domain: ResultDomainV2<'_>,
    seats: &[MemberSeatV1],
    escrow: &AuthenticatedFailureFundingV2<'_>,
) -> Result<Box<EncodedEnsembleFoldV1>, ProgramError> {
    let plan = plan_ensemble_fold_v1(
        request,
        source_state,
        source,
        product_runtime,
        result_domain,
        seats,
        escrow,
    )
    .map_err(map_ensemble_fold_error)?;
    let mut encoded = Box::new(EncodedEnsembleFoldV1 {
        source: [0; SOURCE_RESOLUTION_STATE_BYTES_V2],
        certificate: [0; RESOLUTION_CERTIFICATE_BYTES_V2],
        receipt: [0; ENSEMBLE_FOLD_RECEIPT_V1_BYTES],
        funding: [0; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
        funding_prestate_digest: hash(&escrow.ledger_bytes).to_bytes(),
        funding_lamports_after: plan.funding_lamports_after,
        bounties: plan.bounties,
        captors: plan.captors,
    });
    encoded.source = plan.next_source.to_bytes();
    encoded.certificate = plan
        .certificate
        .to_bytes()
        .map_err(|_| ResolutionError::Transition)?;
    encoded.receipt = plan
        .receipt
        .to_bytes()
        .map_err(|_| ResolutionError::Transition)?;
    encoded.funding = plan.next_funding;
    Ok(encoded)
}

/// The pure fold's refusals, each naming its own conjunct.
pub(crate) const fn map_ensemble_fold_error(error: EnsembleFoldErrorV1) -> ResolutionError {
    match error {
        EnsembleFoldErrorV1::Request => ResolutionError::Instruction,
        EnsembleFoldErrorV1::Source => ResolutionError::SourceMaterial,
        EnsembleFoldErrorV1::Product => ResolutionError::ProductDomain,
        EnsembleFoldErrorV1::Fragment => ResolutionError::EnsembleMember,
        EnsembleFoldErrorV1::Quorum => ResolutionError::EnsembleQuorum,
        EnsembleFoldErrorV1::Transition => ResolutionError::Transition,
        EnsembleFoldErrorV1::Funding => ResolutionError::Funding,
        EnsembleFoldErrorV1::Arithmetic => ResolutionError::Arithmetic,
    }
}

/// The accounts one fold writes, named rather than indexed.
struct EnsembleFoldOutputs<'a, 'info> {
    source_state: &'a AccountInfo<'info>,
    certificate: &'a AccountInfo<'info>,
    receipt: &'a AccountInfo<'info>,
    funding: &'a AccountInfo<'info>,
    worker: &'a AccountInfo<'info>,
    system: &'a AccountInfo<'info>,
}

/// Commit the terminal, the receipt, the debited escrow and every captor's pay.
///
/// All of them move or none do, which is what makes a member's bounty a
/// payment for a capture rather than a claim about one: the receipt that says
/// which fragments were consumed is written by the same transaction that pays
/// the captors those fragments named.
#[inline(never)]
fn commit_ensemble_fold(
    program_id: &Pubkey,
    terminal_sequence: u64,
    outputs: EnsembleFoldOutputs<'_, '_>,
    captors: &[AccountInfo<'_>],
    rent: &Rent,
    encoded: &EncodedEnsembleFoldV1,
) -> ProgramResult {
    initialize_certificate_at_kind(
        program_id,
        // A fold selects an ordinary outcome, so its terminal lives at the
        // success kind's own address; the failure walk's receipt for the same
        // Source state at the same sequence is a different account and neither
        // can overwrite the other.
        ResolutionCertificateKindV2::ResolutionSuccess.kind_seed(),
        terminal_sequence,
        outputs.source_state,
        outputs.certificate,
        outputs.system,
        rent,
    )?;
    initialize_fold_receipt_seat(
        program_id,
        terminal_sequence,
        outputs.source_state,
        outputs.receipt,
        outputs.worker,
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
    let mut receipt_output = outputs
        .receipt
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut funding_output = outputs
        .funding
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if state_output.len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || certificate_output.len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || receipt_output.len() != ENSEMBLE_FOLD_RECEIPT_V1_BYTES
        || funding_output.len() != RESOLUTION_FUNDING_LEDGER_BYTES_V2
        || certificate_output.iter().any(|byte| *byte != 0)
        || receipt_output.iter().any(|byte| *byte != 0)
        || hash(&funding_output).to_bytes() != encoded.funding_prestate_digest
    {
        return Err(ResolutionError::OutputState.into());
    }
    state_output.copy_from_slice(&encoded.source);
    certificate_output.copy_from_slice(&encoded.certificate);
    receipt_output.copy_from_slice(&encoded.receipt);
    funding_output.copy_from_slice(&encoded.funding);
    if funding_output.as_ref() != encoded.funding {
        return Err(ResolutionError::OutputState.into());
    }
    drop(state_output);
    drop(certificate_output);
    drop(receipt_output);
    drop(funding_output);

    // Each consumed member's bounty goes to the captor its own fragment named,
    // at the frame position its member order fixes. The total is then checked
    // against the ledger poststate the plan computed, so the lamports that
    // leave the escrow and the lamports the ledger says left it are one
    // number rather than two.
    let mut paid = 0_u64;
    for (member, release) in encoded.bounties.iter().enumerate() {
        let Some(release) = release else { continue };
        let named = encoded
            .captors
            .get(member)
            .copied()
            .flatten()
            .ok_or(ResolutionError::EnsembleMember)?;
        let captor = captors.get(member).ok_or(ResolutionError::AccountFrame)?;
        if captor.key.to_bytes() != named {
            return Err(ResolutionError::EnsembleMember.into());
        }
        let after = captor
            .lamports()
            .checked_add(release.work_paid)
            .ok_or(ResolutionError::Arithmetic)?;
        let mut captor_lamports = captor
            .try_borrow_mut_lamports()
            .map_err(|_| ResolutionError::OutputState)?;
        **captor_lamports = after;
        paid = paid
            .checked_add(release.work_paid)
            .ok_or(ResolutionError::Arithmetic)?;
    }
    let mut funding_lamports = outputs
        .funding
        .try_borrow_mut_lamports()
        .map_err(|_| ResolutionError::OutputState)?;
    if (**funding_lamports).checked_sub(paid) != Some(encoded.funding_lamports_after) {
        return Err(ResolutionError::OutputState.into());
    }
    **funding_lamports = encoded.funding_lamports_after;
    Ok(())
}

/// Create the fold's receipt seat at its own derived address.
///
/// Unlike the terminal certificate, no founding prepays this account: the
/// receipt exists because a fold happened, so the worker that folds pays its
/// rent. The seat is derived from the Source state and the terminal sequence,
/// so one fold has one receipt and a second fold at the same sequence finds a
/// written account rather than an empty one.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn initialize_fold_receipt_seat<'info>(
    program_id: &Pubkey,
    terminal_sequence: u64,
    source_state: &AccountInfo<'info>,
    receipt: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
) -> ProgramResult {
    let state_key = source_state.key.to_bytes();
    let sequence_seed = terminal_sequence.to_le_bytes();
    let seeds = EnsembleFoldReceiptSeatSeedsV1::new(state_key, terminal_sequence);
    let (expected, bump) = Pubkey::find_program_address(&seeds.seeds(), program_id);
    if receipt.key != &expected {
        return Err(ResolutionError::OutputState.into());
    }
    let bump_seed = [bump];
    let signer: [&[u8]; 4] = [
        ENSEMBLE_FOLD_RECEIPT_PDA_DOMAIN_V1,
        &state_key,
        &sequence_seed,
        &bump_seed,
    ];
    create_prefunded_pda(
        payer,
        receipt,
        system,
        rent.minimum_balance(ENSEMBLE_FOLD_RECEIPT_V1_BYTES),
        ENSEMBLE_FOLD_RECEIPT_V1_BYTES,
        program_id,
        &signer,
    )
}

/// Return a never-written member seat's prepaid rent after the terminal.
///
/// A member seat is prepaid at founding and written only if that member
/// answers inside the window. Once the market has its terminal, a seat still
/// System-owned and empty is rent nobody will ever use, and it goes back to
/// the Source state's own `rent_beneficiary` -- one beneficiary for every seat
/// of one Source, named by the state rather than by the caller.
///
/// The member byte is checked against the material's `k` BEFORE the seat's
/// address is derived, so this route cannot be used to learn where a seat the
/// material declares no member for would live.
#[inline(never)]
fn process_reclaim_member_seat(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: ReclaimMemberSeatInstructionV1,
) -> ProgramResult {
    validate_frame(RelayFrameKindV1::ReclaimMemberSeat, accounts)?;
    let market_account = account(accounts, 1)?;
    let source_state_account = account(accounts, 4)?;
    let seat = account(accounts, 7)?;
    let beneficiary = account(accounts, 8)?;
    let system = account(accounts, 9)?;
    require_system(system)?;
    let generation = request.generation();
    let terminal_sequence = request.terminal_sequence();

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
        account(accounts, 2)?,
        account(accounts, 3)?,
        generation,
        material_id.to_bytes(),
    )?;

    let material_data = account(accounts, 5)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        &market.registry_program,
        account(accounts, 5)?,
        account(accounts, 6)?,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        material_id.to_bytes(),
        &material_data,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(material_data);
    if !material.ensemble().declares_member(request.member()) {
        return Err(ResolutionError::EnsembleMember.into());
    }

    // A seat is only dead rent once the market has its own terminal, and only
    // at the sequence that terminal was written under: reclaiming a seat of a
    // sequence still open would take a member's own place to answer away.
    // `terminal_projection` is the contract's one author of "this state is
    // terminal", so this route asks it rather than restating the phase set.
    if source_state
        .terminal_projection()
        .map_err(|_| ResolutionError::Transition)?
        .terminal_sequence()
        != terminal_sequence
    {
        return Err(ResolutionError::Transition.into());
    }

    let state_key = source_state_account.key.to_bytes();
    let member_seed = [request.member()];
    let sequence_seed = terminal_sequence.to_le_bytes();
    let seeds = EnsembleFragmentSeatSeedsV1::new(state_key, request.member(), terminal_sequence);
    let (expected, bump) = Pubkey::find_program_address(&seeds.seeds(), program_id);
    if seat.key != &expected {
        return Err(ResolutionError::EnsembleMember.into());
    }
    // A written seat is a fragment the fold consumed or refused; either way it
    // is evidence, not rent, and this route does not close it.
    if seat.owner != &system_program::ID
        || seat.executable
        || seat.data_len() != 0
        || seat.lamports() == 0
        || beneficiary.key.to_bytes() != source_state.rent_beneficiary()
    {
        return Err(ResolutionError::OutputState.into());
    }

    let bump_seed = [bump];
    let signer: [&[u8]; 5] = [
        ENSEMBLE_FRAGMENT_PDA_DOMAIN_V1,
        &state_key,
        &member_seed,
        &sequence_seed,
        &bump_seed,
    ];
    let claimed = seat.lamports();
    let beneficiary_after = beneficiary
        .lamports()
        .checked_add(claimed)
        .ok_or(ResolutionError::Arithmetic)?;
    invoke_signed(
        &transfer(seat.key, beneficiary.key, claimed),
        &[seat.clone(), beneficiary.clone(), system.clone()],
        &[&signer],
    )
    .map_err(|_| ResolutionError::OutputState)?;
    if seat.lamports() != 0 || beneficiary.lamports() != beneficiary_after {
        return Err(ResolutionError::OutputState.into());
    }
    Ok(())
}

/// Everything one crank writes, plus the seed that decides where it writes it.
struct EncodedFundedTransitionV1 {
    encoded: EncodedDeadlineFailureV1,
    certificate_kind_seed: u8,
}

/// Plan the crank and encode every byte it writes, on a frame of its own.
///
/// The same reason the failure walk does it here: an authenticated
/// `RecoveryPolicyV2` is roughly half a kilobyte, the plan carries a Source
/// state, a certificate and a complete three-row ledger poststate by value, and
/// encoding them produces another full wire image beside them. The SBF frame is
/// four kilobytes and the caller already holds the authenticated Market, Source
/// graph, ladder and escrow.
///
/// The encoding happens here rather than at the commit, and that ordering is
/// the point: `ResolutionCertificateV2::to_bytes` runs `validate_shape`, which
/// is where the Lean-owned schema refuses a liveness receipt carrying a zero
/// `work_paid`, a nonzero selector or any provider evidence. A receipt the
/// schema would refuse therefore never reaches an account, and no lamport has
/// moved when it is refused.
#[inline(never)]
fn plan_and_encode_funded_transition(
    request: &DeadlineFailureRequestV1,
    source_state: &SourceResolutionStateV2,
    walk_source: &AuthenticatedWalkSourceV1,
    ladder: &AuthenticatedRecoveryPolicyV1,
    escrow: &AuthenticatedFailureFundingV2<'_>,
    observed_fragments: u8,
) -> Result<Box<EncodedFundedTransitionV1>, ProgramError> {
    let plan = process_funded_transition(
        request,
        source_state,
        walk_source,
        ladder,
        escrow,
        observed_fragments,
    )
    .map_err(map_funded_walk_error)?;
    let mut outputs = Box::new(EncodedFundedTransitionV1 {
        encoded: EncodedDeadlineFailureV1 {
            source: [0; SOURCE_RESOLUTION_STATE_BYTES_V2],
            certificate: [0; RESOLUTION_CERTIFICATE_BYTES_V2],
            funding: [0; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
            funding_prestate_digest: hash(&escrow.ledger_bytes).to_bytes(),
            work_paid: plan.work_paid,
            funding_lamports_after: plan.funding_lamports_after,
        },
        certificate_kind_seed: plan.certificate_kind.kind_seed(),
    });
    outputs.encoded.source = plan.next_source.to_bytes();
    outputs.encoded.certificate = plan
        .certificate
        .to_bytes()
        .map_err(|_| ResolutionError::Transition)?;
    outputs.encoded.funding = plan.next_funding;
    Ok(outputs)
}

/// Authenticate the `RecoveryPolicyV2` the material selects.
///
/// A material that selects none has no ladder and is refused here rather than
/// deeper in: the primary exhaustion is that market's terminal and this route
/// must not become a second way to reach it.
#[inline(never)]
fn authenticate_recovery_policy_record(
    registry: &Pubkey,
    accounts: &[AccountInfo<'_>],
    material: SourceMaterialV3,
) -> Result<AuthenticatedRecoveryPolicyV1, ProgramError> {
    let policy_id = material
        .recovery_policy()
        .ok_or(ResolutionError::SourceMaterial)?;
    let data = account(accounts, 10)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 10)?,
        account(accounts, 11)?,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        policy_id.to_bytes(),
        &data,
        RECOVERY_POLICY_BYTES_V2,
    )?;
    let policy = RecoveryPolicyV2::decode(&data).map_err(|_| ResolutionError::SourceMaterial)?;
    Ok(AuthenticatedRecoveryPolicyV1 { policy_id, policy })
}

/// Execute the existing funded primary-deadline walk after a transport-specific
/// caller has authenticated its own additional liveness boundary.
///
/// The account slice remains the exact 22-account canonical failure frame; the
/// sponsored-push family uses this only after proving its canonical head PDA is
/// vacant. Keeping the funding and certificate transition here preserves one
/// semantic owner for the failure payout.
#[inline(never)]
pub(crate) fn process_deadline_failure_coordinates(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    terminal_sequence: u64,
) -> ProgramResult {
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
        generation,
        material_id.to_bytes(),
    )?;
    let walk_source = Box::new(deadline_walk_source(
        &market.registry_program,
        accounts,
        material_id,
    )?);
    let product_runtime = boxed_product_runtime(
        &market.registry_program,
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
        generation,
        material_id.to_bytes(),
    )?);

    let domain_data = account(accounts, 12)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let result_domain =
        ResultDomainV2::decode(&domain_data).map_err(|_| ResolutionError::ProductDomain)?;

    let outputs = plan_and_encode_deadline_failure(
        &DeadlineFailureRequestV1 {
            market: market_account.key.to_bytes(),
            generation,
            terminal_sequence,
            certificate_account: certificate_account.key.to_bytes(),
            current_unix_seconds: clock.unix_timestamp,
        },
        &source_state,
        &walk_source,
        &product_runtime,
        result_domain,
        &escrow,
        // The same count, on the same reasoning as the crank's above.
        0,
    )?;

    let worker_lamports_after = worker
        .lamports()
        .checked_add(outputs.work_paid)
        .ok_or(ResolutionError::Arithmetic)?;
    drop(domain_data);
    drop(manifest_data);
    commit_deadline_failure(
        program_id,
        // The kind is a PDA *seed*, so the failure a deadline walk writes and
        // the success an observation writes live at different addresses for one
        // Source state at one sequence and neither can overwrite the other. It
        // is read from the codec rather than written again here, because a
        // second copy of it would be a second author of where a certificate
        // lives.
        ResolutionCertificateKindV2::ResolutionFailure.kind_seed(),
        terminal_sequence,
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
/// `ResolutionCertificateV2` and a complete three-row `FundingLedgerV2`
/// poststate by value, and encoding them produces another full wire image
/// beside them. The SBF frame is four kilobytes and the arm already holds the
/// authenticated Market, Source graph and escrow, so the two together
/// overflowed it: `cargo build-sbf` reported nine
/// stack-frame-overwrite diagnostics against `process_commit_deadline_failure`
/// and exited zero anyway. Boxing the result is not enough on its own, because
/// the plan is still returned *through* the caller's frame; the planning and
/// the encoding have to happen somewhere else entirely.
struct EncodedDeadlineFailureV1 {
    /// `SourceResolutionStateV2` after `Primary → Exhausted → FailureCommitted`.
    source: [u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    /// The terminal `ResolutionFailure` certificate, already schema-validated.
    certificate: [u8; RESOLUTION_CERTIFICATE_BYTES_V2],
    /// The complete `FundingLedgerV2` after the Failure-row bounty debit.
    funding: [u8; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
    /// Digest of the full ledger prestate authenticated by the plan.
    funding_prestate_digest: [u8; 32],
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
    escrow: &AuthenticatedFailureFundingV2<'_>,
    observed_fragments: u8,
) -> Result<Box<EncodedDeadlineFailureV1>, ProgramError> {
    let plan = plan_deadline_failure_v1(
        request,
        source_state,
        walk_source,
        product_runtime,
        result_domain,
        escrow,
        observed_fragments,
    )
    .map_err(map_funded_walk_error)?;
    let mut encoded = Box::new(EncodedDeadlineFailureV1 {
        source: [0; SOURCE_RESOLUTION_STATE_BYTES_V2],
        certificate: [0; RESOLUTION_CERTIFICATE_BYTES_V2],
        funding: [0; RESOLUTION_FUNDING_LEDGER_BYTES_V2],
        funding_prestate_digest: hash(&escrow.ledger_bytes).to_bytes(),
        work_paid: plan.work_paid,
        funding_lamports_after: plan.funding_lamports_after,
    });
    encoded.source = plan.next_source.to_bytes();
    encoded.certificate = plan
        .certificate
        .to_bytes()
        .map_err(|_| ResolutionError::Transition)?;
    encoded.funding = plan.next_funding;
    Ok(encoded)
}

pub(crate) const fn map_funded_walk_error(error: FundedWalkErrorV1) -> ResolutionError {
    match error {
        FundedWalkErrorV1::Request => ResolutionError::Instruction,
        FundedWalkErrorV1::Source => ResolutionError::SourceMaterial,
        FundedWalkErrorV1::Product => ResolutionError::ProductDomain,
        FundedWalkErrorV1::Transition => ResolutionError::Transition,
        FundedWalkErrorV1::Funding => ResolutionError::Funding,
        FundedWalkErrorV1::QuorumMet => ResolutionError::EnsembleQuorumMet,
    }
}

/// Authenticate exactly the two Source records the walk reads.
///
/// A consumption walks material → spec → provider release → configuration →
/// window → venue, because it has to interpret bytes a provider signed. This
/// walk interprets nothing: it needs the material (for the Product record) and
/// the window (for the deadline), and naming any of the others would make the
/// route depend on the party that has stopped answering.
///
/// The recovery crank reads the same two records and one more, which is why it
/// shares this function rather than growing a second Source walk beside it.
#[inline(never)]
fn deadline_walk_source(
    registry: &Pubkey,
    accounts: &[AccountInfo<'_>],
    material_id: dclutch_source::ContentId,
) -> Result<AuthenticatedWalkSourceV1, ProgramError> {
    let material_data = account(accounts, 6)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 6)?,
        account(accounts, 7)?,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        material_id.to_bytes(),
        &material_data,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let window_spec_id = material.window_spec();
    drop(material_data);

    let window_data = account(accounts, 8)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, 8)?,
        account(accounts, 9)?,
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

/// Authenticate the subset ledger and select its explicit-failure row.
///
/// The ledger PDA binds this controller, Market, generation, manifest and exact
/// three-row mask. The Failure row is then selected by its V6 release and this
/// Market's Source-material configuration; it is never a caller-supplied index.
#[inline(never)]
fn authenticate_failure_funding<'a>(
    program_id: &Pubkey,
    account_info: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'a>,
    generation: u64,
    selecting_config: [u8; 32],
) -> Result<AuthenticatedFailureFundingV2<'a>, ProgramError> {
    if account_info.owner != program_id
        || account_info.executable
        || account_info.data_len() != RESOLUTION_FUNDING_LEDGER_BYTES_V2
    {
        return Err(ResolutionError::Funding.into());
    }
    let data = account_info
        .try_borrow_data()
        .map_err(|_| ResolutionError::Funding)?;
    let ledger = FundingLedgerV2::decode(&data).map_err(|_| ResolutionError::Funding)?;
    let authenticated = ledger
        .authenticate(manifest_id, manifest)
        .map_err(|_| ResolutionError::Funding)?;
    if ledger.slot_count() != 3 {
        return Err(ResolutionError::Funding.into());
    }
    let mut failure_entry_index = None;
    let mut entry_index = 0_u16;
    while entry_index < manifest.entry_count() {
        if ledger.selected_mask() & (1_u16 << u32::from(entry_index)) != 0 {
            let entry = manifest
                .entry(entry_index)
                .map_err(|_| ResolutionError::Funding)?;
            if entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7
                || authenticated
                    .slot(entry_index)
                    .map_err(|_| ResolutionError::Funding)?
                    .status()
                    != FundingLedgerStatusV2::Active
            {
                return Err(ResolutionError::Funding.into());
            }
            if entry.config_id().to_bytes() == selecting_config {
                if failure_entry_index.replace(entry_index).is_some() {
                    return Err(ResolutionError::Funding.into());
                }
            }
        }
        entry_index = entry_index
            .checked_add(1)
            .ok_or(ResolutionError::Arithmetic)?;
    }
    let failure_entry_index = failure_entry_index.ok_or(ResolutionError::Funding)?;
    // The rent the failure ledger RECORDS, never the sysvar of the moment
    // (decision 0030). This figure is the custody conjunct and the refund the
    // relay plans, and the ledger was funded when the market was founded.
    let exact_ledger_rent_lamports = authenticated
        .funded_rent_minimum(RESOLUTION_FUNDING_LEDGER_BYTES_V2)
        .map_err(crate::funded_rent_refusal)?;
    authenticated
        .validate_native_custody(account_info.lamports(), exact_ledger_rent_lamports, false)
        .map_err(crate::funded_rent_refusal)?;
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        program_id.to_bytes(),
        market.key.to_bytes(),
        generation,
        manifest_id,
        ledger,
    )
    .map_err(|_| ResolutionError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), program_id).0
        != *account_info.key
    {
        return Err(ResolutionError::Funding.into());
    }
    let ledger_bytes: [u8; RESOLUTION_FUNDING_LEDGER_BYTES_V2] = data
        .as_ref()
        .try_into()
        .map_err(|_| ResolutionError::Funding)?;
    drop(data);
    Ok(AuthenticatedFailureFundingV2 {
        manifest_id,
        manifest,
        entry_index: failure_entry_index,
        ledger_bytes,
        exact_ledger_rent_lamports,
        ledger_account_lamports: account_info.lamports(),
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
    certificate_kind_seed: u8,
    terminal_sequence: u64,
    outputs: DeadlineFailureOutputs<'_, '_>,
    rent: &Rent,
    encoded: &EncodedDeadlineFailureV1,
    worker_lamports_after: u64,
) -> ProgramResult {
    initialize_certificate_at_kind(
        program_id,
        certificate_kind_seed,
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
        || funding_output.len() != RESOLUTION_FUNDING_LEDGER_BYTES_V2
        || certificate_output.iter().any(|byte| *byte != 0)
        || hash(&funding_output).to_bytes() != encoded.funding_prestate_digest
    {
        return Err(ResolutionError::OutputState.into());
    }
    state_output.copy_from_slice(&encoded.source);
    certificate_output.copy_from_slice(&encoded.certificate);
    funding_output.copy_from_slice(&encoded.funding);
    if funding_output.as_ref() != encoded.funding {
        return Err(ResolutionError::OutputState.into());
    }

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

const fn map_relay_join_error(error: RelayJoinErrorV1) -> ResolutionError {
    match error {
        RelayJoinErrorV1::Request => ResolutionError::Instruction,
        RelayJoinErrorV1::Source => ResolutionError::SourceMaterial,
        RelayJoinErrorV1::Product => ResolutionError::ProductDomain,
        RelayJoinErrorV1::Record => ResolutionError::RelayedRecord,
        RelayJoinErrorV1::Observation => ResolutionError::ProviderObservation,
        RelayJoinErrorV1::Window => ResolutionError::RelayedWindow,
        // The same code the Direct route publishes for the same disagreement.
        // Two provider families, one accusation: this market's own records do
        // not agree about the factor between its two units.
        RelayJoinErrorV1::Scale => ResolutionError::ProviderScale,
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
pub(crate) fn boxed_product_runtime(
    registry: &Pubkey,
    product_record: ProductContentId,
    frame: ProductRuntimeFrameV2<'_, '_>,
) -> Result<Box<AuthenticatedProductRuntimeV2>, ProgramError> {
    Ok(Box::new(
        authenticate_product_runtime_v2(registry, product_record, frame)
            .map_err(|_| ResolutionError::ProductDomain)?,
    ))
}

#[inline(never)]
fn boxed_consume_source_records(
    market: &MarketFacts,
    accounts: &[AccountInfo<'_>],
    venue: RelayedVenueKindV1,
    material_id: [u8; 32],
    source_spec_id: [u8; 32],
) -> Result<Box<AuthenticatedRelaySourceRecordsV1>, ProgramError> {
    Ok(Box::new(consume_source_records(
        market,
        accounts,
        venue,
        material_id,
        source_spec_id,
    )?))
}

#[inline(never)]
fn consume_source_records(
    market: &MarketFacts,
    accounts: &[AccountInfo<'_>],
    venue: RelayedVenueKindV1,
    material_id: [u8; 32],
    source_spec_id: [u8; 32],
) -> Result<AuthenticatedRelaySourceRecordsV1, ProgramError> {
    let registry = &market.registry_program;
    let material_data = consume_slot(accounts, venue, 7)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        consume_slot(accounts, venue, 7)?,
        consume_slot(accounts, venue, 8)?,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        material_id,
        &material_data,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    let window_spec_id = material.window_spec().to_bytes();
    drop(material_data);

    let spec_data = consume_slot(accounts, venue, 9)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        consume_slot(accounts, venue, 9)?,
        consume_slot(accounts, venue, 10)?,
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

    let provider_data = consume_slot(accounts, venue, 11)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        consume_slot(accounts, venue, 11)?,
        consume_slot(accounts, venue, 12)?,
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

    let window_data = consume_slot(accounts, venue, 13)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        consume_slot(accounts, venue, 13)?,
        consume_slot(accounts, venue, 14)?,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id,
        &window_data,
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(window_data);

    // Named by the material, exactly like the window above, and authenticated
    // the same way. It is the record that says how the observation's unit and
    // the Product's result unit relate; without it in this walk the route had
    // to guess that they were the same one.
    let statistic_spec_id = material.statistic_spec().to_bytes();
    let statistic_data = consume_slot(accounts, venue, 15)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        consume_slot(accounts, venue, 15)?,
        consume_slot(accounts, venue, 16)?,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        statistic_spec_id,
        &statistic_data,
        STATISTIC_SPEC_BYTES,
    )?;
    let statistic =
        StatisticSpecV1::decode(&statistic_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(statistic_data);

    let config_data = consume_slot(accounts, venue, 17)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        consume_slot(accounts, venue, 17)?,
        consume_slot(accounts, venue, 18)?,
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

    // The row the configuration selects decides whether this market HAS a venue
    // deployment, and therefore which of the two consumption frames is the one
    // it may be consumed through. The caller's width chose a frame; this is
    // where the market's own configuration is asked whether that was the right
    // one, and the disagreement is its own refusal rather than a missing
    // account.
    let observable = RelayedObservableV1::from_selector(config.observable_selector())
        .map_err(|_| ResolutionError::ProviderConfiguration)?;
    if observable.venue_kind() != venue {
        return Err(ResolutionError::RelayedVenueKind.into());
    }

    let id = |value: [u8; 32]| {
        dclutch_source::ContentId::new(value).map_err(|_| ResolutionError::SourceMaterial)
    };
    // A native row has no upgradeable venue program, so the pair is absent from
    // its frame and there is nothing here to authenticate. `authenticate_graph`
    // is what then binds `adapter_config_id` to the pinned account set instead.
    let venue_release = match venue {
        RelayedVenueKindV1::LoaderV3 => {
            let venue_data = consume_slot(accounts, venue, 19)?
                .try_borrow_data()
                .map_err(|_| ResolutionError::FinalizedRecord)?;
            authenticate_record(
                registry,
                consume_slot(accounts, venue, 19)?,
                consume_slot(accounts, venue, 20)?,
                ARTIFACT_RELEASE_SCHEMA_ID_V1,
                venue_release_id,
                &venue_data,
                ARTIFACT_RELEASE_BYTES_V1,
            )?;
            let release = ArtifactReleaseV1::decode(&venue_data)
                .map_err(|_| ResolutionError::ProviderRelease)?;
            drop(venue_data);
            Some(AuthenticatedVenueReleaseV1 {
                id: id(venue_release_id)?,
                release,
            })
        }
        RelayedVenueKindV1::Native => None,
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
        statistic_spec_id: id(statistic_spec_id)?,
        statistic,
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
pub(crate) fn authenticate_source_state_account(
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
pub(crate) const RESOLUTION_SUCCESS_CERTIFICATE_KIND_SEED: u8 = 1;

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
pub(crate) fn initialize_certificate_at_kind<'info>(
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
        let field = |value: Result<[u8; 32], dclutch_source::relay::Error>| {
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

pub(crate) fn create_prefunded_pda<'info>(
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

pub(crate) fn close_to_beneficiary(
    source: &AccountInfo<'_>,
    beneficiary: &AccountInfo<'_>,
) -> ProgramResult {
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
