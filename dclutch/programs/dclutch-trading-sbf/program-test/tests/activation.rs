//! ProgramTest evidence for the common data-defined Trading activation outer.

use std::vec::Vec;

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_ARTIFACT_PROFILE_V1, ACCOUNT_PROFILE_HEADER_BYTES_V1, ACCOUNT_PROFILE_MAGIC_V1,
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, ACCOUNT_PROFILE_SCHEMA_VERSION_V1,
    EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
    EFFECT_PERMISSION_WRITE_DATA,
};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, CompartmentFundingV1, ContentId, FUNDING_STATE_BYTES, FundingAmountsV1,
    FundingCustodyObservationV1, FundingQuoteV1, FundingStateV1, FundingStatus,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
    CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
    CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1, CAPABILITY_PROGRAM_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_PROFILE_V2, CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
    CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CapabilityProgramV1, CapabilityRootAccountV1,
    CapabilityRootHeaderV1,
};
use dclutch_effect_kernel::v2::SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA;
use dclutch_market_core_codec::{
    CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState, Identity, MarketCoreStateSeedsV2,
    MarketIdentity, Phase, Readiness, Role,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x71; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x72; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x73; 32]);
const WRONG_REGISTRY_ID: Pubkey = Pubkey::new_from_array([0x74; 32]);
const GENERATION: u64 = 7;
const ROOT_INITIAL_DUST: u64 = 1;
const ROOT_TAIL_BYTES: usize = 8;

const PROFILE_RULE_BYTES: usize = 16;
const PROFILE_OPERATION_BYTES: usize = 16;
const PROFILE_ACCOUNT_COUNT: u16 = 2;
const PROFILE_OPERATION_COUNT: u16 = 4;
const SCALAR_COUNT: u16 = 8;
const IDENTITY_COUNT: u16 = 12;

#[derive(Clone)]
struct Fixture {
    instruction: Instruction,
    root: Pubkey,
    funding: Pubkey,
    descriptor_raw: Pubkey,
    hostile_record: Pubkey,
    market: Pubkey,
    root_rent: u64,
    funding_rent: u64,
}

#[derive(Clone, Copy)]
enum Campaign {
    Success,
    LateEffectRefusal,
}

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero content")
}

fn identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("nonzero identity")
}

fn program_identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program")
}

fn put(output: &mut [u8], offset: usize, source: &[u8]) {
    let end = offset.checked_add(source.len()).expect("fixture width");
    output
        .get_mut(offset..end)
        .expect("fixture destination")
        .copy_from_slice(source);
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    put(output, offset, &value.to_le_bytes());
}

fn account_profile() -> Vec<u8> {
    let mut output = vec![
        0_u8;
        ACCOUNT_PROFILE_HEADER_BYTES_V1
            + usize::from(PROFILE_ACCOUNT_COUNT) * PROFILE_RULE_BYTES
            + usize::from(PROFILE_OPERATION_COUNT) * PROFILE_OPERATION_BYTES
    ];
    put(&mut output, 0, &ACCOUNT_PROFILE_MAGIC_V1);
    put_u16(&mut output, 8, ACCOUNT_PROFILE_SCHEMA_VERSION_V1);
    put_u16(&mut output, 10, ACCOUNT_PROFILE_ARTIFACT_PROFILE_V1);
    put_u16(&mut output, 12, PROFILE_ACCOUNT_COUNT);
    put_u16(&mut output, 14, PROFILE_OPERATION_COUNT);
    put_u16(&mut output, 16, SCALAR_COUNT);
    put_u16(&mut output, 18, IDENTITY_COUNT);
    let root_rule = ACCOUNT_PROFILE_HEADER_BYTES_V1;
    *output.get_mut(root_rule).expect("root privileges") = 0x02;
    *output.get_mut(root_rule + 1).expect("root permission") = EFFECT_PERMISSION_CREDIT_LAMPORTS;
    put_u16(&mut output, root_rule + 2, 0);
    let funding_rule = root_rule + PROFILE_RULE_BYTES;
    *output.get_mut(funding_rule).expect("funding privileges") = 0x02;
    *output
        .get_mut(funding_rule + 1)
        .expect("funding permission") =
        EFFECT_PERMISSION_DEBIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA;
    put_u16(&mut output, funding_rule + 2, 1);
    put_u32(
        &mut output,
        funding_rule + 4,
        u32::try_from(FUNDING_STATE_BYTES).expect("funding width"),
    );
    let operations =
        ACCOUNT_PROFILE_HEADER_BYTES_V1 + usize::from(PROFILE_ACCOUNT_COUNT) * PROFILE_RULE_BYTES;
    // Root key equals common identity register 11.
    encode_profile_operation(&mut output, operations, 1, 0, 11, 0);
    // Funding owner equals common Trading identity register 0.
    encode_profile_operation(
        &mut output,
        operations + PROFILE_OPERATION_BYTES,
        2,
        1,
        0,
        0,
    );
    // Funding Rent amount at state offset 64 + allocation amount offset 8.
    encode_profile_operation(
        &mut output,
        operations + 2 * PROFILE_OPERATION_BYTES,
        6,
        1,
        6,
        72,
    );
    // Observe vacant-root dust for the late effect check.
    encode_profile_operation(
        &mut output,
        operations + 3 * PROFILE_OPERATION_BYTES,
        5,
        0,
        7,
        0,
    );
    output
}

fn encode_profile_operation(
    output: &mut [u8],
    offset: usize,
    opcode: u8,
    account: u16,
    register: u16,
    data_offset: u32,
) {
    *output.get_mut(offset).expect("profile opcode") = opcode;
    put_u16(output, offset + 2, account);
    put_u16(output, offset + 4, register);
    put_u32(output, offset + 8, data_offset);
}

fn transition_program() -> Vec<u8> {
    let mut output = vec![0_u8; 40];
    put(&mut output, 0, b"DCTV");
    *output.get_mut(4).expect("transition version") = 2;
    put_u16(&mut output, 6, 1);
    put_u16(&mut output, 8, SCALAR_COUNT);
    put_u16(&mut output, 10, IDENTITY_COUNT);
    // loadConst scalar[0] = activation action. Other projected registers survive.
    *output.get_mut(16).expect("transition opcode") = 0;
    put_u16(&mut output, 18, 0);
    put(
        &mut output,
        32,
        &(CoreEffectActionV1::ActivateCapability as u64).to_le_bytes(),
    );
    output
}

fn effect_program(campaign: Campaign) -> Vec<u8> {
    let instruction_count = match campaign {
        Campaign::Success => 1_u16,
        Campaign::LateEffectRefusal => 2_u16,
    };
    let mut output = vec![0_u8; 16 + usize::from(instruction_count) * 16];
    put(&mut output, 0, b"DCE2");
    *output.get_mut(4).expect("effect version") = 2;
    put_u16(&mut output, 6, instruction_count);
    put_u16(&mut output, 8, PROFILE_ACCOUNT_COUNT);
    put_u16(&mut output, 10, SCALAR_COUNT);
    put_u16(&mut output, 12, IDENTITY_COUNT);
    // Transfer projected Funding Rent scalar[6] from FundingState to root.
    put_u16(&mut output, 18, 1);
    put_u16(&mut output, 20, 0);
    put_u16(&mut output, 22, 6);
    if matches!(campaign, Campaign::LateEffectRefusal) {
        // After the transfer, root lamports cannot still equal prestate scalar[7].
        *output.get_mut(32).expect("late requirement opcode") = 3;
        put_u16(&mut output, 34, 0);
        put_u16(&mut output, 38, 7);
    }
    output
}

fn descriptor(
    profile_id: [u8; 32],
    effect_id: [u8; 32],
    kind: ContentId,
    capacity: ContentId,
    root_schema: ContentId,
    derivation: ContentId,
    config_schema: ContentId,
) -> Vec<u8> {
    let transition = transition_program();
    let mut output = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + transition.len()];
    put(&mut output, 0, &CAPABILITY_PROGRAM_MAGIC_V1);
    put_u16(&mut output, 8, 1);
    put_u16(
        &mut output,
        CAPABILITY_PROGRAM_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_PROFILE_V2,
    );
    for (offset, value) in [
        (CAPABILITY_PROGRAM_KIND_OFFSET, kind.to_bytes()),
        (
            CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET,
            config_schema.to_bytes(),
        ),
        (
            CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
            id(0x23).to_bytes(),
        ),
        (
            CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET,
            root_schema.to_bytes(),
        ),
        (CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, profile_id),
        (
            CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
            derivation.to_bytes(),
        ),
        (
            CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
            capacity.to_bytes(),
        ),
        (CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, effect_id),
    ] {
        put(&mut output, offset, &value);
    }
    put_u32(
        &mut output,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
        u32::try_from(ROOT_TAIL_BYTES).expect("tail width"),
    );
    put(&mut output, CAPABILITY_PROGRAM_HEADER_BYTES_V1, &transition);
    CapabilityProgramV1::decode(&output).expect("descriptor");
    output
}

fn release(program: Pubkey, seed: u8) -> ArtifactReleaseV1 {
    let programdata = Pubkey::new_from_array([seed.wrapping_add(1); 32]);
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(Pubkey::new_from_array([0x91; 32])),
        programdata.to_bytes(),
        id(seed.wrapping_add(2)),
        [seed.wrapping_add(3); 32],
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
        value.program().to_bytes(),
        value.loader_program().to_bytes(),
        true,
        value.programdata(),
        value.loader_program().to_bytes(),
        false,
        value.programdata(),
        value.loader_program().to_bytes(),
        value.deployment_slot(),
        value.elf_digest(),
        value.upgrade_authority(),
    )
    .expect("observation");
    ArtifactActivationInputV1::new(artifact_id(value), value, observation)
}

fn activation_cache() -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x31);
    let trading = release(TRADING_PROGRAM_ID, 0x41);
    let set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(core),
        binding(trading),
        binding(core),
        binding(core),
    )
    .expect("release set");
    let set_id = hash(&set.to_bytes()).to_bytes();
    let content = ContentId::new(set_id).expect("release set content");
    let mut output = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut output, content).expect("initialize cache");
    for (role, value) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, core),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, core),
        (ExecutionRoleV1::Custody, core),
    ] {
        activate_execution_role_into_v1(&mut output, content, &set, role, &activation_input(value))
            .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&output).expect("complete cache");
    (set_id, output)
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_record(test: &mut ProgramTest, schema: [u8; 32], bytes: Vec<u8>) -> (Pubkey, Pubkey) {
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        test,
        raw,
        REGISTRY_PROGRAM_ID,
        Rent::default().minimum_balance(bytes.len()),
        bytes,
    );
    add_account(test, staging, system_program::ID, 1, Vec::new());
    (raw, staging)
}

fn build_fixture(campaign: Campaign) -> (ProgramTest, Fixture) {
    let mut test = ProgramTest::new(
        "dclutch_trading_outer_test_program",
        TRADING_PROGRAM_ID,
        None,
    );
    test.add_program(
        "dclutch_trading_core_caller_test_program",
        CORE_PROGRAM_ID,
        None,
    );
    test.add_program(
        "dclutch_trading_registry_test_program",
        REGISTRY_PROGRAM_ID,
        None,
    );
    test.add_program(
        "dclutch_trading_registry_test_program",
        WRONG_REGISTRY_ID,
        None,
    );

    let rent = Rent::default();
    let root_rent = rent.minimum_balance(232 + ROOT_TAIL_BYTES);
    let funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let profile = account_profile();
    let effect = effect_program(campaign);
    let kind = id(0x11);
    let capacity = id(0x12);
    let root_schema = id(0x13);
    let derivation = id(0x14);
    let config_schema = id(0x15);
    let config = vec![0x61; 32];
    let descriptor = descriptor(
        hash(&profile).to_bytes(),
        hash(&effect).to_bytes(),
        kind,
        capacity,
        root_schema,
        derivation,
        config_schema,
    );
    let descriptor_id = ContentId::new(hash(&descriptor).to_bytes()).expect("descriptor ID");
    let config_id = ContentId::new(hash(&config).to_bytes()).expect("config ID");
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(root_rent - ROOT_INITIAL_DUST)
            .expect("root rent quote"),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .expect("funding amounts");
    let entry = CapabilityEntryV1::new(
        kind,
        descriptor_id,
        config_id,
        capacity,
        root_schema,
        derivation,
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).expect("quote"),
    )
    .expect("entry");
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest");
    let manifest_id = ContentId::new(hash(&manifest).to_bytes()).expect("manifest ID");
    let selection =
        CapabilityExecutionSelectionV1::new(0, manifest_id, kind, descriptor_id, config_id)
            .expect("selection");

    let (release_set, cache_bytes) = activation_cache();
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        activation_cache,
        REGISTRY_PROGRAM_ID,
        rent.minimum_balance(cache_bytes.len()),
        cache_bytes,
    );
    let core_programdata = Pubkey::new_from_array([0x32; 32]);
    let trading_programdata = Pubkey::new_from_array([0x42; 32]);
    add_account(&mut test, core_programdata, system_program::ID, 1, vec![1]);
    add_account(
        &mut test,
        trading_programdata,
        system_program::ID,
        1,
        vec![1],
    );

    let mut state = CoreState {
        phase: Phase::Founding,
        readiness: Readiness::Prepaid,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: identity([0x21; 32]),
            realm_id: identity([0x22; 32]),
            product_record: identity([0x23; 32]),
            product_id: identity([0x24; 32]),
            resolution_policy: identity([0x25; 32]),
            capability_manifest: identity(manifest_id.to_bytes()),
            selected_release_set: identity(release_set),
            registry_program: identity(REGISTRY_PROGRAM_ID.to_bytes()),
            generation: GENERATION,
        },
        outstanding_capabilities: 0,
        rent_beneficiary: identity([0x26; 32]),
        terminal_receipt: None,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    state.identity.market_id = identity(market.to_bytes());
    let state_bytes = state.encode().expect("Core state");
    add_account(
        &mut test,
        market,
        CORE_PROGRAM_ID,
        rent.minimum_balance(state_bytes.len()),
        state_bytes.to_vec(),
    );

    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).expect("release set"),
        market.to_bytes(),
        GENERATION,
        selection,
    )
    .expect("root header");
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID).0;
    add_account(
        &mut test,
        root,
        system_program::ID,
        ROOT_INITIAL_DUST,
        Vec::new(),
    );
    let funding_custody = FundingCustodyObservationV1::native_only(
        funding_rent + root_rent - ROOT_INITIAL_DUST,
        funding_rent,
    )
    .expect("funding custody");
    let funding_state = FundingStateV1::new(
        manifest_id,
        CapabilityManifestV1::decode(&manifest).expect("manifest"),
        0,
        funding_custody,
    )
    .expect("funding state");
    let funding_derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        manifest_id,
        CapabilityManifestV1::decode(&manifest).expect("manifest"),
        funding_state,
    )
    .expect("funding derivation");
    let funding =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &TRADING_PROGRAM_ID).0;
    add_account(
        &mut test,
        funding,
        TRADING_PROGRAM_ID,
        funding_rent + root_rent - ROOT_INITIAL_DUST,
        funding_state.to_bytes().to_vec(),
    );

    let descriptor_record = add_record(
        &mut test,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
        descriptor,
    );
    let config_record = add_record(&mut test, config_schema.to_bytes(), config);
    let profile_record = add_record(&mut test, ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, profile);
    let effect_record = add_record(&mut test, EFFECT_PROGRAM_SCHEMA, effect);
    let hostile_record = Pubkey::new_from_array([0xa1; 32]);
    add_account(
        &mut test,
        hostile_record,
        REGISTRY_PROGRAM_ID,
        rent.minimum_balance(32),
        vec![0xa5; 32],
    );
    let manifest_raw = Pubkey::new_from_array([0xa2; 32]);
    add_account(
        &mut test,
        manifest_raw,
        REGISTRY_PROGRAM_ID,
        rent.minimum_balance(manifest.len()),
        manifest,
    );

    let mut role_request = selection.to_bytes().to_vec();
    role_request.extend_from_slice(
        &dclutch_market_core_codec::CapabilityFundingHeaderV1::new(1)
            .expect("funding header")
            .encode(),
    );
    role_request.push(1);
    let role_digest = hash(&role_request).to_bytes();
    let context = [0x81; 32];
    let authority_seeds = dclutch_release_set_contract::CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        role_digest,
    )
    .expect("caller authority seeds");
    let caller_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), &CORE_PROGRAM_ID).0;
    add_account(
        &mut test,
        caller_authority,
        system_program::ID,
        1,
        Vec::new(),
    );
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::ActivateCapability,
        Role::Trading,
        identity(CORE_PROGRAM_ID.to_bytes()),
        identity(caller_authority.to_bytes()),
        identity(release_set),
        identity(market.to_bytes()),
        identity(context),
        identity(hash(&state_bytes).to_bytes()),
        identity(role_digest),
        GENERATION,
        0,
        0,
        u32::try_from(role_request.len()).expect("request width"),
    )
    .expect("envelope");
    let mut instruction_data = envelope.encode().expect("envelope bytes").to_vec();
    instruction_data.extend_from_slice(&role_request);
    let accounts = vec![
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new(root, false),
        AccountMeta::new(funding, false),
        AccountMeta::new_readonly(manifest_raw, false),
        AccountMeta::new_readonly(market, false),
        AccountMeta::new_readonly(descriptor_record.0, false),
        AccountMeta::new_readonly(descriptor_record.1, false),
        AccountMeta::new_readonly(config_record.0, false),
        AccountMeta::new_readonly(config_record.1, false),
        AccountMeta::new_readonly(profile_record.0, false),
        AccountMeta::new_readonly(profile_record.1, false),
        AccountMeta::new_readonly(effect_record.0, false),
        AccountMeta::new_readonly(effect_record.1, false),
        AccountMeta::new_readonly(activation_cache, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(core_programdata, false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(trading_programdata, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    (
        test,
        Fixture {
            instruction: Instruction {
                program_id: CORE_PROGRAM_ID,
                accounts,
                data: instruction_data,
            },
            root,
            funding,
            descriptor_raw: descriptor_record.0,
            hostile_record,
            market,
            root_rent,
            funding_rent,
        },
    )
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account lookup")
        .expect("account exists")
}

async fn assert_rollback(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
) {
    let root_before = account(context, fixture.root).await;
    let funding_before = account(context, fixture.funding).await;
    assert!(submit(context, instruction).await.is_err());
    assert_eq!(account(context, fixture.root).await, root_before);
    assert_eq!(account(context, fixture.funding).await, funding_before);
}

#[tokio::test]
async fn common_outer_activates_root_and_funding_commit_last() {
    let (test, fixture) = build_fixture(Campaign::Success);
    let mut context = test.start_with_context().await;
    submit(&mut context, fixture.instruction.clone())
        .await
        .expect("activation succeeds");
    let root = account(&mut context, fixture.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert_eq!(root.lamports, fixture.root_rent);
    let descriptor_account = account(&mut context, fixture.descriptor_raw).await;
    let descriptor = CapabilityProgramV1::decode(&descriptor_account.data).expect("descriptor");
    let decoded = CapabilityRootAccountV1::decode(&root.data, descriptor).expect("root account");
    assert_eq!(decoded.header().market(), fixture.market.to_bytes());
    assert!(decoded.state().iter().all(|byte| *byte == 0));
    let funding = account(&mut context, fixture.funding).await;
    assert_eq!(funding.lamports, fixture.funding_rent);
    let funding = FundingStateV1::decode(&funding.data).expect("funding poststate");
    assert_eq!(funding.status(), FundingStatus::Active);
    assert!(funding.activation_slot() > 0);
    assert_eq!(funding.remaining().rent().amount(), 0);
}

#[tokio::test]
async fn substituted_registry_record_and_root_refuse_atomically() {
    for substitution in 0..3 {
        let (test, fixture) = build_fixture(Campaign::Success);
        let mut context = test.start_with_context().await;
        let mut instruction = fixture.instruction.clone();
        match substitution {
            0 => {
                instruction
                    .accounts
                    .get_mut(18)
                    .expect("Registry meta")
                    .pubkey = WRONG_REGISTRY_ID
            }
            1 => {
                instruction
                    .accounts
                    .get_mut(5)
                    .expect("descriptor record meta")
                    .pubkey = fixture.hostile_record
            }
            _ => {
                instruction.accounts.get_mut(1).expect("root meta").pubkey = fixture.hostile_record
            }
        }
        assert_rollback(&mut context, &fixture, instruction).await;
    }
}

#[tokio::test]
async fn late_effect_refusal_rolls_back_the_projected_transfer() {
    let (test, fixture) = build_fixture(Campaign::LateEffectRefusal);
    let mut context = test.start_with_context().await;
    assert_rollback(&mut context, &fixture, fixture.instruction.clone()).await;
}
