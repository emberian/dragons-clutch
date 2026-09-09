//! Real-ELF replay of the first native Core `ExecuteProviderV3` route split.
//!
//! The external fixture is a read-only capture of all 47 instruction accounts
//! immediately after Submit finalized on an owned validator. `exact-old`
//! replays its exact instruction and deployment graph. `reauthored-new` changes
//! only the Core deployment-bound release set, activation, Market binding,
//! request release coordinate, submitted lifecycle release set, and caller PDA
//! required by that new ELF. All Source, provider, Product, economic, and other
//! writable prestates remain captured.
//!
//! Current execution requires a fresh V2 capture with native code commitments for
//! every activated role. The historical bf06/V1 archive runs only with its pinned
//! pre-V2 harness revision; it is not accepted as current-cohort evidence.

use std::{env, fs, path::PathBuf, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_core_contract::ContentId;
use dclutch_core_sbf::CoreSbfError;
use dclutch_market::{CoreState, Identity as MarketIdentity, REQUEST_BYTES, Request};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ActivatedExecutionReleaseSetViewV1, ArtifactActivationInputV1,
    ArtifactReleaseV2, DeploymentObservationV2, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_source::resolution::{
    PROVIDER_EXECUTION_REQUEST_BYTES_V3, ProviderExecutionRequestV3, ProviderUpdateLifecycleV4,
    ProviderUpdateStatusV3, ResolutionCertificateV2,
};
use serde_json::{Value, json};
use solana_account::Account;
use solana_program::{clock::Clock, hash::hash, instruction::AccountMeta, pubkey::Pubkey};
use solana_program_test::ProgramTest;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::system_program;
use solana_transaction::{Instruction, InstructionError, Transaction, TransactionError};

const CORE: usize = 11;
const CORE_PROGRAMDATA: usize = 12;
const RESOLUTION: usize = 15;
const RESOLUTION_PROGRAMDATA: usize = 16;
const CALLER_AUTHORITY: usize = 0;
const SOURCE: usize = 2;
const CERTIFICATE: usize = 3;
const MARKET: usize = 4;
const ACTIVATION: usize = 5;
const LIFECYCLE: usize = 37;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    ExactOld,
    ReauthoredNew,
}

impl Mode {
    fn from_environment() -> Self {
        match env::var("DCLUTCH_CAPTURED_EXECUTE_MODE").as_deref() {
            Ok("exact-old") => Self::ExactOld,
            Ok("reauthored-new") => Self::ReauthoredNew,
            _ => panic!("DCLUTCH_CAPTURED_EXECUTE_MODE must be exact-old or reauthored-new"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::ExactOld => "exact-old",
            Self::ReauthoredNew => "reauthored-new",
        }
    }
}

struct Captured {
    document: Value,
    document_digest: [u8; 32],
    captured_action_digest: String,
    metas: Vec<AccountMeta>,
    accounts: Vec<Option<Account>>,
    instruction: Instruction,
    observation_slot: u64,
    observation_time: i64,
}

fn key(value: &Value, path: &str) -> Pubkey {
    Pubkey::from_str(value.as_str().unwrap_or_else(|| panic!("{path} string")))
        .unwrap_or_else(|error| panic!("{path} pubkey: {error}"))
}

fn number(value: &Value, path: &str) -> u64 {
    value.as_u64().unwrap_or_else(|| panic!("{path} u64"))
}

fn account(value: &Value, path: &str) -> Account {
    let encoded = value
        .pointer("/data/0")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{path}.data[0]"));
    Account {
        lamports: number(&value["lamports"], &format!("{path}.lamports")),
        data: BASE64
            .decode(encoded)
            .unwrap_or_else(|error| panic!("{path}.data base64: {error}")),
        owner: key(&value["owner"], &format!("{path}.owner")),
        executable: value["executable"]
            .as_bool()
            .unwrap_or_else(|| panic!("{path}.executable bool")),
        rent_epoch: number(&value["rentEpoch"], &format!("{path}.rentEpoch")),
    }
}

fn captured() -> Captured {
    let root = PathBuf::from(
        env::var("DCLUTCH_CAPTURED_EXECUTE_PRESTATE")
            .expect("DCLUTCH_CAPTURED_EXECUTE_PRESTATE is required"),
    );
    let bytes = fs::read(root.join("account-prestate.json")).expect("captured account prestate");
    let document_digest = hash(&bytes).to_bytes();
    let document: Value = serde_json::from_slice(&bytes).expect("captured account JSON");
    assert_eq!(
        document["schema"].as_str(),
        Some("dclutch-core-execute-provider-regression-prestate-v2"),
        "fresh ArtifactReleaseV2 capture required; replay historical V1 archives at their pinned pre-V2 revision"
    );
    let checkpoint: Value = serde_json::from_slice(
        &fs::read(root.join("checkpoint.json")).expect("captured checkpoint"),
    )
    .expect("captured checkpoint JSON");
    let stage = &checkpoint["stagePlan"];
    let rows = document["accounts"].as_array().expect("captured accounts");
    let action_rows = stage["action"]["accounts"]
        .as_array()
        .expect("captured action accounts");
    assert_eq!(rows.len(), 47);
    assert_eq!(rows.len(), action_rows.len());
    let metas = action_rows
        .iter()
        .enumerate()
        .map(|(index, value)| {
            assert_eq!(value, &rows[index]["meta"]);
            let address = key(&value["pubkey"], "action account");
            let signer = value["signer"].as_bool().expect("signer bool");
            if value["writable"].as_bool().expect("writable bool") {
                AccountMeta::new(address, signer)
            } else {
                AccountMeta::new_readonly(address, signer)
            }
        })
        .collect::<Vec<_>>();
    let accounts = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let result = row
                .get("account")
                .filter(|value| !value.is_null())
                .map(|value| account(value, &format!("accounts[{index}]")));
            if let (Some(account), Some(expected)) = (&result, row["dataSha256"].as_str()) {
                assert_eq!(
                    hex32(hash(&account.data).to_bytes()),
                    expected,
                    "accounts[{index}] data digest"
                );
            }
            result
        })
        .collect::<Vec<_>>();
    let instruction = Instruction {
        program_id: key(&stage["action"]["programId"], "action program"),
        accounts: metas.clone(),
        data: BASE64
            .decode(stage["action"]["dataBase64"].as_str().expect("action data"))
            .expect("action base64"),
    };
    let captured_action_digest = stage["action"]["sha256"]
        .as_str()
        .expect("captured action digest")
        .to_owned();
    let reconstructed_action_digest = hex32(
        hash(&bincode::serialize(&instruction).expect("serialize reconstructed instruction"))
            .to_bytes(),
    );
    assert_eq!(reconstructed_action_digest, captured_action_digest);
    assert_eq!(
        document["executeInstructionSha256"]
            .as_str()
            .expect("execute instruction digest"),
        captured_action_digest
    );
    Captured {
        document,
        document_digest,
        captured_action_digest,
        metas,
        accounts,
        instruction,
        observation_slot: number(&stage["observationSlot"], "observationSlot"),
        observation_time: stage["observationUnixTimestamp"]
            .as_i64()
            .expect("observationUnixTimestamp i64"),
    }
}

fn artifact_id(release: ArtifactReleaseV2) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact identity")
}

fn hex32(value: [u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn binding(release: ArtifactReleaseV2) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
}

fn activation_input(release: ArtifactReleaseV2) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(release),
        release,
        DeploymentObservationV2::new(
            release.program().to_bytes(),
            release.loader_program().to_bytes(),
            true,
            release.programdata(),
            release.loader_program().to_bytes(),
            false,
            release.programdata(),
            release.loader_program().to_bytes(),
            release.deployment_slot(),
            release.code_commitment(),
            release.upgrade_authority(),
        )
        .expect("deployment observation"),
    )
}

fn reauthor_for_new_core(captured: &mut Captured, core_elf: &[u8]) {
    let old_cache = captured.accounts[ACTIVATION]
        .as_ref()
        .expect("captured activation cache");
    let old = ActivatedExecutionReleaseSetViewV1::decode(&old_cache.data)
        .expect("captured activated release set");
    let old_release_set_id = old
        .execution_release_set_id()
        .expect("captured release-set identity")
        .to_bytes();
    let old_core = old
        .role(ExecutionRoleV1::Core)
        .expect("Core activation")
        .release();
    assert_ne!(
        old_core.code_commitment(),
        dclutch_registry::artifact_code_commitment_v2::code_commitment_v2(core_elf)
            .expect("Core code commitment"),
    );
    let new_slot = old_core
        .deployment_slot()
        .checked_add(1)
        .expect("bounded deployment slot");
    let new_core = ArtifactReleaseV2::new(
        old_core.program(),
        old_core.loader_program(),
        old_core.programdata(),
        old_core.semantic_release_id(),
        dclutch_registry::artifact_code_commitment_v2::code_commitment_v2(core_elf)
            .expect("Core code commitment"),
        new_slot,
        old_core.upgrade_policy(),
        old_core.upgrade_authority(),
    )
    .expect("reauthored Core release");
    let claims = old
        .role(ExecutionRoleV1::Claims)
        .expect("Claims activation")
        .release();
    let trading = old
        .role(ExecutionRoleV1::Trading)
        .expect("Trading activation")
        .release();
    let resolution = old
        .role(ExecutionRoleV1::Resolution)
        .expect("Resolution activation")
        .release();
    let custody = old
        .role(ExecutionRoleV1::Custody)
        .expect("Custody activation")
        .release();
    let release_set = ExecutionReleaseSetV1::new(
        binding(new_core),
        binding(claims),
        binding(trading),
        binding(resolution),
        binding(custody),
    )
    .expect("reauthored release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set identity");
    let mut cache = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, content).expect("activation cache");
    for (role, release) in [
        (ExecutionRoleV1::Core, new_core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, resolution),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut cache,
            content,
            &release_set,
            role,
            &activation_input(release),
        )
        .expect("activate reauthored role");
    }
    ActivatedExecutionReleaseSetV1::decode(&cache).expect("complete reauthored cache");
    let registry = captured.metas[7].pubkey;
    let activation =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set_id], &registry).0;
    captured.metas[ACTIVATION].pubkey = activation;
    let activation_account = captured.accounts[ACTIVATION]
        .as_mut()
        .expect("activation account");
    activation_account.data = cache;

    let market_account = captured.accounts[MARKET].as_mut().expect("Core Market");
    let mut market = CoreState::decode(&market_account.data).expect("Core Market state");
    market.identity.selected_release_set =
        MarketIdentity::new(release_set_id).expect("Market release-set identity");
    market_account.data = market.encode().expect("reauthored Core Market").to_vec();

    let pdata = captured.accounts[CORE_PROGRAMDATA]
        .as_mut()
        .expect("Core ProgramData");
    assert_eq!(u32::from_le_bytes(pdata.data[0..4].try_into().unwrap()), 3);
    pdata.data.truncate(45);
    pdata.data[4..12].copy_from_slice(&new_slot.to_le_bytes());
    pdata.data.extend_from_slice(core_elf);

    let outer = Request::decode(&captured.instruction.data[..REQUEST_BYTES]).expect("Core request");
    let start = REQUEST_BYTES;
    let end = start + PROVIDER_EXECUTION_REQUEST_BYTES_V3;
    let mut provider = ProviderExecutionRequestV3::decode(&captured.instruction.data[start..end])
        .expect("provider request");
    provider.release_set = release_set_id;
    let provider_bytes = provider.to_bytes().expect("reauthored provider request");
    captured.instruction.data[start..end].copy_from_slice(&provider_bytes);
    assert_eq!(
        outer,
        Request::decode(&captured.instruction.data[..REQUEST_BYTES]).unwrap()
    );

    let lifecycle_account = captured.accounts[LIFECYCLE]
        .as_mut()
        .expect("submitted provider lifecycle");
    let mut lifecycle = ProviderUpdateLifecycleV4::decode(&lifecycle_account.data)
        .expect("submitted provider lifecycle");
    assert_eq!(lifecycle.status, ProviderUpdateStatusV3::Submitted);
    assert_eq!(lifecycle.release_set, old_release_set_id);
    lifecycle.release_set = release_set_id;
    lifecycle_account.data = lifecycle
        .to_bytes()
        .expect("reauthored provider lifecycle")
        .to_vec();

    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set_id,
        provider.market,
        ExecutionRoleV1::Core,
        provider.source_state,
        provider.parent_request_digest,
    )
    .expect("Core caller seeds");
    captured.metas[CALLER_AUTHORITY].pubkey =
        Pubkey::find_program_address(&seeds.as_slices(), &captured.metas[CORE].pubkey).0;
    captured.instruction.accounts = captured.metas.clone();
}

fn fixture_keypair() -> Keypair {
    let path = env::var("DCLUTCH_CAPTURED_EXECUTE_RESOLVER_KEYPAIR")
        .expect("DCLUTCH_CAPTURED_EXECUTE_RESOLVER_KEYPAIR is required");
    let bytes: Vec<u8> = serde_json::from_slice(&fs::read(path).expect("resolver keypair file"))
        .expect("resolver keypair JSON");
    assert_eq!(bytes.len(), 64, "resolver keypair must carry 64 bytes");
    let secret: [u8; 32] = bytes[..32].try_into().expect("resolver secret");
    let pair = Keypair::new_from_array(secret);
    assert_eq!(pair.to_bytes().as_slice(), bytes.as_slice());
    pair
}

fn add_fixture_accounts(test: &mut ProgramTest, captured: &Captured) {
    for (index, value) in captured.accounts.iter().enumerate() {
        if captured.metas[index].pubkey == system_program::ID {
            continue;
        }
        if let Some(value) = value {
            if matches!(
                index,
                CORE | CORE_PROGRAMDATA | RESOLUTION | RESOLUTION_PROGRAMDATA
            ) {
                // Bank startup loads the real ELF from these captured ProgramData
                // accounts. Adding a second generated loader pair triggers Agave
                // 4.3's duplicate program-cache replacement panic.
                test.add_genesis_account(captured.metas[index].pubkey, value.clone());
            } else {
                test.add_account(captured.metas[index].pubkey, value.clone());
            }
        }
    }
}

async fn observed(
    context: &mut solana_program_test::ProgramTestContext,
    key: Pubkey,
) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("Banks account")
}

/// Requires an explicit external capture and artifact directory; run once for
/// `exact-old` and once for `reauthored-new` rather than silently skipping a
/// missing real-ELF prerequisite.
#[tokio::test]
#[ignore = "requires DCLUTCH_CAPTURED_EXECUTE_* and SBF_OUT_DIR"]
async fn captured_direct_execute_provider_replays_old_refusal_and_corrected_acceptance() {
    let mode = Mode::from_environment();
    let artifacts = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let core_elf = fs::read(artifacts.join("dclutch_core_sbf.so")).expect("Core ELF");
    let resolution_elf =
        fs::read(artifacts.join("dclutch_resolution_proof_sbf.so")).expect("Resolution ELF");
    let mut captured = captured();
    let exact_instruction_digest = hash(&captured.instruction.data).to_bytes();
    let old_provider = ProviderExecutionRequestV3::decode(
        &captured.instruction.data
            [REQUEST_BYTES..REQUEST_BYTES + PROVIDER_EXECUTION_REQUEST_BYTES_V3],
    )
    .expect("captured provider request");
    if mode == Mode::ReauthoredNew {
        reauthor_for_new_core(&mut captured, &core_elf);
    }
    let resolver = fixture_keypair();
    assert_eq!(resolver.pubkey(), captured.metas[1].pubkey);

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_fixture_accounts(&mut test, &captured);
    assert_eq!(
        hash(&core_elf).to_bytes(),
        hash(&captured.accounts[CORE_PROGRAMDATA].as_ref().unwrap().data[45..]).to_bytes()
    );
    assert_eq!(
        hash(&resolution_elf).to_bytes(),
        hash(
            &captured.accounts[RESOLUTION_PROGRAMDATA]
                .as_ref()
                .unwrap()
                .data[45..]
        )
        .to_bytes()
    );
    let mut context = test.start_with_context().await;
    context
        .warp_to_slot(captured.observation_slot)
        .expect("execute after captured deployment slots");
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Banks Clock");
    assert_eq!(clock.slot, captured.observation_slot);
    clock.unix_timestamp = captured.observation_time;
    context.set_sysvar(&clock);
    let writable = [SOURCE, CERTIFICATE, LIFECYCLE];
    let before = [
        observed(&mut context, captured.metas[SOURCE].pubkey).await,
        observed(&mut context, captured.metas[CERTIFICATE].pubkey).await,
        observed(&mut context, captured.metas[LIFECYCLE].pubkey).await,
    ];
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[captured.instruction.clone()],
        Some(&context.payer.pubkey()),
        &[&context.payer, &resolver],
        blockhash,
    );
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    let metadata = processed.metadata.expect("transaction metadata");
    let units = metadata.compute_units_consumed;
    let after = [
        observed(&mut context, captured.metas[SOURCE].pubkey).await,
        observed(&mut context, captured.metas[CERTIFICATE].pubkey).await,
        observed(&mut context, captured.metas[LIFECYCLE].pubkey).await,
    ];
    match mode {
        Mode::ExactOld => {
            assert_eq!(
                processed.result,
                Err(TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(CoreSbfError::ChildAck as u32),
                ))
            );
            assert_eq!(after, before, "all writable captured accounts roll back");
            assert_eq!(
                hash(&captured.instruction.data).to_bytes(),
                exact_instruction_digest,
                "old replay keeps the captured instruction exact"
            );
        }
        Mode::ReauthoredNew => {
            assert_eq!(processed.result, Ok(()));
            let source = dclutch_source::SourceResolutionStateV2::decode(
                &after[0].as_ref().expect("resolved Source").data,
            )
            .expect("Source state");
            assert_eq!(
                source.phase(),
                dclutch_source::SourceResolutionPhaseV1::Resolved
            );
            let certificate = ResolutionCertificateV2::decode(
                &after[1].as_ref().expect("terminal certificate").data,
            )
            .expect("terminal certificate");
            assert_ne!(
                certificate.route, old_provider.provider_release,
                "Source-selected provider release remains distinct from the Pyth deployment release"
            );
            let lifecycle = ProviderUpdateLifecycleV4::decode(
                &after[2].as_ref().expect("provider lifecycle").data,
            )
            .expect("provider lifecycle");
            assert_eq!(lifecycle.status, ProviderUpdateStatusV3::Consumed);
        }
    }
    let report = json!({
        "schema": "dclutch-captured-execute-provider-v3-real-elf-regression-v1",
        "mode": mode.label(),
        "computeUnitsConsumed": units,
        "result": format!("{:?}", processed.result),
        "coreElfSha256": hex32(hash(&core_elf).to_bytes()),
        "resolutionElfSha256": hex32(hash(&resolution_elf).to_bytes()),
        "originalCapturedActionSha256": captured.captured_action_digest,
        "originalInstructionDataSha256": hex32(exact_instruction_digest),
        "executedInstructionDataSha256": hex32(hash(&captured.instruction.data).to_bytes()),
        "capturedObservationSlot": captured.observation_slot,
        "captureDocumentSha256": hex32(captured.document_digest),
        "capturedGenesisHash": captured.document["genesisHash"],
        "writableAccounts": writable.map(|index| captured.metas[index].pubkey.to_string()),
        "logs": metadata.log_messages,
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if let Ok(path) = env::var("DCLUTCH_CAPTURED_EXECUTE_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).expect("write report");
    }
}
