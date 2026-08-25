use std::{boxed::Box, vec, vec::Vec};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingDerivationV1, CapabilityManifestV1, CompartmentFundingV1,
    FUNDING_STATE_BYTES, FundingAmountsV1, FundingCustodyObservationV1, FundingQuoteV1,
    FundingStateV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileId,
    product::{InstanceV1, InstanceV1Input},
    result_domain::{FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, FiniteResultDomainV1},
};
use dclutch_pyth_svm::{
    FULL_PRICE_UPDATE_V2_LEN, PYTH_RELEASE_V1_ENCODED_LEN, PythReleaseV1, PythReleaseV1Input,
    RECEIVER_CONFIG_V2_DISCRIMINATOR, RECEIVER_CONFIG_V2_LEN,
    price_update::PRICE_UPDATE_V2_DISCRIMINATOR,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, EXECUTION_AUTHORITY_MANIFEST_SCHEMA_ID_V1,
    ExecutionAuthorityManifestV1, ExecutionReleaseActivationInputsV1,
    activate_execution_release_set_v1,
};
use dclutch_registry_svm::LOADER_V3_PROGRAMDATA_METADATA_BYTES;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};
use dclutch_resolution_codec::{
    ACCEPT_PYTH_REQUEST_BYTES, AcceptPythRequestV1, FUNDED_TRANSITION_REQUEST_BYTES,
    FundedTransitionActionV3, FundedTransitionRequestV3, PRIMARY_CERTIFICATE_SEQUENCE_V3,
    PYTH_RELEASE_RECORD_SCHEMA_ID_V1, RESOLUTION_CERTIFICATE_BYTES,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V4,
    ResolutionCertificateKindV1, ResolutionCertificateV1,
};
use dclutch_source_contract::{
    CapacityEnvelope, ContentId as SourceContentId, PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
    ProviderReleaseV1, PythAdapterConfigV1, RecoveryAttemptV1, RecoveryMaterialSlotV1,
    RecoveryPolicyV1, ResolutionPolicyV1, RoundingBoundary, SOURCE_MATERIAL_BYTES,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
    SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialInputV1,
    SourceRecoveryMaterialInputV1, SourceResolutionPhaseV1, SourceResolutionStateV1, SourceSpecV1,
    StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1, encode_source_material_into_v1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use super::{ResolutionError, process_instruction};

const GENERATION: u64 = 1;
const NOW: i64 = 100;
const SLOT: u64 = 77;
const PRICE: i64 = 5;
const FEED: [u8; 32] = [0x2a; 32];

struct Fixture {
    program_id: Pubkey,
    accounts: [AccountInfo<'static>; 23],
    request: [u8; ACCEPT_PYTH_REQUEST_BYTES],
    capability_manifest: AccountInfo<'static>,
    capability_manifest_staging: AccountInfo<'static>,
    recovery_allocation_id: [u8; 32],
    exhaust_allocation_id: [u8; 32],
    material_id: [u8; 32],
    result_domain_id: [u8; 32],
}

fn key(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn core_id(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero Core content identity")
}

fn source_id(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("nonzero Source content identity")
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero Product content identity")
}

fn account(
    key: Pubkey,
    writable: bool,
    lamports: u64,
    data: Vec<u8>,
    owner: Pubkey,
    executable: bool,
) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        false,
        writable,
        Box::leak(Box::new(lamports)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(owner)),
        executable,
    )
}

fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0; 36];
    bytes
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    bytes
        .get_mut(4..36)
        .expect("ProgramData link")
        .copy_from_slice(programdata.as_ref());
    bytes
}

fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; LOADER_V3_PROGRAMDATA_METADATA_BYTES + elf.len()];
    bytes
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    bytes
        .get_mut(LOADER_V3_PROGRAMDATA_METADATA_BYTES..)
        .expect("ELF")
        .copy_from_slice(elf);
    bytes
}

fn receiver_config(router: Pubkey) -> Vec<u8> {
    let mut bytes = vec![0; RECEIVER_CONFIG_V2_LEN];
    bytes
        .get_mut(..8)
        .expect("discriminator")
        .copy_from_slice(&RECEIVER_CONFIG_V2_DISCRIMINATOR);
    bytes
        .get_mut(8..40)
        .expect("governance")
        .copy_from_slice(&[0x31; 32]);
    bytes
        .get_mut(41..73)
        .expect("router")
        .copy_from_slice(router.as_ref());
    bytes
        .get_mut(77..85)
        .expect("fee")
        .copy_from_slice(&1_u64.to_le_bytes());
    *bytes.get_mut(85).expect("minimum signatures") = 1;
    bytes
}

fn full_price_update(confidence: u64) -> Vec<u8> {
    let mut bytes = vec![0; FULL_PRICE_UPDATE_V2_LEN];
    bytes
        .get_mut(..8)
        .expect("discriminator")
        .copy_from_slice(&PRICE_UPDATE_V2_DISCRIMINATOR);
    bytes
        .get_mut(8..40)
        .expect("write authority")
        .copy_from_slice(&[0x41; 32]);
    *bytes.get_mut(40).expect("verification tag") = 1;
    bytes.get_mut(41..73).expect("feed").copy_from_slice(&FEED);
    bytes
        .get_mut(73..81)
        .expect("price")
        .copy_from_slice(&PRICE.to_le_bytes());
    bytes
        .get_mut(81..89)
        .expect("confidence")
        .copy_from_slice(&confidence.to_le_bytes());
    bytes
        .get_mut(89..93)
        .expect("exponent")
        .copy_from_slice(&(-8_i32).to_le_bytes());
    bytes
        .get_mut(93..101)
        .expect("publish time")
        .copy_from_slice(&NOW.to_le_bytes());
    bytes
        .get_mut(101..109)
        .expect("previous publish time")
        .copy_from_slice(&(NOW - 1).to_le_bytes());
    bytes
        .get_mut(109..117)
        .expect("EMA price")
        .copy_from_slice(&PRICE.to_le_bytes());
    bytes
        .get_mut(125..133)
        .expect("posted slot")
        .copy_from_slice(&SLOT.to_le_bytes());
    bytes
}

fn record_pair(
    core_program: Pubkey,
    rent: &Rent,
    schema: [u8; 32],
    digest: [u8; 32],
    data: Vec<u8>,
) -> (AccountInfo<'static>, AccountInfo<'static>) {
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &core_program).0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &core_program,
    )
    .0;
    (
        account(
            raw,
            false,
            rent.minimum_balance(data.len()),
            data,
            core_program,
            false,
        ),
        account(staging, false, 0, Vec::new(), system_program::ID, false),
    )
}

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("nonzero program")
}

fn artifact(
    program: Pubkey,
    programdata: Pubkey,
    semantic_release_id: [u8; 32],
    elf: &[u8],
    slot: u64,
) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata.to_bytes(),
        core_id(semantic_release_id),
        hash(elf).to_bytes(),
        slot,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("valid immutable artifact")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact identity")
}

fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
        release.program().to_bytes(),
        release.loader_program().to_bytes(),
        true,
        release.programdata(),
        release.loader_program().to_bytes(),
        false,
        release.programdata(),
        release.loader_program().to_bytes(),
        release.deployment_slot(),
        release.elf_digest(),
        None,
    )
    .expect("complete deployment observation");
    ArtifactActivationInputV1::new(artifact_id(release), release, observation)
}

fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
}

fn fixture() -> Fixture {
    let rent = Rent::default();
    let core_program = key(0x51);
    let program_id = key(0x52);
    let resolution_programdata =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0;
    let resolution_elf = b"resolution-elf-v1";

    let core_release = artifact(core_program, key(0x61), [0x71; 32], b"core-elf", 70);
    let claims_release = artifact(key(0x53), key(0x62), [0x72; 32], b"claims-elf", 71);
    let trading_release = artifact(key(0x54), key(0x63), [0x73; 32], b"trading-elf", 72);
    let resolution_release = artifact(
        program_id,
        resolution_programdata,
        RESOLUTION_CONTROLLER_RELEASE_ID_V4,
        resolution_elf,
        73,
    );
    let custody_release = artifact(key(0x55), key(0x64), [0x75; 32], b"custody-elf", 74);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core_release),
        binding(claims_release),
        binding(trading_release),
        binding(resolution_release),
        binding(custody_release),
    )
    .expect("valid release set");
    let release_set_id = core_id(hash(&release_set.to_bytes()).to_bytes());
    let activation_inputs = ExecutionReleaseActivationInputsV1::new(
        activation_input(core_release),
        activation_input(claims_release),
        activation_input(trading_release),
        activation_input(resolution_release),
        activation_input(custody_release),
    );
    let activated = activate_execution_release_set_v1(
        program_identity(core_program),
        release_set_id,
        &release_set,
        &activation_inputs,
    )
    .expect("activated release set");
    let receiver_program = key(0x56);
    let receiver_programdata =
        Pubkey::find_program_address(&[receiver_program.as_ref()], &bpf_loader_upgradeable::ID).0;
    let router_program = key(0x57);
    let router_programdata =
        Pubkey::find_program_address(&[router_program.as_ref()], &bpf_loader_upgradeable::ID).0;
    let receiver_config_key = key(0x58);
    let config_bytes = receiver_config(router_program);
    let price_codec_id = [0x91; 32];
    let adapter_id = [0x92; 32];
    let pyth_release = PythReleaseV1::new(PythReleaseV1Input {
        cluster_id: [0x90; 32],
        receiver_program: receiver_program.to_bytes(),
        receiver_programdata: receiver_programdata.to_bytes(),
        receiver_config: receiver_config_key.to_bytes(),
        router_program: router_program.to_bytes(),
        router_programdata: router_programdata.to_bytes(),
        config_digest: hash(&config_bytes).to_bytes(),
        receiver_abi_id: [0x93; 32],
        router_abi_id: [0x94; 32],
        price_update_codec_id: price_codec_id,
        adapter_id,
        receiver_deployment_slot: 75,
        router_deployment_slot: 76,
        guardian_set_count: 19,
        required_guardian_count: 10,
        upstream_commit: [0x95; 20],
        sdk_crate_digest: [0x96; 32],
        activation_time: 90,
    })
    .expect("Pyth release");
    let pyth_release_bytes = pyth_release.to_bytes();
    let pyth_release_id = hash(&pyth_release_bytes).to_bytes();

    let coordinate_id = [0xa1; 32];
    let unit_id = [0xa2; 32];
    let domain = FiniteResultDomainV1::new(product_id(coordinate_id), product_id(unit_id), 1, &[0])
        .expect("Product result domain");
    let domain_bytes = domain.to_bytes();
    let domain_id =
        hashv(&[FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &[0], &domain_bytes]).to_bytes();
    assert_eq!(domain.outcome_count(), 3);

    let product_instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([0xa3; 32]),
        occurrence_id: product_id([0xa4; 32]),
        claim_basis_id: product_id([0xa5; 32]),
        result_domain_id: product_id(domain_id),
        capacity_profile_id: CapacityProfileId::new(product_id([0xa6; 32])),
        partition_cell_count: u32::from(domain.outcome_count()),
    })
    .expect("Product instance");
    let product_instance_id = hash(&product_instance.to_bytes()).to_bytes();

    let capacity = SourceCapacityProfileV1::new(
        CapacityEnvelope::Measured,
        1,
        1,
        source_id([0xb1; 32]),
        source_id([0xb2; 32]),
        256,
        0,
    )
    .expect("Source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let provider = ProviderReleaseV1::new(
        source_id([0xb3; 32]),
        source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
        source_id(pyth_release_id),
        source_id(price_codec_id),
        source_id(adapter_id),
    );
    let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
    let adapter_config =
        PythAdapterConfigV1::new(FEED, -8, 100).expect("Pyth adapter configuration");
    let adapter_config_id = source_id(hash(&adapter_config.to_bytes()).to_bytes());
    let source = SourceSpecV1::new(
        source_id(coordinate_id),
        source_id(unit_id),
        provider_id,
        SourceAccessProfile::PythTerminalOneTransaction,
        adapter_config_id,
        capacity_id,
    );
    let source_spec_id = source_id(hash(&source.to_bytes()).to_bytes());
    let window = WindowSpecV1::new(
        source_spec_id,
        WindowKind::Terminal,
        NOW,
        NOW,
        10,
        2,
        source_id([0xb4; 32]),
    )
    .expect("terminal window");
    let window_id = source_id(hash(&window.to_bytes()).to_bytes());
    let statistic = StatisticSpecV1::new(
        source_id(unit_id),
        source_id(unit_id),
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([0xb5; 32]),
        capacity,
    )
    .expect("terminal statistic");
    let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes());
    let source_product_id = source_id(product_instance_id);
    let recovery_allocation_id = [0xd2; 32];
    let recovery_policy = RecoveryPolicyV1::new(
        capacity_id,
        source_product_id,
        [
            Some(RecoveryAttemptV1::new(
                source_spec_id,
                provider_id,
                120,
                source_id(recovery_allocation_id),
            )),
            None,
            None,
            None,
        ],
        1,
        capacity,
    )
    .expect("ordered recovery policy");
    let recovery_policy_id = source_id(hash(&recovery_policy.to_bytes()).to_bytes());
    let recovery_slot = RecoveryMaterialSlotV1::new(
        source_spec_id,
        source,
        provider_id,
        provider,
        adapter_config,
    )
    .expect("recovery material slot");
    let recovery_slots = [recovery_slot];
    let policy = ResolutionPolicyV1::new(
        capacity_id,
        source_product_id,
        source_spec_id,
        window_id,
        statistic_id,
        source_id(domain_id),
        Some(recovery_policy_id),
    );
    let mut material_bytes = [0; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut material_bytes,
        SourceMaterialInputV1 {
            policy: &policy,
            capacity_profile_id: capacity_id,
            capacity_profile: &capacity,
            primary_source_id: source_spec_id,
            primary_source: &source,
            primary_provider_release_id: provider_id,
            primary_provider_release: &provider,
            primary_adapter_config: &adapter_config,
            window_id,
            window: &window,
            statistic_id,
            statistic: &statistic,
            product_instance_id: source_product_id,
            product_instance: &product_instance,
            result_domain: &domain,
            recovery: Some(SourceRecoveryMaterialInputV1 {
                recovery_policy_id,
                recovery_policy: &recovery_policy,
                slots: &recovery_slots,
            }),
        },
    )
    .expect("Source material");
    let material_id = hash(&material_bytes).to_bytes();

    let exact_funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let funding_quote = FundingQuoteV1::new(
        FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(exact_funding_rent).expect("funding rent"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::native_lamports(7).expect("positive bounty"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("typed funding amounts"),
        None,
    )
    .expect("native funding quote");
    let capability_entries = [
        CapabilityEntryV1::new(
            core_id([0xd3; 32]),
            core_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
            core_id(recovery_allocation_id),
            core_id([0xd5; 32]),
            core_id([0xd6; 32]),
            core_id([0xd7; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            funding_quote,
        )
        .expect("recovery funding entry"),
        CapabilityEntryV1::new(
            core_id([0xd4; 32]),
            core_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
            core_id(recovery_policy_id.to_bytes()),
            core_id([0xd5; 32]),
            core_id([0xd6; 32]),
            core_id([0xd7; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            funding_quote,
        )
        .expect("exhaustion funding entry"),
        CapabilityEntryV1::new(
            core_id([0xd8; 32]),
            core_id(RESOLUTION_CONTROLLER_RELEASE_ID_V4),
            core_id(material_id),
            core_id([0xd5; 32]),
            core_id([0xd6; 32]),
            core_id([0xd7; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            funding_quote,
        )
        .expect("failure funding entry"),
    ];
    let mut capability_manifest_bytes = vec![0; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&capability_entries, &mut capability_manifest_bytes)
        .expect("canonical capability manifest");
    let capability_manifest_id = hash(&capability_manifest_bytes).to_bytes();
    let authority =
        ExecutionAuthorityManifestV1::new(core_id(capability_manifest_id), release_set_id)
            .expect("authority manifest");
    let authority_bytes = authority.to_bytes();
    let authority_id = hash(&authority_bytes).to_bytes();

    let market_key = key(0x59);
    let identity = MarketIdentity::new(
        core_id([0xc1; 32]),
        core_id(product_instance_id),
        core_id([0xc2; 32]),
        core_id(material_id),
        core_id(authority_id),
        GENERATION,
    );
    let mut root = MarketRoot::founding(identity, [0xc3; 32]).expect("Market root");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("open Market");
    let market =
        CategoricalMarketV1::<3>::new(root, 0, [0; 3], CategoricalSettlementSummaryV1::empty())
            .expect("categorical Market");
    let mut market_bytes = vec![0; CategoricalMarketV1::<3>::encoded_len().expect("Market width")];
    market.encode(&mut market_bytes).expect("Market encoding");

    let (state_key, bump) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
            market_key.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &program_id,
    );
    let state = SourceResolutionStateV1::fresh(
        market_key.to_bytes(),
        GENERATION,
        source_id(material_id),
        [0xd1; 32],
        bump,
        0,
        0,
    )
    .expect("fresh Source state")
    .state();
    let certificate_kind = [1_u8];
    let certificate_sequence = PRIMARY_CERTIFICATE_SEQUENCE_V3.to_le_bytes();
    let certificate_key = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            state_key.as_ref(),
            &certificate_kind,
            &certificate_sequence,
        ],
        &program_id,
    )
    .0;

    let (authority_raw, authority_staging) = record_pair(
        core_program,
        &rent,
        EXECUTION_AUTHORITY_MANIFEST_SCHEMA_ID_V1,
        authority_id,
        authority_bytes.to_vec(),
    );
    let (material_raw, material_staging) = record_pair(
        core_program,
        &rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        material_id,
        material_bytes.to_vec(),
    );
    let (capability_manifest, capability_manifest_staging) = record_pair(
        core_program,
        &rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        capability_manifest_id,
        capability_manifest_bytes,
    );
    let domain_record_digest = hash(&domain_bytes).to_bytes();
    let (domain_raw, domain_staging) = record_pair(
        core_program,
        &rent,
        dclutch_product_contract::result_domain::FINITE_RESULT_DOMAIN_RELEASE_ID_V1,
        domain_record_digest,
        domain_bytes.to_vec(),
    );
    let (pyth_raw, pyth_staging) = record_pair(
        core_program,
        &rent,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        pyth_release_id,
        pyth_release_bytes.to_vec(),
    );

    let activation_key = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
        &core_program,
    )
    .0;
    let clock = Clock {
        slot: SLOT,
        epoch_start_timestamp: 0,
        epoch: 0,
        leader_schedule_epoch: 0,
        unix_timestamp: NOW,
    };
    let request = AcceptPythRequestV1 {
        expected_generation: GENERATION,
        expected_result_domain_id: domain_id,
        expected_provider_release_id: pyth_release_id,
    }
    .to_bytes()
    .expect("request");
    let accounts = [
        account(
            state_key,
            true,
            1,
            state.to_bytes().to_vec(),
            program_id,
            false,
        ),
        account(
            certificate_key,
            true,
            rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES),
            vec![0; RESOLUTION_CERTIFICATE_BYTES],
            program_id,
            false,
        ),
        account(market_key, false, 1, market_bytes, core_program, false),
        authority_raw,
        authority_staging,
        account(
            activation_key,
            false,
            1,
            activated.to_bytes().to_vec(),
            core_program,
            false,
        ),
        account(
            program_id,
            false,
            1,
            loader_program_bytes(resolution_programdata),
            bpf_loader_upgradeable::ID,
            true,
        ),
        account(
            resolution_programdata,
            false,
            1,
            immutable_programdata_bytes(73, resolution_elf),
            bpf_loader_upgradeable::ID,
            false,
        ),
        material_raw,
        material_staging,
        domain_raw,
        domain_staging,
        pyth_raw,
        pyth_staging,
        account(
            key(0x5a),
            false,
            rent.minimum_balance(FULL_PRICE_UPDATE_V2_LEN),
            full_price_update(0),
            receiver_program,
            false,
        ),
        account(
            receiver_program,
            false,
            1,
            loader_program_bytes(receiver_programdata),
            bpf_loader_upgradeable::ID,
            true,
        ),
        account(
            receiver_programdata,
            false,
            1,
            immutable_programdata_bytes(75, b"receiver-elf"),
            bpf_loader_upgradeable::ID,
            false,
        ),
        account(
            receiver_config_key,
            false,
            rent.minimum_balance(config_bytes.len()),
            config_bytes,
            receiver_program,
            false,
        ),
        account(
            router_program,
            false,
            1,
            loader_program_bytes(router_programdata),
            bpf_loader_upgradeable::ID,
            true,
        ),
        account(
            router_programdata,
            false,
            1,
            immutable_programdata_bytes(76, b"router-elf"),
            bpf_loader_upgradeable::ID,
            false,
        ),
        account(
            sysvar::clock::ID,
            false,
            1,
            bincode::serialize(&clock).expect("Clock bytes"),
            sysvar::ID,
            false,
        ),
        account(
            sysvar::rent::ID,
            false,
            1,
            bincode::serialize(&rent).expect("Rent bytes"),
            sysvar::ID,
            false,
        ),
        account(
            system_program::ID,
            false,
            1,
            Vec::new(),
            native_loader::ID,
            true,
        ),
    ];
    assert_eq!(pyth_release_bytes.len(), PYTH_RELEASE_V1_ENCODED_LEN);
    Fixture {
        program_id,
        accounts,
        request,
        capability_manifest,
        capability_manifest_staging,
        recovery_allocation_id,
        exhaust_allocation_id: recovery_policy_id.to_bytes(),
        material_id,
        result_domain_id: domain_id,
    }
}

struct FundedFixture {
    program_id: Pubkey,
    accounts: [AccountInfo<'static>; 19],
    request: [u8; FUNDED_TRANSITION_REQUEST_BYTES],
    work_paid: u64,
}

fn funded_fixture(action: FundedTransitionActionV3, prepare_prior: bool) -> FundedFixture {
    let base = fixture();
    let rent = Rent::default();
    let now = match action {
        FundedTransitionActionV3::FailNext => 111,
        FundedTransitionActionV3::Exhaust => 121,
        FundedTransitionActionV3::CommitFailure => 122,
    };
    let clock = Clock {
        slot: SLOT + 1,
        epoch_start_timestamp: 0,
        epoch: 0,
        leader_schedule_epoch: 0,
        unix_timestamp: now,
    };

    if prepare_prior {
        let material_data = base.accounts[8]
            .try_borrow_data()
            .expect("Source material data");
        let material = dclutch_source_contract::SourceMaterialViewV1::decode(&material_data)
            .expect("Source material");
        let mut state_data = base.accounts[0]
            .try_borrow_mut_data()
            .expect("Source state data");
        let mut state = SourceResolutionStateV1::decode(&state_data).expect("Source state");
        state
            .fail_next_view(
                source_id(base.material_id),
                material,
                source_id(base.recovery_allocation_id),
                GENERATION,
                111,
            )
            .expect("enter recovery fixture state");
        if action == FundedTransitionActionV3::CommitFailure {
            state
                .exhaust_view(source_id(base.material_id), material, GENERATION, 121)
                .expect("exhaust recovery fixture state");
        }
        state_data.copy_from_slice(&state.to_bytes());
    }

    let manifest_data = base
        .capability_manifest
        .try_borrow_data()
        .expect("capability manifest");
    let manifest = CapabilityManifestV1::decode(&manifest_data).expect("canonical manifest");
    let manifest_id = core_id(hash(&manifest_data).to_bytes());
    let entry_index = match action {
        FundedTransitionActionV3::FailNext => 0,
        FundedTransitionActionV3::Exhaust => 1,
        FundedTransitionActionV3::CommitFailure => 2,
    };
    let exact_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let custody = FundingCustodyObservationV1::native_only(
        exact_rent
            .checked_mul(2)
            .and_then(|value| value.checked_add(7))
            .expect("funding custody amount"),
        exact_rent,
    )
    .expect("funding custody");
    let mut funding =
        FundingStateV1::new(manifest_id, manifest, entry_index, custody).expect("pending funding");
    funding
        .activate(manifest_id, manifest, custody, 1)
        .expect("active funding");
    let funding_derivation = CapabilityFundingDerivationV1::new(
        base.accounts[2].key.to_bytes(),
        GENERATION,
        manifest_id,
        manifest,
        funding,
    )
    .expect("funding derivation");
    let funding_key =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &base.program_id).0;
    let expected_funding_allocation_id = match action {
        FundedTransitionActionV3::FailNext => base.recovery_allocation_id,
        FundedTransitionActionV3::Exhaust => base.exhaust_allocation_id,
        FundedTransitionActionV3::CommitFailure => base.material_id,
    };
    let expected_recovery_index = match action {
        FundedTransitionActionV3::FailNext => 0,
        FundedTransitionActionV3::Exhaust | FundedTransitionActionV3::CommitFailure => 1,
    };
    let certificate_kind = [match action {
        FundedTransitionActionV3::FailNext => 2,
        FundedTransitionActionV3::Exhaust => 3,
        FundedTransitionActionV3::CommitFailure => 4,
    }];
    let certificate_sequence = match action {
        FundedTransitionActionV3::FailNext => 1_u64,
        FundedTransitionActionV3::Exhaust => 2_u64,
        FundedTransitionActionV3::CommitFailure => 3_u64,
    }
    .to_le_bytes();
    let certificate_key = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            base.accounts[0].key.as_ref(),
            &certificate_kind,
            &certificate_sequence,
        ],
        &base.program_id,
    )
    .0;
    let request = FundedTransitionRequestV3 {
        action,
        expected_generation: GENERATION,
        expected_recovery_index,
        expected_result_domain_id: base.result_domain_id,
        expected_funding_allocation_id,
    }
    .to_bytes()
    .expect("funded request");
    drop(manifest_data);

    let accounts = [
        base.accounts[0].clone(),
        account(
            certificate_key,
            true,
            rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES),
            vec![0; RESOLUTION_CERTIFICATE_BYTES],
            base.program_id,
            false,
        ),
        account(
            funding_key,
            true,
            exact_rent + 7,
            funding.to_bytes().to_vec(),
            base.program_id,
            false,
        ),
        account(key(0xe1), true, 5, Vec::new(), system_program::ID, false),
        base.accounts[2].clone(),
        base.accounts[3].clone(),
        base.accounts[4].clone(),
        base.accounts[5].clone(),
        base.accounts[6].clone(),
        base.accounts[7].clone(),
        base.accounts[8].clone(),
        base.accounts[9].clone(),
        base.accounts[10].clone(),
        base.accounts[11].clone(),
        base.capability_manifest.clone(),
        base.capability_manifest_staging.clone(),
        account(
            sysvar::clock::ID,
            false,
            1,
            bincode::serialize(&clock).expect("Clock bytes"),
            sysvar::ID,
            false,
        ),
        base.accounts[21].clone(),
        base.accounts[22].clone(),
    ];
    FundedFixture {
        program_id: base.program_id,
        accounts,
        request,
        work_paid: 7,
    }
}

fn funded_output_snapshot(fixture: &FundedFixture) -> (Vec<u8>, Vec<u8>, Vec<u8>, u64, u64) {
    (
        fixture.accounts[0]
            .try_borrow_data()
            .expect("Source state")
            .to_vec(),
        fixture.accounts[1]
            .try_borrow_data()
            .expect("certificate")
            .to_vec(),
        fixture.accounts[2]
            .try_borrow_data()
            .expect("funding state")
            .to_vec(),
        fixture.accounts[2].lamports(),
        fixture.accounts[3].lamports(),
    )
}

fn assert_funded_refusal_atomic(fixture: &FundedFixture, expected: ResolutionError) {
    let before = funded_output_snapshot(fixture);
    assert_eq!(
        process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request),
        Err(ProgramError::Custom(expected as u32))
    );
    assert_eq!(funded_output_snapshot(fixture), before);
}

fn output_snapshot(fixture: &Fixture) -> (Vec<u8>, Vec<u8>) {
    let state = fixture
        .accounts
        .first()
        .expect("Source state")
        .try_borrow_data()
        .expect("Source state bytes")
        .to_vec();
    let certificate = fixture
        .accounts
        .get(1)
        .expect("certificate")
        .try_borrow_data()
        .expect("certificate bytes")
        .to_vec();
    (state, certificate)
}

fn assert_refusal_atomic(fixture: &Fixture, expected: ResolutionError) {
    let before = output_snapshot(fixture);
    assert_eq!(
        process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request),
        Err(ProgramError::Custom(expected as u32))
    );
    assert_eq!(output_snapshot(fixture), before);
}

#[test]
fn registry_bound_full_pyth_observation_emits_one_compact_certificate() {
    let fixture = fixture();
    process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request)
        .expect("authenticated primary Pyth admission");
    let state_data = fixture
        .accounts
        .first()
        .expect("Source state")
        .try_borrow_data()
        .expect("Source state data");
    let state = SourceResolutionStateV1::decode(&state_data).expect("resolved Source state");
    assert_eq!(state.phase(), SourceResolutionPhaseV1::Resolved);
    let certificate_data = fixture
        .accounts
        .get(1)
        .expect("certificate")
        .try_borrow_data()
        .expect("certificate data");
    let certificate = ResolutionCertificateV1::decode(&certificate_data).expect("certificate");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV1::ResolutionSuccess
    );
    assert_eq!(certificate.generation, GENERATION);
    assert_eq!(certificate.selector, 1);
    assert_eq!(certificate.result_numerator, i128::from(PRICE));
    assert_eq!(certificate.result_denominator, 1);
    assert_eq!(
        certificate.observed_at,
        u64::try_from(NOW).expect("positive time")
    );
    assert_eq!(certificate.funding_allocation, [0; 32]);
    assert_eq!(certificate.work_paid, 0);
    assert_eq!(certificate.funding_remaining, 0);
}

#[test]
fn first_recovery_consumes_exact_bounty_and_emits_ordered_certificate() {
    let fixture = funded_fixture(FundedTransitionActionV3::FailNext, false);
    let funding_before = fixture.accounts[2].lamports();
    let worker_before = fixture.accounts[3].lamports();
    process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request)
        .expect("funded first recovery");

    let state_data = fixture.accounts[0].try_borrow_data().expect("Source state");
    let state = SourceResolutionStateV1::decode(&state_data).expect("recovery state");
    assert_eq!(state.phase(), SourceResolutionPhaseV1::Recovery);
    assert_eq!(state.active_recovery_attempt(), Some(0));

    let funding_data = fixture.accounts[2]
        .try_borrow_data()
        .expect("funding state");
    let funding = FundingStateV1::decode(&funding_data).expect("funding state");
    assert_eq!(funding.remaining().bounty().amount(), 0);
    assert_eq!(funding.released().bounty().amount(), fixture.work_paid);
    assert_eq!(
        fixture.accounts[2].lamports(),
        funding_before - fixture.work_paid
    );
    assert_eq!(
        fixture.accounts[3].lamports(),
        worker_before + fixture.work_paid
    );

    let certificate_data = fixture.accounts[1]
        .try_borrow_data()
        .expect("recovery certificate");
    let certificate =
        ResolutionCertificateV1::decode(&certificate_data).expect("canonical recovery certificate");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV1::RecoveryAdvanced
    );
    assert_eq!(certificate.attempt_index, 1);
    assert_eq!(certificate.selector, 0);
    assert_eq!(certificate.provider_evidence, [0; 32]);
    assert_eq!(certificate.work_paid, fixture.work_paid);
    assert_eq!(certificate.funding_remaining, 0);
    assert_eq!(certificate.observed_at, 111);
}

#[test]
fn last_recovery_exhaustion_consumes_exact_bounty_and_emits_ordered_certificate() {
    let fixture = funded_fixture(FundedTransitionActionV3::Exhaust, true);
    let funding_before = fixture.accounts[2].lamports();
    let worker_before = fixture.accounts[3].lamports();
    process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request)
        .expect("funded recovery exhaustion");

    let state_data = fixture.accounts[0].try_borrow_data().expect("Source state");
    let state = SourceResolutionStateV1::decode(&state_data).expect("exhausted state");
    assert_eq!(state.phase(), SourceResolutionPhaseV1::Exhausted);
    assert_eq!(state.active_recovery_attempt(), None);

    let funding_data = fixture.accounts[2]
        .try_borrow_data()
        .expect("funding state");
    let funding = FundingStateV1::decode(&funding_data).expect("funding state");
    assert_eq!(funding.remaining().bounty().amount(), 0);
    assert_eq!(funding.released().bounty().amount(), fixture.work_paid);
    assert_eq!(
        fixture.accounts[2].lamports(),
        funding_before - fixture.work_paid
    );
    assert_eq!(
        fixture.accounts[3].lamports(),
        worker_before + fixture.work_paid
    );

    let certificate_data = fixture.accounts[1]
        .try_borrow_data()
        .expect("exhaustion certificate");
    let certificate = ResolutionCertificateV1::decode(&certificate_data)
        .expect("canonical exhaustion certificate");
    assert_eq!(certificate.kind, ResolutionCertificateKindV1::Exhausted);
    assert_ne!(certificate.route, [0; 32]);
    assert_eq!(certificate.attempt_index, 1);
    assert_eq!(certificate.selector, 0);
    assert_eq!(certificate.provider_evidence, [0; 32]);
    assert_eq!(certificate.funding_allocation, fixture.request[64..96]);
    assert_eq!(certificate.work_paid, fixture.work_paid);
    assert_eq!(certificate.funding_remaining, 0);
    assert_eq!(certificate.observed_at, 121);
}

#[test]
fn explicit_failure_consumes_exact_bounty_and_uses_product_final_selector() {
    let fixture = funded_fixture(FundedTransitionActionV3::CommitFailure, true);
    process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request)
        .expect("funded explicit failure");
    let state_data = fixture.accounts[0].try_borrow_data().expect("Source state");
    let state = SourceResolutionStateV1::decode(&state_data).expect("failure state");
    assert_eq!(state.phase(), SourceResolutionPhaseV1::FailureCommitted);

    let certificate_data = fixture.accounts[1]
        .try_borrow_data()
        .expect("failure certificate");
    let certificate =
        ResolutionCertificateV1::decode(&certificate_data).expect("canonical failure certificate");
    assert_eq!(
        certificate.kind,
        ResolutionCertificateKindV1::ResolutionFailure
    );
    assert_eq!(certificate.route, [0; 32]);
    assert_eq!(certificate.attempt_index, 1);
    assert_eq!(certificate.selector, 2);
    assert_eq!(certificate.provider_evidence, [0; 32]);
    assert_eq!(certificate.result_denominator, 0);
    assert_eq!(certificate.observed_at, 0);
    assert_eq!(certificate.work_paid, fixture.work_paid);
}

#[test]
fn funded_recovery_hostile_coordinates_and_late_output_refuse_atomically() {
    let mut wrong_allocation = funded_fixture(FundedTransitionActionV3::FailNext, false);
    wrong_allocation.request[64] ^= 1;
    assert_funded_refusal_atomic(&wrong_allocation, ResolutionError::Transition);

    let mut skipped_index = funded_fixture(FundedTransitionActionV3::FailNext, false);
    skipped_index.request[24] = 1;
    assert_funded_refusal_atomic(&skipped_index, ResolutionError::Transition);

    let early = funded_fixture(FundedTransitionActionV3::FailNext, false);
    let early_clock = Clock {
        slot: SLOT + 1,
        epoch_start_timestamp: 0,
        epoch: 0,
        leader_schedule_epoch: 0,
        unix_timestamp: 110,
    };
    early.accounts[16]
        .try_borrow_mut_data()
        .expect("Clock")
        .copy_from_slice(&bincode::serialize(&early_clock).expect("Clock bytes"));
    assert_funded_refusal_atomic(&early, ResolutionError::Transition);

    let occupied = funded_fixture(FundedTransitionActionV3::FailNext, false);
    occupied.accounts[1]
        .try_borrow_mut_data()
        .expect("certificate")
        .first_mut()
        .map(|byte| *byte = 1)
        .expect("certificate byte");
    assert_funded_refusal_atomic(&occupied, ResolutionError::OutputState);
}

#[test]
fn failure_before_explicit_exhaustion_refuses_all_four_outputs_atomically() {
    let fixture = funded_fixture(FundedTransitionActionV3::CommitFailure, false);
    assert_funded_refusal_atomic(&fixture, ResolutionError::Transition);
}

#[test]
fn exhaustion_before_recovery_or_before_deadline_refuses_all_four_outputs_atomically() {
    let missing_recovery = funded_fixture(FundedTransitionActionV3::Exhaust, false);
    assert_funded_refusal_atomic(&missing_recovery, ResolutionError::Transition);

    let early = funded_fixture(FundedTransitionActionV3::Exhaust, true);
    let early_clock = Clock {
        slot: SLOT + 1,
        epoch_start_timestamp: 0,
        epoch: 0,
        leader_schedule_epoch: 0,
        unix_timestamp: 120,
    };
    early.accounts[16]
        .try_borrow_mut_data()
        .expect("Clock")
        .copy_from_slice(&bincode::serialize(&early_clock).expect("Clock bytes"));
    assert_funded_refusal_atomic(&early, ResolutionError::Transition);
}

#[test]
fn hostile_product_identity_refuses_before_any_output_write() {
    let mut fixture = fixture();
    fixture.request[24] ^= 1;
    assert_refusal_atomic(&fixture, ResolutionError::ProductDomain);
}

#[test]
fn excessive_provider_confidence_refuses_without_partial_state() {
    let fixture = fixture();
    {
        let mut update = fixture
            .accounts
            .get(14)
            .expect("Pyth update")
            .try_borrow_mut_data()
            .expect("Pyth update bytes");
        update
            .get_mut(81..89)
            .expect("confidence")
            .copy_from_slice(&u64::MAX.to_le_bytes());
    }
    assert_refusal_atomic(&fixture, ResolutionError::ProviderObservation);
}

#[test]
fn occupied_certificate_refuses_at_commit_and_preserves_both_outputs() {
    let fixture = fixture();
    {
        let mut certificate = fixture
            .accounts
            .get(1)
            .expect("certificate")
            .try_borrow_mut_data()
            .expect("certificate bytes");
        *certificate.first_mut().expect("first byte") = 1;
    }
    assert_refusal_atomic(&fixture, ResolutionError::OutputState);
}
