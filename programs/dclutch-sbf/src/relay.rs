//! Executable `RelayedMainnetStateV1` observation-record routes.
//!
//! Four permissionless routes create, fill, seal and close one cross-cluster
//! observation record. Everything they enforce lives in
//! `dclutch-relay-contract`; this module is the account boundary: it
//! authenticates the Market, the immutable raw records, the record PDA, the
//! preceding native Ed25519 instruction, and the two clocks, then hands exact
//! bytes to the contract.
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

use alloc::vec::Vec;

use dclutch_relay_contract::{
    RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1, RELAYED_RECORD_PDA_DOMAIN_V1,
    RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1, SOLANA_MAINNET_GENESIS_HASH_V1,
    frame::{RelayAccountPrivilegeV1, RelayFrameKindV1, validate_relay_frame_v1},
    instruction::{
        APPEND_OBSERVATION_PREFIX_BYTES, AppendObservationInstructionV1, CreateRecordInstructionV1,
        RelayInstructionV1, RetireRecordInstructionV1, SEAL_RECORD_PREFIX_BYTES,
        SealRecordInstructionV1,
    },
    record::{
        RelayedObservationRecordViewV1, RelayedRecordBindingV1,
        append_relayed_observation_in_place_v1, create_relayed_observation_record_into_v1,
        relayed_observation_record_bytes_v1, retire_relayed_observation_in_place_v1,
        seal_relayed_observation_in_place_v1,
    },
    release::{RelayedAdapterConfigV1, RelayerKeySetV1, encode_set_digest_seed_preimage_v1},
    signature::{
        ED25519_PROGRAM_ID_3_0, Ed25519InstructionViewV1, inspect_preceding_relay_signature_v1,
    },
    wire::{AttestationMessageV1, ObservationSetSealV1},
};
use dclutch_source_contract::{
    ContentId as SourceContentId, MarketChildDeltaKindV1, SourceAccessProfile,
};
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::sysvar;

use crate::{
    AdapterError,
    records::with_authenticated_finalized_record_v1,
    source::{
        account, authenticate_existing_rent_credit, clock, close_to_rent_credit,
        create_prefunded_pda, market_facts, persist_bytes, register_market_child, require_clock,
        require_register_delta, require_rent, require_retire_delta, require_system,
        retire_market_child, with_authenticated_material,
    },
};

/// Dispatch one exact relay instruction after top-level magic routing.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match RelayInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?
    {
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
    }
}

/// Whether the instruction data is routable to this family.
pub(crate) fn is_routable_instruction(instruction_data: &[u8]) -> bool {
    instruction_data.get(..dclutch_relay_contract::instruction::RELAY_INSTRUCTION_MAGIC.len())
        == Some(&dclutch_relay_contract::instruction::RELAY_INSTRUCTION_MAGIC)
}

/// The immutable release facts every route re-derives rather than trusts.
struct ReleaseFacts {
    provider_release_id: SourceContentId,
    relayer_key_set_id: [u8; 32],
    account_set_id: [u8; 32],
    key_set: RelayerKeySetV1,
}

fn validate_frame(
    kind: RelayFrameKindV1,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let mut observed = Vec::new();
    observed
        .try_reserve_exact(accounts.len())
        .map_err(|_| AdapterError::Arithmetic)?;
    for info in accounts {
        observed.push(RelayAccountPrivilegeV1 {
            key: info.key.to_bytes(),
            is_signer: info.is_signer,
            is_writable: info.is_writable,
        });
    }
    validate_relay_frame_v1(kind, &observed).map_err(|_| AdapterError::AccountFrameLength.into())
}

/// Authenticate one finalized raw record and return its decoded value.
fn with_raw_record<'info, T>(
    program_id: &Pubkey,
    raw: &AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
    schema_release_id: [u8; 32],
    expected_digest: [u8; 32],
    decode: impl FnOnce(&[u8]) -> Result<T, ProgramError>,
) -> Result<T, ProgramError> {
    with_authenticated_finalized_record_v1(
        program_id,
        raw,
        staging,
        rent_sysvar,
        schema_release_id,
        expected_digest,
        |record| decode(record.exact_content()),
    )
}

/// Re-derive every immutable release fact from the authenticated material.
///
/// Nothing here is taken from the instruction: the caller names accounts, and
/// each one has to hash to the identity the previous link already committed to.
#[allow(clippy::too_many_arguments)]
fn release_facts<'info>(
    program_id: &Pubkey,
    material: &AccountInfo<'info>,
    material_staging: &AccountInfo<'info>,
    key_set_raw: &AccountInfo<'info>,
    key_set_staging: &AccountInfo<'info>,
    config_raw: &AccountInfo<'info>,
    config_staging: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
    material_id: SourceContentId,
    source_spec_id: SourceContentId,
    market_child_count: u64,
) -> Result<ReleaseFacts, ProgramError> {
    let (provider_release_id, deployment_release_id, decoding_rules_id, window_max_age_seconds) =
        with_authenticated_material(
            program_id,
            material,
            material_staging,
            rent_sysvar,
            material_id,
            |view| {
                let (source, release_id, release) = view
                    .source(source_spec_id)
                    .map_err(|_| AdapterError::AccountData)?;
                if source.access_profile() != SourceAccessProfile::RelayedObservationRecord {
                    return Err(AdapterError::MarketTransition.into());
                }
                let (_, capacity) = view
                    .capacity_profile()
                    .map_err(|_| AdapterError::AccountData)?;
                // The record is a direct Market child, so an unbounded number of
                // them is an unbounded rent and child-count cost imposed on a
                // Market by any caller.  The capacity profile is where that
                // bound already lives.
                if market_child_count >= u64::from(capacity.max_shared_children()) {
                    return Err(AdapterError::MarketTransition.into());
                }
                let window = view.window().map_err(|_| AdapterError::AccountData)?;
                // The relay configuration record is named by `decoding_rules_id`,
                // not by `adapter_config_id`.  The V1 material binds
                // `adapter_config_id` to its own inline 64-byte slot, which is
                // Pyth-typed and 16 bytes too narrow for a magic-headed relay
                // record -- and the pinned ordered account set is a *decoding
                // rules* fact anyway: it names, per position, the expected
                // owning program and the pinned inline width, which is exactly
                // what the decoding-rules record is for.
                Ok((
                    release_id,
                    release.provider_deployment_release_id(),
                    release.decoding_rules_id(),
                    window.max_age_seconds(),
                ))
            },
        )?;
    let key_set = with_raw_record(
        program_id,
        key_set_raw,
        key_set_staging,
        rent_sysvar,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        deployment_release_id.to_bytes(),
        |bytes| RelayerKeySetV1::decode(bytes).map_err(|_| AdapterError::AccountData.into()),
    )?;
    let config = with_raw_record(
        program_id,
        config_raw,
        config_staging,
        rent_sysvar,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        decoding_rules_id.to_bytes(),
        |bytes| RelayedAdapterConfigV1::decode(bytes).map_err(|_| AdapterError::AccountData.into()),
    )?;
    // Section 4.7's founding-time admission predicate, enforced where a record
    // first comes into existence: the window's own liveness grace must cover the
    // declared two-clock skew allowance, so skew alone can never be the thing
    // that walks a market to its funded failure outcome.
    config
        .require_window_admits_skew(window_max_age_seconds)
        .map_err(|_| AdapterError::MarketTransition)?;
    Ok(ReleaseFacts {
        provider_release_id,
        relayer_key_set_id: deployment_release_id.to_bytes(),
        account_set_id: config.account_set_id(),
        key_set,
    })
}

fn record_binding(
    market: &AccountInfo<'_>,
    generation: u64,
    material_id: SourceContentId,
    facts: &ReleaseFacts,
    observed_slot: u64,
) -> RelayedRecordBindingV1 {
    RelayedRecordBindingV1 {
        market: market.key.to_bytes(),
        generation,
        source_material_id: material_id.to_bytes(),
        account_set_id: facts.account_set_id,
        provider_release_id: facts.provider_release_id.to_bytes(),
        relayer_key_set_id: facts.relayer_key_set_id,
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
) -> Result<(), ProgramError> {
    if record.owner != program_id || record.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = record
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let view =
        RelayedObservationRecordViewV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    let seeds = view.pda_seeds().map_err(|_| AdapterError::AccountData)?;
    if seeds.market() != market.key.to_bytes() {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn process_create_record(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CreateRecordInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(RelayFrameKindV1::CreateRecord, accounts)?;
    let worker = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let record_account = account(accounts, 2)?;
    let material = account(accounts, 3)?;
    let material_staging = account(accounts, 4)?;
    let key_set_raw = account(accounts, 5)?;
    let key_set_staging = account(accounts, 6)?;
    let config_raw = account(accounts, 7)?;
    let config_staging = account(accounts, 8)?;
    let rent_credit = account(accounts, 9)?;
    let rent_sysvar = account(accounts, 10)?;
    let clock_account = account(accounts, 11)?;
    let system = account(accounts, 12)?;
    require_system(system)?;
    require_rent(rent_sysvar)?;
    require_clock(clock_account)?;
    let (material_id, source_spec_id, rent_beneficiary) = create_bindings(&request)?;
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        material_id,
        true,
    )?;
    if market.child_count != request.expected_market_child_count() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    authenticate_existing_rent_credit(program_id, rent_credit, rent_sysvar, rent_beneficiary)?;

    let facts = release_facts(
        program_id,
        material,
        material_staging,
        key_set_raw,
        key_set_staging,
        config_raw,
        config_staging,
        rent_sysvar,
        material_id,
        source_spec_id,
        market.child_count,
    )?;
    if facts.key_set.seal_threshold() != request.seal_threshold() {
        // The threshold is a release parameter, never an instruction one.
        return Err(AdapterError::MarketTransition.into());
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
        .map_err(|_| AdapterError::AccountIdentity)?;
    if record_account.key != &expected {
        // This is the equivocation bound: the address is a function of the
        // observed slot, so a second contradictory observation of the same set
        // at the same slot has nowhere to live.
        return Err(AdapterError::AccountIdentity.into());
    }

    let width = relayed_observation_record_bytes_v1(request.set_count())
        .map_err(|_| AdapterError::AccountData)?;
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
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
    .map_err(|_| AdapterError::AccountData)?;
    let seed_digest = hash(&seed_preimage).to_bytes();

    let created = clock(clock_account)?.unix_timestamp;
    let delta = {
        let mut data = record_account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?;
        create_relayed_observation_record_into_v1(
            &mut data,
            record_binding(
                market_account,
                request.generation(),
                material_id,
                &facts,
                request.observed_slot(),
            ),
            rent_beneficiary,
            request.set_count(),
            request.seal_threshold(),
            seed_digest,
            created,
            request.expected_market_child_count(),
            market.child_count,
        )
        .map_err(|_| AdapterError::MarketTransition)?
    };
    require_register_delta(delta, market.child_count)?;
    let market_bytes = register_market_child(
        program_id,
        market_account,
        request.generation(),
        material_id,
        request.expected_market_child_count(),
    )?;
    persist_bytes(market_account, &market_bytes)
}

fn process_append(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    request: AppendObservationInstructionV1,
    message: &[u8],
) -> Result<(), ProgramError> {
    validate_frame(RelayFrameKindV1::AppendObservation, accounts)?;
    let market_account = account(accounts, 1)?;
    let record_account = account(accounts, 2)?;
    let key_set_raw = account(accounts, 3)?;
    let key_set_staging = account(accounts, 4)?;
    let rent_sysvar = account(accounts, 5)?;
    let instructions = account(accounts, 6)?;
    let clock_account = account(accounts, 7)?;
    require_rent(rent_sysvar)?;
    require_clock(clock_account)?;
    authenticate_record_account(program_id, record_account, market_account)?;

    let attestation =
        AttestationMessageV1::decode(message).map_err(|_| AdapterError::AccountData)?;
    let signer = authenticate_adjacent_signature(
        program_id,
        accounts,
        instruction_data,
        instructions,
        APPEND_OBSERVATION_PREFIX_BYTES,
        message.len(),
    )?;

    let persisted = persisted_binding(
        program_id,
        record_account,
        market_account,
        key_set_raw,
        key_set_staging,
        rent_sysvar,
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
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if attestation.relay_family_id() != dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1 {
        return Err(AdapterError::MarketTransition.into());
    }
    // The attestation's `decoding_rules_id` is not compared here. Filling only
    // moves bytes the signer committed to; the decoding rules are what turn
    // those bytes into an observation, so their identity is checked where they
    // are actually applied, at resolution. A relayer that echoes the wrong
    // rules identity has signed a statement that no resolution will accept.

    let body_width = attestation.body().encoded_len();
    let body = message
        .get(message.len().saturating_sub(body_width)..)
        .ok_or(AdapterError::AccountData)?;

    let mut data = record_account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    let running = {
        let view =
            RelayedObservationRecordViewV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
        view.set_digest().map_err(|_| AdapterError::AccountData)?
    };
    let folded = hashv(&[running.as_slice(), body]).to_bytes();
    append_relayed_observation_in_place_v1(&mut data, persisted.binding, attestation, folded)
        .map_err(|_| AdapterError::MarketTransition.into())
}

fn process_seal(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    request: SealRecordInstructionV1,
    message: &[u8],
) -> Result<(), ProgramError> {
    validate_frame(RelayFrameKindV1::SealRecord, accounts)?;
    let market_account = account(accounts, 1)?;
    let record_account = account(accounts, 2)?;
    let key_set_raw = account(accounts, 3)?;
    let key_set_staging = account(accounts, 4)?;
    let rent_sysvar = account(accounts, 5)?;
    let instructions = account(accounts, 6)?;
    let clock_account = account(accounts, 7)?;
    require_rent(rent_sysvar)?;
    require_clock(clock_account)?;
    authenticate_record_account(program_id, record_account, market_account)?;

    let seal = ObservationSetSealV1::decode(message).map_err(|_| AdapterError::AccountData)?;
    let signer = authenticate_adjacent_signature(
        program_id,
        accounts,
        instruction_data,
        instructions,
        SEAL_RECORD_PREFIX_BYTES,
        message.len(),
    )?;

    let persisted = persisted_binding(
        program_id,
        record_account,
        market_account,
        key_set_raw,
        key_set_staging,
        rent_sysvar,
        request.generation(),
        request.observed_slot(),
    )?;
    // Sealing is m-of-n and the member's position in the release key set is
    // what the bitmap records, so one member cannot reach a quorum alone.
    let member = persisted
        .key_set
        .require_member(&signer)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    let sealed_at = clock(clock_account)?.unix_timestamp;

    let mut data = record_account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::AccountData)?;
    seal_relayed_observation_in_place_v1(&mut data, persisted.binding, seal, member, sealed_at)
        .map_err(|_| AdapterError::MarketTransition.into())
}

fn process_retire(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: RetireRecordInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(RelayFrameKindV1::RetireRecord, accounts)?;
    let market_account = account(accounts, 1)?;
    let record_account = account(accounts, 2)?;
    let rent_credit = account(accounts, 3)?;
    authenticate_record_account(program_id, record_account, market_account)?;

    let (material_id, beneficiary, created) = {
        let data = record_account
            .try_borrow_data()
            .map_err(|_| AdapterError::AccountData)?;
        let view =
            RelayedObservationRecordViewV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
        (
            SourceContentId::new(
                view.source_material_id()
                    .map_err(|_| AdapterError::AccountData)?,
            )
            .map_err(|_| AdapterError::ContentIdentity)?,
            view.rent_credit_beneficiary()
                .map_err(|_| AdapterError::AccountData)?,
            view.created_unix_seconds()
                .map_err(|_| AdapterError::AccountData)?,
        )
    };
    crate::source::authenticate_existing_rent_credit_without_sysvar(
        program_id,
        rent_credit,
        beneficiary,
    )?;
    let market = market_facts(
        program_id,
        market_account,
        request.generation(),
        material_id,
        false,
    )?;
    if market.child_count != request.expected_market_child_count() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let delta = {
        let mut data = record_account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?;
        retire_relayed_observation_in_place_v1(
            &mut data,
            request.generation(),
            created,
            request.expected_market_child_count(),
            market.child_count,
        )
        .map_err(|_| AdapterError::MarketTransition)?
    };
    require_retire_delta(delta, market.child_count)?;
    if delta.kind() != MarketChildDeltaKindV1::Retire {
        return Err(AdapterError::MarketTransition.into());
    }
    let market_bytes = retire_market_child(
        program_id,
        market_account,
        request.generation(),
        material_id,
        request.expected_market_child_count(),
    )?;
    persist_bytes(market_account, &market_bytes)?;
    close_to_rent_credit(record_account, rent_credit)
}

/// Read the Source-graph coordinates a create request names.
///
/// Only creation is told them. Every later route reads them back out of the
/// persisted record instead, so a caller cannot re-point a live record at a
/// different Source graph, and `market_facts` independently requires the
/// material identity to be the one the Market itself persists.
fn create_bindings(
    request: &CreateRecordInstructionV1,
) -> Result<(SourceContentId, SourceContentId, [u8; 32]), ProgramError> {
    let material_id = SourceContentId::new(request.source_material_id())
        .map_err(|_| AdapterError::ContentIdentity)?;
    let source_spec_id = SourceContentId::new(request.source_spec_id())
        .map_err(|_| AdapterError::ContentIdentity)?;
    Ok((material_id, source_spec_id, request.rent_beneficiary()))
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

#[allow(clippy::too_many_arguments)]
fn persisted_binding<'info>(
    program_id: &Pubkey,
    record: &AccountInfo<'info>,
    market: &AccountInfo<'info>,
    key_set_raw: &AccountInfo<'info>,
    key_set_staging: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
    generation: u64,
    observed_slot: u64,
) -> Result<PersistedBindingV1, ProgramError> {
    let binding = {
        let data = record
            .try_borrow_data()
            .map_err(|_| AdapterError::AccountData)?;
        let view =
            RelayedObservationRecordViewV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
        let field = |value: Result<[u8; 32], dclutch_relay_contract::Error>| {
            value.map_err(|_| AdapterError::AccountData)
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
    let key_set = with_raw_record(
        program_id,
        key_set_raw,
        key_set_staging,
        rent_sysvar,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        binding.relayer_key_set_id,
        |bytes| RelayerKeySetV1::decode(bytes).map_err(|_| AdapterError::AccountData.into()),
    )?;
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
        return Err(AdapterError::AccountIdentity.into());
    }
    let current =
        load_current_index_checked(instructions).map_err(|_| AdapterError::DirectAuthentication)?;
    let loaded = load_instruction_at_checked(usize::from(current), instructions)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if loaded.program_id != *program_id
        || loaded.data.as_slice() != instruction_data
        || loaded.accounts.len() != accounts.len()
    {
        return Err(AdapterError::DirectAuthentication.into());
    }
    for (meta, actual) in loaded.accounts.iter().zip(accounts) {
        if meta.pubkey != *actual.key
            || meta.is_signer != actual.is_signer
            || meta.is_writable != actual.is_writable
        {
            return Err(AdapterError::DirectAuthentication.into());
        }
    }
    let preceding_index = current
        .checked_sub(1)
        .ok_or(AdapterError::DirectAuthentication)?;
    let preceding = load_instruction_at_checked(usize::from(preceding_index), instructions)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    if !preceding.accounts.is_empty() || preceding.program_id.to_bytes() != ED25519_PROGRAM_ID_3_0 {
        return Err(AdapterError::DirectAuthentication.into());
    }
    let view = Ed25519InstructionViewV1 {
        program_id: preceding.program_id.to_bytes(),
        ed25519_data: preceding.data.as_slice(),
        preceding_index,
        current_index: current,
        current_data: instruction_data,
    };
    let offset = u16::try_from(message_offset).map_err(|_| AdapterError::Arithmetic)?;
    let length = u16::try_from(message_len).map_err(|_| AdapterError::Arithmetic)?;
    let authorization = inspect_preceding_relay_signature_v1(view, offset, length)
        .map_err(|_| AdapterError::DirectAuthentication)?;
    Ok(authorization.signer())
}
