//! Real-ELF ProgramTest evidence for the canonical affine LBV2 Claims waist.

use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_affine_batch_program_test::{
    AffineBatchObservationV2, AffineMutationInputV2, ConstructedAffineBatchV2,
    FinalizedRecordObservationV2, ObservedAccountV2, ProductGraphObservationV2,
    construct_affine_batch_v2,
    fixture::{
        FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, ProductLbv2FixtureV2,
        compile_product_lbv2_fixture_v2,
    },
};
use dclutch_claims_svm::affine_batch_v2::{DeltaDirectionV2, SignedMagnitudeV2};
use dclutch_core_contract::ContentId;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{Hash, hash},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};
use solana_transaction::{Transaction, versioned::VersionedTransaction};

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const TEST_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa4; 32]);
const GENERATION: u64 = 37;
const REQUEST_ID: [u8; 32] = [0xa5; 32];
const SOURCE_OWNER: Pubkey = Pubkey::new_from_array([0xa6; 32]);
const DESTINATION_OWNER: Pubkey = Pubkey::new_from_array([0xa7; 32]);

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    caller: Vec<u8>,
}

struct Fixture {
    shared: ProductLbv2FixtureV2,
    activation_cache: Pubkey,
    caller_authority: Pubkey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClaimsSnapshot {
    market: Account,
    positions: [Account; 2],
}

struct ChainSnapshot {
    slot: u64,
    activation_cache: Account,
    core_market: Account,
    claims_market: Account,
    linked_basis: Account,
    linked_basis_staging: Account,
    product: Account,
    product_staging: Account,
    result_domain: Account,
    result_domain_staging: Account,
    portfolio: Account,
    portfolio_staging: Account,
    positions: [Account; 2],
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
        caller: read("dclutch_claims_affine_batch_test_caller_sbf.so"),
    }
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority option") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
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

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
}

fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
        release.program().to_bytes(),
        bpf_loader_upgradeable::ID.to_bytes(),
        true,
        release.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        false,
        release.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        release.deployment_slot(),
        release.elf_digest(),
        release.upgrade_authority(),
    )
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(release), release, observation)
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
    for (role, artifact) in [
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
            &activation_input(artifact),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

fn add_finalized(test: &mut ProgramTest, record: &FinalizedRecordFixtureV2) {
    add_account(test, record.raw, record.owner, record.bytes.clone());
    add_account(test, record.staging, system_program::ID, Vec::new());
}

fn observed<'a>(slot: u64, key: Pubkey, account: &'a Account) -> ObservedAccountV2<'a> {
    ObservedAccountV2 {
        slot,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: &account.data,
    }
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
            "dclutch_claims_affine_batch_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    let (release_set, cache_bytes) = activation_cache(&artifacts);
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        activation_cache,
        REGISTRY_PROGRAM_ID,
        cache_bytes.clone(),
    );
    let shared = compile_product_lbv2_fixture_v2(ProductLbv2FixtureInputV2 {
        registry_program: REGISTRY_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        release_set,
        realm_id: [0x61; 32],
        custody_context: [0x62; 32],
        generation: GENERATION,
        source_owner: SOURCE_OWNER,
        destination_owner: DESTINATION_OWNER,
    })
    .expect("shared Product/LBV2 fixture");
    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.substituted_product,
        &shared.substituted_portfolio,
        &shared.linked_basis,
        &shared.substituted_linked_basis,
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

    let slot = 1;
    let cache_account = fixture_account(REGISTRY_PROGRAM_ID, cache_bytes);
    let core_account = fixture_account(CORE_PROGRAM_ID, shared.core_state.clone());
    let market_account = fixture_account(CLAIMS_PROGRAM_ID, shared.claims_market_bytes.clone());
    let linked_account = fixture_account(REGISTRY_PROGRAM_ID, shared.linked_basis.bytes.clone());
    let vacant = fixture_account(system_program::ID, Vec::new());
    let product_account = fixture_account(REGISTRY_PROGRAM_ID, shared.product.bytes.clone());
    let domain_account = fixture_account(REGISTRY_PROGRAM_ID, shared.result_domain.bytes.clone());
    let portfolio_account = fixture_account(REGISTRY_PROGRAM_ID, shared.portfolio.bytes.clone());
    let source_account = fixture_account(CLAIMS_PROGRAM_ID, shared.positions[0].bytes.clone());
    let destination_account = fixture_account(CLAIMS_PROGRAM_ID, shared.positions[1].bytes.clone());
    let position_observations = [
        observed(slot, shared.positions[0].account, &source_account),
        observed(slot, shared.positions[1].account, &destination_account),
    ];
    let offline = construct_affine_batch_v2(
        AffineBatchObservationV2 {
            caller_role: dclutch_claims_svm::CallerRole::Trading,
            request_id: REQUEST_ID,
            registry_program: REGISTRY_PROGRAM_ID,
            activation_cache: observed(slot, activation_cache, &cache_account),
            core_market: observed(slot, shared.core_market, &core_account),
            claims_market: observed(slot, shared.claims_market, &market_account),
            linked_basis: FinalizedRecordObservationV2 {
                raw: observed(slot, shared.linked_basis.raw, &linked_account),
                staging: observed(slot, shared.linked_basis.staging, &vacant),
            },
            product: ProductGraphObservationV2 {
                product: FinalizedRecordObservationV2 {
                    raw: observed(slot, shared.product.raw, &product_account),
                    staging: observed(slot, shared.product.staging, &vacant),
                },
                result_domain: FinalizedRecordObservationV2 {
                    raw: observed(slot, shared.result_domain.raw, &domain_account),
                    staging: observed(slot, shared.result_domain.staging, &vacant),
                },
                portfolio: FinalizedRecordObservationV2 {
                    raw: observed(slot, shared.portfolio.raw, &portfolio_account),
                    staging: observed(slot, shared.portfolio.staging, &vacant),
                },
            },
            positions: &position_observations,
            rent: &Rent::default(),
        },
        &canonical_mutations(),
    )
    .expect("offline caller PDA derivation");
    add_account(
        &mut test,
        offline.caller_authority,
        system_program::ID,
        Vec::new(),
    );
    (
        test,
        Fixture {
            shared,
            activation_cache,
            caller_authority: offline.caller_authority,
        },
    )
}

fn fixture_account(owner: Pubkey, data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

fn delta(direction: DeltaDirectionV2, magnitude: u64) -> SignedMagnitudeV2 {
    SignedMagnitudeV2::new(direction, magnitude).expect("canonical delta")
}

fn canonical_mutations() -> [AffineMutationInputV2; 2] {
    [
        AffineMutationInputV2 {
            source_present: true,
            destination_present: true,
            outcome: 0,
            source_position_index: 0,
            destination_position_index: 1,
            aggregate_delta: delta(DeltaDirectionV2::Neutral, 0),
            source_delta: delta(DeltaDirectionV2::Debit, 7),
            destination_delta: delta(DeltaDirectionV2::Credit, 7),
        },
        AffineMutationInputV2 {
            source_present: true,
            destination_present: true,
            outcome: 257,
            source_position_index: 0,
            destination_position_index: 1,
            aggregate_delta: delta(DeltaDirectionV2::Neutral, 0),
            source_delta: delta(DeltaDirectionV2::Debit, u64::MAX),
            destination_delta: delta(DeltaDirectionV2::Credit, u64::MAX),
        },
    ]
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("existing account")
}

async fn chain_snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> ChainSnapshot {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock");
    ChainSnapshot {
        slot: clock.slot,
        activation_cache: account(context, fixture.activation_cache).await,
        core_market: account(context, fixture.shared.core_market).await,
        claims_market: account(context, fixture.shared.claims_market).await,
        linked_basis: account(context, fixture.shared.linked_basis.raw).await,
        linked_basis_staging: account(context, fixture.shared.linked_basis.staging).await,
        product: account(context, fixture.shared.product.raw).await,
        product_staging: account(context, fixture.shared.product.staging).await,
        result_domain: account(context, fixture.shared.result_domain.raw).await,
        result_domain_staging: account(context, fixture.shared.result_domain.staging).await,
        portfolio: account(context, fixture.shared.portfolio.raw).await,
        portfolio_staging: account(context, fixture.shared.portfolio.staging).await,
        positions: [
            account(context, fixture.shared.positions[0].account).await,
            account(context, fixture.shared.positions[1].account).await,
        ],
    }
}

fn build_from_chain(
    snapshot: &ChainSnapshot,
    fixture: &Fixture,
    mutations: &[AffineMutationInputV2],
) -> dclutch_claims_affine_batch_program_test::Result<ConstructedAffineBatchV2> {
    let positions = [
        observed(
            snapshot.slot,
            fixture.shared.positions[0].account,
            &snapshot.positions[0],
        ),
        observed(
            snapshot.slot,
            fixture.shared.positions[1].account,
            &snapshot.positions[1],
        ),
    ];
    construct_affine_batch_v2(
        AffineBatchObservationV2 {
            caller_role: dclutch_claims_svm::CallerRole::Trading,
            request_id: REQUEST_ID,
            registry_program: REGISTRY_PROGRAM_ID,
            activation_cache: observed(
                snapshot.slot,
                fixture.activation_cache,
                &snapshot.activation_cache,
            ),
            core_market: observed(
                snapshot.slot,
                fixture.shared.core_market,
                &snapshot.core_market,
            ),
            claims_market: observed(
                snapshot.slot,
                fixture.shared.claims_market,
                &snapshot.claims_market,
            ),
            linked_basis: FinalizedRecordObservationV2 {
                raw: observed(
                    snapshot.slot,
                    fixture.shared.linked_basis.raw,
                    &snapshot.linked_basis,
                ),
                staging: observed(
                    snapshot.slot,
                    fixture.shared.linked_basis.staging,
                    &snapshot.linked_basis_staging,
                ),
            },
            product: ProductGraphObservationV2 {
                product: FinalizedRecordObservationV2 {
                    raw: observed(snapshot.slot, fixture.shared.product.raw, &snapshot.product),
                    staging: observed(
                        snapshot.slot,
                        fixture.shared.product.staging,
                        &snapshot.product_staging,
                    ),
                },
                result_domain: FinalizedRecordObservationV2 {
                    raw: observed(
                        snapshot.slot,
                        fixture.shared.result_domain.raw,
                        &snapshot.result_domain,
                    ),
                    staging: observed(
                        snapshot.slot,
                        fixture.shared.result_domain.staging,
                        &snapshot.result_domain_staging,
                    ),
                },
                portfolio: FinalizedRecordObservationV2 {
                    raw: observed(
                        snapshot.slot,
                        fixture.shared.portfolio.raw,
                        &snapshot.portfolio,
                    ),
                    staging: observed(
                        snapshot.slot,
                        fixture.shared.portfolio.staging,
                        &snapshot.portfolio_staging,
                    ),
                },
            },
            positions: &positions,
            rent: &Rent::default(),
        },
        mutations,
    )
}

async fn claims_snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> ClaimsSnapshot {
    ClaimsSnapshot {
        market: account(context, fixture.shared.claims_market).await,
        positions: [
            account(context, fixture.shared.positions[0].account).await,
            account(context, fixture.shared.positions[1].account).await,
        ],
    }
}

fn wrapper_instruction(plan: &ConstructedAffineBatchV2, fail_after: bool) -> Instruction {
    let mut accounts = Vec::with_capacity(plan.instruction.accounts.len().saturating_add(1));
    accounts.push(AccountMeta::new_readonly(plan.claims_program, false));
    for account in &plan.instruction.accounts {
        accounts.push(if account.is_writable {
            AccountMeta::new(account.pubkey, false)
        } else {
            AccountMeta::new_readonly(account.pubkey, false)
        });
    }
    let mut data = Vec::with_capacity(plan.instruction.data.len().saturating_add(1));
    data.push(u8::from(fail_after));
    data.extend_from_slice(&plan.instruction.data);
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

fn substitute_account(instruction: &mut Instruction, child_index: usize, key: Pubkey) {
    let outer_index = child_index.checked_add(1).expect("outer account index");
    instruction
        .accounts
        .get_mut(outer_index)
        .expect("existing child account")
        .pubkey = key;
}

fn lookup_addresses(payer: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer && !addresses.contains(&instruction.program_id) {
            addresses.push(instruction.program_id);
        }
        for account in &instruction.accounts {
            if account.pubkey != payer && !addresses.contains(&account.pubkey) {
                addresses.push(account.pubkey);
            }
        }
    }
    addresses
}

async fn process_legacy(context: &mut ProgramTestContext, instruction: Instruction) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("ALT lifecycle processing");
    assert!(processed.result.is_ok(), "ALT lifecycle must commit");
}

async fn create_live_lookup_table(
    context: &mut ProgramTestContext,
    addresses: &[Pubkey],
) -> Pubkey {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("make lookup-table slot recent");
    let payer = context.payer.pubkey();
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    process_legacy(context, create).await;
    for chunk in addresses.chunks(20) {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
        )
        .await;
    }
    let extension_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extension_clock.slot + 1)
        .expect("activate lookup addresses");
    table
}

async fn submit_v0(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
) -> Result<(bool, Vec<String>), BanksClientError> {
    let blockhash: Hash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let transaction =
        VersionedTransaction::try_new(message, &[&context.payer]).expect("signed v0 transaction");
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let logs = processed
        .metadata
        .map(|metadata| metadata.log_messages)
        .unwrap_or_default();
    Ok((processed.result.is_ok(), logs))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let end = offset.checked_add(8).expect("state offset");
    u64::from_le_bytes(
        bytes
            .get(offset..end)
            .expect("state field")
            .try_into()
            .expect("u64 field"),
    )
}

fn position_balance(account: &Account, outcome: usize) -> u64 {
    read_u64(
        &account.data,
        128_usize
            .checked_add(outcome.checked_mul(8).expect("outcome bytes"))
            .expect("balance offset"),
    )
}

#[tokio::test]
async fn real_sbf_affine_batch_is_runtime_width_exact_and_atomic() {
    let (test, fixture) = fixture();
    let mut context = test.start_with_context().await;
    let chain = chain_snapshot(&mut context, &fixture).await;
    let canonical = build_from_chain(&chain, &fixture, &canonical_mutations())
        .expect("chain-derived canonical plan");
    assert_eq!(canonical.caller_authority, fixture.caller_authority);
    assert_eq!(canonical.instruction.data.len(), 448);

    let duplicate_rows = [canonical_mutations()[0], canonical_mutations()[0]];
    assert!(
        build_from_chain(&chain, &fixture, &duplicate_rows).is_err(),
        "chain builder must refuse duplicate Position/outcome coordinates"
    );
    let mut duplicate_position_chain = chain_snapshot(&mut context, &fixture).await;
    duplicate_position_chain.positions[1] = duplicate_position_chain.positions[0].clone();
    assert!(
        build_from_chain(&duplicate_position_chain, &fixture, &canonical_mutations()).is_err(),
        "chain builder must refuse duplicate Position observations"
    );

    let direct = wrapper_instruction(&canonical, false);
    let late = wrapper_instruction(&canonical, true);
    let mut alias = direct.clone();
    substitute_account(&mut alias, 21, fixture.shared.positions[0].account);
    let mut substituted_product = direct.clone();
    substitute_account(
        &mut substituted_product,
        4,
        fixture.shared.substituted_product.raw,
    );
    substitute_account(
        &mut substituted_product,
        5,
        fixture.shared.substituted_product.staging,
    );
    substitute_account(
        &mut substituted_product,
        8,
        fixture.shared.substituted_portfolio.raw,
    );
    substitute_account(
        &mut substituted_product,
        9,
        fixture.shared.substituted_portfolio.staging,
    );
    let mut substituted_basis = direct.clone();
    substitute_account(
        &mut substituted_basis,
        2,
        fixture.shared.substituted_linked_basis.raw,
    );
    substitute_account(
        &mut substituted_basis,
        3,
        fixture.shared.substituted_linked_basis.staging,
    );
    let instruction_set = [
        direct.clone(),
        late.clone(),
        alias.clone(),
        substituted_product.clone(),
        substituted_basis.clone(),
    ];
    let addresses = lookup_addresses(context.payer.pubkey(), &instruction_set);
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let before = claims_snapshot(&mut context, &fixture).await;

    for hostile in [alias, substituted_product, substituted_basis] {
        let (accepted, _) = submit_v0(&mut context, hostile, table, &addresses)
            .await
            .expect("hostile transaction");
        assert!(!accepted, "hostile affine substitution must refuse");
        assert_eq!(claims_snapshot(&mut context, &fixture).await, before);
    }

    let (accepted, logs) = submit_v0(&mut context, late, table, &addresses)
        .await
        .expect("late caller transaction");
    assert!(!accepted, "test caller must deliberately refuse late");
    assert!(
        logs.iter()
            .any(|log| log == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "real Claims must return before late caller refusal: {logs:#?}"
    );
    assert_eq!(claims_snapshot(&mut context, &fixture).await, before);

    let (accepted, _) = submit_v0(&mut context, direct.clone(), table, &addresses)
        .await
        .expect("canonical affine transaction");
    assert!(accepted, "canonical real-SBF affine batch must commit");
    let after = claims_snapshot(&mut context, &fixture).await;
    assert_eq!(read_u64(&after.market.data, 16), 1);
    assert_eq!(read_u64(&after.positions[0].data, 16), 1);
    assert_eq!(read_u64(&after.positions[1].data, 16), 1);
    assert_eq!(position_balance(&after.positions[0], 0), 0);
    assert_eq!(position_balance(&after.positions[1], 0), 7);
    assert_eq!(position_balance(&after.positions[0], 257), 0);
    assert_eq!(position_balance(&after.positions[1], 257), u64::MAX);
    assert_eq!(read_u64(&after.market.data, 256), 7);
    assert_eq!(read_u64(&after.market.data, 256 + 257 * 8), u64::MAX);

    let (accepted, _) = submit_v0(&mut context, direct, table, &addresses)
        .await
        .expect("stale affine transaction");
    assert!(!accepted, "stale aggregate/Position revisions must refuse");
    assert_eq!(claims_snapshot(&mut context, &fixture).await, after);
}
