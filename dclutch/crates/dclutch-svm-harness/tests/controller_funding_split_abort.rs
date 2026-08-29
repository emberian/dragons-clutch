//! Real-SVM adversarial closure of the split controller-funding rollback domain.
//!
//! This test deliberately starts from the exact durable `Prepared` account
//! graph rather than using a host-only close model.  The production Trading
//! program chooses one of two canonical child-close orders from the manifest
//! masks and persists the first close in the checkpoint. A later transaction
//! may only resume the authenticated suffix. This is the real runtime shape:
//! Resolution alone consumes most of the compute ceiling, so pretending both
//! closes are atomic would strand every expired founding on devnet.

use std::{env, fs, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityEntryV1, CapabilityFundingLedgerDerivationV2, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId,
    ControllerFundingCheckpointDerivationV1, ControllerFundingCheckpointInputV1,
    ControllerFundingCheckpointV1, FundingAmountsV1, FundingLedgerV2, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY, funding_ledger_bytes_v2,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_market_core_codec::{Identity, generic_founding_funding_list_id_v1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_resolution_codec::{
    PreMarketFundingAbortRequestV1, RESOLUTION_CONTROLLER_RELEASE_ID_V7,
    pre_market_funding_ledger_account_digest_v1,
};
use solana_account::{Account, AccountSharedData};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::Signer;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x41; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x42; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x43; 32]);
const GENERATION: u64 = 7;
const EXPIRY_SLOT: u64 = 3;
const CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1: [u8; 8] = *b"DCLTCF1A";
const CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1: [u8; 8] = *b"DCLTCF2A";
const CONTROLLER_FUNDING_ABORT_ACCOUNT_COUNT_V1: usize = 17;
const TRADING_SEMANTIC_RELEASE_ID: [u8; 32] = [0x71; 32];

struct Elves {
    trading: Vec<u8>,
    resolution: Vec<u8>,
    registry: Vec<u8>,
}

#[derive(Clone)]
struct Snapshot {
    checkpoint: Option<Account>,
    resolution_ledger: Option<Account>,
    trading_ledger: Option<Account>,
    funding_source: Account,
    rent_credit: Account,
}

struct Fixture {
    test: Option<ProgramTest>,
    trading_program: Pubkey,
    checkpoint: Pubkey,
    checkpoint_data: Vec<u8>,
    resolution_ledger: Pubkey,
    resolution_lamports: u64,
    resolution_principal: u64,
    trading_ledger: Pubkey,
    trading_lamports: u64,
    trading_principal: u64,
    funding_source: Pubkey,
    rent_credit: Pubkey,
    instruction: Instruction,
    trading_closes_first: bool,
}

#[derive(Clone, Copy)]
enum SlotPinHostile {
    LaterTradingDeployment,
    LaterResolutionDeployment,
    TradingUpgradeAuthority,
    ResolutionUpgradeAuthority,
    ActivationCache,
}

fn artifacts() -> Elves {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    Elves {
        trading: fs::read(directory.join("dclutch_trading_sbf.so")).expect("Trading ELF"),
        resolution: fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
            .expect("Resolution ELF"),
        registry: fs::read(directory.join("dclutch_registry_sbf.so")).expect("Registry ELF"),
    }
}

fn content(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("nonzero content identity")
}

fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(program.to_bytes()).expect("program identity")
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn programdata_body(
    elf: &[u8],
    deployment_slot: u64,
    upgrade_authority: Option<[u8; 32]>,
) -> Vec<u8> {
    let mut bytes = vec![0_u8; 45 + elf.len()];
    bytes[0..4].copy_from_slice(&3_u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&deployment_slot.to_le_bytes());
    if let Some(authority) = upgrade_authority {
        bytes[12] = 1;
        bytes[13..45].copy_from_slice(&authority);
    }
    bytes[45..].copy_from_slice(elf);
    bytes
}

fn add_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
    deployment_slot: u64,
    upgrade_authority: Option<[u8; 32]>,
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    let data = programdata_body(elf, deployment_slot, upgrade_authority);
    test.add_account(
        programdata(program),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn release(program: Pubkey, semantic: [u8; 32], elf: &[u8]) -> ArtifactReleaseV1 {
    release_with_pin(
        program,
        semantic,
        elf,
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
}

fn release_with_pin(
    program: Pubkey,
    semantic: [u8; 32],
    elf: &[u8],
    deployment_slot: u64,
    upgrade_policy: ArtifactUpgradePolicyV1,
    upgrade_authority: Option<[u8; 32]>,
) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        program_identity(program),
        program_identity(bpf_loader_upgradeable::ID),
        programdata(program).to_bytes(),
        content(semantic),
        hash(elf).to_bytes(),
        deployment_slot,
        upgrade_policy,
        upgrade_authority,
    )
    .expect("pinned release")
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
        .expect("deployment observation"),
    )
}

fn manifest(trading_index: usize) -> Vec<u8> {
    let none = CompartmentFundingV1::not_applicable();
    let entries = (0_u8..4)
        .map(|index| {
            let principal = 101_u64 + u64::from(index);
            let amounts = FundingAmountsV1::new(
                none,
                none,
                none,
                none,
                CompartmentFundingV1::native_lamports(principal).expect("native principal"),
                none,
                none,
            )
            .expect("funding amounts");
            CapabilityEntryV1::new(
                CapabilityContentId::new([0x10 + index; 32]).expect("kind"),
                CapabilityContentId::new(if usize::from(index) == trading_index {
                    TRADING_SEMANTIC_RELEASE_ID
                } else {
                    RESOLUTION_CONTROLLER_RELEASE_ID_V7
                })
                .expect("controller release"),
                CapabilityContentId::new([0x20 + index; 32]).expect("config"),
                CapabilityContentId::new([0x30 + index; 32]).expect("capacity"),
                CapabilityContentId::new([0x40; 32]).expect("schema"),
                CapabilityContentId::new([0x50; 32]).expect("derivation"),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                FundingQuoteV1::new(amounts, None).expect("funding quote"),
            )
            .expect("capability entry")
        })
        .collect::<Vec<_>>();
    let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("manifest");
    bytes
}

fn ledger(
    controller: Pubkey,
    market: Pubkey,
    manifest_bytes: &[u8],
    selected_mask: u16,
) -> (Pubkey, Vec<u8>, u64, u64) {
    let manifest = CapabilityManifestV1::decode(manifest_bytes).expect("manifest");
    let manifest_id =
        CapabilityContentId::new(hash(manifest_bytes).to_bytes()).expect("manifest identity");
    let rows = u16::try_from(selected_mask.count_ones()).expect("row count");
    let mut bytes = vec![0_u8; funding_ledger_bytes_v2(rows).expect("ledger width")];
    FundingLedgerV2::initialize(&mut bytes, manifest_id, manifest, selected_mask)
        .expect("Pending ledger");
    let decoded = FundingLedgerV2::decode(&bytes).expect("ledger");
    let principal = decoded
        .authenticate(manifest_id, manifest)
        .and_then(|value| value.remaining_native_lamports_total())
        .expect("native principal");
    let address = Pubkey::find_program_address(
        &CapabilityFundingLedgerDerivationV2::new(
            controller.to_bytes(),
            market.to_bytes(),
            GENERATION,
            manifest_id,
            decoded,
        )
        .expect("ledger derivation")
        .seed_components(),
        &controller,
    )
    .0;
    let lamports = Rent::default()
        .minimum_balance(bytes.len())
        .checked_add(principal)
        .expect("ledger lamports");
    (address, bytes, lamports, principal)
}

fn fixture(trading_index: usize, staged: bool) -> Fixture {
    fixture_with_slot_pin_hostile(trading_index, staged, None)
}

fn fixture_with_slot_pin_hostile(
    trading_index: usize,
    staged: bool,
    slot_pin_hostile: Option<SlotPinHostile>,
) -> Fixture {
    let elves = artifacts();
    // ProgramTest's loaded-program cache is process-global. Each deliberately
    // different ProgramData body therefore gets a distinct program identity;
    // otherwise one hostile test would be testing cache replacement in the
    // harness rather than release authentication in the transaction.
    let nonce = match slot_pin_hostile {
        None => 0,
        Some(SlotPinHostile::LaterTradingDeployment) => 1,
        Some(SlotPinHostile::LaterResolutionDeployment) => 2,
        Some(SlotPinHostile::TradingUpgradeAuthority) => 3,
        Some(SlotPinHostile::ResolutionUpgradeAuthority) => 4,
        Some(SlotPinHostile::ActivationCache) => 5,
    };
    let trading_program = if nonce == 0 {
        TRADING_PROGRAM_ID
    } else {
        Pubkey::new_from_array([0x80 + nonce; 32])
    };
    let resolution_program = if nonce == 0 {
        RESOLUTION_PROGRAM_ID
    } else {
        Pubkey::new_from_array([0x90 + nonce; 32])
    };
    let registry_program = if nonce == 0 {
        REGISTRY_PROGRAM_ID
    } else {
        Pubkey::new_from_array([0xA0 + nonce; 32])
    };
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_program(
        &mut test,
        "dclutch_trading_sbf",
        trading_program,
        &elves.trading,
        0,
        None,
    );
    add_program(
        &mut test,
        "dclutch_resolution_proof_sbf",
        resolution_program,
        &elves.resolution,
        0,
        None,
    );
    add_program(
        &mut test,
        "dclutch_registry_sbf",
        registry_program,
        &elves.registry,
        0,
        None,
    );

    let trading_release = match slot_pin_hostile {
        Some(SlotPinHostile::LaterTradingDeployment) => release_with_pin(
            trading_program,
            TRADING_SEMANTIC_RELEASE_ID,
            &elves.trading,
            1,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        ),
        Some(SlotPinHostile::TradingUpgradeAuthority) => release_with_pin(
            trading_program,
            TRADING_SEMANTIC_RELEASE_ID,
            &elves.trading,
            0,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some([0xA4; 32]),
        ),
        _ => release(trading_program, TRADING_SEMANTIC_RELEASE_ID, &elves.trading),
    };
    let resolution_release = match slot_pin_hostile {
        Some(SlotPinHostile::LaterResolutionDeployment) => release_with_pin(
            resolution_program,
            RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            &elves.resolution,
            1,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        ),
        Some(SlotPinHostile::ResolutionUpgradeAuthority) => release_with_pin(
            resolution_program,
            RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            &elves.resolution,
            0,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some([0xA5; 32]),
        ),
        _ => release(
            resolution_program,
            RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            &elves.resolution,
        ),
    };
    let release_set = ExecutionReleaseSetV1::new(
        binding(trading_release),
        binding(trading_release),
        binding(trading_release),
        binding(resolution_release),
        binding(trading_release),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let mut activation_data = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut activation_data, content(release_set_id))
        .expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, trading_release),
        (ExecutionRoleV1::Claims, trading_release),
        (ExecutionRoleV1::Trading, trading_release),
        (ExecutionRoleV1::Resolution, resolution_release),
        (ExecutionRoleV1::Custody, trading_release),
    ] {
        activate_execution_role_into_v1(
            &mut activation_data,
            content(release_set_id),
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate role");
    }
    if matches!(slot_pin_hostile, Some(SlotPinHostile::ActivationCache)) {
        activation_data[8] ^= 1;
    }
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_id],
        &registry_program,
    )
    .0;
    test.add_account(
        activation,
        Account {
            lamports: Rent::default().minimum_balance(activation_data.len()),
            data: activation_data,
            owner: registry_program,
            executable: false,
            rent_epoch: 0,
        },
    );

    let manifest_data = manifest(trading_index);
    let manifest_digest = hash(&manifest_data).to_bytes();
    let manifest_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &manifest_digest,
        ],
        &registry_program,
    )
    .0;
    let manifest_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &manifest_digest,
        ],
        &registry_program,
    )
    .0;
    test.add_account(
        manifest_raw,
        Account {
            lamports: Rent::default().minimum_balance(manifest_data.len()),
            data: manifest_data.clone(),
            owner: registry_program,
            executable: false,
            rent_epoch: 0,
        },
    );
    let market = Pubkey::new_from_array([0x61; 32]);
    let funding_source = Pubkey::new_from_array([0x62; 32]);
    let rent_credit = Pubkey::new_from_array([0x63; 32]);
    let trading_mask = 1_u16 << trading_index;
    let resolution_mask = 0b1111 ^ trading_mask;
    let (resolution_ledger, resolution_data, resolution_lamports, resolution_principal) =
        ledger(resolution_program, market, &manifest_data, resolution_mask);
    let (trading_ledger, trading_data, trading_lamports, trading_principal) =
        ledger(trading_program, market, &manifest_data, trading_mask);
    let ordered_ledgers = if resolution_mask.trailing_zeros() < trading_mask.trailing_zeros() {
        [resolution_ledger, trading_ledger]
    } else {
        [trading_ledger, resolution_ledger]
    };
    let funding_list = generic_founding_funding_list_id_v1(
        &ordered_ledgers.map(|key| Identity::new(key.to_bytes()).expect("ledger identity")),
    )
    .expect("funding-list identity")
    .to_bytes();
    let checkpoint_input = ControllerFundingCheckpointInputV1 {
        release_set: release_set_id,
        market: market.to_bytes(),
        generation: GENERATION,
        manifest: manifest_digest,
        funding_list,
        found_request_digest: [0x64; 32],
        project_found_receipt_digest: [0x65; 32],
        resolution_ledger: resolution_ledger.to_bytes(),
        resolution_ledger_digest: hash(&resolution_data).to_bytes(),
        trading_ledger: trading_ledger.to_bytes(),
        trading_ledger_digest: hash(&trading_data).to_bytes(),
        funding_source: funding_source.to_bytes(),
        rent_credit: rent_credit.to_bytes(),
        lock_request_digest: [0x66; 32],
        expiry_slot: EXPIRY_SLOT,
        prepared_slot: 1,
        resolution_mask,
        trading_mask,
    };
    let prepared =
        ControllerFundingCheckpointV1::prepared(checkpoint_input).expect("Prepared checkpoint");
    let checkpoint_value = if staged {
        prepared
            .stage_custody(2, hashv(&[b"exact/custody/ladder/test"]).to_bytes())
            .expect("CustodyStaged checkpoint")
    } else {
        prepared
    };
    let checkpoint_data = checkpoint_value.encode().to_vec();
    let checkpoint = Pubkey::find_program_address(
        &ControllerFundingCheckpointDerivationV1::new(
            release_set_id,
            market.to_bytes(),
            GENERATION,
            manifest_digest,
            funding_list,
        )
        .expect("checkpoint derivation")
        .seed_components(),
        &trading_program,
    )
    .0;
    let checkpoint_lamports = Rent::default().minimum_balance(checkpoint_data.len());
    for (key, account) in [
        (
            checkpoint,
            Account {
                lamports: checkpoint_lamports,
                data: checkpoint_data.clone(),
                owner: trading_program,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            resolution_ledger,
            Account {
                lamports: resolution_lamports,
                data: resolution_data.clone(),
                owner: resolution_program,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            trading_ledger,
            Account {
                lamports: trading_lamports,
                data: trading_data.clone(),
                owner: trading_program,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            funding_source,
            Account {
                lamports: 1_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            rent_credit,
            Account {
                lamports: 2_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
    ] {
        test.add_account(key, account);
    }

    let abort_request = PreMarketFundingAbortRequestV1 {
        // Phase two has no Resolution-ledger cleanup request by construction.
        // This account-zero PDA is intentionally the otherwise-canonical
        // Prepared authority; Trading must refuse the phase before using it.
        checkpoint_phase: if staged {
            1
        } else {
            checkpoint_value.phase() as u8
        },
        checkpoint_revision: if staged {
            1
        } else {
            checkpoint_value.revision()
        },
        release_set: release_set_id,
        checkpoint: checkpoint.to_bytes(),
        checkpoint_digest: hash(&checkpoint_data).to_bytes(),
        market: market.to_bytes(),
        generation: GENERATION,
        manifest: manifest_digest,
        funding_list,
        selected_mask: resolution_mask,
        ledger: resolution_ledger.to_bytes(),
        ledger_account_digest: pre_market_funding_ledger_account_digest_v1(
            resolution_ledger.to_bytes(),
            resolution_program.to_bytes(),
            resolution_lamports,
            &resolution_data,
        ),
        funding_source: funding_source.to_bytes(),
        rent_credit: rent_credit.to_bytes(),
        expiry_slot: EXPIRY_SLOT,
    };
    let abort_bytes = abort_request.encode().expect("abort request");
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set_id,
        market.to_bytes(),
        ExecutionRoleV1::Trading,
        manifest_digest,
        hash(&abort_bytes).to_bytes(),
    )
    .expect("abort authority");
    let authority = Pubkey::find_program_address(&authority_seeds.as_slices(), &trading_program).0;
    let accounts = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(trading_program, false),
        AccountMeta::new_readonly(programdata(trading_program), false),
        AccountMeta::new_readonly(resolution_program, false),
        AccountMeta::new_readonly(programdata(resolution_program), false),
        AccountMeta::new(checkpoint, false),
        AccountMeta::new(resolution_ledger, false),
        AccountMeta::new(funding_source, false),
        AccountMeta::new(rent_credit, false),
        AccountMeta::new_readonly(activation, false),
        AccountMeta::new_readonly(registry_program, false),
        AccountMeta::new_readonly(manifest_raw, false),
        AccountMeta::new_readonly(manifest_staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new(trading_ledger, false),
    ];
    assert_eq!(accounts.len(), CONTROLLER_FUNDING_ABORT_ACCOUNT_COUNT_V1);
    let instruction = Instruction {
        program_id: trading_program,
        accounts,
        data: CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1.to_vec(),
    };

    Fixture {
        test: Some(test),
        trading_program,
        checkpoint,
        checkpoint_data,
        resolution_ledger,
        resolution_lamports,
        resolution_principal,
        trading_ledger,
        trading_lamports,
        trading_principal,
        funding_source,
        rent_credit,
        instruction,
        trading_closes_first: trading_mask.trailing_zeros() < resolution_mask.trailing_zeros(),
    }
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("Banks RPC")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> Snapshot {
    Snapshot {
        checkpoint: observed(context, fixture.checkpoint).await,
        resolution_ledger: observed(context, fixture.resolution_ledger).await,
        trading_ledger: observed(context, fixture.trading_ledger).await,
        funding_source: observed(context, fixture.funding_source)
            .await
            .expect("funding source"),
        rent_credit: observed(context, fixture.rent_credit)
            .await
            .expect("RentCredit"),
    }
}

fn assert_same(left: &Snapshot, right: &Snapshot, context: &str) {
    assert_eq!(left.checkpoint, right.checkpoint, "{context}: checkpoint");
    assert_eq!(
        left.resolution_ledger, right.resolution_ledger,
        "{context}: Resolution ledger"
    );
    assert_eq!(
        left.trading_ledger, right.trading_ledger,
        "{context}: Trading ledger"
    );
    assert_eq!(
        left.funding_source, right.funding_source,
        "{context}: funding source"
    );
    assert_eq!(left.rent_credit, right.rent_credit, "{context}: RentCredit");
}

async fn process(context: &mut ProgramTestContext, instruction: Instruction) -> bool {
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
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .is_ok()
}

fn cleanup_instruction(fixture: &Fixture, snapshot: &Snapshot, magic: [u8; 8]) -> Instruction {
    let checkpoint_account = snapshot.checkpoint.as_ref().expect("live checkpoint");
    let checkpoint =
        ControllerFundingCheckpointV1::decode(&checkpoint_account.data).expect("checkpoint decode");
    let resolution = snapshot.resolution_ledger.clone().unwrap_or(Account {
        lamports: 0,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    });
    let input = checkpoint.input();
    let request = PreMarketFundingAbortRequestV1 {
        checkpoint_phase: checkpoint.phase() as u8,
        checkpoint_revision: checkpoint.revision(),
        release_set: input.release_set,
        checkpoint: fixture.checkpoint.to_bytes(),
        checkpoint_digest: hash(&checkpoint_account.data).to_bytes(),
        market: input.market,
        generation: input.generation,
        manifest: input.manifest,
        funding_list: input.funding_list,
        selected_mask: input.resolution_mask,
        ledger: fixture.resolution_ledger.to_bytes(),
        ledger_account_digest: pre_market_funding_ledger_account_digest_v1(
            fixture.resolution_ledger.to_bytes(),
            resolution.owner.to_bytes(),
            resolution.lamports,
            &resolution.data,
        ),
        funding_source: input.funding_source,
        rent_credit: input.rent_credit,
        expiry_slot: input.expiry_slot,
    };
    let request_bytes = request.encode().expect("cleanup request");
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        input.release_set,
        input.market,
        ExecutionRoleV1::Trading,
        input.manifest,
        hash(&request_bytes).to_bytes(),
    )
    .expect("cleanup authority seeds");
    let authority = Pubkey::find_program_address(&seeds.as_slices(), &fixture.trading_program).0;
    let mut instruction = fixture.instruction.clone();
    instruction.accounts[0].pubkey = authority;
    instruction.data = magic.to_vec();
    instruction
}

async fn exercise_canonical_refund_order(trading_index: usize) {
    let mut fixture = fixture(trading_index, false);
    assert_eq!(fixture.trading_closes_first, trading_index == 0);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let initial = snapshot(&mut context, &fixture).await;

    assert!(
        !process(&mut context, fixture.instruction.clone()).await,
        "the Prepared close refuses before expiry"
    );
    let pre_expiry = snapshot(&mut context, &fixture).await;
    assert_same(&initial, &pre_expiry, "pre-expiry refusal");

    context
        .warp_to_slot(EXPIRY_SLOT + 1)
        .expect("past checkpoint expiry");
    assert!(
        process(&mut context, fixture.instruction.clone()).await,
        "the canonical first close persists"
    );
    let prefix = snapshot(&mut context, &fixture).await;
    let prefix_checkpoint = ControllerFundingCheckpointV1::decode(
        &prefix.checkpoint.as_ref().expect("prefix checkpoint").data,
    )
    .expect("prefix decode");
    assert!(matches!(
        prefix_checkpoint.phase(),
        dclutch_capability_contract::ControllerFundingCheckpointPhaseV1::PreparedFirstLedgerClosed
    ));
    let (remaining_key, remaining_account) = if fixture.trading_closes_first {
        (
            fixture.resolution_ledger,
            prefix
                .resolution_ledger
                .clone()
                .expect("Resolution remains live"),
        )
    } else {
        (
            fixture.trading_ledger,
            prefix.trading_ledger.clone().expect("Trading remains live"),
        )
    };
    if fixture.trading_closes_first {
        assert!(prefix.trading_ledger.is_none(), "Trading closes first");
    } else {
        assert!(
            prefix.resolution_ledger.is_none(),
            "Resolution closes first"
        );
    }
    assert!(
        !process(&mut context, fixture.instruction.clone()).await,
        "the first suffix is not replayable"
    );
    let replayed_prefix = snapshot(&mut context, &fixture).await;
    assert_same(&prefix, &replayed_prefix, "step-one replay refusal");

    let mut hostile_remaining = remaining_account.clone();
    hostile_remaining.lamports = hostile_remaining
        .lamports
        .checked_add(1)
        .expect("hostile dust");
    context.set_account(&remaining_key, &AccountSharedData::from(hostile_remaining));
    let hostile = snapshot(&mut context, &fixture).await;
    let hostile_step2 = cleanup_instruction(
        &fixture,
        &hostile,
        CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,
    );
    assert!(
        !process(&mut context, hostile_step2).await,
        "a substituted remaining prestate refuses"
    );
    let hostile_after = snapshot(&mut context, &fixture).await;
    assert_same(&hostile, &hostile_after, "step-two hostile refusal");
    context.set_account(&remaining_key, &AccountSharedData::from(remaining_account));

    let honest_prestate = snapshot(&mut context, &fixture).await;
    let total_before = initial
        .funding_source
        .lamports
        .checked_add(initial.rent_credit.lamports)
        .and_then(|value| value.checked_add(fixture.resolution_lamports))
        .and_then(|value| value.checked_add(fixture.trading_lamports))
        .and_then(|value| {
            value.checked_add(initial.checkpoint.as_ref().expect("checkpoint").lamports)
        })
        .expect("prestate lamport total");
    assert!(
        process(
            &mut context,
            cleanup_instruction(
                &fixture,
                &honest_prestate,
                CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,
            ),
        )
        .await,
        "the authenticated remaining suffix succeeds"
    );
    let closed = snapshot(&mut context, &fixture).await;
    assert!(closed.checkpoint.is_none(), "checkpoint closes last");
    assert!(
        closed.resolution_ledger.is_none(),
        "Resolution ledger closes"
    );
    assert!(closed.trading_ledger.is_none(), "Trading ledger closes");
    assert_eq!(
        closed.funding_source.lamports,
        initial
            .funding_source
            .lamports
            .checked_add(fixture.resolution_principal)
            .and_then(|value| value.checked_add(fixture.trading_principal))
            .expect("principal refunds"),
        "only classified native principal returns to the funding source"
    );
    let checkpoint_rent = initial.checkpoint.as_ref().expect("checkpoint").lamports;
    assert_eq!(
        closed.rent_credit.lamports,
        initial
            .rent_credit
            .lamports
            .checked_add(fixture.resolution_lamports - fixture.resolution_principal)
            .and_then(|value| {
                value.checked_add(fixture.trading_lamports - fixture.trading_principal)
            })
            .and_then(|value| value.checked_add(checkpoint_rent))
            .expect("Rent refunds"),
        "both exact ledger rents and checkpoint rent return to RentCredit"
    );
    assert_eq!(
        closed
            .funding_source
            .lamports
            .checked_add(closed.rent_credit.lamports)
            .expect("closed lamport total"),
        total_before,
        "no lamport remains stranded or changes classification"
    );

    let terminal = closed.clone();
    assert!(
        !process(
            &mut context,
            cleanup_instruction(&fixture, &prefix, CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,),
        )
        .await,
        "a completed close is not replayable"
    );
    let replay = snapshot(&mut context, &fixture).await;
    assert_same(&terminal, &replay, "terminal replay refusal");
}

#[tokio::test]
async fn trading_first_refund_rolls_back_then_closes_exactly() {
    exercise_canonical_refund_order(0).await;
}

#[tokio::test]
async fn resolution_first_refund_rolls_back_then_closes_exactly() {
    exercise_canonical_refund_order(3).await;
}

#[tokio::test]
async fn prepared_cleanup_cannot_consume_a_custody_staged_checkpoint() {
    let mut fixture = fixture(0, true);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    context
        .warp_to_slot(EXPIRY_SLOT + 1)
        .expect("past checkpoint expiry");
    let before = snapshot(&mut context, &fixture).await;
    assert!(
        !process(&mut context, fixture.instruction.clone()).await,
        "Prepared cleanup refuses CustodyStaged"
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_same(
        &before,
        &after,
        "phase-confused cleanup cannot strand or refund funds",
    );

    assert_eq!(
        before.checkpoint.as_ref().expect("staged checkpoint").data,
        fixture.checkpoint_data,
        "the exact CustodyStaged checkpoint survives the refused route"
    );
}

async fn assert_slot_pin_hostile_refuses(hostile: SlotPinHostile) {
    let mut fixture = fixture_with_slot_pin_hostile(3, false, Some(hostile));
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    context
        .warp_to_slot(EXPIRY_SLOT + 1)
        .expect("past checkpoint expiry");
    let before = snapshot(&mut context, &fixture).await;
    assert!(
        !process(&mut context, fixture.instruction.clone()).await,
        "slot-pin substitution must refuse before a ledger close"
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_same(&before, &after, "slot-pin substitution rollback");
}

fn seed_snapshot(test: &mut ProgramTest, keys: [Pubkey; 5], snapshot: &Snapshot) {
    let closed = Account {
        lamports: 0,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    };
    for (key, account) in [
        (keys[0], snapshot.checkpoint.as_ref()),
        (keys[1], snapshot.resolution_ledger.as_ref()),
        (keys[2], snapshot.trading_ledger.as_ref()),
        (keys[3], Some(&snapshot.funding_source)),
        (keys[4], Some(&snapshot.rent_credit)),
    ] {
        test.add_account(key, account.unwrap_or(&closed).clone());
    }
}

async fn assert_suffix_slot_pin_hostile_refuses(hostile: SlotPinHostile) {
    let mut fixture = fixture(0, false);
    let mut context = fixture
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    context
        .warp_to_slot(EXPIRY_SLOT + 1)
        .expect("past checkpoint expiry");
    assert!(
        process(&mut context, fixture.instruction.clone()).await,
        "the honest first cleanup prefix persists"
    );
    let prefix = snapshot(&mut context, &fixture).await;

    // Start a fresh bank from that exact finalized prefix with the live
    // ProgramData/cache hostile present at genesis. Replacing ProgramData in
    // a running ProgramTest bank exercises its loader cache rather than the
    // transaction's release authentication.
    let mut hostile_fixture = fixture_with_slot_pin_hostile(0, false, Some(hostile));
    let hostile_keys = [
        hostile_fixture.checkpoint,
        hostile_fixture.resolution_ledger,
        hostile_fixture.trading_ledger,
        hostile_fixture.funding_source,
        hostile_fixture.rent_credit,
    ];
    seed_snapshot(
        hostile_fixture.test.as_mut().expect("hostile ProgramTest"),
        hostile_keys,
        &prefix,
    );
    let mut hostile_context = hostile_fixture
        .test
        .take()
        .expect("unstarted hostile ProgramTest")
        .start_with_context()
        .await;
    hostile_context
        .warp_to_slot(EXPIRY_SLOT + 1)
        .expect("past checkpoint expiry");
    let before = snapshot(&mut hostile_context, &hostile_fixture).await;
    let suffix = cleanup_instruction(
        &hostile_fixture,
        &before,
        CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,
    );
    assert!(
        !process(&mut hostile_context, suffix).await,
        "the suffix must reauthenticate the current deployment before closing anything"
    );
    let after = snapshot(&mut hostile_context, &hostile_fixture).await;
    assert_same(&before, &after, "slot-pin suffix substitution rollback");
}

#[tokio::test]
async fn cleanup_refuses_substituted_deployment_slot_pin() {
    assert_slot_pin_hostile_refuses(SlotPinHostile::LaterTradingDeployment).await;
    assert_slot_pin_hostile_refuses(SlotPinHostile::LaterResolutionDeployment).await;
}

#[tokio::test]
async fn cleanup_refuses_substituted_upgrade_authority() {
    assert_slot_pin_hostile_refuses(SlotPinHostile::TradingUpgradeAuthority).await;
    assert_slot_pin_hostile_refuses(SlotPinHostile::ResolutionUpgradeAuthority).await;
}

#[tokio::test]
async fn cleanup_refuses_substituted_activation_cache() {
    assert_slot_pin_hostile_refuses(SlotPinHostile::ActivationCache).await;
}

#[tokio::test]
async fn cleanup_suffix_reauthenticates_trading_and_resolution_slot_pins() {
    for hostile in [
        SlotPinHostile::LaterTradingDeployment,
        SlotPinHostile::LaterResolutionDeployment,
        SlotPinHostile::TradingUpgradeAuthority,
        SlotPinHostile::ResolutionUpgradeAuthority,
        SlotPinHostile::ActivationCache,
    ] {
        assert_suffix_slot_pin_hostile_refuses(hostile).await;
    }
}
