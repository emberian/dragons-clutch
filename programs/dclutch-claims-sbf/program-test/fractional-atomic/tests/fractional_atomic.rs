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
use dclutch_claims_affine_batch_program_test::fixture::{
    FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, ProductLbv2FixtureV2,
    compile_product_lbv2_fixture_v2,
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
const OUTCOME: u32 = 0;
const MINT_DECIMALS: u8 = 0;
const TOKEN_ACCOUNT_BYTES: usize = 165;
/// The exact ceiling `fractional_exposure_terms_bytes_v2` admits.
const FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2: u32 = 256;

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

fn activation_cache(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x31, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x32, &artifacts.claims);
    let trading = release(TEST_CALLER_PROGRAM_ID, 0x33, &artifacts.caller);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(claims),
        binding(claims),
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
        (ExecutionRoleV1::Custody, claims),
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
fn finalized(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> FinalizedRecordFixtureV2 {
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &owner).0;
    FinalizedRecordFixtureV2 {
        owner,
        schema,
        digest,
        raw,
        staging,
        bytes,
    }
}

fn add_finalized(test: &mut ProgramTest, record: &FinalizedRecordFixtureV2) {
    add_account(test, record.raw, record.owner, record.bytes.clone());
    add_account(test, record.staging, system_program::ID, Vec::new());
}

fn compile_shared(
    release_set: [u8; 32],
    source_owner: Pubkey,
    destination_owner: Pubkey,
) -> ProductLbv2FixtureV2 {
    compile_product_lbv2_fixture_v2(ProductLbv2FixtureInputV2 {
        registry_program: REGISTRY_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        release_set,
        realm_id: REALM_ID,
        custody_context: CUSTODY_CONTEXT,
        generation: GENERATION,
        source_owner,
        destination_owner,
    })
    .expect("shared N=258 Product/LBV2 fixture")
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

fn token_account_bytes(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
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

struct Fixture {
    shared: ProductLbv2FixtureV2,
    activation_cache: Pubkey,
    caller_authority: Pubkey,
    root: Pubkey,
    terms_record: FinalizedRecordFixtureV2,
    behavior_record: FinalizedRecordFixtureV2,
    shard_mint: Pubkey,
    holder_token: Pubkey,
    actor: Pubkey,
    wrapper: Vec<u8>,
    request: FractionalExposureRequestV2,
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
    let (release_set, cache_bytes) = activation_cache(&artifacts);
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
            exposure_id: [0x7a; 32],
            product_basis: probe.linked_basis.digest,
            representation_basis: probe.semantic_basis_id,
            graph_id: [0x7c; 32],
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
    for position in &shared.positions {
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
        token_account_bytes(shard_mint, actor, 0),
    );
    add_account(&mut test, actor, system_program::ID, Vec::new());

    let request = FractionalExposureRequestV2::new(
        FractionalExposureActionV2::Wrap,
        FractionalExposureRequestInputV2 {
            release_set,
            market: core_market.to_bytes(),
            product_record: shared.product.digest,
            result_domain: shared.result_domain.digest,
            terms: terms_record.digest,
            token_behavior: behavior_record.digest,
            exposure: [0x7a; 32],
            owner: actor.to_bytes(),
            source_token_account: [0; 32],
            destination_token_account: holder_token.to_bytes(),
            terminal_digest: [0; 32],
            expected_revision: ROOT_REVISION,
            quantity: WRAP_NATIVE_CLAIMS,
            representation_coordinate: OUTCOME,
        },
    )
    .expect("canonical Fractional wrap request");
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
    add_account(&mut test, caller_authority, system_program::ID, Vec::new());

    let mut wrapper = Vec::with_capacity(FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES);
    wrapper.push(0);
    wrapper.extend_from_slice(&request_bytes);
    assert_eq!(wrapper.len(), FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES);

    (
        test,
        Fixture {
            shared,
            activation_cache: activation_cache_key,
            caller_authority,
            root,
            terms_record,
            behavior_record,
            shard_mint,
            holder_token,
            actor,
            wrapper,
            request,
        },
    )
}

/// The exact 31-account production frame, in contract coordinate order.
fn child_accounts(fixture: &Fixture) -> Vec<AccountMeta> {
    let shared = &fixture.shared;
    // Claims recomputes the two Position coordinates sorted by owner bytes.
    let (position_0, position_1) = if fixture.actor.to_bytes() < fixture.root.to_bytes() {
        (&shared.positions[0], &shared.positions[1])
    } else {
        (&shared.positions[1], &shared.positions[0])
    };
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

fn instruction(fixture: &Fixture, fail_after: bool) -> Instruction {
    let mut accounts = Vec::with_capacity(FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3 + 1);
    accounts.push(AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false));
    accounts.extend(child_accounts(fixture));
    let mut data = fixture.wrapper.clone();
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

/// What executes today: the real Claims ELF dispatches the Fractional route.
///
/// This is the first execution of `fractional_atomic_v3` by anything. The
/// transaction reaches Claims at CPI depth two, and Claims runs tens of
/// thousands of compute units deep before refusing -- which means the exact
/// 31-account production frame, both `invoke_signed` PDA authorities, the four
/// finalized record pairs, the activated release set, the Fractional root, and
/// the Token-2022 Mint controller all authenticated. The single refusal left is
/// `ClaimsSbfError::Economic`, and the campaign below names exactly why.
#[tokio::test]
async fn the_real_claims_elf_dispatches_the_fractional_route_and_authenticates_the_whole_frame() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;

    let (accepted, logs, units, result) = submit(&mut context, instruction(&fixture, false)).await;
    assert!(!accepted, "see the width-binding campaign below");
    assert!(
        logs.iter()
            .any(|line| line.contains(&CLAIMS_PROGRAM_ID.to_string())
                && line.contains("invoke [2]")),
        "the real Claims ELF must be entered as a child of the test caller"
    );
    assert!(
        units > 30_000,
        "Claims must run deep into the Fractional route, not bounce at the frame: {units} units"
    );
    assert_eq!(
        custom_refusal(&result),
        Some(0x5005),
        "every identity, authority, record, root and Token check must pass, \
         leaving only ClaimsSbfError::Economic"
    );

    // Nothing moved: a refused Fractional wrap is atomic.
    let mint = account(&mut context, fixture.shard_mint).await;
    let holder = account(&mut context, fixture.holder_token).await;
    assert_eq!(u64::from_le_bytes(mint.data[36..44].try_into().unwrap()), 0);
    assert_eq!(u64::from_le_bytes(holder.data[64..72].try_into().unwrap()), 0);
}

/// Why the wrap above cannot commit, named exactly.
///
/// `ClaimsSbfError::Economic` covers the whole lowering onchain, so the reason
/// is recovered here by calling the same kernel with the same authenticated
/// inputs. It is not a fixture accident. Two published widths disagree:
///
/// * `fractional_exposure_terms_bytes_v2` refuses any representation width
///   above 256, so a Fractional capability can name at most 256 shard Mints;
/// * the kernel requires `market.claim_count == terms.representation_width()`;
/// * the only shared Claims LBV2 fixture -- and the runtime width the Claims
///   evidence profile is built around -- is 258.
///
/// So no Fractional capability can bind a Market wider than 256 outcomes, and
/// the 258-outcome fixture cannot carry one at all. Closing this needs a
/// protocol decision (raise the terms ceiling, or bound Fractional Markets at
/// 256 and give the campaign its own narrower Product/LBV2 fixture); it is not
/// something this campaign may paper over.
#[test]
fn the_fractional_representation_width_cannot_bind_a_258_outcome_market() {
    use dclutch_claims_svm::liability_basis_state_v2::LiabilityBasisMarketViewV2;
    use dclutch_fractional_claim_kernel::{
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
        FractionalExposureTermsV2,
    };
    use dclutch_fractional_claims_kernel::{
        FractionalExposureSignedDeltaInputV2, fractional_exposure_signed_delta_shape_v2,
    };

    // The terms codec ceiling is exact.
    assert!(fractional_exposure_terms_bytes_v2(256).is_ok());
    assert!(fractional_exposure_terms_bytes_v2(257).is_err());

    let (_test, fixture) = fixture();
    assert_eq!(fixture.shared.outcome_count, 258);

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
    .expect("the terms themselves are canonical at the 256 ceiling");
    assert_eq!(
        terms.representation_width(),
        FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2
    );

    let market = LiabilityBasisMarketViewV2::decode(&fixture.shared.claims_market_bytes)
        .expect("claims market decode");
    assert_eq!(market.claim_count, fixture.shared.outcome_count);
    assert_ne!(
        market.claim_count,
        terms.representation_width(),
        "this is the whole gap: a 258-outcome Market and a 256-Mint capability"
    );

    let (actor_position, reserve_position) = if fixture.shared.positions[0].owner == fixture.actor {
        (&fixture.shared.positions[0], &fixture.shared.positions[1])
    } else {
        (&fixture.shared.positions[1], &fixture.shared.positions[0])
    };
    let shape = fractional_exposure_signed_delta_shape_v2(FractionalExposureSignedDeltaInputV2 {
        request: fixture.request,
        terms,
        semantic_product_id: market.product_instance_id,
        market_account: fixture.shared.claims_market.to_bytes(),
        market_bytes: &fixture.shared.claims_market_bytes,
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        reserve_owner: fixture.root.to_bytes(),
        reserve_position_bytes: &reserve_position.bytes,
        actor_position_bytes: &actor_position.bytes,
    });
    assert!(
        shape.is_err(),
        "the host kernel must refuse for the same reason the chain did"
    );
}
