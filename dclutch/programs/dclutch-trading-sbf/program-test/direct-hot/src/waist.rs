//! The real-ELF release waist the Registry-authenticated Hot campaign runs on.
//!
//! Extracted from `tests/registry_hot_continuation.rs`, which was its only
//! owner: every hostile Direct case a sibling test wants to add needs `elves`,
//! `add_release_waist`, `direct_case`, `direct_registry_instructions` and
//! `submit_v0`, and copying six hundred lines of waist construction into a
//! second file would be a second authority for the same fact -- it would drift
//! the first time either side moved. One owner, here, beside the chain fixture
//! these already build on.
//!
//! This is test support: it asserts and panics freely, because a fixture that
//! cannot be built has no honest value to return. The crate's `panic`,
//! `unwrap_used` and `indexing_slicing` denials are lifted for this module
//! alone, and for that reason.
#![allow(clippy::panic, clippy::unwrap_used, clippy::indexing_slicing)]

use std::{env, fs, path::PathBuf};

use crate::{
    DirectHotDeploymentWidthsV5,
    chain::install_direct_hot_chain_accounts_v5,
    fixture::{DirectHotChainFixtureV5, DirectHotChainInputV5, build_direct_hot_chain_fixture_v5},
};
use dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3;
use dclutch_core_contract::ContentId;
use dclutch_direct_codec::native_evidence_v3::{
    DIRECT_NATIVE_EVIDENCE_BYTES_V3, encode_direct_headerless_registry_native_evidence_v4_atomic,
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
use solana_account::Account;
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

pub const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x91; 32]);
pub const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x92; 32]);
pub const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x93; 32]);
pub const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x94; 32]);
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x95; 32]);
/// Rent program owning the sole Market-lifecycle RentCredit. It is observed,
/// never invoked, on the Direct Hot path: the adapter re-derives the credit as
/// a PDA of its own account owner and requires that owner in the frame.
pub const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x97; 32]);
pub const LOOKUP_TABLE: Pubkey = Pubkey::new_from_array([0x96; 32]);
pub const COMPUTE_LIMIT: u64 = 1_400_000;

pub struct Elves {
    pub registry: Vec<u8>,
    pub trading: Vec<u8>,
    pub core: Vec<u8>,
    pub claims: Vec<u8>,
    pub custody: Vec<u8>,
}

#[derive(Clone, Copy)]
pub struct Releases {
    pub release_set: [u8; 32],
    pub activation: Pubkey,
    pub activation_digest: [u8; 32],
    pub core_programdata: Pubkey,
    pub trading_programdata: Pubkey,
    pub claims_programdata: Pubkey,
}

pub struct DirectCase {
    pub chain: DirectHotChainFixtureV5,
    pub payer: Keypair,
    pub makers: [Keypair; 2],
}

pub fn content(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero content identity")
}

pub fn program_identity(value: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(value.to_bytes()).expect("nonzero program identity")
}

pub fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

pub fn elves() -> Elves {
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

pub fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
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

pub fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey, elf: &[u8]) {
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

pub fn release(program: Pubkey, semantic: u8, elf: &[u8]) -> ArtifactReleaseV1 {
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

pub fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact identity")
}

pub fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

pub fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
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

pub fn add_release_waist(test: &mut ProgramTest, artifacts: &Elves) -> Releases {
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

pub fn direct_case(
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
pub fn direct_case_v2(
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

pub fn registry_hot_instruction(releases: Releases, mut hot: Instruction) -> (Instruction, Pubkey) {
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

pub fn legacy_registry_hot_instruction(
    releases: Releases,
    hot: Instruction,
) -> (Instruction, Pubkey) {
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

pub fn direct_registry_instructions(releases: Releases, direct: &DirectCase) -> [Instruction; 2] {
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

pub fn canonical_lookup_addresses(instructions: &[Instruction], payer: Pubkey) -> Vec<Pubkey> {
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

pub fn add_lookup_table(test: &mut ProgramTest, addresses: &[Pubkey]) {
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

pub async fn submit_v0(
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
        //
        // `10d5a8b` then appended the Custody callee at logical coordinate 90,
        // taking the Direct profile from ninety fixed accounts to ninety-one.
        // That is one more physical account in the same twice-carried list, so
        // it is the same two index bytes again: 1,226 -> 1,228.
        //
        // !! FOUR BYTES OF MARGIN REMAIN !! Two more accounts appended to this
        // profile overflow the canonical packet, and the failure is a hard
        // refusal at `wire <= 1_232` above, not a partial result. This assertion
        // is the tripwire that made the growth visible at all -- both increments
        // reached it as a stale-pin failure before any execution, which is the
        // behaviour to keep. The next coordinate added here needs a plan for the
        // packet, not just a new number on this line.
        assert_eq!(wire, 1_228, "transparent continuation wire changed");
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
pub struct RefusedExecution {
    pub error: BanksClientError,
    pub logs: Vec<String>,
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
    pub fn invoked(&self, program: Pubkey) -> bool {
        let expected = format!("Program {program} invoke");
        self.logs.iter().any(|line| line.starts_with(&expected))
    }
}

pub fn program_test(artifacts: &Elves) -> ProgramTest {
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
