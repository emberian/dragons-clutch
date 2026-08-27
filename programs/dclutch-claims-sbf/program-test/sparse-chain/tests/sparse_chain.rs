//! Real-ELF evidence for the composed Claims sparse-transfer chain.
//!
//! `crates/dclutch-claims-svm/src/composition_v3.rs` (commit `78bda05`) defines
//! one composition no live route can reach yet: admit the destination Position,
//! carry its exact 512-byte typed receipt into a SparseNativeTransfer, and carry
//! that transfer's exact 448-byte receipt into the Close of the drained source
//! Position. This campaign executes it against the real Claims, Registry, Core
//! and Rent ELFs with the Product Runtime V3 graph, the LiabilityBasisV2
//! aggregate and the lifecycle RentCredit installed directly -- no Hot
//! execution, no open-market bootstrap.
//!
//! With `DCLUTCH_CAMPAIGN_EVIDENCE_DIR` set it also emits the finalized
//! transactions the gauntlet's census folds into the execution ledger.

use dclutch_program_test_evidence::TransactionEvidence;
use std::{env, fs, path::PathBuf, vec::Vec};

use dclutch_claims_affine_batch_program_test::fixture::{
    FinalizedRecordFixtureV2, ProductLbv2FixtureInputV2, ProductLbv2FixtureV2,
    compile_product_lbv2_fixture_v2,
};
use dclutch_claims_sbf::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2,
    PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2, ProtocolPositionActionV2,
    ProtocolPositionAdmissionEvidenceV2, ProtocolPositionAdmissionSeedsV2,
    ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2,
};
use dclutch_claims_sbf::sparse_native_transfer_v1::SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1;
use dclutch_claims_sparse_chain_test_caller_sbf::{
    CLOSE_RENT_TAIL_BYTES, FLAG_FAIL_AFTER, FLAG_SUBSTITUTE_ADMISSION_OWNER, FLAG_WITH_CLOSE,
};
use dclutch_claims_svm::{
    CallerRole,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
    },
    sparse_native_transfer_v1::{SparseNativeTransferInputV1, SparseNativeTransferV1},
};
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
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::versioned::VersionedTransaction;

const CLAIMS: Pubkey = Pubkey::new_from_array([0xb1; 32]);
const REGISTRY: Pubkey = Pubkey::new_from_array([0xb3; 32]);
const CORE: Pubkey = Pubkey::new_from_array([0xb4; 32]);
const TRADING: Pubkey = Pubkey::new_from_array([0xb5; 32]);
const RENT_PROGRAM: Pubkey = Pubkey::new_from_array([0xb6; 32]);
const GENERATION: u64 = 23;
/// The one Product outcome this chain moves. Everything else is zero, so the
/// drained source Position is closable.
const OUTCOME: u32 = 41;
/// The whole source balance at `OUTCOME`.
const QUANTITY: u64 = 9_001;
/// One parent request identity shared by the three stages. The composition's
/// backward receipt dependencies all key on it.
const REQUEST_ID: [u8; 32] = [0x77; 32];

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    caller: Vec<u8>,
    rent: Vec<u8>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR"));
    let read = |name: &str| fs::read(directory.join(name)).expect("real ELF");
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        caller: read("dclutch_claims_sparse_chain_test_caller_sbf.so"),
        rent: read("dclutch_rent_sbf.so"),
    }
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(0..4)
        .expect("programdata state")
        .copy_from_slice(&3_u32.to_le_bytes());
    *bytes.get_mut(12).expect("programdata authority tag") = 0;
    bytes
        .get_mut(45..)
        .expect("programdata ELF")
        .copy_from_slice(elf);
    bytes
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>, lamports: u64) {
    test.add_account(
        key,
        Account {
            lamports: lamports
                .max(Rent::default().minimum_balance(data.len()))
                .max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_account(
        test,
        programdata(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
        1,
    );
}

fn identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program identity")
}

fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact id")
}

fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(release),
        release,
        DeploymentObservationV1::new(
            release.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            release.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            0,
            release.elf_digest(),
            release.upgrade_authority(),
        )
        .expect("observation"),
    )
}

fn activation(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE, 0x51, &artifacts.core);
    let claims = release(CLAIMS, 0x52, &artifacts.claims);
    let trading = release(TRADING, 0x53, &artifacts.caller);
    let rent = release(RENT_PROGRAM, 0x54, &artifacts.rent);
    let set = ExecutionReleaseSetV1::new(
        ExecutionRoleBindingV1::new(core.program(), artifact_id(core)),
        ExecutionRoleBindingV1::new(claims.program(), artifact_id(claims)),
        ExecutionRoleBindingV1::new(trading.program(), artifact_id(trading)),
        ExecutionRoleBindingV1::new(claims.program(), artifact_id(claims)),
        ExecutionRoleBindingV1::new(rent.program(), artifact_id(rent)),
    )
    .expect("release set");
    let id = hash(&set.to_bytes()).to_bytes();
    let content = ContentId::new(id).expect("release id");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("cache");
    for (role, artifact) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, claims),
        (ExecutionRoleV1::Custody, rent),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &set,
            role,
            &activation_input(artifact),
        )
        .expect("activate");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (id, bytes)
}

fn add_record(test: &mut ProgramTest, record: &FinalizedRecordFixtureV2) {
    add_account(test, record.raw, record.owner, record.bytes.clone(), 1);
    add_account(test, record.staging, system_program::ID, Vec::new(), 1);
}

/// Overwrite one exact u64 coordinate in an already-valid LBV2 vector.
fn put_claim(bytes: &mut [u8], header: usize, index: u32, value: u64) {
    let offset = header + index as usize * 8;
    bytes
        .get_mut(offset..offset + 8)
        .expect("claim coordinate")
        .copy_from_slice(&value.to_le_bytes());
}

fn read_claim(bytes: &[u8], header: usize, index: u32) -> u64 {
    let offset = header + index as usize * 8;
    u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .expect("claim coordinate")
            .try_into()
            .expect("exact u64"),
    )
}

struct Fixture {
    release: [u8; 32],
    cache: Pubkey,
    core_market: Pubkey,
    market: Pubkey,
    source: Pubkey,
    source_admission: Pubkey,
    destination: Pubkey,
    destination_admission: Pubkey,
    source_owner: Pubkey,
    destination_owner: Pubkey,
    rent_credit: Pubkey,
    source_lamports: u64,
    source_admission_lamports: u64,
    destination_lamports: u64,
    destination_admission_lamports: u64,
    position_principal: u64,
    admission_principal: u64,
    graph: ProductLbv2FixtureV2,
}

#[allow(clippy::too_many_lines)]
fn fixture() -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    // The real per-transaction compute maximum, treated as a gate.
    test.set_compute_max_units(1_400_000);
    for (name, id, elf) in [
        ("dclutch_claims_sbf", CLAIMS, artifacts.claims.as_slice()),
        (
            "dclutch_registry_sbf",
            REGISTRY,
            artifacts.registry.as_slice(),
        ),
        ("dclutch_core_sbf", CORE, artifacts.core.as_slice()),
        (
            "dclutch_claims_sparse_chain_test_caller_sbf",
            TRADING,
            artifacts.caller.as_slice(),
        ),
        ("dclutch_rent_sbf", RENT_PROGRAM, artifacts.rent.as_slice()),
    ] {
        add_program(&mut test, name, id, elf);
    }
    let (release, cache_bytes) = activation(&artifacts);
    let cache = Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release], &REGISTRY).0;
    add_account(&mut test, cache, REGISTRY, cache_bytes, 1);

    let source_owner = Pubkey::new_from_array([0xd1; 32]);
    let destination_owner = Pubkey::new_from_array([0xd2; 32]);
    let mut graph = compile_product_lbv2_fixture_v2(ProductLbv2FixtureInputV2 {
        registry_program: REGISTRY,
        core_program: CORE,
        claims_program: CLAIMS,
        release_set: release,
        realm_id: [0x61; 32],
        custody_context: [0x62; 32],
        generation: GENERATION,
        source_owner,
        destination_owner,
    })
    .expect("Product/LBV2 fixture");

    // The shared fixture seeds two nonzero coordinates. This chain needs a
    // source that becomes ZERO EVERYWHERE after one transfer, because Close
    // refuses any Position with a nonzero balance. Rewrite the aggregate
    // supplies and the source balances so exactly one coordinate carries
    // value, and keep the aggregate equal to the sum of the positions.
    let claim_count = graph.outcome_count;
    for index in 0..claim_count {
        put_claim(
            &mut graph.claims_market_bytes,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            index,
            0,
        );
        put_claim(
            &mut graph.positions[0].bytes,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            index,
            0,
        );
    }
    put_claim(
        &mut graph.claims_market_bytes,
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        OUTCOME,
        QUANTITY,
    );
    put_claim(
        &mut graph.positions[0].bytes,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        OUTCOME,
        QUANTITY,
    );

    for record in [
        &graph.product,
        &graph.result_domain,
        &graph.portfolio,
        &graph.linked_basis,
    ] {
        add_record(&mut test, record);
    }
    add_account(
        &mut test,
        graph.core_market,
        CORE,
        graph.core_state.clone(),
        1,
    );
    add_account(
        &mut test,
        graph.claims_market,
        CLAIMS,
        graph.claims_market_bytes.clone(),
        1,
    );
    // Both Position owners are Trading-owned identity records: this chain runs
    // under ProtocolPositionOwnerKindV2::TradingRecord throughout.
    add_account(&mut test, source_owner, TRADING, vec![1], 1);
    add_account(&mut test, destination_owner, TRADING, vec![1], 1);

    let position_bytes = graph.positions[0].bytes.len();
    let position_principal = Rent::default().minimum_balance(position_bytes);
    let admission_principal = Rent::default().minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);

    let source = graph.positions[0].account;
    let source_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(
            graph.claims_market.to_bytes(),
            source_owner.to_bytes(),
        )
        .expect("source admission seeds")
        .as_slices(),
        &CLAIMS,
    )
    .0;
    let destination = graph.positions[1].account;
    let destination_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(
            graph.claims_market.to_bytes(),
            destination_owner.to_bytes(),
        )
        .expect("destination admission seeds")
        .as_slices(),
        &CLAIMS,
    )
    .0;

    let refund = RefundAuthority::new([0x71; 32]).expect("refund authority");
    let (rent_credit, bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            graph.core_market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM,
    );
    let rent_credit_data = LifecycleRentCreditV2::new(
        refund,
        LifecycleAccountIdV2::new(graph.core_market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release).expect("release set"),
        GENERATION,
        bump,
    )
    .expect("lifecycle RentCredit")
    .to_bytes()
    .to_vec();
    add_account(&mut test, rent_credit, RENT_PROGRAM, rent_credit_data, 1);

    // The source Position was admitted in some earlier transaction this
    // campaign does not replay; its admission record is installed with exactly
    // the content that Admit would have written, because Close authenticates
    // it and the composition's close join reads its Product identities.
    let source_lamports = position_principal + 13;
    let source_admission_lamports = admission_principal + 11;
    let admission = ProtocolPositionAdmissionV2::new(
        ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
            presence: ProtocolPositionPresenceV2::Vacant,
            release_set: release,
            market: graph.core_market.to_bytes(),
            position_owner: source_owner.to_bytes(),
            parent_request_digest: REQUEST_ID,
            rent_credit: rent_credit.to_bytes(),
            rent_program: RENT_PROGRAM.to_bytes(),
            generation: GENERATION,
            expected_market_revision: 0,
            expected_position_revision: 0,
            observed_position_lamports: source_lamports,
            observed_admission_lamports: source_admission_lamports,
            position_rent_principal: position_principal,
            admission_rent_principal: admission_principal,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        },
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: graph.product.digest,
            semantic_basis_id: graph.semantic_basis_id,
            linked_basis_record_digest: graph.linked_basis.digest,
            request_digest: [0x78; 32],
            claims_program: CLAIMS.to_bytes(),
            trading_program: TRADING.to_bytes(),
            capability_descriptor: [0; 32],
            capability_outcome: 0,
            outcome_count: claim_count,
        },
    )
    .expect("installed source admission");
    add_account(
        &mut test,
        source,
        CLAIMS,
        graph.positions[0].bytes.clone(),
        source_lamports,
    );
    add_account(
        &mut test,
        source_admission,
        CLAIMS,
        admission.to_state_bytes().expect("admission bytes").to_vec(),
        source_admission_lamports,
    );

    // The destination Position and its admission record are VACANT and prepaid:
    // Admit allocates and assigns them inside the chain.
    let destination_lamports = position_principal + 17;
    let destination_admission_lamports = admission_principal + 19;
    add_account(
        &mut test,
        destination,
        system_program::ID,
        Vec::new(),
        destination_lamports,
    );
    add_account(
        &mut test,
        destination_admission,
        system_program::ID,
        Vec::new(),
        destination_admission_lamports,
    );

    (
        test,
        Fixture {
            release,
            cache,
            core_market: graph.core_market,
            market: graph.claims_market,
            source,
            source_admission,
            destination,
            destination_admission,
            source_owner,
            destination_owner,
            rent_credit,
            source_lamports,
            source_admission_lamports,
            destination_lamports,
            destination_admission_lamports,
            position_principal,
            admission_principal,
            graph,
        },
    )
}

fn admit_request(f: &Fixture) -> ProtocolPositionRequestV2 {
    ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: f.release,
        market: f.core_market.to_bytes(),
        position_owner: f.destination_owner.to_bytes(),
        parent_request_digest: REQUEST_ID,
        rent_credit: f.rent_credit.to_bytes(),
        rent_program: RENT_PROGRAM.to_bytes(),
        generation: GENERATION,
        expected_market_revision: 0,
        expected_position_revision: 0,
        observed_position_lamports: f.destination_lamports,
        observed_admission_lamports: f.destination_admission_lamports,
        position_rent_principal: f.position_principal,
        admission_rent_principal: f.admission_principal,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
}

fn transfer_request(f: &Fixture) -> SparseNativeTransferV1 {
    SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
        caller_role: CallerRole::Trading,
        release_set: f.release,
        market: f.core_market.to_bytes(),
        request_id: REQUEST_ID,
        product_record_digest: f.graph.product.digest,
        semantic_basis_id: f.graph.semantic_basis_id,
        linked_basis_record_digest: f.graph.linked_basis.digest,
        source_owner: f.source_owner.to_bytes(),
        destination_owner: f.destination_owner.to_bytes(),
        expected_market_revision: 0,
        expected_source_revision: 0,
        expected_destination_revision: 0,
        generation: GENERATION,
        outcome: OUTCOME,
        claim_count: f.graph.outcome_count,
        quantity: QUANTITY,
    })
    .expect("sparse transfer request")
}

/// The canonical request with its quantity overwritten in place.
///
/// The codec refuses a zero quantity at construction, so the hostile case has
/// to be built as BYTES: this is what an adversary can actually put on the wire,
/// and it is what the on-chain decoder has to refuse.
fn transfer_bytes_with_quantity(f: &Fixture, quantity: u64) -> Vec<u8> {
    let mut bytes = transfer_request(f).to_bytes().to_vec();
    let offset = dclutch_claims_svm::sparse_native_transfer_v1::SparseNativeTransferLayoutV1::QUANTITY;
    bytes
        .get_mut(offset..offset + 8)
        .expect("quantity field")
        .copy_from_slice(&quantity.to_le_bytes());
    bytes
}

fn close_request(f: &Fixture) -> ProtocolPositionRequestV2 {
    ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Close,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence: ProtocolPositionPresenceV2::Existing,
        release_set: f.release,
        market: f.core_market.to_bytes(),
        position_owner: f.source_owner.to_bytes(),
        parent_request_digest: REQUEST_ID,
        rent_credit: f.rent_credit.to_bytes(),
        rent_program: RENT_PROGRAM.to_bytes(),
        generation: GENERATION,
        // The transfer advanced both revisions exactly once. The composition
        // requires the Close to name the transfer receipt's POST revisions.
        expected_market_revision: 1,
        expected_position_revision: 1,
        observed_position_lamports: f.source_lamports,
        observed_admission_lamports: f.source_admission_lamports,
        position_rent_principal: f.position_principal,
        admission_rent_principal: f.admission_principal,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
}

fn authority(f: &Fixture, context: [u8; 32], digest: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(
        &dclutch_release_set_contract::CallerAuthoritySeedsV1::new(
            ContentId::new(f.release).expect("release set"),
            f.core_market.to_bytes(),
            ExecutionRoleV1::Trading,
            context,
            digest,
        )
        .expect("caller authority seeds")
        .as_slices(),
        &TRADING,
    )
    .0
}

fn admit_frame(f: &Fixture, request: ProtocolPositionRequestV2) -> Vec<AccountMeta> {
    let bytes = request.to_bytes().expect("admit request");
    let metas = vec![
        AccountMeta::new_readonly(
            authority(f, request.position_owner, hash(&bytes).to_bytes()),
            false,
        ),
        AccountMeta::new_readonly(f.market, false),
        AccountMeta::new(f.destination, false),
        AccountMeta::new(f.destination_admission, false),
        AccountMeta::new_readonly(f.graph.linked_basis.raw, false),
        AccountMeta::new_readonly(f.graph.linked_basis.staging, false),
        AccountMeta::new_readonly(f.graph.product.raw, false),
        AccountMeta::new_readonly(f.graph.product.staging, false),
        AccountMeta::new_readonly(f.graph.result_domain.raw, false),
        AccountMeta::new_readonly(f.graph.result_domain.staging, false),
        AccountMeta::new_readonly(f.graph.portfolio.raw, false),
        AccountMeta::new_readonly(f.graph.portfolio.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(f.core_market, false),
        AccountMeta::new_readonly(f.cache, false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(programdata(TRADING), false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(programdata(CLAIMS), false),
        AccountMeta::new_readonly(CORE, false),
        AccountMeta::new_readonly(programdata(CORE), false),
        AccountMeta::new_readonly(f.destination_owner, false),
        AccountMeta::new_readonly(f.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM, false),
    ];
    // The width is enumerated by hand above and checked against the contract's
    // own frame constant; neither is derived from the other.
    assert_eq!(metas.len(), PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V2);
    metas
}

fn transfer_frame(f: &Fixture, bytes: &[u8]) -> Vec<AccountMeta> {
    let metas = vec![
        AccountMeta::new_readonly(authority(f, REQUEST_ID, hash(bytes).to_bytes()), false),
        AccountMeta::new(f.market, false),
        AccountMeta::new_readonly(f.graph.linked_basis.raw, false),
        AccountMeta::new_readonly(f.graph.linked_basis.staging, false),
        AccountMeta::new_readonly(f.graph.product.raw, false),
        AccountMeta::new_readonly(f.graph.product.staging, false),
        AccountMeta::new_readonly(f.graph.result_domain.raw, false),
        AccountMeta::new_readonly(f.graph.result_domain.staging, false),
        AccountMeta::new_readonly(f.graph.portfolio.raw, false),
        AccountMeta::new_readonly(f.graph.portfolio.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(f.core_market, false),
        AccountMeta::new_readonly(f.cache, false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(programdata(TRADING), false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(programdata(CLAIMS), false),
        AccountMeta::new_readonly(CORE, false),
        AccountMeta::new_readonly(programdata(CORE), false),
        AccountMeta::new(f.source, false),
        AccountMeta::new(f.destination, false),
    ];
    assert_eq!(metas.len(), SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1);
    metas
}

fn close_frame(f: &Fixture, request: ProtocolPositionRequestV2) -> Vec<AccountMeta> {
    let bytes = request.to_bytes().expect("close request");
    let metas = vec![
        AccountMeta::new_readonly(
            authority(f, request.position_owner, hash(&bytes).to_bytes()),
            false,
        ),
        AccountMeta::new_readonly(f.market, false),
        AccountMeta::new(f.source, false),
        AccountMeta::new(f.source_admission, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(f.cache, false),
        AccountMeta::new_readonly(REGISTRY, false),
        AccountMeta::new_readonly(TRADING, false),
        AccountMeta::new_readonly(programdata(TRADING), false),
        AccountMeta::new_readonly(CLAIMS, false),
        AccountMeta::new_readonly(programdata(CLAIMS), false),
        AccountMeta::new_readonly(f.source_owner, false),
        AccountMeta::new(f.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM, false),
    ];
    assert_eq!(metas.len(), PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2);
    metas
}

fn chain(f: &Fixture, flags: u8, transfer: &[u8]) -> Instruction {
    let admit = admit_request(f);
    let close = close_request(f);
    let with_close = flags & FLAG_WITH_CLOSE != 0;

    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS, false)];
    accounts.extend(admit_frame(f, admit));
    accounts.extend(transfer_frame(f, transfer));
    if with_close {
        accounts.extend(close_frame(f, close));
    }

    let mut data = vec![flags];
    data.extend_from_slice(&admit.to_bytes().expect("admit request"));
    data.extend_from_slice(transfer);
    if with_close {
        // Only the SOURCE Position's four rent facts travel; the wrapper
        // patches everything else out of the two requests the composition
        // already binds. Three full requests inline put the chain at 1,261
        // bytes, past Solana's 1,232-byte packet maximum.
        let mut tail = Vec::with_capacity(CLOSE_RENT_TAIL_BYTES);
        for value in [
            close.observed_position_lamports,
            close.observed_admission_lamports,
            close.position_rent_principal,
            close.admission_rent_principal,
        ] {
            tail.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(tail.len(), CLOSE_RENT_TAIL_BYTES);
        data.extend_from_slice(&tail);
    }
    Instruction {
        program_id: TRADING,
        accounts,
        data,
    }
}

/// Solana's legacy packet maximum. ProgramTest submits no packet and therefore
/// cannot enforce it, so this campaign MEASURES every transaction against it:
/// Found31 was a frame ten bytes past this limit and it survived every fixture
/// test in the tree.
const PACKET_DATA_BYTES: usize = 1_232;

/// The exact wire extent of one signed transaction.
///
/// One shortvec byte for the signature count, 64 bytes per signature, then the
/// serialised message. This is what a validator would receive.
fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    let extent = 1 + signatures * 64 + message.len();
    assert!(
        extent <= PACKET_DATA_BYTES,
        "the transaction serialises to {extent} bytes, past Solana's {PACKET_DATA_BYTES}-byte packet maximum"
    );
    extent
}

async fn process_legacy(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &str,
) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("legacy blockhash");
    let transaction = solana_transaction::Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed ALT transaction")
        .to_string();
    let wire_bytes = wire_extent(transaction.signatures.len(), &transaction.message_data());
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("ALT lifecycle processing");
    let accepted = processed.result.is_ok();
    // The refusal is rendered from what the RUNTIME returned, never from what
    // the campaign expected.
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    assert!(accepted, "ALT lifecycle must commit");
}

fn lookup_addresses(payer: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer && !addresses.contains(&instruction.program_id) {
            addresses.push(instruction.program_id);
        }
        for meta in &instruction.accounts {
            if meta.pubkey != payer && !addresses.contains(&meta.pubkey) {
                addresses.push(meta.pubkey);
            }
        }
    }
    addresses
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
    process_legacy(context, create, "claims sparse: create lookup table").await;
    for (index, chunk) in addresses.chunks(20).enumerate() {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
            &format!("claims sparse: extend lookup table {index}"),
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

struct Outcome {
    accepted: bool,
    logs: Vec<String>,
    returned: Option<(Pubkey, Vec<u8>)>,
    compute_units: u64,
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<Outcome, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
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
        VersionedTransaction::try_new(message, &[&context.payer]).expect("transaction");
    let signature = transaction
        .signatures
        .first()
        .ok_or(BanksClientError::ClientError("unsigned transaction"))?
        .to_string();
    let wire_bytes = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let accepted = processed.result.is_ok();
    // The refusal is rendered from what the RUNTIME returned, never from what
    // the campaign expected.
    let failure = processed.result.clone().err().map(|error| format!("{error:?}"));
    let (logs, returned, compute_units) = processed
        .metadata
        .map(|metadata| {
            (
                metadata.log_messages,
                metadata
                    .return_data
                    .map(|value| (value.program_id, value.data)),
                metadata.compute_units_consumed,
            )
        })
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute_units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Ok(Outcome {
        accepted,
        logs,
        returned,
        compute_units,
    })
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context.banks_client.get_account(key).await.expect("query")
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn real_sbf_admit_transfer_close_chains_by_exact_receipt() {
    let (test, f) = fixture();
    let mut context = test.start_with_context().await;

    let before_market = observed(&mut context, f.market).await.expect("aggregate");
    let before_source = observed(&mut context, f.source).await.expect("source");
    let before_destination = observed(&mut context, f.destination)
        .await
        .expect("vacant destination");
    let before_rent_credit = observed(&mut context, f.rent_credit)
        .await
        .expect("RentCredit");
    assert_eq!(
        read_claim(
            &before_source.data,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            OUTCOME
        ),
        QUANTITY
    );

    let canonical = transfer_request(&f).to_bytes().to_vec();
    let zeroed = transfer_bytes_with_quantity(&f, 0);
    let full = chain(&f, FLAG_WITH_CLOSE, &canonical);
    let late = chain(&f, FLAG_WITH_CLOSE | FLAG_FAIL_AFTER, &canonical);
    let zero = chain(&f, FLAG_WITH_CLOSE, &zeroed);
    let substituted = chain(
        &f,
        FLAG_WITH_CLOSE | FLAG_SUBSTITUTE_ADMISSION_OWNER,
        &canonical,
    );

    let addresses = lookup_addresses(
        context.payer.pubkey(),
        &[full.clone(), late.clone(), zero.clone(), substituted.clone()],
    );
    let table = create_live_lookup_table(&mut context, &addresses).await;

    // ZERO. The exact positive-quantity requirement is in the canonical codec,
    // so a zero transfer never reaches the ledger: Claims refuses the request
    // bytes outright and the whole chain -- admission included -- rolls back.
    let outcome = submit(
        &mut context,
        zero,
        table,
        &addresses,
        "claims sparse: transfer of zero",
    )
    .await
    .expect("zero transfer");
    assert!(!outcome.accepted, "a zero sparse transfer must refuse");
    assert_eq!(
        observed(&mut context, f.destination).await,
        Some(before_destination.clone()),
        "the destination Position must be byte-identical after a refused chain"
    );
    assert_eq!(
        observed(&mut context, f.source).await,
        Some(before_source.clone())
    );
    assert_eq!(
        observed(&mut context, f.market).await,
        Some(before_market.clone())
    );

    // The admission receipt is an EXACT dependency, not a formality: a receipt
    // that decodes but names another Position owner must not join this
    // transfer. (Omitting the suffix entirely is a different case: the adapter
    // treats it as optional and the exact backward dependency is then enforced
    // by ClaimsCompositionV3 in the outer controller, which no live route
    // reaches yet. This campaign drives the check the ADAPTER owns.)
    let outcome = submit(
        &mut context,
        substituted,
        table,
        &addresses,
        "claims sparse: transfer under a substituted admission receipt",
    )
    .await
    .expect("substituted admission");
    assert!(
        !outcome.accepted,
        "an admission receipt for another owner must not join this transfer"
    );
    assert_eq!(
        observed(&mut context, f.source).await,
        Some(before_source.clone())
    );

    // LATE ROLLBACK. Every stage returns; the caller then refuses.
    let outcome = submit(
        &mut context,
        late,
        table,
        &addresses,
        "claims sparse: caller refuses after the complete chain",
    )
    .await
    .expect("late failure");
    assert!(!outcome.accepted, "the late failure must refuse");
    assert!(
        outcome
            .logs
            .iter()
            .filter(|line| line.as_str() == format!("Program {CLAIMS} success"))
            .count()
            >= 3,
        "all three Claims stages must have returned before the caller refused: {:?}",
        outcome.logs
    );
    for (key, before) in [
        (f.market, &before_market),
        (f.source, &before_source),
        (f.destination, &before_destination),
        (f.rent_credit, &before_rent_credit),
    ] {
        assert_eq!(
            observed(&mut context, key).await.as_ref(),
            Some(before),
            "a completed three-route chain must roll back byte-exactly"
        );
    }
    assert!(
        observed(&mut context, f.source_admission).await.is_some(),
        "the source admission record must survive a rolled-back Close"
    );

    // POSITIVE. The chain commits.
    let outcome = submit(
        &mut context,
        full,
        table,
        &addresses,
        "claims sparse: admit, transfer and close",
    )
    .await
    .expect("chain");
    assert!(outcome.accepted, "the composed chain must commit");
    assert!(outcome.compute_units <= 1_400_000);
    let (producer, receipt) = outcome.returned.expect("chain receipt");
    assert_eq!(producer, CLAIMS);
    // The last stage's receipt is the Close receipt; the transfer receipt it
    // consumed is 448 bytes and is checked as a witness against the chain log.
    assert!(!receipt.is_empty());

    let market = observed(&mut context, f.market).await.expect("aggregate");
    let destination = observed(&mut context, f.destination)
        .await
        .expect("admitted destination");
    assert_eq!(destination.owner, CLAIMS);
    assert_eq!(
        read_claim(
            &destination.data,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            OUTCOME
        ),
        QUANTITY,
        "the destination must hold exactly the transferred quantity"
    );
    assert_eq!(
        read_claim(
            &market.data,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            OUTCOME
        ),
        read_claim(
            &before_market.data,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            OUTCOME
        ),
        "a transfer moves liability between Positions and never mints it"
    );
    assert!(
        observed(&mut context, f.source).await.is_none()
            && observed(&mut context, f.source_admission).await.is_none(),
        "the drained source Position and its admission record are reclaimed"
    );
    let rent_credit = observed(&mut context, f.rent_credit)
        .await
        .expect("RentCredit");
    assert_eq!(
        rent_credit.lamports,
        before_rent_credit.lamports + f.source_lamports + f.source_admission_lamports,
        "the Close reclaims exactly the two observed rent principals"
    );

    println!(
        "composed Claims sparse chain CU: chain={} (admit 26 / sparse 22 / close 15 accounts)",
        outcome.compute_units
    );
}
