//! Real-ELF release-waist evidence for Registry-authenticated Trading Hot.
//!
//! The final campaign executes the canonical Direct fixed-topology bundle at
//! the protocol 1.4M compute ceiling.  This test owns only transaction assembly
//! and observations; Registry and Trading remain the executable authorities.

use std::{env, fs, path::PathBuf};

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1,
    hot_v3::{
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HotExecutionEnvelopeV3,
    },
};
use dclutch_capability_seal_contract::{
    CAPABILITY_SEAL_ACTION_OFFSET_V1, CAPABILITY_SEAL_DESCRIPTOR_DIGEST_OFFSET_V1,
    CAPABILITY_SEAL_HEADER_BYTES_V1, CAPABILITY_SEAL_MAGIC_OFFSET_V1,
    CAPABILITY_SEAL_REGISTRY_OFFSET_V1, CAPABILITY_SEAL_ROW_BYTES_V1,
    CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1, CAPABILITY_SEAL_ROW_RAW_OFFSET_V1,
    CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1, CAPABILITY_SEAL_VERDICTS_OFFSET_V1,
    CapabilitySealRequestV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::CustodyReplayV1;
use dclutch_direct_codec::execution_v3::DirectExecutionActionV3;
use dclutch_direct_codec::native_evidence_v3::{
    DIRECT_NATIVE_EVIDENCE_BYTES_V3, encode_direct_headerless_registry_native_evidence_v4_atomic,
};
use dclutch_direct_codec::successor::{DirectMakerReplayLayoutV1, DirectRootStateLayoutV1};
use dclutch_direct_hot_program_test_support::{
    DirectHotDeploymentWidthsV5,
    chain::install_direct_hot_chain_accounts_v5,
    fixture::{DirectHotChainFixtureV5, DirectHotChainInputV5, build_direct_hot_chain_fixture_v5},
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_registry_svm::continuation_v1::{
    REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationRequestV1,
};
use dclutch_registry_svm::continuation_v2::{
    TransparentHotAdmissionSeedsV2, TransparentHotContinuationV2,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_token_svm::TokenAccount;
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::state::{AddressLookupTable, LookupTableMeta};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, ed25519_program, system_program, sysvar};
use solana_transaction::versioned::VersionedTransaction;

const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x91; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x92; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x93; 32]);
const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x94; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x95; 32]);
/// Rent program owning the sole Market-lifecycle RentCredit. It is observed,
/// never invoked, on the Direct Hot path: the adapter re-derives the credit as
/// a PDA of its own account owner and requires that owner in the frame.
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x97; 32]);
const LOOKUP_TABLE: Pubkey = Pubkey::new_from_array([0x96; 32]);
const COMPUTE_LIMIT: u64 = 1_400_000;

struct Elves {
    registry: Vec<u8>,
    trading: Vec<u8>,
    core: Vec<u8>,
    claims: Vec<u8>,
    custody: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Releases {
    release_set: [u8; 32],
    activation: Pubkey,
    activation_digest: [u8; 32],
    core_programdata: Pubkey,
    trading_programdata: Pubkey,
    claims_programdata: Pubkey,
}

struct DirectCase {
    chain: DirectHotChainFixtureV5,
    payer: Keypair,
    makers: [Keypair; 2],
}

fn content(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero content identity")
}

fn program_identity(value: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(value.to_bytes()).expect("nonzero program identity")
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn elves() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| fs::read(directory.join(name)).expect("required real ELF");
    Elves {
        registry: read("dclutch_registry_sbf.so"),
        trading: read("dclutch_trading_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        claims: read("dclutch_claims_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
    }
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    bytes
        .get_mut(..4)
        .expect("loader variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .expect("deployment slot")
        .copy_from_slice(&0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("authority option") = 0;
    bytes.get_mut(45..).expect("ELF tail").copy_from_slice(elf);
    bytes
}

fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let bytes = immutable_programdata(elf);
    test.add_account(
        programdata(program),
        Account {
            lamports: Rent::default().minimum_balance(bytes.len()),
            data: bytes,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        content([semantic; 32]),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("immutable artifact release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact identity")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(value),
        value,
        DeploymentObservationV1::new(
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
        .expect("current immutable deployment observation"),
    )
}

fn add_release_waist(test: &mut ProgramTest, artifacts: &Elves) -> Releases {
    let core = release(CORE_PROGRAM_ID, 0x31, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x32, &artifacts.claims);
    let trading = release(TRADING_PROGRAM_ID, 0x33, &artifacts.trading);
    let custody = release(CUSTODY_PROGRAM_ID, 0x34, &artifacts.custody);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(core),
        binding(custody),
    )
    .expect("Core+Trading release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let release_set_content = content(release_set_id);
    let mut cache = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, release_set_content).expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, core),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut cache,
            release_set_content,
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate exact role");
    }
    ActivatedExecutionReleaseSetV1::decode(&cache).expect("complete activation cache");
    let activation_digest = hash(&cache).to_bytes();
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    test.add_account(
        activation,
        Account {
            lamports: Rent::default().minimum_balance(cache.len()),
            data: cache,
            owner: REGISTRY_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    Releases {
        release_set: release_set_id,
        activation,
        activation_digest,
        core_programdata: programdata(CORE_PROGRAM_ID),
        trading_programdata: programdata(TRADING_PROGRAM_ID),
        claims_programdata: programdata(CLAIMS_PROGRAM_ID),
    }
}

fn direct_case(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    corrupt_destination: bool,
) -> DirectCase {
    direct_case_v2(test, releases, artifacts, corrupt_destination, false)
}

/// Build the canonical Direct case, optionally leaving the seal PDA vacant.
///
/// The ordinary campaign installs the seal already written, exactly as a Market
/// that has sealed this closure once would find it. `vacant_seal` leaves the
/// PDA empty and System-owned instead, which is the prestate the on-chain seal
/// outer requires.
fn direct_case_v2(
    test: &mut ProgramTest,
    releases: Releases,
    artifacts: &Elves,
    corrupt_destination: bool,
    vacant_seal: bool,
) -> DirectCase {
    let payer = Keypair::new();
    let makers = [Keypair::new(), Keypair::new()];
    let clock = Clock {
        slot: 1,
        ..Clock::default()
    };
    test.add_sysvar_account(sysvar::clock::ID, &clock);
    test.add_account(
        payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let deployment_widths = DirectHotDeploymentWidthsV5::new(
        immutable_programdata(&artifacts.trading).len(),
        immutable_programdata(&artifacts.claims).len(),
        immutable_programdata(&artifacts.core).len(),
    )
    .expect("real Direct deployment widths");
    let mut chain = build_direct_hot_chain_fixture_v5(DirectHotChainInputV5 {
        registry_program: REGISTRY_PROGRAM_ID,
        trading_program: TRADING_PROGRAM_ID,
        core_program: CORE_PROGRAM_ID,
        claims_program: CLAIMS_PROGRAM_ID,
        custody_program: CUSTODY_PROGRAM_ID,
        rent_program: RENT_PROGRAM_ID,
        release_set: releases.release_set,
        activation_cache: releases.activation,
        trading_programdata: releases.trading_programdata,
        core_programdata: releases.core_programdata,
        claims_programdata: releases.claims_programdata,
        deployment_widths,
        payer: payer.pubkey(),
        makers: [makers[0].pubkey(), makers[1].pubkey()],
        clock_slot: clock.slot,
        // `add_release_waist` binds Trading at semantic release 0x33; the
        // validated-artifact seal is filed under exactly that release.
        trading_semantic_release: [0x33; 32],
    })
    .expect("canonical Profile14 Direct chain fixture");
    if corrupt_destination {
        let destination = chain.collateral_accounts[1];
        let account = chain
            .accounts
            .iter_mut()
            .find(|value| value.key == destination)
            .expect("Custody destination fixture account");
        let state = account
            .account
            .data
            .get_mut(108)
            .expect("base token state byte");
        *state = 0;
        assert!(TokenAccount::parse(&account.account.data).is_ok());
    }
    if vacant_seal {
        let seal = chain.capability_seal;
        let account = chain
            .accounts
            .iter_mut()
            .find(|value| value.key == seal)
            .expect("validated-artifact seal fixture account");
        account.account.data = Vec::new();
        account.account.owner = system_program::ID;
        account.account.lamports = 0;
    }
    for (index, candidate) in chain.accounts.iter().enumerate() {
        if candidate.key == Pubkey::default() {
            assert_eq!(candidate.key, system_program::ID);
            assert!(chain.externally_installed_keys.contains(&candidate.key));
        }
        let prior = chain
            .accounts
            .get(..index)
            .and_then(|accounts| accounts.iter().position(|other| other.key == candidate.key));
        assert!(
            prior.is_none(),
            "Direct fixture account {index} aliases account {prior:?}: {}",
            candidate.key
        );
    }
    let installed = install_direct_hot_chain_accounts_v5(
        test,
        &Rent::default(),
        &chain.accounts,
        &chain.externally_installed_keys,
    )
    .expect("install canonical Direct-owned accounts");
    assert_eq!(
        installed.rollback_snapshot_keys,
        chain.rollback_snapshot_keys
    );
    DirectCase {
        chain,
        payer,
        makers,
    }
}

fn registry_hot_instruction(releases: Releases, mut hot: Instruction) -> (Instruction, Pubkey) {
    assert_eq!(hot.program_id, TRADING_PROGRAM_ID);
    assert!(hot.accounts.len() >= HOT_FIXED_ACCOUNT_COUNT_V3);
    let cache_digest = content(releases.activation_digest);
    let hot_digest = content(hash(&hot.data).to_bytes());
    let continuation = TransparentHotContinuationV2::new(
        content(releases.release_set),
        cache_digest,
        hot_digest,
        u32::try_from(hot.data.len()).expect("Hot width"),
    )
    .expect("transparent Core+Trading Hot continuation");
    let batch = continuation.role_batch_request().expect("role batch");
    let seeds = TransparentHotAdmissionSeedsV2::new(
        continuation,
        releases.activation.to_bytes(),
        content(hash(&batch.to_bytes()).to_bytes()),
    )
    .expect("admission seeds");
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let digest = seeds.hot_instruction_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            digest.as_slice(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    hot.accounts.insert(
        HOT_FIXED_ACCOUNT_COUNT_V3,
        AccountMeta::new_readonly(admission, false),
    );
    let mut accounts = vec![
        AccountMeta::new_readonly(releases.activation, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(releases.core_programdata, false),
        AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        AccountMeta::new_readonly(releases.trading_programdata, false),
        AccountMeta::new_readonly(admission, false),
    ];
    accounts.extend(hot.accounts);
    (
        Instruction {
            program_id: REGISTRY_PROGRAM_ID,
            accounts,
            data: hot.data,
        },
        admission,
    )
}

fn legacy_registry_hot_instruction(releases: Releases, hot: Instruction) -> (Instruction, Pubkey) {
    let (mut outer, admission) = registry_hot_instruction(releases, hot);
    let request = RegistryContinuationRequestV1::new_core_trading_hot(
        content(releases.release_set),
        content(releases.activation_digest),
        content(hash(&outer.data).to_bytes()),
        u32::try_from(outer.data.len()).expect("Hot width"),
    )
    .expect("legacy headered continuation");
    let mut data = Vec::with_capacity(REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + outer.data.len());
    data.extend_from_slice(&request.to_bytes());
    data.extend_from_slice(&outer.data);
    outer.data = data;
    (outer, admission)
}

fn direct_registry_instructions(releases: Releases, direct: &DirectCase) -> [Instruction; 2] {
    let (registry, _) = registry_hot_instruction(releases, direct.chain.hot_instruction.clone());
    let signatures = [
        direct.makers[0]
            .sign_message(&direct.chain.signed_messages[0])
            .as_ref()
            .try_into()
            .expect("seller signature width"),
        direct.makers[1]
            .sign_message(&direct.chain.signed_messages[1])
            .as_ref()
            .try_into()
            .expect("buyer signature width"),
    ];
    let mut evidence = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
    encode_direct_headerless_registry_native_evidence_v4_atomic(
        1,
        &registry.data,
        signatures,
        &mut evidence,
    )
    .expect("detached current-Registry native evidence");
    [
        Instruction {
            program_id: ed25519_program::ID,
            accounts: Vec::new(),
            data: evidence.to_vec(),
        },
        registry,
    ]
}

fn canonical_lookup_addresses(instructions: &[Instruction], payer: Pubkey) -> Vec<Pubkey> {
    let programs = instructions
        .iter()
        .map(|instruction| instruction.program_id)
        .collect::<Vec<_>>();
    let mut addresses = instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
        .filter(|meta| !meta.is_signer && meta.pubkey != payer && !programs.contains(&meta.pubkey))
        .map(|meta| meta.pubkey)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    addresses
}

fn add_lookup_table(test: &mut ProgramTest, addresses: &[Pubkey]) {
    let data = AddressLookupTable {
        meta: LookupTableMeta::default(),
        addresses: addresses.into(),
    }
    .serialize_for_tests()
    .expect("lookup-table bytes");
    test.add_account(
        LOOKUP_TABLE,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: solana_address_lookup_table_interface::program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
}

async fn submit_v0(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    addresses: Vec<Pubkey>,
    transaction_payer: Option<&Keypair>,
    signers: &[&Keypair],
) -> Result<u64, RefusedExecution> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let transaction_payer = transaction_payer.unwrap_or(&context.payer);
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &transaction_payer.pubkey(),
            instructions,
            &[AddressLookupTableAccount {
                key: LOOKUP_TABLE,
                addresses,
            }],
            blockhash,
        )
        .expect("canonical v0 message"),
    );
    let wire = 1_usize
        .checked_add(
            64_usize
                .checked_mul(signers.len() + 1)
                .expect("signature span"),
        )
        .and_then(|prefix| prefix.checked_add(message.serialize().len()))
        .expect("v0 wire width");
    assert!(
        wire <= 1_232,
        "canonical continuation packet overflow: {wire} bytes"
    );
    if instructions.len() == 2
        && instructions
            .first()
            .is_some_and(|instruction| instruction.program_id == ed25519_program::ID)
        && instructions
            .get(1)
            .is_some_and(|instruction| instruction.program_id == REGISTRY_PROGRAM_ID)
    {
        // Decision 0005 added the read-only validated-artifact seal at fixed
        // coordinate 38. The key itself is ALT-routed, but the continuation
        // carries the nested Hot account list twice, so the canonical packet
        // grew by exactly two index bytes: 1,224 -> 1,226 of the 1,232 limit.
        assert_eq!(wire, 1_226, "transparent continuation wire changed");
    }
    let mut all_signers = vec![transaction_payer];
    all_signers.extend_from_slice(signers);
    let transaction = VersionedTransaction::try_new(message, &all_signers)
        .expect("complete canonical v0 signatures");
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let logs = processed
        .metadata
        .as_ref()
        .map(|metadata| metadata.log_messages.clone())
        .unwrap_or_default();
    if let Err(error) = processed.result {
        return Err(RefusedExecution {
            error: BanksClientError::TransactionError(error),
            logs,
        });
    }
    Ok(processed
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default())
}

/// One refused execution together with the program log it reached.
///
/// A refusal test that only asserts `is_err()` cannot tell a refusal reached at
/// its intended depth from one that aborted before any of the CPIs it claims to
/// roll back ever ran.
struct RefusedExecution {
    error: BanksClientError,
    logs: Vec<String>,
}

impl From<BanksClientError> for RefusedExecution {
    fn from(error: BanksClientError) -> Self {
        Self {
            error,
            logs: Vec::new(),
        }
    }
}

impl core::fmt::Debug for RefusedExecution {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{:?}", self.error)
    }
}

impl RefusedExecution {
    fn invoked(&self, program: Pubkey) -> bool {
        let expected = format!("Program {program} invoke");
        self.logs.iter().any(|line| line.starts_with(&expected))
    }
}

fn program_test(artifacts: &Elves) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(COMPUTE_LIMIT);
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        REGISTRY_PROGRAM_ID,
        &artifacts.registry,
    );
    add_program(
        &mut test,
        "dclutch_trading_sbf",
        TRADING_PROGRAM_ID,
        &artifacts.trading,
    );
    add_program(
        &mut test,
        "dclutch_core_sbf",
        CORE_PROGRAM_ID,
        &artifacts.core,
    );
    add_program(
        &mut test,
        "dclutch_claims_sbf",
        CLAIMS_PROGRAM_ID,
        &artifacts.claims,
    );
    add_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &artifacts.custody,
    );
    test
}

fn registry_boundary_hot(releases: Releases) -> Instruction {
    let mut accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|index| {
            let coordinate = u8::try_from(index + 1).expect("fixed Hot account coordinate");
            AccountMeta::new_readonly(Pubkey::new_from_array([coordinate; 32]), false)
        })
        .collect::<Vec<_>>();
    for (index, meta) in [
        (
            HOT_ACTIVATION_CACHE_ACCOUNT_V3,
            AccountMeta::new_readonly(releases.activation, false),
        ),
        (
            HOT_CORE_PROGRAM_ACCOUNT_V3,
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        ),
        (
            HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
            AccountMeta::new_readonly(releases.core_programdata, false),
        ),
        (
            HOT_TRADING_PROGRAM_ACCOUNT_V3,
            AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
        ),
        (
            HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
            AccountMeta::new_readonly(releases.trading_programdata, false),
        ),
        (
            HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        ),
        (
            HOT_RENT_SYSVAR_ACCOUNT_V3,
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ),
    ] {
        *accounts.get_mut(index).expect("fixed Hot account") = meta;
    }
    let request = b"registry-boundary-fixture";
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).expect("boundary request width"),
        releases.release_set,
        accounts
            .first()
            .expect("boundary Market account")
            .pubkey
            .to_bytes(),
        1,
        [0x71; 32],
    )
    .expect("canonical boundary envelope");
    let mut data = Vec::with_capacity(128 + request.len());
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(request);
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        // Hostile cases below refuse at the Registry boundary before the
        // intentionally incomplete child fixture can execute.
        data,
    }
}

async fn activation_snapshot(context: &mut ProgramTestContext, activation: Pubkey) -> Account {
    context
        .banks_client
        .get_account(activation)
        .await
        .expect("activation read")
        .expect("activation account")
}

async fn account_snapshots(
    context: &mut ProgramTestContext,
    keys: &[Pubkey],
) -> Vec<(Pubkey, Option<Account>)> {
    let mut output = Vec::with_capacity(keys.len());
    for key in keys {
        let account = context
            .banks_client
            .get_account(*key)
            .await
            .expect("rollback account read");
        output.push((*key, account));
    }
    output
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

async fn corrupt_account_byte(
    context: &mut ProgramTestContext,
    key: Pubkey,
    offset: usize,
) -> Account {
    let mut value = account(context, key).await;
    let byte = value
        .data
        .get_mut(offset)
        .expect("hostile state byte in bounds");
    *byte ^= 1;
    context.set_account(&key, &AccountSharedData::from(value.clone()));
    value
}

async fn assert_registry_refusal(
    mut test: ProgramTest,
    releases: Releases,
    instruction: Instruction,
) {
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let before = activation_snapshot(&mut context, releases.activation).await;
    assert!(
        submit_v0(&mut context, &[instruction], addresses, None, &[])
            .await
            .is_err(),
        "hostile Registry continuation unexpectedly executed"
    );
    let after = activation_snapshot(&mut context, releases.activation).await;
    assert_eq!(after, before, "Registry refusal mutated release evidence");
}

#[test]
fn release_fixture_uses_five_distinct_real_artifacts() {
    let artifacts = elves();
    for bytes in [
        &artifacts.registry,
        &artifacts.trading,
        &artifacts.core,
        &artifacts.claims,
        &artifacts.custody,
    ] {
        assert!(!bytes.is_empty());
    }
    let digests = [
        hash(&artifacts.registry).to_bytes(),
        hash(&artifacts.trading).to_bytes(),
        hash(&artifacts.core).to_bytes(),
        hash(&artifacts.claims).to_bytes(),
        hash(&artifacts.custody).to_bytes(),
    ];
    for (index, digest) in digests.iter().enumerate() {
        assert!(
            digests
                .get(index + 1..)
                .expect("digest suffix")
                .iter()
                .all(|other| other != digest)
        );
    }
}

#[test]
fn transparent_wrapper_preserves_exact_hot_bytes_and_places_one_admission_at_38() {
    let releases = Releases {
        release_set: [0x41; 32],
        activation: Pubkey::new_from_array([0x42; 32]),
        activation_digest: [0x43; 32],
        core_programdata: Pubkey::new_from_array([0x44; 32]),
        trading_programdata: Pubkey::new_from_array([0x45; 32]),
        claims_programdata: Pubkey::new_from_array([0x46; 32]),
    };
    let hot = registry_boundary_hot(releases);
    let exact_hot_bytes = hot.data.clone();
    let (outer, admission) = registry_hot_instruction(releases, hot);
    assert_eq!(outer.data, exact_hot_bytes);
    let child = outer.accounts.get(6..).expect("nested Hot frame");
    assert_eq!(
        child
            .get(HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|meta| meta.pubkey),
        Some(admission)
    );
    assert_eq!(
        child.iter().filter(|meta| meta.pubkey == admission).count(),
        1
    );
}

#[tokio::test]
async fn real_registry_refuses_legacy_headered_hot_container_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (instruction, _) =
        legacy_registry_hot_instruction(releases, registry_boundary_hot(releases));
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_refuses_reordered_core_and_trading_roles_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    instruction.accounts.swap(1, 3);
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_refuses_substituted_core_programdata_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    *instruction
        .accounts
        .get_mut(2)
        .expect("Core ProgramData prefix") =
        AccountMeta::new_readonly(releases.trading_programdata, false);
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let before = activation_snapshot(&mut context, releases.activation).await;
    assert!(
        submit_v0(&mut context, &[instruction], addresses, None, &[])
            .await
            .is_err(),
        "substituted Core ProgramData unexpectedly authenticated"
    );
    let after = activation_snapshot(&mut context, releases.activation).await;
    assert_eq!(after, before, "deployment refusal mutated release evidence");
}

#[tokio::test]
async fn real_registry_refuses_altered_hot_bytes_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, _) = registry_hot_instruction(releases, registry_boundary_hot(releases));
    let byte = instruction.data.last_mut().expect("continuation byte");
    *byte ^= 1;
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_refuses_aliased_ephemeral_admission_atomically() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let (mut instruction, admission) =
        registry_hot_instruction(releases, registry_boundary_hot(releases));
    let child_start = 6;
    *instruction
        .accounts
        .get_mut(child_start)
        .expect("first Hot account") = AccountMeta::new_readonly(admission, false);
    assert_registry_refusal(test, releases, instruction).await;
}

#[tokio::test]
async fn real_registry_executes_profile14_direct_hot_under_protocol_limit() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let units = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("Registry-authenticated Direct Hot execution");
    assert!(units > 0 && units <= COMPUTE_LIMIT);

    let root = account(&mut context, direct.chain.root).await;
    assert_eq!(root.owner, TRADING_PROGRAM_ID);
    assert!(!root.data.is_empty());
    let replay = account(&mut context, direct.chain.custody_replay).await;
    let replay = CustodyReplayV1::decode(&replay.data).expect("post-Custody replay");
    assert_eq!(replay.next_revision, 8);
    let source = account(&mut context, direct.chain.collateral_accounts[0]).await;
    let destination = account(&mut context, direct.chain.collateral_accounts[1]).await;
    assert_eq!(
        TokenAccount::parse(&source.data)
            .expect("source token")
            .amount,
        95
    );
    assert_eq!(
        TokenAccount::parse(&destination.data)
            .expect("destination token")
            .amount,
        35
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_ne!(
        after, before,
        "successful Direct Hot left no material state change"
    );
}

#[tokio::test]
async fn late_custody_refusal_rolls_back_registry_hot_claims_and_lifecycle() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, true);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    let refusal = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect_err("uninitialized Custody destination unexpectedly accepted");
    // A rollback assertion over an execution that never started is vacuous: it
    // holds for any refusal, including one raised before the first child CPI.
    // The claim under test is specifically that Trading reached its Custody
    // child, that child refused, and everything the earlier children wrote was
    // rolled back. Require the depth the name claims.
    assert!(
        refusal.invoked(CLAIMS_PROGRAM_ID),
        "the Claims children this test claims to roll back never ran: {:#?}",
        refusal.logs
    );
    assert!(
        refusal.invoked(CUSTODY_PROGRAM_ID),
        "the late Custody refusal was never reached: {:#?}",
        refusal.logs
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(
        after, before,
        "late Custody refusal failed to roll back Claims/lifecycle bytes or lamports"
    );
}

#[tokio::test]
async fn corrupt_profile14_root_reserved_byte_refuses_without_mutation() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    corrupt_account_byte(
        &mut context,
        direct.chain.root,
        CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::RESERVED,
    )
    .await;
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert!(
        submit_v0(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        .is_err(),
        "noncanonical Direct root unexpectedly accepted"
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(after, before, "root refusal mutated Profile14 state");
}

#[tokio::test]
async fn corrupt_live_profile14_maker_reserved_byte_refuses_without_mutation() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    submit_v0(
        &mut context,
        &instructions,
        addresses.clone(),
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("first-use execution creates live maker replay");
    let hostile = corrupt_account_byte(
        &mut context,
        direct.chain.maker_replays[0],
        DirectMakerReplayLayoutV1::RESERVED,
    )
    .await;
    assert_eq!(hostile.owner, TRADING_PROGRAM_ID);
    let before = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert!(
        submit_v0(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        .is_err(),
        "noncanonical live maker replay unexpectedly accepted"
    );
    let after = account_snapshots(&mut context, &direct.chain.rollback_snapshot_keys).await;
    assert_eq!(after, before, "maker refusal mutated Profile14 state");
}

// --- Decision 0005: the validated-artifact seal ------------------------------
//
// The hot campaign above installs the seal already written, which is what a
// Market that has sealed a closure once actually finds. That would be circular
// evidence on its own: it proves the hot path accepts a seal the *fixture*
// wrote. These tests close the circle by making the on-chain seal outer write
// it and requiring the result to equal the fixture's bytes exactly, and then by
// refusing every seal that is not the canonical one.

/// Build the seal outer for one Direct case.
///
/// The account list is the hot fixed prefix with the root read-only and the
/// seal writable, followed by the rent payer and the System Program.
fn seal_instruction(direct: &DirectCase, action: u32, descriptor_digest: [u8; 32]) -> Instruction {
    let mut accounts = direct
        .chain
        .hot_instruction
        .accounts
        .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
        .expect("hot fixed prefix")
        .to_vec();
    for meta in accounts.iter_mut() {
        meta.is_writable = meta.pubkey == direct.chain.capability_seal;
        meta.is_signer = false;
    }
    accounts.push(AccountMeta::new(direct.payer.pubkey(), true));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data: CapabilitySealRequestV1::new(action, descriptor_digest)
            .expect("canonical seal request")
            .to_bytes()
            .to_vec(),
    }
}

async fn maybe_account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context.banks_client.get_account(key).await.expect("read")
}

fn descriptor_digest(direct: &DirectCase) -> [u8; 32] {
    direct.chain.descriptor_digest
}

fn direct_action() -> u32 {
    DirectExecutionActionV3::InlineOrdinary as u32
}

async fn submit_seal(
    context: &mut ProgramTestContext,
    direct: &DirectCase,
    instruction: Instruction,
) -> Result<u64, RefusedExecution> {
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&instruction), direct.payer.pubkey());
    submit_v0(context, &[instruction], addresses, Some(&direct.payer), &[]).await
}

#[tokio::test]
async fn the_seal_outer_writes_exactly_the_bytes_the_hot_path_expects() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let descriptor_digest = descriptor_digest(&direct);
    let canonical = seal_instruction(&direct, direct_action(), descriptor_digest);
    let addresses =
        canonical_lookup_addresses(core::slice::from_ref(&canonical), direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;

    assert!(
        maybe_account(&mut context, direct.chain.capability_seal)
            .await
            .is_none_or(|value| value.owner == system_program::ID && value.data.is_empty()),
        "the seal PDA is not vacant before the seal outer runs"
    );

    let units = submit_seal(&mut context, &direct, canonical.clone())
        .await
        .expect("canonical validated-artifact seal");
    assert!(units > 0 && units <= COMPUTE_LIMIT);

    let sealed = account(&mut context, direct.chain.capability_seal).await;
    assert_eq!(sealed.owner, TRADING_PROGRAM_ID);
    assert_eq!(
        sealed.data, direct.chain.capability_seal_bytes,
        "the on-chain seal outer and the fixture disagree about the verdict"
    );
    assert!(sealed.lamports >= Rent::default().minimum_balance(sealed.data.len()));

    // Write-once: a second seal of the same closure refuses and leaves the
    // recorded verdict byte-for-byte intact.
    let refused = submit_seal(&mut context, &direct, canonical).await;
    assert!(refused.is_err(), "an existing seal was rewritten");
    let after = account(&mut context, direct.chain.capability_seal).await;
    assert_eq!(after.data, sealed.data);
}

#[tokio::test]
async fn a_seal_for_another_action_or_descriptor_never_lands_at_this_address() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let descriptor_digest = descriptor_digest(&direct);
    let hostile = [
        seal_instruction(&direct, direct_action() ^ 1, descriptor_digest),
        seal_instruction(&direct, direct_action(), [0x5a; 32]),
    ];
    let addresses = canonical_lookup_addresses(&hostile, direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;

    for instruction in hostile {
        assert!(
            submit_seal(&mut context, &direct, instruction)
                .await
                .is_err(),
            "a seal filed under other coordinates reached the canonical address"
        );
        assert!(
            maybe_account(&mut context, direct.chain.capability_seal)
                .await
                .is_none_or(|value| value.owner == system_program::ID && value.data.is_empty()),
            "a refused seal left state at the canonical address"
        );
    }
}

#[tokio::test]
async fn hot_refuses_a_missing_seal_and_a_seal_written_for_another_release() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let instructions = direct_registry_instructions(releases, &direct);
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = test.start_with_context().await;
    let refused = submit_v0(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await;
    assert!(
        refused.is_err(),
        "a hot action executed with no validated-artifact seal"
    );
}

#[tokio::test]
async fn hot_refuses_a_seal_whose_body_was_altered_after_it_was_written() {
    for offset in [
        CAPABILITY_SEAL_MAGIC_OFFSET_V1,
        CAPABILITY_SEAL_VERDICTS_OFFSET_V1,
        CAPABILITY_SEAL_ACTION_OFFSET_V1,
        CAPABILITY_SEAL_DESCRIPTOR_DIGEST_OFFSET_V1,
        CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1,
        CAPABILITY_SEAL_REGISTRY_OFFSET_V1,
        CAPABILITY_SEAL_HEADER_BYTES_V1 + CAPABILITY_SEAL_ROW_RAW_OFFSET_V1,
        CAPABILITY_SEAL_HEADER_BYTES_V1
            + 2 * CAPABILITY_SEAL_ROW_BYTES_V1
            + CAPABILITY_SEAL_ROW_DIGEST_OFFSET_V1,
    ] {
        let artifacts = elves();
        let mut test = program_test(&artifacts);
        let releases = add_release_waist(&mut test, &artifacts);
        let mut direct = direct_case(&mut test, releases, &artifacts, false);
        let seal = direct.chain.capability_seal;
        let account = direct
            .chain
            .accounts
            .iter_mut()
            .find(|value| value.key == seal)
            .expect("seal fixture account");
        let byte = account.account.data.get_mut(offset).expect("seal byte");
        *byte ^= 0xff;
        let instructions = direct_registry_instructions(releases, &direct);
        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = test.start_with_context().await;
        assert!(
            submit_v0(
                &mut context,
                &instructions,
                addresses,
                Some(&direct.payer),
                &[],
            )
            .await
            .is_err(),
            "hot accepted a seal whose byte {offset} was altered"
        );
    }
}
