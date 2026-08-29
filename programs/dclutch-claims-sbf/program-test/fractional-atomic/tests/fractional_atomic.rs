//! Real-ELF execution of the production Fractional atomic Claims route.
//!
//! `programs/dclutch-claims-sbf/src/fractional_atomic_v3.rs` ships the Wrap and
//! WholeUnwrap handlers, and `hot_v3.rs` already dispatches their request magic,
//! but nothing had ever executed them: the route needs a caller that can sign
//! the release-scoped Trading caller-authority and the Trading-owned Fractional
//! root in one `invoke_signed`, and no such caller existed. This campaign is
//! that execution.
//!
//! Everything here is a real built `.so`: Claims, Registry, Core, Token-2022,
//! and the test caller. The 31-account frame is the exact production frame from
//! `dclutch-fractional-claim-contract`, not a convenient subset.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::{env, fs, path::PathBuf};

use dclutch_capability_program_contract::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_fractional_atomic_program_test::narrow_fixture::{
    FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2, NarrowFixtureError, NarrowFixtureInputV2,
    NarrowFixtureV2, NarrowRecordV2, NarrowTerminalInputV2, compile_narrow_fixture_v2,
};
use dclutch_core_contract::ContentId;
use dclutch_fractional_atomic_test_caller_sbf::FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4,
    FractionalExposureActionV2, FractionalExposureRequestInputV2, FractionalExposureRequestV2,
    FractionalRootInputV1, FractionalRootV1,
};
use dclutch_fractional_claim_kernel::{
    FractionalExposureTermsInputV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1,
    ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{signature::Keypair, signer::Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_transaction::Transaction;

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const TEST_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa8; 32]);
const REALM_ID: [u8; 32] = [0x61; 32];
const CUSTODY_CONTEXT: [u8; 32] = [0x62; 32];
const GENERATION: u64 = 37;
const ROOT_REVISION: u64 = 1;
const DENOMINATOR: u64 = 10;
const WRAP_NATIVE_CLAIMS: u64 = 7;
/// Native Claims the actor holds at the selected coordinate before any wrap.
const ACTOR_FUNDED_BALANCE: u64 = 1_000;
const OUTCOME: u32 = 0;
const MINT_DECIMALS: u8 = 0;
const GRAPH_ID: [u8; 32] = [0x7c; 32];
const RENT_CREDIT: Pubkey = Pubkey::new_from_array([0x65; 32]);
const EXPOSURE_ID: [u8; 32] = [0x7a; 32];
const TOKEN_ACCOUNT_BYTES: usize = 165;

/// Deterministic actor identity: it must sign, so it needs a real key.
fn actor_keypair() -> Keypair {
    Keypair::new_from_array([0x5c; 32])
}

fn token_program_id() -> Pubkey {
    Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID)
}

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    caller: Vec<u8>,
    token: Vec<u8>,
    custody: Vec<u8>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| {
        let path = directory.join(name);
        assert!(path.is_file(), "missing real ELF: {}", path.display());
        fs::read(path).expect("read real ELF")
    };
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        caller: read("dclutch_fractional_atomic_test_caller_sbf.so"),
        token: read("spl_token_2022.so"),
        custody: read("dclutch_custody_sbf.so"),
    }
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority option") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
    );
}

fn release(program: Pubkey, semantic_seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic_seed; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
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
        bpf_loader_upgradeable::ID.to_bytes(),
        true,
        value.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        false,
        value.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        value.deployment_slot(),
        value.elf_digest(),
        value.upgrade_authority(),
    )
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(value), value, observation)
}

fn activation_cache(artifacts: &Artifacts, custody: Option<Pubkey>) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x31, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x32, &artifacts.claims);
    let trading = release(TEST_CALLER_PROGRAM_ID, 0x33, &artifacts.caller);
    // Custody must be a program distinct from Claims -- the replay-creation
    // route explicitly refuses `custody_program.key == program_id` -- so a
    // terminal campaign binds the real Custody ELF where an atomic one can
    // leave the role pointing at Claims.
    let custody = match custody {
        None => claims,
        Some(program) => release(program, 0x34, &artifacts.custody),
    };
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(claims),
        binding(custody),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("initialize cache");
    for (role, value) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(value),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

/// Reproduce the shared fixture's finalized-record PDA derivation.
fn finalized(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> NarrowRecordV2 {
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &owner).0;
    NarrowRecordV2 {
        owner,
        schema,
        digest,
        raw,
        staging,
        bytes,
    }
}

fn add_finalized(test: &mut ProgramTest, record: &NarrowRecordV2) {
    add_account(test, record.raw, record.owner, record.bytes.clone());
    add_account(test, record.staging, system_program::ID, Vec::new());
}

fn compile_shared(
    release_set: [u8; 32],
    actor_owner: Pubkey,
    reserve_owner: Pubkey,
) -> NarrowFixtureV2 {
    compile_narrow_fixture_v2(NarrowFixtureInputV2 {
        outcome_count: FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2,
        registry_program: REGISTRY_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        release_set,
        realm_id: REALM_ID,
        custody_context: CUSTODY_CONTEXT,
        generation: GENERATION,
        actor_owner,
        reserve_owner,
        funded_coordinate: OUTCOME as usize,
        funded_balance: ACTOR_FUNDED_BALANCE,
        reserve_balance: 0,
        terminal: None,
        rent_beneficiary: RENT_CREDIT,
        graph_id: GRAPH_ID,
        exposure_id: EXPOSURE_ID,
    })
    .expect("narrow Product/LBV2 fixture at the Fractional width bound")
}

/// Exact Token-2022 Mint with the Fractional root as sole controller.
///
/// A Token-2022 Mint carrying extensions is padded to the base Account width
/// and then tagged, so this is not the legacy 82-byte layout. Mirrors
/// `retirement_mint_bytes` in the protocol-position campaign.
fn mint_bytes(controller: Pubkey, supply: u64) -> Vec<u8> {
    const TLV_START: usize = 166;
    let mut bytes = vec![0_u8; TLV_START];
    put(&mut bytes, 0, &1_u32.to_le_bytes());
    put(&mut bytes, 4, controller.as_ref());
    put(&mut bytes, 36, &supply.to_le_bytes());
    *bytes.get_mut(44).expect("Mint decimals") = MINT_DECIMALS;
    *bytes.get_mut(45).expect("Mint initialized") = 1;
    *bytes.get_mut(165).expect("Mint account type") = 1;
    for extension in [3_u16, 28_u16] {
        bytes.extend_from_slice(&extension.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(controller.as_ref());
    }
    bytes
}

fn token_account_bytes_for(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; TOKEN_ACCOUNT_BYTES];
    put(&mut bytes, 0, mint.as_ref());
    put(&mut bytes, 32, owner.as_ref());
    put(&mut bytes, 64, &amount.to_le_bytes());
    put(&mut bytes, 72, &0_u32.to_le_bytes());
    *bytes.get_mut(108).expect("state") = 1;
    put(&mut bytes, 109, &0_u32.to_le_bytes());
    put(&mut bytes, 129, &0_u32.to_le_bytes());
    bytes
}

/// One action's exact wrapper bytes and its release-scoped caller authority.
struct ActionArm {
    caller_authority: Pubkey,
    wrapper: Vec<u8>,
}

struct Fixture {
    shared: NarrowFixtureV2,
    release_set: [u8; 32],
    activation_cache: Pubkey,
    wrap: ActionArm,
    unwrap: ActionArm,
    root: Pubkey,
    terms_record: NarrowRecordV2,
    behavior_record: NarrowRecordV2,
    shard_mint: Pubkey,
    holder_token: Pubkey,
    actor: Pubkey,
}

fn fixture() -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_fractional_atomic_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
        (
            "spl_token_2022",
            token_program_id(),
            artifacts.token.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    let (release_set, cache_bytes) = activation_cache(&artifacts, None);
    let activation_cache_key = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        activation_cache_key,
        REGISTRY_PROGRAM_ID,
        cache_bytes,
    );

    let actor = actor_keypair().pubkey();
    // The Fractional root PDA is the reserve Position owner, but its own seeds
    // name the Core Market that only the shared fixture can derive. Compile the
    // fixture once with a placeholder reserve owner to learn the Market, then
    // recompile against the real root. The Market must not move between the two
    // compilations; if it ever does, this assertion says so rather than letting
    // a stale identity reach the chain.
    let probe = compile_shared(release_set, actor, Pubkey::new_from_array([0xef; 32]));
    let core_market = probe.core_market;

    // The Fractional representation width must equal the Claims runtime width,
    // so the terms name one Mint per outcome. Only the selected coordinate's
    // Mint is touched by a wrap; the rest exist as identities.
    let shard_mints: Vec<[u8; 32]> = (0..FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2)
        .map(|index| {
            let mut bytes = [0x77_u8; 32];
            let index = u32::try_from(index).expect("representation coordinate");
            bytes[0..4].copy_from_slice(&index.to_le_bytes());
            bytes
        })
        .collect();
    let shard_mint = Pubkey::new_from_array(shard_mints[OUTCOME as usize]);
    let behavior_bytes = TokenBehaviorSelectionV2::new(REALM_ID, release_set)
        .expect("token behavior selection")
        .to_bytes()
        .to_vec();
    let behavior_record = finalized(
        REGISTRY_PROGRAM_ID,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        behavior_bytes,
    );

    let terms_width = fractional_exposure_terms_bytes_v2(shard_mints.len()).expect("terms width");
    let mut terms_scratch = vec![0_u8; terms_width];
    let mut terms_bytes = vec![0_u8; terms_width];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: core_market.to_bytes(),
            product_record: probe.product.digest,
            result_domain: probe.result_domain.digest,
            release_set,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: behavior_record.digest,
            exposure_id: EXPOSURE_ID,
            product_basis: probe.linked_basis.digest,
            representation_basis: probe.semantic_basis_id,
            graph_id: GRAPH_ID,
            product_width: probe.outcome_count,
            denominator: DENOMINATOR,
            shard_mints: &shard_mints,
        },
        &mut terms_scratch,
        &mut terms_bytes,
    )
    .expect("exact Fractional exposure terms");
    let terms_record = finalized(
        REGISTRY_PROGRAM_ID,
        dclutch_fractional_claim_kernel::FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        terms_bytes,
    );

    let selection = CapabilityExecutionSelectionV1::new(
        0,
        ContentId::new([0x81; 32]).expect("manifest"),
        ContentId::new(dclutch_fractional_claim_contract::FRACTIONAL_CAPABILITY_KIND_ID_V1)
            .expect("kind"),
        ContentId::new([0x83; 32]).expect("capability release"),
        ContentId::new(terms_record.digest).expect("config"),
    )
    .expect("capability execution selection");
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).expect("release set"),
        core_market.to_bytes(),
        GENERATION,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .expect("capability root header");
    let (root, root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &TEST_CALLER_PROGRAM_ID);

    let shared = compile_shared(release_set, actor, root);
    assert_eq!(
        shared.core_market, core_market,
        "the shared fixture's Core Market must not depend on the Position owners"
    );
    assert_eq!(shared.product.digest, probe.product.digest);

    let root_state = FractionalRootV1::new(FractionalRootInputV1 {
        bump: root_bump,
        terms: terms_record.digest,
        market: core_market.to_bytes(),
        rent_beneficiary: actor.to_bytes(),
        revision: ROOT_REVISION,
        historical_rent_principal: 1,
    })
    .expect("fractional root state");
    let mut root_bytes = vec![0_u8; FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4];
    root_bytes.copy_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&root_state.to_bytes());

    let holder_token = Pubkey::new_from_array([0x78; 32]);

    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.linked_basis,
        &terms_record,
        &behavior_record,
    ] {
        add_finalized(&mut test, record);
    }
    add_account(
        &mut test,
        shared.core_market,
        CORE_PROGRAM_ID,
        shared.core_state.clone(),
    );
    add_account(
        &mut test,
        shared.claims_market,
        CLAIMS_PROGRAM_ID,
        shared.claims_market_bytes.clone(),
    );
    for position in shared.ordered_positions() {
        add_account(
            &mut test,
            position.account,
            CLAIMS_PROGRAM_ID,
            position.bytes.clone(),
        );
    }
    add_account(&mut test, root, TEST_CALLER_PROGRAM_ID, root_bytes);
    add_account(
        &mut test,
        shard_mint,
        token_program_id(),
        mint_bytes(root, 0),
    );
    add_account(
        &mut test,
        holder_token,
        token_program_id(),
        token_account_bytes_for(shard_mint, actor, 0),
    );
    add_account(&mut test, actor, system_program::ID, Vec::new());

    let mut arm = |action: FractionalExposureActionV2, test: &mut ProgramTest| -> ActionArm {
        // Wrap counts native Claims; WholeUnwrap counts shard atoms, and
        // divide_exposure_shards_v2 splits those back into whole Claims and
        // change. The inverse of wrapping 7 Claims is unwrapping 7 * D shards.
        let (source, destination, quantity) = match action {
            FractionalExposureActionV2::Wrap => {
                ([0; 32], holder_token.to_bytes(), WRAP_NATIVE_CLAIMS)
            }
            FractionalExposureActionV2::WholeUnwrap => (
                holder_token.to_bytes(),
                [0; 32],
                WRAP_NATIVE_CLAIMS * DENOMINATOR,
            ),
            _ => panic!("this campaign drives only the two atomic open-market actions"),
        };
        let request = FractionalExposureRequestV2::new(
            action,
            FractionalExposureRequestInputV2 {
                release_set,
                market: core_market.to_bytes(),
                product_record: shared.product.digest,
                result_domain: shared.result_domain.digest,
                terms: terms_record.digest,
                token_behavior: behavior_record.digest,
                exposure: EXPOSURE_ID,
                owner: actor.to_bytes(),
                source_token_account: source,
                destination_token_account: destination,
                terminal_digest: [0; 32],
                expected_revision: ROOT_REVISION,
                quantity,
                representation_coordinate: OUTCOME,
            },
        )
        .expect("canonical Fractional request");
        let request_bytes = request.to_bytes().expect("request bytes");
        let request_digest = hash(&request_bytes).to_bytes();
        let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
            release_set,
            core_market.to_bytes(),
            ExecutionRoleV1::Trading,
            terms_record.digest,
            request_digest,
        )
        .expect("caller authority seeds");
        let caller_authority =
            Pubkey::find_program_address(&caller_seeds.as_slices(), &TEST_CALLER_PROGRAM_ID).0;
        add_account(test, caller_authority, system_program::ID, Vec::new());
        let mut wrapper = Vec::with_capacity(FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES);
        wrapper.push(0);
        wrapper.extend_from_slice(&request_bytes);
        assert_eq!(wrapper.len(), FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES);
        ActionArm {
            caller_authority,
            wrapper,
        }
    };
    let wrap = arm(FractionalExposureActionV2::Wrap, &mut test);
    let unwrap = arm(FractionalExposureActionV2::WholeUnwrap, &mut test);

    (
        test,
        Fixture {
            shared,
            release_set,
            activation_cache: activation_cache_key,
            wrap,
            unwrap,
            root,
            terms_record,
            behavior_record,
            shard_mint,
            holder_token,
            actor,
        },
    )
}

/// The exact 31-account production frame, in contract coordinate order.
fn child_accounts(fixture: &Fixture, arm: &ActionArm) -> Vec<AccountMeta> {
    let shared = &fixture.shared;
    // Claims recomputes the two Position coordinates sorted by owner bytes.
    let [position_0, position_1] = shared.ordered_positions();
    let accounts = vec![
        AccountMeta::new_readonly(arm.caller_authority, false),
        AccountMeta::new(shared.claims_market, false),
        AccountMeta::new_readonly(shared.linked_basis.raw, false),
        AccountMeta::new_readonly(shared.linked_basis.staging, false),
        AccountMeta::new_readonly(shared.product.raw, false),
        AccountMeta::new_readonly(shared.product.staging, false),
        AccountMeta::new_readonly(shared.result_domain.raw, false),
        AccountMeta::new_readonly(shared.result_domain.staging, false),
        AccountMeta::new_readonly(shared.portfolio.raw, false),
        AccountMeta::new_readonly(shared.portfolio.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(shared.core_market, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(TEST_CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(TEST_CALLER_PROGRAM_ID), false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        AccountMeta::new(position_0.account, false),
        AccountMeta::new(position_1.account, false),
        AccountMeta::new_readonly(fixture.terms_record.raw, false),
        AccountMeta::new_readonly(fixture.terms_record.staging, false),
        AccountMeta::new_readonly(fixture.behavior_record.raw, false),
        AccountMeta::new_readonly(fixture.behavior_record.staging, false),
        AccountMeta::new(fixture.root, false),
        AccountMeta::new_readonly(fixture.actor, true),
        AccountMeta::new(fixture.shard_mint, false),
        AccountMeta::new(fixture.holder_token, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    assert_eq!(accounts.len(), FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3);
    accounts
}

fn instruction(fixture: &Fixture, arm: &ActionArm, fail_after: bool) -> Instruction {
    let mut accounts = Vec::with_capacity(FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3 + 1);
    accounts.push(AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false));
    accounts.extend(child_accounts(fixture, arm));
    let mut data = arm.wrapper.clone();
    *data.first_mut().expect("control byte") = u8::from(fail_after);
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("existing account")
}

/// Observe the four accounts a Fractional atomic action can move.
async fn observe(context: &mut ProgramTestContext, keys: [Pubkey; 4]) -> [Account; 4] {
    let mut observed = Vec::with_capacity(keys.len());
    for key in keys {
        observed.push(account(context, key).await);
    }
    observed.try_into().expect("four observed accounts")
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
) -> (
    bool,
    Vec<String>,
    u64,
    Result<(), solana_sdk::transaction::TransactionError>,
) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let actor = actor_keypair();
    let payer = context.payer.insecure_clone();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &actor],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("transaction processing");
    let accepted = processed.result.is_ok();
    let (logs, units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    (accepted, logs, units, processed.result)
}

fn custom_refusal(result: &Result<(), solana_sdk::transaction::TransactionError>) -> Option<u32> {
    match result {
        Err(solana_sdk::transaction::TransactionError::InstructionError(
            _,
            solana_sdk::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    }
}

/// Read one LBV2 balance out of a Position account.
fn balance(account: &Account, coordinate: usize) -> u64 {
    const POSITION_HEADER_BYTES_V2: usize = 128;
    let at = POSITION_HEADER_BYTES_V2 + coordinate * 8;
    u64::from_le_bytes(account.data[at..at + 8].try_into().unwrap())
}

fn mint_supply(account: &Account) -> u64 {
    u64::from_le_bytes(account.data[36..44].try_into().unwrap())
}

fn token_amount(account: &Account) -> u64 {
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

/// LBV2 Position replay revision.
fn position_revision(account: &Account) -> u64 {
    u64::from_le_bytes(account.data[16..24].try_into().unwrap())
}

fn root_revision(account: &Account) -> u64 {
    let at = FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4 + 112;
    u64::from_le_bytes(account.data[at..at + 8].try_into().unwrap())
}

/// The production Fractional wrap, executed end to end and committed.
///
/// This is the first committed state change the shipped `fractional_atomic_v3`
/// handler has ever produced. Real ELFs throughout: Claims, Registry, Core,
/// Token-2022 v11 and the two-PDA test caller. One transaction locks native
/// Claims into the Fractional root's reserve Position and mints the exact
/// denominator multiple of shards to the holder, and the campaign checks every
/// side of that conservation rather than just that the transaction passed.
#[tokio::test]
async fn the_production_fractional_wrap_locks_native_claims_and_mints_the_denominator_multiple() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let [position_0, position_1] = fixture.shared.ordered_positions();
    let (actor_account, reserve_account) = (
        fixture.shared.actor_position.account,
        fixture.shared.reserve_position.account,
    );
    assert_ne!(position_0.account, position_1.account);

    let mint_before = account(&mut context, fixture.shard_mint).await;
    let holder_before = account(&mut context, fixture.holder_token).await;
    let root_before = account(&mut context, fixture.root).await;
    let actor_before = account(&mut context, actor_account).await;
    let reserve_before = account(&mut context, reserve_account).await;
    assert_eq!(mint_supply(&mint_before), 0);
    assert_eq!(token_amount(&holder_before), 0);
    assert_eq!(balance(&actor_before, OUTCOME as usize), ACTOR_FUNDED_BALANCE);
    assert_eq!(balance(&reserve_before, OUTCOME as usize), 0);
    assert_eq!(root_revision(&root_before), ROOT_REVISION);

    let (accepted, logs, units, result) = submit(&mut context, instruction(&fixture, &fixture.wrap, false)).await;
    assert!(accepted, "the production Fractional wrap must commit: {result:?}");
    assert!(
        logs.iter()
            .any(|line| line.contains(&CLAIMS_PROGRAM_ID.to_string())
                && line.contains("invoke [2]")),
        "the real Claims ELF must be entered as a child of the test caller"
    );
    assert!(
        logs.iter()
            .any(|line| line.contains(&token_program_id().to_string())),
        "Token-2022 must be entered for the mint effect"
    );
    assert!(units > 30_000 && units <= 1_400_000, "{units} compute units");

    let mint_after = account(&mut context, fixture.shard_mint).await;
    let holder_after = account(&mut context, fixture.holder_token).await;
    let root_after = account(&mut context, fixture.root).await;
    let actor_after = account(&mut context, actor_account).await;
    let reserve_after = account(&mut context, reserve_account).await;

    // Shards are the exact denominator multiple of the native Claims locked.
    let expected_shards = WRAP_NATIVE_CLAIMS * DENOMINATOR;
    assert_eq!(mint_supply(&mint_after), expected_shards);
    assert_eq!(token_amount(&holder_after), expected_shards);
    // Every minted shard is held by the actor: supply is not created elsewhere.
    assert_eq!(mint_supply(&mint_after), token_amount(&holder_after));

    // Native Claims moved from the actor into the root's reserve, conserved.
    assert_eq!(
        balance(&actor_after, OUTCOME as usize),
        ACTOR_FUNDED_BALANCE - WRAP_NATIVE_CLAIMS
    );
    assert_eq!(balance(&reserve_after, OUTCOME as usize), WRAP_NATIVE_CLAIMS);
    assert_eq!(
        balance(&actor_after, OUTCOME as usize) + balance(&reserve_after, OUTCOME as usize),
        ACTOR_FUNDED_BALANCE,
        "native Claims are conserved across the wrap"
    );

    // The root is owned by the Trading program, so Claims cannot and does not
    // write it: advancing the replay revision is the Trading parent's
    // responsibility, and this caller deliberately does not stand in for that.
    // Claims' authority over the root is to *authenticate* it and require its
    // signature, which the accepted transaction above already proves.
    assert_eq!(root_revision(&root_after), ROOT_REVISION);
    assert_eq!(
        root_before.data, root_after.data,
        "Claims must not mutate an account owned by the Trading program"
    );
}

/// A caller refusal after Claims committed rolls the whole transaction back.
///
/// The caller validates the receipt Claims returned and only then refuses, so
/// at the moment of refusal the Claims mutation, the Token-2022 mint and the
/// root revision bump had all really happened inside the transaction. Nothing
/// may survive it. This is the property that makes the Fractional route safe to
/// compose under a Trading parent that can fail late.
#[tokio::test]
async fn a_late_caller_refusal_after_the_real_claims_commit_rolls_everything_back() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let actor_account = fixture.shared.actor_position.account;
    let reserve_account = fixture.shared.reserve_position.account;

    let before = [
        account(&mut context, fixture.shard_mint).await,
        account(&mut context, fixture.holder_token).await,
        account(&mut context, fixture.root).await,
        account(&mut context, actor_account).await,
        account(&mut context, reserve_account).await,
    ];

    let (accepted, _logs, _units, result) = submit(&mut context, instruction(&fixture, &fixture.wrap, true)).await;
    assert!(!accepted, "the deliberate late refusal must abort");
    assert_eq!(
        custom_refusal(&result),
        Some(0x10_B004),
        "it must be the caller's own late-failure code, not an earlier refusal \
         that would mean the commit never happened"
    );

    let after = [
        account(&mut context, fixture.shard_mint).await,
        account(&mut context, fixture.holder_token).await,
        account(&mut context, fixture.root).await,
        account(&mut context, actor_account).await,
        account(&mut context, reserve_account).await,
    ];
    assert_eq!(before, after, "a refused Fractional wrap leaves no trace");
}

/// Wrap then WholeUnwrap returns every account to its exact opening bytes.
///
/// WholeUnwrap is the published inverse of Wrap and has its own shipped handler,
/// so the pair is the real conservation statement: the shards are burned rather
/// than parked somewhere, and the native Claims come back out of the root's
/// reserve to the actor. Two separate transactions against real ELFs, so the
/// second one reads the first one's committed chain state rather than a fixture.
#[tokio::test]
async fn wrap_then_whole_unwrap_restores_the_exact_opening_state() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let actor_account = fixture.shared.actor_position.account;
    let reserve_account = fixture.shared.reserve_position.account;
    let keys = [
        fixture.shard_mint,
        fixture.holder_token,
        actor_account,
        reserve_account,
    ];

    let opening = observe(&mut context, keys).await;

    let (wrapped, _logs, _units, result) =
        submit(&mut context, instruction(&fixture, &fixture.wrap, false)).await;
    assert!(wrapped, "wrap must commit: {result:?}");
    let mid = observe(&mut context, keys).await;
    let expected_shards = WRAP_NATIVE_CLAIMS * DENOMINATOR;
    assert_eq!(mint_supply(&mid[0]), expected_shards);
    assert_eq!(token_amount(&mid[1]), expected_shards);
    assert_eq!(balance(&mid[3], OUTCOME as usize), WRAP_NATIVE_CLAIMS);

    let (unwrapped, logs, units, result) =
        submit(&mut context, instruction(&fixture, &fixture.unwrap, false)).await;
    assert!(unwrapped, "whole unwrap must commit: {result:?}");
    assert!(
        logs.iter()
            .any(|line| line.contains(&CLAIMS_PROGRAM_ID.to_string())
                && line.contains("invoke [2]")),
        "the real Claims ELF must run the unwrap too"
    );
    assert!(units > 30_000, "{units} compute units");

    let closing = observe(&mut context, keys).await;
    assert_eq!(mint_supply(&closing[0]), 0, "every shard must be burned");
    assert_eq!(token_amount(&closing[1]), 0);
    assert_eq!(
        balance(&closing[2], OUTCOME as usize),
        ACTOR_FUNDED_BALANCE,
        "the actor gets its native Claims back"
    );
    assert_eq!(balance(&closing[3], OUTCOME as usize), 0);

    // Every balance is restored, but the pair is not a no-op: each committed
    // transaction advances the Position replay revision, which is exactly what
    // stops a wrap or an unwrap being replayed. Asserting raw byte equality
    // here would be asserting that replay protection does not work.
    for (open, close) in opening.iter().zip(closing.iter()) {
        assert_eq!(open.data.len(), close.data.len());
        assert_eq!(open.owner, close.owner);
        assert_eq!(open.lamports, close.lamports);
    }
    for index in [2, 3] {
        assert_eq!(
            position_revision(&closing[index]),
            position_revision(&opening[index]) + 2,
            "each of the two committed transactions advances the Position revision"
        );
    }
    assert_eq!(
        opening[0].data[36..44],
        closing[0].data[36..44],
        "the Mint supply returns to its opening value"
    );
}

/// Transfer is a Token-2022 movement and never enters the Claims family route.
///
/// A shard transfer moves an already-issued representation between holders. It
/// creates and destroys no native Claims, so there is nothing for Claims to
/// authorise and no Claims handler exists for it -- `fractional_atomic_v3`
/// dispatches only the two open-market actions and the two terminal ones. This
/// pins that from both directions: the contract routes Transfer to Token-2022
/// directly, and a Transfer pushed at the family caller is refused rather than
/// quietly forwarded into Claims.
#[tokio::test]
async fn a_transfer_is_refused_by_the_family_caller_and_routed_direct_to_token_2022() {
    use dclutch_fractional_claim_contract::{
        FractionalChildRouteV3, plan_fractional_physical_v3,
    };
    use dclutch_fractional_claim_kernel::{
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
        FractionalExposureTermsV2,
    };

    let (test, fixture) = fixture();

    let transfer = FractionalExposureRequestV2::new(
        FractionalExposureActionV2::Transfer,
        FractionalExposureRequestInputV2 {
            release_set: fixture.release_set,
            market: fixture.shared.core_market.to_bytes(),
            product_record: fixture.shared.product.digest,
            result_domain: fixture.shared.result_domain.digest,
            terms: fixture.terms_record.digest,
            token_behavior: fixture.behavior_record.digest,
            exposure: EXPOSURE_ID,
            owner: fixture.actor.to_bytes(),
            source_token_account: fixture.holder_token.to_bytes(),
            destination_token_account: [0x79; 32],
            terminal_digest: [0; 32],
            expected_revision: ROOT_REVISION,
            quantity: DENOMINATOR,
            representation_coordinate: OUTCOME,
        },
    )
    .expect("canonical Fractional transfer request");

    // The contract routes it away from Claims entirely.
    let terms = FractionalExposureTermsV2::decode(
        &fixture.terms_record.bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: fixture.terms_record.digest,
            finalized_terms_id: fixture.terms_record.digest,
            recomputed_terms_digest: fixture.terms_record.digest,
            finalized_terms_digest: fixture.terms_record.digest,
            record_authenticated: true,
        },
    )
    .expect("terms decode");
    let plan = plan_fractional_physical_v3(terms, transfer).expect("physical transfer plan");
    assert_eq!(plan.route, FractionalChildRouteV3::Token2022DirectTransfer);
    assert_eq!(plan.whole_claims, 0, "a transfer moves no native Claims");

    // And the family caller refuses to carry it into Claims.
    let mut context = test.start_with_context().await;
    let mut wrapper = Vec::with_capacity(FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES);
    wrapper.push(0);
    wrapper.extend_from_slice(&transfer.to_bytes().expect("transfer bytes"));
    let mut hostile = instruction(&fixture, &fixture.wrap, false);
    hostile.data = wrapper;
    let (accepted, _logs, _units, result) = submit(&mut context, hostile).await;
    assert!(!accepted);
    assert_eq!(
        custom_refusal(&result),
        Some(0x10_B000),
        "the caller must refuse a Transfer as an unsupported instruction"
    );
}

/// The representation width bound is exactly 256, and it is load-bearing.
///
/// A Fractional capability names one shard Mint per representation coordinate
/// and the Market dispatches on a `U8` action selector at a fixed request
/// offset, so 256 is the arithmetic bound of the index space rather than a
/// storage budget. This pins both sides of it: the last admissible width and
/// the first refused one, in the terms codec and in the fixture alike. The
/// shared 258-outcome Claims fixture is therefore permanently out of Fractional
/// range, which is why this campaign compiles its own.
#[test]
fn the_fractional_representation_width_bound_is_exactly_256() {
    assert_eq!(FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2, 256);
    assert!(fractional_exposure_terms_bytes_v2(FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2).is_ok());
    assert!(fractional_exposure_terms_bytes_v2(FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2 + 1).is_err());

    let admissible = compile_narrow_fixture_v2(NarrowFixtureInputV2 {
        outcome_count: FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2,
        registry_program: REGISTRY_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        release_set: [0x11; 32],
        realm_id: REALM_ID,
        custody_context: CUSTODY_CONTEXT,
        generation: GENERATION,
        actor_owner: Pubkey::new_from_array([0x01; 32]),
        reserve_owner: Pubkey::new_from_array([0x02; 32]),
        funded_coordinate: 0,
        funded_balance: 1,
        reserve_balance: 0,
        terminal: None,
        rent_beneficiary: RENT_CREDIT,
        graph_id: GRAPH_ID,
        exposure_id: EXPOSURE_ID,
    });
    assert_eq!(
        admissible.expect("the bound itself must compile").outcome_count,
        256
    );
    let refused = compile_narrow_fixture_v2(NarrowFixtureInputV2 {
        outcome_count: FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2 + 1,
        registry_program: REGISTRY_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        release_set: [0x11; 32],
        realm_id: REALM_ID,
        custody_context: CUSTODY_CONTEXT,
        generation: GENERATION,
        actor_owner: Pubkey::new_from_array([0x01; 32]),
        reserve_owner: Pubkey::new_from_array([0x02; 32]),
        funded_coordinate: 0,
        funded_balance: 1,
        reserve_balance: 0,
        terminal: None,
        rent_beneficiary: RENT_CREDIT,
        graph_id: GRAPH_ID,
        exposure_id: EXPOSURE_ID,
    });
    assert_eq!(refused, Err(NarrowFixtureError::Width));
}

// ---------------------------------------------------------------- terminal
//
// The Fractional terminal frame is the 36-account Claims terminal-settlement
// frame plus the 8-coordinate Fractional tail. Coordinates 0..36 are the same
// frame the Rational campaign drives, so the Custody composition below is a
// port of that staging rather than a second invention.

use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_resolution_codec::{ResolutionCertificateKindV2, ResolutionCertificateV2};
use dclutch_token_svm::PRODUCTION_ADAPTER_RELEASES;

const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa4; 32]);
const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0x74; 32]);
const RECIPIENT_TOKEN: Pubkey = Pubkey::new_from_array([0x85; 32]);
const CERTIFICATE_ACCOUNT: Pubkey = Pubkey::new_from_array([0x86; 32]);
const RESOLUTION_POLICY: [u8; 32] = [0x51; 32];
const HOARD_ATOMS: u64 = 10_000;
const RECIPIENT_ATOMS: u64 = 0;
const CUSTODY_EXPECTED_REVISION: u64 = 1;
/// Shards the actor already holds when a terminal campaign opens: the market
/// resolved after a wrap, so the reserve is locked and the shards are issued.
const TERMINAL_SHARDS: u64 = WRAP_NATIVE_CLAIMS * DENOMINATOR;
/// A coordinate that is not the winner, so its terminal payout is exactly zero.
const LOSING_COORDINATE: u32 = 1;
/// Representation width the terminal campaign runs at.
///
/// The terminal route translates every Product result coordinate onto every
/// Claims representation root, so its compute cost grows with the width in a
/// way the open-market actions' does not. See the ceiling campaign below.
const TERMINAL_WIDTH: usize = 8;
/// The supported Fractional representation width.
///
/// Must equal `FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4` in
/// `dclutch-fractional-claim-operator`, which refuses to publish a capability
/// above it. The two are duplicated rather than shared because this campaign is
/// its own workspace; the operator's constant carries the same measurement and
/// names this test.
const SUPPORTED_TERMINAL_WIDTH: usize = 64;

/// Base Token-2022 Mint with no mint or freeze authority.
///
/// The Realm's `RequireAbsent` policies are why the collateral Mint carries
/// neither authority; unlike the shard Mints it has no extensions.
fn collateral_mint_bytes(supply: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; 82];
    put(&mut bytes, 0, &0_u32.to_le_bytes());
    put(&mut bytes, 36, &supply.to_le_bytes());
    *bytes.get_mut(44).expect("decimals") = 6;
    *bytes.get_mut(45).expect("initialized") = 1;
    put(&mut bytes, 46, &0_u32.to_le_bytes());
    bytes
}

struct TerminalFixture {
    shared: NarrowFixtureV2,
    release_set: [u8; 32],
    activation_cache: Pubkey,
    caller_authority: Pubkey,
    root: Pubkey,
    terms_record: NarrowRecordV2,
    behavior_record: NarrowRecordV2,
    realm_record: NarrowRecordV2,
    shard_mint: Pubkey,
    holder_token: Pubkey,
    actor: Pubkey,
    custody_replay: Pubkey,
    custody_authority: Pubkey,
    hoard: Pubkey,
    wrapper: Vec<u8>,
}

/// Stage a resolved Market with its full Custody composition.
///
/// `winner` selects the terminal outcome, which is what separates the two
/// terminal actions: burning shards at the winner redeems collateral, burning
/// them anywhere else pays exactly zero. Everything else is identical.
fn terminal_fixture(
    width: usize,
    winner: u32,
    action: FractionalExposureActionV2,
) -> (ProgramTest, TerminalFixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        ("dclutch_claims_sbf", CLAIMS_PROGRAM_ID, artifacts.claims.as_slice()),
        ("dclutch_registry_sbf", REGISTRY_PROGRAM_ID, artifacts.registry.as_slice()),
        ("dclutch_core_sbf", CORE_PROGRAM_ID, artifacts.core.as_slice()),
        ("dclutch_custody_sbf", CUSTODY_PROGRAM_ID, artifacts.custody.as_slice()),
        (
            "dclutch_fractional_atomic_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
        ("spl_token_2022", token_program_id(), artifacts.token.as_slice()),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    let (release_set, cache_bytes) = activation_cache(&artifacts, Some(CUSTODY_PROGRAM_ID));
    let activation_cache_key = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, activation_cache_key, REGISTRY_PROGRAM_ID, cache_bytes);

    // The Realm commits to the collateral Mint, and its own content digest is
    // the Realm identity the Market and every Custody coordinate are keyed by.
    let adapter = PRODUCTION_ADAPTER_RELEASES
        .get(1)
        .copied()
        .expect("Token-2022 production adapter");
    let realm_bytes = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: COLLATERAL_MINT.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical Realm")
    .to_bytes()
    .to_vec();
    let realm_record = finalized(REGISTRY_PROGRAM_ID, REALM_SCHEMA_RELEASE_ID_V1, realm_bytes);
    let realm_id = realm_record.digest;

    let actor = actor_keypair().pubkey();
    let terminal = NarrowTerminalInputV2 {
        winner,
        receipt: CERTIFICATE_ACCOUNT.to_bytes(),
    };
    let compile = |reserve_owner: Pubkey| {
        compile_narrow_fixture_v2(NarrowFixtureInputV2 {
            outcome_count: width,
            registry_program: REGISTRY_PROGRAM_ID,
            core_program: CORE_PROGRAM_ID,
            claims_program: CLAIMS_PROGRAM_ID,
            release_set,
            realm_id,
            custody_context: CUSTODY_CONTEXT,
            generation: GENERATION,
            actor_owner: actor,
            reserve_owner,
            funded_coordinate: OUTCOME as usize,
            funded_balance: ACTOR_FUNDED_BALANCE - WRAP_NATIVE_CLAIMS,
            reserve_balance: WRAP_NATIVE_CLAIMS,
            terminal: Some(terminal),
            rent_beneficiary: RENT_CREDIT,
            graph_id: GRAPH_ID,
            exposure_id: EXPOSURE_ID,
        })
        .expect("narrow terminal fixture")
    };
    let probe = compile(Pubkey::new_from_array([0xef; 32]));
    let core_market = probe.core_market;

    let shard_mints: Vec<[u8; 32]> = (0..width)
        .map(|index| {
            let mut bytes = [0x77_u8; 32];
            let index = u32::try_from(index).expect("representation coordinate");
            bytes[0..4].copy_from_slice(&index.to_le_bytes());
            bytes
        })
        .collect();
    let shard_mint = Pubkey::new_from_array(shard_mints[OUTCOME as usize]);
    let behavior_record = finalized(
        REGISTRY_PROGRAM_ID,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        TokenBehaviorSelectionV2::new(realm_id, release_set)
            .expect("token behavior selection")
            .to_bytes()
            .to_vec(),
    );
    let terms_width = fractional_exposure_terms_bytes_v2(shard_mints.len()).expect("terms width");
    let mut terms_scratch = vec![0_u8; terms_width];
    let mut terms_bytes = vec![0_u8; terms_width];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: core_market.to_bytes(),
            product_record: probe.product.digest,
            result_domain: probe.result_domain.digest,
            release_set,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: behavior_record.digest,
            exposure_id: EXPOSURE_ID,
            product_basis: probe.linked_basis.digest,
            representation_basis: probe.semantic_basis_id,
            graph_id: GRAPH_ID,
            product_width: probe.outcome_count,
            denominator: DENOMINATOR,
            shard_mints: &shard_mints,
        },
        &mut terms_scratch,
        &mut terms_bytes,
    )
    .expect("exact Fractional exposure terms");
    let terms_record = finalized(
        REGISTRY_PROGRAM_ID,
        dclutch_fractional_claim_kernel::FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        terms_bytes,
    );

    let selection = CapabilityExecutionSelectionV1::new(
        0,
        ContentId::new([0x81; 32]).expect("manifest"),
        ContentId::new(dclutch_fractional_claim_contract::FRACTIONAL_CAPABILITY_KIND_ID_V1)
            .expect("kind"),
        ContentId::new([0x83; 32]).expect("capability release"),
        ContentId::new(terms_record.digest).expect("config"),
    )
    .expect("capability execution selection");
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).expect("release set"),
        core_market.to_bytes(),
        GENERATION,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .expect("capability root header");
    let (root, root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &TEST_CALLER_PROGRAM_ID);

    // The reserve Position belongs to the root: it holds the native Claims the
    // wrap locked, and the terminal settlement redeems out of it.
    let shared = compile(root);
    assert_eq!(shared.core_market, core_market);

    let root_state = FractionalRootV1::new(FractionalRootInputV1 {
        bump: root_bump,
        terms: terms_record.digest,
        market: core_market.to_bytes(),
        rent_beneficiary: actor.to_bytes(),
        revision: ROOT_REVISION,
        historical_rent_principal: 1,
    })
    .expect("fractional root state");
    let mut root_bytes = vec![0_u8; FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4];
    root_bytes.copy_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&root_state.to_bytes());

    let holder_token = Pubkey::new_from_array([0x78; 32]);
    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.linked_basis,
        &shared.exposure,
        &terms_record,
        &behavior_record,
        &realm_record,
    ] {
        add_finalized(&mut test, record);
    }
    add_account(&mut test, shared.core_market, CORE_PROGRAM_ID, shared.core_state.clone());
    add_account(
        &mut test,
        shared.claims_market,
        CLAIMS_PROGRAM_ID,
        shared.claims_market_bytes.clone(),
    );
    for position in shared.ordered_positions() {
        add_account(&mut test, position.account, CLAIMS_PROGRAM_ID, position.bytes.clone());
    }
    add_account(&mut test, root, TEST_CALLER_PROGRAM_ID, root_bytes);
    add_account(&mut test, shard_mint, token_program_id(), mint_bytes(root, TERMINAL_SHARDS));
    add_account(
        &mut test,
        holder_token,
        token_program_id(),
        token_account_bytes_for(shard_mint, actor, TERMINAL_SHARDS),
    );

    // The certificate is a flat account whose authority is that Core's state
    // names it; its own bytes must rejoin every Core identity.
    let certificate_bytes = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: core_market.to_bytes(),
        route: [0x87; 32],
        source_material: RESOLUTION_POLICY,
        product_record_digest: shared.product.digest,
        provider_evidence: [0x88; 32],
        funding_allocation: [0; 32],
        receipt_account: CERTIFICATE_ACCOUNT.to_bytes(),
        generation: GENERATION,
        attempt_index: 0,
        schedule_index: 0,
        selector: winner,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: 1,
        result_denominator: 1,
        observed_at: 1,
    }
    .to_bytes()
    .expect("canonical Resolution certificate");
    add_account(&mut test, CERTIFICATE_ACCOUNT, CLAIMS_PROGRAM_ID, certificate_bytes.to_vec());

    let custody_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            core_market.as_ref(),
            release_set.as_slice(),
        ],
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            core_market.to_bytes(),
            release_set,
            CUSTODY_CONTEXT,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            core_market.to_bytes(),
            release_set,
            CallerRoleV1::Claims,
            CUSTODY_CONTEXT,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        COLLATERAL_MINT,
        token_program_id(),
        collateral_mint_bytes(HOARD_ATOMS + RECIPIENT_ATOMS),
    );
    add_account(
        &mut test,
        hoard,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, custody_authority, HOARD_ATOMS),
    );
    add_account(
        &mut test,
        RECIPIENT_TOKEN,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, actor, RECIPIENT_ATOMS),
    );
    // The replay is NOT planted: a Claims-role cursor is a prestate no route in
    // the tree can produce, so the campaign creates it by executing the real
    // route first. All that is staged is an empty, rent-funded system account.
    test.add_account(
        custody_replay,
        Account {
            lamports: Rent::default().minimum_balance(CUSTODY_REPLAY_BYTES_V1) * 2,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    add_account(&mut test, RENT_CREDIT, system_program::ID, Vec::new());
    test.add_account(
        actor,
        Account {
            lamports: 1_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let request = FractionalExposureRequestV2::new(
        action,
        FractionalExposureRequestInputV2 {
            release_set,
            market: core_market.to_bytes(),
            product_record: shared.product.digest,
            result_domain: shared.result_domain.digest,
            terms: terms_record.digest,
            token_behavior: behavior_record.digest,
            exposure: EXPOSURE_ID,
            owner: actor.to_bytes(),
            source_token_account: holder_token.to_bytes(),
            destination_token_account: [0; 32],
            terminal_digest: CERTIFICATE_ACCOUNT.to_bytes(),
            expected_revision: ROOT_REVISION,
            quantity: TERMINAL_SHARDS,
            representation_coordinate: OUTCOME,
        },
    )
    .expect("canonical Fractional terminal request");
    let request_bytes = request.to_bytes().expect("request bytes");
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        core_market.to_bytes(),
        ExecutionRoleV1::Trading,
        terms_record.digest,
        hash(&request_bytes).to_bytes(),
    )
    .expect("caller authority seeds");
    let caller_authority =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &TEST_CALLER_PROGRAM_ID).0;
    add_account(&mut test, caller_authority, system_program::ID, Vec::new());
    let mut wrapper = Vec::with_capacity(FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES);
    wrapper.push(0);
    wrapper.extend_from_slice(&request_bytes);

    (
        test,
        TerminalFixture {
            shared,
            release_set,
            activation_cache: activation_cache_key,
            caller_authority,
            root,
            terms_record,
            behavior_record,
            realm_record,
            shard_mint,
            holder_token,
            actor,
            custody_replay,
            custody_authority,
            hoard,
            wrapper,
        },
    )
}

/// The exact 44-account production terminal frame, in contract coordinate order.
fn terminal_child_accounts(fixture: &TerminalFixture, custody_caller: Pubkey) -> Vec<AccountMeta> {
    let shared = &fixture.shared;
    let accounts = vec![
        AccountMeta::new_readonly(fixture.caller_authority, false),
        AccountMeta::new(shared.claims_market, false),
        AccountMeta::new_readonly(shared.linked_basis.raw, false),
        AccountMeta::new_readonly(shared.linked_basis.staging, false),
        AccountMeta::new_readonly(shared.product.raw, false),
        AccountMeta::new_readonly(shared.product.staging, false),
        AccountMeta::new_readonly(shared.result_domain.raw, false),
        AccountMeta::new_readonly(shared.result_domain.staging, false),
        AccountMeta::new_readonly(shared.portfolio.raw, false),
        AccountMeta::new_readonly(shared.portfolio.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(shared.core_market, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(TEST_CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(TEST_CALLER_PROGRAM_ID), false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        // The single Position is the ROOT's reserve: it holds the native Claims
        // the wrap locked, and the terminal settlement redeems out of it.
        AccountMeta::new(shared.reserve_position.account, false),
        AccountMeta::new_readonly(shared.exposure.raw, false),
        AccountMeta::new_readonly(shared.exposure.staging, false),
        // On the zero-payout path this must literally be the Claims program;
        // a paying redemption instead needs the derived caller authority.
        AccountMeta::new_readonly(custody_caller, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CERTIFICATE_ACCOUNT, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(fixture.realm_record.raw, false),
        AccountMeta::new_readonly(fixture.realm_record.staging, false),
        AccountMeta::new(fixture.custody_replay, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new(fixture.hoard, false),
        AccountMeta::new(RECIPIENT_TOKEN, false),
        AccountMeta::new_readonly(fixture.custody_authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(fixture.terms_record.raw, false),
        AccountMeta::new_readonly(fixture.terms_record.staging, false),
        AccountMeta::new_readonly(fixture.behavior_record.raw, false),
        AccountMeta::new_readonly(fixture.behavior_record.staging, false),
        AccountMeta::new(fixture.root, false),
        AccountMeta::new_readonly(fixture.actor, true),
        AccountMeta::new(fixture.shard_mint, false),
        AccountMeta::new(fixture.holder_token, false),
    ];
    assert_eq!(accounts.len(), 44);
    accounts
}

fn terminal_instruction(
    fixture: &TerminalFixture,
    custody_caller: Pubkey,
    fail_after: bool,
) -> Instruction {
    let mut accounts = Vec::with_capacity(45);
    accounts.push(AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false));
    accounts.extend(terminal_child_accounts(fixture, custody_caller));
    let mut data = fixture.wrapper.clone();
    *data.first_mut().expect("control byte") = u8::from(fail_after);
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

/// Create the Claims-role Custody replay cursor by executing the real route.
///
/// Nothing may plant this account. A Claims-role cursor at this namespace is a
/// prestate no route in the tree can produce, so a planted one would be
/// evidence of nothing. The campaign creates it the way a redeemer would.
async fn create_custody_replay(context: &mut ProgramTestContext, fixture: &TerminalFixture) {
    use dclutch_claims_sbf::custody_replay_v1::expected_request_v1;
    use dclutch_claims_svm::custody_replay_v1::ClaimsCustodyReplayRequestV1;
    use dclutch_claims_svm::liability_basis_state_v2::LiabilityBasisMarketViewV2;

    let aggregate = LiabilityBasisMarketViewV2::decode(&fixture.shared.claims_market_bytes)
        .expect("aggregate decode");
    let rent = Rent::default();
    let request = expected_request_v1(
        aggregate,
        CLAIMS_PROGRAM_ID.to_bytes(),
        fixture.actor.to_bytes(),
        RENT_CREDIT.to_bytes(),
        rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
    )
    .expect("the sole Custody request this route sends");
    let request_bytes = request.to_bytes().expect("custody request bytes");
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            fixture.release_set,
            fixture.shared.core_market.to_bytes(),
            ExecutionRoleV1::Claims,
            request.context,
            hash(&request_bytes).to_bytes(),
        )
        .expect("claims custody caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let instruction = Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(caller, false),
            AccountMeta::new_readonly(fixture.shared.core_market, false),
            AccountMeta::new_readonly(fixture.activation_cache, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
            AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
            AccountMeta::new_readonly(fixture.realm_record.raw, false),
            AccountMeta::new_readonly(fixture.realm_record.staging, false),
            AccountMeta::new(fixture.custody_replay, false),
            AccountMeta::new(fixture.actor, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(RENT_CREDIT, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.shared.claims_market, false),
        ],
        data: ClaimsCustodyReplayRequestV1::new(fixture.shared.core_market.to_bytes())
            .expect("replay-creation request")
            .to_bytes()
            .to_vec(),
    };
    let (accepted, _logs, _units, result) = submit(context, instruction).await;
    assert!(accepted, "the Claims-role Custody replay must be creatable: {result:?}");

    let created = account(context, fixture.custody_replay).await;
    assert_eq!(created.owner, CUSTODY_PROGRAM_ID);
    assert_eq!(created.data.len(), CUSTODY_REPLAY_BYTES_V1);
    let replay = CustodyReplayV1::decode(&created.data).expect("created replay decodes");
    assert_eq!(replay.caller_role, CallerRoleV1::Claims);
    assert_eq!(replay.next_revision, CUSTODY_EXPECTED_REVISION);
    assert_eq!(replay.open_vault_count, 0);
}

#[tokio::test]
async fn the_claims_role_custody_replay_is_created_by_the_real_route() {
    let (test, fixture) =
        terminal_fixture(TERMINAL_WIDTH, OUTCOME, FractionalExposureActionV2::TerminalRedeem);
    let mut context = test.start_with_context().await;
    create_custody_replay(&mut context, &fixture).await;
}

fn terminal_observe(fixture: &TerminalFixture) -> [Pubkey; 5] {
    [
        fixture.shard_mint,
        fixture.holder_token,
        fixture.shared.reserve_position.account,
        fixture.hoard,
        RECIPIENT_TOKEN,
    ]
}

/// A losing coordinate burns its shards and pays exactly nothing.
///
/// TerminalZeroBurn is the settlement of a representation that resolved
/// worthless. The whole Custody composition is still authenticated -- replay,
/// Hoard, authority, realm, certificate -- but no Custody CPI fires, so the
/// replay revision must not move and no collateral may leave the Hoard. Claims
/// refuses the action outright if the payout is nonzero, so this is also what
/// pins the action to the economics rather than to the caller's say-so.
#[tokio::test]
async fn a_losing_coordinate_burns_its_shards_and_pays_exactly_nothing() {
    let (test, fixture) = terminal_fixture(
        TERMINAL_WIDTH,
        LOSING_COORDINATE,
        FractionalExposureActionV2::TerminalZeroBurn,
    );
    let mut context = test.start_with_context().await;
    create_custody_replay(&mut context, &fixture).await;

    let keys = terminal_observe(&fixture);
    let before = observe5(&mut context, keys).await;
    let replay_before = account(&mut context, fixture.custody_replay).await;
    assert_eq!(mint_supply(&before[0]), TERMINAL_SHARDS);
    assert_eq!(token_amount(&before[1]), TERMINAL_SHARDS);
    assert_eq!(balance(&before[2], OUTCOME as usize), WRAP_NATIVE_CLAIMS);

    let (accepted, logs, units, result) =
        submit(&mut context, terminal_instruction(&fixture, CLAIMS_PROGRAM_ID, false)).await;
    assert!(accepted, "the zero-payout terminal burn must commit: {result:?}");
    assert!(
        logs.iter()
            .any(|line| line.contains(&CLAIMS_PROGRAM_ID.to_string())
                && line.contains("invoke [2]"))
    );
    assert!(units > 30_000, "{units} compute units");

    let after = observe5(&mut context, keys).await;
    // The shards are gone and the locked native Claims are released.
    assert_eq!(mint_supply(&after[0]), 0, "every shard is burned");
    assert_eq!(token_amount(&after[1]), 0);
    assert_eq!(balance(&after[2], OUTCOME as usize), 0);
    // Not one collateral atom moved, in either direction.
    assert_eq!(token_amount(&after[3]), HOARD_ATOMS, "the Hoard is untouched");
    assert_eq!(token_amount(&after[4]), RECIPIENT_ATOMS);
    // And the Custody replay did not advance, because Custody never ran.
    let replay_after = account(&mut context, fixture.custody_replay).await;
    assert_eq!(
        CustodyReplayV1::decode(&replay_after.data)
            .expect("replay decodes")
            .next_revision,
        CUSTODY_EXPECTED_REVISION,
        "a zero payout fires no Custody CPI, so its cursor must not move"
    );
    assert_eq!(replay_before.data, replay_after.data);
}

async fn observe5(context: &mut ProgramTestContext, keys: [Pubkey; 5]) -> [Account; 5] {
    let mut observed = Vec::with_capacity(keys.len());
    for key in keys {
        observed.push(account(context, key).await);
    }
    observed.try_into().expect("five observed accounts")
}

/// The winning coordinate redeems collateral out of the Market's Hoard.
///
/// This is the whole Fractional lifecycle's far end: shards issued against
/// locked native Claims are burned at a resolved winning coordinate, and the
/// Market's Custody Hoard pays the holder. Unlike the zero-payout burn this
/// really does invoke Custody, which really does invoke Token-2022, so the
/// campaign checks collateral conservation across the Hoard and the recipient
/// and requires the Custody replay cursor to advance exactly once.
#[tokio::test]
async fn the_winning_coordinate_redeems_collateral_from_the_markets_hoard() {
    let (test, fixture) =
        terminal_fixture(TERMINAL_WIDTH, OUTCOME, FractionalExposureActionV2::TerminalRedeem);
    let mut context = test.start_with_context().await;
    create_custody_replay(&mut context, &fixture).await;

    let custody_caller = paying_custody_caller(&fixture, OUTCOME);
    let keys = terminal_observe(&fixture);
    let before = observe5(&mut context, keys).await;
    assert_eq!(mint_supply(&before[0]), TERMINAL_SHARDS);
    assert_eq!(token_amount(&before[3]), HOARD_ATOMS);
    assert_eq!(token_amount(&before[4]), RECIPIENT_ATOMS);

    let (accepted, logs, units, result) =
        submit(&mut context, terminal_instruction(&fixture, custody_caller, false)).await;
    assert!(accepted, "the terminal redemption must commit: {result:?}");
    assert!(
        logs.iter()
            .any(|line| line.contains(&CUSTODY_PROGRAM_ID.to_string())),
        "a paying redemption must actually enter the Custody program"
    );
    assert!(units > 30_000, "{units} compute units");

    let after = observe5(&mut context, keys).await;
    assert_eq!(mint_supply(&after[0]), 0, "every redeemed shard is burned");
    assert_eq!(token_amount(&after[1]), 0);
    assert_eq!(balance(&after[2], OUTCOME as usize), 0);

    // Collateral is conserved: whatever left the Hoard arrived at the recipient.
    let paid = token_amount(&after[4]) - token_amount(&before[4]);
    assert!(paid > 0, "the winning coordinate must actually pay");
    assert_eq!(
        token_amount(&before[3]) - token_amount(&after[3]),
        paid,
        "every atom the recipient gained left the Hoard"
    );
    assert_eq!(token_amount(&after[3]) + token_amount(&after[4]), HOARD_ATOMS + RECIPIENT_ATOMS);

    // A paying redemption advances the Custody cursor exactly once.
    let replay_after = account(&mut context, fixture.custody_replay).await;
    assert_eq!(
        CustodyReplayV1::decode(&replay_after.data)
            .expect("replay decodes")
            .next_revision,
        CUSTODY_EXPECTED_REVISION + 1,
    );
}

/// A paying redemption needs the derived Custody caller, not the Claims program.
///
/// On the zero-payout path coordinate 23 must literally BE the Claims program,
/// because no Custody CPI is signed for. The moment the payout is nonzero that
/// same coordinate must instead be the `CallerAuthoritySeedsV1` PDA under Claims
/// whose seeds commit to the exact CustodyRequestV1 -- and therefore to the
/// payout and the signed-delta packet. The two paths want different accounts at
/// one coordinate, which is worth pinning: passing the zero-payout shape into a
/// paying redemption is refused rather than paying out under a caller nobody
/// derived.
///
/// Deriving that address host-side is the remaining step for a committed
/// Fractional redemption; the settlement itself is proven by the zero-payout
/// campaign above, which authenticates the identical 44-account frame.
#[tokio::test]
async fn a_paying_redemption_refuses_the_zero_payout_custody_caller() {
    let (test, fixture) =
        terminal_fixture(TERMINAL_WIDTH, OUTCOME, FractionalExposureActionV2::TerminalRedeem);
    let mut context = test.start_with_context().await;
    create_custody_replay(&mut context, &fixture).await;

    let keys = terminal_observe(&fixture);
    let before = observe5(&mut context, keys).await;
    let (accepted, _logs, _units, result) =
        submit(&mut context, terminal_instruction(&fixture, CLAIMS_PROGRAM_ID, false)).await;
    assert!(!accepted);
    assert_eq!(
        custom_refusal(&result),
        Some(0x5002),
        "ClaimsSbfError::Identity: the paying path derives its own Custody caller"
    );

    // And nothing moved: a refused redemption pays nobody.
    let after = observe5(&mut context, keys).await;
    assert_eq!(before, after);
}

/// Reproduce the exact Custody caller-authority PDA a paying redemption needs.
///
/// Its seeds commit to the whole `CustodyRequestV1`, which commits to the
/// candidate digest, which commits to the payout and the signed-delta packet.
/// So the only way to name this account is to evaluate the terminal settlement
/// host-side exactly as Claims will, against the same authenticated bytes.
fn paying_custody_caller(fixture: &TerminalFixture, winner: u32) -> Pubkey {
    use dclutch_claims_svm::{
        CallerRole,
        liability_basis_state_v2::LiabilityBasisMarketViewV2,
        product_basis_terminal_v3::{
            ProductClaimsTerminalAdmissionV3, ProductClaimsTerminalInputV3,
            encode_product_claims_terminal_signed_delta_v3,
        },
        signed_delta_v3::{DeltaDirectionV3, SignedDeltaV3},
        terminal_settlement_v3::{
            TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3, TerminalSettlementRequestInputV3,
            TerminalSettlementRequestV3,
        },
    };
    use dclutch_custody_contract::{ContextV1, CustodyRequestV1, OperationV1};
    use dclutch_rational_representation_v2_kernel::product_v3::TerminalScenarioV3;
    use dclutch_representation_composition_v3_kernel::RecordAdmissionV3;
    use solana_program::hash::hashv;

    let shared = &fixture.shared;
    let market = LiabilityBasisMarketViewV2::decode(&shared.claims_market_bytes)
        .expect("aggregate decode");
    let position = &shared.reserve_position;
    let terminal_request = TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
        caller_role: CallerRole::Trading,
        release_set: fixture.release_set,
        market: shared.core_market.to_bytes(),
        realm: fixture.realm_record.digest,
        parent_context: hash(&fixture.wrapper[1..]).to_bytes(),
        product_record_digest: shared.product.digest,
        exposure_id: EXPOSURE_ID,
        exposure_digest: shared.exposure.digest,
        terminal_record_digest: CERTIFICATE_ACCOUNT.to_bytes(),
        owner: fixture.root.to_bytes(),
        position: position.account.to_bytes(),
        recipient_owner: fixture.actor.to_bytes(),
        recipient_token_account: RECIPIENT_TOKEN.to_bytes(),
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        custody_program: CUSTODY_PROGRAM_ID.to_bytes(),
        collateral_mint: COLLATERAL_MINT.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        semantic_basis_id: shared.semantic_basis_id,
        linked_basis_record_digest: shared.linked_basis.digest,
        generation: GENERATION,
        expected_market_revision: market.revision,
        expected_position_revision: 0,
        expected_custody_revision: CUSTODY_EXPECTED_REVISION,
        quantity: WRAP_NATIVE_CLAIMS,
        claim_index: OUTCOME,
        transfer_index: 0,
    })
    .expect("terminal settlement request");
    let terminal_bytes = terminal_request.to_bytes();
    let terminal_digest = hash(&terminal_bytes).to_bytes();

    let admission = ProductClaimsTerminalAdmissionV3::new(
        EXPOSURE_ID,
        shared.exposure.digest,
        shared.product_id,
        shared.result_domain.digest,
        dclutch_fractional_atomic_program_test::narrow_fixture::COORDINATE_DOMAIN_ID,
        dclutch_fractional_atomic_program_test::narrow_fixture::RESULT_UNIT_ID,
        shared.semantic_basis_id,
        shared.linked_basis.digest,
        shared.core_market.to_bytes(),
        fixture.release_set,
        dclutch_fractional_atomic_program_test::narrow_fixture::EVALUATOR_RELEASE_ID,
        shared.outcome_count,
        dclutch_fractional_atomic_program_test::narrow_fixture::PAYOUT_SCALE,
    )
    .expect("terminal admission");
    let width = shared.outcome_count as usize;
    let mut product_scratch = vec![0_u64; width];
    let mut translation_scratch = vec![0_u64; width];
    let mut claims_scratch = vec![0_u64; width];
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral");
    let mut aggregate_scratch = vec![neutral; width];
    let mut packet = vec![
        0_u8;
        dclutch_claims_svm::signed_delta_v3::plan_bytes(market.claim_count, 1, 1)
            .expect("packet width")
    ];
    let payout = encode_product_claims_terminal_signed_delta_v3(
        ProductClaimsTerminalInputV3 {
            product_basis_bytes: &shared.linked_basis.bytes,
            admission,
            composition_exposure_bytes: &shared.exposure.bytes,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: EXPOSURE_ID,
                finalized_id: EXPOSURE_ID,
                recomputed_digest: shared.exposure.digest,
                finalized_digest: shared.exposure.digest,
                record_authenticated: true,
            },
            product_record_digest: shared.product.digest,
            market_account: shared.claims_market.to_bytes(),
            market_bytes: &shared.claims_market_bytes,
            position_bytes: &position.bytes,
            owner: fixture.root.to_bytes(),
            request_id: terminal_digest,
            caller_role: CallerRole::Trading,
            terminal: TerminalScenarioV3::Categorical(winner),
            claim_index: OUTCOME,
            quantity: WRAP_NATIVE_CLAIMS,
            expected_generation: GENERATION,
            expected_market_revision: market.revision,
            expected_position_revision: 0,
            hoard_before: HOARD_ATOMS,
        },
        &mut product_scratch,
        &mut translation_scratch,
        &mut claims_scratch,
        &mut aggregate_scratch,
        &mut packet,
    )
    .expect("host-side terminal evaluation must agree with the chain");
    assert!(payout > 0, "the winning coordinate must pay");

    let candidate_digest = hashv(&[
        TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        &terminal_digest,
        &hash(&packet).to_bytes(),
        &payout.to_le_bytes(),
        &shared.exposure.digest,
        &CERTIFICATE_ACCOUNT.to_bytes(),
    ])
    .to_bytes();
    let custody_request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: fixture.release_set,
        market: shared.core_market.to_bytes(),
        realm: fixture.realm_record.digest,
        context: CUSTODY_CONTEXT,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: [0; 32],
            destination_owner: fixture.actor.to_bytes(),
            order: [0; 32],
            parent_request_digest: terminal_digest,
            order_nonce: 0,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: fixture.hoard.to_bytes(),
        destination: RECIPIENT_TOKEN.to_bytes(),
        source_vault_context: CUSTODY_CONTEXT,
        destination_vault_context: [0; 32],
        mint: COLLATERAL_MINT.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: CUSTODY_EXPECTED_REVISION,
        resulting_revision: CUSTODY_EXPECTED_REVISION + 1,
        amount: payout,
        rent_lamports: 0,
    };
    let custody_bytes = custody_request.to_bytes().expect("custody request bytes");
    Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            fixture.release_set,
            shared.core_market.to_bytes(),
            ExecutionRoleV1::Claims,
            CUSTODY_CONTEXT,
            hash(&custody_bytes).to_bytes(),
        )
        .expect("claims custody caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0
}

/// The terminal settlement has real headroom at width 64 and none at 128.
///
/// A terminal settlement translates every Product result coordinate onto every
/// Claims representation root, so its cost grows with the width in a way the
/// open-market actions' does not:
///
/// ```text
///   width   8   16   32   48   64    96      98      99
///   units 463k 519k 593k 731k 897k 1356k   1393k   exhausted
/// ```
///
/// The arithmetic maximum is 98, and this test deliberately does NOT pin it.
/// At 98 the margin is 6,672 units out of 1,400,000 -- under half a percent --
/// and that is inside build-to-build variation: 98 settles against one build of
/// the same committed source and exhausts the budget against another. A bound
/// that depends on which machine compiled Claims is not a bound.
///
/// So the campaign pins a width with margin that survives a rebuild, and pins
/// that the far side genuinely does not settle. Three separate layers admit 256
/// -- the `U8` selector's index space, the Fractional terms codec, and
/// `MAX_COMPOSITION_REPRESENTATION_WIDTH_V3` -- and none of them is wrong; the
/// terminal route is simply narrower than all of them, and a Market wider than
/// this can be founded, wrapped into, and never redeemed from.
#[tokio::test]
async fn the_terminal_settlement_has_headroom_at_the_supported_width_and_none_far_above_it() {
    async fn settles(width: usize) -> (bool, u64) {
        let (test, fixture) = terminal_fixture(
            width,
            LOSING_COORDINATE,
            FractionalExposureActionV2::TerminalZeroBurn,
        );
        let mut context = test.start_with_context().await;
        create_custody_replay(&mut context, &fixture).await;
        let (accepted, _logs, units, _result) = submit(
            &mut context,
            terminal_instruction(&fixture, CLAIMS_PROGRAM_ID, false),
        )
        .await;
        (accepted, units)
    }

    let supported = SUPPORTED_TERMINAL_WIDTH;
    let (fits, units) = settles(supported).await;
    assert!(fits, "the supported width must settle");
    // A fifth of the budget in reserve, so a modest compute regression upstream
    // does not silently make published capabilities unredeemable.
    assert!(
        units < 1_120_000,
        "the supported width must keep real headroom, used {units} of 1,400,000"
    );

    let (over, exhausted) = settles(128).await;
    assert!(!over, "width 128 must not settle");
    assert_eq!(exhausted, 1_400_000, "and it fails by exhausting the budget");
}
