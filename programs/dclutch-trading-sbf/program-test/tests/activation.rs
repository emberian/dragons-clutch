//! ProgramTest evidence for the common data-defined Trading activation outer.

use std::vec::Vec;

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1,
    encode_v1::{
        AccountAliasInputV1, AccountEffectPermissionsV1, AccountOperationInputV1,
        AccountPrivilegesV1, AccountRuleInputV1, RegisterGeometryV1, account_profile_v1_bytes,
        encode_account_profile_v1_atomic,
    },
};
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, CompartmentFundingV1, ContentId, FUNDING_STATE_BYTES,
    FUNDING_STATE_REMAINING_RENT_AMOUNT_OFFSET_V1, FundingAmountsV1, FundingCustodyObservationV1,
    FundingQuoteV1, FundingStateV1, FundingStatus, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{
    activation_registers_v2::{
        ACTIVATION_ACTION_SCALAR_V2, ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_GENERATION_SCALAR_V2, ACTIVATION_MARKET_IDENTITY_V2, ACTIVATION_ROOT_ACCOUNT_V2,
        ACTIVATION_ROOT_IDENTITY_V2, ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
    },
    CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
    CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
    CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1, CAPABILITY_PROGRAM_PROFILE_OFFSET,
    CAPABILITY_PROGRAM_PROFILE_V2, CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
    CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CapabilityProgramV1, CapabilityRootAccountV1,
    CapabilityRootHeaderV1,
    set_v2::{
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
        CapabilityProgramSetEntryV2, SelectorWidthV2, encode_program_set_v2,
        encoded_program_set_bytes_v2,
    },
};
use dclutch_effect_kernel::v2::{
    SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA,
    encode::{
        EffectGeometryV2, EffectInstructionV2, effect_program_v2_bytes,
        encode_effect_program_v2_atomic,
    },
};
use dclutch_transition_vm::v2::encode::{
    RegisterGeometryV2 as TransitionRegisterGeometryV2, TransitionInstructionV2,
    encode_transition_program_v2_atomic, transition_program_v2_bytes,
};
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
use dclutch_trading_sbf::TradingSbfError;
use solana_program::instruction::InstructionError;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

/// `TradingSbfError::Root`, the refusal the composite-root plan carries.
const TRADING_ROOT_REFUSAL_CODE: u32 = TradingSbfError::Root as u32;
/// `TradingSbfError::Content`, the refusal record and selection joins carry.
const TRADING_CONTENT_REFUSAL_CODE: u32 = TradingSbfError::Content as u32;
/// `TradingSbfError::UnsupportedContent`, the refusal an unadmitted schema carries.
const TRADING_UNSUPPORTED_REFUSAL_CODE: u32 = TradingSbfError::UnsupportedContent as u32;
/// Selector the family activation request carries, and the set entry's own.
const FAMILY_ACTIVATION_SELECTOR: u32 = 1;

const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x71; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x72; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x73; 32]);
const WRONG_REGISTRY_ID: Pubkey = Pubkey::new_from_array([0x74; 32]);
const GENERATION: u64 = 7;
const ROOT_INITIAL_DUST: u64 = 1;
/// Family root-tail width: one projected scalar then one projected identity.
///
/// The outer never decodes a family tail, so this fixture stands in for a real
/// family root the only way the seam can observe one -- as the exact width the
/// descriptor declares and the exact bytes the effect program's request buffer
/// projects.
const ROOT_TAIL_BYTES: usize = 40;
/// Tail offset of the projected generation scalar.
const TAIL_GENERATION_OFFSET: u32 = 0;
/// Tail offset of the projected Market identity.
const TAIL_MARKET_OFFSET: u32 = 8;
/// Scalar the profile projects the FundingState's remaining Rent quote into.
///
/// It is past the eight common slots the seam seeds, so nothing the outer wrote
/// is overwritten -- the register ABI's own boundary, not a coincidence.
const FUNDING_RENT_SCALAR_REGISTER: u16 = 6;
/// Scalar the profile projects the vacant root's prestate lamports into.
const ROOT_PRESTATE_SCALAR_REGISTER: u16 = 7;

const PROFILE_ACCOUNT_COUNT: u16 = 2;
const SCALAR_COUNT: u16 = 8;
const IDENTITY_COUNT: u16 = 12;

#[derive(Clone)]
struct Fixture {
    instruction: Instruction,
    root: Pubkey,
    funding: Pubkey,
    /// Raw record the selection's `capability_release` names.
    descriptor_raw: Pubkey,
    /// Raw record carrying the `CapabilityProgramV1` the seam actually runs.
    activation_descriptor_raw: Pubkey,
    hostile_record: Pubkey,
    market: Pubkey,
    root_rent: u64,
    funding_rent: u64,
}

#[derive(Clone, Copy)]
enum Campaign {
    Success,
    LateEffectRefusal,
    /// Declares the tail width and projects nothing into it.
    UnwrittenTail,
    /// Projects the whole tail into a request buffer wider than the tail.
    MismatchedTailWidth,
    /// `capability_release` names a `CapabilityProgramSetV2`, not a descriptor.
    ProgramSetRelease,
    /// The selected set entry names a descriptor schema this seam cannot run.
    ProgramSetWrongSchema,
    /// No set entry admits the selector the family activation request carries.
    ProgramSetMissingSelector,
}

impl Campaign {
    /// Whether `capability_release` names a set rather than a flat descriptor.
    const fn program_set(self) -> bool {
        matches!(
            self,
            Self::ProgramSetRelease | Self::ProgramSetWrongSchema | Self::ProgramSetMissingSelector
        )
    }
}

/// One-entry `CapabilityProgramSetV2` naming the activation descriptor.
///
/// The selector is read from byte 0 of the family activation request, which is
/// the same one-byte action the flat campaigns already send. A real family puts
/// its activation action wherever its own request grammar puts an action.
fn program_set(descriptor_id: ContentId, campaign: Campaign) -> Vec<u8> {
    let schema = match campaign {
        Campaign::ProgramSetWrongSchema => id(0x77),
        _ => ContentId::new(CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1).expect("descriptor schema"),
    };
    let selector = match campaign {
        Campaign::ProgramSetMissingSelector => FAMILY_ACTIVATION_SELECTOR + 1,
        _ => FAMILY_ACTIVATION_SELECTOR,
    };
    let entry = CapabilityProgramSetEntryV2::new(
        selector,
        CapabilityDescriptorReferenceV2::new(schema, descriptor_id),
    );
    let mut output = vec![0_u8; encoded_program_set_bytes_v2(1).expect("set width")];
    encode_program_set_v2(0, SelectorWidthV2::U8, &[entry], &mut output).expect("set bytes");
    output
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

/// Encode one artifact through its owning crate's public encoder.
///
/// Every artifact below is built this way. The three generations' offsets and
/// opcodes are private to their crates, so before they had encoders this file
/// wrote `b"DCTV"`, `b"DCE2"` and the AccountProfile header at literal offsets
/// and passed bare opcode integers with comments -- a second ABI authority
/// living in a test. Nothing here writes an artifact byte any more.
fn encoded<E, T>(width: usize, encode: E) -> Vec<u8>
where
    E: Fn(&mut [u8], &mut [u8]) -> Result<(), T>,
    T: core::fmt::Debug,
{
    let mut scratch = vec![0_u8; width];
    let mut output = vec![0_u8; width];
    encode(&mut scratch, &mut output).expect("artifact encodes");
    output
}

fn account_profile() -> Vec<u8> {
    let rules = [
        // The composite root: vacant, credited by the funding transfer.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(false, true, false),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: 0,
        },
        // The FundingState: debited, and rewritten by the outer's own commit.
        AccountRuleInputV1 {
            privileges: AccountPrivilegesV1::new(false, true, false),
            effect_permissions: AccountEffectPermissionsV1::new(true, false, true),
            alias: AccountAliasInputV1::SelfRepresentative,
            data_length: u32::try_from(FUNDING_STATE_BYTES).expect("funding width"),
        },
    ];
    let operations = [
        AccountOperationInputV1::RequireKey {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            expected: ACTIVATION_ROOT_IDENTITY_V2,
        },
        AccountOperationInputV1::RequireOwner {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
            expected: ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
        },
        AccountOperationInputV1::ProjectDataU64 {
            account: ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
            data_offset: u32::try_from(FUNDING_STATE_REMAINING_RENT_AMOUNT_OFFSET_V1)
                .expect("rent quote offset"),
            destination: FUNDING_RENT_SCALAR_REGISTER,
        },
        // Observe vacant-root dust for the late effect check.
        AccountOperationInputV1::ProjectLamports {
            account: ACTIVATION_ROOT_ACCOUNT_V2,
            destination: ROOT_PRESTATE_SCALAR_REGISTER,
        },
    ];
    let width = account_profile_v1_bytes(rules.len(), operations.len()).expect("profile width");
    encoded(width, |scratch, output| {
        encode_account_profile_v1_atomic(
            &rules,
            &operations,
            RegisterGeometryV1 {
                scalars: SCALAR_COUNT,
                identities: IDENTITY_COUNT,
            },
            scratch,
            output,
        )
    })
}

fn transition_program() -> Vec<u8> {
    // loadConst scalar[0] = activation action. Other projected registers survive.
    let instructions = [TransitionInstructionV2::load_const(
        ACTIVATION_ACTION_SCALAR_V2,
        CoreEffectActionV1::ActivateCapability as u64,
    )];
    let width = transition_program_v2_bytes(instructions.len()).expect("transition width");
    encoded(width, |scratch, output| {
        encode_transition_program_v2_atomic(
            TransitionRegisterGeometryV2 {
                scalars: SCALAR_COUNT,
                identities: IDENTITY_COUNT,
            },
            &instructions,
            scratch,
            output,
        )
    })
}

fn effect_program(campaign: Campaign) -> Vec<u8> {
    // Instruction 0 is always the funding transfer. The two request writes that
    // compose the family root tail follow it, except in `UnwrittenTail`. The late
    // requirement, when present, is last so it runs after the transfer.
    let mut instructions = vec![EffectInstructionV2::transfer_lamports(
        ACTIVATION_FIRST_FUNDING_ACCOUNT_V2,
        ACTIVATION_ROOT_ACCOUNT_V2,
        FUNDING_RENT_SCALAR_REGISTER,
    )];
    if !matches!(campaign, Campaign::UnwrittenTail) {
        // Tail[0..8] = the projected Market generation.
        instructions.push(EffectInstructionV2::write_request_u64(
            TAIL_GENERATION_OFFSET,
            ACTIVATION_GENERATION_SCALAR_V2,
        ));
        // Tail[8..40] = the projected Market address.
        instructions.push(EffectInstructionV2::write_request_identity(
            TAIL_MARKET_OFFSET,
            ACTIVATION_MARKET_IDENTITY_V2,
        ));
    }
    if matches!(campaign, Campaign::LateEffectRefusal) {
        // After the transfer, root lamports cannot still equal prestate scalar[7].
        instructions.push(EffectInstructionV2::require_lamports_eq(
            ACTIVATION_ROOT_ACCOUNT_V2,
            ROOT_PRESTATE_SCALAR_REGISTER,
        ));
    }
    let request_bytes = match campaign {
        Campaign::MismatchedTailWidth => ROOT_TAIL_BYTES + 8,
        _ => ROOT_TAIL_BYTES,
    };
    let width = effect_program_v2_bytes(instructions.len()).expect("effect width");
    encoded(width, |scratch, output| {
        encode_effect_program_v2_atomic(
            EffectGeometryV2 {
                accounts: PROFILE_ACCOUNT_COUNT,
                scalars: SCALAR_COUNT,
                identities: IDENTITY_COUNT,
                request_bytes: u16::try_from(request_bytes).expect("request width"),
            },
            &instructions,
            scratch,
            output,
        )
    })
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
    // For a set release the selection names the SET, and the descriptor is one of
    // its entries; for a flat release the two identities are the same record.
    let program_set_bytes = campaign
        .program_set()
        .then(|| program_set(descriptor_id, campaign));
    let release_id = match &program_set_bytes {
        Some(bytes) => ContentId::new(hash(bytes).to_bytes()).expect("release ID"),
        None => descriptor_id,
    };
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
        release_id,
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
    let selection = CapabilityExecutionSelectionV1::new(0, manifest_id, kind, release_id, config_id)
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
    let release_record = match program_set_bytes {
        Some(bytes) => add_record(&mut test, CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, bytes),
        None => descriptor_record,
    };
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
    let mut accounts = vec![
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new(root, false),
        AccountMeta::new(funding, false),
        AccountMeta::new_readonly(manifest_raw, false),
        AccountMeta::new_readonly(market, false),
        AccountMeta::new_readonly(release_record.0, false),
        AccountMeta::new_readonly(release_record.1, false),
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
    if campaign.program_set() {
        // Family accounts 16 and 17: the descriptor the set entry names. A flat
        // release carries neither, and its frame is byte-identical to before.
        accounts.push(AccountMeta::new_readonly(descriptor_record.0, false));
        accounts.push(AccountMeta::new_readonly(descriptor_record.1, false));
    }
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
            descriptor_raw: release_record.0,
            activation_descriptor_raw: descriptor_record.0,
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
) -> BanksClientError {
    let root_before = account(context, fixture.root).await;
    let funding_before = account(context, fixture.funding).await;
    let error = submit(context, instruction)
        .await
        .expect_err("activation refuses");
    assert_eq!(account(context, fixture.root).await, root_before);
    assert_eq!(account(context, fixture.funding).await, funding_before);
    error
}

/// The custom program code the refusal carried, so a test can name it.
fn refusal_code(error: &BanksClientError) -> Option<u32> {
    let transaction = match error {
        BanksClientError::TransactionError(value) => value,
        BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

/// Submit one campaign and require the exact created root and funding poststate.
async fn assert_activation_succeeds(context: &mut ProgramTestContext, fixture: &Fixture) {
    submit(context, fixture.instruction.clone())
        .await
        .expect("activation succeeds");
    let root = account(context, fixture.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert_eq!(root.lamports, fixture.root_rent);
    let descriptor_account = account(context, fixture.activation_descriptor_raw).await;
    let descriptor = CapabilityProgramV1::decode(&descriptor_account.data).expect("descriptor");
    let decoded = CapabilityRootAccountV1::decode(&root.data, descriptor).expect("root account");
    assert_eq!(decoded.header().market(), fixture.market.to_bytes());
    // The family tail is the effect program's projected request buffer, exactly.
    // Before this was so, the seam wrote `vec![0; root_state_bytes]` and no family
    // root -- General's or Direct's -- could be decoded out of what it created.
    let mut expected_tail = vec![0_u8; ROOT_TAIL_BYTES];
    put(&mut expected_tail, 0, &GENERATION.to_le_bytes());
    put(&mut expected_tail, 8, &fixture.market.to_bytes());
    assert_eq!(decoded.state(), expected_tail.as_slice());
    let funding = account(context, fixture.funding).await;
    assert_eq!(funding.lamports, fixture.funding_rent);
    let funding = FundingStateV1::decode(&funding.data).expect("funding poststate");
    assert_eq!(funding.status(), FundingStatus::Active);
    assert!(funding.activation_slot() > 0);
    assert_eq!(funding.remaining().rent().amount(), 0);
}

#[tokio::test]
async fn common_outer_activates_root_and_funding_commit_last() {
    let (test, fixture) = build_fixture(Campaign::Success);
    let mut context = test.start_with_context().await;
    assert_activation_succeeds(&mut context, &fixture).await;
}

/// A `CapabilityProgramSetV2` at `capability_release` activates the same root.
///
/// This is the generation `hot_v3` authenticates. Before it, the seam decoded
/// the record at `selection.capability_release()` as a `CapabilityProgramV1`
/// and nothing else, so a capability whose release is a selector table -- which
/// is every V3/V4 family -- had no route that could create its root at all. The
/// selection is a seed of the root PDA, so one selection could not satisfy both
/// generations and the newer one simply had no door.
///
/// Nothing here is a kind branch: the release generation is read off the raw
/// record's own PDA, and the descriptor the set names must still satisfy the
/// same manifest-entry join the flat generation does.
#[tokio::test]
async fn a_program_set_release_activates_through_its_selected_descriptor() {
    let (test, fixture) = build_fixture(Campaign::ProgramSetRelease);
    let mut context = test.start_with_context().await;
    assert_ne!(fixture.descriptor_raw, fixture.activation_descriptor_raw);
    assert_eq!(fixture.instruction.accounts.len(), 23);
    assert_activation_succeeds(&mut context, &fixture).await;
}

/// Reversion control for the set path, at both of its own joins.
///
/// `ProgramSetWrongSchema` is the case that matters most: a set entry naming a
/// descriptor schema this seam cannot run is refused at the entry, before any
/// account is read, so a hot-action `CapabilityProgramV4` can never arrive here
/// as an activation descriptor. `ProgramSetMissingSelector` is the request-side
/// half -- a family request selecting no entry refuses instead of defaulting.
#[tokio::test]
async fn a_set_entry_this_seam_cannot_run_or_cannot_select_refuses() {
    for (campaign, expected) in [
        (
            Campaign::ProgramSetWrongSchema,
            TRADING_UNSUPPORTED_REFUSAL_CODE,
        ),
        (
            Campaign::ProgramSetMissingSelector,
            TRADING_CONTENT_REFUSAL_CODE,
        ),
    ] {
        let (test, fixture) = build_fixture(campaign);
        let mut context = test.start_with_context().await;
        let error = assert_rollback(&mut context, &fixture, fixture.instruction.clone()).await;
        assert_eq!(refusal_code(&error).expect("custom refusal code"), expected);
    }
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

/// Reversion control for the tail channel, both directions.
///
/// `UnwrittenTail` is the exact prior behaviour of this seam -- a declared tail
/// width with nothing projected into it -- and it now refuses instead of
/// creating a root no family can decode. `MismatchedTailWidth` projects the
/// whole tail into a request buffer eight bytes wider than the descriptor's
/// `root_state_bytes`, which the outer refuses rather than truncating.
#[tokio::test]
async fn a_tail_that_is_unwritten_or_the_wrong_width_refuses() {
    for campaign in [Campaign::UnwrittenTail, Campaign::MismatchedTailWidth] {
        let (test, fixture) = build_fixture(campaign);
        let mut context = test.start_with_context().await;
        let error = assert_rollback(&mut context, &fixture, fixture.instruction.clone()).await;
        assert_eq!(
            refusal_code(&error).expect("custom refusal code"),
            TRADING_ROOT_REFUSAL_CODE
        );
        let root = account(&mut context, fixture.root).await;
        assert_eq!(root.owner, system_program::ID);
        assert!(root.data.is_empty());
    }
}


/// The three artifacts are byte-identical to what this file hand-encoded.
///
/// Before `73f7ec7`/`f98d439`/`d18c32d` gave the three generations public
/// encoders, this file wrote all three wire formats itself. The replacement is
/// only worth having if it moved nothing: these are the exact bytes the deleted
/// hand-encoders produced, captured first. They are also the record digests the
/// descriptor names and the PDA seeds every raw record sits at, so a byte that
/// moved here would move five addresses.
#[test]
fn the_public_encoders_reproduce_the_prior_artifact_bytes() {
    const PROFILE: [u8; 128] = [
        0x44, 0x43, 0x4c, 0x54, 0x41, 0x50, 0x30, 0x31, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x04,
        0x00, 0x08, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x02, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x02, 0x05, 0x01, 0x00, 0x40, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x01, 0x00, 0x06, 0x00, 0x00, 0x00, 0x48,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const TRANSITION: [u8; 40] = [
        0x44, 0x43, 0x54, 0x56, 0x02, 0x00, 0x01, 0x00, 0x08, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const EFFECT: [u8; 64] = [
        0x44, 0x43, 0x45, 0x32, 0x02, 0x00, 0x03, 0x00, 0x02, 0x00, 0x08, 0x00, 0x0c, 0x00, 0x28,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x08, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    assert_eq!(account_profile().as_slice(), PROFILE.as_slice());
    assert_eq!(transition_program().as_slice(), TRANSITION.as_slice());
    assert_eq!(
        effect_program(Campaign::Success).as_slice(),
        EFFECT.as_slice()
    );
}
