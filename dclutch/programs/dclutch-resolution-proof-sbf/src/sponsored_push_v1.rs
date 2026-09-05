//! Permissionless capture and terminal consumption of a sponsored Pyth push feed.
//!
//! The upstream account is mutable latest-value storage. Capture therefore
//! writes an immutable, sponsor-funded candidate and advances one canonical
//! best-valid-submitted head. Candidate admission closes at
//! `window.end + max_age`; settlement and head-vacant funded failure begin only
//! after that same strict deadline.

use alloc::boxed::Box;

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_product::{ContentId as ProductContentId, ResultDomainV2};
use dclutch_product::svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_source::pyth::{
    DEVNET_CLUSTER_ID_V1, FULL_PRICE_UPDATE_V2_LEN, FullPriceUpdateV2,
    PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1, PYTH_SPONSORED_PUSH_RELEASE_V1_ENCODED_LEN,
    PythSponsoredPushReleaseV1, RECEIVER_CONFIG_V2_LEN,
};
use dclutch_registry::svm::ProgramDataV3View;
use dclutch_source::relay::frame::RelayFrameKindV1;
use dclutch_source::resolution::{
    RESOLUTION_CERTIFICATE_BYTES_V2, ResolutionCertificateKindV2, ResolutionCertificateV2,
    SPONSORED_PUSH_CANDIDATE_BYTES_V1, SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1,
    SPONSORED_PUSH_CAPTURE_ACCOUNT_COUNT_V1, SPONSORED_PUSH_CLOSE_CANDIDATE_ACCOUNT_COUNT_V1,
    SPONSORED_PUSH_CLOSE_HEAD_ACCOUNT_COUNT_V1, SPONSORED_PUSH_COMMIT_FAILURE_ACCOUNT_COUNT_V1,
    SPONSORED_PUSH_HEAD_BYTES_V1, SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
    SPONSORED_PUSH_INSTRUCTION_MAGIC_V1, SPONSORED_PUSH_RECEIPT_BYTES_V1,
    SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1, SPONSORED_PUSH_SETTLE_ACCOUNT_COUNT_V1,
    SponsoredPushActionV1, SponsoredPushCandidateV1, SponsoredPushHeadV1,
    SponsoredPushInstructionV1, SponsoredPushReceiptV1,
};
use dclutch_source::{
    ContentId as SourceContentId, PROVIDER_RELEASE_BYTES, PROVIDER_RELEASE_SCHEMA_ID_V1,
    PYTH_ADAPTER_CONFIG_BYTES, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, ProviderReleaseV1,
    PythAdapterConfigV1, PythProviderAdapterObligationV2, SOURCE_FAILURE_POLICY_RELEASE_ID_V2,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_MATERIAL_V3_BYTES,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SOURCE_SPEC_BYTES, SOURCE_SPEC_SCHEMA_ID_V1,
    STATISTIC_SPEC_BYTES, STATISTIC_SPEC_SCHEMA_ID_V1, SourceAccessProfile, SourceMaterialV3,
    SourceResolutionPhaseV1, SourceResolutionStateV2, SourceSpecV1, StatisticSpecV1,
    WINDOW_SPEC_BYTES, WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::system_program;

use crate::market_admission_v1::RESOLUTION_PRIMARY_SOURCE_ADMISSIBLE_STATES_V1;
use crate::{
    ResolutionError, authenticate_clock, authenticate_rent,
    provider_instruction_v3::{authenticate_provider_program, authenticate_record},
    relay_transport_v1::{
        MarketFacts, RESOLUTION_SUCCESS_CERTIFICATE_KIND_SEED, account, authenticate_market,
        authenticate_source_state_account, boxed_product_runtime, close_to_beneficiary,
        create_prefunded_pda, initialize_certificate_at_kind, process_deadline_failure_coordinates,
        require_system, validate_frame,
    },
};

/// Return whether bytes select the sponsored-push family.
pub(crate) fn is_sponsored_push_v1(bytes: &[u8]) -> bool {
    bytes.get(..8) == Some(&SPONSORED_PUSH_INSTRUCTION_MAGIC_V1)
}

/// Dispatch one exact sponsored-push action.
#[inline(never)]
pub(crate) fn process_sponsored_push_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = SponsoredPushInstructionV1::decode(instruction_data)
        .map_err(|_| ResolutionError::Instruction)?;
    match request.action {
        SponsoredPushActionV1::Capture => process_capture(program_id, accounts, request),
        SponsoredPushActionV1::Settle => process_settle(program_id, accounts, request),
        SponsoredPushActionV1::CloseCandidate => {
            process_close_candidate(program_id, accounts, request)
        }
        SponsoredPushActionV1::CloseHead => process_close_head(program_id, accounts, request),
        SponsoredPushActionV1::CommitFailure => {
            process_commit_failure(program_id, accounts, request)
        }
    }
}

#[inline(never)]
fn process_capture(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SponsoredPushInstructionV1,
) -> ProgramResult {
    authenticate_capture_frame(accounts)?;
    let payer = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let head_account = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let source_state_account = account(accounts, 6)?;
    let clock = authenticate_clock(account(accounts, 27)?)?;
    let rent = authenticate_rent(account(accounts, 28)?)?;
    let system = account(accounts, 29)?;
    require_system(system)?;
    if payer.owner != &system_program::ID || clock.slot == 0 || clock.unix_timestamp <= 0 {
        return Err(ResolutionError::AccountFrame.into());
    }

    let market = authenticate_market(
        program_id,
        market_account,
        account(accounts, 2)?,
        account(accounts, 3)?,
        request.generation,
        source_material_id(source_state_account)?,
    )?;
    authenticate_source_state_account(program_id, source_state_account, market_account)?;
    let source_state = boxed_source_state(source_state_account)?;
    if !RESOLUTION_PRIMARY_SOURCE_ADMISSIBLE_STATES_V1.admits(source_state.phase()) {
        return Err(ResolutionError::Transition.into());
    }
    let records = boxed_source_records(accounts, &market, &source_state, 7)?;
    let sponsored = authenticate_sponsored_release(accounts, &market, &records, 7)?;
    let update = authenticate_live_update(accounts, &clock, sponsored, &records)?;
    let primary_deadline = records
        .window
        .end_unix_seconds()
        .checked_add(i64::from(records.window.max_age_seconds()))
        .ok_or(ResolutionError::Arithmetic)?;
    if clock.unix_timestamp > primary_deadline {
        return Err(ResolutionError::ProviderFreshness.into());
    }

    let update_data = account(accounts, 21)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let update_bytes: [u8; FULL_PRICE_UPDATE_V2_LEN] = update_data
        .as_ref()
        .try_into()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let update_digest = hash(&update_bytes).to_bytes();
    let sponsored_bytes = sponsored.to_bytes();
    let sponsored_release_id = hash(&sponsored_bytes).to_bytes();
    let candidate = SponsoredPushCandidateV1 {
        market: market_account.key.to_bytes(),
        source_state: source_state_account.key.to_bytes(),
        provider_release: records.provider_release_id.to_bytes(),
        sponsored_release: sponsored_release_id,
        price_account: account(accounts, 21)?.key.to_bytes(),
        refund_recipient: payer.key.to_bytes(),
        update_digest,
        generation: request.generation,
        snapshot_slot: clock.slot,
        snapshot_unix_seconds: clock.unix_timestamp,
        publish_time: update.publish_time(),
        posted_slot: update.posted_slot(),
        bump: candidate_bump(
            program_id,
            market_account.key,
            request.generation,
            &sponsored_release_id,
            account(accounts, 21)?.key,
            update.publish_time(),
            update.posted_slot(),
            &update_digest,
            candidate_account.key,
        )?,
        update_bytes,
    };
    let candidate_bytes = Box::new(
        candidate
            .to_bytes()
            .map_err(|_| ResolutionError::SponsoredPush)?,
    );
    drop(update_data);
    initialize_candidate(
        program_id,
        payer,
        candidate_account,
        system,
        &rent,
        candidate,
    )?;
    let (head_bytes, initialize_head) = boxed_next_head(
        program_id,
        market_account,
        source_state_account,
        head_account,
        candidate_account,
        candidate,
        payer.key.to_bytes(),
    )?;
    if initialize_head {
        initialize_head_account(
            program_id,
            payer,
            head_account,
            system,
            &rent,
            request.generation,
            market_account.key,
            &sponsored_release_id,
        )?;
    }
    commit_capture(
        candidate_account,
        head_account,
        &candidate_bytes,
        &head_bytes,
    )
}

fn authenticate_capture_frame(accounts: &[AccountInfo<'_>]) -> ProgramResult {
    if accounts.len() != SPONSORED_PUSH_CAPTURE_ACCOUNT_COUNT_V1 {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, info) in accounts.iter().enumerate() {
        let executable = matches!(index, 2 | 22 | 24 | 29);
        if info.is_signer != (index == 0)
            || info.is_writable != matches!(index, 0 | 4 | 5)
            || info.executable != executable
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == info.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    Ok(())
}

fn source_material_id(state: &AccountInfo<'_>) -> Result<[u8; 32], ProgramError> {
    let data = state
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let decoded =
        SourceResolutionStateV2::decode(&data).map_err(|_| ResolutionError::OutputState)?;
    Ok(decoded.material_id().to_bytes())
}

fn boxed_source_state(
    account_info: &AccountInfo<'_>,
) -> Result<Box<SourceResolutionStateV2>, ProgramError> {
    let data = account_info
        .try_borrow_data()
        .map_err(|_| ResolutionError::OutputState)?;
    Ok(Box::new(
        SourceResolutionStateV2::decode(&data).map_err(|_| ResolutionError::OutputState)?,
    ))
}

#[derive(Clone, Copy)]
struct SponsoredSourceRecordsV1 {
    material_id: SourceContentId,
    material: SourceMaterialV3,
    source_spec_id: SourceContentId,
    source: SourceSpecV1,
    provider_release_id: SourceContentId,
    provider_release: ProviderReleaseV1,
    adapter_config_id: SourceContentId,
    adapter: PythAdapterConfigV1,
    window_spec_id: SourceContentId,
    window: WindowSpecV1,
    statistic_spec_id: SourceContentId,
    statistic: StatisticSpecV1,
}

#[inline(never)]
fn boxed_source_records(
    accounts: &[AccountInfo<'_>],
    market: &MarketFacts,
    source_state: &SourceResolutionStateV2,
    record_base: usize,
) -> Result<Box<SponsoredSourceRecordsV1>, ProgramError> {
    Ok(Box::new(capture_records(
        accounts,
        market,
        source_state,
        record_base,
    )?))
}

fn capture_records(
    accounts: &[AccountInfo<'_>],
    market: &MarketFacts,
    source_state: &SourceResolutionStateV2,
    record_base: usize,
) -> Result<SponsoredSourceRecordsV1, ProgramError> {
    let material_id = source_state.material_id();
    let material_data = borrow_record(
        accounts,
        record_base,
        &market.registry_program,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        material_id.to_bytes(),
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if material.product_record_digest().to_bytes() != market.product_record {
        return Err(ResolutionError::SourceMaterial.into());
    }
    drop(material_data);

    let source_spec_id = material.primary_source_spec();
    let source_data = borrow_record(
        accounts,
        record_base + 2,
        &market.registry_program,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id.to_bytes(),
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&source_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if source.access_profile() != SourceAccessProfile::PythSponsoredPushSnapshot {
        return Err(ResolutionError::SourceMaterial.into());
    }
    drop(source_data);

    let provider_release_id = source.provider_release_id();
    let provider_data = borrow_record(
        accounts,
        record_base + 4,
        &market.registry_program,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_id.to_bytes(),
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider_release =
        ProviderReleaseV1::decode(&provider_data).map_err(|_| ResolutionError::ProviderRelease)?;
    drop(provider_data);

    let adapter_config_id = source.adapter_config_id();
    let adapter_data = borrow_record(
        accounts,
        record_base + 6,
        &market.registry_program,
        PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
        adapter_config_id.to_bytes(),
        PYTH_ADAPTER_CONFIG_BYTES,
    )?;
    let adapter = PythAdapterConfigV1::decode(&adapter_data)
        .map_err(|_| ResolutionError::ProviderConfiguration)?;
    drop(adapter_data);

    let window_spec_id = material.window_spec();
    let window_data = borrow_record(
        accounts,
        record_base + 8,
        &market.registry_program,
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_spec_id.to_bytes(),
        WINDOW_SPEC_BYTES,
    )?;
    let window = WindowSpecV1::decode(&window_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(window_data);

    let statistic_spec_id = material.statistic_spec();
    let statistic_data = borrow_record(
        accounts,
        record_base + 10,
        &market.registry_program,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        statistic_spec_id.to_bytes(),
        STATISTIC_SPEC_BYTES,
    )?;
    let statistic =
        StatisticSpecV1::decode(&statistic_data).map_err(|_| ResolutionError::SourceMaterial)?;
    drop(statistic_data);

    PythProviderAdapterObligationV2::from_authenticated_sponsored_push_records(
        material,
        material.product_record_digest(),
        source_spec_id,
        source,
        provider_release_id,
        provider_release,
        adapter_config_id,
        adapter,
        window_spec_id,
        window,
        statistic_spec_id,
        statistic,
        SourceContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2)
            .map_err(|_| ResolutionError::SourceMaterial)?,
    )
    .map_err(|_| ResolutionError::SourceMaterial)?;
    Ok(SponsoredSourceRecordsV1 {
        material_id,
        material,
        source_spec_id,
        source,
        provider_release_id,
        provider_release,
        adapter_config_id,
        adapter,
        window_spec_id,
        window,
        statistic_spec_id,
        statistic,
    })
}

fn borrow_record<'a>(
    accounts: &'a [AccountInfo<'_>],
    index: usize,
    registry: &Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
    len: usize,
) -> Result<core::cell::Ref<'a, &'a mut [u8]>, ProgramError> {
    let data = account(accounts, index)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::FinalizedRecord)?;
    authenticate_record(
        registry,
        account(accounts, index)?,
        account(accounts, index + 1)?,
        schema,
        digest,
        &data,
        len,
    )?;
    Ok(data)
}

#[inline(never)]
fn authenticate_sponsored_release(
    accounts: &[AccountInfo<'_>],
    market: &MarketFacts,
    records: &SponsoredSourceRecordsV1,
    record_base: usize,
) -> Result<PythSponsoredPushReleaseV1, ProgramError> {
    let release_id = records
        .provider_release
        .provider_deployment_release_id()
        .to_bytes();
    let data = borrow_record(
        accounts,
        record_base + 12,
        &market.registry_program,
        PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
        release_id,
        PYTH_SPONSORED_PUSH_RELEASE_V1_ENCODED_LEN,
    )?;
    let release =
        PythSponsoredPushReleaseV1::decode(&data).map_err(|_| ResolutionError::ProviderRelease)?;
    if records.provider_release.provider_family_id().to_bytes() != release.provider_family_id()
        || records.provider_release.adapter_release_id().to_bytes() != release.adapter_id()
        || records.provider_release.decoding_rules_id().to_bytes()
            != release.price_update_codec_id()
        || records.provider_release.transport_profile_id().to_bytes()
            != release.transport_profile_id()
        || release.cluster_id() != DEVNET_CLUSTER_ID_V1
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    Ok(release)
}

fn authenticate_live_update(
    accounts: &[AccountInfo<'_>],
    clock: &Clock,
    release: PythSponsoredPushReleaseV1,
    records: &SponsoredSourceRecordsV1,
) -> Result<FullPriceUpdateV2, ProgramError> {
    let price = account(accounts, 21)?;
    let receiver = account(accounts, 22)?;
    let receiver_programdata = account(accounts, 23)?;
    let push = account(accounts, 24)?;
    let push_programdata = account(accounts, 25)?;
    let receiver_config = account(accounts, 26)?;
    if price.key.to_bytes() != release.price_account()
        || price.owner != receiver.key
        || price.executable
        || price.data_len() != FULL_PRICE_UPDATE_V2_LEN
        || !funded_rent_persists_v1(price.lamports())
        || receiver.key.to_bytes() != release.receiver_program()
        || receiver_programdata.key.to_bytes() != release.receiver_programdata()
        || push.key.to_bytes() != release.push_oracle_program()
        || push_programdata.key.to_bytes() != release.push_oracle_programdata()
        || receiver_config.key.to_bytes() != release.receiver_config()
        || receiver_config.owner != receiver.key
        || receiver_config.executable
        || receiver_config.data_len() != RECEIVER_CONFIG_V2_LEN
        || !funded_rent_persists_v1(receiver_config.lamports())
        || release.activation_time() > clock.unix_timestamp
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    authenticate_provider_program_pin(
        receiver,
        receiver_programdata,
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
        release.receiver_upgrade_authority(),
    )?;
    authenticate_provider_program_pin(
        push,
        push_programdata,
        release.push_oracle_programdata(),
        release.push_oracle_deployment_slot(),
        release.push_oracle_upgrade_authority(),
    )?;
    let config_data = receiver_config
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderRelease)?;
    if hash(&config_data).to_bytes() != release.receiver_config_digest() {
        return Err(ResolutionError::ReleaseSuperseded.into());
    }
    drop(config_data);
    let shard = release.shard().to_le_bytes();
    let (expected_price, bump) =
        Pubkey::find_program_address(&[&shard, &release.feed_id()], push.key);
    let data = price
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderObservation)?;
    let update =
        FullPriceUpdateV2::parse(&data).map_err(|_| ResolutionError::ProviderObservation)?;
    if expected_price != *price.key
        || bump != release.feed_account_bump()
        || update.write_authority() != price.key.to_bytes()
        || update.feed_id() != release.feed_id()
        || update.posted_slot() == 0
        || update.posted_slot() > clock.slot
        || update.publish_time() <= 0
        || update.prev_publish_time() > update.publish_time()
    {
        return Err(ResolutionError::ProviderObservation.into());
    }
    let evidence = SourceContentId::new(hash(&data).to_bytes())
        .map_err(|_| ResolutionError::ProviderObservation)?;
    PythProviderAdapterObligationV2::from_authenticated_sponsored_push_records(
        records.material,
        records.material.product_record_digest(),
        records.source_spec_id,
        records.source,
        records.provider_release_id,
        records.provider_release,
        records.adapter_config_id,
        records.adapter,
        records.window_spec_id,
        records.window,
        records.statistic_spec_id,
        records.statistic,
        SourceContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2)
            .map_err(|_| ResolutionError::SourceMaterial)?,
    )
    .and_then(|obligation| {
        obligation.normalize_authenticated_update(
            evidence,
            update.feed_id(),
            update.price(),
            update.confidence(),
            update.exponent(),
            update.publish_time(),
            clock.unix_timestamp,
        )
    })
    .map_err(|error| match error {
        dclutch_source::Error::InvalidObservationSchedule => {
            ResolutionError::ProviderWindow
        }
        dclutch_source::Error::InvalidPublicationTime => {
            ResolutionError::ProviderFreshness
        }
        dclutch_source::Error::InvalidPythObservation => {
            ResolutionError::ProviderConfiguration
        }
        _ => ResolutionError::ProviderObservation,
    })?;
    Ok(update)
}

/// Authenticate the current Loader-v3 metadata against an immutable release.
///
/// The release also commits the independently reproduced complete ELF digest.
/// Rehashing roughly 1.64 MiB of ProgramData on every capture is not a viable
/// transaction path. Loader-v3 advances the deployment slot on every upgrade,
/// so exact `(ProgramData, slot, authority)` equality makes any byte-changing
/// upgrade fail closed as `ReleaseSuperseded`; release publication remains the
/// one owner of the expensive ELF measurement.
fn authenticate_provider_program_pin(
    program: &AccountInfo<'_>,
    programdata: &AccountInfo<'_>,
    expected_programdata: [u8; 32],
    expected_slot: u64,
    expected_upgrade_authority: [u8; 32],
) -> ProgramResult {
    let bytes = programdata
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProviderRelease)?;
    let view = ProgramDataV3View::parse(&bytes).map_err(|_| ResolutionError::ProviderRelease)?;
    let observed_slot = view.deployment_slot();
    let observed_authority = view.upgrade_authority();
    drop(bytes);
    // The shared helper remains the owner of Loader-v3 ownership, executable,
    // Program→ProgramData linkage, address, and metadata shape. Passing the
    // already parsed observed slot deliberately removes only slot POLICY from
    // that generic boundary; this sponsored release maps deployment drift to
    // the stronger, non-downgradable `ReleaseSuperseded` refusal below.
    authenticate_provider_program(program, programdata, expected_programdata, observed_slot)?;
    if observed_slot != expected_slot || observed_authority != Some(expected_upgrade_authority) {
        return Err(ResolutionError::ReleaseSuperseded.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn candidate_bump(
    program_id: &Pubkey,
    market: &Pubkey,
    generation: u64,
    sponsored_release: &[u8; 32],
    price_account: &Pubkey,
    publish_time: i64,
    posted_slot: u64,
    update_digest: &[u8; 32],
    supplied: &Pubkey,
) -> Result<u8, ProgramError> {
    let generation = generation.to_le_bytes();
    let publish_time = publish_time.to_le_bytes();
    let posted_slot = posted_slot.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1,
            market.as_ref(),
            &generation,
            sponsored_release,
            price_account.as_ref(),
            &publish_time,
            &posted_slot,
            update_digest,
        ],
        program_id,
    );
    if supplied != &expected {
        return Err(ResolutionError::SponsoredPush.into());
    }
    Ok(bump)
}

fn initialize_candidate<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    candidate_account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
    candidate: SponsoredPushCandidateV1,
) -> ProgramResult {
    let generation = candidate.generation.to_le_bytes();
    let publish_time = candidate.publish_time.to_le_bytes();
    let posted_slot = candidate.posted_slot.to_le_bytes();
    let bump = [candidate.bump];
    create_prefunded_pda(
        payer,
        candidate_account,
        system,
        rent.minimum_balance(SPONSORED_PUSH_CANDIDATE_BYTES_V1),
        SPONSORED_PUSH_CANDIDATE_BYTES_V1,
        program_id,
        &[
            SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1,
            &candidate.market,
            &generation,
            &candidate.sponsored_release,
            &candidate.price_account,
            &publish_time,
            &posted_slot,
            &candidate.update_digest,
            &bump,
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn boxed_next_head(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    source_state: &AccountInfo<'_>,
    head: &AccountInfo<'_>,
    candidate_account: &AccountInfo<'_>,
    candidate: SponsoredPushCandidateV1,
    first_head_refund_recipient: [u8; 32],
) -> Result<(Box<[u8; SPONSORED_PUSH_HEAD_BYTES_V1]>, bool), ProgramError> {
    let generation = candidate.generation.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
            market.key.as_ref(),
            &generation,
            &candidate.sponsored_release,
        ],
        program_id,
    );
    if head.key != &expected || source_state.key.to_bytes() != candidate.source_state {
        return Err(ResolutionError::SponsoredPush.into());
    }
    if head.owner == &system_program::ID {
        if head.executable || head.data_len() != 0 {
            return Err(ResolutionError::SponsoredPush.into());
        }
        let first = SponsoredPushHeadV1::first(
            candidate_account.key.to_bytes(),
            candidate,
            first_head_refund_recipient,
            bump,
        )
        .map_err(|_| ResolutionError::SponsoredPush)?;
        return Ok((
            Box::new(
                first
                    .to_bytes()
                    .map_err(|_| ResolutionError::SponsoredPush)?,
            ),
            true,
        ));
    }
    if head.owner != program_id
        || head.executable
        || head.data_len() != SPONSORED_PUSH_HEAD_BYTES_V1
        || !funded_rent_persists_v1(head.lamports())
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    let data = head
        .try_borrow_data()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    let current = SponsoredPushHeadV1::decode(&data).map_err(|_| ResolutionError::SponsoredPush)?;
    if current.market != candidate.market
        || current.source_state != candidate.source_state
        || current.provider_release != candidate.provider_release
        || current.sponsored_release != candidate.sponsored_release
        || current.price_account != candidate.price_account
        || current.generation != candidate.generation
        || current.bump != bump
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    let next = if candidate.selection().is_after(current.selection()) {
        current
            .select(candidate_account.key.to_bytes(), candidate)
            .map_err(|_| ResolutionError::SponsoredPush)?
    } else {
        current
    };
    Ok((
        Box::new(
            next.to_bytes()
                .map_err(|_| ResolutionError::SponsoredPush)?,
        ),
        false,
    ))
}

#[allow(clippy::too_many_arguments)]
fn initialize_head_account<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    head: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
    generation: u64,
    market: &Pubkey,
    sponsored_release: &[u8; 32],
) -> ProgramResult {
    let generation = generation.to_le_bytes();
    let bump = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
            market.as_ref(),
            &generation,
            sponsored_release,
        ],
        program_id,
    )
    .1;
    let bump = [bump];
    create_prefunded_pda(
        payer,
        head,
        system,
        rent.minimum_balance(SPONSORED_PUSH_HEAD_BYTES_V1),
        SPONSORED_PUSH_HEAD_BYTES_V1,
        program_id,
        &[
            SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
            market.as_ref(),
            &generation,
            sponsored_release,
            &bump,
        ],
    )
}

fn commit_capture(
    candidate: &AccountInfo<'_>,
    head: &AccountInfo<'_>,
    candidate_bytes: &[u8; SPONSORED_PUSH_CANDIDATE_BYTES_V1],
    head_bytes: &[u8; SPONSORED_PUSH_HEAD_BYTES_V1],
) -> ProgramResult {
    let mut candidate_data = candidate
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    let mut head_data = head
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    if candidate_data.len() != candidate_bytes.len()
        || candidate_data.iter().any(|byte| *byte != 0)
        || head_data.len() != head_bytes.len()
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    candidate_data.copy_from_slice(candidate_bytes);
    head_data.copy_from_slice(head_bytes);
    Ok(())
}

#[inline(never)]
fn process_settle(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SponsoredPushInstructionV1,
) -> ProgramResult {
    authenticate_settle_frame(accounts)?;
    let resolver = account(accounts, 0)?;
    let market_account = account(accounts, 1)?;
    let head_account = account(accounts, 4)?;
    let candidate_account = account(accounts, 5)?;
    let source_state_account = account(accounts, 6)?;
    let certificate_account = account(accounts, 7)?;
    let receipt_account = account(accounts, 8)?;
    let clock = authenticate_clock(account(accounts, 29)?)?;
    let rent = authenticate_rent(account(accounts, 30)?)?;
    let system = account(accounts, 31)?;
    require_system(system)?;
    if resolver.owner != &system_program::ID || clock.slot == 0 || clock.unix_timestamp <= 0 {
        return Err(ResolutionError::AccountFrame.into());
    }

    let market = authenticate_market(
        program_id,
        market_account,
        account(accounts, 2)?,
        account(accounts, 3)?,
        request.generation,
        source_material_id(source_state_account)?,
    )?;
    authenticate_source_state_account(program_id, source_state_account, market_account)?;
    let source_state = boxed_source_state(source_state_account)?;
    if !RESOLUTION_PRIMARY_SOURCE_ADMISSIBLE_STATES_V1.admits(source_state.phase()) {
        return Err(ResolutionError::Transition.into());
    }
    let records = boxed_source_records(accounts, &market, &source_state, 9)?;
    let sponsored = authenticate_sponsored_release(accounts, &market, &records, 9)?;
    let deadline = primary_deadline(records.window)?;
    if clock.unix_timestamp <= deadline {
        return Err(ResolutionError::ProviderFreshness.into());
    }

    let sealed = boxed_sealed_candidate(
        program_id,
        market_account,
        source_state_account,
        head_account,
        candidate_account,
        request.generation,
        sponsored,
        &records,
        &clock,
    )?;
    let product_runtime = boxed_product_runtime(
        &market.registry_program,
        ProductContentId::new(market.product_record).map_err(|_| ResolutionError::ProductDomain)?,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: account(accounts, 23)?,
                staging: account(accounts, 24)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: account(accounts, 25)?,
                staging: account(accounts, 26)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: account(accounts, 27)?,
                staging: account(accounts, 28)?,
            },
        },
    )?;
    let domain_data = account(accounts, 25)?
        .try_borrow_data()
        .map_err(|_| ResolutionError::ProductDomain)?;
    let result_domain =
        ResultDomainV2::decode(&domain_data).map_err(|_| ResolutionError::ProductDomain)?;
    if product_runtime.product_record.content_digest.to_bytes() != market.product_record
        || product_runtime
            .result_domain_record
            .content_digest
            .to_bytes()
            != hash(&domain_data).to_bytes()
        || records.material.product_record_digest().to_bytes() != market.product_record
        || records.source.domain_id().to_bytes() != result_domain.coordinate_domain_id().to_bytes()
        || records.statistic.result_unit_id().to_bytes()
            != result_domain.result_unit_id().to_bytes()
        || product_runtime.coordinate_domain_id.to_bytes()
            != result_domain.coordinate_domain_id().to_bytes()
        || product_runtime.result_unit_id.to_bytes() != result_domain.result_unit_id().to_bytes()
    {
        return Err(ResolutionError::ProductDomain.into());
    }

    let outcome_count = result_domain
        .outcome_count()
        .map_err(|_| ResolutionError::ProductDomain)?;
    if product_runtime.outcome_count != outcome_count {
        return Err(ResolutionError::ProductDomain.into());
    }
    let keys = Box::new(SettlementKeysV1 {
        resolver: resolver.key.to_bytes(),
        market: market_account.key.to_bytes(),
        source_state: source_state_account.key.to_bytes(),
        certificate: certificate_account.key.to_bytes(),
        receipt: receipt_account.key.to_bytes(),
        head: head_account.key.to_bytes(),
        candidate: candidate_account.key.to_bytes(),
        product_record: market.product_record,
    });
    let decision = boxed_settlement_decision(
        request,
        &keys,
        &source_state,
        &records,
        &sealed,
        result_domain,
        outcome_count,
        &SettlementClockV1 {
            slot: clock.slot,
            unix_timestamp: clock.unix_timestamp,
        },
    )?;
    let certificate = boxed_success_certificate(request, &keys, &records, &sealed, &decision)?;
    let receipt = boxed_sponsored_receipt(program_id, request, &keys, &sealed, &decision)?;
    drop(domain_data);
    commit_settlement(
        program_id,
        request.terminal_sequence,
        resolver,
        source_state_account,
        certificate_account,
        receipt_account,
        system,
        &rent,
        receipt.bump,
        &decision.source,
        &certificate,
        &receipt.bytes,
    )
}

fn authenticate_settle_frame(accounts: &[AccountInfo<'_>]) -> ProgramResult {
    if accounts.len() != SPONSORED_PUSH_SETTLE_ACCOUNT_COUNT_V1 {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, info) in accounts.iter().enumerate() {
        if info.is_signer != (index == 0)
            || info.is_writable != matches!(index, 0 | 6 | 7 | 8)
            || info.executable != matches!(index, 2 | 31)
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == info.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    Ok(())
}

fn primary_deadline(window: WindowSpecV1) -> Result<i64, ProgramError> {
    window
        .end_unix_seconds()
        .checked_add(i64::from(window.max_age_seconds()))
        .ok_or_else(|| ResolutionError::Arithmetic.into())
}

struct SealedSponsoredCandidateV1 {
    candidate: SponsoredPushCandidateV1,
    update: FullPriceUpdateV2,
    candidate_digest: [u8; 32],
    sponsored_release_id: [u8; 32],
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn boxed_sealed_candidate(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    source_state: &AccountInfo<'_>,
    head_account: &AccountInfo<'_>,
    candidate_account: &AccountInfo<'_>,
    generation: u64,
    sponsored: PythSponsoredPushReleaseV1,
    records: &SponsoredSourceRecordsV1,
    clock: &Clock,
) -> Result<Box<SealedSponsoredCandidateV1>, ProgramError> {
    if head_account.owner != program_id
        || head_account.executable
        || head_account.data_len() != SPONSORED_PUSH_HEAD_BYTES_V1
        || !funded_rent_persists_v1(head_account.lamports())
        || candidate_account.owner != program_id
        || candidate_account.executable
        || candidate_account.data_len() != SPONSORED_PUSH_CANDIDATE_BYTES_V1
        || !funded_rent_persists_v1(candidate_account.lamports())
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    let head_data = head_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    let head =
        SponsoredPushHeadV1::decode(&head_data).map_err(|_| ResolutionError::SponsoredPush)?;
    let candidate_data = candidate_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    let candidate = SponsoredPushCandidateV1::decode(&candidate_data)
        .map_err(|_| ResolutionError::SponsoredPush)?;
    let candidate_digest = hash(&candidate_data).to_bytes();
    let sponsored_release_id = hash(&sponsored.to_bytes()).to_bytes();
    let generation_le = generation.to_le_bytes();
    let (expected_head, bump) = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
            market.key.as_ref(),
            &generation_le,
            &sponsored_release_id,
        ],
        program_id,
    );
    candidate_bump(
        program_id,
        market.key,
        generation,
        &sponsored_release_id,
        &Pubkey::new_from_array(candidate.price_account),
        candidate.publish_time,
        candidate.posted_slot,
        &candidate.update_digest,
        candidate_account.key,
    )?;
    let update = FullPriceUpdateV2::parse(&candidate.update_bytes)
        .map_err(|_| ResolutionError::ProviderObservation)?;
    if head_account.key != &expected_head
        || head.bump != bump
        || head.market != market.key.to_bytes()
        || head.source_state != source_state.key.to_bytes()
        || head.provider_release != records.provider_release_id.to_bytes()
        || head.sponsored_release != sponsored_release_id
        || head.price_account != sponsored.price_account()
        || head.generation != generation
        || head.best_candidate != candidate_account.key.to_bytes()
        || head.best_update_digest != candidate.update_digest
        || head.selection() != candidate.selection()
        || candidate.market != market.key.to_bytes()
        || candidate.source_state != source_state.key.to_bytes()
        || candidate.provider_release != records.provider_release_id.to_bytes()
        || candidate.sponsored_release != sponsored_release_id
        || candidate.price_account != sponsored.price_account()
        || candidate.generation != generation
        || candidate.snapshot_slot > clock.slot
        || candidate.snapshot_unix_seconds > clock.unix_timestamp
        || update.write_authority() != candidate.price_account
        || update.feed_id() != sponsored.feed_id()
        || update.publish_time() != candidate.publish_time
        || update.posted_slot() != candidate.posted_slot
        || update.posted_slot() > candidate.snapshot_slot
        || update.prev_publish_time() > update.publish_time()
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    drop(candidate_data);
    drop(head_data);
    Ok(Box::new(SealedSponsoredCandidateV1 {
        candidate,
        update,
        candidate_digest,
        sponsored_release_id,
    }))
}

#[inline(never)]
fn sponsored_normalized_observation(
    records: &SponsoredSourceRecordsV1,
    update: &FullPriceUpdateV2,
    provider_evidence: [u8; 32],
    capture_unix_seconds: i64,
) -> Result<i128, ProgramError> {
    PythProviderAdapterObligationV2::from_authenticated_sponsored_push_records(
        records.material,
        records.material.product_record_digest(),
        records.source_spec_id,
        records.source,
        records.provider_release_id,
        records.provider_release,
        records.adapter_config_id,
        records.adapter,
        records.window_spec_id,
        records.window,
        records.statistic_spec_id,
        records.statistic,
        SourceContentId::new(SOURCE_FAILURE_POLICY_RELEASE_ID_V2)
            .map_err(|_| ResolutionError::SourceMaterial)?,
    )
    .and_then(|obligation| {
        obligation.normalize_authenticated_update(
            SourceContentId::new(provider_evidence)?,
            update.feed_id(),
            update.price(),
            update.confidence(),
            update.exponent(),
            update.publish_time(),
            capture_unix_seconds,
        )
    })
    .map(|normalized| normalized.atoms())
    .map_err(|error| match error {
        dclutch_source::Error::InvalidObservationSchedule => {
            ResolutionError::ProviderWindow.into()
        }
        dclutch_source::Error::InvalidPublicationTime => {
            ResolutionError::ProviderFreshness.into()
        }
        dclutch_source::Error::InvalidPythObservation => {
            ResolutionError::ProviderConfiguration.into()
        }
        _ => ResolutionError::ProviderObservation.into(),
    })
}

struct SettlementKeysV1 {
    resolver: [u8; 32],
    market: [u8; 32],
    source_state: [u8; 32],
    certificate: [u8; 32],
    receipt: [u8; 32],
    head: [u8; 32],
    candidate: [u8; 32],
    product_record: [u8; 32],
}

struct SettlementClockV1 {
    slot: u64,
    unix_timestamp: i64,
}

/// One authenticated, pure decision shared by every terminal output.
///
/// Keeping this fact compact and boxed makes the Source transition the single
/// semantic owner while preventing certificate and receipt construction from
/// independently recomputing the selector or normalized result.
struct SponsoredSettlementDecisionV1 {
    source: [u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    provider_evidence: [u8; 32],
    normalized: i128,
    selector: u32,
    outcome_count: u32,
    consumed_slot: u64,
}

/// The separately encoded sponsored receipt and its authenticated PDA bump.
struct EncodedSponsoredReceiptV1 {
    bytes: [u8; SPONSORED_PUSH_RECEIPT_BYTES_V1],
    bump: u8,
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn boxed_settlement_decision(
    request: SponsoredPushInstructionV1,
    keys: &SettlementKeysV1,
    source_state: &SourceResolutionStateV2,
    records: &SponsoredSourceRecordsV1,
    sealed: &SealedSponsoredCandidateV1,
    result_domain: ResultDomainV2<'_>,
    outcome_count: u32,
    clock: &SettlementClockV1,
) -> Result<Box<SponsoredSettlementDecisionV1>, ProgramError> {
    let provider_evidence = hashv(&[
        dclutch_source::resolution::SPONSORED_PUSH_EVIDENCE_DOMAIN_V1,
        &[0],
        &keys.candidate,
        &sealed.candidate_digest,
        &keys.head,
        &sealed.sponsored_release_id,
    ])
    .to_bytes();
    let normalized = sponsored_normalized_observation(
        records,
        &sealed.update,
        provider_evidence,
        sealed.candidate.snapshot_unix_seconds,
    )?;
    let mut next_source = Box::new(*source_state);
    let transition = next_source
        .resolve_primary_from_authenticated_domain(
            records.material_id,
            records.material,
            records.material.product_record_digest(),
            result_domain,
            SourceContentId::new(provider_evidence)
                .map_err(|_| ResolutionError::ProviderObservation)?,
            normalized,
            1,
            records.statistic.source_scale_exponent(),
            request.generation,
            clock.unix_timestamp,
            request.terminal_sequence,
        )
        .map_err(|_| ResolutionError::Transition)?;
    if transition.selector() >= result_domain.failure_selector()
        || transition.outcome_count() != outcome_count
    {
        return Err(ResolutionError::ProductDomain.into());
    }
    Ok(Box::new(SponsoredSettlementDecisionV1 {
        source: next_source.to_bytes(),
        provider_evidence,
        normalized,
        selector: transition.selector(),
        outcome_count,
        consumed_slot: clock.slot,
    }))
}

#[inline(never)]
fn boxed_success_certificate(
    request: SponsoredPushInstructionV1,
    keys: &SettlementKeysV1,
    records: &SponsoredSourceRecordsV1,
    sealed: &SealedSponsoredCandidateV1,
    decision: &SponsoredSettlementDecisionV1,
) -> Result<Box<[u8; RESOLUTION_CERTIFICATE_BYTES_V2]>, ProgramError> {
    let observed_at =
        u64::try_from(sealed.candidate.publish_time).map_err(|_| ResolutionError::Arithmetic)?;
    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: keys.market,
        route: sealed.sponsored_release_id,
        source_material: records.material_id.to_bytes(),
        product_record_digest: keys.product_record,
        provider_evidence: decision.provider_evidence,
        funding_allocation: [0; 32],
        receipt_account: keys.certificate,
        generation: request.generation,
        attempt_index: 0,
        schedule_index: 0,
        selector: decision.selector,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: decision.normalized,
        result_denominator: 1,
        observed_at,
    };
    certificate
        .validate_terminal_product(keys.product_record, decision.outcome_count)
        .map_err(|_| ResolutionError::Transition)?;
    Ok(Box::new(
        certificate
            .to_bytes()
            .map_err(|_| ResolutionError::Transition)?,
    ))
}

#[inline(never)]
fn boxed_sponsored_receipt(
    program_id: &Pubkey,
    request: SponsoredPushInstructionV1,
    keys: &SettlementKeysV1,
    sealed: &SealedSponsoredCandidateV1,
    decision: &SponsoredSettlementDecisionV1,
) -> Result<Box<EncodedSponsoredReceiptV1>, ProgramError> {
    let sequence = request.terminal_sequence.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1,
            &sealed.candidate.source_state,
            &sequence,
        ],
        program_id,
    );
    if keys.receipt != expected.to_bytes() || keys.source_state != sealed.candidate.source_state {
        return Err(ResolutionError::SponsoredPush.into());
    }
    let receipt = SponsoredPushReceiptV1 {
        market: sealed.candidate.market,
        source_state: sealed.candidate.source_state,
        provider_release: sealed.candidate.provider_release,
        sponsored_release: sealed.sponsored_release_id,
        price_account: sealed.candidate.price_account,
        head: keys.head,
        candidate: keys.candidate,
        candidate_digest: sealed.candidate_digest,
        provider_evidence: decision.provider_evidence,
        certificate: keys.certificate,
        resolver: keys.resolver,
        generation: request.generation,
        terminal_sequence: request.terminal_sequence,
        snapshot_slot: sealed.candidate.snapshot_slot,
        snapshot_unix_seconds: sealed.candidate.snapshot_unix_seconds,
        publish_time: sealed.candidate.publish_time,
        posted_slot: sealed.candidate.posted_slot,
        consumed_slot: decision.consumed_slot,
        selector: decision.selector,
        outcome_count: decision.outcome_count,
        result_numerator: decision.normalized,
        result_denominator: 1,
        bump,
    };
    receipt
        .validate()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    Ok(Box::new(EncodedSponsoredReceiptV1 {
        bytes: receipt
            .to_bytes()
            .map_err(|_| ResolutionError::SponsoredPush)?,
        bump,
    }))
}

#[allow(clippy::too_many_arguments)]
fn commit_settlement<'info>(
    program_id: &Pubkey,
    terminal_sequence: u64,
    resolver: &AccountInfo<'info>,
    source_state: &AccountInfo<'info>,
    certificate: &AccountInfo<'info>,
    receipt: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    rent: &Rent,
    receipt_bump: u8,
    next_source: &[u8; SOURCE_RESOLUTION_STATE_BYTES_V2],
    next_certificate: &[u8; RESOLUTION_CERTIFICATE_BYTES_V2],
    next_receipt: &[u8; SPONSORED_PUSH_RECEIPT_BYTES_V1],
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
    let minimum = rent.minimum_balance(SPONSORED_PUSH_RECEIPT_BYTES_V1);
    let sequence = terminal_sequence.to_le_bytes();
    let bump = [receipt_bump];
    create_prefunded_pda(
        resolver,
        receipt,
        system,
        minimum,
        SPONSORED_PUSH_RECEIPT_BYTES_V1,
        program_id,
        &[
            SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1,
            source_state.key.as_ref(),
            &sequence,
            &bump,
        ],
    )?;
    let mut source_data = source_state
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut certificate_data = certificate
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    let mut receipt_data = receipt
        .try_borrow_mut_data()
        .map_err(|_| ResolutionError::OutputState)?;
    if source_data.len() != next_source.len()
        || certificate_data.len() != next_certificate.len()
        || receipt_data.len() != next_receipt.len()
        || certificate_data.iter().any(|byte| *byte != 0)
        || receipt_data.iter().any(|byte| *byte != 0)
    {
        return Err(ResolutionError::OutputState.into());
    }
    source_data.copy_from_slice(next_source);
    certificate_data.copy_from_slice(next_certificate);
    receipt_data.copy_from_slice(next_receipt);
    Ok(())
}

fn authenticate_cleanup_frame(accounts: &[AccountInfo<'_>], expected: usize) -> ProgramResult {
    if accounts.len() != expected {
        return Err(ResolutionError::AccountFrame.into());
    }
    for (index, info) in accounts.iter().enumerate() {
        if info.is_signer
            || info.is_writable != matches!(index, 2 | 3)
            || info.executable
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == info.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }
    Ok(())
}

fn terminal_source_for_cleanup(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    state: &AccountInfo<'_>,
    generation: u64,
) -> Result<SourceResolutionStateV2, ProgramError> {
    authenticate_source_state_account(program_id, state, market)?;
    let source = boxed_source_state(state)?;
    if source.generation() != generation
        || source.market() != market.key.to_bytes()
        || !matches!(
            source.phase(),
            SourceResolutionPhaseV1::Resolved
                | SourceResolutionPhaseV1::FailureCommitted
                | SourceResolutionPhaseV1::Retired
        )
    {
        return Err(ResolutionError::Transition.into());
    }
    Ok(*source)
}

fn process_close_candidate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SponsoredPushInstructionV1,
) -> ProgramResult {
    authenticate_cleanup_frame(accounts, SPONSORED_PUSH_CLOSE_CANDIDATE_ACCOUNT_COUNT_V1)?;
    let market = account(accounts, 0)?;
    let state = account(accounts, 1)?;
    let candidate_account = account(accounts, 2)?;
    let refund = account(accounts, 3)?;
    terminal_source_for_cleanup(program_id, market, state, request.generation)?;
    if candidate_account.owner != program_id || candidate_account.executable {
        return Err(ResolutionError::SponsoredPush.into());
    }
    let data = candidate_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    let candidate =
        SponsoredPushCandidateV1::decode(&data).map_err(|_| ResolutionError::SponsoredPush)?;
    if candidate.market != market.key.to_bytes()
        || candidate.source_state != state.key.to_bytes()
        || candidate.generation != request.generation
        || candidate.refund_recipient != refund.key.to_bytes()
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    candidate_bump(
        program_id,
        market.key,
        request.generation,
        &candidate.sponsored_release,
        &Pubkey::new_from_array(candidate.price_account),
        candidate.publish_time,
        candidate.posted_slot,
        &candidate.update_digest,
        candidate_account.key,
    )?;
    drop(data);
    close_to_beneficiary(candidate_account, refund)
}

fn process_close_head(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SponsoredPushInstructionV1,
) -> ProgramResult {
    authenticate_cleanup_frame(accounts, SPONSORED_PUSH_CLOSE_HEAD_ACCOUNT_COUNT_V1)?;
    let market = account(accounts, 0)?;
    let state = account(accounts, 1)?;
    let head_account = account(accounts, 2)?;
    let refund = account(accounts, 3)?;
    terminal_source_for_cleanup(program_id, market, state, request.generation)?;
    if head_account.owner != program_id || head_account.executable {
        return Err(ResolutionError::SponsoredPush.into());
    }
    let data = head_account
        .try_borrow_data()
        .map_err(|_| ResolutionError::SponsoredPush)?;
    let head = SponsoredPushHeadV1::decode(&data).map_err(|_| ResolutionError::SponsoredPush)?;
    let generation = request.generation.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
            market.key.as_ref(),
            &generation,
            &head.sponsored_release,
        ],
        program_id,
    );
    if head_account.key != &expected
        || head.bump != bump
        || head.market != market.key.to_bytes()
        || head.source_state != state.key.to_bytes()
        || head.generation != request.generation
        || head.head_refund_recipient != refund.key.to_bytes()
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    drop(data);
    close_to_beneficiary(head_account, refund)
}

fn process_commit_failure(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SponsoredPushInstructionV1,
) -> ProgramResult {
    if accounts.len() != SPONSORED_PUSH_COMMIT_FAILURE_ACCOUNT_COUNT_V1 {
        return Err(ResolutionError::AccountFrame.into());
    }
    validate_frame(
        RelayFrameKindV1::CommitDeadlineFailure,
        accounts.get(..22).ok_or(ResolutionError::AccountFrame)?,
    )?;
    for (index, info) in accounts.iter().enumerate() {
        if (index >= 22 && (info.is_signer || info.is_writable || info.executable))
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == info.key)
        {
            return Err(ResolutionError::AccountFrame.into());
        }
    }

    let market_account = account(accounts, 1)?;
    let source_state_account = account(accounts, 4)?;
    authenticate_source_state_account(program_id, source_state_account, market_account)?;
    let source_state = boxed_source_state(source_state_account)?;
    if !RESOLUTION_PRIMARY_SOURCE_ADMISSIBLE_STATES_V1.admits(source_state.phase())
        || source_state.generation() != request.generation
    {
        return Err(ResolutionError::Transition.into());
    }
    let market = authenticate_market(
        program_id,
        market_account,
        account(accounts, 2)?,
        account(accounts, 3)?,
        request.generation,
        source_state.material_id().to_bytes(),
    )?;

    let material_data = borrow_record(
        accounts,
        6,
        &market.registry_program,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source_state.material_id().to_bytes(),
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material =
        SourceMaterialV3::decode(&material_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if material.product_record_digest().to_bytes() != market.product_record {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let source_spec_id = material.primary_source_spec();
    drop(material_data);

    let source_data = borrow_record(
        accounts,
        23,
        &market.registry_program,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id.to_bytes(),
        SOURCE_SPEC_BYTES,
    )?;
    let source = SourceSpecV1::decode(&source_data).map_err(|_| ResolutionError::SourceMaterial)?;
    if source.access_profile() != SourceAccessProfile::PythSponsoredPushSnapshot {
        return Err(ResolutionError::SourceMaterial.into());
    }
    let provider_release_id = source.provider_release_id();
    drop(source_data);

    let provider_data = borrow_record(
        accounts,
        25,
        &market.registry_program,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        provider_release_id.to_bytes(),
        PROVIDER_RELEASE_BYTES,
    )?;
    let provider =
        ProviderReleaseV1::decode(&provider_data).map_err(|_| ResolutionError::ProviderRelease)?;
    let sponsored_release_id = provider.provider_deployment_release_id().to_bytes();
    drop(provider_data);

    let sponsored_data = borrow_record(
        accounts,
        27,
        &market.registry_program,
        PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
        sponsored_release_id,
        PYTH_SPONSORED_PUSH_RELEASE_V1_ENCODED_LEN,
    )?;
    let sponsored = PythSponsoredPushReleaseV1::decode(&sponsored_data)
        .map_err(|_| ResolutionError::ProviderRelease)?;
    if hash(&sponsored_data).to_bytes() != sponsored_release_id
        || provider.provider_family_id().to_bytes() != sponsored.provider_family_id()
        || provider.adapter_release_id().to_bytes() != sponsored.adapter_id()
        || provider.decoding_rules_id().to_bytes() != sponsored.price_update_codec_id()
        || provider.transport_profile_id().to_bytes() != sponsored.transport_profile_id()
        || sponsored.cluster_id() != DEVNET_CLUSTER_ID_V1
    {
        return Err(ResolutionError::ProviderRelease.into());
    }
    drop(sponsored_data);

    let head = account(accounts, 22)?;
    let generation = request.generation.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
            market_account.key.as_ref(),
            &generation,
            &sponsored_release_id,
        ],
        program_id,
    )
    .0;
    if head.key != &expected
        || head.owner != &system_program::ID
        || head.executable
        || !head
            .try_data_is_empty()
            .map_err(|_| ResolutionError::SponsoredPush)?
    {
        return Err(ResolutionError::SponsoredPush.into());
    }
    process_deadline_failure_coordinates(
        program_id,
        accounts.get(..22).ok_or(ResolutionError::AccountFrame)?,
        request.generation,
        request.terminal_sequence,
    )
}
