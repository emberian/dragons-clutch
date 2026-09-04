//! Real-SVM adversarial closure of the split controller-funding rollback domain.
//!
//! This test deliberately starts from the exact durable `Prepared` account
//! graph rather than using a host-only close model.  The production Trading
//! program chooses one of two canonical child-close orders from the manifest
//! masks and persists the first close in the checkpoint. A later transaction
//! may only resume the authenticated suffix. This is the real runtime shape:
//! Resolution alone consumes most of the compute ceiling, so pretending both
//! closes are atomic would strand every expired founding on devnet.

use std::{collections::BTreeSet, env, fs, path::PathBuf};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CONTROLLER_FUNDING_CUSTODY_ABORT_ANCHOR_DOMAIN_V1,
    CONTROLLER_FUNDING_CUSTODY_LADDER_ACCOUNT_COUNT_V1,
    CONTROLLER_FUNDING_CUSTODY_LADDER_DIGEST_DOMAIN_V1, CapabilityEntryV1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, CompartmentFundingV1,
    ContentId as CapabilityContentId, ControllerFundingCheckpointDerivationV1,
    ControllerFundingCheckpointInputV1, ControllerFundingCheckpointV1, FundingAmountsV1,
    FundingLedgerV2, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    funding_ledger_bytes_v2,
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    CustodyReplayV1, CustodyVaultSeedsV1, FoundingPrestateStageV1,
    OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1, PROJECTED_CUSTODY_STATE_BYTES_V2,
    ProjectedCallerRoleV1, ProjectedCustodyCallerSeedsV1, ProjectedCustodyOperationV1,
    ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1, ProjectedCustodySourceReplaySeedsV1,
    ProjectedCustodyStateSeedsV2, ProjectedCustodyStateV2, SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
};
use dclutch_market_core_codec::{Identity, generic_founding_funding_list_id_v1};
use dclutch_program_test_evidence::TransactionEvidence;
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
use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2,
        LifecycleRentCreditV2,
    },
};
use dclutch_resolution_codec::{
    PreMarketFundingAbortRequestV1, RESOLUTION_CONTROLLER_RELEASE_ID_V7,
    pre_market_funding_ledger_account_digest_v1,
};
use dclutch_token_svm::{ACCOUNT_BYTES, PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID};
use solana_account::{Account, AccountSharedData};
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x41; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x42; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x43; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x44; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0x45; 32]);
const GENERATION: u64 = 7;
const EXPIRY_SLOT: u64 = 3;
const CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1: [u8; 8] = *b"DCLTCF1A";
const CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1: [u8; 8] = *b"DCLTCF2A";
const CONTROLLER_FUNDING_ABORT_ACCOUNT_COUNT_V1: usize = 17;
const PROJECTED_CUSTODY_ABORT_MAGIC_V1: [u8; 8] = *b"DCLTPCA1";
const PROJECTED_CUSTODY_STAGED_ABORT_ACCOUNT_COUNT_V2: usize = 36;
const PROJECTED_CUSTODY_ABORT_ACCOUNT_COUNT_V1: usize =
    PROJECTED_CUSTODY_STAGED_ABORT_ACCOUNT_COUNT_V2 - CONTROLLER_FUNDING_ABORT_ACCOUNT_COUNT_V1;
const TRADING_SEMANTIC_RELEASE_ID: [u8; 32] = [0x71; 32];
const CUSTODY_SEMANTIC_RELEASE_ID: [u8; 32] = [0x72; 32];
const SOURCE_ABORT_PRINCIPAL: u64 = 500;

struct Elves {
    trading: Vec<u8>,
    resolution: Vec<u8>,
    registry: Vec<u8>,
    custody: Vec<u8>,
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
        custody: fs::read(directory.join("dclutch_custody_sbf.so")).expect("Custody ELF"),
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
    add_program(
        &mut test,
        "dclutch_custody_sbf",
        CUSTODY_PROGRAM_ID,
        &elves.custody,
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
    let custody_release = release(
        CUSTODY_PROGRAM_ID,
        CUSTODY_SEMANTIC_RELEASE_ID,
        &elves.custody,
    );
    let release_set = ExecutionReleaseSetV1::new(
        binding(trading_release),
        binding(trading_release),
        binding(trading_release),
        binding(resolution_release),
        binding(custody_release),
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
        (ExecutionRoleV1::Custody, custody_release),
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
    process_with_signers(context, instruction, &[]).await
}

async fn process_with_signers(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signers: &[&Keypair],
) -> bool {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let mut all_signers = Vec::with_capacity(signers.len() + 1);
    all_signers.push(&context.payer);
    all_signers.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &all_signers,
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .is_ok()
}

/// Submit one labelled step, record what the chain did, and say whether it
/// succeeded.
///
/// The plain `process_with_signers` above is deliberately left alone. Only the
/// steps `tools/gauntlet/source-abort/bindings.json` names come through here,
/// and each label names exactly one transaction -- which is what lets a binding
/// carry one outcome and one refusal code. Every other test in this file stays
/// an ordinary test and records nothing, so a campaign run that folds this
/// binary cannot pick up a transaction no binding owns.
///
/// The evidence is written BEFORE the caller asserts anything, so a step that
/// fails its own assertion still leaves behind what the chain did.
///
/// `wire_bytes` is MEASURED, not enforced. ProgramTest submits no packet, so it
/// cannot refuse a frame that overruns Solana's legacy maximum -- Found31 was
/// exactly that defect and it survived every fixture test in the tree. Recording
/// the extent is what lets a witness ask the question the runtime here cannot.
async fn process_recorded(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signers: &[&Keypair],
    label: &str,
) -> bool {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let mut all_signers = Vec::with_capacity(signers.len() + 1);
    all_signers.push(&context.payer);
    all_signers.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &all_signers,
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message.serialize().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("Banks RPC");
    let outcome = processed.result;
    let failure = outcome.as_ref().err().map(|error| format!("{error:?}"));
    let logs = processed
        .metadata
        .as_ref()
        .map(|value| value.log_messages.clone())
        .unwrap_or_default();
    let units = processed
        .metadata
        .as_ref()
        .map_or(0, |value| value.compute_units_consumed);
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
    outcome.is_ok()
}

fn token_mint_data(supply: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: COption::None,
            supply,
            decimals: 6,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("Token-2022 Mint");
    bytes
}

fn token_account_data(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0_u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("Token-2022 account");
    bytes
}

fn token_amount(account: &Account) -> u64 {
    SplAccount::unpack(&account.data)
        .expect("Token-2022 account")
        .amount
}

fn custody_ladder_digest(observations: &[(Pubkey, Pubkey, u64, &[u8])]) -> [u8; 32] {
    // This helper is the fourth author of a preimage the chain also builds in
    // generic_market_founding_v1 and projected_custody_bootstrap_v1, where the
    // arity is structural (`[_; CONTROLLER_FUNDING_CUSTODY_LADDER_ACCOUNT_COUNT_V1]`).
    // Here it was a slice, so this side alone would accept a three- or
    // five-account ladder and hash it happily -- and a hostile that builds the
    // wrong ladder would then fail on a digest mismatch, which reads as "the
    // program refused" rather than "my fixture was the wrong shape". Name the
    // arity here too, so the test harness cannot disagree with the chain about
    // what a ladder IS.
    assert_eq!(
        observations.len(),
        CONTROLLER_FUNDING_CUSTODY_LADDER_ACCOUNT_COUNT_V1,
        "the Custody ladder digest commits exactly this many accounts"
    );
    let mut preimage = Vec::new();
    preimage.extend_from_slice(CONTROLLER_FUNDING_CUSTODY_LADDER_DIGEST_DOMAIN_V1);
    for (key, owner, lamports, data) in observations {
        preimage.extend_from_slice(key.as_ref());
        preimage.extend_from_slice(owner.as_ref());
        preimage.extend_from_slice(&lamports.to_le_bytes());
        preimage.extend_from_slice(
            &u64::try_from(data.len())
                .expect("account width")
                .to_le_bytes(),
        );
        preimage.extend_from_slice(data);
    }
    hash(&preimage).to_bytes()
}

struct SourceAbortFixture {
    base: Fixture,
    beneficiary: Keypair,
    instruction: Instruction,
    projected_state: Pubkey,
    source_vault: Pubkey,
    source_replay: Pubkey,
    hoard_vault: Pubkey,
    destination: Pubkey,
    custody_rent_total: u64,
}

fn source_abort_fixture() -> SourceAbortFixture {
    let mut base = fixture(0, true);
    let beneficiary = Keypair::new();
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let mint = Pubkey::new_from_array([0xb1; 32]);
    let destination = Pubkey::new_from_array([0xb2; 32]);
    let checkpoint =
        ControllerFundingCheckpointV1::decode(&base.checkpoint_data).expect("staged checkpoint");
    let mut input = checkpoint.input();
    let release_set = input.release_set;
    let market = Pubkey::new_from_array(input.market);
    let generation = input.generation;
    let generation_bytes = generation.to_le_bytes();
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation_bytes,
        ],
        &RENT_PROGRAM_ID,
    );
    let rent = Rent::default();
    let state_rent = rent.minimum_balance(PROJECTED_CUSTODY_STATE_BYTES_V2);
    let vault_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let source_replay_rent = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);
    let collateral_release = PRODUCTION_ADAPTER_RELEASES
        .iter()
        .copied()
        .find(|release| release.token_program() == TOKEN_2022_PROGRAM_ID)
        .expect("Token-2022 release");
    let mut lock = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        caller_role: ProjectedCallerRoleV1::TradingCapability,
        market: market.to_bytes(),
        generation,
        realm: [0xc1; 32],
        product_record: [0xc2; 32],
        product: [0xc3; 32],
        source: [0xc4; 32],
        release_set,
        projection_receipt_digest: [0xc5; 32],
        parent_capability_root: [0xc6; 32],
        context_digest: [0xc7; 32],
        caller_program: base.trading_program.to_bytes(),
        payer: [0xc8; 32],
        core_program: base.trading_program.to_bytes(),
        rent_program: RENT_PROGRAM_ID.to_bytes(),
        refund_owner: beneficiary.pubkey().to_bytes(),
        rent_credit: rent_credit.to_bytes(),
        hoard_vault: [0xc9; 32],
        funding_source_vault: [0xca; 32],
        funding_source_context: [0xcb; 32],
        funding_source_compartment: CompartmentV1::Settlement,
        mint: mint.to_bytes(),
        token_program: token_program.to_bytes(),
        collateral_release: hash(&collateral_release.to_bytes()).to_bytes(),
        expiry_slot: EXPIRY_SLOT,
        expected_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1,
        resulting_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1 + 1,
        amount: SOURCE_ABORT_PRINCIPAL,
        state_rent_lamports: state_rent,
        vault_rent_lamports: vault_rent,
        funding_source_replay_revision: SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
        funding_source_state_rent_lamports: source_replay_rent,
        funding_source_vault_rent_lamports: vault_rent,
    };
    let (projected_state, projected_bump) = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::from_request(lock).as_slices(),
        &CUSTODY_PROGRAM_ID,
    );
    let hoard_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            lock.market,
            lock.release_set,
            lock.context_digest,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let source_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            lock.market,
            lock.release_set,
            lock.funding_source_context,
            lock.funding_source_compartment,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    lock.hoard_vault = hoard_vault.to_bytes();
    lock.funding_source_vault = source_vault.to_bytes();
    lock.validate().expect("terminal projected Lock");
    let source_replay = Pubkey::find_program_address(
        &ProjectedCustodySourceReplaySeedsV1::from_request(lock).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &lock.market,
            &lock.release_set,
        ],
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let open_source = lock
        .founding_prestate_stage_v1(FoundingPrestateStageV1::OpenSourceCompartment)
        .expect("OpenSourceCompartment request");
    let open_source_bytes = open_source.encode().expect("OpenSourceCompartment bytes");
    let open_source_digest = hash(&open_source_bytes).to_bytes();
    let source_poststate_commitment = hashv(&[
        &open_source_digest,
        source_vault.as_ref(),
        source_replay.as_ref(),
        &SOURCE_ABORT_PRINCIPAL.to_le_bytes(),
    ])
    .to_bytes();
    let projected_data = ProjectedCustodyStateV2 {
        phase: ProjectedCustodyPhaseV1::SourceFunded,
        request: open_source,
        next_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1,
        locked_amount: SOURCE_ABORT_PRINCIPAL,
        principal_cap_sets: u64::MAX,
        last_request_digest: open_source_digest,
        bump: projected_bump,
    }
    .encode()
    .expect("SourceFunded state")
    .to_vec();
    let source_replay_data = CustodyReplayV1 {
        caller_role: CallerRoleV1::Trading,
        release_set,
        market: market.to_bytes(),
        realm: lock.realm,
        context: lock.funding_source_context,
        caller_program: base.trading_program.to_bytes(),
        rent_refund: rent_credit.to_bytes(),
        open_vault_count: 1,
        next_revision: SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
        generation,
        last_request_digest: open_source_digest,
        last_poststate_commitment: source_poststate_commitment,
    }
    .to_bytes()
    .expect("source replay")
    .to_vec();
    let source_data = token_account_data(mint, custody_authority, SOURCE_ABORT_PRINCIPAL);
    let hoard_data = token_account_data(mint, custody_authority, 0);
    let destination_data = token_account_data(mint, beneficiary.pubkey(), 40);
    let ladder_digest = custody_ladder_digest(&[
        (
            projected_state,
            CUSTODY_PROGRAM_ID,
            state_rent,
            &projected_data,
        ),
        (hoard_vault, token_program, vault_rent, &hoard_data),
        (source_vault, token_program, vault_rent, &source_data),
        (
            source_replay,
            CUSTODY_PROGRAM_ID,
            source_replay_rent,
            &source_replay_data,
        ),
    ]);
    let lock_bytes = lock.encode().expect("terminal Lock bytes");
    input.rent_credit = rent_credit.to_bytes();
    input.lock_request_digest = hash(&lock_bytes).to_bytes();
    let staged = ControllerFundingCheckpointV1::prepared(input)
        .expect("Prepared checkpoint")
        .stage_custody(2, ladder_digest)
        .expect("exact CustodyStaged checkpoint");
    base.checkpoint_data = staged.encode().to_vec();
    base.rent_credit = rent_credit;
    let checkpoint_lamports = rent.minimum_balance(base.checkpoint_data.len());
    base.test
        .as_mut()
        .expect("unstarted ProgramTest")
        .add_account(
            base.checkpoint,
            Account {
                lamports: checkpoint_lamports,
                data: base.checkpoint_data.clone(),
                owner: base.trading_program,
                executable: false,
                rent_epoch: 0,
            },
        );
    let credit_data = LifecycleRentCreditV2::new(
        RefundAuthority::new([0xcc; 32]).expect("refund wallet"),
        LifecycleAccountIdV2::new(market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release_set).expect("release set"),
        generation,
        rent_credit_bump,
    )
    .expect("LifecycleRentCredit")
    .to_bytes()
    .to_vec();
    for (key, account) in [
        (
            rent_credit,
            Account {
                lamports: rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2),
                data: credit_data,
                owner: RENT_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            projected_state,
            Account {
                lamports: state_rent,
                data: projected_data,
                owner: CUSTODY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            source_vault,
            Account {
                lamports: vault_rent,
                data: source_data,
                owner: token_program,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            source_replay,
            Account {
                lamports: source_replay_rent,
                data: source_replay_data,
                owner: CUSTODY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            hoard_vault,
            Account {
                lamports: vault_rent,
                data: hoard_data,
                owner: token_program,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            destination,
            Account {
                lamports: rent.minimum_balance(ACCOUNT_BYTES),
                data: destination_data,
                owner: token_program,
                executable: false,
                rent_epoch: 0,
            },
        ),
        (
            mint,
            Account {
                lamports: rent.minimum_balance(SplMint::LEN),
                data: token_mint_data(SOURCE_ABORT_PRINCIPAL + 40),
                owner: token_program,
                executable: false,
                rent_epoch: 0,
            },
        ),
    ] {
        base.test
            .as_mut()
            .expect("unstarted ProgramTest")
            .add_account(key, account);
    }
    let lock_raw = Pubkey::new_from_array([0xb3; 32]);
    base.test
        .as_mut()
        .expect("unstarted ProgramTest")
        .add_account(
            lock_raw,
            Account {
                lamports: rent.minimum_balance(lock_bytes.len()),
                data: lock_bytes.to_vec(),
                owner: REGISTRY_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    let abort = lock.founding_source_abort_v1();
    let abort_bytes = abort.encode().expect("abort bytes");
    let caller = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(abort, hash(&abort_bytes).to_bytes()).as_slices(),
        &base.trading_program,
    )
    .0;

    let checkpoint_digest = hash(&base.checkpoint_data).to_bytes();
    let authority = Pubkey::find_program_address(
        &[
            CONTROLLER_FUNDING_CUSTODY_ABORT_ANCHOR_DOMAIN_V1,
            base.checkpoint.as_ref(),
            &checkpoint_digest,
        ],
        &base.trading_program,
    )
    .0;
    base.instruction.accounts[0].pubkey = authority;
    base.instruction.accounts[8].pubkey = rent_credit;
    let mut accounts = vec![
        AccountMeta::new_readonly(lock_raw, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata(CUSTODY_PROGRAM_ID), false),
        AccountMeta::new_readonly(caller, false),
        AccountMeta::new(projected_state, false),
        base.instruction.accounts[9].clone(),
        base.instruction.accounts[10].clone(),
        AccountMeta::new_readonly(base.trading_program, false),
        AccountMeta::new_readonly(programdata(base.trading_program), false),
        AccountMeta::new(rent_credit, false),
        AccountMeta::new(source_vault, false),
        AccountMeta::new(source_replay, false),
        AccountMeta::new(hoard_vault, false),
        AccountMeta::new(destination, false),
        AccountMeta::new_readonly(beneficiary.pubkey(), true),
        AccountMeta::new_readonly(custody_authority, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(market, false),
    ];
    accounts.extend(base.instruction.accounts.iter().cloned());
    assert_eq!(
        accounts.len(),
        PROJECTED_CUSTODY_STAGED_ABORT_ACCOUNT_COUNT_V2
    );
    let instruction = Instruction {
        program_id: base.trading_program,
        accounts,
        data: PROJECTED_CUSTODY_ABORT_MAGIC_V1.to_vec(),
    };
    SourceAbortFixture {
        base,
        beneficiary,
        instruction,
        projected_state,
        source_vault,
        source_replay,
        hoard_vault,
        destination,
        custody_rent_total: state_rent
            .checked_add(vault_rent)
            .and_then(|value| value.checked_add(source_replay_rent))
            .and_then(|value| value.checked_add(vault_rent))
            .expect("Custody rent total"),
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceAbortSnapshot {
    checkpoint: Option<Account>,
    resolution_ledger: Option<Account>,
    trading_ledger: Option<Account>,
    funding_source: Account,
    rent_credit: Account,
    projected_state: Option<Account>,
    source_vault: Option<Account>,
    source_replay: Option<Account>,
    hoard_vault: Option<Account>,
    destination: Account,
}

async fn source_abort_snapshot(
    context: &mut ProgramTestContext,
    fixture: &SourceAbortFixture,
) -> SourceAbortSnapshot {
    SourceAbortSnapshot {
        checkpoint: observed(context, fixture.base.checkpoint).await,
        resolution_ledger: observed(context, fixture.base.resolution_ledger).await,
        trading_ledger: observed(context, fixture.base.trading_ledger).await,
        funding_source: observed(context, fixture.base.funding_source)
            .await
            .expect("controller funding source"),
        rent_credit: observed(context, fixture.base.rent_credit)
            .await
            .expect("LifecycleRentCredit"),
        projected_state: observed(context, fixture.projected_state).await,
        source_vault: observed(context, fixture.source_vault).await,
        source_replay: observed(context, fixture.source_replay).await,
        hoard_vault: observed(context, fixture.hoard_vault).await,
        destination: observed(context, fixture.destination)
            .await
            .expect("refund destination"),
    }
}

fn source_abort_lamport_total(snapshot: &SourceAbortSnapshot) -> u64 {
    [
        snapshot.checkpoint.as_ref(),
        snapshot.resolution_ledger.as_ref(),
        snapshot.trading_ledger.as_ref(),
        Some(&snapshot.funding_source),
        Some(&snapshot.rent_credit),
        snapshot.projected_state.as_ref(),
        snapshot.source_vault.as_ref(),
        snapshot.source_replay.as_ref(),
        snapshot.hoard_vault.as_ref(),
        Some(&snapshot.destination),
    ]
    .into_iter()
    .flatten()
    .try_fold(0_u64, |total, account| total.checked_add(account.lamports))
    .expect("closed-domain lamport total")
}

#[tokio::test]
async fn real_custody_source_abort_then_controller_suffix_is_exact_and_resumable() {
    let mut fixture = source_abort_fixture();
    assert_eq!(
        fixture.instruction.accounts.len(),
        PROJECTED_CUSTODY_STAGED_ABORT_ACCOUNT_COUNT_V2
    );
    let mut complete_keys =
        BTreeSet::from([fixture.base.trading_program, fixture.beneficiary.pubkey()]);
    complete_keys.extend(fixture.instruction.accounts.iter().map(|meta| meta.pubkey));
    // The compiled successor transaction also carries the distinct payer and
    // ComputeBudget program. The beneficiary and Trading program are already
    // physical metas in this frame.
    assert_eq!(
        complete_keys.len() + 2,
        33,
        "exact DCLTPCA1 complete-key census"
    );
    assert_eq!(
        fixture
            .instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_signer)
            .count()
            + 1,
        2,
        "payer plus refund owner are the exact signatures"
    );
    let mut context = fixture
        .base
        .test
        .take()
        .expect("unstarted ProgramTest")
        .start_with_context()
        .await;
    let before = source_abort_snapshot(&mut context, &fixture).await;
    let initial_total = source_abort_lamport_total(&before);
    let destination_before = token_amount(&before.destination);
    assert_eq!(
        token_amount(before.source_vault.as_ref().expect("funded source")),
        SOURCE_ABORT_PRINCIPAL
    );

    assert!(
        !process_recorded(
            &mut context,
            fixture.instruction.clone(),
            &[&fixture.beneficiary],
            "DCLTPCA1 refuses to abort a funded source before expiry",
        )
        .await,
        "DCLTPCA1 must refuse while the founding is still satisfiable"
    );
    assert_eq!(
        source_abort_snapshot(&mut context, &fixture).await,
        before,
        "the pre-expiry Custody CPI and checkpoint write roll back together"
    );

    context
        .warp_to_slot(EXPIRY_SLOT + 1)
        .expect("past SourceAbort expiry");
    let expired = source_abort_snapshot(&mut context, &fixture).await;
    let mut wrong_anchor = fixture.instruction.clone();
    wrong_anchor.accounts[PROJECTED_CUSTODY_ABORT_ACCOUNT_COUNT_V1].pubkey =
        sysvar::instructions::ID;
    assert!(
        !process_recorded(
            &mut context,
            wrong_anchor,
            &[&fixture.beneficiary],
            "DCLTPCA1 refuses an unrelated Custody anchor after expiry",
        )
        .await,
        "DCLTPCA1 refuses an unrelated phase-2 anchor"
    );
    assert_eq!(
        source_abort_snapshot(&mut context, &fixture).await,
        expired,
        "anchor substitution cannot enter Custody or advance the checkpoint"
    );
    // The one hostile here that Trading ADMITS. Trading authenticates the Custody
    // sub-frame's programs, its ProgramData and the activated release set; where
    // the principal lands is Custody's to guard, and `abort_source_and_close`
    // refuses `destination.key == hoard.key` by name. So this is the campaign's
    // only evidence that the child was really entered rather than rejected on the
    // way in -- the chain must report CUSTODY's code, at depth two, not Trading's.
    //
    // A destination the token program does not own was tried first and is NOT what
    // this asserts: measured 2026-09-03, Trading refuses that one before the CPI
    // with its own `Content`, so it proves nothing about Custody. The Hoard is the
    // substitution that gets through, and it is the better accusation anyway --
    // routing the refund into the vault the abort is closing would conserve the
    // lamports and lose the principal.
    //
    // The index is found rather than written down: a literal here would silently
    // follow the frame if it moved.
    let destination_index = fixture
        .instruction
        .accounts
        .iter()
        .position(|meta| meta.pubkey == fixture.destination)
        .expect("the refund destination is a physical meta of the DCLTPCA1 frame");
    let mut hoard_destination = fixture.instruction.clone();
    hoard_destination.accounts[destination_index].pubkey = fixture.hoard_vault;
    assert!(
        !process_recorded(
            &mut context,
            hoard_destination,
            &[&fixture.beneficiary],
            "DCLTPCA1 refuses to route the refund into the Hoard it is closing",
        )
        .await,
        "Custody must refuse a refund destination that is the Hoard being closed"
    );
    assert_eq!(
        source_abort_snapshot(&mut context, &fixture).await,
        expired,
        "a refused Custody CPI leaves the funded projection exactly as it was"
    );
    assert!(
        process_recorded(
            &mut context,
            fixture.instruction.clone(),
            &[&fixture.beneficiary],
            "unwind an expired founding's funded source compartment (DCLTPCA1)",
        )
        .await,
        "real Custody abort persists before controller cleanup"
    );
    let after_abort = source_abort_snapshot(&mut context, &fixture).await;
    assert_eq!(
        ControllerFundingCheckpointV1::decode(
            &after_abort
                .checkpoint
                .as_ref()
                .expect("abort checkpoint")
                .data,
        )
        .expect("abort checkpoint")
        .phase(),
        dclutch_capability_contract::ControllerFundingCheckpointPhaseV1::CustodyAborted
    );
    assert!(after_abort.projected_state.is_none());
    assert!(after_abort.source_vault.is_none());
    assert!(after_abort.source_replay.is_none());
    assert!(after_abort.hoard_vault.is_none());
    assert_eq!(
        token_amount(&after_abort.destination),
        destination_before + SOURCE_ABORT_PRINCIPAL,
        "only the original supplier receives the exact principal"
    );
    assert_eq!(
        after_abort.rent_credit.lamports,
        before.rent_credit.lamports + fixture.custody_rent_total,
        "all four Custody rents return to the lifecycle credit"
    );
    assert_eq!(after_abort.resolution_ledger, before.resolution_ledger);
    assert_eq!(after_abort.trading_ledger, before.trading_ledger);
    assert_eq!(source_abort_lamport_total(&after_abort), initial_total);

    assert!(
        !process_recorded(
            &mut context,
            fixture.instruction.clone(),
            &[&fixture.beneficiary],
            "DCLTPCA1 refuses to replay a finalized Custody prefix",
        )
        .await,
        "a finalized Custody prefix is not replayable"
    );
    assert_eq!(
        source_abort_snapshot(&mut context, &fixture).await,
        after_abort,
        "prefix replay cannot disturb the crash-recovery phase owner"
    );

    let abort_base = snapshot(&mut context, &fixture.base).await;
    let first = cleanup_instruction(
        &fixture.base,
        &abort_base,
        CONTROLLER_FUNDING_CLEANUP_STEP1_MAGIC_V1,
    );
    assert!(
        process_recorded(
            &mut context,
            first,
            &[],
            "DCLTCF1A closes the canonical first controller ledger",
        )
        .await,
        "a new process may resume the canonical first ledger close"
    );
    let first_closed = source_abort_snapshot(&mut context, &fixture).await;
    assert_eq!(
        ControllerFundingCheckpointV1::decode(
            &first_closed
                .checkpoint
                .as_ref()
                .expect("first-close checkpoint")
                .data,
        )
        .expect("first-close checkpoint")
        .phase(),
        dclutch_capability_contract::ControllerFundingCheckpointPhaseV1::CustodyFirstLedgerClosed
    );
    assert_eq!(source_abort_lamport_total(&first_closed), initial_total);

    let first_base = snapshot(&mut context, &fixture.base).await;
    let second = cleanup_instruction(
        &fixture.base,
        &first_base,
        CONTROLLER_FUNDING_CLEANUP_STEP2_MAGIC_V1,
    );
    assert!(
        process_recorded(
            &mut context,
            second,
            &[],
            "DCLTCF2A closes the authenticated remaining suffix",
        )
        .await,
        "a second restart closes only the authenticated remaining suffix"
    );
    let terminal = source_abort_snapshot(&mut context, &fixture).await;
    assert!(terminal.checkpoint.is_none());
    assert!(terminal.resolution_ledger.is_none());
    assert!(terminal.trading_ledger.is_none());
    assert_eq!(source_abort_lamport_total(&terminal), initial_total);
    assert_eq!(
        terminal.funding_source.lamports,
        before.funding_source.lamports
            + fixture.base.resolution_principal
            + fixture.base.trading_principal,
        "only controller-native principal returns to its immutable source"
    );
    let controller_rent = (fixture.base.resolution_lamports - fixture.base.resolution_principal)
        + (fixture.base.trading_lamports - fixture.base.trading_principal)
        + before.checkpoint.as_ref().expect("checkpoint").lamports;
    assert_eq!(
        terminal.rent_credit.lamports,
        before.rent_credit.lamports + fixture.custody_rent_total + controller_rent,
        "Custody, both ledger rents, and checkpoint rent retain one owner"
    );
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
